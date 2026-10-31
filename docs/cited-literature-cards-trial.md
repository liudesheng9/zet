# Cited literature Cards: trial, pilot and live backfill (2026-10-31)

Sections 1–2 are the pre-pilot trial, section 3 the pilot on a box copy, section 4 the live backfill of the eight books. The trial ran the built Cited tools (#217–#221: zt 0.49.0 debug build, zt-digest 0.37.1) on real book sources in scratch archives. It touched neither the live archive `D:\zt` nor the live service. It is evidence for the pilot (#222). It is not the pilot: the pilot runs mode `cited-cards` of the installed workflow on a copied box.

## 1. The parser on the seven live books

`zt-digest cited -Step plan` ran on a copy of each book's staging manifest (the real bilingual source and section groups) against an empty scratch archive.

| Book | Entries | Split by the parser | With candidate lines | Candidates in a digested section |
|---|---|---|---|---|
| hansen2021demand | 35 | 35 (100%) | 35 | 35 |
| acemoglu2025nobel | 86 | 85 (99%) | 81 | 81 |
| acemoglu2007equilibrium | 48 | 48 (100%) | 48 | 48 |
| acemoglu2010labor | 43 | 43 (100%) | 42 | 42 |
| becker2025division | 66 | 66 (100%) | 66 | 64 |
| hart1995firms | 238 | 236 (99%) | 229 | 180 |
| ebrill2001modern | 223 | 221 (99%) | 181 | 179 |

The first run found three books at 0% (the Chicago style `Surname, First. 2000. “Title.”`), Hart wrapped entries and Hart's Chinese copy of the list. zt-digest 0.37.1 fixed them (test `the_plan_reads_chicago_years_wrapped_entries_and_skips_a_translated_list`). The entries left (`Forthcoming`, `undated`, one wrapped Hart line, one half-translated Hart line) go to the checked normalize job. The pilot's acceptance point 2 (at least 90% split by the parser) holds on every book.

## 2. One real writer job

A scratch archive held the Hansen tree's 15 section anchors and its live Term list (copied read-only). `cited -Step apply` wrote the Cited list Card at `hansen2021demand/1|b`, beside the Term list. `cited-cards -Step plan` made 9 jobs for the 35 rows. One DeepSeek V4.1 Flash job (official source, effort high) wrote job t005 (Deming and Kahn 2018, Deming 2017, Dessein and Prat 2019, Drucker 2007) in 162 seconds:

- It reported 28 locations for the four works and ended its self-check at `faults: 0`.
- `cited-cards -Step check` read the four rows as `ready` (the 31 rows of the other jobs had no draft).
- The relations are specific: Deming 2017 `依赖、推广、对照、印证` (the book extends its horizontal task exchange to vertical communication), Dessein and Prat 2019 `背景` (one footnote), Drucker 2007 `依赖、印证`.
- Read against the source, the sampled locations name the work: line 129/131 (footnote 8, Dessein and Prat 2019, English and Chinese), lines 113/115 (`(Drucker 2007)` and `Drucker（2007）` in the introduction).

The conversation was deleted after the answer.

## 3. The pilot on a box copy (#222)

zt 0.49.0 and zt-check 0.17.0 were deployed on the live service (backup `D:ztackupszt.sqlite3.pre-0.49.0-20261031T082047`). The pilot ran on a box copy: the live archive copied with `VACUUM INTO` to `D:zt-pilot-cited`, its own service on port 48719, and a copy of the Hansen staging adopted under the 5.10 contract and pinned to the copy by `zt-box.txt`. Mode `cited-cards` of `zt-econ-digest` ran in about 10 minutes.

- Found and fixed first: an adopted staging names the Chinese-only `book.zh.md` as its source, which holds no English reference list. zt-digest 0.37.2 and base 5.10.1 add `-Source` / `citedSource` (the plan freezes the whole source; the Cards read it).
- The list: 35 rows at `hansen2021demand/1|g` (the next free Side label in a backfill), 100% split by the parser.
- The Cards: 9 jobs, all 35 rows ready on the first pass, 35 Cards published, the book unit done.
- The Term-link pass was refused once by the box copy itself: it had no `artifacts` folder, and one Card shows a book figure. After copying `D:ztartifacts` into the box, mode `term-link` linked the terms (Term uses 1,399 → 1,513) and a second plan changed nothing.
- Faithfulness: the Deming (2017) and Premack (1978) Cards and the earlier t005 sample read against their source lines; C3 checks every location line mechanically.
- Tree diff against the live archive: 36 new `cited` Cards, 9 Cards with Term-link-only changes, 11 anchors changed only in their generated Reverse-links section, no other authored change.

All six acceptance points of #216 hold.

## 4. The live backfill (#223)

Backup before the backfill: `D:ztackupszt.sqlite3.pre-cited-backfill-20261031T085956` (4,352 Cards, integrity ok). Each book ran mode `cited-cards` on a copy of its staging (`D:zt-digest<key>-cited`, adopted against the live box), one book at a time.

| Book | List | Rows = Cards | Faults | Only in the list | Term-link second plan |
|---|---|---|---|---|---|
| hansen2021demand | `1|g` | 35 | 0 | 0 | 0 |
| acemoglu2025nobel | `1|ab` | 86 | 0 | 1 | 0 |
| acemoglu2007equilibrium | `1|j` | 48 | 0 | 0 | 0 |
| acemoglu2010labor | `1|g` | 43 | 0 | 0 | 0 |
| becker2025division | `1|f` | 66 | 0 | 0 | 0 |
| grossman2002integration | `1|h` | 28 | 0 | 1 | 0 |
| ebrill2001modern | `1|j` | 223 | 0 | 35 | 0 |
| hart1995firms | `1|d` | 224 | 0 | 2 | 0 |

761 new Cards (8 lists, 753 Cited Cards); the archive holds 5,113 Cards and passes its integrity check. The tree diff of every book against the backup shows only new `cited` Cards and Cards whose only change is Term-link macros.

Faults found on the way, each fixed test-first and resumed at its break (zt-digest 0.37.3–0.37.8):

- Nobel: names written without accents in the text (`Jaszi`, `Gutierrez`), a work cited by initials (`AJR (2001, 2012)`, new `as` field), and a normalized `Forthcoming` entry the Cards did not see (merged rows).
- Acemoglu 2010: `Salter W. E. G. 1966.` has no comma after the surname (C3 accepts the surname's first word).
- Grossman: a QJE list followed by its Chinese translation with Latin names (43 rows → 28; the authors end at the quote), and four stale Cited Cards then removed from the last one back.
- Ebrill: R6 fired on the quoted reference line (`CVAT`); that line is now exempt.
- Hart: the book spells `Boyco` in its list and `Boycko` in its text (one letter apart, with the year).

Known limits: on Acemoglu 2010, Grossman (first run), Ebrill and Hart the content is complete but the ledger's Cited Card book unit stays open, because a job unit failed in the first run waits out its retry backoff and the resume published without a claim. Becker's Term-link pass needed a second application on one Model part to settle (a pre-existing pass weakness, text preserved). Some location sentences name a figure (`图 5`), which the audit counts as named-but-not-shown (advice).
