#Requires -Version 7.5
<#
.SYNOPSIS
  Write the context pack one Sonnet model elaborator starts from: the Model Card, the Cards it
  links and that link it, the book group it came from with the source files and headings to
  read, the anchor Card of that group, and the other models of the book.

  This script forwards to `zt-digest model-context` (tools/zt-digest, installed beside zt.exe by
  scripts/install-zt-digest.ps1). Call `zt-digest model-context` directly when you can: it takes
  the same parameters, writes the same pack, and prints the same line.

.DESCRIPTION
  Parameters: -Staging*, -Model*, -Out*.

  Reads through `zt mcp` (agent Access, read-only calls). The group is the one whose
  owned-gNN.txt in -Staging lists the Model Card; failing that, the anchor the Model Card is
  filed under (`<key>/<n>`). Source files come from -Staging\manifest.json when it exists; the
  elaborator reads them itself (the pack names files and headings with line numbers, it does
  not copy the book). Writes Markdown to -Out and prints one compact JSON line:
  {"model","group","anchor","sections","parts","context"}.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# `pwsh -File` and `&` hand `-Name:value` over as two arguments (a switch value as a Boolean) and a
# list as an array; the tool takes `-Name:value` and a comma-separated list.
$pass = [Collections.Generic.List[string]]::new()
foreach ($a in $args) {
    $last = if ($pass.Count) { $pass[$pass.Count - 1] } else { '' }
    if ($last -match '^-[A-Za-z]\w*:$') { $pass[$pass.Count - 1] = $last + $(if ($a -is [array]) { $a -join ',' } else { "$a" }) }
    elseif ($a -is [bool] -and $last -match '^-[A-Za-z]\w*$') { $pass[$pass.Count - 1] = "${last}:$a" }
    elseif ($a -is [array]) { $pass.Add($a -join ',') }
    else { $pass.Add("$a") }
}
& $exe model-context @pass
exit $LASTEXITCODE
