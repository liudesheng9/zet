#Requires -Version 7.5
<#
.SYNOPSIS
  Show the book's own figures in the digest Cards that refer to them, and link the book's
  documents from its Literature Card.

.DESCRIPTION
  For a digested book (Literature tree -CitationKey; its Scholar Archive Markdown -FileId, or the
  digest's -Staging folder, whose manifest names both):
  1. Keeps every figure of the Markdown in ZET's artifact store and reads its labels
     (`2.C.1`, `IV`/`四`) from the caption next to the image.
  2. Finds the agent Cards of this book that name a figure in their title or body (`图 2.C.1`,
     `图2.C.1`, `Figure 2.C.1`; never `图 2.C.10`). A Card is of this book when it sits in the
     Literature tree or links the Literature Card. Hub Cards, the Literature Card and its anchor
     line (`<key>/<n>`, the section indexes) list other Cards' titles and are left alone.
  3. Puts each figure's `![图 2.C.1](artifact:<sha256>)` line into the Card, as its own paragraph
     right after the paragraph that first names the figure (after a table, list, or chart fence
     that holds the mention; at the top of the body when only the title names it). A Card that
     already shows the figure is skipped, so the script can run again.
  4. Adds `原书文档：[原书 PDF](scholar:…) · …` to the Literature Card when it has no Document
     link yet.
  -FiguresFile takes the figures from a figures.json (prepare-book-digest.ps1 writes one in the
  staging folder) instead of the bridge; -NoDocuments skips step 4. Together they need no Scholar
  Archive.
  Without -Apply it only reports. With -Apply it writes through `zt mcp` (agent Access), passing
  each Card's stored text as `base`, so a Card changed in between is refused, not overwritten.
  Prints a JSON summary; -Report writes the full plan as JSON.

  Parameters: -CitationKey (required), -FileId 0, -Staging, -Source, -MinFigureBytes 3000,
  -Report, -Batch 40, -FiguresFile, -NoDocuments, -Apply.

  This script forwards every argument to `zt-digest figures` (tools/zt-digest/src/figures.rs),
  which does the work in one process with one `zt mcp`; callers should call the tool directly.
  ZT_DIGEST names another zt-digest build.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# `pwsh -File` and `&` hand `-Name:value` over as two arguments (a switch value as a Boolean); the
# tool takes `-Name:value`.
$pass = [Collections.Generic.List[string]]::new()
foreach ($a in $args) {
    $last = if ($pass.Count) { $pass[$pass.Count - 1] } else { '' }
    if ($last -match '^-[A-Za-z]\w*:$') { $pass[$pass.Count - 1] = $last + $(if ($a -is [array]) { $a -join ',' } else { "$a" }) }
    elseif ($a -is [bool] -and $last -match '^-[A-Za-z]\w*$') { $pass[$pass.Count - 1] = "${last}:$a" }
    elseif ($a -is [array]) { $pass.Add($a -join ',') }
    else { $pass.Add("$a") }
}
& $exe figures @pass
exit $LASTEXITCODE
