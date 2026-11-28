# Model passes: muendler2012employee

Owner: Xingtong Chen

Muendler, Rauch and Tocoian (2012), *Employee Spinoffs and Other Entrants: Stylized Facts from Brazil* (Scholar Archive Work 30158). Digest: `D:\zt-digest\muendler2012employee\run-report.json` (`completeness.complete: true`).

## Stage 1: fold

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-postdigest-2026-11-27.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok).

The digest left 1 Model Card. Decision by nature:

| Model Card | Model (surviving head) | Level | Reason |
|---|---|---|---|
| 7\|a | own (7\|a) | empirical | Single empirical model (64 parts): OLS comparison of initial size and 5-year exit rates across four entrant types on Brazilian RAIS 1995–2001 employer-employee microdata. |

No fold required (0 writes).

### Gate

- `model-survey.mjs muendler2012employee`: 1 Model Card (`7|a`, empirical, 64 parts).
- `zt-check --model 'muendler2012employee/7|a' --stage fold`: pass (`members 64 {main:7, off:2, side:55}`, `loose 0`, `self_sufficient true`, `evidence_bound true`).
- `list_broken_links`: verified clean (0 broken links in `muendler2012employee/`).
- Term relink: not needed (no Cards edited in Stage 1).
