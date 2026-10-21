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
            [ordered]@{ id = 'A'; line = 'main'; facet = '假设'; title = '假设：投资者只认清算价值'; spec = '假设'; rests = @(, @('P', '用到命题')); src = @('sections/s01.md:2-2') },
            [ordered]@{ id = 'D1'; line = 'main'; derive = $true; facet = '推导：还款上限'; title = '推导：还款不超过清算价值'; from = '(5.5)'; to = '$$P\le L$$'; steps = 4; rests = @(, @('S', '用到 $L$')); src = @('sections/s01.md:4-4') },
            [ordered]@{ id = 'D2'; line = 'main'; derive = $true; facet = '推导：融资条件'; title = '推导：得到融资条件'; to = '$$L\ge K-w \quad (5.10)$$'; steps = 5; src = @('sections/s01.md:5-5') },
            [ordered]@{ id = 'P'; line = 'main'; facet = '命题'; emphasis = 'core'; title = '命题：清算价值够大才能融资'; spec = '命题'; rests = @(, @('D2', '用到 (5.10)')); src = @('sections/s01.md:5-5') },
            [ordered]@{ id = 'E'; line = 'side'; of = 'P'; facet = '数值例子'; emphasis = 'aside'; title = '例子：L=6 时能融资'; spec = '代入数字'; chart = '画出 L 与 K-w 的比较'; src = @('sections/s01.md:3-3') },
            [ordered]@{ id = 'H'; line = 'off'; facet = '历史'; emphasis = 'aside'; title = '历史：抵押融资的来历'; spec = '历史'; src = @('sections/s01.md:1-1') },
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

    # The architect validates without -Staging, from the workflow's <staging>\models\<batch> folder: a book
    # figure listed in <staging>\figures.json is found there too.
    [IO.File]::WriteAllText((Join-Path $staging 'figures.json'), (@{ figures = @(@{ labels = @('2.F.1'); line = '![图 2.F.1](scholar:f1)' }) } | ConvertTo-Json -Depth 5), $utf8)
    $batch = Join-Path $staging 'models\b1'
    New-Item -ItemType Directory -Force $batch | Out-Null
    $figurePlan = $plan | ConvertTo-Json -Depth 8 | ConvertFrom-Json -AsHashtable
    $figurePlan.parts[5].figure = @('2.F.1')
    $figurePlanFile = Join-Path $batch 'plan.json'
    [IO.File]::WriteAllText($figurePlanFile, ($figurePlan | ConvertTo-Json -Depth 8), $utf8)
    $v = @(pwsh -NoProfile -File $compiler -Step validate -Work $batch -Plan $figurePlanFile 2>&1)
    Assert (@($v)[-1] -eq 'faults: 0') "validate: without -Staging a book figure of <staging>\figures.json is found ($($v -join ' / '))"
    $figurePlan.parts[5].figure = @('9.F.9')
    [IO.File]::WriteAllText($figurePlanFile, ($figurePlan | ConvertTo-Json -Depth 8), $utf8)
    $v = @(pwsh -NoProfile -File $compiler -Step validate -Work $batch -Plan $figurePlanFile 2>&1)
    Assert ($v -match "figure '9.F.9' is not a book figure") "validate: a figure the book does not hold is a fault ($($v -join ' / '))"

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
    # The skeleton of a Derivation part has five sections; a chain member's 思路 opens with its position.
    $d1skel = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $d1 } })).body
    $d2skel = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $d2 } })).body
    Assert ($d1skel -match '(?s)^\*\*思路\*\*\n第 1 张，共 2 张。全链依次推出 \$P\\le L\$、\$L\\ge K-w \\quad \(5\.10\)\$。\n待写.*\*\*起点\*\*.*\*\*步骤\*\*.*\*\*结果\*\*.*\*\*解读\*\*\n待写$') "apply: a Derivation skeleton has 思路 with the chain route, and 解读 ($d1skel)"
    Assert ($d2skel -match '第 2 张，共 2 张：从 \$P\\le L\$ 推到 \$L\\ge K-w \\quad \(5\.10\)\$。') "apply: the second member's 思路 names where it starts and ends ($d2skel)"
    Assert ($deriveText -match '讲清推导') 'derive: the job asks for the explanation'
    Write-Calls @(
        @{ tool = 'edit_card'; arguments = @{ at = $d1; body = "**思路**`n第 1 张，共 2 张。全链依次推出 `$P\le L`$、`$L\ge K-w`$。`n先求还款，再比较。`n`n**起点**`n- 依赖 [[$($s.build.setting)]]：用到 L。`n`n**步骤**`n1. a`n   说明：这一步取上界。`n2. b`n`n**结果**`n`$`$P\le L`$`$`n`n**解读**`n还款不超过清算价值。" } },
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
    Assert ($d1card.body -match '(?s)\*\*结果\*\*\n\$\$P=B\$\$\n\n\*\*解读\*\*\n待写$' -and $d1card.body -match '说明：这一步取上界') "split: the cut keeps the steps and their notes, and clears 解读 ($($d1card.body))"
    Assert ($d1card.body -match '^\*\*思路\*\*\n第 1 张，共 4 张。全链依次推出 \$P=B\$、\$B\\le L\$、\$P\\le L\$、\$L\\ge K-w\$。\n先求还款') "split: the first member's position line names the new chain ($($d1card.body))"
    $d12 = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ids['D1~2'] } })
    Assert ($d12.body -match '^\*\*思路\*\*\n第 2 张，共 4 张：从 \$P=B\$ 推到 \$B\\le L\$。\n待写' -and $d12.body -match '\*\*解读\*\*\n待写$') "split: a new part has 思路 with its position, and 解读 ($($d12.body))"
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
    # The relayed line carries counts; the part list is in report.json (trial 12: a relay cut the lists).
    Assert ($r.parts -is [long] -or $r.parts -is [int]) "report: the line gives the part count, not the list ($($r.parts))"
    $full = Get-Content -LiteralPath $report.file -Raw -Encoding utf8 | ConvertFrom-Json
    Assert (@($full.models[0].parts).Count -eq $r.parts -and $full.models[0].parts[0].facet) "report: report.json holds every part with its facet"
    Assert ($r.derivations.explained -eq 0) "report: no Derivation part has both 思路 and 解读 yet ($($r.derivations.explained))"

    # close: a Model Card that rests on an Idea Card outside the model (closure) and a part whose sentence
    # rests on a later part (forward) go back, as 修正 notes, to the conversations that wrote them
    # (grossman2002integration trial 6: model-finish refused all four models for these faults).
    $state = Get-Content -LiteralPath (Join-Path $work 'state.json') -Raw | ConvertFrom-Json -AsHashtable
    $ids = $state.models[$model].ids
    $null = Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{ at = 'Test2020/1'; kind = 'side'; title = '外部条件：市场利率固定' } })
    $outside = 'Test2020/1|c'
    Assert ((Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $outside } })).title -match '外部条件') 'close: the outside Idea Card is the third side successor'
    # Planned Links the fill left out go back first, as model-finish refuses them (grossman2002integration trial 10:
    # `7|a|a|5` lacked its Link to `3|a|a|2` and close had reported no fault). The fixture's fill wrote P and E
    # without their planned relations to D2 and to P.
    $planned = Last (Run 'close')
    $plannedText = (@($planned.jobs) | ForEach-Object { Get-Content -LiteralPath $_.files[0] -Raw -Encoding utf8 }) -join "`n"
    Assert ($plannedText.Contains("修正：[[$($ids['P'])]]") -and $plannedText.Contains('计划的关系') -and $plannedText.Contains("[[$($ids['D2'])]]")) "close: a planned rests-on Link the part does not author goes back to its writer ($($planned.faults) faults)"
    $pPlain = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ids['P'] } })).body
    $ePlain = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ids['E'] } })).body
    Write-Calls @(
        @{ tool = 'edit_card'; arguments = @{ at = $ids['P']; body = "$pPlain`n依赖 [[$($ids['D2'])]]：用到 (5.10)。" } },
        @{ tool = 'edit_card'; arguments = @{ at = $ids['E']; body = "$ePlain`n例示 [[$($ids['P'])]]：代入数字。" } }
    )
    $headBefore = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })).body
    $aBefore = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ids['A'] } })).body
    Write-Calls @(
        @{ tool = 'edit_card'; arguments = @{ at = $model; body = "$headBefore`n`n依赖 [[$outside]]：利率固定。" } },
        @{ tool = 'edit_card'; arguments = @{ at = $ids['A']; body = "$aBefore`n依赖 [[$($ids['P'])]]：用到命题。" } }
    )
    $close = Last (Run 'close')
    $headJob = @($state.jobs.Keys | Where-Object { @($state.jobs[$_].addresses) -contains $model })[-1]
    $aJob = @($state.jobs.Keys | Where-Object { @($state.jobs[$_].ids) -contains 'A' })[-1]
    Assert ($close.faults -eq 2 -and @($close.jobs).Count -eq 2 -and @($close.unowned).Count -eq 0) "close: two build faults, two follow-ups ($($close | ConvertTo-Json -Compress -Depth 4))"
    $toHead = @($close.jobs | Where-Object { $_.continue -eq $headJob })[0]
    $toA = @($close.jobs | Where-Object { $_.continue -eq $aJob })[0]
    Assert ($toHead -and $toA) "close: each note continues the conversation that wrote its Card (head $headJob, A $aJob)"
    $headNote = Get-Content -LiteralPath $toHead.files[0] -Raw -Encoding utf8
    $aNote = Get-Content -LiteralPath $toA.files[0] -Raw -Encoding utf8
    Assert ($headNote.Contains("修正：[[$model]]") -and $headNote.Contains("[[$outside]]") -and $headNote -match '参照') "close: the Model Card's note names the outside Card and the wording to use ($headNote)"
    Assert ($aNote.Contains("修正：[[$($ids['A'])]]") -and $aNote.Contains("[[$($ids['P'])]] 用到本卡")) "close: the part's note says how to turn the forward sentence around ($aNote)"
    Write-Calls @(
        @{ tool = 'edit_card'; arguments = @{ at = $model; body = $headBefore } },
        @{ tool = 'edit_card'; arguments = @{ at = $ids['A']; body = $aBefore } }
    )
    $closed = Last (Run 'close')
    Assert ($closed.faults -eq 0 -and @($closed.jobs).Count -eq 0) "close: a model with no build faults gets no follow-up ($($closed | ConvertTo-Json -Compress -Depth 4))"
    # close also repairs what a split or a writer left behind (trial 7): a 共用 line with an old address
    # is rebuilt from the plan, a malformed supplied-step marker goes back to its part, and report puts
    # back a 结构 list that a close writer re-sent from an older copy.
    $headNow = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })).body
    $sharedLine = @($headNow -split "`n" | Where-Object { $_ -match '（共用）' })
    Assert ($sharedLine.Count -eq 1 -and $sharedLine[0].Contains("[[$shared]]")) "close: the Model Card takes the other model's part in with one 共用 line ($($sharedLine -join ' / '))"
    $pBefore = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ids['P'] } })).body
    Write-Calls @(
        @{ tool = 'edit_card'; arguments = @{ at = $model; body = $headNow.Replace("[[$shared]]（共用）", "[[Test2020/1|b]]（共用）") } },
        @{ tool = 'edit_card'; arguments = @{ at = $ids['P']; body = "$pBefore`n两式相除。（补：书中略去：两式相除）" } }
    )
    $close3 = Last (Run 'close')
    $headFixed = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })).body
    Assert ($close3.refreshed -eq 1 -and $headFixed.Contains("[[$shared]]（共用）")) "close: a 共用 line with an old address is rebuilt from the plan ($($close3 | ConvertTo-Json -Compress -Depth 4))"
    $pJob = @($state.jobs.Keys | Where-Object { @($state.jobs[$_].ids) -contains 'P' })[-1]
    $toP = @($close3.jobs | Where-Object { $_.continue -eq $pJob })[0]
    Assert ($close3.faults -eq 1 -and $toP -and ((Get-Content -LiteralPath $toP.files[0] -Raw -Encoding utf8) -match '补出标记')) "close: a malformed marker goes back to its part's conversation ($($close3.faults) faults)"
    $eLine = [regex]::Escape("[[$($ids['E'])]]")
    Write-Calls @(@{ tool = 'edit_card'; arguments = @{ at = $model; body = (($headFixed -split "`n") | Where-Object { $_ -notmatch $eLine }) -join "`n" } })
    Assert (-not (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })).body.Contains("[[$($ids['E'])]]")) 'close: (stand-in) a writer re-sends the Model Card without one 结构 line'
    $null = Run 'report'
    $headAfter = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })).body
    Assert ($headAfter.Contains("[[$($ids['E'])]]") -and $headAfter.Contains("[[$shared]]（共用）")) 'report: the 结构 list the compiler wrote is put back after a close round'
    Write-Calls @(@{ tool = 'edit_card'; arguments = @{ at = $ids['P']; body = $pBefore } })
    $null = Invoke-Zt @(@{ tool = 'delete_card'; arguments = @{ at = $outside; confirm = 'delete' } })
    $cardJob = Get-Content -LiteralPath $apply.jobs[0].files[2] -Raw -Encoding utf8
    Assert ($cardJob -notmatch '保留这些模型外的链接' -or $cardJob -match '不用「依赖」「基于」') 'apply: the Model Card job says outside links never rest on'

    # explain: the four parts of the chain lack 思路 or 解读 (D2 is an older three-section part)
    $state = Get-Content -LiteralPath (Join-Path $work 'state.json') -Raw | ConvertFrom-Json -AsHashtable
    $ids = $state.models[$model].ids
    $d2 = $ids['D2']
    # A real part ends its 结果 with a 来源 line. D2 also has a fault before explain (R5): it is kept.
    $d2old = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $d2 } })).body
    $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $d2; body = ($d2old -replace '\*\*步骤\*\*', "- 这里对上界进行代入。`n`n**步骤**").TrimEnd() + "`n`n来源：原书 §5.2" } })
    $d2before = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $d2 } })).body
    $explain = Last (Run 'explain' @('-Models', $model))
    Assert ($explain.jobs.Count -eq 2 -and $explain.parts -eq 4) "explain: a chain of four parts gives two jobs ($($explain.jobs.Count) jobs, $($explain.parts) parts)"
    $again = Last (Run 'explain' @('-Models', $model))
    Assert (@($again.jobs).Count -eq 0) 'explain: a part already in a job gets no second job'
    $xstate = (Get-Content -LiteralPath (Join-Path $work 'state.json') -Raw | ConvertFrom-Json -AsHashtable).explain
    $j1 = $xstate.jobs[$explain.jobs[0].id]; $j2 = $xstate.jobs[$explain.jobs[1].id]
    Assert (@($j2.addresses)[0] -eq $d2 -and $j2.positions[$d2] -match '^第 4 张，共 4 张') "explain: the job holds D2 and its position line ($($j2.positions[$d2]))"
    $jobFile = Get-Content -LiteralPath $explain.jobs[1].files[2] -Raw -Encoding utf8
    Assert ($explain.jobs[1].files[0] -match 'dsh-explain-instructions' -and $jobFile -match '结果文件:' -and $jobFile.Contains("[[$d2]]")) 'explain: the job opens with the explain instructions and names its result file'
    $long = (1..6 | ForEach-Object { "第 $_ 句。" }) -join ''
    [IO.File]::WriteAllText($j1.out, (ConvertTo-Json -Depth 5 -InputObject @(
        [ordered]@{ address = $ids['D1']; idea = $long; notes = @{}; meaning = '它给出上界。' },
        [ordered]@{ address = $ids['D1~2']; blocked = '步骤还没写' }
    )), $utf8)
    [IO.File]::WriteAllText($j2.out, (ConvertTo-Json -Depth 5 -InputObject @(
        [ordered]@{ address = $d2; idea = '把上界代入融资条件。（补：书中未写）'; notes = @{ '1' = '这一步代入上一张的结果。' }; meaning = '清算价值够大，项目才借得到钱。' }
    )), $utf8)
    # explain-check: the writer's own check prints what explain-apply will refuse, and writes nothing.
    Assert ($jobFile -match "SELF-CHECK: zt-digest model -Step explain-check -Work `"[^`"]+`" -Job `"$([regex]::Escape($explain.jobs[1].id))`" -MaxSentences 12 .*-MaxExplainChars 600") "explain: the job file names its SELF-CHECK command ($(($jobFile -split "`n") -match 'SELF-CHECK'))"
    $c1 = @(Run 'explain-check' @('-Job', $explain.jobs[0].id))
    Assert ($c1[-1] -eq 'faults: 2' -and @($c1 | Where-Object { $_ -match "^$([regex]::Escape($ids['D1'])) explain R16 " }).Count -eq 1 -and @($c1 | Where-Object { $_ -match "^$([regex]::Escape($ids['D1~3'])) explain no entry" }).Count -eq 1 -and @($c1 | Where-Object { $_ -match '^\(blocked, not counted\)' }).Count -eq 1) "explain-check: the long 思路 and the missing entry are faults, the blocked part is not counted ($($c1 -join ' / '))"
    $c2 = @(Run 'explain-check' @('-Job', $explain.jobs[1].id))
    Assert ($c2[-1] -eq 'faults: 0' -and @($c2 | Where-Object { $_ -eq "$d2 ok" }).Count -eq 1 -and @($c2 | Where-Object { $_ -match '^\(kept: .*R5 ' }).Count -eq 1) "explain-check: D2 passes; its old R5 fault is kept, not counted ($($c2 -join ' / '))"
    Assert ((Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $d2 } })).body -eq $d2before) 'explain-check: writes nothing'
    $ap = Last (Run 'explain-apply')
    Assert ($ap.applied -eq 1 -and @($ap.blocked).Count -eq 1 -and @($ap.jobs).Count -eq 1 -and $ap.jobs[0].continue -eq $explain.jobs[0].id) "explain-apply: one part written, one blocked, one follow-up to the same conversation ($($ap | ConvertTo-Json -Compress -Depth 4))"
    Assert (@($ap.kept | Where-Object { $_ -match "^$([regex]::Escape($d2)): R5 " }).Count -eq 1) "explain-apply: D2's old R5 fault does not block it and is listed as kept ($($ap.kept -join ' / '))"
    $d2after = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $d2 } })).body
    Assert ($d2after -match '(?s)^\*\*思路\*\*\n第 4 张，共 4 张：从 \$P\\le L\$ 推到 \$L\\ge K-w\$。\n把上界代入.*\*\*起点\*\*.*\*\*步骤\*\*\n1\. c\n   说明：这一步代入上一张的结果。\n\n\*\*结果\*\*\n\$\$L\\ge K-w\$\$\n\n\*\*解读\*\*\n清算价值够大[^\n]*\n\n来源：原书 §5\.2$') "explain-apply: D2 gets 思路 with its position, a 说明 line, and 解读; its 来源 line stays last ($d2after)"
    $core = { param($b) (($b -replace '(?s)\*\*思路\*\*.*?(?=\*\*起点\*\*)', '' -replace '(?s)\n*\*\*解读\*\*.*?(?=\n来源：|$)', '' -split "`n") | Where-Object { $_.Trim() -and $_.Trim() -notmatch '^说明：' } | ForEach-Object { $_.Trim() }) -join "`n" }
    Assert ((& $core $d2after) -eq (& $core $d2before)) 'explain-apply: 起点, the steps, and 结果 do not change'
    $follow = Get-Content -LiteralPath $ap.jobs[0].files[0] -Raw -Encoding utf8
    Assert ($follow.Contains("[[$($ids['D1'])]]") -and $follow -match 'R16' -and $follow.Contains("[[$($ids['D1~3'])]]") -and $follow -match 'no entry') "explain-apply: the follow-up names the long 思路 (R16) and the missing entry ($follow)"
    $xstate = (Get-Content -LiteralPath (Join-Path $work 'state.json') -Raw | ConvertFrom-Json -AsHashtable).explain
    [IO.File]::WriteAllText($xstate.jobs[$explain.jobs[0].id].out, (ConvertTo-Json -Depth 5 -InputObject @(
        [ordered]@{ address = $ids['D1']; idea = '先求还款的上界。'; notes = @{}; meaning = '它给出上界。' },
        [ordered]@{ address = $ids['D1~3']; idea = '把两段合起来。'; notes = @{}; meaning = '还款不超过清算价值。' }
    )), $utf8)
    Assert ($follow -match 'SELF-CHECK: zt-digest model .*-Step explain-check') 'explain-apply: the follow-up names the SELF-CHECK command'
    $c3 = @(Run 'explain-check' @('-Job', $explain.jobs[0].id))
    Assert ($c3[-1] -eq 'faults: 0') "explain-check: the follow-up's new result file passes ($($c3 -join ' / '))"
    $ap2 = Last (Run 'explain-apply')
    Assert ($ap2.applied -eq 2 -and @($ap2.jobs).Count -eq 0) "explain-apply: the follow-up's parts are written, no more jobs ($($ap2 | ConvertTo-Json -Compress -Depth 4))"
    $xr = (Last (Run 'explain-report')).models[0]
    Assert ($xr.derivations -eq 4 -and $xr.explained -eq 3 -and @($xr.missing) -contains $ids['D1~2'] -and @($xr.blocked).Count -eq 1) "explain-report: 3 of 4 explained, the blocked part missing ($($xr | ConvertTo-Json -Compress))"
    $styled = @(Invoke-ZtCheck -Addresses "$d2,$($ids['D1']),$($ids['D1~3'])" -RequireExplain)
    Assert (-not @($styled | Where-Object { $_ -match ' R1[5-7] ' }).Count) "explain-apply: the written parts pass R15 to R17 ($($styled -join ' / '))"
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
