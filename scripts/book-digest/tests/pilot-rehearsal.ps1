#Requires -Version 7.5
<#
.SYNOPSIS
  Rehearse the #90 pilot's ledger commands on a consistent copy of the live archive.

.DESCRIPTION
  docs/digest-pilot-90.md section 7 hands the one authorized pilot run to the human. This harness
  proves every command of that runbook except the provider work itself, on a consistent copy of the
  archive, with no provider call and no live Card write:

    1. Copies -Archive with `VACUUM INTO` (the archive is in WAL mode) and prepares -Source with the
       installed zt-digest (`prepare -Variant econ -Apply`), so the rehearsal runs on a real prepared
       run of the same bounded source.
    2. Claims the run's next ready unit with its own `ledger next` and injects the runbook's failure
       (`ledger fail -Class content -Fault <file>`), and proves that a class the fault code does not
       permit is refused before any state changes.
    3. Runs the one supported `ledger resume` and asserts invocation 2, `resume_count` 1, the reset
       unit, the published downstream closure, the unit's ready state for the next attempt, and that
       the done neighbour keeps its result, attempts and completion time.
    4. Asserts a second resume is refused as `resume_used` with nothing spent, that the run report
       records the invocation and the resume and names its gaps instead of claiming completion, and
       that the copy still passes the full-parser Link check.

  The runbook's step 2 uses the digest-group layer; this rehearsal injects on the first layer the run
  claims after prepare (`terms`) because the digest groups are not yet ready without a provider run.
  The command shape, the fault file, the failure transition and the resume are the same ones.

  It exits 1 when any assertion fails, prints one line per assertion, and prints `failed: N` last.

  pwsh -NoProfile -File scripts\book-digest\tests\pilot-rehearsal.ps1 `
      -Archive D:\zt\zt.sqlite3 -Work D:\zt-target\qualification\pilot-rehearsal
#>
[CmdletBinding()]
param(
    [string]$Archive = 'D:\zt\zt.sqlite3',
    [string]$Source = '',
    [string]$Work = '',
    [string]$Zt = '',
    [string]$Digest = $env:ZT_DIGEST
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$utf8 = [Text.UTF8Encoding]::new($false)
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
if (-not $Source) { $Source = Join-Path $repo 'docs\digest-pilot-short-source.zh.md' }
if (-not $Work) { $Work = Join-Path ([IO.Path]::GetTempPath()) "zt-pilot-rehearsal-$PID" }
$Work = [IO.Path]::GetFullPath($Work)
$allowed = $Work.StartsWith(([IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar), [StringComparison]::OrdinalIgnoreCase) -or
    $Work.StartsWith('D:\zt-target\', [StringComparison]::OrdinalIgnoreCase)
if (-not $allowed) { throw "Work must be under the temp folder or D:\zt-target (got $Work)" }
if (-not (Test-Path -LiteralPath $Archive)) { throw "archive not found: $Archive" }
if (-not (Test-Path -LiteralPath $Source)) { throw "source not found: $Source" }
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$root = Join-Path $Work 'run'
$out = Join-Path $root 'out'
$faultPath = Join-Path $root 'pilot-fault.json'
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Digest([string[]]$arguments) {
    $lines = @(& $Digest @arguments 2>&1 | ForEach-Object { [string]$_ })
    $code = $LASTEXITCODE
    $last = @($lines | Where-Object { $_ -match '^\{' }) | Select-Object -Last 1
    [pscustomobject]@{ exit = $code; json = $(if ($last) { $last | ConvertFrom-Json -DateKind String } else { $null }); text = ($lines -join ' | ') }
}
function Status { (Digest @('ledger', 'status', '-Out', $out, '-Verify')).json }
function Unit([string]$id) { (Status).units | Where-Object { $_.id -eq $id } }
function DoneFacts { (Status).units | Where-Object { $_.state -eq 'done' } | ForEach-Object { [pscustomobject]@{ unit = $_.id; attempts = $_.attempts; completed_at = $_.completed_at; result = ($_.result | ConvertTo-Json -Depth 20 -Compress) } } }

if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
New-Item -ItemType Directory -Force (Join-Path $root 'archive'), (Join-Path $root 'config') | Out-Null
try {
    # 1. A consistent copy of the archive and one real prepared run on it.
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
    $copy = (node $copyScript 2>$null | Select-Object -Last 1) | ConvertFrom-Json
    Assert ($copy.integrity -contains 'ok' -and $copy.cards -gt 0) "the copy is consistent ($($copy.integrity -join ','), $($copy.cards) Cards, $($copy.links) Links)"
    $env:ZT_CONFIG_DIR = Join-Path $root 'config'
    $env:ZT_GUI_PORT = '0'
    zt config set archive_root (Join-Path $root 'archive') | Out-Null
    zt up | Out-Null
    $prep = Digest @('prepare', '-Variant', 'econ', '-Source', $Source, '-Out', $out, '-CitationKey', 'Pilot2026',
        '-BookTitle', 'Release qualification book', '-BibTeX', '@book{Pilot2026,title={Release qualification book},author={Qualification, Ada},year={2026}}',
        '-MinUnitChars', '200', '-MaxUnits', '2', '-NoFigures', '-Apply')
    $prepared = $prep.exit -eq 0 -and $null -ne $prep.json -and @($prep.json.groups).Count -eq 3
    $groupCount = if ($prep.json) { @($prep.json.groups).Count } else { 0 }
    $sourceShown = if ($prep.json -and $prep.json.source_sha256) { $prep.json.source_sha256.Substring(0, 12) } else { 'none' }
    Assert $prepared "prepare binds the bounded source to the copy ($groupCount groups, source $sourceShown)"
    if (-not $prepared) {
        Assert $false "the rehearsal stopped at prepare, so the claim, failure, resume and report checks did not run ($($prep.text))"
        throw 'pilot rehearsal stopped at prepare'
    }
    $before = @(DoneFacts)
    Assert ($before.Count -eq 1 -and $before[0].unit -eq 'prepare/book@1') "the prepared run holds exactly one done unit ($(@($before).Count))"
    Assert ((Status).resume.available -eq $true -and (Status).resume_count -eq 0) 'the prepared run reports its supported resume and has spent none'

    # 2. The claim, the refused mismatched class and the injected content failure.
    $claim = (Digest @('ledger', 'next', '-Out', $out, '-Layer', 'terms', '-Take', '1', '-Owner', 'pilot-rehearsal')).json.units[0]
    Assert ($claim.id -eq 'terms/book@1' -and $claim.token -ge 1) "ledger next claims the run's next ready unit with its own token ($($claim.id), token $($claim.token))"
    $fault = [ordered]@{ schema_version = 1; unit = $claim.id; input_hash = $claim.input_hash; code = 'check_failed'; cause = 'pilot rehearsal: the unit fails once so the run reaches its one resume' }
    [IO.File]::WriteAllText($faultPath, ($fault | ConvertTo-Json -Depth 6), $utf8)
    $bad = Digest @('ledger', 'fail', '-Out', $out, '-Unit', $claim.id, '-Token', "$($claim.token)", '-Class', 'retryable', '-Fault', $faultPath)
    Assert ($bad.exit -ne 0 -and $bad.text -match 'fault classification mismatch') 'a class that the fault code does not permit is refused before any change'
    $failedOnce = Digest @('ledger', 'fail', '-Out', $out, '-Unit', $claim.id, '-Token', "$($claim.token)", '-Class', 'content', '-Fault', $faultPath)
    Assert ($failedOnce.exit -eq 0 -and $failedOnce.json.state -eq 'failed') "the content failure is recorded and the unit stays failed until the resume ($($failedOnce.json.state))"
    Assert ((Unit 'terms/book@1').attempts -eq 1) 'the failed unit holds one attempt'
    Assert (@((Status).unresolved).Count -ge 4) "the failure leaves the unit and its downstream work unresolved ($(@((Status).unresolved).Count) units)"

    # 3. The one supported resume.
    $resume = Digest @('ledger', 'resume', '-Out', $out)
    Assert ($resume.exit -eq 0 -and $resume.json.status -eq 'resumed') "the one resume runs ($($resume.json.status))"
    Assert ($resume.json.invocation -eq 2 -and $resume.json.resume_count -eq 1) "the resume opens invocation 2 and spends the one resume (invocation $($resume.json.invocation), resume_count $($resume.json.resume_count))"
    Assert (@($resume.json.reset | ForEach-Object { $_.unit }) -contains 'terms/book@1') "the resume resets the failed unit ($(@($resume.json.reset | ForEach-Object { $_.unit }) -join ', '))"
    Assert (@($resume.json.closure.downstream | Where-Object { $_ -like 'digest/*' }).Count -eq 3) "the resume publishes the failed unit's downstream closure ($(@($resume.json.closure.downstream | Where-Object { $_ -like 'digest/*' }) -join ', '))"
    $after = @(DoneFacts)
    $changed = @(Compare-Object -ReferenceObject $before -DifferenceObject $after -Property unit, attempts, completed_at, result)
    Assert ($changed.Count -eq 0) "the done neighbour keeps its result, attempts and completion time across the resume ($($changed.Count) changes)"
    Assert ((Unit 'terms/book@1').state -eq 'ready') 'the reset unit is ready for the run to claim it again'

    # 4. The refused second resume, the report and the archive's own check.
    $second = Digest @('ledger', 'resume', '-Out', $out)
    Assert ($second.exit -ne 0 -and $second.text -match 'resume_used') 'a second resume is refused as resume_used with nothing spent'
    Assert ((Status).resume_count -eq 1) 'the refused second resume leaves resume_count at 1'
    $links = (zt status --check-links 2>&1 | Out-String) | ConvertFrom-Json
    Assert ($links.ok -eq $true -and @($links.faults).Count -eq 0) "the copy still passes the full-parser Link check ($($links.cards) Cards, revision $($links.archive_revision))"
    $report = (Digest @('ledger', 'report', '-Out', $out)).json
    Assert ($report.identity.invocation -eq 2 -and $report.identity.resume_count -eq 1) 'the run report records invocation 2 and the spent resume'
    Assert (@($report.gaps).Count -ge 1 -and $report.completeness.complete -eq $false) "the report names its remaining gaps instead of claiming completion ($(@($report.gaps).Count) gaps)"
    Write-Output "rehearsal work: $root"
} finally {
    zt down 2>&1 | Out-Null
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
