# Literature-only digests

Status: in place (2026-10-22; the `-Open` approval gap closed in base 5.0.1). Owner decision: a digest builds the book's Literature tree and
nothing else. Composing ideas into Topic trees, composing Topic trees, and composing hubs and the
concept register belong to zt-idea-grow alone (docs/idea-grow-design.md).

## The rule

A digest run (zt-digest-base and every variant: zt-econ-digest, zt-math-digest) never:

- creates a Topic tree (`create_topic`) or a Card in a Topic tree;
- edits, moves, inserts or deletes a Card of a Topic tree;
- creates or edits a Hub Card (topic, line, field hub) or the concept register;
- writes a Link (`[[N/…]]`) into a Topic-tree Card from a Card it writes.

A Topic tree is a tree whose id is a Topic number (all digits). The digest's own Cards are the Cards
of the book's Literature tree. A cross-book Link from the book's Literature tree to another book's
Literature tree stays allowed (Link, Bridge).

## How it is enforced

1. **The write scope (hard guard).** `zt_check::mcp` gets a process-wide write scope. `zt-digest` sets
   `Literature` for every digest subcommand (prepare, group-start/finish, link-start/finish, card,
   model, math, event, finding, terms, skeleton, figures, bridge, finalize, the ledger finishes,
   audit, concepts) and for `zt-digest call` without `-Open` (the helper line of the digest's group,
   link and card jobs). Under it the client refuses, before the call reaches
   the archive, every write that names a Topic-tree Card, `create_topic`, a Hub type, and any
   authored text that links a Topic-tree Card. `apply_batch` items are checked one by one. The idea-grow,
   hub, register, organize, gate, join, topic-home and publish tools keep the open scope.
2. **The approvals.** The Haiku managers approve a DeepSeek job's zt helper command only without `-Open` (`ZD_NO_OPEN` in every approval rule, `zt-digest-lib` v32, base 5.0.1, econ 6.0.1, math 5.0.1): a job that added the flag would run in the open scope.
3. **The prompts and stages.** The digest prompts file every Card in the Literature tree. The
   blueprints lose the stages that only served Topics and hubs.
4. **The reads.** A digest reads Topic trees no more: the group snapshot, the earlier-Card pack,
   place-beside, the Link candidates and the Bridge candidates come from Literature trees only, and
   a Card belongs to a book only when it is in that book's Literature tree (`book_of`, no Link votes).

## Digest changes (base 5.0.0, econ 6.0.0, math 5.0.0)

- Removed stages: `concepts` (concept review of the term table against Topics), `review`
  (restatement review of new Topic Idea Cards), `hubs-first`, `hubs`. Removed mode `hubs`.
  Removed args `hubs`, `hubWork`, `hubLimits`, `hubTrees`, `hubRebuild`, `hubEffort`,
  `hubFixRounds`, `registerPlan`, and `organize: "legacy"`.
- Organize keeps only Literature work: the notes card cycle on the book's own Cards and the skeleton
  (the Literature Card index and the structure Card). No Topic merges, moves or Topic bodies.
- Finalize refreshes no hubs (the `hubs` layer is excluded at prepare). The audit stays a read-only
  report: it still counts the box's Topic trees and hubs as information, never as a digest fault.
- Bridge: sources, targets and carriers are Literature-tree Cards.
- Prompts: no `create_topic`, no Topic gate, no Topic placement order, no Topic-tree I7 fix.

## zt-idea-grow changes (3.0.0)

Idea-grow already writes the Topic trees, the Topic Cards, the links across trees, the topic, line
and field hubs and the register (2.5.1). It takes over from the digests:

- mode `hubs`: the hubs round and the register alone, over the whole agent Workspace, no new Cards
  (the digest's mode `hubs`);
- the hub options: `hubTrees` (pilot), `hubRebuild`, `hubLimits`, `hubEffort` (at least high),
  `hubFixRounds`;
- the restatement review of the run's new Topic Idea Cards (`zt-digest restate`, one judge job).

The two follow-ups moved in idea-grow 3.1.0 (zt-grow-cycle v10, zt-digest 0.11.0):

- the concept review of each source book's term table against the Topics (`grow -Step
  concept-books` stages a `zt-digest concepts` folder per book from its term-table Card; one judge
  job per book), whose reports the register plan reads as `-Review` (`hubs-report -Concepts`;
  `register -Review` takes several reports), in mode full and mode hubs;
- the Topic maintenance of the old Organize as an optional round before the hubs (`organize: true`):
  `organize-survey -Topics` surveys the run's Topics, one DeepSeek job decides the merges and moves,
  `grow -Step organize-apply` checks them and runs `organize-apply -Open`.

## Decisions made while building it

- A group files only in its own book's Literature tree: `group-finish` refuses a `create_successor` beside another book's Card (that Card is a Link target only). The Bridge may still add a Link sentence to another book's Literature Card, only for an explicitly accepted report id.
- The book's own Literature Idea Cards are Bridge sources (before, the Bridge's Idea sources were Topic Cards given to the book by Link votes).
- The card cycle and the Link gaps count no Link from or to a Topic-tree Card; the read-only audit still reports the box's Topic inbound Links and hubs as information. After zt-idea-grow cites a Literature Card, the two may count its orphan state differently.
- Figures edit only Cards of the book's own Literature tree.
- Left in the code but unreachable from a digest: `earlier::Pack::place_beside` (always empty), the `hubs` ledger layer and its publication scope, and the Topic sections of `organize-survey` (idea-grow's Topic maintenance uses them, through `organize-survey -Topics`). The modules `zt-concepts`, `zt-review` and `zt-hubs` and the library's hub settings were deleted in base 5.1.0 (`zt-digest-lib` v33).
- A digest's report names its follow-up: `next` is the zt-idea-grow run for the book (base 5.1.0, `zt-digest-report` v16), because the book has no Topic Card, Hub Card or register entry until that run.
- Idea-grow 3.2.0 (`zt-grow-cycle` v11) sends the per-book restatement and concept judge jobs in one dispatch, logs the live-box rounds a box-copy run does not run, and guides the sections of mathematics textbooks.

## Checklist

- [x] 1. write scope in zt-check mcp (`mcp::scope`) + zt-digest main (`literature_scope`) + `call` (Literature by default, `-Open` for hand Topic work); group-finish checks each draft call; run option `hubs` always off
- [x] 2. Literature-only reads: book_of (no Link votes), `snapshot::read_literature_trees` in group-start, prepare-link, link_gap, card cycle; earlier pack and register targets; Bridge targets; skeleton (no Topic bodies, no Topic key Cards)
- [x] 3. digest prompts (dsh-digest-base, dsh-card, dsh-fill, dsh-link, dsh-model-cards, dsh-skeleton; legacy dsh-instructions.zh.md deleted, prepare's default is dsh-digest-base + dsh-model-cards); zt skill and zt-mcp.ps1 document `-Open`
- [x] 4. base 5.0.0 / econ 6.0.0 / math 5.0.0 blueprints, modules (zt-organize v6, zt-digest-report v15, zt-digest-prepare v11), sims and framework harness done (harness passes); PowerShell tests: fixtures pass `-Open`, all pass (2026-10-22: 91 test files)
- [x] 5. idea-grow 3.0.0 (zt-grow-cycle v9): mode hubs, hub options (`grow -Step hubs-plan -Limits/-Trees/-Rebuild`), restatement review (`grow -Step restate-cards`, one `zt-digest restate` chain per source book); sim and grow harness updated; zt-digest 0.10.0
- [x] 6. wiki block docs, reinstall zt-digest and the three workflows, verify, tests, commit
