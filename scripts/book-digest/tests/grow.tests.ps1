#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\grow.tests.ps1 [-Digest <zt-digest.exe>]
# Drives `zt-digest grow` (plan, precheck, apply, check, report; docs/idea-grow-design.md) on a scratch archive:
# a Literature tree with two sections, an agent Topic with one Card, and a human Topic. Stands in for DeepSeek's
# answers: a new Topic with a chain of ideas, an idea that continues the agent Card, a `same` idea, a new Topic
# whose name the topic gate refuses, and a bad answer the precheck names. Checks the files, the Cards apply
# writes, a rerun, a fix round, and the Topic share in the report.
param([string]$Digest = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
# The newest zt-digest build: the two development build folders and the installed one. Two sessions build
# in different folders, and an older debug build lacks the newest grow steps.
if (-not $Digest) {
    $found = @('D:\zt-target\debug\zt-digest.exe', 'D:\zt-grow-target\debug\zt-digest.exe', (Get-Command zt-digest -ErrorAction SilentlyContinue).Source) |
        Where-Object { $_ -and (Test-Path -LiteralPath $_) } | ForEach-Object { Get-Item -LiteralPath $_ } | Sort-Object LastWriteTime -Descending
    $Digest = if ($found) { @($found)[0].FullName } else { 'zt-digest' }
}
Write-Output "zt-digest: $Digest"
$scratch = Join-Path ([IO.Path]::GetTempPath()) "grow-test-$PID"
$archive = Join-Path $scratch 'archive'
$work = Join-Path $scratch 'work'
New-Item -ItemType Directory -Force $archive | Out-Null
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$env:ZT_DIGEST_SCRIPTS = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Last($lines) {
    try { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable } catch { throw "not JSON: $(@($lines) -join ' | ')" }
}
function Grow([string[]]$more) { @(& $Digest grow @more 2>&1 | ForEach-Object { [string]$_ }) }
function Write-Calls($calls) {
    $out = @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "zt: $out" }
    , @($out | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Card($at) { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] }
function Answer($id, $value) { $value | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $work "out\$id.json") -Encoding utf8 }
$key = 'Grow2024'
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $null = Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@article{$key, title={Integration}, author={Grossman, Gene}, year={2024}}"; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = $key; kind = 'direct'; title = '第 1 组：搜寻成本决定组织形式'; body = '§1 - 搜寻' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = '外包需要先找到一个专业化的伙伴'; body = '最终产品厂商要外包，先要在市场上找到一个供应商。找到的概率取决于市场两边的厂商数目。' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = '外包价格由事后的纳什议价决定'; body = '双方找到彼此后才议价。议价时供应商的投资已经沉没。' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'direct'; title = '第 2 组：一体化的固定成本'; body = '§2 - 治理成本' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/2"; kind = 'side'; title = '一体化企业承担更高的治理成本'; body = '一体化企业要管理两个部门。管理成本高于专业化企业。' } }
    )
    $null = Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{Other2020, title={Search}, author={Diamond, Peter}, year={2020}}'; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Other2020'; kind = 'direct'; title = '第 1 组：搜寻市场的厚度'; body = '§1 - 厚度' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Other2020/1'; kind = 'side'; title = '市场越厚，找到交易伙伴越容易'; body = '买卖双方越多，每一方越快找到伙伴。' } },
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{Solo2021, title={Notes}, author={Coase, Ronald}, year={2021}}'; body = '第5页 - 企业的边界由交易成本决定。' } }
    )
    $topic = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '敲竹杠'; body = '本主题追问：专用投资以后，谁能拿到投资的回报。' } }))[0] -replace '/0$', ''
    $door = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$topic/0"; title = '事后议价让投资方得不到全部回报'
                body = "投资方先投入，之后才议价。对方只分给它一部分剩余。`n`n来源：[[$key]]" } }))[0]

    # --- plan ---
    $plan = Last (Grow @('-Step', 'plan', '-Book', $key, '-Work', $work))
    Assert ($plan.topic_jobs -eq 1 -and $plan.pending -eq 2 -and $plan.sections -eq 2 -and $plan.topics -ge 1) "plan: one topic job for two sections ($($plan | ConvertTo-Json -Compress))"
    $gt = Get-Content (Join-Path $work 'jobs\gt-001.md') -Raw -Encoding utf8
    Assert ($gt.Contains("[[$key/1]] 第 1 组：搜寻成本决定组织形式") -and $gt.Contains("[[$topic/0]] 敲竹杠") -and $gt.Contains('新建主题最多 2 个') -and $gt.Contains('-Job gt-001')) 'plan: the topic job lists the sections, the Topics, the bound and its SELF-CHECK'

    # --- the run's Topics ---
    Answer 'gt-001' @{ topics = @(
            @{ id = 'T1'; existing = "$topic/0"; sections = @(1) },
            @{ id = 'T2'; title = '企业组织形式的选择'; body = '本主题追问：企业为什么自制或外购。市场环境怎样改变这个选择。'; sections = @(1, 2) },
            @{ id = 'T3'; title = '敲竹杠！'; body = '本主题追问。'; sections = @(9) }
        ) }
    $pre = Grow @('-Step', 'precheck', '-Work', $work, '-Job', 'gt-001')
    Assert (@($pre | Where-Object { $_.StartsWith('T3 G8 ') }).Count -eq 1 -and @($pre | Where-Object { $_.StartsWith('T3 G2 ') }).Count -eq 1) "precheck: a near-name Topic and an unknown section are faults ($($pre -join ' | '))"
    Answer 'gt-001' @{ topics = @(
            @{ id = 'T1'; existing = "$topic/0"; sections = @(1) },
            @{ id = 'T2'; title = '企业组织形式的选择'; body = '本主题追问：企业为什么自制或外购。市场环境怎样改变这个选择。'; sections = @(1, 2) }
        ) }
    $pre = Grow @('-Step', 'precheck', '-Work', $work, '-Job', 'gt-001')
    Assert (@($pre)[-1].Trim() -eq 'faults: 0') "precheck: the topic answer has no fault ($($pre -join ' | '))"
    $tp = Last (Grow @('-Step', 'topics', '-Work', $work, '-Apply'))
    Assert ($tp.jobs -eq 2 -and $tp.topics -eq 2 -and $tp.created -eq 1) "topics: one Topic made, one reused, two section jobs ($($tp | ConvertTo-Json -Compress))"
    $runTopic = [string](@((Get-Content (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable).run_topics | Where-Object { $_.id -eq 'T2' })[0].root)
    Assert ($plan.baseline.topic -eq 1 -and $plan.baseline.literature -eq 3) "plan: the baseline counts the Topic Card that links the book and the three Literature notes ($($plan.baseline | ConvertTo-Json -Compress))"
    $job1 = Get-Content (Join-Path $work 'jobs\gw-001.md') -Raw -Encoding utf8
    Assert ($job1.Contains("SELF-CHECK: zt-digest grow -Step precheck -Work `"$work`" -Job gw-001") -and $job1.Contains("[[$key/1|b]] 外包价格由事后的纳什议价决定（观点卡）")) 'plan: the job carries its SELF-CHECK and the section''s Cards'
    Assert ($job1.Contains("[[$door]] 事后议价让投资方得不到全部回报（代理，可接续）")) "plan: the job lists the closest existing Topic Card ($door)"
    Assert ($job1.Contains("- [[$runTopic]] 企业组织形式的选择（本次新建）") -and $job1.Contains("[[$topic/0]] 敲竹杠（已有）")) "topics: the section job lists the run's Topics ($runTopic)"
    $instr = Get-Content (Join-Path $work 'instructions.zh.md') -Raw -Encoding utf8
    Assert ($instr.Contains("《Integration》") -and $instr.Contains((Join-Path $work 'read-only.allow.txt')) -and -not $instr.Contains('{{')) 'plan: the instructions name the book and the read-only allow file'
    Assert ((Get-Content (Join-Path $work 'topics.md') -Raw -Encoding utf8).Contains("[[$topic/0]] 敲竹杠")) 'plan: topics.md lists the Topic'

    # The read-only helper refuses a write.
    $ro = @('[{"tool":"create_successor","arguments":{"at":"' + $door + '","title":"x","body":"y"}}]' | & $Digest call - -Allow (Join-Path $work 'read-only.allow.txt') 2>&1 | ForEach-Object { [string]$_ })
    Assert (($ro -join ' ') -match 'not one of your Cards') "the read-only allow file refuses a write ($($ro -join ' | '))"

    # --- precheck: a bad answer ---
    Answer 'gw-001' @{ section = "$key/1"; topics = @(@{ id = 'T1'; title = '敲竹杠'; body = '本主题追问。' }); ideas = @(
            @{ id = 'I1'; kind = '核心'; title = '外包价格由事后的纳什议价决定'; body = "依赖 [[$key/1|b]] 的设定。"; sources = @("$key/1|b"); at = 'T1' },
            @{ id = 'I2'; kind = '关联'; title = '议价发生在投资之后所以投资不足'; body = "投资不足。例示 [[$key/1|b]]：本书。见 [[$door]]。"; sources = @("$key/1|b"); at = $door }
        ) }
    $pre = Grow @('-Step', 'precheck', '-Work', $work, '-Job', 'gw-001')
    foreach ($want in @('T1 G8', 'I1 G3', 'I1 G4', 'I1 G7', 'I2 G6', 'I2 G10')) {
        Assert (@($pre | Where-Object { $_.StartsWith("$want ") }).Count -ge 1) "precheck: names $want ($($pre -join ' | '))"
    }
    Assert (@($pre)[-1] -match '^faults: [1-9]') 'precheck: ends with the fault count'

    # --- the good answers ---
    Answer 'gw-001' @{ section = "$key/1"; ideas = @(
            @{ id = 'I1'; kind = '核心'; title = '找到伙伴的难度决定企业是否外包'; body = "外包要先在市场上找到供应商。找不到伙伴时，企业只能自己生产。例示 [[$key/1|a]]：本书用匹配概率刻画这一点。"; sources = @("$key/1|a"); at = $runTopic },
            @{ id = 'I2'; kind = '直觉'; title = '市场越厚外包越容易成立'; body = "市场两边的企业越多，每家企业越容易找到伙伴。因此外包在厚市场中更常见。对应 [[$key/1|a]]：本书的匹配函数。它推广了 [[@I1]] 的判断：伙伴越多，外包越可行。"; sources = @("$key/1|a"); at = 'I1'; as = 'direct' },
            @{ id = 'I3'; kind = '关联'; title = '外包企业的沉没投资削弱供应商议价'; body = "本卡推广 [[$door]]：外包的供应商同样先投资。议价时供应商的投资已经沉没。例示 [[$key/1|b]]：本书的外包价格。"; sources = @("$key/1|b"); at = $door; as = 'side' },
            @{ id = 'I4'; kind = '核心'; same = $door; body = "例示 [[$key/1|b]]：本书的外包价格也在投资之后议定。"; sources = @("$key/1|b") }
        ) }
    $pre = Grow @('-Step', 'precheck', '-Work', $work, '-Job', 'gw-001')
    Assert (@($pre)[-1].Trim() -eq 'faults: 0') "precheck: the good answer has no fault ($($pre -join ' | '))"
    Answer 'gw-002' @{ section = "$key/2"; topics = @(@{ id = 'T1'; title = '企业组织形式选择'; body = '本主题追问：企业为什么自制。' }); ideas = @(
            @{ id = 'I1'; kind = '有用'; title = '治理成本是一体化的固定代价'; body = "一体化企业要管理更多部门，所以它承担更高的固定成本。例示 [[$key/2|a]]：本书把这项成本写成固定成本。"; sources = @("$key/2|a"); at = $runTopic }
        ) }
    $pre = Grow @('-Step', 'precheck', '-Work', $work, '-Job', 'gw-002')
    Assert (@($pre | Where-Object { $_.StartsWith('T1 G8 ') -and $_.Contains($runTopic) }).Count -eq 1) "precheck: a section's new Topic near a run Topic is a fault ($($pre -join ' | '))"
    Answer 'gw-002' @{ section = "$key/2"; ideas = @(
            @{ id = 'I1'; kind = '有用'; title = '治理成本是一体化的固定代价'; body = "一体化企业要管理更多部门，所以它承担更高的固定成本。例示 [[$key/2|a]]：本书把这项成本写成固定成本。"; sources = @("$key/2|a"); at = $runTopic }
        ) }

    # --- apply ---
    $dry = Last (Grow @('-Step', 'apply', '-Work', $work))
    Assert ($dry.created -eq 0 -and -not (Get-Content (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable).cards.Count) 'apply without -Apply writes nothing'
    $applied = Last (Grow @('-Step', 'apply', '-Work', $work, '-Apply'))
    $state = Get-Content (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $s1 = $state.sections["$key/1"]
    $s2 = $state.sections["$key/2"]
    Assert ($applied.applied -eq 2 -and $applied.created -eq 4 -and $applied.topics -eq 0 -and $applied.same -eq 1) "apply: four Cards, one same, no Topic of its own ($($applied | ConvertTo-Json -Compress))"
    $newTopic = $runTopic
    $i1 = [string]($s1.created | Where-Object { $_.id -eq 'I1' }).card
    $i2 = [string]($s1.created | Where-Object { $_.id -eq 'I2' }).card
    $i3 = [string]($s1.created | Where-Object { $_.id -eq 'I3' }).card
    Assert ($i1 -eq ($newTopic -replace '/0$', '/1')) "apply: the first idea is the new Topic's doorway ($i1 of $newTopic)"
    Assert ($i2 -eq ($newTopic -replace '/0$', '/2')) "apply: the next step is the doorway's Direct successor ($i2)"
    Assert ($i3 -eq "$door|a") "apply: the related idea is a Side successor of the agent Card it continues ($i3)"
    $c2 = Card $i2
    Assert ($c2.body.Contains("[[$i1]]") -and $c2.body -match "来源：\[\[$key\]\]") "apply: [[@I1]] becomes the address and the source line is appended ($($c2.body))"
    Assert ((Card $door).body.Contains("例示 [[$key/1|b]]：本书的外包价格也在投资之后议定。")) 'apply: the same idea appends its sentence to the agent Card'
    $g2 = [string]($s2.created | Where-Object { $_.id -eq 'I1' }).card
    Assert ($s2.topics.Count -eq 0 -and $g2 -eq "$i1|a") "apply: a later idea at the Topic root opens a line beside the doorway ($g2)"
    # --- the link round ---
    $lp = Last (Grow @('-Step', 'links', '-Work', $work))
    Assert ($lp.link_jobs -eq 2) "links: one job per Topic tree the run wrote in ($($lp | ConvertTo-Json -Compress))"
    $lstate = Get-Content (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $lid = @($lstate.links.Keys | Where-Object { @($lstate.links[$_].cards) -contains $i2 })[0]
    $ltext = Get-Content (Join-Path $work "jobs\$lid.md") -Raw -Encoding utf8
    Assert ($ltext.Contains("[[$i2]]") -and $ltext.Contains("[[$i3]]") -and $ltext.Contains("-Job $lid")) "links: the job lists its Cards, the other tree's new Cards and its SELF-CHECK ($lid)"
    Answer $lid @{ links = @(
            @{ from = $i2; to = $i1; sentence = "对照 [[$i1]]：同一主题。" },
            @{ from = $i2; to = $i3; sentence = "见 [[$i3]]。" },
            @{ from = $i3; to = $door; sentence = "对照 [[$door]]：x。" }
        ) }
    $pre = Grow @('-Step', 'precheck', '-Work', $work, '-Job', $lid)
    Assert (@($pre | Where-Object { $_ -match '^L1 G11 .*same tree' }).Count -eq 1 -and @($pre | Where-Object { $_ -match '^L2 G11 .*plain reference' }).Count -eq 1 -and @($pre | Where-Object { $_ -match '^L3 G11 .*not a new Card of this job' }).Count -eq 1) "precheck: same tree, a plain reference and a foreign from are G11 ($($pre -join ' | '))"
    Answer $lid @{ links = @(@{ from = $i2; to = $i3; sentence = "对照 [[$i3]]：厚市场让外包可行，沉没投资却削弱供应商的议价地位。" }) }
    $pre = Grow @('-Step', 'precheck', '-Work', $work, '-Job', $lid)
    Assert (@($pre)[-1].Trim() -eq 'faults: 0') "precheck: the link answer has no fault ($($pre -join ' | '))"
    $linked = Last (Grow @('-Step', 'apply', '-Work', $work, '-Apply'))
    $lbody = (Card $i2).body
    Assert ($linked.linked -eq 1 -and $lbody.Contains("对照 [[$i3]]：厚市场让外包可行") -and $lbody.TrimEnd().EndsWith("来源：[[$key]]")) "apply: the Link sentence goes into its Card before the source line ($($linked | ConvertTo-Json -Compress))"
    $again = Last (Grow @('-Step', 'apply', '-Work', $work, '-Apply'))
    Assert ($again.applied -eq 0 -and $again.created -eq 0) 'apply: a rerun skips the applied sections'
    $replan = Last (Grow @('-Step', 'plan', '-Book', $key, '-Work', $work))
    Assert ($replan.pending -eq 0 -and $replan.topic_jobs -eq 0) 'plan: a rerun plans no applied section'

    # --- check and a fix round ---
    $null = Write-Calls @(@{ tool = 'edit_card'; arguments = @{ at = $g2; body = "一体化企业要管理更多部门；它承担更高的固定成本。例示 [[$key/2|a]]：本书。`n`n来源：[[$key]]" } })
    $checked = Last (Grow @('-Step', 'check', '-Work', $work, '-Round', '1'))
    Assert ($checked.fix -eq 1 -and $checked.cards -eq 1) "check: one fix job for the Card with a semicolon ($($checked | ConvertTo-Json -Compress))"
    $fix = Get-Content (Join-Path $work 'jobs\fx1-001.md') -Raw -Encoding utf8
    Assert ($fix.Contains("[[$g2]]") -and $fix -match 'R3' -and $fix.Contains('SELF-CHECK: zt-digest grow -Step precheck')) 'check: the fix job names the Card, its fault and its SELF-CHECK'
    Answer 'fx1-001' @{ edits = @(@{ at = $g2; title = '治理成本是一体化的固定代价'; body = "一体化企业要管理更多部门。它承担更高的固定成本。专业化企业不承担这项成本。例示 [[$key/2|a]]：本书把这项成本写成固定成本。" }) }
    $pre = Grow @('-Step', 'precheck', '-Work', $work, '-Job', 'fx1-001')
    Assert (@($pre)[-1].Trim() -eq 'faults: 0') "precheck: the fix answer has no fault ($($pre -join ' | '))"
    $fixed = Last (Grow @('-Step', 'apply', '-Work', $work, '-Apply'))
    Assert ($fixed.edited -eq 1 -and -not (Card $g2).body.Contains('；')) "apply: the fix edit is written ($($fixed | ConvertTo-Json -Compress))"
    $checked = Last (Grow @('-Step', 'check', '-Work', $work, '-Round', '2'))
    Assert ($checked.fix -eq 0) "check: nothing left after the fix ($($checked | ConvertTo-Json -Compress))"

    # --- report ---
    $report = Last (Grow @('-Step', 'report', '-Work', $work))
    Assert ($report.created -eq 4 -and $report.same -eq 1 -and $report.linked -eq 1 -and $report.faults_left -eq 0) "report: the counts ($($report | ConvertTo-Json -Compress))"
    Assert ($report.share_before -eq 0.25 -and $report.share_after -eq 0.625) "report: the Topic share before and after (1/4 → 5/8) ($($report.share_before) → $($report.share_after))"
    $md = Get-Content (Join-Path $work 'grow-report.md') -Raw -Encoding utf8
    Assert ($md -match '主题份额' -and $md -match '核心 1' -and $md -match '直觉 1' -and $report.topics -eq 1) "report: grow-report.md shows the share and the kinds; one Topic made ($($report.topics))"
    # --- the whole box: every Literature tree, no -Book ---
    $work2 = Join-Path $scratch 'work2'
    $box = Last (Grow @('-Step', 'plan', '-Work', $work2))
    Assert ($box.books -eq 3 -and $box.sections -eq 4 -and $box.pending -eq 4 -and $box.topic_jobs -eq 1) "plan: with no -Book every Literature tree of the box, a tree with no anchor as one section ($($box | ConvertTo-Json -Compress))"
    $gt2 = Get-Content (Join-Path $work2 'jobs\gt-001.md') -Raw -Encoding utf8
    Assert ($gt2.Contains('[[Other2020]]') -and $gt2.Contains('[[Solo2021]]') -and $gt2.Contains("[[$key]]")) 'plan: the topic job lists the sections of every book'
    $s1 = Get-Content (Join-Path $work2 'sections\s001.md') -Raw -Encoding utf8
    Assert ($s1.Contains('其他文献里用词接近的笔记') -and $s1.Contains('[[Other2020/1|a]] 市场越厚，找到交易伙伴越容易') -and $s1.Contains("来源：[[$key]]")) 'plan: a section lists the closest notes of other books and names its own book'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
