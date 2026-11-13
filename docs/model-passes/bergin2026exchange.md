# bergin2026exchange: post-digest model passes

Owner's order (2026-11-13), relayed by the IMEN1 queue session: run the zt-post-digest sequence on the live box while the queue digests assenza2026louder on the same box. Subagents run at effort medium. Digest run report: `complete: true` after the hand cleanup rerun on zt-digest 0.53.2 (bef70bcf).

## Stage 1: fold

Backup: `D:\zt\backups\zt-before-bergin2026exchange-fold.sqlite3` (`VACUUM INTO`, integrity ok).

Survey before: 14 Model Cards (7 formal, 5 empirical, 2 concept), 9 candidate groups.

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| 7\|a | §3.1 | two-country DSGE | 7\|a | formal; the goods-market block (CES indices, price indices, demand) of the paper's one theory model; first formal card, kept as head |
| 8\|a | §3.2 | two-country DSGE | 7\|a | formal; the household block of the same model under capital controls; rests on 7\|a's price index |
| 9\|a | §3.3–3.4 | two-country DSGE | 7\|a | formal; the traded-sector firm block (production, entry and exit, pricing) of the same model |
| 10\|a | §3.5 | two-country DSGE | 7\|a | formal; the government block (tax-financed reserve purchases, full sterilization) of the same model |
| 12\|a | §4.1 | two-country DSGE | 7\|a | formal; solves the same model for the reserve policy's effect on profits and firm value; rests on 9\|a and 10\|a |
| 15\|a | §5.1.2 | two-country DSGE | 7\|a | concept; the firm-location externality is the mechanism of the same model's simulation (home entry lowers the manufacturing price index); becomes a chapter |
| 17\|a | §5.2 | two-country DSGE | 7\|a | formal; a variant that changes one assumption (foreign firm value starts at the exit margin) and keeps the rest; rests on 9\|a, 10\|a, imports 12\|a |
| 18\|a | §5.3 | two-country DSGE | 7\|a | formal; a variant that changes one assumption (infinite planning horizon, T→∞) and keeps the rest; rests on 9\|a, 10\|a, imports 12\|a |
| 2\|a | §1 | two-country DSGE | 7\|a | concept; the introduction's verbal version of the formal model; becomes the model's intuition chapter |
| 6\|a | §2.3 | 45-country panel estimation | 6\|a | empirical; the full estimation (Tables 2–5: within and system-GMM productivity regressions, firm-dynamics regressions, IV), 65 parts; kept as head |
| 2\|b | §1 | 45-country panel estimation | 6\|a | empirical; the introduction's statement of the same estimand on the same data, repeating Tables 2–5 |
| 4\|a | §2.1 | 45-country panel estimation | 6\|a | empirical; the data and measurement built for this estimation, with its descriptive statistics; no other empirical model uses the panel |
| 5\|a | §2.2 | 45-country panel estimation | 6\|a | empirical; the productivity regressions (Table 2) of the same estimand and data |
| 5\|b | §2.2 | 45-country panel estimation | 6\|a | empirical; the firm-dynamics regressions and the undervaluation IV (Tables 3, 5) of the same estimand and data |

Two models: the two-country DSGE (formal, head 7\|a) and the 45-country panel estimation (empirical, head 6\|a). They stay two models: the panel tests the theory's prediction, and a theory and its test are never folded together.

### Fold (executed 2026-11-13, live box)

Method: one Card per run with a copy of the proven per-card fold script (`foldb.mjs`, from the velic2025inflation pass): strip the former head's **结构** and **引入**, mask every Link in its body whose target is a Model Card or Model part of any book (the cross-book Links of 9\|a, 4\|a and 5\|b to atkinson2015lectures, hart1995firms, velic2025inflation and becker2025division parts would otherwise block the retype), park its parts as Side successors of its section anchor, retype it (facet `第 N 章：…`, emphasis core, level cleared), move it `alone` into the head's main line, return the parts under it, and restore the stripped body with the rename map. Idea-note Direct successors (12\|a\|1–6, 15\|a\|1–7) and Idea Side cards (10\|a\|a, 5\|b\|a) went to the section anchor (now 12\|c, 15\|b, 10\|c, 5\|d). Each run was previewed without `--commit`, then committed; every retype and batch returned ok.

**引入** lines: every folded card's import lines (8\|a → 7\|a\|a\|7; 12\|a → 9\|a\|a\|7–9 and 10\|a\|b\|5; 17\|a and 18\|a → 9\|a\|a\|8–9 and 12\|a\|a\|9 or \|7) named parts that are now the head's own parts, so none was appended; 7\|a has no **引入** section.

Where each former head ended:

| Former head | Now | Facet |
|---|---|---|
| 2\|a | 7\|a\|a\|1 | 第 1 章：文字版本与直觉（§1）; placed directly after the setting card 7\|a\|a, before the goods-market block (now 7\|a\|a\|2–8) |
| 8\|a | 7\|a\|a\|9 | 第 2 章：家庭的跨期选择（§3.2） |
| 9\|a | 7\|a\|a\|10 | 第 3 章：贸易部门企业（§3.3–3.4） |
| 10\|a | 7\|a\|a\|11 | 第 4 章：政府储备政策（§3.5） |
| 12\|a | 7\|a\|a\|12 | 第 5 章：储备政策与企业进入（§4.1） |
| 15\|a | 7\|a\|a\|13 | 第 6 章：企业区位转移外部性（§5.1.2） |
| 17\|a | 7\|a\|a\|14 | 第 7 章：外国企业退出（§5.2） |
| 18\|a | 7\|a\|a\|15 | 第 8 章：无限规划期（§5.3） |
| 4\|a | 6\|a\|a\|8 | 第 5 章：数据、度量与描述统计（§2.1） |
| 5\|a | 6\|a\|a\|9 | 第 6 章：部门生产率回归（§2.2 表 2） |
| 5\|b | 6\|a\|a\|10 | 第 7 章：企业动态回归与低估 IV（§2.2） |
| 2\|b | 6\|a\|a\|11 | 第 8 章：引言的经验动机（§1） |

Each chapter card keeps its former parts as its Side successors (its former setting or data card is `<chapter>|a`, its limits card `<chapter>|b`).

Gate:

- `model-survey.mjs bergin2026exchange`: 2 Model Cards, 7\|a (formal, 81 parts) and 6\|a (empirical, 217 parts); one `read` pair 6\|a → 7\|a (kin 8, compare 6), the theory-and-test Links that stay between the two models. The tree still holds 685 Cards.
- `check-model.mjs bergin2026exchange/7|a`: 81 members (16 main, 65 side), every former head a part; no closure or forward fault; `unanchored` only on the eight new chapter cards 7\|a\|a\|1 and 7\|a\|a\|9–15 (the restructure fixes these); self_sufficient false for that reason only.
- `check-model.mjs bergin2026exchange/6|a`: 217 members (12 main, 203 side, 2 off: the limits cards 6\|a\|b, 6\|a\|c); no closure or forward fault; `unanchored` only on the four new chapter cards 6\|a\|a\|8–11.
- `list_broken_links` (agent): empty.
- Term relink: `{"rounds":1,"ok":true,"cards_changed":3,"links_added":4,"cards_refused":0,"links_repaired":0}`.

Duplicates the settings pass must merge:

- Formal 7\|a: each chapter carries its former head's own setting card (7\|a\|a\|1\|a, 7\|a\|a\|9\|a … 7\|a\|a\|15\|a); these are the model's assumptions and go to the setting part. 7\|a\|a\|1\|a (the verbal setting: home reserve policy, manufacturing firms, passive foreign country) restates the setting of 7\|a\|a\|12\|a and 7\|a\|a\|14\|a in words. 7\|a\|a\|14\|a and 7\|a\|a\|15\|a are variant settings (start at the exit margin; T→∞) and need their own branches. 7\|a\|a\|11\|a\|3\|a (part, 脚注 17) and the Idea card 10\|c state the same footnote.
- Empirical 6\|a: five data cards for the same 45-country panel (6\|a\|a, 6\|a\|a\|8\|a, 6\|a\|a\|9\|a, 6\|a\|a\|10\|a, 6\|a\|a\|11\|a), each with its own 假说, 设计, 规格：基准 and 变量表 chain; the measurement cards of 4\|a (under 6\|a\|a\|8\|a: labor productivity, extensive and intensive margins, domestic intermediate share, capital controls and reserve growth, listed firms) repeat 6\|a\|a\|b–d; the chapters of 5\|a, 5\|b and 2\|b repeat 6\|a's Tables 2–5 chapters (productivity, firm dynamics, IV and subsamples, firm-dynamics IV) and must merge into them; the limits cards (6\|a\|b, 6\|a\|c, 6\|a\|a\|8\|b … 6\|a\|a\|11\|b) overlap.
