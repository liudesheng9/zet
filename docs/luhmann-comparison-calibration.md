# Calibrating the standalone and concept advice: review package (S25, #102)

Status: evidence for the human gate of [issue #102](https://github.com/liudesheng9/zet/issues/102) (design §15-phase-1, D367/D368). The gate's decision is **PENDING-HUMAN**: this document records what was measured, what is uncertain, and what the human must approve or return as a correction. It is not permission to rewrite historical Cards, and it changes none.

The reviewed example set is [`scripts/book-digest/tests/calibration.tests.ps1`](../scripts/book-digest/tests/calibration.tests.ps1) (fingerprint `210ffe5b38e51b83c3e5a1b8f495c890f056c7657f427835863dd4b822cfe443`). Every example names its source (the accepted proposal's P9/P11 gap or the design's §14.1 fixture list), the lines the checker's contract requires, and the reviewer's annotation:

| Annotation | Meaning |
| --- | --- |
| `true_positive` | The reported line names a real comprehension gap |
| `false_positive` | The line is reported, but the text is understandable as it stands |
| `false_negative` | The text is a real gap and no line is reported (a retained limit) |
| `valid_counterexample` | No line is reported, and the text is fine |
| `unknown` | The checker says it cannot decide (incomplete context, missing source) |

A family's **false-positive rate** is `FP / (TP + FP)`: among the examples where the checker fired, the share a reader judges wrong. Its **false-negative rate** is `FN / (TP + FN + unknown)`: among the real gaps and the undecided cases, the share no line caught. `null` means the denominator is zero.

## 1. The reviewed set: coverage, counts, rates

The set holds **42 examples** in seven families: 35 through the checker (the public `zt-check --cards` contract on a scratch archive, and the `--json` bundle contract the audit writes) and 7 through the concept review (`zt-digest concepts prepare/judge/report`). All 42 reproduce the contract they are annotated with.

| Family | Rule | Measure | Examples | TP | FP | FN | Counterexamples | Unknown | FP rate | FN rate |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| local numbers | R20 | P9 | 10 | 4 | 0 | 2 | 4 | 0 | 0 | 0.3333 |
| named concepts | R20 | P9 | 3 | 0 | 0 | 0 | 3 | 0 | null | null |
| referents | R21 | P9 | 7 | 4 | 1 | 0 | 2 | 0 | 0.2 | 0 |
| own-source dependencies | I7 | P1 | 6 | 1 | 0 | 0 | 4 | 1 | 0 | 0 |
| duplicate titles | I6 | P11 | 9 | 2 | 1 | 0 | 4 | 2 | 0.3333 | 0 |
| concept senses and statements | concepts | P5/P2 | 7 | 5 | 0 | 0 | 2 | 0 | 0 | 0 |
| **all** | | | **42** | 16 | 2 | 2 | 19 | 3 | | |

What each family covers:

- **Local numbers (R20).** The title and the authored Topic prose. The proposal's own example `式 (12) 含加成项。式 (11) 不含加成项。` gives one advice line: the first sentence carries 4 characters of content beside the label, the second 5, and 5 is the constant (`LOCAL_REFERENCE_CONTENT`). A content-bearing label, a `来源：[[book]] §11.D` locator, a code fence, a quotation, inline code, and an anchor's own section index raise nothing.
- **Named concepts (R20 counterexamples).** `第一福利定理` is the proposal's named false-hit risk and raises nothing; `命题 3.G.3` with its content in the same sentence and `帕累托最优` likewise.
- **Referents (R21).** Each sentence and each title. A Link in the same sentence names the referent; a Link in the sentence before does not; code and quotations are excluded.
- **Own-source dependencies (I7).** A `rests-on` Link into the Card's own source's Literature tree is the one hard layer line; `illustrates`, `corresponds`, `contrasts`, another book, and a Card filed in that tree are no fault; missing source evidence is an unknown line, never a pass.
- **Duplicate titles (I6).** The `--cards` Workspace title context: across trees, one line per unordered pair, a context Card is never the subject, a Hub Card is no subject, another Workspace is no pair, and an incomplete context or an entry naming no Workspace is an explicit unknown. The `--json` cases cover what an agent cannot create on a real archive (a human-Workspace Card).
- **Concept senses and statements.** The book's own four-column term table supplies two 学习 senses; a shared formula under a narrower scope (参与者对称 added) stays a difference with the formula and `retain_both`; the identical claim (same formula, same assumptions) is one statement; the deterministic shortlist keeps an unrelated Topic and an unrelated statement out.

### Retained false positives (2)

1. **A referent named in words (R21).** `剩余控制权的讨论表明，授权之后该卡成立。` is reported because the rule detects a same-sentence *Link*, and the sentence names its subject in words. The design already states the limit (§6.2: R21 cannot determine all Chinese coreference). A reader understands the sentence, so the advice is wrong for this text.
2. **An existing hub's copied title as I6 context.** When a Hub Card's title equals an Idea Card's, the hub is context for the checked Idea Card, so the Card gets an extra line naming the hub (`4/1 I6 (advisory) a title very similar to [[4/1|b]]`). The duplicate is the hub's `anchor-title` fault (P11: 33 such hubs exist in the live archive), not the Idea Card's, and the measure asks the Idea and navigation populations to be reported apart (§14.3 P11). Advice only; nothing is merged or deleted.

### Retained false negatives (2)

1. **A long title with book-local model numbers.** `短期债务在模型 1 触发清算，在模型 3 只是阻止扩张` is one of the proposal's 71 number-carrying Topic titles, but the rest of the title carries enough content to pass R20's threshold. A reader still cannot tell what 模型 1 and 模型 3 are.
2. **A bare section number.** `11.D.1 的市场方案要求产权明确且主体众多` is another proposal example; R20's `§` pattern (`LOCAL_SIGN`) needs the section sign, so a bare `11.D.1` is undetected.

Both are limits of the heuristic *relative to P9's measure*, not regressions: the rule's contract is a label with no nearby content, and both titles do state their claim. Whether either becomes a blocking or extended rule is a separately versioned decision (see §5).

### Unknowns (4)

| Unknown | Evidence |
| --- | --- |
| An incomplete title context | `12/9|b I6 (advisory) Workspace title context incomplete: 1 known title compared, others unknown` — the known rows are still compared, and the run states the limit |
| An entry naming no Workspace | `12/9|c I6 (advisory) Workspace scope unknown: the entry names no Workspace, so nothing is compared` |
| Missing source evidence (I7) | `3/1|c I7 (advisory) source evidence unknown, so [[Cal2026/1]] may be an own-source premise` |
| The semantic judge's accuracy | Not exercised: the concept review's judge answers are canned. Its accuracy is the S26 pilot's evidence (see §4) |

D368's requirement holds: an incomplete context is *stated in a line*, so no run of that mode can read as a complete Workspace check. The `--json` mode counts every printed line in `faults:`, so its zero can never be mistaken for a clean complete run either.

## 2. Advice-only and the semantic review (S25-AC03)

The checker's public modes, measured in the reviewed runs:

| Run | Rules | Lines printed | Advisory | Counted faults |
| --- | --- | --- | --- | --- |
| r20 | R20 | 4 | 4 | 0 |
| named | R20 | 0 | 0 | 0 |
| referent | R21 | 5 | 5 | 0 |
| own | I7 | 2 | 1 | 1 |
| i6-both | I6 | 1 | 1 | 0 |
| i6-one | I6 | 1 | 1 | 0 |
| i6-hub-context | I6 | 2 | 2 | 0 |

R20, R21 and I6 are advice: printed, never counted as a fault. I7's own-source line is the one hard layer fault; its unknown-source line is advice. The semantic review stays required, and the prompts and the checker's own block say so:

- `scripts/book-digest/dsh-card-instructions.zh.md`: I6 is advice; a title match alone deletes or merges nothing.
- `skills/zt-digest-protocol/SKILL.md`: I6 is advice like R20 and R21; the Card review still requires standalone text.
- `docs/wiki/zt-check.md`: R20 and R21 are advisory and counted apart from blocking faults.

Promotion of any heuristic case to a blocking fault remains a separately versioned decision with its own fixtures; this gate does not perform one.

## 3. The read-only baseline (S01) on the live archive

The S01 baseline was run **read-only on a consistent copy** of the live archive, so the live archive was not touched: `VACUUM INTO` a temp copy (`D:\zt-target\calibration-baseline\archive\zt.sqlite3`, integrity `ok`, 10,895 Cards, 56,994 Links), an isolated `ZT_CONFIG_DIR` and service on the copy, then

```powershell
pwsh -NoProfile -File scripts\book-digest\digest-audit.ps1 -CitationKey hart1995firms -Out <copy>\audit -SkipModels
```

The report's own evidence block (verbatim values):

- archive `806009944a966daad513b2aacabddc5f`, revision `0`, 10,895 Cards;
- software `zt-digest 0.1.0`, `zt-check 0.1.0`, `zt 0.20.0`, rules `R2-R21, I1-I7, E1-E5, F1-F4, L1-L3`, advisory rules `I1, I6, E5, L3, I7, R20, R21`;
- thresholds `sample 15`, `sample_seed 1`, `near_title 0.8`, `register_reach_links 2`, `task_sheet_schema 1`;
- scope: book `hart1995firms`, 645 Cards, 208 trees;
- the annotation sample: population 364 Idea Cards, seed 1, 15 drawn — `46/1|b|1|a`, `47/1|a`, `49/1|b|b`, `49/3|c`, `49/3|f|a|a`, `50/2|c`, `52/1|a`, `52/2|f`, `57/1`, `57/2|a|a`, `59/1|a|2`, `hart1995firms/18|a|a`, `hart1995firms/21|a`, `hart1995firms/8|c`, `hart1995firms/8|d|a`;
- coverage: `complete: false`, 5 of 11 measures known. Missing: no earlier baseline (P1, P2, P3/P5, P12/P13, P14 unknown), no concept report (P2), no run report (P4), no task sheet (P9's manual annotation, P12/P13, P15). Each missing input is `known: false` with its reason — never a zero.

The measures this gate reads:

| Measure | Value | Denominator | Note |
| --- | --- | --- | --- |
| P9 local reference | 15 faults on 15 Cards (2.326 per 100) | 645 Cards | The heuristic's output, not the phenomenon's prevalence |
| P9 unnamed referent | 35 faults on 31 Cards (4.806 per 100) | 645 Cards | One Card can carry several faults |
| P9 manual annotation | **unknown**: "the task sheet holds no annotation of the sample" | 15 sampled Cards | The human annotation is not supplied |
| P11 Idea pairs | 1 exact + 6 near = 7 (1.125 per 100) | 622 Idea Cards | `truncated: false` |
| P11 navigation pairs | 0 exact + 0 near | 23 navigation Cards | Populations reported apart as §14.3 asks |
| P11 `anchor-title` faults | 33 | — | Matches the proposal's 33 hubs that copy their anchor's title |
| P7 cross-book hub lines | 48 (1.356 per 100) | 3,540 hub lines | Evidence-only; 815 unsupported lines counted apart |
| P8 accepted register terms | 78 accepted, 0 unresolved rows | 78 rows | 0 Cards within two outbound semantic Links |
| P14 human Cards | 8 | 10,895 Cards | Authored human→agent Links: 0 |

The proposal's P9 baseline (55% of sampled Topic Cards naming a book-local number, 72% pointing with a deictic) counted any such label. The implemented rule fires only when the label *stands in for its content*, on a whole-book scope (645 Cards, 265 Topic Idea Cards) rather than a hand-picked sample. The two numbers therefore answer different questions and are not a delta; the proposal's values keep their original scope (§14.3). The measured rates say the rule is quiet on the current archive, not that the phenomenon is rare.

The baseline's **annotation sample** of 15 Cards is the remaining human work of this gate (see §5): its sheet field is `unknown` until a person annotates those Cards.

## 4. Independent recalculation

The counts above were recomputed from the harness's own `calibration.json` in a separate pass (not by the harness): per family, the example count, the annotations, the printed advice lines and both rates. Every family agrees. The one difference is a formatting artifact: `named-concepts` has a `null` false-positive rate because its denominator is zero, and the recount prints it as an empty string rather than `null`.

Recomputed totals: 42 examples, 42 reproduced; 16 `true_positive`, 2 `false_positive`, 2 `false_negative`, 19 `valid_counterexample`, 3 `unknown`. Across the checker runs: 15 lines printed, 14 of them advisory, 1 counted fault.

## 5. The decision this gate needs

**S25-AC04** — approve the pilot interpretation, or return specific classifier or prompt corrections with retained evidence. Elapsed time is not approval, and this document does not infer it.

The question put to the owner:

> Approve the pilot interpretation recorded here — R20, R21 and I6 stay advice, I7's own-source line stays hard, the semantic Card review stays required, and the two retained false positives and two retained false negatives are accepted as the heuristics' documented limits — or return specific corrections, each with its evidence?

A correction would name the exact rule or prompt, the retained example(s) it concerns, and the intended behavior; a later versioned change would then update the set and the rules together. Nothing in this gate changes a historical Card, a finished model, the live register or the live archive.

**Recorded decision:**

| Date | Decision | Evidence or corrections |
| --- | --- | --- |
| — | **pending-human** | The requested decision is S25-AC04; it is not yet recorded |

## 6. No Card, model or archive changed (S25-AC05)

- The calibration harness runs on its own scratch archive: its archive revision is captured after the last fixture Card is created and must be unchanged after all reviewed runs (`revision: 46` in the measured run), and `zt status --check-links` passes at the end.
- The concept review's `report` step makes no tool call at all (`ZT_TRACE_TOOLS` empty), so it cannot write a Card.
- The baseline ran on a `VACUUM INTO` copy, not on `D:\zt`; the live archive's identity and revision are the copy's, and the live service kept running.
- The gate is human-interactive: this document and the harness's report keep the decision `pending-human`; only the owner's recorded decision completes it.

## 7. Rerunning the evidence

```powershell
# The reviewed example set, the measurements, and the retained unknowns:
pwsh -NoProfile -File scripts\book-digest\tests\calibration.tests.ps1 -Out <folder>
# -> <folder>\calibration.json, <folder>\calibration.md (decision: pending-human)

# The read-only baseline on a copy of the live archive (example; any agent book key works):
node copy.mjs                       # VACUUM INTO the copy, from D:\zt\zt.sqlite3
zt config set archive_root <copy>   # with an isolated ZT_CONFIG_DIR and ZT_GUI_PORT=0
zt up
pwsh -NoProfile -File scripts\book-digest\digest-audit.ps1 -CitationKey hart1995firms -Out <copy>\audit -SkipModels
zt down
```

The harness is also part of `scripts/book-digest/tests/run-all.ps1`, so every later change to R20, R21, I6, I7 or the concept review's report contract either reproduces this set or fails the suite with the difference named.
