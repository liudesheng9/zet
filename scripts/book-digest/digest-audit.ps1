#Requires -Version 7
<#
.SYNOPSIS
  Audit the Cards of a digested book, or of every book in the agent Workspace. Read-only: the
  script makes only zt read calls and changes no Card.

.DESCRIPTION
  Phase 1 of docs/digest-card-layers-proposal.md. For each book (an agent Literature tree), the
  script counts and lists:
    cards        the Cards of the book by category (root, anchor, idea, event, model, part, hub)
                 and Zone. A Card in a Topic tree is of the book whose Literature tree it links most.
                 A Link to the Literature Card or an anchor (a source line) counts 10, a Link to
                 another Card of that tree counts 1. A Topic-tree Card that links no book is
                 "unattributed".
    topics       the Topic trees that hold Cards of the book, and the trees where the book holds
                 most of the Cards.
    style        the Card language faults (zt-check, rules R2 to R17) per rule and per
                 category. A model is checked with its own term table and level, the other Cards of
                 a book with the book's term table (a Literature-tree Card titled 术语表…) when it
                 has one.
    layer        the Card layer faults (rules I1 to L3 of zt-check) per rule, with the
                 advisory rules (I1, I6, E5, L3) counted apart. The script gives the checker each
                 Card's category, Links, kinded Links, parent, and inbound Links.
    orphans      Regular Cards that no Card links to (inbound 0).
    no_source    idea and event Cards with no source line: no Link to the book's Literature Card or
                 to one of its anchors.
    unkinded     idea and event Cards with no kinded Link to a Card other than their parent, and
                 the count of Links that ZT reads as "refers" (Links to a Literature Card or an
                 anchor are source lines and do not count).
    models       each Model Card's level, parts, and build faults. A formal model is
                 self-sufficient when build.closure, build.forward, and build.unanchored are
                 empty. A concept model also needs an empty build.beyond_level.
    figures      Cards that name a figure (`图 2.C.1`) but do not show it (`![图 2.C.1](artifact:…)`).
                 "known" means another Card of the book shows that figure, so ZET holds it.
                 "unknown" means no Card of the book shows it.
  For the Workspace it adds the hub counts, the missing hubs (list_hubs), and the broken Links.
  Writes audit.json, audit.md, audit-faults.txt (every Card language and Card layer fault), and
  audit-cards.json (every Card checked outside a model, with its body and Links) in -Out, and
  prints one compact JSON summary line last.
#>
[CmdletBinding()]
param(
    # One book. Empty: every agent Literature tree.
    [string]$CitationKey = '',
    [Parameter(Mandatory)][string]$Out,
    [int]$MaxSentences = 12,
    [int]$MaxChars = 400,
    [int]$MaxSteps = 6,
    [int]$Throttle = 6,
    [int]$Chunk = 40,
    # Addresses listed per finding in audit.md (audit.json lists all).
    [int]$Sample = 15,
    # Skip the models: no get_model, no model checks, no bodies of Model Cards and parts (card-compiler.ps1).
    [switch]$SkipModels
)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
New-Item -ItemType Directory -Force -Path $Out | Out-Null

# --- read-only zt calls --------------------------------------------------------------------------
$readTools = @('list_trees', 'get_tree', 'get_card', 'get_model', 'get_hub', 'list_hubs', 'list_broken_links', 'search_cards')
function Read-Zt($calls) {
    foreach ($c in $calls) { if ($c.tool -notin $readTools) { throw "digest-audit is read-only: refused $($c.tool)" } }
    Invoke-Zt $calls
}
# Many reads in parallel helper processes; results in call order, $null for a call that fails.
function Read-Many($calls) {
    $calls = @($calls)
    foreach ($c in $calls) { if ($c.tool -notin $readTools) { throw "digest-audit is read-only: refused $($c.tool)" } }
    if (-not $calls.Count) { return @() }
    $chunks = for ($i = 0; $i -lt $calls.Count; $i += $Chunk) { , @{ i = $i; calls = $calls[$i..([Math]::Min($i + $Chunk - 1, $calls.Count - 1))] } }
    $root = $PSScriptRoot
    $done = $chunks | ForEach-Object -ThrottleLimit $Throttle -Parallel {
        . (Join-Path $using:root 'zt-calls.ps1')
        $part = $_
        try { $r = @(Invoke-Zt $part.calls) } catch { $r = @() }
        if ($r.Count -ne $part.calls.Count) {
            # One failing call stops the helper: read the chunk again call by call.
            $r = foreach ($c in $part.calls) { try { @(Invoke-Zt @($c))[0] } catch { $null } }
        }
        @{ i = $part.i; r = @($r) }
    }
    $results = [object[]]::new($calls.Count)
    foreach ($d in $done) { for ($k = 0; $k -lt $d.r.Count; $k++) { $results[$d.i + $k] = $d.r[$k] } }
    , $results
}
function Get-Quantiles($values) {
    $v = @($values | Sort-Object)
    if (-not $v.Count) { return [ordered]@{ n = 0 } }
    $at = { param($q) $v[[Math]::Min($v.Count - 1, [int][Math]::Floor($q * ($v.Count - 1)))] }
    [ordered]@{ n = $v.Count; min = $v[0]; p25 = (& $at 0.25); median = (& $at 0.5); p75 = (& $at 0.75); max = $v[-1] }
}
function Get-Tally($items) {
    $t = [ordered]@{}
    foreach ($g in @($items | Group-Object | Sort-Object Count -Descending)) { $t[$g.Name] = $g.Count }
    $t
}

# --- the trees -----------------------------------------------------------------------------------
$sw = [Diagnostics.Stopwatch]::StartNew()
$trees = @((Read-Zt @(@{ tool = 'list_trees'; arguments = @{ workspace = 'agent' } })).trees)
$books = @($trees | Where-Object { $_.kind -eq 'literature' } | ForEach-Object { [string]$_.tree })
if ($CitationKey) {
    if ($CitationKey -notin $books) { throw "no agent Literature tree ``$CitationKey``" }
    $books = @($CitationKey)
}
$allBooks = @($trees | Where-Object { $_.kind -eq 'literature' } | ForEach-Object { [string]$_.tree })
$treeInfo = @{}
foreach ($t in $trees) { $treeInfo[[string]$t.tree] = $t }
$topicTrees = @($trees | Where-Object { $_.kind -eq 'topic' } | ForEach-Object { [string]$_.tree })
$read = Read-Many @(@($allBooks + $topicTrees) | ForEach-Object { @{ tool = 'get_tree'; arguments = @{ tree = $_ } } })
$cards = @($read | Where-Object { $_ } | ForEach-Object { $_.cards } | Where-Object { $_.workspace -eq 'agent' })
$byAddress = @{}
foreach ($c in $cards) { $byAddress[[string]$c.address] = $c }
Write-Host "read $($cards.Count) Cards in $($trees.Count) trees ($([int]$sw.Elapsed.TotalSeconds) s)"

# The book a Card's Link points into: `Key`, `Key/…`.
function Get-LinkBook([string]$target) {
    $head = ($target -split '/', 2)[0]
    if ($head -in $allBooks) { $head } else { $null }
}
function Test-SourceTarget([string]$target) {
    # A Literature Card or an anchor (`Key/<n>`): a source line or a place in the book.
    $head = ($target -split '/', 2)[0]
    ($head -in $allBooks) -and ($target -eq $head -or $target -match '^[^/]+/\d+$')
}
function Get-Category($c) {
    if ($c.kind -in @('topic', 'literature')) { return 'root' }
    switch ([string]$c.type) {
        'hub' { return 'hub' }
        'event' { return 'event' }
        'model' { if ($c.facet) { return 'part' } else { return 'model' } }
    }
    if ([string]$c.tree -in $allBooks -and [string]$c.address -match '^[^/]+/\d+$') { return 'anchor' }
    # The book's term table (book-terms.ps1): a table, not an idea.
    if ([string]$c.tree -in $allBooks -and [string]$c.title -match '^术语表') { return 'terms' }
    # The book's structure Card (book-skeleton.ps1): it lists the argument; its Links are an index.
    if ([string]$c.tree -in $allBooks -and [string]$c.excerpt -match '^本卡是《.*》的结构卡') { return 'structure' }
    # An older structure Card has no marker: a Side successor of <key>/1 that links at least half
    # of the book's anchors.
    if ([string]$c.tree -in $allBooks -and [string]$c.parent -eq "$($c.tree)/1" -and [string]$c.edge -eq 'side' -and [string]$c.type -eq 'idea') {
        $anchorLinks = @(@($c.links) | Where-Object { [string]$_ -match ('^' + [regex]::Escape([string]$c.tree) + '/\d+$') } | Select-Object -Unique).Count
        if ($anchorLinks -ge 3 -and $anchorLinks * 2 -ge [int]$anchorCount[[string]$c.tree]) { return 'structure' }
    }
    'idea'
}

# Which book each Card belongs to.
# Anchors per book: an older structure Card links at least half of them.
$anchorCount = @{}
foreach ($c in $cards) { if ([string]$c.tree -in $allBooks -and [string]$c.address -match '^[^/]+/\d+$') { $anchorCount[[string]$c.tree] = 1 + [int]$anchorCount[[string]$c.tree] } }
$bookOf = @{}
foreach ($c in $cards) {
    $address = [string]$c.address
    $c | Add-Member -NotePropertyName category -NotePropertyValue (Get-Category $c) -Force
    if ([string]$c.tree -in $allBooks) { $bookOf[$address] = [string]$c.tree; continue }
    $votes = @{}
    # A source line (`来源：[[Key]] §n`) or an anchor names the book. Another Card of the book only
    # suggests it: a Card of one book often links the Cards of another.
    foreach ($l in @($c.links)) { $b = Get-LinkBook ([string]$l); if ($b) { $votes[$b] = $(if (Test-SourceTarget ([string]$l)) { 10 } else { 1 }) + [int]$votes[$b] } }
    if ($votes.Count) { $bookOf[$address] = @($votes.GetEnumerator() | Sort-Object { -$_.Value }, Name)[0].Name }
}
$scope = @($cards | Where-Object { $bookOf[[string]$_.address] -in $books })
# Topic roots and Hub Cards seldom link a book. A Workspace audit checks them too, in their own section.
$unattributed = @($cards | Where-Object { -not $bookOf.ContainsKey([string]$_.address) })
if (-not $CitationKey) { $scope += $unattributed }

# --- bodies: get_model for each Model Card, get_card for the rest ---------------------------------
$heads = if ($SkipModels) { @() } else { @($scope | Where-Object { $_.category -eq 'model' }) }
$structures = Read-Many @($heads | ForEach-Object { @{ tool = 'get_model'; arguments = @{ at = [string]$_.address } } })
$bodies = @{}
$facets = @{}
for ($i = 0; $i -lt $heads.Count; $i++) {
    $m = $structures[$i]
    if (-not $m) { continue }
    if ($m.head) { $bodies[[string]$m.head.address] = @{ title = [string]$m.head.title; body = [string]$m.head.body } }
    foreach ($p in @($m.parts)) { $bodies[[string]$p.address] = @{ title = [string]$p.title; body = [string]$p.body }; $facets[[string]$p.address] = [string]$p.facet }
}
$missing = @($scope | Where-Object { -not $bodies.ContainsKey([string]$_.address) -and -not ($SkipModels -and $_.category -in @('model', 'part')) })
$got = Read-Many @($missing | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = [string]$_.address } } })
$unread = @()
for ($i = 0; $i -lt $missing.Count; $i++) {
    if ($got[$i]) { $bodies[[string]$missing[$i].address] = @{ title = [string]$got[$i].title; body = [string]$got[$i].body } }
    else { $unread += [string]$missing[$i].address }
}
Write-Host "read $($bodies.Count) bodies, $($structures.Count) models ($([int]$sw.Elapsed.TotalSeconds) s)"

# --- Card language: zt-check --json, offline -------------------------
$tmp = Join-Path $Out 'audit-tmp'
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$limits = @{ MaxSentences = $MaxSentences; MaxChars = $MaxChars; MaxSteps = $MaxSteps }
$faultLines = [Collections.Generic.HashSet[string]]::new()
function Invoke-Check($entries, [string]$setting, [string]$level) {
    if (-not @($entries).Count) { return }
    $file = Join-Path $tmp 'cards.json'
    [IO.File]::WriteAllText($file, (ConvertTo-Json -InputObject @($entries) -Depth 4), $utf8)
    $splat = @{ Json = $file } + $limits
    if ($setting) { $splat.Setting = $setting }
    if ($level) { $splat.Level = $level }
    foreach ($line in @(Invoke-ZtCheck @splat)) { if ($line -match '^\S+ [RIEL]\d+ ') { [void]$faultLines.Add([string]$line) } }
}
$inModel = @{}
for ($i = 0; $i -lt $heads.Count; $i++) {
    $m = $structures[$i]
    if (-not $m -or -not $m.head) { continue }
    $line = @{}
    foreach ($o in @($m.build.order)) { $line[[string]$o.address] = [string]$o.line }
    $entries = @([ordered]@{ address = [string]$m.head.address; title = [string]$m.head.title; body = [string]$m.head.body; facet = $null; line = 'head' })
    foreach ($p in @($m.parts)) {
        $entries += [ordered]@{ address = [string]$p.address; title = [string]$p.title; body = [string]$p.body; facet = [string]$p.facet; line = $line[[string]$p.address] }
        $inModel[[string]$p.address] = $true
    }
    $inModel[[string]$m.head.address] = $true
    Invoke-Check $entries ([string]$m.build.setting) ([string]$m.level)
}
# The link graph of the whole Workspace: who links each Card, and how many Idea Cards link it with a
# kind word. The Card layer rules (I5, E4) read it.
$inboundFrom = @{}
$kindedFrom = @{}
foreach ($c in $cards) {
    $kinded = @(@($c.kinds.PSObject.Properties) | ForEach-Object Name)
    foreach ($l in @($c.links | Select-Object -Unique)) {
        $t = [string]$l
        if (-not $inboundFrom.ContainsKey($t)) { $inboundFrom[$t] = [Collections.Generic.List[string]]::new() }
        $inboundFrom[$t].Add([string]$c.address)
        if ($c.category -eq 'idea' -and $t -in $kinded) { $kindedFrom[$t] = 1 + [int]$kindedFrom[$t] }
    }
}
# A book's term table: a Card of its Literature tree whose title starts with 术语表 (proposal 2).
$termCard = @{}
foreach ($c in $cards) { if ([string]$c.tree -in $allBooks -and [string]$c.title -match '^术语表' -and -not $termCard.ContainsKey([string]$c.tree)) { $termCard[[string]$c.tree] = [string]$c.address } }
$rest = @($scope | Where-Object { -not $inModel.ContainsKey([string]$_.address) -and $bodies.ContainsKey([string]$_.address) } | ForEach-Object {
    $b = $bodies[[string]$_.address]
    $a = [string]$_.address
    [ordered]@{
        address = $a; title = $b.title; body = $b.body; facet = $facets[$a]; line = $null
        category = $_.category; tree = [string]$_.tree; book = $bookOf[$a]; parent = [string]$_.parent
        links = @($_.links | ForEach-Object { [string]$_ }); kinds = @(@($_.kinds.PSObject.Properties) | ForEach-Object Name)
        inbound = @($(if ($inboundFrom.ContainsKey($a)) { $inboundFrom[$a] } else { @() })); inbound_kinded = [int]$kindedFrom[$a]
    }
})
# The Cards the layer rules read, with their bodies: card-compiler.ps1 builds its jobs from them.
[IO.File]::WriteAllLines((Join-Path $Out 'audit-addresses.txt'), [string[]]@($cards | ForEach-Object { [string]$_.address }), $utf8)
[IO.File]::WriteAllText((Join-Path $Out 'audit-cards.json'), (ConvertTo-Json -InputObject @($rest) -Depth 5), $utf8)
# One call per book, with the book's term table (R6) when it has one; I6 and E5 compare within it.
foreach ($g in @($rest | Group-Object { [string]$_.book })) { Invoke-Check $g.Group $(if ($g.Name -and $termCard.ContainsKey($g.Name)) { $termCard[$g.Name] } else { '' }) '' }
Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
$faults = @($faultLines | ForEach-Object { $p = $_ -split ' ', 3; [pscustomobject]@{ address = $p[0]; rule = $p[1]; text = $p[2]; line = $_ } })
Write-Host "Card language: $($faults.Count) faults ($([int]$sw.Elapsed.TotalSeconds) s)"

# --- the Workspace -------------------------------------------------------------------------------
$hubList = @(Read-Zt @(@{ tool = 'list_hubs'; arguments = @{ workspace = 'agent' } }, @{ tool = 'list_broken_links'; arguments = @{ workspace = 'agent' } }))
$hubs = $hubList[0]
$broken = @($hubList[1].broken)
$brokenFrom = @($broken | ForEach-Object { if ($_.from) { [string]$_.from } elseif ($_.source) { [string]$_.source } elseif ($_.address) { [string]$_.address } })

# --- per book ------------------------------------------------------------------------------------
$figureName = '图\s?((?:\d+(?:\.[0-9A-Za-z]+)*)|[IVXL]+|[一二三四五六七八九十]+(?![㐀-鿿]))(?![0-9A-Za-z.])'
$report = [ordered]@{
    generated = (Get-Date).ToString('yyyy-MM-dd HH:mm'); scope = $(if ($CitationKey) { $CitationKey } else { 'agent' })
    limits = $limits; seconds = 0
    workspace = [ordered]@{
        trees = $trees.Count; topic_trees = $topicTrees.Count; books = $allBooks.Count; cards = $cards.Count
        cards_by_category = Get-Tally @($cards | ForEach-Object category)
        topic_tree_sizes = Get-Quantiles @($trees | Where-Object { $_.kind -eq 'topic' } | ForEach-Object { [int]$_.count })
        hubs = [ordered]@{ counts = $hubs.counts; missing = @($hubs.missing).Count; with_faults = @($hubs.hubs | Where-Object { @($_.faults).Count }).Count; missing_list = @($hubs.missing) }
        broken_links = $broken.Count
        unattributed = [ordered]@{
            count = $unattributed.Count; by_category = Get-Tally @($unattributed | ForEach-Object category)
            orphans = @($unattributed | Where-Object { $_.category -ne 'root' -and [int]$_.inbound -eq 0 } | ForEach-Object address)
            style = $(if ($CitationKey) { $null } else {
                $set = @{}; foreach ($c in $unattributed) { $set[[string]$c.address] = $c.category }
                $uf = @($faults | Where-Object { $set.ContainsKey($_.address) })
                $byCat = [ordered]@{}
                foreach ($g in @($uf | Group-Object { $set[$_.address] })) { $byCat[$g.Name] = Get-Tally @($g.Group | ForEach-Object rule) }
                [ordered]@{ faults = $uf.Count; cards_with_faults = @($uf | ForEach-Object address | Sort-Object -Unique).Count; by_rule = Get-Tally @($uf | ForEach-Object rule); by_category = $byCat }
            })
            list = @($unattributed | ForEach-Object address)
        }
    }
    books = @()
}
foreach ($key in $books) {
    $mine = @($scope | Where-Object { $bookOf[[string]$_.address] -eq $key })
    $mineSet = @{}
    foreach ($c in $mine) { $mineSet[[string]$c.address] = $c }
    $regular = @($mine | Where-Object { $_.category -ne 'root' })
    $writing = @($mine | Where-Object { $_.category -in @('idea', 'event') })

    # Topic trees with Cards of the book.
    $treeRows = @($mine | Where-Object { [string]$_.tree -in $topicTrees } | Group-Object { [string]$_.tree } | ForEach-Object {
        $t = $treeInfo[$_.Name]
        [ordered]@{ tree = $_.Name; title = [string]$t.title; cards = $_.Count; size = [int]$t.count }
    })
    $majority = @($treeRows | Where-Object { $_.cards * 2 -gt $_.size })

    # Card language.
    $myFaults = @($faults | Where-Object { $mineSet.ContainsKey($_.address) -and $_.rule -like 'R*' })
    $myLayer = @($faults | Where-Object { $mineSet.ContainsKey($_.address) -and $_.rule -notlike 'R*' })
    $advisory = @('I1', 'I6', 'E5', 'L3')
    $ruleByCategory = [ordered]@{}
    foreach ($g in @($myFaults | Group-Object { $mineSet[$_.address].category })) { $ruleByCategory[$g.Name] = Get-Tally @($g.Group | ForEach-Object rule) }

    # Orphans, source lines, kinded Links.
    $orphans = @($regular | Where-Object { [int]$_.inbound -eq 0 })
    # A source line links the Literature Card or an anchor of the book. A Link to another Card of
    # the book is not a source: every Card attributed to the book has one.
    $noSource = @($writing | Where-Object { -not @(@($_.links) | Where-Object { (Test-SourceTarget ([string]$_)) -and (Get-LinkBook ([string]$_)) -eq $key }).Count })
    $refers = 0
    $unkinded = @()
    foreach ($c in $writing) {
        $kinded = @(@($c.kinds.PSObject.Properties) | ForEach-Object Name)
        $others = @(@($c.links) | Where-Object { -not (Test-SourceTarget ([string]$_)) })
        $refers += @($others | Where-Object { $_ -notin $kinded }).Count
        if (-not @($kinded | Where-Object { $_ -ne [string]$c.parent }).Count) { $unkinded += $c }
    }

    # Models.
    $modelRows = @()
    for ($i = 0; $i -lt $heads.Count; $i++) {
        if ($bookOf[[string]$heads[$i].address] -ne $key) { continue }
        $m = $structures[$i]
        if (-not $m) { $modelRows += [ordered]@{ address = [string]$heads[$i].address; error = 'get_model failed' }; continue }
        $b = $m.build
        $n = [ordered]@{ closure = @($b.closure).Count; forward = @($b.forward).Count; unanchored = @($b.unanchored).Count; beyond_level = @($b.beyond_level).Count }
        $open = $n.closure + $n.forward + $n.unanchored + $(if ($m.level -eq 'concept') { $n.beyond_level } else { 0 })
        # Derivation parts, and those with both explanation sections (docs/derivation-explanations-proposal.md).
        $derived = @($m.parts | Where-Object { ([string]$_.facet).StartsWith('推导') })
        $explained = @($derived | Where-Object { [string]$_.body -match '(?m)^\s*\*\*思路\*\*\s*$' -and [string]$_.body -match '(?m)^\s*\*\*解读\*\*\s*$' })
        $modelRows += [ordered]@{ address = [string]$heads[$i].address; title = [string]$heads[$i].title; level = $m.level; parts = @($m.parts).Count; issues = @($m.issues).Count; faults = $n; self_sufficient = ($open -eq 0); derivations = $derived.Count; explained = $explained.Count }
    }

    # Figures: named but not shown.
    $named = @()
    $shownLabels = @{}
    foreach ($c in $regular) {
        if ($c.category -in @('hub', 'anchor')) { continue }
        $b = $bodies[[string]$c.address]
        if (-not $b) { continue }
        foreach ($s in [regex]::Matches($b.body, '!\[图\s?([^\]]+)\]\(artifact:')) { $shownLabels[$s.Groups[1].Value.Trim()] = $true }
    }
    foreach ($c in $regular) {
        if ($c.category -in @('hub', 'anchor')) { continue }
        $b = $bodies[[string]$c.address]
        if (-not $b) { continue }
        $plain = [regex]::Replace("$($b.title)`n$($b.body)", '!\[[^\]]*\]\([^)]*\)', '')
        $labels = @([regex]::Matches($plain, $figureName) | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
        foreach ($label in $labels) {
            if ($b.body -match ('!\[图\s?' + [regex]::Escape($label) + '\]\(artifact:')) { continue }
            $named += [ordered]@{ address = [string]$c.address; figure = $label; known = [bool]$shownLabels[$label] }
        }
    }

    $title = [string]$treeInfo[$key].title
    $report.books += [ordered]@{
        key = $key; title = $title
        cards = $mine.Count
        cards_by_category = Get-Tally @($mine | ForEach-Object category)
        cards_in_topic_trees = @($mine | Where-Object { [string]$_.tree -in $topicTrees }).Count
        unread = @($unread | Where-Object { $mineSet.ContainsKey($_) })
        topics = [ordered]@{ trees = $treeRows.Count; majority = $majority.Count; sizes = Get-Quantiles @($treeRows | ForEach-Object size); list = @($treeRows | Sort-Object { -$_.cards }) }
        style = [ordered]@{
            cards_checked = @($mine | Where-Object { $bodies.ContainsKey([string]$_.address) }).Count
            faults = $myFaults.Count; cards_with_faults = @($myFaults | ForEach-Object address | Sort-Object -Unique).Count
            by_rule = Get-Tally @($myFaults | ForEach-Object rule); by_category = $ruleByCategory
        }
        layer = [ordered]@{
            faults = @($myLayer | Where-Object { $_.rule -notin $advisory }).Count
            advisory = @($myLayer | Where-Object { $_.rule -in $advisory }).Count
            cards_with_faults = @($myLayer | Where-Object { $_.rule -notin $advisory } | ForEach-Object address | Sort-Object -Unique).Count
            by_rule = Get-Tally @($myLayer | ForEach-Object rule)
            term_table = $termCard[$key]
        }
        orphans = [ordered]@{ count = $orphans.Count; by_category = Get-Tally @($orphans | ForEach-Object category); list = @($orphans | ForEach-Object address) }
        no_source = [ordered]@{ count = $noSource.Count; of = $writing.Count; by_category = Get-Tally @($noSource | ForEach-Object category); list = @($noSource | ForEach-Object address) }
        unkinded = [ordered]@{ count = $unkinded.Count; of = $writing.Count; refers_links = $refers; list = @($unkinded | ForEach-Object address) }
        models = [ordered]@{
            count = $modelRows.Count
            levels = Get-Tally @($modelRows | ForEach-Object { if ($_.level) { $_.level } else { 'none' } })
            not_self_sufficient = @($modelRows | Where-Object { -not $_.self_sufficient }).Count
            with_issues = @($modelRows | Where-Object { $_.issues }).Count
            derivations = [int](@($modelRows | ForEach-Object { [int]$_.derivations }) + @(0) | Measure-Object -Sum).Sum
            explained = [int](@($modelRows | ForEach-Object { [int]$_.explained }) + @(0) | Measure-Object -Sum).Sum
            list = $modelRows
        }
        figures = [ordered]@{
            shown_labels = $shownLabels.Count
            missing_known = @($named | Where-Object known).Count; missing_unknown = @($named | Where-Object { -not $_.known }).Count
            list = $named
        }
        broken_links = @($brokenFrom | Where-Object { $mineSet.ContainsKey($_) }).Count
    }
}
$report.seconds = [int]$sw.Elapsed.TotalSeconds

# --- files ---------------------------------------------------------------------------------------
[IO.File]::WriteAllText((Join-Path $Out 'audit.json'), ($report | ConvertTo-Json -Depth 8), $utf8)
[IO.File]::WriteAllLines((Join-Path $Out 'audit-faults.txt'), [string[]]@($faults | Sort-Object address | ForEach-Object line), $utf8)

$md = [Text.StringBuilder]::new()
$w = $report.workspace
$cut = { param($list) $l = @($list); if ($l.Count -gt $Sample) { (($l[0..($Sample - 1)] | ForEach-Object { "``$_``" }) -join ', ') + ", … ($($l.Count))" } elseif ($l.Count) { ($l | ForEach-Object { "``$_``" }) -join ', ' } else { 'none' } }
$tally = { param($t)
    $pairs = if ($t -is [Collections.IDictionary]) { @($t.Keys | ForEach-Object { "$_ $($t[$_])" }) } elseif ($t) { @($t.PSObject.Properties | ForEach-Object { "$($_.Name) $($_.Value)" }) } else { @() }
    if ($pairs.Count) { $pairs -join ', ' } else { 'none' } }
[void]$md.AppendLine("# Digest audit: $($report.scope)").AppendLine()
[void]$md.AppendLine("Generated $($report.generated) by ``scripts/book-digest/digest-audit.ps1`` (read-only), in $($report.seconds) s. Card size limits: $($MaxSentences) sentences, $($MaxChars) characters, $($MaxSteps) steps.").AppendLine()
[void]$md.AppendLine('## Agent Workspace').AppendLine()
[void]$md.AppendLine('| Measure | Value |').AppendLine('| --- | --- |')
[void]$md.AppendLine("| Trees | $($w.trees) ($($w.topic_trees) Topic trees, $($w.books) books) |")
[void]$md.AppendLine("| Cards | $($w.cards): $(& $tally $w.cards_by_category) |")
$q = $w.topic_tree_sizes
[void]$md.AppendLine("| Topic tree size | median $($q.median), 25th percentile $($q.p25), 75th percentile $($q.p75), min $($q.min), max $($q.max) |")
[void]$md.AppendLine("| Hub Cards | $(& $tally $w.hubs.counts). Missing hubs $($w.hubs.missing). Hubs with faults $($w.hubs.with_faults). |")
[void]$md.AppendLine("| Broken Links | $($w.broken_links) |")
[void]$md.AppendLine("| Topic-tree Cards that link no book | $($w.unattributed.count): $(& $tally $w.unattributed.by_category) |")
if ($w.unattributed.style) {
    $us = $w.unattributed.style
    [void]$md.AppendLine("| Their Card language faults | $($us.faults) in $($us.cards_with_faults) Cards. $(@($us.by_category.Keys | ForEach-Object { "$($_): $(& $tally $us.by_category[$_])" }) -join '. ') |")
    [void]$md.AppendLine("| Their orphans | $($w.unattributed.orphans.Count): $(& $cut $w.unattributed.orphans) |")
}
[void]$md.AppendLine()
[void]$md.AppendLine('## Books').AppendLine()
[void]$md.AppendLine('| Book | Cards | Topic trees (most Cards) | Card language faults (Cards) | Card layer faults (Cards), advisory | Orphans | Idea and Event Cards with no source | with no kinded Link | Models not self-sufficient | Figures named, not shown (known) |')
[void]$md.AppendLine('| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |')
foreach ($b in $report.books) {
    [void]$md.AppendLine("| ``$($b.key)`` | $($b.cards) | $($b.topics.trees) ($($b.topics.majority)) | $($b.style.faults) ($($b.style.cards_with_faults)) | $($b.layer.faults) ($($b.layer.cards_with_faults)), $($b.layer.advisory) | $($b.orphans.count) | $($b.no_source.count) of $($b.no_source.of) | $($b.unkinded.count) of $($b.unkinded.of) | $($b.models.not_self_sufficient) of $($b.models.count) | $($b.figures.missing_known + $b.figures.missing_unknown) ($($b.figures.missing_known)) |")
}
[void]$md.AppendLine()
foreach ($b in $report.books) {
    [void]$md.AppendLine("### ``$($b.key)``: $($b.title)").AppendLine()
    [void]$md.AppendLine("- **Cards**: $($b.cards) ($(& $tally $b.cards_by_category)). $($b.cards_in_topic_trees) are in Topic trees.$(if ($b.unread.Count) { " $($b.unread.Count) Cards could not be read." })")
    [void]$md.AppendLine("- **Topic trees**: $($b.topics.trees). The book holds most Cards of $($b.topics.majority). Size median $($b.topics.sizes.median), max $($b.topics.sizes.max).")
    [void]$md.AppendLine("- **Card language**: $($b.style.faults) faults in $($b.style.cards_with_faults) of $($b.style.cards_checked) Cards. By rule: $(& $tally $b.style.by_rule).")
    $termText = if ($b.layer.term_table) { '`' + $b.layer.term_table + '`' } else { 'none' }
    [void]$md.AppendLine("- **Card layer rules** (I, E, L): $($b.layer.faults) faults in $($b.layer.cards_with_faults) Cards, $($b.layer.advisory) advisory (I1, I6, E5, L3). By rule: $(& $tally $b.layer.by_rule). Term table: $termText.")
    foreach ($cat in @($b.style.by_category.Keys)) { [void]$md.AppendLine("  - $($cat): $(& $tally $b.style.by_category[$cat])") }
    [void]$md.AppendLine("- **Orphans** (no inbound Link): $($b.orphans.count) ($(& $tally $b.orphans.by_category)): $(& $cut $b.orphans.list)")
    [void]$md.AppendLine("- **No source line** (Idea and Event Cards with no Link to the Literature Card or an anchor): $($b.no_source.count) of $($b.no_source.of): $(& $cut $b.no_source.list)")
    [void]$md.AppendLine("- **No kinded Link** to a Card other than the parent: $($b.unkinded.count) of $($b.unkinded.of). Links read as ""refers"": $($b.unkinded.refers_links). $(& $cut $b.unkinded.list)")
    [void]$md.AppendLine("- **Models**: $($b.models.count) ($(& $tally $b.models.levels)). Not self-sufficient: $($b.models.not_self_sufficient). With get_model issues: $($b.models.with_issues). Derivation parts with 思路 and 解读: $($b.models.explained) of $($b.models.derivations). $(& $cut @($b.models.list | Where-Object { -not $_.self_sufficient } | ForEach-Object address))")
    [void]$md.AppendLine("- **Figures**: $($b.figures.shown_labels) figures shown. Named but not shown: $($b.figures.missing_known) known, $($b.figures.missing_unknown) unknown. $(& $cut @($b.figures.list | Where-Object known | ForEach-Object { "$($_.address) 图 $($_.figure)" }))")
    [void]$md.AppendLine("- **Broken Links** from the book's Cards: $($b.broken_links)").AppendLine()
}
[IO.File]::WriteAllText((Join-Path $Out 'audit.md'), $md.ToString(), $utf8)

# --- summary line --------------------------------------------------------------------------------
[ordered]@{
    scope = $report.scope; out = $Out; seconds = $report.seconds; cards = $cards.Count
    hubs_missing = $w.hubs.missing; broken_links = $w.broken_links; unattributed = $w.unattributed.count
    books = @($report.books | ForEach-Object {
        [ordered]@{
            key = $_.key; cards = $_.cards; topic_trees = $_.topics.trees; style_faults = $_.style.faults
            layer_faults = $_.layer.faults; layer_advisory = $_.layer.advisory
            orphans = $_.orphans.count; no_source = $_.no_source.count; unkinded = $_.unkinded.count
            models = $_.models.count; not_self_sufficient = $_.models.not_self_sufficient
            derivations = $_.models.derivations; explained = $_.models.explained
            figures_missing = $_.figures.missing_known + $_.figures.missing_unknown
        }
    })
} | ConvertTo-Json -Depth 4 -Compress
