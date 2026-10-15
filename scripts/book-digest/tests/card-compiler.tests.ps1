#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\card-compiler.tests.ps1 [-Zt <zt.exe>]
# Drives card-compiler.ps1 through plan, apply, check, apply, and report on a scratch archive
# (ZT_CONFIG_DIR, its own service), standing in for DeepSeek's JSON files. The live archive is not
# touched. -Zt runs a zt build that is not installed yet.
param([string]$Zt = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "card-compiler-test-$PID"
$archive = Join-Path $scratch 'archive'
$work = Join-Path $scratch 'work'
New-Item -ItemType Directory -Force $archive, $work | Out-Null
if ($Zt) {
    $bin = Join-Path $scratch 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$compiler = Join-Path $PSScriptRoot '..\card-compiler.ps1' | Resolve-Path | ForEach-Object Path
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Run($step, [string[]]$more = @()) {
    $lines = @(pwsh -NoProfile -File $compiler -Step $step -Work $work @more 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "$step failed: $($lines -join "`n")" }
    $lines
}
function Last($lines) { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable }
function Write-Calls($calls) {
    $out = @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "zt: $out" }
    , @($out | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Get-Card($at) { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] }
function Save-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 10), $utf8) }
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $key = 'Key2024'
    $null = Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@book{$key, title={Test Book}, author={Hart, Oliver}, year={2024}}"; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = $key; kind = 'direct'; title = '第 1 组：契约不完全时所有权决定剩余控制权'; body = '§1.1 - 不完全契约' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'direct'; title = '第 2 组：债务把控制权交给债权人'; body = '§2.1 - 债务' } }
    )
    $topic = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '剩余控制权'; body = '本主题追问：谁决定契约没写的事。' } }))[0] -replace '/0$', ''
    $made = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$topic/0"; title = '所有者在契约空白处决定资产用法'
                body = "契约无法写明所有情形。所有者决定空白处的资产用法。依赖 [[$key/1]] 的前提：契约不完全。`n`n来源：[[$key]] §1.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '剩余控制权给所有者谈判地位'
                body = "谈判地位被剩余控制权提高。这一点见 [[@@0]] 的论证，所有者可以拒绝合作。`n`n来源：[[$key]] §1.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; kind = 'side'; title = '所有权的配置影响事前投资'
                body = "所有权决定谈判地位。谈判地位决定事后剩余的分配。依赖 [[@@0]]：事前投资取决于这个分配。`n`n来源：[[$key]] §1.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@1'; title = '债权人在违约时取得控制权'
                body = "债务规定一个还款额。违约时控制权转给债权人。这一点被很多人引用，见 [[@@1]]。`n`n来源：[[$key]] §2.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; type = 'event'; title = '通用汽车收购费雪车身公司'
                body = "通用汽车买下了费雪车身公司。`n`n来源：[[$key]] §1.1" } }
    )
    $a, $b, $c, $d, $e = $made
    # Ownership of the cards by group 1 (the digest's -Log), for a -Groups run.
    New-Item -ItemType Directory -Force (Join-Path $scratch 'staging') | Out-Null
    [IO.File]::WriteAllLines((Join-Path $scratch 'staging\owned-g01.txt'), [string[]]@($a, $b, $c, $e), $utf8)

    # --- plan ---
    $plan = Last (Run 'plan' @('-Book', $key, '-PerJob', '8', '-Staging', (Join-Path $scratch 'staging')))
    $state = Get-Content (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $fix = @($plan.jobs | Where-Object { $_.id -like 'fix-*' })
    $orph = @($plan.jobs | Where-Object { $_ -is [Collections.IDictionary] -and $_.id -like 'orphans-*' })
    Assert ($fix.Count -eq 1) "plan: one fix job ($($fix.Count))"
    Assert ($orph.Count -eq 1) "plan: one orphan job ($($orph.Count))"
    $fixCards = @($state.jobs[$fix[0].id].cards)
    Assert ($b -in $fixCards -and $d -in $fixCards -and $e -in $fixCards) "plan: the passive, refers, and event Cards have faults ($($fixCards -join ', '))"
    Assert ($a -notin $fixCards) 'plan: the good Card has no fault'
    $orphanCards = @($state.jobs[$orph[0].id].cards)
    Assert ($e -in $orphanCards -and $c -in $orphanCards) "plan: the event and the unlinked Card are orphans ($($orphanCards -join ', '))"
    $jobText = Get-Content $state.jobs[$fix[0].id].file -Raw -Encoding utf8
    Assert ($jobText -match "卡片 ``$([regex]::Escape($b))``" -and $jobText -match 'R4') 'plan: the job file lists the Card and its faults'
    Assert ((Test-Path (Join-Path $work 'dsh-card-instructions.zh.md')) -and (Get-Content (Join-Path $work 'dsh-card-instructions.zh.md') -Raw -Encoding utf8) -match "\[\[$key\]\]") 'plan: the instruction file names the book'

    # --- DeepSeek stand-in: the fix job ---
    # b: a good rewrite. d: a rewrite that loses a Link (refused). e: the event template.
    $fixOut = @{ cards = @(
            @{ address = $b; verdict = 'split'; title = '剩余控制权给所有者谈判地位'
                body = "剩余控制权提高所有者的谈判地位。依赖 [[$a]] 的论证：所有者可以拒绝合作。`n`n来源：[[$key]] §1.1"
                split = @(@{ title = '拒绝合作的权利来自所有权'; body = "依赖 [[$b]]：所有者可以拒绝合作；这个权利来自所有权。`n`n来源：[[$key]] §1.1" }) },
            @{ address = $d; verdict = 'fix'; title = '债权人在违约时取得控制权'
                body = "债务规定一个还款额。违约时控制权转给债权人。`n`n来源：[[$key]] §2.1" },
            @{ address = $e; verdict = 'fix'; title = '1926 年通用汽车收购费雪车身公司'
                body = "时间：1926 年。`n地点：美国底特律。`n参与者：通用汽车、费雪车身公司。`n经过：通用汽车买下费雪车身公司其余的股份。`n来源：[[$key]] §1.1" }
        ) }
    Save-Json $state.jobs[$fix[0].id].out $fixOut
    # The orphan job: the event gets a Link from an Idea Card; the unlinked Card from a candidate.
    $cands = $state.jobs[$orph[0].id].candidates
    $links = @()
    if (@($cands[$c]).Count) { $links += @{ to = $c; from = @($cands[$c])[0]; text = "对照 [[$c]]：那里讨论的是所有权的配置怎样通过事后剩余的分配影响事前的专用性投资，这里讨论的是谈判地位本身的来源。" } }
    if (@($cands[$e]).Count) { $links += @{ to = $e; from = @($cands[$e])[0]; text = "例示 [[$e]]：这次收购说明所有权怎样改变谈判地位。" } }
    $links += @{ to = $c; from = $key; text = "对照 [[$c]]：这一条不是候选。" }
    Save-Json $state.jobs[$orph[0].id].out @{ links = $links; none = @() }
    Assert (@($cands[$e]) -notcontains $key -and -not @(@($cands[$e]) | Where-Object { $_ -like "$key/*" -and $_ -notmatch '\|' }).Count) 'plan: an event gets only Idea Cards as candidates'

    # A Card that changes after the plan is refused, not overwritten.
    $null = Write-Calls @(@{ tool = 'append_card'; arguments = @{ at = $d; text = '一位作者另外加了一句。' } })

    # --- apply ---
    $applied = Last (Run 'apply')
    Assert ($applied.counts.fix -eq 2) "apply: two rewrites ($($applied.counts.fix))"
    Assert ($applied.counts.refused -ge 2) "apply: the changed Card and the non-candidate are refused ($($applied.counts.refused))"
    Assert ($applied.counts.orphan_links -eq @($links).Count - 1) "apply: the orphan Links are written ($($applied.counts.orphan_links))"
    $bNow = Get-Card $b
    Assert ($bNow.body -match '依赖 \[\[' -and $bNow.body -notmatch '被') 'apply: the rewrite is in the Card'
    $eNow = Get-Card $e
    Assert ($eNow.title -match '1926' -and $eNow.body -match '(?m)^参与者：') 'apply: the event follows the template'
    Assert ((Get-Card $d).body -match '一位作者另外加了一句') 'apply: a Card changed since the plan keeps its new text'
    $state = Get-Content (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($state.refused[$d].reason -match 'changed') "apply: the refusal names the change ($($state.refused[$d].reason))"
    if (@($cands[$c]).Count) { Assert ((Get-Card @($cands[$c])[0]).body -match "对照 \[\[$([regex]::Escape($c))\]\]") 'apply: the candidate links the orphan' }

    # A rewrite that loses a Link is refused: a second fix job on a fresh plan of one Card.
    $lossWork = Join-Path $scratch 'loss'
    $lines = @(pwsh -NoProfile -File $compiler -Step plan -Work $lossWork -Book $key -NoOrphans -Rules 'R4' 2>&1)
    $lossPlan = [string]$lines[-1] | ConvertFrom-Json -AsHashtable
    $lossState = Get-Content (Join-Path $lossWork 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $lossJob = $lossState.jobs[$lossPlan.jobs[0].id]
    $target = @($lossJob.cards)[0]
    $old = Get-Card $target
    Save-Json $lossJob.out @{ cards = @(@{ address = $target; verdict = 'fix'; title = $old.title; body = '这一版删掉了全部 Link。' }) }
    $lossApply = [string]@(pwsh -NoProfile -File $compiler -Step apply -Work $lossWork 2>&1)[-1] | ConvertFrom-Json -AsHashtable
    $lossState = Get-Content (Join-Path $lossWork 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($lossApply.counts.refused -eq 1 -and $lossState.refused[$target].reason -match 'lost Links') "apply: a rewrite that loses a Link is refused ($($lossState.refused[$target].reason))"
    Assert ((Get-Card $target).body -eq $old.body) 'apply: the refused Card is unchanged'

    # --- check: the refused Card gets a follow-up that continues the job ---
    $state = Get-Content (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $split = @($state.splits | Where-Object { $_.from -eq $b })
    Assert ($split.Count -eq 1) "apply: the split makes one Side successor ($(@($split | ForEach-Object address) -join ', '))"
    $s1 = [string]$split[0].address
    $owned = @(Get-Content (Join-Path $scratch 'staging\owned-g01.txt') -Encoding utf8)
    Assert ($s1 -in $owned) 'apply: the split Card joins the group that owns its origin'
    $checked = Last (Run 'check')
    $follow = @($checked.jobs | Where-Object { $_.continue })
    $newOrphans = @($checked.jobs | Where-Object { $_.id -like 'orphans-*' })
    $freshFix = @($checked.jobs | Where-Object { -not $_.continue -and $_.id -like 'fix-*' })
    Assert ($follow.Count -eq 1 -and $follow[0].continue -eq $fix[0].id) "check: one follow-up continues $($fix[0].id)"
    $state = Get-Content (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $fjob = $state.jobs[$follow[0].id]
    Assert ($d -in @($fjob.cards)) 'check: the follow-up holds the refused Card'
    Assert ($s1 -in @($fjob.cards)) 'check: the follow-up holds the split Card with its fault'
    Assert ($newOrphans.Count -eq 1 -and $s1 -in @($state.jobs[$newOrphans[0].id].cards)) 'check: the split Card gets an orphan job'
    if (@($cands[$c]).Count) { Assert ($freshFix.Count -eq 1 -and @($cands[$c])[0] -in @($state.jobs[$freshFix[0].id].cards)) 'check: the Card that took a long orphan Link gets a fresh fix job' }
    $dNow = Get-Card $d
    $sNow = Get-Card $s1
    Save-Json $fjob.out @{ cards = @(
            @{ address = $d; verdict = 'fix'; title = $dNow.title
                body = "债务规定一个还款额。违约时控制权转给债权人。依赖 [[$b]]：控制权的配置决定谈判地位。一位作者另外加了一句。`n`n来源：[[$key]] §2.1" },
            @{ address = $s1; verdict = 'fix'; title = $sNow.title; body = ($sNow.body -replace '；', '。') }
        ) }
    $ojob = $state.jobs[$newOrphans[0].id]
    Save-Json $ojob.out @{ links = @(@{ to = $s1; from = @($ojob.candidates[$s1])[0]; text = "对照 [[$s1]]：那里谈拒绝合作，这里谈控制权的来源。" }); none = @() }
    $again = Last (Run 'apply')
    Assert ($again.counts.fix -eq 2) "apply: the follow-up rewrites are written ($($again.counts.fix))"
    Assert ($again.counts.orphan_links -eq 1) 'apply: the split Card gets its Link'

    # --- report ---
    $report = Last (Run 'report')
    Assert ($report.faults_after -lt $report.faults_before) "report: faults go down ($($report.faults_before) -> $($report.faults_after))"
    Assert ($report.orphan_links -ge 1) 'report: counts the orphan Links'
    Assert ((Test-Path (Join-Path $work 'report.json')) -and (Test-Path (Join-Path $work 'decisions.json'))) 'report: writes report.json and decisions.json'

    # --- -Notes: Organize's notes become faults N1; with -Rules N1 nothing else is picked ---
    $nWork = Join-Path $scratch 'notes'
    $notesFile = Join-Path $scratch 'decisions.json'
    Save-Json $notesFile @{ thesis = '论点'; merges = @(); moves = @(); notes = @(@{ address = $a; note = '链接事件卡，说明它例示本卡。' }, @{ address = ''; note = '没有地址' }) }
    $nPlan = [string]@(pwsh -NoProfile -File $compiler -Step plan -Work $nWork -Book $key -Notes $notesFile -Rules 'N1' -NoOrphans 2>&1)[-1] | ConvertFrom-Json -AsHashtable
    $nState = Get-Content (Join-Path $nWork 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $nJob = @($nPlan.jobs)[0]
    Assert (@($nPlan.jobs).Count -eq 1 -and @($nState.jobs[$nJob.id].cards).Count -eq 1 -and @($nState.jobs[$nJob.id].cards)[0] -eq $a) "plan -Notes: one job with the noted Card only ($(@($nState.jobs[$nJob.id].cards) -join ', '))"
    Assert ((Get-Content $nState.jobs[$nJob.id].file -Raw -Encoding utf8) -match 'N1 链接事件卡') 'plan -Notes: the job names the note as N1'

    # A note that names a Link lets the rewrite replace one Link with it, and no more.
    foreach ($case in @(@{ name = 'one'; drop = @("$key/1"); ok = $true }, @{ name = 'two'; drop = @("$key/1", "$key"); ok = $false })) {
        $cw = Join-Path $scratch "replace-$($case.name)"
        $cf = Join-Path $scratch "notes-$($case.name).json"
        Save-Json $cf @(@{ address = $a; note = "把前提的出处改为 [[$e]]。" })
        $cp = [string]@(pwsh -NoProfile -File $compiler -Step plan -Work $cw -Book $key -Notes $cf -Rules 'N1' -NoOrphans 2>&1)[-1] | ConvertFrom-Json -AsHashtable
        $cs = Get-Content (Join-Path $cw 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
        $now = Get-Card $a
        $body = $now.body
        foreach ($l in $case.drop) { $body = $body.Replace("[[$l]]", '') }
        $body = $body.TrimEnd() + "`n`n例示 [[$e]]：这次收购说明所有者怎样决定资产用法。"
        Save-Json $cs.jobs[@($cp.jobs)[0].id].out @{ cards = @(@{ address = $a; verdict = 'fix'; title = $now.title; body = $body }) }
        $ca = [string]@(pwsh -NoProfile -File $compiler -Step apply -Work $cw 2>&1)[-1] | ConvertFrom-Json -AsHashtable
        if ($case.ok) { Assert ($ca.counts.fix -eq 1) 'apply -Notes: one Link may give way to the Link the note names' }
        else { Assert ($ca.counts.refused -eq 1) 'apply -Notes: two Links may not give way to one' }
    }

    # --- -Sample: a Card without faults goes to the reviewer with the review instruction ---
    $rWork = Join-Path $scratch 'review'
    $rPlan = [string]@(pwsh -NoProfile -File $compiler -Step plan -Work $rWork -Book $key -Staging (Join-Path $scratch 'staging') -Sample 1 -Rules 'R4' -NoOrphans 2>&1)[-1] | ConvertFrom-Json -AsHashtable
    $rState = Get-Content (Join-Path $rWork 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $rText = (@($rPlan.jobs) | ForEach-Object { Get-Content $rState.jobs[$_.id].file -Raw -Encoding utf8 }) -join "`n"
    Assert ($rPlan.review -ge 1 -and $rText -match '这是抽查' -and $rText -match '原文：') "plan -Sample: a sampled Card gets the review instruction and its source ($($rPlan.review) sampled)"
    Assert ($rText -notmatch "问题：`r?`n`r?`n") 'plan -Sample: no empty fault list'

    # --- a stopped apply: the rerun makes a missing split Card once, and -MaxJobs leaves the rest ---
    $sWork = Join-Path $scratch 'stopped'
    $sNotes = Join-Path $scratch 'notes-stopped.json'
    Save-Json $sNotes @(@{ address = $a; note = '拆成两张卡。' }, @{ address = $c; note = '补一句。' })
    $sPlan = [string]@(pwsh -NoProfile -File $compiler -Step plan -Work $sWork -Book $key -Notes $sNotes -Rules 'N1' -NoOrphans -PerJob 1 2>&1)[-1] | ConvertFrom-Json -AsHashtable
    $sState = Get-Content (Join-Path $sWork 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert (@($sPlan.jobs).Count -eq 2) "stopped: two jobs, one Card each ($(@($sPlan.jobs).Count))"
    $aNow = Get-Card $a
    $aJob = @($sState.jobs.Keys | Where-Object { $a -in @($sState.jobs[$_].cards) })[0]
    $cJob = @($sState.jobs.Keys | Where-Object { $c -in @($sState.jobs[$_].cards) })[0]
    $newBody = $aNow.body.TrimEnd() + "`n`n所有者因此掌握谈判地位。"
    Save-Json $sState.jobs[$aJob].out @{ cards = @(@{ address = $a; verdict = 'split'; title = $aNow.title; body = $newBody; split = @(@{ title = '空白处的决定权带来谈判地位'; body = "依赖 [[$a]]：所有者能拒绝合作，所以他有谈判地位。`n`n来源：[[$key]] §1.1" }) }) }
    $cNow = Get-Card $c
    Save-Json $sState.jobs[$cJob].out @{ cards = @(@{ address = $c; verdict = 'fix'; title = $cNow.title; body = ($cNow.body.TrimEnd() + "`n`n这一点决定事前投资。") }) }
    # The stopped run wrote the edit of $a but not its split Card.
    $null = Write-Calls @(@{ tool = 'edit_card'; arguments = @{ at = $a; body = $newBody } })
    $first = [string]@(pwsh -NoProfile -File $compiler -Step apply -Work $sWork -MaxJobs 1 2>&1)[-1] | ConvertFrom-Json -AsHashtable
    Assert ($first.applied_jobs -eq 1 -and $first.pending_jobs -eq 1) "apply -MaxJobs 1: one job written, one pending ($($first.applied_jobs), $($first.pending_jobs))"
    $second = [string]@(pwsh -NoProfile -File $compiler -Step apply -Work $sWork -MaxJobs 1 2>&1)[-1] | ConvertFrom-Json -AsHashtable
    Assert ($second.pending_jobs -eq 0) 'apply -MaxJobs 1: the second call writes the rest'
    $sides = @((Get-Card $a).sides | Where-Object { $_.title -eq '空白处的决定权带来谈判地位' })
    Assert ($sides.Count -eq 1) "apply after a stop: the missing split Card is made ($($sides.Count))"
    # A second rerun of the same job makes no second split Card.
    $sState = Get-Content (Join-Path $sWork 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $sState.jobs[$aJob].applied = $false
    $sState.splits = @()
    [IO.File]::WriteAllText((Join-Path $sWork 'state.json'), ($sState | ConvertTo-Json -Depth 20), $utf8)
    $null = @(pwsh -NoProfile -File $compiler -Step apply -Work $sWork 2>&1)
    Assert (@((Get-Card $a).sides | Where-Object { $_.title -eq '空白处的决定权带来谈判地位' }).Count -eq 1) 'apply after a stop: a split Card that exists is not made again'

    # A rerun of an orphan job appends no Link twice.
    if (@($cands[$c]).Count) {
        $mState = Get-Content (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
        $mState.jobs[$orph[0].id].applied = $false
        [IO.File]::WriteAllText((Join-Path $work 'state.json'), ($mState | ConvertTo-Json -Depth 20), $utf8)
        $null = @(pwsh -NoProfile -File $compiler -Step apply -Work $work 2>&1)
        $body = (Get-Card @($cands[$c])[0]).body
        Assert ([regex]::Matches($body, [regex]::Escape("对照 [[$c]]")).Count -eq 1) 'apply after a stop: an orphan Link is not appended twice'
    }

    # --- -Compact: each job as {id, kind, file}; the folder, instruction files, and prompts once ---
    $kWork = Join-Path $scratch 'compact'
    $kPlan = [string]@(pwsh -NoProfile -File $compiler -Step plan -Work $kWork -Book $key -Compact 2>&1)[-1] | ConvertFrom-Json -AsHashtable
    $kJobs = @($kPlan.jobs)
    Assert ($kJobs.Count -ge 1 -and -not $kJobs[0].Contains('files') -and $kJobs[0].kind -in @('fix', 'orphans') -and $kJobs[0].file -like '*.md') "plan -Compact: jobs carry id, kind, and file ($(($kJobs[0] | ConvertTo-Json -Compress)))"
    Assert ($kPlan.dir -eq $kWork -and (Test-Path (Join-Path $kWork $kJobs[0].file)) -and (Test-Path $kPlan.lead.fix) -and $kPlan.prompts.Contains($kJobs[0].kind)) 'plan -Compact: dir, lead, and prompts name what a job needs'
    $fullPlan = [string]@(pwsh -NoProfile -File $compiler -Step plan -Work (Join-Path $scratch 'full') -Book $key 2>&1)[-1] | ConvertFrom-Json -AsHashtable
    Assert (@($fullPlan.jobs)[0].Contains('files') -and -not $fullPlan.Contains('lead')) 'plan without -Compact: the output is as before'

    # --- -Groups: only the Cards group 1 owns, and the anchor ---
    $gWork = Join-Path $scratch 'groups'
    $null = @(pwsh -NoProfile -File $compiler -Step plan -Work $gWork -Book $key -Groups '1' -Staging (Join-Path $scratch 'staging') -NoOrphans 2>&1)
    $gState = Get-Content (Join-Path $gWork 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $gPicked = @($gState.picked)
    Assert ($d -notin $gPicked -and $b -in $gPicked -and "$key/1" -in $gPicked -and "$key/2" -notin $gPicked) "plan -Groups: picks the group's Cards and anchor ($($gPicked -join ', '))"
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
