# Digest group boundary

Status: proposal, 2026-11-06. Nothing is implemented. The owner decides the items under "Decisions", and a pilot comes before any release.

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

## Decisions for the owner

1. Boundary: stop at numbered chapters, as proposed, or stop at no heading. The second gives fewer jobs, but a group can cross two chapters.
2. Unit limit: remove the limit of 3 units, as proposed, or keep it. Keeping it gives about 2.5 times as many groups on the history books.
3. Scope: history digest first, as proposed. The economics digest follows only after its Model, Event and Finding layers are tested. The mathematics digest stays as it is.
4. Links: return to 1 group a job when this rule lands, as proposed.
