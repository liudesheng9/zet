#Requires -Version 7
<#
.SYNOPSIS
  List the Model Cards a ZET book digest wrote, so the workflow can hand each one to its own
  Sonnet agent, which builds the model's Model parts and HTML charts.

.DESCRIPTION
  Reads through `zt mcp` (agent Access, read-only calls). With -Groups, a Model Card counts when
  one of those groups wrote it (its address is in owned-gNN.txt in -Staging, written by the
  helper's -Log during the digest). With -All, every Model Card in the agent Literature tree of
  -CitationKey counts, plus every Model Card any group wrote. Model parts are never listed; a
  Model Card whose model already has parts is listed with its part count, so a rerun can skip it.
  Prints one compact JSON line: {"models":[{"address","title","group","parts","issues"}]}.
#>
[CmdletBinding(DefaultParameterSetName = 'Groups')]
param(
    [Parameter(Mandatory)][string]$Staging,
    [Parameter(ParameterSetName = 'Groups', Mandatory)][int[]]$Groups,
    [Parameter(ParameterSetName = 'All', Mandatory)][switch]$All,
    [Parameter(ParameterSetName = 'All')][string]$CitationKey = ''
)
$ErrorActionPreference = 'Stop'
$helper = Join-Path $PSScriptRoot '..\..\skills\zt\scripts\zt-mcp.ps1' | Resolve-Path | ForEach-Object Path

function Invoke-Zt($calls) {
    $lines = ($calls | ConvertTo-Json -Depth 10 -AsArray) | pwsh -NoProfile -File $helper -
    if ($LASTEXITCODE -ne 0) { throw "zt-mcp failed: $($lines -join ' | ')" }
    foreach ($line in $lines) { if ($line -match '^\d+ (\{.*)$') { $Matches[1] | ConvertFrom-Json -DateKind String } }
}

# Which group wrote each address.
$owner = @{}
foreach ($file in Get-ChildItem -LiteralPath $Staging -Filter 'owned-g*.txt' -File -ErrorAction SilentlyContinue) {
    $n = [int]($file.BaseName -replace '^owned-g', '')
    if ($PSCmdlet.ParameterSetName -eq 'Groups' -and $n -notin $Groups) { continue }
    foreach ($address in Get-Content -LiteralPath $file.FullName -Encoding utf8) {
        if ($address.Trim()) { $owner[$address.Trim()] = $n }
    }
}

# Read whole trees, not search_cards: a search stops at 200 results, and a book's Model Cards
# and parts easily pass that. The trees are the book's and those the groups wrote into.
$trees = @(@($owner.Keys | ForEach-Object { ($_ -split '/', 2)[0] }) + @($CitationKey) |
    Where-Object { $_ } | Sort-Object -Unique)
$read = if ($trees.Count) { @(Invoke-Zt @($trees | ForEach-Object { @{ tool = 'get_tree'; arguments = @{ tree = $_ } } })) } else { @() }
$found = @($read | ForEach-Object { $_.cards } | Where-Object { $_.workspace -eq 'agent' -and $_.type -eq 'model' })
$heads = @($found | Where-Object {
    -not $_.facet -and ($owner.ContainsKey($_.address) -or
        ($All -and $CitationKey -and ($_.address -like "$CitationKey/*")))
})
$models = @()
if ($heads.Count) {
    $structures = @(Invoke-Zt @($heads | ForEach-Object { @{ tool = 'get_model'; arguments = @{ at = $_.address } } }))
    for ($i = 0; $i -lt $heads.Count; $i++) {
        $models += [ordered]@{
            address = $heads[$i].address
            title = $heads[$i].title
            group = $owner[$heads[$i].address]
            parts = @($structures[$i].parts).Count
            issues = @($structures[$i].issues).Count
        }
    }
}
[ordered]@{ models = $models } | ConvertTo-Json -Depth 5 -Compress
