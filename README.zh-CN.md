# ZT

[English](README.md) | 中文

ZT 是一个本地优先、以命令行为核心的卡片笔记系统，支持可寻址的 Card 和终端导航。

## 功能

- 使用 Topic 树和 Literature 树组织笔记：每个 Topic 与每个基于 BibTeX 的 Literature Card 都是一棵独立 Regular Card 树的根。
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
go <目标>          # 前往指定 Location 或 Citation key
up                 # 前往父 Card（树根会回到 ROOT）
ls                 # 列出当前树；在 ROOT 列出 Topic 与 Literature
help               # 显示当前位置可用的命令
q                  # 退出 Session
```

ROOT 视图会列出所有 Topic 和 Literature Card。每个 Card 视图末尾都有可点击的
`parent:`、`direct:`、`side:` 链接。`ls`、`lsbk`、`help` 会打开可点击地址的结果面板，
按 `Esc` 关闭。内容过长时可用 `PageUp`/`PageDown` 或鼠标滚轮滚动。

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
zt status
zt stats
zt lsbk
zt dp
zt down
```
