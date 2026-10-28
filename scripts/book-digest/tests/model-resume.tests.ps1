#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\model-resume.tests.ps1
# R235/R236: a partly built model is selected by its checked step state, not by "parts greater
# than zero". resume starts at the first missing/unwritten part in build order, reuses the parts
# whose authored text is already written, and refuses a stale part map. The live archive is not
# touched: one scratch service and one prepared book.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "model-resume-test-$PID"))
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
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $out = Join-Path $scratch 'prepared'
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source, "# 第一章`n`n企业家有财富 `$w`$，项目需要 `$K`$。`n`n清算价值为 `$L`$。`n`n式 (5.5)：`$P\le L`$。`n`n于是得到 (5.10)。`n`n# 参考`n未选择。", $utf8)
    $options = Join-Path $scratch 'options.json'; Save-Json $options @{ terms = $false }
    $null = Run-Digest @('prepare', '-Source', $source, '-Out', $out, '-CitationKey', 'ResumeBook', '-BibTeX', '@book{ResumeBook,title={Resume book}}', '-StartHeading', '第一章', '-EndHeading', '参考', '-MinUnitChars', '1', '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')
    $model = 'ResumeBook/1|a'
    Invoke-Zt @(
        @{ tool = 'create_successor'; arguments = @{ at = 'ResumeBook/1'; kind = 'side'; type = 'model'; level = 'formal'; title = '融资模型：清算价值决定能否融资'; body = "问题：企业能否融到 `$K-w`$？`n`n来源：第一章，(5.5)–(5.10)。" } }
    ) | Out-Null

    $slug = 'ResumeBook_1_a'
    $work = Join-Path $out "models\m-$slug"
    $dir = Join-Path $work $slug
    $step = { param($s, [string[]]$more = @()) Run-Digest (@('model', '-Step', $s, '-Work', $work, '-Staging', $out) + $more) }
    $brief = & $step 'brief' @('-Models', $model)
    Assert (@($brief.jobs).Count -eq 1 -and @($brief.skipped).Count -eq 0) 'brief: a Model Card with no parts gets its context pack and brief job'
    $plan = [ordered]@{
        model = $model; level = 'formal'; card = '问题、机制、结论'
        symbols = @(, @('$L$', '清算价值')); terms = @(, @('清算', '卖资产', '变卖资产'))
        parts = @(
            [ordered]@{ id = 'S'; line = 'main'; facet = '设定'; emphasis = 'core'; title = '设定：企业家、项目与清算价值'; spec = '参与者与时点'; src = @('sections/s01.md:2-3') },
            [ordered]@{ id = 'A'; line = 'main'; facet = '假设'; title = '假设：投资者只认清算价值'; spec = '假设'; rests = @(, @('S', '用到 $L$')); src = @('sections/s01.md:2-2') }
            [ordered]@{ id = 'D'; line = 'main'; derive = $true; facet = '推导：还款上限'; title = '推导：还款不超过清算价值'; from = '(5.5)'; to = '$$P\le L$$'; steps = 2; rests = @(, @('S', '用到 $L$')); src = @('sections/s01.md:4-4') }
            [ordered]@{ id = 'P'; line = 'main'; facet = '命题'; emphasis = 'core'; title = '命题：清算价值够大才能融资'; spec = '命题'; rests = @(, @('D', '用到 (5.10)')); src = @('sections/s01.md:5-5') }
            [ordered]@{ id = 'E'; line = 'side'; of = 'P'; facet = '数值例子'; emphasis = 'aside'; title = '例子：L=6 时能融资'; spec = '代入数字'; src = @('sections/s01.md:3-3') }
        )
        explain = @([ordered]@{
            chain = @('D'); route = '从清算价值直接得到还款上限。'
            parts = @([ordered]@{ id = 'D'; shows = '还款不超过清算价值'; notes = '第 1 步取两种情形'; meaning = '还款上限由清算价值决定' })
        })
    }
    Save-Json (Join-Path $dir 'plan.json') $plan
    $applied = & $step 'apply'
    Assert ($applied.models[0].status -eq 'built' -and @($applied.jobs).Count -eq 1) "apply: the skeleton is built and its one fill job per model is planned ($($applied.models[0].error))"
    $state = State $work
    $ids = $state.models[$model].ids
    Assert (@($ids.Keys | Where-Object { $_ -in 'S', 'A', 'D', 'P', 'E' }).Count -eq 5) 'apply: every planned part has an address in the part map'
    $setting = $ids['S']; $assumption = $ids['A']; $derivation = $ids['D']; $proposition = $ids['P']; $example = $ids['E']
    # Fill the Model Card, the setting, the assumption and the example. The derivation comes before
    # the proposition in build order, so resume must repair the derivation first.
    $headCard = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })
    $structure = ([regex]'(?s)\*\*结构\*\*.*$').Match($headCard.body).Value
    Invoke-Zt @(
        @{ tool = 'edit_card'; arguments = @{ at = $model; body = "问题：企业能否融到 `$K-w`$？机制：清算价值决定还款上限。结论：清算价值够大才能融资。`n`n来源：第一章，(5.5)–(5.10)。`n`n$structure" } },
        @{ tool = 'edit_card'; arguments = @{ at = $setting; body = '企业家有财富 $w$，项目需要 $K$，清算价值为 $L$。' } },
        @{ tool = 'edit_card'; arguments = @{ at = $assumption; body = "投资者只认清算价值。依赖 [[$setting]]：用到 `$L`$。" } },
        @{ tool = 'edit_card'; arguments = @{ at = $example; body = "代入 `$L=6`$：项目能融资。它站在 [[$proposition]] 旁边。" } }
    ) | Out-Null
    $listed = Run-Digest @('list-models', '-Staging', $out, '-All', '-CitationKey', 'ResumeBook')
    $row = @($listed.models | Where-Object { $_.address -eq $model })[0]
    Assert ($row.parts -gt 0 -and $row.todo -eq 2 -and @($row.placeholders) -contains $derivation -and @($row.placeholders) -contains $proposition) "list-models: a partly built model reports its parts and the exact unwritten placeholders ($($row | ConvertTo-Json -Compress -Depth 4))"

    # resume 1: the first missing part in build order is the derivation, so its derive job comes
    # first; the later proposition is not filled before the derivation's result exists.
    $first = & $step 'resume' @('-Models', $model)
    $r1 = @($first.models | Where-Object { $_.model -eq $model })[0]
    Assert ($r1.from -eq $derivation -and @($r1.fill).Count -eq 0 -and @($first.jobs).Count -eq 1) "resume: starts at the derivation before the later proposition ($($first | ConvertTo-Json -Compress -Depth 4))"
    $state = State $work
    $job1 = $state.jobs[$first.jobs[0].id]
    Assert (@($job1.addresses) -contains $derivation -and (Get-Content -LiteralPath $first.jobs[0].files[2] -Raw -Encoding utf8).Contains("[[$derivation]]")) 'resume: the derive job is the real compiler job for that part'
    Assert (@($r1.reused.Keys).Count -eq 4 -and $r1.reused[$setting] -and $r1.reused[$assumption] -and $r1.reused[$example] -and $r1.reused[$model]) "resume: the written parts are reused with their authored fingerprints [S=$setting A=$assumption E=$example keys=$(@($r1.reused.Keys) -join ','), S=$($r1.reused[$setting])]"
    Assert ((State $work).models[$model].status -eq 'derived') 'resume: the derivation round is recorded in the model status'
    $settingFingerprint = $r1.reused[$setting]

    # Fill the derivation, then resume again: the first missing part is now the proposition.
    $derivationBody = "**思路**`n第 1 张，共 1 张。推出 `$P\le L`$。`n先求还款，再比较。`n`n**起点**`n- 依赖 [[$setting]] 的结果：清算价值为 `$L`$。`n`n**步骤**`n1. 还款不超过清算价值。`n`n**结果**`n`$`$P\le L`$`$`n`n**解读**`n还款不超过清算价值。"
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $derivation; body = $derivationBody } }) | Out-Null
    $second = & $step 'resume' @('-Models', $model)
    $r2 = @($second.models | Where-Object { $_.model -eq $model })[0]
    Assert ($r2.from -eq $proposition -and @($r2.derive).Count -eq 0 -and @($second.jobs).Count -eq 1) "resume: after the derivation the first missing step is the proposition ($($second | ConvertTo-Json -Compress -Depth 4))"
    $state = State $work
    $job2 = $state.jobs[$second.jobs[0].id]
    Assert (@($job2.addresses) -contains $proposition) 'resume: the fill job is the real compiler job for the unwritten part'
    Assert ($r2.reused[$derivation] -and $r2.reused[$setting] -eq $settingFingerprint) 'resume: fingerprints are stable for the parts that did not change'

    # Fill the proposition, then resume: no part is missing any more and no job is invented.
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $proposition; body = "清算价值够大，项目才能融资。依赖 [[$derivation]] 的结果。`n`n来源：第一章，(5.10)。" } }) | Out-Null
    $third = & $step 'resume' @('-Models', $model)
    $r3 = @($third.models | Where-Object { $_.model -eq $model })[0]
    Assert (@($third.jobs).Count -eq 0 -and $r3.complete -and @($r3.fill).Count -eq 0 -and @($r3.derive).Count -eq 0) "resume: a fully written model reports complete and starts no job ($($third | ConvertTo-Json -Compress -Depth 4))"
    $again = & $step 'resume' @('-Models', $model)
    Assert (@($again.jobs).Count -eq 0 -and $again.models[0].complete) 'resume: a repeated call is stable and starts no duplicate job'

    # A facet that no longer matches the accepted plan is a stale mapping, not reused work.
    $settingCard = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $setting } })
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $setting; facet = '设定（改）'; body = $settingCard.body } }) | Out-Null
    Refused (@('model', '-Step', 'resume', '-Work', $work, '-Staging', $out, '-Models', $model)) 'stale part' 'resume: a part whose facet differs from the accepted plan is a precise stale-mapping fault'
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $setting; facet = $settingCard.facet; body = $settingCard.body } }) | Out-Null
    Assert ((& $step 'resume' @('-Models', $model)).models[0].complete) 'resume: restoring the accepted facet recovers the same complete model'

    # A part map that names a Card the archive does not hold cannot be repaired by recreating it.
    $statePath = Join-Path $work 'state.json'
    $state = State $work
    $saved = $state.models[$model].ids['E']
    $state.models[$model].ids['E'] = 'ResumeBook/1|z'
    Save-Json $statePath $state
    Refused (@('model', '-Step', 'resume', '-Work', $work, '-Staging', $out, '-Models', $model)) 'missing part' 'resume: a part address the archive does not hold is a precise fault, not a new skeleton'
    $state = State $work
    $state.models[$model].ids['E'] = $saved
    Save-Json $statePath $state
    Assert ((& $step 'resume' @('-Models', $model)).models[0].complete) 'resume: the restored part map is complete again'

    # brief: a model with unwritten parts is not skipped even though it has many parts.
    $propositionCard = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $proposition } })
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $proposition; body = '待写' } }) | Out-Null
    $todoAgain = Run-Digest @('list-models', '-Staging', $out, '-All', '-CitationKey', 'ResumeBook')
    $rowAgain = @($todoAgain.models | Where-Object { $_.address -eq $model })[0]
    Assert ($rowAgain.parts -gt 0 -and $rowAgain.todo -eq 1) 'the model still has parts but one unwritten placeholder, so "parts greater than zero" would call it done'
    $rebrief = & $step 'brief' @('-Models', $model)
    Assert (@($rebrief.skipped).Count -eq 0 -and @($rebrief.jobs).Count -eq 1) "brief: a model with unwritten placeholders is not skipped ($($rebrief | ConvertTo-Json -Compress -Depth 4))"
    $null = $propositionCard

    # A model without an accepted plan has nothing to resume.
    Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{ at = 'ResumeBook/1'; kind = 'side'; type = 'model'; level = 'formal'; title = '第二个模型'; body = '问题：另一个问题？' } }) | Out-Null
    Refused (@('model', '-Step', 'resume', '-Work', $work, '-Staging', $out, '-Models', 'ResumeBook/1|b')) 'no accepted plan' 'resume: a model that was never planned is refused before any write'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'

