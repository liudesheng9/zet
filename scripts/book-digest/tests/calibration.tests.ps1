#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\calibration.tests.ps1 [-Out <folder>]
# #93 S25 / issue #102: calibrate the advice-only standalone checks, the Workspace title check,
# and the concept review against a reviewed set holding the proposal's examples and valid
# counterexamples. Design §15-phase-1 (D367: repeatable audit fields, R20/R21, Workspace I6, the
# anchor-title fault; calibrate on a hand-reviewed set) and D368 (report coverage is explicit,
# known examples reproduce, false-positive cases are retained, and an incomplete context cannot
# appear as `faults: 0`).
#
# The reviewed set is the `$examples` table and the concept block below. Every example names its
# source (the proposal's P9/P11 gap or the design's §14.1 fixture list), what the checker's
# contract requires, and the reviewer's annotation:
#   true_positive          the reported line names a real comprehension gap
#   false_positive         the line is reported, but the text is understandable as it stands
#   false_negative         the text is a real gap and no line is reported (a retained limit)
#   valid_counterexample   no line is reported, and the text is fine
#   unknown                the checker says it cannot decide (incomplete context, missing source)
# The run measures each family against its annotation: denominators, reproduced examples,
# false-positive and false-negative rates, and the unknowns. It writes `calibration.json` and
# `calibration.md` under -Out (default: a temporary folder) and never claims a human approval:
# the `decision` block stays `pending-human` until a reviewed decision is recorded in
# docs/luhmann-comparison-calibration.md.
#
# Read-only: the checker runs are reads; the concept review writes no Card. The scratch archive,
# its service, and every Card of it belong to this test and are removed in `finally`.
param([string]$Out = '')
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "calibration-test-$PID"
$work = Join-Path $scratch 'work'
if (-not $Out) { $Out = Join-Path $scratch 'report' }
New-Item -ItemType Directory -Force $Out, $work | Out-Null
$utf8 = [Text.UTF8Encoding]::new($false)
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$wrapper = (Join-Path $PSScriptRoot '..\concept-compiler.ps1') | Resolve-Path | ForEach-Object Path
$failed = 0
$results = [Collections.Generic.List[object]]::new()
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Write-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 20), $utf8) }
function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}

# The reviewed set: family, id, the Card it is written on, its run, its rule, the lines the
# checker's contract requires (`Naming` fragments must each appear), and the annotation.
$examples = @(
    # --- P9 local numbers: a book-local label that stands in for its content (R20) -----------
    [ordered]@{ family = 'local-numbers'; id = 'r20-label-sentence'; rule = 'R20'; run = 'r20'; card = 'r20-label-sentence'; lines = 1; naming = @('式 (12)'); annotation = 'true_positive'
        source = 'proposal P9 Gap: 113/1|e|a|a|a|a reads 式 (12) 含加成项。式 (11) 不含加成项。'; note = 'the first sentence carries 4 characters of content beside the label, the second 5 (the threshold); only the first is a fault' }
    [ordered]@{ family = 'local-numbers'; id = 'r20-label-with-content'; rule = 'R20'; run = 'r20'; card = 'r20-label-with-content'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §6.2: 式 (12) 的加成项来自需求弹性 is a reviewable statement'; note = 'the label keeps its content beside it' }
    [ordered]@{ family = 'local-numbers'; id = 'r20-chapter-label'; rule = 'R20'; run = 'r20'; card = 'r20-chapter-label'; lines = 1; naming = @('第 6 章'); annotation = 'true_positive'
        source = 'proposal P9 Change: 第 n 章 without its content'; note = 'the rest of the sentence is 4 characters' }
    [ordered]@{ family = 'local-numbers'; id = 'r20-this-section'; rule = 'R20'; run = 'r20'; card = 'r20-this-section'; lines = 1; naming = @('本节'); annotation = 'true_positive'
        source = 'proposal P9 Change: 本节 without its content'; note = '本节 stands in for the section it points at' }
    [ordered]@{ family = 'local-numbers'; id = 'r20-source-locator'; rule = 'R20'; run = 'r20'; card = 'r20-source-locator'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §6.2: a source-labelled locator 来源：[[book]] §11.D is allowed in a body'; note = 'the label lives in the locator line, so it is read as a locator' }
    [ordered]@{ family = 'local-numbers'; id = 'r20-code-and-quote'; rule = 'R20'; run = 'r20'; card = 'r20-code-and-quote'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §6.2: text checks run on authored prose, excluding code, chart fences and quotations'; note = 'a fenced block, a quotation and inline code' }
    [ordered]@{ family = 'local-numbers'; id = 'r20-title-model-number'; rule = 'R20'; run = 'r20'; card = 'r20-title-model-number'; lines = 1; naming = @('模型 1'); annotation = 'true_positive'
        source = 'design §14.1 Local references: a title with a book-local model number'; note = 'the title is a label with 3 characters of content beside it' }
    [ordered]@{ family = 'local-numbers'; id = 'r20-long-model-title'; rule = 'R20'; run = 'r20'; card = 'r20-long-model-title'; lines = 0; annotation = 'false_negative'
        source = 'proposal P9 Gap: 45/1|a 短期债务在模型 1 触发清算，在模型 3 只是阻止扩张'; note = 'the rest of this title passes the content threshold, so its book-local model numbers stay uncalibrated; a reader cannot tell what 模型 1 and 模型 3 are' }
    [ordered]@{ family = 'local-numbers'; id = 'r20-bare-section-number'; rule = 'R20'; run = 'r20'; card = 'r20-bare-section-number'; lines = 0; annotation = 'false_negative'
        source = 'proposal P9 Gap: 11.D.1 的市场方案要求产权明确且主体众多 (113/1|d|b)'; note = 'a bare 11.D.1 has no pattern: LOCAL_SIGN needs the § sign, so this book-local label form is undetected' }
    [ordered]@{ family = 'local-numbers'; id = 'r20-anchor-index'; rule = 'R20'; run = 'r20'; card = 'anchor'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §14.1: an anchor''s section index is a locator, not a claim'; note = 'the anchor body lists §1 and §2 lines' }
    # --- P9 named concepts: a recognized name is no local label (R20 counterexamples) --------
    [ordered]@{ family = 'named-concepts'; id = 'r20-first-welfare'; rule = 'R20'; run = 'named'; card = 'r20-first-welfare'; lines = 0; annotation = 'valid_counterexample'
        source = 'proposal P9 Cost and risk: 第一福利定理 holds 第 and must not be a false hit'; note = 'a recognized concept; no digit follows the name' }
    [ordered]@{ family = 'named-concepts'; id = 'r20-named-proposition-with-content'; rule = 'R20'; run = 'named'; card = 'r20-named-proposition-with-content'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §6.2: a label followed by substantial content is not automatically wrong'; note = '命题 3.G.3 with its content in the same sentence' }
    [ordered]@{ family = 'named-concepts'; id = 'r20-named-pareto'; rule = 'R20'; run = 'named'; card = 'r20-named-pareto'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §6.2: not every digit and not every 第 is a fault'; note = '帕累托最优 carries no local label' }
    # --- P9 referents: a deictic with no named referent (R21) --------------------------------
    [ordered]@{ family = 'referents'; id = 'r21-bare-there'; rule = 'R21'; run = 'referent'; card = 'r21-bare-there'; lines = 1; naming = @('那里'); annotation = 'true_positive'
        source = 'proposal P9 Gap: 144 of 200 sampled Topic Cards point with 那里, 那张卡 or 该卡'; note = 'no Link in the sentence names the referent' }
    [ordered]@{ family = 'referents'; id = 'r21-that-card'; rule = 'R21'; run = 'referent'; card = 'r21-that-card'; lines = 1; naming = @('那张卡'); annotation = 'true_positive'
        source = 'proposal P9 Gap: 那张卡'; note = 'the sentence names no Link' }
    [ordered]@{ family = 'referents'; id = 'r21-title-deictic'; rule = 'R21'; run = 'referent'; card = 'r21-title-deictic'; lines = 1; naming = @('该卡'); annotation = 'true_positive'
        source = 'proposal P9 Change: every title is checked'; note = 'the title states the claim with 该卡 only' }
    [ordered]@{ family = 'referents'; id = 'r21-named-link'; rule = 'R21'; run = 'referent'; card = 'r21-named-link'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §14.1 Referents: a named concept plus a Link in the same sentence'; note = 'the same sentence names the referent' }
    [ordered]@{ family = 'referents'; id = 'r21-previous-sentence-link'; rule = 'R21'; run = 'referent'; card = 'r21-previous-sentence-link'; lines = 1; naming = @('该卡'); annotation = 'true_positive'
        source = 'design §14.1 Referents: a named reference only in the preceding sentence does not name this one'; note = 'the Link of the sentence before is no referent of this sentence' }
    [ordered]@{ family = 'referents'; id = 'r21-named-in-words'; rule = 'R21'; run = 'referent'; card = 'r21-named-in-words'; lines = 1; naming = @('该卡'); annotation = 'false_positive'
        source = 'design §6.2: R21 can detect a same-sentence named Link, not all Chinese coreference'; note = 'the sentence names its subject in words, so the advice is wrong for a reader; the rule detects only a Link' }
    [ordered]@{ family = 'referents'; id = 'r21-code-and-quote'; rule = 'R21'; run = 'referent'; card = 'r21-code-and-quote'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §14.1 Referents: code and quotation exclusion'; note = 'a fenced block and a quotation' }
    # --- P1 own-source dependencies: a rests-on Link into the Card's own source (I7) ---------
    [ordered]@{ family = 'own-source'; id = 'i7-own-source'; rule = 'I7'; run = 'own'; card = 'i7-own-source'; lines = 1; naming = @('rests-on'); annotation = 'true_positive'
        source = 'design §6.1 I7: a rests-on Link points into an identified own-source Literature tree'; note = 'the blocking line; its source is the Card''s 来源 line' }
    [ordered]@{ family = 'own-source'; id = 'i7-illustrates'; rule = 'I7'; run = 'own'; card = 'i7-illustrates'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §6.1: illustrates, corresponds and contrasts are no dependency'; note = 'an illustrative Link' }
    [ordered]@{ family = 'own-source'; id = 'i7-other-book'; rule = 'I7'; run = 'own'; card = 'i7-other-book'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §14.1 Topic test: another book is not the own source'; note = 'a rests-on Link into another book' }
    [ordered]@{ family = 'own-source'; id = 'i7-filed-in-book'; rule = 'I7'; run = 'own'; card = 'i7-filed-in-book'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §6.1: a Card filed in that tree rests on it by design'; note = 'a Card inside the Literature tree' }
    [ordered]@{ family = 'own-source'; id = 'i7-unknown-source'; rule = 'I7'; run = 'own'; card = 'i7-unknown-source'; lines = 1; naming = @('source evidence unknown'); annotation = 'unknown'
        source = 'design §14.1 Topic test: unknown source is unknown, never a pass'; note = 'no 来源 line and no Literature tree, so the checker says it cannot decide' }
    [ordered]@{ family = 'own-source'; id = 'i7-corresponds'; rule = 'I7'; run = 'own'; card = 'i7-corresponds'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §6.1: corresponds expresses a real relation, not a premise'; note = 'the correspondence word is no repair, but a true correspondence is no fault' }
    # --- P11 duplicate titles: Workspace I6 ---------------------------------------------------
    [ordered]@{ family = 'duplicate-titles'; id = 'i6-near-pair'; rule = 'I6'; run = 'i6-both'; card = 'i6-pair-b'; lines = 1; naming = @('[[i6-pair-a]]'); annotation = 'true_positive'
        source = 'design §14.1 I6: one unordered pair is reported once, on the later checked Card'; note = 'two checked Cards of one pair give one line' }
    [ordered]@{ family = 'duplicate-titles'; id = 'i6-context-other-tree'; rule = 'I6'; run = 'i6-one'; card = 'i6-pair-a'; lines = 1; naming = @('[[i6-pair-b]]'); annotation = 'true_positive'
        source = 'design §14.1 I6: similar titles across two trees in one Workspace'; note = 'the check reaches the other tree of the Workspace' }
    [ordered]@{ family = 'duplicate-titles'; id = 'i6-context-not-subject'; rule = 'I6'; run = 'i6-one'; card = 'i6-pair-b'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §14.1 I6: an unchanged context is not a repair target'; note = 'the unchecked Card is never the subject of a line' }
    [ordered]@{ family = 'duplicate-titles'; id = 'i6-unrelated'; rule = 'I6'; run = 'i6-unrelated'; card = 'i6-unrelated'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §14.1 I6: an unrelated title raises nothing'; note = 'no bigram overlap above the threshold' }
    [ordered]@{ family = 'duplicate-titles'; id = 'i6-hub-subject'; rule = 'I6'; run = 'i6-hub'; card = 'i6-hub'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §14.1 I6: a Hub Card is no I6 subject (#93 S03-AC06)'; note = 'a hub that copies its anchor title is the anchor-title check''s matter' }
    [ordered]@{ family = 'duplicate-titles'; id = 'i6-hub-in-context'; rule = 'I6'; run = 'i6-hub-context'; card = 'i6-pair-a'; lines = 2; naming = @('[[i6-pair-b]]', '[[i6-hub]]'); annotation = 'false_positive'
        source = 'design §14.3 P11: report the Idea and the navigation/hub populations apart'; note = 'the existing Hub Card''s title is Workspace context, so the checked Idea Card gets a second advisory line naming the hub; the duplicate is the hub''s anchor-title fault, not the Idea Card''s, and the two populations P11 measures apart are mixed. Advice only; no Card changes.' }
    [ordered]@{ family = 'duplicate-titles'; id = 'i6-other-workspace'; rule = 'I6'; run = 'i6-json-workspace'; card = '12/9|a'; lines = 0; annotation = 'valid_counterexample'
        source = 'design §14.1 I6: the same title in another Workspace is no pair'; note = 'an offline bundle: the human Workspace is read-only for agents, so this case runs through --json' }
    [ordered]@{ family = 'duplicate-titles'; id = 'i6-incomplete-context'; rule = 'I6'; run = 'i6-json-incomplete'; card = '12/9|b'; lines = 2; naming = @('Workspace title context incomplete', '[[12/9|c]]'); annotation = 'unknown'
        source = 'design D136/D368: an incomplete context marks I6 unknown instead of claiming a Workspace-wide check'; note = 'the line names how many titles were compared and the known rows are still compared; `faults: 0` of that mode can then never read as a complete pass' }
    [ordered]@{ family = 'duplicate-titles'; id = 'i6-scope-unknown'; rule = 'I6'; run = 'i6-json-scope'; card = '12/9|c'; lines = 1; naming = @('Workspace scope unknown'); annotation = 'unknown'
        source = 'design D136: an entry that names no Workspace is not compared at all'; note = 'no pair may cross a Workspace boundary' }
)

# The recorded address of every Card the reviewed examples name.
$script:address = @{}

# A naming fragment may name a fixture Card symbolically; the checker prints its address.
function Resolve-Name($fragment) {
    [regex]::Replace($fragment, '\[\[([^\]]+)\]\]', {
            param($m)
            $name = $m.Groups[1].Value
            '[[' + $(if ($script:address.ContainsKey($name)) { $script:address[$name] } else { $name }) + ']]'
        })
}

# One example's observation, recorded with its annotation.
function Record-Example($example, $observed, $mode) {
    $ok = ($observed.Count -eq $example.lines)
    foreach ($naming in @($example.naming)) {
        $fragment = Resolve-Name $naming
        $hit = @($observed | Where-Object { $_.Contains($fragment) }).Count
        if ($ok -and $example.lines -gt 0 -and $hit -lt 1) { $ok = $false }
        if ($ok -and $example.lines -eq 0 -and $hit -gt 0) { $ok = $false }
    }
    $script:results.Add([ordered]@{
            family = $example.family; id = $example.id; rule = $example.rule; run = $example.run
            mode = $mode; address = $(if ($script:address.ContainsKey($example.card)) { $script:address[$example.card] } else { $example.card }); expected_lines = $example.lines
            observed_lines = $observed.Count; reproduced = $ok; annotation = $example.annotation
            source = $example.source; note = $example.note; observed = @($observed)
        })
    if (-not $ok) { $script:failed++ }
    $mark = if ($ok) { 'ok' } else { 'FAIL' }
    Write-Output ("{0}: {1} {2} {3}: {4} line(s), annotation {5}" -f $mark, $example.family, $example.rule, $example.id, $observed.Count, $example.annotation)
    if (-not $ok) { Write-Output ("      observed: " + (@($observed) -join ' | ')) }
}

# The concept fixtures: the book's two learning senses and the shared formula, as Cards.
function New-ConceptFixture($book) {
    $topics = Write-Cards @(
        @{ tool = 'create_topic'; arguments = @{ title = '学习'; body = '本主题追问：学习怎样改变行为？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '控制权'; body = '本主题追问：谁在契约没写的地方做决定？' } }
    )
    $learn = $topics[0] -replace '/0$', ''
    $control = $topics[1] -replace '/0$', ''
    $first = Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = "$learn/0"; title = '重复练习提高个体技能'; body = "同一个体重复练习，技能提高。`n`n来源：[[$book]] §3.1" } })
    $second = Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = $first[0]; kind = 'side'; title = '累计产量提高产业生产率'; body = "产业累计产量每翻一倍，单位成本按固定比例下降。`n`n来源：[[$book]] §4.2" } })
    $entries = @($first[0], $second[0])
    $statements = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$book/1"; kind = 'side'; title = '对称参与者的重复博弈有两个均衡'; body = "当策略互补且参与者对称时，`$x_i=\arg\max_x u_i(x,x_{-i})`$ 有两个均衡。`n`n假设：策略互补、参与者对称。`n`n来源：[[$book]] §2.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$book/1"; kind = 'side'; title = '策略互补时同一经济有多个均衡'; body = "当策略互补时，同一个经济可以有多个均衡：`$x_i=\arg\max_x u_i(x,x_{-i})`$。`n`n假设：策略互补。`n`n来源：[[$book]] §2.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$book/1"; kind = 'side'; title = '债务契约把控制权交给债权人'; body = "违约时债权人取得资产的控制权。`n`n来源：[[$book]] §1" } }
    )
    [pscustomobject]@{ learn = $learn; control = $control; entries = @($entries); statements = @($statements) }
}

# The concept review's four steps over the fixture, with the reviewed canned answers.
function Invoke-ConceptReview($book, $fixture) {
    $learn = $fixture.learn
    $formulaStrong = $fixture.statements[0]
    $formulaIdentical = $fixture.statements[1]
    $debt = $fixture.statements[2]
    $staging = Join-Path $script:scratch 'staging'
    New-Item -ItemType Directory -Force (Join-Path $staging 'concepts') | Out-Null
    Write-Json (Join-Path $staging 'manifest.json') @{ citation_key = $book; book_title = '校准用书'; groups = @(@{ n = 1; chapter = '第一章'; headings = @('学习') }) }
    Write-Json (Join-Path $staging 'concepts\proposals.json') @{ schema_version = 1; proposals = @(
            @{ title = '学习（个体行为）'; question = '个体重复练习怎样改变他的技能？'; statement = '个体重复同一任务会提高他的技能。'; assumptions = @('同一个体'); terms = @('学习（个体行为）') },
            @{ title = '学习（产业生产率）'; question = '产业整体的生产率为什么随累计产量提高？'; statement = '产业累计产量提高会降低单位成本。'; assumptions = @('同一产业'); terms = @('学习（产业生产率）') },
            @{ title = '均衡的多重性'; question = '什么条件下同一个经济有多个均衡？'; statement = '当策略互补时，同一个经济可以有多个均衡：$x_i=\arg\max_x u_i(x,x_{-i})$。'; assumptions = @('策略互补'); terms = @('均衡的多重性') }
        ) }
    $step = {
        param([string[]]$arguments)
        $text = @(& pwsh -NoProfile -File $script:wrapper @arguments 2>&1 | ForEach-Object { [string]$_ }) -join "`n"
        $code = $LASTEXITCODE
        $last = if (-not $code) { ($text -split "`n")[-1] | ConvertFrom-Json -AsHashtable -DateKind String } else { $null }
        [pscustomobject]@{ code = $code; text = $text; last = $last }
    }
    $prepared = & $step @('-Step', 'prepare', '-Staging', $staging)
    if ($prepared.code) { throw "concepts prepare failed: $($prepared.text)" }
    $evidence = Read-Json (Join-Path $staging 'concepts\evidence.json')
    # The reviewed answers: the two learning senses stay apart, the shared formula under a
    # narrower scope stays a difference, the identical claim is one statement.
    $verdict = @{
        "$learn/0|p001" = @{ verdict = 'different'; reason = '这个主题同时收了个体技能的改变与产业生产率的提高，两个意思必须分开。' }
        "$learn/0|p002" = @{ verdict = 'different'; reason = '产业生产率的提高不是同一个个体练习带来的技能变化，两个意思必须分开。' }
        "$formulaStrong|p003" = @{ verdict = 'different'; reason = '公式相同，但候选卡以参与者对称为前提，提议的陈述没有这个前提。'; assumptions = @('策略互补', '参与者对称') }
        "$formulaIdentical|p003" = @{ verdict = 'same'; scope = '策略互补时同一经济可以有多个均衡'; reason = '公式与假设都相同，是同一个陈述。' }
    }
    $rows = @()
    foreach ($proposal in @($evidence.proposals)) {
        foreach ($candidate in @($proposal.topic_candidates)) {
            $key = "$($candidate.address)|$($proposal.id)"
            $answer = if ($verdict.ContainsKey($key)) { $verdict[$key] } else { @{ verdict = 'different'; reason = '候选卡与提议讨论的条件不同。' } }
            $rows += [ordered]@{ kind = 'topic-concept'; proposal = $proposal.id; candidate = $candidate.address; verdict = $answer.verdict
                scope = $answer.scope; reason = $answer.reason; assumptions = @($answer.assumptions); evidence = @($candidate.address) }
        }
        foreach ($candidate in @($proposal.restatement_candidates)) {
            $key = "$($candidate.address)|$($proposal.id)"
            $answer = if ($verdict.ContainsKey($key)) { $verdict[$key] } else { @{ verdict = 'different'; reason = '两处陈述的假设或对象不同。' } }
            $assumptions = if ($answer.assumptions) { @($answer.assumptions) } else { @($candidate.assumptions) }
            $rows += [ordered]@{ kind = 'restatement'; proposal = $proposal.id; candidate = $candidate.address; verdict = $answer.verdict
                scope = $answer.scope; reason = $answer.reason; assumptions = $assumptions; evidence = @($candidate.address) }
        }
    }
    $answerPath = Join-Path $staging 'concepts\out\cj-01.json'
    New-Item -ItemType Directory -Force (Split-Path -Parent $answerPath) | Out-Null
    Write-Json $answerPath @{ schema_version = 1; evidence_id = $evidence.evidence_id; judgments = @($rows) }
    $judged = & $step @('-Step', 'judge', '-Staging', $staging, '-File', $answerPath)
    if ($judged.code) { throw "concepts judge failed: $($judged.text)" }
    # The report makes no tool call, so it cannot write a Card.
    $env:ZT_TRACE_TOOLS = Join-Path $script:scratch 'report-tools.jsonl'
    [IO.File]::WriteAllText($env:ZT_TRACE_TOOLS, '')
    try { $reported = & $step @('-Step', 'report', '-Staging', $staging) } finally { Remove-Item Env:ZT_TRACE_TOOLS -ErrorAction SilentlyContinue }
    if ($reported.code) { throw "concepts report failed: $($reported.text)" }
    $review = Read-Json (Join-Path $staging 'concept-review.json')
    $findings = @($review.findings)
    $finding = {
        param($proposal, $candidate, $kind)
        @($findings | Where-Object { $_.kind -eq $kind -and $_.proposal -eq $proposal -and $_.candidate -eq $candidate }) | Select-Object -First 1
    }
    $expectations = [Collections.Generic.List[object]]::new()
    $expect = {
        param($id, $kind, $proposal, $candidate, $expected, $observed, $annotation, $source, $note, $detail)
        $expectations.Add([ordered]@{ id = $id; kind = $kind; proposal = $proposal; candidate = $candidate
                address = $candidate; expected = $expected; observed = $observed; reproduced = ($expected -eq $observed)
                annotation = $annotation; source = $source; note = $note; detail = @($detail) })
    }
    $individual = & $finding 'p001' "$learn/0" 'topic-concept'
    $individualText = if ($individual) { "a $($individual.verdict) finding" } else { 'no finding' }
    & $expect 'senses-individual' 'topic-concept' 'p001' "$learn/0" 'a different finding' $individualText 'true_positive' 'proposal P5: 179 学习 and 214 学习型经济 may not be one meaning; the judge keeps them apart' 'the individual-learning proposal is not the two-sense Topic' @($individual.verdict, $individual.reason)
    $industry = & $finding 'p002' "$learn/0" 'topic-concept'
    $industryText = if ($industry) { "a $($industry.verdict) finding" } else { 'no finding' }
    & $expect 'senses-industry' 'topic-concept' 'p002' "$learn/0" 'a different finding' $industryText 'true_positive' 'proposal P5: the same two senses' 'the industry-learning proposal is not the two-sense Topic' @($industry.verdict, $industry.reason)
    $formula = & $finding 'p003' $formulaStrong 'restatement'
    $formulaOk = $formula -and $formula.verdict -eq 'different' -and $formula.formula_match -eq $true -and @($formula.remedies.action) -contains 'retain_both'
    $formulaText = if ($formulaOk) { 'a different finding with the formula and retain_both' } elseif ($formula) { "a $($formula.verdict) finding" } else { 'no finding' }
    & $expect 'formula-different-scope' 'restatement' 'p003' $formulaStrong 'a different finding with the formula and retain_both' $formulaText 'true_positive' 'design §14.1 Restatements: the same formula under stronger assumptions' 'the same formula with a narrower scope stays a difference' @($formula.verdict, $formula.formula_match, (@($formula.remedies.action) -join ','))
    $identical = & $finding 'p003' $formulaIdentical 'restatement'
    $identicalText = if ($identical) { "a $($identical.verdict) finding" } else { 'no finding' }
    & $expect 'identical-claim' 'restatement' 'p003' $formulaIdentical 'a same finding' $identicalText 'true_positive' 'design §14.1 Restatements: an identical claim with the same assumptions' 'the same formula with the same assumptions is one statement, so the tool does not call everything different' @($identical.verdict, $identical.scope)
    $topicTitles = @(@($evidence.proposals | Where-Object { $_.id -eq 'p001' })[0].topic_candidates | ForEach-Object { $_.title })
    & $expect 'shortlist-precision-topic' 'candidate_absent' 'p001' $fixture.control 'the unrelated Topic absent' $(if ($topicTitles -contains '控制权') { 'the unrelated Topic present' } else { 'the unrelated Topic absent' }) 'valid_counterexample' 'design §7.1: a bounded shortlist with its unselected count' 'a Topic that shares no evidence with the proposal is no candidate' $topicTitles
    $statementTitles = @(@($evidence.proposals | Where-Object { $_.id -eq 'p003' })[0].restatement_candidates | ForEach-Object { $_.title })
    & $expect 'shortlist-precision-statement' 'candidate_absent' 'p003' $debt 'the unrelated statement absent' $(if ($statementTitles -contains '债务契约把控制权交给债权人') { 'the unrelated statement present' } else { 'the unrelated statement absent' }) 'valid_counterexample' 'design §14.1 Restatements: an unrelated statement is no candidate' 'a statement about debt control is no restatement of the multiplicity claim' $statementTitles
    $tableRows = @($evidence.term_table.rows)
    $learningRows = @($tableRows | Where-Object { $_.term -like '学习*' })
    $senseText = if ($learningRows.Count -eq 2 -and @($learningRows.meaning | Select-Object -Unique).Count -eq 2) { 'two 学习 rows with distinct meanings' } else { "$($learningRows.Count) rows, $(@($learningRows.meaning | Select-Object -Unique).Count) meanings" }
    & $expect 'term-rows-two-senses' 'term_rows' 'p001' $learn 'two 学习 rows with distinct meanings' $senseText 'true_positive' 'proposal P5: the book''s term table is the evidence of two senses' 'the checker''s term-table parser supplies both meanings' @($learningRows | ForEach-Object { "$($_.term)：$($_.meaning)" })
    [pscustomobject]@{
        expectations = @($expectations); learn = $learn; control = $fixture.control; entries = @($fixture.entries)
        statements = @($fixture.statements); archive_id = $prepared.last.archive_id
        archive_revision = $prepared.last.archive_revision; term_rows = $tableRows.Count
        shortlisted_topics = $topicTitles; shortlisted_statements = $statementTitles
        judged = $judged.last.judged; report_status = $review.status; judge_exercised = $false
    }
}

# The report: every example, its annotation, and each family's counts and rates.
function Write-CalibrationReport($concept) {
    $families = @(
        [ordered]@{ id = 'local-numbers'; rule = 'R20'; measure = 'P9'; question = 'A book-local label that stands in for its content'
            coverage = 'The title and the authored Topic prose; a source locator, code, a quotation and a navigation Card are excluded. Two label forms the proposal''s P9 gap names are undetected and retained as false negatives: a long title whose model numbers pass the content threshold, and a bare 11.D.1 (LOCAL_SIGN needs the §).' }
        [ordered]@{ id = 'named-concepts'; rule = 'R20'; measure = 'P9'; question = 'A recognized concept name is no local label'
            coverage = '第一福利定理 (the proposal''s own false-hit risk), 命题 3.G.3 with its content in the same sentence, and a name carrying no digit at all.' }
        [ordered]@{ id = 'referents'; rule = 'R21'; measure = 'P9'; question = 'A deictic with no named referent in its sentence'
            coverage = 'Each sentence and each title; a Link in the same sentence names the referent and a Link in the sentence before does not; code and quotations are excluded. A referent named in words without a Link is still reported and is retained as a false positive (the rule detects only a Link).' }
        [ordered]@{ id = 'own-source'; rule = 'I7'; measure = 'P1'; question = 'A rests-on Link into the Card''s own source'
            coverage = 'Topic Idea Cards that carry relation evidence: illustrates, corresponds, contrasts, another book and a Card filed in that Literature tree are no fault; missing source evidence is an unknown line, never a pass. The own-source case takes its source evidence from the 来源 line and is the one hard line.' }
        [ordered]@{ id = 'duplicate-titles'; rule = 'I6'; measure = 'P11'; question = 'A title very similar to another of the same Workspace'
            coverage = 'The Workspace title context of --cards: across trees, one line per unordered pair, a context Card is never the subject, a Hub Card is no subject, another Workspace is no pair, and an incomplete context or an entry naming no Workspace is an explicit unknown. An existing hub whose title copies its anchor is context, so the checked Idea Card gets an extra line: the Idea and navigation populations P11 measures apart are mixed, retained as a false positive.' }
    )
    $blocks = @()
    foreach ($family in $families) {
        $mine = @($script:results | Where-Object { $_.family -eq $family.id })
        $count = { param($name) @($mine | Where-Object { $_.annotation -eq $name }).Count }
        $tp = & $count 'true_positive'; $fp = & $count 'false_positive'; $fn = & $count 'false_negative'
        $vc = & $count 'valid_counterexample'; $unknown = & $count 'unknown'
        $blocks += [ordered]@{
            family = $family.id; rule = $family.rule; measure = $family.measure; question = $family.question
            coverage = $family.coverage
            counts = [ordered]@{ examples = $mine.Count; fired = @($mine | Where-Object { $_.observed_lines -gt 0 }).Count
                silent = @($mine | Where-Object { $_.observed_lines -eq 0 }).Count
                true_positive = $tp; false_positive = $fp; false_negative = $fn
                valid_counterexample = $vc; unknown = $unknown
                reproduced = @($mine | Where-Object { $_.reproduced }).Count }
            rates = [ordered]@{
                false_positive = $(if (($tp + $fp) -gt 0) { [math]::Round($fp / ($tp + $fp), 4) } else { $null })
                false_negative = $(if (($tp + $fn + $unknown) -gt 0) { [math]::Round($fn / ($tp + $fn + $unknown), 4) } else { $null })
            }
            examples = @($mine)
        }
    }
    $conceptMine = @($concept.expectations)
    $blocks += [ordered]@{
        family = 'concept'; rule = 'concepts'; measure = 'P5/P2'; question = 'Two learning senses and a shared formula under different scope'
        coverage = 'The book''s own term table supplies two 学习 senses; a shared formula under a narrower scope stays a difference with both statements retained; an identical claim is one statement; the deterministic shortlist keeps an unrelated Topic and an unrelated statement out. The semantic judge itself is not exercised here (canned answers).'
        counts = [ordered]@{ examples = $conceptMine.Count; fired = @($conceptMine | Where-Object { $_.reproduced }).Count
            silent = 0; true_positive = @($conceptMine | Where-Object { $_.annotation -eq 'true_positive' }).Count
            false_positive = 0; false_negative = 0
            valid_counterexample = @($conceptMine | Where-Object { $_.annotation -eq 'valid_counterexample' }).Count
            unknown = 0; reproduced = @($conceptMine | Where-Object { $_.reproduced }).Count }
        rates = [ordered]@{ false_positive = 0; false_negative = 0 }
        examples = @($conceptMine | ForEach-Object { [ordered]@{ id = $_.id; rule = "concepts:$($_.kind)"; expected = $_.expected; observed = $_.observed; annotation = $_.annotation; source = $_.source; note = $_.note } })
        shortlisted = [ordered]@{ topics_p001 = @($concept.shortlisted_topics); statements_p003 = @($concept.shortlisted_statements) }
        judge = [ordered]@{ exercised = $concept.judge_exercised; judged_pairs = $concept.judged; report = $concept.report_status
            note = 'canned answers stand in for the semantic judge; its accuracy is not measured here and remains the pilot''s evidence (S26)' }
    }
    $advisor = [ordered]@{}
    foreach ($name in 'r20', 'named', 'referent', 'own', 'i6-both', 'i6-one', 'i6-hub-context') {
        $lines = @($script:archiveLines[$name])
        $advisor[$name] = [ordered]@{
            run = $name; rules = $script:runRules[$name]
            printed = @($lines | Where-Object { $_ -notmatch '^faults: ' }).Count
            advice = @($lines | Where-Object { $_ -match ' \(advisory\) ' }).Count
            counted_faults = $(if ($lines.Count) { [int](($lines[-1] -split ': ')[1]) } else { $null })
        }
    }
    $report = [ordered]@{
        schema_version = 1; gate = 'S25'; issue = 102
        scope = [ordered]@{
            modes = @('zt-check --cards on a scratch archive', 'zt-check --json bundle (the audit''s contract)', 'zt-digest concepts prepare/judge/report')
            archive_root = (Join-Path $script:scratch 'archive'); archive_id = $concept.archive_id; archive_revision = $concept.archive_revision
            concept_cards = [ordered]@{ learning_topic = $concept.learn; control_topic = $concept.control; learning_entries = @($concept.entries); statements = @($concept.statements) }
            set_fingerprint = (Get-Fingerprint)
            exact_sample = @($script:results | ForEach-Object { $_.address } | Select-Object -Unique)
        }
        versions = [ordered]@{
            zt = (& zt version | Out-String).Trim(); zt_check = (& zt-check --version | Out-String).Trim(); zt_digest = (& zt-digest version | Out-String).Trim()
            rules = @('R20', 'R21', 'I7', 'I6'); thresholds = [ordered]@{ local_reference_content = 5; title_similarity = 0.8; layer_e5 = 0.5 }
            note = 'the thresholds are the constants of tools/zt-check/src/check.rs; a change to one is a new calibrated version'
        }
        families = $blocks
        completeness = [ordered]@{
            incomplete_context_line = @($script:jsonLines['i6-json-incomplete'] | Where-Object { $_ -match 'Workspace title context incomplete' }).Count
            unnamed_workspace_line = @($script:jsonLines['i6-json-scope'] | Where-Object { $_ -match 'Workspace scope unknown' }).Count
            missing_source_line = @($script:archiveLines['own'] | Where-Object { $_ -match 'source evidence unknown' }).Count
            note = 'a run whose context is incomplete states it in a line, so its zero faults cannot read as a complete pass; a missing source is an unknown line, never a pass (D368)'
        }
        advice_only = [ordered]@{ runs = $advisor; statement = 'R20, R21, I6 and I7''s unknown-source line are printed as (advisory) and counted as no fault; a promotion to blocking is a separately versioned decision (S25-AC03)' }
        semantic_review = [ordered]@{ required = $true; evidence = @(
                'scripts/book-digest/dsh-card-instructions.zh.md: I6 is advice; a title match alone deletes or merges nothing',
                'skills/zt-digest-protocol/SKILL.md: I6 is advice like R20 and R21; the semantic Card review still requires standalone text',
                'docs/wiki/zt-check.md: R20 and R21 are advisory and counted apart from blocking faults'
            ); note = 'the semantic Card review stays required during the pilot; the heuristic lines never replace it' }
        decision = [ordered]@{ state = 'pending-human'; criteria = 'S25-AC04'; note = 'a human approves or returns specific classifier or prompt corrections with retained evidence; elapsed time is no approval. Recorded in docs/luhmann-comparison-calibration.md.' }
    }
    Write-Json (Join-Path $script:Out 'calibration.json') $report
    # A table cell must not carry a raw pipe: a source line can name an address path.
    function Esc($text) { if ($null -eq $text) { '' } else { [string]$text -replace '\|', '\|' } }
    $md = [Text.StringBuilder]::new()
    [void]$md.AppendLine('# Calibration of the advice-only checks (S25)')
    [void]$md.AppendLine('')
    [void]$md.AppendLine("Generated by ``scripts/book-digest/tests/calibration.tests.ps1``. Decision: **$($report.decision.state)**.")
    [void]$md.AppendLine('')
    [void]$md.AppendLine('| Family | Rule | Examples | Reproduced | TP | FP | FN | Counterexamples | Unknown | FP rate | FN rate |')
    [void]$md.AppendLine('| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |')
    foreach ($block in $blocks) {
        $c = $block.counts; $r = $block.rates
        [void]$md.AppendLine("| $($block.family) | $($block.rule) | $($c.examples) | $($c.reproduced) | $($c.true_positive) | $($c.false_positive) | $($c.false_negative) | $($c.valid_counterexample) | $($c.unknown) | $($r.false_positive) | $($r.false_negative) |")
    }
    [void]$md.AppendLine('')
    [void]$md.AppendLine('## Reviewed examples')
    [void]$md.AppendLine('')
    [void]$md.AppendLine('| Id | Rule | Expected | Observed | Annotation | Source |')
    [void]$md.AppendLine('| --- | --- | --- | --- | --- | --- |')
    foreach ($block in $blocks) { foreach ($example in @($block.examples)) {
            $expected = if ($null -ne $example.expected_lines) { $example.expected_lines } else { $example.expected }
            $observed = if ($null -ne $example.observed_lines) { $example.observed_lines } else { $example.observed }
            [void]$md.AppendLine("| $($example.id) | $($example.rule) | $(Esc $expected) | $(Esc $observed) | $($example.annotation) | $(Esc $example.source) |")
        } }
    [void]$md.AppendLine('')
    [void]$md.AppendLine('## Coverage')
    [void]$md.AppendLine('')
    foreach ($block in $blocks) { [void]$md.AppendLine("- **$($block.family)** ($($block.rule), $($block.measure)): $($block.coverage)") }
    [void]$md.AppendLine('')
    [void]$md.AppendLine('## Advice-only')
    [void]$md.AppendLine('')
    [void]$md.AppendLine('| Run | Rules | Lines printed | Advisory | Counted faults |')
    [void]$md.AppendLine('| --- | --- | --- | --- | --- |')
    foreach ($name in $advisor.Keys) { $row = $advisor[$name]; [void]$md.AppendLine("| $name | $($row.rules) | $($row.printed) | $($row.advice) | $($row.counted_faults) |") }
    [void]$md.AppendLine('')
    [void]$md.AppendLine('## Unknowns')
    [void]$md.AppendLine('')
    [void]$md.AppendLine("- Incomplete title-context lines: $($report.completeness.incomplete_context_line).")
    [void]$md.AppendLine("- Unnamed-Workspace lines: $($report.completeness.unnamed_workspace_line).")
    [void]$md.AppendLine("- Missing-source unknown lines: $($report.completeness.missing_source_line).")
    [void]$md.AppendLine('- Semantic judgments exercised: no (canned answers); the judge''s own accuracy stays unknown until the S26 pilot.')
    [IO.File]::WriteAllText((Join-Path $script:Out 'calibration.md'), $md.ToString(), $utf8)
    Write-Output ''
    Write-Output ("calibration report: {0}" -f (Join-Path $script:Out 'calibration.json'))
    foreach ($block in $blocks) {
        $c = $block.counts
        Write-Output ("  {0,-16} {1,-9} examples={2} reproduced={3} tp={4} fp={5} fn={6} vc={7} unknown={8}" -f $block.family, $block.rule, $c.examples, $c.reproduced, $c.true_positive, $c.false_positive, $c.false_negative, $c.valid_counterexample, $c.unknown)
    }
    Assert (@($script:results | Where-Object { -not $_.reproduced }).Count -eq 0) 'every reviewed checker example reproduces the contract it is annotated with'
    Assert (@($blocks | Where-Object { $_.family -ne 'concept' -and $_.counts.reproduced -ne $_.counts.examples }).Count -eq 0) 'each checker family covers every example it holds'
    Assert ((Get-Fingerprint) -match '^[0-9a-f]{64}$') 'the reviewed set has a stable fingerprint'
}

function Get-Fingerprint {
    $canon = @($examples | ForEach-Object { [ordered]@{ family = $_.family; id = $_.id; rule = $_.rule; lines = $_.lines; annotation = $_.annotation; source = $_.source } }) | ConvertTo-Json -Depth 6 -Compress
    $bytes = [Text.Encoding]::UTF8.GetBytes($canon)
    ([Security.Cryptography.SHA256]::Create().ComputeHash($bytes) | ForEach-Object { $_.ToString('x2') }) -join ''
}

try {
    if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force }
    New-Item -ItemType Directory -Force $Out, $work | Out-Null
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    # --- the fixture ------------------------------------------------------------------------
    # Each family is one main Card of its Topic plus that family's other Cards as its side
    # successors, so every reviewed Card keeps the address its example names.
    function New-Family($root, $cards) {
        $first = Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = "$root/0"; title = $cards[0].title; body = $cards[0].body } })
        $rest = @()
        if ($cards.Count -gt 1) {
            $calls = @($cards[1..($cards.Count - 1)] | ForEach-Object {
                    @{ tool = 'create_successor'; arguments = @{ at = $first[0]; kind = 'side'; title = $_.title; body = $_.body } }
                })
            $rest = Write-Cards $calls
        }
        , (@($first[0]) + @($rest))
    }
    $book = Write-Cards @(
        @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{Cal2026, title={Calibration Examples}, author={Owner, A.}, year={2026}}'; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Cal2026'; title = '第 1 组：局部引用与指代'; body = "§1 局部引用`n`n§2 指代" } },
        @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{Other2026, title={Other Book}, author={Other, B.}, year={2026}}' } }
    )
    $key, $structure, $otherBook = $book
    $table = @'
本卡是《校准用书》的术语表。本书的卡片用左列的术语，不用右列的词。

术语表

| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 学习（个体行为） | 学习 | 个体重复练习带来的技能变化 | §3.1 |
| 学习（产业生产率） | 学习 | 产业整体的生产率随累计产量提高 | §4.2 |
| 均衡的多重性 | 多重均衡 | 同一个经济在给定条件下停在多个均衡的可能性 | §2.1 |

来源：[[Cal2026]]
'@
    $null = Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = $structure; kind = 'side'; title = '术语表：《校准用书》的术语'; body = $table } })
    $roots = [ordered]@{}
    foreach ($name in 'local', 'named', 'referent', 'own', 'titles', 'titles-b') {
        $roots[$name] = (Write-Cards @(@{ tool = 'create_topic'; arguments = @{ title = "校准：$name"; body = "本主题追问：$name 的例子说明什么？" } }))[0] -replace '/0$', ''
    }
    $symbol = @{}
    $local = New-Family $roots.local @(
        @{ title = '加成项只出现在其中一个式子'; body = "式 (12) 含加成项。式 (11) 不含加成项。`n`n来源：[[Cal2026]] §1" },
        @{ title = '加成项来自需求弹性'; body = "式 (12) 的加成项来自需求弹性。`n`n命题 3.G.3 说明均衡唯一。`n`n来源：[[Cal2026]] §1" },
        @{ title = '章节结论给出政策建议'; body = "第 6 章给了结论。`n`n来源：[[Cal2026]] §6" },
        @{ title = '本节的结论是产权重要'; body = "结果见本节。`n`n来源：[[Cal2026]] §7" },
        @{ title = '来源行的定位可以带编号'; body = "式 (12) 的加成项来自需求弹性。`n`n来源：[[Cal2026]] §11.D" },
        @{ title = '代码与引文里的编号'; body = "``````n式 (12) 有该项`n```````n`n> 式 (12) 有该项`n`n``式 (12)`` 有该项。" },
        @{ title = '模型 1 的设定'; body = '模型 1 的设定说明企业如何选择产量。' },
        @{ title = '短期债务在模型 1 触发清算，在模型 3 只是阻止扩张'; body = '短期债务在第一个模型里触发清算，在第三个模型里只是阻止扩张；差别来自债务期限。' },
        @{ title = '11.D.1 的市场方案要求产权明确且主体众多'; body = '该方案要求产权明确，并且参与者人数足够多，价格才能分散。' }
    )
    $named = New-Family $roots.named @(
        @{ title = '第一福利定理在竞争均衡成立'; body = '第一福利定理在竞争均衡下成立。' },
        @{ title = '命题 3.G.3 说明均衡唯一'; body = '命题 3.G.3 说明均衡唯一，因为两条反应曲线只交一次。' },
        @{ title = '帕累托最优要求边际替代率相等'; body = '帕累托最优要求两个人的边际替代率相等。' }
    )
    $referent = New-Family $roots.referent @(
        @{ title = '均衡的存在性'; body = '那里给出了答案。' },
        @{ title = '均衡的唯一性'; body = '那张卡给出了结论。' },
        @{ title = '该卡说明了均衡'; body = '同一机制在两种情形下都成立，所以均衡只有一个。' },
        @{ title = '代码与引文里的指代'; body = "``````n该卡有该项`n```````n`n> 该卡有该项" }
    )
    $own = New-Family $roots.own @(
        @{ title = '剩余控制权提高谈判地位'; body = "依赖 [[$structure]] 的设定：所有者能拒绝合作。`n`n来源：[[Cal2026]] §1" },
        @{ title = '两期模型例示同一机制'; body = "例示 [[$structure]] 的两期模型。`n`n来源：[[Cal2026]] §1" },
        @{ title = '专用性投资不可转用'; body = "依赖 [[$otherBook]] 的前提：投资不可转用。`n`n来源：[[Cal2026]] §2" },
        @{ title = '谈判地位决定事后分配'; body = "依赖 [[$structure]] 的结论：事后分配由地位决定。" },
        @{ title = '两处均衡相互对应'; body = "对应 [[$structure]] 的均衡。`n`n来源：[[Cal2026]] §1" }
    )
    $titles = New-Family $roots.titles @(
        @{ title = '剩余控制权决定契约空白处'; body = '所有者拒绝合作的能力决定契约没有写明的部分。' },
        @{ title = '专用性投资提高合作的剩余'; body = '投资转用以后价值下降，双方因此愿意合作。' }
    )
    $titlesB = New-Family $roots.'titles-b' @(
        @{ title = '剩余控制权决定契约的空白处'; body = '同一机制换了措辞，写在另一条主题树上。' }
    )
    $symbol['anchor'] = $structure
    $symbol['r20-label-sentence'] = $local[0]; $symbol['r20-label-with-content'] = $local[1]; $symbol['r20-chapter-label'] = $local[2]
    $symbol['r20-this-section'] = $local[3]; $symbol['r20-source-locator'] = $local[4]; $symbol['r20-code-and-quote'] = $local[5]
    $symbol['r20-title-model-number'] = $local[6]; $symbol['r20-long-model-title'] = $local[7]; $symbol['r20-bare-section-number'] = $local[8]
    $symbol['r20-first-welfare'] = $named[0]; $symbol['r20-named-proposition-with-content'] = $named[1]; $symbol['r20-named-pareto'] = $named[2]
    $symbol['r21-bare-there'] = $referent[0]; $symbol['r21-that-card'] = $referent[1]; $symbol['r21-title-deictic'] = $referent[2]; $symbol['r21-code-and-quote'] = $referent[3]
    $symbol['i7-own-source'] = $own[0]; $symbol['i7-illustrates'] = $own[1]; $symbol['i7-other-book'] = $own[2]
    $symbol['i7-unknown-source'] = $own[3]; $symbol['i7-corresponds'] = $own[4]
    $symbol['i6-pair-a'] = $titles[0]; $symbol['i6-unrelated'] = $titles[1]; $symbol['i6-pair-b'] = $titlesB[0]
    # Cards that name a Card created above, and the concept fixtures.
    $linked = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = $symbol['r21-bare-there']; kind = 'side'; title = '均衡方法的来源'; body = "该卡 [[$($symbol['r21-bare-there'])]] 说明了均衡。" } },
        @{ tool = 'create_successor'; arguments = @{ at = $symbol['r21-bare-there']; kind = 'side'; title = '均衡结果的来源'; body = "先看 [[$($symbol['r21-bare-there'])]] 的方法。该卡给出了结果。" } },
        @{ tool = 'create_successor'; arguments = @{ at = $symbol['r21-bare-there']; kind = 'side'; title = '授权之后的结论'; body = '剩余控制权的讨论表明，授权之后该卡成立。' } },
        @{ tool = 'create_successor'; arguments = @{ at = $structure; kind = 'side'; title = '企业家用控制权换取融资'; body = "依赖 [[$structure]] 的设定：控制权随状态转移。`n`n来源：[[Cal2026]] §1" } }
    )
    $symbol['r21-named-link'] = $linked[0]; $symbol['r21-previous-sentence-link'] = $linked[1]; $symbol['r21-named-in-words'] = $linked[2]
    $symbol['i7-filed-in-book'] = $linked[3]
    $conceptFixture = New-ConceptFixture $key
    $script:address = $symbol
    $script:scratch = $scratch; $script:Out = $Out

    # --- the archive runs: the public `zt-check --cards` contract -----------------------------
    $script:runRules = @{ 'r20' = 'R20'; 'named' = 'R20'; 'referent' = 'R21'; 'own' = 'I7'; 'i6-one' = 'I6'; 'i6-both' = 'I6'; 'i6-hub' = 'I6'; 'i6-hub-context' = 'I6'; 'i6-unrelated' = 'I6' }
    $runCards = @{
        'r20'          = @($symbol['r20-label-sentence'], $symbol['r20-label-with-content'], $symbol['r20-chapter-label'], $symbol['r20-this-section'],
            $symbol['r20-source-locator'], $symbol['r20-code-and-quote'], $symbol['r20-title-model-number'], $symbol['r20-long-model-title'],
            $symbol['r20-bare-section-number'], $symbol['anchor'])
        'named'        = @($symbol['r20-first-welfare'], $symbol['r20-named-proposition-with-content'], $symbol['r20-named-pareto'])
        'referent'     = @($symbol['r21-bare-there'], $symbol['r21-that-card'], $symbol['r21-title-deictic'], $symbol['r21-named-link'],
            $symbol['r21-previous-sentence-link'], $symbol['r21-named-in-words'], $symbol['r21-code-and-quote'])
        'own'          = @($symbol['i7-own-source'], $symbol['i7-illustrates'], $symbol['i7-other-book'], $symbol['i7-filed-in-book'],
            $symbol['i7-unknown-source'], $symbol['i7-corresponds'])
        'i6-one'       = @($symbol['i6-pair-a'])
        'i6-both'      = @($symbol['i6-pair-a'], $symbol['i6-pair-b'])
        'i6-unrelated' = @($symbol['i6-unrelated'])
    }
    # The runs are reads: the archive revision of the finished fixture is what they must leave.
    $revisionBefore = @(zt status | Where-Object { $_ -match '^revision: ' })
    $script:archiveLines = @{}
    foreach ($run in $runCards.Keys) { $script:archiveLines[$run] = @(Invoke-ZtCheck -Cards ($runCards[$run] -join ',') -Rules $script:runRules[$run]) }
    Assert ($revisionBefore[0] -eq (@(zt status | Where-Object { $_ -match '^revision: ' }))[0]) "the archive runs changed no Card (revision $($revisionBefore[0]))"
    # The existing hub whose title copies its anchor's: created after the pair runs, because a
    # Hub Card is no I6 subject but every title of the Workspace is I6's context.
    $symbol['i6-hub'] = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = $symbol['i6-pair-a']; kind = 'side'; type = 'hub'; level = 'line'; title = '剩余控制权决定契约空白处'; body = "## 主线`n- [[$($symbol['i6-pair-a'])]]：所有者决定契约的空白处。" } }))[0]
    $revisionAfterHub = @(zt status | Where-Object { $_ -match '^revision: ' })
    $script:archiveLines['i6-hub'] = @(Invoke-ZtCheck -Cards $symbol['i6-hub'] -Rules 'I6')
    $script:archiveLines['i6-hub-context'] = @(Invoke-ZtCheck -Cards $symbol['i6-pair-a'] -Rules 'I6')
    foreach ($example in $examples) {
        if ($example.run -like 'i6-json-*') { continue }
        $observed = @($script:archiveLines[$example.run] | Where-Object { $_ -match "^$([regex]::Escape($symbol[$example.card])) $($example.rule) " })
        Record-Example $example $observed 'cards'
    }
    # S25-AC03: advice is printed and counted as no fault; I7's own-source line is the hard one.
    $r20Faults = [int](($script:archiveLines['r20'][-1] -split ': ')[1])
    $r21Faults = [int](($script:archiveLines['referent'][-1] -split ': ')[1])
    $i6Faults = [int](($script:archiveLines['i6-both'][-1] -split ': ')[1])
    $i7Faults = [int](($script:archiveLines['own'][-1] -split ': ')[1])
    Assert ($r20Faults -eq 0 -and $r21Faults -eq 0 -and $i6Faults -eq 0) "R20, R21 and I6 print advice and count no fault (R20 $r20Faults, R21 $r21Faults, I6 $i6Faults)"
    Assert ($i7Faults -eq 1) "I7's own-source dependency is a hard layer fault ($i7Faults); its unknown-source line is advice"
    Assert (@($script:archiveLines['own'] | Where-Object { $_ -match ' I7 \(advisory\) source evidence unknown' }).Count -eq 1) 'missing source evidence is reported as unknown, never as a pass'

    # --- the offline runs: the --json bundle an audit writes --------------------------------
    # The human Workspace is read-only for agents, so the cross-Workspace, the incomplete
    # context and the unnamed-Workspace cases run through the bundle contract the audit's
    # zt-check call uses.
    $jsonRuns = [ordered]@{
        'i6-json-workspace'  = [ordered]@{
            bundle = [ordered]@{
                cards = @([ordered]@{ address = '12/9|a'; category = 'idea'; parent = '12/9/0'; tree = '12/9'; workspace = 'agent'; title = '剩余控制权决定契约空白处'; body = '所有者拒绝合作的能力决定契约没有写明的部分。' })
                context = [ordered]@{ complete = $true; titles = @(
                        [ordered]@{ address = '12/9|a'; title = '剩余控制权决定契约空白处'; workspace = 'agent'; tree = '12/9' },
                        [ordered]@{ address = 'h/1|a'; title = '剩余控制权决定契约空白处'; workspace = 'human'; tree = 'h/1' },
                        [ordered]@{ address = '12/9|b'; title = '所有权的配置影响投资激励'; workspace = 'agent'; tree = '12/9' })
                }
            }
        }
        'i6-json-incomplete' = [ordered]@{
            bundle = [ordered]@{
                cards = @([ordered]@{ address = '12/9|b'; category = 'idea'; parent = '12/9/0'; tree = '12/9'; workspace = 'agent'; title = '剩余控制权决定契约的空白处'; body = '同一机制换了措辞，写在另一条主题树上。' })
                context = [ordered]@{ complete = $false; titles = @(
                        [ordered]@{ address = '12/9|c'; title = '剩余控制权决定契约空白处'; workspace = 'agent'; tree = '12/9' })
                }
            }
        }
        'i6-json-scope'      = [ordered]@{
            bundle = [ordered]@{
                cards = @([ordered]@{ address = '12/9|c'; category = 'idea'; parent = '12/9/0'; tree = '12/9'; title = '剩余控制权决定契约空白处'; body = '所有者拒绝合作的能力决定契约没有写明的部分。' })
                context = [ordered]@{ complete = $true; titles = @(
                        [ordered]@{ address = '12/9|d'; title = '剩余控制权决定契约空白处'; workspace = 'agent'; tree = '12/9' })
                }
            }
        }
    }
    $script:jsonLines = @{}
    foreach ($run in $jsonRuns.Keys) {
        $path = Join-Path $work "$run.json"
        Write-Json $path $jsonRuns[$run].bundle
        # --json prints every rule's lines and counts them; the family selects its own rule.
        $script:jsonLines[$run] = @(Invoke-ZtCheck -Json $path | Where-Object { $_ -match ' I6 ' -or $_ -match '^faults: ' })
    }
    foreach ($example in $examples) {
        if ($example.run -notlike 'i6-json-*') { continue }
        $observed = @($script:jsonLines[$example.run] | Where-Object { $_ -match "^$([regex]::Escape($example.card)) I6 " })
        Record-Example $example $observed 'json'
    }
    Assert (@($script:jsonLines['i6-json-incomplete'] | Where-Object { $_ -match ' I6 \(advisory\) Workspace title context incomplete: 1 known title compared, others unknown$' }).Count -eq 1) 'an incomplete title context is reported as unknown, not as faults: 0 for a Workspace-wide check'
    Assert (@($script:jsonLines['i6-json-scope'] | Where-Object { $_ -match ' I6 \(advisory\) Workspace scope unknown' }).Count -eq 1) 'an entry that names no Workspace is not compared across the boundary'

    # --- the concept families: the reviewed concept examples through the concept review -----
    $concept = Invoke-ConceptReview $key $conceptFixture
    foreach ($finding in $concept.expectations) {
        if (-not $finding.reproduced) { $script:failed++ }
        $mark = if ($finding.reproduced) { 'ok' } else { 'FAIL' }
        Write-Output ("{0}: concept {1}: {2} (annotation {3})" -f $mark, $finding.kind, $finding.id, $finding.annotation)
        if (-not $finding.reproduced) { Write-Output ("      observed: " + (@($finding.detail) -join ' | ')) }
    }
    Assert (@($concept.expectations | Where-Object { -not $_.reproduced }).Count -eq 0) 'the reviewed concept examples reproduce: two learning senses stay apart, the shared formula under different scope stays a difference, the identical claim is one statement'
    Assert ($concept.judge_exercised -eq $false) 'the semantic judge is not exercised here (canned answers); its accuracy remains the pilot''s evidence (S26)'

    # --- read-only: no Card changed and the Link index still matches -------------------------
    $revisionAfter = @(zt status | Where-Object { $_ -match '^revision: ' })
    Assert ($revisionAfterHub.Count -eq 1 -and $revisionAfter.Count -eq 1 -and $revisionAfterHub[0] -eq $revisionAfter[0]) "the reviewed runs changed no Card (revision $($revisionAfterHub[0]))"
    zt status --check-links | Out-Null
    Assert ($LASTEXITCODE -eq 0) 'the Link index still matches every stored Reverse section'

    # --- the report ------------------------------------------------------------------------
    Write-CalibrationReport $concept
} finally {
    Close-ZtServer
    zt down 2>$null | Out-Null
    if (-not $env:ZT_KEEP_SCRATCH) { Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue }
}
if ($failed) { Write-Output "$failed assertion(s) failed"; exit 1 }
Write-Output 'calibration: all assertions passed'
