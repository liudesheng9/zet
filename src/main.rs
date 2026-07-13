use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod dump;
mod session;

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
        [] => {
            let root = require_service_up()?;
            session::run_session(root)
        }
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
        [cmd] if cmd == "dp" => cmd_dump(),
        [cmd] if cmd == "clear" => cmd_clear(),
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
    let sessions = session::session_count(&root);
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
    println!("sessions: {}", session::session_count(&root));
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

fn cmd_dump() -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let conn = open_db(&root)?;
    let cards = load_cards(&conn)?
        .into_iter()
        .map(|card| dump::DumpCard::new(card.location, card.text))
        .collect::<Vec<_>>();
    dump::run(&root, &cards)
}

fn cmd_clear() -> Result<()> {
    let root = require_service_up()?;
    let sessions = session::session_count(&root);
    if sessions > 0 {
        bail!("cannot clear while {sessions} session(s) are open");
    }
    let _lock = acquire_edit_lock(&root)?;
    print!("type clear to confirm: ");
    io::stdout().flush()?;
    let mut confirmation = String::new();
    io::stdin().read_line(&mut confirmation)?;
    if confirmation.trim_end_matches(['\r', '\n']) != "clear" {
        println!("clear canceled");
        return Ok(());
    }
    let mut conn = open_db(&root)?;
    let tx = conn.transaction()?;
    let count: i64 = tx.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
    tx.execute("DELETE FROM cards", [])?;
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('next_topic_id', '0')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [],
    )?;
    tx.commit()?;
    println!("cleared {count} cards; next topic id reset to 0");
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
        println!("  zt dp");
        println!("  zt clear");
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

struct DeletePlan {
    delete_set: BTreeSet<String>,
    confirmation: String,
}

fn delete_plan(conn: &Connection, at: &str) -> Result<DeletePlan> {
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
    let confirmation = if card.is_topic {
        at.to_string()
    } else {
        "delete".to_string()
    };
    Ok(DeletePlan {
        delete_set,
        confirmation,
    })
}

fn delete_verification_lines(plan: &DeletePlan) -> Vec<String> {
    let mut lines = vec!["delete verification:".to_string()];
    for location in &plan.delete_set {
        lines.push(format!("delete {location}"));
    }
    lines.push(format!(
        "successors: {}",
        plan.delete_set.len().saturating_sub(1)
    ));
    lines
}

fn print_delete_verification(plan: &DeletePlan) {
    for line in delete_verification_lines(plan) {
        println!("{line}");
    }
}

fn apply_delete_plan(conn: &mut Connection, at: &str, plan: DeletePlan) -> Result<DeleteResult> {
    let compaction = side_compaction_mapping(conn, at, &plan.delete_set)?;
    let tx = conn.transaction()?;
    for location in &plan.delete_set {
        tx.execute("DELETE FROM cards WHERE location = ?1", [location])?;
    }
    apply_location_mapping_tx(&tx, &compaction)?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(DeleteResult {
        deleted_count: plan.delete_set.len(),
        moved: compaction,
    })
}

fn delete_card<F>(conn: &mut Connection, at: &str, mut confirm: F) -> Result<DeleteResult>
where
    F: FnMut(&str) -> Result<()>,
{
    let plan = delete_plan(conn, at)?;
    print_delete_verification(&plan);
    confirm(&plan.confirmation)?;
    apply_delete_plan(conn, at, plan)
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

struct MovePlan {
    mapping: BTreeMap<String, String>,
    rewritten_links: usize,
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
    let plan = move_plan(conn, at, new_location)?;
    print_move_verification(&plan);
    confirm("move")?;
    let mapping = plan.mapping.clone();
    apply_move_plan(conn, &plan)?;
    Ok(mapping)
}

fn move_plan(conn: &Connection, at: &str, new_location: &str) -> Result<MovePlan> {
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
    let rewritten_links = count_rewritten_links(conn, &mapping)?;
    Ok(MovePlan {
        mapping,
        rewritten_links,
    })
}

fn move_verification_lines(plan: &MovePlan) -> Vec<String> {
    let mut lines = vec!["move verification:".to_string()];
    for (old, new) in &plan.mapping {
        lines.push(format!("{old} -> {new}"));
    }
    lines.push(format!("moved cards: {}", plan.mapping.len()));
    lines.push(format!("link macros rewritten: {}", plan.rewritten_links));
    lines
}

fn print_move_verification(plan: &MovePlan) {
    for line in move_verification_lines(plan) {
        println!("{line}");
    }
}

fn apply_move_plan(conn: &mut Connection, plan: &MovePlan) -> Result<()> {
    let tx = conn.transaction()?;
    apply_location_mapping_tx(&tx, &plan.mapping)?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(())
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
