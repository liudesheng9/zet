#Requires -Version 7.5
<#
.SYNOPSIS
  Write a compact snapshot of every Topic tree (address and title of each Card) for a DeepSeek
  digest run, so it can place and link Cards without spending turns on searches.

.DESCRIPTION
  Reads through `zt mcp` (agent Access, read-only calls). Agent Topic trees list every Card;
  human Topic trees are listed too, marked read-only. Titles are cut to -TitleChars characters.
  Prints the number of Cards listed.

  Parameters: -Out <file> (required), -TitleChars 40.

  This script forwards every argument to `zt-digest snapshot-topics` (tools/zt-digest/src/snapshot.rs),
  which reads through one `zt mcp`; callers should call the tool directly. ZT_DIGEST names another
  zt-digest build.
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
& $exe snapshot-topics @pass
exit $LASTEXITCODE
