# Model passes: velic2025inflation

Velic (2025), *On Inflation Dynamics: International Comovements, Nonlinearities, and Persistence* (TEP Working Paper 1825). Digest run wf_840a17d6-f7c (2026-11-12/13, OpenCode Go), run report `complete: true`, 0 gaps. Post-digest sequence (`zt-post-digest`) on the live box at the owner's request, 2026-11-13. Subagents run at effort medium (owner).

## Stage 1: fold

Backup: `D:\zt\backups\zt-before-velic2025inflation-fold.sqlite3` (`VACUUM INTO`, integrity ok).

Survey before: 8 Model Cards (1 formal, 4 empirical, 3 concept); every candidate group held one card (no rests-on Link between Model Cards).

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| 3\|a | §2.1 | ESTAR | 3\|a | the formal ESTAR model of the idiosyncratic inflation gap |
| 4\|b | §2.4 | ESTAR | 3\|a | concept; continues 3\|a: the same primitives (gap, φ, φ*, F); its persistence 1+φ+φ*F is 3\|a's result, extended by the half-life and the two channels. Becomes chapter `3|a|b|7` 第 1 章：持续性的两渠道分解 |
| 8\|a | §4.2 | ESTAR estimation | 8\|a | empirical; estimates 3\|a country by country: a theory and its estimation are never folded |
| 7\|a | §3, §4.1 | data and descriptive facts | 7\|a | empirical; the HICP panel and covariates built for the whole paper (they feed both 8\|a and the drivers model), with the Table 1–2 facts; cannot belong to one estimation |
| 4\|a | §2.4 | drivers panel regression | 10\|a | empirical; the same estimand, data and equation (4) as 10\|a, and repeats its estimates (Tables 4, 5, 8). Becomes chapter `10|a|a|9` 第 6 章：式 (4) 的设定与假说 |
| 10\|a | §4.4 | drivers panel regression | 10\|a | empirical; the full estimation (Tables 4–11), 66 parts; kept as head |
| 10\|b | §4.4 | drivers panel regression | 10\|a | concept; reads Tables 7–8 to split the drivers' effects into the gap and adjustment channels: the mechanism chapter of that estimation. Becomes chapter `10|a|a|10` 第 7 章：两渠道的识别. Its 引入 line (整块 the two-channel conclusion, now `3|a|b|7|a|2`) moved to 10\|a's new **引入** section |
| 9\|a | §4.3 | triad mechanism | 9\|a | concept; its own question (why comovement, gap and persistence move together); 4 own parts, so it takes the fold only |

Five models. Method: `fold.mjs` (the goldsmithpinkham2020bartik session script: strip **结构**/**引入**, mask Links to Model parts, park the parts, retype, move the head `alone` after the last main-line card, return the parts, restore the body with the rename map), one Card per run. 4\|b's Idea-note chain (4\|b\|1–7) moved to section 4 (now `4|c`…).

Gate: survey lists exactly 3\|a (formal, 12 parts), 7\|a (empirical, 27), 8\|a (empirical, 43), 9\|a (concept, 5 incl. one import), 10\|a (empirical, 120); `list_broken_links` empty; `check-model.mjs`: every former head is a part; the only faults are the `unanchored` new chapter cards (3\|a\|b\|7, 10\|a\|a\|9, 10\|a\|a\|10), left to the restructure. Term relink: 0 Cards changed.

Note: 10\|a now holds two parallel builds of the same estimation (former 4\|a's specification ladder and estimates duplicate 10\|a's own). The settings and restructure passes merge them.
