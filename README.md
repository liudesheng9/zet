# ZT

English | [中文](README.zh-CN.md)

ZT is a local, CLI-first note system built around addressable Cards and terminal navigation.

## Features

- Organize notes as Topic trees and Literature trees: each Topic and each BibTeX-backed Literature Card is the root of its own tree of Regular Cards.
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
n                  # Create the Direct successor (also works on a Literature Card)
b                  # Create a Side successor
l                  # Create a Literature Card
e                  # Edit the current Card
go <target>        # Go to a Location or Citation key
up                 # Go to the parent Card (a tree root goes to ROOT)
ls                 # List the current tree, or Topics and Literature at ROOT
help               # Show the commands available here
q                  # Quit the Session
```

The ROOT view lists every Topic and Literature Card. Every Card view ends with
clickable `parent:`, `direct:`, and `side:` links. `ls`, `lsbk`, and `help` open a
panel whose addresses you can click; press `Esc` to close it. Long views scroll
with `PageUp`/`PageDown` or the mouse wheel.

## Create a Literature Card

Run `l` inside a Session, or run `zt l` from the shell (using `$EDITOR`). Enter and save one complete BibTeX entry, then enter and save the Card body. The BibTeX entry key becomes the Citation key, and its `title` field becomes the Card title.

```bibtex
@book{Smith2024,
  title = {Example Book}
}
```

Inside a Session, press `Ctrl+S` to save each stage or `Esc` to cancel. You can later open this Card with `go Smith2024` or link to it with `[[Smith2024]]`.

## Literature trees

Each Literature Card is the root of its own tree for notes about that work. The
Literature Card keeps its BibTeX metadata; its tree Cards are Regular Cards:

```text
go Smith2024   # Open the Literature Card
n              # Create Smith2024/1
n              # From Smith2024/1, create Smith2024/2
b              # From Smith2024/2, create Smith2024/2|a
```

Literature tree Cards follow the same successor, link, move, and delete rules as
Topic tree Cards, with one boundary: they can move only inside their own
Literature tree, and Topic tree Cards cannot move into a Literature tree.
Renaming the Citation key through `e` → metadata renames the whole tree and
rewrites every Link to it. Deleting a Literature Card deletes its whole tree and
asks you to type its Citation key, just as deleting a Topic asks for its
Location.

## Card addresses

ZT does not use arbitrary IDs for Topic and Regular Cards. It automatically assigns each one a **Location**:

- A Topic uses `<topic-number>/0`; a fresh archive starts with `0/0`, then `1/0`, and so on. Normal deletion does not reuse Topic numbers.
- `n` creates a numbered Direct successor: `0/0` -> `0/1` -> `0/2`.
- `b` creates the next lettered Side successor of a Regular Card: `0/2|a`, `0/2|b`, ... `0/2|z`, `0/2|aa`.
- A Direct successor after a Side successor starts a new numbered segment: `0/2|a` -> `0/2|a|1`.
- A Literature Card has no Location. Its BibTeX entry key is its Citation key.
- Cards in a Literature tree use the Citation key in place of the Topic number: `Smith2024/1`, `Smith2024/1|a`.

Useful shell commands:

```sh
zt help
zt status
zt stats
zt lsbk
zt dp
zt down
```
