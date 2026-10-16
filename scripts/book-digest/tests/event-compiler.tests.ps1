#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\event-compiler.tests.ps1 [-Zt <zt.exe>]
# Drives event-compiler.ps1 (plan, precheck, apply, report) on a scratch archive and a scratch staging folder,
# standing in for DeepSeek's answer: one new event, one event of an existing Event Card, and one event that
# names a Card of another group. Checks the job file, the self-check, the Cards apply writes, and a rerun.
param([string]$Zt = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$scratch = Join-Path ([IO.Path]::GetTempPath()) "event-compiler-test-$PID"
$archive = Join-Path $scratch 'archive'
$staging = Join-Path $scratch 'staging'
New-Item -ItemType Directory -Force $archive, (Join-Path $staging 'sections') | Out-Null
if ($Zt) {
    $bin = Join-Path $scratch 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$compiler = Join-Path $PSScriptRoot '..\event-compiler.ps1' | Resolve-Path | ForEach-Object Path
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Last($lines) {
    try { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable } catch { throw "not JSON: $(@($lines) -join ' | ')" }
}
function Write-Calls($calls) {
    $out = @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "zt: $out" }
    , @($out | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
$key = 'Events2024'
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $null = Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@book{$key, title={Events}, author={Klein, Benjamin}, year={2024}}"; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = $key; kind = 'direct'; title = '第 1 组：一体化消除敲竹杠'; body = '§1 - 一体化' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'direct'; title = '第 2 组：契约可以替代一体化'; body = '§2 - 契约' } },
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{Other2020, title={Other}, author={Coase, Ronald}, year={2020}}'; body = '另一本书。' } }
    )
    $topic = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '纵向一体化'; body = '本主题追问：企业为什么买下供应商。' } }))[0] -replace '/0$', ''
    $ideas = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$topic/0"; title = '纵向一体化消除供应商的敲竹杠'
                body = "买方买下供应商后，供应商不再能抬价。依赖 [[$key/1]]：专用投资引出敲竹杠。`n`n来源：[[$key]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; kind = 'side'; title = '长期契约也能约束供应商的抬价'
                body = "长期契约写明价格，供应商不能抬价。对照 [[$key/1]]：一体化不是唯一的办法。`n`n来源：[[$key]] §2" } }
    )
    $old = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = $ideas[1]; kind = 'side'; type = 'event'; title = '1919 年通用汽车与费雪车身公司签订长期合同'
                body = "时间：1919 年`n地点：美国`n参与者：通用汽车、费雪车身公司`n经过：两家公司签订十年的供货合同。`n来源：[[Other2020]] §4`n`n证据作用：本事件例示 [[$($ideas[1])]] 的论点。" } }))[0]
    Set-Content -LiteralPath (Join-Path $staging 'owned-g01.txt') -Value $ideas[0] -Encoding utf8
    Set-Content -LiteralPath (Join-Path $staging 'owned-g02.txt') -Value $ideas[1] -Encoding utf8
    [IO.File]::WriteAllText((Join-Path $staging 'manifest.json'), (@{ citation_key = $key; book_title = '事件测试'
                groups = @(@{ n = 1; chapter = '第一章'; headings = @('一体化') }, @{ n = 2; chapter = '第二章'; headings = @('契约') }) } | ConvertTo-Json -Depth 5), $utf8)
    [IO.File]::WriteAllText((Join-Path $staging 'events.json'), (ConvertTo-Json -InputObject @(
                @{ group = 1; src = 'sections/s01.md:3'; text = '1926 年，通用汽车买下了费雪车身公司。' },
                @{ group = 2; src = 'sections/s02.md:5'; text = '1919 年，两家公司签订了十年的合同。' }) -Depth 4), $utf8)

    $plan = Last @(pwsh -NoProfile -File $compiler -Step plan -Staging $staging 2>&1)
    Assert ($plan.candidates -eq 2 -and $plan.groups -eq 2 -and @($plan.jobs).Count -eq 1) "plan: two candidates of two groups in one job ($($plan.candidates), $($plan.groups), $(@($plan.jobs).Count))"
    $jobText = Get-Content (Join-Path $staging 'events\ev-01.md') -Raw -Encoding utf8
    Assert ($jobText -match [regex]::Escape($ideas[0]) -and $jobText -match '纵向一体化消除供应商的敲竹杠' -and $jobText -match 'sections/s01.md:3') 'plan: the job lists each group''s Idea Cards with titles and its candidates'
    Assert ($jobText -match 'SELF-CHECK: pwsh -NoProfile -File .*event-compiler\.ps1" -Step precheck .* -Job ev-01') 'plan: the job carries its SELF-CHECK line'
    Assert ((Get-Content (Join-Path $staging 'events\dsh-event-instructions.zh.md') -Raw -Encoding utf8) -match "\[\[$key\]\]") 'plan: the instructions name the book'

    $answer = Join-Path $staging 'events\out\ev-01.json'
    $good = @(
        @{ group = 1; src = 'sections/s01.md:3'; title = '1926 年通用汽车收购费雪车身公司'; time = '1926 年'; place = '美国'; actors = '通用汽车、费雪车身公司'
            course = '通用汽车买下费雪车身公司的全部股份。'; section = '§1'; supports = @($ideas[0]); role = "本事件例示 [[$($ideas[0])]] 的论点：一体化消除敲竹杠。"; existing = '' },
        @{ group = 2; src = 'sections/s02.md:5'; title = '1919 年通用汽车与费雪车身公司签订长期合同'; time = '1919 年'; place = '美国'; actors = '通用汽车、费雪车身公司'
            course = '两家公司签订十年的供货合同。'; section = '§2'; supports = @($ideas[1]); role = "本事件例示 [[$($ideas[1])]] 的论点：契约约束抬价。"; existing = $old }
    )
    $wrong = @{ group = 2; src = 'sections/s02.md:5'; title = '1930 年某事'; time = '1930 年'; place = '原书未写明'; actors = '原书未写明'; course = '发生了一件事。'
        section = '§2'; supports = @($ideas[0]); role = "本事件例示 [[$($ideas[0])]] 的论点。"; existing = '' }
    @{ events = @($good + $wrong) } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $answer -Encoding utf8
    $pre = @(pwsh -NoProfile -File $compiler -Step precheck -Staging $staging -Job ev-01 2>&1 | ForEach-Object { [string]$_ })
    Assert ($pre[-1] -match '^faults: [1-9]' -and @($pre | Where-Object { $_ -match '^event:3 EV2 .* is not a Card of group 2' }).Count -eq 1) "precheck: an event naming another group's Card is a fault ($($pre -join ' | '))"
    Assert (-not @($pre | Where-Object { $_ -match '^event:[12] ' }).Count) "precheck: the two good events pass ($($pre -join ' | '))"

    $applied = Last @(pwsh -NoProfile -File $compiler -Step apply -Staging $staging -Apply 2>&1)
    Assert ($applied.ok -and $applied.created -eq 1 -and $applied.merged -eq 1 -and $applied.linked -eq 2 -and @($applied.refused).Count -eq 1) "apply: one created, one merged, two links, one refused ($($applied | ConvertTo-Json -Compress))"
    $state = Get-Content (Join-Path $staging 'events\state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $made = @($state.applied['ev-01'].created)[0]
    $event = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $made } }))[0]
    Assert ($event.type -eq 'event' -and $event.parent.address -eq $ideas[0] -and $event.body -match '^时间：1926 年' -and $event.body -match "来源：\[\[$key\]\] §1") "apply: the new Event Card sits beside its Idea Card with the labels ($made)"
    $idea = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ideas[0] } }))[0]
    Assert ($idea.body -match "案例 \[\[$([regex]::Escape($made))\]\]：1926 年通用汽车收购费雪车身公司。") 'apply: the Idea Card links its event with the kind word 案例'
    $merged = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $old } }))[0]
    Assert ($merged.body -match "来源：\[\[Other2020\]\] §4`n来源：\[\[$key\]\] §2" -and $merged.body -match '契约约束抬价') 'apply: the existing Event Card gets this book''s source and role, no new Card'
    $other = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ideas[1] } }))[0]
    Assert ($other.body -match "案例 \[\[$([regex]::Escape($old))\]\]") 'apply: the merged event is linked from its Idea Card'
    $check = @(Invoke-ZtCheck -Addresses $made)
    Assert (-not @($check | Where-Object { $_ -match ' E[12] ' }).Count) "zt-check: the new Event Card passes E1 and E2 ($($check -join ' | '))"

    $again = Last @(pwsh -NoProfile -File $compiler -Step apply -Staging $staging -Apply 2>&1)
    Assert ($again.applied_jobs -eq 0 -and $again.created -eq 0) 'apply: a rerun skips the applied job'
    $replan = Last @(pwsh -NoProfile -File $compiler -Step plan -Staging $staging 2>&1)
    Assert (@($replan.jobs).Count -eq 0) 'plan: a rerun plans no applied job'
    $report = Last @(pwsh -NoProfile -File $compiler -Step report -Staging $staging 2>&1)
    Assert ($report.jobs -eq 1 -and $report.created -eq 1 -and $report.merged -eq 1 -and $report.linked -eq 2 -and $report.refused -eq 1) "report: the counts ($($report | ConvertTo-Json -Compress))"
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
