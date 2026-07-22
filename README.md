# ZT

English | [中文](README.zh-CN.md)

ZT is a local, CLI-first note system built around addressable Cards and terminal navigation.

## Features

- Organize notes as Topics, Regular Cards, and BibTeX-backed Literature Cards.
- Navigate Cards by Location or Citation key in an interactive terminal Session.
- Connect Cards with `[[target]]` Links, with generated Reverse links and Broken link checks.
- Safely create, edit, move, and delete Card trees from the Session or shell.
- Store everything locally in SQLite, with status, statistics, dump, and clear commands.

## Install

Install the Rust toolchain, clone this repository, and run:

```sh
cargo install --path . --locked
```

## Quick start

Choose a directory for ZT's local data, start the service, and open a Session:

```sh
zt config set archive_root "<data-directory>"
zt up
zt
```

Inside the Session, commands are entered without the `zt` prefix:

```text
t My first topic   # Create a Topic
n                  # Create its Direct successor
b                  # Create a Side successor
l                  # Create a Literature Card
e                  # Edit the current Card
go <target>        # Go to a Location or Citation key
ls                 # List Cards in the current Topic
help               # Show all Session commands
q                  # Quit the Session
```

## Create a Literature Card

Run `l` inside a Session, or run `zt l` from the shell (using `$EDITOR`). Enter and save one complete BibTeX entry, then enter and save the Card body. The BibTeX entry key becomes the Citation key, and its `title` field becomes the Card title.

```bibtex
@book{Smith2024,
  title = {Example Book}
}
```

Inside a Session, press `Ctrl+S` to save each stage or `Esc` to cancel. You can later open this Card with `go Smith2024` or link to it with `[[Smith2024]]`.

## Card addresses

ZT does not use arbitrary IDs for Topic and Regular Cards. It automatically assigns each one a **Location**:

- A Topic uses `<topic-number>/0`; a fresh archive starts with `0/0`, then `1/0`, and so on. Normal deletion does not reuse Topic numbers.
- `n` creates a numbered Direct successor: `0/0` -> `0/1` -> `0/2`.
- `b` creates the next lettered Side successor of a Regular Card: `0/2|a`, `0/2|b`, ... `0/2|z`, `0/2|aa`.
- A Direct successor after a Side successor starts a new numbered segment: `0/2|a` -> `0/2|a|1`.
- A Literature Card has no Location. Its BibTeX entry key is its Citation key.

Useful shell commands:

```sh
zt help
zt status
zt stats
zt lsbk
zt dp
zt down
```
