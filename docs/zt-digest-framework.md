# ZT digests on the Workflow Framework Protocol

The ZT digests are built by the Workflow Framework Protocol (global skill `workflow-framework-protocol`): one **base** holds every stage the digests share, and each book type is a **variant** that installs its own modules into the base's slots. The assembler composes a variant with the base, builds one self-contained Workflow script, and installs it frozen: while it runs, its stages, their order, the roles and their models, the retries, checks, and modes cannot change; only data moves.

| Workflow | Blueprint | Installed script | Book type |
| --- | --- | --- | --- |
| `zt-econ-digest` | `scripts/book-digest/workflow/zt-econ-digest.blueprint.json` | `.claude/workflows/zt-econ-digest.js` | economics (today's `zt-book-digest`) |
| `zt-math-digest-v2` | `scripts/book-digest/workflow/zt-math-digest-v2.blueprint.json` | `.claude/workflows/zt-math-digest-v2.js` | mathematics (today's `zt-math-digest`) |

Both share `scripts/book-digest/workflow/zt-digest-base.blueprint.json`. The hand-written `zt-book-digest.js`, `zt-math-digest.js`, and `make-math-workflow.mjs` stay beside them until the switch is approved; the new workflows take the same args, build the same commands, and return the same result per mode.

## The base

```
args → source → prepare → [after-prepare] → terms → cards-book → explain → digest → models → [after-models]
     → hubs-first → link-prepare → link → link-check → [after-link] → cleanup → organize → figures → hubs → audit
     → [after-audit] → report
```

| Stage | Module | Runs | On failure |
| --- | --- | --- | --- |
| args | `gate` | always: citationKey (not in modes hubs, audit); source, bibtex, bookTitle (mode full) | halt |
| source | `zt-dsh-source` | not in modes figures, audit: the DeepSeek source probe | halt |
| prepare | `zt-digest-prepare` | mode full: prepare-book-digest.ps1; else args.groups | halt |
| terms | `zt-book-terms` | mode full, `terms` not false | continue |
| cards-book | `zt-cards-book` | mode cards | — |
| explain | `zt-explain` | mode explain | — |
| digest | `zt-digest-blocks` | mode full: digest blocks, each block's models and card cycle as a pipeline, the model sweep, the late clean-up | halt when no group is done |
| models | `zt-models-book` | mode models | halt |
| hubs-first | `zt-hubs` | mode hubs | — |
| link-prepare | `zt-link-prepare` | modes full, link | halt |
| link | `zt-link-pass` | modes full, link | — |
| link-check | `zt-link-check` | modes full, link; `cardChecks` not false | — |
| cleanup | `zt-dsh-cleanup` | modes full, link | — |
| organize | `zt-organize` | mode full, or `organize` true or "legacy" | continue |
| figures | `zt-figures` | mode figures, or full with `figures` not false | — |
| hubs | `zt-hubs` | full with `hubs` not false, or link with `hubs` true | — |
| audit | `zt-audit` | mode audit, or full with `audit` not false | — |
| report | `zt-digest-report` | always: the result in the shape of zt-book-digest.js | — |

Roles: `manager` (Haiku, effort low), `architect` (Sonnet), `organizer` (Opus). All shared machinery (the digest and link managers, `drive`, the model compiler, the card cycle) is the library module `zt-digest-lib`, ported from `zt-book-digest.js`; every manager takes a slot of the DeepSeek Subagent Managed Protocol pool (at most 8 managers in the run).

Modes: `link`, `models`, `cards`, `explain`, `hubs`, `figures`, `audit`, as before. Checkpoint: `until: "digest"` stops before the link pass; go on with mode `link` (`groups`, `organize: true`, `hubs: true`).

The book profile (`consts.profile`) is the base's one open const: `digestPrefix`, `linkPrefix`, `compilerDocs` (extra model-compiler instruction files, in order), `architectSupplement` (an extra architect file), `skipForms` (Model Card forms the model compiler leaves out).

## The economics set

`zt-econ-digest` fills no slot. Its module set is its profile: `shared-prefix.md` and `link-prefix.md` (written by prepare-book-digest.ps1) and the economic model architect `model-architect.md` alone.

## The mathematics set

`zt-math-digest-v2` sets the math profile (`math-prefix.md`, `math-link-prefix.md`, `-BriefDoc`/`-FillDoc`/`-ReviewDoc` under `<out>\math`, `model-architect-math.md`, Theory Cards out of the model compiler) and fills the slots:

| Slot | Stage | Module | Runs |
| --- | --- | --- | --- |
| after-prepare | math-skeleton | `zt-math-skeleton` | mode full: index, docs, skeleton (halt on a failure), notation table (unless `notation` is false) |
| after-models | math-boundary | `zt-math-boundary` | modes full, math; `boundary` not false |
| after-models | math-proof | `zt-math-proof` | modes full, math; `proof` not false |
| after-link | math-theory | `zt-math-theory` | modes full, math; `theory` not false |
| after-audit | math-check | `zt-math-check` | mode math, or `audit` not false with a citationKey |

Mode `math`: args, source, prepare, the four math stages, report. The result adds `math: {index, docs, skeleton, notation, boundary, proof, theory, check}`.

## Changing a digest

1. Change the module (`scripts/book-digest/workflow/modules/*.js`, raise its version when its behaviour changes) or the blueprint (raise the blueprint's version; update the `preflight` lines it touches).
2. Run the fixture: `node scripts/book-digest/tests/framework-harness.mjs`. It builds both variants from their blueprints and runs every check of `workflow-harness.mjs` and `math-workflow-harness.mjs` on them, plus: both variants hold every base stage with the same module and params; the math set is installed only in the math workflow; an edited script, an undeclared arg, and an undeclared mode are refused; the checkpoint stops before the link pass; the installed scripts are the current builds.
3. Install (project scope) and verify:

```powershell
$A = "$HOME\.claude\skills\workflow-framework-protocol\tools\assemble.mjs"
node $A install scripts\book-digest\workflow\zt-econ-digest.blueprint.json --sim scripts\book-digest\workflow\zt-econ-digest.sim.json --scope project --project E:\zet
node $A verify zt-econ-digest --scope project --project E:\zet
```

4. Run by path: `Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-econ-digest.js", args: {...} })`, a pilot first, after a database backup.

A new book type is a new variant: its profile and its own modules in the slots (or a new slot in the base, which every variant then gets empty).
