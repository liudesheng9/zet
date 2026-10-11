#Requires -Version 7
<#
.SYNOPSIS
  Prepare a book for the zt-book-digest workflow: Chinese-only text, section groups, the shared
  DeepSeek instructions, and (with -Apply) the agent Literature tree with one anchor Card per group.

.DESCRIPTION
  - Reads a Chinese bilingual Markdown book (each English block followed by its Chinese
    translation) and keeps the Chinese: a block without CJK characters is dropped when the next
    block is its translation. Formulas, tables, and unpaired blocks are kept; images are dropped.
  - Splits at `#` and `##` headings from -StartHeading up to -EndHeading, merges tiny units into
    the next one, and groups one or two consecutive units of the same chapter per DeepSeek run
    (at most -MaxChars characters per group unless a single unit is larger).
  - Writes to -Out: book.zh.md, groups/gNN.md, shared-prefix.md (identical for every run, so the
    DeepSeek prompt cache can reuse it), and manifest.json.
  - -Apply creates, through `zt mcp` with agent Access, the Literature Card -CitationKey from
    -BibTeX and the anchor line <key>/1 .. <key>/N, one per group. It refuses if the tree exists.
  Prints the manifest as compact JSON.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Source,
    [Parameter(Mandatory)][string]$Out,
    [Parameter(Mandatory)][string]$CitationKey,
    [Parameter(Mandatory)][string]$BibTeX,
    [string]$BookTitle = '',
    [string]$StartHeading = '导论',
    [string]$EndHeading = '参考',
    [int]$MaxChars = 20000,
    [int]$MaxUnits = 3,
    [int]$MinUnitChars = 1000,
    [switch]$Apply
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$utf8 = [Text.UTF8Encoding]::new($false)
$helper = Join-Path $PSScriptRoot '..\..\skills\zt\scripts\zt-mcp.ps1' | Resolve-Path | ForEach-Object Path
$installedHelper = Join-Path $HOME '.codex\skills\zt\scripts\zt-mcp.ps1'
$cjk = '[㐀-鿿豈-﫿]'

# --- Chinese-only blocks -----------------------------------------------------------------------
$raw = (Get-Content -LiteralPath $Source -Raw -Encoding utf8) -replace "`r`n", "`n"
$blocks = [regex]::Split($raw, '\n\s*\n') |
    ForEach-Object { $_.Trim() } | Where-Object { $_ }
$kept = [Collections.Generic.List[string]]::new()
for ($i = 0; $i -lt $blocks.Count; $i++) {
    $block = $blocks[$i]
    if ($block -match '^!\[') { continue }
    $block = ($block -split "`n" | Where-Object { $_ -notmatch '^\s*!\[.*\]\(.*\)\s*$' }) -join "`n"
    if (-not $block.Trim()) { continue }
    if ($block -match $cjk) { $kept.Add($block); continue }
    if ($block -match '^(\$\$|\||<|```)') { $kept.Add($block); continue }
    $next = if ($i + 1 -lt $blocks.Count) { $blocks[$i + 1] } else { '' }
    if ($next -match $cjk) { continue }
    $kept.Add($block)
}

# --- units at # and ## headings ------------------------------------------------------------------
$units = [Collections.Generic.List[object]]::new()
$chapter = ''
$current = $null
$inside = $false
foreach ($block in $kept) {
    if ($block -match '^(#{1,2})\s+(.+)$' -and $block -notmatch "`n") {
        $level = $Matches[1].Length; $heading = $Matches[2].Trim()
        if (-not $inside -and $heading -eq $StartHeading) { $inside = $true }
        if ($inside -and $heading -eq $EndHeading) { break }
        if (-not $inside) { continue }
        if ($level -eq 1) { $chapter = $heading }
        $current = [ordered]@{ chapter = $chapter; heading = $heading; text = [Text.StringBuilder]::new() }
        $units.Add($current)
        [void]$current.text.AppendLine($block).AppendLine()
        continue
    }
    if ($inside -and $current) { [void]$current.text.AppendLine($block).AppendLine() }
}
if (-not $units.Count) { throw "no heading `"$StartHeading`" found in $Source" }

# Merge units shorter than -MinUnitChars (part titles, chapter lead-ins) into the next unit.
$merged = [Collections.Generic.List[object]]::new()
$carry = $null
foreach ($unit in $units) {
    if ($carry) {
        $unit.text.Insert(0, $carry.text.ToString()) | Out-Null
        $unit.headings = @($carry.headings) + $unit.heading
        $carry = $null
    } else { $unit.headings = @($unit.heading) }
    if ($unit.text.Length -lt $MinUnitChars) { $carry = $unit; continue }
    $merged.Add($unit)
}
if ($carry) { $merged[$merged.Count - 1].text.Append($carry.text.ToString()) | Out-Null }

# --- groups of one or two units ------------------------------------------------------------------
$groups = [Collections.Generic.List[object]]::new()
for ($i = 0; $i -lt $merged.Count; $i++) {
    $members = @($merged[$i])
    $size = $merged[$i].text.Length
    while ($members.Count -lt $MaxUnits -and $i + 1 -lt $merged.Count -and
        $merged[$i + 1].chapter -eq $merged[$i].chapter -and
        ($size + $merged[$i + 1].text.Length) -le $MaxChars) {
        $i++; $members += $merged[$i]; $size += $merged[$i].text.Length
    }
    $groups.Add($members)
}

foreach ($dir in 'groups', 'sections') {
    New-Item -ItemType Directory -Force -Path (Join-Path $Out $dir) | Out-Null
    Get-ChildItem -LiteralPath (Join-Path $Out $dir) -Filter '*.md' -File | Remove-Item
}
$staging = (Resolve-Path $Out).Path

# The book stays on disk: one file per unit plus an index. A DeepSeek run receives only its own
# sections in the prompt and reads or searches the rest itself when it needs them.
$book = [Text.StringBuilder]::new()
[void]$book.AppendLine("# 全书正文（中文译本，$StartHeading 至 $EndHeading 之前）").AppendLine()
$unitFile = @{}
for ($u = 0; $u -lt $merged.Count; $u++) {
    $name = 's{0:d2}.md' -f ($u + 1)
    $unitFile[$merged[$u]] = $name
    [IO.File]::WriteAllText((Join-Path $staging "sections\$name"), $merged[$u].text.ToString(), $utf8)
    [void]$book.Append($merged[$u].text.ToString())
}
[IO.File]::WriteAllText((Join-Path $staging 'book.zh.md'), $book.ToString(), $utf8)

# Which other units each group should look at first, so its run needs fewer searches.
. (Join-Path $PSScriptRoot 'relatedness.ps1')
$unitGroup = @{}
for ($g = 0; $g -lt $groups.Count; $g++) { foreach ($unit in $groups[$g]) { $unitGroup[$unitFile[$unit]] = $g + 1 } }
$unitByFile = @{}
foreach ($unit in $merged) { $unitByFile[$unitFile[$unit]] = $unit }
$sectionModel = New-Relatedness @($merged | ForEach-Object { @{ id = $unitFile[$_]; text = $_.text.ToString() } })

$manifest = [ordered]@{
    citation_key = $CitationKey; book_title = $BookTitle; staging = $staging
    shared_prefix = (Join-Path $staging 'shared-prefix.md'); link_prefix = (Join-Path $staging 'link-prefix.md')
    book = (Join-Path $staging 'book.zh.md'); index = (Join-Path $staging 'sections\index.md'); source = $Source; groups = @()
}
$outline = [Text.StringBuilder]::new()
$index = [Text.StringBuilder]::new()
[void]$index.AppendLine("# 全书索引：《$BookTitle》").AppendLine()
[void]$index.AppendLine("每个小节文件位于 ``$staging\sections\``；全书合并文本为 ``$staging\book.zh.md``；双语原文（只读）为 ``$Source``。").AppendLine()
[void]$index.AppendLine('| 文件 | 字数 | 组 | 锚点卡片 | 章 | 小节 |').AppendLine('|---|---|---|---|---|---|')
for ($g = 0; $g -lt $groups.Count; $g++) {
    $n = $g + 1
    $id = 'g{0:d2}' -f $n
    $headings = @($groups[$g] | ForEach-Object { $_.headings } | ForEach-Object { $_ })
    $text = ($groups[$g] | ForEach-Object { $_.text.ToString() }) -join ''
    $file = Join-Path $staging "groups\$id.md"
    $anchor = "$CitationKey/$n"
    $owned = Join-Path $staging "owned-$id.txt"
    $sections = @($groups[$g] | ForEach-Object { $unitFile[$_] })
    foreach ($unit in $groups[$g]) {
        [void]$index.AppendLine("| ``$($unitFile[$unit])`` | $($unit.text.Length) | $n | ``$anchor`` | $($unit.chapter) | $($unit.headings -join '；') |")
    }
    $related = @($sections | ForEach-Object { Find-Related $sectionModel $_ 3 { param($id) $unitGroup[$id] -eq $n } } |
        Select-Object -Unique | Select-Object -First 4 | ForEach-Object {
            $u = $unitByFile[$_]
            "``$_``（第 $($unitGroup[$_]) 组，锚点 ``$CitationKey/$($unitGroup[$_])``）$($u.headings -join '；')"
        })
    $head = @"
# 你的组：第 $n 组

- 锚点卡片：``$anchor``
- 章节：$($groups[$g][0].chapter)；小节：$($headings -join '；')（文件 $($sections -join '、')，原文附在下面）
- 用词最接近的其他小节（先看这些）：$($related -join '；')
- 写入命令：``... | pwsh -NoProfile -File "$installedHelper" - -Log "$owned"``
- 现有主题卡片快照：``$staging\snapshot-$id.md``（本组启动时生成）
- 全书索引：``$($manifest.index)``

## 你的小节原文

"@
    [IO.File]::WriteAllText($file, $head + $text, $utf8)
    [void]$outline.AppendLine("- 第 $n 组 ``$anchor``：$($groups[$g][0].chapter) | $($headings -join '；')")
    $manifest.groups += [ordered]@{ n = $n; id = $id; anchor = $anchor; chapter = $groups[$g][0].chapter; headings = $headings; sections = $sections; chars = $text.Length; file = $file }
}
[IO.File]::WriteAllText($manifest.index, $index.ToString(), $utf8)

foreach ($pair in @(@('dsh-instructions.zh.md', $manifest.shared_prefix), @('dsh-link-instructions.zh.md', $manifest.link_prefix))) {
    $template = Get-Content -LiteralPath (Join-Path $PSScriptRoot $pair[0]) -Raw -Encoding utf8
    $prefix = $template.Replace('{{KEY}}', $CitationKey).Replace('{{TITLE}}', $BookTitle).
        Replace('{{HELPER}}', $installedHelper).Replace('{{STAGING}}', $staging).
        Replace('{{INDEX}}', $manifest.index).Replace('{{SOURCE}}', $Source).
        Replace('{{OUTLINE}}', $outline.ToString().TrimEnd())
    [IO.File]::WriteAllText($pair[1], $prefix, $utf8)
}

# --- agent Literature tree -----------------------------------------------------------------------
if ($Apply) {
    $probe = & pwsh -NoProfile -File $helper -Tool get_card -Json (@{ at = $CitationKey } | ConvertTo-Json -Compress)
    if (($probe -join ' ') -notmatch '^0 error') { throw "Literature tree $CitationKey already exists; delete it or choose another key." }
    $calls = @(@{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = $BibTeX; body = '（全书索引待整理。）' } })
    foreach ($group in $manifest.groups) {
        $at = if ($group.n -eq 1) { $CitationKey } else { "$CitationKey/$($group.n - 1)" }
        $calls += @{ tool = 'create_successor'; arguments = @{ at = $at; kind = 'direct'
            title = "第 $($group.n) 组：$($group.headings -join '；')"
            body = "本卡对应原书 $($group.chapter) 中的：$($group.headings -join '；')。负责该组的代理将把它改写为这一部分的核心论点与小节索引。" } }
    }
    $callsFile = Join-Path $manifest.staging 'setup-calls.json'
    [IO.File]::WriteAllText($callsFile, ($calls | ConvertTo-Json -Depth 10), $utf8)
    $result = & pwsh -NoProfile -File $helper $callsFile
    if ($LASTEXITCODE -ne 0) { throw "zt setup failed: $($result -join ' | ')" }
    $manifest.created = @($result)
}

[IO.File]::WriteAllText((Join-Path $manifest.staging 'manifest.json'), ($manifest | ConvertTo-Json -Depth 10), $utf8)
$manifest | ConvertTo-Json -Depth 10 -Compress
