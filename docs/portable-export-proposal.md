# Portable export of one tree

Status: built (2026-11-03): PRD #257, slices #258–#262. The owner settled all four decisions O1–O4.

**Revised by the owner on 2026-11-03, after seeing the first build: D3 is replaced.** An export shows
the same GUI as the running front end, not a separate thin reader, and a function it cannot do says
so when it is clicked. Where this document still says "viewer", it means that GUI. The two-file form,
the file picker, one tree, the safety rules and O2–O4 all stand.
Target release: zt 0.56.0. It builds on `zt dp` (`src/dump.rs`) and the GUI renderer
(`render_markdown` in `src/gui.rs`).

## The problem

ZET has one way to take Cards out of the archive: `zt dp`. A dump always records the whole box, as
one Markdown file per Card plus manifests, inside a compressed archive. It is a backup. Nobody can
read it without unpacking it and without a running `zt`.

The GUI is the only reader of a Card as it should look (math, figures, charts, Link chips). It needs
the service, the SQLite archive and the Scholar Archive bridge, so it cannot leave this machine.

There is no way to give one book to another person, or to read it on another computer, as a thing
that opens by itself.

## What this proposes

A new command writes one tree into a folder that holds exactly two files:

```text
zt export <tree> [--out <folder>]

grossman2002integration-1793700000/
  cards.json    all the data of this one tree: Card data and rendered HTML
  index.html    the GUI itself, with no Card data in it
```

- **One tree, never the box.** `<tree>` is a tree id (Topic number or Citation key) or any address
  in the tree, the same reading as the MCP `get_tree`. Topic trees and Literature trees of both
  Workspaces work. A bare `zt export` is a usage error, so no form of the command exports the box.
- **Two files, nothing else.** No artifact folder, no separate script, stylesheet or font, no zip.
  Figures, charts and math travel inside these two files.
- **No dependency after export.** The folder needs no `zt`, no service, no Scholar Archive and no
  network.
- **`zt dp` does not change.** It stays the full backup. An export is for reading: it restores
  nothing and `zt` never loads it back.

## D1. `cards.json` holds the data and the HTML

The file is a plain JSON document. Rendering happens once, at export time, through the same
`render_markdown` the GUI uses, so an exported Card reads as it reads in the GUI and the viewer holds
no Markdown parser.

```json
{
  "format": "zt-portable-export",
  "version": 1,
  "tree": {"id": "grossman2002integration", "root": "grossman2002integration",
           "kind": "literature", "workspace": "agent", "title": "Integration versus Outsourcing…"},
  "exported": {"at": "2026-11-03T12:00:00Z", "zt": "0.56.0", "archive_revision": 36216},
  "cards": [
    {"address": "grossman2002integration/2|a", "parent": "grossman2002integration/2",
     "edge": "side", "title": "…", "type": "model", "facet": null, "emphasis": null,
     "level": "formal", "form": null, "term": null, "bibtex": null,
     "text": "the authored Markdown", "html": "<p>the rendered body</p>",
     "links": ["…"], "term_use_targets": ["…"]}
  ],
  "artifacts": {"<sha256>": {"type": "image/jpeg", "base64": "…"}},
  "charts": {"<id>": "<chart page>"},
  "outside": [{"address": "hart1995firms/15|c", "title": "…", "from": ["…"]}]
}
```

- `cards` is in tree order, and `parent` and `edge` are stated. The viewer does no address reasoning.
- `text` is the authored Card text. The generated Reverse section and `## Term uses` are left out:
  they are 5.3 MB of atkinson2015lectures alone, and they name Cards of other trees. The viewer
  derives "Referred by" and "Term uses" from `links` and `term_use_targets` inside the tree.
- `artifacts` holds each figure once, even when several Cards show it.
- `outside` lists every Link target that is not in the tree, so a reader can see what the export
  leaves out.
- Left out on purpose: `archive_root`, the GUI token, row ids, artifact source records, Lock facts,
  Spreads, and any path of this machine.

## D2. What the renderer does differently

`render_markdown` now emits server routes that do not exist in an export. A render target decides
four outputs, and every other rule stays one code path.

| Element | GUI output | Portable output |
|---|---|---|
| Link or Term link inside the tree | chip or quiet link to `#/card/…` | the same route, with `href` and `title` only, to save size |
| Link or Term link outside the tree | live chip | inert chip: address and title, or the Term label with its meaning, marked "not in this export" |
| Artifact | `<img src="/artifact/<sha>">` | `<img data-artifact="<sha>">`, filled by the viewer from `artifacts` |
| Document link | link to `/scholar/file/<id>` | its label as plain text; the document does not travel |
| HTML chart | iframe `data-chart` over `/chart/<id>` | the same `data-chart`, with the page in `charts` |

`render_markdown` keeps its signature for the GUI and its tests. It calls one inner function with
the GUI target, and export calls that function with the portable target and the tree.

An Artifact the archive does not hold is not a failure. The figure shows "figure not in this export"
and the CLI reports the count. Today no tree has one.

## D3. `index.html` is the GUI (revised)

The first build was a separate thin reader. The owner rejected that: an export must show the same
interface as the front end. So `index.html` is the GUI's own page with every asset inlined, and
`app.js`, `app.css` and every view run unchanged.

The page has one way out, `api()`. A small layer loads before `app.js` and answers the page's
`/api/*` requests from `cards.json`, in the service's own shapes. Five hooks in `app.js` cover what
is not a request: when to start, the chart frame, the large figure, a chip outside the tree, and the
notice.

| Runs as on the service | Says "… is not available in the portable version." |
|---|---|
| Map, Table and Spreads, Constellation | Writing, editing, moving, inserting, deleting |
| Reader, Model board, theory board, study desk | Reply, filing a hub |
| Search, history, reading modes, command bar, keys, themes | Ask the box |
| Math, figures, HTML charts, Term tips | Tree Locks, Bridge reports |
| Broken links list | The Scholar Archive |

The buttons stay where they are, so the interface is the same, and each says why when it is reached.
The notice comes before any question the function would ask. A Spread is kept in that browser. A
Link or a Model neighbour outside the tree says the Card is not in this export.

Search has no register in an export, so it finds by title and text; both of the tray's orders read
that one list. The distant Card's slot stays empty, because it would name another tree.

`index.html` is 1.4 MB: the GUI's 0.7 MB and KaTeX. It is no longer thin, and that is the price of
one interface and one code path. The layer itself is 24 KB.

## D4. The browser limit that shapes the design

A page opened from disk cannot read the file next to it. I tested this on 2026-11-03 with a probe
page and a sibling `cards.json`, opened as `file://` in headless Edge and Chrome: both `fetch` and
`XMLHttpRequest` failed in both browsers. Firefox and Safari were not tested.

So the viewer has two ways in:

1. It tries `fetch('cards.json')`, which works when the folder is served over HTTP.
2. When that fails, it shows one quiet slip: "Open cards.json", with a file button and a drop area.
   The reader chooses the file once per opening, and the viewer reads it in memory.

This keeps the two-file form and plain JSON. The cost is one extra step on every double-click, which
the owner accepted (decision O1).

A fixed viewer also gives something back: one `index.html` can open any export.

## D5. Safety of a file someone else made

The viewer may open a `cards.json` it did not come with, and that file holds HTML. Two probes on
2026-11-03 in headless Edge settled the design:

| Page policy | Viewer script | Chart in a sandboxed `srcdoc` frame | Handler injected through `innerHTML` |
|---|---|---|---|
| `script-src` with the viewer's hash | runs | blocked (a `srcdoc` frame inherits the page's policy) | blocked |
| `script-src 'unsafe-inline'` | runs | runs | runs |

A hash policy would kill every chart, so the policy alone cannot protect the page. The viewer
therefore does two things:

- A `<meta>` Content-Security-Policy with no network: `default-src 'none'; script-src 'unsafe-inline';
  style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'self'; frame-src 'self'
  about:`. A hostile file cannot call out.
- Before any Card HTML enters the page, the viewer parses it with `DOMParser` and keeps only the tags
  and attributes the renderer emits. It removes every `on*` attribute, every `<script>`, and every
  `href` that is not `#/card/…`.

Charts run in `<iframe sandbox="allow-scripts" srcdoc>`, so they get an opaque origin and cannot
reach the page. The viewer fills in the theme, `chart-frame.js` and KaTeX when it opens a chart, as
`/chart/<id>` does now, so a chart page in `cards.json` carries no copy of the fonts. No chart in the
archive names an `https:` script, image or link in `src` or `href`, so the no-network policy breaks
none of them.

A file on disk has no response header to carry a sandbox, so the sandbox is the frame's attribute.
The wiki's decision "Charts are sandboxed by response header" records that the desktop browser pane
once refused such frames. On 2026-11-03 it drew them, so an export shows its charts there too.

The real books changed one rule of this section. A Card can hold a bare URL (four in
atkinson2015lectures and acemoglu2007equilibrium together). The filter takes its route and keeps its
text, so the address stays readable and nothing leaves the page. The whole-book check fails if a web
link has any other label, because that would lose the address.

## D6. How the command runs

- `require_service_up`, like every other command. Then one `Snapshot::read`, so all Cards belong to
  one archive revision. It takes no edit lock, as `zt dp` does since #253.
- The default folder is `<archive_root>/export/<tree>-<seconds>/`. `--out <folder>` names another
  one, and a folder that exists and is not empty is refused.
- It writes to a staging folder and renames at the end, so a failure leaves no half export. This is
  the `DumpArtifacts` pattern.
- Success line: `exported 321 cards of grossman2002integration to <folder> (3.1 MB)`, plus one line
  each when there are outside Links or missing figures.
- First release is CLI only. No MCP tool and no GUI button.

New code: `src/export.rs` (collect, write), the render target in `src/gui.rs`, and
`src/gui/portable.html` with its script. `cmd_export` joins the dispatch in `src/main.rs`.

## Sizes

Measured on 2026-11-03 on a consistent copy of the live archive. `index.html` is 1.4 MB in every
export. "Links out of tree" counts occurrences; `outside` lists their distinct targets.

| Tree | Cards | Authored text | Links | Links out of tree | Charts | Figures | `cards.json` |
|---|---:|---:|---:|---:|---:|---:|---:|
| grossman2002integration | 321 | 0.9 MB | 3,035 | 0 | 0 | 8 (75 KB) | 3.4 MB |
| hart1995firms | 1,095 | 1.8 MB | 6,501 | 33 | 10 | 14 (379 KB) | 6.8 MB |
| acemoglu2007equilibrium | 353 | 1.0 MB | 3,372 | 93 | 2 | 0 | 3.6 MB |
| atkinson2015lectures | 7,440 | 17.0 MB | 56,429 | 99 | 70 | 79 (2.8 MB) | 62.9 MB |

A typical book is a few megabytes. atkinson2015lectures is the stress case: it opens in about 0.5 s, a move
takes about 65 ms (at most about 270 ms), and all 69 of its chart Cards draw. Links are what cost the most, so the
portable Link form in D2 carries as little as it can.

## Non-goals

- Restoring or importing an export. `zt dp` is the backup.
- Writing anything back. A function that would write says it is not available.
- Exporting part of a tree, several trees, or a tree with the Cards its Links reach. A block a Model
  imports from another tree shows as an outside Link, not as a copy.
- Carrying source documents. A Document link becomes its label.
- Lock evidence. That remains the dump's job.

## Decisions by the owner (2026-11-03)

**O1. How does the double-click reach the data?** (D4) Decided by the owner on 2026-11-03: the file
picker. The folder stays `cards.json` and `index.html`, and the reader chooses `cards.json` on each
opening from disk.

| Option | Opens by | Files | Cost |
|---|---|---|---|
| **A. Picker** (chosen) | double-click, then choose `cards.json` | `cards.json`, `index.html` | One extra step on each opening |
| B. Script wrapper (rejected) | double-click, nothing else | `cards.js`, `index.html` | The data file is no longer JSON (`window.ZT_CARDS = {…}`), so other tools must strip the prefix |
| C. One file (rejected) | double-click, nothing else | `<tree>.html` with the JSON inside | Not the two-file form; each export carries its own viewer |

**O2. Does the JSON keep the stored Markdown as well as the HTML?** Decided: keep it. It is the real
Card data, other tools can read it, and search runs on it. The cost is about a third of the file.

**O3. Does an outside Link show the other Card's title?** Decided: yes, address and title, inert,
because the sentence around it often needs the title. An exported tree therefore names the titles of
the Cards it Links to, in its inert chips and in `outside`.

**O4. Do HTML charts go in the first release?** Decided: yes. atkinson2015lectures has 70 and
hart1995firms 10, and without them a Card loses content. They are the most expensive part of the
viewer (D5), and that cost is accepted.

## Build order

1. **Data.** `zt export`, the render target, `cards.json`, staging, tests on the JSON and on what
   never appears in it (generated sections, paths, other trees' Cards).
2. **Viewer.** Load path, outline, reader, Links, math, outside chips, back and forward.
3. **Figures, charts, search, theme, the safety pass of D5.**
4. **Release.** `README.md`, `CONTEXT.md` ("Portable export"), the wiki blocks `service-and-cli` and
   `gui`, zt 0.56.0.

## How we will know it works

- Every file in the folder is `cards.json` or `index.html`. Nothing else, on every tree.
- Export of a tree holds exactly that tree's Cards. A test seeds two trees and finds no Card, title
  in `cards`, or generated line of the other tree, except in `outside` and inert chips.
- `index.html` is byte-identical across exports of different trees.
- Two exports at the same revision differ only in `exported.at`.
- With the network off and the service down, a real Edge and a real Chrome open grossman2002integration
  by double-click: a math Card, a figure, a Term link hover and a search all work.
- acemoglu2007equilibrium shows its 93 outside Links as inert chips, and none of them navigates.
- atkinson2015lectures loads and moves between Cards without a lock-up, and a chart Card draws.
- A hand-made `cards.json` with `<img onerror>` and a `javascript:` link runs nothing.
- The GUI's `render_markdown` tests pass unchanged.
