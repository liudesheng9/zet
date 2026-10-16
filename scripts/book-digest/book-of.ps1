#Requires -Version 7.5
# Pure classification and inferred attribution shared by the audit and Bridge compiler.
function Get-LinkBook {
    param([string]$Target, [Collections.Generic.HashSet[string]]$Books)
    $head = ($Target -split '/', 2)[0]
    if ($Books.Contains($head)) { $head }
}
function Test-SourceTarget {
    param([string]$Target, [Collections.Generic.HashSet[string]]$Books)
    [bool]((Get-LinkBook -Target $Target -Books $Books) -and ($Target -notmatch '/' -or $Target -match '^[^/]+/\d+$'))
}
function Get-Category {
    param($Card, [Collections.Generic.HashSet[string]]$Books, $AnchorCount)
    if ($Card.kind -cin @('topic', 'literature')) { return 'root' }
    switch -CaseSensitive ([string]$Card.type) {
        'finding' { return 'finding' }
        'hub' { return 'hub' }
        'event' { return 'event' }
        'model' { if ($Card.facet) { return 'part' }; return 'model' }
    }
    $literature = $Books.Contains([string]$Card.tree)
    if ($literature -and [string]$Card.address -match '^[^/]+/\d+$') { return 'anchor' }
    if ($literature -and [string]$Card.title -match '^术语表') { return 'terms' }
    if ($literature -and [string]$Card.excerpt -match '^本卡是《.*》的结构卡') { return 'structure' }
    if ($literature -and $Card.parent -ceq "$($Card.tree)/1" -and $Card.edge -ceq 'side' -and $Card.type -ceq 'idea') {
        $anchors = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($target in $Card.links) { if ([string]$target -cmatch ('^' + [regex]::Escape([string]$Card.tree) + '/\d+$')) { [void]$anchors.Add([string]$target) } }
        if ($anchors.Count -ge 3 -and $anchors.Count * 2 -ge $AnchorCount[[string]$Card.tree]) { return 'structure' }
    }
    'idea'
}
function Get-BookIndex {
    param([object[]]$Trees, [object[]]$Cards)
    $category = [Collections.Generic.Dictionary[string, object]]::new([StringComparer]::Ordinal)
    $bookOf = [Collections.Generic.Dictionary[string, object]]::new([StringComparer]::Ordinal)
    $books = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($tree in $Trees) { if ($tree.kind -ceq 'literature' -and $tree.workspace -ceq 'agent') { [void]$books.Add([string]$tree.tree) } }
    $literature = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($tree in $Trees) { if ($tree.kind -ceq 'literature') { [void]$literature.Add([string]$tree.tree) } }
    $anchors = [Collections.Generic.Dictionary[string, int]]::new([StringComparer]::Ordinal)
    foreach ($card in $Cards) {
        if ($literature.Contains([string]$card.tree) -and [string]$card.address -match '^[^/]+/\d+$') {
            $key = [string]$card.tree
            $anchors[$key] = 1 + $(if ($anchors.ContainsKey($key)) { $anchors[$key] } else { 0 })
        }
    }
    foreach ($card in $Cards) {
        $category[[string]$card.address] = Get-Category -Card $card -Books $literature -AnchorCount $anchors
        if ($card.workspace -cne 'agent') { continue }
        if ($books.Contains([string]$card.tree)) { $bookOf[[string]$card.address] = [string]$card.tree; continue }
        $votes = [Collections.Generic.Dictionary[string, int]]::new([StringComparer]::Ordinal)
        $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($target in $card.links) {
            if (-not $seen.Add([string]$target)) { continue }
            $book = Get-LinkBook -Target $target -Books $books
            if ($book) { $votes[$book] = $(if ($votes.ContainsKey($book)) { $votes[$book] } else { 0 }) + $(if (Test-SourceTarget -Target $target -Books $books) { 10 } else { 1 }) }
        }
        # The highest vote wins. A tie goes to the first key in Sort-Object order (case-insensitive),
        # as the audit decided before this helper existed. Citation keys never differ only by case.
        if ($votes.Count) { $bookOf[[string]$card.address] = [string]@($votes.GetEnumerator() | Sort-Object { -$_.Value }, Key)[0].Key }
    }
    $orderedBooks = [string[]]@($books)
    [Array]::Sort($orderedBooks, [StringComparer]::Ordinal)
    [pscustomobject]@{ Category = $category; BookOf = $bookOf; Books = $orderedBooks; BookSet = $books }
}
