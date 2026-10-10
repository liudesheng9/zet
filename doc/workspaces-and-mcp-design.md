# Workspaces and the MCP Server Design

Status: implemented in zt 0.3.0.

## Source Authority

- `CONTEXT.md` defines the project language, including **Workspace**, **Zone**, **Access**, and **MCP server**.
- Earlier design documents remain authoritative; this document adds Workspaces to every tree and adds `zt mcp`.

## Goals

1. One archive holds two Workspaces: the human Workspace and the agent (AI agent) Workspace.
2. Each Workspace has a Topic zone and a Literature zone, and each Zone holds any number of trees.
3. Links cross Workspaces; Cards and trees never move between them.
4. An MCP server offers every Card operation to AI agents, may read both Workspaces, and may change only the agent Workspace.

## Locked Model

- Every Card row stores `workspace TEXT NOT NULL DEFAULT 'human' CHECK(workspace IN ('human', 'agent'))`. A tree's Cards all share the Workspace of their Tree root.
- Addresses stay unique across the whole archive. Topic numbers are allocated from one sequence and Citation keys stay unique (case-insensitively) across both Workspaces, so `[[target]]` keeps one meaning everywhere and the Link lifecycle, Reverse links, and Broken links are unchanged.
- A Zone is the tree kind of a Workspace's Tree roots: Topics open Topic trees and Literature Cards open Literature trees.
- `insert_card` requires a Regular Card to name its tree's Workspace; a Topic or Literature Card names the Workspace of the new tree.
- `move_plan` refuses a destination whose tree is in another Workspace, for every client: ``Cards cannot move between workspaces: `<from>` is in the <a> workspace and `<to>` is in the <b> workspace``.
- Migration: `zt up` (and any command that opens the archive) adds the column to a pre-Workspace archive with `ALTER TABLE`; every existing Card lands in the human Workspace.

## Access

| Client | Read | Change |
| --- | --- | --- |
| Session, GUI, shell commands | both Workspaces | both Workspaces |
| `zt mcp` | both Workspaces | agent Workspace only |

Agent Access refuses:

- creating a tree in, or a successor under, a human Card; editing, deleting, or moving a human Card;
- any change whose Location or Citation key mapping would rewrite a `[[target]]` macro in the body of a human Card: moving an agent Card that a human Card links to, renaming such an agent Citation key, or deleting an agent Side successor whose compaction would renumber a Card a human Card links to.

Regenerating a human Card's Reverse link section is not a change to it: the section is generated text. Deleting an agent Card that a human Card links to is allowed and leaves a Broken link in the human Card, exactly as any deletion does.

## Shared Operations

`src/ops.rs` holds the Card operations used by both the GUI and the MCP server. Each takes an `Access`, and destructive operations are two-step: without the expected confirmation they return the plan lines and the confirmation text (`delete`, the Tree root address, or `move`) and change nothing.

## MCP Server

- Transport: JSON-RPC 2.0 over stdio, one message per line; batches are accepted. `initialize` negotiates `2025-11-25`, `2025-06-18`, `2025-03-26`, or `2024-11-05`.
- Tools: `list_workspaces`, `list_trees`, `get_tree`, `get_card`, `search_cards`, `list_links`, `list_broken_links`, `create_topic`, `create_literature`, `create_successor`, `edit_card`, `edit_literature_metadata`, `delete_card`, `move_card`. Read tools carry `readOnlyHint`; delete, move, and metadata edits carry `destructiveHint`.
- Tool failures are results with `isError: true` and an `error` message; protocol errors (unknown method or tool) are JSON-RPC errors.
- Each call checks that the service is up, takes the edit lock for changes, and does not count as a Session, so `zt down` is never blocked by an idle agent.

## Session and GUI

- The Session keeps a current Workspace. ROOT shows its two Zones; `ws` lists Workspaces and `ws <workspace>` switches to its ROOT; `t` and `l` create in it; following the Pointer into a Card adopts that Card's Workspace.
- The GUI cabinet has one tab per Workspace and shows the open Workspace's Zones; the desk groups stacks by Zone; new trees go to the open Workspace; agent Cards carry an AI AGENT tag; Links into the other Workspace are inked in its color. `w` and `ws` switch Workspaces.
