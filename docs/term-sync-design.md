# Term synchronization

The owner's decisions (2026-11-06):

- One job must not read every book's terminology, because the cost would grow with the shelf. Each new book is matched against a standard terminology list, and what cannot be matched joins the list.
- It is an independent skill, conducted and finished by Opus, with the basic work done by DeepSeek subagents.
- A Term never changes. The result is a Link from a Term Card to its equivalents in other books, and nothing else.
- One skill, `zt-term-sync`: running it is running the whole tool.

This replaces two earlier shapes, which never ran on the live box: the Workflow `zt-term-align` 1.0.0 and the skills `zt-term-align` and `zt-term-review`. Those renamed Terms and replaced prose. With no rename there are no remaining variant words to judge, so the review job, its 3 to 7 jobs a book, the respell step, the relink and the cross-book check are all gone.

## The shape

```
status → start → job(s) → Opus reads → apply
```

One start, one queue of jobs under `dsmp-manager`, one finish. A manager reads no files, so Opus pastes the job array into its message. There is no Workflow, so no Haiku-plus-DeepSeek pair spends about 84K Claude tokens to run one command.

| Step | Who | What |
|---|---|---|
| `status` | script | which books the list knows; the largest waiting book is next |
| `start` | script | sorts each row, freezes the offered entries, writes the jobs |
| job | DeepSeek | rows and candidates in the message, thought, draft, self-check with sentences, correction; answers copied ids, `{"align":[{"row":"R12","to":"s34"}]}` |
| `review` | Opus | reads each decision with its sentences and removes the ones it rejects |
| `apply` | script | enters the book into the list and writes the Link lines |

A row is `same` (its spelling is one a book already holds), `new` (no candidate), `known` (this book entered it before) or `open`. Only `open` rows reach an agent. The first book seeds the list with no agent.

## Why the cost stops growing

A script finds each row's candidates, so the agent reads only the new book's rows and at most 6 standard entries each. The list can grow to any length without entering a job.

Measured on the three history books, with 42 known pairs that are not spelled alike:

| Candidates a row | Known pairs found | Mean candidates a row |
|---:|---:|---:|
| 3 | 40/42 | 3.0 |
| 5 | 42/42 | 4.9 |
| 6, above 30% of the best score | **42/42** | **4.7** |
| 6, above 50% | 41/42 | 3.5 |

The index scores shared Han characters and character pairs, weighted by how rare each is in the list. The meaning column finds pairs that share no character, such as 改宗/皈依 and 土地税/哈拉吉. Repeated 不用 words must count once.

The align jobs of the previous shape are this job with the same material:

| Book | List size | In the job | Job size | Time | Tool calls |
|---|---:|---:|---:|---:|---:|
| Hoyland | 915 | 187 | 89 KB | 382 s | 4 |
| Donner | 1,083 | 211 | 103 KB | 123 s | 5 |

Bytes per row held at 475 and 489 while the list grew 18%.

The final shape, on a copied box (2026-11-06): seed 915 entries with no agent; Hoyland 59 Term Cards linked; Donner 210 rows in one job under a `dsmp-manager`, 102 KB, 5 tool calls, about 310 s, 22K manager tokens. The job made 33 decisions, and Opus rejected 3 (萨达卡→施舍税, 苏勒赫→条约, 萨瓦菲→国有土地), leaving 68 Term Cards linked. Term faults and link faults were 0 on all three books, the Term rows did not change, and the Reverse link showed on the earlier book. The job's DeepSeek cost was not read: the ledger is shared with other sessions.

## The standard list

One file per Literature type, `<archive_root>\terms\<type>.json`, kept with the box it describes, so a copied box has its own list. The previous version stays beside it as `.bak`.

```json
{"schema_version":1,"type":"history",
 "entries":[{"id":"s1","term":"撒拉森人","variants":["撒拉逊人"],"meaning":"…","example":"…一句原句…",
             "books":{"scheiner2022baghdad":"撒拉森人","hoyland2015path":"萨拉森人"}}],
 "aligned":{"scheiner2022baghdad":"<sha256 of its terms>"}}
```

- `term` is only the entry's label: the first book's spelling. It has no standing over any book.
- `books` holds each book's own spelling. A later book that spells a word as any book does is `same`, with no job.
- `aligned` says a book's rows are the ones the list knows.
- The list holds no address, because a Card moves. `apply` reads addresses live through `get_terms`.

## The Link

`apply` writes one line at the end of `## 释义` on this book's Term Card, and only there:

```
等义术语：[[hoyland2015path/37|c]]、[[scheiner2022baghdad/4|a]]
```

- It links every other book that holds the entry. The Link chip shows the target's address and title.
- It is an ordinary Link, not a Term link: an equivalence is a relation between Cards, and a Term link would add a false Term use.
- An earlier book is never written. Its Card shows the Link as a Reverse link, so a new book's work stays in the new book, and a locked earlier tree is no obstacle.
- The line sits in an existing section, so the Term template and `zt-check` do not change.
- A later run replaces the line, so a Card holds one. A rerun that finds the same links writes nothing.

## Test seams

1. **Candidates:** the right entry is among the 6, including a pair that shares only meaning.
2. **Sorting:** an exact term or a spelling another book holds settles without a job; a row with no candidate joins by script; a row this book entered needs nothing.
3. **Draft check:** an unknown id, an entry outside the row's candidates and two rows on one entry are faults.
4. **Merge:** an aligned row joins its entry with its own spelling; an unlisted row becomes an entry; a rerun changes nothing.
5. **Equivalents:** a book links every other book of its entries; a term no other book holds has none.
6. **Link line:** it closes `## 释义`, nothing else changes, a rerun replaces it.
7. **Store:** a save keeps the previous version, and a copied box reads its own file.

## Limits

- Recall was measured on one shelf of three history books. A pair the index misses is one Link fewer, which is the cheapest failure this design has.
- A later rerun of `term-cards` on a book would write its Term Cards again and lose the line; `status` would not show it.
- The list is a file, so `VACUUM INTO` backups do not hold it. `.bak` keeps one version.
- History books only, until a second shelf has a `-Type`.
