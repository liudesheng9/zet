#Requires -Version 7
<#
.SYNOPSIS
  Write the context pack one Sonnet model elaborator starts from: the Model Card, the Cards it
  links and that link it, the book group it came from with the source files and headings to
  read, the anchor Card of that group, and the other models of the book.

.DESCRIPTION
  Reads through `zt mcp` (agent Access, read-only calls). The group is the one whose
  owned-gNN.txt in -Staging lists the Model Card; failing that, the anchor the Model Card is
  filed under (`<key>/<n>`). Source files come from -Staging\manifest.json when it exists; the
  elaborator reads them itself (the pack names files and headings with line numbers, it does
  not copy the book). Writes Markdown to -Out and prints one compact JSON line:
  {"model","group","anchor","sections","parts","context"}.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Staging,
    [Parameter(Mandatory)][string]$Model,
    [Parameter(Mandatory)][string]$Out
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$utf8 = [Text.UTF8Encoding]::new($false)

. (Join-Path $PSScriptRoot 'zt-calls.ps1')

$key = ($Model -split '/', 2)[0]
$card, $structure, $found = Invoke-Zt @(
    @{ tool = 'get_card'; arguments = @{ at = $Model } },
    @{ tool = 'get_model'; arguments = @{ at = $Model } },
    # The book's own tree, not search_cards: a search stops at 200 results.
    @{ tool = 'get_tree'; arguments = @{ tree = $key } }
)

# Which group wrote the Model Card.
$group = $null
foreach ($file in Get-ChildItem -LiteralPath $Staging -Filter 'owned-g*.txt' -File -ErrorAction SilentlyContinue) {
    $owned = Get-Content -LiteralPath $file.FullName -Encoding utf8 | ForEach-Object { $_.Trim() }
    if ($owned -contains $Model) { $group = [int]($file.BaseName -replace '^owned-g', ''); break }
}
if (-not $group -and $Model -match '^[^/]+/(\d+)(\||$)') { $group = [int]$Matches[1] }
$anchor = if ($group) { "$key/$group" } else { $null }

$manifestFile = Join-Path $Staging 'manifest.json'
$manifest = if (Test-Path -LiteralPath $manifestFile) { Get-Content -LiteralPath $manifestFile -Raw -Encoding utf8 | ConvertFrom-Json } else { $null }
$entry = if ($manifest -and $group) { @($manifest.groups) | Where-Object { $_.n -eq $group } | Select-Object -First 1 } else { $null }
$anchorCard = if ($anchor) { try { Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $anchor } }) } catch { $null } } else { $null }

$md = [Text.StringBuilder]::new()
$line = { param($text) [void]$md.AppendLine($text) }
& $line "# 模型上下文：[[$Model]]"
& $line ''
$levelName = switch ($structure.level) { 'concept' { '第 1 层，概念模型（concept）' } 'formal' { '第 2 层，形式模型（formal）' } default { '未定（按模型说明第 0 步决定）' } }
& $line "细化层级：$levelName"
& $line ''
& $line "## 模型卡（DeepSeek 写的原稿，需要对照原文核实）"
& $line ''
& $line "**$($card.title)**"
& $line ''
& $line $card.body.Trim()
& $line ''
if (@($structure.parts).Count) {
    & $line "## 已有的模型分卡"
    & $line ''
    foreach ($part in $structure.parts) {
        $where = if ($part.filed -eq $false) { '（归档在别处，由模型卡的 Link 纳入）' } else { '' }
        $also = @($part.also | ForEach-Object { "[[$($_.address)]]" }) -join '、'
        & $line "- [[$($part.address)]] ``$($part.facet)``$(if ($part.emphasis) { " ($($part.emphasis))" })$where$(if ($also) { "（也属于 $also）" })：$($part.title)"
    }
    & $line ''
    if (@($structure.relations).Count) {
        & $line "已有的关系（类型取自 Link 紧前的词；refers 表示没有关系词）："
        & $line ''
        foreach ($relation in $structure.relations) {
            & $line "- [[$($relation.from)]] → [[$($relation.to)]] ``$($relation.kind)``：$($relation.note)"
        }
        & $line ''
    }
}
$neighbours = @($structure.neighbours)
if ($neighbours.Count) {
    & $line "## 模型外的邻居（与模型的卡互相链接的卡，带关系类型）"
    & $line ''
    foreach ($n in $neighbours) {
        $arrow = "[[$($n.from)]] → [[$($n.to)]]"
        & $line "- $arrow ``$($n.kind)``（$($n.card.title)）：$($n.note)"
    }
    & $line ''
}
$inbound = @($card.inbound) | Where-Object { $_.address -notin @($structure.parts.address) }
if ($inbound.Count) {
    & $line "## 讨论这个模型的卡片（链接到它）"
    & $line ''
    foreach ($ref in $inbound) { & $line "- [[$($ref.address)]]（$($ref.workspace)）$($ref.title)" }
    & $line ''
}
$outbound = @($card.outbound) | Where-Object { -not $_.broken }
if ($outbound.Count) {
    & $line "## 模型卡链接的卡片（改写时保留这些链接）"
    & $line ''
    foreach ($ref in $outbound) { & $line "- [[$($ref.address)]]（$($ref.workspace)）$($ref.title)" }
    & $line ''
}

& $line "## 来源"
& $line ''
$sources = @($card.body -split "`n" | Where-Object { $_ -match '来源' })
if ($sources.Count) { & $line "模型卡的来源行：$($sources -join ' / ')" ; & $line '' }
if ($entry) {
    & $line "第 $group 组（锚点 [[$anchor]]），$($entry.chapter)；小节：$(@($entry.headings) -join '；')。"
    & $line ''
    & $line "DeepSeek 写这张模型卡时读的就是这一组的原文：``$($entry.file)``。小节文件与其中的标题（行号: 标题）："
    & $line ''
    foreach ($name in @($entry.sections)) {
        $path = Join-Path $Staging "sections\$name"
        & $line "- ``$path``"
        if (Test-Path -LiteralPath $path) {
            $n = 0
            foreach ($text in [IO.File]::ReadLines($path)) {
                $n++
                if ($text -match '^#{1,4}\s') { & $line "  - $n`: $($text.Trim())" }
            }
        }
    }
    & $line ''
} elseif ($group) {
    & $line "第 $group 组（锚点 [[$anchor]]）。暂存目录里没有 manifest.json：用 Select-String 在 ``$Staging\sections\*.md`` 或 ``$Staging\book.zh.md`` 里找来源小节；若暂存目录不在，用 scholar_archive_* 工具读原书。"
    & $line ''
} else {
    & $line "找不到这张模型卡所属的组：按来源行在 ``$Staging\book.zh.md`` 里检索，或用 scholar_archive_* 工具读原书。"
    & $line ''
}
foreach ($pair in @(@('全书索引', $(if ($manifest) { $manifest.index } else { Join-Path $Staging 'sections\index.md' })), @('全书合并文本', (Join-Path $Staging 'book.zh.md')), @('双语原文（只读）', $(if ($manifest) { $manifest.source } else { '' })))) {
    if ($pair[1] -and (Test-Path -LiteralPath $pair[1])) { & $line "- $($pair[0])：``$($pair[1])``" }
}
& $line ''

if ($anchorCard) {
    & $line "## 本组锚点卡 [[$anchor]]：这一部分在全书中的位置"
    & $line ''
    & $line "**$($anchorCard.title)**"
    & $line ''
    $body = $anchorCard.body.Trim()
    if ($body.Length -gt 1600) { $body = $body.Substring(0, 1600) + '……' }
    & $line $body
    & $line ''
}

$others = @($found.cards | Where-Object { $_.type -eq 'model' -and -not $_.facet -and $_.address -ne $Model })
if ($others.Count) {
    & $line "## 本书的其他模型（可链接；不要重复它们的内容）"
    & $line ''
    foreach ($other in $others) { & $line "- [[$($other.address)]] $($other.title)$(if ($other.level) { " ($($other.level))" })" }
    & $line ''
}

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Out) | Out-Null
[IO.File]::WriteAllText($Out, $md.ToString(), $utf8)
[ordered]@{
    model = $Model; group = $group; anchor = $anchor
    sections = @(if ($entry) { $entry.sections } else { @() })
    parts = @($structure.parts).Count; context = $Out
} | ConvertTo-Json -Depth 4 -Compress
