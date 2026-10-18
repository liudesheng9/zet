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

/// The initial reliable full-run target; date extraction is not an Event-layer switch.
fn target(contract: &Value) -> Value {
    let mut enabled = COMMON.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let mut excluded = Vec::new();
    if contract["options"]["no_figures"] == true {
        enabled.retain(|s| s != "figures");
        excluded.push("figures");
    }
    if contract["variant"] == "econ" {
        enabled.extend(["events".into(), "findings".into()]);
    } else {
        enabled.extend([
            "math-index".into(),
            "math-docs".into(),
            "math-skeleton".into(),
            "notation".into(),
            "boundary".into(),
            "proof".into(),
            "theory".into(),
            "math-check".into(),
        ]);
        excluded.extend(["events", "findings"]);
    }
    json!({"enabled":enabled,"excluded":excluded})
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

impl Prepared {
    /// All filesystem and MCP checks finish before opening a write transaction.
    fn read(out: &str, zt: &mut Zt) -> Result<Self> {
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
        let observed = zt.archive_view()?;
        if observed["archive_identity"] != contract["archive_identity"] {
            bail!("archive identity mismatch; use the original archive or a new output directory");
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

fn verify_binding(conn: &Connection, p: &Prepared) -> Result<()> {
    if meta(conn, "schema_version")? != 1 {
        bail!("incompatible ledger schema");
    }
    if meta(conn, "preparation_hash")? != p.hash || meta(conn, "preparation")? != p.contract {
        bail!("immutable preparation mismatch; use a new output directory");
    }
    Ok(())
}

fn register(
    tx: &Transaction,
    layer: &str,
    key: &str,
    inputs: &Value,
    creator: Option<&str>,
    done: bool,
) -> Result<String> {
    let id = format!("{layer}/{key}@1");
    let now = canon::utc_now_o();
    tx.execute("INSERT INTO units(id,layer,logical_key,revision,state,inputs_json,input_hash,created_by,created_at,completed_at,result_json) VALUES(?1,?2,?3,1,?4,?5,?6,?7,?8,?9,?10)",params![id,layer,key,if done{"done"}else{"registered"},canon::canonical_json(inputs),canon::hash_value(inputs),creator,now,if done{Some(&now)}else{None},if done{Some("{\"disposition\":\"verified\"}")}else{None}])?;
    Ok(id)
}

fn dependency(tx: &Transaction, id: &str, needs: &str, condition: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO deps(unit_id,needs_id,condition) VALUES(?1,?2,?3)",
        params![id, needs, condition],
    )?;
    Ok(())
}

fn init(p: &Prepared) -> Result<Value> {
    let path = p.out.join("ledger.sqlite3");
    // Existing ledgers are verified read-only: init is not an invocation/resume.
    if path.exists() {
        let conn = connect(&path, false)?;
        verify_binding(&conn, p)?;
        return Ok(json!({"schema_version":1,"run_id":meta(&conn,"run_id")?,"initialized":false}));
    }
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
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("create run identity: {e}"))?;
    let run = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let target = target(&p.contract);
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
    let prepare = register(
        &tx,
        "prepare",
        "book",
        &json!({"preparation_hash":p.hash}),
        None,
        true,
    )?;
    let terms = register(
        &tx,
        "terms",
        "book",
        &json!({"preparation_hash":p.hash}),
        Some(&prepare),
        false,
    )?;
    dependency(&tx, &terms, &prepare, "success")?;
    let mut groups = Vec::new();
    for group in p.contract["groups"].as_array().unwrap() {
        let id = register(
            &tx,
            "digest",
            &format!("g{:03}", group["n"].as_u64().unwrap()),
            group,
            Some(&prepare),
            false,
        )?;
        dependency(&tx, &id, &prepare, "success")?;
        dependency(&tx, &id, &terms, "success")?;
        groups.push(id);
    }
    // These fixed obligations cannot disappear when their dynamic producers have no ready rows.
    for layer in [
        "cards-book",
        "link-check",
        "organize",
        "skeleton",
        "figures",
        "bridge",
        "audit",
    ] {
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
            &json!({"preparation_hash":p.hash}),
            Some(&prepare),
            false,
        )?;
        for group in &groups {
            dependency(&tx, &id, group, "settled")?;
        }
    }
    if p.contract["variant"] == "math" {
        for layer in [
            "math-index",
            "math-docs",
            "math-skeleton",
            "notation",
            "theory",
            "math-check",
        ] {
            let id = register(
                &tx,
                layer,
                "book",
                &json!({"preparation_hash":p.hash}),
                Some(&prepare),
                false,
            )?;
            dependency(&tx, &id, &prepare, "success")?;
        }
    }
    for layer in target["enabled"].as_array().unwrap() {
        put_meta(
            &tx,
            &format!("barrier/{}", layer.as_str().unwrap()),
            &json!({"sealed":matches!(layer.as_str(),Some("prepare"|"terms"|"digest")),"producers_settled":false}),
        )?;
    }
    tx.execute("UPDATE units SET state='ready' WHERE id=?1", [terms])?;
    tx.execute("INSERT INTO outputs(unit_id,output_key,kind,locator,fingerprint,data_json) VALUES(?1,'preparation','manifest',?2,?3,?4)",params![prepare,p.out.join("manifest.json").to_string_lossy(),p.hash,canon::canonical_json(&json!({"group_count":groups.len(),"preparation_hash":p.hash}))])?;
    tx.execute(
        "INSERT INTO events(unit_id,invocation,utc,event,facts_json) VALUES(?1,1,?2,'prepared',?3)",
        params![
            prepare,
            canon::utc_now_o(),
            canon::canonical_json(&json!({"groups":groups.len(),"target":target}))
        ],
    )?;
    tx.commit()?;
    Ok(json!({"schema_version":1,"run_id":run,"initialized":true}))
}

/// Preparation shares its existing MCP process; no nested transaction or client.
pub fn init_prepared(out: &str, zt: &mut Zt) -> Result<Value> {
    init(&Prepared::read(out, zt)?)
}

fn status(out: &Path, layer: &str) -> Result<Value> {
    let conn = connect(&out.join("ledger.sqlite3"), false)?;
    let tx = conn.unchecked_transaction()?;
    if meta(&tx, "schema_version")? != 1 {
        bail!("incompatible ledger schema");
    }
    if meta(&tx, "preparation_hash")? != canon::hash_value(&meta(&tx, "preparation")?) {
        bail!("ledger preparation hash mismatch");
    }
    let mut statement=tx.prepare("SELECT id,layer,logical_key,revision,state,attempts,owner,claim_token,input_hash,inputs_json,fault_json,progress_json,result_json FROM units WHERE (?1='' OR layer=?1) ORDER BY id")?;
    let units=statement.query_map([layer],|r|Ok(json!({"id":r.get::<_,String>(0)?,"layer":r.get::<_,String>(1)?,"logical_key":r.get::<_,String>(2)?,"revision":r.get::<_,i64>(3)?,"state":r.get::<_,String>(4)?,"attempts":r.get::<_,i64>(5)?,"owner":r.get::<_,Option<String>>(6)?,"token":r.get::<_,i64>(7)?,"input_hash":r.get::<_,String>(8)?,"inputs_json":r.get::<_,String>(9)?,"fault_json":r.get::<_,Option<String>>(10)?,"progress_json":r.get::<_,String>(11)?,"result_json":r.get::<_,Option<String>>(12)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
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
    let result = json!({"schema_version":1,"run_id":meta(&tx,"run_id")?,"invocation":meta(&tx,"invocation")?,"archive_verification":"not_requested","complete":false,"units":units,"excluded_layers":target["excluded"],"enabled_layers":target["enabled"]});
    drop(statement);
    tx.commit()?;
    Ok(result)
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let command = argv
        .first()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let parsed = (|| -> Result<Args> {
        if !matches!(command.as_str(), "init" | "status") {
            bail!("ledger requires init or status");
        }
        let a = Args::parse(&argv[1..], &["Out", "Layer"], &[])?;
        a.required("Out")?;
        if !a.positional.is_empty() {
            bail!("unexpected ledger positional arguments");
        }
        if command == "init" && a.has("Layer") {
            bail!("init does not accept -Layer");
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
    let result = if command == "init" {
        let mut zt = Zt::start()?;
        let p = Prepared::read(&a.str("Out"), &mut zt)?;
        init(&p)?
    } else {
        status(Path::new(&a.str("Out")), &a.str("Layer"))?
    };
    util::print_line(&serde_json::to_string(&result)?)?;
    Ok(0)
}
