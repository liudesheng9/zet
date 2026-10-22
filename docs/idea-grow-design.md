# zt-idea-grow: growing Topic trees from a Literature tree

Status: design, 2026-10-20. Requested by the user: "create a ZT idea tree grow workflow. It takes the current card box structure and extracts the useful ideas, core ideas and related ideas, and any idea that is useful, interesting, intriguing or intuitive, from the Literature card tree itself, and grows the Topic trees. Follow Niklas Luhmann's best practice and the earlier proposals. Do not add the proposals to the digest workflow; add them to the idea tree grow workflow."

The unit is the **whole box**, not one book (user, 2026-10-20: "not locked on one single literature"). By default the run reads every Literature tree. The more books the box holds, the more each idea can relate to, and one Topic gathers a concept from several sources. The digests (`zt-econ-digest`, `zt-math-digest`) build a book's **Literature tree**: the index, the anchors, the book-bound notes, the models, the events and the findings. They stay as they are. `zt-idea-grow` is the second act. It reads a finished Literature tree and the whole box, then writes the reader's own ideas into the **Topic trees**, filed beside the Cards they continue.

## Luhmann's two acts

Luhmann kept two boxes ([topic-tree-ideas-proposals.md](topic-tree-ideas-proposals.md)):

- The **bibliographic box** holds the source and its notes. In ZT this is the Literature tree.
- The **main box** holds his own reformulated ideas. In ZT these are the Topic trees.

He did the two acts apart. While reading, he wrote the bibliographic notes. Later he asked how each note bore on what the main box already held, and he wrote only what was worth keeping. Each kept idea became a new Card in his own words: one idea, readable without the source, filed behind the Card it continues (57/12 → 57/12a), and linked to distant Cards. In this workflow:

| Luhmann | zt-idea-grow |
| --- | --- |
| Read the bibliographic notes again | `plan` gives each job one section of the Literature tree: an anchor and its subtree, with the Model Cards, the Findings and the Events |
| Ask how a note bears on the box | Each job also gets the Topic catalogue, the Topic Cards that already link this section, and the existing Topic Cards closest to the section by wording; DeepSeek can `search_cards` read-only |
| Keep only what is worth keeping | Each kept idea has a kind: 核心 (core), 关联 (related to Cards already in the box), 有用 (useful), 有趣 (interesting or intriguing), 直觉 (an intuition) |
| Write it in your own words, standalone | Title as a claim, with no book-local numbers (R20). The body carries its own conditions (P4) and cites its source notes with 例示/对应/对照, never 依赖 into its own book (I7, P1) |
| File it behind the Card it continues | `at`: an existing agent Topic Card (Direct successor = the next step, Side successor = a branch or objection, P3/P6), an earlier idea of the same answer (a chain), or a new Topic (P2) |
| Link distant Cards | At least one Link besides the parent and the source is asked for; a Link across trees is the valuable one |
| Hub and structure Cards | The hubs round (2.5.0): topic, line and field hubs through the hub compiler, and the register's terms pointing at the Cards that carry them |

## The proposals, and where they live

The proposals of `docs/topic-tree-ideas-proposals.md` belong to this workflow, not to the digests:

| Proposal | In zt-idea-grow |
| --- | --- |
| P1 two boxes | The prompt files by Luhmann's two boxes. A general claim that rests on the book's model becomes a Topic Card, which cites the Literature note with 例示 or 对应. The Literature note itself stays where it is |
| P2 new Topics | `create_topic` is allowed when the box has no Topic for the concept. The helper's topic gate refuses a near-duplicate name, and `apply` then files the Card under the existing Topic |
| P3 continue the box | The job lists the existing Topic Cards that already link the section, and those closest to it by wording. An idea may continue any agent Topic Card, as a Direct or a Side successor |
| P4 I7 fix order | The prompt and the fix round order the fixes: write the assumption in, link the book with a real relation, and never move the Card to the Literature tree |
| P5 Topic share | The report counts the agent Topic Cards that link the book against the book's Literature-tree Idea Cards, before and after |
| P6 Folgezettel | A next step is a Direct successor, a branch or objection a Side successor. Ideas of one answer can chain (`at` names an earlier idea) |
| P7 dedupe | Each title is compared with every title in the box (Model parts included) and with the other ideas of the answer. It must not repeat its source note's title (own words). A same-claim idea is `same`, never a new Card |

## Stages

```
args (gate) → grow (zt-grow-cycle) → report (value)
```

1.3.0 adds a link round after the sections are written: one job per Topic tree the run wrote in. It proposes Links to Cards in other trees, at most 2 per Card, each one sentence with its kind (G11). 1.1.0 added a first round inside `grow`. One DeepSeek job (gt-001) plans the run's Topics over the whole book, and `-Step topics` makes them before the section jobs are written. In pilot 1, parallel sections each opened a Topic for one concept.

`grow` is one compiler cycle (`zt-digest grow`, Rust):

1. **plan** (`-Book <key> -Work <out>`) reads the Literature tree (either Workspace) and the whole box's titles, and writes:
   - `snapshot.md`: every Card's address and title (the `snapshot-topics` rendering);
   - `topics.md`: the Topic catalogue (address, title, first line);
   - `sections/sNNN.md`: one anchor and its subtree (type, facet, title, body cut to 1200 characters), the Topic Cards that already link each note (parsed from the generated Reverse links), and the closest existing Topic Cards;
   - `jobs/gw-NNN.md` (one per section, or per `-PerJob` sections);
   - `baseline.json`, the Topic share before the run.

   The printed line is short: `{"jobs":N,...}`. The module builds the job ids itself (`gw-001`…), so no list passes through a relay.
2. **DeepSeek** (official source, high effort, skills `zettelkasten` and `asd-ste100-zh`) writes `out/gw-NNN.json` and runs its SELF-CHECK, `zt-digest grow -Step precheck -Work <out> -Job gw-NNN`, until `faults: 0`.
3. **apply** reads every answered job not yet applied, in job order. It refuses each idea with a fault, makes the new Topics through the topic gate (a near-name refusal files the Cards under the existing Topic), and drops a run-level duplicate (a title near one this run already wrote). It creates each Card at its parent (a Direct successor whose slot is taken becomes a Side successor; a Topic root as parent gives the doorway `n/1`, or a new line beside it, see "Lines beside the doorway"), resolves `@` references to earlier ideas, appends the 来源 line when it is missing, and records `state.json`. A `same` idea appends one sentence to the agent Card (with `base`) or, for a human Card, only records it.
4. **check** runs zt-check (`--addresses`) on every Card the run wrote. Each hard fault becomes a fix job, `jobs/fxR-NNN.md`: the Card's text, its faults and the fix order. The answer `out/fxR-NNN.json` holds `{"edits":[{at,title,body}]}`, which `apply` writes with `base`. There are at most `fixRounds` (default 1) rounds.
5. **report** writes `grow-report.json` and `grow-report.md`: ideas per kind and section, the Cards and Topics made, `same`, refused (with reasons), duplicates, remaining faults, and the Topic share before and after. It prints a short line.
6. The module deletes the DeepSeek conversations and removes the workspace.

## The answer

```json
{"section": "Key/3",
 "topics": [{"id": "T1", "title": "敲竹杠", "body": "本主题追问：……"}],
 "ideas": [
  {"id": "I1", "kind": "核心", "title": "<one claim, own words>", "body": "<80–250 字，含成立条件；例示 [[Key/3|a]]：……>",
   "sources": ["Key/3|a"], "at": "T1"},
  {"id": "I2", "kind": "直觉", "title": "…", "body": "…", "sources": ["Key/3|b"], "at": "I1", "as": "direct"},
  {"id": "I3", "kind": "关联", "title": "…", "body": "<第一句写出与 [[12/4]] 的关系>", "sources": ["Key/3|a"], "at": "12/4", "as": "side"},
  {"id": "I4", "kind": "核心", "same": "12/7", "body": "<一句：例示 [[Key/3|c]]：本书……>", "sources": ["Key/3|c"]}
 ],
 "none": "<only when the section holds no idea worth keeping: why>"}
```

## Checks (precheck and apply)

| Code | Fault |
| --- | --- |
| G1 | The answer is not JSON, has no `ideas` (and no `none`), more than 12 ideas, a duplicate id, or an unknown `kind` |
| G2 | A `sources` entry is not a Card of this section |
| G3 | The body does not cite one of its sources with a real relation (例示/案例/印证/对应/对照/放宽/反驳) |
| G4 | The body rests on its own book (`依赖`/`基于 [[Key…]]`): write the assumption in (P4) |
| G5 | `at` is not an agent Topic Card, an earlier idea, or a declared new Topic; a human Card or a Literature-tree Card cannot be a parent |
| G6 | An idea continues an existing Card but its first sentence does not link `[[at]]` with the relation |
| G7 | The title is near its source note's title (own words), near another Card in the box (use `same`, or state the difference), or near another idea of the answer |
| G8 | A new Topic's name is near an agent Topic: file under it |
| G9 | A `same` target does not exist |
| G10 | A Link outside the source line reads as a plain reference: zt's own kind table (a port of `RELATION_KINDS`) finds no kind word before it |
| G11 | A link-round Link that does not start at a new Card of its job, ends in its own tree or at a root, is not the sentence's only Link, reads as a plain reference, or is the third Link from one Card |
| G12 | The destination is Locked by the owner: `at` or `same` names a Card of a Locked tree, a new Topic's name would be filed in a Locked Topic, or `existing` names a Locked Topic root. The answer names another destination: the tool never substitutes a tree or a placement |
| G13 | A second idea of one answer at the same Topic root (`at` the same `n/0` or `T1`): a section opens one line in a Topic, so the idea chains from the first one (`"at": "I1"`) or continues a named Card of the Topic |
| R…, I1–I4 | zt-check on the drafts (category idea): sentence length, semicolons, book-local numbers (R20), claim titles |

## Lines beside the doorway

A Topic's doorway `n/1` is its way in, and each Side successor beside it (`n/1|a`, `n/1|b` …) opens one line of the Topic. The zettelkasten skill keeps about seven Side successors per Card. On the live run of 2026-10-22, Topic 412 got nine lines beside 412/1: section 1 placed three ideas at the root, and seven more sections opened one line each. Two rules keep the fan small:

- **One root placement per section and Topic (G13).** The precheck refuses a second idea at the same Topic root. The section's other ideas for that Topic chain from its first one, or continue a named Card of the Topic (with G6's first-sentence relation).
- **A full doorway sends the line down (`MAX_LINES` = 7).** When the doorway already has seven lines, `apply` files the next root placement as a Side successor of the doorway's last line (`n/1|g|a`). If that line has seven too, the same rule goes one level further down. The Card's receipt records `full` (the doorway), and `grow-report.md` lists each such Card under `## 入口卡已满` with its doorway, so the reader sees where a structure Card or a new Topic would help.

The alternative was to report a full doorway as advice and still open an eighth line. It was rejected because advice does not keep the fan small, and a long-lived Topic only grows. The cost of the rule is that a section's line can branch off a line of another section. The report names each case for that reason.

## Locked trees (S15)

The owner may lock one whole Topic tree: it takes no new Cards until the owner unlocks it in the GUI
(Tree Lock, `CONTEXT.md`). Idea-grow reads a Locked tree and never writes it:

- **A Locked source book is read normally.** A Lock can stop a digest writing mode; it is no reason to
  refuse this workflow. The plan reads a Locked Literature tree like any other and keeps planning its
  sections.
- **A Locked Topic is related context only.** The plan reads the Lock facts with the title export it
  already takes, keeps each Card's `lock` in `titles.json`, marks the tree in the topic catalogue
  (`已锁定：只读，可链接，不能写入`), and lists its Cards in the section file apart from the Cards an
  idea may continue (`用词最接近的已锁定主题卡`). A Locked Card stays a Link target: an idea may cite it.
- **The precheck names the conflict.** A Locked destination is G12: the idea or Topic is reported with
  the Lock id and the tree, and the answer must name another destination. Nothing is rewritten to
  another tree behind the answer's back.
- **The storage gate decides at the write.** A Topic locked after its answers stops the apply stage:
  the affected job keeps its answers and the receipts of the Cards it already made, the stage prints
  the typed `card_locked` refusal (`code`, `tree`, `lock_id`, `mode`, `step`, `at`) and exits 2, and no
  Card is placed elsewhere. `grow-report.md` lists the stopped writes. After the owner unlocks, the
  same `out` folder resumes the job; nothing is retried before that.

## Bounds

- At most 12 ideas per section; the prompt asks for about 3–10, chosen, not exhaustive.
- At most 120 jobs per run, with Haiku managers at 8 DeepSeek runs each and 8 managers at once.
- The workflow writes only agent Topic trees. It never edits a Literature-tree Card, never moves or deletes a Card, and never writes a human Card.

## The only Topic-side workflow (3.0.0)

The owner decided on 2026-10-22 that the digests build only Literature trees (`docs/literature-only-digest-design.md`). Every Topic-side compose is here: the Topic trees, the Topic Cards, the links across trees, the hubs and the register. 3.0.0 takes over what the digests did on that side: mode `hubs` (the hubs round and the register alone), the hub options (`hubTrees`, `hubRebuild`, `hubLimits`, `hubEffort`, `hubFixRounds`), and the restatement review of the run's new Topic Cards (`zt-digest restate`, one judge job per source book, fed by `grow -Step restate-cards`).

## Not in this version

- (Since 2.5.0 the hubs round builds the hubs and the register; see the wiki block doc.)
- (Since 3.1.0 the concept review of each source book's term table feeds the register plan's `-Review`, and the optional Topic maintenance round (`organize: true`) runs the old Organize's Topic merges and Card moves; see the wiki block doc.)
- Pairing across books: the snapshot and the closest-Cards list find related Cards from any book, but no Bridge review runs.
- A second pass that re-files whole branches (join, topic-home): `zt-digest join` and `topic-home` remain separate review tools.
