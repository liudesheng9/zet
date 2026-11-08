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

## Faults met in the backfill, and their fixes

| Fault | Book | Cause | Fix |
|---|---|---|---|
| A publication refused: `link target <key>/10 does not exist` | Acemoglu 2007, Acemoglu 2010, Hansen | A reorganized tree moved its section Cards; the manifest's group anchors still named the old addresses | A carried Card keeps the `另见` anchor its own body names; `apply` drops a manifest anchor that names no Card |
| 2 rows never passed C3 | Hoyland (`Chronicle of 741`) | An anonymous work had one long title word; C3 asked for a surname or two title words | A three-digit number counts as a title word |
| 18 rows never passed C3; 642 of 1,553 works had no surname | Morony | **Root cause:** an author index names the author once per line and lists that author's works after it. The reader copied each work as written, and the reading check R3 demanded that the surname stand inside each entry, so 1,164 of 2,819 entries had no authors. Without a surname a work has no id of its own, no citing lines and no surname for C3, and two authors' works with the same title were one repeat | `apply` gives an entry without authors the authors of the entry before it on the same line (no reader change). Works without a surname: 642 → 80; works: 1,553 → 1,611. C3 also accepts the one title word of a work without a surname whose title has one word |
| 1 row never passed C3 | Baghdad (`al-Sharaf` in the list, `Sharaf` in the notes) | The surname kept its Arabic article | C3 also finds the surname without `al-`, `el-`, `ad-` … |
| A list read again lost the written drafts | Morony | The carry read only the live Cards, which were not yet published | The carry also reads the previous plan's drafts by their entries |
| A Link into a deleted Cited Card could not be relinked | Donner | `relink` needs an existing `from` | `cited-relink.mjs` rewrites the Link in the source text; 11 Links whose old row had merged several works were decided by hand from their sentences |
| Links into Cited Cards drifted | Donner, Atkinson, Baghdad, Morony | `apply` writes Cards in row order, so a row that moves takes another address and a Link keeps the old one | `apply` records each inbound Link with its Card's work and repoints it to the Card that holds that work, before deleting stale Cards. After the repeat rerun, an exact position map found 3 shifted Links, now repointed |
| The same work twice | Atkinson 10, Donner 2, Baghdad 1, Morony 39 | A repeated passage read twice came back with two titles, so the surname, year and title key differed | A row whose folded entry text an earlier row has is a repeat too |

## Publication speed

The Cited Card `apply` wrote each existing Card with its own `get_card` and `edit_card` call, and each stale Card with two `delete_card` calls. Only new Cards went in `apply_batch`. A fresh digest writes only new Cards, so it never met this path. A backfill rewrites nearly every Card in place.

Measured on 2026-11-07:

- A Card read costs about 6 ms on the live archive. Reads are not the cost.
- On a test box built from the live backup (24,299 Cards), 64 single `edit_card` calls took 6.6 s (103 ms each). One `apply_batch` of the same 64 edits took 5.0 s (78 ms each). Batching saves about a quarter. The rest is the per-write work of the archive: the write gate with the book's Term list, the link and Term indexes, the revision record and the commit.
- On the live archive, Morony's and Baghdad's publications ran at the same time as two Term-link passes. The archive has one writer lane, so the four writers queued. Together they reached about 4 writes a second (0.25 s each), about 2.5 times slower than one writer alone. Each book took about 12 minutes.

The fix:

- `apply` reads the current Cards with `get_cards` (200 a call).
- It writes changed Cards in `apply_batch` calls of 64 `edit_card` items, each with its authored `base_fingerprint`.
- It deletes stale Cards in previewed, confirmed tree batches of 64, from the last one back.
- Publications run one book at a time, never beside another writer.

The cost left is the archive's own work per write, about 80 ms. Lowering it is work for the `zt` service, not for this module.
