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

const RUN_FLAGS: &[&str] = &[
    "terms", "models", "events", "findings", "hubs", "figures", "bridge", "notation", "boundary",
    "proof", "theory",
];

/// The publication contract version is part of every aggregate input manifest, so
/// a changed derivation contract cannot silently reuse an older revision.
const PUBLICATION_CONTRACT: i64 = 1;

/// Aggregate publication layers. Each gains a new immutable revision when its
/// sorted input manifest changes; per-sentence or per-group aggregates stay out.
const PUBLICATION: &[&str] = &[
    "cards-book",
    "link-check",
    "organize",
    "skeleton",
    "figures",
    "bridge",
    "audit",
    "hubs",
];

/// The producer layers whose recorded work forms one aggregate publication input.
/// The whole-book Card and Link cycles also enumerate their digest groups, so a
/// group that later produces nothing still changes the manifest.
fn publication_sources(layer: &str) -> Option<&'static [&'static str]> {
    match layer {
        "cards-book" => Some(&["digest", "cards"]),
        "link-check" => Some(&["digest", "link"]),
        "organize" | "skeleton" | "figures" | "bridge" | "audit" | "hubs" => Some(&["digest"]),
        _ => None,
    }
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
            check_file(g, "group")?;
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

/// The compiler artifacts that carry one publication layer's checked result. The
/// workflow names the real files; the ledger binds their canonical path and hash.
fn publication_artifacts(layer: &str) -> Option<&'static [&'static str]> {
    Some(match layer {
        "organize" => &["decisions", "applied"],
        "skeleton" => &["skeleton"],
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

/// Complete one aggregate publication revision. The evidence must account for the
/// whole manifest the revision was registered with and bind the compiler artifacts
/// that carry its result, so an unchecked or stale publication cannot be `done`.
fn publication_finish(p: &Prepared, unit: &str, token: i64, path: &Path) -> Result<Value> {
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
    let mut result = json!({"schema_version":1,"kind":"publication_completion","unit":unit,"token":token,"layer":layer,"key":key,"revision":revision,"state":"done","complete":true,"reused":false,"input_hash":claimed.input_hash,"disposition":disposition,"consumed":consumed,"unresolved":unresolved,"artifacts":artifacts});
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
    output(
        &tx,
        unit,
        "completion",
        "file",
        field(&completion, "path")?,
        field(&completion, "sha256")?,
        &completion,
    )?;
    tx.execute("UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",params![unit,canon::canonical_json(&result),canon::utc_now_o()])?;
    event(
        &tx,
        unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"disposition":disposition,"revision":revision,"completion":completion,"consumed":result["consumed"],"unresolved":result["unresolved"]}),
    )?;
    tx.commit()?;
    Ok(result)
}

/// Derive and register the next revision of one aggregate publication. The same
/// sorted input manifest always returns the same revision; a changed manifest
/// registers the next one and makes every older open revision terminal.
fn publication(p: &Prepared, layer: &str, key: &str, scope: Value) -> Result<Value> {
    if !PUBLICATION.contains(&layer) {
        bail!("`{layer}` is not an aggregate publication layer");
    }
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
    for &layer in BOOK {
        if !target["enabled"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l == layer)
        {
            continue;
        }
        let manifest = publication_manifest(&tx, layer, "book", &Value::Null)?;
        register_publication_unit(&tx, layer, "book", 1, &manifest, Some(&prepare))?;
        seal(&tx, layer)?;
    }
    if p.contract["variant"] == "math" {
        for &layer in MATH_BOOK {
            if !target["enabled"]
                .as_array()
                .unwrap()
                .iter()
                .any(|l| l == layer)
            {
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
        }
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
                "ledger-terms-test-",
                "ledger-prepare-recovery-test-",
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
    for layer in BOOK.iter().copied().filter(|l| enabled(l)) {
        units.insert(
            format!("{layer}/book@1"),
            (
                None,
                groups
                    .iter()
                    .map(|id| (id.clone(), "settled".into()))
                    .collect(),
            ),
        );
    }
    if contract["variant"] == "math" {
        for layer in MATH_BOOK.iter().copied().filter(|l| enabled(l)) {
            units.insert(
                format!("{layer}/book@1"),
                (
                    Some(json!({"preparation_hash":hash})),
                    vec![(root.into(), "success".into())],
                ),
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
        ) {
            bail!(
                "ledger requires init, status, gate, next, renew, done, fail, rerun-list or publish"
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
            vec!["Out", "Layer", "Key"]
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
        publication(&p, &a.str("Layer"), &a.str_or("Key", "book"), Value::Null)?
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
                publication_finish(&p, &unit, token, Path::new(&path))?
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
        status(Path::new(&a.str("Out")), &a.str("Layer"))?
    };
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(if command == "gate" && result["settled"] != true {
        3
    } else {
        0
    })
}
