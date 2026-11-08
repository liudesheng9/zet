# Term Cards, Term Links, and the shared Card check: PRD and issue map

Status: published and read back from GitHub. This is planning/tracker evidence, not implementation or live-release evidence.

PRD: [#142](https://github.com/liudesheng9/zet/issues/142).

Source: [implementation design](term-cards-links-and-check-gate-implementation-design.md), baseline `bab6bd7`, SHA-256 `a27224263597460d0822d7859cd1a822d47e7376bb207686689843fc6d3309d4`.

Local PRD copy: [PRD](term-cards-links-and-check-gate-prd.md). Machine-readable requirement, story, acceptance and live-body evidence: [coverage audit](term-cards-links-and-check-gate-issue-coverage.json).

## Publication summary

- 32 child issues: 29 AFK implementation slices and 3 owner-led gates.
- 643 source clauses, 105 user stories, 233 child acceptance criteria, and 42 native dependency edges.
- All child native parent links, native blockers, body references, exact titles/bodies, labels, and source-clause ownership were verified by live GraphQL readback.
- Coverage extraction accounts for every nonblank substantive source line; headings/fence delimiters/table headers are structural. Examples are translated into their behavioral/field contracts without copying production code paths into issues.
- Every issue embeds its assigned detailed requirements, stories and acceptance criteria, so it is usable without an unpublished local document.
- User authorization covers publication; implementation, owner pilot, deployment and selected historical backfill remain separate tracked work.

## Implementation issues

| Slice / issue | Type | What it delivers | Blocked by | Stories | Acceptance criteria |
|---|---|---|---|---|---|
| [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | AFK | Run the existing Card checker as a reusable process-free library | None | US001, US002, US003 | 6 |
| [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | AFK | Keep legacy Cards editable using structured fault identity | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | US004, US005, US006, US007 | 8 |
| [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | AFK | Configure one archive-specific Card-check policy | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | US008, US009, US010, US011 | 7 |
| [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | AFK | Check every authored write inside its existing transaction | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | US012, US013, US014, US015, US016 | 9 |
| [S05 / #147](https://github.com/liudesheng9/zet/issues/147) | AFK | Preview checked batches and refuse real batches atomically | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | US017, US018, US019 | 7 |
| [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | AFK | Self-check exact drafts with the running service check contract | [S05 / #147](https://github.com/liudesheng9/zet/issues/147) | US020, US021, US022, US023, US024 | 8 |
| [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | AFK | Create Term Cards without weakening archive and Lock identity | [S05 / #147](https://github.com/liudesheng9/zet/issues/147) | US025, US026, US027, US028 | 9 |
| [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | AFK | Read the computed term structure and coverage faults | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | US029, US030, US031 | 7 |
| [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | AFK | Render and check complete term explanations and continuation parts | [S10 / #151](https://github.com/liudesheng9/zet/issues/151) | US032, US033, US034, US035, US036 | 12 |
| [S10 / #151](https://github.com/liudesheng9/zet/issues/151) | AFK | Write direct Term links through the shared Link parser | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | US037, US038, US039 | 7 |
| [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | AFK | Maintain Term uses through moves, Reverse sections, and Locks | [S10 / #151](https://github.com/liudesheng9/zet/issues/151) | US040, US041, US042, US043 | 7 |
| [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | AFK | Preserve visible term prose in checks and projected reads | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | US044, US045, US046 | 7 |
| [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | AFK | Read quiet Term links and explanation Cards in the GUI | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | US047, US048, US049 | 6 |
| [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | AFK | Author Term Cards and complete Term links in existing editors | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | US050, US051, US052 | 6 |
| [S15 / #157](https://github.com/liudesheng9/zet/issues/157) | AFK | Follow label-only Term links in the terminal Session | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | US053, US054 | 5 |
| [S16 / #158](https://github.com/liudesheng9/zet/issues/158) | AFK | Keep Term uses out of reasoning and default graph views | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | US055, US056 | 6 |
| [S17 / #155](https://github.com/liudesheng9/zet/issues/155) | AFK | Round-trip Term data through portable dumps and compatible clients | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | US057, US058 | 5 |
| [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | AFK | Plan source-bound Term-card jobs for one book | [S09 / #152](https://github.com/liudesheng9/zet/issues/152), [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | US059, US060, US061 | 7 |
| [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | AFK | Check, independently review, and fix Term drafts once | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | US062, US063, US064 | 7 |
| [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | AFK | Manage Term writers and reviewers within the shared DSMP pool | [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | US065, US066, US067 | 8 |
| [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | AFK | Publish a ready ordered prefix of Term Cards with receipts | [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | US068, US069, US070, US071 | 9 |
| [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | AFK | Plan exact text-preserving Term links deterministically | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | US072, US073, US074, US075, US076 | 9 |
| [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | AFK | Apply guarded Term-link plans and recover interrupted chunks | [S06 / #148](https://github.com/liudesheng9/zet/issues/148), [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | US077, US078, US079, US080 | 8 |
| [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | AFK | Run parallel Term stages with truthful full-mode dependencies | [S20 / #164](https://github.com/liudesheng9/zet/issues/164), [S21 / #165](https://github.com/liudesheng9/zet/issues/165), [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | US081, US082, US083 | 7 |
| [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | AFK | Backfill selected books through dedicated Term modes | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | US084, US085, US086 | 7 |
| [S26 / #169](https://github.com/liudesheng9/zet/issues/169) | AFK | Relink finalized Cards and audit complete term coverage | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | US087, US088, US089 | 6 |
| [S27 / #166](https://github.com/liudesheng9/zet/issues/166) | AFK | Make all writers self-check and recover Card-check refusals | [S06 / #148](https://github.com/liudesheng9/zet/issues/148), [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | US090, US091, US092 | 6 |
| [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | AFK | Publish compatible frozen workflows and agent guidance | [S14 / #161](https://github.com/liudesheng9/zet/issues/161), [S15 / #157](https://github.com/liudesheng9/zet/issues/157), [S16 / #158](https://github.com/liudesheng9/zet/issues/158), [S17 / #155](https://github.com/liudesheng9/zet/issues/155), [S25 / #168](https://github.com/liudesheng9/zet/issues/168), [S26 / #169](https://github.com/liudesheng9/zet/issues/169), [S27 / #166](https://github.com/liudesheng9/zet/issues/166) | US093, US094, US095 | 7 |
| [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | AFK | Qualify integrated behavior and bounded performance on copies | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | US096, US097, US098 | 9 |
| [S30 / #172](https://github.com/liudesheng9/zet/issues/172) | HITL | Review explanatory depth and Term-link usability on a copied book | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | US099, US100 | 7 |
| [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | HITL | Qualify live release and promote gate policy deliberately | [S30 / #172](https://github.com/liudesheng9/zet/issues/172) | US101, US102, US103 | 7 |
| [S32 / #174](https://github.com/liudesheng9/zet/issues/174) | HITL | Apply one owner-selected historical Term backfill | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | US104, US105 | 7 |

## Test and scope decisions

Use the current CLI/core, real GUI/API/browser, stdio MCP, shell/Session/PTY, SQLite/migration/backstop and dump seams, plus the existing workflow stand-in/frozen-install harness. Verify stored effects, rollback, projections, hashes, revisions, receipts and cleanup. Live provider/owner content/installed-release claims require their own evidence.

Deferred work remains explicit in the PRD: cross-book deterministic Term links, register/idea-grow or exact-name entry-tier integration, symbol Cards/links, synonym/prose rewriting, automatic historical cleanup or theorem proving.

## Source requirement ownership

The source line ranges below bind the unchanged design to one primary issue or the PRD. Full embedded requirement text and its hash are in the coverage audit.

| Requirement | Design section | Source lines | Owner | Contract preview |
|---|---|---|---|---|
| R0001 | 0 | 3–3 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Status: implementation design, 2026-10-24. This document proposes changes; it does not report an implemen… |
| R0002 | 0 | 5–5 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Inputs: Term Cards and Term Links proposal and zt-check gate proposal. The implementation baseline inspec… |
| R0003 | 0 | 7–7 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Navigation: Architecture · Design decisions · Shared checker · Policy and ratchet · Term storage · Term-c… |
| R0004 | 1 | 11–11 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Keep the three existing executables, the existing archive, and the existing digest ledger. Turn the exist… |
| R0005 | 1 | 13–13 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | A Term list, a Term Card, and a Term part all have type term. Their role comes from their term name and L… |
| R0006 | 1 | 15–15 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | The service checks a proposed Card against the same rule implementation that zt-check uses. The service o… |
| R0007 | 1 | 17–17 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | The new digest work consists of: |
| R0008 | 1 | 19–19 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | zt-digest term-cards: plan source-bound jobs, accept DeepSeek JSON, check and review it, render Cards, th… |
| R0009 | 1 | 20–20 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | zt-digest term-link: compute byte-range substitutions in one book, show a plan, then apply guarded edits.… |
| R0010 | 1 | 22–22 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Do not create a terminology database, a gate process, a plugin framework, a new queue, a term-ownership t… |
| R0011 | 1.1 | 28–28 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the shared rule core — The existing rules, structured diagnostics, and Card checks without I/O — The serv… |
| R0012 | 1.1 | 29–29 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | shared text operations — Shared envelope and lexical Link parsing, visible-text projection, and term-tabl… |
| R0013 | 1.1 | 30–30 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | shared rule policy — Rule catalog, defaults, overrides, severity, and legacy-fault comparison — The gate … |
| R0014 | 1.1 | 31–31 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the service gate adapter — Load bounded transaction context and call the pure checker — Storage reads and… |
| R0015 | 1.1 | 32–32 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the term-structure module — Compute list/head/part roles and the term structure from Cards — Readers and … |
| R0016 | 1.1 | 33–33 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the Term-card compiler — The Term-card compiler — Generated explanations need checking, review, publicati… |
| R0017 | 1.1 | 34–34 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the Term-link command — The deterministic link pass — Linking needs a reusable plan/apply operation |
| R0018 | 1.1 | 35–35 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Two workflow modules — Wire these commands into the existing compiler and relay paths — The two new stage… |
| R0019 | 1.1 | 36–36 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Two prompt files — Writer instructions and independent mathematical review — Content creation and review … |
| R0020 | 1.1 | 38–38 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | core.rs, text.rs, and policy.rs are files inside the existing checker crate, not new crates. Preserve che… |
| R0021 | 1.1 | 40–40 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Production storage changes are one nullable cards.term_name column and the widened Card-type constraint. … |
| R0022 | 1.2 | 45–55 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Architecture/stage flow contract: ; inputs["GUI, Session, shell, MCP, apply_batch"] --> tx["Existing writ… |
| R0023 | 2 | 60–60 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | The following are source observations at the baseline, rather than assumptions taken from either proposal… |
| R0024 | 2 | 64–64 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the checker library and its Cargo manifest — The crate already exports check, graph, and mcp; process cod… |
| R0025 | 2 | 65–65 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the checker CLI/input adapter — Input loading, MCP calls, rules, text helpers, printed diagnostics, and p… |
| R0026 | 2 | 66–66 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the service and CLI module validate_card_text — Parses the envelope, checks a repeated leading title, and… |
| R0027 | 2 | 67–67 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | shared Card operations edit_tx — Resolves type/facet/level/form changes separately; a metadata-only edit … |
| R0028 | 2 | 68–68 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | placement operations and shell paths — Some writers call the existing validator directly — Audit all auth… |
| R0029 | 2 | 69–69 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the service and CLI module cards_table — SQLite restricts card_type with a CHECK listing five types — ALT… |
| R0030 | 2 | 70–70 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the service and CLI module ensure_finding_cards — Existing type expansion backs up and rebuilds cards whi… |
| R0031 | 2 | 71–71 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the Card envelope and fingerprint module — Parsing normalizes body line endings and outer blank lines; Re… |
| R0032 | 2 | 72–72 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the root Link facade/lifecycle — One scanner supplies occurrence and target byte ranges; it currently exc… |
| R0033 | 2 | 73–73 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the derived Link index — The derived occurrence table has source, ord, target, kind; normal writes refres… |
| R0034 | 2 | 74–74 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Model relations/projections read_links — Relation kinds are inferred from prose, with inheritance between… |
| R0035 | 2 | 75–75 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the batch.rs functional surface — Real batches accept 1–64 items and at most 1 MiB of authored input — A … |
| R0036 | 2 | 76–76 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the batch.rs functional surface — Preview sequences already exist, with up to 1,024 items and 8 MiB — Ext… |
| R0037 | 2 | 77–77 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | identified digest calls and guards — Identified calls already partition real writes by item/byte limits a… |
| R0038 | 2 | 78–78 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the Card envelope and fingerprint module and the Tree Lock protection module — Authored fingerprints, SQL… |
| R0039 | 2 | 79–79 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | identified digest calls and guards authored_fingerprint — Digest request guards use their own canonical J… |
| R0040 | 2 | 80–80 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | the service and CLI module Config — Existing configuration is a small set of declared process settings — … |
| R0041 | 2 | 81–81 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Digest modules and base blueprint — Existing compilers use Z.layer, Z.chain, relay commands, claims, rece… |
| R0042 | 2 | 83–83 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | The governing domain remains CONTEXT.md, with the block map in the developer wiki. The current wiki state… |
| R0043 | 3 | 87–87 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | These are recommended design defaults. They make the design implementable without requiring answers to th… |
| R0044 | 3 | 91–91 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Card type — Add term, as requested; do not simulate it with title conventions on Idea Cards |
| R0045 | 3 | 92–92 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Role storage — Store only term_name; compute list/head/part roles |
| R0046 | 3 | 93–93 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Placement — Immediate Side successors of the list for term heads; Direct successors for continuation part… |
| R0047 | 3 | 94–94 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Term title — The exact term name on a head; continuation titles use 推导（续 k/n）：… |
| R0048 | 3 | 95–95 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | List preservation — Retype only; preserve parsed title, authored body, row identity, address, and authore… |
| R0049 | 3 | 96–96 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Link syntax — [[label@address]] with a direct Regular Location target |
| R0050 | 3 | 97–97 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Link frequency — first per term per body; support all explicitly |
| R0051 | 3 | 98–98 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Link scope — One agent Literature tree for the digest pass |
| R0052 | 3 | 99–99 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Checker process — In-process library inside zt |
| R0053 | 3 | 100–100 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Self-check service path — Reuse apply_batch preview; do not add check_cards in the first release |
| R0054 | 3 | 101–101 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Checker context — A concrete read-only value supplied to pure rules, rather than a storage-calling trait … |
| R0055 | 3 | 102–102 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Policy — The proposal's human/agent defaults, with archive-specific overrides and a staged enforcement ro… |
| R0056 | 3 | 103–103 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Term generation — DeepSeek V4.1 Flash, official source, high effort; Haiku management through the existin… |
| R0057 | 3 | 104–104 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Mathematical review — Every math term is reviewed in a separate conversation; a changed draft invalidates… |
| R0058 | 3 | 105–105 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Backfill — One selected book at a time after the copy-based pilot; never automatically unlock a tree |
| R0059 | 3 | 106–106 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Optional later work — Entry-search ranking for exact term names and idea-grow/register integration remain… |
| R0060 | 3.1 | 110–110 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | **Bounded publication instead of one unbounded transaction.** Both new commands use existing receipt-back… |
| R0061 | 3.1 | 112–112 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | **Checked publication prefixes instead of arbitrary holes.** The proposal permits successful drafts to pu… |
| R0062 | 3.1 | 114–114 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | **Conditional prediction instead of unconditional offline acceptance.** An offline check without the arch… |
| R0063 | 3.1 | 116–116 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | **Stable faults instead of printed-line identity.** Use structured, unclipped diagnostic identity and mul… |
| R0064 | 3.1 | 118–118 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | **One shared lexical implementation.** Move reusable text/Link scanning into the existing checker crate's… |
| R0065 | 3.1 | 120–120 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | **A persisted auto-link choice.** link:false must survive loss of the staging directory. Store it as an a… |
| R0066 | 3.1 | 122–122 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | **Self-checks precede enforcement.** Ship the compatible CLI, preview support, writer instructions, and c… |
| R0067 | 4 | 126–126 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | These invariants define successful implementation. |
| R0068 | 4 | 128–128 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | A Card's authored Markdown is the source of truth for its explanation and Link occurrences. |
| R0069 | 4 | 129–129 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | A field or table computed from Cards is a projection, never an independent authority. |
| R0070 | 4 | 130–130 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Every authoring route checks the final proposed text and metadata inside the write transaction. |
| R0071 | 4 | 131–131 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Checking uses the proposed Workspace, type, facet, level, form, and term name, not the row's previous typ… |
| R0072 | 4 | 132–132 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | An agent can change only agent Cards. Gate policy cannot grant Access or unlock anything. |
| R0073 | 4 | 133–133 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | A Lock refusal stops the affected operation without retries through another route. |
| R0074 | 4 | 134–134 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Existing envelope, field, Link, Access, and Lock invariants remain mandatory. |
| R0075 | 4 | 135–135 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | A Term link reaches a term head directly; it does not resolve a name through a registry. |
| R0076 | 4 | 136–136 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | A Term use contributes no reasoning edge, Model membership, or Idea inbound/outbound credit. |
| R0077 | 4 | 137–137 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Removing recognized Term-link markup preserves exactly the visible authored body bytes. |
| R0078 | 4 | 138–138 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | A rerun with unchanged inputs makes no authored edits or new Card allocations. |
| R0079 | 4 | 139–139 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Preview changes no persisted Card, receipt, revision, allocation, or Lock history. |
| R0080 | 4 | 140–140 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | A refusal rolls back the actual atomic operation, including receipts and allocations. |
| R0081 | 4 | 141–141 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Unknown context, a stale contract, or an interrupted check cannot be reported as a pass. |
| R0082 | 4 | 142–142 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Mathematical format checks and model review are evidence of review, not proof of mathematical truth. |
| R0083 | 4 | 143–143 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | A run with missing terms or unresolved refusals remains visibly incomplete even when the audit and report… |
| R0084 | 5.1 | 149–149 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | Keep the existing checker crate independently buildable, with its current package and its own workspace.… |
| R0085 | 5.1 | 151–151 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | Recommended feature shape: |
| R0086 | 5.1 | 154–161 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | Cargo contract: pure core is always available, default cli is optional, process-platform dependencies are… |
| R0087 | 5.1 | 164–164 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | The platform dependencies remain target-specific and become optional where they belong to child-process c… |
| R0088 | 5.1 | 166–166 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | The root crate adds: |
| R0089 | 5.1 | 169–169 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | Cargo contract: the root links the existing checker crate as a path dependency with default features disa… |
| R0090 | 5.1 | 172–172 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | zt-digest keeps depending on the same crate with cli enabled. There is no need for an otherwise empty cor… |
| R0091 | 5.1 | 174–174 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | The library shape becomes: |
| R0092 | 5.1 | 177–186 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | Crate-layout contract: the existing checker package exports pure core/text/policy; CLI input adapter, who… |
| R0093 | 5.1 | 189–189 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | Move code, retaining fixtures and behavior, before changing rule policy. check::run_lines(args, Some(&mut… |
| R0094 | 5.2 | 193–193 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Use the existing JSON Card entry shape where it already serves the rules. Introduce only the small typed … |
| R0095 | 5.2 | 195–195 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Conceptually: |
| R0096 | 5.2 | 198–210 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Typed check contract: CheckInput carries card, before, context, policy and purpose; purpose distinguishes… |
| R0097 | 5.2 | 213–213 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | CardEntry includes the parsed envelope, address/kind/Workspace, type fields, and term-role facts. before … |
| R0098 | 5.2 | 215–215 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | CheckContext is an immutable value containing the small set of facts needed for the selected rules: |
| R0099 | 5.2 | 217–217 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Parent and relevant ancestors, with the old and proposed versions where required. |
| R0100 | 5.2 | 218–218 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Referenced target identities and types. |
| R0101 | 5.2 | 219–219 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Book/root identity and applicable Book term table. |
| R0102 | 5.2 | 220–220 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Formal model setting/level/facet/shared membership facts for the checked Card. |
| R0103 | 5.2 | 221–221 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Term list row and head/part ownership where applicable. |
| R0104 | 5.2 | 222–222 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | The availability/completeness of each context family. |
| R0105 | 5.2 | 224–224 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | The service materializes this value using its transaction connection. The CLI materializes the same value… |
| R0106 | 5.2 | 226–226 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Distinguish absent from unavailable. For example, an Idea Card may legitimately have unknown source evide… |
| R0107 | 5.3 | 230–230 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | For a write, load the proposed Card, required ancestry/setting, applicable term list, and distinct refere… |
| R0108 | 5.3 | 232–232 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Limits must be explicit and shared in the check contract. Start with existing batch and authored-input bo… |
| R0109 | 5.3 | 234–234 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Graph rules remain an explicit audit path. Large context is not permission to quietly run I6 against ever… |
| R0110 | 5.4 | 238–238 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | A diagnostic needs at least: |
| R0111 | 5.4 | 241–250 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Structured public contract includes these fields and their stated example semantics: address, rule, fault… |
| R0112 | 5.4 | 253–253 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Keep rule ids and current message wording unless a rule's semantics changes. Format the familiar line at … |
| R0113 | 5.4 | 256–258 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | K/14 E2 missing labels: 参与者、来源 K/14 R20 (advisory) ... faults: 1 |
| R0114 | 5.4 | 261–261 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Formatting collapses whitespace and clips as today. Identity does not. A fault's key is a rule-specific s… |
| R0115 | 5.4 | 263–263 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Preserve the current rule traversal order in the first extraction. Gate and CLI reports use the same orde… |
| R0116 | 6.1 | 269–269 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | The checker owns the rule catalog and default policy. The service owns the selected archive's effective o… |
| R0117 | 6.1 | 271–271 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | Use metadata keys such as gate.agent.R2 and gate.human.E2. Extend zt config set for the declared gate.<wo… |
| R0118 | 6.1 | 273–273 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | Provide zt config output that names effective overrides and their archive. Missing override means the def… |
| R0119 | 6.1 | 275–275 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | Validate rule ids, Workspace names, and values. Reject typos, unsupported rules, and attempts to override… |
| R0120 | 6.1 | 277–277 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | A policy change receives a new policy hash and advances the revision used by check-contract readers. It d… |
| R0121 | 6.2 | 283–283 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | Envelope, valid fields, address/Link grammar, target validation, Access, Locks — mandatory — mandatory — … |
| R0122 | 6.2 | 284–284 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | E1, E2, F1, F2, F3, R18, R19, R22 — block — block — Local/bounded-context format |
| R0123 | 6.2 | 285–285 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | R13–R17 on formal models — advise — block — Bounded model context; concept models retain their exemption |
| R0124 | 6.2 | 286–286 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | R2–R5, R7, R9, R12, I2–I4, I7, L1, L2 — advise — block — Only where the existing rule applies |
| R0125 | 6.2 | 287–287 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | I1, R20, R21, I7 unknown-source — advise — advise — Advisory subcases remain advisory |
| R0126 | 6.2 | 288–288 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | R6 — off — advise — Preserve the existing meaning-sensitive remediation decision |
| R0127 | 6.2 | 289–289 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | T1, T2, T3, T5, T6, T9, T10 — block — block — Term-format requirements, with role-specific applicability |
| R0128 | 6.2 | 290–290 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | T4 — advise — advise — Approximate LaTeX symbol coverage |
| R0129 | 6.2 | 291–291 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | T7 wrong/nonexistent target on a newly introduced Term link — mandatory — mandatory — Existing-style Link… |
| R0130 | 6.2 | 292–292 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | T7 label mismatch, retained invalid target — advise — advise — Report legacy/stale uses without rewriting… |
| R0131 | 6.2 | 293–293 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | T8 and computed term-structure faults — audit advice — audit advice — Coverage/structure can be incomplet… |
| R0132 | 6.2 | 294–294 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | I5, I6, E4, E5, F4, L3 and other graph facts — audit advice — audit advice — Not a write gate |
| R0133 | 6.2 | 296–296 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | T10 is a term-facing label for the same open-formula predicate as R22; do not count the identical fault t… |
| R0134 | 6.2 | 298–298 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | The policy applies to rules that exist mechanically. It does not turn human-language instructions such as… |
| R0135 | 6.2 | 300–300 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | A term list is navigation, so it does not inherit Idea size/title/link-count rules. Term heads and parts … |
| R0136 | 6.3 | 304–304 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | New Cards have no legacy allowance. All configured blocking diagnostics can block them. |
| R0137 | 6.3 | 306–306 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | For an existing Card: |
| R0138 | 6.3 | 308–308 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Check the old authored version using its old metadata and the same current rule version/policy snapshot. |
| R0139 | 6.3 | 309–309 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Check the final proposed version using proposed metadata and transaction context. |
| R0140 | 6.3 | 310–310 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | For comparable rules, build a multiset keyed by (rule, fault, key) from the old report. |
| R0141 | 6.3 | 311–311 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Consume one old occurrence for each matching new occurrence. |
| R0142 | 6.3 | 312–312 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | A matched diagnostic becomes legacy:true advice. An unmatched diagnostic keeps its configured severity. |
| R0143 | 6.3 | 313–313 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Refuse only if an unmatched blocking diagnostic remains. |
| R0144 | 6.3 | 315–315 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Do not compare clipped lines or merely sets. Adding a second copy of an already faulty sentence is a new … |
| R0145 | 6.3 | 317–317 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | A size fault's key includes its measured value and limit, so making an oversized Card larger is not grand… |
| R0146 | 6.3 | 319–319 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | On a type/role/level change, compare only rules whose applicability and meaning remain the same. A Card r… |
| R0147 | 6.3 | 321–321 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | The ratchet covers new quality rules, not hard storage invariants. It cannot legalize malformed Link synt… |
| R0148 | 6.3 | 323–323 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Keep the existing repeated-leading-title allowance tied to authored_changed while moving the envelope imp… |
| R0149 | 6.3 | 325–325 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Pure address changes and generated Reverse-section refreshes do not run prose rules. A real authored-text… |
| R0150 | 6.4 | 329–329 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Compute old and new diagnostics during the operation. This automatically applies a new checker version to… |
| R0151 | 6.4 | 331–331 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | Policy activation therefore preserves editability of existing Cards but does not retroactively clean them… |
| R0152 | 7.1 | 337–337 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Retain validate_card_text as a narrow wrapper where useful, but make gate::check_proposed the shared deci… |
| R0153 | 7.1 | 339–339 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Its input is a resolved candidate, not just (address, text). The candidate contains final text, type fiel… |
| R0154 | 7.1 | 341–341 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Recommended flow: |
| R0155 | 7.1 | 344–353 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Ordered write contract: existing edit lock/transaction; resolve candidate and prior version; Access/Lock … |
| R0156 | 7.1 | 356–356 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Where the existing transaction temporarily materializes an item for structural validation, keep that work… |
| R0157 | 7.1 | 358–358 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Check retypes, facet/level/form/term-name changes, text edits, new successor/insert Cards, and root creat… |
| R0158 | 7.1 | 360–360 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Do not rerun prose checking solely because a move rewrites an address in a Link. The shared visible-text … |
| R0159 | 7.2 | 366–366 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | ops::create_topic/create_literature/create_successor — Check final new Card before commit |
| R0160 | 7.2 | 367–367 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | ops::edit_tx — Check one combined candidate for text plus metadata changes |
| R0161 | 7.2 | 368–368 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | ops::insert_card and place::apply_insert_plan — Check inserted Card in the simulated placement |
| R0162 | 7.2 | 369–369 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Direct shell writers in main.rs — Reach the shared gate with correct owner-selected Workspace |
| R0163 | 7.2 | 370–370 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Session create/edit/type commands — Reach the same gate and print/display returned warnings |
| R0164 | 7.2 | 371–371 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | GUI create/edit/metadata handlers — Return structured warnings and retain editor text after a refusal |
| R0165 | 7.2 | 372–372 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | MCP create/insert/edit — Preserve existing answer fields and add check information |
| R0166 | 7.2 | 373–373 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | apply_batch — Use the same per-item path inside its transaction |
| R0167 | 7.2 | 374–374 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Dump import/load, if it is an authoring route — Validate new authored input; any trusted legacy restore p… |
| R0168 | 7.2 | 376–376 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Read the actual call graph during implementation. This table is an acceptance checklist, not a claim that… |
| R0169 | 7.3 | 380–380 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Extend the normal operation result with a small check report. Do not use global/thread-local warning buff… |
| R0170 | 7.3 | 382–382 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | A successful JSON reply keeps its existing fields and adds, when relevant: |
| R0171 | 7.3 | 385–395 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Structured public contract includes these fields and their stated example semantics: address, warnings, w… |
| R0172 | 7.3 | 398–398 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | A blocking refusal uses the existing typed-error transport pattern: |
| R0173 | 7.3 | 401–409 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | Structured public contract includes these fields and their stated example semantics: error, address, bloc… |
| R0174 | 7.3 | 412–412 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | diagnostics contains the structured faults; the empty array above is only an abbreviated example. The hum… |
| R0175 | 7.3 | 414–414 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | A batch refusal additionally identifies the zero-based item index, tool, and operation key. Preserve the … |
| R0176 | 7.3 | 416–416 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | GUI warnings are a compact group beside the existing save result. They do not open a separate approval fl… |
| R0177 | 7.4 | 420–420 | [S05 / #147](https://github.com/liudesheng9/zet/issues/147) | A real batch checks items in order. Later items see earlier successful candidate effects in the transacti… |
| R0178 | 7.4 | 422–422 | [S05 / #147](https://github.com/liudesheng9/zet/issues/147) | One blocking fault rolls back the entire real batch. Earlier committed chunks of a multi-batch digest rem… |
| R0179 | 7.4 | 424–424 | [S05 / #147](https://github.com/liudesheng9/zet/issues/147) | Extend the existing preview/savepoint implementation to treat card_check like a typed per-item refusal it… |
| R0180 | 7.4 | 426–426 | [S05 / #147](https://github.com/liudesheng9/zet/issues/147) | Preview returns diagnostics, warnings, candidate Cards, and a complete/checked/refused/blocked summary. E… |
| R0181 | 7.4 | 428–428 | [S05 / #147](https://github.com/liudesheng9/zet/issues/147) | Do not raise the real batch cap. The existing larger preview sequence can inspect a planned series, but i… |
| R0182 | 8.1 | 434–434 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Retain --json, --model, --addresses, and --cards. Retain run_lines for zt-digest. Add: |
| R0183 | 8.1 | 436–436 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | --workspace human\|agent, default agent for drafts. |
| R0184 | 8.1 | 437–437 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | --draft <file> for exact proposed writes, including their type fields and optional prior state/context. |
| R0185 | 8.1 | 438–438 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | A structured-output option for check status and contract information, while retaining ordinary fault line… |
| R0186 | 8.1 | 440–440 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Live checks use each stored Card's actual Workspace; a command-line flag cannot reclassify it. Reject a c… |
| R0187 | 8.1 | 442–442 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | At the coordinated release, faults: N counts blocking diagnostics after policy/ratchet in every mode. Adv… |
| R0188 | 8.1 | 444–444 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Recommended CLI process exit contract: |
| R0189 | 8.1 | 448–448 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | 0 — Complete evaluation with no blocking faults |
| R0190 | 8.1 | 449–449 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | 1 — Complete evaluation with blocking faults |
| R0191 | 8.1 | 450–450 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | 2 — Invalid invocation, missing required context, stale/incompatible check contract, or infrastructure fa… |
| R0192 | 8.1 | 452–452 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | For the digest commands, keep their existing operational conventions; a card_locked result stops the comm… |
| R0193 | 8.2 | 456–456 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | A draft bundle carries the exact candidate plus optional context: |
| R0194 | 8.2 | 459–476 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Structured public contract includes these fields and their stated example semantics: workspace, rule_vers… |
| R0195 | 8.2 | 479–479 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Do not invent a complete target address for an offline successor whose allocation is not known. A provisi… |
| R0196 | 8.2 | 481–481 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | A bundle without archive policy explicitly uses built-in defaults and says so. It can demonstrate local c… |
| R0197 | 8.3 | 485–485 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Use a stable rule version, independent of unrelated executable version bumps. Include rule semantics, vis… |
| R0198 | 8.3 | 487–487 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Expose zt_check_version in MCP initialization as requested. Because a stdio proxy and the daemon can be d… |
| R0199 | 8.3 | 489–489 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Extend the existing verified get_cards view, including the empty-address metadata read used by the digest… |
| R0200 | 8.3 | 491–491 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Rule version and diagnostic schema version. |
| R0201 | 8.3 | 492–492 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Effective policy and policy hash. |
| R0202 | 8.3 | 493–493 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Limits and applicability version. |
| R0203 | 8.3 | 494–494 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Archive identity/revision. |
| R0204 | 8.3 | 495–495 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | Relevant syntax capabilities, including term_links and term_cards. |
| R0205 | 8.3 | 497–497 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | No extra check_cards tool or policy endpoint is needed. Existing write replies and previews carry the ver… |
| R0206 | 8.3 | 499–499 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | A live CLI with a rule-version mismatch prints the mismatch and does not return a successful acceptance p… |
| R0207 | 9.1 | 505–505 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Add CardType::Term and cards.term_name TEXT NULL. |
| R0208 | 9.1 | 507–507 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Normalize a supplied term name by trimming its outer whitespace. An absent/null field denotes a list. Emp… |
| R0209 | 9.1 | 509–509 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | term_name is valid only on a Regular Card of type term. A type change away clears it. An explicit non-nul… |
| R0210 | 9.1 | 511–511 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Use the existing partial-field convention: |
| R0211 | 9.1 | 513–513 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Field omitted: preserve the term name on an edit. |
| R0212 | 9.1 | 514–514 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | term:null: clear it, producing a candidate list role. |
| R0213 | 9.1 | 515–515 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | term:"...": set it. |
| R0214 | 9.1 | 517–517 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Creation and type-change validation must check the entire resulting combination. Existing model/hub/math … |
| R0215 | 9.2 | 522–527 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | Role contract: non-term has no role; term with no name is list; named immediate Side child of list is hea… |
| R0216 | 9.2 | 530–530 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | A role is not a stored discriminator. Compute the ancestor chain with cycle/depth protection appropriate … |
| R0217 | 9.2 | 532–532 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | Preserve the faults named by the proposal: |
| R0218 | 9.2 | 536–536 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | term_unfiled — Named Term Card has no valid list/head/part placement |
| R0219 | 9.2 | 537–537 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | term_duplicate — More than one head for a term under one list |
| R0220 | 9.2 | 538–538 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | term_row_without_card — A row has no corresponding head |
| R0221 | 9.2 | 539–539 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | term_card_without_row — A head names no row |
| R0222 | 9.2 | 540–540 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | term_part_name — A continuation's name differs from its preceding head/part |
| R0223 | 9.2 | 541–541 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | term_label_mismatch — An authored label differs from its target's name |
| R0224 | 9.2 | 542–542 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | term_link_target — A target is missing or is not a head |
| R0225 | 9.2 | 544–544 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | Coverage and placement faults are reported, rather than universally blocking hand editing. Newly introduc… |
| R0226 | 9.2 | 546–546 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | If a book has multiple plausible Term lists, do not select the first by address. get_terms {at} can name … |
| R0227 | 9.3 | 550–550 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | Move the existing public four-column term_table implementation into the shared text module. Preserve its … |
| R0228 | 9.3 | 552–552 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | For publication, also inspect raw parsed rows before compatibility deduplication. Duplicate exact or case… |
| R0229 | 9.3 | 554–554 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | The term name stored on the head is exactly the selected table-row spelling. No Unicode normalization, ca… |
| R0230 | 9.4 | 558–558 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | Keep category in the authored 类别： line, not another column. Add one optional recognized head line: |
| R0231 | 9.4 | 561–562 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | Authored opt-out contract: 自动链接：否 and 自动链接理由：... persist the disabled choice and required reason; absence… |
| R0232 | 9.4 | 565–565 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | Absence means true. A link:false draft must carry a reason and renders these lines. This is the only temp… |
| R0233 | 9.4 | 567–567 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | The list's four-column table remains unchanged. Do not put a fifth column, links, hidden comments, or aut… |
| R0234 | 9.5 | 571–571 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | The migration must: |
| R0235 | 9.5 | 573–573 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Make a consistent SQLite backup using the existing migration procedure. |
| R0236 | 9.5 | 574–574 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Rebuild the Card table with term in its CHECK and term_name in the column set. |
| R0237 | 9.5 | 575–575 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Copy every existing row id, identity, text, Workspace, and type field unchanged; initialize the new field… |
| R0238 | 9.5 | 576–576 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Recreate the current indexes and Card-related Lock triggers inside the migration transaction. |
| R0239 | 9.5 | 577–577 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Preserve Lock members, pins, events, receipts, and existing revision semantics. |
| R0240 | 9.5 | 578–578 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Verify row counts, identities, active Lock integrity, and dump/read compatibility before enabling new wri… |
| R0241 | 9.5 | 580–580 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Update the shared Card load/select/insert paths, field snapshots, dump manifest, check inputs, and receip… |
| R0242 | 9.5 | 582–582 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | The rebuilt table should also enforce term_name IS NULL OR card_type='term', with the Regular-Card restri… |
| R0243 | 9.5 | 584–584 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | term_name is authored identity. Include it in fingerprints and frozen-field checks. Preserve legacy finge… |
| R0244 | 9.5 | 586–586 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Update the SQL fingerprint wrapper and all its callers consistently. Keep an eight-argument compatibility… |
| R0245 | 9.5 | 588–588 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | The service's canonical receipt projection and zt-digest's canonical request guard must both include none… |
| R0246 | 9.5 | 590–590 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | Do not rewrite historical Lock events or reseal trees automatically. A migration with unchanged old Cards… |
| R0247 | 9.5 | 592–592 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | The Lock protection generation must change because its canonical protected-field trigger changes. At this… |
| R0248 | 9.5 | 594–594 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | This existing generation check also supplies a concrete compatibility floor: test that the baseline binar… |
| R0249 | 10.1 | 600–600 | [S10 / #151](https://github.com/liudesheng9/zet/issues/151) | The shared scanner exposes: |
| R0250 | 10.1 | 603–608 | [S10 / #151](https://github.com/liudesheng9/zet/issues/151) | Lexical contract: macro_range is entire macro, target_range is address-only, label_range/label exist for … |
| R0251 | 10.1 | 611–611 | [S10 / #151](https://github.com/liudesheng9/zet/issues/151) | All offsets are UTF-8 byte offsets into the original input. Keep the existing target-only rewrite behavio… |
| R0252 | 10.1 | 613–613 | [S10 / #151](https://github.com/liudesheng9/zet/issues/151) | For a Term link, split at the first @. A second @ violates the label/target grammar. A target must be a R… |
| R0253 | 10.1 | 615–615 | [S10 / #151](https://github.com/liudesheng9/zet/issues/151) | The shared lexical parser owns macro shape, label limits, code-span/fence exclusions, and tolerant render… |
| R0254 | 10.1 | 617–617 | [S10 / #151](https://github.com/liudesheng9/zet/issues/151) | Preserve ordinary Link behavior during extraction. Add explicit tests for Term-like examples inside inlin… |
| R0255 | 10.2 | 621–621 | [S16 / #158](https://github.com/liudesheng9/zet/issues/158) | A Term occurrence produces kind:"term", regardless of a preceding relation word. Do not add term to the v… |
| R0256 | 10.2 | 623–623 | [S16 / #158](https://github.com/liudesheng9/zet/issues/158) | In model::read_links: |
| R0257 | 10.2 | 625–625 | [S16 / #158](https://github.com/liudesheng9/zet/issues/158) | A Term link receives term directly. |
| R0258 | 10.2 | 626–626 | [S16 / #158](https://github.com/liudesheng9/zet/issues/158) | It never sets the previous ordinary Link's inherited kind. |
| R0259 | 10.2 | 627–627 | [S16 / #158](https://github.com/liudesheng9/zet/issues/158) | If a preceding ordinary Link in the same sentence is still the relevant prior ordinary Link, keep that or… |
| R0260 | 10.2 | 628–628 | [S16 / #158](https://github.com/liudesheng9/zet/issues/158) | A subsequent ordinary Link cannot inherit term. |
| R0261 | 10.2 | 630–630 | [S16 / #158](https://github.com/liudesheng9/zet/issues/158) | Filter term from Model membership/relations/neighbors and Idea/Event/Finding reasoning counts. Do this at… |
| R0262 | 10.3 | 634–634 | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | Retain the existing links table. A Term occurrence gets its target address and kind='term'. The label rem… |
| R0263 | 10.3 | 636–636 | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | Update indexed Reverse-section generation and the full lifecycle oracle together. Both must split inbound… |
| R0264 | 10.3 | 639–646 | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | Generated-tail contract: existing Reverse marker, ordinary Reverse links section, separate Term uses sect… |
| R0265 | 10.3 | 649–649 | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | Emit only nonempty sections, preserving ordinary formatting for Cards with no Term uses. Deduplicate by s… |
| R0266 | 10.3 | 651–651 | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | Missing targets are broken, with the term label included in lsbk and MCP diagnostics. A target retyped or… |
| R0267 | 10.3 | 653–653 | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | An existing invalid Term link may be retained under the specified legacy rule, but a new one is refused. … |
| R0268 | 10.3 | 655–655 | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | Move/delete/rename plans, Access rewrite checks, Lock pins, and broken-Link handling see all Term occurre… |
| R0269 | 10.4 | 659–659 | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | Implement one shared function that replaces valid Term-link macros with their labels, leaving ordinary ma… |
| R0270 | 10.4 | 661–661 | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | Language/size checking projects Term labels first, then removes ordinary Link markup as it already does. |
| R0271 | 10.4 | 662–662 | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | R6 reads the visible term, not the address. |
| R0272 | 10.4 | 663–663 | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | The Literature scope checks actual targets, never a label@target concatenation. |
| R0273 | 10.4 | 664–664 | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | Optional read mode term_links:"label" projects body/text for read-only prompt consumption. |
| R0274 | 10.4 | 665–665 | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | Raw reads and exports keep the authored macros. |
| R0275 | 10.4 | 667–667 | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | Projected reads should carry term_links:"label" or an equivalent projection marker in the response. Never… |
| R0276 | 10.4 | 669–669 | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | This projection also needs review in statement-comparison consumers: adding Term markup should not falsel… |
| R0277 | 11.1 | 675–675 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | Render from the proposal's draft JSON. DeepSeek supplies content; the compiler owns section spelling and … |
| R0278 | 11.1 | 677–677 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | Common sections and lines: |
| R0279 | 11.1 | 680–695 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | Head-template contract: exact term heading; 术语/原文/不用/类别/首次定义 labels, optional 自动链接：否 and reason; 释义/书中的说法… |
| R0280 | 11.1 | 698–698 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | For a mathematical head, retain hypotheses, tagged definition, symbol table with domains, equivalent form… |
| R0281 | 11.1 | 700–700 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | The compiler must not invent missing scientific content to fill a required section. A writer must explici… |
| R0282 | 11.2 | 704–704 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | Use the same draft schema as the proposal. Partition the rendered derivation/content blocks when needed; … |
| R0283 | 11.2 | 706–706 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | A head carries the common identity, plain explanation, book definition, hypotheses, formal definition, an… |
| R0284 | 11.2 | 708–708 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | A part carries the same stored term name, a continuation title, category/source context, and its actual c… |
| R0285 | 11.2 | 710–710 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | Exact title = term is required on heads, not continuation titles. |
| R0286 | 11.2 | 711–711 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | A math head must establish its mathematical definition; a part can inherit it through its valid chain. |
| R0287 | 11.2 | 712–712 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | T5/T6/T9/T10 apply to the material actually held on each part. |
| R0288 | 11.2 | 713–713 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | The complete term chain must have the required mathematical content. |
| R0289 | 11.2 | 715–715 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | Use a deterministic greedy partition at whole-step/block boundaries, with at most 12 derivation steps and… |
| R0290 | 11.2 | 717–717 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | If the head's mandatory material alone is too large, return a clear split/rewrite fault. The writer can r… |
| R0291 | 11.3 | 723–723 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | T1 — Known ordered labels/sections, required common fields, correct role-specific template — Draft render… |
| R0292 | 11.3 | 724–724 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | T2 — Head title/name/术语：/row spelling agree; part title uses continuation format and retains the same ter… |
| R0293 | 11.3 | 725–725 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | T3 — Math head/chain has hypotheses, displayed definition, symbol table and derivation; concept terms do … |
| R0294 | 11.3 | 726–726 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | T4 — Approximate formula identifiers are covered by the head/chain symbol table — Advice |
| R0295 | 11.3 | 727–727 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | T5 — Numbered steps, a basis per step, a formula or explicit result, step bound, final result line — Loca… |
| R0296 | 11.3 | 728–728 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | T6 — No unexpanded “obvious”/skipped-step phrases; a named previous-step reference is allowed — Local heu… |
| R0297 | 11.3 | 729–729 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | T7 — Syntax, target role and label consistency — Mandatory new-target validation plus advice/audit |
| R0298 | 11.3 | 730–730 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | T8 — Exactly one head per table row, no extra heads — Book audit |
| R0299 | 11.3 | 731–731 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | T9 — Per-Card prose limit, excluding math and tables — Local |
| R0300 | 11.3 | 732–732 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | T10 — Balanced math delimiters through the same predicate as R22 — Local |
| R0301 | 11.3 | 734–734 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | T3/T5 cannot establish that all hypotheses are sufficient or that a result equals a mathematical claim. T… |
| R0302 | 11.3 | 736–736 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | Apply the same T rules to stored manually edited Cards and JSON-rendered drafts wherever inputs are avail… |
| R0303 | 12.1 | 742–742 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | Expose the steps from the proposal: |
| R0304 | 12.1 | 745–749 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | Command contract: zt-digest term-cards -Step plan\|jobs\|check\|review-plan\|fix-plan\|apply\|report -Staging <… |
| R0305 | 12.1 | 752–752 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | Use the existing argument parser, canonical JSON utilities, ledger, source manifests, operation prefixes,… |
| R0306 | 12.1 | 754–754 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | Keep a small fixed set of files under the per-run term-cards: |
| R0307 | 12.1 | 758–758 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | plan.json — Frozen rows, list/source fingerprints, jobs, limits, and compiler/rule version |
| R0308 | 12.1 | 759–759 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | jobs/ — Per-job source-bound inputs |
| R0309 | 12.1 | 760–760 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | out/ — Writer JSON, one stable output per writer job |
| R0310 | 12.1 | 761–761 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | check/ and review/ — Diagnostics and hash-bound review answers |
| R0311 | 12.1 | 762–762 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | state.json — Derived local progress and references to durable archive receipts |
| R0312 | 12.1 | 763–763 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | report.json and report.md — Row coverage, created addresses, refused/pending rows, review evidence |
| R0313 | 12.1 | 765–765 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | The existing run ledger remains the scheduler. state.json is the compiler's recoverable artifact, not a s… |
| R0314 | 12.2 | 769–769 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | Read the list by an explicit/unambiguous role, with a legacy table adapter for a pre-term book. Preserve … |
| R0315 | 12.2 | 771–771 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | For each row gather the exact name, avoid words, short meaning, first-definition locator, source file/lin… |
| R0316 | 12.2 | 773–773 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | Classify as math when the definition/snippet, applicable notation table, or known setting gives formal no… |
| R0317 | 12.2 | 775–775 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | Group math terms one per job and concept terms four per job by default. Keep job ids stable by frozen row… |
| R0318 | 12.3 | 779–779 | [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | Use the existing Z.layer/Z.chain and job continue/fallback fields. Do not implement the proposal sketch's… |
| R0319 | 12.3 | 781–781 | [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | The chain is: |
| R0320 | 12.3 | 784–790 | [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | Readiness flow: write JSON; schema/render check; independent math review; one writer fix when needed; sch… |
| R0321 | 12.3 | 793–793 | [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | A review names the term, source fingerprint, and exact draft/rendered-content hash. A previous {"ok":true… |
| R0322 | 12.3 | 795–795 | [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | A concept term needs schema/format checks but no mathematical reviewer. A changed schema or compiler rend… |
| R0323 | 12.3 | 797–797 | [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | Do not ask a reviewer merely to endorse a formula. Its job includes all the proposal's depth requirements… |
| R0324 | 12.4 | 801–801 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | Retain the existing managed path: |
| R0325 | 12.4 | 803–803 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | Haiku managers, low effort, manage jobs and relay commands; they do not write Card prose or edit JSON on … |
| R0326 | 12.4 | 804–804 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | DeepSeek V4.1 Flash from the official source, high effort, writes/reviews/fixes content. |
| R0327 | 12.4 | 805–805 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | At most eight conversations under one manager, and at most eight managers alive across the whole run. |
| R0328 | 12.4 | 806–806 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | Writers, reviewers, and continuations share the global pool with the digest branch. |
| R0329 | 12.4 | 807–807 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | Stable job ids and registered conversation/resource ids support cleanup and recovery. |
| R0330 | 12.4 | 808–808 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | Collect answers promptly; retain a writer only through its permitted review/fix opportunity, then delete … |
| R0331 | 12.4 | 809–809 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | Remove the DeepSeek workspace registration once its work and required continuations are finished. Preserv… |
| R0332 | 12.4 | 810–810 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | Use the existing bounded retry/runtime budget. A provider failure gets the permitted single retry; a sema… |
| R0333 | 12.4 | 811–811 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | The writer receives the applicable Chinese language skill through the existing job skills field, rather t… |
| R0334 | 12.4 | 813–813 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | Before parallelizing the two branches, verify the implementation holds global slots across a manager's ac… |
| R0335 | 12.4 | 815–815 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | Count retained writer conversations as live, even while idle. A manager cannot retain eight writers and t… |
| R0336 | 12.5 | 819–819 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | For a new book, terms -Step apply creates the list as type term with no name. For a legacy book, the firs… |
| R0337 | 12.5 | 821–821 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | Publish the ready row prefix in order. For each row publish its head and complete continuation chain toge… |
| R0338 | 12.5 | 823–823 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | The first batch may also retype the list. Allocate Side children through create_successor and parts throu… |
| R0339 | 12.5 | 825–825 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | Before publication verify the expected list and existing receipt-owned prefix. If unrelated Side children… |
| R0340 | 12.5 | 827–827 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | Example: rows 1–6 are ready, row 7 failed, rows 8–20 are ready. Publish 1–6; keep 8–20 staged. A repaired… |
| R0341 | 12.5 | 829–829 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | Use stable operation keys tied to run/unit/plan revision/step/row. On a transport interruption, query/rep… |
| R0342 | 12.5 | 831–831 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | A source/list/draft change creates a new plan revision. Reconcile already published receipt-owned Cards a… |
| R0343 | 12.5 | 833–833 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | A book unit is done only when all its required rows and parts are verified. A partial prefix is useful co… |
| R0344 | 12.6 | 837–837 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | Report, per row: |
| R0345 | 12.6 | 839–839 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | Source row number and exact term. |
| R0346 | 12.6 | 840–840 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | Math/concept classification and whether promoted. |
| R0347 | 12.6 | 841–841 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | Writer job, draft hash, check result, review hash/result, and fix disposition. |
| R0348 | 12.6 | 842–842 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | Ready but not yet published, committed, refused, or conflicted status. |
| R0349 | 12.6 | 843–843 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | Head/part addresses and receipt keys. |
| R0350 | 12.6 | 844–844 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | Why publication stopped and which later ready drafts remain staged. |
| R0351 | 12.6 | 846–846 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | For an incomplete stage the workflow continues to its read-only audit/report, but the feature is degraded… |
| R0352 | 13.1 | 853–856 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Command contract: zt-digest term-link -Step plan\|apply -Staging <folder> [-CitationKey <key>] [-Occurrenc… |
| R0353 | 13.1 | 859–859 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | The default scope is the selected agent Literature tree. -Cards must be an exact subset of that tree. Rej… |
| R0354 | 13.1 | 861–861 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Exclude the Literature root, the Term list, the book's structure/index/navigation Cards, and the Book not… |
| R0355 | 13.1 | 863–863 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Include term heads and parts, with their own term protected. Their use of other terms can link normally. |
| R0356 | 13.2 | 867–867 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Read the tree once where practical, including raw bodies, metadata, list rows, current heads, and rule/po… |
| R0357 | 13.2 | 869–869 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | term-link/plan.json records: |
| R0358 | 13.2 | 871–871 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Archive identity, book key, compiler/syntax version, and occurrence policy. |
| R0359 | 13.2 | 872–872 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | List, term-head mapping, and auto-link-choice fingerprints. |
| R0360 | 13.2 | 873–873 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Input Card authored fingerprints and exact raw parsed body hashes. |
| R0361 | 13.2 | 874–874 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Chosen substitutions and relevant skip reasons at UTF-8 byte offsets. |
| R0362 | 13.2 | 875–875 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Output bodies or sufficient exact data to reproduce them. |
| R0363 | 13.2 | 876–876 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Counts, unresolved target/label faults, and a stable plan content hash. |
| R0364 | 13.2 | 878–878 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | preview.md shows changed prose lines and totals. Reports can include timestamps, but the canonical transf… |
| R0365 | 13.3 | 882–882 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Define strip_term_links(B) lexically, not by resolving targets: replace every syntactically valid Term-li… |
| R0366 | 13.3 | 884–884 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | For a book pass: |
| R0367 | 13.3 | 886–886 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Retain foreign-book Term links as protected authored macros and report them. This proposal creates no cro… |
| R0368 | 13.3 | 887–887 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Normalize same-book recognized Term macros to labels even if their old target is now missing or retyped. … |
| R0369 | 13.3 | 888–888 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Refuse malformed Term-like markup for correction rather than guessing its intended label or deleting byte… |
| R0370 | 13.3 | 889–889 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Match and emit new same-book Term macros. |
| R0371 | 13.3 | 890–890 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Verify strip_term_links(new_body) == strip_term_links(old_body) byte for byte. |
| R0372 | 13.3 | 892–892 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | This verification never strips ordinary Links. It does not normalize whitespace, punctuation, escapes, Un… |
| R0373 | 13.3 | 894–894 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Title, metadata, and the Term list body are outside the pass and remain unchanged. Generated Reverse/Term… |
| R0374 | 13.4 | 898–898 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Calculate source ranges without Markdown reserialization. Protect: |
| R0375 | 13.4 | 902–902 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Fenced and inline code — Literal examples must remain literal |
| R0376 | 13.4 | 903–903 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Inline/display math, including supported dollar and backslash forms — Terms inside formulas are not prose… |
| R0377 | 13.4 | 904–904 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Ordinary Links and retained foreign Term links — Their macro bytes and targets must not change |
| R0378 | 13.4 | 905–905 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Markdown links/images, including reference-style forms — Do not change labels, destinations, or artifact/… |
| R0379 | 13.4 | 906–906 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Raw HTML blocks/inline tags and chart fences — Never rewrite markup or executable chart content |
| R0380 | 13.4 | 907–907 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Heading lines and table rows — Navigation/labels and tables remain exact |
| R0381 | 13.4 | 908–908 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Source lines — Preserve citation/locator text |
| R0382 | 13.4 | 909–909 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Term head metadata lines — Preserve identity, category, avoid words, and auto-link policy |
| R0383 | 13.4 | 910–910 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | The current term name anywhere in its own head/parts — No self-reference or redundant continuation use |
| R0384 | 13.4 | 911–911 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Generated Reverse text — It is not part of the authored body |
| R0385 | 13.4 | 913–913 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Use the shared scanner and existing Markdown offset facilities where appropriate. A syntax-aware offset w… |
| R0386 | 13.4 | 915–915 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Only match within an individual eligible text range. Do not match across emphasis delimiters, escapes, an… |
| R0387 | 13.5 | 919–919 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | There are at most 60 table terms by default. Start with a sorted literal candidate scan over link words a… |
| R0388 | 13.5 | 921–921 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | This has a small fixed vocabulary and is easier to inspect than adding Aho–Corasick solely for this pass.… |
| R0389 | 13.5 | 923–923 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Link words are exact term names with one valid head and auto-link enabled. Blockers include: |
| R0390 | 13.5 | 925–925 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Every avoid word. |
| R0391 | 13.5 | 926–926 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Every term name without a usable head or with auto-link disabled. |
| R0392 | 13.5 | 927–927 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Nonlinkable short names. |
| R0393 | 13.5 | 928–928 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Ambiguous unqualified forms of qualified homonyms. |
| R0394 | 13.5 | 930–930 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | For 学习（个体行为） and 学习（产业生产率）, the exact full forms can link, while the bare 学习 is a blocker. Do not mark th… |
| R0395 | 13.5 | 932–932 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Use the proposal's conservative short-name default: do not link a one-Han-character term or a Latin name … |
| R0396 | 13.5 | 934–934 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Latin matches are case-sensitive and require ASCII word boundaries at both ends, treating letters, digits… |
| R0397 | 13.5 | 936–936 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | Examples: |
| R0398 | 13.5 | 940–940 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | 净生产函数 and 生产函数 are terms — Link the longer full match |
| R0399 | 13.5 | 941–941 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | 替代弹性系数 is an avoid word — Consume it as a blocker; leave it unchanged |
| R0400 | 13.5 | 942–942 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | 相对要素价格 is nonlinkable but 要素价格 is linkable — Leave the longer full occurrence unchanged |
| R0401 | 13.5 | 943–943 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | GDP inside GDPR or GDP_2024 — No match |
| R0402 | 13.5 | 944–944 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | A literal term split by Markdown emphasis — Skip; do not cross markup |
| R0403 | 13.5 | 946–946 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | With first, take the first eligible occurrence of each exact term per body. With all, take all eligible o… |
| R0404 | 13.6 | 950–950 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | Apply revalidates archive identity, book/list/head mapping, policy contract, and each Card's authored gua… |
| R0405 | 13.6 | 952–952 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | If the mapping/options/source list changed, invalidate the whole plan and build a new preview. If only on… |
| R0406 | 13.6 | 954–954 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | Use base_fingerprint on every edit. A change after re-reading but before commit is a stale-base refusal, … |
| R0407 | 13.6 | 956–956 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | Pack unchanged-plan edits through existing identified-call chunks. A 300-Card book can require at least f… |
| R0408 | 13.6 | 958–958 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | No-op Cards produce no edit. If the plan has no edits, write no receipt and advance no archive revision. … |
| R0409 | 13.6 | 960–960 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | A card_locked response stops immediately with the typed tree/Lock facts and exit 2. A card_check response… |
| R0410 | 13.7 | 964–964 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | The report includes counts of scanned/changed Cards, added/retained/removed links, committed/pending chun… |
| R0411 | 13.7 | 966–966 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | The guarantees are: |
| R0412 | 13.7 | 968–968 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | Deterministic transformation for equal frozen inputs. |
| R0413 | 13.7 | 969–969 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | Exact visible-body preservation by a checked equation. |
| R0414 | 13.7 | 970–970 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | Idempotence on an unchanged valid tree. |
| R0415 | 13.7 | 971–971 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | Bounded operations and recovery through receipts. |
| R0416 | 13.7 | 972–972 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | Access and Lock enforcement through the existing service. |
| R0417 | 13.7 | 973–973 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | No synonym correction, prose rewrite, term rename, or mathematical edit. |
| R0418 | 13.7 | 975–975 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | The matcher can miss a relationship or skip a potential use. It does not prove linguistic disambiguation.… |
| R0419 | 14.1 | 981–981 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | Add base stages term-cards after terms and term-link after all Card writers and the term-card branch. Eco… |
| R0420 | 14.1 | 984–993 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | Architecture/stage flow contract: ; terms --> digest["Digest and variant Card writers"]; terms --> tc["te… |
| R0421 | 14.1 | 996–996 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | The term-cards branch uses frozen source sections and the shared term list. Concurrent model/digest write… |
| R0422 | 14.1 | 998–998 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | The runtime can parallelize these branches through existing dependency edges. Do not add another worker p… |
| R0423 | 14.2 | 1004–1004 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | termcards/tNNN@1 — Term list prepared; job schema, shared check, required reviews/fix disposition verifie… |
| R0424 | 14.2 | 1005–1005 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | termcards/book@1 — Every required row ready and its expected head/parts committed and read back |
| R0425 | 14.2 | 1006–1006 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | termlink/book@1 — Successful term-card publication and every enabled Card-writer unit complete; plan appl… |
| R0426 | 14.2 | 1007–1007 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | termlink/final@R — A finalization correction round R changed Cards; those Cards relinked and verified |
| R0427 | 14.2 | 1009–1009 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | Use a generation/round identity for finalization rather than one reusable final unit that could suppress … |
| R0428 | 14.2 | 1011–1011 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | A failed term job can leave a published prefix and ready later drafts. The successful book dependency is … |
| R0429 | 14.2 | 1013–1013 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | An explicit term-link mode may operate on the currently valid subset for diagnosis or backfill, reporting… |
| R0430 | 14.2 | 1015–1015 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | If a layer is disabled by args, omit its ledger dependency rather than wait forever for an unregistered u… |
| R0431 | 14.3 | 1021–1021 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | full — Existing preparation and writers, term-card branch, term-link, audits/finalization |
| R0432 | 14.3 | 1022–1022 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | term-cards — Validate existing book, source/sections preparation only, Term cards, Term links, audit, rep… |
| R0433 | 14.3 | 1023–1023 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | term-link — Validate existing book and staging/manifest, deterministic pass, audit, report |
| R0434 | 14.3 | 1024–1024 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | resume — Reconcile frozen plan, receipts, and registered units; resume only unfinished work |
| R0435 | 14.3 | 1026–1026 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | Backfill preparation must create no new root, anchor, group, model, or narrative Card. Add a narrow secti… |
| R0436 | 14.3 | 1028–1028 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | term-link mode needs no provider/source probe. term-cards needs source passages and the official provider… |
| R0437 | 14.3 | 1030–1030 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | Declare the proposal's args in the base schema: termCards, termLink, termCardsPerJob, termCardsMathPerJob… |
| R0438 | 14.4 | 1034–1034 | [S26 / #169](https://github.com/liudesheng9/zet/issues/169) | The first Term-link pass runs after bridge and every enabled variant writer. For each finalization correc… |
| R0439 | 14.4 | 1036–1036 | [S26 / #169](https://github.com/liudesheng9/zet/issues/169) | If a correction changes a Term list, head name, or auto-link policy, its effect is book-wide: invalidate … |
| R0440 | 14.4 | 1038–1038 | [S26 / #169](https://github.com/liudesheng9/zet/issues/169) | Do not add Term-link passes to every writer. The final write boundary is where deterministic linking belo… |
| R0441 | 14.4 | 1040–1040 | [S26 / #169](https://github.com/liudesheng9/zet/issues/169) | Extend audit with the proposal's terms section: rows/heads/parts, math/concept, T faults, uses by Card ty… |
| R0442 | 14.4 | 1042–1042 | [S26 / #169](https://github.com/liudesheng9/zet/issues/169) | Keep audit exit/completion distinct from book quality: “audit ran” does not mean “all terms passed.” Fina… |
| R0443 | 14.5 | 1046–1046 | [S27 / #166](https://github.com/liudesheng9/zet/issues/166) | Add concise guidance to every relevant digest/card/fill/model/explain/correction prompt: |
| R0444 | 14.5 | 1048–1048 | [S27 / #166](https://github.com/liudesheng9/zet/issues/166) | Preserve existing Term macros, or write the visible term plainly. |
| R0445 | 14.5 | 1049–1049 | [S27 / #166](https://github.com/liudesheng9/zet/issues/166) | Do not manufacture target addresses or synonyms. |
| R0446 | 14.5 | 1050–1050 | [S27 / #166](https://github.com/liudesheng9/zet/issues/166) | Run the exact SELF-CHECK before submit. |
| R0447 | 14.5 | 1051–1051 | [S27 / #166](https://github.com/liudesheng9/zet/issues/166) | Treat card_check as content to fix through the current finite fix cycle. |
| R0448 | 14.5 | 1052–1052 | [S27 / #166](https://github.com/liudesheng9/zet/issues/166) | Treat card_locked as a planning stop, never a content retry. |
| R0449 | 14.5 | 1054–1054 | [S27 / #166](https://github.com/liudesheng9/zet/issues/166) | Deterministic compilers preflight through the shared core and, when placement/context is needed, service … |
| R0450 | 15.1 | 1060–1060 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | Preserve existing fields and add: |
| R0451 | 15.1 | 1064–1064 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | Create/insert/edit and batch items — type:"term" and nullable term |
| R0452 | 15.1 | 1065–1065 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | get_card/get_cards/get_tree — Term name/role/list/count facts where applicable; raw default; optional ter… |
| R0453 | 15.1 | 1066–1066 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | get_terms {at} — Computed rows, heads, parts, uses, coverage, and faults for one list/descendant |
| R0454 | 15.1 | 1067–1067 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | search_cards — Accept type:"term" |
| R0455 | 15.1 | 1068–1068 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | list_links and outbound projections — kind:"term" and label |
| R0456 | 15.1 | 1069–1069 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | Broken-Link reads — Preserve target/source and include label/semantic target fault |
| R0457 | 15.1 | 1070–1070 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | Initialization and verified view — Rule/capability information and archive check contract |
| R0458 | 15.1 | 1071–1071 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | Write/preview replies — Structured warnings and card_check refusal data |
| R0459 | 15.1 | 1073–1073 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | get_terms uses one consistent archive read and carries archive revision. Return deterministic natural ord… |
| R0460 | 15.1 | 1075–1075 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | get_tree and list_trees counts/search chips must recognize the sixth Card type. Update tool enums, help t… |
| R0461 | 15.1 | 1077–1077 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | General owner-created Term Cards may be placed in a Topic tree under existing domain rules if the owner c… |
| R0462 | 15.2 | 1081–1081 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | The open Card's prose is the focus. Term links have the same font and size as that prose, with a quiet do… |
| R0463 | 15.2 | 1083–1083 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | A Term link: |
| R0464 | 15.2 | 1085–1085 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | Displays only its authored label. |
| R0465 | 15.2 | 1086–1086 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | Uses the existing Card navigation route and encoded target. |
| R0466 | 15.2 | 1087–1087 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | Offers the list-row short meaning and target address on hover/focus. |
| R0467 | 15.2 | 1088–1088 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | Has a visible keyboard focus state and works without hover. |
| R0468 | 15.2 | 1089–1089 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | Uses existing cross-Workspace color semantics. |
| R0469 | 15.2 | 1090–1090 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | Shows a broken style and accessible explanation when unusable. |
| R0470 | 15.2 | 1092–1092 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | Escape labels, meanings, targets, and attribute values in link_events; URL-encode the route separately. A… |
| R0471 | 15.2 | 1094–1094 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | On the Term Card, use the existing Card reader and Markdown sections. Render the head metadata quietly, a… |
| R0472 | 15.2 | 1096–1096 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | Extend the existing writing desk's type picker and term-name field. Raw editing continues to show macros.… |
| R0473 | 15.2 | 1098–1098 | [S16 / #158](https://github.com/liudesheng9/zet/issues/158) | Graph views default to excluding kind:term, with one explicit visibility toggle. Apply it to the Table/Co… |
| R0474 | 15.3 | 1102–1102 | [S15 / #157](https://github.com/liudesheng9/zet/issues/157) | Render a Term link as its label, with a restrained term color/underline. Record the existing LinkSpan tar… |
| R0475 | 15.3 | 1104–1104 | [S15 / #157](https://github.com/liudesheng9/zet/issues/157) | Keep edit mode raw. Broken targets remain visible as labels with the broken style. Test wrapping, hit ran… |
| R0476 | 15.4 | 1108–1108 | [S17 / #155](https://github.com/liudesheng9/zet/issues/155) | Dump all authored macros unchanged. Add a nullable/omitted term-name map in the existing manifest convent… |
| R0477 | 15.4 | 1110–1110 | [S17 / #155](https://github.com/liudesheng9/zet/issues/155) | A coordinated version/capability floor prevents old binaries from writing a new archive or digesting new … |
| R0478 | 15.4 | 1112–1112 | [S17 / #155](https://github.com/liudesheng9/zet/issues/155) | Do not claim that turning policy to advise makes the schema backward-compatible. It only changes quality … |
| R0479 | 16 | 1116–1116 | [S32 / #174](https://github.com/liudesheng9/zet/issues/174) | Backfill is a separately selected operation, not an automatic migration. |
| R0480 | 16 | 1118–1118 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | Select one book and inspect its list, existing descendants, source availability, Workspace, and Lock fact… |
| R0481 | 16 | 1119–1119 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | If locked, stop and name the tree/Lock. The owner can unlock it through the existing GUI; the workflow ca… |
| R0482 | 16 | 1120–1120 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | Prepare or reuse source sections only. Bind the source/list fingerprints. |
| R0483 | 16 | 1121–1121 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | Generate/check/review drafts and publish the ready ordered prefix. |
| R0484 | 16 | 1122–1122 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | When all required terms are verified, plan/apply Term links. |
| R0485 | 16 | 1123–1123 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | Run the read-only audit and report. |
| R0486 | 16 | 1124–1124 | [S32 / #174](https://github.com/liudesheng9/zet/issues/174) | The owner reviews the result and can lock the tree again. |
| R0487 | 16 | 1126–1126 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | The allowed writes are the existing list's type/name metadata as required, new term heads/parts, and Term… |
| R0488 | 16 | 1128–1128 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | Before applying, show conflicting pre-existing children, duplicate rows, missing source passages, and any… |
| R0489 | 17 | 1132–1132 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Each phase should be reviewable independently. These are implementation phases, not GitHub issues created… |
| R0490 | 17 | 1136–1136 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | 1 — Pure checker extraction and feature boundary — Existing checker/digest fixtures unchanged; root can l… |
| R0491 | 17 | 1137–1137 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | 2 — Shared text parser and structured diagnostics — Old envelope/Link fixtures pass; no duplicate diagnos… |
| R0492 | 17 | 1138–1138 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | 3 — Gate adapter, policy persistence, CLI drafts, version checks, preview aggregation — Every writer rout… |
| R0493 | 17 | 1139–1139 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | 4 — Term type/field migration and computed roles — Old archive/Lock/dump identities preserved; term read/… |
| R0494 | 17 | 1140–1140 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | 5 — Term syntax, index/lifecycle projections, GUI/Session display — Move/Access/Lock/reverse behavior mat… |
| R0495 | 17 | 1141–1141 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | 6 — T rules and both digest commands — Draft/review/order/receipt and byte-preservation/idempotence fixtu… |
| R0496 | 17 | 1142–1142 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | 7 — Base/modules/prompts/finalize/report integration — Both variants assembled and harnessed; refusal rec… |
| R0497 | 17 | 1143–1143 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | 8 — Copy-based pilot and advisory observation — Content depth/layout accepted; performance and legacy-fau… |
| R0498 | 17 | 1144–1144 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | 9 — Coordinated release and enforcement — Compatible binaries/workflows installed together; type then age… |
| R0499 | 17 | 1145–1145 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | 10 — Optional selected backfills — Each book has its own report and completeness/Lock disposition |
| R0500 | 17 | 1147–1147 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Do not switch blocking rules on while old writers still expect “write first, fix later.” Keep existing ha… |
| R0501 | 17 | 1149–1149 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | The gate proposal suggests a week of advisory observation. Treat that as an operational observation windo… |
| R0502 | 17 | 1151–1151 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Use the proposal's version numbers only as candidate release numbers. Check current package versions and … |
| R0503 | 18 | 1155–1155 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | This task writes a design only. The checks below are implementation acceptance work, not results claimed … |
| R0504 | 18.1 | 1159–1159 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | Build/test zt-check with defaults, and build/check it with --no-default-features --lib. |
| R0505 | 18.1 | 1160–1160 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | Verify core-only dependency output has no MCP/process-platform code. |
| R0506 | 18.1 | 1161–1161 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | Run the same old fixture set through the extracted CLI and the gate adapter. |
| R0507 | 18.1 | 1162–1162 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | Add envelope fixtures with the same existing messages and parsing behavior. |
| R0508 | 18.1 | 1163–1163 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | Compare ordered structured diagnostics and formatted lines, not only counts. |
| R0509 | 18.1 | 1164–1164 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | Confirm zt-digest still calls run_lines with its existing shared MCP client. |
| R0510 | 18.2 | 1170–1170 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | New Event/Finding body missing required labels — Refused; no Card/id/receipt/revision change |
| R0511 | 18.2 | 1171–1171 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Advisory style fault in human Workspace — Stored with returned warning |
| R0512 | 18.2 | 1172–1172 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Same fault retained in a legacy Card — Stored with legacy advice |
| R0513 | 18.2 | 1173–1173 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Same faulty paragraph copied a second time — Additional occurrence blocks |
| R0514 | 18.2 | 1174–1174 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Two different long messages sharing a clipped prefix — Distinct faults; neither hides the other |
| R0515 | 18.2 | 1175–1175 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Idea retyped to Event with malformed body — Newly applicable Event format blocks |
| R0516 | 18.2 | 1176–1176 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Concept model promoted to formal — Newly applicable formal checks run |
| R0517 | 18.2 | 1177–1177 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Metadata-only edit/retype — Proposed metadata checked even with identical body |
| R0518 | 18.2 | 1178–1178 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Move/rename/generated Reverse refresh — No spurious prose rejection; existing structural safeguards remai… |
| R0519 | 18.2 | 1179–1179 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Policy override — One archive/Workspace/rule changes, other rules and Access remain intact |
| R0520 | 18.2 | 1180–1180 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Missing context/version mismatch — Incomplete/stale result, never successful acceptance |
| R0521 | 18.2 | 1181–1181 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Mixed batch — One blocker rolls back the whole real batch |
| R0522 | 18.2 | 1182–1182 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Preview with independent/dependent later items — Collect independent faults, mark dependencies blocked, p… |
| R0523 | 18.2 | 1184–1184 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Add a parameterized route matrix across GUI, MCP, shell, Session, insert, and batch paths. Test both Work… |
| R0524 | 18.3 | 1188–1188 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Migrate old archives with active Locks, pinned targets, receipts, and non-Idea types. |
| R0525 | 18.3 | 1189–1189 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Preserve every old row id, authored text, old fingerprint, active seal, and historical event. |
| R0526 | 18.3 | 1190–1190 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Change only term_name on a Locked head and prove refusal. |
| R0527 | 18.3 | 1191–1191 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Round-trip names/types/parts through dump and load. |
| R0528 | 18.3 | 1192–1192 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Parse UTF-8 label/target byte ranges, strict errors, tolerant rendering, and literal code/math examples. |
| R0529 | 18.3 | 1193–1193 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Rewrite only the target on moves/compaction/key rename; refuse an agent rewrite into a human body. |
| R0530 | 18.3 | 1194–1194 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Compare indexed outputs with full lifecycle outputs for ordinary Reverse links and Term uses. |
| R0531 | 18.3 | 1195–1195 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Verify deduplication when a source has repeated Term uses, or both use and reasoned Link. |
| R0532 | 18.3 | 1196–1196 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Prove Term links affect Lock pins but never Model membership/neighbors or reasoning counts. |
| R0533 | 18.3 | 1197–1197 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Prove a malformed or wrong-role new target is refused, while the defined legacy retention is reported. |
| R0534 | 18.4 | 1201–1201 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Source-bound plan generation, classification, promotion, 1/4 job grouping, and stable ids. |
| R0535 | 18.4 | 1202–1202 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Every required JSON field, exact table-row spelling, auto-link persistence, and deterministic rendering. |
| R0536 | 18.4 | 1203–1203 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Mathematical reviewer independent from writer; changed draft invalidates review; one fix bound. |
| R0537 | 18.4 | 1204–1204 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Continuation packing at whole steps, T-rule applicability on heads/parts, and unsplittable overflow. |
| R0538 | 18.4 | 1205–1205 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Prefix publication with an earlier failed row; later ready rows stay staged. |
| R0539 | 18.4 | 1206–1206 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Lost-reply replay, chunk interruption, receipt recovery, stale list/source conflict, and no duplicates. |
| R0540 | 18.4 | 1207–1207 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Leftmost-longest/blocker/homonym/Latin-boundary fixtures. |
| R0541 | 18.4 | 1208–1208 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | All protected spans, CRLF-normalized parsed baselines, Unicode, escapes, reference links, HTML, and incom… |
| R0542 | 18.4 | 1209–1209 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Property test: stripping old/new recognized Term macros gives byte-identical bodies for generated fixture… |
| R0543 | 18.4 | 1210–1210 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Idempotence: a second plan has zero edits and advances no revision. |
| R0544 | 18.4 | 1211–1211 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Changed mapping/body guards, explicit plan revision, and immediate Lock stop. |
| R0545 | 18.5 | 1215–1215 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Existing framework-harness.mjs verifies stage order, disabled-layer dependencies, resume, dedicated modes… |
| R0546 | 18.5 | 1216–1216 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Simulate successful and failed term branches; audit/report still run, but incomplete work never becomes d… |
| R0547 | 18.5 | 1217–1217 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Simulate global manager/conversation caps while digest and term jobs overlap. |
| R0548 | 18.5 | 1218–1218 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Verify every recorded session is collected/deleted and every workspace registration cleaned, including fa… |
| R0549 | 18.5 | 1219–1219 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Confirm term-only modes never rerun digest/model writers or provider probes in deterministic-only mode. |
| R0550 | 18.5 | 1220–1220 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Inspect GUI light/dark, hover/focus, broken/cross-Workspace links, completion, ordinary Link distinction,… |
| R0551 | 18.5 | 1221–1221 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Inspect Session width/wrap/hit testing and raw editing with Han and combining characters. |
| R0552 | 18.6 | 1225–1225 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Target less than 5 ms of additional gate work per typical Card, measured separately from lock waiting, fi… |
| R0553 | 18.6 | 1227–1227 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Benchmark a legal 64-item real batch and a 200-item preview sequence or four legal real batches. Do not b… |
| R0554 | 18.6 | 1229–1229 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | For the relinker, measure planning and apply on a 300-Card fixture and record actual batch count. The sma… |
| R0555 | 18.6 | 1231–1231 | [S30 / #172](https://github.com/liudesheng9/zet/issues/172) | Pilot on a consistent copy of the box, with grossman2002integration if still suitable at implementation t… |
| R0556 | 18.6 | 1233–1233 | [S30 / #172](https://github.com/liudesheng9/zet/issues/172) | Preserve the Term list's authored title/body exactly. |
| R0557 | 18.6 | 1234–1234 | [S30 / #172](https://github.com/liudesheng9/zet/issues/172) | Account for every row as verified head/parts or a named unresolved fault. |
| R0558 | 18.6 | 1235–1235 | [S30 / #172](https://github.com/liudesheng9/zet/issues/172) | Have the owner inspect three mathematical terms and two concept terms for actual explanatory depth. |
| R0559 | 18.6 | 1236–1236 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Verify preservation for every changed body and zero writes on a second pass. |
| R0560 | 18.6 | 1237–1237 | [S30 / #172](https://github.com/liudesheng9/zet/issues/172) | Have the owner inspect ten GUI Cards and judge first-occurrence linking and layout. |
| R0561 | 18.6 | 1238–1238 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | Compare language/Link diagnostics before/after; Term markup alone must add no prose faults. |
| R0562 | 18.6 | 1239–1239 | [S30 / #172](https://github.com/liudesheng9/zet/issues/172) | Measure provider usage and runtime; use actual logs rather than unverified cost estimates. |
| R0563 | 18.6 | 1241–1241 | [S30 / #172](https://github.com/liudesheng9/zet/issues/172) | A stand-in workflow runtime proves orchestration behavior only. It does not prove live provider cleanup, … |
| R0564 | 19 | 1245–1245 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | Do not deploy during an active digest. Follow the repository's Windows service/executable procedure and c… |
| R0565 | 19 | 1247–1247 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | Release zt, zt-check, zt-digest, both rebuilt digest variants, and installed agent guidance as one compat… |
| R0566 | 19 | 1249–1249 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | Back up the archive consistently before schema migration. Validate the copy, run migration and postflight… |
| R0567 | 19 | 1251–1251 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | Policy rollback can return new quality checks to advice, keeping existing mandatory checks. It needs no C… |
| R0568 | 19 | 1253–1253 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | For an interrupted compiler/pass: |
| R0569 | 19 | 1255–1255 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | Read the frozen plan and committed receipts. |
| R0570 | 19 | 1256–1256 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | Verify effects and identity before retrying. |
| R0571 | 19 | 1257–1257 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | Resume the same accepted operation keys; never apply compensating deletion blindly. |
| R0572 | 19 | 1258–1258 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | Report already committed chunks and pending work. |
| R0573 | 19 | 1259–1259 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | Stop on Locks/Access or semantic conflicts. |
| R0574 | 19 | 1260–1260 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | Restore a backup only through a separately authorized, reviewed recovery operation that accounts for subs… |
| R0575 | 19 | 1262–1262 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | Do not automatically strip all Term links, retype Cards back to Idea, delete heads, or restore an old arc… |
| R0576 | 20.1 | 1270–1270 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | G1 / D1–D4: new type, unchanged list, one term per row, parts and order — Sections 9, 11, 12 |
| R0577 | 20.1 | 1271–1271 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | G2–G4 / D5–D10: direct label link, deterministic text-preserving pass, first/all, final pass — Sections 1… |
| R0578 | 20.1 | 1272–1272 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | G5: R6 and idea-grow keep reading the list — Sections 9.3, 10.4, 14; list body unchanged |
| R0579 | 20.1 | 1273–1273 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | G6 / D11–D15: parallel branch, DSMP, writer/reviewer/fix, limits/model — Sections 11, 12, 14 |
| R0580 | 20.1 | 1274–1274 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Proposal 4.4: computed structure and all seven named faults — Section 9.2 |
| R0581 | 20.1 | 1275–1275 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Proposal 5 and appendices A–C: content schema, deep math, examples, review — Sections 11 and 12; reuse th… |
| R0582 | 20.1 | 1276–1276 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Proposal 6: grammar, rewrite, Reverse/use sections, broken targets, Locks — Section 10 |
| R0583 | 20.1 | 1277–1277 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Proposal 7: GUI, editor, Session, raw/projected reads — Sections 10.4 and 15 |
| R0584 | 20.1 | 1278–1278 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Proposal 8: steps/jobs/ledger/failure behavior — Sections 12 and 14, with explicit prefix-publication ref… |
| R0585 | 20.1 | 1279–1279 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Proposal 9 and appendix D: matching, protected spans, preservation — Section 13 |
| R0586 | 20.1 | 1280–1280 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Proposal 10: T1–T10, old rule changes, audit counts — Sections 6, 10.4, 11.3, 14.4 |
| R0587 | 20.1 | 1281–1281 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Proposal 11: CLI/MCP/GUI/Session/digest/skills/docs surfaces — Sections 7, 14, 15, 21 |
| R0588 | 20.1 | 1282–1282 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Proposal 12–15: backfill, versions, pilot, tests, risks — Sections 16–19 and 22 |
| R0589 | 20.1 | 1283–1283 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Q1–Q4 — First occurrence; exact term title; no row links; source-based parallel generation |
| R0590 | 20.1 | 1284–1284 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Q5 — Supported but separately selected after pilot; no live backfill in this task |
| R0591 | 20.1 | 1285–1285 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Q6 / P9 optional — Exact-name entry ranking and register/cross-book integration remain later work |
| R0592 | 20.1 | 1286–1286 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Q7 / N5 — No per-symbol Cards or symbol links |
| R0593 | 20.1 | 1287–1287 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | N1–N4 — Model/notation tables, Topic-side digest scope, and no synonym rewrites preserved |
| R0594 | 20.1 | 1289–1289 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | The optional label-only read projection is included because it is a small use of the shared text function… |
| R0595 | 20.2 | 1295–1295 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | One checker, two products, no service-to-itself process — Sections 1, 5 |
| R0596 | 20.2 | 1296–1296 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Envelope unification and context boundary — Sections 5, 7 |
| R0597 | 20.2 | 1297–1297 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Local/context/graph split — Sections 5.3 and 6.2 |
| R0598 | 20.2 | 1298–1298 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Workspace policy and archive overrides — Section 6 |
| R0599 | 20.2 | 1299–1299 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Legacy ratchet, retypes and all write routes — Sections 6.3 and 7 |
| R0600 | 20.2 | 1300–1300 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Typed refusal and warnings — Sections 5.4 and 7.3 |
| R0601 | 20.2 | 1301–1301 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Atomic real batch and exhaustive preview — Section 7.4 |
| R0602 | 20.2 | 1302–1302 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | --workspace, --draft, blocking-only counts — Section 8 |
| R0603 | 20.2 | 1303–1303 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Initialization version and stale tool behavior — Section 8.3 |
| R0604 | 20.2 | 1304–1304 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Literature scope and box pin stay client-side — Sections 10.4 and 14 |
| R0605 | 20.2 | 1305–1305 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Digest SELF-CHECK and content-fix routing — Section 14.5 |
| R0606 | 20.2 | 1306–1306 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Phased enforcement and service redeploy implications — Sections 17 and 19 |
| R0607 | 20.2 | 1307–1307 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Parity, policy, preview, version, latency, legacy tests — Section 18 |
| R0608 | 20.2 | 1308–1308 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Owner questions — Proposed defaults in sections 3 and 6; existing preview only, no extra check tool |
| R0609 | 20.3 | 1312–1312 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | No cross-book deterministic term linking, symbol-card system, Topic-side Term generation by the digest, a… |
| R0610 | 20.3 | 1314–1314 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | The existing owner workflows may link a term across Workspaces, but that does not expand what a digest is… |
| R0611 | 21 | 1318–1318 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Read each block doc before editing its code, and update the owning doc in the same commit. This design-on… |
| R0612 | 21 | 1322–1322 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Checker — the Cargo.toml functional surface, lib.rs, check.rs, new core.rs/text.rs/policy.rs, the mcp sco… |
| R0613 | 21 | 1323–1323 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Cards/Links — the Card envelope and fingerprint module, card_type.rs, link.rs, shared scanner facade, new… |
| R0614 | 21 | 1324–1324 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Service/storage — Root Cargo dependency, main.rs schema/config/direct writers, dump.rs, link_index.rs, tr… |
| R0615 | 21 | 1325–1325 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Operations — ops.rs candidate/type changes/results, place.rs inserts and snapshots — Card operations |
| R0616 | 21 | 1326–1326 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Models — model.rs relation parsing and Term-use filters — Models and math |
| R0617 | 21 | 1327–1327 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | MCP — mcp.rs arguments, tools, metadata, replies, instructions — MCP server |
| R0618 | 21 | 1328–1328 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | GUI — gui.rs rendering/API, the gui app.js surface editor/graphs/disclosures, app.css — GUI |
| R0619 | 21 | 1329–1329 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Session — session.rs and command/edit result handling — Session |
| R0620 | 21 | 1330–1330 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Digest — New compilers, terms.rs, calls.rs fingerprints, audit/finalize, ledger verification, command dis… |
| R0621 | 21 | 1331–1331 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Workflow — New modules, base schema/stages, shared finite refusal routing, variant frozen builds — Digest… |
| R0622 | 21 | 1332–1332 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Tests/wrappers — New fixtures/harness cases and existing test runner integration — Digest scripts |
| R0623 | 21 | 1333–1333 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Prompts — Two new instructions and concise writer compatibility guidance — Digest prompts |
| R0624 | 21 | 1334–1334 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Skills — the zt functional surface, the zettelkasten functional surface, the zt-digest-protocol functiona… |
| R0625 | 21 | 1335–1335 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Domain/user docs — the domain glossary, user documentation, digest command/workflow docs — Existing repos… |
| R0626 | 21 | 1337–1337 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | Assign new files to exactly one wiki block during implementation. gate.rs belongs to service/storage; the… |
| R0627 | 22 | 1343–1343 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Gate activates before writer compatibility — Deliver draft checks, previews, and finite card_check handli… |
| R0628 | 22 | 1344–1344 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Expensive per-Card whole-box reads — Materialize bounded context; keep graph rules audit-only; measure qu… |
| R0629 | 22 | 1345–1345 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Old faults become an edit lockout — Structured multiset ratchet with role-aware applicability |
| R0630 | 22 | 1346–1346 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | New faults hidden behind old clipped text — Compare unclipped semantic diagnostic keys |
| R0631 | 22 | 1347–1347 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Wrong or shallow mathematical explanation — Source-bound drafts, complete template, separate review, one … |
| R0632 | 22 | 1348–1348 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Auto-link choice lost after staging cleanup — Persist it as an authored metadata line |
| R0633 | 22 | 1349–1349 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Partial generation destroys row order — Publish only a ready contiguous prefix |
| R0634 | 22 | 1350–1350 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Interruption creates duplicate heads — Stable operation keys, receipt recovery, explicit readback |
| R0635 | 22 | 1351–1351 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Target deletion makes old links impossible to normalize — Lexical same-book stripping independent of targ… |
| R0636 | 22 | 1352–1352 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Relinker corrupts Markdown or prose — Source offsets, conservative protected spans, preservation equation… |
| R0637 | 22 | 1353–1353 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Labels disappear from style checks — Shared visible-text projection before existing prose rules |
| R0638 | 22 | 1354–1354 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Dense Term edges overwhelm reasoning views — Default graph filtering and strict Model/Idea-count exclusio… |
| R0639 | 22 | 1355–1355 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Schema change invalidates active Locks — Preserve null-field old hashes; update frozen fields, SQL wrappe… |
| R0640 | 22 | 1356–1356 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Policy/version drift creates a false pass — Service-provided check contract, mismatch/incomplete exit sta… |
| R0641 | 22 | 1357–1357 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Old executable drops new fields — Coordinated capability floor; refuse incompatible writes |
| R0642 | 22 | 1358–1358 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | Partial run presented as finished — Separate command execution, publication coverage, ledger completion, … |
| R0643 | 22 | 1360–1360 | [PRD / #142](https://github.com/liudesheng9/zet/issues/142) | The resulting structure is small: one existing checker crate becomes reusable, one existing Link lifecycl… |

## User-story ownership

| Story | Owner | User goal |
|---|---|---|
| US001 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | As a reader, I want one definition for Card legality, so that self-check and storage agree. |
| US002 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | As a maintainer, I want one process-free rule core in the existing checker crate, so that reuse adds no gate service or dependency loop. |
| US003 | [S01 / #143](https://github.com/liudesheng9/zet/issues/143) | As a digest tool author, I want the existing run_lines interface and fixture ordering, so that current workflows retain behavior during extraction. |
| US004 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | As a Card author, I want structured actionable diagnostics, so that I can fix the exact rejected requirement. |
| US005 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | As an owner, I want retained legacy faults to remain advisory, so that old Cards stay editable. |
| US006 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | As an owner, I want new duplicate faults to be distinguished from retained faults, so that edits cannot quietly worsen quality. |
| US007 | [S02 / #144](https://github.com/liudesheng9/zet/issues/144) | As an author, I want new type and formal-role rules applied on metadata changes, so that retyping cannot bypass format checks. |
| US008 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | As an owner, I want archive- and Workspace-specific policy, so that human notes and agent digest standards can differ. |
| US009 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | As an owner, I want validated policy set/reset and visible defaults, so that configuration typos cannot weaken unrelated checks. |
| US010 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | As an author, I want policy advice and hard invariants distinguished, so that warnings do not grant Access or unlock a tree. |
| US011 | [S03 / #145](https://github.com/liudesheng9/zet/issues/145) | As a maintainer, I want graph-only rules kept in audits, so that writes do not scan the whole box. |
| US012 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | As a Card author, I want every write route checked transactionally, so that GUI, Session, shell and MCP agree. |
| US013 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | As an agent, I want bounded accurate proposed-Card context, so that checks use the state actually being committed. |
| US014 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | As an author, I want warnings on accepted writes, so that I can improve a stored Card without losing my draft. |
| US015 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | As an author, I want refusal rollback without allocated Cards or receipts, so that failed writes leave no partial work. |
| US016 | [S04 / #146](https://github.com/liudesheng9/zet/issues/146) | As an owner, I want pure moves and generated Reverse text handled by existing structural rules, so that navigation maintenance is not blocked by prose heuristics. |
| US017 | [S05 / #147](https://github.com/liudesheng9/zet/issues/147) | As an agent, I want ordered atomic checked batches, so that one invalid item cannot partially publish a batch. |
| US018 | [S05 / #147](https://github.com/liudesheng9/zet/issues/147) | As an agent, I want rollback-only previews that collect independent failures, so that I can correct a plan before writing. |
| US019 | [S05 / #147](https://github.com/liudesheng9/zet/issues/147) | As an agent, I want blocked preview dependencies named honestly, so that unevaluated candidates are not reported as checked. |
| US020 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | As a writer, I want exact draft text and metadata checked before submission, so that I receive the gate answer early. |
| US021 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | As a writer, I want honest incomplete offline outcomes, so that missing archive context is not called a pass. |
| US022 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | As a maintainer, I want the running daemon rule/policy/capability contract, so that stale clients cannot promise acceptance. |
| US023 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | As a workflow author, I want consistent blocker/advisory counts and exit codes, so that all check consumers interpret results the same way. |
| US024 | [S06 / #148](https://github.com/liudesheng9/zet/issues/148) | As an owner, I want stored Card Workspace respected by live checks, so that a client flag cannot reclassify permissions. |
| US025 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | As a reader, I want a dedicated Term Card type and exact term name, so that definitions are discoverable without title guessing. |
| US026 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | As an owner, I want old archives upgraded without identity loss, so that existing Locks and references remain valid. |
| US027 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | As an owner, I want term names included in authored identity, so that a Locked definition cannot change through new metadata. |
| US028 | [S07 / #149](https://github.com/liudesheng9/zet/issues/149) | As an author, I want clear omitted/null/set field semantics, so that editing and retyping are predictable. |
| US029 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | As a reader, I want computed lists, heads, parts and uses, so that term structure needs no second ownership store. |
| US030 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | As an owner, I want duplicate/missing/extra/misfiled term faults, so that coverage debt remains visible. |
| US031 | [S08 / #150](https://github.com/liudesheng9/zet/issues/150) | As an existing reader, I want the original four-column term table unchanged, so that R6 and idea-grow retain their current input. |
| US032 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | As a beginning researcher, I want plain explanations and the source book meaning, so that I can learn a term without already knowing its mathematics. |
| US033 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | As a researcher, I want full mathematical definitions, hypotheses and notation, so that I can see exactly what a formula assumes. |
| US034 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | As a researcher, I want every derivation step, reason, limit and checked example, so that I can reproduce the explanation. |
| US035 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | As a reader, I want bounded continuation parts for long explanations, so that depth does not require an unreadable single Card. |
| US036 | [S09 / #152](https://github.com/liudesheng9/zet/issues/152) | As an owner, I want an automatic-link opt-out that survives export/staging loss, so that a too-general term stays unlinked. |
| US037 | [S10 / #151](https://github.com/liudesheng9/zet/issues/151) | As a reader, I want a direct word-to-Term-Card link, so that I can open the definition from the sentence. |
| US038 | [S10 / #151](https://github.com/liudesheng9/zet/issues/151) | As a maintainer, I want exact UTF-8 macro/label/target ranges, so that rewrites preserve label and surrounding text. |
| US039 | [S10 / #151](https://github.com/liudesheng9/zet/issues/151) | As an author, I want wrong-target and malformed Term uses rejected, so that new stored references remain valid. |
| US040 | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | As a reader, I want generated Term uses separate from Reverse reasoning Links, so that I can inspect definition usage without confusing it with argument. |
| US041 | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | As an author, I want Term links rewritten correctly on moves, so that definitions remain reachable. |
| US042 | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | As an owner, I want Term uses included in Lock pins and Access rewrite checks, so that new syntax cannot bypass existing promises. |
| US043 | [S11 / #153](https://github.com/liudesheng9/zet/issues/153) | As a reader, I want broken and stale uses with their label/source/target, so that I can identify what needs attention. |
| US044 | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | As a reader, I want quality checks to read visible term labels, so that markup does not remove words from a sentence. |
| US045 | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | As a digest author, I want Literature scope to inspect actual Term targets, so that label syntax cannot hide a Topic write. |
| US046 | [S12 / #154](https://github.com/liudesheng9/zet/issues/154) | As a prompt consumer, I want optional label-only reads marked as projections, so that read-only jobs save clutter without corrupting edit bases. |
| US047 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | As a GUI reader, I want quiet label-only links distinct from ordinary chips, so that definitions do not overwhelm prose. |
| US048 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | As a keyboard user, I want focusable navigation and accessible broken/hover meaning, so that I can use Term links without a mouse. |
| US049 | [S13 / #156](https://github.com/liudesheng9/zet/issues/156) | As a GUI reader, I want an ordinary explanation Card with collapsed uses, so that the new content fits the existing reading model. |
| US050 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | As a GUI author, I want term type/name editing in the existing desk, so that I need no separate authoring application. |
| US051 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | As a GUI author, I want name completion disambiguated by book/address, so that I insert the intended direct use. |
| US052 | [S14 / #161](https://github.com/liudesheng9/zet/issues/161) | As an author, I want ordinary reasoned citations to Term Cards still available, so that a claim about a definition remains distinct from a word use. |
| US053 | [S15 / #157](https://github.com/liudesheng9/zet/issues/157) | As a Session reader, I want label-only following with accurate display widths, so that hidden address bytes do not break navigation. |
| US054 | [S15 / #157](https://github.com/liudesheng9/zet/issues/157) | As a Session author, I want raw editing and correct wrapping/copying, so that I can inspect and preserve the stored syntax. |
| US055 | [S16 / #158](https://github.com/liudesheng9/zet/issues/158) | As a researcher, I want Term uses excluded from reasoning credit and Models, so that a vocabulary reference does not become an argument. |
| US056 | [S16 / #158](https://github.com/liudesheng9/zet/issues/158) | As a graph reader, I want Term edges hidden by default and explicitly showable, so that definitions do not dominate every graph. |
| US057 | [S17 / #155](https://github.com/liudesheng9/zet/issues/155) | As an owner, I want portable dumps preserving names/types/raw macros, so that backup and export retain authored meaning. |
| US058 | [S17 / #155](https://github.com/liudesheng9/zet/issues/155) | As a maintainer, I want older supported clients to refuse incompatible archives, so that they cannot silently erase new fields. |
| US059 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | As a digest manager, I want source-bound deterministic term jobs, so that writers read only relevant passages. |
| US060 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | As a digest manager, I want recorded math classification and promotion, so that formal definitions receive the right review. |
| US061 | [S18 / #159](https://github.com/liudesheng9/zet/issues/159) | As a maintainer, I want the existing ledger and compiler artifacts reused, so that the feature adds no second scheduler. |
| US062 | [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | As a researcher, I want schema/format checks before publication, so that incomplete content is not stored as a finished definition. |
| US063 | [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | As a researcher, I want independent review of every exact mathematical draft, so that the writer is not its only judge. |
| US064 | [S19 / #162](https://github.com/liudesheng9/zet/issues/162) | As a manager, I want one finite fix and re-review with hash binding, so that changed content cannot inherit stale approval. |
| US065 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | As an owner, I want the required Haiku and official DeepSeek roles, so that management does not become prose authorship. |
| US066 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | As an operator, I want writer/reviewer lifetime caps shared with digest work, so that parallel branches remain bounded without deadlock. |
| US067 | [S20 / #164](https://github.com/liudesheng9/zet/issues/164) | As an operator, I want collected/deleted conversations and cleaned workspace registrations, so that provider resources do not leak. |
| US068 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | As an owner, I want ordered prefix publication without placeholders, so that table order survives a failed early draft. |
| US069 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | As an operator, I want receipt recovery after a lost publication reply, so that a retry creates no duplicate Term head. |
| US070 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | As an owner, I want truthful publication coverage, so that a partial book is never marked done. |
| US071 | [S21 / #165](https://github.com/liudesheng9/zet/issues/165) | As a maintainer, I want stable plan revisions and historical-content conflicts, so that new drafts do not overwrite accepted Cards blindly. |
| US072 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | As an owner, I want one-book scoped deterministic link planning, so that only eligible agent Literature prose changes. |
| US073 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | As a reader, I want first/all occurrence policy with conservative blockers, so that useful words link without false substring uses. |
| US074 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | As an author, I want all Markdown/math/code/source spans protected, so that linking preserves syntax and scientific content. |
| US075 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | As an owner, I want lexical stale-link normalization and exact visible-text equality, so that missing targets do not prevent safe repair. |
| US076 | [S22 / #160](https://github.com/liudesheng9/zet/issues/160) | As a reviewer, I want a deterministic preview and skip counts, so that I can assess every planned substitution. |
| US077 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | As an operator, I want guarded bounded apply and explicit replanning, so that concurrent edits are never overwritten. |
| US078 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | As an operator, I want per-chunk committed/pending reports and recovery, so that a large book remains resumable. |
| US079 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | As an owner, I want zero-write idempotent reruns, so that unchanged content produces no revisions. |
| US080 | [S23 / #163](https://github.com/liudesheng9/zet/issues/163) | As an operator, I want typed Lock and quality stops, so that the tool responds correctly without rewriting prose to pass. |
| US081 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | As a digest manager, I want Term creation beside digest and linking after all writers, so that the final bodies receive their definitions. |
| US082 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | As a workflow maintainer, I want truthful ledger dependencies and disabled-layer behavior, so that missing work cannot be marked complete to unblock a stage. |
| US083 | [S24 / #167](https://github.com/liudesheng9/zet/issues/167) | As an owner, I want audits/reports even when a branch fails, so that I see incomplete coverage rather than a lost run. |
| US084 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | As an owner, I want dedicated one-book backfill mode, so that finished models need not be digested again. |
| US085 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | As an operator, I want sections-only source reconstruction and deterministic-only provider-free mode, so that resuming does only necessary work. |
| US086 | [S25 / #168](https://github.com/liudesheng9/zet/issues/168) | As an owner, I want Lock/source/occupancy conflicts before dispatch, so that historical work is preserved. |
| US087 | [S26 / #169](https://github.com/liudesheng9/zet/issues/169) | As a reader, I want Term links restored after actual correction rounds, so that final edits do not silently remove definitions. |
| US088 | [S26 / #169](https://github.com/liudesheng9/zet/issues/169) | As a maintainer, I want generation-specific final units and mapping invalidation, so that later corrections are not skipped. |
| US089 | [S26 / #169](https://github.com/liudesheng9/zet/issues/169) | As an owner, I want coverage, T faults, uses and remaining avoid words in final reports, so that execution success is distinct from quality completeness. |
| US090 | [S27 / #166](https://github.com/liudesheng9/zet/issues/166) | As a writer, I want exact pre-submit SELF-CHECK guidance, so that faulty writes can be corrected before the gate. |
| US091 | [S27 / #166](https://github.com/liudesheng9/zet/issues/166) | As a workflow author, I want card_check routed through the finite fix cycle, so that content refusals are not provider crashes. |
| US092 | [S27 / #166](https://github.com/liudesheng9/zet/issues/166) | As an owner, I want card_locked treated as a planning stop, so that writers cannot evade a frozen tree. |
| US093 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | As a maintainer, I want compatible frozen builds and current version/capability manifests, so that deployment does not mix incompatible products. |
| US094 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | As an agent, I want updated domain, command and skill guidance, so that Term work follows the same boundaries everywhere. |
| US095 | [S28 / #170](https://github.com/liudesheng9/zet/issues/170) | As an owner, I want new enforcement staged behind compatibility evidence, so that existing writers are not stranded mid-run. |
| US096 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | As a maintainer, I want public-behavior evidence for the full requirement matrix, so that passing a narrow fixture does not imply feature completion. |
| US097 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | As an operator, I want measured gate/context/batch/relink performance, so that bounded targets are supported by actual evidence. |
| US098 | [S29 / #171](https://github.com/liudesheng9/zet/issues/171) | As an owner, I want explicit limitations of fake runtimes and heuristic checks, so that orchestration tests do not claim semantic or live qualification. |
| US099 | [S30 / #172](https://github.com/liudesheng9/zet/issues/172) | As an owner, I want a copied-book mathematical depth review, so that the explanations meet the requested learning standard. |
| US100 | [S30 / #172](https://github.com/liudesheng9/zet/issues/172) | As an owner, I want a copied-book layout, preservation and resource-use review, so that the visible feature is useful and repeatable. |
| US101 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | As an owner, I want a scheduled compatible live release with verified backups, so that deployment preserves the active archive. |
| US102 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | As an owner, I want advisory observation and deliberate enforcement promotion, so that a timer cannot switch my policy automatically. |
| US103 | [S31 / #173](https://github.com/liudesheng9/zet/issues/173) | As an owner, I want documented policy rollback and schema recovery boundaries, so that rollback does not silently discard later work. |
| US104 | [S32 / #174](https://github.com/liudesheng9/zet/issues/174) | As an owner, I want selection of each historical book and Lock disposition, so that pilot approval is not blanket backfill authority. |
| US105 | [S32 / #174](https://github.com/liudesheng9/zet/issues/174) | As an owner, I want one audited historical backfill with immutable prior prose, so that definitions are added without rerunning the book. |

## Verified publication fingerprints

These hashes describe the live issue bodies at verification, not future edited tracker state. Verification UTC: `2026-10-24T20:22:37.439Z`.

| Issue | Body SHA-256 | Native parent | Native blockers |
|---|---|---|---|
| [#142](https://github.com/liudesheng9/zet/issues/142) | `730a0721b0fba05e83faf4139cd190bae8370d291f3d1c1d0e7ca6207ecc5d8a` | None | None |
| [#143](https://github.com/liudesheng9/zet/issues/143) | `44456b74d163e5d7684ca2f3ce6076e4ef8f8cc52a1ec7e12f489cf9c25dee36` | #142 | None |
| [#144](https://github.com/liudesheng9/zet/issues/144) | `4289b125c4579b8eda71f0d19f2ef4c04caadfd7360fc88245c840b93e1c08e3` | #142 | #143 |
| [#145](https://github.com/liudesheng9/zet/issues/145) | `7342096ce2b24536a1b52fb0a62b8ee51dabc24bcf9ee7ca62ab52c0662646cd` | #142 | #144 |
| [#146](https://github.com/liudesheng9/zet/issues/146) | `f80dc19135194f476bb15669f61e17344837855c593c60608aa8be0061bbcd54` | #142 | #145 |
| [#147](https://github.com/liudesheng9/zet/issues/147) | `c2fade6907123cea41a9a517989e84bdfeafd2cc2a6d335b2dfc42eb8fbecbec` | #142 | #146 |
| [#148](https://github.com/liudesheng9/zet/issues/148) | `3513e52723eee0e333f61b11e6590f3c089c53b755ef99fe230fc5da6a6ff4b0` | #142 | #147 |
| [#149](https://github.com/liudesheng9/zet/issues/149) | `005a1c44e34ac86a9efa1105069e6187c36506ff26ddf6a900a37b4d0d7e81ea` | #142 | #147 |
| [#150](https://github.com/liudesheng9/zet/issues/150) | `835e699f9708b9502135c7325cc523ebac2e98259abdc6d386420e7585aadef8` | #142 | #149 |
| [#151](https://github.com/liudesheng9/zet/issues/151) | `f2456b51b03d08f32ae45b00bce26e5afad63fcf95edf7abe6bed8e89594fb54` | #142 | #150 |
| [#152](https://github.com/liudesheng9/zet/issues/152) | `9a43dd916e937679c4b36ff64ac019b199500c56624982d4583dd0ab564b6545` | #142 | #151 |
| [#153](https://github.com/liudesheng9/zet/issues/153) | `748c77de12a4e3928ad3f23671ffcfaa7e66af35b47c4065efe3b1b5cca73f1c` | #142 | #151 |
| [#154](https://github.com/liudesheng9/zet/issues/154) | `a91bc4397e6b4073481375768fc10db3af46737d10b49c8ad7cb4bca9f3e2893` | #142 | #153 |
| [#155](https://github.com/liudesheng9/zet/issues/155) | `47be2d2e345394c9a83aaf8f9e7628f6269c28fea29f74ee49f313b4400a4092` | #142 | #153 |
| [#156](https://github.com/liudesheng9/zet/issues/156) | `6d935d3c40c6d8dd62edeeba5a7d2db4b0d52b2508f09078fa75ec3ff025de3d` | #142 | #154 |
| [#157](https://github.com/liudesheng9/zet/issues/157) | `225c374633c0b7a97abc614f3698dd18f6478088bf0ff6b3ed0699da12c57e22` | #142 | #154 |
| [#158](https://github.com/liudesheng9/zet/issues/158) | `c832bce56e4004b94f8dc9d54d480b04c7c5662b6fd621847943d2d160797609` | #142 | #154 |
| [#159](https://github.com/liudesheng9/zet/issues/159) | `e40be8f5e6ef86a6c6985c4a195e63c9f2887bd11742f19d6593cc9869bde713` | #142 | #152, #154 |
| [#160](https://github.com/liudesheng9/zet/issues/160) | `8c36977163c9efb21ecf52035bf04ccece13a5b7660b16d2f26f313ca4a0344d` | #142 | #154 |
| [#161](https://github.com/liudesheng9/zet/issues/161) | `55a1babef8c71c8ce3eb6d2d5e6330bf2100a1f11465ff37be2be85fa1cdd3a5` | #142 | #156 |
| [#162](https://github.com/liudesheng9/zet/issues/162) | `34b5e91d713a6f69f4255befdbd7e4901378b937255a1b5e61faefafcce2a55f` | #142 | #159 |
| [#163](https://github.com/liudesheng9/zet/issues/163) | `72bfeed2b3b45ff4d4e7f8abffcf5fe82310219be30be327f768e0f239c25b57` | #142 | #148, #160 |
| [#164](https://github.com/liudesheng9/zet/issues/164) | `c81ce3fa817907e0d3ca3bf6d3f6b38fd225133942d1dee0ab9cef2f10a4149b` | #142 | #162 |
| [#165](https://github.com/liudesheng9/zet/issues/165) | `2c2ee10eb9aa2e794a248268fcf9e82426018eee59666ae4ee83ebfe784f177b` | #142 | #162 |
| [#166](https://github.com/liudesheng9/zet/issues/166) | `4c6911cc4a8153572ef234241edd0e913a72bf3ff778d5fcf6d5486ca712d33a` | #142 | #148, #162 |
| [#167](https://github.com/liudesheng9/zet/issues/167) | `e53276dd2cd13d1080eba91eeb3be1208348e6f0315738a9e422754e415a267d` | #142 | #163, #164, #165 |
| [#168](https://github.com/liudesheng9/zet/issues/168) | `86a30270fc48739a9c1a77549ec55dc580cfc454185c00c1c4f2fec2dbbeeed4` | #142 | #167 |
| [#169](https://github.com/liudesheng9/zet/issues/169) | `8e42c41d12d7fa15ce9f02e94fb37e077f932e2b74c8af4b3d0574a2665a9dd6` | #142 | #167 |
| [#170](https://github.com/liudesheng9/zet/issues/170) | `5fbe8701cae69ad0094ba040445c6b16bece251157aa3828d67487f2bf459fb4` | #142 | #155, #157, #158, #161, #166, #168, #169 |
| [#171](https://github.com/liudesheng9/zet/issues/171) | `058d645f84960cc684dc967d45e01f34925e4a419e0c898bd6d247729211c694` | #142 | #170 |
| [#172](https://github.com/liudesheng9/zet/issues/172) | `fb9317feec5c113bb006754d09b7dc7c0505345680d7f16235bf8bcfc28c6c26` | #142 | #171 |
| [#173](https://github.com/liudesheng9/zet/issues/173) | `bdb85cf6db5a771c70bcac7a5b5b9b2ce918192039f3f70625a5f6f64683f656` | #142 | #172 |
| [#174](https://github.com/liudesheng9/zet/issues/174) | `34cc06d9ae3460d749af712554852f3dc693ebd11985d3038859d0644faa77f2` | #142 | #173 |

Verification result: `0 errors`. No production code, service, workflow, archive, Lock, or prior issue was changed by this publication task.
