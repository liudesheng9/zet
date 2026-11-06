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

## What the term alignment taught

Measured on a copied box, three history books, 1,392 Term rows (2026-11-06). The design is `docs/term-align-design.md`. These jobs ran by hand through `deepseek_run`; the Claude cost of the managers and drive steps has not been measured yet.

| Job | Jobs | Tool calls a job | Slowest job | Ledger cost | Result |
|---|---:|---:|---:|---:|---|
| Align, rows only | 1 | 1 | 388 s | 0.39 | 68 groups, about a quarter wrong |
| Align, review with real sentences | 1 | 5 | 555 s | 0.58 | 33 groups, none of the wrong ones, one true pair missed |
| Align, review also lists open conflicts | 1 | 4 | 315 s | 0.43 | 39 groups, the missed pair found |
| Residual, 150 occurrences a job | 6 | 2 | 260 s | 0.91 | 798 occurrences judged, 627 replaced |

The whole pass cost about 1.3 and rewrote 546 Cards.

### One long job beats many small ones when the material is small

A Term row is short, so 1,392 rows are 110 KB, and one job read them all with one tool call. Splitting would have cost more and lost the comparisons across books, which are the point. Before you split a job, measure the bytes of its material. Size follows the unit of the work (see above), and here the unit is the whole shelf.

### A self-check can hand the job its evidence

Rows alone misled the job: 地形→城市肌理 at 190 uses, 辩论→学术辩论, 精英→统治精英. A separate review job would have cost a second conversation and a drive step. Instead the job's own SELF-CHECK (`term-align -Step review`) prints faults and two real sentences per member, and the same conversation corrects itself. Every wrong group went, for 0.19 more.

### Show a job what it left undone

The reviewed job dropped a true pair. The review now also lists the known conflicts that no group settles, and the job decides each one. Groups rose from 33 to 39, and the job took fewer tool calls and cost less, because this run needed no rewrite round. One run each, so read this as direction and not as a measured saving.

### Answer with ids, and let the script do the edit

A residual job answers with the ids to replace and nothing else: 2 tool calls a job, 0.15 a job, about 0.001 an occurrence. The agent cannot change a sentence, so nothing needs reviewing afterwards. Use this shape whenever the decision is yes or no and a script can act on it.

### Cut the scope before you pay for the work

Counting every rejected word held 3,284 occurrences, about 22 jobs, most of them an old backlog no other book asked to change. Scoping to the rows in a group left 798 in 6 jobs. The narrow scope was also the correct one: the backlog words would have changed sentences for no cross-book reason.

## What the Term Card size test taught

Tested on a staging copy, 24 concept terms of `hoyland2015path` (rows 21-44), 2026-11-06, one variant at a time. Jobs ran by hand through `deepseek_run`, with the real skills, model and effort, and the real `term-cards -Step precheck`. Every variant passed it with `faults: 0`, and all 24 quotes were found in the book. Each row is one run, so small differences are noise. Citations are the `§` marks per term in `explanation` and `role_in_book`.

| Variant | Jobs | Tool calls a job | Ledger cost | Distinctions | Misreadings | Role, characters | Citations |
|---|---:|---:|---:|---:|---:|---:|---:|
| Current job, reads the book | 6 | 25 to 91 | 1.63 | 5.1 | 3.9 | 100 | 3.2 |
| 4 terms, source in the message | 6 | 2 | 0.45 | 5.8 | 5.0 | 108 | 0.7 |
| 8 terms | 3 | 2 | 0.39 | 4.3 | 4.2 | 82 | 0.0 |
| 12 terms | 2 | 2 | 0.37 | 5.4 | 4.5 | 79 | 0.0 |
| 16 terms, then 8 | 2 | 2 | 0.28 | 5.2 | 4.1 | 74 | 0.1 |
| 24 terms | 1 | 2 | 0.27 | 3.2 | 3.1 | 67 | 0.0 |
| **12 terms, sources labelled by section** | 2 | 2 | 0.29 | 4.9 | 4.0 | 101 | 6.5 |

### Browsing was the cost again, and size came second

Keeping 4 terms and putting the source lines in the message cut the cost by 72%. Moving from 4 to 12 terms saved little more. Stop the browsing first, then look at size.

### Too large a job thins the work before it fails

At 24 terms the job still passed every check and cost the least, but it wrote about a third fewer distinctions. A passing check does not show that a job held its quality. Score the content against a baseline at each size.

### A check cannot see what it does not ask for

The size test nearly recommended a job that had lost every section citation, because the checker does not count citations. The lines were labelled `s10.md:17`, so the agent had no section name to cite. Labelling each line with its section restored them. Compare the old output and the new one on what a reader uses, not only on what the checker counts. If a job drops something, look at its input before you blame its size.

### Give the sample the instructions point to

The Term Card instructions point to a sample at the end of the task file, and the task file has none. One baseline agent said it had to go and find the schema. Include the sample in the message.

### Make the job's size follow its new work, not its history

The first alignment job read every book's rows. It worked at three books, but its size grew with the shelf and it redid settled work. Keeping a standard list and letting a script find 6 candidates for each new row made the job follow the new book: when the list grew by 18%, bytes per row moved from 475 to 489. Measure the retrieval first, against answers you already trust. Here it found 42 of 42 known pairs, and that one number decided the design. It cost nothing, because no model ran.

Two lessons from that measurement. A number is only true for the code that produced it: the first Rust index summed 不用 words where the measured script took the maximum, and it found 40 of 42 until a second measurement exposed the difference. And at a small size the new shape may not be cheaper: two jobs here match the one old job at three books. Report the slope as the gain, and do not claim a saving the shelf has not reached yet.

### A self-check must not need the service

A job runs in its own process and may see another box. The review command read the standard list from the box, so on a copied box it would have read the live one. Freeze what a job needs into its work folder at the start. Its SELF-CHECK then reads files only.

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
2. **The same audit on other modules.** Term Cards, Cited Cards, Links and the card cycle have never had a session report. The message-only rule applies to any job that holds its own material, and a SELF-CHECK that returns evidence may replace a review job. Start with the module that spends the most.
3. **Drive steps.** Each deterministic step runs one Haiku manager and one DeepSeek job, about 84K Claude tokens. A book's `mode figures` run spends about 500K tokens this way, whatever its size. Fewer steps, or one chained step, would cut this directly.
4. **Shared context in a job.** Test giving a job only the figures of its own part of the book, against the owner's wish that a figure may stand in any related Card.
5. **The explanation pass.** It ran at 4 images a job and now takes 8, the `deepseek_run` limit (`images` has `maxItems: 8`). It has had one 8-image probe and no full run. It spent about 15K Claude tokens a job on manager polling. More than 8 needs a bridge change.

## Open faults met on the way

- A drive answer came back with its start damaged twice (`ok":true,...` and `ok {"ok":true,...`), which halts a run at the strict reader in `scripts/workflow-shared/modules/zt-workflow-lib.js`. The step's full line is in `drive-lines/<step>.json`.
- Older stagings refuse the stage publication and term-link (`incompatible workflow contract`). The figure step needs no ledger, so Cards are written; those ledgers miss the pass.
- `hoyland2015path` ran the reader before explanations existed.
