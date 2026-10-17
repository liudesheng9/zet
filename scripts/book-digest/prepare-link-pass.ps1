#Requires -Version 7.5
<#
.SYNOPSIS
  Prepare the link pass of the ZT digest workflows: a snapshot of every Card, and for each
  group the Cards it owns (with bodies) plus the allow-list the helper enforces.

.DESCRIPTION
  Reads manifest.json in -Staging. A group owns its anchor <key>/<n>, and either the addresses in
  owned-gNN.txt (written by the helper's -Log during the digest) or, when that file is missing,
  the Cards inferred from the text: Literature-tree Cards under its anchor that cite a section,
  and agent Topic-tree Cards whose first `§` reference names one of its sections. Topic roots,
  Hub Cards (and other Cards with 8 or more Links), and the book's structure Card (no section reference) stay unowned.
  Writes cards-all.md (identical for every group, so DeepSeek caches it), link-gNN.md, and
  allow-gNN.txt, and prints a compact JSON summary.

  Parameters: -Staging <folder> (required), -TitleChars 40.

  This script forwards every argument to `zt-digest prepare-link` (tools/zt-digest/src/prepare.rs),
  which reads the Cards on its own `zt mcp` connections (up to 12 at once); callers should call the
  tool directly. ZT_DIGEST names another zt-digest build.
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
& $exe prepare-link @pass
exit $LASTEXITCODE
