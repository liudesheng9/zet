#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\model-compiler.tests.ps1
# Drives model-compiler.ps1 through brief, validate, apply, derive, review, split, and report on a
# scratch archive (ZT_CONFIG_DIR, its own service), standing in for Sonnet's plan and DeepSeek's
# writing. The live archive is not touched.
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "model-compiler-test-$PID"
$archive = Join-Path $scratch 'archive'
$staging = Join-Path $scratch 'staging'
$work = Join-Path $scratch 'work'
New-Item -ItemType Directory -Force $archive, (Join-Path $staging 'sections'), $work | Out-Null
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$compiler = Join-Path $PSScriptRoot '..\model-compiler.ps1' | Resolve-Path | ForEach-Object Path
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Run($step, [string[]]$more = @()) {
    $lines = @(pwsh -NoProfile -File $compiler -Step $step -Work $work -Staging $staging @more 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "$step failed: $($lines -join "`n")" }
    $lines
}
function Last($lines) { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable }
function Write-Calls($calls) {
    $out = ($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -
    if ($LASTEXITCODE -ne 0) { throw "zt: $out" }
}
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $model = 'Test2020/1|a'
    Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{Test2020, title={Firms}}' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2020'; title = '第一章：融资' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2020/1'; kind = 'side'; type = 'model'; level = 'formal'; title = '融资模型：清算价值决定能否融资'; body = "问题：企业能否融到 $K-w$？`n`n来源：第一章，(5.5)–(5.10)。" } },
        # another, finished model whose Derivation part the plan takes in: the batch never splits it
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2020/1'; kind = 'side'; type = 'model'; level = 'formal'; title = '还款模型：还款不超过清算价值'; body = '问题：企业能还多少？' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2020/1|b'; kind = 'side'; type = 'model'; facet = '推导：别的模型的还款上限'; emphasis = 'aside'; title = '推导：还款上限'; body = "**起点**`n- 从 (5.5) 出发。`n`n**步骤**`n1. a`n`n**结果**`n`$`$P\le L`$`$" } }
    )
    $shared = 'Test2020/1|b|a'
    [IO.File]::WriteAllLines((Join-Path $staging 'sections\s01.md'), [string[]]@('# 第一章', '企业家有财富 $w$，项目需要 $K$。', '清算价值为 $L$。', '式 (5.5)：$P\le L$。', '于是得到 (5.10)。'), $utf8)

    # brief
    $brief = Last (Run 'brief' @('-Models', $model))
    Assert ($brief.jobs.Count -eq 1 -and (Test-Path -LiteralPath $brief.jobs[0].files[1])) 'brief: one job with its job file'
    $dir = Join-Path $work 'Test2020_1_a'
    Assert (Test-Path -LiteralPath (Join-Path $dir 'context.md')) 'brief: context pack written'

    # a plan with a forward premise fails validation
    $plan = [ordered]@{
        model = $model; level = 'formal'; card = '问题、机制、结论'
        symbols = @(, @('$L$', '清算价值')); terms = @(, @('清算', '卖资产', '变卖资产'))
        parts = @(
            [ordered]@{ id = 'S'; line = 'main'; facet = '设定'; emphasis = 'core'; title = '设定：企业家、项目与清算价值'; spec = '参与者与时点'; src = @('sections/s01.md:2-3') },
            [ordered]@{ id = 'A'; line = 'main'; facet = '假设'; title = '假设：投资者只认清算价值'; spec = '假设'; rests = @(, @('P', '用到命题')) },
            [ordered]@{ id = 'D1'; line = 'main'; derive = $true; facet = '推导：还款上限'; title = '推导：还款不超过清算价值'; from = '(5.5)'; to = '$$P\le L$$'; steps = 4; rests = @(, @('S', '用到 $L$')) },
            [ordered]@{ id = 'D2'; line = 'main'; derive = $true; facet = '推导：融资条件'; title = '推导：得到融资条件'; to = '$$L\ge K-w \quad (5.10)$$'; steps = 5 },
            [ordered]@{ id = 'P'; line = 'main'; facet = '命题'; emphasis = 'core'; title = '命题：清算价值够大才能融资'; spec = '命题'; rests = @(, @('D2', '用到 (5.10)')) },
            [ordered]@{ id = 'E'; line = 'side'; of = 'P'; facet = '数值例子'; emphasis = 'aside'; title = '例子：L=6 时能融资'; spec = '代入数字'; chart = '画出 L 与 K-w 的比较' },
            [ordered]@{ id = 'H'; line = 'off'; facet = '历史'; emphasis = 'aside'; title = '历史：抵押融资的来历'; spec = '历史' },
            [ordered]@{ id = 'T'; take = $shared; use = '还款上限的另一种推导' }
        )
    }
    $planFile = Join-Path $dir 'plan.json'
    $plan.Remove('card')
    [IO.File]::WriteAllText($planFile, ($plan | ConvertTo-Json -Depth 8), $utf8)
    $v = Run 'validate' @('-Plan', $planFile)
    Assert ($v -match '^card is missing') 'validate: a plan without card is a fault'
    $plan.card = '问题、机制、结论'
    [IO.File]::WriteAllText($planFile, ($plan | ConvertTo-Json -Depth 8), $utf8)
    $v = Run 'validate' @('-Plan', $planFile)
    Assert ($v -match '^forward: A rests on P') 'validate: a forward premise is a fault'
    $plan.parts[1].rests = @(, @('S', '用到参与者'))
    [IO.File]::WriteAllText($planFile, ($plan | ConvertTo-Json -Depth 8), $utf8)
    $v = Run 'validate' @('-Plan', $planFile)
    Assert (@($v)[-1] -eq 'faults: 0') "validate: the fixed plan has no faults ($($v -join ' / '))"

    # apply: skeleton in build order, 结构 list, fill jobs
    $apply = Last (Run 'apply')
    Assert ($apply.models[0].status -eq 'built') "apply: model built ($($apply.models[0].error))"
    Assert ($apply.jobs.Count -eq 3) "apply: three fill jobs (Model Card + 4 parts in 2 jobs), got $($apply.jobs.Count)"
    $s = Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $model } })
    $main = @($s.build.order | Where-Object { $_.line -eq 'main' } | ForEach-Object { $_.facet })
    Assert (($main -join ',') -eq '设定,假设,推导：还款上限,推导：融资条件,命题') "apply: main line in build order ($($main -join ','))"
    Assert (@($s.build.forward).Count -eq 0 -and @($s.build.unanchored).Count -eq 0 -and @($s.build.closure).Count -eq 0) 'apply: no build faults in the skeleton'
    $head = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })
    Assert ($head.body -match '\*\*结构\*\*' -and $head.body -match '来源') 'apply: Model Card has a 结构 list and keeps 来源'
    $setting = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $s.build.setting } })
    Assert ($setting.body -match '术语表' -and $setting.body -match '符号表') 'apply: setting skeleton has both tables'
    $jobText = Get-Content -LiteralPath $apply.jobs[1].files[2] -Raw -Encoding utf8
    Assert ($jobText -match '清算价值为' -and $jobText -match 'ALLOW:') 'apply: job file carries the source lines and its allow file'
    $cardJob = Get-Content -LiteralPath $apply.jobs[0].files[2] -Raw -Encoding utf8
    Assert ($cardJob -match '模型卡现在的正文' -and $cardJob -match '企业能否融到' -and $cardJob -match '内容：问题、机制、结论') 'apply: the Model Card job carries the plan''s card and the current body'

    # stand-in for the fill round
    $state = Get-Content -LiteralPath (Join-Path $work 'state.json') -Raw | ConvertFrom-Json -AsHashtable
    $ids = $state.models[$model].ids
    Write-Calls @($ids.Keys | Where-Object { $_ -notin 'D1', 'D2' } | ForEach-Object { @{ tool = 'edit_card'; arguments = @{ at = $ids[$_]; body = "已写。依赖 [[$($s.build.setting)]]：用到设定。" } } })

    # derive: one job with the chain's two parts and the filled premises
    $derive = Last (Run 'derive')
    Assert ($derive.jobs.Count -eq 1) "derive: one job for the chain, got $($derive.jobs.Count)"
    $deriveText = Get-Content -LiteralPath $derive.jobs[0].files[2] -Raw -Encoding utf8
    Assert ($deriveText -match '前提卡的正文') 'derive: the job copies the premises in'
    Assert ($deriveText -match '起点：上一步 \[\[[^\]]+\]\] 的结果 \$\$P\\le L\$\$') 'derive: a chain member''s block names the result it starts from'
    $d1 = $ids['D1']; $d2 = $ids['D2']
    Write-Calls @(
        @{ tool = 'edit_card'; arguments = @{ at = $d1; body = "**起点**`n- 依赖 [[$($s.build.setting)]]：用到 L。`n`n**步骤**`n1. a`n2. b`n`n**结果**`n`$`$P\le L`$`$" } },
        @{ tool = 'edit_card'; arguments = @{ at = $d2; body = "**起点**`n- 依赖上一步 [[$d1]] 的结果。`n`n**步骤**`n1. c`n`n**结果**`n`$`$L\ge K-w`$`$" } }
    )

    # review, then a verdict that splits D1 into three
    $review = Last (Run 'review')
    Assert ($review.jobs.Count -eq 1) "review: one job per chain of the model's own parts (not the shared one), got $($review.jobs.Count)"
    Assert (-not ((Get-Content -LiteralPath $review.jobs[0].files[2] -Raw -Encoding utf8).Contains("[[$shared]]"))) 'review: the shared Derivation part is not reviewed'
    $out = ([string]$review.jobs[0].prompt) -replace '^.*写进 (\S+?)，.*$', '$1'
    $verdict = @(
        [ordered]@{ address = $d1; verdict = 'split'; parts = 3; cuts = @('$$P=B$$', '$$B\le L$$'); reason = '第 2 步跳了两步' },
        [ordered]@{ address = $shared; verdict = 'split'; parts = 2; cuts = @('$$P=B$$'); reason = '别的模型的卡' }
    )
    [IO.File]::WriteAllText($out, (ConvertTo-Json -InputObject $verdict -Depth 5), $utf8)
    $split = Last (Run 'split' @('-Need', "$($ids['A'])=2"))
    Assert ($shared -in @($split.orphans)) "split: a shared part of another model is never split (orphans: $($split.orphans -join ', '))"
    $other = Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = 'Test2020/1|b' } })
    Assert (@($other.parts).Count -eq 1) 'split: the other model keeps its one part'
    $s = Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $model } })
    $main = @($s.build.order | Where-Object { $_.line -eq 'main' })
    Assert ($main.Count -eq 8) "split: main line grows to 8 parts, got $($main.Count)"
    $chain = @($s.build.chains | Sort-Object { $_.length } -Descending | Select-Object -First 1)
    Assert ($chain[0].length -eq 4) "split: one Derivation chain of four parts, got $($chain[0].length)"
    Assert (@($s.build.forward).Count -eq 0 -and @($s.build.unanchored).Count -eq 0) 'split: no build faults'
    $state = Get-Content -LiteralPath (Join-Path $work 'state.json') -Raw | ConvertFrom-Json -AsHashtable
    $ids = $state.models[$model].ids
    $d2card = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ids['D2'] } })
    Assert ($d2card.body.Contains("[[$($ids['D1~3'])]]")) 'split: the next part rests on the last new part'
    $d1card = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ids['D1'] } })
    Assert ($d1card.body -match 'P=B') 'split: the split part ends at the first cut'
    $head = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })
    Assert ($head.body.Contains("[[$($ids['D1~2'])]]") -and $head.body.Contains("[[$($ids['A~2'])]]")) 'split: the 结构 list names the new parts'
    Assert (@($split.jobs | Where-Object { $_.continue }).Count -eq 2) "split: follow-ups continue the two conversations, got $($split.jobs.Count) jobs"
    $allow = Get-Content -LiteralPath $state.jobs[$split.jobs[-1].continue].allow -Encoding utf8
    Assert ($allow.Count -ge 2) 'split: the allow file lists the new parts'

    # re-review only the changed chain; report
    $again = Last (Run 'review' @('-Changed'))
    Assert ($again.jobs.Count -eq 1) "review -Changed: the changed chain only, got $($again.jobs.Count)"
    $none = Last (Run 'review' @('-Changed'))
    Assert (@($none.jobs).Count -eq 0) "review -Changed: nothing changed since the last review, no jobs (got $(@($none.jobs).Count))"
    $report = Last (Run 'report')
    $r = $report.models[0]
    Assert ($r.status -eq 'done' -and $r.derivations.total -eq 4 -and $r.derivations.longest -eq 4 -and $r.split -eq 2) "report: 4 Derivation parts, longest chain 4, 2 splits ($($r | ConvertTo-Json -Compress -Depth 3))"
    Assert ($r.todo.Count -ge 3) 'report: lists the parts still 待写'
} catch {
    Write-Output "FAIL: $_"
    Write-Output $_.ScriptStackTrace
    $failed++
} finally {
    zt down 2>$null | Out-Null
    Remove-Item Env:ZT_CONFIG_DIR
}
if ($failed) { Write-Output "$failed failed; scratch kept at $scratch"; exit 1 }
Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
Write-Output 'all passed'
