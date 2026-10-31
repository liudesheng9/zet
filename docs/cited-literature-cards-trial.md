# Cited literature Cards: pre-pilot trial (2026-10-31)

This trial ran the built Cited tools (#217–#221: zt 0.49.0 debug build, zt-digest 0.37.1) on real book sources in scratch archives. It touched neither the live archive `D:\zt` nor the live service. It is evidence for the pilot (#222). It is not the pilot: the pilot runs mode `cited-cards` of the installed workflow on a copied box.

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

## 3. What the pilot still needs

1. Deploy zt 0.49.0 and zt-check 0.17.0 (the live service publishes applicability version 3, and the Cited stages need the `cited` type and `get_cited`).
2. Run `zt-econ-digest` with `{ mode: "cited-cards", out: <adopted staging copy> }` on a copied box, and qualify the six acceptance points of #216.
