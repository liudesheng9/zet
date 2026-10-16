#Requires -Version 7
<#
.SYNOPSIS
  Prepare a book for the ZT digest workflows: Chinese-only text, section groups, the shared
  DeepSeek instructions, and (with -Apply) the agent Literature tree with one anchor Card per group.

.DESCRIPTION
  - Reads a Chinese bilingual Markdown book (each English block followed by its Chinese
    translation) and keeps the Chinese: a block without CJK characters is dropped when the next
    block is its translation. Formulas, tables, and unpaired blocks are kept.
  - Keeps the book's figures: every image of the Markdown (-FileId, its Scholar Archive file id,
    read from a ZET cache path such as ...\scholar-archive\files\83\... when not given) of at
    least -MinFigureBytes is taken into ZET's artifact store through the bridge (figures.ps1), and
    its image line becomes `![图 2.C.1](artifact:<sha256>)`, which a Card can paste to show the
    figure. The caption lines under it stay; the same figure repeated in the next blocks (the
    bilingual copy) is written once. figures.json lists every figure with its labels. Images
    below the size, and every image under -NoFigures, are dropped.
  - Splits at `#` and `##` headings from -StartHeading up to -EndHeading, merges tiny units into
    the next one, and groups one or two consecutive units of the same chapter per DeepSeek run
    (at most -MaxChars characters per group unless a single unit is larger). Units under a heading
    in -SkipHeadings (comma-separated, e.g. a chapter's '参考文献,习题') are left out up to the next
    other heading.
  - Writes to -Out: book.zh.md, groups/gNN.md, shared-prefix.md (identical for every run, so the
    DeepSeek prompt cache can reuse it), and manifest.json.
  - -Instructions (comma-separated instruction files of this folder, default dsh-instructions.zh.md)
    builds shared-prefix.md: the first file, with each further file inserted before its `## 回复`
    section. The framework digests pass dsh-digest-base.zh.md plus their book type's files
    (docs/zt-digest-framework.md). -NoEventCandidates leaves the event candidates out of the group
    files (events.json still lists them, for the event layer).
  - -Apply creates, through `zt mcp` with agent Access, the Literature Card -CitationKey from
    -BibTeX and the anchor line <key>/1 .. <key>/N, one per group. It refuses if the tree exists.
    The Literature Card's body links the Work's PDF and Markdown documents with Document links
    (`[原书 PDF](scholar:<file_id>)`), which the GUI opens.
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
    [string]$SkipHeadings = '',
    [int]$MaxChars = 20000,
    [int]$MaxUnits = 3,
    [int]$MinUnitChars = 1000,
    [int]$FileId = 0,
    [int]$MinFigureBytes = 3000,
    [int]$MaxEventCandidates = 12,
    [string]$Instructions = 'dsh-instructions.zh.md',
    [switch]$NoEventCandidates,
    [switch]$NoFigures,
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
. (Join-Path $PSScriptRoot 'figures.ps1')
if (-not $FileId -and $Source -match '[\\/]scholar-archive[\\/]files[\\/](\d+)[\\/]') { $FileId = [int]$Matches[1] }
$figures = @{}
if ($FileId -and -not $NoFigures) { $figures = Get-BookFigures -Text $raw -FileId $FileId -MinBytes $MinFigureBytes }
$blocks = [regex]::Split($raw, '\n\s*\n') |
    ForEach-Object { $_.Trim() } | Where-Object { $_ }
$kept = [Collections.Generic.List[string]]::new()
# Where each figure was last written: the bilingual copy right after it is not written again.
$figureAt = @{}
for ($i = 0; $i -lt $blocks.Count; $i++) {
    $block = $blocks[$i]
    $lines = [Collections.Generic.List[string]]::new()
    $shown = $false
    foreach ($line in $block -split "`n") {
        $name = Get-FigureImageName $line
        if (-not $name) { $lines.Add($line); continue }
        $figure = $figures[$name]
        if (-not $figure) { continue }
        if ($figureAt.ContainsKey($name) -and $kept.Count - $figureAt[$name] -le 3) { continue }
        $figureAt[$name] = $kept.Count
        $lines.Add($figure.line)
        $shown = $true
    }
    $block = ($lines -join "`n").Trim()
    if (-not $block) { continue }
    if ($shown) { $kept.Add($block); continue }
    if ($block -match $cjk) { $kept.Add($block); continue }
    if ($block -match '^(\$\$|\||<|```)') { $kept.Add($block); continue }
    $next = if ($i + 1 -lt $blocks.Count) { $blocks[$i + 1] } else { '' }
    if ($next -match $cjk) { continue }
    $kept.Add($block)
}

# --- units at # and ## headings ------------------------------------------------------------------
$units = [Collections.Generic.List[object]]::new()
$skip = @($SkipHeadings -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$chapter = ''
$current = $null
$inside = $false
foreach ($block in $kept) {
    if ($block -match '^(#{1,2})\s+(.+)$' -and $block -notmatch "`n") {
        $level = $Matches[1].Length; $heading = $Matches[2].Trim()
        if (-not $inside -and $heading -eq $StartHeading) { $inside = $true }
        if ($inside -and $heading -eq $EndHeading) { break }
        if (-not $inside) { continue }
        if ($heading -in $skip) { $current = $null; continue }
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
    file_id = $FileId; figures = $figures.Count; figures_file = (Join-Path $staging 'figures.json')
    documents = if ($FileId) { Get-BookDocumentLine -FileId $FileId } else { '' }
}
# Every kept figure with its labels: the model compiler hands a part the figure its plan names.
$figureList = @($figures.GetEnumerator() | Sort-Object { $_.Value.path } | ForEach-Object {
        [ordered]@{ name = $_.Key; labels = @($_.Value.labels); alt = $_.Value.alt; line = $_.Value.line; sha256 = $_.Value.sha256; path = $_.Value.path } })
[IO.File]::WriteAllText($manifest.figures_file, ([ordered]@{ file_id = $FileId; figures = $figureList } | ConvertTo-Json -Depth 6), $utf8)
$outline = [Text.StringBuilder]::new()
$eventCandidates = @()
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
    # Event candidates (proposal 4): the lines of this group's sections that name a year. The run
    # decides which are real events; a hypothetical example in a model is none.
    $candidates = @()
    foreach ($unit in $groups[$g]) {
        $lineNo = 0
        foreach ($line in ($unit.text.ToString() -split "`n")) {
            $lineNo++
            if ($candidates.Count -ge $MaxEventCandidates) { break }
            if ($line -notmatch '[㐀-鿿]' -or $line -match '^\s*(!\[|\||#|\d+\))') { continue }
            # A citation's year (`Helper [1991]`, `Holmstrom (1979)`) names a paper, not an event.
            $plain = [regex]::Replace($line, '\\?\[[^\]]*\d{4}[^\]]*\\?\]|[（(]\s*\d{4}[a-z]?\s*[)）]', '')
            if ($plain -match '(1[5-9]\d\d|20[0-2]\d)\s*年(?!代)') {
                $clip = ($line -replace '\s+', ' ').Trim()
                if ($clip.Length -gt 120) { $clip = $clip.Substring(0, 120) + '…' }
                $candidates += [ordered]@{ group = $n; src = "sections/$($unitFile[$unit]):$lineNo"; text = $clip }
            }
        }
    }
    $eventCandidates += $candidates
    $eventLines = if ($candidates.Count) { ($candidates | ForEach-Object { "  - ``$($_.src)`` $($_.text)" }) -join "`n" } else { '  - （没有）' }
    $srcLog = Join-Path $staging "src-$id.txt"
    $gate = Join-Path $staging 'topic-gate.lock'
    $head = @"
# 你的组：第 $n 组

- 锚点卡片：``$anchor``
- 章节：$($groups[$g][0].chapter)；小节：$($headings -join '；')（文件 $($sections -join '、')，原文附在下面）
- 用词最接近的其他小节（先看这些）：$($related -join '；')
- 写入命令：``... | pwsh -NoProfile -File "$installedHelper" - -Log "$owned" -SrcLog "$srcLog" -TopicGate "$gate"``
- SELF-CHECK: ``zt-check --cards "$anchor" --log "$owned" --book $CitationKey``
- 现有主题卡片快照：``$staging\snapshot-$id.md``（本组启动时生成）
- 全书索引：``$($manifest.index)``
- 本书术语表：``$staging\terms.md``（存在时先读它，按它用词）
$(if (-not $NoEventCandidates) { "- 事件候选（本组小节中写出年份的句子；由你判断哪些是真实事件）：`n$eventLines`n" })
## 你的小节原文

"@
    [IO.File]::WriteAllText($file, $head + $text, $utf8)
    [void]$outline.AppendLine("- 第 $n 组 ``$anchor``：$($groups[$g][0].chapter) | $($headings -join '；')")
    $manifest.groups += [ordered]@{ n = $n; id = $id; anchor = $anchor; chapter = $groups[$g][0].chapter; headings = $headings; sections = $sections; chars = $text.Length; file = $file }
}
[IO.File]::WriteAllText($manifest.index, $index.ToString(), $utf8)
[IO.File]::WriteAllText((Join-Path $staging 'events.json'), (ConvertTo-Json -InputObject @($eventCandidates) -Depth 4), $utf8)
$manifest.event_candidates = $eventCandidates.Count

foreach ($pair in @(@($Instructions, $manifest.shared_prefix), @('dsh-link-instructions.zh.md', $manifest.link_prefix))) {
    # The first file is the template; each further file goes in before its "## 回复" section.
    $files = @($pair[0] -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    $template = Get-Content -LiteralPath (Join-Path $PSScriptRoot $files[0]) -Raw -Encoding utf8
    foreach ($extra in @($files | Select-Object -Skip 1)) {
        $part = (Get-Content -LiteralPath (Join-Path $PSScriptRoot $extra) -Raw -Encoding utf8).TrimEnd() + "`n`n"
        $at = $template.IndexOf("`n## 回复")
        $template = if ($at -ge 0) { $template.Substring(0, $at + 1) + $part + $template.Substring($at + 1) } else { $template.TrimEnd() + "`n`n" + $part }
    }
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
    $body = '（全书索引待整理。）' + $(if ($manifest.documents) { "`n`n$($manifest.documents)" })
    $calls = @(@{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = $BibTeX; body = $body } })
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
