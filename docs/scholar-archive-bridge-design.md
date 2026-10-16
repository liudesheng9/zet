# The ZET bridge

This specification is shared: Scholar Archive keeps it as `docs/zet-bridge.md` and ZET as `docs/scholar-archive-bridge-design.md`. Keep the two copies identical.

The ZET bridge is the one connection between Scholar Archive and ZET (`zt`, github.com/liudesheng9/zet).
Through it, ZET reads everything the archive holds: scholars, Works and their sources, versions and
topics, journals, workspaces, source history, the linked PDF/Markdown documents and their artifacts
(the figure images the Markdown shows), any table through read-only SQL, and whole exports. The exchange is **information only**: nothing on the bridge can change
the archive, start collection, contact a provider, or write a file in the archive folder.

Each project has its half of the connector:

| | Scholar Archive | ZET |
| --- | --- | --- |
| Connector | `archive/bridge.py`, served at `POST /bridge/v1/{action}` | `src/scholar.rs` |
| Pairing | `scholar-archive bridge pair\|status\|revoke` | `zt scholar pair\|unpair` (and automatically on first use) |
| Credential | `%LOCALAPPDATA%\ScholarArchive\bridge.json` | `%APPDATA%\zt\scholar-bridge.json` (`ZT_CONFIG_DIR` overrides) |
| Use | | `zt scholar ...` commands and the `scholar_archive_*` tools of `zt mcp` |

## Only the authorized ZET connects

- **Pairing.** `scholar-archive bridge pair --client zet` authorizes ZET: it creates a random 256-bit
  secret and a key ID, keeps them in the archive's per-user state folder, and prints them once as JSON.
  ZET runs this command itself when it has no credential and stores the result in its own config folder.
  `zet` is the only client name the archive accepts; pairing any other name is refused. Pairing again
  returns the same credential; `--rotate` issues a new secret, and `revoke` removes it. Both take effect
  at once without restarting the server.
- **Signed requests.** Every request carries `X-Zet-Bridge-Client` (`zet`), `X-Zet-Bridge-Key`
  (the key ID), `X-Zet-Bridge-Timestamp` (Unix seconds), `X-Zet-Bridge-Nonce` (32–64 lowercase
  hex characters, fresh for each request) and `X-Zet-Bridge-Signature`: the lowercase hex
  HMAC-SHA256, keyed by the secret's bytes, of

  ```text
  ZET-SCHOLAR-BRIDGE-1\n<METHOD>\n<path?query>\n<timestamp>\n<nonce>\n<sha256-hex of body>
  ```

  The archive answers only a paired client whose key ID matches, whose timestamp is within 120 seconds
  of the server clock, whose nonce it has not seen, and whose signature matches (compared in constant
  time). Anything else gets `401` and no information. Requests that carry an `Origin` header (web
  pages) are refused with `403`, and the server still accepts only `127.0.0.1`/`localhost` hosts.
- **Signed answers.** Every answer to an authenticated request, errors included, carries
  `X-Zet-Bridge-Signature`: the HMAC of

  ```text
  ZET-SCHOLAR-BRIDGE-1 RESPONSE\n<nonce>\n<status>\n<sha256-hex of body>
  ```

  ZET refuses any answer whose signature does not match, so it knows the information came from the
  archive it paired with. Both projects test the same known-answer vectors
  (`tests/test_bridge.py`, `src/scholar.rs`).

## Actions

All actions are `POST` with a JSON object body.

| Action | Body | Answer |
| --- | --- | --- |
| `hello` | `{}` | archive UUID, database path, generation, the read tools with their input schemas |
| `call` | `{"tool", "arguments"}` | `{"tool", "result"}` from one of the archive's read MCP tools |
| `sql` | `{"sql", "params"?, "limit"? (1–10000, default 500), "database"? ("archive" or "vectors")}` | `{"columns", "rows", "truncated"}`; BLOBs come as `{"base64": ...}` |
| `file` | `{"file_id"}` | the document's bytes, with `X-Zet-Bridge-File-Sha256` and URL-safe base64 `X-Zet-Bridge-File-Name`/`-Path` |
| `artifact` | `{"file_id", "path"}` | one artifact's bytes (an image a Markdown document references, or one in its artifact folder), with its media type and the same headers as `file` |
| `export` | `{"scholar_id"?, "format"? (json, csv, bibtex), "view"?}` | the export's bytes, as the Export action renders them |

Read tools: `archive_status`, `list_scholars`, `scholar_report`, `list_works`, `get_work`,
`list_journals`, `list_workspaces`, `get_workspace`, `list_work_files`, `get_work_file`,
`list_work_artifacts`, `list_source_labels`, `list_source_updates`, `find_duplicate_works`, `job_status`,
`vector_index_status`, `semantic_search`, `similar_works`. Every other MCP tool (edits, operations,
collection, provider searches, imports, export files, backups, vector updates) is refused with `403`.

Read-only SQL opens the database with `mode=ro` and `PRAGMA query_only`, and an authorizer allows
only reads, functions, recursive queries and schema pragmas (`table_info`, `index_list`,
`foreign_key_list` and the like); writes, `ATTACH` and other pragmas are refused before they run.
A query stops after 20 seconds. Documents are opened with the same pinned-path checks as the cockpit's
downloads and are limited to 512 MB.

## Artifacts

An artifact is an image a Work's Markdown document shows. `list_work_artifacts` (`work_id` or a
Markdown's `file_id`) reads the Markdown's own image references and lists each artifact in reading
order: its `path` below `files/`, the `file_id` of the Markdown that shows it, its caption(s), media
type, size and availability, and whether a document the Work shows first uses it (`shown`). It also
names the artifact folders and lists the images there that no document references (`unused`) and the
references that cannot resolve (`problems`). The `artifact` action returns one artifact's bytes; it
accepts only an image the named Markdown references or one in its artifact folders, limited to 64 MB.

ZET keeps every artifact a Card shows in its own archive, as `artifacts/<sha256>.<ext>` beside a
`<sha256>.json` record of its source (`work_id`, `file_id`, `path`, caption). A Card shows it with
`![caption](artifact:<sha256>)`. The Card depends only on ZET's copy, so the archive may move,
rename or drop its file without breaking the Card.

## Running automatically

The archive server already starts at sign-in and after sleep (see Scholar Archive's README, "Install on this Windows computer"),
so the bridge is always served. On ZET's side, the first `zt scholar` command or `scholar_archive_*`
MCP call pairs ZET when it has no credential. When the archive does not recognize the credential
(for example after `revoke` or a lost `bridge.json`), ZET pairs again once. When the server is not
reachable, ZET runs `scholar-archive start` once and waits up to 30 seconds. ZET finds the command at
`%LOCALAPPDATA%\ScholarArchive\bin\scholar-archive.cmd`, or at `ZT_SCHOLAR_ARCHIVE`.

## Trust boundary

The bridge authenticates ZET against other programs and web pages, and replayed or altered traffic.
Both credential copies live in the Windows user's own profile folders, so a program running as the same
Windows user could read them, just as it could read the archive's database directly. The bridge does
not change that boundary. The cockpit's existing read API (`GET /api/...`) is unchanged.
