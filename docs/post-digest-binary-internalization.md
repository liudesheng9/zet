# Post-Digest Helper Internalization (Waves 1 & 2)

Owner: Xingtong Chen
Date: 2026-11-27
Status: Waves 1 & 2 implemented, gap-closed (P0 + P1), bumped & installed (`zt-check 0.23.0`, `zt-digest 0.65.0`); Wave 3 planned

## Motivation

`skills/zt-post-digest/SKILL.md` orchestrates seven stage skills (`zt-model-fold`, `zt-model-settings`, `zt-model-restructure`, `zt-model-import`, `zt-evidence-dialogue`, `zt-literature-organize`, `zt-model-illustrate`) plus `scripts/book-digest/term-relink.ps1`. Before internalization, those stages relied on 21 Node.js/PowerShell scripts and 7 copy-pasted copies of `zt.mjs`:

- Every helper spawned `zt-digest call` → `zt mcp` subprocesses repeatedly per batch.
- Several helpers manually ported Rust logic into JavaScript (`check-premises.mjs` ported `zt_check::formal::is_local_route_clause` and `proof_route_addresses`; `check-settings.mjs`, `check-drafts.mjs`, and `intuition.mjs` duplicated `zt-check` rules and TeX/link parsers).
- `check-network.mjs` fetched `http://127.0.0.1:4717/layout.js` over HTTP at runtime to call three pure layout functions.
- `explain.mjs` and `term-relink.ps1` wrapped `zt-digest model` and `zt-digest term-link` through temporary `manifest.json` shims.

## Architectural Boundary

- **`zt-check` (`cli` feature, `tools/zt-check/src/post_digest/`)**: owns all **strictly read-only** post-digest audits and gates. Keeping them under `#[cfg(feature = "cli")]` guarantees zero change to the process-free core (`default-features = false`) linked into `zt.exe` — `RULE_VERSION`, `DIAGNOSTIC_SCHEMA`, and the live write gate are unaffected, while `zt-digest` can also call them in-process.
- **`zt-digest` (`tools/zt-digest/src/`)**: owns stateful/mutating post-digest runners (`backup`, `term-link -Step relink`, `model-explain`), benefiting from one persistent `zt mcp` session, `lockcheck`, `boxpin`, Literature write scope, and bundled `rusqlite`.
- **Kept in Node (`zt-model-illustrate`)**: the parametric SVG authoring DSL (`draw.mjs` + per-model `modules/*.mjs`) and headless Edge/Chrome PNG screenshotting (`verify.mjs`). All deterministic chart/intuition lint and coverage checks are internalized into `zt-check`.

Existing `.mjs` scripts and `term-relink.ps1` remain intact and working alongside the internalized commands during rollout.

---

## Wave 1 — Implemented

### `zt-check` (`tools/zt-check/src/post_digest/`)

1. **Stage-scoped model build gate** (`stage.rs`, replaces `check-model.mjs`):
   ```bash
   zt-check --model <head> --stage fold|settings|restructure|final [--format text|json]
   ```
   - Classifies all 20 `get_model` build fault keys into `[blocking]` vs `[deferred -> Stage N]` per stage.
   - Handles pending cross-model `notation` import deferral (Stage 2) and pending cross-model `criterion` import deferral (Stage 3).
   - Enforces strict `self_sufficient === true`, `formal_structured === true` (eligible L2 formal models without `form`), `evidence_bound === true` (L3 empirical models), and zero `build.parts` faults at `--stage final`.

2. **Hidden equation-number premise scan** (`premises.rs`, replaces `check-premises.mjs`):
   ```bash
   zt-check --model <head> --premises [--format text|json]
   ```
   - Reuses `zt_check::formal::is_local_route_clause` directly (no duplicated JS port).
   - Strips pointer-only sections (`为什么需要它`, `解读`, `承上启下`, plus `说明` on Criterion cards) without discarding inline later headings.
   - Exempts exact bound proof routes (`**依据**` local clauses, or Benchmark direct Side proofs without `**依据**`, plus contiguous Direct continuations, matching `src/model/formal.rs`) and ignores legacy shared parts (`line == "shared"` without `import`).

### `zt-digest` (`tools/zt-digest/src/`)

3. **Verified SQLite backup** (`backup.rs`, replaces the `node:sqlite` `VACUUM INTO` one-liner):
   ```bash
   zt-digest backup -Out <path> [-Source <db>] [-Force]
   ```
   - Resolves `<archive_identity.path>/zt.sqlite3` from the live service when `-Source` is omitted, rejects canonicalized same-file source/output paths, executes `VACUUM INTO ?1` from a read-only connection into a unique sibling temp file, verifies `PRAGMA integrity_check` returns strictly `"ok"`, and only then atomically swaps into `-Out` (refusing overwrite without `-Force` and rolling back on replacement error). Prints compact JSON `{ok, source, out, integrity, bytes}`.

4. **Convergent live Term relink** (`term_link.rs`, replaces `scripts/book-digest/term-relink.ps1`):
   ```bash
   zt-digest term-link -Step relink -CitationKey <key> [-Occurrences first|all] [-Cards <a,b>] [-PlanOnly]
   ```
   - Runs `plan` + `apply` in one process over one `Zt` client up to 3 rounds until `cards_changed == 0`, fails if 3 rounds still leave changes, propagates typed `card_locked` (exit 2) / stop (exit 1) immediately, and prints the compact JSON summary.

5. **Single-model Derivation explanation refresh** (`model_explain.rs`, replaces `skills/zt-model-import/scripts/explain.mjs`):
   ```bash
   zt-digest model-explain -Step plan|apply|finish -Model <head> [-Work <dir>] [-Force]
   ```
   - Validates `-Model` is a single Model Card head (`get_model(at).head.address == at`) and passes Literature `lockcheck::preflight` before touching `-Work`; enforces strict `.zt-model-explain-work` `{head, archive: {id, path}}` ownership binding, non-empty foreign-directory refusal, unfinished-state refusal, required `state.json` on `apply`/`finish`, and atomic `finished_ok` invalidation/write.
   - Runs `model -Step explain` / `explain-apply` in-process via `model::compiler::run_silent_with_zt` and runs the subset `term-link` pass in-process via `term_link::plan_with` / `apply_with` over one `Zt` client, preserving typed `card_locked`/stop refusals and failing if any Term card is refused (`cards_refused > 0` or `complete != true`).

---

## Wave 2 — Implemented (`zt-check`, `tools/zt-check/src/post_digest/`)

6. **Settings & empirical data-tree / specification-ladder audit** (`settings.rs`, replaces `check-settings.mjs`):
   ```bash
   zt-check --model <head> --settings [--symbols] [--format text|json]
   ```
   - Formal models: placement (`<head>|a`, `<head>|a|a`), outside/inside roles, chapter-stated assumptions, local definitions (`记 $X$`), uncovered/unused/first-use notation rows (including 1-hop `沿用 [[...]]` validation and formal-v2 `symbol_uncovered`), dependency tree (`**起点**`, `misfiled`, `order`), card format (`**作用域**`, `**直觉**`, `**说明**`), statement repetition (Dice ≥ 0.8), bare assumption citations (`假设 N` / `假设 1.2` / `假设 (B)` / `假设一`), and balance advice.
   - Empirical models: data card / `变量表` placement, uncovered/unused variables (with prose Greek support), specification ladder (`variant_change`, `spec_in_chapter`, `**估计式**` equation requirement), identifying assumptions under `design`, data-tree `**起点**` and order, and Stage 3 deferral (`estimate_unbound`, `test_unbound`, missing-`估计` shape).

7. **Model network audit** (`network.rs`, replaces `check-network.mjs`):
   ```bash
   zt-check --model <head> --network [--format text|json]
   ```
   - Pure Rust port of `blockTree`, `blockLayout`, and `treeLayout` from `src/gui/layout.js` (no HTTP fetch on port 4717), verified by an automated parity test against `src/gui/layout.js`.
   - Reports forward threads (distinguishing maintained setting primitives vs movable derived cards), unanchored chapters (exempting empirical `数据`), short/missing chapter leads, empty-result advice (formal-v2 aware: `结果`/`基准`/`校准`/`数值实验`), `build.parts`, and formal Layer 1 faults.

8. **Draft-vs-original preservation check** (`draft_diff.rs`, replaces `check-drafts.mjs`):
   ```bash
   zt-check --draft-diff <orig.json> <draft.json> [--chapters a,b,...] [--format text|json]
   ```
   - Checks display-formula multiplicity (per card for non-chapters; batch-wide for chapters), equation-tag retention, chapter-order premises, live in-tree & external link target existence, `本模型`/`该模型`, title length limits, and required role headings (`推导`, `论证`, `结果`, `校准`, `数值实验`, `判据`/`均衡定义`/`解概念`).

9. **Literature tree structural audit** (`tree.rs`, replaces `zt-literature-organize/scripts/check.mjs`):
   ```bash
   zt-check --tree <key> [--snapshot <file>] [--format text|json]
   ```
   - Prints top-line branch sizes and checks list placement (Term/Cited lists on top line and at the tail), broken links in `<key>/`, and optional snapshot card-count & whole-unit multiset size changes.

10. **Chart & intuition deterministic lint and coverage** (`charts.rs`, replaces `lint.mjs`, `intuition.mjs` lint/span, and non-render checks of `verify.mjs`):
    ```bash
    zt-check --model <head> --charts [--format text|json]
    zt-check --charts-json <file> [--format text|json]
    ```
    - Checks all 9 chart lint rules (`svg-math`, `svg-tex`, `span-css`, `html-math`, `assumption-name`, `axis-label`, `width`, `dup-id`, `parts`), live `data-zt` dead-target verification, missing figures on Model/setting/assumption cards, and 3-line `**直觉**` presence + lint (`intuition-missing`, `semicolon`, `light-verb`, `depends`, `link`, `display`, `assumption`, `long`).

---

## Astra Audit & Hardening (`gpt-6.2-astra`, effort `high`)

After initial TDD implementation, `gpt-6.2-astra` audited all Wave 1–2 Rust modules against the live ZT core (`src/model/formal.rs`, `src/term.rs`, `src/read.rs`, `tools/zt-check/src/empirical.rs`) and reported 1 Critical, 7 High, 13 Medium, and 3 Low findings. All 24 findings were verified against the codebase and fixed with regression tests:

- **Backup safety (`backup.rs`)**: rejected canonicalized same-file `-Source`/`-Out`, wrote via `VACUUM INTO` to a unique sibling temp file, ran `PRAGMA integrity_check` before closing and swapping into `-Out` (with Windows rollback if rename fails), so `-Force` can never destroy the source DB or an existing good backup on failure.
- **Mutation & lock invariants (`model_explain.rs`, `term_link.rs`, `util.rs`)**: enforced single-head validation (`get_model(at).head.address == at`), `lockcheck::preflight` before resetting `-Work`, strict `.zt-model-explain-work` JSON validation, required `state.json` on `apply`/`finish`, atomic `finished_ok` invalidation before `finish` checks, preserving inner typed `term-link` refusals, failing on refused Term cards (`cards_refused > 0` / `complete != true`), preserving `run_contract` when `-Staging` is passed to `term-link -Step relink`, rejecting explicitly empty `-Cards`, and supporting positional steps & `-PlanOnly:$false` in `util::writes_cards`.
- **Core rule & read parity (`post_digest/*.rs`, `mcp.rs`, `check.rs`)**: added `Zt::cards_optional` for existence probes (`draft_diff`, `charts`, `settings`), `archive_revision` coherence checks across multi-call live audits, strict malformed-JSON validation (`--charts-json`, `--draft-diff`, `--tree` cycle/duplicate check, `build.order` array check), `term_role == "list"` detection and exact `b["source"]` broken-link attribution in `--tree`, core `empirical::section`/`display_formulas` (EM3) and trailing-`\` safety in `--settings`, exact `src/model/formal.rs` `proof_route_addresses` parity in `--premises`, Stage 2 empirical alignment and head-only `pending_notation` deferral in `--stage`, Term-head validation in `--draft-diff`, live intuition lint + chart-fence-only `data-zt` scanning + `@media` `span` CSS detection in `--charts`, and rejecting `--format json` on legacy modes / `--workspace` on post-digest modes while reporting exact `Status.blocking` counts in `check.rs`.

---

## Wave 1–2 Gap-Closure & Post-Audit Hardening (`zt-check 0.23.0` / `zt-digest 0.65.0`)

All 8 P0 blockers, 6 P1 parity/hardening gaps, and all 24 findings (10 High, 12 Medium, 2 Low) from the follow-up Astra (`gpt-6.2-astra`, effort `high`, session `01a21c0c-5b40-7db3-9d45-c470a3512a75`) audit were closed via TDD:

- **Installed binaries & versions**: bumped `zt-check` to `0.23.0` and `zt-digest` to `0.65.0`, rebuilt and installed onto `PATH`, and smoke-tested live commands.
- **`zt-digest` safety & refusal contracts**:
  - `backup.rs`: rejects source DB and SQLite recovery sidecars (`-wal`/`-shm`/`-journal`), rejects directories and symlinks, allocates an exclusive sibling temp directory (`create_dir` + checked `getrandom`) before `VACUUM INTO`, uses atomic no-clobber hard-link publication when `!force` (no racy rename fallback), and atomically replaces regular files under `-Force` via single `rename` (leaving the previous backup intact on failure).
  - `term_link.rs`, `term_link/apply.rs`, `args.rs`, `util.rs`: fails closed on any malformed/incomplete `run_contract` (requiring `archive_identity.{id, path}` and matching `citation_key`), suppresses summary printing on `stopped_by_apply`, preserves recovery `TempDir` on stopped/errored relinks, matches `"the archive changed while"` in `is_changing_view`, dispatches `apply` refusals on `code` before `error` (preserving real `write_conflict` -> `stale_base` typed final lines), verifies receipt addresses against requested items and expected plan entries, and supports `-Flag=value` uniformly across `args.rs`, `util.rs`, and `boxpin.rs`.
  - `model_explain.rs`, `lockcheck.rs`, `model/compiler.rs`: uses fail-closed `lockcheck::preflight_strict` before resetting `-Work`, resolves relative `-Work` to absolute paths, intersects live Derivations with explicit `parts[].filed == true`, invalidates `finished_ok` before fallible live revalidation and validates settled jobs in `has_unfinished_state`, prevents bogus `explained 0 part(s)` formatting on recheck stops (`FinishOutput`), wraps `step_finish` `term_link::plan_with` in `retry_changing_view`, and enforces case-sensitive trimmed equality in `test_explain_job`.
- **`zt-check` P0, P1 & Astra parity/schema hardening**:
  - `boxpin.rs` & `check.rs`: discovers pins only from absolute paths, declared path flags (including `--draft-diff` two-arg paths and inline `-Flag:path`/`-Flag=path`), and `cwd` (never treating citation keys or ordinary arguments as paths); rejects `--report` aliasing any input file, arms temp cleanup only after exclusive `create_new` succeeds, publishes `--report` atomically, and rejects unused legacy flags on post-digest modes.
  - `mcp.rs`: `cards_optional` validates every chunk against the core `get_cards` contract (`cards.len() + missing.len() == chunk.len()`, `complete == missing.is_empty()`, ordered `resolved: Vec<(String, Value)>`).
  - `stage.rs`, `network.rs`, `premises.rs`, `settings.rs`, `tree.rs`, `draft_diff.rs`, `charts.rs`, `check-model.mjs`: fail-closed required-field & row validation across all live audits (`build.parts`, `blocks[]`, `block_edges[]`, `parts[]`, `relations[]`, `tree.cards`, single-root connected tree, canonical `get_tree.tree`); exact `lacks == "估计"` empirical Stage 3 classification (eliminating book-key false deferrals); shared `stage::is_notation_card` / `stage::is_criterion_card` across `settings.rs` and `network.rs`; `premises.rs` 1:1 parity with `src/model/formal.rs::proof_route_addresses`; `settings.rs` canonical non-empty `FormalRole::Notation` lender check + handle lookup + unresolved legacy lender fault + equation-first `**估计式**` check; `draft_diff.rs` duplicate/mixed-book/extra-draft guards, cleared-facet guard, strict `parse_links`, full-width `（n）` tags, line-start headings; `charts.rs` fence unwrapping, empty-default `settings` + optional `facets`, single/double-quote & whitespace-tolerant HTML attribute matching, independent intuition-line lint; `network.rs` `layout.js` parity test fails hard on missing file or `node` error.

---

## Wave 3 — Planned (Next)

Mutating stage tools to internalize into `zt-digest`:
- `model-fold` (`model-survey.mjs` + `mask-links.mjs`)
- `model-import` (`imports.mjs`)
- `evidence-dialogue` (`dialogue.mjs`)
- `tree-mover` (`tree-mover.mjs`)
- `lit-organize` (`outline.mjs`, `batch.mjs`, `leads.mjs` using real `term-link`)
- Remove the 6 repo copies of `zt.mjs`.

## Verification

```powershell
cargo test --manifest-path tools/zt-check/Cargo.toml
cargo check --manifest-path tools/zt-check/Cargo.toml --no-default-features --lib
cargo test --manifest-path tools/zt-digest/Cargo.toml
node --test skills/zt-model-restructure/scripts/check-model.test.mjs
```
