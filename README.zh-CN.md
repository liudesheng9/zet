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
e                  # 编辑当前 Card
go <目标>          # 前往指定 Location 或 Citation key
ls                 # 列出当前 Topic 中的 Card
help               # 显示所有 Session 命令
q                  # 退出 Session
```

常用 Shell 命令：

```sh
zt help
zt status
zt stats
zt lsbk
zt dp
zt down
```
