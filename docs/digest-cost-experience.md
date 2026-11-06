# Lowering a digest module's token cost

What lowered the cost of the figure module (2026-11-05/06), and how to do the same for the next module. Written for the agent that continues this work. The rules in `docs/agents/workflow-principles.md` still govern every change: one start, one queue of managers (8 jobs each, 8 managers), one finish, no switch, the old path deleted.

## Do this for the next module

1. **Measure a real run first.** Keep its sessions while it runs, because a manager deletes each conversation as it ends:

   ```bash
   node scripts/book-digest/session-report.mjs keep "--D-zt-digest-<key>-figures--" <scratch folder> 40
   ```

   Run that in the background, stop it when the run ends, then:

   ```bash
   node scripts/book-digest/session-report.mjs report <scratch folder>
   ```

   The report gives tool calls per job, self-check faults by kind, failed tools and reasoning size. Read `deepseek_usage` before and after each variant: the ledger counts by day and provider, so run variants one at a time, with nothing else on the source.

   For the Claude side of a run, `node scripts/book-digest/token-usage.mjs <session>/subagents/workflows <run id>` gives tokens per agent kind.

2. **Read the fault kinds before you change anything.** A fault that repeats across jobs is a bug in the input, not in the agent. Fix the input.

3. **Test variants on a staging copy of a small book.** Copy `manifest.json` and `book.zh.md`, plan there, and run the same groups through each variant with `deepseek_run`. Move each variant's answers out of the folder before the next one, or the next agent reads them. Delete the conversations and remove the workspace afterwards.

4. **Score coverage as well as cost.** The cheapest variant here lost 15% of the agreed pairs. Without a right answer, use the pairs most variants chose as a stand-in, and say that it is one.

5. **Check the plan on the largest book before a real run.** Print the job sizes of a real plan. The size you tested is the size you may claim.

6. **Pilot one real book, then go book by book, smallest first,** one workflow at a time. Keep builds and test suites off the machine during a finish step.

Done means: the fix ran on a real book, the session report shows the expected tool calls, the ledger shows the cost, and coverage is scored against the old shape.

## What the figure module taught

### Browsing was the cost, not Card count

On the Nobel lecture (117 Cards, 12 figures), one job per group made 158 model calls. Agents listed folders, opened sibling answers and images, and re-checked work, though everything they needed was in the message. Each call resends over 100K tokens.

| Job shape | Jobs | Model calls | Ledger cost | Slowest job | Agreed pairs (of 95) |
|---|---:|---:|---:|---:|---:|
| One group (old) | 5 | 158 | 1.47 | 589 s | 90 |
| Two groups | 3 | 52 | 0.50 | 320 s | 94 |
| Three groups | 2 | 52 | 0.66 | 371 s | 95 |
| Two groups, message only | 3 | 11 | 0.19 | 98 s | 85 |
| Three groups, message only | 2 | 5 | 0.23 | 164 s | 80 |
| Three groups, message only, two passes | 2 | 5 | 0.25 | 185 s | 91 |

Each row is one run, and agent behaviour varies between runs. Agreed pairs are those at least 4 of the 6 variants chose.

**A rule that works:** when a job holds all its material, say so, and set the shape of the work: all material is in this message, think two passes, write once, check once. Stopping the browsing alone also stopped the review and lost pairs. Moving the review into thought (Card by Card, then figure by figure) kept the coverage at about the same price. The wording is in `scripts/book-digest/dsh-figure-instructions.zh.md`, section 工作方式.

### Size a job in the unit of the work

The first release took three groups a job. Tested at 41 and 76 Cards, it reached 410 Cards on `atkinson2015lectures`, because a top-level section of a reorganized tree holds up to 150 Cards. On the same 110 Cards a small job found 32 and 24 pairs and the big jobs 15 and 21, and the big jobs cost more. The job now counts Cards: at most 80 (`CARDS_PER_JOB` in `tools/zt-digest/src/figures.rs`).

### Let an agent copy an id, never reproduce printed text

Sessions of 30 readers showed 21 jobs hitting `10-4 is not a label of the figure list`, 348 times, in up to 10 tool calls a job. The parser read `Figure 10–4` as `10` and `I.1` as `I`. The readers were right and the check was wrong. It also meant a whole chapter's figures shared one label, so Cards showed sibling figures: 1,050 images on 503 Cards had to be removed and read again.

Every figure now has a unique id (`F12`) made for the job, and the printed number is shown in brackets for recognition. After the fix: 29 jobs, 0 faults, 2.0 tool calls a job.

### A clean run is not automatically a cheap run

The repair loops cost little, because they mostly resend cached context. Cost per job stayed flat after the fix: 0.36 before, 0.37 after, about 33K output tokens a job. Reasoning output is what a job costs now. Report this kind of result as it is: fewer faults is a correctness gain until the ledger says otherwise.

## Real run costs

| Run | Jobs | Cards read | Ledger cost | Per job | Per Card |
|---|---:|---:|---:|---:|---:|
| Nobel lecture | 2 | 117 | 0.25 | 0.13 | 0.0021 |
| Donner | 8 | 593 | 2.44 | 0.31 | 0.0041 |
| Re-read of Hart, Ebrill, Atkinson, Baghdad | 73 | 5,706 | 25.94 | 0.36 | 0.0045 |
| Atkinson repair | 29 | 2,266 | 10.72 | 0.37 | 0.0047 |

Larger books cost about twice as much per Card as the Nobel lecture. The likely reason, not yet tested, is that every job carries the book's whole figure table.

## Where to look next

In order of expected saving; none of these is tested yet.

1. **Reasoning effort.** Every job runs at high by the owner's rule, and reasoning is the main cost of a job. Test medium on a staging copy, score coverage, and ask the owner before changing the rule.
2. **The same audit on other modules.** Term Cards, Cited Cards, Links and the card cycle have never had a session report. The message-only rule applies to any job that holds its own material. Start with the module that spends the most.
3. **Drive steps.** Each deterministic step runs one Haiku manager and one DeepSeek job, about 84K Claude tokens. A book's `mode figures` run spends about 500K tokens this way, whatever its size. Fewer steps, or one chained step, would cut this directly.
4. **Shared context in a job.** Test giving a job only the figures of its own part of the book, against the owner's wish that a figure may stand in any related Card.
5. **The explanation pass.** It ran at 4 images a job and now takes 8, the `deepseek_run` limit (`images` has `maxItems: 8`). It has had one 8-image probe and no full run. It spent about 15K Claude tokens a job on manager polling. More than 8 needs a bridge change.

## Open faults met on the way

- A drive answer came back with its start damaged twice (`ok":true,...` and `ok {"ok":true,...`), which halts a run at the strict reader in `scripts/workflow-shared/modules/zt-workflow-lib.js`. The step's full line is in `drive-lines/<step>.json`.
- Older stagings refuse the stage publication and term-link (`incompatible workflow contract`). The figure step needs no ledger, so Cards are written; those ledgers miss the pass.
- `hoyland2015path` ran the reader before explanations existed.
