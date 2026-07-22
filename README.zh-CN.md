# ZT

[English](README.md) | 中文

ZT 是一个本地优先、以命令行为核心的卡片笔记系统，支持可寻址的 Card 和终端导航。

## 功能

- 使用 Topic、Regular Card 和基于 BibTeX 的 Literature Card 组织笔记。
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
n                  # 创建当前 Card 的 Direct successor
b                  # 创建 Side successor
l                  # 创建 Literature Card
e                  # 编辑当前 Card
go <目标>          # 前往指定 Location 或 Citation key
ls                 # 列出当前 Topic 中的 Card
help               # 显示所有 Session 命令
q                  # 退出 Session
```

## 创建 Literature Card

在 Session 中运行 `l`，或在 Shell 中使用 `$EDITOR` 运行 `zt l`。先输入并保存一个完整的 BibTeX 条目，再输入并保存 Card 正文。BibTeX 条目的 key 会成为 Citation key，其中的 `title` 字段会成为 Card 标题。

```bibtex
@book{Smith2024,
  title = {示例书籍}
}
```

在 Session 中，每个阶段按 `Ctrl+S` 保存，按 `Esc` 取消。创建后可以使用 `go Smith2024` 打开，也可以使用 `[[Smith2024]]` 创建 Link。

## Card 地址规则

ZT 不给 Topic 和 Regular Card 分配任意 ID，而是自动为它们分配 **Location**：

- Topic 使用 `<主题编号>/0`；新档案从 `0/0` 开始，然后是 `1/0`，依此类推。普通删除不会复用 Topic 编号。
- `n` 创建数字形式的 Direct successor：`0/0` -> `0/1` -> `0/2`。
- `b` 创建 Regular Card 的下一个字母形式 Side successor：`0/2|a`、`0/2|b`、...、`0/2|z`、`0/2|aa`。
- Side successor 后的 Direct successor 会增加新的数字段：`0/2|a` -> `0/2|a|1`。
- Literature Card 没有 Location；它使用 BibTeX 条目的 key 作为 Citation key。

常用 Shell 命令：

```sh
zt help
zt status
zt stats
zt lsbk
zt dp
zt down
```
