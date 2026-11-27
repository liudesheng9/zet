//! Persistent stdio NDJSON batch runner (`zt-digest batch [-BoxPin <path>]`).
//!
//! Reads one JSON request per line on stdin (`{"id":..., "tool":"...", "args":[...]}`),
//! runs allowlisted digest tools in-process with thread-local `Zt` MCP connection pooling,
//! captures stdout/stderr with last-JSON preservation, and emits one compact NDJSON reply per line.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};

/// Hard ceiling on one NDJSON request line on stdin (4 MiB) to prevent unbounded memory growth.
const MAX_BATCH_REQUEST_BYTES: usize = 4 * 1024 * 1024;

struct BatchRequest {
    id: Value,
    tool: String,
    args: Vec<String>,
}

fn parse_request(line: &str) -> std::result::Result<BatchRequest, (Value, String)> {
    let val: Value = serde_json::from_str(line)
        .map_err(|e| (Value::Null, format!("invalid batch request JSON: {e}")))?;
    let obj = val
        .as_object()
        .ok_or_else(|| (Value::Null, "batch request must be a JSON object".to_string()))?;
    let id = obj.get("id").cloned().unwrap_or(Value::Null);
    let tool = obj
        .get("tool")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| (id.clone(), "batch request missing string `tool`".to_string()))?;
    let args = match obj.get("args") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                let s = item
                    .as_str()
                    .ok_or_else(|| (id.clone(), format!("batch request `args[{i}]` must be a string")))?;
                out.push(s.to_string());
            }
            out
        }
        Some(_) => return Err((id, "batch request `args` must be an array of strings".to_string())),
    };
    Ok(BatchRequest { id, tool, args })
}

struct PoolGuard;

impl PoolGuard {
    fn enter() -> Self {
        crate::zt::begin_batch_pool();
        Self
    }
}

impl Drop for PoolGuard {
    fn drop(&mut self) {
        crate::zt::drain_batch_pool();
    }
}

fn parse_boxpin_arg(argv: &[String]) -> Result<Option<String>> {
    let mut boxpin: Option<String> = None;
    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        if arg.eq_ignore_ascii_case("-BoxPin") {
            let val = argv
                .get(i + 1)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty() && !s.starts_with('-'))
                .context("-BoxPin requires a path argument")?;
            boxpin = Some(val.to_string());
            i += 2;
            continue;
        }
        if let Some((name, val)) = crate::util::split_flag_arg(arg)
            && name.eq_ignore_ascii_case("-BoxPin")
        {
            let v = val.trim();
            if v.is_empty() {
                bail!("-BoxPin requires a non-empty path");
            }
            boxpin = Some(v.to_string());
            i += 1;
            continue;
        }
        bail!("unknown or unexpected argument to `zt-digest batch`: `{arg}`");
    }
    Ok(boxpin)
}

/// Enforces the strict `zt-digest batch` tool allowlist.
/// Explicitly rejects `batch`, `grow`, `call`, `open-call`, `organize-apply -Open*`,
/// `ledger watch`, `ledger hold`, and unknown commands.
pub(crate) fn validate_allowed_tool(tool: &str, args: &[String]) -> Result<()> {
    let low = tool.trim().to_ascii_lowercase();
    if low.is_empty() {
        bail!("batch request `tool` must be non-empty");
    }
    if matches!(low.as_str(), "batch" | "grow" | "call" | "open-call") {
        bail!("tool `{tool}` is not allowed inside `zt-digest batch`");
    }
    if low == "organize-apply" {
        if args.iter().any(|a| crate::is_open_flag(a)) {
            bail!("`organize-apply -Open` is not allowed inside `zt-digest batch`");
        }
        return Ok(());
    }
    if low == "ledger" {
        let sub = args
            .first()
            .map(|s| s.trim().to_ascii_lowercase())
            .unwrap_or_default();
        if sub.is_empty() {
            bail!("`ledger` requires a subcommand");
        }
        if matches!(sub.as_str(), "watch" | "hold") {
            bail!("`ledger {sub}` is not allowed inside `zt-digest batch`");
        }
        return Ok(());
    }
    let allowed = matches!(
        low.as_str(),
        "audit"
            | "card"
            | "coverage"
            | "model"
            | "model-context"
            | "list-models"
            | "bridge"
            | "math"
            | "event"
            | "finding"
            | "gate"
            | "prepare"
            | "prepare-link"
            | "source-faults"
            | "evidence-link"
            | "figures"
            | "finalize"
            | "concepts"
            | "skeleton"
            | "term-cards"
            | "term-link"
            | "term-pilot"
            | "term-sync"
            | "cited"
            | "cited-cards"
            | "organize-survey"
            | "book-cards-finish"
            | "group-start"
            | "group-finish"
            | "link-start"
            | "link-finish"
            | "cards-finish"
            | "events-finish"
            | "findings-finish"
            | "model-finish"
            | "model-deps"
            | "explain-finish"
            | "terms"
            | "cleanup-run"
            | "cleanup-finish"
            | "finish-batch"
            | "service-wait"
            | "version"
            | "--version"
            | "-v"
    );
    if !allowed {
        bail!("tool `{tool}` is not allowed inside `zt-digest batch`");
    }
    Ok(())
}

fn absolutize(raw: &str, cwd: Option<&Path>) -> String {
    let p = Path::new(raw.trim());
    if p.is_absolute() {
        return raw.trim().to_string();
    }
    if let Some(base) = cwd {
        return base.join(p).to_string_lossy().to_string();
    }
    std::path::absolute(p)
        .map(|a| a.to_string_lossy().to_string())
        .unwrap_or_else(|_| raw.trim().to_string())
}

fn collect_request_path_args(bound_args: &[String], cwd: Option<&Path>) -> Vec<String> {
    let mut out = Vec::new();
    let mut batches_files = Vec::new();
    let path_flags = [
        "-Out",
        "-Staging",
        "-Work",
        "-BatchesFile",
        "-UnitsFile",
        "-File",
        "-Plan",
        "-Audit",
        "-Log",
        "-Allow",
        "-Moves",
        "-Outputs",
        "-Source",
        "-Register",
        "-Hub",
        "-Review",
        "-Draft",
        "-RenewUnits",
    ];
    let non_path_value_flags = [
        "-Step",
        "-CitationKey",
        "-Model",
        "-Models",
        "-Occurrences",
        "-Cards",
        "-Round",
        "-Token",
        "-Tool",
        "-Book",
        "-Rules",
        "-Level",
        "-Job",
        "-Need",
        "-Group",
        "-Groups",
        "-PerJob",
        "-Sample",
        "-Seed",
        "-MaxCards",
        "-MaxJobs",
        "-MaxSentences",
        "-MaxChars",
        "-MaxSteps",
        "-MaxExplainSentences",
        "-MaxExplainChars",
        "-MaxParts",
        "-Notes",
        "-Wait",
        "-TitleChars",
    ];
    let mut i = 0;
    while i < bound_args.len() {
        let a = &bound_args[i];
        let mut matched_flag: Option<String> = None;
        let mut val_opt: Option<String> = None;
        let mut consumed_next = false;
        for f in path_flags {
            if a.eq_ignore_ascii_case(f) {
                matched_flag = Some(f.to_string());
                if let Some(next) = bound_args.get(i + 1)
                    && !next.trim().is_empty()
                    && !next.starts_with('-')
                {
                    val_opt = Some(next.clone());
                    consumed_next = true;
                }
                break;
            }
            if let Some((flag, val)) = crate::util::split_flag_arg(a)
                && flag.eq_ignore_ascii_case(f)
            {
                matched_flag = Some(f.to_string());
                let v = val.trim();
                if !v.is_empty() {
                    val_opt = Some(v.to_string());
                }
                break;
            }
        }
        if let (Some(flag), Some(val)) = (matched_flag, val_opt) {
            let abs = absolutize(&val, cwd);
            if flag.eq_ignore_ascii_case("-BatchesFile") {
                batches_files.push(abs.clone());
            }
            out.push(abs);
            if consumed_next {
                i += 1;
            }
        } else if a.starts_with('-') {
            // Skip values of non-path flags (e.g. `-Model K/1`, `-CitationKey K`) so card
            // addresses with slashes are never misinterpreted as relative filesystem paths (H2).
            if non_path_value_flags.iter().any(|f| a.eq_ignore_ascii_case(f))
                && bound_args.get(i + 1).is_some()
            {
                i += 1;
            }
        } else {
            // Only absolute filesystem paths outside flags can anchor a box pin.
            let p = Path::new(a);
            if p.is_absolute() {
                out.push(absolutize(a, cwd));
            }
        }
        i += 1;
    }

    for bf in batches_files {
        if let Ok(Value::Array(items)) = crate::util::read_json(&bf) {
            for item in items {
                for key in ["work", "jobs_file"] {
                    if let Some(s) = item.get(key).and_then(Value::as_str).map(str::trim)
                        && !s.is_empty()
                    {
                        out.push(absolutize(s, cwd));
                    }
                }
            }
        }
    }
    out
}

/// Read-only per-request boxpin verification. Refuses any request that introduces a box pin
/// differing from the session's pinned box (or introduces a pin when the session started unpinned).
pub(crate) fn check_request_boxpin(
    tool: &str,
    args: &[String],
    cwd: Option<&Path>,
    session_config: Option<&str>,
) -> Result<()> {
    let bound = crate::util::bind_tool_args(tool, args);
    let mut check_args = bound.clone();
    check_args.extend(collect_request_path_args(&bound, cwd));
    let pin = zt_check::mcp::boxpin::find(&check_args, cwd)?;
    match (session_config, pin) {
        (None, Some(found)) => bail!(
            "boxpin mismatch: batch session is unpinned (live box), but request references pinned box {} ({})",
            found.file.display(),
            found.config
        ),
        (Some(expected), Some(found)) if !zt_check::mcp::boxpin::same(expected, &found.config) => {
            bail!(
                "boxpin mismatch: batch session is pinned to `{expected}`, but request references {} (`{}`)",
                found.file.display(),
                found.config
            )
        }
        _ => Ok(()),
    }
}

fn emit_response(stdout: &mut impl Write, payload: &Value) -> Result<()> {
    serde_json::to_writer(&mut *stdout, payload)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn process_line(
    line: &str,
    cwd: Option<&Path>,
    session_config: Option<&str>,
    runner: &dyn Fn(&str, &[String]) -> i32,
) -> Value {
    let req = match parse_request(line) {
        Ok(r) => r,
        Err((id, msg)) => {
            return json!({
                "id": id,
                "ok": false,
                "exit_code": 2,
                "error": msg,
                "json": Value::Null,
                "stdout": "",
                "stderr": "",
                "stdout_truncated": false,
                "stderr_truncated": false,
            });
        }
    };
    if let Err(e) = validate_allowed_tool(&req.tool, &req.args)
        .and_then(|()| check_request_boxpin(&req.tool, &req.args, cwd, session_config))
    {
        return json!({
            "id": req.id,
            "ok": false,
            "exit_code": 2,
            "error": format!("{e:#}"),
            "json": Value::Null,
            "stdout": "",
            "stderr": "",
            "stdout_truncated": false,
            "stderr_truncated": false,
        });
    }

    let guard = crate::util::begin_capture(crate::util::DEFAULT_CAPTURE_CAP);
    let exit_code = runner(&req.tool, &req.args);
    let captured = guard.finish();
    json!({
        "id": req.id,
        "ok": exit_code == 0,
        "exit_code": exit_code,
        "json": captured.last_json,
        "stdout": captured.stdout,
        "stderr": captured.stderr,
        "stdout_truncated": captured.stdout_truncated,
        "stderr_truncated": captured.stderr_truncated,
    })
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let boxpin_path = parse_boxpin_arg(argv)?;
    let startup_pin = match &boxpin_path {
        Some(path) => {
            let abs = absolutize(path, std::env::current_dir().ok().as_deref());
            let explicit = ["-Out".to_string(), abs];
            // Resolve ONLY against the explicit -BoxPin anchor first (never cwd / ZT_CONFIG_DIR fallback).
            let Some(found) = zt_check::mcp::boxpin::find(&explicit, None)? else {
                bail!(
                    "-BoxPin `{path}` did not resolve to any {}",
                    zt_check::mcp::boxpin::PIN
                );
            };
            zt_check::mcp::boxpin::apply(&explicit)?;
            Some(found)
        }
        None => zt_check::mcp::boxpin::apply(&[])?,
    };
    let session_config = startup_pin
        .map(|p| p.config)
        .or_else(|| {
            std::env::var("ZT_CONFIG_DIR")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });

    let _pool_guard = PoolGuard::enter();
    let cwd: Option<PathBuf> = std::env::current_dir().ok();
    let stdin = std::io::stdin();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        bytes.clear();
        let n = reader
            .by_ref()
            .take((MAX_BATCH_REQUEST_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)?;
        if n == 0 {
            break;
        }
        if bytes.len() > MAX_BATCH_REQUEST_BYTES {
            if !bytes.ends_with(b"\n") {
                // Drain remainder of oversized line cleanly up to newline or EOF.
                let mut sink = Vec::new();
                loop {
                    sink.clear();
                    let m = reader.by_ref().take(64 * 1024).read_until(b'\n', &mut sink)?;
                    if m == 0 || sink.ends_with(b"\n") {
                        break;
                    }
                }
            }
            let resp = json!({
                "id": Value::Null,
                "ok": false,
                "exit_code": 2,
                "error": format!("batch request line exceeds {MAX_BATCH_REQUEST_BYTES} bytes"),
                "json": Value::Null,
                "stdout": "",
                "stderr": "",
                "stdout_truncated": false,
                "stderr_truncated": false,
            });
            let mut out = std::io::stdout().lock();
            emit_response(&mut out, &resp)?;
            continue;
        }
        let line = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(e) => {
                let resp = json!({
                    "id": Value::Null,
                    "ok": false,
                    "exit_code": 2,
                    "error": format!("batch request is not valid UTF-8: {e}"),
                    "json": Value::Null,
                    "stdout": "",
                    "stderr": "",
                    "stdout_truncated": false,
                    "stderr_truncated": false,
                });
                let mut out = std::io::stdout().lock();
                emit_response(&mut out, &resp)?;
                continue;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let resp = process_line(trimmed, cwd.as_deref(), session_config.as_deref(), &crate::run_one);
        let mut out = std::io::stdout().lock();
        emit_response(&mut out, &resp)?;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svec(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn allowlist_rejects_disallowed_tools_and_allows_safe_digest_tools() {
        for bad in [
            ("batch", vec![]),
            ("grow", svec(&["-Work", "w"])),
            ("call", svec(&["-"])),
            ("open-call", svec(&["-"])),
            ("organize-apply", svec(&["-Work", "w", "-Open"])),
            ("organize-apply", svec(&["-Work", "w", "-Open:true"])),
            ("organize-apply", svec(&["-Work", "w", "-Open=$true"])),
            ("ledger", svec(&["watch"])),
            ("ledger", svec(&["hold", "-Out", "o"])),
            ("ledger", vec![]),
            ("unknown-tool", vec![]),
        ] {
            assert!(validate_allowed_tool(bad.0, &bad.1).is_err(), "expected rejection for {:?}", bad);
        }

        for ok in [
            ("model", svec(&["-Step", "plan-status", "-Work", "w"])),
            ("skeleton", svec(&["-Step", "check", "-Staging", "o"])),
            ("cleanup-run", svec(&["-Out", "o", "-Step", "settle-empty"])),
            ("organize-apply", svec(&["-Work", "w"])),
            ("ledger", svec(&["renew", "-Out", "o", "-UnitsFile", "u.json"])),
            ("version", svec(&["-Workflows"])),
        ] {
            assert!(validate_allowed_tool(ok.0, &ok.1).is_ok(), "expected allow for {:?}", ok);
        }
    }

    #[test]
    fn per_request_boxpin_rejects_mismatched_and_unexpected_pins() {
        let dir = tempfile::tempdir().unwrap();
        let box_a = dir.path().join("run_a");
        let box_b = dir.path().join("run_b");
        let unpinned = dir.path().join("unpinned");
        std::fs::create_dir_all(&box_a).unwrap();
        std::fs::create_dir_all(&box_b).unwrap();
        std::fs::create_dir_all(&unpinned).unwrap();
        std::fs::write(box_a.join("zt-box.txt"), "D:\\box-a\\config\n").unwrap();
        std::fs::write(box_b.join("zt-box.txt"), "D:\\box-b\\config\n").unwrap();

        // Session unpinned vs request naming pinned relative path
        let rel_args = svec(&["-Out", "run_a"]);
        assert!(check_request_boxpin("audit", &rel_args, Some(dir.path()), None).is_err());

        // Session pinned to box_a vs request inside box_a
        assert!(check_request_boxpin("audit", &rel_args, Some(dir.path()), Some("D:\\box-a\\config")).is_ok());

        // Session pinned to box_a vs request pointing via -BatchesFile to box_b
        let batches_file = box_a.join("batches.json");
        std::fs::write(&batches_file, serde_json::to_string(&json!([{"work": box_b.to_string_lossy()}])).unwrap()).unwrap();
        let bf_args = svec(&["-Staging", box_a.to_str().unwrap(), "-BatchesFile", batches_file.to_str().unwrap()]);
        assert!(check_request_boxpin("model", &bf_args, Some(dir.path()), Some("D:\\box-a\\config")).is_err());

        // Session pinned vs neutral command with no pin conflict
        assert!(check_request_boxpin("version", &svec(&["-Workflows"]), Some(&unpinned), Some("D:\\box-a\\config")).is_ok());

        // Multi-byte UTF-8 positional args longer than path flags must never panic
        assert!(check_request_boxpin("version", &svec(&["第一章模型推导", "-Workflows"]), Some(&unpinned), Some("D:\\box-a\\config")).is_ok());

        // -BoxPin=C:\... splits on "=" before drive ":"
        assert_eq!(
            parse_boxpin_arg(&svec(&[r"-BoxPin=C:\box\stage"])).unwrap().as_deref(),
            Some(r"C:\box\stage")
        );
        // -Out=D:\... inside an explicit request still checks boxpin
        let out_eq_arg = format!("-Out={}", box_a.display());
        assert!(check_request_boxpin("audit", &svec(&[out_eq_arg.as_str()]), Some(dir.path()), None).is_err());

        // H2: positional path binding (e.g. `term-link plan run_a`, `organize-apply run_a`)
        // discovers `run_a/zt-box.txt`, while slash-containing non-path values like `-Models run_a/1`
        // do NOT trigger a false pin.
        assert!(check_request_boxpin("term-link", &svec(&["plan", "run_a"]), Some(dir.path()), None).is_err());
        assert!(check_request_boxpin("organize-apply", &svec(&["run_a"]), Some(dir.path()), None).is_err());
        assert!(check_request_boxpin("organize-apply", &svec(&["run_a"]), Some(dir.path()), Some("D:\\box-a\\config")).is_ok());
        assert!(check_request_boxpin("model", &svec(&["-Step", "explain", "-Models", "run_a/1", "-Work", unpinned.to_str().unwrap()]), Some(dir.path()), None).is_ok());
        let bound_me = crate::util::bind_tool_args("model-explain", &svec(&["plan", "K/1", "run_a"]));
        assert_eq!(
            zt_check::mcp::boxpin::find(&bound_me, Some(dir.path())).unwrap().unwrap().config,
            "D:\\box-a\\config"
        );
        let bound_oa = crate::util::bind_tool_args("organize-apply", &svec(&["run_a"]));
        assert_eq!(
            zt_check::mcp::boxpin::find(&bound_oa, Some(dir.path())).unwrap().unwrap().config,
            "D:\\box-a\\config"
        );
        // H2: `model -Report -Work run_a` and `-RenewUnits run_a/u.json` must discover `run_a/zt-box.txt`.
        assert!(check_request_boxpin("model", &svec(&["-Report", "-Work", "run_a"]), Some(dir.path()), None).is_err());
        assert!(check_request_boxpin("model", &svec(&["-RenewUnits", "run_a/u.json"]), Some(dir.path()), None).is_err());
    }

    #[test]
    fn capture_preserves_last_json_even_when_stdout_truncates() {
        let guard = crate::util::begin_capture(32);
        crate::util::print_line(&"x".repeat(100)).unwrap();
        crate::util::print_line(r#"{"ok":true,"count":42}"#).unwrap();
        crate::util::emit_err("diagnostic line");
        let cap = guard.finish();
        assert!(cap.stdout_truncated);
        assert_eq!(cap.stdout.len(), 32);
        assert_eq!(cap.last_json, Some(json!({"ok": true, "count": 42})));
        assert_eq!(cap.stderr, "diagnostic line\n");
        assert!(!cap.stderr_truncated);
    }

    #[test]
    fn batch_subprocess_executes_multiple_ndjson_requests_and_exits_cleanly() {
        use std::process::{Command, Stdio};
        let exe = std::env::current_exe().unwrap();
        // Skip spawning recursively if current_exe IS the test binary; run via `cargo`'s zt-digest.exe sibling if present,
        // otherwise test `process_line` directly through the real dispatcher (`crate::run_one`).
        let candidate_bin = exe
            .parent()
            .and_then(|d| d.parent())
            .map(|d| d.join("zt-digest.exe"));
        let r1 = process_line(
            r#"{"id":1,"tool":"version","args":["-Workflows"]}"#,
            None,
            None,
            &crate::run_one,
        );
        assert_eq!(r1["id"], json!(1));
        assert_eq!(r1["ok"], json!(true));
        assert_eq!(r1["exit_code"], json!(0));
        assert!(r1["json"]["zt_digest"].is_string(), "{r1}");

        let r2 = process_line(
            r#"{"id":"bad","tool":"call","args":["-"]}"#,
            None,
            None,
            &crate::run_one,
        );
        assert_eq!(r2["id"], json!("bad"));
        assert_eq!(r2["ok"], json!(false));
        assert_eq!(r2["exit_code"], json!(2));

        let r3 = process_line(
            r#"not valid json"#,
            None,
            None,
            &crate::run_one,
        );
        assert_eq!(r3["ok"], json!(false));
        assert_eq!(r3["exit_code"], json!(2));

        if let Some(bin) = candidate_bin.filter(|p| p.is_file()) {
            let mut child = Command::new(&bin)
                .arg("batch")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            {
                let mut sin = child.stdin.take().unwrap();
                writeln!(sin, r#"{{"id":10,"tool":"version","args":[]}}"#).unwrap();
                writeln!(sin, r#"{{"id":11,"tool":"grow","args":["-Work","w"]}}"#).unwrap();
                writeln!(sin, r#"{{"id":12,"tool":"version","args":["-Workflows"]}}"#).unwrap();
            }
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
            let lines: Vec<&str> = std::str::from_utf8(&out.stdout).unwrap().lines().filter(|l| !l.trim().is_empty()).collect();
            assert_eq!(lines.len(), 3, "{lines:?}");
            let v0: Value = serde_json::from_str(lines[0]).unwrap();
            let v1: Value = serde_json::from_str(lines[1]).unwrap();
            let v2: Value = serde_json::from_str(lines[2]).unwrap();
            assert_eq!(v0["id"], json!(10));
            assert_eq!(v0["ok"], json!(true));
            assert_eq!(v1["id"], json!(11));
            assert_eq!(v1["ok"], json!(false));
            assert_eq!(v1["exit_code"], json!(2));
            assert_eq!(v2["id"], json!(12));
            assert_eq!(v2["ok"], json!(true));
            assert!(v2["json"]["zt_digest"].is_string());
        }
    }
}
