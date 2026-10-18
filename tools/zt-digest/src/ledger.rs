//! One prepared book's work and evidence. No Card prose or second write authority.
use crate::{args::Args, canon, util, zt::Zt};
use anyhow::{Context, Result, bail};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
};
use serde_json::{Value, json};
use std::{
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

/// The producer layers whose recorded work forms one aggregate publication input.
/// Every publication also reads the Model candidates its groups registered, so a
/// repaired or newly built model changes the input set of every downstream
/// revision. The whole-book Card and Link cycles additionally enumerate their own
/// child layer, and Organize also reads the registered note suggestions it must
/// dispose. A producer that later emits nothing still changes the manifest.
fn publication_sources(layer: &str) -> Option<&'static [&'static str]> {
    match layer {
        "cards-book" => Some(&["digest", "models", "cards"]),
        "link-check" => Some(&["digest", "models", "link"]),
        "organize" => Some(&["digest", "models", "notes"]),
        // A Theory Card's summary depends on the units (their Models) and on the
        // cited-premise Link pass, which the prerequisite revision names.
        "theory" => Some(&["digest", "models"]),
        "skeleton" | "figures" | "bridge" | "audit" | "hubs" => Some(&["digest", "models"]),
        _ => None,
    }
}

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
            options.insert(name.clone(), json!(enabled));
        }
    }
    Ok(Value::Object(options))
}

/// Date extraction is not an Event-layer switch. Disabled work stays explicit.
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

struct Prepared {
    out: PathBuf,
    contract: Value,
    hash: String,
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
    fn read(out: &str, zt: &mut Zt) -> Result<Self> {
        Self::read_mode(out, zt, true)
    }

    fn read_mode(out: &str, zt: &mut Zt, require_cards: bool) -> Result<Self> {
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
        let cards = zt.cards(&addresses, "text")?;
        if cards.archive_revision != observed["archive_revision"].as_u64().unwrap() {
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
        if after["archive_identity"] != contract["archive_identity"]
            || after["archive_revision"].as_u64() != Some(cards.archive_revision)
        {
            bail!("archive changed while checking preparation; retry without publishing work");
        }
        let hash = canon::hash_value(&contract);
        Ok(Self {
            out,
            contract,
            hash,
        })
    }
}

fn connect(path: &Path, writable: bool) -> Result<Connection> {
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

fn meta(conn: &Connection, key: &str) -> Result<Value> {
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
            .map(|link| link.split('|').next().unwrap_or_default().trim().to_string())
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
    let audit = required_publication(&connect(&p.out.join("ledger.sqlite3"), false)?, "audit")?
        .context("the final audit revision was never published")?;
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
    Ok(json!({
        "schema_version":1,
        "run_id":run,
        "reused":applied == 0,
        "applied":applied,
        "moves":mapping,
        "units":units.into_iter().collect::<Vec<_>>()
    }))
}

/// The compiler artifacts that carry one publication layer's checked result. The
/// workflow names the real files; the ledger binds their canonical path and hash.
fn publication_artifacts(layer: &str) -> Option<&'static [&'static str]> {
    Some(match layer {
        "organize" => &["decisions", "applied"],
        "skeleton" => &["skeleton"],
        "theory" => &["applied"],
        "figures" | "bridge" | "hubs" | "audit" | "cards-book" | "link-check" => &["report"],
        _ => return None,
    })
}

/// A publication unit id as its layer, logical key and revision.
fn publication_unit(unit: &str) -> Result<(String, String, i64)> {
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

fn unit_inputs(p: &Prepared, unit: &str) -> Result<Value> {
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
fn registered_notes(p: &Prepared) -> Result<std::collections::BTreeMap<String, Value>> {
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
                if contributing {
                    bail!("publication coverage cannot report `{reference}` as {outcome}: its producer completed");
                }
            }
            _ => bail!("unknown {layer} coverage outcome `{outcome}`"),
        }
        checked.push(json!({"ref":reference,"outcome":outcome,"reason":reason,"producer":producer}));
    }
    Ok(json!(checked))
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


fn publication_finish(p: &Prepared, zt: &mut Zt, unit: &str, token: i64, path: &Path) -> Result<Value> {
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
    let scope = if layer == "hubs" {
        hub_scope(key, declared)?
    } else {
        if !declared.trim().is_empty() {
            bail!("only a `hubs` publication declares -Scope");
        }
        Value::Null
    };
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let manifest = publication_manifest(&tx, layer, key, &scope)?;
    let input_hash = canon::hash_value(&manifest);
    let existing: Option<(String, i64)> = tx
        .query_row(
            "SELECT id,revision FROM units WHERE layer=?1 AND logical_key=?2 AND input_hash=?3 ORDER BY revision LIMIT 1",
            params![layer, key, input_hash],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (id, revision, reused) = match existing {
        Some((id, revision)) => (id, revision, true),
        None => {
            let latest: i64 = tx
                .query_row(
                    "SELECT coalesce(max(revision),0) FROM units WHERE layer=?1 AND logical_key=?2",
                    params![layer, key],
                    |r| r.get(0),
                )?;
            let revision = latest + 1;
            let id = register_publication_unit(&tx, layer, key, revision, &manifest, None)?;
            seal(&tx, layer)?;
            // Every older revision that is still open is terminal: it can never be
            // claimed as current work, and the report names its replacement.
            let superseded = {
                let mut statement = tx.prepare("SELECT id FROM units WHERE layer=?1 AND logical_key=?2 AND revision<?3 AND state<>'done' ORDER BY revision")?;
                statement
                    .query_map(params![layer, key, revision], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            };
            for unit in &superseded {
                tx.execute("UPDATE units SET state='failed',owner=NULL,lease_until=NULL,retry_after=NULL,fault_class='operator',fault_json=?2 WHERE id=?1",params![unit,canon::canonical_json(&json!({"code":"superseded","by":id,"cause":"a changed publication input manifest registers a new revision"}))])?;
            }
            event(
                &tx,
                &id,
                None,
                None,
                "published",
                &json!({"layer":layer,"logical_key":key,"revision":revision,"input_hash":input_hash,"superseded":superseded,"contributions":manifest["contributions"].as_array().unwrap().len(),"dispositions":manifest["dispositions"].as_array().unwrap().len()}),
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
    if let Some(terms) = terms.filter(|_| prepared) {
        tx.execute("UPDATE units SET state='ready' WHERE id=?1", [terms])?;
    }
    if prepared {
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
fn required_publication(conn: &Connection, layer: &str) -> Result<Option<(String, String)>> {
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

fn status(out: &Path, layer: &str) -> Result<Value> {
    let conn = connect(&out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    verify_schema(&tx)?;
    if meta(&tx, "preparation_hash")? != canon::hash_value(&meta(&tx, "preparation")?) {
        bail!("ledger preparation hash mismatch");
    }
    let mut statement=tx.prepare("SELECT id,layer,logical_key,revision,state,attempts,owner,claim_token,input_hash,inputs_json,fault_json,progress_json,result_json,lease_until,retry_after FROM units WHERE (?1='' OR layer=?1) ORDER BY id")?;
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
                "retry_after":r.get::<_,Option<i64>>(14)?
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
    let mut required = vec!["prepare"];
    if layers.iter().any(|l| l == "terms") {
        required.push("terms");
    }
    required.extend(
        BOOK.iter()
            .copied()
            .filter(|layer| layers.iter().any(|l| l == *layer)),
    );
    if preparation["variant"] == "math" {
        required.extend(
            MATH_BOOK
                .iter()
                .copied()
                .filter(|layer| layers.iter().any(|l| l == *layer)),
        );
    }
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
    let mut counts = json!({"total":units.len(),"done":0,"failed":0,"blocked":0,"open":0,"superseded":0});
    let mut unresolved = Vec::new();
    for unit in &units {
        let state = unit["state"].as_str().unwrap();
        // A superseded revision is terminal history: it is neither a failure of
        // this run nor outstanding work, and it is never offered to a resume.
        let bucket = if is_superseded(unit) {
            "superseded"
        } else if matches!(state, "done" | "failed" | "blocked") {
            state
        } else {
            "open"
        };
        counts[bucket] = json!(counts[bucket].as_u64().unwrap() + 1);
        if state != "done" && bucket != "superseded" {
            unresolved.push(unit["id"].clone());
        }
    }
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
    let complete = settled
        && !excluded
        && counts["failed"] == 0
        && counts["blocked"] == 0
        && coverage_faults.is_empty();
    let mut deps_query = tx.prepare("SELECT d.unit_id,d.needs_id,d.condition,u.state FROM deps d JOIN units u ON u.id=d.needs_id JOIN units child ON child.id=d.unit_id WHERE (?1='' OR child.layer=?1) ORDER BY d.unit_id,d.needs_id")?;
    let dependencies=deps_query.query_map([layer],|r|Ok(json!({"unit":r.get::<_,String>(0)?,"needs":r.get::<_,String>(1)?,"condition":r.get::<_,String>(2)?,"state":r.get::<_,String>(3)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let result = json!({"schema_version":1,"run_id":meta(&tx,"run_id")?,"invocation":meta(&tx,"invocation")?,"resume_count":meta(&tx,"resume_count")?,"citation_key":preparation["citation_key"],"variant":preparation["variant"],"archive_identity":preparation["archive_identity"],"source":preparation["source"],"effective_options":preparation["options"],"contracts":preparation["contracts"],"run_options":run_options(field(&preparation,"variant")?,preparation["options"]["no_figures"]==true,preparation["options"].get("run_options"))?,"archive_verification":"not_requested","layer":layer,"sealed":sealed,"settled":settled,"complete":complete,"excluded":excluded,"counts":counts,"unresolved":unresolved,"coverage_faults":coverage_faults,"publications":publications(&tx,&root_hash)?,"dependencies":dependencies,"units":units,"excluded_layers":target["excluded"],"enabled_layers":target["enabled"]});
    drop(deps_query);
    drop(statement);
    tx.commit()?;
    Ok(result)
}

/// R167: which layer can still append units to a dynamic child layer once that
/// producer has run. Only layers whose producer exists are listed here; the other
/// dynamic layers keep their own slice's closure rule rather than a fabricated one.
fn enumeration_producer(layer: &str) -> Option<&'static str> {
    Some(match layer {
        "cards" | "events" | "findings" | "link" | "models" => "digest",
        _ => return None,
    })
}

/// A dynamic child layer is closed only when every producer that can register more of
/// its units is terminal, so an empty claim queue never completes the layer while a
/// producer can still append to it. Closing is monotone and one event names the fact.
fn close_enumerations(tx: &Transaction) -> Result<()> {
    let target = meta(tx, "target")?;
    for layer in target["enabled"]
        .as_array()
        .context("missing declared target")?
        .iter()
        .filter_map(Value::as_str)
    {
        let Some(producer) = enumeration_producer(layer) else {
            continue;
        };
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

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
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

fn writer(p: &Prepared) -> Result<Connection> {
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

fn next(p: &Prepared, layer: &str, take: i64, owner: &str) -> Result<Value> {
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
        tx.execute("UPDATE units SET state='claimed',owner=?2,claim_token=?3,lease_until=?4,retry_after=NULL,attempts=attempts+1 WHERE id=?1",params![id,owner,token,now+120])?;
        let claim = json!({"id":id,"owner":owner,"token":token,"lease_until":now+120,"input_hash":input_hash,"inputs":serde_json::from_str::<Value>(&inputs)?});
        event(
            &tx,
            &id,
            Some(owner),
            Some(token),
            "claimed",
            &json!({"input_hash":input_hash,"lease_until":now+120}),
        )?;
        claimed.push(claim);
    }
    tx.commit()?;
    let mut result = status(&p.out, layer)?;
    result["units"] = json!(claimed);
    Ok(result)
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
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("open unit guard {}", path.display()))?;
    file.try_lock()
        .map_err(|e| anyhow::anyhow!("lock_busy: {unit}: {e}"))?;
    Ok(UnitGuard { _file: file })
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
    Ok(())
}

fn renew(p: &Prepared, unit: &str, token: i64) -> Result<Value> {
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let claim = claim(&tx, unit, token)?;
    let until = now_seconds() + 120;
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
        &json!({"lease_until":until}),
    )?;
    tx.commit()?;
    Ok(json!({"unit":unit,"token":token,"lease_until":until}))
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
        | "stale_content" | "check_failed" => "content",
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
    if field(fault, "unit")? != unit || field(fault, "input_hash")? != claim.input_hash {
        bail!("fault unit/input hash mismatch");
    }
    field(fault, "cause")?;
    let retry = class == "retryable" && claim.attempts < 3;
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
    json!({"schema_version":1,"unit":job["unit"],"token":job["token"],"run_id":job["run_id"],"input_hash":job["input_hash"],"unit_input_hash":job["unit_input_hash"],"job":path.to_string_lossy(),"prompt":job["artifacts"]["prompt"]["path"],"snapshot":job["artifacts"]["snapshot"]["path"],"snapshot_revision":job["inputs"]["snapshot"]["archive_revision"],"draft":job["draft"],"reused":reused,"lease_until":lease_until})
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
    if job["schema_version"] != 1
        || job["mode"] != "draft_only"
        || job["expected_jobs"] != 1
        || job["jobs"].as_array().is_none_or(|jobs| jobs.len() != 1)
        || job["jobs"][0]["permissions"]["archive_write"] != false
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
    let trees = crate::snapshot::read_title_trees(zt)?;
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
    let inputs = json!({"schema_version":1,"prompt_contract":1,"draft_schema":1,"mode":"draft_only","run_id":run,"unit":unit,"token":token,"preparation_hash":p.hash,"unit_input_hash":claimed.input_hash,"group":group,"anchor":{"address":anchor["address"],"fingerprint":crate::calls::authored_fingerprint(anchor)},"prefix":prefix,"snapshot":snapshot_record,"terms":terms,"contracts":p.contract["contracts"]});
    let input_hash = canon::hash_value(&inputs);
    let mut source = String::new();
    for section in group["sections"].as_array().unwrap() {
        let bytes = std::fs::read(field(section, "path")?)?;
        if canon::sha256_hex(&bytes) != section["sha256"] {
            bail!("group source section hash mismatch during startup");
        }
        source.push_str(std::str::from_utf8(&bytes)?);
        source.push_str("\n\n");
    }
    let draft = json!({"schema_version":1,"unit":unit,"token":token,"input_hash":input_hash,"disposition":"cards","calls":[]});
    let self_check_command = format!(
        "zt-digest group-finish -Out \"{}\" -Group {number} -Token {token} -Draft \"{}\" -CheckOnly",
        p.out.display(),
        draft_path.display()
    );
    let prompt = format!(
        "# 第 {number} 组\n\n本任务只生成 JSON 草稿，写入 `{}`。将共享规则中的写入操作表示为 calls 提案。归档写入和实际 SELF-CHECK 由 group-finish 工具完成。\nSELF-CHECK: {self_check_command}\n\n共享规则：`{}`\n只读启动快照：`{}`\n术语输入：{}\n\n草稿必须保留以下 schema_version、unit、token 和 input_hash，填写 calls。仅有空数组或原始占位锚点不能建立覆盖。创建锚点后的卡片时使用 Side successor，保留其他组的直接锚点位置。编辑本锚点时的 base_fingerprint 为 `{}`。\n\n```json\n{}\n```\n\n## 当前锚点\n\n{}\n\n{}\n\n## 本组选定原文\n\n{}",
        draft_path.display(),
        field(&inputs["prefix"], "path")?,
        snapshot_path.display(),
        if terms.is_null() {
            "本次明确关闭术语层。".to_string()
        } else {
            format!("`{}`", field(&terms, "path")?)
        },
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
    ];
    if !terms.is_null() {
        files.push(terms["path"].clone());
    }
    let job = json!({"schema_version":1,"unit":unit,"token":token,"run_id":run,"owner":claimed.owner,"unit_input_hash":claimed.input_hash,"input_hash":input_hash,"inputs":inputs,"mode":"draft_only","expected_jobs":1,"draft":draft_path.to_string_lossy(),"artifacts":{"prompt":prompt,"prefix":prefix,"snapshot":snapshot_record,"terms":terms},"jobs":[{"id":format!("digest-g{number:03}-t{token}"),"kind":"digest","files":files,"prompt":format!("按组任务生成 JSON 草稿并写入 {}，保留绑定字段。",draft_path.display()),"output":draft_path.to_string_lossy(),"permissions":{"archive_read":true,"archive_write":false}}]});
    let record = publish(&job_path, &serde_json::to_string_pretty(&job)?)?;
    progress["group_start"] = json!({"token":token,"input_hash":input_hash,"job":record});
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    claim(&tx, &unit, token)?;
    let lease_until = now_seconds() + 120;
    tx.execute(
        "UPDATE units SET progress_json=?2,lease_until=?3 WHERE id=?1",
        params![unit, canon::canonical_json(&progress), lease_until],
    )?;
    event(
        &tx,
        &unit,
        Some(&claimed.owner),
        Some(token),
        "group_started",
        &json!({"input_hash":input_hash,"job":record,"snapshot_revision":revision,"lease_until":lease_until}),
    )?;
    tx.commit()?;
    Ok(start_view(&job, &job_path, false, lease_until))
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
    let mut zt = Zt::start()?;
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
    if state == "claimed" {
        claim(&tx, unit, token)?;
    }
    let run = meta(&tx, "run_id")?;
    tx.commit()?;
    Ok((
        Claim {
            owner,
            input_hash,
            attempts,
        },
        serde_json::from_str(&progress)?,
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
        let Some(emitted) = result["emitted"].as_array() else {
            faults.push(json!({"code":"producer_enumeration_missing","unit":producer}));
            continue;
        };
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
        let expected_outputs = completion_output_keys(&result)?;
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
        let expected = terms_output_keys(&result)?;
        let actual = conn
            .prepare("SELECT output_key FROM outputs WHERE unit_id='terms/book@1'")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<std::collections::BTreeSet<_>>>()?;
        if actual != expected {
            faults.push(json!({"code":"producer_outputs_mismatch","unit":"terms/book@1","missing":expected.difference(&actual).collect::<Vec<_>>(),"unexpected":actual.difference(&expected).collect::<Vec<_>>()}));
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
        if actual != needs.iter().cloned().collect() {
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
    let mut source_notes = BTreeMap::new();
    for (i, call) in calls.iter_mut().enumerate() {
        if call.as_object().is_none_or(|v| {
            v.keys()
                .any(|k| !["tool", "arguments"].contains(&k.as_str()))
        }) {
            bail!("group call {i} must contain only tool and arguments");
        }
        let tool = call["tool"].as_str().unwrap_or_default();
        let args = call["arguments"]
            .as_object()
            .context("group call arguments must be an object")?;
        let at = args.get("at").and_then(Value::as_str).unwrap_or_default();
        if tool == "create_literature" {
            bail!("group call cannot create another Literature root");
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
    let _topic_guard = if calls.iter().any(|c| c["tool"] == "create_topic") {
        Some(crate::calls::lock_gate(
            &p.out.join("topic-gate.lock").to_string_lossy(),
        )?)
    } else {
        None
    };
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
    // Replayed Topic roots do not compete with themselves. New roots use the
    // same title normalization and Dice threshold as the existing helper gate.
    if _topic_guard.is_some() {
        let trees = crate::snapshot::read_title_trees(zt)?;
        let mut titles = trees
            .iter()
            .filter(|t| t["workspace"] == "agent" && t["kind"] == "topic")
            .flat_map(|t| {
                t["cards"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|c| c["kind"] == "topic")
            })
            .map(|c| {
                (
                    c["address"].as_str().unwrap_or_default().to_string(),
                    crate::calls::title_key(c["title"].as_str().unwrap_or_default()),
                )
            })
            .collect::<Vec<_>>();
        for (i, call) in calls
            .iter()
            .enumerate()
            .filter(|(_, c)| c["tool"] == "create_topic")
        {
            let address = field(&preview["results"][i]["result"], "address")?;
            let key =
                crate::calls::title_key(call["arguments"]["title"].as_str().unwrap_or_default());
            if titles.iter().any(|(at, other)| {
                at != address && (other == &key || crate::calls::similarity(other, &key) >= 0.8)
            }) {
                bail!("Topic gate refuses duplicate/similar Topic title at call {i}");
            }
            titles.push((address.to_string(), key));
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
    let mut zt = Zt::start()?;
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
    check_file(record, &format!("{label} owned manifest"))?;
    let path = Path::new(field(record, "path")?);
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

/// The Link unit's owner binding and exact in-scope set.
fn link_owned(p: &Prepared, inputs: &Value, number: u64) -> Result<Vec<String>> {
    if inputs["producer"] != json!(format!("digest/g{number:03}@1")) {
        bail!("Link unit input does not name its group producer");
    }
    frozen_owned(p, inputs, "Link unit")
}

/// The accepted plan: one owned paragraph per written Card, and one checked
/// disposition for every other owned Card. A Card is never silently omitted.
fn link_plan(owned: &[String], draft: &Value) -> Result<(Vec<Value>, Vec<Value>)> {
    let in_owned = |at: &str| owned.iter().any(|owned| owned == at);
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
                .any(|key| !["at", "outcome", "reason"].contains(&key.as_str()))
        }) {
            bail!("Link disposition {index} must contain only at, outcome and reason");
        }
        let at = entry["at"]
            .as_str()
            .context("Link disposition has no Card address")?;
        if !in_owned(at) {
            bail!("Link disposition {index} names `{at}`, which is not an owned Card of this Link unit");
        }
        let outcome = entry["outcome"].as_str().unwrap_or_default();
        if !["already_linked", "no_link"].contains(&outcome) {
            bail!("Link disposition {index} must be already_linked or no_link");
        }
        let reason = entry["reason"].as_str().unwrap_or_default().trim();
        if reason.is_empty() || reason.chars().count() > 500 || reason.contains(['\r', '\n']) {
            bail!("Link disposition for `{at}` needs one reason line of at most 500 characters");
        }
        if dispositions
            .insert(at.to_string(), (outcome.to_string(), reason.to_string()))
            .is_some()
        {
            bail!("Link draft disposes `{at}` twice");
        }
    }
    let mut outcomes = Vec::new();
    for at in owned {
        match (written.get(at), dispositions.get(at)) {
            (Some(index), None) => {
                outcomes.push(json!({"at":at,"outcome":"linked","call":index}))
            }
            (None, Some((outcome, reason))) => {
                outcomes.push(json!({"at":at,"outcome":outcome,"reason":reason}))
            }
            (Some(_), Some(_)) => {
                bail!("Link draft gives `{at}` both an owned paragraph and a disposition")
            }
            (None, None) => bail!("Link draft has no outcome for the owned Card `{at}`"),
        }
    }
    Ok((calls, outcomes))
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
        .map(|capture| {
            capture[1]
                .split('|')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .filter(|target| !target.is_empty())
        .collect())
}

/// Every owned Card's required kinded Link, its receipt-owned paragraph and its
/// changed-Card set. Missing or invented Card facts are refused, never skipped.
fn link_verify(
    owned: &[String],
    outcomes: &[Value],
    receipts: &[Value],
    cards: &std::collections::BTreeMap<String, Value>,
) -> Result<(Vec<Value>, Vec<String>)> {
    let mut detail = Vec::new();
    for outcome in outcomes {
        let at = field(outcome, "at")?;
        let card = cards
            .get(at)
            .with_context(|| format!("the owned Card `{at}` is missing from the archive read"))?;
        if card["workspace"] != "agent" {
            bail!("the owned Card `{at}` is not in the agent Workspace");
        }
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
                detail.push(json!({"at":at,"outcome":"linked","receipt":field(receipt,"op_key")?,"paragraph":text,"links":targets,"kinded":kinded}));
            }
            "already_linked" => {
                let kinds = kinded_links(card)?.into_iter().collect::<Vec<_>>();
                if kinds.is_empty() {
                    bail!("`{at}` is recorded as already_linked but carries no kinded Link");
                }
                detail.push(json!({"at":at,"outcome":"already_linked","reason":outcome["reason"],"kinds":kinds}));
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
    let (claimed, _progress, run, done) = finish_claim(p, &unit, token)?;
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
    let (calls, outcomes) = link_plan(&owned, &draft)?;
    let run_id = run.as_str().context("missing run id")?;
    let prefix = format!("{run_id}/link-g{number:03}/1/apply");
    let plan = json!({"schema_version":1,"kind":"link_plan","unit":unit,"unit_input_hash":claimed.input_hash,"plan_revision":1,"operation_prefix":prefix,"owned":owned,"outcomes":outcomes,"calls":calls});
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
    let (detail, draft_changed) = link_verify(&owned, &outcomes, &draft_receipts, &cards)?;
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
    if check_only {
        return Ok(json!({"schema_version":1,"unit":unit,"token":token,"input_hash":claimed.input_hash,"check_only":true,"applied":false,"complete":true,"owned":owned,"outcomes":detail,"changed":draft_changed,"self_check":self_check,"archive_revision":read.archive_revision}));
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
    let (detail, changed) = link_verify(&owned, &outcomes, &receipts, &cards)?;
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
    let result = json!({"schema_version":1,"kind":"link_completion","unit":unit,"token":token,"state":"done","complete":true,"applied":!calls.is_empty(),"reused":false,"input_hash":claimed.input_hash,"unit_input_hash":claimed.input_hash,"plan_hash":plan_hash,"draft":draft_record,"disposition":if calls.is_empty(){"no_link"}else{"linked"},"owned":owned,"outcomes":detail,"changed":changed,"card_fingerprints":card_fingerprints,"receipt_keys":receipt_keys,"self_check":self_check,"files":{"accepted":accepted,"coverage":coverage,"self_check":check_record},"archive_revision":revision});
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
    let mut zt = Zt::start()?;
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
    let mut zt = Zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let result = cards_finish(&p, &mut zt, group, token, check_only)?;
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
    tx.execute(
        "UPDATE units SET progress_json=?2,lease_until=?3 WHERE id=?1",
        params![unit, canon::canonical_json(&progress), now_seconds() + 120],
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
    let mut zt = Zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
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
    let view = status(out, "")?;
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

pub fn cli(argv: &[String]) -> Result<i32> {
    let command = argv
        .first()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let parsed = (|| -> Result<Args> {
        if !matches!(
            command.as_str(),
            "init"
                | "status"
                | "gate"
                | "next"
                | "renew"
                | "done"
                | "fail"
                | "rerun-list"
                | "publish"
                | "notes"
                | "relocate"
                | "math"
        ) {
            bail!(
                "ledger requires init, status, gate, next, renew, done, fail, rerun-list, publish, notes, relocate or math"
            );
        }
        let params = if command == "next" {
            vec!["Out", "Layer", "Take", "Owner"]
        } else if command == "renew" {
            vec!["Out", "Unit", "Token"]
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
        } else {
            vec!["Out", "Layer"]
        };
        let a = Args::parse(&argv[1..], &params, &[])?;
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
        if command == "fail" {
            a.choice("Class", &["retryable", "operator", "content"])?;
            a.required("Fault")?;
        }
        if command == "done" {
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
        if matches!(command.as_str(), "init" | "rerun-list") && a.has("Layer") {
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
        let mut zt = Zt::start()?;
        let p = Prepared::read(&a.str("Out"), &mut zt)?;
        publication(&p, &a.str("Layer"), &a.str_or("Key", "book"), &a.str("Scope"))?
    } else if command == "notes" {
        let mut zt = Zt::start()?;
        let p = Prepared::read(&a.str("Out"), &mut zt)?;
        register_notes(&p, &mut zt, &a.str("Suggestions"))?
    } else if command == "relocate" {
        let mut zt = Zt::start()?;
        let p = Prepared::read(&a.str("Out"), &mut zt)?;
        relocate(&p, &mut zt, &a.str("Moves"))?
    } else if command == "math" {
        let mut zt = Zt::start()?;
        let p = Prepared::read(&a.str("Out"), &mut zt)?;
        math_register(&p, &mut zt)?
    } else if matches!(
        command.as_str(),
        "init" | "next" | "renew" | "done" | "fail"
    ) {
        let mut zt = Zt::start()?;
        let preparing = matches!(command.as_str(), "done" | "renew" | "fail")
            && a.str("Unit") == "prepare/book@1";
        let p = Prepared::read_mode(&a.str("Out"), &mut zt, !preparing)?;
        if command == "init" {
            init(&p)?
        } else if command == "renew" {
            renew(&p, &a.str("Unit"), a.str("Token").parse()?)?
        } else if command == "done" {
            let path = a.str("Outputs");
            if preparing {
                let result =
                    prepare_finish(&p, &mut zt, a.str("Token").parse()?, Path::new(&path))?;
                util::print_line(&serde_json::to_string(&result)?)?;
                return Ok(0);
            }
            let evidence = canon::read_json(&path).context("invalid completion evidence")?;
            let unit = a.str("Unit");
            let token = a.str("Token").parse::<i64>()?;
            if unit == "terms/book@1" {                if evidence["schema_version"] != 1
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
            let path = a.str("Fault");
            if std::fs::metadata(&path)?.len() > 65536 {
                bail!("fault evidence exceeds 64 KiB");
            }
            let fault = util::read_json(&path)?;
            fail(
                &p,
                &a.str("Unit"),
                a.str("Token").parse()?,
                &a.choice("Class", &["retryable", "operator", "content"])?,
                &fault,
            )?
        } else {
            next(&p, &a.str("Layer"), a.str("Take").parse()?, &a.str("Owner"))?
        }
    } else if command == "rerun-list" {
        rerun_list(Path::new(&a.str("Out")))?
    } else {
        if command == "gate" {
            // Settlement is a recorded fact, not a guess: close the dynamic layers whose
            // producers have all settled before reporting the gate (R167, R200).
            let mut zt = Zt::start()?;
            let p = Prepared::read_mode(&a.str("Out"), &mut zt, false)?;
            let mut conn = writer(&p)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            verify_binding(&tx, &p)?;
            close_enumerations(&tx)?;
            tx.commit()?;
        }
        status(Path::new(&a.str("Out")), &a.str("Layer"))?
    };
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(if command == "gate" && result["settled"] != true {
        3
    } else {
        0
    })
}
