# One model, one Model Card: merging parted models

Status: proposal (2026-10-28). The workflow step is not implemented. The `becker2025division` repair was done by hand on the live box at the owner's request (2026-10-28, §9). Scope: the model layer of the digest workflows (`zt-digest-base` 5.9.x, `zt-econ-digest` 6.9.x, `zt-math-digest` 5.9.x) and a repair of the books already built. It changes no Topic tree. Related: [model-one-conversation-proposal](model-one-conversation-proposal.md) (one DeepSeek conversation per model), [self-sufficient-models-proposal](self-sufficient-models-proposal.md) (shared parts), [models-and-math](wiki/models-and-math.md) (membership and the build report).

## 1. Problem

A book that develops one model across several sections gets one Model Card per section. The reader then sees a set of small models that each hold one piece of the same model.

### 1.1 Evidence: `becker2025division`

The paper develops one model. Section 3 gives the production technology with task assignment and the efficiency wage. Section 4 solves the closed economy in three stages and gives its equilibrium. Section 5 opens the economy, and section 6 estimates and quantifies the same model. The digest wrote 11 Model Cards for it:

| Model Card | Level | Group, source section | Build order (shared) | Own rests-on | Rests-on Links to other Model Cards |
|---|---|---|---|---|---|
| `1|b` 劳动分工：工厂权衡匹配收益与管理跨度成本 | concept | g1, s01 (引言) | 4 (0) | 2 | — |
| `4|a` 带任务配置的生产 | formal | g4, s06 §3 | 8 (0) | 10 | — |
| `5|a` 效率工资与任务错配 | formal | g5, s07 §3.3 | 11 (0) | 12 | — |
| `6|a` 工厂进入与生产的三阶段 | formal | g6, s08 §4.1 | 6 (2) | 5 | 4|a 1, 5|a 1 |
| `7|a` 最优就业选择（第三阶段） | formal | g7, s09 | 12 (4) | 15 | 4|a 2, 5|a 1, 6|a 2 |
| `8|a` 职业数量的最优选择（第二阶段） | formal | g8, s10 | 20 (3) | 30 | 6|a 2, 7|a 2 |
| `9|a` 第一阶段的最优进入 | formal | g9, s11 | 12 (4) | 16 | 6|a 1, 7|a 2, 8|a 4 |
| `10|a` 自给自足均衡 | formal | g10, s12 §4.2 | 20 (9) | 32 | 5|a 3, 6|a 1, 7|a 2, 8|a 6, 9|a 3 |
| `11|a` 开放经济工厂模型 | formal | g11, s13 §5.1 | 21 (4) | 36 | 8|a 5 |
| `12|a` 基准开放经济均衡模型 | formal | g12, s14–s15 §5.2–5.3 | 30 (10) | 46 | 5|a 3, 7|a 2, 8|a 5, 9|a 5, 10|a 1, 11|a 3 |
| `13|a` 结构估计与量化模型 | formal | g13, s16–s18 §6 | 32 (12) | 52 | 8|a 5, 10|a 2, 11|a 6, 12|a 4 |

The counts come from `get_model` on the live box (2026-10-28, read only). "Shared" is the number of parts that a Model Card takes in from another Model Card. The table shows three facts:

1. **The pieces depend on each other.** Every Model Card from `6|a` on rests on earlier ones, and 48 of the 176 build-order entries are parts taken in from another Model Card. `10|a` (the autarky equilibrium) needs 9 parts from 5 other Model Cards before it is complete.
2. **The pieces know that they are pieces.** The head of `9|a` says: "关联：参照「6|a」的三阶段时序，本模型只求第一阶段。" A model that "only solves stage one" of another model's timing is a part of that model, not a model of its own.
3. **The build check passes.** All 11 report `self_sufficient: true`, because shared parts close each range. The check proves that each piece reads on its own. It does not ask whether the pieces are one model.

### 1.2 Cost for the reader and the archive

- The model's question, setting, timing and main result are spread over 4 to 10 Model Cards. No single Model Card states the whole model.
- The same setting is written several times (`6|a|a`, `7|a|a`, `8|a|a`, `9|a|a` each restate a part of the timing).
- The register, hubs and search show 11 model heads for one model, and zt-idea-grow reads them as 11 candidate concepts.

## 2. Cause

1. **One Model Card per group.** The digest prompt (`scripts/book-digest/dsh-model-cards.zh.md`) asks each group to write a Model Card for "你的小节提出的每个完整模型". Becker's sections 3 to 6 are 10 groups (g4–g13, one section file or a few each), so each group saw only its own piece and wrote it as a complete model.
2. **The groups do not see each other.** The groups run in parallel. The rule "模型若由其他组提出，链接其锚点或快照中的模型卡" works only when the earlier group's Model Card is already in the snapshot, and a parallel group cannot rely on that. So each group writes a new Model Card and links the others by `对应`.
3. **One ledger unit per Model Card.** The digest's completion registers one `models/gNNN-mNNNN@1` unit for each Model Card that a group wrote (`ledger.rs`, the producer children). The model layer compiles units, so it compiles each piece as a model.
4. **Batches split a family.** The architect plans at most 8 formal models per batch, in address order. Becker became two batches: `1|b`–`10|a` (`m-becker2025division_1_b`) and `11|a`–`13|a` (`m-becker2025division_11_a`). The second architect could only take parts in from the first batch, not plan them.
5. **No step judges model identity.** The plan's `merge` joins two parts of one model. A take-in (`<other model>#<id>`) shares one part. Nothing decides "these Model Cards are one model".

## 3. Goal

1. **One Model Card per model that the book presents.** A model that the book develops over several sections has one Model Card. Its main line follows the book's order: the setting, then each stage, the equilibrium, the extensions the book derives from the same setting, and the quantification.
2. **The book decides what one model is.** A section that keeps the agents, the symbols and the equations of an earlier section, and derives from them, continues that model. A section that changes the question or the primitives, and that the book presents as a new model, starts a new Model Card linked by `放宽` or `对照`.
3. **The roles stay as they are.** The Sonnet architect decides model identity, because it already plans the structure of models. Scripts move, retype and relink Cards. DeepSeek writes the merged head and the bridging sentences. No LLM agent does script work.
4. **Readable segments.** A merged model can have more than 100 parts (Becker: about 128 distinct parts). Its main line is grouped into segments with facet labels (`第三阶段：最优就业选择`, `自给自足均衡`, `开放经济`, `结构估计`), so the reader can find a segment, and each segment states what it takes from the earlier ones.

Non-goals: merging models of different books (that is the Bridge layer and zt-idea-grow); changing how a Model part is checked; changing the explanation rules.

## 4. Design

### 4.1 New books: a model identity step before the briefs

The model layer gets one step before batching, `model -Step families`:

1. **Survey (script).** For every Model Card of the book: address, title, level, group, the section headings of its group, its head body (cut to 1,500 characters), and the Links between Model Cards (kind and count). It writes `families-survey.json`.
2. **Decision (Sonnet architect, one call per book).** The architect reads the survey and the section headings and writes `families.json`:

   ```json
   {"families": [
     {"head": "Key/4|a", "absorb": ["Key/5|a", "Key/6|a", "Key/7|a"],
      "segments": [{"from": "Key/4|a", "label": "生产技术"}, {"from": "Key/5|a", "label": "效率工资"}],
      "reason": "§3–§4 keep the agents, the symbols and the timing of §3; §4.1 says it solves the stages of the §3 model"},
     {"head": "Key/11|a", "separate": true, "relates": {"to": "Key/4|a", "kind": "放宽"},
      "reason": "only if the book presents the open economy as a new model"}
   ]}
   ```

   The architect instructions (`model-architect.md`) get one section with the rule of §3.2 and the Becker case as the example. A concept Model Card that only summarises a formal model of the same book (Becker `1|b`, the introduction's verbal statement) is absorbed: its mechanism and conclusion become the merged model's 直觉 and 结论 material.
3. **Merge apply (script).** For each family, `model -Step families-apply` does the deterministic part (§4.3) before the briefs.
4. **Batching keeps a family whole.** A family is one model for the batch rules, so it is never split across two batches.

The ledger records the decision: each absorbed Model unit ends `superseded` with `families.json` as evidence, and the surviving unit's inputs name the absorbed headers. A rerun reads the same `families.json` and does not ask again.

### 4.2 Built books: the same decision as a repair mode

The live books were built before this step, and their models are finished. A repair is a new run on the owner's go-ahead (the rule "finished models stay as built" holds until then), and it needs the owner to unlock each tree.

`zt-econ-digest` mode `families` (and the same mode in the math variant) runs §4.1 steps 1–3 on an existing staging folder, then rebuilds only the surviving Model Card of each family: the brief, the plan and the close check for the merged range. The parts keep their bodies (`existing`), so no derivation is written again. DeepSeek writes the merged head and one bridging line per segment, in the model's one conversation.

### 4.3 How the merge moves Cards

The membership rules decide the mechanics ([models-and-math](wiki/models-and-math.md), "Membership: filed under or linked"):

- A part belongs to the Model Card it is filed under, and to every Model Card whose body links it.
- A Model Card that still has parts cannot be retyped into a part (`model::ensure_allowed`: "is the Model Card of N Model part(s) … change or move them first").

So a merge that only links all the parts from one head would leave the old Model Cards as models. The old Model Cards stop being models only when their parts are filed under the surviving head. The apply step therefore:

1. Moves each absorbed Model Card's setting, with its successors, to the end of the surviving head's main line, in segment order (`move_card` with `to`; it rewrites every Link to the moved Cards). This keeps the parts' bodies and Links.
2. Retypes each absorbed Model Card, which now has no parts, into a Model part with the segment's facet. It becomes the segment's lead part, and keeps its 问题/结论 text as the segment's summary. Links that pointed to it still resolve.
3. Turns the take-ins inside the family into ordinary members: a shared part that now lies in the range is no longer shared. Take-ins of a part that the family already holds twice are listed for the plan's `merge`.
4. Leaves Links from outside the family alone. They point to Cards that still exist; a Link to an old head now reaches the segment's lead part.
5. Writes `families-apply.json` (every move and retype, with old and new addresses). A failure stops before the next family, and the file says which family is done.

All moves stay inside the book's Literature tree. The parts leave their section anchors. The anchor Card of each section keeps a Link to its segment, so the reading notes of that section still lead to the model.

### 4.4 Size and the one-conversation layer

The one-conversation layer gives each model one DeepSeek conversation. A merged model can be too large for that. The rule: one conversation per segment, with the merged head's brief in each segment's fallback files, so each conversation knows the whole setting. A family of at most about 30 parts keeps one conversation. The pilot (§5) measures where the limit is.

## 5. Testing and acceptance

- **Unit tests (`zt-digest`):** the survey lists every Model Card with its Links; `families-apply` on a fixture with three pieces moves the parts, retypes the old heads, and ends with one head whose `get_model` holds every part and no shared part from the family; the ledger supersedes the absorbed units; batching never splits a family.
- **Harness:** mode `families` runs the survey, one architect call, the apply step and the rebuild, and starts no job for an absorbed unit.
- **Pilot (owner gate):** a copied box with an adopted copy of the Becker staging folder, never the live box. Acceptance:
  1. The architect's families match a person's reading of the paper (the owner checks `families.json`).
  2. One Model Card for the family. `get_model` lists every former part, `self_sufficient` is true, and `close-check` prints `faults: 0`.
  3. No Card body is lost. Every former part exists, with the same body or a body edited only in its bridging line.
  4. Every Link in the book resolves (`list_broken_links` is empty).
  5. The DeepSeek and Claude tokens are recorded, and the merged rebuild costs less than building the 11 models did.

## 6. Risks

- **Wrong merges.** Two models that share symbols but answer different questions could be merged. The architect gives a reason for each family, the owner checks the pilot's `families.json`, and `separate` with a relation keeps the link between models that stay apart.
- **Moves in a locked tree.** `becker2025division` is locked (lock 19, since 2026-10-28), and so is `acemoglu2010labor`. A repair needs the owner to unlock each tree, and it never works around a Lock.
- **Address changes.** Moves change addresses. `move_card` rewrites Links inside the archive. Addresses written outside the archive (staging files, earlier reports) become stale, so the repair records the address map.
- **Large segments.** A segment that is itself large may still fill a conversation; the pilot measures it (§4.4).

## 7. Slices

1. **F1:** survey and `families.json` (script, architect section and example, unit tests).
2. **F2:** `families-apply` (moves, retype, ledger supersede, unit tests).
3. **F3:** batching keeps a family whole; one conversation per segment for a large family.
4. **F4:** mode `families` for built books; release (base minor, econ minor, math minor, zt-digest minor).
5. **F5:** pilot on a Becker copy (owner gate).
6. **F6:** repair of the live books, book by book, on the owner's go-ahead.

## 8. Open decisions for the owner

1. **The open economy and the estimation.** Becker §5 and §6 use the setting and the stages of §3–§4. This proposal reads them as segments of the same model. The alternative is two or three models (closed economy, open economy, estimation) linked by `放宽` and `例示`. Which reading should the architect follow?
2. **The introduction's concept model** (`1|b`). Absorb it into the formal model (this proposal), or keep it as a separate concept-level overview?
3. **Scope of the repair.** Becker only, or a survey of all five live books first (§4.1 step 1 is a script and costs no model call)?

## 9. The Becker repair (2026-10-28)

The owner decided (2026-10-28): §5 and §6 are segments of the one model, and the introduction's concept model `1|b` is folded in. The repair ran on the live box (`D:\zt`, revision 10284 to 10380) after the owner unlocked the tree. A copy of the archive taken before the repair is `D:\zt-merge-backup\zt-before-becker-merge.sqlite3`.

**Result.** `becker2025division/4|a` (title: 劳动分工的异质工厂模型：任务配置、三阶段决策、开放经济与结构估计) is the only Model Card of the book. It has 138 members: the main line holds its own 8 parts and 9 segment leads (`4|a|a|8` to `4|a|a|16`, the former `5|a` to `13|a`), each with its former parts as Side parts, and the former `1|b` is the aside part `4|a|b` (概述：引言的文字版本). `self_sufficient` is true, with no closure, forward, reach-in or unanchored fault. The tree still holds 418 Cards, and `list_broken_links` is empty.

**Procedure that worked, per absorbed Model Card X** (it refines §4.3):

1. Remove X's **结构** block (its Links to its own and to shared parts).
2. Move X's Idea-note Side successors to X's section anchor. A Direct chain of notes under X moves up into X's place when X moves alone.
3. Park X's setting and its other Model parts as Side successors of the surviving head.
4. Mask, as plain text, every Link in X's body to a Card that is a Model part now (the earlier segment leads and the surviving head's parts). Save the original body.
5. Retype X: `facet` (the segment label), `emphasis`, `level` empty.
6. Restore X's original body. A Model part may link other parts.
7. Move X alone to the next Direct slot of the main line, then move the parked parts back as X's Side successors.
8. After all segments, add one rests-on line to each lead (`依赖 [[<head setting>]] 的设定：……`), so that no lead is unanchored, and rewrite the surviving head (question, summary, **结构** with the segment leads, source).

Two rules of zt decide this order. A Card that is a Model Card of parts cannot be retyped (`model::ensure_allowed`), and a Model Card's body Link to any Model part counts as a part. So a rests-on line added before step 5, or a Link to a lead that was retyped earlier, refuses the retype.

**Left as it is.** The lead parts keep the full text of the former Model Cards, so the checker gives R13 advisories (13 to 45 sentences against 12) and some R2 advisories. Splitting the leads is a separate Card-quality task. The Becker staging folder `D:\zt-digest\becker2025division` names the old addresses. Its ledger and model batches describe the old 11 models, so a later digest mode on that book has to adopt a fresh copy.
