#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\topic-join.tests.ps1
# Drives `zt-digest join` on a scratch archive (issue #109, S22; design §7.3, §11.2, §11.3, §12.3):
# the P5 review of existing Topic pairs with its concrete alternatives (keep both, a crossing through
# an existing Hub level, an allowed agent subtree move), then report-bound applies on that scratch
# archive: one move at a time, a fresh plan compared with the reviewed one, a quiet window, refreshed
# pending addresses, and a read-back of the persisted addresses, Links, Spreads and model report.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "topic-join-test-$PID"
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed' }
$seam = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
# One `zt-digest join` step: its exit code, its output, and (on success) the last line as JSON.
function Invoke-Join([string[]]$Arguments) {
    $text = @(& $script:exe join @Arguments 2>&1 | ForEach-Object { [string]$_ }) -join "`n"
    $code = $LASTEXITCODE
    $last = $null
    if (-not $code) { $last = ($text -split "`n")[-1] | ConvertFrom-Json -AsHashtable -DateKind String }
    [pscustomobject]@{ code = $code; text = $text; last = $last }
}
function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String }
function Write-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 32), $utf8) }
function Call-Tool($tool, $arguments) {
    # `zt-digest call` prints `<index> <json>` for a read and `<index> ok <address>` for a write.
    $line = @(& $script:exe call -Open -Tool $tool -Json ($arguments | ConvertTo-Json -Compress -Depth 10) 2>&1 | ForEach-Object { [string]$_ } | Where-Object { $_ -match '^\d+ ' })
    if (-not $line) { throw "zt-digest call $tool answered nothing" }
    $text = ($line[-1] -split ' ', 2)[1]
    if ($text -match '^ok ') { return @{ ok = $true; address = $text.Substring(3).Trim() } }
    if ($text -match '^error ') { return @{ ok = $false; error = $text.Substring(6).Trim() } }
    $text | ConvertFrom-Json -AsHashtable -DateKind String
}
function Card-At($at) { Call-Tool 'get_card' @{ at = $at } }
function Revision { (Call-Tool 'get_cards' @{ addresses = @(); view = 'text' }).archive_revision }
function Gui-Token {
    $text = @(zt gui)
    $port = [regex]::Match(($text -join "`n"), '127\.0\.0\.1:(\d+)').Groups[1].Value
    $page = (Invoke-WebRequest -Uri "http://127.0.0.1:$port/" -UseBasicParsing).Content
    [pscustomobject]@{ port = $port; token = [regex]::Match($page, 'name="zt-token" content="([^"]+)"').Groups[1].Value }
}
function Gui-Create($body) {
    $answer = Invoke-RestMethod -Uri "http://127.0.0.1:$($script:gui.port)/api/create" -Method Post -Headers @{ 'X-ZT-Token' = $script:gui.token } -ContentType 'application/json' -Body ($body | ConvertTo-Json -Depth 10)
    $answer.address
}
function Pair-Of($report, $root) { @($report.pairs | Where-Object { @($_.a.root, $_.b.root) -contains $root }) }
function Plan-Of($pair, $kind) {
    $move = @($pair.alternatives | Where-Object { $_.action -eq 'move_subtree' })[0]
    @($move.plans | Where-Object { $_.kind -eq $kind })[0]
}

$work = Join-Path $scratch 'join'
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null

    # --- the scratch fixture: five near-duplicate pairs and every boundary ----------------------
    #   A 多重均衡 / B 均衡的多重性          B has the wider source mix (2 books) and a topic hub
    #   A2 多重均衡的机制                     the third Topic of the same subject: A2 joins A, then B
    #   D 学习 / E 学习（产业生产率）         a human Card links D/1, so the service refuses the move
    #   G 均衡选择 / H 均衡的选择            G/1 is a Model Card; moving it alone leaves G/2 behind
    $books = Write-Cards @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{alpha2020, title={学习与社会}, author={Owner, A.}, year={2020}}'; body = '全书索引。' } },
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{beta2021, title={产业学习}, author={Owner, B.}, year={2021}}'; body = '全书索引。' } }
    )
    $alpha = $books[0]
    $beta = $books[1]
    $topics = Write-Cards @(
        @{ tool = 'create_topic'; arguments = @{ title = '多重均衡'; body = '本主题追问：什么条件下同一个经济有多个均衡？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡的多重性'; body = '本主题追问：什么条件下同一个经济有两个均衡？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '学习'; body = '本主题追问：学习怎样改变生产率？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '学习（产业生产率）'; body = '本主题追问：学习怎样提高产业生产率？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡选择'; body = '本主题追问：多个均衡里哪一个会出现？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡的选择'; body = '本主题追问：多个均衡中哪一个被选中？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '多重均衡的机制'; body = '本主题追问：什么条件下同一个经济会出现两个均衡？' } }
    )
    $a = ($topics[0] -replace '/0$', '')
    $b = ($topics[1] -replace '/0$', '')
    $d = ($topics[2] -replace '/0$', '')
    $e = ($topics[3] -replace '/0$', '')
    $g = ($topics[4] -replace '/0$', '')
    $h = ($topics[5] -replace '/0$', '')
    $a2 = ($topics[6] -replace '/0$', '')

    # A: a Model Card with its part, an entry and a Side successor; one book only.
    $aCards = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$a/0"; type = 'model'; title = '模型：预期渠道'; body = "模型卡：预期与学习怎样互相加强。`n`n来源：[[$alpha]] §2" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$a/1"; type = 'model'; facet = '设定'; title = '设定：预期渠道'; body = "预期越高，本期生产越多。`n`n假设：劳动供给有弹性。`n`n来源：[[$alpha]] §2" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$a/2"; title = '本期生产越多第二期产出越高'; body = "学习提高第二期实际工资。`n`n来源：[[$alpha]] §3" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$a/3"; kind = 'side'; title = '两条轨迹相交两次'; body = "两条向上倾斜的轨迹相交两次，得到两个均衡。`n`n来源：[[$alpha]] §4" } }
    )
    # B: the wider source mix (two books), an agent Card that links A/1, and an existing topic hub.
    $bCards = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$b/0"; title = '预期产出越高本期生产越多'; body = "预期越高，本期生产越多。`n`n来源：[[$alpha]] §2 与 [[$beta]] §4" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$b/1"; title = '劳动供给弹性决定均衡个数'; body = "劳动供给没有弹性时均衡唯一。`n`n来源：[[$beta]] §5" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$b/2"; kind = 'side'; title = '协调失灵'; body = "本文对照 [[$a/1]] 的预期渠道，讨论政策含义。`n`n来源：[[$alpha]] §6" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$b/2"; type = 'hub'; title = '枢纽：均衡的多重性'; body = "## 定义`n- [[$b/1]] 给出预期渠道。`n`n## 邻近`n- [[$b/1]] 讲学习的价值。" } }
    )
    # A2: one book, two entries; it joins A and then B.
    $a2Cards = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$a2/0"; title = '两个交点给出两个均衡'; body = "两条轨迹相交两次，得到两个均衡。`n`n来源：[[$alpha]] §4" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$a2/1"; title = '学习价值的正反馈'; body = "预期越高，学习价值越高。`n`n来源：[[$alpha]] §4" } }
    )
    # D: a Model Card with its part; a human Card links D/1, so the move is refused.
    $dCards = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$d/0"; type = 'model'; title = '模型：学习曲线'; body = "模型卡：重复练习与技能。`n`n来源：[[$alpha]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$d/1"; type = 'model'; facet = '设定'; title = '设定：同一个体'; body = "同一个体重复练习，技能提高。`n`n来源：[[$alpha]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$d/2"; title = '产业累计产量降低单位成本'; body = "产业累计产量每翻一倍，单位成本下降。`n`n来源：[[$alpha]] §2" } }
    )
    $eCards = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$e/0"; title = '重复练习改变个体技能'; body = "同一个体重复练习，技能提高。`n`n来源：[[$alpha]] §1 与 [[$beta]] §2" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$e/1"; title = '产业学习降低单位成本'; body = "产业累计产量提高降低单位成本。`n`n来源：[[$beta]] §3" } }
    )
    # G: a Model Card, its part and an entry; moving the head alone would leave the part behind.
    $gCards = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$g/0"; type = 'model'; title = '模型：均衡选择'; body = "模型卡：多个均衡里哪一个出现。`n`n来源：[[$alpha]] §7" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$g/1"; type = 'model'; facet = '设定'; title = '设定：精炼'; body = "精炼排除不合理的均衡。`n`n来源：[[$alpha]] §7" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$g/2"; title = '颤抖手完美'; body = "颤抖手完美再精炼均衡。`n`n来源：[[$alpha]] §8" } }
    )
    $hCards = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$h/0"; title = '颤抖手完美排除不合理均衡'; body = "颤抖手完美再精炼均衡。`n`n来源：[[$alpha]] §8 与 [[$beta]] §9" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$h/1"; title = '精炼与均衡选择'; body = "精炼决定选中哪个均衡。`n`n来源：[[$beta]] §9" } }
    )
    $aRoot = "$a/0"
    $bRoot = "$b/0"
    $a2Root = "$a2/0"
    $dRoot = "$d/0"
    $gRoot = "$g/0"
    $hub = $bCards[3]
    $agentLink = $bCards[2]

    # The human Workspace is Owner-only: its Topic and the Card that links D/1 are created through
    # the GUI, as the owner creates them.
    $script:gui = Gui-Token
    $human = Gui-Create @{ kind = 'topic'; workspace = 'human'; text = "# 多重均衡笔记`n`n本人的读书笔记。" }
    $null = Gui-Create @{ kind = 'direct'; at = $human; text = "# 学习笔记`n`n本人认为 [[$d/1]] 的设定是本书的核心。" }

    # A Spread on the Table follows its Cards through the join (the GUI owns Spreads).
    $spread = @{ current = 'join'; spreads = @(@{ id = 'join'; name = 'Join'; cards = @(
        @{ at = "$a/1"; x = 10; y = 20 },
        @{ at = "$b/1"; x = 30; y = 40 }
    ) }) }
    $null = Invoke-RestMethod -Uri "http://127.0.0.1:$($script:gui.port)/api/spreads" -Method Post -Headers @{ 'X-ZT-Token' = $script:gui.token } -ContentType 'application/json' -Body ($spread | ConvertTo-Json -Depth 10)
    $revisionBefore = Revision

    # --- plan: the P5 review, read-only ---------------------------------------------------------
    $env:ZT_CHECK_ZT = $seam
    $env:ZT_TRACE_TOOLS = Join-Path $scratch 'plan-tools.jsonl'
    [IO.File]::WriteAllText($env:ZT_TRACE_TOOLS, '')
    $plan = Invoke-Join @('-Step', 'plan', '-Work', $work)
    Remove-Item Env:ZT_TRACE_TOOLS
    Remove-Item Env:ZT_CHECK_ZT
    Assert ($plan.code -eq 0) "plan runs: $($plan.text)"
    $report = Read-Json (Join-Path $work 'topic-join-review.json')
    $markdown = Get-Content -LiteralPath (Join-Path $work 'topic-join-review.md') -Raw -Encoding utf8
    Assert ($report.report_id -match '^tj-[0-9a-f]{64}$' -and $report.kind -eq 'topic-join-review' -and $report.archive.id -match '^[0-9a-f]{32}$') 'the review binds its kind, archive identity and canonical report id'
    Assert ($report.archive.revision -eq $revisionBefore -and (Revision) -eq $revisionBefore) 'the plan step changes no Card: the archive revision is unchanged'
    $trace = @(Get-Content -LiteralPath (Join-Path $scratch 'plan-tools.jsonl') -Encoding utf8 | Where-Object { $_ } | ForEach-Object { $_ | ConvertFrom-Json })
    $writes = @($trace | Where-Object { $_.tool -in @('edit_card', 'create_topic', 'create_successor', 'create_literature', 'delete_card', 'insert_card') })
    Assert ($writes.Count -eq 0) 'the plan makes no write call'
    Assert (@($trace | Where-Object { $_.tool -eq 'move_card' }).Count -ge 5) 'the plan previews real move plans for the enumerated entry chains'
    Assert (@($report.pairs).Count -eq 5) "the review finds the five near-duplicate pairs ($(@($report.pairs).Count))"
    $pair = @(Pair-Of $report $aRoot)[0]
    Assert ($null -ne $pair -and $pair.keeper -eq $bRoot -and $pair.keeper_reason -eq 'books') 'the keeper is the home with the wider source mix, not the smaller tree'
    Assert ($pair.keeper_note -match 'book\(s\)' -and $pair.keeper_note -match 'wider source mix') 'the keeper note states the source-mix evidence'
    Assert (@($pair.alternatives | Where-Object { $_.action -eq 'keep_distinct' }).Count -eq 1 -and @($pair.alternatives | Where-Object { $_.action -eq 'keep_both_with_crossing' }).Count -eq 1) 'the pair offers keep-distinct and keep-both-with-crossing'
    $crossing = @($pair.alternatives | Where-Object { $_.action -eq 'keep_both_with_crossing' })[0]
    Assert ($crossing.level -eq 'topic' -and $crossing.hub -eq $hub) 'the crossing uses the existing topic hub at its level'
    Assert (@($pair.a.books) -contains $alpha -and @($pair.b.books) -contains $beta -and @($pair.source_mix.only_b) -contains $beta) 'the pair lists each Topic meaning and the source mix it read'
    Assert ($pair.a.question -match '什么条件' -and $pair.b.question -match '什么条件' -and @($pair.a.entries).Count -ge 1 -and @($pair.a.entries | Where-Object { $_.statement }).Count -ge 1) 'the pair lists both full questions and the read entries with their statements'
    $whole = Plan-Of $pair 'whole_chain'
    Assert ($whole.status -eq 'offered') 'the whole entry chain is offered as one allowed agent subtree move'
    $operation = @($whole.operations)[0]
    Assert ($null -ne $operation -and $operation.from -eq "$a/1" -and @($operation.moved).Count -eq 4) 'the operation enumerates every moved Card of the entry chain and its Side successor'
    Assert (@($operation.mapping | Where-Object { $_[0] -eq "$a/1" }).Count -eq 1 -and $operation.to -eq "$($pair.joining.address)|a") 'the plan lists every mapped address and the side slot under the joining Card'
    Assert (@($operation.mapping | Where-Object { $_[0] -eq "$a/2" })[0][1] -eq "$($operation.to)|1" -and @($operation.mapping | Where-Object { $_[0] -eq "$a/3" })[0][1] -eq "$($operation.to)|2") 'the Direct-successor continuation is enumerated as every address change'
    $rootsMoved = @($pair.alternatives | Where-Object { $_.action -eq 'move_subtree' } | ForEach-Object { $_.plans } | ForEach-Object { $_.operations } | ForEach-Object { $_.from } | Where-Object { $_ -match '/0$' })
    Assert ($rootsMoved.Count -eq 0) 'a Topic root is never an operation: only entry chains are enumerated'
    Assert (@($pair.alternatives | Where-Object { $_.action -eq 'move_subtree' })[0].plans.Count -ge 2) 'each movable entry chain is enumerated as its own plan'
    Assert ($operation.links_rewritten -ge 1 -and $operation.model_effect.preserved) 'the plan counts the Link rewrite and preserves the finished model'
    Assert (@($report.unread).Count -eq 0 -and $report.status -eq 'complete') 'the review is complete over a readable archive'
    Assert ($markdown -match 'keep_distinct' -and $markdown -match 'move_subtree' -and $markdown -match 'wider source mix') 'the rendered review names the alternatives and the keeper evidence'
    Assert (@($report.endpoints.Keys | Where-Object { $_ -match '^0/0$' }).Count -eq 1 -and (Get-ChildItem (Join-Path $work 'endpoints') -File).Count -ge 5) 'every read Card is retained as an endpoint body with its authored fingerprint'

    # The plan is immutable for one unchanged archive: a second run publishes the same id.
    $again = Invoke-Join @('-Step', 'plan', '-Work', $work)
    Assert ($again.code -eq 0 -and $again.last.report_id -eq $report.report_id -and -not $again.last.written) 'the same archive and bounds give the same report id and change no file'

    # --- the bounds are reported, never hidden ---------------------------------------------------
    $boundedWork = Join-Path $scratch 'bounded'
    $bounded = Invoke-Join @('-Step', 'plan', '-Work', $boundedWork, '-MaxPairs', '1', '-MaxOperations', '2')
    Assert ($bounded.code -eq 0 -and $bounded.last.pairs_selected -eq 1 -and $bounded.last.pairs_unselected -ge 1) 'a bounded review reports the pairs it did not select'
    $boundedReport = Read-Json (Join-Path $boundedWork 'topic-join-review.json')
    $boundedPlans = @(@($boundedReport.pairs)[0].alternatives | Where-Object { $_.action -eq 'move_subtree' })[0].plans
    Assert (@($boundedPlans).Count -eq 3) "the operation bound caps the enumerated entry chains ($(@($boundedPlans).Count))"
    Assert ($boundedReport.bounds.pairs -eq 1 -and $boundedReport.bounds.operations -eq 2 -and $boundedReport.bounds.plan_cards -eq 120) 'the review states its effective bounds'

    # --- an unreadable endpoint keeps the review partial, never clean ---------------------------
    $partialWork = Join-Path $scratch 'partial'
    $env:ZT_CHECK_ZT = $seam
    $env:ZT_BULK_MISSING = $aRoot
    try {
        $partialPlan = Invoke-Join @('-Step', 'plan', '-Work', $partialWork)
        Assert ($partialPlan.code -eq 0 -and $partialPlan.last.unread -ge 1 -and $partialPlan.last.status -eq 'partial') 'a missed read makes the review partial'
        $partialReport = Read-Json (Join-Path $partialWork 'topic-join-review.json')
        Assert (@($partialReport.unread | Where-Object { $_.address -eq $aRoot }).Count -eq 1) 'the partial review names the address it could not read'
        $partialPair = @(Pair-Of $partialReport $aRoot)[0]
        Assert ($partialPair.status -eq 'partial' -and @($partialPair.reasons).Count -ge 1) 'the pair over the missed read states its incomplete evidence'
    } finally { Remove-Item Env:ZT_CHECK_ZT, Env:ZT_BULK_MISSING -ErrorAction SilentlyContinue }

    # --- the refused plans preserve the boundary -------------------------------------------------
    $dPair = @(Pair-Of $report $dRoot)[0]
    $dWhole = Plan-Of $dPair 'whole_chain'
    Assert ($dWhole.status -eq 'refused' -and $dWhole.reason.code -eq 'human_link' -and $dWhole.reason.detail -match 'read-only for AI agents') 'a human Link rewrite is a refused plan, named as policy'
    $dCrossing = @($dPair.alternatives | Where-Object { $_.action -eq 'keep_both_with_crossing' })[0]
    Assert ($null -ne $dCrossing.description) 'the refused move keeps its nonmoving alternative'
    $gPair = @(Pair-Of $report $gRoot)[0]
    $gLone = Plan-Of $gPair 'lone_entry'
    Assert ($gLone.status -eq 'refused' -and $gLone.reason.code -eq 'model_boundary') 'moving a Model Card alone leaves its part behind and is refused as a model boundary'
    $gWhole = Plan-Of $gPair 'whole_chain'
    Assert ($gWhole.status -eq 'offered' -and $gWhole.operations[0].model_effect.preserved) 'moving the whole model keeps its membership and is offered'

    # A selection cannot pick a refused plan or an unknown finding.
    $bad = Join-Path $work 'bad-selection.json'
    Write-Json $bad @{ schema_version = 1; report_id = $report.report_id; decisions = @(@{ finding = $dPair.id; action = 'move_subtree'; plan = $dWhole.id; note = '人写的。' }) }
    $refusedPlan = Invoke-Join @('-Step', 'apply', '-Work', $work, '-File', $bad)
    Assert ($refusedPlan.code -ne 0 -and $refusedPlan.text -match 'select the nonmoving alternative') 'apply refuses a reviewed plan the report refused'
    Write-Json $bad @{ schema_version = 1; report_id = $report.report_id; decisions = @(@{ finding = 't999'; action = 'keep_distinct'; note = 'x' }) }
    $unknown = Invoke-Join @('-Step', 'apply', '-Work', $work, '-File', $bad)
    Assert ($unknown.code -ne 0 -and $unknown.text -match 'does not hold') 'apply refuses a finding the report does not hold'

    # --- a keep-distinct decision writes nothing and stays visible -------------------------------
    $dNonmoving = Join-Path $work 'nonmoving.json'
    Write-Json $dNonmoving @{ schema_version = 1; report_id = $report.report_id; decisions = @(@{ finding = $dPair.id; action = 'keep_both_with_crossing'; note = '两条线各有价值，保留并以主题入口相连。' }) }
    $keep = Invoke-Join @('-Step', 'apply', '-Work', $work, '-File', $dNonmoving)
    Assert ($keep.code -eq 0 -and $keep.last.status -eq 'unchanged' -and @($keep.last.counts.unchanged) -eq 1) 'a nonmoving resolution is unchanged and writes no Card'
    $keepResult = Read-Json (Join-Path $work 'join-result.json')
    Assert (@($keepResult.applied).Count -eq 0 -and $keepResult.items[0].detail -match 'writes no Card') 'the nonmoving result retains its alternative without a write'

    # --- a stale destination is a conflict, not a silent replan ---------------------------------
    $joining = $pair.joining.address
    $destination = $operation.to
    $selection = Join-Path $work 'join-selection.json'
    Write-Json $selection @{ schema_version = 1; report_id = $report.report_id; decisions = @(@{ finding = $pair.id; action = 'move_subtree'; plan = $whole.id; note = '归并这条孤立的入口链。'; fingerprints = @{ "$a/1" = $operation.fingerprints[0][1] } }) }
    $temp = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = $joining; kind = 'side'; title = '并行写作者占了这个位置'; body = '另一个用户独立创建的卡片。' } }))[0]
    Assert ($temp -eq $destination) "the reserved slot is the reviewed destination ($temp)"
    $stale = Invoke-Join @('-Step', 'apply', '-Work', $work, '-File', $selection)
    Assert ($stale.code -eq 0 -and $stale.last.status -eq 'conflict' -and @($stale.last.counts.conflict) -eq 1) 'a destination another writer took is a conflict'
    $staleResult = Read-Json (Join-Path $work 'join-result.json')
    Assert ($staleResult.items[0].operations[0].reason.code -eq 'destination_taken' -and $staleResult.complete -eq $false) 'the stale plan is named and the result is incomplete'
    Assert (@($staleResult.applied).Count -eq 0) 'a conflict applies no move'
    $removed = Call-Tool 'delete_card' @{ at = $temp; confirm = 'delete' }
    Assert ($removed.ok) 'the reserved slot is released for the reviewed plan'

    # A reviewed fingerprint that no longer matches is a conflict before any move.
    $a1 = Card-At "$a/1"
    $selection2 = Join-Path $work 'selection2.json'
    Write-Json $selection2 @{ schema_version = 1; report_id = $report.report_id; decisions = @(@{ finding = $pair.id; action = 'move_subtree'; plan = $whole.id; note = '归并这条孤立的入口链。' }) }
    $null = Call-Tool 'edit_card' @{ at = "$a/1"; body = "$($a1.body)`n`n另一位写作者在复核之后改了这段。" }
    $changed = Invoke-Join @('-Step', 'apply', '-Work', $work, '-File', $selection2)
    $changedResult = Read-Json (Join-Path $work 'join-result.json')
    Assert ($changed.code -eq 0 -and $changedResult.items[0].operations[0].reason.code -eq 'endpoint_changed') 'an edited endpoint after the review is a conflict, not permission to move it'
    $restored = Call-Tool 'edit_card' @{ at = "$a/1"; body = $a1.body }
    Assert ($restored.ok) 'the reviewed text is restored on the scratch archive'

    # A new Card inside the moving subtree changes the plan: the fresh plan is compared with the
    # reviewed one and the difference is a conflict, never a silent replan (D269).
    $newMember = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = "$a/2"; kind = 'side'; title = '复核之后新增的分支'; body = "这段在复核之后加入。`n`n来源：[[$alpha]] §5" } }))[0]
    $planChanged = Invoke-Join @('-Step', 'apply', '-Work', $work, '-File', $selection2)
    $planChangedResult = Read-Json (Join-Path $work 'join-result.json')
    Assert ($planChanged.code -eq 0 -and $planChangedResult.items[0].operations[0].reason.code -eq 'plan_changed') "a fresh plan that differs from the reviewed one is a conflict ($($planChangedResult.items[0].operations[0].reason.code): $($planChangedResult.items[0].operations[0].reason.detail))"
    Assert (@($planChangedResult.applied).Count -eq 0 -and $planChangedResult.complete -eq $false) 'the changed plan applies no move'
    $newMemberGone = Call-Tool 'delete_card' @{ at = $newMember; confirm = 'delete' }
    Assert ($newMemberGone.ok) 'the new branch is removed on the scratch archive'

    # The Relation kind of a rewritten Link is preserved, so a move does not silently reword it.
    $kindBefore = (Call-Tool 'get_cards' @{ addresses = @($agentLink); view = 'text' }).cards[0].kinds["$a/1"]
    Assert ($null -ne $kindBefore) 'the agent Link states its Relation kind before the join'

    # --- a concurrent writer stops the automated move -------------------------------------------
    $twoJoins = Join-Path $work 'two-joins.json'
    $a2Pair = @(@(Pair-Of $report $a2Root) | Where-Object { $_.keeper -eq $aRoot })[0]
    $a2ToB = @(@(Pair-Of $report $a2Root) | Where-Object { $_.keeper -eq $bRoot })[0]
    $a2ToA = Plan-Of $a2Pair 'whole_chain'
    $a2IntoB = Plan-Of $a2ToB 'whole_chain'
    Write-Json $twoJoins @{ schema_version = 1; report_id = $report.report_id; decisions = @(
        @{ finding = $a2Pair.id; action = 'move_subtree'; plan = $a2ToA.id; note = '先把机制线并入同一主题。' },
        @{ finding = $a2ToB.id; action = 'move_subtree'; plan = $a2IntoB.id; note = '再把机制线并入多重均衡主题。' }
    ) }
    $env:ZT_CHECK_ZT = $seam
    $env:ZT_RACE_AFTER_BULK_REPLY = '1'
    $env:ZT_RACE_TARGET = "$h/1"
    $env:ZT_RACE_LINE = '另一位写作者在迁移窗口里改了这一行。'
    $env:ZT_RACE_FLAG = Join-Path $scratch 'race.flag'
    try {
        $raced = Invoke-Join @('-Step', 'apply', '-Work', $work, '-File', $twoJoins)
        Assert ($raced.code -eq 0 -and $raced.last.status -eq 'pending' -and @($raced.last.counts.pending) -eq 2) 'a writer inside the interval leaves every selected move pending'
        $racedResult = Read-Json (Join-Path $work 'join-result.json')
        Assert ($racedResult.items[0].operations[0].reason.code -eq 'quiet_window_required' -and $racedResult.items[0].operations[0].reason.detail -match 'revision') 'the pending item requests a quiet migration window and names the revision change'
        Assert (@($racedResult.pending_operations).Count -eq 2) 'every unapplied operation is retained as a pending item'
        Assert (@($racedResult.applied).Count -eq 0) 'the pending run applied no move'
    } finally { Remove-Item Env:ZT_CHECK_ZT, Env:ZT_RACE_AFTER_BULK_REPLY, Env:ZT_RACE_TARGET, Env:ZT_RACE_LINE, Env:ZT_RACE_FLAG -ErrorAction SilentlyContinue }

    # --- sequential joins: one move at a time, with refreshed pending addresses ------------------
    $sequential = Invoke-Join @('-Step', 'apply', '-Work', $work, '-File', $twoJoins)
    Assert ($sequential.code -eq 0 -and $sequential.last.status -eq 'applied' -and $sequential.last.complete) "the two sequential joins apply: $($sequential.text)"
    $sequence = Read-Json (Join-Path $work 'join-result.json')
    Assert (@($sequence.items | Where-Object { $_.status -eq 'applied-and-verified' }).Count -eq 2) 'both findings are applied-and-verified after read-back'
    $firstMove = $sequence.items[0].operations[0]
    $secondMove = $sequence.items[1].operations[0]
    Assert ($firstMove.status -eq 'applied-and-verified' -and $secondMove.status -eq 'applied-and-verified') 'each move is verified on its own read-back'
    Assert ($secondMove.reviewed.from -eq "$a2/1" -and $secondMove.from -ne "$a2/1") 'the second move used the refreshed source address of the first move'
    Assert (@($sequence.applied | Where-Object { $_[0] -eq $secondMove.from }).Count -eq 1 -and (Card-At $secondMove.to).address -eq $secondMove.to) 'the refreshed address is a real applied mapping and the target Card exists'
    Assert ($sequence.broken_links.before -eq $sequence.broken_links.after) 'the sequential joins add no Broken link'

    # --- after other accepted moves, the first review's A/B plan is stale ------------------------
    $lateTemp = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = $joining; kind = 'side'; title = '后来的卡片占了这个位置'; body = '迁移窗口之后的独立创建。' } }))[0]
    Assert ($lateTemp -eq $destination) "the reviewed slot is taken again by a later writer ($lateTemp)"
    $staleReview = Invoke-Join @('-Step', 'apply', '-Work', $work, '-File', $selection2)
    $staleReviewResult = Read-Json (Join-Path $work 'join-result.json')
    Assert ($staleReview.code -eq 0 -and $staleReviewResult.status -eq 'conflict' -and $staleReviewResult.items[0].operations[0].reason.code -eq 'destination_taken') 'the stale review is a conflict, not a silent replan (D301)'
    Assert (@($staleReviewResult.applied).Count -eq 0) 'the stale review applies no move'
    $lateRemoved = Call-Tool 'delete_card' @{ at = $lateTemp; confirm = 'delete' }
    Assert ($lateRemoved.ok) 'the later Card is removed on the scratch archive'

    # --- a fresh review, then the accepted A/B join ---------------------------------------------
    $replanned = Invoke-Join @('-Step', 'plan', '-Work', $work)
    $report2 = Read-Json (Join-Path $work 'topic-join-review.json')
    Assert ($replanned.code -eq 0 -and $report2.report_id -ne $report.report_id -and $report2.archive.revision -gt $report.archive.revision) 'the regenerated review binds the new archive revision and a new id'
    $pair2 = @(@(Pair-Of $report2 $aRoot) | Where-Object { $_.keeper -eq $bRoot })[0]
    $whole2 = Plan-Of $pair2 'whole_chain'
    Assert ($whole2.status -eq 'offered' -and @($whole2.operations[0].moved).Count -eq 4) 'the fresh review plans the A/B join on the current Cards'
    $selection3 = Join-Path $work 'selection3.json'
    Write-Json $selection3 @{ schema_version = 1; report_id = $report2.report_id; decisions = @(@{ finding = $pair2.id; action = 'move_subtree'; plan = $whole2.id; note = '把预期渠道线并入已有的多重均衡主题。' }) }
    $applied = Invoke-Join @('-Step', 'apply', '-Work', $work, '-File', $selection3)
    Assert ($applied.code -eq 0 -and $applied.last.status -eq 'applied' -and $applied.last.complete) "the accepted A/B plan applies: $($applied.text)"
    $result = Read-Json (Join-Path $work 'join-result.json')
    Assert (@($result.applied).Count -eq 4 -and @($result.rollback).Count -eq 4) 'every applied mapping is retained with its inverse as rollback evidence'
    Assert ($result.items[0].operations[0].status -eq 'applied-and-verified' -and $result.items[0].operations[0].verification.ok) 'the operation is applied-and-verified after read-back'
    Assert ($result.broken_links.before -eq $result.broken_links.after) 'the join adds no Broken link'
    $newRoot = @($result.applied | Where-Object { $_[0] -eq "$a/1" })[0][1]
    Assert ($newRoot -eq $whole2.operations[0].to) "the entry chain lands on the reviewed slot ($newRoot)"
    Assert ($null -eq (Card-At "$a/1") -or (Card-At "$a/1").address -ne "$a/1") 'the old entry address no longer holds the moved Card'
    $landed = Card-At $newRoot
    Assert ($landed.address -eq $newRoot -and $landed.body -match '预期与学习怎样互相加强' -and $landed.title -match '模型：预期渠道') 'the moved Card keeps its authored title and body at the new address'
    $part = @($result.applied | Where-Object { $_[0] -eq "$a/2" })[0][1]
    Assert ((Card-At $part).body -match '假设：劳动供给有弹性') 'the Model part moved with its model and keeps its text'
    $modelReport = Call-Tool 'get_model' @{ at = $newRoot }
    Assert ($null -ne $modelReport.parts -and @($modelReport.parts).Count -ge 1) 'the moved model still reports its parts after the join'
    $agentLinkAfter = Card-At $agentLink
    Assert ($agentLinkAfter.body -match [regex]::Escape($newRoot) -and $agentLinkAfter.body -notmatch [regex]::Escape("[[$a/1]]")) 'an agent Link into the moving chain was rewritten by the move, not broken'
    $kindAfter = (Call-Tool 'get_cards' @{ addresses = @($agentLink); view = 'text' }).cards[0].kinds[$newRoot]
    Assert ($kindAfter -eq $kindBefore) "the rewritten Link keeps its Relation kind ($kindAfter)"
    $spreads = Invoke-RestMethod -Uri "http://127.0.0.1:$($script:gui.port)/api/spreads" -Headers @{ 'X-ZT-Token' = $script:gui.token }
    $spreadCards = @($spreads.spreads | Where-Object { $_.id -eq 'join' })[0].cards
    Assert (@($spreadCards | Where-Object { $_.at -eq $newRoot }).Count -eq 1) 'the saved Spread followed the moved Card'

    # --- replay: the same selection returns the recorded result and writes nothing ---------------
    $before = (Get-FileHash -LiteralPath (Join-Path $work 'join-result.json') -Algorithm SHA256).Hash
    $replay = Invoke-Join @('-Step', 'apply', '-Work', $work, '-File', $selection3)
    Assert ($replay.code -eq 0 -and $replay.last.replayed -and $replay.last.result_id -eq $result.result_id) 'the same selection replays its recorded result'
    Assert ((Get-FileHash -LiteralPath (Join-Path $work 'join-result.json') -Algorithm SHA256).Hash -eq $before) 'the replay changes no result byte'

    # --- an edited report is refused by its own id ----------------------------------------------
    $reportPath = Join-Path $work 'topic-join-review.json'
    $saved = [IO.File]::ReadAllBytes($reportPath)
    [IO.File]::WriteAllText($reportPath, [Text.Encoding]::UTF8.GetString($saved).Replace('多重均衡', '多重均衡X'), $utf8)
    $tampered = Invoke-Join @('-Step', 'apply', '-Work', $work, '-File', $selection3)
    Assert ($tampered.code -ne 0 -and $tampered.text -match 'does not match its own report id') 'an edited report is refused, so no apply consumes stale text'
    [IO.File]::WriteAllBytes($reportPath, $saved)
} finally {
    Remove-Item Env:ZT_CHECK_ZT, Env:ZT_TRACE_TOOLS -ErrorAction SilentlyContinue
    zt down | Out-Null
}
if ($failed) { Write-Output "$failed assertion(s) failed"; exit 1 }
Write-Output 'topic-join: all assertions passed'
