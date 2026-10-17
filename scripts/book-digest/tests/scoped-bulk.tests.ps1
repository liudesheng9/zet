#Requires -Version 7.5
# Public group-plan regression on an isolated archive. No provider/workflow runs.
param([string]$Zt = '', [string]$Checker = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "scoped-bulk-test-$PID"))
$testTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($testTempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the intended test temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) {
    $bin = Join-Path $scratch 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$testZtExecutable = (Get-Command zt -CommandType Application | Select-Object -First 1).Source
function Assert($condition, $message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
$savedCheckZt = $env:ZT_CHECK_ZT
$savedTrace = $env:ZT_TRACE_TOOLS
$savedFailTree = $env:ZT_FAIL_TREE
$savedFailCard = $env:ZT_FAIL_CARD
$savedFailFlag = $env:ZT_FAIL_FLAG
$savedMissing = $env:ZT_BULK_MISSING
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $null = Invoke-Zt @(
        @{tool='create_literature';arguments=@{bibtex='@book{OtherBook,title={Other book}}'}},
        @{tool='create_successor';arguments=@{at='OtherBook';title='External claim';body='这是外部命题。'}},
        @{tool='create_successor';arguments=@{at='OtherBook/1';kind='side';title='外部机制会影响产出';body='这是另一书的机制。'}},
        @{tool='create_literature';arguments=@{bibtex='@book{ScopeBook,title={Scoped book}}'}},
        @{tool='create_successor';arguments=@{at='ScopeBook';title='第一组';body='§1 生产'}},
        @{tool='create_successor';arguments=@{at='ScopeBook/1';kind='side';title='资金提高生产能力';body="资金被企业投入生产。`n`n见 [[OtherBook/1|a]]。`n`n来源：[[ScopeBook]] §1"}},
        @{tool='create_successor';arguments=@{at='ScopeBook/1';kind='side';title='术语表';body="术语表`n`n| 术语 | 不用 |`n| --- | --- |`n| 资本 | 资金 |"}}
    )
    $staging = Join-Path $scratch 'staging'
    New-Item -ItemType Directory -Force $staging | Out-Null
    [IO.File]::WriteAllText((Join-Path $staging 'owned-g01.txt'), "ScopeBook/1|a`n", $utf8)
    $traceFile = Join-Path $scratch 'tools.jsonl'
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_TRACE_TOOLS = $traceFile
    $env:ZT_FAIL_TREE = 'OtherBook'
    $env:ZT_FAIL_CARD = 'ScopeBook/1|a'
    $env:ZT_FAIL_FLAG = Join-Path $scratch 'no-single-card.flag'
    [IO.File]::WriteAllText($env:ZT_FAIL_FLAG, '', $utf8)
    $work = Join-Path $scratch 'work'
    $lines = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\card-compiler.ps1') -Step plan -Work $work -Book ScopeBook -Groups 1 -Staging $staging -NoOrphans -Rules 'R4,R6,L1,L2' 2>&1)
    Assert ($LASTEXITCODE -eq 0) "scoped group plan runs: $($lines -join ' | ')"
    $state = Get-Content -LiteralPath (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ('ScopeBook/1|a' -in $state.picked -and 'OtherBook/1' -notin $state.picked) 'required owned Cards remain in scope when unrelated tree reads and single-Card reads are unavailable'
    $faults = Get-Content -LiteralPath (Join-Path $work 'before\audit-faults.txt') -Raw -Encoding utf8
    Assert ($faults.Contains('ScopeBook/1|a R4') -and $faults.Contains('ScopeBook/1|a R6')) 'scoping retains language faults and reads the required external term table'
    Assert ($faults.Contains('ScopeBook/1|a L1') -and $faults.Contains('ScopeBook/1|a L2')) 'scoping retains the kind and reason faults of a cross-book regular target'
    $trace = @(Get-Content -LiteralPath $traceFile -Encoding utf8 | ForEach-Object { $_ | ConvertFrom-Json })
    Assert (@($trace | Where-Object tool -in @('get_card','get_tree','list_trees','list_hubs')).Count -eq 0 -and @($trace.process_id | Sort-Object -Unique).Count -eq 1) 'group checks use owned bulk inputs through one process without a whole-archive audit'
    $coverage = Get-Content -LiteralPath (Join-Path $work 'before\audit.json') -Raw -Encoding utf8 | ConvertFrom-Json
    Assert ($coverage.complete -and $coverage.scope.kind -eq 'groups' -and $coverage.global_checks -eq 'not_run') 'the report labels its checked scope and does not claim global qualification'
    if (-not $Checker) { $Checker = (Get-Command zt-check -CommandType Application | Select-Object -First 1).Source }
    $checkerTrace = Join-Path $scratch 'checker-tools.jsonl'
    $env:ZT_TRACE_TOOLS = $checkerTrace
    $addressCheck = @(& $Checker --addresses 'ScopeBook/1|a' --setting 'ScopeBook/1|b' 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($addressCheck -join ' ').Contains('ScopeBook/1|a R4') -and ($addressCheck -join ' ').Contains('ScopeBook/1|a R6')) "the public address checker reads its required Cards and term table in bulk: $($addressCheck -join ' | ')"
    $checkTrace = @(Get-Content -LiteralPath $checkerTrace -Encoding utf8 | ForEach-Object { $_ | ConvertFrom-Json })
    Assert (@($checkTrace | Where-Object tool -in @('get_card','get_tree','list_trees')).Count -eq 0) 'the address checker avoids per-Card model projections and tree scans'
    $cardTraceFile = Join-Path $scratch 'card-checker-tools.jsonl'
    $env:ZT_TRACE_TOOLS = $cardTraceFile
    $cardCheck = @(& $Checker --cards 'ScopeBook/1|a' --book ScopeBook --rules 'R4,L1,L2,I3' 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($cardCheck -join ' ').Contains('ScopeBook/1|a R4')) "the public Card checker keeps its faults through bulk inputs: $($cardCheck -join ' | ')"
    $cardTrace = @(Get-Content -LiteralPath $cardTraceFile -Encoding utf8 | ForEach-Object { $_ | ConvertFrom-Json })
    Assert (@($cardTrace | Where-Object tool -in @('get_card','get_tree','list_trees')).Count -eq 0) 'the Card checker uses scoped Link facts without scanning the book tree'
    $null = Invoke-Zt @(
        @{tool='create_successor';arguments=@{at='ScopeBook/1';kind='side';type='model';level='formal';title='资本提高产出';body='问题：资本怎样提高产出？'}},
        @{tool='create_successor';arguments=@{at='ScopeBook/1|c';kind='side';type='model';facet='设定';title='设定';body="术语表`n`n| 术语 | 不用 |`n| --- | --- |`n| 资本 | 资金 |"}},
        @{tool='create_successor';arguments=@{at='ScopeBook/1|c|a';type='model';facet='推导';title='产出会提高';body="**起点**`n依赖 [[ScopeBook/1|c|a]]。`n`n**步骤**`n1. 资金被企业投入生产。`n`n**结果**`n产出会提高。"}}
    )
    $modelTraceFile = Join-Path $scratch 'model-checker-tools.jsonl'
    $env:ZT_TRACE_TOOLS = $modelTraceFile
    $modelCards = 'ScopeBook/1|c|a|1,ScopeBook/1|c|a,ScopeBook/1|c'
    $modelCheck = @(& $Checker --cards $modelCards --rules 'R4,R6,R15' 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($modelCheck -join ' ').Contains(' R4 ') -and ($modelCheck -join ' ').Contains(' R6 ') -and ($modelCheck -join ' ').Contains(' R15 ')) "a filed model retains its term table and formal explanation faults: $($modelCheck -join ' | ')"
    $modelTrace = @(Get-Content -LiteralPath $modelTraceFile -Encoding utf8 | ForEach-Object { $_ | ConvertFrom-Json })
    Assert (@($modelTrace | Where-Object tool -eq 'get_model').Count -eq 1 -and @($modelTrace | Where-Object tool -in @('get_card','get_tree','list_trees')).Count -eq 0) 'one required model context serves its filed parts and head through the same checker process'
    $env:ZT_FAIL_CARD = 'ScopeBook/1|c'
    $modelModeTraceFile = Join-Path $scratch 'model-mode-tools.jsonl'
    $env:ZT_TRACE_TOOLS = $modelModeTraceFile
    $modelMode = @(& $Checker --model 'ScopeBook/1|c' --require-explain 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($modelMode -join ' ').Contains(' R4 ') -and ($modelMode -join ' ').Contains(' R6 ') -and ($modelMode -join ' ').Contains(' R15 ')) "the model checker uses the head and setting already in its complete model context: $($modelMode -join ' | ')"
    $modelModeTrace = @(Get-Content -LiteralPath $modelModeTraceFile -Encoding utf8 | ForEach-Object { $_ | ConvertFrom-Json })
    Assert (@($modelModeTrace | Where-Object tool -in @('get_card','get_tree','list_trees')).Count -eq 0) 'model mode does not reread the head through a full Card projection'
    $env:ZT_FAIL_CARD = 'ScopeBook/1|a'
    # The retained full-book path is the fault oracle during this migration.
    $env:ZT_CHECK_ZT = $savedCheckZt; $env:ZT_TRACE_TOOLS = $savedTrace
    $env:ZT_FAIL_TREE = $savedFailTree; $env:ZT_FAIL_CARD = $savedFailCard; $env:ZT_FAIL_FLAG = $savedFailFlag
    $fullWork = Join-Path $scratch 'full-book'
    $full = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\card-compiler.ps1') -Step plan -Work $fullWork -Book ScopeBook -Staging $staging -NoOrphans -Rules 'R4,R6,L1,L2' 2>&1)
    Assert ($LASTEXITCODE -eq 0) 'the full-book oracle runs on the same fixture'
    # Global inbound rules are deliberately not part of this NoOrphans group.
    $fullFaults = @(Get-Content -LiteralPath (Join-Path $fullWork 'before\audit-faults.txt') -Encoding utf8 | Where-Object { $_ -match '^ScopeBook/1\|a (R4|R6|L1|L2) ' })
    $scopedFaults = @($faults -split '\r?\n' | Where-Object { $_ -match '^ScopeBook/1\|a (R4|R6|L1|L2) ' })
    Assert (($scopedFaults -join "`n") -ceq ($fullFaults -join "`n")) "the scoped and full-book checks return identical requested owned-Card fault lines: $($scopedFaults -join ' | ') / $($fullFaults -join ' | ')"
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_BULK_MISSING = 'ScopeBook/1|a'
    $missingWork = Join-Path $scratch 'missing-input'
    $missing = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\card-compiler.ps1') -Step plan -Work $missingWork -Book ScopeBook -Groups 1 -Staging $staging -NoOrphans 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($missing -join ' ').Contains('incomplete Card read') -and -not (Test-Path -LiteralPath (Join-Path $missingWork 'state.json'))) 'missing required owned input cannot publish a zero-fault plan'
    $env:ZT_BULK_MISSING = $savedMissing
    $noManifestWork = Join-Path $scratch 'missing-manifest-work'
    $noManifestStaging = Join-Path $scratch 'missing-manifest-staging'
    New-Item -ItemType Directory -Force $noManifestStaging | Out-Null
    $noManifest = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\card-compiler.ps1') -Step plan -Work $noManifestWork -Book ScopeBook -Groups 1 -Staging $noManifestStaging -NoOrphans 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($noManifest -join ' ').Contains('owned-g01.txt') -and -not (Test-Path -LiteralPath (Join-Path $noManifestWork 'state.json'))) 'a missing required ownership manifest is incomplete input rather than an empty group'
} finally {
    $env:ZT_CHECK_ZT = $savedCheckZt; $env:ZT_TRACE_TOOLS = $savedTrace
    $env:ZT_FAIL_TREE = $savedFailTree; $env:ZT_FAIL_CARD = $savedFailCard; $env:ZT_FAIL_FLAG = $savedFailFlag
    $env:ZT_BULK_MISSING = $savedMissing
    $stopped = $false
    try {
        if (Test-Path -LiteralPath $testZtExecutable) { & $testZtExecutable down 2>&1 | Out-Null; $stopped = $LASTEXITCODE -eq 0 }
    } catch { Write-Output "test cleanup could not run the original executable: $($_.Exception.Message)" }
    if ($stopped) { Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue }
    else { Write-Output "test scratch retained for inspection: $scratch" }
}
Write-Output 'all passed'
