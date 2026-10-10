//! The Scholar Archive bridge from ZET's side: ZET pairs by itself through the
//! `scholar-archive` command, signs every request, accepts only answers signed
//! by the paired archive, and offers the archive's information read-only on
//! the command line and through `zt mcp`.

use assert_cmd::Command;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tempfile::TempDir;

const SECRET: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const DOCUMENT: &str = "# Fixture paper\n\nExact Unicode: α，β.";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sign(message: &str) -> String {
    let key = (0..SECRET.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&SECRET[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>();
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).unwrap();
    mac.update(message.as_bytes());
    hex(&mac.finalize().into_bytes())
}

/// A stand-in for Scholar Archive's bridge that checks signatures like the real one.
struct FakeArchive {
    url: String,
    seen: Arc<Mutex<Vec<String>>>,
}

impl FakeArchive {
    fn start(tamper: bool) -> Self {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr().to_ip().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        thread::spawn(move || {
            for mut request in server.incoming_requests() {
                let header = |name: &str| {
                    request
                        .headers()
                        .iter()
                        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
                        .map(|h| h.value.as_str().to_string())
                        .unwrap_or_default()
                };
                let (client, timestamp, nonce, signature) = (
                    header("X-Zet-Bridge-Client"),
                    header("X-Zet-Bridge-Timestamp"),
                    header("X-Zet-Bridge-Nonce"),
                    header("X-Zet-Bridge-Signature"),
                );
                let mut body = Vec::new();
                request.as_reader().read_to_end(&mut body).unwrap();
                let target = request.url().to_string();
                let expected = sign(&format!(
                    "ZET-SCHOLAR-BRIDGE-1\nPOST\n{target}\n{timestamp}\n{nonce}\n{}",
                    hex(&Sha256::digest(&body))
                ));
                if client != "zet" || expected != signature {
                    request
                        .respond(
                            tiny_http::Response::from_string("{\"error\":\"bad_signature\"}")
                                .with_status_code(401),
                        )
                        .unwrap();
                    continue;
                }
                let payload: Value = serde_json::from_slice(&body).unwrap();
                log.lock().unwrap().push(format!("{target} {payload}"));
                let (status, answer, extra): (u16, Vec<u8>, Vec<(String, String)>) = match target.as_str() {
                    "/bridge/v1/hello" => (
                        200,
                        json!({ "bridge": 1, "read_only": true, "archive_uuid": "fixture-uuid", "database": "fixture.sqlite3",
                                "tools": [{ "name": "list_scholars", "description": "List scholars.", "inputSchema": {} }] })
                        .to_string()
                        .into_bytes(),
                        vec![],
                    ),
                    "/bridge/v1/call" => (
                        200,
                        json!({ "tool": payload["tool"], "result": match payload["tool"].as_str() {
                            Some("archive_status") => json!({ "scholars": 1, "works": 2 }),
                            _ => json!([{ "id": 7, "name": "Fixture Scholar" }]),
                        }})
                        .to_string()
                        .into_bytes(),
                        vec![],
                    ),
                    "/bridge/v1/sql" => (
                        200,
                        json!({ "columns": ["id"], "rows": [[42]], "truncated": false }).to_string().into_bytes(),
                        vec![],
                    ),
                    "/bridge/v1/file" => (
                        200,
                        DOCUMENT.as_bytes().to_vec(),
                        vec![
                            ("X-Zet-Bridge-File-Name".into(), "cGFwZXIubWQ=".into()),
                            ("X-Zet-Bridge-File-Path".into(), "cGFwZXIubWQ=".into()),
                            ("X-Zet-Bridge-File-Sha256".into(), hex(&Sha256::digest(DOCUMENT.as_bytes()))),
                        ],
                    ),
                    _ => (404, b"{\"error\":\"unknown_action\"}".to_vec(), vec![]),
                };
                let mut signed = sign(&format!(
                    "ZET-SCHOLAR-BRIDGE-1 RESPONSE\n{nonce}\n{status}\n{}",
                    hex(&Sha256::digest(&answer))
                ));
                if tamper {
                    signed = "0".repeat(64);
                }
                let mut response = tiny_http::Response::from_data(answer)
                    .with_status_code(status)
                    .with_header(
                        tiny_http::Header::from_bytes("X-Zet-Bridge-Signature", signed).unwrap(),
                    );
                for (name, value) in extra {
                    response =
                        response.with_header(tiny_http::Header::from_bytes(name, value).unwrap());
                }
                request.respond(response).unwrap();
            }
        });
        Self { url, seen }
    }
}

struct Home {
    _tmp: TempDir,
    config_dir: PathBuf,
    fake_command: PathBuf,
}

impl Home {
    /// A config folder and a fake `scholar-archive` command that issues the credential.
    fn new(archive: &FakeArchive) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join("config");
        let credential =
            json!({ "client": "zet", "key_id": "k1", "secret": SECRET, "url": archive.url })
                .to_string();
        let fake_command = if cfg!(windows) {
            let path = tmp.path().join("scholar-archive.cmd");
            fs::write(
                &path,
                format!("@echo off\r\nif \"%1 %2\"==\"bridge pair\" echo {credential}\r\n"),
            )
            .unwrap();
            path
        } else {
            let path = tmp.path().join("scholar-archive");
            fs::write(
                &path,
                format!("#!/bin/sh\n[ \"$1 $2\" = \"bridge pair\" ] && echo '{credential}'\n"),
            )
            .unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            }
            path
        };
        Self {
            _tmp: tmp,
            config_dir,
            fake_command,
        }
    }

    fn zt(&self) -> Command {
        let mut command = Command::cargo_bin("zt").unwrap();
        command
            .env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_SCHOLAR_ARCHIVE", &self.fake_command)
            .env("ZT_SCHOLAR_NO_START", "1");
        command
    }

    fn output(&self, args: &[&str]) -> (bool, String) {
        let output = self.zt().args(args).output().unwrap();
        let text = String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr);
        (output.status.success(), text)
    }

    fn credential(&self) -> Option<Value> {
        let raw = fs::read_to_string(self.config_dir.join("scholar-bridge.json")).ok()?;
        serde_json::from_str(&raw).ok()
    }
}

#[test]
fn zet_pairs_itself_then_reads_works_sql_and_documents() {
    let archive = FakeArchive::start(false);
    let home = Home::new(&archive);
    assert!(home.credential().is_none());

    let (ok, text) = home.output(&["scholar", "call", "list_scholars"]);
    assert!(ok, "{text}");
    assert!(text.contains("Fixture Scholar"), "{text}");
    assert_eq!(
        home.credential().unwrap()["key_id"],
        "k1",
        "the first use paired ZET"
    );

    let (ok, text) = home.output(&["scholar", "sql", "SELECT id FROM works"]);
    assert!(ok && text.contains("42"), "{text}");

    let (ok, text) = home.output(&["scholar", "file", "3"]);
    assert!(ok, "{text}");
    let summary: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(summary["name"], "paper.md");
    assert_eq!(
        fs::read_to_string(Path::new(summary["path"].as_str().unwrap())).unwrap(),
        DOCUMENT
    );

    let (ok, text) = home.output(&["scholar"]);
    assert!(
        ok && text.contains("connected") && text.contains("read-only"),
        "{text}"
    );

    let seen = archive.seen.lock().unwrap().join("\n");
    assert!(
        seen.contains("/bridge/v1/call") && seen.contains("/bridge/v1/file"),
        "{seen}"
    );
}

#[test]
fn write_tools_are_refused_before_anything_is_sent() {
    let archive = FakeArchive::start(false);
    let home = Home::new(&archive);
    for tool in [
        "update_work",
        "apply_archive_operations",
        "backup_database",
        "start_archive",
    ] {
        let (ok, text) = home.output(&["scholar", "call", tool, "{}"]);
        assert!(
            !ok && text.contains("not a Scholar Archive read tool"),
            "{text}"
        );
    }
    assert!(archive.seen.lock().unwrap().is_empty());
}

#[test]
fn answers_not_signed_by_the_paired_archive_are_refused() {
    let archive = FakeArchive::start(true);
    let home = Home::new(&archive);
    let (ok, text) = home.output(&["scholar", "call", "list_scholars"]);
    assert!(!ok, "{text}");
    assert!(
        text.contains("not signed by the paired Scholar Archive"),
        "{text}"
    );
}

#[test]
fn zt_mcp_offers_the_bridge_as_read_only_tools() {
    let archive = FakeArchive::start(false);
    let home = Home::new(&archive);
    let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .arg("mcp")
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_SCHOLAR_ARCHIVE", &home.fake_command)
        .env("ZT_SCHOLAR_NO_START", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut request = |id: u64, method: &str, params: Value| -> Value {
        writeln!(
            stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
        )
        .unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };
    let tools = request(1, "tools/list", json!({}));
    let bridge_tools = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|tool| {
            tool["name"]
                .as_str()
                .unwrap()
                .starts_with("scholar_archive_")
        })
        .collect::<Vec<_>>();
    assert_eq!(bridge_tools.len(), 5);
    assert!(
        bridge_tools
            .iter()
            .all(|tool| tool["annotations"]["readOnlyHint"] == true)
    );

    let reply = request(
        2,
        "tools/call",
        json!({ "name": "scholar_archive_read", "arguments": { "tool": "list_scholars", "arguments": {} } }),
    );
    assert_eq!(reply["result"]["isError"], false, "{reply}");
    assert_eq!(
        reply["result"]["structuredContent"]["result"][0]["name"],
        "Fixture Scholar"
    );

    let reply = request(
        3,
        "tools/call",
        json!({ "name": "scholar_archive_file", "arguments": { "file_id": 3 } }),
    );
    assert_eq!(
        reply["result"]["structuredContent"]["text"], DOCUMENT,
        "{reply}"
    );

    let reply = request(
        4,
        "tools/call",
        json!({ "name": "scholar_archive_read", "arguments": { "tool": "update_work", "arguments": {} } }),
    );
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    drop(stdin);
    child.wait().unwrap();
}
