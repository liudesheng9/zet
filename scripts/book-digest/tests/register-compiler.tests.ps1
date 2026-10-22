#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\register-compiler.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# #93 S10 / issue #101: plan a small concept register from the agent book term tables, the
# model-head names and a reviewed concept report; publish it as the one register Hub Card of the
# 索引 Topic; read it back through get_register; publish again with no change. A real scratch
# archive; the live archive is not touched. -Zt runs a zt build that is not installed yet, and
# -Digest another zt-digest build; without them the newest built binary is used when it is newer
# than this slice's source, else the installed one.
param([string]$Zt = '', [string]$Digest = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$scratch = Join-Path ([IO.Path]::GetTempPath()) "register-compiler-test-$PID"
$archive = Join-Path $scratch 'archive'
$work = Join-Path $scratch 'work'
New-Item -ItemType Directory -Force $archive, $work, (Join-Path $work 'out') | Out-Null
# The slice that this test needs: a `get_register` tool in zt and the `register` step in zt-digest.
function Newest($stamp, [string[]]$candidates) {
    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath $candidate) -and (Get-Item -LiteralPath $candidate).LastWriteTimeUtc -gt $stamp) { return $candidate }
    }
    ''
}
if (-not $Zt) {
    $Zt = Newest (Get-Item (Join-Path $repo 'src\entry.rs')).LastWriteTimeUtc @(
        (Join-Path $repo 'target\debug\zt.exe'), (Join-Path $repo 'target\release\zt.exe'))
}
if (-not $Zt) { $Zt = (Get-Command zt -ErrorAction Stop).Source }
if (-not $Digest) { $Digest = $env:ZT_DIGEST }
if (-not $Digest) {
    $Digest = Newest (Get-Item (Join-Path $repo 'tools\zt-digest\src\register.rs')).LastWriteTimeUtc @(
        (Join-Path $repo 'tools\zt-digest\target\debug\zt-digest.exe'), 'D:\zt-digest-target\release\zt-digest.exe')
}
if (-not $Digest) { $Digest = (Get-Command zt-digest -ErrorAction Stop).Source }
$bin = Join-Path $scratch 'bin'
New-Item -ItemType Directory -Force $bin | Out-Null
Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
$env:PATH = "$bin;$env:PATH"
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$wrapper = Join-Path $PSScriptRoot '..\register-compiler.ps1' | Resolve-Path | ForEach-Object Path
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Run([string]$step, [string[]]$more = @()) {
    $lines = @(pwsh -NoProfile -File $wrapper -Step $step -Work $work @more 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "$step failed: $($lines -join "`n")" }
    $lines
}
function Last($lines) { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable -DateKind String }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $($lines -join ' | ')" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Terms($rows) {
    $map = [ordered]@{}
    foreach ($row in @($rows)) {
        $map[$row.term] = @($row.targets | ForEach-Object { if ($_ -is [string]) { $_ } elseif ($_.exists) { $_.address } })
    }
    $map
}
function Same([string[]]$a, [string[]]$b) { @(Compare-Object @($a) @($b)).Count -eq 0 }
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    # One agent book: its first anchor and the term table filed as its Side successor. The table
    # column 不用 is the book's own naming evidence, so each rejected word becomes a second
    # accepted spelling of the same concept.
    $book = Write-Cards @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{Rev2026, title={概念复核}, author={Owner, A.}, year={2026}}'; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Rev2026'; kind = 'direct'; title = '第 1 组：均衡与技能'; body = '§1 模型与术语' } }
    )
    $tableBody = @'
本卡是《概念复核》的术语表。本书的卡片用左列的术语，不用右列的词。

术语表

| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 多重均衡 | 均衡的多重性 | 策略互补时可能有多个均衡 | §2.1 |
| 学习 | 习得 | 个体通过重复练习积累技能 | §3.1 |
| 无着落 | 悬空词 | 本书没有可以指向的卡 | §9.9 |
'@
    $table = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{ at = $book[1]; kind = 'side'; title = '术语表：概念复核'; body = $tableBody } }))[0]
    # A statement the review accepts as the entry of 多重均衡, without naming the term in its title.
    $statement = (Write-Cards @(
            @{ tool = 'create_topic'; arguments = @{ title = '策略互补'; body = '互补策略怎样带来多个均衡？' } },
            @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '策略互补时可能有多个均衡'; body = '反应曲线相交不止一次。' } }
        ))[1]
    # 学习: one statement and one whole model.
    $learn = Write-Cards @(
        @{ tool = 'create_topic'; arguments = @{ title = '技能积累'; body = '技能从哪里来？' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '学习是个体行为'; body = '重复练习改变个体技能。' } },
        @{ tool = 'create_topic'; arguments = @{ title = '学习模型'; body = '学习怎样进入生产？' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@2'; type = 'model'; title = '学习模型'; body = '问题：累计产量怎样降低成本？' } }
    )
    # 学习's Topic root is a different sense: the review says so, so it is never an entry.
    $otherSense = (Write-Cards @(@{ tool = 'create_topic'; arguments = @{ title = '学习'; body = '产业整体的生产率怎样提高？' } }))[0]
    # 多重均衡: a Topic claim and a whole model, so the term has three targets with the reviewed one.
    $multi = Write-Cards @(
        @{ tool = 'create_topic'; arguments = @{ title = '多重均衡'; body = '什么时候会有多个均衡？' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; type = 'model'; title = '多重均衡模型'; body = '问题：协调博弈的均衡集是什么？' } }
    )
    $quote = (Write-Cards @(
            @{ tool = 'create_topic'; arguments = @{ title = '工资报价'; body = '厂商怎样报价？' } },
            @{ tool = 'create_successor'; arguments = @{ at = '@@0'; type = 'model'; title = '工资报价模型'; body = '问题：厂商怎样报价？' } }
        ))[1]
    # The reviewed concept report: read-only evidence with its own id, bound to this archive.
    $facts = @(Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @(); view = 'text' } }))[0]
    $reportId = 'r-' + ('a' * 64)
    $reviewPath = Join-Path $scratch 'concept-review.json'
    $review = [ordered]@{
        schema_version = 1
        report_id      = $reportId
        created_at     = '2026-10-19T00:00:00.0000000Z'
        book           = [ordered]@{ citation_key = 'Rev2026'; title = '概念复核' }
        archive        = $facts.archive_identity
        status         = 'complete'
        completeness   = [ordered]@{ complete = $true }
        evidence_path  = 'e-1'
        proposals      = @(
            [ordered]@{ id = 'p001'; title = '多重均衡' },
            [ordered]@{ id = 'p002'; title = '学习' }
        )
        findings       = @(
            [ordered]@{ id = 'c001'; kind = 'topic-concept'; proposal = 'p001'; candidate = $multi[0]; entry = $statement; verdict = 'same'; scope = '策略互补时'; reason = '同一个问题。' },
            [ordered]@{ id = 'c002'; kind = 'topic-concept'; proposal = 'p002'; candidate = $otherSense; entry = $null; verdict = 'different'; reason = '个体行为与产业生产率不同。' }
        )
    }
    [IO.File]::WriteAllText($reviewPath, ($review | ConvertTo-Json -Depth 10), $utf8)
    # The accepted pinyin readings the drafting job supplies (#104, D183): one polyphonic term read
    # as the accepted sense, one uncertain reading, one malformed initial, and terms the file never
    # mentions. This step checks their shape only; it never transliterates (D184).
    $readingsPath = Join-Path $work 'out\register-readings.json'
    $readings = [ordered]@{
        schema_version = 1
        readings       = [ordered]@{
            '学习'     = [ordered]@{ reading = 'xué xí'; initial = 'x' }
            '多重均衡' = [ordered]@{ reading = 'duō chóng jūn héng'; initial = 'D' }
            '习得'     = [ordered]@{ reading = 'xí dé'; initial = $null; uncertain = $true }
            '工资报价' = [ordered]@{ reading = 'gōng zī bào jià'; initial = 'gz' }
        }
    }
    [IO.File]::WriteAllText($readingsPath, ($readings | ConvertTo-Json -Depth 6), $utf8)

    # plan: term tables plus model heads, reviewed meanings, unresolved rows.
    $plan = Last (Run 'plan' @('-Review', $reviewPath))
    $staged = Get-Content -Raw -LiteralPath (Join-Path $work 'out\register-plan.json') | ConvertFrom-Json -AsHashtable -DateKind String
    Assert ($plan.review.report_id -eq $reportId) 'plan: records the reviewed report id it read from'
    Assert (@($plan.review.accepted_spellings) -contains '多重均衡' -and @($plan.review.distinct_senses) -contains '学习') 'plan: the review combines the accepted spelling and keeps the distinct sense'
    Assert (@($staged.unresolved).Count -eq 1 -and $staged.unresolved[0].term -eq '无着落') 'plan: a term with no defensible target is unresolved with its source row'
    Assert (@($staged.unresolved[0].spellings).Count -eq 2) 'plan: one unresolved finding names both accepted spellings of its row'
    Assert ($staged.unresolved[0].sources[0].term_table -eq $table) 'plan: the unresolved row names its term table'
    # The accepted readings are recorded, and what the file does not settle stays unresolved.
    Assert (@($staged.readings.accepted).Count -eq 2) "plan: the accepted readings are recorded ($(@($staged.readings.accepted | ForEach-Object { $_.term }) -join ', '))"
    Assert (@($staged.readings.accepted | Where-Object { $_.term -eq '多重均衡' -and $_.initial -eq 'd' }).Count -eq 1) 'plan: a polyphonic reading is accepted as supplied, with its lower-cased initial'
    Assert (@($staged.readings.unresolved | Where-Object { $_.term -eq '习得' -and $_.reason -eq 'uncertain' }).Count -eq 1) 'plan: an uncertain reading is unresolved, not classified'
    Assert (@($staged.faults | Where-Object { $_.fault -eq 'reading_initial' -and $_.term -eq '工资报价' }).Count -eq 1) 'plan: a malformed initial is a reading fault'
    Assert (@($staged.readings.unresolved | Where-Object { $_.term -eq '学习模型' -and $_.reason -eq 'missing' }).Count -eq 1) 'plan: a term the readings file never mentions is missing its reading'
    Assert ($staged.terms_on_root -eq $true -and @($staged.parts).Count -eq 0) 'plan: a register of seven rows stays on the root'
    $terms = Terms $staged.terms
    Assert ($terms.Keys.Count -eq 7) "plan: three book terms with their accepted spellings and three model heads ($($terms.Keys -join ', '))"
    Assert (@($terms['多重均衡']).Count -eq 3 -and @($terms['多重均衡'])[0] -eq $statement) 'plan: three targets, the reviewed statement first'
    Assert ((Same $terms['多重均衡'] $terms['均衡的多重性']) -and @($terms['均衡的多重性']).Count -eq 3) 'plan: a rejected book word points at the same three entries'
    Assert (@($terms['学习']).Count -eq 2 -and @($terms['学习']) -notcontains $otherSense) 'plan: the reviewed distinct sense is never an entry'
    Assert ((Same $terms['学习'] $terms['习得']) -and @($terms['习得']).Count -eq 2) 'plan: the second spelling of 学习 points at the same two entries'
    Assert (@($terms['工资报价模型']).Count -eq 1 -and $terms['工资报价模型'][0] -eq $quote) 'plan: a model-head name is a term that points at its Model Card'

    # apply: the register is created under the Topic 索引, then read back and verified.
    $unrelated = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $learn[0] } }))[0]
    $authored = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $statement } }))[0]
    $titlesBefore = @(Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{ workspace = 'agent' } }))[0].count
    $applied = Last (Run 'apply')
    Assert ($applied.register -and $applied.created -eq 2) 'apply: creates the Topic 索引 and its register Hub Card'
    Assert ($applied.verified -eq $true -and $applied.complete -eq $true -and $applied.entries -eq 7) 'apply: verifies every published term and target'
    Assert ($applied.review.report_id -eq $reportId) 'apply: reports the reviewed report it published from'
    $published = @(Invoke-Zt @(@{ tool = 'get_register'; arguments = @{ workspace = 'agent' } }))[0]
    Assert ($published.register -eq $applied.register -and $published.register_missing -eq $false) 'get_register: reads the published register'
    $entries = Terms $published.entries
    Assert ($entries.Keys.Count -eq 7 -and (Same @($terms.Keys) @($entries.Keys))) 'get_register: the published terms are the staged terms'
    Assert (Same $entries['多重均衡'] $terms['多重均衡']) 'get_register: every target of every term is readable'
    Assert (Same $entries['均衡的多重性'] $terms['多重均衡']) 'get_register: both accepted spellings read the same three entries'
    Assert (@($entries['学习']) -notcontains $otherSense) 'get_register: the distinct sense is absent'
    $hub = @(Invoke-Zt @(@{ tool = 'get_hub'; arguments = @{ at = $applied.register } }))[0]
    Assert ($hub.level -eq 'register' -and $hub.entry_total -eq 7 -and @($hub.faults).Count -eq 0) "get_hub: the register report carries the entries and no fault ($($hub.faults -join ', '))"
    $card = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $applied.register } }))[0]
    Assert ($card.body -match '(?m)^## 词条$' -and $card.body -match [regex]::Escape("- 多重均衡：[[$statement]]")) 'the register body is one 词条 row per accepted term'
    $after = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $learn[0] } }))[0]
    Assert ($after.text -eq $unrelated.text) 'apply: a Card the register does not link is unchanged'
    $linked = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $statement } }))[0]
    Assert ($linked.title -eq $authored.title -and $linked.body -eq $authored.body) 'apply: a linked Card keeps its authored title and body (only its generated Reverse section follows the new Link)'

    # a second apply over the same plan writes nothing and keeps the plan id.
    $again = Last (Run 'apply')
    Assert ($again.created -eq 0 -and $again.changed -eq $false) 'apply again: no new Card and no changed body'
    Assert ($again.plan_id -eq $applied.plan_id -and $again.register -eq $applied.register) 'apply again: the same plan id and the same register address'
    $titlesAfter = @(Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{ workspace = 'agent' } }))[0].count
    Assert ($titlesAfter -eq $titlesBefore + 2) 'apply again: the archive holds only the two Cards the first run created'
    $check = Last (Run 'check')
    Assert (@($check.missing).Count -eq 0 -and @($check.diverged).Count -eq 0 -and @($check.extra).Count -eq 0 -and $check.complete -eq $true) 'check: the published register matches the staged plan'

    # a second plan over the published register stages the same acceptance and the same id.
    $replan = Last (Run 'plan' @('-Review', $reviewPath))
    Assert ($replan.plan_id -eq $plan.plan_id) 'plan again: the same accepted plan has the same content id'
    $restaged = Get-Content -Raw -LiteralPath (Join-Path $work 'out\register-plan.json') | ConvertFrom-Json -AsHashtable -DateKind String
    Assert (@($restaged.unresolved).Count -eq 1) 'plan again: the unresolved row is still reported'
    Assert ((Same @($restaged.terms.term) @($staged.terms.term))) 'plan again: the published register is not a new candidate source'
}
finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
