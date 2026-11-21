# Backslash repair, 2026-11-21

Owner request 2026-11-21 ("solve them"): repair the formulas in agent Cards whose TeX commands lost their backslashes. Box backup before the writes: `D:/zt/backups/zt-before-backslash-repair-2026-11-21.sqlite3` (integrity ok).

## Scan method

The scan reads `D:/zt/zt.sqlite3` read-only and covers every Card of every agent Literature tree, for all literature types. It takes each `$…$` and `$$…$$` span of the Card text before the reverse-links marker. It ignores ```` ```chart ```` fences and the arguments of `\text{}`, `\mathrm{}`, `\operatorname{}` and similar commands. It flags a TeX command word (Greek letters, `frac`/`dfrac`/`tfrac`, `sum`, `quad`, `equiv`, `dots`/`ldots`/`cdots`, `mathrm`/`mathcal`/`mathbb`/`boldsymbol`/`operatorname`, `ln`/`log`/`exp`/`min`/`max`/`lim`/`sup`/`inf`/`int`, `leq`/`geq`/`neq`, `pm`/`mp`, `to`/`rightarrow`/`Rightarrow`/`iff`, `overline`/`underline`/`widehat`/`widetilde`, `begin`/`end`, and others) that has no backslash before it and no letter on either side. It also flags the literal placeholder `@BS@` anywhere in a Card. Each hit was judged by eye. A dated history of every flagged Card was taken across all 88 backups in `D:/zt/backups` (backslash count by `row_id`, and whether the text stripped of backslashes equals the live text).

The first run gave 1,304 hits. 17 Cards were damaged. Every other hit is a false hit of one of these kinds:

- `in`, `ge`, `int`, `mp` and similar words inside a subscript or superscript label: `_{in}`, `\varepsilon^{ge}`, `\psi_\pi^{int}`, `t_mp`, `\theta_{Xi}`.
- A product of single-letter symbols: `e^{nu}` (n·u), `mp_XX` (m·p_X·X), `R^ge^{…}` (R^g e^{…}).
- A variable name: `exp_{i,t}` (expenditure) in furceri2026divide/9|b.
- A word inside `\text {…}` written with a space before the brace: `\text {subject to}`, `\text {implies}`.
- Transliterated names that sit between two `$` signs in history books: `maẓālim`, `amīr al-mu'minīn`, `geōrgos`.
- Plain Unicode math that never held TeX commands: `ln(SR)`, `Δln y=0.70×ln 1.10`, `max_{t_1=t_2}R` in ebrill2001modern/5|a|a|…, and `u < ln θ0*` in becker2025division/6|g|a|3. These Cards have no backslash in any backup.

## Repaired Cards

All writes went through the zt tool `edit_card` with `base` (one `zt-digest call` batch, 17 ok, 0 errors). Each repair replaced a whole math span by the same span with its backslashes restored. The read-back check strips every backslash (and `@BS@`) from the old and the new text: all 17 are equal, and the `[[…]]` Link lists are identical. No backup holds an intact version, so the restored forms come from TeX syntax and, for the `diag` braces, from the source Markdown (`\Delta = diag\{\delta_1, \dots, \delta_n\}`, Scholar Archive file 95).

| Card | Spans | One formula before → after |
| --- | --- | --- |
| ebrill2001modern/21\|b\|a\|a | 5 | `mathrm{diag}{delta_1,dots,delta_N}` → `\mathrm{diag}\{\delta_1,\dots,\delta_N\}` |
| ebrill2001modern/21\|b\|a\|a\|1 | 1 | `R=y'(I_N-Delta A)Pt quad (II.2)` → `R=y'(I_N-\Delta A)Pt \quad (II.2)` |
| ebrill2001modern/21\|b\|a\|a\|5 | 5 | `t_I^*equiv[(I_N-A)P]^{-1}(I_N-Delta A)Pt quad (II.5)` → `t_I^*\equiv[(I_N-A)P]^{-1}(I_N-\Delta A)Pt \quad (II.5)` |
| ebrill2001modern/21\|b\|a\|a\|6 | 4 | `q=(I_N-A)^{-1}v+(I_N-A)^{-1}(I_N-Delta A)Pt quad (II.4)` → `…(I_N-\Delta A)Pt \quad (II.4)` |
| ebrill2001modern/21\|b\|a\|a\|7 | 5 | `t_I^*=t quad (Delta=I_N)` → `t_I^*=t \quad (\Delta=I_N)` |
| ebrill2001modern/21\|b\|a\|a\|8 | 4 | `t_I^*-t=P^{-1}(I_N-A)^{-1}(I_N-Delta)APt` → `…(I_N-\Delta)APt` |
| ebrill2001modern/21\|b\|a\|a\|10 | 1 | `R=x'Pt_I^* quad (II.6)` → `R=x'Pt_I^* \quad (II.6)` |
| ebrill2001modern/21\|c\|a\|a | 3 | `w_{ik}=a_{ik}p_k/sum_j a_{ij}p_j` → `w_{ik}=a_{ik}p_k/\sum_j a_{ij}p_j` |
| acemoglu2025nobel/3\|i\|a\|a\|1\|a | 1 | `delta` → `\delta` |
| atkinson2015lectures/17\|b\|a\|b\|4 | 1 (title and body) | `@BS@theta = 0` → `\theta = 0` |
| atkinson2015lectures/20\|e\|a\|a\|a\|a | 1 | `sum_h t_h=1` → `\sum_h t_h=1` |
| atkinson2015lectures/19\|e\|b\|a\|a | 1 | `Psi_{h}` → `\Psi_{h}` |
| atkinson2015lectures/20\|d\|c\|a\|a\|a | 2 | `eta^{1},eta^{2},eta^{3}` → `\eta^{1},\eta^{2},\eta^{3}` |
| dell2010persistent/4\|c\|b\|a\|4\|b | 1 | `gamma` → `\gamma` |
| liu2026managing/6\|a\|a\|a\|a | 1 | `hat f` → `\hat f` |
| assenza2026louder/5\|e\|a\|a | 1 | `Psi\geq0` → `\Psi\geq0` |
| furceri2026divide/3\|e\|a\|2\|a\|b | 1 | `mathrm{shock}_{t}` → `\mathrm{shock}_{t}` |

Most damaged spans sit in notation tables (符号表, 符号表与读法, 变量表) and in equation lines with a `(II.x)` tag.

## Root cause

Dating: 15 of the 17 Cards were already damaged in the oldest backup (2026-11-13 02:15, `zt-before-velic2025inflation-imports2`). assenza2026louder/5|e|a|a was created damaged between the backups `-settings` (11-13 23:08) and `-settings2` (11-13 23:28), that is, in the zt-model-settings pass of assenza. furceri2026divide/3|e|a|2|a|b was created damaged between `-settings` (11-21 00:04) and `-restructure` (11-21 00:23), in its zt-model-settings pass. No file in `E:\zet\scripts`, `E:\zet\tools` or `~/.claude/skills` contains `@BS@`.

The damage pattern identifies the mechanism. Every lost command starts with a letter that is not a JavaScript escape letter (`delta`, `Delta`, `quad`, `equiv`, `sum`, `dots`, `mathrm`, `Psi`, `eta`, `gamma`, `hat`, and `\{` → `{`). No lost command starts with `t`, `b`, `n`, `f`, `r`, `v`, `u` or `x`. A JavaScript string literal drops the backslash of an unknown escape (`'\delta'` is `delta`, `'\{'` is `{`), and it would turn `\theta`, `\beta`, `\nu`, `\frac` into control characters instead. The Bash tool of this harness also halves doubled backslashes inside a quoted heredoc: a test file written with `cat > t.js <<'EOF'` holding `'\\delta \\quad \\sum_j'` was stored as `'\delta \quad \sum_j'`, and node printed `delta quad sum_j`. The same thing happened to the scan script of this repair until it was rewritten with the Write tool.

Finding: the damage was written by ad-hoc node scripts that an agent wrote through the Bash tool (heredoc or `node -e`) during model passes, with Card text in JavaScript string literals. The doubled backslashes were halved by the tool, and the JavaScript parser then dropped the remaining ones. The `@BS@` placeholder is a guard that such an ad-hoc script used to protect backslashes and failed to restore for one span. No checked-in script is at fault, so no file and line can be named. The pass scripts in the skills (`zt.mjs` `call`) write JSON files with `JSON.stringify` and keep backslashes intact; this repair used them.

Recommended prevention (not done here; code unchanged): agents write any script that carries TeX with the Write tool, never with a Bash heredoc or `node -e`; and the widened scan (`C:\Users\chenx\AppData\Local\Temp\claude\c2655a31b3642726\87d2963d-af4a-4119-968b-538cdc4456ef\scratchpad\bsr\scan.cjs`, a temporary copy) can be a gate after each model pass.

## Gates

- Widened scan rerun: 1,265 hits, all of the false kinds above; 0 real hits, 0 `@BS@`.
- `list_broken_links` (agent): empty.
- `check-settings.mjs` and `check-model.mjs` on each affected model: atkinson2015lectures/20|e|a|a and 20|d|c|a, dell2010persistent/4|c|b, liu2026managing/6|a|a, assenza2026louder/5|e and furceri2026divide/3|e are clean (assenza has three advice lines only). ebrill2001modern/21|b|a and 21|c|a report format faults of the setting card (sections, a symbol table in the setting card, a 依赖 line outside **起点**), acemoglu2025nobel/3|i|a reports a first-use row for `\delta^{*}` and two setting-card formats, and atkinson2015lectures/19|e|b reports first-use rows for `U` and `M`. These faults concern sections and symbols that the repair did not touch (`\delta^{*}`, `U`, `M` were intact before); they are earlier open items of those models. All check-model runs report `self_sufficient true`.
