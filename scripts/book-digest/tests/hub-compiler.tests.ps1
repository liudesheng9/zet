#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\hub-compiler.tests.ps1 [-Zt <zt.exe>]
# Drives hub-compiler.ps1 through survey, plan, apply, check, and report on a scratch archive
# (ZT_CONFIG_DIR, its own service), standing in for the Haiku manager's limits and DeepSeek's JSON
# files. The live archive is not touched. -Zt runs a zt build that is not installed yet (it is
# copied to the scratch folder and put first on PATH).
param([string]$Zt = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "hub-compiler-test-$PID"
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
$compiler = Join-Path $PSScriptRoot '..\hub-compiler.ps1' | Resolve-Path | ForEach-Object Path
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
function Save-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 10), $utf8) }
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    # Topic A: a main line of 4, a branch of 5 under A/2, and a legacy hub (an Idea Card) under A/1.
    $a = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '逆向选择'; body = '本主题追问：私人信息怎样让市场萎缩。' } }))[0] -replace '/0$', ''
    $b = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '信号传递'; body = '本主题追问：知情者怎样证明类型。' } }))[0] -replace '/0$', ''
    $calls = @(
        @{ tool = 'create_successor'; arguments = @{ at = "$a/0"; title = '私人信息让高质量卖者退出' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '市场可能完全瓦解' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@1'; title = '约束帕累托最优是评价标准' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@2'; title = '干预只在均衡非约束最优时有用' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@1'; kind = 'side'; title = '多重均衡'; body = '逆向选择市场可以有多个均衡。' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@4'; title = '最高工资均衡' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@5'; title = '博弈选出最高工资' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@6'; title = '其他均衡不稳定' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@7'; type = 'model'; title = '工资报价模型'; body = '问题：厂商怎样报价？' } }
    )
    $made = Write-Calls $calls
    $legacyBody = "本卡是主题「逆向选择」的枢纽。下列卡片按树中顺序给出核心论点。`n`n" + ((@($made[0..3]) + @($made[4..7]) | ForEach-Object { "- [[$_]]：一条。" }) -join "`n")
    $legacy = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = $made[0]; kind = 'side'; title = '逆向选择的核心卡'; body = $legacyBody } }))[0]
    $bCards = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$b/0"; title = '信号必须对低类型更贵'; body = "对照 [[$($made[0])]]：同一个市场。" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '分离均衡'; body = "对照 [[$($made[1])]]：瓦解的反面。" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@1'; title = '混同均衡'; body = "对照 [[$($made[2])]]：评价标准相同。" } }
    )

    # survey
    $stats = Last (Run 'survey')
    Assert ($stats.topics -eq 2) 'survey: two Topics'
    Assert ($stats.legacy -eq 1) 'survey: the legacy hub is found'
    Assert ($stats.tree_cards.max -eq 9) 'survey: tree sizes without hubs'
    Assert ((Get-Content -Raw (Join-Path $work 'survey.md')) -match '信号传递') 'survey: survey.md lists the Topics'

    # plan: the hub manager's limits
    $plan = Last (Run 'plan' @('-Limits', '{"topic_min":5,"line_min":5,"max_lines":20,"neighbour_links":3,"per_job":4,"field_min":2}'))
    Assert ($plan.limits.topic_min -eq 5 -and $plan.limits.max_lines -eq 20) 'plan: limits saved'
    Assert ($plan.trees -contains $a -and -not ($plan.trees -contains $b)) 'plan: only the tree that needs a hub'
    Assert (@($plan.jobs).Count -eq 2) 'plan: one topic job and one field job'
    $job = Get-Content -Raw -LiteralPath $plan.jobs[0].files[1]
    Assert ($job -match '必须写线索枢纽的分支' -and $job -match [regex]::Escape("[[$($made[4])]]")) 'plan: the branch needs a line hub'
    Assert ($job -match '邻近主题' -and $job -match [regex]::Escape($legacy)) 'plan: neighbours and the legacy hub in the job'
    Assert (-not ($job -match [regex]::Escape("- $legacy |"))) 'plan: the legacy hub is not listed as a member'
    $selfCheck = "SELF-CHECK: zt-digest hub -Step precheck -Work `"$work`" -File `"$(Join-Path $work "out\hub-$a.json")`""
    Assert ($job.Contains($selfCheck)) 'plan: the topic job names the SELF-CHECK command of its file (zt-digest hub, no pwsh)'
    Assert ((Get-Content -Raw -LiteralPath $plan.jobs[1].files[1]) -match 'SELF-CHECK: zt-digest hub -Step precheck .*fields\.json') 'plan: the field job names its SELF-CHECK command'
    Assert ($job -match '(?m)^HELPER: zt-digest call\r?$') 'plan: the topic job names the helper as zt-digest call'

    # DeepSeek's files
    $out = Join-Path $work 'out'
    $hubPlan = [ordered]@{
        tree = $a
        topic = [ordered]@{
            title = '私人信息让市场萎缩'; scope = "主题 $a 的入口。"
            groups = @(
                [ordered]@{ name = '后果'; items = @([ordered]@{ ref = $made[0]; reason = '高质量卖者退出，平均质量下降。' }, [ordered]@{ ref = $made[1]; reason = '极端时市场完全瓦解，没有交易。' }) },
                [ordered]@{ name = '均衡与评价'; items = @([ordered]@{ ref = "line:$($made[4])"; reason = '多个均衡怎样产生，怎样筛选。' }, [ordered]@{ ref = $made[2]; reason = '用约束帕累托最优评价均衡。' }, [ordered]@{ ref = $made[8]; reason = '工资报价模型给出厂商的报价。' }) }
            )
            neighbours = @([ordered]@{ kind = '对照'; ref = $bCards[0]; reason = '信号传递是知情一方的应对办法。' })
        }
        lines = @([ordered]@{
                anchor = $made[4]; title = '逆向选择市场可以有多个均衡'; scope = "从 [[$($made[4])]] 起的一支。"
                groups = @(
                    [ordered]@{ name = '筛选'; items = @([ordered]@{ ref = $made[6]; reason = '博弈选出最高工资的那个均衡。' }) },
                    [ordered]@{ name = '模型'; items = @([ordered]@{ ref = $made[8]; reason = '工资报价模型把筛选写成博弈。' }, [ordered]@{ ref = $made[5]; reason = '最高工资均衡是筛选的结果。' }) }
                )
                neighbours = @()
            })
    }
    Save-Json (Join-Path $out "hub-$a.json") $hubPlan
    Save-Json (Join-Path $out 'fields.json') ([ordered]@{
            fields = @([ordered]@{ name = '信息经济学'; title = '私人信息改变市场的均衡与效率'; scope = '讲私人信息的主题。'; groups = @(
                        [ordered]@{ name = '市场'; items = @([ordered]@{ tree = $a; reason = '逆向选择让市场萎缩。' }) },
                        [ordered]@{ name = '应对'; items = @([ordered]@{ tree = $b; reason = '信号让知情者证明类型。' }) }) })
            register = @([ordered]@{ term = '逆向选择'; refs = @("tree:$a", 'field:信息经济学') }, [ordered]@{ term = '不存在'; refs = @('tree:999') })
        })

    # precheck: the writer checks its own file before it replies; nothing is written
    $stateBefore = Get-Content -Raw (Join-Path $work 'state.json')
    $clean = Run 'precheck' @('-File', (Join-Path $out "hub-$a.json"))
    Assert (@($clean)[-1] -eq 'faults: 0') "precheck: a clean file gives faults: 0 ($($clean -join ' | '))"
    Assert ((Get-Content -Raw (Join-Path $work 'state.json')) -eq $stateBefore -and -not (Test-Path (Join-Path $work 'report.json'))) 'precheck: no state change'
    $drafts = Join-Path $work 'drafts'
    New-Item -ItemType Directory -Force $drafts | Out-Null
    function Precheck-Variant($name, [scriptblock]$change) {
        $copy = $hubPlan | ConvertTo-Json -Depth 10 | ConvertFrom-Json -AsHashtable
        & $change $copy
        $path = Join-Path $drafts $name
        Save-Json $path $copy
        Run 'precheck' @('-File', $path)
    }
    $short = Precheck-Variant 'short.json' { param($p) $p.topic.groups[0].items[0].reason = '短' }
    Assert (@($short -match ('^topic:' + [regex]::Escape($a) + ' bare ' + [regex]::Escape($made[0]))).Count -and @($short)[-1] -eq 'faults: 1') "precheck: a short reason is bare ($($short -join ' | '))"
    $many = Precheck-Variant 'groups.json' { param($p) $p.topic.groups = @($p.topic.groups) + @(3..8 | ForEach-Object { @{ name = "第 $_ 组"; items = @(@{ ref = $made[3]; reason = '干预只在均衡不是约束最优时有用。' }) } }) }
    Assert (@($many -match ('^topic:' + [regex]::Escape($a) + ' oversize .*8 组')).Count) "precheck: eight groups are too many ($($many -join ' | '))"
    $dead = Precheck-Variant 'dead.json' { param($p) $p.topic.groups[1].items = @($p.topic.groups[1].items) + @(@{ ref = "$a/99"; reason = '这张卡在树里并不存在。' }) }
    Assert (@($dead -match ('^topic:' + [regex]::Escape($a) + ' broken ' + [regex]::Escape("$a/99"))).Count) "precheck: a ref to a Card that does not exist ($($dead -join ' | '))"
    $semicolon = Precheck-Variant 'style.json' { param($p) $p.topic.groups[0].items[1].reason = '极端时市场完全瓦解；没有交易。' }
    Assert (@($semicolon -match ('^topic:' + [regex]::Escape($a) + ' R3 ')).Count) "precheck: zt-check finds a semicolon ($($semicolon -join ' | '))"
    $alone = Precheck-Variant 'alone.json' { param($p) $p.topic.neighbours = @(); $p.lines = @(); $p.topic.groups[1].items = @($p.topic.groups[1].items | Where-Object { $_.ref -notlike 'line:*' -and $_.ref -ne $made[8] }) }
    Assert (@($alone -match ' isolated ').Count -and @($alone -match (' unlisted .*' + [regex]::Escape("[[$($made[8])]]"))).Count -and @($alone -match (' unlisted .*' + [regex]::Escape("[[$($made[4])]]"))).Count) "precheck: isolated, an unlisted Model Card, a missing line hub ($($alone -join ' | '))"
    $fieldCheck = Run 'precheck' @('-File', (Join-Path $out 'fields.json'))
    Assert (@($fieldCheck -match '^register broken .*tree:999').Count -and -not @($fieldCheck -match '^field:').Count) "precheck: fields.json, a dead register ref ($($fieldCheck -join ' | '))"

    # apply
    $applied = Last (Run 'apply')
    Assert ($applied.trees -eq 1 -and $applied.fields -eq 1 -and $applied.register) 'apply: topic, field, and register'
    $state = Get-Content -Raw (Join-Path $work 'state.json') | ConvertFrom-Json -AsHashtable
    Assert ($state.trees[$a].topic -eq $legacy) 'apply: the legacy hub is rewritten, not duplicated'
    $topicCard = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $legacy } })
    Assert ($topicCard.type -eq 'hub' -and $topicCard.level -eq 'topic') 'apply: the legacy hub is a topic hub now'
    $lineHub = $state.trees[$a].lines[$made[4]]
    Assert ($lineHub -eq "$($made[4])|a") 'apply: the line hub is a Side successor of its anchor'
    Assert ($topicCard.body -match [regex]::Escape("[[$lineHub]]（枢纽）")) 'apply: line:<anchor> resolves to the line hub'
    $root = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = "$a/0" } })
    Assert ($root.body -match [regex]::Escape("[[$legacy]]")) 'apply: the Topic body links its hub'
    $register = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $applied.register } })
    Assert ($register.level -eq 'register' -and $register.body -match '逆向选择' -and $register.body -notmatch '不存在') 'apply: the register lists its terms and drops a dead one'

    # check: nothing to fix
    $check = Last (Run 'check')
    Assert ($check.hubs -eq 4 -and @($check.jobs).Count -eq 0) "check: four hubs, no faults ($($check.faults))"

    # a short reason: apply again updates in place, and check sends a fix to the writer's conversation
    $hubPlan.topic.groups[0].items[0].reason = '短'
    Save-Json (Join-Path $out "hub-$a.json") $hubPlan
    $again = Last (Run 'apply')
    Assert ($again.created -eq 0) 'apply again: no new Cards'
    $check = Last (Run 'check')
    Assert (@($check.jobs).Count -eq 1 -and $check.jobs[0].continue -eq $plan.jobs[0].id) 'check: one fix job continues the writer'
    Assert ((Get-Content -Raw -LiteralPath $check.jobs[0].files[0]) -match 'bare') 'check: the fix names the fault'
    Assert ((Get-Content -Raw -LiteralPath $check.jobs[0].files[0]).Contains($selfCheck)) 'check: the fix names the SELF-CHECK command'

    # report
    $report = Last (Run 'report')
    Assert ($report.counts.topic -eq 1 -and $report.counts.line -eq 1 -and $report.counts.field -eq 1 -and $report.counts.register -eq 1) 'report: one hub of each level'

    # a new Topic with no hub and no line hubs: apply creates its topic hub under <n>/1. Here no
    # line anchor loads the Card cache before the topic hub is made (the acemoglu digest, Topic 197).
    $c = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '选举权扩大'; body = '本主题追问：精英为什么让出权力。' } }))[0] -replace '/0$', ''
    $cCards = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$c/0"; title = '革命威胁迫使精英让步' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '承诺不可信时只能让权' } }
    )
    $null = Run 'survey'
    Move-Item -LiteralPath (Join-Path $out "hub-$a.json") -Destination (Join-Path $work "hub-$a.json.aside")
    Remove-Item -LiteralPath (Join-Path $out 'fields.json')
    Save-Json (Join-Path $out "hub-$c.json") ([ordered]@{
            tree = $c
            topic = [ordered]@{ title = '选举权扩大是精英对革命威胁的让步'; scope = "主题 $c 的入口。"
                groups = @([ordered]@{ name = '机制'; items = @([ordered]@{ ref = $cCards[0]; reason = '革命威胁让精英不得不让步。' }, [ordered]@{ ref = $cCards[1]; reason = '许诺不可信，所以让出的是权力。' }) })
                neighbours = @() }
            lines = @()
        })
    $newTree = Last (Run 'apply')
    $state = Get-Content -Raw (Join-Path $work 'state.json') | ConvertFrom-Json -AsHashtable
    Assert ($newTree.created -ge 1 -and $state.trees[$c].topic -eq "$c/1|a") "apply: a new Topic gets its topic hub under $c/1 ($($state.trees[$c].topic))"
}
finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
