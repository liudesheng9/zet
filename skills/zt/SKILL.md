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
- Card types: a Regular Card is an Idea Card by default; pass `type` `event` (one real-world event: what, when, where, who, source) or `model` (one whole model) to `create_successor` or `edit_card` (`type` may be given alone). Topic and Literature Cards are always `idea`. `get_card`, `get_tree`, and `search_cards` report `type`; `search_cards` with `type` and an empty `query` lists every Card of that type.
- Model structure: a model is its Model Card plus **Model parts**, Model Cards with a `facet` filed under the Model Card (as its successor or a successor of another part). The facet is a short label you choose (at most 32 characters, e.g. `设定`, `均衡`, `数值例子`; the common ones are `setting`, `assumptions`, `timing`, `dynamics`, `solution`, `results`, `intuition`, `limits`), and a part may carry an `emphasis`: `core` (what a reader must see) or `aside` (skippable detail). Pass `facet` and `emphasis` to `create_successor` (a facet implies `type: model`) or to `edit_card` (alone; `""` clears either). Parts relate by Links (a part links what it rests on, with the reason), not by order; a part with no such Link is loose. `get_model` with any Card of the model returns the Model Card, its parts in filing order with their bodies, its `relations` (each with the sentence that makes it), its `loose` parts, and `issues` (Links missing between the Model Card and its parts): resolve every issue. ZT refuses a facet on a Card not filed under a Model Card, and refuses to retype a Model Card that still has parts.
- Bodies may show a paper's figure with `![caption](artifact:<sha256>)`, taken with `scholar_archive_artifact` (see below); the GUI draws it as a figure with its caption.
- Bodies may hold TeX (`$...$`, `$$...$$`) and HTML charts in ```` ```chart ```` fences; the GUI typesets the math and runs each chart in a sandbox with the card's palette (`--zt-*` CSS variables). A click on an element with `data-zt="<address>"` opens that Card. See the `zettelkasten` skill for how to structure a model and draw its charts.
- `delete_card`, `move_card`, and Citation key renames return a plan first and change nothing. Show the plan to the user when the change is significant, then call again with the `confirm` text the plan names (`delete`, the tree root's address, or `move`).

Card bodies are Markdown. Compose, place, and link every Card by the `zettelkasten` skill: one idea in your own words under a claim title, placed beside the Card it continues, with Links that state their reason. Never write a Reverse links section: ZT generates it below the `<!-- zt:reverse-links -->` marker.

## Read the Scholar Archive

The `scholar_archive_*` tools read the user's local Scholar Archive (scholars, Works, abstracts, DOIs, journals, workspaces, and the PDF/Markdown documents linked to Works) through its read-only bridge. They pair and start the archive by themselves and do not need `zt up`.

- `scholar_archive_status`: the archive's identity, counts, and its read tools with their schemas.
- `scholar_archive_read` with `tool` and `arguments`: `list_scholars`, `list_works` (`query`, `scholar_id`, `journal_key`, `workspace_id`), `get_work`, `list_work_files`, `semantic_search` (`query`, `scope`), and the other read tools.
- `scholar_archive_sql`: one SELECT or schema pragma, for counts and joins.
- `scholar_archive_file` with `file_id`: Markdown comes back as `text`; for a PDF, read the returned `path`.
- `scholar_archive_export`: JSON, CSV, or BibTeX of the archive or one scholar.
- Figures: `scholar_archive_read` with `list_work_artifacts` (`work_id`, or one Markdown's `file_id`) lists a Work's artifacts, the figure and table images its Markdown shows, in reading order, each with `number`, `caption`, `file_id`, `path`, and `shown` (used by the Markdown the Work shows first; prefer those). `scholar_archive_artifact` with that `file_id` and `path` (and optionally your own `caption`) keeps the image in ZET's archive and returns the `markdown` to paste into a Card, `![caption](artifact:<sha256>)`, and a local `path` you can open to look at the figure first. ZET refuses a Card that shows an `artifact:` it does not hold, so always take the figure before writing it.

Nothing here changes the archive. Copy titles, authors, and abstracts verbatim; when you file a paper in ZET, make a Literature Card from its actual metadata (DOI included) in the agent Workspace.

## Rules

- Never change the human Workspace by any route. If the server refuses a change (for example ``the human workspace is read-only for AI agents``, or a change that would rewrite Links inside a human Card), that is the user's policy, not a bug: do not work around it with the `zt` shell commands, the GUI API, or the SQLite database. Tell the user what you wanted to change and let them do it.
- Write only through the MCP server. Do not edit the archive's `zt.sqlite3` directly.
- Cite Cards by address (`3/1|a`, `Smith2024`) when you report back.

## When the server is unavailable

- A tool error saying the service is not available: run `zt status`; if it is down, run `zt up`, then retry.
- No `zt` tools at all (DeepSeek Harness has none; elsewhere the server may not be loaded): call the same tools, with the same agent Access, through this skill's `scripts/zt-mcp.ps1`. Write the calls to a UTF-8 JSON file, `[{"tool": "create_successor", "arguments": {...}}, ...]`, and run `pwsh -NoProfile -File <skill dir>/scripts/zt-mcp.ps1 calls.json`; a single call can also go inline as `-Tool get_card -Json '{"at":"3/1"}'`. In any string argument, `@@N` becomes the address returned by call N, so one file can build a chain of Cards and link them. The output is one line per call (`N ok <address>`, `N <json>`, or `N error <reason>`), and the run stops at the first error, so re-run only from the failed call. Batch reads in one file and writes in another to save turns. The `zt` shell commands act with human Access, so never write with them.

## Developing zt itself

The source is the `zet` repository (github.com/liudesheng9/zet; a Rust crate that builds the `zt` binary). Read its `CONTEXT.md` for the domain language and `AGENTS.md` for repository workflow before changing it.

- Test with `cargo test` from PowerShell. Under Git Bash the dump tests fail because of PATH, not the code.
- Deploy a change: `zt down`, back up `<archive_root>\zt.sqlite3` when the schema changes (`zt status` prints the archive root), `cargo install --path . --locked`, `zt up`, then confirm with `zt version` and `zt status`. `zt down` refuses while a Session or the GUI is open. `cargo install` also installs `zt-boot`, the login command `zt autostart on` registers; `zt autostart` shows whether it is on and how the last login start went.
- After changing a skill (`skills/zt/`, `skills/zettelkasten/`) or the server registration, re-run `scripts/install-agent-integrations.ps1` so Claude Code, Codex, and DeepSeek Harness get the update. New conversations pick up MCP changes; open ones keep the server they started with.
