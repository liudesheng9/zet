use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DELIM: &str = "<--->";

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => cmd_session(),
        [cmd] if cmd == "version" => {
            println!("zt {VERSION}");
            Ok(())
        }
        [cmd] if cmd == "help" => cmd_help(),
        [cmd] if cmd == "up" => cmd_up(),
        [cmd] if cmd == "down" => cmd_down(),
        [cmd] if cmd == "status" => cmd_status(),
        [cmd] if cmd == "stats" => cmd_stats(),
        [cmd] if cmd == "lsbk" => cmd_lsbk(),
        [cmd, root] if cmd == "__daemon" => cmd_daemon(PathBuf::from(root)),
        [cmd, sub] if cmd == "config" && sub == "show" => cmd_config_show(),
        [cmd, sub, key, value] if cmd == "config" && sub == "set" && key == "archive_root" => {
            cmd_config_set_archive_root(value)
        }
        [cmd, title @ ..] if cmd == "t" => {
            let title = title.join(" ");
            shell_create_topic(&title)
        }
        [cmd, rest @ ..] if cmd == "n" => {
            let at = parse_at(rest)?;
            shell_create_direct(&at)
        }
        [cmd, rest @ ..] if cmd == "b" => {
            let at = parse_at(rest)?;
            shell_create_side(&at)
        }
        [cmd, rest @ ..] if cmd == "e" => {
            let at = parse_at(rest)?;
            shell_edit(&at)
        }
        [cmd, rest @ ..] if cmd == "del" => {
            let at = parse_at(rest)?;
            shell_delete(&at)
        }
        [cmd, rest @ ..] if cmd == "mv" => {
            let (at, new_location) = parse_mv_args(rest)?;
            shell_move(&at, &new_location)
        }
        _ => bail!("unknown command"),
    }
}

fn parse_at(args: &[String]) -> Result<String> {
    match args {
        [flag, value] if flag == "--at" => Ok(value.clone()),
        _ => bail!("expected --at <location>"),
    }
}

fn parse_mv_args(args: &[String]) -> Result<(String, String)> {
    match args {
        [flag, at, new_location] if flag == "--at" => Ok((at.clone(), new_location.clone())),
        _ => bail!("expected --at <location> <new-location>"),
    }
}

#[derive(Debug, Default)]
struct Config {
    archive_root: Option<PathBuf>,
}

impl Config {
    fn load() -> Result<Self> {
        let path = config_file()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        let mut config = Config::default();
        for line in raw.lines() {
            if let Some(value) = line.strip_prefix("archive_root=") {
                config.archive_root = Some(PathBuf::from(value));
            }
        }
        Ok(config)
    }

    fn save(&self) -> Result<()> {
        let path = config_file()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }
        let mut body = String::new();
        if let Some(root) = &self.archive_root {
            body.push_str("archive_root=");
            body.push_str(&root.to_string_lossy());
            body.push('\n');
        }
        fs::write(&path, body).with_context(|| format!("failed to write config {}", path.display()))
    }
}

fn config_file() -> Result<PathBuf> {
    if let Some(dir) = env::var_os("ZT_CONFIG_DIR") {
        return Ok(PathBuf::from(dir).join("config.toml"));
    }
    #[cfg(windows)]
    {
        let appdata = env::var_os("APPDATA").context("APPDATA is not set")?;
        Ok(PathBuf::from(appdata).join("zt").join("config.toml"))
    }
    #[cfg(not(windows))]
    {
        let home = env::var_os("HOME").context("HOME is not set")?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("zt")
            .join("config.toml"))
    }
}

fn require_archive_root() -> Result<PathBuf> {
    Config::load()?
        .archive_root
        .context("archive_root is not configured; run `zt config set archive_root <path>`")
}

fn require_service_up() -> Result<PathBuf> {
    let root = require_archive_root()?;
    if !is_service_up(&root) {
        bail!("service is not up");
    }
    Ok(root)
}

fn db_path(root: &Path) -> PathBuf {
    root.join("zt.sqlite3")
}

fn log_path(root: &Path) -> PathBuf {
    root.join("zt.log")
}

fn service_path(root: &Path) -> PathBuf {
    root.join("zt.pid")
}

fn sessions_path(root: &Path) -> PathBuf {
    root.join("zt.sessions")
}

fn edit_lock_path(root: &Path) -> PathBuf {
    root.join("zt.edit.lock")
}

fn log_event(root: &Path, event: &str) -> Result<()> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path(root))
        .with_context(|| format!("failed to open {}", log_path(root).display()))?;
    writeln!(file, "{seconds} {event}")?;
    Ok(())
}

fn is_service_up(root: &Path) -> bool {
    service_path(root).exists()
}

fn cmd_config_show() -> Result<()> {
    let config = Config::load()?;
    match config.archive_root {
        Some(root) => println!("archive_root={}", root.display()),
        None => println!("archive_root=<unset>"),
    }
    Ok(())
}

fn cmd_config_set_archive_root(value: &str) -> Result<()> {
    let new_root = PathBuf::from(value);
    if let Ok(current_root) = require_archive_root()
        && is_service_up(&current_root)
    {
        bail!("cannot change archive_root while service is up");
    }
    let config = Config {
        archive_root: Some(new_root),
    };
    config.save()?;
    println!("archive_root={value}");
    Ok(())
}

fn cmd_up() -> Result<()> {
    let root = require_archive_root()?;
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;
    if is_service_up(&root) {
        let pid = fs::read_to_string(service_path(&root)).unwrap_or_else(|_| "unknown".to_string());
        println!("service already running pid: {}", pid.trim());
        return Ok(());
    }
    initialize_database(&root)?;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path(&root))
        .with_context(|| format!("failed to open {}", log_path(&root).display()))?;
    let child_id = spawn_daemon(&root)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    while !service_path(&root).exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    if !service_path(&root).exists() {
        bail!("daemon did not start");
    }
    let pid = fs::read_to_string(service_path(&root)).unwrap_or_else(|_| {
        child_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    });
    println!("service started pid: {}", pid.trim());
    Ok(())
}

#[cfg(windows)]
fn spawn_daemon(root: &Path) -> Result<Option<u32>> {
    let exe = env::current_exe()?;
    let script = format!(
        "Start-Process -WindowStyle Hidden -FilePath {} -ArgumentList @('__daemon', {})",
        powershell_quote(&exe),
        powershell_quote(root)
    );
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to launch daemon through PowerShell")?;
    if !status.success() {
        bail!("failed to launch daemon");
    }
    Ok(None)
}

#[cfg(windows)]
fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

#[cfg(not(windows))]
fn spawn_daemon(root: &Path) -> Result<Option<u32>> {
    let child = Command::new(env::current_exe()?)
        .arg("__daemon")
        .arg(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to start daemon")?;
    Ok(Some(child.id()))
}

fn cmd_daemon(root: PathBuf) -> Result<()> {
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;
    log_event(&root, "daemon started")?;
    let pid = std::process::id().to_string();
    fs::write(service_path(&root), &pid)
        .with_context(|| format!("failed to write {}", service_path(&root).display()))?;
    if !sessions_path(&root).exists() {
        fs::write(sessions_path(&root), "").ok();
    }
    loop {
        thread::sleep(Duration::from_millis(250));
        match fs::read_to_string(service_path(&root)) {
            Ok(current) if current.trim() == pid => {}
            _ => break,
        }
    }
    log_event(&root, "daemon stopped")?;
    Ok(())
}

fn initialize_database(root: &Path) -> Result<()> {
    let conn = Connection::open(db_path(root))
        .with_context(|| format!("failed to open {}", db_path(root).display()))?;
    let integrity: String = conn
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .context("failed to run SQLite integrity check")?;
    if integrity != "ok" {
        bail!("SQLite database is corrupted: {integrity}");
    }
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS cards (
            location TEXT PRIMARY KEY,
            is_topic INTEGER NOT NULL,
            text TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )?;
    Ok(())
}

fn cmd_down() -> Result<()> {
    let root = match Config::load()?.archive_root {
        Some(root) => root,
        None => {
            println!("service already stopped");
            return Ok(());
        }
    };
    if !is_service_up(&root) {
        println!("service already stopped");
        return Ok(());
    }
    let sessions = session_count(&root);
    if sessions > 0 {
        bail!("cannot stop service while {sessions} session(s) are open");
    }
    fs::remove_file(service_path(&root))
        .with_context(|| format!("failed to remove {}", service_path(&root).display()))?;
    log_event(&root, "service stop requested").ok();
    println!("service stopped");
    Ok(())
}

fn cmd_status() -> Result<()> {
    let config = Config::load()?;
    let Some(root) = config.archive_root else {
        println!("state: down");
        println!("archive_root: <unset>");
        return Ok(());
    };
    let up = is_service_up(&root);
    println!("state: {}", if up { "up" } else { "down" });
    if up {
        let pid = fs::read_to_string(service_path(&root)).unwrap_or_else(|_| "unknown".to_string());
        println!("pid: {}", pid.trim());
    }
    println!("archive_root: {}", root.display());
    println!("sqlite: {}", db_path(&root).display());
    println!(
        "cards: {}",
        card_counts(&root).map(|c| c.total).unwrap_or(0)
    );
    println!("sessions: {}", session_count(&root));
    Ok(())
}

fn cmd_stats() -> Result<()> {
    let root = require_service_up()?;
    let counts = card_counts(&root)?;
    println!("total: {}", counts.total);
    println!("topics: {}", counts.topics);
    println!("regular: {}", counts.regular);
    Ok(())
}

fn cmd_help() -> Result<()> {
    let up = Config::load()?
        .archive_root
        .as_ref()
        .map(|root| is_service_up(root))
        .unwrap_or(false);
    println!("zt commands:");
    println!("  zt up");
    println!("  zt down");
    println!("  zt status");
    println!("  zt version");
    println!("  zt help");
    println!("  zt config show");
    println!("  zt config set archive_root <path>");
    if up {
        println!("  zt");
        println!("  zt t <title>");
        println!("  zt n --at <location>");
        println!("  zt b --at <location>");
        println!("  zt e --at <location>");
        println!("  zt del --at <location>");
        println!("  zt mv --at <location> <new-location>");
        println!("  zt stats");
        println!("  zt lsbk");
    }
    Ok(())
}

#[derive(Default)]
struct Counts {
    total: i64,
    topics: i64,
    regular: i64,
}

fn card_counts(root: &Path) -> Result<Counts> {
    if !db_path(root).exists() {
        return Ok(Counts::default());
    }
    let conn = Connection::open(db_path(root))?;
    let total = conn
        .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
        .unwrap_or(0);
    let topics = conn
        .query_row("SELECT COUNT(*) FROM cards WHERE is_topic = 1", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);
    Ok(Counts {
        total,
        topics,
        regular: total - topics,
    })
}

fn open_db(root: &Path) -> Result<Connection> {
    Connection::open(db_path(root))
        .with_context(|| format!("failed to open {}", db_path(root).display()))
}

#[derive(Clone, Debug)]
struct Card {
    location: String,
    is_topic: bool,
    text: String,
}

#[derive(Clone, Debug)]
struct ParsedCard {
    title: String,
    body: String,
    reverse: String,
}

fn parse_card_text(text: &str) -> Result<ParsedCard> {
    let lines: Vec<&str> = text.lines().collect();
    let delimiter_positions: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| (*line == DELIM).then_some(idx))
        .collect();
    if delimiter_positions.len() != 2 {
        bail!("card text must contain exactly two {DELIM} delimiter lines");
    }
    let first = delimiter_positions[0];
    let second = delimiter_positions[1];
    let title_lines = &lines[..first];
    if title_lines.len() != 1 {
        bail!("card title must be a single line");
    }
    let title = title_lines[0].to_string();
    if title.trim().is_empty() {
        bail!("card title cannot be empty");
    }
    Ok(ParsedCard {
        title,
        body: lines[first + 1..second].join("\n"),
        reverse: lines[second + 1..].join("\n"),
    })
}

fn compose_card_text(title: &str, body: &str, reverse: &str) -> String {
    format!("{title}\n{DELIM}\n{body}\n{DELIM}\n{reverse}")
}

fn topic_template(title: &str) -> String {
    compose_card_text(title, "", "")
}

fn regular_template() -> String {
    format!("\n{DELIM}\n\n{DELIM}\n")
}

fn extract_link_locations(text: &str) -> Result<Vec<String>> {
    let mut links = Vec::new();
    let mut offset = 0;
    while let Some(start_rel) = text[offset..].find("[[") {
        let start = offset + start_rel;
        let content_start = start + 2;
        let Some(end_rel) = text[content_start..].find("]]") else {
            bail!("invalid link macro");
        };
        let end = content_start + end_rel;
        let content = &text[content_start..end];
        if !is_valid_location(content) {
            bail!("invalid link macro target `{content}`");
        }
        links.push(content.to_string());
        offset = end + 2;
    }
    if text[offset..].contains("]]") {
        bail!("invalid link macro");
    }
    Ok(links)
}

fn validate_card_text(
    conn: &Connection,
    location: &str,
    is_topic: bool,
    text: &str,
    old_text: Option<&str>,
) -> Result<ParsedCard> {
    let parsed = parse_card_text(text)?;
    let links = extract_link_locations(&parsed.body)?;
    if is_topic && !links.is_empty() {
        bail!("topic descriptions cannot contain link macros");
    }
    let mut existing_broken = BTreeSet::new();
    if let Some(old_text) = old_text {
        let old = parse_card_text(old_text)?;
        for link in extract_link_locations(&old.body)? {
            if !location_exists(conn, &link)? {
                existing_broken.insert(link);
            }
        }
    }
    for link in links {
        if link == location && location_exists(conn, location)? {
            continue;
        }
        if !location_exists(conn, &link)? && !existing_broken.contains(&link) {
            bail!("link target `{link}` does not exist");
        }
    }
    Ok(parsed)
}

fn is_valid_location(location: &str) -> bool {
    is_valid_topic_location(location) || is_valid_regular_location(location)
}

fn is_valid_topic_location(location: &str) -> bool {
    let Some((topic, rest)) = location.split_once('/') else {
        return false;
    };
    rest == "0" && valid_topic_id(topic)
}

fn is_valid_regular_location(location: &str) -> bool {
    let Some((topic, rest)) = location.split_once('/') else {
        return false;
    };
    if !valid_topic_id(topic) {
        return false;
    }
    let mut segments = rest.split('|');
    let Some(first) = segments.next() else {
        return false;
    };
    if !valid_positive_number(first) {
        return false;
    }
    segments.all(|segment| valid_positive_number(segment) || valid_side_label(segment))
}

fn valid_topic_id(value: &str) -> bool {
    value == "0" || valid_positive_number(value)
}

fn valid_positive_number(value: &str) -> bool {
    if value.is_empty() || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    if value.len() > 1 && value.starts_with('0') {
        return false;
    }
    value != "0"
}

fn valid_side_label(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_lowercase())
}

fn topic_id(location: &str) -> Result<&str> {
    location
        .split_once('/')
        .map(|(topic, _)| topic)
        .context("invalid location")
}

fn location_exists(conn: &Connection, location: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cards WHERE location = ?1)",
        [location],
        |row| row.get::<_, i64>(0),
    )? == 1)
}

fn load_card(conn: &Connection, location: &str) -> Result<Card> {
    conn.query_row(
        "SELECT location, is_topic, text FROM cards WHERE location = ?1",
        [location],
        |row| {
            Ok(Card {
                location: row.get(0)?,
                is_topic: row.get::<_, i64>(1)? == 1,
                text: row.get(2)?,
            })
        },
    )
    .with_context(|| format!("card `{location}` does not exist"))
}

fn load_cards(conn: &Connection) -> Result<Vec<Card>> {
    let mut stmt = conn.prepare("SELECT location, is_topic, text FROM cards ORDER BY location")?;
    let rows = stmt.query_map([], |row| {
        Ok(Card {
            location: row.get(0)?,
            is_topic: row.get::<_, i64>(1)? == 1,
            text: row.get(2)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn last_segment(location: &str) -> &str {
    location
        .rsplit_once('|')
        .map(|(_, segment)| segment)
        .or_else(|| location.rsplit_once('/').map(|(_, segment)| segment))
        .unwrap_or(location)
}

fn replace_last_segment(location: &str, replacement: &str) -> String {
    if let Some((prefix, _)) = location.rsplit_once('|') {
        format!("{prefix}|{replacement}")
    } else if let Some((topic, _)) = location.rsplit_once('/') {
        format!("{topic}/{replacement}")
    } else {
        replacement.to_string()
    }
}

fn direct_successor(location: &str) -> Result<String> {
    if is_valid_topic_location(location) {
        let topic = topic_id(location)?;
        return Ok(format!("{topic}/1"));
    }
    if !is_valid_regular_location(location) {
        bail!("invalid regular card location `{location}`");
    }
    let last = last_segment(location);
    if valid_positive_number(last) {
        let next = last.parse::<u64>()? + 1;
        Ok(replace_last_segment(location, &next.to_string()))
    } else {
        Ok(format!("{location}|1"))
    }
}

fn parent_location(location: &str) -> Option<String> {
    if is_valid_topic_location(location) {
        return None;
    }
    if !is_valid_regular_location(location) {
        return None;
    }
    let last = last_segment(location);
    if valid_side_label(last) {
        return location
            .rsplit_once('|')
            .map(|(prefix, _)| prefix.to_string());
    }
    let number = last.parse::<u64>().ok()?;
    if number > 1 {
        return Some(replace_last_segment(location, &(number - 1).to_string()));
    }
    if let Some((prefix, _)) = location.rsplit_once('|') {
        Some(prefix.to_string())
    } else {
        topic_id(location).ok().map(|topic| format!("{topic}/0"))
    }
}

fn immediate_side_label(location: &str, parent: &str) -> Option<String> {
    let prefix = format!("{parent}|");
    let rest = location.strip_prefix(&prefix)?;
    let label = rest.split('|').next()?;
    valid_side_label(label).then_some(label.to_string())
}

fn side_label_to_number(label: &str) -> u64 {
    label
        .bytes()
        .fold(0_u64, |acc, byte| acc * 26 + (byte - b'a' + 1) as u64)
}

fn number_to_side_label(mut value: u64) -> String {
    let mut chars = Vec::new();
    while value > 0 {
        value -= 1;
        chars.push((b'a' + (value % 26) as u8) as char);
        value /= 26;
    }
    chars.iter().rev().collect()
}

fn next_side_successor(
    conn: &Connection,
    parent: &str,
    excluding: &BTreeSet<String>,
) -> Result<String> {
    let mut max_label = 0;
    for card in load_cards(conn)? {
        if excluding.contains(&card.location) {
            continue;
        }
        if let Some(label) = immediate_side_label(&card.location, parent) {
            max_label = max_label.max(side_label_to_number(&label));
        }
    }
    Ok(format!("{parent}|{}", number_to_side_label(max_label + 1)))
}

fn descendant_locations(conn: &Connection, root: &str) -> Result<BTreeSet<String>> {
    let cards = load_cards(conn)?;
    let locations: BTreeSet<String> = cards.iter().map(|card| card.location.clone()).collect();
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for location in &locations {
        if let Some(parent) = parent_location(location)
            && locations.contains(&parent)
        {
            children.entry(parent).or_default().push(location.clone());
        }
    }
    let mut result = BTreeSet::new();
    let mut queue = VecDeque::from([root.to_string()]);
    while let Some(location) = queue.pop_front() {
        if !result.insert(location.clone()) {
            continue;
        }
        if let Some(children) = children.get(&location) {
            for child in children {
                queue.push_back(child.clone());
            }
        }
    }
    Ok(result)
}

fn next_topic_location(conn: &Connection) -> Result<String> {
    let next = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'next_topic_id'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| {
            let max_existing: Option<u64> = load_cards(conn)
                .unwrap_or_default()
                .iter()
                .filter(|card| card.is_topic)
                .filter_map(|card| topic_id(&card.location).ok()?.parse::<u64>().ok())
                .max();
            max_existing.map(|value| value + 1).unwrap_or(0)
        });
    Ok(format!("{next}/0"))
}

fn bump_next_topic_id(conn: &Connection, topic_location: &str) -> Result<()> {
    let next = topic_id(topic_location)?.parse::<u64>()? + 1;
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES('next_topic_id', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [next.to_string()],
    )?;
    Ok(())
}

enum EditOutcome {
    Saved(String),
    Canceled,
}

fn run_editor(initial_text: &str) -> Result<EditOutcome> {
    let editor = env::var("EDITOR").context("EDITOR is not set")?;
    let mut temp = tempfile::Builder::new().suffix(".zt.md").tempfile()?;
    temp.write_all(initial_text.as_bytes())?;
    temp.flush()?;
    let temp_path = temp.into_temp_path();
    let path = temp_path.to_path_buf();
    let mut parts = editor.split_whitespace();
    let command = parts.next().context("EDITOR is empty")?;
    let args: Vec<&str> = parts.collect();
    let status = Command::new(command)
        .args(args)
        .arg(&path)
        .status()
        .with_context(|| format!("failed to run editor `{editor}`"))?;
    if !status.success() {
        return Ok(EditOutcome::Canceled);
    }
    let edited = fs::read_to_string(&path)?;
    drop(temp_path);
    Ok(EditOutcome::Saved(edited))
}

fn run_session_editor<R: BufRead>(input: &mut R, initial_text: &str) -> Result<EditOutcome> {
    println!("edit mode");
    println!("{initial_text}");
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        let read = input.read(&mut byte)?;
        if read == 0 {
            return Ok(EditOutcome::Canceled);
        }
        match byte[0] {
            0x13 => {
                let text = String::from_utf8(bytes).context("edited text is not valid UTF-8")?;
                return Ok(EditOutcome::Saved(text));
            }
            0x1b => return Ok(EditOutcome::Canceled),
            value => bytes.push(value),
        }
    }
}

fn session_edit_until_saved<R, F>(input: &mut R, initial_text: &str, mut save: F) -> Result<bool>
where
    R: BufRead,
    F: FnMut(&str) -> Result<()>,
{
    let mut current = initial_text.to_string();
    loop {
        match run_session_editor(input, &current)? {
            EditOutcome::Canceled => return Ok(false),
            EditOutcome::Saved(text) => match save(&text) {
                Ok(()) => return Ok(true),
                Err(err) => {
                    println!("{err:#}");
                    current = text;
                }
            },
        }
    }
}

struct EditLock {
    path: PathBuf,
}

impl Drop for EditLock {
    fn drop(&mut self) {
        fs::remove_file(&self.path).ok();
    }
}

fn acquire_edit_lock(root: &Path) -> Result<EditLock> {
    let path = edit_lock_path(root);
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path);
    match file {
        Ok(_) => Ok(EditLock { path }),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => bail!("edit in progress"),
        Err(err) => Err(err).with_context(|| format!("failed to create {}", path.display())),
    }
}

fn shell_create_topic(title: &str) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    if title.trim().is_empty() || title.contains('\n') || title.contains('\r') {
        bail!("topic title must be non-empty single-line text");
    }
    let mut conn = open_db(&root)?;
    let location = next_topic_location(&conn)?;
    match run_editor(&topic_template(title))? {
        EditOutcome::Canceled => Ok(()),
        EditOutcome::Saved(text) => {
            insert_card(&mut conn, &location, true, &text)?;
            bump_next_topic_id(&conn, &location)?;
            println!("{location}");
            Ok(())
        }
    }
}

fn shell_create_direct(at: &str) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let mut conn = open_db(&root)?;
    let parent = load_card(&conn, at)?;
    let location = direct_successor(&parent.location)?;
    if location_exists(&conn, &location)? {
        bail!("direct successor already exists: {location}");
    }
    match run_editor(&regular_template())? {
        EditOutcome::Canceled => Ok(()),
        EditOutcome::Saved(text) => {
            insert_card(&mut conn, &location, false, &text)?;
            println!("{location}");
            Ok(())
        }
    }
}

fn shell_create_side(at: &str) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let mut conn = open_db(&root)?;
    let parent = load_card(&conn, at)?;
    if parent.is_topic {
        bail!("zt b is not valid on a topic card");
    }
    let location = next_side_successor(&conn, &parent.location, &BTreeSet::new())?;
    match run_editor(&regular_template())? {
        EditOutcome::Canceled => Ok(()),
        EditOutcome::Saved(text) => {
            insert_card(&mut conn, &location, false, &text)?;
            println!("{location}");
            Ok(())
        }
    }
}

fn shell_edit(at: &str) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let mut conn = open_db(&root)?;
    let card = load_card(&conn, at)?;
    match run_editor(&card.text)? {
        EditOutcome::Canceled => Ok(()),
        EditOutcome::Saved(text) => update_card_text(&mut conn, &card, &text),
    }
}

fn insert_card(conn: &mut Connection, location: &str, is_topic: bool, text: &str) -> Result<()> {
    if !is_valid_location(location) {
        bail!("invalid location `{location}`");
    }
    if location_exists(conn, location)? {
        bail!("card `{location}` already exists");
    }
    validate_card_text(conn, location, is_topic, text, None)?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO cards(location, is_topic, text) VALUES(?1, ?2, ?3)",
        params![location, if is_topic { 1 } else { 0 }, text],
    )?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(())
}

fn update_card_text(conn: &mut Connection, card: &Card, text: &str) -> Result<()> {
    validate_card_text(conn, &card.location, card.is_topic, text, Some(&card.text))?;
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE cards SET text = ?1 WHERE location = ?2",
        params![text, card.location],
    )?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(())
}

fn regenerate_reverse_links_tx(conn: &Connection) -> Result<()> {
    let cards = load_cards(conn)?;
    let existing: BTreeSet<String> = cards.iter().map(|card| card.location.clone()).collect();
    let titles: BTreeMap<String, String> = cards
        .iter()
        .filter_map(|card| {
            parse_card_text(&card.text)
                .ok()
                .map(|parsed| (card.location.clone(), parsed.title))
        })
        .collect();
    let mut inbound: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for source in &cards {
        let parsed = parse_card_text(&source.text)?;
        let mut seen = BTreeSet::new();
        for target in extract_link_locations(&parsed.body)? {
            if existing.contains(&target) && seen.insert(target.clone()) {
                inbound
                    .entry(target)
                    .or_default()
                    .insert(source.location.clone());
            }
        }
    }
    for card in &cards {
        let parsed = parse_card_text(&card.text)?;
        let reverse = inbound
            .get(&card.location)
            .into_iter()
            .flat_map(|sources| sources.iter())
            .map(|source| {
                let title = titles.get(source).cloned().unwrap_or_default();
                format!("This note has been referred by note [[{source}]] {title}")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let text = compose_card_text(&parsed.title, &parsed.body, &reverse);
        conn.execute(
            "UPDATE cards SET text = ?1 WHERE location = ?2",
            params![text, card.location],
        )?;
    }
    Ok(())
}

fn replace_link_locations(text: &str, mapping: &BTreeMap<String, String>) -> Result<String> {
    let mut output = String::new();
    let mut offset = 0;
    while let Some(start_rel) = text[offset..].find("[[") {
        let start = offset + start_rel;
        let content_start = start + 2;
        let Some(end_rel) = text[content_start..].find("]]") else {
            bail!("invalid link macro");
        };
        let end = content_start + end_rel;
        output.push_str(&text[offset..content_start]);
        let target = &text[content_start..end];
        output.push_str(mapping.get(target).map(String::as_str).unwrap_or(target));
        output.push_str("]]");
        offset = end + 2;
    }
    output.push_str(&text[offset..]);
    Ok(output)
}

fn shell_delete(at: &str) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let mut conn = open_db(&root)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let result = delete_card(&mut conn, at, |expected| {
        read_confirmation(&mut input, expected)
    })?;
    println!("deleted: {}", result.deleted_count);
    if !result.moved.is_empty() {
        println!("compacted:");
        for (old, new) in result.moved {
            println!("{old} -> {new}");
        }
    }
    Ok(())
}

struct DeleteResult {
    deleted_count: usize,
    moved: BTreeMap<String, String>,
}

fn delete_card<F>(conn: &mut Connection, at: &str, mut confirm: F) -> Result<DeleteResult>
where
    F: FnMut(&str) -> Result<()>,
{
    let card = load_card(conn, at)?;
    let delete_set = if card.is_topic {
        let topic = topic_id(&card.location)?;
        load_cards(conn)?
            .into_iter()
            .filter(|card| card.location.starts_with(&format!("{topic}/")))
            .map(|card| card.location)
            .collect::<BTreeSet<_>>()
    } else {
        descendant_locations(conn, at)?
    };
    println!("delete verification:");
    for location in &delete_set {
        println!("delete {location}");
    }
    println!("successors: {}", delete_set.len().saturating_sub(1));
    let confirmation = if card.is_topic { at } else { "delete" };
    confirm(confirmation)?;
    let compaction = side_compaction_mapping(conn, at, &delete_set)?;
    let tx = conn.transaction()?;
    for location in &delete_set {
        tx.execute("DELETE FROM cards WHERE location = ?1", [location])?;
    }
    apply_location_mapping_tx(&tx, &compaction)?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(DeleteResult {
        deleted_count: delete_set.len(),
        moved: compaction,
    })
}

fn read_confirmation<R: BufRead>(input: &mut R, expected: &str) -> Result<()> {
    println!("type `{expected}` to confirm:");
    let mut line = String::new();
    input.read_line(&mut line)?;
    if line.trim() != expected {
        bail!("confirmation did not match");
    }
    Ok(())
}

fn side_compaction_mapping(
    conn: &Connection,
    deleted_root: &str,
    delete_set: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>> {
    let last = last_segment(deleted_root);
    if !valid_side_label(last) {
        return Ok(BTreeMap::new());
    }
    let Some(parent) = parent_location(deleted_root) else {
        return Ok(BTreeMap::new());
    };
    let deleted_rank = side_label_to_number(last);
    let cards = load_cards(conn)?;
    let mut labels = BTreeSet::new();
    for card in &cards {
        if delete_set.contains(&card.location) {
            continue;
        }
        if let Some(label) = immediate_side_label(&card.location, &parent)
            && side_label_to_number(&label) > deleted_rank
        {
            labels.insert(label);
        }
    }
    let mut prefix_map = BTreeMap::new();
    for label in labels {
        let rank = side_label_to_number(&label);
        prefix_map.insert(
            format!("{parent}|{label}"),
            format!("{parent}|{}", number_to_side_label(rank - 1)),
        );
    }
    let mut mapping = BTreeMap::new();
    for card in cards {
        if delete_set.contains(&card.location) {
            continue;
        }
        for (old_prefix, new_prefix) in &prefix_map {
            if card.location == *old_prefix || card.location.starts_with(&format!("{old_prefix}|"))
            {
                let suffix = &card.location[old_prefix.len()..];
                mapping.insert(card.location.clone(), format!("{new_prefix}{suffix}"));
                break;
            }
        }
    }
    Ok(mapping)
}

fn apply_location_mapping_tx(conn: &Connection, mapping: &BTreeMap<String, String>) -> Result<()> {
    if mapping.is_empty() {
        return Ok(());
    }
    for old in mapping.keys() {
        conn.execute(
            "UPDATE cards SET location = ?1 WHERE location = ?2",
            params![format!("__moving__{old}"), old],
        )?;
    }
    for (old, new) in mapping {
        conn.execute(
            "UPDATE cards SET location = ?1 WHERE location = ?2",
            params![new, format!("__moving__{old}")],
        )?;
    }
    let cards = load_cards(conn)?;
    for card in cards {
        let text = replace_link_locations(&card.text, mapping)?;
        conn.execute(
            "UPDATE cards SET text = ?1 WHERE location = ?2",
            params![text, card.location],
        )?;
    }
    Ok(())
}

fn shell_move(at: &str, new_location: &str) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let mut conn = open_db(&root)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mapping = move_card(&mut conn, at, new_location, |expected| {
        read_confirmation(&mut input, expected)
    })?;
    println!("moved: {}", mapping.len());
    for (old, new) in mapping {
        println!("{old} -> {new}");
    }
    Ok(())
}

fn move_card<F>(
    conn: &mut Connection,
    at: &str,
    new_location: &str,
    mut confirm: F,
) -> Result<BTreeMap<String, String>>
where
    F: FnMut(&str) -> Result<()>,
{
    let card = load_card(conn, at)?;
    if card.is_topic {
        bail!("topic cards cannot be moved");
    }
    if !is_valid_regular_location(new_location) {
        bail!("invalid move location `{new_location}`");
    }
    let moved = descendant_locations(conn, at)?;
    if moved.contains(new_location) {
        bail!("cannot move a card into its own successors");
    }
    validate_move_target(conn, &moved, new_location)?;
    let mapping = move_mapping(conn, at, new_location, &moved)?;
    let conflicts: Vec<String> = mapping
        .values()
        .filter(|location| {
            location_exists(conn, location).unwrap_or(false) && !moved.contains(*location)
        })
        .cloned()
        .collect();
    if !conflicts.is_empty() {
        bail!(
            "destination locations already exist: {}",
            conflicts.join(", ")
        );
    }
    println!("move verification:");
    for (old, new) in &mapping {
        println!("{old} -> {new}");
    }
    println!("moved cards: {}", mapping.len());
    println!(
        "link macros rewritten: {}",
        count_rewritten_links(conn, &mapping)?
    );
    confirm("move")?;
    let tx = conn.transaction()?;
    apply_location_mapping_tx(&tx, &mapping)?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(mapping)
}

fn validate_move_target(
    conn: &Connection,
    moved: &BTreeSet<String>,
    new_location: &str,
) -> Result<()> {
    let parent = parent_location(new_location).context("move target has no parent")?;
    if !location_exists(conn, &parent)? || moved.contains(&parent) {
        bail!("move target parent does not exist outside the moved subtree");
    }
    let last = last_segment(new_location);
    if valid_side_label(last) {
        let expected = next_side_successor(conn, &parent, moved)?;
        if expected != new_location {
            bail!("side successor move target must be the next available label: {expected}");
        }
    } else if direct_successor(&parent)? != new_location {
        bail!("move target does not preserve direct successor rules");
    }
    Ok(())
}

fn move_mapping(
    conn: &Connection,
    old_root: &str,
    new_root: &str,
    moved: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>> {
    let cards = load_cards(conn)?;
    let locations: BTreeSet<String> = cards.iter().map(|card| card.location.clone()).collect();
    let mut mapping = BTreeMap::new();
    for location in moved {
        let edges = path_edges(old_root, location, &locations)?;
        let mut target = new_root.to_string();
        for edge in edges {
            target = match edge {
                Edge::Direct => direct_successor(&target)?,
                Edge::Side(label) => format!("{target}|{label}"),
            };
        }
        mapping.insert(location.clone(), target);
    }
    Ok(mapping)
}

enum Edge {
    Direct,
    Side(String),
}

fn path_edges(root: &str, location: &str, existing: &BTreeSet<String>) -> Result<Vec<Edge>> {
    if root == location {
        return Ok(Vec::new());
    }
    let mut current = location.to_string();
    let mut reversed = Vec::new();
    while current != root {
        let parent = parent_location(&current).context("location has no parent")?;
        if !existing.contains(&parent) && parent != root {
            bail!("missing parent `{parent}`");
        }
        let last = last_segment(&current);
        if valid_side_label(last) {
            reversed.push(Edge::Side(last.to_string()));
        } else {
            reversed.push(Edge::Direct);
        }
        current = parent;
    }
    reversed.reverse();
    Ok(reversed)
}

fn count_rewritten_links(conn: &Connection, mapping: &BTreeMap<String, String>) -> Result<usize> {
    let mut count = 0;
    for card in load_cards(conn)? {
        let parsed = parse_card_text(&card.text)?;
        for link in extract_link_locations(&parsed.body)? {
            if mapping.contains_key(&link) {
                count += 1;
            }
        }
    }
    Ok(count)
}

fn cmd_lsbk() -> Result<()> {
    let root = require_service_up()?;
    let conn = open_db(&root)?;
    let cards = load_cards(&conn)?;
    for card in &cards {
        let parsed = parse_card_text(&card.text)?;
        for line in parsed.body.lines() {
            for target in extract_link_locations(line)? {
                if !location_exists(&conn, &target)? {
                    println!("{} {} -> {}: {}", card.location, parsed.title, target, line);
                }
            }
        }
    }
    Ok(())
}

fn cmd_session() -> Result<()> {
    let root = require_service_up()?;
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        return cmd_tui_session(root);
    }
    cmd_line_session(root)
}

fn cmd_line_session(root: PathBuf) -> Result<()> {
    register_session(&root)?;
    let guard = SessionGuard { root: root.clone() };
    let mut pointer = Pointer::Root;
    print_view(&root, &pointer)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            break;
        }
        if !is_service_up(&root) {
            println!("service disconnected");
            break;
        }
        let line = line.trim_end_matches(['\r', '\n']).to_string();
        match handle_session_command(&root, &mut pointer, &line, &mut input) {
            Ok(keep_going) => {
                if !keep_going {
                    break;
                }
                print_view(&root, &pointer)?;
            }
            Err(err) => {
                println!("{err:#}");
            }
        }
    }
    drop(guard);
    Ok(())
}

enum Pointer {
    Root,
    Card(String),
}

struct SessionGuard {
    root: PathBuf,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        unregister_session(&self.root).ok();
    }
}

fn session_count(root: &Path) -> i64 {
    let active = active_session_pids(root);
    let body = active
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(sessions_path(root), body).ok();
    active.len() as i64
}

fn active_session_pids(root: &Path) -> Vec<u32> {
    fs::read_to_string(sessions_path(root))
        .ok()
        .map(|value| {
            value
                .lines()
                .filter_map(|line| line.trim().parse::<u32>().ok())
                .filter(|pid| process_exists(*pid))
                .collect()
        })
        .unwrap_or_default()
}

fn register_session(root: &Path) -> Result<()> {
    let pid = std::process::id();
    let mut active = active_session_pids(root);
    if !active.contains(&pid) {
        active.push(pid);
    }
    write_session_pids(root, &active)
}

fn unregister_session(root: &Path) -> Result<()> {
    let pid = std::process::id();
    let active = active_session_pids(root)
        .into_iter()
        .filter(|active_pid| *active_pid != pid)
        .collect::<Vec<_>>();
    write_session_pids(root, &active)
}

fn write_session_pids(root: &Path, pids: &[u32]) -> Result<()> {
    let body = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(sessions_path(root), body)?;
    Ok(())
}

#[cfg(windows)]
fn process_exists(pid: u32) -> bool {
    let filter = format!("PID eq {pid}");
    Command::new("tasklist")
        .args(["/FI", &filter, "/NH"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn process_exists(pid: u32) -> bool {
    Command::new("sh")
        .args(["-c", &format!("kill -0 {pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .is_some_and(|status| status.success())
}

#[derive(Clone)]
struct LinkSpan {
    row: u16,
    start: u16,
    end: u16,
    target: String,
}

struct TuiGuard {
    root: PathBuf,
}

impl Drop for TuiGuard {
    fn drop(&mut self) {
        crossterm::terminal::disable_raw_mode().ok();
        crossterm::execute!(
            io::stdout(),
            crossterm::event::DisableMouseCapture,
            crossterm::terminal::LeaveAlternateScreen
        )
        .ok();
        unregister_session(&self.root).ok();
    }
}

fn cmd_tui_session(root: PathBuf) -> Result<()> {
    register_session(&root)?;
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let _guard = TuiGuard { root: root.clone() };
    let mut pointer = Pointer::Root;
    let mut command = String::new();
    let mut message = String::new();
    loop {
        if !is_service_up(&root) {
            message = "service disconnected".to_string();
            draw_tui(&root, &pointer, &command, &message)?;
            break;
        }
        let links = draw_tui(&root, &pointer, &command, &message)?;
        match crossterm::event::read()? {
            crossterm::event::Event::Key(key) if is_tui_input_key(&key) => match key.code {
                crossterm::event::KeyCode::Char('c')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    break;
                }
                crossterm::event::KeyCode::Char(ch) => command.push(ch),
                crossterm::event::KeyCode::Backspace => {
                    command.pop();
                }
                crossterm::event::KeyCode::Enter => {
                    let line = command.trim().to_string();
                    command.clear();
                    match handle_tui_command(&root, &mut pointer, &line, &mut message) {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(err) => message = format!("{err:#}"),
                    }
                }
                crossterm::event::KeyCode::Esc => command.clear(),
                _ => {}
            },
            crossterm::event::Event::Mouse(mouse) => {
                if matches!(
                    mouse.kind,
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
                ) && let Some(target) = link_target_at(&links, mouse.row, mouse.column)
                {
                    let conn = open_db(&root)?;
                    if location_exists(&conn, target)? {
                        pointer = Pointer::Card(target.to_string());
                        message.clear();
                    } else {
                        message = format!("location `{target}` does not exist");
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn is_tui_input_key(key: &crossterm::event::KeyEvent) -> bool {
    matches!(
        key.kind,
        crossterm::event::KeyEventKind::Press | crossterm::event::KeyEventKind::Repeat
    )
}

fn link_target_at(links: &[LinkSpan], row: u16, column: u16) -> Option<&str> {
    links
        .iter()
        .find(|span| span.row == row && column >= span.start && column < span.end)
        .map(|span| span.target.as_str())
}

fn draw_tui(root: &Path, pointer: &Pointer, command: &str, message: &str) -> Result<Vec<LinkSpan>> {
    let conn = open_db(root)?;
    let mut stdout = io::stdout();
    let mut links = Vec::new();
    let mut row = 0_u16;
    crossterm::queue!(
        stdout,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0)
    )?;
    match pointer {
        Pointer::Root => {
            write_tui_plain_line(&mut stdout, &mut row, "ROOT")?;
            for card in load_cards(&conn)?.into_iter().filter(|card| card.is_topic) {
                let parsed = parse_card_text(&card.text)?;
                write_tui_location_line(
                    &mut stdout,
                    &mut row,
                    &card.location,
                    &format!(" {}", parsed.title),
                    &mut links,
                )?;
            }
        }
        Pointer::Card(location) => {
            let card = load_card(&conn, location)?;
            let parsed = parse_card_text(&card.text)?;
            write_tui_plain_line(
                &mut stdout,
                &mut row,
                &format!("location: {}", card.location),
            )?;
            write_tui_plain_line(&mut stdout, &mut row, &format!("title: {}", parsed.title))?;
            for line in parsed.body.lines() {
                write_tui_link_line(&mut stdout, &conn, &mut row, line, &mut links)?;
            }
            if !parsed.reverse.trim().is_empty() {
                for line in parsed.reverse.lines() {
                    write_tui_link_line(&mut stdout, &conn, &mut row, line, &mut links)?;
                }
            }
            if let Ok(direct) = direct_successor(&card.location)
                && location_exists(&conn, &direct)?
            {
                write_tui_link_line(
                    &mut stdout,
                    &conn,
                    &mut row,
                    &format!("direct: [[{direct}]]"),
                    &mut links,
                )?;
            }
            for side in side_successors(&conn, &card.location)? {
                write_tui_link_line(
                    &mut stdout,
                    &conn,
                    &mut row,
                    &format!("side: [[{side}]]"),
                    &mut links,
                )?;
            }
        }
    }
    row = row.saturating_add(1);
    if !message.is_empty() {
        write_tui_plain_line(&mut stdout, &mut row, message)?;
    }
    write_tui_plain_line(&mut stdout, &mut row, &format!("zt> {command}"))?;
    stdout.flush()?;
    Ok(links)
}

fn write_tui_plain_line<W: Write>(stdout: &mut W, row: &mut u16, text: &str) -> Result<()> {
    crossterm::queue!(
        stdout,
        crossterm::cursor::MoveTo(0, *row),
        crossterm::style::Print(text)
    )?;
    *row = row.saturating_add(1);
    Ok(())
}

fn write_tui_location_line<W: Write>(
    stdout: &mut W,
    row: &mut u16,
    location: &str,
    suffix: &str,
    links: &mut Vec<LinkSpan>,
) -> Result<()> {
    crossterm::queue!(stdout, crossterm::cursor::MoveTo(0, *row))?;
    queue_tui_valid_link(stdout, *row, 0, location, location, links)?;
    crossterm::queue!(stdout, crossterm::style::Print(suffix))?;
    *row = row.saturating_add(1);
    Ok(())
}

fn write_tui_link_line<W: Write>(
    stdout: &mut W,
    conn: &Connection,
    row: &mut u16,
    text: &str,
    links: &mut Vec<LinkSpan>,
) -> Result<()> {
    crossterm::queue!(stdout, crossterm::cursor::MoveTo(0, *row))?;
    let mut col = 0_u16;
    let mut offset = 0_usize;
    while let Some(start_rel) = text[offset..].find("[[") {
        let start = offset + start_rel;
        let plain = &text[offset..start];
        crossterm::queue!(stdout, crossterm::style::Print(plain))?;
        col = col.saturating_add(plain.chars().count() as u16);
        let content_start = start + 2;
        let Some(end_rel) = text[content_start..].find("]]") else {
            let rest = &text[start..];
            crossterm::queue!(stdout, crossterm::style::Print(rest))?;
            *row = row.saturating_add(1);
            return Ok(());
        };
        let end = content_start + end_rel;
        let target = &text[content_start..end];
        let macro_text = &text[start..end + 2];
        if is_valid_location(target) && location_exists(conn, target)? {
            queue_tui_valid_link(stdout, *row, col, macro_text, target, links)?;
        } else {
            crossterm::queue!(
                stdout,
                crossterm::style::SetForegroundColor(crossterm::style::Color::DarkRed),
                crossterm::style::SetAttribute(crossterm::style::Attribute::Underlined),
                crossterm::style::Print(macro_text),
                crossterm::style::SetAttribute(crossterm::style::Attribute::NoUnderline),
                crossterm::style::ResetColor
            )?;
        }
        col = col.saturating_add(macro_text.chars().count() as u16);
        offset = end + 2;
    }
    let rest = &text[offset..];
    crossterm::queue!(stdout, crossterm::style::Print(rest))?;
    *row = row.saturating_add(1);
    Ok(())
}

fn queue_tui_valid_link<W: Write>(
    stdout: &mut W,
    row: u16,
    col: u16,
    text: &str,
    target: &str,
    links: &mut Vec<LinkSpan>,
) -> Result<()> {
    crossterm::queue!(
        stdout,
        crossterm::style::SetForegroundColor(crossterm::style::Color::Blue),
        crossterm::style::SetAttribute(crossterm::style::Attribute::Underlined),
        crossterm::style::Print(text),
        crossterm::style::SetAttribute(crossterm::style::Attribute::NoUnderline),
        crossterm::style::ResetColor
    )?;
    links.push(LinkSpan {
        row,
        start: col,
        end: col.saturating_add(text.chars().count() as u16),
        target: target.to_string(),
    });
    Ok(())
}

fn handle_tui_command(
    root: &Path,
    pointer: &mut Pointer,
    line: &str,
    message: &mut String,
) -> Result<bool> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(true);
    }
    if parts.first() != Some(&"zt") {
        bail!("session commands must start with `zt`");
    }
    match parts.as_slice() {
        ["zt", "q"] => Ok(false),
        ["zt", "root"] => {
            *pointer = Pointer::Root;
            message.clear();
            Ok(true)
        }
        ["zt", "go", location] => {
            let conn = open_db(root)?;
            if !location_exists(&conn, location)? {
                bail!("location `{location}` does not exist");
            }
            *pointer = Pointer::Card((*location).to_string());
            message.clear();
            Ok(true)
        }
        ["zt", "ls"] => {
            *message = tui_ls(root, pointer)?;
            Ok(true)
        }
        ["zt", "stats"] => {
            let counts = card_counts(root)?;
            *message = format!(
                "total: {} | topics: {} | regular: {}",
                counts.total, counts.topics, counts.regular
            );
            Ok(true)
        }
        ["zt", "status"] => {
            *message = format!("state: up | sessions: {}", session_count(root));
            Ok(true)
        }
        ["zt", "lsbk"] => {
            *message = tui_lsbk(root)?;
            Ok(true)
        }
        ["zt", "help"] => {
            *message = "zt root | zt go <location> | zt ls | zt t <title> | zt n | zt b | zt e | zt del | zt mv <new-location> | zt stats | zt status | zt lsbk | zt q".to_string();
            Ok(true)
        }
        ["zt", "t", title @ ..] => {
            tui_create_topic(root, pointer, &title.join(" "))?;
            message.clear();
            Ok(true)
        }
        ["zt", "n"] => {
            let at = current_card(pointer)?;
            tui_create_direct(root, pointer, &at)?;
            message.clear();
            Ok(true)
        }
        ["zt", "b"] => {
            let at = current_card(pointer)?;
            tui_create_side(root, pointer, &at)?;
            message.clear();
            Ok(true)
        }
        ["zt", "e"] => {
            let at = current_card(pointer)?;
            tui_edit_card(root, &at)?;
            message.clear();
            Ok(true)
        }
        ["zt", "del"] => {
            let at = current_card(pointer)?;
            let _lock = acquire_edit_lock(root)?;
            let mut conn = open_db(root)?;
            delete_card(&mut conn, &at, tui_read_confirmation)?;
            *pointer = parent_location(&at)
                .map(Pointer::Card)
                .unwrap_or(Pointer::Root);
            message.clear();
            Ok(true)
        }
        ["zt", "mv", new_location] => {
            let at = current_card(pointer)?;
            let _lock = acquire_edit_lock(root)?;
            let mut conn = open_db(root)?;
            move_card(&mut conn, &at, new_location, tui_read_confirmation)?;
            *pointer = Pointer::Card((*new_location).to_string());
            message.clear();
            Ok(true)
        }
        _ => bail!("unknown session command"),
    }
}

fn tui_create_topic(root: &Path, pointer: &mut Pointer, title: &str) -> Result<()> {
    if title.trim().is_empty() || title.contains('\n') || title.contains('\r') {
        bail!("topic title must be non-empty single-line text");
    }
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let location = next_topic_location(&conn)?;
    if tui_edit_until_saved(&topic_template(title), |text| {
        insert_card(&mut conn, &location, true, text)?;
        bump_next_topic_id(&conn, &location)?;
        Ok(())
    })? {
        *pointer = Pointer::Card(location);
    }
    Ok(())
}

fn tui_create_direct(root: &Path, pointer: &mut Pointer, at: &str) -> Result<()> {
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let parent = load_card(&conn, at)?;
    let location = direct_successor(&parent.location)?;
    if location_exists(&conn, &location)? {
        bail!("direct successor already exists: {location}");
    }
    if tui_edit_until_saved(&regular_template(), |text| {
        insert_card(&mut conn, &location, false, text)
    })? {
        *pointer = Pointer::Card(location);
    }
    Ok(())
}

fn tui_create_side(root: &Path, pointer: &mut Pointer, at: &str) -> Result<()> {
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let parent = load_card(&conn, at)?;
    if parent.is_topic {
        bail!("zt b is not valid on a topic card");
    }
    let location = next_side_successor(&conn, &parent.location, &BTreeSet::new())?;
    if tui_edit_until_saved(&regular_template(), |text| {
        insert_card(&mut conn, &location, false, text)
    })? {
        *pointer = Pointer::Card(location);
    }
    Ok(())
}

fn tui_edit_card(root: &Path, at: &str) -> Result<()> {
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let card = load_card(&conn, at)?;
    tui_edit_until_saved(&card.text, |text| update_card_text(&mut conn, &card, text))?;
    Ok(())
}

fn tui_edit_until_saved<F>(initial_text: &str, mut save: F) -> Result<bool>
where
    F: FnMut(&str) -> Result<()>,
{
    let mut current = initial_text.to_string();
    let mut error = String::new();
    loop {
        match run_tui_editor(&current, &error)? {
            EditOutcome::Canceled => return Ok(false),
            EditOutcome::Saved(text) => match save(&text) {
                Ok(()) => return Ok(true),
                Err(err) => {
                    current = text;
                    error = format!("{err:#}");
                }
            },
        }
    }
}

fn run_tui_editor(initial_text: &str, error: &str) -> Result<EditOutcome> {
    let mut buffer = initial_text.to_string();
    loop {
        let mut stdout = io::stdout();
        crossterm::queue!(
            stdout,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::cursor::MoveTo(0, 0),
            crossterm::style::Print("edit mode: Ctrl+S save, Esc cancel\n")
        )?;
        if !error.is_empty() {
            crossterm::queue!(
                stdout,
                crossterm::style::SetForegroundColor(crossterm::style::Color::DarkRed),
                crossterm::style::Print(error),
                crossterm::style::ResetColor,
                crossterm::style::Print("\n")
            )?;
        }
        crossterm::queue!(stdout, crossterm::style::Print(&buffer))?;
        stdout.flush()?;
        if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
            if !is_tui_input_key(&key) {
                continue;
            }
            match key.code {
                crossterm::event::KeyCode::Char('s')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    return Ok(EditOutcome::Saved(buffer));
                }
                crossterm::event::KeyCode::Esc => return Ok(EditOutcome::Canceled),
                crossterm::event::KeyCode::Enter => buffer.push('\n'),
                crossterm::event::KeyCode::Backspace => {
                    buffer.pop();
                }
                crossterm::event::KeyCode::Char(ch) => buffer.push(ch),
                _ => {}
            }
        }
    }
}

fn tui_read_confirmation(expected: &str) -> Result<()> {
    let mut input = String::new();
    loop {
        let mut stdout = io::stdout();
        crossterm::queue!(
            stdout,
            crossterm::style::Print(format!("\ntype `{expected}` to confirm: {input}"))
        )?;
        stdout.flush()?;
        if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
            if !is_tui_input_key(&key) {
                continue;
            }
            match key.code {
                crossterm::event::KeyCode::Enter => {
                    if input.trim() == expected {
                        return Ok(());
                    }
                    bail!("confirmation did not match");
                }
                crossterm::event::KeyCode::Esc => bail!("confirmation did not match"),
                crossterm::event::KeyCode::Backspace => {
                    input.pop();
                }
                crossterm::event::KeyCode::Char(ch) => input.push(ch),
                _ => {}
            }
        }
    }
}

fn tui_ls(root: &Path, pointer: &Pointer) -> Result<String> {
    let conn = open_db(root)?;
    let topic = match pointer {
        Pointer::Root => None,
        Pointer::Card(location) => Some(topic_id(location)?.to_string()),
    };
    let mut lines = Vec::new();
    for card in load_cards(&conn)? {
        if let Some(topic) = &topic
            && !card.location.starts_with(&format!("{topic}/"))
        {
            continue;
        }
        let parsed = parse_card_text(&card.text)?;
        lines.push(format!("{} {}", card.location, parsed.title));
    }
    Ok(lines.join(" | "))
}

fn tui_lsbk(root: &Path) -> Result<String> {
    let conn = open_db(root)?;
    let mut lines = Vec::new();
    for card in load_cards(&conn)? {
        let parsed = parse_card_text(&card.text)?;
        for line in parsed.body.lines() {
            for target in extract_link_locations(line)? {
                if !location_exists(&conn, &target)? {
                    lines.push(format!(
                        "{} {} -> {}: {}",
                        card.location, parsed.title, target, line
                    ));
                }
            }
        }
    }
    if lines.is_empty() {
        Ok("no broken links".to_string())
    } else {
        Ok(lines.join(" | "))
    }
}

fn handle_session_command<R: BufRead>(
    root: &Path,
    pointer: &mut Pointer,
    line: &str,
    input: &mut R,
) -> Result<bool> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(true);
    }
    if parts.first() != Some(&"zt") {
        bail!("session commands must start with `zt`");
    }
    match parts.as_slice() {
        ["zt", "q"] => Ok(false),
        ["zt", "root"] => {
            *pointer = Pointer::Root;
            Ok(true)
        }
        ["zt", "go", location] => {
            let conn = open_db(root)?;
            if !location_exists(&conn, location)? {
                bail!("location `{location}` does not exist");
            }
            *pointer = Pointer::Card((*location).to_string());
            Ok(true)
        }
        ["zt", "ls"] => {
            session_ls(root, pointer)?;
            Ok(true)
        }
        ["zt", "stats"] => {
            cmd_stats()?;
            Ok(true)
        }
        ["zt", "status"] => {
            cmd_status()?;
            Ok(true)
        }
        ["zt", "lsbk"] => {
            cmd_lsbk()?;
            Ok(true)
        }
        ["zt", "help"] => {
            cmd_help()?;
            Ok(true)
        }
        ["zt", "t", title @ ..] => {
            let title = title.join(" ");
            if title.trim().is_empty() || title.contains('\n') || title.contains('\r') {
                bail!("topic title must be non-empty single-line text");
            }
            let _lock = acquire_edit_lock(root)?;
            let mut conn = open_db(root)?;
            let location = next_topic_location(&conn)?;
            if session_edit_until_saved(input, &topic_template(&title), |text| {
                insert_card(&mut conn, &location, true, text)?;
                bump_next_topic_id(&conn, &location)?;
                Ok(())
            })? {
                *pointer = Pointer::Card(location);
            }
            Ok(true)
        }
        ["zt", "n"] => {
            let at = current_card(pointer)?;
            shell_like_session_create_direct(root, &at, pointer, input)?;
            Ok(true)
        }
        ["zt", "b"] => {
            let at = current_card(pointer)?;
            shell_like_session_create_side(root, &at, pointer, input)?;
            Ok(true)
        }
        ["zt", "e"] => {
            let at = current_card(pointer)?;
            let _lock = acquire_edit_lock(root)?;
            let mut conn = open_db(root)?;
            let card = load_card(&conn, &at)?;
            session_edit_until_saved(input, &card.text, |text| {
                update_card_text(&mut conn, &card, text)
            })?;
            Ok(true)
        }
        ["zt", "del"] => {
            let at = current_card(pointer)?;
            let _lock = acquire_edit_lock(root)?;
            let mut conn = open_db(root)?;
            delete_card(&mut conn, &at, |expected| {
                read_confirmation(input, expected)
            })?;
            *pointer = parent_location(&at)
                .map(Pointer::Card)
                .unwrap_or(Pointer::Root);
            Ok(true)
        }
        ["zt", "mv", new_location] => {
            let at = current_card(pointer)?;
            let _lock = acquire_edit_lock(root)?;
            let mut conn = open_db(root)?;
            move_card(&mut conn, &at, new_location, |expected| {
                read_confirmation(input, expected)
            })?;
            *pointer = Pointer::Card((*new_location).to_string());
            Ok(true)
        }
        _ => bail!("unknown session command"),
    }
}

fn current_card(pointer: &Pointer) -> Result<String> {
    match pointer {
        Pointer::Root => bail!("pointer is on ROOT"),
        Pointer::Card(location) => Ok(location.clone()),
    }
}

fn shell_like_session_create_direct<R: BufRead>(
    root: &Path,
    at: &str,
    pointer: &mut Pointer,
    input: &mut R,
) -> Result<()> {
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let parent = load_card(&conn, at)?;
    let location = direct_successor(&parent.location)?;
    if location_exists(&conn, &location)? {
        bail!("direct successor already exists: {location}");
    }
    if session_edit_until_saved(input, &regular_template(), |text| {
        insert_card(&mut conn, &location, false, text)
    })? {
        *pointer = Pointer::Card(location);
    }
    Ok(())
}

fn shell_like_session_create_side<R: BufRead>(
    root: &Path,
    at: &str,
    pointer: &mut Pointer,
    input: &mut R,
) -> Result<()> {
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let parent = load_card(&conn, at)?;
    if parent.is_topic {
        bail!("zt b is not valid on a topic card");
    }
    let location = next_side_successor(&conn, &parent.location, &BTreeSet::new())?;
    if session_edit_until_saved(input, &regular_template(), |text| {
        insert_card(&mut conn, &location, false, text)
    })? {
        *pointer = Pointer::Card(location);
    }
    Ok(())
}

fn print_view(root: &Path, pointer: &Pointer) -> Result<()> {
    let conn = open_db(root)?;
    match pointer {
        Pointer::Root => {
            println!("ROOT");
            for card in load_cards(&conn)?.into_iter().filter(|card| card.is_topic) {
                let parsed = parse_card_text(&card.text)?;
                println!("{} {}", card.location, parsed.title);
            }
        }
        Pointer::Card(location) => {
            let card = load_card(&conn, location)?;
            let parsed = parse_card_text(&card.text)?;
            println!("location: {}", card.location);
            println!("title: {}", parsed.title);
            println!("{}", parsed.body);
            if !parsed.reverse.trim().is_empty() {
                println!("{}", parsed.reverse);
            }
            if let Ok(direct) = direct_successor(&card.location)
                && location_exists(&conn, &direct)?
            {
                println!("direct: [[{direct}]]");
            }
            for side in side_successors(&conn, &card.location)? {
                println!("side: [[{side}]]");
            }
        }
    }
    Ok(())
}

fn side_successors(conn: &Connection, parent: &str) -> Result<Vec<String>> {
    let mut by_label = BTreeMap::new();
    for card in load_cards(conn)? {
        if let Some(label) = immediate_side_label(&card.location, parent) {
            by_label
                .entry(side_label_to_number(&label))
                .or_insert(card.location);
        }
    }
    Ok(by_label.into_values().collect())
}

fn session_ls(root: &Path, pointer: &Pointer) -> Result<()> {
    let conn = open_db(root)?;
    let topic = match pointer {
        Pointer::Root => None,
        Pointer::Card(location) => Some(topic_id(location)?.to_string()),
    };
    for card in load_cards(&conn)? {
        if let Some(topic) = &topic
            && !card.location.starts_with(&format!("{topic}/"))
        {
            continue;
        }
        let parsed = parse_card_text(&card.text)?;
        println!("{} {}", card.location, parsed.title);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tui_key_filter_ignores_release_events() {
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        let mut key = KeyEvent {
            code: KeyCode::Char('z'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        assert!(is_tui_input_key(&key));

        key.kind = KeyEventKind::Repeat;
        assert!(is_tui_input_key(&key));

        key.kind = KeyEventKind::Release;
        assert!(!is_tui_input_key(&key));
    }

    #[test]
    fn link_target_hit_testing_uses_half_open_spans() {
        let links = vec![LinkSpan {
            row: 2,
            start: 5,
            end: 12,
            target: "0/1|a".to_string(),
        }];

        assert_eq!(link_target_at(&links, 2, 4), None);
        assert_eq!(link_target_at(&links, 2, 5), Some("0/1|a"));
        assert_eq!(link_target_at(&links, 2, 11), Some("0/1|a"));
        assert_eq!(link_target_at(&links, 2, 12), None);
        assert_eq!(link_target_at(&links, 3, 6), None);
    }
}
