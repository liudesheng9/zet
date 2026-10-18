//! The Scholar Archive bridge: ZET's side of the one read-only connection to
//! the local Scholar Archive.
//!
//! Scholar Archive pairs with ZET only (`scholar-archive bridge pair`), and
//! ZET keeps the issued credential in its config folder. Every request is
//! signed with HMAC-SHA256 over the method, path, timestamp, a fresh nonce,
//! and the body hash; every answer must come back signed over the same nonce,
//! so ZET knows it reads the real archive. The connection is automatic: the
//! first use pairs ZET when no credential exists, re-pairs once when the
//! archive no longer knows it, and starts the archive's server when it is not
//! running. The bridge exchanges information only; nothing here can change
//! the archive.

use anyhow::{Context, Result, bail};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PROTOCOL: &str = "ZET-SCHOLAR-BRIDGE-1";
const CLIENT: &str = "zet";
const DEFAULT_URL: &str = "http://127.0.0.1:8765";
const CREDENTIAL_FILE: &str = "scholar-bridge.json";
const MAX_ANSWER_BYTES: u64 = 600 * 1024 * 1024;

/// The archive's read tools offered through the bridge, as Scholar Archive names them.
pub(crate) const READ_TOOLS: &[&str] = &[
    "archive_status",
    "list_scholars",
    "scholar_report",
    "list_works",
    "get_work",
    "list_journals",
    "list_workspaces",
    "get_workspace",
    "list_work_files",
    "get_work_file",
    "list_work_artifacts",
    "list_source_labels",
    "list_source_updates",
    "find_duplicate_works",
    "job_status",
    "vector_index_status",
    "semantic_search",
    "similar_works",
];

#[derive(Clone, Debug, PartialEq)]
struct Credential {
    key_id: String,
    secret: String,
    url: String,
}

impl Credential {
    fn from_json(value: &Value) -> Result<Self> {
        let field = |name: &str| {
            value[name]
                .as_str()
                .map(str::to_string)
                .with_context(|| format!("the bridge credential has no `{name}`"))
        };
        if value["client"]
            .as_str()
            .is_some_and(|client| client != CLIENT)
        {
            bail!("the bridge credential is not for ZET");
        }
        let secret = field("secret")?;
        if secret.len() != 64 || !secret.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            bail!("the bridge credential's secret is malformed");
        }
        let url = field("url").unwrap_or_else(|_| DEFAULT_URL.to_string());
        if !(url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:")) {
            bail!("the Scholar Archive bridge only connects to this computer, not `{url}`");
        }
        Ok(Self {
            key_id: field("key_id")?,
            secret,
            url: url.trim_end_matches('/').to_string(),
        })
    }

    fn to_json(&self) -> Value {
        json!({ "client": CLIENT, "key_id": self.key_id, "secret": self.secret, "url": self.url })
    }
}

fn credential_path() -> Result<PathBuf> {
    let config = crate::config_file()?;
    Ok(config
        .parent()
        .context("the config file has no folder")?
        .join(CREDENTIAL_FILE))
}

fn cache_dir() -> Result<PathBuf> {
    Ok(credential_path()?
        .parent()
        .context("the config file has no folder")?
        .join("scholar-archive"))
}

fn load_credential() -> Result<Option<Credential>> {
    let path = credential_path()?;
    match fs::read_to_string(&path) {
        Ok(raw) => {
            let value: Value = serde_json::from_str(&raw)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            Credential::from_json(&value).map(Some)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn save_credential(credential: &Credential) -> Result<()> {
    let path = credential_path()?;
    let folder = path.parent().context("the credential has no folder")?;
    fs::create_dir_all(folder).with_context(|| format!("failed to create {}", folder.display()))?;
    let temporary = folder.join(format!(".{CREDENTIAL_FILE}.tmp"));
    fs::write(
        &temporary,
        serde_json::to_string_pretty(&credential.to_json())?,
    )?;
    fs::rename(&temporary, &path).with_context(|| format!("failed to write {}", path.display()))
}

/// The `scholar-archive` command: `ZT_SCHOLAR_ARCHIVE`, else the installed one.
fn archive_command() -> Result<PathBuf> {
    if let Some(path) = env::var_os("ZT_SCHOLAR_ARCHIVE") {
        return Ok(PathBuf::from(path));
    }
    #[cfg(windows)]
    {
        let local = env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is not set")?;
        let installed = PathBuf::from(local)
            .join("ScholarArchive")
            .join("bin")
            .join("scholar-archive.cmd");
        if installed.exists() {
            return Ok(installed);
        }
        bail!(
            "Scholar Archive is not installed for this user ({} is missing); run its install.cmd",
            installed.display()
        )
    }
    #[cfg(not(windows))]
    {
        Ok(PathBuf::from("scholar-archive"))
    }
}

fn run_archive_command(args: &[&str]) -> Result<String> {
    let program = archive_command()?;
    #[cfg(any(windows, unix))]
    if args == ["start"]
        && env::var_os("ZT_MCP_OWNED_JOB").as_deref() == Some(std::ffi::OsStr::new("1"))
    {
        crate::start_independent_service_command(&program)?;
        return Ok(String::new());
    }
    let mut command = Command::new(&program);
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let output = command
        .output()
        .with_context(|| format!("failed to run {}", program.display()))?;
    if !output.status.success() {
        bail!(
            "`scholar-archive {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Ask Scholar Archive to authorize ZET and keep the credential it issues.
pub(crate) fn pair(rotate: bool) -> Result<Value> {
    let mut args = vec!["bridge", "pair", "--client", CLIENT];
    if rotate {
        args.push("--rotate");
    }
    let stdout = run_archive_command(&args)?;
    let line = stdout
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with('{'))
        .context("Scholar Archive printed no bridge credential")?;
    let issued: Value =
        serde_json::from_str(line).context("failed to parse the bridge credential")?;
    let credential = Credential::from_json(&issued)?;
    save_credential(&credential)?;
    Ok(json!({
        "paired": true,
        "client": CLIENT,
        "key_id": credential.key_id,
        "url": credential.url,
        "credential": credential_path()?.display().to_string(),
    }))
}

/// Forget ZET's credential and ask Scholar Archive to revoke it.
pub(crate) fn unpair() -> Result<Value> {
    let path = credential_path()?;
    let removed_here = match fs::remove_file(&path) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(error).with_context(|| format!("failed to remove {}", path.display()));
        }
    };
    let revoked = run_archive_command(&["bridge", "revoke", "--client", CLIENT])
        .map(|stdout| stdout.contains("\"revoked\": true"))
        .unwrap_or(false);
    Ok(json!({ "removed_credential": removed_here, "revoked_by_archive": revoked }))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn canonical_request(
    method: &str,
    target: &str,
    timestamp: &str,
    nonce: &str,
    body: &[u8],
) -> String {
    format!(
        "{PROTOCOL}\n{}\n{target}\n{timestamp}\n{nonce}\n{}",
        method.to_ascii_uppercase(),
        sha256_hex(body)
    )
}

fn canonical_response(nonce: &str, status: u16, body: &[u8]) -> String {
    format!(
        "{PROTOCOL} RESPONSE\n{nonce}\n{status}\n{}",
        sha256_hex(body)
    )
}

fn sign(secret: &str, message: &str) -> Result<String> {
    let key = (0..secret.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&secret[index..index + 2], 16))
        .collect::<std::result::Result<Vec<u8>, _>>()
        .context("the bridge secret is not hexadecimal")?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).context("invalid bridge secret")?;
    mac.update(message.as_bytes());
    Ok(hex(&mac.finalize().into_bytes()))
}

fn verify(secret: &str, message: &str, signature: &str) -> Result<bool> {
    let expected = sign(secret, message)?;
    // Constant-time comparison of the two hexadecimal strings.
    let same_length = expected.len() == signature.len();
    let difference = expected
        .bytes()
        .zip(signature.bytes())
        .fold(0u8, |acc, (left, right)| acc | (left ^ right));
    Ok(same_length && difference == 0)
}

fn nonce() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| anyhow::anyhow!("no randomness: {error}"))?;
    Ok(hex(&bytes))
}

/// One verified answer from the archive.
pub(crate) struct Answer {
    pub body: Vec<u8>,
    headers: Vec<(String, String)>,
}

impl Answer {
    pub(crate) fn json(&self) -> Result<Value> {
        serde_json::from_slice(&self.body).context("the archive answered with invalid JSON")
    }

    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

enum Failure {
    /// The server could not be reached.
    Offline(String),
    /// The archive does not recognize ZET's credential.
    NotPaired(String),
    Other(anyhow::Error),
}

pub(crate) struct Bridge {
    credential: Credential,
    agent: ureq::Agent,
}

impl Bridge {
    /// Connect automatically: pair when needed and start the archive server when it is down.
    pub(crate) fn connect() -> Result<Self> {
        let credential = match load_credential()? {
            Some(credential) => credential,
            None => {
                pair(false).context("ZET is not paired with Scholar Archive and pairing failed")?;
                load_credential()?.context("pairing left no credential")?
            }
        };
        Ok(Self {
            credential,
            agent: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(5))
                .timeout(Duration::from_secs(600))
                .build(),
        })
    }

    #[cfg(test)]
    fn with(url: &str, key_id: &str, secret: &str) -> Self {
        Self {
            credential: Credential {
                key_id: key_id.into(),
                secret: secret.into(),
                url: url.into(),
            },
            agent: ureq::AgentBuilder::new().build(),
        }
    }

    /// Send one read request, recovering from a stopped server or a lost pairing once each.
    pub(crate) fn request(&mut self, action: &str, payload: &Value) -> Result<Answer> {
        let body = serde_json::to_vec(payload)?;
        let mut started = false;
        let mut repaired = false;
        loop {
            match self.send(action, &body) {
                Ok(answer) => return Ok(answer),
                Err(Failure::Offline(reason))
                    if !started && env::var_os("ZT_SCHOLAR_NO_START").is_none() =>
                {
                    started = true;
                    run_archive_command(&["start"]).with_context(|| {
                        format!(
                            "Scholar Archive is not running ({reason}) and could not be started"
                        )
                    })?;
                    self.wait_until_up();
                }
                Err(Failure::NotPaired(_)) if !repaired => {
                    repaired = true;
                    pair(false)?;
                    self.credential = load_credential()?.context("pairing left no credential")?;
                }
                Err(Failure::Offline(reason)) => {
                    bail!("Scholar Archive is not reachable: {reason}")
                }
                Err(Failure::NotPaired(reason)) => {
                    bail!("Scholar Archive refused ZET's bridge credential: {reason}")
                }
                Err(Failure::Other(error)) => return Err(error),
            }
        }
    }

    fn wait_until_up(&self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if self
                .agent
                .get(&format!("{}/api/status", self.credential.url))
                .call()
                .is_ok()
            {
                return;
            }
            thread::sleep(Duration::from_millis(500));
        }
    }

    fn send(&self, action: &str, body: &[u8]) -> std::result::Result<Answer, Failure> {
        let target = format!("/bridge/v1/{action}");
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| Failure::Other(error.into()))?
            .as_secs()
            .to_string();
        let nonce = nonce().map_err(Failure::Other)?;
        let message = canonical_request("POST", &target, &timestamp, &nonce, body);
        let signature = sign(&self.credential.secret, &message).map_err(Failure::Other)?;
        let request = self
            .agent
            .post(&format!("{}{target}", self.credential.url))
            .set("Content-Type", "application/json")
            .set("X-Zet-Bridge-Client", CLIENT)
            .set("X-Zet-Bridge-Key", &self.credential.key_id)
            .set("X-Zet-Bridge-Timestamp", &timestamp)
            .set("X-Zet-Bridge-Nonce", &nonce)
            .set("X-Zet-Bridge-Signature", &signature);
        let response = match request.send_bytes(body) {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(transport)) => {
                return Err(Failure::Offline(transport.to_string()));
            }
        };
        let status = response.status();
        let headers = response
            .headers_names()
            .into_iter()
            .filter_map(|name| {
                let value = response.header(&name)?.to_string();
                Some((name, value))
            })
            .collect::<Vec<_>>();
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(MAX_ANSWER_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|error| Failure::Other(error.into()))?;
        let answer = Answer {
            body: bytes,
            headers,
        };
        if status == 401 {
            let detail = answer
                .json()
                .ok()
                .and_then(|value| value["detail"].as_str().map(str::to_string))
                .unwrap_or_else(|| "unauthorized".into());
            return Err(Failure::NotPaired(detail));
        }
        let signed = answer.header("X-Zet-Bridge-Signature").unwrap_or_default();
        let verified = verify(
            &self.credential.secret,
            &canonical_response(&nonce, status, &answer.body),
            signed,
        )
        .map_err(Failure::Other)?;
        if !verified {
            return Err(Failure::Other(anyhow::anyhow!(
                "the answer from {} is not signed by the paired Scholar Archive; refusing it",
                self.credential.url
            )));
        }
        if !(200..300).contains(&status) {
            let detail = answer
                .json()
                .ok()
                .map(|value| {
                    format!(
                        "{}: {}",
                        value["error"].as_str().unwrap_or("error"),
                        value["detail"].as_str().unwrap_or("")
                    )
                })
                .unwrap_or_else(|| format!("HTTP {status}"));
            return Err(Failure::Other(anyhow::anyhow!("Scholar Archive: {detail}")));
        }
        Ok(answer)
    }

    pub(crate) fn url(&self) -> &str {
        &self.credential.url
    }

    pub(crate) fn hello(&mut self) -> Result<Value> {
        self.request("hello", &json!({}))?.json()
    }

    pub(crate) fn call(&mut self, tool: &str, arguments: &Value) -> Result<Value> {
        if !READ_TOOLS.contains(&tool) {
            bail!(
                "`{tool}` is not a Scholar Archive read tool; the bridge offers: {}",
                READ_TOOLS.join(", ")
            );
        }
        let answer = self.request("call", &json!({ "tool": tool, "arguments": arguments }))?;
        Ok(answer.json()?["result"].take())
    }

    pub(crate) fn sql(
        &mut self,
        sql: &str,
        params: &Value,
        limit: u64,
        database: &str,
    ) -> Result<Value> {
        self.request(
            "sql",
            &json!({ "sql": sql, "params": params, "limit": limit, "database": database }),
        )?
        .json()
    }

    /// Fetch a document's bytes and keep a verified copy at `out` (default: ZET's cache).
    pub(crate) fn file(&mut self, file_id: u64, out: Option<&Path>) -> Result<FileCopy> {
        let answer = self.request("file", &json!({ "file_id": file_id }))?;
        let digest = sha256_hex(&answer.body);
        if answer.header("X-Zet-Bridge-File-Sha256") != Some(digest.as_str()) {
            bail!("document {file_id} arrived damaged: its SHA-256 does not match");
        }
        let decode = |name: &str| -> Option<String> {
            let value = answer.header(name)?;
            String::from_utf8(base64_url_decode(value)?).ok()
        };
        let relative_path = decode("X-Zet-Bridge-File-Path").unwrap_or_default();
        let name = decode("X-Zet-Bridge-File-Name")
            .filter(|name| {
                !name.is_empty() && !name.contains(['/', '\\']) && name != ".." && name != "."
            })
            .unwrap_or_else(|| format!("file-{file_id}"));
        let path = match out {
            Some(path) => path.to_path_buf(),
            None => cache_dir()?
                .join("files")
                .join(file_id.to_string())
                .join(&name),
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, &answer.body)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(FileCopy {
            file_id,
            name,
            relative_path,
            path,
            sha256: digest,
            bytes: answer.body,
        })
    }

    /// Fetch one artifact (an image of a Markdown document) and verify its SHA-256.
    pub(crate) fn artifact(&mut self, file_id: u64, path: &str) -> Result<(Vec<u8>, String)> {
        let answer = self.request("artifact", &json!({ "file_id": file_id, "path": path }))?;
        let digest = sha256_hex(&answer.body);
        if answer.header("X-Zet-Bridge-File-Sha256") != Some(digest.as_str()) {
            bail!("artifact {path} arrived damaged: its SHA-256 does not match");
        }
        Ok((answer.body, digest))
    }

    pub(crate) fn export(
        &mut self,
        scholar_id: Option<u64>,
        format: &str,
        view: &str,
    ) -> Result<Answer> {
        self.request(
            "export",
            &json!({ "scholar_id": scholar_id, "format": format, "view": view }),
        )
    }
}

/// Take an artifact from the Scholar Archive into this archive's artifact store, and say how a
/// Card shows it. The caption is the one given, else the one the archive read from its Markdown.
pub(crate) fn keep_artifact(
    bridge: &mut Bridge,
    root: &Path,
    file_id: u64,
    path: &str,
    caption: Option<&str>,
) -> Result<Value> {
    let listing = bridge.call("list_work_artifacts", &json!({ "file_id": file_id }))?;
    let listed = listing["artifacts"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["path"] == path))
        .cloned()
        .unwrap_or(Value::Null);
    let (bytes, digest) = bridge.artifact(file_id, path)?;
    let source = json!({
        "scholar_archive": bridge.url(),
        "work_id": listing["work_id"],
        "file_id": file_id,
        "path": path,
        "caption": listed["caption"],
    });
    let stored = crate::artifact::store(root, &bytes, &source)?;
    debug_assert_eq!(stored, digest);
    let caption = caption
        .map(str::to_string)
        .or_else(|| listed["caption"].as_str().map(str::to_string))
        .unwrap_or_default();
    Ok(json!({
        "sha256": digest,
        "address": format!("{}{digest}", crate::artifact::SCHEME),
        "markdown": figure_markdown(&caption, &digest),
        "caption": caption,
        "work_id": listing["work_id"],
        "file_id": file_id,
        "path": crate::artifact::path(root, &digest).map(|path| path.display().to_string()),
        "media_type": crate::artifact::sniff(&bytes).map(|(_, media_type)| media_type),
        "bytes": bytes.len(),
    }))
}

/// The Markdown that shows a kept artifact in a Card.
fn figure_markdown(caption: &str, digest: &str) -> String {
    let alt = caption
        .replace(['\r', '\n'], " ")
        .replace('[', "(")
        .replace(']', ")");
    format!("![{}]({}{digest})", alt.trim(), crate::artifact::SCHEME)
}

/// Take every figure a Markdown document shows into this archive's artifact store, in reading
/// order, so a book digest can hand its writers the `artifact:` Markdown of each one. Images
/// smaller than `min_bytes` (rules, marks, OCR crumbs) and images the archive lacks are skipped.
pub(crate) fn keep_figures(
    bridge: &mut Bridge,
    root: &Path,
    file_id: u64,
    min_bytes: u64,
) -> Result<Value> {
    let listing = bridge.call("list_work_artifacts", &json!({ "file_id": file_id }))?;
    let mut figures = Vec::new();
    let mut skipped = 0;
    for item in listing["artifacts"].as_array().into_iter().flatten() {
        let path = item["path"].as_str().unwrap_or_default();
        let present = item["availability"] == "present";
        if path.is_empty() || !present || item["size_bytes"].as_u64().unwrap_or(0) < min_bytes {
            skipped += 1;
            continue;
        }
        let (bytes, digest) = bridge.artifact(file_id, path)?;
        let source = json!({
            "scholar_archive": bridge.url(),
            "work_id": listing["work_id"],
            "file_id": file_id,
            "path": path,
            "caption": item["caption"],
        });
        crate::artifact::store(root, &bytes, &source)?;
        let caption = item["caption"].as_str().unwrap_or_default();
        figures.push(json!({
            "number": item["number"],
            "name": item["name"],
            "path": path,
            "caption": item["caption"],
            "captions": item["captions"],
            "sha256": digest,
            "markdown": figure_markdown(caption, &digest),
            "bytes": bytes.len(),
        }));
    }
    Ok(json!({
        "work_id": listing["work_id"],
        "file_id": file_id,
        "figures": figures,
        "skipped": skipped,
    }))
}

/// The address scheme of a Document link in a Card body: `[label](scholar:<file_id>)`, with an
/// optional fragment such as `#page=12` for a PDF page.
pub(crate) const DOCUMENT_SCHEME: &str = "scholar:";

/// A Document link's file id and fragment, or None when the address is not one.
pub(crate) fn document_target(dest: &str) -> Option<(u64, Option<&str>)> {
    let rest = dest.strip_prefix(DOCUMENT_SCHEME)?;
    let (id, fragment) = match rest.split_once('#') {
        Some((id, fragment)) => (id, Some(fragment)),
        None => (rest, None),
    };
    if id.is_empty() || id.len() > 18 || !id.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let file_id = id.parse::<u64>().ok().filter(|id| *id > 0)?;
    let fragment = fragment.filter(|fragment| {
        !fragment.is_empty()
            && fragment.len() <= 64
            && fragment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"=._-&".contains(&byte))
    });
    Some((file_id, fragment))
}

/// The Markdown of a Document link to a Scholar Archive document.
pub(crate) fn document_markdown(label: &str, file_id: u64) -> String {
    let label = label
        .replace(['\r', '\n'], " ")
        .replace('[', "(")
        .replace(']', ")");
    format!("[{}]({DOCUMENT_SCHEME}{file_id})", label.trim())
}

/// A Scholar Archive document as ZET holds it: a verified copy in ZET's cache.
pub(crate) struct Document {
    pub file_id: u64,
    pub kind: String,
    pub name: String,
    /// The document's path below the archive's `files/` folder.
    pub relative_path: String,
    pub path: PathBuf,
}

impl Bridge {
    /// A document the archive manages, read from ZET's cache while the cached copy still has the
    /// size the archive records, and fetched (and verified) again otherwise.
    pub(crate) fn document(&mut self, file_id: u64) -> Result<Document> {
        let answer = self.sql(
            "SELECT relative_path, kind, size_bytes, availability FROM work_files WHERE id = ?1",
            &json!([file_id]),
            1,
            "archive",
        )?;
        let Some(row) = answer["rows"].as_array().and_then(|rows| rows.first()) else {
            bail!("Scholar Archive has no document {file_id}");
        };
        let relative_path = row[0].as_str().unwrap_or_default().to_string();
        let kind = row[1].as_str().unwrap_or_default().to_string();
        if row[3].as_str() != Some("present") {
            bail!("Scholar Archive's document {file_id} ({relative_path}) is not present");
        }
        let name = relative_path
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty() && *name != ".." && *name != ".")
            .unwrap_or_default()
            .to_string();
        let cached = cache_dir()?
            .join("files")
            .join(file_id.to_string())
            .join(&name);
        let fresh = !name.is_empty()
            && fs::metadata(&cached)
                .map(|meta| meta.is_file() && Some(meta.len()) == row[2].as_u64())
                .unwrap_or(false);
        let path = if fresh {
            cached
        } else {
            self.file(file_id, None)?.path
        };
        Ok(Document {
            file_id,
            kind,
            name,
            relative_path,
            path,
        })
    }
}

/// Where an export is kept in ZET's cache.
pub(crate) fn export_path(scholar_id: Option<u64>, format: &str) -> Result<PathBuf> {
    let folder = cache_dir()?.join("exports");
    fs::create_dir_all(&folder)
        .with_context(|| format!("failed to create {}", folder.display()))?;
    let scope = scholar_id.map_or_else(|| "all".to_string(), |id| format!("scholar-{id}"));
    let extension = if format == "bibtex" { "bib" } else { format };
    Ok(folder.join(format!("scholar-archive-{scope}.{extension}")))
}

pub(crate) struct FileCopy {
    pub file_id: u64,
    pub name: String,
    pub relative_path: String,
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: Vec<u8>,
}

impl FileCopy {
    /// A Markdown or plain-text document, decoded, when it is small enough to show.
    pub(crate) fn text(&self, limit: usize) -> Option<&str> {
        let lower = self.name.to_ascii_lowercase();
        let textual = [".md", ".markdown", ".txt"]
            .iter()
            .any(|ext| lower.ends_with(ext));
        (textual && self.bytes.len() <= limit)
            .then(|| std::str::from_utf8(&self.bytes).ok())
            .flatten()
    }

    pub(crate) fn summary(&self) -> Value {
        json!({
            "file_id": self.file_id,
            "name": self.name,
            "relative_path": self.relative_path,
            "path": self.path.display().to_string(),
            "bytes": self.bytes.len(),
            "sha256": self.sha256,
        })
    }
}

fn base64_url_decode(text: &str) -> Option<Vec<u8>> {
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = Vec::new();
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(value);
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    Some(out)
}

// `zt scholar ...` ----------------------------------------------------------------------------------

pub(crate) fn run_cli(args: &[String]) -> Result<()> {
    let print = |value: &Value| {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        )
    };
    match args {
        [] => status_cli(),
        [sub] if sub == "status" => status_cli(),
        [sub, rest @ ..] if sub == "pair" => {
            let rotate = match rest {
                [] => false,
                [flag] if flag == "--rotate" => true,
                _ => bail!("expected `zt scholar pair [--rotate]`"),
            };
            print(&pair(rotate)?);
            Ok(())
        }
        [sub] if sub == "unpair" => {
            print(&unpair()?);
            Ok(())
        }
        [sub] if sub == "tools" => {
            let hello = Bridge::connect()?.hello()?;
            for tool in hello["tools"].as_array().into_iter().flatten() {
                let description = tool["description"].as_str().unwrap_or("");
                let first = description.split(". ").next().unwrap_or(description);
                println!("{:<22} {}", tool["name"].as_str().unwrap_or(""), first);
            }
            Ok(())
        }
        [sub, tool, rest @ ..] if sub == "call" => {
            let arguments = match rest {
                [] => json!({}),
                [raw] => serde_json::from_str(raw).context("arguments must be a JSON object")?,
                _ => bail!("expected `zt scholar call <tool> [<json-arguments>]`"),
            };
            print(&Bridge::connect()?.call(tool, &arguments)?);
            Ok(())
        }
        [sub, rest @ ..] if sub == "sql" => {
            let mut database = "archive".to_string();
            let mut limit = 500u64;
            let mut index = 0;
            while index + 1 < rest.len() && rest[index].starts_with("--") {
                match rest[index].as_str() {
                    "--db" => database = rest[index + 1].clone(),
                    "--limit" => {
                        limit = rest[index + 1].parse().context("--limit takes a number")?
                    }
                    other => bail!("unknown option `{other}`"),
                }
                index += 2;
            }
            let [sql] = &rest[index..] else {
                bail!("expected `zt scholar sql [--db archive|vectors] [--limit <n>] <sql>`");
            };
            print(&Bridge::connect()?.sql(sql, &json!([]), limit, &database)?);
            Ok(())
        }
        [sub, id] if sub == "figures" => {
            let work_id: u64 = id.parse().context("work id must be a number")?;
            let listing =
                Bridge::connect()?.call("list_work_artifacts", &json!({ "work_id": work_id }))?;
            for item in listing["artifacts"].as_array().into_iter().flatten() {
                println!(
                    "{:>3} {} {}  {}  [file {} · {}]",
                    item["number"],
                    if item["shown"] == true { "•" } else { " " },
                    item["availability"].as_str().unwrap_or(""),
                    item["caption"].as_str().unwrap_or("(no caption)"),
                    item["file_id"],
                    item["path"].as_str().unwrap_or(""),
                );
            }
            Ok(())
        }
        [sub, id, path, rest @ ..] if sub == "figure" => {
            let file_id: u64 = id.parse().context("file id must be a number")?;
            let caption = match rest {
                [] => None,
                [flag, caption] if flag == "--caption" => Some(caption.as_str()),
                _ => bail!("expected `zt scholar figure <file-id> <path> [--caption <text>]`"),
            };
            let root = crate::require_archive_root()?;
            print(&keep_artifact(
                &mut Bridge::connect()?,
                &root,
                file_id,
                path,
                caption,
            )?);
            Ok(())
        }
        [sub, id, rest @ ..] if sub == "keep-figures" => {
            let file_id: u64 = id.parse().context("file id must be a number")?;
            let (mut min_bytes, mut out) = (0u64, None);
            for pair in rest.chunks(2) {
                match pair {
                    [flag, value] if flag == "--min-bytes" => {
                        min_bytes = value.parse().context("--min-bytes takes a number")?
                    }
                    [flag, value] if flag == "--out" => out = Some(PathBuf::from(value)),
                    _ => bail!(
                        "expected `zt scholar keep-figures <markdown-file-id> [--min-bytes <n>] [--out <path>]`"
                    ),
                }
            }
            let root = crate::require_archive_root()?;
            let kept = keep_figures(&mut Bridge::connect()?, &root, file_id, min_bytes)?;
            match out {
                Some(path) => {
                    fs::write(&path, serde_json::to_string_pretty(&kept)?)
                        .with_context(|| format!("failed to write {}", path.display()))?;
                    println!(
                        "{} figures kept ({} skipped) -> {}",
                        kept["figures"].as_array().map_or(0, Vec::len),
                        kept["skipped"],
                        path.display()
                    );
                }
                None => print(&kept),
            }
            Ok(())
        }
        [sub, id, rest @ ..] if sub == "file" => {
            let file_id: u64 = id.parse().context("file id must be a number")?;
            let out = match rest {
                [] => None,
                [flag, path] if flag == "--out" => Some(PathBuf::from(path)),
                _ => bail!("expected `zt scholar file <file-id> [--out <path>]`"),
            };
            let copy = Bridge::connect()?.file(file_id, out.as_deref())?;
            print(&copy.summary());
            Ok(())
        }
        [sub, rest @ ..] if sub == "export" => {
            let (mut scholar, mut format, mut view, mut out) =
                (None, "json".to_string(), "profile".to_string(), None);
            let mut pairs = rest.chunks(2);
            for pair in pairs.by_ref() {
                match pair {
                    [flag, value] if flag == "--scholar" => {
                        scholar = Some(value.parse().context("--scholar takes a scholar id")?)
                    }
                    [flag, value] if flag == "--format" => format = value.clone(),
                    [flag, value] if flag == "--view" => view = value.clone(),
                    [flag, value] if flag == "--out" => out = Some(PathBuf::from(value)),
                    _ => bail!(
                        "expected `zt scholar export [--scholar <id>] [--format json|csv|bibtex] [--view profile|combined] [--out <path>]`"
                    ),
                }
            }
            let answer = Bridge::connect()?.export(scholar, &format, &view)?;
            match out {
                Some(path) => {
                    fs::write(&path, &answer.body)
                        .with_context(|| format!("failed to write {}", path.display()))?;
                    println!("{} ({} bytes)", path.display(), answer.body.len());
                }
                None => println!("{}", String::from_utf8_lossy(&answer.body)),
            }
            Ok(())
        }
        _ => bail!(
            "expected `zt scholar [status|pair [--rotate]|unpair|tools|call <tool> [<json>]|sql <sql>|file <id> [--out <path>]|figures <work-id>|figure <file-id> <path>|keep-figures <file-id>|export ...]`"
        ),
    }
}

fn status_cli() -> Result<()> {
    let mut bridge = Bridge::connect()?;
    let hello = bridge.hello()?;
    let status = bridge.call("archive_status", &json!({}))?;
    println!("Scholar Archive bridge: connected at {}", bridge.url());
    println!("archive: {}", hello["archive_uuid"].as_str().unwrap_or("?"));
    println!("database: {}", hello["database"].as_str().unwrap_or("?"));
    println!(
        "access: read-only ({} read tools, SQL, documents, figures, exports)",
        hello["tools"].as_array().map_or(0, Vec::len)
    );
    for (label, key) in [
        ("scholars", "scholars"),
        ("works", "works"),
        ("files", "file_count"),
    ] {
        if let Some(value) = status.get(key).filter(|value| !value.is_null()) {
            println!("{label}: {value}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Scholar Archive's tests/test_bridge.py checks the same vectors.
    #[test]
    fn signatures_match_the_archive_known_answers() {
        let request = canonical_request(
            "POST",
            "/bridge/v1/hello",
            "1700000000",
            &"0".repeat(32),
            b"{}",
        );
        assert_eq!(
            sign(&"11".repeat(32), &request).unwrap(),
            "4147ee46b4159b1d8bd2a96f910160fb098c7487ee6d878934a72cf144445127"
        );
        let response = canonical_response(&"0".repeat(32), 200, b"{}");
        assert_eq!(
            sign(&"11".repeat(32), &response).unwrap(),
            "f05b90ff23107ea6fc9268deea3ae357429be2f93b455e0671de3e03739e92dc"
        );
        assert!(
            verify(
                &"11".repeat(32),
                &response,
                "f05b90ff23107ea6fc9268deea3ae357429be2f93b455e0671de3e03739e92dc"
            )
            .unwrap()
        );
        assert!(!verify(&"11".repeat(32), &response, "00").unwrap());
    }

    #[test]
    fn credentials_only_reach_this_computer() {
        let valid = json!({ "client": "zet", "key_id": "ab", "secret": "11".repeat(32), "url": "http://127.0.0.1:8765/" });
        assert_eq!(
            Credential::from_json(&valid).unwrap().url,
            "http://127.0.0.1:8765"
        );
        let remote = json!({ "client": "zet", "key_id": "ab", "secret": "11".repeat(32), "url": "http://example.com:8765" });
        assert!(Credential::from_json(&remote).is_err());
        let other = json!({ "client": "other", "key_id": "ab", "secret": "11".repeat(32) });
        assert!(Credential::from_json(&other).is_err());
    }

    #[test]
    fn a_document_link_names_a_file_and_maybe_a_page() {
        assert_eq!(document_target("scholar:83"), Some((83, None)));
        assert_eq!(
            document_target("scholar:85#page=12"),
            Some((85, Some("page=12")))
        );
        // A fragment that could break out of an attribute is dropped, not passed on.
        assert_eq!(document_target("scholar:85#\"><x"), Some((85, None)));
        for bad in [
            "scholar:",
            "scholar:0",
            "scholar:-1",
            "scholar:8a",
            "scholar:../x",
            "artifact:83",
        ] {
            assert_eq!(document_target(bad), None, "{bad}");
        }
        assert_eq!(
            document_markdown("Book [draft].pdf\n", 85),
            "[Book (draft).pdf](scholar:85)"
        );
    }

    #[test]
    fn decodes_url_safe_base64_names() {
        assert_eq!(base64_url_decode("cGFwZXIubWQ=").unwrap(), b"paper.md");
        assert_eq!(
            base64_url_decode("zrHvvIzOsi5wZGY=").unwrap(),
            "α，β.pdf".as_bytes()
        );
    }

    #[test]
    fn refuses_answers_not_signed_by_the_paired_archive() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr().to_ip().unwrap());
        let handle = thread::spawn(move || {
            let request = server.recv().unwrap();
            let response = tiny_http::Response::from_string("{\"bridge\":1}").with_header(
                "X-Zet-Bridge-Signature: 00"
                    .parse::<tiny_http::Header>()
                    .unwrap(),
            );
            request.respond(response).unwrap();
        });
        let mut bridge = Bridge::with(&url, "ab", &"11".repeat(32));
        let error = bridge.hello().unwrap_err().to_string();
        assert!(
            error.contains("not signed by the paired Scholar Archive"),
            "{error}"
        );
        handle.join().unwrap();
    }
}
