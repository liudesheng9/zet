---
name: zt-term-align
description: "Align one ZET Literature book's terminology to the standard terminology list of its Literature type. Opus conducts and finishes it; DeepSeek agents do the matching. A new book is compared only with its own candidates from the standard list, so the cost follows the book and not the shelf, and what cannot be aligned joins the list. Use after a history digest, or when the user asks to align terms, check terminology across books, or build the standard term list. Replacing the remaining variant words is the separate skill zt-term-review. Give it a citation key, or take the next unaligned book from `status`."
---

# Align a book to the standard terminology list

You conduct this skill and you finish it. Scripts read, match and write. DeepSeek agents decide each row. You read their decisions and remove any you reject before anything is written. Design and evidence: `docs/term-align-design.md`.

Run nothing else on the box meanwhile, and align one book at a time.

## Steps

**1. See the shelf.**

```bash
zt-digest term-align -Step status
```

Each book is `aligned`, `not aligned`, or follows a respelled term, and the last line names the next one. On a new shelf, start with the largest book: it seeds the list, costs nothing and gives later books more to match.

**2. Back up the box** before the first write of a session: `VACUUM INTO` a file under `D:\zt\backups`, then check its integrity. The list is `<archive_root>\terms\<type>.json`, with one earlier version beside it as `.bak`.

**3. Start.** Use a fresh folder for each book.

```bash
zt-digest term-align -Step start -Book <key> -Work "D:\zt-digest\_term-align\<key>"
```

The line counts each row's fate: `same` (already a standard term), `new` (no candidate, joins the list by script) and `open` (goes to a job). With `"jobs":0`, go to step 6.

**4. Run the jobs.** `jobs` lists them in `drive\jobs.json`. Start one `dsmp-manager` agent per 8 jobs with this message, filling in the file and folder:

> Read the JSON array in FILE and run EVERY entry: deepseek_run with cwd WORK, prompt_files = the entry's files, prompt = the entry's prompt, model = the entry's model, reasoning_effort "high", wait_seconds 0. Poll with deepseek_wait (wait_seconds 90). Answer "needs_response" approvals: allow only a command that runs `zt-digest term-align -Step review` and a write of a JSON file under WORK\out. A session that ended in an error gets one more attempt at once, before you wait on any other job. Delete each conversation when it ends, and never archive one. Return the ids of the jobs that failed.

A job needs no other instruction: its file tells it to think in passes, write once, check itself on real sentences and correct itself.

**5. Read the decisions yourself.**

```bash
zt-digest term-align -Step review -Work "D:\zt-digest\_term-align\<key>"
```

Each decision shows the book's sentences and the standard entry. `faults` must be 0. Reject a decision when the two sentences do not name one concept in use, when one word is wider than the other, or when the author's wording is the argument. To reject, remove its row from the answer file in `out\`. A row with no decision becomes a new standard entry, so rejecting is safe.

**6. Apply.**

```bash
zt-digest term-align -Step apply -Work "D:\zt-digest\_term-align\<key>" -Relink "E:\zet\scripts\book-digest\term-relink.ps1"
```

This renames the book's Terms (row, Term Card, parts, link labels), replaces safe prose, enters the book into the list and relinks it. Earlier books are never touched. `plan_stale` means a Card changed since the start: run the start again. `no_answer` means a job has no answer file: run that job, because a missing answer is no decision.

**7. Check.**

```bash
zt-digest term-align -Step check
```

It lists each term of one book that another book rejects. A line left here is usually a real distinction. A line that shows a poor spelling goes to step 8. Report the counts, then run `zt-term-review` for this book.

**8. Correct a poor standard spelling.** The first book's spelling is not always good.

```bash
zt-digest term-align -Step respell -Entry <id> -Term "<spelling>"
```

This changes the list only. Each book that holds the old spelling now shows as following in `status`. For each, run steps 3 and 6 with a fresh folder: it has no job, and it renames the book by its own record. Do this before `zt-term-review`.

## Rules

- A book gives one row to a standard entry. A standard term changes only through `respell`, and only you decide one.
- Never run `apply` with an answer you have not read.
- A book's own 不用 words stay as they are, unless a second book shares the entry.
