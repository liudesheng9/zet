# The Bridge Implementation: Review and Fixes

Status: review and fixes, 2026-10-16. Claude Code reviewed commit `3e066aa` ("Implement report-first cross-book Bridges with guarded apply", written by a Codex model). The review compared the code with `slip-box-luhmann-design.md` (C1 and C2) and with the implementing agent's specification `slip-box-luhmann-implementation-design.md`. This document lists what the review found, what it fixed, and what stays open. The text follows ASD-STE100 as the other design documents do.

## Short answer

- **C1 (the skill texts) is complete.** No text under `skills/` or in the installed copies says that ZET cannot insert a Card into a line. The whole-Card Link rule and the pointer to the decision documents are in place. The review fixed two small gaps in the Luhmann reference.
- **C2 (the Bridge stage) is implemented, with bugs.** The review found 12 bugs (F1 to F12) and 6 missing or partial features (F13 to F18). It also found 3 test and tooling faults, and stale text in 13 documents. This change fixes all of them. Each code fix has a test.
- **The principles of C2 hold.** The report comes first. Writes happen only with an explicit selection that names the report id. The new book carries the Link. Human Cards never carry a Link. No accepted path lets a Model Card take in a Model part or adds a rests-on closure fault.
- **Two items stay open, by decision.** Two steps need the user's go-ahead: a real Bridge pilot on a digested book (GitHub issue #62), and a live apply of accepted proposals (#63). This review ran no judge and changed no Card in the live archive.

## How the review worked

1. Three review agents read the code in parallel. One read the compiler (`bridge-compiler.ps1`, `book-of.ps1`). One read the workflow integration (module, blueprints, workflows, harnesses). One read the audit, the skills, and the documents.
2. Claude Code checked each finding in the code before fixing it. Findings that the code did not support were dropped.
3. The full digest test suite ran before and after the fixes. The suite uses scratch archives only.

## Feature coverage

The table checks each feature of the design against the code after the fixes.

| Design feature | State before the review | State now |
| --- | --- | --- |
| C1.1 "Insertion renumbers" in `SKILL.md` | Done | Done |
| C1.2 The mapping in `luhmann-and-practice.md` | Partly: the text lost the point that Luhmann filed 57/12a between 57/12 and 57/13 | Done (F21) |
| C1.3 The whole-Card Link and split rule | Done | Done |
| C1.4 The pointer to the decision documents | Partly: GitHub URLs that a local agent cannot open, no Z1 to Z12 note | Done (F22) |
| C1 rollout: installed skill copies | Done | Done again after the fixes: Claude Code, Codex, and DeepSeek Harness copies equal the repository |
| C2 New terms in `CONTEXT.md` | Done, with ids `b001` | Done |
| C2 Report first, apply after the go-ahead | Done | Done |
| C2 Place: after Figures, before Hubs, in all three paths | Done | Done |
| C2 Arguments and modes | Done, but the framework variants checked `bridgeMax` and `bridgeCandidates` only after a full digest | Done (F10) |
| C2.1 The compiler cycle | Done | Done |
| C2.2 Shared attribution in `book-of.ps1` | Done, with a changed tie rule and a Finding regression | Done (F1, F2) |
| C2.3 Source Cards (book and math profiles) | Done | Done |
| C2.4 Candidates: relatedness and term match | Bug: one term list for the whole book | Done (F3) |
| C2.5 The judge prompt and its questions | Partly: the five questions were missing | Done (F15) |
| C2.6 The answer format | Done (one result per source, as the specification decided) | Done |
| C2.7 Kind rules | Bug: Model Card carriers could use `印证` and `案例` | Done (F7) |
| C2.8 Direction | Done | Done |
| C2.9 The precheck | Bugs: spacing (F4) and R6 fallback (F8) | Done |
| C2.10 The report | Partly: no book title, no kinds per book, no source titles | Done (F16) |
| C2.11 Go-ahead and apply | Bugs: F5, F6, F9, F12 | Done |
| C2.12 The audit count | Partly: no islands in `audit.md`, "island" for a book with nothing to link | Done (F13) |
| C2.13 Roles, models, limits | Bug: cleanup could be skipped (F11) | Done |
| C2.14 Failure policy | Bugs: a failed run froze an empty report (F14), a failed stage showed as done (F17) | Done |
| C2.15 Module, blueprints, versions | Done | Done, new versions below |
| C2.16 Tests | Partly: one test needed Python, which this machine does not have | Done (F19), with new tests |
| C2.17 Acceptance (real pilot) | Open: #62, #63 | Open, needs the user's go-ahead |

## Bugs fixed

| No. | Where | Fault | Failure scenario | Fix | Test |
| --- | --- | --- | --- | --- | --- |
| F1 | `digest-audit.ps1`, `card-compiler.ps1` | The new `finding` category dropped Finding Cards out of `no_source`, `unkinded`, the inbound kinded count, and the card fix cycle. Before the commit they were read as Idea Cards. | The card cycle refused every Finding Card ("category finding is not writable"). The audit stopped checking their source lines. | `finding` is back in those checks. The card cycle treats it as writable, and its F4 orphans get Idea Card candidates. A Finding Card's kinded Link counts toward a target, except toward another Finding Card. `dsh-card-instructions.zh.md` gets F1 to F3. | `digest-audit.tests.ps1` |
| F2 | `book-of.ps1` | The attribution tie rule changed from the old case-insensitive order to an ordinal order, with no note. | With tied votes, `aghion2000` and `Hart1995` swapped owners. | The tie rule is the old one again (`Sort-Object`). | `book-of.tests.ps1` |
| F3 | `bridge-compiler.ps1` plan | The term match used one term set for the whole book, so every source got the same two term candidates. The loop ran sources × all Cards × terms in PowerShell. | A math unit `罗尔定理` never got the economics Card that names it. Every unit got the two titles with the most book terms. | Per source: the book terms in the source's title or excerpt, and the source's model or unit name. Each term is matched once against every Card. | `bridge-compiler.tests.ps1` |
| F4 | precheck BR5 | BR5 needed a space between the kind word and the Link. ZT and the prompt accept none. | `对应[[x]]：…` was refused three times, then lost. | BR5 accepts zero or more spaces. The prompt shows one example sentence. | `bridge-validation.tests.ps1` |
| F5 | apply, reconcile | A saved but unwritten intent with ids outside a new selection was marked `conflict`, but its body was still written. | Run 1 accepts `b001,b002` and stops before the edit. Run 2 accepts `b001`. The carrier still got both paragraphs. | An unwritten intent that the new selection does not match is dropped (`abandoned`, its ids `open`). A written one is verified. Any other state is a conflict that blocks the carrier. | compiler and retry tests |
| F6 | apply, after the write | The L1 and L2 check of a changed carrier did nothing for Model Cards: `zt-check` runs these rules only for Idea, anchor, Event, and Finding drafts. | The most common carrier was never checked after a write. | Both drafts go to `zt-check` as Idea drafts, as BR9 already did. | compiler tests |
| F7 | precheck BR4 | A Model Card carrier could use the evidence words `印证` and `案例`. Design C2.7 excludes them. | A model "illustrated" another book's idea as if it were evidence. | BR4 refuses them for a Model Card carrier. | `bridge-validation.tests.ps1` |
| F8 | precheck R6 | When a model's setting had no `术语表`, R6 had no table at all. | Wrong terms passed with `term_check: unavailable`. | R6 falls back to the carrier book's term table. | `bridge-validation.tests.ps1` |
| F9 | apply, verification | Model faults were compared with the report-time snapshot. | A part added to the model between report and apply blocked the carrier as `written_unverified` for good. | The apply step stores the model shape just before the edit and compares with it. | compiler tests |
| F10 | blueprints | The framework variants checked `bridgeMax` and `bridgeCandidates` only in the module. | `bridgeMax: 1.5` ran a whole digest, then the Bridge stage failed. | The gate checks both before any work. The base blueprint is 2.1.3. | `bridge-workflow-harness.mjs` |
| F11 | `zt-bridge.js`, `zt-book-digest.js` | The cleanup ran only inside the report call. | When the report call threw, up to ten judge conversations and the DeepSeek workspace stayed. That breaks the "delete after use" rule. | A separate cleanup call runs in its own `finally`. The library exports `cleanupDsh`. | all three harnesses |
| F12 | apply | A carrier blocked by an earlier conflict skipped newly accepted ids with no outcome. | The summary said "Not accepted or applied" for an id the user accepted. | Each such id gets the outcome `blocked`, with the reason. | compiler tests |

## Missing features supplemented

| No. | Where | What was missing | What it does now | Test |
| --- | --- | --- | --- | --- |
| F13 | `digest-audit.ps1` | `audit.md` did not name islands. A book with no Card outside it was an "island". | The Agent Workspace table has an "Islands" row. Each book has a readable Bridge line. `island` is null, with `peers: 0`, when no substantive Card outside the book exists. | `digest-audit.tests.ps1` |
| F14 | `zt-bridge.js`, `zt-book-digest.js` | A failed plan or judge job still published a frozen report, so a rerun on the same `out` could not resume. | The report is published only after every judge job answered. The failed result says to rerun mode `bridge` with the same `out`. | all three harnesses |
| F15 | `dsh-bridge-instructions.zh.md` | The five questions of C2.5. | The prompt asks: same mechanism, same concept, evidence, tension, and (math profile only) use. | prompt review |
| F16 | `bridge-compiler.ps1` report | The book title, the kinds per target book, and source titles. | `report.md` shows all three. | compiler tests |
| F17 | base blueprint, `zt-digest-report.js` | A failed Bridge stage showed as `done` to the kernel and as `null` in the result. | The stage expects `status != failed`. The run result keeps `{status: failed}` in mode `full`. | `framework-harness.mjs` |
| F18 | `zt-digest-lib.js`, `zt-book-digest.js` | The judges' approval list mixed the read-only rules with the rules of other compilers. It did not allow the helper with the empty allow-list that the prompt names. | One exclusive read-only list: the Bridge precheck, `zt-check`, the helper with `read-only.allow.txt`, reads under `out`, and JSON writes under `bridge\out`. | `bridge-workflow-harness.mjs` |

## Test and tooling faults fixed

| No. | Where | Fault | Fix |
| --- | --- | --- | --- |
| F19 | `bridge-human.tests.ps1` | The test needed `-Python`. This machine has no Python. | The fixture seeds its scratch database with `node:sqlite`. `-Python` is accepted and not used. |
| F20 | `zt-econ-digest.sim.json`, `zt-math-digest.sim.json` | The dry runs had no answer for the judge jobs and no `step` in the report summary. | The sims answer the judge jobs and the summary carries `step`. The assembler's install dry run passes. |
| — | `bridge-compiler.ps1` summary | The summary line repeated every outcome (tens of kilobytes) for a Haiku agent to copy. | The line carries `outcome_counts`, and `outcomes` only when there are at most 30. `report.md` and `state.json` hold the rest. |

## Stale documents fixed

| No. | Document | Fix |
| --- | --- | --- |
| F21 | `skills/zettelkasten/references/luhmann-and-practice.md` | The 57/12a point and `insert_card before` in "Internal branching" and in the mapping. |
| F22 | the same | Repository paths instead of GitHub URLs. The Z1 to Z12 note. |
| — | `docs/zt-book-digest-workflow.md` | The stage diagram, the stage table, the modes, the seven Bridge arguments, and the audit section. |
| — | `docs/wiki/digest-workflows.md`, `digest-scripts.md`, `digest-prompts.md`, `agent-skills.md` | Bridge rows in the stage, mode, argument, and interface tables. The text "apply is under implementation" is replaced. The Bridge contract moved out of the Purpose sections. Change history lines. |
| — | `docs/zt-digest-framework.md` | "Bridge discovery and apply" is its own section, not part of the economics module set. |
| — | `skills/zt-digest-protocol/SKILL.md`, `references/scripts.md` | Bridge in the stage diagram and the Audit row. The publish and cleanup rules. |
| — | `CONTEXT.md`, `slip-box-luhmann-design.md` | Ids are `b001`, `b002`. "suggestion" is an _Avoid_ word again. |
| — | `docs/math-cards-luhmann-comparison.md` | L6 is carried out by C2 with the math profile. |
| — | `docs/slip-box-luhmann-implementation-design.md` | Section 3 records the narrowed Model Card words and the id format. |

## Deviations from the design that stay

The implementing agent changed some features on purpose. The review accepts these, because each one is safer than the design text:

- **No title relocation.** Design C2.11 said that apply finds a moved Card by title. A title is not unique, so apply refuses a stale endpoint and the user prepares a fresh report.
- **Only supplied candidates.** Design C2.6 let the judge find a target with `search_cards`. Version 1 accepts only the planned candidates, so a plan stays bounded and checkable.
- **`edit_card` with `base`, not `append_card`.** The helper's `append_card` does not carry `base`. Apply sends `edit_card` with the carrier's raw text, so ZT refuses an edit to a Card that changed.
- **A report id on apply.** The arguments add `bridgeReport`, so `b001` or `"all"` cannot point to a different report after a rerun.
- **One result per source.** The answer holds one result per source with zero to two Bridges. So "every source appears once" and "two Bridges" both hold.

## The report-only pilot (#62), 2026-10-16

At the user's request, Claude Code ran the installed workflow on one digested book. The run applied nothing. This record holds counts and ids only. The Card text stays in the local report.

| Item | Value |
| --- | --- |
| Command | `Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-econ-digest.js", args: { mode: "bridge", out: "D:\\zt-digest\\bridge-pilot\\hart1995firms", citationKey: "hart1995firms", bridgeMax: 12, bridgeCandidates: 5 } })` |
| Installed workflow | `zt-econ-digest` 3.1.3, fingerprint `f58e668a16054da5` |
| Stages run | args, source (DeepSeek official), bridge, report. No prepare, digest, link, organize, hubs, or models stage ran. |
| Plan and report | `p-f449f647…b208`, `r-4cc98a89…311c` |
| Read coverage | complete: 149 trees, 8,052 Card summaries, 67 endpoints, 2 term tables, 27 model settings |
| Sources | 295 eligible, 12 selected (all Model Cards of the book: formal before concept), 283 left out by the cap |
| Judge | 1 job (`br-001`), 1 DeepSeek conversation, 0 faults after its self-check |
| Proposals | 16, all carried by the new book's Cards (`b001` to `b016`). 0 for earlier Cards. 0 refused. 11 sources with a proposal, 1 with a reasoned "none". |
| Kinds | 9 `contrasts` (对照), 7 `corresponds` (对应) |
| Target books | `mascolell1995microeconomic` 11, `missingmarkets2026interviews` 4, `grossman2002integration` 1 |
| Card changes | none: a SHA-256 over every Card (location, type, Workspace, full text) was identical before and after the run |
| Cleanup | the cleanup agent deleted the judge conversation, and the DeepSeek workspace of the run no longer exists |
| Time | about 7 minutes, 5 agents |

Observations for the next step:

- **The proposals need a human read.** Some name a specific shared mechanism, for example two definitions of ownership or the same relation-specific investment mechanism in another field. Others are broad analogies, for example "both use a threshold condition". The report gives each sentence and reason, so the user can accept ids one by one.
- **The source cap took only Model Cards.** With `bridgeMax: 12`, the formal and concept models fill the cap. The book's Topic-tree Idea Cards did not get a turn. A larger `bridgeMax` reaches them.
- **The report is long.** The list of sources left out by the cap has 283 rows. A later version can shorten that section.

## The first live apply (#63), 2026-10-16

The user accepted 7 proposals from report `r-4cc98a89…311c`: `b001`, `b006`, `b008`, `b012`, `b013`, `b015`, `b016`.

| Step | Result |
| --- | --- |
| Backup | `D:\zt\backups\zt.sqlite3.pre-bridge-apply-20261016T224911Z` by `VACUUM INTO`: integrity `ok`, 8,052 Cards, SHA-256 `3e9d8f11…04a9` |
| Preview (apply without `-Apply`) | all 7 `ready`, no state file written |
| Apply (`zt-econ-digest` 3.1.3, mode `bridge`) | 7 `verified`, 9 `open` (not accepted). 6 carriers changed: `5|a` carries two. Stages run: args and bridge only, no source probe. |
| Row diff against the backup | exactly 13 rows changed: 6 carriers gained only the accepted `跨书关联：` paragraphs (earlier text unchanged), and 7 targets changed only in their generated Reverse links. No Card added, removed, or retyped. |
| Relation kinds (`get_model` neighbours) | all 7 as accepted: 2 `contrasts`, 5 `corresponds` |
| Reverse links | each target lists its carrier |
| Models of the 6 carriers | all formal, all self-sufficient, 0 `closure`, `forward`, `unanchored`, `beyond_level` faults |
| `zt-check` on the 6 carriers, before and after | 0 faults before, 0 after (L1 to L3, R2 to R5, R7, R9) |
| Repeat apply | `done`, no paragraph added again |

The apply took about 7 minutes for 6 Cards. Most of the time went to whole-archive reads: the apply step reads every tree before and after each carrier. A later change can read only the endpoints and the tree index once (see "What stays open").

## What stays open

- **#62, a real report-only pilot.** Done on 2026-10-16 (see above). The issue can close when the user agrees.
- **#63, a live apply.** Done for 7 proposals on 2026-10-16 (see above). The other 9 proposals stay open.
- **Apply reads the archive once per carrier.** The first live apply showed the cost: about 7 minutes for 6 Cards. `Current-Context` reads every tree for each carrier. With 60 carriers that is about 60 full reads. It gives the right result, but it is slow. A later change can read only the endpoints.
- **The sims dry-run mode `full` only.** The Bridge harness covers mode `bridge` with discovery, apply, and failures. A mode `bridge` case in the sim files is left for a later version.

## Versions and installation

| Item | Before | Now |
| --- | --- | --- |
| Base blueprint `zt-digest-base` | 2.1.2 | 2.1.3 |
| `zt-econ-digest` | 3.1.2 | 3.1.3 |
| `zt-math-digest` | 2.2.2 | 2.2.3 |
| Module `zt-bridge` | v3 | v4 |
| Module `zt-digest-lib` | v3 | v4 |
| Module `zt-digest-report` | v4 | v5 |

- The assembler installed both variants (project scope) and `verify` returned `ok: true`, with no problems and no library drift.
- `make-math-workflow.mjs` rebuilt the legacy math workflow from `zt-book-digest.js`.
- The skills `zettelkasten`, `zt-digest-protocol`, and `zt` were mirrored to `~/.claude/skills` and `~/.codex/skills`. DeepSeek Harness reads the Codex copies. A hash comparison found no difference.
- The zt service runs 0.20.0 from `~/.cargo/bin/zt.exe` on `D:\zt`. The review changed no Rust code, so the service needs no restart.

## Verification

All commands run from `E:\zet`. The PowerShell tests use scratch archives with `ZT_GUI_PORT=0`.

| Command | Before the fixes | After the fixes |
| --- | --- | --- |
| `pwsh -NoProfile -File scripts/book-digest/tests/bridge-compiler.tests.ps1` | pass | pass, with the per-source term test |
| `pwsh -NoProfile -File scripts/book-digest/tests/bridge-validation.tests.ps1` | pass | pass, with 4 new checks (no space, Model Card evidence word, Model Card correspondence, reverse pair) |
| `pwsh -NoProfile -File scripts/book-digest/tests/bridge-human.tests.ps1` | fail: needs `-Python` | pass, no Python |
| `pwsh -NoProfile -File scripts/book-digest/tests/bridge-math.tests.ps1` | pass | pass; the fixture no longer proposes both directions of one pair |
| `pwsh -NoProfile -File scripts/book-digest/tests/bridge-planning.tests.ps1` | pass | pass |
| `pwsh -NoProfile -File scripts/book-digest/tests/bridge-attribution-retry.tests.ps1` | pass | pass |
| `pwsh -NoProfile -File scripts/book-digest/tests/digest-audit.tests.ps1` | pass | pass, with 4 new checks (Finding checks, no-peer island, islands in `audit.md`) |
| `pwsh -NoProfile -File scripts/book-digest/tests/book-of.tests.ps1` | pass | pass |
| `pwsh -NoProfile -File scripts/book-digest/tests/card-compiler.tests.ps1` | pass | pass |
| The other 13 `scripts/book-digest/tests/*.tests.ps1` files | pass | pass |
| `node scripts/book-digest/tests/framework-harness.mjs` | pass | pass, 0 failed, with 3 new checks |
| `node scripts/book-digest/tests/workflow-harness.mjs` | pass | pass, 0 failed |
| `node scripts/book-digest/tests/math-workflow-harness.mjs` | pass | pass, 0 failed |
| `node scripts/book-digest/tests/bridge-workflow-harness.mjs` | pass | pass, with failed-judge and failed-plan cases |
| `node <assembler> verify zt-econ-digest` and `zt-math-digest` | ok | ok, no problems, no library drift |
| `node ~/.claude/skills/project-wiki/scripts/wiki.mjs check` | current | current |

The review changed no Rust code, so it did not run `cargo test`. The harnesses first failed after the fixes because their stand-ins lacked the `step` field and used the old cleanup label. Those stand-ins now follow the new contract.
