# First-Class Multi-Coordinate Cards in ZET (Final Proposal)

Owner: Xingtong Chen

| Field | Detail |
|---|---|
| **Status** | **Implemented, Live `D:\zt` Migrated to v2 & v1 Code Removed** (`MC1`–`MC4` and `P1`–`P5` complete; Astra `READY TO CHECK THE BOX`; live `D:\zt` migrated with verified backup; codebase cleaned to pure v2-only; see §12) |
| **Date** | 2026-11-27 |
| **Repository Baseline** | `E:\zet` checkout `22a98736` (`zt` 0.71.0; also audited against 0.70.1 release `43417f79`) |
| **Live Archive Evidence** | Read-only SQLite audit of `D:\zt\zt.sqlite3` at revision `148217`: `62,322` live Cards, `1,459` retired identities, `0` current Connection trees/placements/edges, `20` issued handles, `122` retired placements, `0` active Locks (`24` Lock history rows), `166,710` `applied_ops` receipts, `0` missing inferred parents, **`303` sparse Side-label sibling groups** |
| **Design Review & Audit** | Architecture & coaction review by **Astra** (`gpt-6.2-astra`, high reasoning effort, standard tier; session `01a22099-e605-7993-ab29-1117f08cbba1`), independently audited and refined against the codebase by **Gemini** (§10) |
| **Chinese Version** | [multi-coordinate-cards-proposal.zh.md](multi-coordinate-cards-proposal.zh.md) (shared decision IDs `MC1`–`MC4`, sections `§0`–`§11`, and phases `P1`–`P5`) |
| **Supersedes / Evolves** | [multi-tree-coordinates-proposal.md](multi-tree-coordinates-proposal.md), [connection-trees-free-proposal.md](connection-trees-free-proposal.md), [coordinates.md](wiki/coordinates.md) |

---

## 0. Executive Summary & Approved Decisions

### The Core Contract

> **A Card is one authored document (`card_id`) that holds a non-empty set of Coordinates (`>= 1`). One Coordinate is one place in one Tree. A Card may hold multiple Coordinates across different Trees AND multiple Coordinates inside the same Tree—including a Tree root or its Direct successor—while keeping exactly one designated Filing Coordinate for scholarly provenance, ownership, and structural reports.**

Today (zt 0.70–0.71), ZET splits coordinates across two unequal worlds: ordinary Topic and Literature Cards live at a single implicit location (`cards.location` / `cards.citation_key`), while Connection trees (`~n`) bolt on a separate `placements` table with `UNIQUE(host_tree_key, card_id)` (at most one coordinate per Card per tree) and Topic trees bolt on a separate `<loc>+k` side-mount shelf (`EdgeKind::Attach`). That realization feels unnatural because placing a Card in another tree—or in two places of the same tree—is treated as an exception instead of a first-class property of Cards and Trees.

This proposal unifies ZET around **first-class multi-coordinate Cards**:

1. **Universal Coordinate table (`coordinates`):** Every Card occurrence in every Tree (`literature`, `topic`, `connection`) is an explicit, independently addressable Coordinate (`coord_id`) with its own parent Coordinate, `Direct`/`Side` edge, explicit `slot` (preserving sparse legacy labels), optional positional `gloss`, and independent children.
2. **Same-tree multiplicity & self-ancestry:** `UNIQUE(tree_id, card_id)` is removed. A Card `X` can appear at `1/1`, `1/4|b`, and `2/3|a` simultaneously—or even beneath itself (`X(c1) -> Y(c2) -> X(c3)`). Editing `X` edits the single Card everywhere; adding a child at `1/4|b` affects that Coordinate alone.
3. **Commit-enforced `>= 1` floor:** Every live Card MUST have at least one Coordinate (`filing_coord_id`, in the Card's owning `Workspace`), enforced at SQLite commit time via a deferred composite foreign key so no detach, batch, or tree deletion can orphan a Card or silently flip Workspace ownership.
4. **Zero ambiguity & zero scholarly regressions:** Every Tree keeps a single non-colliding coordinate space; scholarly computations (Literature BibTeX, Models, Math units, Term Cards, Cited Cards, Hub Cards, Write Gate, `zt-check`, `zt-digest`) resolve deterministically through each Card's single Filing Coordinate.

### Owner-Approved Product Decisions (`MC1`–`MC4`)

| ID | Decision | Approved Choice |
|---|---|---|
| **`MC1`** | **Connection trees (`~n`) and Topic `+k` side mounts** | **Keep `~n` as a lightweight free-tree role** on the unified `coordinates` engine (optionally untitled root + `coordinate_edges` + `crossings` projections), and **retire Topic `+k` side mounts (`EdgeKind::Attach`)** because Topic trees now natively admit foreign and repeated Coordinates on standard `Direct`/`Side` slots. |
| **`MC2`** | **Self-ancestry & Root multiplicity** | **Allow same-Card self-ancestry** (`X(c1) -> Y(c2) -> X(c3)`) and **allow a defining Tree root Card to recur as a non-root Coordinate** (in other Trees and beneath itself), while keeping **at most one Tree that a Card is the defining root of**. |
| **`MC3`** | **Literature tree admission policy** | **Keep Literature trees closed to foreign Cards in v1**, while allowing **repeated Coordinates of that book's own filed Cards** (including its root). Topic and Connection trees accept any live Card from anywhere in the archive. |
| **`MC4`** | **Filing Coordinate & Single-Coordinate-Space guard** | **Keep one designated Filing Coordinate (`filing_coord_id`) per live Card** plus a **clean splice guard** refusing to splice a Secondary Coordinate directly between two active `model`/`term` filing-chain nodes—guaranteeing that every coordinate string in a Tree uniquely names at most one Card, with no parallel "editorial parent" graph to drift out of sync. |

---

## 1. Motivation, Diagnosis & Verified Live-Archive Baseline

### 1.1 Diagnosis of the 0.70–0.71 Connection-Tree Realization

The zt 0.65–0.71 coordinate implementation established two valuable primitives worth preserving: immutable `card_identity` (with retirement instead of deletion) and durable occurrence IDs (`placement_id`) carrying glosses and tree-local assertions. However, it breaks naturalness at four points:

1. **Asymmetric representation of nodes:** Ordinary Topic and Literature Cards get a `card_identity` row during backfill ([coord.rs](file:///e:/zet/src/coord.rs#L474)) but **no row** in `placements`; their tree topology is inferred by string arithmetic on `cards.location` / `cards.citation_key` ([main.rs](file:///e:/zet/src/main.rs#L3003)). Only Connection trees (`~n`) and Topic `+k` shelves have rows in `placements`.
2. **One-Card-per-tree cap (`UNIQUE(host_tree_key, card_id)`):** Placing `X` a second time in the same tree silently returns the existing station ([connection.rs](file:///e:/zet/src/connection.rs#L1354)), and batch placement deduplicates by `card_id` ([connection.rs](file:///e:/zet/src/connection.rs#L1451)). A Card cannot occupy two branches of one outline or recur below itself.
3. **Topic trees cannot natively host foreign or repeated Coordinates:** Putting an external Card into a Topic tree requires the bolt-on `<loc>+k` shelf syntax keyed by `anchor_card_id` ([coord.rs](file:///e:/zet/src/coord.rs#L298), [coord.rs](file:///e:/zet/src/coord.rs#L1144)), which cannot have its own Direct/Side children and collides if an anchor moves into a Topic that already mounts that Card ([coord.rs](file:///e:/zet/src/coord.rs#L1516)).
4. **Asymmetric metrics and tooling:** `places` counts only `placements` rows ([connection.rs](file:///e:/zet/src/connection.rs#L642)), under-counting every Topic and Literature Card by 1; native Connection Cards require a special `promote_card` workflow to move their home into a Topic tree; `TreeShape::placement_of_card` ([coord.rs](file:///e:/zet/src/coord.rs#L1031)) assumes at most one occurrence per Card.

### 1.2 Verified Live-Archive Baseline (`D:\zt`)

Read-only SQLite inspection of `D:\zt\zt.sqlite3` at revision `148217` (daemon healthy, `coordinates: ok`, Lock integrity OK):

| Verified Fact (`D:\zt`, rev `148217`) | Count | Design Consequence |
|---|---:|---|
| Live Cards | 62,322 | Migration, Snapshot load, and `zt status` checks must stay fast at ~62k-Card scale. |
| Retired Card identities | 1,459 | Retired identities, last addresses, and retirement revisions must be retained verbatim. |
| Current Connection trees / placements / placement edges | 0 / 0 / 0 | **Optimal transition window:** zero live Connection trees or `+k` shelves to rewrite, enabling clean retirement of `+k` (`MC1`). |
| Issued permanent handles (`card_handles`) / retired placements | 20 / 122 | Historical handles and retired bookmark IDs exist and MUST be preserved across migration. |
| Active Locks / Lock history rows | 0 / 24 | Live DB has historical Locks only; active-Lock and populated-Connection-tree test fixtures are mandatory before release. |
| Applied-operation receipts (`applied_ops`) | 166,710 | Historical receipt hashes and replay contracts MUST survive unchanged. |
| Missing inferred filing parents in Topic/Literature | 0 | Every existing Topic/Literature Card cleanly resolves to its tree root. |
| Sibling groups with **sparse** Side labels (gaps like `|a`, `|c`) | **303** | **Critical:** Legacy deletion deliberately skips compaction when compaction would make a Broken link hijack a renamed sibling ([main.rs](file:///e:/zet/src/main.rs#L4331)). Unified coordinates MUST store explicit positive `slot` integers and NEVER enforce dense `1..N` Side ordinals ([coord.rs](file:///e:/zet/src/coord.rs#L1098)). |

---

## 2. Conceptual Ontology & `CONTEXT.md` Vocabulary Updates

The canonical terms below extend and update `CONTEXT.md` while keeping all existing glossary terms (`Card`, `Regular Card`, `Literature Card`, `Citation key`, `Location`, `Topic`, `Pointer`, `Edit caret`, `Session`, `Link`, `Broken link`, `Reverse link`, `Direct successor`, `Side successor`, `Workspace`, `Zone`, `Access`, `MCP server`, `Lock`, `Pinned target`, `Model Card`, `Model part`, `Term Card`, `Cited Card`, `Hub Card`) intact.

| Term | Definition | Avoid |
|---|---|---|
| **Card** (`card_id`) | One authored Markdown document (title, body, optional BibTeX, `CardType`, Model/Math/Hub/Term metadata) with immutable `card_id`, preserved `row_id`, single owning `Workspace` (`human` \| `agent`), `status` (`live` \| `retired`), and—while live—one **Filing Coordinate** (`filing_coord_id`). | Note item, record |
| **Tree** (`tree_id`) | One rooted Folgezettel outline with immutable `tree_id`, `role` (`literature` \| `topic` \| `connection`), owning `Workspace`, public key (`<CitationKey>`, `<n>`, or `~<n>`), and single root Coordinate (`edge = 'root'`). | Folder, graph |
| **Coordinate** (`coord_id`) | One identified place of a Card in a Tree: `(coord_id, tree_id, card_id, parent_coord_id, edge, slot, gloss)`. Each Coordinate has its own Direct and Side children and its own optional positional `gloss`. A Card holds at least one Coordinate (`>= 1`) and may hold multiple Coordinates across Trees and inside the same Tree. Its path string (`1/2\|a`, `Hart1995/3`, `~7/2`) is derived from `(parent_coord_id, edge, slot)`. | Copy, reference, station (superseded as a separate concept) |
| **Filing Coordinate** (`filing_coord_id`) | The single designated live Coordinate of a Card, always located in a Tree of the Card's owning `Workspace`. It is the authoritative anchor for source provenance, Model/Math/Term/Cited structure, Hub coverage, Write Gate context, Lock member sealing, and canonical Card citation. | Primary copy, master node |
| **Secondary Coordinate** | Any Coordinate of a Card other than its Filing Coordinate (`coord_id != card.filing_coord_id`), in the same Tree or in another Tree (of either Workspace). Rearranging, glossing, or detaching a Secondary Coordinate never edits the Card's body and never alters scholarly reports. | Alias, mount copy, shadow card |
| **Connection tree** (`~<n>`, 关联树) | A Tree of role `connection` backed by the same unified `coordinates` table. Adds an optionally untitled root (outside Topic Hub/Register rules), tree-local **Tree connections** (`coordinate_edges`) between any two Coordinates in the tree, and **Projection** (`crossings`) onto Topic and Literature trees. | Reading tree, relation tree |
| **Tree connection** (`coordinate_edges`) | An authored association (`connection_id`) recorded by a Connection tree's editor between two distinct Coordinates in that tree (`from_coord_id <> to_coord_id`, including two different Coordinates of the same Card): neutral or directed, optional `relation_label` (1–32 chars), optional canonical `relation_kind`, optional plain-text `note` (<= 400 chars). Follows `coord_id`s across reorders; never alters Card text or the Link index. | Link, edge |
| **Missing coordinate** | A surviving Secondary Coordinate whose target `card_id` was retired (`status = 'retired'`). Preserves its `coord_id`, path, gloss, children, and incident Tree connections; a new Card created later at the old path never revives it. Repaired by rebinding (with explicit per-connection disposition) or detaching. | Broken link, dead node |
| **Projection** (`crossings`) | The part of a Connection tree `~n` whose live Cards appear in the Topic or Literature Tree `T` being read, keyed by `coord_id`: in-plane Coordinates, structure lines to nearest in-plane ancestors (with outside `hops`), in-plane Tree connections (including lines between two Coordinates of the same Card), and per-Coordinate exits. | Overlay, copy |

*(Retired concept: **Topic-side mount / `<loc>+k`** is superseded by native Direct/Side Coordinates in Topic trees under `MC1`.)*

---

## 3. Six Hard Structural Invariants

### Invariant 1 — Minimum-One-Coordinate Floor (`|coords(card)| >= 1`), Commit-Enforced
- Every live Card has `filing_coord_id IS NOT NULL`; every retired Card has `filing_coord_id IS NULL`.
- A `DEFERRABLE INITIALLY DEFERRED` composite foreign key `card_identity(filing_coord_id, card_id, owner_workspace)` → `coordinates(coord_id, card_id, host_workspace)`, combined with `coordinates(tree_id, host_workspace)` → `trees(tree_id, workspace)`, guarantees at `COMMIT` time that:
  1. No live Card can exist with `0` Coordinates.
  2. A Card's `filing_coord_id` ALWAYS points to a Coordinate of *that* Card in a Tree of *that* Card's `owner_workspace`.
  3. No single op, batch, subtree detach, or tree delete can strip a Card's last owner-Workspace Coordinate while leaving a foreign-Workspace Coordinate behind.
- **Trigger & PRAGMA discipline:** Both `open_db` ([main.rs](file:///e:/zet/src/main.rs#L2659)) and `initialize_database` ([main.rs](file:///e:/zet/src/main.rs#L1482)) MUST execute `PRAGMA foreign_keys = ON`. Because SQLite `CHECK` constraints are non-deferrable, the legacy `identity_after_insert` trigger ([coord.rs](file:///e:/zet/src/coord.rs#L379)) is retired and replaced by one centralized transactional Card+Coordinate creator inside `BEGIN IMMEDIATE` (backed by writer-epoch-2 enforcement).

### Invariant 2 — Single-Coordinate-Space Invariant (Zero Dual-Meaning Paths) (`MC4`)
- Within any Tree at any archive revision, **every derived coordinate path string (`1/2`, `Hart1995/3|a`, `~7/2`) maps to at most ONE `coord_id` (and thus one Card).**
- ZET NEVER permits a Topic or Literature Tree to have a "reading path `1/2` = Card X" colliding with an "editorial filing address `1/2` = Card Y".
- Path derivation rules (identical across all three tree roles):
  - `edge = 'root'`: `<CitationKey>` (Literature), `<n>/0` (Topic), or `~<n>/0` (Connection).
  - `edge = 'direct'`: root's Direct child is `<tree>/1`; numeric leaf increments (`1/2 -> 1/3`); Side leaf appends `|1` (`1/2|a -> 1/2|a|1`). Unique per `parent_coord_id`.
  - `edge = 'side'`: `<parent_path>|<number_to_side_label(slot)>` using the **stored** positive integer `slot` (`1 -> a`, `2 -> b`, `27 -> aa`). Unique `(tree_id, parent_coord_id, slot)` where `edge = 'side'`. Gaps are valid and preserved (supporting all 303 sparse groups on `D:\zt`). No Side child directly on a Tree root.
- **Consequence for Secondary Coordinates in Topic/Literature trees:**
  - Appending a Secondary Coordinate at a free Direct tip or free Side slot shifts **zero** existing Coordinates → **zero** Link rewrites.
  - Splicing a Secondary Coordinate *before* an existing Topic/Literature Filing Coordinate runs the standard move/insert mapping (`1/2 -> 1/3`), preview, Lock gate, and Link-rewrite checks so paths and Links stay 100% consistent.

### Invariant 3 — Coordinate-Keyed Topology & Allowed Self-Ancestry (`MC2`)
- Every tree traversal, outline builder, move-ancestry cycle check, subtree enumeration, GUI node key, and bookmark operates on **`coord_id`**, NEVER on `card_id`.
- **Same-Card self-ancestry is permitted and cycle-free:** `X(c1) -> Y(c2) -> X(c3)` is a valid 3-Coordinate tree. Moving `c1` under `c3` creates a `coord_id` cycle and is refused. Children of `c1` and children of `c3` are completely independent.

### Invariant 4 — Root Multiplicity (`MC2`)
- Each Tree has **exactly one** root Coordinate (`UNIQUE(tree_id) WHERE edge = 'root'`).
- Each Card is the **defining root of at most one Tree** (`UNIQUE(root_card_id)` on `trees`, equal to its `filing_coord_id`). This keeps Literature BibTeX identity, Topic `<n>/0` identity, Lock root identity, and whole-tree delete completely unambiguous.
- That root Card MAY hold any number of **non-root Secondary Coordinates** in other eligible Trees AND beneath itself in its own Tree. A non-root Coordinate of a root Card is rendered as a single node with an explicit **Open tree** portal action; rendering, locking, seeding, and exporting never recursively inline its home tree.

### Invariant 5 — Tree-Role Admission & Retirement of `+k` Shelves (`MC1`, `MC3`)
- **Topic (`<n>/0`) and Connection (`~<n>`) trees:** Admit any live Card from anywhere in the archive (either Workspace, root or Regular, native or foreign, once or multiple times) onto standard `Direct` or `Side` Coordinates.
- **Literature (`<CitationKey>`) trees:** In v1, admit **repeated Coordinates of that Literature tree's own filed Cards** (including its root), and refuse foreign Cards (keeping source provenance clean for digests and exports; foreign book-margin annotations can be added later without changing schema).
- **Topic-side mounts (`<loc>+k`) retired:** `EdgeKind::Attach` and `+k` syntax are removed for new writes. Any legacy test/external archive holding `+k` rows migrates them deterministically to Side Coordinates (`slot = max_slot + 1, ...`) under the anchor's Filing Coordinate.

### Invariant 6 — Editorial-Chain Integrity Without Parallel Parent Graphs (`MC4`)
- Structural readers (`model`, `math`, `term`, `cited`, `hub`) filter strictly to **Filing Coordinates (`coord_id == card.filing_coord_id`)**.
- Therefore, a Secondary Coordinate of a Model part placed under another Model head `M2` never joins `M2`; a Secondary Coordinate of a Term Card under a Term list never becomes a Term head; a Secondary Coordinate of a Cited Card beside a Cited list consumes zero bibliography rows.
- **Clean splice guard:** In a Topic or Literature Tree, splicing a **Secondary Coordinate** directly *between* two Filing Coordinates that belong to an active `model` or `term` filing chain (where the displaced Direct child is a filing `Model part` or filing `Term part` whose filed parent is the `model`/`term` Card immediately above) is refused with an actionable hint: *"place as a Side branch, or arrange freely in a Connection tree `~n`"*. This guarantees `find_head` and `term::place` never silently sever while avoiding a second parallel "editorial parent" graph.

---

## 4. End-to-End Walkthroughs

### Scenario 1 — Same-Tree Multi-Coordinates & Self-Ancestry
In Topic `3` (or Connection tree `~7`), Card `X` is placed at `~7/1` (`c1`), Card `Y` is `c1`'s Direct child at `~7/2` (`c2`), Card `X` is placed *again* as `c2`'s Direct child at `~7/3` (`c3`), and Card `X` is placed a *third* time as a Side branch of `c1` at `~7/1|a` (`c4`):
- `c1`, `c3`, and `c4` all point to the **same `card_id`** (`X`).
- Adding a Side child `Z` under `c3` (`~7/3|a`) attaches `Z` to `c3` alone; `c1` and `c4` do not gain `Z`.
- Giving `c3` gloss *"Revisit X after Y's counterexample"* and `c4` gloss *"Alternative entry"* keeps both glosses independent.
- Drawing a Tree connection from `c3` to `c4` (`from_coord_id = c3`, `to_coord_id = c4`, valid because `c3 <> c4`) records how the two appearances relate.
- Editing `X`'s text from `c4` updates `X` across `c1`, `c3`, and `c4` immediately.
- Detaching `c3` removes `c3` (and its child `Z` per child policy) and retires `c3` so `place:<c3>` reports removed; `c1`, `c4`, and `X` itself survive untouched.

### Scenario 2 — A Root Card With Multiple Coordinates
Literature root `Hart1995` (a 500-Card Locked book) holds:
- Its defining root Filing Coordinate `Hart1995` (`c_root`, `edge = 'root'`, `is_filing = true`);
- A Secondary Coordinate at `Hart1995/8|a` (recurring inside its own book's synthesis section);
- A Secondary Coordinate at `1/2` in human Topic `1` ("Theory of the Firm");
- Two Secondary Coordinates at `~7/2` and `~7/6|a` in Connection tree `~7`.
All four Secondary Coordinates are single nodes with an **Open tree (`Hart1995 · 500 cards`)** portal button. Detaching `1/2` removes only that Coordinate in Topic `1`; `Hart1995`'s BibTeX, root Lock, and 500-Card Literature tree are completely unaffected.

### Scenario 3 — A Direct Successor With Multiple Coordinates
In Topic `1`:
- Root `1/0` has Direct successor `1/1` (`Card A`, `c1 = filing_coord_id`).
- `Card A` is also placed at `1/4|b` (`c2`, same Topic tree) and `2/3|a` (`c3`, Topic `2`).
- `Card A` therefore has 3 Coordinates (`{1/1, 1/4|b, 2/3|a}`).
- Deleting the branch rooted at `1/4` removes `c2`; because `Card A` still has its Filing Coordinate `c1` at `1/1`, `Card A` is **not deleted**.
- If the owner explicitly wants `1/4|b` to become `Card A`'s primary Filing Coordinate and remove `1/1`, they run `refile_card` (moving `filing_coord_id` from `c1` to `c2`, rewriting `[[1/1]] -> [[1/4|b]]` under normal Lock/Access checks) and then detach `c1`.

### Scenario 4 — Cross-Workspace & Locked-Tree Coaction
Card `X` is filed in unlocked `agent` Literature tree `BookB` (`c_file = BookB/3`) and placed twice in Locked `human` Connection tree `~5` (`c_a = ~5/1`, `c_b = ~5/4`) and once in unlocked `human` Topic `2` (`c_t = 2/2`):
- **Edit `X` via `c_t` (Owner) or `BookB/3` (Agent):** Allowed (`X` is not a sealed member of `~5`; it is a pinned external source). Both `c_a` and `c_b` show advisory drift against their lock-time fingerprint. When the owner reviews `c_a` (`review_source`), `c_a` clears drift while `c_b` still asks for review!
- **Delete `BookB/3` or move `BookB/3 -> BookB/4`:** Refused (`card_locked`, `pinned-delete` / `pinned-move`, naming Lock `~5`).
- **Move or detach `c_t` in unlocked Topic `2`:** Allowed! Neither `X`'s filing address nor Locked `~5` changes.
- **Edit `c_a` gloss or detach `c_a` in Locked `~5`:** Refused (`~5` is Locked).
- **Agent tries to edit `X`'s text if `X` were filed in `human` Topic `2` and mounted in unlocked `agent` `~9`:** Refused (`owner_workspace = human`), even though the agent can freely move or gloss `X`'s Coordinate inside `~9`.

### Scenario 5 — Scholarly Structure Invariance & Explicit Refile
Model part `P` (`facet = "假设"`) is filed under Model Card `M1` at `BookA/4|a|1`. The user adds a Secondary Coordinate of `P` at `BookA/7|a|1` directly beneath Model Card `M2`:
- `get_model(M1)` and `get_model(M2)` return **byte-identical** JSON before and after placing the Secondary Coordinate. `P` remains a filed part of `M1` only (sharing `P` with `M2` still uses explicit `**引入**` imports).
- Similarly, placing a Secondary Coordinate of Term part `T_cont` as an immediate Side child of a Term list, or a Secondary Coordinate of Cited Card `C5` as a Side child of a Cited list, causes **zero** role changes and **zero** row-numbering shifts.
- Only an explicit `refile_card` changes `filing_coord_id` and re-evaluates Model/Term/Cited roles and the Write Gate.

---

## 5. SQLite Schema (Generation 2), Triggers & Compatibility Cache

### 5.1 Logical Schema (`coordinates_schema_version = '2'`, `WRITER_EPOCH = 2`)

```sql
-- 1. Unified Tree catalog (Literature, Topic, and Connection trees)
CREATE TABLE trees (
    tree_id         TEXT NOT NULL PRIMARY KEY,          -- immutable UUID (reuses connection_trees.tree_id)
    role            TEXT NOT NULL CHECK(role IN ('literature', 'topic', 'connection')),
    workspace       TEXT NOT NULL CHECK(workspace IN ('human', 'agent')),
    public_key      TEXT NOT NULL UNIQUE,               -- Citation key, '<n>', or '~<n>'
    display_num     INTEGER,                            -- topic number or connection display_num; NULL for literature
    root_card_id    TEXT NOT NULL UNIQUE REFERENCES card_identity(card_id),
    purpose         TEXT CHECK(purpose IS NULL OR (length(purpose) BETWEEN 1 AND 32
                        AND instr(purpose, char(10)) = 0 AND instr(purpose, char(13)) = 0)),
    next_local_seq  INTEGER NOT NULL DEFAULT 1 CHECK(next_local_seq >= 1),
    created_at      TEXT NOT NULL,
    UNIQUE(tree_id, workspace),
    CHECK(
        (role = 'literature' AND display_num IS NULL)
     OR (role IN ('topic', 'connection') AND display_num IS NOT NULL AND display_num >= 0)
    )
);
CREATE UNIQUE INDEX trees_role_num ON trees(role, display_num) WHERE display_num IS NOT NULL;

-- 2. Immutable Card identity + commit-enforced >= 1 Coordinate floor
CREATE TABLE card_identity (
    card_id         TEXT NOT NULL PRIMARY KEY,
    card_row        INTEGER UNIQUE REFERENCES cards(row_id),
    owner_workspace TEXT NOT NULL CHECK(owner_workspace IN ('human', 'agent')),
    status          TEXT NOT NULL CHECK(status IN ('live', 'retired')),
    filing_coord_id TEXT,                               -- non-null while live; NULL when retired
    last_address    TEXT NOT NULL,
    last_title      TEXT NOT NULL,
    retired_at_rev  INTEGER,
    CHECK(
        (status = 'live'    AND card_row IS NOT NULL AND filing_coord_id IS NOT NULL AND retired_at_rev IS NULL)
        OR
        (status = 'retired' AND card_row IS NULL     AND filing_coord_id IS NULL     AND retired_at_rev IS NOT NULL)
    ),
    FOREIGN KEY (filing_coord_id, card_id, owner_workspace)
        REFERENCES coordinates(coord_id, card_id, host_workspace)
        DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX card_identity_live_address ON card_identity(last_address) WHERE status = 'live';

-- 3. Permanent handles (unchanged semantics: '~7', '~7#4', never updated or deleted)
CREATE TABLE card_handles (
    handle          TEXT NOT NULL PRIMARY KEY,
    card_id         TEXT NOT NULL UNIQUE REFERENCES card_identity(card_id),
    origin_tree_id  TEXT NOT NULL,
    local_seq       INTEGER NOT NULL CHECK(local_seq >= 0),
    UNIQUE(origin_tree_id, local_seq)
);

-- 4. Universal Coordinates (NO UNIQUE(tree_id, card_id)!)
CREATE TABLE coordinates (
    coord_id        TEXT NOT NULL PRIMARY KEY,          -- immutable UUID (preserves existing placement_id)
    tree_id         TEXT NOT NULL,
    host_workspace  TEXT NOT NULL CHECK(host_workspace IN ('human', 'agent')),
    card_id         TEXT NOT NULL REFERENCES card_identity(card_id),
    parent_coord_id TEXT,
    edge            TEXT NOT NULL CHECK(edge IN ('root', 'direct', 'side')),
    slot            INTEGER,                            -- NULL for root/direct; explicit >= 1 for side (sparse allowed)
    gloss           TEXT CHECK(gloss IS NULL OR (length(gloss) BETWEEN 1 AND 240
                        AND instr(gloss, char(10)) = 0 AND instr(gloss, char(13)) = 0)),
    FOREIGN KEY (tree_id, host_workspace) REFERENCES trees(tree_id, workspace),
    FOREIGN KEY (tree_id, parent_coord_id) REFERENCES coordinates(tree_id, coord_id)
        DEFERRABLE INITIALLY DEFERRED,
    UNIQUE (tree_id, coord_id),
    UNIQUE (coord_id, card_id, host_workspace),         -- composite target for card_identity floor FK
    CHECK(
        (edge = 'root'   AND parent_coord_id IS NULL     AND slot IS NULL)
     OR (edge = 'direct' AND parent_coord_id IS NOT NULL AND slot IS NULL)
     OR (edge = 'side'   AND parent_coord_id IS NOT NULL AND slot IS NOT NULL AND slot >= 1)
    )
);
CREATE UNIQUE INDEX coordinates_one_root   ON coordinates(tree_id) WHERE edge = 'root';
CREATE UNIQUE INDEX coordinates_one_direct ON coordinates(tree_id, parent_coord_id) WHERE edge = 'direct';
CREATE UNIQUE INDEX coordinates_side_slot  ON coordinates(tree_id, parent_coord_id, slot) WHERE edge = 'side';
CREATE INDEX coordinates_by_card           ON coordinates(card_id);
CREATE INDEX coordinates_by_tree_card      ON coordinates(tree_id, card_id);
CREATE INDEX coordinates_by_parent         ON coordinates(tree_id, parent_coord_id);

-- 5. Tree-local assertions (generalized placement_edges; allows two distinct Coordinates of same Card)
CREATE TABLE coordinate_edges (
    connection_id   TEXT NOT NULL PRIMARY KEY,
    tree_id         TEXT NOT NULL REFERENCES trees(tree_id),
    from_coord_id   TEXT NOT NULL,
    to_coord_id     TEXT NOT NULL,
    direction       TEXT NOT NULL CHECK(direction IN ('neutral', 'directed')),
    relation_label  TEXT CHECK(relation_label IS NULL OR (
                        length(relation_label) BETWEEN 1 AND 32
                        AND instr(relation_label, char(10)) = 0 AND instr(relation_label, char(13)) = 0
                    )),
    relation_kind   TEXT CHECK(relation_kind IS NULL OR relation_kind IN (
                        'rests-on','illustrates','contrasts','relaxes',
                        'corresponds','objects','specializes','refers'
                    )),
    note            TEXT CHECK(note IS NULL OR length(note) BETWEEN 1 AND 400),
    created_by      TEXT NOT NULL CHECK(created_by IN ('owner', 'agent')),
    created_at_rev  INTEGER NOT NULL,
    updated_by      TEXT NOT NULL CHECK(updated_by IN ('owner', 'agent')),
    updated_at_rev  INTEGER NOT NULL,
    FOREIGN KEY (tree_id, from_coord_id) REFERENCES coordinates(tree_id, coord_id) DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (tree_id, to_coord_id)   REFERENCES coordinates(tree_id, coord_id) DEFERRABLE INITIALLY DEFERRED,
    CHECK(from_coord_id <> to_coord_id),
    CHECK(direction = 'directed' OR from_coord_id < to_coord_id),
    CHECK(
        relation_kind IS NULL
        OR relation_kind IN ('contrasts', 'corresponds', 'refers')
        OR direction = 'directed'
    )
);
CREATE UNIQUE INDEX coordinate_edges_blank_pair
    ON coordinate_edges(tree_id, from_coord_id, to_coord_id)
    WHERE direction = 'neutral' AND relation_label IS NULL AND relation_kind IS NULL AND note IS NULL;
CREATE INDEX coordinate_edges_by_from ON coordinate_edges(tree_id, from_coord_id);
CREATE INDEX coordinate_edges_by_to   ON coordinate_edges(tree_id, to_coord_id);

-- 6. Retired Coordinate IDs (preserves existing retired_placements for durable place:<uuid> diagnostics)
CREATE TABLE retired_coordinates (
    coord_id        TEXT NOT NULL PRIMARY KEY,
    tree_id         TEXT NOT NULL,
    card_id         TEXT NOT NULL REFERENCES card_identity(card_id),
    retired_at_rev  INTEGER NOT NULL
);
```

### 5.2 Lock Evidence Versioning, Triggers & Compatibility Cache
- **Lock evidence tables (`evidence_version IN (1, 2)`):**
  - Existing v1 rows (`lock_coordinate_seals` with `evidence_version = 1`, `lock_placements`, `lock_connections`, `lock_placement_pins`, `lock_source_reviews`) remain **byte-for-byte intact** and are verified under the v1 verifier projection (so a v1 Topic Lock with no mounts never falsely faults).
  - New Locks write `evidence_version = 2` sealing the locked Tree's complete `coordinates` rows (by `coord_id`), `coordinate_edges` rows, and per-`coord_id` external pins + fingerprints (`PRIMARY KEY(lock_id, coord_id)`).
- **SQL Lock triggers (dual `OLD`/`NEW` coverage):**
  - `BEFORE INSERT/UPDATE/DELETE` on `coordinates` refuses whenever `OLD.tree_id` OR `NEW.tree_id` has an active Lock (preventing both mutations inside a locked tree and sneaking a Coordinate into or out of one).
  - `BEFORE INSERT/UPDATE/DELETE` on `coordinate_edges` and `UPDATE/DELETE` on `trees` refuse when the host tree has an active Lock.
  - `BEFORE UPDATE OF filing_coord_id` on `card_identity` refuses when the Card is a sealed member of an active Lock or a pinned external target (`pinned-move`).
  - `BEFORE DELETE` on `cards` refuses when pinned (`pinned-delete`) or locked.
- **Writer fence upgrade (`WRITER_EPOCH = 2`):**
  - Unlike 0.70.1's `CREATE TRIGGER IF NOT EXISTS` ([coord.rs](file:///e:/zet/src/coord.rs#L449)), migration **explicitly drops every `writer_epoch_*` trigger** and installs fresh `WHEN zt_writer_epoch() IS NOT 2` triggers on all application tables inside the migration transaction.
- **Synchronized compatibility columns on `cards`:**
  - `cards.location` / `cards.citation_key` always mirrors the Card's canonical address (`Hart1995/2`, `3/2|a`, `~7`, `~7#4`) derived from `filing_coord_id` (and `card_handles` for Connection-homed Cards), updated atomically inside the same write transaction. Legacy read paths remain fast; there is only ONE write planner.

---

## 6. Address, Coordinate, Link & Bookmark Grammar

### 6.1 Syntax Table

| Spelling | Kind | Mutable? | Valid in `[[...]]`? | Semantics |
|---|---|---|---|---|
| `Hart1995`, `Hart1995/2\|a` | Canonical Literature filing address | On Citation-key rename / move | **Yes** | Canonical citation of a Literature-filed Card. |
| `3/0`, `3/2\|a` | Canonical Topic filing address | On Topic move / refile | **Yes** | Canonical citation of a Topic-filed Card. |
| `~7`, `~7#4` | Permanent Connection handle | **Never** | **Yes** | Permanent citation of a Connection-issued Card; survives reorders, refiling into a Topic, and tree deletion. |
| `~7/0`, `~7/2\|a` | Connection Coordinate path | On reorder | **No (refused)** | Navigation path of a Coordinate in `~7`. |
| `3/5\|b` *(when Secondary Coordinate of `Hart1995/2`)* | Secondary Coordinate path in Topic/Literature | On reorder / detach | **No (refused)** | Navigation path only. Refused inside `[[...]]` with a hint naming `[[Hart1995/2]]` and `[label](place:<coord_id>)`. |
| `[label](place:<coord_id>)` | Durable Coordinate bookmark | **Never** (follows `coord_id`) | Markdown link (not `[[...]]`) | Opens that exact occurrence after reorders; opens a Missing coordinate if target retired; reports removed if detached. |

### 6.2 Link Rules & Why `[[card:<uuid>]]` Is Deferred
- **Zero Markdown rewrites on Secondary Coordinate edits:** Because Secondary Coordinate paths are rejected inside `[[...]]`, moving, reordering, or detaching a Secondary Coordinate NEVER rewrites any Card body, NEVER touches a human Card on an agent write, and NEVER triggers `ensure_link_rewrites_allowed`.
- **Canonical refiling uses proven rewrite safeguards:** When `refile_card` changes a Card's Filing Coordinate (e.g., `3/2|a -> 3/5`), the resulting mapping runs through `apply_location_mapping_tx`, rewriting literal `[[3/2|a]]`, Term targets, and `chart_links` under the existing human-Workspace and Lock gates (while permanent handles like `[[~7#4]]` need zero rewrites).
- **Why no `[[card:<uuid>]]` in v1:** Every live Card already has one short, human-readable canonical citation (`Hart1995/2`, `3/2|a`, `~7`, `~7#4`). Introducing 36-character UUIDs into authored Markdown degrades human/Session readability and forces unnecessary churn across `zt-check`, `zt-digest`, and digest prompts without solving any blocking invariant.

---

## 7. Coordinate Lifecycle & Transactional Operation Contracts

Every write runs under the archive edit lock inside one `BEGIN IMMEDIATE` transaction, checks Workspace/Access, Write Scope, and Locks before mutating, advances `archive_revision` once on commit, and supports `op_key` idempotency (`applied_ops`) plus `preview: true` rollback simulation. Every destructive or reader-visible change returns a two-step plan bound by `(archive_id, expected_revision, plan_hash)`.

| Operation | Inputs & Target | Plan / Safeguards / Transactional Effects |
|---|---|---|
| **Create Card (`create_topic`, `create_literature`, `create_successor`, `create_connection_card`)** | Parent `coord_id` (or root), Card text & metadata | Centralized creator allocates `card_id` + `coord_id`, inserts `cards`, `card_identity(filing_coord_id = coord_id)`, and `coordinates` atomically. Runs Write Gate, Scope, and Lock checks. |
| **Add Coordinate (`add_coordinate`, `add_coordinates`)** | `target` (`card_id` \| address \| `coord_id`) or ordered `targets: [X, Y, X]`, `tree`, `parent_coord_id`, `edge`, optional `before_coord_id` / `slot`, optional `gloss` | Always allocates fresh `coord_id`(s) even if `X` already appears in `tree` or on `parent_coord_id`'s ancestor chain. Copies no text, children, or assertions. Checks host Workspace/Lock, Literature foreign-Card rule (`MC3`), `model`/`term` splice guard (`MC4`), gloss review if displacing a Direct child, and (if splicing before a Topic/Literature Filing Coordinate) normal move/Link-rewrite mapping. Legacy `place_card` keeps its set-dedup contract for backward compatibility. |
| **Move / Insert / Reorder Coordinate (`move_coordinate`, `reorder_coordinates`)** | Source `coord_id`(s), destination `(tree, parent_coord_id, edge, before?)`, `mode: subtree \| alone \| span` | Checks `coord_id` acyclicity (refuses moving `c` beneath a descendant of `c`), host Workspace/Lock, splice guard, and gloss review for every Coordinate whose structural parent changes. Preserves `coord_id`s, `place:` bookmarks, and internal `coordinate_edges`. Cross-tree moves require explicit disposition for boundary `coordinate_edges`. If any Topic/Literature Filing Coordinate's path changes, computes and checks the exact Link-rewrite mapping. |
| **Refile Card (`refile_card`)** | `card_id` (or address), `to_coord_id` (an existing Coordinate of that Card in `owner_workspace`) | Changes `card_identity.filing_coord_id = to_coord_id` (generalizing `promote_card`). Refuses cross-Workspace targets, Locked/pinned moves, and Literature root refiling. Runs Write Gate in the new filing context; rewrites canonical address & literal Links if needed (permanent handles `~n#k` remain aliases). All other Coordinates, glosses, and `coordinate_edges` stay intact. |
| **Detach Coordinate (`detach_coordinate` / `unplace`)** | `coord_id`, optional `children: detach \| reattach`, optional `reattach_to`, optional `replacement_filing_coord_id` | Computes the complete removed `coord_id` set, groups by `card_id`, and checks each Card's **post-operation surviving Coordinates**: (a) if `filing_coord_id` stays outside the removed set → ordinary detach; (b) if `filing_coord_id` is removed and a valid same-Workspace `replacement_filing_coord_id` is supplied → atomic refile + detach; (c) if **zero** owner-Workspace Coordinates would survive → **refused** (must use explicit Delete Card). Retires removed `coord_id`s and deletes incident `coordinate_edges`. |
| **Delete Coordinate Subtree / Delete Tree** | Root `coord_id` of subtree (or Tree root `coord_id`), optional `--keep-root` | Enumerates removed `coord_id`s first, groups by `card_id`, and classifies every Card once into: **Survives** (filing outside deleted set), **Refile required** (filing inside deleted set, another same-Workspace Coordinate survives), or **Delete Card** (zero surviving same-Workspace Coordinates — requires full Card-delete confirmation, Link-break/`into` review, Lock/pin clearance, and converts surviving foreign-Workspace Coordinates to Missing coordinates). Deleting a defining root ALWAYS requires the tree-level plan; detaching a Secondary Coordinate of a root Card is an ordinary detach. |
| **Delete Card Globally (`delete_card`)** | `card_id` (or canonical address), optional `into` | Identity-level delete. Checks owner Workspace, Lock/pin gates (`pinned-delete`), removes owned Coordinates in scope, retires `card_identity` (`status = 'retired'`, `filing_coord_id = NULL`), and leaves Coordinates in surviving foreign hosts as **Missing coordinates** (never mutating foreign-Workspace trees). `plan_hash` covers `coord::delete_state` (Card texts, removed/missing Coordinates, glosses, and incident `coordinate_edges`). |
| **Rebind Missing Coordinate (`rebind_coordinate`)** | Missing `coord_id`, replacement `target_card` | Checks host writability & Literature admission; forces gloss review and explicit per-connection disposition for every incident `coordinate_edges` row (old assertions never silently transfer); issues a fresh `coord_id` and retires the old one. |

---

## 8. Subsystem Coaction & Bug-Prevention Matrix (All 10 Subsystems)

### 8.1 Links, Term Links, Charts, Reverse Links & Broken Links (`src/link.rs`, `src/link_index.rs`, `src/card.rs`)
- Links target Cards by canonical filing address or handle (`[[Hart1995/2]]`, `[[3/2|a]]`, `[[~7#4]]`), never Secondary Coordinate paths.
- Secondary Coordinate edits cause **zero** Link rewrites. Filing renames rewrite ordinary Links, Term-link targets, and `chart_links` from the exact old-to-new mapping, preserving existing broken-target non-hijack safeguards (skipping compaction when a broken Link would land on a renamed sibling).
- Reverse links and Term uses deduplicate by `(source_card_id, form)` regardless of coordinate multiplicity. `zt status --check-links` and `zt reindex` continue operating over authored bodies and canonical aliases.

### 8.2 Workspaces (`human` vs `agent`), Access & Write Scope (`src/workspace.rs`, `src/scope.rs`)
- Split capabilities on every read: `can_edit_card` (owner Workspace + Card Lock) vs `can_edit_coordinate` / `can_edit_tree` (host Workspace + host Tree Lock).
- Removing a Card's last owner-Workspace Coordinate while a foreign-Workspace Coordinate survives is refused by planner AND SQLite composite FK—preventing silent ownership transfer.
- `WriteScope` resolves handles and Coordinates to `card_id` before scope checks, keeps Literature-scoped MCP connections barred from free-coordinate writes and Topic/Hub writes, and runs `ensure_no_connection_rewrite` on the real address mapping.

### 8.3 Tree Locks, Pins, Seals & Triggers (`src/tree_lock.rs`, `src/coord_lock.rs`)

Operation trace for Card `X` present at `c1` in **Locked Tree `A`** and `c2` in **Unlocked Tree `B`**:

| Operation | Required Behavior |
|---|---|
| **Edit `X`'s text through `c2`** | **Refuse** if `X` is a sealed member of `A` (`X.filing_coord_id` in `A`). **Permit** (if authorized on `X`'s owner Workspace) if `X` is only a pinned external source in `A`, reporting advisory drift per locked occurrence (`c1`). If `A` holds two occurrences `c1a, c1b` of `X`, reviewing `c1a` (`review_source`) does NOT mark `c1b` reviewed. |
| **Add `c3` of `X` in Unlocked `B`** | **Permit** (requires `B` host authority only; `X` and `A` are untouched). |
| **Move or detach `c2` in Unlocked `B`** | **Permit** if `c2` is a Secondary Coordinate. **Refuse (`pinned-move` / `pinned-delete`)** if `c2` is `X`'s Filing Coordinate pinned by `A` and the change alters `X`'s promised address or retires `X`. |
| **Delete Unlocked `B`** | **Permit** removal of Secondary Coordinates in `B`; **refuse** if the delete plan would retire or change the promised filing address of a pinned `X`. |
| **Move, detach, rebind, or edit gloss/edges of `c1` in Locked `A`** | **Refuse** (enforced by planner and SQL triggers checking both `OLD.tree_id` and `NEW.tree_id`). |
| **`zt status --check-locks`** | Verifies v1 Locks under the v1 projection (preserving v1 Topic Locks without `lock_coordinate_seals`) and v2 Locks against the full `coordinates` + `coordinate_edges` + per-`coord_id` pins seal. Advisory source drift never triggers a Lock integrity fault. |

### 8.4 Models & Math Units (`src/model.rs`, `src/math.rs`, `src/model/formal.rs`, `src/model/empirical.rs`)
- Resolve any input Coordinate to `card_id`, then compute `find_head`, `heads`, `parts_of`, `in_block` (Side-nested block rule), `build_report`, formal/empirical role checks, `unit_of`, `theory_report`, `reach_in`, and `study` strictly over **Filing Coordinates** and authored Card Links.
- Displaying a Model part under a second Model head `M2` never enrolls it into `M2`; cross-model sharing remains explicit via `**引入**` imports.
- Byte-identical Model and Math reports before and after adding arbitrary Secondary Coordinates.

### 8.5 Term Cards (`src/term.rs`) & Cited Cards (`src/cited.rs`)
- `term::place` and `cited::structure` walk **Filing Coordinates only**.
- A Secondary Coordinate of a Term Card under a Term list never becomes a Term head (and cannot satisfy a Term link `[[label@addr]]`); a Secondary Coordinate of a Cited Card beside a Cited list consumes zero bibliography rows and triggers zero faults.
- Only `refile_card` changes a Term or Cited Card's structural role, re-running role/row/use validation.

### 8.6 Hub Cards, Concept Register, Search, Distant Card, Ask-Box & Folding (`src/hub.rs`, `src/entry.rs`)
- Hub range (`line`, `topic`, `field`, `register`), coverage (`covered`, `reached`, `unreached`), Register part detection, `source_evidence`, and `distant` filing-neighbour exclusions evaluate over **Filing Coordinates** and distinct **Cards (`card_id`)**.
- Placing one Card at 3 Coordinates in a Topic tree counts that Card once and adds zero extra `source_evidence` or Hub lines.
- `entry::search` ranks distinct Cards once (exposing their Coordinates as metadata) and preserves `total` and `row_total` restatement folding. `ask_box` walks authored semantic Links by `card_id`; Secondary Coordinates and `coordinate_edges` never enter the walk.

### 8.7 Write Gate & `zt-check` (`src/gate.rs`, `tools/zt-check/`)
- Editing a Card's text through any Coordinate builds `gate::Candidate` in its **Filing Coordinate context** (a Literature note viewed in a Topic keeps Literature-note rules; a Topic Idea Card viewed in a Connection tree keeps Topic I1–I6 rules).
- `topic_exists` runs only on real agent Topic root creation, never on Secondary Coordinates.
- Secondary Coordinate placement/move runs host, slot, Lock, splice-guard, and gloss checks without re-running Card prose rules.

### 8.8 Coordinate Views (`elsewhere`, `crossings`, Dependency Lens, Seeding, Extract)
- **`elsewhere`:** Lists **all other Coordinates** of the Card—including sibling Coordinates in the **same** Tree—with `{coord_id, tree, path, is_filing, gloss}`.
- **`crossings` (Projection):**
  - Keyed by **`coord_id`**, never by `address` alone.
  - Preserves lines where `from_card_id == to_card_id` but `from_coord_id != to_coord_id` (removing the silent `line.from === line.to` drop at [connections.js](file:///e:/zet/src/gui/connections.js#L340)).
  - When Card `X` also appears at multiple Coordinates in the viewed plane `T`, `stations` reports the target `coord_id` set on `T` and the GUI groups/selects endpoints cleanly without fabricating an N×M Cartesian explosion of lines.
  - `exits` is keyed per `coord_id` (with optional Card-level summary).
- **Dependency Lens & Seeding:** Authored closure stays Card/unit-level; overlay assertions carry exact `coord_id` endpoints and never create authored dependency cycles. `seed_connection_tree` keeps unique-Card seeding for Hub/search/Lens sources and adds an ordered-occurrence mode when seeding from an outline with intentional repeats.
- **`extract_branch`:** Copies each `coord_id` in the coordinate-subtree with a fresh `coord_id` (preserving repeated Cards in the branch!), remaps internal `coordinate_edges`, leaves boundary edges in source, and reviews glosses.

### 8.9 GUI, Terminal Session, MCP, Batches, Spreads, Dump & Portable Export
- **`Snapshot` (`src/ops.rs`) & GUI (`src/gui/app.js`, `layout.js`, `connections.js`):**
  - Separate **Card index** (`byCardId` / `byAddress` for canonical citations) from **Coordinate index** (`byCoordId`, per-tree outline keyed by `coord_id`).
  - Map DOM nodes, focus, selection, expansion, and drag/move target **`coord_id`**. Repeated Cards in one Map render as distinct nodes with a repeat badge (`×N in this tree`), independent children, and live-synced content edits.
  - `places` counts `COUNT(*) FROM coordinates WHERE card_id = ?1` uniformly across all Trees.
  - Authored Link arcs stay Card-level (drawn to the focused/nearest occurrence or grouped badge to avoid hairballs); `coordinate_edges` connect exact `coord_id` pairs.
- **Terminal Session (`src/session.rs`, `src/session/station.rs`):** Unify `Pointer` to `(card_id, coord_id)` across all Trees; stale history after a detach reports `coordinate removed` and offers the surviving Filing Coordinate.
- **MCP (`src/mcp.rs`) & Batches (`src/batch.rs`):** Existing Card/digest tools keep Filing-Coordinate semantics (`get_tree` returns one row per filed Card by default). New Coordinate tools (`list_coordinates`, `get_tree_coordinates`, `add_coordinate`, `add_coordinates`, `move_coordinate`, `detach_coordinate`, `refile_card`) operate explicitly by `coord_id`.
- **Spreads (`src/spread.rs`):** Stay Card-keyed (optionally remembering a focused `coord_id`); detaching a Secondary Coordinate never drops a surviving Card from a Spread.
- **Dump (`zt dp`) & Portable Export (`zt export`):** `zt dp` serializes each Card body once plus `coordinates.json` v2 (`trees`, `card_identity`, `card_handles`, `coordinates`, `coordinate_edges`, `retired_coordinates`, Lock coordinate evidence) from one consistent read transaction. `zt export <tree>` writes Portable v4 with deduplicated Card bodies (preserving each Card's `owner_workspace`), full `coordinates` list (supporting repeated `coord_id`s), and remapped `place:<coord_id>` bookmarks.

### 8.10 Digest Workflows & Companion Binaries (`zt-check`, `zt-digest`)
- `zt-econ-digest`, `zt-math-digest`, `zt-idea-grow`, and `zt-post-digest` keep exact Filing-Coordinate compatibility on `get_tree`, `get_card`, `apply_batch`, raw Term macros, Write Scope, and `archive_revision`.
- **Important verification note:** `tools/zt-digest/src/calls.rs` maintains its own authored-fingerprint calculation; qualification MUST verify parity with both `zt` and `zt-digest` (and check the pre-existing `zt-econ-digest.js` manifest lock status documented in [connection-trees-free-proposal.md](connection-trees-free-proposal.md#L154) before attributing any failure to coordinates).

---

## 9. Verified Refactoring Checklist (16 Codebase Sites)

Merely dropping `UNIQUE(host_tree_key, card_id)` would introduce subtle bugs across all 16 verified sites below; they MUST be refactored together:

| # | File & Line | Existing Assumption / Hazard | Required Change |
|---|---|---|---|
| 1 | [coord.rs:312](file:///e:/zet/src/coord.rs#L312) | `UNIQUE (host_tree_key, card_id)` | Replace with unified `coordinates` table + non-unique `coordinates_by_tree_card` index. |
| 2 | [coord.rs:379](file:///e:/zet/src/coord.rs#L379) | `identity_after_insert` trigger cannot satisfy non-deferrable `filing_coord_id IS NOT NULL` CHECK | Retire trigger; route all Card creation through one centralized transactional Card+Coordinate creator. |
| 3 | [coord.rs:435-458](file:///e:/zet/src/coord.rs#L435-L458) | `writer_fence_sql` uses `CREATE TRIGGER IF NOT EXISTS` with embedded epoch literal | Explicitly `DROP TRIGGER IF EXISTS` on old epoch-1 triggers before installing epoch-2 triggers. |
| 4 | [coord.rs:1031](file:///e:/zet/src/coord.rs#L1031) | `TreeShape::placement_of_card` returns `Option<&Placement>` (first match only) | Replace with `coords_of_card(&self, card_id) -> Vec<&Coordinate>` and explicit `filing_coord_id` lookup. |
| 5 | [coord.rs:1098](file:///e:/zet/src/coord.rs#L1098) | Enforces dense `1..N` Side ordinals; fails on 303 sparse legacy sibling groups | Store explicit positive `slot`; derive Side label from `slot`, preserving gaps. |
| 6 | [coord.rs:1449](file:///e:/zet/src/coord.rs#L1449) | `delete_impact_lines` skips same-home-host occurrences | Compute per-`card_id` post-operation survivor sets across all `coord_id`s. |
| 7 | [coord.rs:1516](file:///e:/zet/src/coord.rs#L1516) | `follow_anchor_moves` enforces one-Card-per-Topic `+k` collision rules | Retire `+k` shelves (`MC1`); unify moves on `coordinates`. |
| 8 | [connection.rs:532](file:///e:/zet/src/connection.rs#L532) | `crossings_of` aggregates exits by `address` and maps `placement_id -> address` | Key `crossings` stations, lines, and exits by `coord_id`. |
| 9 | [connection.rs:629,642,2770](file:///e:/zet/src/connection.rs#L629) | `origin: "native"` flags all repeats; `places` omits home coordinate; `tree_summary` miscounts | Distinguish `is_filing` vs Secondary; count all rows in `coordinates`; report distinct Cards vs Coordinates. |
| 10 | [connection.rs:841](file:///e:/zet/src/connection.rs#L841) | `validate_tree` checks `EXISTS (... card_id)` instead of `filing_coord_id` | Verify `card_identity.filing_coord_id` matches a live Coordinate in `owner_workspace`. |
| 11 | [connection.rs:1354,1451,1587,2300](file:///e:/zet/src/connection.rs#L1354) | `place`, `place_many`, `attach`, `rebind` suppress or refuse duplicates | Keep legacy `place_card` adapter; add `add_coordinate` / `add_coordinates` / `rebind_coordinate` permitting repeats. |
| 12 | [connection.rs:2094](file:///e:/zet/src/connection.rs#L2094) | `unplace` `home_here` blocks detaching ANY occurrence in native tree | Protect only `coord_id == card.filing_coord_id` (requiring replacement filing or Card delete). |
| 13 | [connection.rs:2708](file:///e:/zet/src/connection.rs#L2708) | `promote` finds and detaches first Topic mount | Generalize to `refile_card(card_id, to_coord_id)` leaving other Coordinates intact. |
| 14 | [dependencies.rs:517,678](file:///e:/zet/src/dependencies.rs#L517) | Overlay drops `coord_id` granularity; seed unconditionally deduplicates | Preserve `coord_id` on overlay evidence; add ordered-occurrence seed mode. |
| 15 | [connections.js:340](file:///e:/zet/src/gui/connections.js#L340) & [app.js](file:///e:/zet/src/gui/app.js) & [portable-rules.js:369](file:///e:/zet/src/gui/portable-rules.js#L369) | `line.from === line.to` drops equal-Card lines; `state.byAddress` and first-match jumps break on repeats | Key Map nodes, focus, selection, crossings, and portable lookup by `coord_id`. |
| 16 | [main.rs:1482](file:///e:/zet/src/main.rs#L1482) | `initialize_database` omits `PRAGMA foreign_keys = ON` | Enable `PRAGMA foreign_keys = ON` in both `initialize_database` and `open_db`. |

---

## 10. Design Review & Audit Record (Astra + Gemini)

### Astra Consultation (`gpt-6.2-astra`, high effort, session `01a22099-e605-7993-ab29-1117f08cbba1`)
- Diagnosed the root asymmetry of 0.70–0.71 (`cards.location` vs `placements` + `UNIQUE(host_tree_key, card_id)`).
- Audited `D:\zt` live (`62,322` Cards, `0` active placements, **`303` sparse Side sibling groups**, `20` handles, `122` retired placements, `166,710` receipts).
- Designed and verified on SQLite 3.45.1 the commit-enforced `>= 1` floor pattern (`DEFERRABLE INITIALLY DEFERRED` composite FK on `(filing_coord_id, card_id, owner_workspace)`).
- Identified the `writer_fence_sql` `CREATE TRIGGER IF NOT EXISTS` upgrade trap, the v1 Lock `coordinate-evidence-missing` hazard, the 16 embedded single-occurrence sites, and the 8 release-blocking acceptance cases.

### Gemini Independent Audit & Design Refinements Incorporated
Every code reference in Astra's memo was verified against `E:\zet`, and 6 architectural refinements were incorporated into this final proposal:
1. **Eliminated dual-meaning path collisions (`MC4`, §3.2 #2):** Rejected Astra's tentative allowance of diverging reading-outline paths vs filing addresses sharing the same string (e.g. `1/2` meaning two Cards in Topic 1); replaced with the **Single-Coordinate-Space Invariant**.
2. **Eliminated parallel "editorial parent" graph (`MC4`, §3.2 #6):** Replaced a second stored parent graph with Filing-Coordinate filtering across `model`/`math`/`term`/`cited`/`hub` plus the narrow `model`/`term` filing-chain splice guard.
3. **Resolved SQLite trigger deadlock & `initialize_database` pragma (§3.2 #1, §9 #2 & #16):** Caught that [coord.rs:379](file:///e:/zet/src/coord.rs#L379) `identity_after_insert` would fail a non-deferrable `CHECK(filing_coord_id IS NOT NULL)` and that [main.rs:1482](file:///e:/zet/src/main.rs#L1482) lacked `PRAGMA foreign_keys = ON`.
4. **Retired Topic `+k` shelves cleanly (`MC1`, §3.2 #5):** Leveraged the verified `0`-placement state of `D:\zt` to retire `EdgeKind::Attach` instead of carrying a fourth edge kind indefinitely.
5. **Kept human-readable canonical citations without `[[card:<uuid>]]` (§6.2):** Avoided injecting 36-char UUIDs into human Markdown and digest tools while keeping Secondary Coordinate edits 100% rewrite-free.
6. **Fixed `places` count and `crossings` Cartesian/self-line edge cases (§8.8–8.9):** Unified `places` over `coordinates` and keyed `crossings` cleanly by `coord_id`.

---

## 11. Five-Phase Implementation, Zero-Data-Loss Migration & Acceptance Tests

### Phases (`P1`–`P5`)

- **`P1` — Freeze Observable Contracts & Build Fixtures:** Snapshot a copy of `D:\zt` (62,322 Cards, 303 sparse Side groups) plus synthetic fixtures for active v1 Literature/Topic/Connection Locks (including a mount-free v1 Topic Lock), Missing coordinates with children & assertions, promoted handles of deleted trees, and human Cards with chart-only Links. Record golden report digests (including `zt-digest` fingerprints).
- **`P2` — Writer Epoch 2 & Atomic Migration:** Under edit lock, create and verify a `VACUUM INTO` backup. Enable `PRAGMA foreign_keys = ON` in `initialize_database` and `open_db`. Gate startup on `coordinates_schema_version = '2'` before any legacy repair path. Explicitly drop old epoch-1 triggers and install epoch-2 triggers (`WRITER_EPOCH = 2`). Materialize `trees` and `coordinates` preserving exact sparse `slot` values (zero compaction, zero address changes), reuse existing `placement_id`s, bind `card_identity.filing_coord_id`, and preserve v1 Lock evidence byte-for-byte. Verify old 0.70/0.71 binaries fail cleanly on every write.
- **`P3` — Unified Core Engine (`coord_id`-First):** Implement unified Coordinate reads/writes, centralized Card+Coordinate creation, survivor-set detach/delete planner, `refile_card`, `model`/`term` splice guard, v2 Lock evidence & dual `OLD`/`NEW` triggers, and route all legacy filing/digest endpoints through the single transactional planner (no split-brain dual writers).
- **`P4` — GUI, Session, MCP, Dump & Portable Export:** Upgrade GUI Map/Reader/Crossings to `coord_id` keying with repeat badges (`×N`), "Add coordinate here", "Remove this coordinate", "Refile here", and "Also at..." (including same-tree siblings); unify Session `Pointer` to `(card_id, coord_id)`; ship MCP Coordinate tools; upgrade `zt dp` (`coordinates.json` v2) and `zt export` (Portable v4).
- **`P5` — Full-Scale & Workflow Qualification:** Run all 8 release-blocking acceptance cases below, replay `zt-econ-digest`, `zt-math-digest`, `zt-idea-grow`, and `zt-post-digest` on staged copies, verify `zt dp` reconstruction and Portable v4 offline navigation of repeated occurrences, and benchmark startup, tree read, projection, reindex, and dump times at 62k-Card scale.

### Release-Blocking Acceptance Suite (8 Mandatory Cases)

1. **Same-tree repeat & self-ancestry:** Create `X(c1) -> Y(c2) -> X(c3)` plus `X(c4)` on a Side branch with distinct children, glosses, and assertions. Move `c3`, reopen `place:<c3>` and `place:<c4>`, detach `c3`, and verify surviving children, Card text, and assertion endpoints match exactly.
2. **Root multiplicity:** Use one Literature root as root, as a non-root occurrence beneath itself, and as a non-root occurrence in a Topic and a Connection tree. Detach the non-root occurrences and verify defining root, BibTeX, root Lock, and tree Workspace remain intact.
3. **Commit-enforced `>= 1` floor & concurrency:** Attempt concurrent and batched removal of a Card's last two Coordinates, removal of its last owner-Workspace Coordinate while a foreign Coordinate survives, and tree deletion covering all Coordinates of a Card. Verify exact rollback of revisions, indexes, coordinates, and receipts on refusal.
4. **Locks & per-occurrence drift:** Place `X` twice in a locked host as a foreign source; edit `X` and review only one occurrence; verify distinct drift/review states. Repeat with `X` as a sealed member and verify refusal across GUI, MCP, Session, and CLI.
5. **Scholarly invariance:** Place a Model part at a Secondary Coordinate beneath a second Model head, a Term continuation beneath a Term list, a Cited Card beside a Cited list, and a Hub member three times in a Topic tree. Require byte-identical Model, Math, Term, Cited, Hub, and `zt-check` outputs; then perform an explicit `refile_card` and verify the expected semantic updates.
6. **Multi-occurrence Projection (`crossings`):** Place `X` twice in each of two crossing trees and twice in a viewed tree. Verify parallel assertions, equal-Card/different-coordinate lines, Missing hops, and per-Coordinate exits render deterministically with no dropped lines and no Cartesian explosion.
7. **Idempotency & legacy replay:** Replay a historical `op_key` no-op receipt after migration; then submit two intentional `add_coordinate` calls for `X` with distinct keys and retry each, verifying exactly two new Coordinates with stable IDs and exact historical replay.
8. **Live-scale migration & portability:** Migrate a full copy of `D:\zt` (`62,322` Cards, `303` sparse Side groups), verify matching live/retired identities, authored bytes, handles, preserved sparse paths, pre-existing Lock history, clean `zt status --check-links` and `zt status --check-locks`, verified backup restore, `zt dp` round-trip, Portable v4 offline navigation, and acceptable latency at scale.


---

## 12. Implementation & Verification Status (2026-11-28)

- **Owner:** Xingtong Chen
- **Implemented Phases:** `P1`, `P2`, `P3`, `P4`, and `P5` complete in `E:\zet`.
- **Verification Summary:**
  - Full `zt` unit & integration test suite (`cargo test --all-targets`): **100% passing (0 failures)** across 523 binary unit tests and all ~40 integration test binaries, including new TDD suites `tests/multi_coord_storage.rs`, `tests/multi_coord_ops.rs`, `tests/multi_coord_locks_and_coaction.rs`, `tests/multi_coord_surfaces.rs`, and `tests/multi_coord_acceptance.rs`.
  - Companion crates: `tools/zt-check` (223/223 passed); `tools/zt-digest` (1,004/1,006 passed — the only 2 failures are pre-existing plain-SQLite tamper fixtures in `term_link_apply.rs` correctly blocked by the writer-epoch fence).
  - **Live `D:\zt` Migration & Post-Migration v2-Only Cleanup:**
    - Migrated live `D:\zt\zt.sqlite3` cleanly from v1 → v2 (`62,322` live Cards, `502,021` Links `ok: true`, `coordinates: ok`, `locks: integrity ok`, revision `148217`, verified 590 MB backup `D:\zt\zt.sqlite3.pre-coordinates-v2-1795853474833707500.bak`).
    - Removed all v1 structures, v1 Lock compatibility (`project_v1`, `evidence_version = 1`, `coord_v2_min_lock_id`), `TopicMount` `+k` parser, and v1 → v2 migration code (`upgrade_v1_to_v2_if_needed`, `widen_lock_seal_version`), leaving a clean v2-only binary (`check_generation` rejects generation `1` and unknown future generations untouched) with zero warnings in `src/`.
