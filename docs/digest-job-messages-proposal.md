# Proposal and design: Cited Cards and Term Card jobs as conversations of 4 messages

| | |
|---|---|
| Status | **Implemented 2026-11-08** (zt-digest 0.49.0, base 5.11.40, econ 6.11.40, math 5.11.40, history 1.0.28, zt-term-cards v16). Section 2 describes the design as built; section 5 records the verification and its cost. |
| Restart point | [docs/agents/digest-job-map.md](agents/digest-job-map.md) — the rows marked PROPOSED are this proposal |
| Code read | `main` at 3a1dcf33: zt-digest 0.48.2, `zt-digest-lib` v59, `zt-cited-cards` v6, `zt-term-cards` v14 |
| Read first | [docs/agents/workflow-principles.md](agents/workflow-principles.md), [docs/digest-cost-experience.md](digest-cost-experience.md) |

## 1. Proposal

### 1.1 The owner's order

| Job | Today | Target |
|---|---|---|
| Cited Cards | 12 works, 1 message | **4 messages × 6 works = 24 works** per subagent |
| Term Cards, concept | 12 terms, 1 message | **4 messages × 6 terms = 24 terms** per subagent |

Term Cards, mathematical (2 terms, one message) are not part of this order and stay as they are.

### 1.2 The Cited list read stays as it is

zt-digest 0.48.0 (2026-11-07) deleted the reference-list parser and its normalize jobs. The work moved to the **Cited list read**: `zt-digest cited -Step plan` cuts the reference material into parts of up to 60 lines (`PART_LINES`) and 12,000 characters (`PART_CHARS`), and one job takes 4 parts (`PARTS_PER_JOB`), one message per part. That job is already a conversation of 4 messages.

The first draft of this proposal (2026-11-08) cut its parts to 30 lines (120 lines a job). **The owner withdrew that change on 2026-11-08: the Cited list read keeps 4 parts of 60 lines, up to 240 lines a job.** It is out of scope here.

### 1.3 Why

- A DeepSeek turn writes about 35K–60K output tokens whatever it holds, and it thinks only about what its user message gave it. On the card review, the same 40 Cards found 1 of 12 natural faults in one message and 9 of 12 in messages of 12 (docs/digest-cost-experience.md).
- At 24 terms in one message, Term Cards passed every check but wrote a third fewer distinctions. Messages of 6 terms give each term the depth of a small job. One conversation of 24 terms reads the shared material (instructions, sample, the book's term list) once, as a cached prefix, instead of four times.
- The owner's rule (2026-11-06): one DeepSeek subagent does a long sequence or a large round in one job. Fewer conversations also mean fewer manager slots and fewer Claude tokens per book.

### 1.4 Expected effect (to be measured, not claimed)

| Job | Jobs per book | Depth per item |
|---|---|---|
| Term Cards, concept | about half (Hoyland: 19 jobs at 12 terms → about 10) | each message holds 6 terms, half of today's 12 |
| Cited Cards | about half (12 → 24 works a job) | each message holds 6 works, half of today's 12 |

## 2. Design

### 2.1 One shape for both jobs: a job is a message, four jobs are a conversation

Both jobs use the existing **Messages** pattern (`then`). The design as built keeps the existing **job** as the unit of everything that is checked and recorded (its rows, its output file, its SELF-CHECK, its ledger unit), and makes it **one message**:

- A job holds 6 items (`-PerJob` default 6). The compiler's `jobs` step lists four jobs, in plan order, as **one conversation**: the first job is the entry with its full files; jobs 2–4 are items of its `then`.
- **Message 1** is the first job's full file: the shared instructions, the sample, the shared material (the book's term list and notation, or the Term spellings and relation words) and its own 6 items. **Messages 2–4** are each job's **follow-up file** (`jobs\<id>.next.md`, written by `plan`): its own items, its own output file and its own SELF-CHECK line, and a short note that the shared material is in the first message and that the work must be as deep as in a job of its own.
- Each message writes its own output file and runs its own SELF-CHECK before it answers. A fault is fixed inside the message that caused it.
- The manager sends each next message only after the conversation answers `finished`, one at a time, with `deepseek_prompt` on the same `session_id`. The entry's answer is the answer to its last message.
- `finish` is unchanged: it completes each job's ledger unit from the drafts on disk. A conversation shorter than four messages is the last of a book; there is never an empty message.

This was chosen over per-part answers inside a larger job because every later step (precheck, check, readiness, review, apply, the ledger units) already works per job, so no step behind the planner changed.

One structure, no fallback ([workflow-principles](agents/workflow-principles.md)): the old one-message path for Cited Cards and Term Cards (concept) is deleted in the same change. No switch keeps it.

### 2.2 Term Cards, concept (`term_cards.rs`, `term-steps.ps1`, `zt-term-cards.js`)

| What | Today | New |
|---|---|---|
| Terms per job (= per message) | `DEFAULT_PER_JOB` 12, `PER_JOB_RANGE` (1, 12) | `DEFAULT_PER_JOB` 6, range unchanged |
| Jobs per conversation | 1 | `MESSAGES_PER_CONVERSATION` 4 (concept jobs in plan order; a math job stays alone) |
| Message 1 | the job file | unchanged: instructions, `WORKING_RULES`, `SAMPLE`, the book's other terms, the notation, its 6 terms with their source lines |
| Messages 2–4 | — | `jobs\<id>.next.md` (`job_text` with `follow_up`): its 6 terms with their source lines, `FOLLOW_UP_RULES`, its output and SELF-CHECK, prompt `NEXT_PROMPT` |
| SELF-CHECK | `term-cards -Step precheck -Staging … -Jobs <id>` | unchanged, per job (= per message) |
| `term-steps.ps1 start` | self-review clause per concept job | the same clause for each `then` item with its own verdict files; the line answers `messages` beside `jobs` (conversations) |
| Manager prompt | no `then` rule | the `then` rule, worded as in `zt-cited-cards` v6 rule 3; `deepseek_prompt` loaded (`zt-term-cards` v16) |
| Blueprint | `termCardsPerJob` default 12 | default 6 (base blueprint, so all variants) |

- The 2026-11-06 instructions stay: labelled source lines (`【§小节名】`), the full sample, the stated amounts (explanation 4–8 sentences, distinctions 4–6, misreadings 3–5, role 2–4) and the no-browsing rule. The working rules say "all material is in this message"; message 1 says the later messages bring the next terms, and each later message says its terms' material is in that message.
- The math job path (`DEFAULT_MATH_PER_JOB` 2) does not change.
- The claim lease (7200 s) covers a 4-message job; check the slowest job of the test against it.

### 2.3 Cited list read (`cited.rs`): no change

`PART_LINES` 60, `PART_CHARS` 12,000 and `PARTS_PER_JOB` 4 stay. One job covers up to 240 lines in 4 messages.

### 2.4 Cited Cards (`cited_cards.rs`, `cited-steps.ps1`, `zt-cited-cards.js`)

| What | Today | New |
|---|---|---|
| Works per job (= per message) | `-PerJob` default 12, range 1–12 | `DEFAULT_PER_JOB` 6, range unchanged |
| Jobs per conversation | 1 | `MESSAGES_PER_CONVERSATION` 4 |
| Job file | `jobs\<id>.md` | unchanged, plus `jobs\<id>.next.md`: the works, output and SELF-CHECK without the Term spellings and relation words |
| Draft, SELF-CHECK | one per job | unchanged, one per job (= per message) |
| Carried works | `carried_items.chunks(per_job)` | unchanged: carried jobs never reach a writer |
| Manager prompt | rule 3 sends `then` already | unchanged |
| Blueprint | `citedCardsPerJob` default 12 | default 6 |
| `cited-read-drive.mjs --cards` (backfill) | one message a job | sends every `then` message in the same conversation |

- The plan stores `per_job`, and `plan` reuses an earlier plan only when `per_job` is the same. A plan made at 12 is planned again at 6; jobs with a draft on disk keep their rows; carried works stay carried.

### 2.5 Shared manager rule

The `then` rule is now written twice (`zt-digest-lib` content manager, `zt-cited-cards` manager) and would be written a third time in `zt-term-cards`. Use the same wording in all three. The harness checks that each of these managers' rendered prompts carries the rule (render the template and read it: the 2026-11-06 change once landed in the wrong prompt).

### 2.6 Releases

- zt-digest 0.49.0 (job plans change shape). Base, econ, math and history blueprints: patch releases with the new defaults, so a prepared run stays drivable. Install in both scopes; `assemble.mjs install` must pass.
- Update in the same commit: the job map's PROPOSED rows (to "current"), `docs/wiki/digest-workflows.md`, `docs/wiki/digest-scripts.md`, `docs/wiki/zt-digest.md`, and the release history.
- Never cross the DeepSeek balance; stay on the official source at effort high.

## 3. Tests and acceptance

### 3.1 Unit and harness tests (test first)

As built, at the agreed seams:

- `term_cards.rs` (unit): 13 concept terms make jobs of 6, 6, 1; a concept job's follow-up text holds its own terms, source lines, output and SELF-CHECK and none of the shared parts; a math job has no follow-up; `conversations` groups concept jobs four to an entry across a math job, which stays alone.
- `tests/term_cards_plan.rs` (binary): `plan` writes `t002.next.md` for the concept job and none for the math job.
- `cited_cards.rs` (unit): nine jobs make conversations of 4, 4 and 1; `tests/cited_cards.rs` (binary): two jobs are one conversation whose second message reads only its follow-up file without the Term spellings; the default plan holds 6 works a job.
- `term-steps.tests.ps1` (69): a later message carries the self-review clause with its own verdict files; `messages` counts messages; a chunk counts conversations.
- `framework-harness.mjs`: defaults 6 reach both scripts; every Term Card writer manager loads `deepseek_prompt` and carries the `then` rule.

### 3.2 Measured test on a staging copy (the method of docs/digest-cost-experience.md)

Run the old and new shape on the same items, one variant at a time, with `session-report.mjs` and the usage ledger before and after.

| Job | Items | Baseline to beat or match |
|---|---|---|
| Term Cards, concept | Hoyland rows 21–44 (24 terms, the size-test set) | distinctions 5.0, misreadings 3.9, citations 8.4, 12 of 12 quotes found (the 12-term built code); role length near the published Cards |
| Cited Cards | 24 works of `morony2005iraq` | locations 4.5 a work, role 80 characters, relationship 61 characters (8 works a job) |

Accept when: every part passes its SELF-CHECK, the content scores are at or above the baseline, the slowest job stays inside the 7200 s lease, and the cost is reported as measured (more or less).

### 3.3 First real book

Then one real book in its single-stage modes (`mode term-cards`, `mode cited-cards`) on a staging copy, score it against the published Cards, and only then the next book.

## 4. Open points for the owner

1. ~~The Cited list read at 30 lines a part.~~ Withdrawn 2026-11-08: it keeps 240 lines a job (section 1.2).
2. Should the math Term Card job (2 terms) also become messages? Not part of this order.
3. The first measurement (section 5) shows the new shape costs more per item than the old one, while it writes as much or more. Decide whether to keep it, or to try to cut the re-read cost (for example a Cited job that searches less, or a stricter first draft).

## 5. Verification and cost (2026-11-08)

Real DeepSeek jobs on staging copies (`D:\zt-digest\_job-messages-test`, removed afterwards), official source, effort high, the same 24 items in both shapes, one run each. The old shape ran with zt-digest 0.48.2 and the new shape with 0.49.0, each with its own installed self-check. Jobs ran through the bridge directly (as a manager runs them), without the Term step script's self-review clause, in both shapes. Tokens come from each conversation's session file; cost is priced at the rates of docs/digest-cost-experience.md (1.0 uncached input, 0.14 cached input, 3.2 output, CNY a million).

| Module | Shape | Conversations | Model calls | Uncached in | Cached in | Output | Cost | Wall time |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Term Cards, Hoyland rows 25–48 | old: 2 jobs × 12 terms, 1 message | 2 | 14 | 46,657 | 1,424,640 | 93,103 | 0.54 | 250 s |
| | new: 1 conversation × 4 messages × 6 terms | 1 | 34 | 136,710 | 3,979,904 | 70,780 | 0.92 | 393 s |
| Cited Cards, Morony rows 817–840 | old: 2 jobs × 12 works, 1 message | 2 | 53 | 198,744 | 6,440,064 | 48,952 | 1.26 | 214 s |
| | new: 1 conversation × 4 messages × 6 works | 1 | 65 | 134,881 | 8,898,816 | 52,390 | 1.55 | 427 s |

Per message of the new shape: Term 0.36, 0.12, 0.22, 0.22 (message 1 needed 14 edits to clear its self-check); Cited 0.35, 0.22, 0.34, 0.63 (each message searches the source, so the conversation grows and every later call re-reads it).

Content (every draft of both shapes passed its SELF-CHECK, `faults: 0`):

| Term Cards (per term) | Explanation sentences | Distinctions | Misreadings | Role, characters | Section citations | Quotes |
|---|---:|---:|---:|---:|---:|---:|
| Published Cards (4-term jobs, 0.45.4) | 7.8 | 4.8 | 4.3 | 93 | 5.4 | 24/24 |
| Old shape, run 1 / run 2 | 6.4 / 7.1 | 4.8 / 4.7 | 3.4 / 3.9 | 72 / 63 | 7.2 / 7.8 | 24/24 |
| New shape | 7.0 | 4.5 | 4.0 | 100 | 9.1 | 24/24 |

| Cited Cards (per work) | Locations | Why, characters | Role, characters | Relationship, characters | Works without a location |
|---|---:|---:|---:|---:|---:|
| Published Cards (8-work jobs) | 2.5 | 78 | 90 | 64 | 2 |
| Old shape, run 1 / run 2 | 3.2 / 3.2 | 55 / 56 | 57 / 55 | 49 / 53 | 0 |
| New shape | 3.3 | 73 | 73 | 61 | 0 |

Reading: the new shape writes as much (Term) or more (Cited) per item, closer to the published Cards, and uses fewer output tokens on Terms. It costs more per item in this run: Term +69%, Cited +23%. The extra cost is cached re-reads of a longer conversation, not writing. One run each, so read this as direction. The account balance fell 2.95 CNY (235.44 → 232.49) over the window that holds most of these runs, about half the priced figure, so the rates above overstate the real price.

Total DeepSeek cost of the verification: the two old-shape runs (the first one's sessions were deleted before they were read, so it is priced like the second, about 1.80) and the new-shape run (2.47), about 6.1 CNY at the doc rates and about 3 CNY on the account.

## 6. Second test: 4 messages × 12 items (2026-11-08)

The owner asked whether 4 messages of 12 items (48 a subagent) cost less than 4 of 6. The setting needs no code change: `-PerJob 12` makes each job 12 items and the `jobs` step still puts four jobs in one conversation. The released default stays 6 until the owner decides. Same method as section 5, zt-digest 0.49.0, on 48 items per module: Hoyland Term rows 25–72 and Morony Cited rows 817–864. The old one-message jobs of 12 for rows 25–48 and 817–840 are the section 5 runs; rows 49–72 and 841–864 ran now.

| Module | Shape | Items | Conversations | Model calls | Uncached in | Cached in | Output | Cost | Cost an item | Against old |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Term Cards | old: 1 message × 12 | 48 | 4 | 24 | 169,009 | 2,327,808 | 190,409 | 1.10 | 0.0230 | — |
| | 4 messages × 6 (section 5) | 24 | 1 | 34 | 136,710 | 3,979,904 | 70,780 | 0.92 | 0.0383 | +67% |
| | **4 messages × 12** | 48 | 1 | 30 | 181,365 | 5,464,320 | 112,792 | 1.31 | 0.0272 | **+18%** |
| Cited Cards | old: 1 message × 12 | 48 | 4 | 108 | 455,974 | 13,285,504 | 105,880 | 2.65 | 0.0553 | — |
| | 4 messages × 6 (section 5) | 24 | 1 | 65 | 134,881 | 8,898,816 | 52,390 | 1.55 | 0.0645 | +17% |
| | **4 messages × 12** | 48 | 1 | 64 | 299,941 | 15,917,056 | 71,798 | 2.76 | 0.0575 | **+4%** |

Per message of 4 × 12: Term 0.31, 0.28, 0.34, 0.38 (an old job of 12: 0.23–0.32); Cited 0.62, 0.53, 0.87, 0.74 (an old job of 12: 0.55–0.70). Wall time of one conversation: Term 575 s, Cited 495 s (an old job: 200–270 s, and the old jobs run side by side).

Content (every draft passed its SELF-CHECK, `faults: 0`):

| Term Cards (per term) | Explanation sentences | Distinctions | Misreadings | Role, characters | Section citations | Quotes |
|---|---:|---:|---:|---:|---:|---:|
| Published Cards, rows 25–72 | 7.9 | 4.6 | 4.4 | 92 | 7.0 | 48/48 |
| Old 1 × 12, rows 49–72 (rows 25–48 in section 5) | 7.2 | 4.2 | 3.4 | 60 | 7.8 | 24/24 |
| 4 × 12, rows 25–72 | 7.0 | 5.0 | 3.0 | 64 | 7.4 | 48/48 |

| Cited Cards (per work) | Locations | Why, characters | Role, characters | Relationship, characters | Works without a location |
|---|---:|---:|---:|---:|---:|
| Published Cards, rows 817–864 | 2.5 | 71 | 91 | 53 | 5 |
| Old 1 × 12, rows 841–864 (rows 817–840 in section 5) | 3.3 | 58 | 67 | 48 | 0 |
| 4 × 12, rows 817–864 | 3.2 | 90 | 50 | 36 | 0 |

Reading:

- **4 × 12 removes most of the extra cost of 4 × 6** (Term +67% → +18%, Cited +17% → +4%), but **it is still not cheaper than one message of 12 in its own conversation.** Every model call re-reads the whole conversation, so a later message pays again for the earlier messages' material; the shared material that a conversation saves sending is already a cheap cached prefix in separate jobs.
- **Depth at 4 × 12 is about that of the old jobs, not more:** Term distinctions up (5.0 against 4.2–4.8), misreadings down (3.0 against 3.4–3.9); Cited why up (90 against 55–58), role and relationship down (50/36 against 55–67/48–53). The 4 × 6 shape (section 5) was the deeper one.
- One run each, so read these as a direction.

The account balance fell 2.35 CNY (232.06 → 229.71) during these runs (6.3 CNY at the doc rates). Cost of this second test: about 6.3 CNY at the doc rates, about 2.4 CNY on the account.
