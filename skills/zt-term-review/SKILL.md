---
name: zt-term-review
description: "Review the remaining variant words of one aligned ZET Literature book: after zt-term-align has renamed a book's Terms and replaced what is safe, DeepSeek agents judge every other use of a shared term's variants, and a script replaces the ones they choose. Agents answer with ids and never change a sentence. Use after zt-term-align, or when the user asks to replace remaining term variants or clean up old spellings in a book. It reads only the standard list and the live Cards, so it can run later or again. Give it a citation key."
---

# Review the remaining variant words of a book

Chinese has no word boundary, so a short word or a variant needs a reader: 瓦拉 may stand inside 瓦拉什一世, and 安息 may be the Sabbath and not Parthia. This skill is that reader. You conduct it; DeepSeek agents judge; the script edits. Design: `docs/term-align-design.md`.

The scope is the variants of entries this book shares with a second book. A book's other 不用 words stay R6's advice.

## Steps

**1. Back up the box** if this session has not done so.

**2. Start.** Use a fresh folder.

```bash
zt-digest term-review -Step start -Book <key> -Work "D:\zt-digest\_term-review\<key>"
```

`held` counts the occurrences, at most 150 a job. With `"jobs":0` the book is clean.

**3. Run the jobs.** `drive\jobs-001.json`, `jobs-002.json` and so on hold up to 8 jobs each. Start one `dsmp-manager` agent per file, at most 8 at once:

> Read the JSON array in FILE and run EVERY entry: deepseek_run with cwd WORK, prompt_files = the entry's files, prompt = the entry's prompt, model = the entry's model, reasoning_effort "high", wait_seconds 0. Poll with deepseek_wait (wait_seconds 90). Answer "needs_response" approvals: allow only a command that runs `zt-digest term-review -Step check` and a write of a JSON file under WORK\out. A session that ended in an error gets one more attempt at once, before you wait on any other job. Delete each conversation when it ends, and never archive one. Return the ids of the jobs that failed.

**4. Spot-check.** Read a sample of the answers against `held.json`: some kept and some replaced. Remove any id you reject from the answer file.

**5. Finish.**

```bash
zt-digest term-review -Step finish -Work "D:\zt-digest\_term-review\<key>" -Relink "E:\zet\scripts\book-digest\term-relink.ps1"
```

It replaces each chosen occurrence at its place and relinks the book. `stale` counts occurrences left alone because the text changed since the start. `open` above 0 means a job gave no answer: run those again, then finish again.

Report held, replaced, stale and links added.
