#Requires -Version 7.5
<#
.SYNOPSIS
  List the Model Cards a ZET book digest wrote, so the workflow can hand each one to its own
  Sonnet agent, which builds the model's Model parts and HTML charts.

  This script forwards to `zt-digest list-models` (tools/zt-digest, installed beside zt.exe by
  scripts/install-zt-digest.ps1). Call `zt-digest list-models` directly when you can: it takes
  the same parameters and prints the same line.

.DESCRIPTION
  Parameters: -Staging*, and -Groups (a string such as `1,2,3`) or -All with -CitationKey.

  Reads through `zt mcp` (agent Access, read-only calls). With -Groups, a Model Card counts when
  one of those groups wrote it (its address is in owned-gNN.txt in -Staging, written by the
  helper's -Log during the digest). With -All, every Model Card in the agent Literature tree of
  -CitationKey counts, plus every Model Card any group wrote. Model parts are never listed; a
  Model Card whose model already has parts is listed with its part count, so a rerun can skip it.
  Each model also names its elaboration level (concept, formal, or null when not yet decided) and its
  build faults (`faults`: closure, forward, unanchored, and beyond_level entries together).
  Each model also names its Math form (null for an ordinary model; zt-math-digest leaves Theory
  Cards to its theory pass).
  Prints one compact JSON line: {"models":[{"address","title","group","level","form","parts","issues","faults"}]}.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# `pwsh -File` and `&` hand `-Name:value` over as two arguments (a switch value as a Boolean) and a
# list as an array (`-Groups 1,2,3` from `&`); the tool takes `-Name:value` and a comma-separated list.
$pass = [Collections.Generic.List[string]]::new()
foreach ($a in $args) {
    $last = if ($pass.Count) { $pass[$pass.Count - 1] } else { '' }
    if ($last -match '^-[A-Za-z]\w*:$') { $pass[$pass.Count - 1] = $last + $(if ($a -is [array]) { $a -join ',' } else { "$a" }) }
    elseif ($a -is [bool] -and $last -match '^-[A-Za-z]\w*$') { $pass[$pass.Count - 1] = "${last}:$a" }
    elseif ($a -is [array]) { $pass.Add($a -join ',') }
    else { $pass.Add("$a") }
}
& $exe list-models @pass
exit $LASTEXITCODE
