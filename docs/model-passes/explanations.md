# Derivation explanations: four finished books (2026-10-31)

Live box (`D:\zt`, service 127.0.0.1:4717). The task: give every Derivation part (a Model part whose facet starts with `推导`) its 思路, 说明 and 解读 with the digest workflow's mode `explain` (design: `docs/derivation-explanations-proposal.md`). All four trees were unlocked.

## Audit method

For each book, a fresh copy of its latest staging folder (`D:\zt-digest\<key>-cited`) became `D:\zt-digest\<key>-explain`: ledger, `adoption\`, the old `explain\` work folder and the `explain-*.json` records removed, `run_contract` stripped, every path repointed. `zt-digest ledger adopt -Out <copy> -Variant econ` (dry run) then read every Model Card of the live tree and its Derivation parts. A second, independent count came from the compiler's own planning step, `zt-digest model -Step explain` on a scratch work folder: it lists every part that lacks 思路 or 解读 and every part whose 思路 position line no longer matches its current chain (R301). It writes no Card.

## Counts per book

| Book | Formal models with Derivation parts | Derivation parts (own) | Explained before | Stale position lines before | Run | Explained after | Stale after |
|---|---|---|---|---|---|---|---|
| acemoglu2007equilibrium | `2\|a` | 31 | 31 | 0 | skipped: already fully explained | 31 | 0 |
| acemoglu2010labor | `2\|a` | 77 | 77 | 0 | skipped: already fully explained | 77 | 0 |
| becker2025division | `4\|a` | 72 | 72 | 0 | skipped: already fully explained | 72 | 0 |
| ebrill2001modern | 13 (`3\|a`, `7\|a`, `11\|a`, `14\|a`, `24\|a`, `32\|b`, `33\|a`, `40\|a`, `53\|a`, `63\|a`, `71\|a`, `72\|a`, `73\|a`) | 61 | 61 | 7 (`71\|a\|a\|5`; `72\|a\|a\|2`–`4`; `73\|a\|a\|4`–`6`) | mode `explain`, 3 units, 7 parts | 61 | 0 |

(The tables write `\|` for `|` in addresses.) "Stale position lines" counts what the explain step can see, that is chains on the main line; see the finding below for Side-line chains. The 27 concept models of ebrill2001modern have no Derivation part. The run's audit stage counts 69 Derivation parts with 思路 and 解读 in ebrill2001modern, because it also counts the parts a model imports from another model.

The seven ebrill parts had all five sections, but their 思路 position line (`第 k 张，共 n 张：从 … 推到 …`) quoted older forms of the chain's results. For example, `72|a|a|3`'s 结果 reads `\left(t_i-\sum_k w_{ik}t_k\right)` and its position line still read `\,(t_i-\sum_k w_{ik}t_k)`. The model passes edited the results after the explanations were written.

## The ebrill2001modern run

- Backup: `D:\zt\backups\zt-before-explain-ebrill2001modern.sqlite3` (`VACUUM INTO`, integrity ok).
- Run folder: `D:\zt-digest\ebrill2001modern-explain` (adopted; the first `-Apply` was refused with "changing view across archive revisions" while another worker wrote to the box, left no ledger, and the retry applied).
- `Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-econ-digest.js", args: { mode: "explain", out: "D:\\zt-digest\\ebrill2001modern-explain", citationKey: "ebrill2001modern" } })`, zt-econ-digest 6.10.2, run `wf_357205a3-69d`, 9.3 minutes, 12 agents, no error.
- Explain: 3 units, 3 DeepSeek jobs (V4.1 Flash, official source), 7 parts applied, 0 refused, 0 blocked, 3 units verified. Term link: 4 Cards changed (6 Term links added); the second plan changes 0 Cards. Cleanup: 4 conversations (2 deleted, 2 already gone), 2 workspaces removed.
- The run report shows `complete: false` with 24 open units. They are adoption units that mode `explain` does not own (4 digest groups adoption could not verify, 11 recovered note suggestions, and the registered whole-book publications). This is expected for an adopted folder.

## Audit after

- Explain plan again (`model -Step explain` on all 13 formal models): 0 parts left.
- For the seven parts, 起点, the step lines (without 说明) and 结果 are identical to the backup (Term links read as labels). Each part now has 思路 and 解读 and 1–2 说明 lines.
- `zt-check --model <m> --require-explain --skip-shared` on `71|a`, `72|a`, `73|a`: 0 faults. On the three skipped books, no R15–R17 fault (their other style faults, R2/R3/R20 and so on, predate this job and are not explanation faults).
- `list_broken_links` (agent Workspace): none.
- Orphans: 189 before and after (`audit.json`).
- Tree diff against the backup (1,664 Cards before and after, none added or removed): 10 Cards changed. Seven are the explained parts. Three differ only in the archive-generated `<!-- zt:reverse-links -->` section: `73|a|a|8` (the new 解读 of `73|a|a|6` names the next part in prose and no longer links it), and Term Cards `1|a|c` and `1|a|p` (they gained the Term uses that the Term-link pass added to `73|a|a|4` and `71|a|a|5`). No authored text outside the seven parts changed.

## Workflow faults met and fixed

1. **Adoption did not see stale position lines** (zt-digest 0.37.9). `ledger adopt` registered an `explain/` unit only for parts with an empty 思路 or 解读, but the compiler's explain step also counts a stale chain position line as unfinished (R301). With 0 needs, mode `explain` would have claimed nothing on ebrill2001modern. Adoption now uses the compiler's own check (`model::compiler::stale_positions`) and records such a part as `missing: ["位置行"]`. Test: `a_stale_chain_position_line_is_an_explanation_need`.
2. **Mode `explain` ran the Link stages and Organize** (base 5.10.2, econ 6.10.2, math 5.10.2, zt-digest 0.37.10). The mode also ran `link-prepare`, `link`, `link-check` (a card cycle over the book) and `organize` (an Opus decision, a notes card cycle and the skeleton, which rewrites the Literature index and the structure Card). The explanations only add sections inside finished parts (design D9), so those stages had nothing to do except rewrite Cards outside the Derivation parts of a hand-curated book. Mode `explain` now runs `explain`, `term-link`, `audit`, `cleanup`, `report` (math also `math-theory`, `math-check`). Modes `models`, `events` and `findings` are unchanged. Harness checks `mode explain (econ)` and `mode explain (math)`; both workflows reinstalled and verified in project and global scope.

Wiki: `docs/wiki/zt-digest.md` and `docs/wiki/digest-workflows.md` (change history, mode table). Nothing is committed.

## Second batch: acemoglu2025nobel and hansen2021demand (same day, after their model passes)

Same method: fresh copies `D:\zt-digest\<key>-explain` of `<key>-cited`, adoption dry run, the compiler's explain plan on a scratch work folder, and `zt-check --model <m> --require-explain --skip-shared`. Both trees were unlocked.

| Book | Formal models with Derivation parts | Derivation parts (own) | Explained | Stale position lines | Run | R15–R17 faults |
|---|---|---|---|---|---|---|
| acemoglu2025nobel | `4\|a` (15), `5\|a` (3) | 18 | 18 | 0 | skipped: already fully explained | 0 |
| hansen2021demand | `10\|a` (7), `5\|a` (1) | 8 | 8 | 0 | skipped: already fully explained | 0 |

The restructured `acemoglu2025nobel/5|a` was the expected gap. Its three Derivation parts (`5|a|a|1|a|a`, `|1`, `|2`) already have 思路 and 解读, and each 思路 opens with a position line (`第 k 张，共 3 张…`) whose quoted formulas match the parts' 结果 exactly. `get_model` lists no chain for this side-line run (it lists only the four chains of the imported `4|a` parts), so the explain plan does not check these position lines. They are correct as written. `hansen2021demand/5|a` has one Derivation part, which is explained. No Card changed, so no backup was taken and no workflow ran. The other zt-check faults on these models (R2, R3, R13, R20, R21) are style faults outside the explanations.

## Third batch: hart1995firms (same day, after its model passes)

Same method (fresh copy `D:\zt-digest\hart1995firms-explain` of `hart1995firms-cited`; the tree was unlocked). The adoption dry run and the compiler's explain plan agree:

| Book | Formal models with Derivation parts | Derivation parts (own) | Lack 思路 or 解读 | Stale position lines the explain step sees | Run |
|---|---|---|---|---|---|
| hart1995firms | 12 (`3\|a` 2, `3\|b` 1, `5\|a` 9, `9\|a` 6, `10\|a` 5, `11\|a\|7` 4, `11\|a\|12` 14, `13\|a` 4, `14\|a` 8, `15\|c` 7, `16\|a` 16, `20\|d` 5) | 81 | 0 | 0 | none needed by the workflow |

No Card changed, so no backup was taken and no workflow ran.

## Side-line proof chains: finding, fix and the fourth batch (same day)

### Finding

The explain step (and adoption since 0.37.9) checked a part's 思路 position line against `get_model`'s `build.chains`. zt builds those chains from the main and shared lines only (`src/model.rs`: runs of Derivation parts in the build order; Side parts are skipped). `zt-model-restructure` files every proof chain as a Side branch of its result, so on a restructured model `build.chains` is empty and no position line was checked. The "0 stale" counts in the earlier sections were therefore true only for chains on the main line. A read-only scan showed real stale lines: chains to which a model pass added a part (the new part had no position line, the older parts counted the old length), chains that the restructure split across results, results edited after the explanation, and becker2025division's per-chapter wording `推导链第 k 步，共 n 步` (chapter 5 numbered 13 steps over three results).

### Decision and fix (zt-digest 0.37.11; the main session decided, no zt change)

- One chain rule, `model::compiler::proof_chains`: the runs of zt's `build.chains`, plus every Side-line proof chain as zt-model-restructure files it: a Derivation part X, then X|1, X|2, … while they are Side-line Derivation parts. A chain may start at X|1 under a card that is no Derivation part (hart `11|a|12|f|1|a|a|1`–`4` under a contract setting). A chain ends at the first continuation that is no Derivation part (hart `10|a|a|2|b|a|3`, a summary card), and the Derivation part after it stands alone. Letter siblings (`1|a`, `1|b`) are separate Side branches in ZET's grammar (the Direct successor of `x|a` is `x|a|1`), so they are separate chains.
- One wording: the per-chain line `第 k 张，共 n 张：从 … 推到 …`. A part is stale when its chain wants another line, when a chain member has none, or when a part that stands alone keeps a position line in either wording.
- The explain step's plan and job builder, adoption, and `explain-finish` use this one rule (`stale_positions`). Test first: `side_line_proof_chains_are_checked_for_their_position_lines` (a chain a pass extended, a chain under a non-derivation card, a chain that ends at a non-derivation card, becker's wording, a leftover line on a lone part). All 612 unit tests and all integration tests pass; `explain-finish`, `ledger-adopt` and `finish-batch-explain` PS tests pass. `model-compiler.tests.ps1` has 10 failures (close follow-ups on a plan "take" fault, and the explain-check R16 case); the same 10 fail with the 0.37.9 binary, before this change, so they are older than it.
- zt-digest 0.37.11 installed (blueprints unchanged, so no workflow reinstall). Wiki: `docs/wiki/zt-digest.md`; design note: the chain-rule row in `docs/derivation-explanations-proposal.md` ("As built").

### Fourth batch: the runs

Each book: backup (`VACUUM INTO`, integrity ok), fresh adopted copy, mode `explain` (zt-econ-digest 6.10.2), then the audit: the explain plan again (0 parts), 起点/steps/结果 of every changed part equal to the backup (Term links read as labels), the tree diff, `zt-check --require-explain`.

| Book | Run folder | Parts (stale or missing line) | Refused | Re-plan after | Changed Cards outside the parts | R15–R17 after |
|---|---|---|---|---|---|---|
| hart1995firms | `hart1995firms-explain` | 16 (`11|a|12` 2, `16|a` 14), 2 units | 0 | 0 | 54: 41 Term links only, 13 generated sections only | 0 |
| acemoglu2007equilibrium | `acemoglu2007equilibrium-explain` | 3 (`2|a|a|6|a|a`, `|4`, `|5`) | 0 | 0 | 3, generated sections only | 0 |
| acemoglu2010labor | `acemoglu2010labor-explain` | 4 (`2|a|a|10|b|a`; `15|b|a`, `|3`, `|4`) | 0 | 0 | 2, generated sections only | 0 |
| becker2025division | `becker2025division-explain` | 72 (all: per-chapter wording), 1 unit | 0 | 0 | 28, generated sections only | 0 |
| ebrill2001modern | `ebrill2001modern-explain2` | 4 (`14|a|a|1|a`, `1|b`; `24|a|a|1|a|a`, `|2`) | 0 | 0 | 1, generated section only | 0 |
| acemoglu2025nobel | (plan only) | 0 | – | 0 | – | 0 |
| hansen2021demand | (plan only) | 0 | – | 0 | – | 0 |
| grossman2002integration | `grossman2002integration-explain` (dry run only) | 0 of 28 | – | 0 | – | 0 |

Notes:
- "Generated sections" are the archive's `<!-- zt:reverse-links -->` sections (they follow Link changes in the rewritten parts). "Term links only" means the text is equal to the backup with Term links read as labels: on hart the Term-link pass moved 81 term links to their first occurrence on chapter and result Cards that the restructure had rewritten without a Term-link pass (21 removed). No other text outside the parts changed in any book.
- becker2025division: after the run no part carries `推导链第`; 67 parts carry the per-chain `第 k 张` line and the 5 that stand alone carry none. Its other zt-check faults fell from 169 to 119.
- The rewritten parts get a new 思路, new 说明 lines and a new 解读, by design of the explain step (D8); 起点, the step lines and 结果 are unchanged.
- No run failed or was refused; the DeepSeek conversations were deleted by each run's cleanup. `list_broken_links` (agent Workspace): none. The five backups are in the Recycle Bin; the run folders are kept.

## Notes for later

- `cargo test` in `tools/zt-digest`: `tests/cited.rs` (2 of 6) and `tests/cited_cards.rs` (6 of 6) failed with `zt: card `` does not exist` from their zt seam. Resolved the same day: the tests ran a stale zt 0.42.0 from `target\debug`; a shared helper now picks the newest zt, and the full suite passes (`cited-tests.md`).
