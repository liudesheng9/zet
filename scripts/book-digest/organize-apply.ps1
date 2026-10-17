#Requires -Version 7.5
<#
.SYNOPSIS
  Carry out the Topic merges and Card moves that Opus decided in the Organize stage
  (docs/digest-card-layers-proposal.md, proposal 8). Opus writes decisions.json; this script makes
  every zt call, so the moves follow one rule and are logged.

  This script forwards to `zt-digest organize-apply` (tools/zt-digest/src/organize/apply.rs,
  installed beside zt.exe by scripts/install-zt-digest.ps1), which does the work in one process
  with one `zt mcp`. Call `zt-digest organize-apply` directly when you can: it takes the same
  parameters, writes the same files, and prints the same lines.

.DESCRIPTION
  -Work is the Organize folder: it holds decisions.json and gets applied.json. decisions.json:
    {"thesis": "…",
     "merges": [{"drop": "15/0", "keep": "12/0", "under": "12/1", "reason": "…"}],
     "moves":  [{"at": "15/2|a", "under": "12/3", "reason": "…"}],
     "notes":  [{"address": "40/1", "note": "…"}]}
  A merge moves the first Card of the dropped Topic (`<n>/1`, with every Card after it) to the
  next free Side successor slot of `under` (a Card of the kept Topic; default `<keep>/1`), then
  deletes the emptied Topic root. A move puts one Card with its successors under `under` the same
  way. Each move and delete is one call through the helper with -Moves <Work>\moves.jsonl; every
  later address is translated through the `moved` maps zt returns, so a decision written before
  the first move still names the right Card. The tool refuses a decision that names a Literature
  tree Card, a Model Card or part, a Hub Card, or a Topic that is not the agent's, and a drop root
  that still holds Cards after its move. `notes` are not applied here: the workflow hands them to
  a card cycle fix job (`zt-digest card -Notes`); their addresses follow the moves, and
  decisions.json is written again.
  Prints one compact JSON line: merges and moves done, refusals, and the address map.

  Parameters: -Work (required), -DryRun. ZT_DIGEST names another zt-digest build.
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
& $exe organize-apply @pass
exit $LASTEXITCODE
