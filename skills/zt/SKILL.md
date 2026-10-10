---
name: zt
description: "Read and write the user's ZT (zet) slip-box of Markdown Cards through the `zt` MCP server: browse both Workspaces, search, follow Links, and create, edit, move, or delete Cards in the agent Workspace (the human Workspace is read-only). Use when the user mentions zt, ZET, the slip-box, zettelkasten, Cards, Topic or Literature trees, or asks you to record, file, look up, or connect notes, ideas, or papers; also use when developing the zt project itself."
---

# ZT slip-box

ZT stores addressable Markdown **Cards** in one local archive split into two **Workspaces**:

- `human`: the user's own Cards. **Read-only for you.**
- `agent`: your Cards. You may read and change them.

Each Workspace has two **Zones**: Topic trees (rooted at a Topic such as `3/0`) and Literature trees (rooted at a Literature Card named by its BibTeX Citation key, such as `Smith2024`). Addresses are unique across both Workspaces: `3/1` is the direct successor of Topic `3/0`, `3/2` follows `3/1`, `3/1|a` branches sideways from `3/1`, and `Smith2024/1` is a note in a Literature tree. Links are written `[[address]]` in a Card body and may cross Workspaces; Cards never move between Workspaces.

## Use the MCP server

The server is registered globally as `zt` (Claude Code tools `mcp__zt__*`, Codex tools on server `zt`). Start every task by reading before writing:

1. `list_workspaces`: what exists, and your access.
2. `list_trees` (filter by `workspace`, `zone`), then `get_tree` or `get_card`.
3. `search_cards` before creating anything, so you extend or link existing Cards instead of duplicating them. `list_links` with `cross_workspace_only` shows how your Cards connect to the user's.

Writing (agent Workspace only):

- New idea area: `create_topic` with a one-line `title` and a short `body`.
- New paper or book: `create_literature` with exactly one BibTeX entry that has a `title`; its entry key becomes the Citation key.
- Grow a tree: `create_successor` with `at`, `title`, `body`, and `kind` `direct` (continue the line) or `side` (a tangent; not from a tree root).
- Change a Card: `edit_card` with `title` and/or `body` (or the whole `text`). A Literature Card's title comes from its BibTeX; change that with `edit_literature_metadata`.
- `delete_card`, `move_card`, and Citation key renames return a plan first and change nothing. Show the plan to the user when the change is significant, then call again with the `confirm` text the plan names (`delete`, the tree root's address, or `move`).

Card bodies are Markdown. One idea per Card; link generously with `[[address]]` to related Cards in either Workspace, including the user's. Never write a Reverse links section: ZT generates it below the `<!-- zt:reverse-links -->` marker.

## Rules

- Never change the human Workspace by any route. If the server refuses a change (for example ``the human workspace is read-only for AI agents``, or a change that would rewrite Links inside a human Card), that is the user's policy, not a bug: do not work around it with the `zt` shell commands, the GUI API, or the SQLite database. Tell the user what you wanted to change and let them do it.
- Write only through the MCP server. Do not edit the archive's `zt.sqlite3` directly.
- Cite Cards by address (`3/1|a`, `Smith2024`) when you report back.

## When the server is unavailable

- A tool error saying the service is not available: run `zt status`; if it is down, run `zt up`, then retry.
- No `zt` tools at all: the server is not loaded in this conversation. Check that `zt mcp` runs (`zt version`), re-run the installer from the zt repository (`scripts/install-agent-integrations.ps1`), and ask the user to start a new conversation. Meanwhile you may read with `zt ws`, `zt stats`, `zt lsbk`, or a piped line Session such as `printf 'go 3/1\nq\n' | zt`; do not write.

## Developing zt itself

The source is the `zet` repository (github.com/liudesheng9/zet; a Rust crate that builds the `zt` binary). Read its `CONTEXT.md` for the domain language and `AGENTS.md` for repository workflow before changing it.

- Test with `cargo test` from PowerShell. Under Git Bash the dump tests fail because of PATH, not the code.
- Deploy a change: `zt down`, back up `<archive_root>\zt.sqlite3` when the schema changes (`zt status` prints the archive root), `cargo install --path . --locked`, `zt up`, then confirm with `zt version` and `zt status`. `zt down` refuses while a Session or the GUI is open.
- After changing this skill (`skills/zt/`) or the server registration, re-run `scripts/install-agent-integrations.ps1` so Claude Code and Codex get the update. New conversations pick up MCP changes; open ones keep the server they started with.
