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
e                  # Edit the current Card
go <target>        # Go to a Location or Citation key
ls                 # List Cards in the current Topic
help               # Show all Session commands
q                  # Quit the Session
```

Useful shell commands:

```sh
zt help
zt status
zt stats
zt lsbk
zt dp
zt down
```
