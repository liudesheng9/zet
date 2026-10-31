# Cited Literature Cards: Proposal

Status: accepted by the user on 2026-10-31, with three changes to the first draft: every listed work gets a Cited Card written from the whole source (the digest notes often leave citations out), there is no Citation link, and the rest of the open questions were left to the implementer (decided in Section 12). Nothing is built. The PRD and its slices are GitHub issues (Section 10).

It follows the Term module ([term-cards-and-term-links-proposal.md](term-cards-and-term-links-proposal.md)) for its list and its Cards, and it obeys the module rules in [workflow-module-structure](wiki/workflow-module-structure.md) and the working rules in [workflow-principles](agents/workflow-principles.md).

## Summary

A digested book today records what the book says, its terms, models, events and findings. It records nothing about the literature the book builds on. The reference list of the source is staged by `prepare` and then dropped (it is outside the digested headings, or it is in `skipHeadings`). The in-text citations (`Deming（2017）`, `Bloom 等（2013）`) survive in the Literature notes only where a note writer kept them, and the notes often leave them out.

This proposal adds a **cited-literature module** with the list-and-Cards shape of the Term module:

| Term module (built) | Cited-literature module (new) |
|---|---|
| Term list Card `<key>/1\|a`, one table of the book's terms | **Cited list Card** `<key>/1\|b`, one table of the works in the book's reference list |
| Term Card per term, an immediate Side successor of the list | **Cited Card** per listed work, an immediate Side successor of the list |
| Stage `terms` (the list), `term-cards` (DeepSeek writes the Cards) | Stage `cited` (the list), `cited-cards` (DeepSeek writes the Cards) |
| Card type `term`, field `term_name` | Card type `cited`, field `cited_ref` |
| Term link, a deterministic pass | **None.** In-text citations have too many forms for a deterministic pass, and no writer follows a citation list, so a citation pass would add complexity for little gain. |

Each Cited Card states one cited work: its full reference, where the book uses it, **why the book cites it**, **what role it plays in the book's research**, and **what the relationship between the book and the work is**. The DeepSeek job that writes it reviews how the work is involved in the **whole source**, not in the digest's notes, and every sentence rests on the source lines it reports.

## 1. What the user asked

1. Read the principles for developing a digest module, and state them before any design.
2. Add a new module to the digest: the literature the book cites.
3. A new top-level Card of the book holds the list of the literature the book cites.
4. Under this list Card, one Card per cited work. Each Card briefly states why the book cites the work, what role the work plays in the book's research, and the relationship between the book and the work.
5. The structure of the module is similar to the Term module.
6. (2026-10-31) For each work, a subagent reviews how the work is involved in the whole material and writes that work's Card from it, because the digest often leaves the citations behind.
7. (2026-10-31) No Citation link between Cards for now.

## 2. The principles this module follows

These are the rules from `docs/agents/workflow-principles.md`, the module-structure proposal and the user's standing feedback, applied to this module.

### 2.1 Module structure

- P1. **A feature block is one Haiku manager and several DeepSeek subagents.** At most 8 DeepSeek conversations per manager and 8 managers at once (DSMP). Conversations are deleted after collection, never archived. The workspace is removed at the end.
- P2. **No Claude agent runs a command.** Every deterministic chain (extract, plan, claim, check, publish) is one step of a step script (`cited-steps.ps1`), run by a DeepSeek drive job through `ztDrive`.
- P3. **The module is a straight line**: `start → writers → finish`. Its only branches are "no work" and "a step failed". Per-unit outcomes are ledger data, not module branches.
- P4. **The ledger is the truth.** Each step records its unit outcomes and prints one compact JSON line (≤400 characters, boolean top-level `ok`, counts and paths only). Larger data goes to the step's `drive-lines` detail file.
- P5. **One shot, then report.** No JavaScript retry loop. A retry is the one DSMP retry of a job, or a later run.
- P6. **Release what you hold.** `start` releases the claims a stopped run left. Long stages claim with the 7200 s lease.
- P7. **Install with the protocol.** Both variants installed in the project and global scope, with the DSMP block, and `verify` ok.

### 2.2 Roles and models

- P8. **Claude tokens are precious.** Haiku (effort low) only manages. No Sonnet or Opus role in this module: there is no design or judgement task in it.
- P9. **DeepSeek does the labour**: V4.1 Flash, official source only, effort high, for every content and drive job.
- P10. **Scripts do the deterministic work.** Parsing the reference list, indexing the candidate locations in the source, matching a work to a Literature tree in the box, checking the reported locations, rendering the Card body and publishing are script work. No LLM agent per item for script work.

### 2.3 Content

- P11. **Digests build Literature trees only.** Every Card of this module is in the book's own Literature tree. No Topic Card, hub or register entry.
- P12. **Faithful to the source.** Every sentence of a Cited Card rests on source lines the writer reports, and the script checks that each reported line exists and names the work. The writer does not use what it knows about the cited work from elsewhere. No invented reference data (no guessed DOI, journal or page).
- P13. **The whole source, not the notes.** The writer reviews the work's involvement in the whole source (every heading, footnotes and appendices included, both languages of the bilingual file), because the Literature notes often leave citations out. The script's candidate index is a starting point, not a limit.
- P14. **One DeepSeek conversation does the whole job**: write, self-check (`zt-digest cited-cards -Step precheck`, the same rules the publication applies), fix until `faults: 0`, at most three runs. No separate review job and no fix job (a Cited Card has no mathematics).
- P15. **Card language** follows `asd-ste100-zh`. Terms keep the Term list spelling. The writer writes no `[[…]]` macro; the script adds the Links.

### 2.4 Ordering

- P16. **No writer runs beside a writer it depends on.** `cited-cards` runs after `cards-book` (the lesson of the stale Term-card plan, base 5.7.0), because a Cited Card links the Literature notes that cover its locations.
- P17. **Every stage or mode that writes a Card is followed by the Term-link pass** (`framework-harness.mjs` guard). `cited` and `cited-cards` join that guard, so terms in the Cited Cards get their Term links.

### 2.5 Testing and release

- P18. **Test first.** A test that fails before the fix and reproduces the live case. Harnesses green: `framework-harness.mjs`, `cited-steps.tests.ps1`, the Rust tests of `zt`, `zt-check` and `zt-digest`.
- P19. **Test by resuming or by a single-stage mode** (`mode cited-cards`) on a staging copy. Never a fresh full digest to verify. Never a finished book rerun.
- P20. **The live box only when the user names it**, one book at a time, after `VACUUM INTO` and an integrity check. Pilots run on a copied box; legacy books are adopted on a copy of their staging folder.
- P21. **Release discipline.** A new stage and a new mode are a minor bump of base, econ and math together. Rebuild and deploy `zt-digest` (it embeds the blueprints), rebuild `zt` on `D:`, rename a locked exe before the copy, never deploy under another session's running digest. The wiki block doc changes in the same commit.
- P22. **A typed refusal is the owner's decision.** `card_locked` stops the step, writes nothing more and names the command to run after the owner acts.
- P23. **Simplicity over branches.** Do not overbuild. The optional parts (Section 9) wait.

## 3. Current state

| Fact | Where |
|---|---|
| The source keeps its reference list. Example: Hansen et al. 2021 has `# References` / `# 参考文献` at line 1296 of the bilingual Markdown, with 39 English entries such as `Bandiera, O., Prat, A., Hansen, S., and Sadun, R. (2020). CEO Behavior and Firm Performance. Journal of Political Economy, 128(4):1325–1369.` | Scholar Archive file 806 |
| The entries are not translated, so `chinese_blocks` drops them, and the section is outside the digested range | `tools/zt-digest/src/prepare.rs` |
| The staged Chinese sections keep in-text citations: 23 distinct `Surname（year）` forms in `hansen2021demand/sections/*.md` (`Deming（2017）` in s01, s08, s09, s10, s13, s14, s15), plus parenthetical forms such as `（Bloom 等，2013）` | `D:\zt-digest\<key>\sections` |
| The staged sections cover only the digested range; footnotes, appendices and skipped headings are not staged | `prepare.rs` (`staged_sections`) |
| The Literature notes cover every unit (100% coverage since #183–#192), but a note does not always keep the unit's citations | `zt-digest coverage`; user report 2026-10-31 |
| The Term list stands at `<key>/1\|a`, a Side successor of the first anchor; the digest writers then take `/1\|b`, `/1\|c`, … | `hansen2021demand/1\|a` (56 Term Cards, 32 parts) |
| A Tree root takes no Side successor | `CONTEXT.md` (Tree root) |
| The Term type is `CardType::Term` with column `term_name` (a CHECK on the cards table, so a new column rebuilds the table in one transaction) | `src/card_type.rs`, `src/term.rs`, `src/main.rs` |

## 4. Decisions

| No. | Question | Decision | Reason |
|---|---|---|---|
| D1 | Where does the Cited list Card stand? | **An immediate Side successor of the first anchor, right after the Term list: `<key>/1\|b` in a fresh run.** In a backfill of an existing book it takes the next free Side label of `/1` (the Cards there do not move). Accepted by the user. | A Tree root has no Side successors, and a new Direct successor `<key>/N+1` would read as a new section after the book's last one. Beside the Term list, the two apparatus Cards stand together at the top of the tree. |
| D2 | A new Card type? | **Yes: type `cited`, one type with two roles computed from one stored field `cited_ref`**: empty → Cited list Card; a reference id → Cited Card. | The same pattern as `term`. The GUI, search and the checker find the Cards by type, not by title. "Reference Card" is an avoided word for a Literature Card, so the type is `cited`. |
| D3 | Where do Cited Cards stand? | **Immediate Side successors of the Cited list Card, in list row order**: `<key>/1\|b\|a`, `\|b`, …, `\|z`, `\|aa`, … | Every Cited Card is a child of the list; a Direct chain would say that work 12 continues work 11. |
| D4 | Row order of the list | **The order of the book's own reference list.** | The order a reader of the book knows, and deterministic. |
| D5 | Which works get a Cited Card? | **Every row of the list.** The writer reviews the whole source for the work. A work the source names only in its reference list gets a short Card that says so (`本书只在参考文献中列出这篇文献，正文没有引用它。`) and the relation `背景`. | The user's decision: the digest notes leave citations out, so a Card is written from the whole source for every work. |
| D6 | The reference id (`cited_ref`) | **A deterministic id from the first author's surname, the year and a suffix**: `deming2017`, `bandiera2020a`. At most 40 characters, `[a-z0-9]` only. | Stable across plan revisions, readable, never a Citation key or Location. |
| D7 | Who builds the list? | **A script** (`zt-digest cited -Step plan\|apply`). It parses each entry (authors, year, title, venue). An entry the parser cannot split goes to **one DeepSeek normalize job**, whose row the script accepts only when every author surname and the year stand in the raw entry. | P10; OCR reference lists vary in form, so a small checked model job closes the parser's gaps. |
| D8 | What does the writer start from? | **A candidate index built by the script** over the whole source file (both languages, every heading, footnotes and appendices): each line that names the first author's surname with the year, or two or more title words. Each candidate carries its line number, its heading and, when the line is in a digested unit, the Literature-note Card that covers the unit. | A cheap deterministic start; P13 makes it a start, not a limit. |
| D9 | Who writes a Cited Card? | **DeepSeek content jobs, 4 works per job** (run argument `citedCardsPerJob`, 1–8). Each job reads the whole source as it needs (search by surname, translated name and title words), writes a JSON draft with the locations it relies on, and runs its own precheck. `zt-digest cited-cards -Step apply` renders the Markdown and creates the Cards. | P14. A whole-source review per work is heavier than a Term Card, so 4 per job: 39 works are 10 jobs under 2 managers. |
| D10 | The relationship field | **One or more fixed relation words** read from the source: `依赖` (rests-on: the book uses its result, method or data), `推广` (relaxes: the book extends it), `对照` (contrasts), `反驳` (objects), `印证` (illustrates: the book's result agrees with it), `参照` (corresponds), `背景` (refers: background only). | The words already have meaning in ZET's Relation kinds, and a fixed set keeps the Cards comparable across books. |
| D11 | Links on the Cited Card | **The script adds them**: `来源：[[<key>]]`; for each reported location inside a digested unit, `另见 [[<note Card>]]`; and `[[<CitationKey>]]` when the work is itself a Literature tree in the box (matched by normalized title and year, never by guess). | P15; it connects the Cited Card to the notes and to the other books of the box. |
| D12 | Citation links in the Card bodies | **None** (user, 2026-10-31). | Citations have many forms, and no writer follows a citation list as writers follow the Term list. |
| D13 | Stage order | `prepare → cited (beside terms) → … → cards-book → cited-cards (beside term-cards) → … → bridge → term-link → audit` | P16, P17: the list needs only the source; the Cards need the settled notes; the Term-link pass comes after both. |
| D14 | Backfill of existing books | **Mode `cited-cards`**: one already digested book, `out` only, runs `prepare` (backfill validation), `cited`, `cited-cards`, `term-link`, `audit`, `cleanup`, `report`. | The contract of mode `term-cards` (S25, #168). |
| D15 | Size of a Cited Card | **Short**: the header lines, three sections of 1–4 sentences, and one line per location used (at most 12). | The user asked for a brief introduction. |

## 5. The Card type `cited`

### 5.1 Glossary entries (to add to `CONTEXT.md`)

**Cited Card type**: the Card type `cited`: a Card of the cited literature of one book. A `cited` Card is a Cited list Card or a Cited Card, computed from its `cited_ref` field and its Location. Only Regular Cards can be `cited` Cards.
_Avoid_: Reference Card, bibliography entry

**Cited list Card**: a Regular Card of type `cited` with no reference id: the table of the works in one digested book's reference list (`参考文献：《…》引用的 N 篇文献`). Its immediate Side successors are its Cited Cards, in table row order.
_Avoid_: Bibliography, reference list Card

**Cited Card**: a Regular Card of type `cited` with a reference id, an immediate Side successor of a Cited list Card. It states one work of the book's reference list: its reference, where the book uses it, why, its role in the book's research and the relationship between the book and the work.
_Avoid_: Reference Card, citation note

**Reference id**: the stored `cited_ref` field of a Cited Card: the first author's surname, the year and a letter suffix when needed, in `[a-z0-9]`, at most 40 characters.
_Avoid_: Cite key (that is a Citation key)

### 5.2 Storage (zt core)

| Change | Detail |
|---|---|
| `CardType::Cited` | New variant in `src/card_type.rs`; refused on a Tree root |
| New column | `cited_ref TEXT` beside `term_name`, with a CHECK that it stands only on a `cited` Card (the table rebuild of the `term_name` upgrade); cleared by a retype |
| Validation | `cited_ref` is `[a-z0-9]{1,40}` |
| Dump and load | `zt dump` writes and reads the field |
| Structure and faults | `src/cited.rs` (as `src/term.rs`): `cited_unfiled`, `cited_duplicate`, `cited_row_without_card`, `cited_card_without_row` |
| MCP | `get_cited {at}`: rows, Cards, faults, coverage (as `get_terms`); `create_successor`/`edit_card` accept `type: cited` and `cited_ref` |
| GUI and Session | show the `cited` type label and the list as an ordinary table; no new layout |

### 5.3 The tree

```text
hansen2021demand                                  Literature Card
└─ /1        引言：高管技能需求的第一份直接证据与社交技能模型
   ├─ /1|a   术语表：《The Demand for Executive Skills》的 56 个术语      Term list Card
   ├─ /1|b   参考文献：《The Demand for Executive Skills》引用的 39 篇文献   Cited list Card
   │   ├─ /1|b|a   Adams, Akyol 和 Verwijmeren（2018）：董事技能组合        Cited Card (row 1)
   │   ├─ …
   │   ├─ /1|b|n   Deming（2017）：劳动力市场中社交技能的重要性上升          Cited Card (row 14)
   │   └─ …
   └─ /1|c   (the digest's other Side successors)
```

(In a backfill of hansen2021demand, `/1|b` is taken, so the list takes the next free label, `/1|g`. Row numbers here are illustrative.)

## 6. The two Card bodies

### 6.1 The Cited list Card (rendered by the script)

```markdown
本卡是《The Demand for Executive Skills》的参考文献表。表中按原书顺序列出它引用的 39 篇文献；每篇文献有一张文献引用卡，排在本卡之后。

| 编号 | 文献 | 年份 | 引用处 | 关系 |
| --- | --- | --- | --- | --- |
| 1 | Adams, Akyol, Verwijmeren. Director skill sets. *Journal of Financial Economics* 130(3) | 2018 | §1 | 背景 |
| 14 | Deming. The Growing Importance of Social Skills in the Labor Market. *QJE* 132(4) | 2017 | §1、§3.2、§5（7 处） | 依赖、推广 |
| 22 | … | 2009 | 仅参考文献 | 背景 |

来源：[[hansen2021demand]] 参考文献
```

The `apply` step of the list writes the first three columns; the `apply` step of the Cards fills `引用处` and `关系` from the published Cited Cards.

### 6.2 The Cited Card (rendered by the script from the JSON draft)

```markdown
文献：Deming, D. J. (2017). The Growing Importance of Social Skills in the Labor Market. The Quarterly Journal of Economics, 132(4):1593–1640.
引用处：§1、§3.2、§5（共 7 处）
关系：依赖、推广

## 为什么引用

<1–3 句：本书在哪里、为了什么引用这篇文献。>

## 在本书研究中的作用

<1–4 句：这篇文献为本书提供了什么（问题、方法、数据、结果、对照）；本书的哪一部分用到它。>

## 本书与该文献的关系

<1–4 句：本书继承、推广、对照或反驳了它的什么。关系词与「关系」行一致。>

## 引用处

- §1（第 52 行）：<这一处怎样使用该文献，一句转述> 另见 [[hansen2021demand/1|f]]
- §5（第 640 行）：<…> 另见 [[hansen2021demand/9|a]]
- 脚注 12（第 1180 行）：<…>

本库中的这篇文献：[[deming2017growing]]
来源：[[hansen2021demand]] 参考文献第 14 条
```

The `文献`, `引用处`, `关系` header lines, the `另见` and box Links and the `来源` line are written by the script. DeepSeek writes the three sections and one sentence per location. A location outside every digested unit (a footnote, an appendix) has no `另见`. The box line appears only when D11 finds the work in the box. (The addresses and the box key are illustrative.)

## 7. The stages

### 7.1 `cited` (the list): deterministic, with one checked normalize job

- `cited-steps.ps1 -Step list`: `zt-digest cited -Step plan` parses the reference section of the raw source (headings `References`, `Bibliography`, `参考文献`, or the run argument `citedHeading`), builds the candidate index over the whole source (D8), matches the works to Literature trees in the box, claims `cited/book@1`, and writes `cited/plan.json` and the normalize job file for the entries it could not split. Prints `{ok, rows, unparsed, jobs, file}`.
- If `jobs > 0`: one Haiku manager runs the normalize jobs (DeepSeek; self-check `zt-digest cited -Step precheck`).
- `cited-steps.ps1 -Step list-finish`: `zt-digest cited -Step apply` accepts each normalized row only when its surnames and year stand in the raw entry, creates the Cited list Card and completes `cited/book@1` from the apply report. Prints `{ok, rows, refused, card}`. A source with no reference section answers `{ok:true, rows:0}` and the module skips `cited-cards` ("no work").

### 7.2 `cited-cards` (the Cards): Haiku managers, DeepSeek writers

Module `zt-cited-cards` v1, the shape of `zt-term-cards` v8 without the review round:

```text
drive start   → plan, claim every ready citedcards/tNNN@rev unit, list the jobs (drive/jobs.json)
writers       → DeepSeek, 4 works per job, ≤8 jobs per manager, managers in parallel; each job self-checks
drive finish  → precheck every draft, complete each job unit from the readiness result, fail the rest,
                publish every ready row under citedcards/book@rev, fill the list's 引用处 and 关系 columns
```

Each job file names, for each work: its entry and reference id, the path of the whole source file, the candidate index lines with their headings and note Cards, the book's Term list (for spelling), the relation word set, and the `SELF-CHECK:` line. The instruction file is `scripts/book-digest/dsh-cited-card-instructions.zh.md`.

## 8. Checks

`zt-digest cited-cards -Step precheck` and `zt-check` rules C1–C7 (the publication applies the same set):

| Rule | Check |
|---|---|
| C1 | The three sections are present, each 1–4 sentences. |
| C2 | The `关系` words are from the D10 set, and each one is named in `本书与该文献的关系`. |
| C3 | Each reported location is a line of the source file, and that line (or the line before or after it) names the work: the first author's surname (Latin or the source's Chinese rendering) or two title words. |
| C4 | No author, year, title, venue, number or DOI that the entry and the reported lines do not contain. |
| C5 | No `[[…]]` macro in the draft. |
| C6 | Term spellings: no word from a Term list `不用` column (R6). |
| C7 | The list table: five columns, one row per entry, row order equal to the source. |

A draft with no in-text location must say so in `为什么引用` and use only `背景` (C2, C3).

## 9. Optional parts (later, not in this PRD)

- **Scholar Archive match**: a Document link `[原文](scholar:<file_id>)` on a Cited Card when the Scholar Archive holds the work with a file, matched by DOI or exact title and year.
- **GUI**: a card-like rendering of the Cited list; it starts from a mockup the user accepts.
- **`zt-idea-grow`**: read the `关系` words across books to find works that several books build on.
- **Citation links**: removed by the user on 2026-10-31; reopen only with a new proposal.

## 10. Slices (GitHub issues)

| Slice | Block | Content | Blocked by |
|---|---|---|---|
| R1 | zt core | `CardType::Cited`, `cited_ref` column, dump/load, `src/cited.rs` structure and faults, `get_cited`, `CONTEXT.md` | — |
| R2 | zt-check | Cited Card category and rules C1–C2, C4–C7 (C3 reads the source, so it stays in the precheck) | R1 |
| R3 | zt-digest | `cited -Step plan\|precheck\|apply`: parser, candidate index, box match, normalize jobs, the list Card, ledger layer `cited` | R1 |
| R4 | zt-digest + prompt | `cited-cards -Step plan\|jobs\|precheck\|check\|apply`, the instruction file, ledger layer `citedcards`, list column refresh | R1, R2, R3 |
| R5 | workflow | `cited-steps.ps1`, modules `zt-cited-list`, `zt-cited-cards`, the stages and mode `cited-cards`, the Term-link guard, report, release base 5.10.0 / econ 6.10.0 / math 5.10.0 | R3, R4 |
| R6 | pilot | mode `cited-cards` on a copied box with an adopted hansen2021demand staging copy | R5 |
| R7 | live backfill | the live books, one at a time, only when the user names it, after a backup | R6 |

Each slice updates its wiki block doc in the same commit: [cards-and-links](wiki/cards-and-links.md), [mcp-server](wiki/mcp-server.md), [zt-check](wiki/zt-check.md), [zt-digest](wiki/zt-digest.md), [digest-prompts](wiki/digest-prompts.md), [digest-workflows](wiki/digest-workflows.md), [workflow-module-structure](wiki/workflow-module-structure.md).

## 11. Risks

- **Reference formats vary.** Author-year papers parse well; a book with footnote citations or chapter-end lists (Ebrill 2001, Hart 1995) needs the normalize job and per-chapter headings. The pilot measures the share of rows the parser alone splits.
- **Names in the Chinese text** may be translated (`德明`) or abbreviated (`X 等人`). The whole-source review and C3's Chinese-rendering rule cover them; an unmatched location is refused, never guessed.
- **Same surname, same year.** Two entries that the book does not tell apart (no `a`/`b` suffix) make each location ambiguous; the writer must name the title words that decide it, or the location is refused.
- **Large books.** A 400-entry book is 100 jobs; the eight-manager DSMP pool runs them in waves. The plan freezes the job list, so a resume drains it.
- **Locked trees.** Live books may be locked by the owner. A backfill stops with `card_locked` and names the command to run after the owner acts.

## 12. Decisions the user left to the implementer (2026-10-31)

- Q3 (Citation links): removed (D12).
- Q4 (relation vocabulary): the seven words of D10.
- Q5 (pilot book): hansen2021demand: short, author-year, 39 entries, its staging folder at `D:\zt-digest\hansen2021demand`.
