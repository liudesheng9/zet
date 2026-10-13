#Requires -Version 7
<#
.SYNOPSIS
  Write a compact snapshot of every Topic tree (address and title of each Card) for a DeepSeek
  digest run, so it can place and link Cards without spending turns on searches.

.DESCRIPTION
  Reads through `zt mcp` (agent Access, read-only calls). Agent Topic trees list every Card;
  human Topic trees are listed too, marked read-only. Titles are cut to -TitleChars characters.
  Prints the number of Cards listed.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Out,
    [int]$TitleChars = 40
)
$ErrorActionPreference = 'Stop'
# The helper writes UTF-8; read its output as UTF-8, not the console's code page.
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$utf8 = [Text.UTF8Encoding]::new($false)
$helper = Join-Path $PSScriptRoot '..\..\skills\zt\scripts\zt-mcp.ps1' | Resolve-Path | ForEach-Object Path

function Invoke-Zt($calls) {
    $lines = ($calls | ConvertTo-Json -Depth 10 -AsArray) | pwsh -NoProfile -File $helper -
    if ($LASTEXITCODE -ne 0) { throw "zt-mcp failed: $($lines -join ' | ')" }
    foreach ($line in $lines) {
        if ($line -match '^\d+ (\{.*)$') { $Matches[1] | ConvertFrom-Json -DateKind String }
    }
}
function Cut($text) { if ($text.Length -le $TitleChars) { $text } else { $text.Substring(0, $TitleChars) + '…' } }
# Common facets read in Chinese; any other facet is the label its author wrote.
$facetNames = @{ setting = '设定'; assumptions = '假设'; timing = '时序'; dynamics = '动态'; solution = '求解'; results = '结论'; intuition = '直觉'; limits = '局限' }
function Mark($card) {
    if ($card.type -eq 'model' -and $card.facet) { $name = if ($facetNames.ContainsKey($card.facet)) { $facetNames[$card.facet] } else { $card.facet }; return " [模型·$name]" }
    switch ($card.type) { 'model' { ' [模型]' } 'event' { ' [事件]' } default { '' } }
}

$trees = (Invoke-Zt @(@{ tool = 'list_trees'; arguments = @{} })).trees
$details = @(Invoke-Zt @($trees | ForEach-Object { @{ tool = 'get_tree'; arguments = @{ tree = $_.tree } } }))

$text = [Text.StringBuilder]::new()
[void]$text.AppendLine('# 现有卡片快照（本组启动时生成；之后其他组可能新增卡片；[模型] 为模型卡，[模型·假设] 等为模型分卡，[事件] 为事件卡）').AppendLine()
$count = 0
foreach ($workspace in 'agent', 'human') {
    [void]$text.AppendLine($(if ($workspace -eq 'agent') { '## 代理工作区' } else { '## 人类工作区（只读，可链接）' }))
    foreach ($tree in $details | Where-Object { $_.workspace -eq $workspace }) {
        foreach ($card in $tree.cards | Sort-Object { $_.address }) {
            $indent = if ($card.edge -eq 'root') { '- ' } else { '  - ' }
            [void]$text.AppendLine("$indent``$($card.address)``$(Mark $card) $(Cut $card.title)")
            $count++
        }
    }
    [void]$text.AppendLine()
}
[IO.File]::WriteAllText($Out, $text.ToString(), $utf8)
"$count cards -> $Out"
