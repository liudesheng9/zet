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

### 9.1 The readable structure (2026-10-28)

The owner found the merged model hard to read: a flat main line of 17 steps, each segment a former Model Card. The model was regrouped by role on the live box (backup `D:\zt-merge-backup\zt-before-becker-structure.sqlite3`, revision 10380 to 10429). The main line is now seven groups:

| Main step | Facet | Holds (Side successors) |
|---|---|---|
| `4|a|a` | 设定 | every setting of the model: 信息与努力, 工资表 (7), 三阶段时序 (with 进入条件 and 固定成本), 三元正态抽签, 开放经济, 基准开放经济, 估计的数据 |
| `4|a|a|1` | 过程一：生产技术 (new header) | the production chain (1)–(6) and its conclusion |
| `4|a|a|2` | 过程二：工资与离散度 | the wage chain (8)–(10) and the η proxy |
| `4|a|a|3` | 求解：三阶段逆向归纳 | 第三阶段, 第二阶段, 第一阶段, each with its own setting and chain |
| `4|a|a|4` | 变体：三种均衡 (new header) | 变体一 封闭经济均衡, 变体二 开放经济, 变体三 开放经济基准 |
| `4|a|a|5` | 量化：结构估计与反事实 | the estimation chain, tables 2 and 3, the counterfactual, the limitation |
| `4|a|a|6` | 主要结论 (new) | links to the core result of each group |

The introduction's verbal version stays the aside part `4|a|b`. The head's **结构** section lists the seven groups and the cards one level below each. Result: 141 members, `self_sufficient` true, 421 Cards in the tree (418 plus the three new cards), no broken Link.

This is the target shape for §4.1: a merged model's architect plan should name these groups (settings, processes, solution, variants, quantification, conclusions), not only the segments of the source sections.

### 9.2 Readable cards and a consistent notation (2026-10-28)

The owner found the cards themselves hard to read: settings scattered with their own symbol tables, derivation cards that did not say what they were for, unexplained marks such as 「（补：书中未写）」, and messy relations. A third pass (backup `D:\zt-merge-backup\zt-before-becker-cards.sqlite3`) did four things:

1. **One notation card.** `4|a|a|a` (符号表与读法) holds every symbol of the model, grouped by block, and a legend of the marks: equation numbers, 「（论文略去，此处补出）」, 「推导链第 k 步，共 n 步」, the subscripts a/1/x/0. The 8 local symbol tables (79 symbols, 25 defined twice) were removed. One conflict was settled against the source: $\pi$ is the circle constant (it enters (10) as $\sqrt{4-\pi(\pi-2)}$), not a wage parameter.
2. **Every card rewritten to one format by its role** (rules in `D:\zt-model-tidy\becker\style.md`): a group card opens with one sentence, the question, the key points, the reading order and its premises; a setting card says what it sets and why it is needed; a derivation card opens with 「目的：」 in words, keeps 思路/起点/步骤/结果/解读, lists each premise once, and names its temporary symbols; a result card gives the result in words, then the exact statement, the meaning and the basis. DeepSeek V4.1 Flash (official, high) drafted the 141 cards in 11 group jobs; a script checked them (every display formula and equation number kept, Links only to existing Cards, no 「本模型」, the required sections) and put back verbatim the 14 quoted premise formulas that drafts had dropped.
3. **Relations.** The pointer to the notation card is written without a premise cue, so it is a plain reference. Result: 446 relations, 302 rests-on and 134 refers, no duplicate pair, no internal 对应 left; `self_sufficient` true, no broken Link.
4. **GUI (zt 0.42.1).** A Link to a Model part shows the part's facet in its chip (for example 推导：利润 (13)) instead of the full address, which moves to the tooltip.

### 9.3 An intuition spine (2026-10-28)

The owner's principle: comprehension costs working memory, so the reader must hold as little as possible at a time. Grouping by card type (all settings together, all processes together) did not lower that load, because each chapter was still one long chain that mixed settings, derivation steps and results. The model was rebuilt in three levels (backup `D:\zt-merge-backup\zt-before-becker-spine.sqlite3`; 260 tracked moves by `D:\zt-model-tidy\becker\spine.mjs`):

1. **Main line: intuition.** The setting, then 11 chapter cards (第 1 章：直觉 … 第 11 章：主要结论). A chapter card says in plain words what mechanism the chapter shows and why (**直觉**), lists its results (**本章结果**, plain references), and says what it takes from earlier chapters (**承上启下**, rests-on only to earlier chapters). The introduction's verbal mechanism is chapter 1.
2. **Results** hang as Side branches of their chapter (26 result cards, labelled 结果：…). A result card gives the result in words, the exact statement with its formulas, **为什么** (the intuition), and where its proof starts.
3. **Proofs** hang as Side branches of their result: the derivation chain that ends in it. Chapter-level settings and asides (本阶段的设定, footnotes, limitations, extensions) hang directly under their chapter.

A chapter never rests on its own results, so the build has no forward fault, and every main step rests on an earlier one. Result: 142 members, 12 main steps, `self_sufficient` true, no broken Link. In the GUI, the Map's Focus mode (`x`) then shows the low-memory view by itself: the main line as slips, the selected chapter's results, every proof branch folded into a "+N" count.

This three-level shape (intuition spine, results, proofs) is the target for §4: the architect's plan should name the chapters and their results, and the compiler should file each derivation chain under the result it proves.

### 9.4 The network repair (2026-10-28)

The model network (zt 0.44.0) and `scripts/check-network.mjs` of the restructure skill showed two forward faults: settings filed under 设定 that rest on proof steps of later chapters. Simulated against all 240 rests-on Links first, then moved on the live box (backup `D:\zt-merge-backup\zt-before-becker-network.sqlite3`; the tree was unlocked):

- `设定：信息与努力` (was `4|a|a|b`, rests on the efficiency step of chapter 2) and the wage table it supports (`4|a|a|c`, which rests on it) became chapter 3's own settings `4|a|a|3|a` and `4|a|a|3|b`, before the chapter's result (whose proof rests on the wage table).
- The derivation chain `推导：管理跨度项` → `设定：固定成本` (was `4|a|a|d|2`, `4|a|a|d|3`; rests on (2) of chapter 2 and (8) of chapter 3) became `4|a|a|3|e`, `4|a|a|3|e|1`.
- Chapter 3's result and derivation moved to `3|c`, `3|d`; the setting's remaining sides were repacked to `a`–`f` (side slots take only the next free letter, so the cards were parked under chapter 11 and brought back in order). The head's 结构 section follows.

After: `check-network` prints `network: clean` (chapter 4 now hangs under chapter 3, whose wage table it uses); the build is self-sufficient with the same 142 members; no broken Link. Chapters 1 (直觉) and 11 (主要结论) have no result card by design.

### 9.5 Hidden premises (2026-10-28)

The owner's rule: a forward fault is fixed by moving cards, never by deleting a Link or dropping its cue. The earlier passes had written Links to later cards without a premise cue, so a dependency could hide in prose. All 114 plain references to later cards were read, and `scripts/check-premises.mjs` (new) listed every card naming an equation a later card derives. Two were real uses, both fixed:

- The setting cards `4|a|a|b` (three-stage timing) and `4|a|a|b|1` (entry) stated "β̃ 与 n 重复，可由式 (2) 忽略", a result of chapter 2's (2). The statement moved to the card of (2), `4|a|a|2|a|a|1` (item 5), and the two setting cards no longer state it.
- The derivation of (3), `4|a|a|2|a|a|2`, listed formula (4) as a starting formula, which its steps never use and the next card derives; an earlier restore had put it there. Its start is back to the original: the sum of the two integrals.

The other hits are pointers (the notation table, "provides the parts for (6)", where π appears in (8)) and roadmaps (the chapter 4 solution guide, the three-variants overview). After: `check-network` clean, `self_sufficient` true, 142 members, no broken Link.

### 9.6 Settings back in the setting part (2026-10-28)

The owner corrected §9.4: the forward Links came from settings, and a setting belongs in the setting part. A setting is a primitive and rests on nothing derived later, so those Links pointed the wrong way. Backup `D:zt-merge-backupzt-before-becker-settings-back.sqlite3`.

- `设定：信息与努力` claimed to rest on (4). It only names the efficiency variables λ(i,ω) and λ(ω); (4) derives their formula. Its item 4 now names them and points to chapter 2 (式 (4)) without claiming a premise.
- `设定：固定成本` claimed to rest on the derivation `推导：管理跨度项`. The setting defines f(ω) directly; the derivation shows its bracket equals average efficiency by (2) and (8). Now the derivation rests on the setting (`依赖` the fixed cost), the setting points to the derivation, and the setting no longer states the derived fact.
- Moved back: the information setting and the wage table to `4|a|a|b`, `4|a|a|c` (the original letters), the fixed cost to `4|a|a|d|2` after the entry condition; the setting sides repacked `a`–`h`. The derivation `推导：管理跨度项` stays in chapter 3 (`4|a|a|3|c`), after (8) it uses; chapter 3 repacked to result `3|a`, inversion `3|b`, interpretation `3|c`. The head's 结构 rebuilt from the tree.

After: the setting part rests on nothing outside it; `check-network` clean (chapter 4 hangs under chapter 2 again); `self_sufficient` true, 142 members, no broken Link. The skill (Step 3, and `check-network.mjs`'s fix text) now says a setting with a forward Link has a reversed relation and stays in the setting part.

### 9.7 Every setting in the setting part (2026-10-29)

The owner restricted the setting part: every setting, every assumption and every symbol of the model goes there, and no other part keeps one. The rule is now its own skill, `zt-model-settings`, with `check-settings.mjs`. Its first run on Becker found six settings filed in chapters and six derivation symbols missing from the notation card. Backup `D:\zt-merge-backup\zt-before-becker-settings-part.sqlite3`.

- Moved into the setting part: the introduction's setting (from chapter 1), the task allocation (2) and the aggregation (5) (two steps of chapter 2's proof chain, moved `alone`), the third-stage setting (chapter 4), the second-stage setting (chapter 5), and the free-entry equilibrium condition (chapter 7).
- The profit (11), filed under the third-stage setting, is a derivation: it stays in chapter 4 as the first step of the price result's proof chain (`4|a|a|4|b|a`).
- The setting part now reads `a` 符号表, `b` 引言的设定, `c` 信息与努力, `d` 任务配置 (2) → `d|1` 职业平均效率与产出, `e` 工资表 (7), `f` 三阶段时序 → `f|1` 进入条件 → `f|2` 固定成本, with the stage settings `f|a` (第二阶段), `f|b` (第三阶段) and the equilibrium condition `f|1|a`, then `g` 三元正态, `h` 开放经济, `i` 基准开放经济, `j` 估计的数据. Chapters 1, 4, 5 and 7 repacked.
- Text: the two stage settings named results the chapters derive (the price (12); the constraints (1), (5), (6)). Those sentences went to the cards that use them (the constraints to the profit card), and the settings point to their chapter. The profit (13) and the first derivation of chapter 7 now rest on the stage setting and on the equilibrium condition. Every `依赖 [[setting]] 的结果` reads `的设定`. The notation card gained a section for the auxiliary symbols (x, A, H, C(ω), Q(ω), U, ψ_T^e, Δψ_T) and the general r̄(u).

After: `check-settings` all in the setting part; `check-network` clean; `check-premises` only pointers and roadmaps; `self_sufficient` true, 142 members, 422 Cards, no broken Link. The head's 结构 rebuilt.

## 10. The Acemoglu 2010 fold (2026-10-28)

The `zt-model-fold` skill folded `acemoglu2010labor` on the live box (`D:\zt`, up to revision 11146) at the owner's request. The backup is `D:\zt-merge-backup\zt-before-acemoglu2010labor-fold.sqlite3` (`VACUUM INTO`, integrity `ok`).

**Decision.** All 15 Model Cards are one model. Economies D, E and M (`2|a`, `3|a`, `4|a`) share G, C, θ, L and Z. §3 to §4 (`5|a` to `10|b`) rest on Economy M or on its comparative statics, and §5 and the appendix (`11|a` to `15|b`) extend the same setting. `2|a` (Economy D) is the surviving head.

**Locks.** Two other locked trees pinned cards of the book: `acemoglu2007equilibrium` (27 cards, among them the head `10|a`) and `becker2025division` (4 cards). zt refuses a move of a pinned target and of any card above it (`pinned-move`). The owner unlocked both trees before the fold, and both are still unlocked after it.

**Result.** `acemoglu2010labor/2|a` (title: 劳动力稀缺与技术进步模型：技术强劳动节约时稀缺才引致技术进步) is the only Model Card of the book, with 179 members. The main line holds its own setting chain (`2|a|a` to `2|a|a|8`) and 14 segment leads `2|a|a|9` to `2|a|a|22` (经济 E, 经济 M, 劳动力稀缺, 外生工资上升, 最低工资, 绿色技术, 柯布—道格拉斯, CES 要素增强, 机器替代劳动, 动态扩展, 多部门, 内生劳动供给, 均衡偏向, 经济 O). Each lead keeps its former parts as Side parts. Each lead has one rests-on line to the card it builds on (the setting `2|a|a`, Economy M `2|a|a|10`, the comparative statics `2|a|a|11`, the wage framework `2|a|a|12` or the machine model `2|a|a|17`). The head's question, summary, 结构 and source were rewritten. `self_sufficient` is true, the tree still holds 460 Cards, and `list_broken_links` is empty.

**Procedure.** The §9 procedure ran as a one-head-per-run script. The script found each head by title, removed its 结构 block, parked its parts under the head, masked every Link in its body to any Model Card (head or part, in any tree), retyped it, restored the body, moved it `alone` to the next Direct slot of the head's main line, and brought its parts back. A head's Direct-slot address replaces the last number of the main line's last card (`…|8` → `…|9`), never appends to it.

**Next.** The settings go to `zt-model-settings`, then the structure to `zt-model-restructure`. The staging folder `D:\zt-digest\acemoglu2010labor` names the old addresses.

### 10.1 Every setting in the setting part (2026-10-28)

The `zt-model-settings` pass ran on `acemoglu2010labor/2|a` on the live box. The backup is `D:\zt-merge-backup\zt-before-acemoglu2010labor-settings.sqlite3` (`VACUUM INTO`, integrity `ok`).

- **Notation card.** A new card `2|a|a|a` (符号表与读法) holds the base symbol table and the term table, which moved out of the setting `2|a|a`. It also holds a section for the 12 auxiliary symbols that the derivations introduce ($F$, $\theta'$, $A$, $q$, $X$, $D$, $s$, $\Phi(\theta)$, $r$, $\delta$, $\tilde\delta$), each with the chapters that use it. Several letters mean different things in different chapters, and the card says so. The tree now holds 461 Cards (460 plus this card).
- **Setting part.** The Sides of `2|a|a` are `a` 符号表, `b` 假设（G−C 联合凹）, `c` 经济 E, `d` 经济 M, `e` 柯布—道格拉斯, `f` CES, `g` 机器替代 → `g|1` 技术成本, `h` 动态, `i` 内生劳动供给 → Side `i|a` 例 2, and `j` 均衡偏向. The Economy M settings sit under `d`: `d|a` 劳动供给变动 → `d|a|1` 假设 1, `d|b` 净收益与海塞矩阵 (Sides `d|b|a` 例 1, `d|b|b` 规制者), `d|c` 污染, `d|d` 多部门, `d|e` 寡头. Each setting moved `alone`, so its derivation chain closed up and stays in its chapter. The Side cards of two settings (`放宽` in 多部门 and 经济 O) went to their chapter leads.
- **Facets.** The example cards became `设定：例 1` and `设定：例 2`. The remark `评述：放松假设 1 时结果退为弱比较静态` is a relaxation, not an assumption, so its facet changed from `假设 1 的限定` to `放宽：假设 1`, and it stays in 绿色技术.
- **Stated assumptions.** Three chapter sentences moved into their settings, and the chapter cards now rest on those settings (`依赖 [[…]] 的设定`): κ continuous and strictly increasing (机器替代), the nondegenerate Hessian (多部门), and the concavity of G (经济 O). Theorem 5's opening line was reordered (`定理 5 断言：假设 1 成立时，…`).
- **Premises.** Three settings named results that chapters derive. In `d|a` and `d|b`, "(13)" became a pointer to Economy M's Proposition 3. In `g|1`, the rests-on to (20) and the display of (14) moved to the first cost derivation (`2|a|a|16|a|9`), and the setting now points to both chapters. The other `check-premises` hits are symbol-table rows and chain roadmaps.

After the pass, `check-settings` reports every setting in the setting part, `self_sufficient` is true, and `list_broken_links` is empty. The head's 结构 is rebuilt. `check-network` still asks for **直觉** sections on the chapter leads: that is the spine work of `zt-model-restructure`.

### 10.2 The intuition spine (2026-10-29)

The `zt-model-restructure` pass ran on `acemoglu2010labor/2|a` on the live box. The backup is `D:\zt-merge-backup\zt-before-acemoglu2010labor-spine.sqlite3` (`VACUUM INTO`, integrity `ok`).

- **Spine.** The main line is the setting `2|a|a` and 15 chapter cards `2|a|a|1` to `2|a|a|15` in the paper's order: 经济 D, 经济 E, 经济 M, 劳动力稀缺, 外生工资上升, 最低工资, 绿色技术, 柯布—道格拉斯, CES 要素增强, 机器替代劳动, 动态扩展, 多部门, 内生劳动供给, 均衡偏向, 经济 O. Chapter 1 is a new card. Before the pass, Economy D's seven proof cards sat on the main line with no chapter card. Each chapter's results are its Side branches, and each result's derivation chain is a Side branch of the result. Definitions, remarks, relaxations and the side derivation of factor prices are Side branches of their chapter or result. Two cards changed chapter by meaning. The example-1 chain (equilibrium technology, wage, employment, technology effect) became the proof of the minimum-wage result in chapter 6. Definition 1 went back into Theorem 1's proof chain, after the card that derives (15), because the definition uses (15).
- **Mover.** `tree-mover.mjs` made 320 moves (160 cards detached and placed), at about 3 s per move. Two settings still rested on chapters: `g|1` (沿用 the machine chapter and Economy M) and `例 2` (依赖 the proposition it illustrates). Both relations were turned into pointers, and the chapter derivations rest on the settings.
- **Text.** DeepSeek V4.1 Flash (official source, effort high) drafted 75 cards in 15 jobs under two Haiku managers. The cards are 15 chapter cards (**直觉**, **本章结果**, **承上启下** with one `依赖` line), the result cards (`结果：` titles and facets; **结果**, **为什么**, **证明**) and the asides. All 15 conversations were deleted and the workspace removed. `check-drafts` found no lost formula. Its other findings were the kept cross-book Links, a "(3)" that named a condition and not an equation, and one missing `目的：` line, which was added. Derivation cards kept their text. One ch03 agent deleted other jobs' temporary scripts in the work folder; the outputs were not affected.
- **Head and notation.** The 结构 lists the main line and one level under each main card. **分部**: 三种经济 (from the setting), 劳动力稀缺与工资 (chapter 4), 技术类型的例子 (chapter 8), 扩展 (chapter 11), 附录的一般结果 (chapter 14). The notation card gained a legend of the marks (**怎样读卡片上的记号**).

After the pass, `check-model` reports `self_sufficient` true and `check-settings` reports every setting in the setting part. `check-network` reports `network: clean` (16 blocks, 6 layers; chapters hang under 1, 3 or 4 as their rests-on Links say). The 7 remaining `check-premises` hits are symbol-table rows and chain roadmaps. `list_broken_links` is empty, and the tree holds 462 Cards (460 + the notation card + the chapter 1 card).

### 9.8 The setting part as a dependency tree (2026-10-29)

The owner judged §9.7 unreadable: the settings were appended one after another, with no sign of which setting builds on which or which variant uses it, so a reader could not build the setting from scratch. The setting part is now a dependency tree, and the rule is in `zt-model-settings`. Backup `D:\zt-merge-backup\zt-before-becker-settings-tree.sqlite3`.

- Blocks of the base model: the setting card (原始设定); the overview; technology as one chain, 任务配置 (2) → 信息与努力 → 职业加总 (5) → 工资表 (7); timing, 三阶段时序 → 进入条件, with the fixed cost and the two stage settings as its Sides; the draws.
- Variants: `设定：封闭经济` (变体一) and `设定：开放经济` (变体二), with the benchmark (δ̃=1) and the estimation under the open economy.
- Every setting card has a **起点** section naming its premises (`依赖 [[…]] 的…`) or `原始设定`; 16 cards, 28 new rests-on Links, all to earlier settings.
- Symbols at first use: the information card defines the sensitivity η (used first by the fixed cost); the draws card defines the augmented productivity θ̂ and ξ (used by the draws and every variant); the timing card points to the fixed cost for the form of f(ω); the notation card separates the utility U from the auxiliary U=v+ιu.
- Facets name the role and the variant: `设定：概述`, `设定：任务配置 (2)`, `设定：职业加总 (5)`, `设定：第二阶段`, `设定：第三阶段`, `设定：封闭经济`.

After: `check-settings` reports all in the setting part, as a dependency tree; `check-network` clean; `check-premises` only pointers and roadmaps; `self_sufficient` true; 422 Cards, no broken Link. The head's 结构 rebuilt.

## 10. Acemoglu 2010: the setting part as a dependency tree (2026-10-29)

The first run of `zt-model-settings` after Becker, on `acemoglu2010labor/2|a` (live box, owner's request). No setting was outside the setting part; the faults were the tree's: no setting card had an **起点** section, and eight were filed under settings they did not build on. Backup `D:\zt-merge-backup\zt-before-acemoglu2010-settings.sqlite3`.

- The tree: economy D (the setting card, 原始设定) and its joint-concavity assumption; 变体一 economy E with its dynamic extension under it; 变体二 economy M with its applications under it, in chapter order (labour supply change with Assumption 1, net payoff and Hessian with example 1 and the minimum wage, green technology, Cobb–Douglas, CES, many sectors, endogenous labour supply with example 2, the oligopoly economy O); the task model of chapter 10 with its cost card; last the common frame of chapter 14 (equilibrium bias), which rests on economy D and Assumption 1 and points to the four economies.
- Moves: eight subtrees parked under chapter 15 and placed (Cobb–Douglas, CES, endogenous labour supply and economy O from the top level into economy M; the dynamic extension into economy E). 20 setting cards got their **起点**; the loose `依赖` lines of the assumption, Assumption 1 and example 1 moved into it. Facets name the role: `假设：经济 D 的联合凹`, `设定：经济 E`, `设定：经济 M`, `假设 1`, `设定：最低工资`, …
- The notation card now says that each variant's own symbols are in its setting card's table (the same letter can mean different things in different variants). The local tables stay, except three that copied their parent's word for word (example 1, the task cost, example 2): on the owner's go-ahead they were removed, each card now points to its parent's tables (`符号与术语见 […] 的符号表与术语表`), and `check-premises` dropped to 6 hits.
- Helper fix met on the way: `zt-digest call` exits 1 when one call of a batch fails, and `zt.mjs` threw away its output; it now returns it, so `writes()` reports the error lines (all three skill copies).

After: `check-settings` all in the setting part, as a dependency tree; `check-network` clean; `check-premises` 7 hits, all look-up rows of the local tables (`由 (14) 给出`) and chapter 10's roadmap; `self_sufficient` true; 462 Cards, no broken Link. The head's 结构 rebuilt.

## 11. The Acemoglu 2007 fold (2026-10-29)

The `zt-model-fold` skill folded `acemoglu2007equilibrium` on the live box at the owner's request (`/zt-model-fold acemoglu2007equilibrium`). The backup is `D:\zt-merge-backup\zt-before-acemoglu2007equilibrium-fold.sqlite3` (`VACUUM INTO`, integrity `ok`). The tree was not locked, and no other tree pinned its cards.

**Decision.** The book holds one model. Every Model Card keeps the same primitives: final-good firms that use the factors Z and L, a technology θ, the production function G, the cost C and the net production function F = G − C. Every Model Card also asks the same question: does an increase in the supply of a factor induce technology biased toward that factor? Economies C, M and O change one assumption of economy D (who chooses the technology), and each shows that the equilibrium technology maximizes F. The three bias sections answer the paper's question on that reduced form. The concept card of §1 is the verbal version of the formal model.

| Model Card | Model | Surviving head | Reason |
|---|---|---|---|
| `2/a` 经济 D | the model | `2/a` | first formal Model Card: competitive firms choose θ; equilibrium θ maximizes F |
| `1/b` 拟鞍点 (concept) | the model | `2/a` | verbal version of the formal model (§1): same F, same question; becomes the intuition chapter |
| `3/a` 经济 C | the model | `2/a` | same G, C, F; changes only who chooses θ (a research firm); Proposition 2 = same rule |
| `4/a` 经济 M | the model | `2/a` | same primitives; a monopolist sells the technology in intermediates; Proposition 3 = same rule |
| `5/a` 经济 O | the model | `2/a` | economy M with S technology producers; equilibrium θ is a fixed point of the same problems |
| `6/a` 相对均衡偏向 | the model | `2/a` | the paper's question on the same F, with θ = (A_Z, A_L) factor-augmenting |
| `7/a` 绝对均衡偏向 | the model | `2/a` | the paper's question on the reduced form "θ maximizes F" (Theorem 2) |
| `8/a` 强绝对均衡偏向 | the model | `2/a` | continues `7/a` (rests on its technology response (37)); Corollary 1 is about economy D |

(The table writes `/` for `|` in addresses.) The evidence settled every row, so the fold ran without a question to the owner.

**Procedure.** The §10 one-head-per-run script, with the book's key, in the book's order: 1|b (facet `直觉`), 3|a (`经济 C`), 4|a (`经济 M`), 5|a (`经济 O`), 6|a (`相对均衡偏向`), 7|a (`绝对均衡偏向`), 8|a (`强绝对均衡偏向`), all `core`. Four Links to Model Cards were masked during the retypes and restored after them.

**Result.** `acemoglu2007equilibrium/2|a` is the only Model Card of the book, with 83 members. The main line holds economy D's own chain (`2|a|a` to `2|a|a|9`) and seven leads `2|a|a|10` to `2|a|a|16` (直觉, 经济 C, 经济 M, 经济 O, 相对均衡偏向, 绝对均衡偏向, 强绝对均衡偏向). Each lead keeps its former parts as Side parts. The tree still holds 302 Cards, and `list_broken_links` is empty. `self_sufficient` is false: the seven leads have no rests-on line back to the setting. `zt-model-restructure` adds them, when it writes each chapter's `依赖` line. The head's text still describes economy D only. The restructure rewrites it.

**Next.** `zt-model-settings`, then `zt-model-restructure`, on `acemoglu2007equilibrium/2|a`.

### 11.1 The setting part as a dependency tree (2026-10-29)

`zt-model-settings` ran on `acemoglu2007equilibrium/2|a` on the live box at the owner's request. The backup is `D:\zt-merge-backup\zt-before-acemoglu2007equilibrium-settings.sqlite3` (`VACUUM INTO`, integrity `ok`).

Before the pass, the fold had left each former Model Card's setting in its chapter. Seven settings, Assumption 1, Assumption 2 (twice) and A1 were outside the setting part. The setting part had no notation card, and no setting had a **起点** section.

**The tree.** The setting card `2|a|a` (the common environment of economies D, C, M and O) is the only primitive. Its Sides are:

```
2|a|a 设定（共同环境）                      原始设定
├ a 符号表与读法                            new card
├ b 设定：概述 (引言)                       ← 2|a|a (the one-factor verbal version of §1)
├ c 设定：经济 D 的企业问题 (2)              ← 2|a|a
│   c|1 设定：净生产函数 (4) → c|2 假设 1
├ d 设定：经济 C                            变体一 ← 2|a|a, 净生产函数
│   d|1 企业问题 (6) → d|2 研究企业问题 (7);  d|a 假设 2
├ e 设定：经济 M                            变体二 ← 2|a|a, 假设 2, 市场出清 (3)
│   e|1 企业问题 (10)
├ f 设定：经济 O                            变体三 ← 经济 M
│   f|1 假设 2：经济 O
├ g 设定：相对均衡偏向                      ← 经济 C, M, O, 净生产函数
└ h 设定：绝对均衡偏向 (34), 定义 8–9         ← 净生产函数, 经济 D, C, M
    h|1 假设 A1 → h|2 设定：强绝对均衡偏向 (定义 12)
```

The firm problems, the research-firm problem and the net production function set up the agents' problems, so they became settings with `设定：…` facets. The monopolist's problem (12) and the oligopoly equilibrium (Definition 4) stay in their chapters, because each rests on a derived demand curve ((11), (15)). Each setting moved `alone`, so its derivation chain closed up in its chapter. The paper's reduction "each economy's equilibrium technology solves (34)" is a pointer from `h` to Propositions 1–3, not a premise.

**Text.**
- Every setting has a **起点**. Ten bare `依赖` lines moved into it.
- The setting card's symbol table and term table moved to the notation card. The notation card groups the symbols by block, gives each one's first card, and lists the derivations' auxiliary symbols ($G_1$, $h$, $B$, $x$, $H$, $Q$, $b$, $v$, $\mathcal D_C$). It notes that $B$ means two things and that $\Pi$ means two things. The variants' local tables stay.
- One stated assumption in the strong-bias proof was rewritten: 「假设 1 或 2 使 F 关于 Z 凹」 became 「由假设 1 或 2，F 关于 Z 凹」, and the proof now rests on both assumption cards.
- Seven chapter leads (直觉 through the intro setting, 经济 C, 经济 M, 经济 O, 相对均衡偏向, 绝对均衡偏向, 强绝对均衡偏向) got one `依赖 [[setting]] 的设定` line each in **关联**. The head's 结构 was rebuilt.

**After.** `check-settings` reports every setting in the setting part, as a dependency tree. `self_sufficient` is true. `check-premises` has one hit, a roadmap line of the economy M chain. The tree holds 303 Cards (302 plus the notation card), and `list_broken_links` is empty. `check-network` asks for chapter cards with **直觉** sections and result cards: that is the spine work of `zt-model-restructure`.

### 11.2 The intuition spine (2026-10-29)

`zt-model-restructure` ran on `acemoglu2007equilibrium/2|a` on the live box at the owner's request. The backup is `D:\zt-merge-backup\zt-before-acemoglu2007equilibrium-spine.sqlite3` (`VACUUM INTO`, integrity `ok`).

- **Spine.** The main line is the setting `2|a|a` and 8 chapter cards `2|a|a|1` to `2|a|a|8`, in the paper's order: 直觉 (§1), 经济 D, 经济 C, 经济 M, 经济 O (§2), 相对均衡偏向 (§3), 绝对均衡偏向 (§4), 强绝对均衡偏向 (§5). Chapter 2 is a new card. Before the pass, economy D's proof cards sat on the main line with no chapter card. The results are 13 cards: the two §1 conclusions, Propositions 1–4, the welfare result, the fixed-point result of economy O, and Theorems 1, 2, 4, 5 and 6 with Corollary 1. Each result's proof chain is a Side branch of the result. Examples, extensions, the CES case, the market-size intuition and the Le Chatelier remark are Side branches of their result or chapter. The factor-price derivations of economies D, C and M are Side branches of their chapters, after the result they use.
- **Mover.** `tree-mover.mjs` made 132 moves (66 cards). No card rests on a later card.
- **Text.** DeepSeek V4.1 Flash (official source, effort high) drafted 35 cards in 8 jobs under one Haiku manager:
  - 8 chapter cards (**直觉**, **本章结果**, **承上启下** with one `依赖` line);
  - the result cards (`结果：` titles and facets; **结果**, **为什么**, **证明**);
  - the asides.

  All 8 conversations were deleted and the workspace removed. `check-drafts` found no lost formula or equation number. Four small fixes were made by hand: 本模型 → 本章 in two copied cross-book lines, a `目的：` line on two side derivations, and chapter 8's `依赖` line (chapter 7's result (37), not a 设定). Two lines that opened with 「假设 1 成立时」 now open with 「在假设 1 下」, so they use the assumption without stating it. Derivation cards kept their text.
- **Head and notation.** The Model Card's title, question and summary now cover the whole model (the four economies, the reduced form (5)/(34), Theorems 1, 2 and 4–6). Formula (5) is kept. The 结构 lists the main line and one level under each main card. **分部** has three parts: 直觉与构件 (from the setting), 四种经济 (chapter 2) and 均衡偏向 (chapter 6). The notation card gained the legend **怎样读卡片上的记号**.

**After.**
- `check-model`: `self_sufficient` is true.
- `check-settings`: every setting is in the setting part, as a dependency tree.
- `check-network`: `network: clean` (10 blocks, 3 layers). Chapter 5 hangs under chapter 4; the other chapters hang under the setting, as their rests-on Links say.
- `check-premises`: one hit, the roadmap line of the economy M chain.
- `list_broken_links` is empty. The tree holds 304 Cards (302 + the notation card + the chapter 2 card).

## 12. The Modern VAT fold (2026-10-29)

The `zt-model-fold` skill folded `ebrill2001modern` on the live box at the owner's request (`/zt-model-fold ebrill2001modern`, live box confirmed in the same session). The backup is `D:\zt-merge-backup\zt-before-ebrill2001modern-fold.sqlite3` (`VACUUM INTO`, integrity `ok`). The tree was not locked. Two of its cards are pinned by other locked trees (`14/a/b` by `becker2025division`, `22/d/2/a` by `acemoglu2010labor`); the fold moved neither.

**Decision.** The book is a policy book. Most of its 29 Model Cards ask their own question with their own primitives, so most stay alone. Four pairs are one model each: a verbal or framing card joins its formal model, and two extensions join the base model whose setting they keep. The book holds 25 models.

| Model Card | Model | Surviving head | Reason |
|---|---|---|---|
| `2/a` 抵扣机制 (concept) | invoice-credit VAT | `3/a` | verbal version of `3/a`: the same two-stage example (10 + 30 = 40) and the same question; becomes the intuition chapter |
| `10/a` 效率收益的收入检验 (concept) | VAT revenue test | `11/a` | the framing of §3 that `11/a` estimates (revenue ratio with and without a VAT); facet `理论框架` |
| `26/a` 税率差异化模型 | indirect-tax redistribution | `24/a` | keeps `24/a`'s two types, two goods and single government goal; adds the revenue need g and the administration cost γ; its bound falls to `w1/w2` at g = γ = 0 |
| `41/a` 流转税模型 | VAT threshold | `40/a` | keeps `40/a`'s balance condition and (11.1); adds a turnover tax below the threshold; falls to (11.1) at t = 0, A' = C' = 0 |

Every other Model Card is a model of its own: `14/a`, `15/a`, `22/a`, `23/a`, `25/a`, `28/a`, `29/a`, `30/a`, `31/a`, `32/a`, `32/b`, `33/a`, `33/d`, `36/a`, `37/a`, `47/a`, `48/a`, `49/a`, `50/a`, `51/a`, `53/a`. The survey's `read` pairs among them are comparisons or borrowed results (for example `49/a` measures readiness against `48/a`'s seven conditions, `51/a` gives the deterrence mechanism behind `50/a`'s audit design, `14/a` meets `11/a` only at the Δ of note 53). Three pairs needed judgement, and the owner decided them. `32/a` (the pure interest rate splits the bank spread) and `32/b` (cash-flow VAT taxes the spread without it) are two models. `33/a` (cash-flow VAT on pure insurance) is its own model; it shares `32/b`'s cash-flow rule as a part, and that rule stays a cross-model premise. `36/a` (lifetime-consumption framework) and `37/a` (progressivity by benchmark and base structure) are two models.

(The table writes `/` for `|` in addresses.)

**Procedure.** The §11 one-head-per-run script with the head given by title: 2|a into 3|a (facet `直觉`), 10|a into 11|a (`理论框架`), 26|a into 24|a (`扩展：筹资需要与管理成本`), 41|a into 40|a (`扩展：起征点以下的流转税`), all `core`. Six Links to Model Cards were masked during the retypes and restored after them.

**Result.** `model-survey.mjs` lists exactly the 25 decided heads. The four folded models are `3|a` (13 members, lead `3|a|a|5`), `11|a` (14, lead `11|a|a|7`), `24|a` (22, lead `24|a|a|6`) and `40|a` (20, lead `40|a|a|6`); each lead keeps its former parts as Side parts. The tree still holds 1420 Cards, and `list_broken_links` is empty. `40|a` is self-sufficient. The other three report the new lead as unanchored (no rests-on line back to the setting yet); `zt-model-restructure` adds it. `11|a` also rests on `6|b|a|1` through the former `10|a` mechanism (`11|a|a|7|a|1`): a borrowed result of the Diamond-Mirrlees model, kept as a cross-model premise. The heads' texts still describe their base models only; the restructure rewrites them.

**Next.** `zt-model-settings`, then `zt-model-restructure`, on `ebrill2001modern/3|a`, `ebrill2001modern/11|a`, `ebrill2001modern/24|a` and `ebrill2001modern/40|a`.

### 12.1 The setting parts as dependency trees (2026-10-29)

`zt-model-settings` ran on the four folded models of `ebrill2001modern` (`3|a`, `11|a`, `24|a`, `40|a`) on the live box at the owner's request. The backup is `D:\zt-merge-backup\zt-before-ebrill2001modern-settings.sqlite3` (`VACUUM INTO`, integrity `ok`).

**Faults before.** None of the four had a notation card or a **起点** section. Each folded Model Card had brought its setting into its chapter (`3|a|a|5|a`, `11|a|a|7|a`, `24|a|a|6|a`, `40|a|a|6|a`). In `24|a` the assumption card `24|a|a|1` was the first step of the proof chain. `11|a|a|2` carried a copy of the setting's symbol and term tables.

**Moves.** A notation card (`符号表与读法`) became the first Side of each setting card. Each misplaced setting moved `alone` into the setting part, so its derivation chain closed up and stays in its chapter:

| Model | Setting part after the pass |
|---|---|
| `3/a` | `3/a/a` 设定 (原始设定) · `a` 符号表 · `b` 设定：概述 (直觉), the verbal setting of the former `2/a` (原始设定, no symbols) |
| `11/a` | `11/a/a` 设定 (原始设定) · `a` 符号表 · `b` 设定：理论框架, the §3 framework of the former `10/a` (原始设定, no symbols) |
| `24/a` | `24/a/a` 设定 (原始设定, Box 7.4) · `a` 符号表 · `b` 假设 (← 设定) · `c` 设定：筹资需要与管理成本, the Box 7.5 variant (← 设定, 假设) |
| `40/a` | `40/a/a` 设定 (原始设定, §11.2) · `a` 符号表 · `b` 设定：起征点以下的流转税, the variant (← 设定) |

(The table writes `/` for `|` in addresses.) The proof chain of `24/a` closed up: its steps are now `24/a/a/1` to `24/a/a/4`, and the extension lead is `24/a/a/5`.

**Text.** Every setting card has a **起点** section (`原始设定`, or `依赖 [[…]] 的设定：…` lines; each variant names what it changes). The setting cards' symbol and term tables moved to the notation cards, and each notation card lists every symbol with the card where it is first used, in sections for the base setting, the variant and the auxiliary symbols of the derivations (for example N, D, M and z_0 in `40/a`, the regression coefficients and the fitted values in `11/a`). The notation card of `24/a` states that `s` changes meaning between the two boxes: a subsidy rate in Box 7.4 and a net tax rate of the opposite sign in Box 7.5. Three symbols lost their definition with the tables, so one sentence now defines each where it is first used: Y, OPEN and AGR in `11/a/a`, w_1 and w_2 in `24/a/a`, z* in `40/a/a`. The copied tables left `11/a/a/2`, which already rests on the setting. Each head's **结构** list was rebuilt from the tree: the setting part first, then the main line with its Side parts.

**Result.** `check-settings.mjs` prints `settings: all in the setting part, as a dependency tree` for all four models, and `check-premises.mjs` finds no premise used before its card. The tree holds 1424 Cards (1420 and the four notation cards), and `list_broken_links` is empty. `40/a` is self-sufficient. The leads `3/a/a/5`, `11/a/a/7` and `24/a/a/5` are still unanchored, and `11/a` still rests on `6/b/a/1` (a cross-model premise, §12). `check-network.mjs` also advises that no chapter has a result card. Both belong to `zt-model-restructure`.

**Next.** `zt-model-restructure` on the four heads.

### 12.2 The intuition spines (2026-10-29)

`zt-model-restructure` ran on the four folded models of `ebrill2001modern` on the live box at the owner's request. The backup is `D:\zt-merge-backup\zt-before-ebrill2001modern-spine.sqlite3` (`VACUUM INTO`, integrity `ok`).

**Spines.** Each main line is the setting and two or three chapter cards in the book's order. Five chapter cards are new (`3/a/a/2`, `11/a/a/2`, `11/a/a/3`, `24/a/a/1`, `40/a/a/1`); the others are the leads the fold made.

| Model | Chapters | Results (proof chain) |
|---|---|---|
| `3/a` | 1 直觉 (the former `2/a`), 2 两阶段发票抵扣 | `1/a` (mechanism); `2/a` (two derivations), with the example, the merger, the n-stage extension and the contrast with the retail sales tax as asides |
| `11/a` | 1 理论框架 (the former `10/a`), 2 交互项回归, 3 逐国预测 | `1/a` (mechanism); `2/a`, `2/b`, `2/c` (regression columns, no chain); `3/a` (two derivations), robustness as aside |
| `24/a` | 1 专栏 7.4 的上限, 2 筹资需要与管理成本 | `1/a` (three derivations); `2/a` (four), `2/b` (one) |
| `40/a` | 1 最优起征点, 2 起征点以下的流转税 | `1/a` (two), `1/b` (one); `2/a` (two), `2/b` (three) |

(Addresses relative to each setting card `…|a|a`; the table writes `/` for `|`.) Each result's proof chain is its first Side branch, and the head's former Side cards (the contrast, the EU case, the two limits of `40/a`) became asides of their chapter. `tree-mover.mjs` made 130 moves (65 cards); no card rests on a later card.

**Text.** DeepSeek V4.1 Flash (official source, effort high) drafted 43 cards in 9 jobs under two Haiku managers: the 10 chapter cards (**直觉**, **本章结果**, **承上启下** with one `依赖` line), the result cards (`结果：` titles and facets; **结果**, **为什么**, **证明**) and the asides. All 9 conversations were deleted and the workspace removed. `check-drafts` found no lost formula or equation number. Two fixes were made by hand: a `目的：` line on the n-stage derivation, and one sentence of `24/a` chapter 2 that had the sign of the net tax rate s reversed. Derivation cards kept their text.

**Anchors.** Chapter 1 of `3/a` and of `11/a` rested on a verbal setting marked `原始设定`, so they had no path to the setting card. As in Acemoglu 2007 (`2/a/a/b`), the verbal setting `3/a/a/b` and the framework `11/a/a/b` now rest on their formal setting. `11/a` chapter 3 now rests on chapter 2, whose coefficients it uses.

**Heads and notation.** Each Model Card's question and summary now cover the whole model (both boxes of `24/a`, both chapters of `40/a`, the framework and the two regression chapters of `11/a`, the verbal and the formal chapter of `3/a`); formula (11.1) of `40/a` is kept. The 结构 lists the main line and one level under each main card. No model has five chapters, so none has **分部**. Each notation card gained the legend **怎样读卡片上的记号**. The 结构 of `11/a` takes in `6/b/a/1` (Diamond-Mirrlees) with a `共用` line, so zt reports it as a tolerated premise, not a closure fault.

**After.**
- `check-model`: `self_sufficient` is true for all four.
- `check-settings`: every setting is in the setting part, as a dependency tree.
- `check-network`: `network: clean` for `3/a`, `24/a` and `40/a`. `11/a` reports one forward thread: its chapter 1 mechanism rests on the shared part `6/b/a/1`, and `get_model` files shared parts in an aside block after the chapters. No move inside this model can fix it; the Link is a cross-model premise and stays. Whether a rests-on Link to a tolerated shared part should count as forward is a question for zt's block builder.
- `check-premises`: no premise used before its card.
- `list_broken_links` is empty. The tree holds 1429 Cards (1424 + the five chapter cards).

### 12.3 The other formal models, and why the concept models stay (2026-10-29)

The owner left the rest of the book to my judgement. After the four folded models, 21 models of `ebrill2001modern` had had neither the settings pass nor the restructure.

**Which models.** 17 of the 21 are concept models (`15/a`, `22/a`, `23/a`, `25/a`, `28/a`, `29/a`, `30/a`, `31/a`, `32/a`, `33/d`, `36/a`, `37/a`, `47/a`, `48/a`, `49/a`, `50/a`, `51/a`). Each has three or four parts on one main line (设定 → 机制 → 结论), and `check-model` already reports each as self-sufficient. zt defines a concept model as the Model Card and at most four parts, so a notation card and chapter cards would push each one past its level, and the main line already is the short spine the restructure aims at. They stay as they are. Their only network advice is that the main-line cards have no **直觉** section, and a concept model's cards are plain-language in full. The four formal models (`14/a`, `32/b`, `33/a`, `53/a`) went through both passes, on the live box. The backups are `D:\zt-merge-backup\zt-before-ebrill2001modern-formal-settings.sqlite3` and `…-formal-spine.sqlite3` (`VACUUM INTO`, integrity `ok`).

**Settings.** Each setting card has a notation card (`符号表与读法`, with the legend **怎样读卡片上的记号**) and a **起点** section. Its symbol and term tables moved to the notation card. One sentence (`本设定的记号：…`) now defines each symbol that was defined only in a table, for example τ, L, r_B, r_L and p of `32/b`, and i, z, α, λ and e of `53/a`. The assumption card of `14/a` (`假设：规则随环境而定`) left the main line for the setting part. So did the cash-flow rule of `32/b` (`设定：现金流量规则`), which `33/a` shares with a `共用` line. The regression card of `14/a` lost its copied tables. Its notation card gained the rows r⁺(m) and r⁻(m).

**Spines.**

| Model | Chapters | Results (proof chain) |
|---|---|---|
| `14/a` | 1 简化式框架, 2 税率弹性, 3 环境与结构变量 | ch. 1: two framework derivations, no result card; ch. 2: the regression, the elasticity 0.70 (one step); ch. 3: openness and capacity, rate range (each with its derivation as aside) |
| `32/b` | 1 现金流量增值税 | the tax on unregistered spreads (four steps), two examples, the limit |
| `33/a` | 1 保险人的净税额, 2 被保险人的两种情形 | the net tax 20% of value added (one step); the consumer case (one), the registered case (two), the comparison, the limit |
| `53/a` | 1 退税基准 | the refund ratio (four steps), the example, the comparative statics |

The limitation card `14/a/b` stays a Side card of its head: the locked tree `becker2025division` pins it (`card_locked`), so it was not moved. DeepSeek V4.1 Flash (official source, effort high) drafted 28 cards in 7 jobs under one Haiku manager. One job needed its retry. All 8 conversations were deleted and the workspace removed. `check-drafts` found no lost formula. Five derivation asides got a `目的：` line by hand. Each head's 结构 was rebuilt; the summaries already described one model each.

**Tool fixes met on the way.**
- `check-settings.mjs` reported the shared cash-flow rule as a setting "filed in null" in `33/a`. It now skips a part shared from another model, whose home model checks it (test added).
- `tree-mover.mjs` saved its step before advancing it, so a resume repeated the last move: after the `card_locked` stop, it tried to park a card into a slot under itself. It now saves the next step.
- The forward thread of `11/a` (§12.2) was a defect of zt's block builder. A separate session fixed it (zt 0.46.1, commit 592b9b3: a part shared from another model is a premise block before the setting). It is merged into main, and `11/a` is clean with no card changed.

**After.** All eight formal models of the book (`3/a`, `11/a`, `14/a`, `24/a`, `32/b`, `33/a`, `40/a`, `53/a`) report `self_sufficient` true, `network: clean`, `settings: all in the setting part, as a dependency tree`, and no premise used before its card. `list_broken_links` is empty. The tree holds 1440 Cards (1429 + four notation cards + seven chapter cards).
