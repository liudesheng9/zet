# Math digest P1a: source identity and one primary destination per source object

| Field | Value |
|---|---|
| Status | Design, not built. Written with Astra (rounds 8–11, session `01a20267-721b-7b50-8d38-18d0ea32b49f`); rounds 10 and 11 rechecked it and their fixes are applied here. Round 11: Milestone 1 is ready to implement |
| Date | 2026-11-23 |
| Parent | [`docs/math-digest-redesign-proposal.md`](math-digest-redesign-proposal.md): P1, §3.3 primary-owner coverage, §3.1a attention gates, §5.1 stage contracts, §5.4 qualification |
| Baseline | `c13ca62c`: zt 0.64.0, zt-digest 0.56.1, zt-math-digest 5.13.2 (P0 plus the Thompson pilot fixes) |
| Pilot text | Baby Rudin, *Principles of Mathematical Analysis* (3rd ed.), Chapters 1–3 with the Chapter 1 Appendix and each chapter's Exercises: book pp. 1–75 = PDF pp. 13–87 of `E:\Principles of Mathematical Analysis Third Edition.pdf`, cut to `E:\OCR_inputs\rudin1976principles-ch1-3\Rudin_1976_Principles_of_Mathematical_Analysis_ch1-3.pdf`. The owner ruled on 2026-11-23 that every paid math pilot uses the first chapters of baby Rudin. |

## 1. Why this slice first

The P0 pilot on Thompson (box copy `D:\zt-pilot-thompson`, 2026-11-23) proved that the unit pipeline builds and settles every allocated unit: 17/17 units, 0 untyped Model Cards, 0 placeholders, 0 blocking faults. It did not prove that a reader meets each piece of mathematics once:

- The chapter narrative re-taught the power rule in nine Idea Cards (`/5|c` … `/5|c|8`) beside the theorem's own proof chain. Coverage reported 112/112 because it measures span overlap, not who is responsible for a span (`tools/zt-digest/src/coverage.rs:359`).
- The Exercise I answers were published twice: in the exercise part `/5|a|e` and in the Idea Card `/5|c|9`.
- The canonical numerical example was wrong (`/5|a|c`: 10,200 where the exact value is 10,201), and one non-example was wrong (`/3|b|c`).

Rudin, a proof-led text, raises a sharper form of the same defect. A theorem can be accepted without the proof the book gives. Theorem 1.19 states its result on p. 8, and its proof is the Appendix, pp. 16–20. The finder records one section, one `[line, end]` and one `proof` line per item (`tools/zt-digest/src/math/index.rs:99`). A deferred proof therefore either violates non-overlap or becomes `proof: 0`, and the result is indexed as a concept (`index.rs:226`). The slice holds 140 numbered main-text items and 75 numbered exercises, with no printed answers.

P1a establishes source identity and one primary destination per source object for the consumers named in §3. It keeps the current job sizes and adds no production stage, queue, drive or model role. Later glossary and arc-planning slices can consume these IDs.

## 2. The contract: the interim ownership projection

`math/index.json` gains schema version 2. In P1a it is the **interim projection** of the accepted teaching plan (parent §5.1, Stage 3). When the Stage 3 accepted teaching plan exists, it replaces this file as the one source of the same projection; nothing else changes for the consumers. `math/units.json` (the skeleton's allocation) is derived from it. There is no second ownership path.

Three things are kept apart on purpose:

- **Identity**: `object_id`, the source object a passage belongs to.
- **Primary destination**: the one place a reader goes for it (parent §3.3 dispositions).
- **Writer binding**: the job that writes it, and after allocation the Card address with any section or subpart. The binding lives in the allocation and publication receipts (§4), never in the index.

### 2.1 Source leaves

`prepare` freezes the edition hash, the printed chapter boundaries as **section line ranges** (a chapter break may fall inside a staged section), and a stable ID for every source leaf.

```json
"source": {"sha256": "<bilingual source>", "sections": {"s01.md": "<sha256>"}},
"chapters": [{"id": "ch1", "label": "Chapter 1", "ranges": [{"section": "s01.md", "line": 1, "end": 412}, {"section": "s02.md", "line": 1, "end": 96}]}],
"leaves": [{
  "id": "s03.md:41-47", "section": "s03.md", "line": 41, "end": 47, "col": null, "end_col": null,
  "kind": "paragraph|list|table|display|heading|figure",
  "object_id": "ch1:theorem:1.19",
  "obligation": "write|none", "reason": "",
  "disposition": null
}]
```

- A leaf's `kind` is physical; editorial matter and boilerplate are object kinds (§2.2). A plain heading owes no claim; a heading the OCR made of a labelled item (`# 3.4 Theorem.`) owes one.
- A leaf keeps its physical `kind`, its source locator, its coverage `obligation` and its `disposition` as separate fields. Short text, standalone equations, figures, editorial matter and boilerplate are all leaves. None is dropped. `coverage::units` (`coverage.rs:75`) stays the legacy paragraph projection that today's coverage reads, and it is derived from the leaves.
- A leaf has a unique locator within its frozen section. When two objects share a line (a Rudin Chapter 2 hint that begins on its question's line), the locator adds character offsets (`col`, `end_col`: zero-based Unicode scalar offsets, exclusive end), and the leaf ID includes them (`s07.md:12:0-12:58`).
- An `obligation: none` leaf records its `reason`.
- The paired English and Chinese passages of the bilingual source may support the same object.
- The Appendix lies in Chapter 1's ranges.

### 2.2 Objects and destinations

```json
"objects": [{
  "id": "ch1:theorem:1.19", "kind": "definition|axiom|theorem|lemma|corollary|proposition|example|remark|exercise|hint|answer|notation|exposition|figure|editorial|boilerplate",
  "key": {"chapter": "ch1", "namespace": "theorem", "label": "1.19", "sublabel": null},
  "statement": [{"section": "s02.md", "line": 120, "end": 126, "quote": "…"}],
  "proof": {"status": "inline|deferred_in_slice|source_omits|external|not_applicable", "reason": "",
            "steps": [{"label": "Step 1", "spans": [{"section": "s05.md", "line": 3, "end": 18, "quote": "…"}]}]},
  "destination": "unit:ch1:theorem:1.19",
  "relations": [{"rel": "contains|illustrates|cites", "target": "ch2:exercise:2.26", "status": "resolved|unresolved", "raw": "Exercise 26", "reason": ""}]
}]
```

- **Identity.** The typed key `(chapter, namespace, label, sublabel)` makes `object_id`. Namespaces are separate, so Theorem 1.19 and Exercise 1.19 never collide. An unnumbered item (the corollary after 1.21) gets an ordinal in book order, `ch1:corollary:after-1.21-1`, and keeps its locator. Every printed identity, named entries such as 1.22 *Decimals* included, is an object or has an explicit disposition.
- **Non-mathematical objects.** Exposition, figures, editorial matter and boilerplate are source objects too, so every leaf's destination is carried by an object. An exposition object is a declared run of residual prose (§2.4).
- **Primary destination.** Each leaf has exactly one primary claim, through its object's `destination`. The destination values are the parent §3.3 dispositions: `unit:<id>`, `narrative:<chapter>`, `exercise:<id>`, `figure:<id>`, `provenance`, `boilerplate_skip`.
- **Relations cross ownership; claims do not.** `contains`, `illustrates` and `cites` may point across destinations. A proof that contains a nested example references it at its place in the proof. The example keeps its own destination. A worked example needs a canonical teaching destination. A contextual remark may have an explicitly accepted narrative destination. An `illustrates` relation the finder cannot resolve stays open.
- **Proof status.** `deferred_in_slice` requires step spans. `source_omits` and `external` require a `reason`. Definitions take `not_applicable`.
- **Exercises.** An exercise has destination `exercise:<id>`, under its chapter set in Rudin or under a unit in a book that places exercises there. A proof may cite an exercise without acquiring it. Hints and printed answers are objects of their own (`hint`, `answer`) with spans, related to their exercise or subpart. Printed hints render as `提示`. A matched printed answer renders under `答案` at its own question or subpart. An answer is never written without its answer span. `exercises[]` is a derived view of these objects, not a second store.

### 2.3 Who decides

DeepSeek proposes objects and destinations in its finder answer. The script accepts them in two passes. Pass one collects, validates and indexes the independently identified objects of all packets, exercises and chapter sets included; their unit-form subset is the build inventory of this run. Pass two binds the dependent fragments: a proof to its statement, a hint or answer to its exercise or subpart. An ambiguous assignment stays open and blocks the work that depends on it. In the future Stage 3, Sonnet makes the disputed teaching decisions once and supplies the same projection. No role decides an object twice.

### 2.4 Finding, joining and reconciling

- **The finder emits packet-local fragments.** Each fragment carries source IDs, exact quote anchors and its typed target key. A proof fragment carries ordered spans even when the source numbers no steps; each printed step carries its own spans. The target key of a dependent fragment needs evidence supplied with the packet: a local citation, or a source-backed enclosing heading or forward pointer. Without it the fragment stays unresolved, and an unresolved required proof blocks acceptance. A packet that sees Appendix Step 6 but never sees Theorem 1.19 still emits Step 6 with the key `(ch1, theorem, 1.19)` when the packet supplies that key evidence. The finder's SELF-CHECK (`math -Step index-check`) checks every new field: spans in range, quotes found, no overlapping primary claims inside the answer.
- **`index-apply` joins.** It joins uniquely keyed fragments in source order, assigns ordinals to unnumbered items in book order, resolves relations after every packet has returned (forward references included), and keeps every ambiguous or missing target with its evidence. Nothing is dropped. This replaces today's silent `filter_map` (`index.rs:232`).
- **The label scan reports candidates only.** A deterministic scan lists the printed labels per chapter (for the Rudin slice: 38 + 47 + 55 main-text items, 20 + 30 + 25 exercises). It never creates an object or decides promotion.
- **Residual exposition is declared, never defaulted.** The finder may mark a contiguous run of leaf IDs as residual exposition in one declaration. Index finish accepts that positive disposition after it checks the bounds, competing claims and the printed-identity reconciliation. Every other unclaimed instructional leaf stays open and blocks index settlement. The absence of a printed label is no evidence of who teaches a passage: a missed unnumbered corollary (after 1.21) or a lost Appendix proof continuation would otherwise gain narrative coverage while all numbered identities reconcile. A wrong explicit classification is caught only by a fidelity fixture or reader review.
- **`math-index/book@1` finishes only on identity.** Every expected printed identity must match an object or an approved disposition. Totals alone cannot settle it: a duplicate cannot mask a missing theorem.
- **The settled index is immutable.** The index receipt hashes source identities, spans, destinations and obligations. Allocation and publication receipts bind that hash to jobs, addresses, subparts and fingerprints, and consumers join those records by logical ID. Allocation never rewrites the settled index, so the first allocation cannot invalidate its own producer.

## 3. Consumers

| Consumer | Change |
|---|---|
| **Narrative writer** (`group_start`, `ledger.rs:8320`) | The prompt sends the group's assigned leaves to write, the unit summaries and addresses to link, and only the adjacent context it needs. Each printed chapter has one entry anchor that lists its units in reading order, bound to one writer group. Only that group edits the route, and its group settles with a new **`route`** disposition when it has nothing else to write: an edit of the anchor that adds the route and nothing else (`no_cards` forbids writes, so it cannot represent this). A group with no prose, no exercise delivery and no route assignment settles with a verified zero-write disposition. Both are validated at draft, preview, finish and replay. The writer of a chapter exercise set gets every question and hint span of the set, including spans in other groups. It writes the problems faithfully, renders hints as `提示`, links the units each exercise uses, and writes no answer that has no answer span. |
| **Coverage** (`coverage.rs`) | Group coverage checks the group's assigned leaf IDs. Book coverage checks the union of all verified dispositions. New blocking fault **C6**: a draft claims (`src`) a leaf whose primary destination is elsewhere ("leaf `<id>` belongs to `<destination>`: link it, do not restate it"). The existing C3 (advisory numerical fidelity) stays as it is. A reference earns no coverage. C6 proves only declared claims: a writer can still paraphrase a proof under a different `src`, which the pilot samples separately. |
| **Card review and fix** (`card.rs` task builder, `card.rs:650`) | Each review packet carries the assigned leaf IDs, the allowed endpoints, the source excerpts and the claims actually outstanding. A defect in a unit is routed to the unit's owner, never fixed in prose. |
| **Unit compiler** (`model/context.rs:229`) | Brief and architect get the unit's ordered owned spans: statement, proof steps (all sections, in order), attached remarks and referenced nested material. The pack no longer points at the group's sections. After the architect assigns evidence to parts, each part's evidence is checked against the part source budget (`compiler.rs:1745`, 12,000 UTF-16 units) before fill. An oversized assignment goes back for bounded correction. |
| **Proof review** (`math -Step proof`) | Every source proof owes fidelity evidence. The fidelity obligation and its verification mode are frozen before fill: the model receipt records accepted inline evidence, and the independent proof receipt records its clause and proof verdicts. One verdict may discharge the same obligation at the same revision. The review job carries the source proof spans next to the unit's proof steps. It also carries the published head and the source statement, and asks for a clause-level verdict on the statement besides the proof verdict. |
| **Skeleton** (`step_skeleton`) | Allocates one unit head per `unit:` destination, in source order, filed under the entry anchor of its chapter. The group unit list (`math.rs:816`) is generated from the projection. |
| **Math check** (`check_value`) | **M0**: expected unit IDs, allocation and live typed heads agree. **M19**: a leaf with no primary claim or with two. **M20**: a unit with no current build, a required proof unjoined, or no **passing** fidelity verdict for the current head and proof fingerprints (a missing, failed or stale verdict blocks). **M21**: every expected exercise and subpart ID is compared with its published endpoint in both directions, so an omitted exercise fails as well as a wrong one. The receipt records the question, hint and answer leaf IDs, `answer_status` and the accepted rendered fields; the check renders those fields and compares the read-back Card against that rendering. Each canonical learning task needs one reachable endpoint; folded objects may share it, and a justified exclusion (`boilerplate_skip`, `provenance`) needs only its disposition. M19–M21 count in `blocking` (`math.rs:1944`). |

## 4. Ledger and settlement

No new unit kind. Existing units carry the evidence:

| Unit | Evidence it records |
|---|---|
| `math-index/book@1` | The projection's hash over source identities, spans, destinations and obligations; the structural checks of §2.4 run in its finish, before any dispatch. |
| `math-skeleton/book@1` | The object-ID-to-address binding against that hash (the index itself is not rewritten). |
| `digest/gNNN@1` | The assigned leaf IDs it covered, its disposition (`cards`, `route`, zero-write), the route links, the exercise rendered fields, and the read-back content fingerprint. |
| `models/gNNN-uIIII@1` | The owned object IDs, source hashes, the published head and part fingerprints, and, when the proof is accepted inline, the obligation ID, the passing verdict and the judged fingerprints. |
| `proof/<unit>@1` | The obligation ID, the source proof spans the verdict covered, the clause and proof verdicts, and the head and proof fingerprints they judged. |
| `math-check/book@1` | The join of every receipt against the projection hash, the obligation ID and the current content revision. |

A changed dependency reopens the work that depends on it. Resume reuses a receipt whose projection hash, obligation ID and content fingerprint still match. A blocked prerequisite withholds quality even when the math check cannot run. Pre-finalize bounded repair of math faults (parent §5.1 Stage 8) stays outstanding; it is not part of P1a.

## 5. Job map

These are **interim P0 sizes**: P1a changes only payloads. The parent still proposes chapter-bounded finder packets, chapter narrative jobs and one substantial proof per review; those are later slices. Each module keeps its one manager queue.

| Workflow/module | Job | One subagent covers | Messages per conversation | Pattern | Status | Where the size is set |
|---|---|---|---|---|---|---|
| Math statement finder | Fragments with typed keys, statement and step spans, relations, hints, answers | 30,000-character packing target; an oversized group currently travels alone | 1 | One message | PROPOSED payload | `math-steps.ps1 -IndexChars`, `math.rs index_jobs` (`math.rs:257`) |
| Digest groups, math | Assigned leaves and route; exercise sets without unsourced answers | One group (unchanged) | 1 | One message | PROPOSED denominator | `group_start`, `coverage.rs` |
| Card review and fix | Residual coverage and route fidelity | 40 Cards/job | 12/message, existing fix rounds | Messages, then Continued | PROPOSED payload | `card.rs` task builder and fix SELF-CHECK |
| Math unit builds | Write the unit from its owned spans | One unit (unchanged) | As the model layer | Continued | PROPOSED source input | `model/context.rs` |
| Math proof review | Proof and statement fidelity against the source spans | ≤ 4 units (unchanged) | 1 | One message | PROPOSED evidence | `math.rs step_proof` |

## 6. Build order and tests

P1a is built in two milestones. Every step is test-first at the seam it names.

**Milestone 1 (one session): inventory and joining, offline.**

1. Leaf inventory: no dropped short text, an attached display and a genuinely isolated display as separate cases, chapter ranges that split a staged section, and a hint that begins on its question's line (two leaves with distinct IDs, told apart by character offsets).
2. Identity and joining: Theorem 1.19 in one section, its nine Appendix steps in another packet, Exercise 1.19 in a third. One theorem object with a joined `deferred_in_slice` proof of nine ordered steps, a distinct exercise object, no claim on the theorems between statement and proof. The statement pass runs before the dependent pass, and a proof fragment with no key evidence stays unresolved and blocks.
3. Relations and reconciliation: a nested example (its own destination, referenced by the proof), a forward reference resolved, an unknown reference kept open with its reason, and a missing label masked by a duplicate (blocks). Residual prose: explicitly declared ordinary prose passes; silently omitted ordinary prose blocks; omitting the unnumbered corollary after 1.21 blocks even when all numbered identities reconcile.
4. Immutability: allocation leaves the index hash unchanged.

**Milestone 1 status (2026-11-23):** built as `tools/zt-digest/src/math/source.rs` (offline; 38 tests), checked on the Rudin slice's OCR, reviewed by Astra rounds 13–16 ("Milestone 1 done").

**Milestone 2: consumers and settlement.** This milestone must pass before a paid pilot. Carried from the Milestone 1 review: the label-token boundary also checks the original neighbouring characters (a `1.1` followed by whitespace and another number is valid).

5. Owner projections through the draft SELF-CHECK (C6), the `route` and zero-write dispositions at draft, preview, finish and replay (only the bound writer group edits a route), the card review packet and its routing of unit defects, and compiler packaging with the budget check after the architect.
6. Exercise rendering and read-back (M21), against the expected set in both directions: Rudin's `no_source_answer` sets with hints, and an omitted exercise that fails; Thompson's Exercise I with its printed key as an offline fixture (`matched`, 10/10 pairs).
7. Settlement: stale receipts reopen their dependents, crash and replay keep addresses and receipts, a failed or stale fidelity verdict blocks (M20), and quality is withheld on a blocked prerequisite.

Thompson's matched answers, its exact-versus-approximate arithmetic and its figure provenance stay as **offline fixtures**. Every paid pilot uses Rudin.

## 7. The Rudin pilot

The pilot runs on a fresh box copy built the way the Thompson box was (own config and archive, `zt-box.txt` pin, source check to 0 faults, `figures: false`, `bridge: false`), from the bilingual Markdown of the Rudin slice. Before dispatch, the pilot freezes, independently of the finder and the label scan (so an item both miss is still caught), the expected printed keys and their locators, the expected unnumbered objects by kind and source locator, and the designated main-result IDs of each chapter. A copied-box pass earns **pilot** status for P1a only. It qualifies neither full P1 nor production replacement.

| Gate | Kind | Pass |
|---|---|---|
| Identities | Hard | Every frozen expected key accepted (140 numbered main-text items, 75 exercises, and the unnumbered items), three chapters, the Appendix joined to 1.19 |
| Destinations | Hard | 0 leaves with no or two primary claims; 0 C6 at publication; one reachable endpoint per canonical learning task (folded objects may share one; justified exclusions need only their disposition) |
| Exercises | Hard | Every expected exercise and subpart has its endpoint and every endpoint its exercise; every question and hint read back against its rendering; 75 `no_source_answer`; 0 answers without an answer span |
| Proof and statement fidelity | Hard (passing verdicts) and fixture (paid, small) | Every required verdict passes for the current head and proof fingerprints. The seeded fixture: 1.19 without the rational subfield, 2.41 stated for arbitrary metric spaces, 3.11 claiming Cauchy implies convergence in every metric space, and an omitted or reordered Appendix step, each flagged; the correct controls pass. Check every fixture against the PDF, because text extraction can drop closure and conjugation bars. |
| Route distance | Hard | The final check traverses labelled Links from the rendered chapter entry to each frozen main-result endpoint: ≤ 2 moves |
| Restatement | Sample | Reviewers look for proof restatements in prose that a different `src` let past C6 |
| Reader tasks | Pilot measure | Retrieve 1.19's construction; say exactly where 2.41 needs Euclidean space; distinguish 3.11 (a), (b), (c); find Exercise 2.26's hint. Record correctness, wrong turns, Cards opened and the sample size; provisional target ≥ 80 % correct. |
| Cost | Pilot measure | Matched Rudin input, model and effort; usage coverage, retries, drives, DeepSeek cost per accepted canonical unit and per reader-ready chapter, Claude weighted tokens (`session-report.mjs`, `token-usage.mjs`). Job sizes stay the owner's. |

Study Desk parity, the Theory routes and the full P1 reader qualification remain outstanding after this pilot.

## 8. Deferred to later P1 slices

The compact glossary Term format (next), the true chapter spine with promotion, folding and arc planning (Sonnet once per arc, replacing this interim projection with the accepted teaching plan), L5 compact units with the section-aware Study Desk, notation scope, figure binding, the remaining P1 visual requirements, chapter-bounded job sizes, and pre-finalize bounded repair.

## 9. Changes to the parent proposal

- §§3.3 and 5.1: document the interim ownership projection, its decision rule (§2.3), and its replacement by the Stage 3 accepted teaching plan.
- §5.2: the five interim rows above.
- §§5.4 and 6.2–6.4: paid qualification uses Rudin only. Thompson's paired answers, arithmetic and figure provenance stay as offline fixtures.
- §6.1: record the Thompson P0 observations (box copy, `c13ca62c`) and the three outstanding controls separately: the injected allocation crash, a deliberately unwritten theorem, and an economics `NO_FORM` control.
- P1a acceptance qualifies neither full P1 nor production replacement.
