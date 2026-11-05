<#
.SYNOPSIS
  Brings one book's Term links up to date on the live box: plan and apply until a plan changes nothing.
.DESCRIPTION
  Run it after any write that follows a digest's own Term-link pass: a Literature organize, a model pass, a
  hand edit. It needs no run: it reads the tree, so it works on any agent Literature tree at any time. The pass
  preserves every visible character. A repair can free a word that only the next round links, so the command
  repeats, and it fails when three rounds leave a Card still changing.

    pwsh -NoProfile -File term-relink.ps1 -CitationKey <key> [-Occurrences first|all] [-PlanOnly]

  Prints one JSON line with the counts of every applied round.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$CitationKey,
    [ValidateSet('first', 'all')][string]$Occurrences = 'first',
    [switch]$PlanOnly
)
$ErrorActionPreference = 'Stop'
$rounds = 3
$work = Join-Path ([IO.Path]::GetTempPath()) "zt-term-relink-$CitationKey-$PID"
New-Item -ItemType Directory -Force $work | Out-Null
try {
    @{ citation_key = $CitationKey } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $work 'manifest.json') -Encoding utf8NoBOM
    $total = @{ ok = $true; book = $CitationKey; cards_changed = 0; links_added = 0; links_repaired = 0; cards_refused = 0; rounds = 0; applied = $false }
    for ($round = 0; ; $round++) {
        $line = & zt-digest term-link -Step plan -Staging $work -CitationKey $CitationKey -Occurrences $Occurrences | Select-Object -Last 1
        if ($LASTEXITCODE -ne 0) { throw "the plan failed: $line" }
        $plan = $line | ConvertFrom-Json
        $total.cards_refused = [int]$plan.cards_refused
        if ($plan.cards_changed -eq 0) { break }
        if ($PlanOnly) {
            $total.cards_changed = [int]$plan.cards_changed
            $total.links_added = [int]$plan.links_added
            $total.links_repaired = [int]$plan.links_repaired
            break
        }
        if ($round -eq $rounds) { throw "$rounds rounds left $($plan.cards_changed) Card(s) still changing" }
        $applied = & zt-digest term-link -Step apply -Staging $work -CitationKey $CitationKey -Occurrences $Occurrences -Apply | Select-Object -Last 1
        if ($LASTEXITCODE -ne 0) { throw "the apply failed: $applied" }
        $total.cards_changed += [int]$plan.cards_changed
        $total.links_added += [int]$plan.links_added
        $total.links_repaired += [int]$plan.links_repaired
        $total.rounds = $round + 1
        $total.applied = $true
    }
    $total | ConvertTo-Json -Compress
}
finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
