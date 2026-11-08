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

Measured on a copied box, three history books, 1,392 Term rows (2026-11-06). The design is `docs/term-sync-design.md`. These jobs ran by hand through `deepseek_run`; the Claude cost of the managers and drive steps has not been measured yet.

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

### The cheapest job is the one the design no longer needs

The rename design needed a second stage: 3 to 7 review jobs a book to judge each remaining word, a relink, a spelling step and a cross-book check. The owner's change to add a Link and leave every Term as it is removed all of them. The cost of a wrong decision also fell, from a damaged sentence to one Link a reader can ignore. Before tuning a job, ask whether its result needs to be this expensive to undo.

### Make the job's size follow its new work, not its history

The first alignment job read every book's rows. It worked at three books, but its size grew with the shelf and it redid settled work. Keeping a standard list and letting a script find 6 candidates for each new row made the job follow the new book: when the list grew by 18%, bytes per row moved from 475 to 489. Measure the retrieval first, against answers you already trust. Here it found 42 of 42 known pairs, and that one number decided the design. It cost nothing, because no model ran.

Two lessons from that measurement. A number is only true for the code that produced it: the first Rust index summed 不用 words where the measured script took the maximum, and it found 40 of 42 until a second measurement exposed the difference. And at a small size the new shape may not be cheaper: two jobs here match the one old job at three books. Report the slope as the gain, and do not claim a saving the shelf has not reached yet.

### A self-check must not need the service

A job runs in its own process and may see another box. The review command read the standard list from the box, so on a copied box it would have read the live one. Freeze what a job needs into its work folder at the start. Its SELF-CHECK then reads files only.

### A sample sets the amount of work

The first build of this change used a short sample with 1 distinction and 1 misreading. Real jobs passed every check and dropped to 2.8 distinctions and 2.0 misreadings. A full-sized sample and stated amounts brought them back. Real jobs from the built code then matched the published Cards:

| Job | Distinctions | Misreadings | Citations | Quotes in the book |
|---|---:|---:|---:|---:|
| Hoyland, 12 terms, new | 5.0 | 3.9 | 8.4 | 12 of 12 |
| Hoyland, same terms, published | 5.5 | 4.3 | 4.3 | |
| Baghdad, 12 terms, 154 KB job, new | 5.8 | 4.0 | 8.8 | 12 of 12 |
| Baghdad, same terms, published | 6.3 | 4.1 | 0.5 | |

`role_in_book` runs about a fifth shorter than the published Cards. Run the built code on a real job after a test: a test variant and the code that replaces it can differ in a detail that matters.

A plan at 12 terms makes 19 jobs for Hoyland, against 57. Baghdad's jobs reach 154 KB, because each job lists the book's other terms, and that size also held.

## What the Morony explanation fault taught

The size test above restored citations by asking every fact of `explanation` to read `§小节名 说……`, and it scored citations per term. On the first real book, morony2005iraq, that rule gave 85% of 679 explanations as a list of the book's statements (2 to 10% in eight other books). Citations belong in `book_definition`, `misreadings` and `role_in_book`; `explanation` carries none (zt-digest 0.47.6). Score a job by reading its Cards as well as by counting.

The repair (`scripts/book-digest/term-explain-repair.mjs`) rewrites the `## 释义` alone: 12 terms a job, message only, 17 KB, 2 tool calls. A first prompt that only forbade the marks ran 59 s and kept the list; a prompt that set the structure ran 200 s and gave 5.5 connected sentences. A whole Term job for the same work would resend every source line and rewrite fields that were right.

### The fault was in every book, not only Morony

Morony made it visible, but the first Term module's instruction already asked `explanation` to say "本书怎么用它", so every book's 释义 held "本书用它……" or "（§x）" lines. The module rule (no `§`, chapter, 本书, 作者/书中 说) selected 2,770 of 3,304 Term Cards outside Morony, Kennedy included. The same message-only repair ran as 237 jobs of 12 terms under 30 Haiku managers (about 31K tokens each), with 3 silent empty turns rerun; a rescan of every book then selected 0. Counting lines that start with `§` had shown 2 to 10% in those books and hid the fault: count with the rule the check uses.

### Judging the other books' mathematical Term Cards

Every book was checked for maths Term Cards built on no mathematics (2026-11-07). History books are false by rule (Donner 2, Baghdad 3). In economics books, 371 of 695 maths Cards had no real formula behind their plan row (footnote marks such as `$^{17}$` in plans before 0.44.2, writer promotions, and books digested before plans recorded evidence). A count of book lines with the term and a formula could not separate them: Atkinson's "dd 曲线" is real maths with no such line, and Ebrill's "增值税" has many.

A message-only judging job (12 Cards, the Card's definition formula and up to four book lines, answer by id) costs about 2 tool calls. The first rule, "the book gives no formula", flagged 42 Cards, and many were standard mathematical concepts with correct textbook definitions (基尼系数, 双曲贴现, 偏序). A second pass over those 42 alone, with the rule "the term is no mathematical object and the model was built for it", kept 28 and flagged 14 (一致同意, 配套拨款, 机会主义, 路径依赖, 波特假说 …). Because the second rule only narrows the first, the first pass's maths verdicts stand, and the second pass costs one small job a book. Each Haiku manager of 8 jobs cost about 31K tokens.

## What the Card review size test taught

Measured 2026-11-06 for the owner's request that one DeepSeek job carry three times the Card faithful check. A job held one group: 10.8 Cards on average over 689 real jobs. The 118 Card jobs of `morony2005iraq` (kept sessions) made 31 model calls a job and read 4.25M cached tokens, and each cost about 0.9. All three sets below come from real plan-time job files of finished books, run by hand through `deepseek_run` and `deepseek_prompt` on the official source, at high effort.

### Remove the loop before you measure the job

Most of the old cost was a fault no job could clear. `coverage -Draft` did not read a draft's `span`, so C2 stayed: 6,160 lines and 5.4 coverage runs a job on Morony. A log line `<file>; sections/s52.md:3-3` also counted as file-only, and every anchor did. The C3 block listed each note's `§52` as a number. All of this was input, and the first draft of a typical job already passed the Card check. These are fixed in `coverage.rs`. The job now has one check.

### One message thinks about what it holds, and no more

A turn wrote 35K to 60K output tokens whatever it held. Over 16 Cards or 40 it was the same, and 65,520 tokens looks like a cap on one turn. Planted errors hide this: they are coarse, and a thin read still finds them. Each row covers Donner groups 3-6 and 13-17 (78 Cards) unless it says otherwise.

| Job shape | Conversations | Planted errors found | Real-run `unfaithful` Cards flagged | Output tokens | Cost |
|---|---:|---:|---:|---:|---:|
| 1x, one group a job, message only | 9 | 16 of 18 | 6 of 12 | 442K | 2.05 |
| 3x, one message, one write | 2 | 15 of 18 | 1 of 12 | 107K | 0.64 |
| 3x, one message, a file per part | 1 of 2 | 7 of 9 | 1 of 4 | 85K | 0.47 |
| 3x, every sentence quoted from the source | 2 | 10 of 18 | 2 of 12 | 167K | 1.00 |
| 3x, effort max (set B only) | 1 | 8 of 9 | 2 of 8 | 63K | 0.49 |
| 3x, a tool prints the next part | 2 | 18 of 18 | 2 of 12 | 175K | 1.47 |
| **3x, one conversation, one message per 12 Cards** | **2** | **18 of 18** | **9 of 12** | **309K** | **1.78** |

The same final shape on Atkinson groups 29-32 (39 Cards, no orphans): 7 of 7 planted, 0.44, against 7 of 7 and 0.85 in four 1x jobs. Together, 117 Cards: 3 conversations against 13, 25 of 25 planted against 23 of 25, 9 of 13 natural against 6 of 13, 2.22 against 2.93, 24 tool calls against 30. Each row is one run. Costs are computed from tokens at the prices the usage ledger implies (about 1.0 miss, 0.14 cached, 3.2 output a million), because the source was shared with a live run. The natural Cards are the old run's own findings, not a right answer: the jobs also flag others, and those were not all read.

A second message is not a retry: the depth came back on message 2 and 3 of the same conversation, at about 30K tokens each. What resets the thinking is a user message. A tool result does not do it, a new file does not do it, written quotes do not do it, and more effort does not do it. Quotes came back on topic and proved nothing.

So the job's message count follows the unit of the work, one message per 12 Cards, and the conversation holds three times as many Cards at about a third of the old cost per Card (0.023 against 0.075). The manager sends each part; the Claude cost of that has not been measured on a run.

### What a plan looks like

Whole groups fill a job up to 40 Cards (`CARDS_PER_JOB`, `card.rs`). A group is not split, so 42 Cards is the largest job seen.

| Book | Jobs, old | Cards a job, old | Jobs, new | Cards a job, new | Largest | Largest message set |
|---|---:|---:|---:|---:|---:|---:|
| hansen2021demand | 15 | 10.1 | 5 | 30.4 | 39 | 171 KB |
| donner1981conquests | 52 | 12.4 | 20 | 32.3 | 40 | 244 KB |
| hoyland2015path | 59 | 9.4 | 16 | 34.7 | 40 | 248 KB |
| morony2005iraq | 118 | 11.9 | 41 | 34.3 | 40 | 234 KB |
| scheiner2022baghdad | 182 | 10.8 | 59 | 33.3 | 40 | 314 KB |
| atkinson2015lectures | 229 | 9.3 | 62 | 34.4 | 40 | 145 KB |
| hart1995firms | 21 | 21.2 | 14 | 31.8 | 42 | 196 KB |

The old job held 10.8 Cards, so 33 is three times that. A book's jobs fall to about a third.

### Render what you changed

The manager's new rule first landed in the wrong prompt, because the string it was inserted before occurs three times in the library. Rendering the template and reading it found that; no test would have.

## Job sizes set by the owner, 2026-11-06

The sizes in `docs/digest-job-size-proposal.md` are the owner's decision and have not run on a book: Cited Cards 12 works, Term Cards 12 concept and 2 mathematical terms, Links 3 groups, Cited normalize 60 rows. Cited Cards at 8 works wrote slightly less than at 4 on Morony (locations 4.5 against 4.7, role text 80 against 88 characters). Score the first book against those numbers.

## Digest job shape, 2026-11-07

One chapter as one conversation of one message per unit cost 0.35 and 0.42 CNY, against 0.81 and 0.92 for one job per group, at the published density. One big message cost 0.30 and lost a third of the Cards. Details are in `docs/digest-group-boundary-proposal.md`.

## Real run costs

| Run | Jobs | Cards read | Ledger cost | Per job | Per Card |
|---|---:|---:|---:|---:|---:|
| Nobel lecture | 2 | 117 | 0.25 | 0.13 | 0.0021 |
| Donner | 8 | 593 | 2.44 | 0.31 | 0.0041 |
| Re-read of Hart, Ebrill, Atkinson, Baghdad | 73 | 5,706 | 25.94 | 0.36 | 0.0045 |
| Atkinson repair | 29 | 2,266 | 10.72 | 0.37 | 0.0047 |

Larger books cost about twice as much per Card as the Nobel lecture. The likely reason, not yet tested, is that every job carries the book's whole figure table.

## What the Cited list backfill taught (2026-11-07)

Reading a whole book's reference material cost 0.38 CNY for Kennedy (281 lines, 2 jobs) and 104 parts over 31 jobs for the other 13 books, every part passing its self-check, most on the first draft. A job is message-only: the numbered lines are the material, so the reader never browses. Two lessons:

- **Carry what is already written.** Matching each new row to the live Card (and the previous plan's drafts) by its entry text left only the new works to writers: Atkinson 4 jobs of 72, Hoyland 2 of 16, Nobel 1 of 8. A changed list is otherwise a full rewrite.
- **Batch the writes, one writer at a time.** The per-Card `edit_card` path of `cited-cards apply` took 0.25 s a write while four writers queued on the one writer lane; a test box measured 103 ms a single edit and 78 ms an edit in a batch of 64. Publication now batches edits, creates and deletes, and a backfill publishes one book at a time. Details in `docs/cited-list-reading-proposal.md`.

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

## What the Term and Cited message conversations taught (2026-11-08)

The owner's order made Term Cards (concept) and Cited Cards one conversation of four messages of 6 items (`docs/digest-job-messages-proposal.md`, section 5 has the tables). On the same 24 items, one run each:

- **Depth held or rose.** Term role text 100 characters against 63–72, citations 9.1 against 7.2–7.8; Cited why/role/relationship 73/73/61 characters against 55/55/51, nearer the published Cards. Every draft passed its self-check.
- **Cost rose: Term +69%, Cited +23%.** Output fell (Term 71K against 93K tokens); the extra cost is cached re-reads. Every model call in a conversation re-reads all of it, so a later message with many tool calls costs more than the same message alone (Cited message 4: 0.63 against 0.22–0.35). A message-only job grows the conversation little; a job that searches the source grows it fast.
- **The first message carries the variance.** Term message 1 needed 14 small edits to clear its self-check; messages 2–4 cost 0.12–0.22 each.
- **Price per session, not per day.** The usage ledger keeps daily totals per provider, so a shared source hides one module's cost. Copy the session file before the conversation is deleted and sum `usage` (`inputTokens`, `cacheReadTokens`, `outputTokens`) of each `assistant/message` by `turn`. The balance in `provider-snapshots.json` gives the real spend; the doc rates here priced about twice the balance change.
- **More items a message helps, but a conversation is not cheaper than separate jobs.** At 4 messages × 12 items the extra cost fell to Term +18% and Cited +4% against one message of 12 in its own conversation (proposal section 6). A conversation saves only the shared prefix, which separate jobs already read as a cheap cache hit, and it adds the re-read of every earlier message on each later call. Use messages for depth (the 4 × 6 shape wrote the deepest Cards), not to save tokens.
