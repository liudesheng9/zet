//! Workspaces: the human's and the AI agent's halves of one archive.
//!
//! Every tree belongs to exactly one Workspace, and every Card stores its
//! tree's Workspace. Addresses stay unique across the whole archive, so Links
//! cross Workspaces freely, but Cards and trees never move between them. An
//! [`Access`] says which Workspaces a client may change: the GUI, Session, and
//! shell may change both, while the MCP server may change only the agent's.

use anyhow::{Result, bail};
use rusqlite::Connection;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum Workspace {
    Human,
    Agent,
}

impl Workspace {
    pub(crate) const ALL: [Workspace; 2] = [Workspace::Human, Workspace::Agent];

    /// The stored and typed name.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Workspace::Human => "human",
            Workspace::Agent => "agent",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Workspace::Human => "Human",
            Workspace::Agent => "AI agent",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "human" => Ok(Workspace::Human),
            "agent" | "ai" => Ok(Workspace::Agent),
            other => bail!("unknown workspace `{other}`; expected human or agent"),
        }
    }

    /// Read a stored value; anything unexpected is treated as the human's.
    pub(crate) fn from_stored(value: &str) -> Self {
        Workspace::parse(value).unwrap_or(Workspace::Human)
    }
}

impl std::fmt::Display for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// Which Workspaces a client may change. Every client may read both.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Access {
    /// The human's own tools: the Session, the GUI, and shell commands.
    Owner,
    /// The MCP server used by AI agents.
    Agent,
}

impl Access {
    pub(crate) fn can_write(self, workspace: Workspace) -> bool {
        match self {
            Access::Owner => true,
            Access::Agent => workspace == Workspace::Agent,
        }
    }

    pub(crate) fn ensure_writable(self, workspace: Workspace) -> Result<()> {
        if !self.can_write(workspace) {
            bail!(
                "the {} workspace is read-only for AI agents; only the agent workspace can be changed",
                workspace.id()
            );
        }
        Ok(())
    }

    /// Renames rewrite `[[target]]` macros in every Card that links to a
    /// renamed address. Refuse the change if that would edit the body of a
    /// Card this client may not write. Reverse link sections are generated
    /// text and are not counted as edits.
    pub(crate) fn ensure_link_rewrites_allowed(
        self,
        conn: &Connection,
        mapping: &BTreeMap<String, String>,
    ) -> Result<()> {
        if mapping.is_empty() || self == Access::Owner {
            return Ok(());
        }
        for card in crate::load_cards(conn)? {
            if self.can_write(card.workspace) || mapping.contains_key(&card.address) {
                continue;
            }
            let parsed = crate::parse_card_text(&card.text)?;
            if crate::link::LinkBody::parse(&parsed.body)?
                .rewrite_targets(mapping)
                .count
                > 0
            {
                bail!(
                    "this change would rewrite Links in {} workspace Card `{}`, which is read-only for AI agents",
                    card.workspace.id(),
                    card.address
                );
            }
        }
        Ok(())
    }
}

/// How many trees each Zone of a Workspace holds, and how many Cards in all.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WorkspaceCounts {
    pub(crate) topic_trees: usize,
    pub(crate) literature_trees: usize,
    pub(crate) cards: usize,
}

pub(crate) fn counts(cards: &[crate::Card], workspace: Workspace) -> WorkspaceCounts {
    let mut counts = WorkspaceCounts::default();
    for card in cards.iter().filter(|card| card.workspace == workspace) {
        counts.cards += 1;
        if card.is_topic {
            counts.topic_trees += 1;
        } else if card.is_lit {
            counts.literature_trees += 1;
        }
    }
    counts
}

/// The Workspace of the tree that holds `address`, read from its Tree root.
pub(crate) fn tree_workspace(conn: &Connection, address: &str) -> Result<Workspace> {
    let root = crate::tree_root_address(crate::tree_id(address));
    match crate::load_card(conn, &root) {
        Ok(card) => Ok(card.workspace),
        Err(_) => bail!("tree root `{root}` does not exist"),
    }
}

/// Cards and trees stay in the Workspace they were created in.
pub(crate) fn ensure_same_workspace(conn: &Connection, from: &str, to: &str) -> Result<()> {
    let source = tree_workspace(conn, from)?;
    let destination = tree_workspace(conn, to)?;
    if source != destination {
        bail!(
            "Cards cannot move between workspaces: `{from}` is in the {source} workspace and `{to}` is in the {destination} workspace"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_names_parse_and_round_trip() {
        for workspace in Workspace::ALL {
            assert_eq!(Workspace::parse(workspace.id()).unwrap(), workspace);
        }
        assert_eq!(Workspace::parse(" AI ").unwrap(), Workspace::Agent);
        assert!(Workspace::parse("shared").is_err());
        assert_eq!(Workspace::from_stored("bogus"), Workspace::Human);
    }

    #[test]
    fn agent_access_writes_only_the_agent_workspace() {
        assert!(Access::Owner.can_write(Workspace::Human));
        assert!(Access::Owner.can_write(Workspace::Agent));
        assert!(Access::Agent.can_write(Workspace::Agent));
        assert!(!Access::Agent.can_write(Workspace::Human));
        assert_eq!(
            Access::Agent
                .ensure_writable(Workspace::Human)
                .unwrap_err()
                .to_string(),
            "the human workspace is read-only for AI agents; only the agent workspace can be changed"
        );
    }
}
