use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Where the Pointer is decides which commands are available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PointerContext {
    Root,
    /// A Topic Card or Literature Card.
    TreeRoot,
    Regular,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Availability {
    Always,
    OnCard,
    OnRegular,
}

const COMMANDS: &[(&str, &str, Availability)] = &[
    (
        "go [<target>]",
        "go to a Location or Citation key; bare go returns to ROOT",
        Availability::Always,
    ),
    ("root", "return to ROOT", Availability::Always),
    (
        "ws [<workspace>]",
        "list workspaces, or switch to the human or agent workspace ROOT",
        Availability::Always,
    ),
    (
        "up",
        "go to the parent card; a tree root goes to ROOT",
        Availability::OnCard,
    ),
    (
        "back",
        "go back to the card you were at before",
        Availability::Always,
    ),
    (
        "fwd",
        "go forward again to the card you went back from",
        Availability::Always,
    ),
    (
        "ls",
        "list Topics and Literature at ROOT, or the current tree",
        Availability::Always,
    ),
    (
        "t <title>",
        "create a Topic in the current workspace",
        Availability::Always,
    ),
    (
        "l",
        "create a Literature Card in the current workspace from one BibTeX entry",
        Availability::Always,
    ),
    ("n", "create the direct successor", Availability::OnCard),
    (
        "b",
        "create the next side successor",
        Availability::OnRegular,
    ),
    ("e", "edit the current card", Availability::OnCard),
    (
        "del [keep-root]",
        "delete the current card and its successors; at a Topic or Literature Card, keep-root empties the tree and keeps its root",
        Availability::OnCard,
    ),
    (
        "ins",
        "insert a new card in the current card's place; it and its successors move down",
        Availability::OnRegular,
    ),
    (
        "mv [alone] <new-location>",
        "move the current card and its successors (alone: only the card; its chain closes up)",
        Availability::OnRegular,
    ),
    (
        "mv [alone] before <location>",
        "move the current card into another card's place; that card moves down",
        Availability::OnRegular,
    ),
    (
        "ty <idea|event|finding|model|hub|term|cited> [<facet>]",
        "make the current card an Idea, Event, Finding, Model, Hub, Term, or Cited Card; a facet (any short label) makes it a Model part",
        Availability::OnRegular,
    ),
    (
        "lv <level|none>",
        "set the level of the current Model Card (concept or formal) or Hub Card (line, topic, field, or register)",
        Availability::OnRegular,
    ),
    (
        "mf <form|none>",
        "set the Math form of the current Model Card: theory, definition, axiom, theorem, proposition, lemma, or corollary",
        Availability::OnRegular,
    ),
    ("stats", "count cards by kind", Availability::Always),
    ("status", "show service status", Availability::Always),
    ("lsbk", "list broken links", Availability::Always),
    (
        "help",
        "show the commands available here",
        Availability::Always,
    ),
    ("q", "quit the session", Availability::Always),
];

fn command_available(availability: Availability, context: PointerContext) -> bool {
    match availability {
        Availability::Always => true,
        Availability::OnCard => context != PointerContext::Root,
        Availability::OnRegular => context == PointerContext::Regular,
    }
}

pub(super) fn help_for(context: PointerContext) -> Help {
    let commands = COMMANDS
        .iter()
        .filter(|(_, _, availability)| command_available(*availability, context))
        .map(|(syntax, description, _)| (*syntax, *description))
        .collect::<Vec<_>>();
    Help {
        summary: commands
            .iter()
            .map(|(syntax, _)| *syntax)
            .collect::<Vec<_>>()
            .join(" | "),
        commands,
    }
}

/// Print the warnings of an accepted write, compactly, above the refreshed view (R0176).
fn show_check(accepted: &crate::gate::Accepted) {
    for line in accepted.lines() {
        println!("check: {line}");
    }
}

pub(super) fn pointer_context(root: &Path, pointer: &Pointer) -> Result<PointerContext> {    match pointer {
        Pointer::Root => Ok(PointerContext::Root),
        Pointer::Card(target) => {
            let conn = crate::open_db(root)?;
            let card = crate::load_card(&conn, target)?;
            Ok(if crate::is_tree_root(&card) {
                PointerContext::TreeRoot
            } else {
                PointerContext::Regular
            })
        }
    }
}

pub(super) trait Interaction {
    fn edit(
        &mut self,
        initial_text: &str,
        header: &str,
        save: &mut dyn FnMut(&str) -> Result<()>,
    ) -> Result<bool>;

    fn choose_literature_edit_part(&mut self) -> Result<crate::EditPart>;

    fn confirm(&mut self, expected: &str, lines: &[String]) -> Result<()>;

    /// Present one accepted write's warnings in the front end's own convention (R0176,
    /// S15-AC05). The line Session prints them above its refreshed view; the TUI collects them
    /// for its status row, because printing into the alternate screen would erase them.
    fn check(&mut self, accepted: &crate::gate::Accepted) {
        show_check(accepted);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Pointer {
    Root,
    Card(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SessionState {
    pointer: Pointer,
    /// The Workspace shown at ROOT and given new Topics and Literature. It
    /// follows the Pointer into whichever Workspace holds its Card.
    workspace: crate::Workspace,
    /// Where the Pointer was before each move, newest last, for `back`.
    back: Vec<Visit>,
    /// Where `back` came from, newest last, for `fwd`; a new move clears it.
    forward: Vec<Visit>,
}

/// One place the Pointer has been: the Card (or ROOT) and the Workspace shown.
type Visit = (Pointer, crate::Workspace);

/// How many earlier places `back` remembers.
const HISTORY_LIMIT: usize = 100;

impl SessionState {
    pub(super) fn root() -> Self {
        Self {
            pointer: Pointer::Root,
            workspace: crate::Workspace::Human,
            back: Vec::new(),
            forward: Vec::new(),
        }
    }

    pub(super) fn pointer(&self) -> &Pointer {
        &self.pointer
    }

    pub(super) fn workspace(&self) -> crate::Workspace {
        self.workspace
    }

    pub(super) fn follow_link(&mut self, root: &Path, target: String) {
        let before = self.visit();
        self.pointer = Pointer::Card(target);
        self.sync_workspace(root);
        self.remember(before);
    }

    fn visit(&self) -> Visit {
        (self.pointer.clone(), self.workspace)
    }

    /// Record the place the Pointer just left, as a file browser does: a new
    /// move forgets the places `back` came from.
    fn remember(&mut self, before: Visit) {
        if before.0 == self.pointer {
            return;
        }
        self.back.push(before);
        if self.back.len() > HISTORY_LIMIT {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    /// Step `back` (or with `forward`, `fwd`) to the nearest remembered place
    /// that still exists; places whose Card was deleted are skipped.
    fn step(&mut self, root: &Path, forward: bool) -> Result<()> {
        let conn = crate::open_db(root)?;
        loop {
            let next = if forward {
                self.forward.pop()
            } else {
                self.back.pop()
            };
            let Some((pointer, workspace)) = next else {
                bail!(
                    "{}",
                    if forward {
                        "nothing to go forward to; fwd follows back"
                    } else {
                        "no earlier card to go back to"
                    }
                );
            };
            if let Pointer::Card(target) = &pointer
                && !crate::target_exists(&conn, target)?
            {
                continue;
            }
            let here = self.visit();
            if forward {
                self.back.push(here);
            } else {
                self.forward.push(here);
            }
            self.pointer = pointer;
            self.workspace = workspace;
            return Ok(());
        }
    }

    /// Adopt the Workspace of the Card under the Pointer, if there is one.
    pub(super) fn sync_workspace(&mut self, root: &Path) {
        if let Pointer::Card(target) = &self.pointer
            && let Ok(conn) = crate::open_db(root)
            && let Ok(card) = crate::load_card(&conn, target)
        {
            self.workspace = card.workspace;
        }
    }

    fn current_card(&self, command: &str) -> Result<String> {
        match &self.pointer {
            Pointer::Root => bail!("{command} needs a Card; use go <target> first"),
            Pointer::Card(target) => Ok(target.clone()),
        }
    }

    #[cfg(test)]
    fn at(target: &str) -> Self {
        Self {
            pointer: Pointer::Card(target.to_string()),
            workspace: crate::Workspace::Human,
            back: Vec::new(),
            forward: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CardSummary {
    pub(super) address: String,
    pub(super) title: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Listing {
    pub(super) heading: String,
    pub(super) cards: Vec<CardSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Help {
    pub(super) summary: String,
    pub(super) commands: Vec<(&'static str, &'static str)>,
}

impl Help {
    pub(super) fn lines(&self) -> Vec<String> {
        let width = self
            .commands
            .iter()
            .map(|(syntax, _)| syntax.len())
            .max()
            .unwrap_or(0);
        std::iter::once(format!("commands: {}", self.summary))
            .chain(
                self.commands
                    .iter()
                    .map(|(syntax, description)| format!("  {syntax:<width$}  {description}")),
            )
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Stats {
    pub(super) total: i64,
    pub(super) topics: i64,
    pub(super) regular: i64,
    pub(super) literature: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Status {
    pub(super) pid: String,
    pub(super) archive_root: String,
    pub(super) sqlite: String,
    pub(super) cards: i64,
    /// The `locks:` line: the active promise and its integrity, exactly as `zt status` reports it
    /// (R044, R234, S08-AC04).
    pub(super) locks: String,
    pub(super) sessions: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CommandResult {
    Noop,
    Navigated,
    Changed,
    List(Listing),
    Stats(Stats),
    Status(Status),
    BrokenLinks(Vec<crate::link::BrokenLink>),
    Help(Help),
    Workspaces(Vec<WorkspaceSummary>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WorkspaceSummary {
    pub(super) workspace: crate::Workspace,
    pub(super) counts: crate::workspace::WorkspaceCounts,
    pub(super) current: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Execution {
    Continue(CommandResult),
    Exit,
}

/// If another process removed the Pointer's Card, move the Pointer to its
/// nearest existing ancestor (or ROOT) and describe what happened.
pub(super) fn recover_missing_pointer(
    root: &Path,
    state: &mut SessionState,
) -> Result<Option<String>> {
    let Pointer::Card(target) = &state.pointer else {
        return Ok(None);
    };
    let conn = crate::open_db(root)?;
    if crate::target_exists(&conn, target)? {
        return Ok(None);
    }
    let missing = target.clone();
    let mut ancestor = crate::parent_location(&missing);
    while let Some(candidate) = &ancestor {
        if crate::target_exists(&conn, candidate)? {
            break;
        }
        ancestor = crate::parent_location(candidate);
    }
    state.pointer = ancestor.map(Pointer::Card).unwrap_or(Pointer::Root);
    Ok(Some(format!("card `{missing}` no longer exists")))
}

pub(super) fn execute(
    root: &Path,
    state: &mut SessionState,
    line: &str,
    interaction: &mut dyn Interaction,
) -> Result<Execution> {
    let before = state.visit();
    let execution = execute_command(root, state, line, interaction);
    state.sync_workspace(root);
    // Every move of the Pointer is remembered, except the steps through that memory.
    if !matches!(line.split_whitespace().next(), Some("back" | "fwd")) {
        state.remember(before);
    }
    execution
}

fn execute_command(
    root: &Path,
    state: &mut SessionState,
    line: &str,
    interaction: &mut dyn Interaction,
) -> Result<Execution> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    match parts.as_slice() {
        [] => Ok(Execution::Continue(CommandResult::Noop)),
        ["q"] => Ok(Execution::Exit),
        ["go"] | ["root"] => {
            state.pointer = Pointer::Root;
            Ok(Execution::Continue(CommandResult::Navigated))
        }
        ["go", target] => {
            let conn = crate::open_db(root)?;
            if !crate::target_exists(&conn, target)? {
                bail!("target `{target}` does not exist");
            }
            state.pointer = Pointer::Card((*target).to_string());
            Ok(Execution::Continue(CommandResult::Navigated))
        }
        ["go", _, _, ..] => bail!("usage: go [<target>]"),
        ["ws"] => {
            let conn = crate::open_db(root)?;
            let cards = crate::load_cards(&conn)?;
            Ok(Execution::Continue(CommandResult::Workspaces(
                crate::Workspace::ALL
                    .into_iter()
                    .map(|workspace| WorkspaceSummary {
                        workspace,
                        counts: crate::workspace::counts(&cards, workspace),
                        current: workspace == state.workspace,
                    })
                    .collect(),
            )))
        }
        ["ws", name] => {
            state.workspace = crate::Workspace::parse(name)?;
            state.pointer = Pointer::Root;
            Ok(Execution::Continue(CommandResult::Navigated))
        }
        ["ws", ..] => bail!("usage: ws [<workspace>]"),
        ["up"] => {
            if let Pointer::Card(target) = &state.pointer {
                state.pointer = crate::parent_location(target)
                    .map(Pointer::Card)
                    .unwrap_or(Pointer::Root);
            }
            Ok(Execution::Continue(CommandResult::Navigated))
        }
        ["up", ..] => bail!("usage: up"),
        ["back"] => {
            state.step(root, false)?;
            Ok(Execution::Continue(CommandResult::Navigated))
        }
        ["back", ..] => bail!("usage: back"),
        ["fwd"] => {
            state.step(root, true)?;
            Ok(Execution::Continue(CommandResult::Navigated))
        }
        ["fwd", ..] => bail!("usage: fwd"),
        ["ls"] => {
            let conn = crate::open_db(root)?;
            let (heading, cards) = super::cards_for_list(&conn, state.pointer(), state.workspace)?;
            let cards = cards
                .into_iter()
                .map(|card| {
                    let parsed = crate::parse_card_text(&card.text)?;
                    Ok(CardSummary {
                        address: card.address,
                        title: parsed.title,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Execution::Continue(CommandResult::List(Listing {
                heading,
                cards,
            })))
        }
        ["stats"] => {
            let counts = crate::card_counts(root)?;
            Ok(Execution::Continue(CommandResult::Stats(Stats {
                total: counts.total,
                topics: counts.topics,
                regular: counts.regular,
                literature: counts.literature,
            })))
        }
        ["status"] => {
            let counts = crate::card_counts(root)?;
            let pid = fs::read_to_string(crate::service_path(root))
                .unwrap_or_else(|_| "unknown".to_string());
            Ok(Execution::Continue(CommandResult::Status(Status {
                pid: pid.trim().to_string(),
                archive_root: root.display().to_string(),
                sqlite: crate::db_path(root).display().to_string(),
                cards: counts.total,
                locks: crate::lock_status_line(root),
                sessions: super::session_count(root),
            })))
        }
        ["lsbk"] => {
            let conn = crate::open_db(root)?;
            Ok(Execution::Continue(CommandResult::BrokenLinks(
                crate::link_lifecycle(&conn)?.broken_links()?,
            )))
        }
        ["help"] => Ok(Execution::Continue(CommandResult::Help(help_for(
            pointer_context(root, state.pointer())?,
        )))),
        ["t", title @ ..] => {
            let title = title.join(" ");
            if title.trim().is_empty() || title.contains('\n') || title.contains('\r') {
                bail!("topic title must be non-empty single-line text");
            }
            let _lock = crate::acquire_edit_lock(root, "session Topic")?;
            let mut conn = crate::open_db(root)?;
            let workspace = state.workspace;
            let mut created = None;
            let mut accepted = None;
            let mut save = |text: &str| {
                let (location, shown) = crate::create_topic(&mut conn, workspace, text)?;
                accepted = Some(shown);
                created = Some(location);
                Ok(())
            };
            if interaction.edit(&crate::topic_template(&title), "edit mode", &mut save)?
                && let Some(location) = created
            {
                state.pointer = Pointer::Card(location);
            }
            if let Some(accepted) = accepted {
                interaction.check(&accepted);
            }
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["n"] => {
            let at = state.current_card("n")?;
            let _lock = crate::acquire_edit_lock(root, "session n")?;
            let mut conn = crate::open_db(root)?;
            let parent = crate::load_card(&conn, &at)?;
            let location = crate::direct_successor(&parent.address)?;
            if crate::location_exists(&conn, &location)? {
                bail!("direct successor already exists: {location}");
            }
            // R232 (S08-AC02): a Locked destination refuses before the editor opens. The writer
            // checks again, because an editor can outlive the view this precheck reads.
            crate::tree_lock::ensure_unlocked(&conn, crate::tree_lock::Change::CreateAt(&location))?;
            let mut accepted = None;
            let mut save = |text: &str| {
                accepted = Some(crate::insert_card(
                    &mut conn,
                    &location,
                    false,
                    parent.workspace,
                    crate::CardType::Idea,
                    None,
                    text,
                )?);
                Ok(())
            };
            if interaction.edit(&crate::regular_template(), "edit mode", &mut save)? {
                state.pointer = Pointer::Card(location);
            }
            if let Some(accepted) = accepted {
                interaction.check(&accepted);
            }
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["b"] => {
            let at = state.current_card("b")?;
            let _lock = crate::acquire_edit_lock(root, "session b")?;
            let mut conn = crate::open_db(root)?;
            let parent = crate::load_card(&conn, &at)?;
            crate::ensure_side_parent(&parent)?;
            let location = crate::next_side_successor(&conn, &parent.address, &BTreeSet::new())?;
            crate::tree_lock::ensure_unlocked(&conn, crate::tree_lock::Change::CreateAt(&location))?;
            let mut accepted = None;
            let mut save = |text: &str| {
                accepted = Some(crate::insert_card(
                    &mut conn,
                    &location,
                    false,
                    parent.workspace,
                    crate::CardType::Idea,
                    None,
                    text,
                )?);
                Ok(())
            };
            if interaction.edit(&crate::regular_template(), "edit mode", &mut save)? {
                state.pointer = Pointer::Card(location);
            }
            if let Some(accepted) = accepted {
                interaction.check(&accepted);
            }
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["l"] => {
            let _lock = crate::acquire_edit_lock(root, "session l")?;
            let mut conn = crate::open_db(root)?;
            let mut accepted = None;
            let mut save_metadata = |text: &str| {
                let metadata = crate::literature::parse_metadata(text)?;
                if crate::citation_key_exists_case_insensitive(&conn, &metadata.citation_key)? {
                    bail!("Literature Card `{}` already exists", metadata.citation_key);
                }
                accepted = Some((text.to_string(), metadata));
                Ok(())
            };
            if !interaction.edit("", "metadata edit", &mut save_metadata)? {
                return Ok(Execution::Continue(CommandResult::Changed));
            }
            let (bibtex, metadata) =
                accepted.context("validated Literature metadata is missing")?;
            let initial_text = crate::card::card_template(&metadata.title);
            let citation_key = metadata.citation_key.clone();
            let workspace = state.workspace;
            let mut checked = None;
            let mut save_text = |edited: &str| {
                let parsed = crate::parse_literature_edit_text(edited)?;
                let text = crate::compose_card_text(&metadata.title, &parsed.body, "");
                checked = Some(crate::insert_literature_card(
                    &mut conn,
                    &citation_key,
                    workspace,
                    &bibtex,
                    &text,
                )?);
                Ok(())
            };
            if interaction.edit(&initial_text, "edit mode", &mut save_text)? {
                state.pointer = Pointer::Card(citation_key);
            }
            if let Some(checked) = checked {
                interaction.check(&checked);
            }
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["e"] => {
            let at = state.current_card("e")?;
            let _lock = crate::acquire_edit_lock(root, "session e")?;
            let mut conn = crate::open_db(root)?;
            let card = crate::load_card(&conn, &at)?;
            // R232 (S08-AC02): the Locked Card refuses before the part choice or the editor opens.
            // The writers check again, so an editor opened before a Lock cannot save into it.
            crate::tree_lock::ensure_unlocked(&conn, crate::tree_lock::Change::Edit(card.row_id))?;
            if card.is_lit {
                return match interaction.choose_literature_edit_part()? {
                    crate::EditPart::Metadata => {
                        let old_key = card
                            .citation_key
                            .as_deref()
                            .context("Literature Card is missing its Citation key")?;
                        let initial = card
                            .bibtex
                            .as_deref()
                            .context("Literature Card is missing its BibTeX metadata")?;
                        let mut accepted = None;
                        let mut save = |text: &str| {
                            let metadata = crate::literature::parse_metadata(text)?;
                            if metadata.citation_key != old_key
                                && (metadata.citation_key.eq_ignore_ascii_case(old_key)
                                    || crate::citation_key_conflicts(
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
                        };
                        if !interaction.edit(initial, "metadata edit", &mut save)? {
                            return Ok(Execution::Continue(CommandResult::Changed));
                        }
                        let (bibtex, metadata) =
                            accepted.context("validated Literature metadata is missing")?;
                        if metadata.citation_key != old_key {
                            let lines = crate::citation_rename_lines(
                                &conn,
                                old_key,
                                &metadata.citation_key,
                            )?;
                            interaction.confirm("move", &lines)?;
                        }
                        let accepted = crate::update_literature_metadata(
                            &mut conn,
                            &card,
                            &metadata.citation_key,
                            &bibtex,
                            &metadata.title,
                        )?;
                        interaction.check(&accepted);
                        state.pointer = Pointer::Card(metadata.citation_key);
                        Ok(Execution::Continue(CommandResult::Changed))
                    }
                    crate::EditPart::Text => {
                        let mut accepted = None;
                        let mut save = |text: &str| {
                            accepted = Some(crate::update_literature_text(&mut conn, &card, text)?);
                            Ok(())
                        };
                        interaction.edit(&card.text, "edit mode", &mut save)?;
                        if let Some(accepted) = accepted {
                            interaction.check(&accepted);
                        }
                        Ok(Execution::Continue(CommandResult::Changed))
                    }
                };
            }
            let mut accepted = None;
            let mut save = |text: &str| {
                accepted = Some(crate::update_card_text(&mut conn, &card, text)?);
                Ok(())
            };
            interaction.edit(&card.text, "edit mode", &mut save)?;
            if let Some(accepted) = accepted {
                interaction.check(&accepted);
            }
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["del", rest @ ..] => {
            let keep_root = match rest {
                [] => false,
                ["keep-root"] => true,
                _ => bail!("usage: del [keep-root]"),
            };
            let at = state.current_card("del")?;
            let _lock = crate::acquire_edit_lock(root, "session del")?;
            let mut conn = crate::open_db(root)?;
            let plan = if keep_root {
                crate::keep_root_delete_plan(&conn, &at)?
            } else {
                crate::delete_plan(&conn, &at)?
            };
            // R232 (S08-AC02): no confirmation is asked for a deletion the Lock forbids. R030: a
            // Locked tree is refused even when its delete set is empty.
            crate::tree_lock::ensure_unlocked(&conn, crate::tree_lock::Change::Delete(&plan))?;
            interaction.confirm(&plan.confirmation, &crate::delete_verification_lines(&plan))?;
            let result = crate::apply_delete_plan(&mut conn, plan)?;
            state.pointer = match result.root_kept {
                Some(root_address) => Pointer::Card(root_address),
                None => crate::parent_location(&at)
                    .map(Pointer::Card)
                    .unwrap_or(Pointer::Root),
            };
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["mv", rest @ ..] if !rest.is_empty() => {
            let at = state.current_card("mv")?;
            let (alone, rest) = match rest {
                ["alone", rest @ ..] => (true, rest),
                rest => (false, rest),
            };
            let target = match rest {
                ["before", before] => crate::place::Target::Before((*before).to_string()),
                [to] if *to != "before" && *to != "alone" => {
                    crate::place::Target::To((*to).to_string())
                }
                _ => bail!("usage: mv [alone] <new-location> | mv [alone] before <location>"),
            };
            let _lock = crate::acquire_edit_lock(root, "session mv")?;
            let mut conn = crate::open_db(root)?;
            let plan = crate::place::move_plan(&conn, &at, &target, alone)?;
            // R232 (S08-AC02): the complete mapping refuses before the confirmation prompt.
            crate::tree_lock::ensure_unlocked(&conn, crate::tree_lock::Change::Mapping(&plan.mapping))?;
            interaction.confirm("move", &crate::move_verification_lines(&plan))?;
            crate::apply_move_plan(&mut conn, &plan)?;
            let moved_to = plan.mapping.get(&at).cloned().unwrap_or(at);
            state.pointer = Pointer::Card(moved_to);
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["mv"] => bail!("usage: mv [alone] <new-location> | mv [alone] before <location>"),
        ["ins"] => {
            let at = state.current_card("ins")?;
            let _lock = crate::acquire_edit_lock(root, "session ins")?;
            let mut conn = crate::open_db(root)?;
            let plan = crate::place::insert_plan(&conn, &at)?;
            // R232 (S08-AC02): the renumbering and the new Card's destination are both checked
            // before the confirmation or the editor opens.
            crate::tree_lock::ensure_unlocked(&conn, crate::tree_lock::Change::Mapping(&plan.mapping))?;
            crate::tree_lock::ensure_unlocked(&conn, crate::tree_lock::Change::CreateAt(&plan.location))?;
            interaction.confirm("insert", &crate::place::insert_verification_lines(&plan))?;
            let mut accepted = None;
            let mut save = |text: &str| {
                accepted = Some(crate::place::apply_insert_plan(
                    &mut conn,
                    &plan,
                    crate::CardType::Idea,
                    None,
                    None,
                    text,
                )?);
                Ok(())
            };
            if interaction.edit(&crate::regular_template(), "edit mode", &mut save)? {
                state.pointer = Pointer::Card(plan.location.clone());
            }
            if let Some(accepted) = accepted {
                interaction.check(&accepted);
            }
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["ty", name, label @ ..] => {
            let at = state.current_card("ty")?;
            let card_type = crate::CardType::parse(name)?;
            // A facet is the rest of the line, so it may hold spaces.
            let facet = match label {
                [] => None,
                _ if card_type == crate::CardType::Model => {
                    Some(crate::model::Facet::parse(&label.join(" "))?)
                }
                _ => bail!("only Model Cards can have a Model facet"),
            };
            let _lock = crate::acquire_edit_lock(root, "session ty")?;
            let mut conn = crate::open_db(root)?;
            let card = crate::load_card(&conn, &at)?;
            // A part keeps its emphasis while it stays a part. The type change is one combined
            // candidate through the same gate as every other authoring route (R0163).
            let emphasis = card.emphasis.filter(|_| facet.is_some());
            let change = crate::ops::TypeChange {
                card_type: Some(card_type),
                facet: Some(facet),
                emphasis: Some(emphasis),
                ..crate::ops::TypeChange::default()
            };
            let tx = conn.transaction()?;
            let (address, changed, accepted) = crate::ops::edit_tx(
                &tx,
                crate::Access::Owner,
                &at,
                None,
                change,
                None,
            )?;
            if changed {
                crate::link_index::source_changed(&tx, &address)?;
            }
            tx.commit()?;
            interaction.check(&accepted);
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["ty", ..] => bail!("usage: ty <idea|event|finding|model|hub|term|cited> [<facet>]"),
        ["lv", value] => {
            let at = state.current_card("lv")?;
            let _lock = crate::acquire_edit_lock(root, "session lv")?;
            let mut conn = crate::open_db(root)?;
            let card = crate::load_card(&conn, &at)?;
            let change = if card.card_type == crate::CardType::Hub {
                let level = if value.eq_ignore_ascii_case("none") {
                    None
                } else {
                    Some(crate::hub::HubLevel::parse(value)?)
                };
                crate::ops::TypeChange {
                    hub_level: Some(level),
                    ..crate::ops::TypeChange::default()
                }
            } else {
                let level = crate::model::parse_optional_level(value)?;
                crate::ops::TypeChange {
                    level: Some(level),
                    ..crate::ops::TypeChange::default()
                }
            };
            let tx = conn.transaction()?;
            let (address, changed, accepted) =
                crate::ops::edit_tx(&tx, crate::Access::Owner, &at, None, change, None)?;
            if changed {
                crate::link_index::source_changed(&tx, &address)?;
            }
            tx.commit()?;
            interaction.check(&accepted);
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["lv", ..] => bail!("usage: lv <level|none>"),
        ["mf", value] => {
            let at = state.current_card("mf")?;
            let form = crate::math::parse_optional_form(value)?;
            let _lock = crate::acquire_edit_lock(root, "session mf")?;
            let mut conn = crate::open_db(root)?;
            let change = crate::ops::TypeChange {
                form: Some(form),
                ..crate::ops::TypeChange::default()
            };
            let tx = conn.transaction()?;
            let (address, changed, accepted) =
                crate::ops::edit_tx(&tx, crate::Access::Owner, &at, None, change, None)?;
            if changed {
                crate::link_index::source_changed(&tx, &address)?;
            }
            tx.commit()?;
            interaction.check(&accepted);
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["mf", ..] => bail!("usage: mf <form|none>"),
        [name, ..] => bail!("unknown session command: {name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct NoopInteraction;

    impl Interaction for NoopInteraction {
        fn edit(
            &mut self,
            _initial_text: &str,
            _header: &str,
            _save: &mut dyn FnMut(&str) -> Result<()>,
        ) -> Result<bool> {
            panic!("read-only command unexpectedly requested editing")
        }

        fn choose_literature_edit_part(&mut self) -> Result<crate::EditPart> {
            panic!("read-only command unexpectedly requested an edit choice")
        }

        fn confirm(&mut self, _expected: &str, _lines: &[String]) -> Result<()> {
            panic!("read-only command unexpectedly requested confirmation")
        }
    }

    fn execute_without_interaction(
        root: &Path,
        state: &mut SessionState,
        line: &str,
    ) -> Result<Execution> {
        execute(root, state, line, &mut NoopInteraction)
    }

    struct ScriptedInteraction {
        edits: VecDeque<Option<String>>,
        choices: VecDeque<crate::EditPart>,
        confirmations: VecDeque<String>,
    }

    impl Interaction for ScriptedInteraction {
        fn edit(
            &mut self,
            _initial_text: &str,
            _header: &str,
            save: &mut dyn FnMut(&str) -> Result<()>,
        ) -> Result<bool> {
            let Some(text) = self.edits.pop_front().expect("scripted edit") else {
                return Ok(false);
            };
            save(&text)?;
            Ok(true)
        }

        fn choose_literature_edit_part(&mut self) -> Result<crate::EditPart> {
            Ok(self.choices.pop_front().expect("scripted edit choice"))
        }

        fn confirm(&mut self, expected: &str, _lines: &[String]) -> Result<()> {
            let actual = self
                .confirmations
                .pop_front()
                .expect("scripted confirmation");
            if actual == expected {
                Ok(())
            } else {
                bail!("confirmation did not match")
            }
        }
    }

    #[test]
    fn bare_go_returns_to_root_and_extra_arguments_preserve_the_pointer() {
        let mut state = SessionState::at("0/1");

        let error = execute_without_interaction(Path::new("unused"), &mut state, "go 0/0 extra")
            .expect_err("extra go arguments must fail");
        assert_eq!(error.to_string(), "usage: go [<target>]");
        assert_eq!(state.pointer(), &Pointer::Card("0/1".to_string()));

        assert_eq!(
            execute_without_interaction(Path::new("unused"), &mut state, "  go  ")
                .expect("bare go"),
            Execution::Continue(CommandResult::Navigated)
        );
        assert_eq!(state.pointer(), &Pointer::Root);
    }

    #[test]
    fn go_moves_only_to_an_existing_location_or_citation_key() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)\
             VALUES('0/0', NULL, 1, 0, NULL, '# Topic\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert target");
        drop(conn);
        let mut state = SessionState::root();

        assert_eq!(
            execute_without_interaction(temp.path(), &mut state, "go 0/0").expect("go to target"),
            Execution::Continue(CommandResult::Navigated)
        );
        assert_eq!(state.pointer(), &Pointer::Card("0/0".to_string()));

        let error = execute_without_interaction(temp.path(), &mut state, "go Missing")
            .expect_err("missing target must fail");
        assert_eq!(error.to_string(), "target `Missing` does not exist");
        assert_eq!(state.pointer(), &Pointer::Card("0/0".to_string()));
    }

    #[test]
    fn q_exits_without_changing_the_pointer() {
        let mut state = SessionState::at("0/1");

        assert_eq!(
            execute_without_interaction(Path::new("unused"), &mut state, "q").expect("quit"),
            Execution::Exit
        );
        assert_eq!(state.pointer(), &Pointer::Card("0/1".to_string()));
    }

    #[test]
    fn unknown_commands_report_only_the_first_token() {
        let mut state = SessionState::root();

        for (line, expected) in [
            ("zt e", "unknown session command: zt"),
            ("mystery extra", "unknown session command: mystery"),
            ("root extra", "unknown session command: root"),
        ] {
            let error = execute_without_interaction(Path::new("unused"), &mut state, line)
                .expect_err("unknown command must fail");
            assert_eq!(error.to_string(), expected);
            assert_eq!(state.pointer(), &Pointer::Root);
        }
    }

    #[test]
    fn ls_returns_structured_cards_for_the_current_pointer() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             ('0/0', NULL, 1, 0, NULL, '# Topic\n\n<!-- zt:reverse-links -->\n'),\
             (NULL, 'LitA', 0, 1, '@book{LitA, title={Work}}', '# Work\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert cards");
        drop(conn);
        let mut state = SessionState::root();

        assert_eq!(
            execute_without_interaction(temp.path(), &mut state, "ls").expect("list Cards"),
            Execution::Continue(CommandResult::List(Listing {
                heading: "human workspace topics and literature: 2".to_string(),
                cards: vec![
                    CardSummary {
                        address: "0/0".to_string(),
                        title: "Topic".to_string(),
                    },
                    CardSummary {
                        address: "LitA".to_string(),
                        title: "Work".to_string(),
                    },
                ],
            }))
        );
    }

    fn insert_literature_tree(root: &Path) {
        let conn = crate::open_db(root).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             ('0/0', NULL, 1, 0, NULL, '# Topic\n\n<!-- zt:reverse-links -->\n'),\
             ('0/1', NULL, 0, 0, NULL, '# Idea\n\n<!-- zt:reverse-links -->\n'),\
             (NULL, 'LitA', 0, 1, '@book{LitA, title={Work}}', '# Work\n\n<!-- zt:reverse-links -->\n'),\
             ('LitA/1', NULL, 0, 0, NULL, '# Note\n\n<!-- zt:reverse-links -->\n'),\
             ('LitA/1|a', NULL, 0, 0, NULL, '# Aside\n\n<!-- zt:reverse-links -->\n'),\
             (NULL, 'LitB', 0, 1, '@book{LitB, title={Other}}', '# Other\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert Literature tree");
    }

    #[test]
    fn ls_on_a_literature_tree_lists_only_that_tree_root_first() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());

        for pointer in ["LitA", "LitA/1|a"] {
            let mut state = SessionState::at(pointer);
            let Execution::Continue(CommandResult::List(listing)) =
                execute_without_interaction(temp.path(), &mut state, "ls").expect("list tree")
            else {
                panic!("ls must return a Listing");
            };
            assert_eq!(listing.heading, "Literature tree LitA: 3 cards");
            assert_eq!(
                listing
                    .cards
                    .iter()
                    .map(|card| card.address.as_str())
                    .collect::<Vec<_>>(),
                ["LitA", "LitA/1", "LitA/1|a"]
            );
        }
    }

    #[test]
    fn up_walks_to_the_parent_then_the_tree_root_then_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());
        let mut state = SessionState::at("LitA/1|a");

        for expected in [
            Pointer::Card("LitA/1".to_string()),
            Pointer::Card("LitA".to_string()),
            Pointer::Root,
            Pointer::Root,
        ] {
            assert_eq!(
                execute_without_interaction(temp.path(), &mut state, "up").expect("up"),
                Execution::Continue(CommandResult::Navigated)
            );
            assert_eq!(state.pointer(), &expected);
        }
        let error = execute_without_interaction(temp.path(), &mut state, "up 2")
            .expect_err("up takes no arguments");
        assert_eq!(error.to_string(), "usage: up");
    }

    #[test]
    fn back_and_fwd_retrace_the_pointer_like_a_file_browser() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());
        let mut state = SessionState::at("LitA/1|a");
        let run = |state: &mut SessionState, line: &str| {
            execute_without_interaction(temp.path(), state, line).map(|_| state.pointer().clone())
        };
        let card = |address: &str| Pointer::Card(address.to_string());

        run(&mut state, "go LitA").expect("go");
        run(&mut state, "up").expect("up");
        assert_eq!(run(&mut state, "back").expect("back"), card("LitA"));
        assert_eq!(run(&mut state, "back").expect("back"), card("LitA/1|a"));
        let error = run(&mut state, "back").expect_err("nothing earlier");
        assert_eq!(error.to_string(), "no earlier card to go back to");
        assert_eq!(state.pointer(), &card("LitA/1|a"));
        assert_eq!(run(&mut state, "fwd").expect("fwd"), card("LitA"));
        assert_eq!(run(&mut state, "fwd").expect("fwd"), Pointer::Root);
        run(&mut state, "fwd").expect_err("nothing later");

        // A new move forgets the places back came from.
        run(&mut state, "back").expect("back");
        run(&mut state, "go LitA/1").expect("go");
        run(&mut state, "fwd").expect_err("a new move clears fwd");
        assert_eq!(run(&mut state, "back").expect("back"), card("LitA"));

        // A place whose Card is gone is skipped.
        run(&mut state, "go LitA/1|a").expect("go");
        run(&mut state, "go LitA/1").expect("go");
        crate::open_db(temp.path())
            .expect("open database")
            .execute("DELETE FROM cards WHERE location = 'LitA/1|a'", [])
            .expect("delete card");
        assert_eq!(run(&mut state, "back").expect("back"), card("LitA"));
        run(&mut state, "back 2").expect_err("back takes no arguments");
    }

    #[test]
    fn literature_card_direct_successor_starts_its_own_tree() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());
        let mut state = SessionState::at("LitB");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some(
                "# First\n\nnote\n\n<!-- zt:reverse-links -->\n".to_string(),
            )]),
            choices: VecDeque::new(),
            confirmations: VecDeque::new(),
        };

        execute(temp.path(), &mut state, "n", &mut interaction).expect("create LitB/1");
        assert_eq!(state.pointer(), &Pointer::Card("LitB/1".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        let card = crate::load_card(&conn, "LitB/1").expect("Literature tree Card");
        assert!(!card.is_lit && !card.is_topic);
        let root = crate::load_card(&conn, "LitB").expect("Literature Card");
        assert_eq!(root.bibtex.as_deref(), Some("@book{LitB, title={Other}}"));
    }

    #[test]
    fn tree_roots_refuse_side_successors_and_moves_with_consistent_wording() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());

        for (pointer, line, expected) in [
            ("0/0", "b", "side successors cannot start from a Topic Card"),
            (
                "LitA",
                "b",
                "side successors cannot start from a Literature Card",
            ),
            ("0/0", "mv 0/1|a", "Topic Cards cannot be moved"),
            (
                "LitA",
                "mv 0/1|a",
                "Literature Card cannot be moved; edit metadata to change its Citation key",
            ),
        ] {
            let mut state = SessionState::at(pointer);
            let error = execute_without_interaction(temp.path(), &mut state, line)
                .expect_err("tree roots refuse this command");
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn literature_tree_cards_never_cross_the_tree_boundary() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());

        for (pointer, line, expected) in [
            (
                "LitA/1|a",
                "mv LitB/1",
                "Literature tree Cards cannot leave Literature tree `LitA`",
            ),
            (
                "LitA/1|a",
                "mv 0/2",
                "Literature tree Cards cannot leave Literature tree `LitA`",
            ),
            (
                "0/1",
                "mv LitA/2",
                "Topic tree Cards cannot move into Literature tree `LitA`",
            ),
        ] {
            let mut state = SessionState::at(pointer);
            let error = execute_without_interaction(temp.path(), &mut state, line)
                .expect_err("cross-tree move must fail before confirmation");
            assert_eq!(error.to_string(), expected);
            assert_eq!(state.pointer(), &Pointer::Card(pointer.to_string()));
        }
        let conn = crate::open_db(temp.path()).expect("open database");
        for location in ["LitA/1|a", "0/1"] {
            crate::load_card(&conn, location).expect("refused move leaves Card in place");
        }
    }

    #[test]
    fn literature_tree_cards_move_within_their_own_tree() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());
        let mut state = SessionState::at("LitA/1|a");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::new(),
            choices: VecDeque::new(),
            confirmations: VecDeque::from(["move".to_string()]),
        };

        execute(temp.path(), &mut state, "mv LitA/2", &mut interaction).expect("move in tree");
        assert_eq!(state.pointer(), &Pointer::Card("LitA/2".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        assert!(crate::load_card(&conn, "LitA/1|a").is_err());
        crate::load_card(&conn, "LitA/2").expect("moved within LitA");
    }

    #[test]
    fn deleting_a_literature_card_deletes_its_tree_after_citation_key_confirmation() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());
        let mut state = SessionState::at("LitA");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::new(),
            choices: VecDeque::new(),
            confirmations: VecDeque::from(["delete".to_string(), "LitA".to_string()]),
        };

        let error = execute(temp.path(), &mut state, "del", &mut interaction)
            .expect_err("a tree root needs its own address as confirmation");
        assert_eq!(error.to_string(), "confirmation did not match");
        execute(temp.path(), &mut state, "del", &mut interaction).expect("delete tree");
        assert_eq!(state.pointer(), &Pointer::Root);
        let conn = crate::open_db(temp.path()).expect("open database");
        for address in ["LitA", "LitA/1", "LitA/1|a"] {
            assert!(
                crate::load_card(&conn, address).is_err(),
                "{address} remains"
            );
        }
        crate::load_card(&conn, "LitB").expect("other Literature tree survives");
        crate::load_card(&conn, "0/1").expect("Topic tree survives");
    }

    #[test]
    fn citation_key_rename_renames_the_whole_literature_tree_and_its_links() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "UPDATE cards SET text = '# Idea\n\nsee [[LitA/1|a]] and [[LitA]]\n\n<!-- zt:reverse-links -->\n'\
             WHERE location = '0/1'",
            [],
        )
        .expect("link into the Literature tree");
        drop(conn);
        let mut state = SessionState::at("LitA");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some("@book{LitC, title={Work}}".to_string())]),
            choices: VecDeque::from([crate::EditPart::Metadata]),
            confirmations: VecDeque::from(["move".to_string()]),
        };

        execute(temp.path(), &mut state, "e", &mut interaction).expect("rename tree");
        assert_eq!(state.pointer(), &Pointer::Card("LitC".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        for (old, new) in [("LitA/1", "LitC/1"), ("LitA/1|a", "LitC/1|a")] {
            assert!(crate::load_card(&conn, old).is_err(), "{old} remains");
            crate::load_card(&conn, new).expect("renamed tree Card");
        }
        assert!(
            crate::load_card(&conn, "0/1")
                .expect("link source")
                .text
                .contains("see [[LitC/1|a]] and [[LitC]]")
        );
        assert_eq!(
            crate::citation_rename_lines(&conn, "LitC", "LitD").expect("rename preview"),
            [
                "citation key rename:",
                "LitC -> LitD",
                "LitC/1 -> LitD/1",
                "LitC/1|a -> LitD/1|a",
                "renamed cards: 3",
                "link macros rewritten: 2",
            ]
        );
    }

    #[test]
    fn pointer_commands_on_root_name_the_command_and_the_fix() {
        let mut state = SessionState::root();
        for command in ["n", "b", "e", "del"] {
            let error = execute_without_interaction(Path::new("unused"), &mut state, command)
                .expect_err("ROOT has no current Card");
            assert_eq!(
                error.to_string(),
                format!("{command} needs a Card; use go <target> first")
            );
        }
        let error = execute_without_interaction(Path::new("unused"), &mut state, "ty event")
            .expect_err("ROOT has no current Card");
        assert_eq!(error.to_string(), "ty needs a Card; use go <target> first");
    }

    #[test]
    fn ty_sets_the_type_of_the_current_regular_card_only() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             ('0/0', NULL, 1, 0, NULL, '# Topic\n\n<!-- zt:reverse-links -->\n'),\
             ('0/1', NULL, 0, 0, NULL, '# Base\n\nbody\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert Cards");
        drop(conn);
        let card_type = |at: &str| {
            let conn = crate::open_db(temp.path()).expect("open database");
            crate::load_card(&conn, at).expect("card").card_type
        };
        assert_eq!(card_type("0/1"), crate::CardType::Idea);

        let mut state = SessionState::at("0/1");
        for (line, expected) in [
            ("ty event", crate::CardType::Event),
            ("ty finding", crate::CardType::Finding),
            ("ty MODEL", crate::CardType::Model),
            ("ty TERM", crate::CardType::Term),
            ("ty idea", crate::CardType::Idea),
        ] {
            assert_eq!(
                execute_without_interaction(temp.path(), &mut state, line).expect("set type"),
                Execution::Continue(CommandResult::Changed)
            );
            assert_eq!(card_type("0/1"), expected);
            assert_eq!(state.pointer(), &Pointer::Card("0/1".to_string()));
        }
        for (line, expected) in [
            (
                "ty fact",
                "unknown card type `fact`; expected idea, event, finding, model, hub, term, or cited",
            ),
            ("ty", "usage: ty <idea|event|finding|model|hub|term|cited> [<facet>]"),
            (
                "ty event model x",
                "only Model Cards can have a Model facet",
            ),
            (
                "ty event setting",
                "only Model Cards can have a Model facet",
            ),
        ] {
            let error =
                execute_without_interaction(temp.path(), &mut state, line).expect_err("invalid ty");
            assert_eq!(error.to_string(), expected);
        }

        let mut state = SessionState::at("0/0");
        let error = execute_without_interaction(temp.path(), &mut state, "ty event")
            .expect_err("tree roots stay Idea Cards");
        assert_eq!(
            error.to_string(),
            "only Regular Cards can be Event, Finding, Model, Hub, Term, or Cited Cards"
        );
        assert_eq!(card_type("0/0"), crate::CardType::Idea);
    }

    /// S04-AC03/R0163: the Session's own type setter reaches the shared gate; an
    /// enforcing archive refuses the retype and leaves the stored type alone.
    #[test]
    fn the_session_retype_reaches_the_shared_gate() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let mut conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, is_topic, is_lit, text, workspace, card_type) VALUES\
             ('0/0', 1, 0, '# Topic\n\n主题说明。\n', 'agent', 'idea'),\
             ('0/1', 0, 0, '# Base\n\n这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。\n', 'agent', 'idea')",
            [],
        )
        .expect("insert Cards");
        crate::gate::change_stage(&mut conn, "enforce").expect("enforce");
        drop(conn);
        let card_type = |at: &str| {
            let conn = crate::open_db(temp.path()).expect("open database");
            crate::load_card(&conn, at).expect("card").card_type
        };
        let mut state = SessionState::at("0/1");
        let error = execute_without_interaction(temp.path(), &mut state, "ty event")
            .expect_err("an Event must meet the Event format");
        assert!(
            error.downcast_ref::<crate::gate::Refusal>().is_some(),
            "{error:#}"
        );
        assert_eq!(card_type("0/1"), crate::CardType::Idea);
    }

    /// S11 (#134) R263: `del keep-root` empties a Tree and keeps the Pointer on its retained root.
    #[test]
    fn keep_root_empties_a_tree_and_keeps_the_pointer_on_its_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());
        let mut state = SessionState::at("LitA");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::new(),
            choices: VecDeque::new(),
            confirmations: VecDeque::from(["LitA".to_string()]),
        };

        execute(temp.path(), &mut state, "del keep-root", &mut interaction).expect("empty the tree");
        assert_eq!(state.pointer(), &Pointer::Card("LitA".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        crate::load_card(&conn, "LitA").expect("the root stays");
        for address in ["LitA/1", "LitA/1|a"] {
            assert!(crate::load_card(&conn, address).is_err(), "{address} remains");
        }
        crate::load_card(&conn, "LitB").expect("another Literature tree survives");
        crate::load_card(&conn, "0/1").expect("a Topic tree survives");
    }

    /// S11 (#134) S11-AC01: only a Tree root may be emptied, and the word `keep-root` is fixed.
    #[test]
    fn keep_root_refuses_a_regular_card_and_any_other_argument() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());
        let mut state = SessionState::at("LitA/1");

        let error = execute_without_interaction(temp.path(), &mut state, "del keep-root")
            .expect_err("a Regular Card cannot be emptied");
        assert_eq!(
            error.to_string(),
            "keep root takes a Tree root (a Topic like 3/0 or a Citation key); `LitA/1` is a Regular Card"
        );
        assert_eq!(state.pointer(), &Pointer::Card("LitA/1".to_string()));
        for line in ["del keep root", "del Keep-Root", "del --all"] {
            let error = execute_without_interaction(temp.path(), &mut state, line)
                .expect_err("another argument is not a delete scope");
            assert_eq!(error.to_string(), "usage: del [keep-root]", "{line}");
        }
        let conn = crate::open_db(temp.path()).expect("open database");
        for address in ["LitA", "LitA/1", "LitA/1|a"] {
            crate::load_card(&conn, address).expect("a refused command changes nothing");
        }
    }

    #[test]
    fn missing_pointer_recovers_to_the_nearest_existing_ancestor() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());
        let mut state = SessionState::at("LitA/1|a|3");

        assert_eq!(
            recover_missing_pointer(temp.path(), &mut state).expect("recover"),
            Some("card `LitA/1|a|3` no longer exists".to_string())
        );
        assert_eq!(state.pointer(), &Pointer::Card("LitA/1|a".to_string()));
        assert_eq!(
            recover_missing_pointer(temp.path(), &mut state).expect("existing Pointer"),
            None
        );
    }

    #[test]
    fn stats_returns_structured_card_kind_counts() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             ('0/0', NULL, 1, 0, NULL, '# Topic\n\n<!-- zt:reverse-links -->\n'),\
             ('0/1', NULL, 0, 0, NULL, '# Card\n\n<!-- zt:reverse-links -->\n'),\
             (NULL, 'LitA', 0, 1, '@book{LitA, title={Work}}', '# Work\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert cards");
        drop(conn);
        let mut state = SessionState::root();

        assert_eq!(
            execute_without_interaction(temp.path(), &mut state, "stats").expect("Card stats"),
            Execution::Continue(CommandResult::Stats(Stats {
                total: 3,
                topics: 1,
                regular: 1,
                literature: 1,
            }))
        );
    }

    #[test]
    fn status_returns_session_service_data_without_formatting_it() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        fs::write(crate::service_path(temp.path()), "123\n").expect("service pid");
        let mut state = SessionState::root();

        assert_eq!(
            execute_without_interaction(temp.path(), &mut state, "status").expect("Session status"),
            Execution::Continue(CommandResult::Status(Status {
                pid: "123".to_string(),
                archive_root: temp.path().display().to_string(),
                sqlite: crate::db_path(temp.path()).display().to_string(),
                cards: 0,
                locks: "locks: 0 trees, 0 cards, 0 pinned targets; integrity ok".to_string(),
                sessions: 0,
            }))
        );
    }

    #[test]
    fn lsbk_returns_the_link_modules_structured_broken_links() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)\
             VALUES('0/0', NULL, 1, 0, NULL, '# Topic\n\nsee [[Missing]]\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert source");
        drop(conn);
        let mut state = SessionState::root();

        assert_eq!(
            execute_without_interaction(temp.path(), &mut state, "lsbk").expect("Broken links"),
            Execution::Continue(CommandResult::BrokenLinks(vec![crate::link::BrokenLink {
                source_address: "0/0".to_string(),
                source_title: "Topic".to_string(),
                target: "Missing".to_string(),
                label: None,
                reason: crate::link::BrokenReason::Missing,
                source_line: "see [[Missing]]".to_string(),
            }]))
        );
    }

    #[test]
    fn help_shows_only_the_commands_available_at_the_pointer() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());

        for (pointer, expected) in [
            (
                SessionState::root(),
                "go [<target>] | root | ws [<workspace>] | back | fwd | ls | t <title> | l | stats | status | lsbk | help | q",
            ),
            (
                SessionState::at("0/0"),
                "go [<target>] | root | ws [<workspace>] | up | back | fwd | ls | t <title> | l | n | e | del [keep-root] | stats | status | lsbk | help | q",
            ),
            (
                SessionState::at("LitA"),
                "go [<target>] | root | ws [<workspace>] | up | back | fwd | ls | t <title> | l | n | e | del [keep-root] | stats | status | lsbk | help | q",
            ),
            (
                SessionState::at("LitA/1"),
                "go [<target>] | root | ws [<workspace>] | up | back | fwd | ls | t <title> | l | n | b | e | del [keep-root] | ins | mv [alone] <new-location> | mv [alone] before <location> | ty <idea|event|finding|model|hub|term|cited> [<facet>] | lv <level|none> | mf <form|none> | stats | status | lsbk | help | q",
            ),
        ] {
            let mut state = pointer;
            let Execution::Continue(CommandResult::Help(help)) =
                execute_without_interaction(temp.path(), &mut state, "help").expect("help")
            else {
                panic!("help must return Help");
            };
            assert_eq!(help.summary, expected);
            let lines = help.lines();
            assert_eq!(lines[0], format!("commands: {expected}"));
            assert_eq!(lines.len(), help.commands.len() + 1);
        }
    }

    #[test]
    fn topic_creation_uses_the_editor_and_moves_pointer_only_after_save() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let mut state = SessionState::root();
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some(
                "# My Topic\n\nbody\n\n<!-- zt:reverse-links -->\n".to_string(),
            )]),
            choices: VecDeque::new(),
            confirmations: VecDeque::new(),
        };

        assert_eq!(
            execute(temp.path(), &mut state, "t My Topic", &mut interaction).expect("create Topic"),
            Execution::Continue(CommandResult::Changed)
        );
        assert_eq!(state.pointer(), &Pointer::Card("0/0".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        assert_eq!(
            crate::load_card(&conn, "0/0").expect("created Topic").text,
            "# My Topic\n\nbody\n\n<!-- zt:reverse-links -->\n"
        );
    }

    #[test]
    fn direct_successor_creation_uses_current_pointer_and_moves_after_save() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)\
             VALUES('0/0', NULL, 1, 0, NULL, '# Topic\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert Topic");
        drop(conn);
        let mut state = SessionState::at("0/0");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some(
                "# Direct\n\nbody\n\n<!-- zt:reverse-links -->\n".to_string(),
            )]),
            choices: VecDeque::new(),
            confirmations: VecDeque::new(),
        };

        assert_eq!(
            execute(temp.path(), &mut state, "n", &mut interaction)
                .expect("create Direct successor"),
            Execution::Continue(CommandResult::Changed)
        );
        assert_eq!(state.pointer(), &Pointer::Card("0/1".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        assert_eq!(
            crate::load_card(&conn, "0/1")
                .expect("created Direct successor")
                .text,
            "# Direct\n\nbody\n\n<!-- zt:reverse-links -->\n"
        );
    }

    #[test]
    fn side_successor_creation_uses_current_pointer_and_moves_after_save() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             ('0/0', NULL, 1, 0, NULL, '# Topic\n\n<!-- zt:reverse-links -->\n'),\
             ('0/1', NULL, 0, 0, NULL, '# Base\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert Cards");
        drop(conn);
        let mut state = SessionState::at("0/1");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some(
                "# Side\n\nbody\n\n<!-- zt:reverse-links -->\n".to_string(),
            )]),
            choices: VecDeque::new(),
            confirmations: VecDeque::new(),
        };

        assert_eq!(
            execute(temp.path(), &mut state, "b", &mut interaction).expect("create Side successor"),
            Execution::Continue(CommandResult::Changed)
        );
        assert_eq!(state.pointer(), &Pointer::Card("0/1|a".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        assert_eq!(
            crate::load_card(&conn, "0/1|a")
                .expect("created Side successor")
                .text,
            "# Side\n\nbody\n\n<!-- zt:reverse-links -->\n"
        );
    }

    #[test]
    fn literature_creation_uses_metadata_and_card_edit_stages_atomically() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let mut state = SessionState::root();
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([
                Some("@book{LitA, title={Work}}".to_string()),
                Some("# Work\n\nbody\n\n<!-- zt:reverse-links -->\n".to_string()),
            ]),
            choices: VecDeque::new(),
            confirmations: VecDeque::new(),
        };

        assert_eq!(
            execute(temp.path(), &mut state, "l", &mut interaction)
                .expect("create Literature Card"),
            Execution::Continue(CommandResult::Changed)
        );
        assert_eq!(state.pointer(), &Pointer::Card("LitA".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        let card = crate::load_card(&conn, "LitA").expect("created Literature Card");
        assert_eq!(card.citation_key.as_deref(), Some("LitA"));
        assert_eq!(card.text, "# Work\n\nbody\n\n<!-- zt:reverse-links -->\n");
    }

    #[test]
    fn regular_card_edit_uses_current_pointer_and_preserves_it_after_save() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)\
             VALUES('0/0', NULL, 1, 0, NULL, '# Topic\n\nold\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert Topic");
        drop(conn);
        let mut state = SessionState::at("0/0");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some(
                "# Topic\n\nnew body\n\n<!-- zt:reverse-links -->\n".to_string(),
            )]),
            choices: VecDeque::new(),
            confirmations: VecDeque::new(),
        };

        assert_eq!(
            execute(temp.path(), &mut state, "e", &mut interaction).expect("edit Card"),
            Execution::Continue(CommandResult::Changed)
        );
        assert_eq!(state.pointer(), &Pointer::Card("0/0".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        assert_eq!(
            crate::load_card(&conn, "0/0").expect("edited Card").text,
            "# Topic\n\nnew body\n\n<!-- zt:reverse-links -->\n"
        );
    }

    #[test]
    fn literature_text_edit_uses_numbered_choice_and_preserves_citation_pointer() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)\
             VALUES(NULL, 'LitA', 0, 1, '@book{LitA, title={Work}}',\
                    '# Work\n\nold\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert Literature Card");
        drop(conn);
        let mut state = SessionState::at("LitA");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some(
                "# Work\n\nnew body\n\n<!-- zt:reverse-links -->\n".to_string(),
            )]),
            choices: VecDeque::from([crate::EditPart::Text]),
            confirmations: VecDeque::new(),
        };

        assert_eq!(
            execute(temp.path(), &mut state, "e", &mut interaction)
                .expect("edit Literature Card text"),
            Execution::Continue(CommandResult::Changed)
        );
        assert_eq!(state.pointer(), &Pointer::Card("LitA".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        assert_eq!(
            crate::load_card(&conn, "LitA")
                .expect("edited Literature Card")
                .text,
            "# Work\n\nnew body\n\n<!-- zt:reverse-links -->\n"
        );
    }

    #[test]
    fn literature_metadata_edit_updates_title_without_moving_the_pointer() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)\
             VALUES(NULL, 'LitA', 0, 1, '@book{LitA, title={Old Work}}',\
                    '# Old Work\n\nbody\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert Literature Card");
        drop(conn);
        let mut state = SessionState::at("LitA");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some("@book{LitA, title={New Work}}".to_string())]),
            choices: VecDeque::from([crate::EditPart::Metadata]),
            confirmations: VecDeque::new(),
        };

        assert_eq!(
            execute(temp.path(), &mut state, "e", &mut interaction)
                .expect("edit Literature metadata"),
            Execution::Continue(CommandResult::Changed)
        );
        assert_eq!(state.pointer(), &Pointer::Card("LitA".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        let card = crate::load_card(&conn, "LitA").expect("edited Literature Card");
        assert_eq!(
            card.bibtex.as_deref(),
            Some("@book{LitA, title={New Work}}")
        );
        assert!(card.text.starts_with("# New Work\n\nbody\n"));
    }

    #[test]
    fn delete_requires_confirmation_then_moves_the_pointer_to_the_parent() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             ('0/0', NULL, 1, 0, NULL, '# Topic\n\n<!-- zt:reverse-links -->\n'),\
             ('0/1', NULL, 0, 0, NULL, '# Base\n\n<!-- zt:reverse-links -->\n'),\
             ('0/1|a', NULL, 0, 0, NULL, '# Side\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert Cards");
        drop(conn);
        let mut state = SessionState::at("0/1|a");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::new(),
            choices: VecDeque::new(),
            confirmations: VecDeque::from(["cancel".to_string(), "delete".to_string()]),
        };

        let error = execute(temp.path(), &mut state, "del", &mut interaction)
            .expect_err("wrong confirmation must cancel delete");
        assert_eq!(error.to_string(), "confirmation did not match");
        assert_eq!(state.pointer(), &Pointer::Card("0/1|a".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        crate::load_card(&conn, "0/1|a").expect("canceled delete preserves Card");
        drop(conn);

        assert_eq!(
            execute(temp.path(), &mut state, "del", &mut interaction).expect("delete Card"),
            Execution::Continue(CommandResult::Changed)
        );
        assert_eq!(state.pointer(), &Pointer::Card("0/1".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        assert!(crate::load_card(&conn, "0/1|a").is_err());
    }

    #[test]
    fn move_requires_confirmation_then_moves_the_pointer_and_subtree() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             ('0/0', NULL, 1, 0, NULL, '# Topic\n\n<!-- zt:reverse-links -->\n'),\
             ('0/1', NULL, 0, 0, NULL, '# Base\n\n<!-- zt:reverse-links -->\n'),\
             ('0/1|a', NULL, 0, 0, NULL, '# Side\n\n<!-- zt:reverse-links -->\n'),\
             ('0/1|a|1', NULL, 0, 0, NULL, '# Child\n\n<!-- zt:reverse-links -->\n'),\
             ('0/2', NULL, 0, 0, NULL, '# Target\n\nsee [[0/1|a]]\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert Cards");
        drop(conn);
        let mut state = SessionState::at("0/1|a");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::new(),
            choices: VecDeque::new(),
            confirmations: VecDeque::from(["cancel".to_string(), "move".to_string()]),
        };

        let error = execute(temp.path(), &mut state, "mv 0/2|a", &mut interaction)
            .expect_err("wrong confirmation must cancel move");
        assert_eq!(error.to_string(), "confirmation did not match");
        assert_eq!(state.pointer(), &Pointer::Card("0/1|a".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        crate::load_card(&conn, "0/1|a").expect("canceled move preserves source");
        assert!(crate::load_card(&conn, "0/2|a").is_err());
        drop(conn);

        assert_eq!(
            execute(temp.path(), &mut state, "mv 0/2|a", &mut interaction).expect("move subtree"),
            Execution::Continue(CommandResult::Changed)
        );
        assert_eq!(state.pointer(), &Pointer::Card("0/2|a".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        assert!(crate::load_card(&conn, "0/1|a").is_err());
        crate::load_card(&conn, "0/2|a").expect("moved root");
        crate::load_card(&conn, "0/2|a|1").expect("moved successor");
        assert!(
            crate::load_card(&conn, "0/2")
                .expect("link source")
                .text
                .contains("[[0/2|a]]")
        );
    }

    #[test]
    fn ins_and_mv_before_put_a_card_between_two_cards() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             ('0/0', NULL, 1, 0, NULL, '# Topic\n\n<!-- zt:reverse-links -->\n'),\
             ('0/1', NULL, 0, 0, NULL, '# One\n\n<!-- zt:reverse-links -->\n'),\
             ('0/2', NULL, 0, 0, NULL, '# Two\n\nafter [[0/1]]\n\n<!-- zt:reverse-links -->\n'),\
             ('0/3', NULL, 0, 0, NULL, '# Three\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert Cards");
        drop(conn);
        let title = |at: &str| {
            let conn = crate::open_db(temp.path()).expect("open database");
            crate::parse_card_text(&crate::load_card(&conn, at).expect("card").text)
                .expect("parse")
                .title
        };
        let mut state = SessionState::at("0/2");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some(
                "# Step\n\nfrom [[0/2]]\n\n<!-- zt:reverse-links -->\n".to_string(),
            )]),
            choices: VecDeque::new(),
            confirmations: VecDeque::from(["insert".to_string(), "move".to_string()]),
        };
        assert_eq!(
            execute(temp.path(), &mut state, "ins", &mut interaction).expect("insert"),
            Execution::Continue(CommandResult::Changed)
        );
        assert_eq!(state.pointer(), &Pointer::Card("0/2".to_string()));
        assert_eq!(
            [title("0/1"), title("0/2"), title("0/3"), title("0/4")],
            ["One", "Step", "Two", "Three"]
        );
        let conn = crate::open_db(temp.path()).expect("open database");
        assert!(
            crate::load_card(&conn, "0/2")
                .unwrap()
                .text
                .contains("from [[0/3]]")
        );
        drop(conn);

        // Move the last Card alone before the first: the others move down.
        let mut state = SessionState::at("0/4");
        assert_eq!(
            execute(
                temp.path(),
                &mut state,
                "mv alone before 0/1",
                &mut interaction
            )
            .expect("move before"),
            Execution::Continue(CommandResult::Changed)
        );
        assert_eq!(state.pointer(), &Pointer::Card("0/1".to_string()));
        assert_eq!(
            [title("0/1"), title("0/2"), title("0/3"), title("0/4")],
            ["Three", "One", "Step", "Two"]
        );
        assert_eq!(
            execute(temp.path(), &mut state, "mv before", &mut interaction)
                .expect_err("usage")
                .to_string(),
            "usage: mv [alone] <new-location> | mv [alone] before <location>"
        );
    }

    #[test]
    fn literature_citation_rename_uses_the_same_confirmation_boundary() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             (NULL, 'LitA', 0, 1, '@book{LitA, title={Old Work}}',\
                    '# Old Work\n\nbody\n\n<!-- zt:reverse-links -->\n'),\
             ('0/0', NULL, 1, 0, NULL, '# Topic\n\nsee [[LitA]]\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert Cards");
        drop(conn);
        let mut state = SessionState::at("LitA");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some("@book{LitB, title={New Work}}".to_string())]),
            choices: VecDeque::from([crate::EditPart::Metadata]),
            confirmations: VecDeque::from(["move".to_string()]),
        };

        assert_eq!(
            execute(temp.path(), &mut state, "e", &mut interaction).expect("rename Citation key"),
            Execution::Continue(CommandResult::Changed)
        );
        assert_eq!(state.pointer(), &Pointer::Card("LitB".to_string()));
        let conn = crate::open_db(temp.path()).expect("open database");
        assert!(crate::load_card(&conn, "LitA").is_err());
        crate::load_card(&conn, "LitB").expect("renamed Literature Card");
        assert!(
            crate::load_card(&conn, "0/0")
                .expect("link source")
                .text
                .contains("[[LitB]]")
        );
    }
}
