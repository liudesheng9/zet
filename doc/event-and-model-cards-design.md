# Event and Model Cards Design

Status: implemented in zt 0.5.0.

## Source Authority

- `CONTEXT.md` defines the project language, including **Card type**, **Idea Card**, **Event Card**, and **Model Card**.
- Earlier design documents remain authoritative; this document adds a Card type to every Card.

## Goals

1. Most Cards hold one idea. Two kinds of Card hold something else and should be recognisable as such by people and agents:
   - an **Event Card** records one event that happened in the real world (what, when, where, who, and the source that reports it), not an idea or an argument;
   - a **Model Card** presents one whole model (its question, setup, assumptions, timing and information, solution, results, intuition, and limits) so that the Cards discussing the model can rely on one complete statement of it.
2. Idea Cards cite these Cards with ordinary Links: an idea that uses an event as evidence or illustration links to its Event Card; an idea that discusses, extends, or criticises a model links to its Model Card. The Model Card's Reverse links then list every Card that discusses the model.
3. The type is data, not formatting: it is stored, shown, searchable, and survives dumps.

## Locked Model

- Every Card has exactly one **Card type**: `idea`, `event`, or `model`. `idea` is the default.
- Only Regular Cards may be `event` or `model`. Topic and Literature Cards are always `idea`; setting another type on a Tree root is refused: ``only Regular Cards can be Event or Model Cards``.
- Storage: `card_type TEXT NOT NULL DEFAULT 'idea' CHECK(card_type IN ('idea', 'event', 'model'))` on `cards`. Opening an archive without the column adds it with `ALTER TABLE`; every existing Card becomes an Idea Card. The Tree-root rule is enforced in code.
- The type belongs to the Card, not to its Location: moves, Side-successor compaction, and Citation key renames keep it. Deleting a Card deletes its type with it.
- The type does not change Card text, Links, Reverse links, Locations, or the successor rules. An Event or Model Card can have Direct and Side successors like any Regular Card, and any Card can link to it.
- Changing a type is an edit of that Card and follows the same Access rules: agent Access may change only agent Cards.

## Card Conventions

The system stores only the type; these body conventions are taught to people and agents (`skills/zettelkasten`) and are not validated.

- **Event Card.** Title: the event as one sentence with its date (`1926 年通用汽车收购费雪车身`). Body: what happened; when and where; who acted; the source (`来源：[[Key]] §2.3`, or a dated public record); then, in a separate final paragraph, which ideas it is evidence for, with Links. No interpretation in the description itself.
- **Model Card.** Title: the model's name and what it shows (`GHM 产权模型：资产所有权决定事前专用性投资激励`). Body sections, in order: question; setup (agents, assets, actions); assumptions; timing and information; solution or equilibrium; main results; intuition; limits; source. Formulas in `$...$`. Each later Card that derives, interprets, extends, or criticises the model links to it.

## Interfaces

- **MCP.** `create_successor` takes an optional `type` (`idea`, `event`, `model`; default `idea`); `edit_card` takes an optional `type`; `get_card`, `get_tree` Cards, and `search_cards` results report `type`; `search_cards` takes an optional `type` filter, and with a `type` the `query` may be empty to list every Card of that type.
- **GUI.** Event and Model Cards carry an EVENT or MODEL stamp on the card face, in the card view, on desk stacks, and in the Folgezettel map; the editor of a Regular Card has a type selector.
- **Session.** The card view shows `[event]` or `[model]` after the Location of a non-Idea Card; `ty <idea|event|model>` sets the type of the current Regular Card.
- **Dump.** `archive.json` gains a third key, `types`, after `workspaces`: `{"next_topic_id":…,"workspaces":{…},"types":{"<address>":"event"|"model",…}}`, listing only non-Idea Cards in address order (`{}` when there are none). Markdown files, hashes, `mapping.json`, and `.bib` files are unchanged.
