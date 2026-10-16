#Requires -Version 7.5
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\book-of.ps1')
function Assert($condition, [string]$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
$trees = @(@{ tree = 'Alpha'; kind = 'literature'; workspace = 'agent' })
$cards = @(@{ address = 'Alpha/1'; tree = 'Alpha'; kind = 'regular'; type = 'finding'; workspace = 'agent'; links = @() })
$index = Get-BookIndex -Trees $trees -Cards $cards
Assert ($index.Category['Alpha/1'] -ceq 'finding') 'a Finding Card keeps its category before anchor detection'

$trees += @{ tree = 'Beta'; kind = 'literature'; workspace = 'agent' }
$cards += @(
    @{ address = '4/1'; tree = '4'; kind = 'regular'; type = 'idea'; workspace = 'agent'; links = @('Beta', 'Alpha/1|a', 'Alpha/1|b') },
    @{ address = '4/2'; tree = '4'; kind = 'regular'; type = 'idea'; workspace = 'agent'; links = @('Beta/1|a', 'Alpha/1|a', 'Beta/1|a') },
    @{ address = '4/3'; tree = '4'; kind = 'regular'; type = 'idea'; workspace = 'agent'; links = @() }
)
$index = Get-BookIndex -Trees $trees -Cards $cards
Assert ($index.BookOf['Alpha/1'] -ceq 'Alpha' -and $index.BookOf['4/1'] -ceq 'Beta' -and $index.BookOf['4/2'] -ceq 'Alpha' -and -not $index.BookOf.ContainsKey('4/3')) 'book attribution weights source targets, deduplicates votes, and breaks ties ordinally'

$trees += @{ tree = 'Human'; kind = 'literature'; workspace = 'human' }
$cards += @(
    @{ address = 'Alpha'; tree = 'Alpha'; kind = 'literature'; type = 'idea'; workspace = 'agent'; links = @() },
    @{ address = 'Alpha/2'; tree = 'Alpha'; kind = 'regular'; type = 'idea'; workspace = 'agent'; links = @() },
    @{ address = 'Alpha/3'; tree = 'Alpha'; kind = 'regular'; type = 'idea'; workspace = 'agent'; links = @() },
    @{ address = 'Alpha/1|m'; tree = 'Alpha'; kind = 'regular'; type = 'model'; facet = '设定'; workspace = 'agent'; links = @() },
    @{ address = 'Alpha/1|t'; tree = 'Alpha'; kind = 'regular'; type = 'idea'; title = '术语表'; workspace = 'agent'; links = @() },
    @{ address = 'Alpha/1|s'; tree = 'Alpha'; kind = 'regular'; type = 'idea'; parent = 'Alpha/1'; edge = 'side'; workspace = 'agent'; links = @('Alpha/1', 'Alpha/2', 'Alpha/3') },
    @{ address = '4/4'; tree = '4'; kind = 'regular'; type = 'idea'; workspace = 'human'; links = @('Alpha') },
    @{ address = '4/5'; tree = '4'; kind = 'regular'; type = 'idea'; workspace = 'agent'; links = @('alpha') },
    @{ address = 'Case/1'; tree = '4'; kind = 'regular'; type = 'model'; workspace = 'agent'; links = @('Beta') },
    @{ address = 'case/1'; tree = '4'; kind = 'regular'; type = 'hub'; workspace = 'agent'; links = @('Alpha') }
)
$index = Get-BookIndex -Trees $trees -Cards $cards
Assert ($index.Category['Alpha'] -ceq 'root' -and $index.Category['Alpha/2'] -ceq 'anchor' -and $index.Category['Alpha/1|m'] -ceq 'part' -and $index.Category['Alpha/1|t'] -ceq 'terms' -and $index.Category['Alpha/1|s'] -ceq 'structure') 'public categories retain roots, anchors, parts, terms, and older structure Cards'
Assert (-not $index.BookOf.ContainsKey('4/4') -and -not $index.BookOf.ContainsKey('4/5') -and 'Human' -cnotin $index.Books -and $index.BookOf['Case/1'] -ceq 'Beta' -and $index.BookOf['case/1'] -ceq 'Alpha') 'human Cards stay unattributed and address and Citation-key comparison is case-sensitive'

$reordered = Get-BookIndex -Trees @($trees | Sort-Object tree -Descending -CaseSensitive) -Cards @($cards | Sort-Object address -Descending -CaseSensitive)
foreach($at in $index.Category.Keys){Assert ($index.Category[$at] -ceq $reordered.Category[$at]) "classification is independent of read order: $at"}
foreach($at in $index.BookOf.Keys){Assert ($index.BookOf[$at] -ceq $reordered.BookOf[$at]) "attribution is independent of read order: $at"}
