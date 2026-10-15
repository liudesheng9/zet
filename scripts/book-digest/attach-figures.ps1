#Requires -Version 7
<#
.SYNOPSIS
  Show the book's own figures in the digest Cards that refer to them, and link the book's
  documents from its Literature Card.

.DESCRIPTION
  For a digested book (Literature tree -CitationKey; its Scholar Archive Markdown -FileId, or the
  digest's -Staging folder, whose manifest names both):
  1. Keeps every figure of the Markdown in ZET's artifact store and reads its labels
     (figures.ps1: `2.C.1`, `IV`/`四`).
  2. Finds the agent Cards of this book that name a figure in their title or body (`图 2.C.1`,
     `图2.C.1`, `Figure 2.C.1`; never `图 2.C.10`). A Card is of this book when it sits in the
     Literature tree or links the Literature Card. Hub Cards, the Literature Card and its anchor
     line (`<key>/<n>`, the section indexes) list other Cards' titles and are left alone.
  3. Puts each figure's `![图 2.C.1](artifact:<sha256>)` line into the Card, as its own paragraph
     right after the paragraph that first names the figure (after a table, list, or chart fence
     that holds the mention; at the top of the body when only the title names it). A Card that
     already shows the figure is skipped, so the script can run again.
  4. Adds `原书文档：[原书 PDF](scholar:…) · …` to the Literature Card when it has no Document
     link yet.
  -FiguresFile takes the figures from a figures.json (prepare-book-digest.ps1 writes one in the
  staging folder) instead of the bridge; -NoDocuments skips step 4. Together they need no Scholar
  Archive.
  Without -Apply it only reports. With -Apply it writes through `zt mcp` (agent Access), passing
  each Card's stored text as `base`, so a Card changed in between is refused, not overwritten.
  Prints a JSON summary; -Report writes the full plan as JSON.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$CitationKey,
    [int]$FileId = 0,
    [string]$Staging = '',
    [string]$Source = '',
    [int]$MinFigureBytes = 3000,
    [string]$Report = '',
    [int]$Batch = 40,
    [string]$FiguresFile = '',
    [switch]$NoDocuments,
    [switch]$Apply
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
. (Join-Path $PSScriptRoot 'figures.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)

if ($Staging) {
    $manifest = Get-Content -LiteralPath (Join-Path $Staging 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
    if (-not $Source) { $Source = $manifest.source }
    if (-not $FileId -and $manifest.file_id) { $FileId = [int]$manifest.file_id }
}
if (-not $FileId -and $Source -match '[\\/]scholar-archive[\\/]files[\\/](\d+)[\\/]') { $FileId = [int]$Matches[1] }
if ($FiguresFile) {
    $figures = @{}
    foreach ($f in (Get-Content -LiteralPath $FiguresFile -Raw -Encoding utf8 | ConvertFrom-Json).figures) {
        $figures[$f.name] = [ordered]@{ sha256 = $f.sha256; path = $f.path; labels = @($f.labels); alt = $f.alt; line = $f.line }
    }
} else {
    if (-not $FileId) { throw 'give -FileId (the book Markdown''s Scholar Archive file id), or a -Staging or -Source that names it' }
    if (-not $Source -or -not (Test-Path -LiteralPath $Source)) {
        $copy = (& zt scholar file $FileId) -join "`n" | ConvertFrom-Json
        if ($LASTEXITCODE -ne 0) { throw "zt scholar file $FileId failed" }
        $Source = $copy.path
    }
    $text = Get-Content -LiteralPath $Source -Raw -Encoding utf8
    $figures = Get-BookFigures -Text $text -FileId $FileId -MinBytes $MinFigureBytes
}

# One entry per label; a label two images share keeps the first.
$byLabel = [ordered]@{}
foreach ($figure in ($figures.Values | Sort-Object { $_.path })) {
    foreach ($label in $figure.labels) { if (-not $byLabel.Contains($label)) { $byLabel[$label] = $figure } }
}

# --- the Cards of this book that name a figure ------------------------------------------------------
$queries = foreach ($label in $byLabel.Keys) {
    foreach ($q in @("图 $label", "图$label")) { @{ tool = 'search_cards'; arguments = @{ query = $q; workspace = 'agent'; limit = 200 } } }
}
$candidates = [Collections.Generic.HashSet[string]]::new()
for ($i = 0; $i -lt $queries.Count; $i += 60) {
    foreach ($found in @(Invoke-Zt @($queries[$i..([Math]::Min($i + 59, $queries.Count - 1))]))) {
        foreach ($hit in $found.results) { [void]$candidates.Add($hit.address) }
    }
}
$anchor = '^' + [regex]::Escape($CitationKey) + '/\d+$'
$inTree = '^' + [regex]::Escape($CitationKey) + '/'
$cites = '[[' + $CitationKey
$addresses = @($candidates | Where-Object { $_ -ne $CitationKey -and $_ -notmatch $anchor })
$cards = [Collections.Generic.List[object]]::new()
for ($i = 0; $i -lt $addresses.Count; $i += $Batch) {
    $calls = @($addresses[$i..([Math]::Min($i + $Batch - 1, $addresses.Count - 1))] | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } })
    foreach ($card in @(Invoke-Zt $calls)) {
        if ($card.workspace -ne 'agent' -or $card.type -eq 'hub') { continue }
        if ($card.address -notmatch $inTree -and -not $card.body.Contains($cites)) { continue }
        $cards.Add([ordered]@{ address = $card.address; title = $card.title; body = [string]$card.body; text = [string]$card.text })
    }
}

# --- where each figure goes in a Card ----------------------------------------------------------------
function Get-MentionText([string]$block) {
    # Image alt text and Link targets are not mentions.
    return ($block -replace '!\[[^\]]*\]\([^)]*\)', '' -replace '\[\[[^\]]*\]\]', '')
}

function Add-FiguresToBody($card) {
    $body = $card.body.Trim()
    $blocks = [Collections.Generic.List[string]]::new()
    foreach ($b in [regex]::Split($body, '\n\s*\n')) { if ($b.Trim()) { $blocks.Add($b) } }
    # Whether a chart or code fence is still open after each block.
    $open = [bool[]]::new($blocks.Count)
    $inside = $false
    for ($b = 0; $b -lt $blocks.Count; $b++) {
        foreach ($line in $blocks[$b] -split "`n") { if ($line -match '^\s*(```|~~~)') { $inside = -not $inside } }
        $open[$b] = $inside
    }
    $after = @{}   # block index (-1: before the first block) -> figure lines, in order
    $added = [Collections.Generic.List[string]]::new()
    $seen = [Collections.Generic.HashSet[string]]::new()
    foreach ($label in $byLabel.Keys) {
        $figure = $byLabel[$label]
        if (-not $seen.Add($figure.sha256)) { continue }
        if ($body.Contains("artifact:$($figure.sha256)")) { continue }
        $patterns = @($figure.labels | ForEach-Object { Get-FigureMentionPattern $_ })
        $named = { param($t) foreach ($p in $patterns) { if ($t -match $p) { return $true } }; return $false }
        $at = $null
        for ($b = 0; $b -lt $blocks.Count; $b++) {
            $openBefore = $b -gt 0 -and $open[$b - 1]
            if (-not $openBefore -and (& $named (Get-MentionText $blocks[$b]))) { $at = $b; break }
        }
        if ($null -eq $at) {
            if (-not (& $named $card.title)) { continue }
            $at = -1
        }
        # A mention in a block that opens a fence goes after the fence closes.
        while ($at -ge 0 -and $open[$at] -and $at + 1 -lt $blocks.Count) { $at++ }
        if (-not $after.ContainsKey($at)) { $after[$at] = [Collections.Generic.List[string]]::new() }
        $after[$at].Add($figure.line)
        $added.Add($figure.alt)
    }
    if (-not $added.Count) { return $null }
    $out = [Collections.Generic.List[string]]::new()
    if ($after.ContainsKey(-1)) { $out.AddRange($after[-1]) }
    for ($b = 0; $b -lt $blocks.Count; $b++) {
        $out.Add($blocks[$b])
        if ($after.ContainsKey($b)) { $out.AddRange($after[$b]) }
    }
    return [ordered]@{ body = ($out -join "`n`n"); figures = @($added) }
}

$plan = [Collections.Generic.List[object]]::new()
foreach ($card in $cards) {
    $change = Add-FiguresToBody $card
    if ($change) { $plan.Add([ordered]@{ address = $card.address; title = $card.title; figures = $change.figures; body = $change.body; base = $card.text }) }
}

# --- the Literature Card's documents --------------------------------------------------------------
$documents = if ($NoDocuments -or -not $FileId) { '' } else { Get-BookDocumentLine -FileId $FileId }
$root = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $CitationKey } })
$rootChange = $null
if ($documents -and $root.body -notmatch '\]\(scholar:\d+') {
    $rootChange = [ordered]@{ address = $CitationKey; body = ($root.body.TrimEnd() + "`n`n" + $documents); base = $root.text }
}

if ($Apply) {
    $edits = @($plan | ForEach-Object { @{ tool = 'edit_card'; arguments = @{ at = $_.address; body = $_.body; base = $_.base } } })
    if ($rootChange) { $edits += @{ tool = 'edit_card'; arguments = @{ at = $rootChange.address; body = $rootChange.body; base = $rootChange.base } } }
    for ($i = 0; $i -lt $edits.Count; $i += $Batch) {
        $null = Invoke-Zt @($edits[$i..([Math]::Min($i + $Batch - 1, $edits.Count - 1))])
    }
}

if ($Report) {
    [IO.File]::WriteAllText($Report, ([ordered]@{ citation_key = $CitationKey; file_id = $FileId; cards = $plan; literature = $rootChange } | ConvertTo-Json -Depth 6), $utf8)
}
[ordered]@{
    citation_key = $CitationKey; file_id = $FileId; applied = [bool]$Apply
    figures = $figures.Count; labelled = @($figures.Values | Where-Object { $_.labels.Count }).Count
    candidates = $candidates.Count; book_cards = $cards.Count; cards = $plan.Count
    shown = [int](@($plan | ForEach-Object { $_.figures.Count }) | Measure-Object -Sum).Sum
    documents = $documents; literature = [bool]$rootChange
    sample = @($plan | Select-Object -First 8 | ForEach-Object { "$($_.address) <- $($_.figures -join ', ')" })
} | ConvertTo-Json -Depth 4 -Compress
