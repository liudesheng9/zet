# Model Structure, Math, and HTML Charts Design

Status: implemented in zt 0.6.0.

## Source Authority

- `CONTEXT.md` defines **Model Card**, **Model part**, **Model facet**, and **HTML chart**.
- `event-and-model-cards-design.md` remains authoritative for Card types; this document adds the inner structure of a model and the GUI's rendering of math and charts.

## Goals

1. A whole model rarely fits one Card. A model is presented by its **Model Card** and by **Model parts**, one per facet it needs, so that each part can state its facet in depth (the setting with a diagram, the timing as a timeline, the dynamics as an interactive plot) and every discussion can link the exact part it concerns.
2. The parts of a model are connected the same way everywhere: by placement (they are filed under the Model Card), by their stored facet, and by Links between them, so that a reader who lands on any part can see and walk the whole model.
3. The GUI renders the TeX that model Cards are full of, offline.
4. A Card can hold an HTML chart that illustrates the model, without giving agent-written HTML any reach into the GUI.

## Locked Model

- **Facet storage.** `model_facet TEXT CHECK(model_facet IS NULL OR model_facet IN ('setting','assumptions','timing','dynamics','solution','results','intuition','limits'))` on `cards`. Opening an older archive adds the column with `ALTER TABLE`; every existing Model Card becomes a Model Card without parts.
- **A facet requires the Model type.** Setting a facet makes a Card a Model Card with that facet; setting any other type clears it; naming the Model type alone keeps a part's facet.
- **A part is filed under its Model Card.** The **head** of a Card at Location `L` is found by walking `parent_location` from `L`'s parent through Model parts until a Model Card without a facet. A facet is refused when there is no head: ``a Model part (`<facet>`) must be filed under its Model Card: create it as a successor of the Model Card or of one of its parts``. Parts may form a chain (`3/2|a` setting, `3/2|a|1` assumptions, …) or each branch from the head.
- **The structure is computed, never stored.** A model's parts are the Model parts in the head's tree whose head is that Model Card, ordered by facet, then address. Moves keep facets on their rows, so a model moved with its subtree stays whole; a part moved away alone becomes detached and is reported as such.
- **A Model Card cannot abandon its parts.** Retyping a Model Card that heads parts, or giving it a facet, is refused: ``` `<address>` is the Model Card of N Model part(s) (…); change or move them first```.
- **Consistency is reported, not enforced.** `issues` lists each part the Model Card does not link and each part that does not link its Model Card. `missing` lists the facets with no part. `discussion` counts Cards outside the model that link any of its Cards.
- Tree summaries count models by their Model Cards, not their parts.

## Rendering

- **Math.** pulldown-cmark parses `$...$` and `$$...$$` (`ENABLE_MATH`) into `math-inline` / `math-display` spans, so `_`, `*`, and `\\` inside TeX are never Markdown. `\(...\)` and `\[...\]` in prose are rewritten to dollars first; code keeps its text. The page typesets the spans with KaTeX 0.16.22, bundled in the binary with its WOFF2 fonts (`/vendor/katex/…`), and typesets `$...$` runs in titles.
- **Charts.** A ```` ```chart ```` (or ```` ```html chart ````) fence is a chart. So is a raw HTML block whose first line opens `<figure>`, `<div>`, `<svg>`, `<table>`, `<section>`, `<canvas>`, `<script>`, `<style>`, or `<iframe>`: it runs to the line that closes that element (blank lines inside do not end it), and chart elements separated only by blank lines join into one chart. Each chart becomes `<iframe class="zt-chart-frame" data-chart="<id>">`, and the page loads `/chart/<id>?theme=light|dark`.
- **Chart isolation.** `/chart/<id>` serves pages only from an in-memory store of charts this server rendered (512 most recent), keyed by the SHA-256 of the page, so ids cannot be guessed. Its policy starts `sandbox allow-scripts`, which puts the page in an opaque origin: it cannot reach the GUI page, its DOM, or its token, and the API answers no preflights. The sandbox travels in the response, not in an iframe `sandbox` attribute, because some embedded browsers refuse to load attribute-sandboxed frames; for the same reason the page is self-contained: the frame script and, when the chart holds TeX, KaTeX with data-URI fonts are inlined when the page is served. The policy allows inline scripts, HTTPS libraries, and framing only by the GUI's own origins.
- **Chart contract.** The frame script gives the chart the card's palette as `--zt-*` CSS variables for both themes, typesets TeX in its HTML text, turns `[[address]]` text into links (and SVG text with it into clickable text), and posts only two messages to the page: `{zt:'height'}` and `{zt:'go', target}` for a click on `[[address]]` or an element with `data-zt`. The page sizes the frame and opens the Card if it exists.
- **Other HTML.** Raw HTML that is not a chart shows as text, except plain inline tags without attributes (`sub`, `sup`, `br`, `kbd`, `mark`, `small`, `u`, `ins`, `del`, `s`, `b`, `i`, `em`, `strong`); HTML comments are dropped. Excerpts skip lines that start with `<`.
- **Links.** Link macros inside a chart fence are code to the Link engine, so they are not Links: a Model Card lists its parts in Markdown as well as drawing them. Raw HTML charts are prose to the Link engine, so `[[…]]` in them must be valid Links.

## Interfaces

- **MCP.** `create_successor` takes `facet` (implies type `model`); `edit_card` takes `facet` (empty clears it), alone or with other changes; `get_card` reports `facet` and `model` (head, facet, parts with their Links inside the model, `head_links`, `missing`, `discussion`, `issues`); `get_tree` and `search_cards` report `facet`, and tree Cards report `head`; `get_model` returns the structure with every body and the facet glossary. The server instructions name facets, math, and charts.
- **GUI.** Parts carry a `MODEL · <FACET>` stamp in the reader, the map, and search. A Model Card or part shows divider tabs along its top edge (Model, then each facet; blank dashed tabs for missing facets; the open Card's tab stands up) and, on the desk below it, a model map: its Cards in reading order with an arc for each Link between them, the open Card's arcs inked, plus missing facets and issues. The writing desk shows a facet selector beside the Model type; the command bar takes `ty <type> [<facet>]`.
- **Session.** A part shows `[model · <facet>]` after its Location; `ty model <facet>` sets a facet, `ty model` makes a Model Card.
- **Dump.** `archive.json` gains a fourth key, `facets`, after `types`, listing every Model part in address order (`{}` when there are none).
