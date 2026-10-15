#Requires -Version 7
<#
.SYNOPSIS
  The figures of a Scholar Archive book, for the book digest. Dot-source it:
  . (Join-Path $PSScriptRoot 'figures.ps1')

.DESCRIPTION
  Get-BookFigures keeps every figure a book's Markdown shows in ZET's artifact store (`zt scholar
  keep-figures`, through the Scholar Archive bridge) and reads each figure's label from the text
  next to the image: a caption line such as `Figure 2.C.1 (left)`, `FIGURE IV`, or `图四`, in the
  image's own block or in the next few short blocks (a MinerU `<details>` description is skipped).
  It returns a hashtable keyed by the image's file name; each value holds:
    sha256, markdown (`![…](artifact:<sha256>)`), labels (e.g. '2.C.1', '2.C.2'; Roman and Chinese
    numerals both: 'IV', '四'), alt (`图 2.C.1、2.C.2`), and line (`![图 2.C.1、2.C.2](artifact:…)`).
  A Card shows a figure by pasting its `line`. ZET holds the bytes, so the Card keeps its figure
  even when the archive moves or drops its copy.

  Get-FigureImageName returns the file name an image line refers to, or $null for a line that is
  not an image.

  Get-FigureMentionPattern returns the regex that finds a mention of one label in Card text
  (`图 2.C.1`, `图2.C.1`, `Figure 2.C.1`), never a longer number (`图 2.C.10`).
#>

$script:ChineseNumerals = @('', '一', '二', '三', '四', '五', '六', '七', '八', '九', '十',
    '十一', '十二', '十三', '十四', '十五', '十六', '十七', '十八', '十九', '二十')
$script:RomanNumerals = @('', 'I', 'II', 'III', 'IV', 'V', 'VI', 'VII', 'VIII', 'IX', 'X',
    'XI', 'XII', 'XIII', 'XIV', 'XV', 'XVI', 'XVII', 'XVIII', 'XIX', 'XX')
$script:FigureLabel = '^\s*(?:FIGURE|Figure|Fig\.?|FIG\.?)\s+([0-9]+(?:[.\-][0-9A-Za-z]+)*|[IVXLC]+)\b|^\s*图\s*([0-9]+(?:[.\-][0-9A-Za-z]+)*|[一二三四五六七八九十]+|[IVXLC]+)'

function Get-FigureImageName([string]$line) {
    if ($line -notmatch '^\s*!\[[^\]]*\]\(([^)\s]+)') { return $null }
    $target = [uri]::UnescapeDataString(($Matches[1] -split '[?#]')[0])
    return ($target -split '[\\/]')[-1]
}

# The labels a caption line gives, with the other numeral spelling of a Roman or Chinese number.
function Get-FigureLabels([string]$line) {
    $labels = @()
    if ($line -cnotmatch $script:FigureLabel) { return $labels }
    $label = if ($Matches[1]) { $Matches[1] } else { $Matches[2] }
    $labels += $label
    $roman = [array]::IndexOf($script:RomanNumerals, $label)
    $chinese = [array]::IndexOf($script:ChineseNumerals, $label)
    if ($roman -gt 0) { $labels += $script:ChineseNumerals[$roman] }
    if ($chinese -gt 0) { $labels += $script:RomanNumerals[$chinese] }
    return $labels
}

function Get-FigureMentionPattern([string]$label) {
    $escaped = [regex]::Escape($label)
    return "(?:图|Figure|Fig\.)\s*$escaped(?!\.?[0-9A-Za-z一二三四五六七八九十])"
}

function Get-BookFigures {
    param(
        [Parameter(Mandatory)][string]$Text,
        [Parameter(Mandatory)][int]$FileId,
        [int]$MinBytes = 3000,
        [string]$Zt = 'zt'
    )
    $out = [IO.Path]::GetTempFileName()
    try {
        $said = & $Zt scholar keep-figures $FileId --min-bytes $MinBytes --out $out 2>&1
        if ($LASTEXITCODE -ne 0) { throw "zt scholar keep-figures $FileId failed: $($said -join ' ')" }
        $kept = Get-Content -LiteralPath $out -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String
    } finally { Remove-Item -LiteralPath $out -ErrorAction SilentlyContinue }

    $figures = @{}
    foreach ($figure in $kept.figures) {
        $figures[$figure.name] = [ordered]@{
            sha256 = $figure.sha256; markdown = $figure.markdown; path = $figure.path
            labels = [Collections.Generic.List[string]]::new(); alt = ''; line = ''
        }
    }

    $blocks = [regex]::Split(($Text -replace "`r`n", "`n"), '\n\s*\n')
    for ($i = 0; $i -lt $blocks.Count; $i++) {
        $lines = $blocks[$i] -split "`n"
        for ($k = 0; $k -lt $lines.Count; $k++) {
            $name = Get-FigureImageName $lines[$k]
            if (-not $name -or -not $figures.ContainsKey($name)) { continue }
            $figure = $figures[$name]
            # The caption: the rest of the image's own block, then the next short blocks.
            $nearby = [Collections.Generic.List[string]]::new()
            for ($m = $k + 1; $m -lt $lines.Count; $m++) {
                if ($lines[$m] -and -not (Get-FigureImageName $lines[$m])) { $nearby.Add($lines[$m]) }
            }
            $inDetails = $false
            for ($j = $i + 1; $j -lt [Math]::Min($blocks.Count, $i + 8); $j++) {
                $block = $blocks[$j].Trim()
                if ($inDetails) { if ($block -match '</details>') { $inDetails = $false }; continue }
                if ($block -match '^<details') { if ($block -notmatch '</details>') { $inDetails = $true }; continue }
                if ($block -match '^#' -or (Get-FigureImageName $block) -or $block.Length -gt 400) { break }
                $block -split "`n" | ForEach-Object { $nearby.Add($_) }
            }
            foreach ($line in $nearby) {
                foreach ($label in (Get-FigureLabels $line)) {
                    if (-not $figure.labels.Contains($label)) { $figure.labels.Add($label) }
                }
            }
        }
    }
    foreach ($figure in $figures.Values) {
        # The alt text names each figure once: an Arabic or Roman label, else the Chinese one.
        $shown = @($figure.labels | Where-Object { $_ -notmatch '^[一二三四五六七八九十]+$' })
        if (-not $shown.Count) { $shown = @($figure.labels) }
        $figure.alt = if ($shown.Count) { '图 ' + ($shown -join '、') } else { '' }
        $figure.line = "![$($figure.alt)](artifact:$($figure.sha256))"
    }
    return $figures
}

# The Work's own documents as Document links, for a Literature Card:
# `原书文档：[原书 PDF](scholar:85) · [中英对照 Markdown](scholar:83) · …`; '' when it has none.
function Get-BookDocumentLine {
    param([Parameter(Mandatory)][int]$FileId, [string]$Zt = 'zt')
    $sql = "SELECT id, kind, chinese_bilingual FROM work_files WHERE work_id = (SELECT work_id FROM work_files WHERE id = $FileId) " +
        "AND availability = 'present' AND kind IN ('pdf', 'markdown') AND shown = 1 ORDER BY kind DESC, chinese_bilingual DESC, id"
    $said = & $Zt scholar sql $sql 2>&1
    if ($LASTEXITCODE -ne 0) { throw "zt scholar sql failed: $($said -join ' ')" }
    $links = foreach ($row in ($said -join "`n" | ConvertFrom-Json).rows) {
        $label = if ($row[1] -eq 'pdf') { '原书 PDF' } elseif ($row[2] -eq 1) { '中英对照 Markdown' } else { '原文 Markdown' }
        "[$label](scholar:$($row[0]))"
    }
    if (-not $links) { return '' }
    return '原书文档：' + ($links -join ' · ')
}
