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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ScreenPosition {
    row: u16,
    column: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ScreenSelection {
    anchor: ScreenPosition,
    active: ScreenPosition,
}

impl ScreenSelection {
    fn ordered(self) -> (ScreenPosition, ScreenPosition) {
        if self.anchor <= self.active {
            (self.anchor, self.active)
        } else {
            (self.active, self.anchor)
        }
    }
}

struct MouseGesture {
    anchor: ScreenPosition,
    dragged: bool,
}

struct TuiFrame {
    links: Vec<LinkSpan>,
    screen: RenderedScreen,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScreenGlyph {
    ch: char,
    width: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RenderedRow {
    glyphs: Vec<ScreenGlyph>,
    hard_break_after: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RenderedScreen {
    rows: Vec<RenderedRow>,
}

impl RenderedScreen {
    fn from_logical_lines(lines: &[String], width: u16) -> Self {
        let width = width.max(1);
        let mut rows = Vec::new();
        for line in lines {
            let mut glyphs = Vec::new();
            let mut used = 0_u16;
            for ch in line.chars() {
                let expanded = if ch == '\t' {
                    vec![' '; char_display_width('\t')]
                } else {
                    vec![ch]
                };
                for rendered_ch in expanded {
                    let glyph_width = char_display_width(rendered_ch).max(1) as u16;
                    if used > 0 && used.saturating_add(glyph_width) > width {
                        rows.push(RenderedRow {
                            glyphs,
                            hard_break_after: false,
                        });
                        glyphs = Vec::new();
                        used = 0;
                    }
                    glyphs.push(ScreenGlyph {
                        ch: rendered_ch,
                        width: glyph_width,
                    });
                    used = used.saturating_add(glyph_width);
                }
            }
            rows.push(RenderedRow {
                glyphs,
                hard_break_after: true,
            });
        }
        Self { rows }
    }

    fn selected_text(&self, selection: ScreenSelection) -> String {
        let (start, end) = selection.ordered();
        let mut selected = String::new();
        for row_index in start.row..=end.row {
            let Some(row) = self.rows.get(row_index as usize) else {
                continue;
            };
            let first = if row_index == start.row {
                start.column
            } else {
                0
            };
            let last = if row_index == end.row {
                end.column
            } else {
                u16::MAX
            };
            let mut column = 0_u16;
            for glyph in &row.glyphs {
                let glyph_end = column.saturating_add(glyph.width).saturating_sub(1);
                if glyph_end >= first && column <= last {
                    selected.push(glyph.ch);
                }
                column = column.saturating_add(glyph.width);
            }
            if row.hard_break_after && row_index < end.row {
                selected.push('\n');
            }
        }
        selected
    }
}

trait ClipboardAccess {
    fn get_text(&mut self) -> Result<String>;
    fn set_text(&mut self, text: &str) -> Result<()>;
}

enum OsClipboardState {
    Ready(arboard::Clipboard),
    Unavailable(String),
}

struct OsClipboard {
    state: OsClipboardState,
}

impl OsClipboard {
    fn new() -> Self {
        let state = match arboard::Clipboard::new() {
            Ok(clipboard) => OsClipboardState::Ready(clipboard),
            Err(error) => OsClipboardState::Unavailable(error.to_string()),
        };
        Self { state }
    }

    fn unavailable<T>(error: &str) -> Result<T> {
        bail!("operating-system clipboard is unavailable: {error}")
    }
}

impl ClipboardAccess for OsClipboard {
    fn get_text(&mut self) -> Result<String> {
        match &mut self.state {
            OsClipboardState::Ready(clipboard) => clipboard
                .get_text()
                .context("failed to read the operating-system clipboard"),
            OsClipboardState::Unavailable(error) => Self::unavailable(error),
        }
    }

    fn set_text(&mut self, text: &str) -> Result<()> {
        match &mut self.state {
            OsClipboardState::Ready(clipboard) => clipboard
                .set_text(text)
                .context("failed to write the operating-system clipboard"),
            OsClipboardState::Unavailable(error) => Self::unavailable(error),
        }
    }
}

#[cfg(debug_assertions)]
struct TestClipboard {
    text: String,
    output: Option<PathBuf>,
}

#[cfg(debug_assertions)]
impl ClipboardAccess for TestClipboard {
    fn get_text(&mut self) -> Result<String> {
        Ok(self.text.clone())
    }

    fn set_text(&mut self, text: &str) -> Result<()> {
        self.text = text.to_string();
        if let Some(path) = &self.output {
            fs::write(path, text).context("failed to write test clipboard output")?;
        }
        Ok(())
    }
}

enum SessionClipboard {
    Os(OsClipboard),
    #[cfg(debug_assertions)]
    Test(TestClipboard),
}

impl SessionClipboard {
    fn new() -> Self {
        #[cfg(debug_assertions)]
        if let Some(text) = std::env::var_os("ZT_TEST_CLIPBOARD_TEXT") {
            return Self::Test(TestClipboard {
                text: text.to_string_lossy().into_owned(),
                output: std::env::var_os("ZT_TEST_CLIPBOARD_OUTPUT").map(PathBuf::from),
            });
        }
        Self::Os(OsClipboard::new())
    }
}

impl ClipboardAccess for SessionClipboard {
    fn get_text(&mut self) -> Result<String> {
        match self {
            Self::Os(clipboard) => clipboard.get_text(),
            #[cfg(debug_assertions)]
            Self::Test(clipboard) => clipboard.get_text(),
        }
    }

    fn set_text(&mut self, text: &str) -> Result<()> {
        match self {
            Self::Os(clipboard) => clipboard.set_text(text),
            #[cfg(debug_assertions)]
            Self::Test(clipboard) => clipboard.set_text(text),
        }
    }
}

fn copy_screen_selection(
    screen: &RenderedScreen,
    selection: Option<ScreenSelection>,
    clipboard: &mut dyn ClipboardAccess,
) -> Result<()> {
    if let Some(selection) = selection {
        clipboard.set_text(&screen.selected_text(selection))?;
    }
    Ok(())
}

fn normalize_command_bar_clipboard(text: &str) -> Result<String> {
    let normalized_newlines = text.replace("\r\n", "\n");
    let mut normalized = String::with_capacity(normalized_newlines.len());
    for ch in normalized_newlines.chars() {
        match ch {
            '\r' | '\n' | '\t' => normalized.push(' '),
            ch if ch.is_control() => {
                bail!("clipboard text contains an unsupported control character")
            }
            ch => normalized.push(ch),
        }
    }
    Ok(normalized)
}

fn normalize_editor_clipboard(text: &str) -> Result<String> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    if normalized
        .chars()
        .any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
    {
        bail!("clipboard text contains an unsupported control character");
    }
    Ok(normalized)
}

fn paste_command_bar(command: &mut String, clipboard: &mut dyn ClipboardAccess) -> Result<()> {
    let clipboard_text = clipboard.get_text()?;
    let normalized = normalize_command_bar_clipboard(&clipboard_text)?;
    command.push_str(&normalized);
    Ok(())
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
    let mut clipboard = SessionClipboard::new();
    let mut selection = None;
    let mut mouse_gesture: Option<MouseGesture> = None;
    let mut frame = draw_tui(&root, &pointer, &command, &message, selection.as_ref())?;
    loop {
        if !is_service_up(&root) {
            message = "service disconnected".to_string();
            draw_tui(&root, &pointer, &command, &message, None)?;
            break;
        }
        match read_tui_event()? {
            crossterm::event::Event::Key(key) if is_tui_input_key(&key) => {
                if is_copy_shortcut(&key) {
                    if let Err(error) =
                        copy_screen_selection(&frame.screen, selection, &mut clipboard)
                    {
                        message = format!("clipboard copy failed: {error:#}");
                    }
                    frame = draw_tui(&root, &pointer, &command, &message, selection.as_ref())?;
                    continue;
                }
                if is_paste_shortcut(&key) {
                    match paste_command_bar(&mut command, &mut clipboard) {
                        Ok(()) => selection = None,
                        Err(error) => {
                            message = format!("clipboard paste failed: {error:#}");
                        }
                    }
                    frame = draw_tui(&root, &pointer, &command, &message, selection.as_ref())?;
                    continue;
                }
                selection = None;
                match key.code {
                    _ if is_plain_ctrl_c(&key) => {
                        break;
                    }
                    crossterm::event::KeyCode::Char(ch) => command.push(ch),
                    crossterm::event::KeyCode::Backspace => {
                        command.pop();
                    }
                    crossterm::event::KeyCode::Enter => {
                        let line = command.trim().to_string();
                        command.clear();
                        match handle_tui_command(
                            &root,
                            &mut pointer,
                            &line,
                            &mut message,
                            &mut clipboard,
                        ) {
                            Ok(true) => {}
                            Ok(false) => break,
                            Err(err) => message = format!("{err:#}"),
                        }
                    }
                    crossterm::event::KeyCode::Esc => command.clear(),
                    _ => {}
                }
            }
            crossterm::event::Event::Mouse(mouse) => {
                let position = ScreenPosition {
                    row: mouse.row,
                    column: mouse.column,
                };
                match mouse.kind {
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                        selection = None;
                        mouse_gesture = Some(MouseGesture {
                            anchor: position,
                            dragged: false,
                        });
                    }
                    crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                        if let Some(gesture) = mouse_gesture.as_mut() {
                            gesture.dragged = true;
                            selection = Some(ScreenSelection {
                                anchor: gesture.anchor,
                                active: position,
                            });
                        } else {
                            continue;
                        }
                    }
                    crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                        let Some(gesture) = mouse_gesture.take() else {
                            continue;
                        };
                        if gesture.dragged {
                            selection = Some(ScreenSelection {
                                anchor: gesture.anchor,
                                active: position,
                            });
                        } else if let Some(target) =
                            link_target_at(&frame.links, mouse.row, mouse.column)
                        {
                            let conn = open_db(&root)?;
                            if target_exists(&conn, target)? {
                                pointer = Pointer::Card(target.to_string());
                                message.clear();
                            } else {
                                message = format!("target `{target}` does not exist");
                            }
                        }
                    }
                    _ => continue,
                }
            }
            crossterm::event::Event::Resize(_, _) => {
                selection = None;
                mouse_gesture = None;
            }
            _ => continue,
        }
        frame = draw_tui(&root, &pointer, &command, &message, selection.as_ref())?;
    }
    Ok(())
}

fn is_tui_input_key(key: &crossterm::event::KeyEvent) -> bool {
    matches!(
        key.kind,
        crossterm::event::KeyEventKind::Press | crossterm::event::KeyEventKind::Repeat
    )
}

fn read_tui_event() -> Result<crossterm::event::Event> {
    let event = crossterm::event::read()?;
    #[cfg(debug_assertions)]
    if std::env::var_os("ZT_TEST_CLIPBOARD_TEXT").is_some()
        || std::env::var_os("ZT_TEST_SHORTCUTS").is_some()
    {
        let shortcut = match event {
            crossterm::event::Event::Key(crossterm::event::KeyEvent {
                code: crossterm::event::KeyCode::Char('\\'),
                kind: crossterm::event::KeyEventKind::Press | crossterm::event::KeyEventKind::Repeat,
                ..
            }) => Some('c'),
            crossterm::event::Event::Key(crossterm::event::KeyEvent {
                code: crossterm::event::KeyCode::Char(']'),
                kind: crossterm::event::KeyEventKind::Press | crossterm::event::KeyEventKind::Repeat,
                ..
            }) => Some('v'),
            _ => None,
        };
        if let Some(ch) = shortcut {
            return Ok(crossterm::event::Event::Key(
                crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Char(ch),
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT,
                ),
            ));
        }
    }
    Ok(event)
}

fn is_copy_shortcut(key: &crossterm::event::KeyEvent) -> bool {
    matches!(key.code, crossterm::event::KeyCode::Char('c' | 'C'))
        && key.modifiers
            == (crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT)
}

fn is_paste_shortcut(key: &crossterm::event::KeyEvent) -> bool {
    matches!(key.code, crossterm::event::KeyCode::Char('v' | 'V'))
        && key.modifiers
            == (crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT)
}

fn is_plain_ctrl_c(key: &crossterm::event::KeyEvent) -> bool {
    matches!(key.code, crossterm::event::KeyCode::Char('c' | 'C'))
        && key.modifiers == crossterm::event::KeyModifiers::CONTROL
}

fn link_target_at(links: &[LinkSpan], row: u16, column: u16) -> Option<&str> {
    links
        .iter()
        .find(|span| span.row == row && column >= span.start && column < span.end)
        .map(|span| span.target.as_str())
}

fn draw_tui(
    root: &Path,
    pointer: &Pointer,
    command: &str,
    message: &str,
    selection: Option<&ScreenSelection>,
) -> Result<TuiFrame> {
    let conn = open_db(root)?;
    let mut stdout = io::stdout();
    let mut links = Vec::new();
    let mut lines = Vec::new();
    let mut row = 0_u16;
    crossterm::queue!(
        stdout,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0)
    )?;
    match pointer {
        Pointer::Root => {
            write_tui_plain_line(&mut stdout, &mut row, "ROOT")?;
            lines.push("ROOT".to_string());
            for card in load_cards(&conn)?.into_iter().filter(|card| card.is_topic) {
                let parsed = parse_card_text(&card.text)?;
                write_tui_location_line(
                    &mut stdout,
                    &mut row,
                    &card.address,
                    &format!(" {}", parsed.title),
                    &mut links,
                )?;
                lines.push(format!("{} {}", card.address, parsed.title));
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
                lines.push(format!(
                    "citation key: {}",
                    card.citation_key
                        .as_deref()
                        .context("Literature Card is missing its Citation key")?
                ));
            } else {
                write_tui_plain_line(
                    &mut stdout,
                    &mut row,
                    &format!("location: {}", card.address),
                )?;
                lines.push(format!("location: {}", card.address));
            }
            write_tui_plain_line(&mut stdout, &mut row, &format!("title: {}", parsed.title))?;
            lines.push(format!("title: {}", parsed.title));
            if card.is_lit {
                write_tui_plain_line(&mut stdout, &mut row, "metadata:")?;
                lines.push("metadata:".to_string());
                for line in card
                    .bibtex
                    .as_deref()
                    .context("Literature Card is missing its BibTeX metadata")?
                    .lines()
                {
                    write_tui_plain_line(&mut stdout, &mut row, line)?;
                    lines.push(line.to_string());
                }
            }
            for line in parsed.body.lines() {
                write_tui_link_line(&mut stdout, &conn, &mut row, line, &mut links)?;
                lines.push(line.to_string());
            }
            if !parsed.reverse.trim().is_empty() {
                for line in parsed.reverse.lines() {
                    write_tui_link_line(&mut stdout, &conn, &mut row, line, &mut links)?;
                    lines.push(line.to_string());
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
                lines.push(format!("direct: [[{direct}]]"));
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
                    lines.push(format!("side: [[{side}]]"));
                }
            }
        }
    }
    row = row.saturating_add(1);
    lines.push(String::new());
    write_tui_plain_line(&mut stdout, &mut row, message)?;
    lines.push(message.to_string());
    write_tui_plain_line(&mut stdout, &mut row, &format!("> {command}"))?;
    lines.push(format!("> {command}"));
    let width = crossterm::terminal::size()?.0;
    let screen = RenderedScreen::from_logical_lines(&lines, width);
    if let Some(selection) = selection {
        queue_screen_selection(&mut stdout, &screen, *selection)?;
    }
    stdout.flush()?;
    Ok(TuiFrame { links, screen })
}

fn queue_screen_selection<W: Write>(
    stdout: &mut W,
    screen: &RenderedScreen,
    selection: ScreenSelection,
) -> Result<()> {
    let (start, end) = selection.ordered();
    for row in start.row..=end.row {
        let Some(rendered_row) = screen.rows.get(row as usize) else {
            continue;
        };
        let first = if row == start.row { start.column } else { 0 };
        let last = if row == end.row { end.column } else { u16::MAX };
        let mut column = 0_u16;
        let mut selected_column = None;
        let mut selected = String::new();
        for glyph in &rendered_row.glyphs {
            let glyph_end = column.saturating_add(glyph.width).saturating_sub(1);
            if glyph_end >= first && column <= last {
                selected_column.get_or_insert(column);
                selected.push(glyph.ch);
            }
            column = column.saturating_add(glyph.width);
        }
        let Some(selected_column) = selected_column else {
            continue;
        };
        crossterm::queue!(
            stdout,
            crossterm::cursor::MoveTo(selected_column, row),
            crossterm::style::SetAttribute(crossterm::style::Attribute::Reverse),
            crossterm::style::Print(selected),
            crossterm::style::SetAttribute(crossterm::style::Attribute::NoReverse)
        )?;
    }
    Ok(())
}

fn write_tui_plain_line<W: Write>(stdout: &mut W, row: &mut u16, text: &str) -> Result<()> {
    let rendered = expand_display_tabs(text);
    crossterm::queue!(
        stdout,
        crossterm::cursor::MoveTo(0, *row),
        crossterm::style::Print(rendered)
    )?;
    *row = row.saturating_add(tui_line_height(text));
    Ok(())
}

fn expand_display_tabs(text: &str) -> String {
    text.chars()
        .flat_map(|ch| {
            if ch == '\t' {
                vec![' '; char_display_width('\t')]
            } else {
                vec![ch]
            }
        })
        .collect()
}

fn tui_text_width(text: &str) -> usize {
    text.chars().map(char_display_width).sum()
}

fn tui_line_height(text: &str) -> u16 {
    let width = crossterm::terminal::size()
        .map(|(width, _)| width.max(1) as usize)
        .unwrap_or(80);
    let display_width = tui_text_width(text).max(1);
    display_width.div_ceil(width) as u16
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
    crossterm::queue!(stdout, crossterm::style::Print(expand_display_tabs(suffix)))?;
    *row = row.saturating_add(tui_line_height(&format!("{location}{suffix}")));
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
        crossterm::queue!(stdout, crossterm::style::Print(expand_display_tabs(plain)))?;
        col = col.saturating_add(tui_text_width(plain) as u16);
        let content_start = start + 2;
        let Some(end_rel) = text[content_start..].find("]]") else {
            let rest = &text[start..];
            crossterm::queue!(stdout, crossterm::style::Print(expand_display_tabs(rest)))?;
            *row = row.saturating_add(tui_line_height(text));
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
                crossterm::style::Print(expand_display_tabs(macro_text)),
                crossterm::style::SetAttribute(crossterm::style::Attribute::NoUnderline),
                crossterm::style::ResetColor
            )?;
        }
        col = col.saturating_add(tui_text_width(macro_text) as u16);
        offset = end + 2;
    }
    let rest = &text[offset..];
    crossterm::queue!(stdout, crossterm::style::Print(expand_display_tabs(rest)))?;
    *row = row.saturating_add(tui_line_height(text));
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
    let width = crossterm::terminal::size()?.0.max(1);
    let mut span_row = row.saturating_add(col / width);
    let mut span_start = col % width;
    let mut remaining = tui_text_width(text) as u16;
    while remaining > 0 {
        let span_width = remaining.min(width.saturating_sub(span_start));
        links.push(LinkSpan {
            row: span_row,
            start: span_start,
            end: span_start.saturating_add(span_width),
            target: target.to_string(),
        });
        remaining = remaining.saturating_sub(span_width);
        span_row = span_row.saturating_add(1);
        span_start = 0;
    }
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
    clipboard: &mut dyn ClipboardAccess,
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
            tui_create_topic(root, pointer, &title.join(" "), clipboard)?;
            message.clear();
            Ok(true)
        }
        ["zt", "l"] => {
            tui_create_literature(root, pointer, clipboard)?;
            message.clear();
            Ok(true)
        }
        ["zt", "n"] => {
            let at = current_card(pointer)?;
            tui_create_direct(root, pointer, &at, clipboard)?;
            message.clear();
            Ok(true)
        }
        ["zt", "b"] => {
            let at = current_card(pointer)?;
            tui_create_side(root, pointer, &at, clipboard)?;
            message.clear();
            Ok(true)
        }
        ["zt", "e"] => {
            let at = current_card(pointer)?;
            let target = tui_edit_card(root, &at, clipboard)?;
            *pointer = Pointer::Card(target);
            message.clear();
            Ok(true)
        }
        ["zt", "del"] => {
            let at = current_card(pointer)?;
            let _lock = acquire_edit_lock(root)?;
            let mut conn = open_db(root)?;
            let plan = delete_plan(&conn, &at)?;
            tui_read_confirmation(
                &plan.confirmation,
                &delete_verification_lines(&plan),
                clipboard,
            )?;
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
            tui_read_confirmation("move", &move_verification_lines(&plan), clipboard)?;
            apply_move_plan(&mut conn, &plan)?;
            *pointer = Pointer::Card((*new_location).to_string());
            message.clear();
            Ok(true)
        }
        _ => bail!("{}", unknown_session_command(&parts)),
    }
}

fn tui_create_topic(
    root: &Path,
    pointer: &mut Pointer,
    title: &str,
    clipboard: &mut dyn ClipboardAccess,
) -> Result<()> {
    if title.trim().is_empty() || title.contains('\n') || title.contains('\r') {
        bail!("topic title must be non-empty single-line text");
    }
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let location = next_topic_location(&conn)?;
    if tui_edit_until_saved(&topic_template(title), clipboard, |text| {
        insert_card(&mut conn, &location, true, text)?;
        bump_next_topic_id(&conn, &location)?;
        Ok(())
    })? {
        *pointer = Pointer::Card(location);
    }
    Ok(())
}

fn tui_create_literature(
    root: &Path,
    pointer: &mut Pointer,
    clipboard: &mut dyn ClipboardAccess,
) -> Result<()> {
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let mut accepted = None;
    if !tui_edit_until_saved_with_header("", "metadata edit", clipboard, |text| {
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
    if tui_edit_until_saved(&initial_text, clipboard, |edited| {
        let parsed = parse_literature_edit_text(edited)?;
        let text = compose_card_text(&metadata.title, &parsed.body, "");
        insert_literature_card(&mut conn, &citation_key, &bibtex, &text)
    })? {
        *pointer = Pointer::Card(citation_key);
    }
    Ok(())
}

fn tui_create_direct(
    root: &Path,
    pointer: &mut Pointer,
    at: &str,
    clipboard: &mut dyn ClipboardAccess,
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
    if tui_edit_until_saved(&regular_template(), clipboard, |text| {
        insert_card(&mut conn, &location, false, text)
    })? {
        *pointer = Pointer::Card(location);
    }
    Ok(())
}

fn tui_create_side(
    root: &Path,
    pointer: &mut Pointer,
    at: &str,
    clipboard: &mut dyn ClipboardAccess,
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
    if tui_edit_until_saved(&regular_template(), clipboard, |text| {
        insert_card(&mut conn, &location, false, text)
    })? {
        *pointer = Pointer::Card(location);
    }
    Ok(())
}

fn tui_edit_card(root: &Path, at: &str, clipboard: &mut dyn ClipboardAccess) -> Result<String> {
    let _lock = acquire_edit_lock(root)?;
    let mut conn = open_db(root)?;
    let card = load_card(&conn, at)?;
    if !card.is_lit {
        tui_edit_until_saved(&card.text, clipboard, |text| {
            update_card_text(&mut conn, &card, text)
        })?;
        return Ok(card.address);
    }
    match tui_select_literature_edit_part(clipboard)? {
        EditPart::Metadata => tui_edit_literature_metadata(&mut conn, &card, clipboard),
        EditPart::Text => {
            tui_edit_until_saved(&card.text, clipboard, |text| {
                update_literature_text(&mut conn, &card, text)
            })?;
            Ok(card.address)
        }
    }
}

fn tui_edit_literature_metadata(
    conn: &mut Connection,
    card: &Card,
    clipboard: &mut dyn ClipboardAccess,
) -> Result<String> {
    let old_key = card
        .citation_key
        .as_deref()
        .context("Literature Card is missing its Citation key")?;
    let initial = card
        .bibtex
        .as_deref()
        .context("Literature Card is missing its BibTeX metadata")?;
    let mut accepted = None;
    if !tui_edit_until_saved_with_header(initial, "metadata edit", clipboard, |text| {
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
            clipboard,
        )?;
    }
    update_literature_metadata(conn, card, &metadata.citation_key, &bibtex, &metadata.title)?;
    Ok(metadata.citation_key)
}

fn tui_select_literature_edit_part(clipboard: &mut dyn ClipboardAccess) -> Result<EditPart> {
    let mut input = String::new();
    let mut error = String::new();
    let mut selection = None;
    let mut mouse_gesture: Option<MouseGesture> = None;
    loop {
        let screen = draw_tui_literature_edit_selection(&input, &error, selection.as_ref())?;
        match read_tui_event()? {
            crossterm::event::Event::Key(key) if is_tui_input_key(&key) => {
                if is_copy_shortcut(&key) {
                    if let Err(copy_error) = copy_screen_selection(&screen, selection, clipboard) {
                        error = format!("clipboard copy failed: {copy_error:#}");
                    }
                    continue;
                }
                if is_paste_shortcut(&key) {
                    continue;
                }
                selection = None;
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
            crossterm::event::Event::Mouse(mouse) => {
                update_modal_screen_selection(mouse, &mut selection, &mut mouse_gesture);
            }
            crossterm::event::Event::Resize(_, _) => {
                selection = None;
                mouse_gesture = None;
            }
            _ => {}
        }
    }
}

fn update_modal_screen_selection(
    mouse: crossterm::event::MouseEvent,
    selection: &mut Option<ScreenSelection>,
    mouse_gesture: &mut Option<MouseGesture>,
) {
    let position = ScreenPosition {
        row: mouse.row,
        column: mouse.column,
    };
    match mouse.kind {
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            *selection = None;
            *mouse_gesture = Some(MouseGesture {
                anchor: position,
                dragged: false,
            });
        }
        crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
            if let Some(gesture) = mouse_gesture.as_mut() {
                gesture.dragged = true;
                *selection = Some(ScreenSelection {
                    anchor: gesture.anchor,
                    active: position,
                });
            }
        }
        crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
            if let Some(gesture) = mouse_gesture.take()
                && gesture.dragged
            {
                *selection = Some(ScreenSelection {
                    anchor: gesture.anchor,
                    active: position,
                });
            }
        }
        _ => {}
    }
}

fn draw_tui_literature_edit_selection(
    input: &str,
    error: &str,
    selection: Option<&ScreenSelection>,
) -> Result<RenderedScreen> {
    let mut stdout = io::stdout();
    let mut row = 0_u16;
    let lines = vec![
        "edit literature card:".to_string(),
        "  1. metadata".to_string(),
        "  2. main text".to_string(),
        error.to_string(),
        format!("select edit: {input}"),
    ];
    crossterm::queue!(
        stdout,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0)
    )?;
    for (index, line) in lines.iter().enumerate() {
        if index == 3 && !line.is_empty() {
            crossterm::queue!(
                stdout,
                crossterm::cursor::MoveTo(0, row),
                crossterm::style::SetForegroundColor(crossterm::style::Color::DarkRed),
                crossterm::style::Print(line),
                crossterm::style::ResetColor
            )?;
            row = row.saturating_add(1);
        } else {
            write_tui_plain_line(&mut stdout, &mut row, line)?;
        }
    }
    let width = crossterm::terminal::size()?.0;
    let screen = RenderedScreen::from_logical_lines(&lines, width);
    if let Some(selection) = selection {
        queue_screen_selection(&mut stdout, &screen, *selection)?;
    }
    stdout.flush()?;
    Ok(screen)
}

fn tui_edit_until_saved<F>(
    initial_text: &str,
    clipboard: &mut dyn ClipboardAccess,
    save: F,
) -> Result<bool>
where
    F: FnMut(&str) -> Result<()>,
{
    tui_edit_until_saved_with_header(initial_text, "edit mode", clipboard, save)
}

fn tui_edit_until_saved_with_header<F>(
    initial_text: &str,
    header: &str,
    clipboard: &mut dyn ClipboardAccess,
    mut save: F,
) -> Result<bool>
where
    F: FnMut(&str) -> Result<()>,
{
    let mut editor = EditorModel::new(initial_text);
    let mut error = String::new();
    loop {
        match run_tui_editor(&mut editor, &error, header, clipboard)? {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct EditorPoint {
    line: usize,
    column: usize,
}

impl EditorPoint {
    fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EditorSelection {
    anchor: EditorPoint,
    active: EditorPoint,
}

struct EditorMouseGesture {
    anchor: EditorPoint,
    dragged: bool,
}

impl EditorSelection {
    fn ordered(self) -> (EditorPoint, EditorPoint) {
        if self.anchor <= self.active {
            (self.anchor, self.active)
        } else {
            (self.active, self.anchor)
        }
    }
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
    selection: Option<EditorSelection>,
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
            selection: None,
            desired_col: None,
            row_offset: 0,
            col_offset: 0,
        }
    }

    fn apply(&mut self, action: EditorAction, viewport: ViewportSize) {
        if matches!(
            action,
            EditorAction::MoveLeft
                | EditorAction::MoveRight
                | EditorAction::MoveUp
                | EditorAction::MoveDown
                | EditorAction::Home
                | EditorAction::End
        ) {
            self.selection = None;
        }
        if self.selection.is_some()
            && matches!(action, EditorAction::Backspace | EditorAction::Delete)
        {
            self.delete_selection();
            self.ensure_visible(viewport);
            return;
        }
        let replacement_start = if self.selection.is_some()
            && matches!(action, EditorAction::Insert(_) | EditorAction::Enter)
        {
            self.delete_selection()
        } else {
            None
        };
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
        if let Some(start) = replacement_start {
            self.caret_line = start.line;
            self.caret_col = start.column;
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

    fn clamped_point(&self, point: EditorPoint) -> EditorPoint {
        let line = point.line.min(self.lines.len().saturating_sub(1));
        let column = point.column.min(self.lines[line].chars().count());
        EditorPoint::new(line, column)
    }

    fn point_for_viewport_cell(&self, row: usize, column: usize) -> EditorPoint {
        let line_index = self
            .row_offset
            .saturating_add(row)
            .min(self.lines.len().saturating_sub(1));
        let target_cell = self.col_offset.saturating_add(column);
        let mut cell = 0_usize;
        for (char_column, ch) in self.lines[line_index].chars().enumerate() {
            let next = cell.saturating_add(char_display_width(ch));
            if target_cell < next {
                return EditorPoint::new(line_index, char_column);
            }
            cell = next;
        }
        EditorPoint::new(line_index, self.lines[line_index].chars().count())
    }

    fn point_after(&self, point: EditorPoint) -> EditorPoint {
        let point = self.clamped_point(point);
        if point.column < self.lines[point.line].chars().count() {
            EditorPoint::new(point.line, point.column + 1)
        } else {
            point
        }
    }

    fn scroll_for_drag(&mut self, screen_row: u16, screen_column: u16, viewport: ViewportSize) {
        let last_screen_row = viewport.rows.saturating_add(1) as u16;
        if screen_row <= 2 {
            self.row_offset = self.row_offset.saturating_sub(1);
        } else if screen_row >= last_screen_row
            && self.row_offset.saturating_add(viewport.rows) < self.lines.len()
        {
            self.row_offset += 1;
        }

        let last_screen_column = viewport.cols.saturating_sub(1) as u16;
        if screen_column == 0 {
            self.col_offset = self.col_offset.saturating_sub(1);
        } else if screen_column >= last_screen_column {
            let max_width = self
                .lines
                .iter()
                .map(|line| display_width_to_char_col(line, line.chars().count()))
                .max()
                .unwrap_or(0);
            if self.col_offset < max_width {
                self.col_offset += 1;
            }
        }
    }

    fn select(&mut self, anchor: EditorPoint, active: EditorPoint, viewport: ViewportSize) {
        let anchor = self.clamped_point(anchor);
        let active = self.clamped_point(active);
        self.caret_line = active.line;
        self.caret_col = active.column;
        self.selection = (anchor != active).then_some(EditorSelection { anchor, active });
        self.desired_col = None;
        self.ensure_visible(viewport);
    }

    fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection?.ordered();
        if start.line == end.line {
            return Some(
                self.lines[start.line]
                    .chars()
                    .skip(start.column)
                    .take(end.column.saturating_sub(start.column))
                    .collect(),
            );
        }

        let mut selected = self.lines[start.line]
            .chars()
            .skip(start.column)
            .collect::<String>();
        selected.push('\n');
        for line in (start.line + 1)..end.line {
            selected.push_str(&self.lines[line]);
            selected.push('\n');
        }
        selected.extend(self.lines[end.line].chars().take(end.column));
        Some(selected)
    }

    fn delete_selection(&mut self) -> Option<EditorPoint> {
        let (start, end) = self.selection.take()?.ordered();
        if start.line == end.line {
            let line = &mut self.lines[start.line];
            let start_byte = byte_index_for_char_col(line, start.column);
            let end_byte = byte_index_for_char_col(line, end.column);
            line.replace_range(start_byte..end_byte, "");
        } else {
            let left = self.lines[start.line]
                .chars()
                .take(start.column)
                .collect::<String>();
            let right = self.lines[end.line]
                .chars()
                .skip(end.column)
                .collect::<String>();
            self.lines.splice(
                start.line..=end.line,
                std::iter::once(format!("{left}{right}")),
            );
        }
        self.caret_line = start.line;
        self.caret_col = start.column;
        self.desired_col = None;
        Some(start)
    }

    fn insert_text(&mut self, text: &str, viewport: ViewportSize) {
        let replacement_start = self.delete_selection();
        let line_index = self.caret_line;
        let column = self.caret_col;
        let byte = byte_index_for_char_col(&self.lines[line_index], column);
        let right = self.lines[line_index].split_off(byte);
        let mut inserted_lines = text.split('\n');
        let first = inserted_lines.next().unwrap_or_default();
        self.lines[line_index].push_str(first);
        let mut final_line = line_index;
        let mut final_column = column + first.chars().count();
        for part in inserted_lines {
            final_line += 1;
            final_column = part.chars().count();
            self.lines.insert(final_line, part.to_string());
        }
        self.lines[final_line].push_str(&right);
        self.caret_line = final_line;
        self.caret_col = final_column;
        if let Some(start) = replacement_start {
            self.caret_line = start.line;
            self.caret_col = start.column;
        }
        self.desired_col = None;
        self.ensure_visible(viewport);
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

fn copy_editor_selection(editor: &EditorModel, clipboard: &mut dyn ClipboardAccess) -> Result<()> {
    if let Some(text) = editor.selected_text() {
        clipboard.set_text(&text)?;
    }
    Ok(())
}

fn paste_editor(
    editor: &mut EditorModel,
    viewport: ViewportSize,
    clipboard: &mut dyn ClipboardAccess,
) -> Result<()> {
    let clipboard_text = clipboard.get_text()?;
    let normalized = normalize_editor_clipboard(&clipboard_text)?;
    editor.insert_text(&normalized, viewport);
    Ok(())
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

fn run_tui_editor(
    editor: &mut EditorModel,
    error: &str,
    header: &str,
    clipboard: &mut dyn ClipboardAccess,
) -> Result<EditOutcome> {
    let mut status = error.to_string();
    let mut mouse_gesture: Option<EditorMouseGesture> = None;
    loop {
        let viewport = draw_tui_editor(editor, &status, header)?;
        match read_tui_event()? {
            crossterm::event::Event::Key(key) if is_tui_input_key(&key) => {
                if is_copy_shortcut(&key) {
                    if let Err(copy_error) = copy_editor_selection(editor, clipboard) {
                        status = format!("clipboard copy failed: {copy_error:#}");
                    }
                    continue;
                }
                if is_paste_shortcut(&key) {
                    if let Err(paste_error) = paste_editor(editor, viewport, clipboard) {
                        status = format!("clipboard paste failed: {paste_error:#}");
                    }
                    continue;
                }
                match editor_intent_from_key(key) {
                    Some(EditorIntent::Save) => return Ok(EditOutcome::Saved(editor.text())),
                    Some(EditorIntent::Cancel) => return Ok(EditOutcome::Canceled),
                    Some(EditorIntent::Edit(action)) => editor.apply(action, viewport),
                    None => {}
                }
            }
            crossterm::event::Event::Mouse(mouse) => match mouse.kind {
                crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                    if let Some(point) = editor_point_from_mouse(editor, mouse, viewport) {
                        editor.select(point, point, viewport);
                        mouse_gesture = Some(EditorMouseGesture {
                            anchor: point,
                            dragged: false,
                        });
                    }
                }
                crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                    let Some(gesture) = mouse_gesture.as_mut() else {
                        continue;
                    };
                    gesture.dragged = true;
                    editor.scroll_for_drag(mouse.row, mouse.column, viewport);
                    if let Some(point) = editor_point_from_mouse(editor, mouse, viewport) {
                        let (anchor, active) =
                            editor_drag_selection_points(editor, gesture.anchor, point);
                        editor.select(anchor, active, viewport);
                    }
                }
                crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                    let Some(gesture) = mouse_gesture.take() else {
                        continue;
                    };
                    if gesture.dragged
                        && let Some(point) = editor_point_from_mouse(editor, mouse, viewport)
                    {
                        let (anchor, active) =
                            editor_drag_selection_points(editor, gesture.anchor, point);
                        editor.select(anchor, active, viewport);
                    }
                }
                _ => {}
            },
            crossterm::event::Event::Resize(_, _) => {}
            _ => {}
        }
    }
}

fn editor_drag_selection_points(
    editor: &EditorModel,
    anchor: EditorPoint,
    active: EditorPoint,
) -> (EditorPoint, EditorPoint) {
    if active >= anchor {
        (anchor, editor.point_after(active))
    } else {
        (editor.point_after(anchor), active)
    }
}

fn editor_point_from_mouse(
    editor: &EditorModel,
    mouse: crossterm::event::MouseEvent,
    viewport: ViewportSize,
) -> Option<EditorPoint> {
    let last_row = viewport.rows.saturating_add(1) as u16;
    if mouse.row < 2 || mouse.row > last_row {
        return None;
    }
    let row = mouse.row.saturating_sub(2) as usize;
    let column = (mouse.column as usize).min(viewport.cols.saturating_sub(1));
    Some(editor.point_for_viewport_cell(row, column))
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
        if editor.lines.get(line_index).is_some() {
            queue_editor_line(&mut stdout, editor, line_index, (row + 2) as u16, viewport)?;
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

fn queue_editor_line<W: Write>(
    stdout: &mut W,
    editor: &EditorModel,
    line_index: usize,
    screen_row: u16,
    viewport: ViewportSize,
) -> Result<()> {
    let line = &editor.lines[line_index];
    let visible_end = editor.col_offset.saturating_add(viewport.cols);
    let selection = editor.selection.map(EditorSelection::ordered);
    let mut cell = 0;
    for (column, ch) in line.chars().enumerate() {
        let ch_width = char_display_width(ch);
        let next = cell + ch_width;
        if next <= editor.col_offset {
            cell = next;
            continue;
        }
        if cell >= visible_end {
            break;
        }
        let selected = selection.is_some_and(|(start, end)| {
            let point = EditorPoint::new(line_index, column);
            point >= start && point < end
        });
        let rendered = if ch == '\t' {
            let visible_start = cell.max(editor.col_offset);
            let visible_stop = next.min(visible_end);
            " ".repeat(visible_stop.saturating_sub(visible_start))
        } else {
            ch.to_string()
        };
        if selected {
            crossterm::queue!(
                stdout,
                crossterm::style::SetAttribute(crossterm::style::Attribute::Reverse),
                crossterm::style::Print(rendered),
                crossterm::style::SetAttribute(crossterm::style::Attribute::NoReverse)
            )?;
        } else {
            crossterm::queue!(stdout, crossterm::style::Print(rendered))?;
        }
        cell = next;
    }

    let line_end = EditorPoint::new(line_index, line.chars().count());
    let newline_selected = line_index + 1 < editor.lines.len()
        && selection.is_some_and(|(start, end)| line_end >= start && line_end < end);
    let newline_cell = cell.saturating_sub(editor.col_offset);
    if newline_selected && newline_cell < viewport.cols {
        crossterm::queue!(
            stdout,
            crossterm::cursor::MoveTo(newline_cell as u16, screen_row),
            crossterm::style::SetAttribute(crossterm::style::Attribute::Reverse),
            crossterm::style::Print(' '),
            crossterm::style::SetAttribute(crossterm::style::Attribute::NoReverse)
        )?;
    }
    Ok(())
}

fn tui_read_confirmation(
    expected: &str,
    lines: &[String],
    clipboard: &mut dyn ClipboardAccess,
) -> Result<()> {
    let mut input = String::new();
    let mut message = String::new();
    let mut selection = None;
    let mut mouse_gesture: Option<MouseGesture> = None;
    let mut screen = draw_tui_confirmation(lines, expected, &input, &message, None)?;
    loop {
        let redraw = match read_tui_event()? {
            crossterm::event::Event::Key(key) if is_tui_input_key(&key) => {
                if is_copy_shortcut(&key) {
                    if let Err(copy_error) = copy_screen_selection(&screen, selection, clipboard) {
                        message = format!("clipboard copy failed: {copy_error:#}");
                        true
                    } else {
                        false
                    }
                } else if is_paste_shortcut(&key) {
                    false
                } else {
                    selection = None;
                    match key.code {
                        crossterm::event::KeyCode::Enter => {
                            if input.trim() == expected {
                                return Ok(());
                            }
                            bail!("confirmation did not match");
                        }
                        crossterm::event::KeyCode::Esc => {
                            bail!("confirmation did not match")
                        }
                        crossterm::event::KeyCode::Backspace => {
                            input.pop();
                            true
                        }
                        crossterm::event::KeyCode::Char(ch) => {
                            input.push(ch);
                            true
                        }
                        _ => false,
                    }
                }
            }
            crossterm::event::Event::Mouse(mouse) => {
                let is_left_gesture = matches!(
                    mouse.kind,
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
                        | crossterm::event::MouseEventKind::Drag(
                            crossterm::event::MouseButton::Left
                        )
                        | crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left)
                );
                update_modal_screen_selection(mouse, &mut selection, &mut mouse_gesture);
                is_left_gesture
            }
            crossterm::event::Event::Resize(_, _) => {
                selection = None;
                mouse_gesture = None;
                true
            }
            _ => false,
        };
        if redraw {
            screen = draw_tui_confirmation(lines, expected, &input, &message, selection.as_ref())?;
        }
    }
}

fn draw_tui_confirmation(
    lines: &[String],
    expected: &str,
    input: &str,
    message: &str,
    selection: Option<&ScreenSelection>,
) -> Result<RenderedScreen> {
    let mut stdout = io::stdout();
    let mut row = 0_u16;
    let mut rendered_lines = lines.to_vec();
    rendered_lines.push(String::new());
    rendered_lines.push(message.to_string());
    rendered_lines.push(format!("type `{expected}` to confirm: {input}"));
    crossterm::queue!(
        stdout,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0)
    )?;
    for (index, line) in rendered_lines.iter().enumerate() {
        if index == lines.len() + 1 && !line.is_empty() {
            crossterm::queue!(
                stdout,
                crossterm::cursor::MoveTo(0, row),
                crossterm::style::SetForegroundColor(crossterm::style::Color::DarkRed),
                crossterm::style::Print(line),
                crossterm::style::ResetColor
            )?;
            row = row.saturating_add(1);
        } else {
            write_tui_plain_line(&mut stdout, &mut row, line)?;
        }
    }
    let width = crossterm::terminal::size()?.0;
    let screen = RenderedScreen::from_logical_lines(&rendered_lines, width);
    if let Some(selection) = selection {
        queue_screen_selection(&mut stdout, &screen, *selection)?;
    }
    stdout.flush()?;
    Ok(screen)
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
    fn ctrl_shift_c_is_copy_but_not_plain_ctrl_c() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let copy = KeyEvent::new(
            KeyCode::Char('C'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        let cancel = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);

        assert!(is_copy_shortcut(&copy));
        assert!(!is_plain_ctrl_c(&copy));
        assert!(!is_copy_shortcut(&cancel));
        assert!(is_plain_ctrl_c(&cancel));
    }

    #[test]
    fn only_ctrl_shift_v_is_the_session_paste_shortcut() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        assert!(is_paste_shortcut(&KeyEvent::new(
            KeyCode::Char('V'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        )));
        assert!(!is_paste_shortcut(&KeyEvent::new(
            KeyCode::Char('v'),
            KeyModifiers::CONTROL,
        )));
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
    fn rendered_screen_selection_preserves_hard_lines_and_joins_soft_wraps() {
        let screen =
            RenderedScreen::from_logical_lines(&["ab中c".to_string(), "d\t".to_string()], 4);
        let selection = ScreenSelection {
            anchor: ScreenPosition { row: 0, column: 0 },
            active: ScreenPosition {
                row: 3,
                column: u16::MAX,
            },
        };

        assert_eq!(screen.selected_text(selection), "ab中c\nd    ");
        assert_eq!(
            screen.selected_text(ScreenSelection {
                anchor: selection.active,
                active: selection.anchor,
            }),
            "ab中c\nd    "
        );
    }

    #[test]
    fn copying_a_rendered_selection_writes_exact_text_to_the_clipboard() {
        #[derive(Default)]
        struct RecordingClipboard {
            text: Option<String>,
        }

        impl ClipboardAccess for RecordingClipboard {
            fn get_text(&mut self) -> Result<String> {
                unreachable!("copy must not read the clipboard")
            }

            fn set_text(&mut self, text: &str) -> Result<()> {
                self.text = Some(text.to_string());
                Ok(())
            }
        }

        let screen = RenderedScreen::from_logical_lines(&["ab中c".to_string()], 80);
        let selection = ScreenSelection {
            anchor: ScreenPosition { row: 0, column: 1 },
            active: ScreenPosition { row: 0, column: 3 },
        };
        let mut clipboard = RecordingClipboard::default();

        copy_screen_selection(&screen, Some(selection), &mut clipboard).unwrap();

        assert_eq!(clipboard.text.as_deref(), Some("b中"));
    }

    #[test]
    fn screen_copy_failure_preserves_the_selection() {
        struct FailingClipboard;

        impl ClipboardAccess for FailingClipboard {
            fn get_text(&mut self) -> Result<String> {
                unreachable!("copy must not read the clipboard")
            }

            fn set_text(&mut self, _text: &str) -> Result<()> {
                Err(anyhow::anyhow!("clipboard unavailable"))
            }
        }

        let screen = RenderedScreen::from_logical_lines(&["ROOT".to_string()], 80);
        let selection = ScreenSelection {
            anchor: ScreenPosition { row: 0, column: 0 },
            active: ScreenPosition { row: 0, column: 3 },
        };

        assert!(copy_screen_selection(&screen, Some(selection), &mut FailingClipboard).is_err());
        assert_eq!(screen.selected_text(selection), "ROOT");
    }

    #[test]
    fn command_bar_clipboard_text_flattens_line_breaks_and_tabs() {
        assert_eq!(
            normalize_command_bar_clipboard("one\r\ntwo\rthree\nfour\tfive").unwrap(),
            "one two three four five"
        );
    }

    #[test]
    fn invalid_command_bar_clipboard_text_is_rejected_atomically() {
        struct StaticClipboard;

        impl ClipboardAccess for StaticClipboard {
            fn get_text(&mut self) -> Result<String> {
                Ok("safe\u{1b}unsafe".to_string())
            }

            fn set_text(&mut self, _text: &str) -> Result<()> {
                unreachable!("paste must not write the clipboard")
            }
        }

        let mut command = "existing".to_string();
        let error = paste_command_bar(&mut command, &mut StaticClipboard).unwrap_err();

        assert_eq!(command, "existing");
        assert!(error.to_string().contains("unsupported control character"));
    }

    #[test]
    fn clipboard_read_failure_preserves_command_and_editor_state() {
        struct FailingClipboard;

        impl ClipboardAccess for FailingClipboard {
            fn get_text(&mut self) -> Result<String> {
                Err(anyhow::anyhow!("clipboard unavailable"))
            }

            fn set_text(&mut self, _text: &str) -> Result<()> {
                unreachable!("paste must not write the clipboard")
            }
        }

        let mut command = "existing".to_string();
        assert!(paste_command_bar(&mut command, &mut FailingClipboard).is_err());
        assert_eq!(command, "existing");

        let viewport = ViewportSize::new(1, 3);
        let mut editor = EditorModel::new("abcdef\nsecond");
        editor.select(EditorPoint::new(0, 2), EditorPoint::new(1, 3), viewport);
        let before = (
            editor.text(),
            editor.selected_text(),
            editor.position(),
            editor.viewport_offsets(),
        );

        assert!(paste_editor(&mut editor, viewport, &mut FailingClipboard).is_err());
        assert_eq!(
            (
                editor.text(),
                editor.selected_text(),
                editor.position(),
                editor.viewport_offsets(),
            ),
            before
        );
    }

    #[test]
    fn editor_clipboard_normalizes_newlines_and_preserves_tabs_and_spaces() {
        assert_eq!(
            normalize_editor_clipboard("中  \r\n\rnext\t \n").unwrap(),
            "中  \n\nnext\t \n"
        );
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
    fn editor_logical_selection_extracts_exact_multiline_unicode_and_tabs() {
        let mut editor = EditorModel::new("a中b\nc\td\n终");
        let viewport = ViewportSize::new(10, 20);

        editor.select(EditorPoint::new(0, 1), EditorPoint::new(2, 1), viewport);
        assert_eq!(editor.selected_text().as_deref(), Some("中b\nc\td\n终"));

        editor.select(EditorPoint::new(2, 1), EditorPoint::new(0, 1), viewport);
        assert_eq!(editor.selected_text().as_deref(), Some("中b\nc\td\n终"));
    }

    #[test]
    fn editor_movement_clears_selection_and_moves_from_drag_endpoint() {
        let mut editor = EditorModel::new("abcdef");
        let viewport = ViewportSize::new(10, 20);
        editor.select(EditorPoint::new(0, 1), EditorPoint::new(0, 4), viewport);

        editor.apply(EditorAction::MoveLeft, viewport);

        assert_eq!(editor.position(), EditorPosition::new(1, 4));
        assert_eq!(editor.selected_text(), None);
    }

    #[test]
    fn editor_mouse_cell_maps_wide_characters_tabs_and_line_end() {
        let editor = EditorModel::new("a中\tb");

        assert_eq!(editor.point_for_viewport_cell(0, 0), EditorPoint::new(0, 0));
        assert_eq!(editor.point_for_viewport_cell(0, 1), EditorPoint::new(0, 1));
        assert_eq!(editor.point_for_viewport_cell(0, 2), EditorPoint::new(0, 1));
        assert_eq!(editor.point_for_viewport_cell(0, 4), EditorPoint::new(0, 2));
        assert_eq!(
            editor.point_for_viewport_cell(0, 99),
            EditorPoint::new(0, 4)
        );
    }

    #[test]
    fn editor_mouse_drag_is_direction_independent() {
        let viewport = ViewportSize::new(10, 20);
        let low = EditorPoint::new(0, 1);
        let high = EditorPoint::new(0, 4);

        let mut forward = EditorModel::new("abcdef");
        let (anchor, active) = editor_drag_selection_points(&forward, low, high);
        forward.select(anchor, active, viewport);

        let mut backward = EditorModel::new("abcdef");
        let (anchor, active) = editor_drag_selection_points(&backward, high, low);
        backward.select(anchor, active, viewport);

        assert_eq!(forward.selected_text().as_deref(), Some("bcde"));
        assert_eq!(backward.selected_text(), forward.selected_text());
    }

    #[test]
    fn copying_editor_selection_writes_exact_logical_text() {
        #[derive(Default)]
        struct RecordingClipboard(Option<String>);

        impl ClipboardAccess for RecordingClipboard {
            fn get_text(&mut self) -> Result<String> {
                unreachable!("copy must not read the clipboard")
            }

            fn set_text(&mut self, text: &str) -> Result<()> {
                self.0 = Some(text.to_string());
                Ok(())
            }
        }

        let viewport = ViewportSize::new(10, 20);
        let mut editor = EditorModel::new("a中\tb\nnext");
        editor.select(EditorPoint::new(0, 1), EditorPoint::new(1, 2), viewport);
        let mut clipboard = RecordingClipboard::default();

        copy_editor_selection(&editor, &mut clipboard).unwrap();

        assert_eq!(clipboard.0.as_deref(), Some("中\tb\nne"));
        assert_eq!(editor.selected_text().as_deref(), Some("中\tb\nne"));
    }

    #[test]
    fn typing_replaces_selection_and_leaves_caret_at_former_start() {
        let viewport = ViewportSize::new(10, 20);
        let mut editor = EditorModel::new("abc\ndef");
        editor.select(EditorPoint::new(0, 1), EditorPoint::new(1, 2), viewport);

        editor.apply(EditorAction::Insert('X'), viewport);

        assert_eq!(editor.text(), "aXf");
        assert_eq!(editor.position(), EditorPosition::new(1, 2));
        assert_eq!(editor.selected_text(), None);
    }

    #[test]
    fn editor_paste_replaces_selection_with_normalized_multiline_text() {
        struct StaticClipboard;

        impl ClipboardAccess for StaticClipboard {
            fn get_text(&mut self) -> Result<String> {
                Ok("中\r\n\t ".to_string())
            }

            fn set_text(&mut self, _text: &str) -> Result<()> {
                unreachable!("paste must not write the clipboard")
            }
        }

        let viewport = ViewportSize::new(10, 20);
        let mut editor = EditorModel::new("abc\ndef");
        editor.select(EditorPoint::new(0, 1), EditorPoint::new(1, 2), viewport);

        paste_editor(&mut editor, viewport, &mut StaticClipboard).unwrap();

        assert_eq!(editor.text(), "a中\n\t f");
        assert_eq!(editor.position(), EditorPosition::new(1, 2));
        assert_eq!(editor.selected_text(), None);
    }

    #[test]
    fn editor_selection_handles_tab_enter_backspace_and_delete() {
        let viewport = ViewportSize::new(10, 20);
        for (action, expected) in [
            (EditorAction::Insert('\t'), "a\tf"),
            (EditorAction::Enter, "a\nf"),
            (EditorAction::Backspace, "af"),
            (EditorAction::Delete, "af"),
        ] {
            let mut editor = EditorModel::new("abc\ndef");
            editor.select(EditorPoint::new(0, 1), EditorPoint::new(1, 2), viewport);

            editor.apply(action, viewport);

            assert_eq!(editor.text(), expected);
            assert_eq!(editor.position(), EditorPosition::new(1, 2));
            assert_eq!(editor.selected_text(), None);
        }
    }

    #[test]
    fn invalid_editor_paste_preserves_buffer_selection_caret_and_viewport() {
        struct InvalidClipboard;

        impl ClipboardAccess for InvalidClipboard {
            fn get_text(&mut self) -> Result<String> {
                Ok("unsafe\u{7}".to_string())
            }

            fn set_text(&mut self, _text: &str) -> Result<()> {
                unreachable!("paste must not write the clipboard")
            }
        }

        let viewport = ViewportSize::new(1, 3);
        let mut editor = EditorModel::new("abcdef\nsecond");
        editor.select(EditorPoint::new(0, 2), EditorPoint::new(1, 3), viewport);
        let before = (
            editor.text(),
            editor.selected_text(),
            editor.position(),
            editor.viewport_offsets(),
        );

        assert!(paste_editor(&mut editor, viewport, &mut InvalidClipboard).is_err());

        assert_eq!(
            (
                editor.text(),
                editor.selected_text(),
                editor.position(),
                editor.viewport_offsets(),
            ),
            before
        );
    }

    #[test]
    fn editor_paste_without_selection_inserts_at_caret() {
        struct StaticClipboard;

        impl ClipboardAccess for StaticClipboard {
            fn get_text(&mut self) -> Result<String> {
                Ok("中\nX".to_string())
            }

            fn set_text(&mut self, _text: &str) -> Result<()> {
                unreachable!("paste must not write the clipboard")
            }
        }

        let viewport = ViewportSize::new(10, 20);
        let mut editor = EditorModel::new("ab");
        editor.apply(EditorAction::MoveRight, viewport);

        paste_editor(&mut editor, viewport, &mut StaticClipboard).unwrap();

        assert_eq!(editor.text(), "a中\nXb");
        assert_eq!(editor.position(), EditorPosition::new(2, 2));
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
