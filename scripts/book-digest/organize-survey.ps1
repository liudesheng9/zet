#Requires -Version 7.5
<#
.SYNOPSIS
  The survey of the Organize stage (docs/digest-card-layers-proposal.md, proposal 8): everything Opus
  needs to decide, and nothing it must find by reading the trees. Read-only.

.DESCRIPTION
  -Staging is the digest's staging folder (manifest.json). -Work is the Organize folder. The script
  runs digest-audit.ps1 on the book (into <Work>\audit) and writes survey.md (for Opus) and
  survey.json:
    1. Topics: every agent Topic that holds Cards of the book, with its size and the share of the
       book's Cards in it.
    2. Duplicate candidates: pairs of agent Topics, one of them the book's, whose titles are
       similar (character bigram similarity -Similar, 0.5, or one title inside the other) or that
       link each other often (-CrossLinks, 4, Links between their trees).
    3. Small Topics: the book's Topics with -Small (3) Cards or fewer.
    4. Merge proposals of the card cycles (decisions.json in <Staging>\cards*), and the Idea Cards
       the audit names as similar (I6) or short (I2 merge candidate).
    5. Models that are not self-sufficient or have get_model issues, and the architect's notes
       (-Notes: a JSON file of {"model","notes":[...]}, written by the workflow).
    6. Broken Links from the book's Cards, and the hubs the policy requires but the Workspace lacks.
  Prints one compact JSON summary line last.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Staging,
    [Parameter(Mandatory)][string]$Work,
    [string]$Notes = '',
    [double]$Similar = 0.5,
    [int]$CrossLinks = 4,
    [int]$Small = 3
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
New-Item -ItemType Directory -Force -Path $Work | Out-Null
$manifest = Get-Content -LiteralPath (Join-Path $Staging 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
$key = [string]$manifest.citation_key
$auditDir = Join-Path $Work 'audit'
$lines = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot 'digest-audit.ps1') -CitationKey $key -Out $auditDir 2>&1)
if ($LASTEXITCODE -ne 0) { throw "digest-audit failed: $($lines -join ' | ')" }
$audit = Get-Content -LiteralPath (Join-Path $auditDir 'audit.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
$book = @($audit.books)[0]
$cards = @(Get-Content -LiteralPath (Join-Path $auditDir 'audit-cards.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable)
$faults = @(Get-Content -LiteralPath (Join-Path $auditDir 'audit-faults.txt') -Encoding utf8)

function Get-Bigrams([string]$t) {
    $p = ($t -replace '[\s\p{P}\p{S}]', '')
    $set = [Collections.Generic.HashSet[string]]::new()
    for ($i = 0; $i + 1 -lt $p.Length; $i++) { [void]$set.Add($p.Substring($i, 2)) }
    , $set
}
function Get-Similarity($a, $b) {
    if (-not $a.Count -or -not $b.Count) { return 0 }
    $c = 0
    foreach ($x in $a) { if ($b.Contains($x)) { $c++ } }
    2.0 * $c / ($a.Count + $b.Count)
}

# --- Topics --------------------------------------------------------------------------------------
$trees = @((Invoke-Zt @(@{ tool = 'list_trees'; arguments = @{ workspace = 'agent' } })).trees | Where-Object { $_.kind -eq 'topic' })
$mine = @{}
foreach ($t in @($book.topics.list)) { $mine[[string]$t.tree] = $t }
$topics = @($trees | Where-Object { $mine.ContainsKey([string]$_.tree) } | ForEach-Object {
        [ordered]@{ root = [string]$_.root; tree = [string]$_.tree; title = [string]$_.title; size = [int]$_.count; book_cards = [int]$mine[[string]$_.tree].cards; excerpt = [string]$_.excerpt }
    })
# Links between trees, from the book's Cards.
$cross = @{}
foreach ($c in $cards) {
    $from = [string]$c.tree
    foreach ($l in @($c.links)) {
        $to = ([string]$l -split '/', 2)[0]
        if ($to -eq $from -or $to -notmatch '^\d+$' -or $from -notmatch '^\d+$') { continue }
        $pair = @($from, $to) | Sort-Object
        $k = "$($pair[0])~$($pair[1])"
        $cross[$k] = 1 + [int]$cross[$k]
    }
}
$grams = @{}
foreach ($t in $trees) { $grams[[string]$t.tree] = Get-Bigrams ([string]$t.title) }
$pairs = @()
foreach ($a in $topics) {
    foreach ($b in $trees) {
        $bt = [string]$b.tree
        if ($bt -eq $a.tree -or ($mine.ContainsKey($bt) -and [int]$bt -lt [int]$a.tree)) { continue }
        $sim = Get-Similarity $grams[$a.tree] $grams[$bt]
        $inside = $a.title.Contains([string]$b.title) -or ([string]$b.title).Contains($a.title)
        $pair = @($a.tree, $bt) | Sort-Object
        $links = [int]$cross["$($pair[0])~$($pair[1])"]
        if ($sim -ge $Similar -or $inside -or $links -ge $CrossLinks) {
            $pairs += [ordered]@{ a = $a.root; a_title = $a.title; a_size = $a.size; b = [string]$b.root; b_title = [string]$b.title; b_size = [int]$b.count; similarity = [Math]::Round($sim, 2); inside = $inside; links = $links }
        }
    }
}
$pairs = @($pairs | Sort-Object { - $_.similarity }, { - $_.links })
$smallTopics = @($topics | Where-Object { $_.size -le $Small -and $_.book_cards * 2 -gt $_.size })

# --- merge proposals -----------------------------------------------------------------------------
$proposals = @()
foreach ($file in @(Get-ChildItem -LiteralPath $Staging -Directory -Filter 'cards*' -ErrorAction SilentlyContinue | ForEach-Object { Join-Path $_.FullName 'decisions.json' } | Where-Object { Test-Path -LiteralPath $_ })) {
    $d = Get-Content -LiteralPath $file -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    foreach ($m in @($d.merges)) { $proposals += [ordered]@{ card = [string]$m.address; with = [string]$m.merge_with; why = [string]$m.note; from = 'card cycle' } }
    foreach ($n in @($d.orphan_none)) { $proposals += [ordered]@{ card = [string]$n.orphan; with = ''; why = "no related Card: $($n.note)"; from = 'orphan pass' } }
}
foreach ($f in $faults) {
    if ($f -match '^(\S+) I6 \(advisory\) a title very similar to \[\[([^\]]+)\]\]') { $proposals += [ordered]@{ card = $Matches[1]; with = $Matches[2]; why = 'similar titles (I6)'; from = 'audit' } }
    elseif ($f -match '^(\S+) I2 \((\d+) characters < \d+\) merge candidate') { $proposals += [ordered]@{ card = $Matches[1]; with = ''; why = "short Card ($($Matches[2]) characters, I2)"; from = 'audit' } }
}

# --- models and notes ----------------------------------------------------------------------------
$models = @($book.models.list | Where-Object { -not $_.self_sufficient -or $_.issues })
$noteList = @()
if ($Notes -and (Test-Path -LiteralPath $Notes)) { $noteList = @(Get-Content -LiteralPath $Notes -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable | Where-Object { @($_.notes).Count }) }

# --- survey.md -----------------------------------------------------------------------------------
$sb = [Text.StringBuilder]::new()
[void]$sb.AppendLine("# Organize survey: $key").AppendLine()
[void]$sb.AppendLine("《$($manifest.book_title)》: $($book.cards) Cards ($(@($book.cards_by_category.Keys | ForEach-Object { "$_ $($book.cards_by_category[$_])" }) -join ', ')). Anchors $key/1 … $key/$(@($manifest.groups).Count).").AppendLine()
[void]$sb.AppendLine('## 1. The book''s Topics').AppendLine()
[void]$sb.AppendLine('| Topic | Title | Cards | of the book |').AppendLine('| --- | --- | --- | --- |')
foreach ($t in ($topics | Sort-Object { - $_.book_cards })) { [void]$sb.AppendLine("| ``$($t.root)`` | $($t.title) | $($t.size) | $($t.book_cards) |") }
[void]$sb.AppendLine().AppendLine('## 2. Duplicate candidates').AppendLine()
if ($pairs.Count) {
    [void]$sb.AppendLine('| Topic A | Topic B | similarity | one inside the other | Links between |').AppendLine('| --- | --- | --- | --- | --- |')
    foreach ($p in $pairs) { [void]$sb.AppendLine("| ``$($p.a)`` $($p.a_title) ($($p.a_size)) | ``$($p.b)`` $($p.b_title) ($($p.b_size)) | $($p.similarity) | $($p.inside) | $($p.links) |") }
} else { [void]$sb.AppendLine('None.') }
[void]$sb.AppendLine().AppendLine("## 3. Small Topics (≤ $Small Cards)").AppendLine()
if ($smallTopics.Count) { foreach ($t in $smallTopics) { [void]$sb.AppendLine("- ``$($t.root)`` $($t.title) ($($t.size) Cards)") } } else { [void]$sb.AppendLine('None.') }
[void]$sb.AppendLine().AppendLine('## 4. Card merge proposals').AppendLine()
if ($proposals.Count) {
    foreach ($p in $proposals) {
        $with = if ($p.with) { ' with `' + $p.with + '`' } else { '' }
        [void]$sb.AppendLine("- ``$($p.card)``$($with): $($p.why) ($($p.from))")
    }
} else { [void]$sb.AppendLine('None.') }
[void]$sb.AppendLine().AppendLine('## 5. Models').AppendLine()
[void]$sb.AppendLine("$($book.models.count) Model Cards ($(@($book.models.levels.Keys | ForEach-Object { "$_ $($book.models.levels[$_])" }) -join ', ')); $($book.models.not_self_sufficient) not self-sufficient.").AppendLine()
foreach ($m in $models) { [void]$sb.AppendLine("- ``$($m.address)`` $($m.title): level $($m.level), faults $(($m.faults | ConvertTo-Json -Compress)), issues $($m.issues)") }
if ($noteList.Count) {
    [void]$sb.AppendLine().AppendLine('Architect notes (Cards outside a model that should link a part of it):').AppendLine()
    foreach ($n in $noteList) { foreach ($t in @($n.notes)) { [void]$sb.AppendLine("- ``$($n.model)``: $t") } }
}
[void]$sb.AppendLine().AppendLine('## 6. Links and hubs').AppendLine()
[void]$sb.AppendLine("- Broken Links from the book's Cards: $($book.broken_links). In the Workspace: $($audit.workspace.broken_links).")
[void]$sb.AppendLine("- Required hubs missing in the Workspace: $($audit.workspace.hubs.missing) (the hub stage builds them after Organize).")
[void]$sb.AppendLine("- Card language faults left: $($book.style.faults); Card layer faults left: $($book.layer.faults); orphans: $($book.orphans.count).")
[IO.File]::WriteAllText((Join-Path $Work 'survey.md'), $sb.ToString(), $utf8)
$survey = [ordered]@{ key = $key; title = [string]$manifest.book_title; topics = $topics; pairs = $pairs; small = $smallTopics; proposals = $proposals; models = $models; notes = $noteList; broken_links = $book.broken_links; hubs_missing = $audit.workspace.hubs.missing }
[IO.File]::WriteAllText((Join-Path $Work 'survey.json'), ($survey | ConvertTo-Json -Depth 8), $utf8)
[ordered]@{ survey = (Join-Path $Work 'survey.md'); topics = $topics.Count; pairs = $pairs.Count; small = $smallTopics.Count; proposals = $proposals.Count; models = $models.Count; notes = $noteList.Count; broken_links = $book.broken_links } | ConvertTo-Json -Compress
