# The fixed reader task sheet (S28, #121)

This is the fixed task sheet [US042](https://github.com/liudesheng9/zet/issues/93) asks for: the same
four short reader tasks, run by the human reader on the installed service, so that "the new flows
are checked on real tasks" is a repeatable observation instead of a claim. Every run uses this
sheet and records the same fields. It has no telemetry: the reader writes the numbers by hand, and
`zt-digest audit -TaskSheet` reads them as P12/P13/P15 (design §14.3).

**Scope of one run.** Record it before the first task and do not change it during the run:

| Field | Value for the first run |
| --- | --- |
| Archive | the live archive (`D:\zt`, archive identity from `zt status` / `/api/overview`) |
| Revision | the `archive_revision` the service reports when the run starts (`0` at this writing) |
| Service | the installed `zt` the running daemon reports (`zt version`, `zt status`) |
| Workspaces | `agent` (the register's own terms live there) and `human` |
| Installed Workflow versions | `.claude/workflow-framework/*/lock.json` (they pin what produced the Cards) |
| Interval | the UTC start and end of the run |

A later sheet is comparable only when it names the same four tasks and a recorded scope. A sheet
whose useful Card no longer exists, or whose "removed" Card still exists, is reported as an invalid
observation by the audit — never as a zero.

## The four tasks

Do them in order, on the installed reader at `http://127.0.0.1:4717/`, without a plan to make the
numbers look good. Record seconds with a watch or the clock. A Card is "useful" when it answers the
question well enough that you would keep it; a Spread is "kept" when you press Keep and it stays.

| Task | Kind | Question | Do | Record |
| --- | --- | --- | --- | --- |
| **T1** | `first-useful-card` | `消费者偏好如何决定需求函数？` | Ask the box (`Ask` or the `ask` command). Read the entry rows first. Open the first Card that answers the question. | `seconds` to the open Card, `useful_address`, `entry_rank` = its 1-based position in the Ask result / search tray, counting a folded `+N` row as one |
| **T2** | `first-useful-card` | `限制资本流动` | Use plain search (the search tray, not Ask). Narrow with the type filter to `model` if the list is long. Open the first useful Card. | `seconds`, `useful_address`, `entry_rank` = its 1-based position in the plain-search list |
| **T3** | `kept-spread` | `多重均衡如何让政策效果不确定？` | Ask the box, read the proposal, press Keep. Do not edit the Table afterwards. | `seconds` to Keep, `kept_spread` = the name the picker shows (the question's first 80 characters) |
| **T4** | `distant-card` | (the distant slot of `270/3`) | Open Card `270/3`, read the one distant suggestion beside it, then lay it on the Table (or leave it). | `offered` (0 or 1), `opened` (0 or 1), `laid` (0 or 1) — each must not exceed the one before |

If a task cannot be done as written (the useful Card is gone, the term is no longer accepted), say so
in the run note and leave that task's fields out; the audit then reports that measure as unknown
with the reason. Do not invent a number.

## What the sheet file looks like

The observations go into one JSON file,
`D:\zt-target\qualification\issue121\reader-task-sheet.json` (the audit refuses any other shape).
Copy this skeleton and fill every field; an empty interval, `useful_address` or `kept_spread` is an
unfinished sheet, not a measured zero:

```json
{
  "schema_version": 1,
  "kind": "zt-task-sheet",
  "interval": { "from": "2026-10-20T02:30:00Z", "to": "2026-10-20T02:50:00Z" },
  "tasks": [
    { "id": "T1", "kind": "first-useful-card", "question": "消费者偏好如何决定需求函数？", "seconds": 0, "useful_address": "", "entry_rank": 1 },
    { "id": "T2", "kind": "first-useful-card", "question": "限制资本流动", "seconds": 0, "useful_address": "", "entry_rank": 1 },
    { "id": "T3", "kind": "kept-spread", "question": "多重均衡如何让政策效果不确定？", "seconds": 0, "kept_spread": "" },
    { "id": "T4", "kind": "distant-card", "question": "270/3", "offered": 0, "opened": 0, "laid": 0 }
  ],
  "removed_cards": [],
  "human_replies": 0,
  "annotation": []
}
```

Then turn the sheet into the measures (read-only; about six minutes on the whole archive):

```powershell
zt-digest audit -Out D:\zt-target\qualification\issue121\audit `
  -TaskSheet D:\zt-target\qualification\issue121\reader-task-sheet.json -SkipModels
```

`audit.json` reports `baseline.measures.P12_P13`, `P15` and `P14` with the sheet's interval and
validity. See [`docs/luhmann-reader-release.md`](luhmann-reader-release.md) §7 for the handoff and
what stays unresolved until then.

`removed_cards` lists addresses of Cards you deleted during the run (none is expected). `human_replies`
counts the Reply Cards you saved on the run. `annotation` stays empty unless you hand-annotate the
sample the audit reports.

## The checks beside the four tasks (AC02, AC04, AC07)

These are observations, not sheet fields. Record each one as PASS/FAIL and the phrase the reader saw:

1. **Entry and plain search**: ask/enter the T1 and T2 questions; the entry rows come first, then a hub
   member line, then body hits; the type filter narrows the list and the counts follow.
2. **Ask**: the T1 and T3 questions show a proposal with a reason per Card; a blank question shows its
   error; Cancel leaves the Table as it was; Keep adds one Spread and changes no Card; a
   non-matching question (`汇率制度与资本流动的关系是什么？`) shows the low-confidence fallback and
   `truncated` bounds instead of pretending it answered.
3. **+N recovery**: if no folded `+N` row appears (the live register has no accepted restatement at
   this revision), record "not present in this archive"; the scratch suites prove the folding.
4. **Filters and sparse results**: `量子引力如何统一四种基本力？` is sparse (no Card); record what the
   box showed.
5. **Agent hub draft placement**: file the T3 Spread as a card (Save), then read the new agent Hub
   Card: it is filed under the register at level `field`, and its Links carry the Spread's reasons.
   Cancel writes nothing if you choose Cancel instead.
6. **Reply**: with an agent Card open, press `r`, write your own reply, Save. One human Card appears
   in a human Topic, the agent Card gains the Reverse link, and the agent Card's body is unchanged.
   Cancel writes nothing. Agents still cannot write human Cards (the MCP server refuses them).
7. **Distant**: opening the T4 Card offers exactly one candidate; opening or laying it writes no Link
   and no Bridge.
8. **Switches**: `zt config set gui_search off` turns the search tray off while Cards, trees, the map,
   the Table and the saved Spreads stay readable; the same for `gui_ask` and `gui_reply`; `on`
   restores each one without a restart. (`docs/luhmann-reader-release.md` §4.)
9. **Keyboard and attention**: `t`, `/`, space, `f`, `g` do not fire inside the search input, the
   editor, the command bar or a modal; the held Card keeps the focus when the distant slot opens;
   light and dark themes stay legible.

## What is not measured here

Time to first useful Card and a kept Spread is one reader's observation, not a benchmark; the
audit keeps it beside its scope and never turns it into a target. No questionnaire, no session
recording, no telemetry store.
