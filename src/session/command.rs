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
        "up",
        "go to the parent card; a tree root goes to ROOT",
        Availability::OnCard,
    ),
    (
        "ls",
        "list Topics and Literature at ROOT, or the current tree",
        Availability::Always,
    ),
    ("t <title>", "create a Topic", Availability::Always),
    (
        "l",
        "create a Literature Card from one BibTeX entry",
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
        "del",
        "delete the current card and its successors",
        Availability::OnCard,
    ),
    (
        "mv <new-location>",
        "move the current card and its successors",
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

pub(super) fn pointer_context(root: &Path, pointer: &Pointer) -> Result<PointerContext> {
    match pointer {
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Pointer {
    Root,
    Card(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SessionState {
    pointer: Pointer,
}

impl SessionState {
    pub(super) fn root() -> Self {
        Self {
            pointer: Pointer::Root,
        }
    }

    pub(super) fn pointer(&self) -> &Pointer {
        &self.pointer
    }

    pub(super) fn follow_link(&mut self, target: String) {
        self.pointer = Pointer::Card(target);
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
        ["up"] => {
            if let Pointer::Card(target) = &state.pointer {
                state.pointer = crate::parent_location(target)
                    .map(Pointer::Card)
                    .unwrap_or(Pointer::Root);
            }
            Ok(Execution::Continue(CommandResult::Navigated))
        }
        ["up", ..] => bail!("usage: up"),
        ["ls"] => {
            let conn = crate::open_db(root)?;
            let (heading, cards) = super::cards_for_list(&conn, state.pointer())?;
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
            let _lock = crate::acquire_edit_lock(root)?;
            let mut conn = crate::open_db(root)?;
            let location = crate::next_topic_location(&conn)?;
            let mut save = |text: &str| {
                crate::insert_card(&mut conn, &location, true, text)?;
                crate::bump_next_topic_id(&conn, &location)
            };
            if interaction.edit(&crate::topic_template(&title), "edit mode", &mut save)? {
                state.pointer = Pointer::Card(location);
            }
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["n"] => {
            let at = state.current_card("n")?;
            let _lock = crate::acquire_edit_lock(root)?;
            let mut conn = crate::open_db(root)?;
            let parent = crate::load_card(&conn, &at)?;
            let location = crate::direct_successor(&parent.address)?;
            if crate::location_exists(&conn, &location)? {
                bail!("direct successor already exists: {location}");
            }
            let mut save = |text: &str| crate::insert_card(&mut conn, &location, false, text);
            if interaction.edit(&crate::regular_template(), "edit mode", &mut save)? {
                state.pointer = Pointer::Card(location);
            }
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["b"] => {
            let at = state.current_card("b")?;
            let _lock = crate::acquire_edit_lock(root)?;
            let mut conn = crate::open_db(root)?;
            let parent = crate::load_card(&conn, &at)?;
            crate::ensure_side_parent(&parent)?;
            let location = crate::next_side_successor(&conn, &parent.address, &BTreeSet::new())?;
            let mut save = |text: &str| crate::insert_card(&mut conn, &location, false, text);
            if interaction.edit(&crate::regular_template(), "edit mode", &mut save)? {
                state.pointer = Pointer::Card(location);
            }
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["l"] => {
            let _lock = crate::acquire_edit_lock(root)?;
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
            let initial_text = crate::compose_card_text(&metadata.title, "", "");
            let citation_key = metadata.citation_key.clone();
            let mut save_text = |edited: &str| {
                let parsed = crate::parse_literature_edit_text(edited)?;
                let text = crate::compose_card_text(&metadata.title, &parsed.body, "");
                crate::insert_literature_card(&mut conn, &citation_key, &bibtex, &text)
            };
            if interaction.edit(&initial_text, "edit mode", &mut save_text)? {
                state.pointer = Pointer::Card(citation_key);
            }
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["e"] => {
            let at = state.current_card("e")?;
            let _lock = crate::acquire_edit_lock(root)?;
            let mut conn = crate::open_db(root)?;
            let card = crate::load_card(&conn, &at)?;
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
                        crate::update_literature_metadata(
                            &mut conn,
                            &card,
                            &metadata.citation_key,
                            &bibtex,
                            &metadata.title,
                        )?;
                        state.pointer = Pointer::Card(metadata.citation_key);
                        Ok(Execution::Continue(CommandResult::Changed))
                    }
                    crate::EditPart::Text => {
                        let mut save =
                            |text: &str| crate::update_literature_text(&mut conn, &card, text);
                        interaction.edit(&card.text, "edit mode", &mut save)?;
                        Ok(Execution::Continue(CommandResult::Changed))
                    }
                };
            }
            let mut save = |text: &str| crate::update_card_text(&mut conn, &card, text);
            interaction.edit(&card.text, "edit mode", &mut save)?;
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["del"] => {
            let at = state.current_card("del")?;
            let _lock = crate::acquire_edit_lock(root)?;
            let mut conn = crate::open_db(root)?;
            let plan = crate::delete_plan(&conn, &at)?;
            interaction.confirm(&plan.confirmation, &crate::delete_verification_lines(&plan))?;
            crate::apply_delete_plan(&mut conn, &at, plan)?;
            state.pointer = crate::parent_location(&at)
                .map(Pointer::Card)
                .unwrap_or(Pointer::Root);
            Ok(Execution::Continue(CommandResult::Changed))
        }
        ["mv", new_location] => {
            let at = state.current_card("mv")?;
            let _lock = crate::acquire_edit_lock(root)?;
            let mut conn = crate::open_db(root)?;
            let plan = crate::move_plan(&conn, &at, new_location)?;
            interaction.confirm("move", &crate::move_verification_lines(&plan))?;
            crate::apply_move_plan(&mut conn, &plan)?;
            state.pointer = Pointer::Card((*new_location).to_string());
            Ok(Execution::Continue(CommandResult::Changed))
        }
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
             VALUES('0/0', NULL, 1, 0, NULL, 'Topic\n<--->\n\n<--->\n')",
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
             ('0/0', NULL, 1, 0, NULL, 'Topic\n<--->\n\n<--->\n'),\
             (NULL, 'LitA', 0, 1, '@book{LitA, title={Work}}', 'Work\n<--->\n\n<--->\n')",
            [],
        )
        .expect("insert cards");
        drop(conn);
        let mut state = SessionState::root();

        assert_eq!(
            execute_without_interaction(temp.path(), &mut state, "ls").expect("list Cards"),
            Execution::Continue(CommandResult::List(Listing {
                heading: "topics and literature: 2".to_string(),
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
             ('0/0', NULL, 1, 0, NULL, 'Topic\n<--->\n\n<--->\n'),\
             ('0/1', NULL, 0, 0, NULL, 'Idea\n<--->\n\n<--->\n'),\
             (NULL, 'LitA', 0, 1, '@book{LitA, title={Work}}', 'Work\n<--->\n\n<--->\n'),\
             ('LitA/1', NULL, 0, 0, NULL, 'Note\n<--->\n\n<--->\n'),\
             ('LitA/1|a', NULL, 0, 0, NULL, 'Aside\n<--->\n\n<--->\n'),\
             (NULL, 'LitB', 0, 1, '@book{LitB, title={Other}}', 'Other\n<--->\n\n<--->\n')",
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
    fn literature_card_direct_successor_starts_its_own_tree() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        insert_literature_tree(temp.path());
        let mut state = SessionState::at("LitB");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some("First\n<--->\nnote\n<--->\n".to_string())]),
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
            "UPDATE cards SET text = 'Idea\n<--->\nsee [[LitA/1|a]] and [[LitA]]\n<--->\n'\
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
             ('0/0', NULL, 1, 0, NULL, 'Topic\n<--->\n\n<--->\n'),\
             ('0/1', NULL, 0, 0, NULL, 'Card\n<--->\n\n<--->\n'),\
             (NULL, 'LitA', 0, 1, '@book{LitA, title={Work}}', 'Work\n<--->\n\n<--->\n')",
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
             VALUES('0/0', NULL, 1, 0, NULL, 'Topic\n<--->\nsee [[Missing]]\n<--->\n')",
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
                "go [<target>] | root | ls | t <title> | l | stats | status | lsbk | help | q",
            ),
            (
                SessionState::at("0/0"),
                "go [<target>] | root | up | ls | t <title> | l | n | e | del | stats | status | lsbk | help | q",
            ),
            (
                SessionState::at("LitA"),
                "go [<target>] | root | up | ls | t <title> | l | n | e | del | stats | status | lsbk | help | q",
            ),
            (
                SessionState::at("LitA/1"),
                "go [<target>] | root | up | ls | t <title> | l | n | b | e | del | mv <new-location> | stats | status | lsbk | help | q",
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
            edits: VecDeque::from([Some("My Topic\n<--->\nbody\n<--->\n".to_string())]),
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
            "My Topic\n<--->\nbody\n<--->\n"
        );
    }

    #[test]
    fn direct_successor_creation_uses_current_pointer_and_moves_after_save() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)\
             VALUES('0/0', NULL, 1, 0, NULL, 'Topic\n<--->\n\n<--->\n')",
            [],
        )
        .expect("insert Topic");
        drop(conn);
        let mut state = SessionState::at("0/0");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some("Direct\n<--->\nbody\n<--->\n".to_string())]),
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
            "Direct\n<--->\nbody\n<--->\n"
        );
    }

    #[test]
    fn side_successor_creation_uses_current_pointer_and_moves_after_save() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             ('0/0', NULL, 1, 0, NULL, 'Topic\n<--->\n\n<--->\n'),\
             ('0/1', NULL, 0, 0, NULL, 'Base\n<--->\n\n<--->\n')",
            [],
        )
        .expect("insert Cards");
        drop(conn);
        let mut state = SessionState::at("0/1");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some("Side\n<--->\nbody\n<--->\n".to_string())]),
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
            "Side\n<--->\nbody\n<--->\n"
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
                Some("Work\n<--->\nbody\n<--->\n".to_string()),
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
        assert_eq!(card.text, "Work\n<--->\nbody\n<--->\n");
    }

    #[test]
    fn regular_card_edit_uses_current_pointer_and_preserves_it_after_save() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)\
             VALUES('0/0', NULL, 1, 0, NULL, 'Topic\n<--->\nold\n<--->\n')",
            [],
        )
        .expect("insert Topic");
        drop(conn);
        let mut state = SessionState::at("0/0");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some("Topic\n<--->\nnew body\n<--->\n".to_string())]),
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
            "Topic\n<--->\nnew body\n<--->\n"
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
                    'Work\n<--->\nold\n<--->\n')",
            [],
        )
        .expect("insert Literature Card");
        drop(conn);
        let mut state = SessionState::at("LitA");
        let mut interaction = ScriptedInteraction {
            edits: VecDeque::from([Some("Work\n<--->\nnew body\n<--->\n".to_string())]),
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
            "Work\n<--->\nnew body\n<--->\n"
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
                    'Old Work\n<--->\nbody\n<--->\n')",
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
        assert!(card.text.starts_with("New Work\n<--->\nbody\n"));
    }

    #[test]
    fn delete_requires_confirmation_then_moves_the_pointer_to_the_parent() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             ('0/0', NULL, 1, 0, NULL, 'Topic\n<--->\n\n<--->\n'),\
             ('0/1', NULL, 0, 0, NULL, 'Base\n<--->\n\n<--->\n'),\
             ('0/1|a', NULL, 0, 0, NULL, 'Side\n<--->\n\n<--->\n')",
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
             ('0/0', NULL, 1, 0, NULL, 'Topic\n<--->\n\n<--->\n'),\
             ('0/1', NULL, 0, 0, NULL, 'Base\n<--->\n\n<--->\n'),\
             ('0/1|a', NULL, 0, 0, NULL, 'Side\n<--->\n\n<--->\n'),\
             ('0/1|a|1', NULL, 0, 0, NULL, 'Child\n<--->\n\n<--->\n'),\
             ('0/2', NULL, 0, 0, NULL, 'Target\n<--->\nsee [[0/1|a]]\n<--->\n')",
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
    fn literature_citation_rename_uses_the_same_confirmation_boundary() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             (NULL, 'LitA', 0, 1, '@book{LitA, title={Old Work}}',\
                    'Old Work\n<--->\nbody\n<--->\n'),\
             ('0/0', NULL, 1, 0, NULL, 'Topic\n<--->\nsee [[LitA]]\n<--->\n')",
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
