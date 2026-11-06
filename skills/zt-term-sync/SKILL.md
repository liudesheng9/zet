---
name: zt-term-sync
description: "Link the equivalent Terms of ZET Literature books: each Term Card of a book gets a Link to the Term Cards that name the same concept in the other books of its Literature type. No Term, sentence or Card text changes otherwise. Opus conducts and finishes it, DeepSeek agents do the matching, and a standard terminology list keeps each job to the new book's rows and their candidates. Use after a digest, or when the user asks to synchronize, align or link terms across books. Running this skill does the whole job for every waiting book, or for one citation key if given."
---

# Link equivalent Terms across books

You conduct this skill and you finish it. Scripts read, match and write. DeepSeek agents decide each open row. You read their decisions and remove any you reject before anything is written. Design and evidence: `docs/term-sync-design.md`.

Run nothing else on the box meanwhile, and work one book at a time.

## Steps

**1. See the shelf.**

```bash
zt-digest term-sync -Step status
```

Each book is `linked` or `not linked`, and the last line names the next one, largest first. Do steps 3 to 6 for each waiting book, or only for the book the user named. The largest book seeds an empty list and costs nothing.

**2. Back up the box** before the first write of a session: `VACUUM INTO` a file under `D:\zt\backups`, then check its integrity. The list is `<archive_root>\terms\<type>.json`, with one earlier version beside it as `.bak`.

**3. Start.** Use a fresh folder for each book.

```bash
zt-digest term-sync -Step start -Book <key> -Work "D:\zt-digest\_term-sync\<key>"
```

The line counts each row's fate: `same` (a spelling another book holds), `new` (no candidate, joins the list by script), `known` (entered earlier) and `open` (goes to a job). With `"jobs":0`, go to step 6.

**4. Run the jobs.** `file` names `drive\jobs.json`. A manager reads no files, so read that array yourself and paste it into the message. Start one `dsmp-manager` agent per 8 jobs with this message:

> Run EVERY entry of this JSON array: JOBS. For each entry: deepseek_run with cwd WORK, prompt_files = the entry's files, prompt = the entry's prompt, model = the entry's model, reasoning_effort "high", wait_seconds 0. Poll with deepseek_wait (wait_seconds 90). Answer "needs_response" approvals: allow only a command that runs `zt-digest term-sync -Step review` and a write of a JSON file under WORK\out. A session that ended in an error gets one more attempt at once, before you wait on any other job. Delete each conversation when it ends, and never archive one. Return the ids of the jobs that failed.

A job needs no other instruction: its file tells it to think in passes, write once, check itself on real sentences and correct itself.

**5. Read the decisions yourself.**

```bash
zt-digest term-sync -Step review -Work "D:\zt-digest\_term-sync\<key>"
```

Each decision shows the book's sentences and the list's entry. `faults` must be 0. Reject a decision when the two sentences do not name one concept, or when one word is wider than the other. To reject, remove its row from the answer file in `out\`. A row with no decision becomes a new entry, so rejecting is safe.

**6. Apply.**

```bash
zt-digest term-sync -Step apply -Work "D:\zt-digest\_term-sync\<key>"
```

This enters the book into the list and writes one line in `## 释义` of each of this book's Term Cards that has an equivalent: `等义术语：[[address]]、[[address]]`. The other book's Card shows it as a Reverse link, so earlier books are never written. `no_answer` means a job has no answer file: run that job, because a missing answer is no decision.

Report what `apply` printed for each book.

## Rules

- A Term never changes. There is no rename and no replacement.
- Never run `apply` with an answer you have not read.
