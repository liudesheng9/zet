# Cited list: DeepSeek reads the reference material

Status: released 2026-11-07 (zt-digest 0.48.0, zt-cited-cards v6, base 5.11.39, econ 6.11.39, math 5.11.39, history 1.0.27). The owner asked for it after `kennedy2001armies/1|b` showed a messy and incomplete list.

## The fault

The Cited list of every finished book came from a rule-based parser (`cited.rs`, #219). It read the reference sections line by line and split each line into authors, year, title and venue with regular expressions. A DeepSeek normalize job saw only the entries the parser could not split. OCR and the bilingual pipeline give reference lists no fixed shape, and the parser broke in five ways. A survey of the 14 live lists on 2026-11-07 found these faults.

| Fault | Cause in the parser | Books |
|---|---|---|
| Several works in one row | An entry starts a row only when it opens `Surname, `. Entries that open `al-Azdī:`, `‘Arīb:`, `Bal., Ansāb:`, an OCR line join, or a preface sentence ran into the row before. | Kennedy, Donner, Baghdad, Hoyland |
| The title read as the authors | The author string is everything before the year, which fits `Author (Year) Title` only. In `Author, Title (Place, Year)` the title fell into the authors. | Kennedy, Donner, Baghdad, Morony |
| One error carried down a run of `——` entries | A dash run copies the previous row's authors, so a wrong author string passed to every following entry (`Bates, M., ‘History …’, 65 ‘The Dirham Mint …’`). | Kennedy, Donner |
| A passage listed twice | OCR page overlap and the bilingual merge repeat whole passages. The parser kept every copy, and each copy got its own Cited Card. | Atkinson 729 rows, Morony 517, Donner 150, Ebrill 88, Baghdad 62 |
| Wrong material | A book without a list under a known heading gave its author index (Morony), or nothing. | Morony |

The normalize job could not repair any of these faults. It saw only one unsplit entry at a time. It never saw the source lines around that entry, and it never saw the other works.

## The design

The script still finds the material. It takes the sections under the reference headings (`References`, `Bibliography`, `参考文献`, …; `-Heading` adds more, comma-separated), without their translation. It then cuts the material into parts of about 60 lines and writes one reading job per four parts. Each job runs in one conversation and gets one message per part, through the job's `then` list. Each line carries its source line number (`L7447: …`).

For every work, the job writes one entry: `lines`, `entry` (copied letter by letter), `authors`, `year`, `title` and `venue`. A line that is not a work goes into `skip`. The job's self-check (`zt-digest cited -Step precheck`) checks seven things:

- every line an entry names belongs to this job (R1);
- `entry` is a substring of its lines, compared after whitespace and quote marks are normalized (R2);
- the first surname is in `entry` (R3);
- a dash entry names the authors it repeats (R3);
- the year is in `entry`, and a year that `entry` holds is written in `year` (R4);
- the title is copied from `entry` (R5);
- every line of the part is in an entry or in `skip`, and the check shows the line's text when it is not (R6).

The job does not remove repeated works. `apply` refuses while any part lacks a passing answer. It then orders the entries by line and removes a work whose surname, year and title are already listed. It replaces a leading dash run with the authors. It indexes the lines that cite each work and writes `cited\merged.json`, the rows that the Cited Cards read, and the list Card.

The job's material is all in its message. Its answers are checked against source text it cannot change. The script removes the repeated works, because that decision is mechanical. This follows the long-job rules in `docs/agents/workflow-principles.md`.

## Changed lists keep their Cards

A new reading changes the rows. `cited-cards -Step plan` reads every live Cited Card of the list back into its draft item (the inverse of the render). It carries an item to a row when the row's entry has the same letters and digits as the Card's `文献：` line, and when the item passes the Card checks against its new row. Carried rows fill their own jobs, and their drafts are written at plan time. `jobs` lists only the other rows, and the old drafts move to `drafts-old-N`. `apply` then edits the Cards in row order. A backfill pays only for the works that the old list lacked or merged.

## Placement

The list stays where it is. The plan, `apply` and `cited-cards` find the list with `get_cited`, so an unorganized book (`<key>/1|b`) and a reorganized book (the list at the tail of the top line, for example `atkinson2015lectures/23`) take the same path. The Cards stay the list's Side successors.

## Cost

Kennedy's material is 281 lines in 5 parts and 2 jobs. Reading it cost 0.38 CNY on the official source at high effort. The answers held 251 works after 28 repeats were removed, against the parser's 237 rows, which included merged and duplicated rows. Every part passed its self-check, mostly on the first draft. The conversation of four parts read parts 2–4 for about 36 s each, at 3 tool calls a part.

## Backfill

Backfill reads every finished book again (`scripts/book-digest/cited-read-drive.mjs`, no workflow, no manager agent), then applies the list. It carries the live Cards, writes Cards for the new rows only (`--cards`), applies them and runs `term-relink.ps1`. A link into a Cited Card from another Card is pointed again to the Card of the same work. Results by book are in the table below.
