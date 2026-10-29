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
