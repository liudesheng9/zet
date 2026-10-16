# Literature Card Implementation Evidence

Scope: GitHub #27 through #34, implementing the contract locked in
[`literature-card-design.md`](literature-card-design.md).

## Implemented behavior

- Literature Cards use the shared `cards` table with an internal `row_id`, a
  separate client-assigned Citation key, `is_lit`, exact raw BibTeX, and normal
  three-section Card text.
- Citation keys are exact-case addresses, case-insensitively unique, safe as
  Windows `.bib` filenames, and cannot parse as Locations.
- BibTeX is parsed as exactly one ordinary entry with a literal nonempty title;
  invalid input is never stored.
- Shell `zt l`, Session `l`, dual-part Literature editing, atomic Citation-key
  rename and body-Link rewriting, cross-kind Links/Reverse links/Broken links,
  kind-aware navigation/listing/counts, Literature deletion, and `zt clear` are
  implemented at the existing seams.
- Dump writes one hashed Literature Markdown file and exact
  `<citation-key>.bib`, with Citation key-to-Markdown mapping and deterministic
  mixed ordering.
- Released code refuses the exact legacy schema without modifying it. The
  one-time migration helper was kept under ignored `target/` during the live
  operation and deleted afterward; no migration framework is shipped.

## Automated verification

Final verification against `zt 0.1.7`:

- `cargo fmt -- --check`: passed.
- `cargo check --all-targets`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all-targets`: passed, 89 tests total.
  - Unit: 21 passed.
  - CLI integration: 51 passed.
  - Real PTY: 17 passed.
- `git diff --check`: passed (Git reported only the repository's Windows line
  ending notices).

The suite includes schema constraints and non-mutating legacy refusal, BibTeX
and Citation-key parsing, atomic creation/cancellation, shell and Session edit
selection, rename rollback and body-only Link rewriting, every Card-kind Link
combination, Reverse/Broken links, deletion/clear, YAML round trips, exact
`.bib` bytes, mixed dump ordering, mouse activation, and existing Topic/Regular
regressions.

## Live database migration

Configured archive root: `D:\zt`.

Before migration, the installed `zt 0.1.6` service was stopped, its process was
confirmed gone, the Session registry was empty, and no edit lock existed. A
consistent SQLite backup was made and retained:

- Backup:
  `D:\zt\backups\zt.sqlite3.pre-literature-20260713T174725Z.backup`
- Size: `20480` bytes.
- SHA-256:
  `41846f3f674b23675bdb178b8124d939ddd63c8176ed22b1625b08e05f2dce63`
- Backup `PRAGMA quick_check`: `ok`.
- Snapshot evidence:
  `D:\zt\backups\zt.sqlite3.pre-literature-20260713T174725Z.backup.json`
- Migration evidence:
  `D:\zt\backups\zt.sqlite3.pre-literature-20260713T174725Z.backup.migrated.json`

The pre-migration database contained zero Cards, zero Topics, zero Regular
Cards, zero Literature Cards, and `next_topic_id = 0`. Its projected-row
inventory SHA-256 was
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.

Before migration, installed `zt 0.1.7` rejected the legacy schema. The database
file SHA-256 remained
`27962357fd079eed0fad2e84773981a4ec393ff37d6ce9af169a2aeb3cdb5f9f`
across that refusal. The manual migration then replaced and copied the schema
inside one SQLite transaction. Pre/post row inventory and counts matched,
`next_topic_id` remained `0`, the new columns matched the locked schema, and
post-migration `PRAGMA quick_check` returned `ok`.

The installed `zt 0.1.7` service started successfully and read the migrated
database. After the final formatting and Clippy cleanup, the exact verified
workspace source was reinstalled and the service restarted as PID `36380`.

## Installed-binary TUI and dump smoke

The live installed binary was exercised through a real PTY:

1. Session `l` created `SmokeLit` from raw BibTeX and main text.
2. Metadata editing renamed it to `SmokeRenamed` and updated the generated
   title; main-text editing used the normal Card editor path.
3. A Topic and Regular Card both cited the Literature Card. A real SGR mouse
   click on the rendered Citation-key Link navigated to it, whose view showed
   both Reverse links.
4. A second metadata edit renamed the Citation key to `SmokeFinal`, displayed
   a rewrite count of `2`, required `move`, and rewrote both inbound body Links.
5. ROOT showed only the Topic automatically; ROOT `ls` included Topic plus
   Literature, and Literature `ls` listed the Literature collection. Live
   `stats` reported total `3`, topics `1`, regular `1`, literature `1`.
6. A real dump created
   `D:\zt\dump\zt-archive-1783965100.zip` and reported three Cards. Its mapping
   was exactly
   `{"0/0":"9253367d28.md","0/1":"c854ee5b64.md","SmokeFinal":"ae42b5c624.md"}`.
   The archive contained those three hashed Markdown files and exact
   `SmokeFinal.bib`; the Literature Markdown contained YAML BibTeX metadata,
   generated title, edited body, and both Reverse links.
7. Session `del` required `delete`, removed only `SmokeFinal`, and returned the
   Pointer to ROOT. `zt lsbk` then reported the Topic and Regular citations as
   Broken.
8. `zt clear` deleted the remaining two smoke Cards and reset Topic allocation.

## Final live state

- Service: up, PID `36380`, installed version `zt 0.1.7`.
- Cards: total `0`, topics `0`, regular `0`, literature `0`.
- Session registry length: `0`; edit lock: absent.
- Live database `PRAGMA quick_check`: `ok`; `next_topic_id = 0`.
- Retained backup size and SHA-256 still match the values recorded above.
- The real smoke dump remains under `D:\zt\dump` as verification evidence.
