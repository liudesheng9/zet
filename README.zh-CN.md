# ZT

[English](README.md) | 中文

ZT 是一个本地优先的卡片笔记系统：每张 Card 都是可寻址的 Markdown 文档，既可以在终端 Session 中使用，也可以在卡片盒 GUI 中浏览和编辑。

## 功能

- 所有 Card 都用 Markdown 书写：`# 标题`、Markdown 正文，以及自动生成的 Reverse link。
- 使用 `zt gui` 浏览与编辑：卡片柜、按地址排布的 Folgezettel 地图、把各处相关 Card 并排摊开的桌面（Table）、全部 Card 的星图（Constellation），以及带实时预览的 Markdown 写作台。
- 使用 Topic 树和 Literature 树组织笔记：每个 Topic 与每个基于 BibTeX 的 Literature Card 都是一棵独立 Regular Card 树的根。
- 可以把 Regular Card 标为 Event Card（一个真实发生的事件）或 Model Card（一个完整的模型），引用它的其他 Card 用 Link 指向它。
- 维护两个 Workspace：一个属于你，一个属于 AI agent，各有 Topic zone 与 Literature zone；Link 可以跨 Workspace，Card 不能在 Workspace 之间移动。
- 通过 `zt mcp` 服务器让编程 agent 在 agent Workspace 中工作，而你的 Workspace 对它只读。
- 在交互式终端 Session 中，通过 Location 或 Citation key 导航 Card。
- 使用 `[[target]]` Link 连接 Card，并自动生成 Reverse link、检查 Broken link。
- 在 Session 或 Shell 中安全地创建、编辑、移动和删除 Card 树。
- 使用本地 SQLite 存储数据，并提供状态、统计、导出和清空命令。
- 使用 `zt autostart on` 让服务在登录时启动、在机器关机时停止（Windows）。

## 安装

安装 Rust 工具链，克隆本仓库，然后运行：

```sh
cargo install --path . --locked
```

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
写在 `行内代码` 或代码块中的内容只是普通文本。终端不会渲染 Markdown：Session 会原样显示和编辑
存储的 Markdown。

0.2.0 之前创建的档案会在 `zt up` 时自动转换；转换前会先把原数据库备份为档案目录中的
`zt.sqlite3.pre-markdown-<时间>.bak`。

## GUI

服务启动后运行：

```sh
zt gui                 # 打开应用窗口（Edge/Chrome）或默认浏览器
zt gui --no-open       # 只打印本地地址
zt gui --port 4717     # 指定端口
```

- **卡片柜**（左侧）：human 与 agent 两个 Workspace 的标签页、当前 Workspace 的 Topic zone 与 Literature zone，以及全文搜索。
- **书桌**（ROOT）：当前 Workspace 的每棵树显示为一叠索引卡片，按 Zone 分组。
- **地图（Map）**：用聚焦透镜呈现当前树的 Folgezettel 地图。Direct successor 沿同一列向下延伸，
  Side successor 进入右边一列。Pointer 所在的卡片被拉到前面；它的路径、后继和相邻卡片以小卡片立起；
  其余卡片只露出一行卡头，远离 Pointer 的分支折叠为 "+N" 卡叠。只画 Pointer 的 Link。按 `x` 展开整棵树。
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

命令栏使用与 Session 相同的命令（`go 1/2`、`n`、`b`、`e`、`t 标题`、`l`、`ws agent`、`del`、`mv 1/3`、
`ty event`、`lsbk`、`help`）。删除、移动和修改 Citation key 时需要输入与 CLI 相同的确认文本。快捷键：
`:` 命令栏、`/` 搜索、`g` 前往、`n` `b` `e` `m`、`Delete`、`u` 或方向键在地图中移动、
`x` 展开或折叠整棵树、`p` / `P` 把卡片放上桌面、`v` 切换视图、`w` 切换 Workspace、`f` 适配窗口。

GUI 只监听 `127.0.0.1`，每次启动都会使用新的访问令牌，并且会被计为一个 Session，
因此需要先用电源按钮或 `Ctrl+C` 关闭 GUI，`zt down` 才能停止服务。

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

## Event Card 与 Model Card

每张 Card 都有一个 **Card type**。大多数 Card 是 **Idea Card**（`idea`，默认值）：用自己的话写下一个想法。
Regular Card 也可以是：

- **Event Card**（`event`）：记录一个真实发生的事件（什么、何时、何地、谁，以及报道它的来源），描述部分不做解释；
- **Model Card**（`model`）：一个完整模型的入口（它的问题和它说明了什么），带 facet 时则是模型的一个 part（见“模型结构”）。

Idea Card 用普通的 `[[target]]` Link 引用它们，因此 Model Card 的 Reverse link 会列出所有讨论该模型的 Card。
Topic 和 Literature Card 永远是 Idea Card。类型属于 Card 本身：移动、Side successor 压缩编号和修改 Citation key
都不会改变它，它也不影响 Card 的其他任何内容。0.5.0 之前创建的归档会在 `zt up` 时自动升级，所有已有 Card 都是 Idea Card。

- **Session**：非 Idea Card 的视图会在 Location 后显示 `[event]` 或 `[model]`；`ty event`、`ty model` 或
  `ty idea` 设置当前 Regular Card 的类型。
- **GUI**：Event 与 Model Card 在阅读器卡片、地图和搜索结果中带有 EVENT 或 MODEL 印章，书桌上的每叠卡片会统计它们的数量；
  Regular Card 的写作台有类型选择器，命令栏也支持 `ty`。
- **MCP**：`create_successor` 和 `edit_card` 接受 `type`（`idea`、`event` 或 `model`；`edit_card` 可以只传 `type`），
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
不再被它链接之前不能改类型。详见 `doc/open-models-design.md`。

- **Session**：part 显示为 `[model · <facet>]`（以及它的 emphasis）；`ty model <facet>` 把当前 Card 设为 part
  （facet 取该行余下的全部文字），`ty model` 则恢复为 Model Card。
- **GUI**：Model Card 和 part 的上沿有分隔卡式的标签，按 Model Card 列出 part 的顺序以 facet 命名（核心标签着墨，
  aside 标签浅淡）；卡片下方是**模型板**：part 按依赖关系排布，每条 `rests-on` 关系一根细线，其他类型的关系画成
  标出类型的虚线弧，松散的 part 放在一旁的托盘里。模型板随读者的注意力而变：打开的 part 被抬起并显示完整标题，
  与它相关的 part 保持着墨并列出类型和理由，核心 part 保持可读，其余退为标签。模型板下方列出打开的卡在模型之外
  的邻居，上方列出共享 part 所服务的其他模型。写作台在 Model 类型旁填写 facet（提示常见的几种）并选择 emphasis。
- **MCP**：`create_successor` 和 `edit_card` 接受 `facet` 和 `emphasis`（空字符串清除）；`get_card` 返回 `facet`、
  `emphasis` 和 `model`；`get_model` 返回整个模型及每张卡的正文、该卡所属的各个模型、带类型的关系、松散的 part
  和模型邻居。
- **导出**：`archive.json` 在 `facets` 中列出每个 Model part 的 facet，在 `emphasis` 中列出每个 core 或 aside part。

### 数学公式与 HTML 图表

GUI 用内置的 KaTeX 排版 TeX（无需联网）：`$...$` 行内、`$$...$$` 独立成行，`\(...\)` 与 `\[...\]` 也可以；标题中也可以写
`$...$`。**HTML 图表**写在 ```` ```chart ```` 代码块中（或以 `<figure>`、`<div>`、`<svg>`、`<table>` 等开头的原始 HTML 块），
显示为卡片上的一幅插图。每个图表运行在独立的沙箱页面中，与 GUI 及其令牌隔离，因此可以使用脚本（以及通过 HTTPS 加载的库）；
它通过 `--zt-*` CSS 变量（`--zt-ink`、`--zt-model`、`--zt-series-1` 等）使用卡片的配色，随明暗主题切换，会排版其中的 TeX，
读者点击 `[[地址]]` 文本或带 `data-zt="<地址>"` 的元素即可打开对应 Card。其他原始 HTML 以文本显示，
`<sub>`、`<sup>`、`<br>`、`<kbd>` 等简单行内标签除外。

### 消化一本书

Claude Code 工作流 `zt-book-digest` 把 Scholar Archive 中的一本书转成 agent Card：DeepSeek Harness 逐节消化，
每一批完成后，由 Sonnet 把其中写出的每张 Model Card 对照原文重建为带图表的结构化模型，随后连接全书各部分，最后由 Opus 整理结构。
详见 [doc/zt-book-digest-workflow.md](doc/zt-book-digest-workflow.md)。

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
| `get_model` | 整个模型：Model Card 与各 part（归档在其下或被它链接）的正文、facet 和 emphasis，带类型的关系、松散的 part 和模型外的邻居 |
| `search_cards`、`list_links`、`list_broken_links` | 全文搜索（可限定 Card `type`）、Link 图（可只看跨 Workspace 的 Link）、Broken link |
| `create_topic`、`create_literature` | 在 agent Workspace 中新建 Topic 树或 Literature 树 |
| `create_successor` | 为 agent Card 添加 direct 或 side successor，可以是 Event 或 Model Card（`type`），或 Model part（`facet`、`emphasis`） |
| `edit_card`、`edit_literature_metadata` | 修改 agent Card 的标题、正文、类型、facet、emphasis 或 BibTeX |
| `delete_card`、`move_card` | 删除或移动 agent Card |

删除、移动和修改 Citation key 会先返回一份计划；用计划中给出的 `confirm` 文本（`delete`、树根地址或
`move`）再次调用才会真正执行。使用前需要先启动服务。

zt 附带两个 agent skill：`zt`（`skills/zt/SKILL.md`）说明 Workspace、工具和规则；`zettelkasten`
（`skills/zettelkasten/SKILL.md`）依据卢曼（Niklas Luhmann）的做法和后来实践者的经验，说明如何撰写一张 Card
以及如何组织 Card 之间的关系（资料来源见 `skills/zettelkasten/references/`）。要让每个 Claude Code、Codex 和
DeepSeek Harness 对话都能使用这两个 skill，并让 Claude Code 与 Codex 能使用该服务器，在 Windows 上运行一次以下命令；更新 skill 或移动 `zt` 可执行文件后再运行一次：

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
zt scholar file 22                 # 获取文档（校验 SHA-256）并存入 ZET 缓存
zt scholar export --format bibtex --out works.bib
zt scholar pair --rotate           # 换发新密钥；`zt scholar unpair` 断开连接
```

`zt mcp` 以五个只读工具向 agent 提供同样的读取能力：`scholar_archive_status`、`scholar_archive_read`
（archive 的任意只读工具）、`scholar_archive_sql`、`scholar_archive_file`（Markdown 以文本返回，PDF 返回缓存路径）
和 `scholar_archive_export`。这些工具不需要启动 ZET 服务。协议见
[doc/scholar-archive-bridge-design.md](doc/scholar-archive-bridge-design.md)。


### 来自 archive 的图

Scholar Archive 会列出每个 Work 的 **artifact**：其 Markdown 显示的图与表的图片及图题（`list_work_artifacts`）。
Card 用 `![图题](artifact:<sha256>)` 显示一张图。ZET 把图片本身只存一次，放在 `<archive_root>/artifacts/<sha256>.<ext>`，
旁边的 `.json` 记录来源，因此 archive 整理文件夹不会让 Card 失效，`zt dp` 的转储也会带上 Card 显示的图。
Agent 用 `scholar_archive_artifact` 工具取图，它返回可直接粘贴的 Markdown；MCP 服务器拒绝显示 ZET 未保存的图的 Card。
在 GUI 中，编辑器里的 **＋ Figure** 打开选图面板，从当前 Literature 树对应的 Work 开始。Card 中的图像一张夹在卡片上的照片，点击即可放大。

## 导出

`zt dp` 会在 `<archive_root>/dump/` 下写出一个压缩归档，完整记录整个卡片系统：

- `mapping.json`：每个 Card 地址到其 Markdown 文件的映射。
- 每张 Card 一个 Markdown 文件，内容是其存储的原文；Literature Card 的文件以包含 BibTeX 的 YAML front matter 开头，
  `<citation-key>.bib` 保存 BibTeX 的原始字节。
- `archive.json`：每个 Card 地址所属的 Workspace、下一个 Topic 编号（因此已删除的 Topic 编号不会被重用），每张 Event 与 Model Card 的类型，每个 Model part 的 facet，以及每个 core 或 aside part 的 emphasis，例如
  `{"next_topic_id":3,"workspaces":{"0/0":"human","0/1":"human","0/1|a":"human","1/0":"agent"},"types":{"0/1":"model","0/1|a":"model"},"facets":{"0/1|a":"setting"},"emphasis":{"0/1|a":"core"}}`。

`zt dp` 只列出主机上的工具真正能写出的格式（`zip`、`tar.gz`、`tar.zst`、`7z`）：它会依次尝试 `PATH` 上的每个
`tar`、`zip` 或 `7z`，保留第一个其探测归档带有正确格式签名的工具。在 Windows 上，Git for Windows 自带的 GNU tar
会被跳过，改用系统自带的 `tar.exe`。

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
