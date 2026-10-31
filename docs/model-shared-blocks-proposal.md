# Shared Building Blocks Between Models: Proposal

Status: **accepted** (2026-10-30). The owner accepted the recommended answer to each of the five decisions (see "Decisions"). Nothing in this document is implemented yet. The GUI part that could be done without a model change (the 共用 shelf and keeping the borrowing model in view) shipped in zt 0.47.0 (commit `55a3de0`); see "GUI" below.

## Summary

A model today can borrow another model's building block only **one Card at a time**, and only through **any Link its Model Card writes**. A building block is rarely one Card. A setting is a dependency tree, a result comes with its proof chain, and a chapter comes with its results. When a model borrows one Card of such a block, the rest of the block stays behind as "tolerated" premises, and the model is no longer self-sufficient. The newer tools (the one-conversation model compiler, `zt-model-fold`, `zt-model-settings`, `zt-model-restructure`) then treat the borrowing as a defect. They fold the two models together, or they make the borrower restate the setting and its symbols. The result is that models copy their building blocks instead of sharing them.

This proposal makes a building block a **subtree with an interface**, and makes sharing an **explicit import** (`**引入**`) on the borrowing Model Card. An import brings the whole block and everything the block rests on in its home model. The borrower can name the block's symbols in its own notation, and it can specialize a borrowed Card instead of copying it. Every tool in the pipeline then reads an import as intended structure, not as a split model.

## The problem

### What the owner sees

"The latest model card module cannot use other models' building blocks; they cannot share the building blocks between models." The live box confirms this in three ways:

1. **A share of one Card, then a copy of the rest.** `hart1995firms/15|c` (model 2) borrows `14|a|a`, the setting of model 1 (`- 共用 [[hart1995firms/14|a|a]]（共用）`). Hart builds model 2 on the same three dates, the same priority debts and the same default rule. But the restructure filed model 1's whole setting and main line under `14|a|a`, and an import takes one Card. So `15|c|a`, the setting of model 2, restates `y_1, y_2, L, P_1, P_2, R` and `π^A/π^B`, and it points back at `14|a|a` with `参照` (corresponds), not with a rests-on Link.
2. **Ten single Cards and sixteen premises left behind.** `grossman2002integration/7|a` lists ten `共用` lines, each one Card from models `2|a`, `3|a`, `4|a` and `5|a`. The build report lists 16 `tolerated` premises: Cards that those ten rest on, which stay in their home models. A reader of `7|a` must leave the model sixteen times to rebuild it.
3. **A comparison that makes a member.** `hart1995firms/14|a` writes `- 对照 [[hart1995firms/11|a|12|f|2]]：多期债务模型要求……` in its `**关联**` section. Because every Link from a Model Card to a Model part makes membership, that chapter Card of another model becomes a `shared` part of model 1, with one tolerated premise.

Before zt 0.47.0 the GUI hid all of this. The 共用 tab only reopened the Model Card, and opening a borrowed Card jumped to its home model's board.

### How sharing works today

| Step | Code | Behaviour |
|---|---|---|
| Who makes a member | `model::heads` (`src/model.rs`, the `linking` filter) | Any Link from a Model Card (type `model`, no facet) to a Model part. The section and the relation word do not matter. |
| What becomes a member | `model::parts_of` | Each linked Model part, one Card per Link. The Cards filed under a borrowed Card do not come with it. Idea, Finding and Model Cards (a whole model) cannot be members. |
| The borrowed Card's own premises | `build_report` → `tolerated` | Listed, not taken in. The fix says "take it in too", one Card at a time. |
| Where it shows | `blocks_report` | One `shared` block (共用前提) before the setting, with every borrowed Card in one bag. |
| Which model a shared Card shows | `structure` | Its first model (before 0.47.0 the GUI could not ask for another one). |

### Where the pipeline breaks sharing

The research pass over the code (2026-10-30) found eight breakpoints. They are listed with the most damaging first.

| # | Component | What happens | Effect |
|---|---|---|---|
| B1 | `zt-model-fold` `scripts/model-survey.mjs` (`propose`: `l.rests + l.shared > 0` joins a group); `zt-model-restructure` SKILL.md ("if other Model Cards share its parts, the model is parted: fold it first") | A shared part counts as structural evidence that two Model Cards are one model. | Intended sharing is folded away. |
| B2 | `model::parts_of` | One Card per Link; no subtree. | A setting tree, a result with its proof, or a chapter cannot be shared as a unit; its other Cards become `tolerated`. |
| B3 | `model::heads` | Any Link from a Model Card makes membership, whatever its relation. | A `对照` or `参照` line makes a false share. |
| B4 | `model::ensure_allowed` / `parts_in_db`; `zt-model-fold` `mask-links.mjs` | A Model Card that links any Model part, a borrowed one included, cannot be retyped. | A fold must mask and restore the Links of other models. |
| B5 | `zt-model-settings` `scripts/check-settings.mjs` | The notation card must be filed in the borrower's own setting part, and every symbol a borrowed Card uses must be restated there. | The settings check forces copies. |
| B6 | Tree Lock (`src/tree_lock.rs`) | A locked borrower pins every address it links. | The lender cannot be restructured or folded around the borrowed Card. |
| B7 | Model compiler (`tools/zt-digest/src/model/compiler.rs`: `refresh_shared`, the `<model>#<id>` resolution; `model/context.rs`); the architect brief `scripts/book-digest/model-architect.md` | Shared lines are repaired only inside one batch; the architect sees only its own book; the brief still allows "restate it as a new part". | No sharing across batches or books; copies are allowed. |
| B8 | `model::parts_of` (`is_part`), `closure_fix` ("a model rests on parts, not on a whole model") | Only a Model part can be shared. | A model cannot build on a whole other model (the Hart–Moore model as one block), nor on an Idea or Finding Card. |

## Goals

1. A model can take in another model's building block as **one unit**: a Card and everything filed under it, in that Card's own order.
2. A model that takes in a block is **self-sufficient**: the block's premises in its home model come with it, so no `tolerated` premise is left.
3. Sharing is **explicit and intended**. A comparison or a pointer never makes a member.
4. The borrower **never restates** a borrowed definition or symbol. It may name a borrowed symbol in its own notation, and it may specialize a borrowed Card.
5. Every tool (fold, settings, restructure, the checks, the compiler, the GUI) reads an import as intended structure.
6. A block can come from **another book**.

## Non-goals

- No copies with synchronization. One Card stays in one place (the 2026-10-12 decision "one shared part" stands).
- No automatic discovery of identical blocks across books. A future idea-grow or bridge stage could propose imports; this proposal only makes them possible.
- No change to Topic trees or hubs.

## Proposed terms

| Term | Meaning |
|---|---|
| **Building block** (构件) | A Model part together with every Model part filed under it in its home model (its subtree), or a whole model (its Model Card). The block keeps its home model and its addresses. |
| **Home model** | The model whose Model Card the block is filed under. |
| **Import** (引入) | One line in the `**引入**` section of a borrowing Model Card that names a block's root. The only way a Card of another model becomes a member. |
| **Closure of an import** | The block's subtree plus, recursively, every Model part in the home model that a Card of the block rests on. |
| **Interface** (接口) | What a block gives its borrowers: the symbols it defines and the results it states. It is read from the block's notation and result Cards, not written twice. |
| **Binding** (对接) | One line in the borrower's notation card that names a borrowed symbol in the borrower's notation, for example `w 即 [[A/1|a|a]] 的 w_A`. |
| **Variant** | A part of the borrower that `specializes` or `relaxes` a borrowed Card: it states only the change and rests on the borrowed Card. |

## Design

### D1. An import is explicit: the `**引入**` section

The borrowing Model Card names each block in one section:

```markdown
**引入**

- 整块 [[hart1995firms/14|a|a]]：模型 1 的三时点设定、优先债务与违约即清算
- 单卡 [[grossman2002integration/2|a|a|14]]：规模报酬不变时的行业需求水平 (10)
```

- `整块` (whole block) imports the Card and its subtree. `单卡` (one Card) imports that Card alone and keeps today's behaviour.
- Membership comes only from this section and from the Model Card's own `结构` list for Cards filed under it. A Link from a Model Card anywhere else (`**关联**`, `问题`, `概要`) is a Model neighbour, never a member. This fixes B3.
- **Transition.** Until the live box is migrated, a `结构` line marked `（共用）` reads as `单卡`, so every finished model keeps its members on the day of the release.

### D2. A block is a subtree, and its closure comes with it

`parts_of` expands each `整块` import into the block's subtree, in Folgezettel order, and then adds the closure: each Model part of the home model that a member of the block rests on, recursively. The build report places the whole closure as one shared run before the first step of the borrower that rests on any Card of it (the existing `take_in_shared` logic, applied to the closure in its own build order).

- `tolerated` keeps its meaning but should be empty for an `整块` import. A non-empty `tolerated` list now points at a real gap in the home model.
- A closure is bounded by its home model: it never follows a Link out of the home model. A premise outside the home model is the home model's own `closure` fault.
- An import cycle (A imports a block of B that rests on a block A lends to B) is a new `import_cycle` fault with the two blocks named.

For `hart1995firms/15|c` this means one line, `整块 [[hart1995firms/14|a|a]]`, instead of the restated setting `15|c|a`. Because model 1's restructure filed the main line under `14|a|a`, the subtree is too large here. That is the reason for D2a.

### D2a. The block root must be the block, not the model's main line

A whole-block import of a setting must not bring the setting's model with it. Two rules keep the subtree the right size:

1. `整块` stops at a **chapter boundary**. The subtree of a setting Card contains its Side successors (the setting's dependency tree, as `zt-model-settings` builds it) and their subtrees, but no Direct successor that is a chapter card of the spine (`**分部**` or `第 n 章`). This matches how `blocks_report` already separates the setting block from the chapter blocks.
2. A borrower may also import a **block** of the board by its block head. `blocks_report` already computes blocks; an import of a block head is an import of that block's members, its children and their proofs.

### D3. A whole model as a building block

A borrower may import a whole model: `整块 [[hart1995firms/11|a|12]]`. The closure is every member of that model. On the borrower's board this model appears as one 共用 block named by the home model's title, and `closure_fix` no longer says "a model rests on parts, not on a whole model" for an imported Model Card (B8). The Cards a borrower rests on must still be named precisely: a rests-on Link goes to a part, not to the Model Card.

### D4. Interface and notation: no restated symbols

- The **interface** of a block is computed from its Cards: the symbols defined in its notation card and setting Cards, and its result Cards (`结果`, `命题`, `定理`). `get_model` reports it per import (`imports[].interface`).
- The borrower's notation card lists borrowed symbols by a **binding** line, `- w 即 [[A/1|a|a]] 的 w_A`, or by a single line `- 沿用 [[A/1|a|a]] 的全部符号`. A binding is a Link with a symbol, so the board and the reader can show it.
- `check-settings.mjs` counts the symbols of every imported block as defined, and it no longer asks for an own notation card when the borrower adds no symbol (fixes B5). It still refuses a symbol that a borrowed Card uses and no Card of the closure defines.

### D5. Variants: specialize instead of copy

When the borrower changes one assumption (Grossman–Helpman 7|a: the matching function now has increasing returns), it writes **one variant Card** that `specializes` (or `relaxes`) the borrowed Card and states only the change. The variant rests on the borrowed Card; the rest of the borrowed block stays shared. `specializes` and `relaxes` already exist as relation kinds (`RelationKind::Specializes`, `Relaxes`). The model architect's brief replaces "restate it as a new part" with "import the block and write a variant for the change".

### D6. Build report, blocks and faults

| Field | Change |
|---|---|
| `get_model.imports` (new) | `[{root, mode: "block"\|"card"\|"model", home, members, closure, interface, used_by}]`: each import with what it brought. |
| `build.order` | Shared entries keep `line: "shared"` and `home`; each adds `import` (the root it came through). |
| `build.tolerated` | Only for `单卡` imports. |
| `build.faults` | New `import_cycle`; new `false_share` warning during the transition (a member that only a `关联`-style Link makes). |
| `blocks` | One `shared` block **per import**, named by its home model and root title, before the setting, instead of one 共用 bag. |
| `get_model.lent` (new, on the home model) | The models that import a block of this model, with the blocks: the lender sees who depends on it before it restructures. |

### D7. GUI

Done in zt 0.47.0:

- The 共用 and 旁注 tabs open their shelf in place on the Model Card; each borrowed Card carries a home stamp that opens the model it lives in.
- `/api/card?via=` keeps the borrowing model: a borrowed Card opens in the board the reader came from.

With this proposal:

- One 共用 tab per import (D6), labelled by the home model; the shelf shows the block as its own small tree, not as a flat cascade.
- On the lender's board, a block lent to other models carries a small "⧉ 2" flag (lent to two models); pointing at it lists the borrowers.
- A binding line in the notation card shows the borrowed symbol beside the local one.

### D8. MCP and write checks

- `get_model` adds `imports` and `lent` (D6).
- `edit_card` on a Model Card with a `**引入**` section checks each line: the target exists, is a Model part or a Model Card, and is not filed under this Model Card. An `整块` line whose root has no subtree gets a warning ("use 单卡").
- `ensure_allowed` (B4) ignores the Links of the `**引入**` section when it decides whether a Model Card may be retyped; it still refuses the retype when the Card has its own filed parts.

## Pipeline changes

| Tool | Change |
|---|---|
| `zt-model-fold` (`model-survey.mjs`) | An import never joins a fold group (B1). A `单卡` share still counts as evidence until the migration ends, marked "legacy share". The fold keeps the `**引入**` section of the folded Model Card and moves it to the target. |
| `zt-model-restructure` | Drop "if other Model Cards share its parts, fold it first". An import is legitimate; the spine treats the imported closure as premises before the setting, as `blocks_report` already does. |
| `zt-model-settings` (`check-settings.mjs`) | D4: imported symbols count as defined; bindings are checked. |
| `check-premises.mjs` | Equation numbers are scoped to the model and its imports (today an equation number from any shared part can hide or create a hit). |
| Model compiler and architect | The architect receives a **block catalogue**: the setting blocks and result blocks of the models already in the box, across books, with their interfaces. A plan may import from it (`take` with `mode`). `refresh_shared` repairs imports in every model, not only the batch (B7). The brief drops "restate it as a new part". |
| zt-check | No rule change; `--skip-shared` skips the imported closure. |
| Tree Lock | Unchanged rule. `get_model.lent` lets a restructure see the pins before it starts; the skills stop and report when a block they would move is pinned by a locked borrower (B6). |

## Migration of the live box

All steps are deterministic scripts over file paths and zt calls, run once per book (no agent per item):

1. **Convert legacy shares.** For each `结构` line marked `（共用）`, write a `**引入**` line. If the shared Card is a block root (it has a subtree) and the borrower's tolerated list names Cards of that subtree, write `整块`; otherwise `单卡`. The script prints each decision for review.
2. **Remove false shares.** A member made only by a Link outside `结构` and `**引入**` (for example `14|a` → `11|a|12|f|2`) stops being a member; the script lists them, and nothing changes in the Card text.
3. **List the copies.** The script reports each borrower setting that restates symbols of an imported block (first case: `hart1995firms/15|c|a`). By decision 5 nothing is changed: a copy is replaced by an import and a binding only when the owner names that model.
4. **Rebuild the boards** and re-run `check-model.mjs`, `check-network.mjs`, `check-settings.mjs` on every migrated model.

Locked trees (`becker2025division`, `grossman2002integration`) take no edits. Their models keep the transitional reading (D1) until the owner unlocks them.

## Slices

| Slice | Content | Acceptance |
|---|---|---|
| S1 | `**引入**` parsing; membership only from `结构` and `**引入**`; the transitional `（共用）` reading; `false_share` warning | Every live model keeps its members and build order; `14|a` reports one `false_share`. |
| S2 | `整块` import: subtree, chapter boundary (D2a), closure, `import_cycle` | A test model importing a setting block has an empty `tolerated` list; `grossman2002integration/7|a` rebuilt on a box copy with `整块` imports has 0 tolerated premises. |
| S3 | Whole-model import (D3); `closure_fix` update | A model importing a whole model builds with no `closure` fault. |
| S4 | `get_model.imports`, `lent`; `blocks` one shared block per import | MCP and GUI tests. |
| S5 | GUI: one 共用 tab per import, the shelf as a small tree, lent flags, bindings | Screenshots of `hart1995firms/15|c` and `grossman2002integration/7|a` in light and dark. |
| S6 | Skills: fold, restructure, settings, check-premises (D4, pipeline table) | The skill tests pass; `model-survey.mjs` no longer groups `14|a`, `15|c`, `16|a`. |
| S7 | Compiler and architect: block catalogue, imports across batches and books, no restating | A digest pilot on a box copy that imports one block from another book. |
| S8 | Migration scripts and the live run (after the owner reviews the printed decisions) | Every migrated model passes the three checks; the copies list goes to the owner. |

## Decisions

The owner decided the five questions on 2026-10-30, each by the recommended answer:

| # | Question | Decision |
|---|---|---|
| 1 | Whole-model import (D3): a whole other model, or only blocks of it? | **Allowed.** A borrower may import a whole model (`整块 [[<Model Card>]]`). Its rests-on Links still name parts. |
| 2 | Transition length | **Gradual.** `（共用）` lines and Links outside `**引入**` keep reading as members (with the `false_share` warning for the latter) until every book is migrated; the transitional reading is removed in a later release. |
| 3 | Bindings (D4): name each borrowed symbol, or one line? | **One line is enough.** `- 沿用 [[<block>]] 的全部符号` binds every symbol of the block; a per-symbol line (`w 即 [[…]] 的 w_A`) is needed only when the borrower renames a symbol. |
| 4 | Across books | **Allowed by default.** An import may name a block in any agent Literature tree; the human Workspace stays read-only as a lender in the ordinary way (a Link is allowed). |
| 5 | Existing copies | **Left as they are.** Finished models keep their restated settings (first case `hart1995firms/15|c|a`); migration step 3 only reports the copies, and a copy is converted only when the owner names it. |

## Risks

- **Large closures.** An `整块` import of a Card high in a home model could bring most of that model. D2a bounds it at chapter boundaries, and `edit_card` warns when a closure has more than 20 Cards.
- **Pins.** More imports mean more pinned addresses in locked trees. `lent` makes the pins visible before a restructure.
- **Two readings during the transition.** A model may hold both `（共用）` lines and `**引入**` lines. The build report names the source of each shared entry (`import` vs `legacy`), and the migration ends the mixed state book by book.
