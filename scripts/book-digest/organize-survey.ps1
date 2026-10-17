#Requires -Version 7.5
<#
.SYNOPSIS
  The survey of the Organize stage (docs/digest-card-layers-proposal.md, proposal 8): everything Opus
  needs to decide, and nothing it must find by reading the trees. Read-only.

  This script forwards to `zt-digest organize-survey` (tools/zt-digest/src/organize/survey.rs,
  installed beside zt.exe by scripts/install-zt-digest.ps1), which does the work in one process,
  with the audit in-process. Call `zt-digest organize-survey` directly when you can: it takes the
  same parameters, writes the same files, and prints the same lines.

.DESCRIPTION
  -Staging is the digest's staging folder (manifest.json). -Work is the Organize folder. The tool
  runs the audit on the book (into <Work>\audit) and writes survey.md (for Opus) and survey.json:
    1. Topics: every agent Topic that holds Cards of the book, with its size and the share of the
       book's Cards in it.
    2. Duplicate candidates: pairs of agent Topics, one of them the book's, whose titles are
       similar (character bigram similarity -Similar, 0.5, or one title inside the other) or that
       link each other often (-CrossLinks, 4, Links between their trees).
    3. Small Topics: the book's Topics with -Small (3) Cards or fewer.
    4. Merge proposals of the card cycles (decisions.json in <Staging>\cards*), and the Idea Cards
       the audit names as similar (I6) or short (I2 merge candidate).
    5. Models that are not self-sufficient or have get_model issues, and the architect's notes
       (-Notes: a JSON file of {"model","notes":[...]}, written by the workflow).
    6. Broken Links from the book's Cards, and the hubs the policy requires but the Workspace lacks.
  Prints one compact JSON summary line last.

  Parameters: -Staging and -Work (required), -Notes, -Similar 0.5, -CrossLinks 4, -Small 3.
  ZT_DIGEST names another zt-digest build.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# `pwsh -File` and `&` hand `-Name:value` over as two arguments; the tool takes `-Name:value`.
$pass = [Collections.Generic.List[string]]::new()
foreach ($a in $args) {
    $last = if ($pass.Count) { $pass[$pass.Count - 1] } else { '' }
    if ($last -match '^-[A-Za-z]\w*:$') { $pass[$pass.Count - 1] = $last + $(if ($a -is [array]) { $a -join ',' } else { "$a" }) }
    elseif ($a -is [bool] -and $last -match '^-[A-Za-z]\w*$') { $pass[$pass.Count - 1] = "${last}:$a" }
    elseif ($a -is [array]) { $pass.Add($a -join ',') }
    else { $pass.Add("$a") }
}
& $exe organize-survey @pass
exit $LASTEXITCODE
