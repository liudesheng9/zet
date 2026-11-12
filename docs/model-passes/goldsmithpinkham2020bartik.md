# Model passes: goldsmithpinkham2020bartik (2026-11-12)

Goldsmith-Pinkham, Sorkin & Swift (2020), "Bartik Instruments: What, When, Why, and How", AER 110(8). Digest wf_80084296-a4e + resume wf_aee79c2c-c41 (run report complete, 0 gaps). Live box (`D:\zt`), on the owner's request (`/zt-post-digest goldsmithpinkham2020bartik`), in the order of the `zt-post-digest` skill. The tree was not locked. Addresses below write `|` as in the box.

## Stage 1: fold

Backup: `D:\zt\backups\zt-before-goldsmithpinkham2020bartik-fold.sqlite3` (`VACUUM INTO`, integrity ok).

Survey before: 10 Model Cards (7 formal, 2 concept, 1 empirical); candidate group G2 = 2|a, 3|a, 4|a, 5|a, 7|a by rests-on Links.

| Model Card | Model | Surviving head | Reason |
|---|---|---|---|
| 2\|a | Bartik estimator | 2\|a | the TSLS system of §I.A: the primitives (locations, industries, periods, shares, growth rates) every other card uses |
| 1\|c | Bartik estimator | 2\|a | the verbal version in the introduction: becomes the intuition chapter |
| 3\|a | Bartik estimator | 2\|a | continues 2\|a: the same system, proves Bartik = GMM with shares as instruments, W = GG′ (§I) |
| 4\|a | Bartik estimator | 2\|a | continues 2\|a: normalization invariance of the same first stage (§I, note 1) |
| 5\|a | Bartik estimator | 2\|a | continues 2\|a: consistency of the same estimator under strict exogeneity of shares (§II.A) |
| 6\|a | Bartik estimator | 2\|a | verbal discussion of when the same identification holds: shares vs shocks (§II.B) |
| 7\|a | Bartik estimator | 2\|a | continues 2\|a: Rotemberg decomposition of the same estimator (§III) |
| 10\|b | Bartik estimator | 2\|a | a variant on the same setting that changes one assumption: β₀ → β_l (§IV.A) |
| 11\|a | Bartik estimator | 2\|a | continues 10\|b: Assumption 3 and Proposition 4 under that heterogeneity (§IV.B) |
| 15\|a | immigrant enclave | 15\|a | empirical, its own data and estimand (§VII); stays alone |

Two models. Method: the session's `fold.mjs` (strip **结构**/**引入**, mask Links to Model parts, park the parts, retype, move the head `alone` after the last main-line card, return the parts, restore the body with the rename map), one Card per run, in book order. Each former head's Idea-note chain moved to its section anchor (1|c|1–16, 3|a|1–10, 4|a|1–4, 6|a|1–8, 7|a|1–4, 11|a|1–8). The 引入 line of 10|b (单卡 2|a|a) was dropped: it names a part of the head.

New chapters on the main line of 2|a after the setting chain 2|a|a|1–4: `2|a|a|5` 第 1 章：直觉（引言） … `2|a|a|12` 第 8 章：命题 4 与异质性下的权重（§IV.B）.

Gate: survey lists exactly 2|a (formal, 62 parts) and 15|a (empirical, 43 parts); `list_broken_links` empty; `check-model.mjs` shows every former head as a part; its only faults are 8 `unanchored` chapter cards, left to the restructure. Term relink: 2 Cards, 2 links.

## Stage 2: settings (strict)

Backup: `D:\zt\backups\zt-before-goldsmithpinkham2020bartik-settings.sqlite3` (integrity ok). Done by one subagent (Opus, medium) under `zt-model-settings`. S = `2|a|a`.

```
S 设定：完整面板两阶段系统 (1)           原始设定
├ a 符号表与读法                         (new: 80 rows in five blocks + term table)
├ b 假设：地点间 i.i.d.、控制变量严格外生、共同增长率固定   (was S|2 on the main line)
├ c 设定：单一横截面 (引言)              (old 1|c setting)
│   c|1 份额和为 1 → c|2 三个特例与 Bartik/GMM 估计量 → c|3 Rotemberg 定义
├ d 设定：L→∞，T、K 固定
│   d|a 假定 1 相关性;  d|b 假定 2 严格外生;  d|c 份额读法与冲击读法
└ e 设定：异质性变体 (7)(8)               变体
    e|a 假设：控制变量离散饱和，β_l 矩良好;  e|b 设定：受限异质性;  e|c 恰好识别估计量;  e|d 假定 3
```

Normalization (c|1) stands before the special cases because the two-industry case uses z_l2 = 1 − z_l1. Equations added (owner rule of 2026-11-12, both stages and reduced forms): the cross-section reduced form (补写), the two-period panel structural equation (补写), the two Bartik constructions (stated by the paper), the reduced form of (7)–(8) (补写); the panel reduced form in S marked 补写. Moved to chapters: the (5) block (to chapter 5), the Rotemberg sum Σα_kβ̂_k (a Proposition 3 result, to chapter 6). One restated A2 formula dropped; the formula stays in d|b. Bare 假定 1/2/3 in 12 cards became Links. Chapters renumbered: 第 1–8 章 are now `2|a|a|4`–`2|a|a|11`.

15|a: every 规格 card already has an 估计式 with both IV stages; eight openings `（补：书中未写）` were reworded to the standard 补写 wording.

Gate: `check-settings` 2|a: `settings: all in the setting part, as a dependency tree`; 15|a: `the data tree and the specification ladder are in place`; relations show no relaxes/objects; check-premises only two old roadmap pointers; `list_broken_links` empty; Cards 469 → 470 (the notation card). Term relink: 15 Cards, 59 links. Open: 8 unanchored chapters (Stage 3); S|1–3 derivations of the old head still before chapter 1; d restates (1) with τ where (1) has ρ (kept: mathematics unchanged).

## Stage 3: restructure (spines)

Backup: `D:\zt\backups\zt-before-goldsmithpinkham2020bartik-spine.sqlite3`. One subagent under `zt-model-restructure`.

`2|a` (formal, intuition spine). `tree-mover.mjs` made 59 moves (park `2|a|b`). Main line: 设定 `2|a|a` → 第 1–8 章 `2|a|a|1`–`|8`; each result is a Side branch of its chapter and its proof chain a Side branch of the result. The three derivations of the old head (`S|1–3`) became result `1|b` (identification from initial shares) with its two proofs `1|b|a`, `1|b|b` in chapter 1. Results: ch1 `1|a`, `1|b`; ch2 `2|a`, `2|b`, `2|c` (命题 1); ch3 `3|a`; ch4 `4|a` (命题 2, aside 注 6 `4|b`); ch5 `5|a` (two arguments `5|a|a`, `5|a|b`); ch6 `6|a` (命题 3, aside `6|a|b`); ch7 none (地点异质性: points to settings `e`, `e|b`, `e|d`); ch8 `8|a` (命题 4), `8|b` (式 (10), aside `8|b|b`). Three DeepSeek jobs (opencode-go/deepseek-v4.1-flash, high; conversations deleted, workspace removed) drafted the 8 chapter cards and 11 result cards; `check-drafts.mjs` found no lost formula (only cross-book Links outside the tree, kept). Fixes by hand: settings `cite` (bare 假定 1/2/3 → Links in ch4, 4|a, ch8), ch7 section renamed so it states no assumption, notation rows `$g_j$`, `$\tilde\tau_t$`, `$\omega_{lk}$` repointed to the result cards where the reader now meets them (`$\tilde\tau_t$` and `$g_j$` defined there), notation row `$\tilde\tau_t$` reworded (its word 代入 made a forward premise on chapter 2), two TeX faults from Stage 2 repaired in the notation card (`E[\cdot\mid\cdot]`, `\beta_0` with a lost backslash). **结构** rebuilt; **分部** added (设定 / Bartik 即份额 GMM from ch1 / 识别与一致性 from ch4 / 权重与异质性 from ch6). Network: 设定 → {1, 2 → 3, 4 → 5, 6 → 7, 8}.

`15|a` (empirical, evidence spine). No moves: the main line already ran data → hypothesis → design → baseline specification → 4 chapters. Each main-line card got a **直觉** opening; the chapter cards take the format **直觉** / **本章估计** / **承上启下**; chapters 2–4 rest on chapter 1 (`依赖 [[15|a|a|4]]`), so robustness and Rotemberg hang under the main estimates; chapter 4 points to the limitation `15|a|b` (no longer loose). **结构** rebuilt.

Gates (both heads): check-model self_sufficient true, no fault; check-network `network: clean` (advice: ch7 of 2|a has no result card, intended); check-premises none; check-settings clean; `list_broken_links` empty; Cards 470 → 470 (none created). Term relink: 29 Cards, 117 links.

## Stage 4: imports and explanations

Backup: `D:\zt\backups\zt-before-goldsmithpinkham2020bartik-imports.sqlite3`. Done under `zt-model-import`, 2|a first, then 15|a.

`2|a` (formal). `imports.mjs check` passed and `plan` said `nothing to import` before any change: the model rests on no part of another model. Its cross-book Links (对应 `becker2025division/2|g|a|2|a` in chapter 4, `acemoglu2001colonial/3|d|a` and `atkinson2015lectures/17|e|b|a|1` in chapter 6) are comparisons, not premises: no import.

`15|a` (empirical). `plan` named one possible copy: the hypothesis `15|a|a|1` against chapters 4 and 6 of 2|a, which it cited with `参照`. The hypothesis restates no setting; it names the theory's predictions. By the empirical rule it now rests on the two result cards (`依赖` in place of `参照` on the chapter cards), and `apply` wrote the two import lines:

- `单卡 [[goldsmithpinkham2020bartik/2|a|a|4|a]]`：命题 2, the consistency the hypothesis tests.
- `单卡 [[goldsmithpinkham2020bartik/2|a|a|6|a]]`：命题 3, the Rotemberg decomposition the hypothesis and chapter 4 use.

Both stay `单卡` (a test takes the prediction, not its proof); tolerated 0 → 6 (the two proof chains and 假定 1/2 stay in 2|a), as intended. The second `plan` said `nothing to import`. The text after `：` was reworded by hand to say what the model uses. No notation change: 15|a keeps its own 变量表. The 对照 to `deming2015growing/7|a|6` stays a Link for `zt-evidence-dialogue`.

Explanations: `explain.mjs plan` 2|a found 3 Derivation parts (the proof chain of 命题 2: `2|a|a|4|a|a`, `|a|1`, `|a|2`) in one job; 15|a `nothing to explain`. One DeepSeek job, `opencode-go/deepseek-v4.1-flash`, high, 76 s, 1 turn (no token count in the turn report): done 3, blocked 0; `apply` applied 3, refused 0; `finish` 2|a: `explained 3 part(s); Term links changed 1 Card(s), second plan 0; left 0; refused 0`. Conversation deleted, workspace removed.

Gates (both heads): `imports.mjs check` exit 0; check-model `self_sufficient true`; check-network `network: clean`; check-premises `none used before their card`; check-settings 2|a `all in the setting part, as a dependency tree` (advice only: 8|b names an assumption), 15|a `the data tree and the specification ladder are in place`; `list_broken_links` empty; no Card created. Term relink: 1 Card, 2 links.

Open: none for this stage. Stage 5 (illustrate 2|a) and the evidence dialogue of 15|a follow.

## Stage 5: illustrate

Backup: `D:\zt\backups\zt-before-goldsmithpinkham2020bartik-illustrate.sqlite3`. Done under `zt-model-illustrate` on `2|a` only (15|a is empirical: not illustrated, by the owner's rule). Module: `~/.claude/skills/zt-model-illustrate/modules/goldsmithpinkham2020bartik-2_a.mjs`.

Figures (16):

- `2|a` (**图**): 一、估计的顺序 (given shares and national growth → B → first stage → (1) → β̂, with the chapter order); 二、the estimator structure (exogenous z_lk0, g_kt, g̃_lkt, ε_lt → B_lt and x_lt → first stage and (1) → Bartik TSLS, share GMM with W = GG′ (ch2), the heterogeneity branch β_l (e, ch8), the Rotemberg row β̂_k with α̂_k (ch6), the exclusion restriction as a dashed arc opening d|b); 三、eight panels, one per chapter. Every box opens its Card.
- `|a`: initial share bar Z_l0, G_t → B_lt, the growth split, x_lt, first stage, (1), M_D residualization.
- `|b`: H1 independent locations under a fixed G_K; H2, H3.
- `|c`: mosaic (width = z_lk, height = g_lk split into g_k and g̃_lk; axes named); B_l and x_l as areas.
- `|c|1`: share bars summing to 1, K − 1 free shares, G and j, first stage.
- `|c|2`: two-industry one- and two-period cases; Z × G = B; the two estimators (no equivalence drawn).
- `|c|3`: columns Z_k → β̂_k; bar plot of g_k Z_k′X⊥ (示意, axes named) and α̂_k as a bar's share.
- `|d`: L × K share table growing in L, timeline base period → shocks, the TSLS estimator.
- `|d|a`: z_lk0·1(t=s) → x_lt via C_k,s, the C table, number line of the rank condition with 0 excluded.
- `|d|b`: z → x → y with the cut z–ε link; constrained only where g_k ≠ 0.
- `|d|c`: one exposure structure, share branch (opens d|b) against shock branch.
- `|e`: y against x with one slope β_l per location (示意); chain z_lk →π_lk→ x_l →β_l→ y_l with the unobserved errors.
- `|e|a`: saturated dummy cells; density of β_l (示意).
- `|e|b`: within-location straight line, equal shocks give equal effects; another location's slope.
- `|e|c`: residualize y, x, z_lk on D_l → β̂_k.
- `|e|d`: number lines of π_lk (same sign allowed, sign change excluded); the conditional moment.

Intuitions: 15 **直觉** sections rewritten (every setting and assumption card, including 假定 1–3 and `e|b`, which had none). The digest's sections were replaced because several held address Links, `（补：书中未写）` tags or result wording. No 定义 part exists in the chapters (no facet 定义). One fix after the first write: `Δ` in the `e|b` figure was not in the notation card (check-settings `uncovered`); the figure now says it in words.

Gates: `verify.mjs --model 2|a`: `16 chart(s) in 64 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉` (all 32 PNGs reviewed, light and dark); a rerun of `apply.mjs` prints `same` for all 16; check-settings `settings: all in the setting part, as a dependency tree`; check-model `self_sufficient true`; check-network `network: clean`; check-premises `premises: none used before their card`; `list_broken_links` empty. Term relink (last): 8 Cards, 14 links. zt-check on the 16 Cards: 6 R3 faults, all semicolons in older text (summaries and settings of `|a`, `|c|1`, `|d`, `|d|c`, `|e`), none in a figure or a 直觉.

Open: those 6 R3 semicolons (text this pass may not change).

## Stage 6: evidence dialogue

Backup: `D:\zt\backups\zt-before-goldsmithpinkham2020bartik-dialogue.sqlite3`. Done under `zt-evidence-dialogue` on `15|a` only (2|a is formal: no dialogue).

Step 0: `term-relink.ps1` before the survey: 0 Cards, 0 links (the book's Term links were current after Stage 5).

Survey: `dialogue.mjs survey goldsmithpinkham2020bartik/15|a`: `{"ours":9,"theirs":886,"pairs":0,"messages":[]}`. Estimand Terms: 技能组 (`1|a|cf`), 残差对数工资差距 (`1|a|ck`), 工时比 (`1|a|cj`), 同等学力 (`1|a|ce`), and the book root. No other book in the box estimates the inverse elasticity of substitution between immigrants and natives. A by-hand search confirmed it: the 移民 Cards of other books (acemoglu2001colonial, acemoglu2025nobel, atkinson2015lectures) are about settlement and colonial policy, and the 替代弹性 Finding Cards (atkinson2015lectures commodity-tax and Harberger elasticities, becker2025division 5.67 trade elasticity) state other estimands. The 对照 to `deming2015growing/7|a|6` written in Stage 4 is already a Link, so the survey does not propose it again.

Judging: none. No pair, so no DeepSeek conversation was started and `apply` did not run (cost 0 CNY).

Gates: `dialogue.mjs check`: `{"broken":0,"proposed_again":0,"ok":true}`; check-model `self_sufficient true`; check-network `network: clean`; check-settings `settings: the data tree and the specification ladder are in place`; `list_broken_links` (agent) empty. Term relink (last): 0 Cards, 0 links. No Card was created or changed.

Open: none. If a later book has an immigrant–native substitution elasticity (for example Card 2009, or Ottaviano–Peri), a new survey of 15|a will pair it.

## Stage 7: literature organize

Backup: `D:\zt\backups\zt-before-goldsmithpinkham2020bartik-organize.sqlite3`. Done under `zt-literature-organize` (one `batch.mjs` batch of 30 moves, `leads.mjs` for the leads, hand edits for crosser heads, the root index rewritten). No DeepSeek job: the six summaries were written from the Cards under each head and the old group anchors.

Final top line:

```
1   引言 (unchanged head; findings 1|d–h, structure Card 1|i, intro chain 1|j)
2   I.   等价性 (new summary)   2|a §I preamble, 2|b §I.A (holds model 2|b|a), 2|c §I.B, 2|d §I.C
3   II.  渐近一致性 (new summary) 3|a §II preamble, 3|b §II.A, 3|c §II.B
4   III. 黑箱 (rewritten)        4|b preamble, 4|c §III.A, 4|d §III.B, 4|e §III.C
5   IV.  异质性效应 (new summary) 5|a preamble, 5|b §IV.A, 5|c §IV.B
6   V.   可信性检验 (rewritten)   6|a preamble, 6|b §V.A, 6|c §V.B, 6|d §V.C
7   VI.  实证例子一 (rewritten)   7|a preamble, 7|h A, 7|i B, 7|j C, 7|k D, 7|l E
8   VII. 实证例子二 (retitled, source line added; model 8|a, findings C then E)
9   VIII. 总结 (unchanged)
10  Term list
11  Cited list
```

Leads: 3 demoted (old anchors 2, 5, 10 became the heads of §I.A, §II.A, §IV.A under new summaries), 3 rewritten (old anchors 7, 12, 13 now speak for §III, §V, §VI). Old 15 (§VII) kept its summary text and got a title in the source-name form. The root index lists the new top line with the Term and Cited lists.

Crossers, settled by the source's own position: old 4 (§I proof + §I.C) joined §I.B as an overview Card of the proof and Note 1 (`2|c|c`), with the §I.C note as `2|d`; old 6 (§II proof + §II.B) gave the proof cards to §II.A and stays the §II.B head; old 9 (§III.B + C) stays §III.B, §III.C is `4|e`; old 11 (§IV.B) gave Assumption 3, its footnotes and Proposition 4 to §IV.A (they stand before `# B.` in the source) and stays the §IV.B head; old 13/14 split into §VI.A–E (old 14 = §VI.E, its §VI.D card `7|k`). Sentences each move made false were corrected, and titles retitled.

Models: formal `2|a` → `2|b|a` (under §I.A, where it starts), empirical `15|a` → `8|a`. Inner structure unchanged.

Gates: `check.mjs`: lists at the tail, no broken Link, no whole unit changed; Cards 470 → 473 (the 3 new summaries). Both heads: check-model `self_sufficient true`; check-network `network: clean`; check-premises `none used before their card`; check-settings 2|b|a `all in the setting part, as a dependency tree`, 8|a `the data tree and the specification ladder are in place`; `imports.mjs check` passes. `list_broken_links` (agent) empty. Term relink: run 1 6 Cards, 13 links; run 2 0 Cards changed.

Open: the source lines of `5|b|2` (+a, b, c) and `5|b|3` still read `§IV.B（…）` although the source puts them in §IV.A (not changed: note text). Figures 3 and 4 and findings `9|b–d` stay in §VIII, where the source file places them, though they show §VII data. §VII has no subsection heads (its A, B, D content lives inside model 8|a). The structure Card `1|i` still lists the old group anchors (its Links follow the Cards).

## §VI empirical model (7|a): build and passes

Backup: `D:\zt\backups\zt-before-goldsmithpinkham2020bartik-7a-passes.sqlite3`. The model `7|a` (empirical, 59 parts) was built by the digest's model layer (wf_6d20e928-12e) for §VI "Empirical example I: canonical setting" and sits as the first Side branch of the §VI division `7`, as `8|a` does in §VII. The passes followed the treatment of `8|a`: settings, restructure, import, evidence dialogue. No illustrate pass (empirical) and no literature organize (the tree was organized in Stage 7). `2|b|a` and `8|a` did not change.

**Settings (data tree and specification ladder).** Two moves. The just-identified single-share specification went under the over-identified share TSLS (`3|e` → `3|d|d`), as in `8|a`. The share regression (Table 2) left the main line and became a variant of the baseline (`4` → `3|e`, `alone`), so the main line reads data → hypothesis → design → baseline → chapters 0–4 (`4`–`8`). All eleven 规格 cards were rewritten in the format of `8|a` (summary, 见/依赖, **估计式**, **各项** 1–7, **相对基准的改动**) and in the source's notation (y = income growth, x = employment growth, z_lk,1980 shares, g_kt growth, B_lt Bartik), in place of the digest's Δln w, Δln E, s_lk, z_lt. Every 估计式 is 补写 with the standard opening; the `（补：书中未写）` tags and file-line references were removed. Equations added: the Bartik construction B_lt = Σ_k z_lk,1980 g_kt in the baseline, the leave-out construction B^LO in `3|c`, the first stage in the MBTSLS, LIML and HFUL cards (they had only the second stage), the first stage and the §III.C aggregation α_k = Σ_t α_kt, β_k = Σ_t (α_kt/α_k) β_kt in the single-share card, and the Bartik-on-characteristics regression in the share-regression card. One correction: the digest's over-identified first stage Σ_k π_k s_lk τ_t (one coefficient per industry, which period fixed effects and CZ fixed effects absorb) became Σ_k π_kt z_lk,1980 (industry × period, as the Table 3 note says). The baseline's growth rates are those of all CZs (Table 3 row 3, 1.22); the leave-out version is `3|c` (row 2, 1.28). The 变量表 was rewritten (30 rows). The measure card `b` got its **起点**.

**Restructure (evidence spine).** No further moves. **直觉** leads were added to the data, hypothesis, design and baseline cards. The design card now says that Table 3 reports both growth-rate versions. Chapters 2 and 3 rest on chapter 1 and chapter 4 on chapter 0, so the network reads 规格 → {0 → 4, 1 → 3} and 设计 → 2. **结构** was rebuilt.

**Imports.** The hypothesis `7|a|a|1` cited the whole Model Card `2|b|a` with `参照`. It now rests on the two results, and `apply` wrote the two lines that `8|a` also has:

- `单卡 [[goldsmithpinkham2020bartik/2|b|a|a|4|a]]`：命题 2, the consistency the hypothesis tests.
- `单卡 [[goldsmithpinkham2020bartik/2|b|a|a|6|a]]`：命题 3, the Rotemberg decomposition that the hypothesis and chapter 0 use.

Tolerated 0 → 6 (proof chains and 假定 1/2 stay in `2|b|a`), as intended. A trial `依赖` from chapter 0 to 命题 3 made `plan` propose `整块` for `6|a`. The line was removed, so both imports stay `单卡` (a test takes the prediction, not its proof), and the second `plan` said `nothing to import`. `explain.mjs plan`: `nothing to explain`.

**Evidence dialogue.** Step 0 relink: 17 Cards, 62 links. Survey: `{"ours":23,"theirs":895,"pairs":10}`. All 10 pairs were against `8|a`, joined only by generic Terms (点估计, Rotemberg 权重, 过度识别检验). One DeepSeek conversation (`opencode-go/deepseek-v4.1-flash`, high, 12 s, 1 turn) judged all 10 `same: false`. `apply` wrote 0 links (no change to `8|a`), and the judged record keeps the 10 pairs. A search by hand found no other local inverse labor-supply elasticity in the box. The labor-supply elasticities of atkinson2015lectures (Heckman 1974, Cain 1976, Gronau 1973 Cited Cards) are individual-level elasticities, not this estimand. The conversation was deleted and the workspace removed.

Gates. `7|a`: check-model `self_sufficient true`; check-network `network: clean`; check-premises `premises: none used before their card`; check-settings `settings: the data tree and the specification ladder are in place`; `imports.mjs check` passes (exit 0); `dialogue.mjs check` `{"broken":0,"proposed_again":0,"ok":true}`. `2|b|a`: `self_sufficient true`, `network: clean`, premises none, `settings: all in the setting part, as a dependency tree`, imports pass. `8|a`: `self_sufficient true`, `network: clean`, premises none, `the data tree and the specification ladder are in place`, imports pass, dialogue ok. `check.mjs` on the book: `no fault`. `list_broken_links` (agent) empty. Cards 0 created. Term relink (last): run 1 0 Cards, run 2 0 Cards.

Open: the design card's title still reads "全国留一增长率" (the text now names both versions). The MBTSLS, LIML and HFUL cards write the TSLS system and name the estimator in words; no k-class formula is given. Chapter 2 hangs under the design card on the board (its tests rest on the assumptions), not under chapter 1.
