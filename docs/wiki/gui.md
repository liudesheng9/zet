---
block: gui
title: GUI
paths:
  - "src/gui.rs"
  - "src/gui/**"
  - "!src/gui/vendor/**"
  - "src/spread.rs"
  - "src/reply.rs"
synced: d0b7068543c7
updated: 2026-10-19
---

# GUI

## Purpose

The GUI is the local browser interface to the slip-box. The service daemon runs it for its whole life: a loopback HTTP server inside the daemon process serves one single-page app and a JSON API. The human uses it to read Cards (Markdown with TeX, HTML charts, artifacts and Document links), to walk trees on the Folgezettel map, to lay Cards from anywhere side by side on the Table, to see the whole archive as a Constellation, to study models and mathematical units, and to write Cards on the writing desk. Every change it makes goes through the same Card operations as the shell commands and the Session, with Owner Access (both Workspaces). An open GUI window is not a Session: it goes down with the service.

Beside the open or held Card's neighbours, the reader and the Table each show one **distant Card** (S19, #108, design §10.2): at most one Card of another book that shares an accepted register term or a model name with it, with one line of evidence. The slot is a read: opening, laying, and replying write no Link.

A `Bridge report` reader action lets the reader choose one existing immutable Bridge report for this browser session (S20, #112, design §10.3). The service projects that one file read-only through the installed `zt-digest` helper, caches the answer by report content hash and archive revision, and the slot prefers the report's still-current proposals; a missing helper, missing or invalid report, changed endpoint, or timeout is one small status line, and the reader keeps its term and model evidence. Nothing on this path applies a proposal: that still needs the digest compiler's exact report and proposal acceptance.

The service also answers a **question** (S15, #114, design §9.3-§9.5): the authenticated `POST /api/ask` route returns one unsaved Spread proposal — the matched accepted terms, the held entry Card, up to nine explained Cards with their paths, endpoint fingerprints, counts, bounds, and the archive revision and identity — from the same `entry::gather` function the MCP server's `ask_box` calls. It is read-only: it saves no Spread and changes no Card. The `Ask the box` command, its temporary Table preview, and keeping a proposal as a normal Spread are S16's work (#116) and are not in this block.

**Owns:** the HTTP server in the daemon (bind, port, the per-service token, the `zt.gui` record), every `/api/` route and static asset route, the cached Snapshot the API reads, the Card-body Markdown-to-HTML renderer (Link chips, math spans, HTML charts, artifact figures, Document links), the sandboxed chart pages, the Scholar Archive reading page, the front-end views (`src/gui/*.js`, `*.css`, `index.html`), the pure layout algorithms (`layout.js`) and writing-desk text rules (`draft.js`), Spreads (`src/spread.rs`, the `gui_spreads` metadata row), the reader and Table slots that show the distant Card (`/api/distant`, `distantSlot`, `paintDistant`, `requestDistant`), the optional Bridge report projection behind that route (`Projections`: the fixed helper argument vector, its ten-second cancellation, the content-hash/archive-revision cache, and the report status line), and the `POST /api/ask` route (`App::ask`: `AskArgs::from_args` then `read::ask`, with the existing token guard).

**Does not own:**
- Starting and stopping the daemon, the service record, `zt up` / `zt down` / `zt status`: [Service, CLI and storage](service-and-cli.md).
- Card creation, edit, delete, move, insert, their plans and confirmations, and the Snapshot type itself (`ops::Snapshot`): [Card operations](card-operations.md).
- Card text format, Link parsing and classification, Workspaces and Access: [Cards, Workspaces and Links](cards-and-links.md).
- The model structure, build report, Relation kinds, and the `math` report the boards draw: [Models and math](models-and-math.md).
- Hub membership (`hub::hubs_of`) and the distant selection's own rules (`entry::distant`: its candidate sources, exclusions, cross-source check, and explanation): [Hub Cards](hub-cards.md).
- The read-only projection itself (`zt-digest bridge -Step project`: report-id, endpoint-fingerprint, and eligibility validation, and the twenty-candidate bound): [zt-digest](zt-digest.md).
- The Scholar Archive bridge, artifact storage and Document targets (`scholar.rs`, `artifact.rs`): [Scholar bridge](scholar-bridge.md).
- The terminal Session, whose command vocabulary the command bar copies: [Terminal Session](session.md).

## At a glance

| | |
|---|---|
| Entry points | `gui::start(root)` (called by the service daemon in `src/main.rs`), `gui::run(args)` (`zt gui [--open]`), `App::handle(request)` (every HTTP request), `boot()` in `src/gui/app.js` (the page) |
| Key files | `src/gui.rs` — server, routes, Snapshot cache, chart store, Card Markdown renderer, response policies |
| | `src/gui/document.rs` — `/scholar/file/<id>` and `/scholar/figure/<id>`: Scholar Archive documents (PDF passthrough, Markdown reading page) |
| | `src/spread.rs` — load, save, normalize, and follow Spreads in the `metadata` table |
| | `src/gui/index.html` — the page shell; the token is substituted for `__ZT_TOKEN__` |
| | `src/gui/app.js` — the whole front end (state, routing, history, search tray, cabinet, desk, Map, Table, Constellation, reader, model board, theory board, study desk, dialogs, writing desk, command bar, keys, heartbeat) |
| | `src/gui/layout.js` — pure, DOM-free geometry (`ZtLayout`): outline, focus lens, Constellation rings, Table placements, threads, theory rows, study stations |
| | `src/gui/draft.js` — pure, DOM-free writing-desk text rules (`ZtDraft`): the Reply draft skeleton, the relation-prefix swap, the destination Topic choice |
| | `src/gui/chart-frame.js` — the script inlined into every chart page: palette tokens, `[[address]]` links, TeX, height reports |
| | `src/gui/doc.js` — the reading page's script: lazy KaTeX and centring a `#fig-…` target |
| | `src/gui/app.css` — the whole look (index-card paper, light and dark themes) |
| | `src/gui/vendor/katex/**` — vendored KaTeX 0.16 (JS, CSS, WOFF2 fonts); not documented here |
| Depends on | [Card operations](card-operations.md) (`ops::*`, `ops::Snapshot`), [Cards, Workspaces and Links](cards-and-links.md) (`LinkBody`, `LinkStatus`, `parse_card_text`, `Workspace`, `Access`), [Models and math](models-and-math.md) (`model::Part`, `Facet`, `Emphasis`, `math::MathForm`, `Snapshot::model`), [Hub Cards](hub-cards.md) (`hub::hubs_of`), [Scholar bridge](scholar-bridge.md) (`scholar::Bridge`, `artifact::read/source/sniff`), [Service, CLI and storage](service-and-cli.md) (`open_db`, `card_counts`, `log_event`, `require_service_up`); crates `tiny_http` 0.12, `pulldown-cmark` 0.13, `sha2`, `getrandom`, `rusqlite`, `serde_json`; KaTeX in the browser |
| Used by | [Service, CLI and storage](service-and-cli.md) (daemon starts/stops it, `zt status`/`zt up` print its URL, delete/move call `spread::follow`); the human's browser |
| Tests | unit tests in `src/gui.rs` (`tests`, `render_tests`), `src/gui/document.rs`, `src/spread.rs`; integration tests `tests/cli_service.rs`, `tests/model_structure.rs`, `tests/card_types.rs`, `tests/workspaces_mcp.rs`, `tests/self_sufficient_models.rs`, `tests/scholar_bridge.rs` |

## Concepts

- `GUI` — the browser interface the service runs while it is up (CONTEXT.md). `src/gui.rs` `Running` is the running server; `App` holds its state.
- `Access::Owner` — the GUI's Access (`const ACCESS` in `src/gui.rs`): the human's own tool, so it may change both Workspaces.
- `Token` — 32 hex characters from 16 random bytes (`random_token`), new for each daemon start, embedded in the page as `<meta name="zt-token">` and required on every `/api/` call as the `X-ZT-Token` header.
- `Snapshot` — every Card with its parsed text and Link graph (`ops::Snapshot`); the GUI keeps one in `Snapshots` until another commit moves SQLite's `data_version`.
- `Card summary` — the per-Card JSON `Snapshot::summary` builds (address, tree, workspace, title, kind, type, facet, emphasis, level, form, head, edge, parent, excerpt, links, kinds, inbound); `/api/graph` and `/api/tree` return lists of them, and the page indexes them in `state.byAddress`.
- `Fingerprint` — a 24-hex digest of every Card's address, text, BibTeX, Workspace, type, facet, emphasis, level, and form (`Snapshot::fingerprint`); the page polls it to notice changes from other clients.
- `HTML chart` — HTML in a Card body shown as a sandboxed figure (CONTEXT.md). The server stores its page in `Charts` under a content id and the Card HTML holds `<iframe class="zt-chart-frame" data-chart="<id>">`.
- `Artifact` / `Document link` — `![caption](artifact:<sha256>)` renders as a `.zt-figure`; `[label](scholar:<file_id>)` renders as an `a.zt-doc` to `/scholar/file/<file_id>` (CONTEXT.md; storage in [Scholar bridge](scholar-bridge.md)).
- `Pointer` — the Card the page is at (`state.pointer`, `null` for ROOT), carried in the URL hash `#/card/<address>`.
- `Cabinet` — the left rail: the Workspace switch, the Topic zone and Literature zone drawers (`renderRail`).
- `Desk` — the ROOT view: one stack of index cards per tree of the current Workspace (`renderDesk`).
- `Folgezettel map` — the tree view, laid out by address through a focus lens (`map` in `app.js`, `ZtLayout.focusLens`).
- `Outline` — `layout.js`'s reading of a tree: a Card's outline children are the runs of its Side successors (a run is a Card and its chain of Direct successors); a tree root's outline children are its own Direct run.
- `Table` / `Spread` — the working surface where Cards from anywhere lie side by side, and one named, persisted set of Cards on it with their positions (CONTEXT.md; `tableView`, `spreads` in `app.js`, `src/spread.rs`).
- `Within reach` — the held Card's neighbours that are not yet on the Table, offered as slips beside it (`candidates`, `renderReach`).
- `Distant Card` — the one Card of another book the reader's open or held Card shares an accepted register term or a model name with, and the faint slot that shows it in the reader and under the Table's slips (CONTEXT.md "Bridge proposal"; §10.2; `distantSlot`, `distantBody`, `paintDistant`, `requestDistant`, `entry::distant`). The slot is one slot per view, never a second recommendation tray (D292).
- `Constellation` — every tree as a radial outline, the trees set around one ring, Links between trees bundled (`sky` in `app.js`, `ZtLayout.constellation`).
- `Reader` — the right column: the open Card as a ruled index card, then its tools (`renderReader`).
- `Model board` — the view of one model below a Model Card or Model part (CONTEXT.md; `modelBoard`, `modelTabs`, `buildFaults`).
- `Theory board` / `Study desk` — the views of a Theory Card's units by dependency and of a unit's study order, station by station (`theoryBoard`, `study`; layout in `ZtLayout.theoryRows`, `studyStations`, `theoryRun`).
- `Reading mode` — `learn`, `review`, or `lookup` (`READING` in `app.js`), kept in `localStorage` `zt-reading`.
- `Writing desk` — the editor dialog: raw Markdown on the left, a server-rendered preview on the right (`openEditor`).
- `Relation kind` — what a Link says (CONTEXT.md); the page reads it from summary `kinds` and model `relations` and names it with `KINDS` in `app.js`.

## Architecture

Three layers: the server in the daemon (`src/gui.rs` with `src/gui/document.rs` and `src/spread.rs`), the page (`index.html`, `app.js`, `layout.js`, `draft.js`, `app.css`, KaTeX), and two kinds of isolated pages the server builds (chart pages with `chart-frame.js` inlined, and the Scholar Archive reading page with `doc.js`).

```mermaid
flowchart LR
  subgraph Daemon["service daemon (src/main.rs)"]
    start["gui::start"] --> worker["zt-gui thread\nApp::handle"]
    worker --> statics["static routes\n(include_str! assets)"]
    worker --> api["App::route\n/api/*"]
    api --> snaps["Snapshots\n(cached ops::Snapshot)"]
    api --> ops["ops::create/edit/delete/move/insert\n(Access::Owner)"]
    api --> spread["spread::load/save"]
    api --> render["render_markdown"]
    render --> charts["Charts store\n(512 pages, FIFO)"]
    worker --> chartroute["/chart/&lt;id&gt;\nserve_chart_page"]
    worker --> docs["document::serve\n(thread per request)"]
    docs --> bridge["scholar::Bridge"]
    api --> bridge
  end
  db[("zt.sqlite3")]
  snaps --> db
  ops --> db
  spread --> db
  subgraph Browser
    page["app.js + layout.js + draft.js\n(state, views, desk rules)"]
    frame["chart iframe\n(opaque origin)"]
    reading["reading page\n(doc.js)"]
  end
  page -- "fetch /api/* + X-ZT-Token" --> api
  page -- "GET /, /app.js, …" --> statics
  frame -- "GET /chart/id?theme" --> chartroute
  frame -- "postMessage height / go" --> page
  page -- "open /scholar/file/id" --> reading
  reading --> docs
```

How data and control move:

1. The daemon calls `gui::start`, which binds a `tiny_http::Server` on `127.0.0.1`, builds an `App` (root, token, port, `Charts`, `Snapshots`), writes `<archive_root>/zt.gui`, and spawns the `zt-gui` thread. That thread handles requests one at a time; each request runs inside `catch_unwind`, so a panic answers nothing for that request but keeps the server up.
2. `App::handle` checks the `Host` header, serves static assets, artifacts, chart pages, and Scholar documents without a token, and passes `/api/*` to `App::route` only with the right `X-ZT-Token`.
3. Read routes take the cached Snapshot (`Snapshots::get`), so most reads do no SQL beyond `PRAGMA data_version`. Write routes call `ops::*` with `ACCESS`, which open their own connections and transactions; the commit moves `data_version`, so the next read rebuilds the Snapshot.
4. Card bodies leave the server as HTML from `render_markdown`; charts inside them become iframes whose pages the server keeps in `Charts`.
5. `/scholar/file/*` and `/scholar/figure/*` are handed to `document::serve`, which answers on a thread of its own, because the bridge may take seconds.
6. On the page, `app.js` holds one `state` object; `route()` follows the URL hash; `renderAll()` redraws the topbar, rail, stage (desk, Map, Table, or Constellation), and reader. Geometry comes from `ZtLayout` (`layout.js`), and the writing desk's Reply text rules come from `ZtDraft` (`draft.js`); both are pure and can also be loaded as CommonJS modules.
7. The page polls `/api/overview` every 4 s (and on window focus) for the fingerprint and `/api/ping` every 2 s for the heartbeat; a changed fingerprint triggers `refresh()`.

## Interfaces

### Rust functions (crate-internal)

| Item | Signature | Behaviour |
|---|---|---|
| `gui::run` | `pub(crate) fn run(args: &[String]) -> Result<()>` | `zt gui [--open]`. Requires the service up (`require_service_up`), reads `zt.gui`, prints `zt gui: <url>` and `port: <n>`; `--open` calls `open_window`. Any other argument fails with a usage line that says `--port` and `--no-open` are gone and names `ZT_GUI_PORT`. Fails when the service has no recorded URL (pre-0.14 daemon, or the GUI could not start; points at the log). |
| `gui::recorded_url` | `pub(crate) fn recorded_url(root: &Path) -> Option<String>` | The trimmed contents of `<root>/zt.gui`, or `None` when missing or empty. Used by `zt up`, `zt status`, `print_gui_line` in `src/main.rs`. |
| `gui::clear_record` | `pub(crate) fn clear_record(root: &Path)` | Removes `zt.gui`; ignores errors. Called on stop, when start fails, when a stale service is cleared, and when the system ends the session. |
| `gui::start` | `pub(crate) fn start(root: &Path) -> Result<Running>` | Binds (see Configuration), makes the token, writes `zt.gui` (`http://127.0.0.1:<port>/\n`), spawns the `zt-gui` thread, logs `gui started on <url>`. Errors: invalid `ZT_GUI_PORT`, bind failure, cannot write the record, cannot spawn. The daemon logs `gui did not start: …` and runs on without a GUI. |
| `Running::stop` | `pub(crate) fn stop(self)` | Clears the record, `server.unblock()`, joins the worker (the request in hand finishes), logs `gui stopped`. |
| `spread::load` | `pub(crate) fn load(conn: &Connection) -> Result<Value>` | The stored Spreads, normalized; an unreadable or invalid row reads as `{"current": null, "spreads": []}`. |
| `spread::save` | `pub(crate) fn save(conn: &Connection, value: &Value) -> Result<Value>` | Normalizes (errors on bad shape or bounds) and replaces the row; returns what was kept. |
| `spread::follow` | `pub(crate) fn follow(conn, mapping: &BTreeMap<String,String>, deleted: &BTreeSet<String>) -> Result<()>` | Rewrites every Spread's addresses by `mapping` and drops `deleted` Cards; writes only when something changed. Called inside the delete transaction and by the Location-mapping step in `src/main.rs`, so Spreads follow moves, Citation-key renames, and side-successor compaction. |
| `document::figure_anchor` | `pub(crate) fn figure_anchor(path: &str) -> String` | `fig-<file stem>` with every character outside `[A-Za-z0-9_-]` replaced by `-`. Used by `/api/artifact` to build `in_document`. |

### CLI

```text
zt gui [--open]
```

Output when up:

```text
zt gui: http://127.0.0.1:4717/
port: 4717
```

`--open` prefers a chromeless app window (`--app=<url>`) of Edge (`ProgramFiles(x86)`, `ProgramFiles`) or Chrome (`ProgramFiles`, `LOCALAPPDATA`) on Windows, else `cmd /C start`; `open` on macOS; `xdg-open` on other Unix (`open_window`).

### Static and page routes (no token)

Every request first passes `host_allowed`: the `Host` header must be exactly `127.0.0.1:<port>` or `localhost:<port>` (case-insensitive), else `421 misdirected request`.

| Method and path | Answer | Headers |
|---|---|---|
| `GET /`, `GET /index.html` | `index.html` with `__ZT_TOKEN__` replaced by the token | `respond_text` policy (below) |
| `GET /app.css`, `/app.js`, `/layout.js`, `/draft.js`, `/ask.js`, `/doc.js` | the embedded file | `respond_text` policy |
| `GET /vendor/katex/katex.min.js`, `/vendor/katex/katex.min.css` | embedded KaTeX | `Cache-Control: max-age=86400` |
| `GET /vendor/katex/fonts/<name>` | one of the 20 embedded WOFF2 fonts (`font/woff2`), else 404 | `max-age=86400` |
| `GET /artifact/<sha256>` | the artifact bytes from `artifact::read`, with its media type; 404 `artifact not in this archive` | `max-age=31536000, immutable`; CSP `sandbox; default-src 'none'; style-src 'unsafe-inline'` |
| `GET /chart/<id>?theme=dark\|light` | the stored chart page with theme and scripts filled in (`serve_chart_page`); 404 `chart expired; reopen the Card` | chart CSP (below), `no-store` |
| `GET /scholar/file/<file_id>` | via `document::serve`: a Markdown document as a reading page; a PDF inline (`application/pdf`); any other file as an attachment (`application/octet-stream`); 502 when the bridge fails, 500 when the file cannot be read | reading page: `READING_POLICY`, `no-store`, `Referrer-Policy: no-referrer`; files: `Content-Disposition` with an RFC 5987 file name, `private, max-age=60`; all: `Cross-Origin-Resource-Policy: same-origin` |
| `GET /scholar/figure/<file_id>?path=<path>` | an image of the document, fetched through the bridge and sniffed; 415 `not an image`; 502 on bridge errors | `private, max-age=86400`, CSP `sandbox; …` |
| any other non-`/api/` path | 404 `not found` | |

The `respond_text` policy (page, scripts, CSS, JSON) is `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, and `Content-Security-Policy: default-src 'self'; img-src 'self' https: data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self'; frame-ancestors 'none'`.

The chart policy (`respond_chart`) is `sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline' <local> https:; style-src 'unsafe-inline' <local> https:; font-src <local> https: data:; img-src <local> https: data: blob:; connect-src https:; frame-ancestors <local>; form-action 'none'; base-uri 'none'`, where `<local>` is `http://127.0.0.1:<port> http://localhost:<port>`.

### API routes (`/api/*`, token required)

All answers are JSON. A missing or wrong token gives `403 {"error":"missing or invalid GUI token"}`. A POST body that is not UTF-8 or not JSON gives 400; an empty POST body reads as `{}`. Any error from a handler gives `400 {"error":"<message with causes>"}`; an unknown path gives 400 `unknown endpoint <path>`. Query parameters are percent-decoded, `+` is a space (`parse_query`).

| Method and path | Inputs | Output | Side effects |
|---|---|---|---|
| `GET /api/ping` | — | `{"ok":true}` | none (heartbeat) |
| `GET /api/overview` | — | `{fingerprint, archive_root, stats:{total,topics,regular,literature}, broken, workspaces:[{id,label,topic_trees,literature_trees,cards,agent_writable}], trees:[Snapshot::trees]}` | built once per Snapshot |
| `GET /api/tree?id=<tree>` | tree id (Topic number or Citation key) | `{tree, root, workspace, kind:"topic"\|"literature", title, cards:[summary]}` | error `tree … does not exist` |
| `GET /api/card?at=<address>` | Location or Citation key | `{address, tree, workspace, kind, type, facet, emphasis, level, form, model, hubs, context:"tree-root"\|"regular", title, text, body, html, bibtex, citation_key, authored_fingerprint, parent, direct, sides, inbound, outbound}` | `authored_fingerprint` is the Card's authored identity (title, body, BibTeX and type fields; the generated Reverse section is not authored and not in it), which a Reply captures and re-checks. Renders the body (may add charts to the store). |
| `GET /api/graph` | — | `{nodes:[summary]}` for every Card | — |
| `GET /api/register?ws=&entry_offset=&entry_limit=&at=` | `ws` required (`human` or `agent`); `entry_offset`/`entry_limit` page the flattened entries (default 200, at most 1,000); `at` names one Card | `read::register`: `{workspace, register, title, register_missing, duplicate_registers, entries:[{term, line, origin, targets:[{address, exists, title, type, facet, workspace, tree, body, body_chars, body_lines, body_truncated}], complete, faults}], entry_offset, entry_limit, entry_total, entry_next_offset, parts, faults, complete, terms_by_card, restatements:[{entry, substantive, terminal, title}], bounds, archive_revision, archive_identity, terms_at, terms}` (`terms_at` is null and `terms` empty unless `at` is given; then `terms` holds the accepted terms that Card carries) | reads the archive in its own transaction (`src/read.rs`), never the cached Snapshot; no writes |
| `GET /api/search?q=&ws=&type=&mode=` | `q` required; `ws` `human`\|`agent`\|`all`; `type` a Card type or `all`; `mode` `entry` or `plain` (default `plain`) | `mode=plain`: `Snapshot::find` with limit 80 (`SEARCH_LIMIT`) plus `mode: "plain"`: `{mode, results:[{address,title,kind,type,facet,emphasis,level,form,workspace,tree,tree_title,field,score,snippet}], total, workspaces:{id:n}, types:{type:n}}`. `mode=entry`: `entry::search` with the same limit: the ranked rows (with `role`, `rank_tier`, evidence, `origin`, `entry_targets`, and the fold fields `folded`, `member_total`, `representative`, `members`, `members_truncated`) plus `mode`, `shown`, `total`, `row_total`, `folded_total`, `workspaces`, `types`, `registers`, `register_missing`, `register_faults`, `bounds`, `scanned`, `archive_fingerprint` — the same value the MCP `search_cards mode: entry` returns. An empty query stays the plain listing, whatever mode was asked for. | — |
| `GET /api/distant?at=&table=&report=` | `at` required (the open or held Card); `table` the addresses of the Cards on the Table, comma-separated, at most 400; `report` the exact absolute path of one Bridge report the reader selected | `entry::distant`: `{at, title, workspace, focus:{terms,sources}, register, candidate (null or one Card with its `source`, `term`, `origin`, `report`, `reason`, `evidence`), source, excluded, bounds, scanned, archive_fingerprint}`; with `report`, also `report:{selected, hash, status, report_id, candidates, counts, errors, cached}`, and the candidate's proposals are the report's current ones (`source: "bridge_report"`). `status` is `pending`, `done`, `stale`, `not_in_report`, `missing_report`, `missing_helper`, `invalid_report`, `timeout`, or `failed`. A missing Card is a 400 naming it. | none (read-only; no Card, Link, view, or report is written; the optional helper runs with a fixed argument vector and is cancelled after ten seconds) |
| `GET /api/broken` | — | `{broken:[{source,title,workspace,target,line}]}` | — |
| `POST /api/ask` | `{question, workspace?: "human"\|"agent"\|"all", limit?: 5..9}`: a trimmed nonblank question of at most 512 characters, the Workspace filter (default `all`), and the selection size (default 7, clamped) | `read::ask` → `entry::gather`: the unsaved Spread proposal — `{question, workspace, limit, matched_terms, terms:[{term, at, origins, targets}], ambiguous_terms, fallback, registers, faults, held, spread:{name, question, cards:[{at,x,y}]}, cards:[{at, held, title, workspace, tree, type, term, origin, via_hub, depth, seed, path, statement, reason, authored_fingerprint, position}], reasons, threads, excluded, sparse, truncated, counts, bounds, archive_revision, archive_identity}` — the same value the MCP `ask_box` returns | the token guard (403); a blank, missing, or over-512-character question is a 400 with `error`; read-only, so it saves no Spread and changes no Card. The page's Keep then posts the ordinary Spread document to `POST /api/spreads` (S16, #116) |
| `POST /api/render` | `{text, literature?:bool}` | `{title, html, error}`; `error` is the parse or Link error, the HTML is still given (raw text rendered when the Card text does not parse) | may add charts |
| `POST /api/create` | `{kind:"topic"\|"direct"\|"side", text, at? (successors), workspace? (topic, default human), type?, facet?, emphasis?, level?, form?}` | `{address}` | `ops::create_topic` or `ops::create_successor`; then, when `level`/hub level/`form` is given, a second `ops::edit` |
| `POST /api/literature/check` | `{bibtex}` | `{citation_key, title, exists}` (case-insensitive) | reads the DB |
| `POST /api/literature` | `{bibtex, text, workspace?}` | `{address}` | `ops::create_literature` |
| `POST /api/edit` | `{at, text?, base?, type?, facet?, emphasis?, level?, form?}` | `{address}` | `ops::edit`; `text` required when no type field is given; `base` makes the save fail if the Card changed since it was opened |
| `POST /api/metadata` | `{at, bibtex, base?, confirm?}` | `{confirm, lines}` (plan) or the done result with `address` | `ops::edit_metadata` (a Citation-key rename needs `confirm`) |
| `POST /api/delete` | `{at, into?, confirm?}` | plan `{confirm, lines}`, then `{address, deleted, moved}` | `ops::delete` |
| `POST /api/move` | `{at, to \| before, alone?, confirm?}` | plan, then `{address, moved}` | `ops::move_card`; exactly one of `to`/`before`, else `give either \`to\` or \`before\`` |
| `POST /api/insert` | `{before, text, type?, facet?, emphasis?, confirm?}` | plan, then `{address}` | `ops::insert_card` |
| `POST /api/reply` | `{at, base, text, topic?}`: the agent Card, its authored fingerprint as the page read it, the reply Card text, and the human Topic last used (absent, empty or gone: the default) | `{saved:true, address, kind, target:{address,title,workspace,authored_fingerprint}, destination:{topic,title,created_topic,parent,edge}}` or `{saved:false, reason, detail, target:{…fresh…}}` with 200, so the reader keeps the draft and sees the fresh context; reasons are `target_changed`, `target_not_agent`, `reply_bare`, `link_unkinded`, `target_link_missing` | `src/reply.rs`: one transaction writes the human Card (creating the default Topic `回应` when needed), then regenerates Reverse links; a missing Card or field is the ordinary 400 |
| `GET /api/artifact?sha=<sha256>` | digest | `{sha256, held, source, in_document}`; `in_document` is `/scholar/file/<file_id>#fig-<stem>` when the source names a file and path | — |
| `GET /api/scholar/works?q=` | text (may be empty) | `{works:[{work_id,title,year,authors,doi}]}`: Works with a Markdown file, title or author contains `q`, newest first, 40 at most | bridge SQL |
| `GET /api/scholar/artifacts?work_id=<n>` | number | the bridge's `list_work_artifacts` answer | bridge call |
| `GET /api/scholar/files?work_id=<n>` | number | `{files:[{file_id,name,path,kind,bytes,shown,bilingual,markdown}]}`; only the shown files when the Work shows any; `markdown` is the Document link to insert | bridge SQL |
| `POST /api/scholar/preview` | `{file_id, path}` | `{data_url}` (base64 image) | nothing kept |
| `POST /api/scholar/artifact` | `{file_id, path, caption?}` | `scholar::keep_artifact` result, with `markdown` | copies the image into `artifacts/` |
| `GET /api/spreads` | — | `{current, spreads:[{id, name, question?, cards:[{at,x,y}]}]}` (`question` is present only on a Spread kept from a question) | — |
| `POST /api/spreads` | the whole Spread document | the normalized document: a `question` is kept (at most 512 characters), and an over-long `name` is cut to 80 characters | replaces the `gui_spreads` row |

Field rules shared by create, edit, and insert (`type_field`, `facet_field`, `emphasis_field`, `form_field`, `ops::parse_level_arg`): an absent field leaves the value alone; `""` or `null` clears it; a facet makes the Card a Model Card (another type with a facet is refused: `only Model Cards can have a Model facet`); a level needs a Model Card without a facet (`LEVEL_NEEDS_MODEL_CARD`) or, for hub levels, a Hub Card (`LEVEL_NEEDS_HUB_CARD`); a form needs a Model Card without a facet (`FORM_NEEDS_MODEL_CARD`); a Topic must be an Idea Card.

### Messages between a chart frame and the page

| Direction | Message | Effect |
|---|---|---|
| frame → page | `{zt:"height", height:<px>}` | the page sets that iframe's height, clamped to 40–4000 px |
| frame → page | `{zt:"go", target:"<address>"}` | the page goes to the Card, or toasts `card … does not exist` |

The page accepts a message only when `event.source` is the `contentWindow` of one of its `iframe.zt-chart-frame` elements.

### Chart authoring surface (inside a chart)

- `window.ZT_LINKS` — `{ "<address>": {title, exists} }` for each `[[address]]` in the chart source.
- `[[address]]` in chart text becomes `a.zt-link` (or, in SVG text, sets `data-zt` on the text element); any element with `data-zt="<address>"` or `a[href^="#/card/"]` opens that Card on click.
- CSS tokens `--zt-paper`, `--zt-ink`, `--zt-ink-soft`, `--zt-ink-faint`, `--zt-rule`, `--zt-line`, `--zt-accent(-soft)`, `--zt-link(-soft)`, `--zt-topic(-soft)`, `--zt-model(-soft)`, `--zt-event(-soft)`, `--zt-series-1` … `--zt-series-6`, `--zt-serif`, `--zt-sans`, `--zt-mono`, with a dark set under `:root[data-theme="dark"]`.
- TeX in `$…$`, `$$…$$`, `\(…\)`, `\[…\]` in chart text (outside `script, style, textarea, code, pre, svg`) is typeset when KaTeX was inlined.

### `ZtLayout` (`src/gui/layout.js`)

Exported as `window.ZtLayout` or `module.exports`. All functions are pure.

| Function | Inputs → output |
|---|---|
| `sideRank(address)` | the last segment's letter label as a number (`a`=1, `z`=26, `aa`=27) |
| `outline(cards, rootAddress)` | summaries with `address`, `parent`, `edge` → `{root, card, kids, up, depth, size, order}` |
| `pathTo(tree, address)` | outline path from the root, inclusive |
| `focusLens(tree, focus, {links, expandAll})` | `{nodes:[{address,lod,x,y,w,h,depth,folded,role}], index, edges:[{from,to,kind,shape,hot}], focus, bounds}` |
| `edgePath(edge, p, c)`, `elbow(p, c)`, `linkArc(a, b)` | SVG path strings for Map edges and Link arcs |
| `constellation(nodes, treeInfo)` | `{trees:[{id,root,r,local,count,outline,x,y,angle,…}], pos: Map, R, bundles:[{a,b,count}]}` |
| `addressOrder(a, b)` | Folgezettel comparator |
| `freeSpot(rects, want, size)` | nearest free top-left corner |
| `readingOrder(addresses, linksOf)` | an order that follows Links depth first |
| `lineUp(order, size)`, `gather(addresses, focus, isNear, size)`, `byTree(addresses, size)` | `Map(address → {x,y})` |
| `thread(a, b, back)` | `{d, mid, end, angle}` for a sagging thread |
| `theoryRows(units)` | `{rows:[[address]], base:[address], hung: Map}` |
| `labelsIn(text)` | `Set` of `H1`, `C2`, … |
| `studyStations(study, mode, head, isPlan)` | stations of one unit in one reading mode |
| `theoryRun(math, head, mode, unitStations)` | stations of a whole theory |
| `LOD`, `GUTTER`, `TABLE` | sizes: focus 304×136, near 236×76, slip 236×30, gutter 56; Table card 236×150, gap 30 |

### `ZtDraft` (`src/gui/draft.js`)

Exported as `window.ZtDraft` or `module.exports`; the page loads it before `app.js`. All functions are pure, so the Reply desk's text rules run under Node without a browser.

| Function | Inputs → output |
|---|---|
| `replyDraft(word, target)` | the desk's opening text: `# ` title line, blank line, `<word> [[<target>]]：`, blank line, the `<!-- zt:reverse-links -->` marker |
| `replySwapPrefix(value, before, after)` | the draft with `before` replaced by `after` wherever it stands; when `before` is gone, `after` is inserted at the start of the body — after the title heading, never above it |
| `replyDestination(topics, recent, defaultTitle)` | the human Topic entry for `recent` while it exists, else the entry titled `defaultTitle`, else `null` (the desk then offers to create the default) |

### `ZtAsk` (`src/gui/ask.js`)

Exported as `window.ZtAsk` or `module.exports`; the page loads it before `app.js` (after `draft.js`). All functions are pure, so the Ask rules run under Node without a browser.

| Function | Inputs → output |
|---|---|
| `askQuestion(raw)` | `{ok:true, question, name}` for a trimmed nonblank question of at most 512 characters (counted as characters), else `{ok:false, error}` |
| `askName(value)` | the display name: trimmed, then cut to the Spread schema's 80 characters — the cut counts characters, so it never splits a surrogate pair |
| `askPreview(proposal)` | the temporary Table: `{id: "ask:" + question, name, question, held, cards:[{at, x, y, title, reason, fingerprint}], sparse, truncated, fallback, matched_terms, ambiguous_terms, counts}`; positions and places come from `proposal.spread.cards`, the reasons and fingerprints from `proposal.cards` by address |
| `askReason(preview, at)` | the reason the Table holds that Card (`""` when the answer explained it with nothing) |
| `askKeep(doc, preview, id)` | a new Spread document holding every earlier Spread as it was plus this question's one `{id, name, question?, cards:[{at,x,y}]}` as `current`; the reasons and fingerprints are not written |
| `askChanged(preview, fresh)` | `{changed:[{at, then, now}], missing:[{at, then}], unchanged:[at]}` — a Card whose authored fingerprint moved (a new occupant of an old address included) is `changed`, a vanished one `missing`, and a proposal with no fingerprint is never trusted silently |
| `askWithout(preview, addresses)` | the same temporary Table without the named Cards (Keep saves only what still stands) |

### Keys and command bar

The command bar (`installCommandBar`, `runCommand`) speaks the Session vocabulary listed in `COMMANDS` in `app.js`: `go [<target>]`, `root`, `ws [<workspace>]`, `up`, `back`, `fwd`, `ls`, `t <title>`, `l`, `n`, `b`, `e`, `reply`, `ask [<question>]`, `del`, `ins`, `mv [alone] <new-location>`, `mv [alone] before <location>`, `ty <idea|event|finding|model|hub> [<facet>]`, `lv <level|none>`, `mf <form|none>`, `stats`, `status`, `lsbk`, `help`, `q`, each with an availability (`always`, `card`, `regular`, `agent-card`). Text that is no command offers `search "…"`.

Keys outside text fields (`installKeys`): `:` command bar, `/` or `Ctrl+K` search, `Alt+←`/`Alt+→` or `Backspace` history, `g` go, `n` `b` `e` `r` `t` `l` `m` `i` `Delete`, `u`/`↑`/`←` parent, `↓` Direct successor, `→` first Side successor, `v` next view, `p` lay on / take off the Table, `P` lay down with every related Card, `y` keep a question's proposed Table, `w` switch Workspace, `f` fit, `x` unfold the whole tree, `s` study, `?` help, `Esc`. `r` replies to the open agent Card and does nothing for a human Card or a tree-root-only view; `y` does nothing unless a question's proposal lies on the Table, and `Esc` cancels that proposal before it closes anything else. No key fires inside an input, the writing desk, or a modal (`isTyping`, `state.editing`, `finder.isOpen`). While the study desk is open it takes the keys (`study.key`: `←`/`→`/`j`/`k`/PageUp/PageDown, Space/Enter turn over, `1`/`2`/`3` reading mode, Esc).

## Behaviour

### Daemon start and stop

1. `src/main.rs` (daemon) takes the daemon lock, logs `daemon started`, then calls `gui::start(&root)` before it writes the service record, so a service that reads as up already has its `zt.gui`.
2. `configured_port()` reads `ZT_GUI_PORT`; `bind()` tries that port once, or the default 4717 up to 10 times 100 ms apart (a just-stopped service may still hold it), then port 0 (any free port).
3. `start` creates `App { root, token: random_token(), port, charts, snapshots }`, writes `zt.gui`, and spawns `zt-gui`, which loops over `server.incoming_requests()`.
4. When `shutdown::watch` returns, the daemon calls `Running::stop`: clear the record, unblock the server, join the thread, log `gui stopped`.
5. A restarted service has a new token; old pages get 403 and reload (see Heartbeat).

### Handling one request (`App::handle`)

1. Split the URL into path and query; `parse_query` decodes the query.
2. Refuse a wrong `Host` with 421.
3. Match static routes in order: page, CSS/JS, KaTeX assets and fonts, `/artifact/`, `/doc.js`, `/scholar/file/` and `/scholar/figure/` (handed to `document::serve`, which spawns a thread and returns at once), `/chart/`.
4. Anything else not under `/api/` is 404.
5. Check `X-ZT-Token` (exact match), read and parse a POST body, call `route`, answer 200 with the value or 400 with `{"error"}`.

### The Snapshot and its caching (`Snapshots`)

1. `Snapshots::get` locks the mutex and calls `current`.
2. `current` takes the held `Cached { conn, version, snapshot, overview }`. If `PRAGMA data_version` on the held connection still equals `version`, the same `Arc<Snapshot>` is returned.
3. Otherwise (another connection committed), it keeps the connection (or opens a new one if the query failed), reads `data_version` first, then `Snapshot::read(root)`, and stores both with `overview: None`. Reading the version before the Snapshot means a commit in between makes the next request read again rather than keep a stale Snapshot.
4. `Snapshots::overview(build)` returns the overview JSON memoized for the current Snapshot, building it on the first ask. The page asks for it every 4 seconds, so it must stay cheap.
5. `data_version` changes only for commits on other connections. Every write the GUI makes goes through `ops::*` on its own connection, so the GUI's own writes also invalidate the cache. The unit test `a_snapshot_is_kept_until_another_connection_changes_the_cards` checks this.

### Rendering a Card body (`render_markdown`)

1. `normalize_math_delimiters` rewrites `\(…\)` to `$…$` and `\[…\]` to `$$…$$` outside code ranges (`link::code_ranges`); `\\(` (an escaped backslash) is left alone, and an unclosed opener stops the rewrite.
2. `fence_html_charts` wraps raw HTML charts in a chart fence: a line (indent at most 3 spaces) that opens one of `figure, div, svg, table, section, canvas, script, style, iframe` starts a chart that runs to the line where that element's depth returns to 0 (`chart_end`, counting `<tag` and `</tag` per line; a first line ending `/>` closes itself; an element that never closes ends at the first blank line). Charts separated only by blank lines join into one (a `<div>` and its `<script>`). Lines inside existing fences are skipped. The fence uses one more backtick than the longest run inside.
3. `pulldown_cmark` parses with tables, strikethrough, task lists, footnotes, and math. Events are rewritten:
   - a fenced block whose info string is `chart` or `html … chart` (`is_chart_info`) collects its text; `chart_page` builds a page, `Charts::add` stores it, and the event becomes `<figure class="zt-chart"><iframe class="zt-chart-frame" data-chart="<id>" loading="lazy" title="Chart"></iframe></figure>`;
   - an image whose URL starts with `artifact:` becomes `figure_html`: `<span class="zt-figure" data-artifact="<digest>"><img src="/artifact/<digest>" …><span class="zt-figcaption">…</span></span>`; a digest that `artifact::is_digest` rejects becomes `data-artifact=""`;
   - a link whose URL starts with `scholar:` opens with `document_link_open`: `<a class="zt-doc" href="/scholar/file/<id>[#frag]" data-doc="<id>">`, or `<a class="zt-doc broken">` when `scholar::document_target` rejects it;
   - prose text is buffered and flushed through `link_events`, which uses `lifecycle.classify_for_rendering` to turn each `[[target]]` into a valid chip `<a class="zt-link" href="#/card/<pct>" data-target="…"><span class="zt-link-addr">…</span><span class="zt-link-title">title</span></a>` or a broken chip `<span class="zt-link broken">…missing</span>`; text in code blocks is never linked;
   - plain inline tags without attributes (`sub sup br kbd mark small u ins del s b i em strong`) pass through lower-cased; HTML comments vanish; every other raw HTML is pushed as text, so `push_html` escapes it.
4. `$…$` and `$$…$$` arrive as `<span class="math math-inline|math-display">`; the page typesets them with KaTeX (`typeset` in `app.js`).

### Chart pages

1. `chart_page(source, snapshot)` collects each `[[target]]` in the source (non-empty, under 200 bytes, no newline or bracket) with its title and existence into a JSON object, escapes `</` in it, and writes a template: `<html data-theme="__ZT_THEME__">`, a KaTeX placeholder `<!--zt:katex-->` only when the source holds `$`, `\(`, or `\[`, `<script>window.ZT_LINKS = …</script>`, a frame placeholder `<!--zt:frame-->`, and the source inside `<div id="zt-chart">`.
2. `Charts::add` keys the page by the first 16 bytes of its SHA-256 (32 hex characters) and keeps at most 512 pages, dropping the oldest inserted first; adding an existing page keeps its place.
3. The page sets each iframe's `src` to `/chart/<id>?theme=<light|dark>` in `wireCharts` (on every render and on theme toggle).
4. `/chart/<id>` fills the theme, inlines `chart-frame.js` and, when the placeholder is there, KaTeX's JS and its CSS with every font as a `data:font/woff2;base64,…` URI (`katex_inline_css`, built once), and answers with the chart CSP.
5. In the frame, `chart-frame.js` adds the palette, turns `[[…]]` into links, typesets TeX, and reports its height on load, on resize, and at 300 ms and 1500 ms; clicks post `go`.

### Page boot, refresh, and heartbeat (`app.js`)

1. `boot()` installs the theme, steady hand (no text selection during drags), pan/zoom, command bar, keys, buttons, and history; sets the view mode from `localStorage`; then fetches `overview`, `graph`, and `spreads` in parallel, indexes the nodes (`setNodes`: `byAddress`, `inboundOf`, `childrenOf`), and calls `route()`.
2. `route()` reads `#/card/<address>` into `state.pointer`, records the step in `nav`, closes the search tray when the Pointer moved, and calls `loadPointer()`, which fetches `/api/card`, switches the Workspace to the Card's, fetches `/api/tree` when the tree changed (building `state.outline`), and calls `renderAll()`. A Card that no longer exists toasts and falls back to its nearest known ancestor, or keeps stepping in the history direction (`nav.skipMissing`).
3. `watchForChanges()` (every 4 s and on focus, skipped while hidden or editing) compares the overview fingerprint and calls `refresh()` on a change. `refresh()` flushes Spreads, refetches overview, graph, and Spreads, resets the tree, invalidates the Constellation, and reloads the Pointer.
4. `heartbeat()` (every 2 s) pings `/api/ping`. A network failure calls `serviceDown()`: the window closes, or, while a dialog holds text, a notice says the text is kept. A 403 (new token) or a service that is back reloads the page when nothing is being edited; while editing, it fetches `/`, takes the new token from the meta tag, and lets the reader save.

### A change through Card operations

Every change the page makes goes through `api()` to a write route, which calls the same `ops::*` functions as the shell and the Session, with Owner Access:

1. Before any POST except `spreads` and `render`, `api()` awaits `spreads.flush()`, because the server rewrites the Spreads it holds when Cards move or are deleted and must hold the latest ones first.
2. Create: `newCard(kind)` opens the writing desk with the terminal's template (`# <title>\n\n…\n<!-- zt:reverse-links -->\n`); Save posts `/api/create`; `afterChange` toasts, refreshes, and goes to the new address.
3. Edit: `editCard()` posts `/api/edit` with `base: card.text`, so a Card changed elsewhere since it was opened is refused. A Literature Card first asks whether to edit the text or the BibTeX (`choosePart`); metadata goes to `/api/metadata`, and a Citation-key rename returns a plan that `confirmPlan` shows before the confirmed retry.
4. Delete, move, insert are two-phase: the first call without `confirm` returns `{confirm, lines}`; `confirmPlan` shows the lines and asks the reader to type the word (`delete`, `move`, the tree root address, …), exactly like the CLI's type-to-confirm; the second call carries `confirm`. Insert confirms the renumbering first, then opens the writing desk (inside a model, with facet `推导` and emphasis `aside` preset) and sends the text with the confirm word.
5. `ty`, `lv`, `mf` post `/api/edit` with only the type, level, or form field.

### Writing desk (`openEditor`)

1. A dialog with a Markdown textarea (and a BibTeX textarea for Literature Cards), a type picker for Regular Cards, and for Model Cards a free facet field with the common facets as a datalist, an emphasis select (parts) or a level select (whole models).
2. Every edit debounces (160 ms) a `POST /api/render`; the preview shows the title (from BibTeX for Literature Cards), the stamp, and the server HTML, and the message line shows a parse or Link error.
3. `[[` opens Link completion over `state.nodes` (address or title contains the typed text, 8 at most); Enter or Tab accepts.
4. "＋ Figure or document" opens `pickFigure`: search Works (prefilled with the current Literature tree's title), list the Work's documents and figures, preview thumbnails one after another (`/api/scholar/preview`), and insert either a Document link or, after `/api/scholar/artifact` keeps the image, its artifact Markdown.
5. `Ctrl+S` saves; Esc or Cancel asks before discarding a changed text.

### Reply (`src/reply.rs`)

1. Reply is an Owner action of the reader, offered on an agent Card: the reader button, `r`, and the command `reply`. It captures the target's address, title and `authored_fingerprint`, and opens the writing desk with an empty claim title and the body `印证 [[target]]：` (`ZtDraft.replyDraft`).
2. The dialog's reply row offers the four relation words (印证 → `illustrates`, 反驳 → `objects`, 放宽 → `relaxes`, 对应 → `corresponds`) and the destination Topic. Changing the relation calls `ZtDraft.replySwapPrefix`: it replaces the supplied prefix wherever it stands in the draft — or puts it back at the start of the body, after the title, when the reader removed it — and leaves the title and every composed sentence as it was.
3. `POST /api/reply` re-reads the target in one write transaction: the Card must still be an agent Card with the same authored fingerprint, the reply body must carry a kinded Link to it, and it must hold at least eight letters or digits of the reader's own thought beyond that Link and its relation word (`MIN_SUBSTANCE`). A refusal answers 200 with `saved:false` and the reason, so the page keeps the draft; on `target_changed` it also shows the fresh title and takes the new fingerprint, so the reader's renewed Save is checked against the Card they now see.
4. The desk starts on the human Topic the reader last replied in while it still exists — even when a Topic titled `回应` exists — else on that default Topic, else on creating it (`ZtDraft.replyDestination`). The route resolves the destination again at save time; when there is none, a human Topic titled `回应` is created in the reply's own transaction. The reply is filed as a Side successor of the Topic's last Regular Card in Folgezettel order, or the first Direct successor of an empty Topic. A stale address falls back rather than following whatever now occupies it; a Save that fails after the Topic was created rolls that Topic back with it.
5. After the write, the page stores `zt-reply-topic` — only on success — toasts, refreshes and opens the new human Card. The agent Card keeps its authored text and gains the reply in its generated Reverse section; nothing is appended to it.
6. The operation is a GUI route with Owner access and the ordinary GUI token: there is no MCP tool that writes a human Card, and the agent tools' human-Workspace and human-Link refusals are unchanged.

### Views

- **Cabinet and desk.** `renderRail` lists the current Workspace's trees as drawers by Zone; `renderDesk` draws each tree as a stack (up to four sheets by size), with stamps and tallies for Event, Finding, and Model Cards and a gauge relative to the largest tree, plus a "new" stack per Zone.
- **Search tray** (`finder`). Debounced (110 ms) `/api/search`; the default `Entry points` order is `mode=entry`, the same ranking the MCP server serves ([Hub Cards](hub-cards.md#entry-first-search)): one flat ranked list, never regrouped by tree, with a role badge on each row (`entry · term`, `hub line`, `address`, `model`, `title`), the explanation under an entry or member row, and a note when the register is missing or has faults. A row that folds a confirmed restatement ([Hub Cards](hub-cards.md#restatement-folding)) draws `+N more stating the same statement`; opening it lists every member under the row as a slip of its own — address, title, and what it is (`statement`, `entry`, `same statement`, plus `matched`, `filtered out`, `this row`) — and any member opens like a slip (S14, #108). A row whose Card is a matching member of a statement it does not hold names that Card on a `canonical <address>` button. The counts name rows and Cards apart (`3 rows · 9 cards`, `best 1 of 3 rows · 9 cards`), because a folded row stands for several Cards. `All matches` switches to `mode=plain`, the existing `Snapshot::find` order grouped by tree behind guide cards, three slips per tree and six trees before a `+N` fold. Workspace and type chips with counts in both modes; the selected slip's full Card loads beside the tray (`/api/card`, cached per opening); Enter flies the card to the reader, Shift+Enter lays it on the Table. With no words it lists the last visited Cards (`nav.recent`).
- **History** (`nav`). Every Pointer move is one browser history entry carrying `ztNav: <index>`; the trail of addresses is kept in `sessionStorage` `zt-nav`, so the arrow buttons can name their step and list their steps on right-click or hold.
- **Folgezettel map** (`map`, `ZtLayout.focusLens`). Shown: the root-to-focus path, every outline child of each Card on it, the focus's grandchildren when it has at most 14 children, and the focus's in-tree Link targets with their paths (or every Card with `x`). The focus is a `focus` card, its path, children (at most 14), and run neighbours are `near` cards, the rest are `slip`s; hidden outline descendants fold into a `+N` badge. Columns: depth − 1 (the root's Direct run shares its column); each column is as wide as its widest shown card. Edges follow addresses (a Direct successor hangs `down`, a Side successor branches with an `elbow`), the address chain to the focus is `hot`. Only the Pointer's Links are drawn as arcs (outbound always; inbound only when at most 16), plus the hovered card's. Each Pointer move re-runs the layout and glides cards (360 ms) from where they were; a new card slides out from its nearest shown ancestor. Shift-click lays a card on the Table.
- **Table** (`tableView`, `spreads`). Cards lie at a fixed per-address angle (±1.7°); the held Card (the Pointer, when it lies there) is squared, scaled, and on top; Cards it relates to (`relatedTo`: its Links, Cards linking it, its parent, its successors) keep ink, the rest recede. Threads (`ZtLayout.thread`) join Cards on the Table that link each other, styled by Relation kind and labelled when hot or warm. "Within reach" lists the held Card's neighbours not on the Table (kinded Links first, then plain Links, then its tree), placed in the nearest gap beside it; the distant Card's slot sits under those slips, and the box appears for the distant Card alone when the held Card has no neighbour left (S19, #108). Drag to move (the card swings with the hand), drop on the box to put it back, click to open. Arrange: Gather (`ZtLayout.gather` around the held Card, nearest cells first), Line up (`readingOrder` then `lineUp`), By tree (`byTree`). "File as card" writes a new Card (a Side successor of the held Regular Card, else a Direct successor) listing the Spread's Cards in reading order as `- [[address]] title`, and lays it on the Table. Every Spread change is saved 450 ms later (`spreads.changed` → `flush`). A question's proposal lies on the same Table as a **temporary** one (S16, #116): the answer's held Card is squared up and lifted, each proposed Card shows the reason it was chosen in place of its excerpt (`.why`), the bar above the Table names the question and the matched terms with Keep and Cancel, and no neighbour slips, distant slot, box, put-back, "File as card", or Spreads picker is offered until the reader keeps or cancels it.
- **Constellation** (`sky`, `ZtLayout.constellation`). Canvas drawing: trees ordered human before agent, Topic before Literature, numerically; each tree a radial outline where leaves share the circle; trees around one ring, each taking an arc as long as its diameter; Links between trees bundled into one arc per pair, width by `log2(1+count)`. The Pointer's tree (or, at ROOT, the current Workspace) is lit; only the Pointer's and the hovered Card's single Links are drawn. Click a dot opens it, click a disc opens the tree root.
- **Reader** (`renderReader`). The Card as an index card: model tabs (`modelTabs`, in build order) or a hub guide tab, meta (kind, Workspace, type stamp, address stamp that copies), the hubs that list it (`card.hubs`), title, body; below it the theory board or model board, BibTeX, Referred by and Links out chips (cross-Workspace chips marked), the distant Card's slot, actions, and the stored Markdown.
- **Model board** (`modelBoard`). Rows by `rests-on` depth (a Link closing a cycle is left out of the depth), within a row under the mean slot of what a part rests on; Loose parts in a tray; Derivation parts as slim slips; threads drawn after layout and on every resize (`ResizeObserver`); attention levels `here`/`near`/`mid`/`far`; reasons of the open part's relations; "Beyond the model" neighbours (six, then folded); other models a shared part serves; `buildFaults` lists `closure`, `forward`, `unanchored` faults with their fixes.
- **Theory board and study desk** (`theoryBoard`, `study`). The theory board lays a Theory Card's units by `ZtLayout.theoryRows`, inks by degree of interest, puts tools from other theories and the theories that use it on the edges, and in Review mode lays main results face down. The study desk opens with `s` on a unit or Theory Card: one station of `math.study` at a time (`studyStations` / `theoryRun`), the focus in the centre, up to three companions, Derivation slips below, the rest in a `+N` stack, cited premises as tool tabs that pull a unit half out in Look up mode (`pullOut`), a lemma studied in place (`study.enter` / `leave`), FLIP motion between stations. Label chips (`chipLabels`, `wireLabels`) lift the Cards that use a hypothesis or clause. Card reads go through `cardAt`, a promise cache cleared when the fingerprint changes.
- **Figures.** `wireMarkdown` marks a figure whose image fails as "figure not in this archive"; clicking a figure opens `showFigure`, which asks `/api/artifact` and links "open it in the book" to the reading page anchor.

### The distant Card's slot (`distantSlot`, `paintDistant`, `requestDistant`)

1. `renderReader` and `renderReach` each place one empty `div.distant-slot[data-distant]`; `paintDistant` fills both from one cached answer and hides a slot that has none. Nothing else redraws a view when an answer arrives, so the Card held keeps its place and motion.
2. `requestDistant` asks `GET /api/distant?at=…&table=…` once per read: the key is the Card's address, the archive fingerprint, the Table's Cards, and the selected Bridge report's path. A repaint, a hover, or the other view reuses the answer. A tree root asks nothing (it is navigation and has no slot).
3. `distantBody` draws the slot: a `Distant` label and its source (`register term`, `model name`, or `bridge report`), a title button that opens the Card, one line of evidence, and `Lay on the Table` plus, for an agent Card, `Reply`. `Reply` opens the ordinary human draft for that Card (`replyTo(candidate)`) and does not move the reader.
4. `.distant` stays at 58% opacity with a dashed rule until `:hover` or `:focus-within`; it has no shadow and no animation, so it never competes with the held Card's ink or lift. Its buttons are ordinary buttons, in reading order, with a visible focus ring.
5. The Table keeps its box (`#reach`) visible for a distant Card alone, and `spreads.changed` asks again with the new Table's Cards, because a Card on the Table is one of the selection's exclusions.
6. On a service restart the token changes as for every route; the page already handles that. A refused read (a Card that vanished) leaves the slot hidden and the reader otherwise unchanged.

### The optional Bridge report (`useBridgeReport`, `bridgeStatusSlot`, `Projections`)

1. `useBridgeReport` (the reader's `Bridge report` action) asks for the exact path of one existing `proposals.json` and keeps it in `sessionStorage` under `zt-bridge-report` — the browser session, never the archive. An empty answer clears it; the small status line carries the clear action. No folder is listed and no "latest report" is guessed.
2. `requestDistant` adds `&report=<encodeURIComponent(path)>` while a report is selected. `Projections::read` (`src/gui.rs`) validates the path (absolute, an existing file, at most 16 MiB, at most 4 096 characters), hashes its bytes, and keys the answer by that hash, the archive revision, and the Card. A key already answered is reused; a key in flight reports `pending` instead of starting a second run, so a repaint or hover starts no helper.
3. The first read of a key starts `zt-digest bridge -Step project -Report <path> -At <card>` in a background thread with that fixed argument vector (the path is one argument, never a shell word) and a ten-second lifetime on its own temporary output files. Past the lifetime the child is killed and reaped and the answer is `timeout`. `ZT_DIGEST_HELPER` names the binary exactly (tests use it); otherwise it is `zt-digest` on `PATH`.
4. Only the helper's successful projection supplies candidates: at most twenty current proposals, each with its report id, proposal id, target, and Relation kind. They go into `entry::distant` as its first candidate source; the pure function opens no report. A `pending` answer, a failure (`missing_report`, `missing_helper`, `invalid_report`, `timeout`, `failed`) or a successful projection with no current proposal (`stale`, `not_in_report`) leaves the ordinary term and model evidence in place.
5. `bridgeStatusText` and `bridgeStatusSlot` draw one faint line under the slot (`Bridge report r-…: 2 current proposals`, `Bridge report is not a valid immutable report: …`, `Bridge report helper took longer than ten seconds; showing term and model evidence.`) with a `×` that clears the selection. `scheduleBridgePoll` asks once more after 700 ms while the status is `pending`; nothing is polled when it is settled.
6. Nothing here applies a Bridge. Opening or laying the candidate writes no Link, and acceptance still runs `zt-digest bridge -Step apply` with the exact report id and proposal ids, the distinct earlier-carrier selection, and its guarded apply; the status line's or the slot's click is never an accept-all.

### Source documents (`document.rs`, `doc.js`)

1. `/scholar/file/<id>` (id > 0) asks `Bridge::document(file_id)` for the document's name, kind, relative path, and local path.
2. A Markdown document (`kind == "markdown"` or `.md`) is read from disk and set by `reading_page`: a head with the file id, name, and path, the KaTeX stylesheet, `READING_CSS` (light/dark by `prefers-color-scheme`), the body from `render_document`, and the scripts `/vendor/katex/katex.min.js` and `/doc.js`.
3. `render_document` parses with tables, strikethrough, footnotes, and math (no task lists); each relative image is resolved below the document's folder (`resolve`: percent-decoded, no scheme, not absolute, `..` may not climb above the archive's `files/` root) and becomes `<img id="fig-<stem>" src="/scholar/figure/<file_id>?path=<path>" loading="lazy">`. Other images are left as they are. Raw HTML passes through, but `READING_POLICY` allows only this server's scripts, so it cannot act.
4. `doc.js` typesets math spans as they near the screen (`IntersectionObserver`, 1200 px margin) and centres the `#fig-…` target, again when its image loads, when the page loads, and on every hash change.
5. A PDF is streamed inline (`Response::from_file`); other files download as attachments.

### Spreads (`src/spread.rs`)

1. `normalize` requires `spreads` to be a list of at most 64; each needs a unique non-empty `id` (≤ 40 characters), a `name` (trimmed and cut to 80 characters), an optional `question` (trimmed; blank or `null` is absent; at most 512 characters, else the document is refused), and `cards` (≤ 400) of `{at (non-empty, ≤ 512), x, y}`.
2. A Card listed twice keeps only its last entry (the one on top); coordinates must be finite, are clamped to ±1e6 and rounded to 0.1.
3. `current` must name a kept Spread, else it becomes the first Spread's id (or `null`).
4. In the page, `spreads.take(kept)` replaces the local document only when nothing is waiting to be saved; `spreads.cards()` hides Cards that no longer exist.
5. A Saved Spread with a `question` is the archive's only durable trace of the question: `load`/`save`/`follow` keep it, and a legacy document without the field loads and saves unchanged (normalization adds no field). **Binary rollback risk:** an older binary does not know `question`, so its next `POST /api/spreads` normalizes the document without it and drops every kept question; the Cards, the Spreads, and their places are unaffected. A rollback that must keep the questions needs that binary rebuilt with this field.
6. Only the display `name` is cut to 80 characters (by characters, not bytes); the whole question stays in `question`. A longer `name` is therefore truncated rather than refused — the one place the page deliberately shortens a name instead of reporting an error.

### Ask the box (S16, #116; `askBox`, `keepAskSpread`, `cancelAskSpread`, `src/gui/ask.js`)

1. The command is `ask [<question>]`, the Table's `Ask the box` button, and the `Ask` dialog when no question is typed (an input of at most 512 characters; Enter asks; Escape closes the dialog and consumes that Escape, so it does not also reach the page). `ZtAsk.askQuestion` checks the question exactly as `AskArgs::from_args` does, and the dialog shows its error in place.
2. `askBox` posts `POST /api/ask` and decks one temporary Table (`ZtAsk.askPreview`) with the proposed Cards, their suggested places, the answer's held Card, the matched/ambiguous terms, and each Card's reason. The stage title reads `Ask · N proposed cards on a temporary Table`, and the bar names the question, the matched terms, and any `low confidence` (bigram fallback), `sparse` (fewer than five Cards), or `bounded` (the walk hit its limit) news. Keep is disabled when the answer justified no Card. The bar is a child of the stage bar, in the room the title and the Table's tools vacate while it shows (`#stage-title` and `#table-tools` are hidden in preview), so it never covers the view tabs or the zoom; text it cannot fit is ellipsized with its full text in `title`.
3. A **failed** question (400/403/500) toasts its reason and leaves the Table, the saved Spread document, and every Card exactly as they were; the proposal is page state only, so a repaint, a second question, or another Card opened does not mutate Cards either.
4. **Cancel** (the bar's Cancel, `Esc`) drops the preview; the saved document was never touched, so the previous Spread comes back on the Table unchanged.
5. **Keep** (`Keep`, `y`) first re-reads every proposed endpoint with `GET /api/card` and compares its `authored_fingerprint` with the proposal's (`ZtAsk.askChanged`). Any changed or vanished Card is shown with its fresh title (and the changed Cards are shown with the refreshed Table) before the reader may `Keep anyway` / `Keep the rest`; a new occupant of an old address is a change, never silently the Card the question selected, and a vanished Card leaves the Spread. The dialog's `Back to the Table` and its `Esc` both return without keeping (and consume that Escape); only `Keep anyway` proceeds. Only then does `ZtAsk.askKeep` build the document and `spreads.adopt` post it once to the existing `POST /api/spreads`; the reader is told the Spread was kept only after the archive answered. A failed save leaves the Spread on the Table to be saved with the next Table change.
6. While a proposal lies on the Table the Table is a closed set: `pull`, put-back, the box, "File as card", the Spreads picker, and the neighbour/distant box are refused (each says to Keep or Cancel first), while moving a proposed Card is allowed and its place is what Keep saves. The held Card stays the visual focus: the proposed Cards carry `.proposed`, and each reason sits in a readable band along its Card's bottom edge over the ruled paper (its link counts stand down), in both themes. The bar is one slim row of the stage bar rather than a dialog over the Cards.
7. The MCP server has no keep route: `ask_box` still returns the unsaved proposal only, and it cannot replace the user's saved Table (S16-AC06).

### Rules the code keeps

- Every request is answered by one thread in arrival order, except Scholar documents, which run on their own threads; a slow handler blocks the rest of the API.
- A panic in a request is caught; the client sees the connection close without an answer.
- The token never reaches a chart: chart pages run in an opaque origin (CSP `sandbox allow-scripts`), `connect-src` allows only `https:`, and the API answers no CORS preflight.
- Artifacts and Scholar figures are served sandboxed so an SVG opened on its own cannot script the GUI origin.
- The page never writes to Spreads while a Card write is in flight out of order: writes wait for `spreads.flush()`.
- Saving an edit carries `base`, so a Card changed by the CLI, a Session, or the MCP server since it was opened is refused, not overwritten.
- A write that needs confirmation never happens on the first call.

## Data and storage

### `<archive_root>/zt.gui`

One line, the URL of the running GUI; present only while it runs.

```text
http://127.0.0.1:4717/
```

### `metadata` row `gui_spreads`

`src/spread.rs` stores the Spread document as JSON text in the `metadata` table under the key `gui_spreads` (`INSERT … ON CONFLICT(key) DO UPDATE`). The schema of `metadata` belongs to [Service, CLI and storage](service-and-cli.md).

```json
{
  "current": "lq3x9k2ab1",
  "spreads": [
    {
      "id": "lq3x9k2ab1",
      "name": "Firms",
      "cards": [
        { "at": "1/2", "x": 10.0, "y": -5.0 },
        { "at": "1/1", "x": 300.0, "y": 40.0 }
      ]
    },
    {
      "id": "m1x7c0d3ee",
      "name": "为什么学习可能造成多重均衡？",
      "question": "为什么学习可能造成多重均衡，它和策略互补是什么关系，为什么多个结果都能自我维持？",
      "cards": [
        { "at": "161/1", "x": 0.0, "y": 0.0 },
        { "at": "254/1", "x": 266.0, "y": 0.0 }
      ]
    }
  ]
}
```

The last Card in `cards` lies on top. The page makes ids from `Date.now()` and random base-36 digits. `question` is optional and appears only on a Spread kept from a question (S16, #116); a document without it is a legacy one and loads unchanged.

### In-memory caches (lost on restart)

- `Snapshots` — one `Cached { conn, version, snapshot, overview }` (see Behaviour).
- `Charts` — up to 512 chart pages (`CHART_CAPACITY`) by 32-hex id, insertion-ordered. A page asks for an expired id and gets 404 `chart expired; reopen the Card`; reopening the Card renders the body again and stores the page again.
- `katex_inline_css` — the KaTeX stylesheet with fonts as data URIs, built once per process.

### Browser storage

| Key | Store | Value |
|---|---|---|
| `zt-mode` | `localStorage` | `map`, `table`, or `constellation` |
| `zt-workspace` | `localStorage` | `human` or `agent` |
| `zt-expand` | `localStorage` | `1` when the Map shows the whole tree |
| `zt-theme` | `localStorage` | `light` or `dark` (absent: follow the system) |
| `zt-reading` | `localStorage` | `learn`, `review`, or `lookup` |
| `zt-theory-fold` | `localStorage` | `1` when "In theory" is open under a unit |
| `zt-nav` | `sessionStorage` | `{trail:[address or null, …]}` of this tab |
| `zt-reply-topic` | `localStorage` | the human Topic address the last successful Reply was filed in |

Every access is wrapped in `try`/`catch` (`store` in `app.js`), so private mode works without them.

### Wire format examples

A Card summary from `/api/graph`:

```json
{
  "address": "0/1", "tree": "0", "workspace": "human", "title": "Ownership matters",
  "kind": "regular", "type": "idea", "facet": null, "emphasis": null, "level": null, "form": null,
  "head": null, "edge": "direct", "parent": "0/0", "excerpt": "body",
  "links": ["0/0"], "kinds": { "0/0": "rests-on" }, "inbound": 0
}
```

`/api/card` adds `context`, `text`, `body`, `html`, `bibtex`, `citation_key`, `model` (see [Models and math](models-and-math.md)), `hubs` (`[{address, level, title}]`), `parent`/`direct` (`{address, title, workspace}` or `null`), `sides`, `inbound` (references), `outbound` (`{address, title, workspace, broken}`), and the two derived fields the same read serves: `restatement` (the navigation role of a `## 同述入口` entry, §4.5 D086) and `statement_fingerprint` (the exact statement hash, D087). Neither is stored, and both agree with the MCP `get_card` view.

## Configuration

| Setting | Where | Default | Effect |
|---|---|---|---|
| `ZT_GUI_PORT` | environment of the process that starts the daemon (`zt up`) | unset | A port number binds exactly that port (failure: the GUI does not start). `0` lets the system choose. Unset or blank: 4717, retried 10 × 100 ms, then any free port. A non-numeric value fails `gui::start` with `invalid ZT_GUI_PORT`. Tests set it to `0`. |
| `DEFAULT_PORT` | `src/gui.rs` constant | 4717 | the preferred port |
| `CHART_CAPACITY` | `src/gui.rs` constant | 512 | chart pages kept in memory |
| `MAX_SPREADS`, `MAX_CARDS`, `MAX_NAME`, `MAX_ID`, `MAX_COORD` | `src/spread.rs` constants | 64, 400, 80, 40, 1e6 | Spread bounds |
| search limit | `SEARCH_LIMIT` in `App::search` | 80 | hits returned by `/api/search`, in either mode |
| polling | `boot()` in `app.js` | 4 s fingerprint, 2 s heartbeat | how fast other clients' changes and a stopped service show up |
| theme | `zt-theme` in `localStorage`, toggle button | system | light or dark; chart frames reload with `?theme=` |

The page, scripts, styles, and KaTeX are compiled into the binary with `include_str!`/`include_bytes!`; changing any of them needs a rebuild.

## Usage

### Find and call the GUI from a test or a script

Take the URL from `zt.gui` (or `zt gui`) and the token from the page's meta tag, then send the token on every API call, with a matching `Host`. The integration tests do it with raw HTTP (`tests/cli_service.rs` `http_request`, `gui_token`):

```text
GET /api/card?at=0%2F1 HTTP/1.1
Host: 127.0.0.1:4717
X-ZT-Token: 0123456789abcdef0123456789abcdef
Connection: close
```

```text
POST /api/create HTTP/1.1
Host: 127.0.0.1:4717
X-ZT-Token: 0123456789abcdef0123456789abcdef
Content-Type: application/json
Content-Length: <byte length of the body>

{"kind":"direct","at":"0/0","text":"# A first card\n\nbody\n"}
```

A two-phase delete (from `tests/cli_service.rs`):

```json
POST /api/delete {"at":"0/1"}                     → {"confirm":"delete","lines":[…]}
POST /api/delete {"at":"0/1","confirm":"delete"}  → {"address":"0/0","deleted":1,"moved":{}}
```

`address` is the parent to open next (or `null`), `deleted` the number of Cards removed, `moved` the compaction mapping.

### Write a chart in a Card

````markdown
```chart
<svg viewBox="0 0 120 40">
  <rect x="2" y="2" width="50" height="30" fill="var(--zt-series-1)"/>
  <text x="60" y="22">[[0/1]]</text>
</svg>
<p>Price $p$ falls; see [[0/2]].</p>
```
````

A raw block that starts with `<figure>`, `<div>`, `<svg>`, `<table>`, … works too, without the fence.

### Add an API route

1. Add an arm to `App::route` in `src/gui.rs`, for example `(Method::Get, "/api/thing") => self.thing(&param("at")?)`.
2. Write `fn thing(&self, at: &str) -> ApiResult`; read through `self.snapshot()?` (never open your own full read per request); for a write, call an `ops::*` function with `ACCESS` and return its `into_value()` when it has a confirm phase.
3. If the result is built over all Cards and the page polls it, memoize it per Snapshot as `Snapshots::overview` does.
4. Call it from `app.js` with `api('thing?at=' + enc(address))` or `api('thing', body)`; a POST automatically waits for pending Spread saves.
5. Add an integration test that starts a test service with `ZT_GUI_PORT=0` (see `tests/model_structure.rs` `Gui::start`).

### Add a command to the command bar

Add a `[syntax, description, scope]` row to `COMMANDS` and a `case` to `runCommand` in `app.js`; keep the vocabulary and messages the same as the Session's ([Terminal Session](session.md)). Add a key in `installKeys` if it needs one.

### Add a Relation kind or Card type to the views

`KINDS` (Relation kinds), `CARD_TYPES`, `FACETS`, `LEVELS`, `HUB_LEVELS`, `MATH_FORMS` in `app.js` mirror Rust enums (`RelationKind` in `src/model.rs`, `CardType`, `Level`, `MathForm`). Update them with the Rust side, add the CSS (`.type-stamp.<type>`, `.node.type-<type>`, `.thread.kind-<kind>`, dark variables in both dark blocks of `app.css`), and grep `app.css`/`app.js`/`layout.js` for the new class name first so it does not collide.

## Testing

Unit tests (the crate is the binary `zt`):

```bash
cargo test --bin zt gui::
cargo test --bin zt spread::
```

`cargo test --bin zt gui::` runs `src/gui.rs` `tests` (query decoding, the recorded URL, the writing desk's draft rules — `src/gui/draft.js` run under Node when it is installed: the desk skeleton, the prefix swap, the destination choice — the Ask rules — `src/gui/ask.js` run under Node: question bounds and the character-counted name cut, the temporary Table and its reasons, Keep's new document and untouched earlier Spreads, the changed/missing/unchanged endpoint set, the dropped Cards — every `RelationKind` id as a `KINDS` key in `app.js`, no `@keyframes` name defined twice in `app.css`) and `render_tests` (Snapshot cache invalidation by another connection, TeX spans and LaTeX delimiters, chart fences and raw HTML charts into frames, plain inline tags and comments, artifact figures and forged digests, Document links, KaTeX inlining with fonts, chart store capacity) and `src/gui/document.rs` `tests` (image path resolution, reading page and its script policy).

Integration tests (each starts a test service with `ZT_GUI_PORT=0` and talks raw HTTP):

```bash
cargo test --test cli_service gui
cargo test --test model_structure gui_renders_math_and_sandboxed_charts_and_edits_model_parts
cargo test --test card_types gui_creates_and_edits_typed_cards_and_reports_types
cargo test --test workspaces_mcp gui_creates_trees_in_either_workspace_and_reports_workspaces
cargo test --test self_sufficient_models the_gui_inserts_a_card_and_moves_one_between_two_cards
cargo test --test scholar_bridge a_card_shows_a_figure_only_after_zet_keeps_it_from_the_archive
cargo test --test register_entries a_register_projects_one_two_and_three_targets_with_bounded_bodies
cargo test --test search_rank
cargo test --test distant_card
cargo test --test ask_spread
```

They cover: the GUI runs with the service and stops with it, `zt gui` only reports, the token guard and a new token after restart, `/api/quit` is gone (400), a dead daemon leaves no `zt.gui`, the API's create/edit/delete with plans and confirmations, rendering of Links and code, KaTeX assets and fonts, chart pages and their CSP, typed Cards, both Workspaces, insert and move, artifacts, `/api/register` answering the same entry view as MCP `get_register` (addresses, order, total, revision, identity), `/api/search` `mode=entry` answering exactly the MCP `search_cards mode: entry` value (`tests/search_rank.rs`), including the register-missing fallback and the unknown-mode refusal, `/api/distant` answering the one distant Card (`tests/distant_card.rs`), and `/api/ask` answering exactly the MCP `ask_box` value (`tests/ask_spread.rs`).

`tests/ask_spread.rs` (S15, #114) drives `POST /api/ask` on a scratch archive with two books that share an accepted term and link to each other: the proposal's held Card, matched term and origin, the second Card's authored `kind`/`direction` path step, both authored threads, `sparse`, the counts and bounds, the archive revision and identity; the MCP `ask_box` and the route returning byte-identical values; the same question answering identically twice; `/api/overview`, `/api/graph`, `/api/spreads`, `/api/register`, and `/api/broken` unchanged after the read; a missing, blank, and 513-character question as a 400 with an `error` (and a tool error over MCP); the token guard still answering 403; 512 Chinese characters accepted; and `workspace`/`limit` honoured, including a human-only question with no accepted term. The read-only guarantee is proved by `ZT_DIGEST_HELPER` naming no file: a helper or model call would fail.

The same file (S16, #116) drives Keep through the real routes: `a_kept_question_survives_a_restart_and_a_failed_question_leaves_it_alone` keeps the page's own document (`keep_document`, exactly what `ZtAsk.askKeep` builds) with a 126-character question, proves the saved `name` is 80 characters while `question` is whole and the places are kept, that `ask_box` still returns the same proposal and changes no Spread, that blank/over-513-character/untokenized questions leave the saved document exactly as it was, that an edited Card's `authored_fingerprint` moves and a deleted one no longer loads (the data Keep's check compares), that the kept question survives `zt down`/`zt up`, and that a Card deleted afterwards leaves the Spread while the question stays; `legacy_spreads_load_beside_a_kept_question_and_follow_a_move_and_a_delete` proves a legacy document loads and saves byte for byte, gains no `question`, sits beside a kept question, and that the ordinary `move_card` (a `before` move that rewrites two addresses) and `delete_card` rewrite or drop the addresses while the kept question stays. The `src/gui.rs` Node tests run the shipped `ask.js` (`the_ask_question_is_trimmed_and_bounded_and_only_the_name_is_cut`, `a_proposal_becomes_a_temporary_table_with_its_reasons`, `the_temporary_table_names_why_each_proposed_card_is_there`, `keeping_a_proposal_adds_one_ordinary_spread_and_leaves_the_others_as_they_were`, `a_changed_or_missing_endpoint_is_named_before_keep`), and `the_front_end_asks_the_box_and_keeps_only_on_the_readers_choice` pins the page: the command, one save route for the kept document, the capture of the proposal, the reason line, the re-read before Keep, the changed-set dialog (with Esc as the way back, never Keep), the missing-card drop, Cancel's message, the four preview guards, `y`/`Esc` ownership behind the input and desk guards, the loaded module, the bar inside the stage row with the title and Table tools standing down for it, and the theme-variable CSS with its focus ring and the reason band.

`tests/distant_card.rs` (S19, #108) drives `GET /api/distant` on a scratch archive with two books and a register: the candidate's address, title, source, term, origin, evidence, and one-line reason; the same answer twice; the Table's Card excluded and restored; no change to `/api/graph`, `/api/spreads`, `/api/broken`, or the register's `archive_revision` after the read, and after laying the suggestion on the Table (the Spread is the only write); a Reply from the slot saving through `/api/reply` with the candidate's own `authored_fingerprint`; the token guard, the missing `at`, and a missing Card as a 400; the served page's one request and its two slots with their open, lay, and Reply actions; and the CSS rule that keeps the slot faint until focus or hover. `the_cached_gui_read_follows_changed_evidence` edits the focus and the register through `zt mcp` and proves the cached read follows the new revision. `the_front_end_offers_one_distant_card_beside_the_neighbours` in `src/gui.rs` pins the page itself: one read per Card, Table, and archive read, no rotation or telemetry, one slot in each view, a late answer that repaints only the slots, the ordinary open/lay/Reply actions, and a slot with no shadow or animation.

The same integration file (S20, #112) drives the optional Bridge report: `a_selected_bridge_report_supplies_the_distant_slot_without_applying_it` writes an immutable report identified exactly as the compiler identifies one, reads the pending answer and then the cached `done` one (`report.proposal`, `report.report`, the one-line reason), proves the read changed no Card, Link, Spread, register revision, or report byte, stales the proposal with one endpoint edit and restores it, and shows `invalid_report` and `missing_report` falling back to the term evidence; `a_missing_bridge_helper_leaves_the_term_suggestion` starts the service with `ZT_DIGEST_HELPER` naming no file and asserts `missing_helper` beside the ordinary suggestion. The `src/gui.rs` adapter tests answer through a fake run: every helper answer's status, the bound, one run per report hash/archive revision/Card with the exact fixed argument vector (a path with `&`, `;`, and spaces is one argument), no run for a missing or relative path, and the deadline that kills a stuck helper. `the_front_end_uses_one_selected_bridge_report` pins the action, the session-only selection, the encoded `report` parameter, the pending re-read, every status phrase, and that the section writes nothing.

`tests/cli_service.rs::gui_reply_writes_one_human_card_and_refuses_what_is_not_a_reply` (S18, #99) drives Reply through the real HTTP API on a scratch archive: the served `/draft.js` module, an agent Card's captured `authored_fingerprint`, the bare-prefix, unkinded-Link and missing-Link refusals (and that no refusal wrote a human Card), a Save that passes the reply checks but fails Card storage (its default Topic is rolled back with it), a Card edited between the read and the Save (`target_changed` with the fresh title and fingerprint, and the renewed Save succeeding), the empty human archive creating the default Topic `回应` with the reply as its first Direct successor although the request carries `workspace: agent` (the route writes only the human Workspace), the remembered Topic taking the next reply as a Side successor of its last Regular Card, all four relation words (印证 → `illustrates`, 反驳 → `objects`, 放宽 → `relaxes`, 对应 → `corresponds`), a reply to a Side-successor agent Card whose address carries a pipe (`[[0/1|a]]`, the desk's usual target, with its Reverse link), a vanished destination falling back to the default, a human Card refused as a target (`target_not_agent`), the missing token answering 403, a missing Card answering 400, the generated Reverse link on the agent Card, the agent Card's authored body unchanged, and `zt status --check-links` reporting no fault. The unit tests `the_reply_desk_starts_with_the_title_then_the_supplied_prefix`, `the_relation_selector_replaces_only_the_supplied_prefix`, `a_removed_prefix_comes_back_after_the_title` and `the_reply_destination_is_the_last_topic_then_the_default` run the shipped `draft.js` under Node, so the page's text rules are tested rather than described. `the_front_end_replies_to_an_agent_card_from_the_human_workspace` checks the shipped page: the four relation words, the single `replyTo` path, the `r` key restricted to an agent Card and to reader navigation (the desk, finder and input guards checked before it), the reader's Reply action, the one `api('reply'` call inside the desk's save, the browser-local destination preference read and written only after success, the default-Topic option, the fresh-fingerprint line, the `agent-card` availability, the draft module the desk uses, and that `zt mcp` has no reply tool. The human-Workspace write refusals the agent tools keep are covered by `tests/workspaces_mcp.rs`.

Gaps: the page has no browsed test harness. Its two DOM-free modules with rules to get wrong, `draft.js` and `ask.js`, are run under Node by the unit tests above (they say so and skip when Node is missing); `layout.js` exports for CommonJS but has no automated tests, and the views of `app.js` — the Ask preview's light/dark look, its focus order, and the changed-set dialog included — are checked by the CSS and page-source tests and by hand in a browser, not by a browsed test. `/scholar/file` and `/scholar/figure` are tested only through `document.rs` unit tests, not over HTTP. Spreads following moves and deletes is tested in `src/spread.rs` and `src/ops.rs` unit tests and, for a kept question, through the GUI's own routes in `tests/ask_spread.rs`. `/api/register` has no page that renders the register yet; the route is the read seam the entry view uses.

## Decisions and pitfalls

### Decisions

- **The GUI runs inside the daemon, and a window is not a Session** (since `4f0414d`, zt 0.14). `zt gui` starts nothing, `zt down` does not wait for windows, and a stopped service closes the windows. The record is written before the service record, so a service that reads as up always shows its GUI. See [docs/markdown-cards-and-gui-design.md](../markdown-cards-and-gui-design.md) "GUI".
- **Loopback, Host check, per-service token, strict CSP.** The token is a fresh random value per daemon, delivered only inside the page; the Host check blocks DNS rebinding; raw HTML in Cards is shown as text. Same design doc, "Security".
- **The same rules as the CLI.** Writes go through `ops::*`; delete, move, and Citation-key renames show the same lines and need the same typed confirmation; edits carry `base`. Same design doc, "Same rules as the CLI".
- **Charts are sandboxed by response header and self-contained.** The sandbox travels in the chart page's CSP (`sandbox allow-scripts`) instead of the iframe's `sandbox` attribute, and the page inlines `chart-frame.js`, KaTeX, and its fonts, because some embedded browsers refuse to load a frame with a `sandbox` attribute or refuse every subresource of a sandboxed page (comments on `respond_chart` and `serve_chart_page`). The opaque origin keeps chart scripts away from the GUI page and its token. See [docs/model-structure-design.md](../model-structure-design.md) "Rendering".
- **The Snapshot is cached and invalidated by `PRAGMA data_version`** (`33ecec2`): reading and parsing every Card takes hundreds of milliseconds, too slow per request; `data_version` sees commits from every other connection, so CLI, Session, and MCP writes show up on the next request.
- **Layout is linear and deterministic.** `layout.js` has no simulation; every function is linear in Cards (Table helpers at most quadratic in a few dozen Cards), so views re-lay out on each Pointer move. Attention is spent by lift, size, and ink, never blur. See [docs/table-and-spreads-design.md](../table-and-spreads-design.md) and [docs/math-cards-design.md](../math-cards-design.md) "The GUI views".
- **Spreads change no Card** and follow their Cards in the same transaction as the move or delete. See [docs/table-and-spreads-design.md](../table-and-spreads-design.md) "Spreads".
- **A question's answer is a temporary Table, never an edit of the saved one** (#116 S16, design §9.5 D232-D234). `ask` decks the proposal in page memory (`spreads.previewing`, `spreads.held`); Cancel drops it and the saved document is untouched, and Keep is the only action that writes, through the one existing `POST /api/spreads`. A failed question writes nothing. Just before Keep, each proposed endpoint is re-read and compared by `authored_fingerprint`, so a changed Card, a vanished one, or a new occupant of an old address is a visible choice (`askChangedDialog`) rather than a silent substitution. The Spread keeps the whole question in `question` (≤ 512) while its `name` is cut to 80, and a legacy document without the field is unchanged (D233, D282).
- **Hubs are guide cards**, their tab position showing the level. See [docs/hub-cards-design.md](../hub-cards-design.md) "The GUI: a hub is a guide card".
- **Search has one ranking contract and two presentations** (#93 S13, D198, D210, D278). The tray's default `Entry points` asks `mode=entry` and draws the shared ranked rows as one flat list with role badges and explanations, never regrouped by tree; `All matches` asks `mode=plain` and keeps the existing `Snapshot::find` lexical order with tree guide cards and `+N` folds. The API's omitted mode is `plain`, so an older caller keeps its order and fields. Enter and Shift+Enter act on a ranked row exactly as on a plain slip.
- **A folded restatement row is one row that opens into its members** (#93 S14, design §9.2 D207, D209, S14-AC03, S14-AC05). The tray draws the row's `folded` count as `+N more stating the same statement`; opening it lists every `members` entry with its address, title, and what it is, and each member is a slip-like row that opens that Card (`items` includes `.fslip-member`, so the keyboard reaches it). A row that names a statement another Card holds shows `canonical <address>`; a row in no confirmed group draws neither. The fold is display state of the open tray (`foldedOpen`), cleared with the tree folds by `clearFolds`, so it never needs a second request and never writes a Card.
- **The distant Card is a pencil note beside the neighbours, not a second pane** (#93 S19, design §10.2, D249, D292, S19-AC05/06). The reader and the Table each carry one slot, filled from one cached `GET /api/distant`; it stays faint until focus or hover, has no shadow and no animation, and its actions are the ordinary open, lay-on-Table, and Reply. Opening or laying writes no Link and no accepted relation, `Reply` opens the ordinary human draft for that agent Card, and the service records no view. The slot asks again only when the Card, the archive fingerprint, or the Table's Cards change (S19-AC04, S19-AC07).
- **Model boards are laid out by `rests-on` relations, not by address.** See [docs/model-structure-design.md](../model-structure-design.md) "The model board".

### Pitfalls

- **Assets are compiled in.** Every change to `index.html`, `app.js`, `layout.js`, `draft.js`, `ask.js`, `app.css`, `chart-frame.js`, `doc.js`, or KaTeX needs a rebuild and a service restart.
- **One request thread.** A slow API handler (for example a Scholar bridge call in `/api/scholar/*`) blocks every other API call; only `/scholar/file` and `/scholar/figure` run on their own threads. Those threads are unbounded.
- **Per-request work over all Cards must be memoized per Snapshot**, as the overview is (the page polls it every 4 s). `Snapshot::find` folds case once per Snapshot for the same reason.
- **The `Host` check covers static files too.** A client that sends another `Host` (a proxy, a hostname alias) gets 421 even for `/`.
- **`/api/create` with a level or form is two transactions**: the Card is created, then `ops::edit` sets the level or form; if the second fails, the Card stays without it and the client sees an error.
- **`/api/literature/check` and `/api/spreads` open their own connections** instead of using the Snapshot.
- **`/api/register` deliberately bypasses the cached Snapshot.** It calls `read::register`, which opens its own read transaction, because the entry view must name the same archive revision and identity the MCP `get_register` tool reports. It is a read-only route and changes no Card; a register page can memoize it on the fingerprint later.
- **The chart store is FIFO, not LRU.** Re-rendering a Card does not refresh its chart's place; with more than 512 distinct charts rendered, a chart still on screen can expire and its frame shows 404 until the Card is opened again.
- **The search tray and the Map share words.** The Map draws far cards as `.node.slip` and folds as `.badge.fold`; the tray uses `.fslip`/`.ffold`/`.in-hand` (`d7168d2` fixed a collision). Grep for a class name before adding one. Entry-mode ranks add `.fslip-role`, `.fslip-why`, `.finder-note`, and `.drawer-run.ranked`; they are the tray's own.
- **Entry-mode rows are one flat ranked list.** `render()` draws `data.results` in order under `.drawer-run.ranked` and never feeds them to the tree-grouping branch, which would destroy the rank (D210). The tree `+N` folds and guide cards belong to `All matches` only; entry mode has its own statement fold (`.fslip-fold-toggle`, `data-fold`) and reports `row_total`/`total` instead. The role badge reads `entry · <term>`, `hub line`, `address`, `model`, or `title`; a plain row has no `role`, so `roleLabel` must stay behind its `hit.role` guard (the "Visited last" slips take the same path).
- **`kindOf` falls back to `rests-on`.** A kind missing from `KINDS` is drawn on the model board as a `rests-on` thread and placed by it, and the Table labels it "refers to". The unit test `the_front_end_names_every_relation_kind` fails when `RelationKind` gains a kind that `KINDS` lacks.
- **CSS animation names are global.** A second `@keyframes` with the same name replaces the first for the whole page (the table hand once overrode the dialogs' `rise`; it is now `hand-rise`). `each_css_animation_name_is_defined_once` checks it.
- **The system-dark block in `app.css` lacks `--hub`.** `:root[data-theme="dark"]` sets `--hub: #8fd1ae`, but `@media (prefers-color-scheme: dark) :root:not([data-theme="light"])` does not, so with the system theme dark and no explicit choice, hub ink keeps the light green.
- **Mirrors must stay in step.** `parentOf` in `app.js` mirrors `parent_location`; `CARD_TYPES`, `FACETS`, `LEVELS`, `HUB_LEVELS`, `MATH_FORMS`, `KINDS` mirror Rust enums; the command bar mirrors the Session's vocabulary.
- **Tree roots that half the tree links** would light everything: the Map draws inbound Link arcs only when there are at most 16.
- **A restarted service changes the token.** A page with an open writing desk keeps its text and takes the new token from `/`; any other page reloads.
- **The reading page has no `connect-src`**: `fetch()` from it fails by design (`READING_POLICY`).
- **`api()` flushes Spreads before every POST except `spreads` and `render`**; a new write route that should not wait must be added to that exception, and one that moves Cards must not be.
- **The distant slot asks once per read, never per repaint.** `requestDistant` returns while the answer for the same Card, archive fingerprint, and Table is cached or in flight; use `requestDistant`/`paintDistant` rather than calling the route from a render path. The slot is a `div[data-distant]` whose contents `paintDistant` replaces, so a late answer must not re-run `renderReader` or `tableView.render` (that would drop the reader's scroll and the held Card's motion).
- **A Card on the Table is an exclusion, so laying one changes the answer.** `spreads.changed` refreshes the Table's slips and asks again; a slot action that lays the suggested Card therefore makes the slot re-check itself, and that is intended.
- **`replyTo` takes its target.** `r`, the reader's Reply button, and the distant slot's Reply all call the same `replyTo`; the slot passes the candidate object (address, title, `authored_fingerprint`), which is the same shape `/api/card` gives, so no second reply path exists.
- **The Table's reach box may hold a distant Card alone.** `renderReach` shows the box when there are neighbours *or* a distant answer, and `reachOrigin` adds the slot's height so the free-gap search never puts it over a Card.
- **The Bridge report selection is session state, not archive state.** `zt-bridge-report` is in `sessionStorage` only; no route, metadata row, or Card records it. A path the reader typed is one `encodeURIComponent` query argument and one helper argument, never a shell word; the adapter validates it before any run, so a missing or relative path costs no process.
- **A helper answer is never awaited inside a paint.** `Projections::read` starts one run per key in a background thread and answers `pending` at once; the page asks again after 700 ms only while the status is `pending`. Adding a synchronous wait here would freeze the daemon's one request thread and the page's heartbeat.
- **A selected report never re-judges or applies anything.** The helper's projection supplies current candidates only; opening, laying, and clearing write no Link, and applying still needs `zt-digest bridge -Step apply` with the exact report id and proposal ids. Do not add an accept route, an accept-all default, or a second validator here.
- **The proposal's reasons and fingerprints are response state, not archive state.** `ZtAsk.askPreview` keeps them in the page's `preview`; `ZtAsk.askKeep` writes only `{id, name, question?, cards:[{at,x,y}]}`. Never store a fingerprint, a reason, or a held address in `gui_spreads`, and never let `spreads.changed` save while `preview` is set (it returns early).
- **Keep confirms the save; the ordinary Table save does not.** `spreads.adopt` posts the document itself and reports failure, so the page never says "kept" for a document the archive did not take; a failure leaves the Spread on the Table with `dirty` set so the next Table change saves it. Do not route Keep back through the debounced `flush` without keeping that confirmation.
- **An over-long Spread name is cut, not refused.** `normalize` truncates `name` to 80 characters by characters and refuses only a `question` over 512; this is what lets a long question name its own Spread. An older binary that lacks the field silently drops every `question` on its next Spread save (rollback risk; see Spreads §5).

## Change history

- `2026-10-19` — #116 S16 follow-up, from a headless-browser pass over the real page: the ask bar was an absolutely positioned overlay at the top of the viewport, so it covered the view tabs, the Table's tools and the zoom. It is now a child of the stage bar, and while it shows the stage title and the Table's tools stand down (they are hidden in preview), so the row reads tabs · question · Cancel/Keep · zoom and nothing overlaps. The proposed Card's reason is a readable band along the Card's bottom edge over the ruled paper (its link counts stand down for it), in both themes. The changed-set dialog gained its own `Esc` = `Back to the Table`, and both new dialogs consume their Escape, so the page's own Escape (closing a pull-out or the cabinet) does not also fire as the dialog closes. Verified in a real headless Chromium over CDP on a scratch archive: 28 checks (preview, reasons, held Card, light and dark, window state, changing the question, Keep's saved question and 80-character name, Cancel restoring the very Table, the changed-endpoint dialog, Esc/Back, and no Card changed by any preview), plus the light/dark screenshots.
- `2026-10-19` — #116 S16: the page keeps a question's answer without overwriting the Table. `ask [<question>]`, the Table's `Ask the box` button, and the Ask dialog deck `POST /api/ask`'s unsaved proposal on a temporary Table (`ZtAsk.askPreview`): the answer's held Card is squared up, each proposed Card shows why it was chosen (`.why`), and a slim bar names the question, the matched terms, and the sparse/truncated/low-confidence news with Keep and Cancel. Cancel (`Esc`) drops the preview and the saved Spread document was never touched; Keep (`y`) first re-reads every endpoint and compares `authored_fingerprint` (`ZtAsk.askChanged`), shows any changed or vanished Card with its fresh title, and only then writes one ordinary Spread through the existing `POST /api/spreads` (`ZtAsk.askKeep`, `spreads.adopt`); a failed question or a failed save writes nothing and never claims success. The Spread schema gains the optional `question` (≤ 512 characters, kept in `load`/`save`/`follow`, absent on legacy documents) while `name` is cut to 80 characters (D232-D234, D282, D307, D343). The new DOM-free `src/gui/ask.js` (served at `/ask.js`) is run under Node by five unit tests and pinned in the page by `the_front_end_asks_the_box_and_keeps_only_on_the_readers_choice`; `tests/ask_spread.rs` gains the restart, failed-question, legacy, move and delete coverage. The MCP `ask_box` still returns only the unsaved proposal.
- `2026-10-19` — #114 S15: the authenticated `POST /api/ask` route returns one question's unsaved Spread proposal (D226-D228, D279). `App::ask` parses `AskArgs::from_args` (a nonblank question of at most 512 Unicode characters, a Workspace filter, and a 5-to-9 selection) and returns `read::ask` unchanged, so the GUI and the MCP `ask_box` tool answer one question with the same value from the same transaction and the same archive revision and identity. The route writes no Card and no Spread: it is the read half of the `Ask the box` feature, and the temporary Table preview, `Keep`/`Cancel`, and the Spread's own `question` field remain S16 (#116). New coverage: `tests/ask_spread.rs` (`one_question_proposal_serves_both_transports_without_a_write`, `a_question_refuses_a_blank_or_oversize_input_on_both_transports`).
- `2026-10-19` — #112 S20: a selected Bridge report supplies the distant slot's first source. The reader's `Bridge report` action asks for one existing `proposals.json` and keeps it in `sessionStorage`; `GET /api/distant` gains `report`, the exact path as one encoded argument. `Projections` (`src/gui.rs`) validates the file, hashes it, caches its answer by hash, archive revision, and Card, and runs the installed `zt-digest bridge -Step project -Report <path> -At <card>` in the background with a fixed argument vector and a ten-second cancellation, so a repaint or hover starts no helper and a stuck one never holds the reader. Current proposals become `entry::distant`'s first candidate source with the exact report and proposal id and `Bridge report r-…, proposal b003: contrasts`; a pending, missing, invalid, stale, or timed-out projection keeps the term and model evidence and shows one faint status line with a clear action. Reading, opening, laying, and clearing write nothing, and applying still needs the compiler's exact report and proposal acceptance (S20-AC01–AC08). New coverage: `tests/distant_card.rs` (`a_selected_bridge_report_supplies_the_distant_slot_without_applying_it`, `a_missing_bridge_helper_leaves_the_term_suggestion`), the `src/gui.rs` adapter tests (`a_bridge_projection_reports_every_helper_answer`, `the_bridge_projection_runs_once_per_report_revision_and_card`, `a_bridge_projection_without_a_readable_report_starts_no_helper`, `a_bridge_helper_past_its_lifetime_is_cancelled`), the page test `the_front_end_uses_one_selected_bridge_report`, and `scripts/book-digest/tests/bridge-projection.tests.ps1` for the helper.
- `2026-10-19` — #111 S14: the search tray folds confirmed restatements. An entry-mode row that stands for more than one Card draws `+N more stating the same statement`; opening it lists every member (address, title, `statement`/`entry`/`same statement`, matched or filtered out) and each member opens its Card. A matching member that does not hold the statement names the canonical Card on a `canonical <address>` button. The tray counts rows and Cards apart (`row_total` against `total`) and no longer calls a folded row a missing match. Covered by the served `app.js` assertions in `the_search_tray_renders_a_folded_row_and_every_member` and the end-to-end reply equality in `tests/search_rank.rs`.
- `2026-10-19` — #108 S19: the distant Card's slot in the reader and the Table. `GET /api/distant?at=…&table=…` serves `entry::distant` (one Card of another book that shares an accepted register term or a model name, with one line of evidence; §10.2, D280). The reader places its slot after the Referred-by and Links-out chips, and the Table places one under its "within reach" slips (the box appears for a distant Card alone). Both slots are filled by `paintDistant` from one cached answer per Card, archive fingerprint, and Table, so a repaint or hover asks nothing and a late answer moves no Card; `.distant` stays faint until `:hover` or `:focus-within` and has no shadow or animation (S19-AC05). The slot opens the Card, lays it on the Table, and — for an agent Card — opens the ordinary human Reply draft through the same `replyTo`; none of the three writes a Link, and the route writes nothing at all (S19-AC06). New coverage: `tests/distant_card.rs` and `the_front_end_offers_one_distant_card_beside_the_neighbours`.
- `2026-10-19` — #106 S13 re-verification: `GET /api/register` takes an optional `at` and answers `terms_at`/`terms`, the accepted register terms that one Card carries (its rows and the labels its own substantive title and prose state, §9.1 D196), from the same one-transaction read as the entry view. The route still bypasses the cached Snapshot, so a title, body, register row, part, or Link edit is visible on the next request; `tests/search_rank.rs` pins that for `/api/search` in both orders.
- `2026-10-19` — Re-verify #99 S18 (Reply) again on the tree that carries #100–#106 with the tool binaries rebuilt from `HEAD`: no behavioural defect found. The delivered evidence never replied to a Side-successor agent Card, whose address carries a pipe (the desk's usual target), so `tests/cli_service.rs` now proves that case end to end: the route reads `[[0/1|a]]` whole as the Link's target, saves the reply, and the generated Reverse link reaches that Card. The two module comments in `src/reply.rs` spelled the default Topic `回應` (traditional) while the code and the wiki say `回应`; they now agree.
- `2026-10-19` — Re-verify #99 S18 (Reply) on the tree that carries #100–#106: two page defects fixed, and the desk's text rules made testable. The relation selector matched the supplied prefix against the whole draft, but the desk puts the title heading first, so it never matched; the row inserted the new prefix above the title and left the old one, giving the reply an empty title and two prefixes. It now replaces the prefix where it stands, or restores it after the title when the reader removed it. The destination preference picked a Topic titled `回应` ahead of the Topic used last, so AC03's last-used Topic was not honoured; `ZtDraft.replyDestination` now chooses the last used Topic, then `回应`, then creating it. Both rules live in the new DOM-free `src/gui/draft.js` (served at `/draft.js`, loaded before `app.js`), which the Node-run unit tests execute, and `tests/cli_service.rs` now also covers all four relation words, the ignored `workspace` field, and a failed Save that leaves no empty Topic.
- `2026-10-19` — #107 S08: `/api/card` also returns the derived `restatement` navigation role and the exact `statement_fingerprint` (§4.5, D086-D087), so the GUI card view and the MCP `get_card` view stay one contract; `/api/register`'s `restatements` rows now name the `terminal` substantive Card the entry holds. Neither adds a stored field or a new route.\n- `2026-10-19` — Add Reply, the Owner's human answer to an agent Card (#99, S18; design §10.1; D238–D248, D281, D308, D344). The reader gains a `Reply` action, `r`, and the command `reply` (availability `agent-card`), all guarded so no key fires inside an input, the desk, or a modal. Reply captures the target's address, title and new `authored_fingerprint` (from `/api/card`), opens the writing desk with an empty claim title and `印证 [[target]]：`, offers the four relation words through their existing Relation kinds, and names the destination Topic. The new GUI-only `POST /api/reply` (`src/reply.rs`) re-reads the target in one write transaction and refuses a changed target, a non-agent target, an unkinded or missing Link, and a bare Link with no reader thought (`MIN_SUBSTANCE` 8); every refusal is a 200 `{saved:false,reason,detail,target}` that keeps the draft, and a `target_changed` refusal hands back the fresh fingerprint the renewed Save uses. The reply is filed in the last used human Topic (else the default `回应`, created in the same transaction), as a Side successor of the Topic's last Regular Card or the first Direct successor of an empty Topic; the browser remembers `zt-reply-topic` only after a successful save. The agent Card keeps its authored text and gains the generated Reverse link. No MCP tool writes a human Card. Public coverage: `cargo test --test cli_service gui_reply` and the front-end unit test.

- `2026-10-19` — #93 S13 (#106): the search tray ranks entry points first. `GET /api/search` takes `mode` (`entry` or `plain`, default `plain`, plus `mode` in both replies); `mode=entry` returns `entry::search`, the same value MCP `search_cards mode: entry` returns, and the page asks it by default. The tray gains an `Entry points` / `All matches` switch: entry mode is one flat ranked list with role badges (`entry · term`, `hub line`, `address`, `model`, `title`), the explanation under entry and member rows, and a note for a missing or faulted register; `All matches` keeps the existing lexical order, tree guide cards, and `+N` folds. Enter and Shift+Enter work on both. The API's omitted mode stays plain, so an older caller keeps its order and fields.
- `2026-10-19` — #93 S10 (#101): `GET /api/register?ws=` serves the concept register's accepted entry view from `read::register` / `src/entry.rs`, the same contract as MCP `get_register` (every target of every term with its bounded body, origins, faults, `entry_offset`/`entry_limit` paging, archive revision/identity; an absent register answers `register_missing`). No page renders it yet.
- `2026-10-16` — `KINDS` gains `specializes` (`.thread.kind-specializes`), so a `特例` Link is no longer drawn as `rests-on`; the table hand's animation is `hand-rise`, so dialogs, menus, and toasts get their `rise` again; unit tests for both.
- `2026-10-16` `753bd7b` — Finding Cards: the `finding` type in the type picker, stamps, search chips, and desk tallies (zt 0.20.0).
- `2026-10-15` `2597c6c` — Study a whole theory on the desk (`theoryRun`), and a cited lemma inside its step.
- `2026-10-15` `1e67fdc` — GUI views for mathematical units: theory board, study desk, label chips, tool tabs; `theoryRows`, `studyStations`, `labelsIn` in `layout.js`.
- `2026-10-15` `9918ba6` — Math form on Model Cards: `mf` command, `form` in summaries, stamps.
- `2026-10-15` `6b36918` — Book figures and Document links: `document.rs`, `doc.js`, `/scholar/file`, `/scholar/figure`, `/api/scholar/files`, "open it in the book" (zt 0.18.0).
- `2026-10-15` `33ecec2` — Keep the GUI's Snapshot until the Cards change (`Snapshots`, memoized overview).
- `2026-10-15` `1220aec`, `d7168d2` — Search tray and back/forward history (zt 0.17.0); the tray's classes kept off the Map's cards (zt 0.17.1).
- `2026-10-14` `82eac86` — Hub Cards as guide cards, hub chips above titles, hub levels (zt 0.16.0).
- `2026-10-13` `1bec685` — Derivation chains in model tabs and the build report (zt 0.15.0).
- `2026-10-13` `4f0414d`, `4b28237` — The GUI runs inside the service; `zt gui` only reports; heartbeat and new-token handling (zt 0.14.0); drags select no text (zt 0.14.1).
- `2026-10-12` `7a878da`, `7d636e4` — Insert between Cards, build faults, `alone`/`before` moves (zt 0.12.0); elaboration levels in the editor and stamps (zt 0.13.0).
- `2026-10-12` `45552c6` — The Table and Spreads (`spread.rs`, `/api/spreads`, summary `kinds`) (zt 0.11.0).
- `2026-10-11` `eec0940`, `2b05ba6`, `d92f367`, `ea9a8f0` — Model parts, math and HTML charts (zt 0.6.0); author-designed model board (zt 0.7.0); artifacts in Cards (zt 0.9.0); open models and Link kinds (zt 0.10.0).
- `2026-10-10` `d52b4ae`, `09f7e46`, `f9dea67` — Workspaces in the GUI; Event and Model Cards; focus lens for the Map and Constellation.
- `2026-09-30` `316b454` — Markdown Cards and the first `zt gui` slip-box interface; `2026-10-01` real index-card look.
- `2026-10-16` — Wiki doc created.
