#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\pilot.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
<#
.SYNOPSIS
  The one-book pilot gate records, guards and qualifies a pilot (issue #117, S26).

.DESCRIPTION
  `zt-digest pilot` has three read-only steps over one prepared run: `plan` names the source,
  archive, staging, groups, baseline sample, installed binaries, Workflow versions, caps and
  approved new-Card scope before launch (S26-AC01); `preflight` refuses a launch that would reuse
  old evidence, a moved archive, a changed protected Card or a spent run (S26-AC02); `qualify`
  reads the run's own retained evidence and states what happened, with the decision always
  `pending-human` (S26-AC03 to S26-AC08).

  This suite drives the real public command on a scratch prepared run and proves:

    * a plan without its authorization record, an old-Card edit, a cap that differs from the
      prepared contract, or a run that already moved is refused, and the refusal names the gate;
    * a complete plan records the identity, the installed binaries with their hashes, the frozen
      Workflow version and the protected scope, and its `plan_id` covers the scope it approves;
    * preflight passes on a launch that still equals the plan, and refuses a plan after the run
      claimed work, after the archive moved, and after a protected Card changed;
    * qualify refuses without the run's own verified report, then reports a partial pilot as
      partial: the new Cards with their owning group and review gap, the missing Link completion,
      the unknown measured baseline, no exercised resume, and concrete old-data proposals;
    * a new Card the run did not write (an independent write) is named as unowned and unreviewed,
      and a changed protected Card is named rather than counted;
    * a retained Card-cycle review and Link completion are read per Card and per group, so an
      outcome is never invented; the decision stays `pending-human` in every run.

  The Card-cycle report and the Link completion are retained-artifact fixtures here: their real
  writers are `card-compiler.tests.ps1` and `link-gaps.tests.ps1`; this suite proves the gate reads
  the named shape and never fills a missing outcome. It starts no provider run and no workflow, and
  it does not touch the live archive.
#>
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "pilot-gate-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$here = Join-Path $PSScriptRoot '..' | Resolve-Path | ForEach-Object Path
$repo = Join-Path $PSScriptRoot '..\..\..' | Resolve-Path | ForEach-Object Path
. (Join-Path $here 'zt-calls.ps1')
$out = Join-Path $scratch 'prepared'
$work = Join-Path $scratch 'pilot'
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1 | ForEach-Object { "$_" })
    $json = $null
    if ($lines.Count) { try { $json = $lines[-1] | ConvertFrom-Json -AsHashtable -DateKind String } catch { } }
    [pscustomobject]@{ exit = $LASTEXITCODE; text = ($lines -join ' | '); json = $json }
}
function Run-Digest([string[]]$Arguments) {
    $got = Digest $Arguments
    if ($got.exit -ne 0) { throw "digest failed: $($got.text)" }
    $got.json
}
function Save-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 30), $utf8) }
function Hash($path) { (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() }
function Revision { (Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @(); view = 'text' } })).archive_revision }
function Backup {
    $source = Join-Path $scratch 'archive\zt.sqlite3'
    $target = Join-Path $scratch 'backup.sqlite3'
    $env:SRC = $source; $env:DST = $target
    node -e "const {DatabaseSync}=require('node:sqlite'); const s=new DatabaseSync(process.env.SRC,{readOnly:true}); s.exec(`"VACUUM INTO '`"+process.env.DST.replace(/\\/g,'/')+`"'`"); s.close();" 2>&1 | Out-Null
    [pscustomobject]@{ path = $target; sha256 = (Hash $target); bytes = (Get-Item -LiteralPath $target).Length }
}
function Authorization($overrides = @{}) {
    $ztPath = (Get-Command zt -CommandType Application | Select-Object -First 1).Source
    $checkPath = (Get-Command zt-check -CommandType Application | Select-Object -First 1).Source
    $digestPath = (Get-Item -LiteralPath $Digest).FullName
    $backup = Backup
    $auth = @{
        schema_version = 1
        approved_scope = @{ statement = 'One bounded pilot book: its own new Cards only.'; groups = @(1, 2, 3); old_card_edits = @() }
        backup = @{ path = $backup.path; sha256 = $backup.sha256; bytes = $backup.bytes; integrity = 'ok' }
        baseline = @{ report = @{ path = (Join-Path $scratch 'baseline\audit.json'); sha256 = (Hash (Join-Path $scratch 'baseline\audit.json')) }; addresses = @{ path = (Join-Path $scratch 'baseline\audit-addresses.txt'); sha256 = (Hash (Join-Path $scratch 'baseline\audit-addresses.txt')) } }
        preserved_builds = @(@{ path = $ztPath; sha256 = (Hash $ztPath); version = (& zt version) })
        comparison = @{ previous_run = (Join-Path $scratch 'previous'); variant_version = '4.4.0' }
        binaries = @(
            @{ name = 'zt'; path = $ztPath },
            @{ name = 'zt-check'; path = $checkPath },
            @{ name = 'zt-digest'; path = $digestPath }
        )
        launch = @{ mode = 'full'; args = @{ out = $out; citationKey = 'PilotGate'; maxAttempts = 3; dshRunCap = 1200 } }
    }
    foreach ($key in $overrides.Keys) { $auth[$key] = $overrides[$key] }
    $path = Join-Path $scratch "authorization-$([Guid]::NewGuid().ToString('N').Substring(0, 6)).json"
    Save-Json $path $auth
    $path
}
function Plan([string]$file) { Digest @('pilot', '-Step', 'plan', '-Staging', $out, '-Work', $work, '-File', $file, '-Project', $repo) }
function Preflight { Digest @('pilot', '-Step', 'preflight', '-Staging', $out, '-Work', $work, '-Project', $repo) }
function Qualify([string[]]$More = @()) { Digest (@('pilot', '-Step', 'qualify', '-Staging', $out, '-Work', $work, '-Project', $repo) + $More) }

try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    New-Item -ItemType Directory -Force (Join-Path $scratch 'baseline') | Out-Null
    New-Item -ItemType Directory -Force (Join-Path $scratch 'previous') | Out-Null

    # --- earlier material: an older book's Literature tree and one Card, so an earlier filing can be
    # read (a digest files no Card in a Topic tree: docs/literature-only-digest-design.md) ---
    $null = Invoke-Zt @(@{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{Earlier2020,title={Earlier book},author={Old, Ann},year={2020}}'; body = '一本早已读过的书。' } })
    $null = Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{ at = 'Earlier2020'; kind = 'direct'; title = '企业的边界由交易成本决定'; body = '企业的边界由交易成本决定。' } })
    $earlier = 'Earlier2020/1'

    # --- one prepared run of the pilot source ---
    $source = Join-Path $repo 'docs\digest-pilot-short-source.zh.md'
    $prepared = Run-Digest @('prepare', '-Variant', 'econ', '-Source', $source, '-Out', $out, '-CitationKey', 'PilotGate',
        '-BookTitle', 'Pilot gate book', '-BibTeX', '@book{PilotGate,title={Pilot gate book},author={Gate, Ada},year={2026}}',
        '-RunOptions', '{"terms":false}', '-Apply')
    Assert (@($prepared.groups).Count -eq 3) "prepare binds the pilot source with three groups ($(@($prepared.groups).Count))"
    $audit = Run-Digest @('audit', '-CitationKey', 'PilotGate', '-Out', (Join-Path $scratch 'baseline'), '-SkipModels')
    Assert (Test-Path (Join-Path $scratch 'baseline\audit.json')) 'the read-only baseline audit wrote its report and address list'

    # --- plan: refusals ---
    $noFile = Digest @('pilot', '-Step', 'plan', '-Staging', $out, '-Work', $work, '-Project', $repo)
    Assert ($noFile.exit -ne 0 -and $noFile.text -match 'File is required') 'a plan without its authorization record is refused before any file is written'
    $oldEdit = Authorization @{ approved_scope = @{ statement = 'one book'; groups = @(1, 2, 3); old_card_edits = @('1/1') } }
    $refused = Plan $oldEdit
    Assert ($refused.exit -ne 0 -and $refused.text -match 'scope.old_card_edits') 'a plan that approves an old-Card edit is refused (S26-AC06)'
    $wrongCap = Authorization @{ launch = @{ mode = 'full'; args = @{ out = $out; citationKey = 'PilotGate'; maxAttempts = 3; dshRunCap = 900 } } }
    $refused = Plan $wrongCap
    Assert ($refused.exit -ne 0 -and $refused.text -match 'launch.caps.*900.*1200') 'a cap that differs from the prepared contract is refused (S26-AC01)'
    $wrongGroups = Authorization @{ approved_scope = @{ statement = 'one book'; groups = @(1, 2); old_card_edits = @() } }
    $refused = Plan $wrongGroups
    Assert ($refused.exit -ne 0 -and $refused.text -match 'scope.groups') 'an approved group set that differs from the prepared run is refused'
    $missingBinary = Authorization @{ binaries = @(@{ name = 'zt'; path = (Get-Command zt -CommandType Application | Select-Object -First 1).Source }) }
    $refused = Plan $missingBinary
    Assert ($refused.exit -ne 0 -and $refused.text -match 'binaries.complete') 'a plan that does not name every installed binary is refused (S26-AC01)'

    # --- plan: the record ---
    $authPath = Authorization
    $plan = Plan $authPath
    Assert ($plan.exit -eq 0) "plan accepts a complete authorization record ($($plan.text))"
    $planRecord = Get-Content -LiteralPath (Join-Path $work 'pilot-plan.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($planRecord.decision -eq 'pending-human') 'the plan records the pending human decision, never an approval'
    # The frozen variant version is the econ blueprint's own, never a number copied into the test.
    $variantVersion = (Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot '..\workflow\zt-econ-digest.blueprint.json') | ConvertFrom-Json).version
    Assert ($planRecord.identity.citation_key -eq 'PilotGate' -and $planRecord.identity.contracts.variant_version -eq $variantVersion) 'the plan names the prepared book and the frozen variant version (S26-AC01)'
    Assert ($planRecord.identity.workflow.version -eq $variantVersion -and $planRecord.identity.workflow.script_sha256 -eq $planRecord.identity.workflow.recorded_sha256) 'the plan names the installed frozen Workflow and verifies its script hash'
    Assert (@($planRecord.binaries).Count -eq 3 -and ($planRecord.binaries | Where-Object { $_.name -eq 'zt-digest' }).sha256) 'the plan names all three installed binaries with their SHA-256'
    Assert ($planRecord.protected.count -ge 2 -and $planRecord.protected.prepared -ge 4 -and $planRecord.protected.old_digest.Length -eq 64) 'the plan fixes the protected scope with one digest over every old Card and names the prepared placeholders in the approved scope (S26-AC03)'
    Assert ($planRecord.identity.settings.max_attempts -eq 3 -and $planRecord.identity.settings.dsh_run_cap -eq 1200) 'the plan names the caps the run will use'
    Assert ($planRecord.identity.launch.invocation -match 'Workflow\(\{scriptPath:') 'the plan fixes the exact launch invocation'
    Assert (Test-Path (Join-Path $work 'pilot-plan.md')) 'the plan writes its readable package beside the record'
    Assert ($planRecord.backup.integrity -eq 'ok' -and $planRecord.backup.sha256.Length -eq 64) 'the plan records the verified backup as the rollback point (S26-AC01)'
    Assert ($planRecord.baseline.facts.archive_id -eq $planRecord.identity.archive.id) 'the plan reads the baseline report and its archive identity'
    Assert ($planRecord.comparison.previous_run -and $planRecord.preserved_builds.Count -ge 1) 'the plan keeps the previous run and the preserved build for comparison (S26-AC01)'

    # --- preflight: the launch still equals the plan ---
    $pre = Preflight
    Assert ($pre.exit -eq 0 -and $pre.json.ok -eq $true) "preflight passes on a run that has not started ($($pre.text))"
    Assert (@($pre.json.link.link_check_needs).Count -eq 3) 'preflight reports the Link prerequisite of every prepared group (S26-AC02)'
    Assert ($pre.json.archive.revision -eq (Revision)) 'preflight binds the archive revision it verified'

    # --- a moved archive is refused, and so is a changed protected Card ---
    $null = Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{ at = $earlier; title = '独立写入的新卡片'; body = '一张与本次授权无关的新卡片。' } })
    $moved = Preflight
    Assert ($moved.exit -ne 0 -and $moved.text -match 'archive.revision') 'preflight refuses an archive that moved since the plan (S26-AC02)'
    $movedRecord = Get-Content -LiteralPath (Join-Path $work 'pilot-preflight.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($movedRecord.ok -eq $false -and @($movedRecord.problems).Count -ge 1) 'a refused preflight keeps its facts instead of only printing an error'

    # --- a run that already claimed work is refused ---
    $second = Join-Path $scratch 'second'
    $null = Run-Digest @('prepare', '-Variant', 'econ', '-Source', $source, '-Out', $second, '-CitationKey', 'PilotGate2',
        '-BookTitle', 'Pilot gate book two', '-BibTeX', '@book{PilotGate2,title={Pilot gate book two},author={Gate, Ada},year={2026}}',
        '-RunOptions', '{"terms":false}', '-Apply')
    $workTwo = Join-Path $scratch 'pilot-two'
    $authTwo = Authorization @{ launch = @{ mode = 'full'; args = @{ out = $second; citationKey = 'PilotGate2'; maxAttempts = 3; dshRunCap = 1200 } } }
    $planTwo = Digest @('pilot', '-Step', 'plan', '-Staging', $second, '-Work', $workTwo, '-File', $authTwo, '-Project', $repo)
    Assert ($planTwo.exit -eq 0) "a second prepared run also gets a plan ($($planTwo.text))"
    $claim = Run-Digest @('ledger', 'next', '-Out', $second, '-Layer', 'digest', '-Take', '1', '-Owner', 'pilot-test')
    $afterClaim = Digest @('pilot', '-Step', 'preflight', '-Staging', $second, '-Work', $workTwo, '-Project', $repo)
    Assert ($afterClaim.exit -ne 0 -and $afterClaim.text -match 'run.fresh') 'preflight refuses a run that already claimed work (S26-AC02)'
    $planAfterClaim = Digest @('pilot', '-Step', 'plan', '-Staging', $second, '-Work', $workTwo, '-File', $authTwo, '-Project', $repo)
    Assert ($planAfterClaim.exit -ne 0 -and $planAfterClaim.text -match 'run.fresh') 'a plan is refused once the run moved: a plan describes a run before its first Card'

    # --- qualify: the run report is required ---
    $noReport = Qualify
    Assert ($noReport.exit -ne 0 -and $noReport.text -match 'no run report') 'qualify refuses without the run''s own verified report'

    # --- a real group cycle writes the run's first Cards ---
    $started = Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'pilot-test')
    $unit = $started.units[0]
    $groupStart = Run-Digest @('group-start', '-Out', $out, '-Group', '1', '-Token', "$($unit.token)")
    $job = Get-Content -LiteralPath $groupStart.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $prefix = "$($groupStart.run_id)/digest-g001/1/apply"
    $anchorBody = "§导论 - 为什么存在企业`n`n§1.1 - 企业边界来自成本比较"
    $draft = @{ schema_version = 1; unit = $unit.id; token = $unit.token; input_hash = $groupStart.input_hash; disposition = 'cards'; calls = @(
            @{ tool = 'edit_card'; arguments = @{ at = 'PilotGate/1'; title = '企业因市场失灵而出现'; body = $anchorBody; base_fingerprint = $job.inputs.anchor.fingerprint } },
            @{ tool = 'create_successor'; arguments = @{ at = 'PilotGate/1'; kind = 'side'; title = '企业因市场失灵而出现'; body = "市场失灵时企业替代价格机制。`n`n来源：[[PilotGate]] §导论" } },
            @{ tool = 'create_successor'; arguments = @{ at = 'PilotGate/1'; kind = 'side'; title = '企业边界是一次成本比较'; body = "企业边界来自一次成本比较。对照 [[$earlier]]：旧书把边界归于交易成本。`n`n来源：[[PilotGate]] §导论" } },
            @{ tool = 'edit_card'; arguments = @{ at = 'PilotGate/1'; base_receipt = "$prefix/item-000000"; body = "$anchorBody`n`n承接 [[@@1]]，后者说明市场失灵。`n`n承接 [[@@2]]，后者把边界放进旧书的笔记。" } }
        ) }
    Save-Json $groupStart.draft $draft
    $finished = Run-Digest @('group-finish', '-Out', $out, '-Group', '1', '-Token', "$($unit.token)", '-Draft', $groupStart.draft)
    $owned = @($finished.owned)
    Assert ($finished.state -eq 'done' -and $owned.Count -ge 2) "the run completes group 1 with its own receipts ($($owned -join ', '))"
    # docs/literature-only-digest-design.md: a group files only in its own Literature tree and links
    # the earlier book's Card instead of filing beside it.
    Assert (-not @($owned | Where-Object { $_ -like "$earlier|*" }).Count -and @($owned | Where-Object { $_ -like 'PilotGate/1|*' }).Count -ge 2) "the run files its Cards in its own Literature tree ($($owned -join ', '))"
    $null = Run-Digest @('ledger', 'report', '-Out', $out)

    # --- qualify without the reviewed evidence: partial, never a pass ---
    $partial = Qualify
    Assert ($partial.exit -eq 0 -and $partial.json.verdict -eq 'partial') "qualify reports a partial pilot as partial ($($partial.text))"
    Assert ($partial.json.decision -eq 'pending-human') 'the qualification always records the pending human decision (S26-AC08)'
    $qual = Get-Content -LiteralPath (Join-Path $work 'pilot-qualification.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $newCards = @($qual.cards.new)
    Assert ($newCards.Count -ge 2) "the qualification names every new Card ($($newCards.Count))"
    $filed = @($newCards | Where-Object { $_.earlier_filing -eq $true })
    Assert ($filed.Count -ge 1) 'the qualification names the new Card filed beside earlier thought'
    Assert (@($qual.cards.unreviewed).Count -ge 1 -and @($qual.checks | Where-Object { $_.name -eq 'cards.reviewed' -and $_.ok -ne $true })) 'a new Card with no review outcome is a named gap (S26-AC03)'
    Assert (@($qual.cards.link_completions_missing).Count -ge 1) 'a group with no Link completion is a named gap'
    Assert ($qual.interruption.exercised -eq $false -and $qual.interruption.reason) 'a run without the interruption says it was not exercised (S26-AC05)'
    Assert ($qual.measures.own_source.known -eq $false) 'the measured baseline is unknown without the audit, never zero'
    Assert (@($qual.old_data.changed).Count -eq 0 -and @($qual.old_data.applied).Count -eq 0) 'no old Card changed and the gate applied nothing (S26-AC06)'
    Assert ($qual.old_data.restatements.read -eq $false) 'a run without a Card cycle report leaves the restatement resolution unknown, never clean'
    Assert (@($qual.review.sample.addresses).Count -ge 1 -and $qual.review.sample.proves -eq 'sample') 'the human gets a deterministic representative sample (S26-AC08)'

    # --- a retained review and Link completion are read per Card and per group ---
    $reviewPath = Join-Path $out 'cards\report.json'
    New-Item -ItemType Directory -Force (Split-Path -Parent $reviewPath) | Out-Null
    Save-Json $reviewPath @{ kind = 'cards_cycle'; unit_kind = 'book'; book = 'PilotGate'; review = @(
            @{ address = @($newCards | Where-Object { $_.group -eq 1 })[0].address; kind = 'idea'; category = 'idea'; verdict = 'ok'; fingerprint = 'f' }
        ); restatement = @{ path = (Join-Path $out 'restate-resolution.json'); counts = @{ pending_existing_card_change = 1; unresolved = 0; recommended = 0; resolved_by_rewrite = 0; unverified = 0; different = 0 }; complete = $false } }
    $linkPath = Join-Path $out 'link-completion-g001.json'
    Save-Json $linkPath @{ schema_version = 1; unit = 'link/g001@1'; counts = @{ owned = 2; gaps = 0; linked = 0; linked_from = 0; already_linked = 2; no_link = 0 }; quality = @{ faults = 0; checked = 2 } }
    $qualified = Qualify @('-Audit', (Join-Path $scratch 'baseline\audit.json'))
    $qual = Get-Content -LiteralPath (Join-Path $work 'pilot-qualification.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($qualified.exit -eq 0) "qualify reads the audit and the retained review evidence ($($qualified.text))"
    $group1 = @($qual.cards.groups)[0]
    Assert ($group1.digest_state -eq 'done' -and $group1.owned -ge 2) 'the qualification reports the group''s own done state and owned manifest'
    Assert ($group1.link_completion.counts.already_linked -eq 2) 'the qualification reads the group''s Link completion counts'
    Assert ($qual.measures.own_source.known -eq $false) 'an audit without the baseline comparison leaves the pilot measure unknown'
    Assert ($qual.old_data.restatements.read -eq $true -and $qual.old_data.restatements.counts.pending_existing_card_change -eq 1) 'a retained Card cycle report contributes its restatement resolution'
    Assert (@($qual.old_data.proposals | Where-Object { $_.kind -eq 'restatement-pending-existing-card-change' -and $_.count -eq 1 }).Count -eq 1) 'a pending existing-Card change becomes one concrete old-data proposal for the separate gate (S26-AC06)'
    Assert (@($qual.cards.reviewed) -gt 0) 'one retained review outcome is counted as reviewed'

    # --- an independent write and a changed protected Card are named ---
    $null = Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{ at = $earlier; kind = 'side'; title = '另一张独立卡片'; body = '另一张独立卡片。' } })
    $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $earlier; title = '企业的边界由交易成本决定'; body = '被改动的旧卡片正文。' } })
    $changed = Qualify @('-Audit', (Join-Path $scratch 'baseline\audit.json'))
    $qual = Get-Content -LiteralPath (Join-Path $work 'pilot-qualification.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($changed.exit -eq 0) "qualify still reports when the scope moved ($($changed.text))"
    Assert (@($qual.scope.changed) -contains $earlier) 'a changed protected Card is named by address'
    Assert (@($qual.old_data.proposals | Where-Object { $_.kind -eq 'old-card-change' -and $_.address -eq $earlier }).Count -eq 1) 'the changed old Card becomes one concrete old-data proposal for the separate gate (S26-AC06)'
    Assert (@($qual.checks | Where-Object { $_.name -eq 'scope.no_old_change' -and $_.ok -ne $true })) 'the changed old Card fails the gate''s own scope check'
    Assert (@($qual.cards.unowned).Count -ge 1) 'a new Card the run did not write is named as unowned'
} finally {
    try { zt down | Out-Null } catch { }
}
Write-Output "failed: $failed"
exit $(if ($failed) { 1 } else { 0 })
