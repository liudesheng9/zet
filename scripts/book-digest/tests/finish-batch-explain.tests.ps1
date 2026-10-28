#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\finish-batch-explain.tests.ps1
# §3.7 (#195): finish-batch records the Explain completion of a Model whose explanations the batch wrote,
# right after the Model's own checked completion, so the Explain stage starts no job for that model. A Model
# whose explanations are still open leaves its Explain unit ready for the Explain stage and records no fault
# against the Model. One scratch service and one prepared book; the live archive is not touched.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "finish-batch-explain-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest; $env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition, $message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) { throw "digest failed: $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Save-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 30), $utf8) }
function State($work) { Get-Content -LiteralPath (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable }
function Units([string]$layer) { @((Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', $layer)).units) }

# One formal model of the fixture: a setting, one Derivation part and a proposition. The plan carries the
# architect's explain design of the chain (model-architect.md, §3.7).
function New-FormalModel([string]$model, [bool]$explained) {
    $slug = $model -replace '[\\/|]', '_'
    $work = Join-Path $out "models\m-$slug"
    $dir = Join-Path $work $slug
    $step = { param($s, [string[]]$more = @()) Run-Digest (@('model', '-Step', $s, '-Work', $work, '-Staging', $out) + $more) }
    $null = & $step 'brief' @('-Models', $model)
    $plan = [ordered]@{
        model = $model; level = 'formal'; card = '问题、机制、结论'
        symbols = @(, @('$L$', '清算价值')); terms = @(, @('清算', '卖资产', '变卖资产'))
        parts = @(
            [ordered]@{ id = 'S'; line = 'main'; facet = '设定'; emphasis = 'core'; title = '设定：企业家、项目与清算价值'; spec = '参与者与时点'; src = @('sections/s01.md:2-3') },
            [ordered]@{ id = 'D'; line = 'main'; derive = $true; facet = '推导：还款上限'; title = '推导：还款不超过清算价值'; from = '(5.5)'; to = '$$P\le L$$'; steps = 2; rests = @(, @('S', '用到 $L$')); src = @('sections/s01.md:4-4') },
            [ordered]@{ id = 'P'; line = 'main'; facet = '命题'; emphasis = 'core'; title = '命题：清算价值够大才能融资'; spec = '命题'; rests = @(, @('D', '用到 (5.5)')); src = @('sections/s01.md:5-5') }
        )
        explain = @([ordered]@{
            chain = @('D'); route = '先求还款上限，再看清算价值够不够大。'
            parts = @([ordered]@{ id = 'D'; shows = '还款不超过清算价值'; notes = ''; meaning = '还款上限由清算价值决定。' })
        })
    }
    Save-Json (Join-Path $dir 'plan.json') $plan
    $null = & $step 'apply'
    $ids = (State $work).models[$model].ids
    $head = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })
    $structure = ([regex]'(?s)\*\*结构\*\*.*$').Match($head.body).Value
    $derivation = "**起点**`n- 依赖 [[$($ids['S'])]] 的结果：清算价值为 `$L`$。`n`n**步骤**`n1. 还款不超过清算价值。`n`n**结果**`n`$`$P\le L`$`$"
    if ($explained) {
        $derivation = "**思路**`n还款不超过清算价值。全链只有一张分卡，直接写出上限。`n`n**起点**`n- 依赖 [[$($ids['S'])]] 的结果：清算价值为 `$L`$。`n`n**步骤**`n1. 还款不超过清算价值。`n`n**结果**`n`$`$P\le L`$`$`n`n**解读**`n结果说明还款上限由清算价值决定。检验：清算价值为 0 时企业借不到钱。"
    }
    Invoke-Zt @(
        @{ tool = 'edit_card'; arguments = @{ at = $model; body = "问题：企业能否融到 `$K-w`$？机制：清算价值决定还款上限。结论：清算价值够大才能融资。`n`n来源：第一章，(5.5)。`n`n$structure" } },
        @{ tool = 'edit_card'; arguments = @{ at = $ids['S']; body = '企业家有财富 $w$，项目需要 $K$，清算价值为 $L$。' } },
        @{ tool = 'edit_card'; arguments = @{ at = $ids['D']; body = $derivation } },
        @{ tool = 'edit_card'; arguments = @{ at = $ids['P']; body = "清算价值够大，项目才能融资。依赖 [[$($ids['D'])]].`n`n来源：第一章。" } }
    ) | Out-Null
    $null = & $step 'report'
    $null = & $step 'derive'
    $null = & $step 'review'
    return @{ model = $model; explained = $explained }
}

try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $out = Join-Path $scratch 'prepared'
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source, "# 第一章`n`n企业家有财富 `$w`$，项目需要 `$K`$。`n`n清算价值为 `$L`$。`n`n式 (5.5)：`$P\le L`$。`n`n于是得到 (5.10)。`n`n# 参考`n未选择。", $utf8)
    $options = Join-Path $scratch 'options.json'; Save-Json $options @{ terms = $false }
    $null = Run-Digest @('prepare', '-Source', $source, '-Out', $out, '-CitationKey', 'BatchExplain', '-BibTeX', '@book{BatchExplain,title={Batch explain}}', '-StartHeading', '第一章', '-EndHeading', '参考', '-MinUnitChars', '1', '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')
    # One digest group writes its anchor and two formal Models: one whose explanations the batch will
    # write, and one whose Derivation part the batch leaves unexplained.
    $claim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'digest-manager')).units[0]
    $started = Run-Digest @('group-start', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)")
    $job = Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $draft = @{ schema_version = 1; unit = $claim.id; token = $claim.token; input_hash = $started.input_hash; disposition = 'cards'; calls = @(
        @{ tool = 'edit_card'; arguments = @{ at = 'BatchExplain/1'; title = '融资约束影响企业投资'; body = '融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。'; base_fingerprint = $job.inputs.anchor.fingerprint } },
        @{ tool = 'create_successor'; arguments = @{ at = 'BatchExplain/1'; kind = 'side'; type = 'model'; level = 'formal'; title = '融资模型：清算价值决定还款上限'; body = "问题：企业能否融到 `$K-w`$？`n`n来源：第一章，(5.5)。" } },
        @{ tool = 'create_successor'; arguments = @{ at = 'BatchExplain/1'; kind = 'side'; type = 'model'; level = 'formal'; title = '还款模型：清算价值决定融资条件'; body = "问题：企业能否融到 `$K-w`$？`n`n来源：第一章，(5.10)。" } }
    ) }
    Save-Json $started.draft $draft
    $group = Run-Digest @('group-finish', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)", '-Draft', $started.draft)
    $first = @($group.owned | Where-Object { $_ -match '\|a$' })[0]
    $second = @($group.owned | Where-Object { $_ -match '\|b$' })[0]
    Assert ($first -and $second -and $group.state -eq 'done') "the group completes and registers two Model candidates ($first, $second)"

    $null = New-FormalModel $first $true
    $null = New-FormalModel $second $false

    # Both Model units are finished by ONE finish-batch command. The explained model's Explain unit is
    # completed right after its Model completion; the unexplained one is left ready.
    $claimed = @((Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'models', '-Take', '8', '-Owner', 'model-block')).units)
    Assert ($claimed.Count -eq 2) 'both Model units are claimed'
    $list = ($claimed | ForEach-Object { "$($_.id):$($_.token)" }) -join ','
    $line = Run-Digest @('finish-batch', '-Out', $out, '-Units', $list)
    Assert ($line.kind -eq 'finish_batch' -and $line.done -eq 2 -and $line.failed -eq 0) "both Model completions are recorded [$($line | ConvertTo-Json -Compress -Depth 6)]"
    $explains = @($line.explain)
    Assert ($explains.Count -eq 2 -and $line.explain_done -eq 1 -and $line.explain_ready -eq 1) "the batch records one Explain completion and leaves one ready [$($line | ConvertTo-Json -Compress -Depth 6)]"
    $modelRows = Units 'models'
    $explainRows = Units 'explain'
    $explainedKey = ($modelRows | Where-Object { $_.inputs.header -eq $first }).id
    $unexplainedKey = ($modelRows | Where-Object { $_.inputs.header -eq $second }).id
    $explainedUnit = 'explain/' + ($explainedKey -replace '^models/', '' -replace '@1$', '') + '@1'
    $unexplainedUnit = 'explain/' + ($unexplainedKey -replace '^models/', '' -replace '@1$', '') + '@1'
    $explainedDone = @($explains | Where-Object { $_.unit -eq $explainedUnit })[0]
    $unexplainedReady = @($explains | Where-Object { $_.unit -eq $unexplainedUnit })[0]
    Assert ($explainedDone -and $explainedDone.status -eq 'done' -and $explainedDone.token -gt 0) "the explained model's Explain unit is completed by the batch [$($explainedDone | ConvertTo-Json -Compress)]"
    Assert ($unexplainedReady -and $unexplainedReady.status -eq 'ready') "the unexplained model's Explain unit stays ready [$($unexplainedReady | ConvertTo-Json -Compress)]"
    $explainedRow = @($explainRows | Where-Object { $_.id -eq $explainedUnit })[0]
    $unexplainedRow = @($explainRows | Where-Object { $_.id -eq $unexplainedUnit })[0]
    Assert ($explainedRow -and $explainedRow.state -eq 'done' -and $explainedRow.result.kind -eq 'explain_completion' -and $explainedRow.result.explanation.status -eq 'explained') 'the ledger holds a typed, checked Explain completion for the explained model'
    Assert ($unexplainedRow -and $unexplainedRow.state -eq 'ready' -and -not $unexplainedRow.result) 'the unexplained Explain unit is left for the Explain stage, with no fault of its own'
    Assert (@($modelRows | Where-Object { $_.state -ne 'done' }).Count -eq 0) 'both Model units are done'
    $gate = @(& $Digest @('ledger', 'gate', '-Out', $out, '-Layer', 'explain') 2>&1)
    $gateJson = $gate[-1] | ConvertFrom-Json -AsHashtable
    Assert ($LASTEXITCODE -eq 3 -and $gateJson.counts.open -eq 1 -and @($gateJson.unresolved) -contains $unexplainedUnit) 'the Explain layer stays open on exactly the unexplained model'

    # A repeated batch is idempotent: the done Explain unit stays done, the open one stays ready.
    $again = Run-Digest @('finish-batch', '-Out', $out, '-Units', $list)
    Assert ($again.done -eq 2 -and $again.explain_done -eq 1 -and $again.explain_ready -eq 1) 'a repeated batch keeps the Explain outcomes'
    Assert (@((Units 'explain') | Where-Object { $_.id -eq $explainedUnit }).state -eq 'done' -and @((Units 'explain') | Where-Object { $_.id -eq $unexplainedUnit }).state -eq 'ready') 'the repeated batch never reopens or faults an Explain unit'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
