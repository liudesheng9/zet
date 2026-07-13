use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::*;

pub(crate) fn run_session(root: PathBuf) -> Result<()> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        return cmd_tui_session(root);
    }
    cmd_line_session(root)
}

fn run_session_editor_with_header<R: BufRead>(
    input: &mut R,
    initial_text: &str,
    header: &str,
) -> Result<EditOutcome> {
    println!("{header}");
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

fn session_edit_until_saved<R, F>(input: &mut R, initial_text: &str, save: F) -> Result<bool>
where
    R: BufRead,
    F: FnMut(&str) -> Result<()>,
{
    session_edit_until_saved_with_header(input, initial_text, "edit mode", save)
}

fn session_edit_until_saved_with_header<R, F>(
    input: &mut R,
    initial_text: &str,
    header: &str,
    mut save: F,
) -> Result<bool>
where
    R: BufRead,
    F: FnMut(&str) -> Result<()>,
{
    let mut current = initial_text.to_string();
    loop {
        match run_session_editor_with_header(input, &current, header)? {
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

pub(crate) fn session_count(root: &Path) -> i64 {
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
        crossterm::execute!(
            io::stdout(),
            crossterm::event::DisableMouseCapture,
            crossterm::terminal::LeaveAlternateScreen
        )
        .ok();
        // ConPTY needs cooked input restored after the terminal UI has been left.
        crossterm::terminal::disable_raw_mode().ok();
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
    let mut links = draw_tui(&root, &pointer, &command, &message)?;
    loop {
        if !is_service_up(&root) {
            message = "service disconnected".to_string();
            draw_tui(&root, &pointer, &command, &message)?;
            break;
        }
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
                    if target_exists(&conn, target)? {
                        pointer = Pointer::Card(target.to_string());
                        message.clear();
                    } else {
                        message = format!("target `{target}` does not exist");
                    }
                } else {
                    continue;
                }
            }
            _ => continue,
        }
        links = draw_tui(&root, &pointer, &command, &message)?;
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
                    &card.address,
                    &format!(" {}", parsed.title),
                    &mut links,
                )?;
            }
        }
        Pointer::Card(location) => {
            let card = load_card(&conn, location)?;
            let parsed = parse_card_text(&card.text)?;
            if card.is_lit {
                write_tui_plain_line(
                    &mut stdout,
                    &mut row,
                    &format!(
                        "citation key: {}",
                        card.citation_key
                            .as_deref()
                            .context("Literature Card is missing its Citation key")?
                    ),
                )?;
            } else {
                write_tui_plain_line(
                    &mut stdout,
                    &mut row,
                    &format!("location: {}", card.address),
                )?;
            }
            write_tui_plain_line(&mut stdout, &mut row, &format!("title: {}", parsed.title))?;
            if card.is_lit {
                write_tui_plain_line(&mut stdout, &mut row, "metadata:")?;
                for line in card
                    .bibtex
                    .as_deref()
                    .context("Literature Card is missing its BibTeX metadata")?
                    .lines()
                {
                    write_tui_plain_line(&mut stdout, &mut row, line)?;
                }
            }
            for line in parsed.body.lines() {
                write_tui_link_line(&mut stdout, &conn, &mut row, line, &mut links)?;
            }
            if !parsed.reverse.trim().is_empty() {
                for line in parsed.reverse.lines() {
                    write_tui_link_line(&mut stdout, &conn, &mut row, line, &mut links)?;
                }
            }
            if !card.is_lit
                && let Ok(direct) = direct_successor(&card.address)
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
            if !card.is_lit {
                for side in side_successors(&conn, &card.address)? {
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
    }
    row = row.saturating_add(1);
    if !message.is_empty() {
        write_tui_plain_line(&mut stdout, &mut row, message)?;
    }
    write_tui_plain_line(&mut stdout, &mut row, &format!("> {command}"))?;
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
        if (is_valid_location(target) || literature::validate_citation_key(target).is_ok())
            && target_exists(conn, target)?
        {
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

fn normalize_session_command(line: &str) -> Option<Vec<&str>> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let mut parts = Vec::with_capacity(tokens.len() + 1);
    parts.push("zt");
    parts.extend(tokens);
    Some(parts)
}

fn unknown_session_command(parts: &[&str]) -> String {
    format!(
        "unknown session command: {}",
        parts.get(1).copied().unwrap_or_default()
    )
}

fn session_help_text() -> &'static str {
    "root | go <target> | ls | t <title> | l | n | b | e | del | mv <new-location> | stats | status | lsbk | q"
}

fn handle_tui_command(
    root: &Path,
    pointer: &mut Pointer,
    line: &str,
    message: &mut String,
) -> Result<bool> {
    let Some(parts) = normalize_session_command(line) else {
        return Ok(true);
    };
    match parts.as_slice() {
        ["zt", "q"] => Ok(false),
        ["zt", "root"] => {
            *pointer = Pointer::Root;
            message.clear();
            Ok(true)
        }
        ["zt", "go", target] => {
            let conn = open_db(root)?;
            if !target_exists(&conn, target)? {
                bail!("target `{target}` does not exist");
            }
            *pointer = Pointer::Card((*target).to_string());
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
                "total: {} | topics: {} | regular: {} | literature: {}",
                counts.total, counts.topics, counts.regular, counts.literature
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
            *message = session_help_text().to_string();
            Ok(true)
        }
        ["zt", "t", title @ ..] => {
            tui_create_topic(root, pointer, &title.join(" "))?;
            message.clear();
            Ok(true)
        }
        ["zt", "l"] => {
            tui_create_literature(root, pointer)?;
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
            let target = tui_edit_card(root, &at)?;
            *pointer = Pointer::Card(target);
            message.clear();
            Ok(true)
        }
        ["zt", "del"] => {
            let at = current_card(pointer)?;
            let _lock = acquire_edit_lock(root)?;
            let mut conn = open_db(root)?;
            let plan = delete_plan(&conn, &at)?;
            tui_read_confirmation(&plan.confirmation, &delete_verification_lines(&plan))?;
            apply_delete_plan(&mut conn, &at, plan)?;
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
            let plan = move_plan(&conn, &at, new_location)?;
            tui_read_confirmation("move", &move_verification_lines(&plan))?;
            apply_move_plan(&mut conn, &plan)?;
            *pointer = Pointer::Card((*new_location).to_string());
            message.clear();
            Ok(true)
        }
        _ => bail!("{}", unknown_session_command(&parts)),
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

fn tui_create_literature(root: &Path, pointer: &mut Pointer) -> Result<()> {
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let mut accepted = None;
    if !tui_edit_until_saved_with_header("", "metadata edit", |text| {
        let metadata = literature::parse_metadata(text)?;
        if citation_key_exists_case_insensitive(&conn, &metadata.citation_key)? {
            bail!("Literature Card `{}` already exists", metadata.citation_key);
        }
        accepted = Some((text.to_string(), metadata));
        Ok(())
    })? {
        return Ok(());
    }
    let (bibtex, metadata) = accepted.context("validated Literature metadata is missing")?;
    let initial_text = compose_card_text(&metadata.title, "", "");
    let citation_key = metadata.citation_key.clone();
    if tui_edit_until_saved(&initial_text, |edited| {
        let parsed = parse_literature_edit_text(edited)?;
        let text = compose_card_text(&metadata.title, &parsed.body, "");
        insert_literature_card(&mut conn, &citation_key, &bibtex, &text)
    })? {
        *pointer = Pointer::Card(citation_key);
    }
    Ok(())
}

fn tui_create_direct(root: &Path, pointer: &mut Pointer, at: &str) -> Result<()> {
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let parent = load_card(&conn, at)?;
    if parent.is_lit {
        bail!("zt n is not valid on a Literature Card");
    }
    let location = direct_successor(&parent.address)?;
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
    if parent.is_lit {
        bail!("zt b is not valid on a Literature Card");
    }
    if parent.is_topic {
        bail!("zt b is not valid on a topic card");
    }
    let location = next_side_successor(&conn, &parent.address, &BTreeSet::new())?;
    if tui_edit_until_saved(&regular_template(), |text| {
        insert_card(&mut conn, &location, false, text)
    })? {
        *pointer = Pointer::Card(location);
    }
    Ok(())
}

fn tui_edit_card(root: &Path, at: &str) -> Result<String> {
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let card = load_card(&conn, at)?;
    if !card.is_lit {
        tui_edit_until_saved(&card.text, |text| update_card_text(&mut conn, &card, text))?;
        return Ok(card.address);
    }
    match tui_select_literature_edit_part()? {
        EditPart::Metadata => tui_edit_literature_metadata(&mut conn, &card),
        EditPart::Text => {
            tui_edit_until_saved(&card.text, |text| {
                update_literature_text(&mut conn, &card, text)
            })?;
            Ok(card.address)
        }
    }
}

fn tui_edit_literature_metadata(conn: &mut Connection, card: &Card) -> Result<String> {
    let old_key = card
        .citation_key
        .as_deref()
        .context("Literature Card is missing its Citation key")?;
    let initial = card
        .bibtex
        .as_deref()
        .context("Literature Card is missing its BibTeX metadata")?;
    let mut accepted = None;
    if !tui_edit_until_saved_with_header(initial, "metadata edit", |text| {
        let metadata = literature::parse_metadata(text)?;
        if metadata.citation_key != old_key
            && (metadata.citation_key.eq_ignore_ascii_case(old_key)
                || citation_key_conflicts(conn, &metadata.citation_key, card.row_id)?)
        {
            bail!(
                "Citation key `{}` conflicts with an existing Literature Card",
                metadata.citation_key
            );
        }
        accepted = Some((text.to_string(), metadata));
        Ok(())
    })? {
        return Ok(card.address.clone());
    }
    let (bibtex, metadata) = accepted.context("validated Literature metadata is missing")?;
    if metadata.citation_key != old_key {
        let rewritten_links = count_target_links(conn, old_key)?;
        tui_read_confirmation(
            "move",
            &[
                "citation key rename:".to_string(),
                format!("{old_key} -> {}", metadata.citation_key),
                format!("link macros rewritten: {rewritten_links}"),
            ],
        )?;
    }
    update_literature_metadata(conn, card, &metadata.citation_key, &bibtex, &metadata.title)?;
    Ok(metadata.citation_key)
}

fn tui_select_literature_edit_part() -> Result<EditPart> {
    let mut input = String::new();
    let mut error = String::new();
    loop {
        draw_tui_literature_edit_selection(&input, &error)?;
        let crossterm::event::Event::Key(key) = crossterm::event::read()? else {
            continue;
        };
        if !is_tui_input_key(&key) {
            continue;
        }
        match key.code {
            crossterm::event::KeyCode::Char(ch) => input.push(ch),
            crossterm::event::KeyCode::Backspace => {
                input.pop();
            }
            crossterm::event::KeyCode::Enter => match input.as_str() {
                "1" => return Ok(EditPart::Metadata),
                "2" => return Ok(EditPart::Text),
                _ => {
                    error = "invalid edit option".to_string();
                    input.clear();
                }
            },
            _ => {}
        }
    }
}

fn draw_tui_literature_edit_selection(input: &str, error: &str) -> Result<()> {
    let mut stdout = io::stdout();
    crossterm::queue!(
        stdout,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0),
        crossterm::style::Print("edit literature card:\n  1. metadata\n  2. main text\n")
    )?;
    if !error.is_empty() {
        crossterm::queue!(
            stdout,
            crossterm::style::SetForegroundColor(crossterm::style::Color::DarkRed),
            crossterm::style::Print(format!("{error}\n")),
            crossterm::style::ResetColor
        )?;
    }
    crossterm::queue!(
        stdout,
        crossterm::style::Print(format!("select edit: {input}"))
    )?;
    stdout.flush()?;
    Ok(())
}

fn tui_edit_until_saved<F>(initial_text: &str, save: F) -> Result<bool>
where
    F: FnMut(&str) -> Result<()>,
{
    tui_edit_until_saved_with_header(initial_text, "edit mode", save)
}

fn tui_edit_until_saved_with_header<F>(
    initial_text: &str,
    header: &str,
    mut save: F,
) -> Result<bool>
where
    F: FnMut(&str) -> Result<()>,
{
    let mut editor = EditorModel::new(initial_text);
    let mut error = String::new();
    loop {
        match run_tui_editor(&mut editor, &error, header)? {
            EditOutcome::Canceled => return Ok(false),
            EditOutcome::Saved(text) => match save(&text) {
                Ok(()) => return Ok(true),
                Err(err) => {
                    error = format!("{err:#}");
                }
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorAction {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    Home,
    End,
    Insert(char),
    Enter,
    Backspace,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorIntent {
    Edit(EditorAction),
    Save,
    Cancel,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EditorPosition {
    line: usize,
    column: usize,
}

#[cfg(test)]
impl EditorPosition {
    #[cfg(test)]
    fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, Clone, Copy)]
struct ViewportSize {
    rows: usize,
    cols: usize,
}

impl ViewportSize {
    fn new(rows: usize, cols: usize) -> Self {
        Self { rows, cols }
    }
}

struct EditorModel {
    lines: Vec<String>,
    caret_line: usize,
    caret_col: usize,
    desired_col: Option<usize>,
    row_offset: usize,
    col_offset: usize,
}

impl EditorModel {
    fn new(text: &str) -> Self {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines = normalized
            .split('\n')
            .map(str::to_string)
            .collect::<Vec<_>>();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self {
            lines,
            caret_line: 0,
            caret_col: 0,
            desired_col: None,
            row_offset: 0,
            col_offset: 0,
        }
    }

    fn apply(&mut self, action: EditorAction, viewport: ViewportSize) {
        match action {
            EditorAction::MoveLeft => {
                if self.caret_col > 0 {
                    self.caret_col -= 1;
                } else if self.caret_line > 0 {
                    self.caret_line -= 1;
                    self.caret_col = self.current_line_len();
                }
                self.desired_col = None;
            }
            EditorAction::MoveRight => {
                if self.caret_col < self.current_line_len() {
                    self.caret_col += 1;
                } else if self.caret_line + 1 < self.lines.len() {
                    self.caret_line += 1;
                    self.caret_col = 0;
                }
                self.desired_col = None;
            }
            EditorAction::MoveUp => {
                if self.caret_line > 0 {
                    let desired = self.desired_col.unwrap_or(self.caret_col);
                    self.caret_line -= 1;
                    self.caret_col = desired.min(self.current_line_len());
                    self.desired_col = Some(desired);
                }
            }
            EditorAction::End => {
                self.caret_col = self.current_line_len();
                self.desired_col = None;
            }
            EditorAction::Home => {
                self.caret_col = 0;
                self.desired_col = None;
            }
            EditorAction::MoveDown => {
                if self.caret_line + 1 < self.lines.len() {
                    let desired = self.desired_col.unwrap_or(self.caret_col);
                    self.caret_line += 1;
                    self.caret_col = desired.min(self.current_line_len());
                    self.desired_col = Some(desired);
                }
            }
            EditorAction::Insert(ch) => {
                let col = self.caret_col;
                let line = &mut self.lines[self.caret_line];
                let byte = byte_index_for_char_col(line, col);
                line.insert(byte, ch);
                self.caret_col += 1;
                self.desired_col = None;
            }
            EditorAction::Enter => {
                let col = self.caret_col;
                let line = &mut self.lines[self.caret_line];
                let byte = byte_index_for_char_col(line, col);
                let right = line.split_off(byte);
                self.caret_line += 1;
                self.caret_col = 0;
                self.lines.insert(self.caret_line, right);
                self.desired_col = None;
            }
            EditorAction::Backspace => {
                if self.caret_col > 0 {
                    let col = self.caret_col;
                    let line = &mut self.lines[self.caret_line];
                    let start = byte_index_for_char_col(line, col - 1);
                    let end = byte_index_for_char_col(line, col);
                    line.replace_range(start..end, "");
                    self.caret_col -= 1;
                } else if self.caret_line > 0 {
                    let current = self.lines.remove(self.caret_line);
                    self.caret_line -= 1;
                    self.caret_col = self.current_line_len();
                    self.lines[self.caret_line].push_str(&current);
                }
                self.desired_col = None;
            }
            EditorAction::Delete => {
                if self.caret_col < self.current_line_len() {
                    let col = self.caret_col;
                    let line = &mut self.lines[self.caret_line];
                    let start = byte_index_for_char_col(line, col);
                    let end = byte_index_for_char_col(line, col + 1);
                    line.replace_range(start..end, "");
                } else if self.caret_line + 1 < self.lines.len() {
                    let next = self.lines.remove(self.caret_line + 1);
                    self.lines[self.caret_line].push_str(&next);
                }
                self.desired_col = None;
            }
        }
        self.ensure_visible(viewport);
    }

    #[cfg(test)]
    fn position(&self) -> EditorPosition {
        EditorPosition {
            line: self.caret_line + 1,
            column: self.caret_col + 1,
        }
    }

    #[cfg(test)]
    fn viewport_offsets(&self) -> (usize, usize) {
        (self.row_offset, self.col_offset)
    }

    fn current_line_len(&self) -> usize {
        self.lines[self.caret_line].chars().count()
    }

    fn text(&self) -> String {
        self.lines.join("\n")
    }

    fn ensure_visible(&mut self, viewport: ViewportSize) {
        if self.caret_line < self.row_offset {
            self.row_offset = self.caret_line;
        } else if self.caret_line >= self.row_offset.saturating_add(viewport.rows) {
            self.row_offset = self.caret_line + 1 - viewport.rows;
        }

        let caret_display_col =
            display_width_to_char_col(&self.lines[self.caret_line], self.caret_col);
        if caret_display_col < self.col_offset {
            self.col_offset = caret_display_col;
        } else if caret_display_col >= self.col_offset.saturating_add(viewport.cols) {
            self.col_offset = caret_display_col + 1 - viewport.cols;
        }
    }

    fn line_column_label(&self) -> String {
        format!("Ln {}, Col {}", self.caret_line + 1, self.caret_col + 1)
    }
}

fn byte_index_for_char_col(text: &str, col: usize) -> usize {
    text.char_indices()
        .nth(col)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

fn display_width_to_char_col(text: &str, col: usize) -> usize {
    text.chars().take(col).map(char_display_width).sum()
}

fn char_display_width(ch: char) -> usize {
    match ch {
        '\t' => 4,
        ch if ch.is_control() => 0,
        ch if ch.is_ascii() => 1,
        _ => 2,
    }
}

fn run_tui_editor(editor: &mut EditorModel, error: &str, header: &str) -> Result<EditOutcome> {
    loop {
        let viewport = draw_tui_editor(editor, error, header)?;
        if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
            if !is_tui_input_key(&key) {
                continue;
            }
            match editor_intent_from_key(key) {
                Some(EditorIntent::Save) => return Ok(EditOutcome::Saved(editor.text())),
                Some(EditorIntent::Cancel) => return Ok(EditOutcome::Canceled),
                Some(EditorIntent::Edit(action)) => editor.apply(action, viewport),
                None => {}
            }
        }
    }
}

fn editor_intent_from_key(key: crossterm::event::KeyEvent) -> Option<EditorIntent> {
    if key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        return match key.code {
            crossterm::event::KeyCode::Char('s') => Some(EditorIntent::Save),
            crossterm::event::KeyCode::Char('c') => Some(EditorIntent::Cancel),
            _ => None,
        };
    }
    if key.code == crossterm::event::KeyCode::Esc {
        return Some(EditorIntent::Cancel);
    }
    editor_action_from_key_code(key.code).map(EditorIntent::Edit)
}

fn editor_action_from_key_code(code: crossterm::event::KeyCode) -> Option<EditorAction> {
    match code {
        crossterm::event::KeyCode::Left => Some(EditorAction::MoveLeft),
        crossterm::event::KeyCode::Right => Some(EditorAction::MoveRight),
        crossterm::event::KeyCode::Up => Some(EditorAction::MoveUp),
        crossterm::event::KeyCode::Down => Some(EditorAction::MoveDown),
        crossterm::event::KeyCode::Home => Some(EditorAction::Home),
        crossterm::event::KeyCode::End => Some(EditorAction::End),
        crossterm::event::KeyCode::Enter => Some(EditorAction::Enter),
        crossterm::event::KeyCode::Backspace => Some(EditorAction::Backspace),
        crossterm::event::KeyCode::Delete => Some(EditorAction::Delete),
        crossterm::event::KeyCode::Tab => Some(EditorAction::Insert('\t')),
        crossterm::event::KeyCode::Char(ch) => Some(EditorAction::Insert(ch)),
        crossterm::event::KeyCode::Esc => None,
        _ => None,
    }
}

fn draw_tui_editor(editor: &mut EditorModel, error: &str, header: &str) -> Result<ViewportSize> {
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let viewport = ViewportSize::new((rows as usize).saturating_sub(3).max(1), cols as usize);
    editor.ensure_visible(viewport);

    let mut stdout = io::stdout();
    crossterm::queue!(
        stdout,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0),
        crossterm::style::Print(format!("{header}: Ctrl+S save, Esc cancel"))
    )?;
    crossterm::queue!(stdout, crossterm::cursor::MoveTo(0, 1))?;
    if !error.is_empty() {
        crossterm::queue!(
            stdout,
            crossterm::style::SetForegroundColor(crossterm::style::Color::DarkRed),
            crossterm::style::Print(error),
            crossterm::style::ResetColor
        )?;
    }

    for row in 0..viewport.rows {
        let line_index = editor.row_offset + row;
        crossterm::queue!(stdout, crossterm::cursor::MoveTo(0, (row + 2) as u16))?;
        if let Some(line) = editor.lines.get(line_index) {
            crossterm::queue!(
                stdout,
                crossterm::style::Print(render_editor_line(line, editor.col_offset, viewport.cols))
            )?;
        }
    }

    let footer_row = rows.saturating_sub(1);
    crossterm::queue!(
        stdout,
        crossterm::cursor::MoveTo(0, footer_row),
        crossterm::style::Print(format!(
            "Ctrl+S Save | Esc Cancel | {}",
            editor.line_column_label()
        ))
    )?;

    let caret_display_col =
        display_width_to_char_col(&editor.lines[editor.caret_line], editor.caret_col);
    let cursor_col = caret_display_col.saturating_sub(editor.col_offset) as u16;
    let cursor_row = (editor.caret_line.saturating_sub(editor.row_offset) + 2) as u16;
    crossterm::queue!(stdout, crossterm::cursor::MoveTo(cursor_col, cursor_row))?;
    stdout.flush()?;
    Ok(viewport)
}

fn render_editor_line(line: &str, offset: usize, width: usize) -> String {
    let mut out = String::new();
    let mut cell = 0;
    for ch in line.chars() {
        let ch_width = char_display_width(ch);
        let next = cell + ch_width;
        if next <= offset {
            cell = next;
            continue;
        }
        if cell >= offset.saturating_add(width) {
            break;
        }
        out.push(ch);
        cell = next;
    }
    out
}

fn tui_read_confirmation(expected: &str, lines: &[String]) -> Result<()> {
    let mut input = String::new();
    let mut dirty = true;
    loop {
        if dirty {
            draw_tui_confirmation(lines, expected, &input)?;
            dirty = false;
        }
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
                    dirty = true;
                }
                crossterm::event::KeyCode::Char(ch) => {
                    input.push(ch);
                    dirty = true;
                }
                _ => {}
            }
        }
    }
}

fn draw_tui_confirmation(lines: &[String], expected: &str, input: &str) -> Result<()> {
    let mut stdout = io::stdout();
    let mut row = 0_u16;
    crossterm::queue!(
        stdout,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0)
    )?;
    for line in lines {
        write_tui_plain_line(&mut stdout, &mut row, line)?;
    }
    row = row.saturating_add(1);
    write_tui_plain_line(
        &mut stdout,
        &mut row,
        &format!("type `{expected}` to confirm: {input}"),
    )?;
    stdout.flush()?;
    Ok(())
}

fn tui_ls(root: &Path, pointer: &Pointer) -> Result<String> {
    let conn = open_db(root)?;
    let mut lines = Vec::new();
    for card in cards_for_list(&conn, pointer)? {
        let parsed = parse_card_text(&card.text)?;
        lines.push(format!("{} {}", card.address, parsed.title));
    }
    Ok(lines.join(" | "))
}

fn cards_for_list(conn: &Connection, pointer: &Pointer) -> Result<Vec<Card>> {
    let cards = load_cards(conn)?;
    match pointer {
        Pointer::Root => Ok(cards
            .into_iter()
            .filter(|card| card.is_topic || card.is_lit)
            .collect()),
        Pointer::Card(target) => {
            let current = load_card(conn, target)?;
            if current.is_lit {
                Ok(cards.into_iter().filter(|card| card.is_lit).collect())
            } else {
                let topic = topic_id(&current.address)?;
                Ok(cards
                    .into_iter()
                    .filter(|card| card.address.starts_with(&format!("{topic}/")))
                    .collect())
            }
        }
    }
}

fn tui_lsbk(root: &Path) -> Result<String> {
    let conn = open_db(root)?;
    let mut lines = Vec::new();
    for card in load_cards(&conn)? {
        let parsed = parse_card_text(&card.text)?;
        for line in parsed.body.lines() {
            for target in extract_link_targets(line)? {
                if !target_exists(&conn, &target)? {
                    lines.push(format!(
                        "{} {} -> {}: {}",
                        card.address, parsed.title, target, line
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
    let Some(parts) = normalize_session_command(line) else {
        return Ok(true);
    };
    match parts.as_slice() {
        ["zt", "q"] => Ok(false),
        ["zt", "root"] => {
            *pointer = Pointer::Root;
            Ok(true)
        }
        ["zt", "go", target] => {
            let conn = open_db(root)?;
            if !target_exists(&conn, target)? {
                bail!("target `{target}` does not exist");
            }
            *pointer = Pointer::Card((*target).to_string());
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
            println!("{}", session_help_text());
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
        ["zt", "l"] => {
            line_create_literature(root, pointer, input)?;
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
            if card.is_lit {
                match line_select_literature_edit_part(input)? {
                    EditPart::Metadata => {
                        let old_key = card
                            .citation_key
                            .as_deref()
                            .context("Literature Card is missing its Citation key")?;
                        let initial = card
                            .bibtex
                            .as_deref()
                            .context("Literature Card is missing its BibTeX metadata")?;
                        let mut accepted = None;
                        if session_edit_until_saved_with_header(
                            input,
                            initial,
                            "metadata edit",
                            |text| {
                                let metadata = literature::parse_metadata(text)?;
                                if metadata.citation_key != old_key
                                    && (metadata.citation_key.eq_ignore_ascii_case(old_key)
                                        || citation_key_conflicts(
                                            &conn,
                                            &metadata.citation_key,
                                            card.row_id,
                                        )?)
                                {
                                    bail!(
                                        "Citation key `{}` conflicts with an existing Literature Card",
                                        metadata.citation_key
                                    );
                                }
                                accepted = Some((text.to_string(), metadata));
                                Ok(())
                            },
                        )? {
                            let (bibtex, metadata) =
                                accepted.context("validated Literature metadata is missing")?;
                            if metadata.citation_key != old_key {
                                println!("citation key rename:");
                                println!("{old_key} -> {}", metadata.citation_key);
                                println!(
                                    "link macros rewritten: {}",
                                    count_target_links(&conn, old_key)?
                                );
                                read_confirmation(input, "move")?;
                            }
                            update_literature_metadata(
                                &mut conn,
                                &card,
                                &metadata.citation_key,
                                &bibtex,
                                &metadata.title,
                            )?;
                            *pointer = Pointer::Card(metadata.citation_key);
                        }
                    }
                    EditPart::Text => {
                        session_edit_until_saved(input, &card.text, |text| {
                            update_literature_text(&mut conn, &card, text)
                        })?;
                    }
                }
            } else {
                session_edit_until_saved(input, &card.text, |text| {
                    update_card_text(&mut conn, &card, text)
                })?;
            }
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
        _ => bail!("{}", unknown_session_command(&parts)),
    }
}

fn current_card(pointer: &Pointer) -> Result<String> {
    match pointer {
        Pointer::Root => bail!("pointer is on ROOT"),
        Pointer::Card(location) => Ok(location.clone()),
    }
}

fn line_select_literature_edit_part<R: BufRead>(input: &mut R) -> Result<EditPart> {
    loop {
        println!("edit literature card:");
        println!("  1. metadata");
        println!("  2. main text");
        println!("select edit:");
        let mut choice = String::new();
        if input.read_line(&mut choice)? == 0 {
            bail!("edit selection canceled");
        }
        match choice.trim_end_matches(['\r', '\n']) {
            "1" => return Ok(EditPart::Metadata),
            "2" => return Ok(EditPart::Text),
            _ => println!("invalid edit option"),
        }
    }
}

fn line_create_literature<R: BufRead>(
    root: &Path,
    pointer: &mut Pointer,
    input: &mut R,
) -> Result<()> {
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let mut accepted = None;
    if !session_edit_until_saved_with_header(input, "", "metadata edit", |text| {
        let metadata = literature::parse_metadata(text)?;
        if citation_key_exists_case_insensitive(&conn, &metadata.citation_key)? {
            bail!("Literature Card `{}` already exists", metadata.citation_key);
        }
        accepted = Some((text.to_string(), metadata));
        Ok(())
    })? {
        return Ok(());
    }
    let (bibtex, metadata) = accepted.context("validated Literature metadata is missing")?;
    let initial_text = compose_card_text(&metadata.title, "", "");
    let citation_key = metadata.citation_key.clone();
    if session_edit_until_saved(input, &initial_text, |edited| {
        let parsed = parse_literature_edit_text(edited)?;
        let text = compose_card_text(&metadata.title, &parsed.body, "");
        insert_literature_card(&mut conn, &citation_key, &bibtex, &text)
    })? {
        *pointer = Pointer::Card(citation_key);
    }
    Ok(())
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
    if parent.is_lit {
        bail!("zt n is not valid on a Literature Card");
    }
    let location = direct_successor(&parent.address)?;
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
    if parent.is_lit {
        bail!("zt b is not valid on a Literature Card");
    }
    if parent.is_topic {
        bail!("zt b is not valid on a topic card");
    }
    let location = next_side_successor(&conn, &parent.address, &BTreeSet::new())?;
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
                println!("{} {}", card.address, parsed.title);
            }
        }
        Pointer::Card(location) => {
            let card = load_card(&conn, location)?;
            let parsed = parse_card_text(&card.text)?;
            if card.is_lit {
                println!(
                    "citation key: {}",
                    card.citation_key
                        .as_deref()
                        .context("Literature Card is missing its Citation key")?
                );
            } else {
                println!("location: {}", card.address);
            }
            println!("title: {}", parsed.title);
            if card.is_lit {
                println!("metadata:");
                print!(
                    "{}",
                    card.bibtex
                        .as_deref()
                        .context("Literature Card is missing its BibTeX metadata")?
                );
                if !card
                    .bibtex
                    .as_deref()
                    .is_some_and(|raw| raw.ends_with('\n'))
                {
                    println!();
                }
            }
            println!("{}", parsed.body);
            if !parsed.reverse.trim().is_empty() {
                println!("{}", parsed.reverse);
            }
            if !card.is_lit
                && let Ok(direct) = direct_successor(&card.address)
                && location_exists(&conn, &direct)?
            {
                println!("direct: [[{direct}]]");
            }
            if !card.is_lit {
                for side in side_successors(&conn, &card.address)? {
                    println!("side: [[{side}]]");
                }
            }
        }
    }
    Ok(())
}

fn side_successors(conn: &Connection, parent: &str) -> Result<Vec<String>> {
    let mut by_label = BTreeMap::new();
    for card in load_cards(conn)? {
        if let Some(label) = immediate_side_label(&card.address, parent) {
            by_label
                .entry(side_label_to_number(&label))
                .or_insert(card.address);
        }
    }
    Ok(by_label.into_values().collect())
}

fn session_ls(root: &Path, pointer: &Pointer) -> Result<()> {
    let conn = open_db(root)?;
    for card in cards_for_list(&conn, pointer)? {
        let parsed = parse_card_text(&card.text)?;
        println!("{} {}", card.address, parsed.title);
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

    #[test]
    fn tui_keys_translate_to_editor_intents() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        assert_eq!(
            editor_intent_from_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)),
            Some(EditorIntent::Save)
        );
        assert_eq!(
            editor_intent_from_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(EditorIntent::Cancel)
        );
        assert_eq!(
            editor_intent_from_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(EditorIntent::Cancel)
        );
        assert_eq!(
            editor_intent_from_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            Some(EditorIntent::Edit(EditorAction::MoveLeft))
        );
    }

    #[test]
    fn editor_model_counts_chinese_characters_and_preserves_vertical_column() {
        let mut editor = EditorModel::new("你好abc\n短\nabcdef");

        editor.apply(EditorAction::End, ViewportSize::new(10, 20));
        assert_eq!(editor.position(), EditorPosition::new(1, 6));

        editor.apply(EditorAction::MoveDown, ViewportSize::new(10, 20));
        assert_eq!(editor.position(), EditorPosition::new(2, 2));

        editor.apply(EditorAction::MoveDown, ViewportSize::new(10, 20));
        assert_eq!(editor.position(), EditorPosition::new(3, 6));
    }

    #[test]
    fn editor_model_edits_text_at_caret_and_preserves_blank_lines() {
        let mut editor = EditorModel::new("abc\r\ndef\n");
        let viewport = ViewportSize::new(10, 20);

        editor.apply(EditorAction::MoveRight, viewport);
        editor.apply(EditorAction::Insert('中'), viewport);
        editor.apply(EditorAction::Enter, viewport);
        editor.apply(EditorAction::Insert('\t'), viewport);
        editor.apply(EditorAction::End, viewport);
        editor.apply(EditorAction::Delete, viewport);
        editor.apply(EditorAction::Backspace, viewport);

        assert_eq!(editor.text(), "a中\n\tbdef\n");
        assert_eq!(editor.position(), EditorPosition::new(2, 3));
    }

    #[test]
    fn editor_model_preserves_trailing_spaces_tabs_and_normalized_newlines() {
        let mut editor = EditorModel::new("one  \r\ntwo\t\r\n");
        let viewport = ViewportSize::new(10, 20);

        assert_eq!(editor.text(), "one  \ntwo\t\n");

        editor.apply(EditorAction::End, viewport);
        editor.apply(EditorAction::MoveDown, viewport);
        editor.apply(EditorAction::End, viewport);
        editor.apply(EditorAction::Delete, viewport);
        assert_eq!(editor.text(), "one  \ntwo\t");
    }

    #[test]
    fn editor_model_tracks_display_cell_viewport_separately_from_character_columns() {
        let viewport = ViewportSize::new(2, 4);
        let mut editor = EditorModel::new("你好abc\n短\nlast");

        editor.apply(EditorAction::End, viewport);
        assert_eq!(editor.position(), EditorPosition::new(1, 6));
        assert_eq!(editor.viewport_offsets(), (0, 4));

        editor.apply(EditorAction::MoveDown, viewport);
        editor.apply(EditorAction::MoveDown, viewport);
        assert_eq!(editor.position(), EditorPosition::new(3, 5));
        assert_eq!(editor.viewport_offsets(), (1, 2));
    }

    #[test]
    fn editor_model_moves_home_left_and_up_across_lines() {
        let viewport = ViewportSize::new(10, 20);
        let mut editor = EditorModel::new("alpha\nbeta");

        editor.apply(EditorAction::MoveDown, viewport);
        editor.apply(EditorAction::End, viewport);
        assert_eq!(editor.position(), EditorPosition::new(2, 5));

        editor.apply(EditorAction::MoveLeft, viewport);
        assert_eq!(editor.position(), EditorPosition::new(2, 4));

        editor.apply(EditorAction::Home, viewport);
        assert_eq!(editor.position(), EditorPosition::new(2, 1));

        editor.apply(EditorAction::MoveUp, viewport);
        assert_eq!(editor.position(), EditorPosition::new(1, 1));
    }
}
