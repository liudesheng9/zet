## Problem Statement

Readers currently see a short Book term table but cannot open a complete explanation directly from a word in a Card. Mathematical terms omit the depth needed to reproduce definitions and derivations. Separately, the service accepts authored Cards that a later digest self-check rejects, so format/quality legality depends on which client wrote the Card. Existing stored faults, Locked trees, and Windows service lifetimes make a second checker or indiscriminate enforcement unsafe.

## Solution

Give each book term an ordinary addressable Term Card with a full source-bound explanation and optional continuation parts. Link visible words directly to those Cards without address chips in prose. Run one shared checker both in the service transaction and as a pre-submit tool, keeping owner/agent policy distinct and old faults editable. Extend the current archive, Link lifecycle and digest compiler pattern so deterministic linking preserves every visible body byte and generation/recovery remains bounded and auditable.

## User Stories

1. **US001** As a reader, I want one definition for Card legality, so that self-check and storage agree.
2. **US002** As a maintainer, I want one process-free rule core in the existing checker crate, so that reuse adds no gate service or dependency loop.
3. **US003** As a digest tool author, I want the existing run_lines interface and fixture ordering, so that current workflows retain behavior during extraction.
4. **US004** As a Card author, I want structured actionable diagnostics, so that I can fix the exact rejected requirement.
5. **US005** As an owner, I want retained legacy faults to remain advisory, so that old Cards stay editable.
6. **US006** As an owner, I want new duplicate faults to be distinguished from retained faults, so that edits cannot quietly worsen quality.
7. **US007** As an author, I want new type and formal-role rules applied on metadata changes, so that retyping cannot bypass format checks.
8. **US008** As an owner, I want archive- and Workspace-specific policy, so that human notes and agent digest standards can differ.
9. **US009** As an owner, I want validated policy set/reset and visible defaults, so that configuration typos cannot weaken unrelated checks.
10. **US010** As an author, I want policy advice and hard invariants distinguished, so that warnings do not grant Access or unlock a tree.
11. **US011** As a maintainer, I want graph-only rules kept in audits, so that writes do not scan the whole box.
12. **US012** As a Card author, I want every write route checked transactionally, so that GUI, Session, shell and MCP agree.
13. **US013** As an agent, I want bounded accurate proposed-Card context, so that checks use the state actually being committed.
14. **US014** As an author, I want warnings on accepted writes, so that I can improve a stored Card without losing my draft.
15. **US015** As an author, I want refusal rollback without allocated Cards or receipts, so that failed writes leave no partial work.
16. **US016** As an owner, I want pure moves and generated Reverse text handled by existing structural rules, so that navigation maintenance is not blocked by prose heuristics.
17. **US017** As an agent, I want ordered atomic checked batches, so that one invalid item cannot partially publish a batch.
18. **US018** As an agent, I want rollback-only previews that collect independent failures, so that I can correct a plan before writing.
19. **US019** As an agent, I want blocked preview dependencies named honestly, so that unevaluated candidates are not reported as checked.
20. **US020** As a writer, I want exact draft text and metadata checked before submission, so that I receive the gate answer early.
21. **US021** As a writer, I want honest incomplete offline outcomes, so that missing archive context is not called a pass.
22. **US022** As a maintainer, I want the running daemon rule/policy/capability contract, so that stale clients cannot promise acceptance.
23. **US023** As a workflow author, I want consistent blocker/advisory counts and exit codes, so that all check consumers interpret results the same way.
24. **US024** As an owner, I want stored Card Workspace respected by live checks, so that a client flag cannot reclassify permissions.
25. **US025** As a reader, I want a dedicated Term Card type and exact term name, so that definitions are discoverable without title guessing.
26. **US026** As an owner, I want old archives upgraded without identity loss, so that existing Locks and references remain valid.
27. **US027** As an owner, I want term names included in authored identity, so that a Locked definition cannot change through new metadata.
28. **US028** As an author, I want clear omitted/null/set field semantics, so that editing and retyping are predictable.
29. **US029** As a reader, I want computed lists, heads, parts and uses, so that term structure needs no second ownership store.
30. **US030** As an owner, I want duplicate/missing/extra/misfiled term faults, so that coverage debt remains visible.
31. **US031** As an existing reader, I want the original four-column term table unchanged, so that R6 and idea-grow retain their current input.
32. **US032** As a beginning researcher, I want plain explanations and the source book meaning, so that I can learn a term without already knowing its mathematics.
33. **US033** As a researcher, I want full mathematical definitions, hypotheses and notation, so that I can see exactly what a formula assumes.
34. **US034** As a researcher, I want every derivation step, reason, limit and checked example, so that I can reproduce the explanation.
35. **US035** As a reader, I want bounded continuation parts for long explanations, so that depth does not require an unreadable single Card.
36. **US036** As an owner, I want an automatic-link opt-out that survives export/staging loss, so that a too-general term stays unlinked.
37. **US037** As a reader, I want a direct word-to-Term-Card link, so that I can open the definition from the sentence.
38. **US038** As a maintainer, I want exact UTF-8 macro/label/target ranges, so that rewrites preserve label and surrounding text.
39. **US039** As an author, I want wrong-target and malformed Term uses rejected, so that new stored references remain valid.
40. **US040** As a reader, I want generated Term uses separate from Reverse reasoning Links, so that I can inspect definition usage without confusing it with argument.
41. **US041** As an author, I want Term links rewritten correctly on moves, so that definitions remain reachable.
42. **US042** As an owner, I want Term uses included in Lock pins and Access rewrite checks, so that new syntax cannot bypass existing promises.
43. **US043** As a reader, I want broken and stale uses with their label/source/target, so that I can identify what needs attention.
44. **US044** As a reader, I want quality checks to read visible term labels, so that markup does not remove words from a sentence.
45. **US045** As a digest author, I want Literature scope to inspect actual Term targets, so that label syntax cannot hide a Topic write.
46. **US046** As a prompt consumer, I want optional label-only reads marked as projections, so that read-only jobs save clutter without corrupting edit bases.
47. **US047** As a GUI reader, I want quiet label-only links distinct from ordinary chips, so that definitions do not overwhelm prose.
48. **US048** As a keyboard user, I want focusable navigation and accessible broken/hover meaning, so that I can use Term links without a mouse.
49. **US049** As a GUI reader, I want an ordinary explanation Card with collapsed uses, so that the new content fits the existing reading model.
50. **US050** As a GUI author, I want term type/name editing in the existing desk, so that I need no separate authoring application.
51. **US051** As a GUI author, I want name completion disambiguated by book/address, so that I insert the intended direct use.
52. **US052** As an author, I want ordinary reasoned citations to Term Cards still available, so that a claim about a definition remains distinct from a word use.
53. **US053** As a Session reader, I want label-only following with accurate display widths, so that hidden address bytes do not break navigation.
54. **US054** As a Session author, I want raw editing and correct wrapping/copying, so that I can inspect and preserve the stored syntax.
55. **US055** As a researcher, I want Term uses excluded from reasoning credit and Models, so that a vocabulary reference does not become an argument.
56. **US056** As a graph reader, I want Term edges hidden by default and explicitly showable, so that definitions do not dominate every graph.
57. **US057** As an owner, I want portable dumps preserving names/types/raw macros, so that backup and export retain authored meaning.
58. **US058** As a maintainer, I want older supported clients to refuse incompatible archives, so that they cannot silently erase new fields.
59. **US059** As a digest manager, I want source-bound deterministic term jobs, so that writers read only relevant passages.
60. **US060** As a digest manager, I want recorded math classification and promotion, so that formal definitions receive the right review.
61. **US061** As a maintainer, I want the existing ledger and compiler artifacts reused, so that the feature adds no second scheduler.
62. **US062** As a researcher, I want schema/format checks before publication, so that incomplete content is not stored as a finished definition.
63. **US063** As a researcher, I want independent review of every exact mathematical draft, so that the writer is not its only judge.
64. **US064** As a manager, I want one finite fix and re-review with hash binding, so that changed content cannot inherit stale approval.
65. **US065** As an owner, I want the required Haiku and official DeepSeek roles, so that management does not become prose authorship.
66. **US066** As an operator, I want writer/reviewer lifetime caps shared with digest work, so that parallel branches remain bounded without deadlock.
67. **US067** As an operator, I want collected/deleted conversations and cleaned workspace registrations, so that provider resources do not leak.
68. **US068** As an owner, I want ordered prefix publication without placeholders, so that table order survives a failed early draft.
69. **US069** As an operator, I want receipt recovery after a lost publication reply, so that a retry creates no duplicate Term head.
70. **US070** As an owner, I want truthful publication coverage, so that a partial book is never marked done.
71. **US071** As a maintainer, I want stable plan revisions and historical-content conflicts, so that new drafts do not overwrite accepted Cards blindly.
72. **US072** As an owner, I want one-book scoped deterministic link planning, so that only eligible agent Literature prose changes.
73. **US073** As a reader, I want first/all occurrence policy with conservative blockers, so that useful words link without false substring uses.
74. **US074** As an author, I want all Markdown/math/code/source spans protected, so that linking preserves syntax and scientific content.
75. **US075** As an owner, I want lexical stale-link normalization and exact visible-text equality, so that missing targets do not prevent safe repair.
76. **US076** As a reviewer, I want a deterministic preview and skip counts, so that I can assess every planned substitution.
77. **US077** As an operator, I want guarded bounded apply and explicit replanning, so that concurrent edits are never overwritten.
78. **US078** As an operator, I want per-chunk committed/pending reports and recovery, so that a large book remains resumable.
79. **US079** As an owner, I want zero-write idempotent reruns, so that unchanged content produces no revisions.
80. **US080** As an operator, I want typed Lock and quality stops, so that the tool responds correctly without rewriting prose to pass.
81. **US081** As a digest manager, I want Term creation beside digest and linking after all writers, so that the final bodies receive their definitions.
82. **US082** As a workflow maintainer, I want truthful ledger dependencies and disabled-layer behavior, so that missing work cannot be marked complete to unblock a stage.
83. **US083** As an owner, I want audits/reports even when a branch fails, so that I see incomplete coverage rather than a lost run.
84. **US084** As an owner, I want dedicated one-book backfill mode, so that finished models need not be digested again.
85. **US085** As an operator, I want sections-only source reconstruction and deterministic-only provider-free mode, so that resuming does only necessary work.
86. **US086** As an owner, I want Lock/source/occupancy conflicts before dispatch, so that historical work is preserved.
87. **US087** As a reader, I want Term links restored after actual correction rounds, so that final edits do not silently remove definitions.
88. **US088** As a maintainer, I want generation-specific final units and mapping invalidation, so that later corrections are not skipped.
89. **US089** As an owner, I want coverage, T faults, uses and remaining avoid words in final reports, so that execution success is distinct from quality completeness.
90. **US090** As a writer, I want exact pre-submit SELF-CHECK guidance, so that faulty writes can be corrected before the gate.
91. **US091** As a workflow author, I want card_check routed through the finite fix cycle, so that content refusals are not provider crashes.
92. **US092** As an owner, I want card_locked treated as a planning stop, so that writers cannot evade a frozen tree.
93. **US093** As a maintainer, I want compatible frozen builds and current version/capability manifests, so that deployment does not mix incompatible products.
94. **US094** As an agent, I want updated domain, command and skill guidance, so that Term work follows the same boundaries everywhere.
95. **US095** As an owner, I want new enforcement staged behind compatibility evidence, so that existing writers are not stranded mid-run.
96. **US096** As a maintainer, I want public-behavior evidence for the full requirement matrix, so that passing a narrow fixture does not imply feature completion.
97. **US097** As an operator, I want measured gate/context/batch/relink performance, so that bounded targets are supported by actual evidence.
98. **US098** As an owner, I want explicit limitations of fake runtimes and heuristic checks, so that orchestration tests do not claim semantic or live qualification.
99. **US099** As an owner, I want a copied-book mathematical depth review, so that the explanations meet the requested learning standard.
100. **US100** As an owner, I want a copied-book layout, preservation and resource-use review, so that the visible feature is useful and repeatable.
101. **US101** As an owner, I want a scheduled compatible live release with verified backups, so that deployment preserves the active archive.
102. **US102** As an owner, I want advisory observation and deliberate enforcement promotion, so that a timer cannot switch my policy automatically.
103. **US103** As an owner, I want documented policy rollback and schema recovery boundaries, so that rollback does not silently discard later work.
104. **US104** As an owner, I want selection of each historical book and Lock disposition, so that pilot approval is not blanket backfill authority.
105. **US105** As an owner, I want one audited historical backfill with immutable prior prose, so that definitions are added without rerunning the book.

## Implementation Decisions

1. Keep the existing three executables, SQLite archive, Link index, writer transaction/mutex, digest ledger, receipts, and frozen workflow mechanism. Add no terminology database, gate service, second scheduler, generic plugin framework, ownership table, or second Link parser.
2. Use one pure checker core with optional CLI/MCP/process adapters. The root links it without those adapters; the standalone checker and digest retain existing library interfaces and fixtures.
3. Share envelope/lexical Link/visible-text/table operations; root address grammar, storage/Link existence, Access and Locks remain service facts. Materialize a bounded immutable context value rather than hide I/O behind a trait hierarchy.
4. Check the final proposed Card and metadata inside the transaction on every authoring route. Preserve warnings in operation results and typed card_check data across CLI/API/MCP/batch boundaries.
5. Use validated per-archive/Workspace/rule metadata overrides, shared policy/catalog/limits, applicability-aware structured multiset legacy comparison, and audit-only graph rules.
6. Add one nullable term_name field and type term. Compute list/head/part roles by name and Location. Exact heads are Side children in table order; long explanations use same-name Direct parts.
7. Preserve the term-list authored title/body/place and four-column table. Persist disabled automatic linking and its required reason in authored Term-head lines rather than another column.
8. Upgrade Card constraints and Lock protection together, preserve old null-name fingerprints/row ids/seals/history, synchronize distinct receipt projections, and use a tested generation-2 compatibility floor.
9. Accept direct labeled Term macros with target-only rewrites and separate generated Term uses. Keep them in Link/Access/Lock integrity while excluding reasoning/Model semantics and default graph edges.
10. Render labels quietly in GUI/Session, retain ordinary reasoned definition citations and raw editing, provide marked opt-in label-only reads, and preserve exact authored exports.
11. Generate source-bound JSON with official DeepSeek V4.1 Flash/high effort under Haiku low-effort DSMP managers. Reuse shared caps and cleanup, count idle writers, and review changed math independently after at most one writer fix.
12. Publish the longest ready contiguous row prefix through existing legal chunks and durable receipts. No placeholders, address reservations, historical blind overwrites, or unbounded transaction.
13. Use a deterministic source-offset literal longest-match/blocker pass, first occurrence by default/all optional, conservative protected spans, lexical stale-link normalization and an exact stripped-body equality check.
14. Guard every relink edit, replan explicitly on changed inputs, report committed/pending chunks, and make unchanged reruns zero-write/no-revision operations.
15. Run Term creation beside digest from frozen source, link after all writers, relink actual finalization changes per generation, and keep failed coverage visibly incomplete while audit/report run.
16. Provide dedicated one-book backfill, deterministic-only relink, sections-only preparation, and receipt-aware resume. The workflow cannot unlock trees, rewrite synonyms/prose/notation tables, or rerun completed model work.
17. Ship draft/preview/version/refusal compatibility before enforcing new quality rules; coordinate all binaries/workflows/skills, pilot on a copy, observe advisory results, and promote policy deliberately with the owner.

### Policy defaults and mandatory boundaries

The human Workspace is advised on language/formal-model style, blocked on newly applicable E/F/type/open-formula format, and defaults R6 off. The agent Workspace blocks applicable language/formal-model/type rules, with R6 advice. I1, R20/R21, unknown-source I7, approximate T4, graph and term coverage/structure remain advice/audit. T1/T2/T3/T5/T6/T9/T10 enforce applicable term format; T10 shares R22. Mandatory envelope/field/Link/Access/Lock invariants cannot be disabled. The attached requirement register below retains every exact rule grouping and exception.

### Delivery slices and dependencies

- **S01 — Run the existing Card checker as a reusable process-free library** (AFK). Blocked by: none. Stories: US001, US002, US003.
- **S02 — Keep legacy Cards editable using structured fault identity** (AFK). Blocked by: S01. Stories: US004, US005, US006, US007.
- **S03 — Configure one archive-specific Card-check policy** (AFK). Blocked by: S02. Stories: US008, US009, US010, US011.
- **S04 — Check every authored write inside its existing transaction** (AFK). Blocked by: S03. Stories: US012, US013, US014, US015, US016.
- **S05 — Preview checked batches and refuse real batches atomically** (AFK). Blocked by: S04. Stories: US017, US018, US019.
- **S06 — Self-check exact drafts with the running service check contract** (AFK). Blocked by: S05. Stories: US020, US021, US022, US023, US024.
- **S07 — Create Term Cards without weakening archive and Lock identity** (AFK). Blocked by: S05. Stories: US025, US026, US027, US028.
- **S08 — Read the computed term structure and coverage faults** (AFK). Blocked by: S07. Stories: US029, US030, US031.
- **S09 — Render and check complete term explanations and continuation parts** (AFK). Blocked by: S10. Stories: US032, US033, US034, US035, US036.
- **S10 — Write direct Term links through the shared Link parser** (AFK). Blocked by: S08. Stories: US037, US038, US039.
- **S11 — Maintain Term uses through moves, Reverse sections, and Locks** (AFK). Blocked by: S10. Stories: US040, US041, US042, US043.
- **S12 — Preserve visible term prose in checks and projected reads** (AFK). Blocked by: S11. Stories: US044, US045, US046.
- **S13 — Read quiet Term links and explanation Cards in the GUI** (AFK). Blocked by: S12. Stories: US047, US048, US049.
- **S14 — Author Term Cards and complete Term links in existing editors** (AFK). Blocked by: S13. Stories: US050, US051, US052.
- **S15 — Follow label-only Term links in the terminal Session** (AFK). Blocked by: S12. Stories: US053, US054.
- **S16 — Keep Term uses out of reasoning and default graph views** (AFK). Blocked by: S12. Stories: US055, US056.
- **S17 — Round-trip Term data through portable dumps and compatible clients** (AFK). Blocked by: S11. Stories: US057, US058.
- **S18 — Plan source-bound Term-card jobs for one book** (AFK). Blocked by: S09, S12. Stories: US059, US060, US061.
- **S19 — Check, independently review, and fix Term drafts once** (AFK). Blocked by: S18. Stories: US062, US063, US064.
- **S20 — Manage Term writers and reviewers within the shared DSMP pool** (AFK). Blocked by: S19. Stories: US065, US066, US067.
- **S21 — Publish a ready ordered prefix of Term Cards with receipts** (AFK). Blocked by: S19. Stories: US068, US069, US070, US071.
- **S22 — Plan exact text-preserving Term links deterministically** (AFK). Blocked by: S12. Stories: US072, US073, US074, US075, US076.
- **S23 — Apply guarded Term-link plans and recover interrupted chunks** (AFK). Blocked by: S06, S22. Stories: US077, US078, US079, US080.
- **S24 — Run parallel Term stages with truthful full-mode dependencies** (AFK). Blocked by: S20, S21, S23. Stories: US081, US082, US083.
- **S25 — Backfill selected books through dedicated Term modes** (AFK). Blocked by: S24. Stories: US084, US085, US086.
- **S26 — Relink finalized Cards and audit complete term coverage** (AFK). Blocked by: S24. Stories: US087, US088, US089.
- **S27 — Make all writers self-check and recover Card-check refusals** (AFK). Blocked by: S06, S19. Stories: US090, US091, US092.
- **S28 — Publish compatible frozen workflows and agent guidance** (AFK). Blocked by: S14, S15, S16, S17, S25, S26, S27. Stories: US093, US094, US095.
- **S29 — Qualify integrated behavior and bounded performance on copies** (AFK). Blocked by: S28. Stories: US096, US097, US098.
- **S30 — Review explanatory depth and Term-link usability on a copied book** (HITL). Blocked by: S29. Stories: US099, US100.
- **S31 — Qualify live release and promote gate policy deliberately** (HITL). Blocked by: S30. Stories: US101, US102, US103.
- **S32 — Apply one owner-selected historical Term backfill** (HITL). Blocked by: S31. Stories: US104, US105.

### Architecture, invariants, scope and risk requirements

- **R0001 (design 0)** Status: implementation design, 2026-10-24. This document proposes changes; it does not report an implementation, a deployment, or a change to the archive.
- **R0002 (design 0)** Inputs: Term Cards and Term Links proposal and zt-check gate proposal. The implementation baseline inspected for this design is commit `bab6bd7`. The proposals remain the feature specification; this document resolves how to build the features together.
- **R0003 (design 0)** Navigation: Architecture · Design decisions · Shared checker · Policy and ratchet · Term storage · Term-card compiler · Deterministic linking · Workflows · Verification · Feature coverage.
- **R0004 (design 1)** Keep the three existing executables, the existing archive, and the existing digest ledger. Turn the existing `zt-check` crate into a pure checking library with an optional command-line adapter. Link that library into `zt`. Teach the existing Link machinery the Term-link syntax. Add one term-name column to Cards, one derived term-structure module, and two digest commands.
- **R0005 (design 1)** A Term list, a Term Card, and a Term part all have type `term`. Their role comes from their term name and Location. They are Cards in the current archive, with the current Access, Lock, move, revision, receipt, and dump behavior. Their explanation is authored Markdown, rather than a second definition store.
- **R0006 (design 1)** The service checks a proposed Card against the same rule implementation that `zt-check` uses. The service owns the final decision because it has current transaction context. The standalone tool predicts that decision when it has matching rules, policy, limits, metadata, and context. An incomplete offline check must say that it is incomplete.
- **R0007 (design 1)** The new digest work consists of:
- **R0008 (design 1)** `zt-digest term-cards`: plan source-bound jobs, accept DeepSeek JSON, check and review it, render Cards, then publish in table order through existing receipts.
- **R0009 (design 1)** `zt-digest term-link`: compute byte-range substitutions in one book, show a plan, then apply guarded edits. No model chooses or writes a substitution.
- **R0010 (design 1)** Do not create a terminology database, a gate process, a plugin framework, a new queue, a term-ownership table, or an independent Link parser for the digest.
- **R0011 (design 1.1)** `the shared rule core` — The existing rules, structured diagnostics, and Card checks without I/O — The service must use the actual checker without spawning itself
- **R0012 (design 1.1)** `shared text operations` — Shared envelope and lexical Link parsing, visible-text projection, and term-table parsing — Labels, scopes, rendering, and checks must agree on the text and target of a Term link
- **R0013 (design 1.1)** `shared rule policy` — Rule catalog, defaults, overrides, severity, and legacy-fault comparison — The gate and self-check must make the same decision
- **R0014 (design 1.1)** `the service gate adapter` — Load bounded transaction context and call the pure checker — Storage reads and write results belong in `zt`
- **R0015 (design 1.1)** `the term-structure module` — Compute list/head/part roles and the term structure from Cards — Readers and write validation need one definition of a Term Card
- **R0016 (design 1.1)** `the Term-card compiler` — The Term-card compiler — Generated explanations need checking, review, publication, and recovery
- **R0017 (design 1.1)** `the Term-link command` — The deterministic link pass — Linking needs a reusable plan/apply operation
- **R0018 (design 1.1)** Two workflow modules — Wire these commands into the existing compiler and relay paths — The two new stages have different execution contracts
- **R0019 (design 1.1)** Two prompt files — Writer instructions and independent mathematical review — Content creation and review need separate tasks
- **R0020 (design 1.1)** `core.rs`, `text.rs`, and `policy.rs` are files inside the existing checker crate, not new crates. Preserve `check.rs` as the CLI/input compatibility adapter. Do not split the rule implementation into one module per rule family merely to mirror the proposal diagram. Extract a further module only when a concrete responsibility becomes hard to navigate.
- **R0021 (design 1.1)** Production storage changes are one nullable `cards.term_name` column and the widened Card-type constraint. Use existing `metadata` rows for archive policy and existing `links.kind` for `term` occurrences. No new persistent table is required.
- **R0022 (design 1.2)** Architecture/stage flow contract: ; inputs["GUI, Session, shell, MCP, apply_batch"] --> tx["Existing write transaction"]; tx --> gate["gate.rs: proposed Card + bounded context"]; gate --> check["zt-check core + policy + text"]; check --> decision["Accept with warnings, or card_check"]; cli["zt-check CLI: drafts or archive reads"] --> check; drafts["DeepSeek JSON + independent math review"] --> compiler["zt-digest term-cards"]; compiler --> inputs; planner["zt-digest term-link: deterministic plan"] --> inputs; tx --> archive[("Existing SQLite archive")]; archive --> readers["Existing Card, GUI, Session, dump, and MCP readers"]
- **R0023 (design 2)** The following are source observations at the baseline, rather than assumptions taken from either proposal.
- **R0024 (design 2)** `the checker library` and its Cargo manifest — The crate already exports `check`, `graph`, and `mcp`; process code is not feature-separated — Separate the existing crate; do not introduce another checker crate
- **R0025 (design 2)** `the checker CLI/input adapter` — Input loading, MCP calls, rules, text helpers, printed diagnostics, and policy-like filtering share one file — Extract pure computation before putting it in the service
- **R0026 (design 2)** `the service and CLI module validate_card_text` — Parses the envelope, checks a repeated leading title, and calls `link_index::validate` — Retain this compatibility entry point, but pass proposed Card metadata and delegate checking
- **R0027 (design 2)** `shared Card operations edit_tx` — Resolves type/facet/level/form changes separately; a metadata-only edit need not validate text — Checking only `validate_card_text` would miss retypes and metadata-only changes
- **R0028 (design 2)** `placement operations` and shell paths — Some writers call the existing validator directly — Audit all authoring paths, including direct callers, rather than adding a gate only to MCP
- **R0029 (design 2)** `the service and CLI module cards_table` — SQLite restricts `card_type` with a CHECK listing five types — `ALTER TABLE ... ADD COLUMN` alone cannot add type `term`
- **R0030 (design 2)** `the service and CLI module ensure_finding_cards` — Existing type expansion backs up and rebuilds `cards` while preserving row ids — Follow this pattern, accounting for the newer Lock triggers and tables
- **R0031 (design 2)** `the Card envelope and fingerprint module` — Parsing normalizes body line endings and outer blank lines; Reverse text is generated — The byte-preservation contract applies to the body returned by the Card parser, not arbitrary original file bytes
- **R0032 (design 2)** `the root Link facade/lifecycle` — One scanner supplies occurrence and target byte ranges; it currently excludes fenced and inline code — Reuse the scanner; do not assume it already excludes every math or HTML span
- **R0033 (design 2)** `the derived Link index` — The derived occurrence table has `source, ord, target, kind`; normal writes refresh affected sources/targets — Store `kind='term'` there and keep authored macros authoritative
- **R0034 (design 2)** `Model relations/projections read_links` — Relation kinds are inferred from prose, with inheritance between adjacent Links — Syntax-assigned `term` must not become an inherited reason for an ordinary Link
- **R0035 (design 2)** `the batch.rs functional surface` — Real batches accept 1–64 items and at most 1 MiB of authored input — A 300-Card book cannot be one real batch
- **R0036 (design 2)** `the batch.rs functional surface` — Preview sequences already exist, with up to 1,024 items and 8 MiB — Extend the existing simulation to aggregate Card-check refusals
- **R0037 (design 2)** `identified digest calls and guards` — Identified calls already partition real writes by item/byte limits and recover through receipts — Use this path rather than a new bulk-write endpoint
- **R0038 (design 2)** `the Card envelope and fingerprint module` and `the Tree Lock protection module` — Authored fingerprints, SQL checks, member seals, and frozen field lists predate `term_name` — Adding a field must also update these integrity contracts
- **R0039 (design 2)** `identified digest calls and guards authored_fingerprint` — Digest request guards use their own canonical JSON projection — Update that projection and the service receipt projection together; do not substitute the Lock hash
- **R0040 (design 2)** `the service and CLI module Config` — Existing configuration is a small set of declared process settings — Archive-specific gate overrides need explicit persistence, not an assumption that arbitrary config keys already work
- **R0041 (design 2)** Digest modules and base blueprint — Existing compilers use `Z.layer`, `Z.chain`, relay commands, claims, receipts, and frozen workflow builds — Add two modules to that machinery; do not build another orchestrator
- **R0042 (design 2)** The governing domain remains CONTEXT.md, with the block map in the developer wiki. The current wiki statement that body conventions are taught rather than validated is a baseline decision that this change intentionally replaces for configured gate rules.
- **R0043 (design 3)** These are recommended design defaults. They make the design implementable without requiring answers to the proposals' open questions before writing this document. They are not permission to backfill or deploy.
- **R0044 (design 3)** Card type — Add `term`, as requested; do not simulate it with title conventions on Idea Cards
- **R0045 (design 3)** Role storage — Store only `term_name`; compute list/head/part roles
- **R0046 (design 3)** Placement — Immediate Side successors of the list for term heads; Direct successors for continuation parts
- **R0047 (design 3)** Term title — The exact term name on a head; continuation titles use `推导（续 k/n）：…`
- **R0048 (design 3)** List preservation — Retype only; preserve parsed title, authored body, row identity, address, and authored Links
- **R0049 (design 3)** Link syntax — `[[label@address]]` with a direct Regular Location target
- **R0050 (design 3)** Link frequency — `first` per term per body; support `all` explicitly
- **R0051 (design 3)** Link scope — One agent Literature tree for the digest pass
- **R0052 (design 3)** Checker process — In-process library inside `zt`
- **R0053 (design 3)** Self-check service path — Reuse `apply_batch preview`; do not add `check_cards` in the first release
- **R0054 (design 3)** Checker context — A concrete read-only value supplied to pure rules, rather than a storage-calling trait hierarchy
- **R0055 (design 3)** Policy — The proposal's human/agent defaults, with archive-specific overrides and a staged enforcement rollout
- **R0056 (design 3)** Term generation — DeepSeek V4.1 Flash, official source, high effort; Haiku management through the existing DSMP path
- **R0057 (design 3)** Mathematical review — Every math term is reviewed in a separate conversation; a changed draft invalidates its previous review
- **R0058 (design 3)** Backfill — One selected book at a time after the copy-based pilot; never automatically unlock a tree
- **R0059 (design 3)** Optional later work — Entry-search ranking for exact term names and idea-grow/register integration remain explicit follow-ups
- **R0060 (design 3.1)** **Bounded publication instead of one unbounded transaction.** Both new commands use existing receipt-backed chunks. A chunk is atomic; an entire large book is recoverable but not atomic. Reports distinguish planned, committed, pending, and refused items. This preserves the current batch contract.
- **R0061 (design 3.1)** **Checked publication prefixes instead of arbitrary holes.** The proposal permits successful drafts to publish while others fail, and also demands table-row placement order. Publish the longest contiguous prefix of ready terms, never a later row ahead of an unresolved earlier row. Keep later ready drafts on disk. This supports partial progress without address reservations, placeholders, or moving already published Cards. Section 12 specifies recovery.
- **R0062 (design 3.1)** **Conditional prediction instead of unconditional offline acceptance.** An offline check without the archive's policy or required context cannot guarantee acceptance. It returns an incomplete result; the service preview is the authoritative preflight.
- **R0063 (design 3.1)** **Stable faults instead of printed-line identity.** Use structured, unclipped diagnostic identity and multiplicity for the legacy ratchet. Printed lines remain the human-readable interface.
- **R0064 (design 3.1)** **One shared lexical implementation.** Move reusable text/Link scanning into the existing checker crate's pure text module. `the root Link facade/lifecycle` remains the service-facing Link facade and lifecycle implementation, including address-grammar and storage checks. This avoids a dependency cycle and avoids three inconsistent regex implementations.
- **R0065 (design 3.1)** **A persisted auto-link choice.** `link:false` must survive loss of the staging directory. Store it as an authored head line on the Term Card, rather than adding another column or relying on the draft file forever.
- **R0066 (design 3.1)** **Self-checks precede enforcement.** Ship the compatible CLI, preview support, writer instructions, and `card_check` recovery before turning new agent blocking defaults on. The proposal's rollout list placed some of these after enforcement; that ordering would strand existing writers.
- **R0067 (design 4)** These invariants define successful implementation.
- **R0068 (design 4)** A Card's authored Markdown is the source of truth for its explanation and Link occurrences.
- **R0069 (design 4)** A field or table computed from Cards is a projection, never an independent authority.
- **R0070 (design 4)** Every authoring route checks the final proposed text and metadata inside the write transaction.
- **R0071 (design 4)** Checking uses the proposed Workspace, type, facet, level, form, and term name, not the row's previous type.
- **R0072 (design 4)** An agent can change only agent Cards. Gate policy cannot grant Access or unlock anything.
- **R0073 (design 4)** A Lock refusal stops the affected operation without retries through another route.
- **R0074 (design 4)** Existing envelope, field, Link, Access, and Lock invariants remain mandatory.
- **R0075 (design 4)** A Term link reaches a term head directly; it does not resolve a name through a registry.
- **R0076 (design 4)** A Term use contributes no reasoning edge, Model membership, or Idea inbound/outbound credit.
- **R0077 (design 4)** Removing recognized Term-link markup preserves exactly the visible authored body bytes.
- **R0078 (design 4)** A rerun with unchanged inputs makes no authored edits or new Card allocations.
- **R0079 (design 4)** Preview changes no persisted Card, receipt, revision, allocation, or Lock history.
- **R0080 (design 4)** A refusal rolls back the actual atomic operation, including receipts and allocations.
- **R0081 (design 4)** Unknown context, a stale contract, or an interrupted check cannot be reported as a pass.
- **R0082 (design 4)** Mathematical format checks and model review are evidence of review, not proof of mathematical truth.
- **R0083 (design 4)** A run with missing terms or unresolved refusals remains visibly incomplete even when the audit and report run successfully.
- **R0576 (design 20.1)** G1 / D1–D4: new type, unchanged list, one term per row, parts and order — Sections 9, 11, 12
- **R0577 (design 20.1)** G2–G4 / D5–D10: direct label link, deterministic text-preserving pass, first/all, final pass — Sections 10, 13, 14
- **R0578 (design 20.1)** G5: R6 and idea-grow keep reading the list — Sections 9.3, 10.4, 14; list body unchanged
- **R0579 (design 20.1)** G6 / D11–D15: parallel branch, DSMP, writer/reviewer/fix, limits/model — Sections 11, 12, 14
- **R0580 (design 20.1)** Proposal 4.4: computed structure and all seven named faults — Section 9.2
- **R0581 (design 20.1)** Proposal 5 and appendices A–C: content schema, deep math, examples, review — Sections 11 and 12; reuse the source schema rather than redefine its content fields
- **R0582 (design 20.1)** Proposal 6: grammar, rewrite, Reverse/use sections, broken targets, Locks — Section 10
- **R0583 (design 20.1)** Proposal 7: GUI, editor, Session, raw/projected reads — Sections 10.4 and 15
- **R0584 (design 20.1)** Proposal 8: steps/jobs/ledger/failure behavior — Sections 12 and 14, with explicit prefix-publication refinement
- **R0585 (design 20.1)** Proposal 9 and appendix D: matching, protected spans, preservation — Section 13
- **R0586 (design 20.1)** Proposal 10: T1–T10, old rule changes, audit counts — Sections 6, 10.4, 11.3, 14.4
- **R0587 (design 20.1)** Proposal 11: CLI/MCP/GUI/Session/digest/skills/docs surfaces — Sections 7, 14, 15, 21
- **R0588 (design 20.1)** Proposal 12–15: backfill, versions, pilot, tests, risks — Sections 16–19 and 22
- **R0589 (design 20.1)** Q1–Q4 — First occurrence; exact term title; no row links; source-based parallel generation
- **R0590 (design 20.1)** Q5 — Supported but separately selected after pilot; no live backfill in this task
- **R0591 (design 20.1)** Q6 / P9 optional — Exact-name entry ranking and register/cross-book integration remain later work
- **R0592 (design 20.1)** Q7 / N5 — No per-symbol Cards or symbol links
- **R0593 (design 20.1)** N1–N4 — Model/notation tables, Topic-side digest scope, and no synonym rewrites preserved
- **R0594 (design 20.1)** The optional label-only read projection is included because it is a small use of the shared text function. Exact-name entry ranking needs its own relevance/pilot decision; it is not silently included in the register tier.
- **R0595 (design 20.2)** One checker, two products, no service-to-itself process — Sections 1, 5
- **R0596 (design 20.2)** Envelope unification and context boundary — Sections 5, 7
- **R0597 (design 20.2)** Local/context/graph split — Sections 5.3 and 6.2
- **R0598 (design 20.2)** Workspace policy and archive overrides — Section 6
- **R0599 (design 20.2)** Legacy ratchet, retypes and all write routes — Sections 6.3 and 7
- **R0600 (design 20.2)** Typed refusal and warnings — Sections 5.4 and 7.3
- **R0601 (design 20.2)** Atomic real batch and exhaustive preview — Section 7.4
- **R0602 (design 20.2)** `--workspace`, `--draft`, blocking-only counts — Section 8
- **R0603 (design 20.2)** Initialization version and stale tool behavior — Section 8.3
- **R0604 (design 20.2)** Literature scope and box pin stay client-side — Sections 10.4 and 14
- **R0605 (design 20.2)** Digest SELF-CHECK and content-fix routing — Section 14.5
- **R0606 (design 20.2)** Phased enforcement and service redeploy implications — Sections 17 and 19
- **R0607 (design 20.2)** Parity, policy, preview, version, latency, legacy tests — Section 18
- **R0608 (design 20.2)** Owner questions — Proposed defaults in sections 3 and 6; existing preview only, no extra check tool
- **R0609 (design 20.3)** No cross-book deterministic term linking, symbol-card system, Topic-side Term generation by the digest, automatic avoid-word rewrite, automatic mathematical theorem proving, automatic historical cleanup, new register authority, or background reconciliation service.
- **R0610 (design 20.3)** The existing owner workflows may link a term across Workspaces, but that does not expand what a digest is permitted to write.
- **R0627 (design 22)** Gate activates before writer compatibility — Deliver draft checks, previews, and finite `card_check` handling before enforcing
- **R0628 (design 22)** Expensive per-Card whole-box reads — Materialize bounded context; keep graph rules audit-only; measure query counts
- **R0629 (design 22)** Old faults become an edit lockout — Structured multiset ratchet with role-aware applicability
- **R0630 (design 22)** New faults hidden behind old clipped text — Compare unclipped semantic diagnostic keys
- **R0631 (design 22)** Wrong or shallow mathematical explanation — Source-bound drafts, complete template, separate review, one fix, human pilot; no proof claims
- **R0632 (design 22)** Auto-link choice lost after staging cleanup — Persist it as an authored metadata line
- **R0633 (design 22)** Partial generation destroys row order — Publish only a ready contiguous prefix
- **R0634 (design 22)** Interruption creates duplicate heads — Stable operation keys, receipt recovery, explicit readback
- **R0635 (design 22)** Target deletion makes old links impossible to normalize — Lexical same-book stripping independent of target existence
- **R0636 (design 22)** Relinker corrupts Markdown or prose — Source offsets, conservative protected spans, preservation equation, stale guards
- **R0637 (design 22)** Labels disappear from style checks — Shared visible-text projection before existing prose rules
- **R0638 (design 22)** Dense Term edges overwhelm reasoning views — Default graph filtering and strict Model/Idea-count exclusions
- **R0639 (design 22)** Schema change invalidates active Locks — Preserve null-field old hashes; update frozen fields, SQL wrappers, triggers, and fixtures together
- **R0640 (design 22)** Policy/version drift creates a false pass — Service-provided check contract, mismatch/incomplete exit status, final transactional decision
- **R0641 (design 22)** Old executable drops new fields — Coordinated capability floor; refuse incompatible writes
- **R0642 (design 22)** Partial run presented as finished — Separate command execution, publication coverage, ledger completion, and audit findings
- **R0643 (design 22)** The resulting structure is small: one existing checker crate becomes reusable, one existing Link lifecycle gains a second syntax form, one existing Card table gains one authored field, and two digest compilers use the existing scheduler and receipts. Detail lives in explicit contracts, rather than additional services or storage layers.

## Testing Decisions

1. Prefer the existing highest public seams: real GUI/browser and token-guarded API, stdio MCP, shell commands, Session line/PTY, actual SQLite transaction/migration/backstop and extracted dumps, with current workflow stand-in/frozen-install harnesses.
2. Test observable stored outcomes and full rollback, hashes, revisions, receipts, resource ids and projections, rather than implementation-shaped mocks or mere message matches.
3. Use one old/new fixture corpus for CLI/core/gate parity, structured ratchet multiplicity/applicability, complete/incomplete context, policy isolation, and stale running-service contracts.
4. Exercise all write routes and metadata-only edits, both Workspaces under their actual Access, ordered batch refusal and exhaustive rollback-only preview with dependent blocked items.
5. Verify Term migrations/Lock generations/fingerprints/dumps, exact parser offsets, indexed/full lifecycle parity, move/rewrite/pin integrity, reasoning exclusions and raw/projected text.
6. Property-test stripped-body preservation and idempotence against Markdown/math/code/Unicode fixtures; inject stale mappings, concurrent edits, interrupted chunks and lost committed replies.
7. Test source classification/promotion, JSON rendering/T1–T10/continuations, independent exact-hash review and one fix, prefix publication, receipt recovery, DSMP waves/caps/cleanup and every workflow mode/finalization generation.
8. Measure median/tail extra gate time and bounded context/query/index work on legal 64-item batches, 200-item preview/four legal batches, and a 300-Card relink fixture. Under 5 ms typical added gate work is a target until measured.
9. Use a real copied-book pilot for owner-reviewed depth (three math/two concept terms), ten GUI Cards, exact list/prose preservation, second-run no-op and measured provider usage/cleanup.
10. Keep live release/advisory observation/enforcement and selected historical backfill as owner gates; stand-in, copy-based, semantic and installed-service evidence prove different things.

## Out of Scope

- No cross-book deterministic Term links, automatic register/idea-grow integration, or exact-name register-tier ranking in the first release; these need a separate follow-up decision after the pilot.
- No per-symbol Card system or symbol links, new notation/model-table structure, or Topic-side Term generation by the digest.
- No synonym/avoid-word correction, prose or mathematical rewrite by the deterministic pass, automatic historical cleanup, theorem proving, or background reconciliation service.
- No automatic unlock/relock, all-book backfill, live digest rerun, archive restore, macro stripping or head deletion to simplify rollback.
- The scope of this publication task is PRD/issues/traceability; implementation, deployment, and live backfill are deliverables of the tracked issues, not actions claimed complete here.

## Further Notes

The local implementation design is the authority for detailed scope (2026-10-24; inspected code baseline bab6bd7; source SHA-256 a27224263597460d0822d7859cd1a822d47e7376bb207686689843fc6d3309d4). This PRD and its child issues embed their assigned requirements rather than relying on a local-only link. The proposed public test seams are the existing ones identified above; publication and these seams are covered by the user authorization.

The plan comprises 29 AFK implementation slices and 3 owner-led gates. Labels indicate who can act after blockers complete, not that dependencies have vanished. No existing PRD or unrelated issue is closed or rewritten.

Source coverage is identified by stable R identifiers and design sections; child issues carry the assigned requirement text and Sxx-ACxx acceptance criteria. A repository-local issue map and machine-readable audit will bind the published bodies, native parent/dependency relationships, stories and source requirements.
