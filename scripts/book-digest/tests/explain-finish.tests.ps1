#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\explain-finish.tests.ps1
# R219/R301/R302: a formal model with Derivation parts emits required explanation work, and the
# Explain/model unit completes only from the checked model: every in-scope Derivation part/chain
# has 思路 and 解读, the chain position lines are the compiler's own, and zt-check's explanation
# rules pass. A concept model records the layer as inapplicable; a model build stays complete
# while its explanation work is open; only the unfinished parts are planned and written. One
# scratch service and one prepared book; the live archive is not touched.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "explain-finish-test-$PID"))
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
    return ($lines -join ' ')
}
function Gate([string]$layer, [string]$text, [int]$code) {
    $lines = @(& $Digest @('ledger', 'gate', '-Out', $out, '-Layer', $layer) 2>&1)
    Assert ($LASTEXITCODE -eq $code -and ($lines -join ' ').Contains($text)) "gate $layer is exit $code and says $text [$($lines -join ' ')]"
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Save-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 30), $utf8) }
function State($work) { Get-Content -LiteralPath (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable }
function Receipts([string]$database) {
    $query = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');
const db=new DatabaseSync(path.resolve(process.argv[1]));
const row=db.prepare('select count(*) as n from applied_ops').get();
console.log(String(row.n));db.close();
'@
    return [int]((@(& node --no-warnings -e $query $database) -join '').Trim())
}
function Body($address) { return (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $address } })).body }
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $out = Join-Path $scratch 'prepared'
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source, "# 第一章`n`n企业家有财富 `$w`$，项目需要 `$K`$。`n`n清算价值为 `$L`$。`n`n式 (5.5)：`$P\le L`$。`n`n于是得到 (5.10)。`n`n# 参考`n未选择。", $utf8)
    $options = Join-Path $scratch 'options.json'; Save-Json $options @{ terms = $false }
    $null = Run-Digest @('prepare', '-Source', $source, '-Out', $out, '-CitationKey', 'ExplainBook', '-BibTeX', '@book{ExplainBook,title={Explain book}}', '-StartHeading', '第一章', '-EndHeading', '参考', '-MinUnitChars', '1', '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')
    # One digest group writes its anchor and three Model Cards: a formal model with a three-part
    # Derivation chain, a formal model with no Derivation part, and a concept model.
    $claim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'digest-manager')).units[0]
    $started = Run-Digest @('group-start', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)")
    $job = Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $draft = @{ schema_version = 1; unit = $claim.id; token = $claim.token; input_hash = $started.input_hash; disposition = 'cards'; calls = @(
        @{ tool = 'edit_card'; arguments = @{ at = 'ExplainBook/1'; title = '清算价值影响企业融资'; body = '清算价值改变企业的融资选择。清算价值高时企业借得到钱，投资随之增加。'; base_fingerprint = $job.inputs.anchor.fingerprint } },
        @{ tool = 'create_successor'; arguments = @{ at = 'ExplainBook/1'; kind = 'side'; type = 'model'; level = 'formal'; title = '融资模型：清算价值决定还款上限'; body = "问题：企业能否融到 `$K-w`$？`n`n来源：第一章，(5.5)–(5.10)。" } },
        @{ tool = 'create_successor'; arguments = @{ at = 'ExplainBook/1'; kind = 'side'; type = 'model'; level = 'formal'; title = '还款模型：项目价值决定参与'; body = "问题：投资者什么时候参与？`n`n来源：第一章，(5.10)。" } },
        @{ tool = 'create_successor'; arguments = @{ at = 'ExplainBook/1'; kind = 'side'; type = 'model'; level = 'concept'; title = '概念模型：抵押与融资'; body = '问题：抵押怎样影响融资？' } }
    ) }
    Save-Json $started.draft $draft
    $group = Run-Digest @('group-finish', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)", '-Draft', $started.draft)
    $formal = @($group.owned | Where-Object { $_ -match '\|a$' })[0]
    $plain = @($group.owned | Where-Object { $_ -match '\|b$' })[0]
    $concept = @($group.owned | Where-Object { $_ -match '\|c$' })[0]
    Assert ($formal -and $plain -and $concept -and $group.state -eq 'done') "the group completes and registers three Model candidates ($formal, $plain, $concept)"
    Assert (@((Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'models')).units).Count -eq 3) 'one Model unit is registered per actual Model header'

    # The formal model with a chain: brief, plan, apply, fill, report, derive, review.
    $slug = $formal -replace '[\\/|]', '_'
    $work = Join-Path $out "models\m-$slug"
    $dir = Join-Path $work $slug
    $step = { param($s, [string[]]$more = @()) Run-Digest (@('model', '-Step', $s, '-Work', $work, '-Staging', $out) + $more) }
    $null = & $step 'brief' @('-Models', $formal)
    $plan = [ordered]@{
        model = $formal; level = 'formal'; card = '问题、机制、结论'
        symbols = @(, @('$L$', '清算价值'), , @('$P$', '还款额')); terms = @(, @('清算', '卖资产', '变卖资产'))
        parts = @(
            [ordered]@{ id = 'S'; line = 'main'; facet = '设定'; emphasis = 'core'; title = '设定：企业家、项目与清算价值'; spec = '参与者与时点'; src = @('sections/s01.md:2-3') },
            [ordered]@{ id = 'A'; line = 'main'; facet = '假设'; title = '假设：投资者只认清算价值'; spec = '假设'; rests = @(, @('S', '用到 $L$')); src = @('sections/s01.md:2-2') }
            [ordered]@{ id = 'D1'; line = 'main'; derive = $true; facet = '推导：还款上限'; title = '推导：还款不超过清算价值'; from = '(5.5)'; to = '$$P\le L$$'; steps = 2; rests = @(, @('S', '用到 $L$')); src = @('sections/s01.md:4-4') }
            [ordered]@{ id = 'D2'; line = 'main'; derive = $true; facet = '推导：融资条件'; title = '推导：项目价值要超过成本'; from = '(5.10)'; to = '$$y_1+fy_2>K$$'; steps = 2; rests = @(, @('D1', '用到 (5.5)')); src = @('sections/s01.md:5-5') }
            [ordered]@{ id = 'D3'; line = 'main'; derive = $true; facet = '推导：净资产条件'; title = '推导：净资产要为正'; from = '(5.10)'; to = '$$K-w\le L$$'; steps = 2; rests = @(, @('D2', '用到 (5.10)')); src = @('sections/s01.md:5-5') }
            [ordered]@{ id = 'P'; line = 'main'; facet = '命题'; emphasis = 'core'; title = '命题：清算价值够大才能融资'; spec = '命题'; rests = @(, @('D3', '用到净资产条件')); src = @('sections/s01.md:5-5') }
        )
        explain = @([ordered]@{
            chain = @('D1', 'D2', 'D3'); route = '先求还款上限，再代入参与约束，最后看净资产条件。'
            parts = @(
                [ordered]@{ id = 'D1'; shows = '还款不超过清算价值'; notes = '第 2 步分情形讨论'; meaning = '还款上限由清算价值决定' },
                [ordered]@{ id = 'D2'; shows = '项目价值超过成本'; notes = ''; meaning = '项目价值够大时投资者参与' },
                [ordered]@{ id = 'D3'; shows = '净资产为正'; notes = '第 1 步移项'; meaning = '企业家自己的财富也要够' }
            )
        })
    }
    Save-Json (Join-Path $dir 'plan.json') $plan
    $null = & $step 'apply'
    $state = State $work
    $ids = $state.models[$formal].ids
    $setting = $ids['S']; $assumption = $ids['A']; $cap = $ids['D1']; $condition = $ids['D2']; $net = $ids['D3']; $proposition = $ids['P']
    $headCard = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $formal } })
    $structure = ([regex]'(?s)\*\*结构\*\*.*$').Match($headCard.body).Value
    Invoke-Zt @(
        @{ tool = 'edit_card'; arguments = @{ at = $formal; body = "问题：企业能否融到 `$K-w`$？机制：清算价值决定还款上限。结论：清算价值够大才能融资。`n`n来源：第一章，(5.5)–(5.10)。`n`n$structure" } },
        @{ tool = 'edit_card'; arguments = @{ at = $setting; body = '企业家有财富 $w$，项目需要 $K$，清算价值为 $L$。' } },
        @{ tool = 'edit_card'; arguments = @{ at = $assumption; body = "投资者只认清算价值。依赖 [[$setting]]：用到 `$L`$。" } },
        @{ tool = 'edit_card'; arguments = @{ at = $cap; body = "**起点**`n- 依赖 [[$setting]] 的结果：清算价值为 `$L`$。`n`n**步骤**`n1. 还款不超过清算价值。`n`n**结果**`n`$`$P\le L`$`$" } },
        @{ tool = 'edit_card'; arguments = @{ at = $condition; body = "**起点**`n- 依赖上一步 [[$cap]] 的结果：还款不超过清算价值。`n`n**步骤**`n1. 项目的价值超过成本。`n`n**结果**`n`$`$y_1+fy_2>K`$`$" } },
        @{ tool = 'edit_card'; arguments = @{ at = $net; body = "**起点**`n- 依赖上一步 [[$condition]] 的结果：项目价值超过成本。`n`n**步骤**`n1. 企业家自己的财富也要够。`n`n**结果**`n`$`$K-w\le L`$`$" } },
        @{ tool = 'edit_card'; arguments = @{ at = $proposition; body = "清算价值够大，项目才能融资。依赖 [[$net]] 的结果。`n`n来源：第一章，(5.10)。" } }
    ) | Out-Null
    $null = & $step 'report'
    $null = & $step 'derive'
    $null = & $step 'review'
    $formalClaim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'models', '-Take', '1', '-Owner', 'model-manager')).units | Where-Object { $_.inputs.header -eq $formal } | Select-Object -First 1
    $key = $formalClaim.id -replace '^models/', '' -replace '@1$', ''
    $built = Run-Digest @('model-finish', '-Out', $out, '-Model', $formal, '-Token', "$($formalClaim.token)")
    Assert ($built.complete -and $built.explanation.status -eq 'required' -and $built.explanation.applicable -eq $true) 'a formal model with Derivation parts emits required explanation work'
    Assert (@($built.explanation.needs).Count -eq 3 -and $built.explanation.derivations -eq 3 -and $built.explanation.explained -eq 0) 'the need counts the actual own Derivation parts of the checked model'
    Assert (@($built.explanation.needs[0].missing) -contains '思路' -and @($built.explanation.needs[0].missing) -contains '解读') 'both 思路 and 解读 are named as the missing sections'
    Assert (@($built.explanation.chains).Count -eq 1 -and @($built.explanation.chains[0].parts).Count -eq 3 -and @($built.explanation.chains[0].parts) -contains $net) 'the chain of the three neighbouring Derivation parts is the registered scope'
    Assert ($built.explain_unit -eq "explain/$key@1") 'the completion emits one Explain/model unit for its explanation work'
    $explainView = Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'explain')
    $explainRow = @($explainView.units | Where-Object { $_.id -eq $built.explain_unit })[0]
    Assert ($explainRow -and $explainRow.state -eq 'registered' -and $explainRow.inputs.model -eq $formal -and @($explainRow.inputs.parts).Count -eq 3) 'the explanation unit is registered with the exact checked-model scope'
    # A model whose build is done is not an explanation completion: the Explain layer stays open
    # while the other Model candidates are still unfinished.
    $pending = Gate 'explain' '"sealed":false' 3
    Assert (-not $pending.sealed -and -not $pending.settled) 'the Explain layer is unsettled while a Model producer can still register explanation work'

    # A formal model without a Derivation part: an explicit no-op, and no invented obligation.
    $plainSlug = $plain -replace '[\\/|]', '_'
    $plainWork = Join-Path $out "models\m-$plainSlug"
    $plainDir = Join-Path $plainWork $plainSlug
    $plainStep = { param($s, [string[]]$more = @()) Run-Digest (@('model', '-Step', $s, '-Work', $plainWork, '-Staging', $out) + $more) }
    $null = & $plainStep 'brief' @('-Models', $plain)
    $plainPlan = [ordered]@{
        model = $plain; level = 'formal'; card = '问题、机制、结论'
        symbols = @(, @('$y_1$', '项目第一期的收益')); terms = @(, @('参与', '加入', '进场'))
        parts = @(
            [ordered]@{ id = 'S'; line = 'main'; facet = '设定'; emphasis = 'core'; title = '设定：投资者与项目的价值'; spec = '参与者与时点'; src = @('sections/s01.md:2') },
            [ordered]@{ id = 'P'; line = 'main'; facet = '命题'; emphasis = 'core'; title = '命题：价值超过成本才参与'; spec = '命题'; rests = @(, @('S', '用到价值')); src = @('sections/s01.md:2-2') }
        )
    }
    Save-Json (Join-Path $plainDir 'plan.json') $plainPlan
    $null = & $plainStep 'apply'
    $plainState = State $plainWork
    $plainIds = $plainState.models[$plain].ids
    $plainHead = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $plain } })
    $plainStructure = ([regex]'(?s)\*\*结构\*\*.*$').Match($plainHead.body).Value
    Invoke-Zt @(
        @{ tool = 'edit_card'; arguments = @{ at = $plain; body = "问题：投资者什么时候参与？机制：项目价值与成本比较。结论：价值超过成本时参与。`n`n来源：第一章，(5.10)。`n`n$plainStructure" } },
        @{ tool = 'edit_card'; arguments = @{ at = $plainIds['S']; body = '投资者按项目价值决定是否参与。' } },
        @{ tool = 'edit_card'; arguments = @{ at = $plainIds['P']; body = "项目价值超过成本时投资者参与。依赖 [[$($plainIds['S'])]].`n`n来源：第一章，(5.10)。" } }
    ) | Out-Null
    $null = & $plainStep 'report'
    $plainClaim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'models', '-Take', '1', '-Owner', 'model-manager')).units | Where-Object { $_.inputs.header -eq $plain } | Select-Object -First 1
    $plainDone = Run-Digest @('model-finish', '-Out', $out, '-Model', $plain, '-Token', "$($plainClaim.token)")
    Assert ($plainDone.complete -and $plainDone.explanation.status -eq 'none' -and $plainDone.explanation.reason -eq 'no_derivations' -and $plainDone.explanation.applicable -eq $true) 'a formal model with no Derivation part records an explicit no-derivation no-op'
    Assert (-not $plainDone.explain_unit) 'a no-derivation model registers no explanation obligation'

    # A concept model records the layer as inapplicable.
    $conceptSlug = $concept -replace '[\\/|]', '_'
    $conceptWork = Join-Path $out "models\m-$conceptSlug"
    $conceptDir = Join-Path $conceptWork $conceptSlug
    $conceptStep = { param($s, [string[]]$more = @()) Run-Digest (@('model', '-Step', $s, '-Work', $conceptWork, '-Staging', $out) + $more) }
    $null = & $conceptStep 'brief' @('-Models', $concept)
    $conceptPlan = [ordered]@{
        model = $concept; level = 'concept'; card = '问题、机制、结论'
        parts = @(
            [ordered]@{ id = 'C1'; line = 'main'; facet = '机制'; title = '机制：抵押提高还款承诺'; spec = '机制'; src = @('sections/s01.md:2') }
        )
    }
    Save-Json (Join-Path $conceptDir 'plan.json') $conceptPlan
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
    $conceptClaim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'models', '-Take', '1', '-Owner', 'model-manager')).units | Where-Object { $_.inputs.header -eq $concept } | Select-Object -First 1
    $conceptDone = Run-Digest @('model-finish', '-Out', $out, '-Model', $concept, '-Token', "$($conceptClaim.token)")
    Assert ($conceptDone.complete -and $conceptDone.explanation.status -eq 'none' -and $conceptDone.explanation.reason -eq 'concept' -and $conceptDone.explanation.applicable -eq $false) 'a concept model records the explanation layer as inapplicable'
    Assert (-not $conceptDone.explain_unit) 'an inapplicable explanation layer registers no obligation'

    # R302: the build completion and the explanation completion stay separately visible. The
    # Models layer settles on its built units while the Explain layer still names its open work.
    $modelsDone = Gate 'models' '"layer":"models"' 0
    Assert ($modelsDone.sealed -and $modelsDone.settled -and -not $modelsDone.complete -and $modelsDone.counts.done -eq 3) 'the Models layer settles on the three checked builds without claiming the run complete'
    $open = Gate 'explain' '"layer":"explain"' 3
    Assert ($open.sealed -and -not $open.settled -and $open.counts.open -eq 1 -and @($open.unresolved) -contains $built.explain_unit) 'the Explain layer is sealed and still open on the unexplained parts'
    $receiptsBefore = Receipts (Join-Path $scratch 'archive\zt.sqlite3')

    # The completion is the checked model, not the agent's word: the three Derivation parts still
    # lack their sections, so the Explain unit cannot complete and names the exact work left.
    $explainClaim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'explain', '-Take', '1', '-Owner', 'explain-manager')).units[0]
    Assert ($explainClaim.id -eq $built.explain_unit -and $explainClaim.inputs.model -eq $formal) 'the Explain layer hands out the registered explanation unit'
    $unit = $explainClaim.id; $token = $explainClaim.token
    $finish = @('explain-finish', '-Out', $out, '-Model', $formal, '-Token', "$token")
    Refused $finish 'explanation_missing' 'unexplained Derivation parts keep the Explain unit open'
    Refused $finish $cap 'the refusal names the exact unexplained part'
    Refused $finish '3 of 3' 'the refusal counts every in-scope part of the checked model'
    $still = @((Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'explain')).units | Where-Object { $_.id -eq $unit })[0]
    Assert ($still.state -eq 'claimed') 'a refused completion leaves the Explanation unit claimed'

    # A model repair leaves the explanation work in place: resume reports a complete build with
    # no invented job, and the Explain obligation still covers the checked model (T24).
    $resume = & $step 'resume' @('-Models', $formal)
    Assert (@($resume.jobs).Count -eq 0 -and $resume.models[0].complete) 'a resume of the completed build starts no job'
    Assert (@($explainRow.inputs.parts).Count -eq 3) 'the explanation obligation survives a model repair pass'

    # The real compiler writes the explanations: one job per chain, only the parts that still
    # lack 思路 or 解读, and it keeps 起点, every step and 结果 exactly as they were.
    $xstep = { param($s, [string[]]$more = @()) Run-Digest (@('model', '-Step', $s, '-Work', $work, '-Staging', $out) + $more) }
    $planned = & $xstep 'explain' @('-Models', $formal)
    Assert ($planned.parts -eq 3 -and @($planned.jobs).Count -eq 1) 'the explain plan names the three unexplained Derivation parts in one chain job'
    $xstate = State $work
    $xjob = $xstate.explain.jobs[$planned.jobs[0].id]
    Assert (@($xjob.addresses).Count -eq 3 -and @($xjob.addresses) -contains $cap -and @($xjob.addresses) -contains $net) 'the explain job covers exactly the unexplained chain'
    $positions = $xjob.positions
    Assert ($positions[$cap].StartsWith('第 1 张，共 3 张') -and $positions[$condition].StartsWith('第 2 张，共 3 张') -and $positions[$net].StartsWith('第 3 张，共 3 张')) 'the job carries the compiler position line of every chain member'
    $answer = @(
        @{ address = $cap; idea = '先求还款额的上限，再看项目价值。'; notes = @{}; meaning = '还款受清算价值限制，清算价值越高越容易融资。' },
        @{ address = $condition; idea = '把上限代入参与约束，得到融资条件。'; notes = @{ '1' = '项目的收益按概率加权。' }; meaning = '项目价值超过成本时投资者参与。' },
        @{ address = $net; idea = '把还款上限代入财富条件。'; notes = @{}; meaning = '企业家自己的财富也要够，净资产为正才能融资。' }
    )
    Save-Json $xjob.out $answer
    $selfCheck = @(& $Digest @('model', '-Step', 'explain-check', '-Work', $work, '-Staging', $out, '-Job', $planned.jobs[0].id) 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($selfCheck -join ' ').Contains('faults: 0') -and ($selfCheck -join ' ').Contains("$net ok")) "the writer's own check passes on every part [$($selfCheck -join ' | ')]"
    $applied = & $xstep 'explain-apply'
    Assert ($applied.applied -eq 3 -and @($applied.refused).Count -eq 0 -and @($applied.blocked).Count -eq 0) 'explain-apply writes the three accepted parts and nothing else'
    $capCard = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $cap } })
    Assert ($capCard.body.Contains($positions[$cap]) -and $capCard.body.Contains('**起点**') -and $capCard.body.Contains('**解读**') -and -not $capCard.body.Contains('待写')) 'the written part carries 思路 with the compiler position line, its 起点 and its 解读'
    Assert ($capCard.body.Contains('**结果**') -and $capCard.body.Contains('P\le L')) 'the explanation leaves the derived result unchanged'
    $xreport = & $xstep 'explain-report'
    Assert ($xreport.models[0].derivations -eq 3 -and $xreport.models[0].explained -eq 3 -and @($xreport.models[0].missing).Count -eq 0) 'the explain report counts three explained Derivation parts of the model'

    # T17: one part loses its explanation (a split or a repair can leave it unwritten). The plan
    # names only that part, explain-finish names only that part, and the explained neighbours keep
    # their authored fingerprints.
    $conditionCard = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $condition } })
    $capBody = Body $cap; $netBody = Body $net
    $wiped = '**思路**' + "`n待写`n`n" + ($conditionCard.body -replace '(?s)^.*?(\*\*起点\*\*)', '$1')
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $condition; body = $wiped } }) | Out-Null
    $second = & $xstep 'explain' @('-Models', $formal)
    Assert ($second.parts -eq 1 -and @($second.jobs).Count -eq 1) 'the second plan names only the one unfinished Derivation part'
    $secondJob = (State $work).explain.jobs[$second.jobs[0].id]
    Assert (@($secondJob.addresses).Count -eq 1 -and $secondJob.addresses[0] -eq $condition) 'the resumed job covers exactly the unfinished part'
    Assert ((Body $cap) -eq $capBody -and (Body $net) -eq $netBody) 'the explained neighbours keep their authored text'
    $partialLines = @(& $Digest $finish 2>&1)
    $partial = ($partialLines -join ' ')
    Assert ($LASTEXITCODE -ne 0 -and $partial.Contains('1 of 3')) 'only the unfinished part keeps the Explain unit open'
    Assert (-not $partial.Contains($cap) -and -not $partial.Contains($net) -and $partial.Contains($condition)) 'the resumed refusal names only the unfinished part'
    Save-Json $secondJob.out @(@{ address = $condition; idea = '把上限代入参与约束，得到融资条件。'; notes = @{}; meaning = '项目价值超过成本时投资者参与。' })
    $null = & $xstep 'explain-apply'
    $conditionWritten = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $condition } })
    Assert ($conditionWritten.body.Contains($positions[$condition]) -and $conditionWritten.body.Contains('**解读**')) 'the resumed apply restores the explanation and its chain position line'
    Assert ((Body $cap) -eq $capBody -and (Body $net) -eq $netBody) 'the resumed apply leaves the explained neighbours unchanged'
    $xreport2 = & $xstep 'explain-report'
    Assert ($xreport2.models[0].explained -eq 3 -and @($xreport2.models[0].missing).Count -eq 0) 'the report counts the whole chain explained again'

    # The completed neighbouring model and the group receipts do not change because another model
    # gained its explanations.
    $plainAgain = Run-Digest @('model-finish', '-Out', $out, '-Model', $plain, '-Token', "$($plainClaim.token)")
    Assert ($plainAgain.reused -and $plainAgain.card_fingerprints[$plainIds['S']] -eq $plainDone.card_fingerprints[$plainIds['S']]) 'a completed neighbouring model keeps its build fingerprints and receipts'
    Assert ((Receipts (Join-Path $scratch 'archive\zt.sqlite3')) -eq $receiptsBefore) 'the explanation work adds no archive receipt of its own'

    # A later change of a part's 思路 makes the chain structure stale, not silently accepted.
    $netCard = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $net } })
    $staleLine = $netCard.body -replace [regex]::Escape($positions[$net]), '第 3 张，共 9 张：旧的。'
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $net; body = $staleLine } }) | Out-Null
    Refused $finish 'explanation_chain' 'a wrong chain position line keeps the Explain unit open'
    Assert ((& $xstep 'explain' @('-Models', $formal)).parts -eq 1) 'a stale chain position line is the explain plan''s own repair work'
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $net; body = $netCard.body } }) | Out-Null

    # The preview checks the whole obligation and writes nothing.
    $checkOnly = Run-Digest ($finish + @('-CheckOnly'))
    Assert ($checkOnly.check_only -and -not $checkOnly.applied -and $checkOnly.kind -eq 'explain_completion') 'the preview checks the explanation without writing'
    Assert ($checkOnly.explanation.status -eq 'explained' -and $checkOnly.explanation.derivations -eq 3 -and $checkOnly.explanation.explained -eq 3) 'the preview records all three checked Derivation parts'
    Assert (@($checkOnly.explanation.parts).Count -eq 3 -and @($checkOnly.explanation.chains).Count -eq 1 -and @($checkOnly.explanation.chains[0].parts).Count -eq 3) 'the completion enumerates the actual parts and the chain of the checked model'
    Assert (@($checkOnly.explanation.positions).Count -eq 3 -and @($checkOnly.card_fingerprints.Keys).Count -eq 3 -and $checkOnly.explanation.check.count -eq 0) 'the completion retains the chain positions, the changed fingerprints and a clean explanation check'
    Assert ($checkOnly.build.unit -eq $built.unit -and $checkOnly.build.complete -and $checkOnly.build.parts -eq 6 -and $checkOnly.build.plan_hash -eq $built.plan_hash) 'the build completion stays a separate recorded fact beside the explanation completion'
    $key2 = $unit -replace '^explain/', '' -replace '@1$', ''
    Assert (-not (Test-Path (Join-Path $out "explain-completion-$key2.json"))) 'the preview publishes no evidence'

    # A lost ledger commit leaves the unit claimed; the retry recovers the same completion.
    $env:ZT_LEDGER_COMPLETE_FAULT = 'after_outputs'
    Refused $finish 'injected completion fault' 'an injected fault rolls the Explain completion back after its evidence is written'
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    Assert (@((Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'explain')).units | Where-Object { $_.id -eq $unit })[0].state -eq 'claimed') 'the injected fault leaves the Explain unit claimed'
    $explainDone = Run-Digest $finish
    Assert ($explainDone.schema_version -eq 1 -and $explainDone.kind -eq 'explain_completion' -and $explainDone.unit -eq $unit -and $explainDone.state -eq 'done' -and $explainDone.complete -and -not $explainDone.reused) 'the real explanation completion records typed evidence'
    Assert ($explainDone.explanation.status -eq 'explained' -and $explainDone.explanation.derivations -eq 3 -and @($explainDone.explanation.needs).Count -eq 0) 'the completion records every checked Derivation part'
    Assert (@($explainDone.publication_inputs.layers) -contains 'link-check' -and $explainDone.publication_inputs.fingerprints[$cap] -and $explainDone.publication_inputs.fingerprints[$net]) 'the completion emits the changed fingerprints and the downstream publication inputs'
    $explainGate = Gate 'explain' '"layer":"explain"' 0
    Assert ($explainGate.sealed -and $explainGate.settled -and -not $explainGate.complete -and $explainGate.counts.done -eq 1) 'the Explain layer settles on its checked completion without claiming the run complete'

    # The same evidence through the generic ledger command, and a repeated completion.
    $completionFile = Join-Path $out "explain-completion-$key2.json"
    Assert ((Test-Path $completionFile) -and $explainDone.files.completion.sha256 -and $explainDone.files.check.sha256) 'the completion and check evidence are retained and hash-bound'
    $again = Run-Digest @('ledger', 'done', '-Out', $out, '-Unit', $unit, '-Token', "$token", '-Outputs', $completionFile)
    Assert ($again.reused -and $again.unit -eq $unit -and $again.complete) 'ledger done dispatches the same Explain verifier and reuses the checked result'
    Refused @('ledger', 'done', '-Out', $out, '-Unit', $unit, '-Token', "$token", '-Outputs', (Join-Path $out "model-completion-$key.json")) 'invalid Explain completion evidence' 'another layer evidence cannot complete an Explain unit'
    Refused @('explain-finish', '-Out', $out, '-Model', $formal, '-Token', "$($token + 1)") 'stale_claim' 'a wrong claim token cannot complete the Explain unit'
    Refused @('explain-finish', '-Out', $out, '-Model', 'ExplainBook/1|z', '-Token', "$token") 'no registered Explain unit' 'an unknown model has no explanation obligation'
    # A changed explained part makes the retained completion stale instead of describing other Cards.
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $cap; body = "$capBody`n`n后来的一条独立补充。" } }) | Out-Null
    Refused $finish 'completed Explain unit changed' 'a later edit of an explained part makes the Explain completion stale'
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $cap; body = $capBody } }) | Out-Null
    Assert ((Run-Digest $finish).reused) 'restoring the checked part recovers the same Explain completion'

    # A book whose models are all inapplicable leaves the layer empty: it settles only after the
    # Models enumeration closes, so a concept-only book never blocks on explanation work.
    $emptyOut = Join-Path $scratch 'prepared-empty'
    $emptySource = Join-Path $scratch 'empty.md'
    [IO.File]::WriteAllText($emptySource, "# 第一章`n`n抵押提高还款承诺，投资者愿意放款。`n`n# 参考`n未选择。", $utf8)
    $null = Run-Digest @('prepare', '-Source', $emptySource, '-Out', $emptyOut, '-CitationKey', 'EmptyBook', '-BibTeX', '@book{EmptyBook,title={Empty book}}', '-StartHeading', '第一章', '-EndHeading', '参考', '-MinUnitChars', '1', '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')
    $emptyClaim = (Run-Digest @('ledger', 'next', '-Out', $emptyOut, '-Layer', 'digest', '-Take', '1', '-Owner', 'digest-manager')).units[0]
    $emptyStarted = Run-Digest @('group-start', '-Out', $emptyOut, '-Group', '1', '-Token', "$($emptyClaim.token)")
    $emptyJob = Get-Content -LiteralPath $emptyStarted.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $emptyDraft = @{ schema_version = 1; unit = $emptyClaim.id; token = $emptyClaim.token; input_hash = $emptyStarted.input_hash; disposition = 'cards'; calls = @(
        @{ tool = 'edit_card'; arguments = @{ at = 'EmptyBook/1'; title = '抵押与还款承诺'; body = '抵押提高还款承诺。投资者因此愿意放款。'; base_fingerprint = $emptyJob.inputs.anchor.fingerprint } },
        @{ tool = 'create_successor'; arguments = @{ at = 'EmptyBook/1'; kind = 'side'; type = 'model'; level = 'concept'; title = '概念模型：抵押的作用'; body = '问题：抵押怎样帮助融资？' } }
    ) }
    Save-Json $emptyStarted.draft $emptyDraft
    $emptyGroup = Run-Digest @('group-finish', '-Out', $emptyOut, '-Group', '1', '-Token', "$($emptyClaim.token)", '-Draft', $emptyStarted.draft)
    $emptyHead = @($emptyGroup.owned | Where-Object { $_ -match '\|a$' })[0]
    $emptySlug = $emptyHead -replace '[\\/|]', '_'
    $emptyWork = Join-Path $emptyOut "models\m-$emptySlug"
    $emptyDir = Join-Path $emptyWork $emptySlug
    $emptyStep = { param($s, [string[]]$more = @()) Run-Digest (@('model', '-Step', $s, '-Work', $emptyWork, '-Staging', $emptyOut) + $more) }
    $null = & $emptyStep 'brief' @('-Models', $emptyHead)
    Save-Json (Join-Path $emptyDir 'plan.json') ([ordered]@{ model = $emptyHead; level = 'concept'; card = '问题、机制、结论'; parts = @(
        [ordered]@{ id = 'C1'; line = 'main'; facet = '机制'; title = '机制：抵押提高还款承诺'; spec = '机制'; src = @('sections/s01.md:2') }
    ) })
    $null = & $emptyStep 'apply'
    $emptyIds = (State $emptyWork).models[$emptyHead].ids
    $emptyHeadCard = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $emptyHead } })
    $emptyStructure = ([regex]'(?s)\*\*结构\*\*.*$').Match($emptyHeadCard.body).Value
    Invoke-Zt @(
        @{ tool = 'edit_card'; arguments = @{ at = $emptyHead; body = "问题：抵押怎样帮助融资？机制：抵押提高还款承诺。结论：有抵押更容易融资。`n`n来源：第一章。`n`n$emptyStructure" } },
        @{ tool = 'edit_card'; arguments = @{ at = $emptyIds['C1']; body = "抵押提高还款承诺，投资者愿意放款。`n`n来源：第一章。" } }
    ) | Out-Null
    $null = & $emptyStep 'report'
    $emptyBefore = @(& $Digest @('ledger', 'gate', '-Out', $emptyOut, '-Layer', 'explain') 2>&1)
    $emptyBeforeJson = $emptyBefore[-1] | ConvertFrom-Json -AsHashtable
    Assert ($LASTEXITCODE -eq 3 -and -not $emptyBeforeJson.sealed -and $emptyBeforeJson.counts.total -eq 0) 'an empty Explain layer stays unsealed while a Model unit can still register work'
    $emptyModelClaim = (Run-Digest @('ledger', 'next', '-Out', $emptyOut, '-Layer', 'models', '-Take', '1', '-Owner', 'model-manager')).units[0]
    $emptyDone = Run-Digest @('model-finish', '-Out', $emptyOut, '-Model', $emptyHead, '-Token', "$($emptyModelClaim.token)")
    Assert ($emptyDone.explanation.reason -eq 'concept' -and -not $emptyDone.explain_unit) 'a concept-only book registers no explanation unit'
    $emptyGate = @(& $Digest @('ledger', 'gate', '-Out', $emptyOut, '-Layer', 'explain') 2>&1)
    $emptyGateJson = $emptyGate[-1] | ConvertFrom-Json -AsHashtable
    Assert ($LASTEXITCODE -eq 0 -and $emptyGateJson.sealed -and $emptyGateJson.settled -and $emptyGateJson.counts.total -eq 0) 'an empty Explain layer settles once its Models enumeration closes'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
