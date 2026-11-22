# deming2015growing: post-digest sequence

Deming (2015), *The Growing Importance of Social Skills in the Labor Market* (NBER WP 21473). Stages 1–6 (fold, settings, restructure, import with the explanation refresh, illustrate of the formal model, evidence dialogue of the empirical models) ran on 2026-11-10 and are recorded in `docs/empirical-models-proposal.md` §14.12. This file records stage 7, which had not run: the model passes were done, but the tree still had the digest's group layout.

## Stage 7: literature organize (2026-11-21)

Date 2026-11-21, live box (owner's request "solve this"), under `zt-literature-organize`, done in one side session by scripts (`outline.mjs`, six structure batches through `batch.mjs`, one `leads.mjs` run, scripted `edit_card` writes against a base, one `delete_card`). No subagent and no DeepSeek job ran, so there is no DeepSeek cost. Backup: `D:\zt\backups\zt-before-deming2015growing-organize.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Every batch was previewed first (complete true, refusals none, `links_that_will_break` empty) and committed under its plan hash. Every text edit was sent with the stored text as its base, because another session was repairing TeX backslashes on the box at the same time; no edit was refused for a changed base, and no formula span was changed. Only Cards of deming2015growing were written. No `card_locked`, `scope_refused` or `read_view_changed` error occurred; the tree was unlocked throughout.

### Source divisions

From `D:\zt-digest\deming2015growing\book.zh.md` (the Scholar Archive Markdown the digest read; it holds the body from I. 引言 to before the reference list, so there is no front matter): I. 引言; II. 模型 (an untitled opening, then A. 设定, B. 无成本贸易下的均衡, C. 含社会技能的均衡, D. 对 $\theta$ 的解读, E. 实证预测); III. 数据 (A. O\*NET 与人口普查/ACS 数据, B. NLSY79, C. NLSY97 数据); IV. NLSY79 结果 (title only, then A. 技能的劳动力市场回报与互补性, B. 按技能的职业分选, C. 按职业任务强度划分的技能回报); V. 社会技能日益增长的重要性 (A–C); VI. 结论; 参考文献 (the Cited list). The OCR wrote the lower headings as "II..B", "III..A" and so on; the tree now uses the paper's "II.B". The model appendix (3.6–3.7) is not in the digested source (§14.12), so the tree holds no appendix to organize. The top line takes the six numbered sections, then the two lists.

The digest had filed the paper as 11 groups: old 1 §I with the Term list (`1|a`), the Cited list (`1|b`) and the structure Card (`1|j`) under it; old 2 the §II opening with the formal model 2\|a, whose Direct chain held the opening's reading notes; old 3 §II.A and §II.B on one interleaved chain; old 4–6 §II.C–E; old 7 §III.A and III.B on one chain; old 8 §III.C and §IV.A at once (its head text and the first two Cards of the Direct chain of model 8\|a were §III.C); old 9 §IV.B and IV.C with models 9\|a and 9\|b; old 10 §V.A–C on one chain with models 10\|b and 10\|c; old 11 §VI.

### Final tree (top line and divisions)

```
1  I. 引言 (old 1)                          c §I chain · d–i findings · j structure Card
2  II. 模型 (new, demote of old 2)           a 开篇 (old 2, with the opening notes old 2|a|1..8 as its chain) · b model 2|b (old 2|a)
                                            c A. 设定 (old 3|a chain) · d B. (old 3, notes in the source's order) · e C. (old 4) · f D. (old 5) · g E. (old 6)
3  III. 数据 (old 7, rewrite)                a A. (old 7|a chain + finding) · e B. (old 7|a|6..14 + 2 findings) · f C. (old 8|a|1..2)
4  IV. NLSY79 结果 (new, demote of old 8)    a A. (old 8; chain old 8|a|3..5, model 4|a|a, findings, footnote) · b B. (old 9|c..9|c|2, model 4|b|a, 2 findings)
                                            c C. (old 9; notes old 9|c|3..6, model 4|c|b, 2 findings)
5  V. 社会技能日益增长的重要性 (old 10, rewrite) a A. (old 10|a..10|a|6 + 3 findings) · b B. (old 10|a|7..15, model 5|b|a, 5 findings) · c C. (old 10|a|16..20, model 5|c|a, 3 findings)
6  VI. 结论 (old 11)
7  术语表 (Term list, from 1|a)             110 Cards
8  参考文献 (Cited list, from 1|b)           85 Cards
```

Batches: §V (15 moves and one reorder, 88 Cards renamed); §IV (6 moves and one reorder, 177 renamed); §III (4 moves, 12 renamed); §II (8 moves, 280 renamed); the two lists to the tail (2 moves, 195 renamed); the reading-note chains off the Model Cards (3 moves, 15 renamed). Then `leads.mjs` (two demotes, two rewrites), the hand edits, and one delete.

### Crossers and how each was settled

- Old 1 held §I and both lists: the lists went to the tail (`7`, `8`); the structure Card stays under §I as `1|j`, since the source has no front matter.
- Old 2 was the §II opening and the head of all of §II: demoted. A new §II summary took its place, the old Card became `2|a` (title `II 开篇：…`, its old 小节索引 and position line replaced by a source line and a position line), and the opening's eight reading notes, which ran on from the Model Card, became the opening's own chain (`2|a|1..8`). The model stands after the opening as `2|b`, before A–E, because it spans all of §II.
- Old 3 interleaved II.A (`§2.1`) and II.B (`§2.2`): cut by source line. II.A is `2|c` (old 3\|a with 3\|a\|1, 3\|a\|2 and its footnote-10 Card, 3\|a\|3). II.B is old 3 (`2|d`) with one chain in the source's order: task price (eq. 7), marginal task (eq. 8), demand (eq. 9, with its footnote-12 Card), the unique marginal task, wages. Old 3 went to II.B by Card count; its text was trimmed (opening sentence now recaps A with a Link to `2|c`, the `§2.1` index line dropped, source line `§II.B`).
- Old 7 ran III.A and III.B on one chain: cut at the first `§III.B` Card; A is `3|a`, B `3|e`. The footnote-20 Card (O\*NET's four social-skill items, labelled `§III.B 脚注 20`) stays in A under the Card on the three task measures, because it defines an O\*NET measure.
- Old 8 was III.C and IV.A at once: the two §III.C notes (the first two Cards of model 8\|a's Direct chain) moved to §III as its C head `3|f`; old 8 was demoted under a new §IV summary and became the IV.A head `4|a` (its III.C sentences, its `§III.C`/`§IV` index lines and the `§III.C` source label removed); the three IV.A notes that ran on from the Model Card became its chain (`4|a|1..3`). A §III.C Card written in this session before the two notes were found was deleted into `3|f` (2 Links re-pointed).
- Old 9 was IV.B and IV.C: cut. IV.B is `4|b` (old 9\|c with 9\|c\|1–2, model 9\|a, the two IV.B findings); IV.C is old 9 (`4|c`, trimmed to IV.C) with the notes 9\|c\|3–6, model 9\|b and the two IV.C findings. Old 9 went to IV.C by Card count.
- Old 10 was all of §V on one chain: cut into A, B, C at the first `§V.B` and `§V.C` Cards; each model and finding filed under its subdivision.
- Whole units: each model moved once with all its parts: 2\|a → `2|b` (40 parts), 8\|a → `4|a|a` (29), 9\|a → `4|b|a` (27), 9\|b → `4|c|b` (32), 10\|b → `5|b|a` (35), 10\|c → `5|c|a` (29). `check.mjs` reports two unit-size changes (`2|b` 50 → 41 Cards, `4|a|a` 35 → 29): these are the reading-note chains (8 + 1 footnote, 5) that left the Model Cards' Direct chains; no model part moved out.

Calls that could go the other way: the formal model filed after the §II opening rather than after II.E; the footnote-20 Card kept in III.A; the structure Card kept under §I; old 3 and old 9 heads given to the subdivision with more Cards.

### Summary leads and other text

- `leads.mjs`: demote `2` (§II) and `4` (§IV); rewrite `3` (§III) and `5` (§V). Each has the source's name and main claim as title, a walk through the subdivisions, `小节索引：` with a Link to each subdivision head, `在全书中的位置：` and the source line. Its Term-use pass linked three shorter Terms inside longer words in `3` (常规任务 in 常规任务强度, 社会技能 in 社会技能强度, 任务 in 任务内容) and one in `5` (技能回报 in 社会技能回报); all four were unlinked by hand.
- Titles with the section letter or number: `1` (I.), `6` (VI.), `2|c`–`2|g` (A.–E.), `3|a`, `3|e`, `3|f` (A.–C.), `4|a`–`4|c` (A.–C.), `5|a`–`5|c` (A.–C.). Trimmed heads: `2|a`, `2|d`, `4|a`, `4|c`. Source lines added where a former group head lost its anchor role: `2|e` (`§II.C`), `2|f` (`§II.D`), `2|a` (`§II（开篇）`).
- Source lines: the 34 Finding Cards named their old group head as source Link (`来源：[[…/1]] §1`, `[[…/8]] §IV.A`, …). All 34 now link the book root with the label, and the digest's numeric labels are normalized (`§1` → `§I`, `§3.1` → `§III.A`, `§3.2` → `§III.B`, `§6` → `§VI`).
- The root index lists the eight top Cards (the old 11-group list named digest groups and "II..B"-style headings); its lead line now says 每部分一行. The structure Card `1|j` lists the six parts and the six models at their new addresses; its old model list named the pre-fold Cards `2|a|a|1`–`2|a|a|4` as models.
- Chart targets of the formal model `2|b`: the moves rewrote every `data-zt` target with the Links (13 targets, all opening the intended model parts), so no chart needed a write. The figures module `~/.claude/skills/zt-model-illustrate/modules/deming2015growing-2_a.mjs` now names its head `deming2015growing/2|b`, so `apply.mjs` keeps working; its dry run reports 5 module lint faults (data-part wrappers, axis labels) that come from lint rules newer than the module, while the stored charts lint clean.
- `zt-check --cards` on the root, the structure Card, the 21 written heads and the 34 Findings: first run `faults: 15` (R2 lines over 40 characters and R3 two Links in one line in the root and `1|j`, I3 on `2|e` and `2|f`), fixed; final run `faults: 0` (advisory L1/L2/I4 lines only).

### Gate

All lines below are the reruns after the Term relink.

- `check.mjs deming2015growing`: top line 1 I. 引言 (25), 2 II. 模型 (91), 3 III. 数据 (24), 4 IV. NLSY79 结果 (109), 5 V. (96), 6 VI. 结论 (13), 7 Term list (110), 8 Cited list (85); no structural fault. Card count 552 at the start, 554 now (two new summary leads: §II and §IV).
- Source-line walk (171 note Cards with a source line, model parts and the two lists excluded): every Card stands under its own top division and subdivision (`2|c` §2.1; `2|d` §II.B/§2.2; `2|e`–`2|g` §II.C–E; `3|a` §III.A plus the footnote-20 call; `3|e` §III.B; `3|f` §III.C; `4|a`–`4|c` §IV.A–C; `5|a`–`5|c` §V.A–C). No crosser left.
- `list_broken_links` (agent): `{"broken":[]}`.
- On each model at its new address (`2|b`, `4|a|a`, `4|b|a`, `4|c|b`, `5|b|a`, `5|c|a`): `check-model.mjs` self_sufficient true; `check-network.mjs` `network: clean`; `check-settings.mjs` clean (`all in the setting part, as a dependency tree` for `2|b`, `the data tree and the specification ladder are in place` for the five empirical models); `imports.mjs check` `takes what it uses from other models by import lines`; `check-premises.mjs` clean except the pointer hits §14.12 already accepted (`4|b|a` names column (3) of Table II, 3 hits; `5|c|a`'s 假设 1 names equation (21), 1 hit).
- `model-survey.mjs deming2015growing`: the same 6 Model Cards: `2|b` formal (40 parts), `4|a|a` (29), `4|b|a` (27), `4|c|b` (32), `5|b|a` (35), `5|c|a` (29) empirical.
- `verify.mjs --model deming2015growing/2|b`: 11 charts in 41 Cards, 0 lint faults (dead targets included), 0 Cards without a figure, 0 setting Cards without a full 直觉.
- Term relink: `{"applied":true,"links_added":11,"cards_changed":4,"cards_refused":0,"rounds":1,"links_repaired":0,"ok":true}`; after the last text edit a second run and `-PlanOnly`: `cards_changed 0, links_added 0`. No relink bug.

### Open items

1. The relink tool links 认知技能 inside 非认知技能 (21 places in this book before this stage, one more in `3|f`). The Term-use rule says a shorter Term inside a longer word is wrong, but the relink would restore any hand fix; this is a tool question, not fixed here.
2. The unnumbered opening of §II and the title-only §IV have no source heading of their own; the opening is a Side branch `2|a` titled `II 开篇`, and §IV's new summary states that the source gives IV only a title.
3. The model appendix (3.6–3.7) is still not in the digested source (§14.12); an appendix extension would add a division after VI. **Done 2026-11-21**, see "Online appendix" below. Item 1 is fixed in zt-digest 0.54.12 (a Term after a privative prefix stays unlinked) and the relink was rerun on every tree.

## Online appendix (2026-11-21)

The owner asked to include the appendix in the digest source and apply a fast fix. Full working records: `x/deming-appendix-formal.md`, `x/deming-appendix-empirical.md`, `x/deming-appendix-organize.md` (untracked scratch, not in the repository).

### Source and extension digest

- NBER data appendix `Deming_SocialSkills_APPENDIX_May2017_FINAL.pdf` (51 pages) went through pdf-ocr-bilingual and was archived on the book's Work (files 2413–2418, not shown). The bilingual appendix was appended to source file 1472 as divisions A (supplementary tables and figures), B (data appendix §1 O*NET, §2 Occ1990dd) and C (model appendix §1–§3); the appendix's own reference list is skipped as back matter. Backup `D:\zt\backups\source-normalize-2026-11-21\…before-appendix`; new SHA `808ea79a…`. Source check: one coverage flag (the title page) judged original; then 0 faults.
- `zt-digest prepare -Extend` into `D:\zt-digest\deming2015growing-appendix` (groups 9–14), then zt-econ-digest mode resume (cited and Cited Cards off). The run left four gaps, closed one by one:
  - `models/g013-m0000`: the Model Card `13|a` linked the main-text chapter `2|b|a|2` from its head (a false share). The brief counted the Link as a part and skipped the model as built, and the finish refused it with "no accepted plan". The Link was turned into plain text, the unit reopened, and mode models built it.
  - Figures: 5 appendix figures had no explanation; zt-figure-explain on file 1472, then mode complete.
  - `cards-book/book@2` stayed claimed after the model failure: `ledger reclaim -Release`, mode cards (0 hard faults left, 4 duplicates deleted).
  - Cleanup failed twice: the Haiku manager ran the plan command itself, then the cleanup's own drive conversation stayed open. The conversation was deleted by hand and the unit settled with `zt-digest cleanup-finish -Ready` (the bedford2026learning recovery in workflow-principles).
  - Run report: `complete: true` (824 Cards, 12 Model Cards, 51 derivations explained, 76 Terms).

### Model passes

- **Formal `2|b`** (40 → 83 parts): `12|a`, `13|a`, `14|a` folded in; 22 duplicates deleted into their twins. Where the main text had only a 补 reconstruction, the appendix proof was kept: (9), (10), (15), (16)(17). Chapter 3 gains the symmetric closed form (50)(51), the wages proof (52)–(60) and the cognitive × social complementarity proof (61)–(68). Chapter 4 carries the §3.6 proof (69)–(77) and the §3.7 proof (78)–(85) as Side branches of their results. New chapter 5 (lognormal source of γ, θ≈2σ). Settings: new 假设 3 (lognormal draws); the notation card maps appendix to main-text equation numbers. Explanation refresh: one DeepSeek job, 19 derivation parts. Illustrate: the new setting card has its figure and 直觉. check-settings, check-model, check-network, imports and verify clean; check-premises 13 hits, all appendix/main-text equation-number clashes.
- **Empirical**: appendix Tables A.2–A.4 estimate equation (18) on the same NLSY79 person-years, so the appendix model was folded into `4|a|a` (now 92 parts; chapters 3 and 4; 假设 5–7). 23 spec cards got a 补写 estimating equation, six of them Table I variants that had said 书中没有写出估计式. Tables A.5–A.6 (census/ACS occupation cells) stay a model of their own, now `7|a|8|b`; it imports the prediction `2|b|a|1|b` (单卡), and its false-share Link to `2|b|a|3` is gone. The O*NET measures are a concept model, now `8|a|a` (fold only). Evidence dialogue: `4|a|a` 30 `对照` Links (against `5|b|a`, `5|c|a`); `7|a|8|b` 0 pairs. Cost about 5.2 CNY (official, an upper bound). All empirical gates clean.

### Organize

Top line now: 1 I. 引言, 2 II. 模型, 3 III. 数据, 4 IV. NLSY79 结果, 5 V., 6 VI. 结论, 7 在线附录 A 补充图表, 8 在线附录 B 数据附录 (8|a §1 O*NET, 8|b §2 Occ1990dd), 9 在线附录 C 模型附录 (9|a §1, 9|b §2, 9|c §3), 10 Term list (110), 11 Cited list (85). Two new summary leads (B, C); 797 Cards. No Card or Link deleted, no model split. `check.mjs` no structural fault; every model's checks clean; `list_broken_links` empty; backslash scan 0 hits; Term relink 0 changes on a second run.

### Open items (appendix)

1. The import of `7|a|8|b` could be `2|b|a|4|c` (the S*×θ time prediction) instead of `2|b|a|1|b`.
2. 15 spec cards of `4|b|a`, `4|c|b`, `5|b|a`, `5|c|a` still lack an estimating equation (part of the open count in formal-estimating-equations).
3. The installed `check-settings.mjs` does not flag a missing estimating equation.
4. The evidence-dialogue judged record is keyed by address, so an organize that renames addresses makes it judge and pay for old pairs again.
5. A Model Card whose head links another model's part makes the model brief skip it as built (this run's `13|a`); the brief could read a false share as no part.
6. Appendix and main-text equation numbers clash at (7) and (12)–(17); the notation card maps them.
