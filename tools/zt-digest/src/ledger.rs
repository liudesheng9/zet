//! One prepared book's work and evidence. No Card prose or second write authority.
mod adopt;
use crate::event::layer::prop;
use crate::link_gap;
use crate::ps::{self, list, pipe, s, truthy};
use crate::{args::Args, canon, report, util, zt::Zt};
use anyhow::{Context, Result, bail};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, 
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::Duration,
};

const SCHEMA: &str = r#"
CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE units(
 id TEXT PRIMARY KEY, layer TEXT NOT NULL, logical_key TEXT NOT NULL,
 revision INTEGER NOT NULL CHECK(revision>0),
 state TEXT NOT NULL CHECK(state IN ('registered','ready','claimed','retry','failed','blocked','done')),
 attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts>=0), owner TEXT,
 claim_token INTEGER NOT NULL DEFAULT 0 CHECK(claim_token>=0), lease_until INTEGER, retry_after INTEGER,
 fault_class TEXT, fault_json TEXT CHECK(fault_json IS NULL OR json_valid(fault_json)),
 inputs_json TEXT NOT NULL CHECK(json_valid(inputs_json)), input_hash TEXT NOT NULL,
 progress_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(progress_json)),
 result_json TEXT CHECK(result_json IS NULL OR json_valid(result_json)),
 created_by TEXT REFERENCES units(id), created_at TEXT NOT NULL, completed_at TEXT,
 UNIQUE(layer,logical_key,revision));
CREATE TABLE deps(unit_id TEXT NOT NULL REFERENCES units(id), needs_id TEXT NOT NULL REFERENCES units(id),
 condition TEXT NOT NULL CHECK(condition IN ('success','settled')), PRIMARY KEY(unit_id,needs_id),
 CHECK(unit_id<>needs_id));
CREATE TABLE outputs(unit_id TEXT NOT NULL REFERENCES units(id), output_key TEXT NOT NULL,
 kind TEXT NOT NULL, locator TEXT NOT NULL, fingerprint TEXT NOT NULL,
 data_json TEXT NOT NULL CHECK(json_valid(data_json)), PRIMARY KEY(unit_id,output_key));
CREATE TABLE events(seq INTEGER PRIMARY KEY, unit_id TEXT REFERENCES units(id), invocation INTEGER NOT NULL,
 utc TEXT NOT NULL, owner TEXT, claim_token INTEGER, event TEXT NOT NULL,
 facts_json TEXT NOT NULL CHECK(json_valid(facts_json)));
CREATE INDEX units_ready ON units(layer,state,retry_after);
CREATE INDEX deps_needs ON deps(needs_id);
CREATE TRIGGER events_no_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT,'events are append-only'); END;
CREATE TRIGGER events_no_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT,'events are append-only'); END;
PRAGMA user_version=1;
"#;

const COMMON: &[&str] = &[
    "prepare",
    "terms",
    "digest",
    "cards",
    "cards-book",
    "models",
    "explain",
    "link",
    "link-check",
    "notes",
    "organize",
    "skeleton",
    "hubs",
    "figures",
    "bridge",
    "audit",
    "cleanup",
];

const MATH: &[&str] = &[
    "math-index",
    "math-docs",
    "math-skeleton",
    "notation",
    "boundary",
    "proof",
    "theory",
    "math-check",
];
const BOOK: &[&str] = &[
    "cards-book",
    "link-check",
    "organize",
    "skeleton",
    "figures",
    "bridge",
    "audit",
];
const MATH_BOOK: &[&str] = &[
    "math-index",
    "math-docs",
    "math-skeleton",
    "notation",
    "theory",
    "math-check",
];

/// The mathematics variant's per-unit obligations: one ledger unit per planned
/// mathematical unit, registered from the compiler's retained plan (R239, R240).
const MATH_OBLIGATIONS: &[&str] = &["boundary", "proof"];

/// The layer whose checked result a mathematics layer consumes, in the variant's
/// declared stage order. The notation table precedes the proof and model work that
/// depends on it, and the M1–M18 check describes the final audit revision (R238, R241).
fn math_prerequisite(layer: &str) -> Option<&'static str> {
    Some(match layer {
        "math-index" => return None,
        "math-docs" => "math-index",
        "math-skeleton" => "math-docs",
        "notation" => "math-skeleton",
        "math-check" => "audit",
        _ => return None,
    })
}

/// The rule a recorded whole-book Card cycle left out, if any. A report written
/// before the standalone rules of #93 S02 joined the default set is a complete
/// deterministic pass over the rules it ran, so it still completes its own run;
/// a report written now must name every current default rule.
fn card_cycle_rule_missing<'a>(declared: &std::collections::BTreeSet<&str>) -> Option<&'a str> {
    /// The Card cycle's default rule set before #93 S02.
    const PRIOR_CARD_RULES: &str = "R2,R3,R4,R5,R7,R9,R12,R18,R19,I1,I2,I3,I4,L1,L2,E1,E2";
    let complete = |rules: &'a str| rules.split(',').all(|rule| declared.contains(rule));
    if complete(crate::card::DEFAULT_CARD_RULES) || complete(PRIOR_CARD_RULES) {
        return None;
    }
    crate::card::DEFAULT_CARD_RULES
        .split(',')
        .find(|rule| !declared.contains(rule))
}

const RUN_FLAGS: &[&str] = &[
    "terms", "models", "events", "findings", "hubs", "figures", "bridge", "notation", "boundary",
    "proof", "theory",
];

/// The publication contract version is part of every aggregate input manifest, so
/// a changed derivation contract cannot silently reuse an older revision.
const PUBLICATION_CONTRACT: i64 = 1;

/// Aggregate publication layers. Each gains a new immutable revision when its
/// sorted input manifest changes; per-sentence or per-group aggregates stay out.
/// The mathematics `theory` layer is one of them: a Theory Card's checked
/// membership/dependency summary is rebuilt after its units or cited-premise
/// Links change, and it is published after link-check and before Organize (R241).
const PUBLICATION: &[&str] = &[
    "cards-book",
    "link-check",
    "organize",
    "skeleton",
    "figures",
    "bridge",
    "audit",
    "hubs",
    "theory",
];

/// The publisher layers whose recorded work forms one aggregate publication input.
/// Every publication also reads the Model candidates its groups registered, so a
/// repaired or newly built model changes the input set of every downstream
/// revision. The whole-book Card and Link cycles additionally enumerate their own
/// child layer, and Organize also reads the registered note suggestions it must
/// dispose. A producer that later emits nothing still changes the manifest.
/// Checked explanation work edits model Card bodies, so every aggregate published
/// after it consumes those fingerprints too (R219, R301).
fn publication_sources(layer: &str) -> Option<&'static [&'static str]> {
    match layer {
        "cards-book" => Some(&["digest", "models", "cards"]),
        "link-check" => Some(&["digest", "models", "link", "explain"]),
        "organize" => Some(&["digest", "models", "notes", "explain"]),
        // A Theory Card's summary depends on the units (their Models) and on the
        // cited-premise Link pass, which the prerequisite revision names.
        "theory" => Some(&["digest", "models", "explain"]),
        "skeleton" | "figures" | "bridge" | "audit" | "hubs" => {
            Some(&["digest", "models", "explain"])
        }
        _ => None,
    }
}

/// The aggregate publications that consume a checked explanation revision, recorded
/// in its own evidence so the emitted work names its downstream inputs.
const EXPLANATION_CONSUMERS: &[&str] = &[
    "link-check",
    "organize",
    "skeleton",
    "figures",
    "bridge",
    "audit",
    "hubs",
    "theory",
];

/// The publication whose checked revision this layer consumes: the mathematics theory
/// revision follows link-check, and Organize follows theory, so a changed revision of
/// the earlier layer is a changed input of the later one (R241).
fn publication_prerequisite(conn: &Connection, layer: &str) -> Result<Option<String>> {
    let Some(previous) = (match layer {
        "theory" => Some("link-check"),
        "organize" => Some("theory"),
        _ => None,
    }) else {
        return Ok(None);
    };
    let target = meta(conn, "target")?;
    let enabled = target["enabled"]
        .as_array()
        .is_some_and(|layers| layers.iter().any(|l| l == previous));
    if !enabled {
        return Ok(None);
    }
    Ok(required_publication(conn, previous)?.map(|(id, _)| id))
}

/// Flat effective choices use the existing workflow flag names. Core verification stays required.
pub(crate) fn run_options(variant: &str, no_figures: bool, input: Option<&Value>) -> Result<Value> {
    if !["econ", "math"].contains(&variant) {
        bail!("unknown run variant `{variant}`");
    }
    let mut options = serde_json::Map::new();
    for &name in RUN_FLAGS {
        let applicable = match name {
            "events" | "findings" => variant == "econ",
            "notation" | "boundary" | "proof" | "theory" => variant == "math",
            // docs/literature-only-digest-design.md: a digest builds no hubs; zt-idea-grow does.
            "hubs" => false,
            _ => true,
        };
        options.insert(
            name.into(),
            json!(applicable && !(name == "figures" && no_figures)),
        );
    }
    if let Some(input) = input {
        for (name, value) in input
            .as_object()
            .context("run options must be a JSON object")?
        {
            if !RUN_FLAGS.contains(&name.as_str()) {
                bail!("unknown run option `{name}`; core verification layers are required");
            }
            let enabled = value
                .as_bool()
                .with_context(|| format!("run option `{name}` must be boolean"))?;
            if enabled
                && ((variant == "econ"
                    && ["notation", "boundary", "proof", "theory"].contains(&name.as_str()))
                    || (variant == "math" && ["events", "findings"].contains(&name.as_str())))
            {
                bail!("run option `{name}` is not available for {variant}");
            }
            if enabled && name == "figures" && no_figures {
                bail!("run option figures conflicts with NoFigures");
            }
            if enabled && name == "hubs" {
                bail!("run option hubs is not available: a digest builds only the Literature tree; zt-idea-grow builds the hubs and the register");
            }
            options.insert(name.clone(), json!(enabled));
        }
    }
    Ok(Value::Object(options))
}

/// The managed run's bounded settings (R273): the per-unit attempt budget and the
/// external provider run cap. They are persisted at preparation, so a resume reuses
/// the effective values instead of accepting a second, different configuration.
pub(crate) const DEFAULT_MAX_ATTEMPTS: i64 = 3;
pub(crate) const DEFAULT_DSH_RUN_CAP: i64 = 1200;

pub(crate) fn run_settings(max_attempts: Option<i64>, dsh_run_cap: Option<i64>) -> Result<Value> {
    let max_attempts = max_attempts.unwrap_or(DEFAULT_MAX_ATTEMPTS);
    if !(1..=10).contains(&max_attempts) {
        bail!("-MaxAttempts must be a whole number from 1 through 10, not {max_attempts}");
    }
    let dsh_run_cap = dsh_run_cap.unwrap_or(DEFAULT_DSH_RUN_CAP);
    if !(60..=3600).contains(&dsh_run_cap) {
        bail!("-DshRunCap must be a whole number of seconds from 60 through 3600, not {dsh_run_cap}");
    }
    Ok(json!({"max_attempts":max_attempts,"dsh_run_cap":dsh_run_cap}))
}

/// The effective settings of a prepared run; a schema-1 contract written before
/// the settings existed keeps the documented defaults instead of inventing a value.
pub(crate) fn settings_of(contract: &Value) -> Result<Value> {
    match contract.get("settings") {
        None | Some(Value::Null) => run_settings(None, None),
        Some(declared) => {
            let max_attempts = declared["max_attempts"].as_i64();
            let dsh_run_cap = declared["dsh_run_cap"].as_i64();
            if max_attempts.is_none() && dsh_run_cap.is_none() {
                bail!("the prepared run's settings are unreadable");
            }
            run_settings(max_attempts, dsh_run_cap)
        }
    }
}

/// Date extraction is not an Event-layer switch. Disabled work stays explicit.
pub(crate) fn target_of(contract: &Value) -> Result<Value> {
    target(contract)
}

fn target(contract: &Value) -> Result<Value> {
    let mut enabled = COMMON.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let mut excluded = Vec::new();
    if contract["options"]["no_figures"] == true {
        enabled.retain(|s| s != "figures");
        excluded.push("figures");
    }
    if contract["variant"] == "econ" {
        enabled.extend(["events".into(), "findings".into()]);
        excluded.extend_from_slice(MATH);
    } else {
        enabled.extend(MATH.iter().map(|s| s.to_string()));
        excluded.extend(["events", "findings"]);
    }
    let declared = contract["options"].get("run_options");
    let run = run_options(
        field(contract, "variant")?,
        contract["options"]["no_figures"] == true,
        declared,
    )?;
    for &name in RUN_FLAGS {
        if run[name] != false {
            continue;
        }
        let affected = if name == "models" {
            vec!["models", "explain"]
        } else {
            vec![name]
        };
        for layer in affected {
            enabled.retain(|l| l != layer);
            if !excluded.contains(&layer) {
                excluded.push(layer);
            }
        }
    }
    Ok(json!({"enabled":enabled,"excluded":excluded}))
}

pub(crate) struct Prepared {
    pub(crate) out: PathBuf,
    pub(crate) contract: Value,
    pub(crate) hash: String,
}

fn field<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .with_context(|| format!("incomplete prepared contract: {key}"))
}

fn check_file(record: &Value, label: &str) -> Result<()> {
    let path = Path::new(field(record, "path")?);
    let bytes = std::fs::read(path)
        .with_context(|| format!("missing {label} artifact {}", path.display()))?;
    if canon::sha256_hex(&bytes) != field(record, "sha256")? {
        bail!(
            "{label} hash mismatch at {}; use a new output directory",
            path.display()
        );
    }
    if std::fs::canonicalize(path)? != path {
        bail!("{label} path is not canonical: {}", path.display());
    }
    Ok(())
}

/// A prepared group file is exact unless the run variant declares a group extension, in
/// which case the compiler may append its own enumeration at the frozen boundary: the
/// prepared prefix still hashes exactly and the append opens with the declared marker.
/// Nothing before that boundary may change (R238).
fn check_group_file(record: &Value, label: &str, extension: Option<&str>) -> Result<()> {
    let Some(marker) = extension else {
        return check_file(record, label);
    };
    let path = Path::new(field(record, "path")?);
    let bytes = std::fs::read(path)
        .with_context(|| format!("missing {label} artifact {}", path.display()))?;
    if std::fs::canonicalize(path)? != path {
        bail!("{label} path is not canonical: {}", path.display());
    }
    let frozen = record["bytes"]
        .as_u64()
        .with_context(|| format!("incomplete {label} artifact length"))? as usize;
    if bytes.len() < frozen || canon::sha256_hex(&bytes[..frozen]) != field(record, "sha256")? {
        bail!(
            "{label} prepared text changed at {}; use a new output directory",
            path.display()
        );
    }
    if bytes.len() == frozen {
        return Ok(());
    }
    let appended =
        std::str::from_utf8(&bytes[frozen..]).context("group extension must be strict UTF-8")?;
    if !appended.trim_start().starts_with(marker) {
        bail!("{label} changed outside its declared extension marker {marker:?}");
    }
    Ok(())
}

fn json_artifact(path: &Path) -> Result<(Value, Value)> {
    let path = std::fs::canonicalize(path)?;
    let mut bytes = Vec::new();
    let mut input = std::io::Read::take(std::fs::File::open(&path)?, 16 * 1024 * 1024 + 1);
    std::io::Read::read_to_end(&mut input, &mut bytes)?;
    if bytes.len() > 16 * 1024 * 1024 {
        bail!("JSON artifact exceeds 16 MiB");
    }
    let value =
        canon::parse_json(std::str::from_utf8(&bytes).context("artifact must be strict UTF-8")?)?;
    let record = json!({"path":path.to_string_lossy(),"sha256":canon::sha256_hex(&bytes)});
    Ok((value, record))
}

fn bound_json(record: &Value, label: &str) -> Result<Value> {
    let (value, observed) = json_artifact(Path::new(field(record, "path")?))?;
    if observed["path"] != record["path"] || observed["sha256"] != record["sha256"] {
        bail!("{label} hash mismatch");
    }
    Ok(value)
}

impl Prepared {
    /// All filesystem and MCP checks finish before opening a write transaction.
    pub(crate) fn read(out: &str, zt: &mut Zt) -> Result<Self> {
        Self::read_mode(out, zt, true)
    }

    pub(crate) fn read_mode(out: &str, zt: &mut Zt, require_cards: bool) -> Result<Self> {
        let out = std::fs::canonicalize(out).context("prepared output directory does not exist")?;
        let manifest = util::read_json(out.join("manifest.json"))?;
        let contract = manifest["run_contract"].clone();
        if contract["schema_version"] != 1 || contract["contracts"]["artifact_schema"] != 1 {
            bail!(
                "incompatible workflow contract: prepared-run schema 1 is required; legacy adoption is separate"
            );
        }
        let variant = field(&contract, "variant")?;
        let base: Value = serde_json::from_str(include_str!(
            "../../../scripts/book-digest/workflow/zt-digest-base.blueprint.json"
        ))?;
        let profile: Value = serde_json::from_str(match variant {
            "econ" => {
                include_str!("../../../scripts/book-digest/workflow/zt-econ-digest.blueprint.json")
            }
            "math" => {
                include_str!("../../../scripts/book-digest/workflow/zt-math-digest.blueprint.json")
            }
            _ => bail!("incompatible workflow contract: unknown variant {variant}"),
        })?;
        if contract["contracts"]["base_version"] != base["version"]
            || contract["contracts"]["variant_version"] != profile["version"]
        {
            bail!("incompatible workflow contract: base/variant versions differ");
        }
        if contract["citation_key"] != manifest["citation_key"]
            || std::fs::canonicalize(field(&manifest, "staging")?)? != out
            || contract["source"]["file_id"] != manifest["file_id"]
            || std::fs::canonicalize(field(&manifest, "source")?)?
                != Path::new(field(&contract["source"], "path")?)
        {
            bail!("prepared identity mismatch: Citation key, output directory or source locator");
        }
        field(&contract, "citation_key")?;
        if !contract["options"].is_object() {
            bail!("incomplete prepared options");
        }
        if let Some(declared) = contract["options"].get("run_options") {
            if run_options(
                variant,
                contract["options"]["no_figures"] == true,
                Some(declared),
            )? != *declared
            {
                bail!("incomplete effective run options");
            }
        }
        check_file(&contract["source"], "source")?;
        let groups = contract["groups"]
            .as_array()
            .filter(|g| !g.is_empty())
            .context("incomplete prepared groups")?;
        let manifest_groups = manifest["groups"]
            .as_array()
            .context("incomplete group manifest")?;
        if manifest_groups.len() != groups.len()
            || canon::hash_value(&contract["groups"]) != contract["group_manifest_hash"]
        {
            bail!("group manifest hash mismatch");
        }
        for (i, (g, m)) in groups.iter().zip(manifest_groups).enumerate() {
            if g["n"].as_u64() != Some(i as u64 + 1)
                || g["anchor"] != format!("{}/{}", field(&contract, "citation_key")?, i + 1)
            {
                bail!("group manifest identity mismatch at group {}", i + 1);
            }
            for key in ["n", "id", "anchor", "chapter", "headings", "chars"] {
                if g[key] != m[key] {
                    bail!("group manifest mismatch at group {} field {key}", i + 1);
                }
            }
            if std::fs::canonicalize(field(m, "file")?)? != Path::new(field(g, "path")?) {
                bail!("group locator mismatch at group {}", i + 1);
            }
            check_group_file(
                g,
                "group",
                contract["group_extension"]["marker"].as_str(),
            )?;
            let sections = g["sections"]
                .as_array()
                .context("incomplete prepared sections")?;
            let names = m["sections"]
                .as_array()
                .context("incomplete group section list")?;
            if sections.len() != names.len() || sections.is_empty() {
                bail!("group section manifest mismatch at group {}", i + 1);
            }
            for (section, name) in sections.iter().zip(names) {
                if std::fs::canonicalize(
                    out.join("sections")
                        .join(name.as_str().context("invalid section name")?),
                )? != Path::new(field(section, "path")?)
                {
                    bail!("group section locator mismatch");
                }
                check_file(section, "section")?;
            }
        }
        for key in [
            "book",
            "index",
            "shared_prefix",
            "link_prefix",
            "figures_file",
            "events",
        ] {
            let artifact = &contract["artifacts"][key];
            let locator = if key == "events" {
                out.join("events.json")
            } else {
                PathBuf::from(field(&manifest, key)?)
            };
            if std::fs::canonicalize(locator)? != Path::new(field(artifact, "path")?) {
                bail!("{key} artifact locator mismatch");
            }
            check_file(artifact, key)?;
        }
        if let Some(setup) = contract["artifacts"].get("setup_calls") {
            if std::fs::canonicalize(out.join("setup-calls.json"))?
                != Path::new(field(setup, "path")?)
            {
                bail!("setup artifact locator mismatch");
            }
            check_file(setup, "setup")?;
        }
        // Concurrent lanes write Cards while this read runs, so a moved revision is re-read (each
        // attempt is a pure read) instead of failing the command: a refused `ledger next` ended a whole
        // layer on ebrill2001modern trial 1.
        let mut attempt = 0;
        loop {
            attempt += 1;
            let raced = |attempt: u32| {
                std::env::var("ZT_LEDGER_VIEW_RACE")
                    .ok()
                    .and_then(|n| n.parse::<u32>().ok())
                    .is_some_and(|n| attempt <= n)
            };
            let observed = zt.archive_view()?;
            if observed["archive_identity"] != contract["archive_identity"] {
                bail!("archive identity mismatch; use the original archive or a new output directory");
            }
            if !require_cards {
                let hash = canon::hash_value(&contract);
                return Ok(Self {
                    out,
                    contract,
                    hash,
                });
            }
            let mut addresses = vec![field(&contract, "citation_key")?.to_string()];
            addresses.extend(
                groups
                    .iter()
                    .map(|g| g["anchor"].as_str().unwrap().to_string()),
            );
            // T21: a prepared root or anchor that no longer sits at its frozen address is a precise
            // conflict, never a silently re-selected Card. A structural move of a published Card has
            // its own recorded mapping (`ledger relocate`); the prepared anchors themselves cannot move
            // without a new preparation.
            let cards = zt.cards(&addresses, "text").with_context(|| {
                format!(
                    "anchor_moved: the prepared Literature root or a group anchor is no longer at its frozen address ({}); a published Card move needs `zt-digest ledger relocate`, a prepared anchor move needs a new preparation",
                    addresses.join(", ")
                )
            })?;
            if cards.archive_revision != observed["archive_revision"].as_u64().unwrap() || raced(attempt) {
                if attempt < PREPARED_VIEW_READS {
                    continue;
                }
                bail!("archive changed while checking preparation; retry");
            }
            if cards.cards.iter().any(|c| c["workspace"] != "agent")
                || cards.cards[0]["kind"] != "literature"
                || cards.cards[1..].iter().any(|c| c["kind"] != "regular")
            {
                bail!("prepared Literature/anchor ownership mismatch");
            }
            if canon::sha256_hex(field(&cards.cards[0], "bibtex")?.as_bytes())
                != contract["options"]["bibtex_sha256"]
            {
                bail!("prepared Literature metadata mismatch");
            }
            let after = zt.archive_view()?;
            // Only a moving revision is read again; an archive replaced during the check refuses at once.
            if after["archive_identity"] != contract["archive_identity"] {
                bail!("archive changed while checking preparation; retry without publishing work");
            }
            if after["archive_revision"].as_u64() != Some(cards.archive_revision) {
                if attempt < PREPARED_VIEW_READS {
                    continue;
                }
                bail!("archive changed while checking preparation; retry without publishing work");
            }
            break;
        }
        let hash = canon::hash_value(&contract);
        Ok(Self {
            out,
            contract,
            hash,
        })
    }
}

/// How often the prepared-run check reads its archive view before a moving revision fails it.
const PREPARED_VIEW_READS: u32 = 5;

pub(crate) fn connect(path: &Path, writable: bool) -> Result<Connection> {
    let flags = if writable {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    };
    let conn = Connection::open_with_flags(path, flags)
        .with_context(|| format!("open ledger {}", path.display()))?;
    conn.busy_timeout(Duration::from_secs(10))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    if writable {
        conn.pragma_update(None, "journal_mode", "WAL")?;
    }
    Ok(conn)
}

pub(crate) fn meta(conn: &Connection, key: &str) -> Result<Value> {
    let value: String = conn
        .query_row("SELECT value FROM meta WHERE key=?1", [key], |r| r.get(0))
        .with_context(|| format!("ledger metadata missing: {key}"))?;
    Ok(serde_json::from_str(&value)?)
}

fn put_meta(tx: &Transaction, key: &str, value: &Value) -> Result<()> {
    tx.execute(
        "INSERT INTO meta(key,value) VALUES(?1,?2)",
        params![key, canon::canonical_json(value)],
    )?;
    Ok(())
}

/// Sealing is monotonic. A layer's producer enumeration is complete once the
/// publication revision that enumerates it is registered, so the revision can
/// leave `registered` as soon as its producers settle (US26).
fn seal(tx: &Transaction, layer: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=?2",
        params![
            format!("barrier/{layer}"),
            canon::canonical_json(&json!({"sealed":true}))
        ],
    )?;
    Ok(())
}

fn verify_schema(conn: &Connection) -> Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version != 1 || meta(conn, "schema_version")? != 1 {
        bail!("incompatible ledger schema");
    }
    let journal: String = conn.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
    if journal != "wal" {
        bail!("ledger journal mode mismatch: expected wal, observed {journal}");
    }
    let mut tables=conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;
    let tables = tables
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if tables != ["deps", "events", "meta", "outputs", "units"] {
        bail!("ledger table mismatch: {tables:?}");
    }
    for (table, columns) in [
        ("meta", "key,value"),
        (
            "units",
            "id,layer,logical_key,revision,state,attempts,owner,claim_token,lease_until,retry_after,fault_class,fault_json,inputs_json,input_hash,progress_json,result_json,created_by,created_at,completed_at",
        ),
        ("deps", "unit_id,needs_id,condition"),
        (
            "outputs",
            "unit_id,output_key,kind,locator,fingerprint,data_json",
        ),
        (
            "events",
            "seq,unit_id,invocation,utc,owner,claim_token,event,facts_json",
        ),
    ] {
        conn.prepare(&format!("SELECT {columns} FROM {table} LIMIT 0"))
            .with_context(|| format!("ledger table mismatch: {table} columns"))?;
    }
    let mut indexes = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='index' AND sql IS NOT NULL ORDER BY name",
    )?;
    let indexes = indexes
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if indexes != ["deps_needs", "units_ready"] {
        bail!("ledger index mismatch: {indexes:?}");
    }
    for (name, expected) in [
        ("deps_needs", &["needs_id"][..]),
        ("units_ready", &["layer", "state", "retry_after"][..]),
    ] {
        let mut columns = conn.prepare(&format!("PRAGMA index_info({name})"))?;
        let columns = columns
            .query_map([], |r| r.get::<_, String>(2))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if columns != expected {
            bail!("ledger index mismatch: {name} has {columns:?}");
        }
    }
    let compact = |sql: &str| {
        sql.chars()
            .filter(|c| !c.is_ascii_whitespace())
            .map(|c| c.to_ascii_lowercase())
            .collect::<String>()
            .trim_end_matches(';')
            .to_string()
    };
    for expected in SCHEMA
        .lines()
        .filter(|line| line.starts_with("CREATE TRIGGER "))
    {
        let name = expected.split_whitespace().nth(2).unwrap();
        let actual: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='trigger' AND name=?1",
                [name],
                |r| r.get(0),
            )
            .optional()?;
        if actual.as_deref().map(compact) != Some(compact(expected)) {
            bail!("ledger event protection mismatch: {name}");
        }
    }
    for (table, columns) in [
        ("units", &["created_by"][..]),
        ("deps", &["unit_id", "needs_id"][..]),
        ("outputs", &["unit_id"][..]),
        ("events", &["unit_id"][..]),
    ] {
        let mut keys = conn.prepare(&format!("PRAGMA foreign_key_list({table})"))?;
        let mut keys = keys
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        keys.sort();
        let mut expected = columns
            .iter()
            .map(|c| ("units".to_string(), c.to_string(), "id".to_string()))
            .collect::<Vec<_>>();
        expected.sort();
        if keys != expected {
            bail!("ledger foreign key mismatch: {table}");
        }
    }
    let violation: Option<(String, Option<i64>, String)> = conn
        .query_row("PRAGMA foreign_key_check", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()?;
    if let Some(violation) = violation {
        bail!("ledger foreign key violation: {violation:?}");
    }
    Ok(())
}

fn verify_binding(conn: &Connection, p: &Prepared) -> Result<()> {
    verify_schema(conn)?;
    if meta(conn, "preparation_hash")? != p.hash || meta(conn, "preparation")? != p.contract {
        bail!("immutable preparation mismatch; use a new output directory");
    }
    if meta(conn, "target")? != target(&p.contract)? {
        bail!("ledger declared target mismatch");
    }
    Ok(())
}

fn register(
    tx: &Transaction,
    layer: &str,
    key: &str,
    revision: i64,
    inputs: &Value,
    creator: Option<&str>,
    done: bool,
) -> Result<String> {
    let id = format!("{layer}/{key}@{revision}");
    let now = canon::utc_now_o();
    tx.execute("INSERT INTO units(id,layer,logical_key,revision,state,inputs_json,input_hash,created_by,created_at,completed_at,result_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![id,layer,key,revision,if done{"done"}else{"registered"},canon::canonical_json(inputs),canon::hash_value(inputs),creator,now,if done{Some(&now)}else{None},if done{Some("{\"disposition\":\"verified\"}")}else{None}])?;
    Ok(id)
}

fn dependency(tx: &Transaction, id: &str, needs: &str, condition: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO deps(unit_id,needs_id,condition) VALUES(?1,?2,?3)",
        params![id, needs, condition],
    )?;
    Ok(())
}

/// A producer's verified outputs as one order-independent contribution fingerprint.
/// A producer without registered outputs has no verified contribution yet.
fn verified_outputs(conn: &Connection, unit: &str) -> Result<String> {
    let mut statement = conn
        .prepare("SELECT output_key,fingerprint FROM outputs WHERE unit_id=?1 ORDER BY output_key")?;
    let rows = statement
        .query_map([unit], |r| {
            Ok(json!([r.get::<_, String>(0)?, r.get::<_, String>(1)?]))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(canon::hash_value(&Value::Array(rows)))
}

/// The trees a completed producer actually touched, from its retained result.
fn recorded_trees(result: Option<&str>) -> Result<Vec<String>> {
    let Some(result) = result else {
        return Ok(Vec::new());
    };
    let result: Value = serde_json::from_str(result).context("invalid producer result")?;
    Ok(result["trees"]
        .as_array()
        .map(|trees| {
            trees
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default())
}

/// A unit's retained fault, exactly as recorded.
fn parsed_fault(fault: Option<&str>) -> Result<Value> {
    match fault {
        Some(fault) => serde_json::from_str(fault).context("invalid unit fault"),
        None => Ok(Value::Null),
    }
}

/// The disposition code a publication manifest records. Scheduling states
/// (`registered`, `ready`, `claimed`, `retry`) are one `pending` disposition:
/// only real status changes alter a publication manifest, and a status change
/// always does (R159).
fn disposition_fault(fault: Option<&str>) -> Result<Value> {
    let fault = parsed_fault(fault)?;
    Ok(if fault.is_null() {
        Value::Null
    } else {
        json!({"code":fault["code"]})
    })
}

/// The disposition vocabulary of one unresolved producer.
fn disposition_of(state: &str) -> &'static str {
    match state {
        "failed" | "blocked" => "unresolved",
        _ => "pending",
    }
}

/// Whether one unit is a superseded publication revision: terminal history that
/// a later revision replaced, never a failure and never resumable work.
fn is_superseded(unit: &Value) -> bool {
    unit["fault"]["code"] == "superseded"
}

/// The sorted input manifest of one aggregate publication revision: contributing
/// units with their verified output fingerprints, the disposition of every
/// relevant unresolved unit, and the publication contract version (R159).
fn publication_manifest(conn: &Connection, layer: &str, key: &str, scope: &Value) -> Result<Value> {
    let sources = publication_sources(layer)
        .with_context(|| format!("`{layer}` is not an aggregate publication layer"))?;
    let mut contributions = Vec::new();
    let mut dispositions = Vec::new();
    for source in sources {
        let mut statement =
            conn.prepare("SELECT id,state,fault_json,result_json FROM units WHERE layer=?1 ORDER BY id")?;
        let rows = statement
            .query_map([source], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, state, fault, result) in rows {
            // A hub refresh is coalesced by touched tree. A producer that has not
            // run yet has no recorded tree, so it stays a required disposition.
            if layer == "hubs"
                && state == "done"
                && !recorded_trees(result.as_deref())?.iter().any(|t| t == key)
            {
                continue;
            }
            if state == "done" {
                contributions.push(json!({"unit":id,"state":state,"fingerprint":verified_outputs(conn,&id)?}));
            } else {
                dispositions.push(json!({"unit":id,"disposition":disposition_of(&state),"fault":disposition_fault(fault.as_deref())?}));
            }
        }
    }
    // A declared prerequisite publication is one more input: its checked revision is
    // either a contribution or an unresolved disposition of this revision (R241).
    if let Some(previous) = publication_prerequisite(conn, layer)? {
        let row: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT state,fault_json FROM units WHERE id=?1",
                [&previous],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (state, fault) = row.with_context(|| {
            format!("the prerequisite publication `{previous}` of `{layer}` is not registered")
        })?;
        if state == "done" {
            contributions.push(json!({"unit":previous,"state":state,"fingerprint":verified_outputs(conn,&previous)?}));
        } else {
            dispositions.push(json!({"unit":previous,"disposition":disposition_of(&state),"fault":disposition_fault(fault.as_deref())?}));
        }
    }
    Ok(json!({
        "schema_version":1,
        "publication_contract":PUBLICATION_CONTRACT,
        "layer":layer,
        "logical_key":key,
        "preparation_hash":meta(conn,"preparation_hash")?,
        "scope":scope,
        "contributions":contributions,
        "dispositions":dispositions
    }))
}

/// Every older revision of one logical key that is no longer the current
/// publication: it is terminal and can never be claimed as current work.
fn superseded_units(conn: &Connection, layer: &str, key: &str, before: i64) -> Result<Vec<String>> {
    let mut statement = conn.prepare(
        "SELECT id,state,fault_json FROM units WHERE layer=?1 AND logical_key=?2 AND revision<?3 ORDER BY revision",
    )?;
    let rows = statement
        .query_map(params![layer, key, before], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut superseded = Vec::new();
    for (id, state, fault) in rows {
        if state != "done" && disposition_fault(fault.as_deref())?["code"] == "superseded" {
            superseded.push(id);
        }
    }
    Ok(superseded)
}

/// Register one derived aggregate revision and its manifest dependencies.
fn register_publication_unit(
    tx: &Transaction,
    layer: &str,
    key: &str,
    revision: i64,
    manifest: &Value,
    creator: Option<&str>,
) -> Result<String> {
    let id = register(tx, layer, key, revision, manifest, creator, false)?;
    for contribution in manifest["contributions"].as_array().unwrap() {
        dependency(tx, &id, field(contribution, "unit")?, "success")?;
    }
    for disposition in manifest["dispositions"].as_array().unwrap() {
        dependency(tx, &id, field(disposition, "unit")?, "settled")?;
    }
    Ok(id)
}

/// An aggregate revision stays bound to its preparation, layer and logical key;
/// only the immutable revision suffix and its manifest may differ.
fn verify_publication_input(id: &str, layer: &str, key: &str, hash: &str, inputs: &Value) -> Result<()> {
    if inputs["schema_version"] != 1
        || inputs["publication_contract"] != PUBLICATION_CONTRACT
        || inputs["layer"] != layer
        || inputs["logical_key"] != key
        || inputs["preparation_hash"] != hash
        || !inputs["contributions"].is_array()
        || !inputs["dispositions"].is_array()
    {
        bail!("preparation registration: changed publication binding for {id}");
    }
    Ok(())
}

/// One independent note suggestion: its own readable key, the Card it names and
/// the authored Card state the suggestion was made from (R224).
fn note_suggestions(zt: &mut Zt, path: &str) -> Result<Vec<Value>> {
    let raw = util::read_json(path).context("invalid note suggestions")?;
    let items = raw
        .as_array()
        .context("note suggestions must be a JSON array")?;
    if items.is_empty() || items.len() > 512 {
        bail!("note suggestions must name between 1 and 512 suggestions");
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut suggestions = Vec::new();
    for item in items {
        let id = field(item, "id")?;
        if id.is_empty()
            || id.len() > 64
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            bail!("note suggestion id `{id}` must be a readable key of at most 64 bytes");
        }
        if !seen.insert(id.to_string()) {
            bail!("note suggestion `{id}` is declared twice");
        }
        let note = field(item, "note")?;
        if note.trim().is_empty() {
            bail!("note suggestion `{id}` must carry its suggestion");
        }
        let target = note
            .split("[[")
            .nth(1)
            .and_then(|rest| rest.split("]]").next())
            .map(|link| link.trim().to_string())
            .filter(|target| !target.is_empty())
            .with_context(|| format!("note suggestion `{id}` must name the Link it proposes"))?;
        suggestions.push(json!({"id":id,"card":field(item,"card")?,"note":note,"target":target}));
    }
    // One bounded read binds every suggestion to the Card state it was made from.
    let addresses = suggestions
        .iter()
        .map(|s| field(s, "card").map(str::to_string))
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
    let cards = zt.cards(
        &addresses.iter().cloned().collect::<Vec<_>>(),
        "text",
    )?;
    for suggestion in &mut suggestions {
        let at = field(suggestion, "card")?;
        let card = cards
            .cards
            .iter()
            .find(|card| card["address"] == at)
            .with_context(|| {
                format!(
                    "note suggestion `{}` names a missing Card `{at}`",
                    field(suggestion, "id").unwrap_or_default()
                )
            })?;
        suggestion["fingerprint"] = json!(crate::calls::authored_fingerprint(card));
    }
    suggestions.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    Ok(suggestions)
}

/// Register one unit per independent note suggestion. The same suggestion key with
/// the same content reuses its unit; rebinding a key to different content is refused.
fn register_notes(p: &Prepared, zt: &mut Zt, path: &str) -> Result<Value> {
    let suggestions = note_suggestions(zt, path)?;
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let mut units = Vec::new();
    let mut reused = true;
    for suggestion in &suggestions {
        let id = field(suggestion, "id")?;
        let unit = format!("notes/{id}@1");
        let hash = canon::hash_value(suggestion);
        let existing: Option<String> = tx
            .query_row(
                "SELECT input_hash FROM units WHERE id=?1",
                [&unit],
                |r| r.get(0),
            )
            .optional()?;
        match existing {
            Some(stored) => {
                if stored != hash {
                    bail!("a registered suggestion key cannot be rebound to different content: {unit}");
                }
            }
            None => {
                reused = false;
                register(&tx, "notes", id, 1, suggestion, None, false)?;
                dependency(&tx, &unit, "prepare/book@1", "success")?;
                event(
                    &tx,
                    &unit,
                    None,
                    None,
                    "registered",
                    &json!({"card":suggestion["card"],"input_hash":hash}),
                )?;
            }
        }
        units.push(json!({
            "unit":unit,
            "card":suggestion["card"],
            "note":suggestion["note"],
            "target":suggestion["target"],
            "fingerprint":suggestion["fingerprint"],
            "input_hash":hash,
            "state":"registered"
        }));
    }
    let run = meta(&tx, "run_id")?;
    tx.commit()?;
    Ok(json!({
        "schema_version":1,
        "run_id":run,
        "reused":reused,
        "suggestions":suggestions.len(),
        "units":units
    }))
}

/// The mathematics variant's per-unit obligations (R237). The compiler's own boundary
/// and proof plans enumerate the mathematical units, and each planned unit becomes one
/// unit of the same ledger: no second ledger, scheduler or compiler framework. A unit
/// binds the exact plan entry it was enumerated from, so the same plan reuses its units
/// and rebinding one key to different work is refused (R167).
fn math_register(p: &Prepared, _zt: &mut Zt) -> Result<Value> {
    if p.contract["variant"] != "math" {
        bail!("ledger math requires a prepared mathematics run");
    }
    let declared = target(&p.contract)?;
    let enabled = |name: &str| {
        declared["enabled"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == name)
    };
    let notation = enabled("notation");
    let mut plans = Vec::new();
    for layer in MATH_OBLIGATIONS {
        if !enabled(layer) {
            continue;
        }
        let path = p.out.join("math").join(layer).join("plan.json");
        if !path.exists() {
            continue;
        }
        plans.push((*layer, math_plan(layer, &util::read_json(&path)?)?));
    }
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let mut units = Vec::new();
    let mut reused = true;
    for (layer, plan) in &plans {
        for entry in plan.as_array().context("invalid math plan")? {
            let key = field(entry, "unit")?;
            let inputs = json!({"schema_version":1,"layer":layer,"key":key,"plan":entry});
            let unit = format!("{layer}/{key}@1");
            let hash = canon::hash_value(&inputs);
            let existing: Option<String> = tx
                .query_row("SELECT input_hash FROM units WHERE id=?1", [&unit], |r| {
                    r.get(0)
                })
                .optional()?;
            match existing {
                Some(stored) => {
                    if stored != hash {
                        bail!("a registered mathematical unit cannot be rebound to different work: {unit}");
                    }
                }
                None => {
                    reused = false;
                    register(&tx, layer, key, 1, &inputs, None, false)?;
                    dependency(&tx, &unit, "math-skeleton/book@1", "success")?;
                    if notation {
                        // Notation must be applied and available before the proof and
                        // model work that requires it (R238).
                        dependency(&tx, &unit, "notation/book@1", "success")?;
                    }
                    event(
                        &tx,
                        &unit,
                        None,
                        None,
                        "registered",
                        &json!({"layer":layer,"plan":entry}),
                    )?;
                }
            }
            units.push(json!({"unit":unit,"layer":layer,"key":key,"input_hash":hash}));
        }
    }
    let run = meta(&tx, "run_id")?;
    tx.commit()?;
    Ok(json!({
        "schema_version":1,
        "run_id":run,
        "reused":reused,
        "layers":plans.iter().map(|(layer,_)| *layer).collect::<Vec<_>>(),
        "units":units
    }))
}

/// One retained compiler plan: every entry names one mathematical unit and the work
/// that unit owes. An entry that does not is refused before any unit is registered.
fn math_plan(layer: &str, plan: &Value) -> Result<Value> {
    if plan["schema_version"] != 1 {
        bail!("invalid {layer} plan: schema 1 is required");
    }
    let entries = plan["units"].as_array().context("invalid math plan")?;
    let mut seen = std::collections::BTreeSet::new();
    for entry in entries {
        let key = field(entry, "unit")?;
        if key.trim().is_empty()
            || key.len() > 200
            || key.contains(['\n', '\r'])
            || !seen.insert(key.to_string())
        {
            bail!("`{key}` is not one readable mathematical unit of the {layer} plan");
        }
        if field(entry, "id").is_err() || entry["group"].as_i64().is_none() {
            bail!("the {layer} plan entry for `{key}` names no indexed statement");
        }
        match layer {
            "boundary" => {
                let need = entry["need"].as_array().context("invalid math plan")?;
                if need.is_empty() || need.iter().any(|n| n.as_str().is_none_or(str::is_empty)) {
                    bail!("the boundary plan entry for `{key}` names no boundary work");
                }
            }
            "proof" => {
                let steps = entry["steps"].as_array().context("invalid math plan")?;
                if steps.is_empty()
                    || steps.iter().any(|step| {
                        field(step, "address").is_err() || field(step, "facet").is_err()
                    })
                {
                    bail!("the proof plan entry for `{key}` names no proof step");
                }
            }
            _ => {}
        }
    }
    Ok(Value::Array(entries.clone()))
}

/// The mathematics variant's book-level layers whose completion has its own checked
/// contract: the prepared enumeration, the prompt files and the applied notation table.
/// Everything after them is the per-unit work of `MATH_OBLIGATIONS` (R238).
fn math_book_layer(unit: &str) -> Option<&'static str> {
    Some(match unit {
        "math-index/book@1" => "math-index",
        "math-docs/book@1" => "math-docs",
        "math-skeleton/book@1" => "math-skeleton",
        "notation/book@1" => "notation",
        "math-check/book@1" => "math-check",
        _ => return None,
    })
}

/// One verified mathematics layer completion: its result file is published, its checked
/// outputs are recorded, the claim is rechecked and the unit completes in one
/// transaction. A repeated completion rereads the retained result instead of trusting a
/// second file (R160).
fn math_record(
    p: &Prepared,
    unit: &str,
    token: i64,
    claimed: &Claim,
    done: bool,
    mut result: Value,
    outputs: &[(String, String, String, String, Value)],
    file: &str,
) -> Result<Value> {
    if done {
        let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
        let stored: String =
            conn.query_row("SELECT result_json FROM units WHERE id=?1", [unit], |r| {
                r.get(0)
            })?;
        let mut stored: Value = serde_json::from_str(&stored)?;
        check_file(&stored["result"], "mathematics completion")?;
        stored["reused"] = json!(true);
        return Ok(stored);
    }
    let record = publish(&p.out.join(file), &serde_json::to_string_pretty(&result)?)?;
    result["result"] = record.clone();
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, unit, token)?;
    output(
        &tx,
        unit,
        "result",
        "file",
        field(&record, "path")?,
        field(&record, "sha256")?,
        &record,
    )?;
    for (key, kind, locator, fingerprint, data) in outputs {
        output(&tx, unit, key, kind, locator, fingerprint, data)?;
    }
    tx.execute("UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",params![unit,canon::canonical_json(&result),canon::utc_now_o()])?;
    event(
        &tx,
        unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"layer":result["layer"],"disposition":result["disposition"],"checks":result["checks"]}),
    )?;
    tx.commit()?;
    Ok(result)
}

/// The prepared run's own manifest, re-read where a verifier needs the frozen paths.
fn prepared_manifest(p: &Prepared) -> Result<Value> {
    util::read_json(p.out.join("manifest.json"))
}

/// `math-index/book@1`: every numbered statement of every prepared section file is
/// indexed, and the retained index is exactly what those prepared bytes produce. A
/// missing or changed section is a fault, never an empty index.
fn math_index_finish(p: &Prepared, unit: &str, token: i64, path: &Path) -> Result<Value> {
    let _guard = unit_guard(p, unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, unit, token)?;
    let manifest = prepared_manifest(p)?;
    let expected = p.out.join("math").join("index.json");
    let path = std::fs::canonicalize(path).context("math index evidence does not exist")?;
    if path != std::fs::canonicalize(&expected)? {
        bail!("`{unit}` completes at the prepared index it enumerated: {}", expected.display());
    }
    let retained = util::read_json(&path).context("invalid retained math index")?;
    let (units, others) = crate::math::scan_index(&p.out.to_string_lossy(), &manifest)?;
    if retained["units"] != json!(units) || retained["others"] != json!(others) {
        bail!("the retained math index does not match the prepared sections it must account for");
    }
    let sections = manifest["groups"]
        .as_array()
        .context("incomplete prepared groups")?
        .iter()
        .map(|g| {
            g["sections"]
                .as_array()
                .context("incomplete prepared sections")
                .map(|s| s.len())
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .sum::<usize>();
    let mut forms = std::collections::BTreeMap::<String, u64>::new();
    for unit in &units {
        *forms.entry(field(unit, "form")?.to_string()).or_default() += 1;
    }
    let result = json!({
        "schema_version":1,
        "kind":"math_index_completion",
        "unit":unit,
        "token":token,
        "layer":"math-index",
        "state":"done",
        "reused":false,
        "input_hash":claimed.input_hash,
        "disposition":"indexed",
        "units":units.len(),
        "others":others.len(),
        "sections":sections,
        "forms":forms,
        "checks":{"prepared_sections":sections,"indexed_statements":units.len()}
    });
    math_record(p, unit, token, &claimed, done, result, &[], "math/completion-index.json")
}

/// `math-docs/book@1`: each math prompt file is the frozen shared instruction file plus
/// its math supplement, with this run's own paths filled in (R238).
fn math_docs_finish(p: &Prepared, unit: &str, token: i64, path: &Path) -> Result<Value> {
    let _guard = unit_guard(p, unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, unit, token)?;
    let manifest = prepared_manifest(p)?;
    let expected = p.out.join("math").join("docs.json");
    let path = std::fs::canonicalize(path).context("math docs evidence does not exist")?;
    if path != std::fs::canonicalize(&expected)? {
        bail!("`{unit}` completes at the prepared docs report: {}", expected.display());
    }
    let report = util::read_json(&path).context("invalid math docs report")?;
    // The matcher uses the staging spelling the preparation froze, which is what the
    // compiler filled into the prompt files.
    let staging = field(&manifest, "staging")?.to_string();
    let wanted = crate::math::docs_files(
        field(&p.contract, "citation_key")?,
        manifest["book_title"].as_str().unwrap_or_default(),
        &staging,
        &manifest,
    )?;
    let files = report["files"]
        .as_array()
        .context("the math docs report names no prompt files")?;
    if report["ok"] != true || files.len() != wanted.len() {
        bail!("the math docs report must name all {} math prompt files", wanted.len());
    }
    let mut bound = Vec::new();
    for ((target, text), named) in wanted.iter().zip(files) {
        let named = std::fs::canonicalize(named.as_str().context("invalid prompt file path")?)
            .with_context(|| format!("missing math prompt file for {target}"))?;
        if named != std::fs::canonicalize(target)? {
            bail!("math prompt file mismatch at {}", named.display());
        }
        let actual = std::fs::read(&named)?;
        if actual != text.as_bytes() {
            bail!(
                "the math prompt file {} is not the shared instruction file with its math supplement",
                named.display()
            );
        }
        bound.push(bind_file(&named.to_string_lossy(), "math prompt")?);
    }
    let result = json!({
        "schema_version":1,
        "kind":"math_docs_completion",
        "unit":unit,
        "token":token,
        "layer":"math-docs",
        "state":"done",
        "reused":false,
        "input_hash":claimed.input_hash,
        "disposition":"docs",
        "files":files.len(),
        "checks":{"prompt_files":files.len()}
    });
    let outputs = bound
        .iter()
        .enumerate()
        .map(|(i, record)| {
            (
                format!("file/{i}"),
                "file".to_string(),
                field(record, "path").unwrap_or("").to_string(),
                field(record, "sha256").unwrap_or("").to_string(),
                record.clone(),
            )
        })
        .collect::<Vec<_>>();
    math_record(p, unit, token, &claimed, done, result, &outputs, "math/completion-docs.json")
}

/// `math-skeleton/book@1`: every indexed statement is accounted for exactly once, each
/// addressed unit and Theory Card really exists in the agent Workspace, and each prepared
/// group file carries its own unit list. Allocation is not a built unit: a `待写`
/// statement stays open and this layer never reports it built (R238).
fn math_skeleton_finish(p: &Prepared, unit: &str, token: i64, zt: &mut Zt, path: &Path) -> Result<Value> {
    let _guard = unit_guard(p, unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, unit, token)?;
    let manifest = prepared_manifest(p)?;
    let relayed = p.out.join("math").join("units.json");
    let path = std::fs::canonicalize(path).context("math skeleton evidence does not exist")?;
    if path != std::fs::canonicalize(&relayed)? {
        bail!("`{unit}` completes at the skeleton unit list: {}", relayed.display());
    }
    let retained = util::read_json(&path).context("invalid retained unit list")?;
    let (index_units, index_others) = crate::math::scan_index(&p.out.to_string_lossy(), &manifest)?;
    let units = retained["units"].as_array().context("invalid unit list")?;
    if units.len() != index_units.len() {
        bail!(
            "the skeleton accounts for {} of the {} indexed statements",
            units.len(),
            index_units.len()
        );
    }
    let mut addresses = Vec::new();
    let mut group_units = std::collections::BTreeMap::<u64, Vec<Value>>::new();
    for (allocated, indexed) in units.iter().zip(&index_units) {
        if field(allocated, "id")? != field(indexed, "id")? || allocated["form"] != indexed["form"] {
            bail!("the skeleton's units are not the indexed statements in book order");
        }
        if allocated["address"].is_null() {
            continue;
        }
        let at = field(allocated, "address")?.to_string();
        let group = allocated["group"]
            .as_u64()
            .context("a skeleton unit names no group")?;
        group_units.entry(group).or_default().push(allocated.clone());
        addresses.push(at);
    }
    let theories = retained["theories"].as_array().context("invalid Theory list")?;
    let mut chapters = std::collections::BTreeSet::new();
    for theory in theories {
        if theory["address"].is_null() {
            continue;
        }
        let chapter = field(theory, "chapter")?;
        if !chapters.insert(chapter.to_string())
            || theory["units"].as_array().is_none_or(|u| u.is_empty())
        {
            bail!("a Theory Card must cover one chapter that holds units");
        }
        addresses.push(field(theory, "address")?.to_string());
    }
    // The archive decides existence: an allocation that names no real Card is a gap.
    let cards = zt.cards(&addresses, "text")?;
    if cards.cards.len() != addresses.len() {
        bail!("the skeleton names {} Cards the agent Workspace does not hold", addresses.len() - cards.cards.len());
    }
    let tree = field(&p.contract, "citation_key")?;
    if cards
        .cards
        .iter()
        .any(|c| c["workspace"] != "agent" || c["tree"] != tree)
    {
        bail!("every allocated mathematical unit belongs to this book's agent Literature tree");
    }
    // The declared group extension: the prepared text plus this group's own unit list.
    let marker = p.contract["group_extension"]["marker"]
        .as_str()
        .context("the mathematics preparation declares no group extension")?;
    let mut appended = Vec::new();
    for group in manifest["groups"].as_array().context("incomplete prepared groups")? {
        let n = group["n"].as_u64().context("invalid prepared group number")?;
        let file = field(group, "file")?;
        let text = util::read_text(file)?;
        let allocated = group_units.get(&n).map_or(0, Vec::len);
        let others = index_others
            .iter()
            .filter(|o| o["group"].as_u64() == Some(n))
            .count();
        let block = match text.find(marker) {
            Some(at) if allocated + others == 0 => {
                bail!("group {n} carries a mathematical unit list for work it does not have")
            }
            Some(at) => &text[at..],
            None if allocated + others > 0 => {
                bail!("group {n} carries no mathematical unit list")
            }
            None => "",
        };
        for unit in group_units.get(&n).into_iter().flatten() {
            if !block.contains(&format!("[[{}]]", field(unit, "address")?)) {
                bail!(
                    "group {n} does not list its allocated unit {}",
                    field(unit, "id")?
                );
            }
        }
        for other in index_others.iter().filter(|o| o["group"].as_u64() == Some(n)) {
            if !block.contains(field(other, "id")?) {
                bail!("group {n} does not list its {} {}", field(other, "kind")?, field(other, "id")?);
            }
        }
        appended.push(json!({"group":n,"units":allocated,"others":others}));
    }
    let result = json!({
        "schema_version":1,
        "kind":"math_skeleton_completion",
        "unit":unit,
        "token":token,
        "layer":"math-skeleton",
        "state":"done",
        "reused":false,
        "input_hash":claimed.input_hash,
        "disposition":"allocated",
        "indexed":index_units.len(),
        "allocated":units.iter().filter(|u| !u["address"].is_null()).count(),
        "gaps":units.iter().filter(|u| u["address"].is_null()).count(),
        "theories":theories.iter().filter(|t| !t["address"].is_null()).count(),
        "groups":appended,
        "checks":{"indexed_statements":index_units.len(),"card_addresses":addresses.len()}
    });
    let outputs = cards
        .cards
        .iter()
        .map(|card| {
            (
                format!("card/{}", field(card, "address").unwrap_or_default()),
                "card".to_string(),
                field(card, "address").unwrap_or_default().to_string(),
                crate::calls::authored_fingerprint(card),
                json!({"purpose":"mathematical_unit"}),
            )
        })
        .collect::<Vec<_>>();
    math_record(p, unit, token, &claimed, done, result, &outputs, "math/completion-skeleton.json")
}

/// `notation/book@1`: the notation table is applied and available. The retained answer
/// re-derives the exact Card this run wrote; a dependent proof or model pass may rely on
/// it from here (R238).
fn notation_finish(p: &Prepared, unit: &str, token: i64, zt: &mut Zt, path: &Path) -> Result<Value> {
    let _guard = unit_guard(p, unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, unit, token)?;
    let expected = p.out.join("math").join("notation").join("applied.json");
    let path = std::fs::canonicalize(path).context("notation evidence does not exist")?;
    if path != std::fs::canonicalize(&expected)? {
        bail!("`{unit}` completes at the applied notation record: {}", expected.display());
    }
    let applied = util::read_json(&path).context("invalid applied notation record")?;
    if applied["schema_version"] != 1 || applied["kind"] != "notation_apply" {
        bail!("invalid applied notation record");
    }
    let answer_path = p.out.join("math").join("notation").join("out").join("notation.json");
    let answer = if answer_path.exists() {
        util::read_json(&answer_path)?
    } else {
        Value::Null
    };
    let units = {
        let file = p.out.join("math").join("units.json");
        if file.exists() {
            util::read_json(&file)?
        } else {
            Value::Null
        }
    };
    let max_symbols = applied["max_symbols"]
        .as_i64()
        .context("the applied notation record names no symbol bound")?;
    let (symbols, title, body) = crate::math::notation_table(
        &answer,
        &units,
        field(&p.contract, "citation_key")?,
        prepared_manifest(p)?["book_title"].as_str().unwrap_or_default(),
        max_symbols,
    );
    if applied["symbols"].as_u64() != Some(symbols as u64)
        || field(&applied, "body_sha256")? != canon::sha256_hex(body.as_bytes())
        || field(&applied, "title")? != title
    {
        bail!("the applied notation record does not match the answer it claims to have applied");
    }
    let disposition = if symbols == 0 { "no_symbols" } else { "applied" };
    if applied["disposition"] != disposition {
        bail!("the applied notation record states the wrong disposition");
    }
    let mut outputs = Vec::new();
    let mut card_address = Value::Null;
    if symbols == 0 {
        if !applied["card"].is_null() {
            bail!("an empty notation table writes no Card");
        }
    } else {
        let at = field(&applied, "card")?.to_string();
        let cards = zt.cards(&[at.clone()], "text")?;
        let card = cards
            .cards
            .first()
            .with_context(|| format!("the notation table Card `{at}` is missing"))?;
        if card["workspace"] != "agent"
            || card["tree"] != field(&p.contract, "citation_key")?
            || card["title"] != json!(title)
            || card["body"] != json!(body)
        {
            bail!("the notation table Card `{at}` is not the applied table of this run");
        }
        card_address = json!(at);
        outputs.push((
            format!("card/{at}"),
            "card".to_string(),
            at,
            crate::calls::authored_fingerprint(card),
            json!({"purpose":"notation_table","symbols":symbols}),
        ));
    }
    let result = json!({
        "schema_version":1,
        "kind":"notation_completion",
        "unit":unit,
        "token":token,
        "layer":"notation",
        "state":"done",
        "reused":false,
        "input_hash":claimed.input_hash,
        "disposition":disposition,
        "symbols":symbols,
        "available":symbols > 0,
        "card":card_address,
        "checks":{"symbols":symbols,"applied":applied["applied"]}
    });
    math_record(p, unit, token, &claimed, done, result, &outputs, "math/completion-notation.json")
}

/// One mathematical unit's boundary work (R239). Every candidate the plan enumerated
/// keeps its own disposition; a supplied candidate carries its independent review; a
/// rejected one is gone through the archive's confirmed deletion and names what was
/// removed. A batch count is never the record, and no candidate may disappear silently.
fn boundary_finish(p: &Prepared, unit: &str, token: i64, zt: &mut Zt, path: &Path) -> Result<Value> {
    let registered = unit_inputs(p, unit)?;
    let plan = registered["plan"].clone();
    let at = field(&plan, "unit")?.to_string();
    let _guard = unit_guard(p, unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, unit, token)?;
    let expected = p.out.join("math").join("boundary").join("settled.json");
    let path = std::fs::canonicalize(path).context("boundary settle evidence does not exist")?;
    if path != std::fs::canonicalize(&expected)? {
        bail!("`{unit}` completes at the settled boundary record: {}", expected.display());
    }
    let settled = util::read_json(&path).context("invalid settled boundary record")?;
    if settled["schema_version"] != 1 || settled["kind"] != "boundary_settle" {
        bail!("invalid settled boundary record");
    }
    let mine = settled["candidates"]
        .as_array()
        .context("the settled boundary record enumerates no candidates")?
        .iter()
        .filter(|c| c["unit"] == json!(at))
        .cloned()
        .collect::<Vec<_>>();
    if mine.is_empty() {
        bail!("`{unit}` planned boundary work and the settle record holds no candidate for it");
    }
    let reviews = settled["reviews"]
        .as_array()
        .context("the settled boundary record holds no reviews")?;
    let deletions = settled["deleted"]
        .as_array()
        .context("the settled boundary record holds no deletions")?;
    // A confirmed deletion renumbers the later siblings. The settle keeps the archive's
    // own mapping, and a recorded candidate resolves through it to the Card that holds
    // it now, in the order the deletions ran (R239).
    let renames = settled["renames"].as_array().cloned().unwrap_or_default();
    let resolve = |address: &str| -> String {
        let mut current = address.to_string();
        for entry in &renames {
            if let Some(next) = entry["moved"].get(&current).and_then(Value::as_str) {
                current = next.to_string();
            }
        }
        current
    };
    let mut kept = Vec::new();
    let mut removed = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for candidate in &mine {
        let address = field(candidate, "address")?;
        if !seen.insert(address.to_string()) {
            bail!("boundary candidate `{address}` is recorded twice");
        }
        if field(candidate, "facet")?.trim().is_empty() {
            bail!("boundary candidate `{address}` names no facet");
        }
        let supplied = candidate["supplied"] == true;
        let review = reviews
            .iter()
            .find(|r| field(r, "address").is_ok_and(|a| a == address));
        let verdict = match review {
            Some(review) => field(review, "verdict")?.to_string(),
            // A supplied candidate is the one an independent review decides; a part the
            // book itself gave needs no review.
            None if supplied => {
                bail!("the supplied boundary candidate `{address}` has no independent review")
            }
            None => "book".to_string(),
        };
        if verdict == "unreviewed" {
            bail!("the supplied boundary candidate `{address}` has no independent review");
        }
        if !["wrong", "ok", "book"].contains(&verdict.as_str()) {
            bail!("unknown boundary review verdict `{verdict}` for `{address}`");
        }
        let now = resolve(&address);
        match deletions
            .iter()
            .find(|d| field(d, "address").is_ok_and(|a| a == address))
        {
            // A rejected supplied part, or the role part that named it, is removed through
            // the archive's own confirmed deletion: the record names what went and the
            // confirmation issued for exactly that Card.
            Some(deletion) => {
                if verdict == "wrong"
                    && review
                        .map(|r| r["reason"].as_str().unwrap_or_default().trim().is_empty())
                        .unwrap_or(true)
                {
                    bail!("the rejected boundary candidate `{address}` names no reason");
                }
                if verdict != "wrong" && supplied {
                    bail!("the supplied boundary candidate `{address}` was deleted without a review judging it wrong");
                }
                if field(deletion, "confirm")?.trim().is_empty()
                    || field(deletion, "removed")?.trim().is_empty()
                    || field(deletion, "reason")?.trim().is_empty()
                {
                    bail!("the deletion of `{address}` records no confirmed operation");
                }
                removed.push(json!({"address":address,"reason":field(deletion, "reason")?,"removed":field(deletion, "removed")?}));
            }
            None => {
                if verdict == "wrong" {
                    bail!("the rejected boundary candidate `{address}` was not deleted through the confirmed path");
                }
                if supplied && verdict != "ok" {
                    bail!("the kept supplied boundary candidate `{address}` has no review judging it");
                }
                kept.push((address.to_string(), now, field(candidate, "fingerprint")?.to_string()));
            }
        }
    }
    // The archive decides which parts are there: a kept candidate is the same Card at
    // the address the recorded mapping gives it, and a rejected one is gone from the
    // unit's own model. Nothing may have vanished behind the count.
    for candidate in &mine {
        let Some(partner) = candidate["partner"].as_str().filter(|p| !p.is_empty()) else {
            continue;
        };
        if !removed
            .iter()
            .any(|r| field(r, "address").is_ok_and(|a| a == partner))
        {
            continue;
        }
        let address = field(candidate, "address")?;
        if !deletions
            .iter()
            .any(|d| field(d, "address").is_ok_and(|a| a == address))
        {
            bail!(
                "the part `{address}` that names the rejected `{partner}` was not deleted through the confirmed path"
            );
        }
    }
    let kept_addresses = kept.iter().map(|(_, now, _)| now.clone()).collect::<Vec<_>>();
    let kept_cards = if kept_addresses.is_empty() {
        Vec::new()
    } else {
        zt.cards(&kept_addresses, "text")?.cards
    };
    if kept_cards.len() != kept.len() {
        bail!("a kept boundary candidate of `{at}` is gone from the archive");
    }
    for ((address, now, fingerprint), card) in kept.iter().zip(&kept_cards) {
        if field(card, "address")? != now || crate::calls::authored_fingerprint(card) != *fingerprint
        {
            bail!("the kept boundary candidate `{address}` is no longer the part this settle recorded");
        }
    }
    if !removed.is_empty() {
        let model = zt.call("get_model", json!({"at":at}))?;
        let parts = model["parts"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|part| field(part, "address").map(str::to_string))
            .collect::<Result<Vec<_>>>()?;
        let current = if parts.is_empty() {
            Vec::new()
        } else {
            zt.cards(&parts, "text")?.cards
        };
        let present = current
            .iter()
            .map(crate::calls::authored_fingerprint)
            .collect::<std::collections::BTreeSet<_>>();
        for part in &removed {
            if present.contains(field(part, "removed")?) {
                bail!(
                    "the rejected boundary candidate `{}` is recorded as deleted and is still part of {at}",
                    field(part, "address")?
                );
            }
        }
    }
    let result = json!({
        "schema_version":1,
        "kind":"boundary_completion",
        "unit":unit,
        "token":token,
        "layer":"boundary",
        "state":"done",
        "reused":false,
        "input_hash":claimed.input_hash,
        "disposition":"settled",
        "candidates":mine.len(),
        "kept":kept.len(),
        "removed":removed,
        "renames":renames,
        "need":plan["need"],
        "checks":{"candidates":mine.len(),"kept":kept.len(),"removed":removed.len()}
    });
    let outputs = kept_cards
        .iter()
        .map(|card| {
            (
                format!("card/{}", field(card, "address").unwrap_or_default()),
                "card".to_string(),
                field(card, "address").unwrap_or_default().to_string(),
                crate::calls::authored_fingerprint(card),
                json!({"purpose":"boundary_candidate"}),
            )
        })
        .collect::<Vec<_>>();
    math_record(p, unit, token, &claimed, done, result, &outputs, "math/completion-boundary.json")
}

/// One selected formal result's proof review (R240). Every proof step of the unit as it
/// stands now has a verdict: an accepted fix was written and kept every Link it had, an
/// untouched step is the very Card the review read, and a refused correction stays a
/// named obligation instead of quietly completing.
fn proof_finish(p: &Prepared, unit: &str, token: i64, zt: &mut Zt, path: &Path) -> Result<Value> {
    let registered = unit_inputs(p, unit)?;
    let plan = registered["plan"].clone();
    let at = field(&plan, "unit")?.to_string();
    let _guard = unit_guard(p, unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, unit, token)?;
    let expected = p.out.join("math").join("proof").join("applied.json");
    let path = std::fs::canonicalize(path).context("proof apply evidence does not exist")?;
    if path != std::fs::canonicalize(&expected)? {
        bail!("`{unit}` completes at the applied proof record: {}", expected.display());
    }
    let applied = util::read_json(&path).context("invalid applied proof record")?;
    if applied["schema_version"] != 1 || applied["kind"] != "proof_apply" {
        bail!("invalid applied proof record");
    }
    let planned = plan["steps"]
        .as_array()
        .context("the proof plan names no step")?
        .iter()
        .map(|step| field(step, "address").map(str::to_string))
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
    // The current proof decides: every main-line step the review must cover is the step
    // list of the unit's own model now, not the list the plan remembered.
    let model = zt.call("get_model", json!({"at":at}))?;
    let current = model["build"]["order"]
        .as_array()
        .context("the unit's model has no build order")?
        .iter()
        .filter(|step| step["line"] == "main")
        .filter(|step| {
            step["facet"]
                .as_str()
                .is_some_and(crate::math::is_proof_step)
        })
        .map(|step| field(step, "address").map(str::to_string))
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
    if current.is_empty() {
        bail!("`{unit}` was selected for proof review and its model has no proof step now");
    }
    if current != planned {
        bail!("the proof of `{at}` changed since it was planned; re-run the proof review");
    }
    let reviewed = applied["reviewed"]
        .as_array()
        .context("the applied proof record holds no review")?
        .iter()
        .filter(|r| r["unit"] == json!(at))
        .cloned()
        .collect::<Vec<_>>();
    let mut seen = std::collections::BTreeSet::new();
    let mut fixed = 0;
    let mut untouched = Vec::new();
    for step in &current {
        let entry = reviewed
            .iter()
            .find(|r| field(r, "address").is_ok_and(|a| a == step))
            .with_context(|| format!("the selected proof step `{step}` was not reviewed"))?;
        if !seen.insert(step.clone()) {
            bail!("the proof step `{step}` is reviewed twice");
        }
        let verdict = field(entry, "verdict")?;
        match verdict {
            "ok" => untouched.push(step.clone()),
            "fix" => {
                if field(entry, "before")? == field(entry, "after")? {
                    bail!("the accepted proof fix for `{step}` did not change the step");
                }
                fixed += 1;
            }
            "refused" => bail!(
                "the refused correction of `{step}` in `{at}` remains a named obligation: {}",
                field(entry, "reason")?
            ),
            other => bail!("unknown proof verdict `{other}` for `{step}`"),
        }
    }
    let addresses = current.iter().cloned().collect::<Vec<_>>();
    let cards = zt.cards(&addresses, "text")?.cards;
    if cards.len() != addresses.len() {
        bail!("a reviewed proof step of `{at}` is gone from the archive");
    }
    for (address, card) in addresses.iter().zip(&cards) {
        let entry = reviewed
            .iter()
            .find(|r| field(r, "address").is_ok_and(|a| a == address))
            .context("missing proof review entry")?;
        let fingerprint = crate::calls::authored_fingerprint(card);
        let body = card["body"].as_str().unwrap_or_default();
        match field(entry, "verdict")? {
            "ok" => {
                if fingerprint != field(entry, "before")? {
                    bail!("the reviewed proof step `{address}` changed after its review without an accepted fix");
                }
            }
            "fix" => {
                if fingerprint != field(entry, "after")? {
                    bail!("the accepted proof fix for `{address}` is not the step the archive holds");
                }
                // A correction keeps every Link the step had: the plan cites its premises.
                for link in entry["kept_links"]
                    .as_array()
                    .context("the accepted proof fix records no kept Link")?
                {
                    let link = link.as_str().context("invalid kept Link")?;
                    if !body.contains(&format!("[[{link}")) {
                        bail!("the accepted proof fix for `{address}` dropped the Link to `{link}`");
                    }
                }
            }
            _ => {}
        }
    }
    let result = json!({
        "schema_version":1,
        "kind":"proof_completion",
        "unit":unit,
        "token":token,
        "layer":"proof",
        "state":"done",
        "reused":false,
        "input_hash":claimed.input_hash,
        "disposition":"reviewed",
        "steps":current.len(),
        "fixed":fixed,
        "untouched":untouched.len(),
        "checks":{"proof_steps":current.len(),"fixed":fixed}
    });
    let outputs = cards
        .iter()
        .map(|card| {
            (
                format!("card/{}", field(card, "address").unwrap_or_default()),
                "card".to_string(),
                field(card, "address").unwrap_or_default().to_string(),
                crate::calls::authored_fingerprint(card),
                json!({"purpose":"proof_step"}),
            )
        })
        .collect::<Vec<_>>();
    math_record(p, unit, token, &claimed, done, result, &outputs, "math/completion-proof.json")
}

/// `math-check/book@1`: the M1–M18 checks describe the revision the audit published.
/// The report is re-run against the archive the run holds now and must be exactly the
/// retained one, so a stale or hand-written report cannot complete the layer (R241).
fn math_check_finish(p: &Prepared, unit: &str, token: i64, zt: &mut Zt, path: &Path) -> Result<Value> {
    let _guard = unit_guard(p, unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, unit, token)?;
    let expected = p.out.join("math-audit.json");
    let path = std::fs::canonicalize(path).context("math check evidence does not exist")?;
    if path != std::fs::canonicalize(&expected)? {
        bail!("`{unit}` completes at the book's math audit: {}", expected.display());
    }
    let retained = util::read_json(&path).context("invalid retained math audit")?;
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let audit = required_publication(&conn, "audit")?
        .context("the final audit revision was never published")?;
    // A check describes a revision the ledger has verified, never one that is still open
    // or already failed: the final audit revision is the checked one (R241).
    let audit_state: String =
        conn.query_row("SELECT state FROM units WHERE id=?1", [&audit.0], |r| r.get(0))?;
    if audit_state != "done" {
        bail!("the final audit revision `{}` is not checked yet", audit.0);
    }
    let revision = audit
        .0
        .rsplit_once('@')
        .and_then(|(_, r)| r.parse::<i64>().ok())
        .context("invalid audit revision")?;
    // Read the checks again without writing: a refused completion must leave the
    // retained evidence exactly as the run published it.
    let fresh = crate::math::check_value(zt, field(&p.contract, "citation_key")?)?;
    if fresh["units"] != retained["units"]
        || fresh["theories"] != retained["theories"]
        || fresh["forms"] != retained["forms"]
        || fresh["by_rule"] != retained["by_rule"]
        || fresh["list"] != retained["list"]
    {
        bail!("the retained math audit is not the M1–M18 report of the current revision");
    }
    let result = json!({
        "schema_version":1,
        "kind":"math_check_completion",
        "unit":unit,
        "token":token,
        "layer":"math-check",
        "state":"done",
        "reused":false,
        "input_hash":claimed.input_hash,
        "disposition":if retained["faults"] == 0 {"clean"} else {"faults"},
        "units":retained["units"],
        "theories":retained["theories"],
        "faults":retained["faults"],
        "by_rule":retained["by_rule"],
        "audit":audit.0,
        "audit_revision":revision,
        "checks":{"units":retained["units"],"theories":retained["theories"],"faults":retained["faults"]}
    });
    math_record(p, unit, token, &claimed, done, result, &[], "math/completion-check.json")
}

/// Complete one note suggestion: it is applied as a checked Link, already present,
/// or refused with a source or relationship reason (R224). The archive read decides
/// whether the proposed Link is authored now, and the registered Card state decides
/// whether this pass changed the Card.
fn note_disposition(p: &Prepared, zt: &mut Zt, unit: &str, token: i64, path: &Path) -> Result<Value> {
    let (layer, rest) = unit.split_once('/').context("invalid note unit id")?;
    let (id, revision) = rest.rsplit_once('@').context("invalid note unit id")?;
    if layer != "notes" || revision != "1" {
        bail!("`{unit}` is not a note suggestion unit");
    }
    let _guard = unit_guard(p, unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, unit, token)?;
    let registered = unit_inputs(p, unit)?;
    let expected = p.out.join(format!("note-{id}.json"));
    if std::fs::canonicalize(path).context("note disposition evidence does not exist")?
        != std::fs::canonicalize(&expected)?
    {
        bail!("note disposition evidence locator mismatch");
    }
    let plan = util::read_json(path).context("invalid note disposition evidence")?;
    if plan["schema_version"] != 1
        || plan["kind"] != "note_disposition"
        || plan["unit"] != unit
        || plan["token"] != token
        || plan["suggestion"] != registered
    {
        bail!("note disposition identity mismatch");
    }
    let outcome = field(&plan, "outcome")?;
    if !["applied", "already_present", "refused"].contains(&outcome) {
        bail!("note disposition outcome must be applied, already_present or refused");
    }
    let reason = plan["reason"].as_str().unwrap_or_default().trim().to_string();
    let at = field(&registered, "card")?;
    let card = zt.cards(&[at.to_string()], "text")?;
    let card = card
        .cards
        .first()
        .with_context(|| format!("note suggestion {unit} names a missing Card `{at}`"))?;
    let body = card["body"].as_str().unwrap_or_default();
    let target = field(&registered, "target")?;
    let present = body.contains(&format!("[[{target}"));
    let fingerprint = crate::calls::authored_fingerprint(card);
    match outcome {
        "applied" if !present => {
            bail!("note disposition claims an applied Link that `{at}` does not author")
        }
        "applied" if fingerprint == registered["fingerprint"] => bail!(
            "note disposition claims an applied Link without changing `{at}`; report already_present instead"
        ),
        "already_present" if !present => {
            bail!("note disposition claims a present Link that `{at}` does not author")
        }
        "already_present" if fingerprint != registered["fingerprint"] => {
            bail!("`{at}` changed since the suggestion was registered; re-register the suggestion")
        }
        "refused" if reason.is_empty() => {
            bail!("a refused note suggestion must give its source or relationship reason")
        }
        _ => {}
    }
    let mut result = json!({
        "schema_version":1,
        "kind":"note_disposition",
        "unit":unit,
        "token":token,
        "state":"done",
        "reused":false,
        "input_hash":claimed.input_hash,
        "suggestion":registered,
        "outcome":outcome,
        "reason":reason,
        "present":present,
        "fingerprint":fingerprint
    });
    let disposition_path = p.out.join(format!("note-disposition-{id}.json"));
    if done {
        let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
        let stored: String =
            conn.query_row("SELECT result_json FROM units WHERE id=?1", [unit], |r| {
                r.get(0)
            })?;
        let mut stored: Value = serde_json::from_str(&stored)?;
        check_file(&stored["disposition"], "note disposition")?;
        stored["reused"] = json!(true);
        return Ok(stored);
    }
    let disposition = publish(&disposition_path, &serde_json::to_string_pretty(&result)?)?;
    result["disposition"] = disposition.clone();
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, unit, token)?;
    output(
        &tx,
        unit,
        "record",
        "file",
        field(&disposition, "path")?,
        field(&disposition, "sha256")?,
        &disposition,
    )?;
    output(
        &tx,
        unit,
        &format!("card/{at}"),
        "card",
        at,
        &fingerprint,
        &json!({"purpose":"note_suggestion","outcome":outcome}),
    )?;
    tx.execute("UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",params![unit,canon::canonical_json(&result),canon::utc_now_o()])?;
    event(
        &tx,
        unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"outcome":outcome,"card":at,"present":present}),
    )?;
    tx.commit()?;
    Ok(result)
}

/// Existing addresses among `wanted`, read in bounded chunks. A missing address is
/// explicit input, never an empty successful read.
fn existing_cards(zt: &mut Zt, wanted: &[String]) -> Result<std::collections::BTreeSet<String>> {
    let mut present = std::collections::BTreeSet::new();
    for chunk in wanted.chunks(200) {
        let reply = zt.call("get_cards", json!({"addresses":chunk,"view":"text"}))?;
        for card in reply["cards"].as_array().into_iter().flatten() {
            if let Some(at) = card["address"].as_str() {
                present.insert(at.to_string());
            }
        }
    }
    Ok(present)
}

/// A recorded structural mapping: either one JSON object of old-to-new addresses
/// or the helper's `-Moves` log of `{"i":n,"moved":{...}}` records. Chained moves
/// compose, so an address that moved twice ends at its final place.
fn recorded_moves(path: &str) -> Result<std::collections::BTreeMap<String, String>> {
    let text = util::read_text(path).context("invalid recorded move map")?;
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut collect = |moved: &serde_json::Map<String, Value>| -> Result<()> {
        for (old, new) in moved {
            if old.trim().is_empty() {
                bail!("a recorded move must name the address it replaces");
            }
            let new = new
                .as_str()
                .filter(|new| !new.trim().is_empty())
                .with_context(|| format!("recorded move `{old}` must name its new address"))?;
            pairs.push((old.clone(), new.to_string()));
        }
        Ok(())
    };
    let mut handled = false;
    if let Ok(Value::Object(object)) = serde_json::from_str::<Value>(text.trim()) {
        if let Some(moved) = object.get("moved").and_then(Value::as_object) {
            collect(moved)?;
            handled = true;
        } else if object.values().all(Value::is_string) {
            collect(&object)?;
            handled = true;
        }
    }
    if !handled {
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let record: Value = serde_json::from_str(line).context("invalid recorded move line")?;
            if let Some(moved) = record["moved"].as_object() {
                collect(moved)?;
            }
        }
    }
    if pairs.len() > 512 {
        bail!("a recorded move map names more than 512 moves");
    }
    let direct = pairs.iter().cloned().collect::<std::collections::BTreeMap<_, _>>();
    let mut composed = std::collections::BTreeMap::new();
    for (old, _) in &pairs {
        let mut at = old.clone();
        let mut seen = std::collections::BTreeSet::new();
        while let Some(next) = direct.get(&at) {
            if !seen.insert(at.clone()) {
                break;
            }
            at = next.clone();
        }
        if at != *old {
            composed.insert(old.clone(), at);
        }
    }
    Ok(composed)
}

/// Apply a recorded structural mapping to every Card address this run published.
/// A published address the archive no longer has, with no recorded mapping that
/// explains it, is a precise conflict rather than a silent locator (T21).
fn relocate(p: &Prepared, zt: &mut Zt, path: &str) -> Result<Value> {
    let mapping = recorded_moves(path)?;
    if !mapping.is_empty() {
        let targets = mapping.values().cloned().collect::<Vec<_>>();
        let present = existing_cards(zt, &targets)?;
        if let Some(missing) = targets.iter().find(|at| !present.contains(at.as_str())) {
            bail!("a recorded move must name a Card that exists, not `{missing}`");
        }
    }
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let rows = {
        let mut statement = tx.prepare(
            "SELECT unit_id,output_key,locator FROM outputs WHERE kind='card' ORDER BY unit_id,output_key",
        )?;
        statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let unexplained = rows
        .iter()
        .filter(|(_, _, at)| !mapping.contains_key(at))
        .map(|(_, _, at)| at.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let present = existing_cards(zt, &unexplained)?;
    let vanished = unexplained
        .iter()
        .filter(|at| !present.contains(at.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !vanished.is_empty() {
        bail!(
            "unreconciled_move: addresses this run published have moved without a recorded mapping: {}",
            vanished.join(", ")
        );
    }
    let mut applied = 0;
    let mut units = std::collections::BTreeSet::new();
    for (unit, key, at) in &rows {
        let Some(new) = mapping.get(at) else {
            continue;
        };
        let taken: i64 = tx.query_row(
            "SELECT count(*) FROM outputs WHERE unit_id=?1 AND output_key=?2",
            params![unit, format!("card/{new}")],
            |r| r.get(0),
        )?;
        if taken != 0 {
            bail!("a recorded move would duplicate the recorded locator card/{new} of {unit}");
        }
        tx.execute(
            "UPDATE outputs SET output_key=?2,locator=?3 WHERE unit_id=?1 AND output_key=?4",
            params![unit, format!("card/{new}"), new, key],
        )?;
        event(
            &tx,
            unit,
            None,
            None,
            "relocated",
            &json!({"from":at,"to":new}),
        )?;
        units.insert(unit.clone());
        applied += 1;
    }
    let run = meta(&tx, "run_id")?;
    tx.commit()?;
    let manifests = relocate_manifests(&p.out, &mapping)?;
    Ok(json!({
        "schema_version":1,
        "run_id":run,
        "reused":applied == 0 && manifests.is_empty(),
        "applied":applied,
        "moves":mapping,
        "units":units.into_iter().collect::<Vec<_>>(),
        "manifests":manifests
    }))
}

/// The staging manifests that name owned Card addresses follow the same recorded moves: each line of an
/// `owned-gNN.txt` that is a moved address, and the address before the tab of each `src-gNN.txt` line. On
/// grossman2002integration trial 11 Organize moved `374/1|b` to `374/1|a|a`, `owned-g01.txt` kept the old
/// address, and every later card cycle stopped at "incomplete Card read: … missing [\"374/1|b\"]". Returns the
/// files it rewrote.
pub(crate) fn relocate_manifests(out: &Path, mapping: &std::collections::BTreeMap<String, String>) -> Result<Vec<String>> {
    let mut rewritten = Vec::new();
    if mapping.is_empty() {
        return Ok(rewritten);
    }
    for entry in std::fs::read_dir(out).with_context(|| format!("read the staging folder {}", out.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let owned = name.starts_with("owned-g") && name.ends_with(".txt");
        let source = name.starts_with("src-g") && name.ends_with(".txt");
        if !(owned || source) || !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let mut changed = false;
        let lines: Vec<String> = text
            .lines()
            .map(|line| {
                let (at, rest) = match line.split_once('\t') {
                    Some((at, rest)) if source => (at, Some(rest)),
                    _ => (line, None),
                };
                match mapping.get(at.trim()) {
                    Some(new) => {
                        changed = true;
                        match rest {
                            Some(rest) => format!("{new}\t{rest}"),
                            None => new.clone(),
                        }
                    }
                    None => line.to_string(),
                }
            })
            .collect();
        if changed {
            let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
            let mut body = lines.join(ending);
            if text.ends_with('\n') {
                body.push_str(ending);
            }
            util::write_text(&path, &body)?;
            rewritten.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    rewritten.sort();
    Ok(rewritten)
}

/// The compiler artifacts that carry one publication layer's checked result. The
/// workflow names the real files; the ledger binds their canonical path and hash.
pub(crate) fn publication_artifacts(layer: &str) -> Option<&'static [&'static str]> {
    Some(match layer {
        "organize" => &["decisions", "applied"],
        "skeleton" => &["skeleton"],
        "theory" => &["applied"],
        "figures" | "bridge" | "hubs" | "audit" | "cards-book" | "link-check" => &["report"],
        _ => return None,
    })
}

/// A publication unit id as its layer, logical key and revision.
pub(crate) fn publication_unit(unit: &str) -> Result<(String, String, i64)> {
    let (layer, rest) = unit.split_once('/').context("invalid publication unit id")?;
    let (key, revision) = rest.rsplit_once('@').context("invalid publication unit id")?;
    if !PUBLICATION.contains(&layer) {
        bail!("`{unit}` is not an aggregate publication unit");
    }
    let revision = revision
        .parse::<i64>()
        .ok()
        .filter(|n| *n > 0)
        .context("invalid publication revision")?;
    Ok((layer.to_string(), key.to_string(), revision))
}

/// Bind an existing compiler artifact by its canonical path and content hash.
fn bind_file(path: &str, label: &str) -> Result<Value> {
    let canonical = std::fs::canonicalize(path).with_context(|| format!("{label} does not exist"))?;
    let bytes = std::fs::read(&canonical).with_context(|| format!("read {label}"))?;
    Ok(json!({"path":canonical.to_string_lossy(),"sha256":canon::sha256_hex(&bytes)}))
}

pub(crate) fn unit_inputs(p: &Prepared, unit: &str) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let raw: String = conn.query_row("SELECT inputs_json FROM units WHERE id=?1", [unit], |r| {
        r.get(0)
    })?;
    canon::parse_json(&raw)
}

/// One completed producer's retained result, or null while it has not completed.
fn unit_result(p: &Prepared, unit: &str) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let raw: Option<String> = conn
        .query_row("SELECT result_json FROM units WHERE id=?1", [unit], |r| {
            r.get(0)
        })
        .optional()?;
    match raw {
        Some(raw) => serde_json::from_str(&raw).context("invalid producer result"),
        None => Ok(Value::Null),
    }
}

/// Every registered note suggestion with the outcome its own unit retained.
pub(crate) fn registered_notes(p: &Prepared) -> Result<std::collections::BTreeMap<String, Value>> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let mut statement =
        conn.prepare("SELECT id,result_json FROM units WHERE layer='notes' ORDER BY id")?;
    let rows = statement
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut notes = std::collections::BTreeMap::new();
    for (id, result) in rows {
        notes.insert(
            id,
            match result {
                Some(result) => serde_json::from_str(&result).context("invalid note result")?,
                None => Value::Null,
            },
        );
    }
    Ok(notes)
}

/// The prepared members one publication layer must classify exactly once. A
/// skeleton indexes the prepared groups, so every anchor is either verified
/// substantive work or a named gap (R226).
fn publication_coverage(p: &Prepared, layer: &str) -> Result<Option<Vec<Value>>> {
    match layer {
        "skeleton" => Ok(Some(
            p.contract["groups"]
                .as_array()
                .context("missing prepared groups")?
                .clone(),
        )),
        _ => Ok(None),
    }
}

/// Verify one immutable Bridge report and its exact proposal set (R229). Its id
/// must recompute from its own content, and every proposal must reach an explicit
/// outcome: an open proposal is a review decision, never a missing digest write.
fn verify_bridge_report(p: &Prepared, report_path: &str, plan: &Value) -> Result<Value> {
    let report = util::read_json(report_path).context("invalid Bridge report")?;
    let key = field(&p.contract, "citation_key")?;
    let id = field(&report, "report_id").context("Bridge report has no report_id")?;
    if report["schema"] != 1
        || report["citation_key"] != key
        || id.len() != 66
        || !id.starts_with("r-")
        || !id[2..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        bail!("a Bridge publication requires an immutable report for `{key}`");
    }
    if id != format!("r-{}", canon::content_hash(&report, &["report_id", "created_at"])) {
        bail!("the Bridge report `{id}` does not match its own content");
    }
    let proposals = report["proposals"]
        .as_array()
        .context("Bridge report is missing its proposals")?;
    let mut members = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for proposal in proposals {
        let proposal_id = field(proposal, "id")?;
        if proposal_id.len() != 4
            || !proposal_id.starts_with('b')
            || !proposal_id[1..].chars().all(|c| c.is_ascii_digit())
            || !seen.insert(proposal_id.to_string())
        {
            bail!("Bridge report proposal `{proposal_id}` is not one unique readable id");
        }
        members.push(proposal_id.to_string());
    }
    let entries = plan["coverage"]
        .as_array()
        .context("publication completion is missing its checked coverage")?;
    if entries.len() != members.len() {
        bail!("Bridge coverage must classify every proposal in the report exactly once");
    }
    let mut checked = Vec::new();
    let mut recorded = std::collections::BTreeSet::new();
    for entry in entries {
        let reference = field(entry, "ref")?.to_string();
        if !members.contains(&reference) || !recorded.insert(reference.clone()) {
            bail!("Bridge coverage names `{reference}`, which is not one unclassified proposal");
        }
        let outcome = field(entry, "outcome")?;
        let reason = entry["reason"].as_str().unwrap_or_default().trim().to_string();
        match outcome {
            "accepted" | "open" => {}
            "refused" if !reason.is_empty() => {}
            "refused" => bail!("Bridge coverage must give a reason for the refused proposal `{reference}`"),
            _ => bail!("unknown bridge coverage outcome `{outcome}`"),
        }
        checked.push(json!({"ref":reference,"outcome":outcome,"reason":reason}));
    }
    Ok(json!(checked))
}

/// Verify the layer's checked coverage: every required member exactly once, no
/// invented member, and no fabricated coverage for a producer that did not
/// produce substantive work.
fn verify_coverage(
    p: &Prepared,
    zt: &mut Zt,
    layer: &str,
    manifest: &Value,
    plan: &Value,
    artifacts: &serde_json::Map<String, Value>,
) -> Result<Value> {
    if layer == "theory" {
        return verify_theory_coverage(p, zt, plan, artifacts);
    }
    if layer == "bridge" {
        let report = artifacts
            .get("report")
            .and_then(|record| record["path"].as_str())
            .context("a Bridge publication requires its immutable report")?;
        return verify_bridge_report(p, report, plan);
    }
    if layer == "figures" {
        let prepared = util::read_json(field(&p.contract["artifacts"]["figures_file"], "path")?)?;
        let recognized = prepared["figures"]
            .as_array()
            .context("the prepared figures file is missing its references")?;
        let entries = plan["coverage"]
            .as_array()
            .context("publication completion is missing its checked coverage")?;
        if entries.len() != recognized.len() {
            bail!("figure coverage must classify every recognized reference exactly once");
        }
        let mut checked = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for entry in entries {
            let reference = field(entry, "ref")?.to_string();
            if !seen.insert(reference.clone()) {
                bail!("figure coverage names `{reference}` twice");
            }
            let figure = recognized
                .iter()
                .find(|figure| figure["name"] == json!(reference))
                .with_context(|| {
                    format!("figure coverage names `{reference}`, which no source reference matches")
                })?;
            let outcome = field(entry, "outcome")?;
            let reason = entry["reason"].as_str().unwrap_or_default().trim().to_string();
            match outcome {
                "shown" => {
                    // Artifact integrity: a shown reference must be the prepared one, unchanged.
                    for field_name in ["sha256", "path"] {
                        if entry[field_name] != figure[field_name] {
                            bail!("figure coverage shows `{reference}` with a changed {field_name}");
                        }
                    }
                }
                "unmatched" | "unresolved" if !reason.is_empty() => {}
                "unmatched" | "unresolved" => {
                    bail!("figure coverage must give a reason for `{reference}`")
                }
                _ => bail!("unknown figure coverage outcome `{outcome}`"),
            }
            checked.push(json!({"ref":reference,"outcome":outcome,"reason":reason,"path":figure["path"],"sha256":figure["sha256"]}));
        }
        return Ok(json!(checked));
    }
    if layer == "cards-book" {
        return verify_cards_book_coverage(p, zt, artifacts);
    }
    if layer == "organize" {
        // Organize disposes every registered note suggestion, and the ledger checks
        // the recorded disposition against the suggestion's own retained outcome.
        let recorded = registered_notes(p)?;
        let entries = plan["coverage"]
            .as_array()
            .context("publication completion is missing its checked coverage")?;
        if entries.len() != recorded.len() {
            bail!("organize coverage must dispose every registered note suggestion exactly once");
        }
        let mut checked = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for entry in entries {
            let reference = field(entry, "ref")?.to_string();
            if !seen.insert(reference.clone()) {
                bail!("organize coverage names `{reference}` twice");
            }
            let outcome = field(entry, "outcome")?;
            if !["applied", "already_present", "refused"].contains(&outcome) {
                bail!("organize coverage outcome for `{reference}` must be applied, already_present or refused");
            }
            let suggestion = recorded
                .get(&reference)
                .with_context(|| format!("organize coverage names `{reference}`, which is not a registered suggestion"))?;
            let disposition = suggestion["outcome"].as_str().with_context(|| {
                format!("{reference} has no retained suggestion outcome yet")
            })?;
            if disposition != outcome {
                bail!("organize coverage says `{reference}` is {outcome}, but its own outcome is {disposition}");
            }
            checked.push(json!({"ref":reference,"outcome":outcome,"reason":entry["reason"]}));
        }
        return Ok(json!(checked));
    }
    let Some(groups) = publication_coverage(p, layer)? else {
        return Ok(json!([]));
    };
    if groups.is_empty() {
        return Ok(json!([]));
    }
    let contributions = manifest["contributions"]
        .as_array()
        .context("publication manifest is missing its contributions")?;
    let entries = plan["coverage"]
        .as_array()
        .context("publication completion is missing its checked coverage")?;
    if entries.len() != groups.len() {
        bail!("publication coverage must classify every required member exactly once");
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut checked = Vec::new();
    for entry in entries {
        let reference = field(entry, "ref")?.to_string();
        if !seen.insert(reference.clone()) {
            bail!("publication coverage names `{reference}` twice");
        }
        let group = groups
            .iter()
            .find(|group| group["anchor"] == json!(reference))
            .with_context(|| {
                format!("publication coverage names `{reference}`, which is not a required member")
            })?;
        let producer = format!(
            "digest/g{:03}@1",
            group["n"].as_u64().context("invalid prepared group number")?
        );
        let contributing = contributions
            .iter()
            .any(|c| c["unit"] == json!(producer));
        let substantive = if contributing {
            unit_result(p, &producer)?["disposition"] != "no_cards"
        } else {
            false
        };
        let outcome = field(entry, "outcome")?;
        let reason = entry["reason"].as_str().unwrap_or_default().trim().to_string();
        match (layer, outcome) {
            ("skeleton", "indexed") if !contributing => {
                bail!("publication coverage cannot index `{reference}`: its producer is unresolved")
            }
            ("skeleton", "indexed") if !substantive => {
                bail!("publication coverage must mark `{reference}` as a gap: its producer has no substantive Cards")
            }
            ("skeleton", "indexed") => {}
            ("skeleton", "gap" | "unresolved") => {
                if reason.is_empty() {
                    bail!("publication coverage must give a reason for `{reference}`");
                }
                if substantive {
                    bail!("publication coverage cannot report `{reference}` as {outcome}: its producer created substantive work");
                }
            }
            _ => bail!("unknown {layer} coverage outcome `{outcome}`"),
        }
        checked.push(json!({"ref":reference,"outcome":outcome,"reason":reason,"producer":producer}));
    }
    Ok(json!(checked))
}
/// R217, R247, R296-R298: the checked coverage of one whole-book Card cycle revision.
///
/// The cycle's report must describe the exact Cards it checked, and the deterministic pass is
/// re-derived here from that recorded checked set: the recorded fault lines, the Cards' current
/// authoring fingerprints and the archive they were read at must agree, so a later edit or a
/// hand-written report can never stand in for a real pass. Every Model draft of the book must
/// have one reviewed owner: the model chain's own checked source-review artifact when the layer
/// is enabled, or this cycle's recorded review when no chain elaborates it. The Idea sample
/// records its population, seed, addresses, source spans and group coverage, and it is reported
/// as a sample, never as proof for every Idea Card. A residual hard fault is a named obligation.
fn verify_cards_book_coverage(
    p: &Prepared,
    zt: &mut Zt,
    artifacts: &serde_json::Map<String, Value>,
) -> Result<Value> {
    use std::collections::{BTreeMap, BTreeSet};
    let key = field(&p.contract, "citation_key")?;
    let report_path = artifacts
        .get("report")
        .and_then(|record| record["path"].as_str())
        .context("a cards-book revision requires the whole-book Card cycle's report")?;
    let report = util::read_json(report_path).context("invalid whole-book Card cycle report")?;
    if report["kind"] != "cards_cycle" || report["unit_kind"] != "book" {
        bail!("a cards-book revision needs a whole-book Card cycle report");
    }
    if !ps::eq(&s(&report["book"]), &key) {
        bail!("the Card cycle report describes `{}`, not `{key}`", s(&report["book"]));
    }
    let audit_dir = Path::new(field(&report["audit"], "after")?);
    let cards_file = audit_dir.join("audit-cards.json");
    let faults_file = audit_dir.join("audit-faults.txt");
    let coverage_file = audit_dir.join("audit.json");
    for file in [&cards_file, &faults_file, &coverage_file] {
        if !file.is_file() {
            bail!("the Card cycle left no checked pass at {}", file.display());
        }
    }
    let cards = util::read_json(&cards_file)?;
    let cards = cards.as_array().context("the Card cycle's checked Card view is not an array")?;
    let checked_pass = util::read_json(&coverage_file)?;
    let checked: Vec<String> = checked_pass["scope"]["checked_addresses"]
        .as_array()
        .context("the checked pass names no checked Cards")?
        .iter()
        .map(&s)
        .collect();
    if checked.is_empty() {
        bail!("the whole-book Card cycle checked no Card of `{key}`");
    }
    let rules: String = match &report["rules"] {
        Value::Array(list) => list.iter().map(|rule| s(rule)).collect::<Vec<_>>().join(","),
        Value::String(text) => text.clone(),
        _ => String::new(),
    };
    if rules.trim().is_empty() {
        bail!("the whole-book Card cycle report names no rules");
    }
    let declared_rules: std::collections::BTreeSet<&str> = rules.split(',').map(str::trim).collect();
    if let Some(missing) = card_cycle_rule_missing(&declared_rules) {
        bail!("the whole-book Card cycle ran without the {missing} rule; a cards-book revision needs the full deterministic pass");
    }
    let by_address: BTreeMap<String, Value> = cards
        .iter()
        .map(|card| (ps::key(&s(&card["address"])), card.clone()))
        .collect();
    let term = cards
        .iter()
        .find(|card| s(&card["tree"]) == key && s(&card["title"]).trim_start().starts_with("术语表"))
        .cloned();
    let mut entries = Vec::new();
    for at in &checked {
        let card = by_address
            .get(&ps::key(at))
            .with_context(|| format!("the checked pass names `{at}`, which its Card view does not hold"))?;
        let body = s(&card["body"]);
        let mut entry = json!({
            "address": s(&card["address"]),
            "title": card["title"],
            "body": body,
            "category": card["category"],
            "book": card["book"],
            "parent": card["parent"],
            "links": crate::card::text::link_targets(&body),
            "kinds": crate::card::text::link_kinds(&body),
        });
        if card["inbound"].is_array() {
            entry["inbound"] = card["inbound"].clone();
        }
        if !card["inbound_kinded"].is_null() {
            entry["inbound_kinded"] = card["inbound_kinded"].clone();
        }
        entries.push(entry);
    }
    let setting = term.as_ref().map(|card| s(&card["address"])).unwrap_or_default();
    let mut all = entries.clone();
    if let Some(term) = &term {
        if !checked.iter().any(|at| ps::eq(at, &s(&term["address"]))) {
            all.push(json!({"address": term["address"], "title": term["title"], "body": term["body"]}));
        }
    }
    let mut file = tempfile::NamedTempFile::new()?;
    serde_json::to_writer(&mut file, &Value::Array(all))?;
    std::io::Write::flush(&mut file)?;
    let check = crate::check::Check {
        json: file.path().to_string_lossy().into_owned(),
        setting,
        ..Default::default()
    };
    // The pass is the cycle's own rule set: the recorded report names it.
    let lines = zt_check::check::run_lines(&check.args(), None)
        .context("the whole-book deterministic pass did not run")?;
    lines
        .last()
        .and_then(|line| line.strip_prefix("faults: "))
        .and_then(|n| n.parse::<usize>().ok())
        .context("the whole-book deterministic pass did not report completeness")?;
    let derived: BTreeSet<String> = lines[..lines.len() - 1]
        .iter()
        .filter(|line| {
            let at = line.split(' ').next().unwrap_or_default();
            checked.iter().any(|known| known == at)
        })
        .cloned()
        .collect();
    let recorded: BTreeSet<String> = std::fs::read_to_string(&faults_file)
        .context("unreadable checked fault list")?
        .replace('\r', "")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    let missing: Vec<&String> = recorded.difference(&derived).collect();
    let invented: Vec<&String> = derived.difference(&recorded).collect();
    if !missing.is_empty() || !invented.is_empty() {
        bail!(
            "the recorded deterministic pass does not describe the checked Cards (missing {}, invented {})",
            missing.iter().map(|l| l.as_str()).take(3).collect::<Vec<_>>().join(" | "),
            invented.iter().map(|l| l.as_str()).take(3).collect::<Vec<_>>().join(" | ")
        );
    }
    // The checked Cards must still be the Cards this report describes.
    let read = zt.cards(&checked, "text")?;
    let fresh: BTreeMap<String, Value> = read
        .cards
        .iter()
        .map(|card| (ps::key(&s(&card["address"])), card.clone()))
        .collect();
    for at in &checked {
        let now = fresh
            .get(&ps::key(at))
            .with_context(|| format!("the checked Card `{at}` no longer exists"))?;
        let then = by_address.get(&ps::key(at)).context("incomplete checked Card view")?;
        if crate::calls::authored_fingerprint(now) != crate::calls::authored_fingerprint(then) {
            bail!("the checked Card `{at}` changed after the cycle; run the Card cycle again");
        }
        if now["workspace"] != "agent" {
            bail!("the checked Card `{at}` is not in the agent Workspace");
        }
    }
    let mut covered: Vec<Value> = Vec::new();
    // R247: one reviewed owner per Model draft of the book.
    let declared = target(&p.contract)?;
    let models_enabled = declared["enabled"]
        .as_array()
        .context("incomplete declared target")?
        .iter()
        .any(|layer| layer == "models");
    let tree = zt.call("get_tree", json!({"tree":key}))?;
    // A Model draft is a whole-model Model Card the digest wrote: not a Model part, and not a
    // mathematical unit or Theory Card, which the mathematics layers build and check (R238).
    let model_drafts: Vec<String> = tree["cards"]
        .as_array()
        .context("incomplete book tree read")?
        .iter()
        .filter(|card| {
            card["type"] == "model"
                && card["facet"].is_null()
                && !["theory", "definition", "axiom", "theorem", "proposition", "lemma", "corollary"]
                    .iter()
                    .any(|form| card["form"] == json!(form))
        })
        .map(|card| s(&card["address"]))
        .collect();
    let reviews: Vec<Value> = report["reviews"].as_array().cloned().unwrap_or_default();
    let review_of = |at: &str| reviews.iter().find(|review| ps::eq(&s(&review["address"]), at)).cloned();
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    for at in &model_drafts {
        let current = zt.cards(&[at.clone()], "text")?;
        let fingerprint = current
            .cards
            .first()
            .map(crate::calls::authored_fingerprint)
            .unwrap_or_default();
        if models_enabled {
            let (unit, _) = model_unit_for_header(&conn, at)?.with_context(|| {
                format!("the Model draft `{at}` has no registered model unit; register its chain first")
            })?;
            let state: String = conn.query_row("SELECT state FROM units WHERE id=?1", [&unit], |row| row.get(0))?;
            if state != "done" {
                covered.push(json!({"kind":"model","ref":at,"owner":"chain","outcome":"unfinished_chain","unit":unit,"state":state}));
                continue;
            }
            let result = unit_result(p, &unit)?;
            let review = &result["source_review"];
            let path = review["path"].as_str().unwrap_or_default();
            let artifact = crate::model::source_review::read(path, at, &s(&result["plan_hash"]))
                .with_context(|| format!("the model chain of `{at}` recorded no checked source review"))?;
            if artifact["complete"] != json!(true) {
                bail!("the model chain of `{at}` holds an incomplete source review");
            }
            // R247: one writer. The elaborated Model Card must still be the Card its own chain
            // completed, so no later Card cycle changed the reviewed draft behind the review.
            if let Some(completed) = result["card_fingerprints"].get(at.as_str()).and_then(Value::as_str)
                && completed != fingerprint
            {
                bail!(
                    "the Model draft `{at}` changed after its model chain completed; the chain keeps the review, so the Card cycle must not rewrite it"
                );
            }
            covered.push(json!({"kind":"model","ref":at,"owner":"architect","outcome":"reviewed","unit":unit,"artifact":review}));
        } else {
            let review = review_of(at)
                .with_context(|| format!("the Card cycle reviewed no source for the Model draft `{at}`"))?;
            let verdict = s(&review["verdict"]);
            if verdict.is_empty() || verdict == "unreviewed" {
                bail!("the Card cycle left the Model draft `{at}` unreviewed");
            }
            if s(&review["fingerprint"]) != fingerprint {
                bail!("the cycle's review of `{at}` describes another Card");
            }
            covered.push(json!({"kind":"model","ref":at,"owner":"cycle","outcome":"reviewed","verdict":verdict,"spans":review["spans"]}));
        }
    }
    // R297: the deterministic Idea sample and its recorded coverage.
    let sample = &report["sample"];
    if sample["proves"] != "sample" {
        bail!("the Idea sample must be recorded as a sample, never as proof for every Idea Card");
    }
    let population = sample["population"].as_i64().unwrap_or(-1);
    let eligible = sample["eligible"].as_i64().unwrap_or(-1);
    let size = sample["size"].as_i64().unwrap_or(-1);
    if population < 0 || eligible < 0 || size < 0 || population < eligible {
        bail!("the Idea sample record is inconsistent");
    }
    if sample["sampled"] == json!(true) {
        if sample["seed"] != report["seed"] {
            bail!("the Idea sample names another seed than its cycle");
        }
        if size == 0 || size < sample["target"].as_i64().unwrap_or(0) {
            bail!("the Idea sample reviewed fewer Cards than its own target");
        }
        for group in sample["by_group"].as_array().context("the Idea sample names no groups")? {
            if group["eligible"].as_i64().unwrap_or(0) > 0
                && group["picked"].as_array().is_none_or(|picked| picked.is_empty())
                && sample["feasible"] == json!(true)
            {
                bail!("the Idea sample leaves group {} with no reviewed Card", s(&group["group"]));
            }
        }
        let addresses: Vec<String> = sample["addresses"]
            .as_array()
            .context("the Idea sample names no addresses")?
            .iter()
            .map(&s)
            .collect();
        let current = zt.cards(&addresses, "text")?;
        let fresh: BTreeMap<String, Value> = current
            .cards
            .iter()
            .map(|card| (ps::key(&s(&card["address"])), card.clone()))
            .collect();
        for at in &addresses {
            let review = review_of(at)
                .with_context(|| format!("the Idea sample names `{at}`, but no review outcome records it"))?;
            let now = fresh
                .get(&ps::key(at))
                .with_context(|| format!("the sampled Card `{at}` no longer exists"))?;
            if s(&review["fingerprint"]) != crate::calls::authored_fingerprint(now) {
                bail!("the review of the sampled Card `{at}` describes another Card");
            }
        }
        covered.push(json!({
            "kind": "sample", "ref": "idea-sample", "outcome": "reviewed",
            "population": population, "eligible": eligible, "size": size,
            "seed": sample["seed"], "target": sample["target"],
            "groups": sample["groups"], "covered": sample["covered"], "feasible": sample["feasible"],
            "addresses": sample["addresses"], "spans": sample["spans"],
            "source_flagged": sample["source_flagged"], "proves": "sample"
        }));
    } else {
        covered.push(json!({
            "kind": "sample", "ref": "idea-sample", "outcome": "no_eligible_cards",
            "population": population, "eligible": eligible, "size": 0, "proves": "sample"
        }));
    }
    // R298: a residual hard fault stays an explicit targeted-correction obligation. The cycle's
    // own rule set decides what it undertakes; a rule it does not run is not its obligation.
    for line in &recorded {
        let mut parts = line.splitn(3, ' ');
        let at = parts.next().unwrap_or_default();
        let rule = parts.next().unwrap_or_default();
        if !declared_rules.contains(rule) {
            continue;
        }
        covered.push(json!({"kind":"fault","ref":line,"at":at,"rule":rule,"outcome":"unresolved","obligation":"final-fix"}));
    }
    Ok(json!(covered))
}

// ==== The Cards-book/book revision (R217, R247, R296-R298) ===================================

/// The whole-book Card cycle revision this command completes: the named unit, or the latest
/// registered one. A cards-book revision is built by `ledger publish -Layer cards-book`.
fn cards_book_unit(p: &Prepared, unit: &str) -> Result<String> {
    if !unit.trim().is_empty() {
        let (layer, _, _) = publication_unit(unit)?;
        if layer != "cards-book" {
            bail!("`{unit}` is not a whole-book Card cycle revision");
        }
        return Ok(unit.to_string());
    }
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    conn.query_row(
        "SELECT id FROM units WHERE layer='cards-book' ORDER BY revision DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .optional()?
    .context("no cards-book revision is registered; publish one with ledger publish -Layer cards-book")
}

/// The publication completion evidence of one whole-book Card cycle revision: its retained
/// report as the bound artifact, every registered contribution consumed, every unsettled input
/// named instead of hidden.
fn cards_book_evidence(
    p: &Prepared,
    unit: &str,
    token: i64,
    work: &str,
) -> Result<(Value, serde_json::Map<String, Value>)> {
    let (layer, key, _revision) = publication_unit(unit)?;
    if layer != "cards-book" {
        bail!("`{unit}` is not a whole-book Card cycle revision");
    }
    if work.trim().is_empty() {
        bail!("-Work must name the card compiler's work folder");
    }
    let report = Path::new(work).join("report.json");
    if !report.is_file() {
        bail!("the whole-book Card cycle left no report at {}", report.display());
    }
    let record = bind_file(report.to_string_lossy().as_ref(), "Card cycle report")?;
    let manifest = unit_inputs(p, unit)?;
    let contributions = manifest["contributions"]
        .as_array()
        .context("publication manifest is missing its contributions")?;
    let dispositions = manifest["dispositions"]
        .as_array()
        .context("publication manifest is missing its dispositions")?;
    let consumed: Vec<Value> = contributions
        .iter()
        .map(|item| json!({"unit": item["unit"], "fingerprint": item["fingerprint"]}))
        .collect();
    let unresolved: Vec<Value> = dispositions
        .iter()
        .map(|item| json!({"unit": item["unit"], "disposition": item["disposition"], "fault": item["fault"]}))
        .collect();
    let disposition = if contributions.is_empty() && dispositions.is_empty() {
        "no_work"
    } else if unresolved.is_empty() {
        "published"
    } else {
        "partial"
    };
    let plan = json!({
        "schema_version": 1,
        "kind": "publication_completion",
        "unit": unit,
        "token": token,
        "layer": layer,
        "key": key,
        "input_hash": canon::hash_value(&manifest),
        "disposition": disposition,
        "consumed": consumed,
        "unresolved": unresolved,
        "artifacts": {"report": record["path"]},
    });
    let mut artifacts = serde_json::Map::new();
    artifacts.insert("report".into(), record);
    Ok((plan, artifacts))
}

fn book_cards_finish(
    p: &Prepared,
    zt: &mut Zt,
    unit: &str,
    token: i64,
    work: &str,
    check_only: bool,
) -> Result<Value> {
    let (_, key, revision) = publication_unit(unit)?;
    let (plan, artifacts) = cards_book_evidence(p, unit, token, work)?;
    if check_only {
        let manifest = unit_inputs(p, unit)?;
        let coverage = verify_coverage(p, zt, "cards-book", &manifest, &plan, &artifacts)?;
        return Ok(json!({
            "schema_version": 1, "check_only": true, "applied": false, "unit": unit,
            "layer": "cards-book", "key": key, "revision": revision,
            "disposition": plan["disposition"], "consumed": plan["consumed"],
            "unresolved": plan["unresolved"], "coverage": coverage, "artifacts": artifacts,
        }));
    }
    let path = p.out.join(format!("publication-cards-book-{key}-{revision}.json"));
    publish(&path, &serde_json::to_string_pretty(&plan)?)?;
    publication_finish(p, zt, unit, token, &path)
}

pub fn book_cards_finish_cli(argv: &[String]) -> Result<i32> {
    let parsed = (|| -> Result<(String, String, i64, String, bool)> {
        let a = Args::parse(argv, &["Out", "Work", "Token", "Unit"], &["CheckOnly"])?;
        if !a.positional.is_empty() || a.required("Out")?.trim().is_empty() {
            bail!("explicit -Out is required");
        }
        let token = a
            .required("Token")?
            .parse::<i64>()
            .ok()
            .filter(|number| *number > 0)
            .context("-Token must be positive")?;
        Ok((a.str("Out"), a.str("Work"), token, a.str("Unit"), a.flag("CheckOnly")))
    })();
    let (out, work, token, declared, check_only) = match parsed {
        Ok(value) => value,
        Err(error) => {
            eprintln!("zt-digest book-cards-finish: {error}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let unit = cards_book_unit(&p, &declared)?;
    let result = book_cards_finish(&p, &mut zt, &unit, token, &work, check_only)?;
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}

/// The mathematics theory revision (R241). The archive decides its members: every
/// Theory Card of the book. Each one's applied record must name the membership and
/// cited-premise dependencies its model has now, and the Card must hold exactly the
/// summary that record produced. A changed unit or cited-premise Link therefore makes
/// the older revision stale instead of silently current.
fn verify_theory_coverage(
    p: &Prepared,
    zt: &mut Zt,
    plan: &Value,
    artifacts: &serde_json::Map<String, Value>,
) -> Result<Value> {
    let key = field(&p.contract, "citation_key")?;
    let tree = zt.call("get_tree", json!({"tree":key}))?;
    let theories = tree["cards"]
        .as_array()
        .context("the book tree read is incomplete")?
        .iter()
        .filter(|card| card["type"] == "model" && card["form"] == "theory" && card["facet"].is_null())
        .map(|card| field(card, "address").map(str::to_string))
        .collect::<Result<Vec<_>>>()?;
    let record = artifacts
        .get("applied")
        .context("a theory publication requires its applied record")?;
    let applied = bound_json(record, "applied theory record")?;
    if applied["schema_version"] != 1 || applied["kind"] != "theory_apply" {
        bail!("a theory publication requires the applied theory record of this run");
    }
    let records = applied["theories"]
        .as_array()
        .context("the applied theory record names no Theory Card")?;
    let entries = plan["coverage"]
        .as_array()
        .context("publication completion is missing its checked coverage")?;
    if entries.len() != theories.len() {
        bail!(
            "theory coverage must classify every Theory Card exactly once ({} of {})",
            entries.len(),
            theories.len()
        );
    }
    let addresses = theories.clone();
    let cards = if addresses.is_empty() {
        Vec::new()
    } else {
        zt.cards(&addresses, "text")?.cards
    };
    let mut checked = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for entry in entries {
        let reference = field(entry, "ref")?.to_string();
        if !theories.contains(&reference) || !seen.insert(reference.clone()) {
            bail!("theory coverage names `{reference}`, which is not one unclassified Theory Card");
        }
        let outcome = field(entry, "outcome")?;
        let reason = entry["reason"].as_str().unwrap_or_default().trim().to_string();
        if outcome != "rebuilt" {
            if reason.is_empty() {
                bail!("theory coverage must give a reason for `{reference}`");
            }
            bail!("theory coverage cannot report `{reference}` as {outcome}: the run rebuilt its summary");
        }
        let applied = records
            .iter()
            .find(|record| record["address"] == json!(reference))
            .with_context(|| format!("the applied theory record has no entry for `{reference}`"))?;
        let model = zt.call("get_model", json!({"at":reference}))?;
        let order = model["math"]["order"]
            .as_array()
            .context("the Theory Card's model has no dependency order")?
            .iter()
            .map(|unit| field(unit, "address").map(str::to_string))
            .collect::<Result<Vec<_>>>()?;
        let members = applied["members"]
            .as_array()
            .context("the applied theory record names no member")?
            .iter()
            .map(|at| {
                at.as_str()
                    .map(str::to_string)
                    .context("invalid theory member")
            })
            .collect::<Result<Vec<_>>>()?;
        let recorded_tools = applied["tools"]
            .as_array()
            .context("the applied theory record names no cited premise")?
            .iter()
            .map(|at| {
                at.as_str()
                    .map(str::to_string)
                    .context("invalid cited premise")
            })
            .collect::<Result<Vec<_>>>()?;
        // A Theory Card takes in whole units by Link, so its membership *and* its cited
        // premises together are the units its model holds now. A rebuild that no longer
        // accounts for them is stale, whatever the Card still says (R241).
        let described = members
            .iter()
            .chain(&recorded_tools)
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        if described.len() != members.len() + recorded_tools.len()
            || described != order.iter().cloned().collect()
        {
            bail!("the applied theory record for `{reference}` does not account for its model's current units");
        }
        let card = cards
            .iter()
            .find(|card| card["address"] == json!(reference))
            .with_context(|| format!("the Theory Card `{reference}` is missing"))?;
        if crate::canon::sha256_hex(card["body"].as_str().unwrap_or_default().as_bytes())
            != field(applied, "body_sha256")?
        {
            bail!("the Theory Card `{reference}` does not hold the summary this revision applied");
        }
        let structure = crate::math::body_section(card["body"].as_str().unwrap_or_default(), "结构");
        if links_of(&structure) != members {
            bail!("the `{reference}` structure section is not its units in dependency order");
        }
        let tools_section = crate::math::body_section(card["body"].as_str().unwrap_or_default(), "所用工具");
        if links_of(&tools_section) != recorded_tools {
            bail!("the `{reference}` tool section is not its cited premises");
        }
        checked.push(json!({"ref":reference,"outcome":outcome,"reason":reason,"members":members.len(),"tools":recorded_tools.len()}));
    }
    Ok(json!(checked))
}

/// The `[[address]]` links of one Card section, in the order they appear. A ZET address
/// carries `|` in its own right, so the link text is the address verbatim.
fn links_of(section: &str) -> Vec<String> {
    let mut links = Vec::new();
    for part in section.split("[[").skip(1) {
        if let Some(end) = part.find("]]") {
            links.push(part[..end].trim().to_string());
        }
    }
    links
}


pub(crate) fn publication_finish(p: &Prepared, zt: &mut Zt, unit: &str, token: i64, path: &Path) -> Result<Value> {
    let (layer, key, revision) = publication_unit(unit)?;
    let _guard = unit_guard(p, unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, unit, token)?;
    let expected = p.out.join(format!("publication-{layer}-{key}-{revision}.json"));
    if std::fs::canonicalize(path).context("publication evidence does not exist")?
        != std::fs::canonicalize(&expected)?
    {
        bail!("publication evidence locator mismatch");
    }
    let plan = util::read_json(path).context("invalid publication evidence")?;
    if plan["schema_version"] != 1
        || plan["kind"] != "publication_completion"
        || plan["unit"] != unit
        || plan["token"] != token
        || plan["layer"] != layer
        || plan["key"] != key
        || plan["input_hash"] != claimed.input_hash
    {
        bail!("publication completion identity mismatch");
    }
    let manifest = unit_inputs(p, unit)?;
    let contributions = manifest["contributions"]
        .as_array()
        .context("publication manifest is missing its contributions")?;
    let dispositions = manifest["dispositions"]
        .as_array()
        .context("publication manifest is missing its dispositions")?;
    let name = |item: &Value| -> Result<String> { Ok(field(item, "unit")?.to_string()) };
    let expected_consumed = contributions
        .iter()
        .map(name)
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
    let expected_unresolved = dispositions
        .iter()
        .map(name)
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
    let consumed = plan["consumed"]
        .as_array()
        .context("publication completion is missing its consumed outputs")?;
    let unresolved = plan["unresolved"]
        .as_array()
        .context("publication completion is missing its unresolved dispositions")?;
    let actual_consumed = consumed
        .iter()
        .map(name)
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
    let actual_unresolved = unresolved
        .iter()
        .map(name)
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
    if actual_consumed.len() != consumed.len() || actual_unresolved.len() != unresolved.len() {
        bail!("publication completion names one manifest entry twice");
    }
    if !actual_consumed.is_subset(&expected_consumed) {
        bail!("publication completion consumes work outside its registered manifest");
    }
    if !actual_unresolved.is_subset(&expected_unresolved) {
        bail!("publication completion disposes work outside its registered manifest");
    }
    let mut accounted = actual_consumed.clone();
    accounted.extend(actual_unresolved.iter().cloned());
    if accounted.len() != expected_consumed.len() + expected_unresolved.len() {
        bail!("publication completion leaves manifest entries unaccounted for");
    }
    for entry in consumed {
        let at = name(entry)?;
        let registered = contributions
            .iter()
            .find(|item| item["unit"] == json!(at))
            .context("publication completion names an unknown contribution")?;
        if entry["fingerprint"] != registered["fingerprint"] {
            bail!("publication completion consumes a different verified output for {at}");
        }
    }
    let disposition = field(&plan, "disposition")?;
    let checked = if expected_consumed.is_empty() && expected_unresolved.is_empty() {
        "no_work"
    } else if actual_unresolved.is_empty() {
        "published"
    } else {
        "partial"
    };
    if disposition != checked {
        bail!("publication disposition `{disposition}` does not match its accounted manifest ({checked})");
    }
    let required = publication_artifacts(&layer).context("unknown publication layer")?;
    let declared = plan["artifacts"]
        .as_object()
        .context("publication completion is missing its artifacts")?;
    if declared.len() != required.len() || declared.keys().any(|n| !required.contains(&n.as_str())) {
        bail!("publication completion must name exactly the {layer} artifacts: {}", required.join(", "));
    }
    let mut artifacts = serde_json::Map::new();
    for artifact in required {
        let at = declared[*artifact]
            .as_str()
            .with_context(|| format!("{artifact} artifact path must be a string"))?;
        artifacts.insert((*artifact).to_string(), bind_file(at, artifact)?);
    }
    let coverage = verify_coverage(p, zt, &layer, &manifest, &plan, &artifacts)?;
    let mut result = json!({"schema_version":1,"kind":"publication_completion","unit":unit,"token":token,"layer":layer,"key":key,"revision":revision,"state":"done","complete":true,"reused":false,"input_hash":claimed.input_hash,"disposition":disposition,"consumed":consumed,"unresolved":unresolved,"coverage":coverage,"artifacts":artifacts});
    let completion_path = p.out.join(format!("publication-completion-{layer}-{key}-{revision}.json"));
    if done {
        let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
        let stored: String = conn.query_row("SELECT result_json FROM units WHERE id=?1", [unit], |r| {
            r.get(0)
        })?;
        let mut stored: Value = serde_json::from_str(&stored)?;
        check_file(&stored["completion"], "publication completion")?;
        stored["reused"] = json!(true);
        return Ok(stored);
    }
    let completion = publish(&completion_path, &serde_json::to_string_pretty(&result)?)?;
    result["completion"] = completion.clone();
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, unit, token)?;
    for (artifact, record) in artifacts.iter() {
        output(
            &tx,
            unit,
            artifact,
            "file",
            field(record, "path")?,
            field(record, "sha256")?,
            record,
        )?;
    }
    completion_fault(p, "after_outputs")?;
    output(
        &tx,
        unit,
        "completion",
        "file",
        field(&completion, "path")?,
        field(&completion, "sha256")?,
        &completion,
    )?;
    for record in result["coverage"].as_array().unwrap() {
        let reference = field(record, "ref")?;
        let locator = match field(record, "producer") {
            Ok(producer) => producer,
            Err(_) => reference,
        };
        output(
            &tx,
            unit,
            &format!("coverage/{reference}"),
            "coverage",
            locator,
            &canon::hash_value(record),
            record,
        )?;
    }
    tx.execute("UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",params![unit,canon::canonical_json(&result),canon::utc_now_o()])?;
    event(
        &tx,
        unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"disposition":disposition,"revision":revision,"completion":completion,"consumed":result["consumed"],"unresolved":result["unresolved"]}),
    )?;
    completion_fault(p, "before_commit")?;
    tx.commit()?;
    completion_fault(p, "after_commit")?;
    Ok(result)
}

/// The declared scope of one hub refresh. A hub refresh is coalesced by touched
/// tree, so it must name that tree and its relevant upper hubs and Register (R227);
/// the hub compiler's own report is what verifies their structure and membership.
fn hub_scope(key: &str, path: &str) -> Result<Value> {
    if path.trim().is_empty() {
        bail!("a `hubs` publication requires -Scope naming the touched tree, its required upper hubs and the Register");
    }
    let raw = util::read_json(path).context("invalid hub scope")?;
    let object = raw.as_object().context("hub scope must be a JSON object")?;
    if let Some(name) = object
        .keys()
        .find(|k| !["tree", "upper", "register"].contains(&k.as_str()))
    {
        bail!("hub scope accepts only tree, upper and register, not `{name}`");
    }
    let tree = field(&raw, "tree").context("hub scope requires its tree")?;
    if tree != key {
        bail!("hub scope tree `{tree}` differs from the requested key `{key}`");
    }
    let upper = raw["upper"]
        .as_array()
        .context("hub scope requires its required upper hubs")?
        .iter()
        .map(|at| {
            at.as_str()
                .filter(|at| !at.trim().is_empty())
                .map(str::to_string)
                .context("hub scope upper hubs must be nonempty addresses")
        })
        .collect::<Result<Vec<_>>>()?;
    if upper.len() > 64 {
        bail!("hub scope names more than 64 required upper hubs");
    }
    let register = match raw.get("register") {
        Some(Value::Null) => None,
        Some(Value::String(at)) if !at.trim().is_empty() => Some(at.clone()),
        Some(_) => bail!("hub scope register must be a nonempty address or null"),
        None => bail!("hub scope requires an explicit register address or null"),
    };
    Ok(json!({"tree":key,"upper":upper,"register":register}))
}

/// Derive and register the next revision of one aggregate publication. The same
/// sorted input manifest always returns the same revision; a changed manifest
/// registers the next one and makes every older open revision terminal.
fn publication(p: &Prepared, layer: &str, key: &str, declared: &str) -> Result<Value> {
    let scope = if layer == "hubs" {
        hub_scope(key, declared)?
    } else {
        if !declared.trim().is_empty() {
            bail!("only a `hubs` publication declares -Scope");
        }
        Value::Null
    };
    publish_revision(p, layer, key, scope)
}

/// A finalization refresh publishes a new revision whose declared scope is the exact
/// changed-Card/touched-tree manifest it consumes (R299): the same manifest reuses the
/// registered revision and a changed one derives the next, so the refresh is idempotent
/// and never an undeclared loop.
pub(crate) fn publication_refresh(p: &Prepared, layer: &str, scope: &Value) -> Result<Value> {
    publish_revision(p, layer, "book", scope.clone())
}

fn publish_revision(p: &Prepared, layer: &str, key: &str, scope: Value) -> Result<Value> {
    if !PUBLICATION.contains(&layer) {
        bail!("`{layer}` is not an aggregate publication layer");
    }
    // A layer the prepared run deliberately excluded can never gain work here.
    if !target(&p.contract)?["enabled"]
        .as_array()
        .context("missing declared target")?
        .iter()
        .any(|enabled| enabled == layer)
    {
        bail!("`{layer}` is an excluded layer of this prepared run; it cannot publish a revision");
    }
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let manifest = publication_manifest(&tx, layer, key, &scope)?;
    let input_hash = canon::hash_value(&manifest);
    // Only the latest revision of this logical key may be reused: an older revision
    // that once had the same manifest is terminal history, so returning it would hand
    // back superseded work and leave no current revision for the present inputs.
    let latest: Option<(String, i64, String)> = tx
        .query_row(
            "SELECT id,revision,input_hash FROM units WHERE layer=?1 AND logical_key=?2 ORDER BY revision DESC LIMIT 1",
            params![layer, key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let previous = latest.as_ref().map(|(_, revision, _)| *revision).unwrap_or(0);
    let (id, revision, reused) = match latest {
        Some((id, revision, hash)) if hash == input_hash => (id, revision, true),
        _ => {
            let revision = previous + 1;
            let id = register_publication_unit(&tx, layer, key, revision, &manifest, None)?;
            seal(&tx, layer)?;
            // Every older revision that is still open is terminal: it can never be
            // claimed as current work, and the report names its replacement. A revision
            // that already failed keeps its own retained fault, so a real content
            // failure is never rewritten as supersession and an earlier replacement
            // record still names the revision that made it stale.
            let superseded = {
                let mut statement = tx.prepare("SELECT id FROM units WHERE layer=?1 AND logical_key=?2 AND revision<?3 AND state NOT IN ('done','failed') ORDER BY revision")?;
                statement
                    .query_map(params![layer, key, revision], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            };
            for unit in &superseded {
                tx.execute("UPDATE units SET state='failed',owner=NULL,lease_until=NULL,retry_after=NULL,fault_class='operator',fault_json=?2 WHERE id=?1",params![unit,canon::canonical_json(&json!({"code":"superseded","by":id,"cause":"a changed publication input manifest registers a new revision"}))])?;
            }
            // An open obligation that depends on a revision this one replaces follows the
            // replacement instead of dead-ending on terminal history: the final mathematics
            // check consumes the final audit revision, whichever revision that is (R241).
            // An aggregate publication is left alone: its own manifest is immutable, and
            // the next publish derives the revision that consumes the replacement.
            let publication_layers = PUBLICATION
                .iter()
                .map(|layer| format!("'{layer}'"))
                .collect::<Vec<_>>()
                .join(",");
            let mut repointed = Vec::new();
            for unit in &superseded {
                let dependents = {
                    let query = format!(
                        "SELECT d.unit_id FROM deps d JOIN units u ON u.id=d.unit_id \
                         WHERE d.needs_id=?1 AND u.state NOT IN ('done','failed') \
                         AND u.layer NOT IN ({publication_layers}) ORDER BY d.unit_id"
                    );
                    let mut statement = tx.prepare(&query)?;
                    statement
                        .query_map([unit], |r| r.get::<_, String>(0))?
                        .collect::<rusqlite::Result<Vec<_>>>()?
                };
                for dependent in dependents {
                    tx.execute(
                        "UPDATE OR IGNORE deps SET needs_id=?2 WHERE unit_id=?1 AND needs_id=?3",
                        params![dependent, id, unit],
                    )?;
                    tx.execute(
                        "DELETE FROM deps WHERE unit_id=?1 AND needs_id=?2",
                        params![dependent, unit],
                    )?;
                    event(
                        &tx,
                        &dependent,
                        None,
                        None,
                        "dependency",
                        &json!({"from":unit,"to":id,"cause":"the prerequisite publication revision was replaced"}),
                    )?;
                    repointed.push(json!({"unit":dependent,"from":unit,"to":id}));
                }
            }
            event(
                &tx,
                &id,
                None,
                None,
                "published",
                &json!({"layer":layer,"logical_key":key,"revision":revision,"input_hash":input_hash,"superseded":superseded,"repointed":repointed,"contributions":manifest["contributions"].as_array().unwrap().len(),"dispositions":manifest["dispositions"].as_array().unwrap().len()}),
            )?;
            (id, revision, false)
        }
    };
    let superseded = superseded_units(&tx, layer, key, revision)?;
    let view = json!({
        "schema_version":1,
        "run_id":meta(&tx,"run_id")?,
        "layer":layer,
        "logical_key":key,
        "unit":id,
        "revision":revision,
        "input_hash":input_hash,
        "reused":reused,
        "superseded":superseded,
        "scope":scope,
        "contributions":manifest["contributions"],
        "dispositions":manifest["dispositions"]
    });
    tx.commit()?;
    Ok(view)
}

fn init(p: &Prepared) -> Result<Value> {
    initialize(p, true)
}

fn initialize(p: &Prepared, prepared: bool) -> Result<Value> {
    let path = p.out.join("ledger.sqlite3");
    // Existing ledgers are verified read-only: init is not an invocation/resume.
    if path.exists() {
        let conn = connect(&path, false)?;
        verify_binding(&conn, p)?;
        return Ok(json!({"schema_version":1,"run_id":meta(&conn,"run_id")?,"initialized":false}));
    }
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("create run identity: {e}"))?;
    let run = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    // Publish the immutable plan before the transaction: every durable pending
    // preparation has both its run identity and a recoverable plan locator.
    let setup_plan = if prepared {
        Value::Null
    } else {
        setup_calls(p)?;
        accept_plan(
            &p.out.join("prepare-plan.json"),
            &json!({"schema_version":1,"kind":"prepare_setup","unit":"prepare/book@1","token":1,"preparation_hash":p.hash,"operation_prefix":format!("{run}/prepare-book/1/setup"),"setup":p.contract["artifacts"]["setup_calls"]}),
        )?
    };
    let mut conn = connect(&path, true)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // Another initializer may have committed between the existence check and this lock.
    if tx
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='meta'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some()
    {
        verify_binding(&tx, p)?;
        let run = meta(&tx, "run_id")?;
        tx.commit()?;
        return Ok(json!({"schema_version":1,"run_id":run,"initialized":false}));
    }
    let tables: i64 = tx.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table'",
        [],
        |r| r.get(0),
    )?;
    if tables != 0 {
        bail!("refusing a non-ledger database");
    }
    tx.execute_batch(SCHEMA)?;
    let target = target(&p.contract)?;
    for (k, v) in [
        ("schema_version", json!(1)),
        ("run_id", json!(run)),
        ("preparation", p.contract.clone()),
        ("preparation_hash", json!(p.hash)),
        ("archive_identity", p.contract["archive_identity"].clone()),
        ("citation_key", p.contract["citation_key"].clone()),
        ("source", p.contract["source"].clone()),
        (
            "group_manifest_hash",
            p.contract["group_manifest_hash"].clone(),
        ),
        ("effective_options", p.contract["options"].clone()),
        ("contracts", p.contract["contracts"].clone()),
        ("created_at", json!(canon::utc_now_o())),
        ("invocation", json!(1)),
        ("resume_count", json!(0)),
        ("target", target.clone()),
    ] {
        put_meta(&tx, k, &v)?;
    }
    // Enumerations are seeded before any producer enumerates them; a publication
    // revision seals its own layer afterwards.
    for layer in target["enabled"].as_array().unwrap() {
        let layer = layer.as_str().unwrap();
        put_meta(
            &tx,
            &format!("barrier/{layer}"),
            &json!({"sealed":matches!(layer,"prepare"|"terms"|"digest")}),
        )?;
    }
    let prepare = register(
        &tx,
        "prepare",
        "book",
        1,
        &json!({"preparation_hash":p.hash}),
        None,
        prepared,
    )?;
    if !prepared {
        tx.execute("UPDATE units SET state='claimed',owner='prepare',claim_token=1,attempts=1,lease_until=?2,progress_json=?3 WHERE id=?1", params![prepare, now_seconds()+120, canon::canonical_json(&json!({"prepare_setup":{"plan":setup_plan}}))])?;
    }
    let terms = if target["enabled"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l == "terms")
    {
        Some(register(
            &tx,
            "terms",
            "book",
            1,
            &json!({"preparation_hash":p.hash}),
            Some(&prepare),
            false,
        )?)
    } else {
        None
    };
    if let Some(terms) = &terms {
        dependency(&tx, terms, &prepare, "success")?;
    }
    let mut groups = Vec::new();
    for group in p.contract["groups"].as_array().unwrap() {
        let id = register(
            &tx,
            "digest",
            &format!("g{:03}", group["n"].as_u64().unwrap()),
            1,
            group,
            Some(&prepare),
            false,
        )?;
        dependency(&tx, &id, &prepare, "success")?;
        if let Some(terms) = &terms {
            dependency(&tx, &id, terms, "success")?;
        }
        groups.push(id);
    }
    // These fixed obligations cannot disappear when their dynamic producers have no ready rows.
    let enabled_layer = |name: &str| {
        target["enabled"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l == name)
    };
    for &layer in BOOK.iter().filter(|l| **l != "organize") {
        if !enabled_layer(layer) {
            continue;
        }
        let manifest = publication_manifest(&tx, layer, "book", &Value::Null)?;
        register_publication_unit(&tx, layer, "book", 1, &manifest, Some(&prepare))?;
        seal(&tx, layer)?;
    }
    if p.contract["variant"] == "math" {
        for &layer in MATH_BOOK {
            if !enabled_layer(layer) {
                continue;
            }
            // A mathematics publication layer takes its input manifest from the units and
            // the prerequisite revision it consumes; a fixed obligation binds the
            // preparation and the checked result of the layer before it.
            if PUBLICATION.contains(&layer) {
                let manifest = publication_manifest(&tx, layer, "book", &Value::Null)?;
                register_publication_unit(&tx, layer, "book", 1, &manifest, Some(&prepare))?;
                seal(&tx, layer)?;
                continue;
            }
            let id = register(
                &tx,
                layer,
                "book",
                1,
                &json!({"preparation_hash":p.hash}),
                Some(&prepare),
                false,
            )?;
            dependency(&tx, &id, &prepare, "success")?;
            if let Some(previous) = math_prerequisite(layer) {
                dependency(&tx, &id, &format!("{previous}/book@1"), "success")?;
            }
            // Each mathematics layer enumerates its own book-level obligation here, so
            // its barrier is sealed from the start: the final check waits for the audit
            // revision rather than for an enumeration that never comes.
            seal(&tx, layer)?;
        }
    }
    // Organize is the tail publication: it follows the mathematics theory revision, so
    // it is registered after that revision exists and consumes its checked result (R241).
    if enabled_layer("organize") {
        let manifest = publication_manifest(&tx, "organize", "book", &Value::Null)?;
        register_publication_unit(&tx, "organize", "book", 1, &manifest, Some(&prepare))?;
        seal(&tx, "organize")?;
    }
    // The run-level cleanup owns what no single unit registered: the shared workspaces and their
    // removal. It waits (`settled`, never `success`) for the last content layer, so a halted run
    // still owes its cleanup (R323, R330).
    if enabled_layer("cleanup") {
        register(
            &tx,
            "cleanup",
            "book",
            1,
            &json!({"preparation_hash":p.hash}),
            None,
            false,
        )?;
        let tail = if p.contract["variant"] == "math" {
            "math-check/book@1"
        } else {
            "audit/book@1"
        };
        dependency(&tx, "cleanup/book@1", tail, "settled")?;
    }
    if let Some(terms) = terms.filter(|_| prepared) {
        tx.execute("UPDATE units SET state='ready' WHERE id=?1", [terms])?;
    }
    if prepared {
        // An adopted preparation is a typed adoption record, not the placeholder a completion
        // writes (R413). It declares exactly the one output row registered below and names the
        // manifest it was verified from; it claims no historic prepare command or receipt.
        let result = json!({
            "schema_version":1,
            "kind":"preparation_adoption",
            "unit":"prepare/book@1",
            "state":"done",
            "disposition":"adopted",
            "complete":true,
            "outputs":["preparation"],
            "preparation_hash":p.hash,
            "groups":groups.len(),
            "verification":{
                "mode":"read_only",
                "tool":format!("zt-digest {}", env!("CARGO_PKG_VERSION")),
                "note":"the prepared manifest was written and verified by legacy adoption; no historic prepare command is claimed",
            },
        });
        tx.execute(
            "UPDATE units SET result_json=?2 WHERE id=?1",
            params![prepare, canon::canonical_json(&result)],
        )?;
        tx.execute("INSERT INTO outputs(unit_id,output_key,kind,locator,fingerprint,data_json) VALUES(?1,'preparation','manifest',?2,?3,?4)",params![prepare,p.out.join("manifest.json").to_string_lossy(),p.hash,canon::canonical_json(&json!({"group_count":groups.len(),"preparation_hash":p.hash}))])?;
    }
    tx.execute(
        "INSERT INTO events(unit_id,invocation,utc,event,facts_json) VALUES(?1,1,?2,?3,?4)",
        params![
            prepare,
            canon::utc_now_o(),
            if prepared {
                "prepared"
            } else {
                "prepare_started"
            },
            canon::canonical_json(&json!({"groups":groups.len(),"target":target}))
        ],
    )?;
    tx.commit()?;
    Ok(json!({"schema_version":1,"run_id":run,"initialized":true}))
}

/// Preparation shares its existing MCP process; no nested transaction or client.
pub fn prepare_setup(out: &str, zt: &mut Zt) -> Result<Value> {
    let p = Prepared::read_mode(out, zt, false)?;
    initialize(&p, false)?;
    prepare_finish(&p, zt, 1, &p.out.join("prepare-plan.json"))
}

/// Whether one registered aggregate revision still describes the present inputs.
/// A non-publication obligation (the mathematics root) binds only its preparation.
fn publication_current(
    conn: &Connection,
    layer: &str,
    inputs: &Value,
    root_hash: &str,
) -> Result<bool> {
    if inputs.get("publication_contract").is_some() {
        Ok(
            canon::hash_value(&publication_manifest(
                conn,
                layer,
                field(inputs, "logical_key")?,
                &inputs["scope"],
            )?) == canon::hash_value(inputs),
        )
    } else {
        Ok(canon::hash_value(inputs) == root_hash)
    }
}

/// The latest registered revision of one layer's book-level obligation.
pub(crate) fn required_publication(conn: &Connection, layer: &str) -> Result<Option<(String, String)>> {
    Ok(conn
        .query_row(
            "SELECT id,inputs_json FROM units WHERE layer=?1 AND logical_key='book' ORDER BY revision DESC LIMIT 1",
            [layer],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

/// Every aggregate publication of this run: the latest required revision of each
/// logical key and the earlier revisions it superseded, so a report can never
/// present a stale publication as current (R158).
fn publications(conn: &Connection, root_hash: &str) -> Result<Vec<Value>> {
    let list = PUBLICATION
        .iter()
        .map(|layer| format!("'{layer}'"))
        .collect::<Vec<_>>()
        .join(",");
    let mut statement = conn.prepare(&format!(
        "SELECT layer,logical_key,id,revision,state,inputs_json,fault_json FROM units WHERE layer IN ({list}) ORDER BY layer,logical_key,revision"
    ))?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut ordered: Vec<(String, String, Vec<Value>)> = Vec::new();
    for (layer, key, id, revision, state, inputs, fault) in rows {
        let record = json!({
            "unit":id,"revision":revision,"state":state,
            "fault":parsed_fault(fault.as_deref())?,
            "inputs":canon::parse_json(&inputs)?
        });
        match ordered.last_mut() {
            Some((seen_layer, seen_key, revisions))
                if *seen_layer == layer && *seen_key == key =>
            {
                revisions.push(record);
            }
            _ => ordered.push((layer, key, vec![record])),
        }
    }
    let mut result = Vec::new();
    for (layer, key, revisions) in ordered {
        let latest = revisions.last().unwrap();
        let current = publication_current(conn, &layer, &latest["inputs"], root_hash)?;
        let superseded = revisions[..revisions.len() - 1]
            .iter()
            .map(|revision| {
                json!({"unit":revision["unit"],"revision":revision["revision"],"state":revision["state"],"fault":revision["fault"]})
            })
            .collect::<Vec<_>>();
        result.push(json!({
            "layer":layer,
            "key":key,
            "latest":latest["unit"],
            "revision":latest["revision"],
            "state":latest["state"],
            "current":current,
            "superseded":superseded
        }));
    }
    Ok(result)
}

/// The checked dispositions of work that was deliberately not done. A no-op is named,
/// never counted as success without its evidence (R333).
const NOOP_DISPOSITIONS: &[&str] = &[
    "no_cards",
    "no_work",
    "no_link",
    "no_terms",
    "no_build",
    "no_change",
    "not_applicable",
    "disabled",
];

/// What a non-terminal unit or a reported fault costs the run, in the words of R335.
fn gap_kind(layer: &str) -> &'static str {
    match layer {
        "prepare" => "preparation",
        "terms" => "terms",
        "digest" => "group_gap",
        "cards" | "cards-book" => "quality",
        "models" => "model_build",
        "explain" => "explanation",
        "events" | "findings" => "empirical_layer",
        "link" | "link-check" => "link_fault",
        "notes" => "note",
        "organize" => "organize",
        "skeleton" => "skeleton_gap",
        "hubs" => "hub_fault",
        "figures" => "figure",
        "bridge" => "bridge",
        "audit" => "audit",
        "cleanup" => "cleanup",
        "theory" | "math-index" | "math-docs" | "math-skeleton" | "notation" | "boundary"
        | "proof" | "math-check" => "math",
        _ => "unit",
    }
}

fn is_noop(result: &Value) -> bool {
    result["noop"] == true
        || result["disposition"]
            .as_str()
            .is_some_and(|value| NOOP_DISPOSITIONS.contains(&value))
}

/// Refusals the checked result itself records (a note it declined, a Bridge proposal, a
/// set of coverage entries): a deliberate refusal is a disposition, never outstanding work.
fn refused_of(result: &Value) -> u64 {
    let mut count = 0u64;
    for key in ["coverage", "outcomes", "dispositions", "notes"] {
        if let Some(list) = result[key].as_array() {
            count += list
                .iter()
                .filter(|item| {
                    item["outcome"] == json!("refused") || item["disposition"] == json!("refused")
                })
                .count() as u64;
        }
    }
    count
}

/// R333: declared obligations and the state/disposition counts of each layer. `declared`
/// is one per logical key (a superseded revision is history, not declared work).
fn counts_of(units: &[Value]) -> (Value, Value) {
    let mut per_layer: std::collections::BTreeMap<String, Value> =
        std::collections::BTreeMap::new();
    let mut total = json!({
        "declared":0,"done":0,"noop":0,"refused":0,"failed":0,"blocked":0,
        "registered":0,"ready":0,"claimed":0,"retry":0,"superseded":0,"open":0,"total":0
    });
    let mut declared: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for unit in units {
        let layer = unit["layer"].as_str().unwrap_or_default();
        let entry = per_layer.entry(layer.to_string()).or_insert_with(|| {
            json!({
                "declared":0,"done":0,"noop":0,"refused":0,"failed":0,"blocked":0,
                "registered":0,"ready":0,"claimed":0,"retry":0,"superseded":0,"open":0,"total":0
            })
        });
        let bump = |value: &mut Value, key: &str| {
            value[key] = json!(value[key].as_u64().unwrap_or(0) + 1);
        };
        bump(entry, "total");
        bump(&mut total, "total");
        let state = unit["state"].as_str().unwrap_or_default();
        let superseded = is_superseded(unit);
        if !superseded {
            let key = format!("{layer}\u{1}{}", unit["logical_key"].as_str().unwrap_or_default());
            if declared.insert(key) {
                bump(entry, "declared");
                bump(&mut total, "declared");
            }
        }
        let bucket = if superseded {
            "superseded"
        } else {
            match state {
                "done" | "failed" | "blocked" | "registered" | "ready" | "claimed" | "retry" => state,
                _ => "open",
            }
        };
        bump(entry, bucket);
        bump(&mut total, bucket);
        if state == "done" && !superseded {
            if is_noop(&unit["result"]) {
                bump(entry, "noop");
                bump(&mut total, "noop");
            }
            let refused = refused_of(&unit["result"]);
            if refused > 0 {
                entry["refused"] = json!(entry["refused"].as_u64().unwrap_or(0) + refused);
                total["refused"] = json!(total["refused"].as_u64().unwrap_or(0) + refused);
            }
        }
    }
    total["open"] = json!(
        total["registered"].as_u64().unwrap_or(0)
            + total["ready"].as_u64().unwrap_or(0)
            + total["claimed"].as_u64().unwrap_or(0)
            + total["retry"].as_u64().unwrap_or(0)
    );
    (total, Value::Object(per_layer.into_iter().collect()))
}

/// R335: every gap with its exact unit, address or file and reason. Nothing here is an
/// agent's summary: each entry names the recorded evidence that produced it.
fn gaps_of(units: &[Value], coverage_faults: &[Value], placeholders: &Value) -> Vec<Value> {
    let mut gaps = Vec::new();
    for unit in units {
        if is_superseded(unit) {
            continue;
        }
        let layer = unit["layer"].as_str().unwrap_or_default();
        let state = unit["state"].as_str().unwrap_or_default();
        let fault = &unit["fault"];
        if state != "done" {
            let reason = fault["cause"]
                .as_str()
                .or_else(|| fault["code"].as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("the unit is {state} and has not settled"));
            gaps.push(json!({
                "kind": gap_kind(layer), "layer": layer, "unit": unit["id"],
                "state": state, "address": Value::Null,
                "code": fault["code"], "class": unit["fault_class"],
                "reason": reason
            }));
        }
        // A registered external run that never reached a terminal state still owns its unit.
        for job in unit["progress"]["jobs"].as_array().cloned().unwrap_or_default() {
            if job["state"] == json!("terminal") {
                continue;
            }
            gaps.push(json!({
                "kind": "pending_job", "layer": layer, "unit": unit["id"],
                "address": job["id"], "job_kind": job["kind"], "state": job["state"],
                "started_at": job["started_at"], "cap_seconds": job["cap_seconds"],
                "reason": "the external run is not terminal, so its work cannot settle"
            }));
        }
    }
    for fault in coverage_faults {
        gaps.push(json!({
            "kind": "coverage", "unit": fault["unit"], "address": Value::Null,
            "code": fault["code"], "reason": fault["code"].as_str().unwrap_or("recorded coverage fault")
        }));
    }
    // A publication names the exact work it could not resolve, and the exact coverage it
    // checked as a gap, an unmatched figure or an unresolved entry.
    for unit in units {
        if unit["state"] != json!("done") || is_superseded(unit) {
            continue;
        }
        let layer = unit["layer"].as_str().unwrap_or_default();
        let result = &unit["result"];
        if result["disposition"] == json!("partial") {
            for entry in result["unresolved"].as_array().cloned().unwrap_or_default() {
                gaps.push(json!({
                    "kind": "publication_unresolved", "layer": layer, "unit": unit["id"],
                    "address": entry["unit"].as_str().or_else(|| entry["ref"].as_str()),
                    "code": "unresolved", "reason": "this publication input has no resolved disposition"
                }));
            }
        }
        for entry in result["coverage"].as_array().cloned().unwrap_or_default() {
            let outcome = entry["outcome"].as_str().unwrap_or_default();
            if !["gap", "unresolved", "unmatched", "open"].contains(&outcome) {
                continue;
            }
            gaps.push(json!({
                "kind": gap_kind(layer), "layer": layer, "unit": unit["id"],
                "address": entry["ref"], "code": outcome,
                "reason": entry["reason"].as_str().filter(|text| !text.is_empty())
                    .unwrap_or("the checked coverage records no reason")
            }));
        }
    }
    // R336: a placeholder is current verified work, so its count is a gap only when the read
    // succeeded; an unknown count is reported by the completeness and placeholder sections.
    for model in placeholders["models"].as_array().cloned().unwrap_or_default() {
        for address in model["placeholders"].as_array().cloned().unwrap_or_default() {
            gaps.push(json!({
                "kind": "placeholder", "layer": "models", "unit": Value::Null,
                "address": address, "code": "still_written",
                "reason": "this part still holds 待写 in the checked model"
            }));
        }
    }
    // A cleanup unit settles its own resources; its remaining issues are named here.
    for unit in units {
        if unit["layer"] != json!("cleanup") || unit["state"] != json!("done") {
            continue;
        }
        for issue in unit["result"]["partial_reasons"].as_array().cloned().unwrap_or_default() {
            gaps.push(json!({
                "kind": "cleanup", "layer": "cleanup", "unit": unit["id"],
                "address": issue["resource"].as_str().or_else(|| issue["conversation"].as_str()),
                "code": issue["code"], "class": issue["class"], "reason": issue["cause"]
            }));
        }
    }
    gaps
}

/// R337: stage elapsed, queue wait, tool time, external-run time and the critical path.
/// A stage's elapsed time is wall clock, and parallel stages overlap, so they are never
/// added into a claimed total.
fn timing_of(conn: &Connection, units: &[Value]) -> Result<Value> {
    let mut claims: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    let mut terminal: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    {
        let mut statement = conn.prepare(
            "SELECT unit_id,event,utc FROM events WHERE unit_id IS NOT NULL ORDER BY seq",
        )?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (unit, event, utc) = row?;
            if event == "claimed" {
                claims.entry(unit).or_default().push(utc);
            } else if matches!(event.as_str(), "completed" | "failed") {
                terminal.insert(unit, utc);
            }
        }
    }
    let mut per_layer: std::collections::BTreeMap<String, (f64, f64, f64, f64, usize, Option<i64>, Option<i64>)> =
        std::collections::BTreeMap::new();
    let mut wall_first: Option<i64> = None;
    let mut wall_last: Option<i64> = None;
    let mut queue_total = 0.0;
    let mut tool_total = 0.0;
    let mut external_total = 0.0;
    let mut spans: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    let mut layers: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for unit in units {
        let id = unit["id"].as_str().unwrap_or_default().to_string();
        let layer = unit["layer"].as_str().unwrap_or_default().to_string();
        layers.insert(id.clone(), layer.clone());
        let created = unit["created_at"].as_str().and_then(report::epoch_millis);
        let finished = unit["completed_at"]
            .as_str()
            .and_then(report::epoch_millis)
            .or_else(|| terminal.get(&id).and_then(|utc| report::epoch_millis(utc)));
        // Queue wait: from registration to the first claim of this unit.
        let first_claim = claims
            .get(&id)
            .and_then(|list| list.first())
            .and_then(|utc| report::epoch_millis(utc));
        let queue = match (created, first_claim) {
            (Some(created), Some(claim)) => ((claim - created).max(0) as f64) / 1_000.0,
            _ => 0.0,
        };
        // Tool time: each claimed attempt to its recorded terminal event.
        let mut tool = 0.0;
        if let Some(list) = claims.get(&id) {
            for claim in list {
                if let Some(end) = terminal.get(&id) {
                    tool += report::span_seconds(claim, end).unwrap_or(0.0);
                }
            }
        }
        let span = match (created, finished) {
            (Some(created), Some(finished)) => ((finished - created).max(0) as f64) / 1_000.0,
            _ => 0.0,
        };
        spans.insert(id.clone(), span);
        // External-run time: each registered provider run from its start to its observation.
        let mut external = 0.0;
        for job in unit["progress"]["jobs"].as_array().cloned().unwrap_or_default() {
            let started = job["started_at"].as_i64();
            let observed = job["observed_at"]
                .as_str()
                .and_then(report::epoch_millis)
                .map(|millis| millis as f64 / 1_000.0);
            if let (Some(started), Some(observed)) = (started, observed) {
                external += (observed - started as f64).max(0.0);
            }
        }
        queue_total += queue;
        tool_total += tool;
        external_total += external;
        let entry = per_layer.entry(layer).or_insert((0.0, 0.0, 0.0, 0.0, 0, None, None));
        entry.0 += queue;
        entry.1 += tool;
        entry.2 += external;
        entry.4 += 1;
        if let Some(created) = created {
            entry.5 = Some(entry.5.map_or(created, |value| value.min(created)));
            wall_first = Some(wall_first.map_or(created, |value| value.min(created)));
        }
        if let Some(finished) = finished {
            entry.6 = Some(entry.6.map_or(finished, |value| value.max(finished)));
            wall_last = Some(wall_last.map_or(finished, |value| value.max(finished)));
        }
    }
    // Critical path: the longest chain through the recorded dependency graph. A path is a
    // sequence, so adding its spans is exact; unrelated parallel work is not folded in.
    let edges: Vec<(String, String)> = {
        let mut statement = conn.prepare("SELECT unit_id,needs_id FROM deps ORDER BY unit_id")?;
        statement
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let (critical, path) = critical_path(&spans, &edges);
    let mut stages = Vec::new();
    for (layer, (queue, tool, external, elapsed, count, first, last)) in per_layer {
        let wall = match (first, last) {
            (Some(first), Some(last)) => ((last - first).max(0) as f64) / 1_000.0,
            _ => elapsed,
        };
        stages.push(json!({
            "layer":layer, "units":count,
            "elapsed_seconds":report::number_json(wall),
            "queue_wait_seconds":report::number_json(queue),
            "tool_seconds":report::number_json(tool),
            "external_run_seconds":report::number_json(external)
        }));
    }
    let wall = match (wall_first, wall_last) {
        (Some(first), Some(last)) => ((last - first).max(0) as f64) / 1_000.0,
        _ => 0.0,
    };
    Ok(json!({
        "wall_seconds": report::number_json(wall),
        "queue_wait_seconds": report::number_json(queue_total),
        "tool_seconds": report::number_json(tool_total),
        "external_run_seconds": report::number_json(external_total),
        "stages": stages,
        "critical_path": {"seconds": report::number_json(critical), "units": path},
        "parallel_stages_not_summed": true
    }))
}

/// The longest dependency chain by recorded unit span, with its units in order.
fn critical_path(spans: &std::collections::BTreeMap<String, f64>, edges: &[(String, String)]) -> (f64, Vec<String>) {
    let mut children: std::collections::BTreeMap<&str, Vec<&str>> = std::collections::BTreeMap::new();
    let mut indegree: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for id in spans.keys() {
        indegree.insert(id.as_str(), 0);
    }
    for (unit, needs) in edges {
        if !spans.contains_key(unit) || !spans.contains_key(needs) {
            continue;
        }
        children.entry(needs.as_str()).or_default().push(unit.as_str());
        *indegree.entry(unit.as_str()).or_insert(0) += 1;
    }
    let mut best: std::collections::BTreeMap<&str, (f64, Option<&str>)> = std::collections::BTreeMap::new();
    for id in spans.keys() {
        best.insert(id.as_str(), (spans[id], None));
    }
    let mut ready: std::collections::BTreeSet<&str> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| *id)
        .collect();
    while let Some(id) = ready.iter().next().copied() {
        ready.remove(id);
        let (value, _) = *best.get(id).unwrap();
        for child in children.get(id).cloned().unwrap_or_default() {
            let candidate = value + spans[child];
            if candidate > best[child].0 {
                best.insert(child, (candidate, Some(id)));
            }
            if let Some(degree) = indegree.get_mut(child) {
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    ready.insert(child);
                }
            }
        }
    }
    let (end, value) = best
        .iter()
        .max_by(|a, b| a.1.0.partial_cmp(&b.1.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(id, (value, _))| (*id, *value))
        .unwrap_or(("", 0.0));
    let mut path = Vec::new();
    let mut at = Some(end);
    while let Some(id) = at {
        path.push(id.to_string());
        at = best.get(id).and_then(|(_, parent)| *parent);
    }
    path.reverse();
    (value, path)
}

/// R340: the supported resume invocation, printed only while the one resume is unused.
/// After it is consumed the report lists the manual decisions instead of suggesting another.
fn resume_facts(preparation: &Value, out: &Path, unresolved: &[Value], gaps: &[Value], resume_count: i64) -> Value {
    if unresolved.is_empty() {
        return json!({"available":false,"reason":"no unresolved work remains","manual":[]});
    }
    if resume_count > 0 {
        let manual: Vec<Value> = gaps
            .iter()
            .filter(|gap| gap["kind"] != json!("cleanup"))
            .take(20)
            .map(|gap| {
                json!(format!(
                    "{} {}: {}",
                    gap["kind"].as_str().unwrap_or("gap"),
                    gap["unit"].as_str().or_else(|| gap["address"].as_str()).unwrap_or("run"),
                    gap["reason"].as_str().unwrap_or("no recorded reason")
                ))
            })
            .collect();
        return json!({
            "available":false,
            "reason":"the one supported resume has already been used; the remaining work needs a manual decision",
            "manual": manual
        });
    }
    let variant = preparation["variant"].as_str().unwrap_or("");
    let scripts = crate::model::Scripts::find();
    let repo = std::path::Path::new(&scripts.dir)
        .parent()
        .and_then(std::path::Path::parent)
        .map(|path| path.to_path_buf())
        .unwrap_or_default();
    let script = repo
        .join(".claude")
        .join("workflows")
        .join(format!("zt-{variant}-digest.js"));
    let out = report::plain_path(&out.display().to_string());
    let invocation = format!(
        "Workflow({{scriptPath: \"{}\", args: {{mode: \"resume\", out: \"{}\"}}}})",
        script.display().to_string().replace('\\', "\\\\"),
        out.replace('\\', "\\\\")
    );
    if !script.is_file() {
        return json!({
            "available":false,
            "reason":format!("the installed {} workflow was not found at {}", variant, script.display()),
            "manual":[]
        });
    }
    json!({"available":true,"script":script,"invocation":invocation,"manual":[]})
}

/// The immutable Bridge report of this run, with its open proposals labelled as review
/// decisions rather than missing digest writes (R339).
fn bridge_facts(units: &[Value]) -> Result<Value> {
    let Some(unit) = units
        .iter()
        .filter(|unit| unit["layer"] == json!("bridge") && unit["state"] == json!("done"))
        .max_by_key(|unit| unit["revision"].as_i64().unwrap_or(0))
    else {
        return Ok(Value::Null);
    };
    let result = &unit["result"];
    let coverage = result["coverage"].as_array().cloned().unwrap_or_default();
    let count = |name: &str| coverage.iter().filter(|entry| entry["outcome"] == json!(name)).count();
    let report_id = result["artifacts"]["report"]["path"]
        .as_str()
        .and_then(|path| util::read_json(path).ok())
        .and_then(|report| report["report_id"].as_str().map(str::to_string));
    Ok(json!({
        "report_id": report_id,
        "unit": unit["id"],
        "revision": unit["revision"],
        "open": count("open"),
        "accepted": count("accepted"),
        "refused": count("refused"),
        "state": "awaiting_optional_review"
    }))
}

/// R331/R332: the run report, from the ledger and the verified reads, with its two files.
///
/// Rendering is read-only with respect to Cards: the two files under the prepared output
/// directory are the only writes. The same facts give the same bytes, so a second render
/// changes nothing (R331).
fn run_report(p: &Prepared, verified: Value) -> Result<Value> {
    let base = status(&p.out, "", Some(verified.clone()))?;
    let manifest = util::read_json(p.out.join("manifest.json")).unwrap_or(Value::Null);
    let contract = &p.contract;
    let observed = zt_check::mcp::observed_service();
    let verified_read = verified["status"] == json!("verified");
    let revision = if verified_read {
        verified["revision"].clone()
    } else {
        observed
            .as_ref()
            .and_then(|facts| facts.revision)
            .map(|revision| json!(revision))
            .unwrap_or(Value::Null)
    };
    let units = base["units"].as_array().cloned().unwrap_or_default();
    let mut publications = base["publications"].as_array().cloned().unwrap_or_default();
    for publication in &mut publications {
        let latest = publication["latest"].as_str().unwrap_or_default();
        if let Some(unit) = units.iter().find(|unit| unit["id"] == json!(latest)) {
            publication["unit"] = unit["id"].clone();
            publication["disposition"] = unit["result"]["disposition"].clone();
        }
    }
    let identity = json!({
        "run_id": base["run_id"],
        "invocation": base["invocation"],
        "resume_count": base["resume_count"],
        "citation_key": contract["citation_key"],
        "variant": contract["variant"],
        "book_title": manifest["book_title"],
        "source": {
            "path": contract["source"]["path"],
            "sha256": contract["source"]["sha256"],
            "file_id": contract["source"]["file_id"]
        },
        "range": {
            "groups": contract["groups"].as_array().map(Vec::len),
            "group_manifest_hash": contract["group_manifest_hash"],
            "book_sha256": contract["artifacts"]["book"]["sha256"]
        },
        "archive": {
            "id": contract["archive_identity"]["id"],
            "path": report::plain_path(contract["archive_identity"]["path"].as_str().unwrap_or_default()),
            "revision": revision,
            "verified": verified_read,
            "service": {
                "up": observed.as_ref().map(|facts| facts.up),
                "pid": observed.as_ref().and_then(|facts| facts.pid.clone()),
                "last_exit_abnormal": observed.as_ref().map(|facts| facts.last_exit_abnormal)
            }
        },
        "workflow": {
            "base_version": contract["contracts"]["base_version"],
            "variant_version": contract["contracts"]["variant_version"],
            "artifact_schema": contract["contracts"]["artifact_schema"]
        },
        "binaries": {
            "zt_digest": env!("CARGO_PKG_VERSION"),
            "zt_check": zt_check::VERSION,
            "zt": observed.as_ref().and_then(|facts| facts.version.clone())
        },
        "settings": base["settings"],
        "enabled_layers": base["enabled_layers"],
        "excluded_layers": base["excluded_layers"],
        "source_faults": manifest["source_faults"]
    });
    Ok(json!({
        "schema_version": 1,
        "kind": "run_report",
        "identity": identity,
        "counts": base["counts"],
        "completeness": base["completeness"],
        "gaps": base["gaps"],
        "placeholders": base["placeholders"],
        "timing": base["timing"],
        "usage": base["usage"],
        "resume": base["resume"],
        "bridge": base["bridge"],
        "publications": publications,
        "cleanup": base["cleanup"]
    }))
}

/// `ledger status -Summary`: the run's identity, settings and counts, with the units replaced by
/// their number per layer and state. A workflow reads it through one relayed output line; the
/// full status carries every unit's inputs, progress and result.
fn status_summary(full: &Value) -> Value {
    let mut layers = serde_json::Map::new();
    for unit in full["units"].as_array().into_iter().flatten() {
        let layer = unit["layer"].as_str().unwrap_or("").to_string();
        let state = unit["state"].as_str().unwrap_or("").to_string();
        let entry = layers.entry(layer).or_insert_with(|| json!({"total":0}));
        entry[&state] = json!(entry[&state].as_i64().unwrap_or(0) + 1);
        entry["total"] = json!(entry["total"].as_i64().unwrap_or(0) + 1);
    }
    let mut summary = serde_json::Map::new();
    for key in ["schema_version", "run_id", "invocation", "resume_count", "citation_key", "variant", "settings", "counts", "settled", "complete", "sealed"] {
        if let Some(value) = full.get(key) {
            summary.insert(key.to_string(), value.clone());
        }
    }
    summary.insert("layers".to_string(), Value::Object(layers));
    Value::Object(summary)
}

fn status(out: &Path, layer: &str, placeholders: Option<Value>) -> Result<Value> {
    let conn = connect(&out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_schema(&tx)?;
    if meta(&tx, "preparation_hash")? != canon::hash_value(&meta(&tx, "preparation")?) {
        bail!("ledger preparation hash mismatch");
    }
    let mut statement=tx.prepare("SELECT id,layer,logical_key,revision,state,attempts,owner,claim_token,input_hash,inputs_json,fault_json,progress_json,result_json,lease_until,retry_after,created_at,completed_at,fault_class FROM units WHERE (?1='' OR layer=?1) ORDER BY id")?;
    let units = statement
        .query_map([layer], |r| {
            Ok(json!({
                "id":r.get::<_,String>(0)?,
                "layer":r.get::<_,String>(1)?,
                "logical_key":r.get::<_,String>(2)?,
                "revision":r.get::<_,i64>(3)?,
                "state":r.get::<_,String>(4)?,
                "attempts":r.get::<_,i64>(5)?,
                "owner":r.get::<_,Option<String>>(6)?,
                "token":r.get::<_,i64>(7)?,
                "input_hash":r.get::<_,String>(8)?,
                "inputs_json":r.get::<_,String>(9)?,
                "fault_json":r.get::<_,Option<String>>(10)?,
                "progress_json":r.get::<_,String>(11)?,
                "result_json":r.get::<_,Option<String>>(12)?,
                "lease_until":r.get::<_,Option<i64>>(13)?,
                "retry_after":r.get::<_,Option<i64>>(14)?,
                "created_at":r.get::<_,String>(15)?,
                "completed_at":r.get::<_,Option<String>>(16)?,
                "fault_class":r.get::<_,Option<String>>(17)?
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut units = units;
    for unit in &mut units {
        for key in ["inputs", "fault", "progress", "result"] {
            let raw = unit
                .as_object_mut()
                .unwrap()
                .remove(&format!("{key}_json"))
                .unwrap();
            unit[key] = if let Some(raw) = raw.as_str() {
                serde_json::from_str(raw)?
            } else {
                Value::Null
            };
        }
    }
    let target = meta(&tx, "target")?;
    if target != self::target(&meta(&tx, "preparation")?)? {
        bail!("ledger declared target mismatch");
    }
    let excluded = target["excluded"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l == layer);
    let layers = target["enabled"]
        .as_array()
        .context("missing declared target")?;
    let preparation = meta(&tx, "preparation")?;
    let required: Vec<&'static str> = required_layers(&tx)?;
    let mut sealed = true;
    for name in layers
        .iter()
        .filter_map(Value::as_str)
        .filter(|name| layer.is_empty() || *name == layer)
    {
        sealed &= meta(&tx, &format!("barrier/{name}"))?["sealed"] == true;
        // An absent required publication is outstanding work, not an empty queue.
        if required.iter().any(|r| *r == name) {
            sealed &= required_publication(&tx, name)?.is_some();
        }
    }
    let (mut counts, per_layer) = counts_of(&units);
    let mut unresolved = Vec::new();
    for unit in &units {
        // A superseded revision is terminal history: it is neither a failure of
        // this run nor outstanding work, and it is never offered to a resume.
        if unit["state"] != json!("done") && !is_superseded(unit) {
            unresolved.push(unit["id"].clone());
        }
    }
    counts["per_layer"] = per_layer;
    counts["open"] = json!(
        counts["registered"].as_u64().unwrap_or(0)
            + counts["ready"].as_u64().unwrap_or(0)
            + counts["claimed"].as_u64().unwrap_or(0)
            + counts["retry"].as_u64().unwrap_or(0)
    );
    counts["invocations"] = {
        let mut statement = tx.prepare(
            "SELECT invocation,SUM(event='claimed'),SUM(event='completed'),SUM(event='failed'),MIN(utc),MAX(utc) FROM events GROUP BY invocation ORDER BY invocation",
        )?;
        let rows = statement.query_map([], |r| {
            Ok(json!({
                "invocation":r.get::<_,i64>(0)?,
                "claimed":r.get::<_,Option<i64>>(1)?,
                "completed":r.get::<_,Option<i64>>(2)?,
                "failed":r.get::<_,Option<i64>>(3)?,
                "first_event":r.get::<_,Option<String>>(4)?,
                "last_event":r.get::<_,Option<String>>(5)?
            }))
        })?;
        Value::Array(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    };
    let mut coverage_faults = Vec::new();
    let root_hash = canon::hash_value(&json!({"preparation_hash":meta(&tx,"preparation_hash")?}));
    for layer in required {
        // The required publication is the latest revision of `<layer>/book`. It must
        // exist, stay bound to this preparation, and still describe the present
        // inputs: a stale revision is reported instead of being called current.
        let latest = required_publication(&tx, layer)?;
        let current = match &latest {
            None => false,
            Some((_, raw)) => publication_current(&tx, layer, &canon::parse_json(raw)?, &root_hash)?,
        };
        if !current {
            coverage_faults.push(json!({"code":"required_unit_missing_or_changed","unit":latest.map(|(id,_)|id).unwrap_or(format!("{layer}/book@1"))}));
        }
    }
    for group in preparation["groups"]
        .as_array()
        .context("missing prepared group target")?
    {
        let id = format!(
            "digest/g{:03}@1",
            group["n"]
                .as_u64()
                .context("invalid prepared group number")?
        );
        let hash: Option<String> = tx
            .query_row("SELECT input_hash FROM units WHERE id=?1", [&id], |r| {
                r.get(0)
            })
            .optional()?;
        if hash.as_deref() != Some(canon::hash_value(group).as_str()) {
            coverage_faults.push(json!({"code":"prepared_group_missing_or_changed","unit":id}));
        }
    }
    coverage_faults.extend(emission_faults(&tx)?);
    let settled = sealed && counts["open"] == 0;
    // Cleanup reports its own half of the run beside the content half (R330): content success is
    // never erased by a cleanup failure, and a cleanup failure still makes the whole run partial.
    let cleanup_facts = {
        let mut statement = tx
            .prepare("SELECT id,state,result_json FROM units WHERE layer='cleanup' ORDER BY id")?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        let mut cleanup_units = Vec::new();
        let mut open = Vec::new();
        let mut cleanup_complete = true;
        for (id, state, result) in rows {
            let result: Value = match result {
                Some(raw) => serde_json::from_str(&raw)?,
                None => Value::Null,
            };
            if state != "done" {
                open.push(json!(id));
            }
            if state != "done" || result["cleanup_complete"] != true {
                cleanup_complete = false;
            }
            cleanup_units.push(json!({
                "id":id,
                "state":state,
                "status":result["status"],
                "content_complete":result["content_complete"],
                "cleanup_complete":result["cleanup_complete"],
                "resources":result["resources"].as_array().map(Vec::len),
                "issues":result["partial_reasons"].as_array().map(Vec::len)
            }));
        }
        let content = content_facts(&tx)?;
        let content_complete = content["complete"] == true;
        json!({
            "units":cleanup_units,
            "open":open,
            "content_complete":content_complete,
            "cleanup_complete":cleanup_complete,
            "run_complete":content_complete && cleanup_complete,
            "unresolved":content["unresolved"],
            "reconciliation":[]
        })
    };
    // R334: the four halves stay separate. An unverified placeholder count is unknown, so it
    // can never certify quality (R336).
    let placeholders = match placeholders {
        Some(value) => value,
        None => json!({
            "status":"not_read","count":Value::Null,"models":[],"revision":Value::Null,
            "reason":"ledger status reads no archive Card; run `zt-digest ledger report` for the verified count"
        }),
    };
    let gaps = gaps_of(&units, &coverage_faults, &placeholders);
    let content_gaps = gaps
        .iter()
        .filter(|gap| gap["kind"] != json!("cleanup"))
        .count();
    // The layer's own completion keeps the pre-existing meaning the gates and their callers use: the
    // enumeration settled, nothing failed or blocked, and every required publication current.
    let content_complete = settled
        && !excluded
        && counts["failed"] == json!(0)
        && counts["blocked"] == json!(0)
        && coverage_faults.is_empty();
    // Quality needs the verified current placeholder count: an unread count cannot certify it.
    let quality_reason = if placeholders["status"] != json!("verified") {
        Value::String("placeholders_not_verified".into())
    } else if placeholders["count"] != json!(0) {
        Value::String("placeholders_present".into())
    } else if !content_complete {
        Value::String("content_incomplete".into())
    } else if content_gaps > 0 {
        Value::String("content_gaps".into())
    } else {
        Value::Null
    };
    let quality_complete = quality_reason.is_null();
    let cleanup_complete = cleanup_facts["cleanup_complete"] == true;
    let fully_complete = content_complete
        && quality_complete
        && cleanup_complete;
    let completeness = json!({
        "settled":settled,
        "content_complete":content_complete,
        "quality_complete":quality_complete,
        "quality_reason":quality_reason,
        "cleanup_complete":cleanup_complete,
        "placeholders_verified":placeholders["status"] == json!("verified"),
        "complete":fully_complete
    });
    let usage = report::usage_facts(&recorded_resources(&tx, None)?);
    let resume = resume_facts(&preparation, out, &unresolved, &gaps, meta(&tx, "resume_count")?.as_i64().unwrap_or(0));
    let timing = timing_of(&tx, &units)?;
    let bridge = bridge_facts(&units)?;
    let mut deps_query = tx.prepare("SELECT d.unit_id,d.needs_id,d.condition,u.state FROM deps d JOIN units u ON u.id=d.needs_id JOIN units child ON child.id=d.unit_id WHERE (?1='' OR child.layer=?1) ORDER BY d.unit_id,d.needs_id")?;
    let dependencies=deps_query.query_map([layer],|r|Ok(json!({"unit":r.get::<_,String>(0)?,"needs":r.get::<_,String>(1)?,"condition":r.get::<_,String>(2)?,"state":r.get::<_,String>(3)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let result = json!({"schema_version":1,"run_id":meta(&tx,"run_id")?,"invocation":meta(&tx,"invocation")?,"resume_count":meta(&tx,"resume_count")?,"citation_key":preparation["citation_key"],"variant":preparation["variant"],"archive_identity":preparation["archive_identity"],"source":preparation["source"],"effective_options":preparation["options"],"contracts":preparation["contracts"],"run_options":run_options(field(&preparation,"variant")?,preparation["options"]["no_figures"]==true,preparation["options"].get("run_options"))?,"settings":settings_of(&preparation)?,"archive_verification":"not_requested","layer":layer,"sealed":sealed,"settled":settled,"complete":content_complete,"excluded":excluded,"counts":counts,"completeness":completeness,"gaps":gaps,"placeholders":placeholders,"timing":timing,"usage":usage,"resume":resume,"bridge":bridge,"unresolved":unresolved,"coverage_faults":coverage_faults,"publications":publications(&tx,&root_hash)?,"dependencies":dependencies,"units":units,"excluded_layers":target["excluded"],"enabled_layers":target["enabled"],"cleanup":cleanup_facts});
    drop(deps_query);
    drop(statement);
    tx.commit()?;
    Ok(result)
}

/// R167: which layer can still append units to a dynamic child layer once that
/// producer has run. Only layers whose producer exists are listed here; the other
/// dynamic layers keep their own slice's closure rule rather than a fabricated one.
/// The second field names the closures that must already be sealed, because a
/// producer can only enumerate its rows after its own enumeration closed.
fn enumeration_closure(layer: &str) -> Option<(&'static str, &'static [&'static str])> {
    Some(match layer {
        "cards" | "events" | "findings" | "link" | "models" => ("digest", &[][..]),
        // Explanation work is registered by Model completions, which can only appear while
        // the Digest enumeration is still open, so the Models closure comes first (R301).
        "explain" => ("models", &["models"][..]),
        _ => return None,
    })
}

/// A dynamic child layer is closed only when every producer that can register more of
/// its units is terminal, so an empty claim queue never completes the layer while a
/// producer can still append to it. Closing is monotone and one event names the fact.
///
/// The `cleanup` layer's producers are every other layer: a manager registers a resource
/// while it owns any unit, so no further cleanup unit can appear once every other layer is
/// sealed and every unit outside cleanup is terminal (R323).
fn close_enumerations(tx: &Transaction) -> Result<()> {
    let target = meta(tx, "target")?;
    let layers: Vec<&str> = target["enabled"]
        .as_array()
        .context("missing declared target")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for layer in &layers {
        if *layer == "cleanup" {
            if meta(tx, &format!("barrier/{layer}"))?["sealed"] == true {
                continue;
            }
            // A resource is registered by the owner of a claimed unit, so no further cleanup unit can
            // appear once every other unit is terminal. A layer whose own closure belongs to another
            // slice (notes) stays unsealed without keeping cleanup open for ever.
            let open: i64 = tx.query_row(
                "SELECT count(*) FROM units WHERE layer<>'cleanup' AND state NOT IN ('done','failed','blocked')",
                [],
                |row| row.get(0),
            )?;
            if open > 0 {
                continue;
            }
            seal(tx, layer)?;
            tx.execute(
                "INSERT INTO events(unit_id,invocation,utc,event,facts_json) VALUES(NULL,?1,?2,?3,?4)",
                params![
                    meta(tx, "invocation")?.as_i64().context("invalid invocation number")?,
                    canon::utc_now_o(),
                    "sealed_enumeration",
                    canon::canonical_json(&json!({"layer":layer,"producer":Value::Null,"cause":"every unit that can register a resource is terminal"}))
                ],
            )?;
            continue;
        }
        let Some((producer, after)) = enumeration_closure(layer) else {
            continue;
        };
        let closed = after.iter().try_fold(true, |closed, previous| {
            Ok::<bool, anyhow::Error>(
                closed && meta(tx, &format!("barrier/{previous}"))?["sealed"] == true,
            )
        })?;
        if !closed {
            continue;
        }
        let open: i64 = tx.query_row(
            "SELECT count(*) FROM units WHERE layer=?1 AND state NOT IN ('done','failed','blocked')",
            [producer],
            |row| row.get(0),
        )?;
        if open > 0 || meta(tx, &format!("barrier/{layer}"))?["sealed"] == true {
            continue;
        }
        seal(tx, layer)?;
        tx.execute(
            "INSERT INTO events(unit_id,invocation,utc,event,facts_json) VALUES(NULL,?1,?2,?3,?4)",
            params![
                meta(tx, "invocation")?.as_i64().context("invalid invocation number")?,
                canon::utc_now_o(),
                "sealed_enumeration",
                canon::canonical_json(&json!({"layer":layer,"producer":producer,"cause":"every producer that can append to this layer is terminal"}))
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// The longest claim lifetime a caller may request (`ledger next -Lease`): the frozen run cap
/// tops out at 3600 seconds (`dshRunCap`), and the claim holds a margin over it.
const LEASE_MAX: i64 = 7200;

/// R178: the lease is 120 seconds and a held claim renews it every 30 seconds. The two
/// variables exist only so a public test can observe a renewal inside one command.
fn lease_seconds() -> i64 {
    std::env::var("ZT_LEDGER_LEASE_SECONDS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|n| *n >= 1)
        .unwrap_or(120)
}

fn heartbeat_interval() -> Duration {
    let ms = std::env::var("ZT_LEDGER_HEARTBEAT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|n| *n >= 10)
        .unwrap_or(30_000);
    Duration::from_millis(ms)
}

/// Names the processes whose heartbeat already renews one claim, so repeated
/// `finish_claim` calls in the same command start exactly one renewing thread.
static HEARTBEATS: std::sync::OnceLock<std::sync::Mutex<std::collections::BTreeSet<String>>> =
    std::sync::OnceLock::new();

/// A registered external run that is not terminal still owns the work that produced it,
/// so neither a replacement attempt nor a completion may treat the unit as settled (R184).
fn pending_jobs(unit: &str, progress: &Value) -> Result<()> {
    let running = progress["jobs"]
        .as_array()
        .map(|jobs| {
            jobs.iter()
                .filter(|job| job["state"] != "terminal")
                .filter_map(|job| job["id"].as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !running.is_empty() {
        bail!(
            "jobs_running: {unit} still owns running external runs: {} (request cancellation and observe each terminal state before completion)",
            running.join(", ")
        );
    }
    Ok(())
}

/// One heartbeat write: extend the lease only while this exact token is the current claim.
/// The recorded fact is a timestamp and the process id, never an agent's claim of success.
fn heartbeat_renew(
    out: &Path,
    unit: &str,
    token: i64,
    seq: u64,
    interval: Duration,
) -> Result<()> {
    let mut conn = connect(&out.join("ledger.sqlite3"), true)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let row: Option<(String, i64, Option<i64>, Option<String>)> = tx
        .query_row(
            "SELECT state,claim_token,lease_until,owner FROM units WHERE id=?1",
            [unit],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let (state, current, lease, owner) =
        row.with_context(|| format!("stale_claim: unknown unit {unit}"))?;
    if state != "claimed" || current != token {
        bail!("stale_claim: {unit} has no current claim for token {token}");
    }
    // A renewal never shortens a lease another owner/config already extended.
    let until = (now_seconds() + lease_seconds()).max(lease.unwrap_or(0));
    tx.execute(
        "UPDATE units SET lease_until=?2 WHERE id=?1",
        params![unit, until],
    )?;
    event(
        &tx,
        unit,
        owner.as_deref(),
        Some(token),
        "renewed",
        &json!({"lease_until":until,"heartbeat":{"pid":std::process::id(),"interval_ms":interval.as_millis() as u64,"seq":seq,"utc":canon::utc_now_o()}}),
    )?;
    // R318: the same beat keeps the archive's active-run marker fresh, so the daemon's heartbeat
    // still names a run whose command is busy for minutes (an apply), and stops once the run is
    // gone because no command renews the marker any more.
    let archive = tx
        .query_row("SELECT value FROM meta WHERE key='archive_identity'", [], |r| {
            r.get::<_, String>(0)
        })
        .ok()
        .and_then(|value| serde_json::from_str::<Value>(&value).ok())
        .and_then(|identity| identity["path"].as_str().map(std::path::PathBuf::from));
    tx.commit()?;
    if let Some(archive) = archive {
        crate::zt::renew_active_run(&archive, out, Some(unit));
    }
    Ok(())
}

/// A long command renews itself from a small internal heartbeat while it does bounded
/// work (R178). A lost claim ends the thread; a transient ledger fault waits for the next
/// beat. The process holds the thread for its whole life, so no command has to stop it.
fn start_heartbeat(p: &Prepared, unit: &str, token: i64) {
    let key = format!("{}|{unit}|{token}", p.out.display());
    let registry = HEARTBEATS.get_or_init(|| std::sync::Mutex::new(std::collections::BTreeSet::new()));
    if let Ok(mut keys) = registry.lock()
        && !keys.insert(key)
    {
        return;
    }
    let out = p.out.clone();
    let unit = unit.to_string();
    let interval = heartbeat_interval();
    std::thread::spawn(move || {
        let mut seq = 0u64;
        loop {
            seq += 1;
            if let Err(error) = heartbeat_renew(&out, &unit, token, seq, interval) {
                let text = error.to_string();
                if text.contains("stale_claim") || text.contains("unknown unit") {
                    return;
                }
            }
            std::thread::sleep(interval);
        }
    });
}

fn event(
    tx: &Transaction,
    unit: &str,
    owner: Option<&str>,
    token: Option<i64>,
    name: &str,
    facts: &Value,
) -> Result<()> {
    tx.execute("INSERT INTO events(unit_id,invocation,utc,owner,claim_token,event,facts_json) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![unit,meta(tx,"invocation")?.as_i64().context("invalid invocation number")?,canon::utc_now_o(),owner,token,name,canon::canonical_json(facts)])?;
    Ok(())
}

pub(crate) fn writer(p: &Prepared) -> Result<Connection> {
    // Refuse a changed binding before opening a connection that can change WAL mode.
    verify_binding(&connect(&p.out.join("ledger.sqlite3"), false)?, p)?;
    connect(&p.out.join("ledger.sqlite3"), true)
}

/// Diagnose pending cycles before scheduling. An explicit stack avoids recursion depth limits.
fn detect_cycles(tx: &Transaction) -> Result<()> {
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    let mut statement = tx.prepare(
        "SELECT id FROM units WHERE state IN ('registered','ready','blocked','retry') ORDER BY id",
    )?;
    let ids = statement
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut graph = ids
        .into_iter()
        .map(|id| (id, Vec::<String>::new()))
        .collect::<BTreeMap<_, _>>();
    let mut edges=tx.prepare("SELECT d.unit_id,d.needs_id FROM deps d JOIN units c ON c.id=d.unit_id JOIN units p ON p.id=d.needs_id WHERE c.state IN ('registered','ready','blocked','retry') AND p.state IN ('registered','ready','blocked','retry') ORDER BY d.unit_id,d.needs_id")?;
    for edge in edges.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (from, to) = edge?;
        graph.get_mut(&from).unwrap().push(to);
    }
    let mut seen = BTreeSet::new();
    let mut finish = Vec::new();
    for start in graph.keys() {
        if !seen.insert(start.clone()) {
            continue;
        }
        let mut stack = vec![(start.clone(), 0usize)];
        while let Some((node, index)) = stack.last_mut() {
            let neighbors = &graph[node];
            if *index == neighbors.len() {
                let (done, _) = stack.pop().unwrap();
                finish.push(done);
                continue;
            }
            let next = neighbors[*index].clone();
            *index += 1;
            if seen.insert(next.clone()) {
                stack.push((next, 0));
            }
        }
    }
    // Reverse finishing order finds every cyclic producer, including overlapping cycles.
    let mut reverse = graph
        .keys()
        .map(|id| (id.clone(), Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for (from, neighbors) in &graph {
        for to in neighbors {
            reverse.get_mut(to).unwrap().push(from.clone());
        }
    }
    seen.clear();
    let mut cycles = BTreeMap::<String, Vec<String>>::new();
    while let Some(start) = finish.pop() {
        if !seen.insert(start.clone()) {
            continue;
        }
        let mut component = BTreeSet::from([start.clone()]);
        let mut stack = vec![start.clone()];
        while let Some(node) = stack.pop() {
            for next in &reverse[&node] {
                if seen.insert(next.clone()) {
                    component.insert(next.clone());
                    stack.push(next.clone());
                }
            }
        }
        if component.len() == 1 && !graph[&start].contains(&start) {
            continue;
        }
        for member in &component {
            // Return a real closed path through this member, not a sorted component label.
            let mut parents = BTreeMap::from([(member.clone(), None::<String>)]);
            let mut queue = VecDeque::from([member.clone()]);
            let mut found = None;
            'search: while let Some(node) = queue.pop_front() {
                for next in graph[&node].iter().filter(|n| component.contains(*n)) {
                    if next == member {
                        let mut cycle = vec![node.clone()];
                        let mut cursor = &node;
                        while let Some(parent) = &parents[cursor] {
                            cycle.push(parent.clone());
                            cursor = parent;
                        }
                        cycle.reverse();
                        cycle.push(member.clone());
                        found = Some(cycle);
                        break 'search;
                    }
                    if !parents.contains_key(next) {
                        parents.insert(next.clone(), Some(node.clone()));
                        queue.push_back(next.clone());
                    }
                }
            }
            cycles.insert(
                member.clone(),
                found.context("cyclic dependency component has no closed path")?,
            );
        }
    }
    for (id, cycle) in cycles {
        let fault = json!({"code":"dependency_cycle","cause":format!("dependency cycle: {}",cycle.join(" -> ")),"cycle":cycle});
        tx.execute("UPDATE units SET state='failed',owner=NULL,lease_until=NULL,retry_after=NULL,fault_class='content',fault_json=?2 WHERE id=?1",params![id,canon::canonical_json(&fault)])?;
        event(tx, &id, None, None, "dependency_cycle", &fault)?;
    }
    Ok(())
}

/// Dependency waiting consumes no attempt. Aggregates wait for their explicit enumeration barrier.
fn reconcile(tx: &Transaction, now: i64) -> Result<()> {
    detect_cycles(tx)?;
    close_enumerations(tx)?;
    loop {
        let mut statement=tx.prepare("SELECT id,layer,state,retry_after,fault_class,fault_json FROM units WHERE state IN ('registered','ready','blocked','retry') ORDER BY id")?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        let mut changed = false;
        for (id, layer, state, retry_after, fault_class, previous_fault) in rows {
            let mut deps=tx.prepare_cached("SELECT d.needs_id,d.condition,u.state FROM deps d JOIN units u ON u.id=d.needs_id WHERE d.unit_id=?1 ORDER BY d.needs_id")?;
            let deps = deps
                .query_map([&id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let failed = deps
                .iter()
                .filter(|(_, c, s)| c == "success" && matches!(s.as_str(), "failed" | "blocked"))
                .map(|(id, _, _)| id.clone())
                .collect::<Vec<_>>();
            let fault = if failed.is_empty() {
                Value::Null
            } else {
                json!({"code":"dependency_failed","prerequisites":failed})
            };
            let ready = deps.iter().all(|(_, c, s)| {
                s == "done" || (c == "settled" && matches!(s.as_str(), "failed" | "blocked"))
            });
            let aggregate = matches!(
                layer.as_str(),
                "cards-book"
                    | "link-check"
                    | "organize"
                    | "skeleton"
                    | "figures"
                    | "bridge"
                    | "audit"
                    | "theory"
                    | "math-check"
            );
            let enumerated = !aggregate || meta(tx, &format!("barrier/{layer}"))?["sealed"] == true;
            let next = if !failed.is_empty() {
                "blocked"
            } else if state == "retry" && retry_after.is_some_and(|at| at > now) {
                "retry"
            } else if ready && enumerated {
                "ready"
            } else {
                "registered"
            };
            if state == next
                && (next != "blocked"
                    || previous_fault.as_deref() == Some(canon::canonical_json(&fault).as_str()))
            {
                continue;
            }
            tx.execute("UPDATE units SET state=?2,fault_class=CASE WHEN ?2='blocked' THEN 'dependency' WHEN fault_class='dependency' THEN NULL ELSE fault_class END,fault_json=CASE WHEN ?2='blocked' THEN ?3 WHEN fault_class='dependency' THEN NULL ELSE fault_json END WHERE id=?1",params![id,next,if fault.is_null(){None}else{Some(canon::canonical_json(&fault))}])?;
            event(
                tx,
                &id,
                None,
                None,
                "readiness",
                &json!({"from":state,"to":next,"fault":fault,"previous_fault_class":fault_class}),
            )?;
            changed = true;
        }
        if !changed {
            break;
        }
    }
    Ok(())
}

/// Claim ready units of one layer in a bounded block.
///
/// `-Wait <seconds>` is the tool-side readiness wait (R244/R270): when nothing is
/// ready yet, zt-digest keeps the call open until a unit becomes ready, the layer
/// settles, or the deadline passes. Timekeeping lives here, never in a workflow
/// module. The wait never spends an attempt and never changes a unit state except
/// by the ordinary bounded claim.
pub(crate) fn next(p: &Prepared, layer: &str, take: i64, owner: &str, wait: i64, lease: i64) -> Result<Value> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait.max(0) as u64);
    loop {
        let claimed = next_once(p, layer, take, owner, lease)?;
        let empty = claimed["units"].as_array().is_none_or(|units| units.is_empty());
        if !empty || wait <= 0 || std::time::Instant::now() >= deadline {
            return Ok(claimed);
        }
        if claimed["settled"] == true {
            return Ok(claimed);
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

/// One claim's lease: the longer of the base lease and the lifetime its caller asks for.
///
/// A caller that starts a provider run asks for that run's whole frozen cap (`-Lease`), so a healthy
/// job can never be re-readied under itself (#92). The base lease and its heartbeat stay the floor.
fn claim_lease(base: i64, requested: i64) -> i64 {
    base.max(requested)
}

/// The lease a start step writes: the base lease from now, or the claim's current lease when that
/// is longer, so the step never cuts the provider-run lease its claim was granted (#92).
fn start_lease(now: i64, base: i64, current: Option<i64>) -> i64 {
    (now + base).max(current.unwrap_or(0))
}

/// `start_lease` for one claimed unit inside the step's transaction.
fn held_lease(tx: &Transaction, unit: &str) -> Result<i64> {
    let current: Option<i64> = tx.query_row("SELECT lease_until FROM units WHERE id=?1", [unit], |r| r.get(0))?;
    Ok(start_lease(now_seconds(), lease_seconds(), current))
}

/// One bounded claim of `take` ready units.
///
/// `lease` is the claim's own requested lifetime in seconds: the workflow passes the frozen
/// `dshRunCap` of the prepared run, because a claim whose holder starts a provider run must stay
/// current for at least that run's whole cap. A claim therefore never expires under a healthy job
/// (#92); the base lease and its heartbeat remain the floor and the ordinary renewal (#R178).
fn next_once(p: &Prepared, layer: &str, take: i64, owner: &str, lease: i64) -> Result<Value> {
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let now = now_seconds();
    reconcile(&tx, now)?;
    let mut statement=tx.prepare("SELECT id,input_hash,inputs_json,claim_token FROM units WHERE layer=?1 AND state='ready' ORDER BY logical_key,revision,id LIMIT ?2")?;
    let rows = statement
        .query_map(params![layer, take], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    let mut claimed = Vec::new();
    for (id, input_hash, inputs, old_token) in rows {
        let token = old_token.checked_add(1).context("claim token exhausted")?;
        let lease_until = now + claim_lease(lease_seconds(), lease);
        tx.execute("UPDATE units SET state='claimed',owner=?2,claim_token=?3,lease_until=?4,retry_after=NULL,attempts=attempts+1 WHERE id=?1",params![id,owner,token,lease_until])?;
        let claim = json!({"id":id,"owner":owner,"token":token,"lease_until":lease_until,"lease_seconds":lease_until-now,"input_hash":input_hash,"inputs":serde_json::from_str::<Value>(&inputs)?});
        event(
            &tx,
            &id,
            Some(owner),
            Some(token),
            "claimed",
            &json!({"input_hash":input_hash,"lease_until":lease_until,"process":{"pid":std::process::id()}}),
        )?;
        claimed.push(claim);
    }
    tx.commit()?;
    let status = status(&p.out, layer, None)?;
    Ok(claim_reply(&status, &claimed))
}

/// `ledger next -Brief`: the claimed units without their frozen inputs (the commands a block manager runs
/// read those from the ledger), so a claim of eight units stays a short line a relay copies whole.
fn brief_units(claimed: &[Value]) -> Vec<Value> {
    claimed
        .iter()
        .map(|unit| {
            let mut unit = unit.clone();
            if let Some(object) = unit.as_object_mut() {
                if let Some(inputs) = object.remove("inputs").as_ref().and_then(brief_inputs) {
                    object.insert("inputs".to_string(), inputs);
                }
            }
            unit
        })
        .collect()
}

/// The part of a unit's frozen inputs a brief reply keeps: the Model address a model or explain unit
/// works on (`header`, `model`), a short string the driver names in its finish command. Everything
/// else stays in the ledger (a relay dropped the 1.3 KB inputs of four model units on
/// grossman2002integration trial 4, and the driver finished `-Model 'undefined'`).
fn brief_inputs(inputs: &Value) -> Option<Value> {
    let mut kept = serde_json::Map::new();
    for key in ["header", "model"] {
        if let Some(value) = inputs.get(key).filter(|value| value.is_string()) {
            kept.insert(key.to_string(), value.clone());
        }
    }
    (!kept.is_empty()).then(|| Value::Object(kept))
}

/// `ledger status -Units <ids>`: the outcome of the named units, in the order named. A block driver reads
/// each held unit's state here instead of trusting a manager's report of it.
fn unit_view(units: &[Value], ids: &[String]) -> Value {
    let listed: Vec<Value> = ids
        .iter()
        .map(|id| match units.iter().find(|unit| unit["id"] == json!(id)) {
            Some(unit) => json!({
                "id": id,
                "state": unit["state"],
                "token": unit["token"],
                "input_hash": unit["input_hash"],
                "attempts": unit["attempts"],
                "fault_class": unit["fault_class"],
                "disposition": unit["result"]["disposition"],
            }),
            None => json!({"id": id, "state": "unknown"}),
        })
        .collect();
    json!({"schema_version": 1, "units": listed})
}

/// The claims one owner holds, in the shape a claim reply lists them (id, owner, token, lease, input
/// hash, inputs). A layer driver reads it when the relayed claim line came back summarized: the
/// ledger, not the relay, says which units the claim took (grossman2002integration trial 3).
fn held_view(units: &[Value], owner: &str) -> Value {
    let held: Vec<Value> = units
        .iter()
        .filter(|unit| unit["state"] == json!("claimed") && unit["owner"] == json!(owner))
        .map(|unit| {
            json!({
                "id": unit["id"],
                "owner": unit["owner"],
                "token": unit["token"],
                "lease_until": unit["lease_until"],
                "input_hash": unit["input_hash"],
                "inputs": brief_inputs(&unit["inputs"]).unwrap_or(Value::Null),
            })
        })
        .collect();
    json!({"schema_version": 1, "owner": owner, "units": held})
}

/// A claim reply is relayed whole by one manager: it carries the claimed units and the layer's
/// scheduling state, not the run report or the dependency graph. Those belong to `ledger status`
/// (and `-Verify`, `ledger report`, R331), so they are not repeated on every claim.
fn claim_reply(status: &Value, claimed: &[Value]) -> Value {
    let mut reply = serde_json::Map::new();
    for key in ["schema_version", "run_id", "invocation", "citation_key", "variant", "layer", "sealed", "settled", "complete", "counts"] {
        if let Some(value) = status.get(key) {
            reply.insert(key.to_string(), value.clone());
        }
    }
    reply.insert("units".to_string(), json!(claimed));
    Value::Object(reply)
}

struct Claim {
    owner: String,
    input_hash: String,
    attempts: i64,
}

struct UnitGuard {
    _file: std::fs::File,
}

fn unit_guard(p: &Prepared, unit: &str) -> Result<UnitGuard> {
    let directory = p.out.join("locks");
    std::fs::create_dir_all(&directory)?;
    let directory = std::fs::canonicalize(directory)?;
    if !directory.starts_with(&p.out) {
        bail!("unit guard path escaped the prepared output directory");
    }
    let path = directory.join(format!("{}.lock", canon::hash_value(&json!(unit))));
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
    {
        Ok(file) => file,
        // A Windows sharing violation is the same contended guard as a failed exclusive
        // lock, and R187 keeps both in the retryable `lock_busy` class.
        Err(error) if is_sharing_violation(&error) => {
            bail!("lock_busy: {unit}: {error}")
        }
        Err(error) => {
            return Err(error).with_context(|| format!("open unit guard {}", path.display()));
        }
    };
    file.try_lock()
        .map_err(|e| anyhow::anyhow!("lock_busy: {unit}: {e}"))?;
    Ok(UnitGuard { _file: file })
}

fn is_sharing_violation(error: &std::io::Error) -> bool {
    // ERROR_SHARING_VIOLATION (32) and ERROR_LOCK_VIOLATION (33) on Windows.
    matches!(error.raw_os_error(), Some(32) | Some(33))
}

/// The quiet window of `ledger reclaim -Release`: a held claim records activity at least every 30
/// seconds (the heartbeat) or every wait round (a manager's renewal), so a claim with no activity
/// for this long belongs to a stopped run. The variable exists so a public test can shorten it.
fn release_quiet_seconds() -> i64 {
    std::env::var("ZT_LEDGER_RELEASE_QUIET_SECONDS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|n| *n >= 1)
        .unwrap_or(120)
}

/// Whether a claim's last recorded activity (epoch milliseconds) is older than the quiet window.
fn quiet_claim(last_activity_ms: Option<i64>, now_ms: i64, quiet_seconds: i64) -> bool {
    last_activity_ms.is_none_or(|last| now_ms - last >= quiet_seconds * 1000)
}

/// The time of the last event recorded for one claim (the unit at that token), in epoch milliseconds.
fn last_activity_ms(tx: &Transaction, unit: &str, token: i64) -> Result<Option<i64>> {
    let utc: Option<String> = tx
        .query_row(
            "SELECT utc FROM events WHERE unit_id=?1 AND claim_token=?2 ORDER BY seq DESC LIMIT 1",
            params![unit, token],
            |r| r.get(0),
        )
        .optional()?;
    Ok(utc.as_deref().and_then(crate::report::epoch_millis))
}

/// A stranded claim is recoverable only through this guarded path: the reclaimer takes the
/// unit's own OS guard first, so it can never revoke a claim in the middle of an active
/// apply (R182), and it moves the token forward so the lost owner is fenced (R177).
///
/// `release` (`-Release`) also takes a claim whose lease has not ended, but only a quiet one: no
/// event for that claim within the quiet window, its guard free and no registered run live. It is
/// how a stopped run's claims are freed at once instead of after their provider-run lease.
fn reclaim(p: &Prepared, only: Option<&str>, take: i64, release: bool) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let run = meta(&tx, "run_id")?;
    let now = now_seconds();
    let mut candidates = Vec::new();
    match only {
        Some(unit) => {
            let row: Option<(String, i64, Option<i64>, Option<String>)> = tx
                .query_row(
                    "SELECT state,claim_token,lease_until,owner FROM units WHERE id=?1",
                    [unit],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?;
            let (state, token, lease, owner) =
                row.with_context(|| format!("unknown unit {unit}"))?;
            if state != "claimed" {
                bail!("claim_active: {unit} is {state}, not claimed");
            }
            if lease.unwrap_or(0) > now && !release {
                bail!(
                    "claim_active: {unit} still holds a live lease until {}",
                    lease.unwrap_or_default()
                );
            }
            candidates.push((unit.to_string(), token, owner));
        }
        None => {
            // With -Release every claim is a candidate; the quiet check below decides each one.
            let mut statement = tx.prepare(
                "SELECT id,claim_token,owner FROM units WHERE state='claimed' AND (?3 OR coalesce(lease_until,0)<=?1) ORDER BY id LIMIT ?2",
            )?;
            let rows = statement.query_map(params![now, take, release], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })?;
            for row in rows {
                candidates.push(row?);
            }
        }
    }
    tx.commit()?;
    drop(conn);
    let mut reclaimed = Vec::new();
    let mut blocked = Vec::new();
    let mut skipped = Vec::new();
    for (unit, token, owner) in candidates {
        let guard = match unit_guard(p, &unit) {
            Ok(guard) => guard,
            Err(error) => {
                if only.is_some() {
                    return Err(error);
                }
                skipped.push(json!({"unit":unit,"reason":"lock_busy","cause":error.to_string()}));
                continue;
            }
        };
        let mut conn = writer(p)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        verify_binding(&tx, p)?;
        let row: Option<(String, i64, Option<i64>, String)> = tx
            .query_row(
                "SELECT state,claim_token,lease_until,progress_json FROM units WHERE id=?1",
                [&unit],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let (state, current, lease, progress) =
            row.with_context(|| format!("unknown unit {unit}"))?;
        let live = lease.unwrap_or(0) > now;
        if state != "claimed" || current != token || (live && !release) {
            if only.is_some() {
                bail!("claim_active: {unit} changed while its guard was acquired; no claim was reclaimed");
            }
            skipped.push(json!({"unit":unit,"reason":"claim_active"}));
            tx.rollback()?;
            drop(guard);
            continue;
        }
        if live && !quiet_claim(last_activity_ms(&tx, &unit, current)?, now * 1000, release_quiet_seconds()) {
            // A holder that is still working records activity: leave its claim alone.
            if only.is_some() {
                bail!(
                    "claim_recent: {unit} recorded activity within the last {} seconds; its holder may still be working",
                    release_quiet_seconds()
                );
            }
            skipped.push(json!({"unit":unit,"reason":"claim_recent"}));
            tx.rollback()?;
            drop(guard);
            continue;
        }
        let progress: Value = serde_json::from_str(&progress)?;
        let running = progress["jobs"]
            .as_array()
            .map(|jobs| {
                jobs.iter()
                    .filter(|job| job["state"] != "terminal")
                    .filter_map(|job| job["id"].as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !running.is_empty() {
            // R184: a replacement attempt may not start while a registered run is live.
            if only.is_some() {
                bail!(
                    "jobs_running: {unit} still owns running external runs: {} (request cancellation and observe each terminal state before reclaiming)",
                    running.join(", ")
                );
            }
            blocked.push(json!({"unit":unit,"reason":"jobs_running","jobs":running}));
            tx.rollback()?;
            drop(guard);
            continue;
        }
        let new_token = current.checked_add(1).context("claim token exhausted")?;
        tx.execute(
            "UPDATE units SET state='ready',owner=NULL,claim_token=?2,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",
            params![unit, new_token],
        )?;
        event(
            &tx,
            &unit,
            owner.as_deref(),
            Some(current),
            "reclaimed",
            &json!({"previous_owner":owner,"previous_token":current,"token":new_token,"lease_expired_at":lease,"released":live,"guard":"held","process":{"pid":std::process::id()}}),
        )?;
        tx.commit()?;
        drop(guard);
        reclaimed.push(json!({"unit":unit,"previous_owner":owner,"previous_token":current,"token":new_token,"state":"ready","released":live}));
    }
    Ok(json!({
        "schema_version":1,
        "run_id":run,
        "reclaimed":reclaimed,
        "blocked":blocked,
        "skipped":skipped
    }))
}

fn jobs_of(progress: &Value) -> Vec<Value> {
    progress["jobs"].as_array().cloned().unwrap_or_default()
}

fn job_field(job: &Value, key: &str) -> Result<String> {
    job[key]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .with_context(|| format!("job needs a nonempty {key}"))
}

fn job_record(input: &Value, token: Option<i64>, owner: Option<&str>, cap_seconds: i64) -> Result<Value> {
    let id = job_field(input, "id")?;
    if id.len() > 200 {
        bail!("job id is at most 200 bytes");
    }
    let kind = input["kind"].as_str().filter(|k| !k.trim().is_empty()).unwrap_or("external");
    if kind.len() > 40 {
        bail!("job kind is at most 40 bytes");
    }
    let mut facts = input["facts"].as_object().cloned().unwrap_or_default();
    if let Some(extra) = input.as_object() {
        for (key, value) in extra {
            if !["id", "kind", "facts"].contains(&key.as_str()) {
                facts.insert(key.clone(), value.clone());
            }
        }
    }
    // The cap is measured from the registered provider start time (R267): a run
    // that a restart picks up again does not get a fresh 20 minutes.
    let now = now_seconds();
    let started = match facts.get("started_at").and_then(Value::as_i64) {
        Some(at) if at > 0 && at <= now => at,
        Some(at) => bail!("job started_at must be an epoch second from 1 through the current time, not {at}"),
        None => now,
    };
    let mut job = json!({
        "id":id,
        "kind":kind,
        "state":"running",
        "token":token,
        "owner":owner,
        "registered_at":canon::utc_now_o(),
        "started_at":started,
        "cap_seconds":cap_seconds,
        "facts":Value::Object(facts),
        "cancel_requested_at":Value::Null,
        "observed_at":Value::Null,
        "outcome":Value::Null
    });
    if let Some(token) = token {
        job["registration_token"] = json!(token);
    }
    Ok(job)
}

/// Elapsed and cap facts for one registered external run. A running run past its cap
/// is reported as `cap_exceeded` with the action the manager must take (R268): request
/// cancellation, observe the terminal state, then record a transient fault. A cancelled
/// run that is still running is reported as `cancellation_unconfirmed`, which blocks a
/// replacement attempt until its terminal observation arrives.
fn job_cap_facts(job: &mut Value, now: i64) {
    let Some(started) = job["started_at"].as_i64() else {
        return;
    };
    let cap = job["cap_seconds"].as_i64().unwrap_or(DEFAULT_DSH_RUN_CAP);
    let elapsed = (now - started).max(0);
    let running = job["state"].as_str() == Some("running");
    let cancelled = !job["cancel_requested_at"].is_null();
    let exceeded = running && elapsed >= cap;
    job["elapsed_seconds"] = json!(elapsed);
    job["cap_remaining_seconds"] = json!((cap - elapsed).max(0));
    job["cap_exceeded"] = json!(exceeded);
    job["action"] = json!(if exceeded && !cancelled {
        "cancel"
    } else if exceeded && cancelled {
        "await_terminal"
    } else {
        "wait"
    });
}

fn write_jobs(tx: &Transaction, unit: &str, jobs: &[Value]) -> Result<()> {
    let progress: String = tx.query_row(
        "SELECT progress_json FROM units WHERE id=?1",
        [unit],
        |r| r.get(0),
    )?;
    let mut progress: Value = serde_json::from_str(&progress)?;
    if !progress.is_object() {
        bail!("invalid unit progress object");
    }
    progress["jobs"] = json!(jobs);
    tx.execute(
        "UPDATE units SET progress_json=?2 WHERE id=?1",
        params![unit, canon::canonical_json(&progress)],
    )?;
    Ok(())
}

fn job_list(p: &Prepared, unit: &str) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let progress: String = tx
        .query_row("SELECT progress_json FROM units WHERE id=?1", [unit], |r| r.get(0))
        .optional()?
        .with_context(|| format!("unknown unit {unit}"))?;
    let progress: Value = serde_json::from_str(&progress)?;
    tx.commit()?;
    let now = now_seconds();
    let mut jobs = jobs_of(&progress);
    for job in jobs.iter_mut() {
        job_cap_facts(job, now);
    }
    Ok(json!({"schema_version":1,"action":"list","unit":unit,"settings":settings_of(&p.contract)?,"jobs":jobs}))
}

fn job_register(p: &Prepared, unit: &str, token: i64, path: &str) -> Result<Value> {
    let input = canon::read_json(path).context("invalid job registration")?;
    let cap_seconds = settings_of(&p.contract)?["dsh_run_cap"].as_i64().unwrap_or(DEFAULT_DSH_RUN_CAP);
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let claim = claim(&tx, unit, token)?;
    let mut jobs = {
        let progress: String = tx.query_row(
            "SELECT progress_json FROM units WHERE id=?1",
            [unit],
            |r| r.get(0),
        )?;
        jobs_of(&serde_json::from_str(&progress)?)
    };
    let record = job_record(&input, Some(token), Some(&claim.owner), cap_seconds)?;
    let id = job_field(&record, "id")?;
    if jobs.iter().any(|job| job["id"] == json!(id)) {
        bail!("job_conflict: {unit} already registered the run `{id}`");
    }
    jobs.push(record.clone());
    write_jobs(&tx, unit, &jobs)?;
    event(
        &tx,
        unit,
        Some(&claim.owner),
        Some(token),
        "job_registered",
        &json!({"job":id,"kind":record["kind"],"facts":record["facts"]}),
    )?;
    tx.commit()?;
    let now = now_seconds();
    for job in jobs.iter_mut() {
        job_cap_facts(job, now);
    }
    Ok(json!({"schema_version":1,"action":"register","unit":unit,"jobs":jobs}))
}

fn job_observe(p: &Prepared, unit: &str, path: &str) -> Result<Value> {
    let input = canon::read_json(path).context("invalid job observation")?;
    let id = job_field(&input, "id")?;
    let state = match input["state"].as_str() {
        Some("terminal") => "terminal",
        Some("running") => "running",
        _ => bail!("job observation state must be running or terminal"),
    };
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let mut jobs = {
        let progress: String = tx
            .query_row("SELECT progress_json FROM units WHERE id=?1", [unit], |r| r.get(0))
            .optional()?
            .with_context(|| format!("unknown unit {unit}"))?;
        jobs_of(&serde_json::from_str(&progress)?)
    };
    let index = jobs
        .iter()
        .position(|job| job["id"] == json!(id))
        .with_context(|| format!("unknown job: {unit} has no registered run `{id}`"))?;
    if jobs[index]["state"] == "terminal" {
        bail!("job_terminal: {unit} already observed run `{id}` as terminal");
    }
    let mut facts = jobs[index]["facts"].as_object().cloned().unwrap_or_default();
    for (key, value) in input["facts"]
        .as_object()
        .cloned()
        .unwrap_or_default()
    {
        facts.insert(key, value);
    }
    jobs[index]["facts"] = Value::Object(facts);
    jobs[index]["observed_at"] = json!(canon::utc_now_o());
    if state == "terminal" {
        let outcome = input["outcome"]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .context("a terminal observation records its outcome")?;
        if outcome.len() > 200 {
            bail!("job outcome is at most 200 bytes");
        }
        jobs[index]["state"] = json!("terminal");
        jobs[index]["outcome"] = json!(outcome);
    }
    write_jobs(&tx, unit, &jobs)?;
    event(
        &tx,
        unit,
        None,
        None,
        "job_observed",
        &json!({"job":id,"state":state,"outcome":jobs[index]["outcome"]}),
    )?;
    tx.commit()?;
    let now = now_seconds();
    for job in jobs.iter_mut() {
        job_cap_facts(job, now);
    }
    Ok(json!({"schema_version":1,"action":"observe","unit":unit,"jobs":jobs}))
}

fn job_cancel(p: &Prepared, unit: &str, id: &str) -> Result<Value> {
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let mut jobs = {
        let progress: String = tx
            .query_row("SELECT progress_json FROM units WHERE id=?1", [unit], |r| r.get(0))
            .optional()?
            .with_context(|| format!("unknown unit {unit}"))?;
        jobs_of(&serde_json::from_str(&progress)?)
    };
    let index = jobs
        .iter()
        .position(|job| job["id"] == json!(id))
        .with_context(|| format!("unknown job: {unit} has no registered run `{id}`"))?;
    if jobs[index]["state"] == "terminal" {
        bail!("job_terminal: {unit} already observed run `{id}` as terminal");
    }
    if jobs[index]["cancel_requested_at"].is_null() {
        jobs[index]["cancel_requested_at"] = json!(canon::utc_now_o());
    }
    write_jobs(&tx, unit, &jobs)?;
    event(
        &tx,
        unit,
        None,
        None,
        "job_cancel_requested",
        &json!({"job":id}),
    )?;
    tx.commit()?;
    let now = now_seconds();
    for job in jobs.iter_mut() {
        job_cap_facts(job, now);
    }
    Ok(json!({"schema_version":1,"action":"cancel","unit":unit,"jobs":jobs}))
}

fn claim(tx: &Transaction, unit: &str, token: i64) -> Result<Claim> {
    tx.query_row("SELECT owner,input_hash,attempts FROM units WHERE id=?1 AND state='claimed' AND claim_token=?2 AND lease_until>?3",params![unit,token,now_seconds()],|r|Ok(Claim{owner:r.get(0)?,input_hash:r.get(1)?,attempts:r.get(2)?})).optional()?.with_context(||format!("stale_claim: {unit} has no current unexpired claim for token {token}"))
}

fn check_claim(p: &Prepared, unit: &str, token: i64) -> Result<()> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    claim(&tx, unit, token)?;
    tx.commit()?;
    start_heartbeat(p, unit, token);
    Ok(())
}

fn renew(p: &Prepared, unit: &str, token: i64) -> Result<Value> {
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let claim = claim(&tx, unit, token)?;
    let current: Option<i64> = tx.query_row(
        "SELECT lease_until FROM units WHERE id=?1",
        [unit],
        |r| r.get(0),
    )?;
    let until = (now_seconds() + lease_seconds()).max(current.unwrap_or(0));
    tx.execute(
        "UPDATE units SET lease_until=?2 WHERE id=?1",
        params![unit, until],
    )?;
    event(
        &tx,
        unit,
        Some(&claim.owner),
        Some(token),
        "renewed",
        &json!({"lease_until":until,"manager":{"pid":std::process::id()}}),
    )?;
    tx.commit()?;
    Ok(json!({"unit":unit,"token":token,"lease_until":until}))
}

/// The claims a `ledger hold` keeps current: `id:token` pairs, comma separated.
fn hold_targets(text: &str) -> Result<Vec<(String, i64)>> {
    let mut targets = Vec::new();
    for item in text.split(',').map(str::trim).filter(|item| !item.is_empty()) {
        let (unit, token) = item
            .rsplit_once(':')
            .with_context(|| format!("-Units names {item:?}; each entry is <unit id>:<token>"))?;
        let token: i64 = token
            .trim()
            .parse()
            .ok()
            .filter(|token| *token > 0)
            .with_context(|| format!("-Units names {item:?}; its token is not a positive integer"))?;
        targets.push((unit.trim().to_string(), token));
    }
    if targets.is_empty() {
        bail!("-Units names no claim to hold");
    }
    Ok(targets)
}

/// `ledger hold`: renews every listed claim each `every` seconds for at most `seconds`, and returns as
/// soon as none of them is still held (a claim that is finished, failed or reclaimed answers
/// `stale_claim` and is released from the hold). A workflow step that runs longer than a claim's lease
/// without its own renewals (the model compiler's chains: on grossman2002integration trial 5 four model
/// claims expired mid-build) runs this beside the work, one bounded command after another.
fn hold(p: &Prepared, targets: &[(String, i64)], seconds: u64, every: u64) -> Result<Value> {
    let started = std::time::Instant::now();
    let mut held: Vec<(String, i64)> = targets.to_vec();
    let mut released: Vec<Value> = Vec::new();
    let mut renewals = 0u64;
    loop {
        let mut still = Vec::new();
        for (unit, token) in held {
            match renew(p, &unit, token) {
                Ok(_) => {
                    renewals += 1;
                    still.push((unit, token));
                }
                Err(error) => {
                    let text = error.to_string();
                    if text.contains("stale_claim") || text.contains("unknown unit") {
                        released.push(json!({"unit": unit, "token": token}));
                    } else {
                        return Err(error);
                    }
                }
            }
        }
        held = still;
        if held.is_empty() || started.elapsed().as_secs() + every > seconds {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(every));
    }
    Ok(json!({
        "schema_version": 1,
        "held": held.iter().map(|(unit, token)| json!({"unit": unit, "token": token})).collect::<Vec<_>>(),
        "released": released,
        "renewals": renewals,
        "seconds": started.elapsed().as_secs(),
    }))
}

fn fault_class(code: &str) -> Result<&'static str> {
    Ok(match code {
        "lock_busy"
        | "sharing_violation"
        | "sqlite_busy"
        | "transport_closed"
        | "command_timeout"
        | "service_unavailable"
        | "provider_cap"
        | "provider_transient"
        | "missing_result"
        | "missing_jobs"
        | "pending_jobs"
        | "read_view_changed"
        | "lease_lost" => "retryable",
        "access_refused"
        | "invalid_arguments"
        | "identity_mismatch"
        | "operation_conflict"
        | "owned_paragraph_conflict"
        | "unreconciled_move" => "operator",
        "invalid_plan" | "dependency_cycle" | "source_unavailable" | "unsupported_claim"
        | "stale_content" | "check_failed" | "unknown_producer" | "named_part_missing"
        | "named_part_changed" | "explanation_missing"
        // R326/R337: the owner's Lock stops the write its destination asked for. It is the destination's
        // decision, so it is a content fault and never a retryable one.
        | "card_locked" => "content",
        _ => bail!("unknown fault code `{code}`"),
    })
}

fn fail(p: &Prepared, unit: &str, token: i64, class: &str, fault: &Value) -> Result<Value> {
    check_claim(p, unit, token)?;
    let _guard = unit_guard(p, unit)?;
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let claim = claim(&tx, unit, token)?;
    if fault_class(field(fault, "code")?)? != class {
        bail!("fault classification mismatch: code does not permit class {class}");
    }
    // A fault without its input hash takes the claim's own: the token already fences the attempt
    // (ebrill2001modern trial 1: the cleanup stage sent its faults with an empty hash, every one was
    // refused, and the failed cleanup units stayed claimed). A hash that names another input is refused.
    let mut fault = fault.clone();
    if fault["input_hash"].as_str().is_none_or(|hash| hash.trim().is_empty()) {
        fault["input_hash"] = json!(claim.input_hash);
    }
    let fault = &fault;
    if field(fault, "unit")? != unit || field(fault, "input_hash")? != claim.input_hash {
        bail!("fault unit/input hash mismatch");
    }
    field(fault, "cause")?;
    // The attempt budget is the prepared run's own setting (R273): the workflow declares
    // it, preparation freezes it, and this transition spends it. A resume reuses it.
    let max_attempts = settings_of(&p.contract)?["max_attempts"]
        .as_i64()
        .unwrap_or(DEFAULT_MAX_ATTEMPTS);
    let retry = class == "retryable" && claim.attempts < max_attempts;
    let now = now_seconds();
    let retry_after = if retry {
        let mut jitter = [0u8; 1];
        getrandom::fill(&mut jitter).map_err(|e| anyhow::anyhow!("retry jitter: {e}"))?;
        Some(now + if claim.attempts == 1 { 5 } else { 20 } + i64::from(jitter[0] % 2))
    } else {
        None
    };
    let state = if retry { "retry" } else { "failed" };
    tx.execute("UPDATE units SET state=?2,owner=NULL,lease_until=NULL,retry_after=?3,fault_class=?4,fault_json=?5 WHERE id=?1",params![unit,state,retry_after,class,canon::canonical_json(fault)])?;
    event(
        &tx,
        unit,
        Some(&claim.owner),
        Some(token),
        "faulted",
        &json!({"state":state,"class":class,"fault":fault,"attempt":claim.attempts,"attempt_budget_exhausted":class=="retryable" && !retry,"retry_after":retry_after}),
    )?;
    reconcile(&tx, now)?;
    tx.commit()?;
    Ok(json!({"unit":unit,"token":token,"state":state,"fault":fault,"retry_after":retry_after}))
}

fn publish(path: &Path, text: &str) -> Result<Value> {
    util::write_atomic(path, text)?;
    let record = json!({"path":std::fs::canonicalize(path)?.to_string_lossy(),"sha256":canon::sha256_hex(text.as_bytes())});
    check_file(&record, "group job artifact")?;
    Ok(record)
}

fn start_view(job: &Value, path: &Path, reused: bool, lease_until: i64) -> Value {
    json!({"schema_version":1,"unit":job["unit"],"token":job["token"],"run_id":job["run_id"],"input_hash":job["input_hash"],"unit_input_hash":job["unit_input_hash"],"job":path.to_string_lossy(),"prompt":job["artifacts"]["prompt"]["path"],"snapshot":job["artifacts"]["snapshot"]["path"],"snapshot_revision":job["inputs"]["snapshot"]["archive_revision"],"earlier_context":job["earlier_context"],"earlier_context_path":job["artifacts"]["earlier_context"]["path"],"earlier_context_text":job["artifacts"]["earlier_context_text"]["path"],"draft":job["draft"],"expected_jobs":job["expected_jobs"],"reused":reused,"recovered":job["recovery"]["reason"]==json!("receipts_committed"),"recovery":job["recovery"],"lease_until":lease_until})
}

fn read_group_job(
    p: &Prepared,
    unit: &str,
    token: i64,
    input_hash: &str,
    progress: &Value,
    run: &Value,
) -> Result<(PathBuf, Value)> {
    if progress["group_start"]["token"] != token {
        bail!("missing current group-start binding");
    }
    let accepted = &progress["group_start"]["job"];
    let path = Path::new(field(accepted, "path")?);
    let job = bound_json(accepted, "group job artifact")?;
    // R280 (#87): a recovered group's retained accepted plan already committed, so its job offers
    // no agent work at all. Every other job keeps the one declared draft-only job contract.
    let recovered = job["recovery"]["reason"] == json!("receipts_committed");
    if job["schema_version"] != 1
        || job["mode"] != "draft_only"
        || if recovered {
            job["expected_jobs"] != 0
                || job["jobs"].as_array().is_none_or(|jobs| !jobs.is_empty())
                || job["artifacts"]["rebound_draft"]["sha256"]
                    != canon::sha256_hex(&std::fs::read(field(&job, "draft")?)?)
        } else {
            job["expected_jobs"] != 1
                || job["jobs"].as_array().is_none_or(|jobs| jobs.len() != 1)
                || job["jobs"][0]["permissions"]["archive_write"] != false
        }
    {
        bail!("incompatible group job contract");
    }
    if job["unit"] != unit
        || job["token"] != token
        || job["run_id"] != *run
        || job["unit_input_hash"] != input_hash
        || job["input_hash"] != canon::hash_value(&job["inputs"])
        || job["input_hash"] != progress["group_start"]["input_hash"]
    {
        bail!("group job binding mismatch");
    }
    for record in job["artifacts"]
        .as_object()
        .context("missing group job artifacts")?
        .values()
        .filter(|r| !r.is_null())
    {
        check_file(record, "group job artifact")?;
    }
    let path = std::fs::canonicalize(path)?;
    if !path.starts_with(&p.out) {
        bail!("group job path escaped prepared output directory");
    }
    Ok((path, job))
}

fn group_start(p: &Prepared, zt: &mut Zt, number: u64, token: i64) -> Result<Value> {
    let group = p.contract["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["n"].as_u64() == Some(number))
        .context("prepared group does not exist")?;
    let unit = format!("digest/g{number:03}@1");
    check_claim(p, &unit, token)?;
    let _guard = unit_guard(p, &unit)?;
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let claimed = claim(&tx, &unit, token)?;
    let progress: String = tx.query_row(
        "SELECT progress_json FROM units WHERE id=?1",
        [&unit],
        |r| r.get(0),
    )?;
    let mut progress: Value = serde_json::from_str(&progress)?;
    if !progress.is_object() {
        bail!("invalid group progress object");
    }
    // S05 (#98): a group that has not completed is still a placeholder anchor, so it is never
    // offered to another group as earlier thought (D094).
    let unfinished = {
        let mut statement = tx.prepare("SELECT id FROM units WHERE id GLOB 'digest/g*' AND state<>'done'")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let run = meta(&tx, "run_id")?;
    tx.commit()?;
    drop(conn);
    if claimed.input_hash != canon::hash_value(group) {
        bail!("group input hash mismatch");
    }
    if progress["group_start"]["token"] == token {
        let (path, job) = read_group_job(p, &unit, token, &claimed.input_hash, &progress, &run)?;
        let renewed = renew(p, &unit, token)?;
        return Ok(start_view(
            &job,
            &path,
            true,
            renewed["lease_until"].as_i64().unwrap(),
        ));
    }
    renew(p, &unit, token)?;
    let anchor = zt.cards(&[field(group, "anchor")?.to_string()], "text")?;
    let trees = crate::snapshot::read_literature_trees(zt)?;
    let revision = trees
        .first()
        .and_then(|t| t["archive_revision"].as_u64())
        .context("group snapshot has no prepared archive view")?;
    let after = zt.archive_view()?;
    if after["archive_identity"] != p.contract["archive_identity"]
        || after["archive_revision"].as_u64() != Some(revision)
        || anchor.archive_revision != revision
    {
        bail!("read_view_changed: group-start archive view changed; retry");
    }
    // docs/literature-only-digest-design.md: the group reads and links Literature trees only.
    let (snapshot, _) = crate::snapshot::render(&trees, 40);
    let directory = p
        .out
        .join("group-jobs")
        .join(format!("g{number:03}"))
        .join(format!("claim-{token}"));
    std::fs::create_dir_all(&directory)?;
    let directory = std::fs::canonicalize(directory)?;
    if !directory.starts_with(&p.out) {
        bail!("group job path escaped the prepared output directory");
    }
    let snapshot_path = directory.join("snapshot.md");
    let prompt_path = directory.join("prompt.md");
    let job_path = directory.join("job.json");
    let draft_path = directory.join("draft.json");
    let mut snapshot_record = publish(&snapshot_path, &snapshot)?;
    snapshot_record["archive_revision"] = json!(revision);
    let run_options = run_options(
        field(&p.contract, "variant")?,
        p.contract["options"]["no_figures"] == true,
        p.contract["options"].get("run_options"),
    )?;
    let mut terms_text = String::new();
    let terms = if run_options["terms"] == true {
        let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
        let required:Option<(String,String)>=conn.query_row("SELECT o.locator,o.fingerprint FROM outputs o JOIN units u ON u.id=o.unit_id WHERE o.unit_id='terms/book@1' AND o.output_key='terms' AND o.kind='file' AND u.state='done'",[],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let (locator, fingerprint) =
            required.context("required verified term-table output is missing")?;
        drop(conn);
        let source_path = p.out.join("terms.md");
        let bytes =
            std::fs::read(&source_path).context("required applied term table is missing")?;
        if std::fs::canonicalize(&source_path)? != Path::new(&locator)
            || canon::sha256_hex(&bytes) != fingerprint
        {
            bail!("required term-table output changed");
        }
        let terms = std::str::from_utf8(&bytes)?;
        let terms = terms.strip_prefix('\u{feff}').unwrap_or(terms);
        terms_text = terms.to_string();
        let path = directory.join("terms.md");
        publish(&path, terms)?
    } else {
        Value::Null
    };
    let anchor = &anchor.cards[0];
    let source_prefix = &p.contract["artifacts"]["shared_prefix"];
    let prefix_bytes = std::fs::read(field(source_prefix, "path")?)?;
    if canon::sha256_hex(&prefix_bytes) != source_prefix["sha256"] {
        bail!("shared prefix hash mismatch during group-start");
    }
    let prefix = publish(
        &directory.join("shared-prefix.md"),
        std::str::from_utf8(&prefix_bytes)?,
    )?;
    let mut source = String::new();
    for section in group["sections"].as_array().unwrap() {
        let bytes = std::fs::read(field(section, "path")?)?;
        if canon::sha256_hex(&bytes) != section["sha256"] {
            bail!("group source section hash mismatch during startup");
        }
        source.push_str(std::str::from_utf8(&bytes)?);
        source.push_str("\n\n");
    }
    // S05 (#98): the bounded earlier-Card pack, built once from this start's own title catalogue
    // and one bounded body read. It is frozen in `inputs`, so a resumed group reads the same pack
    // (D088, D093).
    let earlier = {
        let key = field(&p.contract, "citation_key")?;
        let mut exclude: BTreeSet<String> = BTreeSet::new();
        for id in &unfinished {
            if let Some(number) = id
                .strip_prefix("digest/g")
                .and_then(|rest| rest.split('@').next())
                .and_then(|number| number.parse::<u64>().ok())
            {
                exclude.insert(format!("{key}/{number}"));
            }
        }
        exclude.insert(field(anchor, "address")?.to_string());
        let mut title = s(&group["chapter"]);
        for heading in group["headings"].as_array().into_iter().flatten() {
            let heading = s(heading);
            if !heading.is_empty() && !title.contains(&heading) {
                title.push(' ');
                title.push_str(&heading);
            }
        }
        let claims = match p.contract["group_extension"]["marker"].as_str() {
            Some(marker) => crate::earlier::group_claims(field(group, "path")?, marker),
            None => Vec::new(),
        };
        let request = crate::earlier::Request {
            unit: unit.clone(),
            group: number,
            query: crate::earlier::Query {
                title,
                source: source.clone(),
                terms: terms_text.clone(),
                claims,
            },
            exclude,
        };
        let pack = crate::earlier::build(zt, &trees, revision, &p.contract["archive_identity"], &request)?;
        let record = publish(
            &directory.join("earlier-context.json"),
            &serde_json::to_string_pretty(&pack.json)?,
        )?;
        let text = publish(&directory.join("earlier-context.md"), &pack.markdown)?;
        // S06 (#103): the earlier agent Regular Cards this group may continue as a Side successor.
        // The file is what the writer passes as `-Beneath`, so a standalone claim can be filed beside
        // earlier thought without any older Card, tree or carrier becoming writable (D100, D104).
        let beside: Vec<String> = pack.place_beside();
        let placed = publish(
            &directory.join("place-beside.txt"),
            &format!("{}\n", beside.join("\n")).trim_start_matches('\n').to_string(),
        )?;
        (record, text, pack, placed, beside)
    };
    let inputs = json!({"schema_version":1,"prompt_contract":1,"draft_schema":1,"mode":"draft_only","run_id":run,"unit":unit,"token":token,"preparation_hash":p.hash,"unit_input_hash":claimed.input_hash,"group":group,"anchor":{"address":anchor["address"],"fingerprint":crate::calls::authored_fingerprint(anchor)},"prefix":prefix,"snapshot":snapshot_record,"terms":terms,"earlier_context":earlier.0,"place_beside":earlier.3,"contracts":p.contract["contracts"]});
    let input_hash = canon::hash_value(&inputs);
    // R280 (#87): a previous attempt's accepted plan whose every call already committed is the
    // group's checked result. Rebound to this claim's token it starts no DeepSeek job: the group
    // finishes from the archive receipts without rebuilding a Card.
    let recovery = recover_group(p, zt, number, &unit, &run, &claimed.input_hash)?;
    let recovered = recovery["status"] == json!("recovered");
    let (disposition, calls) = if recovered {
        // The accepted plan keeps each call's `src` apart in `source_notes`; the rebound draft puts
        // it back, so `group-finish` rebuilds the very plan it accepted.
        let mut calls = recovery["plan"]["calls"].clone();
        if let Some(notes) = recovery["plan"]["source_notes"].as_object() {
            for (i, note) in notes {
                let i: usize = i.parse().context("accepted plan source note index")?;
                calls
                    .get_mut(i)
                    .and_then(|call| call["arguments"].as_object_mut())
                    .context("accepted plan source note names no call")?
                    .insert("src".to_string(), note.clone());
            }
        }
        (
            field(&recovery["plan"], "disposition")?.to_string(),
            calls,
        )
    } else {
        ("cards".to_string(), json!([]))
    };
    let draft = json!({"schema_version":1,"unit":unit,"token":token,"input_hash":input_hash,"disposition":disposition,"calls":calls});
    let draft_record = if recovered {
        // The rebound draft keeps the retained calls and carries this attempt's own binding.
        Some(publish(&draft_path, &serde_json::to_string_pretty(&draft)?)?)
    } else {
        None
    };
    let self_check_command = format!(
        "zt-digest group-finish -Out \"{}\" -Group {number} -Token {token} -Draft \"{}\" -CheckOnly",
        p.out.display(),
        draft_path.display()
    );
    let prompt = format!(
        "# 第 {number} 组\n\n本任务只生成 JSON 草稿，写入 `{}`。将共享规则中的写入操作表示为 calls 提案。归档写入和实际 SELF-CHECK 由 group-finish 工具完成。\nSELF-CHECK: {self_check_command}\n\n共享规则：`{}`\n只读启动快照：`{}`\n术语输入：{}\n早前上下文包：`{}`（本组之前最多 {} 张卡片，含摘要与读全文的 locator；写卡前先读它）\n全部卡片都写在本书的文献树里：不新建主题，不写主题树的卡，不写枢纽卡，不链接主题树的卡（主题树归 zt-idea-grow）。\n\n草稿必须保留以下 schema_version、unit、token 和 input_hash，填写 calls。仅有空数组或原始占位锚点不能建立覆盖。创建锚点后的卡片时使用 Side successor，保留其他组的直接锚点位置。编辑本锚点时的 base_fingerprint 为 `{}`。\n\n```json\n{}\n```\n\n## 当前锚点\n\n{}\n\n{}\n\n## 本组选定原文\n\n{}",
        draft_path.display(),
        field(&inputs["prefix"], "path")?,
        snapshot_path.display(),
        if terms.is_null() {
            "本次明确关闭术语层。".to_string()
        } else {
            format!("`{}`", field(&terms, "path")?)
        },
        field(&earlier.1, "path")?,
        earlier.2.candidates,
        inputs["anchor"]["fingerprint"].as_str().unwrap(),
        serde_json::to_string_pretty(&draft)?,
        field(anchor, "title")?,
        field(anchor, "body")?,
        source
    );
    let prompt = publish(&prompt_path, &prompt)?;
    let mut files = vec![
        prefix["path"].clone(),
        json!(prompt_path.to_string_lossy()),
        json!(snapshot_path.to_string_lossy()),
        earlier.1["path"].clone(),
        earlier.3["path"].clone(),
    ];
    if !terms.is_null() {
        files.push(terms["path"].clone());
    }
    let jobs = if recovered {
        // The recovered draft is already the group's checked result: no job is offered to an agent.
        json!([])
    } else {
        json!([{"id":format!("digest-g{number:03}-t{token}"),"kind":"digest","files":files,"prompt":format!("按组任务生成 JSON 草稿并写入 {}，保留绑定字段。",draft_path.display()),"output":draft_path.to_string_lossy(),"permissions":{"archive_read":true,"archive_write":false}}])
    };
    let expected_jobs = if recovered { json!(0) } else { json!(1) };
    let mut job = json!({"schema_version":1,"unit":unit,"token":token,"run_id":run,"owner":claimed.owner,"unit_input_hash":claimed.input_hash,"input_hash":input_hash,"inputs":inputs,"mode":"draft_only","expected_jobs":expected_jobs,"draft":draft_path.to_string_lossy(),"earlier_context":earlier.2.facts(),"place_beside":{"count":earlier.4.len()},"artifacts":{"prompt":prompt,"prefix":prefix,"snapshot":snapshot_record,"terms":terms,"earlier_context":earlier.0,"earlier_context_text":earlier.1,"place_beside":earlier.3,"rebound_draft":draft_record},"jobs":jobs});
    if recovered {
        job["recovery"] = json!({
            "reason":"receipts_committed",
            "plan":recovery["path"],
            "plan_hash":recovery["plan_hash"],
            "receipts":recovery["receipts"],
            "archive_revision":recovery["archive_revision"],
        });
    } else if recovery["status"] == json!("pending") {
        // An accepted plan exists but its receipts do not all verify yet: the agent must finish the
        // remaining calls of that same accepted plan, so the ordinary job is published unchanged.
        job["recovery"] = json!({"reason":"receipts_pending","plan":recovery["path"]});
    }
    let record = publish(&job_path, &serde_json::to_string_pretty(&job)?)?;
    progress["group_start"] = json!({"token":token,"input_hash":input_hash,"job":record});
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, &unit, token)?;
    let tx_lease = held_lease(&tx, &unit)?;
    tx.execute(
        "UPDATE units SET progress_json=?2,lease_until=?3 WHERE id=?1",
        params![unit, canon::canonical_json(&progress), tx_lease],
    )?;
    event(
        &tx,
        &unit,
        Some(&claimed.owner),
        Some(token),
        "group_started",
        &json!({"input_hash":input_hash,"job":record,"snapshot_revision":revision,"earlier_context":earlier.2.facts(),"lease_until":tx_lease,"recovered":recovered}),
    )?;
    if recovered {
        event(
            &tx,
            &unit,
            Some(&claimed.owner),
            Some(token),
            "group_recovered",
            &json!({"plan":recovery["path"],"plan_hash":recovery["plan_hash"],"receipts":recovery["receipts"],"archive_revision":recovery["archive_revision"],"job":record}),
        )?;
    }
    tx.commit()?;
    Ok(start_view(&job, &job_path, recovered, tx_lease))
}

/// R280 (#87): reconcile the group's retained accepted plan against the archive receipts.
///
/// The accepted plan is immutable once `group-finish` accepted it, so a later attempt can only ever
/// complete the same calls. When every call already has its committed receipt, the group's result
/// exists and the attempt must not rebuild it: the plan is returned so `group-start` can rebound it
/// to the new claim and publish a job with no work for an agent.
fn recover_group(
    p: &Prepared,
    zt: &mut Zt,
    number: u64,
    unit: &str,
    run: &Value,
    unit_input_hash: &str,
) -> Result<Value> {
    let path = p.out.join(format!("accepted-g{number:03}.json"));
    if !path.exists() {
        return Ok(json!({"status":"absent"}));
    }
    let (plan, record) = json_artifact(&path)?;
    if plan["schema_version"] != json!(1)
        || plan["unit"] != json!(unit)
        || plan["unit_input_hash"] != json!(unit_input_hash)
        || plan["run_id"] != *run
    {
        // An accepted plan of another unit, input or run is not this group's result.
        return Ok(json!({"status":"pending","path":record["path"],"reason":"plan_binding_mismatch"}));
    }
    let disposition = plan["disposition"].as_str().unwrap_or_default();
    if !matches!(disposition, "cards" | "no_cards") {
        return Ok(json!({"status":"pending","path":record["path"],"reason":"plan_disposition"}));
    }
    let calls = plan["calls"]
        .as_array()
        .context("accepted plan calls must be an array")?;
    if calls.is_empty() {
        // A `no_cards` plan writes nothing; its completion is still verified by `group-finish`.
        return Ok(json!({
            "status":"recovered",
            "path":record["path"],
            "plan_hash":record["sha256"],
            "plan":plan,
            "receipts":[],
            "archive_revision":Value::Null
        }));
    }
    let prefix = format!(
        "{}/digest-g{number:03}/1/apply",
        run.as_str().context("missing run id")?
    );
    let preview = match crate::calls::preview_identified(zt, calls, &prefix, None) {
        Ok(preview) => preview,
        // A plan whose calls no longer replay is not recovered here: the ordinary attempt and
        // `group-finish` report the exact conflict instead of a fabricated completion.
        Err(_) => return Ok(json!({"status":"pending","path":record["path"],"reason":"receipts_unavailable"})),
    };
    let results = preview["results"]
        .as_array()
        .context("incomplete receipt readback")?;
    let mut receipts = Vec::new();
    for (index, result) in results.iter().enumerate() {
        if result["replayed"] != true {
            return Ok(json!({
                "status":"pending",
                "path":record["path"],
                "reason":"receipts_pending",
                "missing":index
            }));
        }
        receipts.push(json!({
            "op_key":result["op_key"],
            "address":result["result"]["address"],
        }));
    }
    Ok(json!({
        "status":"recovered",
        "path":record["path"],
        "plan_hash":record["sha256"],
        "plan":plan,
        "receipts":receipts,
        "archive_revision":preview["archive_revision"],
    }))
}

pub fn group_start_cli(argv: &[String]) -> Result<i32> {
    let parsed = (|| -> Result<(String, u64, i64)> {
        let a = Args::parse(argv, &["Out", "Group", "Token"], &[])?;
        let out = a.required("Out")?;
        if out.trim().is_empty() {
            bail!("-Out is required");
        }
        if !a.positional.is_empty() {
            bail!("unexpected group-start positional arguments");
        }
        let group = a
            .required("Group")?
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .context("-Group must be a positive integer")?;
        let token = a
            .required("Token")?
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .context("-Token must be a positive integer")?;
        Ok((out, group, token))
    })();
    let (out, group, token) = match parsed {
        Ok(v) => v,
        Err(e) => {
            eprintln!("zt-digest group-start: {e}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let result = group_start(&p, &mut zt, group, token)?;
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}

fn finish_claim(p: &Prepared, unit: &str, token: i64) -> Result<(Claim, Value, Value, bool)> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let row:Option<(String,i64,String,String,i64,String)>=tx.query_row("SELECT state,claim_token,coalesce(owner,''),input_hash,attempts,progress_json FROM units WHERE id=?1",[unit],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
    let (state, current, owner, input_hash, attempts, progress) =
        row.with_context(|| format!("stale_claim: unknown unit {unit}"))?;
    if current != token || !matches!(state.as_str(), "claimed" | "done") {
        bail!("stale_claim: {unit} has no current completion for token {token}");
    }
    let progress: Value = serde_json::from_str(&progress)?;
    if state == "claimed" {
        claim(&tx, unit, token)?;
        pending_jobs(unit, &progress)?;
    }
    let run = meta(&tx, "run_id")?;
    tx.commit()?;
    if state == "claimed" {
        start_heartbeat(p, unit, token);
    }
    Ok((
        Claim {
            owner,
            input_hash,
            attempts,
        },
        progress,
        run,
        state == "done",
    ))
}

fn completion_output_keys(result: &Value) -> Result<std::collections::BTreeSet<String>> {
    let mut keys = std::collections::BTreeSet::from(
        ["accepted", "owned", "source", "self_check", "completion"].map(str::to_string),
    );
    for at in result["owned"]
        .as_array()
        .context("missing completion owned set")?
    {
        keys.insert(format!(
            "card/{}",
            at.as_str().context("invalid owned output")?
        ));
    }
    for key in result["receipt_keys"]
        .as_array()
        .context("missing completion receipt enumeration")?
    {
        keys.insert(format!(
            "receipt/{}",
            key.as_str().context("invalid receipt key")?
        ));
    }
    Ok(keys)
}

fn terms_output_keys(result: &Value) -> Result<std::collections::BTreeSet<String>> {
    if result["schema_version"] != 1
        || result["kind"] != "terms_completion"
        || result["unit"] != "terms/book@1"
        || result["self_check"]["ran"] != true
        || result["self_check"]["count"] != 0
    {
        bail!("incomplete typed Terms completion evidence");
    }
    let evidence = result["evidence"]
        .as_object()
        .context("missing Terms evidence enumeration")?;
    if evidence.len() != 5
        || evidence.keys().any(|key| {
            !["plan", "accepted", "answer", "terms", "self_check"].contains(&key.as_str())
        })
    {
        bail!("incomplete Terms evidence enumeration");
    }
    let mut keys = std::collections::BTreeSet::from(
        ["plan", "accepted", "terms", "self_check", "completion"].map(str::to_string),
    );
    for name in ["plan", "accepted", "terms", "self_check"] {
        field(&evidence[name], "path")?;
        field(&evidence[name], "sha256")?;
    }
    let receipts = result["receipt_keys"]
        .as_array()
        .context("missing Terms receipt enumeration")?;
    match result["disposition"].as_str() {
        Some("terms") => {
            if result["terms"].as_u64().is_none_or(|n| n == 0)
                || result["candidate_count"].as_u64().is_none_or(|n| n == 0)
                || receipts.len() != 1
            {
                bail!("incomplete applied Terms table evidence");
            }
            field(&evidence["answer"], "path")?;
            field(&evidence["answer"], "sha256")?;
            keys.insert("answer".into());
            keys.insert(format!("card/{}", field(result, "card")?));
            keys.insert(format!(
                "receipt/{}",
                receipts[0].as_str().context("invalid Terms receipt key")?
            ));
        }
        Some("no_terms") => {
            if result["terms"] != 0
                || result["candidate_count"] != 0
                || result["no_op"]["reason"] != "no_terms_candidates"
                || !receipts.is_empty()
                || !result["card"].is_null()
                || !evidence["answer"].is_null()
            {
                bail!("incomplete no-terms enumeration evidence");
            }
        }
        _ => bail!("invalid Terms disposition"),
    }
    Ok(keys)
}

/// Read-only ledger reconciliation. File/Card checks remain a typed verifier's job.
/// Whether one done unit's record is an adoption verification rather than a historic
/// completion (R413). An adoption declares the outputs and children it registered itself, so
/// the same emission invariant holds without pretending a command enumeration exists.
fn is_adoption(result: &Value) -> bool {
    result["kind"]
        .as_str()
        .is_some_and(|kind| kind.ends_with("_adoption"))
}

/// The output keys an adoption record declares. Adoption writes no Card and starts no command,
/// so its enumeration is exactly the evidence it registered.
fn adoption_output_keys(result: &Value) -> Result<std::collections::BTreeSet<String>> {
    result["outputs"]
        .as_array()
        .context("missing adoption output enumeration")?
        .iter()
        .map(|key| {
            Ok(key.as_str().context("invalid adoption output key")?.to_string())
        })
        .collect()
}

fn emission_faults(conn: &Connection) -> Result<Vec<Value>> {
    let producers = conn
        .prepare(
            "SELECT id,result_json FROM units WHERE layer='digest' AND state='done' ORDER BY id",
        )?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut faults = Vec::new();
    let preparation = meta(conn, "preparation")?;
    if preparation["artifacts"].get("setup_calls").is_some() {
        let hash = meta(conn, "preparation_hash")?;
        if let Err(e) = preparation_registration(
            conn,
            &preparation,
            hash.as_str().context("invalid preparation hash")?,
        ) {
            faults.push(json!({"code":"preparation_registration_mismatch","unit":"prepare/book@1","cause":e.to_string()}));
        }
        let raw: Option<String> = conn
            .query_row(
                "SELECT result_json FROM units WHERE id='prepare/book@1' AND state='done'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(raw) = raw {
            let result: Value = serde_json::from_str(&raw)?;
            if let Err(e) = preparation_outputs(conn, &result) {
                faults.push(json!({"code":"preparation_outputs_mismatch","unit":"prepare/book@1","cause":e.to_string()}));
            }
        }
    }
    for (producer, raw) in producers {
        let result: Value = serde_json::from_str(&raw)?;
        let adoption = is_adoption(&result);
        // R413: an adoption carries its declaration too. A record that names neither is a
        // producer whose enumeration is missing, exactly as a completion without one is.
        let declared = if adoption {
            result.get("emitted").and_then(Value::as_array).cloned().unwrap_or_default()
        } else {
            match result["emitted"].as_array() {
                Some(emitted) => emitted.clone(),
                None => {
                    faults.push(json!({"code":"producer_enumeration_missing","unit":producer}));
                    continue;
                }
            }
        };
        let emitted = &declared;
        let mut expected = std::collections::BTreeSet::new();
        for child in emitted {
            let id = field(child, "id")?;
            expected.insert(id.to_string());
            let row: Option<(String, Option<String>, String)> = conn
                .query_row(
                    "SELECT input_hash,created_by,inputs_json FROM units WHERE id=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let matches = if let Some((hash, by, inputs)) = &row {
                child["input_hash"] == *hash
                    && by.as_deref() == Some(&producer)
                    && canon::hash_value(&serde_json::from_str(inputs)?) == *hash
            } else {
                false
            };
            if !matches {
                faults.push(
                    json!({"code":"emitted_unit_missing_or_changed","unit":id,"producer":producer}),
                );
            }
            for edge in child["dependencies"]
                .as_array()
                .context("incomplete producer dependency enumeration")?
            {
                let condition: Option<String> = conn
                    .query_row(
                        "SELECT condition FROM deps WHERE unit_id=?1 AND needs_id=?2",
                        params![field(edge, "unit")?, field(edge, "needs")?],
                        |r| r.get(0),
                    )
                    .optional()?;
                if condition.as_deref() != edge["condition"].as_str() {
                    faults.push(json!({"code":"emitted_dependency_missing_or_changed","producer":producer,"unit":edge["unit"],"needs":edge["needs"]}));
                }
            }
        }
        let actual = conn
            .prepare("SELECT id FROM units WHERE created_by=?1 ORDER BY id")?
            .query_map([&producer], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<std::collections::BTreeSet<_>>>()?;
        if actual != expected {
            faults.push(json!({"code":"producer_children_mismatch","unit":producer,"expected":expected,"actual":actual}));
        }
        let expected_outputs = if adoption {
            adoption_output_keys(&result)?
        } else {
            completion_output_keys(&result)?
        };
        let actual_outputs = conn
            .prepare("SELECT output_key FROM outputs WHERE unit_id=?1")?
            .query_map([&producer], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<std::collections::BTreeSet<_>>>()?;
        if expected_outputs != actual_outputs {
            faults.push(json!({"code":"producer_outputs_mismatch","unit":producer,"missing":expected_outputs.difference(&actual_outputs).collect::<Vec<_>>(),"unexpected":actual_outputs.difference(&expected_outputs).collect::<Vec<_>>()}));
        }
    }
    let terms: Option<String> = conn
        .query_row(
            "SELECT result_json FROM units WHERE id='terms/book@1' AND state='done'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(raw) = terms {
        let result: Value = serde_json::from_str(&raw)?;
        let expected = if is_adoption(&result) {
            adoption_output_keys(&result)?
        } else {
            terms_output_keys(&result)?
        };
        let actual = conn
            .prepare("SELECT output_key FROM outputs WHERE unit_id='terms/book@1'")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<std::collections::BTreeSet<_>>>()?;
        if actual != expected {
            faults.push(json!({"code":"producer_outputs_mismatch","unit":"terms/book@1","missing":expected.difference(&actual).collect::<Vec<_>>(),"unexpected":actual.difference(&expected).collect::<Vec<_>>()}));
        }
    }
    // A completed Events/Findings unit states its own required output registration: the
    // accepted list, source links, Link/Organize inputs, check and every Card it touched.
    for layer in EMPIRICAL {
        let completed = conn
            .prepare("SELECT id,result_json FROM units WHERE layer=?1 AND state='done' ORDER BY id")?
            .query_map([layer.layer], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, raw) in completed {
            let result: Value = serde_json::from_str(&raw)?;
            let Ok(expected) = empirical_output_keys(&result) else {
                faults.push(json!({"code":"producer_enumeration_missing","unit":id}));
                continue;
            };
            let actual = conn
                .prepare("SELECT output_key FROM outputs WHERE unit_id=?1")?
                .query_map([&id], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<std::collections::BTreeSet<_>>>()?;
            if actual != expected {
                faults.push(json!({"code":"producer_outputs_mismatch","unit":id,"missing":expected.difference(&actual).collect::<Vec<_>>(),"unexpected":actual.difference(&expected).collect::<Vec<_>>()}));
            }
        }
    }
    Ok(faults)
}

fn group_quality(cards: &[Value], p: &Prepared) -> Result<Value> {
    use std::collections::{HashMap, HashSet};
    let book = field(&p.contract, "citation_key")?;
    let literature = HashSet::from([book.to_string()]);
    let anchors = HashMap::from([(
        book.to_string(),
        p.contract["groups"].as_array().unwrap().len(),
    )]);
    let mut entries = Vec::new();
    for card in cards {
        if !card["links"].is_array() || !card["kinds"].is_object() {
            bail!("incomplete group checker Link facts");
        }
        let mut entry = card.clone();
        entry["category"] = json!(zt_check::graph::category(card, &literature, &anchors));
        entry["parent"] = card["parent_address"].clone();
        entry["kinds"] = json!(
            card["kinds"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>()
        );
        entry["book"] = json!(book);
        entries.push(entry);
    }
    let mut file = tempfile::NamedTempFile::new()?;
    serde_json::to_writer(&mut file, &entries)?;
    std::io::Write::flush(&mut file)?;
    let check = crate::check::Check {
        json: file.path().to_string_lossy().into_owned(),
        ..Default::default()
    };
    let lines = zt_check::check::run_lines(&check.args(), None)
        .context("group draft checker did not run")?;
    checked_lines(lines, true)
}

fn checked_lines(lines: Vec<String>, advisories_counted: bool) -> Result<Value> {
    let count = lines
        .last()
        .and_then(|l| l.strip_prefix("faults: "))
        .and_then(|n| n.parse::<usize>().ok())
        .context("group checker did not report completeness")?;
    let fault_line = regex::Regex::new(r"^\S+ [A-Z][0-9]+ ")?;
    let observations = &lines[..lines.len() - 1];
    if observations.iter().any(|line| !fault_line.is_match(line)) {
        bail!("incomplete group checker fault format");
    }
    let (advisories, faults): (Vec<_>, Vec<_>) = observations
        .iter()
        .partition(|line| line.contains("(advisory)"));
    if count
        != if advisories_counted {
            observations.len()
        } else {
            faults.len()
        }
    {
        bail!("incomplete group checker fault count");
    }
    if let Some(fault) = faults.iter().find(|line| {
        line.split_whitespace()
            .nth(1)
            .is_some_and(|r| matches!(r, "R18" | "R19"))
    }) {
        bail!("group draft corruption: {fault}");
    }
    Ok(
        json!({"ran":true,"count":faults.len(),"reported_count":count,"faults":faults,"advisories":advisories,"lines":lines}),
    )
}

fn source_is_empty(group: &Value) -> Result<Value> {
    let heading = regex::Regex::new(r"^#{1,6}(?:\s|$)")?;
    let mut sections = Vec::new();
    for section in group["sections"]
        .as_array()
        .context("missing source sections")?
    {
        check_file(section, "group source")?;
        let source = util::read_text(field(section, "path")?)?;
        if let Some((index, line)) = source
            .lines()
            .enumerate()
            .find(|(_, line)| !line.trim().is_empty() && !heading.is_match(line.trim()))
        {
            bail!(
                "no_cards source enumeration contains authored body text at {}:{}: {}",
                field(section, "path")?,
                index + 1,
                line.chars().take(120).collect::<String>()
            );
        }
        sections.push(section.clone());
    }
    if sections.is_empty() {
        bail!("no_cards has no source enumeration");
    }
    Ok(json!({"reason":"source_has_no_body","sections":sections,"body_lines":0}))
}

fn verify_group_consumers(p: &Prepared, cards: &[Value], check: &Value) -> Result<()> {
    use std::collections::{HashMap, HashSet};
    let declared = target(&p.contract)?;
    let models = declared["enabled"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "models");
    let book = field(&p.contract, "citation_key")?.to_string();
    let literature = HashSet::from([book.clone()]);
    let anchors = HashMap::from([(book, p.contract["groups"].as_array().unwrap().len())]);
    for fault in check["faults"]
        .as_array()
        .context("missing checked group faults")?
    {
        let at = fault
            .as_str()
            .and_then(|f| f.split_whitespace().next())
            .context("incomplete group fault address")?;
        let card = cards
            .iter()
            .find(|c| c["address"] == at)
            .context("group fault has no owned Card")?;
        if card["type"] == "model" {
            if !models {
                bail!("group quality fault has no enabled Model consumer: {fault}");
            }
        } else if !["idea", "anchor", "event", "finding"]
            .contains(&zt_check::graph::category(card, &literature, &anchors).as_str())
        {
            bail!("group quality fault has no writable Card consumer: {fault}");
        }
    }
    Ok(())
}

fn completion_fault(p: &Prepared, point: &str) -> Result<()> {
    if std::env::var("ZT_LEDGER_COMPLETE_FAULT").ok().as_deref() == Some(point)
        && p.out
            .starts_with(std::fs::canonicalize(std::env::temp_dir())?)
        && p.out.parent().and_then(Path::file_name).is_some_and(|n| {
            [
                "ledger-group-finish-test-",
                "ledger-link-test-",
                "ledger-cards-test-",
                "ledger-terms-test-",
                "ledger-prepare-recovery-test-",
                "ledger-publication-test-",
                "ledger-event-finding-test-",
                "model-finish-test-",
                "model-dependencies-test-",
                "explain-finish-test-",
                "ledger-resume-test-",
                "ledger-link-gaps-test-",
            ]
            .iter()
            .any(|prefix| n.to_string_lossy().starts_with(prefix))
        })
    {
        bail!("injected completion fault: {point}");
    }
    Ok(())
}

fn accept_plan(path: &Path, plan: &Value) -> Result<Value> {
    let record = if path.exists() {
        let (old, record) = json_artifact(path)?;
        if old != *plan {
            bail!(
                "accepted_plan_changed: keep the immutable accepted calls; a correction needs a new plan revision"
            );
        }
        record
    } else {
        let bytes = serde_json::to_string_pretty(plan)?.into_bytes();
        let mut temp = tempfile::NamedTempFile::new_in(
            path.parent().context("accepted plan has no directory")?,
        )?;
        std::io::Write::write_all(&mut temp, &bytes)?;
        temp.as_file().sync_all()?;
        temp.persist_noclobber(path).map_err(|e| e.error)?;
        json!({"path":std::fs::canonicalize(path)?.to_string_lossy(),"sha256":canon::sha256_hex(&bytes)})
    };
    check_file(&record, "accepted plan")?;
    Ok(record)
}

fn output(
    tx: &Transaction,
    unit: &str,
    key: &str,
    kind: &str,
    locator: &str,
    fingerprint: &str,
    data: &Value,
) -> Result<()> {
    tx.execute("INSERT INTO outputs(unit_id,output_key,kind,locator,fingerprint,data_json) VALUES(?1,?2,?3,?4,?5,?6)",params![unit,key,kind,locator,fingerprint,canon::canonical_json(data)])?;
    Ok(())
}

fn verify_archive_receipts(
    zt: &mut Zt,
    plan: &Value,
    keys: &Value,
) -> Result<std::collections::BTreeSet<String>> {
    let calls = plan["calls"]
        .as_array()
        .context("accepted plan is missing calls")?;
    let keys = keys
        .as_array()
        .context("completion is missing receipt enumeration")?;
    if calls.len() != keys.len() {
        bail!("incomplete archive receipt enumeration");
    }
    if calls.is_empty() {
        return Ok(std::collections::BTreeSet::new());
    }
    let prefix = keys[0]
        .as_str()
        .and_then(|key| key.rsplit_once('/'))
        .map(|v| v.0)
        .context("invalid archive receipt key")?;
    if let Some(expected) = plan["operation_prefix"].as_str() {
        if expected != prefix {
            bail!("accepted receipt prefix changed");
        }
    }
    let preview = crate::calls::preview_identified(zt, calls, prefix, None)
        .context("archive receipt verification failed")?;
    let results = preview["results"]
        .as_array()
        .context("incomplete archive receipt readback")?;
    for (index, (result, key)) in results.iter().zip(keys).enumerate() {
        if key != &format!("{prefix}/item-{index:06}")
            || result["op_key"] != *key
            || result["replayed"] != true
        {
            bail!("missing retained archive receipt: {key}");
        }
    }
    if zt.archive_view()?["archive_revision"] != preview["archive_revision"] {
        bail!("read_view_changed: archive receipt verification");
    }
    results
        .iter()
        .map(|r| field(&r["result"], "address").map(str::to_string))
        .collect()
}

/// Setup creates only the frozen Literature root and its exact Direct anchors.
/// No address allocation, edit, alias, or caller-selected replacement is allowed.
fn setup_calls(p: &Prepared) -> Result<Vec<Value>> {
    let calls = bound_json(&p.contract["artifacts"]["setup_calls"], "setup")?;
    let calls = calls.as_array().context("setup must be a call array")?;
    let groups = p.contract["groups"].as_array().unwrap();
    let key = field(&p.contract, "citation_key")?;
    if calls.len() != groups.len() + 1 {
        bail!("setup does not enumerate every prepared anchor");
    }
    for (i, call) in calls.iter().enumerate() {
        let a = &call["arguments"];
        let allowed: &[&str] = if i == 0 {
            &["workspace", "bibtex", "body"]
        } else {
            &["at", "kind", "title", "body"]
        };
        if call.as_object().is_none_or(|v| v.len() != 2)
            || a.as_object().is_none_or(|v| {
                v.len() != allowed.len() || v.keys().any(|k| !allowed.contains(&k.as_str()))
            })
        {
            bail!("invalid retained setup operation {i}");
        }
        if i == 0 {
            if call["tool"] != "create_literature"
                || a["workspace"] != "agent"
                || canon::sha256_hex(field(a, "bibtex")?.as_bytes())
                    != p.contract["options"]["bibtex_sha256"]
            {
                bail!("setup Literature metadata mismatch");
            }
        } else if call["tool"] != "create_successor"
            || a["kind"] != "direct"
            || a["at"]
                != if i == 1 {
                    key.to_string()
                } else {
                    format!("{key}/{}", i - 1)
                }
        {
            bail!("setup anchor target mismatch at group {i}");
        }
        field(a, "body")?;
        if i > 0 {
            field(a, "title")?;
        }
    }
    Ok(calls.clone())
}

/// The prerequisites a book unit may hold beyond its frozen ones: the child units a digest group's
/// completion registered for it (#69/#70: cards-book waits on cards/gNNN, link-check on link/gNNN).
/// `children` holds (unit id, layer) of every unit a digest unit created.
fn registration_deps_ok(
    id: &str,
    needs: &std::collections::BTreeSet<(String, String)>,
    actual: &std::collections::BTreeSet<(String, String)>,
    children: &std::collections::BTreeSet<(String, String)>,
) -> bool {
    let child_layer = match id {
        "cards-book/book@1" => Some("cards"),
        "link-check/book@1" => Some("link"),
        _ => None,
    };
    needs.is_subset(actual)
        && actual.difference(needs).all(|(needed, condition)| {
            condition == "settled"
                && child_layer.is_some_and(|layer| children.iter().any(|(child, l)| child == needed && l == layer))
        })
}

/// `ledger gate -Brief`: the layer decision a driver relays (identity, sealed/settled/complete, the counts
/// without the per-layer table, the layer's own unresolved units and coverage faults), not the full status.
fn gate_brief(full: &Value, layer: &str) -> Value {
    let mut brief = serde_json::Map::new();
    for key in ["schema_version", "run_id", "invocation", "layer", "sealed", "settled", "complete"] {
        if let Some(value) = full.get(key) {
            brief.insert(key.to_string(), value.clone());
        }
    }
    let layer_counts = full["counts"].clone();
    let mut counts = full["counts"].clone();
    if let Some(object) = counts.as_object_mut() {
        object.remove("per_layer");
    }
    brief.insert("counts".to_string(), counts);
    let prefix = format!("{layer}/");
    let unresolved: Vec<Value> = full["unresolved"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|unit| layer.is_empty() || unit.as_str().is_some_and(|id| id.starts_with(&prefix)))
        .cloned()
        .collect();
    brief.insert("unresolved".to_string(), json!(unresolved));
    let own_faults = layer_faults(full["coverage_faults"].as_array().map(Vec::as_slice).unwrap_or(&[]), layer);
    // A gate for one layer decides `complete` from that layer's own facts: settled, every unit done or
    // no-op, none of its own unresolved units or coverage faults. The run-wide `complete` also waits on
    // later layers (a `cards-book` publication that does not exist yet), which held every layer's gate
    // open (grossman2002integration trial 4: events 8/8 done reported active). An empty layer keeps the
    // run-wide decision.
    if !layer.is_empty() {
        let zero = |key: &str| layer_counts[key].as_i64().unwrap_or(0) == 0;
        let own_complete = full["settled"] == json!(true)
            && full["sealed"] == json!(true)
            && ["open", "failed", "blocked", "refused", "ready", "claimed", "retry", "registered"].iter().all(|key| zero(key))
            && unresolved.is_empty()
            && own_faults.is_empty();
        brief.insert("complete".to_string(), json!(own_complete));
    }
    brief.insert("coverage_faults".to_string(), json!(own_faults));
    Value::Object(brief)
}

/// The coverage faults that belong to one layer (a fault names its unit), as a brief gate lists them.
fn layer_faults(faults: &[Value], layer: &str) -> Vec<Value> {
    if layer.is_empty() {
        return faults.to_vec();
    }
    let prefix = format!("{layer}/");
    faults
        .iter()
        .filter(|fault| fault["unit"].as_str().is_some_and(|unit| unit.starts_with(&prefix)))
        .cloned()
        .collect()
}

fn preparation_registration(conn: &Connection, contract: &Value, hash: &str) -> Result<()> {
    use std::collections::{BTreeMap, BTreeSet};
    let target = target(contract)?;
    let enabled = |name: &str| {
        target["enabled"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == name)
    };
    let root = "prepare/book@1";
    let terms = "terms/book@1";
    let mut units = BTreeMap::from([(
        root.to_string(),
        (
            Some(json!({"preparation_hash":hash})),
            Vec::<(String, String)>::new(),
        ),
    )]);
    if enabled("terms") {
        units.insert(
            terms.into(),
            (
                Some(json!({"preparation_hash":hash})),
                vec![(root.into(), "success".into())],
            ),
        );
    }
    let groups = contract["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            let id = format!("digest/g{:03}@1", g["n"].as_u64().unwrap());
            let mut needs = vec![(root.to_string(), "success".to_string())];
            if enabled("terms") {
                needs.push((terms.to_string(), "success".to_string()));
            }
            units.insert(id.clone(), (Some(g.clone()), needs));
            id
        })
        .collect::<Vec<_>>();
    // An aggregate publication keeps only its binding here: its contributing
    // manifest gains immutable revisions, which are verified where they register.
    // The mathematics theory revision is published after link-check, and Organize
    // is the tail publication after it, so both name that prerequisite here (R241).
    for layer in BOOK.iter().copied().filter(|l| enabled(l)) {
        let mut needs = groups
            .iter()
            .map(|id| (id.clone(), "settled".into()))
            .collect::<Vec<_>>();
        if layer == "organize" && enabled("theory") {
            needs.push(("theory/book@1".into(), "settled".into()));
        }
        units.insert(format!("{layer}/book@1"), (None, needs));
    }
    if contract["variant"] == "math" {
        for layer in MATH_BOOK.iter().copied().filter(|l| enabled(l)) {
            if layer == "theory" {
                let mut needs = groups
                    .iter()
                    .map(|id| (id.clone(), "settled".into()))
                    .collect::<Vec<_>>();
                if enabled("link-check") {
                    needs.push(("link-check/book@1".into(), "settled".into()));
                }
                units.insert(format!("{layer}/book@1"), (None, needs));
                continue;
            }
            let mut needs = vec![(root.to_string(), "success".to_string())];
            if let Some(previous) = math_prerequisite(layer) {
                // The mathematics variant's declared stage order: each layer consumes the
                // checked result of the layer before it, and the final check reads the
                // audit revision rather than rebuilding the domain checks (R238, R241).
                needs.push((format!("{previous}/book@1"), "success".to_string()));
            }
            units.insert(
                format!("{layer}/book@1"),
                (Some(json!({"preparation_hash":hash})), needs),
            );
        }
    }
    // The child units each digest group's completion registered (#69/#70).
    let digest_children: BTreeSet<(String, String)> = conn
        .prepare("SELECT id,layer FROM units WHERE created_by LIKE 'digest/%'")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<BTreeSet<_>>>()?;
    for (id, (inputs, needs)) in &units {
        let row: Option<(String, String, Option<String>)> = conn
            .query_row(
                "SELECT input_hash,inputs_json,created_by FROM units WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((stored, raw, by)) = row else {
            bail!("preparation registration: missing unit {id}");
        };
        let parsed = canon::parse_json(&raw)?;
        if stored != canon::hash_value(&parsed)
            || by.as_deref() != if id == root { None } else { Some(root) }
        {
            bail!("preparation registration: changed unit {id}");
        }
        match inputs {
            Some(inputs) => {
                if stored != canon::hash_value(inputs) || parsed != *inputs {
                    bail!("preparation registration: changed unit {id}");
                }
            }
            None => {
                let (layer, _) = id
                    .split_once('/')
                    .context("invalid publication unit id")?;
                verify_publication_input(id, layer, "book", hash, &parsed)?;
            }
        }
        let actual = conn
            .prepare("SELECT needs_id,condition FROM deps WHERE unit_id=?1")?
            .query_map([id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<BTreeSet<_>>>()?;
        let frozen: BTreeSet<(String, String)> = needs.iter().cloned().collect();
        if !registration_deps_ok(id, &frozen, &actual, &digest_children) {
            bail!("preparation registration: changed prerequisites for {id}");
        }
    }
    let children = conn
        .prepare("SELECT id FROM units WHERE created_by=?1")?
        .query_map([root], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<BTreeSet<_>>>()?;
    if children
        != units
            .keys()
            .filter(|id| id.as_str() != root)
            .cloned()
            .collect()
    {
        bail!("preparation registration: changed root enumeration");
    }
    Ok(())
}

fn preparation_outputs(conn: &Connection, result: &Value) -> Result<()> {
    use std::collections::{BTreeMap, BTreeSet};
    // R413: an adopted preparation declares exactly the outputs adoption registered. It holds no
    // historic receipt enumeration, so the invariant verifies its own declaration instead of
    // pretending a prepare command ran.
    if is_adoption(result) {
        if result["unit"] != "prepare/book@1"
            || result["preparation_hash"] != meta(conn, "preparation_hash")?
        {
            bail!("incomplete adopted preparation evidence");
        }
        let expected = adoption_output_keys(result)?;
        let actual = conn
            .prepare("SELECT output_key FROM outputs WHERE unit_id='prepare/book@1'")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<BTreeSet<_>>>()?;
        if expected != actual {
            bail!("adopted preparation outputs mismatch");
        }
        return Ok(());
    }
    let contract = meta(conn, "preparation")?;
    let run = meta(conn, "run_id")?;
    let expected_addresses = std::iter::once(contract["citation_key"].clone())
        .chain(
            contract["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|g| g["anchor"].clone()),
        )
        .collect::<Vec<_>>();
    let expected_receipts = (0..expected_addresses.len())
        .map(|i| {
            json!(format!(
                "{}/prepare-book/1/setup/item-{i:06}",
                run.as_str().unwrap()
            ))
        })
        .collect::<Vec<_>>();
    if result["schema_version"] != 1
        || result["kind"] != "prepare_completion"
        || result["unit"] != "prepare/book@1"
        || result["preparation_hash"] != meta(conn, "preparation_hash")?
        || result["root"] != expected_addresses[0]
        || result["anchors"] != json!(&expected_addresses[1..])
        || result["receipt_keys"] != json!(expected_receipts)
        || result["card_fingerprints"]
            .as_object()
            .is_none_or(|v| v.len() != expected_addresses.len())
    {
        bail!("preparation output registration: invalid typed completion");
    }
    let mut expected = BTreeMap::new();
    for name in ["plan", "completion"] {
        expected.insert(
            name.to_string(),
            (
                "file".to_string(),
                field(&result[name], "path")?.to_string(),
                field(&result[name], "sha256")?.to_string(),
            ),
        );
    }
    for at in &expected_addresses {
        let at = at.as_str().unwrap();
        expected.insert(
            format!("card/{at}"),
            (
                "card".into(),
                at.to_string(),
                field(&result["card_fingerprints"], at)?.to_string(),
            ),
        );
    }
    let rows = conn.prepare("SELECT output_key,kind,locator,fingerprint,data_json FROM outputs WHERE unit_id='prepare/book@1'")?.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut receipt_addresses = BTreeSet::new();
    for (i, key) in expected_receipts.iter().enumerate() {
        let key = key.as_str().unwrap();
        let row = rows
            .iter()
            .find(|r| r.0 == format!("receipt/{key}"))
            .context("preparation output registration: missing receipt")?;
        let data = canon::parse_json(&row.4)?;
        let address = field(&data, "address")?;
        if data["address"] != expected_addresses[i]
            || data["after_fingerprint"] != result["card_fingerprints"][address]
        {
            bail!("preparation output registration: changed receipt result");
        }
        receipt_addresses.insert(address.to_string());
        expected.insert(
            format!("receipt/{key}"),
            (
                "archive_receipt".into(),
                key.to_string(),
                canon::hash_value(&data),
            ),
        );
    }
    let actual = rows
        .into_iter()
        .map(|(k, t, p, h, _)| (k, (t, p, h)))
        .collect::<BTreeMap<_, _>>();
    if actual != expected || receipt_addresses.len() != expected_addresses.len() {
        bail!("preparation output registration: missing or changed evidence");
    }
    Ok(())
}

/// Reconcile committed setup receipts before any unseen chunk is written.
/// Stop at the first missing chunk during recovery; its simulated writes roll
/// back. Complete readback uses these same ordinary 64-item/1-MiB bounds.
fn setup_readback(
    zt: &mut Zt,
    calls: &[Value],
    prefix: &str,
    complete: bool,
) -> Result<Vec<Value>> {
    let before = zt.archive_view()?;
    let mut receipts = Vec::new();
    while receipts.len() < calls.len() {
        let start = receipts.len();
        let mut items = Vec::new();
        let mut bytes = 0;
        for (i, call) in calls.iter().enumerate().skip(start).take(64) {
            let size: usize = ["title", "body", "bibtex"]
                .iter()
                .map(|k| call["arguments"][k].as_str().map(str::len).unwrap_or(0))
                .sum();
            if size > 1024 * 1024 {
                bail!("setup operation exceeds 1048576 authored bytes");
            }
            if bytes + size > 1024 * 1024 {
                break;
            }
            bytes += size;
            items.push(json!({"op_key":format!("{prefix}/item-{i:06}"),"client_ref":i.to_string(),"tool":call["tool"],"arguments":call["arguments"]}));
        }
        let reply = zt.call("apply_batch", json!({"preview":true,"items":items}))?;
        if reply["preview"] != true
            || reply["complete"] != true
            || reply["archive_revision"] != before["archive_revision"]
        {
            bail!("read_view_changed: setup receipt reconciliation");
        }
        let results = reply["results"]
            .as_array()
            .context("missing setup receipt readback")?;
        if results.len() != items.len() {
            bail!("incomplete setup receipt readback");
        }
        let mut missing = false;
        for (offset, result) in results.iter().enumerate() {
            let i = start + offset;
            if result["op_key"] != format!("{prefix}/item-{i:06}")
                || result["client_ref"] != i.to_string()
                || !result["replayed"].is_boolean()
            {
                bail!("invalid setup receipt identity");
            }
            missing |= result["replayed"] != true;
            receipts.push(result.clone());
        }
        if missing {
            if complete {
                bail!("missing retained setup receipt");
            }
            break;
        }
    }
    if zt.archive_view()? != before {
        bail!("read_view_changed: setup receipt reconciliation");
    }
    Ok(receipts)
}

fn prepare_finish(p: &Prepared, zt: &mut Zt, token: i64, path: &Path) -> Result<Value> {
    let unit = "prepare/book@1";
    let _guard = unit_guard(p, unit)?;
    let (claimed, progress, run, done) = finish_claim(p, unit, token)?;
    preparation_registration(
        &connect(&p.out.join("ledger.sqlite3"), false)?,
        &p.contract,
        &p.hash,
    )?;
    let record = &progress["prepare_setup"]["plan"];
    if std::fs::canonicalize(path)? != Path::new(field(record, "path")?) {
        bail!("preparation plan locator mismatch");
    }
    let plan = bound_json(record, "preparation plan")?;
    let prefix = format!(
        "{}/prepare-book/1/setup",
        run.as_str().context("invalid run identity")?
    );
    if plan["schema_version"] != 1
        || plan["kind"] != "prepare_setup"
        || plan["unit"] != unit
        || plan["token"] != token
        || plan["preparation_hash"] != p.hash
        || plan["operation_prefix"] != prefix
        || plan["setup"] != p.contract["artifacts"]["setup_calls"]
    {
        bail!("preparation plan identity mismatch");
    }
    let calls = setup_calls(p)?;
    // A mismatched BibTeX key must fail before creating even the root.
    let root = crate::calls::preview_identified(zt, &calls[..1], &prefix, None)?;
    if root["results"][0]["result"]["address"] != p.contract["citation_key"] {
        bail!("setup BibTeX Citation key differs from prepared Citation key");
    }
    let reconciled = setup_readback(zt, &calls, &prefix, done)?;
    bound_json(record, "preparation plan")?;
    Prepared::read_mode(&p.out.to_string_lossy(), zt, false)?;
    let receipts = if done {
        reconciled
    } else {
        renew(p, unit, token)?;
        crate::calls::invoke_identified(zt, &calls, &prefix, None)?
    };
    let key = field(&p.contract, "citation_key")?;
    let addresses = std::iter::once(key.to_string())
        .chain(
            p.contract["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|g| g["anchor"].as_str().unwrap().to_string()),
        )
        .collect::<Vec<_>>();
    if receipts.len() != addresses.len()
        || receipts
            .iter()
            .zip(&addresses)
            .any(|(r, a)| r["result"]["address"] != *a)
    {
        bail!("persisted setup addresses differ from prepared target");
    }
    Prepared::read(&p.out.to_string_lossy(), zt)?;
    let read = zt.cards(&addresses, "text")?;
    let mut fingerprints = serde_json::Map::new();
    for (card, receipt) in read.cards.iter().zip(&receipts) {
        let fp = crate::calls::authored_fingerprint(card);
        if fp != field(&receipt["result"], "after_fingerprint")? {
            bail!("prepared Card differs from its setup receipt");
        }
        fingerprints.insert(field(card, "address")?.to_string(), json!(fp));
    }
    if done {
        let conn = writer(p)?;
        let result: String =
            conn.query_row("SELECT result_json FROM units WHERE id=?1", [unit], |r| {
                r.get(0)
            })?;
        let mut result: Value = serde_json::from_str(&result)?;
        preparation_outputs(&conn, &result)?;
        check_file(&result["completion"], "preparation completion")?;
        if result["plan"] != *record || result["card_fingerprints"] != json!(fingerprints) {
            bail!("preparation completion differs from verified outputs");
        }
        result["reused"] = json!(true);
        return Ok(result);
    }
    // The manifest may gain actual setup lines; the run contract and all inputs
    // stay byte-bound. A crash here is recovered from archive receipts.
    let mut manifest = util::read_json(p.out.join("manifest.json"))?;
    manifest["created"] = json!(
        addresses
            .iter()
            .enumerate()
            .map(|(i, a)| format!("{i} ok {a}"))
            .collect::<Vec<_>>()
    );
    publish(
        &p.out.join("manifest.json"),
        &serde_json::to_string_pretty(&manifest)?,
    )?;
    let receipt_keys = receipts
        .iter()
        .map(|r| r["op_key"].clone())
        .collect::<Vec<_>>();
    let mut result = json!({"schema_version":1,"kind":"prepare_completion","unit":unit,"token":token,"state":"done","complete":true,"reused":false,"input_hash":claimed.input_hash,"preparation_hash":p.hash,"plan":record,"root":key,"anchors":&addresses[1..],"card_fingerprints":fingerprints,"receipt_keys":receipt_keys});
    let completion = publish(
        &p.out.join("prepare-completion.json"),
        &serde_json::to_string_pretty(&result)?,
    )?;
    result["completion"] = completion.clone();
    bound_json(record, "preparation plan")?;
    Prepared::read(&p.out.to_string_lossy(), zt)?;
    let after = zt.archive_view()?;
    if after["archive_identity"] != p.contract["archive_identity"]
        || after["archive_revision"].as_u64() != Some(read.archive_revision)
    {
        bail!("read_view_changed: preparation completion");
    }
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, unit, token)?;
    preparation_registration(&tx, &p.contract, &p.hash)?;
    for (name, file) in [("plan", record), ("completion", &completion)] {
        output(
            &tx,
            unit,
            name,
            "file",
            field(file, "path")?,
            field(file, "sha256")?,
            file,
        )?;
    }
    for (at, fp) in fingerprints {
        output(
            &tx,
            unit,
            &format!("card/{at}"),
            "card",
            &at,
            fp.as_str().unwrap(),
            &json!({"purpose":"preparation"}),
        )?;
    }
    for receipt in receipts {
        let key = field(&receipt, "op_key")?;
        output(
            &tx,
            unit,
            &format!("receipt/{key}"),
            "archive_receipt",
            key,
            &canon::hash_value(&receipt["result"]),
            &receipt["result"],
        )?;
    }
    completion_fault(p, "after_outputs")?;
    tx.execute(
        "UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL WHERE id=?1",
        params![unit, canon::canonical_json(&result), canon::utc_now_o()],
    )?;
    preparation_outputs(&tx, &result)?;
    event(
        &tx,
        unit,
        Some(&claimed.owner),
        Some(token),
        "prepared",
        &json!({"anchors":addresses.len()-1,"completion":completion}),
    )?;
    reconcile(&tx, now_seconds())?;
    completion_fault(p, "before_commit")?;
    tx.commit()?;
    completion_fault(p, "after_commit")?;
    Ok(result)
}

fn verify_done_group(p: &Prepared, zt: &mut Zt, unit: &str, token: i64) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let raw: String = tx.query_row(
        "SELECT result_json FROM units WHERE id=?1 AND state='done' AND claim_token=?2",
        params![unit, token],
        |r| r.get(0),
    )?;
    let mut result: Value = serde_json::from_str(&raw)?;
    if emission_faults(&tx)?.iter().any(|fault| {
        fault["code"] != "producer_outputs_mismatch"
            && (fault["producer"] == unit || fault["unit"] == unit)
    }) {
        bail!(
            "incomplete group completion: registered child contracts differ from accepted enumeration"
        );
    }
    let rows=tx.prepare("SELECT output_key,kind,locator,fingerprint FROM outputs WHERE unit_id=?1 ORDER BY output_key")?.query_map([unit],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let children = tx
        .prepare("SELECT id FROM units WHERE created_by=?1 ORDER BY id")?
        .query_map([unit], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut expected = result["children"]
        .as_array()
        .context("incomplete group completion children")?
        .iter()
        .map(|c| c.as_str().context("invalid child id").map(str::to_string))
        .collect::<Result<Vec<_>>>()?;
    expected.sort();
    if children != expected {
        bail!("incomplete group completion: registered children differ from accepted enumeration");
    }
    if rows
        .iter()
        .map(|(key, _, _, _)| key.clone())
        .collect::<std::collections::BTreeSet<_>>()
        != completion_output_keys(&result)?
    {
        bail!("incomplete group completion: required output registration is missing or changed");
    }
    tx.commit()?;
    drop(conn);
    check_file(&result["draft"], "group completion evidence")?;
    let accepted = bound_json(&result["accepted"], "group completion evidence")?;
    if canon::hash_value(&accepted) != result["plan_hash"] {
        bail!("completed group plan hash changed");
    }
    let receipt_addresses = verify_archive_receipts(zt, &accepted, &result["receipt_keys"])?;
    if result["disposition"] != "no_cards"
        && receipt_addresses
            != result["owned"]
                .as_array()
                .context("missing owned set")?
                .iter()
                .map(|a| {
                    a.as_str()
                        .context("invalid owned address")
                        .map(str::to_string)
                })
                .collect::<Result<std::collections::BTreeSet<_>>>()?
    {
        bail!("completed group ownership differs from archive receipts");
    }
    let mut cards = std::collections::BTreeMap::new();
    for (_, kind, locator, fingerprint) in rows {
        match kind.as_str() {
            "file" => check_file(
                &json!({"path":locator,"sha256":fingerprint}),
                "group completion evidence",
            )?,
            "card" => {
                if result["card_fingerprints"][&locator] != fingerprint {
                    bail!("incomplete group completion: Card fingerprint registration changed");
                }
                cards.insert(locator, fingerprint);
            }
            "archive_receipt" => {}
            _ => bail!("unknown group completion evidence kind"),
        }
    }
    if cards.is_empty() {
        bail!("incomplete group completion Card outputs");
    }
    let read = zt.cards(&cards.keys().cloned().collect::<Vec<_>>(), "text")?;
    for card in &read.cards {
        if card["workspace"] != "agent"
            || cards.get(field(card, "address")?) != Some(&crate::calls::authored_fingerprint(card))
        {
            bail!("completed group output changed: {}", card["address"]);
        }
    }
    if zt.archive_view()?["archive_revision"].as_u64() != Some(read.archive_revision) {
        bail!("read_view_changed: group completion verification");
    }
    result["reused"] = json!(true);
    Ok(result)
}

fn group_finish(
    p: &Prepared,
    zt: &mut Zt,
    number: u64,
    token: i64,
    draft_path: &str,
    check_only: bool,
) -> Result<Value> {
    use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
    let unit = format!("digest/g{number:03}@1");
    finish_claim(p, &unit, token)?;
    let _guard = unit_guard(p, &unit)?;
    let (claimed, progress, run, done) = finish_claim(p, &unit, token)?;
    let group = p.contract["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["n"].as_u64() == Some(number))
        .context("prepared group does not exist")?;
    let (_, job) = read_group_job(p, &unit, token, &claimed.input_hash, &progress, &run)?;
    if claimed.input_hash != canon::hash_value(group) {
        bail!("group input hash mismatch");
    }
    let path = std::fs::canonicalize(draft_path).context("missing group draft JSON")?;
    if path != std::fs::canonicalize(field(&job, "draft")?)? || !path.starts_with(&p.out) {
        bail!("group draft binding path mismatch");
    }
    if std::fs::metadata(&path)?.len() > 16 * 1024 * 1024 {
        bail!("group draft exceeds 16 MiB encoded input");
    }
    let (draft, draft_record) = json_artifact(&path)?;
    if draft["schema_version"] != 1
        || draft["unit"] != unit
        || draft["token"] != token
        || draft["input_hash"] != job["input_hash"]
    {
        bail!("group draft binding mismatch: schema, unit, token or input hash");
    }
    if draft
        .as_object()
        .context("group draft must be an object")?
        .keys()
        .any(|k| {
            ![
                "schema_version",
                "unit",
                "token",
                "input_hash",
                "disposition",
                "calls",
            ]
            .contains(&k.as_str())
        })
    {
        bail!("unknown group draft field");
    }
    let disposition = field(&draft, "disposition")?;
    if !matches!(disposition, "cards" | "no_cards") {
        bail!("group draft disposition must be cards or no_cards");
    }
    let mut calls = draft["calls"]
        .as_array()
        .context("group draft calls must be an array")?
        .clone();
    let anchor = field(group, "anchor")?;
    // S06 (#103, D100/D104): a Card this group read in full may be continued only as a Side
    // successor. A Direct successor would extend the earlier line of thought, so the rule the
    // writer's helper enforces while it writes holds here too, where the group actually applies its
    // draft: the new Card is created, and nothing of the earlier Card, tree or chain becomes
    // writable.
    let beside: HashSet<String> = if done {
        HashSet::new()
    } else {
        let text = job["artifacts"]["place_beside"]["path"]
            .as_str()
            .filter(|path| !path.is_empty())
            .map(std::fs::read_to_string)
            .transpose()
            .context("missing place-beside file")?
            .unwrap_or_default();
        text.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect()
    };
    let mut source_notes = BTreeMap::new();
    for (i, call) in calls.iter_mut().enumerate() {
        if call.as_object().is_none_or(|v| {
            v.keys()
                .any(|k| !["tool", "arguments"].contains(&k.as_str()))
        }) {
            bail!("group call {i} must contain only tool and arguments");
        }
        let tool = call["tool"].as_str().unwrap_or_default();
        // docs/literature-only-digest-design.md: a group writes only the book's Literature tree.
        if let Some(why) = zt_check::mcp::scope::literature_refusal(tool, &call["arguments"]) {
            bail!("group call {i}: {why}");
        }
        let args = call["arguments"]
            .as_object()
            .context("group call arguments must be an object")?;
        let at = args.get("at").and_then(Value::as_str).unwrap_or_default();
        if tool == "create_literature" {
            bail!("group call cannot create another Literature root");
        }
        // docs/literature-only-digest-design.md: a group files its Cards in its own book's
        // Literature tree only; another book's tree is a Link target, never a destination.
        let book = field(&p.contract, "citation_key")?;
        if tool == "create_successor"
            && !at.starts_with("@@")
            && at.split('/').next().unwrap_or_default() != book
        {
            bail!(
                "group call {i}: a group files its Cards in the book's Literature tree `{book}`, not beside `{at}`; link that Card instead"
            );
        }
        if tool == "edit_card" {
            let earlier = at
                .strip_prefix("@@")
                .and_then(|n| n.parse::<usize>().ok())
                .is_some_and(|n| n < i);
            if at != anchor && !earlier {
                bail!("group edit {i} is outside the owned anchor and earlier accepted outputs");
            }
            if args.contains_key("owned_paragraph") {
                bail!(
                    "group drafts use content edits; receipt-owned Link paragraphs have their own checker"
                );
            }
        }
        if tool == "create_successor"
            && at == anchor
            && args.get("kind").and_then(Value::as_str) != Some("side")
        {
            bail!(
                "create a Side successor beneath the group anchor to preserve direct group positions"
            );
        }
        if tool == "create_successor"
            && at != anchor
            && beside.contains(at)
            && args.get("kind").and_then(Value::as_str) != Some("side")
        {
            bail!(
                "file a new Card beside `{at}` only as a Side successor; its Direct chain belongs to its own line of thought"
            );
        }
        if let Some(src) = call["arguments"].as_object_mut().unwrap().remove("src") {
            let src = src.as_str().context("group source note must be text")?;
            if src.contains(['\n', '\r', '\t']) {
                bail!("group source note must be one line");
            }
            source_notes.insert(i.to_string(), src.to_string());
        }
    }
    let no_cards = if disposition == "no_cards" {
        if !calls.is_empty() {
            bail!("no_cards requires no proposed writes");
        }
        source_is_empty(group)?
    } else {
        Value::Null
    };
    if disposition == "cards" && calls.is_empty() {
        bail!(
            "group coverage requires substantive owned Cards; an empty answer is not a completion"
        );
    }
    let plan = json!({"schema_version":1,"run_id":run,"unit":unit,"unit_input_hash":claimed.input_hash,"plan_revision":1,"disposition":disposition,"calls":calls,"source_notes":source_notes,"no_cards":no_cards});
    let accepted_path = p.out.join(format!("accepted-g{number:03}.json"));
    if accepted_path.exists() && canon::read_json(&accepted_path.to_string_lossy())? != plan {
        bail!("accepted_plan_changed: group calls differ from the immutable accepted plan");
    }
    if done {
        let mut result = verify_done_group(p, zt, &unit, token)?;
        if check_only {
            result["check_only"] = json!(true);
            result["applied"] = json!(false);
        }
        return Ok(result);
    }
    let prefix = format!(
        "{}/digest-g{number:03}/1/apply",
        run.as_str().context("missing run id")?
    );
    let preview = if calls.is_empty() {
        let read = zt.cards(&[anchor.to_string()], "text")?;
        json!({"results":[],"cards":read.cards,"archive_revision":read.archive_revision})
    } else {
        crate::calls::preview_identified(zt, &calls, &prefix, None)?
    };
    let cards = preview["cards"]
        .as_array()
        .context("incomplete group preview Cards")?;
    for (i, call) in calls
        .iter()
        .enumerate()
        .filter(|(_, c)| c["tool"] == "create_successor")
    {
        let at = call["arguments"]["at"].as_str().unwrap_or_default();
        let at = if let Some(n) = at.strip_prefix("@@").and_then(|n| n.parse::<usize>().ok()) {
            field(&preview["results"][n]["result"], "address")?
        } else {
            at
        };
        if at == anchor && call["arguments"]["kind"] != "side" {
            bail!(
                "create a Side successor beneath the group anchor, including references to its earlier edit"
            );
        }
        let address = field(&preview["results"][i]["result"], "address")?;
        if address.starts_with(&format!("{}/", field(&p.contract, "citation_key")?))
            && !address.starts_with(&format!("{anchor}|"))
        {
            bail!(
                "new group Cards must use the anchor's Side branch; direct Literature anchor locations are reserved"
            );
        }
    }
    let literature = HashSet::from([field(&p.contract, "citation_key")?.to_string()]);
    let anchors = HashMap::from([(
        field(&p.contract, "citation_key")?.to_string(),
        p.contract["groups"].as_array().unwrap().len(),
    )]);
    let owned = cards
        .iter()
        .map(|c| field(c, "address").map(str::to_string))
        .collect::<Result<BTreeSet<_>>>()?;
    let substantive_for = |cards: &[Value]| -> Result<Vec<String>> {
        cards
            .iter()
            .filter(|c| {
                c["address"] != anchor
                    && zt_check::graph::SUBSTANTIVE
                        .contains(&zt_check::graph::category(c, &literature, &anchors).as_str())
                    && c["body"].as_str().is_some_and(|s| !s.trim().is_empty())
            })
            .map(|c| field(c, "address").map(str::to_string))
            .collect::<Result<Vec<_>>>()
    };
    let substantive = substantive_for(cards)?;
    if disposition == "cards" && (!owned.contains(anchor) || substantive.is_empty()) {
        bail!("group coverage requires its checked anchor plus substantive owned Cards");
    }
    let self_check = group_quality(cards, p)?;
    verify_group_consumers(p, cards, &self_check)?;
    if zt.archive_view()?["archive_revision"] != preview["archive_revision"] {
        bail!("read_view_changed: group draft preview");
    }
    check_file(&draft_record, "group draft")?;
    if check_only {
        return Ok(
            json!({"schema_version":1,"unit":unit,"token":token,"input_hash":job["input_hash"],"check_only":true,"applied":false,"complete":true,"owned":owned,"substantive":substantive,"self_check":self_check,"no_cards":no_cards}),
        );
    }
    renew(p, &unit, token)?;
    let accepted = accept_plan(&accepted_path, &plan)?;
    let receipts = if calls.is_empty() {
        Vec::new()
    } else {
        crate::calls::invoke_identified(zt, &calls, &prefix, None)?
    };
    // Allocation can change between the rolled-back preview and the commit.
    // Receipt results are the authority for the actual owned set and locators.
    let owned = if receipts.is_empty() {
        BTreeSet::from([anchor.to_string()])
    } else {
        receipts
            .iter()
            .map(|r| field(&r["result"], "address").map(str::to_string))
            .collect::<Result<BTreeSet<_>>>()?
    };
    let read = zt.cards(&owned.iter().cloned().collect::<Vec<_>>(), "text")?;
    let substantive = substantive_for(&read.cards)?;
    if disposition == "cards" && (!owned.contains(anchor) || substantive.is_empty()) {
        bail!("persisted group is missing its anchor or substantive owned Cards");
    }
    let owned_path = p.out.join(format!("owned-{}.txt", field(group, "id")?));
    let owned_record = publish(
        &owned_path,
        &format!("{}\n", owned.iter().cloned().collect::<Vec<_>>().join("\n")),
    )?;
    let mut src = BTreeMap::new();
    let sections = group["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| field(s, "path"))
        .collect::<Result<Vec<_>>>()?
        .join("; ");
    for (i, receipt) in receipts.iter().enumerate() {
        src.insert(
            field(&receipt["result"], "address")?.to_string(),
            source_notes
                .get(&i.to_string())
                .map(|s| format!("{sections}; {s}"))
                .unwrap_or_else(|| sections.clone()),
        );
    }
    src.entry(anchor.to_string()).or_insert(sections);
    let source_record = publish(
        &p.out.join(format!("src-{}.txt", field(group, "id")?)),
        &src.iter()
            .map(|(a, s)| format!("{a}\t{s}\n"))
            .collect::<String>(),
    )?;
    let check = crate::check::Check {
        cards: owned.iter().cloned().collect::<Vec<_>>().join(","),
        book: field(&p.contract, "citation_key")?.to_string(),
        ..Default::default()
    };
    let self_check = checked_lines(check.run(zt)?, false)?;
    verify_group_consumers(p, &read.cards, &self_check)?;
    if zt.archive_view()?["archive_revision"].as_u64() != Some(read.archive_revision) {
        bail!("read_view_changed: persisted group check");
    }
    let mut expected = BTreeMap::new();
    for r in &receipts {
        expected.insert(
            field(&r["result"], "address")?.to_string(),
            field(&r["result"], "after_fingerprint")?.to_string(),
        );
    }
    for c in &read.cards {
        let at = field(c, "address")?;
        let fp = crate::calls::authored_fingerprint(c);
        if c["workspace"] != "agent"
            || (!calls.is_empty() && expected.get(at) != Some(&fp))
            || (calls.is_empty() && job["inputs"]["anchor"]["fingerprint"] != fp)
        {
            bail!("persisted group output changed at {at}");
        }
    }
    let check_record = publish(
        &p.out.join(format!("self-check-g{number:03}.json")),
        &serde_json::to_string_pretty(
            &json!({"schema_version":1,"unit":unit,"token":token,"input_hash":job["input_hash"],"plan_hash":canon::hash_value(&plan),"archive_revision":read.archive_revision,"owned":owned,"self_check":self_check}),
        )?,
    )?;
    let base = json!({"group":group,"producer":unit,"producer_input_hash":claimed.input_hash,"plan_hash":canon::hash_value(&plan),"owned":owned_record,"source":source_record,"self_check":check_record});
    let declared = target(&p.contract)?;
    let enabled = declared["enabled"].as_array().unwrap();
    let mut children = Vec::new();
    for layer in ["cards", "events", "findings", "link"] {
        if enabled.iter().any(|v| v == layer) {
            children.push(json!({"layer":layer,"key":format!("g{number:03}"),"inputs":base}));
        }
    }
    let mut model_consumers = BTreeMap::new();
    let mut model_headers = BTreeSet::new();
    if enabled.iter().any(|v| v == "models") {
        for (i, r) in receipts.iter().enumerate() {
            let at = field(&r["result"], "address")?;
            if read
                .cards
                .iter()
                .any(|c| c["address"] == at && c["type"] == "model")
            {
                let model = zt.call("get_model", json!({"at":at}))?;
                let header = field(&model["head"], "address")
                    .context("incomplete Model candidate header")?;
                let consumer = if model_headers.insert(header.to_string()) {
                    let key = format!("g{number:03}-m{i:04}");
                    let consumer = format!("models/{key}@1");
                    let head = zt.cards(&[header.to_string()], "text")?;
                    children.push(json!({"layer":"models","key":key,"inputs":{"producer":base,"header":header,"fingerprint":crate::calls::authored_fingerprint(&head.cards[0])}}));
                    consumer
                } else {
                    model_consumers
                        .get(header)
                        .cloned()
                        .context("missing Model consumer binding")?
                };
                model_consumers.insert(header.to_string(), consumer.clone());
                model_consumers.insert(at.to_string(), consumer.clone());
                for part in model["parts"]
                    .as_array()
                    .context("incomplete Model candidate parts")?
                {
                    model_consumers.insert(field(part, "address")?.to_string(), consumer.clone());
                }
            }
        }
    }
    let child_ids = children
        .iter()
        .map(|c| {
            format!(
                "{}/{}@1",
                c["layer"].as_str().unwrap(),
                c["key"].as_str().unwrap()
            )
        })
        .collect::<Vec<_>>();
    let emitted=children.iter().zip(&child_ids).map(|(child,id)|{
        let mut dependencies=vec![json!({"unit":id,"needs":unit,"condition":"success"})];
        if child["layer"]=="link" {dependencies.push(json!({"unit":id,"needs":format!("cards/g{number:03}@1"),"condition":"success"}));}
        let book=match child["layer"].as_str(){Some("cards")=>Some("cards-book"),Some("link")=>Some("link-check"),_=>None};
        if let Some(layer)=book {dependencies.push(json!({"unit":format!("{layer}/book@1"),"needs":id,"condition":"settled"}));}
        json!({"id":id,"input_hash":canon::hash_value(&child["inputs"]),"dependencies":dependencies})
    }).collect::<Vec<_>>();
    let deferrals=self_check["faults"].as_array().unwrap().iter().map(|fault| {
        let text=fault.as_str().unwrap();let at=text.split_whitespace().next().unwrap_or_default();
        let model=model_consumers.get(at).cloned();
        json!({"fault":fault,"consumer":model.unwrap_or_else(||format!("cards/g{number:03}@1"))})
    }).collect::<Vec<_>>();
    let trees = read
        .cards
        .iter()
        .map(|c| field(c, "tree").map(str::to_string))
        .collect::<Result<BTreeSet<_>>>()?;
    let card_fingerprints = read
        .cards
        .iter()
        .map(|c| {
            Ok((
                field(c, "address")?.to_string(),
                crate::calls::authored_fingerprint(c),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let receipt_keys = receipts
        .iter()
        .map(|r| field(r, "op_key").map(str::to_string))
        .collect::<Result<Vec<_>>>()?;
    let result = json!({"schema_version":1,"kind":"digest_completion","unit":unit,"token":token,"state":"done","complete":true,"applied":!calls.is_empty(),"reused":false,"input_hash":job["input_hash"],"unit_input_hash":claimed.input_hash,"plan_hash":canon::hash_value(&plan),"draft":draft_record,"disposition":if disposition=="no_cards"{"no_cards"}else if deferrals.is_empty(){"verified"}else{"deferred"},"no_cards":no_cards,"accepted":accepted,"owned":owned,"substantive":substantive,"trees":trees,"self_check":self_check,"deferred":deferrals,"children":child_ids,"emitted":emitted,"card_fingerprints":card_fingerprints,"receipt_keys":receipt_keys,"files":{"owned":owned_record,"source":source_record,"self_check":check_record}});
    let result_record = publish(
        &p.out.join(format!("completion-g{number:03}.json")),
        &serde_json::to_string_pretty(&result)?,
    )?;
    for record in [
        draft_record.clone(),
        accepted.clone(),
        owned_record.clone(),
        source_record.clone(),
        check_record.clone(),
        result_record.clone(),
    ] {
        check_file(&record, "group completion evidence")?;
    }
    let final_view = zt.archive_view()?;
    if final_view["archive_identity"] != p.contract["archive_identity"]
        || final_view["archive_revision"].as_u64() != Some(read.archive_revision)
    {
        bail!("read_view_changed: group completion evidence changed before ledger commit");
    }
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, &unit, token)?;
    for (key, record) in [
        ("accepted", &accepted),
        ("owned", &owned_record),
        ("source", &source_record),
        ("self_check", &check_record),
        ("completion", &result_record),
    ] {
        output(
            &tx,
            &unit,
            key,
            "file",
            field(record, "path")?,
            field(record, "sha256")?,
            record,
        )?;
    }
    for c in &read.cards {
        let at = field(c, "address")?;
        output(
            &tx,
            &unit,
            &format!("card/{at}"),
            "card",
            at,
            &crate::calls::authored_fingerprint(c),
            &json!({"tree":c["tree"],"type":c["type"],"facet":c["facet"]}),
        )?;
    }
    for receipt in &receipts {
        let key = field(receipt, "op_key")?;
        output(
            &tx,
            &unit,
            &format!("receipt/{key}"),
            "archive_receipt",
            key,
            &canon::hash_value(&receipt["result"]),
            &receipt["result"],
        )?;
    }
    completion_fault(p, "after_outputs")?;
    completion_fault(p, "before_children")?;
    for child in &children {
        let id = register(
            &tx,
            field(child, "layer")?,
            field(child, "key")?,
            1,
            &child["inputs"],
            Some(&unit),
            false,
        )?;
        dependency(&tx, &id, &unit, "success")?;
        if child["layer"] == "link" {
            dependency(&tx, &id, &format!("cards/g{number:03}@1"), "success")?;
        }
        let book = match field(child, "layer")? {
            "cards" => Some("cards-book"),
            "link" => Some("link-check"),
            _ => None,
        };
        if let Some(layer) = book {
            dependency(&tx, &format!("{layer}/book@1"), &id, "settled")?;
        }
        event(
            &tx,
            &id,
            Some(&claimed.owner),
            Some(token),
            "registered",
            &json!({"producer":unit,"input_hash":canon::hash_value(&child["inputs"])}),
        )?;
    }
    completion_fault(p, "after_children")?;
    for deferred in &deferrals {
        if !child_ids.iter().any(|id| deferred["consumer"] == *id) {
            bail!("hard group fault has no registered consumer");
        }
    }
    tx.execute("UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",params![unit,canon::canonical_json(&result),canon::utc_now_o()])?;
    event(
        &tx,
        &unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"disposition":result["disposition"],"children":child_ids,"plan_hash":result["plan_hash"],"completion":result_record,"touched_trees":trees}),
    )?;
    // The last group to finish closes the child enumerations its children belong to,
    // in the same transaction that makes it terminal.
    close_enumerations(&tx)?;
    completion_fault(p, "before_commit")?;
    tx.commit()?;
    completion_fault(p, "after_commit")?;
    Ok(result)
}

pub fn group_finish_cli(argv: &[String]) -> Result<i32> {
    let parsed = (|| -> Result<(String, u64, i64, String, bool)> {
        let a = Args::parse(argv, &["Out", "Group", "Token", "Draft"], &["CheckOnly"])?;
        if !a.positional.is_empty() || a.required("Out")?.trim().is_empty() {
            bail!("explicit -Out is required");
        }
        let group = a
            .required("Group")?
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .context("-Group must be positive")?;
        let token = a
            .required("Token")?
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .context("-Token must be positive")?;
        Ok((
            a.str("Out"),
            group,
            token,
            a.required("Draft")?,
            a.flag("CheckOnly"),
        ))
    })();
    let (out, group, token, draft, check_only) = match parsed {
        Ok(v) => v,
        Err(e) => {
            eprintln!("zt-digest group-finish: {e}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let result = group_finish(&p, &mut zt, group, token, &draft, check_only)?;
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}

/// R222: a Link unit is complete only when every owned in-scope Card carries the
/// required kinded Link or a checked disposition, the receipt-owned paragraphs
/// still exist, and the Link SELF-CHECK passes over the persisted Cards. The unit
/// enumerates its scope from the group's registered owned manifest, never from a
/// caller-supplied address list.
const LINK_RULES: &str = "L1,L2";
const LINK_MAX_CALLS: usize = 64;
const LINK_MAX_BYTES: usize = 1024 * 1024;

fn link_unit(number: u64) -> String {
    format!("link/g{number:03}@1")
}

/// A producer-registered unit's frozen inputs; re-hashing them proves the
/// enumeration below is the accepted one.
fn frozen_inputs(p: &Prepared, unit: &str) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let raw: String = conn
        .query_row("SELECT inputs_json FROM units WHERE id=?1", [unit], |r| {
            r.get(0)
        })
        .with_context(|| format!("unknown unit {unit}"))?;
    Ok(serde_json::from_str(&raw)?)
}

/// The exact in-scope set from the group's published owned manifest, re-hashed here
/// so a changed or shortened file cannot silently shrink a child obligation.
fn frozen_owned(p: &Prepared, inputs: &Value, label: &str) -> Result<Vec<String>> {
    let record = &inputs["owned"];
    let path = Path::new(field(record, "path")?);
    // A card split appends to the manifest after it was frozen (`card::add_owned`): the frozen lines must still
    // lead the file unchanged; the split Cards after them join the scope (grossman2002integration trial 19).
    if let Err(error) = check_file(record, &format!("{label} owned manifest")) {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if !frozen_prefix_holds(&text, field(record, "sha256")?) {
            return Err(error);
        }
    }
    if !path.starts_with(&p.out) {
        bail!("{label} owned manifest escaped the prepared output directory");
    }
    let text = util::read_text(path)?;
    let mut owned = std::collections::BTreeSet::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.len() > 200 || line.chars().any(char::is_control) {
            bail!("{label} owned manifest holds an unreadable Card address");
        }
        if !owned.insert(line.to_string()) {
            bail!("{label} owned manifest names `{line}` twice");
        }
    }
    if owned.is_empty() {
        bail!("{label} owned manifest enumerates no Card");
    }
    Ok(owned.into_iter().collect())
}

/// Whether the text's leading whole lines hash to `sha256`: the frozen manifest, possibly followed by appended lines.
fn frozen_prefix_holds(text: &str, sha256: &str) -> bool {
    let mut end = 0;
    loop {
        if canon::sha256_hex(text[..end].as_bytes()) == sha256 {
            return true;
        }
        match text[end..].find('\n') {
            Some(at) => end += at + 1,
            None if end < text.len() => end = text.len(),
            None => return false,
        }
    }
}

/// The Link unit's owner binding and exact in-scope set.
fn link_owned(p: &Prepared, inputs: &Value, number: u64) -> Result<Vec<String>> {
    if inputs["producer"] != json!(format!("digest/g{number:03}@1")) {
        bail!("Link unit input does not name its group producer");
    }
    frozen_owned(p, inputs, "Link unit")
}

/// The accepted plan: one owned paragraph per written Card, and one checked
/// The Link unit's frozen gap scope: the prepared enumeration when `link-start` published it,
/// else the same deterministic classification over the unit's owned Cards (the compatibility path
/// for a Link unit claimed before the start step existed).
struct LinkScope {
    book: String,
    anchor: String,
    gaps: Vec<link_gap::Gap>,
    skipped: Vec<String>,
    revision: u64,
}

fn link_scope(
    p: &Prepared,
    zt: &mut Zt,
    number: u64,
    owned: &[String],
    progress: &Value,
) -> Result<LinkScope> {
    let book = field(&p.contract, "citation_key")?.to_string();
    let anchor = p.contract["groups"]
        .as_array()
        .context("incomplete prepared groups")?
        .iter()
        .find(|group| group["n"].as_i64() == Some(number as i64))
        .and_then(|group| group["anchor"].as_str())
        .with_context(|| format!("the prepared group {number} has no anchor"))?
        .to_string();
    if progress.get("link_start").is_some_and(Value::is_object) {
        let record = &progress["link_start"]["gap"];
        check_file(record, "Link gap scope")?;
        let value = util::read_json(field(record, "path")?)?;
        let group = &value["group"];
        let mut frozen: Vec<String> = ps::ps_items(&group["owned"]).iter().map(s).collect();
        let mut here: Vec<String> = owned.to_vec();
        ps::ps_sort(&mut frozen, |a, b| ps::culture_cmp(a, b));
        ps::ps_sort(&mut here, |a, b| ps::culture_cmp(a, b));
        if frozen != here {
            bail!(
                "the prepared Link gap scope does not match the unit's frozen owned enumeration; rerun prepare-link"
            );
        }
        let mut gaps = Vec::new();
        for gap in ps::ps_items(&group["gaps"]) {
            gaps.push(link_gap::Gap {
                at: field(&gap, "at")?.to_string(),
                reasons: ps::ps_items(&gap["reasons"]).iter().map(s).collect(),
            });
        }
        return Ok(LinkScope {
            book: field(&value, "book")?.to_string(),
            anchor,
            gaps,
            skipped: ps::ps_items(&group["skipped"]).iter().map(s).collect(),
            revision: value["archive_revision"]
                .as_u64()
                .context("the prepared Link gap scope has no archive revision")?,
        });
    }
    // The accepted plan is immutable once written, so a lost final reply re-reads the same frozen
    // gap scope instead of classifying Cards the first attempt already changed (AC08).
    let accepted = p.out.join(format!("link-accepted-g{number:03}.json"));
    if accepted.exists() {
        let plan = canon::read_json(&accepted.to_string_lossy())?;
        if plan["unit"] == json!(link_unit(number)) {
            let mut gaps = Vec::new();
            for gap in ps::ps_items(&plan["gap"]) {
                gaps.push(link_gap::Gap {
                    at: field(&gap, "at")?.to_string(),
                    reasons: ps::ps_items(&gap["reasons"]).iter().map(s).collect(),
                });
            }
            return Ok(LinkScope {
                book,
                anchor,
                gaps,
                skipped: ps::ps_items(&plan["skipped"]).iter().map(s).collect(),
                revision: plan["archive_revision"].as_u64().unwrap_or(0),
            });
        }
    }
    let facts = link_gap::read(zt, &book, owned)?;
    let groups = link_gap::classify(
        &facts,
        &[link_gap::Group {
            n: number as i64,
            id: format!("g{number:02}"),
            anchor: anchor.clone(),
            owned: owned.to_vec(),
        }],
    )?;
    let scope = &groups[0];
    Ok(LinkScope {
        book,
        anchor,
        gaps: scope.gaps.clone(),
        skipped: scope.skipped.clone(),
        revision: facts.revision,
    })
}
/// The accepted plan: one owned paragraph or one checked disposition for every gap Card, and a
/// verified `already_linked` disposition for every owned Card the frozen gap set excluded. A gap
/// Card is never silently omitted, and the job can never widen to a Card outside the gap set.
fn link_plan(
    owned: &[String],
    gaps: &[link_gap::Gap],
    draft: &Value,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let in_owned = |at: &str| owned.iter().any(|owned| owned == at);
    let gap_of = |at: &str| gaps.iter().find(|gap| gap.at == at);
    let mut calls = Vec::new();
    let mut written = std::collections::BTreeMap::new();
    let mut bytes = 0usize;
    for (index, call) in draft["calls"]
        .as_array()
        .context("Link draft calls must be an array")?
        .iter()
        .enumerate()
    {
        if call.as_object().is_none_or(|value| {
            value
                .keys()
                .any(|key| !["tool", "arguments"].contains(&key.as_str()))
        }) {
            bail!("Link draft call {index} must contain only tool and arguments");
        }
        if call["tool"] != "edit_card" {
            bail!("Link draft call {index} must edit one owned Card's paragraph");
        }
        let arguments = call["arguments"]
            .as_object()
            .context("Link draft call arguments must be an object")?;
        if arguments.keys().any(|key| {
            !["at", "base_fingerprint", "base_receipt", "owned_paragraph"].contains(&key.as_str())
        }) {
            bail!("Link draft call {index} may carry only an owned paragraph and its authored base");
        }
        let at = arguments
            .get("at")
            .and_then(Value::as_str)
            .context("Link draft call has no Card address")?;
        if !in_owned(at) {
            bail!("Link draft call {index} names `{at}`, which is not an owned Card of this Link unit");
        }
        let Some(gap) = gap_of(at) else {
            // AC01/AC02: only the gap set receives a Link job. A Card outside it is already linked.
            bail!("Link draft call {index} names `{at}`, which is not a semantic gap of this Link unit");
        };
        if written.insert(at.to_string(), index).is_some() {
            bail!("Link draft writes `{at}` twice; a correction needs a new plan revision");
        }
        if arguments.get("base").is_some() {
            bail!("Link draft call {index} must use an authored base, not a raw base");
        }
        if arguments.get("base_fingerprint").is_none() && arguments.get("base_receipt").is_none() {
            bail!("Link draft call {index} has no authored base for its owned paragraph");
        }
        let paragraph = arguments
            .get("owned_paragraph")
            .and_then(Value::as_object)
            .context("Link draft call must supply one receipt-owned paragraph")?;
        if paragraph
            .keys()
            .any(|key| !["text", "replaces_receipt", "adopt_legacy"].contains(&key.as_str()))
        {
            bail!("Link draft call {index} has an unknown owned paragraph field");
        }
        let text = paragraph
            .get("text")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .context("Link draft call has an empty owned paragraph")?;
        // D121/AC05: the anchor's one coordinator-owned paragraph is its `在全书中的位置` statement.
        if gap.reasons.iter().any(|reason| reason == link_gap::ANCHOR_MISSING)
            && !text.starts_with(link_gap::POSITION_PREFIX)
        {
            bail!(
                "the anchor `{at}` has no `{}` paragraph; its owned paragraph must start with it",
                link_gap::POSITION_PREFIX
            );
        }
        bytes += text.len();
        calls.push(call.clone());
    }
    if calls.len() > LINK_MAX_CALLS || bytes > LINK_MAX_BYTES {
        bail!(
            "a Link plan holds at most {LINK_MAX_CALLS} owned paragraphs within {LINK_MAX_BYTES} bytes; check each revision separately"
        );
    }
    let mut dispositions = std::collections::BTreeMap::new();
    for (index, entry) in draft["dispositions"]
        .as_array()
        .context("Link draft dispositions must be an array")?
        .iter()
        .enumerate()
    {
        if entry.as_object().is_none_or(|value| {
            value
                .keys()
                .any(|key| !["at", "outcome", "reason", "from"].contains(&key.as_str()))
        }) {
            bail!("Link disposition {index} must contain only at, outcome, reason and from");
        }
        let at = entry["at"]
            .as_str()
            .context("Link disposition has no Card address")?;
        if !in_owned(at) {
            bail!("Link disposition {index} names `{at}`, which is not an owned Card of this Link unit");
        }
        if gap_of(at).is_none() {
            bail!("Link disposition {index} names `{at}`, which is not a semantic gap of this Link unit");
        }
        // AC06 writes the outcome `none`; `no_link` is the retained name of the same result.
        let outcome = match entry["outcome"].as_str().unwrap_or_default() {
            "none" => "no_link",
            other => other,
        };
        let reason = entry["reason"].as_str().unwrap_or_default().trim();
        if reason.is_empty() || reason.chars().count() > 500 || reason.contains(['\r', '\n']) {
            bail!("Link disposition for `{at}` needs one reason line of at most 500 characters");
        }
        if outcome == "linked_from" {
            let from = entry["from"]
                .as_str()
                .context("a linked_from outcome needs the Card that holds the new sentence")?;
            if !in_owned(from) {
                bail!("Link disposition for `{at}` names the unowned source `{from}`");
            }
            if !written.contains_key(from) {
                bail!("`{at}` is recorded as linked_from `{from}`, which this draft does not write");
            }
        } else if !["already_linked", "no_link"].contains(&outcome) {
            bail!("Link disposition {index} must be already_linked, no_link, none or linked_from");
        }
        if dispositions
            .insert(at.to_string(), (outcome.to_string(), reason.to_string(), entry["from"].as_str().unwrap_or_default().to_string()))
            .is_some()
        {
            bail!("Link draft disposes `{at}` twice");
        }
    }
    let mut outcomes = Vec::new();
    for at in owned {
        let gap = gap_of(at);
        match (written.get(at), dispositions.get(at)) {
            (Some(index), None) => {
                outcomes.push(json!({"at":at,"outcome":"linked","call":index}))
            }
            (None, Some((outcome, reason, from))) => {
                if outcome == "linked_from" {
                    outcomes.push(json!({"at":at,"outcome":"linked_from","from":from,"reason":reason}))
                } else {
                    outcomes.push(json!({"at":at,"outcome":outcome,"reason":reason}))
                }
            }
            (Some(_), Some(_)) => {
                bail!("Link draft gives `{at}` both an owned paragraph and a disposition")
            }
            (None, None) => {
                if gap.is_none() {
                    // The frozen gap set excluded it: it already carries a valid semantic Link.
                    outcomes.push(json!({"at":at,"outcome":"already_linked","reason":NO_GAP_REASON}));
                } else {
                    bail!("Link draft has no outcome for the gap Card `{at}`");
                }
            }
        }
    }
    Ok((calls, outcomes))
}

/// The reason recorded for an owned Card the frozen gap set excluded (AC01/AC07).
const NO_GAP_REASON: &str = "no semantic gap: the Card already carries a valid kinded Link outside its own group";

/// Whether an `already_linked` outcome must show a kinded Link of the Card's own. A writer's claim must;
/// a Card the frozen gap set itself left out (`NO_GAP_REASON`) need not: the gap set asks a kinded Link only
/// of an Idea Card, so a Topic root or a Model Card it left out may carry none (grossman2002integration
/// trials 7 and 8: `363/0`, `366/0` and an unbuilt Model Card were refused although no job could be asked to
/// link them).
fn already_linked_needs_kind(reason: &Value) -> bool {
    reason.as_str() != Some(NO_GAP_REASON)
}

/// A Card's kinded Link targets, as the archive itself computes them: a Link
/// whose Relation kind is not a plain `refers` statement.
fn kinded_links(card: &Value) -> Result<std::collections::BTreeSet<String>> {
    let kinds = card["kinds"]
        .as_object()
        .context("Link unit Card is missing its kind facts")?;
    Ok(kinds
        .iter()
        .filter(|(_, kind)| kind.as_str() != Some("refers"))
        .map(|(target, _)| target.clone())
        .collect())
}

/// The Link targets written in one receipt-owned paragraph.
fn paragraph_targets(text: &str) -> Result<Vec<String>> {
    let target = regex::Regex::new(r"\[\[([^\]]+)\]\]")?;
    Ok(target
        .captures_iter(text)
        // The whole [[...]] is the address: ZT addresses carry '|' (`b/2|a|a|17`), and a Link has no
        // alias part (grossman2002integration trial 7: a paragraph that rests on a model part was read as
        // a Link to `b/2` and refused as carrying no kinded Link).
        .map(|capture| capture[1].trim().to_string())
        .filter(|target| !target.is_empty())
        .collect())
}

/// Every gap Card's outcome, the receipt-owned paragraph it wrote, and its changed-Card set. A
/// false claim is refused, never skipped: a written paragraph must carry a kinded Link outside the
/// Card's own group, `already_linked` must be true, and `linked_from` must name a written Card
/// whose paragraph links the gap Card.
fn link_verify(
    scope: &LinkScope,
    owned: &[String],
    outcomes: &[Value],
    receipts: &[Value],
    cards: &std::collections::BTreeMap<String, Value>,
) -> Result<(Vec<Value>, Vec<String>)> {
    let own = |target: &str| owned.iter().any(|at| at == target);
    let parent_of = |card: &Value| s(&card["parent_address"]);
    // D119/D122: the Link must be kinded and name a Card outside the group, not the parent, and not
    // a source place (the book Card or one of its anchors).
    let closed_by = |card: &Value, targets: &[String]| -> Vec<String> {
        let parent = parent_of(card);
        let Ok(kinds) = kinded_links(card) else {
            return Vec::new();
        };
        targets
            .iter()
            .filter(|target| {
                kinds.contains(*target)
                    && !ps::eq(target, &parent)
                    && !link_gap::is_book_place(target)
                    && !own(target)
            })
            .cloned()
            .collect()
    };
    let anchor_position = |card: &Value| -> bool {
        let paragraphs = link_gap::position_paragraphs(&s(&card["body"]));
        paragraphs.len() == 1
            && paragraph_targets(&paragraphs[0]).is_ok_and(|targets| {
                targets.iter().any(|target| {
                    !ps::eq(target, &scope.book)
                        && !ps::eq(target, &scope.anchor)
                        && !own(target)
                })
            })
    };
    let gap_of = |at: &str| scope.gaps.iter().find(|gap| gap.at == at);
    let mut detail = Vec::new();
    for outcome in outcomes {
        let at = field(outcome, "at")?;
        let card = cards
            .get(at)
            .with_context(|| format!("the owned Card `{at}` is missing from the archive read"))?;
        if card["workspace"] != "agent" {
            bail!("the owned Card `{at}` is not in the agent Workspace");
        }
        let gap = gap_of(at);
        match field(outcome, "outcome")? {
            "linked" => {
                let index = outcome["call"]
                    .as_u64()
                    .context("missing Link call index")? as usize;
                let receipt = receipts
                    .get(index)
                    .with_context(|| format!("missing the owned paragraph receipt of `{at}`"))?;
                let receipt_at = field(&receipt["result"], "address")?;
                if receipt_at != at {
                    bail!("the owned paragraph receipt of `{at}` names `{receipt_at}`");
                }
                if crate::calls::authored_fingerprint(card)
                    != field(&receipt["result"], "after_fingerprint")?
                {
                    bail!("the owned paragraph receipt of `{at}` no longer describes its Card");
                }
                let text = field(&receipt["result"]["owned_paragraph"], "text")?;
                if !card["body"].as_str().unwrap_or_default().contains(text) {
                    bail!("the owned paragraph of `{at}` is absent from its persisted Card");
                }
                if at == scope.anchor && !text.starts_with(link_gap::POSITION_PREFIX) {
                    bail!(
                        "the anchor `{at}` owns its `{}` paragraph; the written sentence does not start with it",
                        link_gap::POSITION_PREFIX
                    );
                }
                let targets = paragraph_targets(text)?;
                let kinds = kinded_links(card)?;
                let kinded: Vec<String> = targets
                    .iter()
                    .filter(|target| kinds.contains(*target))
                    .cloned()
                    .collect();
                if kinded.is_empty() {
                    bail!("the owned paragraph of `{at}` carries no kinded Link");
                }
                let closed = closed_by(card, &kinded);
                if gap.is_some_and(|gap| {
                    gap.reasons
                        .iter()
                        .any(|reason| reason == link_gap::NO_SEMANTIC_LINK)
                }) && closed.is_empty()
                {
                    bail!(
                        "the owned paragraph of `{at}` names no Card outside its own group; it does not close the semantic gap"
                    );
                }
                detail.push(json!({"at":at,"outcome":"linked","receipt":field(receipt,"op_key")?,"paragraph":text,"links":targets,"kinded":kinded,"closed":closed}));
            }
            "already_linked" => {
                let is_anchor = at == scope.anchor;
                let kinds: Vec<String> = kinded_links(card)?.into_iter().collect();
                let closed = closed_by(card, &kinds);
                if is_anchor {
                    if !anchor_position(card) {
                        bail!("the anchor `{at}` is recorded as already_linked but holds no linked `{}` paragraph", link_gap::POSITION_PREFIX);
                    }
                } else if kinds.is_empty() && already_linked_needs_kind(&outcome["reason"]) {
                    bail!("`{at}` is recorded as already_linked but carries no kinded Link");
                }
                if gap.is_some_and(|gap| {
                    gap.reasons
                        .iter()
                        .any(|reason| reason == link_gap::NO_SEMANTIC_LINK)
                }) && closed.is_empty()
                    && !is_anchor
                {
                    bail!("`{at}` is recorded as already_linked but carries no kinded Link outside its own group");
                }
                detail.push(json!({"at":at,"outcome":"already_linked","reason":outcome["reason"],"kinds":kinds,"closed":closed}));
            }
            "linked_from" => {
                let from = field(outcome, "from")?;
                let source = cards
                    .get(from)
                    .with_context(|| format!("the source Card `{from}` of `{at}` is missing"))?;
                let source_outcome = outcomes
                    .iter()
                    .find(|other| {
                        field(other, "at").is_ok_and(|other_at| other_at == from)
                            && field(other, "outcome").is_ok_and(|kind| kind == "linked")
                    })
                    .with_context(|| format!("`{at}` is recorded as linked_from `{from}`, which the plan does not write"))?;
                let index = source_outcome["call"]
                    .as_u64()
                    .context("missing Link call index")? as usize;
                let receipt = receipts
                    .get(index)
                    .with_context(|| format!("missing the owned paragraph receipt of `{from}`"))?;
                let text = field(&receipt["result"]["owned_paragraph"], "text")?;
                if !paragraph_targets(text)?.iter().any(|target| target == at) {
                    bail!("`{at}` is recorded as linked_from `{from}`, whose owned paragraph does not link it");
                }
                if !kinded_links(source)?.contains(at) {
                    bail!("`{at}` is recorded as linked_from `{from}`, whose Link to it is not kinded");
                }
                detail.push(json!({"at":at,"outcome":"linked_from","from":from,"reason":outcome["reason"]}));
            }
            "no_link" => detail.push(json!({"at":at,"outcome":"no_link","reason":outcome["reason"]})),
            other => bail!("unknown Link outcome `{other}`"),
        }
    }
    let changed = receipts
        .iter()
        .filter(|receipt| {
            receipt["result"]["before_fingerprint"] != receipt["result"]["after_fingerprint"]
        })
        .map(|receipt| field(&receipt["result"], "address").map(str::to_string))
        .collect::<Result<Vec<_>>>()?;
    let mut seen = std::collections::BTreeSet::new();
    for at in &changed {
        if !owned.iter().any(|owned| owned == at) {
            bail!("a Link receipt changed the unowned Card `{at}`");
        }
        if !seen.insert(at.clone()) {
            bail!("a Link receipt changed `{at}` twice");
        }
    }
    Ok((detail, changed))
}

/// The Link SELF-CHECK: the existing checker's Link rules over every owned Card.
/// A language fault the group already deferred stays the card cycle's obligation.
fn link_self_check(cards: &[Value], p: &Prepared) -> Result<Value> {
    use std::collections::{HashMap, HashSet};
    let book = field(&p.contract, "citation_key")?;
    let literature = HashSet::from([book.to_string()]);
    let anchors = HashMap::from([(
        book.to_string(),
        p.contract["groups"].as_array().unwrap().len(),
    )]);
    let mut entries = Vec::new();
    for card in cards {
        let mut entry = card.clone();
        entry["category"] = json!(zt_check::graph::category(card, &literature, &anchors));
        entry["parent"] = card["parent_address"].clone();
        entry["kinds"] = json!(
            card["kinds"]
                .as_object()
                .context("Link self-check Card is missing its kind facts")?
                .keys()
                .collect::<Vec<_>>()
        );
        entry["book"] = json!(book);
        entries.push(entry);
    }
    let mut file = tempfile::NamedTempFile::new()?;
    serde_json::to_writer(&mut file, &entries)?;
    std::io::Write::flush(&mut file)?;
    let check = crate::check::Check {
        json: file.path().to_string_lossy().into_owned(),
        ..Default::default()
    };
    let lines =
        zt_check::check::run_lines(&check.args(), None).context("Link self-check did not run")?;
    let count = lines
        .last()
        .and_then(|line| line.strip_prefix("faults: "))
        .and_then(|n| n.parse::<usize>().ok())
        .context("Link self-check did not report completeness")?;
    if lines.len() != count + 1 {
        bail!("incomplete Link self-check count");
    }
    let observed = &lines[..lines.len() - 1];
    let link_rule = |line: &String| {
        line.split_whitespace()
            .nth(1)
            .is_some_and(|rule| LINK_RULES.split(',').any(|known| known == rule))
    };
    let faults: Vec<String> = observed
        .iter()
        .filter(|line| !line.contains("(advisory)") && link_rule(line))
        .cloned()
        .collect();
    if !faults.is_empty() {
        bail!("Link SELF-CHECK found faults: {}", faults.join("; "));
    }
    Ok(json!({"ran":true,"rules":LINK_RULES,"count":0,"checked":observed.len(),"faults":faults,"lines":observed}))
}

fn link_output_keys(result: &Value) -> Result<std::collections::BTreeSet<String>> {
    if result["schema_version"] != 1
        || result["kind"] != "link_completion"
        || result["self_check"]["ran"] != true
        || result["self_check"]["count"] != 0
    {
        bail!("incomplete typed Link completion evidence");
    }
    let files = result["files"]
        .as_object()
        .context("missing Link evidence enumeration")?;
    if files.len() != 4
        || files
            .keys()
            .any(|key| !["accepted", "coverage", "self_check", "completion"].contains(&key.as_str()))
    {
        bail!("incomplete Link evidence enumeration");
    }
    let mut keys = std::collections::BTreeSet::from(
        ["accepted", "coverage", "self_check", "completion"].map(str::to_string),
    );
    for name in ["accepted", "coverage", "self_check", "completion"] {
        field(&files[name], "path")?;
        field(&files[name], "sha256")?;
    }
    for at in result["changed"]
        .as_array()
        .context("missing Link changed-Card enumeration")?
    {
        keys.insert(format!(
            "card/{}",
            at.as_str().context("invalid changed Link Card")?
        ));
    }
    for key in result["receipt_keys"]
        .as_array()
        .context("missing Link receipt enumeration")?
    {
        keys.insert(format!(
            "receipt/{}",
            key.as_str().context("invalid Link receipt key")?
        ));
    }
    Ok(keys)
}

/// Repeated completion re-reads the evidence it registered; a later edit of a
/// changed Card, a changed plan or a lost receipt makes the unit incomplete.
fn link_done(p: &Prepared, zt: &mut Zt, number: u64, unit: &str, token: i64) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let raw: String = tx.query_row(
        "SELECT result_json FROM units WHERE id=?1 AND state='done' AND claim_token=?2",
        params![unit, token],
        |r| r.get(0),
    )?;
    let mut result: Value = serde_json::from_str(&raw)?;
    let rows=tx.prepare("SELECT output_key,kind,locator,fingerprint FROM outputs WHERE unit_id=?1 ORDER BY output_key")?.query_map([unit],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    tx.commit()?;
    drop(conn);
    let expected = link_output_keys(&result)?;
    if rows
        .iter()
        .map(|(key, _, _, _)| key.clone())
        .collect::<std::collections::BTreeSet<_>>()
        != expected
    {
        bail!("incomplete Link completion output registration");
    }
    for (key, kind, locator, fingerprint) in &rows {
        match kind.as_str() {
            "file" => {
                if result["files"][key]["path"] != *locator
                    || result["files"][key]["sha256"] != *fingerprint
                {
                    bail!("changed Link evidence registration");
                }
                check_file(
                    &json!({"path":locator,"sha256":fingerprint}),
                    "Link completion evidence",
                )?;
            }
            "card" | "archive_receipt" => {}
            _ => bail!("unknown Link evidence kind"),
        }
    }
    let accepted = bound_json(&result["files"]["accepted"], "Link completion evidence")?;
    if canon::hash_value(&accepted) != result["plan_hash"] {
        bail!("completed Link plan hash changed");
    }
    let inputs = frozen_inputs(p, unit)?;
    if canon::hash_value(&inputs) != field(&result, "unit_input_hash")? {
        bail!("Link unit input hash mismatch");
    }
    let owned = link_owned(p, &inputs, number)?;
    if json!(owned) != result["owned"] {
        bail!("completed Link unit owned enumeration changed");
    }
    let enumerated = |value: &Value, label: &str| -> Result<std::collections::BTreeMap<String, String>> {
        let mut outcomes = std::collections::BTreeMap::new();
        for outcome in value.as_array().with_context(|| format!("missing {label} outcomes"))? {
            let at = field(outcome, "at")?.to_string();
            let kind = field(outcome, "outcome")?.to_string();
            if outcomes.insert(at.clone(), kind).is_some() {
                bail!("{label} outcomes name `{at}` twice");
            }
        }
        Ok(outcomes)
    };
    if enumerated(&accepted["outcomes"], "accepted plan")?
        != enumerated(&result["outcomes"], "completion")?
    {
        bail!("completed Link plan enumeration changed");
    }
    let receipt_addresses = verify_archive_receipts(zt, &accepted, &result["receipt_keys"])?;
    let changed = result["changed"]
        .as_array()
        .context("missing Link changed-Card enumeration")?
        .iter()
        .map(|at| {
            at.as_str()
                .context("invalid changed Link Card")
                .map(str::to_string)
        })
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
    if receipt_addresses != changed {
        bail!("completed Link changed set differs from its archive receipts");
    }
    if !changed.is_empty() {
        let read = zt.cards(&changed.iter().cloned().collect::<Vec<_>>(), "text")?;
        for card in &read.cards {
            let at = field(card, "address")?;
            if card["workspace"] != "agent"
                || result["card_fingerprints"][at] != json!(crate::calls::authored_fingerprint(card))
            {
                bail!("completed Link output changed: {at}");
            }
        }
        if zt.archive_view()?["archive_revision"].as_u64() != Some(read.archive_revision) {
            bail!("read_view_changed: Link completion verification");
        }
    }
    result["reused"] = json!(true);
    Ok(result)
}

fn link_finish(
    p: &Prepared,
    zt: &mut Zt,
    number: u64,
    token: i64,
    draft_path: &str,
    check_only: bool,
) -> Result<Value> {
    use std::collections::BTreeMap;
    let unit = link_unit(number);
    finish_claim(p, &unit, token)?;
    let _guard = unit_guard(p, &unit)?;
    let (claimed, progress, run, done) = finish_claim(p, &unit, token)?;
    let inputs = frozen_inputs(p, &unit)?;
    if canon::hash_value(&inputs) != claimed.input_hash {
        bail!("Link unit input hash mismatch");
    }
    let owned = link_owned(p, &inputs, number)?;
    if done {
        let mut result = link_done(p, zt, number, &unit, token)?;
        if check_only {
            result["check_only"] = json!(true);
            result["applied"] = json!(false);
        }
        return Ok(result);
    }
    // The frozen gap scope: only these Cards may receive a Link. It is read from the start's
    // retained evidence when one published it, else classified from the same archive read.
    let scope = link_scope(p, zt, number, &owned, &progress)?;
    let path = std::fs::canonicalize(draft_path).context("missing Link draft JSON")?;
    if !path.starts_with(&p.out) {
        bail!("Link draft binding path mismatch");
    }
    let (draft, draft_record) = json_artifact(&path)?;
    if draft["schema_version"] != 1
        || draft["unit"] != unit
        || draft["token"] != token
        || draft["input_hash"] != claimed.input_hash
    {
        bail!("Link draft binding mismatch: schema, unit, token or input hash");
    }
    if draft
        .as_object()
        .context("Link draft must be an object")?
        .keys()
        .any(|key| {
            !["schema_version", "unit", "token", "input_hash", "calls", "dispositions"]
                .contains(&key.as_str())
        })
    {
        bail!("unknown Link draft field");
    }
    let (calls, outcomes) = link_plan(&owned, &scope.gaps, &draft)?;
    let run_id = run.as_str().context("missing run id")?;
    let prefix = format!("{run_id}/link-g{number:03}/1/apply");
    let plan = json!({"schema_version":1,"kind":"link_plan","unit":unit,"unit_input_hash":claimed.input_hash,"plan_revision":1,"operation_prefix":prefix,"owned":owned,"gap":scope.gaps.iter().map(link_gap::Gap::json).collect::<Vec<_>>(),"skipped":scope.skipped,"archive_revision":scope.revision,"outcomes":outcomes,"calls":calls});
    let accepted_path = p.out.join(format!("link-accepted-g{number:03}.json"));
    if accepted_path.exists() && canon::read_json(&accepted_path.to_string_lossy())? != plan {
        bail!("accepted_plan_changed: the Link draft differs from the immutable accepted plan");
    }
    // The draft view runs the real transaction validators and rolls back; only the
    // persisted view below can establish completion.
    let preview = if calls.is_empty() {
        None
    } else {
        Some(crate::calls::preview_identified(zt, &calls, &prefix, None).context("Link draft precheck")?)
    };
    let mut read = zt.cards(&owned, "text")?;
    if let Some(preview) = &preview {
        for card in preview["cards"]
            .as_array()
            .context("incomplete Link draft Cards")?
        {
            let at = field(card, "address")?;
            if let Some(slot) = read.cards.iter_mut().find(|known| known["address"] == at) {
                *slot = card.clone();
            }
        }
    }
    let mut cards = BTreeMap::new();
    for card in &read.cards {
        let at = field(card, "address")?;
        if !owned.iter().any(|owned| owned == at) {
            bail!("the archive read returned the unowned Card `{at}`");
        }
        cards.insert(at.to_string(), card.clone());
    }
    for at in &owned {
        if !cards.contains_key(at) {
            bail!("the owned Card `{at}` does not exist");
        }
    }
    let draft_receipts: Vec<Value> = match &preview {
        Some(preview) => preview["results"]
            .as_array()
            .context("incomplete Link draft receipts")?
            .clone(),
        None => Vec::new(),
    };
    let (detail, draft_changed) = link_verify(&scope, &owned, &outcomes, &draft_receipts, &cards)?;
    let self_check = link_self_check(
        &owned
            .iter()
            .map(|at| cards[at].clone())
            .collect::<Vec<_>>(),
        p,
    )?;
    if zt.cards(&[], "text")?.archive_revision != read.archive_revision {
        bail!("read_view_changed: Link draft check");
    }
    check_file(&draft_record, "Link draft")?;
    let evidence = json!({
        "gap": scope.gaps.iter().map(link_gap::Gap::json).collect::<Vec<_>>(),
        "skipped": scope.skipped,
        "scope_revision": scope.revision,
        "counts": {
            "owned": owned.len(),
            "gaps": scope.gaps.len(),
            "skipped": scope.skipped.len(),
            "linked": detail.iter().filter(|outcome| outcome["outcome"] == "linked").count(),
            "linked_from": detail.iter().filter(|outcome| outcome["outcome"] == "linked_from").count(),
            "already_linked": detail.iter().filter(|outcome| outcome["outcome"] == "already_linked").count(),
            "no_link": detail.iter().filter(|outcome| outcome["outcome"] == "no_link").count(),
        },
        "quality": {"rules": LINK_RULES, "faults": self_check["count"], "checked": self_check["checked"]},
    });
    if check_only {
        return Ok(json!({"schema_version":1,"unit":unit,"token":token,"input_hash":claimed.input_hash,"check_only":true,"applied":false,"complete":true,"owned":owned,"outcomes":detail,"changed":draft_changed,"self_check":self_check,"archive_revision":read.archive_revision,"gap":evidence["gap"],"skipped":evidence["skipped"],"counts":evidence["counts"],"quality":evidence["quality"]}));
    }
    renew(p, &unit, token)?;
    let accepted = accept_plan(&accepted_path, &plan)?;
    let receipts = if calls.is_empty() {
        Vec::new()
    } else {
        crate::calls::invoke_identified(zt, &calls, &prefix, None)?
    };
    let read = zt.cards(&owned, "text")?;
    let mut cards = BTreeMap::new();
    for card in &read.cards {
        let at = field(card, "address")?;
        if card["workspace"] != "agent" {
            bail!("the owned Card `{at}` is not in the agent Workspace");
        }
        cards.insert(at.to_string(), card.clone());
    }
    for at in &owned {
        if !cards.contains_key(at) {
            bail!("the persisted Link unit is missing its owned Card `{at}`");
        }
    }
    let (detail, changed) = link_verify(&scope, &owned, &outcomes, &receipts, &cards)?;
    let self_check = link_self_check(
        &owned
            .iter()
            .map(|at| cards[at].clone())
            .collect::<Vec<_>>(),
        p,
    )?;
    let revision = read.archive_revision;
    if zt.cards(&[], "text")?.archive_revision != revision {
        bail!("read_view_changed: persisted Link check");
    }
    let card_fingerprints = changed
        .iter()
        .map(|at| Ok((at.clone(), crate::calls::authored_fingerprint(&cards[at]))))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let receipt_keys = receipts
        .iter()
        .map(|receipt| field(receipt, "op_key").map(str::to_string))
        .collect::<Result<Vec<_>>>()?;
    let plan_hash = canon::hash_value(&plan);
    let coverage = publish(
        &p.out.join(format!("link-coverage-g{number:03}.json")),
        &serde_json::to_string_pretty(
            &json!({"schema_version":1,"unit":unit,"token":token,"input_hash":claimed.input_hash,"plan_hash":plan_hash,"operation_prefix":prefix,"owned":owned,"outcomes":detail,"changed":changed,"archive_revision":revision}),
        )?,
    )?;
    let check_record = publish(
        &p.out.join(format!("link-self-check-g{number:03}.json")),
        &serde_json::to_string_pretty(
            &json!({"schema_version":1,"unit":unit,"token":token,"input_hash":claimed.input_hash,"plan_hash":plan_hash,"rules":LINK_RULES,"self_check":self_check}),
        )?,
    )?;
    let result = json!({"schema_version":1,"kind":"link_completion","unit":unit,"token":token,"state":"done","complete":true,"applied":!calls.is_empty(),"reused":false,"input_hash":claimed.input_hash,"unit_input_hash":claimed.input_hash,"plan_hash":plan_hash,"draft":draft_record,"disposition":if calls.is_empty(){"no_link"}else{"linked"},"owned":owned,"outcomes":detail,"changed":changed,"card_fingerprints":card_fingerprints,"receipt_keys":receipt_keys,"self_check":self_check,"files":{"accepted":accepted,"coverage":coverage,"self_check":check_record},"archive_revision":revision,"gap":evidence["gap"],"skipped":evidence["skipped"],"counts":evidence["counts"],"quality":evidence["quality"]});
    let completion = publish(
        &p.out.join(format!("link-completion-g{number:03}.json")),
        &serde_json::to_string_pretty(&result)?,
    )?;
    let mut result = result;
    result["files"]["completion"] = completion.clone();
    link_output_keys(&result)?;
    for record in [&accepted, &coverage, &check_record, &completion] {
        check_file(record, "Link completion evidence")?;
    }
    let final_view = zt.archive_view()?;
    if final_view["archive_identity"] != p.contract["archive_identity"]
        || final_view["archive_revision"].as_u64() != Some(revision)
    {
        bail!("read_view_changed: Link completion evidence changed before ledger commit");
    }
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, &unit, token)?;
    for (name, record) in [
        ("accepted", &accepted),
        ("coverage", &coverage),
        ("self_check", &check_record),
        ("completion", &completion),
    ] {
        output(
            &tx,
            &unit,
            name,
            "file",
            field(record, "path")?,
            field(record, "sha256")?,
            record,
        )?;
    }
    for (at, fingerprint) in &card_fingerprints {
        output(
            &tx,
            &unit,
            &format!("card/{at}"),
            "card",
            at,
            fingerprint,
            &json!({"tree":cards[at]["tree"],"type":cards[at]["type"],"facet":cards[at]["facet"]}),
        )?;
    }
    for receipt in &receipts {
        let op_key = field(receipt, "op_key")?;
        output(
            &tx,
            &unit,
            &format!("receipt/{op_key}"),
            "archive_receipt",
            op_key,
            &canon::hash_value(&receipt["result"]),
            &receipt["result"],
        )?;
    }
    completion_fault(p, "after_outputs")?;
    tx.execute("UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",params![unit,canon::canonical_json(&result),canon::utc_now_o()])?;
    event(
        &tx,
        &unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"disposition":result["disposition"],"completion":completion,"owned":result["owned"].as_array().map(Vec::len),"changed":result["changed"],"plan_hash":plan_hash}),
    )?;
    completion_fault(p, "before_commit")?;
    tx.commit()?;
    completion_fault(p, "after_commit")?;
    Ok(result)
}

/// The start of one claimed Link unit: it freezes the prepared gap scope against the unit's owned
/// receipts, writes the one job that writes the draft, and hands the chain the exact binding. An
/// empty gap set starts no provider run: the empty draft it publishes completes through the same
/// checked `link-finish`, and the stage records the group as skipped (AC07).
fn link_start(p: &Prepared, zt: &mut Zt, number: u64, token: i64) -> Result<Value> {
    let unit = link_unit(number);
    let (claimed, mut progress, run, done) = finish_claim(p, &unit, token)?;
    let inputs = frozen_inputs(p, &unit)?;
    if canon::hash_value(&inputs) != claimed.input_hash {
        bail!("Link unit input hash mismatch");
    }
    let owned = link_owned(p, &inputs, number)?;
    let id = p.contract["groups"]
        .as_array()
        .context("incomplete prepared groups")?
        .iter()
        .find(|group| group["n"].as_i64() == Some(number as i64))
        .and_then(|group| group["id"].as_str())
        .with_context(|| format!("the prepared group {number} has no id"))?
        .to_string();
    let gap_path = p.out.join(format!("link-gap-{id}.json"));
    let (gap_value, gap_record) = json_artifact(&gap_path).with_context(|| {
        format!(
            "missing the prepared Link gap scope {}; run prepare-link first",
            gap_path.display()
        )
    })?;
    let group = &gap_value["group"];
    let mut frozen: Vec<String> = ps::ps_items(&group["owned"]).iter().map(s).collect();
    let mut here: Vec<String> = owned.clone();
    ps::ps_sort(&mut frozen, |a, b| ps::culture_cmp(a, b));
    ps::ps_sort(&mut here, |a, b| ps::culture_cmp(a, b));
    if frozen != here {
        bail!("the prepared Link gap scope does not match the unit's frozen owned enumeration; rerun prepare-link");
    }
    let gaps = ps::ps_items(&group["gaps"]);
    let skipped: Vec<String> = ps::ps_items(&group["skipped"]).iter().map(s).collect();
    let draft_path = p.out.join(format!("link-draft-g{number:03}.json"));
    let prefix = p.out.join("link-prefix.md");
    let gap_doc = p.out.join(format!("link-{id}.md"));
    let job_file = p.out.join(format!("link-job-g{number:03}.md"));
    // The Link pass runs this group's DeepSeek job with cwd `<out>\link-jobs`; create it here, as
    // group-start creates `group-jobs` (acemoglu2025nobel trial 1: every Link job failed to start).
    std::fs::create_dir_all(p.out.join("link-jobs"))?;
    let empty = gaps.is_empty();
    // A group with no gap still completes its Link unit: the empty draft is bound to this claim
    // and checked by the same `link-finish`, which records every owned Card as already linked.
    let draft_record = if empty {
        Some(publish(
            &draft_path,
            &serde_json::to_string_pretty(&json!({
                "schema_version":1,"unit":unit,"token":token,"input_hash":claimed.input_hash,
                "calls":[],"dispositions":[],
            }))?,
        )?)
    } else {
        None
    };
    let cards = zt.cards(&owned, "text")?;
    let mut fingerprints: BTreeMap<String, String> = BTreeMap::new();
    for card in &cards.cards {
        fingerprints.insert(
            field(card, "address")?.to_string(),
            crate::calls::authored_fingerprint(card),
        );
    }
    if zt.cards(&[], "text")?.archive_revision != cards.archive_revision {
        bail!("read_view_changed: Link start facts");
    }
    let self_check = format!(
        "zt-digest link-finish -Out \"{}\" -Group {number} -Token {token} -Draft \"{}\" -CheckOnly",
        p.out.display(),
        draft_path.display()
    );
    let mut body = String::new();
    let mut line = |text: &str| {
        body.push_str(text);
        body.push('\n');
    };
    line(&format!("# 第 {number} 组的连接任务"));
    line("");
    line(&format!("本组拥有 {} 张卡片，语义缺口 {} 张；只改下面列出的缺口卡片（AC01、AC02）。", owned.len(), gaps.len()));
    line(&format!("- 共享连接规则：`{}`（先读它）", prefix.display()));
    line(&format!("- 本组缺口清单与卡片正文：`{}`", gap_doc.display()));
    line(&format!("- 只写一个 JSON 草稿：`{}`，绑定字段原样保留。", draft_path.display()));
    line(&format!("- 绑定字段：schema_version=1，unit=`{unit}`，token={token}，input_hash=`{}`。", claimed.input_hash));
    line("");
    line("## 草稿格式");
    line("");
    line("```json");
    line("{\"schema_version\":1,\"unit\":\"…\",\"token\":0,\"input_hash\":\"…\",\"calls\":[],\"dispositions\":[]}");
    line("```");
    line("");
    line("- `calls`：每项 `{\"tool\":\"edit_card\",\"arguments\":{\"at\":\"缺口卡片地址\",\"base_fingerprint\":\"该卡当前指纹\",\"owned_paragraph\":{\"text\":\"一句带关系词的链接句\"}}}`。");
    line("- `dispositions`：每项 `{\"at\":\"缺口卡片地址\",\"outcome\":\"already_linked|no_link|linked_from\",\"reason\":\"一句话理由\"}`；`linked_from` 还要加 `\"from\":\"写入新句的缺口卡片地址\"`。");
    line("- 每张缺口卡片恰好一个结果：写入一句说明关系的链接句，或记 `no_link`（即 none）并写明理由。没有真实关系时不要编造。");
    line(&format!("- 锚点：它的 `{}` 段由这里写（段落以它开头）。本组锚点状态：{}。", link_gap::POSITION_PREFIX, group["position"].as_str().unwrap_or("absent")));
    line("");
    line("## 缺口卡片");
    line("");
    for gap in &gaps {
        let at = s(&gap["at"]);
        let fingerprint = fingerprints.get(&at).cloned().unwrap_or_default();
        line(&format!("- `{at}`（{}）base_fingerprint=`{fingerprint}`", ps::ps_items(&gap["reasons"]).iter().map(s).collect::<Vec<_>>().join("、")));
    }
    line("");
    line(&format!("SELF-CHECK: {self_check}"));
    let job_record = publish(&job_file, &body)?;
    let expected = if empty { 0 } else { 1 };
    let jobs = if empty {
        json!([])
    } else {
        json!([{
            "id": format!("link-g{number:03}-t{token}"),
            "kind": "link",
            "files": [
                prefix.to_string_lossy(),
                gap_doc.to_string_lossy(),
                job_file.to_string_lossy(),
            ],
            "prompt": format!("按任务说明把连接草稿写入 {}，保留绑定字段。", draft_path.display()),
            "output": draft_path.to_string_lossy(),
            "permissions": {"archive_read": true, "archive_write": false},
        }])
    };
    let no_op = if empty {
        json!({"reason":"no_semantic_gaps","gaps":0,"skipped":skipped.len()})
    } else {
        Value::Null
    };
    let job = json!({
        "schema_version": 1, "unit": unit, "token": token, "run_id": run,
        "unit_input_hash": claimed.input_hash, "input_hash": claimed.input_hash,
        "mode": "draft_only", "expected_jobs": expected, "draft": draft_path.to_string_lossy(),
        "gap": gaps, "skipped": skipped, "counts": group["counts"], "position": group["position"],
        "no_op": no_op, "done": done,
        "artifacts": {"job": job_record, "gap": gap_record, "draft": draft_record},
        "jobs": jobs,
    });
    let record = publish(
        &p.out.join(format!("link-start-g{number:03}.json")),
        &serde_json::to_string_pretty(&job)?,
    )?;
    progress["link_start"] = json!({
        "token": token, "input_hash": claimed.input_hash, "job": record, "gap": gap_record,
    });
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, &unit, token)?;
    let lease_until = held_lease(&tx, &unit)?;
    tx.execute(
        "UPDATE units SET progress_json=?2,lease_until=?3 WHERE id=?1",
        params![unit, canon::canonical_json(&progress), lease_until],
    )?;
    event(
        &tx,
        &unit,
        Some(&claimed.owner),
        Some(token),
        "link_started",
        &json!({"input_hash":claimed.input_hash,"job":record,"gap":gap_record,"expected_jobs":expected,"gaps":ps::ps_items(&group["gaps"]).len(),"skipped":skipped.len(),"lease_until":lease_until}),
    )?;
    tx.commit()?;
    Ok(json!({
        "schema_version":1,"unit":unit,"token":token,"run_id":run,
        "input_hash":claimed.input_hash,"unit_input_hash":claimed.input_hash,
        "job": record["path"], "draft": draft_path.to_string_lossy(),
        "expected_jobs": expected, "no_op": no_op, "done": done,
        "gap": ps::ps_items(&group["gaps"]), "skipped": skipped,
        "position": group["position"], "counts": group["counts"],
        "lease_until": lease_until,
    }))
}

pub fn link_start_cli(argv: &[String]) -> Result<i32> {
    let parsed = (|| -> Result<(String, u64, i64)> {
        let a = Args::parse(argv, &["Out", "Group", "Token"], &[])?;
        if !a.positional.is_empty() || a.required("Out")?.trim().is_empty() {
            bail!("explicit -Out is required");
        }
        let group = a
            .required("Group")?
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .context("-Group must be positive")?;
        let token = a
            .required("Token")?
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .context("-Token must be positive")?;
        Ok((a.str("Out"), group, token))
    })();
    let (out, group, token) = match parsed {
        Ok(value) => value,
        Err(error) => {
            eprintln!("zt-digest link-start: {error}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let result = link_start(&p, &mut zt, group, token)?;
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}
pub fn link_finish_cli(argv: &[String]) -> Result<i32> {
    let parsed = (|| -> Result<(String, u64, i64, String, bool)> {
        let a = Args::parse(argv, &["Out", "Group", "Token", "Draft"], &["CheckOnly"])?;
        if !a.positional.is_empty() || a.required("Out")?.trim().is_empty() {
            bail!("explicit -Out is required");
        }
        let group = a
            .required("Group")?
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .context("-Group must be positive")?;
        let token = a
            .required("Token")?
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .context("-Token must be positive")?;
        Ok((
            a.str("Out"),
            group,
            token,
            a.required("Draft")?,
            a.flag("CheckOnly"),
        ))
    })();
    let (out, group, token, draft, check_only) = match parsed {
        Ok(v) => v,
        Err(e) => {
            eprintln!("zt-digest link-finish: {e}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let result = link_finish(&p, &mut zt, group, token, &draft, check_only)?;
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}

/// R216: the Cards/group unit is complete only when the offline zt-check has run
/// over exactly the group's owned Cards, every hard fault it found has a registered
/// consumer that can repair that Card category, and the checked result is retained.
/// The unit enumerates its scope from the group's registered owned manifest, never
/// from a caller-supplied address list; the book Card cycle is the consumer of the
/// work it hands onward.
const CARDS_CONSUMER: &str = "cards-book/book@1";

fn cards_unit(number: u64) -> String {
    format!("cards/g{number:03}@1")
}

fn cards_number(unit: &str) -> Option<u64> {
    unit.strip_prefix("cards/g")
        .and_then(|value| value.strip_suffix("@1"))
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|number| *number > 0)
}

/// The offline SELF-CHECK: the existing checker over the exact owned set, through
/// the same `zt mcp` the completion already holds, with the rules the whole-book
/// Card cycle applies to that scope.
fn cards_self_check(owned: &[String], book: &str, zt: &mut Zt) -> Result<Value> {
    let check = crate::check::Check {
        cards: owned.join(","),
        book: book.to_string(),
        rules: crate::card::DEFAULT_CARD_RULES.to_string(),
        ..Default::default()
    };
    let lines = check.run(zt)?;
    let reported = lines
        .last()
        .and_then(|line| line.strip_prefix("faults: "))
        .and_then(|count| count.parse::<usize>().ok())
        .context("Cards SELF-CHECK did not report completeness")?;
    let observed = &lines[..lines.len() - 1];
    let (advisories, faults): (Vec<_>, Vec<_>) = observed
        .iter()
        .partition(|line| line.contains("(advisory)"));
    if faults.len() != reported {
        bail!("incomplete Cards SELF-CHECK fault count");
    }
    Ok(json!({
        "ran": true,
        "rules": crate::card::DEFAULT_CARD_RULES,
        "count": faults.len(),
        "reported_count": reported,
        "faults": faults,
        "advisories": advisories,
        "lines": observed,
    }))
}

fn cards_fault_address(line: &str) -> Result<&str> {
    line.split_whitespace()
        .next()
        .filter(|at| !at.is_empty())
        .context("Cards SELF-CHECK fault line has no Card address")
}

/// Every hard fault must name an owned Card whose category the book cycle can repair,
/// or the group's registered Model consumer for that Model Card (R211, R216).
fn cards_consumer(p: &Prepared, zt: &mut Zt, number: u64, at: &str, card: &Value) -> Result<String> {
    use std::collections::{HashMap, HashSet};
    if card["type"] == "model" {
        let models = target(&p.contract)?["enabled"]
            .as_array()
            .context("incomplete declared target")?
            .iter()
            .any(|layer| layer == "models");
        if !models {
            bail!("Cards SELF-CHECK fault at `{at}` has no enabled Model consumer");
        }
        let model = zt.call("get_model", json!({"at":at}))?;
        let head = field(&model["head"], "address").context("incomplete Model candidate header")?;
        let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
        let children = conn
            .prepare("SELECT id,inputs_json FROM units WHERE layer='models' AND logical_key LIKE ?1 ORDER BY id")?
            .query_map([format!("g{number:03}-m%")], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, inputs) in children {
            let inputs: Value = serde_json::from_str(&inputs)?;
            if inputs["header"] == json!(head) {
                return Ok(id);
            }
        }
        bail!("Cards SELF-CHECK fault at `{at}` has no registered Model consumer");
    }
    let book = field(&p.contract, "citation_key")?.to_string();
    let literature = HashSet::from([book.clone()]);
    let anchors = HashMap::from([(
        book,
        p.contract["groups"].as_array().unwrap().len(),
    )]);
    let category = zt_check::graph::category(card, &literature, &anchors);
    if !["idea", "anchor", "event", "finding"].contains(&category.as_str()) {
        bail!("Cards SELF-CHECK fault at `{at}` has no writable Card consumer: it is a {category} Card");
    }
    Ok(CARDS_CONSUMER.to_string())
}

/// A registered consumer is a real repair path only while it can still run.
fn cards_consumer_ready(p: &Prepared, consumer: &str) -> Result<()> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let state: Option<String> = conn
        .query_row("SELECT state FROM units WHERE id=?1", [consumer], |row| {
            row.get(0)
        })
        .optional()?;
    let state = state.with_context(|| {
        format!("Cards SELF-CHECK fault has no registered consumer `{consumer}`")
    })?;
    if !matches!(state.as_str(), "registered" | "ready" | "claimed" | "retry") {
        bail!("Cards SELF-CHECK fault consumer `{consumer}` is {state}, so it cannot repair the fault");
    }
    Ok(())
}

/// The Cards unit exists only after its group producer accepted a checked completion;
/// a failed or missing producer is not a scope to check.
fn cards_producer(p: &Prepared, inputs: &Value, number: u64) -> Result<()> {
    let producer = field(inputs, "producer")?;
    if producer != format!("digest/g{number:03}@1") {
        bail!("Cards unit input does not name its group producer");
    }
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT state,input_hash FROM units WHERE id=?1",
            [producer],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (state, hash) = row.with_context(|| format!("Cards unit producer {producer} is not registered"))?;
    if state != "done" {
        bail!("Cards unit producer {producer} is {state}, not a checked completion");
    }
    if inputs["producer_input_hash"] != json!(hash) {
        bail!("Cards unit producer input hash changed");
    }
    Ok(())
}

fn cards_verified(result: &Value) -> Result<()> {
    if result["schema_version"] != 1
        || result["kind"] != "cards_completion"
        || result["state"] != "done"
        || result["complete"] != true
        || result["self_check"]["ran"] != true
    {
        bail!("incomplete typed Cards completion evidence");
    }
    Ok(())
}

fn cards_output_keys(result: &Value) -> Result<std::collections::BTreeSet<String>> {
    cards_verified(result)?;
    let files = result["files"]
        .as_object()
        .context("missing Cards evidence enumeration")?;
    if files.len() != 2
        || files
            .keys()
            .any(|key| !["check", "completion"].contains(&key.as_str()))
    {
        bail!("incomplete Cards evidence enumeration");
    }
    let mut keys = std::collections::BTreeSet::from(
        ["check", "completion"].map(str::to_string),
    );
    for name in ["check", "completion"] {
        field(&files[name], "path")?;
        field(&files[name], "sha256")?;
    }
    for at in result["owned"]
        .as_array()
        .context("missing Cards owned enumeration")?
    {
        keys.insert(format!(
            "card/{}",
            at.as_str().context("invalid owned Cards output")?
        ));
    }
    Ok(keys)
}

/// The complete checked outcome of the frozen owned set: the offline SELF-CHECK, its
/// hard faults and each fault's registered consumer.
fn cards_result(
    p: &Prepared,
    zt: &mut Zt,
    number: u64,
    token: i64,
    input_hash: &str,
    inputs: &Value,
    owned: &[String],
    read: &zt_check::mcp::CardRead,
) -> Result<Value> {
    use std::collections::{BTreeMap, BTreeSet};
    let book = field(&p.contract, "citation_key")?;
    let mut cards = BTreeMap::new();
    for card in &read.cards {
        let at = field(card, "address")?;
        if !owned.iter().any(|owned| owned == at) {
            bail!("the archive read returned the unowned Card `{at}`");
        }
        if card["workspace"] != "agent" {
            bail!("the owned Card `{at}` is not in the agent Workspace");
        }
        cards.insert(at.to_string(), card.clone());
    }
    for at in owned {
        if !cards.contains_key(at) {
            bail!("the owned Card `{at}` does not exist");
        }
    }
    let self_check = cards_self_check(owned, book, zt)?;
    if zt.cards(&[], "text")?.archive_revision != read.archive_revision {
        bail!("read_view_changed: Cards SELF-CHECK");
    }
    let mut deferred = Vec::new();
    for fault in self_check["faults"]
        .as_array()
        .context("missing checked Cards faults")?
    {
        let line = fault
            .as_str()
            .context("invalid Cards SELF-CHECK fault")?;
        let at = cards_fault_address(line)?;
        let card = cards
            .get(at)
            .context("Cards SELF-CHECK fault has no owned Card")?;
        let consumer = cards_consumer(p, zt, number, at, card)?;
        cards_consumer_ready(p, &consumer)?;
        deferred.push(json!({"fault":line,"at":at,"consumer":consumer}));
    }
    let fingerprints = owned
        .iter()
        .map(|at| {
            Ok((
                at.clone(),
                crate::calls::authored_fingerprint(&cards[at]),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let checked = owned.iter().cloned().collect::<BTreeSet<_>>();
    Ok(json!({
        "schema_version": 1,
        "kind": "cards_completion",
        "unit": cards_unit(number),
        "token": token,
        "state": "done",
        "complete": true,
        "reused": false,
        "input_hash": input_hash,
        "unit_input_hash": input_hash,
        "producer": field(inputs, "producer")?,
        "producer_input_hash": field(inputs, "producer_input_hash")?,
        "plan_hash": field(inputs, "plan_hash")?,
        "owned": checked,
        "disposition": if deferred.is_empty() { "verified" } else { "deferred" },
        "self_check": self_check,
        "deferred": deferred,
        "card_fingerprints": fingerprints,
        "archive_revision": read.archive_revision,
    }))
}

/// Repeated completion re-reads the Cards it checked and re-runs their SELF-CHECK; a
/// changed Card or a lost consumer makes the retained completion stale.
fn cards_done(p: &Prepared, zt: &mut Zt, number: u64, unit: &str, token: i64) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let raw: String = tx.query_row(
        "SELECT result_json FROM units WHERE id=?1 AND state='done' AND claim_token=?2",
        params![unit, token],
        |row| row.get(0),
    )?;
    let mut result: Value = serde_json::from_str(&raw)?;
    let rows = tx
        .prepare("SELECT output_key,kind,locator,fingerprint FROM outputs WHERE unit_id=?1 ORDER BY output_key")?
        .query_map([unit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    tx.commit()?;
    drop(conn);
    cards_verified(&result)?;
    let expected = cards_output_keys(&result)?;
    if rows
        .iter()
        .map(|(key, _, _, _)| key.clone())
        .collect::<std::collections::BTreeSet<_>>()
        != expected
    {
        bail!("incomplete Cards completion output registration");
    }
    for (key, kind, locator, fingerprint) in &rows {
        match kind.as_str() {
            "file" => {
                if result["files"][key]["path"] != *locator
                    || result["files"][key]["sha256"] != *fingerprint
                {
                    bail!("changed Cards evidence registration");
                }
                check_file(
                    &json!({"path":locator,"sha256":fingerprint}),
                    "Cards completion evidence",
                )?;
            }
            "card" => {}
            _ => bail!("unknown Cards evidence kind"),
        }
    }
    let inputs = frozen_inputs(p, unit)?;
    if canon::hash_value(&inputs) != field(&result, "unit_input_hash")? {
        bail!("Cards unit input hash mismatch");
    }
    cards_producer(p, &inputs, number)?;
    let owned = frozen_owned(p, &inputs, "Cards unit")?;
    if json!(owned) != result["owned"] {
        bail!("completed Cards unit owned enumeration changed");
    }
    let read = zt.cards(&owned, "text")?;
    let current = cards_result(
        p,
        zt,
        number,
        token,
        field(&result, "unit_input_hash")?,
        &inputs,
        &owned,
        &read,
    )?;
    if current["card_fingerprints"] != result["card_fingerprints"] {
        bail!("completed Cards unit owned Cards changed; keep the checked result and re-run from a new claim");
    }
    if current["self_check"]["lines"] != result["self_check"]["lines"]
        || current["disposition"] != result["disposition"]
    {
        bail!("completed Cards SELF-CHECK no longer describes the owned Cards");
    }
    result["reused"] = json!(true);
    Ok(result)
}

fn cards_finish(p: &Prepared, zt: &mut Zt, number: u64, token: i64, check_only: bool) -> Result<Value> {
    let unit = cards_unit(number);
    finish_claim(p, &unit, token)?;
    let _guard = unit_guard(p, &unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, &unit, token)?;
    let inputs = frozen_inputs(p, &unit)?;
    if canon::hash_value(&inputs) != claimed.input_hash {
        bail!("Cards unit input hash mismatch");
    }
    cards_producer(p, &inputs, number)?;
    let owned = frozen_owned(p, &inputs, "Cards unit")?;
    if done {
        let mut result = cards_done(p, zt, number, &unit, token)?;
        if check_only {
            result["check_only"] = json!(true);
            result["applied"] = json!(false);
        }
        return Ok(result);
    }
    let read = zt.cards(&owned, "text")?;
    let result = cards_result(p, zt, number, token, &claimed.input_hash, &inputs, &owned, &read)?;
    if check_only {
        let mut result = result;
        result["check_only"] = json!(true);
        result["applied"] = json!(false);
        return Ok(result);
    }
    renew(p, &unit, token)?;
    let check_record = publish(
        &p.out.join(format!("cards-check-g{number:03}.json")),
        &serde_json::to_string_pretty(
            &json!({"schema_version":1,"unit":unit,"token":token,"input_hash":claimed.input_hash,"owned":owned,"self_check":result["self_check"],"deferred":result["deferred"],"archive_revision":result["archive_revision"]}),
        )?,
    )?;
    let mut result = result;
    result["files"] = json!({"check": check_record.clone()});
    let completion = publish(
        &p.out.join(format!("cards-completion-g{number:03}.json")),
        &serde_json::to_string_pretty(&result)?,
    )?;
    result["files"]["completion"] = completion.clone();
    cards_output_keys(&result)?;
    for record in [&check_record, &completion] {
        check_file(record, "Cards completion evidence")?;
    }
    let final_view = zt.archive_view()?;
    if final_view["archive_identity"] != p.contract["archive_identity"]
        || final_view["archive_revision"].as_u64()
            != Some(result["archive_revision"].as_u64().unwrap())
    {
        bail!("read_view_changed: Cards completion evidence changed before ledger commit");
    }
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, &unit, token)?;
    for (name, record) in [("check", &check_record), ("completion", &completion)] {
        output(
            &tx,
            &unit,
            name,
            "file",
            field(record, "path")?,
            field(record, "sha256")?,
            record,
        )?;
    }
    for (at, fingerprint) in result["card_fingerprints"]
        .as_object()
        .context("missing checked Cards enumeration")?
    {
        output(
            &tx,
            &unit,
            &format!("card/{at}"),
            "card",
            at,
            fingerprint.as_str().context("invalid checked Card fingerprint")?,
            &json!({"owned": true}),
        )?;
    }
    completion_fault(p, "after_outputs")?;
    tx.execute("UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",params![unit,canon::canonical_json(&result),canon::utc_now_o()])?;
    event(
        &tx,
        &unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"disposition":result["disposition"],"completion":completion,"owned":owned,"faults":result["self_check"]["count"],"rules":result["self_check"]["rules"],"deferred":result["deferred"]}),
    )?;
    completion_fault(p, "before_commit")?;
    tx.commit()?;
    completion_fault(p, "after_commit")?;
    Ok(result)
}

pub fn cards_finish_cli(argv: &[String]) -> Result<i32> {
    let parsed = (|| -> Result<(String, u64, i64, bool)> {
        let a = Args::parse(argv, &["Out", "Group", "Token"], &["CheckOnly"])?;
        if !a.positional.is_empty() || a.required("Out")?.trim().is_empty() {
            bail!("explicit -Out is required");
        }
        let group = a
            .required("Group")?
            .parse::<u64>()
            .ok()
            .filter(|number| *number > 0)
            .context("-Group must be positive")?;
        let token = a
            .required("Token")?
            .parse::<i64>()
            .ok()
            .filter(|number| *number > 0)
            .context("-Token must be positive")?;
        Ok((a.str("Out"), group, token, a.flag("CheckOnly")))
    })();
    let (out, group, token, check_only) = match parsed {
        Ok(value) => value,
        Err(error) => {
            eprintln!("zt-digest cards-finish: {error}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let result = cards_finish(&p, &mut zt, group, token, check_only)?;
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}

// ==== The Events/Findings layer unit (R220, R221) ===========================================

/// One empirical layer the group released: the Cards it writes, the kind word of its
/// Links, and the Card rules its accepted output must pass.
struct Empirical {
    /// The ledger layer and its unit prefix (`events/g001@1`).
    layer: &'static str,
    /// The compiler tool that plans and applies this layer (`zt-digest event`).
    tool: &'static str,
    /// The work folder under the prepared out folder (`<out>/events`).
    work_dir: &'static str,
    completion_kind: &'static str,
    /// The Card type every created Card must have.
    card_type: &'static str,
    /// The kind word of the Link each supported Idea Card gets.
    link_kind: &'static str,
    /// The zt-check rules the accepted Cards must pass.
    rules: &'static str,
    /// A created Event Card is filed beside its first supported Card; a Finding Card
    /// under its group anchor.
    placement: Placement,
    /// The frozen source of the layer's candidate enumeration.
    candidate_source: &'static str,
}

#[derive(PartialEq)]
enum Placement {
    Support,
    Anchor,
}

const EVENTS: Empirical = Empirical {
    layer: "events",
    tool: "event",
    work_dir: "events",
    completion_kind: "events_completion",
    card_type: "event",
    link_kind: "案例",
    rules: "E1,E2,E5",
    placement: Placement::Support,
    candidate_source: "artifact",
};

const FINDINGS: Empirical = Empirical {
    layer: "findings",
    tool: "finding",
    work_dir: "findings",
    completion_kind: "findings_completion",
    card_type: "finding",
    link_kind: "印证",
    rules: "F1,F2,F3,F4",
    placement: Placement::Anchor,
    candidate_source: "sections",
};

const EMPIRICAL: [&Empirical; 2] = [&EVENTS, &FINDINGS];

fn empirical_unit(layer: &Empirical, number: u64) -> String {
    format!("{}/g{number:03}@1", layer.layer)
}

fn empirical_group_key(number: u64) -> String {
    format!("g{number:03}")
}

fn empirical_candidate_id(number: u64, index: usize) -> String {
    format!("{}/c{:03}", empirical_group_key(number), index + 1)
}

/// The unit and group of a registered Events/Findings layer unit, if `unit` is one.
fn empirical_unit_of(unit: &str) -> Option<(&'static Empirical, u64)> {
    for layer in EMPIRICAL {
        if let Some(number) = unit
            .strip_prefix(layer.layer)
            .and_then(|rest| rest.strip_prefix("/g"))
            .and_then(|rest| rest.strip_suffix("@1"))
            .and_then(|rest| rest.parse::<u64>().ok())
            .filter(|number| *number > 0)
        {
            return Some((layer, number));
        }
    }
    None
}

fn prepared_group(p: &Prepared, number: u64) -> Result<&Value> {
    p.contract["groups"]
        .as_array()
        .context("missing prepared groups")?
        .iter()
        .find(|group| group["n"].as_u64() == Some(number))
        .context("prepared group does not exist")
}

/// One declared candidate: its stable id, the source span it was read from, and its text.
struct Candidate {
    id: String,
    src: String,
    text: String,
}

impl Candidate {
    fn value(&self) -> Value {
        json!({"id": self.id, "src": self.src, "text": self.text})
    }
}

/// The frozen event candidates of one group: the prepared `events.json` artifact rows of
/// that group, in order, as the preparation enumerated them.
fn event_candidates(p: &Prepared, number: u64) -> Result<Vec<Candidate>> {
    let record = &p.contract["artifacts"]["events"];
    check_file(record, "events candidate artifact")?;
    let value = util::read_json(field(record, "path")?).context("invalid events candidate artifact")?;
    let rows = value
        .as_array()
        .context("the events candidate artifact is not an array")?;
    let mut candidates: Vec<Candidate> = Vec::new();
    for row in rows {
        if ps::int(prop(row, "group"))? != number as i64 {
            continue;
        }
        let (src, text) = (s(prop(row, "src")), s(prop(row, "text")));
        if src.trim().is_empty() {
            bail!("the events candidate artifact holds a candidate without a source span");
        }
        let index = candidates.len();
        candidates.push(Candidate {
            id: empirical_candidate_id(number, index),
            src,
            text,
        });
    }
    Ok(candidates)
}

/// The frozen result candidates of one group: its prepared section lines that report data
/// with a number, in order and at most `cap` per group, exactly as the compiler plans them.
fn finding_candidates(p: &Prepared, number: u64, cap: usize) -> Result<Vec<Candidate>> {
    let group = prepared_group(p, number)?;
    let mut candidates: Vec<Candidate> = Vec::new();
    for section in group["sections"]
        .as_array()
        .context("missing group source sections")?
    {
        if candidates.len() >= cap {
            break;
        }
        check_file(section, "group source section")?;
        let path = field(section, "path")?;
        let name = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .context("group source section has no file name")?;
        let text = util::read_text(path)?;
        for (index, line) in text.lines().enumerate() {
            if candidates.len() >= cap {
                break;
            }
            if let Some(clip) = crate::finding::candidate(line) {
                let place = candidates.len();
                candidates.push(Candidate {
                    id: empirical_candidate_id(number, place),
                    src: format!("sections/{name}:{}", index + 1),
                    text: clip,
                });
            }
        }
    }
    Ok(candidates)
}

/// The layer's accepted plan manifest and its file record: the candidates of every planned
/// group with stable ids and source spans, and the jobs that carry them. It must name this
/// prepared staging folder, so it cannot be reused from another run.
fn empirical_plan(p: &Prepared, layer: &Empirical) -> Result<(Value, Value)> {
    let path = p.out.join(layer.work_dir).join("plan.json");
    if !path.exists() {
        bail!(
            "missing accepted {} candidate manifest `{}`: run zt-digest {} -Step plan before completing the unit",
            layer.layer,
            path.display(),
            layer.tool
        );
    }
    let bytes = std::fs::read(&path)?;
    let plan: Value = serde_json::from_slice(&bytes).context("invalid accepted plan manifest")?;
    let staging = std::fs::canonicalize(&p.out)?.to_string_lossy().into_owned();
    if plan["schema_version"] != 1
        || plan["layer"] != layer.layer
        || plan["tool"] != layer.tool
        || plan["staging"] != json!(staging)
        || !plan["candidates"].is_object()
        || plan["jobs"].as_array().is_none()
        || plan["candidate_source"]["kind"] != layer.candidate_source
    {
        bail!(
            "the accepted {} candidate manifest does not describe this prepared run",
            layer.layer
        );
    }
    let record = json!({"path":std::fs::canonicalize(&path)?.to_string_lossy(),"sha256":canon::sha256_hex(&bytes)});
    Ok((plan, record))
}

/// The compiler's own state: every applied job keeps its answer, the candidate entries it
/// accepted and the entries it refused, and a failed job stays under `partial`. A missing
/// state is no applied answer at all, which the job checks below report precisely.
fn empirical_state(p: &Prepared, layer: &Empirical) -> Result<Value> {
    let path = p.out.join(layer.work_dir).join("state.json");
    if !path.exists() {
        return Ok(json!({"applied": {}, "partial": {}}));
    }
    let state = util::read_json(&path).context("invalid layer compiler state")?;
    if state["applied"].as_object().is_none() || state["partial"].as_object().is_none() {
        bail!("incomplete {} compiler state: applied and partial must be objects", layer.layer);
    }
    Ok(state)
}

/// The accepted answer of one applied job: its bytes are still the ones it was applied from.
fn saved_answer(path: &str) -> Result<Value> {
    let path = std::fs::canonicalize(path)
        .with_context(|| format!("missing applied answer file `{path}`"))?;
    let bytes = std::fs::read(&path)?;
    Ok(json!({"path":path.to_string_lossy(),"sha256":canon::sha256_hex(&bytes)}))
}

/// The producer contract: the group's digest unit is a done completion of this group and the
/// accepted plan hash the layer unit was registered against.
fn empirical_producer(p: &Prepared, inputs: &Value, layer: &Empirical, number: u64) -> Result<()> {
    let unit = empirical_unit(layer, number);
    let producer = format!("digest/g{number:03}@1");
    if field(inputs, "producer")? != producer {
        bail!("{unit} input does not name its group producer");
    }
    if canon::hash_value(prepared_group(p, number)?) != canon::hash_value(&inputs["group"]) {
        bail!("{unit} frozen group contract no longer matches the prepared group");
    }
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let row: Option<(String, String, Option<String>)> = conn
        .query_row(
            "SELECT state,input_hash,result_json FROM units WHERE id=?1",
            [&producer],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let (state, hash, result) =
        row.with_context(|| format!("{unit} producer {producer} is not registered"))?;
    if state != "done" {
        bail!("{unit} producer {producer} is {state}, not a checked completion");
    }
    if inputs["producer_input_hash"] != json!(hash) {
        bail!("{unit} producer input hash changed");
    }
    let result: Value = serde_json::from_str(
        result
            .as_deref()
            .with_context(|| format!("{unit} producer result is missing"))?,
    )?;
    if inputs["plan_hash"] != result["plan_hash"] {
        bail!("{unit} producer plan hash changed");
    }
    Ok(())
}

/// The kinded Link every supported Idea Card holds for one accepted Card.
fn empirical_link(idea: &Value, layer: &Empirical, card: &str) -> Result<bool> {
    let body = idea["body"].as_str().unwrap_or_default();
    Ok(body.lines().any(|line| {
        line.trim_start()
            .starts_with(&format!("{} [[{card}]]", layer.link_kind))
    }))
}

/// The stated value of one fixed Card label: its own text, or the first list item a label
/// introduces (`结果：` followed by `- …`). None when the label is absent or empty.
fn stated_label(body: &str, name: &str) -> Option<String> {
    fn value_of(line: &str, name: &str) -> Option<String> {
        let mut text = line.trim();
        if let Some(rest) = text.strip_prefix("- ").or_else(|| text.strip_prefix("* ")) {
            text = rest;
        }
        let text = text.strip_prefix("**").unwrap_or(text);
        let rest = text.strip_prefix(name)?;
        let rest = rest.strip_prefix("**").unwrap_or(rest);
        let rest = rest.strip_prefix('：').or_else(|| rest.strip_prefix(':'))?;
        Some(rest.trim().to_string())
    }
    let lines: Vec<&str> = body.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let Some(value) = value_of(line, name) else {
            continue;
        };
        if !value.is_empty() {
            return Some(value);
        }
        // A label that introduces a list states its value in the first list item under it.
        return lines[index + 1..]
            .iter()
            .find(|line| !line.trim().is_empty())
            .and_then(|line| {
                let item = line.trim();
                [item.strip_prefix("- "), item.strip_prefix("* ")]
                    .into_iter()
                    .flatten()
                    .map(str::trim)
                    .find(|item| !item.is_empty())
                    .map(str::to_string)
            });
    }
    None
}

/// The fixed labels every accepted Card of one layer must state with a value.
fn empirical_labels(layer: &Empirical) -> &'static [&'static str] {
    match layer.layer {
        "events" => &["时间", "地点", "参与者", "经过"],
        _ => &["测量", "数据", "方法", "结果", "局限"],
    }
}

/// The complete checked outcome of one empirical group: the accepted candidate manifest
/// against the frozen source, every job applied with no pending answer, every accepted Card
/// reread, and its layer rules.
fn empirical_result(
    p: &Prepared,
    zt: &mut Zt,
    layer: &Empirical,
    number: u64,
    token: i64,
    input_hash: &str,
    inputs: &Value,
) -> Result<Value> {
    use std::collections::{BTreeMap, BTreeSet};
    let unit = empirical_unit(layer, number);
    let key = field(&p.contract, "citation_key")?.to_string();
    let anchor = field(prepared_group(p, number)?, "anchor")?.to_string();
    let (plan, plan_record) = empirical_plan(p, layer)?;
    let state = empirical_state(p, layer)?;
    let owned = frozen_owned(p, inputs, &format!("{} unit", layer.layer))?;
    // 1. The declared candidates, recomputed from the frozen source.
    let declared = match layer.candidate_source {
        "artifact" => {
            if plan["candidate_source"]["path"] != p.contract["artifacts"]["events"]["path"]
                || plan["candidate_source"]["sha256"] != p.contract["artifacts"]["events"]["sha256"]
            {
                bail!(
                    "the accepted events candidate manifest names another source than the prepared events artifact"
                );
            }
            event_candidates(p, number)?
        }
        _ => {
            let cap = plan["candidate_source"]["max_candidates"]
                .as_u64()
                .context("the accepted finding plan holds no candidate bound")?;
            if cap == 0 || cap > 256 {
                bail!("the accepted finding plan holds an unusable candidate bound {cap}");
            }
            finding_candidates(p, number, cap as usize)?
        }
    };
    let declared_values: Vec<Value> = declared.iter().map(Candidate::value).collect();
    let manifest_candidates = plan["candidates"][empirical_group_key(number)].clone();
    let manifest_candidates = if manifest_candidates.is_null() {
        json!([])
    } else {
        manifest_candidates
    };
    if manifest_candidates != json!(declared_values) {
        bail!(
            "the accepted {} candidate manifest no longer describes group {number} of the frozen source",
            layer.layer
        );
    }
    // 2. The jobs that carry this group, and the applied evidence of each.
    let mut covering: Vec<Value> = Vec::new();
    let mut job_ids = BTreeSet::new();
    for job in plan["jobs"].as_array().context("missing planned jobs")? {
        let id = field(job, "id")?;
        if id.len() > 40 || !id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
            bail!("the accepted plan holds an unreadable job id");
        }
        if !job_ids.insert(id.to_string()) {
            bail!("the accepted plan names job {id} twice");
        }
        let answer = field(job, "answer")?;
        if !Path::new(answer).starts_with(&p.out) {
            bail!("the accepted plan's answer `{answer}` escaped the prepared output directory");
        }
        let groups = job["groups"]
            .as_array()
            .with_context(|| format!("the accepted plan's job {id} enumerates no groups"))?;
        if groups.iter().any(|group| group.as_u64() == Some(number)) {
            covering.push(job.clone());
        }
    }
    let applied = state["applied"]
        .as_object()
        .context("missing applied jobs")?
        .clone();
    let partial = state["partial"]
        .as_object()
        .context("missing pending jobs")?
        .clone();
    let covering_ids: BTreeSet<String> = covering.iter().map(|job| s(prop(job, "id"))).collect();
    let mut accepted: Vec<Value> = Vec::new();
    let mut jobs_evidence: Vec<Value> = Vec::new();
    for (id, entry) in &applied {
        if entry["entries"].as_array().is_none() || entry["refused"].as_array().is_none() {
            bail!(
                "applied {} job {id} holds no accepted candidate manifest; apply it again with the current tool",
                layer.layer
            );
        }
        let saved = saved_answer(field(&entry["answer"], "path")?)?;
        if entry["answer"]["sha256"] != saved["sha256"] {
            bail!(
                "the applied {} answer `{}` changed after it was accepted",
                layer.layer,
                s(prop(&saved, "path"))
            );
        }
        let group_entries = entry["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["group"].as_u64() == Some(number))
            .cloned()
            .collect::<Vec<_>>();
        let group_refused = entry["refused"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["group"].as_u64() == Some(number))
            .cloned()
            .collect::<Vec<_>>();
        if !covering_ids.contains(id) && !(group_entries.is_empty() && group_refused.is_empty()) {
            bail!(
                "{unit} names work of group {number} in {} job {id}, which the accepted plan does not cover",
                layer.layer
            );
        }
        if !covering_ids.contains(id) {
            continue;
        }
        let planned = covering
            .iter()
            .find(|job| s(prop(job, "id")) == *id)
            .context("missing covering job")?;
        if std::fs::canonicalize(field(planned, "answer")?)? != std::fs::canonicalize(field(&saved, "path")?)? {
            bail!("{unit} applied answer path differs from the accepted plan's");
        }
        if let Some(pending) = partial.get(id) {
            bail!("{} job {id} is pending: {}", layer.layer, s(prop(pending, "error")));
        }
        if let Some(item) = group_refused.first() {
            bail!(
                "{unit} holds a refused {} entry of group {number}: {}",
                layer.layer,
                s(prop(item, "fault"))
            );
        }
        // A refused entry for this group is unresolved work, so reaching here means none.
        accepted.extend(group_entries.iter().cloned());
        jobs_evidence.push(json!({
            "id": id,
            "answer": saved,
            "entries": group_entries.len(),
            "refused": group_refused.len()
        }));
    }
    for job in &covering {
        let id = s(prop(job, "id"));
        if !applied.contains_key(&id) {
            if let Some(pending) = partial.get(&id) {
                bail!("{} job {id} is pending: {}", layer.layer, s(prop(pending, "error")));
            }
            bail!(
                "{} job {id} covers group {number} but was never applied; apply it before completing the unit",
                layer.layer
            );
        }
    }
    if !declared.is_empty() && covering.is_empty() {
        bail!(
            "the accepted {} candidate manifest lists {} candidates for group {number} but no planned job covers it",
            layer.layer,
            declared.len()
        );
    }
    if declared.is_empty() && !accepted.is_empty() {
        bail!(
            "group {number} has accepted {} Cards but the accepted plan declares no candidate",
            layer.layer
        );
    }
    let disposition = if !accepted.is_empty() {
        "verified"
    } else if declared.is_empty() {
        "no_candidates"
    } else {
        "no_cards"
    };
    // 3. Reread every accepted Card and every supported Card it names.
    let mut addresses: BTreeSet<String> = BTreeSet::new();
    for entry in &accepted {
        addresses.insert(field(entry, "card")?.to_string());
        for support in entry["supports"]
            .as_array()
            .with_context(|| format!("{unit} accepted entry has no supports"))?
        {
            addresses.insert(s(support));
        }
    }
    let address_list: Vec<String> = addresses.into_iter().collect();
    let read = zt.cards(&address_list, "text")?;
    let mut cards: BTreeMap<String, Value> = BTreeMap::new();
    for card in &read.cards {
        let at = field(card, "address")?.to_string();
        if !address_list.iter().any(|address| *address == at) {
            bail!("the archive read returned the unrequested Card `{at}`");
        }
        cards.insert(at, card.clone());
    }
    for at in &address_list {
        if !cards.contains_key(at) {
            bail!("the accepted {unit} work names `{at}`, which does not exist");
        }
    }
    let declared_by_id: BTreeMap<String, Candidate> =
        declared.iter().map(|item| (item.id.clone(), Candidate { id: item.id.clone(), src: item.src.clone(), text: item.text.clone() })).collect();
    let mut accepted_out: Vec<Value> = Vec::new();
    let mut links: Vec<Value> = Vec::new();
    let mut created: BTreeSet<String> = BTreeSet::new();
    let mut fingerprints: BTreeMap<String, String> = BTreeMap::new();
    let mut trees: BTreeSet<String> = BTreeSet::new();
    for entry in &accepted {
        let card_at = field(entry, "card")?;
        let candidate = field(entry, "candidate")?;
        let candidate_entry = declared_by_id
            .get(candidate)
            .with_context(|| format!("{unit} accepted an undeclared candidate `{candidate}`"))?;
        if candidate_entry.src != field(entry, "src")? {
            bail!("{unit} candidate `{candidate}` changed its source span");
        }
        let card = cards
            .get(card_at)
            .with_context(|| format!("{unit} accepted Card `{card_at}` is missing"))?;
        if card["workspace"] != "agent" {
            bail!("accepted {unit} Card `{card_at}` is not in the agent Workspace");
        }
        if card["type"] != layer.card_type {
            bail!("accepted {unit} Card `{card_at}` is not a {} Card", layer.card_type);
        }
        if owned.iter().any(|address| address == card_at) {
            bail!("accepted {unit} work `{card_at}` is a group Card, not a new Card");
        }
        let merged = entry["merged"] == true;
        let source = format!(
            "来源：[[{}]] {}",
            if layer.placement == Placement::Support { &key } else { &anchor },
            field(entry, "section")?
        );
        let body = card["body"].as_str().context("accepted Card has no body")?;
        if !body.lines().any(|line| line.trim() == source) {
            bail!("accepted {unit} Card `{card_at}` holds no accepted source line `{source}`");
        }
        for label in empirical_labels(layer) {
            if stated_label(body, label).is_none() {
                bail!("accepted {unit} Card `{card_at}` states no {label} value");
            }
        }
        if !merged {
            if card["title"].as_str().unwrap_or_default() != field(entry, "title")? {
                bail!("accepted {unit} Card `{card_at}` title changed");
            }
            if card["tree"] != json!(key) {
                bail!("accepted {unit} Card `{card_at}` is outside the book tree");
            }
            let supports = entry["supports"].as_array().unwrap();
            let target = match layer.placement {
                Placement::Support => supports
                    .first()
                    .and_then(Value::as_str)
                    .with_context(|| format!("{unit} accepted entry names no support"))?,
                Placement::Anchor => &anchor,
            };
            if card["parent_address"] != json!(target) {
                bail!("accepted {unit} Card `{card_at}` is not filed at its accepted place");
            }
            if !created.insert(card_at.to_string()) {
                bail!("two accepted {unit} entries created the same Card `{card_at}`");
            }
        }
        for support in entry["supports"].as_array().unwrap() {
            let at = support
                .as_str()
                .with_context(|| format!("{unit} accepted entry holds an invalid support"))?;
            if !owned.iter().any(|address| address == at) {
                bail!("{unit} accepted entry supports `{at}`, which is not a group Card");
            }
            let idea = cards
                .get(at)
                .with_context(|| format!("{unit} supported Card `{at}` is missing"))?;
            if workspace_of(idea) != "agent" {
                bail!("{unit} supported Card `{at}` is not in the agent Workspace");
            }
            if !empirical_link(idea, layer, card_at)? {
                bail!(
                    "supported Card `{at}` holds no `{} [[{card_at}]]` Link to the accepted Card",
                    layer.link_kind
                );
            }
            links.push(json!({"card":card_at,"idea":at,"kind":layer.link_kind}));
            fingerprints.insert(at.to_string(), crate::calls::authored_fingerprint(idea));
            if let Some(tree) = idea["tree"].as_str() {
                trees.insert(tree.to_string());
            }
        }
        fingerprints.insert(card_at.to_string(), crate::calls::authored_fingerprint(card));
        if let Some(tree) = card["tree"].as_str() {
            trees.insert(tree.to_string());
        }
        accepted_out.push(json!({
            "candidate": candidate,
            "card": card_at,
            "title": card["title"],
            "merged": merged,
            "supports": entry["supports"],
            "source": source,
            "fingerprint": crate::calls::authored_fingerprint(card)
        }));
    }
    // 4. The layer's own Card rules over the accepted set.
    let check_cards: Vec<String> = created
        .iter()
        .cloned()
        .chain(
            accepted_out
                .iter()
                .filter(|entry| entry["merged"] == true)
                .filter_map(|entry| entry["card"].as_str().map(str::to_string)),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let self_check = if check_cards.is_empty() {
        json!({"ran":true,"rules":layer.rules,"count":0,"reported_count":0,"faults":[],"advisories":[],"lines":[]})
    } else {
        let check = crate::check::Check {
            cards: check_cards.join(","),
            book: key.clone(),
            rules: layer.rules.to_string(),
            ..Default::default()
        };
        let lines = check.run(zt)?;
        let mut checked = checked_lines(lines, false)?;
        checked["rules"] = json!(layer.rules);
        checked
    };
    if self_check["count"] != 0 {
        bail!(
            "the accepted {} Cards fail their rules: {}",
            layer.layer,
            self_check["faults"][0]
        );
    }
    if zt.archive_view()?["archive_revision"].as_u64() != Some(read.archive_revision) {
        bail!("read_view_changed: {unit} Card read");
    }
    let candidates_out = declared
        .iter()
        .map(|candidate| {
            let accepted = accepted_out
                .iter()
                .find(|entry| entry["candidate"] == json!(candidate.id));
            match accepted {
                Some(entry) => json!({"id":candidate.id,"src":candidate.src,"text":candidate.text,"outcome":"card","card":entry["card"]}),
                None => json!({"id":candidate.id,"src":candidate.src,"text":candidate.text,"outcome":"not_accepted"}),
            }
        })
        .collect::<Vec<_>>();
    let tree_list: Vec<String> = trees.iter().cloned().collect();
    let list = json!({
        "schema_version": 1,
        "kind": format!("{}_list", layer.layer),
        "unit": unit,
        "token": token,
        "group": empirical_group_key(number),
        "anchor": anchor,
        "disposition": disposition,
        "candidate_source": plan["candidate_source"],
        "candidates": candidates_out,
        "accepted": accepted_out,
        "jobs": jobs_evidence,
        "refused": json!([]),
        "self_check": self_check,
        "archive_revision": read.archive_revision
    });
    let inputs_manifest = json!({
        "schema_version": 1,
        "kind": format!("{}_inputs", layer.layer),
        "unit": unit,
        "tree": key,
        "trees": tree_list,
        "anchor": anchor,
        "link": {"cards": fingerprint_addresses(&fingerprints), "links": links},
        "organize": {"anchor": anchor, "touched_trees": tree_list, "accepted": accepted_out.iter().map(|entry| entry["card"].clone()).collect::<Vec<_>>()},
        "hubs": {"touched_trees": tree_list}
    });
    Ok(json!({
        "schema_version": 1,
        "kind": layer.completion_kind,
        "unit": unit,
        "token": token,
        "state": "done",
        "complete": true,
        "reused": false,
        "input_hash": input_hash,
        "unit_input_hash": input_hash,
        "producer": field(inputs, "producer")?,
        "producer_input_hash": field(inputs, "producer_input_hash")?,
        "plan_hash": field(inputs, "plan_hash")?,
        "anchor": anchor,
        "disposition": disposition,
        "candidate_source": plan["candidate_source"],
        "manifest": plan_record,
        "expected_jobs": covering.len(),
        "candidates": list["candidates"],
        "accepted": list["accepted"],
        "jobs": jobs_evidence,
        "refused": json!([]),
        "links": links,
        "owned": owned,
        "trees": tree_list,
        "self_check": self_check,
        "card_fingerprints": fingerprints,
        "archive_revision": read.archive_revision,
        "list": list,
        "inputs_manifest": inputs_manifest
    }))
}

/// The Card's workspace id (`agent` or `human`).
fn workspace_of(card: &Value) -> &str {
    card["workspace"].as_str().unwrap_or_default()
}

/// The sorted addresses a fingerprint map names.
fn fingerprint_addresses(fingerprints: &std::collections::BTreeMap<String, String>) -> Vec<String> {
    fingerprints.keys().cloned().collect()
}

/// The required output registration of one completed empirical unit.
fn empirical_output_keys(result: &Value) -> Result<std::collections::BTreeSet<String>> {
    if result["schema_version"] != 1
        || result["state"] != "done"
        || result["complete"] != true
        || result["self_check"]["ran"] != true
        || result["self_check"]["count"] != 0
    {
        bail!("incomplete typed Events/Findings completion evidence");
    }
    let layer = empirical_unit_of(&s(&result["unit"])).map(|(layer, _)| layer).context("invalid Events/Findings completion unit")?;
    if result["kind"] != layer.completion_kind {
        bail!("Events/Findings completion evidence names another layer");
    }
    let files = result["files"]
        .as_object()
        .context("missing Events/Findings evidence enumeration")?;
    let names = ["list", "source", "inputs", "check", "completion"];
    if files.len() != names.len() || files.keys().any(|key| !names.contains(&key.as_str())) {
        bail!("incomplete Events/Findings evidence enumeration");
    }
    let mut keys = std::collections::BTreeSet::from(names.map(str::to_string));
    for name in names {
        field(&files[name], "path")?;
        field(&files[name], "sha256")?;
    }
    for at in result["card_fingerprints"]
        .as_object()
        .context("missing Events/Findings Card enumeration")?
        .keys()
    {
        keys.insert(format!("card/{at}"));
    }
    Ok(keys)
}

/// Repeated completion rereads the accepted Cards and re-runs their rules; a changed Card or
/// a changed answer makes the retained completion stale.
fn empirical_done(p: &Prepared, zt: &mut Zt, layer: &Empirical, number: u64, token: i64) -> Result<Value> {
    let unit = empirical_unit(layer, number);
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let raw: String = tx.query_row(
        "SELECT result_json FROM units WHERE id=?1 AND state='done' AND claim_token=?2",
        params![unit, token],
        |row| row.get(0),
    )?;
    let mut result: Value = serde_json::from_str(&raw)?;
    let rows = tx
        .prepare("SELECT output_key,kind,locator,fingerprint FROM outputs WHERE unit_id=?1 ORDER BY output_key")?
        .query_map([&unit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    tx.commit()?;
    drop(conn);
    let expected = empirical_output_keys(&result)?;
    if rows
        .iter()
        .map(|(key, _, _, _)| key.clone())
        .collect::<std::collections::BTreeSet<_>>()
        != expected
    {
        bail!("incomplete Events/Findings completion output registration");
    }
    for (key, kind, locator, fingerprint) in &rows {
        match kind.as_str() {
            "file" => {
                if result["files"][key]["path"] != *locator
                    || result["files"][key]["sha256"] != *fingerprint
                {
                    bail!("changed Events/Findings evidence registration");
                }
                check_file(
                    &json!({"path":locator,"sha256":fingerprint}),
                    "Events/Findings completion evidence",
                )?;
            }
            "card" => {}
            _ => bail!("unknown Events/Findings evidence kind"),
        }
    }
    let inputs = frozen_inputs(p, &unit)?;
    if canon::hash_value(&inputs) != field(&result, "unit_input_hash")? {
        bail!("{unit} input hash mismatch");
    }
    empirical_producer(p, &inputs, layer, number)?;
    let current = empirical_result(p, zt, layer, number, token, field(&result, "unit_input_hash")?, &inputs)?;
    if current["candidates"] != result["candidates"]
        || current["accepted"] != result["accepted"]
        || current["disposition"] != result["disposition"]
        || current["jobs"] != result["jobs"]
    {
        bail!(
            "completed {unit} no longer describes its accepted Cards; keep the checked result and re-run from a new claim"
        );
    }
    if current["self_check"]["lines"] != result["self_check"]["lines"] {
        bail!("completed {unit} rules no longer describe the accepted Cards");
    }
    if current["card_fingerprints"] != result["card_fingerprints"] {
        bail!("completed {unit} Cards changed; re-run the layer from a new claim");
    }
    result["reused"] = json!(true);
    Ok(result)
}

fn empirical_finish(
    p: &Prepared,
    zt: &mut Zt,
    layer: &Empirical,
    number: u64,
    token: i64,
    check_only: bool,
) -> Result<Value> {
    let unit = empirical_unit(layer, number);
    finish_claim(p, &unit, token)?;
    let _guard = unit_guard(p, &unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, &unit, token)?;
    let inputs = frozen_inputs(p, &unit)?;
    if canon::hash_value(&inputs) != claimed.input_hash {
        bail!("{unit} input hash mismatch");
    }
    empirical_producer(p, &inputs, layer, number)?;
    if done {
        let mut result = empirical_done(p, zt, layer, number, token)?;
        if check_only {
            result["check_only"] = json!(true);
            result["applied"] = json!(false);
        }
        return Ok(result);
    }
    let mut result = empirical_result(p, zt, layer, number, token, &claimed.input_hash, &inputs)?;
    if check_only {
        result["check_only"] = json!(true);
        result["applied"] = json!(false);
        return Ok(result);
    }
    renew(p, &unit, token)?;
    let tag = format!("g{number:03}");
    let list_record = publish(
        &p.out.join(format!("{}-list-{tag}.json", layer.layer)),
        &serde_json::to_string_pretty(&result["list"])?,
    )?;
    let source: String = result["accepted"]
        .as_array()
        .context("missing accepted Cards")?
        .iter()
        .map(|entry| {
            Ok(format!(
                "{}\t{}\n",
                field(entry, "card")?,
                field(entry, "candidate")?
            ))
        })
        .collect::<Result<String>>()?;
    let source_record = publish(
        &p.out.join(format!("{}-src-{tag}.txt", layer.layer)),
        &source,
    )?;
    let inputs_record = publish(
        &p.out.join(format!("{}-inputs-{tag}.json", layer.layer)),
        &serde_json::to_string_pretty(&result["inputs_manifest"])?,
    )?;
    let check_record = publish(
        &p.out.join(format!("{}-check-{tag}.json", layer.layer)),
        &serde_json::to_string_pretty(
            &json!({"schema_version":1,"unit":unit,"token":token,"input_hash":claimed.input_hash,"manifest":result["manifest"],"disposition":result["disposition"],"self_check":result["self_check"],"archive_revision":result["archive_revision"]}),
        )?,
    )?;
    result["files"] = json!({
        "list": list_record,
        "source": source_record,
        "inputs": inputs_record,
        "check": check_record
    });
    let completion = publish(
        &p.out.join(format!("{}-completion-{tag}.json", layer.layer)),
        &serde_json::to_string_pretty(&result)?,
    )?;
    result["files"]["completion"] = completion.clone();
    empirical_output_keys(&result)?;
    for name in ["list", "source", "inputs", "check", "completion"] {
        check_file(&result["files"][name], "Events/Findings completion evidence")?;
    }
    let final_view = zt.archive_view()?;
    if final_view["archive_identity"] != p.contract["archive_identity"]
        || final_view["archive_revision"].as_u64()
            != Some(result["archive_revision"].as_u64().unwrap())
    {
        bail!("read_view_changed: Events/Findings completion evidence changed before ledger commit");
    }
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, &unit, token)?;
    for name in ["list", "source", "inputs", "check", "completion"] {
        let record = &result["files"][name];
        output(
            &tx,
            &unit,
            name,
            "file",
            field(record, "path")?,
            field(record, "sha256")?,
            record,
        )?;
    }
    for (at, fingerprint) in result["card_fingerprints"]
        .as_object()
        .context("missing Events/Findings Card enumeration")?
    {
        output(
            &tx,
            &unit,
            &format!("card/{at}"),
            "card",
            at,
            fingerprint
                .as_str()
                .context("invalid Events/Findings Card fingerprint")?,
            &json!({"layer":layer.layer}),
        )?;
    }
    completion_fault(p, "after_outputs")?;
    tx.execute("UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",params![unit,canon::canonical_json(&result),canon::utc_now_o()])?;
    event(
        &tx,
        &unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"disposition":result["disposition"],"completion":completion,"accepted":result["accepted"],"trees":result["trees"],"jobs":result["jobs"],"self_check":{"rules":result["self_check"]["rules"],"count":result["self_check"]["count"]}}),
    )?;
    completion_fault(p, "before_commit")?;
    tx.commit()?;
    completion_fault(p, "after_commit")?;
    Ok(result)
}

fn empirical_finish_cli(argv: &[String], layer: &Empirical, name: &str) -> Result<i32> {
    let parsed = (|| -> Result<(String, u64, i64, bool)> {
        let a = Args::parse(argv, &["Out", "Group", "Token"], &["CheckOnly"])?;
        if !a.positional.is_empty() || a.required("Out")?.trim().is_empty() {
            bail!("explicit -Out is required");
        }
        let group = a
            .required("Group")?
            .parse::<u64>()
            .ok()
            .filter(|number| *number > 0)
            .context("-Group must be positive")?;
        let token = a
            .required("Token")?
            .parse::<i64>()
            .ok()
            .filter(|number| *number > 0)
            .context("-Token must be positive")?;
        Ok((a.str("Out"), group, token, a.flag("CheckOnly")))
    })();
    let (out, group, token, check_only) = match parsed {
        Ok(value) => value,
        Err(error) => {
            eprintln!("zt-digest {name}: {error}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let result = empirical_finish(&p, &mut zt, layer, group, token, check_only)?;
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}

pub fn events_finish_cli(argv: &[String]) -> Result<i32> {
    empirical_finish_cli(argv, &EVENTS, "events-finish")
}

pub fn findings_finish_cli(argv: &[String]) -> Result<i32> {
    empirical_finish_cli(argv, &FINDINGS, "findings-finish")
}

// ==== The Models/model unit (R218, R235, R236) =============================================

const MODEL_STATES: [&str; 3] = ["built", "derived", "reviewed"];

/// One registered Model unit and its frozen inputs, by the Model Card it builds.
fn model_unit_for_header(conn: &Connection, header: &str) -> Result<Option<(String, Value)>> {
    let mut statement = conn.prepare("SELECT id,inputs_json FROM units WHERE layer='models' ORDER BY id")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (id, raw) in rows {
        let inputs: Value = serde_json::from_str(&raw)?;
        if inputs["header"] == json!(header) {
            return Ok(Some((id, inputs)));
        }
    }
    Ok(None)
}

/// The compiler's own work folder for one model: `<out>/models/<batch>/<slug>/plan.json`.
fn model_plan_dir(p: &Prepared, header: &str) -> Result<PathBuf> {
    let slug: String = header
        .chars()
        .map(|c| if matches!(c, '/' | '|' | '\\') { '_' } else { c })
        .collect();
    let root = p.out.join("models");
    let mut dirs = Vec::new();
    if root.is_dir() {
        for batch in std::fs::read_dir(&root)? {
            let dir = batch?.path().join(&slug);
            if dir.join("plan.json").is_file() {
                dirs.push(dir);
            }
        }
    }
    dirs.sort();
    match dirs.len() {
        1 => Ok(dirs.remove(0)),
        0 => bail!("no accepted plan for `{header}` under {}; plan and build the model first", root.display()),
        _ => bail!(
            "more than one accepted plan for `{header}` under {}; keep one work folder",
            root.display()
        ),
    }
}

/// The accepted plan (its exact bytes and hash) and the batch's compiler state.
fn model_plan_state(p: &Prepared, header: &str) -> Result<(PathBuf, String, Value, Value)> {
    let dir = model_plan_dir(p, header)?;
    let bytes = std::fs::read(dir.join("plan.json"))?;
    let plan_hash = canon::sha256_hex(&bytes);
    let plan: Value = serde_json::from_str(
        std::str::from_utf8(&bytes).context("plan.json must be strict UTF-8")?,
    )
    .context("invalid plan.json")?;
    let batch = dir.parent().context("plan folder outside a model batch")?;
    let state_path = batch.join("state.json");
    if !state_path.is_file() {
        bail!("no compiler state at {}; build the model first", state_path.display());
    }
    let state = util::read_json(state_path.to_string_lossy().as_ref())?;
    Ok((dir, plan_hash, plan, state))
}

fn model_todo(body: &str) -> bool {
    body.replace('\r', "").split('\n').any(|line| line.trim() == "待写")
}

fn require_step(entry: &Value, name: &str) -> Result<Value> {
    let step = &entry["steps"][name];
    if !step.is_object() {
        bail!("missing `{name}` command evidence in the model's step state");
    }
    Ok(step.clone())
}

/// The `**结构**` section of the Model Card: the compiler's structure contribution.
fn structure_lines(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in body.replace('\r', "").split('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("**结构**") {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("**") || trimmed.starts_with('#') || trimmed.starts_with("来源") {
            break;
        }
        out.push(trimmed.to_string());
    }
    out
}

/// R218: the complete checked outcome of one Model unit, from its own compiler's step
/// state and the authored Cards. It writes nothing; the caller publishes and commits.
fn model_evidence(p: &Prepared, zt: &mut Zt, unit: &str, token: i64, inputs: &Value) -> Result<Value> {
    use std::collections::{BTreeMap, BTreeSet};
    let header = field(inputs, "header")?;
    let base = &inputs["producer"];
    let producer = field(base, "producer")?;
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT state,input_hash FROM units WHERE id=?1",
            [producer],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (state, hash) = row.with_context(|| format!("Model unit producer {producer} is not registered"))?;
    if state != "done" {
        bail!("Model unit producer {producer} is {state}, not a checked completion");
    }
    if base["producer_input_hash"] != json!(hash) {
        bail!("Model unit producer input hash changed");
    }
    let (_dir, plan_hash, plan, compiler) = model_plan_state(p, header)?;
    if !ps::eq(&s(&plan["model"]), header) {
        bail!("the accepted plan describes `{}`, not `{header}`", s(&plan["model"]));
    }
    let entry = &compiler["models"][header];
    if !entry.is_object() {
        bail!("no compiler state for `{header}`; plan and build the model first");
    }
    let status = s(&entry["status"]);
    if !ps::is_in(&status, &MODEL_STATES) {
        bail!("the model's build is `{status}`, not a completed build; plan and build the model first");
    }
    if s(&entry["plan_hash"]) != plan_hash {
        bail!("plan.json changed after the accepted build; keep the accepted plan or rebuild the model as a new revision");
    }
    let ids = entry["ids"]
        .as_object()
        .context("the model's part map is missing; plan and build the model first")?;
    let added = entry["added"].as_object().cloned().unwrap_or_default();
    let plan_parts = plan["parts"].as_array().context("the accepted plan has no parts")?.clone();
    let plan_ids: BTreeSet<String> = plan_parts.iter().map(|part| s(&part["id"])).filter(|id| !id.is_empty()).collect();
    for id in ids.keys() {
        if !plan_ids.contains(id) && !added.contains_key(id) {
            bail!("part id `{id}` is neither in the accepted plan nor a recorded split addition");
        }
    }
    // The step chain every built model must show, with a level-specific no-op.
    let brief = require_step(entry, "brief")?;
    let context_hash = field(&brief, "context_hash")?;
    let context_path = Path::new(field(&brief, "context")?);
    if canon::sha256_hex(&std::fs::read(context_path).context("missing brief context pack")?) != context_hash {
        bail!("the brief's context pack changed after the brief; a source review cannot be claimed from different bytes");
    }
    // R247: the accepted plan's checked source review is the one owner of this model draft's
    // review. It must describe this plan and the exact source pack the architect read.
    let review_path = _dir.join("source-review.json");
    let review = crate::model::source_review::read(
        review_path.to_string_lossy().as_ref(),
        header,
        &plan_hash,
    )
    .with_context(|| {
        format!("no checked source-review artifact for `{header}`; run the model compiler's validate or apply step")
    })?;
    if review["complete"] != json!(true) {
        bail!("the source-review artifact of `{header}` is incomplete; every written part needs a reviewed source span");
    }
    if !ps::eq(&s(&review["context"]["sha256"]), &context_hash) {
        bail!("the source-review artifact of `{header}` describes another context pack");
    }
    let plan_step = require_step(entry, "plan")?;
    if field(&plan_step, "plan_hash")? != plan_hash || plan_step["faults"] != json!(0) {
        bail!("the accepted plan's validation evidence does not describe plan.json");
    }
    let apply = require_step(entry, "apply")?;
    if field(&apply, "plan_hash")? != plan_hash || apply["faults"] != json!(0) {
        bail!("the apply evidence does not describe the accepted plan");
    }
    let report = require_step(entry, "report")?;
    if report["todo"] != json!(0) {
        bail!("the last report names unwritten parts; report again after the fill");
    }
    let derived = plan_parts.iter().any(|part| truthy(&part["derive"]));
    let derive_evidence = entry["steps"]["derive"].is_object()
        || (entry["steps"]["resume"]["round"] == json!("derive"));
    if derived && !derive_evidence {
        bail!("missing derive evidence: the plan has Derivation parts but no derive job was commanded");
    }
    if derived && !entry["steps"]["review"].is_object() {
        bail!("missing review evidence: the plan has Derivation parts but review was not commanded");
    }
    if ps::int(&entry["split"]).unwrap_or(0) > 0 && !entry["steps"]["split"].is_object() {
        bail!("missing split evidence: the model was split but no split command is recorded");
    }
    // The exact part map: every planned id, no unwritten Card, no Card outside the model.
    let mut addresses = vec![header.to_string()];
    let mut part_map = serde_json::Map::new();
    for (id, value) in ids {
        let address = s(value);
        if address.is_empty() {
            bail!("part id `{id}` has no address in the model's part map");
        }
        addresses.push(address.clone());
        part_map.insert(id.clone(), json!(address));
    }
    for part in &plan_parts {
        let id = s(&part["id"]);
        if !id.is_empty() && !ids.contains_key(&id) {
            bail!("the accepted plan's part `{id}` has no address in the model's part map");
        }
    }
    let read = zt.cards(&addresses, "text")?;
    let cards: BTreeMap<String, Value> = read
        .cards
        .iter()
        .map(|card| Ok((field(card, "address")?.to_string(), card.clone())))
        .collect::<Result<_>>()?;
    let head_card = cards
        .get(header)
        .with_context(|| format!("the Model Card `{header}` does not exist"))?;
    if head_card["workspace"] != "agent" {
        bail!("the Model Card `{header}` is not in the agent Workspace");
    }
    // The frozen fingerprint is the group's starting point: the model compiler is meant to
    // rewrite the Model Card, so completion records the current text without pretending the
    // Card is unchanged.
    let model = zt.call("get_model", json!({"at":header}))?;
    let order: BTreeMap<String, Value> = model["build"]["order"]
        .as_array()
        .context("incomplete model build order")?
        .iter()
        .map(|o| (s(&o["address"]), o.clone()))
        .collect();
    let mut part_ids = BTreeMap::new();
    for (id, address) in &part_map {
        let address = s(address);
        let card = cards
            .get(&address)
            .with_context(|| format!("part id `{id}` names `{address}`, which the archive does not hold"))?;
        if card["workspace"] != "agent" {
            bail!("part `{address}` is not in the agent Workspace");
        }
        if !order.contains_key(&address) {
            bail!("part `{address}` (id `{id}`) is mapped but belongs to no model; keep the accepted part map or rebuild the model");
        }
        if model_todo(&s(&card["body"])) {
            bail!("part `{address}` (id `{id}`) is still 待写; resume the model from its first missing part");
        }
        let expected = s(&entry["facets"][id]);
        let actual = s(&card["facet"]);
        if !expected.trim().is_empty() && !ps::eq(expected.trim(), actual.trim()) {
            bail!("part `{address}` has facet `{actual}`, the accepted plan says `{expected}`");
        }
        part_ids.insert(address.clone(), id.clone());
    }
    for address in order.keys() {
        if !part_ids.contains_key(address) {
            bail!("the model holds a part `{address}` that its accepted plan does not map; rebuild the model as a new revision");
        }
    }
    if model_todo(&s(&head_card["body"])) || s(&head_card["body"]).trim().is_empty() {
        bail!("the Model Card is still unwritten; resume the model from its first missing part");
    }
    // R298: a part the review records as an independently supplied step carries one of the two
    // established Chinese markers, and no Card of the model carries a malformed one.
    let mut supplied = Vec::new();
    for part in review["parts"].as_array().context("the source-review artifact has no parts")? {
        let id = s(&part["part"]);
        let outcome = s(&part["outcome"]);
        let address = part_ids
            .iter()
            .find(|(_, mapped)| **mapped == id)
            .map(|(address, _)| address.clone());
        let body = address.as_ref().and_then(|address| cards.get(address)).map(|card| s(&card["body"])).unwrap_or_default();
        if !crate::model::source_review::markers_ok(&body) {
            bail!(
                "part `{id}` carries a malformed supplied-step marker; use {} or {}",
                crate::model::source_review::MARKERS[0],
                crate::model::source_review::MARKERS[1]
            );
        }
        if outcome == "supplied" {
            if !crate::model::source_review::supplied_marked(&body) {
                bail!(
                    "part `{id}` is reviewed as an independently supplied step but carries no {} or {} marker",
                    crate::model::source_review::MARKERS[0],
                    crate::model::source_review::MARKERS[1]
                );
            }
            supplied.push(json!({"part":id,"address":address}));
        }
    }
    if !crate::model::source_review::markers_ok(&s(&head_card["body"])) {
        bail!(
            "the Model Card carries a malformed supplied-step marker; use {} or {}",
            crate::model::source_review::MARKERS[0],
            crate::model::source_review::MARKERS[1]
        );
    }
    let level = s(&model["level"]);
    if !ps::eq(&level, &s(&plan["level"])) || level.is_empty() {
        bail!("the model's level is `{level}`, the accepted plan says `{}`", s(&plan["level"]));
    }
    let build = &model["build"];
    for name in ["closure", "forward", "unanchored", "beyond_level", "reach_in"] {
        if build[name].as_array().is_some_and(|faults| !faults.is_empty()) {
            bail!("the model has build faults ({name}); repair the model before completing it");
        }
    }
    let structure = structure_lines(&s(&head_card["body"]));
    for (address, id) in &part_ids {
        if !structure.iter().any(|line| line.contains(&format!("[[{address}]]"))) {
            bail!("the 结构 list does not name part `{address}` (id `{id}`); keep the compiler's structure contribution");
        }
    }
    // Semantic Link requirements: each planned rests-on/of relation of a part this build
    // wrote must be authored in the Card.
    let pairs: BTreeSet<(String, String)> = model["relations"]
        .as_array()
        .context("incomplete model relations")?
        .iter()
        .map(|r| (s(&r["from"]), s(&r["to"])))
        .collect();
    let resolve = |reference: &str| -> Option<String> {
        if let Some(address) = ids.get(reference).map(s).filter(|address| !address.is_empty()) {
            return Some(address);
        }
        if let Some((other, part)) = reference.split_once('#') {
            let from_state = compiler["models"].get(other).and_then(|m| m["ids"].get(part)).map(s);
            if let Some(address) = from_state.filter(|address| !address.is_empty()) {
                return Some(address);
            }
            let from_refs = entry["refs"].get(reference).and_then(|r| r["address"].as_str()).map(str::to_string);
            if from_refs.is_some() {
                return from_refs;
            }
            return None;
        }
        Some(reference.to_string())
    };
    let mut requirements = Vec::new();
    for part in &plan_parts {
        let id = s(&part["id"]);
        if id.is_empty() || truthy(&part["take"]) || truthy(&part["existing"]) {
            continue;
        }
        let address = ids.get(&id).map(s).unwrap_or_default();
        if address.is_empty() {
            continue;
        }
        let mut wanted = Vec::new();
        for rest in list(&part["rests"]) {
            let reference = s(&ps::at(&pipe(rest), 0));
            if reference.is_empty() {
                continue;
            }
            if let Some(target) = resolve(&reference) {
                wanted.push(json!({"ref":reference,"to":target,"why":ps::at(&pipe(rest), 1)}));
            }
        }
        if truthy(&part["of"])
            && let Some(target) = resolve(&s(&part["of"]))
        {
            wanted.push(json!({"ref":s(&part["of"]),"to":target,"why":"of"}));
        }
        for want in &wanted {
            let target = field(want, "to")?;
            if target == header || !part_ids.contains_key(target) {
                continue;
            }
            // A split part is authored through any of its pieces (grossman2002integration trial 16).
            let mut pieces = crate::model::compiler::reference_pieces(&entry["ids"], &compiler["models"], &s(&want["ref"]));
            if pieces.is_empty() {
                pieces.push(target.to_string());
            }
            let Some(authored) = pieces.iter().find(|piece| pairs.contains(&(address.clone(), (*piece).clone()))) else {
                bail!("part `{address}` does not author the Link to `{target}` its accepted plan requires");
            };
            requirements.push(json!({"from":address,"to":authored,"ref":want["ref"],"why":want["why"]}));
        }
    }
    // Explanation needs: the exact own Derivation parts that still lack 思路 or 解读. The
    // parts counted are the checked model's own, in build order, never an older count (R301).
    let own_derivations: Vec<String> = model["build"]["order"]
        .as_array()
        .context("incomplete model build order")?
        .iter()
        .filter(|o| o["derivation"] == json!(true) && o["line"] != json!("shared"))
        .map(|o| s(&o["address"]))
        .collect();
    let mut needs = Vec::new();
    let mut thoughts = Vec::new();
    for address in &own_derivations {
        let body = cards.get(address).map(|c| s(&c["body"])).unwrap_or_default();
        if crate::model::text::explained(&body) {
            thoughts.push(address.clone());
            continue;
        }
        let sections = crate::model::text::split_sections(&body);
        let mut missing = Vec::new();
        for name in ["思路", "解读"] {
            if crate::model::text::section_text(&sections, name).is_empty() {
                missing.push(name);
            }
        }
        needs.push(json!({"address":address,"missing":missing}));
    }
    let needs_at: BTreeSet<String> = needs.iter().map(|n| s(&n["address"])).collect();
    let own_at: BTreeSet<String> = own_derivations.iter().cloned().collect();
    let chains: Vec<Value> = build["chains"]
        .as_array()
        .context("incomplete derivation chains")?
        .iter()
        .map(|chain| {
            json!({
                "parts": pipe(&chain["parts"]).into_iter().filter(|p| own_at.contains(&s(p))).cloned().collect::<Vec<_>>(),
                "after": chain["after"],
                "before": chain["before"],
            })
        })
        .filter(|chain| {
            chain["parts"]
                .as_array()
                .is_some_and(|parts| !parts.is_empty() && parts.iter().any(|p| needs_at.contains(&s(p))))
        })
        .collect();
    // Style faults are the whole-book Card cycle's work: retained as a tolerance, not a
    // second quality gate that would hide the model's own build facts.
    let check = crate::check::Check {
        model: header.to_string(),
        skip_shared: true,
        ..Default::default()
    };
    let lines = check.run(zt)?;
    let style: Vec<String> = lines
        .iter()
        .filter(|line| !line.starts_with("faults: "))
        .cloned()
        .collect();
    let split = ps::int(&entry["split"]).unwrap_or(0);
    let steps: serde_json::Map<String, Value> = ["brief", "plan", "apply", "derive", "review", "split", "report", "resume"]
        .iter()
        .filter(|name| entry["steps"][**name].is_object())
        .map(|name| ((*name).to_string(), entry["steps"][*name].clone()))
        .collect();
    let trees: BTreeSet<String> = cards
        .values()
        .map(|card| field(card, "tree").map(str::to_string))
        .collect::<Result<_>>()?;
    let fingerprints: BTreeMap<String, String> = cards
        .iter()
        .map(|(address, card)| (address.clone(), crate::calls::authored_fingerprint(card)))
        .collect();
    let part_order: Vec<Value> = order
        .iter()
        .map(|(address, o)| {
            json!({"address":address,"id":part_ids.get(address),"line":o["line"],"facet":o["facet"],"derivation":o["derivation"]})
        })
        .collect();
    Ok(json!({
        "schema_version": 1,
        "kind": "model_completion",
        "unit": unit,
        "token": token,
        "state": "done",
        "complete": true,
        "reused": false,
        "model": header,
        "level": level,
        "plan_hash": plan_hash,
        "source_review": {"path": review_path.to_string_lossy(), "complete": true, "parts": review["parts"].as_array().map(Vec::len).unwrap_or(0), "outcomes": review["outcomes"], "supplied": supplied},
        "producer": producer,
        "producer_input_hash": field(base, "producer_input_hash")?,
        "plan_parts": plan_parts.len(),
        "parts": part_map,
        "part_order": part_order,
        "head_fingerprint": crate::calls::authored_fingerprint(head_card),
        "start_fingerprint": field(inputs, "fingerprint")?,
        "card_fingerprints": fingerprints,
        "structure": {"lines":structure,"fingerprint":canon::hash_value(&json!(structure))},
        "steps": steps,
        "split": split,
        "explanation": explanation_facts(&level, &own_derivations, &thoughts, needs, chains),
        "notes": entry["notes"],
        "links": {"requirements":requirements},
        "build": {"order":part_order.len(),"tolerated":build["tolerated"],"cited":build["cited"]},
        "style": {"count":style.len(),"lines":style},
        "trees": trees,
        "archive_revision": read.archive_revision,
    }))
}

/// The recorded state of one model's explanation layer (R301, R302). `applicable` separates a
/// concept model, whose layer is inapplicable, from a formal model that still holds unexplained
/// Derivation parts. The counts are the checked model's own parts, not an older report's.
fn explanation_facts(
    level: &str,
    own: &[String],
    thoughts: &[String],
    needs: Vec<Value>,
    chains: Vec<Value>,
) -> Value {
    let applicable = !ps::eq(level, "concept");
    let (status, reason) = if !applicable {
        ("none", "concept")
    } else if own.is_empty() {
        ("none", "no_derivations")
    } else if needs.is_empty() {
        ("none", "explained")
    } else {
        ("required", "unexplained")
    };
    json!({
        "status": status,
        "reason": reason,
        "applicable": applicable,
        "derivations": own.len(),
        "explained": thoughts.len(),
        "needs": needs,
        "chains": chains,
    })
}

/// Repeating a completed Model unit re-reads its Cards and its step state; a changed
/// Card or plan makes the retained completion stale.
fn model_done(p: &Prepared, zt: &mut Zt, unit: &str, token: i64) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let raw: String = tx.query_row(
        "SELECT result_json FROM units WHERE id=?1 AND state='done' AND claim_token=?2",
        params![unit, token],
        |row| row.get(0),
    )?;
    let mut result: Value = serde_json::from_str(&raw)?;
    let rows = tx
        .prepare("SELECT output_key,kind,locator,fingerprint FROM outputs WHERE unit_id=?1 ORDER BY output_key")?
        .query_map([unit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    tx.commit()?;
    drop(conn);
    let inputs = frozen_inputs(p, unit)?;
    if canon::hash_value(&inputs) != field(&result, "unit_input_hash").unwrap_or("") {
        bail!("Model unit input hash mismatch");
    }
    let current = model_evidence(p, zt, unit, token, &inputs)?;
    for key in ["parts", "card_fingerprints", "plan_hash", "steps", "explanation", "structure", "source_review"] {
        if current[key] != result[key] {
            bail!("completed Model unit changed ({key}); keep the checked result and re-run from a new claim");
        }
    }
    for (key, kind, locator, fingerprint) in &rows {
        match kind.as_str() {
            "file" => {
                if result["files"][key]["path"] != *locator || result["files"][key]["sha256"] != *fingerprint {
                    bail!("changed Model evidence registration");
                }
                check_file(&json!({"path":locator,"sha256":fingerprint}), "Model completion evidence")?;
            }
            "part" | "card" => {}
            _ => bail!("unknown Model evidence kind"),
        }
    }
    result["reused"] = json!(true);
    Ok(result)
}

fn model_finish(p: &Prepared, zt: &mut Zt, unit: &str, token: i64, check_only: bool) -> Result<Value> {
    let _guard = unit_guard(p, unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, unit, token)?;
    let inputs = frozen_inputs(p, unit)?;
    if canon::hash_value(&inputs) != claimed.input_hash {
        bail!("Model unit input hash mismatch");
    }
    if done {
        let mut result = model_done(p, zt, unit, token)?;
        if check_only {
            result["check_only"] = json!(true);
            result["applied"] = json!(false);
        }
        return Ok(result);
    }
    let mut result = model_evidence(p, zt, unit, token, &inputs)?;
    result["input_hash"] = json!(claimed.input_hash);
    result["unit_input_hash"] = json!(claimed.input_hash);
    if check_only {
        result["check_only"] = json!(true);
        result["applied"] = json!(false);
        return Ok(result);
    }
    // Newly discovered explanation work becomes its own registered unit; the model's
    // build still completes when explanations remain open (R302 keeps them visible).
    let key = unit
        .strip_prefix("models/")
        .and_then(|v| v.strip_suffix("@1"))
        .with_context(|| format!("not a Model unit: {unit}"))?;
    let explain = if result["explanation"]["status"] == json!("required") {
        let wanted = json!({
            "schema_version":1,
            "model":field(&inputs, "header")?,
            "unit":unit,
            "plan_hash":result["plan_hash"],
            "parts":result["explanation"]["needs"].as_array().unwrap().iter().map(|n| n["address"].clone()).collect::<Vec<_>>(),
            "chains":result["explanation"]["chains"],
        });
        Some((format!("explain/{key}@1"), wanted))
    } else {
        None
    };
    renew(p, unit, token)?;
    let check_record = publish(
        &p.out.join(format!("model-check-{key}.json")),
        &serde_json::to_string_pretty(&json!({
            "schema_version":1,"unit":unit,"token":token,"model":result["model"],"plan_hash":result["plan_hash"],
            "steps":result["steps"],"parts":result["parts"],"build":result["build"],"style":result["style"],
            "explanation":result["explanation"],"links":result["links"],"archive_revision":result["archive_revision"]
        }))?,
    )?;
    result["files"] = json!({"check": check_record.clone()});
    let completion = publish(
        &p.out.join(format!("model-completion-{key}.json")),
        &serde_json::to_string_pretty(&result)?,
    )?;
    result["files"]["completion"] = completion.clone();
    for record in [&check_record, &completion] {
        check_file(record, "Model completion evidence")?;
    }
    let final_view = zt.archive_view()?;
    if final_view["archive_identity"] != p.contract["archive_identity"]
        || final_view["archive_revision"].as_u64() != Some(result["archive_revision"].as_u64().unwrap())
    {
        bail!("read_view_changed: Model completion evidence changed before ledger commit");
    }
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, unit, token)?;
    for (name, record) in [("check", &check_record), ("completion", &completion)] {
        output(&tx, unit, name, "file", field(record, "path")?, field(record, "sha256")?, record)?;
    }
    for (id, address) in result["parts"].as_object().context("missing part map")? {
        let address = address.as_str().context("invalid part address")?;
        let fingerprint = result["card_fingerprints"][address]
            .as_str()
            .with_context(|| format!("missing Card fingerprint for `{address}`"))?;
        output(&tx, unit, &format!("part/{id}"), "part", address, fingerprint, &json!({"id":id,"address":address}))?;
    }
    for (address, fingerprint) in result["card_fingerprints"].as_object().context("missing Card fingerprints")? {
        output(&tx, unit, &format!("card/{address}"), "card", address, fingerprint.as_str().unwrap_or_default(), &json!({"address":address}))?;
    }
    completion_fault(p, "after_outputs")?;
    if let Some((explain_unit, inputs)) = &explain {
        let id = register(&tx, "explain", key, 1, inputs, Some(unit), false)?;
        if id != *explain_unit {
            bail!("the registered explanation unit id changed");
        }
        dependency(&tx, &id, unit, "success")?;
        event(&tx, &id, Some(&claimed.owner), Some(token), "registered", &json!({"producer":unit,"input_hash":canon::hash_value(inputs)}))?;
        result["explain_unit"] = json!(id);
    }
    // The last Model unit to finish closes the Explain layer its completions feed (R301).
    completion_fault(p, "before_commit")?;
    tx.execute("UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",params![unit,canon::canonical_json(&result),canon::utc_now_o()])?;
    event(
        &tx,
        unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"disposition":"verified","plan_hash":result["plan_hash"],"parts":result["parts"].as_object().map(|p| p.len()),"explanation":result["explanation"]["status"],"trees":result["trees"],"completion":completion,"check":check_record}),
    )?;
    close_enumerations(&tx)?;
    completion_fault(p, "after_commit")?;
    tx.commit()?;
    Ok(result)
}

pub fn model_finish_cli(argv: &[String]) -> Result<i32> {
    let parsed = (|| -> Result<(String, String, i64, bool)> {
        let a = Args::parse(argv, &["Out", "Model", "Token"], &["CheckOnly"])?;
        if !a.positional.is_empty() || a.required("Out")?.trim().is_empty() {
            bail!("explicit -Out is required");
        }
        let token = a
            .required("Token")?
            .parse::<i64>()
            .ok()
            .filter(|number| *number > 0)
            .context("-Token must be positive")?;
        Ok((a.str("Out"), a.required("Model")?, token, a.flag("CheckOnly")))
    })();
    let (out, model, token, check_only) = match parsed {
        Ok(value) => value,
        Err(error) => {
            eprintln!("zt-digest model-finish: {error}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let unit = model_unit_for_header(&conn, &model)?.map(|(unit, _)| unit);
    drop(conn);
    let unit = unit.with_context(|| format!("no registered Model unit names `{model}`"))?;
    let result = model_finish(&p, &mut zt, &unit, token, check_only)?;
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}

/// Every `X#A` reference the accepted plan makes: a part of another model, or a shared
/// part taken in from it. The producer of `A` must be registered before dependent fill.
fn plan_cross_refs(plan: &Value) -> Result<Vec<String>> {
    let mut refs = BTreeSet::new();
    let Some(parts) = plan["parts"].as_array() else {
        return Ok(Vec::new());
    };
    for part in parts {
        let mut push = |value: &Value| -> Result<()> {
            let text = s(value);
            if let Some((other, name)) = text.split_once('#') {
                if other.trim().is_empty() || name.trim().is_empty() {
                    bail!("incomplete model reference `{text}` in the accepted plan");
                }
                if !other.contains('/') {
                    bail!("reference `{text}` must name a Model Card address before `#`");
                }
                refs.insert(text);
            }
            Ok(())
        };
        push(&part["take"])?;
        push(&part["of"])?;
        for rest in list(&part["rests"]) {
            push(&ps::at(&pipe(rest), 0))?;
        }
    }
    Ok(refs.into_iter().collect())
}

/// Whether `other` is a model this batch planned: it is still in the batch's order and its plan.json is in
/// the batch folder, other than the dependent model itself. The plan file, not a plan hash: the compiler
/// records the hash only at the fill, after `model-deps` has run (grossman2002integration trial 10: every
/// model of the first batch showed an empty plan hash, so `3|a`, `4|a` and `7|a` still waited for `2|a`).
fn planned_in_batch(compiler: &Value, batch: &Path, other: &str, header: &str) -> bool {
    let slug: String = other.chars().map(|c| if matches!(c, '/' | '|' | '\\') { '_' } else { c }).collect();
    !ps::eq(other, header)
        && list(&compiler["order"]).iter().any(|model| ps::eq(&s(model), other))
        && batch.join(slug).join("plan.json").is_file()
}

/// R233/R234: register the explicit producer dependency of every `X#A` reference before
/// any dependent fill/apply. A producer that is not done makes the dependent wait without
/// spending an attempt; a completed producer without the named part is a content fault.
fn model_deps(p: &Prepared, zt: &mut Zt, unit: &str, token: i64) -> Result<Value> {
    let (claimed, _progress, _run, done) = finish_claim(p, unit, token)?;
    if done {
        bail!("Model unit {unit} is already complete; its dependencies need no registration");
    }
    let inputs = frozen_inputs(p, unit)?;
    if canon::hash_value(&inputs) != claimed.input_hash {
        bail!("Model unit input hash mismatch");
    }
    let header = field(&inputs, "header")?.to_string();
    let (dir, plan_hash, plan, compiler) = model_plan_state(p, &header)?;
    if !ps::eq(&s(&plan["model"]), &header) {
        bail!("the plan for `{header}` describes `{}`", s(&plan["model"]));
    }
    // A reference to a model this same batch planned is resolved by the compiler from the batch's own part
    // map as the fill writes it (the architect plans a shared part in the first model and takes it from the
    // next); it waits for no ledger producer. On grossman2002integration trial 9 three of four models of one
    // batch were released to wait for `2|a`, which the same batch was about to build.
    let (in_batch, refs): (Vec<String>, Vec<String>) = plan_cross_refs(&plan)?
        .into_iter()
        .partition(|reference| {
            reference.split_once('#').is_some_and(|(other, _)| {
                dir.parent().is_some_and(|batch| planned_in_batch(&compiler, batch, other, &header))
            })
        });
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let mut wanted = Vec::new();
    let mut resolutions = Vec::new();
    let mut fault: Option<Value> = None;
    for reference in &refs {
        let (other, part) = reference.split_once('#').context("incomplete model reference")?;
        if other == header {
            bail!("`{header}` cannot depend on its own part `{part}`");
        }
        let Some((producer, producer_inputs)) = model_unit_for_header(&conn, other)? else {
            fault = Some(json!({
                "code":"unknown_producer","unit":unit,"input_hash":claimed.input_hash,
                "cause":format!("reference `{reference}` names no registered Model unit for `{other}`"),
                "reference":reference,"model":other,"part":part
            }));
            break;
        };
        wanted.push((reference.clone(), producer, producer_inputs));
    }
    drop(conn);
    if fault.is_none() {
        let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
        for (reference, producer, _) in &wanted {
            let (part, other) = (reference.split_once('#').context("incomplete model reference")?.1, reference.split_once('#').context("incomplete model reference")?.0);
            let state: String = conn.query_row("SELECT state FROM units WHERE id=?1", [producer], |row| row.get(0))?;
            if state != "done" {
                continue;
            }
            let part_row: Option<(String, String)> = conn
                .query_row(
                    "SELECT locator,fingerprint FROM outputs WHERE unit_id=?1 AND output_key=?2",
                    params![producer, format!("part/{part}")],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((address, fingerprint)) = part_row else {
                fault = Some(json!({
                    "code":"named_part_missing","unit":unit,"input_hash":claimed.input_hash,
                    "cause":format!("`{other}` completed without part `{part}`; reference `{reference}` cannot be resolved"),
                    "reference":reference,"model":other,"part":part,"producer":producer
                }));
                break;
            };
            let card = zt.cards(&[address.clone()], "text")?;
            let Some(card) = card.cards.first() else {
                fault = Some(json!({
                    "code":"named_part_changed","unit":unit,"input_hash":claimed.input_hash,
                    "cause":format!("part `{part}` of `{other}` was checked at `{address}`, which is gone"),
                    "reference":reference,"model":other,"part":part,"producer":producer,"address":address
                }));
                break;
            };
            if crate::calls::authored_fingerprint(card) != fingerprint {
                fault = Some(json!({
                    "code":"named_part_changed","unit":unit,"input_hash":claimed.input_hash,
                    "cause":format!("part `{part}` of `{other}` changed after its accepted output"),
                    "reference":reference,"model":other,"part":part,"producer":producer,"address":address
                }));
                break;
            }
            resolutions.push(json!({"ref":reference,"model":other,"part":part,"producer":producer,"address":address,"fingerprint":fingerprint}));
        }
        drop(conn);
    }
    if let Some(fault) = fault {
        // A precise content fault, recorded through the same checked fault path as a
        // compiler failure; the caller does not spend another attempt guessing.
        let recorded = fail(p, unit, token, "content", &fault)?;
        return Ok(json!({"ok":false,"unit":unit,"fault":fault,"recorded":recorded}));
    }
    let waiting: Vec<Value> = wanted
        .iter()
        .filter(|(reference, producer, _)| !resolutions.iter().any(|r| r["ref"] == json!(reference) && r["producer"] == json!(producer)))
        .map(|(reference, producer, _)| json!({"ref":reference,"producer":producer}))
        .collect();
    // The dependency graph must see this unit, so release the claim for cycle detection
    // inside the same transaction; no attempt is spent while a prerequisite is incomplete.
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let claim = claim(&tx, unit, token)?;
    for (_, producer, _) in &wanted {
        let existing: Option<String> = tx
            .query_row(
                "SELECT condition FROM deps WHERE unit_id=?1 AND needs_id=?2",
                params![unit, producer],
                |row| row.get(0),
            )
            .optional()?;
        match existing.as_deref() {
            Some("success") => {}
            Some(other) => bail!("the Model unit's dependency on {producer} is `{other}`, not `success`"),
            None => dependency(&tx, unit, producer, "success")?,
        }
    }
    let lease_until = now_seconds() + lease_seconds();
    tx.execute(
        "UPDATE units SET state='registered',owner=NULL,lease_until=NULL WHERE id=?1",
        params![unit],
    )?;
    detect_cycles(&tx)?;
    let after: Option<(String, Option<String>)> = tx
        .query_row("SELECT state,fault_json FROM units WHERE id=?1", [unit], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()?;
    let (state, fault_json) = after.context("the Model unit disappeared during dependency registration")?;
    if state == "failed" {
        tx.commit()?;
        let fault: Value = fault_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or(Value::Null);
        return Ok(json!({"ok":false,"unit":unit,"cycle":fault["cycle"],"fault":fault}));
    }
    let result = if waiting.is_empty() {
        tx.execute(
            "UPDATE units SET state='claimed',owner=?2,claim_token=?3,lease_until=?4 WHERE id=?1",
            params![unit, claim.owner, token, lease_until],
        )?;
        let progress_raw: String = tx.query_row("SELECT progress_json FROM units WHERE id=?1", [unit], |row| row.get(0))?;
        let mut progress: Value = serde_json::from_str(&progress_raw)?;
        progress["model_plan"] = json!({"plan_hash":plan_hash,"refs":resolutions,"unit":unit});
        tx.execute(
            "UPDATE units SET progress_json=?2 WHERE id=?1",
            params![unit, canon::canonical_json(&progress)],
        )?;
        event(&tx, unit, Some(&claim.owner), Some(token), "model_dependencies", &json!({"resolved":resolutions,"waiting":[],"plan_hash":plan_hash}))?;
        json!({"ok":true,"unit":unit,"waiting":false,"refs":resolutions,"in_batch":in_batch,"plan_hash":plan_hash,"state":"claimed","lease_until":lease_until})
    } else {
        tx.execute(
            "UPDATE units SET state='registered',owner=NULL,lease_until=NULL,retry_after=NULL,attempts=MAX(attempts-1,0) WHERE id=?1",
            params![unit],
        )?;
        event(&tx, unit, Some(&claim.owner), Some(token), "deferred_waiting", &json!({"prerequisites":waiting,"resolved":resolutions,"attempts_returned":1}))?;
        json!({"ok":true,"unit":unit,"waiting":true,"prerequisites":waiting,"refs":resolutions,"in_batch":in_batch,"plan_hash":plan_hash,"state":"registered"})
    };
    tx.commit()?;
    drop(conn);
    // The compiler resolves `X#A` from the same part map the batch uses; a ref that waited
    // is written only once its producer has checked the named part.
    if waiting.is_empty() {
        let mut state = compiler.clone();
        for r in &resolutions {
            let (other, part) = (field(r, "model")?, field(r, "part")?);
            if !state["models"][other].is_object() {
                state["models"][other] = json!({"ids":{}});
            }
            if !state["models"][other]["ids"].is_object() {
                state["models"][other]["ids"] = json!({});
            }
            state["models"][other]["ids"][part] = r["address"].clone();
            state["models"][&header]["refs"][field(r, "ref")?] = r.clone();
        }
        let state_path = dir.parent().context("model folder outside a batch")?.join("state.json");
        canon::save_json(state_path.to_string_lossy().as_ref(), &state)?;
        let key = unit.strip_prefix("models/").and_then(|v| v.strip_suffix("@1")).unwrap_or("model");
        let record = publish(
            &p.out.join(format!("model-refs-{key}.json")),
            &serde_json::to_string_pretty(&json!({"schema_version":1,"unit":unit,"model":header,"plan_hash":plan_hash,"refs":resolutions}))?,
        )?;
        let mut result = result;
        result["refs_file"] = record;
        return Ok(result);
    }
    Ok(result)
}

pub fn model_deps_cli(argv: &[String]) -> Result<i32> {
    let parsed = (|| -> Result<(String, String, i64)> {
        let a = Args::parse(argv, &["Out", "Model", "Token"], &[])?;
        if !a.positional.is_empty() || a.required("Out")?.trim().is_empty() {
            bail!("explicit -Out is required");
        }
        let token = a
            .required("Token")?
            .parse::<i64>()
            .ok()
            .filter(|number| *number > 0)
            .context("-Token must be positive")?;
        Ok((a.str("Out"), a.required("Model")?, token))
    })();
    let (out, model, token) = match parsed {
        Ok(value) => value,
        Err(error) => {
            eprintln!("zt-digest model-deps: {error}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let unit = model_unit_for_header(&conn, &model)?.map(|(unit, _)| unit);
    drop(conn);
    let unit = unit.with_context(|| format!("no registered Model unit names `{model}`"))?;
    let result = model_deps(&p, &mut zt, &unit, token)?;
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}

/// The one Explain unit a Model header owns; a registered obligation is never inferred.
fn explain_unit_for_model(conn: &Connection, header: &str) -> Result<Option<String>> {
    let mut statement = conn.prepare(
        "SELECT id,inputs_json FROM units WHERE layer='explain' ORDER BY id",
    )?;
    let rows = statement
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    let mut found = None;
    for (id, inputs) in rows {
        if serde_json::from_str::<Value>(&inputs)?["model"] == json!(header) {
            if found.is_some() {
                bail!("more than one Explain unit names `{header}`");
            }
            found = Some(id);
        }
    }
    Ok(found)
}

/// The chains of the checked model restricted to its own (not shared) Derivation parts, each
/// with the position line the compiler writes into 思路: `(address, expected line)`.
fn explain_chains(model: &Value) -> Result<Vec<Value>> {
    let own: BTreeSet<String> = model["build"]["order"]
        .as_array()
        .context("incomplete model build order")?
        .iter()
        .filter(|o| o["derivation"] == json!(true) && o["line"] != json!("shared"))
        .map(|o| s(&o["address"]))
        .collect();
    let mut chains = Vec::new();
    for chain in model["build"]["chains"]
        .as_array()
        .context("incomplete derivation chains")?
    {
        let parts: Vec<String> = pipe(&chain["parts"])
            .into_iter()
            .map(s)
            .filter(|address| own.contains(address))
            .collect();
        if parts.is_empty() {
            continue;
        }
        chains.push(json!({"parts":parts,"after":chain["after"],"before":chain["before"]}));
    }
    Ok(chains)
}

/// R301: enumerate the checked model's actual own Derivation parts and chains, verify both
/// sections and the compiler's chain/size constraints, and refuse while any of that work is
/// still unfinished. Nothing here trusts the registered need as the count of the work.
fn explain_evidence(p: &Prepared, zt: &mut Zt, unit: &str, token: i64, inputs: &Value) -> Result<Value> {
    use std::collections::BTreeMap;
    let header = field(inputs, "model")?;
    let model_unit = field(inputs, "unit")?;
    let plan_hash = field(inputs, "plan_hash")?;
    let registered: BTreeSet<String> = inputs["parts"]
        .as_array()
        .context("incomplete Explain unit scope")?
        .iter()
        .map(|part| Ok(field(&json!({"part":part}), "part")?.to_string()))
        .collect::<Result<_>>()?;
    // The producer stays a checked Model completion for this header and plan: a built model
    // is the explanation layer's premise, and its own facts stay separately recorded.
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT state,result_json FROM units WHERE id=?1",
            [model_unit],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (state, raw) = row.with_context(|| format!("Model unit {model_unit} is not registered"))?;
    if state != "done" {
        bail!("Model unit {model_unit} is {state}, not a checked completion");
    }
    let build: Value = serde_json::from_str(&raw.context("missing Model completion evidence")?)?;
    // R413: an adopted Model unit carries a read-only adoption verification, and that is a
    // different artifact from a historic compiler completion. Both record the build and the
    // explanation need; the Explain layer's premise is the checked build, not its origin.
    if !matches!(build["kind"].as_str(), Some("model_completion" | "model_adoption"))
        || build["model"] != json!(header)
        || build["plan_hash"] != json!(plan_hash)
        || build["complete"] != json!(true)
    {
        bail!("the Explain unit's Model completion does not describe `{header}`");
    }
    let explained_at_build: BTreeSet<String> = build["explanation"]["needs"]
        .as_array()
        .context("incomplete Model explanation evidence")?
        .iter()
        .map(|need| s(&need["address"]))
        .collect();
    if explained_at_build != registered {
        bail!("the Explain unit's registered scope is not the Model completion's recorded need");
    }
    let model = zt.call("get_model", json!({"at":header}))?;
    let level = s(&model["level"]);
    let own: Vec<String> = model["build"]["order"]
        .as_array()
        .context("incomplete model build order")?
        .iter()
        .filter(|o| o["derivation"] == json!(true) && o["line"] != json!("shared"))
        .map(|o| s(&o["address"]))
        .collect();
    for address in &registered {
        if !own.contains(address) {
            bail!("stale_content: the checked model no longer holds the registered Derivation part `{address}`");
        }
    }
    let chains = explain_chains(&model)?;
    let read = zt.cards(&own, "text")?;
    let cards: BTreeMap<String, Value> = read
        .cards
        .iter()
        .map(|card| Ok((field(card, "address")?.to_string(), card.clone())))
        .collect::<Result<_>>()?;
    let mut missing = Vec::new();
    let mut parts = Vec::new();
    for address in &own {
        let card = cards
            .get(address)
            .with_context(|| format!("the Derivation part `{address}` does not exist"))?;
        if card["workspace"] != "agent" {
            bail!("Derivation part `{address}` is not in the agent Workspace");
        }
        let body = s(&card["body"]);
        let sections = crate::model::text::split_sections(&body);
        let mut absent = Vec::new();
        for name in ["思路", "解读"] {
            if crate::model::text::section_text(&sections, name).is_empty() {
                absent.push(name);
            }
        }
        if !absent.is_empty() {
            missing.push(format!("{address} ({})", absent.join(", ")));
        }
        parts.push(json!({
            "address": address,
            "facet": card["facet"],
            "title": card["title"],
            "explained": absent.is_empty(),
            "missing": absent,
        }));
    }
    if !missing.is_empty() {
        bail!(
            "explanation_missing: {} of {} Derivation parts of `{header}` still lack 思路 or 解读: {}; run the model compiler's explain step and apply the accepted answers",
            missing.len(),
            own.len(),
            missing.join("; ")
        );
    }
    // The chain position line is the compiler's own text: a stale one would misplace the part
    // in its chain, so it is checked against the current chain and results.
    let mut chain_faults = Vec::new();
    let mut chain_facts = Vec::new();
    for chain in &chains {
        let members: Vec<String> = pipe(&chain["parts"]).into_iter().map(s).collect();
        let tos: Vec<String> = members
            .iter()
            .map(|address| crate::model::text::result_of(&s(&cards[address]["body"])))
            .collect();
        let count = members.len();
        for (index, address) in members.iter().enumerate() {
            let expected = crate::model::text::chain_line(index + 1, count, &tos);
            chain_facts.push(json!({"address":address,"position":index + 1,"chain":count,"line":expected}));
            if expected.is_empty() {
                continue;
            }
            // The first authored line of 思路, position line included: `section_text` drops it.
            let first = crate::model::text::position_of(&s(&cards[address]["body"]));
            if first != expected {
                chain_faults.push(format!(
                    "{address} 思路 starts `{first}`, the chain of {count} parts wants `{expected}`"
                ));
            }
        }
    }
    if !chain_faults.is_empty() {
        bail!("explanation_chain: {}", chain_faults.join("; "));
    }
    // The explanation rules, not the whole Card language: R15 to R17 on these parts are this
    // layer's gate; ordinary language faults stay the whole-book Card cycle's work (R302).
    let check = crate::check::Check {
        model: header.to_string(),
        skip_shared: true,
        require_explain: true,
        ..Default::default()
    };
    let lines = check.run(zt)?;
    let own_at: BTreeSet<String> = own.iter().cloned().collect();
    let explanation: Vec<String> = lines
        .iter()
        .filter(|line| !line.starts_with("faults: "))
        .filter(|line| {
            let mut fields = line.splitn(3, ' ');
            let address = fields.next().unwrap_or_default();
            let rule = fields.next().unwrap_or_default();
            own_at.contains(address) && matches!(rule, "R15" | "R16" | "R17")
        })
        .cloned()
        .collect();
    if !explanation.is_empty() {
        bail!(
            "check_failed: the explanation rules find {} fault(s): {}",
            explanation.len(),
            explanation.join("; ")
        );
    }
    let fingerprints: BTreeMap<String, String> = own
        .iter()
        .map(|address| (address.clone(), crate::calls::authored_fingerprint(&cards[address])))
        .collect();
    let trees: BTreeSet<String> = cards
        .values()
        .map(|card| field(card, "tree").map(str::to_string))
        .collect::<Result<_>>()?;
    let checked: Vec<String> = lines
        .iter()
        .filter(|line| !line.starts_with("faults: "))
        .cloned()
        .collect();
    Ok(json!({
        "schema_version": 1,
        "kind": "explain_completion",
        "unit": unit,
        "token": token,
        "state": "done",
        "complete": true,
        "reused": false,
        "model": header,
        "level": level,
        "plan_hash": plan_hash,
        "producer": model_unit,
        "producer_input_hash": build["unit_input_hash"],
        "build": {"unit":model_unit,"complete":true,"parts":build["build"]["order"],"plan_parts":build["plan_parts"],"plan_hash":plan_hash},
        "explanation": {
            "status": "explained",
            "derivations": own.len(),
            "explained": own.len(),
            "needs": [],
            "parts": parts,
            "chains": chains,
            "positions": chain_facts,
            "check": {"ran":true,"rules":["R15","R16","R17"],"count":checked.len(),"lines":checked},
        },
        "publication_inputs": {"layers":EXPLANATION_CONSUMERS,"unit":unit,"fingerprints":fingerprints},
        "card_fingerprints": fingerprints,
        "trees": trees,
        "archive_revision": read.archive_revision,
    }))
}

/// Repeating a completed Explain unit re-reads the checked model and its Cards; a changed part
/// makes the retained completion stale instead of describing other text.
fn explain_done(p: &Prepared, zt: &mut Zt, unit: &str, token: i64) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let raw: String = tx.query_row(
        "SELECT result_json FROM units WHERE id=?1 AND state='done' AND claim_token=?2",
        params![unit, token],
        |row| row.get(0),
    )?;
    let mut result: Value = serde_json::from_str(&raw)?;
    let rows = tx
        .prepare("SELECT output_key,kind,locator,fingerprint FROM outputs WHERE unit_id=?1 ORDER BY output_key")?
        .query_map([unit], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    tx.commit()?;
    drop(conn);
    let inputs = frozen_inputs(p, unit)?;
    if canon::hash_value(&inputs) != field(&result, "unit_input_hash").unwrap_or("") {
        bail!("Explain unit input hash mismatch");
    }
    let current = explain_evidence(p, zt, unit, token, &inputs)?;
    for key in ["explanation", "card_fingerprints", "plan_hash", "publication_inputs"] {
        if current[key] != result[key] {
            bail!("completed Explain unit changed ({key}); keep the checked result and re-run from a new claim");
        }
    }
    for (key, kind, locator, fingerprint) in &rows {
        match kind.as_str() {
            "file" => {
                if result["files"][key]["path"] != *locator || result["files"][key]["sha256"] != *fingerprint {
                    bail!("changed Explain evidence registration");
                }
                check_file(&json!({"path":locator,"sha256":fingerprint}), "Explain completion evidence")?;
            }
            "card" => {}
            _ => bail!("unknown Explain evidence kind"),
        }
    }
    result["reused"] = json!(true);
    Ok(result)
}

fn explain_finish(p: &Prepared, zt: &mut Zt, unit: &str, token: i64, check_only: bool) -> Result<Value> {
    let _guard = unit_guard(p, unit)?;
    let (claimed, _progress, _run, done) = finish_claim(p, unit, token)?;
    let inputs = frozen_inputs(p, unit)?;
    if canon::hash_value(&inputs) != claimed.input_hash {
        bail!("Explain unit input hash mismatch");
    }
    if done {
        let mut result = explain_done(p, zt, unit, token)?;
        if check_only {
            result["check_only"] = json!(true);
            result["applied"] = json!(false);
        }
        return Ok(result);
    }
    let mut result = explain_evidence(p, zt, unit, token, &inputs)?;
    result["input_hash"] = json!(claimed.input_hash);
    result["unit_input_hash"] = json!(claimed.input_hash);
    if check_only {
        result["check_only"] = json!(true);
        result["applied"] = json!(false);
        return Ok(result);
    }
    let key = unit
        .strip_prefix("explain/")
        .and_then(|value| value.strip_suffix("@1"))
        .with_context(|| format!("not an Explain unit: {unit}"))?;
    renew(p, unit, token)?;
    let check_record = publish(
        &p.out.join(format!("explain-check-{key}.json")),
        &serde_json::to_string_pretty(&json!({
            "schema_version":1,"unit":unit,"token":token,"model":result["model"],"level":result["level"],
            "plan_hash":result["plan_hash"],"explanation":result["explanation"],"archive_revision":result["archive_revision"]
        }))?,
    )?;
    result["files"] = json!({"check": check_record.clone()});
    let completion = publish(
        &p.out.join(format!("explain-completion-{key}.json")),
        &serde_json::to_string_pretty(&result)?,
    )?;
    result["files"]["completion"] = completion.clone();
    for record in [&check_record, &completion] {
        check_file(record, "Explain completion evidence")?;
    }
    let final_view = zt.archive_view()?;
    if final_view["archive_identity"] != p.contract["archive_identity"]
        || final_view["archive_revision"].as_u64() != Some(result["archive_revision"].as_u64().unwrap())
    {
        bail!("read_view_changed: Explain completion evidence changed before ledger commit");
    }
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, unit, token)?;
    for (name, record) in [("check", &check_record), ("completion", &completion)] {
        output(&tx, unit, name, "file", field(record, "path")?, field(record, "sha256")?, record)?;
    }
    for (address, fingerprint) in result["card_fingerprints"]
        .as_object()
        .context("missing explanation fingerprints")?
    {
        output(
            &tx,
            unit,
            &format!("card/{address}"),
            "card",
            address,
            fingerprint.as_str().unwrap_or_default(),
            &json!({"address":address}),
        )?;
    }
    completion_fault(p, "after_outputs")?;
    completion_fault(p, "before_commit")?;
    tx.execute("UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",params![unit,canon::canonical_json(&result),canon::utc_now_o()])?;
    event(
        &tx,
        unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"disposition":"verified","model":result["model"],"plan_hash":result["plan_hash"],"parts":result["explanation"]["derivations"],"chains":result["explanation"]["chains"],"trees":result["trees"],"completion":completion,"check":check_record}),
    )?;
    completion_fault(p, "after_commit")?;
    tx.commit()?;
    Ok(result)
}

pub fn explain_finish_cli(argv: &[String]) -> Result<i32> {
    let parsed = (|| -> Result<(String, String, i64, bool)> {
        let a = Args::parse(argv, &["Out", "Model", "Token"], &["CheckOnly"])?;
        if !a.positional.is_empty() || a.required("Out")?.trim().is_empty() {
            bail!("explicit -Out is required");
        }
        let token = a
            .required("Token")?
            .parse::<i64>()
            .ok()
            .filter(|number| *number > 0)
            .context("-Token must be positive")?;
        Ok((a.str("Out"), a.required("Model")?, token, a.flag("CheckOnly")))
    })();
    let (out, model, token, check_only) = match parsed {
        Ok(value) => value,
        Err(error) => {
            eprintln!("zt-digest explain-finish: {error}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let unit = explain_unit_for_model(&conn, &model)?;
    drop(conn);
    let unit =
        unit.with_context(|| format!("no registered Explain unit names `{model}`"))?;
    let result = explain_finish(&p, &mut zt, &unit, token, check_only)?;
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}

fn terms_view(job: &Value, path: &Path, reused: bool) -> Value {
    json!({"schema_version":1,"unit":job["unit"],"token":job["token"],"input_hash":job["input_hash"],"unit_input_hash":job["unit_input_hash"],"job":path.to_string_lossy(),"answer":job["answer"],"expected_jobs":job["expected_jobs"],"jobs":job["jobs"],"candidate_count":job["inputs"]["candidate_count"],"counts":job["inputs"]["counts"],"no_op":job["no_op"],"reused":reused})
}

fn terms_data_hash(data: &Value) -> String {
    canon::hash_value(
        &json!({"job":data["job_text"],"instructions":data["instructions_text"],"candidate_count":data["candidate_count"],"counts":data["counts"]}),
    )
}

fn read_terms_job(
    p: &Prepared,
    claimed: &Claim,
    progress: &Value,
    run: &Value,
    token: i64,
    data: &Value,
) -> Result<(PathBuf, Value)> {
    if progress["terms_start"]["token"] != token {
        bail!("missing current Terms planning evidence");
    }
    let record = &progress["terms_start"]["job"];
    let path = std::fs::canonicalize(field(record, "path")?)?;
    if !path.starts_with(&p.out) {
        bail!("Terms plan escaped prepared output directory");
    }
    let job = bound_json(record, "Terms plan artifact")?;
    let expected = if data["candidate_count"].as_u64() == Some(0) {
        0
    } else {
        1
    };
    if job["schema_version"] != 1
        || job["unit"] != "terms/book@1"
        || job["token"] != token
        || job["run_id"] != *run
        || job["unit_input_hash"] != claimed.input_hash
        || job["input_hash"] != canon::hash_value(&job["inputs"])
        || job["input_hash"] != progress["terms_start"]["input_hash"]
        || job["inputs"]["plan_data_hash"] != terms_data_hash(data)
        || job["expected_jobs"] != expected
        || job["jobs"]
            .as_array()
            .is_none_or(|jobs| jobs.len() != expected as usize)
    {
        bail!("Terms job binding or candidate enumeration mismatch");
    }
    if job["answer"].as_str()
        != Some(
            p.out
                .join("terms/out/terms.json")
                .to_string_lossy()
                .as_ref(),
        )
    {
        bail!("Terms answer locator mismatch");
    }
    for record in job["artifacts"]
        .as_object()
        .context("missing Terms artifacts")?
        .values()
    {
        check_file(record, "Terms plan artifact")?;
    }
    if expected == 1 && job["jobs"][0]["permissions"]["archive_write"] != false {
        bail!("Terms job cannot have archive write permission");
    }
    Ok((path, job))
}

fn terms_plan(
    p: &Prepared,
    token: i64,
    max_terms: i64,
    max_candidates: i64,
    zt: &mut Zt,
) -> Result<Value> {
    let unit = "terms/book@1";
    finish_claim(p, unit, token)?;
    let _guard = unit_guard(p, unit)?;
    let (claimed, mut progress, run, done) = finish_claim(p, unit, token)?;
    if !progress.is_object() {
        bail!("invalid Terms progress object");
    }
    if claimed.input_hash != canon::hash_value(&json!({"preparation_hash":p.hash})) {
        bail!("Terms unit input hash mismatch");
    }
    let parameters = json!({"max_terms":max_terms,"max_candidates":max_candidates});
    if !progress["terms_config"].is_null() && progress["terms_config"] != parameters {
        bail!("Terms options are frozen for this prepared unit");
    }
    let data = crate::terms::claimed_plan(&p.out.to_string_lossy(), max_terms, max_candidates)?;
    if progress["terms_start"]["token"] == token {
        let (path, job) = read_terms_job(p, &claimed, &progress, &run, token, &data)?;
        if !done {
            renew(p, unit, token)?;
        }
        return Ok(terms_view(&job, &path, true));
    }
    if done {
        bail!("completed Terms unit has no accepted planning evidence");
    }
    renew(p, unit, token)?;
    let work = p.out.join("terms");
    let directory = work.join(format!("claim-{token}"));
    std::fs::create_dir_all(&directory)?;
    std::fs::create_dir_all(work.join("out"))?;
    let directory = std::fs::canonicalize(directory)?;
    if !directory.starts_with(&p.out) {
        bail!("Terms job escaped prepared output directory");
    }
    // Keep the legacy plan locations and freeze the actual files consumed by this claim.
    publish(&work.join("job.md"), field(&data, "job_text")?)?;
    publish(
        &work.join("dsh-terms-instructions.zh.md"),
        field(&data, "instructions_text")?,
    )?;
    let inputs = json!({"schema_version":1,"prompt_contract":1,"run_id":run,"unit":unit,"token":token,"preparation_hash":p.hash,"unit_input_hash":claimed.input_hash,"parameters":parameters,"plan_data_hash":terms_data_hash(&data),"candidate_count":data["candidate_count"],"counts":data["counts"]});
    let input_hash = canon::hash_value(&inputs);
    let expected = if data["candidate_count"].as_u64() == Some(0) {
        0
    } else {
        1
    };
    let answer_path = work.join("out/terms.json");
    let envelope = json!({"schema_version":1,"unit":unit,"token":token,"input_hash":input_hash,"disposition":if expected==0{"no_terms"}else{"terms"},"terms":[]});
    let text = format!(
        "{}\n\n## 当前任务绑定\n\n只写 JSON，不写入归档。结果文件：`{}`。保留以下绑定字段，并填写 terms。最多选择 {max_terms} 个术语。每行包含 term、avoid 字符串数组、非空 meaning 和 section。一个术语只有一个定义。不得用空对象或空 terms 代替缺失的作业。\n\n```json\n{}\n```",
        field(&data, "job_text")?,
        answer_path.display(),
        serde_json::to_string_pretty(&envelope)?
    );
    let prompt = publish(&directory.join("job.md"), &text)?;
    let instructions = publish(
        &directory.join("instructions.md"),
        field(&data, "instructions_text")?,
    )?;
    let jobs = if expected == 0 {
        json!([])
    } else {
        json!([{"id":"terms","files":[instructions["path"],prompt["path"]],"prompt":"按术语任务生成保留绑定字段的 JSON 文件。","output":answer_path.to_string_lossy(),"permissions":{"archive_read":true,"archive_write":false}}])
    };
    let job = json!({"schema_version":1,"run_id":run,"unit":unit,"token":token,"unit_input_hash":claimed.input_hash,"input_hash":input_hash,"inputs":inputs,"expected_jobs":expected,"jobs":jobs,"no_op":if expected==0{json!({"reason":"no_terms_candidates","enumeration_hash":terms_data_hash(&data),"candidate_count":0,"counts":data["counts"]})}else{Value::Null},"answer":answer_path.to_string_lossy(),"artifacts":{"prompt":prompt,"instructions":instructions}});
    let path = directory.join("job.json");
    let record = publish(&path, &serde_json::to_string_pretty(&job)?)?;
    progress["terms_config"] = parameters;
    progress["terms_start"] = json!({"token":token,"input_hash":input_hash,"job":record});
    let after = zt.archive_view()?;
    if after["archive_identity"] != p.contract["archive_identity"] {
        bail!("archive identity changed during Terms planning");
    }
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, unit, token)?;
    let lease_until = held_lease(&tx, unit)?;
    tx.execute(
        "UPDATE units SET progress_json=?2,lease_until=?3 WHERE id=?1",
        params![unit, canon::canonical_json(&progress), lease_until],
    )?;
    event(
        &tx,
        unit,
        Some(&claimed.owner),
        Some(token),
        "terms_planned",
        &json!({"job":record,"input_hash":input_hash,"candidate_count":data["candidate_count"],"expected_jobs":expected}),
    )?;
    tx.commit()?;
    Ok(terms_view(&job, &path, false))
}

fn terms_done(p: &Prepared, zt: &mut Zt, token: i64) -> Result<Value> {
    let unit = "terms/book@1";
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let raw: String = tx.query_row(
        "SELECT result_json FROM units WHERE id=?1 AND state='done' AND claim_token=?2",
        params![unit, token],
        |r| r.get(0),
    )?;
    let mut result: Value = serde_json::from_str(&raw)?;
    let rows=tx.prepare("SELECT output_key,kind,locator,fingerprint FROM outputs WHERE unit_id=?1 ORDER BY output_key")?.query_map([unit],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    tx.commit()?;
    drop(conn);
    let expected = terms_output_keys(&result)?;
    for record in result["evidence"]
        .as_object()
        .unwrap()
        .values()
        .filter(|r| !r.is_null())
    {
        check_file(record, "Terms completion evidence")?;
    }
    if rows
        .iter()
        .map(|(key, _, _, _)| key.clone())
        .collect::<std::collections::BTreeSet<_>>()
        != expected
    {
        bail!("incomplete Terms completion output registration");
    }
    for (key, kind, locator, fingerprint) in &rows {
        match kind.as_str() {
            "file" => {
                if key != "completion"
                    && (result["evidence"][key]["path"] != *locator
                        || result["evidence"][key]["sha256"] != *fingerprint)
                {
                    bail!("changed Terms evidence registration");
                }
                check_file(
                    &json!({"path":locator,"sha256":fingerprint}),
                    "Terms completion evidence",
                )?;
            }
            "card" => {
                let card = zt.cards(&[locator.clone()], "text")?;
                let book = field(&p.contract, "citation_key")?;
                if card.cards[0]["workspace"] != "agent"
                    || card.cards[0]["kind"] != "regular"
                    || card.cards[0]["type"] != "idea"
                    || card.cards[0]["tree"] != book
                    || card.cards[0]["edge"] != "side"
                    || card.cards[0]["parent_address"] != format!("{book}/1")
                    || crate::calls::authored_fingerprint(&card.cards[0]) != *fingerprint
                    || result["card_fingerprint"] != *fingerprint
                {
                    bail!("completed Terms Card changed");
                }
            }
            "archive_receipt" => {}
            _ => bail!("unknown Terms output evidence kind"),
        }
    }
    let accepted = bound_json(&result["evidence"]["accepted"], "Terms completion evidence")?;
    if canon::hash_value(&accepted) != result["plan_hash"] {
        bail!("completed Terms plan hash changed");
    }
    let receipt_addresses = verify_archive_receipts(zt, &accepted, &result["receipt_keys"])?;
    if let Some(card) = result["card"].as_str() {
        if receipt_addresses != std::collections::BTreeSet::from([card.to_string()]) {
            bail!("completed Terms Card differs from archive receipts");
        }
    }
    result["reused"] = json!(true);
    Ok(result)
}

fn terms_apply(
    p: &Prepared,
    zt: &mut Zt,
    token: i64,
    max_terms: i64,
    max_candidates: i64,
) -> Result<Value> {
    let unit = "terms/book@1";
    finish_claim(p, unit, token)?;
    let _guard = unit_guard(p, unit)?;
    let (claimed, progress, run, done) = finish_claim(p, unit, token)?;
    if claimed.input_hash != canon::hash_value(&json!({"preparation_hash":p.hash})) {
        bail!("Terms unit input hash mismatch");
    }
    if progress["terms_config"] != json!({"max_terms":max_terms,"max_candidates":max_candidates}) {
        bail!("Terms options differ from the frozen planning evidence");
    }
    let data = crate::terms::claimed_plan(&p.out.to_string_lossy(), max_terms, max_candidates)?;
    let (_, job) = read_terms_job(p, &claimed, &progress, &run, token, &data)?;
    if done {
        return terms_done(p, zt, token);
    }
    let work = p.out.join("terms");
    let answer_path = work.join("out/terms.json");
    let no_terms = job["expected_jobs"] == 0;
    let (answer, answer_record) = if no_terms {
        if answer_path.exists() {
            bail!("unexpected Terms answer for a zero-candidate plan");
        }
        (json!({"terms":[]}), Value::Null)
    } else {
        if !answer_path.is_file() {
            bail!("missing Terms answer for the expected job");
        }
        if std::fs::metadata(&answer_path)?.len() > 16 * 1024 * 1024 {
            bail!("Terms answer exceeds 16 MiB encoded input");
        }
        let (answer, record) = json_artifact(&answer_path)?;
        if !Path::new(field(&record, "path")?).starts_with(&p.out) {
            bail!("Terms answer escaped the prepared output directory");
        }
        if answer["schema_version"] != 1
            || answer["unit"] != unit
            || answer["token"] != token
            || answer["input_hash"] != job["input_hash"]
        {
            bail!("Terms answer binding mismatch");
        }
        if answer
            .as_object()
            .context("Terms answer must be an object")?
            .keys()
            .any(|k| {
                ![
                    "schema_version",
                    "unit",
                    "token",
                    "input_hash",
                    "disposition",
                    "terms",
                ]
                .contains(&k.as_str())
            })
        {
            bail!("unknown Terms answer field");
        }
        if answer["disposition"] != "terms" {
            bail!("enumerated Terms candidates require a nonempty terms answer");
        }
        if answer["terms"].as_array().is_none_or(Vec::is_empty) {
            bail!("enumerated Terms candidates require nonempty terms");
        }
        (answer, record)
    };
    let manifest = util::read_json(p.out.join("manifest.json"))?;
    let key = field(&p.contract, "citation_key")?;
    let rendered = crate::terms::claimed_table(
        &answer,
        max_terms,
        manifest["book_title"].as_str().unwrap_or_default(),
        key,
    )?;
    let prefix = format!(
        "{}/terms-book/1/apply",
        run.as_str().context("missing run id")?
    );
    let path = work.join("accepted-ledger.json");
    let calls = if path.exists() {
        let saved = canon::read_json(&path.to_string_lossy())?;
        saved["calls"]
            .as_array()
            .context("missing accepted Terms calls")?
            .clone()
    } else if no_terms {
        Vec::new()
    } else {
        let call = crate::terms::term_call(
            zt,
            key,
            field(&rendered, "title")?,
            field(&rendered, "body")?,
            true,
        )?;
        if call["tool"] == "edit_card" {
            bail!(
                "legacy_adoption_required: an existing unreceipted Terms Card cannot be overwritten"
            );
        }
        vec![call]
    };
    if (no_terms && !calls.is_empty()) || (!no_terms && calls.len() != 1) {
        bail!("incomplete accepted Terms operation list");
    }
    let expected_calls = if no_terms {
        Vec::new()
    } else {
        vec![
            json!({"tool":"create_successor","arguments":{"at":format!("{key}/1"),"kind":"side","title":rendered["title"],"body":rendered["body"]}}),
        ]
    };
    if calls != expected_calls {
        bail!("accepted Terms calls differ from the required table and Side-successor placement");
    }
    let plan = json!({"schema_version":1,"unit":unit,"unit_input_hash":claimed.input_hash,"plan_revision":1,"operation_prefix":prefix,"input":rendered,"terms":answer["terms"],"disposition":if no_terms{"no_terms"}else{"terms"},"calls":calls});
    if path.exists() && canon::read_json(&path.to_string_lossy())? != plan {
        bail!("accepted_plan_changed: Terms answer differs from accepted input");
    }
    let check = if no_terms {
        let mut file = tempfile::NamedTempFile::new()?;
        serde_json::to_writer(
            &mut file,
            &json!([{"address":"terms.md","title":rendered["title"],"body":rendered["body"],"category":"terms"}]),
        )?;
        std::io::Write::flush(&mut file)?;
        let check = crate::check::Check {
            json: file.path().to_string_lossy().into_owned(),
            ..Default::default()
        };
        checked_lines(zt_check::check::run_lines(&check.args(), None)?, true)?
    } else {
        let preview = crate::calls::preview_identified(zt, &calls, &prefix, None)?;
        group_quality(
            preview["cards"].as_array().context("missing Terms draft")?,
            p,
        )?
    };
    if check["count"] != 0 {
        bail!("Terms table did not pass its checker: {}", check["faults"]);
    }
    if !answer_record.is_null() {
        check_file(&answer_record, "Terms answer")?;
    }
    renew(p, unit, token)?;
    let accepted = accept_plan(&path, &plan)?;
    let receipts = if no_terms {
        Vec::new()
    } else {
        crate::calls::invoke_identified(zt, &calls, &prefix, None)?
    };
    let card = receipts
        .first()
        .map(|r| r["result"]["address"].clone())
        .unwrap_or(Value::Null);
    let (self_check, card_fingerprint, revision) = if let Some(address) = card.as_str() {
        let read = zt.cards(&[address.to_string()], "text")?;
        let c = &read.cards[0];
        if c["workspace"] != "agent"
            || c["kind"] != "regular"
            || c["type"] != "idea"
            || c["tree"] != key
            || c["edge"] != "side"
            || c["parent_address"] != format!("{key}/1")
            || c["title"] != rendered["title"]
            || c["body"].as_str().unwrap_or_default().replace("\r\n", "\n")
                != field(&rendered, "body")?.replace("\r\n", "\n")
            || crate::calls::authored_fingerprint(c)
                != field(&receipts[0]["result"], "after_fingerprint")?
        {
            bail!("persisted Terms table differs from accepted input");
        }
        let check = crate::check::Check {
            cards: address.to_string(),
            book: key.to_string(),
            ..Default::default()
        };
        (
            checked_lines(check.run(zt)?, false)?,
            json!(crate::calls::authored_fingerprint(c)),
            read.archive_revision,
        )
    } else {
        (
            check,
            Value::Null,
            zt.archive_view()?["archive_revision"]
                .as_u64()
                .context("missing archive revision")?,
        )
    };
    if self_check["count"] != 0 {
        bail!("persisted Terms table did not pass its checker");
    }
    let terms = publish(&p.out.join("terms.md"), field(&rendered, "body")?)?;
    let checked = publish(
        &work.join("self-check.json"),
        &serde_json::to_string_pretty(
            &json!({"schema_version":1,"unit":unit,"token":token,"input_hash":job["input_hash"],"plan_hash":canon::hash_value(&plan),"scope":if no_terms{"term_table_file"}else{"persisted_term_card"},"self_check":self_check}),
        )?,
    )?;
    let evidence = json!({"plan":progress["terms_start"]["job"],"accepted":accepted,"answer":answer_record,"terms":terms,"self_check":checked});
    let receipt_keys = receipts
        .iter()
        .map(|r| r["op_key"].clone())
        .collect::<Vec<_>>();
    let result = json!({"schema_version":1,"kind":"terms_completion","plan_hash":canon::hash_value(&plan),"unit":unit,"token":token,"input_hash":job["input_hash"],"state":"done","complete":true,"reused":false,"disposition":if no_terms{"no_terms"}else{"terms"},"terms":rendered["terms"],"with_avoid":rendered["with_avoid"],"card":card,"card_fingerprint":card_fingerprint,"file":terms["path"],"receipt_keys":receipt_keys,"self_check":self_check,"candidate_count":data["candidate_count"],"no_op":job["no_op"],"evidence":evidence});
    terms_output_keys(&result)?;
    let completion = publish(
        &work.join("completion.json"),
        &serde_json::to_string_pretty(&result)?,
    )?;
    for record in evidence
        .as_object()
        .unwrap()
        .values()
        .filter(|r| !r.is_null())
    {
        check_file(record, "Terms completion evidence")?;
    }
    let after = zt.archive_view()?;
    if after["archive_identity"] != p.contract["archive_identity"]
        || after["archive_revision"].as_u64() != Some(revision)
    {
        bail!("read_view_changed: Terms completion");
    }
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, unit, token)?;
    for (key, record) in evidence
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, r)| !r.is_null())
    {
        output(
            &tx,
            unit,
            key,
            "file",
            field(record, "path")?,
            field(record, "sha256")?,
            record,
        )?;
    }
    output(
        &tx,
        unit,
        "completion",
        "file",
        field(&completion, "path")?,
        field(&completion, "sha256")?,
        &completion,
    )?;
    if let Some(card) = card.as_str() {
        output(
            &tx,
            unit,
            &format!("card/{card}"),
            "card",
            card,
            card_fingerprint.as_str().unwrap(),
            &json!({"type":"idea","purpose":"terms"}),
        )?;
    }
    for receipt in &receipts {
        let key = field(receipt, "op_key")?;
        output(
            &tx,
            unit,
            &format!("receipt/{key}"),
            "archive_receipt",
            key,
            &canon::hash_value(&receipt["result"]),
            &receipt["result"],
        )?;
    }
    completion_fault(p, "after_outputs")?;
    tx.execute("UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",params![unit,canon::canonical_json(&result),canon::utc_now_o()])?;
    event(
        &tx,
        unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"disposition":result["disposition"],"completion":completion,"terms":rendered["terms"],"candidate_count":data["candidate_count"]}),
    )?;
    completion_fault(p, "before_commit")?;
    tx.commit()?;
    completion_fault(p, "after_commit")?;
    Ok(result)
}

pub(crate) fn terms_cli(a: &Args) -> Result<i32> {
    let parsed = (|| -> Result<(String, String, i64)> {
        let out = a.required("Staging")?;
        if out.trim().is_empty() || !a.positional.is_empty() {
            bail!("explicit -Staging is required");
        }
        let step = a.choice("Step", &["plan", "apply"])?;
        let token = a
            .required("Token")?
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .context("-Token must be positive")?;
        if a.has("OperationPrefix") {
            bail!("claimed Terms operations derive their receipt prefix from the ledger");
        }
        if (step == "apply") != a.flag("Apply") {
            bail!("claimed Terms apply requires -Apply; plan does not accept it");
        }
        Ok((out, step, token))
    })();
    let (out, step, token) = match parsed {
        Ok(v) => v,
        Err(e) => {
            eprintln!("zt-digest terms: {e}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    // R328 (§15.2): the claimed Terms plan allocates the one Terms job and its apply writes the
    // table Card into the book's Literature tree. Both stop before their first file or Card when
    // that tree is locked by the owner.
    if let Some(refusal) = crate::lockcheck::preflight(&mut zt, "terms", &step, &field(&p.contract, "citation_key")?)? {
        return crate::lockcheck::stop("terms", &refusal);
    }
    let (_, progress, _, _) = finish_claim(&p, "terms/book@1", token)?;
    let config = &progress["terms_config"];
    let max_terms = if a.has("MaxTerms") {
        a.int_or("MaxTerms", 60)?
    } else {
        config["max_terms"].as_i64().unwrap_or(60)
    };
    let max_candidates = if a.has("MaxCandidates") {
        a.int_or("MaxCandidates", 160)?
    } else {
        config["max_candidates"].as_i64().unwrap_or(160)
    };
    if max_terms < 1 || max_candidates < 1 {
        bail!("claimed Terms require positive MaxTerms and MaxCandidates");
    }
    let result = if step == "plan" {
        terms_plan(&p, token, max_terms, max_candidates, &mut zt)?
    } else {
        terms_apply(&p, &mut zt, token, max_terms, max_candidates)?
    };
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}

fn rerun_list(out: &Path) -> Result<Value> {
    let view = status(out, "", None)?;
    let units = view["units"].as_array().unwrap();
    let unfinished = units
        .iter()
        .filter(|u| u["state"] != "done" && !is_superseded(u))
        .filter_map(|u| u["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let roots = units
        .iter()
        .filter(|u| !is_superseded(u))
        .filter(|u| {
            matches!(
                u["state"].as_str(),
                Some("failed" | "blocked" | "retry" | "claimed")
            )
        })
        .filter_map(|u| u["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let closure = |downstream: bool| {
        let mut seen = roots.clone();
        loop {
            let before = seen.len();
            for dep in view["dependencies"].as_array().unwrap() {
                let (from, to) = if downstream {
                    ("needs", "unit")
                } else {
                    ("unit", "needs")
                };
                let from = dep[from].as_str().unwrap();
                let to = dep[to].as_str().unwrap();
                if seen.contains(from) && unfinished.contains(to) {
                    seen.insert(to);
                }
            }
            if before == seen.len() {
                break;
            }
        }
        seen.into_iter().collect::<Vec<_>>()
    };
    Ok(
        json!({"schema_version":1,"run_id":view["run_id"],"complete":view["complete"],"unresolved":view["unresolved"],"prerequisites":closure(false),"downstream":closure(true),"units":units.iter().filter(|u|u["state"]!="done"&&!is_superseded(u)).collect::<Vec<_>>(),"dependencies":view["dependencies"],"archive_verification":"not_requested"}),
    )
}

// --- Resume: invocation 2 of one prepared run (R274–R285) ----------------------------------------
//
// A resume never prepares, never generates a file and never accepts a new configuration: it reads
// the frozen identity, the enabled layers, the versions, the effective settings and any existing
// authorization-bound selection from the ledger. It refuses a changed source, group manifest,
// workflow contract or archive, a live owner, and a second resume. One `BEGIN IMMEDIATE`
// transaction opens invocation 2, spends `resume_count`, resets the attempt budget of exactly the
// unresolved units, and returns blocked dependants to dependency evaluation. Done builds are never
// reopened: a completed group or model keeps its result, attempt count and completion time.

/// The live claims of this run. A lease that has not expired still belongs to another invocation,
/// so nothing may be reclaimed or reset under it (R279).
fn live_claims(conn: &Connection, now: i64) -> Result<Vec<Value>> {
    let mut statement = conn.prepare(
        "SELECT id,layer,owner,claim_token,lease_until FROM units WHERE state='claimed' AND coalesce(lease_until,0)>?1 ORDER BY id",
    )?;
    let rows = statement
        .query_map([now], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "layer": r.get::<_, String>(1)?,
                "owner": r.get::<_, Option<String>>(2)?,
                "token": r.get::<_, i64>(3)?,
                "lease_until": r.get::<_, Option<i64>>(4)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// The unresolved prerequisite or downstream closure of one status view: the same selection
/// `rerun-list` reports, so a resume states exactly what the invocation drains (R283).
fn closure_of(view: &Value, downstream: bool) -> Vec<String> {
    let Some(units) = view["units"].as_array() else {
        return Vec::new();
    };
    let unfinished = units
        .iter()
        .filter(|u| u["state"] != json!("done") && !is_superseded(u))
        .filter_map(|u| u["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut seen = units
        .iter()
        .filter(|u| !is_superseded(u))
        .filter(|u| {
            matches!(
                u["state"].as_str(),
                Some("failed" | "blocked" | "retry" | "claimed" | "ready")
            )
        })
        .filter_map(|u| u["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let Some(dependencies) = view["dependencies"].as_array() else {
        return seen.into_iter().map(str::to_string).collect();
    };
    loop {
        let before = seen.len();
        for dependency in dependencies {
            let (from, to) = if downstream {
                ("needs", "unit")
            } else {
                ("unit", "needs")
            };
            let (Some(from), Some(to)) = (dependency[from].as_str(), dependency[to].as_str()) else {
                continue;
            };
            if seen.contains(from) && unfinished.contains(to) {
                seen.insert(to);
            }
        }
        if before == seen.len() {
            break;
        }
    }
    seen.into_iter().map(str::to_string).collect()
}

/// R278–R285: one resume of a prepared run.
fn resume_run(p: &Prepared) -> Result<Value> {
    // R278: the persisted identity, sources, versions, artifacts and effective options were
    // verified by `Prepared::read` before this call; the ledger's own binding is next.
    let before = status(&p.out, "", None)?;
    let spent = meta_i64(&before, "resume_count")?;
    if spent >= 1 {
        bail!(
            "resume_used: this run already spent its one supported resume (resume_count {spent}); the remaining work is a manual worklist, and a later repair or rebuild is a separate preparation"
        );
    }
    let now = now_seconds();
    // R279: no live owner, and no registered external run may still be live, before anything is
    // reclaimed. Both checks read the ledger only.
    {
        let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
        let tx = conn.unchecked_transaction()?;
        verify_binding(&tx, p)?;
        let active = live_claims(&tx, now)?;
        if !active.is_empty() {
            let names = active
                .iter()
                .filter_map(|unit| {
                    Some(format!(
                        "{} ({}, lease until {})",
                        unit["id"].as_str()?,
                        unit["owner"].as_str().unwrap_or("no owner"),
                        unit["lease_until"].as_i64().unwrap_or(0)
                    ))
                })
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "run_active: another invocation still owns {names}; finish, fail or reclaim those claims before a resume"
            );
        }
        tx.commit()?;
    }
    let unresolved = before["unresolved"].as_array().cloned().unwrap_or_default();
    if unresolved.is_empty() {
        // Nothing is executable or failed: a resume would only spend the one-rerun budget on no
        // work, so it reports the retained coverage and changes nothing (R284).
        return Ok(json!({
            "schema_version":1,
            "status":"nothing_to_resume",
            "resumed":false,
            "run_id":before["run_id"],
            "invocation":before["invocation"],
            "resume_count":before["resume_count"],
            "citation_key":before["citation_key"],
            "variant":before["variant"],
            "archive_identity":before["archive_identity"],
            "settings":before["settings"],
            "reclaimed":[],
            "reset":[],
            "reevaluated":[],
            "retained":{"units":[]},
            "unresolved":[],
            "coverage_faults":before["coverage_faults"],
            "closure":{"prerequisites":[],"downstream":[]},
            "ready":[],
            "manual":[],
            "complete":before["complete"]
        }));
    }
    // R279/R182: a lost owner is reclaimed only through the existing guarded path, so a late writer
    // is fenced by a newer token before the resume counts the unit as unresolved work again.
    let reclaimed = reclaim(p, None, 64, false)?;
    let blocked = reclaimed["blocked"].as_array().cloned().unwrap_or_default();
    if !blocked.is_empty() {
        let names = blocked
            .iter()
            .filter_map(|unit| {
                Some(format!(
                    "{} ({})",
                    unit["unit"].as_str()?,
                    unit["jobs"]
                        .as_array()?
                        .iter()
                        .filter_map(|job| job.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })
            .collect::<Vec<_>>()
            .join("; ");
        bail!(
            "run_active: {names} still own running external runs; request cancellation and observe each terminal state before a resume"
        );
    }
    let reclaimed_units = reclaimed["reclaimed"].as_array().cloned().unwrap_or_default();
    let reclaimed_ids = reclaimed_units
        .iter()
        .filter_map(|unit| unit["unit"].as_str().map(str::to_string))
        .collect::<std::collections::BTreeSet<_>>();
    // R281/R285: invocation 2 in one transaction. Every reset is recorded with the prior attempts,
    // so the append-only history keeps what the previous invocation spent.
    let mut reset = Vec::new();
    let mut reevaluated = Vec::new();
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let invocation = meta(&tx, "invocation")?.as_i64().unwrap_or(1) + 1;
    let resume_count = meta(&tx, "resume_count")?.as_i64().unwrap_or(0) + 1;
    tx.execute(
        "UPDATE meta SET value=?2 WHERE key='invocation'",
        params!["invocation", canon::canonical_json(&json!(invocation))],
    )?;
    tx.execute(
        "UPDATE meta SET value=?2 WHERE key='resume_count'",
        params!["resume_count", canon::canonical_json(&json!(resume_count))],
    )?;
    let rows = {
        let mut statement = tx.prepare(
            "SELECT id,layer,state,attempts FROM units WHERE state!='done' ORDER BY id",
        )?;
        statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let superseded = before["units"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|unit| is_superseded(unit))
        .filter_map(|unit| unit["id"].as_str().map(str::to_string))
        .collect::<std::collections::BTreeSet<_>>();
    for (unit, layer, state, attempts) in rows {
        if superseded.contains(&unit) {
            // A superseded revision is terminal history, never outstanding work.
            continue;
        }
        if reclaimed_ids.contains(&unit) {
            tx.execute("UPDATE units SET attempts=0 WHERE id=?1", [&unit])?;
            event(
                &tx,
                &unit,
                None,
                None,
                "attempt_reset",
                &json!({"prior_state":"claimed","prior_state_recorded":"ready","prior_attempts":attempts,"invocation":invocation,"reason":"reclaimed"}),
            )?;
            reset.push(json!({"unit":unit,"layer":layer,"prior_state":"claimed","prior_attempts":attempts}));
            continue;
        }
        match state.as_str() {
            "failed" | "retry" => {
                // R281: only the selected unresolved units get their invocation budget back.
                tx.execute(
                    "UPDATE units SET state='retry',attempts=0,owner=NULL,lease_until=NULL,retry_after=NULL WHERE id=?1",
                    [&unit],
                )?;
                event(
                    &tx,
                    &unit,
                    None,
                    None,
                    "attempt_reset",
                    &json!({"prior_state":state,"prior_attempts":attempts,"invocation":invocation}),
                )?;
                reset.push(json!({"unit":unit,"layer":layer,"prior_state":state,"prior_attempts":attempts}));
            }
            "blocked" => {
                // R281/AC3: a blocked dependant returns to dependency evaluation, never directly to
                // ready. `reconcile` below decides its readiness again from the current producers.
                tx.execute(
                    "UPDATE units SET state='registered',fault_class=NULL,fault_json=NULL WHERE id=?1",
                    [&unit],
                )?;
                event(
                    &tx,
                    &unit,
                    None,
                    None,
                    "dependency_reevaluated",
                    &json!({"prior_state":state,"invocation":invocation}),
                )?;
                reevaluated.push(json!({"unit":unit,"layer":layer,"prior_state":state}));
            }
            // registered/ready are already eligible; a claimed unit was reclaimed above.
            _ => {}
        }
    }
    reconcile(&tx, now)?;
    // The run-level fact belongs to no single unit, so it carries no unit id (the column is nullable
    // for exactly this record, as the enumeration seals already are).
    tx.execute(
        "INSERT INTO events(unit_id,invocation,utc,event,facts_json) VALUES(NULL,?1,?2,?3,?4)",
        params![
            invocation,
            canon::utc_now_o(),
            "resumed",
            canon::canonical_json(&json!({
                "invocation":invocation,
                "resume_count":resume_count,
                "reclaimed":reclaimed_ids.iter().cloned().collect::<Vec<_>>(),
                "reset":reset.iter().filter_map(|unit| unit["unit"].clone().as_str().map(str::to_string)).collect::<Vec<_>>(),
                "reevaluated":reevaluated.iter().filter_map(|unit| unit["unit"].clone().as_str().map(str::to_string)).collect::<Vec<_>>(),
                "unresolved_before":unresolved,
            }))
        ],
    )?;
    tx.commit()?;
    drop(conn);
    let after = status(&p.out, "", None)?;
    let retained = after["units"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|unit| unit["state"] == json!("done") && !is_superseded(unit))
        .filter_map(|unit| unit["id"].as_str().map(str::to_string))
        .collect::<Vec<_>>();
    let mut ready = std::collections::BTreeMap::<String, i64>::new();
    for unit in after["units"].as_array().into_iter().flatten() {
        if unit["state"] == json!("ready") {
            *ready
                .entry(unit["layer"].as_str().unwrap_or_default().to_string())
                .or_default() += 1;
        }
    }
    Ok(json!({
        "schema_version":1,
        "status":"resumed",
        "resumed":true,
        "run_id":after["run_id"],
        "invocation":after["invocation"],
        "resume_count":after["resume_count"],
        "citation_key":after["citation_key"],
        "variant":after["variant"],
        "archive_identity":after["archive_identity"],
        "settings":after["settings"],
        "reclaimed":reclaimed_units,
        "reset":reset,
        "reevaluated":reevaluated,
        "retained":{"units":retained},
        "unresolved":after["unresolved"],
        "closure":{"prerequisites":closure_of(&after,false),"downstream":closure_of(&after,true)},
        "ready":ready.into_iter().map(|(layer,count)| json!({"layer":layer,"count":count})).collect::<Vec<_>>(),
        "manual":[],
        "complete":false
    }))
}

/// One integer field of a status view, which carries JSON numbers where the ledger stores text.
fn meta_i64(view: &Value, key: &str) -> Result<i64> {
    view[key]
        .as_i64()
        .with_context(|| format!("incomplete run state: {key}"))
}

// --- Owned external resources and their cleanup units (R231, R322–R330) ---------------------------
//
// Every created external conversation belongs to the work that created it. The manager records the
// provider's returned ids immediately (`ledger resource -Register`), which creates one cleanup unit
// per (owner unit, attempt token). That unit waits on a `settled` dependency of its owner, so a fix
// round keeps the conversation alive and a halted path still owes the cleanup. `-Observe` records the
// provider's own facts (terminal state, retained evidence, deletion of one exact id, workspace
// listing), and `cleanup-finish` verifies those facts, removes only registered scratch paths inside
// the staging root, and completes the unit with `content_complete` reported apart from
// `cleanup_complete`.

/// The durable evidence path of one prepared run: retained answers, transcripts and usage sit here,
/// outside every scratch subtree that cleanup may remove (R325, R329).
const CLEANUP_EVIDENCE: &str = "cleanup/evidence";

/// The evidence folder as a canonical path inside the prepared output directory.
fn cleanup_evidence(p: &Prepared) -> Result<PathBuf> {
    let path = p.out.join(CLEANUP_EVIDENCE);
    std::fs::create_dir_all(&path)?;
    let path = std::fs::canonicalize(&path)?;
    if !path.starts_with(&p.out) {
        bail!("cleanup evidence path escaped the prepared output directory");
    }
    Ok(path)
}

/// A unit identifier in a form a logical key, a label and a folder name can all carry.
fn unit_slug(unit: &str) -> String {
    unit.chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' })
        .collect()
}

/// The external label of one owned resource: the run id, the unit id and the attempt token (R322).
/// The manager passes exactly this label to the provider, so a lost creation response is recovered
/// only by matching it inside this run's own workspace.
fn resource_label(run: &str, unit: &str, token: i64) -> String {
    format!("zet-{run}-{}-t{token}", unit_slug(unit))
}

fn cleanup_key(unit: &str, token: i64) -> String {
    format!("{}-t{token}", unit_slug(unit))
}

fn cleanup_unit_id(unit: &str, token: i64) -> String {
    format!("cleanup/{}@1", cleanup_key(unit, token))
}

/// The recorded resources of one cleanup unit, or of every cleanup unit of the run.
fn recorded_resources(conn: &Connection, only: Option<&str>) -> Result<Vec<Value>> {
    let mut statement = conn.prepare(
        "SELECT id,progress_json FROM units WHERE layer='cleanup' AND (?1='' OR id=?1) ORDER BY id",
    )?;
    let rows = statement
        .query_map([only.unwrap_or_default()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut resources = Vec::new();
    for (unit, raw) in rows {
        let progress: Value = serde_json::from_str(&raw)?;
        for resource in progress["resources"].as_array().cloned().unwrap_or_default() {
            let mut resource = resource;
            resource["cleanup_unit"] = json!(unit);
            resources.push(resource);
        }
    }
    Ok(resources)
}

fn write_resources(tx: &Transaction, unit: &str, resources: &[Value]) -> Result<()> {
    let progress: String = tx.query_row(
        "SELECT progress_json FROM units WHERE id=?1",
        [unit],
        |r| r.get(0),
    )?;
    let mut progress: Value = serde_json::from_str(&progress)?;
    if !progress.is_object() {
        bail!("invalid unit progress object");
    }
    progress["resources"] = json!(resources);
    tx.execute(
        "UPDATE units SET progress_json=?2 WHERE id=?1",
        params![unit, canon::canonical_json(&progress)],
    )?;
    Ok(())
}

/// A bounded provider identifier: no control characters and no ambiguous emptiness.
fn resource_id(record: &Value, key: &str) -> Result<String> {
    let id = record[key]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("a resource record needs a nonempty {key}"))?;
    if id.chars().count() > 200 || id.chars().any(char::is_control) {
        bail!("resource {key} must be at most 200 readable characters");
    }
    Ok(id.to_string())
}

/// A canonical Windows path carries the extended-length prefix; a lexical path does not. Comparing
/// the two needs one form, and the plain form is what a manager passes on the command line.
fn local_path(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => path.to_path_buf(),
    }
}

/// A scratch target is declared inside the exact staging root, outside every retained subtree and
/// never as a top-level staging file. Registration refuses a target that cannot be resolved that way
/// (R329): the completion later re-checks the same rule with canonical paths.
fn declared_scratch(p: &Prepared, given: &str) -> Result<()> {
    let path = Path::new(given);
    if !path.is_absolute() {
        bail!("scratch target {given} must be an absolute path");
    }
    let root = local_path(&p.out);
    let resolved = local_path(&std::path::absolute(path)?);
    if !resolved.starts_with(&root) || resolved == root {
        bail!(
            "scratch target {} is outside the exact staging root {}",
            resolved.display(),
            root.display()
        );
    }
    if let Some(name) = retained_subtree(p, &resolved) {
        bail!("scratch target {} lies in the retained `{name}` subtree", resolved.display());
    }
    if resolved.parent() == Some(root.as_path()) {
        bail!(
            "scratch target {} is a top-level staging file, not compiler scratch",
            resolved.display()
        );
    }
    Ok(())
}

/// One registered resource, exactly as the manager recorded it.
fn resource_record(
    p: &Prepared,
    input: &Value,
    unit: &str,
    token: i64,
    label: &str,
    cleanup_unit: &str,
) -> Result<Value> {
    let workspace = input["workspace"]
        .as_object()
        .context("a resource record names its provider workspace")?;
    let path = workspace
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("a resource workspace needs its path")?;
    let workspace_id = match workspace.get("id").and_then(Value::as_str) {
        Some(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => path.to_string(),
    };
    if workspace_id.chars().count() > 400 || workspace_id.chars().any(char::is_control) {
        bail!("resource workspace id must be at most 400 readable characters");
    }
    let conversations = input["conversations"]
        .as_array()
        .filter(|list| !list.is_empty())
        .context("a resource record names at least one conversation")?;
    let mut recorded = Vec::new();
    for conversation in conversations {
        if conversation["lost"] == true {
            // The creation response was lost: only the label this run passed can recover it.
            recorded.push(json!({"id":Value::Null,"lost":true,"label":label,"state":Value::Null,"outcome":Value::Null,"delete":Value::Null,"evidence":Value::Null,"usage":Value::Null}));
            continue;
        }
        let id = resource_id(conversation, "id")?;
        let run_id = conversation["run"].as_str().map(str::trim).filter(|v| !v.is_empty());
        // R325: the accepted answer is retained with its hash the moment the id is recorded, so the
        // completion never depends on an agent's later memory of what the run produced.
        let evidence = match conversation["answer"].as_str().filter(|text| !text.trim().is_empty()) {
            Some(text) => evidence_written(p, &id, text)?,
            None => Value::Null,
        };
        recorded.push(json!({
            "id":id,
            "run":run_id,
            "lost":false,
            "label":label,
            "state":Value::Null,
            "outcome":Value::Null,
            "delete":Value::Null,
            "evidence":if evidence.is_null() {Value::Null} else {json!({"answer":evidence})},
            "usage":Value::Null
        }));
    }
    let mut scratch = Vec::new();
    for entry in input["scratch"].as_array().cloned().unwrap_or_default() {
        let path = entry["path"]
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .with_context(|| format!("a scratch entry needs its path ({entry})"))?;
        declared_scratch(p, path)?;
        let mut retained = Vec::new();
        for item in entry["retained"].as_array().cloned().unwrap_or_default() {
            let from = item["from"].as_str().context("a retained item names the file it copies")?;
            let to = item["path"].as_str().context("a retained item names its durable copy")?;
            let sha256 = item["sha256"].as_str().context("a retained item records its hash")?;
            if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                bail!("a retained item hash must be 64 hexadecimal characters");
            }
            retained.push(json!({"from":from,"path":to,"sha256":sha256}));
        }
        scratch.push(json!({"path":path,"retained":retained,"removed":Value::Null}));
    }
    Ok(json!({
        "id":format!("{cleanup_unit}#{workspace_id}"),
        "cleanup_unit":cleanup_unit,
        "owner":unit,
        "token":token,
        "label":label,
        "workspace":{"id":workspace_id,"path":path},
        "conversations":recorded,
        "scratch":scratch,
        "listing":Value::Null,
        "usage":Value::Null,
        "workspace_removed":Value::Null,
        "registered_at":canon::utc_now_o()
    }))
}

/// The durable state of one resource: what is still owed before its cleanup can be called complete.
fn resource_facts(resource: &Value) -> Value {
    let conversations: Vec<Value> = resource["conversations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|conversation| {
            let lost = conversation["lost"] == true || conversation["id"].is_null();
            let terminal = conversation["state"] == json!("terminal");
            let deleted = matches!(
                conversation["delete"].as_str(),
                Some("confirmed") | Some("already_absent")
            );
            json!({
                "id":conversation["id"],
                "label":conversation["label"],
                "lost":lost,
                "terminal":terminal,
                "outcome":conversation["outcome"],
                "evidence":conversation["evidence"],
                "run":conversation["run"],
                "usage":conversation["usage"],
                "deleted":if lost {Value::Null} else {json!(deleted)},
                "delete":conversation["delete"]
            })
        })
        .collect();
    let unknown: Vec<Value> = resource["listing"]["unknown"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    json!({
        "id":resource["id"],
        "cleanup_unit":resource["cleanup_unit"],
        "owner":resource["owner"],
        "label":resource["label"],
        "workspace":resource["workspace"],
        "conversations":conversations,
        "usage":resource["usage"],
        "listing":resource["listing"],
        "unknown":unknown,
        "workspace_removed":resource["workspace_removed"],
        "scratch":resource["scratch"]
    })
}

fn resource_view(resources: &[Value]) -> Vec<Value> {
    resources.iter().map(resource_facts).collect()
}

fn resource_register(p: &Prepared, path: &str) -> Result<Value> {
    let input = canon::read_json(path).context("invalid resource registration")?;
    resource_register_input(p, input)
}

/// `ledger resource -RegisterId`: the manager that ran a DeepSeek job records the returned id with
/// plain arguments, at once (R322). zt-digest builds the record, so no agent writes a JSON file
/// that restates what another agent saw; the answer is added later as a file (`-EvidenceFor`).
fn resource_register_id(p: &Prepared, unit: &str, token: Option<i64>, id: &str, workspace: &str) -> Result<Value> {
    let workspace = workspace.trim();
    if workspace.is_empty() {
        bail!("-RegisterId needs -Workspace, the DeepSeek workspace (cwd) of the run");
    }
    let id = id.trim();
    if placeholder_id(id) {
        bail!("`{id}` is not a conversation id: register only the session_id that deepseek_run returned, after it returned");
    }
    let mut input = json!({
        "schema_version": 1,
        "unit": unit,
        "workspace": {"id": workspace, "path": workspace},
        "conversations": [{"id": id.trim()}],
    });
    if unit != "run" {
        input["token"] = json!(token.context("-RegisterId for a unit needs its -Token")?);
    }
    resource_register_input(p, input)
}

/// A word a manager writes before it has a run (ebrill2001modern trial 1: an explain manager "checked" the
/// command with `-RegisterId PENDING`, and cleanup could neither delete nor evidence that id). A provider
/// id is one token without spaces or angle brackets and is never one of these words.
fn placeholder_id(id: &str) -> bool {
    const WORDS: [&str; 10] = ["pending", "placeholder", "tbd", "todo", "unknown", "none", "null", "undefined", "session_id", "id"];
    id.is_empty()
        || id.chars().any(|c| c.is_whitespace() || "<>{}".contains(c))
        || WORDS.contains(&id.to_ascii_lowercase().as_str())
}

/// `ledger resource -EvidenceFor <id> [-Evidence <file>] [-Usage <json>]`: the facts of one recorded
/// conversation, named by the manager that ran it. The answer is a file inside the prepared output
/// directory (the job's own output, or the answer text the manager saved); zt-digest copies it into
/// the retained evidence with its hash (R325). The usage is the provider's own numbers for the run.
fn resource_evidence_for(p: &Prepared, id: &str, evidence: Option<&str>, usage: Option<&str>, unit: &str, token: Option<i64>) -> Result<Value> {
    let id = id.trim();
    let key = resource_holding(p, id)?;
    let mut result = Value::Null;
    if let Some(file) = evidence {
        let path = std::fs::canonicalize(file.trim()).with_context(|| format!("missing evidence file {file}"))?;
        if !path.starts_with(&p.out) {
            bail!("an evidence file must sit inside the prepared output directory; {file} does not");
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("evidence file {file} is not UTF-8 text"))?;
        if text.trim().is_empty() {
            bail!("evidence file {file} is empty");
        }
        let input = json!({"schema_version":1,"op":"evidence","resource":key,"conversation":id,"text":text,"name":id});
        result = resource_observe_input(p, input, unit, token)?;
    }
    if let Some(usage) = usage {
        let numbers: Value = serde_json::from_str(usage).context("-Usage is a JSON object of numbers")?;
        let input = json!({"schema_version":1,"op":"usage","resource":key,"conversation":id,"usage":numbers});
        result = resource_observe_input(p, input, unit, token)?;
    } else if evidence.is_some() && conversation_usage(p, id)?.is_null() {
        // Evidence without usage: the run's result stated no token usage, so the usage is recorded as
        // unavailable (R345) instead of left absent; the report counts the run as missing.
        let input = json!({"schema_version":1,"op":"usage_unavailable","resource":key,"conversation":id,"reason":"the run's result stated no token usage"});
        result = resource_observe_input(p, input, unit, token)?;
    }
    if result.is_null() {
        bail!("-EvidenceFor needs -Evidence, -Usage, or both");
    }
    Ok(result)
}

/// The one line an agent-facing record command (`-RegisterId`, `-Settle`, `-SettleWorkspace`, `-EvidenceFor`)
/// prints: its top-level facts with `recorded: true`, never the resource views. A `-SettleWorkspace` on the
/// shared hub workspace printed about 260 KB (every resource of the run), and on grossman2002integration
/// trial 10 its cleanup agent could not see that the records had succeeded and reported all six unrecorded.
fn record_brief(result: &Value) -> Value {
    let mut brief = serde_json::Map::new();
    if let Some(fields) = result.as_object() {
        for (key, value) in fields {
            // The prepared run's id is not the recorded id: on grossman2002integration trial 11 a brief manager
            // read `run_id` in the short answer and reported that registration had stored the wrong id.
            if key == "run_id" {
                continue;
            }
            if !value.is_object() && !value.is_array() {
                brief.insert(key.clone(), value.clone());
            }
        }
    }
    brief.insert("recorded".into(), json!(true));
    Value::Object(brief)
}

/// The claim token a record command names with `-Token` (only beside `-Unit`, the cleanup unit it holds).
fn record_token(a: &Args) -> Result<Option<i64>> {
    a.has("Token").then(|| a.str("Token").parse::<i64>()).transpose().context("-Token must be a positive integer")
}

/// The usage record one recorded conversation retains, or null.
fn conversation_usage(p: &Prepared, id: &str) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    for resource in recorded_resources(&conn, None)? {
        for conversation in resource["conversations"].as_array().into_iter().flatten() {
            if conversation["id"] == json!(id) {
                return Ok(conversation["usage"].clone());
            }
        }
    }
    Ok(Value::Null)
}

/// The recorded resource that holds one conversation id.
fn resource_holding(p: &Prepared, id: &str) -> Result<String> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let resource = recorded_resources(&conn, None)?
        .into_iter()
        .find(|resource| {
            resource["conversations"]
                .as_array()
                .is_some_and(|list| list.iter().any(|c| c["id"] == json!(id)))
        })
        .with_context(|| format!("unknown conversation: no recorded resource holds `{id}`"))?;
    Ok(field(&resource, "id")?.to_string())
}

/// Whether a recorded resource retains its run's usage: a resource-wide reading, a per-run reading, or an
/// explicit unavailable record (R345: the run's result stated no token usage, and the report counts it
/// as a missing run). Only a resource with none of these is `cleanup_evidence_missing`. On
/// grossman2002integration trial 10 no manager passed `-Usage`, nothing recorded the usage as unavailable,
/// and cleanup-finish refused every digest cleanup unit although its conversation was deleted with evidence.
fn usage_retained(resource: &Value) -> bool {
    !resource["usage"].is_null()
        || resource["conversations"]
            .as_array()
            .is_some_and(|list| list.iter().any(|conversation| conversation["usage"].is_object()))
}

/// `ledger resource -Settle <id> -State <state> -Deleted <result> [-Message <text>]`: the cleanup agent
/// records, with plain arguments and right after it acted, the provider's terminal state of one
/// recorded conversation and the result of its deletion (R323, R326). A conversation that is already
/// gone (`absent`) is recorded as finished with the outcome `absent`.
fn resource_settle(p: &Prepared, id: &str, state: &str, deleted: &str, message: &str, unit: &str, token: Option<i64>) -> Result<Value> {
    let id = id.trim();
    let key = resource_holding(p, id)?;
    let state = state.trim().to_ascii_lowercase();
    let (terminal, outcome) = match state.as_str() {
        "finished" | "error" | "cancelled" | "failed" => (state.clone(), state.clone()),
        "absent" => ("finished".to_string(), "absent".to_string()),
        _ => bail!("-State is finished, error, cancelled, failed or absent, not `{state}`"),
    };
    resource_observe_input(p, json!({"schema_version":1,"op":"terminal","resource":key,"conversation":id,"state":terminal,"outcome":outcome}), unit, token)?;
    match deleted.trim().to_ascii_lowercase().as_str() {
        result @ ("confirmed" | "already_absent") => resource_observe_input(
            p,
            json!({"schema_version":1,"op":"deleted","resource":key,"conversation":id,"result":result}),
            unit,
            token,
        ),
        "failed" => resource_observe_input(
            p,
            json!({"schema_version":1,"op":"delete_failed","resource":key,"conversation":id,"code":"provider","message":message.chars().take(300).collect::<String>()}),
            unit,
            token,
        ),
        other => bail!("-Deleted is confirmed, already_absent or failed, not `{other}`"),
    }
}

/// `ledger resource -SettleWorkspace <path> -Removed true|false [-Remaining <ids>] [-Unknown <ids>]`: the
/// cleanup agent's own listing of one workspace after its deletions (the recorded ids still present,
/// the ids no resource recorded) and whether it removed the workspace, for every resource there.
fn resource_settle_workspace(p: &Prepared, path: &str, removed: bool, remaining: &str, unknown: &str, unit: &str, token: Option<i64>) -> Result<Value> {
    let path = path.trim();
    let keys: Vec<String> = {
        let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
        recorded_resources(&conn, None)?
            .into_iter()
            .filter(|resource| resource["workspace"]["path"].as_str().is_some_and(|known| known.eq_ignore_ascii_case(path)))
            .filter_map(|resource| resource["id"].as_str().map(str::to_string))
            .collect()
    };
    if keys.is_empty() {
        bail!("unknown workspace: no recorded resource uses `{path}`");
    }
    let ids = |list: &str| -> Vec<String> {
        list.split(',').map(str::trim).filter(|id| !id.is_empty()).map(str::to_string).collect()
    };
    let listed: Vec<Value> = ids(remaining).into_iter().map(|id| json!({"id":id,"state":"unknown"})).collect();
    let strays: Vec<Value> = ids(unknown).into_iter().map(|id| json!({"id":id,"title":""})).collect();
    let mut result = Value::Null;
    for key in keys {
        resource_observe_input(p, json!({"schema_version":1,"op":"listing","resource":key,"conversations":listed,"unknown":strays}), unit, token)?;
        result = resource_observe_input(p, json!({"schema_version":1,"op":"workspace","resource":key,"removed":removed}), unit, token)?;
    }
    Ok(result)
}

fn resource_register_input(p: &Prepared, input: Value) -> Result<Value> {
    if input["schema_version"] != 1 {
        bail!("a resource registration declares schema_version 1");
    }
    // `unit: "run"` marks a conversation that belongs to no single unit (a stage-wide chain): the
    // run-level cleanup owns it, and only that unit settles it (R322).
    let run_owned = input["unit"] == json!("run");
    let unit = if run_owned {
        "run".to_string()
    } else {
        resource_id(&input, "unit")?
    };
    let token = if run_owned {
        1
    } else {
        input["token"]
            .as_i64()
            .filter(|token| *token > 0)
            .context("a resource registration records its positive attempt token")?
    };
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let target = meta(&tx, "target")?;
    if !target["enabled"]
        .as_array()
        .is_some_and(|layers| layers.iter().any(|layer| layer == "cleanup"))
    {
        bail!("this run's target excludes the cleanup layer");
    }
    let run = meta(&tx, "run_id")?
        .as_str()
        .context("the ledger records no run identity")?
        .to_string();
    let (label, cleanup_unit) = if run_owned {
        (
            resource_label(&run, "run", token),
            "cleanup/book@1".to_string(),
        )
    } else {
        (resource_label(&run, &unit, token), cleanup_unit_id(&unit, token))
    };
    if run_owned {
        // The run-level unit is registered by preparation: it never creates a second unit.
        let (state, _): (String, i64) = tx
            .query_row(
                "SELECT state,claim_token FROM units WHERE id=?1",
                [&cleanup_unit],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .with_context(|| format!("unknown unit {cleanup_unit}"))?;
        if matches!(state.as_str(), "done" | "failed" | "blocked") {
            bail!("stale_claim: the run-level cleanup {cleanup_unit} has already settled");
        }
    } else {
        // The ids belong to one attempt of one unit; a superseded token cannot register work (R322).
        let (state, current): (String, i64) = tx
            .query_row(
                "SELECT state,claim_token FROM units WHERE id=?1",
                [&unit],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .with_context(|| format!("unknown unit {unit}"))?;
        if !matches!(state.as_str(), "claimed" | "done") || current != token {
            bail!("stale_claim: {unit} has no current attempt for token {token}");
        }
    }
    let owner = if run_owned { "run".to_string() } else { unit.clone() };
    let record = resource_record(p, &input, &owner, token, &label, &cleanup_unit)?;
    let inputs = json!({"owner":owner,"token":token,"label":label});
    let created: Option<String> = tx
        .query_row("SELECT id FROM units WHERE id=?1", [&cleanup_unit], |r| r.get(0))
        .optional()?;
    let mut resources = if created.is_some() {
        let raw: String = tx.query_row(
            "SELECT progress_json FROM units WHERE id=?1",
            [&cleanup_unit],
            |r| r.get(0),
        )?;
        serde_json::from_str::<Value>(&raw)?["resources"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    } else {
        register(
            &tx,
            "cleanup",
            &cleanup_key(&unit, token),
            1,
            &inputs,
            None,
            false,
        )?;
        if run_owned {
            bail!("the run-level cleanup unit vanished after it was checked");
        }
        dependency(&tx, &cleanup_unit, &unit, "settled")?;
        Vec::new()
    };
    match resources
        .iter()
        .position(|resource| resource["id"] == record["id"])
    {
        Some(index) => {
            // A manager may create a second conversation in the same workspace: merge the new ids
            // and never overwrite a fact another observation already recorded.
            let mut known: BTreeSet<String> = resources[index]["conversations"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|c| c["id"].as_str().map(str::to_string))
                .collect();
            let mut merged = resources[index]["conversations"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for conversation in record["conversations"].as_array().cloned().unwrap_or_default() {
                let fresh = conversation["id"]
                    .as_str()
                    .map(|id| known.insert(id.to_string()))
                    .unwrap_or(false);
                if fresh {
                    merged.push(conversation);
                }
            }
            resources[index]["conversations"] = json!(merged);
            let mut scratch = resources[index]["scratch"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for entry in record["scratch"].as_array().cloned().unwrap_or_default() {
                if !scratch.iter().any(|known| known["path"] == entry["path"]) {
                    scratch.push(entry);
                }
            }
            resources[index]["scratch"] = json!(scratch);
        }
        None => resources.push(record),
    }
    resources.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    write_resources(&tx, &cleanup_unit, &resources)?;
    event(
        &tx,
        &cleanup_unit,
        None,
        None,
        "resource_registered",
        &json!({"owner":unit,"token":token,"label":label,"resources":resources.len()}),
    )?;
    reconcile(&tx, now_seconds())?;
    tx.commit()?;
    Ok(json!({
        "schema_version":1,
        "action":"register",
        "run_id":run,
        "unit":unit,
        "token":token,
        "label":label,
        "cleanup_unit":cleanup_unit,
        "resources":resource_view(&resources),
    }))
}

/// The recorded resources and their durable state; the workflow drives deletion from this list
/// instead of guessing at titles across the user's account (R322).
/// `ledger resource -List -Brief`: the recorded conversations per DeepSeek workspace, ids only: every recorded
/// id, the ones not deleted yet (`open`) and the ones without retained evidence (`bare`). It is what the cleanup
/// stage acts on, small enough to relay (the full listing was about 52 KB for an eight-group paper on
/// grossman2002integration trial 8, came back as prose, and the cleanup settled nothing).
fn resource_brief(view: &[Value]) -> Value {
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::BTreeMap<String, (Value, Vec<Value>, Vec<Value>, Vec<Value>)> = std::collections::BTreeMap::new();
    for resource in view {
        let path = resource["workspace"]["path"].as_str().unwrap_or_default().to_string();
        if !groups.contains_key(&path) {
            order.push(path.clone());
            groups.insert(path.clone(), (resource["workspace"]["id"].clone(), Vec::new(), Vec::new(), Vec::new()));
        }
        let entry = groups.get_mut(&path).expect("the group was just inserted");
        for conversation in resource["conversations"].as_array().into_iter().flatten() {
            let id = &conversation["id"];
            if id.is_null() {
                continue;
            }
            let lost = conversation["lost"] == json!(true);
            entry.1.push(id.clone());
            if !lost && conversation["deleted"] != json!(true) {
                entry.2.push(id.clone());
            }
            if !lost && !conversation["evidence"].is_object() {
                entry.3.push(id.clone());
            }
        }
    }
    let workspaces: Vec<Value> = order
        .iter()
        .map(|path| {
            let (id, recorded, open, bare) = &groups[path];
            json!({"id": id, "path": path, "recorded": recorded, "open": open, "bare": bare})
        })
        .collect();
    json!({"schema_version": 1, "action": "list", "brief": true, "resources": view.len(), "workspaces": workspaces})
}

/// `ledger resource -List -Counts`: the brief listing as numbers only, per workspace (recorded, open, bare). A
/// relay passes it unchanged; on grossman2002integration trial 11 the brief listing's id arrays (about 120 ids) came
/// back abridged ("8 ids (none open)") and without the two model workspaces, so their 83 conversations were never
/// deleted and the run-level cleanup could not settle.
fn resource_counts(brief: &Value) -> Value {
    let workspaces: Vec<Value> = ps::ps_items(&brief["workspaces"])
        .into_iter()
        .map(|w| {
            let n = |key: &str| w[key].as_array().map_or(0, Vec::len);
            json!({"id": w["id"], "path": w["path"], "recorded": n("recorded"), "open": n("open"), "bare": n("bare")})
        })
        .collect();
    json!({"schema_version": 1, "action": "list", "counts": true, "resources": brief["resources"], "workspaces": workspaces})
}

/// `ledger resource -List -Workspace <path>`: the brief entry of one workspace, which the agent that deletes its
/// conversations reads itself (its ids never pass through a relay).
fn resource_brief_of(brief: &Value, path: &str) -> Value {
    let entry = ps::ps_items(&brief["workspaces"])
        .into_iter()
        .find(|w| ps::eq(&ps::s(&w["path"]), path.trim()))
        .unwrap_or_else(|| json!({"path": path.trim(), "recorded": [], "open": [], "bare": []}));
    json!({"schema_version": 1, "action": "list", "workspace": entry})
}

/// `ledger resource -SweepEvidence <answers folder>`: the retained evidence of every recorded conversation that has
/// none yet, from `<folder>\<id>.txt` where the manager wrote it; the ledger reads the ids itself, so no id list
/// passes through a relay. Prints how many it recorded and how many have no answer file (with the first eight).
fn resource_sweep_evidence(p: &Prepared, answers: &str) -> Result<Value> {
    let bare: Vec<String> = {
        let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
        recorded_resources(&conn, None)?
            .iter()
            .flat_map(|resource| resource["conversations"].as_array().cloned().unwrap_or_default())
            .filter(|c| c["lost"] != json!(true) && !c["id"].is_null() && !c["evidence"].is_object())
            .map(|c| ps::s(&c["id"]))
            .collect()
    };
    let mut recorded = 0;
    let mut missing = Vec::new();
    for id in &bare {
        let file = Path::new(answers.trim()).join(format!("{id}.txt"));
        if file.is_file() && resource_evidence_for(p, id, Some(file.to_string_lossy().as_ref()), None, "", None).is_ok() {
            recorded += 1;
        } else {
            missing.push(id.clone());
        }
    }
    Ok(json!({"schema_version": 1, "action": "sweep_evidence", "recorded": recorded, "missing": missing.len(), "first_missing": missing.iter().take(8).collect::<Vec<_>>()}))
}

fn resource_list(p: &Prepared, unit: &str, brief: bool) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let resources = recorded_resources(&tx, (!unit.is_empty()).then_some(unit))?;
    let label = resources
        .first()
        .map(|resource| resource["label"].clone())
        .unwrap_or(Value::Null);
    tx.commit()?;
    if brief {
        return Ok(resource_brief(&resource_view(&resources)));
    }
    Ok(json!({
        "schema_version":1,
        "action":"list",
        "unit":unit,
        "label":label,
        "resources":resource_view(&resources),
    }))
}

/// One durable evidence file inside the run's evidence path (R325). The observation records the
/// path and its hash; the completion re-reads and re-hashes it.
fn evidence_file(p: &Prepared, record: &Value, current: Option<&Value>) -> Result<Value> {
    let given = record
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("an evidence observation names its file")?;
    let path = std::fs::canonicalize(given)
        .with_context(|| format!("missing retained evidence {given}"))?;
    let root = cleanup_evidence(p)?;
    if !path.starts_with(&root) {
        bail!(
            "retained evidence must sit under {}; {given} is outside the durable evidence path",
            root.display()
        );
    }
    let bytes = std::fs::read(&path)?;
    let record = json!({"path":path.to_string_lossy(),"sha256":canon::sha256_hex(&bytes)});
    if let Some(current) = current
        && current["path"] == record["path"]
        && current["sha256"] != record["sha256"]
    {
        bail!("retained evidence changed after it was recorded: {}", path.display());
    }
    Ok(record)
}

/// A readable evidence file name for one conversation.
fn evidence_name(conversation: &str, suffix: &str) -> String {
    let stem: String = conversation
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' })
        .collect();
    format!("{stem}.{suffix}")
}

/// Write one accepted answer (or transcript) as durable evidence and record its hash (R325). The bytes
/// are the caller's own report, so the completion can prove what the conversation produced.
fn evidence_written(p: &Prepared, name: &str, text: &str) -> Result<Value> {
    if text.len() > 1024 * 1024 {
        bail!("retained evidence is at most 1 MiB");
    }
    let root = cleanup_evidence(p)?;
    let path = root.join(evidence_name(name, "answer.txt"));
    util::write_atomic(&path, text)?;
    let path = std::fs::canonicalize(&path)?;
    if !path.starts_with(&root) {
        bail!("retained evidence escaped the durable evidence path");
    }
    let bytes = std::fs::read(&path)?;
    Ok(json!({"path":path.to_string_lossy(),"sha256":canon::sha256_hex(&bytes)}))
}

fn usage_facts(record: &Value) -> Result<Value> {
    let usage = record
        .as_object()
        .context("usage is a JSON object of numeric fields")?;
    if usage.is_empty() {
        bail!("usage needs at least one numeric field");
    }
    let mut facts = serde_json::Map::new();
    for (name, value) in usage {
        let number = value
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0)
            .with_context(|| format!("usage field {name} must be a non-negative number"))?;
        facts.insert(name.clone(), json!(number));
    }
    Ok(Value::Object(facts))
}

/// Record one provider fact about an already registered resource. Only the exact recorded ids are
/// addressed: no observation may invent a conversation or a workspace member (R322, R326). Once the
/// cleanup unit is claimed, only its current token may record another fact (T13).
fn resource_observe(p: &Prepared, path: &str, unit: &str, token: Option<i64>) -> Result<Value> {
    let input = canon::read_json(path).context("invalid resource observation")?;
    resource_observe_input(p, input, unit, token)
}

fn resource_observe_input(p: &Prepared, input: Value, unit: &str, token: Option<i64>) -> Result<Value> {
    if input["schema_version"] != 1 {
        bail!("a resource observation declares schema_version 1");
    }
    let op = input["op"]
        .as_str()
        .context("a resource observation declares its op")?
        .to_ascii_lowercase();
    if ![
        "terminal",
        "evidence",
        "usage",
        "usage_unavailable",
        "deleted",
        "delete_failed",
        "listing",
        "workspace",
        "recovered",
    ]
    .contains(&op.as_str())
    {
        bail!("unknown resource observation op `{op}`");
    }
    let key = input["resource"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("a resource observation names the recorded resource")?;
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let all = recorded_resources(&tx, None)?;
    let target = all
        .iter()
        .find(|resource| resource["id"] == json!(key) || resource["label"] == json!(key))
        .with_context(|| format!("unknown resource: no recorded resource `{key}`"))?
        .clone();
    let cleanup_unit = field(&target, "cleanup_unit")?.to_string();
    // A claim fences later observations: while a cleanup agent holds a unit, only its current token
    // may record another fact (T13). A resource is settled by its own unit or by the run-level one,
    // because the shared workspace is removed there.
    let fence = if unit.is_empty() { cleanup_unit.clone() } else { unit.to_string() };
    if fence != cleanup_unit && fence != "cleanup/book@1" {
        bail!("-Unit must name the resource's own cleanup unit {cleanup_unit} or cleanup/book@1");
    }
    match token {
        Some(token) => {
            claim(&tx, &fence, token)?;
        }
        None => {
            let (state, owner, lease_until): (String, Option<String>, Option<i64>) = tx.query_row(
                "SELECT state,owner,lease_until FROM units WHERE id=?1",
                [&cleanup_unit],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            // Only a live claim fences: a claim whose lease ran out belongs to a run that ended (the
            // ebrill2001modern resume met claim_active on every record of a unit its first run had held).
            if state == "claimed" && lease_until.is_some_and(|until| until > now_seconds()) {
                bail!(
                    "claim_active: {cleanup_unit} is claimed by {}; pass -Unit and -Token",
                    owner.unwrap_or_default()
                );
            }
        }
    }
    let mut resources: Vec<Value> = all
        .into_iter()
        .filter(|resource| resource["cleanup_unit"] == json!(cleanup_unit))
        .collect();
    let index = resources
        .iter()
        .position(|resource| resource["id"] == target["id"])
        .context("recorded resource vanished")?;
    let known_ids: BTreeSet<String> = resources
        .iter()
        .flat_map(|resource| {
            resource["conversations"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|conversation| conversation["id"].as_str().map(str::to_string))
        .collect();
    let resource = &mut resources[index];
    let conversation_at = |resource: &Value, id: &str| -> Result<usize> {
        resource["conversations"]
            .as_array()
            .and_then(|list| list.iter().position(|conversation| conversation["id"] == json!(id)))
            .with_context(|| format!("unknown conversation: {id} is not recorded for this resource"))
    };
    match op.as_str() {
        "terminal" => {
            let id = resource_id(&input, "conversation")?;
            let at = conversation_at(resource, &id)?;
            let state = input["state"]
                .as_str()
                .map(str::to_ascii_lowercase)
                .context("a terminal observation records the provider state")?;
            if !["finished", "error", "cancelled", "failed"].contains(&state.as_str()) {
                bail!("a terminal provider state must be finished, error, cancelled or failed, not `{state}`");
            }
            let outcome = input["outcome"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(&state)
                .to_string();
            if outcome.chars().count() > 200 || outcome.chars().any(char::is_control) {
                bail!("a terminal outcome is at most 200 readable characters");
            }
            if resource["conversations"][at]["state"] == json!("terminal")
                && resource["conversations"][at]["outcome"] != json!(outcome)
            {
                bail!("operation_conflict: this conversation already recorded another terminal outcome");
            }
            resource["conversations"][at]["state"] = json!("terminal");
            resource["conversations"][at]["outcome"] = json!(outcome);
            resource["conversations"][at]["observed_at"] = json!(canon::utc_now_o());
        }
        "evidence" => {
            let id = resource_id(&input, "conversation")?;
            let at = conversation_at(resource, &id)?;
            let mut evidence = resource["conversations"][at]["evidence"].clone();
            if !evidence.is_object() {
                evidence = json!({});
            }
            for name in ["answer", "result"] {
                if input[name].is_null() {
                    continue;
                }
                let record = evidence_file(p, &input[name], evidence.get(name))?;
                evidence[name] = record;
            }
            // A caller-supplied text (the run's own accepted answer) is written here, so the retained
            // evidence is durable even when the caller cannot write a file itself (R325).
            if let Some(text) = input["text"].as_str().filter(|text| !text.trim().is_empty()) {
                let name = input["name"].as_str().unwrap_or(&id);
                let record = evidence_written(p, name, text)?;
                evidence["answer"] = record;
            }
            if !evidence.is_object() || evidence.as_object().is_some_and(|e| e.is_empty()) {
                bail!("an evidence observation names an answer or a result file");
            }
            resource["conversations"][at]["evidence"] = evidence;
        }
        "usage" => {
            let usage = usage_facts(&input["usage"])?;
            if let Some(conversation) = input["conversation"].as_str().map(str::trim).filter(|value| !value.is_empty()) {
                // R343: exact per-run usage, keyed by the provider run id (and, when the provider
                // reports requests, by request id). The same run recorded again replaces its own
                // numbers, so retrieving one result twice or continuing a run adds no double count.
                let at = conversation_at(resource, conversation)?;
                let run = input["run"]
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .or_else(|| resource["conversations"][at]["run"].as_str().map(str::to_string))
                    .unwrap_or_else(|| conversation.to_string());
                if run.chars().count() > 200 || run.chars().any(char::is_control) {
                    bail!("a provider run id is at most 200 readable characters");
                }
                let mut record = match resource["conversations"][at]["usage"].as_object() {
                    Some(existing) if existing.get("run") == Some(&json!(run)) => Value::Object(existing.clone()),
                    _ => json!({"run":run,"recorded_at":canon::utc_now_o()}),
                };
                if let Some(request) = input["request"].as_str().map(str::trim).filter(|value| !value.is_empty()) {
                    if !record["requests"].is_object() {
                        record["requests"] = json!({});
                    }
                    record["requests"][request] = usage;
                } else {
                    record["total"] = usage;
                    record["requests"] = Value::Null;
                }
                if let Some(estimate) = input.get("estimate").filter(|value| !value.is_null()) {
                    record["estimate"] = usage_facts(estimate)?;
                }
                for key in ["phase", "state", "currency"] {
                    if let Some(value) = input[key].as_str().map(str::trim).filter(|value| !value.is_empty()) {
                        if value.chars().count() > 40 || value.chars().any(char::is_control) {
                            bail!("a usage {key} is at most 40 readable characters");
                        }
                        record[key] = json!(value.to_ascii_lowercase());
                    }
                }
                // The invocation is server-owned, so usage can be separated into initial work,
                // retries, fixes and the supported resume without trusting agent prose (R339).
                record["invocation"] = meta(&tx, "invocation")?;
                record["observed_at"] = json!(canon::utc_now_o());
                resource["conversations"][at]["usage"] = record;
            } else {
                // A resource-wide reading is a measurement, not a run cost: a later reading of the
                // rolling total replaces it, and the report still keeps it apart from per-run usage
                // (R344). Only the last reading is retained as this resource's evidence.
                resource["usage"] = usage;
            }
        }
        "usage_unavailable" => {
            // R345: a provider that exposes only account-wide aggregates leaves this run's usage
            // unknown. The report states the known subtotal and the missing count; it never
            // differences two rolling totals to invent one.
            let id = resource_id(&input, "conversation")?;
            let at = conversation_at(resource, &id)?;
            let reason = input["reason"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .context("a missing usage record names its reason")?;
            if reason.chars().count() > 200 || reason.chars().any(char::is_control) {
                bail!("a usage reason is at most 200 readable characters");
            }
            let run = input["run"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| resource["conversations"][at]["run"].as_str().map(str::to_string));
            resource["conversations"][at]["usage"] = json!({
                "run":run,
                "unavailable":reason,
                "invocation":meta(&tx, "invocation")?,
                "observed_at":canon::utc_now_o()
            });
        }
        "deleted" => {
            let id = resource_id(&input, "conversation")?;
            let at = conversation_at(resource, &id)?;
            let result = input["result"]
                .as_str()
                .map(str::to_ascii_lowercase)
                .context("a deletion observation records its result")?;
            if !["confirmed", "already_absent"].contains(&result.as_str()) {
                bail!("a deletion result is confirmed or already_absent, not `{result}`");
            }
            resource["conversations"][at]["delete"] = json!(result);
        }
        "delete_failed" => {
            let id = resource_id(&input, "conversation")?;
            let at = conversation_at(resource, &id)?;
            let code = input["code"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .context("a failed deletion records its code")?;
            let message = input["message"].as_str().unwrap_or_default();
            resource["conversations"][at]["delete"] = json!("failed");
            resource["conversations"][at]["delete_error"] =
                json!({"code":code,"message":message,"at":canon::utc_now_o()});
        }
        "listing" => {
            let mut listed = Vec::new();
            for entry in input["conversations"].as_array().cloned().unwrap_or_default() {
                let id = resource_id(&entry, "id")?;
                let state = entry["state"].as_str().unwrap_or("unknown");
                listed.push(json!({"id":id,"state":state}));
            }
            let mut unknown = Vec::new();
            for entry in input["unknown"].as_array().cloned().unwrap_or_default() {
                let id = resource_id(&entry, "id")?;
                let title = entry["title"].as_str().unwrap_or_default();
                unknown.push(json!({"id":id,"title":title}));
            }
            resource["listing"] =
                json!({"conversations":listed,"unknown":unknown,"observed_at":canon::utc_now_o()});
        }
        "workspace" => {
            let removed = input["removed"]
                .as_bool()
                .context("a workspace observation records removed=true|false")?;
            resource["workspace_removed"] = json!(removed);
        }
        _ => {
            // A lost creation response is recovered only by this run's own label (R322).
            let id = resource_id(&input, "conversation")?;
            let label = input["label"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .context("a recovery names the label it searched for")?;
            if label != field(resource, "label")? {
                bail!("a recovered conversation must carry this run's own resource label");
            }
            if known_ids.contains(&id) {
                bail!("operation_conflict: {id} is already recorded for this run");
            }
            let at = resource["conversations"]
                .as_array()
                .and_then(|list| list.iter().position(|conversation| conversation["lost"] == true))
                .context("no lost creation response is recorded for this resource")?;
            resource["conversations"][at]["id"] = json!(id);
            resource["conversations"][at]["lost"] = json!(false);
            resource["conversations"][at]["recovered_at"] = json!(canon::utc_now_o());
        }
    }
    write_resources(&tx, &cleanup_unit, &resources)?;
    let facts = json!({ "op": op, "resource": target["id"], "record": input });
    event(&tx, &cleanup_unit, None, None, "resource_observed", &facts)?;
    tx.commit()?;
    Ok(json!({
        "schema_version":1,
        "action":"observe",
        "op":op,
        "cleanup_unit":cleanup_unit,
        "resources":resource_view(&resources),
    }))
}

/// The required aggregate obligations of one run: preparation, the enabled Terms table, and every
/// enabled aggregate publication. A run is content-complete when all of them are current.
fn required_layers(conn: &Connection) -> Result<Vec<&'static str>> {
    let target = meta(conn, "target")?;
    let layers = target["enabled"]
        .as_array()
        .context("missing declared target")?;
    let enabled = |layer: &str| layers.iter().any(|value| value == layer);
    let mut required = vec!["prepare"];
    if enabled("terms") {
        required.push("terms");
    }
    required.extend(BOOK.iter().copied().filter(|layer| enabled(layer)));
    if meta(conn, "preparation")?["variant"] == "math" {
        required.extend(MATH_BOOK.iter().copied().filter(|layer| enabled(layer)));
    }
    Ok(required)
}

/// The content half of the run: every unit outside the cleanup layer is terminal and no content
/// failure or stale required publication remains. Cleanup reports this independently of its own
/// completion, so a cleanup failure never erases content success (R330).
fn content_facts(tx: &Connection) -> Result<Value> {
    let mut statement =
        tx.prepare("SELECT id,state,fault_json FROM units WHERE layer<>'cleanup' ORDER BY id")?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut unresolved = Vec::new();
    for (id, state, fault) in rows {
        let superseded = parsed_fault(fault.as_deref())?["code"] == "superseded";
        if state != "done" && !superseded {
            unresolved.push(id);
        }
    }
    let root_hash = canon::hash_value(&json!({"preparation_hash":meta(tx, "preparation_hash")?}));
    let mut publications = Vec::new();
    for layer in required_layers(tx)? {
        let latest = required_publication(tx, layer)?;
        let current = match &latest {
            None => false,
            Some((_, raw)) => publication_current(tx, layer, &canon::parse_json(raw)?, &root_hash)?,
        };
        publications.push(json!({
            "layer":layer,
            "unit":latest.map(|(id,_)|id),
            "current":current
        }));
    }
    let stale: Vec<Value> = publications
        .iter()
        .filter(|publication| publication["current"] != true)
        .cloned()
        .collect();
    Ok(json!({
        "complete":unresolved.is_empty() && stale.is_empty(),
        "unresolved":unresolved,
        "required_publications":publications,
        "stale_publications":stale
    }))
}

/// The staging subtrees a cleanup never removes: accepted plans, part maps, source spans, receipts,
/// reports and the retained evidence itself stay for resume and audit (R328).
fn retained_subtree(p: &Prepared, path: &Path) -> Option<&'static str> {
    let path = local_path(path);
    for name in ["groups", "sections", "notes", "cleanup", "locks"] {
        let root = local_path(&p.out.join(name));
        if path == root || path.starts_with(&root) {
            return Some(name);
        }
    }
    None
}

/// A file a cleanup may remove only after an identical retained copy exists outside its subtree.
fn protected_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "plan.json"
        || lower == "state.json"
        || lower == "manifest.json"
        || lower.starts_with("part-map")
        || lower.starts_with("source-review")
        || lower.starts_with("accepted")
        || lower.contains("report")
        || lower.contains("receipt")
        || lower.contains("completion")
        || lower.contains("source-faults")
        || lower.contains("self-check")
}

/// One resolved scratch target: inside the exact staging root, outside every retained subtree, and
/// every protected file in it already copied to the durable evidence path (R328, R329).
fn scratch_target(p: &Prepared, entry: &Value) -> Result<PathBuf> {
    let given = field(entry, "path")?;
    let path = std::fs::canonicalize(given)
        .with_context(|| format!("missing scratch target {given}"))?;
    if !path.starts_with(&p.out) || path == p.out {
        bail!(
            "scratch target {} is outside the exact staging root {}",
            path.display(),
            p.out.display()
        );
    }
    if let Some(name) = retained_subtree(p, &path) {
        bail!(
            "scratch target {} lies in the retained `{name}` subtree; retained material never belongs to a cleanup",
            path.display()
        );
    }
    if path.parent() == Some(p.out.as_path()) {
        bail!(
            "scratch target {} is a top-level staging file, not compiler scratch",
            path.display()
        );
    }
    Ok(path)
}

/// Every file under a target, in a stable order.
fn tree_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(next) = stack.pop() {
        if next.is_dir() {
            let mut entries = std::fs::read_dir(&next)?
                .map(|entry| entry.map(|e| e.path()))
                .collect::<std::io::Result<Vec<_>>>()?;
            entries.sort();
            stack.extend(entries);
        } else if next.is_file() {
            files.push(next);
        }
    }
    files.sort();
    Ok(files)
}

/// Validate one scratch target and remove it when asked. A protected file needs its durable copy
/// first; nothing is ever removed because an agent named it in free text. A target that is already
/// absent is a successful replay.
fn scratch_settle(p: &Prepared, entry: &Value, remove: bool) -> Result<Value> {
    declared_scratch(p, field(entry, "path")?)?;
    let absent = !Path::new(field(entry, "path")?).exists();
    let path = if absent {
        PathBuf::from(std::path::absolute(Path::new(field(entry, "path")?))?)
    } else {
        scratch_target(p, entry)?
    };
    if absent {
        return Ok(json!({"path":path.to_string_lossy(),"removed":true,"retained":[]}));
    }
    let retained = entry["retained"].as_array().cloned().unwrap_or_default();
    let mut copies = Vec::new();
    for item in &retained {
        let source = field(item, "from")?.to_string();
        let copy = field(item, "path")?.to_string();
        let from = std::fs::canonicalize(&source)
            .with_context(|| format!("missing retained source {source}"))?;
        let to = std::fs::canonicalize(&copy)
            .with_context(|| format!("missing retained copy {copy}"))?;
        if !from.starts_with(&path) {
            bail!(
                "retained source {} does not belong to the scratch target",
                from.display()
            );
        }
        if !to.starts_with(cleanup_evidence(p)?) {
            bail!(
                "retained copy {} is outside the durable evidence path",
                to.display()
            );
        }
        let source = std::fs::read(&from)?;
        let copy = std::fs::read(&to)?;
        if canon::sha256_hex(&source) != canon::sha256_hex(&copy) {
            bail!(
                "retained copy {} does not hold the bytes of {}; copy it before removing the scratch",
                to.display(),
                from.display()
            );
        }
        copies.push(json!({"from":from.to_string_lossy(),"path":to.to_string_lossy(),"sha256":canon::sha256_hex(&copy)}));
    }
    for file in tree_files(&path)? {
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if !protected_name(&name) {
            continue;
        }
        let canonical = std::fs::canonicalize(&file)?;
        if !copies
            .iter()
            .any(|copy| copy["from"].as_str() == Some(canonical.to_string_lossy().as_ref()))
        {
            bail!(
                "retained_material_missing: {} holds an accepted plan, receipt or report with no durable copy",
                canonical.display()
            );
        }
    }
    let removed = if remove {
        if path.is_dir() {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove scratch {}", path.display()))?;
        } else {
            std::fs::remove_file(&path)?;
        }
        true
    } else {
        false
    };
    Ok(json!({"path":path.to_string_lossy(),"removed":removed,"retained":copies}))
}

/// One reason a cleanup cannot be called complete. `transient` is retryable, `operator` needs the
/// record fixed, and `reconcile` is a terminal workspace state a human must reconcile.
fn cleanup_issue(
    class: &str,
    code: &str,
    resource: &Value,
    conversation: Option<&Value>,
    cause: &str,
) -> Value {
    json!({
        "class":class,
        "code":code,
        "resource":resource["id"],
        "cleanup_unit":resource["cleanup_unit"],
        "owner":resource["owner"],
        "workspace":resource["workspace"]["id"],
        "conversation":conversation.map(|c| c["id"].clone()).unwrap_or(Value::Null),
        "cause":cause
    })
}

/// Verify the recorded resources of one cleanup unit (or of every unit, for the run-level unit) and
/// settle only what the ledger can prove: the exact ids, the retained evidence, and the workspace
/// rule that no unknown member is removed wholesale (R324–R330).
fn cleanup_assessment(p: &Prepared, unit: &str, remove: bool) -> Result<Value> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_binding(&tx, p)?;
    let run_level = unit == "cleanup/book@1";
    let all = recorded_resources(&tx, None)?;
    let resources: Vec<Value> = if run_level {
        all.clone()
    } else {
        all.iter()
            .filter(|resource| resource["cleanup_unit"] == json!(unit))
            .cloned()
            .collect()
    };
    let content = content_facts(&tx)?;
    tx.commit()?;
    drop(conn);
    if resources.is_empty() && !run_level {
        bail!("cleanup_not_registered: {unit} has no recorded resource to settle");
    }
    let deleted_id = |resource: &Value, id: &str| -> bool {
        resource["conversations"].as_array().is_some_and(|list| {
            list.iter().any(|conversation| {
                conversation["id"] == json!(id)
                    && matches!(
                        conversation["delete"].as_str(),
                        Some("confirmed") | Some("already_absent")
                    )
            })
        })
    };
    let mut issues = Vec::new();
    let mut views = Vec::new();
    let mut scratch_removed = Vec::new();
    let mut retained = Vec::new();
    for resource in &resources {
        let facts = resource_facts(resource);
        for conversation in resource["conversations"].as_array().cloned().unwrap_or_default() {
            if conversation["lost"] == true || conversation["id"].is_null() {
                issues.push(cleanup_issue(
                    "reconcile",
                    "lost_creation_response",
                    resource,
                    Some(&conversation),
                    "the creation response was lost and this run's label found no conversation",
                ));
                continue;
            }
            if conversation["state"] != json!("terminal") {
                issues.push(cleanup_issue(
                    "transient",
                    "cleanup_transient",
                    resource,
                    Some(&conversation),
                    "the external run is not terminal yet; cancel it and observe it before deleting it",
                ));
                continue;
            }
            if !conversation["evidence"].is_object() {
                issues.push(cleanup_issue(
                    "operator",
                    "cleanup_evidence_missing",
                    resource,
                    Some(&conversation),
                    "no accepted answer or transcript evidence was retained for this conversation",
                ));
            }
            if conversation["delete"] == json!("failed") {
                let error = &conversation["delete_error"];
                issues.push(cleanup_issue(
                    "transient",
                    "cleanup_transient",
                    resource,
                    Some(&conversation),
                    &format!(
                        "the deletion failed ({}); retry it for this exact id",
                        error["code"].as_str().unwrap_or("unknown")
                    ),
                ));
            } else if !matches!(
                conversation["delete"].as_str(),
                Some("confirmed") | Some("already_absent")
            ) {
                issues.push(cleanup_issue(
                    "transient",
                    "cleanup_transient",
                    resource,
                    Some(&conversation),
                    "the recorded conversation has no confirmed deletion yet",
                ));
            }
        }
        if !usage_retained(resource) {
            issues.push(cleanup_issue(
                "operator",
                "cleanup_evidence_missing",
                resource,
                None,
                "no per-run usage was retained for this resource",
            ));
        }
        let unknown = resource["listing"]["unknown"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !unknown.is_empty() {
            let ids = unknown
                .iter()
                .filter_map(|member| member["id"].as_str())
                .collect::<Vec<_>>()
                .join(", ");
            issues.push(cleanup_issue(
                "reconcile",
                "workspace_unknown_member",
                resource,
                None,
                &format!("the workspace still holds conversations this run never recorded: {ids}"),
            ));
        }
        let workspace = resource["workspace"]["id"].as_str().unwrap_or_default();
        let same_workspace: Vec<&Value> = all
            .iter()
            .filter(|other| other["workspace"]["id"].as_str() == Some(workspace))
            .collect();
        // The workspace rule is one fact about the whole shared workspace (R327): every conversation
        // this run recorded there is absent and no other member remains.
        let workspace_unknown: Vec<Value> = same_workspace
            .iter()
            .flat_map(|other| {
                other["listing"]["unknown"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();
        let workspace_settled = workspace_unknown.is_empty()
            && same_workspace.iter().all(|other| {
                other["conversations"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .filter(|conversation| !conversation["id"].is_null())
                    .all(|conversation| deleted_id(other, conversation["id"].as_str().unwrap_or_default()))
            });
        if resource["workspace_removed"] == true {
            if !workspace_settled {
                issues.push(cleanup_issue(
                    "operator",
                    "cleanup_workspace_removed_early",
                    resource,
                    None,
                    "the workspace was reported removed while a recorded conversation or an unknown member remained",
                ));
            }
        } else if run_level {
            issues.push(cleanup_issue(
                "transient",
                "cleanup_transient",
                resource,
                None,
                "the owned workspace is not removed yet; remove it once every recorded conversation is absent",
            ));
        }
        for entry in resource["scratch"].as_array().cloned().unwrap_or_default() {
            match scratch_settle(p, &entry, remove) {
                Ok(settled) => {
                    scratch_removed.push(settled["path"].clone());
                    if let Some(copies) = settled["retained"].as_array() {
                        retained.extend(copies.iter().cloned());
                    }
                }
                Err(error) => issues.push(cleanup_issue(
                    "operator",
                    "cleanup_scratch_refused",
                    resource,
                    None,
                    &format!("{error:#}"),
                )),
            }
        }
        views.push(facts);
    }
    let hard = issues
        .iter()
        .filter(|issue| issue["class"] != "reconcile")
        .count();
    let reconcile: Vec<Value> = issues
        .iter()
        .filter(|issue| issue["class"] == "reconcile")
        .cloned()
        .collect();
    let cleanup_complete = issues.is_empty();
    Ok(json!({
        "schema_version":1,
        "unit":unit,
        "run_level":run_level,
        "content_complete":content["complete"],
        "cleanup_complete":cleanup_complete,
        "issues":issues,
        "hard_issues":hard,
        "reconciliation":reconcile,
        "content":content,
        "resources":views,
        "scratch":{"removed":scratch_removed,"retained":retained},
        "status":if content["complete"] == true && cleanup_complete {"complete"} else {"partial"}
    }))
}

/// The checked completion of one cleanup unit. A transient or unverifiable fact refuses with its own
/// code so the driver records a retry; a terminal reconciliation records a partial completion instead,
/// because no retry can remove a conversation this run does not own (R327, R330).
fn cleanup_finish(p: &Prepared, unit: &str, token: i64, check_only: bool) -> Result<Value> {
    let _guard = unit_guard(p, unit)?;
    let (claimed, _progress, run, done) = finish_claim(p, unit, token)?;
    if done {
        let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
        let raw: String = conn.query_row(
            "SELECT result_json FROM units WHERE id=?1 AND state='done' AND claim_token=?2",
            params![unit, token],
            |row| row.get(0),
        )?;
        let mut result: Value = serde_json::from_str(&raw)?;
        drop(conn);
        let current = cleanup_assessment(p, unit, false)?;
        if current["content_complete"] != result["content_complete"]
            || current["cleanup_complete"] != result["cleanup_complete"]
            || current["resources"] != result["resources"]
        {
            bail!("completed cleanup unit changed; keep the checked result and re-run from a new claim");
        }
        result["reused"] = json!(true);
        if check_only {
            result["check_only"] = json!(true);
            result["applied"] = json!(false);
        }
        return Ok(result);
    }
    let assessment = cleanup_assessment(p, unit, !check_only)?;
    let issues = assessment["issues"].as_array().cloned().unwrap_or_default();
    if let Some(issue) = issues.iter().find(|issue| issue["class"] != "reconcile") {
        bail!("{}: {}", field(issue, "code")?, field(issue, "cause")?);
    }
    let mut result = json!({
        "schema_version":1,
        "kind":"cleanup_completion",
        "unit":unit,
        "token":token,
        "run_id":run,
        "owner":claimed.owner,
        "state":"done",
        "status":assessment["status"],
        "content_complete":assessment["content_complete"],
        "cleanup_complete":assessment["cleanup_complete"],
        "partial_reasons":issues,
        "reconciliation":assessment["reconciliation"],
        "content":assessment["content"],
        "resources":assessment["resources"],
        "scratch":assessment["scratch"],
        "input_hash":claimed.input_hash,
        "unit_input_hash":claimed.input_hash
    });
    if check_only {
        result["check_only"] = json!(true);
        result["applied"] = json!(false);
        return Ok(result);
    }
    let key = unit
        .strip_prefix("cleanup/")
        .and_then(|value| value.strip_suffix("@1"))
        .with_context(|| format!("not a cleanup unit: {unit}"))?;
    renew(p, unit, token)?;
    let folder = p.out.join("cleanup");
    std::fs::create_dir_all(&folder)?;
    let completion = publish(
        &folder.join(format!("completion-{key}.json")),
        &serde_json::to_string_pretty(&result)?,
    )?;
    result["files"] = json!({"completion":completion});
    check_file(&completion, "cleanup completion evidence")?;
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, unit, token)?;
    output(
        &tx,
        unit,
        "completion",
        "file",
        field(&completion, "path")?,
        field(&completion, "sha256")?,
        &completion,
    )?;
    for path in result["scratch"]["removed"].as_array().cloned().unwrap_or_default() {
        let locator = path.as_str().unwrap_or_default();
        let fingerprint = canon::sha256_hex(locator.as_bytes());
        output(
            &tx,
            unit,
            &format!("scratch/{fingerprint}"),
            "scratch",
            locator,
            &fingerprint,
            &json!({"path":path}),
        )?;
    }
    completion_fault(p, "after_outputs")?;
    completion_fault(p, "before_commit")?;
    tx.execute("UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",params![unit,canon::canonical_json(&result),canon::utc_now_o()])?;
    event(
        &tx,
        unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({
            "disposition":result["status"],
            "content_complete":result["content_complete"],
            "cleanup_complete":result["cleanup_complete"],
            "resources":result["resources"].as_array().map(Vec::len),
            "completion":completion
        }),
    )?;
    completion_fault(p, "after_commit")?;
    tx.commit()?;
    Ok(result)
}

/// `cleanup-finish -Ready`: claims every ready cleanup unit and settles each through its checked verifier
/// in one process, the run-level unit last; a unit that cannot settle gets its retryable fault with the
/// claim's own hash and is claimed again within the run's attempt budget. ebrill2001modern trial 1 spent one
/// agent per unit (about 150, one after another), and every failed finish left its unit claimed. Prints one
/// short line: counts, the first open causes, and the run-level unit's completeness.
fn cleanup_ready(p: &Prepared, owner: &str) -> Result<Value> {
    const ROUNDS: usize = 6;
    let mut claimed = 0usize;
    let mut done = 0usize;
    let mut partial = 0usize;
    let mut open: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut book = Value::Null;
    let mut rounds = 0usize;
    for round in 0..ROUNDS {
        // A unit faulted in an earlier round becomes ready after its short retry delay.
        let wait = if round > 0 && open.values().any(|(state, _)| state == "retry") { 30 } else { 0 };
        let mut units = next(p, "cleanup", 64, owner, wait, 0)?["units"].as_array().cloned().unwrap_or_default();
        if units.is_empty() {
            break;
        }
        rounds += 1;
        units.sort_by_key(|unit| unit["id"] == json!("cleanup/book@1"));
        for unit in units {
            claimed += 1;
            let id = field(&unit, "id")?.to_string();
            let token = unit["token"].as_i64().context("a claimed unit carries its token")?;
            let outcome = match cleanup_finish(p, &id, token, false) {
                Ok(result) => {
                    done += 1;
                    if result["status"] == json!("partial") {
                        partial += 1;
                    }
                    open.remove(&id);
                    Ok(result)
                }
                Err(error) => {
                    let cause: String = format!("{error:#}").chars().take(300).collect();
                    let fault = json!({"schema_version":1,"unit":id,"input_hash":unit["input_hash"],"code":"provider_transient","cause":cause});
                    let state = match fail(p, &id, token, "retryable", &fault) {
                        Ok(failed) => failed["state"].as_str().unwrap_or("failed").to_string(),
                        Err(refused) => format!("claimed: {refused:#}").chars().take(200).collect(),
                    };
                    open.insert(id.clone(), (state, cause.clone()));
                    Err(cause)
                }
            };
            if id == "cleanup/book@1" {
                book = match outcome {
                    Ok(result) => json!({
                        "unit":id,"ok":true,"status":result["status"],
                        "content_complete":result["content_complete"],"cleanup_complete":result["cleanup_complete"],
                        "partial_reasons":result["partial_reasons"].as_array().map(|list| list.iter().take(8).map(|r| json!({"code":r["code"],"cause":r["cause"].as_str().map(|c| c.chars().take(160).collect::<String>())})).collect::<Vec<_>>()),
                        "reconciliation":result["reconciliation"].as_array().map(Vec::len).unwrap_or(0),
                        "scratch_removed":result["scratch"]["removed"].as_array().map(Vec::len).unwrap_or(0)
                    }),
                    Err(cause) => json!({"unit":id,"ok":false,"cause":cause}),
                };
            }
        }
        if !open.values().any(|(state, _)| state == "retry") {
            break;
        }
    }
    let list = |wanted: &dyn Fn(&str) -> bool| -> Vec<Value> {
        open.iter().filter(|(_, (state, _))| wanted(state)).take(8).map(|(unit, (state, cause))| json!({"unit":unit,"state":state,"cause":cause})).collect()
    };
    let count = |wanted: &dyn Fn(&str) -> bool| open.values().filter(|(state, _)| wanted(state)).count();
    Ok(json!({
        "schema_version":1,"kind":"cleanup_ready","rounds":rounds,"claimed":claimed,"done":done,"partial":partial,
        "retry_count":count(&|state| state == "retry"),"failed_count":count(&|state| state != "retry"),
        "retry":list(&|state| state == "retry"),"failed":list(&|state| state != "retry"),
        "book":book
    }))
}

pub fn cleanup_finish_cli(argv: &[String]) -> Result<i32> {
    let parsed = (|| -> Result<(String, String, i64, bool, Option<String>)> {
        let a = Args::parse(argv, &["Out", "Unit", "Token", "Owner"], &["CheckOnly", "Ready"])?;
        if !a.positional.is_empty() || a.required("Out")?.trim().is_empty() {
            bail!("explicit -Out is required");
        }
        if a.flag("Ready") {
            if a.has("Unit") || a.has("Token") || a.flag("CheckOnly") {
                bail!("-Ready claims its own units; it takes no -Unit, -Token or -CheckOnly");
            }
            let owner = if a.has("Owner") { a.str("Owner") } else { "cleanup-ready".to_string() };
            if owner.trim().is_empty() {
                bail!("-Owner must not be empty");
            }
            return Ok((a.str("Out"), String::new(), 0, false, Some(owner)));
        }
        if a.required("Unit")?.trim().is_empty() {
            bail!("-Unit is required");
        }
        let token = a
            .required("Token")?
            .parse::<i64>()
            .ok()
            .filter(|number| *number > 0)
            .context("-Token must be positive")?;
        Ok((a.str("Out"), a.str("Unit"), token, a.flag("CheckOnly"), None))
    })();
    let (out, unit, token, check_only, ready) = match parsed {
        Ok(value) => value,
        Err(error) => {
            eprintln!("zt-digest cleanup-finish: {error}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let result = match ready {
        Some(owner) => cleanup_ready(&p, &owner)?,
        None => cleanup_finish(&p, &unit, token, check_only)?,
    };
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let command = argv
        .first()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    // R410 (#88): legacy adoption has its own operation and its own parameters. It is never
    // reached through `init`, and `init` never fabricates a legacy folder's history.
    if command == "adopt" {
        return adopt::cli(argv);
    }
    let parsed = (|| -> Result<Args> {
        if !matches!(
            command.as_str(),
            "init"
                | "status"
                | "gate"
                | "next"
                | "renew"
                | "hold"
                | "done"
                | "fail"
                | "rerun-list"
                | "publish"
                | "notes"
                | "relocate"
                | "math"
                | "reclaim"
                | "job"
                | "resource"
                | "report"
                | "resume"
        ) {
            bail!(
                "ledger requires init, status, gate, next, renew, hold, done, fail, rerun-list, resume, publish, notes, relocate, math, reclaim, job, resource, report or adopt"
            );
        }        let params = if command == "report" || command == "resume" {
            vec!["Out"]
        } else if command == "next" {
            vec!["Out", "Layer", "Take", "Owner", "Wait", "Lease"]
        } else if command == "renew" {
            vec!["Out", "Unit", "Token"]
        } else if command == "hold" {
            vec!["Out", "Units", "Seconds", "Every"]
        } else if command == "fail" {
            vec!["Out", "Unit", "Token", "Class", "Fault"]
        } else if command == "done" {
            vec!["Out", "Unit", "Token", "Outputs"]
        } else if command == "publish" {
            vec!["Out", "Layer", "Key", "Scope"]
        } else if command == "notes" {
            vec!["Out", "Suggestions"]
        } else if command == "relocate" {
            vec!["Out", "Moves"]
        } else if command == "math" {
            vec!["Out"]
        } else if command == "reclaim" {
            vec!["Out", "Unit", "Take"]
        } else if command == "job" {
            vec!["Out", "Unit", "Token", "Register", "Observe", "Cancel"]
        } else if command == "resource" {
            vec!["Out", "Unit", "Token", "Register", "Observe", "Recover", "RegisterId", "Workspace", "EvidenceFor", "Evidence", "Usage", "Settle", "State", "Deleted", "Message", "SettleWorkspace", "Removed", "Remaining", "Unknown", "SweepEvidence"]
        } else if command == "status" {
            vec!["Out", "Layer", "Units", "Held"]
        } else {
            vec!["Out", "Layer"]
        };
        let a = Args::parse(
            &argv[1..],
            &params,
            if command == "job" {
                &["List"]
            } else if command == "resource" {
                &["List", "Brief", "Counts"]
            } else if command == "status" {
                &["Verify", "Summary"]
            } else if command == "reclaim" {
                &["Release"]
            } else if command == "next" || command == "gate" {
                &["Brief"]
            } else {
                &[]
            },
        )?;
        a.required("Out")?;
        if a.str("Out").trim().is_empty() {
            bail!("-Out is required");
        }
        if matches!(command.as_str(), "gate" | "next") {
            a.required("Layer")?;
        }
        if command == "publish" {
            a.required("Layer")?;
            if !PUBLICATION.contains(&a.str("Layer").as_str()) {
                bail!("-Layer must be an aggregate publication layer: {}", PUBLICATION.join(", "));
            }
            let key = a.str_or("Key", "book");
            if key.len() > 200
                || !key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            {
                bail!("-Key must be a readable logical key of at most 200 bytes");
            }
        }
        if command == "next" {
            a.required("Take")?
                .parse::<i64>()
                .ok()
                .filter(|n| (1..=64).contains(n))
                .context("-Take must be a positive integer from 1 through 64")?;
            if a.required("Owner")?.trim().is_empty() || a.str("Owner").len() > 256 {
                bail!("-Owner must be nonempty and at most 256 bytes");
            }
            if a.has("Wait") {
                a.required("Wait")?
                    .parse::<i64>()
                    .ok()
                    .filter(|n| (0..=600).contains(n))
                    .context("-Wait must be a whole number of seconds from 0 through 600")?;
            }
            if a.has("Lease") {
                a.required("Lease")?
                    .parse::<i64>()
                    .ok()
                    .filter(|n| (1..=LEASE_MAX).contains(n))
                    .with_context(|| format!("-Lease must be a whole number of seconds from 1 through {LEASE_MAX}"))?;
            }
        }
        if matches!(command.as_str(), "renew" | "fail" | "done") {
            if a.required("Unit")?.trim().is_empty() {
                bail!("-Unit is required");
            }
            a.required("Token")?
                .parse::<i64>()
                .ok()
                .filter(|n| *n > 0)
                .context("-Token must be a positive integer")?;
        }
        if command == "reclaim" {
            if a.has("Unit") && a.str("Unit").trim().is_empty() {
                bail!("-Unit must not be empty");
            }
            if a.has("Take") {
                a.required("Take")?
                    .parse::<i64>()
                    .ok()
                    .filter(|n| (1..=64).contains(n))
                    .context("-Take must be a positive integer from 1 through 64")?;
            }
        }
        if command == "job" {
            if a.required("Unit")?.trim().is_empty() {
                bail!("-Unit is required");
            }
            let actions = ["Register", "Observe", "Cancel"]
                .iter()
                .filter(|name| a.has(name))
                .count()
                + usize::from(a.flag("List"));
            if actions != 1 {
                bail!("job takes exactly one of -Register, -Observe, -Cancel or -List");
            }
            if a.has("Register") || a.has("Observe") {
                a.required(if a.has("Register") { "Register" } else { "Observe" })?;
            }
            if a.has("Register") {
                a.required("Token")?
                    .parse::<i64>()
                    .ok()
                    .filter(|n| *n > 0)
                    .context("-Token must be a positive integer")?;
            }
        }
        if command == "resource" {
            if a.has("Token") {
                a.required("Token")?
                    .parse::<i64>()
                    .ok()
                    .filter(|n| *n > 0)
                    .context("-Token must be a positive integer")?;
            }
            // -SweepEvidence is an action of its own (grossman2002integration trial 19: it was left out of this count, so
            // the cleanup stage's evidence sweep was always refused).
            let actions = ["Register", "Observe", "Recover", "RegisterId", "EvidenceFor", "Settle", "SettleWorkspace", "SweepEvidence"]
                .iter()
                .filter(|name| a.has(name))
                .count()
                + usize::from(a.flag("List"));
            if actions != 1 {
                bail!("resource takes exactly one of -Register, -Observe, -Recover, -RegisterId, -EvidenceFor, -Settle, -SettleWorkspace, -SweepEvidence or -List");
            }
            if a.has("Settle") && (a.required("State")?.trim().is_empty() || a.required("Deleted")?.trim().is_empty()) {
                bail!("-Settle needs -State and -Deleted");
            }
            if a.has("SettleWorkspace") && !matches!(a.required("Removed")?.trim(), "true" | "false") {
                bail!("-SettleWorkspace needs -Removed true or false");
            }
            if a.has("RegisterId") {
                if a.required("RegisterId")?.trim().is_empty() || a.required("Workspace")?.trim().is_empty() {
                    bail!("-RegisterId needs a conversation id and -Workspace");
                }
                if a.required("Unit")?.trim() != "run" {
                    a.required("Token")?;
                }
            }
            // A cleanup claim holder fences its records with -Unit and -Token, always both.
            if ["EvidenceFor", "Settle", "SettleWorkspace"].iter().any(|name| a.has(name)) && a.has("Unit") != a.has("Token") {
                bail!("-Unit and -Token go together: the cleanup unit you hold and its claim token");
            }
            if a.has("EvidenceFor") && !a.has("Evidence") && !a.has("Usage") {
                bail!("-EvidenceFor needs -Evidence, -Usage, or both");
            }
            for name in ["Register", "Observe", "Recover"] {
                if a.has(name) && a.required(name)?.trim().is_empty() {
                    bail!("-{name} needs a file path");
                }
            }
            if a.has("Unit") && a.str("Unit").trim().is_empty() {
                bail!("-Unit must not be empty");
            }
        }
        if command == "fail" {
            a.choice("Class", &["retryable", "operator", "content"])?;
            a.required("Fault")?;
        }
        if command == "done" && !a.str("Unit").starts_with("cleanup/") {
            a.required("Outputs")?;
        }
        if command == "notes" && a.required("Suggestions")?.trim().is_empty() {
            bail!("-Suggestions is required");
        }
        if command == "relocate" && a.required("Moves")?.trim().is_empty() {
            bail!("-Moves is required");
        }
        if a.has("Layer")
            && !COMMON.contains(&a.str("Layer").as_str())
            && ![
                "events",
                "findings",
                "math-index",
                "math-docs",
                "math-skeleton",
                "notation",
                "boundary",
                "proof",
                "theory",
                "math-check",
            ]
            .contains(&a.str("Layer").as_str())
        {
            bail!("unknown ledger layer");
        }
        if !a.positional.is_empty() {
            bail!("unexpected ledger positional arguments");
        }
        if matches!(command.as_str(), "init" | "rerun-list" | "report" | "resume") && a.has("Layer") {
            bail!("{command} does not accept -Layer");
        }
        Ok(a)
    })();
    let a = match parsed {
        Ok(a) => a,
        Err(e) => {
            eprintln!("zt-digest ledger: {e}");
            return Ok(2);
        }
    };
    let result = if command == "publish" {
        let mut zt = crate::zt::start()?;
        let p = Prepared::read(&a.str("Out"), &mut zt)?;
        publication(&p, &a.str("Layer"), &a.str_or("Key", "book"), &a.str("Scope"))?
    } else if command == "reclaim" {
        let mut zt = crate::zt::start()?;
        let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
        let only = a.has("Unit").then(|| a.str("Unit"));
        reclaim(&p, only.as_deref(), a.int_or("Take", 64)?, a.flag("Release"))?
    } else if command == "job" {
        let mut zt = crate::zt::start()?;
        let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
        let unit = a.str("Unit");
        if a.has("Register") {
            job_register(&p, &unit, a.str("Token").parse()?, &a.str("Register"))?
        } else if a.has("Observe") {
            job_observe(&p, &unit, &a.str("Observe"))?
        } else if a.has("Cancel") {
            job_cancel(&p, &unit, &a.str("Cancel"))?
        } else {
            job_list(&p, &unit)?
        }
    } else if command == "resource" && ["RegisterId", "Settle", "SettleWorkspace", "EvidenceFor"].iter().any(|name| a.has(name)) {
        let mut zt = crate::zt::start()?;
        let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
        let recorded = if a.has("RegisterId") {
            let token = a.has("Token").then(|| a.str("Token").parse::<i64>()).transpose()?;
            resource_register_id(&p, &a.str("Unit"), token, &a.str("RegisterId"), &a.str("Workspace"))?
        } else if a.has("Settle") {
            resource_settle(&p, &a.str("Settle"), &a.str("State"), &a.str("Deleted"), &a.str("Message"), &a.str("Unit"), record_token(&a)?)?
        } else if a.has("SettleWorkspace") {
            resource_settle_workspace(&p, &a.str("SettleWorkspace"), a.str("Removed").trim() == "true", &a.str("Remaining"), &a.str("Unknown"), &a.str("Unit"), record_token(&a)?)?
        } else {
            let evidence = a.has("Evidence").then(|| a.str("Evidence"));
            let usage = a.has("Usage").then(|| a.str("Usage"));
            resource_evidence_for(&p, &a.str("EvidenceFor"), evidence.as_deref(), usage.as_deref(), &a.str("Unit"), record_token(&a)?)?
        };
        let (key, value) = if a.has("RegisterId") {
            ("conversation", a.str("RegisterId"))
        } else if a.has("Settle") {
            ("conversation", a.str("Settle"))
        } else if a.has("SettleWorkspace") {
            ("workspace", a.str("SettleWorkspace"))
        } else {
            ("conversation", a.str("EvidenceFor"))
        };
        let mut brief = record_brief(&recorded);
        brief[key] = json!(value.trim());
        brief
    } else if command == "resource" {
        if a.has("Register") {
            let mut zt = crate::zt::start()?;
            let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
            resource_register(&p, &a.str("Register"))?
        } else if a.has("RegisterId") {
            let mut zt = crate::zt::start()?;
            let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
            let token = a.has("Token").then(|| a.str("Token").parse::<i64>()).transpose()?;
            resource_register_id(&p, &a.str("Unit"), token, &a.str("RegisterId"), &a.str("Workspace"))?
        } else if a.has("Settle") {
            let mut zt = crate::zt::start()?;
            let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
            resource_settle(&p, &a.str("Settle"), &a.str("State"), &a.str("Deleted"), &a.str("Message"), &a.str("Unit"), record_token(&a)?)?
        } else if a.has("SettleWorkspace") {
            let mut zt = crate::zt::start()?;
            let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
            resource_settle_workspace(&p, &a.str("SettleWorkspace"), a.str("Removed").trim() == "true", &a.str("Remaining"), &a.str("Unknown"), &a.str("Unit"), record_token(&a)?)?
        } else if a.has("EvidenceFor") {
            let mut zt = crate::zt::start()?;
            let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
            let evidence = a.has("Evidence").then(|| a.str("Evidence"));
            let usage = a.has("Usage").then(|| a.str("Usage"));
            resource_evidence_for(&p, &a.str("EvidenceFor"), evidence.as_deref(), usage.as_deref(), &a.str("Unit"), record_token(&a)?)?
        } else if a.has("Observe") {
            let mut zt = crate::zt::start()?;
            let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
            resource_observe(&p, &a.str("Observe"), &a.str("Unit"), a.has("Token").then(|| a.str("Token").parse::<i64>()).transpose()?)?
        } else if a.has("SweepEvidence") {
            let mut zt = crate::zt::start()?;
            let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
            resource_sweep_evidence(&p, &a.str("SweepEvidence"))?
        } else {
            let mut zt = crate::zt::start()?;
            let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
            let listed = resource_list(&p, &a.str("Unit"), a.flag("Brief") || a.flag("Counts") || a.has("Workspace"))?;
            if a.flag("Counts") {
                resource_counts(&listed)
            } else if a.has("Workspace") {
                resource_brief_of(&listed, &a.str("Workspace"))
            } else {
                listed
            }
        }
    } else if command == "notes" {
        let mut zt = crate::zt::start()?;
        let p = Prepared::read(&a.str("Out"), &mut zt)?;
        register_notes(&p, &mut zt, &a.str("Suggestions"))?
    } else if command == "relocate" {
        let mut zt = crate::zt::start()?;
        let p = Prepared::read(&a.str("Out"), &mut zt)?;
        relocate(&p, &mut zt, &a.str("Moves"))?
    } else if command == "math" {
        let mut zt = crate::zt::start()?;
        let p = Prepared::read(&a.str("Out"), &mut zt)?;
        math_register(&p, &mut zt)?
    } else if matches!(
        command.as_str(),
        "init" | "next" | "renew" | "hold" | "done" | "fail"
    ) {
        let mut zt = crate::zt::start()?;
        let preparing = matches!(command.as_str(), "done" | "renew" | "fail")
            && a.str("Unit") == "prepare/book@1";
        let p = Prepared::read_mode(&a.str("Out"), &mut zt, !preparing)?;
        if command == "init" {
            init(&p)?
        } else if command == "renew" {
            renew(&p, &a.str("Unit"), a.str("Token").parse()?)?
        } else if command == "hold" {
            let bounded = |name: &str, default: u64, max: u64| -> Result<u64> {
                if !a.has(name) {
                    return Ok(default);
                }
                a.str(name).parse::<u64>().ok().filter(|n| (1..=max).contains(n)).with_context(|| format!("-{name} must be a whole number of seconds from 1 through {max}"))
            };
            hold(&p, &hold_targets(&a.str("Units"))?, bounded("Seconds", 540, 580)?, bounded("Every", 60, 120)?)?
        } else if command == "done" {
            let path = a.str("Outputs");
            if preparing {
                let result =
                    prepare_finish(&p, &mut zt, a.str("Token").parse()?, Path::new(&path))?;
                util::print_line(&serde_json::to_string(&result)?)?;
                return Ok(0);
            }
            let unit = a.str("Unit");
            let token = a.str("Token").parse::<i64>()?;
            if unit.starts_with("cleanup/") {
                // The cleanup verifier is self-contained: it re-reads the recorded resources, the
                // durable evidence and the staging root, so no caller-supplied record is accepted.
                let result = cleanup_finish(&p, &unit, token, false)?;
                util::print_line(&serde_json::to_string(&result)?)?;
                return Ok(0);
            }
            let evidence = canon::read_json(&path).context("invalid completion evidence")?;
            if unit == "terms/book@1" {
                if evidence["schema_version"] != 1
                    || evidence["kind"] != "terms_completion"
                    || evidence["unit"] != unit
                    || evidence["token"] != token
                    || std::fs::canonicalize(&path)?
                        != std::fs::canonicalize(p.out.join("terms/completion.json"))?
                {
                    bail!("invalid Terms completion evidence");
                }
                // The library verifier performs the same claim/job/output checks; no address shortcut.
                let (_, progress, _, _) = finish_claim(&p, "terms/book@1", token)?;
                let terms = progress["terms_config"]["max_terms"]
                    .as_i64()
                    .context("missing Terms planning options")?;
                let candidates = progress["terms_config"]["max_candidates"]
                    .as_i64()
                    .context("missing Terms planning options")?;
                terms_apply(&p, &mut zt, token, terms, candidates)?
            } else if publication_unit(&unit).is_ok() {
                publication_finish(&p, &mut zt, &unit, token, Path::new(&path))?
            } else if let Some(layer) = math_book_layer(&unit) {
                match layer {
                    "math-index" => math_index_finish(&p, &unit, token, Path::new(&path))?,
                    "math-docs" => math_docs_finish(&p, &unit, token, Path::new(&path))?,
                    "math-skeleton" => {
                        math_skeleton_finish(&p, &unit, token, &mut zt, Path::new(&path))?
                    }
                    "notation" => notation_finish(&p, &unit, token, &mut zt, Path::new(&path))?,
                    "math-check" => math_check_finish(&p, &unit, token, &mut zt, Path::new(&path))?,
                    _ => bail!("unknown mathematics layer `{layer}`"),
                }
            } else if unit.starts_with("boundary/") {
                boundary_finish(&p, &unit, token, &mut zt, Path::new(&path))?
            } else if unit.starts_with("proof/") {
                proof_finish(&p, &unit, token, &mut zt, Path::new(&path))?
            } else if unit.starts_with("notes/") {
                note_disposition(&p, &mut zt, &unit, token, Path::new(&path))?
            } else if let Some((layer, number)) = empirical_unit_of(&unit) {
                // The Events/Findings unit's checked output is its own compiler state and the
                // accepted candidate manifest: caller-supplied addresses cannot establish it.
                if evidence["schema_version"] != 1
                    || evidence["kind"] != layer.completion_kind
                    || evidence["unit"] != unit
                    || evidence["token"] != token
                {
                    bail!("invalid {} completion evidence", layer.layer);
                }
                if std::fs::canonicalize(&path)?
                    != std::fs::canonicalize(
                        p.out
                            .join(format!("{}-completion-g{number:03}.json", layer.layer)),
                    )?
                {
                    bail!("completion evidence locator mismatch");
                }
                empirical_finish(&p, &mut zt, layer, number, token, false)?
            } else if let Some(number) = cards_number(&unit) {
                // The Cards/group unit has no caller-supplied draft: its deterministic
                // verifier re-runs the offline SELF-CHECK over the frozen owned set.
                if evidence["schema_version"] != 1
                    || evidence["kind"] != "cards_completion"
                    || evidence["unit"] != unit
                    || evidence["token"] != token
                {
                    bail!("invalid Cards completion evidence");
                }
                if std::fs::canonicalize(&path)?
                    != std::fs::canonicalize(
                        p.out
                            .join(format!("cards-completion-g{number:03}.json")),
                    )?
                {
                    bail!("completion evidence locator mismatch");
                }
                cards_finish(&p, &mut zt, number, token, false)?
            } else if let Some(key) = unit
                .strip_prefix("models/")
                .and_then(|value| value.strip_suffix("@1"))
            {
                // The Model unit's checked output is its own compiler's step state and part
                // map: caller-supplied addresses cannot establish it.
                if evidence["schema_version"] != 1
                    || evidence["kind"] != "model_completion"
                    || evidence["unit"] != unit
                    || evidence["token"] != token
                {
                    bail!("invalid Model completion evidence");
                }
                if std::fs::canonicalize(&path)?
                    != std::fs::canonicalize(p.out.join(format!("model-completion-{key}.json")))?
                {
                    bail!("completion evidence locator mismatch");
                }
                model_finish(&p, &mut zt, &unit, token, false)?
            } else if let Some(key) = unit
                .strip_prefix("explain/")
                .and_then(|value| value.strip_suffix("@1"))
            {
                // The Explain unit's checked output is the checked model's own Derivation parts
                // and chains: caller-supplied addresses cannot establish it.
                if evidence["schema_version"] != 1
                    || evidence["kind"] != "explain_completion"
                    || evidence["unit"] != unit
                    || evidence["token"] != token
                {
                    bail!("invalid Explain completion evidence");
                }
                if std::fs::canonicalize(&path)?
                    != std::fs::canonicalize(p.out.join(format!("explain-completion-{key}.json")))?
                {
                    bail!("completion evidence locator mismatch");
                }
                explain_finish(&p, &mut zt, &unit, token, false)?
            } else {
                let number = unit
                    .strip_prefix("digest/g")
                    .and_then(|v| v.strip_suffix("@1"))
                    .and_then(|v| v.parse::<u64>().ok())
                    .filter(|n| *n > 0);
                if evidence["schema_version"] != 1
                    || evidence["kind"] != "digest_completion"
                    || evidence["unit"] != unit
                    || evidence["token"] != token
                    || number.is_none()
                {
                    bail!(
                        "completion evidence requires the layer's deterministic checked output; addresses alone are insufficient"
                    );
                }
                if std::fs::canonicalize(&path)?
                    != std::fs::canonicalize(
                        p.out
                            .join(format!("completion-g{:03}.json", number.unwrap())),
                    )?
                {
                    bail!("completion evidence locator mismatch");
                }
                group_finish(
                    &p,
                    &mut zt,
                    number.unwrap(),
                    token,
                    field(&evidence["draft"], "path")?,
                    false,
                )?
            }
        } else if command == "fail" {
            let given = a.str("Fault");
            // The value is a file path or, as the workflow modules pass it, the fault document as JSON
            // text; the 64 KiB bound applies to either form.
            let size = match std::fs::metadata(&given) {
                Ok(meta) => meta.len(),
                Err(_) => given.len() as u64,
            };
            if size > 65536 {
                bail!("fault evidence exceeds 64 KiB");
            }
            let fault = crate::canon::read_json_arg(&given)?;
            fail(
                &p,
                &a.str("Unit"),
                a.str("Token").parse()?,
                &a.choice("Class", &["retryable", "operator", "content"])?,
                &fault,
            )?
        } else {
            {
                let mut claimed = next(&p, &a.str("Layer"), a.str("Take").parse()?, &a.str("Owner"), a.int_or("Wait", 0)?, a.int_or("Lease", 0)?)?;
                if a.flag("Brief") {
                    let units = brief_units(claimed["units"].as_array().map(Vec::as_slice).unwrap_or(&[]));
                    claimed["units"] = json!(units);
                }
                claimed
            }
        }
    } else if command == "rerun-list" {
        rerun_list(Path::new(&a.str("Out")))?
    } else if command == "resume" {
        // R278: a resume requires a prepared compatible ledger. A legacy staging folder is
        // adoption work, so the command refuses it instead of inferring any completion.
        let directory = std::fs::canonicalize(a.str("Out"))
            .context("prepared output directory does not exist")?;
        if !directory.join("manifest.json").is_file() {
            bail!(
                "legacy_staging: {} has no prepared manifest; a legacy staging folder needs the read-only adoption before a resume",
                directory.display()
            );
        }
        if !directory.join("ledger.sqlite3").is_file() {
            bail!(
                "legacy_staging: {} has a prepared manifest but no prepared run ledger; the read-only adoption must create and verify one before a resume",
                directory.display()
            );
        }
        let mut zt = crate::zt::start()?;
        let p = Prepared::read_mode(&a.str("Out"), &mut zt, true)?;
        // R328 (§15.2): a resume dispatches the retained run's work again, and that work writes this
        // book's Literature tree. It stops before it resets a unit or claims anything when the tree
        // is locked by the owner; the retained drafts, results and receipts stay exactly as they are.
        if let Some(refusal) = crate::lockcheck::preflight(&mut zt, "ledger", "resume", &field(&p.contract, "citation_key")?)? {
            return crate::lockcheck::stop("ledger resume", &refusal);
        }
        resume_run(&p)?
    } else if command == "report" {
        // R331: the run report. It reads the ledger and verifies the current placeholder state
        // through the prepared run's own client, writes its two files and prints the facts.
        let mut zt = crate::zt::start()?;
        let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
        let verified = report::placeholders(&mut zt, field(&p.contract, "citation_key")?);
        let mut facts = run_report(&p, verified)?;
        facts["report"] = report::publish(&p.out, &facts)?;
        util::print_line(&serde_json::to_string(&facts)?)?;
        return Ok(0);
    } else {
        let verified = if command == "status" && a.flag("Verify") {
            let mut zt = crate::zt::start()?;
            let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
            Some(report::placeholders(&mut zt, field(&p.contract, "citation_key")?))
        } else {
            None
        };
        if command == "gate" {
            // Settlement is a recorded fact, not a guess: close the dynamic layers whose
            // producers have all settled before reporting the gate (R167, R200).
            let mut zt = crate::zt::start()?;
            let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
            let mut conn = writer(&p)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            verify_binding(&tx, &p)?;
            close_enumerations(&tx)?;
            tx.commit()?;
        }
        let full = status(Path::new(&a.str("Out")), &a.str("Layer"), verified)?;
        if command == "gate" && a.flag("Brief") {
            gate_brief(&full, &a.str("Layer"))
        } else if command == "status" && a.has("Units") {
            let ids: Vec<String> = a.str("Units").split(',').map(str::trim).filter(|id| !id.is_empty()).map(str::to_string).collect();
            unit_view(full["units"].as_array().map(Vec::as_slice).unwrap_or(&[]), &ids)
        } else if command == "status" && a.has("Held") {
            held_view(full["units"].as_array().map(Vec::as_slice).unwrap_or(&[]), &a.str("Held"))
        } else if command == "status" && a.flag("Summary") {
            status_summary(&full)
        } else {
            full
        }
    };
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(if command == "gate" && result["settled"] != true {
        3
    } else {
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // A card split appends the new Card to its group's owned manifest (`add_owned`): a manifest whose leading
    // lines still hash to the frozen value is the frozen enumeration plus split Cards; any other change is not
    // (grossman2002integration trial 19: the whole-book cycle split a Card of group 8 and link-start refused
    // `owned-g08.txt` with a hash mismatch).
    #[test]
    fn an_owned_manifest_extended_by_split_cards_keeps_its_frozen_prefix() {
        let frozen = "B/8\nB/8|a\n";
        let sha = canon::sha256_hex(frozen.as_bytes());
        assert!(frozen_prefix_holds("B/8\nB/8|a\nB/8|b\n", &sha));
        assert!(frozen_prefix_holds(frozen, &sha));
        assert!(!frozen_prefix_holds("B/8\nB/8|c\nB/8|b\n", &sha), "a changed line is not an extension");
        assert!(!frozen_prefix_holds("B/8\n", &sha), "a shortened manifest is not an extension");
    }

    // #92: a claim that starts a provider run asks for that run's cap, and the base lease never
    // shortens the request.
    #[test]
    fn a_claim_lease_covers_a_requested_provider_run() {
        assert_eq!(claim_lease(120, 0), 120);
        assert_eq!(claim_lease(120, 1500), 1500);
        assert_eq!(claim_lease(2, 60), 60);
        assert_eq!(LEASE_MAX, 7200);
    }

    // A claim reply is relayed whole by one manager, so it carries only the claimed units and the
    // layer's scheduling state. The full status (the dependency graph of a 171-group book) made the
    // reply tens of kilobytes, and a relay that could not return it lost the claimed unit.
    #[test]
    fn a_claim_reply_carries_the_units_and_the_scheduling_state_only() {
        let status = json!({
            "schema_version":1,"run_id":"r1","invocation":2,"citation_key":"K","variant":"econ","layer":"digest",
            "sealed":true,"settled":false,"complete":false,"counts":{"open":3},
            "dependencies":[{"unit":"audit/book@1"}],"unresolved":["digest/g001@1"],"units":[{"id":"x"}],
            "archive_identity":{"id":"a"},"publications":[],"coverage_faults":[],
        });
        let reply = claim_reply(&status, &[json!({"id":"digest/g001@1","token":1})]);
        assert_eq!(reply["units"], json!([{"id":"digest/g001@1","token":1}]));
        for key in ["schema_version", "run_id", "invocation", "citation_key", "variant", "layer", "sealed", "settled", "complete", "counts"] {
            assert_eq!(reply[key], status[key], "{key}");
        }
        for key in ["dependencies", "unresolved", "archive_identity", "publications", "coverage_faults"] {
            assert!(reply.get(key).is_none(), "{key} is not part of a claim reply");
        }
    }

    // A group's completion registers its child units and makes the book units wait on them (#69/#70:
    // cards-book on cards/gNNN, link-check on link/gNNN). The preparation registration check accepts exactly
    // those child prerequisites on top of the frozen ones; any other change is still a mismatch.    #[test]
    fn the_registration_check_accepts_the_child_prerequisites_of_book_units() {
        use std::collections::BTreeSet;
        let pair = |a: &str, b: &str| (a.to_string(), b.to_string());
        let needs: BTreeSet<_> = [pair("digest/g001@1", "settled")].into();
        let children: BTreeSet<_> = [pair("cards/g001@1", "cards"), pair("link/g001@1", "link")].into();
        let with_cards: BTreeSet<_> = [pair("digest/g001@1", "settled"), pair("cards/g001@1", "settled")].into();
        assert!(registration_deps_ok("cards-book/book@1", &needs, &with_cards, &children));
        assert!(registration_deps_ok("cards-book/book@1", &needs, &needs, &children));
        let with_link: BTreeSet<_> = [pair("digest/g001@1", "settled"), pair("link/g001@1", "settled")].into();
        assert!(!registration_deps_ok("cards-book/book@1", &needs, &with_link, &children), "a Link child is not a cards-book prerequisite");
        assert!(registration_deps_ok("link-check/book@1", &needs, &with_link, &children));
        let missing: BTreeSet<_> = [pair("cards/g001@1", "settled")].into();
        assert!(!registration_deps_ok("cards-book/book@1", &needs, &missing, &children), "a frozen prerequisite may not disappear");
        let foreign: BTreeSet<_> = [pair("digest/g001@1", "settled"), pair("audit/book@1", "settled")].into();
        assert!(!registration_deps_ok("organize/book@1", &needs, &foreign, &children));
    }

    // A brief gate lists the coverage faults of its own layer; `complete` keeps its run-wide meaning (the
    // layer's own completion is `settled` with nothing open, failed or blocked).
    #[test]
    fn a_owner_lock_is_a_content_fault_the_ledger_accepts() {
        // #140: the workflow records the owner's refusal against the unit it stopped (`ledger fail -Class
        // content -Fault {code: card_locked}`). An unknown code is refused, so the refusal has to be a known
        // content fault: it is a destination decision, never a retryable provider fault.
        assert_eq!(fault_class("card_locked").unwrap(), "content");
        for code in ["provider_transient", "lock_busy", "check_failed", "invalid_plan"] {
            assert!(fault_class(code).is_ok(), "{code} is a known code");
        }
        assert!(fault_class("not_a_code").is_err(), "an unknown code is refused");
    }

    #[test]
    fn a_brief_gate_lists_only_its_own_layers_coverage_faults() {
        let faults = vec![
            json!({"code":"required_unit_missing_or_changed","unit":"cards-book/book@1"}),
            json!({"code":"prepared_group_missing_or_changed","unit":"digest/g002@1"}),
        ];
        assert!(layer_faults(&faults, "events").is_empty());
        assert_eq!(layer_faults(&faults, "digest").len(), 1);
        assert_eq!(layer_faults(&faults, "").len(), 2);
    }

    // `ledger gate -Brief`: the gate line a layer driver relays (the full status was about 49 KB on an
    // eight-group paper, and a relay that summarized it left the driver with no gate at all).
    #[test]
    fn a_brief_gate_keeps_the_layer_decision_only() {
        let full = json!({
            "schema_version":1,"run_id":"r","invocation":1,"layer":"events","sealed":true,"settled":true,"complete":true,
            "counts":{"open":0,"done":8,"failed":0,"per_layer":{"digest":{"total":8}}},
            "unresolved":["events/g001@1","cards-book/book@1"],
            "coverage_faults":[{"code":"required_unit_missing_or_changed","unit":"cards-book/book@1"}],
            "units":[{"id":"x"}],"dependencies":[{"unit":"y"}],
        });
        let brief = gate_brief(&full, "events");
        for key in ["schema_version", "run_id", "invocation", "layer", "sealed", "settled"] {
            assert_eq!(brief[key], full[key], "{key}");
        }
        assert_eq!(brief["counts"], json!({"open":0,"done":8,"failed":0}));
        assert_eq!(brief["unresolved"], json!(["events/g001@1"]));
        assert_eq!(brief["coverage_faults"], json!([]));
        assert!(brief.get("units").is_none() && brief.get("dependencies").is_none());
        assert_eq!(brief["complete"], json!(false), "a layer with an unresolved unit of its own is not complete");
    }

    // A layer gate decides `complete` from the layer's own facts; the run-wide decision also waits on later
    // layers (grossman2002integration trial 4: events 8/8 done, settled, reported active).
    #[test]
    fn a_brief_gate_is_complete_when_its_own_layer_is() {
        let full = json!({
            "schema_version":1,"layer":"events","sealed":true,"settled":true,"complete":false,
            "counts":{"declared":8,"done":8,"noop":0,"refused":0,"failed":0,"blocked":0,"registered":0,"ready":0,"claimed":0,"retry":0,"open":0,"total":8},
            "unresolved":["cards-book/book@1"],
            "coverage_faults":[{"code":"required_unit_missing_or_changed","unit":"cards-book/book@1"}],
        });
        assert_eq!(gate_brief(&full, "events")["complete"], json!(true));
        // explain on trial 7: no unit at all, sealed and settled, is complete; an unsealed empty queue is not.
        let empty = json!({"layer":"explain","sealed":true,"settled":true,"complete":false,"counts":{"open":0,"failed":0,"total":0},"unresolved":[],"coverage_faults":[]});
        assert_eq!(gate_brief(&empty, "explain")["complete"], json!(true));
        let mut unsealed = empty.clone();
        unsealed["sealed"] = json!(false);
        assert_eq!(gate_brief(&unsealed, "explain")["complete"], json!(false));
        assert_eq!(gate_brief(&full, "")["complete"], json!(false), "the run-wide gate keeps the run-wide decision");
        let mut failed = full.clone();
        failed["counts"]["failed"] = json!(1);
        assert_eq!(gate_brief(&failed, "events")["complete"], json!(false));
        let mut unsettled = full.clone();
        unsettled["settled"] = json!(false);
        assert_eq!(gate_brief(&unsettled, "events")["complete"], json!(false));
        let mut faulted = full.clone();
        faulted["coverage_faults"] = json!([{"code":"prepared_group_missing_or_changed","unit":"events/g002@1"}]);
        assert_eq!(gate_brief(&faulted, "events")["complete"], json!(false));
    }

    // `ledger next -Brief`: a block manager relays the claim line, so each claimed unit keeps only what the
    // commands it runs need (id, token, input hash, lease); the frozen inputs stay in the ledger.
    #[test]
    fn a_brief_claim_drops_the_unit_inputs() {
        let claimed = vec![json!({"id":"digest/g001@1","owner":"lane","token":1,"lease_until":9,"lease_seconds":5,"input_hash":"ih","inputs":{"sections":[1,2,3]}})];
        let brief = brief_units(&claimed);
        assert_eq!(brief, vec![json!({"id":"digest/g001@1","owner":"lane","token":1,"lease_until":9,"lease_seconds":5,"input_hash":"ih"})]);
        let model = vec![json!({"id":"models/g001-m0000@1","token":1,"input_hash":"hm","inputs":{"fingerprint":"f","header":"Key/1|b","producer":{"group":{"sections":[1]}}}})];
        assert_eq!(brief_units(&model), vec![json!({"id":"models/g001-m0000@1","token":1,"input_hash":"hm","inputs":{"header":"Key/1|b"}})], "a brief claim keeps the Model address only");
    }

    // `ledger status -Units`: the outcome of named units, small enough to relay, so a block driver decides
    // each unit from the ledger instead of a manager's report.
    #[test]
    fn a_unit_view_is_the_state_of_the_named_units_only() {
        let units = vec![
            json!({"id":"digest/g001@1","layer":"digest","state":"done","attempts":1,"token":1,"owner":"lane","fault_class":null,"result":{"disposition":"verified","owned":["a","b"]},"inputs":{"big":1}}),
            json!({"id":"digest/g002@1","layer":"digest","state":"claimed","attempts":1,"token":2,"owner":"lane","input_hash":"h2","fault_class":null,"result":null}),
            json!({"id":"digest/g003@1","layer":"digest","state":"ready"}),
        ];
        let view = unit_view(&units, &["digest/g002@1".to_string(), "digest/g001@1".to_string(), "digest/g009@1".to_string()]);
        assert_eq!(view["units"][0], json!({"id":"digest/g002@1","state":"claimed","token":2,"input_hash":"h2","attempts":1,"fault_class":null,"disposition":null}));
        assert_eq!(view["units"][1], json!({"id":"digest/g001@1","state":"done","token":1,"input_hash":null,"attempts":1,"fault_class":null,"disposition":"verified"}));
        assert_eq!(view["units"][2], json!({"id":"digest/g009@1","state":"unknown"}));
    }

    // model-deps: a reference to a model this batch planned (in its order, with a plan hash) is the
    // compiler's to resolve; a model of another batch, a deferred one, or the model itself is not.
    #[test]
    fn a_reference_inside_the_batch_needs_no_ledger_wait() {
        // The compiler has no plan hash yet at model-deps time; the plan files decide.
        let batch = std::env::temp_dir().join(format!("planned-in-batch-{}", std::process::id()));
        for model in ["K_2_a", "K_3_a", "K_4_a"] {
            std::fs::create_dir_all(batch.join(model)).unwrap();
            std::fs::write(batch.join(model).join("plan.json"), "{}").unwrap();
        }
        let compiler = json!({"order":["K/2|a","K/3|a"],"models":{"K/2|a":{"status":"brief"},"K/3|a":{"status":"brief"},"K/4|a":{"status":"deferred"}}});
        assert!(planned_in_batch(&compiler, &batch, "K/2|a", "K/3|a"));
        assert!(!planned_in_batch(&compiler, &batch, "K/3|a", "K/3|a"));
        assert!(!planned_in_batch(&compiler, &batch, "K/4|a", "K/3|a"));
        assert!(!planned_in_batch(&compiler, &batch, "K/7|a", "K/3|a"));
        std::fs::remove_dir_all(&batch).unwrap();
    }

    // The counts listing keeps numbers only; the one-workspace listing holds that workspace's ids.
    #[test]
    fn the_cleanup_listings_carry_no_id_list_through_a_relay() {
        let brief = json!({"schema_version":1,"action":"list","brief":true,"resources":2,"workspaces":[
            {"id":"W1","path":"D:/w1","recorded":["a","b"],"open":["b"],"bare":[]},
            {"id":"W2","path":"D:/w2","recorded":["c"],"open":["c"],"bare":["c"]}]});
        assert_eq!(resource_counts(&brief)["workspaces"][1], json!({"id":"W2","path":"D:/w2","recorded":1,"open":1,"bare":1}));
        assert_eq!(resource_brief_of(&brief, "D:/w1")["workspace"]["open"], json!(["b"]));
        assert_eq!(resource_brief_of(&brief, "D:/none")["workspace"]["open"], json!([]));
    }

    // relocate: the owned and source manifests follow a recorded move; other lines and files stay as they are.
    #[test]
    fn a_recorded_move_rewrites_the_owned_and_source_manifests() {
        let out = std::env::temp_dir().join(format!("relocate-manifests-{}", std::process::id()));
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("owned-g01.txt"), "K/1|a\r\n374/1|b\r\n374/1|c\r\n").unwrap();
        std::fs::write(out.join("src-g01.txt"), "374/1|b\tsections/s01.md:3-4\nK/1|a\tsections/s01.md:1-2\n").unwrap();
        std::fs::write(out.join("allow-g01.txt"), "374/1|b\n").unwrap();
        let mapping: std::collections::BTreeMap<String, String> = [("374/1|b".to_string(), "374/1|a|a".to_string())].into_iter().collect();
        let rewritten = relocate_manifests(&out, &mapping).unwrap();
        assert_eq!(rewritten, vec!["owned-g01.txt".to_string(), "src-g01.txt".to_string()]);
        assert_eq!(std::fs::read_to_string(out.join("owned-g01.txt")).unwrap(), "K/1|a\r\n374/1|a|a\r\n374/1|c\r\n");
        assert_eq!(std::fs::read_to_string(out.join("src-g01.txt")).unwrap(), "374/1|a|a\tsections/s01.md:3-4\nK/1|a\tsections/s01.md:1-2\n");
        assert_eq!(std::fs::read_to_string(out.join("allow-g01.txt")).unwrap(), "374/1|b\n");
        std::fs::remove_dir_all(&out).unwrap();
    }

    // An agent-facing record command answers its top-level facts only, never the resource views.
    #[test]
    fn a_record_command_answers_one_short_line() {
        let brief = record_brief(&json!({"schema_version":1,"run_id":"r1","action":"observe","op":"workspace","cleanup_unit":"cleanup/run-t1@1","resources":[{"conversations":[1,2,3]}],"view":{"a":1}}));
        assert_eq!(brief, json!({"schema_version":1,"action":"observe","op":"workspace","cleanup_unit":"cleanup/run-t1@1","recorded":true}));
    }

    // cleanup-finish: a resource retains its usage as a reading or as an explicit unavailable record; only a
    // resource with neither is missing its evidence.
    #[test]
    fn an_unavailable_usage_record_is_retained_usage() {
        assert!(usage_retained(&json!({"usage":{"total":{"input_tokens":1}},"conversations":[]})));
        assert!(usage_retained(&json!({"usage":null,"conversations":[{"id":"a","usage":{"total":{"input_tokens":1}}}]})));
        assert!(usage_retained(&json!({"usage":null,"conversations":[{"id":"a","usage":{"unavailable":"the run's result stated no token usage"}}]})));
        assert!(!usage_retained(&json!({"usage":null,"conversations":[{"id":"a","usage":null}]})));
    }

    // `ledger resource -List -Brief`: per workspace, every recorded id, the ones still to delete, and the ones
    // without evidence; a lost creation response is neither open nor bare.
    #[test]
    fn a_brief_resource_list_groups_the_ids_by_workspace() {
        let view = vec![
            json!({"workspace":{"id":"W1","path":"D:/w1"},"conversations":[{"id":"a","lost":false,"deleted":false,"evidence":{"answer":{}}},{"id":"b","lost":false,"deleted":true,"evidence":null}]}),
            json!({"workspace":{"id":"W2","path":"D:/w2"},"conversations":[{"id":"c","lost":true,"deleted":false,"evidence":null}]}),
            json!({"workspace":{"id":"W1","path":"D:/w1"},"conversations":[{"id":"d","lost":false,"deleted":false,"evidence":null}]}),
        ];
        let brief = resource_brief(&view);
        assert_eq!(brief["resources"], json!(3));
        assert_eq!(brief["workspaces"][0], json!({"id":"W1","path":"D:/w1","recorded":["a","b","d"],"open":["a","d"],"bare":["b","d"]}));
        assert_eq!(brief["workspaces"][1], json!({"id":"W2","path":"D:/w2","recorded":["c"],"open":[],"bare":[]}));
    }

    // An owned Card the frozen gap set left out needs no kinded Link of its own; a writer's already_linked does.
    #[test]
    fn only_a_claimed_already_linked_must_show_its_kinded_link() {
        assert!(!already_linked_needs_kind(&json!(NO_GAP_REASON)));
        assert!(already_linked_needs_kind(&json!("本卡已有对照句")));
        assert!(already_linked_needs_kind(&Value::Null));
    }

    // A Link target is the whole address inside [[...]]: ZT addresses carry '|'.
    #[test]
    fn a_paragraph_target_keeps_the_whole_address() {
        assert_eq!(
            paragraph_targets("依赖 [[b/2|a|a|17]] 的均衡条件，对应 [[b/3]] 与 [[ b/3|c ]]。").unwrap(),
            vec!["b/2|a|a|17".to_string(), "b/3".to_string(), "b/3|c".to_string()]
        );
    }

    // `ledger hold -Units`: the claims a long step keeps current, named <unit id>:<token>.
    #[test]
    fn a_hold_names_each_claim_by_unit_and_token() {
        assert_eq!(
            hold_targets(" models/g001-m0000@1:1, models/g002-m0001@1:3 ").unwrap(),
            vec![("models/g001-m0000@1".to_string(), 1), ("models/g002-m0001@1".to_string(), 3)]
        );
        assert!(hold_targets("models/g001-m0000@1").is_err(), "a claim needs its token");
        assert!(hold_targets("models/g001-m0000@1:0").is_err(), "a token is positive");
        assert!(hold_targets(" , ").is_err(), "a hold names at least one claim");
    }

    // `ledger status -Held <owner>`: the units one claim owner holds, with their tokens and inputs, so a
    // driver whose claim line came back summarized takes its units from the ledger.
    #[test]
    fn a_held_view_lists_only_the_owners_current_claims() {
        let units = vec![
            json!({"id":"cards/g005@1","layer":"cards","state":"claimed","token":3,"owner":"cards claim 2","lease_until":9,"input_hash":"h5","inputs":{"group":5}}),
            json!({"id":"cards/g006@1","layer":"cards","state":"claimed","token":4,"owner":"cards claim 1","lease_until":9,"input_hash":"h6","inputs":{"group":6}}),
            json!({"id":"cards/g007@1","layer":"cards","state":"done","token":5,"owner":"cards claim 2","lease_until":null,"input_hash":"h7","inputs":{"group":7}}),
        ];
        let view = held_view(&units, "cards claim 2");
        assert_eq!(view["owner"], json!("cards claim 2"));
        assert_eq!(view["units"], json!([{"id":"cards/g005@1","owner":"cards claim 2","token":3,"lease_until":9,"input_hash":"h5","inputs":null}]));
        let models = vec![json!({"id":"models/g001-m0000@1","state":"claimed","token":1,"owner":"models claim 1","lease_until":9,"input_hash":"hm","inputs":{"header":"Key/1|b","producer":{"big":1}}})];
        assert_eq!(held_view(&models, "models claim 1")["units"][0]["inputs"], json!({"header":"Key/1|b"}));
    }

    // `ledger reclaim -Release` takes a claim before its lease ends only when nothing happened to it
    // for the quiet window: a live holder renews every 30 seconds (a heartbeat) or with every wait
    // round (a manager), so a quiet claim belongs to a stopped run.
    #[test]
    fn a_release_takes_only_a_quiet_claim() {
        assert!(quiet_claim(Some(1_000_000), 1_000_000 + 121_000, 120));
        assert!(!quiet_claim(Some(1_000_000), 1_000_000 + 119_000, 120));
        assert!(quiet_claim(None, 5, 120), "a claim with no recorded activity is quiet");
    }

    // `ledger status -Summary` keeps the run's identity, settings and counts and replaces the units
    // with their number per layer and state, so a workflow can read it through one relayed line
    // (the full status of a 171-group book is about 450 KB).
    #[test]
    fn a_status_summary_counts_the_units_per_layer() {
        let full = json!({
            "schema_version":1,"run_id":"r1","invocation":2,"resume_count":1,"citation_key":"Key",
            "variant":"econ","settings":{"max_attempts":3,"dsh_run_cap":1200},
            "counts":{"declared":4},"settled":false,"complete":false,
            "dependencies":[{"unit":"audit/book@1","needs":"digest/g001@1"}],
            "units":[
                {"id":"digest/g001@1","layer":"digest","state":"ready","progress":{"big":"x"}},
                {"id":"digest/g002@1","layer":"digest","state":"registered"},
                {"id":"terms/book@1","layer":"terms","state":"ready"},
                {"id":"prepare/book@1","layer":"prepare","state":"done"},
            ],
        });
        let s = status_summary(&full);
        assert_eq!(s["citation_key"], json!("Key"));
        assert_eq!(s["variant"], json!("econ"));
        assert_eq!(s["settings"]["dsh_run_cap"], json!(1200));
        assert_eq!(s["invocation"], json!(2));
        assert_eq!(s["counts"]["declared"], json!(4));
        assert_eq!(s["layers"]["digest"], json!({"ready":1,"registered":1,"total":2}));
        assert_eq!(s["layers"]["terms"]["total"], json!(1));
        assert!(s.get("units").is_none() && s.get("dependencies").is_none());
    }

    // A start step (terms plan, group start, link start) refreshes the claim's lease but never
    // cuts the provider-run lease the claim was granted: a 3-minute DeepSeek run under a
    // 1500-second claim lost its claim when the plan reset the lease to 120 seconds.
    #[test]
    fn a_start_step_keeps_the_longer_claim_lease() {
        assert_eq!(start_lease(1000, 120, Some(2500)), 2500);
        assert_eq!(start_lease(1000, 120, Some(1050)), 1120);
        assert_eq!(start_lease(1000, 120, None), 1120);
    }

    #[test]
    fn a_prior_card_cycle_rule_list_still_completes_its_own_report() {
        fn declared(rules: &'static str) -> std::collections::BTreeSet<&'static str> {
            rules.split(',').collect()
        }
        // The current default set, and the set a report written before #93 S02 names.
        assert_eq!(card_cycle_rule_missing(&declared(crate::card::DEFAULT_CARD_RULES)), None);
        assert_eq!(
            card_cycle_rule_missing(&declared("R2,R3,R4,R5,R7,R9,R12,R18,R19,I1,I2,I3,I4,L1,L2,E1,E2")),
            None
        );
        // A report that left out a rule of both sets names the missing rule.
        assert_eq!(card_cycle_rule_missing(&declared("R2,R3,R4")), Some("R5"));
        assert!(crate::card::DEFAULT_CARD_RULES.split(',').any(|rule| rule == "I7"));
    }

    #[test]
    fn a_done_bridge_publication_names_its_report_and_counts_its_proposals() {
        // R339: an open proposal is a review decision with the immutable report id beside it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bridge-report.json");
        std::fs::write(
            &path,
            json!({"schema":1,"report_id":"r-abc","proposals":[{"id":"b001"},{"id":"b002"}]}).to_string(),
        )
        .unwrap();
        let units = vec![json!({
            "id":"bridge/book@1","layer":"bridge","revision":1,"state":"done",
            "result":{
                "artifacts":{"report":{"path":path.to_string_lossy()}},
                "coverage":[
                    {"ref":"b001","outcome":"open"},
                    {"ref":"b002","outcome":"accepted"}
                ]
            }
        })];
        let facts = bridge_facts(&units).unwrap();
        assert_eq!(facts["report_id"], json!("r-abc"));
        assert_eq!(facts["open"], json!(1));
        assert_eq!(facts["accepted"], json!(1));
        assert_eq!(facts["refused"], json!(0));
        assert_eq!(facts["state"], json!("awaiting_optional_review"));
        assert_eq!(bridge_facts(&[]).unwrap(), Value::Null);
    }

    #[test]
    fn counts_separate_declared_work_from_a_deliberate_no_op_and_a_refusal() {
        // R333: declared work is one obligation per logical key; a superseded revision is history.
        let units = vec![
            json!({"id":"digest/g001@1","layer":"digest","logical_key":"g001","revision":1,"state":"done",
                   "fault":Value::Null,"progress":{},"result":{"disposition":"no_cards"}}),
            json!({"id":"digest/g002@1","layer":"digest","logical_key":"g002","revision":1,"state":"failed",
                   "fault":{"code":"check_failed"},"progress":{},"result":Value::Null}),
            json!({"id":"digest/g002@2","layer":"digest","logical_key":"g002","revision":2,"state":"failed",
                   "fault":{"code":"superseded"},"progress":{},"result":Value::Null}),
            json!({"id":"notes/n1@1","layer":"notes","logical_key":"n1","revision":1,"state":"done",
                   "fault":Value::Null,"progress":{},"result":{"coverage":[{"ref":"s1","outcome":"refused"},{"ref":"s2","outcome":"applied"}]}}),
        ];
        let (total, per_layer) = counts_of(&units);
        assert_eq!(per_layer["digest"]["declared"], json!(2));
        assert_eq!(per_layer["digest"]["done"], json!(1));
        assert_eq!(per_layer["digest"]["noop"], json!(1));
        assert_eq!(per_layer["digest"]["failed"], json!(1));
        assert_eq!(per_layer["digest"]["superseded"], json!(1));
        assert_eq!(per_layer["notes"]["refused"], json!(1));
        assert_eq!(total["declared"], json!(3));
        assert_eq!(total["noop"], json!(1));
        assert_eq!(total["refused"], json!(1));
        assert_eq!(total["open"], json!(0));
    }

    #[test]
    fn a_pending_external_run_and_a_placeholder_are_named_gaps() {
        // R335/R336: the report names the exact unit, job or address that is still owed.
        let units = vec![json!({
            "id":"models/m1@1","layer":"models","logical_key":"m1","revision":1,"state":"claimed",
            "fault":Value::Null,"fault_class":Value::Null,
            "progress":{"jobs":[{"id":"dsh-run-2","kind":"dsh","state":"running","started_at":1,"cap_seconds":60}]},
            "result":Value::Null
        })];
        let placeholders = json!({"status":"verified","count":1,"models":[{"model":"K/1|a","placeholders":["K/1|a"]}]});
        let gaps = gaps_of(&units, &[json!({"code":"prepared_group_missing_or_changed","unit":"digest/g005@1"})], &placeholders);
        assert_eq!(gaps[0]["kind"], json!("model_build"));
        assert_eq!(gaps[0]["unit"], json!("models/m1@1"));
        assert_eq!(gaps[1]["kind"], json!("pending_job"));
        assert_eq!(gaps[1]["address"], json!("dsh-run-2"));
        assert_eq!(gaps[2]["kind"], json!("coverage"));
        assert_eq!(gaps[2]["unit"], json!("digest/g005@1"));
        assert_eq!(gaps[3]["kind"], json!("placeholder"));
        assert_eq!(gaps[3]["address"], json!("K/1|a"));
        // An unknown placeholder read yields no placeholder gap and no false count.
        let unknown = gaps_of(&units, &[], &json!({"status":"unknown","count":Value::Null,"models":[]}));
        assert_eq!(unknown.len(), 2);
    }

    #[test]
    fn the_resume_invocation_is_offered_once_and_a_manual_list_afterwards() {
        // R340: after the one resume, the report lists the decisions instead of suggesting another.
        let preparation = json!({"variant":"econ"});
        let out = Path::new("D:\\zt-digest\\book");
        let unresolved = vec![json!("digest/g001@1")];
        let gaps = vec![json!({"kind":"group_gap","unit":"digest/g001@1","reason":"the group draft held an unsourced claim"})];
        let before = resume_facts(&preparation, out, &unresolved, &gaps, 0);
        assert_eq!(before["available"], json!(true));
        let text = before["invocation"].as_str().unwrap();
        assert!(text.contains("zt-econ-digest.js"));
        assert!(text.contains("mode: \"resume\""));
        assert!(text.contains(r"D:\\zt-digest\\book"));
        assert!(!text.contains(r"\\?\"));
        let after = resume_facts(&preparation, out, &unresolved, &gaps, 1);
        assert_eq!(after["available"], json!(false));
        assert!(after["reason"].as_str().unwrap().contains("already been used"));
        assert_eq!(after["manual"][0], json!("group_gap digest/g001@1: the group draft held an unsourced claim"));
        let done = resume_facts(&preparation, out, &[], &[], 0);
        assert_eq!(done["available"], json!(false));
        assert_eq!(done["reason"], json!("no unresolved work remains"));
    }

    #[test]
    fn an_empirical_unit_names_its_layer_and_group() {
        let (layer, number) = empirical_unit_of("events/g007@1").expect("an Events unit");
        assert_eq!(layer.layer, "events");
        assert_eq!(number, 7);
        let (layer, number) = empirical_unit_of("findings/g012@1").expect("a Findings unit");
        assert_eq!(layer.layer, "findings");
        assert_eq!(number, 12);
        assert!(empirical_unit_of("events/g000@1").is_none());
        assert!(empirical_unit_of("cards/g001@1").is_none());
        assert!(empirical_unit_of("events/g001@2").is_none());
        assert_eq!(empirical_candidate_id(3, 0), "g003/c001");
        assert_eq!(empirical_group_key(12), "g012");
    }

    #[test]
    fn a_stated_label_reads_its_value_or_the_list_under_it() {
        let body = "测量：比例。\n数据：48 家企业。\n方法：访谈。\n结果：\n  - 31 家推迟投资。\n局限：自报原因。\n来源：[[K/1]] §1";
        assert_eq!(stated_label(body, "测量").as_deref(), Some("比例。"));
        assert_eq!(stated_label(body, "结果").as_deref(), Some("31 家推迟投资。"));
        assert_eq!(stated_label(body, "局限").as_deref(), Some("自报原因。"));
        assert_eq!(stated_label(body, "来源").as_deref(), Some("[[K/1]] §1"));
        assert!(stated_label(body, "样本").is_none());
        // A label left empty with nothing listed under it states no value.
        let empty = "测量：比例。\n局限：\n来源：[[K/1]] §1";
        assert!(stated_label(empty, "局限").is_none());
        // The bold and list spellings the checker accepts read the same value.
        assert_eq!(stated_label("- **局限**：自报原因。", "局限").as_deref(), Some("自报原因。"));
        assert_eq!(stated_label("结果:\n- 31 家推迟投资。", "结果").as_deref(), Some("31 家推迟投资。"));
    }

    #[test]
    fn a_resource_label_carries_the_run_unit_and_token() {
        // R322: the label is derived from the three identifiers, never from free-form agent text.
        assert_eq!(resource_label("r1", "digest/g001@1", 3), "zet-r1-digest-g001-1-t3");
        assert_eq!(cleanup_key("digest/g001@1", 3), "digest-g001-1-t3");
        assert_eq!(cleanup_unit_id("digest/g001@1", 3), "cleanup/digest-g001-1-t3@1");
        assert!(protected_name("plan.json"));
        assert!(protected_name("PART-MAP.json"));
        assert!(protected_name("explain-completion-x.json"));
        assert!(protected_name("bridge-report.json"));
        assert!(!protected_name("notes.md"));
    }

    /// A ledger with one content obligation: the content half is reported without any cleanup unit.
    fn ledger_with_content(state: &str, fault: Option<&str>) -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = connect(&dir.path().join("ledger.sqlite3"), true).unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        for (key, value) in [
            ("target", json!({"enabled":[],"excluded":[]})),
            ("preparation", json!({"variant":"econ"})),
            ("preparation_hash", json!("abc")),
        ] {
            conn.execute(
                "INSERT INTO meta(key,value) VALUES(?1,?2)",
                params![key, canon::canonical_json(&value)],
            )
            .unwrap();
        }
        let inputs = canon::canonical_json(&json!({"preparation_hash":"abc"}));
        conn.execute("INSERT INTO units(id,layer,logical_key,revision,state,inputs_json,input_hash,created_at) VALUES('prepare/book@1','prepare','book',1,?1,?2,?3,'now')",params![state,inputs,canon::hash_value(&json!({"preparation_hash":"abc"}))]).unwrap();
        if let Some(fault) = fault {
            conn.execute(
                "UPDATE units SET fault_json=?1 WHERE id='prepare/book@1'",
                params![canon::canonical_json(&json!({"code":fault}))],
            )
            .unwrap();
        }
        (dir, conn)
    }

    #[test]
    fn content_completion_is_reported_apart_from_cleanup() {
        // The content half is complete: every unit is done and every required obligation is current.
        let (_dir, conn) = ledger_with_content("done", None);
        let tx = conn.unchecked_transaction().unwrap();
        let facts = content_facts(&tx).unwrap();
        assert_eq!(facts["complete"], json!(true));
        assert_eq!(facts["unresolved"], json!([]));
        drop(tx);
        // One open unit makes the content incomplete: the cleanup half is not part of that answer.
        conn.execute("UPDATE units SET state='registered' WHERE id='prepare/book@1'", [])
            .unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        let facts = content_facts(&tx).unwrap();
        assert_eq!(facts["complete"], json!(false));
        assert_eq!(facts["unresolved"], json!(["prepare/book@1"]));
        drop(tx);
        // A superseded revision is terminal history, not an unfinished content obligation.
        let (_dir2, superseded) = ledger_with_content("failed", Some("superseded"));
        let tx = superseded.unchecked_transaction().unwrap();
        let facts = content_facts(&tx).unwrap();
        assert_eq!(facts["complete"], json!(true));
        assert_eq!(facts["unresolved"], json!([]));
    }

    #[test]
    fn the_fixed_labels_of_each_layer() {
        assert_eq!(empirical_labels(&EVENTS), ["时间", "地点", "参与者", "经过"]);
        assert_eq!(empirical_labels(&FINDINGS), ["测量", "数据", "方法", "结果", "局限"]);
        assert_eq!(EVENTS.rules, "E1,E2,E5");
        assert_eq!(FINDINGS.rules, "F1,F2,F3,F4");
    }

    #[test]
    fn an_explanation_layer_separates_inapplicable_from_unfinished_work() {
        let part = |address: &str| json!({"address": address, "missing": ["思路"]});
        let own = vec!["K/1|a|a|1".to_string(), "K/1|a|a|2".to_string()];
        // A concept model records the layer as inapplicable, never as unexplained work.
        let concept = explanation_facts("concept", &own, &[], Vec::new(), Vec::new());
        assert_eq!(concept["applicable"], json!(false));
        assert_eq!(concept["reason"], json!("concept"));
        assert_eq!(concept["status"], json!("none"));
        // A formal model without Derivation parts is applicable and still a no-op.
        let plain = explanation_facts("formal", &[], &[], Vec::new(), Vec::new());
        assert_eq!(plain["applicable"], json!(true));
        assert_eq!(plain["reason"], json!("no_derivations"));
        assert_eq!(plain["derivations"], json!(0));
        // A formal model with unexplained parts carries the exact need and its count.
        let unfinished = explanation_facts("formal", &own, &[], vec![part("K/1|a|a|1"), part("K/1|a|a|2")], Vec::new());
        assert_eq!(unfinished["status"], json!("required"));
        assert_eq!(unfinished["reason"], json!("unexplained"));
        assert_eq!(unfinished["derivations"], json!(2));
        assert_eq!(unfinished["explained"], json!(0));
        assert_eq!(unfinished["needs"].as_array().map(Vec::len), Some(2));
        // Every in-scope part explained is the completed layer, not a second kind of work.
        let explained = explanation_facts(
            "formal",
            &own,
            &["K/1|a|a|1".to_string(), "K/1|a|a|2".to_string()],
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(explained["status"], json!("none"));
        assert_eq!(explained["reason"], json!("explained"));
        assert_eq!(explained["explained"], json!(2));
    }

    #[test]
    fn the_resume_closure_names_prerequisites_and_downstream_work_only() {
        // R283 (#87): a resume states exactly what it will drain: the unresolved prerequisites of the
        // failed roots and every unfinished unit that waits on them, never a done build.
        let view = json!({
            "units": [
                {"id": "prepare/book@1", "state": "done"},
                {"id": "digest/g001@1", "state": "done"},
                {"id": "cards/g001@1", "state": "failed"},
                {"id": "link/g001@1", "state": "blocked"},
                {"id": "digest/g002@1", "state": "claimed"},
                {"id": "cards/g002@1", "state": "registered"},
                {"id": "audit/book@1", "state": "done"},
                {"id": "audit/book@0", "state": "failed", "fault": {"code": "superseded", "replacement": "audit/book@1"}},
            ],
            "dependencies": [
                {"unit": "digest/g001@1", "needs": "prepare/book@1", "state": "done"},
                {"unit": "cards/g001@1", "needs": "digest/g001@1", "state": "done"},
                {"unit": "link/g001@1", "needs": "cards/g001@1", "state": "failed"},
                {"unit": "cards/g002@1", "needs": "digest/g002@1", "state": "claimed"},
            ],
        });
        let prerequisites = closure_of(&view, false);
        // Every unresolved root is a prerequisite, including the blocked dependant whose failure must
        // be explained; a done unit above it is never part of the resume.
        assert!(prerequisites.contains(&"digest/g002@1".to_string()));
        assert!(prerequisites.contains(&"cards/g001@1".to_string()));
        assert!(prerequisites.contains(&"link/g001@1".to_string()));
        assert!(!prerequisites.contains(&"prepare/book@1".to_string()));
        assert!(!prerequisites.contains(&"digest/g001@1".to_string()));
        let downstream = closure_of(&view, true);
        assert!(downstream.contains(&"cards/g002@1".to_string()));
        assert!(downstream.contains(&"cards/g001@1".to_string()));
        assert!(!downstream.contains(&"digest/g001@1".to_string()));
        assert!(!downstream.contains(&"audit/book@1".to_string()));
        // A superseded revision is terminal history: it is never offered to a resume.
        assert!(!downstream.contains(&"audit/book@0".to_string()));
        assert!(!prerequisites.contains(&"audit/book@0".to_string()));
    }

    #[test]
    fn resume_requires_an_explicit_prepared_output_directory() {
        // A resume never infers its staging folder, and a missing or empty -Out is an invalid
        // invocation (exit 2), not an operational failure of a run.
        assert_eq!(cli(&["resume".to_string()]).unwrap(), 2);
        assert_eq!(
            cli(&["resume".to_string(), "-Out".to_string(), "  ".to_string()]).unwrap(),
            2
        );
        // The command surface keeps -Layer out of a resume: the run's own target comes from the ledger.
        assert_eq!(
            cli(&[
                "resume".to_string(),
                "-Out".to_string(),
                ".".to_string(),
                "-Layer".to_string(),
                "digest".to_string(),
            ])
            .unwrap(),
            2
        );
    }
}





