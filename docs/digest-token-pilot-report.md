# Digest Token Pilot: Report

Date: 2026-10-23. Book: *Integration versus Outsourcing in Industry Equilibrium* (`grossman2002integration`, 8 section groups). This report compares one digest run of the token-saving release (zt-econ-digest 6.2.0, zt-digest 0.12.0/0.12.1) with the last clean run of the same book before the release (trial 20, zt-econ-digest 5.10.0). For the design and the gates, see [digest-claude-token-proposal.md](digest-claude-token-proposal.md).

## Summary

| Question | Answer |
|---|---|
| Tokens saved? | Yes. The workflow agents spent **10.3M weighted tokens instead of 17.0M (−39%)** and needed **94 agents instead of 258 (−64%)**. |
| Faster? | Yes. **79 minutes instead of 128 minutes (−38%)** from start to the finished run report, in one invocation instead of a run plus a resume. |
| Quality unchanged? | Yes, at parity, and better on one measure. The models, their derivations and the figures are the same, with no model fault. The run report is complete; trial 20's was not (six figures named by no Card). The final audit has three more soft wording faults (5 against 2). |

## Runs compared

| | Trial 20 (before) | Pilot (after) |
|---|---|---|
| Workflow | zt-econ-digest 5.10.0 (base 4.10.0), zt-digest 0.5.3 | zt-econ-digest 6.2.0 (base 5.2.0), zt-digest 0.12.1 |
| Workflow runs | `wf_fb4825c8-bc9` and its resume `wf_016c84a9-9ca` | `wf_6d9c5c29-c4c`, no resume |
| Date | 2026-10-21 | 2026-10-23 |
| Source and range | Scholar Archive file 75, from 一、引言 to 参考文献, 8 groups | the same |
| DeepSeek | V4.1 Flash, official source, effort high | the same |
| Archive | the live archive | a copy of the box with its own zt service (at the user's direction), holding a locked copy of acemoglu2025nobel for the Bridge stage |

## 1. Tokens

Measured with `node scripts/book-digest/token-usage.mjs` on each run's agent transcripts. Weighted tokens = input + 1.25 × cache write + 0.1 × cache read + 5 × output, in units of each model's own input price. The figures cover the Claude Code workflow agents only, not the session that started and watched the run, and not DeepSeek.

| Agent kind | Trial 20: agents | Trial 20: weighted | Pilot: agents | Pilot: weighted | Change |
|---|---:|---:|---:|---:|---:|
| relays (one script command each) | 165 | 7.64M | 46 | 2.77M | −4.87M |
| Haiku managers (DeepSeek jobs, blocks, chains) | 73 | 6.91M | 45 | 6.05M | −0.86M |
| cleanup agents | 16 | 1.26M | 1 | 0.12M | −1.14M |
| Sonnet architect | 2 | 0.95M | 1 | 1.28M | +0.33M |
| Opus organizer | 2 | 0.23M | 1 | 0.13M | −0.10M |
| **total** | **258** | **17.0M** | **94** | **10.3M** | **−6.7M (−39%)** |

Where the saving comes from:

- **Batch finishes (P1, P2).** The relays that finish units (cards, events, findings, models, explanations, model dependencies) fell from 42 to 6. One `zt-digest finish-batch` command now finishes a block of units.
- **Cleanup without a model (zt-digest 0.11.7).** Cleanup went from 26 relays plus 16 agents to one agent around one bridge call.
- **Per ledger unit:** 0.56 relays in the pilot against 1.92 in trial 20 (and 1.23 on ebrill2001modern trial 1).
- **Not saved yet:** each remaining relay still costs about 60k weighted, because its first request carries about 61k tokens of context. The `zt-relay` agent type (P5, adopted after this pilot in base 5.3.0) starts a relay with about 9k. On this book that is about 2.3M weighted more, not yet measured on a run.

For scale, the same measure on the large book ebrill2001modern trial 1 (6.1.1, before the release): 876 agents and 68.7M weighted, of which relays were 613 agents and 32.4M.

## 2. Speed

| | Trial 20 | Pilot |
|---|---:|---:|
| wall time, start to run report | 128 min (99 min run + 29 min resume) | 79 min |
| invocations | 2 (the first run did not settle its cleanup) | 1 |
| digest stage finished at | 11 min | 11 min |
| models stage finished at | 45 min | 47 min |
| Link stage finished at | 70 min | 55 min |
| audit finished at | 93 min | 78 min |

The stages that DeepSeek writing bounds (digest, models) took the same time. The gain lies after the models, where the relays and finish agents were: the Link, Organize, Figures, Bridge and audit stages ended about 15 minutes earlier. The pilot also needed no resume: its cleanup settled in the first invocation.

## 3. Quality

| Measure | Trial 20 | Pilot | Reading |
|---|---:|---:|---|
| units failed / open | 0 / 0 | 0 / 0 | same |
| run report `complete` / `content_complete` | false / false | true / true | better: trial 20 had six figures named by no Card |
| `quality_complete` / `cleanup_complete` | false / true | true / true | better |
| Cards in the Literature tree | 143 | 152 | similar |
| Idea Cards (with the Literature Card and anchors) / anchors | 61 / 8 | 72 / 8 | similar |
| formal models, at the same addresses (2\|a, 3\|a, 4\|a, 5\|a, 7\|a, 8\|a) | 6 | 6 | same |
| concept models | 1 (1\|b) | 0 | trial 20 has one more |
| Model parts / derivation parts | 75 / 28 | 74 / 34 | similar |
| derivation parts with 思路 and 解读 | 28 of 28 | 34 of 34 | same |
| model build faults (closure, forward, reach-in, unanchored) / model issues | 0 / 0 | 0 / 0 | same |
| Event Cards / Finding Cards (the paper has none) | 0 / 0 | 0 / 0 | same |
| Cards that show a figure | 19 | 19 | same |
| Links in the tree | 579 | 634 | similar |
| broken Links | 0 | 0 | same |
| final audit faults | 2 (1 R6, 1 I7 on an old Topic Card) | 5 (all R6) | slightly worse |
| Bridge proposals to another book | 0 (no other book) | 19, open for optional review | new (another book was present) |

The differences in Card counts, the concept model and the R6 faults vary from run to run as the DeepSeek writers vary. R6 is the soft term-table wording rule, which the card cycle does not fix by default. The release changed no prompt, job list, model, effort, verifier or fault class: it only moved script commands out of separate agents. So none of the differences comes from the token changes.

## 4. Deviations and fixes during the pilot

- **Organize read the live archive.** Organize's Opus agent ran a helper read with no path argument from `E:\zet`, so the box pin did not apply. It read the live archive's locked trial-20 tree; nothing was written. It then left its notes empty because it believed the tree was locked. **Fixed** in zt-organize v7 (base 5.2.1, econ 6.2.1, math 5.2.1): the reads pass an empty read-only `-Allow` list from the staging folder.
- **The watcher reported superseded revisions as FAILED.** By design, a new publication revision supersedes the old one. `watch-run.mjs` now leaves them out.
- **Not exercised:** gate 3 (fault behaviour on a real run) and gate 4 (resume). The pilot met no unit fault and needed no resume, so both rest on the harness tests alone.

## 5. Conclusion

On the same book, the token-saving release cut the workflow agents' Claude Code tokens by 39% and the agent count by 64%, and it finished 38% sooner. The result is of the same quality: the same six formal models, no model fault, every derivation explained, and a run report that is complete where the old one was not. The next measured step is the first real run with the `zt-relay` agent type (P5), which should cut the remaining relay cost by about four fifths.
