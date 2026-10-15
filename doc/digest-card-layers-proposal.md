# Digest Card Layers: Proposal

Status: proposal (2026-10-15). Phase 1 (the audit report) is built: `scripts/book-digest/digest-audit.ps1`, the workflow mode `audit`, and the Audit stage at the end of a full run. The other phases are not built.

The `zt-book-digest` workflow builds models and hubs with care. A compiler plans each model or hub, a script builds and checks it, DeepSeek writes it, and a fix round fixes the faults. The other Cards of a digest do not get this care. One DeepSeek run writes the Idea Cards, Event Cards, new Topics, and the anchor Card of a group in four commands. No step checks them, reviews them, or reports their faults. Opus then fixes the structure in one long free-text job.

This proposal gives the other layers the same shape as the model and hub layers. It adds these things:

1. A read-only **audit report** for a book or for the agent Workspace.
2. A **book term table** at Prepare.
3. A **check, review, and fix** cycle for Idea Cards and Event Cards.
4. A **Topic gate** in the zt helper, so a new Topic does not repeat an old one.
5. A **check and fix** cycle for the Links of the link pass, and an **orphan pass**.
6. A script **skeleton** for the anchor Cards, the structure Card, and the Literature Card index.
7. A narrower **Organize** stage: Opus makes decisions, and scripts and DeepSeek do the edits.
8. New Card language rules for Idea Cards, Event Cards, and Links.

## Decisions

I made these decisions in this proposal. Each one is open for the user to change.

| No. | Question | Decision | Reason |
| --- | --- | --- | --- |
| D1 | Which pattern do the new layers use? | **The compiler pattern of the model and hub stages**: survey, plan, write, check, fix, report. | The pattern works. A second pattern doubles what an agent and the user must learn. |
| D2 | Who does the labour? | **DeepSeek writes and reviews. Scripts build and check. Haiku relays jobs and chooses limits.** No Sonnet in the new layers. | This follows the model compiler roles. Idea Cards need no structural plan like a model. |
| D3 | Does Opus stay in Organize? | **Yes, for judgment only**: Topic merges and the structure Card thesis. Scripts and DeepSeek do the edits. | Opus is the most expensive agent. It now does mechanical work that a script does better. |
| D4 | One checker or many? | **Extend `card-style-check.ps1`** with rule sets for Idea Cards, Event Cards, and Links. | The checker already knows the Card language rules R2 to R14. One checker gives one fault format. |
| D5 | What runs first, a check or a review? | **The check.** A DeepSeek review reads only flagged Cards and a sample. | A script check costs nothing. A review of every Card costs a DeepSeek job per Card. |
| D6 | How many fix rounds? | **One**, the same as `hubFixRounds`. The faults go to the same DeepSeek conversation. | The conversation keeps its cache prefix. A second round adds cost for few faults. |
| D7 | Do the new layers touch models? | **No.** They skip Model Cards and Model parts. They never restart a model manager. | Finished models stay as built. |
| D8 | Where does the Topic gate go? | **In the zt helper**, as an option. It is not a new stage. | Digest blocks run in parallel. A new serial stage stops the pipeline. |
| D9 | A new Card type for empirical findings? | **Not now.** See open question Q1. | A new type changes ZT itself. The audit report must show the need first. |
| D10 | Which part comes first? | **The audit report.** Its numbers decide the order of the other parts. | It is read-only and safe on the four books in the slip-box today. |

## Principles

These principles guide every proposal below.

1. **One shape for every layer.** Each layer goes through survey, plan, write, check, fix, and report. A layer can skip a step, but it does not change the order.
2. **Scripts do structure. Models do language and judgment.** A script places Cards, counts, and checks. DeepSeek writes text. An expensive model only decides what a script cannot decide.
3. **Check before review.** A mechanical check runs first. A model review reads only what a check flags, plus a sample.
4. **Bounded fix.** A layer fixes its faults one time. The run reports the faults that stay. It does not loop.
5. **Measure every layer.** Each layer prints one JSON line with its counts and faults. The run result shows every layer.
6. **The book is the authority.** Each Card records the source lines it comes from. A reviewer compares the Card with those lines.
7. **Limit attention.** The limits come from a survey of the slip-box, as in the hub stage. The user can override each limit.
8. **Do not disturb finished work.** A rerun changes only the Cards that a check flags. It never touches a model.
9. **Keep the pipeline parallel.** Gates go into scripts and the helper, not into new serial stages.
10. **Audit first.** Build the read-only report before the other parts. Let its numbers decide which layer to build next.

## Evidence

I measured the agent Workspace on 2026-10-15 with read-only zt calls.

| Measure | Value |
| --- | --- |
| Topic trees | 132. Median 11 Cards, 25th percentile 9, maximum 85. Three trees hold 5 Cards or fewer. |
| Cards in Topic trees | 1,928 |
| Event Cards in the Workspace | 9. Seven are in Topic trees and two are in `hart1995firms`. |
| Event Cards per book | `mascolell1995microeconomic` 0, `grossman2002integration` 0, `missingmarkets2026interviews` 0, `hart1995firms` 2 |
| Literature tree sizes | `mascolell1995microeconomic` 5,054 Cards and 343 models. `hart1995firms` 327 Cards and 22 models. |
| Hub Cards | 153 |

I also checked a sample: the 104 Regular Idea Cards of Topic trees 108, 115, 121, and 130.

| Measure | Value |
| --- | --- |
| Card language faults (`card-style-check.ps1`, rules R2 to R12) | 22: R4 (passive voice) 13, R2 (sentence length) 7, R3 (semicolon) 2 |
| Idea Cards with no inbound Link | 6 |
| Idea Cards with no kinded Link | 0 |
| Links per Idea Card | median 6 |

What the numbers show:

- The text of the Idea Cards is good in this sample. The gap is not the writing. The gap is that no step finds the faults that exist.
- The run has no record of these faults. I found them only with a manual check.
- The empirical book `missingmarkets2026interviews` has no Event Card. Its findings from 48 firm interviews are not events and not models. No Card type fits them well.
- The sample is small. It cannot show how faithful the Cards are to the book. Only a review against the source lines can show that.

## The layers today

| Layer | Plan | Check | Review | Fix round | Report | Rerun mode |
| --- | --- | --- | --- | --- | --- | --- |
| Model | Sonnet `plan.json` from a DeepSeek brief | `card-style-check.ps1` R2 to R14, `get_model` build report | DeepSeek, two rounds | yes | per model | `models` |
| Hub | Haiku limits from a survey | `get_hub` | none | one | counts and faults | `hubs`, with `state.json` |
| Idea Card | none | none | none | none | a count per group | none |
| Event Card | none | none | none | none | a count per group | none |
| Topic | none | none | Opus merges at the end | none | free text | none |
| Link (link pass) | script candidates (`relatedness.ps1`) | the helper allow-list only | Opus at the end | none | counts | `link` |
| Anchor, structure Card, Literature Card | none | none | Opus at the end | none | free text | none |

## Proposals

### 1. Audit report

A new script, `digest-audit.ps1`, reads a book or the whole agent Workspace. It changes nothing. A new workflow mode, `audit`, runs it through one Haiku agent.

The report gives these counts, per book and per Card type:

1. The Cards of each type: Idea, Event, Model, Model part, Hub.
2. The Card language faults per rule (R2 to R14) and per new rule (I, E, and L rules below).
3. The orphans: Regular Cards with no inbound Link.
4. The Cards with no source line.
5. The Links that ZT reads as "refers", with no Relation kind.
6. The Topics that the book created, and the size of each Topic tree.
7. The models that are not self-sufficient (from `get_model` `build`).
8. The required hubs that are missing (from `list_hubs`).
9. The book figures that a Card names but does not show.

The script writes `audit.json` and `audit.md` in the staging folder. The workflow result includes the JSON line. Run the audit on the four books before and after each change. The difference measures the change.

### 2. Book term table (Prepare)

Today only a formal model has a term table, in its setting. Rule R6 (one term, one meaning) cannot check an Idea Card.

The proposal:

1. A DeepSeek job at Prepare reads the section headings, the bold terms, and the book index.
2. It writes `terms.json`: each key term, the words not to use for it, and the first section that defines it.
3. The script writes the same table as `术语表` in a Side successor of the Literature Card.
4. The digest runs and the link runs read this table. `card-style-check.ps1` applies R6 to every Card of the book.
5. A model term table stays the authority inside its model.

The table also names the Topic candidates. The Topic gate (proposal 5) uses it.

### 3. Idea Card layer: check, review, fix

**Source lines.** Each `create_successor` call of a digest run gets an optional `src` field, for example `"src": "sections/s05.md:120-160"`. The helper removes the field before it calls zt. It writes the address and the source lines to the `-Log` file. A reviewer then knows which lines to compare.

**Check.** After each digest block, `card-style-check.ps1 -Book <key> -Types idea` checks every Idea Card that the block wrote. It applies R2 to R12, R6 with the book term table, and the I rules below.

**Review.** A DeepSeek reviewer reads two sets of Cards:

- every Card with an I-rule fault that a script cannot fix.
- a random sample of the other Cards (`ideaSample`, default 10%).

For each Card, the reviewer compares the Card with its source lines. It gives one of these verdicts as a JSON line:

- `ok`: the Card agrees with the book.
- `fix`: the text is wrong or unclear.
- `split`: the Card holds two ideas.
- `merge`: another Card holds the same idea.
- `unfaithful`: the book does not say this.

When more than `ideaFaultRate` of a group's sample is not `ok`, the reviewer reads every Card of that group.

**Fix.** The verdicts go to the conversation of the digest run that wrote the Cards. That run fixes the Cards one time. A `split` puts the second idea in a new Side successor. A `merge` goes to the Organize decisions, because it can move a subtree.

**Report.** The layer prints one JSON line: Cards checked, faults per rule, verdicts per kind, and faults still open.

### 4. Event Card layer

**Candidates.** At Prepare, a script finds candidate events in the source: years, dates, court cases, laws, named firms with a date, and crises. It writes `events.json` with the line of each candidate. The digest run reads its candidates. It decides if each one is a real event or a hypothetical example.

**Template.** The body of an Event Card uses fixed labels, so that a script can check it:

```
时间：……
地点：……
参与者：……
经过：……
来源：[[Key]] §n
证据作用：本事件支持 [[…]] 的……
```

**De-duplication.** One event gets one Event Card in the Workspace. Before it writes an Event Card, the run searches for an Event Card with the same date and an overlapping actor. When one exists, the run appends its source line and its `证据作用` line to that Card. It does not write a second Card.

**Check and fix.** The E rules below apply. Faults go to the conversation of the run that wrote the Card, one time.

### 5. Topic gate

Today a digest run calls `create_topic` when it finds no fitting Topic in its snapshot. Blocks run in parallel, so two blocks can create the same Topic. Opus merges the duplicates at the end.

The proposal adds an option to the zt helper, `-TopicGate <file>`:

1. Before a `create_topic` call, the helper compares the new title with the titles of every Topic and with the terms in `terms.json`.
2. When a title is the same after normalization, or very similar (character bigram similarity at or above `topicSimilarity`, default 0.8), the helper refuses the call.
3. The refusal is `N error topic exists: <address>`, then the usual resume line. The run files its Card under that Topic and runs the command again.
4. The helper locks the gate file during the check, so two blocks cannot create the same Topic at the same time.
5. A new Topic goes into the gate file at once. The next block sees it.

The gate is an option. Other users of the helper do not see it.

### 6. Link layer: check, fix, orphans

**Check.** After the link pass, a script reads every Link that the pass appended. It applies the L rules below. ZT already reads the Relation kind of each Link, so the script uses `get_card` kinds and does not parse the words itself.

**Fix.** The faults go to the conversation of the link run, one time. The run rewrites the sentence of each faulty Link with `append_card` or `edit_card` on its own Cards only.

**Orphan pass.** The script lists the Regular Cards of the book that no Card links to. Each orphan goes to the link run of a group whose Cards are its best candidates (`relatedness.ps1`). That run adds one reasoned Link to the orphan, or it reports that the orphan does not belong to the book. The second case goes to the Organize decisions.

**Report.** One JSON line: Links checked, faults per rule, faults fixed, orphans before and after, and the share of "refers" Links.

### 7. Anchor Cards, structure Card, and Literature Card

Today Opus writes the structure Card and the Literature Card index from nothing. A script can build most of both from the manifest.

1. `book-skeleton.ps1` writes the Literature Card index lines: one line per chapter, `第N章 章名 → [[anchor]]`. It keeps the `原书文档：` line.
2. It writes the frame of the structure Card. The frame holds one line per anchor in book order, the Model Cards with their levels, and the Event Cards.
3. A DeepSeek job writes the short text of each line: what the chapter says, and what each part adds to the part before it.
4. Opus writes only the title of the structure Card (the book's central thesis) and checks the order of the argument.
5. A check makes sure that each anchor title is a claim and each anchor has a `在全书中的位置：` paragraph with at least one Link.

### 8. Organize

Today one Opus agent does eight different jobs and returns free text. The proposal divides the work by who is best at it.

| Work | Today | Proposal |
| --- | --- | --- |
| Find duplicate Topics | Opus reads every tree | `organize-survey.ps1` lists candidate pairs: similar titles, shared terms, many Links between them |
| Decide merges and moves | Opus | Opus, from the survey list. It writes `decisions.json`. |
| Do merges and moves | Opus, call by call | A script runs the `move_card` and `delete_card` plans from `decisions.json`. |
| Rewrite Topic bodies | Opus | A DeepSeek job, from the tree after the merges |
| Check models | Opus runs `get_model` | The audit lists the faults. A DeepSeek job fixes outside Links. It does not change a model's parts. |
| Fix vague Links | Opus | The link check and fix (proposal 6) |
| Fix broken Links | Opus | A script, from `list_broken_links` |
| Structure Card | Opus writes all | Skeleton and DeepSeek (proposal 7). Opus writes the thesis. |
| Report | free Chinese text | A JSON schema: merges, moves, Topics after the merge, faults still open |

## New Card language rules

These rules go into `card-style-check.ps1`. A rule marked "advisory" reports a fault but does not count against a limit. The user chooses each number. The numbers here are first values.

### Idea Cards (I rules)

| Rule | Check |
| --- | --- |
| I1 | The title states a claim. Advisory: a title of six characters or fewer with no verb is a fault. |
| I2 | The body holds 80 to 200 characters of prose. Over 300 is a split candidate. Under 40 is a merge candidate. Formulas, Links, and figures do not count. |
| I3 | A Card of the book has a source line `来源：[[Key]] §n`, or a Link to its anchor with a section number. |
| I4 | The Card has at least one kinded Link to a Card other than its parent. |
| I5 | At least one Card other than its parent links to the Card. |
| I6 | No Card in the same Topic tree has a very similar title. Advisory: a similar title is a merge candidate for the review. |

### Event Cards (E rules)

| Rule | Check |
| --- | --- |
| E1 | The title holds a date, at least a year. |
| E2 | The body holds the labels `时间`, `地点`, `参与者`, `经过`, and `来源`. |
| E3 | The `经过` paragraph states what happened. It holds no evaluation. Advisory: the reviewer checks it. |
| E4 | At least one Idea Card links to the Event Card with a kind word, such as `例示` or `依赖`. |
| E5 | No other Event Card in the Workspace has the same date and an overlapping actor. |

### Links of the link pass (L rules)

| Rule | Check |
| --- | --- |
| L1 | ZT reads a Relation kind for the Link. A Link that ZT reads as "refers" is a fault. |
| L2 | The sentence of the Link holds at least `min_reason` characters other than the Link (default 8, the same as the hub stage). |
| L3 | The Link does not repeat a Link that the Card already has. |
| L4 | The target is in another group, another book, or the human Workspace. |
| L5 | The sentence follows R2 to R9. |

## The flow after the change

```
Prepare (terms.json, events.json, figures)
  ─▶ Digest block (DeepSeek writes, helper Topic gate, src log)
       ─▶ Idea and Event check ─▶ review (flagged + sample) ─▶ fix ×1 ─▶ report
       ─▶ Models for the block (unchanged)
  ─▶ Link ─▶ Link check ─▶ fix ×1 ─▶ orphan pass ─▶ report
  ─▶ Cleanup
  ─▶ Skeleton (script) ─▶ Organize survey (script) ─▶ Opus decisions ─▶ scripts and DeepSeek apply ─▶ report
  ─▶ Figures (unchanged) ─▶ Hubs (unchanged)
  ─▶ Audit (script) ─▶ the run result
```

The Idea and Event cycle of one block runs while later blocks still digest. It runs beside the model stage of the same block. It never touches a model, so the two do not collide.

## New arguments

| Argument | Meaning |
| --- | --- |
| `mode: "audit"` | Run only the audit report. Read-only. Needs `out`, and `citationKey` for one book. |
| `mode: "cards"` | Run the Idea, Event, and Link checks and one fix round on an existing digest. It never touches a model. |
| `cardChecks` | `false` skips the new checks in a full run. |
| `ideaSample` | The share of unflagged Idea Cards that the reviewer reads (default 0.1). |
| `ideaFaultRate` | When more of a group's sample fails, the reviewer reads the whole group (default 0.2). |
| `topicSimilarity` | The similarity at which the Topic gate refuses a new Topic (default 0.8). |
| `cardFixRounds` | Fix rounds of the Idea, Event, and Link layers (default 1). |
| `reviewEffort` | It applies to the Idea review too. Every DeepSeek effort stays `high` or `max`. |

## Cost

- The audit, the checks, the Topic gate, the skeleton, and the survey are scripts. They cost no model tokens.
- The fix rounds continue the conversation that wrote the Cards. They read the cached prefix.
- The Idea review costs one DeepSeek job per group. It reads only flagged Cards and the sample.
- Organize can cost less than today. Opus reads a short survey and writes a decision file. It does not read every tree.
- The audit report measures the real cost of each layer. Compare it with the DeepSeek usage of a run before the change.

## Rollout

Each phase ends when its exit test passes. The user decides when the next phase starts.

| Phase | Work | Exit test |
| --- | --- | --- |
| 1 | `digest-audit.ps1` and mode `audit` | The audit runs on the four books and the whole agent Workspace. The numbers agree with a manual check of one Topic tree. |
| 2 | I, E, and L rules in `card-style-check.ps1` | The rules run on the four books in mode `audit`. A manual check of 30 flagged Cards finds few false faults. |
| 3 | Mode `cards`: the Idea, Event, and Link fix rounds and the orphan pass on one existing book | The faults and orphans of that book go down. No Model Card or Model part changes. |
| 4 | `src` log, Idea review, Topic gate, book term table | A pilot digest of a short book or two chapters. The audit shows fewer duplicate Topics and gives a faithfulness rate. |
| 5 | Skeleton and the new Organize | Opus input and output tokens go down. The structure Card keeps its quality in the user's opinion. |

## Open questions for the user

| No. | Question | Recommendation |
| --- | --- | --- |
| Q1 | Empirical findings (such as the 48 firm interviews) fit no Card type. Do they need a new Card type `finding`, or an Idea Card with a fixed evidence template (`样本`, `方法`, `发现`, `来源`)? | The template first. Decide on a new type after the audit of one more empirical paper. |
| Q2 | Does the Idea layer need a plan step (a claim brief before the write), like the model brief? | Not now. Add it only if phase 4 shows a high `split` or `unfaithful` rate. |
| Q3 | Opus or Sonnet for the Organize decisions? | Opus for the first pilot. Compare with Sonnet on the same survey after it. |
| Q4 | What sample rate for the Idea review? | 10% first. Change it from the faithfulness rate of phase 4. |
| Q5 | Does phase 3 run on the existing four books, or only on new digests? | On one book first. It changes only flagged Idea Cards, Event Cards, and Links. |

## What this proposal does not do

- It does not change the model stage, the model compiler, or any model.
- It does not change the hub stage or the Figures stage.
- It does not add a Card type to ZT.
- It does not change the Card language rules R2 to R14. It adds rules beside them.
- It does not rerun a finished stage. Mode `cards` changes only the Cards that a check flags.
