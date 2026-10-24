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
if (-not $Digest -and $env:ZT_DIGEST) { $Digest = (Resolve-Path -LiteralPath $env:ZT_DIGEST).Path }
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
function Grow-Run([string[]]$more) {
    $lines = @(& $Digest grow @more 2>&1 | ForEach-Object { [string]$_ })
    [pscustomobject]@{ code = $LASTEXITCODE; lines = $lines; last = @($lines)[-1]; text = ($lines -join ' ') }
}
# The GUI of this scratch service: its URL and the token its page carries. The owner sets and releases
# a Lock only here (Tree Lock S15 needs a real Lock the tool cannot take).
function Get-Gui {
    $line = @(@(zt gui 2>&1) | Where-Object { $_ -match '^zt gui: http' })[0]
    if (-not $line) { throw 'zt gui printed no URL' }
    $url = (($line -replace '^zt gui: ', '') -replace '/+$', '')
    $page = (Invoke-WebRequest -UseBasicParsing -Uri $url).Content
    $token = [regex]::Match($page, 'name="zt-token" content="([^"]+)"').Groups[1].Value
    if (-not $token) { throw 'the served page carries no token' }
    [pscustomobject]@{ url = $url; token = $token }
}
function Invoke-Gui($gui, [string]$path, $body) {
    Invoke-RestMethod -Method Post -Uri "$($gui.url)$path" -Headers @{ 'X-ZT-Token' = $gui.token } -ContentType 'application/json' -Body ($body | ConvertTo-Json -Depth 8)
}
function Lock-Tree([string]$root, [string]$note = 'reviewed') {
    $gui = Get-Gui
    $plan = Invoke-Gui $gui '/api/lock' @{ root = $root; note = $note }
    if (-not $plan.plan.plan_hash) { throw "the lock plan carries no plan_hash: $($plan | ConvertTo-Json -Depth 8 -Compress)" }
    $done = Invoke-Gui $gui '/api/lock' @{ root = $root; note = $note; confirm = 'lock'; plan_hash = $plan.plan.plan_hash }
    if (-not $done.applied) { throw "the owner lock was not applied: $($done | ConvertTo-Json -Depth 8 -Compress)" }
    $done.lock
}
function Unlock-Tree([int]$lockId) {
    $gui = Get-Gui
    $plan = Invoke-Gui $gui '/api/unlock' @{ lock_id = $lockId }
    if (-not $plan.plan.plan_hash) { throw "the unlock plan carries no plan_hash: $($plan | ConvertTo-Json -Depth 8 -Compress)" }
    $done = Invoke-Gui $gui '/api/unlock' @{ lock_id = $lockId; confirm = 'unlock'; plan_hash = $plan.plan.plan_hash }
    if (-not $done.applied) { throw "the owner unlock was not applied: $($done | ConvertTo-Json -Depth 8 -Compress)" }
    $done
}
function Write-Calls($calls) {
    $out = @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "zt: $out" }
    , @($out | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Card($at) { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] }
function Answer($id, $value, $folder = $work) { $value | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $folder "out\$id.json") -Encoding utf8 }
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
    Assert ($job1.Contains('一节在一个主题的根上只放一个想法') -and $job1.Contains('G13')) 'plan: the run-topics block asks one root placement per Topic (G13)'
    Assert ($job1.Contains("- [[$runTopic]] 企业组织形式的选择（本次新建）") -and $job1.Contains("[[$topic/0]] 敲竹杠（已有）")) "topics: the section job lists the run's Topics ($runTopic)"
    $instr = Get-Content (Join-Path $work 'instructions.zh.md') -Raw -Encoding utf8
    Assert ($instr.Contains("《Integration》") -and $instr.Contains((Join-Path $work 'read-only.allow.txt')) -and -not $instr.Contains('{{')) 'plan: the instructions name the book and the read-only allow file'
    Assert ((Get-Content (Join-Path $work 'topics.md') -Raw -Encoding utf8).Contains("[[$topic/0]] 敲竹杠")) 'plan: topics.md lists the Topic'

    # The read-only helper refuses a write.
    $ro = @('[{"tool":"create_successor","arguments":{"at":"' + $door + '","title":"x","body":"y"}}]' | & $Digest call -Open - -Allow (Join-Path $work 'read-only.allow.txt') 2>&1 | ForEach-Object { [string]$_ })
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
    # Two ideas at one Topic root would open two lines beside its doorway: the second is G13.
    Answer 'gw-002' @{ section = "$key/2"; ideas = @(
            @{ id = 'I1'; kind = '有用'; title = '治理成本是一体化的固定代价'; body = "一体化企业要管理更多部门，所以它承担更高的固定成本。例示 [[$key/2|a]]：本书把这项成本写成固定成本。"; sources = @("$key/2|a"); at = $runTopic },
            @{ id = 'I2'; kind = '直觉'; title = '专业化企业省下管理第二个部门的开销'; body = "专业化企业只管一个部门，不必为第二个部门设管理层，所以它的固定成本更低。例示 [[$key/2|a]]：本书的固定成本只落在一体化企业身上。"; sources = @("$key/2|a"); at = $runTopic }
        ) }
    $pre = Grow @('-Step', 'precheck', '-Work', $work, '-Job', 'gw-002')
    Assert (@($pre | Where-Object { $_.StartsWith('I2 G13 ') -and $_.Contains('"at": "I1"') }).Count -eq 1 -and @($pre | Where-Object { $_.StartsWith('I1 G13 ') }).Count -eq 0) "precheck: a second idea at one Topic root is G13 and names the idea to chain from ($($pre -join ' | '))"
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
    # --- the hubs round (2.5.0): the hub compiler and the register, wrapped as grow jobs ---
    $hp = Last (Grow @('-Step', 'hubs-plan', '-Work', $work))
    Assert ($hp.hub_jobs -ge 1 -and $hp.limits.topic_min -ge 5 -and $hp.limits.max_lines -le 25) "hubs-plan: the hub jobs and the limits from the survey ($($hp | ConvertTo-Json -Compress))"
    $gh = Get-Content (Join-Path $work 'jobs\gh-001.md') -Raw -Encoding utf8
    Assert ($gh.Contains('# 枢纽任务 gh-001') -and $gh.Contains('SELF-CHECK: zt-digest hub -Step precheck')) 'hubs-plan: a grow hub job holds the hub instructions and the hub task with its SELF-CHECK'
    $pre = Grow @('-Step', 'precheck', '-Work', $work, '-Job', 'gh-001')
    Assert (@($pre)[-1] -match '^faults: [1-9]' -and ($pre -join ' ') -match 'missing') "precheck: a hub job whose JSON file is missing has a fault ($($pre -join ' | '))"
    $hr = Last (Grow @('-Step', 'hubs-report', '-Work', $work, '-Register', 'plan'))
    Assert ($hr.register.status -eq 'planned' -and $null -ne $hr.missing) "hubs-report: the hub report and a staged register plan, nothing published ($($hr | ConvertTo-Json -Compress -Depth 4))"

    # --- the whole box: every Literature tree, no -Book ---
    $work2 = Join-Path $scratch 'work2'
    $box = Last (Grow @('-Step', 'plan', '-Work', $work2))
    Assert ($box.books -eq 3 -and $box.sections -eq 4 -and $box.pending -eq 4 -and $box.topic_jobs -eq 1) "plan: with no -Book every Literature tree of the box, a tree with no anchor as one section ($($box | ConvertTo-Json -Compress))"
    $gt2 = Get-Content (Join-Path $work2 'jobs\gt-001.md') -Raw -Encoding utf8
    Assert ($gt2.Contains('[[Other2020]]') -and $gt2.Contains('[[Solo2021]]') -and $gt2.Contains("[[$key]]")) 'plan: the topic job lists the sections of every book'
    $s1 = Get-Content (Join-Path $work2 'sections\s001.md') -Raw -Encoding utf8
    Assert ($s1.Contains('其他文献里用词接近的笔记') -and $s1.Contains('[[Other2020/1|a]] 市场越厚，找到交易伙伴越容易') -and $s1.Contains("来源：[[$key]]")) 'plan: a section lists the closest notes of other books and names its own book'

    # --- Tree Lock (S15): a locked source stays readable, a locked Topic takes nothing -------------
    # A Topic whose Card shares wording with section 2, so the read-only context list has a candidate
    # the section does not already link.
    $integrationTopic = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '一体化'; body = '本主题追问：企业为什么把两个部门合在一起管理。' } }))[0] -replace '/0$', ''
    $integrationCard = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$integrationTopic/0"; title = '一体化企业承担更高的管理成本'; body = '一体化企业要管理两个部门。管理成本高于专业化企业。' } }))[0]
    # The owner locks the whole source Literature tree and two agent Topics through the GUI.
    $sourceLock = Lock-Tree $key 'the source reviewed'
    $topicRoot = "$topic/0"
    $topicLock = Lock-Tree $topicRoot 'the Topic reviewed'
    $contextLock = Lock-Tree "$integrationTopic/0" 'the related Topic reviewed'
    Assert ($sourceLock.root -eq $key -and $topicLock.lock_id -gt 0 -and $topicLock.root -eq $topicRoot -and $contextLock.lock_id -gt 0) "lock: the owner locks the source tree, a Topic and a related Topic (locks $($sourceLock.lock_id), $($topicLock.lock_id), $($contextLock.lock_id))"
    $lockWork = Join-Path $scratch 'work3'
    $plan3Run = Grow-Run @('-Step', 'plan', '-Book', $key, '-Work', $lockWork)
    $plan3 = Last $plan3Run.lines
    Assert ($plan3Run.code -eq 0 -and $plan3.pending -eq 2 -and $plan3.sections -eq 2) "lock: the plan reads a locked source book normally (exit $($plan3Run.code); $($plan3 | ConvertTo-Json -Compress))"
    Assert ($plan3.locked -eq 2) "lock: the plan reports the two locked Topics of the box ($($plan3.locked))"
    $topics3 = Get-Content (Join-Path $lockWork 'topics.md') -Raw -Encoding utf8
    Assert ($topics3 -match "\[\[$([regex]::Escape($topic))/0\]\] 敲竹杠（代理，\d+ 张，已锁定：只读，可链接，不能写入）") 'lock: the Topic catalogue marks the locked Topic read-only'
    Assert ($topics3 -match "\[\[$([regex]::Escape($integrationTopic))/0\]\] 一体化（代理，\d+ 张，已锁定：只读，可链接，不能写入）") 'lock: the catalogue marks every locked Topic'
    $titles3 = @(Get-Content (Join-Path $lockWork 'titles.json') -Raw -Encoding utf8 | ConvertFrom-Json)
    $lockedRow = @($titles3 | Where-Object { $_.address -eq $door })
    Assert ($lockedRow.Count -eq 1 -and $lockedRow[0].lock.lock_id -eq $topicLock.lock_id) "lock: titles.json carries the Lock of the locked Card ($($lockedRow[0].lock | ConvertTo-Json -Compress))"
    $s3 = Get-Content (Join-Path $lockWork 'sections\s002.md') -Raw -Encoding utf8
    $s3lists = @($s3 -split "`n" | Where-Object { $_ -match '用词最接近|已锁定|可接续' }) -join ' / '
    Assert ($s3.Contains("[[$integrationCard]] 一体化企业承担更高的管理成本（已锁定：只读，可链接）")) "lock: the section lists the locked Card as read-only context ($s3lists)"
    Assert (-not $s3.Contains("[[$integrationCard]] 一体化企业承担更高的管理成本（代理，可接续）")) "lock: the locked Card is never offered as a Card to continue ($s3lists)"

    # A destination the answer names while it is locked: the precheck reports the conflict (G12).
    Answer 'gt-001' @{ topics = @(
            @{ id = 'T1'; existing = "$topic/0"; sections = @(1, 2) },
            @{ id = 'T2'; title = '敲竹杠'; body = '本主题追问：谁拿到专用投资的回报。'; sections = @(1) }
        ) } $lockWork
    $pre = Grow @('-Step', 'precheck', '-Work', $lockWork, '-Job', 'gt-001')
    Assert (@($pre | Where-Object { $_.StartsWith('T1 G12 ') }).Count -eq 1) "lock: an existing locked Topic root is a G12 conflict ($($pre -join ' | '))"
    Assert (@($pre | Where-Object { $_.StartsWith('T2 G12 ') }).Count -eq 1) "lock: a new Topic named like a locked Topic is G12, not G8's 'file under it' ($($pre -join ' | '))"
    Answer 'gt-001' @{ topics = @(@{ id = 'T1'; title = '外包与议价'; body = '本主题追问：外包以后价格怎样形成。专用投资怎样影响议价。'; sections = @(1, 2) }) } $lockWork
    $pre = Grow @('-Step', 'precheck', '-Work', $lockWork, '-Job', 'gt-001')
    Assert (@($pre)[-1].Trim() -eq 'faults: 0') "lock: a Topic away from the locked one passes ($($pre -join ' | '))"
    $tp3 = Last (Grow @('-Step', 'topics', '-Work', $lockWork, '-Apply'))
    $runTopic3 = [string](@((Get-Content (Join-Path $lockWork 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable).run_topics | Where-Object { $_.id -eq 'T1' })[0].root)
    Assert ($tp3.created -eq 1 -and $tp3.jobs -eq 2) "lock: a new Topic is made while the source and another Topic are locked ($($tp3 | ConvertTo-Json -Compress))"

    # The same answers without the locked destination: their Cards stand ready for a mid-run Lock.
    Answer 'gw-001' @{ section = "$key/1"; ideas = @(
            @{ id = 'I1'; kind = '核心'; title = '外包要先在市场上配对才能成交'; body = "外包要先在市场上找到供应商。找不到伙伴时，企业只能自己生产。例示 [[$key/1|a]]：本书用匹配概率刻画这一点。"; sources = @("$key/1|a"); at = $runTopic3 },
            @{ id = 'I2'; kind = '直觉'; title = '厚市场让配对更快因此外包更多'; body = "市场两边的企业越多，每家企业越容易找到伙伴。因此外包在厚市场中更常见。对应 [[$key/1|a]]：本书的匹配函数。它推广了 [[@I1]] 的判断：伙伴越多，外包越可行。"; sources = @("$key/1|a"); at = 'I1' }
        ) } $lockWork
    Answer 'gw-002' @{ section = "$key/2"; ideas = @(
            @{ id = 'I1'; kind = '有用'; title = '一体化多出一层管理成本'; body = "一体化企业要管理更多部门。它承担更高的固定成本。例示 [[$key/2|a]]：本书把这项成本写成固定成本。"; sources = @("$key/2|a"); at = $runTopic3 }
        ) } $lockWork
    foreach ($job in @('gw-001', 'gw-002')) {
        $pre = Grow @('-Step', 'precheck', '-Work', $lockWork, '-Job', $job)
        Assert (@($pre)[-1].Trim() -eq 'faults: 0') "lock: $job passes while its source book is locked ($(@($pre)[-1]))"
    }

    # The owner locks the run's own Topic after its answers: the storage gate stops the stage.
    $runLock = Lock-Tree $runTopic3 'locked after its answers'
    $stopped = Grow-Run @('-Step', 'apply', '-Work', $lockWork, '-Apply')
    Assert ($stopped.code -eq 2) "lock: apply stops with the refusal exit code (exit $($stopped.code))"
    $typed = $null
    foreach ($line in $stopped.lines) {
        $trimmed = $line.TrimStart()
        if (-not $trimmed.StartsWith('{')) { continue }
        try { $candidate = $trimmed | ConvertFrom-Json } catch { continue }
        if ($candidate.code) { $typed = $candidate }
    }
    Assert ($typed -and $typed.code -eq 'card_locked' -and $typed.tree -eq $runTopic3 -and $typed.lock_id -eq $runLock.lock_id) "lock: the stop answers the typed card_locked JSON with the tree and Lock ($($stopped.lines -join ' | '))"
    Assert ($typed.message -and $typed.step -eq 'apply' -and $typed.at) "lock: the typed refusal names the step and the address it tried ($($typed | ConvertTo-Json -Compress))"
    Assert (-not ($typed.hint -match '/api/|token|Access')) "lock: the hint is no bypass: $($typed.hint)"
    $st3 = Get-Content (Join-Path $lockWork 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($st3.sections["$key/1"].applied -ne $true -and $st3.sections["$key/1"].stopped.lock_id -eq $runLock.lock_id) "lock: the stopped section keeps its place and names the Lock ($($st3.sections["$key/1"] | ConvertTo-Json -Compress))"
    Assert (Test-Path -LiteralPath (Join-Path $lockWork 'out\gw-001.json')) 'lock: the stopped stage keeps its answers'
    $runTree3 = $runTopic3 -replace '/0$', ''
    $inLocked = @((Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{} })).titles | Where-Object { $_.address -like "$runTree3/*" })
    Assert ($inLocked.Count -eq 1 -and @($inLocked | Where-Object { $_.title -eq '外包要先在市场上配对才能成交' }).Count -eq 0) "lock: nothing was written into the locked tree (only its root: $(@($inLocked | ForEach-Object { $_.address }) -join ', '))"
    $report3 = Last (Grow @('-Step', 'report', '-Work', $lockWork))
    Assert ($report3.stopped -eq 1) "lock: the report names the stopped write ($($report3 | ConvertTo-Json -Compress))"
    Assert ((Get-Content (Join-Path $lockWork 'grow-report.md') -Raw -Encoding utf8) -match '已锁定而停下的写入') 'lock: grow-report.md names the stop and the unlock first'

    # After the owner unlocks, the same answers are written: no bypass, no lost work.
    $null = Unlock-Tree $runLock.lock_id
    $applied3 = Last (Grow @('-Step', 'apply', '-Work', $lockWork, '-Apply'))
    Assert ($applied3.applied -eq 2 -and $applied3.created -eq 3 -and $applied3.stopped -eq 0) "lock: after the unlock the same answers are written ($($applied3 | ConvertTo-Json -Compress))"
    Assert (-not (Get-Content (Join-Path $lockWork 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable).sections["$key/1"].stopped) 'lock: the completed section carries no stop'
    $report4 = Last (Grow @('-Step', 'report', '-Work', $lockWork))
    Assert ($report4.stopped -eq 0 -and $report4.created -eq 3) "lock: the finished report counts three Cards and no stop, while the source book is still locked ($($report4 | ConvertTo-Json -Compress))"

    # --- A destination the answer names while it is locked: a conflict, never a substitute ----------
    $refuseWork = Join-Path $scratch 'work4'
    $null = Last (Grow @('-Step', 'plan', '-Book', $key, '-Work', $refuseWork))
    Answer 'gt-001' @{ topics = @(@{ id = 'T1'; title = '外包关系的价格形成'; body = '本主题追问：外包的价格由哪些条件决定。谁在议价中让步。'; sections = @(1) }) } $refuseWork
    $tp4 = Last (Grow @('-Step', 'topics', '-Work', $refuseWork, '-Apply'))
    Assert ($tp4.created -eq 1) "lock: a Topic is planned for the refused answer ($($tp4 | ConvertTo-Json -Compress))"
    $before = @((Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{} })).titles | Where-Object { $_.address -like '0/*' }).Count
    Answer 'gw-001' @{ section = "$key/1"; ideas = @(
            @{ id = 'I1'; kind = '关联'; title = '报价在外包谈判里定下'; body = "本卡推广 [[$door]]：外包的供应商也先投资。例示 [[$key/1|b]]：本书的外包价格。"; sources = @("$key/1|b"); at = $door; as = 'side' },
            @{ id = 'I2'; kind = '核心'; same = $door; body = "例示 [[$key/1|b]]：本书同样在投资之后议价。"; sources = @("$key/1|b") }
        ) } $refuseWork
    $pre = Grow @('-Step', 'precheck', '-Work', $refuseWork, '-Job', 'gw-001')
    Assert (@($pre | Where-Object { $_.StartsWith('I1 G12 ') -and $_ -match 'Lock \d+' }).Count -eq 1) "lock: a locked Card takes no successor and the fault names its Lock ($($pre -join ' | '))"
    Assert (@($pre | Where-Object { $_.StartsWith('I2 G12 ') }).Count -eq 1) "lock: a locked Card takes no appended sentence ($($pre -join ' | '))"
    $refused = Last (Grow @('-Step', 'apply', '-Work', $refuseWork, '-Apply'))
    Assert ($refused.created -eq 0 -and $refused.refused -eq 2 -and $refused.stopped -eq 0) "lock: both ideas are refused, neither is placed elsewhere ($($refused | ConvertTo-Json -Compress))"
    $after = @((Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{} })).titles | Where-Object { $_.address -like '0/*' }).Count
    $beside = @((Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{} })).titles | Where-Object { $_.address -eq "$door|b" })
    Assert ($after -eq $before -and $beside.Count -eq 0) "lock: the locked Topic keeps its $before Cards and nothing is placed beside $door"

    # --- A full doorway (seven lines): the next root placement goes beside its last line --------------
    function Has-Card($at) { @((Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{} })).titles | Where-Object { $_.address -eq $at }).Count -gt 0 }
    $fullTopic = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '供应商的专业化程度'; body = '本主题追问：供应商为什么只做一种中间品。' } }))[0] -replace '/0$', ''
    $fullDoor = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$fullTopic/0"; title = '专业化让供应商的单位成本下降'; body = "只做一种中间品，单位成本更低。`n`n来源：[[$key]]" } }))[0]
    $lineTitles = @('学习效应来自重复生产', '专用设备只服务一种用途', '规模越大固定成本摊得越薄', '专业化供应商依赖少数买家', '买家少时供应商议价更弱', '多样化是对冲需求风险的办法', '地理集聚降低专业化的风险')
    $null = Write-Calls @($lineTitles | ForEach-Object { @{ tool = 'create_successor'; arguments = @{ at = $fullDoor; kind = 'side'; title = $_; body = "一条思路的开头。`n`n来源：[[$key]]" } } })
    $lastLine = "$fullDoor|g"
    Assert ((Card $lastLine).title -eq '地理集聚降低专业化的风险' -and -not (Has-Card "$fullDoor|h")) "full doorway: $fullDoor has seven lines, the last $lastLine"
    $fullWork = Join-Path $scratch 'work5'
    $null = Last (Grow @('-Step', 'plan', '-Book', $key, '-Work', $fullWork))
    Answer 'gt-001' @{ topics = @(@{ id = 'T1'; existing = "$fullTopic/0"; sections = @(1, 2) }) } $fullWork
    $null = Last (Grow @('-Step', 'topics', '-Work', $fullWork, '-Apply'))
    Answer 'gw-001' @{ section = "$key/1"; ideas = @(
            @{ id = 'I1'; kind = '关联'; title = '找伙伴的难度限制了供应商的专业化'; body = "供应商越专业化，越难找到合适的买家。例示 [[$key/1|a]]：本书用匹配概率刻画找到伙伴的难度。"; sources = @("$key/1|a"); at = "$fullTopic/0" },
            @{ id = 'I2'; kind = '直觉'; title = '冷清的市场让专业化供应商等得更久'; body = "买家少时，专业化供应商要等更久才成交，等待的时间吞掉了专业化省下的一部分成本。例示 [[$key/1|a]]：本书的匹配函数。"; sources = @("$key/1|a"); at = 'I1'; as = 'direct' }
        ) } $fullWork
    $pre = Grow @('-Step', 'precheck', '-Work', $fullWork, '-Job', 'gw-001')
    Assert (@($pre)[-1].Trim() -eq 'faults: 0') "full doorway: the answer passes ($($pre -join ' | '))"
    $null = Last (Grow @('-Step', 'apply', '-Work', $fullWork, '-Apply'))
    $fullState = (Get-Content (Join-Path $fullWork 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable).sections["$key/1"]
    $placed = @($fullState.created | Where-Object { $_.id -eq 'I1' })[0]
    $chained = [string](@($fullState.created | Where-Object { $_.id -eq 'I2' })[0].card)
    Assert ($placed.card -eq "$lastLine|a" -and $placed.parent -eq $lastLine -and $placed.full -eq $fullDoor) "full doorway: the root placement is a Side successor of the last line, not an eighth line ($($placed | ConvertTo-Json -Compress))"
    Assert (-not (Has-Card "$fullDoor|h") -and $chained -eq "$lastLine|a|1") "full doorway: no eighth line, and the section chains in the new branch ($chained)"
    $fullReport = Last (Grow @('-Step', 'report', '-Work', $fullWork))
    Assert ($fullReport.deeper -eq 1 -and (Get-Content (Join-Path $fullWork 'grow-report.md') -Raw -Encoding utf8).Contains("[[$fullDoor]]")) "full doorway: the report names the full doorway ($($fullReport | ConvertTo-Json -Compress))"

    # --- 3.1.0: the concept review of each source book's term table, read by the register plan --------
    # The run's own book has no term table: it is skipped with its reason, and no review runs.
    $cb0 = Last (Grow @('-Step', 'concept-books', '-Work', $work))
    Assert ($cb0.books -eq 0 -and @($cb0.skipped | Where-Object { $_.book -eq $key -and $_.reason -match '术语表' }).Count -eq 1) "concept-books: a grown book without a term table is skipped with why ($($cb0 | ConvertTo-Json -Compress -Depth 4))"
    $termKey = 'Terms2025'
    $null = Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@book{$termKey, title={Holdup}, author={Klein, Benjamin}, year={2025}}"; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = $termKey; kind = 'direct'; title = '第 1 组：专用投资'; body = '§1 - 专用投资' } }
    )
    $tableBody = "本卡是《Holdup》的术语表。本书的卡片用左列的术语，不用右列的词。`n`n术语表`n`n| 术语 | 不用 | 意思 | 首次定义 |`n| --- | --- | --- | --- |`n| 敲竹杠 | 套牢 | 专用投资之后对方压低价格 | §1 |`n| 专用投资 | 专有投资 | 只在一段关系里有价值的投资 | §1 |`n`n来源：[[$termKey]]"
    $tableCard = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$termKey/1"; kind = 'side'; title = '术语表：《Holdup》的 2 个术语'; body = $tableBody } }))[0]
    $cb = Last (Grow @('-Step', 'concept-books', '-Work', $work, '-Books', "$termKey,Other2020"))
    $review = @($cb.reviews)[0]
    Assert ($cb.books -eq 1 -and $review.book -eq $termKey -and $review.table -eq $tableCard -and @($cb.skipped | Where-Object { $_.book -eq 'Other2020' }).Count -eq 1) "concept-books: one review for the named book with a term table, the other skipped ($($cb | ConvertTo-Json -Compress -Depth 4))"
    $manifest = Get-Content (Join-Path $review.staging 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
    $terms = Get-Content (Join-Path $review.staging 'terms.md') -Raw -Encoding utf8
    Assert ($manifest.citation_key -eq $termKey -and $manifest.book_title -and $terms.Contains('| 敲竹杠 | 套牢 |')) "concept-books: the staging folder holds the book's manifest and its term table ($($manifest | ConvertTo-Json -Compress))"
    $cw = Join-Path $review.staging 'work'
    $proposed = @(& $Digest concepts -Step propose -Staging $review.staging -Work $cw 2>&1 | ForEach-Object { [string]$_ })
    Assert ((Last $proposed).proposals -eq 2) "concepts: the staged term table proposes its two concepts ($($proposed -join ' | '))"
    $prepared = @(& $Digest concepts -Step prepare -Staging $review.staging -Work $cw 2>&1 | ForEach-Object { [string]$_ })
    Assert ((Last $prepared).ok -and @((Last $prepared).jobs).Count -eq 1) "concepts: the evidence is frozen with one judge job ($(@($prepared)[-1]))"
    $reported = @(& $Digest concepts -Step report -Staging $review.staging -Work $cw 2>&1 | ForEach-Object { [string]$_ })
    Assert ((Test-Path -LiteralPath (Join-Path $review.staging 'concept-review.json')) -and (Last $reported).report_id) "concepts: an unjudged review still renders its (partial) report ($(@($reported)[-1]))"
    $hrc = Last (Grow @('-Step', 'hubs-report', '-Work', $work, '-Register', 'plan', '-Concepts'))
    Assert ($hrc.register.status -eq 'planned' -and $hrc.register.reviews -eq 1) "hubs-report -Concepts: the register plan reads the run's concept report ($($hrc.register | ConvertTo-Json -Compress -Depth 4))"
    $planFile = Get-Content (Join-Path $work 'register\out\register-plan.json') -Raw -Encoding utf8 | ConvertFrom-Json
    Assert ($planFile.review.report_id -eq (Get-Content (Join-Path $review.staging 'concept-review.json') -Raw -Encoding utf8 | ConvertFrom-Json).report_id) 'hubs-report -Concepts: the staged plan names the review it read'
    $hrn = Last (Grow @('-Step', 'hubs-report', '-Work', $work, '-Register', 'plan'))
    Assert ($hrn.register.reviews -eq 0) 'hubs-report: without -Concepts the register plan reads no review'

    # --- 3.1.0: Topic maintenance, the old Organize's Topic merges and Card moves ----------------------
    $mtA = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '专用投资的回报'; body = '本主题追问：专用投资之后，投资方能拿回多少回报。' } }))[0] -replace '/0$', ''
    $mtB = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '专用投资的回报问题'; body = '本主题追问：投资方为什么拿不回专用投资的全部回报。' } }))[0] -replace '/0$', ''
    $mtC = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '纵向一体化'; body = '本主题追问：企业为什么把供应商并进来。' } }))[0] -replace '/0$', ''
    $aDoor = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$mtA/0"; title = '专用投资让投资方被对方压价'; body = "投资之后，对方可以压低价格。`n`n来源：[[$termKey]]" } }))[0]
    $bDoor = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$mtB/0"; title = '议价时投资已经沉没'; body = "议价发生在投资之后，投资方无法收回投资。`n`n来源：[[$termKey]]" } }))[0]
    $cDoor = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$mtC/0"; title = '一体化消除了事后议价'; body = "供应商并入以后，双方不再议价。`n`n来源：[[$termKey]]" } }))[0]
    $stray = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = $aDoor; kind = 'side'; title = '并购供应商是对付压价的办法'; body = "依赖 [[$cDoor]]：并入以后没有事后议价。对照 [[$mtC/0]]：一体化主题追问同一个办法。`n`n来源：[[$termKey]]" } }))[0]
    $mw = Join-Path $scratch 'work6'
    New-Item -ItemType Directory -Force (Join-Path $mw 'out') | Out-Null
    # The run's own Cards: the stray Card is one, so its address in state.json must follow the move.
    @{ sections = @{}; cards = @{ $stray = @{ book = $termKey; section = "$termKey/1" } }; fixes = @{}; fixed = @{} } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $mw 'state.json') -Encoding utf8
    $op = Last (Grow @('-Step', 'organize-plan', '-Work', $mw, '-Trees', "$mtA,$mtB"))
    Assert ($op.organize_jobs -eq 1 -and $op.topics -eq 2 -and $op.pairs -ge 1 -and $op.moves -eq 1) "organize-plan: the survey finds the duplicate pair and the stray Card ($($op | ConvertTo-Json -Compress))"
    $go = Get-Content (Join-Path $mw 'jobs\go-001.md') -Raw -Encoding utf8
    Assert ($go.Contains('# 主题维护任务 go-001') -and $go.Contains("-Job go-001") -and $go.Contains("``$mtA/0`` 专用投资的回报") -and $go.Contains("``$stray``")) 'organize-plan: the decision job holds its rules, its SELF-CHECK and the survey'
    $survey = Get-Content (Join-Path $mw 'organize\survey.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($survey.mode -eq 'topics' -and $survey.cards.ContainsKey($cDoor)) 'organize-plan: survey.json lists every agent Topic-tree Card a decision may name'
    Answer 'go-001' @{ merges = @(@{ drop = "$mtB/0"; keep = "$mtA/0" }); moves = @(@{ at = $stray; under = $stray; reason = 'x' }) } $mw
    $pre = Grow @('-Step', 'precheck', '-Work', $mw, '-Job', 'go-001')
    Assert (@($pre | Where-Object { $_.StartsWith('merge1 O1 ') }).Count -eq 1 -and @($pre | Where-Object { $_.StartsWith('move1 O6 ') }).Count -eq 1) "precheck go-001: a merge without its reason and a move into itself are faults ($($pre -join ' | '))"
    Answer 'go-001' @{ merges = @(@{ drop = "$mtB/0"; keep = "$mtA/0"; under = $aDoor; reason = '两个主题追问同一个问题：投资方拿不回回报。' })
        moves = @(@{ at = $stray; under = $cDoor; reason = '这张卡讨论的是一体化这个办法。' }) } $mw
    $pre = Grow @('-Step', 'precheck', '-Work', $mw, '-Job', 'go-001')
    Assert (@($pre)[-1].Trim() -eq 'faults: 0') "precheck go-001: the answer has no fault ($($pre -join ' | '))"
    $oa = Last (Grow @('-Step', 'organize-apply', '-Work', $mw))
    Assert ($oa.applied -and $oa.merges -eq 1 -and $oa.moves -eq 1 -and $oa.complete -and $oa.run_cards_renamed -eq 1) "organize-apply: one merge and one move carried out, the run's Card renamed ($($oa | ConvertTo-Json -Compress))"
    $titles = @((Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{} })).titles)
    Assert (@($titles | Where-Object { $_.tree -eq $mtB }).Count -eq 0 -and @($titles | Where-Object { $_.title -eq '议价时投资已经沉没' -and $_.tree -eq $mtA }).Count -eq 1) "organize-apply: the dropped Topic's Card now sits in the kept Topic and the dropped Topic is gone"
    $movedTo = @($titles | Where-Object { $_.title -eq '并购供应商是对付压价的办法' })[0].address
    Assert ($movedTo -like "$cDoor|*") "organize-apply: the stray Card moved under $cDoor ($movedTo)"
    $mstate = Get-Content (Join-Path $mw 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($mstate.cards.ContainsKey($movedTo) -and -not $mstate.cards.ContainsKey($stray)) "organize-apply: the run's state follows the move ($(@($mstate.cards.Keys) -join ', '))"
    $oa2 = Last (Grow @('-Step', 'organize-apply', '-Work', (Join-Path $scratch 'work7')))
    Assert (-not $oa2.applied -and $oa2.skipped) "organize-apply: no answer applies nothing ($($oa2 | ConvertTo-Json -Compress))"

    # --- a mathematics textbook (zt-grow-cycle v11): its units are notes of a section like any other ---
    $mk = 'Algebra2024'
    $null = Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@book{$mk, title={Algebra}, author={Artin, Michael}, year={2024}}"; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = $mk; kind = 'direct'; title = '第 1 组：群与子群'; body = '§1 - 群' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$mk/1"; kind = 'side'; type = 'model'; form = 'theorem'; title = '有限群的子群阶数整除群的阶'
                body = '（H1）$G$ 是有限群。（H2）$H$ 是 $G$ 的子群。则（C1）$|H|$ 整除 $|G|$。' } }
    )
    $mathWork = Join-Path $scratch 'work-math'
    $mplan = Last (Grow @('-Step', 'plan', '-Book', $mk, '-Work', $mathWork))
    Assert ($mplan.sections -eq 1 -and $mplan.pending -eq 1) "math: the textbook's anchor plans as one section ($($mplan | ConvertTo-Json -Compress))"
    $msec = Get-Content (Join-Path $mathWork 'sections\s001.md') -Raw -Encoding utf8
    Assert ($msec.Contains("## [[$mk/1|a]] 有限群的子群阶数整除群的阶（模型卡）") -and $msec.Contains('（C1）$|H|$ 整除 $|G|$')) 'math: the section lists the theorem unit as a Model Card with its statement'
    $minstr = Get-Content (Join-Path $mathWork 'instructions.zh.md') -Raw -Encoding utf8
    Assert ($minstr.Contains('**数学教材的小节**') -and $minstr.Contains('单元的陈述、前提和证明都留在文献树里')) 'math: the instructions keep a unit''s statement and proof in the Literature tree'
    # --- 4.0.0: the run as one command (grow -Step next, docs/workflow-simplicity-proposal.md S2/S3/S7) -----
    # The driver's loop by hand: each jobs line is answered (or not) and confirmed with -After; a call without
    # -After is a rerun after a stop. The checksum is computed here a third way (PowerShell) and must match.
    function Fnv([string]$text) {
        [uint32]$h = 2166136261
        for ($i = 0; $i -lt $text.Length; $i++) {
            $cp = [char]::ConvertToUtf32($text, $i)
            if ([char]::IsHighSurrogate($text[$i])) { $i++ }
            $h = [uint32](($h -bxor [uint32]$cp) * [uint64]16777619 % 4294967296)
        }
        '{0:x8}' -f $h
    }
    function Next-Line([string]$opts, [int]$after = -1, [string]$folder = $workNext) {
        $more = @('-Step', 'next', '-Work', $folder, '-Options', $opts)
        if ($after -ge 0) { $more += @('-After', [string]$after) }
        $r = Grow-Run $more
        if ($r.code -ne 0) { throw "next exited $($r.code): $($r.text)" }
        Last $r.lines
    }
    $workNext = Join-Path $scratch 'work-next'
    $opts = (@{ book = 'Other2020'; tool = $Digest; zt_config = $env:ZT_CONFIG_DIR; restate = $true; concepts = $false; hubs = $false } | ConvertTo-Json -Compress)
    $n1 = Next-Line $opts
    Assert ($n1.next -eq 'jobs' -and $n1.round -eq 'topics-jobs' -and @($n1.ids) -join ',' -eq 'gt-001' -and $n1.token -eq 1) "next: the first job set is the Topic plan ($($n1 | ConvertTo-Json -Compress -Depth 4))"
    $sumText = ([string](@($n1.ids) -join ','), $n1.job.cwd, (@($n1.job.files) -join "`n"), $n1.job.prompt, $n1.job.approve, $n1.job.expect) -join "`n"
    Assert ((Fnv $sumText) -eq $n1.sum) "next: the checksum matches an independent FNV-1a ($(Fnv $sumText) vs $($n1.sum))"
    Assert (@($n1.job.files)[1] -eq (Join-Path $workNext 'jobs\{id}.md') -and $n1.job.prompt.Contains("-Job {id}") -and $n1.job.model -eq 'deepseek-official/deepseek-flash' -and $n1.job.effort -eq 'high') 'next: the job template names its files, its SELF-CHECK and the frozen model'
    Assert ((Get-Content (Join-Path $workNext 'zt-box.txt') -Raw).Trim() -eq $env:ZT_CONFIG_DIR) 'next: a run on a box copy pins its work folder to that box'
    $st = Last (Grow @('-Step', 'status', '-Work', $workNext))
    Assert ($st.sum -eq $n1.sum -and $st.token -eq 1) 'status: repeats the last line next printed'
    # A rerun before the job set was answered (a stop mid-dispatch) prints the same jobs again, under a new token.
    $n2 = Next-Line $opts
    Assert ($n2.next -eq 'jobs' -and $n2.round -eq 'topics-jobs' -and $n2.token -eq 2) "next: a rerun prints the unanswered job set again ($($n2.round) $($n2.token))"
    $other = Next-Line (@{ book = 'Solo2021'; tool = $Digest } | ConvertTo-Json -Compress)
    Assert ($other.next -eq 'fault' -and $other.code -eq 'options_changed') "next: a rerun with other options is refused ($($other | ConvertTo-Json -Compress))"
    Answer 'gt-001' @{ topics = @(@{ id = 'T1'; title = '交易伙伴的搜寻'; body = '本主题追问：交易双方怎样找到彼此，市场的厚度怎样改变这件事。'; sections = @(1) }) } $workNext
    $n3 = Next-Line $opts 2
    Assert ($n3.next -eq 'jobs' -and $n3.round -eq 'ideas-jobs' -and @($n3.ids) -join ',' -eq 'gw-001') "next -After: the Topics are made and the section jobs follow ($($n3 | ConvertTo-Json -Compress -Depth 2))"
    $root3 = [string](@((Get-Content (Join-Path $workNext 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable).run_topics)[0].root)
    Answer 'gw-001' @{ section = 'Other2020/1'; ideas = @(
            @{ id = 'I1'; kind = '核心'; title = '交易伙伴越多搜寻越快'; body = '市场两边的参与者越多，每一方越快找到伙伴。所以厚市场降低搜寻的成本。例示 [[Other2020/1|a]]：本书用市场厚度说明这一点。'; sources = @('Other2020/1|a'); at = $root3 }
        ) } $workNext
    $pre = Grow @('-Step', 'precheck', '-Work', $workNext, '-Job', 'gw-001')
    Assert (@($pre)[-1].Trim() -eq 'faults: 0') "precheck: the next run's section answer has no fault ($($pre -join ' | '))"
    $n4 = Next-Line $opts $n3.token
    $guard = 0
    while ($n4.next -eq 'jobs' -and $guard -lt 6) { $guard++; $n4 = Next-Line $opts $n4.token }
    Assert ($n4.next -eq 'done' -and $n4.created -ge 1 -and $n4.report -like '*grow-report.md' -and $null -ne $n4.restate) "next: the run ends done with the report and the restatement review ($($n4 | ConvertTo-Json -Compress -Depth 3))"
    $mem = Get-Content (Join-Path $workNext 'next.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert (@($mem.done) -contains 'restate-finish' -and @($mem.done) -contains 'report' -and -not (@($mem.done) -contains 'hubs-plan')) "next.json records every round done, and no hubs round with hubs off ($(@($mem.done) -join ','))"
    $reportTime = (Get-Item (Join-Path $workNext 'grow-report.md')).LastWriteTimeUtc
    $n5 = Next-Line $opts
    Assert ($n5.next -eq 'done' -and $n5.created -eq $n4.created -and (Get-Item (Join-Path $workNext 'grow-report.md')).LastWriteTimeUtc -eq $reportTime) 'next: a rerun of a finished run redoes nothing and answers the same done line'
    $bad = Next-Line '{"mode":"cards"}' -1 (Join-Path $scratch 'work-next-bad')
    Assert ($bad.next -eq 'fault' -and $bad.code -eq 'bad_options') "next: options it does not know are a typed fault ($($bad | ConvertTo-Json -Compress))"

} finally {
    zt down 2>&1 | Out-Null
    # GROW_TEST_KEEP=1 keeps the scratch archive for a look at the files a failure left.
    if ($env:GROW_TEST_KEEP) { Write-Output "kept: $scratch" } else { Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue }
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
