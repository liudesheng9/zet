use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(super) const HELP: &str = "go [<target>] | root | ls | t <title> | l | n | b | e | del | mv <new-location> | stats | status | lsbk | q";

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

    fn current_card(&self) -> Result<String> {
        match &self.pointer {
            Pointer::Root => bail!("pointer is on ROOT"),
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
    List(Vec<CardSummary>),
    Stats(Stats),
    Status(Status),
    BrokenLinks(Vec<crate::link::BrokenLink>),
    Help(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Execution {
    Continue(CommandResult),
    Exit,
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
        ["ls"] => {
            let conn = crate::open_db(root)?;
            let cards = super::cards_for_list(&conn, state.pointer())?
                .into_iter()
                .map(|card| {
                    let parsed = crate::parse_card_text(&card.text)?;
                    Ok(CardSummary {
                        address: card.address,
                        title: parsed.title,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Execution::Continue(CommandResult::List(cards)))
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
        ["help"] => Ok(Execution::Continue(CommandResult::Help(HELP))),
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
            let at = state.current_card()?;
            let _lock = crate::acquire_edit_lock(root)?;
            let mut conn = crate::open_db(root)?;
            let parent = crate::load_card(&conn, &at)?;
            if parent.is_lit {
                bail!("zt n is not valid on a Literature Card");
            }
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
            let at = state.current_card()?;
            let _lock = crate::acquire_edit_lock(root)?;
            let mut conn = crate::open_db(root)?;
            let parent = crate::load_card(&conn, &at)?;
            if parent.is_lit {
                bail!("zt b is not valid on a Literature Card");
            }
            if parent.is_topic {
                bail!("zt b is not valid on a topic card");
            }
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
            let at = state.current_card()?;
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
                            let rewritten_links = crate::count_target_links(&conn, old_key)?;
                            interaction.confirm(
                                "move",
                                &[
                                    "citation key rename:".to_string(),
                                    format!("{old_key} -> {}", metadata.citation_key),
                                    format!("link macros rewritten: {rewritten_links}"),
                                ],
                            )?;
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
            let at = state.current_card()?;
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
            let at = state.current_card()?;
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
            Execution::Continue(CommandResult::List(vec![
                CardSummary {
                    address: "0/0".to_string(),
                    title: "Topic".to_string(),
                },
                CardSummary {
                    address: "LitA".to_string(),
                    title: "Work".to_string(),
                },
            ]))
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
    fn help_returns_the_exact_session_only_command_text() {
        let mut state = SessionState::root();

        assert_eq!(
            execute_without_interaction(Path::new("unused"), &mut state, "help")
                .expect("Session help"),
            Execution::Continue(CommandResult::Help(
                "go [<target>] | root | ls | t <title> | l | n | b | e | del | mv <new-location> | stats | status | lsbk | q"
            ))
        );
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
