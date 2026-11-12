# Proposal: stop the bilingual translators from losing the tails of long paragraphs

Status: proposal, 2026-11-11. P1, P2 and P3 implemented 2026-11-12: pdf-ocr-bilingual source commits 7757d08 and
e2164ab in `E:\google_scholar_archive` (installed), and zt-digest 0.53.0 (`tools/zt-digest/src/prepare/long_lines.rs`).
P4 (the archive and Card repair) runs in separate sessions.

## 1. The fault, shown on donner2026industry

The Introduction of *Industry and Industrialism* (Work 29946, bilingual file 1863) has a paragraph on the Pirenne
hypothesis. The English block is 2,888 characters on one line. The Chinese block stops after
"……首先，穆斯林控制的港口并未对基督教商人关闭。" The rest of the paragraph has no Chinese:

- "but it was nevertheless compellingly argued … right up to today. $^{5}$"
- "From our point of view, one key aspect of Pirenne's hypothesis … the rise of Islam."
- "Related to this … Fernand Braudel's massive *The Mediterranean* … sixteenth century. $^{6}$"

That is about 850 characters of English, with notes 5 and 6. The Chinese ends on a clean full stop, so it reads as
complete.

The same book has four more cuts of the same kind. Each one drops the text after character 2,000 of the English line:

| Block | English after char 2,000 | What the Chinese lost |
|---|---|---|
| 277 | 97 chars | "founded in 1929 by Lucien Febvre and Marc Bloch", note 3 |
| 285 | 852 chars | the Pirenne tail above, notes 5–6 |
| 307 | 143 chars | "if only subconsciously … mainly relied", note 16 |
| 401 | 548 chars | Smith's division-of-labour sentence, England's commercial development, Kelly on the Chinese agrarian sector, notes 11–12 |
| 572 | 21 chars | "a network of trading communities" became "一个……的网络" |

## 2. Root causes

**R1. The DeepSeek `read` tool cuts a line at 2,000 characters.** MinerU writes each paragraph as one line. A
translator that reads a unit file sees the first 2,000 characters of a long paragraph and the note
`(line truncated to 2000 chars)`. It translates what it sees and closes the Chinese at the nearest sentence end.
Sometimes it writes `……` for the part it did not see. This was found on 2026-11-07 (memory: read-cuts-long-lines).

**R2. The 2026-11-07 fix was lost before Donner was translated.** The fix (`wrap_long`, `write_long_file`,
`cut_short`, `READ_LIMIT`, a rule in `prompts/translate.md`) was written into the *installed* copy
`~/.claude/workflows/pdf-ocr-bilingual/` only. The compiled `__pycache__/pdfocr.cpython-314.pyc` (2026-11-06 22:50)
still contains these functions as proof. The fix was never put into the source repository
`E:\google_scholar_archive\workflows\pdf-ocr-bilingual\` (`git log -S wrap_long` finds nothing). The next
`install.py` runs from that source (commit 1970c66, 2026-11-07 21:48, and 73ca79c, 2026-11-08) copied the old
`pdfocr.py` and prompts over the installed copy. Today no `pdfocr.py` anywhere has `wrap_long`, and the orphaned
`tests/test_long_lines.py` fails. The Donner bilingual file was made in the batches of 2026-11-08, so it was
translated without the fix. Every PDF translated since then is exposed in the same way.

**R3. The checks look at the end of the Chinese, not at its coverage.** Three checks exist and all three pass a cut
that ends on a full stop:

- `suspect_pairs` in `pdfocr.py` flags a pair only when Han characters < 0.6 × English words. A full translation
  has 1.5–2 Han characters per word. The Pirenne block has about 1.1 per word (30 % lost) and passes.
- `cut_short` (2026-11-07, now lost) and the zt-digest `prepare` check look for a final `……` or a missing final
  stop. A cut that ends on `。` is invisible to them; a `……` in the middle (block 572) is invisible too.
- The pre-digest repair of 2026-11-11 used the same end-only test, found 6 Donner cuts and missed these 5.

**R4. The fill pass cannot repair a cut.** `prep-fill` hands the fill translator the same long English line, so a
flagged pair would be cut again at character 2,000.

**R5. The digest side has the same exposure, smaller.** The digest group files keep English lists and HTML tables as
single lines longer than 2,000 characters: Donner `groups/g03.md` line 25 (the occupations list, 3,862 chars) and
eight Hickey tables (2,257–6,414 chars). A digest job that reads these files does not see the rows past character
2,000. Chinese prose lines are short enough (one Chinese line of 2,000+ characters in the whole archive).

## 3. Scope in the archive

A scan of every `*.zh-bilingual.md` in the Scholar Archive (script and output:
`C:\Users\chenx\AppData\Local\Temp\claude\c2655a31b3642726\24c8ba5d-6ad0-46f5-93a9-0e2bae0cbbde\scratchpad\tail.mjs`
and `...\scratchpad\tail.json`):

- 656 English blocks have a line of 2,000 characters or more.
- 282 of them are suspect: a note mark, a number or most of the Latin names that occur only after character 2,000
  are missing from the Chinese, or the Chinese has a `……` that the English does not have. 221 are in books, 61 in
  papers, across 60 files.
- A sample of 12 suspects held 10 real cuts. The false alarms are names that the translator rendered in Chinese
  (for example Constantinople → 君士坦丁堡) and HTML tables. In Donner, 5 of the 9 suspects are real.
- The largest counts: Ugolini 43, Scheiner–Toral (Baghdad) 42, Donner 1981 22, Kennedy–Bessard 20, Bowman et al.
  19, Borrut–Donner 11, Morony 11, Sijpesteijn 10, Donner 2026 9, Hickey 9, Kennedy 2001 7, Hoyland 6. Books
  that were already "repaired" on 2026-11-07 still have cuts, because that repair used the end-only test.

The digested books carry the loss into their Cards, because a digest reads the Chinese only.

## 4. Proposed solution

### P1. Prevention: no DeepSeek job ever gets a line of 2,000 characters (fixes R1, R4)

Put one guard in `pdfocr.py` that every prep step uses to write a unit file (align, translate, glossary, polish,
terms, fill). The guard breaks each line longer than 1,500 characters at a sentence end (else at a space) with a
single newline. A single newline inside a Markdown paragraph does not change the block: the pairing by blank lines
and the rendered text stay the same. The apply step joins the lines back, so the final files keep one paragraph per
line. The guard also refuses to write a unit file that still has a line of 1,800 characters or more, so a new stage
cannot bypass it. Tests: restore `test_long_lines.py` and add one test per prep step.

Do the same in zt-digest `prepare` for the group files (R5): break a long list at commas and a long HTML table
between rows (`</tr>`), and refuse a group file with a line of 1,800 characters or more.

### P2. Make the source repository the only place a fix can live (fixes R2)

Restore the fix in `E:\google_scholar_archive\workflows\pdf-ocr-bilingual\` (the source), commit it, then install.
Change `install.py` so that it refuses to overwrite an installed file that differs from the hash it installed last
time (an edit made in the installed copy), and names the file. That turns this class of silent loss into a stop.

### P3. Detection by coverage, not by the ending (fixes R3)

Replace `suspect_pairs`' 0.6 threshold and `cut_short` with one deterministic coverage check per English/Chinese
pair. A pair is suspect when any of these holds:

1. A note mark (`$^{n}$`) of the English is missing from the Chinese.
2. A number of the English's last third is missing from the Chinese.
3. The Han-per-word ratio is below 0.75 × the median ratio of the same document.
4. The Chinese has `……` and the English has no ellipsis.

Run it as a gate after translate, after polish and after fill; suspect pairs go to the fill pass, which now reads
wrapped English (P1). Use the same check in zt-digest `prepare` so a digest refuses a cut source.

### P4. Repair what is already in the archive

1. Run the coverage check on all bilingual files and write one suspect list (the 282 above, re-counted with P3).
2. One DeepSeek job per about 40 suspects, with the English wrapped (P1): it confirms each suspect and writes the
   full Chinese of the block. About 7–8 jobs.
3. A script replaces each Chinese block in the archive file by exact match, once (the find/replace lesson of
   2026-11-07: never run a replace whose result still contains its find). Keep the originals as
   `*.before-cut-repair-2`.
4. For the digested books (Donner 2026, Hickey, Baghdad, Donner 1981, Kennedy–Bessard, Kennedy 2001, Sijpesteijn,
   Morony, Hoyland, and any econ book in the list), give the lost tails to the Cards that cover those paragraphs,
   as on 2026-11-07 (84 Cards then): one DeepSeek job per book finds the Card and writes the addition; a script
   applies it with an exact edit and relinks Terms. Box backup first.

## 5. Order of work

P1 + P2 + P3 first (code, tests, wiki, install), then P4 on the archive, then P4.4 on the Cards. P4 must use the P1
wrapping, or the repair jobs will cut the same lines again.

## 6. Decisions for the owner

1. Repair scope: all 60 bilingual files, or only the books that are digested?
2. Card backfill (P4.4): all digested books, or Donner 2026 first as a pilot?
