#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\finalize.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# The finite finalization branch (#86, R299/R300) at its real command boundary: a scratch archive, real
# `prepare -Variant econ -Apply` runs, the real `zt-digest audit`, the real `zt-digest card` correction cycle
# scoped by the audit plan, and `zt-digest finalize plan|scope|refresh|final` on the same staging folder.
# It checks the four branches (no_fix, fix, conflict, unresolved), the exact changed-Card manifest, the new
# publication revisions (and their reuse), the final audit always after the corrections (a stale report is
# refused), residual faults reported as partial, and that a second correction cycle is refused, never looped.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "finalize-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')

function Assert($condition, $message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) { throw "zt-digest $($Arguments -join ' ') failed ($LASTEXITCODE): $($lines -join ' | ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Try-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    return [pscustomobject]@{ code = $LASTEXITCODE; text = ($lines -join ' '); json = $(if (-not $LASTEXITCODE) { $lines[-1] | ConvertFrom-Json -AsHashtable } else { $null }) }
}
function Finalize([string]$Step, [string]$Out, [string[]]$More = @()) {
    return Run-Digest (@('finalize', '-Step', $Step, '-Out', $Out) + $More)
}
function Save-Json($path, $value) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
    [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 12), $utf8)
}
function Read-Json($path) { Get-Content -Raw -LiteralPath $path -Encoding utf8 | ConvertFrom-Json -AsHashtable }
function Write-Calls($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "zt: $($lines -join ' | ')" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}

# Mark every upstream unit done, so the finalization's aggregate publications have their real
# contributions and their inputs are settled: the fixture stands in for a completed digest.
$markDone = @'
const { DatabaseSync } = require('node:sqlite');
const path = require('node:path');
const os = require('node:os');
const ledger = path.resolve(process.argv[1]);
const keep = (process.argv[2] || '').split(',').filter(Boolean);
if (!ledger.startsWith(path.resolve(os.tmpdir()) + path.sep) || !path.basename(path.dirname(path.dirname(ledger))).startsWith('finalize-test-')) throw new Error('not the isolated ledger');
const db = new DatabaseSync(ledger);
for (const row of db.prepare("select id,layer,state from units").all()) {
  if (row.layer === 'cleanup' || row.layer === 'audit' || keep.includes(row.layer)) continue;
  if (row.state === 'done') continue;
  const result = row.layer === 'digest'
    ? { schema_version: 1, kind: 'digest_completion', disposition: 'verified', emitted: [], owned: [], receipt_keys: [] }
    : { schema_version: 1, disposition: 'verified' };
  db.prepare("update units set state='done',attempts=1,result_json=? where id=?").run(JSON.stringify(result), row.id);
}
db.close();
'@
function Complete-Upstream($Out, [string]$Keep = '') {
    & node --no-warnings -e $markDone (Join-Path $Out 'ledger.sqlite3') $Keep
    if ($LASTEXITCODE) { throw 'fixture: failing to complete the upstream units' }
}

# The optional content layers stay out, so every fixture run has the same small set of publications.
$runOptions = Join-Path $scratch 'run-options.json'

# One prepared run of a three-section book; the fixture Cards are added by the caller.
function New-Prepared($Name) {
    $source = Join-Path $scratch "$Name.md"
    [IO.File]::WriteAllText($source, @"
# 导论

本书讲契约与所有权。

# 第 1 章 不完全契约

契约无法写明所有情形，因此所有权决定剩余控制权。

# 第 2 章 债务与治理

债务在违约时把控制权交给债权人，因此治理结构随融资方式改变。
"@, $utf8)
    $out = Join-Path $scratch $Name
    $null = Run-Digest @('prepare', '-Source', $source, '-Out', $out, '-CitationKey', $Name, '-BibTeX', "@book{$Name,title={$Name}}", '-StartHeading', '导论', '-EndHeading', '', '-MinUnitChars', '1', '-NoFigures', '-RunOptions', $runOptions, '-Variant', 'econ', '-Apply')
    return $out
}

try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    [IO.File]::WriteAllText($runOptions, '{"terms":false,"events":false,"findings":false,"models":false,"hubs":false,"bridge":false}', $utf8)

    # --- the fix scenario: one correctable Card, one Card a done Model build owns -----------------------
    $key = 'FinalizeBook'
    $out = New-Prepared $key
    # "依赖 [[…]]，后者…" is a kinded Link with its reason, so the two Idea Cards are each other's
    # kinded peer and each has an authored inbound Link: the only hard fault left is the semicolon.
    $made = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = '所有权决定空白处的用法'
                body = "契约无法写明所有情形，所有者因此决定空白处的资产用法，这个用法由所有权决定。`n`n来源：[[$key]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = '剩余控制权提高谈判地位'
                body = "剩余控制权提高谈判地位；所有者可以拒绝合作并带走资产，谈判地位因此提高。`n`n来源：[[$key]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/2"; kind = 'side'; type = 'model'; level = 'concept'; title = '债务治理模型'
                body = "债务把控制权交给债权人；违约时治理结构改变。`n`n来源：[[$key]] §2" } }
    )
    $good, $faulty, $model = $made
    $null = Write-Calls @(
        @{ tool = 'edit_card'; arguments = @{ at = $good; body = "契约无法写明所有情形，所有者因此决定空白处的资产用法，这个用法由所有权决定。依赖 [[$faulty]]，后者给出剩余控制权提高谈判地位的论证。`n`n来源：[[$key]] §1" } },
        @{ tool = 'edit_card'; arguments = @{ at = $faulty; body = "剩余控制权提高谈判地位；所有者可以拒绝合作并带走资产，谈判地位因此提高。依赖 [[$good]]，后者给出契约不完全时所有权决定用法的前提。`n`n来源：[[$key]] §1" } }
    )
    Complete-Upstream $out

    # --- the scheduling audit: the read-only revision the plan reads --------------------------------------
    $null = Run-Digest @('audit', '-CitationKey', $key, '-Out', $out)
    $audit = Read-Json (Join-Path $out 'audit.json')
    $scheduledRevision = $audit.archive_revision
    Assert ($audit.complete -and $scheduledRevision -gt 0) "audit: a complete read-only audit at graph revision $scheduledRevision"
    $faultText = Get-Content -Raw (Join-Path $out 'audit-faults.txt') -Encoding utf8
    Assert ($faultText -match [regex]::Escape($faulty) -and $faultText -match 'R3') 'audit: the semicolon Card is an R3 fault'
    Assert ($faultText -match [regex]::Escape($model) -and $faultText -match 'R3') 'audit: the Model Card carries its own R3 fault'
    Assert (-not ($faultText -match ("(?m)^" + [regex]::Escape($good) + ' '))) 'audit: the linked Idea Card is clean'

    # --- plan: the scheduling audit becomes a revision, and the plan names the branches --------------------
    $plan = Finalize 'plan' $out
    $scheduledUnit = $plan.audit.unit
    $scheduledAuditRevision = $plan.audit.revision
    Assert ($plan.kind -eq 'audit_plan' -and $scheduledUnit -match '^audit/book@\d+$' -and $scheduledAuditRevision -ge 1) "plan: the scheduling audit is published as a checked audit revision ($scheduledUnit)"
    Assert ($plan.audit.graph_revision -eq $scheduledRevision) "plan: the plan names the graph revision the audit read ($($plan.audit.graph_revision))"
    Assert ($plan.branch -ceq 'fix') "plan: one correctable fault takes the fix branch ($($plan.branch))"
    Assert (@($plan.fix_cycle.cards) -contains $faulty -and @($plan.fix_cycle.cards).Count -eq 1) "plan: the cycle's Card list is exactly the correctable Card ($(@($plan.fix_cycle.cards) -join ', '))"
    Assert (@($plan.fix_cycle.rules) -contains 'R3' -and $plan.fix_cycle.permitted -eq 1 -and $plan.fix_cycle.used -eq 0) "plan: one permitted cycle and the rules it corrects ($(@($plan.fix_cycle.rules) -join ','))"
    $modelFault = @($plan.faults.list | Where-Object { $_.address -ceq $model })
    Assert ($modelFault.Count -ge 1 -and @($modelFault | Where-Object { $_.outcome -ceq 'blocked' }).Count -eq $modelFault.Count) 'plan: a fault on a done Model build is blocked, never a correction target'
    Assert (@($plan.refresh | Where-Object { $_.layer -eq 'figures' -and $_.applicable -eq $false -and $_.reason -match 'excluded' }).Count -eq 1) 'plan: an excluded layer names its own reason'
    Assert (@($plan.refresh | Where-Object { $_.layer -eq 'audit' -and $_.applicable -eq $true }).Count -eq 1) 'plan: the final audit is always the branch last step'
    Assert ((Test-Path (Join-Path $out 'finalize\audit-plan.json')) -and (Test-Path (Join-Path $out 'finalize\audit-plan.md'))) 'plan: the immutable plan and its report are written'
    $cardList = @(Get-Content (Join-Path $out 'finalize\fix-cards.txt') -Encoding utf8 | Where-Object { $_.Trim() })
    Assert ($cardList.Count -eq 1 -and $cardList[0] -ceq $faulty) 'plan: the fix card list names exactly the planned Card'

    # --- the one correction cycle: the real card compiler, scoped by the plan ------------------------------
    $fixWork = Join-Path $out 'finalize\fix'
    $cardPlan = Run-Digest @('card', '-Step', 'plan', '-Book', $key, '-Work', $fixWork, '-Cards', (Join-Path $out 'finalize\fix-cards.txt'), '-Rules', (@($plan.fix_cycle.rules) -join ','), '-NoOrphans')
    Assert ($cardPlan.picked -eq 1 -and @($cardPlan.cards_scope).Count -eq 1 -and @($cardPlan.cards_scope)[0] -ceq $faulty) "cards: the cycle picks only the planned Card ($($cardPlan.picked) picked)"
    $state = Read-Json (Join-Path $fixWork 'state.json')
    $fixJobs = @($state.jobs.Keys | Where-Object { $_ -like 'fix-*' })
    Assert ($fixJobs.Count -eq 1) 'cards: the scoped plan has one fix job'
    $fixJob = $state.jobs[$fixJobs[0]]
    Assert (@($fixJob.cards) -contains $faulty -and @($fixJob.cards).Count -eq 1) 'cards: the job names the planned Card only'
    Save-Json (Join-Path $fixWork "out\$($fixJobs[0]).json") @{ cards = @(@{ address = $faulty; verdict = 'fix'; title = '剩余控制权提高谈判地位'
                body = "剩余控制权提高谈判地位。所有者可以拒绝合作并带走资产，谈判地位因此提高。依赖 [[$good]]，后者给出契约不完全时所有权决定用法的前提。`n`n来源：[[$key]] §1" }) }
    $null = Run-Digest @('card', '-Step', 'apply', '-Work', $fixWork)
    $check = Run-Digest @('card', '-Step', 'check', '-Work', $fixWork)
    Assert ($check.faults -eq 0) "cards: the rewritten Card has no fault left ($($check.faults))"
    $null = Run-Digest @('card', '-Step', 'apply', '-Work', $fixWork)
    $cycle = Run-Digest @('card', '-Step', 'report', '-Work', $fixWork)
    Assert ($cycle.faults_before -gt 0 -and $cycle.faults_after -eq 0 -and $cycle.cards -eq 1) "cards: the one cycle corrected the planned Card ($($cycle.faults_before) → $($cycle.faults_after))"
    $fixState = Read-Json (Join-Path $fixWork 'state.json')
    Assert (@($fixState.applied.Keys) -contains $faulty) 'cards: the applied Card is the planned Card'

    # --- scope: the exact changed-Card/touched-tree manifest ----------------------------------------------
    $changes = Finalize 'scope' $out @('-FixWork', $fixWork)
    Assert ($changes.kind -eq 'finalize_changes' -and $changes.counts.cards -eq 1) "scope: one changed Card is recorded ($($changes.counts.cards))"
    Assert (@($changes.changed | Where-Object { $_.address -ceq $faulty }).Count -eq 1) 'scope: the manifest names the corrected Card'
    Assert (@($changes.kinds) -contains 'structure') "scope: the change kinds feed the structure publications ($(@($changes.kinds) -join ', '))"
    Assert (@($changes.touched_trees) -contains $key) 'scope: the touched tree is the book'
    $manifest = Read-Json (Join-Path $out 'finalize\changes.json')
    Assert ($manifest.change_hash -match '^[0-9a-f]{64}$' -and $manifest.graph_revision -ge $scheduledRevision) 'scope: the manifest carries its own hash and the graph revision after the fix'
    Assert (@($changes.refresh | Where-Object { $_.layer -eq 'link-check' -and $_.applicable -eq $true }).Count -eq 1) 'scope: the changed manifest makes the Link check refresh applicable'
    $second = Try-Digest @('finalize', '-Step', 'scope', '-Out', $out, '-FixWork', $fixWork)
    Assert ($second.code -eq 1 -and $second.text -match 'already recorded') "scope: the plan's one cycle cannot be spent twice ($($second.code))"

    # --- refresh: affected downstream publications get new revisions --------------------------------------
    $report = Join-Path $fixWork 'report.json'
    $linkCheck = Finalize 'refresh' $out @('-Layer', 'link-check', '-Report', $report)
    $linkCheckUnit = $linkCheck.unit
    $linkCheckRevision = $linkCheck.revision
    Assert ($linkCheckUnit -match '^link-check/book@\d+$' -and $linkCheck.state -ceq 'done' -and $linkCheck.changed_cards -eq 1) "refresh: the Link check revision completes on the changed-Card manifest ($linkCheckUnit)"
    $linkCheckStatus = Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'link-check')
    $linkCheckInputs = @($linkCheckStatus.units | Where-Object { $_.id -ceq $linkCheckUnit })[0].inputs
    Assert ($linkCheckInputs.scope.kind -ceq 'finalize_refresh' -and @($linkCheckInputs.scope.changed_cards).Count -eq 1) 'refresh: the revision scope is the changed-Card manifest, so the same changes reuse one revision'
    $again = Finalize 'refresh' $out @('-Layer', 'link-check', '-Report', $report)
    Assert ($again.revision -eq $linkCheckRevision -and $again.reused -eq $true -and @($again.superseded).Count -eq 0) 'refresh: the identical manifest reuses the registered revision instead of looping'
    # The layers whose own stage generates their content are named as stage-owned, never
    # republished from an artifact this branch did not re-derive.
    $organize = Try-Digest @('finalize', '-Step', 'refresh', '-Out', $out, '-Layer', 'organize', '-Decisions', (Join-Path $fixWork 'decisions.json'), '-Applied', $report)
    Assert ($organize.code -eq 1 -and $organize.text -match 'its own stage generates this content') "refresh: a stage-owned layer is refused with its reason ($($organize.code))"
    $notAffected = Try-Digest @('finalize', '-Step', 'refresh', '-Out', $out, '-Layer', 'bridge', '-Report', $report, '-Coverage', $report)
    Assert ($notAffected.code -eq 1 -and ($notAffected.text -match 'needs no refresh' -or $notAffected.text -match 'its own stage generates')) "refresh: a layer the change does not affect is refused with its reason ($($notAffected.code))"

    # --- final: a fresh read-only audit after the corrections, never the stale one ------------------------
    $stale = Try-Digest @('finalize', '-Step', 'final', '-Out', $out, '-Report', (Join-Path $out 'audit.json'))
    Assert ($stale.code -eq 1 -and $stale.text -match 'stale report can never be the final audit') "final: the report that scheduled the corrections is refused ($($stale.code))"
    $final = Finalize 'final' $out
    Assert ($final.audit.final.revision -eq ($scheduledAuditRevision + 1) -and $final.audit.final.unit -match '^audit/book@\d+$') "final: the final audit is published as the next audit revision ($($final.audit.final.unit))"
    Assert ($final.graph_revision -gt $scheduledRevision) "final: the final report names the new graph revision ($($final.graph_revision) > $scheduledRevision)"
    Assert ($final.status -ceq 'partial' -and $final.complete -eq $false) "final: residual hard faults leave the branch partial, never claiming the scheduling audit ($($final.status))"
    $residual = @($final.faults.list)
    Assert (@($residual | Where-Object { $_.address -ceq $model -and $_.rule -ceq 'R3' -and $_.class -ceq 'hard' }).Count -eq 1) 'final: the residual Model fault is named as a hard fault'
    Assert ($final.faults.by_class.hard -ge 1 -and $final.faults.residual -eq $residual.Count) 'final: every residual fault is counted by class'
    Assert (@($final.refresh | Where-Object { $_.layer -eq 'link-check' -and $_.refreshed -eq $true -and $_.revision -eq $linkCheckRevision }).Count -eq 1) 'final: the refreshed Link check revision is reported as current'
    Assert (@($final.refresh | Where-Object { $_.layer -eq 'audit' -and $_.unit -ceq $final.audit.final.unit }).Count -eq 1) 'final: the final audit revision is part of the refresh facts'
    $organizeFact = @($final.refresh | Where-Object { $_.layer -eq 'organize' })[0]
    Assert ($organizeFact.applicable -eq $false -and $organizeFact.stage_owned -eq $true -and $organizeFact.reason -match 'its own stage generates') 'final: a stage-owned publication names its owner instead of claiming a refresh'
    Assert (@($final.refresh | Where-Object { $_.layer -eq 'link-check' -and $_.applicable -eq $true }).Count -eq 1) 'final: the refreshed Link check is reported as applicable at the change manifest'
    Assert ((Test-Path (Join-Path $out 'finalize\final-report.json')) -and (Test-Path (Join-Path $out 'finalize\final-report.md'))) 'final: the final report is written'
    $finalReport = Read-Json (Join-Path $out 'finalize\final-report.json')
    Assert ($finalReport.audit.scheduled.unit -ceq $scheduledUnit -and $finalReport.audit.final.unit -ceq $final.audit.final.unit) 'final: the report keeps the scheduling audit and the final audit distinct'
    $finalText = Get-Content -Raw (Join-Path $out 'finalize\final-report.md') -Encoding utf8
    Assert ($finalText -match 'partial' -and $finalText -match [regex]::Escape($model)) 'final: the markdown report names the residual fault and the partial status'
    $auditStatus = Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'audit')
    $published = @($auditStatus.publications | Where-Object { $_.layer -eq 'audit' })
    $auditRow = @($published | Where-Object { $_.layer -eq 'audit' })[0]
    Assert ($auditRow.revision -eq $final.audit.final.revision -and $auditRow.current -eq $true) 'final: the final audit revision is the current publication'
    $history = @($auditRow.superseded | Where-Object { $_.revision -eq $scheduledAuditRevision })
    Assert ($history.Count -eq 1 -and $history[0].state -ceq 'done') 'final: the superseded scheduling audit keeps its checked completion as history'

    # --- the no-fix branch: a book with no hard fault takes no correction cycle ---------------------------
    $cleanKey = 'FinalizeClean'
    $cleanOut = New-Prepared $cleanKey
    Complete-Upstream $cleanOut
    $null = Run-Digest @('audit', '-CitationKey', $cleanKey, '-Out', $cleanOut)
    $cleanPlan = Finalize 'plan' $cleanOut
    Assert ($cleanPlan.branch -ceq 'no_fix' -and @($cleanPlan.fix_cycle.cards).Count -eq 0) "no_fix: a faultless book schedules no correction cycle ($($cleanPlan.branch))"
    Assert (-not (Test-Path (Join-Path $cleanOut 'finalize\fix-cards.txt'))) 'no_fix: no fix card list is written'
    $cleanFinal = Finalize 'final' $cleanOut
    Assert ($cleanFinal.status -ceq 'done' -and $cleanFinal.changes.cards -eq 0) "no_fix: the branch finishes done with no changed Card ($($cleanFinal.status))"
    Assert ($cleanFinal.audit.final.revision -eq ($cleanPlan.audit.revision + 1) -and $cleanFinal.graph_revision -ge $cleanPlan.graph_revision) 'no_fix: the final audit is a fresh revision of the same graph'

    # --- the conflict branch: a Card edited after the audit it scheduled ----------------------------------
    $conflictKey = 'FinalizeConflict'
    $conflictOut = New-Prepared $conflictKey
    # The peer Card keeps the audited Card's kinded-peer and inbound obligations clean, so the
    # semicolon is its only hard fault before the hand edit that takes the conflict branch.
    $peer = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$conflictKey/1"; kind = 'side'; title = '所有权决定空白处的用法'
                body = "契约无法写明所有情形，所有者因此决定空白处的资产用法，这个用法由所有权决定。`n`n来源：[[$conflictKey]] §1" } }))[0]
    $conflictCard = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$conflictKey/1"; kind = 'side'; title = '剩余控制权提高谈判地位'
                body = "剩余控制权提高谈判地位；所有者可以拒绝合作，谈判地位因此提高，这是本节的主张。依赖 [[$peer]]，后者给出契约不完全时所有权决定用法的前提。`n`n来源：[[$conflictKey]] §1" } }))[0]
    $null = Write-Calls @(@{ tool = 'edit_card'; arguments = @{ at = $peer; body = "契约无法写明所有情形，所有者因此决定空白处的资产用法，这个用法由所有权决定。依赖 [[$conflictCard]]，后者给出剩余控制权提高谈判地位的论证。`n`n来源：[[$conflictKey]] §1" } })
    Complete-Upstream $conflictOut
    $null = Run-Digest @('audit', '-CitationKey', $conflictKey, '-Out', $conflictOut)
    $null = Write-Calls @(@{ tool = 'edit_card'; arguments = @{ at = $conflictCard; body = "剩余控制权提高谈判地位。所有者可以拒绝合作，谈判地位因此提高，这是本节的主张。依赖 [[$peer]]，后者给出契约不完全时所有权决定用法的前提。`n`n来源：[[$conflictKey]] §1" } })
    $conflictPlan = Finalize 'plan' $conflictOut
    Assert ($conflictPlan.branch -ceq 'conflict' -and @($conflictPlan.fix_cycle.cards).Count -eq 0) "conflict: a Card that changed after its audit takes the conflict branch ($($conflictPlan.branch))"
    Assert (@($conflictPlan.faults.list | Where-Object { $_.address -ceq $conflictCard -and $_.outcome -ceq 'conflict' }).Count -ge 1) 'conflict: the stale audited base is named instead of rewritten'
    $conflictFinal = Finalize 'final' $conflictOut
    Assert ($conflictFinal.status -ceq 'done' -and $conflictFinal.faults.residual -eq 0) "conflict: the hand edit resolved the fault, so the fresh final audit is clean ($($conflictFinal.status))"

    # --- the unresolved branch: a fault the finite branch may never correct -------------------------------
    $blockedKey = 'FinalizeBlocked'
    $blockedOut = New-Prepared $blockedKey
    $null = Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$blockedKey/2"; kind = 'side'; type = 'model'; level = 'concept'; title = '债务治理模型'
                body = "债务把控制权交给债权人；违约时治理结构改变。`n`n来源：[[$blockedKey]] §1" } })
    Complete-Upstream $blockedOut
    $null = Run-Digest @('audit', '-CitationKey', $blockedKey, '-Out', $blockedOut)
    $blockedPlan = Finalize 'plan' $blockedOut
    Assert ($blockedPlan.branch -ceq 'unresolved' -and @($blockedPlan.fix_cycle.cards).Count -eq 0) "unresolved: a fault only a done build could carry takes the unresolved branch ($($blockedPlan.branch))"
    Assert (@($blockedPlan.faults.list | Where-Object { $_.outcome -ceq 'blocked' }).Count -ge 1) 'unresolved: the blocked fault names the reason the bounded branch may not rewrite it'
    $blockedFinal = Finalize 'final' $blockedOut
    Assert ($blockedFinal.status -ceq 'partial' -and @($blockedFinal.faults.list | Where-Object { $_.class -ceq 'hard' }).Count -ge 1) "unresolved: the final report keeps the hard fault as partial ($($blockedFinal.status))"

    # --- stage: a stage-owned publication is completed by its own stage ----------------------------------
    # grossman2002integration trial 8: no stage published its revision, so organize/skeleton/figures/bridge
    # book@1 stayed ready and no run could be content-complete.
    $stageKey = 'FinalizeStage'
    $stageOut = New-Prepared $stageKey
    Complete-Upstream $stageOut 'organize,skeleton'
    $stageWork = Join-Path $stageOut 'stage-artifacts'
    Save-Json (Join-Path $stageWork 'decisions.json') @{ thesis = '所有权决定剩余控制权'; merges = @(); moves = @(); notes = @() }
    Save-Json (Join-Path $stageWork 'applied.json') @{ merges = 0; moves = 0; refused = 0 }
    Save-Json (Join-Path $stageWork 'skeleton.json') @{ thesis = '所有权决定剩余控制权' }
    $stageOrganize = Finalize 'stage' $stageOut @('-Layer', 'organize', '-Decisions', (Join-Path $stageWork 'decisions.json'), '-Applied', (Join-Path $stageWork 'applied.json'))
    Assert ($stageOrganize.unit -match '^organize/book@\d+$' -and $stageOrganize.state -ceq 'done') "stage: Organize publishes its own revision ($($stageOrganize.unit) $($stageOrganize.state))"
    $stageSkeleton = Finalize 'stage' $stageOut @('-Layer', 'skeleton', '-Skeleton', (Join-Path $stageWork 'skeleton.json'))
    Assert ($stageSkeleton.unit -match '^skeleton/book@\d+$' -and $stageSkeleton.state -ceq 'done' -and $stageSkeleton.coverage.indexed -eq 3) "stage: the skeleton publishes its revision with its coverage counted per outcome ($($stageSkeleton.coverage | ConvertTo-Json -Compress))"
    $stageAgain = Finalize 'stage' $stageOut @('-Layer', 'skeleton', '-Skeleton', (Join-Path $stageWork 'skeleton.json'))
    Assert ($stageAgain.revision -eq $stageSkeleton.revision -and $stageAgain.reused -eq $true) 'stage: publishing the same stage again reuses its revision'
    $stageFigures = Finalize 'stage' $stageOut @('-Layer', 'figures', '-Report', (Join-Path $stageWork 'applied.json'))
    Assert ($stageFigures.state -ceq 'excluded') "stage: an excluded layer publishes nothing ($($stageFigures.state))"
    $stageWrong = Try-Digest @('finalize', '-Step', 'stage', '-Out', $stageOut, '-Layer', 'link-check', '-Report', (Join-Path $stageWork 'applied.json'))
    Assert ($stageWrong.code -eq 1 -and $stageWrong.text -match 'not a stage-owned publication') "stage: a layer no stage owns is refused ($($stageWrong.code))"
    $stageStatus = Run-Digest @('ledger', 'status', '-Out', $stageOut, '-Layer', 'skeleton')
    $stageRow = @($stageStatus.publications | Where-Object { $_.layer -eq 'skeleton' })[0]
    Assert ($stageRow.current -eq $true -and $stageRow.revision -eq $stageSkeleton.revision) 'stage: the published skeleton revision is the current publication'

    Write-Output 'all finalization checks passed'
} finally {
    & zt down *> $null
    if (Test-Path $scratch) { Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue }
}
