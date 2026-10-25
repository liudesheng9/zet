# Term Cards and Term Links: the S30 copied-book pilot (issue #172)

This is the operator runbook for **S30 / #172**, the owner-led pilot of the Term work on a copied
book. It states what the agent part delivers, the one prerequisite the 2026-10 environment does not
satisfy, the exact run procedure, the owner's review, and which claims each piece of evidence
supports. It does not authorize a live-archive write, a historical backfill, an enforcement change
or a release promotion; those are later issues.

Source binding: the [implementation design](term-cards-links-and-check-gate-implementation-design.md),
§18.6 (SHA-256 `a27224263597460d0822d7859cd1a822d47e7376bb207686689843fc6d3309d4`), whose seven
pilot steps this runbook turns into one planned, preflighted, qualified and recorded run.

## 1. Status

**Delivered and verified.** `zt-digest term-pilot` (`tools/zt-digest/src/term_pilot.rs`) plans,
preflights, qualifies and records one copied-book Term pilot. It is read-only over Cards: it writes
only under `-Work`, and it never writes a Card, a Term link, a Lock or an archive row. Its tests
(`tests/term_pilot.rs` and the module's unit tests) run the real `term-cards` and `term-link`
commands on a scratch box and a real prepared run, then drive all four steps. The delivered pieces
of S30 are:

- the consistent-copy evidence: the box's archive identity, the retained `copy-tree.mjs` snapshot
  (`live-tree.json`), its zero-difference `report.json`, the copied tree's authored fingerprint and
  the live archive's identity/revision in the owner's authorization;
- the retained run/source/list fingerprints: the prepared run's contract and manifest, the source
  path/SHA-256, the one Term list's authored title/body and row terms, and the book tree's
  before-state fingerprint;
- the row accounting, preservation equation, zero-write second pass, language/Link diagnostics and
  actual provider usage/runtime, all read from the run's own artifacts;
- the owner's review sheet: three mathematical terms, two concept terms and ten GUI Cards, with the
  GUI links, the source meaning and the decision fields;
- the decision record: an approval is refused until the full sample is reviewed; a rejection names
  its unresolved work; the gate applies nothing.

**Not claimed here.** No real pilot book has been run by this delivery. No provider call happened,
no live-archive Card was written, no Lock was changed, and no owner judgment is recorded. The
review's mathematical depth, layout judgment and final accept/reject remain the S30 human gate. The
live installed-service qualification remains S31 (#173), and the historical backfill remains S32
(#174).

**Deferred.** On 2026-10-25 the owner deferred the pilot run: no compatible prepared run exists
(§2), and only the owner starts the two workflows that produce and use one (§4.2 and §4.5). The
gate, its tests and this runbook are delivered; the commands below are the owner's to run when the
pilot is scheduled. Until then, S30-AC01..AC07 are not verified by a real pilot.

## 2. The book and the one prerequisite

The design names `grossman2002integration` "if still suitable at implementation time". The book is
still in the live archive (142 Cards under its Literature tree), but its **prepared run is not
usable with the installed Term-capable workflow**:

- its retained staging `D:\zt-digest\grossman2002integration-test` froze contract base `4.10.0` /
  variant `5.10.0`;
- the installed frozen variant is `zt-econ-digest` **6.6.0** on base **5.6.0**;
- `Prepared::read` refuses a run prepared under another major.minor contract
  (`incompatible workflow contract: base/variant versions differ`), and the gate refuses such a
  staging before any work.

No other retained staging is on the Term-capable line either (the 2026-10-24 stagings are
`6.1.x`/`6.4.x`). A pilot therefore needs a **prepared run on the current contract, with the Term
layers disabled**, so that the pilot's `term-cards` mode is the one run that adds the Terms. The
supported way to produce it is a normal full digest of the book on the copied box at the installed
variant with `termCards: false, termLink: false` (the workflow's disabled-layer path, already tested
by S25/#168). The copied book tree of the same book is retained as the copy evidence and as an
earlier carrier for the Bridge. The pilot tree then uses a distinct `citationKey` so the copied tree
is never edited.

**Run the project-scope frozen variant.** The repository's project registry
(`.claude/workflow-framework/zt-econ-digest`) is **6.6.0** on base **5.6.0**, and the installed
`zt-digest` 0.29.0 embeds the same blueprints. The **global** installed registry at
`~/.claude/workflow-framework/zt-econ-digest` is 6.5.6 on base 5.5.6 (installed by S29/#171), and a
minor release is a new run contract: a run prepared by the global script would be refused by the
installed `zt-digest` (`incompatible workflow contract: base/variant versions differ`). Run the
workflow from `E:\zet` so it loads the project-scope 6.6.0 script, and check the plan's
`workflow.version`; the plan also records `workflow.global` and `global_compatible` for the release
record. Re-installing the global set as one compatible set is **S31 (#173)'s** live-release action,
not part of this pilot.

If the owner instead provides a compatible prepared run (same major.minor contract as the installed
variant), step 4's full-digest run may be skipped; every other step is unchanged.

## 3. Evidence scopes

| Evidence | Proves | Does not prove |
|---|---|---|
| `tests/term_pilot.rs` on a scratch box | the gate's checks over the artifact shapes the real commands write | the pilot book's content, the owner's judgment or a live provider run |
| `copy-tree.mjs`'s retained report | the copied tree is byte-for-byte the live tree it names | that the live archive was untouched after the copy (the runbook forbids touching it) |
| the run's `term-cards/plan.json` and `report.json` | every row's source binding, category, draft and publication status | mathematical truth or explanatory depth |
| the run's `term-link/plan.json` and `apply.json` | the exact byte substitutions, the preserved bodies and the committed chunks | that a second run would write nothing (the retained second plan proves that) |
| the run report and `token-usage.mjs` | the provider's own measured usage, the run's timing and the Claude agent/token counts | a complete bill; a missing run stays missing coverage |
| the owner's filled review sheet | explanatory depth and GUI usability, item by item | any other term or Card, or a future book |

A stand-in workflow runtime proves orchestration behavior only. It does not prove live provider
cleanup, mathematical quality or installed-binary qualification (design §18.6, R0563).

## 4. Procedure

All commands run on the Windows host from the repository root. `$Box` is the copied box's folder;
`$Work` is the pilot's evidence folder; `$Out` is the prepared run's staging folder. Never point any
of these at `D:\zt`.

### 4.1 Create the copied box

```powershell
$Box  = 'D:\zt-pilot-172'
$Work = "$Box\work"
$Out  = "$Box\staging"
New-Item -ItemType Directory -Force "$Box\config", "$Box\archive", $Work, $Out | Out-Null
$env:ZT_CONFIG_DIR = "$Box\config"
$env:ZT_GUI_PORT   = '48717'
zt config set archive_root "$Box\archive"
zt up
zt status                     # state: up, archive_root: D:\zt-pilot-172\archive
```

Copy the book's existing tree through the live box's read-only tools (`copy-tree.mjs` reads the
live box with read tools only; it never opens or copies the live SQLite):

```powershell
node scripts\book-digest\copy-tree.mjs --tree grossman2002integration `
  --to-config "$Box\config" --out "$Work\copy"
```

The command writes `$Work\copy\live-tree.json` (the live snapshot) and
`$Work\copy\report.json` (the comparison). Both are retained as the copy evidence. The copied box
tree is unlocked even though the live tree is Locked (the Lock stays in the live box).

Write the box pin into the staging folder so every `zt-digest`, `zt-check` and `zt` child of the run
serves the copy:

```powershell
New-Item -ItemType Directory -Force $Out | Out-Null
Set-Content -Path "$Out\zt-box.txt" -Value "$Box\config" -NoNewline
```

### 4.2 Produce the compatible prepared run (skip if one exists)

The owner starts this run in a Claude Code session opened from `E:\zet` (so the project-scope
`zt-econ-digest` 6.6.0 loads). Run the installed `zt-econ-digest` workflow in mode `full` with the
Term layers disabled, on the copied box and a new pilot Citation key:

```js
{
  out: "D:\\zt-pilot-172\\staging",           // the $Out folder from §4.1
  book: "grossman2002integration",          // the Scholar Archive Work id or exact title
  citationKey: "grossman2002integration-pilot",
  termCards: false,
  termLink: false,
  audit: true,
  finalize: true
}
```

This is a real digest of the book and the owner's decision to spend it. It must complete with the
run report showing its normal digest stages, and its staging must keep `manifest.json`,
`ledger.sqlite3`, `sections/` and `groups/`. **It adds no Term Cards**: `termCards: false` and
`termLink: false` exclude those stages, which is what makes the next run the pilot's one Term run.
Do not run a second digest on this staging.

### 4.3 Write the authorization

`$Work\authorization.json` names what the owner authorized. Every field is checked by `plan`:

```json
{
  "schema_version": 1,
  "pilot": "S30",
  "approved": true,
  "note": "S30 copied-book Term pilot: grossman2002integration on the copied box, term-cards mode, owner review of 3 math + 2 concept terms and 10 GUI Cards",
  "book": "grossman2002integration-pilot",
  "mode": "backfill",
  "box": {"config_dir": "D:\\zt-pilot-172\\config", "archive_root": "D:\\zt-pilot-172\\archive"},
  "live": {"archive_id": "<the live archive id, 32 hex>", "archive_root": "D:\\zt", "revision": <the revision copy-tree read>},
  "copy": {"folder": "D:\\zt-pilot-172\\work\\copy", "tree": "grossman2002integration"},
  "review": {"math_terms": 3, "concept_terms": 2, "gui_cards": 10}
}
```

The live archive id is the `archive_id` the Scholar Archive or `zt-digest term-link` records for the
live box; the revision is the `archive_revision` in `$Work\copy\live-tree.json`. `plan` refuses a
record whose box is the live archive, whose copy is not the retained snapshot, whose tree is not the
copied tree, or whose review asks for less than the S30 minimum.

### 4.4 Plan and preflight

```powershell
$env:ZT_CONFIG_DIR = "$Box\config"
zt-digest term-pilot -Step plan -Staging $Out -Work $Work `
  -File "$Work\authorization.json" -Copy "$Work\copy"
zt-digest term-pilot -Step preflight -Staging $Out -Work $Work
```

`plan` writes `term-pilot-plan.json`, `term-pilot-before.json` and the before diagnostics report.
`preflight` exits 0 only when the plan, copy, box, prepared run, source, list, binaries, frozen
workflow and `<Staging>\zt-box.txt` still verify and no Term Card is published and no
`term-cards\report.json` exists. A refused preflight writes `term-pilot-preflight.json` with
`ok: false` and names every problem; fix the named fact and plan again.

### 4.5 Run the pilot's one Term run

The owner starts this run in the same Claude Code session from `E:\zet`. Run the installed
`zt-econ-digest` workflow in mode `term-cards` on the same staging:

```js
{
  out: "D:\\zt-pilot-172\\staging",           // the same $Out folder
  termCards: true,
  termLink: true,
  termCardsPerJob: 4,
  termCardsMathPerJob: 1,
  termLinkOccurrences: "first"
}
```

The mode runs the backfill preparation, the Term-card branch (writers, checks, independent
mathematical reviews, one finite fix, publication) and the deterministic Term-link pass, then the
read-only audit, cleanup and run report. It starts no digest, Card-cycle, model, Link, Organize,
Figure or Bridge stage. The run's own artifacts are the evidence:

- `$Out\term-cards\plan.json`, `report.json` and `report.md`;
- `$Out\term-link\plan.json`, `preview.md`, `apply.json` and `apply.md`;
- `$Out\run-report.json` and `run-report.md`;
- `$Out\ledger.sqlite3` (check the `termcards/*` and `termlink/*` units and the cleanup records).

Retain the first link plan before the second pass (the next `plan` overwrites it):

```powershell
Copy-Item "$Out\term-link\plan.json" "$Out\term-link\first-plan.json"
```

### 4.6 The zero-write second pass

```powershell
zt-digest term-link -Step plan -Staging $Out      # must report cards_changed: 0
Copy-Item "$Out\term-link\plan.json" "$Out\term-link\second-plan.json"
Copy-Item "$Out\term-link\first-plan.json" "$Out\term-link\plan.json"
```

A non-zero `cards_changed` is a named unresolved fault for the qualification; do not apply it.

### 4.7 Measure the Claude usage and qualify

```powershell
node scripts\book-digest\token-usage.mjs "<session>\subagents\workflows" <workflow run id> |
  Set-Content "$Work\claude-usage.json" -Encoding utf8
zt-digest term-pilot -Step qualify -Staging $Out -Work $Work `
  -ClaudeUsage "$Work\claude-usage.json"
```

The qualification reads the run's own `run-report.json` for DeepSeek usage, timing and cleanup, and
the optional `token-usage.mjs` record for the Claude agents and weighted tokens. It writes
`term-pilot-qualification.json`, `term-pilot-review.json`, `term-pilot-review.md` and the after
diagnostics report. Its `verdict` is `complete` only when every row is a verified head/parts, the
list is unchanged, every changed body strips back to its planned text, the second pass writes
nothing, the diagnostics are unchanged and the run report was retained. A missing artifact is named
in `problems`, never treated as zero. `decision` is always `pending-human`.

### 4.8 The owner's review

Read `$Work\term-pilot-review.md` and open every URL in the copied box's GUI
(`http://127.0.0.1:48717/`). The sheet asks for:

- three mathematical terms: the explanation, the symbols, the derivation, the limits and the
  distinction from the neighbouring terms;
- two concept terms: the explanation, the distinctions and the examples;
- ten linked Cards: label-only layout, first-occurrence linking, the ordinary-Link distinction and
  keyboard readability (Tab order, focus ring, hover/focus labels, broken and cross-Workspace Links).

Fill `term-pilot-review.json`: each item gets `"decision": "accept"` or `"correct"`; a correction
gets its exact `note`. Set `"decision": "approve"` only if every item is accepted or corrected and
the sheet's `complete` is true; otherwise set `"decision": "reject"` and list the concrete
`unresolved` work. Do not edit the item lists, the URLs or the `review_id`.

### 4.9 Record the decision

```powershell
zt-digest term-pilot -Step record -Staging $Out -Work $Work `
  -Decision "$Work\term-pilot-review.json"
```

`record` writes `term-pilot-decision.json` with the decision, the decided item count, the named
corrections and the unresolved work, and `applies: []`. An incomplete approval is refused. The gate
applies no correction: a correction is S32's or another owner-selected change, never this pilot.

## 5. Acceptance criteria map

| Criterion | Evidence in this run |
|---|---|
| **S30-AC01** consistent verified copy, grossman if suitable, live archive preserved, copied run/source/list fingerprints retained | §4.1's `live-tree.json` and `report.json`; `term-pilot-plan.json`'s `live`, `copy`, `box`, `staging.source` and `box.list`; the gate never opens the live archive; §2 states why the grossman staging itself is not suitable and how the compatible run is produced |
| **S30-AC02** every row as verified head/parts or a named unresolved fault; list title/body exact | `term-pilot-qualification.json`'s `rows` (each verified row read back by type, Term name and role; each fault with its status and reason) and `list` (authored/current title and body SHA-256) |
| **S30-AC03** owner reads three math and two concept terms against source/notation and records acceptance or corrections | `term-pilot-review.json`'s `math` and `concept` items with their source meaning, defined location, head Card and GUI link; the frozen decisions in `term-pilot-decision.json` |
| **S30-AC04** owner reads ten GUI Cards and judges label-only layout, first-occurrence policy, ordinary-Link distinction and keyboard readability | `term-pilot-review.json`'s `cards` (the Cards the link plan actually edited, with their Term uses and GUI links); the review sheet's four checks; the owner's decisions |
| **S30-AC05** preservation for every changed body, zero writes on the second run, no new language/Link fault from Term markup alone | `qualification.preservation` (changed, stripped-equal, violations), `qualification.second` (the retained second plan's zero edits and the unchanged revision), `qualification.diagnostics` (the before/after reports, `T*` counted apart) |
| **S30-AC06** actual provider usage/runtime, cleanup and remaining failures, no unverified pricing | `qualification.run` (the run report's usage/timing/cleanup, with missing runs kept missing) and `qualification.claude` (the token-usage record); `rows.faults`, `problems` and the run's `completeness` name the remaining failures |
| **S30-AC07** explicit approval/rejection and unresolved work; no blanket live backfill or automatic enforcement | `term-pilot-decision.json` with `decision`, `corrections`, `unresolved` and `applies: []`; the runbook's evidence scopes state that a copied pilot authorizes nothing live |

## 6. Limits and what comes next

- The pilot's provider usage is the run's own measured records; a provider run the record lacks is
  missing coverage, never zero and never a priced estimate.
- The diagnostics comparison re-runs the installed checker on the planned addresses. It cannot
  check text that no longer exists in the archive; the preservation equation proves that the only
  change to those bodies is the Term macros, and the checker's own T-rule behaviour on a Term Card
  is covered by S29's tests.
- The owner's review is a judgment about the three + two terms and ten Cards read. It does not
  generalize to every Term or Card, and it is not a release qualification.
- A rejected or corrected pilot leaves the copied box and its staging as evidence. The live archive
  is not touched by any step, and a correction becomes an explicitly selected later change, not an
  automatic rewrite.
- S31 (#173) qualifies the live release and the deliberate gate-policy promotion; S32 (#174)
  applies one owner-selected historical Term backfill. Neither follows from this pilot's approval.
