#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\finding-compiler.tests.ps1 [-Zt <zt.exe>] [-ZtCheck <zt-check.exe>]
# Drives finding-compiler.ps1 (plan, precheck, apply, report) on a scratch archive and a scratch staging folder,
# standing in for DeepSeek's answer: two good findings, one with a missing field, one naming another group's
# Card, and one whose title breaks the Card language. Needs zt 0.20.0 or later (the Card type finding) and a
# zt-check with the F rules: by default the debug builds in D:\zt-target and D:\zt-check-target.
param([string]$Zt = 'D:\zt-target\debug\zt.exe', [string]$ZtCheck = 'D:\zt-check-target\debug\zt-check.exe')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$scratch = Join-Path ([IO.Path]::GetTempPath()) "finding-compiler-test-$PID"
$archive = Join-Path $scratch 'archive'
$staging = Join-Path $scratch 'staging'
$bin = Join-Path $scratch 'bin'
New-Item -ItemType Directory -Force $archive, (Join-Path $staging 'sections'), $bin | Out-Null
foreach ($exe in @(@($Zt, 'zt.exe'), @($ZtCheck, 'zt-check.exe'))) {
    if (-not (Test-Path -LiteralPath $exe[0])) { throw "not found: $($exe[0]) (build it, or pass -Zt / -ZtCheck)" }
    Copy-Item -LiteralPath $exe[0] -Destination (Join-Path $bin $exe[1])
}
$env:PATH = "$bin;$env:PATH"
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$compiler = Join-Path $PSScriptRoot '..\finding-compiler.ps1' | Resolve-Path | ForEach-Object Path
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Last($lines) {
    try { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable } catch { throw "not JSON: $(@($lines) -join ' | ')" }
}
function Write-Calls($calls) {
    $out = @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "zt: $out" }
    , @($out | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
$key = 'Findings2024'
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $null = Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@book{$key, title={Missing Markets}, author={Rossi, Ana}, year={2024}}"; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = $key; kind = 'direct'; title = '第 1 组：缺少保险市场推迟投资'; body = '§1 - 访谈' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'direct'; title = '第 2 组：信贷约束限制扩张'; body = '§2 - 调查' } }
    )
    # docs/literature-only-digest-design.md: each group's Idea Card is in the book's Literature tree, under its anchor.
    $ideas = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = '缺少保险市场使企业推迟投资'
                body = "企业不能转移风险，所以推迟投资。依赖 [[$key/1]]：访谈的结论。`n`n来源：[[$key]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/2"; kind = 'side'; title = '信贷约束使小企业放弃扩张'
                body = "小企业借不到钱，所以不扩张。依赖 [[$key/2]]：调查的结论。`n`n来源：[[$key]] §2" } }
    )
    Set-Content -LiteralPath (Join-Path $staging 'owned-g01.txt') -Value $ideas[0] -Encoding utf8
    Set-Content -LiteralPath (Join-Path $staging 'owned-g02.txt') -Value $ideas[1] -Encoding utf8
    [IO.File]::WriteAllText((Join-Path $staging 'sections\s01.md'), "# 第一章`n`n作者访谈了 48 家企业。`n`n其中 31 家受访企业说，缺少保险市场使它们推迟了投资。`n`n这是一个模型的假设：企业是风险中性的。`n", $utf8)
    [IO.File]::WriteAllText((Join-Path $staging 'sections\s02.md'), "# 第二章`n`n一项覆盖 1200 家小企业的问卷调查显示，42% 的企业因为借不到钱放弃了扩张。`n", $utf8)
    [IO.File]::WriteAllText((Join-Path $staging 'manifest.json'), (@{ citation_key = $key; book_title = '缺失的市场'
                groups = @(@{ n = 1; chapter = '第一章'; headings = @('访谈'); sections = @('s01.md') }, @{ n = 2; chapter = '第二章'; headings = @('调查'); sections = @('s02.md') }) } | ConvertTo-Json -Depth 5), $utf8)

    $plan = Last @(pwsh -NoProfile -File $compiler -Step plan -Staging $staging 2>&1)
    Assert ($plan.candidates -eq 3 -and $plan.groups -eq 2 -and @($plan.jobs).Count -eq 1) "plan: three result candidates (with a number and an empirical cue) in two groups, one job ($($plan.candidates), $($plan.groups))"
    $jobText = Get-Content (Join-Path $staging 'findings\fd-g001-g002.md') -Raw -Encoding utf8
    Assert ($jobText -match 'sections/s01.md:5' -and $jobText -match '42%' -and $jobText -notmatch '风险中性') 'plan: the job lists the result sentences, not the model assumption'
    Assert ($jobText -match [regex]::Escape($ideas[0]) -and $jobText -match "锚点：``$key/1``" -and $jobText.Contains("SELF-CHECK: zt-digest finding -Step precheck -Staging `"$staging`" -Work `"$(Join-Path $staging 'findings')`" -Job fd-g001-g002")) 'plan: the job names the anchors, the Idea Cards, and its SELF-CHECK line'

    $good = @(
        @{ group = 1; src = 'sections/s01.md:5'; title = '31 家受访企业因为缺少保险市场推迟了投资'; measure = '企业推迟投资的原因。'; data = '48 家企业的访谈，原书未写明年份。'
            method = '访谈。'; result = '48 家中 31 家因为缺少保险市场推迟了投资。'; limits = '原因由企业自报。'; section = '§1'; supports = @($ideas[0]); role = "这一结果印证 [[$($ideas[0])]] 的判断：缺少保险市场使企业推迟投资。" },
        @{ group = 2; src = 'sections/s02.md:3'; title = '42% 的受调查小企业因为借不到钱放弃了扩张'; measure = '小企业放弃扩张的原因。'; data = '1200 家小企业的问卷，原书未写明年份和地点。'
            method = '问卷调查。'; result = '42% 的企业因为借不到钱放弃了扩张。'; limits = '原书未写明。'; section = '§2'; supports = @($ideas[1]); role = "这一结果印证 [[$($ideas[1])]] 的判断：信贷约束限制扩张。" }
    )
    $noLimits = $good[0].Clone(); $noLimits.title = '受访企业的投资推迟了'; $noLimits.limits = ''
    $foreign = $good[1].Clone(); $foreign.title = '小企业放弃扩张的比例接近一半'; $foreign.supports = @($ideas[0]); $foreign.role = "这一结果印证 [[$($ideas[0])]] 的判断。"
    $semicolon = $good[0].Clone(); $semicolon.title = '31 家企业推迟投资；原因是缺少保险'
    $answer = Join-Path $staging 'findings\out\fd-g001-g002.json'
    @{ findings = @($good + $noLimits + $foreign + $semicolon) } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $answer -Encoding utf8
    $pre = @(pwsh -NoProfile -File $compiler -Step precheck -Staging $staging -Job fd-g001-g002 2>&1 | ForEach-Object { [string]$_ })
    Assert (@($pre | Where-Object { $_ -match '^finding:3 FD1 missing field: limits' }).Count -eq 1) "precheck: a missing field is a fault ($($pre -join ' | '))"
    Assert (@($pre | Where-Object { $_ -match '^finding:4 FD2 .* is not a Card of group 2' }).Count -eq 1) 'precheck: a supported Card of another group is a fault'
    Assert (@($pre | Where-Object { $_ -match '^finding:5 R' }).Count -ge 1) "precheck: zt-check flags the title with ； ($($pre -join ' | '))"
    Assert (-not @($pre | Where-Object { $_ -match '^finding:[12] ' }).Count -and $pre[-1] -match '^faults: [1-9]') "precheck: the two good findings pass, F1 to F3 included ($($pre -join ' | '))"

    $applied = Last @(pwsh -NoProfile -File $compiler -Step apply -Staging $staging -Apply 2>&1)
    Assert ($applied.ok -and $applied.created -eq 2 -and $applied.linked -eq 2 -and @($applied.refused).Count -eq 3) "apply: two created, two links, three refused ($($applied | ConvertTo-Json -Compress))"
    $state = Get-Content (Join-Path $staging 'findings\state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $made = @($state.applied['fd-g001-g002'].created)
    $card = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $made[0] } }))[0]
    Assert ($card.type -eq 'finding' -and $card.parent.address -eq "$key/1" -and $card.body -match '^测量：' -and $card.body -match "来源：\[\[$key/1\]\] §1") "apply: the Finding Card sits beside its group's anchor with the six labels ($($made[0]))"
    $idea = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ideas[0] } }))[0]
    Assert ($idea.body -match "印证 \[\[$([regex]::Escape($made[0]))\]\]：31 家受访企业因为缺少保险市场推迟了投资。") 'apply: the Idea Card links its finding with the kind word 印证'
    $check = @(Invoke-ZtCheck -Addresses ($made -join ','))
    Assert (-not @($check | Where-Object { $_ -match ' F[1-4] ' }).Count) "zt-check: the Finding Cards pass F1 to F4 ($($check -join ' | '))"
    $search = (@(Invoke-Zt @(@{ tool = 'search_cards'; arguments = @{ type = 'finding'; workspace = 'agent' } })) | ConvertTo-Json -Depth 8 -Compress)
    Assert ($made.Count -eq 2 -and $search.Contains("`"$($made[0])`"") -and $search.Contains("`"$($made[1])`"")) "zt: search_cards type finding lists both ($($search.Substring(0, [Math]::Min(300, $search.Length))))"

    $again = Last @(pwsh -NoProfile -File $compiler -Step apply -Staging $staging -Apply 2>&1)
    Assert ($again.applied_jobs -eq 0 -and $again.created -eq 0) 'apply: a rerun skips the applied job'
    $report = Last @(pwsh -NoProfile -File $compiler -Step report -Staging $staging 2>&1)
    Assert ($report.jobs -eq 1 -and $report.created -eq 2 -and $report.linked -eq 2 -and $report.refused -eq 3) "report: the counts ($($report | ConvertTo-Json -Compress))"

    # A parallel stage (models, events) edits an Idea Card between apply's read and its write. The edit carries
    # base, so zt refuses it; the job stays pending, and a rerun writes the Link and keeps the other stage's line.
    # zt-digest's test seam stands in for the parallel stage: before the first write batch that edits
    # ZT_RACE_TARGET it appends ZT_RACE_LINE to that Card once (ZT_RACE_FLAG marks it done).
    $env:ZT_RACE_TARGET = $ideas[1]
    $env:ZT_RACE_FLAG = Join-Path $scratch 'raced'
    $env:ZT_RACE_LINE = '事件阶段在这里补了一句。'
    $racer = $compiler
    $late = $good[1].Clone(); $late.title = '1200 家受调查小企业中多数申请过贷款'; $late.result = '多数受调查小企业申请过贷款。'
    @{ findings = @($late) } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $staging 'findings\out\fd-02.json') -Encoding utf8
    $raced = Last @(pwsh -NoProfile -File $racer -Step apply -Staging $staging -Apply 2>&1)
    $idea = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ideas[1] } }))[0]
    Assert ($idea.body.Contains($env:ZT_RACE_LINE)) 'apply: an Idea Card edited by a parallel stage keeps that stage''s line'
    Assert (-not $raced.ok -and $raced.applied_jobs -eq 0 -and $raced.pending_jobs -eq 1 -and $raced.created -eq 1) "apply: a helper run that fails part way leaves the job pending ($($raced | ConvertTo-Json -Compress))"
    $retry = Last @(pwsh -NoProfile -File $racer -Step apply -Staging $staging -Apply 2>&1)
    $state = Get-Content (Join-Path $staging 'findings\state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $lateMade = @($state.applied['fd-02'].created)
    Assert ($retry.ok -and $retry.applied_jobs -eq 1 -and $retry.created -eq 0 -and $retry.linked -eq 1 -and $lateMade.Count -eq 1) "apply: the rerun applies the job and reuses the Finding Card made before the failure ($($retry | ConvertTo-Json -Compress))"
    $idea = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ideas[1] } }))[0]
    Assert ($idea.body.Contains($env:ZT_RACE_LINE) -and $idea.body -match "印证 \[\[$([regex]::Escape([string]$lateMade[0]))\]\]：1200 家") "apply: the rerun writes the missing Link ($($idea.body))"
    $found = @(@(Invoke-Zt @(@{ tool = 'search_cards'; arguments = @{ query = '1200 家受调查小企业中多数申请过贷款'; type = 'finding' } }))[0].results)
    Assert ($found.Count -eq 1) "apply: no second Finding Card ($($found.Count))"
    $env:ZT_RACE_TARGET = ''
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
