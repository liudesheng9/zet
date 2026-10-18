#Requires -Version 7.5
<#
.SYNOPSIS
  Rehearse the #91 Stiglitz reconciliation on a consistent copy of the live archive.

.DESCRIPTION
  docs/digest-recovery-91.md hands the authorized maintenance window to the human. This harness
  proves every command of that runbook except the provider work itself, on a consistent copy of the
  archive and a copy of the real legacy staging folder, with no provider call and no live Card write:

    1. Copies -Archive with `VACUUM INTO` (the archive is in WAL mode) and a copy of -Staging, so
       the live folder is read only for this run; it re-reads the live folder's hashes afterwards.
    2. Recounts every backlog category on the copy: `ledger adopt` is the read-only plan (groups,
       models, retained Finding jobs, recovered note suggestions, the retained Bridge report by
       hash, derived artifacts), and `audit` is the card-level recount (repeated paragraphs R18,
       repeated title lines R19, orphans, unkinded Links, derivation explanations, figures). The
       plan's own counts are asserted, and the dated numbers of the proposal are not trusted.
    3. Applies the adoption: the ledger binds the archive identity, every verified group and model
       becomes a done adopted unit with its exact fingerprints, and every missing group, model,
       note, finding job and publication becomes a targeted obligation. Only genuinely unbuilt
       publications may appear as coverage faults (R413).
    4. Injects one content failure on the run's next ready group and spends the one supported
       resume: invocation 2, resume_count 1, the reset unit, its published downstream closure, and
       every adopted unit's result, attempts and completion time unchanged.
    5. Asserts the second resume is refused as `resume_used`, the run report names its gaps instead
       of claiming completion, the Bridge report stays report-only with its exact hash and open
       proposal count, the after recount of the untouched categories is identical, and the copy
       still passes the full-parser Link check. It writes the before/after manifest of the affected
       units, source/index/report hashes and fingerprints as `before-after.json`.

  It exits 1 when any assertion fails, prints one line per assertion, and prints `failed: N` last.

  pwsh -NoProfile -File scripts\book-digest\tests\recovery-rehearsal.ps1 `
      -Archive D:\zt\zt.sqlite3 -Staging D:\zt-digest\stiglitz2014learning `
      -Work D:\zt-target\qualification\recovery-rehearsal
#>
[CmdletBinding()]
param(
    [string]$Archive = 'D:\zt\zt.sqlite3',
    [string]$Staging = 'D:\zt-digest\stiglitz2014learning',
    [string]$Work = '',
    [string]$Zt = '',
    [string]$Digest = $env:ZT_DIGEST,
    [string]$CitationKey = 'stiglitz2014learning'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$utf8 = [Text.UTF8Encoding]::new($false)
if (-not $Work) { $Work = Join-Path ([IO.Path]::GetTempPath()) "zt-recovery-rehearsal-$PID" }
$Work = [IO.Path]::GetFullPath($Work)
$allowed = $Work.StartsWith(([IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar), [StringComparison]::OrdinalIgnoreCase) -or
    $Work.StartsWith('D:\zt-target\', [StringComparison]::OrdinalIgnoreCase)
if (-not $allowed) { throw "Work must be under the temp folder or D:\zt-target (got $Work)" }
if (-not (Test-Path -LiteralPath $Archive)) { throw "archive not found: $Archive" }
if (-not (Test-Path -LiteralPath $Staging)) { throw "staging folder not found: $Staging" }
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$root = Join-Path $Work 'run'
$out = Join-Path $root 'staging'
$auditBefore = Join-Path $root 'audit-before'
$auditAfter = Join-Path $root 'audit-after'
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Digest([string[]]$arguments) {
    $lines = @(& $Digest @arguments 2>&1 | ForEach-Object { [string]$_ })
    $code = $LASTEXITCODE
    $last = @($lines | Where-Object { $_ -match '^\{' }) | Select-Object -Last 1
    [pscustomobject]@{ exit = $code; json = $(if ($last) { $last | ConvertFrom-Json -DateKind String } else { $null }); text = ($lines -join ' | ') }
}
function Status { (Digest @('ledger', 'status', '-Out', $out)).json }
function Verified { (Digest @('ledger', 'status', '-Out', $out, '-Verify')).json }
function Unit([string]$id) { (Status).units | Where-Object { $_.id -eq $id } }
function DoneFacts { (Status).units | Where-Object { $_.state -eq 'done' } | ForEach-Object { [pscustomobject]@{ unit = $_.id; attempts = $_.attempts; completed_at = $_.completed_at; result = ($_.result | ConvertTo-Json -Depth 30 -Compress) } } }
function FolderHashes([string]$path) {
    $files = Get-ChildItem -LiteralPath $path -Recurse -File | Where-Object { $_.FullName -notmatch '\\adoption\\' } | Sort-Object FullName
    $files | ForEach-Object { "$($_.FullName.Substring($path.Length))=$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash)" }
}
function Recount([string]$auditDir) {
    $report = Get-Content -LiteralPath (Join-Path $auditDir 'audit.json') -Raw | ConvertFrom-Json
    $book = @($report.books)[0]
    [pscustomobject]@{
        complete = $report.complete
        revision = $report.archive_revision
        cards = $book.cards
        style_faults = $book.style.faults
        style_by_rule = ($book.style.by_rule | ConvertTo-Json -Compress)
        r18 = [int]$book.style.by_rule.R18
        r19 = [int]$book.style.by_rule.R19
        layer_faults = $book.layer.faults
        orphans = $book.orphans.count
        unkinded = $book.unkinded.count
        no_source = $book.no_source.count
        models = $book.models.count
        derivations = $book.models.derivations
        explained = $book.models.explained
        figures_shown = $book.figures.shown_labels
        figures_missing_known = $book.figures.missing_known
        figures_missing_unknown = $book.figures.missing_unknown
        broken_links = $book.broken_links
    }
}

if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
New-Item -ItemType Directory -Force (Join-Path $root 'archive'), (Join-Path $root 'config') | Out-Null
$liveHashes = @(FolderHashes ([IO.Path]::GetFullPath($Staging)))
try {
    # 1. A consistent copy of the archive and one copy of the real legacy staging folder.
    $copyScript = Join-Path $root 'copy.mjs'
    [IO.File]::WriteAllText($copyScript, @'
import { DatabaseSync } from 'node:sqlite';
const db = new DatabaseSync(process.env.ZT_SRC, { readOnly: true });
db.exec(`VACUUM INTO '${process.env.ZT_DST}'`);
db.close();
const check = new DatabaseSync(process.env.ZT_DST, { readOnly: true });
const integrity = check.prepare('PRAGMA integrity_check').all().map(r => r.integrity_check);
const cards = check.prepare('SELECT count(*) AS n FROM cards').get().n;
const links = check.prepare('SELECT count(*) AS n FROM links').get().n;
const meta = Object.fromEntries(check.prepare('SELECT key, value FROM metadata').all().map(r => [r.key, r.value]));
check.close();
console.log(JSON.stringify({ integrity, cards, links, archive_id: meta.archive_id, archive_revision: meta.archive_revision }));
'@, $utf8)
    $env:ZT_SRC = (Resolve-Path -LiteralPath $Archive).Path
    $env:ZT_DST = (Join-Path $root 'archive\zt.sqlite3')
    $copy = (node --no-warnings $copyScript 2>$null | Select-Object -Last 1) | ConvertFrom-Json
    Assert ($copy.integrity -contains 'ok' -and $copy.cards -gt 0) "the copied archive is consistent ($($copy.integrity -join ','), $($copy.cards) Cards, $($copy.links) Links, revision $($copy.archive_revision))"
    New-Item -ItemType Directory -Force $out | Out-Null
    Copy-Item -Path (Join-Path $Staging '*') -Destination $out -Recurse -Force
    Assert (Test-Path (Join-Path $out 'sections\index.md')) 'the legacy preparation index reached the copied staging folder'
    $env:ZT_CONFIG_DIR = Join-Path $root 'config'
    $env:ZT_GUI_PORT = '0'
    zt config set archive_root (Join-Path $root 'archive') | Out-Null
    zt up | Out-Null

    # 2. The read-only recount: the plan first, then the card-level audit.
    $plan = Digest @('ledger', 'adopt', '-Out', $out, '-Variant', 'econ')
    Assert ($plan.exit -eq 0 -and $plan.json.status -eq 'ready' -and $plan.json.apply.allowed -eq $true) 'the read-only adoption plan is ready to apply'
    $incompleteGroups = @($plan.json.groups | Where-Object state -EQ 'incomplete' | ForEach-Object { [int]$_.n } | Sort-Object)
    Assert (@($plan.json.groups).Count -eq 171 -and $incompleteGroups.Count -eq 11) "every prepared group is recounted; 11 of 171 stay incomplete ($($incompleteGroups -join ','))"
    Assert (($incompleteGroups -join ',') -eq '10,18,92,93,94,95,127,163,168,169,170') 'the incomplete groups are the exact ones the plan names'
    $verifiedGroups = @($plan.json.groups | Where-Object state -EQ 'verified')
    Assert ($verifiedGroups.Count -eq 160 -and $verifiedGroups[0].substantive -gt 0) 'a verified group is recounted from substantive Cards, not from its skeleton anchor'
    $planModels = $plan.json.models | Group-Object state | ForEach-Object { "$($_.Name)=$($_.Count)" }
    Assert (@($plan.json.models).Count -eq 170 -and ($planModels -join ',') -eq 'incomplete=116,verified=54') "every Model Card is recounted as verified ($(($planModels) -join ', '))"
    Assert ($plan.json.proposal.adopted -eq 214 -and $plan.json.proposal.unresolved -eq 127) "the plan separates 214 adopted units from 127 unresolved obligations"
    Assert (@($plan.json.classification.unknown | Where-Object { $_.what -eq 'Model source review' }).Count -ge 1) 'a model without a retained source review is a named provenance gap, not a claimed review'
    Assert ($plan.json.notes.Count -eq 0) 'the retained folder holds no model plan, so no note suggestion is invented'
    $bridgeReport = Join-Path $out 'bridge\report.md'
    $bridgeSha = (Get-FileHash -LiteralPath $bridgeReport -Algorithm SHA256).Hash.ToLowerInvariant()
    $bridgeLine = @(Get-Content -LiteralPath $bridgeReport | Where-Object { $_ -match '^Status: done\. Counts:' })[0]
    $bridgeCounts = [regex]::Match($bridgeLine, '\{.*\}').Value | ConvertFrom-Json
    Assert ($plan.json.evidence.retained_outputs.bridge.report.sha256 -eq $bridgeSha) 'the retained Bridge report is recorded by its exact hash'
    Assert ($bridgeCounts.proposals -eq 57 -and $bridgeCounts.refused -eq 0) "the retained Bridge report names its open proposals ($($bridgeCounts.proposals))"
    Assert (@($plan.json.classification.unknown | Where-Object { $_.what -eq 'retained Bridge report' })[0].reason.Contains('no proposal without its explicit acceptance')) 'the Bridge proposals stay report-only, never applied by adoption'
    Assert (@($plan.json.derived).Count -ge 3 -and @($plan.json.derived | ForEach-Object { $_.artifact }) -contains 'events.json') 'missing artifacts are named as derived, never invented as retained'
    Assert (-not (Test-Path (Join-Path $out 'ledger.sqlite3')) -and -not (Test-Path (Join-Path $out 'manifest.json'))) 'the read-only plan writes no ledger and no prepared manifest'

    $beforeAudit = Digest @('audit', '-CitationKey', $CitationKey, '-Out', $auditBefore)
    Assert ($beforeAudit.exit -eq 0) 'the card-level recount runs read-only on the copy'
    $recount = Recount $auditBefore
    Assert ($recount.complete -eq $true -and $recount.cards -gt 0) "the recount is complete over the book's Cards ($($recount.cards))"
    Assert ($recount.r18 -ge 0 -and $recount.r19 -ge 0) "the recount counts repeated paragraphs (R18 $($recount.r18)) and repeated title lines (R19 $($recount.r19)) from the current Cards"
    Assert ($recount.orphans -ge 0 -and $recount.unkinded -ge 0 -and $recount.broken_links -eq 0) "the recount names the orphans ($($recount.orphans)), unkinded Links ($($recount.unkinded)) and broken Links ($($recount.broken_links))"
    Assert ($recount.explained -le $recount.derivations) "the explanation recount cannot exceed its derivations ($($recount.explained) of $($recount.derivations))"
    Assert ($recount.models -eq 170) 'the audit recounts the same 170 models the plan does'
    [IO.File]::WriteAllText((Join-Path $root 'recount.json'), ($recount | ConvertTo-Json -Depth 6), $utf8)

    # 3. The authorized apply, and the coverage the ledger can prove about itself.
    $applied = Digest @('ledger', 'adopt', '-Out', $out, '-Variant', 'econ', '-Apply')
    Assert ($applied.exit -eq 0 -and $applied.json.status -eq 'applied' -and $applied.json.kind -eq 'adoption_applied') 'the adoption writes the prepared ledger from the reviewed plan'
    $status = Status
    $verifiedStatus = Verified
    Assert ($status.archive_identity.id -eq $copy.archive_id -and $status.source.sha256 -eq (Get-FileHash -LiteralPath (Join-Path $out 'book.zh.md') -Algorithm SHA256).Hash.ToLowerInvariant()) 'the ledger binds the archive identity and the frozen source hash'
    Assert ($status.counts.done -eq 216 -and $status.counts.ready -eq 127 -and $status.counts.registered -eq 8) "the adopted ledger holds 216 done units beside 127 ready obligations ($($status.counts.done)/$($status.counts.ready)/$($status.counts.registered))"
    Assert ($verifiedStatus.completeness.complete -eq $false -and $verifiedStatus.completeness.quality_reason -eq 'placeholders_present') 'the verified run reports itself incomplete because placeholders remain'
    Assert (@($verifiedStatus.placeholders.models).Count -eq 170 -and @($verifiedStatus.placeholders.models | Where-Object { @($_.placeholders).Count -gt 0 }).Count -eq 42) 'the placeholder recount covers every model and names the 42 that still hold 待写'
    Assert ($verifiedStatus.placeholders.status -eq 'verified' -and $verifiedStatus.placeholders.count -eq 232) "the placeholder recount reads the archive at revision 0 and counts every placeholder Card ($($verifiedStatus.placeholders.count))"
    $mismatched = @($status.coverage_faults | Where-Object { $_.code -ne 'required_unit_missing_or_changed' })
    Assert ($mismatched.Count -eq 0) "only unbuilt publications may be coverage faults, not the adoption's own records ($(@($mismatched | ForEach-Object { $_.code }) -join ', '))"
    Assert (@($status.publications | Where-Object current).Count -eq 0 -and @($status.publications).Count -eq 7) 'the seven aggregate publications are registered and none claims to be current'
    Assert (@($status.units | Where-Object { $_.layer -eq 'bridge' -and $_.state -eq 'registered' }).Count -eq 1) 'the Bridge publication is an open obligation carrying the immutable report'

    # 4. The before manifest, the injected failure and the one supported resume.
    $before = @(DoneFacts)
    $doneGroups = @($before | Where-Object { $_.unit -like 'digest/*' }).Count
    Assert ($doneGroups -eq 160 -and $before.Count -eq 216) "the before manifest records every adopted unit ($doneGroups groups of $($before.Count) done units)"
    Assert ((Status).resume.available -eq $true -and (Status).resume_count -eq 0) 'the adopted run reports its one supported resume and has spent none'
    $claim = (Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'recovery-rehearsal')).json.units[0]
    Assert ($claim.id -eq 'digest/g010@1' -and $claim.token -ge 1) "the run's next ready group is claimed with its own token ($($claim.id), token $($claim.token))"
    $bad = Digest @('ledger', 'fail', '-Out', $out, '-Unit', $claim.id, '-Token', "$($claim.token)", '-Class', 'retryable', '-Fault', ('{"schema_version":1,"unit":"' + $claim.id + '","input_hash":"' + $claim.input_hash + '","code":"check_failed","cause":"recovery rehearsal"}'))
    Assert ($bad.exit -ne 0 -and $bad.text -match 'fault classification mismatch') 'a fault class the code does not permit is refused before any state changes'
    $fault = '{"schema_version":1,"unit":"digest/g010@1","input_hash":"' + $claim.input_hash + '","code":"check_failed","cause":"recovery rehearsal: the group fails once so the run reaches its one resume"}'
    $failedOnce = Digest @('ledger', 'fail', '-Out', $out, '-Unit', $claim.id, '-Token', "$($claim.token)", '-Class', 'content', '-Fault', $fault)
    Assert ($failedOnce.exit -eq 0 -and $failedOnce.json.state -eq 'failed') 'the content failure is recorded and the group stays failed until the resume'
    $resume = Digest @('ledger', 'resume', '-Out', $out)
    Assert ($resume.exit -eq 0 -and $resume.json.status -eq 'resumed') 'the one authorized resume runs'
    Assert ($resume.json.invocation -eq 2 -and $resume.json.resume_count -eq 1) "the resume opens invocation 2 and spends the one resume (invocation $($resume.json.invocation), resume_count $($resume.json.resume_count))"
    Assert (@($resume.json.reset | ForEach-Object { $_.unit }) -contains 'digest/g010@1') 'the resume resets exactly the failed group'
    Assert (@($resume.json.closure.downstream).Count -ge 100) "the resume publishes the failed group's downstream closure ($(@($resume.json.closure.downstream).Count) units)"
    $after = @(DoneFacts)
    $changed = @(Compare-Object -ReferenceObject $before -DifferenceObject $after -Property unit, attempts, completed_at, result)
    Assert ($changed.Count -eq 0 -and $after.Count -eq 216) "every adopted unit keeps its result, attempts and completion time across the resume ($($changed.Count) changes)"
    Assert (((Unit 'digest/g010@1').state) -eq 'ready' -and (Unit 'digest/g010@1').attempts -eq 0) 'the reset group is ready for its next attempt with its attempt budget reset'
    $second = Digest @('ledger', 'resume', '-Out', $out)
    Assert ($second.exit -ne 0 -and $second.text -match 'resume_used') 'a second resume is refused as resume_used with nothing spent'
    Assert ((Status).resume_count -eq 1) 'the refused second resume leaves resume_count at 1'

    # 5. The report, the unchanged recount and the archive's own check.
    $report = Digest @('ledger', 'report', '-Out', $out)
    Assert ($report.exit -eq 0 -and $report.json.completeness.complete -eq $false -and @($report.json.gaps).Count -ge 1) "the run report names its gaps instead of claiming completion ($(@($report.json.gaps).Count) gaps)"
    Assert ($report.json.identity.resume_count -eq 1 -and $report.json.identity.invocation -eq 2) 'the report records the spent resume as its own invocation'
    Assert (@($report.json.cleanup.unresolved).Count -ge 127 -and @($report.json.cleanup.unresolved) -contains 'digest/g010@1') "the report keeps every unresolved unit, the reset group included ($(@($report.json.cleanup.unresolved).Count) units)"
    $bridgeReportAfter = (Get-FileHash -LiteralPath $bridgeReport -Algorithm SHA256).Hash.ToLowerInvariant()
    Assert ($bridgeReportAfter -eq $bridgeSha) 'the Bridge report is byte-identical: no proposal was applied by the resume'
    $afterAudit = Digest @('audit', '-CitationKey', $CitationKey, '-Out', $auditAfter)
    Assert ($afterAudit.exit -eq 0) 'the after recount runs on the same copy'
    $again = Recount $auditAfter
    $same = @(Compare-Object -ReferenceObject $recount -DifferenceObject $again -Property r18, r19, orphans, unkinded, no_source, derivations, explained, figures_shown, figures_missing_known, figures_missing_unknown, broken_links)
    Assert ($same.Count -eq 0) "the resume changed no card-level category ($($same.Count) differences)"
    $links = (zt status --check-links 2>&1 | Out-String) | ConvertFrom-Json
    Assert ($links.ok -eq $true -and @($links.faults).Count -eq 0) "the copy still passes the full-parser Link check ($($links.cards) Cards, $($links.links) Links, revision $($links.archive_revision))"
    $liveAfter = @(FolderHashes ([IO.Path]::GetFullPath($Staging)))
    Assert ((($liveAfter -join "`n") -ceq ($liveHashes -join "`n")) -and -not (Test-Path (Join-Path $Staging 'adoption'))) 'the live staging folder was not mutated by the rehearsal'

    # The retained before/after manifest: what was adopted, what was spent, and what remains.
    $manifest = [ordered]@{
        schema_version = 1
        kind = 'recovery_rehearsal'
        ran_at = (Get-Date).ToUniversalTime().ToString('o')
        tool = (& $Digest version)
        archive = [ordered]@{ id = $copy.archive_id; path = $Archive; revision = $copy.archive_revision; cards = $copy.cards; links = $copy.links }
        source = [ordered]@{ path = (Join-Path $out 'book.zh.md'); sha256 = $status.source.sha256 }
        index_sha256 = $plan.json.evidence.index_sha256
        bridge = [ordered]@{ report = $bridgeSha; proposals = $bridgeCounts.proposals; accepted = 0; state = 'report_only' }
        before = [ordered]@{
            groups = [ordered]@{ verified = 160; incomplete = $incompleteGroups }
            models = [ordered]@{ verified = 54; incomplete = 116; with_placeholders = 42 }
            adopted_units = 214
            unresolved = 127
            done_units = $before
        }
        recount_before = $recount
        recount_after = $again
        resume = [ordered]@{ invocation = $resume.json.invocation; resume_count = $resume.json.resume_count; reset = @($resume.json.reset | ForEach-Object { $_.unit }); downstream = @($resume.json.closure.downstream).Count }
        after = [ordered]@{ done_units = $after; unresolved = @($report.json.cleanup.unresolved) }
        gaps = @($report.json.gaps).Count
        complete = $report.json.completeness.complete
    }
    [IO.File]::WriteAllText((Join-Path $Work 'before-after.json'), ($manifest | ConvertTo-Json -Depth 20), $utf8)
    Write-Output "rehearsal work: $Work"
} finally {
    zt down 2>&1 | Out-Null
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
