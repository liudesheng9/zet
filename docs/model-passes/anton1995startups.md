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
