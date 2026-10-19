#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\concepts.tests.ps1
# Drives `zt-digest concepts` (through scripts\book-digest\concept-compiler.ps1) on a scratch
# archive: prepare reads the book's term table and bounded Topic/statement candidates, judge
# validates canned answers, report writes one immutable concept-review.json/.md under the
# staging folder, and decision validates a reviewed gate decision against that report.
# Issue #97, design D063/D064/D076-D082/D287/D288/D300/D302.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "concepts-test-$PID"
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
$wrapper = (Join-Path $PSScriptRoot '..\concept-compiler.ps1') | Resolve-Path | ForEach-Object Path
$seam = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
# One concepts step through the forwarding script: its exit code, its output, and (on success) the
# last line as JSON.
function Invoke-Concepts([string[]]$Arguments) {
    $text = @(& pwsh -NoProfile -File $wrapper @Arguments 2>&1 | ForEach-Object { [string]$_ }) -join "`n"
    $code = $LASTEXITCODE
    $last = $null
    if (-not $code) { $last = ($text -split "`n")[-1] | ConvertFrom-Json -AsHashtable -DateKind String }
    [pscustomobject]@{ code = $code; text = $text; last = $last }
}
function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String }
function Write-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 20), $utf8) }
function New-Staging([string]$name, $proposals) {
    $staging = Join-Path $scratch $name
    New-Item -ItemType Directory -Force (Join-Path $staging 'concepts') | Out-Null
    Write-Json (Join-Path $staging 'manifest.json') @{ citation_key = $script:key; book_title = '概念复核'; groups = @(@{ n = 1; chapter = '第一章'; headings = @('均衡') }) }
    Write-Json (Join-Path $staging 'concepts\proposals.json') $proposals
    $staging
}
$script:key = 'Rev2026'
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $book = Write-Cards @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@book{$($script:key), title={Concept Review}, author={Owner, A.}, year={2026}}"; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = $script:key; kind = 'direct'; title = '第 1 组：均衡与学习'; body = '§1 - 概念' } }
    )
    # The book's term table: four rows, the two 学习 senses and their book-local rejected words.
    $table = @'
本卡是《概念复核》的术语表。本书的卡片用左列的术语，不用右列的词。

术语表

| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 均衡的多重性 | 多重均衡 | 同一个经济在给定条件下停在多个均衡的可能性 | §2.1 |
| 学习（个体行为） | 学习 | 个体重复练习带来的技能变化 | §3.1 |
| 学习（产业生产率） | 学习 | 产业整体的生产率随累计产量提高 | §4.2 |
| 剩余控制权 | 剩余控制 | 契约没有写明时决定资产用法的权利 | §1 |

来源：[[Rev2026]]
'@
    $termCard = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = "$($script:key)/1"; kind = 'side'; title = '术语表：《概念复核》的术语'; body = $table } }))[0]
    # Four statements of the book: one repeats the proposal's formula under stronger assumptions,
    # one is the individual-learning statement, one the industry one, one is unrelated.
    $null = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$($script:key)/1"; kind = 'side'; title = '对称参与者的重复博弈有两个均衡'; body = "当策略互补且参与者对称时，`$x_i=\arg\max_x u_i(x,x_{-i})`$ 有两个均衡。`n`n假设：策略互补、参与者对称。`n`n来源：[[Rev2026]] §2.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$($script:key)/1"; kind = 'side'; title = '同一个人练得越多技能越高'; body = "个体重复同一任务，技能随时间提高。`n`n假设：同一个体。`n`n来源：[[Rev2026]] §3.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$($script:key)/1"; kind = 'side'; title = '产业累计产量降低单位成本'; body = "产业累计产量每翻一倍，单位成本按固定比例下降。`n`n假设：同一产业。`n`n来源：[[Rev2026]] §4.2" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$($script:key)/1"; kind = 'side'; title = '债务契约把控制权交给债权人'; body = "违约时债权人取得资产的控制权。`n`n来源：[[Rev2026]] §1" } }
    )
    # Twelve Topics that share 均衡, so one concept has more candidates than the ten it may shortlist.
    $topics = Write-Cards @(
        @{ tool = 'create_topic'; arguments = @{ title = '多重均衡'; body = '本主题追问：什么条件下同一个经济有两个以上的均衡？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡的多重性'; body = '本主题追问：多重均衡从哪里来？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡选择'; body = '本主题追问：多个均衡里哪一个会出现？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡精炼'; body = '本主题追问：怎样排除不合理的均衡？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡的存在性'; body = '本主题追问：什么条件下均衡一定存在？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡的唯一性'; body = '本主题追问：什么条件下均衡只有一个？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡的稳定性'; body = '本主题追问：扰动之后均衡会回来吗？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡的比较'; body = '本主题追问：两个均衡怎样比较优劣？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡与效率'; body = '本主题追问：均衡是不是有效率的？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡与政策'; body = '本主题追问：政策怎样改变均衡？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡的计算'; body = '本主题追问：均衡怎样算出来？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡的例子'; body = '本主题追问：常见的均衡例子有哪些？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '学习'; body = '本主题追问：学习怎样改变行为？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '控制权'; body = '本主题追问：谁在契约没写的地方做决定？' } }
    )
    $multi = $topics[0] -replace '/0$', ''
    $learn = $topics[12] -replace '/0$', ''
    $null = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$multi/0"; title = '策略互补时两个均衡同时成立'; body = "策略互补让两个均衡同时成立。`n`n来源：[[Rev2026]] §2.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$learn/0"; title = '重复练习改变个体技能'; body = "同一个体重复练习，技能提高。`n`n来源：[[Rev2026]] §3.1" } }
    )
    $multiEntry = "$multi/1"
    # The canned answers for one frozen evidence set: every shortlisted pair judged, the two 学习
    # senses kept apart, the shared formula under different assumptions kept a difference.
    function New-Judgments($evidence, $multi, $learn) {
        $rows = @()
        foreach ($proposal in $evidence.proposals) {
            foreach ($c in @($proposal.topic_candidates)) {
                $same = $proposal.id -eq 'p001' -and $c.address -eq "$multi/0"
                $entry = if ($same) { "$multi/1" } else { $null }
                $learnDifferent = $c.address -eq "$learn/0"
                $rows += [ordered]@{ kind = 'topic-concept'; proposal = $proposal.id; candidate = $c.address; verdict = $(if ($same) { 'same' } else { 'different' })
                    scope = $(if ($same) { '策略互补时的均衡多重性' } else { $null }); assumptions = $(if ($same) { @('参与者同时选择策略') } else { @() })
                    entry = $entry; reason = $(if ($same) { '两张卡讨论同一个均衡条件与同一组问题。' } elseif ($learnDifferent) { '个体技能的改变与产业生产率的提高是两个不同的意思。' } else { '两个入口讨论的条件不同。' })
                    evidence = $(if ($same) { @("$multi/0", "$multi/1") } else { @($c.address) }) }
            }
            foreach ($s in @($proposal.restatement_candidates)) {
                $formula = $s.title -match '对称参与者'
                $rows += [ordered]@{ kind = 'restatement'; proposal = $proposal.id; candidate = $s.address; verdict = 'different'
                    reason = $(if ($formula) { '公式相同，但候选卡以参与者对称为前提，提议的陈述没有这个前提。' } else { '两处陈述的假设不同。' })
                    assumptions = @($s.assumptions); evidence = @($s.address) }
            }
        }
        , $rows
    }

    $proposals = @{ schema_version = 1; proposals = @(
            @{ title = '均衡的多重性'; question = '什么条件下同一个经济有多个均衡？'
                statement = '当策略互补时，同一个经济可以有多个均衡：$x_i=\arg\max_x u_i(x,x_{-i})$。'
                assumptions = @('策略互补'); terms = @('均衡的多重性') },
            @{ title = '学习（个体行为）'; question = '个体重复练习怎样改变他的技能？'
                statement = '个体重复同一任务会提高他的技能。'; assumptions = @('同一个体'); terms = @('学习（个体行为）') },
            @{ title = '学习（产业生产率）'; question = '产业整体的生产率为什么随累计产量提高？'
                statement = '产业累计产量提高会降低单位成本。'; assumptions = @('同一产业'); terms = @('学习（产业生产率）') }
        ) }
    $staging = New-Staging 'staging' $proposals
    $work = Join-Path $staging 'concepts'

    # --- prepare: the term rows, the bounded shortlists and the frozen evidence -----------------
    $prepare = Invoke-Concepts @('-Step', 'prepare', '-Staging', $staging)
    Assert ($prepare.code -eq 0) "prepare runs: $($prepare.text)"
    $p = $prepare.last
    Assert ($p.ok -and $p.archive_id -match '^[0-9a-f]{32}$' -and $p.archive_revision -is [long]) 'prepare binds the archive identity and revision'
    Assert ($p.proposals -eq 3 -and $p.term_rows -eq 4) "prepare reads three proposals and the book's four term rows ($($p.proposals), $($p.term_rows))"
    $evidence = Read-Json (Join-Path $work 'evidence.json')
    Assert ($evidence.evidence_id -match '^e-[0-9a-f]{64}$') 'the frozen evidence carries its canonical id'
    $p1 = @($evidence.proposals | Where-Object { $_.id -eq 'p001' })[0]
    Assert ($p1.term.term -eq '均衡的多重性' -and $p1.term.meaning -match '多个均衡' -and $p1.term.defined -eq '§2.1' -and @($p1.term.avoid) -contains '多重均衡') 'the checker term-table parser supplies 术语, 不用, 意思 and 首次定义'
    Assert (@($evidence.term_table.rows).Count -eq 4 -and $evidence.term_table.address -eq $termCard) 'the whole four-column table is retained with its Card address'
    Assert (@($p1.topic_candidates).Count -eq 10 -and @($p1.topic_candidates | Where-Object { $_.title -eq '多重均衡' }).Count -eq 1) 'the 均衡 concept gets exactly ten Topic candidates, the matching one among them'
    Assert ($evidence.coverage.topic_unselected -ge 2 -and @($evidence.proposals | Where-Object { @($_.topic_candidates).Count -gt 10 }).Count -eq 0) 'no proposal exceeds the ten-Topic shortlist and the unselected ones are reported'
    Assert (@($p1.topic_candidates | Where-Object { $_.title -eq '控制权' }).Count -eq 0) 'a Topic that shares no evidence with the proposal is not a candidate'
    $cand = @($p1.topic_candidates | Where-Object { $_.title -eq '多重均衡' })[0]
    Assert (@($cand.entries).Count -eq 1 -and @($cand.own_book) -contains $script:key) 'a candidate carries its representative Cards and the own-book dependency'
    $rest = @($p1.restatement_candidates | Where-Object { $_.title -match '对称参与者' })[0]
    Assert ($null -ne $rest -and $rest.formula_match -and @($rest.formulas) -contains '$x_i=\arg\max_x u_i(x,x_{-i})$' -and @($rest.assumptions).Count -ge 1) 'a restatement candidate keeps the statement, its assumptions, its formulas and the source evidence'
    Assert (@($p1.restatement_candidates).Count -le 10 -and $evidence.coverage.restatement_selected -eq @($evidence.proposals | ForEach-Object { @($_.restatement_candidates).Count } | Measure-Object -Sum).Sum) 'restatement candidates are bounded and their coverage is recorded'
    $job = Get-Content -LiteralPath (Join-Path $work 'cj-01.md') -Raw -Encoding utf8
    Assert ($job -match '均衡的多重性' -and $job -match '首次定义' -and $job -match '多重均衡' -and $job -match 'SELF-CHECK') 'the judge job shows the preferred word, the rejected word, the meaning, the book and the candidates together'
    Assert (Test-Path -LiteralPath (Join-Path $work 'dsh-concept-instructions.zh.md')) 'the prompt file is copied beside the job'

    # --- judge: canned answers, refused when malformed -----------------------------------------
    $out = Join-Path $work 'out'
    New-Item -ItemType Directory -Force $out | Out-Null
    $answerFile = Join-Path $out 'cj-01.json'
    $judgments = New-Judgments $evidence $multi $learn
    Write-Json $answerFile @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = $judgments }
    $judge = Invoke-Concepts @('-Step', 'judge', '-Staging', $staging, '-File', $answerFile)
    Assert ($judge.code -eq 0 -and $judge.last.judged -eq $judgments.Count -and $judge.last.unjudged -eq 0) "judge accepts the canned answers ($($judgments.Count) prepared, refused: $($judge.text))"
    Assert (Test-Path -LiteralPath (Join-Path $work 'judged.json')) 'judge freezes the accepted answers'

    $bad = Join-Path $out 'bad.json'
    [IO.File]::WriteAllText($bad, '{"schema_version":1,"evidence_id":"e-0","evidence_id":"e-1","judgments":[]}', $utf8)
    $duplicate = Invoke-Concepts @('-Step', 'judge', '-Staging', $staging, '-File', $bad)
    Assert ($duplicate.code -ne 0 -and $duplicate.text -match 'duplicate JSON key') 'judge refuses an answer file with a duplicate key'
    Write-Json $bad @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = @(
            @{ kind = 'topic-concept'; proposal = 'p001'; candidate = "$multi/0"; verdict = 'equivalent'; reason = 'x'; evidence = @("$multi/0") }) }
    $unknown = Invoke-Concepts @('-Step', 'judge', '-Staging', $staging, '-File', $bad)
    Assert ($unknown.code -ne 0 -and $unknown.text -match 'verdict') 'judge refuses an unknown verdict'
    Write-Json $bad @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = @(
            @{ kind = 'topic-concept'; proposal = 'p001'; candidate = "$multi/0"; verdict = 'same'; scope = 'x'; reason = 'x'; evidence = @("$multi/0") },
            @{ kind = 'topic-concept'; proposal = 'p001'; candidate = "$multi/0"; verdict = 'different'; reason = 'y'; evidence = @("$multi/0") }) }
    $contradiction = Invoke-Concepts @('-Step', 'judge', '-Staging', $staging, '-File', $bad)
    Assert ($contradiction.code -ne 0 -and $contradiction.text -match 'contradict') 'judge refuses two outcomes for the same pair'
    Write-Json $bad @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = @(
            @{ kind = 'restatement'; proposal = 'p001'; candidate = "$($rest.address)"; verdict = 'different'; reason = 'x'; evidence = @() }) }
    $missing = Invoke-Concepts @('-Step', 'judge', '-Staging', $staging, '-File', $bad)
    Assert ($missing.code -ne 0 -and $missing.text -match 'evidence') 'judge refuses a judgment with no evidence'
    Write-Json $bad @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = @(
            @{ kind = 'restatement'; proposal = 'p001'; candidate = "$($rest.address)"; verdict = 'different'; reason = 'x'; evidence = @("$script:key/999") }) }
    $unread = Invoke-Concepts @('-Step', 'judge', '-Staging', $staging, '-File', $bad)
    Assert ($unread.code -ne 0 -and $unread.text -match 'read|evidence') 'judge refuses evidence that names an endpoint the review did not read'
    Write-Json $bad @{ schema_version = 1; evidence_id = 'e-0000000000000000000000000000000000000000000000000000000000000000'; judgments = @() }
    $stale = Invoke-Concepts @('-Step', 'judge', '-Staging', $staging, '-File', $bad)
    Assert ($stale.code -ne 0 -and $stale.text -match 'evidence') 'judge refuses answers for another frozen evidence set'

    # --- report: one immutable report, zero Card writes ----------------------------------------
    $env:ZT_TRACE_TOOLS = Join-Path $scratch 'report-tools.jsonl'
    [IO.File]::WriteAllText($env:ZT_TRACE_TOOLS, '')
    $report = Invoke-Concepts @('-Step', 'report', '-Staging', $staging)
    Assert ($report.code -eq 0 -and $report.last.written) "report writes the review: $($report.text)"
    $json = Join-Path $staging 'concept-review.json'
    $md = Join-Path $staging 'concept-review.md'
    Assert ((Test-Path -LiteralPath $json) -and (Test-Path -LiteralPath $md) -and $report.last.paths.Count -eq 2) 'the review is concept-review.json and concept-review.md under the staging folder'
    Assert (@(Get-Content -LiteralPath $env:ZT_TRACE_TOOLS -Encoding utf8 | Where-Object { $_ }).Count -eq 0) 'report generation makes no tool call, so no Card write'
    $review = Read-Json $json
    Assert ($review.schema_version -eq 1 -and $review.report_id -match '^r-[0-9a-f]{64}$' -and $review.archive.id -eq $p.archive_id -and $review.archive.revision -eq $p.archive_revision) 'the report binds its schema, archive identity, revision and canonical id'
    Assert ($review.bounds.topic_candidates -eq 10 -and $review.bounds.restatement_candidates -eq 10 -and $review.coverage.topic_unselected -ge 2) 'the report states its selection bounds and the unselected coverage'
    Assert ($review.completeness.complete -and $review.completeness.judged -eq $judgments.Count -and $review.completeness.unjudged -eq 0 -and $review.status -eq 'complete') 'a fully judged review is complete'
    Assert (@($review.findings.id) -contains 'c001' -and @($review.findings | Where-Object { $_.id -notmatch '^c\d{3}$' }).Count -eq 0) 'findings carry report-local ids'
    $same = @($review.findings | Where-Object { $_.kind -eq 'topic-concept' -and $_.verdict -eq 'same' })[0]
    Assert ($same.proposal -eq 'p001' -and $same.candidate -eq "$multi/0" -and @($same.addresses) -contains "$multi/1" -and $same.scope -match '策略互补' -and @($same.remedies | Where-Object { $_.action -eq 'reuse_topic' }).Count -eq 1) 'a same verdict names its scope, addresses and the reuse remedy'
    $formulaFinding = @($review.findings | Where-Object { $_.kind -eq 'restatement' -and $_.candidate -eq $rest.address })[0]
    Assert ($formulaFinding.verdict -eq 'different' -and $formulaFinding.formula_match -and @($formulaFinding.assumptions).Count -ge 1 -and @($formulaFinding.remedies.action) -contains 'retain_both') 'a shared formula under different assumptions stays a difference with both statements retained'
    $senses = @($review.findings | Where-Object { $_.candidate -eq "$learn/0" -and $_.kind -eq 'topic-concept' })
    Assert ($senses.Count -eq 2 -and (@($senses.proposal) -contains 'p002') -and (@($senses.proposal) -contains 'p003') -and @($senses | Where-Object { $_.verdict -eq 'different' }).Count -eq 2) 'individual and industrial learning remain two distinct findings'
    Assert (@($review.findings | Where-Object { $_.kind -eq 'topic-home' }).Count -ge 1) 'a Topic whose entries depend on their own book is reported as a topic-home finding'
    Assert (@($review.unresolved).Count -eq 0 -and (@(Get-Content -LiteralPath $md -Raw -Encoding utf8) -match 'c001')) 'the rendered review names the findings'
    $before = (Get-FileHash -LiteralPath $json -Algorithm SHA256).Hash
    $again = Invoke-Concepts @('-Step', 'report', '-Staging', $staging)
    Assert ($again.code -eq 0 -and $again.last.report_id -eq $review.report_id -and -not $again.last.written -and (Get-FileHash -LiteralPath $json -Algorithm SHA256).Hash -eq $before) 'the same frozen evidence gives the same report id and changes no file'
    Remove-Item Env:ZT_TRACE_TOOLS
    # An edited evidence file is not the frozen set any more: the id no longer matches, and no step
    # reads it as if it were reviewed.
    $evidencePath = Join-Path $work 'evidence.json'
    $saved = [IO.File]::ReadAllBytes($evidencePath)
    [IO.File]::WriteAllText($evidencePath, [Text.Encoding]::UTF8.GetString($saved).Replace('均衡的多重性', '均衡的多重性X'), $utf8)
    $tampered = Invoke-Concepts @('-Step', 'report', '-Staging', $staging)
    Assert ($tampered.code -ne 0 -and $tampered.text -match 'does not match its evidence id') 'an edited evidence file is refused by its own id, so a report cannot be built on it'
    [IO.File]::WriteAllBytes($evidencePath, $saved)

    # --- decision: only this report's findings and their supported verdicts --------------------
    $decisionFile = Join-Path $work 'reviewed.json'
    $decide = { param($value, $at) if (-not $at) { $at = $staging }; Write-Json $decisionFile $value; Invoke-Concepts @('-Step', 'decision', '-Staging', $at, '-File', $decisionFile) }
    $wrongId = & $decide @{ schema_version = 1; report_id = ($review.report_id -replace '.$', $(if ($review.report_id.EndsWith('0')) { '1' } else { '0' })); decisions = @(@{ finding = 'c001'; action = 'reuse_topic'; verdict = 'same'; note = '复用。' }) }
    Assert ($wrongId.code -ne 0 -and $wrongId.text -match 'report') 'decision refuses another report id'
    $noFinding = & $decide @{ schema_version = 1; report_id = $review.report_id; decisions = @(@{ finding = 'c999'; action = 'reuse_topic'; verdict = 'same'; note = '复用。' }) }
    Assert ($noFinding.code -ne 0 -and $noFinding.text -match 'finding') 'decision refuses an unknown finding id'
    $badVerdict = & $decide @{ schema_version = 1; report_id = $review.report_id; decisions = @(@{ finding = $same.id; action = 'keep_distinct'; verdict = 'different'; note = '区分。' }) }
    Assert ($badVerdict.code -ne 0 -and $badVerdict.text -match 'verdict') 'decision refuses a verdict the report does not support'
    $badAction = & $decide @{ schema_version = 1; report_id = $review.report_id; decisions = @(@{ finding = $formulaFinding.id; action = 'reuse_topic'; verdict = 'different'; note = '复用。' }) }
    Assert ($badAction.code -ne 0 -and $badAction.text -match 'action') 'decision refuses an action the finding does not offer'
    $badHash = & $decide @{ schema_version = 1; report_id = $review.report_id; decisions = @(@{ finding = $same.id; action = 'reuse_topic'; verdict = 'same'; note = '复用。'; fingerprints = @{ "$multi/0" = '0000000000000000000000000000000000000000000000000000000000000000' } }) }
    Assert ($badHash.code -ne 0 -and $badHash.text -match 'fingerprint') 'decision refuses a fingerprint that differs from the reviewed endpoint'
    $accepted = & $decide @{ schema_version = 1; report_id = $review.report_id; decisions = @(@{ finding = $same.id; action = 'reuse_topic'; verdict = 'same'; note = '复用既有的主题入口。' }) }
    Assert ($accepted.code -eq 0 -and $accepted.last.accepted -eq 1 -and $accepted.last.decision_id -match '^d-[0-9a-f]{64}$') 'decision accepts a reviewed decision that binds the report id and the finding'
    $decision = Read-Json (Join-Path $work 'decision.json')
    Assert ($decision.report_id -eq $review.report_id -and @($decision.decisions)[0].finding -eq $same.id) 'the accepted decision is retained with its report id'
    # A report file edited after the review no longer matches its own content id, so no decision
    # binds it (the wrong-hash refusal of 11.3).
    $reportPath = Join-Path $staging 'concept-review.json'
    $savedReport = [IO.File]::ReadAllBytes($reportPath)
    [IO.File]::WriteAllText($reportPath, [Text.Encoding]::UTF8.GetString($savedReport).Replace('c001', 'c901'), $utf8)
    $tamperedReport = & $decide @{ schema_version = 1; report_id = $review.report_id; decisions = @(@{ finding = 'c001'; action = 'reuse_topic'; verdict = 'same'; note = '复用。' }) }
    Assert ($tamperedReport.code -ne 0 -and $tamperedReport.text -match 'report id') 'an edited report file is refused by its own report id'
    [IO.File]::WriteAllBytes($reportPath, $savedReport)
    $restored = Invoke-Concepts @('-Step', 'report', '-Staging', $staging)
    Assert ($restored.code -eq 0 -and $restored.last.report_id -eq $review.report_id) 'the restored report is the reviewed one again'

    # --- incomplete reads: an endpoint that could not be read stays visible -------------------
    $unreadStaging = New-Staging 'staging-unread' @{ schema_version = 1; proposals = @(@{ title = '均衡的多重性'; question = '什么条件下同一个经济有多个均衡？'
                statement = '当策略互补时，同一个经济可以有多个均衡。'; terms = @('均衡的多重性') }) }
    $env:ZT_CHECK_ZT = $seam
    $env:ZT_BULK_MISSING = $multiEntry
    try {
        $partial = Invoke-Concepts @('-Step', 'prepare', '-Staging', $unreadStaging)
        Assert ($partial.code -eq 0 -and $partial.last.unread -eq 1) 'prepare keeps running when a candidate Card cannot be read, and names it'
        $partialEvidence = Read-Json (Join-Path $unreadStaging 'concepts\evidence.json')
        $inner = @($partialEvidence.proposals | Where-Object { $_.id -eq 'p001' })
        $pair = @($inner.topic_candidates | Where-Object { $_.address -eq "$multi/0" })[0]
        $unreadAnswer = Join-Path $unreadStaging 'concepts\out\cj-01.json'
        New-Item -ItemType Directory -Force (Split-Path -Parent $unreadAnswer) | Out-Null
        Write-Json $unreadAnswer @{ schema_version = 1; evidence_id = $partialEvidence.evidence_id; judgments = @(
                @{ kind = 'topic-concept'; proposal = 'p001'; candidate = "$multi/0"; verdict = 'same'; scope = 'x'; reason = 'x'; evidence = @("$multi/0") }) }
        $refused = Invoke-Concepts @('-Step', 'judge', '-Staging', $unreadStaging, '-File', $unreadAnswer)
        Assert ($refused.code -ne 0 -and $refused.text -match 'read') 'judge refuses a verdict on an endpoint that was not read'
        Write-Json $unreadAnswer @{ schema_version = 1; evidence_id = $partialEvidence.evidence_id; judgments = @() }
        $judgedEmpty = Invoke-Concepts @('-Step', 'judge', '-Staging', $unreadStaging, '-File', $unreadAnswer)
        $partialReport = Invoke-Concepts @('-Step', 'report', '-Staging', $unreadStaging)
        Assert ($judgedEmpty.code -eq 0 -and $partialReport.code -eq 0 -and -not $partialReport.last.complete -and $partialReport.last.status -eq 'partial') 'a report over an unread endpoint and unjudged pairs is partial'
        $partialReview = Read-Json (Join-Path $unreadStaging 'concept-review.json')
        Assert ((-not $partialReview.completeness.complete) -and @($partialReview.completeness.unread).Count -eq 1 -and $partialReview.completeness.unjudged -ge 1) 'the partial review states its unread endpoint and its unjudged candidates'
        Assert (@($partialReview.unresolved).Count -ge 1 -and @($partialReview.findings | Where-Object { $_.unjudged }).Count -ge 1) 'unjudged pairs become uncertain findings with what must be read, not a clean result'
        $emptyDecision = & $decide @{ schema_version = 1; report_id = $partialReview.report_id; decisions = @(@{ finding = @($partialReview.findings)[0].id; action = 'reuse_topic'; verdict = 'uncertain'; note = 'x' }) } $unreadStaging
        Assert ($emptyDecision.code -ne 0) 'decision refuses a finding the partial report leaves unresolved'
    } finally { Remove-Item Env:ZT_BULK_MISSING -ErrorAction SilentlyContinue }
    # A read the archive answered as missing for a Card that never became a candidate: the frozen
    # evidence records it, so no complete review can be declared over it (D082) even when every
    # shortlisted pair is judged.
    $hiddenStaging = New-Staging 'staging-hidden' $proposals
    # 控制权: the one Topic no proposal names, so it can only be an unread catalogue entry.
    $hiddenAddress = $topics[13]
    Assert ($hiddenAddress -match '^\d+/0$') "the hidden Topic root has a Topic address ($hiddenAddress)"
    $env:ZT_CHECK_ZT = $seam
    $env:ZT_BULK_MISSING = $hiddenAddress
    try {
        $hidden = Invoke-Concepts @('-Step', 'prepare', '-Staging', $hiddenStaging)
        Assert ($hidden.code -eq 0 -and $hidden.last.unread -eq 1) "prepare records a read the archive answered as missing: $($hidden.text)"
        $hiddenEvidence = Read-Json (Join-Path $hiddenStaging 'concepts\evidence.json')
        $hiddenAnswer = Join-Path $hiddenStaging 'concepts\out\cj-01.json'
        New-Item -ItemType Directory -Force (Split-Path -Parent $hiddenAnswer) | Out-Null
        Write-Json $hiddenAnswer @{ schema_version = 1; evidence_id = $hiddenEvidence.evidence_id; judgments = (New-Judgments $hiddenEvidence $multi $learn) }
        $hiddenJudge = Invoke-Concepts @('-Step', 'judge', '-Staging', $hiddenStaging, '-File', $hiddenAnswer)
        $hiddenReport = Invoke-Concepts @('-Step', 'report', '-Staging', $hiddenStaging)
        Assert ($hiddenJudge.code -eq 0 -and $hiddenReport.code -eq 0) "the judged review over the incomplete set renders: $($hiddenJudge.text) $($hiddenReport.text)"
        Assert ($hiddenReport.last.judged -eq $judgments.Count -and $hiddenReport.last.unjudged -eq 0 -and -not $hiddenReport.last.complete -and $hiddenReport.last.status -eq 'partial') 'every shortlisted pair judged, yet the Card the review could not read keeps it partial'
        $hiddenReview = Read-Json (Join-Path $hiddenStaging 'concept-review.json')
        Assert (@($hiddenReview.completeness.unread).Count -eq 1 -and @($hiddenReview.completeness.unread) -contains $hiddenAddress) 'the partial review names the address it could not read'
        Assert (@($hiddenReview.completeness.reasons) -contains 'unread_endpoint' -and @($hiddenReview.unresolved | Where-Object { $_ -match [regex]::Escape($hiddenAddress) }).Count -ge 1) 'the missed read is named among the reasons and the unresolved items'
    } finally { Remove-Item Env:ZT_BULK_MISSING -ErrorAction SilentlyContinue }

    # --- a changing archive revision: retried once, then a fault -------------------------------
    $driftStaging = New-Staging 'staging-drift' @{ schema_version = 1; proposals = @(@{ title = '均衡的多重性'; question = '什么条件下同一个经济有多个均衡？'; terms = @('均衡的多重性') }) }
    $env:ZT_DRIFT_REVISION = 'once'
    try {
        $retried = Invoke-Concepts @('-Step', 'prepare', '-Staging', $driftStaging)
        Assert ($retried.code -eq 0 -and (Test-Path -LiteralPath (Join-Path $driftStaging 'concepts\evidence.json'))) 'a view that settles after one retry still produces the evidence'
    } finally { Remove-Item Env:ZT_DRIFT_REVISION }
    $faultStaging = New-Staging 'staging-fault' @{ schema_version = 1; proposals = @(@{ title = '均衡的多重性'; question = '什么条件下同一个经济有多个均衡？'; terms = @('均衡的多重性') }) }
    $env:ZT_DRIFT_REVISION = '1'
    try {
        $fault = Invoke-Concepts @('-Step', 'prepare', '-Staging', $faultStaging)
        Assert ($fault.code -ne 0 -and $fault.text -match 'changing read view' -and -not (Test-Path -LiteralPath (Join-Path $faultStaging 'concepts\evidence.json'))) 'a view that keeps changing is a changing-view fault that writes no evidence'
    } finally { Remove-Item Env:ZT_DRIFT_REVISION, Env:ZT_CHECK_ZT }
} finally { zt down | Out-Null }
if ($failed) { Write-Output "$failed assertion(s) failed"; exit 1 }
Write-Output 'concepts: all assertions passed'
