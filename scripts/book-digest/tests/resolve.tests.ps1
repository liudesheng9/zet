#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\resolve.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# Issue #122, S29 (design §15-phase-6, D377/D378): the separately selected historical resolution
# batch. `zt-digest resolve -Step plan` turns the exact items a person selected out of the reports
# `topic-home` and `join` already produced into one batch, binds the archive identity, the current
# address and authored fingerprint of every Card it touches, the exact texts or operations, the
# earlier-carrier treatment and the approved backup, and leaves every unselected item visibly
# deferred. `-Step apply` refuses a deferred batch, re-verifies the backup and the protected scope,
# executes only the accepted items through those qualified paths and reports each one; `-Step check`
# re-reads the archive and verifies what the batch claims. A real scratch archive; the live archive
# is not touched.
param([string]$Zt = '', [string]$Digest = '')
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "resolve-test-$PID-$([guid]::NewGuid().ToString('n').Substring(0, 8))"
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
if (-not $Zt) {
    $installed = (Get-Command zt -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    $Zt = if ((Test-Path 'D:\zt-target\debug\zt.exe') -and -not $installed) { 'D:\zt-target\debug\zt.exe' } else { $installed }
}
if (-not $Zt) { $Zt = (Get-Command zt -ErrorAction Stop).Source }
# The named build serves this scratch archive: it is what the tool's own `zt mcp` resolves, too.
$env:PATH = "$(Split-Path $Zt);$env:PATH"
if (-not $Digest) { $Digest = $env:ZT_DIGEST }
if (-not $Digest) {
    # The installed tool is what the release deploys; the built one is the fallback for a developer
    # whose install is behind this checkout.
    $installedDigest = (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    $built = Join-Path $PSScriptRoot '..\..\..\tools\zt-digest\target\debug\zt-digest.exe'
    $Digest = if ($installedDigest) { $installedDigest } elseif (Test-Path $built) { $built } else { '' }
}
if (-not $Digest) { throw 'zt-digest is not installed' }
$env:ZT_DIGEST = $Digest
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
# One `zt-digest <tool>` step: its exit code, its output and (on a successful step) its last line.
function Invoke-Tool([string]$tool, [string[]]$Arguments) {
    $text = @(& $script:Digest $tool @Arguments 2>&1 | ForEach-Object { [string]$_ }) -join "`n"
    $code = $LASTEXITCODE
    $last = $null
    if (-not $code) { $last = ($text -split "`n")[-1] | ConvertFrom-Json -AsHashtable -DateKind String }
    [pscustomobject]@{ code = $code; text = $text; last = $last }
}
function Resolve-Step([string[]]$Arguments) { Invoke-Tool 'resolve' $Arguments }
function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String }
function Write-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 32), $utf8) }
function Call-Tool($tool, $arguments) {
    $line = @(& $script:Digest call -Open -Tool $tool -Json ($arguments | ConvertTo-Json -Compress -Depth 10) 2>&1 | ForEach-Object { [string]$_ } | Where-Object { $_ -match '^\d+ ' })
    if (-not $line) { throw "zt-digest call $tool answered nothing" }
    $text = ($line[-1] -split ' ', 2)[1]
    if ($text -match '^ok ') { return @{ ok = $true; address = $text.Substring(3).Trim() } }
    if ($text -match '^error ') { return @{ ok = $false; error = $text.Substring(6).Trim() } }
    $text | ConvertFrom-Json -AsHashtable -DateKind String
}
function Card-At($at) { Call-Tool 'get_card' @{ at = $at } }
function Tree-Cards($tree) { (Call-Tool 'get_tree' @{ tree = $tree }).cards }
function Revision { (Call-Tool 'get_cards' @{ addresses = @(); view = 'text' }).archive_revision }
function Transfer-Of($batch, $entry) { @($batch.items | Where-Object { $_.kind -eq 'transfer' -and $_.operation.entry -eq $entry })[0] }
function Item-Of($batch, $id) { @($batch.items | Where-Object { $_.id -eq $id })[0] }
# The verified backup the owner takes before an approved write: the service is stopped so the copy
# is one coherent SQLite file, and the record names its exact bytes.
function New-Backup {
    zt down | Out-Null
    $path = Join-Path $scratch "backup-$([guid]::NewGuid().ToString('n').Substring(0, 8)).sqlite3"
    Copy-Item -LiteralPath (Join-Path $scratch 'archive\zt.sqlite3') -Destination $path -Force
    zt up | Out-Null
    [ordered]@{
        path = $path
        sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        bytes = (Get-Item -LiteralPath $path).Length
    }
}

$staging = Join-Path $scratch 'staging'
$work = Join-Path $scratch 'work'
New-Item -ItemType Directory -Force $work, $staging, (Join-Path $work 'endpoints') | Out-Null
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null

    # --- the fixture: two books, one Literature tree, the keeper Topic and a near-duplicate -----
    $books = Write-Cards @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{alpha2020, title={学习与社会}, author={Owner, A.}, year={2020}}'; body = '全书索引。' } },
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{beta2021, title={产业学习}, author={Owner, B.}, year={2021}}'; body = '全书索引。' } }
    )
    $alpha = $books[0]
    $beta = $books[1]
    $bookCards = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = $alpha; title = '第 1 组：均衡与学习'; body = '§1 - 概念' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$alpha/1"; title = '第 2 组：学习与生产率'; body = '§2 - 概念' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$alpha/2"; type = 'model'; title = '模型：预期渠道'; body = "模型卡：预期与学习互相加强。`n`n来源：[[$alpha]] §2" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$alpha/3"; title = '第 4 组：政策'; body = '§4 - 概念' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$alpha/4"; title = '第 5 组：制度'; body = '§5 - 概念' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$alpha/2"; kind = 'side'; title = '产业累计产量降低单位成本'; body = "产业累计产量每翻一倍，单位成本按固定比例下降。`n`n来源：[[$alpha]] §4.2" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$alpha/3"; kind = 'side'; type = 'model'; facet = '设定'; title = '设定：预期渠道'; body = "预期越高本期生产越多。`n`n来源：[[$alpha]] §2" } }
    )
    $carrier = $bookCards[5]
    $anchor1 = "$alpha/1"
    $anchor2 = "$alpha/2"
    $anchor4 = "$alpha/4"
    $part = "$alpha/3|a"
    $table = @'
本卡是《学习与社会》的术语表。本书的卡片用左列的术语，不用右列的词。

术语表

| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 均衡的多重性 | 多重均衡 | 同一个经济在给定条件下停在多个均衡的可能性 | §2.1 |
| 预期渠道 | 预期效应 | 预期改变本期生产的传导 | §2.2 |

来源：[[alpha2020]]
'@
    $null = Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = $anchor4; kind = 'side'; title = '术语表：《学习与社会》的术语'; body = $table } })

    $topics = Write-Cards @(
        @{ tool = 'create_topic'; arguments = @{ title = '均衡的多重性'; body = '本主题追问：什么条件下同一个经济有多个均衡？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '多重均衡'; body = '本主题追问：什么条件下同一个经济有两个均衡？' } }
    )
    $topicA = ($topics[0] -replace '/0$', '')
    $dup = ($topics[1] -replace '/0$', '')
    $entries = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/0"; title = '对称参与者的重复博弈有两个均衡'; body = "当策略互补且参与者对称时，`$x_i=\arg\max_x u_i(x,x_{-i})`$ 有两个均衡。`n`n假设：策略互补、参与者对称。`n`n来源：[[$anchor1]] §2.1 与 [[$alpha/3]]" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/1"; type = 'model'; title = '模型：协调失灵'; body = "模型卡：协调失灵让一个均衡持续。`n`n来源：[[$alpha]] §3" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/2"; title = '没有来源的判断'; body = '本卡没有来源行，因此不能用一张文献卡证明内容。' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/3"; title = '入口卡：产业学习的成本效应'; body = "## 同述入口`n`n- [[$carrier]] 见上。`n`n来源：[[$alpha]] §4.2" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/4"; title = '产业累计产量降低单位成本'; body = "产业累计产量每翻一倍，单位成本按固定比例下降。`n`n来源：[[$alpha]] §4.2" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/5"; title = '预期渠道让政策有短期效果'; body = "预期渠道让政策在短期有效。`n`n假设：劳动供给有弹性。`n`n来源：[[$anchor2]] §2.2" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/6"; title = '政策可信时预期渠道更强'; body = "政策可信时预期渠道更强。`n`n假设：政策有承诺。`n`n来源：[[$alpha/4]] §4.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/7"; title = '产业学习把成本优势固定下来'; body = "产业学习把成本优势固定下来。`n`n来源：[[$beta]] §3" } }
    )
    $a1 = $entries[0]
    $a5 = $entries[4]
    $a6 = $entries[5]
    $a7 = $entries[6]
    # The near-duplicate Topic: one sourced entry only, so the keeper is the Topic whose entries
    # state two sources. The second entry states no source, so it is not a transferable statement.
    $dupCards = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$dup/0"; title = '两个交点给出两个均衡'; body = "两条轨迹相交两次，得到两个均衡。`n`n来源：[[$alpha]] §4" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$dup/1"; title = '策略互补放大冲击'; body = '策略互补放大冲击，因此多个均衡可能同时存在。' } }
    )
    $dup1 = $dupCards[0]
    $null = Call-Tool 'edit_card' @{ at = "$topicA/0"; body = "$((Card-At "$topicA/0").body)`n`n入口：依赖 [[$a1]] 的模型。" }

    # A human Card links the entry a transfer rewrites, and a finished model sits beside the Target.
    $guiPort = [regex]::Match((@(zt gui) -join "`n"), '127\.0\.0\.1:(\d+)').Groups[1].Value
    $token = [regex]::Match((Invoke-WebRequest -Uri "http://127.0.0.1:$guiPort/" -UseBasicParsing).Content, 'name="zt-token" content="([^"]+)"').Groups[1].Value
    $script:gui = [pscustomobject]@{ port = $guiPort; token = $token }
    function Gui-Create($body) {
        (Invoke-RestMethod -Uri "http://127.0.0.1:$($script:gui.port)/api/create" -Method Post -Headers @{ 'X-ZT-Token' = $script:gui.token } -ContentType 'application/json' -Body ($body | ConvertTo-Json -Depth 10)).address
    }
    $humanTopic = Gui-Create @{ kind = 'topic'; workspace = 'human'; text = "# 多重均衡笔记`n`n本人的读书笔记。" }
    $humanCard = Gui-Create @{ kind = 'direct'; at = $humanTopic; text = "# 本人笔记`n`n本人认为 [[$a1]] 是本书的核心。" }
    $humanBefore = Card-At $humanCard
    $humanTreeBefore = @(Tree-Cards ($humanTopic -replace '/0$', '')).Count
    $modelBefore = (Call-Tool 'get_model' @{ at = "$alpha/3" })

    # --- the reports the two qualified paths produced -------------------------------------------
    Write-Json (Join-Path $staging 'manifest.json') @{ citation_key = $alpha; book_title = '学习与社会'; groups = @(@{ n = 1; chapter = '第一章'; headings = @('均衡') }) }
    New-Item -ItemType Directory -Force (Join-Path $staging 'concepts\out') | Out-Null
    Write-Json (Join-Path $staging 'concepts\proposals.json') @{ schema_version = 1; proposals = @(
            @{ title = '均衡的多重性'; question = '什么条件下同一个经济有多个均衡？'
                statement = '当策略互补时，同一个经济可以有多个均衡：$x_i=\arg\max_x u_i(x,x_{-i})$。'
                assumptions = @('策略互补'); terms = @('均衡的多重性') }
        ) }
    $prepare = Invoke-Tool 'concepts' @('-Step', 'prepare', '-Staging', $staging)
    Assert ($prepare.code -eq 0) "concepts prepare runs: $($prepare.text)"
    $evidence = Read-Json (Join-Path $staging 'concepts\evidence.json')
    $judgments = @()
    foreach ($c in @(@($evidence.proposals)[0].topic_candidates)) {
        $rows = @($c.address) + @(@($c.entries) | ForEach-Object { $_.address })
        $judgments += [ordered]@{ kind = 'topic-concept'; proposal = 'p001'; candidate = $c.address; verdict = 'different'
            reason = '两张卡讨论的条件不同。'; evidence = $rows }
    }
    foreach ($s in @(@($evidence.proposals)[0].restatement_candidates)) {
        $judgments += [ordered]@{ kind = 'restatement'; proposal = 'p001'; candidate = $s.address; verdict = 'different'
            reason = '两处陈述的假设不同。'; evidence = @($s.address) }
    }
    New-Item -ItemType Directory -Force (Join-Path $staging 'concepts\out') | Out-Null
    $answer = Join-Path $staging 'concepts\out\cj-01.json'
    Write-Json $answer @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = $judgments }
    $judge = Invoke-Tool 'concepts' @('-Step', 'judge', '-Staging', $staging, '-File', $answer)
    Assert ($judge.code -eq 0) "concepts judge accepts the canned answers: $($judge.text)"
    $report = Invoke-Tool 'concepts' @('-Step', 'report', '-Staging', $staging)
    Assert ($report.code -eq 0) "concepts report writes the review: $($report.text)"
    $homePlan = Invoke-Tool 'topic-home' @('-Step', 'plan', '-Work', $work, '-Report', (Join-Path $staging 'concept-review.json'))
    Assert ($homePlan.code -eq 0) "topic-home plan writes its transfer review: $($homePlan.text)"
    $joinPlan = Invoke-Tool 'join' @('-Step', 'plan', '-Work', $work)
    Assert ($joinPlan.code -eq 0) "join plan writes its Topic-pair review: $($joinPlan.text)"
    $homeReview = Read-Json (Join-Path $work 'topic-home-review.json')
    $joinReview = Read-Json (Join-Path $work 'topic-join-review.json')
    $pair = @($joinReview.pairs | Where-Object { $_.keeper -eq "$topicA/0" -and @($_.a.root, $_.b.root) -contains "$dup/0" })[0]
    Assert ($null -ne $pair) 'the join review pairs the near-duplicate Topic with the keeper Topic'
    $move = @($pair.alternatives | Where-Object { $_.action -eq 'move_subtree' })[0]
    $offeredPlan = @($move.plans | Where-Object { $_.status -eq 'offered' -and $_.kind -eq 'whole_chain' })[0]
    if ($null -eq $offeredPlan) { $offeredPlan = @($move.plans | Where-Object { $_.status -eq 'offered' })[0] }
    Assert ($null -ne $offeredPlan) "the join review offers a move plan for that pair: $($joinPlan.text)"

    # --- plan without a selection: the optional gate is deferred and says so ---------------------
    $revisionBefore = Revision
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_TRACE_TOOLS = Join-Path $scratch 'plan-tools.jsonl'
    [IO.File]::WriteAllText($env:ZT_TRACE_TOOLS, '')
    $deferred = Resolve-Step @('-Step', 'plan', '-Work', $work)
    Remove-Item Env:ZT_CHECK_ZT
    Remove-Item Env:ZT_TRACE_TOOLS
    Assert ($deferred.code -eq 0) "plan runs without a selection: $($deferred.text)"
    $batch = Read-Json (Join-Path $work 'resolution-batch.json')
    Assert ($batch.batch_id -match '^rb-[0-9a-f]{64}$' -and $batch.contract -eq 'resolve/1' -and $batch.kind -eq 'resolution-batch') 'the batch binds its contract and a canonical content id'
    Assert ($batch.status -eq 'deferred' -and $batch.deferred.code -eq 'no-selection') 'a batch with no selection is deferred and says why'
    Assert (@($batch.reports | Where-Object { $_.kind -eq 'topic-home' }).Count -eq 1 -and @($batch.reports | Where-Object { $_.kind -eq 'join' }).Count -eq 1) 'the batch names both produced reports it read, with their ids'
    Assert (@($batch.items | Where-Object { $_.disposition -eq 'selected' }).Count -eq 0) 'nothing is selected without a selection'
    Assert (@($batch.items | Where-Object { $_.kind -eq 'transfer' -and $_.disposition -eq 'deferred' }).Count -ge 3) 'every offered transfer is listed and deferred'
    Assert (@($batch.items | Where-Object { $_.disposition -eq 'ineligible' }).Count -ge 3) 'the items the reports did not offer are listed as ineligible, never selectable'
    $transferItem = Transfer-Of $batch $a1
    Assert ($null -ne $transferItem -and $transferItem.disposition -eq 'deferred' -and $transferItem.reason.code -eq 'awaiting-selection') 'the book-bound entry is an offered transfer candidate and it is deferred'
    Assert ($transferItem.report_id -eq $homeReview.report_id -and $transferItem.finding -and $transferItem.item) 'the candidate binds its report, finding and item ids'
    Assert ($transferItem.operation.statement.body -match [regex]::Escape('$x_i=\arg\max_x u_i(x,x_{-i})$') -and @($transferItem.operation.statement.sources) -contains $anchor1 -and $transferItem.operation.entry_remedy.body -match '## 同述入口') 'the candidate records the exact statement, its source Links and the exact old-Card entry text'
    $entryAddress = @($transferItem.addresses | Where-Object { $_.role -eq 'entry' })[0]
    $slotAddress = @($transferItem.addresses | Where-Object { $_.role -eq 'statement' })[0]
    Assert ($entryAddress.address -eq $a1 -and $entryAddress.exists -and $entryAddress.current_fingerprint -eq $homeReview.endpoints[$a1].fingerprint) 'the candidate records the current authored fingerprint of the Card it rewrites'
    Assert ($entryAddress.reviewed_fingerprint -eq $entryAddress.current_fingerprint) 'the current fingerprint is the one the report reviewed, so the base is not stale'
    Assert ($slotAddress.exists -eq $false -and $slotAddress.expected_free) 'the statement slot is recorded as still free'
    Assert ($transferItem.earlier_carrier.treatment -eq 'create') 'a statement the book does not carry names its earlier-carrier treatment'
    $reuseItem = Transfer-Of $batch $a5
    Assert ($reuseItem.earlier_carrier.treatment -eq 'reuse' -and $reuseItem.earlier_carrier.address -eq $carrier) 'a statement the book already carries is recorded as a reuse of that carrier'
    $blocked = @($batch.items | Where-Object { $_.kind -eq 'transfer' -and $_.disposition -eq 'ineligible' -and $_.reason.code -eq 'model_boundary' })[0]
    Assert ($null -ne $blocked) 'a Model Card entry is ineligible on a named separate model decision'
    $moveItem = @($batch.items | Where-Object { $_.kind -eq 'join' -and $_.plan -eq $offeredPlan.id })[0]
    Assert ($null -ne $moveItem -and $moveItem.action -eq 'move_subtree' -and @($moveItem.operation.operations).Count -ge 1) 'the join review offers its move plan as a batch candidate'
    Assert (@($moveItem.addresses | Where-Object { $_.role -eq 'landing' -and $_.exists -eq $false }).Count -ge 1) 'the move candidate names the free landing slot it would occupy'
    Assert ($batch.backup -eq $null) 'a deferred batch approves no backup'
    Assert ($batch.protected.digest -and $batch.protected.cards -gt 0) 'the batch records the protected scope it read'
    Assert (@($batch.items | Where-Object { @($_.stale).Count -gt 0 }).Count -eq 0) 'a batch planned over an unchanged archive records no stale base'
    Assert ((Revision) -eq $revisionBefore) 'plan writes no Card: the archive revision is unchanged'
    $trace = @(Get-Content -LiteralPath (Join-Path $scratch 'plan-tools.jsonl') -Encoding utf8 | Where-Object { $_ } | ForEach-Object { $_ | ConvertFrom-Json })
    $writes = @($trace | Where-Object { $_.tool -in @('edit_card', 'create_topic', 'create_successor', 'create_literature', 'delete_card', 'insert_card', 'move_card', 'apply_batch') })
    Assert ($writes.Count -eq 0) 'plan makes no write call'
    Assert ((Get-ChildItem (Join-Path $work 'resolve-endpoints') -File).Count -ge 4) 'the bodies the batch read are retained under its own folder'
    $markdown = Get-Content -LiteralPath (Join-Path $work 'resolution-batch.md') -Raw -Encoding utf8
    Assert ($markdown -match 'deferred' -and $markdown -match '历史决议批次复核') 'the rendered batch shows the deferred disposition'

    # --- apply on a deferred batch is refused, and writes nothing --------------------------------
    $refusedApply = Resolve-Step @('-Step', 'apply', '-Work', $work)
    Assert ($refusedApply.code -ne 0 -and $refusedApply.text -match 'selected no item') 'apply refuses a deferred batch'
    Assert ((Revision) -eq $revisionBefore) 'a refused apply writes no Card'

    # --- a declined selection stays deferred, with its disposition visible -----------------------
    $declining = Join-Path $work 'declined.json'
    Write-Json $declining @{ schema_version = 1; note = '这一批历史问题暂不处理。' }
    $declined = Resolve-Step @('-Step', 'plan', '-Work', $work, '-File', $declining)
    Assert ($declined.code -eq 0) "a declining selection plans a deferred batch: $($declined.text)"
    $batch = Read-Json (Join-Path $work 'resolution-batch.json')
    Assert ($batch.status -eq 'deferred' -and $batch.deferred.code -eq 'declined') 'a declined batch is deferred, not silently dropped'
    Assert ($batch.selection.present -and $batch.selection.note -eq '这一批历史问题暂不处理。' -and -not $batch.selection.topic_home) 'the declined batch keeps the note it was declined with'
    $check = Resolve-Step @('-Step', 'check', '-Work', $work)
    Assert ($check.code -eq 0 -and $check.last.status -eq 'deferred' -and $check.last.claim -match 'deferred') 'check reports the deferral instead of claiming live corrections'

    # --- the selection refusals: unknown field, ineligible item, no backup -----------------------
    $bad = Join-Path $work 'bad.json'
    Write-Json $bad @{ schema_version = 1; note = 'x'; everything = $true }
    $unknown = Resolve-Step @('-Step', 'plan', '-Work', $work, '-File', $bad)
    Assert ($unknown.code -ne 0 -and $unknown.text -match 'unknown field') 'a selection with an unknown field is refused'
    Write-Json $bad @{ schema_version = 1; note = 'x'; topic_home = @{ schema_version = 1; report_id = $homeReview.report_id
            decisions = @(@{ finding = $blocked.finding; entry = $blocked.operation.entry; remedy = 'entry'; note = 'x' }) } }
    $ineligible = Resolve-Step @('-Step', 'plan', '-Work', $work, '-File', $bad)
    Assert ($ineligible.code -ne 0 -and $ineligible.text -match 'not an offered transfer') 'a selection cannot accept an item the report did not offer'
    Write-Json $bad @{ schema_version = 1; note = 'x'; topic_home = @{ schema_version = 1; report_id = $homeReview.report_id
            decisions = @(@{ finding = $transferItem.finding; entry = $a1; remedy = 'entry'; note = '接受。' }) } }
    $noBackup = Resolve-Step @('-Step', 'plan', '-Work', $work, '-File', $bad)
    Assert ($noBackup.code -ne 0 -and $noBackup.text -match 'verified backup') 'a selected batch without a verified backup is refused'
    Write-Json $bad @{ schema_version = 1; note = 'x'; backup = @{ path = (Join-Path $scratch 'missing.sqlite3'); sha256 = 'x'; bytes = 1 }
        topic_home = @{ schema_version = 1; report_id = $homeReview.report_id
            decisions = @(@{ finding = $transferItem.finding; entry = $a1; remedy = 'entry'; note = '接受。' }) } }
    $missingBackup = Resolve-Step @('-Step', 'plan', '-Work', $work, '-File', $bad)
    Assert ($missingBackup.code -ne 0 -and $missingBackup.text -match 'does not exist') 'a backup that is not there is refused'

    # --- the approved batch: one transfer and one move, executed in that order -------------------
    $backup = New-Backup
    $selectionPath = Join-Path $work 'selection.json'
    $dupTreeBefore = @(Tree-Cards $dup).Count
    Write-Json $selectionPath @{
        schema_version = 1
        note = '只处理这两条：本书的陈述转成文献陈述，重复主题并入多重均衡。'
        backup = $backup
        topic_home = @{ schema_version = 1; report_id = $homeReview.report_id; decisions = @(
                @{ finding = $transferItem.finding; entry = $a1; remedy = 'entry'; note = '把本书的陈述转成文献陈述，旧卡改成入口。' }
            ) }
        join = @{ schema_version = 1; report_id = $joinReview.report_id; decisions = @(
                @{ finding = $moveItem.finding; action = 'move_subtree'; plan = $moveItem.plan; note = '把重复主题的入口链并入多重均衡。' }
            ) }
    }
    $selected = Resolve-Step @('-Step', 'plan', '-Work', $work, '-File', $selectionPath)
    Assert ($selected.code -eq 0) "an approved selection plans a selected batch: $($selected.text)"
    $batch = Read-Json (Join-Path $work 'resolution-batch.json')
    Assert ($batch.status -eq 'selected' -and $batch.counts.selected -eq 2 -and $batch.deferred -eq $null) 'exactly the two accepted items are selected'
    Assert ($batch.backup.sha256 -eq $backup.sha256 -and $batch.backup.bytes -eq $backup.bytes -and $batch.backup.verified) 'the selected batch binds the verified backup it was approved against'
    Assert ($batch.selection.topic_home.report_id -eq $homeReview.report_id -and $batch.selection.join.report_id -eq $joinReview.report_id) 'the batch retains the exact per-report selections it accepted'
    Assert (@($batch.items | Where-Object { $_.disposition -eq 'deferred' }).Count -ge 1) 'every item the selection did not accept stays deferred'
    $selectedTransfer = Transfer-Of $batch $a1
    $selectedMove = Item-Of $batch $moveItem.id
    Assert ($selectedTransfer.disposition -eq 'selected' -and $selectedTransfer.remedy -eq 'entry' -and $selectedTransfer.note) 'a selected item keeps the remedy and the note it was accepted with'
    Assert ($selectedMove.disposition -eq 'selected' -and $selectedMove.plan -eq $offeredPlan.id) 'the selected move keeps the reviewed plan id'

    # The applied batch: the receipt prefix is the batch id, so a retry replays the same receipts.
    $environment = @{ ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'; ZT_TRACE_TOOLS = (Join-Path $scratch 'apply-tools.jsonl') }
    [IO.File]::WriteAllText($environment.ZT_TRACE_TOOLS, '')
    $env:ZT_CHECK_ZT = $environment.ZT_CHECK_ZT
    $env:ZT_TRACE_TOOLS = $environment.ZT_TRACE_TOOLS
    try { $applied = Resolve-Step @('-Step', 'apply', '-Work', $work) } finally { Remove-Item Env:ZT_CHECK_ZT, Env:ZT_TRACE_TOOLS }
    Assert ($applied.code -eq 0 -and $applied.last.status -eq 'applied-and-verified' -and $applied.last.complete -and $applied.last.done) "the selected batch applies: $($applied.text)"
    $result = Read-Json (Join-Path $work 'resolution-batch-result.json')
    Assert ($result.batch_id -eq $batch.batch_id -and $result.result_id -match '^rr-[0-9a-f]{64}$') 'the result binds the batch and its own id'
    $doneTransfer = @($result.items | Where-Object { $_.kind -eq 'transfer' })[0]
    $doneMove = @($result.items | Where-Object { $_.kind -eq 'join' })[0]
    Assert ($doneTransfer.status -eq 'applied-and-verified' -and $doneMove.status -eq 'applied-and-verified') 'both selected items are applied and verified'
    Assert ($result.counts.applied_and_verified -eq 2 -and @($result.unresolved).Count -eq 0) 'the batch reports the two verified items and no unresolved item'

    # The transfer: a Literature statement with the reviewed text, and an entry that is not a copy.
    $created = $doneTransfer.evidence.created.address
    Assert ($created -and (Card-At $created).statement_fingerprint -eq $transferItem.operation.statement.statement_fingerprint) 'the created Literature statement carries the exact reviewed statement fingerprint'
    Assert ((Card-At $created).body -match [regex]::Escape("[[$anchor1]]") -and (Card-At $created).body -match [regex]::Escape('$x_i=\arg\max_x u_i(x,x_{-i})$')) 'the created statement keeps its source Link and its formula'
    $rewritten = Card-At $a1
    Assert ($rewritten.title -eq '对称参与者的重复博弈有两个均衡' -and $rewritten.body -match '## 同述入口' -and $rewritten.body -match [regex]::Escape("[[$created]]")) 'the old Card is an explicit entry naming the new statement'
    Assert ($rewritten.statement_fingerprint -ne (Card-At $created).statement_fingerprint) 'the entry is not a second copy of the statement'
    Assert ($doneTransfer.evidence.rewritten.inbound_after -eq $doneTransfer.evidence.rewritten.inbound_before) 'existing inbound Links still reach the old address'

    # The move: every reviewed Card is filed under the joining Card at its mapped address.
    $mapping = @{}
    foreach ($pairRow in @($result.applied)) { $mapping[$pairRow[0]] = $pairRow[1] }
    Assert ($mapping.Count -ge 2 -and $mapping.ContainsKey($dup1)) 'the result retains the returned mapping the move consumed'
    $movedLanding = $mapping[$dup1]
    Assert ($movedLanding -and (Card-At $movedLanding).title -eq '两个交点给出两个均衡') 'the moved Card is at its mapped address and still carries its own text'
    Assert (-not (Card-At $dup1).ok -and -not (Card-At $dup2).ok) 'the reviewed source addresses are vacated by the move'
    $moveVerification = @($doneMove.evidence.operations)[0].verification
    Assert ($moveVerification.ok -and $moveVerification.cards -ge 2) 'the qualified join path verified the persisted addresses and fingerprints'

    # The protected scope and the human body are untouched by the whole batch.
    Assert ($result.protected.unchanged -and @($result.protected.changed).Count -eq 0) 'no protected human Card or finished model changed'
    Assert ((Card-At $humanCard).body -eq $humanBefore.body -and (Card-At $humanCard).body -match [regex]::Escape("[[$a1]]")) 'the human Card body is untouched and its Link still reaches the old address'
    Assert (@(Tree-Cards ($humanTopic -replace '/0$', '')).Count -eq $humanTreeBefore) 'no human Card is created by the batch'
    Assert (Test-Path (Join-Path $work 'resolution-batch-result.md')) 'the batch result has its rendered view'
    Assert ($result.broken_links.before -eq $result.broken_links.after) 'the batch adds no Broken link'
    Assert (@($result.rollback.transfers).Count -eq 2 -and @($result.rollback.moves).Count -ge 1) 'the result retains the rollback evidence of both writes and of the mapping'
    Assert ($result.scope -match 'no other historical fault') 'the result says it fixed no other historical fault'

    # --- check: the batch's own claims, re-read from the archive --------------------------------
    $check = Resolve-Step @('-Step', 'check', '-Work', $work)
    Assert ($check.code -eq 0 -and $check.last.ok) "check verifies the applied batch: $($check.text)"
    Assert (@($check.last.faults).Count -eq 0) 'check finds no fault in the applied batch'
    Assert ($check.last.protected.unchanged -and $check.last.broken_links.recorded -eq $check.last.broken_links.now) 'check confirms the protected scope and the Broken-link count'
    Assert (@($check.last.items | Where-Object { $_.ok -eq $false }).Count -eq 0) 'every applied item still holds what the batch verified'

    # A later writer that drops the entry heading is a fault, and check repairs nothing.
    $entryBody = (Card-At $a1).body
    $null = Call-Tool 'edit_card' @{ at = $a1; body = ($entryBody -replace '## 同述入口', '## 入口') }
    $brokenCheck = Resolve-Step @('-Step', 'check', '-Work', $work)
    Assert ($brokenCheck.code -ne 0 -and -not $brokenCheck.last.ok) 'a changed entry makes check fail'
    Assert (@($brokenCheck.last.faults).Count -ge 1) 'check names the fault it found and repairs nothing'
    Assert (-not ((Card-At $a1).body -match '## 同述入口')) 'check leaves the archive exactly as it found it'
    $null = Call-Tool 'edit_card' @{ at = $a1; body = $entryBody }
    $restoredCheck = Resolve-Step @('-Step', 'check', '-Work', $work)
    Assert ($restoredCheck.code -eq 0 -and $restoredCheck.last.ok) 'restoring the verified text makes check pass again'

    # --- the same batch applied again replays its receipts instead of writing twice --------------
    $literatureBefore = @(Tree-Cards $alpha).Count
    $replay = Resolve-Step @('-Step', 'apply', '-Work', $work)
    $replayResult = Read-Json (Join-Path $work 'resolution-batch-result.json')
    Assert ($replay.code -eq 0 -and $replayResult.status -eq 'unchanged' -and $replayResult.complete) "the same batch replays its receipts instead of writing twice: $($replay.text)"
    Assert (@($replayResult.items | Where-Object { $_.status -eq 'unchanged' }).Count -eq 2) 'both items report that their own receipts were found already applied'
    Assert (@(Tree-Cards $alpha).Count -eq $literatureBefore) 'no second Literature statement is created by the replay'
    $replayMapping = @{}
    foreach ($pairRow in @($replayResult.applied)) { $replayMapping[$pairRow[0]] = $pairRow[1] }
    Assert ($replayMapping.Count -eq $mapping.Count -and $replayMapping[$dup1] -eq $mapping[$dup1]) 'the replay reports the mapping it already applied and moves nothing again'

    # --- a stale base: the reviewed Card changed after the plan, so nothing is written ----------
    $second = Join-Path $work 'second.json'
    $a6Item = Transfer-Of $batch $a6
    Write-Json $second @{ schema_version = 1; note = '第二批：只处理政策这条陈述。'; backup = (New-Backup)
        topic_home = @{ schema_version = 1; report_id = $homeReview.report_id; decisions = @(
                @{ finding = $a6Item.finding; entry = $a6; remedy = 'entry'; note = '把政策这条陈述转成文献陈述。' }) } }
    $secondPlan = Resolve-Step @('-Step', 'plan', '-Work', $work, '-File', $second)
    Assert ($secondPlan.code -eq 0) "a second batch plans one item: $($secondPlan.text)"
    $batch2 = Read-Json (Join-Path $work 'resolution-batch.json')
    Assert ($batch2.counts.selected -eq 1 -and (Item-Of $batch2 $a6Item.id).disposition -eq 'selected') 'the second batch selects exactly one item'
    $a6Before = Card-At $a6
    $null = Call-Tool 'edit_card' @{ at = $a6; body = "$($a6Before.body)`n`n另一位写作者在复核之后加了这段。" }
    $raced = Resolve-Step @('-Step', 'apply', '-Work', $work)
    $racedResult = Read-Json (Join-Path $work 'resolution-batch-result.json')
    Assert ($raced.code -eq 0 -and $racedResult.status -eq 'conflict' -and $racedResult.complete -eq $false) "a changed reviewed Card is a conflict, not a write: $($raced.text)"
    Assert (@($racedResult.unresolved).Count -eq 1 -and @($racedResult.unresolved)[0].reason.code -eq 'endpoint_changed') 'the unresolved item names the changed authored base'
    Assert ((Card-At $a6).body -match '另一位写作者在复核之后加了这段' -and -not ((Card-At $a6).body -match '同述入口')) "the other writer's text is preserved and the entry is not written"
    $null = Call-Tool 'edit_card' @{ at = $a6; title = $a6Before.title; body = $a6Before.body }

    # --- protected content moved between the plan and the apply: nothing is written -------------
    $third = Join-Path $work 'third.json'
    $a7Item = Transfer-Of $batch $a7
    Write-Json $third @{ schema_version = 1; note = '第三批：预期渠道这条陈述。'; backup = (New-Backup)
        topic_home = @{ schema_version = 1; report_id = $homeReview.report_id; decisions = @(
                @{ finding = $a7Item.finding; entry = $a7; remedy = 'entry'; note = '把预期渠道这条陈述转成文献陈述。' }) } }
    $thirdPlan = Resolve-Step @('-Step', 'plan', '-Work', $work, '-File', $third)
    Assert ($thirdPlan.code -eq 0) "a third batch plans one item: $($thirdPlan.text)"
    $partBefore = Card-At $part
    $null = Call-Tool 'edit_card' @{ at = $part; body = "$($partBefore.body)`n`n另一位写作者改了这张模型卡。" }
    $quiet = Resolve-Step @('-Step', 'apply', '-Work', $work)
    $quietResult = Read-Json (Join-Path $work 'resolution-batch-result.json')
    Assert ($quiet.code -eq 0 -and $quietResult.status -eq 'pending' -and -not $quietResult.complete) "a changed protected Model Card leaves the batch pending: $($quiet.text)"
    Assert (@($quietResult.items | Where-Object { $_.status -eq 'pending' -and $_.reason.code -eq 'quiet_window_required' }).Count -eq 1) 'the selected item waits for a quiet window instead of writing'
    Assert (@($quietResult.protected.changed).Count -ge 1 -and -not $quietResult.protected.unchanged) 'the result names the protected Card that moved'
    Assert (-not ((Card-At $a7).body -match '同述入口')) 'a pending batch writes nothing'
    $null = Call-Tool 'edit_card' @{ at = $part; title = $partBefore.title; body = $partBefore.body }

    # --- an edited batch is refused by its own id, so no stale selection is applied --------------
    $batchPath = Join-Path $work 'resolution-batch.json'
    $saved = [IO.File]::ReadAllBytes($batchPath)
    [IO.File]::WriteAllText($batchPath, [Text.Encoding]::UTF8.GetString($saved).Replace('第三批：预期渠道这条陈述。', '第三批：改名了。'), $utf8)
    $tampered = Resolve-Step @('-Step', 'apply', '-Work', $work)
    Assert ($tampered.code -ne 0 -and $tampered.text -match 'does not match its own batch id' -or $tampered.text -match 'no longer matches its own batch id') 'an edited batch is refused instead of applied'
    [IO.File]::WriteAllBytes($batchPath, $saved)
    $replanned = Resolve-Step @('-Step', 'plan', '-Work', $work, '-File', $third)
    Assert ($replanned.code -eq 0) 'the restored evidence plans the batch again'
} finally {
    Remove-Item Env:ZT_CHECK_ZT, Env:ZT_TRACE_TOOLS, Env:ZT_DROP_BATCH_REPLY_FLAG, Env:ZT_BULK_MISSING, Env:ZT_RACE_AFTER_BULK_REPLY -ErrorAction SilentlyContinue
    zt down | Out-Null
}
if ($failed) { Write-Output "$failed assertion(s) failed"; exit 1 }
Write-Output 'resolve: all assertions passed'
