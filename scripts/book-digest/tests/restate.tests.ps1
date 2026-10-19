#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\restate.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# S08 (#107): resolving a new restatement without rewriting a finished model, through the public
# Card-cycle seam (`restate-compiler.ps1` -> `zt-digest restate`). A scratch archive holds a book
# with anchors and a model part, an earlier Topic, and the run's new Topic Cards. The suite proves
# the bounded comparison pairs, the judge rules, the receipt-owned rewrite of the new Card only,
# the readback and check, the explicit statuses, the pending merge verdict, the stale authored base
# and a lost reply replayed by the same operation. The live archive is not touched.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "restate-test-$PID"
if ($Zt) {
    $bin = Join-Path $scratch 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
if ($Digest) { $env:ZT_DIGEST = $Digest }
$utf8 = [Text.UTF8Encoding]::new($false)
$wrapper = (Join-Path $PSScriptRoot '..\restate-compiler.ps1') | Resolve-Path | ForEach-Object Path
$seam = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Invoke-Restate([string[]]$Arguments) {
    $text = @(& pwsh -NoProfile -File $wrapper @Arguments 2>&1 | ForEach-Object { [string]$_ }) -join "`n"
    $code = $LASTEXITCODE
    $last = $null
    if (-not $code) { $last = ($text -split "`n")[-1] | ConvertFrom-Json -AsHashtable -DateKind String }
    [pscustomobject]@{ code = $code; text = $text; last = $last }
}
function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String }
function Write-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 20), $utf8) }
function New-Staging([string]$name, [string[]]$cards) {
    $staging = Join-Path $scratch $name
    New-Item -ItemType Directory -Force (Join-Path $staging 'restate') | Out-Null
    Write-Json (Join-Path $staging 'manifest.json') @{ citation_key = $script:key; book_title = '重述复核'; groups = @(@{ n = 1; chapter = '第一章'; headings = @('斯卢茨基') }) }
    $file = Join-Path $staging 'new-cards.txt'
    [IO.File]::WriteAllText($file, (($cards -join "`n") + "`n"), $utf8)
    [pscustomobject]@{ staging = $staging; cards = $file }
}
function Card($at) { (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] }
function Frapp($at) { $card = Card $at; "$($card.title)`u{1}$($card.body)" }

$script:key = 'Restate2026'
$partStatement = "斯卢茨基分解把价格变化对需求的总效应分成替代效应与收入效应：`$dx=\partial x_s+\partial x_i`$。`n`n假设：价格变化很小。`n`n来源：[[Restate2026]] §2.1"
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    # The book: a Literature root, one anchor, and a formal model with a setting part. The part is
    # the finished model statement that must keep its home.
    $book = (Write-Cards @(
            @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@book{$($script:key), title={Restatement Review}, author={Owner, A.}, year={2026}}"; body = '全书索引。' } },
            @{ tool = 'create_successor'; arguments = @{ at = $script:key; kind = 'direct'; title = '第 2 章：斯卢茨基分解'; body = '§2.1 - §2.3' } }
        ))
    $anchor = $book[1]
    $model = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = $anchor; kind = 'side'; type = 'model'; level = 'formal'; title = '斯卢茨基分解'; body = '问题：价格变化怎样分成两种效应？' } }))[0]
    $part = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = $model; kind = 'side'; facet = '设定'; title = '设定：两种效应相加'; body = $partStatement } }))[0]
    # An earlier Topic whose claim states the formula under different conditions: a `different` pair.
    $earlier = (Write-Cards @(@{ tool = 'create_topic'; arguments = @{ title = '斯卢茨基分解'; body = '本主题追问：总效应怎样分成两种效应？' } }))[0] -replace '/0$', ''
    $null = Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = "$earlier/0"; title = '斯卢茨基分解按价格变化的大小分两种情形'; body = "价格变化很大时，替代效应与收入效应不能相加：`$dx=\partial x_s+\partial x_i`$ 只是近似。`n`n假设：价格变化很大。`n`n来源：[[Restate2026]] §2.1" } })
    # The run's new Topic: four new claims, a Hub Card the review skips, and one more claim for the
    # lost-reply fixture.
    $new = (Write-Cards @(@{ tool = 'create_topic'; arguments = @{ title = '新的斯卢茨基解读'; body = '本主题追问：斯卢茨基分解还有哪些后果？' } }))[0] -replace '/0$', ''
    $first = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = "$new/0"; title = '斯卢茨基分解的陈述入口'; body = $partStatement } }))[0]
    $newCards = (Write-Cards @(
            @{ tool = 'create_successor'; arguments = @{ at = $first; kind = 'side'; title = '斯卢茨基分解在价格不变时的后果'; body = $partStatement } },
            @{ tool = 'create_successor'; arguments = @{ at = $first; kind = 'side'; title = '价格变化很大时两种效应不可加'; body = "价格变化很大时两种效应不能相加：`$dx=\partial x_s+\partial x_i`$ 只是近似。`n`n假设：价格变化很大。`n`n来源：[[Restate2026]] §2.1" } },
            @{ tool = 'create_successor'; arguments = @{ at = $first; kind = 'side'; title = '斯卢茨基分解的符号相反'; body = "价格变化时替代效应减去收入效应：`$dx=\partial x_s-\partial x_i`$。`n`n假设：价格变化很小。`n`n来源：[[Restate2026]] §2.1" } },
            @{ tool = 'create_successor'; arguments = @{ at = $first; kind = 'side'; title = '斯卢茨基分解的出处入口'; body = $partStatement } }
        ))
    $same = $first
    $distinct = $newCards[0]
    $conditioned = $newCards[1]
    $sign = $newCards[2]
    $pending = $newCards[3]
    $interpretation = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = "$first"; kind = 'side'; title = '斯卢茨基分解的一种解读'; body = "一种解读：`$dx=\partial x_s+\partial x_i`$ 说明总效应可以看作两项之和，不能说明两项各自如何决定。`n`n假设：价格变化很小。`n`n来源：[[Restate2026]] §2.1" } }))[0]
    $hub = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = "$first"; kind = 'side'; type = 'hub'; level = 'line'; title = '新的斯卢茨基解读的入口'; body = "## 后果`n- [[$first]] 陈述入口。`n" } }))[0]
    # The register makes the entry's terminal readable through the archive's own projection.
    $index = (Write-Cards @(@{ tool = 'create_topic'; arguments = @{ title = '索引'; body = '本主题收放代理工作区的入口。' } }))[0]
    $null = Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = $index; type = 'hub'; level = 'register'; title = '索引：代理工作区的入口'; body = "## 词条`n- 斯卢茨基分解：[[$part]]`n" } })

    $work = New-Staging 'staging' @($same, $distinct, $conditioned, $sign, $pending, $interpretation, $hub)
    $staging = $work.staging

    # --- prepare: bounded comparison pairs from both populations --------------------------------
    $missingCards = Invoke-Restate @('-Step', 'prepare', '-Staging', $staging)
    Assert ($missingCards.code -ne 0 -and $missingCards.text -match '-Cards') 'prepare refuses a review that names no new Card'
    $onlyHub = New-Staging 'staging-hub' @($hub)
    $noClaim = Invoke-Restate @('-Step', 'prepare', '-Staging', $onlyHub.staging)
    Assert ($noClaim.code -ne 0 -and $noClaim.text -match 'Topic Idea Card') 'a review of no Topic Idea Card is refused, not reported as a clean comparison'
    $prepare = Invoke-Restate @('-Step', 'prepare', '-Staging', $staging, '-Cards', $work.cards)
    Assert ($prepare.code -eq 0) "prepare runs: $($prepare.text)"
    $p = $prepare.last
    Assert ($p.ok -and $p.archive_id -match '^[0-9a-f]{32}$' -and $p.archive_revision -is [long]) 'prepare binds the archive identity and revision'
    Assert ($p.reviewable -eq 6 -and @($p.skipped).Count -eq 1 -and @($p.skipped)[0].address -eq $hub) 'prepare reviews the six claims and skips the Hub Card with its reason'
    $evidence = Read-Json (Join-Path $staging 'restate\evidence.json')
    Assert ($evidence.evidence_id -match '^e-[0-9a-f]{64}$' -and $evidence.book -eq $script:key -and $evidence.archive.revision -eq $p.archive_revision) 'the frozen evidence carries its id, book and revision'
    Assert ($evidence.bounds.candidates -eq 10 -and $evidence.context.anchors -ge 1 -and $evidence.context.parts -ge 1 -and $evidence.context.earlier_topic_ideas -ge 1) 'the comparison populations are the book anchors, its model parts and the earlier Topic Cards'
    $sameRow = @($evidence.new_cards | Where-Object { $_.address -eq $same })[0]
    Assert (@($sameRow.candidates).Count -ge 1 -and @($sameRow.candidates).Count -le 10) 'a new Card gets a bounded candidate list'
    $modelCandidate = @($sameRow.candidates | Where-Object { $_.address -eq $part })[0]
    Assert ($null -ne $modelCandidate -and $modelCandidate.kind -eq 'part' -and $modelCandidate.statement_fingerprint -eq $sameRow.statement_fingerprint) 'the existing model part is a candidate, and an identical statement carries the same fingerprint'
    Assert ($modelCandidate.formula_match -and $modelCandidate.relatedness -match '^\d+\.\d{4}$' -and $modelCandidate.title_similarity -match '^\d+\.\d{4}$') 'a candidate carries the shared-formula, relatedness and title-similarity evidence, none of them a verdict'
    $earlierRow = @($evidence.new_cards | Where-Object { $_.address -eq $conditioned })[0]
    Assert (@($earlierRow.candidates | Where-Object { $_.address -eq "$earlier/1" }).Count -eq 1) 'an earlier Topic Idea Card is a candidate of the new claim that repeats it'
    Assert (@($evidence.endpoints.Keys | Where-Object { $_ -eq $part }).Count -eq 1) 'the selected candidate body is retained as evidence'
    $job = Get-Content -LiteralPath (Join-Path $staging 'restate\rj-01.md') -Raw -Encoding utf8
    Assert ($job -match '两种效应' -and $job -match $part -and $job -match 'SELF-CHECK' -and $job -match '同述入口') 'the judge job shows both complete statements, the terminal and the answer rules'
    Assert (Test-Path -LiteralPath (Join-Path $staging 'restate\dsh-restate-instructions.zh.md')) 'the judge instruction file is copied beside the job'

    # --- judge: one verdict per pair, and every accepted rewrite shaped correctly --------------
    function New-Answers($evidence) {
        $rows = @()
        foreach ($card in $evidence.new_cards) {
            foreach ($c in $card.candidates) {
                $row = [ordered]@{ card = $card.address; candidate = $c.address; verdict = 'different'; reason = '两张卡讨论的对象与陈述都不同。'; evidence = @($card.address, $c.address) }
                if ($c.address -eq $part -and $card.address -eq $same) {
                    $row.verdict = 'same'; $row.canonical = $part; $row.scope = '价格变化很小的局部比较静态'; $row.reason = '两张卡是同一陈述，假设与范围相同。'
                    $row.remedy = 'entry'; $row.title = '斯卢茨基分解的陈述入口'
                    $row.body = "## 同述入口`n`n- [[$part]] 给出补偿需求项与收入效应项的完整定义。`n`n来源：[[Restate2026]] §2.1"
                    $row.evidence = @($card.address, $part)
                } elseif ($c.address -eq $part -and $card.address -eq $distinct) {
                    $row.verdict = 'same'; $row.canonical = $part; $row.scope = '价格变化很小的局部比较静态'; $row.reason = '两张卡是同一陈述，假设与范围相同。'
                    $row.remedy = 'distinct-implication'; $row.title = '斯卢茨基分解在价格不变时的后果'
                    $row.body = "本卡依赖 [[$part]] 的定义：价格不变时两项效应都为零。这条推论只用于局部比较。它不适用于价格变化很大的情形。`n`n来源：[[Restate2026]] §2.1"
                    $row.evidence = @($card.address, $part)
                } elseif ($c.address -eq $part -and $card.address -eq $pending) {
                    $row.verdict = 'same'; $row.canonical = $part; $row.scope = '价格变化很小的局部比较静态'; $row.reason = '两张卡是同一陈述，假设与范围相同。'
                    $row.remedy = 'existing-card-change'
                    $row.evidence = @($card.address, $part)
                } elseif ($c.address -eq $part -and ($card.address -eq $conditioned -or $card.address -eq $sign -or $card.address -eq $interpretation)) {
                    $row.reason = if ($card.address -eq $sign) { '公式里的符号相反：一个相加，一个相减。' } elseif ($card.address -eq $interpretation) { '候选卡陈述定理本身，本卡只是对它的解读，二者不是同一陈述。' } else { '公式相同，但假设不同：一张卡假设价格变化很大。' }
                }
                $rows += $row
            }
        }
        , $rows
    }
    $out = Join-Path $staging 'restate\out'
    New-Item -ItemType Directory -Force $out | Out-Null
    $answerFile = Join-Path $out 'rj-01.json'
    $judgments = New-Answers $evidence
    Write-Json $answerFile @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = $judgments }
    $judge = Invoke-Restate @('-Step', 'judge', '-Staging', $staging, '-File', $answerFile)
    Assert ($judge.code -eq 0 -and $judge.last.judged -eq $judgments.Count -and $judge.last.unjudged -eq 0) "judge accepts every prepared pair ($($judgments.Count) prepared): $($judge.text)"
    $judged = Read-Json (Join-Path $staging 'restate\judged.json')
    Assert ($judged.judged_id -match '^j-[0-9a-f]{64}$' -and @($judged.judgments | Where-Object { $_.status -eq 'recommended' }).Count -eq 2) 'the accepted rewrites are frozen as recommended until they are written'

    $bad = Join-Path $out 'bad.json'
    $refuse = {
        param($value)
        Write-Json $bad $value
        Invoke-Restate @('-Step', 'judge', '-Staging', $staging, '-File', $bad)
    }
    $unknown = & $refuse @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = @(@{ card = $same; candidate = $part; verdict = 'equivalent'; reason = 'x'; evidence = @($same) }) }
    Assert ($unknown.code -ne 0 -and $unknown.text -match 'unknown verdict') 'judge refuses an unknown verdict'
    $unprepared = & $refuse @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = @(@{ card = $same; candidate = 'Restate2026/99'; verdict = 'different'; reason = 'x'; evidence = @($same) }) }
    Assert ($unprepared.code -ne 0 -and $unprepared.text -match 'did not prepare') 'judge refuses a pair the evidence never prepared'
    $unread = & $refuse @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = @(@{ card = $same; candidate = $part; verdict = 'different'; reason = 'x'; evidence = @('9/9') }) }
    Assert ($unread.code -ne 0 -and $unread.text -match 'did not read') 'judge refuses a verdict that cites an address the pair did not read'
    $noScope = & $refuse @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = @(@{ card = $same; candidate = $part; verdict = 'same'; canonical = $part; reason = '两张卡是同一陈述。'; remedy = 'existing-card-change'; evidence = @($same, $part) }) }
    Assert ($noScope.code -ne 0 -and $noScope.text -match 'scope') "a same verdict states the scope and assumptions that coincide :: $($noScope.text)"
    $chain = & $refuse @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = @(@{ card = $same; candidate = $part; verdict = 'same'; canonical = $part; scope = '价格变化很小的局部比较静态'; reason = '两张卡是同一陈述。'; remedy = 'entry'; title = '入口卡'; body = "## 同述入口`n`n- [[$anchor]] 给出完整定义。`n`n来源：[[Restate2026]] §2.1"; evidence = @($same, $part) }) }
    Assert ($chain.code -ne 0 -and $chain.text -match 'terminal') "an entry that names a non-terminal Card is refused :: $($chain.text)"
    $dropped = & $refuse @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = @(@{ card = $same; candidate = $part; verdict = 'same'; canonical = $part; scope = '价格变化很小的局部比较静态'; reason = '两张卡是同一陈述。'; remedy = 'entry'; title = '入口卡'; body = "## 同述入口`n`n- [[$part]] 给出补偿需求项与收入效应项的完整定义。`n"; evidence = @($same, $part) }) }
    Assert ($dropped.code -ne 0 -and $dropped.text -match 'source') "an accepted rewrite that drops the source evidence is refused :: $($dropped.text)"
    $twice = & $refuse @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = @(@{ card = $same; candidate = $part; verdict = 'different'; reason = '两张卡讨论的条件不同。'; evidence = @($same) }, @{ card = $same; candidate = $part; verdict = 'same'; canonical = $part; scope = '价格变化很小的局部比较静态'; remedy = 'existing-card-change'; reason = '两张卡是同一陈述。'; evidence = @($same) }) }
    Assert ($twice.code -ne 0 -and $twice.text -match 'twice') "two outcomes for one pair are refused :: $($twice.text)"

    # The frozen evidence is immutable: an edited file no longer matches its own id, so no step
    # reviews it as the set the answers were given against.
    $evidencePath = Join-Path $staging 'restate\evidence.json'
    $savedEvidence = [IO.File]::ReadAllBytes($evidencePath)
    [IO.File]::WriteAllText($evidencePath, [Text.Encoding]::UTF8.GetString($savedEvidence).Replace('斯卢茨基分解的陈述入口', '斯卢茨基分解的陈述入口X'), $utf8)
    $tampered = Invoke-Restate @('-Step', 'report', '-Staging', $staging)
    Assert ($tampered.code -ne 0 -and $tampered.text -match 'does not match its evidence id') 'an edited evidence file is refused by its own id'
    [IO.File]::WriteAllBytes($evidencePath, $savedEvidence)
    # --- the stale authored base conflicts safely ----------------------------------------------
    $noPrefix = Invoke-Restate @('-Step', 'apply', '-Staging', $staging)
    Assert ($noPrefix.code -ne 0 -and $noPrefix.text -match 'OperationPrefix') 'apply requires an operation prefix'
    $beforeFixtures = @{}
    foreach ($at in @($part, $model, "$earlier/1")) { $beforeFixtures[$at] = Frapp $at }
    $null = Write-Cards @(@{ tool = 'edit_card'; arguments = @{ at = $distinct; body = "被别的写者改过的正文。`n`n来源：[[Restate2026]] §2.1" } })
    $stale = Invoke-Restate @('-Step', 'apply', '-Staging', $staging, '-OperationPrefix', 'restate-run/restate/1/apply')
    Assert ($stale.code -ne 0 -and $stale.text -match 'authored base changed') "an independent edit after prepare conflicts with the frozen authored base :: $($stale.text)"
    Assert ((Card $same).body -notmatch '同述入口' -and (Card $distinct).body -match '被别的写者改过') 'the conflicting apply wrote no Card'

    # --- apply: the new Card only, receipted, read back and checked ----------------------------
    Write-Json (Join-Path $staging 'restate\merges.json') @{ merges = @(
            @{ address = $same; merge_with = $part; note = '卡片循环建议合并' },
            @{ address = "$earlier/1"; merge_with = $part; note = '已有的两张卡' }
        ) }
    # Restore the exact prepared text of the distinct Card, so its frozen authored base matches again.
    $null = Write-Cards @(@{ tool = 'edit_card'; arguments = @{ at = $distinct; title = '斯卢茨基分解在价格不变时的后果'; body = $partStatement } })
    $apply = Invoke-Restate @('-Step', 'apply', '-Staging', $staging, '-OperationPrefix', 'restate-run/restate/1/apply', '-Merges', (Join-Path $staging 'restate\merges.json'))
    Assert ($apply.code -eq 0) "apply runs: $($apply.text)"
    $a = $apply.last
    Assert ($a.written -eq 2 -and $a.counts.resolved_by_rewrite -eq 2 -and $a.counts.pending_existing_card_change -eq 2 -and $a.counts.different -ge 2 -and $a.complete) 'apply reports two resolved rewrites, two pending existing-Card changes and the different verdicts: ' + "$($a.counts | ConvertTo-Json -Compress)"
    $resolution = Read-Json (Join-Path $staging 'restate\resolution.json')
    Assert (@($resolution.findings | Where-Object { $_.card -eq $same -and $_.status -eq 'resolved-by-rewrite' -and $_.checked -and $_.after_fingerprint -match '^[0-9a-f]{64}$' }).Count -eq 1) 'the entry rewrite is resolved only after its receipt and readback fingerprint'
    Assert (@($resolution.findings | Where-Object { $_.card -eq $pending -and $_.status -eq 'pending-existing-card-change' }).Count -eq 1) 'a resolution that would change the existing Card stays pending'
    Assert (@($resolution.merges | Where-Object { $_.address -eq $same -and $_.status -eq 'resolved-by-rewrite' }).Count -eq 1) 'a merge verdict covered by the rewrite is resolved by the rewrite, not counted as an applied merge'
    Assert (@($resolution.merges | Where-Object { $_.address -eq "$earlier/1" -and $_.status -eq 'pending-existing-card-change' }).Count -eq 1) 'a merge verdict naming two existing Cards stays pending'
    $entry = Card $same
    Assert ($entry.restatement -eq $true -and $entry.body -match '## 同述入口' -and $entry.body -match "\[\[$part\]\]") 'the rewritten entry is navigation that names its terminal substantive Card'
    $register = (Invoke-Zt @(@{ tool = 'get_register'; arguments = @{ workspace = 'agent' } }))[0]
    Assert (@($register.restatements | Where-Object { $_.entry -eq $same -and $_.substantive -eq $part -and $_.terminal -eq $part }).Count -eq 1) "the archive's own view confirms the entry and its terminal: $($register.restatements | ConvertTo-Json -Compress)"
    $rewritten = Card $distinct
    Assert ($rewritten.body -match "\[\[$part\]\]" -and $rewritten.body -notmatch '## 同述入口' -and $rewritten.body -match '推论') 'the distinct implication keeps a reasoned Link to the existing statement and adds no entry section'
    Assert ((Card $conditioned).title -match '价格变化很大' -and (Card $sign).body -match '\\partial x_s-') 'a different verdict changes no Card'
    Assert ((Card $pending).body -match '这两种效应' -or (Card $pending).body -match '总效应') 'a pending resolution changes no Card either'
    # The finished model keeps its statement, its parts and its membership.
    foreach ($at in @($part, $model, "$earlier/1")) { Assert ((Frapp $at) -eq $beforeFixtures[$at]) "the existing Card $at keeps its authored text" }
    $modelNow = (Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $model } }))[0]
    Assert ($modelNow.head.address -eq $model -and @($modelNow.parts | Where-Object { $_.address -eq $part }).Count -eq 1) 'the model still has its part and its build'
    # Retrying the same operation replays its receipts instead of writing again.
    $revisionBefore = (Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @($same); view = 'text' } }))[0].archive_revision
    $again = Invoke-Restate @('-Step', 'apply', '-Staging', $staging, '-OperationPrefix', 'restate-run/restate/1/apply', '-Merges', (Join-Path $staging 'restate\merges.json'))
    $revisionAfter = (Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @($same); view = 'text' } }))[0].archive_revision
    Assert ($again.code -eq 0 -and $revisionBefore -eq $revisionAfter -and (Card $same).body -eq $entry.body) 'a repeated operation replays its receipts and writes no second Card'
    $missingMerge = Read-Json (Join-Path $staging 'restate-resolution.json')
    Assert ($missingMerge.counts.resolved_by_rewrite -eq 2) 'the staging copy of the resolution is what the Card-cycle report reads'

    # --- a lost reply is recovered by the same operation --------------------------------------
    $lost = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = "$first"; kind = 'side'; title = '斯卢茨基分解的另一个入口'; body = $partStatement } }))[0]
    $lostWork = New-Staging 'staging-lost' @($lost)
    $lostPrepare = Invoke-Restate @('-Step', 'prepare', '-Staging', $lostWork.staging, '-Cards', $lostWork.cards)
    Assert ($lostPrepare.code -eq 0) "the lost-reply fixture prepares: $($lostPrepare.text)"
    $lostEvidence = Read-Json (Join-Path $lostWork.staging 'restate\evidence.json')
    $lostRow = @($lostEvidence.new_cards)[0]
    $lostPart = @($lostRow.candidates | Where-Object { $_.address -eq $part })[0]
    $lostAnswer = Join-Path $lostWork.staging 'restate\out\rj-01.json'
    New-Item -ItemType Directory -Force (Split-Path -Parent $lostAnswer) | Out-Null
    Write-Json $lostAnswer @{ schema_version = 1; evidence_id = $lostEvidence.evidence_id; judgments = @(
            @{ card = $lost; candidate = $part; verdict = 'same'; canonical = $part; scope = '价格变化很小的局部比较静态'; remedy = 'entry'
                title = '斯卢茨基分解的另一个入口'; body = "## 同述入口`n`n- [[$part]] 给出补偿需求项与收入效应项的完整定义。`n`n来源：[[Restate2026]] §2.1"
                reason = '两张卡是同一陈述。'; evidence = @($lost, $part) }) }
    $lostJudge = Invoke-Restate @('-Step', 'judge', '-Staging', $lostWork.staging, '-File', $lostAnswer)
    Assert ($lostJudge.code -eq 0) 'the lost-reply answers are accepted'
    $env:ZT_CHECK_ZT = $seam
    $env:ZT_DROP_BATCH_REPLY_FLAG = Join-Path $scratch 'committed-without-reply.flag'
    try {
        $cut = Invoke-Restate @('-Step', 'apply', '-Staging', $lostWork.staging, '-OperationPrefix', 'restate-run/lost/1/apply')
        Assert ($cut.code -ne 0 -and (Test-Path -LiteralPath $env:ZT_DROP_BATCH_REPLY_FLAG)) 'the rewrite can commit while its reply is lost'
        Assert ((Card $lost).restatement -eq $true) 'the committed rewrite is in the archive although the answer was lost'
    } finally { Remove-Item Env:ZT_CHECK_ZT, Env:ZT_DROP_BATCH_REPLY_FLAG -ErrorAction SilentlyContinue }
    $recovered = Invoke-Restate @('-Step', 'apply', '-Staging', $lostWork.staging, '-OperationPrefix', 'restate-run/lost/1/apply')
    $lostResolution = Read-Json (Join-Path $lostWork.staging 'restate\resolution.json')
    Assert ($recovered.code -eq 0 -and $lostResolution.counts.resolved_by_rewrite -eq 1 -and (Card $lost).body -match '## 同述入口') 'the same operation recovers the lost reply without a second rewrite'

    # --- report: the explicit statuses, with one pending and one verified example --------------
    $report = Invoke-Restate @('-Step', 'report', '-Staging', $staging)
    Assert ($report.code -eq 0 -and $report.last.report_id -match '^rr-[0-9a-f]{64}$') 'report renders the resolution with its content id'
    $review = Read-Json (Join-Path $staging 'restate\restate-report.json')
    Assert ($review.counts.resolved_by_rewrite -eq 2 -and $review.counts.pending_existing_card_change -eq 2 -and $review.counts.recommended -eq 0 -and $review.complete) 'the report counts verified, pending and recommended findings apart'
    Assert ($review.status -eq 'resolved-with-pending' -and @($review.findings | Where-Object { $_.status -eq 'different' }).Count -ge 2) 'a different verdict is a finding, not a resolution'
    $md = Get-Content -LiteralPath (Join-Path $staging 'restate\restate-report.md') -Raw -Encoding utf8
    Assert ($md -match '一个待处理的解决' -and $md -match '一个已核实的解决' -and $md -match '待改已有卡|pending') 'the rendered report shows a pending and a verified resolution with their reasons'
    $againReport = Invoke-Restate @('-Step', 'report', '-Staging', $staging)
    Assert ($againReport.code -eq 0 -and $againReport.last.report_id -eq $report.last.report_id) 'the same frozen files give the same report id'
    $unjudgedWork = New-Staging 'staging-unjudged' @($conditioned)
    $null = Invoke-Restate @('-Step', 'prepare', '-Staging', $unjudgedWork.staging, '-Cards', $unjudgedWork.cards)
    $partial = Invoke-Restate @('-Step', 'report', '-Staging', $unjudgedWork.staging)
    Assert ($partial.code -eq 0 -and -not $partial.last.complete -and $partial.last.status -eq 'partial' -and $partial.last.counts.unjudged -ge 1) 'an unjudged review is partial and names its unjudged pairs'
}
finally {
    Remove-Item Env:ZT_DIGEST, Env:ZT_CHECK_ZT, Env:ZT_DROP_BATCH_REPLY_FLAG -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
if ($failed) { throw "$failed assertion(s) failed" }
Write-Output 'all passed'
