#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\ledger-adopt-settled.tests.ps1
# `ledger adopt -Settled` (2026-11-09, the empirical models' live migration): a book whose Literature tree was
# reorganized after its digest (zt-literature-organize) has group anchors and owned Cards that moved. Without
# -Settled, adoption names the moved anchor as missing scope and writes nothing. With -Settled, a group it cannot
# verify is closed (a content failure that names why), never resumed under a guessed anchor, and a Model Card of
# such a group stands on the prepared book, so a model rebuild can run. One scratch archive; the live archive is
# never touched.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-adopt-settled-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest; $env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition, $message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments, [int]$ExpectedExit = 0) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE -ne $ExpectedExit) { throw "digest failed ($LASTEXITCODE expected $ExpectedExit): $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Refused([string[]]$Arguments, [string]$Text, [string]$Message) {
    $lines = @(& $Digest @Arguments 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($lines -join ' ').Contains($Text)) "$message [$($lines -join ' ')]"
}
function Save-Json($Path, $Value) { [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 30), $utf8) }
function Tree($key) { return (Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $key } }))[0] }
function Address-Of($tree, [string]$title) { return @($tree.cards | Where-Object { $_.title -eq $title })[0].address }
$archive = Join-Path $scratch 'archive'
$book = Join-Path $scratch 'book.md'
$key = 'SettledBook'
try {
    zt config set archive_root $archive | Out-Null; zt up | Out-Null
    [IO.File]::WriteAllText($book, "# 第一章`n`n融资约束改变企业的生产选择。`n`n# 第二章`n`n贷款收紧使企业减少设备投入。`n`n# 第三章`n`n投入减少限制下一期的生产能力。`n`n# 参考`n未选择。", $utf8)
    $options = Join-Path $scratch 'options.json'; Save-Json $options @{ terms = $false }
    $legacy = Join-Path $scratch 'legacy'
    $null = Run-Digest @('prepare', '-Source', $book, '-Out', $legacy, '-CitationKey', $key, '-BibTeX', "@book{$key,title={Settled book}}", '-StartHeading', '第一章', '-EndHeading', '参考', '-MinUnitChars', '1', '-MaxUnits', '1', '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')
    foreach ($name in @('manifest.json', 'ledger.sqlite3', 'events.json', 'shared-prefix.md', 'link-prefix.md', 'setup-calls.json', 'preparation.json', 'source-faults.md', 'source-faults.json')) {
        $path = Join-Path $legacy $name
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
    }
    $legacy = (Resolve-Path $legacy).Path

    # The digest: three groups, a model in group 2, and the owned logs the digest helper wrote.
    $null = Invoke-Zt @(
        @{ tool = 'edit_card'; arguments = @{ at = "$key/1"; title = '融资约束改变企业的生产选择'; body = '融资成本影响企业的投资选择。来源：[[SettledBook]]' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = '融资成本决定企业投入规模'; body = '融资成本决定企业的设备投入。来源：[[SettledBook/1]]' } },
        @{ tool = 'edit_card'; arguments = @{ at = "$key/2"; title = '贷款收紧减少设备投入'; body = '贷款收紧使企业减少设备投入。来源：[[SettledBook]]' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/2"; kind = 'side'; title = '设备投入随贷款条件变化'; body = '贷款条件放松时设备投入上升。来源：[[SettledBook/2]]' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/2"; kind = 'side'; title = '贷款与投入的模型'; body = "问题：贷款条件如何改变投入。`n`n**结构**`n`n- 设定（核心）：企业以贷款购买设备"; type = 'model' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@4'; kind = 'side'; title = '设定：企业以贷款购买设备'; body = '企业以贷款购买设备，投入随贷款利率下降。'; type = 'model'; facet = '设定' } },
        @{ tool = 'edit_card'; arguments = @{ at = "$key/3"; title = '投入下降压低下一期生产能力'; body = '投入减少限制下一期的生产能力。来源：[[SettledBook]]' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/3"; kind = 'side'; title = '生产能力约束下一期产出'; body = '生产能力决定下一期产出。来源：[[SettledBook/3]]' } }
    )
    foreach ($g in 1..3) {
        $cards = @((Tree $key).cards | Where-Object { $_.address -eq "$key/$g" -or $_.address.StartsWith("$key/$g|") } | ForEach-Object { $_.address })
        [IO.File]::WriteAllText((Join-Path $legacy ('owned-g{0:D2}.txt' -f $g)), (($cards -join "`n") + "`n"), $utf8)
    }

    # The reorganization: a Card of group 2 and the whole of group 3 move under group 1.
    $null = Invoke-Zt @(@{ tool = 'move_card'; arguments = @{ at = "$key/2|a"; before = "$key/1|a"; alone = $true; confirm = 'move' } })
    $null = Invoke-Zt @(@{ tool = 'move_card'; arguments = @{ at = "$key/3"; before = "$key/1|a"; confirm = 'move' } })
    $tree = Tree $key
    $model = Address-Of $tree '贷款与投入的模型'
    Assert (-not @($tree.cards | Where-Object { $_.address -eq "$key/3" }).Count -and $model.StartsWith("$key/2|")) "the reorganized tree: group 3's anchor moved, the model stays in group 2 ($model)"

    # Without -Settled: the moved anchor is missing scope, and nothing is written.
    $plain = Run-Digest @('ledger', 'adopt', '-Out', $legacy, '-Variant', 'econ') 3
    Assert ($plain.status -eq 'insufficient_evidence' -and @($plain.missing | Where-Object { $_.what -eq 'group 3 anchor' }).Count -eq 1) 'without -Settled the moved anchor is missing scope'
    Refused @('ledger', 'adopt', '-Out', $legacy, '-Variant', 'econ', '-Apply') 'insufficient_evidence' 'without -Settled the apply writes no ledger'
    Assert (-not (Test-Path (Join-Path $legacy 'ledger.sqlite3'))) 'no ledger was written'

    # With -Settled: the plan is ready and says why each unverified group is closed.
    $plan = Run-Digest @('ledger', 'adopt', '-Out', $legacy, '-Variant', 'econ', '-Settled')
    Assert ($plan.status -eq 'ready' -and $plan.settled -eq $true) 'with -Settled the plan of a reorganized tree is ready'
    $g3 = @($plan.classification.incomplete | Where-Object { $_.unit -eq 'digest/g003@1' })[0]
    Assert ($g3 -and $g3.reason.Contains('settled')) "the moved group is incomplete and says it is closed ($($g3.reason))"
    $applied = Run-Digest @('ledger', 'adopt', '-Out', $legacy, '-Variant', 'econ', '-Settled', '-Apply')
    $status = Run-Digest @('ledger', 'status', '-Out', $legacy)
    $unit = { param($id) @($status.units | Where-Object { $_.id -eq $id })[0] }
    foreach ($id in @('digest/g002@1', 'digest/g003@1')) {
        $u = & $unit $id
        Assert ($u.state -eq 'failed' -and $u.fault_class -eq 'content' -and $u.fault.code -eq 'settled_literature') "$id is closed with the settled fault ($($u.state) $($u.fault_class))"
    }
    $modelUnit = "models/adopted-$($model.Replace('/','-').Replace('|','-'))@1"
    Assert ((& $unit $modelUnit)) "the model of the closed group has its unit ($modelUnit)"
    $deps = @($status.dependencies | Where-Object { $_.unit -eq $modelUnit })
    Assert ($deps.Count -ge 1 -and @($deps | Where-Object { $_.needs -eq 'prepare/book@1' }).Count -eq 1 -and -not @($deps | Where-Object { $_.needs -eq 'digest/g002@1' }).Count) "the model stands on the prepared book, not on its closed group ($(($deps | ForEach-Object { $_.needs }) -join ','))"
    # A resume never runs a closed group again.
    $resumed = Run-Digest @('ledger', 'resume', '-Out', $legacy)
    $after = (Run-Digest @('ledger', 'status', '-Out', $legacy)).units.Where({ $_.id -eq 'digest/g003@1' })[0]
    Assert ($after.state -eq 'failed') "the closed group stays closed across the resume ($($after.state) $($resumed | ConvertTo-Json -Compress -Depth 4))"
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
