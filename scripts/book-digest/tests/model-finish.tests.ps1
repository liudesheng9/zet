#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\model-finish.tests.ps1
# R218/R235/R236: the Models/model unit completes only through its own compiler's checked
# step state, accepted plan map and authored Cards. A placeholder blocks completion; a
# change after the check makes the retained completion stale; explanation work is emitted as
# its own registered unit. One scratch service and one prepared book; the live archive is not
# touched.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "model-finish-test-$PID"))
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
function DepCondition([string]$database, [string]$unit, [string]$needs) {
    $query = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');
const db=new DatabaseSync(path.resolve(process.argv[1]));
const row=db.prepare('select condition from deps where unit_id=? and needs_id=?').get(process.argv[2],process.argv[3]);
console.log(row && row.condition ? row.condition : 'none');db.close();
'@
    return (@(& node --no-warnings -e $query $database $unit $needs) -join '').Trim()
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $out = Join-Path $scratch 'prepared'
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source, "# 第一章`n`n企业家有财富 `$w`$，项目需要 `$K`$。`n`n清算价值为 `$L`$。`n`n式 (5.5)：`$P\le L`$。`n`n于是得到 (5.10)。`n`n# 参考`n未选择。", $utf8)
    $options = Join-Path $scratch 'options.json'; Save-Json $options @{ terms = $false }
    $null = Run-Digest @('prepare', '-Source', $source, '-Out', $out, '-CitationKey', 'FinishBook', '-BibTeX', '@book{FinishBook,title={Finish book}}', '-StartHeading', '第一章', '-EndHeading', '参考', '-MinUnitChars', '1', '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')
    # One digest group writes its anchor and two Model Cards; the group registers one Model
    # unit per actual Model header, and its offline check defers their quality work to them.
    $claim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'digest-manager')).units[0]
    $started = Run-Digest @('group-start', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)")
    $job = Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $draft = @{ schema_version = 1; unit = $claim.id; token = $claim.token; input_hash = $started.input_hash; disposition = 'cards'; calls = @(
        @{ tool = 'edit_card'; arguments = @{ at = 'FinishBook/1'; title = '融资约束影响企业投资'; body = '融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。'; base_fingerprint = $job.inputs.anchor.fingerprint } },
        @{ tool = 'create_successor'; arguments = @{ at = 'FinishBook/1'; kind = 'side'; type = 'model'; level = 'formal'; title = '融资模型：清算价值决定能否融资'; body = "问题：企业能否融到 `$K-w`$？`n`n来源：第一章，(5.5)–(5.10)。" } },
        @{ tool = 'create_successor'; arguments = @{ at = 'FinishBook/1'; kind = 'side'; type = 'model'; level = 'concept'; title = '概念模型：抵押与融资'; body = '问题：抵押怎样影响融资？' } }
    ) }
    Save-Json $started.draft $draft
    $group = Run-Digest @('group-finish', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)", '-Draft', $started.draft)
    $model = @($group.owned | Where-Object { $_ -match '\|a$' })[0]
    $concept = @($group.owned | Where-Object { $_ -match '\|b$' })[0]
    Assert ($model -and $concept -and $group.state -eq 'done') "the group completes and registers both Model candidates ($model, $concept)"
    $modelClaim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'models', '-Take', '1', '-Owner', 'model-manager')).units | Where-Object { $_.inputs.header -eq $model } | Select-Object -First 1
    Assert ($modelClaim -and $modelClaim.inputs.producer.producer -eq 'digest/g001@1' -and $modelClaim.id -like 'models/g001-m*@1') "the Models layer hands out the registered Model unit ($($modelClaim.id))"
    Assert (@((Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'models')).units).Count -eq 2) 'one Model unit is registered per actual Model header, not per group'
    $unit = $modelClaim.id; $token = $modelClaim.token

    # The build goes through the real model compiler: brief, plan, apply, fill, report, derive, review.
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
    $null = & $step 'apply'
    $state = State $work
    $ids = $state.models[$model].ids
    $setting = $ids['S']; $assumption = $ids['A']; $derivation = $ids['D']; $proposition = $ids['P']; $example = $ids['E']
    $headCard = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })
    $structure = ([regex]'(?s)\*\*结构\*\*.*$').Match($headCard.body).Value
    Invoke-Zt @(
        @{ tool = 'edit_card'; arguments = @{ at = $model; body = "问题：企业能否融到 `$K-w`$？机制：清算价值决定还款上限。结论：清算价值够大才能融资。`n`n来源：第一章，(5.5)–(5.10)。`n`n$structure" } },
        @{ tool = 'edit_card'; arguments = @{ at = $setting; body = '企业家有财富 $w$，项目需要 $K$，清算价值为 $L$。' } },
        @{ tool = 'edit_card'; arguments = @{ at = $assumption; body = "投资者只认清算价值。依赖 [[$setting]]：用到 `$L`$。" } },
        @{ tool = 'edit_card'; arguments = @{ at = $proposition; body = "清算价值够大，项目才能融资。依赖 [[$derivation]] 的结果。`n`n来源：第一章，(5.10)。" } },
        @{ tool = 'edit_card'; arguments = @{ at = $example; body = "代入 `$L=6`$：项目能融资。它站在 [[$proposition]] 旁边。" } }
    ) | Out-Null
    $finish = @('model-finish', '-Out', $out, '-Model', $model, '-Token', "$token")
    # The report sees the Derivation part still unwritten: its checked todo blocks completion.
    $partial = & $step 'report'
    Assert ($partial.models[0].todo.Count -eq 1) 'the report names the one unwritten Derivation part'
    Refused $finish 'the last report names unwritten parts' 'a model with an unwritten placeholder cannot complete'
    # All parts written, but the report and the derive/review commands are still missing.
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $derivation; body = "**起点**`n- 依赖 [[$setting]] 的结果：清算价值为 `$L`$。`n`n**步骤**`n1. 还款不超过清算价值。`n`n**结果**`n`$`$P\le L`$`$" } }) | Out-Null
    Refused $finish 'the last report names unwritten parts' 'a report older than the fill cannot complete the model'
    $report = & $step 'report'
    Assert ($report.models[0].todo.Count -eq 0 -and $report.models[0].parts -eq 5) 'the fresh report names five parts and no unwritten placeholder'
    Refused $finish 'missing derive evidence' 'a formal model without a commanded derive step cannot complete'
    $derive = & $step 'derive'
    Assert (@($derive.jobs).Count -eq 1) 'the derive step commands the derivation job'
    Refused $finish 'missing review evidence' 'a formal model without a commanded review cannot complete'
    $review = & $step 'review'
    Assert (@($review.jobs).Count -eq 1) 'the review step commands the review job'

    # Preview writes nothing; the checked evidence is the compiler's own step state and Cards.
    $checkOnly = Run-Digest ($finish + @('-CheckOnly'))
    Assert ($checkOnly.check_only -and -not $checkOnly.applied -and $checkOnly.kind -eq 'model_completion') 'the preview checks the model without writing'
    Assert ($checkOnly.explanation.status -eq 'required' -and @($checkOnly.explanation.needs).Count -eq 1 -and @($checkOnly.explanation.needs[0].missing) -contains '思路' -and @($checkOnly.explanation.needs[0].missing) -contains '解读') 'the unexplained Derivation part is the exact explanation need'
    $key = $unit -replace '^models/', '' -replace '@1$', ''
    Assert (-not (Test-Path (Join-Path $out "model-completion-$key.json"))) 'the preview publishes no evidence'
    # A lost ledger commit leaves the unit claimed and writes no done state.
    $env:ZT_LEDGER_COMPLETE_FAULT = 'after_outputs'
    Refused $finish 'injected completion fault' 'an injected fault rolls the Model completion back after its evidence is written'
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    Assert (@((Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'models')).units | Where-Object { $_.id -eq $unit })[0].state -eq 'claimed') 'the injected fault leaves the Model unit claimed'
    $done = Run-Digest $finish
    Assert ($done.schema_version -eq 1 -and $done.kind -eq 'model_completion' -and $done.unit -eq $unit -and $done.state -eq 'done' -and $done.complete -and -not $done.reused) 'the real model completion records typed evidence'
    Assert ($done.model -eq $model -and $done.level -eq 'formal' -and $done.plan_hash -and @($done.parts.Keys).Count -eq 5 -and @($done.part_order).Count -eq 5) 'the completion retains the accepted plan, level and exact part map'
    Assert ($done.steps.brief.context_hash -and $done.steps.plan.plan_hash -eq $done.plan_hash -and $done.steps.apply.plan_hash -eq $done.plan_hash -and $done.steps.derive.jobs -eq 1 -and $done.steps.review.jobs -eq 1 -and $done.steps.report.todo -eq 0 -and $done.steps.report.parts -eq 5) 'every required step has command evidence for the accepted plan'
    Assert ($done.structure.lines.Count -eq 5 -and $done.structure.fingerprint -and @($done.links.requirements).Count -eq 4) 'the completion retains the structure contribution and every required semantic Link'
    Assert (@($done.trees).Count -eq 1 -and $done.trees[0] -eq 'FinishBook' -and $done.build.order -eq 5 -and $done.style -and $done.style.count -eq 0) 'the completion names the touched tree and keeps style/build facts as tolerances'
    Assert ($done.explanation.status -eq 'required' -and $done.explain_unit -eq "explain/$key@1") 'the completion emits one explanation unit for its unchecked explanation work'
    $view = Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'models')
    $row = @($view.units | Where-Object { $_.id -eq $unit })[0]
    Assert ($row.state -eq 'done' -and $row.result.kind -eq 'model_completion' -and $row.result.complete) 'the ledger records the checked Model completion'
    $explainView = Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'explain')
    $explainRow = @($explainView.units | Where-Object { $_.id -eq $done.explain_unit })[0]
    Assert ($explainRow -and $explainRow.state -eq 'registered') 'the explanation need is a registered Explain unit'
    Assert ((DepCondition (Join-Path $out 'ledger.sqlite3') $done.explain_unit $unit) -eq 'success') 'the explanation unit depends on the completed model unit'
    $pendingGate = @(& $Digest @('ledger', 'gate', '-Out', $out, '-Layer', 'models') 2>&1)
    Assert ($LASTEXITCODE -eq 3 -and ($pendingGate -join ' ').Contains('models/g001-m0002@1')) 'an open Model unit keeps the Models gate unsettled'

    # The same evidence through the generic ledger command, and a repeated completion.
    $completionFile = Join-Path $out "model-completion-$key.json"
    Assert ((Test-Path $completionFile) -and $done.files.completion.sha256 -and $done.files.check.sha256) 'the completion and check evidence are retained and hash-bound'
    $again = Run-Digest @('ledger', 'done', '-Out', $out, '-Unit', $unit, '-Token', "$token", '-Outputs', $completionFile)
    Assert ($again.reused -and $again.unit -eq $unit -and $again.complete) 'ledger done dispatches the same Model verifier and reuses the checked result'
    Refused @('ledger', 'done', '-Out', $out, '-Unit', $unit, '-Token', "$token", '-Outputs', (Join-Path $out 'completion-g001.json')) 'invalid Model completion evidence' 'another layer evidence cannot complete a Model unit'
    Refused @('model-finish', '-Out', $out, '-Model', $model, '-Token', "$($token + 1)") 'stale_claim' 'a wrong claim token cannot complete the Model unit'
    Refused @('model-finish', '-Out', $out, '-Model', 'FinishBook/1|z', '-Token', "$token") 'no registered Model unit' 'an unknown model has no Model obligation'
    # A changed part makes the retained completion stale instead of describing different Cards.
    $propositionBody = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $proposition } })).body
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $proposition; body = "$propositionBody`n`n后来的一条独立补充。" } }) | Out-Null
    Refused $finish 'completed Model unit changed' 'a later edit of a part makes the completed Model check stale'
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $proposition; body = $propositionBody } }) | Out-Null
    Assert ((Run-Digest $finish).reused) 'restoring the checked part recovers the same Model completion'

    # A concept model records an explicit explanation no-op instead of an empty obligation.
    $conceptSlug = $concept -replace '[\\/|]', '_'
    $conceptWork = Join-Path $out "models\m-$conceptSlug"
    $conceptStep = { param($s, [string[]]$more = @()) Run-Digest (@('model', '-Step', $s, '-Work', $conceptWork, '-Staging', $out) + $more) }
    $null = & $conceptStep 'brief' @('-Models', $concept)
    $conceptPlan = [ordered]@{
        model = $concept; level = 'concept'; card = '问题、机制、结论'
        parts = @(
            [ordered]@{ id = 'C1'; line = 'main'; facet = '机制'; title = '机制：抵押提高还款承诺'; spec = '机制'; src = @('sections/s01.md:2') }
        )
    }
    Save-Json (Join-Path (Join-Path $conceptWork $conceptSlug) 'plan.json') $conceptPlan
    $null = & $conceptStep 'apply'
    $conceptState = State $conceptWork
    $conceptIds = $conceptState.models[$concept].ids
    $conceptHead = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $concept } })
    $conceptStructure = ([regex]'(?s)\*\*结构\*\*.*$').Match($conceptHead.body).Value
    Invoke-Zt @(
        @{ tool = 'edit_card'; arguments = @{ at = $concept; body = "问题：抵押怎样影响融资？机制：抵押提高还款承诺。结论：有抵押的企业更容易融资。`n`n来源：第一章。`n`n$conceptStructure" } },
        @{ tool = 'edit_card'; arguments = @{ at = $conceptIds['C1']; body = "抵押提高还款承诺，投资者愿意放款。`n`n来源：第一章。" } }
    ) | Out-Null
    $null = & $conceptStep 'report'
    $conceptClaim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'models', '-Take', '5', '-Owner', 'model-manager')).units | Where-Object { $_.inputs.header -eq $concept } | Select-Object -First 1
    Assert ($conceptClaim) 'the second Model candidate is a registered Model unit'
    $conceptDone = Run-Digest @('model-finish', '-Out', $out, '-Model', $concept, '-Token', "$($conceptClaim.token)")
    Assert ($conceptDone.complete -and $conceptDone.explanation.status -eq 'none' -and $conceptDone.explanation.reason -eq 'concept' -and $conceptDone.explanation.applicable -eq $false) 'a concept model records the explanation layer as inapplicable'
    Assert (-not $conceptDone.explain_unit) 'a no-op explanation registers no explanation obligation'
    $gate = Run-Digest @('ledger', 'gate', '-Out', $out, '-Layer', 'models')
    Assert ($gate.sealed -and $gate.settled -and $gate.counts.done -eq 2 -and -not $gate.complete) 'the Models layer settles only after both Model units are done, without claiming the run complete'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
