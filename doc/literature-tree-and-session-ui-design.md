# Literature Tree and Consistent Session UI Design

Status: design locked for implementation.

## Source Authority

- `CONTEXT.md` defines the project language. This document adds `Topic tree`, `Literature tree`, and `Tree root`.
- `doc/zt-note-system-design.md`, `doc/literature-card-design.md`, and `doc/session-bare-subcommands-design.md` remain authoritative except where this document explicitly changes them.
- This document supersedes the Literature Card rules that a Literature Card has no successors, that `n` is refused on a Literature Card, that deleting a Literature Card requires typing `delete`, that `ls` on a Literature Card lists the whole Literature collection, and that the automatic ROOT view shows Topic Cards only.

## Goals

1. Every Literature Card can grow its own independent successor tree, with the Literature Card as the root and its bibliographic metadata unchanged.
2. Literature tree Cards can never leave their originating Literature tree, and can never be moved into a Topic tree (the idea tree).
3. Session operations behave the same way on both kinds of tree, and the terminal interface makes navigation and discovery easier.

## Locked Literature Tree Requirements

- A Literature tree is the successor tree rooted at one Literature Card.
- The Literature Card is the Tree root. It keeps its Citation key address, BibTeX metadata, generated title, body, and Reverse links exactly as before.
- Non-root Literature tree Cards are Regular Cards. They are stored exactly like Topic-tree Regular Cards (`is_lit = 0`, `is_topic = 0`, non-null `location`); no schema change is needed.
- Their Location uses the Citation key where a Topic tree uses its Topic number: `<citation-key>/<positive-number>` followed by zero or more `|<positive-number>` or `|<side-label>` segments, for example `Smith2024/1`, `Smith2024/2`, `Smith2024/1|a`, `Smith2024/1|a|1`.
- A Location's tree id is the text before `/`. A numeric tree id names a Topic tree; a Citation-key tree id names a Literature tree. The two grammars cannot collide because Citation keys start with a letter and cannot contain `/` or `|`.
- `<citation-key>/0` is not a valid Location; the Literature Card itself is addressed only by its Citation key.
- The Tree root of a Literature tree behaves like a Topic Card:
  - `n` on the Literature Card creates `<citation-key>/1`, and is refused if it already exists.
  - `b` on the Literature Card is refused, as it is on a Topic Card.
  - `mv` of the Literature Card is refused; its Citation key changes only through metadata editing.
- Direct successor, Side successor, side-label compaction, subtree move, Link rewriting, Reverse link, and Broken link rules for Literature tree Cards are exactly the Topic-tree rules.
- The parent of `<citation-key>/1` is the Literature Card `<citation-key>`.
- Links may target Literature tree Cards by Location from any Card kind, for example `[[Smith2024/1|a]]`.
- Shell `zt n --at <target>` accepts a Citation key. Shell `zt b --at <location>` and `zt mv --at <location> <new-location>` accept Literature tree Locations.

## Locked Tree Boundary

- `mv` never changes a Card's tree when either the source or destination tree is a Literature tree.
- A Literature tree Card can move only to another Location in the same Literature tree. Any other destination fails with `Literature tree Cards cannot leave Literature tree `<citation-key>``.
- A Topic tree Card cannot move into any Literature tree. It fails with `Topic tree Cards cannot move into Literature tree `<citation-key>``.
- Moves between Topic trees keep their existing behavior.
- The boundary is checked before any destination-parent or conflict check, and a refused move changes nothing.

## Locked Citation-Key Rename

- Renaming a Citation key renames the whole Literature tree: `Old -> New` and every `Old/<suffix> -> New/<suffix>`.
- Every Link to the Literature Card or to any of its tree Cards is rewritten.
- The rename verification shows every old-to-new mapping, the number of renamed Cards, and the number of rewritten Link macros, and still requires typing `move`.
- Rename, node relocation, Link rewriting, title regeneration, and Reverse link regeneration run in one SQLite transaction.
- A rename is refused if any existing Location already starts with `New/`.

## Locked Deletion

- Deleting a Tree root deletes its whole tree: a Topic Card deletes its Topic tree and a Literature Card deletes its Literature tree.
- Deleting any Tree root requires typing the root's own address: the Topic Location (for example `1/0`) or the Citation key (for example `Smith2024`). Other Cards still require `delete`.
- Deleting a Literature tree Regular Card deletes that Card and its successors, with the normal side-label compaction.
- After Session deletion, the Pointer moves to the deleted Card's parent if it still exists, otherwise to ROOT.

## Locked Session Navigation and Listing

- The automatic ROOT view shows a `topics:` section and a `literature:` section. Each entry shows its address and title, and each address is a clickable link. An empty section shows a one-line hint naming the command that creates the first entry.
- `ls` at ROOT lists Topic Cards and Literature Cards (unchanged).
- `ls` on any Card lists that Card's whole tree, root first: the Topic tree for Topic-tree Cards and the Literature tree for Literature tree Cards.
- Every Card view shows its navigation links after the Reverse links: `parent: [[<parent>]]` (omitted on Tree roots), `direct: [[<direct>]]`, and `side: [[<side>]]` lines. Literature Cards now show their `direct:` link.
- New bare Session command `up` moves the Pointer to the current Card's parent; on a Tree root it moves to ROOT; on ROOT it is a no-op. `up <extra>` fails with `usage: up`.

## Locked Command Consistency

- Session `help` shows only the commands available at the current Pointer, as one summary line followed by one line per command with a short description.
  - ROOT: `go [<target>] | root | ls | t <title> | l | stats | status | lsbk | help | q`
  - Tree root (Topic Card or Literature Card): `go [<target>] | root | up | ls | t <title> | l | n | e | del | stats | status | lsbk | help | q`
  - Regular Card: `go [<target>] | root | up | ls | t <title> | l | n | b | e | del | mv <new-location> | stats | status | lsbk | help | q`
- Error text shared by the Session and the shell never names the `zt` executable. Card-kind refusals use the same wording everywhere:
  - `side successors cannot start from a Topic Card`
  - `side successors cannot start from a Literature Card`
  - `Topic Cards cannot be moved`
  - `Literature Card cannot be moved; edit metadata to change its Citation key`
- Pointer-dependent commands on ROOT fail with `<command> needs a Card; use go <target> first`.

## Locked Terminal Layout

- The command bar is pinned to the last terminal row.
- One message row sits directly above the command bar. Errors are shown in red. When there is no message, a dim hint shows `help: commands | PgUp/PgDn: scroll | q: quit`.
- The card view or a result panel fills the rows above the message row and scrolls with PageUp/PageDown and the mouse wheel. When the content does not fit, the hint shows the visible line range.
- `ls`, `lsbk`, and `help` open a result panel in the content area instead of a single joined message line. Panel entries show one item per line, and addresses in `ls` and `lsbk` panels are clickable links.
- A panel closes on the next command, on link navigation, or on Esc with an empty command bar.
- `stats` and `status` remain one-line messages.
- The line Session prints the same view lines and the same result lines as the TUI.

## Acceptance and Verification

- Unit tests cover Literature tree Location grammar, parents, direct successors, tree ids, tree membership, the move boundary, context help, and `up`.
- CLI integration tests cover Literature tree creation from the shell and Session, listing, Links into Literature trees, compaction, the move boundary in both directions, Citation-key rename of a whole tree, whole-tree deletion with Citation-key confirmation, and dump mapping entries.
- PTY tests cover `n` on a Literature Card, clickable ROOT literature entries, `up`, the `ls` panel, context help, and the updated refusal wording.
- `cargo test` passes in full.
