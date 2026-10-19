#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\audit-baseline.tests.ps1
# #93 S01 (#94): the reproducible baseline at the audit's public boundary. A real scratch archive,
# the fixed manual task sheet, a reviewed concept report with its decision, a run report, and an
# earlier audit as the baseline. Read-only throughout: every run changes no Card.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "audit-baseline-test-$PID"
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition, [string]$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String }
# One audit through the public script, with its report; a failed run throws.
function Audit($out, [string[]]$more = @()) {
    $lines = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -Out $out -Sample 3 @more 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "audit failed: $($lines -join ' | ')" }
    Read-Json (Join-Path $out 'audit.json')
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    # --- the fixture: two books, a model with a part that has no source of its own, Topic Cards
    # with one, two, and no explicit source books, a hub whose lines cross books, a hub that
    # copies its anchor's title, a register with a valid and a malformed row, and title pairs.
    # A Link target must exist when the Card is written, so every body names an earlier Card.
    $made = Write-Cards @(
        @{tool='create_literature';arguments=@{bibtex='@book{A2020,title={Alpha},year={2020}}'}},                          # 0  A2020
        @{tool='create_successor';arguments=@{at='A2020';title='第一组';body="§1 甲`n`n§2 乙"}},                              # 1  A2020/1
        @{tool='create_literature';arguments=@{bibtex='@book{B2020,title={Beta},year={2020}}'}},                          # 2  B2020
        @{tool='create_successor';arguments=@{at='B2020';title='第一组';body='§1 丙'}},                                      # 3  B2020/1
        @{tool='create_successor';arguments=@{at='@@1';kind='side';type='model';title='甲的模型';body='概要说：甲决定乙。'}}, # 4  head
        @{tool='create_successor';arguments=@{at='@@4';kind='side';type='model';facet='设定';title='模型设定';body="对应 [[@@3]]：两处都由融资约束决定投资。"}}, # 5 part
        @{tool='create_topic';arguments=@{title='剩余控制权';body='谁在契约空白处做决定？'}},                                  # 6  N/0
        @{tool='create_successor';arguments=@{at='@@6';title='剩余控制权决定投资';body="依赖 [[@@1]]：所有者能拒绝合作。`n`n来源：[[@@0]] §1"}}, # 7 N/1
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='剩余控制权决定投资水平';body="依赖 [[@@3]] 的设定。`n`n来源：[[@@0]] §2"}}, # 8 N/1|a
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='剩余控制权提高谈判地位';body="关联：这一条把谈判地位与事后分配连起来。`n`n来源：[[@@0]] §1"}}, # 9
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='专用性投资不可转用';body="对应 [[@@3]]：另一本书的同一机制。"}}, # 10 unknown
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='两本书都给出这一前提';body="两处都给出这一前提。`n`n来源：[[@@0]] §1`n`n来源：[[@@2]] §2"}}, # 11 multiple
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='另一本书的机制';body="对应 [[@@3]]：同一机制。`n`n来源：[[@@2]] §1"}}, # 12 cross-book line target
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='同一标题的两张卡';body='这一张没有写出处。'}},         # 13 idea exact pair
        @{tool='create_successor';arguments=@{at='@@7';kind='side';type='hub';level='topic';title='剩余控制权枢纽';body="范围：剩余控制权的入口。`n`n## 主线`n- 依赖 [[@@1]]：本书的设定。`n- 对应 [[@@12]]：另一本书的同一机制。"}}, # 14 topic hub
        @{tool='create_topic';arguments=@{title='专用性投资';body='投资不可转用吗？'}},                                        # 15 Q/0
        @{tool='create_successor';arguments=@{at='@@15';title='专用性投资与契约';body='说明这一话题的边界。'}},                  # 16 Q/1
        @{tool='create_successor';arguments=@{at='@@16';kind='side';title='同一标题的两张卡';body='这一张也没有写出处。'}},      # 17 idea exact pair
        @{tool='create_successor';arguments=@{at='@@16';type='hub';level='line';title='专用性投资与契约';body="## 主线`n- 依赖 [[@@7]]：相关的观点。"}}, # 18 anchor-title + navigation title
        @{tool='create_topic';arguments=@{title='契约的边界';body='契约到哪里为止？'}},                                        # 19 Q2/0
        @{tool='create_successor';arguments=@{at='@@19';title='契约边界说明';body='说明契约的边界。'}},                          # 20 Q2/1
        @{tool='create_successor';arguments=@{at='@@20';kind='side';type='hub';level='line';title='专用性投资与契约';body="## 主线`n- 依赖 [[@@7]]：同一个话题的另一处入口。"}}, # 21 navigation exact pair
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='事后剩余由地位决定';body='这一结果取决于所有权。'}},    # 22 W
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='两种来源给出同一结果';body='这一结果在两种设定下相同。'}}, # 23 Y2
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='谈判地位由所有权决定';body="对应 [[@@22]]：谈判地位随所有权改变。`n`n来源：[[@@0]] §1"}}, # 24 Y1
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='剩余控制权分配事后剩余';body="依赖 [[@@24]]：事后分配由地位决定。`n`n对应 [[@@23]]：两种来源给出同一结果。`n`n来源：[[@@0]] §1"}}, # 25 Z, the register's entry target
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='式 (12) 有该项';body="式 (12) 有该项。`n`n来源：[[@@0]] §1"}},   # 26 R20 advice
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='该卡给出了结论';body="该卡给出了结论。`n`n来源：[[@@0]] §1"}},     # 27 R21 advice
        @{tool='create_successor';arguments=@{at='@@7';kind='side';type='model';facet='设定';title='共享的设定';body='这个设定在两本书里都用。'}}, # 28 a part in a Topic tree: no source of its own
        @{tool='create_successor';arguments=@{at='@@7';kind='side';title='跨书桥接的观点';body="跨书关联：对应 [[@@2]] 与 [[@@3]]，两本书给出同一机制。`n`n来源：[[@@0]] §1"}} # 29 a Bridge that changes the inferred book
    )
    # The Topic-filed part belongs to the book's model because its Model Card names it (D071).
    $shared = Write-Cards @(@{tool='edit_card';arguments=@{at=$made[4];body="概要说：甲决定乙。`n`n设定见 [[$($made[28])]]。"}})
    Assert ($shared[0] -ceq $made[4]) 'the Model Card takes the Topic-filed part into its model'
    # The register is the first Card of the Topic 索引, and its row names the entry target above.
    $index = Write-Cards @(
        @{tool='create_topic';arguments=@{title='索引';body='代理工作区的入口。'}},
        @{tool='create_successor';arguments=@{at='@@0';type='hub';level='register';title='索引：代理工作区的入口';body="范围：代理工作区的全部入口。`n`n## 词条`n- 剩余控制权：[[$($made[25])]]`n- 过长的行：[[$($made[1])]] [[$($made[3])]] [[$($made[7])]] [[$($made[8])]]"}}
    )
    $firstOut = Join-Path $scratch 'first'
    $first = Audit $firstOut
    $e = $first.baseline.evidence
    # --- S01-AC01: the evidence a reader needs to reproduce this report.
    Assert ($first.baseline.schema_version -eq 1 -and $first.baseline.complete -eq $false) 'the baseline section exists and is not complete without its named inputs'
    Assert ($e.archive.id -match '^[0-9a-f]{32}$' -and $e.archive.revision -eq $first.archive_revision -and $e.archive.path) 'the report records the archive identity at the revision it measured'
    Assert ($e.software.zt_digest -and $e.software.zt_check -and $e.software.zt -and $e.software.rules) "the report records the software and rule versions: $($e.software | ConvertTo-Json -Compress)"
    Assert ($e.thresholds.max_sentences -eq 12 -and $e.thresholds.max_chars -eq 400 -and $e.thresholds.max_steps -eq 6 -and $e.thresholds.sample -eq 3 -and $e.thresholds.sample_seed -eq 1 -and $e.thresholds.near_title -eq 0.8) 'the report records the effective thresholds, the sample size, the seed and the near-title threshold'
    Assert ($e.scope.kind -eq 'workspace' -and $e.scope.name -eq 'agent' -and $e.scope.cards -ge 25 -and $e.scope.trees -ge 4 -and @($e.scope.books).Count -eq 2) "the report records its exact scope: $($e.scope | ConvertTo-Json -Compress)"
    $sample = $first.baseline.evidence.sample
    Assert ($sample.seed -eq 1 -and $sample.drawn -eq 3 -and @($sample.addresses).Count -eq 3 -and $sample.population -ge @($sample.addresses).Count) 'the sample records its seed, its population and the addresses it drew'
    $again = Audit (Join-Path $scratch 'again')
    Assert ((($again.baseline.evidence.sample.addresses) -join ',') -ceq (($sample.addresses) -join ',')) 'the same seed draws the same sample addresses'
    foreach ($at in @($sample.addresses)) { $card = (Invoke-Zt @(@{tool='get_card';arguments=@{at=$at}}))[0]; if (-not $card) { throw "sampled address $at is not a Card" } }
    Assert $true 'every sampled address is a Card of the archive'
    $missingKinds = @($first.baseline.evidence.missing | ForEach-Object { $_.kind } | Sort-Object)
    Assert (($missingKinds -join ',') -ceq 'baseline,concept_report,run_report,task_sheet') "the missing evidence is named, not left as a zero: $($missingKinds -join ',')"
    Assert ($first.baseline.measures.P1.known -eq $false -and $first.baseline.measures.P1.reason -match 'no baseline') 'a measure whose interval is unknown says so instead of reporting a number'
    Assert ($first.baseline.measures.P3_P5.filing.known -eq $false -and $first.baseline.measures.P3_P5.topics.known -eq $true) 'the mixed-source filing needs the interval while the multi-book Topic count does not'
    # --- S01-AC02: the explicit source set beside the inferred attribution.
    $s = $first.baseline.sources
    Assert ($s.cards.known -ge 5 -and $s.cards.unknown -ge 3 -and $s.cards.multiple -eq 1 -and $s.cards.derived -eq 1) "the source sets keep known, unknown, multiple and derived apart: $($s.cards | ConvertTo-Json -Compress)"
    Assert ($s.direct.literature_tree -ge 2 -and $s.direct.source_line -ge 3) 'a Literature tree and a 来源 line are both counted as direct evidence'
    $unknownRow = @($s.differences | Where-Object { $_.address -ceq $made[10] })[0]
    Assert ($null -ne $unknownRow -and $unknownRow.inferred -ceq 'B2020' -and @($unknownRow.sources).Count -eq 0) 'a semantic Link to another book is the inferred owner but no source evidence, and the difference is shown'
    $sourcesFile = Read-Json (Join-Path $firstOut 'audit-sources.json')
    $partRow = @($sourcesFile.cards | Where-Object { $_.address -ceq $made[28] })[0]
    Assert ($partRow.sources.derived -contains 'A2020' -and @($partRow.sources.derived_from).Count -eq 1 -and $partRow.sources.derived_from[0] -ceq $made[4]) 'a Model part with no direct source borrows its model head evidence and names the head'
    $literalRow = @($sourcesFile.cards | Where-Object { $_.address -ceq $made[11] })[0]
    Assert (@($literalRow.sources.direct).Count -eq 2 -and $literalRow.sources.multiple -eq $true) 'two 来源 books stay multiple, never forced to one owner'
    # A Bridge (a semantic Link to another book) moves the *inferred* attribution, and the report
    # shows both readings so an already reviewed meaning cannot silently reverse (D074, D075).
    $bridgeRow = @($s.differences | Where-Object { $_.address -ceq $made[29] })[0]
    Assert ($null -ne $bridgeRow -and $bridgeRow.inferred -ceq 'B2020' -and (@($bridgeRow.sources) -join ',') -ceq 'A2020') 'a Bridge changes the inferred book while the explicit source evidence stays A2020, and the difference is shown'
    # --- S01-AC03: the measures that this read can answer.
    $p1 = $first.baseline.measures.P1
    Assert ($p1.values.current.value -eq 1 -and $p1.values.current.with_rests_on -eq 3 -and $p1.values.current.unknown_source -ge 1 -and $p1.values.current.denominator -ge 12) "the current-scope own-source rests-on is counted with its denominator: $($p1.values.current | ConvertTo-Json -Compress)"
    $p4 = $first.baseline.measures.P4
    Assert ($p4.tails.value -eq 1 -and $p4.tails.cards -eq 1 -and $p4.duplicate_owned_paragraphs.value -eq 0) 'the appended 关联： tail is counted, and no duplicate owned paragraph is invented'
    Assert ($p4.known -eq $false -and $p4.run.known -eq $false) 'the Link jobs and per-run usage are unknown without a run report'
    $p7 = $first.baseline.measures.P7
    Assert ($p7.cross_book_lines.value -eq 1 -and $p7.cross_book_lines.denominator -ge 2 -and $p7.unsupported.value -ge 1) "a hub line crosses books only on explicit evidence, and an unsupported line is counted apart: $($p7.cross_book_lines | ConvertTo-Json -Compress)"
    Assert ($p7.hubs.mirror -eq 0 -and $p7.hubs.one_book -ge 0) 'the hub advice is counted, and no ratio rewards an unsupported Link'
    $p8 = $first.baseline.measures.P8
    Assert ($p8.known -eq $true -and $p8.terms.accepted -eq 1 -and $p8.terms.unresolved_rows -eq 1) "the register reports its accepted terms and its unresolved rows apart: $($p8.terms | ConvertTo-Json -Compress)"
    Assert ($p8.direct.value -eq 1 -and $p8.reach.direction -ceq 'outbound' -and $p8.reach.hops -eq 2 -and $p8.reach.at_1 -eq 2 -and $p8.reach.at_2 -eq 1) "the reach is two outbound semantic Links from the entry target, with register navigation no hop: $($p8.reach | ConvertTo-Json -Compress)"
    Assert ($p8.hub_unreached.known -eq $true -and $p8.complete -eq $false) 'the hub unreached measure is reported apart from the register figures'
    $p9 = $first.baseline.measures.P9
    Assert ($p9.local_reference.cards -ge 1 -and $p9.unnamed_referent.cards -ge 1 -and $p9.manual.known -eq $false) 'the standalone heuristic rates are counted, and their manual annotation is unknown without the sheet'
    $p11 = $first.baseline.measures.P11
    # The idea population holds the one exact pair and its one near pair; the navigation
    # population is counted apart, with two exact pairs (two root titles repeat) and the
    # anchor-title fault.
    Assert ($p11.idea.exact_pairs -eq 1 -and $p11.idea.near_pairs -eq 1 -and $p11.navigation.exact_pairs -eq 2 -and $p11.navigation.near_pairs -eq 1 -and $p11.anchor_title_faults -eq 1) "the exact and near title pairs are reported for the idea and navigation populations apart, with the anchor-title fault: $($p11.idea | ConvertTo-Json -Compress) / $($p11.navigation | ConvertTo-Json -Compress)"
    Assert ($first.baseline.measures.P14.interval -eq $null) 'the human half is observed now, and its interval needs a baseline that recorded it'
    $md = Get-Content -LiteralPath (Join-Path $firstOut 'audit.md') -Raw -Encoding utf8
    Assert ($md -match '## Baseline measures \(#94 S01\)' -and $md -match 'Missing evidence' -and $md -match '### Source evidence' -and $md -match '### Register and reach \(P8\)' -and $md -match 'No measure below is reported as a zero') 'audit.md renders the baseline evidence, the source sets, the register reach and every unknown measure'
    # --- the new-Card interval, and every named input.
    $null = Write-Cards @(
        @{tool='create_successor';arguments=@{at=$made[7];kind='side';title='新的自有来源观点';body="依赖 [[$($made[1])]]：同一设定。`n`n来源：[[$($made[0])]] §1"}},
        @{tool='create_successor';arguments=@{at=$made[16];kind='side';title='新的未知来源观点';body='这一观点没有写出处。'}}
    )
    # The annotation names a Card of this revision's sample, so the sample is read first: the
    # same revision draws the same addresses, and a later change to the scope draws new ones.
    $probe = Audit (Join-Path $scratch 'probe') @('-Baseline', $firstOut)
    $sample = $probe.baseline.evidence.sample
    $sheetPath = Join-Path $scratch 'sheet.json'
    $sheet = [ordered]@{
        schema_version = 1; kind = 'zt-task-sheet'
        interval = [ordered]@{ from = '2026-10-01'; to = '2026-10-18' }
        tasks = @(
            [ordered]@{ id = 'T1'; kind = 'first-useful-card'; question = '融资约束如何影响投资？'; seconds = 40; useful_address = $made[7]; entry_rank = 2 },
            [ordered]@{ id = 'T2'; kind = 'first-useful-card'; question = '剩余控制权是什么？'; seconds = 60; useful_address = $made[13]; entry_rank = 4 },
            [ordered]@{ id = 'T3'; kind = 'kept-spread'; question = '多重均衡'; seconds = 100; kept_spread = '均衡' },
            [ordered]@{ id = 'T4'; kind = 'distant-card'; question = '跨书对照'; offered = 3; opened = 2; laid = 1 }
        )
        removed_cards = @('12/9')
        human_replies = 2
        annotation = @([ordered]@{ address = $sample.addresses[0]; local_reference_false_positive = $true; unnamed_referent_false_positive = $false })
    }
    [IO.File]::WriteAllText($sheetPath, ($sheet | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    $conceptDir = Join-Path $scratch 'concepts'
    New-Item -ItemType Directory -Force $conceptDir | Out-Null
    $concept = [ordered]@{
        schema_version = 1; report_id = 'r-s01-fixture'; created_at = '2026-10-18T09:00:00Z'
        book = [ordered]@{ citation_key = 'A2020'; title = 'Alpha' }
        archive = [ordered]@{ id = $e.archive.id; path = $e.archive.path; revision = $e.archive.revision }
        coverage = [ordered]@{ restatement_candidates = 2 }
        completeness = [ordered]@{ complete = $false; judged = 1; unjudged = 1; unread = @(); reasons = @('unjudged_candidates') }
        findings = @(
            [ordered]@{ id = 'c001'; kind = 'restatement'; verdict = 'same'; unjudged = $false; candidate = $made[8] },
            [ordered]@{ id = 'c002'; kind = 'restatement'; verdict = 'uncertain'; unjudged = $true; candidate = $made[11] },
            [ordered]@{ id = 'c003'; kind = 'topic-concept'; verdict = 'different'; unjudged = $false; candidate = $made[7] }
        )
        status = 'partial'
    }
    [IO.File]::WriteAllText((Join-Path $conceptDir 'concept-review.json'), ($concept | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    $decision = [ordered]@{
        schema_version = 1; report_id = 'r-s01-fixture'; decision_id = 'd-s01-fixture'
        decisions = @([ordered]@{ finding = 'c001'; kind = 'restatement'; action = 'keep_one'; verdict = 'same'; note = '读过了。' })
    }
    [IO.File]::WriteAllText((Join-Path $conceptDir 'decision.json'), ($decision | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    $runPath = Join-Path $scratch 'run-report.json'
    $runReport = [ordered]@{
        schema_version = 1
        identity = [ordered]@{ run_id = 'run-s01'; citation_key = 'A2020'; variant = 'econ'; binaries = [ordered]@{ zt_digest = '0.1.0'; zt_check = '0.1.0'; zt = '0.20.0' } }
        counts = [ordered]@{ per_layer = [ordered]@{ link = [ordered]@{ declared = 3; done = 2; noop = 1; refused = 0; failed = 0 } } }
        usage = [ordered]@{ status = 'complete'; known_runs = 2; missing_runs = 0; totals = [ordered]@{ requests = 120; prompt_tokens = 9000; completion_tokens = 3000 } }
    }
    [IO.File]::WriteAllText($runPath, ($runReport | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    $secondOut = Join-Path $scratch 'second'
    $second = Audit $secondOut @('-Baseline', $firstOut, '-TaskSheet', $sheetPath, '-ConceptReport', $conceptDir, '-RunReport', $runPath)
    $b = $second.baseline
    Assert ($b.complete -eq $true -and @($b.evidence.missing).Count -eq 0) "every named input was read, so the baseline evidence is complete: $($b.evidence.missing | ConvertTo-Json -Compress)"
    Assert ($b.comparison.matched -eq $true -and $b.comparison.baseline.scope -ceq 'agent' -and $b.comparison.new_cards.count -eq 2) "the baseline counts the two new Cards inside the same scope: $($b.comparison.new_cards | ConvertTo-Json -Compress)"
    Assert ($b.comparison.measures.cards.delta -eq 2 -and $b.comparison.measures.cards.status -ceq 'matched') 'a matched baseline gives a delta'
    $p1b = $b.measures.P1
    Assert ($p1b.known -eq $true -and $p1b.values.value -eq 1 -and $p1b.values.denominator -eq 2 -and $p1b.values.unknown_source -eq 1 -and $p1b.rate.per_100 -eq 50) "P1 counts the new Topic Idea Cards with an own-source rests-on over all checked new Topic Idea Cards: $($p1b.values | ConvertTo-Json -Compress -Depth 5)"
    Assert ($p1b.target -eq 30) 'P1 carries the pilot target it is measured against'
    $p2 = $b.measures.P2
    Assert ($p2.known -eq $true -and $p2.values.candidates -eq 2 -and $p2.values.judged -eq 1 -and $p2.values.judged_same -eq 1 -and $p2.values.resolved -eq 1 -and $p2.values.pending -eq 1) "P2 keeps candidate, judged-same, resolved and pending apart: $($p2.values | ConvertTo-Json -Compress)"
    $p3 = $b.measures.P3_P5
    Assert ($p3.filing.known -eq $true -and $p3.filing.value -eq 1 -and $p3.filing.denominator -eq 2 -and $p3.topics.value -eq 1 -and $p3.topics.denominator -ge 2) "P3/P5 count the new Card filed in the mixed-source Topic and the Topics that name several books: $($p3 | ConvertTo-Json -Compress -Depth 8)"
    $p4b = $b.measures.P4
    Assert ($p4b.known -eq $true -and $p4b.run.link.declared -eq 3 -and $p4b.run.link.done -eq 2 -and $p4b.run.usage.totals.requests -eq 120 -and $p4b.run.usage.status -ceq 'complete') "P4 carries the Link jobs and the exact per-run usage from the run report: $($p4b.run | ConvertTo-Json -Compress -Depth 8)"
    $p12 = $b.measures.P12_P13
    Assert ($p12.known -eq $true -and $p12.first_useful_card.median_seconds -eq 50 -and $p12.kept_spread.median_seconds -eq 100 -and $p12.entry_rank.median -eq 3 -and $p12.removed_cards.verified_absent -eq 1 -and $p12.human_replies -eq 2) "the fixed task sheet gives the times, the entry rank, the removed Card and the human replies: $($p12 | ConvertTo-Json -Compress -Depth 6)"
    $p15 = $b.measures.P15
    Assert ($p15.offered -eq 3 -and $p15.opened -eq 2 -and $p15.laid -eq 1) 'the task sheet records the distant Cards offered, opened and laid'
    $p9b = $b.measures.P9
    Assert ($p9b.manual.known -eq $true -and $p9b.manual.annotated -eq 1 -and $p9b.manual.local_reference_false_positive -eq 1 -and $p9b.manual.outside_sample -eq 0) 'the manual annotation is kept apart from the heuristic count'
    Assert ($b.task_sheet.valid -eq $true -and @($b.task_sheet.faults).Count -eq 0) 'the fixed task sheet validates against the archive'
    Assert ($b.measures.P14.interval.baseline_revision -eq $first.archive_revision -and $null -ne $b.measures.P14.interval.human_cards_net_change) 'the human half names its comparison interval when the baseline recorded the counts'
    $md2 = Get-Content -LiteralPath (Join-Path $secondOut 'audit.md') -Raw -Encoding utf8
    Assert ($md2 -match 'Scopes match' -and $md2 -match '### Fixed manual task sheet' -and $md2 -match 'The accepted proposal.s baseline, in its original scope') 'audit.md renders the matched comparison, the task sheet and the proposal baseline with its scope'
    # --- S01-AC06: a full-Workspace baseline is never compared with a selected-book report.
    $bookOut = Join-Path $scratch 'book'
    $book = Audit $bookOut @('-CitationKey', 'A2020', '-Baseline', $firstOut)
    $c = $book.baseline.comparison
    Assert ($c.matched -eq $false -and $c.measures.cards.delta -eq $null -and $c.measures.cards.status -ceq 'scope_mismatch') "a scope change produces no delta: $($c.measures.cards | ConvertTo-Json -Compress)"
    Assert ($c.scope_change -match 'agent' -and $c.scope_change -match 'A2020' -and $book.baseline.comparison.proposal.scope_change -match 'original scope') 'the report names both scopes and the proposal baseline keeps its original scope'
    Assert ($book.baseline.evidence.scope.kind -ceq 'book' -and $book.baseline.measures.P1.known -eq $true) 'a book scope measures its own perimeter against the same baseline revision'
    $md3 = Get-Content -LiteralPath (Join-Path $bookOut 'audit.md') -Raw -Encoding utf8
    Assert ($md3 -match 'Scope change, no delta') 'audit.md states the scope change where a delta would otherwise be read'
    # --- an observation that does not match the archive is a fault, never a measured number.
    $badSheet = Join-Path $scratch 'bad-sheet.json'
    $bad = [ordered]@{
        schema_version = 1; kind = 'zt-task-sheet'; interval = [ordered]@{ from = '2026-10-01'; to = '2026-10-18' }
        tasks = @([ordered]@{ id = 'T1'; kind = 'first-useful-card'; question = '哪里？'; seconds = 40; useful_address = '12/9|zz'; entry_rank = 2 })
        removed_cards = @($made[7]); human_replies = 0; annotation = @()
    }
    [IO.File]::WriteAllText($badSheet, ($bad | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    $badOut = Join-Path $scratch 'bad'
    $badRun = Audit $badOut @('-TaskSheet', $badSheet)
    Assert ($badRun.baseline.task_sheet.valid -eq $false -and @($badRun.baseline.task_sheet.faults).Count -eq 2) "an observation the archive contradicts is a reported fault: $($badRun.baseline.task_sheet.faults | ConvertTo-Json -Compress)"
    Assert ($badRun.baseline.measures.P12_P13.known -eq $false -and $badRun.baseline.complete -eq $false) 'a faulty task sheet leaves its measures unknown and the baseline incomplete'
    # A sheet that does not answer the fixed schema is refused before any report is published.
    $wrongSheet = Join-Path $scratch 'wrong-sheet.json'
    $wrong = [ordered]@{
        schema_version = 1; kind = 'zt-task-sheet'; interval = [ordered]@{ from = '2026-10-01'; to = '2026-10-18' }
        tasks = @([ordered]@{ id = 'T1'; kind = 'first-useful-card'; question = '哪里？'; seconds = 40; useful_address = $made[7]; entry_rank = 2; started_at = '10:00' })
        removed_cards = @(); human_replies = 0; annotation = @()
    }
    [IO.File]::WriteAllText($wrongSheet, ($wrong | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    $refusedOut = Join-Path $scratch 'refused'
    $refused = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -Out $refusedOut -TaskSheet $wrongSheet 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($refused -join ' ') -match 'not a valid fixed task sheet' -and -not (Test-Path -LiteralPath (Join-Path $refusedOut 'audit.json'))) 'a sheet outside the fixed schema refuses the report instead of reading a column it does not own'
    # A baseline from another archive cannot define what is new.
    $otherArchive = Join-Path $scratch 'other-archive'
    New-Item -ItemType Directory -Force $otherArchive | Out-Null
    $edited = Read-Json (Join-Path $firstOut 'audit.json')
    $edited.baseline.evidence.archive.id = '0123456789abcdef0123456789abcdef'
    [IO.File]::WriteAllText((Join-Path $otherArchive 'audit.json'), ($edited | ConvertTo-Json -Depth 40), [Text.UTF8Encoding]::new($false))
    Copy-Item -LiteralPath (Join-Path $firstOut 'audit-addresses.txt') -Destination (Join-Path $otherArchive 'audit-addresses.txt')
    $otherOut = Join-Path $scratch 'other'
    $other = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -Out $otherOut -Baseline $otherArchive 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($other -join ' ') -match 'measures another archive' -and -not (Test-Path -LiteralPath (Join-Path $otherOut 'audit.json'))) 'a baseline from another archive is refused, never compared'
    # --- read-only: the archive is exactly as it was.
    $before = (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    $null = Audit (Join-Path $scratch 'readonly')
    $after = (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    Assert ($before -eq $after) 'the audit changes no Card'
    $status = @(zt status)
    Assert (@($status | Where-Object { $_ -match '^state: up$' }).Count -eq 1) 'the audit left the service up'
    zt status --check-links | Out-Null
    Assert ($LASTEXITCODE -eq 0) 'the link index still matches every stored Reverse section'
} finally { zt down | Out-Null }
