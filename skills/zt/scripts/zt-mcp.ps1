#Requires -Version 7.5
<#
.SYNOPSIS
  Call zt MCP tools from a shell, for agents whose harness has no `zt` tools (DeepSeek Harness).

.DESCRIPTION
  This script forwards to `zt-digest call` (tools/zt-digest, installed beside zt.exe by
  scripts/install-zt-digest.ps1), which does the work in one process. Call `zt-digest call`
  directly when you can: it takes the same parameters and prints the same lines, and starts no pwsh.

    zt-digest call <file|-> [-From N -Addresses a0,a1,...] [-Log f] [-Allow f] [-Moves f] [-SrcLog f]
                   [-Open] [-TopicGate f] [-TopicSimilarity 0.8]
    zt-digest call -Tool <tool> [-Json '{...}']

  -File takes a UTF-8 JSON file holding one call or an array of calls, or `-` to read the same
  JSON from stdin (so one shell command can carry a whole batch in a here-string):
    [{"tool": "create_successor", "arguments": {"at": "Key", "title": "...", "body": "..."}},
     {"tool": "create_successor", "arguments": {"at": "@@0", "kind": "side", "title": "...", "body": "... [[@@0]] ..."}}]
  Inside any string argument, `@@N` is replaced by the address returned by an earlier call N
  (0-based). -Tool with -Json runs a single call given inline.

  Output: `N ok <address>` for writes, `N <json>` for reads, `N error <message>` on failure.
  After a failure the last line is `resume: -From N -Addresses a0,a1,...`: fix call N in the same
  input and run again with those two arguments; calls before N are skipped and their addresses
  still resolve `@@`.

  A failure the archive answered with a typed refusal also prints one line just before the resume
  line: `<code> <compact JSON>`. The owner's Lock therefore reads as
  `card_locked {"error":"card_locked","tree":"K","reason":"create-inside",...}`: the tree, Lock id,
  reason and hint survive as JSON, a Card with `lock` set stays readable and linkable, and a
  `card_locked` answer is a planning/destination refusal. Do not retry it as a provider fault,
  request another Access, call the GUI routes or write the database directly: ask the owner to
  unlock the tree in the GUI, or continue the thought in another unlocked tree.

  Scope (docs/literature-only-digest-design.md): without -Open the helper runs in the Literature
  scope, as every ZT digest does: it refuses create_topic, any write to a Topic-tree Card, a Hub
  Card, and authored text that links a Topic-tree Card. Pass -Open for hand work on Topic trees;
  zt-idea-grow writes its Topic trees in-process and needs no -Open.

  -Log <file> appends the address of every Card a create call makes, one per line. -Allow <file>
  (one address per line) limits the run to those Cards: edit_card, append_card, and
  create_successor must target a listed address or a Card made earlier in the same run, and
  create_topic, move_card, insert_card, delete_card, create_literature, and
  edit_literature_metadata are refused. -Moves <file> appends {"i":N,"moved":{...}} per call that
  renumbered Cards. A create call may carry `src` (its source lines): the helper removes it and
  -SrcLog <file> appends `<address><TAB><src>`. -TopicGate <file> locks the file around
  create_topic and refuses a Topic whose title equals or resembles (character bigrams,
  -TopicSimilarity) an agent Topic, naming that Topic. append_card {at, text} adds a paragraph to
  the end of a Card.

  zt-calls: direct reads. scripts/book-digest/zt-calls.ps1 sends batches of read-only calls
  straight to one long-lived `zt mcp` and reproduces this helper's read output (`N <compact JSON>`)
  and its stop-at-first-error rule. Keep the two the same. A replacement helper without this line
  (a test seam) gets every call.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
$OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
if ($args -contains '-') { [Console]::In.ReadToEnd() | & $exe call @args } else { & $exe call @args }
exit $LASTEXITCODE
