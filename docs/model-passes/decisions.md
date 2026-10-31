# Open items of the model passes: decisions (2026-10-31)

The owner was away and asked the main session to decide every fault or item that the worker sessions left for a human. Each item below names its source record, the decision and the reason.

| Item | Source | Decision | Reason |
|---|---|---|---|
| `hart1995firms/13|a` keeps its migrated import `整块 [[11|a|12|f|3]]：共用` (16 borrowed Cards; nothing in `13|a` rests on them). | `hart1995firms.md` | Keep. | The 引入 migration of 2026-10-31 kept every listed share unchanged (decision 5 of `docs/model-shared-blocks-proposal.md`). The import check passes, so the share does no harm. |
| `imports.mjs plan` reads the line `对应 [[5|a|a|1]]` in `hart1995firms/11|a|7|b|1` as a possible copy. | `hart1995firms.md` | Keep the line. | The chapter states the Aghion–Bolton E-control result and does not restate `5|a`. The line is a correspondence. |
| Chapters 2 and 3 of `hart1995firms/16|a` have no `结果：` card. | `hart1995firms.md` | Leave as they are. | `check-network.mjs` gives this as advice, not as a fault. The chapters hold derivation cards and case cards, and they have no single result to state. |
| `model-survey.mjs` labels a false share as `legacy share`; `tree-mover.mjs` has no test for its resume fix. | `hart1995firms.md` | Fix both. | Assigned to the Hart worker session. |
| The derivation line `说明：独立性是写出似然所需的假设，书中没有写出` in `hansen2021demand/5|a`. | `hansen2021demand.md` | Keep. | `check-settings.mjs` gives this as advice. The line explains one proof step at the point where the derivation uses it. |
| zt-check style faults R2, R3, R13, R20 and R21 on the Nobel and Hansen models. | `explanations.md` | Leave for a later style pass. | These faults were there before this work and are not explanation faults. Rewriting card text is outside the scope of the model passes. |
| Stale `E:\zet\target\debug\zt.exe` and `target\release\zt.exe` (built 2026-10-26). | `cited-tests.md` | Moved to the Recycle Bin. | They are older than the installed zt 0.49.0. A test or script must not pick them. The next `cargo build` writes new copies. |
| One wiki line on how the zt-digest integration tests choose `zt`. | `cited-tests.md` | Added to `docs/wiki/zt-digest.md` (Testing). | The block doc must record the change. |
| The note in `explanations.md` about the 8 failing cited tests. | `explanations.md` | Marked as resolved. | `cited-tests.md` fixed the cause. |
| The run report of `D:\zt-digest\ebrill2001modern-explain` shows `complete: false` (24 open adoption units). | `explanations.md` | Accept. | Mode `explain` does not own these units. They come from adopting a finished book. |
| Side-line proof chains were not checked for their 思路 position lines; becker2025division used a per-chapter wording. | `explanations.md` | zt-digest derives the chains itself with the restructure's filing rule (no zt change, no redeploy); one per-chain wording `第 k 张，共 n 张`; mode explain reran on five books (99 parts). | A zt change would need a redeploy of the live service while workers wrote to it. The filing rule is already fixed by `zt-model-restructure`. |
| `model-compiler.tests.ps1`: 10 close/explain checks fail. | `explanations.md` | Leave to issue #214. | They fail the same way with the 0.37.9 binary, before this work, and #214 tracks them. |
| Three backups from the owner's Grossman session (fold, settings, spine). | `D:\zt\backups` | Moved to the Recycle Bin. | The owner asked that backups be removed; that session is finished and its tree checks clean. |
| acemoglu2007equilibrium, acemoglu2010labor, becker2025division and grossman2002integration are still unlocked. | memory notes | Owner action only. | Only the owner sets Locks. The owner can relock these trees in the GUI. |
