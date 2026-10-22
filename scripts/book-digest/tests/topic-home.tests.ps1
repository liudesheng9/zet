#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\topic-home.tests.ps1
# Drives `zt-digest topic-home` on a scratch archive (issue #113, S21; design §11.1, §11.3, §12.3):
# the report-bound Topic-home transfer that consumes a real concept review, proposes the Literature
# statement and the old-Card edit for every book-bound Topic entry, and applies exactly the reviewed
# selection — one identified write at a time, with read-back verification, receipt replay, conflict
# handling at a concurrent base change and a lost reply, and no duplicate creation on resume.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "topic-home-test-$PID"
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
# One `zt-digest <tool>` step (concepts, topic-home): its exit code, its output and its last line.
function Invoke-Digest([string]$tool, [string[]]$Arguments) {
    $text = @(& $script:exe $tool @Arguments 2>&1 | ForEach-Object { [string]$_ }) -join "`n"
    $code = $LASTEXITCODE
    $last = $null
    if (-not $code) { $last = ($text -split "`n")[-1] | ConvertFrom-Json -AsHashtable -DateKind String }
    [pscustomobject]@{ code = $code; text = $text; last = $last }
}
function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String }
function Write-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 32), $utf8) }
function Call-Tool($tool, $arguments) {
    $line = @(& $script:exe call -Open -Tool $tool -Json ($arguments | ConvertTo-Json -Compress -Depth 10) 2>&1 | ForEach-Object { [string]$_ } | Where-Object { $_ -match '^\d+ ' })
    if (-not $line) { throw "zt-digest call $tool answered nothing" }
    $text = ($line[-1] -split ' ', 2)[1]
    if ($text -match '^ok ') { return @{ ok = $true; address = $text.Substring(3).Trim() } }
    if ($text -match '^error ') { return @{ ok = $false; error = $text.Substring(6).Trim() } }
    $text | ConvertFrom-Json -AsHashtable -DateKind String
}
function Card-At($at) { Call-Tool 'get_card' @{ at = $at } }
function Cards-At([string[]]$addresses) { (Call-Tool 'get_cards' @{ addresses = $addresses; view = 'text' }) }
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
function Item-Of($review, $entry) { @($review.items | Where-Object { $_.entry -eq $entry })[0] }
function Tree-Cards($tree) { (Call-Tool 'get_tree' @{ tree = $tree }).cards }

$staging = Join-Path $scratch 'staging'
$work = Join-Path $staging 'concepts'
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null

    # --- the scratch fixture: two books, one book's Literature tree, one book-bound Topic ---------
    $books = Write-Cards @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{alpha2020, title={学习与社会}, author={Owner, A.}, year={2020}}'; body = '全书索引。' } },
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{beta2021, title={产业学习}, author={Owner, B.}, year={2021}}'; body = '全书索引。' } }
    )
    $alpha = $books[0]
    $beta = $books[1]
    # The book's Literature tree: five anchors, an existing statement (the earlier carrier) and a
    # Model Card with one part. A statement at `<key>/N` is an anchor, so the new statement is filed
    # as a Side successor of an anchor.
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
    $anchor5 = "$alpha/5"
    $part = "$alpha/3|a"
    # The book's term table, as the digest writes it beside the first anchor.
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

    # Topic A: its entries are the book-bound statements the review reports.
    $topics = Write-Cards @(
        @{ tool = 'create_topic'; arguments = @{ title = '多重均衡'; body = '本主题追问：什么条件下同一个经济有多个均衡？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '学校里的重复练习'; body = '本主题追问：练习次数怎样改变考试成绩？' } }
    )
    $topicA = ($topics[0] -replace '/0$', '')
    $entries = Write-Cards @(
        # 1: book-bound, with a formula, an anchor named by its own source line and a model it names.
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/0"; title = '对称参与者的重复博弈有两个均衡'; body = "当策略互补且参与者对称时，`$x_i=\arg\max_x u_i(x,x_{-i})`$ 有两个均衡。`n`n假设：策略互补、参与者对称。`n`n来源：[[$anchor1]] §2.1 与 [[$alpha/3]]" } },
        # 2: a Model Card: transferring it would create a Model part (D267).
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/1"; type = 'model'; title = '模型：协调失灵'; body = "模型卡：协调失灵让一个均衡持续。`n`n来源：[[$alpha]] §3" } },
        # 3: no source line at all: not a book-bound statement.
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/2"; title = '没有来源的判断'; body = '本卡没有来源行，因此不能用一张文献卡证明内容。' } },
        # 4: already an accepted restatement entry.
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/3"; title = '入口卡：产业学习的成本效应'; body = "## 同述入口`n`n- [[$carrier]] 见上。`n`n来源：[[$alpha]] §4.2" } },
        # 5: the same statement the book already carries: reuse, never a second copy.
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/4"; title = '产业累计产量降低单位成本'; body = "产业累计产量每翻一倍，单位成本按固定比例下降。`n`n来源：[[$alpha]] §4.2" } },
        # 6: a second book-bound statement under the second anchor (a distinct interpretation).
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/5"; title = '预期渠道让政策有短期效果'; body = "预期渠道让政策在短期有效。`n`n假设：劳动供给有弹性。`n`n来源：[[$anchor2]] §2.2" } },
        # 7: a book-bound statement under the fourth anchor (the lost-reply resume).
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/6"; title = '政策可信时预期渠道更强'; body = "政策可信时预期渠道更强。`n`n假设：政策有承诺。`n`n来源：[[$anchor4]] §4.1" } },
        # 8: a book-bound statement under the fifth anchor (the concurrent base change).
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/7"; title = '制度决定契约的空白处'; body = "制度决定契约没有写明时谁做决定。`n`n假设：契约不完整。`n`n来源：[[$anchor5]] §5.1" } },
        # 9: a book-bound statement under the fourth anchor (the changed-finding conflict).
        @{ tool = 'create_successor'; arguments = @{ at = "$topicA/8"; title = '承诺降低通货膨胀的成本'; body = "承诺降低通货膨胀的成本。`n`n假设：中央银行有信誉。`n`n来源：[[$anchor4]] §4.2" } }
    )
    $a1 = $entries[0]
    $a5 = $entries[4]
    $a6 = $entries[5]
    $a7 = $entries[6]
    $a8 = $entries[7]
    $a9 = $entries[8]
    # The Topic root links its first entry, so the old Card has an authored inbound Link.
    $root = Card-At "$topicA/0"
    $null = Call-Tool 'edit_card' @{ at = "$topicA/0"; body = "$($root.body)`n`n入口：依赖 [[$a1]] 的模型。" }

    # A human Card links the entry the transfer rewrites: its body is untouched and the Link keeps
    # reaching the old address.
    $script:gui = Gui-Token
    $human = Gui-Create @{ kind = 'topic'; workspace = 'human'; text = "# 多重均衡笔记`n`n本人的读书笔记。" }
    $humanLink = Gui-Create @{ kind = 'direct'; at = $human; text = "# 本人笔记`n`n本人认为 [[$a1]] 是本书的核心。" }
    $humanBefore = Card-At $humanLink
    $humanTreeBefore = @(Tree-Cards ($human -replace '/0$', '')).Count

    # --- the concept review: the real report a topic-home report consumes ------------------------
    New-Item -ItemType Directory -Force $work | Out-Null
    Write-Json (Join-Path $staging 'manifest.json') @{ citation_key = $alpha; book_title = '学习与社会'; groups = @(@{ n = 1; chapter = '第一章'; headings = @('均衡') }) }
    Write-Json (Join-Path $work 'proposals.json') @{ schema_version = 1; proposals = @(
            @{ title = '均衡的多重性'; question = '什么条件下同一个经济有多个均衡？'
                statement = '当策略互补时，同一个经济可以有多个均衡：$x_i=\arg\max_x u_i(x,x_{-i})$。'
                assumptions = @('策略互补'); terms = @('均衡的多重性') }
        ) }
    $prepare = Invoke-Digest 'concepts' @('-Step', 'prepare', '-Staging', $staging)
    Assert ($prepare.code -eq 0) "concepts prepare runs: $($prepare.text)"
    $evidence = Read-Json (Join-Path $work 'evidence.json')
    $candidate = @(@($evidence.proposals)[0].topic_candidates | Where-Object { $_.address -eq "$topicA/0" })[0]
    Assert ($null -ne $candidate -and @($candidate.own_book) -contains $alpha -and @($candidate.other_books).Count -eq 0) 'the concept review reads the Topic as book-bound'
    $judgments = @()
    foreach ($c in @(@($evidence.proposals)[0].topic_candidates)) {
        $evidenceRows = @($c.address) + @(@($c.entries) | ForEach-Object { $_.address })
        $judgments += [ordered]@{ kind = 'topic-concept'; proposal = 'p001'; candidate = $c.address; verdict = 'different'
            reason = '两张卡讨论的条件不同。'; evidence = $evidenceRows }
    }
    foreach ($s in @(@($evidence.proposals)[0].restatement_candidates)) {
        $judgments += [ordered]@{ kind = 'restatement'; proposal = 'p001'; candidate = $s.address; verdict = 'different'
            reason = '两处陈述的假设不同。'; evidence = @($s.address) }
    }
    New-Item -ItemType Directory -Force (Join-Path $work 'out') | Out-Null
    $answerFile = Join-Path $work 'out\cj-01.json'
    Write-Json $answerFile @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = $judgments }
    $judge = Invoke-Digest 'concepts' @('-Step', 'judge', '-Staging', $staging, '-File', $answerFile)
    Assert ($judge.code -eq 0 -and $judge.last.unjudged -eq 0) "concepts judge accepts the canned answers: $($judge.text)"
    $report = Invoke-Digest 'concepts' @('-Step', 'report', '-Staging', $staging)
    Assert ($report.code -eq 0) "concepts report writes the review: $($report.text)"
    $conceptReview = Read-Json (Join-Path $staging 'concept-review.json')
    $homeFinding = @($conceptReview.findings | Where-Object { $_.kind -eq 'topic-home' -and $_.candidate -eq "$topicA/0" })[0]
    Assert ($null -ne $homeFinding -and $homeFinding.verdict -eq 'different' -and -not $homeFinding.unjudged) 'the concept review holds a judged topic-home finding for the Topic'

    $revisionBefore = Revision
    # --- plan: the report-bound review, read-only -------------------------------------------------
    $env:ZT_CHECK_ZT = $seam
    $env:ZT_TRACE_TOOLS = Join-Path $scratch 'plan-tools.jsonl'
    [IO.File]::WriteAllText($env:ZT_TRACE_TOOLS, '')
    $plan = Invoke-Digest 'topic-home' @('-Step', 'plan', '-Work', $work, '-Report', (Join-Path $staging 'concept-review.json'))
    Remove-Item Env:ZT_TRACE_TOOLS
    Remove-Item Env:ZT_CHECK_ZT
    Assert ($plan.code -eq 0) "plan runs: $($plan.text)"
    $review = Read-Json (Join-Path $work 'topic-home-review.json')
    $markdown = Get-Content -LiteralPath (Join-Path $work 'topic-home-review.md') -Raw -Encoding utf8
    Assert ($review.report_id -match '^th-[0-9a-f]{64}$' -and $review.kind -eq 'topic-home-review' -and $review.archive.id -match '^[0-9a-f]{32}$') 'the review binds its kind, archive identity and canonical report id'
    Assert ($review.concept_report.report_id -eq $conceptReview.report_id -and @($review.findings | Where-Object { $_.id -eq $homeFinding.id }).Count -eq 1) 'the review binds the concept report and its finding id'
    Assert ($review.archive.revision -eq $revisionBefore -and (Revision) -eq $revisionBefore) 'the plan step changes no Card: the archive revision is unchanged'
    $trace = @(Get-Content -LiteralPath (Join-Path $scratch 'plan-tools.jsonl') -Encoding utf8 | Where-Object { $_ } | ForEach-Object { $_ | ConvertFrom-Json })
    $writes = @($trace | Where-Object { $_.tool -in @('edit_card', 'create_topic', 'create_successor', 'create_literature', 'delete_card', 'insert_card', 'move_card', 'apply_batch') })
    Assert ($writes.Count -eq 0) 'the plan makes no write call'
    Assert ($review.completeness.offered -eq 6 -and $review.completeness.blocked -eq 1 -and $review.completeness.refused -eq 2) "the review offers six transfers, blocks one model item and refuses two ($($review.completeness.offered)/$($review.completeness.blocked)/$($review.completeness.refused))"
    $item1 = Item-Of $review $a1
    Assert ($item1.status -eq 'offered' -and $item1.finding -eq $homeFinding.id -and $item1.zone -eq 'topic') 'a book-bound Topic entry becomes an offered transfer item'
    Assert ($item1.target.anchor -eq $anchor1 -and $item1.target.to -eq "$anchor1|a" -and $item1.target.book_root -eq $alpha) 'the target is the anchor the entry names, its next Side slot and the reviewed book root'
    Assert ($item1.target.statement_address -eq $item1.target.to -and $item1.carrier_scan.treatment -eq 'create') 'a statement the book does not carry yet is created'
    Assert (@($item1.statement.sources) -contains $anchor1 -and @($item1.statement.sources) -contains "$alpha/3") 'the item keeps the source Links the entry names, the anchor and the model alike'
    Assert (@($item1.statement.formulas) -contains '$x_i=\arg\max_x u_i(x,x_{-i})$') 'the item keeps the formula the statement must not lose'
    Assert (@($item1.target.model_context | ForEach-Object { $_.address }) -contains "$alpha/3" -and @($item1.target.model_context)[0].kind -eq 'model') 'the model the entry names is recorded as read-only model context'
    Assert ($item1.remedies.proposed -eq 'entry' -and $item1.remedies.entry.body -match '## 同述入口' -and $item1.remedies.entry.body -match [regex]::Escape("[[$anchor1|a]]") -and $item1.remedies.entry.title -eq '对称参与者的重复博弈有两个均衡') 'the proposed old-Card edit is an explicit entry naming the new statement and keeping the entry title'
    Assert ($item1.remedies.entry.body -match '来源：\[\[alpha2020\]\]' -and @($item1.remedies.distinct.requires).Count -ge 3) 'the entry keeps a source line and the distinct alternative states what a draft must satisfy'
    $item2 = Item-Of $review $entries[1]
    Assert ($item2.status -eq 'blocked' -and $item2.reason.code -eq 'model_boundary' -and $item2.reason.detail -match 'model design decision') 'a Model Card entry is blocked on a named separate model decision (D267)'
    $item3 = Item-Of $review $entries[2]
    Assert ($item3.status -eq 'refused' -and $item3.reason.code -eq 'not_book_bound') 'an entry with no own-book source Link is refused, not silently transferred'
    $item4 = Item-Of $review $entries[3]
    Assert ($item4.status -eq 'refused' -and $item4.reason.code -eq 'already_entry') 'an entry Card is refused: its statement already has a home'
    $item5 = Item-Of $review $a5
    Assert ($item5.status -eq 'offered' -and $item5.earlier_carrier.address -eq $carrier -and $item5.carrier_scan.treatment -eq 'reuse') 'a statement the Literature tree already carries is reused, never copied'
    Assert ($item5.remedies.entry.body -match [regex]::Escape("[[$carrier]]") -and $item5.target.statement_address -eq $carrier) 'the reuse entry names the existing statement'
    Assert (@($review.endpoints.Keys | Where-Object { $_ -eq $a1 }).Count -eq 1 -and (Get-ChildItem (Join-Path $work 'endpoints') -File).Count -ge 5) 'every read Card is retained as an endpoint body with its authored fingerprint'
    Assert ($markdown -match '同述入口' -and $markdown -match 'model_context' -or $markdown -match '模型上下文') 'the rendered review names the entry text and the model context'
    $again = Invoke-Digest 'topic-home' @('-Step', 'plan', '-Work', $work, '-Report', (Join-Path $staging 'concept-review.json'))
    Assert ($again.code -eq 0 -and $again.last.report_id -eq $review.report_id -and -not $again.last.written) 'the same reviewed evidence gives the same report id and changes no file'

    # --- the selection refuses what the review does not support ---------------------------------
    $selectionPath = Join-Path $work 'selection.json'
    $blockedSelection = Join-Path $work 'blocked.json'
    Write-Json $blockedSelection @{ schema_version = 1; report_id = $review.report_id; decisions = @(@{ finding = $homeFinding.id; entry = $entries[1]; remedy = 'entry'; note = '移动这张模型卡。' }) }
    $blocked = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $blockedSelection, '-OperationPrefix', 'r/u/1/transfer')
    Assert ($blocked.code -ne 0 -and $blocked.text -match 'not an offered transfer') 'apply refuses a blocked item'
    Write-Json $blockedSelection @{ schema_version = 1; report_id = 'th-0000'; decisions = @(@{ finding = $homeFinding.id; entry = $a1; remedy = 'entry'; note = 'x' }) }
    $wrongReport = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $blockedSelection, '-OperationPrefix', 'r/u/1/transfer')
    Assert ($wrongReport.code -ne 0 -and $wrongReport.text -match 'belongs to report') 'apply refuses a selection of another report'
    Write-Json $blockedSelection @{ schema_version = 1; report_id = $review.report_id; decisions = @(@{ finding = $homeFinding.id; entry = $a1; remedy = 'entry'; note = '' }) }
    $noNote = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $blockedSelection, '-OperationPrefix', 'r/u/1/transfer')
    Assert ($noNote.code -ne 0 -and $noNote.text -match 'note') 'apply refuses a decision with no note'
    $noPrefix = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $blockedSelection)
    Assert ($noPrefix.code -ne 0 -and $noPrefix.text -match 'OperationPrefix') 'apply needs an operation prefix, so a retry can replay the same receipts'

    # --- one accepted transfer: the Literature statement, then the guarded old-Card edit ----------
    Write-Json $selectionPath @{ schema_version = 1; report_id = $review.report_id; decisions = @(
            @{ finding = $homeFinding.id; entry = $a1; remedy = 'entry'; note = '把本书的陈述转成文献陈述，旧卡改成入口。' }
        ) }
    $env:ZT_CHECK_ZT = $seam
    $env:ZT_TRACE_TOOLS = Join-Path $scratch 'apply-tools.jsonl'
    [IO.File]::WriteAllText($env:ZT_TRACE_TOOLS, '')
    $applied = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $selectionPath, '-OperationPrefix', 'r/u/1/transfer')
    Remove-Item Env:ZT_TRACE_TOOLS
    Remove-Item Env:ZT_CHECK_ZT
    Assert ($applied.code -eq 0 -and $applied.last.status -eq 'applied' -and $applied.last.complete) "the accepted transfer applies: $($applied.text)"
    $trace = @(Get-Content -LiteralPath (Join-Path $scratch 'apply-tools.jsonl') -Encoding utf8 | Where-Object { $_ } | ForEach-Object { $_ | ConvertFrom-Json })
    Assert (@($trace | Where-Object { $_.tool -eq 'move_card' }).Count -eq 0) 'the transfer never calls move_card: the Topic-to-Literature move refusal is untouched'
    Assert (@($trace | Where-Object { $_.tool -eq 'apply_batch' }).Count -eq 2) 'the transfer is two identified writes: the statement first, the old Card second'
    $result = Read-Json (Join-Path $work 'topic-home-result.json')
    $done = @($result.items)[0]
    $created = $done.created.address
    Assert ($done.status -eq 'applied-and-verified' -and $created -eq "$anchor1|a" -and $done.rewritten.address -eq $a1) "the created statement and the rewritten old Card are read back ($created)"
    Assert ((Card-At $created).statement_fingerprint -eq $item1.statement.statement_fingerprint) 'the created Literature Card carries the exact reviewed statement fingerprint'
    Assert ((Card-At $created).body -match [regex]::Escape("[[$anchor1]]") -and (Card-At $created).body -match [regex]::Escape("[[$alpha/3]]")) 'the created Card keeps every source Link of the statement'
    Assert ((Card-At $created).body -match [regex]::Escape('$x_i=\arg\max_x u_i(x,x_{-i})$')) 'the created Card keeps the formula'
    $rewritten = Card-At $a1
    Assert ($rewritten.title -eq '对称参与者的重复博弈有两个均衡' -and $rewritten.body -match '## 同述入口' -and $rewritten.body -match [regex]::Escape("[[$created]]")) 'the old Card is an explicit entry naming the new statement and keeps its title'
    Assert ($rewritten.statement_fingerprint -ne (Card-At $created).statement_fingerprint -and -not ($rewritten.body -match [regex]::Escape('$x_i=\arg\max'))) 'the entry is not a second copy of the statement'
    Assert ($done.rewritten.inbound_before -ge 1 -and $done.rewritten.inbound_after -eq $done.rewritten.inbound_before) 'existing inbound Links still reach the old address'
    Assert (@($done.one_home.cards).Count -eq 1 -and @($done.one_home.cards)[0] -eq $created) 'exactly one Card carries the substantive statement after the transfer'
    Assert (@($done.model_reports | Where-Object { $_.preserved -eq $true }).Count -eq @($done.model_reports).Count -and @($done.model_reports).Count -ge 1) 'the model the statement concerns keeps its membership'
    Assert ($result.broken_links.before -eq $result.broken_links.after) 'the transfer adds no Broken link'
    Assert (@($result.rollback).Count -eq 2 -and @($result.rollback | Where-Object { $_.kind -eq 'created' }).Count -eq 1 -and @($result.rollback | Where-Object { $_.kind -eq 'rewritten' -and $_.before_title -and $_.before_body }).Count -eq 1) 'the result retains rollback evidence for both writes'
    $humanAfter = Card-At $humanLink
    Assert ($humanAfter.body -eq $humanBefore.body -and $humanAfter.body -match [regex]::Escape("[[$a1]]")) 'the human Card body is untouched and its Link still reaches the old address'
    Assert (@(Tree-Cards ($human -replace '/0$', '')).Count -eq $humanTreeBefore) 'no human Card is created by the transfer'

    # --- replay: the same selection returns the recorded result and writes nothing ---------------
    $before = (Get-FileHash -LiteralPath (Join-Path $work 'topic-home-result.json') -Algorithm SHA256).Hash
    $replay = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $selectionPath, '-OperationPrefix', 'r/u/1/transfer')
    Assert ($replay.code -eq 0 -and $replay.last.replayed -and $replay.last.result_id -eq $result.result_id) 'the same selection replays its recorded result'
    Assert ((Get-FileHash -LiteralPath (Join-Path $work 'topic-home-result.json') -Algorithm SHA256).Hash -eq $before) 'the replay changes no result byte'

    # --- the authored distinct interpretation -----------------------------------------------
    $distinctPath = Join-Path $work 'distinct.json'
    Write-Json $distinctPath @{ schema_version = 1; report_id = $review.report_id; decisions = @(
            @{ finding = $homeFinding.id; entry = $a6; remedy = 'distinct'; note = '这个主题自己的解释与本书的模型不同，写成独立的解释。'
                draft = @{ title = '本主题把预期渠道当作习惯的形成'; body = "本主题把预期渠道当作习惯的形成，而不是本书模型里的短期弹性。`n`n本卡对照 [[$anchor2|b]] 的模型，解释主题的范围。`n`n来源：[[$alpha]] §2.2" } }
        ) }
    $distinct = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $distinctPath, '-OperationPrefix', 'r/u/1/transfer')
    Assert ($distinct.code -eq 0 -and $distinct.last.status -eq 'applied') "the authored distinct interpretation applies: $($distinct.text)"
    $distinctResult = Read-Json (Join-Path $work 'topic-home-result.json')
    $distinctItem = @($distinctResult.items)[0]
    $distinctCreated = $distinctItem.created.address
    $distinctCard = Card-At $a6
    Assert ($distinctItem.remedy -eq 'distinct' -and $distinctCard.title -eq '本主题把预期渠道当作习惯的形成' -and $distinctCard.statement_fingerprint -ne (Card-At $distinctCreated).statement_fingerprint) 'the distinct remedy keeps an authored claim other than the transferred statement'
    Assert ($distinctCard.body -match [regex]::Escape("[[$distinctCreated]]") -and @($distinctItem.rewritten.kinds.Keys) -contains $distinctCreated) 'the distinct body links the statement and the read-back reports the actual Relation kind'

    # --- the earlier carrier is reused, so no second copy appears --------------------------------
    $reusePath = Join-Path $work 'reuse.json'
    Write-Json $reusePath @{ schema_version = 1; report_id = $review.report_id; decisions = @(
            @{ finding = $homeFinding.id; entry = $a5; remedy = 'entry'; note = '本书已经有这条陈述，旧卡改成它的入口。' }
        ) }
    $treeBefore = @(Tree-Cards $alpha).Count
    $reuse = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $reusePath, '-OperationPrefix', 'r/u/1/transfer')
    Assert ($reuse.code -eq 0 -and $reuse.last.status -eq 'applied') "the reuse transfer applies: $($reuse.text)"
    $reuseResult = Read-Json (Join-Path $work 'topic-home-result.json')
    $reuseItem = @($reuseResult.items)[0]
    Assert ($reuseItem.created -eq $null -and $reuseItem.one_home.statement_address -eq $carrier) 'a reuse item creates no Card: the existing statement is its home'
    Assert (@(Tree-Cards $alpha).Count -eq $treeBefore) 'the Literature tree gains no Card when the statement is reused'
    Assert ((Card-At $a5).body -match [regex]::Escape("[[$carrier]]") -and (Card-At $carrier).statement_fingerprint -eq $item5.statement.statement_fingerprint) "the old Card becomes the existing statement's entry"

    # --- a lost reply after the creation: the retry replays the receipt, never a second copy -----
    $addressesBefore = @(Tree-Cards $alpha | ForEach-Object { $_.address })
    $lostPath = Join-Path $work 'lost.json'
    Write-Json $lostPath @{ schema_version = 1; report_id = $review.report_id; decisions = @(
            @{ finding = $homeFinding.id; entry = $a7; remedy = 'entry'; note = '把政策这条陈述转成文献陈述。' }
        ) }
    $env:ZT_CHECK_ZT = $seam
    $env:ZT_DROP_BATCH_REPLY_FLAG = Join-Path $scratch 'dropped.flag'
    try {
        $lost = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $lostPath, '-OperationPrefix', 'r/u/1/transfer')
        Assert ($lost.code -ne 0 -or $lost.last.status -eq 'pending') 'a lost reply after the creation never claims an applied transfer'
    } finally { Remove-Item Env:ZT_CHECK_ZT, Env:ZT_DROP_BATCH_REPLY_FLAG -ErrorAction SilentlyContinue }
    $a7Before = (Card-At $a7)
    $afterCrash = @(Tree-Cards $alpha | ForEach-Object { $_.address })
    $stranded = @($afterCrash | Where-Object { $addressesBefore -notcontains $_ })
    Assert ($stranded.Count -eq 1 -and $a7Before.body -match '来源' -and -not ($a7Before.body -match '同述入口')) 'the crash leaves the created statement and the old Card unchanged'
    $resume = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $lostPath, '-OperationPrefix', 'r/u/1/transfer')
    Assert ($resume.code -eq 0 -and $resume.last.status -eq 'applied') "the retry finishes the item: $($resume.text)"
    $resumeResult = Read-Json (Join-Path $work 'topic-home-result.json')
    $resumeItem = @($resumeResult.items)[0]
    Assert ($resumeItem.created.replayed -and $resumeItem.created.address -eq $stranded[0] -and -not $resumeItem.created.written) 'the retry replays the creation receipt instead of creating another copy'
    $after = @(Tree-Cards $alpha | ForEach-Object { $_.address })
    Assert ($after.Count -eq $afterCrash.Count) 'exactly one statement exists after the resume: no second copy'
    Assert ((Card-At $a7).body -match '## 同述入口' -and (Card-At $a7).body -match [regex]::Escape("[[$($resumeItem.created.address)]]")) 'the resumed run finishes the old-Card edit'

    # --- a concurrent base change: the created statement is preserved, the edit is not applied ---
    $racePath = Join-Path $work 'race.json'
    Write-Json $racePath @{ schema_version = 1; report_id = $review.report_id; decisions = @(
            @{ finding = $homeFinding.id; entry = $a8; remedy = 'entry'; note = '把制度这条陈述转成文献陈述。' }
        ) }
    $a8Before = Card-At $a8
    $env:ZT_CHECK_ZT = $seam
    $env:ZT_RACE_AFTER_CREATE = '1'
    $env:ZT_RACE_TARGET = $a8
    $env:ZT_RACE_LINE = '另一位写作者在复核之后改了这段。'
    $env:ZT_RACE_FLAG = Join-Path $scratch 'race.flag'
    try {
        $raced = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $racePath, '-OperationPrefix', 'r/u/1/transfer')
        Assert ($raced.code -eq 0 -and $raced.last.status -eq 'conflict') "a writer between the two writes is a conflict: $($raced.text)"
    } finally { Remove-Item Env:ZT_CHECK_ZT, Env:ZT_RACE_AFTER_CREATE, Env:ZT_RACE_TARGET, Env:ZT_RACE_LINE, Env:ZT_RACE_FLAG -ErrorAction SilentlyContinue }
    $racedResult = Read-Json (Join-Path $work 'topic-home-result.json')
    $racedItem = @($racedResult.items)[0]
    Assert ($racedItem.status -eq 'conflict' -and $racedItem.reason.code -eq 'source_changed') 'the changed old Card is a conflict, not permission to overwrite it'
    Assert ($racedItem.created.address -and $racedItem.rewritten.written -eq $false) 'the created statement is preserved and the old-Card edit is not applied'
    Assert ($racedItem.revised_review_item.required -match 'review the old Card' -and $racedItem.revised_review_item.current_fingerprint) 'the conflict produces a revised review item with the observed fingerprint'
    Assert ((Card-At $a8).body -match '另一位写作者在复核之后改了这段' -and -not ((Card-At $a8).body -match '同述入口')) "the old Card keeps the other writer's text"
    # Restoring the reviewed text lets the same accepted item finish on the next run.
    $null = Call-Tool 'edit_card' @{ at = $a8; title = $a8Before.title; body = $a8Before.body }
    $finished = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $racePath, '-OperationPrefix', 'r/u/1/transfer')
    Assert ($finished.code -eq 0 -and $finished.last.status -eq 'applied') "the restored Card finishes on the retry: $($finished.text)"
    $finishedResult = Read-Json (Join-Path $work 'topic-home-result.json')
    Assert (@($finishedResult.items)[0].created.replayed -and (Card-At $a8).body -match '同述入口') 'the retry reuses the created statement and finishes the old-Card edit'

    # --- a change during the item's own reads is a pending quiet window -------------------------
    $findingPath = Join-Path $work 'finding.json'
    Write-Json $findingPath @{ schema_version = 1; report_id = $review.report_id; decisions = @(
            @{ finding = $homeFinding.id; entry = $a9; remedy = 'entry'; note = '把承诺这条陈述转成文献陈述。' }
        ) }
    $a9Before = Card-At $a9
    $env:ZT_CHECK_ZT = $seam
    $env:ZT_RACE_AFTER_BULK_REPLY = '1'
    $env:ZT_RACE_TARGET = $a9
    $env:ZT_RACE_LINE = '另一位写作者在这条入口复核期间改了它。'
    $env:ZT_RACE_FLAG = Join-Path $scratch 'quiet.flag'
    $quietTreeBefore = @(Tree-Cards $alpha).Count
    try {
        $quiet = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $findingPath, '-OperationPrefix', 'r/u/1/transfer')
        Assert ($quiet.code -eq 0 -and $quiet.last.status -eq 'pending') "a change during the item's reads leaves it pending: $($quiet.text)"
    } finally { Remove-Item Env:ZT_CHECK_ZT, Env:ZT_RACE_AFTER_BULK_REPLY, Env:ZT_RACE_TARGET, Env:ZT_RACE_LINE, Env:ZT_RACE_FLAG -ErrorAction SilentlyContinue }
    $quietResult = Read-Json (Join-Path $work 'topic-home-result.json')
    Assert (@($quietResult.items)[0].reason.code -eq 'quiet_window_required' -and @($quietResult.items)[0].created -eq $null) 'the pending item names the quiet window and writes nothing'
    Assert (@(Tree-Cards $alpha).Count -eq $quietTreeBefore) 'a pending item creates no Card'

    # --- a changed finding is a conflict; a taken destination needs a new review ------------------
    $null = Call-Tool 'edit_card' @{ at = $a9; body = "$((Card-At $a9).body)`n`n来源：[[$beta]] §3" }
    $changed = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $findingPath, '-OperationPrefix', 'r/u/1/transfer')
    $changedResult = Read-Json (Join-Path $work 'topic-home-result.json')
    Assert ($changed.code -eq 0 -and @($changedResult.items)[0].reason.code -eq 'finding_changed') 'an entry that now names another book is a stale finding, not a transfer'
    $null = Call-Tool 'edit_card' @{ at = $a9; title = $a9Before.title; body = $a9Before.body }
    $taken = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $findingPath, '-OperationPrefix', 'r/u/1/transfer')
    $takenResult = Read-Json (Join-Path $work 'topic-home-result.json')
    Assert ($taken.code -eq 0 -and @($takenResult.items)[0].reason.code -eq 'destination_taken') 'a reviewed destination another run already used needs a new review'
    Assert (@($takenResult.items)[0].created -eq $null) 'a taken destination creates no Card'

    # --- an edited review or selection is refused by its own id ---------------------------------
    $reportPath = Join-Path $work 'topic-home-review.json'
    $saved = [IO.File]::ReadAllBytes($reportPath)
    [IO.File]::WriteAllText($reportPath, [Text.Encoding]::UTF8.GetString($saved).Replace('同述入口', '同述入口X'), $utf8)
    $tampered = Invoke-Digest 'topic-home' @('-Step', 'apply', '-Work', $work, '-File', $selectionPath, '-OperationPrefix', 'r/u/2/transfer')
    Assert ($tampered.code -ne 0 -and $tampered.text -match 'does not match its own report id') 'an edited review is refused, so no apply consumes stale text'
    [IO.File]::WriteAllBytes($reportPath, $saved)
    $restored = Invoke-Digest 'topic-home' @('-Step', 'plan', '-Work', $work, '-Report', (Join-Path $staging 'concept-review.json'))
    Assert ($restored.code -eq 0) 'a fresh plan over the restored review runs again'
} finally {
    Remove-Item Env:ZT_CHECK_ZT, Env:ZT_TRACE_TOOLS, Env:ZT_BULK_MISSING, Env:ZT_DROP_BATCH_REPLY_FLAG -ErrorAction SilentlyContinue
    zt down | Out-Null
}
if ($failed) { Write-Output "$failed assertion(s) failed"; exit 1 }
Write-Output 'topic-home: all assertions passed'
