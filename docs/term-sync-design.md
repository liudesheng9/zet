# Terminology alignment as two skills

The owner's proposal (2026-11-06): one job must not read every book's terminology, because the cost would grow with the shelf. Terminology alignment leaves the digest workflow and becomes an independent skill, conducted and finished by Opus, with the basic work done by DeepSeek subagents. A book that is not yet aligned is matched against a standard terminology list, and what cannot be aligned is added to that list. The replacement review is separate from the alignment.

This replaces the first design (`zt-term-align` 1.0.0, one job over all rows of the shelf), which never ran on the live box.

## Why the cost stops growing

The first design cost one job over every row of every book. Its size grows with the shelf, and each new book repeats all earlier work.

The new unit is one unaligned book against the standard list:

```
standard list  (kept, grows slowly)        new book's rows
        │                                        │
        └────────  candidate index  ─────────────┘
                 at most 6 candidates per row
```

A script finds each row's candidates, so the agent reads only the new book's rows and a few standard entries for each. The job size follows the new book. The standard list can grow to any length without entering a job.

Measured on the three history books, with the 39 groups of the first design as the known answers (42 pairs that are not already spelled alike):

| Candidates a row | Known pairs found | Mean candidates a row |
|---:|---:|---:|
| 3 | 40/42 | 3.0 |
| 4 | 41/42 | 4.0 |
| 5 | 42/42 | 4.9 |
| 6 | 42/42 | 5.9 |
| 6, above 30% of the best score | **42/42** | **4.7** |
| 6, above 40% | 42/42 | 4.1 |
| 6, above 50% | 41/42 | 3.5 |

The index scores shared Han characters and character pairs, weighted by how rare each is in the list. The meaning column finds pairs that share no character, such as 改宗/皈依 and 土地税/哈拉吉: without it, 3 of 47 pairs were lost. Repeated 不用 words must count once. Added up, they found 40 of 42. The job takes 6 candidates above 30% of the best, which leaves a margin on both limits.

A row with no shared character pair has no candidate. It joins the list by script, and no agent reads it. A row whose term equals a standard term is the same term, and no agent reads it either.

Measured on a copied box:

| Book | List size | Rows | Settled by script | In the job | Job size | Time | Tool calls |
|---|---:|---:|---:|---:|---:|---:|---:|
| Baghdad (seed) | 0 | 915 | 915 | 0 | none | 11 s | 0 |
| Hoyland | 915 | 227 | 40 | 187 | 89 KB | 382 s | 4 |
| Donner | 1,083 | 250 | 39 | 211 | 103 KB | 123 s | 5 |

The list grew by 18% between the two jobs, and bytes per row held at 475 and 489. At three books this is not cheaper than the first design, whose one job read all rows for 0.43. The saving is in the slope: that job grows with every book on the shelf, and this one does not.

The first book seeds the list and spends nothing. Seed with the largest book, so later books find more matches.

## The standard list

One file per Literature type, `<archive_root>\terms\<type>.json`, kept with the box it describes and never in a repository, so a copied box has its own list and cannot touch the live one. The previous version stays beside it as `<type>.json.bak`.

```json
{"schema_version":1,"type":"history",
 "entries":[{"id":"s1","term":"撒拉森人","variants":["萨拉森人"],"meaning":"…","example":"…一句原句…",
             "books":{"scheiner2022baghdad":"撒拉森人","hoyland2015path":"萨拉森人"}}],
 "aligned":{"scheiner2022baghdad":"<sha256 of its rows>"}}
```

- `example` is one sentence kept with the entry, so evidence for an old term never needs an old tree.
- `aligned` records the state of each book's rows. A book with no record, or with rows that changed, is unaligned.
- `books` records what each book holds now, so a book whose spelling differs from `term` must follow a respell.

A Term list in the box stays the book's own record. The standard list is the shelf's index over them.

## Two skills

Each is a skill Opus conducts by hand, not a Workflow. Opus runs the deterministic commands, starts the DeepSeek jobs through a `dsmp-manager`, reads the result and decides whether to apply it. There is no drive step, so no Haiku-plus-DeepSeek pair spends about 84K Claude tokens to run one command.

### `zt-term-align`: decide and rename

```
zt-digest term-align -Step status              which books are aligned
zt-digest term-align -Step start -Book <key>   seed, or settle exact matches and write the jobs
  DeepSeek align job(s)                        rows + candidates in the message; thought, draft, self-check with sentences, correction
zt-digest term-align -Step apply -Work <dir>   rename this book's Terms, replace safe prose, update the list
zt-digest term-align -Step respell -Entry <id> -Term <spelling>   Opus corrects a standard spelling
```

The job answers with copied ids, `{"align":[{"row":"R12","to":"s34"}]}`. A row it leaves out becomes a new standard entry. Aligning a new book never rewrites an earlier book.

Opus finishes the work. It reads the decisions and their sentences before `apply` and removes any it rejects: on the test it removed 6 of 52, such as 伊斯兰征服→阿拉伯征服, where the wording is Donner's own argument, and 条约→保护契约, which is wider. It also corrects standard spellings, because the first book's spelling is not always good: the seed gave `法律意见（fatwa）` and `卡理斯玛`. After a `respell`, each book that holds the old spelling shows as following, and its next `start` and `apply` rename it with no agent. That path uses a book's own record and never a word that merely matches. On the test, 15 respells moved 31 Terms across the three books with no job.

A job's `review` runs in its own process, so it reads the entries frozen at the start and never opens the box. `apply` refuses a job with no answer: read as "nothing aligned", it would add every row as a new entry.

### `zt-term-review`: judge the remaining words

```
zt-digest term-review -Step start  -Book <key>  scan the book for variants of its aligned entries, write the jobs
  DeepSeek review jobs                          occurrences in the message; answer with ids
zt-digest term-review -Step finish -Book <key> -Relink <script>  replace the chosen ids, relink the book
```

It reads only the standard list and the live Cards, so it has no dependence on an alignment run and can run later or again. It looks at entries a second book shares: a book's other 不用 words stay R6's advice. Replacement stays as measured before: an occurrence with a stale Card is left alone, and an agent never writes a sentence.

## What leaves the JS workflow

| Part | Now | After |
|---|---|---|
| Align job, apply, cross-book check | Workflow `zt-term-align` | skill `zt-term-align` |
| Replacement review, relink after it | same Workflow | skill `zt-term-review` |
| Drive steps and manager prompts for them | module `zt-term-align.js` | deleted |
| Shelf rows in a history book's Term merge job (78 KB a job) | `shelf_canon` in `ledger.rs` | deleted |
| `term_cross` in the audit | `audit.rs`, `book-steps.ps1` | deleted; `term-align -Step check` |

The digest stays as it was before this work. A finished digest is followed, when the owner chooses, by the two skills. The cost is a second write over a new book's Cards, in exchange for a digest that knows nothing about the shelf.

## Test seams

Tests sit at the behaviour each step promises, on literal examples, not at helpers:

1. **Candidates:** given a standard list and a row, the right entry is among the 6, including a pair that shares only meaning.
2. **Start:** an exact term settles without a job; a row with no candidate joins by script; only the rest reach a job.
3. **Draft check:** an unknown id, an entry outside the row's candidates and two rows on one entry are faults.
4. **Merge:** an aligned row renames this book and leaves earlier books alone; an unlisted row becomes an entry; a rerun changes nothing; a book follows a respell, including a book that was renamed.
5. **Review scope:** only entries shared by a second book are scanned, and a word inside a longer kept term is left alone.
6. **Store:** a save keeps the previous version, and a copied box reads its own file.

The replacement functions keep their existing tests.

## Limits

- Recall was measured on one shelf of three history books, and a known pair the index misses becomes a duplicate entry. The decisions Opus reads show only what was aligned, so a miss stays quiet; `check` catches one that a 不用 word names.
- A word the review kept is listed again by a later review: Donner's second start held the 59 its agents had kept.
- The two jobs ran by hand on a copied box. The first run through a `dsmp-manager` will be a live one, and the ledger figures here were shared with another session, so job cost is not stated.
- The list is a file, so `VACUUM INTO` backups do not hold it; `.bak` keeps one version.
- History books only, by Literature type.
