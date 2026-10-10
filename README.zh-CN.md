# ZT

[English](README.md) | 中文

ZT 是一个本地优先的卡片笔记系统：每张 Card 都是可寻址的 Markdown 文档，既可以在终端 Session 中使用，也可以在卡片盒 GUI 中浏览和编辑。

## 功能

- 所有 Card 都用 Markdown 书写：`# 标题`、Markdown 正文，以及自动生成的 Reverse link。
- 使用 `zt gui` 浏览与编辑：卡片柜、按地址排布的 Folgezettel 地图、全部 Card 的星图（Constellation），以及带实时预览的 Markdown 写作台。
- 使用 Topic 树和 Literature 树组织笔记：每个 Topic 与每个基于 BibTeX 的 Literature Card 都是一棵独立 Regular Card 树的根。
- 维护两个 Workspace：一个属于你，一个属于 AI agent，各有 Topic zone 与 Literature zone；Link 可以跨 Workspace，Card 不能在 Workspace 之间移动。
- 通过 `zt mcp` 服务器让编程 agent 在 agent Workspace 中工作，而你的 Workspace 对它只读。
- 在交互式终端 Session 中，通过 Location 或 Citation key 导航 Card。
- 使用 `[[target]]` Link 连接 Card，并自动生成 Reverse link、检查 Broken link。
- 在 Session 或 Shell 中安全地创建、编辑、移动和删除 Card 树。
- 使用本地 SQLite 存储数据，并提供状态、统计、导出和清空命令。

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
- **地图（Map）**：把当前树画成 Folgezettel 地图。Direct successor 沿同一列向下延伸，
  Side successor 分出新的一列，Link 显示为虚线弧。
- **星图（Constellation）**：所有 Card 与 Link 组成的一张图，按树着色。
- **阅读器**（右侧）：把 Card 呈现为一张带横线的索引卡片：地址印章和标题在红线之上，Markdown 写在蓝色横线上；
  Link 标签、Reverse link、操作按钮和存储的 Markdown 原文放在卡片下方。parent/direct/side 移动交给地图和方向键。
- **写作台**：左侧是 Markdown 原文，右侧是实时预览；输入 `[[` 可补全 Link，
  `Ctrl+S` 保存，`Esc` 取消。

命令栏使用与 Session 相同的命令（`go 1/2`、`n`、`b`、`e`、`t 标题`、`l`、`ws agent`、`del`、`mv 1/3`、
`lsbk`、`help`）。删除、移动和修改 Citation key 时需要输入与 CLI 相同的确认文本。快捷键：
`:` 命令栏、`/` 搜索、`g` 前往、`n` `b` `e` `m`、`Delete`、`u` 或方向键在地图中移动、
`v` 切换视图、`w` 切换 Workspace、`f` 适配窗口。

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
| `search_cards`、`list_links`、`list_broken_links` | 全文搜索、Link 图（可只看跨 Workspace 的 Link）、Broken link |
| `create_topic`、`create_literature` | 在 agent Workspace 中新建 Topic 树或 Literature 树 |
| `create_successor` | 为 agent Card 添加 direct 或 side successor |
| `edit_card`、`edit_literature_metadata` | 修改 agent Card 的标题、正文或 BibTeX |
| `delete_card`、`move_card` | 删除或移动 agent Card |

删除、移动和修改 Citation key 会先返回一份计划；用计划中给出的 `confirm` 文本（`delete`、树根地址或
`move`）再次调用才会真正执行。使用前需要先启动服务。

在本仓库中使用 Claude Code 时，已提交的 `.mcp.json` 已经注册了它。在其他地方只需注册一次：

```sh
claude mcp add zt -- zt mcp
```

其他 MCP 客户端也可以用同样的方式把 `zt mcp` 作为 stdio 服务器启动。

## 导出

`zt dp` 会在 `<archive_root>/dump/` 下写出一个压缩归档，完整记录整个卡片系统：

- `mapping.json`：每个 Card 地址到其 Markdown 文件的映射。
- 每张 Card 一个 Markdown 文件，内容是其存储的原文；Literature Card 的文件以包含 BibTeX 的 YAML front matter 开头，
  `<citation-key>.bib` 保存 BibTeX 的原始字节。
- `archive.json`：每个 Card 地址所属的 Workspace，以及下一个 Topic 编号（因此已删除的 Topic 编号不会被重用），例如
  `{"next_topic_id":3,"workspaces":{"0/0":"human","1/0":"agent"}}`。

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
zt stats
zt ws
zt mcp
zt lsbk
zt dp
zt down
```
