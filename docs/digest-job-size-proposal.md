# Digest job sizes

Status: decided by the owner on 2026-11-06. Implementation follows this record.

This proposal sets the unit of one DeepSeek job in all three digest workflows (history, economics and mathematics). The base stages are shared, so a change here applies to all three. The Morony run (`morony2005iraq`, 118 groups, about 590,000 Chinese characters) is the reference for the job counts.

## Decisions

| Job | Unit before | Unit after | Change |
|---|---|---|---|
| Figure explanation | 8 images | 8 images | none |
| Term list, explore | about 30,000 characters | about 30,000 characters | none |
| Term list, merge | whole list | whole list | none |
| Cited list, normalize | 40 unparsed rows | **60 unparsed rows** | `NORMALIZE_PER_JOB` |
| Digest | 1 group, at most 20,000 characters | 1 group, **soft limit 25,000 characters** | `MaxChars` default |
| Whole-book check | up to 40 Cards | up to 40 Cards | none |
| Links | 1 group | **3 groups** | job merge in `layer-steps.ps1` |
| Link check | up to 40 Cards | up to 40 Cards | none |
| Organize notes, skeleton | as now | as now | none |
| Term Cards, concept | 4 terms in runs, 12 in the binary | **12 terms in runs** | workflow default and limit |
| Term Cards, mathematical | 1 term | **2 terms** | compiler and workflow default |
| Cited Cards | 4 works in runs, 8 in the binary | **12 works** | compiler range, workflow default and limit |
| Figure placement, Bridge | 80 Cards, 12 sources | as now | none |

The whole-book check and the link check were tripled on 2026-11-06 (`3efe0dff`) and stay at that size. Every job that is not named above stays as it is.

## Rules of each change

**Cited normalize.** A job holds at most 60 rows. Morony's 1,579 unparsed rows take 27 jobs in place of 40.

**Digest.** A group takes the next unit of its chapter while the group stays at or under 25,000 characters. One unit larger than the limit stays whole as its own group, so the limit is soft. The limit of 3 units a group stays.

**Links.** One start still claims every `link` unit, and each group keeps its own `link-start`, gap list, draft, binding fields, `SELF-CHECK` and `link-finish`. The ledger stays one unit per group. Only the DeepSeek job changes: consecutive groups that have gaps are joined, three to a job, so one conversation writes three drafts. A group with no gap needs no job, as it does now. A failed group is retried in the next round, joined with the other failed groups. The record commands of a joined job name its first group's unit, because a conversation is recorded once. Morony's 118 Link jobs become 40.

**Term Cards.** The binary has held 12 concept terms since `f3e744f3`, and no run has used that size. The workflow passed its own default of 4, and its limit of 8 refused 12. The workflow default and limit become 12. The size of 4 is not kept as a default. A mathematical job holds 2 terms.

**Cited Cards.** The compiler default and limit become 12, and the workflow default and limit become 12. A plan at a new size keeps every job with a draft on disk (`2d26fb73`). Morony's 396 jobs would be 132.

## Expected job count for a book like Morony

| Job | Before | After |
|---|---:|---:|
| Cited normalize | 40 | 27 |
| Links | 118 | 40 |
| Term Cards | 213 | about 73 (57 concept, 16 mathematical) |
| Cited Cards | 396 | 132 |

These four stages fall from 767 jobs to about 272. The Term Card estimate assumes that the 31 mathematical rows of the Morony plan stay mathematical.

## Measured limit of the digest change

A dry run of `prepare` at both limits gave these group counts.

| Book | 20,000 characters | 25,000 characters |
|---|---:|---:|
| Morony | 118 | 117 |
| Kennedy | 53 | 53 |

The new limit changes almost nothing on these books. A group can only join units of one chapter, and `prepare` starts a new chapter at every top-level heading. These translated books write their section headings at the top level, so most groups hold one unit. The Morony manifest has 120 sections in 118 groups and no group of 3 units. The change is applied as decided. A larger Digest job would need a change to the chapter boundary, which this proposal does not make.

## Risks

- `docs/digest-cost-experience.md` records that a job can thin its work before it fails. Term Cards at 24 terms passed every check and wrote about a third fewer distinctions.
- Cited Cards at 8 works wrote a little less than at 4 on Morony: locations 4.5 against 4.7, role text 80 against 88 characters, relationship text 61 against 68 characters. The size of 12 has not been tested.
- A Term Card job at 12 terms was tested with labelled sources and a full sample. It has not run through the workflow.
- A joined Link job has not run on a real book. Three self-checks in one conversation may raise the tool calls of a job.

## Scoring after the first real book

Score the first book against the Morony numbers: Cited locations and text length per Card, Term distinctions, misreadings and citations, Link gaps filled, faults per job, and tool calls per job. Keep the session files of that run with `session-report.mjs`, because a manager deletes each conversation when it ends.

## Implementation

1. `tools/zt-digest/src/cited.rs`: `NORMALIZE_PER_JOB` 60.
2. `tools/zt-digest/src/prepare.rs`, `scripts/book-digest/book-steps.ps1` and `zt-digest-prepare.js`: default `MaxChars` 25,000.
3. `tools/zt-digest/src/term_cards.rs`: `DEFAULT_MATH_PER_JOB` 2.
4. `tools/zt-digest/src/cited_cards.rs`: default and range 12.
5. `scripts/book-digest/workflow/zt-digest-base.blueprint.json`: `termCardsPerJob` 12, `termCardsMathPerJob` 2, `citedCardsPerJob` 12, with the limits.
6. `scripts/book-digest/layer-steps.ps1`: join Link jobs in threes. `dsh-link-instructions.zh.md` says that a job can hold up to three groups.
7. Tests first, wiki, blueprint patch bump, build, deploy, and install of all three digests in both scopes.
