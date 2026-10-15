#Requires -Version 7
<#
.SYNOPSIS
  Carry out the Topic merges and Card moves that Opus decided in the Organize stage
  (doc/digest-card-layers-proposal.md, proposal 8). Opus writes decisions.json; this script makes
  every zt call, so the moves follow one rule and are logged.

.DESCRIPTION
  -Work is the Organize folder: it holds decisions.json and gets applied.json. decisions.json:
    {"thesis": "…",
     "merges": [{"drop": "15/0", "keep": "12/0", "under": "12/1", "reason": "…"}],
     "moves":  [{"at": "15/2|a", "under": "12/3", "reason": "…"}],
     "notes":  [{"address": "40/1", "note": "…"}]}
  A merge moves the first Card of the dropped Topic (`<n>/1`, with every Card after it) to the next
  free Side successor slot of `under` (a Card of the kept Topic; default `<keep>/1`), then deletes
  the emptied Topic root. A move puts one Card with its successors under `under` the same way.
  Every later address is translated through the `moved` maps zt returns, so a decision written
  before the first move still names the right Card. The script refuses a decision that names a
  Literature tree Card, a Model Card or part, a Hub Card, or a Topic that is not the agent's, and a
  drop root that still holds Cards after its move. `notes` are not applied here: the workflow
  hands them to a card-compiler.ps1 fix job.
  Prints one compact JSON line: merges and moves done, refusals, and the address map.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Work,
    [switch]$DryRun
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
$decisions = Get-Content -LiteralPath (Join-Path $Work 'decisions.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
$map = [ordered]@{}
$done = @()
$refused = @()

function Get-Now([string]$address) {
    # The address today: through every rename so far, longest prefix first (a moved subtree).
    $a = $address
    foreach ($old in @($map.Keys | Sort-Object { - $_.Length })) {
        if ($a -eq $old) { return [string]$map[$old] }
        if ($a.StartsWith("$old|") -or $a.StartsWith("$old/")) { return [string]$map[$old] + $a.Substring($old.Length) }
    }
    $a
}
# Runs the calls through the helper; every rename a call makes goes into the address map (the
# helper's -Moves log holds the whole `moved` map of each move and delete).
function Invoke-Calls($calls) {
    $movesFile = Join-Path $Work 'moves.jsonl'
    if (Test-Path -LiteralPath $movesFile) { Remove-Item -LiteralPath $movesFile }
    $lines = @(($calls | ConvertTo-Json -Depth 8 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Moves $movesFile)
    $error1 = @($lines | Where-Object { $_ -match '^\d+ error ' })
    if ($LASTEXITCODE -ne 0 -or $error1.Count) { throw "zt: $(($error1 + $lines)[0])" }
    if (Test-Path -LiteralPath $movesFile) {
        foreach ($line in Get-Content -LiteralPath $movesFile -Encoding utf8) {
            $entry = $line | ConvertFrom-Json -AsHashtable
            # A later rename of an address the map already holds updates that entry too.
            foreach ($old in @($map.Keys)) { if ($entry.moved.ContainsKey([string]$map[$old])) { $map[$old] = [string]$entry.moved[[string]$map[$old]] } }
            foreach ($k in $entry.moved.Keys) { $map[$k] = [string]$entry.moved[$k] }
        }
    }
    $lines
}
function Get-Card([string]$at) { try { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] } catch { $null } }
function Get-FreeSide([string]$under) {
    $tree = ($under -split '/', 2)[0]
    $cards = @((@(Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $tree } }))[0]).cards | ForEach-Object { [string]$_.address })
    $max = 0
    foreach ($c in $cards) {
        if (-not $c.StartsWith("$under|")) { continue }
        $label = ($c.Substring($under.Length + 1) -split '\|')[0]
        if ($label -notmatch '^[a-z]+$') { continue }
        $n = 0
        foreach ($ch in $label.ToCharArray()) { $n = $n * 26 + ([int]$ch - 96) }
        if ($n -gt $max) { $max = $n }
    }
    $n = $max + 1
    $label = ''
    while ($n -gt 0) { $n--; $label = [char](97 + ($n % 26)) + $label; $n = [Math]::Floor($n / 26) }
    "$under|$label"
}
function Test-Movable($card, [string]$what) {
    if (-not $card) { return "$what does not exist" }
    if ($card.workspace -ne 'agent') { return "$what is not in the agent Workspace" }
    if ($card.type -in @('model', 'hub')) { return "$what is a $($card.type) Card" }
    if ($card.kind -eq 'literature' -or ($card.tree -and [string]$card.tree -notmatch '^\d+$')) { return "$what is in a Literature tree" }
    $null
}
# Move `at` (with its successors) to the next free Side successor of `under`; record the renames.
function Move-Under([string]$at, [string]$under) {
    $to = Get-FreeSide $under
    if ($DryRun) { return $to }
    $null = Invoke-Calls @(@{ tool = 'move_card'; arguments = @{ at = $at; to = $to; confirm = 'move' } })
    if (-not $map.Contains($at)) { $map[$at] = $to }
    $to
}

foreach ($m in @($decisions.merges)) {
    $drop = Get-Now ([string]$m.drop)
    $keep = Get-Now ([string]$m.keep)
    $why = $null
    if ($drop -notmatch '^\d+/0$' -or $keep -notmatch '^\d+/0$') { $why = 'drop and keep must be Topic roots (<n>/0)' }
    elseif ($drop -eq $keep) { $why = 'drop and keep are the same Topic' }
    if (-not $why) {
        $dropRoot = Get-Card $drop
        $keepRoot = Get-Card $keep
        if (-not $dropRoot -or -not $keepRoot -or $dropRoot.workspace -ne 'agent' -or $keepRoot.workspace -ne 'agent') { $why = 'both Topics must exist in the agent Workspace' }
    }
    $under = if ($m.under) { Get-Now ([string]$m.under) } else { $keep -replace '/0$', '/1' }
    if (-not $why -and ($under -split '/', 2)[0] -ne ($keep -split '/', 2)[0]) { $why = "under ``$under`` is not in the kept Topic" }
    if (-not $why -and -not (Get-Card $under)) { $why = "under ``$under`` does not exist" }
    if ($why) { $refused += [ordered]@{ decision = 'merge'; drop = [string]$m.drop; keep = [string]$m.keep; reason = $why }; continue }
    $first = $drop -replace '/0$', '/1'
    try {
        $to = if (Get-Card $first) { Move-Under $first $under } else { $null }
        $tree = ($drop -split '/', 2)[0]
        $left = @((@(Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $tree } }))[0]).cards | Where-Object { [string]$_.address -ne $drop })
        if ($left.Count -and -not $DryRun) { $refused += [ordered]@{ decision = 'merge'; drop = [string]$m.drop; keep = [string]$m.keep; reason = "the dropped Topic still holds $($left.Count) Cards; its root stays" }; continue }
        if (-not $DryRun) { $null = Invoke-Calls @(@{ tool = 'delete_card'; arguments = @{ at = $drop; into = $keep; confirm = $drop } }) }
        $done += [ordered]@{ decision = 'merge'; drop = $drop; keep = $keep; moved_to = $to; reason = [string]$m.reason }
    } catch { $refused += [ordered]@{ decision = 'merge'; drop = [string]$m.drop; keep = [string]$m.keep; reason = $_.Exception.Message } }
}
foreach ($m in @($decisions.moves)) {
    $at = Get-Now ([string]$m.at)
    $under = Get-Now ([string]$m.under)
    $card = Get-Card $at
    $why = Test-Movable $card "``$at``"
    if (-not $why -and $at -match '^\d+/0$') { $why = 'a Topic root moves only by a merge' }
    if (-not $why -and -not (Get-Card $under)) { $why = "under ``$under`` does not exist" }
    if (-not $why -and ($under -eq $at -or $under.StartsWith("$at|") -or $under.StartsWith("$at/"))) { $why = 'a Card cannot move under itself' }
    if ($why) { $refused += [ordered]@{ decision = 'move'; at = [string]$m.at; reason = $why }; continue }
    try {
        $to = Move-Under $at $under
        $done += [ordered]@{ decision = 'move'; at = $at; to = $to; reason = [string]$m.reason }
    } catch { $refused += [ordered]@{ decision = 'move'; at = [string]$m.at; reason = $_.Exception.Message } }
}
# The notes name Cards by the addresses Opus read: translate them, so the card cycle finds them.
$translated = 0
if (-not $DryRun -and $map.Count -and @($decisions.notes).Count) {
    foreach ($n in @($decisions.notes)) {
        $now = Get-Now ([string]$n.address)
        if ($now -ne [string]$n.address) { $n.address = $now; $translated++ }
    }
    [IO.File]::WriteAllText((Join-Path $Work 'decisions.json'), ($decisions | ConvertTo-Json -Depth 8), $utf8)
}
$result = [ordered]@{ dry_run = [bool]$DryRun; done = $done; refused = $refused; moved = $map; notes_translated = $translated }
[IO.File]::WriteAllText((Join-Path $Work 'applied.json'), ($result | ConvertTo-Json -Depth 8), $utf8)
[ordered]@{ merges = @($done | Where-Object { $_.decision -eq 'merge' }).Count; moves = @($done | Where-Object { $_.decision -eq 'move' }).Count; refused = $refused.Count; renamed = $map.Count } | ConvertTo-Json -Compress
