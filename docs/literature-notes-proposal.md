| L7 / [#190](https://github.com/liudesheng9/zet/issues/190) || L6 / [#189](https://github.com/liudesheng9/zet/issues/189) || L5 / [#188](https://github.com/liudesheng9/zet/issues/188) || L4 / [#187](https://github.com/liudesheng9/zet/issues/187) || L3 / [#186](https://github.com/liudesheng9/zet/issues/186) || L2 / [#185](https://github.com/liudesheng9/zet/issues/185) || L1 / [#184](https://github.com/liudesheng9/zet/issues/184) |# Faithful Literature notes: proposal and design

Status: proposal (2026-10-26), published as PRD [#183](https://github.com/liudesheng9/zet/issues/183) with slices #184–#190. Scope: the Literature-tree Cards that the digest workflows (`zt-digest-base` 5.7.1, `zt-econ-digest` 6.7.1, `zt-math-digest` 5.7.1) write. The proposal changes no Topic tree and no part of `zt-idea-grow`. Every change follows the module structure in [workflow-module-structure](wiki/workflow-module-structure.md) and the rules in [workflow-principles](agents/workflow-principles.md).

## 1. Problem

The user's verdict on the current Literature Cards is that they are hard to understand and do not cover what the material holds. The live trees confirm this. The evidence below comes from `acemoglu2025nobel` (archive revision 7982, 238 Cards: 87 Idea, 63 Model, 7 Finding, 2 Event, 79 Term) and its staging folder `D:\zt-digest\acemoglu2025nobel-terms`.

### 1.1 The digest applies the Idea-card standard to reading notes

The digest prompt (`scripts/book-digest/dsh-digest-base.zh.md`) and the checker treat a Literature Card as a Zettelkasten Idea Card:

| Rule | Where | Effect on a reading note |
|---|---|---|
| "本组约写 6–12 张卡，宁少而精" | digest prompt | A group of two or three sections gets at most about 12 Cards, whatever the material holds. |
| Body "约 80–200 字"; I2 blocks above `max_idea_chars` 300 | prompt; `zt-check` `Limits` | A passage of 600–1,500 characters is compressed into 300 characters or less. Examples, numbers, caveats and the steps of an argument drop out. |
| "一卡一观点", I1 claim title, I6 near-duplicate titles, I7 standalone claim | prompt; checker | The writer cuts the book's argument into separate claims and loses the thread that joins them. |
| STE rules R2 (40 characters per sentence), R3 (no semicolon), R4 (no passive), R5, R7, "一句只说一件事" | prompt; checker blocks them | The prose reads as a list of short statements ("靠近赤道的国家人均 GDP 更低。这些国家多是前欧洲殖民地。"). It has no connectives, so it is not fluent, and the reader must rebuild the logic. |
| I4/L1/L2: every Link needs a relation word and a reason | checker blocks them | Writers add Link sentences after the source line to satisfy the rule ("这次行动依赖 [[…/3]] 的框架：两种机制都来自该框架。"). These sentences say nothing the book says. |
| D1: an Idea Card that restates a Model part | card cycle | The same result appears twice (for example `/4|d` and `/4|a|a|22`, `/5|b` and `/5|a|a|5`). The writer either duplicates the model or adds an invented "angle". |
| At most 7 commands per job, "不要通读全书" | digest prompt | The writer reads the sections of its own group only by search hits and context windows. It does not read them in full. |

### 1.2 Coverage is not measured

Each `create_successor` call carries a `src` field, and `zt-digest call` logs it to `src-gNN.txt`. In the Nobel run 141 of 212 logged spans name only a file (`sections/s03.md`) and give no line range, so coverage cannot be computed from them. Where line ranges exist, the Cards cover 233 of 381 substantive paragraphs (61%; 64% of the characters). No stage reports coverage, and no job learns which passages it left out.

### 1.3 Faithfulness is checked by sampling only

The card cycle checks style faults on every Card. It checks faithfulness (`unfaithful`) only for a Card whose task file includes the source lines, and those are sampled. A number, a date or an attribution that the book does not support passes when the Card is not sampled.

## 2. Goal

A Literature tree is a faithful, complete and readable set of reading notes on one source:

1. **Coverage.** Every substantive paragraph of the source is covered by at least one Card (an Idea note, a Model part, a Finding or an Event), or it is listed as skipped with a reason from a fixed list.
2. **Fidelity.** Every statement on a Card is supported by the source lines that the Card names. The Card keeps the author's attributions, hedges, numbers, examples and caveats. It adds no claim that the source does not make, unless the Card marks it as an explanation.
3. **Comprehensibility.** A reader who has not read the book understands the Card. The Card explains each term and symbol at its first use, or links its Term Card. It explains the argument step by step in the book's order.
4. **Fluency.** The prose is connected academic Chinese. Sentences use connectives (因为、因此、但是、换言之), and paragraphs follow one line of thought. The text has no hype words and uses one word for each term.
5. **Relations as in the source.** The tree follows the source's own order and relations. The reading thread runs as Direct successors. Examples, digressions, footnotes and the author's critiques of other work are Side successors. A Link goes where the source itself connects two passages, and it is written inside the sentence that states the connection.

The Idea-card standard (one idea per Card, claim titles, short standalone statements, STE sentence limits) remains the standard for Topic trees. Literature notes do not use it.

### Non-goals

- No change to Topic trees, hubs, the register or `zt-idea-grow`.
- No change to Model Cards, Model parts, Term Cards, Event Cards or Finding Cards, or to their checks.
- No new workflow stage and no new Claude role.
- No rerun of a whole digest to test the change.

## 3. Design

### 3.1 Principle

The design adds a profile and a measurement. It adds no orchestration:

- **Check profile.** `zt-check` gets one Card category, the *Literature note*, with its own applicability. The Idea-card rules do not apply to it. The faithfulness-related rules (source line, formulas, figures, Terms) still block.
- **Measurement.** `zt-digest coverage` is a deterministic command. It maps the source paragraphs to the Cards' line spans and checks the numbers on each Card against its span. It is the self-check of the jobs that write notes, and it is the data the report shows.
- **Content.** The writing prompt and the review-and-fix prompt change to the Literature-note standard. The existing jobs keep their shape: one DeepSeek job per group writes, and one DeepSeek job per group reviews, fixes and fills gaps.

The module rules hold without exception: a Haiku manager runs at most 8 DeepSeek conversations; each deterministic chain is one script step run by a drive job; each content job runs its own self-check; each module is a straight line; the ledger is the truth; each step answers one line of at most 400 characters.

### 3.2 The Literature-note check profile (`zt-check`)

A **Literature note** is a Regular Card of type `idea` in a Literature tree of the agent Workspace, other than the tree's root. The anchor Cards of the sections are Literature notes too. Event, Finding, Model and Term Cards keep their categories and their rules.

The applicability version goes from 2 to 3. For a Literature note:

| Rule | Idea Card (unchanged) | Literature note |
|---|---|---|
| I1 claim title | advise | off |
| R12 title | block (one claim, ≤40) | advise: title ≤60 characters, one line, no `；` |
| I2 size | block above 300, advise below 40 | advise below 80 only; no upper bound |
| R2 sentence length, R3 semicolon, R4 passive, R5 nominalization, R7 `的` count | block | off |
| R9 hedging and praise words | block | advise |
| I6 near-duplicate title, I7 standalone claim, D1 restated Model part | as now | off |
| I4/L1 relation word before a Link, L2 Link reason | block | advise |
| L3 one target linked many times | advise | advise |
| R20 numbers instead of content, R21 unnamed reference | advise | advise |
| I3 source line, R22 unclosed formula, F1–F4 figures, T1–T9 Terms, E/F rules | as now | as now |

The policy hash changes, so the service's check contract changes: `zt-check` 0.15.0 and `zt` 0.42.0. The human Workspace keeps its policy, and its Literature notes use the same applicability rows at its own levels.

### 3.3 The coverage command (`zt-digest coverage`)

```
zt-digest coverage -Out <staging> [-Group N] [-Draft <file>] [-Step]
```

- **Units.** The command splits each staged section file (`sections/sNN.md`) into paragraph units. A unit is a non-blank block of lines with at least 20 characters. A heading or a figure line (`![…](artifact:…)`) is not a unit. A display formula belongs to the paragraph before it, and a table is one unit.
- **Spans.** It reads every source log of the run: the digest logs (`src-gNN.txt`), the event and finding logs (`events-src-gNN.txt`, `findings-src-gNN.txt`), and the `src` spans of each Model part in the model plans (`models/m-*`). A span that gives a line range covers each unit it overlaps. A span that names only a file covers nothing and is reported as fault `C2`.
- **Skips.** A unit is skipped when a job lists it with a reason from the fixed list: `致谢`, `参考文献`, `目录`, `版式`, `重复` (the same text appears in another unit, which must be named). A skip with another reason is fault `C4`.
- **Numbers.** For each Literature note, Finding and Event, it collects the numbers in the body outside formulas: integers, decimals, percentages and years. Each number must occur in the text of the Card's spans. A missing number is advisory fault `C3`, because a derived number (a difference or a ratio) can be correct.
- **Output.** With `-Group N` (the self-check form) it prints one fault per line, `sNN.md:120-128 C1 uncovered "首句前 30 字…"`, and the last line is `faults: N`. `C1` and `C2` count as faults. `C3` and `C4` lines carry `(advisory)` and `C4` counts. With `-Draft <file>` it reads a review-and-fix draft and counts the Cards the draft adds, so the job can check its own gap fill before the apply step. With `-Step` it writes `coverage.json` (per section: units, covered, skipped, uncovered, the covering addresses; per Card: its span and any C3 lines) and prints one compact line: `{"ok":true,"units":381,"covered":372,"skipped":6,"uncovered":3,"pct":99,"detail":"…\\coverage.json"}`.

The command reads files only. It changes no Card and no ledger row, and the same input gives the same output.

### 3.4 Writing the notes (digest group job)

The group job keeps its place (`digest`, module `zt-group-cycle`), its manager, its write path (`zt-digest call`) and its reply line. Its prompt `dsh-digest-base.zh.md` is rewritten to the Literature-note standard:

1. **Read the group's sections in full.** The task file already attaches them. The command budget goes from 7 to 12 commands so that the job can read and write in several parts.
2. **Write one note per passage.** A passage is one step of the author's text: a paragraph or a few paragraphs that make one argument, give one example, define one thing or review one body of work. The material sets the number of Cards. The prompt has no Card count and no upper size bound. A body is usually 200–1,200 characters.
3. **Title.** The title says what the passage says, in at most 60 characters. It may be a statement ("地理假说难以解释殖民地之间的制度差异") or a topic phrase for a review passage ("作者对地理假说的三点反驳").
4. **Body.** The body is fluent, connected prose in the author's order. It keeps the author's attributions ("作者认为", "Acemoglu 与 Robinson 指出"), hedges, examples, numbers and caveats. It explains each term and symbol at its first use. An explanation that the source does not give is marked `（解释）` at the start of its sentence. Long paragraphs are split by line of thought, not by a character count.
5. **Source span.** Each call carries `src` with line ranges (`sections/s05.md:120-160,171-175`). The coverage check reports a span that names only a file.
6. **Placement.** The reading thread is a chain of Direct successors under the section anchor. An example, a digression, a footnote, a critique of other work or a reply to an objection is a Side successor of the note it belongs to.
7. **Models, findings, events.** A passage that a model compiles is covered by its Model parts. The note for that passage states, in prose, what the passage does and why the author needs it, and it links the Model Card ("该推导见 [[模型卡]]"). It does not restate the derivation step by step. The same applies to Findings and Events: the note gives the context and links the Card.
8. **Links.** A Link goes where the source connects two passages ("作者在第二部分回到这个问题"), and the relation is stated in the same sentence. The prompt removes the rule that every Link needs a relation word before it. It also removes the rule that a continuing Card must state its relation in its first sentence.
9. **Anchor.** The anchor's title states what the part holds. Its body is a summary of the part in prose (what the part asks, how it argues and what it concludes, in 3–8 sentences). An index follows, one line per section. A last paragraph, "在全书中的位置：", says which parts it follows from and leads to.
10. **Language.** The prompt no longer loads `asd-ste100-zh`. It gives five style rules: one word for each term (`terms.md`); no hype words; connectives instead of bare lists of short sentences; numbered lists only for three or more parallel steps or conditions; formulas, quotations and figure lines kept verbatim.
11. **Self-check.** The `SELF-CHECK` line runs both checks: `zt-check` on the job's Cards (Literature-note profile) and `zt-digest coverage -Group N`. The job fixes faults until both print `faults: 0`, at most three times, as now.

### 3.5 Review, fix and fill (card layer job)

The card layer (`cards`, module `zt-cards-layer`, script `layer-steps.ps1 -Layer cards`) keeps its three steps (start, apply, finish) and its one job per group. The job changes from a style fix to a fidelity review that also fixes the Cards and fills the gaps:

- **Task file.** The start step writes, for each Card of the group, its title, its body, its span and the text of those lines. It also writes the group's uncovered units from `coverage.json`, the C3 lines, and the checker faults.
- **One job.** The job reviews each note against its lines: is every statement supported, is anything important in the lines missing, is the prose clear and fluent, are the terms and symbols explained. It writes one verdict per Card: `ok`, `fix` (a new title and body), `unfaithful` (a corrected title and body, with a note on what differed) or `merge` (the same passage twice; the apply step only records it). For each uncovered unit it writes an `add` entry (title, body, span) or a `skip` entry with a reason from the fixed list.
- **Self-check.** The existing card precheck, plus `zt-digest coverage -Group N -Draft <file>`. The job iterates until both print `faults: 0`, at most three times.
- **Apply.** The apply step writes the fixes as now. It creates each `add` Card as a Side successor of the note that covers the unit just before the gap, or of the section anchor when no note does. This is the placement path the `split` verdict already uses. It records the skips in `coverage-skips-gNN.json`, which the coverage command reads.
- **Prompt.** `dsh-card-instructions.zh.md` is replaced by the review, fix and fill instructions. The STE fault list (R2, R3, R4, R5, R7, I1, I2, D1, I4, L1, L2) is removed.

The fix rounds (`cardFixRounds`) and the one DSMP retry stay as they are. The module stays a straight line.

### 3.6 Report

The report module (`zt-digest-report`) reads `coverage.json` and adds `coverage: {units, covered, skipped, uncovered, pct}` to the run result. The audit lists the uncovered units and the C3 lines as findings. The audit does not fail the run on them, and a later `mode cards` run fills the gaps.

### 3.7 Versions and release

| Part | Change | Version |
|---|---|---|
| `zt-check` | applicability 3, Literature-note rows | 0.15.0 |
| `zt` | check contract with the new policy hash | 0.42.0 |
| `zt-digest` | `coverage`; card cycle `add`/`skip` verdicts; embedded contracts | 0.30.0 |
| `zt-digest-base` | prompts, card-layer params, report field | 5.8.0 (minor: params and module behaviour; no stage or data-flow change) |
| `zt-econ-digest`, `zt-math-digest` | rebuilt on the base | 6.8.0, 5.8.0 |

A minor blueprint bump makes the prepared runs of 5.7 non-drivable. Old staging folders are adopted with `zt-digest ledger adopt -Variant econ -Apply` on a copy, as in the Term backfill. Both scopes (project and global) are installed and verified, the DSMP block included. The harnesses (`framework-harness.mjs`, `layer-steps.tests.ps1`) and the wiki docs (`zt-check.md`, `zt-digest.md`, `digest-workflows.md`, `digest-prompts.md`) change in the same commits.

### 3.8 Testing and acceptance

- Unit tests: the checker's applicability (one test per changed row); the coverage command on fixtures (units, spans, file-only spans, skips, numbers, drafts); the card cycle's `add` and `skip` apply.
- Pilot (no full rerun): a copy of the archive (`D:\zt-pilot-lit`) and an adopted copy of the Nobel staging folder. Run `mode cards` there, which runs only the card layer, the audit and the report. Acceptance on the pilot:
  1. coverage of at least 95% of the units, with every skip carrying a listed reason;
  2. no blocking fault of the Literature-note profile;
  3. every C3 line read and resolved by the job (corrected or explained);
  4. a read-through of ten sampled notes against their source lines finds no unsupported statement;
  5. the Model, Term, Event and Finding Cards are unchanged, except for the Link targets of merged notes.
- The full digest prompt (§3.4) is first exercised on the next new book. That run is the pilot of the writing stage, as the user's rule requires.
- The backfill of the five live books (`mode cards` on adopted copies of their staging folders) waits for the user's go-ahead after the pilot.

## 4. Risks

| Risk | Answer |
|---|---|
| Longer notes cost more DeepSeek tokens. | DeepSeek does the labour. The Claude cost does not change: no new manager, stage or agent. |
| Notes may repeat Model parts. | Section 3.4 rule 7 links the Model Card instead of restating it. The coverage command counts Model spans, so the note does not need to repeat a derivation to cover its lines. |
| A job skips a unit to pass the check. | Only listed reasons count. `重复` must name the other unit, and the audit lists every skip. |
| Number checks give false alarms. | C3 is advisory. The job resolves each line, and the report counts the lines that remain. |
| `zt-idea-grow` reads the longer notes. | It reads Literature trees through `get_tree`/`get_cards` and chunks by Card, so a longer Card is more context, not a format change. |
| Rewriting live Cards moves Links. | Fixes keep every `[[…]]` and Term link, as the card cycle requires now. Addresses do not change, and `merge` is only recorded. |

## 5. Slices

| Slice | Delivers | Blocked by | Type |
|---|---|---|---|
| L1 | `zt-check` Literature-note category and applicability 3; `zt` 0.42.0 contract | none | AFK |
| L2 | `zt-digest coverage` (units, spans, skips, numbers, `-Group`, `-Draft`, `-Step`) | none | AFK |
| L3 | The digest group prompt and its double self-check (§3.4) | L1, L2 | AFK |
| L4 | The card layer's review, fix and fill job, with the `add`/`skip` apply (§3.5) | L1, L2 | AFK |
| L5 | Report and audit coverage; the blueprint release 5.8.0 / 6.8.0 / 5.8.0; harnesses, installs, wiki | L3, L4 | AFK |
| L6 | Pilot `mode cards` on an adopted Nobel copy in a copied box, against the acceptance list in §3.8 | L5 | AFK |
| L7 | Backfill the five live books with `mode cards` on adopted copies | L6 and the user's go-ahead | owner gate |
