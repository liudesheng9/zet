# rotunno2026industrial: post-digest model passes

Rotunno, Ruta and Verma, *Industrial Policy and Trade Tensions in Strategic Sectors* (IMF WP/26/155), Work 29955. The digest is settled with two gaps the owner accepted (skeleton/book@2 and the cleanup wait); see `D:\zt-digest\_queue\imen1-queue.md`. All passes ran on the live box (`D:\zt`), 2026-11-14.

## Stage 1: fold

Backup: `D:\zt\backups\zt-before-rotunno2026industrial-fold.sqlite3` (integrity_check ok).

The survey found 5 Model Cards and 5 candidate groups. No rests-on Link joined any two of them. The evidence settles every row, so the fold ran at once.

| Model Card | Level | Model | Surviving head | Reason |
|---|---|---|---|---|
| 4\|a | formal | quantitative trade model | 4\|a | the §3 model: N countries, J sectors, external economies of scale, subsidies and tariffs as price wedges |
| 2\|a | concept | quantitative trade model | 4\|a | the §1 verbal version of 4\|a; the Card itself says it is only the conceptual entry to the same framework; it becomes the model's first chapter |
| 9\|a\|3 | formal | quantitative trade model | 4\|a | Appendix A.1: the equilibrium conditions and the hat-algebra form of the same model; same agents, technology and policy instruments; its five **引入** lines all named 4\|a's own parts and were dropped |
| 3\|a | empirical | subsidy measurement | 3\|a | §2: the amount–count relation estimated on NIPO and the back-cast subsidy rates; own data and estimand; the theory only takes its rates as input |
| 10\|a | empirical | subsidy measurement | 3\|a | Appendix A.2: the test of 3\|a's extrapolation (GTA prediction against NIPO observation, Table A.2), same data and estimand; it becomes the model's fifth chapter |

The book holds **2 models**: `4|a` (formal) and `3|a` (empirical).

### Fold (executed 2026-11-14, live box)

- 2\|a → `4|a|a|1`, facet "第 1 章：文字版本：补贴、规模经济与三类效应（§1）", its setting chain filed under it.
- 9\|a\|3 → `4|a|a|13`, facet "附录章：均衡条件与变化率形式（附录 A.1）", its setting and solution parts filed under it. The idea notes 9\|a\|4–6 stay in section 9.
- 10\|a → `3|a|a|7`, facet "第 5 章：预测检验（附录 A.2）", its data and limitation parts filed under it. Its 17 idea notes (the A.2 tables and figures) moved to section 10 as `10|e` and its chain.
- Incident: the first retype of 10\|a was refused because the facet was longer than 32 characters, after the parking had applied. The retype, the move and the body restore were completed by a recovery script; the masked body Links were restored to the new addresses.

### Gate

- `model-survey.mjs`: exactly 2 Model Cards, 3\|a (63 parts) and 4\|a (40 parts).
- `check-model.mjs`: every former head is a part. Remaining faults are the unanchored new chapters (`4|a|a|1`, `4|a|a|13`, `3|a|a|7`), which the restructure fixes.
- `list_broken_links`: empty. `term-relink.ps1`: 1 Link added, 1 Card changed.

### Open

- 3\|a and its new chapter 5 both carry the Table A.2 specifications and estimates (3\|a\|a\|4 and 3\|a\|a\|7). The settings and restructure passes reconcile the duplication.
