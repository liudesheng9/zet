# Finding Cards Design

Status: built in zt 0.20.0 and zt-check (2026-10-16). Tested on scratch archives only. No digest writes Finding Cards yet: the empirical stage of the economics digest comes later and uses this type. The live archive changes on the first `zt up` of 0.20.0 (a table rebuild after a backup copy, below).

## Source authority

- `CONTEXT.md` defines **Card type**, **Finding Card**, and **Card layer rules**.
- `event-and-model-cards-design.md` defines how a Card type is stored, shown, searched, and dumped. A Finding Card follows those rules. This document adds only what is different.
- `digest-card-layers-proposal.md` (D9, Q1) records why the type exists.

## Why a new type

The book `missingmarkets2026interviews` reports its results from 48 firm interviews. A result of a study is not an event: it has no single date and place, and it holds a measurement, not an occurrence. It is not a model: it states no mechanism. As an Idea Card it loses its data and its method, and an Idea Card that rests on it cannot say "this is evidence". Q1 of the card layers proposal asked: a new type `finding`, or an Idea Card with a fixed evidence template. The user chose the new type, so that a digest stage, a search, and the GUI can find every empirical result by its type.

## Locked model

- `finding` is a fifth Card type, beside `idea`, `event`, `model`, and `hub`.
- Only Regular Cards can be Finding Cards. Topic and Literature Cards stay Idea Cards. The refusal for a tree root is: ``only Regular Cards can be Event, Finding, Model, or Hub Cards``.
- A Finding Card has no facet, emphasis, level, or form. Those stay with Model and Hub Cards.
- The type belongs to the Card. A move, a Side-successor compaction, and a Citation key rename keep it.

## Storage and upgrade

- `card_type TEXT NOT NULL DEFAULT 'idea' CHECK(card_type IN ('idea', 'event', 'finding', 'model', 'hub'))`.
- No column is added. SQLite cannot change a CHECK, so `zt up` reads the stored table definition. If the definition does not name `'finding'`, ZT copies the archive to `zt.sqlite3.pre-finding-cards-<seconds>.bak` and rebuilds the cards table in one transaction (`ensure_finding_cards` in `src/main.rs`). Every Card keeps its row id, type, facet, emphasis, levels, form, and text. The log gets one line: `added finding cards; no card is a finding card yet`. The rebuild runs once.
- An archive older than Hub Cards is rebuilt by the hub step with today's table, so it skips this step and gets no second backup.

## Interfaces

- **MCP.** `create_successor`, `insert_card`, and `edit_card` take `type: "finding"`. `search_cards` takes `type: "finding"` and then needs no query. `get_card`, `get_tree`, and `search_cards` report the type. `list_trees` gives each tree a `findings` count beside `events` and `models`. The server instructions name the Finding Card and its template.
- **GUI.** A Finding Card carries a FINDING stamp in plum ink (`--finding`) on the card, in the map, and in search hits. Each desk stack counts its Finding Cards. The type selector and the search type chips list Finding.
- **Session.** The card view shows `[finding]`. `ty finding` sets the type.
- **Dump.** `archive.json` `types` lists Finding Cards as `"finding"`.

## Card template

The system stores only the type. The digest and the `zettelkasten` skill teach the template, and zt-check checks it (F rules, below). The Card language rules (ASD-STE100, adapted for Chinese) apply as on every Card.

- **Title**: the result as one claim, with its scope: `31 家受访企业因为缺少保险市场推迟了投资`.
- **Body**: six labelled lines, in this order, then one paragraph of Links.

| Label | What it holds |
| --- | --- |
| `测量` | What the study measured: the variable or the question. |
| `数据` | The sample, the period, and the place: `48 家企业的访谈，2024 年，美国`. |
| `方法` | The method: interviews, a survey, a regression, an experiment, a case comparison. |
| `结果` | One result, with its size or its direction: `48 家中 31 家推迟了投资`. |
| `局限` | What the result does not show: the sample, the self-report, the identification. |
| `来源` | The source line: `来源：[[Key/n]] §3.2`. |

- The last paragraph names the ideas the result is evidence for, each Link with a kind word: `这一结果印证 [[5/1]] 的判断：……`.
- **One result per Card.** A study with three results gives three Finding Cards, filed as Side successors of the anchor of the part that reports them. The Idea Card that a result supports links it with `印证`, `依赖`, or `例示`.
- A Finding Card states the result. It does not interpret it. The interpretation goes in the Idea Card that links it.

## zt-check rules (F rules)

`zt-check` reads a Finding Card as category `finding` (in `--cards` mode, from the Card's type). It checks the Card language rules R2 to R17, the shared rules I5, L1, L2, and L3, and these rules:

| Rule | Check | Default |
| --- | --- | --- |
| F1 | The body holds the labels `测量`, `数据`, `方法`, `结果`, `局限`, and `来源`, each at the start of a line (a list marker and bold are allowed). | yes |
| F2 | The `来源` line names its source. When the book is known, the line links its Literature Card or one of its anchors. With no book, the line is not empty. | yes |
| F3 | One result per Card: one `结果` label, and at most one list item under it (up to the next label or blank line). | yes |
| F4 | At least one Idea Card links the Finding Card with a kind word (from `inbound_kinded`, as E4). | no, as E4 |

- I3 does not apply to a Finding Card: F2 checks its source line, which is stricter.
- I1, I2, I4, and I6 do not apply. A Finding Card is sized by its template, and its Links out are the source and the ideas it supports.
- The default rule set of `zt-check --cards` adds F1, F2, and F3.
- The unit test `the_finding_rules_read_labels_the_source_line_and_one_result` in `tools/zt-check/src/check.rs` raises each rule once.

## Not built

- No digest stage writes Finding Cards yet. The economics digest's empirical stage will use this type.
- `digest-audit.ps1` does not yet put Finding Cards in category `finding`. Until it does, the F rules run through `zt-check --cards` and `--json` only.
- No duplicate check (an F5 like E5).
