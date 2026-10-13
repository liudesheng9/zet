#Requires -Version 7
<#
.SYNOPSIS
  Prepare the link pass of the zt-book-digest workflow: a snapshot of every Card, and for each
  group the Cards it owns (with bodies) plus the allow-list the helper enforces.

.DESCRIPTION
  Reads manifest.json in -Staging. A group owns its anchor <key>/<n>, and either the addresses in
  owned-gNN.txt (written by the helper's -Log during the digest) or, when that file is missing,
  the Cards inferred from the text: Literature-tree Cards under its anchor that cite a section,
  and agent Topic-tree Cards whose first `§` reference names one of its sections. Topic roots,
  hubs, and the book's structure Card (no section reference) stay unowned.
  Writes cards-all.md (identical for every group, so DeepSeek caches it), link-gNN.md, and
  allow-gNN.txt, and prints a compact JSON summary.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Staging,
    [int]$TitleChars = 40
)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$installedHelper = Join-Path $HOME '.codex\skills\zt\scripts\zt-mcp.ps1'
$manifest = Get-Content -LiteralPath (Join-Path $Staging 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
$key = $manifest.citation_key

. (Join-Path $PSScriptRoot 'zt-calls.ps1')
function Cut($text) { if ($text.Length -le $TitleChars) { $text } else { $text.Substring(0, $TitleChars) + '…' } }
# Common facets read in Chinese; any other facet is the label its author wrote.
$facetNames = @{ setting = '设定'; assumptions = '假设'; timing = '时序'; dynamics = '动态'; solution = '求解'; results = '结论'; intuition = '直觉'; limits = '局限' }
function Mark($card) {
    if ($card.type -eq 'model' -and $card.facet) { $name = if ($facetNames.ContainsKey($card.facet)) { $facetNames[$card.facet] } else { $card.facet }; return " [模型·$name]" }
    switch ($card.type) { 'model' { ' [模型]' } 'event' { ' [事件]' } default { '' } }
}

# --- every tree, both Workspaces ----------------------------------------------------------------
$trees = (Invoke-Zt @(@{ tool = 'list_trees'; arguments = @{} })).trees
$details = @(Invoke-Zt @($trees | ForEach-Object { @{ tool = 'get_tree'; arguments = @{ tree = $_.tree } } }))
$snapshot = [Text.StringBuilder]::new()
[void]$snapshot.AppendLine('# 全部卡片快照（地址与标题；[模型] 为模型卡，[模型·假设] 等为模型分卡，[事件] 为事件卡）').AppendLine()
foreach ($workspace in 'agent', 'human') {
    [void]$snapshot.AppendLine($(if ($workspace -eq 'agent') { '## 代理工作区' } else { '## 人类工作区（只读，可链接）' }))
    foreach ($tree in $details | Where-Object { $_.workspace -eq $workspace } | Sort-Object { $_.zone }, { $_.tree }) {
        foreach ($card in $tree.cards | Sort-Object { $_.address }) {
            $indent = if ($card.edge -eq 'root') { '- ' } else { '  - ' }
            [void]$snapshot.AppendLine("$indent``$($card.address)``$(Mark $card) $(Cut $card.title)")
        }
    }
    [void]$snapshot.AppendLine()
}
[IO.File]::WriteAllText((Join-Path $Staging 'cards-all.md'), $snapshot.ToString(), $utf8)

# --- section id -> group -------------------------------------------------------------------------
$numerals = @{ '一' = '1'; '二' = '2'; '三' = '3'; '四' = '4'; '五' = '5'; '六' = '6'; '七' = '7'; '八' = '8'; '九' = '9' }
$sectionGroup = @{}
foreach ($group in $manifest.groups) {
    foreach ($heading in $group.headings) {
        $id = if ($heading -match '^(Intro\.\d+|A\d+|\d+(\.\d+)?)\s') { $Matches[1] }
              elseif ($heading -match '^第(.)章') { $numerals[$Matches[1]] }
              elseif ($heading -eq '导论') { '导论' } else { $null }
        if ($id -and -not $sectionGroup.ContainsKey($id)) { $sectionGroup[$id] = [int]$group.n }
    }
}
function Group-OfSection($id) {
    if ($sectionGroup.ContainsKey($id)) { return $sectionGroup[$id] }
    $parts = $id -split '\.'
    if ($parts.Count -gt 2 -and $sectionGroup.ContainsKey("$($parts[0]).$($parts[1])")) { return $sectionGroup["$($parts[0]).$($parts[1])"] }
    if ($id -like 'Intro*' -and $sectionGroup.ContainsKey('导论')) { return $sectionGroup['导论'] }
    if ($sectionGroup.ContainsKey($parts[0])) { return $sectionGroup[$parts[0]] }
    return $null
}

# --- ownership -----------------------------------------------------------------------------------
$agentCards = @($details | Where-Object { $_.workspace -eq 'agent' } | ForEach-Object { $_.cards } | Where-Object { $_.kind -ne 'topic' })
$bodies = @{}
foreach ($card in @(Invoke-Zt @($agentCards | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_.address } } }))) {
    $bodies[$card.address] = $card
}
$owner = @{}
$logged = @{}
foreach ($group in $manifest.groups) {
    $log = Join-Path $Staging ("owned-{0}.txt" -f $group.id)
    if (Test-Path -LiteralPath $log) {
        $logged[[int]$group.n] = $true
        Get-Content -LiteralPath $log -Encoding utf8 | ForEach-Object { $_.Trim() } | Where-Object { $_ -and $bodies.ContainsKey($_) } |
            ForEach-Object { $owner[$_] = [int]$group.n }
    }
    $owner[$group.anchor] = [int]$group.n
}
foreach ($card in $agentCards) {
    $address = $card.address
    if ($owner.ContainsKey($address)) { continue }
    $body = [string]$bodies[$address].body
    $n = $null
    # Hub and structure Cards (long lists of Links) belong to the coordinator, not to a group.
    if (@($bodies[$address].outbound).Count -ge 8) { continue }
    $section = '(Intro\.\d+|A\d+|\d+(?:\.\d+)*|导论)'
    if ($address -match "^$([regex]::Escape($key))/(\d+)\|") {
        $anchorGroup = [int]$Matches[1]
        if ($body -match '§|来源') { $n = $anchorGroup }
    } elseif ($body -match "来源[：:]\s*\[\[$([regex]::Escape($key))\]\]\s*§?\s*$section") {
        $n = Group-OfSection $Matches[1]
    } elseif ($body -match "§\s*$section") {
        $n = Group-OfSection $Matches[1]
    }
    if ($n -and -not $logged[$n]) { $owner[$address] = $n }
}

# --- candidate targets: the most similar Cards outside the owner's group --------------------------
. (Join-Path $PSScriptRoot 'relatedness.ps1')
$titles = @{}
$cardsByAddress = @{}
$documents = foreach ($tree in $details) {
    foreach ($card in $tree.cards) {
        $titles[$card.address] = $card.title
        $cardsByAddress[$card.address] = $card
        $text = if ($bodies.ContainsKey($card.address)) { "$($card.title) $($card.title) $($bodies[$card.address].body)" } else { "$($card.title) $($card.title) $($card.excerpt)" }
        @{ id = $card.address; text = $text }
    }
}
$cardModel = New-Relatedness @($documents) 80
function Get-Candidates($address, $n) {
    $linked = @{}
    foreach ($out in @($bodies[$address].outbound)) { $linked[$out.address] = $true }
    Find-Related $cardModel $address 5 { param($id) $owner[$id] -eq $n -or $linked[$id] -or $id -like '*/0' }
}

# --- per-group files -----------------------------------------------------------------------------
$summary = @()
foreach ($group in $manifest.groups) {
    $n = [int]$group.n
    $mine = @($owner.Keys | Where-Object { $owner[$_] -eq $n } | Sort-Object)
    # The workflow writes linked-gNN.txt when a group's link pass succeeds; a rerun skips it.
    $done = Test-Path -LiteralPath (Join-Path $Staging ("linked-{0}.txt" -f $group.id))
    $allow = Join-Path $Staging ("allow-{0}.txt" -f $group.id)
    [IO.File]::WriteAllText($allow, ($mine -join "`n") + "`n", $utf8)
    $sectionList = (@($group.sections | ForEach-Object { '`' + (Join-Path $Staging "sections\$_") + '`' })) -join '、'
    $text = [Text.StringBuilder]::new()
    [void]$text.AppendLine("# 你的组：第 $n 组").AppendLine()
    [void]$text.AppendLine("- 锚点卡片：``$($group.anchor)``；小节：$($group.headings -join '；')")
    [void]$text.AppendLine("- 你这一部分的原文：$sectionList")
    [void]$text.AppendLine("- 全书索引：``$($manifest.index)``；全部卡片索引：``$(Join-Path $Staging 'cards-all.md')``")
    [void]$text.AppendLine("- 写入命令：``... | pwsh -NoProfile -File `"$installedHelper`" - -Allow `"$allow`"``")
    [void]$text.AppendLine("- 你负责的卡片（$($mine.Count) 张）：").AppendLine()
    foreach ($address in $mine) {
        $card = $bodies[$address]
        [void]$text.AppendLine("### ``$address`` $($card.title)").AppendLine().AppendLine(([string]$card.body).Trim()).AppendLine()
        $candidates = @(Get-Candidates $address $n | ForEach-Object { "``$_``$(Mark $cardsByAddress[$_]) $(Cut $titles[$_])" })
        if ($candidates) { [void]$text.AppendLine("> 候选目标（用词最接近、尚未链接、属于其他部分）：$($candidates -join '；')").AppendLine() }
    }
    [IO.File]::WriteAllText((Join-Path $Staging ("link-{0}.md" -f $group.id)), $text.ToString(), $utf8)
    $summary += [ordered]@{ n = $n; owned = $mine.Count; done = [bool]$done }
}
$unowned = @($agentCards | Where-Object { -not $owner.ContainsKey($_.address) }).Count
[ordered]@{
    pending = @($summary | Where-Object { -not $_.done } | ForEach-Object { $_.n })
    groups = $summary; unowned = $unowned; cards_all = (Join-Path $Staging 'cards-all.md')
} | ConvertTo-Json -Depth 5 -Compress
