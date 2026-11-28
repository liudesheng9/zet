# Model passes: anton1995startups

Owner: Xingtong Chen

Anton and Yao (1995), *Start-ups, Spin-offs, and Internal Projects* (Scholar Archive Work 30159). Digest: `D:\zt-digest\anton1995startups\run-report.json` (`completeness.complete: true`).

## Stage 1: fold

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-postdigest-2026-11-27.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok).

The digest left 1 Model Card. Decision by nature:

| Model Card | Model (surviving head) | Level | Reason |
|---|---|---|---|
| 2\|a | own (2\|a) | formal | Single formal model (12 parts, `formal_structured=true`, `claim_objects: strategic-allocation, choice-optimum`): employee private invention without IP rights choosing among start-up, spin-off, and internal project under 4-state reduced-form profits. |

No fold required (0 writes).

### Gate

- `model-survey.mjs anton1995startups`: 1 Model Card (`2|a`, formal, 12 parts, `formal_structured=true`).
- `zt-check --model 'anton1995startups/2|a' --stage fold`: pass (`members 12 {main:2, off:1, side:9}`, `loose 0`, `self_sufficient true`, `formal_structured true`).
- `list_broken_links`: verified clean (0 broken links in `anton1995startups/`).
- Term relink: not needed (no Cards edited in Stage 1).

## Stage 2: settings

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-stage2-settings-2026-11-27.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok).

`anton1995startups/2|a` (formal, 12 parts) already passed strict settings checks out of the digest with 0 faults (0 writes required):
- Setting card `2|a|a` (introduces $q, \alpha, \pi_0, \sigma_0, \sigma_{\mathrm{M}}, \pi_{\mathrm{L}}, \pi_{\mathrm{M}}, \sigma_{\mathrm{D}}, \pi_{\mathrm{D}}$), notation card `2|a|a|a`, Side setting cards `2|a|a|b` (introduces $R_{\mathrm{M}}, R_{\mathrm{D}}, R_0, R_{\sigma}, R_x$) and `2|a|a|d` (introduces $L$).
- All 7 `uses` advice lines inspected via `--format json`: mid-sentence prose references in `2|a|a|1`, `2|a|a|1|a`, and limitation card `2|a|b` (all stay per skill rule).

### Gate

- `zt-check --model 'anton1995startups/2|a' --settings --symbols`: `ok: true` (`settings: all in the setting part`, 0 faults).
- `zt-check --model 'anton1995startups/2|a' --stage settings`: `pass` (`self_sufficient true`, `formal_structured true`).
- `list_broken_links`: 0 broken.
- Term relink: not needed (0 Cards edited).

## Stage 3: restructure

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-stage3-restructure-2026-11-27.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok).

`anton1995startups/2|a` (formal, 12 parts, 1 chapter) already passed all Stage 3 gates out of the digest with 0 faults (0 writes required):
- Spine: `2|a|a` (设定, 6 parts) → `2|a|a|1` (`第 1 章：利润蕴含与期望收益`, 5 parts, 2 results), plus limitation card `2|a|b`.

### Gate

- `zt-check --model 'anton1995startups/2|a' --stage restructure`: `pass` (`self_sufficient true`, `formal_structured true`, `loose 0`).
- `zt-check --model 'anton1995startups/2|a' --network`: `network: clean` (3 blocks, 2 layers).
- `zt-check --model 'anton1995startups/2|a' --premises`: `premises: none used before their card`.
- `zt-check --model 'anton1995startups/2|a' --skip-shared`: `faults: 0` (1 advisory R20 only).
- `zt-check --model 'anton1995startups/2|a' --settings --symbols`: `ok: true`.
- `list_broken_links`: 0 broken.
- Term relink: not needed (0 Cards edited).

## Stage 4: imports and explanations

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-stage4-import-2026-11-27.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok).

- Imports (`anton1995startups/2|a`): single self-contained formal model in the book; `imports.mjs check` and `imports.mjs plan` report `nothing to import` (`0` tolerated premises, `0` false shares, `0` legacy shares).
- Explanation refresh (Step 8): `zt-digest model-explain -Step plan -Model 'anton1995startups/2|a'` returned `nothing to explain` (`parts: 0`, `jobs: []`).

### Gate

- `imports.mjs check 'anton1995startups/2|a'`: exit 0 (`takes what it uses from other models by import lines`).
- `zt-check --model 'anton1995startups/2|a' --stage final`: pass (`self_sufficient true`, `formal_structured true`, `loose 0`).
- `zt-check --model 'anton1995startups/2|a' --skip-shared`: `faults: 0`.
- `list_broken_links`: 0 broken.
- Term relink: not needed (0 Cards edited).

## Stage 5: evidence dialogue

Not applicable (`anton1995startups` holds only formal model `2|a`; Stage 5 applies to empirical models only).

## Stage 6: literature organize

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run), under `zt-literature-organize`. Backup: `D:\zt\backups\zt-before-iof10-stage6-organize-2026-11-27.sqlite3`. No subagent and no DeepSeek job ran.

### Source divisions

From `book.zh.md` headings: Abstract + `1. 引言`, `2. 模型`, `3. 初创企业与衍生企业的事后激励分析` (`3.1 基准分析`, `3.2 议价能力`, `3.3 专用性创新与通用性创新`, `3.4 外部选择`), `4. 事前激励`, `5. 讨论`, `6. 拓展分析：产权与其他制度安排` (preamble + `6.1 部分产权保护`, `6.2 更强的商业秘密保护`, `6.3 雇主实施权`), `附录：证明`, followed by the Term list and Cited list at the tail.

### Final tree (top line and divisions)

```
1  1. 引言：弱产权下雇员创业与衍生企业的激励冲突 (13)     c 摘要 + §1 链 (1|c..1|c|10) · d 结构卡 (1|d)
2  2. 模型：无产权与有限责任下的雇员发明博弈 (18)          a 形式模型 2|a (12 部件 + §2 笔记链 2|a|1..4)
3  3. 初创企业与衍生企业的事后激励分析 (9, rewrite)        a 3.1 (3|a..3|a|1, 3|a|1|a) · b 3.2 · c 3.3 (3|c|a) · d 3.4 (3|d..3|d|1)
4  4. 事前激励：有限责任与逆向选择下事前契约的财富阈值 (7)  a §4 链 (4|a..4|a|5)
5  5. 讨论：资金约束、互补性资产与声誉如何影响初创企业 (8)  a §5 链 (5|a..5|a|4, 5|a|3|a, 5|a|4|a)
6  6. 拓展分析：部分产权、商业秘密保护与雇主实施权 (5, rewrite) a 开篇 (§6) · b 6.1 · c 6.2 · d 6.3
7  附录：引理 1、命题 2 与命题 3 的证明 (5)                a 附录链 (7|a..7|a|3)
8  术语表 (Term list, from 1|b) (25)
9  参考文献 (Cited list, from 1|a) (13)
```

### Crossers and how each was settled

- Old `1` held §1 and both reference lists (`1|a` Cited, `1|b` Term): moved Term list to `8` and Cited list to `9` (`batch.mjs`, 1 atomic batch, 8 moves, 46 renamed, 515 link rewrites, 0 broken). Structure Card `1|d` stays under `1`.
- Old `3` ran `§3.1`–`§3.4` on a single Direct chain (`3|a..3|a|5`): split into four Side branches `3|a` (`3.1`), `3|b` (`3.2`), `3|c` (`3.3`, with `3|c|a`), `3|d` (`3.4`, with `3|d|1`).
- Old `6` ran `§6` preamble and `§6.1`–`§6.3` on a single Direct chain (`6|a..6|a|3`): split into `6|a` (preamble), `6|b` (`6.1`), `6|c` (`6.2`), `6|d` (`6.3`).
- Whole unit: formal Model Card `anton1995startups/2|a` stayed intact at `anton1995startups/2|a` (12 model parts; `2|a|1..4` are all `§2` notes and remain on `2|a`'s Direct chain, so unit size is unchanged at 17 Cards).
- Summary leads & text: `leads.mjs --commit` rewrote `3` and `6` with `小节索引：` linking every subdivision; digest-group prefixes (`第 1/2/4 组：`) removed from `1`, `2`, `4` with `来源` lines added; subdivision head titles prefixed with `3.1`–`3.4` and `6.1`–`6.3`; root index (`anton1995startups`) and structure Card (`1|d`) updated to final addresses.

### Gate

- `zt-digest term-link -Step relink -CitationKey anton1995startups`: `ok: true`, `cards_changed: 0`, `links_added: 0`, `links_repaired: 0`; `-PlanOnly`: `cards_changed: 0`, `links_added: 0`.
- `check.mjs anton1995startups <snapshot>` & `zt-check --tree anton1995startups --snapshot <snapshot>`: `no fault` (exit 0; 104 Cards before and after, 0 unit-size changes).
- `zt-check --cards` on root, `1|d`, `1`–`7`, `3|a`–`3|d`, `6|b`–`6|d`: `faults: 0` (exit 0; advisory L1/L2/I4 only).
- `model-survey.mjs anton1995startups`: 1 formal Model Card at exact final address `anton1995startups/2|a` (12 parts, `formal_structured=true`).
- Formal model gates at `anton1995startups/2|a`: `zt-check --model 'anton1995startups/2|a' --stage final` (`pass`, exit 0), `--settings --symbols` (`settings: all in the setting part`, exit 0), `--network` (`network: clean`, 3 blocks, 2 layers, exit 0).
- `list_broken_links` for `anton1995startups/`: empty (`[]`).

## Stage 7: illustrate

Date: 2026-11-28, live box (`D:\zt`, user-authorized autonomous run), under `zt-model-illustrate`. Backup: `D:\zt\backups\zt-before-iof10-stage7-illustrate-2026-11-27.sqlite3`.

- Module: `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\anton1995startups-2_a.mjs` (`head = 'anton1995startups/2|a'`).
- Cards illustrated (5 charts, 4 `**直觉**` sections; no owned `定义` parts in chapters):
  - `anton1995startups/2|a` (Model Card `**图**`, 3 `part()` subcharts: `一 · 博弈顺序`, `二 · 因果结构`, `三 · 比较维度`).
  - `anton1995startups/2|a|a` (设定: timing flow + 2×2 four-state reduced-form profit matrix + refreshed `**直觉**`).
  - `anton1995startups/2|a|a|b` (设定：契约空间与不完备性: ex-post vs ex-ante contract slots + verifiability/incompleteness flow + refreshed `**直觉**`).
  - `anton1995startups/2|a|a|c` (假设 1：基本利润排序与盈利性: schematic joint-profit stacked bar chart with labeled axes + profitability boxes + refreshed `**直觉**` without stating derived Chapter 1 results).
  - `anton1995startups/2|a|a|d` (假设 2：雇员有限责任约束: $R_x \ge -L$ number line with feasible/excluded regions and contract scopes + refreshed `**直觉**`).
- Visual inspection: opened and inspected all 16 light and dark PNGs (including `-part1/2/3-*`) via `read_file`; fixed 4 layout polish items on first pass (2×2 grid for part 1, non-crossing vertical $\alpha$ channel in part 2, wider `雇员三选一` box in `2|a|a`, wider arrow gaps in `2|a|a|b`, bottom-aligned axis line in `2|a|a|d`) and re-verified all updated light and dark PNGs with zero text overlap, zero clipping, and all TeX/SVG subscripts rendered cleanly.

### Gate

- `zt-digest term-link -Step relink -CitationKey anton1995startups`: `ok: true`, `cards_changed: 3`, `links_added: 5`, `links_repaired: 0`; `-PlanOnly`: `cards_changed: 0`, `links_added: 0`.
- Idempotence (`node scripts/apply.mjs modules/anton1995startups-2_a.mjs` post-relink): `same` for all 5 Cards.
- `zt-check --model 'anton1995startups/2|a' --charts`: exit 0 (`5 chart(s) in 13 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 card(s) without a full 直觉`).
- `node scripts/verify.mjs --model 'anton1995startups/2|a'`: exit 0 (`5 chart(s) in 13 card(s); 0 lint fault(s); 0 render failure(s); 0 card(s) without a figure; 0 card(s) without a full 直觉`).
- `zt-check --model 'anton1995startups/2|a' --settings --symbols`: exit 0 (`settings: all in the setting part`).
- `zt-check --model 'anton1995startups/2|a' --stage final`: exit 0 (`stage(final) pass`, `self_sufficient true`, `formal_structured true`, `loose 0`).
- `zt-check --model 'anton1995startups/2|a' --network`: exit 0 (`network: clean`, 3 blocks, 2 layers).
- `zt-check --model 'anton1995startups/2|a' --premises`: exit 0 (`premises: none used before their card`).
- `zt-check --model 'anton1995startups/2|a' --skip-shared`: exit 0 (`faults: 0`; 1 advisory R20 only).
- `list_broken_links` for `anton1995startups/`: empty (`[]`).

