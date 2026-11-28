# ZET 一等公民多坐标卡片架构设计方案（最终提案）

Owner: Xingtong Chen

| 字段 | 内容 |
|---|---|
| **状态** | **已批准——最终提案**（`MC1`–`MC4` 已于 2026-11-27 经 Xingtong Chen 批准；已就绪可转入实施计划与任务拆解；暂未修改任何代码、`CONTEXT.md` 或正式卡片盒） |
| **日期** | 2026-11-27 |
| **仓库基线** | `E:\zet` 检出版本 `22a98736`（`zt` 0.71.0；同时对照审计了 0.70.1 发布版本 `43417f79`） |
| **正式库实盘核查** | 对 `D:\zt\zt.sqlite3`（修订号 `148217`）执行只读 SQLite 审计：`62,322` 张存活卡片、`1,459` 条已退役身份、`0` 棵现存关联树 / `0` 条落点 / `0` 条树内关联、`20` 个已签发永久句柄、`122` 条已退役落点、`0` 把活跃树锁（`24` 条历史锁记录）、`166,710` 条 `applied_ops` 操作回执、`0` 处缺失推断归属父节点、**`303` 组带空洞的稀疏旁支标签** |
| **设计评审与审计** | 由 **Astra**（`gpt-6.2-astra`，高推理力度，标准档；会话 `01a22099-e605-7993-ab29-1117f08cbba1`）完成架构与子系统联动分析，由 **Gemini** 对照源码独立审计并修正收敛（见 §10） |
| **英文版** | [multi-coordinate-cards-proposal.md](multi-coordinate-cards-proposal.md)（共享决策编号 `MC1`–`MC4`、章节编号 `§0`–`§11` 与推进阶段 `P1`–`P5`） |
| **演进 / 取代** | [multi-tree-coordinates-proposal.zh.md](multi-tree-coordinates-proposal.zh.md)、[connection-trees-free-proposal.md](connection-trees-free-proposal.md)、[coordinates.md](wiki/coordinates.md) |

---

## 0. 核心摘要与已批准决策

### 核心契约

> **一张卡片是一份具有不可变身份（`card_id`）的独立文档，拥有一个非空坐标集合（坐标数 `>= 1`）。每一个坐标都代表这张卡片在某一棵树中的一个位置。同一张卡片既可以跨多棵树拥有多个坐标，也可以在同一棵树内拥有多个坐标（包括树根及其直接后继）；同时每张存活卡片精确指定一个归档坐标（Filing Coordinate），用以维系文献溯源、工作区归属与各类学术结构报告的确定性。**

在当前版本（zt 0.70–0.71）中，ZET 把坐标割裂成了两个不对等的世界：普通主题树与文献树卡片只拥有一个隐含在 `cards.location` / `cards.citation_key` 中的单一位置，而关联树（`~n`）则外挂了一张 `placements` 表，强制 `UNIQUE(host_tree_key, card_id)`（每棵树至多一个坐标），主题树又额外外挂了一套 `<loc>+k` 主题旁挂（`EdgeKind::Attach`）。这种实现并不自然：把卡片放到另一棵树、或放到同一棵树的两个位置，都被当成特殊外挂而非卡片与树本身的一等能力。

本方案以**一等公民多坐标卡片**统一整套架构：

1. **统一坐标表（`coordinates`）：** 无论文献树、主题树还是关联树，卡片在树中的每一次出现都是一条显式、可独立寻址的坐标记录（`coord_id`），拥有独立的父坐标、`Direct`/`Side` 边类型、显式正整数 `slot`（完整保留历史稀疏空洞）、可选位置短注（`gloss`）以及独立的子节点子树。
2. **原生支持同树多坐标与自祖先出现：** 移除 `UNIQUE(tree_id, card_id)`。卡片 `X` 可以同时拥有 `1/1`、`1/4|b` 与 `2/3|a` 三个坐标，甚至出现在自身之下（`X(c1) -> Y(c2) -> X(c3)`）。编辑 `X` 正文会全局同步生效；在 `1/4|b` 下新增子节点只影响该坐标，不污染其他坐标。
3. **提交期硬性保障 `>= 1` 坐标底线：** 每张存活卡片必有且仅有一个位于其所属 `Workspace` 内的归档坐标（`filing_coord_id`），由 SQLite 延迟复合外键在 `COMMIT` 时强制校验，确保任何解挂、批处理或删树都绝不会造出 0 坐标孤卡或静默转移工作区归属。
4. **零歧义与学术结构零回归：** 每棵树保持单一无碰撞坐标空间；文献 BibTeX、模型、数学单元、术语卡、引文卡、枢纽卡、写入门禁、`zt-check` 与 `zt-digest` 均通过唯一归档坐标确定性求值，不受任何辅助坐标干扰。

### 已批准的产品决策（`MC1`–`MC4`）

| 编号 | 决策议题 | 已批准方案 |
|---|---|---|
| **`MC1`** | **关联树（`~n`）与主题旁挂（`<loc>+k`）的去留** | **保留 `~n` 作为统一 `coordinates` 引擎之上的轻量自由树角色**（支持无标题根、树内关联 `coordinate_edges` 与跨树投影 `crossings`），并**退役主题旁挂 `<loc>+k`（`EdgeKind::Attach`）**——因为主题树现已直接在常规 `Direct`/`Side` 槽位上原生支持外部卡片与重复坐标。 |
| **`MC2`** | **同卡自祖先出现与根节点多坐标** | **允许同卡自祖先出现**（`X(c1) -> Y(c2) -> X(c3)`），并**允许树根卡作为非根节点在其他树或自身子树中再次出现**；同时保持**每张卡至多只作为一棵树的定义根节点**。 |
| **`MC3`** | **文献树的坐标准入规则** | **v1 阶段文献树继续对外来卡片保持封闭**，但允许**本文献树自身已归档卡片（含文献根）在树内拥有重复坐标**；主题树与关联树则允许放入全库任意存活卡片。 |
| **`MC4`** | **归档坐标与单坐标空间护栏** | **每张存活卡片保留一个指定的归档坐标（`filing_coord_id`）**，并增加一条**简洁的插队护栏**（禁止将辅助坐标直接插入活跃的 `model`/`term` 归档链中间），从而在无需维护第二套“编辑父子图”的前提下，保证每棵树内任意路径字符串至多唯一指向一张卡片。 |

---

## 1. 动机、现状诊断与正式库实盘基线

### 1.1 当前 0.70–0.71 关联树实现的四处结构不自然

zt 0.65–0.71 留下了两项值得继承的好基础设施：不可变卡片身份（`card_identity`，删除即退役而非抹除）与持久化的出现位标识（`placement_id`，承载位置短注与树内关联）。但它在四个环节破坏了自然性：

1. **节点表示严重不对称：** 普通主题卡和文献卡在回填时仅获得 `card_identity`（[coord.rs](file:///e:/zet/src/coord.rs#L474)），在 `placements` 中**没有任何行**，其树形拓扑完全靠 `cards.location` / `cards.citation_key` 字符串算术反推（[main.rs](file:///e:/zet/src/main.rs#L3003)）。只有关联树（`~n`）和主题旁挂（`+k`）才写入 `placements`。
2. **人为禁止同树多坐标（`UNIQUE(host_tree_key, card_id)`）：** 在同一棵树里第二次放置 `X` 会被静默当成“返回已有站点”（[connection.rs](file:///e:/zet/src/connection.rs#L1354)），批量挂载也会按 `card_id` 去重（[connection.rs](file:///e:/zet/src/connection.rs#L1451)），导致同一张卡无法在一棵树的两条分支中同时出现。
3. **主题树无法原生容纳外部或重复坐标：** 要把外部卡片放到主题树里，必须使用按 `anchor_card_id` 索引的外挂语法 `<loc>+k`（[coord.rs](file:///e:/zet/src/coord.rs#L298)、[coord.rs](file:///e:/zet/src/coord.rs#L1144)），它不能拥有自己的 Direct/Side 后继，且锚点迁移时容易触发同卡碰撞报错（[coord.rs](file:///e:/zet/src/coord.rs#L1516)）。
4. **计数、动词与寻址割裂：** `places` 只统计 `placements` 行数（[connection.rs](file:///e:/zet/src/connection.rs#L642)），对所有主题/文献卡都少算了主坐标 1 次；关联树原生卡必须走专门的 `promote_card` 才能进入主题树；`TreeShape::placement_of_card`（[coord.rs](file:///e:/zet/src/coord.rs#L1031)）假设每卡每树至多一个站点。

### 1.2 正式卡片盒 `D:\zt` 实盘基线

对 `D:\zt\zt.sqlite3`（修订号 `148217`，守护进程健康、`coordinates: ok`、锁完整性正常）执行只读事务核查的结果如下：

| `D:\zt` 实盘核查项（修订 `148217`） | 数值 | 对架构与迁移的直接约束 |
|---|---:|---|
| 存活卡片（live Cards） | 62,322 | 迁移、快照加载与 `zt status` 核查必须在 6 万卡规模下保持低延迟。 |
| 已退役卡片身份（retired identities） | 1,459 | 必须原样保留历史身份、最后地址与退役修订号。 |
| 现存关联树 / 落点 / 树内关联 | 0 / 0 / 0 | **最佳统一窗口：** 当前无存量关联树或 `+k` 旁挂数据，可干净退役 `+k`（`MC1`）。 |
| 已签发永久句柄 / 已退役落点 | 20 / 122 | 历史句柄与已退役书签 ID 真实存在，迁移必须完整保留。 |
| 活跃树锁 / 历史锁记录 | 0 / 24 | 正式库目前无活跃锁；发布前必须构造活跃锁与含数据关联树的专门测试夹具。 |
| 已提交操作回执（`applied_ops`） | 166,710 | 历史回执哈希与幂等重放语义必须原样保持。 |
| 主题/文献树中缺失推断父节点的卡片 | 0 | 全部现有卡片均可无损物化为完整有根树。 |
| **带空洞的稀疏旁支兄弟组**（如仅有 `\|a`、`\|c`） | **303** | **关键发现：** 历史删除在“紧缩旁支会让断链错落到重命名兄弟上”时会刻意跳过紧缩（[main.rs](file:///e:/zet/src/main.rs#L4331)）。统一坐标表必须存储显式正整数 `slot`，绝不可强制 `1..N` 致密序号（[coord.rs](file:///e:/zet/src/coord.rs#L1098)）。 |

---

## 2. 概念本体与 `CONTEXT.md` 词汇更新

下列规范术语将补充并更新 `CONTEXT.md`，同时完整保留既有全部术语（`Card`、`Regular Card`、`Literature Card`、`Citation key`、`Location`、`Topic`、`Pointer`、`Edit caret`、`Session`、`Link`、`Broken link`、`Reverse link`、`Direct successor`、`Side successor`、`Workspace`、`Zone`、`Access`、`MCP server`、`Lock`、`Pinned target`、`Model Card`、`Model part`、`Term Card`、`Cited Card`、`Hub Card`）。

| 术语 | 定义 | 禁用近义词 |
|---|---|---|
| **Card（卡片，`card_id`）** | 一份作者撰写的 Markdown 文档（标题、正文、可选 BibTeX、`CardType`、模型/数学/枢纽/术语元数据），拥有不可变 `card_id`、保留的 `row_id`、唯一所属 `Workspace`（`human` \| `agent`）、状态（`live` \| `retired`），以及存活期唯一的**归档坐标（`filing_coord_id`）**。 | 笔记条目、记录 |
| **Tree（树，`tree_id`）** | 一棵有根 Folgezettel 树，拥有不可变 `tree_id`、角色 `role`（`literature` \| `topic` \| `connection`）、所属 `Workspace`、公开键（`<CitationKey>`、`<n>` 或 `~<n>`）以及唯一根坐标（`edge = 'root'`）。 | 文件夹、图 |
| **Coordinate（坐标，`coord_id`）** | 某张卡片在某棵树中的一个具名出现位置：`(coord_id, tree_id, card_id, parent_coord_id, edge, slot, gloss)`。每个坐标拥有独立的直接后继与旁支后继，以及可选的位置短注 `gloss`。每张存活卡片至少拥有 1 个坐标（`>= 1`），且可跨多棵树或在单棵树内拥有多个坐标。其展示路径（`1/2\|a`、`Hart1995/3`、`~7/2`）由 `(parent_coord_id, edge, slot)` 确定性派生。 | 副本、引用、站点（不再作为割裂概念） |
| **Filing Coordinate（归档坐标，`filing_coord_id`）** | 每张存活卡片唯一指定的一个主归属坐标，必位于该卡片所属 `Workspace` 的某棵树中。它是文献溯源、模型/数学/术语/引文结构计算、枢纽覆盖、写入门禁上下文、树锁成员封存与规范引用的唯一权威锚点。 | 主副本、母节点 |
| **Secondary Coordinate（辅助坐标）** | 卡片除归档坐标之外的任意其他坐标（`coord_id != card.filing_coord_id`），可位于同一棵树，也可位于任意其他树（不限工作区）。移动、加注或解挂辅助坐标永不修改卡片正文，也不改变任何学术结构报告。 | 别名、挂载副本、影子卡 |
| **Connection tree（关联树，`~<n>`）** | 角色为 `connection`、建立在同一套统一 `coordinates` 表上的树。额外提供三项轻量能力：可无标题的根节点（不参与主题枢纽/索引表规则）、树内任意两坐标间的**树内关联（`coordinate_edges`）**，以及向主题树/文献树的**投影（`crossings`）**。 | 阅读树、关系树 |
| **Tree connection（树内关联，`coordinate_edges`）** | 由关联树编辑者在树内任意两个不同坐标（`from_coord_id <> to_coord_id`，允许是同一张卡的两个不同坐标）之间记录的局部关联（`connection_id`）：可为中性或有向，带可选自由标签 `relation_label`（1–32 字符）、可选规范 `relation_kind` 与可选纯文本 `note`（<= 400 字符）。随 `coord_id` 自动跟随重排，永不改写卡片正文或链接索引。 | 链接（Link）、边 |
| **Missing coordinate（缺失坐标）** | 目标 `card_id` 已退役（`status = 'retired'`）后在外部宿主树中留存的辅助坐标。保留原 `coord_id`、路径、短注、子坐标与树内关联；后续在旧路径新建的卡片永不复活它；由宿主拥有者通过重绑（逐条处置关联）或解挂修复。 | 断链、死节点 |
| **Projection（投影，`crossings`）** | 某棵关联树 `~n` 落在当前阅读的主题树或文献树 `T` 平面内的部分，以 `coord_id` 为键：平面内坐标、指向最近平面内祖先的结构线（含平面外 `hops` 跳数）、平面内树内关联（含连接同一卡片两个不同坐标的线）及按坐标统计的平面外出口（`exits`）。 | 叠加层、复制 |

*（已退役概念：根据 `MC1`，**主题旁挂 / `<loc>+k`** 由主题树原生 Direct/Side 多坐标取代。）*

---

## 3. 六条硬性结构不变量

### 不变量 1 —— 至少一个坐标的提交期硬底线（`|coords(card)| >= 1`）
- 存活卡片必有 `filing_coord_id IS NOT NULL`；已退役卡片必有 `filing_coord_id IS NULL`。
- 通过 `DEFERRABLE INITIALLY DEFERRED` 复合外键 `card_identity(filing_coord_id, card_id, owner_workspace)` → `coordinates(coord_id, card_id, host_workspace)`，再配合 `coordinates(tree_id, host_workspace)` → `trees(tree_id, workspace)`，在数据库 `COMMIT` 时刻硬性保证：
  1. 绝不可能提交出 0 坐标的存活卡片；
  2. `filing_coord_id` 必然指向**同一张卡**位于其**所属工作区**某棵树中的真实坐标；
  3. 任何单次操作、批处理、子树解挂或删树，都不可能删光卡片在所属工作区的全部坐标而只留跨工作区坐标（杜绝静默工作区所有权转移）。
- **触发器与 PRAGMA 纪律：** `open_db`（[main.rs](file:///e:/zet/src/main.rs#L2659)）与 `initialize_database`（[main.rs](file:///e:/zet/src/main.rs#L1482)）都必须开启 `PRAGMA foreign_keys = ON`。由于 SQLite 的 `CHECK` 约束不可延迟，原有 `identity_after_insert` 触发器（[coord.rs](file:///e:/zet/src/coord.rs#L379)）必须退役，改由统一的事务内卡片+坐标创建器在 `BEGIN IMMEDIATE` 中原子写入（配合写入代次 epoch 2 围栏）。

### 不变量 2 —— 单一坐标空间不变量（零双义路径，`MC4`）
- 在任意修订号的任意一棵树内，**每一个派生坐标路径字符串（如 `1/2`、`Hart1995/3|a`、`~7/2`）至多唯一对应一个 `coord_id`（从而唯一对应一张卡片）。**
- 绝不允许在同一棵主题树或文献树里出现“阅读路径 `1/2` 指向卡片 X，而归档地址 `1/2` 仍指向卡片 Y”的分裂双义状态。
- 三种树角色共用完全一致的路径派生算术：
  - `edge = 'root'`：`<CitationKey>`（文献树）、`<n>/0`（主题树）或 `~<n>/0`（关联树）。
  - `edge = 'direct'`：根的直接后继为 `<tree>/1`；数字尾段递增（`1/2 -> 1/3`）；旁支尾段追加 `|1`（`1/2|a -> 1/2|a|1`）。每个 `parent_coord_id` 至多一个 Direct 子坐标。
  - `edge = 'side'`：`<parent_path>|<number_to_side_label(slot)>`，直接使用**存储的**正整数 `slot`（`1 -> a`、`2 -> b`、`27 -> aa`），按 `(tree_id, parent_coord_id, slot)` 唯一索引，天然保留稀疏空洞（完整兼容 `D:\zt` 的 303 组稀疏旁支）。树根不接 Side 子坐标。
- **对主题/文献树中辅助坐标的影响：**
  - 将辅助坐标挂在空闲的 Direct 末端或空闲的 Side 槽位上，移动 **0** 个既有坐标 → **0** 条链接改写。
  - 若将辅助坐标插队到既有主题/文献归档坐标*之前*（例如插入到 `1/2` 之前令原 `1/2 -> 1/3`），则走标准移动/插入映射预览、树锁门禁与链接改写检查，保持路径与链接 100% 一致。

### 不变量 3 —— 以 `coord_id` 为键的拓扑与允许同卡自祖先出现（`MC2`）
- 所有树遍历、大纲构建、移动防环校验、子树枚举、GUI 节点标识与书签均以 **`coord_id`** 为键，绝不以 `card_id` 判重或递归展开。
- **允许同卡自祖先出现且无环：** `X(c1) -> Y(c2) -> X(c3)` 是完全合法的 3 坐标树。试图把 `c1` 移到 `c3` 之下会构成 `coord_id` 环而被拒绝。`c1` 与 `c3` 的子节点完全独立。

### 不变量 4 —— 根节点多重性规则（`MC2`）
- 每棵树有且仅有**一个**根坐标（`UNIQUE(tree_id) WHERE edge = 'root'`）。
- 每张卡片至多只是**一棵树的定义根节点**（`trees.root_card_id UNIQUE`，即其 `filing_coord_id`），以此保障 BibTeX 文献身份、主题 `<n>/0` 身份、树锁根与整树删除语义的唯一性。
- 该树根卡可以在其他合法树中、甚至在自身子树中任意多次作为**非根辅助坐标**出现。非根位置上的树根卡显示为单个带“打开原树”入口按钮的节点，渲染、加锁、播种与导出均绝不递归展开其原树。

### 不变量 5 —— 三类树角色准入与退役 `+k` 旁挂（`MC1`、`MC3`）
- **主题树（`<n>/0`）与关联树（`~<n>`）：** 任意 Direct/Side 槽位均可接收全库任意存活卡片（跨工作区、根节点或普通卡、外来或本土、单次或重复）。
- **文献树（`<CitationKey>`）：** v1 阶段允许**本文献树自身已归档卡片（含文献根）在树内拥有重复坐标**，同时继续对外部卡片保持封闭，确保文献消化与便携导出的出处纯净（未来如需书内跨书批注，可在不改表结构的前提下作为显式选项开放）。
- **退役主题旁挂 `<loc>+k`：** 新写入彻底移除 `EdgeKind::Attach` 与 `+k` 语法；若迁移遇到含 `+k` 数据的历史测试/外部归档，则确定性地转为锚点归档坐标下的常规 Side 坐标（`slot = max_slot + 1, ...`）。

### 不变量 6 —— 无平行父子图的归档链保护（`MC4`）
- 所有学术结构读取器（`model`、`math`、`term`、`cited`、`hub`）严格仅读取**归档坐标（`coord_id == card.filing_coord_id`）**。
- 因此，把某张模型分卡以辅助坐标放在另一模型头 `M2` 下不会把它纳入 `M2`；把术语续卡以辅助坐标放在术语表下不会变成术语头；把引文卡以辅助坐标放在参考文献表下不会占用任何编号行。
- **简洁的插队护栏：** 在主题树或文献树中，若试图将一个**辅助坐标**直接插队到活跃 `model` 或 `term` 归档链的两个归档坐标*之间*（即被挤推的 Direct 子节点是归档模型分卡或归档术语续卡，且其紧邻父节点是同链 `model`/`term` 归档卡），写入器拒绝并给出清晰提示：“请放置为旁支（Side），或在关联树 `~n` 中自由编排”。这样既永不悄悄打断 `find_head` / `term::place`，也完全避免维护第二套易漂移的“平行编辑父子图”。

---

## 4. 五个端到端核心场景演练

### 场景 1 —— 同树多坐标与自祖先出现
在主题树 `3`（或关联树 `~7`）中，卡片 `X` 位于 `~7/1`（`c1`），卡片 `Y` 是 `c1` 的直接后继 `~7/2`（`c2`），卡片 `X` 再次作为 `c2` 的直接后继出现在 `~7/3`（`c3`），同时又作为 `c1` 的旁支出现在 `~7/1|a`（`c4`）：
- `c1`、`c3`、`c4` 指向同一个 `card_id`（`X`）。
- 在 `c3` 下新增旁支 `Z`（`~7/3|a`）只挂在 `c3` 上，`c1` 和 `c4` 均不受影响。
- `c3` 写位置短注“在 Y 反例之后回看 X”，`c4` 写“替代入口”，彼此独立。
- 可在 `c3` 与 `c4` 之间建立树内关联（`from_coord_id = c3`, `to_coord_id = c4`，满足 `c3 <> c4`），记录同一张卡两处出现之间的关系。
- 从 `c4` 编辑 `X` 正文，三处即时同步。解挂 `c3` 仅移除 `c3`（并按子策略处置 `Z`）并退役 `c3`；`c1`、`c4` 与 `X` 本身完好无损。

### 场景 2 —— 拥有多个坐标的树根卡
已锁定的 500 卡文献树根 `Hart1995` 同时拥有：
- 自身文献树的定义根归档坐标 `Hart1995`（`c_root`, `edge = 'root'`, `is_filing = true`）；
- 本书综述小节中的辅助坐标 `Hart1995/8|a`；
- 人类主题树 `1`（“企业理论”）中的辅助坐标 `1/2`；
- 关联树 `~7` 中的两个辅助坐标 `~7/2` 与 `~7/6|a`。
全部四个辅助坐标均显示为单节点并带 **打开原树（`Hart1995 · 500 cards`）** 入口。从主题树 `1` 解挂 `1/2` 仅移除该坐标，`Hart1995` 的 BibTeX、根树锁与 500 卡文献树毫发无损。

### 场景 3 —— 拥有多个坐标的直接后继
在主题树 `1` 中：
- 树根 `1/0` 的直接后继 `1/1` 是卡片 `A`（`c1 = filing_coord_id`）。
- 卡片 `A` 同时在同树 `1/4|b`（`c2`）与主题树 `2` 的 `2/3|a`（`c3`）拥有辅助坐标，共 3 个坐标（`{1/1, 1/4|b, 2/3|a}`）。
- 删除以 `1/4` 为根的分支会移除 `c2`；因为 `A` 仍保有归档坐标 `c1`（`1/1`），`A` **不会被删除**。
- 若主人希望把 `1/4|b` 转为 `A` 的主归档位置并撤掉 `1/1`，只需执行 `refile_card`（将 `filing_coord_id` 由 `c1` 迁至 `c2`，在既有树锁与权限护栏下改写 `[[1/1]] -> [[1/4|b]]`），再解挂 `c1` 即可。

### 场景 4 —— 跨工作区与树锁联动
卡片 `X` 归档于未锁定的 `agent` 文献树 `BookB`（`c_file = BookB/3`），同时在已锁定的 `human` 关联树 `~5` 中出现两次（`c_a = ~5/1`、`c_b = ~5/4`），并在未锁定的 `human` 主题树 `2` 中出现一次（`c_t = 2/2`）：
- **通过 `c_t`（Owner）或 `BookB/3`（Agent）编辑 `X` 正文：** 允许（`X` 并非 `~5` 的封存成员，而是被锚定的外部源卡）。`c_a` 与 `c_b` 各自独立显示相对锁定时指纹的提示性漂移；主人对 `c_a` 执行 `review_source` 仅清除 `c_a` 漂移，`c_b` 仍独立提示待审。
- **删除 `BookB/3` 或把 `BookB/3` 移至 `BookB/4`：** 拒绝（`card_locked`，`pinned-delete` / `pinned-move`，指明锁 `~5`）。
- **在未锁定主题树 `2` 中移动或解挂 `c_t`：** 允许！既不改变 `X` 的归档地址，也不触碰已锁定的 `~5`。
- **在已锁定 `~5` 中修改 `c_a` 短注或解挂 `c_a`：** 拒绝。
- **若 `X` 归档于 `human` 主题树 `2` 并放入未锁定 `agent` 树 `~9`：** Agent 可在 `~9` 内自由移动或加注 `X` 的坐标，但试图编辑 `X` 正文一律拒绝（`owner_workspace = human`）。

### 场景 5 —— 学术结构不变性与显式重新归档（`refile_card`）
模型分卡 `P`（`facet = "假设"`）归档在模型卡 `M1` 下的 `BookA/4|a|1`。用户在模型卡 `M2` 下的 `BookA/7|a|1` 新增一个 `P` 的辅助坐标：
- `get_model(M1)` 与 `get_model(M2)` 的 JSON 输出在添加前后**逐字节完全一致**。`P` 仍仅归属于 `M1`（跨模型复用仍通过显式 `**引入**` 区块实现）。
- 同理，将术语续卡以辅助坐标放在术语表下、或将引文卡以辅助坐标放在参考文献表下，均产生 **0** 项角色变更与 **0** 项编号错位。
- 唯有显式执行 `refile_card` 才会变更 `filing_coord_id`，并重新评估模型/术语/引文角色与写入门禁。

---

## 5. SQLite 表结构（第 2 代）、触发器与兼容缓存

### 5.1 逻辑表定义（`coordinates_schema_version = '2'`，`WRITER_EPOCH = 2`）

```sql
-- 1. 统一树目录（涵盖文献树、主题树与关联树）
CREATE TABLE trees (
    tree_id         TEXT NOT NULL PRIMARY KEY,          -- 不可变 UUID（复用原 connection_trees.tree_id）
    role            TEXT NOT NULL CHECK(role IN ('literature', 'topic', 'connection')),
    workspace       TEXT NOT NULL CHECK(workspace IN ('human', 'agent')),
    public_key      TEXT NOT NULL UNIQUE,               -- Citation key、'<n>' 或 '~<n>'
    display_num     INTEGER,                            -- 主题号或关联树号；文献树为 NULL
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

-- 2. 不可变卡片身份 + 提交期硬性 >= 1 坐标底线
CREATE TABLE card_identity (
    card_id         TEXT NOT NULL PRIMARY KEY,
    card_row        INTEGER UNIQUE REFERENCES cards(row_id),
    owner_workspace TEXT NOT NULL CHECK(owner_workspace IN ('human', 'agent')),
    status          TEXT NOT NULL CHECK(status IN ('live', 'retired')),
    filing_coord_id TEXT,                               -- 存活期非空；退役后为 NULL
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

-- 3. 永久句柄（语义不变：'~7'、'~7#4'，永不更新或删除）
CREATE TABLE card_handles (
    handle          TEXT NOT NULL PRIMARY KEY,
    card_id         TEXT NOT NULL UNIQUE REFERENCES card_identity(card_id),
    origin_tree_id  TEXT NOT NULL,
    local_seq       INTEGER NOT NULL CHECK(local_seq >= 0),
    UNIQUE(origin_tree_id, local_seq)
);

-- 4. 统一坐标表（无 UNIQUE(tree_id, card_id)！）
CREATE TABLE coordinates (
    coord_id        TEXT NOT NULL PRIMARY KEY,          -- 不可变 UUID（原样保留既有 placement_id）
    tree_id         TEXT NOT NULL,
    host_workspace  TEXT NOT NULL CHECK(host_workspace IN ('human', 'agent')),
    card_id         TEXT NOT NULL REFERENCES card_identity(card_id),
    parent_coord_id TEXT,
    edge            TEXT NOT NULL CHECK(edge IN ('root', 'direct', 'side')),
    slot            INTEGER,                            -- root/direct 为 NULL；side 为显式正整数 >= 1（允许稀疏空洞）
    gloss           TEXT CHECK(gloss IS NULL OR (length(gloss) BETWEEN 1 AND 240
                        AND instr(gloss, char(10)) = 0 AND instr(gloss, char(13)) = 0)),
    FOREIGN KEY (tree_id, host_workspace) REFERENCES trees(tree_id, workspace),
    FOREIGN KEY (tree_id, parent_coord_id) REFERENCES coordinates(tree_id, coord_id)
        DEFERRABLE INITIALLY DEFERRED,
    UNIQUE (tree_id, coord_id),
    UNIQUE (coord_id, card_id, host_workspace),         -- card_identity 底线外键的复合目标
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

-- 5. 树内关联（泛化自 placement_edges；允许端点为同一卡片的两个不同坐标）
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

-- 6. 已退役坐标表（继承 retired_placements，用于 place:<uuid> 书签诊断）
CREATE TABLE retired_coordinates (
    coord_id        TEXT NOT NULL PRIMARY KEY,
    tree_id         TEXT NOT NULL,
    card_id         TEXT NOT NULL REFERENCES card_identity(card_id),
    retired_at_rev  INTEGER NOT NULL
);
```

### 5.2 树锁证据版本化、触发器与兼容缓存
- **锁证据分版本（`evidence_version IN (1, 2)`）：**
  - 既有 v1 锁行（`lock_coordinate_seals` `evidence_version = 1`、`lock_placements`、`lock_connections`、`lock_placement_pins`、`lock_source_reviews`）**逐字节原样保留**，通过 v1 兼容投影校验（确保无旁挂的旧主题锁绝不误报 `coordinate-evidence-missing`）。
  - 新建树锁写入 `evidence_version = 2`，完整封存锁定树的全部 `coordinates`（按 `coord_id`）、`coordinate_edges` 以及按 `coord_id` 索引的外部源卡锚定与指纹（`PRIMARY KEY(lock_id, coord_id)`）。
- **SQL 锁触发器（`OLD`/`NEW` 双侧覆盖）：**
  - `coordinates` 上的 `BEFORE INSERT/UPDATE/DELETE` 只要 `OLD.tree_id` 或 `NEW.tree_id` 存在活跃树锁一律拒绝，同时杜绝树内改动与跨树偷运。
  - `coordinate_edges` 与 `trees` 的写操作在宿主锁定时一律拒绝。
  - `card_identity` 上 `BEFORE UPDATE OF filing_coord_id` 在卡片为活跃锁成员或被锚定目标（`pinned-move`）时拒绝；`cards` 删除在锁定或锚定（`pinned-delete`）时拒绝。
- **写入代次围栏升级（`WRITER_EPOCH = 2`）：**
  - 修正 0.70.1 使用 `CREATE TRIGGER IF NOT EXISTS` 导致旧 epoch-1 触发器残留的缺陷（[coord.rs](file:///e:/zet/src/coord.rs#L449)）：迁移事务内**显式 `DROP TRIGGER IF EXISTS`** 删除全部旧 `writer_epoch_*` 触发器，再重新安装 `WHEN zt_writer_epoch() IS NOT 2` 触发器。
- **`cards` 兼容缓存列：**
  - `cards.location` / `cards.citation_key` 在同一写入事务内始终同步镜像卡片的规范地址（`Hart1995/2`、`3/2|a`、`~7`、`~7#4`），由 `filing_coord_id`（关联树原生卡结合 `card_handles`）派生。既有只读查询保持高速，且全系统只有唯一一个事务写入规划器。

---

## 6. 地址、坐标、链接与书签语法

### 6.1 语法对照表

| 写法 | 类别 | 是否可变 | 允许写入 `[[...]]`？ | 语义 |
|---|---|---|---|---|
| `Hart1995`、`Hart1995/2\|a` | 规范文献归档地址 | Citation key 重命名 / 移动时 | **允许** | 文献归档卡片的规范引用。 |
| `3/0`、`3/2\|a` | 规范主题归档地址 | 主题移动 / 重新归档时 | **允许** | 主题归档卡片的规范引用。 |
| `~7`、`~7#4` | 永久关联树句柄 | **永不变** | **允许** | 关联树签发的永久引用；跨越重排、转归主题树乃至原关联树删除仍有效。 |
| `~7/0`、`~7/2\|a` | 关联树导航坐标 | 重排时变化 | **拒绝** | 关联树中某一坐标的导航路径。 |
| `3/5\|b` *（当它是指向 `Hart1995/2` 的辅助坐标时）* | 主题/文献树中的辅助坐标路径 | 重排或解挂时变化 | **拒绝** | 仅用于导航。写入 `[[...]]` 时拒绝并提示改写为 `[[Hart1995/2]]` 或书签 `[label](place:<coord_id>)`。 |
| `[label](place:<coord_id>)` | 持久坐标书签 | **永不变**（跟随 `coord_id`） | 普通 Markdown 链接（非 `[[...]]`） | 重排后仍直达该特定出现位；目标退役则打开缺失坐标；该坐标被解挂则明确提示已移除。 |

### 6.2 链接规则及 v1 不引入 `[[card:<uuid>]]` 的原因
- **辅助坐标变更触发 0 次 Markdown 链接改写：** 由于 `[[...]]` 拒绝辅助坐标路径，移动、重排或解挂辅助坐标永不改写任何卡片正文，永不因 Agent 操作触发人类卡片只读拒绝。
- **规范重新归档沿用成熟护栏：** 当 `refile_card` 变更卡片的归档坐标（如 `3/2|a -> 3/5`）时，映射走既有 `apply_location_mapping_tx`，在人类工作区只读保护与树锁门禁下同步改写字面 `[[3/2|a]]`、术语目标与 `chart_links`（而永久句柄 `[[~7#4]]` 无需任何改写）。
- **为何 v1 不增加 `[[card:<uuid>]]`：** 每张存活卡片都已拥有一个简短、人类可读的规范引用（`Hart1995/2`、`3/2|a`、`~7`、`~7#4`）。在手写 Markdown 中塞入 36 位 UUID 会破坏人类与终端 Session 的可读性，并无谓牵动 `zt-check`、`zt-digest` 与全部消化提示词。

---

## 7. 坐标生命周期与事务操作契约

所有写操作均在归档编辑锁下以单次 `BEGIN IMMEDIATE` 事务执行，先过工作区权限、写入作用域与树锁门禁，提交时仅推进一次 `archive_revision`，并支持 `op_key` 幂等回执（`applied_ops`）与 `preview: true` 回滚预演。所有破坏性或改变读者既有认知的写操作均采用两步确认，并由 `(archive_id, expected_revision, plan_hash)` 强绑定计划。

| 操作 | 输入与目标 | 计划 / 护栏 / 事务效果 |
|---|---|---|
| **创建卡片（`create_topic`、`create_literature`、`create_successor`、`create_connection_card`）** | 父 `coord_id`（或根）、卡片正文与元数据 | 统一创建器原子分配 `card_id` 与初始 `coord_id`，同事务写入 `cards`、`card_identity(filing_coord_id = coord_id)` 与 `coordinates`。执行写入门禁、作用域与树锁校验。 |
| **新增坐标（`add_coordinate`、`add_coordinates`）** | `target`（`card_id` \| 地址 \| `coord_id`）或有序列表 `targets: [X, Y, X]`、`tree`、`parent_coord_id`、`edge`、可选 `before_coord_id` / `slot` 与 `gloss` | 始终分配全新的 `coord_id`——**即使 `X` 已出现在 `tree` 中或位于 `parent_coord_id` 的祖先链上**。不复制正文、子坐标或树内关联。校验宿主权限/树锁、文献树外部卡规则（`MC3`）、`model`/`term` 插队护栏（`MC4`）、被挤推 Direct 节点的短注审查，以及（若挤推主题/文献归档坐标）常规映射与链接改写。旧 `place_card` 保留集合去重语义兼容既有调用方。 |
| **移动 / 插入 / 重排坐标（`move_coordinate`、`reorder_coordinates`）** | 源 `coord_id` 列表、目标 `(tree, parent_coord_id, edge, before?)`、`mode: subtree \| alone \| span` | 检查 `coord_id` 无环（禁止将 `c` 移入自身后代）、宿主权限/树锁、插队护栏及所有结构父节点变更的短注审查。保留 `coord_id`、`place:` 书签及内部 `coordinate_edges`；跨树移动须显式处置跨边界 `coordinate_edges`。若导致主题/文献归档坐标路径变化，则计算并校验精确链接改写映射。 |
| **重新归档（`refile_card`）** | `card_id`（或地址）、`to_coord_id`（该卡片在 `owner_workspace` 内的另一个既有坐标） | 显式变更 `card_identity.filing_coord_id = to_coord_id`（泛化取代 `promote_card`）。拒绝跨工作区目标、被锁定/被锚定移动及文献根重归档。在新归档上下文重跑写入门禁，按需改写规范地址与字面链接（永久句柄 `~n#k` 保持有效），保留所有其他坐标、短注与树内关联。 |
| **解挂坐标（`detach_coordinate` / `unplace`）** | `coord_id`、可选 `children: detach \| reattach`、`reattach_to`、`replacement_filing_coord_id` | 先枚举全部被移坐标并按 `card_id` 分组计算**操作后剩余坐标集合**：(a) 归档坐标在被移集合之外 → 普通解挂；(b) 归档坐标被移且提供了同工作区有效 `replacement_filing_coord_id` → 原子重归档并解挂；(c) 该卡在所属工作区将剩 **0** 个坐标 → **拒绝**（必须走显式删卡）。被移 `coord_id` 记入 `retired_coordinates` 并删除其关联的 `coordinate_edges`。 |
| **删除坐标子树 / 删除整树** | 子树根 `coord_id`（或树根 `coord_id`）、可选 `--keep-root` | 先枚举被删 `coord_id` 再按 `card_id` 分类：(a) **存活无恙**（归档坐标在删除集外）；(b) **需先重归档**（归档坐标在删除集内，但同工作区另有存活坐标）；(c) **彻底删卡**（同工作区剩余 0 坐标——需完整删卡确认、断链/`into` 审查与锁/锚定校验，并将跨工作区存活坐标转为缺失坐标）。删除定义根必须走整树计划；解挂根卡的辅助坐标只是普通解挂。 |
| **全局删卡（`delete_card`）** | `card_id`（或规范地址）、可选 `into` | 身份级删除。校验所有者工作区与锁/锚定门禁（`pinned-delete`），移除范围内自有坐标，将 `card_identity` 置为退役（`status = 'retired'`、`filing_coord_id = NULL`），外部存活宿主树中的坐标自动变为**缺失坐标**（绝不修改外部工作区树）。`plan_hash` 覆盖 `coord::delete_state`（含被删卡正文、受影响坐标、短注及树内关联）。 |
| **重绑缺失坐标（`rebind_coordinate`）** | 缺失的 `coord_id`、替代 `target_card` | 校验宿主可写性与文献准入；强制短注审查并要求逐条显式处置所有附着 `coordinate_edges`（旧断言绝不静默套用到新卡）；签发新 `coord_id` 并退役旧 ID。 |

---

## 8. 全系统 10 大子系统联动与防错矩阵

### 8.1 链接、术语链接、图表、反向链接与断链（`src/link.rs`、`src/link_index.rs`、`src/card.rs`）
- 链接一律按规范归档地址或句柄指向卡片（`[[Hart1995/2]]`、`[[3/2|a]]`、`[[~7#4]]`），拒绝辅助坐标路径。
- 辅助坐标变更导致 **0** 次链接改写；归档坐标变更按精确映射同步改写普通链接、术语链接目标与 `chart_links`，保留断链防劫持机制（避免紧缩导致断链错落到重命名兄弟上）。
- 反向链接与术语用法按 `(source_card_id, form)` 去重，不受坐标数量影响；`zt status --check-links` 与 `zt reindex` 继续以正文与规范别名为准。

### 8.2 工作区（`human` / `agent`）、权限与写入作用域（`src/workspace.rs`、`src/scope.rs`）
- 所有读接口明确拆分 `can_edit_card`（所属工作区 + 卡片锁状态）与 `can_edit_coordinate` / `can_edit_tree`（宿主工作区 + 宿主树锁状态）。
- 规划器与 SQLite 复合外键双重禁止移除卡片在所属工作区的最后一个坐标而保留异工作区坐标，杜绝静默所有权转移。
- `WriteScope` 在作用域检查前先将句柄与坐标解析为 `card_id`，文献作用域 MCP 连接继续禁止自由坐标写入与主题/枢纽改动，并对真实地址映射执行 `ensure_no_connection_rewrite`。

### 8.3 树锁、锚定、封存与触发器（`src/tree_lock.rs`、`src/coord_lock.rs`）

当卡片 `X` 同时位于**已锁定树 `A`（`c1`）**与**未锁定树 `B`（`c2`）**时的精确行为表：

| 操作 | 规定结果 |
|---|---|
| **通过 `c2` 编辑 `X` 正文** | 若 `X` 是 `A` 的封存成员（`X.filing_coord_id` 在 `A` 内）→ **拒绝**；若 `X` 仅作为外部源卡在 `A` 中被锚定 → 在满足 `X` 所属工作区权限时**允许**，并在 `c1` 记录提示性漂移。若 `A` 中有 `X` 的两个坐标 `c1a, c1b`，审阅 `c1a`（`review_source`）不会自动标记 `c1b` 已审。 |
| **在未锁定 `B` 中新增 `X` 的坐标 `c3`** | **允许**（仅校验 `B` 的宿主权限；`X` 与 `A` 均未被改动）。 |
| **在未锁定 `B` 中移动或解挂 `c2`** | 若 `c2` 是辅助坐标 → **允许**；若 `c2` 是被 `A` 锚定的 `X` 归档坐标且会改变承诺地址或退役 `X` → **拒绝（`pinned-move` / `pinned-delete`）**。 |
| **删除未锁定树 `B`** | `B` 中的辅助坐标可正常移除；若删除计划会退役或改变被 `A` 锚定的 `X` 的归档地址 → **拒绝**并在计划中指明阻塞锁。 |
| **在已锁定 `A` 中移动、解挂、重绑或改写 `c1` 短注/关联** | **拒绝**（由规划器与校验 `OLD.tree_id`/`NEW.tree_id` 的 SQL 触发器双重拦截）。 |
| **运行 `zt status --check-locks`** | v1 锁沿用 v1 投影核查（确保无旁挂旧主题锁绝不误报），v2 锁核查完整 `coordinates`、`coordinate_edges` 与按 `coord_id` 索引的锚定封存。外部源卡漂移永不触发完整性故障。 |

### 8.4 模型与数学单元（`src/model.rs`、`src/math.rs`、`src/model/formal.rs`、`src/model/empirical.rs`）
- 任意坐标输入先解析为 `card_id`，再严格基于**归档坐标**与正文链接计算 `find_head`、`heads`、`parts_of`、`in_block`（首边为 Side 的整块规则）、`build_report`、形式/实证角色故障、`unit_of`、`theory_report`、`reach_in` 与 `study`。
- 将模型分卡以辅助坐标放在另一模型头 `M2` 下绝不使其加入 `M2`；跨模型共享仍仅通过显式 `**引入**` 进行。
- 在任意树添加任意辅助坐标前后，既有模型与数学报告保证**逐字节一致**。

### 8.5 术语卡（`src/term.rs`）与引文卡（`src/cited.rs`）
- `term::place` 与 `cited::structure` 仅沿**归档坐标**求值。
- 术语卡的辅助坐标不会在术语表下变成术语头（也不能作为 `[[label@addr]]` 目标）；引文卡的辅助坐标在参考文献表下不占任何行号、不触发错位故障。
- 唯有显式 `refile_card` 会改变术语/引文卡的结构角色并重跑校验。

### 8.6 枢纽卡、概念索引表、入口搜索、远卡、问盒与同述折叠（`src/hub.rs`、`src/entry.rs`）
- 枢纽范围（`line`/`topic`/`field`/`register`）、覆盖度（`covered`/`reached`/`unreached`）、索引表分册判定、`source_evidence` 与 `distant` 归档邻接排除均严格基于**归档坐标**与去重后的**卡片（`card_id`）**。
- 同一张卡在主题树里出现 3 次，枢纽覆盖仍只计 1 次，不会凭空多出出处证据或重复枢纽行。
- `entry::search` 对不同卡片只排 1 行（附带其全部坐标元数据），保持 `total` 与 `row_total` 折叠计数准确；`ask_box` 仅按 `card_id` 沿正文语义链接游走，辅助坐标与 `coordinate_edges` 永不混入。

### 8.7 写入门禁与 `zt-check`（`src/gate.rs`、`tools/zt-check/`）
- 从任意坐标编辑卡片正文，均以该卡片的**归档坐标上下文**构建 `gate::Candidate`（在主题树中查看并编辑文献笔记卡时仍按文献笔记规则校验；在关联树中编辑主题卡时仍按主题 I1–I6 校验）。
- `topic_exists` 仅检查真正的新建 Agent 主题根，永不因辅助坐标触发。
- 添加/移动辅助坐标仅执行宿主、槽位、树锁、插队护栏与短注检查，不重跑正文文风规则。

### 8.8 既有坐标视图（`elsewhere`、`crossings`、依赖透镜、播种、分支提取）
- **`elsewhere`：** 返回该卡片的**全部其他坐标**（包含**同一棵树内**的兄弟坐标），每项含 `{coord_id, tree, path, is_filing, gloss}`。
- **`crossings`（投影）：**
  - 以 **`coord_id`** 为主键，不再仅按 `address` 聚合。
  - 完整保留 `from_card_id == to_card_id` 但 `from_coord_id != to_coord_id` 的线（移除 [connections.js](file:///e:/zet/src/gui/connections.js#L340) 中 `line.from === line.to` 的静默丢弃）。
  - 当目标平面树 `T` 中卡片 `X` 也存在多个坐标时，`stations` 返回 `X` 在 `T` 中的目标 `coord_id` 集合，GUI 分组或按选定端点渲染，避免制造 N×M 笛卡尔积连线爆炸。
  - `exits` 按 `coord_id` 统计（可附带卡片级汇总）。
- **依赖透镜与播种：** 正文依赖闭包维持卡片/单元级；叠加层保留精确 `coord_id` 端点且不向正文依赖闭包注入环。`seed_connection_tree` 对枢纽/搜索/透镜来源保持按卡去重，同时为含刻意重复的大纲来源新增有序出现位播种模式。
- **`extract_branch`：** 为选中坐标子树中的每个 `coord_id` 分配新 `coord_id`（保留分支内同一卡的多次出现），重映射内部 `coordinate_edges`，保留跨界关联在原树不动，并执行短注审查。

### 8.9 GUI、终端 Session、MCP、批处理、卡阵、转储与便携导出
- **`Snapshot`（`src/ops.rs`）与 GUI（`src/gui/app.js`、`layout.js`、`connections.js`）：**
  - 拆分**卡片索引**（`byCardId` / 规范 `byAddress`）与**坐标索引**（`byCoordId`、按 `coord_id` 建树的单树大纲）。
  - Folgezettel 地图的 DOM 节点、焦点、选中、展开与拖拽一律绑定 **`coord_id`**。同树重复出现的卡片独立绘制并带简洁徽章（`同树 ×N`），各自展开独立子节点，正文编辑实时同步。
  - `places` 统一取 `COUNT(*) FROM coordinates WHERE card_id = ?1`。
  - 正文链接弧保持卡片级（连向焦点/最近出现位或分组徽章，防止毛线团）；`coordinate_edges` 精确连接 `coord_id` 对。
- **终端 Session（`src/session.rs`、`src/session/station.rs`）：** 所有树统一 `Pointer` 为 `(card_id, coord_id)`；解挂后的失效历史明确提示 `coordinate removed` 并给出存活的归档坐标。
- **MCP（`src/mcp.rs`）与批处理（`src/batch.rs`）：** 既有卡片/消化工具保持归档坐标兼容（`get_tree` 默认每张归档卡返回一行）。新增显式坐标工具：`list_coordinates`、`get_tree_coordinates`、`add_coordinate`、`add_coordinates`、`move_coordinate`、`detach_coordinate`、`refile_card`。
- **卡阵（`src/spread.rs`）：** 保持按卡片索引（可选记录焦点 `coord_id`）；解挂辅助坐标不会令存活卡片从卡阵消失。
- **转储（`zt dp`）与便携导出（`zt export`）：** `zt dp` 在单次一致读事务内将每卡正文只写一次，附带 `coordinates.json` v2（含 `trees`、`card_identity`、`card_handles`、`coordinates`、`coordinate_edges`、`retired_coordinates` 与锁坐标证据）。`zt export <tree>` 输出 Portable v4：卡片正文去重（保留各自 `owner_workspace`），完整写入树内全部 `coordinates`（支持同卡多 `coord_id`）并重映射 `place:<coord_id>` 书签。

### 8.10 消化工作流与配套二进制（`zt-check`、`zt-digest`）
- `zt-econ-digest`、`zt-math-digest`、`zt-idea-grow`、`zt-post-digest` 在 `get_tree`、`get_card`、`apply_batch`、原始术语宏、写入作用域与 `archive_revision` 上保持 100% 归档视图兼容。
- **专项核验提示：** `tools/zt-digest/src/calls.rs` 维护着独立一份正文指纹实现；发布验收必须双边核验 `zt` 与 `zt-digest` 指纹一致（并预先核清 [connection-trees-free-proposal.md](connection-trees-free-proposal.md#L154) 记录的既有 `zt-econ-digest.js` 清单锁差异基线，避免归因混淆）。

---

## 9. 已核实的代码重构清单（16 处关键位点）

绝不可仅靠删掉 `UNIQUE(host_tree_key, card_id)` 仓促上线；以下 16 处经源码逐行核实的位点必须成套改造：

| # | 文件与行号 | 既有单坐标假设 / 隐患 | 必须完成的改造 |
|---|---|---|---|
| 1 | [coord.rs:312](file:///e:/zet/src/coord.rs#L312) | `UNIQUE (host_tree_key, card_id)` | 切换为统一 `coordinates` 表与非唯一索引 `coordinates_by_tree_card`。 |
| 2 | [coord.rs:379](file:///e:/zet/src/coord.rs#L379) | `identity_after_insert` 触发器无法满足不可延迟的 `filing_coord_id IS NOT NULL` CHECK | 退役触发器；全部卡片创建统一经由单事务卡片+坐标创建器写入。 |
| 3 | [coord.rs:435-458](file:///e:/zet/src/coord.rs#L435-L458) | `writer_fence_sql` 使用 `CREATE TRIGGER IF NOT EXISTS` 且内嵌代次常量 | 升级至 epoch 2 时必须先显式 `DROP TRIGGER IF EXISTS` 清理全部旧触发器再重建。 |
| 4 | [coord.rs:1031](file:///e:/zet/src/coord.rs#L1031) | `TreeShape::placement_of_card` 仅返回首个匹配 `Option<&Placement>` | 改为 `coords_of_card(&self, card_id) -> Vec<&Coordinate>` 及显式 `filing_coord_id` 查询。 |
| 5 | [coord.rs:1098](file:///e:/zet/src/coord.rs#L1098) | 强制旁支序号为 `1..N` 致密排列，在 303 组稀疏历史旁支上报错 | 显式存储正整数 `slot`，直接由 `slot` 派生旁支字母，保留空洞。 |
| 6 | [coord.rs:1449](file:///e:/zet/src/coord.rs#L1449) | `delete_impact_lines` 跳过同原宿主树出现位 | 改为按 `card_id` 计算操作后全局剩余坐标集合。 |
| 7 | [coord.rs:1516](file:///e:/zet/src/coord.rs#L1516) | `follow_anchor_moves` 围绕 `+k` 主题旁挂做同树碰撞拒绝 | 随 `+k` 退役（`MC1`），统一到 `coordinates` 移动逻辑。 |
| 8 | [connection.rs:532](file:///e:/zet/src/connection.rs#L532) | `crossings_of` 按 `address` 聚合出口并把 `placement_id` 压扁为 `address` | 投影的站点、连线与出口全部改按 `coord_id` 建键。 |
| 9 | [connection.rs:629,642,2770](file:///e:/zet/src/connection.rs#L629) | `origin: "native"` 把所有重复位都标成原生；`places` 漏计主坐标；`tree_summary` 统计失真 | 严格区分 `is_filing` 与辅助坐标；`places` 统计全表 `coordinates`；分别报告不同卡片数与总坐标数。 |
| 10 | [connection.rs:841](file:///e:/zet/src/connection.rs#L841) | `validate_tree` 只检查 `EXISTS(... card_id)` 而非归档坐标 | 校验 `card_identity.filing_coord_id` 指向所属工作区内的存活坐标。 |
| 11 | [connection.rs:1354,1451,1587,2300](file:///e:/zet/src/connection.rs#L1354) | `place`、`place_many`、`attach`、`rebind` 静默吞重或拒绝重复 | 保留旧 `place_card` 兼容适配；新增允许重复的 `add_coordinate` / `add_coordinates` / `rebind_coordinate`。 |
| 12 | [connection.rs:2094](file:///e:/zet/src/connection.rs#L2094) | `unplace` 的 `home_here` 阻止解挂本土树中的任意出现位 | 仅保护 `coord_id == card.filing_coord_id`（需提供替代归档坐标或走删卡）。 |
| 13 | [connection.rs:2708](file:///e:/zet/src/connection.rs#L2708) | `promote` 仅查找并解挂首个主题旁挂 | 泛化为 `refile_card(card_id, to_coord_id)`，保留其他所有坐标。 |
| 14 | [dependencies.rs:517,678](file:///e:/zet/src/dependencies.rs#L517) | 依赖透镜叠加层丢失 `coord_id`；播种无条件去重 | 叠加证据保留 `coord_id`；播种支持有序出现位模式。 |
| 15 | [connections.js:340](file:///e:/zet/src/gui/connections.js#L340)、[app.js](file:///e:/zet/src/gui/app.js)、[portable-rules.js:369](file:///e:/zet/src/gui/portable-rules.js#L369) | `line.from === line.to` 误删同卡不同坐标连线；`state.byAddress` 与首匹配跳转在重复坐标下失效 | 地图节点、焦点、选中、投影与离线查找全面切换至 `coord_id`。 |
| 16 | [main.rs:1482](file:///e:/zet/src/main.rs#L1482) | `initialize_database` 未开启 `PRAGMA foreign_keys = ON` | 在 `initialize_database` 与 `open_db` 中同时强制开启 `PRAGMA foreign_keys = ON`。 |

---

## 10. 设计评审与审计记录（Astra 咨询 + Gemini 独立审计）

### Astra 架构分析（`gpt-6.2-astra`，高推理力度，会话 `01a22099-e605-7993-ab29-1117f08cbba1`）
- 诊断了 0.70–0.71 的核心不对称（`cards.location` 与 `placements` 割裂 + `UNIQUE(host_tree_key, card_id)`）。
- 实盘核查 `D:\zt`（`62,322` 卡、`0` 存量落点、**`303` 组稀疏旁支**、`20` 句柄、`122` 退役落点、`166,710` 回执）。
- 设计并在 SQLite 3.45.1 内存库上实测验证了提交期 `>= 1` 底线延迟复合外键方案（8 组边界用例全通）。
- 提前捕获了 `writer_fence_sql` 的 `CREATE TRIGGER IF NOT EXISTS` 升级陷阱、v1 主题锁 `coordinate-evidence-missing` 误报风险、全库 16 处单出现假设位点以及 8 项发布阻塞验收用例。

### Gemini 独立审计与收敛修正
逐行对照 `E:\zet` 核实了 Astra 的全部源码引用，并在最终提案中落定了 6 项关键修正：
1. **消除双义路径碰撞（`MC4`，§3.2 #2）：** 否决“允许阅读大纲路径与归档地址在同一棵树里共用相同字符串（如 `1/2` 同时指代两张卡）”的设想，确立**单一坐标空间不变量**。
2. **消除平行“编辑父子图”漂移风险（`MC4`，§3.2 #6）：** 用“学术读取器仅过滤归档坐标 + 极窄的 `model`/`term` 归档链插队护栏”取代第二套存储父子图。
3. **修复卡片创建触发器死锁与 `initialize_database` 外键开关（§3.2 #1、§9 #2 与 #16）：** 发现 [coord.rs:379](file:///e:/zet/src/coord.rs#L379) `identity_after_insert` 会撞上不可延迟的 `CHECK(filing_coord_id IS NOT NULL)`，且 [main.rs:1482](file:///e:/zet/src/main.rs#L1482) 漏开 `PRAGMA foreign_keys = ON`。
4. **干净退役主题旁挂 `+k`（`MC1`，§3.2 #5）：** 借助 `D:\zt` 当前 0 落点的实盘窗口彻底退役 `EdgeKind::Attach`，不再背负第四种边类型。
5. **v1 坚持可读规范引用、不引入 `[[card:<uuid>]]`（§6.2）：** 避免 36 位 UUID 污染手写 Markdown 与消化工具链，同时维持辅助坐标变动 0 次链接改写。
6. **修正 `places` 漏计与 `crossings` 自连线/笛卡尔积边界（§8.8–8.9）：** 统一 `places` 口径并以 `coord_id` 精确建键投影视图。

---

## 11. 五阶段实施、零丢数迁移方案与 8 项发布阻塞验收用例

### 推进阶段（`P1`–`P5`）

- **`P1` —— 冻结可观测契约与构造完整夹具：** 从 `D:\zt` 拷贝一致基线快照（62,322 卡、303 组稀疏旁支），并补齐实盘暂缺的测试夹具：活跃 v1 文献/主题/关联树锁（含无旁挂的 v1 主题锁）、带子节点与关联的缺失坐标、原树已删的转归句柄、仅在图表内被引用的双工作区人类卡；固化全套黄金报告摘要（含 `zt-digest` 指纹）。
- **`P2` —— 写入代次 epoch 2 围栏与原子迁移：** 在编辑锁下生成并校验 `VACUUM INTO` 备份；在 `initialize_database` 与 `open_db` 均开启 `PRAGMA foreign_keys = ON`；启动门禁优先校验 `coordinates_schema_version = '2'`；显式 `DROP` 旧 epoch-1 触发器并安装 epoch-2 触发器；物化 `trees` 与 `coordinates` 并原样保留 303 组稀疏 `slot`（0 紧缩、0 地址改动），复用全部既有 `placement_id`，绑定 `card_identity.filing_coord_id`，逐字节保留 v1 锁证据；用真实 0.70/0.71 旧二进制验证任何写入均干净失败。
- **`P3` —— 统一核心引擎（`coord_id` 优先）：** 实现统一坐标读写、集中式卡片+坐标创建器、剩余集解挂/删树规划器、`refile_card`、`model`/`term` 插队护栏、v2 锁封存与双侧 `OLD`/`NEW` 触发器，并将所有既有归档/消化写入端点统一收口至单一事务规划器（杜绝双写分裂窗口）。
- **`P4` —— GUI、Session、MCP、转储与便携导出：** GUI 地图/阅读器/投影全面切换至 `coord_id` 键控，新增重复徽章（`同树 ×N`）、“在此新增坐标”、“移除此坐标”、“重新归档至此”及“其他位置…”切换器（含同树兄弟）；统一 Session `Pointer` 为 `(card_id, coord_id)`；上线 MCP 坐标工具集；升级 `zt dp`（`coordinates.json` v2）与 `zt export`（Portable v4）。
- **`P5` —— 全量压测与工作流认证：** 跑通下方全部 8 项发布阻塞验收用例，在演练副本上重放 `zt-econ-digest`、`zt-math-digest`、`zt-idea-grow`、`zt-post-digest`，验证 `zt dp` 重建与 Portable v4 离线浏览同卡多坐标，并实测 6.2 万卡规模下的启动、读树、投影、重建索引与转储耗时。

### 发布阻塞验收用例（8 项必过）

1. **同树重复与自祖先出现：** 构造 `X(c1) -> Y(c2) -> X(c3)` 及旁支 `X(c4)`，赋予 `c3`、`c4` 各自不同的子节点、短注与树内关联；移动 `c3`，重开 `place:<c3>` 与 `place:<c4>`，解挂 `c3`，验证剩余子节点、卡片正文与关联端点完全准确。
2. **根节点多坐标：** 让同一文献根卡同时作为定义根、本树内部非根节点以及主题树/关联树中的非根节点；解挂所有非根坐标，验证定义根、BibTeX、根树锁与树工作区毫发无损。
3. **提交期 `>= 1` 底线与并发：** 并发或同批移除卡片最后两个坐标、移除所属工作区最后一个坐标（保留异工作区坐标）、以及删树波及卡片全部坐标；验证拒绝时修订号、索引、坐标与回执全部干净回滚，绝无孤卡。
4. **树锁与逐出现位漂移：** 在已锁定宿主中把外部源卡 `X` 放两次，编辑 `X` 后只审阅其中一处，验证两处漂移与审阅状态彼此独立；将 `X` 换作封存成员重复测试，验证 GUI/MCP/Session/CLI 全入口均拒绝编辑。
5. **学术结构不变性：** 将模型分卡以辅助坐标放在另一模型头下、将术语续卡放在术语表下、将引文卡放在参考文献表下、将枢纽成员在主题树中重复 3 次，要求模型、数学、术语、引文、枢纽与 `zt-check` 输出逐字节不变；随后执行显式 `refile_card`，验证语义按预期更新。
6. **多出现位投影（`crossings`）：** 在两棵跨树关联树与当前浏览树中均放置 `X` 两次，验证平行关联、同卡不同坐标连线、缺失坐标跳数与按坐标统计的出口均确定性呈现，无丢线、无笛卡尔积爆炸。
7. **幂等性与历史回执重放：** 迁移后重放历史 `op_key` 空操作回执；再以不同 key 连续提交两次 `add_coordinate(X)` 并各自重试，确认精确生成两个新坐标、ID 稳定且历史重放无漂移。
8. **实盘规模迁移与便携性：** 迁移 `D:\zt` 完整副本（`62,322` 卡、`303` 组稀疏旁支），核验存活/退役身份集合、正文哈希、句柄、稀疏路径、既有锁历史完全一致，`zt status --check-links` 与 `zt status --check-locks` 全绿，验证备份恢复、`zt dp` 往返重建、Portable v4 离线多出现位导航与 6.2 万卡延迟指标达标。
