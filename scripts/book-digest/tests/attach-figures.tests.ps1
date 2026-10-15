#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\attach-figures.tests.ps1 [-Zt <zt.exe>]
# Drives attach-figures.ps1 on a scratch archive (ZT_CONFIG_DIR, its own service) with a figures.json
# standing in for the Scholar Archive, so no bridge is needed. Checks where a figure lands, that a
# longer number (图 2.C.10) and Cards outside the book or on the anchor line are left alone, that a
# Chinese numeral (图四) finds a Roman label (IV), and that a second run changes nothing. Also checks
# the caption labels figures.ps1 reads. The live archive is not touched. -Zt runs a zt build that
# is not installed yet (copied to the scratch folder and put first on PATH).
param([string]$Zt = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "attach-figures-test-$PID"
$archive = Join-Path $scratch 'archive'
New-Item -ItemType Directory -Force $archive | Out-Null
if ($Zt) {
    $bin = Join-Path $scratch 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$attach = Join-Path $PSScriptRoot '..\attach-figures.ps1' | Resolve-Path | ForEach-Object Path
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
. (Join-Path $PSScriptRoot '..\figures.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }

# --- caption labels and mention patterns (no archive needed) -------------------------------------------
Assert ((Get-FigureLabels 'Figure 2.C.1 (left)\') -join ',' -eq '2.C.1') 'a caption line gives its label'
Assert ((Get-FigureLabels 'FIGURE IV') -join ',' -eq 'IV,四') 'a Roman label also gives its Chinese numeral'
Assert ((Get-FigureLabels '图四\') -join ',' -eq '四,IV') 'a Chinese numeral also gives its Roman label'
Assert (-not (Get-FigureLabels 'The figure shows a curve.')) 'prose is not a caption'
Assert ((Get-FigureImageName '![](../Assets/G%20H/images/abc.jpg)\') -eq 'abc.jpg') 'an image line names its file'
$p = Get-FigureMentionPattern '2.C.1'
Assert (('如图 2.C.1 所示' -match $p) -and ('图2.C.1。' -match $p) -and -not ('图 2.C.10' -match $p) -and -not ('图 2.C.1.a' -match $p)) 'a mention never matches a longer number'
Assert (('见图 I 中' -match (Get-FigureMentionPattern 'I')) -and -not ('图 II' -match (Get-FigureMentionPattern 'I'))) 'Roman labels do not run into each other'

try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    # Two figures ZET holds: the bytes sit in the archive's artifact store under their SHA-256.
    $store = Join-Path $archive 'artifacts'
    New-Item -ItemType Directory -Force $store | Out-Null
    $figs = foreach ($n in 'a', 'b') {
        $bytes = [byte[]](0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A) + [Text.Encoding]::ASCII.GetBytes("fixture-$n")
        $sha = -join ([Security.Cryptography.SHA256]::HashData($bytes) | ForEach-Object { $_.ToString('x2') })
        [IO.File]::WriteAllBytes((Join-Path $store "$sha.png"), $bytes)
        $sha
    }
    $figuresFile = Join-Path $scratch 'figures.json'
    $figureList = @(
        [ordered]@{ name = 'a.jpg'; labels = @('2.C.1'); alt = '图 2.C.1'; line = "![图 2.C.1](artifact:$($figs[0]))"; sha256 = $figs[0]; path = 'B/images/a.jpg' },
        [ordered]@{ name = 'b.jpg'; labels = @('IV', '四'); alt = '图 IV'; line = "![图 IV](artifact:$($figs[1]))"; sha256 = $figs[1]; path = 'B/images/b.jpg' }
    )
    [IO.File]::WriteAllText($figuresFile, ([ordered]@{ file_id = 0; figures = $figureList } | ConvertTo-Json -Depth 5), $utf8)

    $table = "| 情形 | 图 |`n| --- | --- |`n| 凸 | 图 2.C.1 |"
    $null = Invoke-Zt @(
        @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{Test2020, title={Microeconomics}}'; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2020'; title = '第一组：消费集'; body = '§2.C - 图 2.C.1 画出消费集。' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2020/1'; kind = 'side'; title = '消费集可以不凸'; body = "图 2.C.1 画出几种消费集。`n`n第二段说明凸性。`n`n来源：[[Test2020]] §2.C" } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2020/1|a'; title = '整数商品使消费集不凸'; body = "见图 2.C.10 与图 2.C.11。`n`n来源：[[Test2020]] §2.C" } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2020/1|a|1'; title = '三种情形的比较'; body = "$table`n`n凸性排除第二种情形。" } },
        @{ tool = 'create_topic'; arguments = @{ title = '组织模式'; body = '追问企业边界。' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@5'; title = '替代弹性能反转外包的可行性'; body = "如图四所示，曲线呈倒 U 形。`n`n来源：[[Test2020]] §4" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@6'; title = '别的书也有图 2.C.1'; body = '这张卡不属于这本书，图 2.C.1 指别处。' } }
    )

    $run = { param([switch]$Apply)
        $lines = @(pwsh -NoProfile -File $attach -CitationKey Test2020 -FiguresFile $figuresFile -NoDocuments -Apply:$Apply 2>&1)
        $last = @($lines | Where-Object { "$_" -match '^{' })[-1]
        if (-not $last) { throw "attach-figures.ps1 failed:`n$($lines -join "`n")" }
        $last
    }
    $dry = (& $run) | ConvertFrom-Json
    Assert ($dry.cards -eq 3 -and -not $dry.applied) "the dry run plans three Cards: $($dry | ConvertTo-Json -Compress)"
    $before = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = 'Test2020/1|a' } })
    Assert ($before.body -notmatch 'artifact:') 'the dry run writes nothing'

    $applied = (& $run -Apply) | ConvertFrom-Json
    Assert ($applied.applied -and $applied.shown -eq 3) "the run shows three figures: $($applied | ConvertTo-Json -Compress)"
    $cards = @{}
    foreach ($c in @(Invoke-Zt @('Test2020/1', 'Test2020/1|a', 'Test2020/1|a|1', 'Test2020/1|a|2', '0/1', '0/2' | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } }))) { $cards[$c.address] = $c.body }
    $line1 = "![图 2.C.1](artifact:$($figs[0]))"
    Assert ($cards['Test2020/1|a'] -eq "图 2.C.1 画出几种消费集。`n`n$line1`n`n第二段说明凸性。`n`n来源：[[Test2020]] §2.C") "the figure follows the paragraph that names it: $($cards['Test2020/1|a'])"
    Assert ($cards['Test2020/1|a|1'] -notmatch 'artifact:') 'a longer number (图 2.C.10) is not a mention'
    Assert ($cards['Test2020/1|a|2'] -eq "$table`n`n$line1`n`n凸性排除第二种情形。") "a mention in a table puts the figure after the table: $($cards['Test2020/1|a|2'])"
    Assert ($cards['0/1'] -match [regex]::Escape("![图 IV](artifact:$($figs[1]))")) 'a Topic Card of the book gets the figure its Chinese numeral names'
    Assert ($cards['0/2'] -notmatch 'artifact:') 'a Card outside the book is left alone'
    Assert ($cards['Test2020/1'] -notmatch 'artifact:') 'the anchor line is left alone'

    $again = (& $run -Apply) | ConvertFrom-Json
    Assert ($again.cards -eq 0) 'a second run changes nothing'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
}
if ($failed) { Write-Output "$failed failed"; exit 1 }
Write-Output 'all passed'
