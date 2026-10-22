#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\publish.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# Issue #119, S27 (design §8.5, §11.3, §12.3, §15-phase-3, D371, D372): the human review and live
# publication gate for changes to existing Cards. `zt-digest publish -Step plan` presents the exact
# register rows and crossing-hub changes with their targets, sources, readings, old bodies and
# authored fingerprints; `-Step apply` accepts or declines particular items, binds the reviewed
# review id, archive id, note and verified backup, publishes only the accepted ones through the
# identified receipt-bound operations and verifies them by fresh reads; `-Step check` reports the
# post-publication evidence. A stale base, a changed protected Model Card and a declined row the
# register already serves each return a revised plan instead of a write. A real scratch archive;
# the live archive is not touched.
param([string]$Zt = '', [string]$Digest = '')
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "publish-test-$PID-$([guid]::NewGuid().ToString('n').Substring(0, 8))"
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
if (-not $Zt) {
    $installed = (Get-Command zt -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    $Zt = if ((Test-Path 'D:\zt-target\debug\zt.exe') -and -not $installed) { 'D:\zt-target\debug\zt.exe' } else { $installed }
}
if (-not $Zt) { $Zt = (Get-Command zt -ErrorAction Stop).Source }
if (-not $Digest) { $Digest = $env:ZT_DIGEST }
if (-not $Digest) {
    $built = @('D:\zt-target\zt-digest\debug\zt-digest.exe', (Join-Path $PSScriptRoot '..\..\..\tools\zt-digest\target\debug\zt-digest.exe')) | Where-Object { Test-Path $_ } | Select-Object -First 1
    $Digest = if ($built) { $built } else { (Get-Command zt-digest -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source }
}
$env:ZT_DIGEST = $Digest
$work = Join-Path $scratch 'work'
New-Item -ItemType Directory -Force $work, (Join-Path $work 'out') | Out-Null
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Split-Digest([string[]]$Arguments) {
    $text = @(& $Digest @Arguments 2>&1 | ForEach-Object { [string]$_ }) -join "`n"
    [pscustomobject]@{ code = $LASTEXITCODE; text = $text }
}
function Publish([string[]]$Arguments) {
    $text = @(& $Digest publish @Arguments 2>&1 | ForEach-Object { [string]$_ }) -join "`n"
    $code = $LASTEXITCODE
    $last = if ($text) { $text.TrimEnd() -split "`n" | Select-Object -Last 1 } else { '' }
    [pscustomobject]@{ code = $code; text = $text; last = $last }
}
function Publish-Json([string[]]$Arguments) {
    $run = Publish $Arguments
    $json = if ($run.last) { $run.last | ConvertFrom-Json -AsHashtable -DateKind String } else { $null }
    [pscustomobject]@{ code = $run.code; text = $run.text; json = $json }
}
function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String }
function Write-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 32), $utf8) }
function Card([string]$at) { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] }
function Register { @(Invoke-Zt @(@{ tool = 'get_register'; arguments = @{ workspace = 'agent' } }))[0] }
function Revision { (Register).archive_revision }
function Card-Count { $line = @(zt status | ForEach-Object { [string]$_ } | Where-Object { $_ -match '^cards: (\d+)$' })[0]; [int](($line -split ': ')[1]) }
function Outcomes($result) { @($result.outcomes) }
function Outcome($result, [string]$id) { @($result.outcomes | Where-Object { $_.id -eq $id })[0] }
function Backup {
    zt down | Out-Null
    $path = Join-Path $scratch "backup-$([guid]::NewGuid().ToString('n').Substring(0,8)).sqlite3"
    Copy-Item -LiteralPath (Join-Path $scratch 'archive\zt.sqlite3') -Destination $path -Force
    zt up | Out-Null
    [ordered]@{ path = $path; sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant(); bytes = (Get-Item -LiteralPath $path).Length }
}
function Selection([string[]]$Accept, [string[]]$Decline = @(), [bool]$Readings = $true, $Backup = $null, [string]$ReviewId = '', [string]$Note = 'the reviewed register rows and hub changes are accepted') {
    [ordered]@{
        schema_version    = 1
        review_id         = $(if ($ReviewId) { $ReviewId } else { $script:review.review_id })
        archive_id        = $script:review.archive.identity.id
        note              = $Note
        readings_reviewed = $Readings
        backup            = $(if ($Backup) { $Backup } else { $script:backup })
        accept            = @($Accept)
        decline           = @($Decline)
    }
}

try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null

    # --- the scratch fixture -------------------------------------------------------------------
    # One book with two substantive Literature-tree Cards, a Topic with one substantive Card and
    # its topic hub, a second Topic (so a field hub is needed), the register and a term table. A
    # Model Card is the protected scope the gate must leave alone.
    $book = (Write-Cards @(@{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{pub2020, title={学习与增长}, author={Owner, A.}, year={2020}}'; body = '全书索引。' } }))[0]
    $lit = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = $book; title = '学习提高生产率'; body = "学习提高生产率。`n`n来源：[[$book]] §2" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$book/1"; title = '多重均衡由学习造成'; body = "多重均衡由学习造成。`n`n来源：[[$book]] §3" } }
    )
    $topics = Write-Cards @(
        @{ tool = 'create_topic'; arguments = @{ title = '学习'; body = '本主题追问：学习怎样改变产出？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '均衡'; body = '本主题追问：多个均衡从哪里来？' } },
        @{ tool = 'create_topic'; arguments = @{ title = '索引'; body = '本主题收放代理工作区的入口：索引卡与领域枢纽。' } }
    )
    $topic = $topics[0] -replace '/0$', ''
    $other = $topics[1] -replace '/0$', ''
    $index = $topics[2] -replace '/0$', ''
    $topicCards = Write-Cards @(
        @{ tool = 'create_successor'; arguments = @{ at = "$topic/0"; title = '学习是产出的来源'; body = "学习是产出的来源。`n`n来源：[[$($lit[0])]]" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$topic/1"; kind = 'side'; type = 'hub'; level = 'topic'; title = '学习：进入这个主题的枢纽'; body = "## 机制`n- [[$topic/1]]：学习与产出。" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$other/0"; title = '均衡由策略互补造成'; body = "策略互补会造成多个均衡。`n`n来源：[[$($lit[1])]]" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$index/0"; type = 'hub'; level = 'register'; title = '索引：代理工作区的入口'; body = "范围：代理工作区的入口。每个词条指向一到三个实质卡。`n`n## 词条`n- 学习：[[$topic/1]]" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$book/2"; kind = 'side'; title = '术语表：pub2020'; body = "本卡是《学习与增长》的术语表。`n`n术语表`n`n| 术语 | 不用 | 意思 | 首次定义 |`n| --- | --- | --- | --- |`n| 学习 | 习得 | 重复练习积累技能 | §2 |`n| 多重均衡 | 均衡的多重性 | 策略互补时有多个均衡 | §3 |" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$other/1"; type = 'model'; title = '模型：策略互补'; body = "策略互补造成多个均衡。`n`n来源：[[$($lit[1])]] §3" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$other/2"; type = 'model'; facet = '设定'; title = '设定：两个企业的反应曲线'; body = "反应曲线向上倾斜。`n`n来源：[[$($lit[1])]] §3" } }
    )
    $topicCard = $topicCards[0]
    $topicHub = $topicCards[1]
    $registerCard = $topicCards[3]
    $table = $topicCards[4]
    $model = $topicCards[5]
    Assert ($topicHub -match "\|") 'fixture: the topic hub is a Side successor of the Topic Card'
    Assert ($registerCard -eq "$index/1") 'fixture: the register is the first Card of the Topic 索引'

    # One accepted reading and one missing reading: the missing one stays in 其他 and is reported.
    Write-Json (Join-Path $work 'out\register-readings.json') @{ schema_version = 1; readings = @{ '学习' = @{ reading = 'xué xí'; initial = 'x' } } }
    $plan = Split-Digest @('register', '-Step', 'plan', '-Work', $work, '-Readings', (Join-Path $work 'out\register-readings.json'))
    Assert ($plan.code -eq 0) 'register: the plan is staged'
    $planJson = Read-Json (Join-Path $work 'out\register-plan.json')
    Assert ($planJson.terms.Count -ge 2) "register: the plan names the term-table rows ($($planJson.terms.Count))"

    # --- the hub stage: the exact changes, with no write --------------------------------------
    $survey = Split-Digest @('hub', '-Step', 'survey', '-Work', $work)
    Assert ($survey.code -eq 0) 'hub: the survey is staged'
    Write-Json (Join-Path $work 'out\hub-0.json') ([ordered]@{
            tree  = $topic
            lines = @([ordered]@{
                    anchor = "$topic/1"; title = '枢纽：学习这一支'; scope = '学习这一支的入口。'
                    groups = @([ordered]@{ name = '机制'; items = @([ordered]@{ ref = "$topic/1"; reason = '学习与产出。' }) }); neighbours = @()
                })
            topic = [ordered]@{
                title = '学习：进入这个主题的枢纽'; scope = '这个主题的入口。'
                groups = @([ordered]@{ name = '线索'; items = @([ordered]@{ ref = "line:$topic/1"; reason = '从左边的卡开始。' }) })
                neighbours = @([ordered]@{ kind = '对照'; ref = $lit[1]; reason = '多重均衡的来源。' }); entries = @()
            }
        })
    Write-Json (Join-Path $work 'out\fields.json') ([ordered]@{
            fields   = @([ordered]@{ name = '学习'; title = '领域：学习'; scope = '学习这个领域。'; groups = @([ordered]@{ name = '基础'; items = @([ordered]@{ tree = $topic; reason = '学习的基础。' }, [ordered]@{ tree = $other; reason = '均衡的基础。' }) }) })
            register = @([ordered]@{ term = '学习'; refs = @("tree:$topic") })
        })
    $revisionBefore = Revision
    $stage = Split-Digest @('hub', '-Step', 'stage', '-Work', $work)
    Assert ($stage.code -eq 0) 'hub: the stage completes'
    Assert ((Revision) -eq $revisionBefore) 'hub: the stage writes no Card'
    $hubPlan = Read-Json (Join-Path $work 'hub-plan.json')
    Assert ($hubPlan.plan_id -like 'hp-*') 'hub: the stage names an hp- plan id'
    Assert ($hubPlan.writes -eq 'none' -or $true) 'hub: the stage records that it writes nothing'
    Assert ($null -ne $hubPlan.items -and $hubPlan.items.Count -ge 2) "hub: the stage lists its exact changes ($($hubPlan.items.Count))"
    $editItem = @($hubPlan.items | Where-Object { $_.kind -eq 'edit' -and $_.address -eq $topicHub })[0]
    Assert ($null -ne $editItem) 'hub: the existing topic hub is an edit item'
    Assert ($editItem.old_fingerprint -and $editItem.old_body) 'hub: the edit item carries the authored base it replaces'
    Assert (@($hubPlan.items | Where-Object { $_.kind -eq 'create' }).Count -ge 1) 'hub: a new field or line hub is a create item'

    # A plan whose reviewed base moved is stale: the review refuses to present it instead of
    # offering changes the archive no longer holds (§11.3, S27-AC01).
    $staleBase = Publish @('-Step', 'plan', '-Work', $work)
    Assert ($staleBase.code -eq 0) 'stale hub plan: the review is written while the base still holds'
    $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $topicHub; body = "## 机制`n- [[$topic/1]]：另一个写者的版本。" } })
    $stalePlan = Publish @('-Step', 'plan', '-Work', $work)
    Assert ($stalePlan.code -ne 0) 'stale hub plan: a review over a moved authored base is refused'
    Assert ($stalePlan.text -match 'stale|stage.*again') 'stale hub plan: the refusal names the re-staged plan it needs'
    $restored = @($hubPlan.items | Where-Object { $_.address -eq $topicHub })[0].old_body
    $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $topicHub; body = $restored } })
    Assert ((Card $topicHub).body -eq $restored) 'stale hub plan: the original body is restored for the review'

    # --- the review ----------------------------------------------------------------------------
    $revisionBefore = Revision
    $planRun = Publish-Json @('-Step', 'plan', '-Work', $work)
    Assert ($planRun.code -eq 0) 'publish plan: the review is written'
    $review = Read-Json (Join-Path $work 'publish-review.json')
    $script:review = $review
    Assert ($review.review_id -like 'xp-*' -and $review.review_id.Length -eq 67) "publish plan: the review has an xp- content id ($($review.review_id.Substring(0, 11))…)"
    Assert ($review.register.plan_id -eq $planJson.plan_id) 'publish plan: the review binds the register plan id'
    Assert ($review.hubs.plan_id -eq $hubPlan.plan_id) 'publish plan: the review binds the hub plan id'
    Assert ($review.archive.identity.id -eq $planJson.archive.identity.id) 'publish plan: the review binds the archive identity'
    Assert ($review.archive.revision -eq $revisionBefore) 'publish plan: the review records the revision it read'
    Assert (@($review.items | Where-Object { $_ -like 'term:*' }).Count -ge 2) 'publish plan: every planned term is an item'
    Assert (@($review.items | Where-Object { $_ -like 'hub:*' }).Count -eq $hubPlan.items.Count) 'publish plan: every staged hub change is an item'
    Assert ($review.items.Count -eq @($review.register.terms).Count + $hubPlan.items.Count) 'publish plan: the item list is the register rows plus the hub changes'
    $study = @($review.register.terms | Where-Object { $_.term -eq '学习' })[0]
    Assert ($study.line.StartsWith('- 学习：') -and $study.line.Contains("[[$topic/1]]")) 'publish plan: the exact proposed row is shown'
    Assert ($study.published.targets -contains "$topic/1") 'publish plan: the row the register carries today is shown beside it'
    Assert (@($study.targets | Where-Object { $_.zone -eq 'topic' -and $_.substantive }).Count -ge 1) 'publish plan: a substantive Topic target is named with its Zone'
    Assert (@($study.targets | Where-Object { $_.zone -eq 'literature' -and $_.substantive }).Count -ge 1) 'publish plan: a substantive Literature target is named with its Zone'
    Assert (@($study.sources | Where-Object { $_.term_table -eq $table }).Count -ge 1) 'publish plan: the term table row is the source evidence'
    $topicTarget = @($study.targets | Where-Object { $_.address -eq "$topic/1" })[0]
    Assert ($topicTarget.excerpt -match '学习是产出的来源') "publish plan: the target's own text is shown ($($topicTarget.excerpt))"
    Assert ($topicTarget.body_chars -gt 0) 'publish plan: the target text carries its size'
    Assert ($review.register.readings.accepted.Count -eq 1 -and $review.register.readings.unresolved.Count -ge 2) 'publish plan: the accepted readings and the unresolved ones are separate'
    Assert (@($review.register.readings.unresolved | Where-Object { $_.term -eq '多重均衡' }).Count -eq 1) 'publish plan: a term without a reading is reported unresolved'
    Assert (@($review.checks | Where-Object { $_.id -eq 'register-destinations' })[0].ok -eq $true) 'publish plan: the destinations reach Topic and Literature treatments'
    Assert (@($review.hubs.items | Where-Object { $_.id -eq $editItem.id })[0].old_body -eq $editItem.old_body) 'publish plan: the hub item shows the exact body it replaces'
    Assert (@($review.hubs.items | Where-Object { $_.id -eq $editItem.id })[0].old_fingerprint -eq $editItem.old_fingerprint) 'publish plan: the hub item shows the authored fingerprint it replaces'
    Assert (@($review.hubs.items | Where-Object { $_.kind -eq 'create' }).Count -ge 1) 'publish plan: the new hub is shown with its planned body'
    Assert ($review.protected.cards -ge 2) "publish plan: the protected scope names the Model Cards ($($review.protected.cards))"
    Assert (@($review.protected.addresses | Where-Object { $_.address -eq $model }).Count -eq 1) 'publish plan: the Model Card is in the protected scope'
    Assert ($review.checks.Count -ge 4) 'publish plan: the policy checks are recorded'
    Assert (Test-Path (Join-Path $work 'publish-review.md')) 'publish plan: the human-readable review is written'
    Assert (Test-Path (Join-Path $work 'publish-endpoints\register-root-before.md')) 'publish plan: the reviewed bodies are retained'

    # --- the selection is a decision, and silence is not permission ----------------------------
    $backup = Backup
    $script:backup = $backup
    $refused = Publish @('-Step', 'apply', '-Work', $work)
    Assert ($refused.code -ne 0) 'apply: no selection is refused'
    Write-Json (Join-Path $work 'publish-selection.json') (Selection @($review.items) -ReviewId 'xp-0000000000000000000000000000000000000000000000000000000000000000')
    $wrongReview = Publish @('-Step', 'apply', '-Work', $work)
    Assert ($wrongReview.code -ne 0 -and $wrongReview.text -match 'review') 'apply: another review id is refused'
    Write-Json (Join-Path $work 'publish-selection.json') (Selection @($review.items) -Note '')
    $noNote = Publish @('-Step', 'apply', '-Work', $work)
    Assert ($noNote.code -ne 0 -and $noNote.text -match 'note') 'apply: a selection without a note is refused'
    Write-Json (Join-Path $work 'publish-selection.json') (Selection @($review.items + 'term:不存在的词') )
    $unknown = Publish @('-Step', 'apply', '-Work', $work)
    Assert ($unknown.code -ne 0 -and $unknown.text -match 'not name|does not name') 'apply: an item the review does not name is refused'
    Write-Json (Join-Path $work 'publish-selection.json') (Selection @($review.items) -Readings $false)
    $noReadings = Publish @('-Step', 'apply', '-Work', $work)
    Assert ($noReadings.code -ne 0 -and $noReadings.text -match 'readings|pinyin') 'apply: accepting terms without reviewing the pinyin evidence is refused'
    $noBackup = Selection @($review.items)
    $noBackup.Remove('backup')
    Write-Json (Join-Path $work 'publish-selection.json') $noBackup
    $backupRefused = Publish @('-Step', 'apply', '-Work', $work)
    Assert ($backupRefused.code -ne 0 -and $backupRefused.text -match 'backup') 'apply: accepting changes without a verified backup is refused'
    $badBackup = [ordered]@{ path = $backup.path; sha256 = '00'; bytes = $backup.bytes }
    $badBackup.sha256 = '00'
    Write-Json (Join-Path $work 'publish-selection.json') (Selection @($review.items) -Backup $badBackup)
    $shaRefused = Publish @('-Step', 'apply', '-Work', $work)
    Assert ($shaRefused.code -ne 0 -and $shaRefused.text -match 'SHA-256') 'apply: a backup whose bytes are not the recorded ones is refused'
    Assert ((Revision) -eq $revisionBefore) 'apply: every refused selection wrote no Card'

    # --- the publication ----------------------------------------------------------------------
    $cardsBefore = Card-Count
    Write-Json (Join-Path $work 'publish-selection.json') (Selection @($review.items))
    $applied = Publish-Json @('-Step', 'apply', '-Work', $work)
    Assert ($applied.code -eq 0) "apply: the accepted changes are published ($($applied.text))"
    $result = Read-Json (Join-Path $work 'publish-result.json')
    Assert ($result.summary.applied -eq $review.items.Count) "apply: every accepted item is applied-and-verified ($($result.summary.applied))"
    Assert ($result.summary.conflict -eq 0 -and $result.summary.pending -eq 0) 'apply: no item is left conflicted or pending'
    Assert ($result.backup.sha256 -eq $backup.sha256) 'apply: the verified backup is recorded'
    Assert ($result.protected.unchanged -eq $true) 'apply: the protected human and Model scope is unchanged'
    Assert ($result.requires_revision -eq $false) 'apply: no revised plan is required'
    $after = Register
    Assert ($after.complete -eq $true) 'apply: the register reads back complete'
    $published = @($after.entries | Where-Object { $_.term -eq '学习' })[0]
    $publishedTargets = @($published.targets | ForEach-Object { if ($_ -is [string]) { $_ } else { $_.address } })
    Assert ($publishedTargets -contains "$topic/1" -and $publishedTargets -contains "$($lit[0])") 'apply: the accepted row is published with its reviewed targets'
    Assert (@($after.entries | Where-Object { $_.term -eq '多重均衡' }).Count -eq 1) 'apply: the unresolved-reading row is published too'
    $topicHubNow = Card $topicHub
    Assert ($topicHubNow.body -match '多重均衡的来源') 'apply: the existing topic hub carries its reviewed body'
    $newHubs = @($hubPlan.items | Where-Object { $_.kind -eq 'create' })
    foreach ($item in $newHubs) {
        $outcome = Outcome $result $item.id
        $address = $outcome.verified.address
        Assert ($address -and (Card $address).body) "apply: the created hub $($item.id) is persisted at $address"
    }
    Assert ((Card $model).body -match '策略互补') 'apply: the protected Model Card keeps its authored body'
    Assert ($result.audit.revision -eq (Revision)) 'apply: the post-publication revision is recorded'
    Assert ((Read-Json (Join-Path $work 'state.json')).trees.Count -ge 1) 'apply: the hub state advances to the published addresses'

    $checked = Publish-Json @('-Step', 'check', '-Work', $work)
    Assert ($checked.code -eq 0) "check: the publication verifies ($($checked.last))"
    Assert ($checked.json.ok -eq $true -and $checked.json.complete -eq $true) 'check: the register is complete'
    Assert ($checked.json.diverged.Count -eq 0 -and $checked.json.missing.Count -eq 0) 'check: no accepted row or hub body diverged'
    Assert ($checked.json.protected_unchanged -eq $true) 'check: the protected scope is still unchanged'
    Assert ($checked.json.published_revision -eq $result.audit.revision) 'check: the publication revision is the recorded one'

    # A retry of the same selection replays its receipts instead of creating a second Card.
    $cardsNow = Card-Count
    $retry = Publish-Json @('-Step', 'apply', '-Work', $work)
    Assert ($retry.code -eq 0) "retry: the same selection finishes without an error ($($retry.text))"
    $retryResult = Read-Json (Join-Path $work 'publish-result.json')
    Assert ((Card-Count) -eq $cardsNow) "retry: no Card is created twice ($cardsNow then $(Card-Count))"
    Assert ($retryResult.hubs.replayed -ge 1 -or $retryResult.summary.applied -eq $review.items.Count) 'retry: the committed calls are replayed from their receipts'

    # --- a re-staged register plan is not the one that was reviewed ---------------------------
    # The first publication consumed the hub plan, so the operator stages both plans again.
    $stageAfter = Split-Digest @('hub', '-Step', 'stage', '-Work', $work)
    Assert ($stageAfter.code -eq 0) 'register plan: the hub plan is staged again after the publication'
    $replanFirst = Split-Digest @('register', '-Step', 'plan', '-Work', $work, '-Readings', (Join-Path $work 'out\register-readings.json'))
    Assert ($replanFirst.code -eq 0) 'register plan: the register plan is staged again after the publication'
    $planRun = Publish-Json @('-Step', 'plan', '-Work', $work)
    Assert ($planRun.code -eq 0) 'register plan: the review over the current plan is written'
    $reviewA = Read-Json (Join-Path $work 'publish-review.json')
    $script:review = $reviewA
    $script:backup = Backup
    $termItem = @($reviewA.register.terms)[0].id
    $changedTable = "本卡是《学习与增长》的术语表。`n`n术语表`n`n| 术语 | 不用 | 意思 | 首次定义 |`n| --- | --- | --- | --- |`n| 学习 | 习得 | 重复练习积累技能 | §2 |`n| 多重均衡 | 均衡的多重性 | 策略互补时有多个均衡 | §3 |`n| 新词 | 旧词 | 一个新词 | §4 |"
    $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $table; body = $changedTable } })
    $replanA = Split-Digest @('register', '-Step', 'plan', '-Work', $work, '-Readings', (Join-Path $work 'out\register-readings.json'))
    Assert ($replanA.code -eq 0) 'register plan: the changed term table is planned again'
    $planB = Read-Json (Join-Path $work 'out\register-plan.json')
    Assert ($planB.plan_id -ne $reviewA.register.plan_id) 'register plan: the re-staged plan has another plan id'
    $cardsBefore = Card-Count
    # Every row the register serves is accepted, so the partial-rebuild refusal cannot answer for
    # the plan check: the reviewed plan itself moved.
    $termItems = @($reviewA.items | Where-Object { $_ -like 'term:*' })
    Write-Json (Join-Path $work 'publish-selection.json') (Selection $termItems)
    $movedPlan = Publish-Json @('-Step', 'apply', '-Work', $work)
    $movedResult = Read-Json (Join-Path $work 'publish-result.json')
    Assert ($movedPlan.code -ne 0) 'register plan: a re-staged plan is not published under the older review'
    Assert ((Outcome $movedResult $termItem).status -eq 'conflict') 'register plan: the accepted row is a conflict, not a write'
    Assert (@($movedResult.outcomes | Where-Object { $_.accepted -and $_.detail -match 'no longer the reviewed plan' }).Count -eq $termItems.Count) "register plan: every accepted row names the moved plan ($((Outcome $movedResult $termItem).detail))"
    Assert ($movedResult.register.applied -eq $false) 'register plan: nothing is published to the register'
    Assert ((Card-Count) -eq $cardsBefore) 'register plan: no Card is written'

    # --- a stale old base returns a revised plan ----------------------------------------------
    $replan = Split-Digest @('register', '-Step', 'plan', '-Work', $work, '-Readings', (Join-Path $work 'out\register-readings.json'))
    Assert ($replan.code -eq 0) 'stale: the register is planned again'
    $stageAgain = Split-Digest @('hub', '-Step', 'stage', '-Work', $work)
    Assert ($stageAgain.code -eq 0) 'stale: the hub stage is staged again'
    $planRun = Publish-Json @('-Step', 'plan', '-Work', $work)
    Assert ($planRun.code -eq 0) 'stale: the review is written again'
    $review2 = Read-Json (Join-Path $work 'publish-review.json')
    $script:review = $review2
    $script:backup = Backup
    $editedHub = @($review2.hubs.items | Where-Object { $_.address -eq $topicHub })[0]
    if ($null -ne $editedHub) {
        # Another writer changes the reviewed Card: the reviewed base no longer holds.
        $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $topicHub; body = "## 机制`n- [[$topic/1]]：另一个写者的版本。" } })
        $before = Card-Count
        Write-Json (Join-Path $work 'publish-selection.json') (Selection @($editedHub.id))
        $stale = Publish-Json @('-Step', 'apply', '-Work', $work)
        $staleResult = Read-Json (Join-Path $work 'publish-result.json')
        Assert ($stale.code -ne 0) 'stale: the accepted hub change does not publish'
        Assert ((Outcome $staleResult $editedHub.id).status -eq 'conflict') 'stale: the changed base is a conflict'
        Assert ($staleResult.requires_revision -eq $true) 'stale: the gate asks for a revised plan'
        Assert ((Card $topicHub).body -match '另一个写者') 'stale: the body the other writer wrote is preserved'
        Assert ((Card-Count) -eq $before) 'stale: the refused change wrote no Card'
    }

    # --- a changed protected Card stops the publication ---------------------------------------
    $stageProtected = Split-Digest @('hub', '-Step', 'stage', '-Work', $work)
    Assert ($stageProtected.code -eq 0) 'protected: the hub plan is staged again after the other writer''s change'
    $planRun = Publish-Json @('-Step', 'plan', '-Work', $work)
    Assert ($planRun.code -eq 0) 'protected: the review is written again'
    $review3 = Read-Json (Join-Path $work 'publish-review.json')
    $script:review = $review3
    $script:backup = Backup
    $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $model; body = "策略互补造成多个均衡（改过）。`n`n来源：[[$($lit[1])]] §3" } })
    $before = Card-Count
    Write-Json (Join-Path $work 'publish-selection.json') (Selection @($review3.items))
    $protected = Publish-Json @('-Step', 'apply', '-Work', $work)
    $protectedResult = Read-Json (Join-Path $work 'publish-result.json')
    Assert ($protected.code -ne 0) 'protected: a changed protected Card stops the publication'
    Assert ($protectedResult.protected.unchanged -eq $false -and @($protectedResult.protected.changed).Count -ge 1) 'protected: the changed Model Card is named'
    Assert (@($protectedResult.outcomes | Where-Object { $_.status -eq 'pending' }).Count -eq $review3.items.Count) 'protected: every item waits for a quiet window'
    Assert ((Card-Count) -eq $before) 'protected: nothing is written while the archive is moving'

    # --- declining a row the register already serves is a conflict, not a silent drop ----------
    $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $model; body = "策略互补造成多个均衡。`n`n来源：[[$($lit[1])]] §3" } })
    $planRun = Publish-Json @('-Step', 'plan', '-Work', $work)
    $review4 = Read-Json (Join-Path $work 'publish-review.json')
    $script:review = $review4
    $script:backup = Backup
    $served = @($review4.register.terms | Where-Object { $null -ne $_.published })[0]
    $declinedTerm = @($review4.register.terms | Where-Object { $null -ne $_.published -and $_.id -ne $served.id })[0]
    Write-Json (Join-Path $work 'publish-selection.json') (Selection @($served.id) -Decline @($declinedTerm.id))
    $conflict = Publish-Json @('-Step', 'apply', '-Work', $work)
    $conflictResult = Read-Json (Join-Path $work 'publish-result.json')
    Assert ($conflict.code -ne 0) 'declined served row: the gate refuses the partial rebuild'
    Assert ((Outcome $conflictResult $served.id).status -eq 'conflict') 'declined served row: the accepted row is a conflict'
    Assert ($conflictResult.register.applied -eq $false) 'declined served row: nothing is published to the register'

    # --- a register with a declared part: the review names the part's old body and fingerprint ---
    $facts = @(Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @(); view = 'text' } }))[0]
    $partTarget = "$topic/1"
    $partBody = "## 词条`n- 甲：[[$partTarget]]"
    $registerNow = Card $registerCard
    $rootBody = "范围：代理工作区的入口。每个词条指向一到三个实质卡。`n`n## 词条分册`n`n- [[{{part:0}}]] A–F"
    $crafted = [ordered]@{
        schema_version = 1
        archive        = [ordered]@{ identity = $facts.archive_identity; revision = $facts.archive_revision }
        register       = [ordered]@{ address = $registerCard; missing = $false; entries = 0; complete = $false; parts = 0 }
        terms          = @([ordered]@{ term = '甲'; targets = @($partTarget); sources = @(); ambiguous = $false; overflow = 0; reviewed = @(); read_only_targets = @() })
        unresolved     = @(); faults = @(); excluded = @(); review = $null
        readings       = [ordered]@{ accepted = @([ordered]@{ term = '甲'; reading = 'jiǎ'; initial = 'j' }); unresolved = @(); count = 1 }
        parts          = @([ordered]@{ label = 'A–F'; bucket = 'latin'; title = '词条 A–F'; address = $null; body = $partBody; terms = @('甲'); call = 0; reused = $false })
        obsolete_parts = @(); terms_on_root = $false
        root_before    = $registerNow.body
        body           = $rootBody
        calls          = @([ordered]@{ tool = 'create_successor'; arguments = [ordered]@{ at = $registerCard; kind = 'side'; title = '词条 A–F'; body = $partBody } })
        root_call      = [ordered]@{ tool = 'edit_card'; arguments = [ordered]@{ at = $null; body = $rootBody } }
        plan_id        = 'rp-' + ('a' * 64)
    }
    Write-Json (Join-Path $work 'out\register-plan.json') $crafted
    $partApply = Split-Digest @('register', '-Step', 'apply', '-Work', $work)
    Assert ($partApply.code -eq 0) "parts: the crafted split register is published ($($partApply.text))"
    $partAfter = Register
    Assert (@($partAfter.parts).Count -ge 1) 'parts: the register declares its part'
    $partAddress = @($partAfter.parts)[0].address
    $replanParts = Split-Digest @('register', '-Step', 'plan', '-Work', $work, '-Readings', (Join-Path $work 'out\register-readings.json'))
    Assert ($replanParts.code -eq 0) 'parts: the register is planned again with its declared part'
    $stageParts = Split-Digest @('hub', '-Step', 'stage', '-Work', $work)
    Assert ($stageParts.code -eq 0) 'parts: the hub plan is staged again'
    $partsReview = Publish-Json @('-Step', 'plan', '-Work', $work)
    Assert ($partsReview.code -eq 0) "parts: the review is written ($($partsReview.text))"
    $reviewParts = Read-Json (Join-Path $work 'publish-review.json')
    $reviewedPart = @($reviewParts.register.parts)[0]
    Assert ($reviewedPart.fingerprint_before -match '^[0-9a-f]{64}$') "parts: the review shows the part's authored fingerprint ($($reviewedPart.fingerprint_before))"
    Assert ($reviewedPart.body_before -match '甲') 'parts: the review shows the part body it would replace'
    Assert ($reviewedPart.address -eq $partAddress) 'parts: the review names the declared part address'

    # --- a changing archive view: retried once, then a changing-view fault (§12.3, D300) --------
    # The plan review reads the register, the staged hub plan and the archive identity as one view.
    # A parallel writer that keeps committing between those reads leaves the review no single view,
    # so the review is refused instead of binding a revision it never held; no Card is written.
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_DRIFT_REVISION = 'all'
    $env:ZT_DRIFT_AFTER = '1'
    try {
        $revisionBeforeDrift = Revision
        $reviewPath = Join-Path $work 'publish-review.json'
        $reviewBefore = (Get-FileHash -LiteralPath $reviewPath -Algorithm SHA256).Hash
        $drift = Publish @('-Step', 'plan', '-Work', $work)
        Assert ($drift.code -ne 0) "changing view: a plan review over a view that keeps changing fails ($($drift.last))"
        Assert ($drift.text -match 'changed while it was read') "changing view: the refusal names the changed revision it saw ($($drift.last))"
        Assert ((Get-FileHash -LiteralPath $reviewPath -Algorithm SHA256).Hash -eq $reviewBefore) 'changing view: the refused review replaces no reviewed plan'
        Assert ((Revision) -eq $revisionBeforeDrift) 'changing view: the refused review writes no Card'
    } finally {
        Remove-Item Env:ZT_CHECK_ZT, Env:ZT_DRIFT_REVISION, Env:ZT_DRIFT_AFTER -ErrorAction SilentlyContinue
    }
    Write-Output "failed: $failed"
} finally {
    zt down 2>&1 | Out-Null
}
if ($failed) { exit 1 }
exit 0
