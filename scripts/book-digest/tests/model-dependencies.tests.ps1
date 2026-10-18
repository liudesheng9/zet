#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\model-dependencies.tests.ps1
# R233/R234: a plan reference `X#A` becomes an explicit producer dependency before any
# dependent fill/apply. An incomplete producer makes the dependent wait without spending an
# attempt; a completed producer without the named part is a precise content fault; a cycle is
# detected before scheduling; a changed part cannot be reused. One scratch service.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "model-dependencies-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
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
function Refused([string[]]$Arguments, [string]$text, [string]$message) {
    $lines = @(& $Digest @Arguments 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($lines -join ' ').Contains($text)) "$message [$($lines -join ' ')]"
}
function Save-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 30), $utf8) }
function State($work) { Get-Content -LiteralPath (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable }
function Plan($model, $take = $null) {
    $parts = @(
        [ordered]@{ id = 'S'; line = 'main'; facet = '设定'; emphasis = 'core'; title = '设定：企业与清算价值'; spec = '参与者'; src = @('sections/s01.md:2-3') }
    )
    if ($take) {
        $parts += [ordered]@{ id = 'T'; take = $take; use = '生产者模型的命题' }
        $parts += [ordered]@{ id = 'P'; line = 'main'; facet = '命题'; emphasis = 'core'; title = '命题：清算价值决定融资'; spec = '命题'; rests = @(, @('T', '用到它')); src = @('sections/s01.md:4') }
    } else {
        $parts += [ordered]@{ id = 'P'; line = 'main'; facet = '命题'; emphasis = 'core'; title = '命题：清算价值决定融资'; spec = '命题'; rests = @(, @('S', '用到设定')); src = @('sections/s01.md:4') }
    }
    return [ordered]@{
        model = $model; level = 'formal'; card = '问题、机制、结论'
        symbols = @(, @('$L$', '清算价值')); terms = @(, @('清算', '卖资产'))
        parts = $parts
    }
}
function Work($out, $model) { $slug = $model -replace '[\\/|]', '_'; return (Join-Path $out "models\m-$slug") }
function PlanFile($work, $model) { return (Join-Path (Join-Path $work ($model -replace '[\\/|]', '_')) 'plan.json') }
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $out = Join-Path $scratch 'prepared'
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source, "# 第一章`n`n企业家有财富 `$w`$，项目需要 `$K`$。`n`n清算价值为 `$L`$。`n`n式 (5.5)：`$P\le L`$。`n`n于是得到 (5.10)。`n`n# 参考`n未选择。", $utf8)
    $options = Join-Path $scratch 'options.json'; Save-Json $options @{ terms = $false }
    $null = Run-Digest @('prepare', '-Source', $source, '-Out', $out, '-CitationKey', 'DepsBook', '-BibTeX', '@book{DepsBook,title={Deps book}}', '-StartHeading', '第一章', '-EndHeading', '参考', '-MinUnitChars', '1', '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')
    # One group creates seven Model Cards, so the Models layer registers seven units.
    $claim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'digest-manager')).units[0]
    $started = Run-Digest @('group-start', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)")
    $job = Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $calls = @(@{ tool = 'edit_card'; arguments = @{ at = 'DepsBook/1'; title = '融资约束影响企业投资'; body = '融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。'; base_fingerprint = $job.inputs.anchor.fingerprint } })
    $titles = @('生产者模型', '依赖模型', '循环甲', '循环乙', '缺件模型', '未知引用', '变更模型')
    foreach ($title in $titles) {
        $calls += @{ tool = 'create_successor'; arguments = @{ at = 'DepsBook/1'; kind = 'side'; type = 'model'; level = 'formal'; title = $title; body = '问题：它怎样影响融资？来源：第一章。' } }
    }
    $draft = @{ schema_version = 1; unit = $claim.id; token = $claim.token; input_hash = $started.input_hash; disposition = 'cards'; calls = $calls }
    Save-Json $started.draft $draft
    $group = Run-Digest @('group-finish', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)", '-Draft', $started.draft)
    $models = @($group.owned | Where-Object { $_ -match '/1\|[a-g]$' } | Sort-Object)
    Assert ($models.Count -eq 7) "the group registers seven Model candidates ($($models -join ', '))"
    $M1, $M2, $M3, $M4, $M5, $M6, $M7 = $models
    $claims = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'models', '-Take', '10', '-Owner', 'model-manager')).units
    Assert ($claims.Count -eq 7) 'all seven Model units are ready independently'
    $tokenOf = @{}; foreach ($c in $claims) { $tokenOf[$c.inputs.header] = $c.token }
    $unitOf = @{}; foreach ($c in $claims) { $unitOf[$c.inputs.header] = $c.id }
    # Each model's work folder gets its brief and plan; only M1 and M2 are built now.
    $workOf = @{}; foreach ($m in $models) { $workOf[$m] = Work $out $m; $null = Run-Digest @('model', '-Step', 'brief', '-Work', $workOf[$m], '-Staging', $out, '-Models', $m) }
    Save-Json (PlanFile $workOf[$M1] $M1) (Plan $M1 $null)
    Save-Json (PlanFile $workOf[$M2] $M2) (Plan $M2 "$M1#P")
    Save-Json (PlanFile $workOf[$M3] $M3) (Plan $M3 "$M4#P")
    Save-Json (PlanFile $workOf[$M4] $M4) (Plan $M4 "$M3#P")
    Save-Json (PlanFile $workOf[$M5] $M5) (Plan $M5 "$M1#ZZ")
    Save-Json (PlanFile $workOf[$M6] $M6) (Plan $M6 'Other/1|a#S')
    Save-Json (PlanFile $workOf[$M7] $M7) (Plan $M7 "$M1#P")

    # M1 is not complete yet: M2 waits without spending its attempt, and no dependent fill runs.
    $waiting = Run-Digest @('model-deps', '-Out', $out, '-Model', $M2, '-Token', "$($tokenOf[$M2])")
    Assert ($waiting.ok -and $waiting.waiting -and @($waiting.prerequisites).Count -eq 1 -and $waiting.prerequisites[0].producer -eq $unitOf[$M1]) "an incomplete producer makes the dependent wait on it ($($waiting | ConvertTo-Json -Compress -Depth 4))"
    $m2Row = @((Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'models')).units | Where-Object { $_.id -eq $unitOf[$M2] })[0]
    Assert ($m2Row.state -eq 'registered' -and $m2Row.attempts -eq 0) 'a dependency wait returns the claim reservation instead of spending an attempt'
    $none = Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'models', '-Take', '10', '-Owner', 'model-manager-2')
    Assert (@($none.units | Where-Object { $_.id -eq $unitOf[$M2] }).Count -eq 0) 'a waiting dependent is never handed out early'

    # Build and complete M1 through its own compiler and the checked Model completion.
    $null = Run-Digest @('model', '-Step', 'apply', '-Work', $workOf[$M1], '-Staging', $out)
    $s1 = (State $workOf[$M1]).models[$M1].ids['S']
    $p1 = (State $workOf[$M1]).models[$M1].ids['P']
    $m1Head = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $M1 } })
    $m1Structure = ([regex]'(?s)\*\*结构\*\*.*$').Match($m1Head.body).Value
    Invoke-Zt @(
        @{ tool = 'edit_card'; arguments = @{ at = $M1; body = "问题：清算价值怎样决定融资？机制：还款上限。结论：清算价值够大才能融资。`n`n来源：第一章。`n`n$m1Structure" } },
        @{ tool = 'edit_card'; arguments = @{ at = $s1; body = '企业家有财富 $w$，项目需要 $K$，清算价值为 $L$。' } },
        @{ tool = 'edit_card'; arguments = @{ at = $p1; body = "清算价值够大，项目才能融资。依赖 [[$s1]]。`n`n来源：第一章。" } }
    ) | Out-Null
    $null = Run-Digest @('model', '-Step', 'report', '-Work', $workOf[$M1], '-Staging', $out)
    $m1Done = Run-Digest @('model-finish', '-Out', $out, '-Model', $M1, '-Token', "$($tokenOf[$M1])")
    Assert ($m1Done.state -eq 'done' -and $m1Done.parts.S -eq $s1) 'the producer model completes through its checked Model unit'
    $m1Fingerprints = $m1Done.card_fingerprints | ConvertTo-Json -Compress

    # A late successful prerequisite releases the dependent with its own full budget.
    $releasedBatch = Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'models', '-Take', '10', '-Owner', 'model-manager-2')
    $released = @($releasedBatch.units | Where-Object { $_.id -eq $unitOf[$M2] })[0]
    $releasedRow = @((Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'models')).units | Where-Object { $_.id -eq $unitOf[$M2] })[0]
    Assert ($released -and $released.token -eq ($tokenOf[$M2] + 1) -and $releasedRow.attempts -eq 1 -and $releasedRow.state -eq 'claimed') 'the late prerequisite releases the dependent with its own full attempt budget'
    $ready = Run-Digest @('model-deps', '-Out', $out, '-Model', $M2, '-Token', "$($released.token)")
    Assert ($ready.ok -and -not $ready.waiting -and @($ready.refs).Count -eq 1 -and $ready.refs[0].address -eq $p1) 'a completed producer resolves the exact named part for the dependent'
    Assert ($ready.refs_file -and [IO.File]::Exists($ready.refs_file.path) -and $ready.refs_file.sha256) 'the resolved reference is retained as evidence'
    $m2State = State $workOf[$M2]
    Assert ($m2State.models[$M1].ids['P'] -eq $p1 -and $m2State.models[$M2].refs["$M1#P"].address -eq $p1) 'the compiler can resolve the cross-model reference from its own part map'
    # Now the dependent's plan validates and its fill runs against the producer's real output.
    $applied = Run-Digest @('model', '-Step', 'apply', '-Work', $workOf[$M2], '-Staging', $out)
    Assert ($applied.models[0].status -eq 'built') 'the dependent plan validates only after its producer dependency is resolved'
    $m2Ids = (State $workOf[$M2]).models[$M2].ids
    $m2Head = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $M2 } })
    $m2Structure = ([regex]'(?s)\*\*结构\*\*.*$').Match($m2Head.body).Value
    Invoke-Zt @(
        @{ tool = 'edit_card'; arguments = @{ at = $M2; body = "问题：第二个模型。机制：依赖生产者。结论：成立。`n`n来源：第一章。`n`n$m2Structure" } },
        @{ tool = 'edit_card'; arguments = @{ at = $m2Ids['S']; body = '第二个模型的设定。' } },
        @{ tool = 'edit_card'; arguments = @{ at = $m2Ids['P']; body = "命题成立。依赖 [[$($m2Ids['T'])]]。`n`n来源：第一章。" } }
    ) | Out-Null
    $null = Run-Digest @('model', '-Step', 'report', '-Work', $workOf[$M2], '-Staging', $out)
    $m2Done = Run-Digest @('model-finish', '-Out', $out, '-Model', $M2, '-Token', "$($released.token)")
    Assert ($m2Done.state -eq 'done' -and $m2Done.complete) 'the dependent model completes on the producer output it waited for'
    $m1After = @((Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'models')).units | Where-Object { $_.id -eq $unitOf[$M1] })[0]
    Assert (($m1After.result.card_fingerprints | ConvertTo-Json -Compress) -eq $m1Fingerprints -and $m1After.result.reused -eq $false) 'the completed neighbour model keeps its build fingerprints and receipts'

    # A dependency cycle is detected before scheduling: both models become content faults with
    # the real closed path, and no premise is invented.
    $null = Run-Digest @('model-deps', '-Out', $out, '-Model', $M3, '-Token', "$($tokenOf[$M3])")
    $cycle = Run-Digest @('model-deps', '-Out', $out, '-Model', $M4, '-Token', "$($tokenOf[$M4])")
    Assert (-not $cycle.ok -and $cycle.fault.code -eq 'dependency_cycle' -and @($cycle.cycle).Count -ge 3 -and $cycle.cycle[0] -eq $cycle.cycle[-1]) "the cycle is refused with its closed path ($($cycle | ConvertTo-Json -Compress -Depth 5))"
    $rows = (Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'models')).units
    foreach ($m in $M3, $M4) {
        $row = @($rows | Where-Object { $_.id -eq $unitOf[$m] })[0]
        Assert ($row.state -eq 'failed' -and $row.fault.code -eq 'dependency_cycle' -and @($row.fault.cycle) -contains $unitOf[$M3] -and @($row.fault.cycle) -contains $unitOf[$M4]) "the cycle names both producers for $m"
    }

    # A completed producer without the named part is a precise content fault.
    $missing = Run-Digest @('model-deps', '-Out', $out, '-Model', $M5, '-Token', "$($tokenOf[$M5])")
    Assert (-not $missing.ok -and $missing.fault.code -eq 'named_part_missing' -and $missing.fault.part -eq 'ZZ' -and $missing.fault.model -eq $M1 -and $missing.fault.producer -eq $unitOf[$M1]) "a missing named part is a precise content fault ($($missing.fault | ConvertTo-Json -Compress))"
    $m5Row = @((Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'models')).units | Where-Object { $_.id -eq $unitOf[$M5] })[0]
    Assert ($m5Row.state -eq 'failed' -and $m5Row.fault.code -eq 'named_part_missing') 'the dependent records the missing-part fault instead of running its fill'

    # A model cannot depend on itself; an unregistered producer is also a content fault.
    Save-Json (PlanFile $workOf[$M6] $M6) (Plan $M6 "$M6#P")
    Refused @('model-deps', '-Out', $out, '-Model', $M6, '-Token', "$($tokenOf[$M6])") 'cannot depend on its own part' 'a self reference is refused before any write'
    Save-Json (PlanFile $workOf[$M6] $M6) (Plan $M6 'Other/1|a#S')
    $unknown = Run-Digest @('model-deps', '-Out', $out, '-Model', $M6, '-Token', "$($tokenOf[$M6])")
    Assert (-not $unknown.ok -and $unknown.fault.code -eq 'unknown_producer') 'an unregistered producer is a precise content fault'

    # A part changed after its accepted output cannot be reused by a dependent.
    $p1Body = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $p1 } })).body
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $p1; body = "$p1Body`n`n后来的一条独立补充。" } }) | Out-Null
    $changed = Run-Digest @('model-deps', '-Out', $out, '-Model', $M7, '-Token', "$($tokenOf[$M7])")
    Assert (-not $changed.ok -and $changed.fault.code -eq 'named_part_changed' -and $changed.fault.address -eq $p1) 'a changed named part is refused instead of reused'
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $p1; body = $p1Body } }) | Out-Null
    $m7Row = @((Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'models')).units | Where-Object { $_.id -eq $unitOf[$M7] })[0]
    Assert ($m7Row.state -eq 'failed' -and $m7Row.fault.code -eq 'named_part_changed') 'the changed-part fault is retained on the dependent unit'
    $gate = Run-Digest @('ledger', 'gate', '-Out', $out, '-Layer', 'models')
    Assert ($gate.sealed -and $gate.settled -and -not $gate.complete -and $gate.counts.done -eq 2 -and $gate.counts.failed -eq 5) "the Models gate settles partial with two done and five named content faults ($($gate.counts | ConvertTo-Json -Compress))"
    Assert (@($gate.unresolved | Where-Object { $_ -in @($unitOf[$M3], $unitOf[$M4], $unitOf[$M5], $unitOf[$M6], $unitOf[$M7]) }).Count -eq 5) 'every failed dependent stays named in the gate instead of reporting done'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
