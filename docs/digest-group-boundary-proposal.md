# Digest group boundary

Status: pilot done 2026-11-07, variant C adopted for the history digest. See "Pilot results" and "Decisions taken".

## Problem

`docs/digest-job-size-proposal.md` raised the digest group limit to 25,000 characters, and the group count hardly moved: Morony went from 118 groups to 117, and Kennedy stayed at 53.

`group_units` in `tools/zt-digest/src/prepare.rs` stops a group at two walls. A group holds at most 3 units, and it holds units of one `chapter` only. `chapter` is the latest level-1 heading. The translated books write every section heading at level 1, so nearly every section is its own `chapter`. On Morony, 118 groups hold 120 sections, and the median group is 4,151 characters. The limit of 25,000 characters is almost never reached.

Four stages run one job per group or derive from groups, so these small groups set the job count of Digest, whole-book check, Links and link check.

## Proposed rule

A group fills with consecutive units in reading order, up to the soft limit of 25,000 characters. It stops early at a chapter start. A chapter start is a heading that carries a chapter or part number, such as `第三章`, `第二部分` or `Chapter 3`, or an appendix heading. The limit of 3 units goes. One unit larger than the limit stays whole, as it does now.

The rule replaces the old one. Finished books keep their groups, because a manifest is frozen at preparation.

## Simulated group counts

The simulation ran on the real sections of each staging folder, with a limit of 25,000 characters. It uses a simple pattern for chapter starts, and the pattern needs tuning: it found 4 chapter starts on Donner and 7 on Hoyland, where some chapters have no number in their heading.

| Book | Sections | Chapter starts | Groups now | Median now | Chapter stop, 3 units | Chapter stop, no unit limit | No chapter stop |
|---|---:|---:|---:|---:|---:|---:|---:|
| Morony | 119 | 17 | 118 | 4,151 | 49 | **36** (median 18,805) | 27 |
| Hoyland | 62 | 7 | 59 | 2,901 | 23 | **12** (median 22,302) | 8 |
| Donner | 57 | 4 | 52 | 4,073 | 21 | **13** (median 20,279) | 10 |
| Baghdad | 189 | 14 | 182 | 3,516 | 72 | **44** (median 20,167) | 39 |
| Atkinson | 249 | 2 | 229 | 2,214 | 84 | 27 | 27 |
| Hart | 47 | 3 | 21 | 7,224 | 18 | 10 | 8 |
| Modern VAT | 84 | 3 | 73 | 2,068 | 30 | 12 | 10 |

The proposed rule cuts the history books' groups to about a quarter or a third.

## The main risk: larger groups thin the notes

Existing runs show that a Digest job writes less per character as its group grows. The figures below come from the accepted drafts of four history books. Output is the characters of Card text per character of source.

| Group size | Morony Cards per 1,000 | Morony output | Baghdad Cards per 1,000 | Baghdad output | Donner Cards per 1,000 | Donner output |
|---|---:|---:|---:|---:|---:|---:|
| under 5,000 | 2.96 | 1.10 | 2.79 | 1.18 | 2.87 | 1.20 |
| 5,000 to 10,000 | 1.93 | 0.76 | 2.05 | 0.92 | 2.54 | 1.07 |
| 10,000 to 15,000 | 1.82 | 0.75 | 1.53 | 0.73 | 1.88 | 0.98 |
| 15,000 and more | 1.39 | 0.65 | 1.21 | 0.72 | | |

A group of 15,000 characters or more holds about 22 to 24 Cards. At that rate a group of 20,000 characters writes about half the Cards that the same text writes in small groups. This agrees with the whole-book check: a DeepSeek turn writes a limited amount whatever it holds, and it thinks only about what one user message gave it. There, 40 Cards in one message found 1 of 12 faults, and messages of 12 Cards found 9 (`docs/digest-cost-experience.md`).

So the rule must not send 25,000 characters in one message. The small groups may also have other causes, such as more short sections per character, and these are correlations from finished runs, not a controlled test.

## Proposed job shape

One Digest job stays one conversation, and it gets one message per source unit. Units over about 8,000 characters stay single messages, as they are. This is the shape the whole-book check already uses, and the manager's `then` list already carries it. The first message holds the shared rules and the first unit. Each later message holds the next unit and asks the writer to continue the same notes. One SELF-CHECK runs on the last message.

Whether this keeps the density at a lower cost is not known. The pilot below tests it.

## What the change touches

| Part | Effect | Action |
|---|---|---|
| Links | A group would hold about three of today's groups, so a job of 3 groups would hold about 9 | Return Links to 1 group a job and delete `Join-LinkJobs`. Morony stays at about 36 jobs |
| Whole-book check, link check | A group at the Morony rate holds about 39 Cards, near the 40-Card job | No change. A larger group stays one job with more messages |
| Anchors and the index | One anchor per group, so 36 anchors against 118, and the index has fewer lines | Check that the index reads well, and that `zt-literature-organize` can file Cards by section |
| Group spanning level-1 headings | `chapter` reads the first unit only, and `terms.rs` and the group title use it | Keep every heading in `headings`, which already happens |
| Chapter notes | A group takes the notes of one `chapter` only | Join the notes of all its units' chapters. History books attach no endnotes now, so this matters for the economics books |
| Event candidates (economics) | The limit of 12 candidates a group would lose candidates | Count the limit per unit |
| Models, Events, Findings (economics) | These layers plan by group | Test before any change to the economics digest |
| Mathematics digest | Units are statements with their own chapter structure | Leave out for now |

## Pilot

Run on a staging copy of one Morony chapter, one variant at a time, with real jobs, real instructions and the real SELF-CHECK. Keep the session files with `session-report.mjs`, and read `deepseek_usage` before and after each variant. Nothing touches the live box.

| Variant | Job shape |
|---|---|
| A | Today's groups, one job each |
| B | One group of about 20,000 characters, one message |
| C | One group of about 20,000 characters, one message per unit |

Score Cards per 1,000 characters, output per source character, source coverage, faithfulness on a sample of notes, tool calls, and ledger cost. Compare what a reader uses as well as what the checker counts. Adopt the rule only if C holds the density of A at a lower cost. Then run one small history book before any large one.

## Pilot results, 2026-11-07

Two chapters ran with real instructions, the real model and effort, and the coverage SELF-CHECK. The card check `group-finish -CheckOnly` could not run, because the archive sources changed after these books were digested, so both shapes faced only the coverage check. Each cell is one run. Names and years count those of the chapter's source text that appear in the Cards. Cost is the DeepSeek ledger in CNY.

**Morony, chapter 7 (4 sections, 21,961 characters)**

| Variant | Jobs | Model calls | Cost | Cards | Text | Cards per 1,000 | Names of 196 | Years of 42 | Coverage faults |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Published run | 4 | | | 43 | 17,446 | 1.96 | 180 | 31 | |
| A, one job per group | 4 | 38 | 0.81 | 40 | 16,048 | 1.82 | 176 | 27 | 0 |
| B, one big message | 1 | 19 | 0.30 | 26 | 13,474 | 1.18 | 168 | 27 | 0 |
| C, one message per unit | 1 | 15 | 0.35 | 49 | 18,596 | 2.23 | 193 | 33 | 0 |

**Donner, groups 19 to 22 (4 sections, 18,625 characters)**

| Variant | Jobs | Model calls | Cost | Cards | Text | Cards per 1,000 | Names of 174 | Coverage faults |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Published run | 4 | | | 50 | 19,801 | 2.68 | 131 | |
| A, one job per group | 4 | 58 | 0.92 | 53 | 21,682 | 2.85 | 138 | 0 |
| C, one message per unit | 1 | 14 | 0.42 | 47 | 19,741 | 2.52 | 138 | 0 |

C cost 57% and 54% less than A. B confirmed the risk: one message lost a third of the Cards. C wrote more than A on Morony and 9% less text than A on Donner, which equals the published text. C wrote fewer Link phrases on Donner, 49 against 84; the Link stage fills those gaps afterwards. A read of five sampled Cards of C against their source lines found their statements in the source and no repeated title. This is a small sample, and C needs its score on a whole book.

The cost covers the job shape only. It leaves out the jobs that a group count saves in later stages.

## Decisions taken

The owner handed these decisions to this session on 2026-11-07.

1. Boundary: a group stops at a numbered chapter, a part, or an appendix.
2. Unit limit: removed for the history digest.
3. Scope: the history digest first. The economics and mathematics digests keep their groups until their own layers are tested.
4. Job shape: one conversation, one message per unit, one check on the last message.
5. Links: a job holds up to 3 groups and up to 30 gaps, so a large group runs alone and the three digests share one rule.

## Release, 2026-11-07

Released in zt-digest 0.47.5, history 1.0.26, base and mathematics 5.11.38, economics 6.11.38. A dry run gives Morony 36 groups and Kennedy 15. A Haiku manager drove a three-message `then` job through DeepSeek in the required order, on a small test job. The whole flow has not run on a book. The manager rule says a conversation "answers finished"; it should say its status is `finished`, and that wording waits for the next release. The chapter notes of a group that crosses chapters still come from its first chapter only. The history books attach no endnotes now, so this waits until they do, or until the economics digest adopts the rule.
