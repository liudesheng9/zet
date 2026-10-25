# ZT

[English](README.md) | 中文

ZT 是一个本地优先的卡片笔记系统：每张 Card 都是可寻址的 Markdown 文档，既可以在终端 Session 中使用，也可以在卡片盒 GUI 中浏览和编辑。

## 功能

- 所有 Card 都用 Markdown 书写：`# 标题`、Markdown 正文，以及自动生成的 Reverse link。
- 使用 `zt gui` 浏览与编辑：卡片柜、按地址排布的 Folgezettel 地图、把各处相关 Card 并排摊开的桌面（Table）、全部 Card 的星图（Constellation），以及带实时预览的 Markdown 写作台。
- 使用 Topic 树和 Literature 树组织笔记：每个 Topic 与每个基于 BibTeX 的 Literature Card 都是一棵独立 Regular Card 树的根。
- 可以把 Regular Card 标为 Event Card（一个真实发生的事件）、Finding Card（来源中的一个实证结果）、Model Card（一个完整的模型）或 Term Card（书中的一个术语，带准确名称与词条链接），引用它的其他 Card 用 Link 指向它。
- 维护两个 Workspace：一个属于你，一个属于 AI agent，各有 Topic zone 与 Literature zone；Link 可以跨 Workspace，Card 不能在 Workspace 之间移动。
- 通过 `zt mcp` 服务器让编程 agent 在 agent Workspace 中工作，而你的 Workspace 对它只读。
- 在交互式终端 Session 中，通过 Location 或 Citation key 导航 Card。
- 使用 `[[target]]` Link 连接 Card，并自动生成 Reverse link、检查 Broken link。
- 每次写入都经过同一道检查：格式、Link、Access 与 Lock 的强制规则总会拒绝，较新的质量规则先只给出建议，等 owner 主动升级 gate。
- 在 Session 或 Shell 中安全地创建、编辑、移动和删除 Card 树。
- 使用本地 SQLite 存储数据，并提供状态、统计、导出，以及一条删除命令：可清空一棵 Card 树、保留树根，或重置整个 archive。
- 使用 `zt autostart on` 让服务在登录时启动、在机器关机时停止（Windows）。

## 安装

安装 Rust 工具链，克隆本仓库，然后运行：

```sh
cargo install --path . --locked
```

## 发布

一次发布把 `zt`、`zt-check`、`zt-digest`、两个冻结的消化工作流以及安装好的 agent 指南作为**一套兼容的产品**一起交付。四组版本互相关联但彼此独立：可执行文件版本（`zt`、`zt-check`、`zt-digest`）、检查器的规则/诊断模式/适用性版本、归档模式 generation，以及冻结工作流的构建版本。本二进制把它承载的版本彼此分开地发布成一个 JSON 对象：

```sh
zt version          # 一行人类可读的版本
zt version --json   # zt、checker、rule_version、diagnostic_schema、applicability_version、schema、capabilities
```

候选发布集记录在 `docs/term-cards-links-and-check-gate-release-manifest.json`；它的静态一致性由测试
`tests/release_manifest.rs` 保证，`scripts/verify-release.ps1` 再把已安装的二进制、冻结脚本与其 lock、已安装的
skill 以及运行中服务的元数据与该清单逐项核对。运行中的 daemon 版本以 daemon 记录为准，不是磁盘上二进制副本的版本：
`zt status` 会打印它，服务也会在每次读取 Card 时一并发布自己的规则与能力契约。

## 快速开始

选择 ZT 的本地数据目录，启动服务，然后打开 Session：

```sh
zt config set archive_root "<数据目录>"
zt up
zt
```

在 Session 中输入命令时，不需要添加 `zt` 前缀：

```text
t 我的第一个主题   # 创建 Topic
n                  # 创建当前 Card 的 Direct successor（也可用于 Literature Card）
b                  # 创建 Side successor
l                  # 创建 Literature Card
e                  # 编辑当前 Card
ty event           # 把当前 Card 设为 Event Card（或 ty model、ty idea）
ws [agent|human]   # 列出 Workspace，或切换到其中一个
go <目标>          # 前往指定 Location 或 Citation key
up                 # 前往父 Card（树根会回到 ROOT）
ls                 # 列出当前树；在 ROOT 列出 Topic 与 Literature
help               # 显示当前位置可用的命令
q                  # 退出 Session
```

ROOT 视图会列出所有 Topic 和 Literature Card。每个 Card 视图末尾都有可点击的
`parent:`、`direct:`、`side:` 链接。`ls`、`lsbk`、`help` 会打开可点击地址的结果面板，
按 `Esc` 关闭。内容过长时可用 `PageUp`/`PageDown` 或鼠标滚轮滚动。

## 随机器启停

在 Windows 上，可以让服务跟随机器启动和停止：

```sh
zt autostart on    # 每次登录时启动服务
zt autostart       # 查看登录命令，以及上一次登录启动的结果
zt autostart off   # 不再在登录时启动
```

`zt autostart on` 会把与 `zt` 一同安装的无窗口程序 `zt-boot` 登记到当前账户的 Run 键下（任务管理器的
“启动应用”中可以看到它），因此不需要管理员权限，登录时也不会弹出控制台窗口。每次登录启动的结果都记录在
配置文件旁的 `autostart.log` 中；`zt status` 会显示自动启动是否开启。

机器关机、重启或你注销时，守护进程会自行停止服务，并把原因写入 `zt.log`；它不会等待仍打开的 Session，
因为它们会随机器一起结束。如果守护进程没有机会清理就结束了（断电、进程被强制结束），服务会显示为已停止，
下一次登录或 `zt up` 会重新启动它。

## Markdown Card

每张 Card 都以一个 Markdown 文档存储：

```markdown
# 原子化笔记

每张卡片只写一个想法。参见 [[0/0]] 与 [[Smith2024]]。

- 简短
- 可链接

<!-- zt:reverse-links -->
## Reverse links

- [[0/2]] 地址即位置
```

Topic 和 Regular Card 必须以 `# 标题` 开头。`<!-- zt:reverse-links -->` 标记之后的内容由 ZT
自动生成，每次保存都会重写，因此在那里所做的修改会被忽略。`[[target]]` Link 只在正文中生效；
写在 `行内代码` 或代码块中的内容只是普通文本。Term Card 的直接词条链接写作 `[[标签@地址]]`：
标签是句中的那个词（1–40 个字符），恰好一个 `@` 分开标签与地址，地址必须是一张 Term head
（术语表的直接侧后继）。移动只改写地址字节，从不改动标签。终端不会渲染 Markdown：Session 会原样显示和编辑
存储的 Markdown。唯一的例外是 Term 链接的阅读视图：Session 读者只看到标签，编辑模式仍显示完整的
`[[标签@地址]]` 宏。

有名字的 Term Card 按一套固定模板书写，T1–T10 规则在每次写入时检查：`术语：`、可选的 `原文：`、
`不用：`、`类别：`、`首次定义：` 这些头部行；固定顺序的 `释义`、`书中的说法`、数学概念所需的数学各节、
`区别`、可选的 `常见误读` 和 `在本书中的作用`；最后一行 `来源：[[书]] 定位`。数学卡还要有编号前提、
带标签的定义、符号表、带依据与结果的编号推导、极限、算例、直觉和常见误读。写 `自动链接：否` 并附
必需的 `自动链接理由：` 行，表示该术语保持不自动链接。很长的说明续写在标题为 `推导（续 k/n）：…`
的 Term part 中，续卡保留同一术语名并链接回 head。

### Term Card、术语表与 Term use

**术语表卡**（Term list Card）是没有术语名、类型为 `term` 的 Regular Card：正文保留书的四列表格，它的直接侧后继
按表格行序就是它的 **Term Card**。Term Card 只陈述一个准确的**术语名**：至多 40 个字符、只占一行，不能含 `[`、
`]`、`@`、`|` 或反引号；标题就是该名称，正文按上面的固定模板书写，最后一行 `来源：` 指明书名与定位。**Term part**
沿用 head 的术语名，续写很长的说明。术语表、head 与 part 的角色由存储的术语名和 Location 计算，不另存一份结构。
`自动链接：否` 与它必需的 `自动链接理由：` 一起，表示这个过于宽泛的术语不进入确定性链接流程。

**Term use** 是另一张 Card 正文里的一个词，写成 `[[均衡@Smith2024/1|a]]`：标签就是句中那个词，恰好一个 `@`
分开标签与地址，地址必须指向一张 Term head。移动只改写地址字节，从不改动标签；读者只看到标签。head 自动生成的
`## Term uses` 段单独列出使用它的 Card，与 Reverse link 分开。在 Link 扫描中，Term use 的关系类型是 `term`，
由语法决定，与它前面的关系词无关：它是术语引用，不是论证 Link，因此不给 Model 成员、关系或邻居，也不给
Idea/Event/Finding 的论证计数，默认也不画图边。Map、Table 与 Constellation 在读者打开 Term uses 开关之前都会隐藏
Term 边；Broken links、Term uses、Lock pins 这些通用 Link 读取仍然能看到它。

检查器的 T1–T10 规则在每次写入时检查 Term Card 的固定模板：已知且有序的头部行与各节、标题/名称/表格行的身份、
数学 head 所需的数学内容、符号表覆盖、带依据与结果的编号推导、不得留下未展开的跳步、Term 链接目标与标签一致、
一行一个 head、正文长度，以及公式配对。数学 Term Card 还要写出前提、带标签的定义、带理由的等价形式、极限、算例、
直觉、区别与常见误读。来源忠实是模板的一部分：书的原话保留在 `书中的说法`，书中略去的推导步骤标为
`（补：书中略去）`，绝不悄悄补写成书里的说法；术语表行保留原拼写。

读取可以把 Term use 投影成读者看到的标签：在 `get_card`、`get_cards` 或 `get_tree`（带正文）上传
`term_links: "label"`，回复给出可见的句子文本并标记为投影。默认的原始读取保留作者写下的宏；编辑底稿、回执哈希与
指纹必须使用原始读取，因为投影文本只用于显示。消息摘要始终是投影后的显示文本。

0.2.0 之前创建的档案会在 `zt up` 时自动转换；转换前会先把原数据库备份为档案目录中的
`zt.sqlite3.pre-markdown-<时间>.bak`。

## GUI

GUI 运行在服务内部：`zt up` 启动它，`zt down` 停止它，所以服务启动时 GUI 就在运行。
`zt gui` 不启动任何进程，只告诉你 GUI 的地址：

```sh
zt gui                 # 打印 GUI 的地址和端口
zt gui --open          # 同时打开应用窗口（Edge/Chrome）或默认浏览器
```

`zt up` 和 `zt status` 也会打印这个地址。GUI 默认监听 4717 端口，被占用时改用空闲端口；
在 `zt up` 之前设置 `ZT_GUI_PORT` 可以指定端口（`0` 表示由系统选择）。

- **卡片柜**（左侧）：human 与 agent 两个 Workspace 的标签页、当前 Workspace 的 Topic zone 与 Literature zone，以及搜索框。
- **搜索抽屉**（`/` 或 `Ctrl+K`）：在书桌上拉出的卡片目录抽屉。卡片必须包含查询中的每个词，标题命中排在最前。
  命中结果按树分组，每棵树一张导卡，后面立着几张卡片条（其余折叠）；选中的卡片条抬起，整张卡片显示在抽屉旁，
  `Enter` 把它带到阅读器（`Shift+Enter` 放上桌面）。可按 Workspace 或 Card 类型筛选；输入框为空时显示最近访问的卡片。
- **后退与前进**：路径旁的箭头像文件浏览器一样在访问过的卡片间后退、前进：`Alt+←` / `Alt+→`、`Backspace`、
  鼠标的后退与前进键，或命令栏（以及 Session）中的 `back` / `fwd`。右键点击箭头可列出可去的位置。
- **书桌**（ROOT）：当前 Workspace 的每棵树显示为一叠索引卡片，按 Zone 分组。
- **地图（Map）**：用聚焦透镜呈现当前树的 Folgezettel 地图。Direct successor 沿同一列向下延伸，
  Side successor 进入右边一列。Pointer 所在的卡片被拉到前面；它的路径、后继和相邻卡片以小卡片立起；
  其余卡片只露出一行卡头，远离 Pointer 的分支折叠为 "+N" 卡叠。只画 Pointer 的 Link。按 `x` 展开整棵树。在任意位置（包括卡片上）拖动即可移动视图，单击仍会打开卡片；在地图、桌面和星图中拖动都不会选中文字。
- **桌面（Table）**：一块自由的工作台面，用来把一组相关 Card 摊开并排，不论它们归档在哪里。按 `p`
  把正在读的 Card 放上桌面（再按一次放回卡盒），按 `P` 连同与它相关的所有 Card 一起放上；也可以在地图中
  Shift-点击卡片，或点搜索结果上的 `+`。点击桌上的卡片即“拿起”它：它被摆正、抬起，与它相关的卡片保持清晰并以
  丝线相连（丝线标出关系类型：依赖、对照……），其余卡片退后；尚未上桌的相邻卡片放在它旁边“触手可及”，点一下即可上桌。
  卡片可任意拖动，拖到卡盒上即放回。**Gather** 把卡片围绕手中那张摆开，**Line up** 沿 Link 按阅读顺序排成行，
  **By tree** 按树各排一列。每组卡片是一个 **Spread**：保存在档案中，Card 移动或改名时随之更新，且不改动任何 Card；
  **File as card** 把它写成一张列出这些卡片的 Card，归入卡片盒。
- **星图（Constellation）**：每棵树画成一个径向大纲，所有树排成一圈；两棵树之间的 Link
  合并为一条按数量加粗的弧线。Pointer 所在的树会被突出。
- **阅读器**（右侧）：把 Card 呈现为一张带横线的索引卡片：地址印章和标题在红线之上，Markdown 写在蓝色横线上；
  Link 标签、Reverse link、操作按钮和存储的 Markdown 原文放在卡片下方。parent/direct/side 移动交给地图和方向键。
- **写作台**：左侧是 Markdown 原文，右侧是实时预览；输入 `[[` 可补全 Link，
  `Ctrl+S` 保存，`Esc` 取消。

命令栏使用与 Session 相同的命令（`go 1/2`、`back`、`fwd`、`n`、`b`、`e`、`t 标题`、`l`、`ws agent`、`del`、`ins`、`mv 1/3`、
`mv alone before 1/2`、`ty event`、`lsbk`、`help`）。删除、移动和修改 Citation key 时需要输入与 CLI 相同的确认文本。快捷键：
`:` 命令栏、`/` 或 `Ctrl+K` 搜索、`Alt+←` / `Alt+→` 后退与前进、`g` 前往、`n` `b` `e` `m` `i`（插入）、`Delete`、`u` 或方向键在地图中移动、
`x` 展开或折叠整棵树、`p` / `P` 把卡片放上桌面、`v` 切换视图、`w` 切换 Workspace、`f` 适配窗口。

GUI 只监听 `127.0.0.1`，服务每次启动都会使用新的访问令牌。打开的 GUI 窗口不算 Session，
`zt down` 不会等待它：窗口的电源按钮（或 `q`）只关闭窗口；服务停止时，打开的窗口会自行关闭，
若正在写作则保留文本并提示服务已停止。服务重新启动后，页面会自动换用新的令牌。

## Workspace

一个归档包含两个 **Workspace**：属于你自己的 **human** Workspace，以及属于 AI agent 的
**agent** Workspace。每个 Workspace 都有两个 **Zone**：Topic zone 和 Literature zone，每个 Zone
都可以容纳任意多棵树。

- 每棵树只属于一个 Workspace；新建的 successor 自动加入所在树的 Workspace。
- 地址在两个 Workspace 之间全局唯一，因此 `[[target]]` Link（以及生成的 Reverse link）可以跨 Workspace。
- Card 和树永远不能在 Workspace 之间移动：`mv` 会拒绝另一个 Workspace 中的目标位置。
- 在 Workspace 出现之前创建的归档会在 `zt up` 时自动升级，所有已有 Card 都归入 human Workspace。

```sh
zt ws                          # 列出两个 Workspace 及其 Zone
zt t --ws agent "Agent topic"  # 在 agent Workspace 中创建 Topic（默认是 human）
zt l --ws agent                # 在 agent Workspace 中创建 Literature Card
```

在 Session 中，ROOT 显示当前 Workspace 的两个 Zone。`ws` 列出两个 Workspace，`ws agent` /
`ws human` 切换；`t` 和 `l` 在当前 Workspace 中创建，沿 Link 进入另一个 Workspace 时会自动切换。
在 GUI 中，卡片柜顶部的两个标签页（或快捷键 `w`、命令 `ws`）用于切换 Workspace；agent 的 Card
带有紫色的 AI AGENT 标记，指向另一个 Workspace 的 Link 会用该 Workspace 的颜色显示。

## Event Card、Finding Card 与 Model Card

每张 Card 都有一个 **Card type**。大多数 Card 是 **Idea Card**（`idea`，默认值）：用自己的话写下一个想法。
Regular Card 也可以是：

- **Event Card**（`event`）：记录一个真实发生的事件（什么、何时、何地、谁，以及报道它的来源），描述部分不做解释；
- **Finding Card**（`finding`）：记录来源中的一个实证结果，正文有六行标签：`测量`、`数据`（样本、时期、地点）、`方法`、`结果`（一个结果，带大小或方向）、`局限`、`来源`；见 `docs/finding-cards-design.md`；
- **Model Card**（`model`）：一个完整模型的入口（它的问题和它说明了什么），带 facet 时则是模型的一个 part（见“模型结构”）。

Idea Card 用普通的 `[[target]]` Link 引用它们，因此 Model Card 的 Reverse link 会列出所有讨论该模型的 Card。
Topic 和 Literature Card 永远是 Idea Card。类型属于 Card 本身：移动、Side successor 压缩编号和修改 Citation key
都不会改变它，它也不影响 Card 的其他任何内容。0.5.0 之前创建的归档会在 `zt up` 时自动升级，所有已有 Card 都是 Idea Card。

- **Session**：非 Idea Card 的视图会在 Location 后显示 `[event]`、`[finding]` 或 `[model]`；`ty event`、`ty finding`、`ty model` 或
  `ty idea` 设置当前 Regular Card 的类型。
- **GUI**：Event、Finding 与 Model Card 在阅读器卡片、地图和搜索结果中带有 EVENT、FINDING 或 MODEL 印章，书桌上的每叠卡片会统计它们的数量；
  Regular Card 的写作台有类型选择器，命令栏也支持 `ty`。
- **MCP**：`create_successor`、`insert_card` 和 `edit_card` 接受 `type`（`idea`、`event`、`finding`、`model`、`hub` 或 `term`；`edit_card` 可以只传 `type`），
  `get_card`、`get_tree` 和 `search_cards` 会返回类型；`search_cards` 可按 `type` 筛选，此时 `query` 可以为空，
  用来列出该类型的全部 Card。
- **导出**：`archive.json` 在 `types` 中列出所有非 Idea Card。

### 模型结构

一个模型往往一张卡写不下。**Model Card** 是模型的入口：写明问题和模型说明了什么。**Model part** 是带 facet 的
Model Card，每张深入呈现模型的一部分。按照卢曼的原则，卡片归档在哪里并不决定它属于什么：part 属于它归档在其下
的 Model Card（作为 Model Card 或另一个 part 的 successor），**也属于每一张链接它的 Model Card**。所以 part 可以
留在它最初写下的位置，一个 part 也可以同时服务几个模型。**模型的结构由撰写者决定**：

- **Model facet**：每个 part 的名字，由作者自拟的简短标签（不超过 32 个字符），如 `设定`、`均衡`、`数值例子`。
  没有必须齐备的集合，也不规定顺序。
- **Emphasis**：part 可以标为 `core`（模型的核心，读者首先要看的）或 `aside`（可以跳过的细节）。
- **关系**：part 链接与它相关的其他 part，并用一句话说明理由；Link 前面的词给出**关系类型**：`rests-on`
  （`依赖`、`基于`）、`illustrates`（`例示`、`案例`）、`contrasts`（`对照`、`区别`）、`relaxes`（`放宽`、`推广`）、
  `corresponds`（`对应`、`类比`）、`objects`（`反驳`、`批评`），没有这类词时为 `refers`。模型内部的这些 Link 构成
  一张图，而不是序列或树；不与任何 part 相关的 part 是**松散**的（例如放在模型旁的案例或变体）。
- **模型邻居**：模型与模型外 Card 之间的 Link（双向，带类型），例如放宽某条假设的 Idea Card，或某个 part 所对应的
  另一个模型。

ZT 根据 Location、facet 和 Link 计算每个模型的结构，除 facet 和 emphasis 外不存储任何结构。Model Card 与 part
之间不必互相链接；`get_model` 只报告不属于任何模型的 part。带有 part 的 Model Card 在其 part 被修改、移走或
不再被它链接之前不能改类型。详见 `docs/open-models-design.md`。

- **Session**：part 显示为 `[model · <facet>]`（以及它的 emphasis）；`ty model <facet>` 把当前 Card 设为 part
  （facet 取该行余下的全部文字），`ty model` 则恢复为 Model Card。
- **GUI**：Model Card 和 part 的上沿有分隔卡式的标签，按构建顺序以 facet 命名（核心标签着墨，
  aside 标签浅淡，共享 part 为虚线并标 ⧉，推导 part 细窄，构建之外的 part 隔开放在最后）；卡片下方是**模型板**：part 按依赖关系排布，每条 `rests-on` 关系一根细线，其他类型的关系画成
  标出类型的虚线弧，松散的 part 放在一旁的托盘里。模型板随读者的注意力而变：打开的 part 被抬起并显示完整标题，
  与它相关的 part 保持着墨并列出类型和理由，核心 part 保持可读，其余退为标签。模型板下方列出打开的卡在模型之外
  的邻居，上方列出共享 part 所服务的其他模型。写作台在 Model 类型旁填写 facet（提示常见的几种）并选择 emphasis。
- **MCP**：`create_successor` 和 `edit_card` 接受 `facet` 和 `emphasis`（空字符串清除）；`get_card` 返回 `facet`、
  `emphasis` 和 `model`；`get_model` 返回整个模型及每张卡的正文、该卡所属的各个模型、带类型的关系、松散的 part
  和模型邻居。
- **导出**：`archive.json` 在 `facets` 中列出每个 Model part 的 facet，在 `emphasis` 中列出每个 core 或 aside part，
  在 `levels` 中列出每张 Model Card 的细化层级。

### 细化层级

每张 Model Card 带有其模型的**细化层级**（zt 0.13.0，见 `docs/model-elaboration-levels-design.md`）：

- **第 1 层 `concept`**：**概念模型**，原书用文字或示意图给出（一个机制、一个分类、一个框架）。Model Card 和至多几张
  part 用文字写明问题、机制和结论；它没有推导 part，`get_model` 把其中的推导 part 列在 `build.beyond_level` 中。
- **第 2 层 `formal`**：**形式模型**，原书用数学给出。它按下文建成自足模型，原书略去的每一步都由一张推导 part 补出。
- 没有层级的 Model Card 尚未分类，其构建报告按形式模型的规则检查。
- **Session**：卡片视图显示 `[model · concept]`；`lv <concept|formal|none>` 设置层级。**GUI**：印章显示
  `model · L1 concept` 或 `model · L2 formal`；写作台为整个模型提供层级选择，命令栏接受 `lv`。**MCP**：
  `create_successor` 和 `edit_card` 接受 `level`（`concept`、`formal`、`1`、`2`，空字符串清除）；
  `get_card`、`get_tree`、`search_cards` 和 `get_model` 返回它。

### 自足的模型

读者只按**构建顺序**读一个形式模型自己的卡片，就应能从零重建整个模型（zt 0.12.0，见
`docs/self-sufficient-models-proposal.md`）：

- **构建顺序**：Model Card 的第一个侧后继是设定（参与者、时序、信息、符号表和 `术语表`）；它的直接后继是**主线**，
  即逐步的推导；例子放在它所例示的那一步旁边（该步的侧后继）；Model Card 后面的侧后继位于构建之外；共享 part
  放在第一个直接或经由其他共享 part 需要它的步骤之前，并排在它自己所依赖的共享 part 之后（zt 0.15.0）。
  所以一个模型可以把另一个模型的整段推导作为**构件**收入，并按顺序阅读。
- **推导 part**（facet 以 `推导` 开头）补出原书略去的步骤，使推导没有跳步；模型板把它画成它所连接的那根线上的细窄纸条。
  一个较长的步骤由一**串**推导 part 承载，张数由步骤的行数决定，每张从上一张的结果出发；`build.chains`
  列出每一串及其前后的成员，串中每个标签标明它的位置（“derivation step 2 of 3”）。共享 part 的前提若是推导 part，
  会附上提示：把它也收入。
- `get_model` 返回 `build`：按构建顺序列出的成员及其 `line`，以及妨碍模型**自足**的问题：`closure`（模型范围内的
  卡以 rests-on 链接了不属于模型的卡，并给出修正办法）、`forward`（某一步依赖后面的步骤）、`unanchored`（某一步没有
  沿 rests-on 回到设定的路径）。共享 part 留在其所属模型中的前提列为 `tolerated`，不算问题。ZT 只报告，不强制。
- **插入与移到两卡之间**：`insert_card`（MCP）、`ins`（Session）和 GUI 的 Insert 操作把新卡放到一张现有卡的位置上；
  那张卡及其子树下移一位，成为新卡的直接后继。`move_card` / `mv` 也接受 `before`（占用另一张卡的位置）和 `alone`
  （只移动这张卡，它的直接后继上移补位）。所有改名的地址先列出确认，所有 Link 随之改写。
- **卡片语言**：卡片文字遵守按中文改写的 ASD-STE100（一句一事、短句、不用 `；`、主动语态、一个术语一个意思）；
  `zt-check` (`tools/zt-check`) 负责检查。

### 数学公式与 HTML 图表

GUI 用内置的 KaTeX 排版 TeX（无需联网）：`$...$` 行内、`$$...$$` 独立成行，`\(...\)` 与 `\[...\]` 也可以；标题中也可以写
`$...$`。**HTML 图表**写在 ```` ```chart ```` 代码块中（或以 `<figure>`、`<div>`、`<svg>`、`<table>` 等开头的原始 HTML 块），
显示为卡片上的一幅插图。每个图表运行在独立的沙箱页面中，与 GUI 及其令牌隔离，因此可以使用脚本（以及通过 HTTPS 加载的库）；
它通过 `--zt-*` CSS 变量（`--zt-ink`、`--zt-model`、`--zt-series-1` 等）使用卡片的配色，随明暗主题切换，会排版其中的 TeX，
读者点击 `[[地址]]` 文本或带 `data-zt="<地址>"` 的元素即可打开对应 Card。其他原始 HTML 以文本显示，
`<sub>`、`<sup>`、`<br>`、`<kbd>` 等简单行内标签除外。

### 消化一本书

Claude Code 工作流 `zt-econ-digest`（经济学书）和 `zt-math-digest`（数学教材）由同一个基础蓝图构建，把 Scholar Archive 中的一本书转成 agent Card。它们只建这本书的 **Literature 树**。DeepSeek Harness 逐节消化；**Term-card 分支**在书的术语表之后与消化写手并行，写出 Term Card（每批一个写手、一次独立的数学复核和一次有限的修正，共用 DeepSeek 池）；全书消化之后，模型编译器把其中写出的每个模型建到它的细化层级（Sonnet 架构师只规划每个模型的结构；脚本写骨架；DeepSeek 写卡片和图表，再写推导分卡；另一个复核把跳步或过密的推导拆开）；随后连接全书各部分，由 Opus 整理结构；书中的图随书进入卡片；报告优先的 Bridge 找出跨书连接；最后，确定性的 **Term-link 流程**在所有写手之后连接最终正文。Topic 树、Topic Card、枢纽卡与索引块属于 `zt-idea-grow`：它在消化完成后从这棵 Literature 树生长出来，运行报告会给出这一步的后续命令。

`mode` 用单个阶段代替完整运行：`full`、`resume`（每次运行一次）、`link`、`cards`、`models`、`explain`、`events`、`findings`、`figures`、`audit`、`bridge`、`term-cards`、`term-link`，数学变体再加 `math`。两个 Term 模式为**一本已经消化完的书**补建术语：`term-cards` 只准备或复用这本书已暂存的源切片，并按表格顺序发布 Term Card（只发布已就绪的前缀；失败的行留在暂存区），`term-link` 对这本书当前的 Term Card 执行确定性链接。两者都不会重跑消化、模型、整理或 Bridge 阶段；除 Term 设置外只接受 `out`：`termCards`（默认 true，Term-card 分支）、`termLink`（默认 true，最终链接流程）、`termCardsPerJob`（1–8，默认 4 个概念术语/写手）、`termCardsMathPerJob`（1–8，默认 1 个数学术语）与 `termLinkOccurrences`（`first` 默认，或 `all`）。这些设置在准备阶段就冻结进运行契约。

写手在提交前检查自己写出的确切草稿。`card_check` 拒绝属于内容问题：写手在该任务有限的修正轮次内改好，不当作脚本或网络故障。`card_locked` 是选址决定：阶段停止，保留已写出的草稿与回执，绝不重试、解锁或改换写入路径。审计运行过不等于所有术语都通过：它的 `terms` 段单独报告覆盖率、T 规则故障、term use、未解决的行与检查契约，使执行成功、发布完整性与内容质量互不冒充。

详见 [docs/zt-digest-workflow.md](docs/zt-digest-workflow.md) 与
[zt-digest-protocol skill](skills/zt-digest-protocol/SKILL.md)。

### 枢纽卡

**枢纽卡**（类型 `hub`，zt 0.16.0）是进入一个主题的入口：它在 `##` 组标题下每个成员占一行，写成 `- [[地址]]：这张卡贡献什么`，并在 `## 邻近` 组里指向别的树。它的**枢纽层级**说明主题有多宽：`line`（一条思路）、`topic`（一棵主题树，由 Topic 正文链接）、`field`（几个主题）、`register`（整个工作区的关键词，放在主题 `索引` 里）。枢纽只列更低层级的枢纽。`get_hub` 报告一张枢纽的各行、上下层枢纽和问题（过大、没有理由、没有走出本树、只按地址抄了本树等）；`list_hubs` 给出枢纽图和工作区还缺的枢纽；`get_card` 列出列入一张卡的枢纽。GUI 把枢纽画成导卡：主题越宽，卡顶的标签越靠左；每张卡的标题上方显示可以上溯的枢纽。`zt-idea-grow` 在消化之后写出它们：Haiku 枢纽经理根据普查选定阈值，DeepSeek 规划领域、写每张枢纽，`scripts/book-digest/hub-compiler.ps1` 建卡并检查。详见 [docs/hub-cards-design.md](docs/hub-cards-design.md)。

**索引卡**（层级 `register`）是工作区里唯一的索引枢纽，是主题 `索引` 的第一张卡。它的 `## 词条` 组每行一个词条，写成 `- 学习：[[5/1]] [[5/2]]`：冒号前是词条，后指一到三张承载这个词条的卡；范围写在词条里，解释写在目标卡里。词条超过 120 行时，这些行搬进 `## 词条分册` 声明的普通 Idea Card，每张对应一个公认的拼音首字母区间（`- [[2/1|a]] A–F`；读音缺失或不确定的词条进 `其他`）。每张分册有自己的 `## 词条` 组，是索引卡的直接侧后继，自己不能再有分册。`get_register` 把这些行当作一个可分页的整体来读；带上 `at` 时，它还会给出一张卡承载的全部公认词条——命名它的词条行，以及它自己实质性标题或正文里写到的词条——`scripts/book-digest/register-compiler.ps1` 规划、发布并复查它。

## MCP 服务器

`zt mcp` 通过 stdio 运行一个 [Model Context Protocol](https://modelcontextprotocol.io) 服务器，
让编程 agent 也能使用卡片盒。它可以**读取两个 Workspace**，但**只能修改 agent Workspace**：
对 human Workspace 的任何写入都会被拒绝；如果 agent 的修改会改写 human Card 中的 Link（例如移动或
重命名一张被 human Card 链接的 agent Card），同样会被拒绝。agent 链接到 human Workspace 是允许的，
这只会让 human Card 生成新的 Reverse link。

| 工具 | 作用 |
| --- | --- |
| `list_workspaces` | 两个 Workspace、各自的 Zone、Card 数量以及你的权限 |
| `list_trees` | 按 Workspace 和 Zone 筛选的树根 |
| `get_tree`、`get_card` | 一整棵树，或一张 Card 及其相邻 Card 与出入 Link |
| `get_terms` | 一张术语表的计算结构：四列行、各 head、part 与 unfiled Card、已存的 term use、覆盖率，以及结构问题（七种放置/覆盖问题，加上 T8 的表格形状问题和隐藏注释）；同时给出读取时的归档 revision |
| `get_model` | 整个模型：Model Card 与各 part（归档在其下或被它链接）的正文、facet 和 emphasis，带类型的关系、松散的 part、模型外的邻居，以及 `build` 报告（构建顺序与自足性问题） |
| `search_cards`、`list_links`、`list_broken_links` | 全文搜索（可限定 Card `type`；`mode: entry` 先给出登记簿词条及其入口卡、hub 成员行和模型入口，再给普通标题与正文命中；已确认的同述入口，或两张陈述指纹完全相同的 Idea Card，合成一行，并给出规范陈述卡与全部成员）、Link 图（可只看跨 Workspace 的 Link）、Broken link |
| `create_topic`、`create_literature` | 在 agent Workspace 中新建 Topic 树或 Literature 树 |
| `create_successor` | 为 agent Card 添加 direct 或 side successor，可以是 Event、Finding、Model、Hub 或 Term Card（`type`；Model Card 的 `level`/`form`、Model part 的 `facet`/`emphasis`、Hub Card 的 `level`、Term Card 的 `term` 名称） |
| `edit_card`、`edit_literature_metadata` | 修改 agent Card 的标题、正文、类型、术语名、facet、emphasis、层级、hub 层级、数学形式或 BibTeX |
| `delete_card`、`move_card` | 删除或移动 agent Card（删除计划列出会断开的 Link，`into` 把它们改指一张保留的卡；`move_card` 用 `to` 移到空位，用 `before` 占用另一张卡的位置，用 `alone` 不带后继） |
| `insert_card` | 把新的 agent Card 插到另一张卡的位置上；那张卡及其子树下移 |

删除、移动、插入和修改 Citation key 会先返回一份计划；用计划中给出的 `confirm` 文本（`delete`、树根地址、
`move` 或 `insert`）再次调用才会真正执行。使用前需要先启动服务。

zt 附带三个 agent skill：`zt`（`skills/zt/SKILL.md`）说明 Workspace、工具、Term Card 与 Term use 以及规则；
`zettelkasten`（`skills/zettelkasten/SKILL.md`）依据卢曼（Niklas Luhmann）的做法和后来实践者的经验，说明如何撰写一张
Card 以及如何组织 Card 之间的关系（资料来源见 `skills/zettelkasten/references/`）；`zt-digest-protocol`
（`skills/zt-digest-protocol/SKILL.md`）是消化工作流及其工具的操作手册。要让每个 Claude Code、Codex 和 DeepSeek
Harness 对话都能使用这些 skill，并让 Claude Code 与 Codex 能使用该服务器，在 Windows 上运行一次以下命令；更新
skill 或移动 `zt` 可执行文件后再运行一次：

```powershell
pwsh -File scripts/install-agent-integrations.ps1
```

它会把每个 skill 复制到 `~/.claude/skills/<name>` 与 `~/.codex/skills/<name>`，把 `~/.dsh/skills/<name>`
链接到 Codex 副本供 DeepSeek Harness 使用，在 Claude Code 中把 `zt` 注册为用户级
MCP 服务器（并允许其工具），在 Codex 中注册为全局服务器，并检查服务器握手；修改每个配置文件前都会先备份。
已经打开的对话仍使用原来的服务器。在其他环境中，可以用 `claude mcp add -s user zt -- zt mcp` 或
`codex mcp add zt -- zt mcp` 手动注册；其他 MCP 客户端也可以用同样的方式把 `zt mcp` 作为 stdio 服务器启动。

## Scholar Archive

ZET 通过 **Scholar Archive bridge** 读取本机的 [Scholar Archive](https://github.com/liudesheng9/google_scholar_archive)。
这是一条经过认证的只读连接，两个项目各有一个连接器。ZET 能看到 archive 中的全部信息：学者、Work 及其来源、版本、
摘要与主题、期刊、workspace、来源更新记录、关联的 PDF 与 Markdown 文档、通过只读 SQL 访问的任意数据表、语义搜索，
以及完整导出。通过 bridge 进行的任何操作都不会修改 archive，读到的内容也不会修改任何 Card。

连接会自动运行。首次使用时，ZET 会运行 `scholar-archive bridge pair`（随 Scholar Archive 一起安装）完成配对，
并把凭据保存在 ZET 配置目录下的 `scholar-bridge.json` 中。每个请求都用该密钥签名，ZET 也只接受用同一密钥签名的回复，
因此只有已配对的 ZET 能读取 archive，ZET 也能确认读到的是真正的 archive。如果 archive 不再认识这份凭据，ZET 会重新配对；
如果服务器未运行，ZET 会启动它。

```text
zt scholar                         # 连接并显示 archive 的标识与数量
zt scholar tools                   # archive 的只读工具
zt scholar call list_works '{"query":"democracy","limit":5}'
zt scholar call get_work '{"work_id":42}'
zt scholar sql "SELECT id, title, year FROM works WHERE doi IS NOT NULL LIMIT 10"
zt scholar figures 42              # 一个 Work 的图：编号、图题、file id 与路径
zt scholar figure 22 nber/w1_mineru/images/a1.jpg   # 收下一张图，并打印显示它的 Markdown
zt scholar keep-figures 83 --min-bytes 3000 --out figs.json   # 收下一份 Markdown 的全部图
zt scholar file 22                 # 获取文档（校验 SHA-256）并存入 ZET 缓存
zt scholar export --format bibtex --out works.bib
zt scholar pair --rotate           # 换发新密钥；`zt scholar unpair` 断开连接
```

`zt mcp` 以五个只读工具向 agent 提供同样的读取能力：`scholar_archive_status`、`scholar_archive_read`
（archive 的任意只读工具）、`scholar_archive_sql`、`scholar_archive_file`（Markdown 以文本返回，PDF 返回缓存路径）
和 `scholar_archive_export`。这些工具不需要启动 ZET 服务。第六个工具 `scholar_archive_artifact` 会写入：
它把一张图复制进 ZET 的 artifact 存储（见下文），所以需要 ZET 服务处于运行状态。协议见
[docs/scholar-archive-bridge-design.md](docs/scholar-archive-bridge-design.md)。


### 来自 archive 的图

Scholar Archive 会列出每个 Work 的 **artifact**：其 Markdown 显示的图与表的图片及图题（`list_work_artifacts`）。
Card 用 `![图题](artifact:<sha256>)` 显示一张图。ZET 把图片本身只存一次，放在 `<archive_root>/artifacts/<sha256>.<ext>`，
旁边的 `.json` 记录来源，因此 archive 整理文件夹不会让 Card 失效，`zt dp` 的转储也会带上 Card 显示的图。
Agent 用 `scholar_archive_artifact` 工具取图，它返回可直接粘贴的 Markdown；MCP 服务器拒绝显示 ZET 未保存的图的 Card。
在 GUI 中，编辑器里的 **＋ Figure** 打开选图面板，从当前 Literature 树对应的 Work 开始。Card 中的图像一张夹在卡片上的照片，点击即可放大，放大后的链接可在原书中打开这张图所在的位置。

### 来自 archive 的文档

**文档链接** `[文字](scholar:<file_id>)` 指向 archive 管理的一整份 PDF 或 Markdown 文档（在 id 后加 `#page=12` 可打开 PDF 的指定页）。
GUI 把它显示为带小页面图标的链接，并在卡片盒旁打开：PDF 用浏览器自带的阅读器，Markdown 显示为带原图和排版公式的阅读页。
服务通过 bridge 取得文档，并在 archive 中的副本大小不变时使用 ZET 缓存。编辑器的 **＋ Figure or document** 面板在图的上方列出 Work 的文档，
`scholar_archive_file` 也以 `link` 返回这条链接。

## Card 检查与策略

每条写入路径——Shell（`n`、`b`、`e`、`ins`、`del`、`mv`）、行式与全屏 Session、GUI 路由、MCP 工具以及一次批量或
预览——都在自己的写入事务内、对即将存储的那张确切 Card 做同一次 Card 检查。格式、Link、Access 与 Lock 的强制规则
总会拒绝：拒绝带有类型（`card_check`、`card_locked` 等），说明原因，且不存储任何 Card、回执、revision 或 Topic
分配。被接受的写入连同它的建议一起存储，因此警告从不需要牺牲草稿。检查只读取有界上下文（父卡与模型层级、书的术语表、
不同的 Link 目标、Term Card 的角色与表格行）；图规则只留在审计里；一次真实批量至多 64 项，预览序列至多检查 200 项。

检查器拥有规则目录，归档只保存自己的覆盖项：

```sh
zt config policy                                   # gate 阶段、policy 哈希、revision、规则版本与每条规则
zt config set gate.stage enforce                   # 或 advisory（默认）/ default
zt config set gate.human.R3 advise                 # 按 Workspace 与规则：block / advise / off / default
zt config set gate.agent.R6 default                # 显式 default：删除该行
```

新归档从 `advisory` 开始：每条质量诊断都作为建议返回，Card 照常存储，因此新规则不会让已有的写手中途卡死。`enforce`
才允许 `block` 规则拒绝。这个阶段由 owner 主动升级，绝不由计时器自动推进。已有 Card 上保留的（legacy）故障仍是建议，
同一故障的新出现则在 `enforce` 下阻止写入，所以旧归档不会变得无法编辑；强制不变量在两个阶段都会拒绝。把阶段退回
`advisory` 只改变哪些质量问题会拒绝写入，绝不改变语法/模式兼容边界。

## 导出

`zt dp` 会在 `<archive_root>/dump/` 下写出一个压缩归档，完整记录整个卡片系统：

- `mapping.json`：每个 Card 地址到其 Markdown 文件的映射。
- 每张 Card 一个 Markdown 文件，内容是其存储的原文，因此作者写下的 Term 链接保留原始 `[[label@address]]` 宏，
  Term Card 保留生成的 `## Term uses` 段；Literature Card 的文件以包含 BibTeX 的 YAML front matter 开头，
  `<citation-key>.bib` 保存 BibTeX 的原始字节。
- `archive.json`：每个 Card 地址所属的 Workspace、下一个 Topic 编号（因此已删除的 Topic 编号不会被重用），每张 Event、
  Finding、Model、Hub 与 Term Card 的类型，每个有名字的 Term Card 与 Term part 的术语名（Term list 与其他 Card 都不出现，
  与 Idea Card 不出现在类型表中相同），每个 Model part 的 facet，每个 core 或 aside part 的 emphasis，以及每张有细化层级的
  Model Card 的层级，例如
  `{"next_topic_id":3,"workspaces":{"0/0":"human","0/1":"human","0/1|a":"human","1/0":"agent","1/1|a":"agent"},"types":{"0/1":"model","0/1|a":"model","1/1|a":"term"},"term_names":{"1/1|a":"均衡"},"facets":{"0/1|a":"setting"},"emphasis":{"0/1|a":"core"},"levels":{"0/1":"formal"}}`。
- `locks.json`：同一次读取的 Tree Lock 证据——每个 Lock（生效或已结束）及其封条；生效 Lock 的成员快照与 pin，
  含地址、行身份与作者指纹；全部 Lock 事件（按顺序）；以及完整性事实。归档没有 Lock 时也会写出；发现故障时只作为
  证据记录，不会阻止诊断性导出。它只记录历史：导出不会恢复任何内容，也不会导入 Lock。

`zt dp` 只列出主机上的工具真正能写出的格式（`zip`、`tar.gz`、`tar.zst`、`7z`）：它会依次尝试 `PATH` 上的每个
`tar`、`zip` 或 `7z`，保留第一个其探测归档带有正确格式签名的工具。在 Windows 上，Git for Windows 自带的 GNU tar
会被跳过，改用系统自带的 `tar.exe`。

含有 Term Card 或 Term 链接的归档与旧版 `zt` 不向后兼容：归档的 generation 标记会让旧二进制拒绝启动，而不是悄悄丢掉
`term_name`，或把 `[[label@address]]` 宏当成普通 Link 读取。`zt version --json` 发布本二进制安装并理解的
generation，`docs/term-cards-links-and-check-gate-release-manifest.json` 记录这套兼容发布集。把 Card 检查的 gate 退回
`advisory` 只改变哪些质量问题会拒绝写入，绝不改变这条语法/模式边界；Term 数据存在之后的完整二进制降级需要经过审查的
转换或 Term 之前的备份恢复。

## 删除

一条命令覆盖全部删除。它会要求该范围对应的确认词，并在提示之前拒绝仍在生效的 Locked tree：

```sh
zt del --at <card>                  # 删掉该 Card 及其子树（确认词：delete）
zt del --at <root>                  # 删掉整棵树，含树根（确认词：树根地址）
zt del --at <root> --keep-root      # 清空整棵树，保留树根（确认词：树根地址）
zt del --all                        # 删掉全部 Card 并重置 Topic 编号（确认词：delete all）
```

`--keep-root` 留下的是 **Emptied tree**（清空的树）：树根保留其行身份、地址、类型、Workspace、
BibTeX 与正文，来自外部的 Link 仍然有效，下一个 Direct successor 重新是 `<root>/1`，全局 Topic
编号不变。`zt del --all` 就是原来的 `zt clear`：本版本中 `zt clear` 只打印一条指向 `zt del --all`
的提示并以 1 退出，下一版本会移除这个提示处理器。

## 创建 Literature Card

在 Session 中运行 `l`，或在 Shell 中使用 `$EDITOR` 运行 `zt l`。先输入并保存一个完整的 BibTeX 条目，再输入并保存 Card 正文。BibTeX 条目的 key 会成为 Citation key，其中的 `title` 字段会成为 Card 标题。

```bibtex
@book{Smith2024,
  title = {示例书籍}
}
```

在 Session 中，每个阶段按 `Ctrl+S` 保存，按 `Esc` 取消。创建后可以使用 `go Smith2024` 打开，也可以使用 `[[Smith2024]]` 创建 Link。

## Literature 树

每个 Literature Card 都是一棵独立树的根，用来记录关于这篇文献的笔记。Literature Card
保留其 BibTeX 元数据；树中的其他 Card 都是 Regular Card：

```text
go Smith2024   # 打开 Literature Card
n              # 创建 Smith2024/1
n              # 在 Smith2024/1 上创建 Smith2024/2
b              # 在 Smith2024/2 上创建 Smith2024/2|a
```

Literature 树中的 Card 遵循与 Topic 树相同的后继、Link、移动和删除规则，但有一条边界：
它们只能在所属的 Literature 树内移动，Topic 树（idea 树）中的 Card 也不能移入 Literature 树。
通过 `e` → metadata 修改 Citation key 会重命名整棵树并重写所有指向它的 Link。
删除 Literature Card 会删除整棵树，并要求输入其 Citation key 确认，与删除 Topic 时输入 Location 一致。

## Card 地址规则

ZT 不给 Topic 和 Regular Card 分配任意 ID，而是自动为它们分配 **Location**：

- Topic 使用 `<主题编号>/0`；新档案从 `0/0` 开始，然后是 `1/0`，依此类推。普通删除不会复用 Topic 编号。
- `n` 创建数字形式的 Direct successor：`0/0` -> `0/1` -> `0/2`。
- `b` 创建 Regular Card 的下一个字母形式 Side successor：`0/2|a`、`0/2|b`、...、`0/2|z`、`0/2|aa`。
- Side successor 后的 Direct successor 会增加新的数字段：`0/2|a` -> `0/2|a|1`。
- Literature Card 没有 Location；它使用 BibTeX 条目的 key 作为 Citation key。
- Literature 树中的 Card 用 Citation key 代替 Topic 编号：`Smith2024/1`、`Smith2024/1|a`。

常用 Shell 命令：

```sh
zt help
zt gui
zt status
zt autostart [on|off]
zt stats
zt ws
zt mcp
zt lsbk
zt dp
zt down
```
