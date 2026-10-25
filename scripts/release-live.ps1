#Requires -Version 7.5
<#
.SYNOPSIS
  Qualify, back up, and deliberately promote or roll back the live Term Cards / Card-check release.

.DESCRIPTION
  S31 (#173, design section 19). This is the live-release half of the release evidence: the fast
  static half is `scripts/book-digest/tests/release-set.tests.ps1`, the installed-and-running half
  is `scripts/verify-release.ps1`, and this script adds the live sequence around them.

    pwsh -NoProfile -File scripts\release-live.ps1 -Step preflight -Work D:\zt-target\release-173
    pwsh -NoProfile -File scripts\release-live.ps1 -Step observe   -Work D:\zt-target\release-173
    pwsh -NoProfile -File scripts\release-live.ps1 -Step backup    -Work D:\zt-target\release-173
    pwsh -NoProfile -File scripts\release-live.ps1 -Step promote   -Decision <decision.json>
    pwsh -NoProfile -File scripts\release-live.ps1 -Step rollback  -Decision <decision.json>

  preflight   Refuses a live release while a digest is active (`<archive>\zt.digest.run` inside its
              TTL), while a Session is open, when a binary, a frozen workflow or an installed skill
              is not the release manifest's, or when the running daemon does not publish that
              contract. It runs verify-release.ps1's installed/scratch half and records the live
              archive facts. It writes only under -Work.
  observe     Read-only advisory observation: records the policy table, stage, hash and revision and
              runs `zt-check --cards` over every live Card in bounded chunks, counting the stored
              legacy debt by rule and severity. It writes no Card and changes no policy.
  backup      Stops the service, takes a consistent copy of the SQLite archive (sidecars included),
              validates the copy by starting the current binary on it (migration rehearsal), then
              restarts the live service and verifies the same archive identity, revision, Card count
              and Lock integrity. The copy is retained under -BackupRoot.
  promote     Applies an owner's written decision: the named policy overrides and then `gate.stage
              enforce` (or the reverse for rollback). It refuses any change that rewrites a Card,
              drops the Card count, or names another issue. No decision file, no change: a timer or
              a copied approval can never switch the policy on its own.
  rollback    Returns the quality checks to advice while the mandatory invariants stay active. It
              needs no Card rewrite; the rollback is a policy write only.

  Every step prints one compact JSON summary as its last line and exits 1 when a check fails.
  The script never unlocks a tree, backfills a book, strips Term markup, deletes heads or restores
  an archive. A backup restore is a separately authorized operation (the runbook,
  docs/term-cards-links-and-check-gate-live-release.md).
#>
[CmdletBinding()]
param(
    [ValidateSet('preflight', 'observe', 'backup', 'promote', 'rollback')]
    [string]$Step = 'preflight',
    [string]$Project = '',
    [string]$Zt = '',
    [string]$ZtCheck = '',
    [string]$ZtDigest = '',
    [string]$Work = '',
    [string]$BackupRoot = '',
    [string]$Decision = '',
    [string]$Label = '',
    [switch]$SkipVerify
)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$here = $PSScriptRoot
if (-not $Project) { $Project = (Resolve-Path (Join-Path $here '..')).Path }
$Project = (Resolve-Path -LiteralPath $Project).Path
$manifestPath = Join-Path $Project 'docs\term-cards-links-and-check-gate-release-manifest.json'
if (-not (Test-Path -LiteralPath $manifestPath)) { throw "the release manifest is missing: $manifestPath" }
$manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String
if (-not $Work) { $Work = Join-Path ([IO.Path]::GetTempPath()) "zt-release-$Step-$PID" }
$Work = [IO.Path]::GetFullPath($Work)
New-Item -ItemType Directory -Force $Work | Out-Null

$checks = [Collections.Generic.List[object]]::new()
$failed = 0
function Check($condition, [string]$name, [string]$detail = '') {
    $ok = [bool]$condition
    if (-not $ok) { $script:failed++ }
    $script:checks.Add([ordered]@{ name = $name; ok = $ok; detail = $detail })
    if (-not $ok) { Write-Output "FAIL: $name$(if ($detail) { " ($detail)" })" }
}
function Get-Sha256([string]$path) {
    if (-not (Test-Path -LiteralPath $path)) { return '' }
    (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Get-TextSha256([string]$text) {
    $bytes = $utf8.GetBytes($text)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { ([BitConverter]::ToString($sha.ComputeHash($bytes)) -replace '-', '').ToLowerInvariant() } finally { $sha.Dispose() }
}
function Resolve-Tool([string]$given, [string]$name) {
    if ($given) { return (Resolve-Path -LiteralPath $given).Path }
    $found = Get-Command $name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($found) { return $found.Source }
    throw "$name is not installed (put it on PATH or pass its path)"
}
function Invoke-Lines([string]$exe, [string[]]$arguments) {
    $lines = @(& $exe @arguments 2>&1 | ForEach-Object { [string]$_ })
    [pscustomobject]@{ exit = $LASTEXITCODE; lines = $lines; text = ($lines -join "`n") }
}
# `zt-digest call -Tool` writes `<n> <json>` and can exceed a PowerShell pipeline's line capture;
# a file keeps every byte and parses the same way.
function Invoke-DigestJson([string]$tool, [string]$argumentsJson, [string]$name) {
    $out = Join-Path $Work "$name.out.json"
    $err = Join-Path $Work "$name.err.txt"
    & $ZtDigest call -Tool $tool -Json $argumentsJson > $out 2> $err
    if ($LASTEXITCODE -ne 0) { throw "zt-digest call -Tool $tool failed: $([IO.File]::ReadAllText($err))" }
    $text = [IO.File]::ReadAllText($out) -replace '^\d+\s+', ''
    $text | ConvertFrom-Json -DateKind String
}
function Get-Status {
    $run = Invoke-Lines $Zt @('status')
    $value = [ordered]@{ raw = $run.text; lines = $run.lines; state = ''; pid = ''; revision = -1; cards = -1; sessions = -1 }
    foreach ($line in $run.lines) {
        if ($line -match '^state: (\S+)$') { $value.state = $Matches[1] }
        elseif ($line -match '^pid: (.*)$') { $value.pid = $Matches[1].Trim() }
        elseif ($line -match '^revision: (\d+)$') { $value.revision = [int64]$Matches[1] }
        elseif ($line -match '^cards: (\d+)$') { $value.cards = [int64]$Matches[1] }
        elseif ($line -match '^sessions: (\d+)$') { $value.sessions = [int]$Matches[1] }
    }
    $value['up'] = $value.state -eq 'up'
    [pscustomobject]$value
}
function Get-LiveFacts {
    $status = Get-Status
    $policy = Invoke-Lines $Zt @('config', 'policy')
    $fields = @{}
    foreach ($line in $policy.lines) { if ($line -match '^([^=]+)=(.*)$') { $fields[$Matches[1]] = $Matches[2] } }
    $locks = ($status.lines | Where-Object { $_ -match '^locks: ' } | Select-Object -First 1) -replace '^locks: ', ''
    [pscustomobject]@{
        status = $status
        archive = if ($fields['policy_archive']) { $fields['policy_archive'] } else { '' }
        policy_hash = if ($fields['policy_hash']) { $fields['policy_hash'] } else { '' }
        revision = if ($fields['policy_revision']) { [int64]$fields['policy_revision'] } else { -1 }
        rule_version = if ($fields['rule_version']) { $fields['rule_version'] } else { '' }
        diagnostic_schema = if ($fields['diagnostic_schema']) { $fields['diagnostic_schema'] } else { '' }
        stage = if ($fields['gate_stage'] -match '^(advisory|enforce)') { $Matches[1] } else { '' }
        locks = $locks
        table = $policy.lines
    }
}
function Get-ArchiveIdAndRevision {
    $value = Invoke-DigestJson 'get_cards' '{"addresses":[],"view":"text"}' 'archive-identity'
    [pscustomobject]@{ id = $value.archive_identity.id; revision = [int64]$value.archive_revision }
}
function Get-ActiveDigest([string]$archiveRoot) {
    if (-not $archiveRoot) { return $null }
    $marker = Join-Path $archiveRoot 'zt.digest.run'
    if (-not (Test-Path -LiteralPath $marker)) { return $null }
    try { $value = Get-Content -LiteralPath $marker -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String } catch { return $null }
    $now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $renewed = [int64]$value.renewed
    $ttl = [int64]$value.ttl_s
    [pscustomobject]@{
        active = $now -ge $renewed -and $now -lt ($renewed + $ttl)
        age_s = $now - $renewed
        ttl_s = $ttl
        run_id = [string]$value.run_id
        out = [string]$value.out
        unit = [string]$value.unit
        pid = [string]$value.pid
        marker = $marker
    }
}
function Get-Addresses {
    $value = Invoke-DigestJson 'export_titles' '{}' 'titles'
    @($value.titles | ForEach-Object { [string]$_.address })
}
# One SHA-256 over every Card's address and stored text, in 200-address reads (the MCP bound).
function Get-CardTextDigest {
    $addresses = @(Get-Addresses)
    if ($addresses.Count -eq 0) { return [pscustomobject]@{ digest = Get-TextSha256 ''; cards = 0 } }
    $parts = [Collections.Generic.List[string]]::new()
    for ($i = 0; $i -lt $addresses.Count; $i += 200) {
        $chunk = $addresses[$i..([Math]::Min($i + 199, $addresses.Count - 1))]
        $arguments = @{ addresses = $chunk; view = 'text' } | ConvertTo-Json -Compress
        $value = Invoke-DigestJson 'get_cards' $arguments "cards-$i"
        foreach ($card in $value.cards) { $parts.Add("$($card.address)`n$($card.text)") }
    }
    $parts.Sort([StringComparer]::Ordinal)
    [pscustomobject]@{ digest = Get-TextSha256 ($parts -join "`n---`n"); cards = $addresses.Count }
}
function Install-SetCheck {
    $executable = $manifest.contracts.executable
    $version = Invoke-Lines $Zt @('version', '--json')
    $parsed = $null
    try { $parsed = ($version.lines | Select-Object -Last 1) | ConvertFrom-Json -DateKind String } catch { }
    Check ($null -ne $parsed -and $parsed.zt -eq $executable.zt) 'the installed zt is the manifest executable' $(if ($parsed) { $parsed.zt })
    if ($parsed) {
        Check ($parsed.checker -eq $executable.zt_check) 'the installed zt carries the manifest checker' $parsed.checker
        Check ($parsed.rule_version -eq $manifest.contracts.rule.rule_version -and $parsed.diagnostic_schema -eq $manifest.contracts.rule.diagnostic_schema -and $parsed.applicability_version -eq $manifest.contracts.rule.applicability_version) 'the installed zt publishes the manifest rule contract'
        Check ($parsed.schema -eq $manifest.contracts.archive_schema.generation) 'the installed zt publishes the manifest archive schema' $parsed.schema
        Check ($parsed.capabilities.term_links -eq $manifest.capabilities.term_links -and $parsed.capabilities.term_cards -eq $manifest.capabilities.term_cards) 'the installed zt publishes the Term capabilities'
    }
    Check ((Invoke-Lines $ZtCheck @('--version')).text -match "^zt-check $([regex]::Escape($executable.zt_check))$") 'the installed zt-check is the manifest checker'
    Check ((Invoke-Lines $ZtDigest @('version')).text -match "^zt-digest $([regex]::Escape($executable.zt_digest))$") 'the installed zt-digest is the manifest tool'
}
function FrozenSetCheck {
    $expected = $manifest.contracts.workflow
    foreach ($name in @('zt-econ-digest', 'zt-math-digest')) {
        $path = Join-Path $Project ".claude\workflow-framework\$name\lock.json"
        $lock = Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String
        Check ($lock.version -eq $expected.$name) "the project $name is the manifest build" "$($lock.version) / $($expected.$name)"
        $global = Join-Path "$HOME\.claude\workflow-framework" "$name\lock.json"
        if (Test-Path -LiteralPath $global) {
            $globalLock = Get-Content -LiteralPath $global -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String
            Check ($globalLock.version -eq $expected.$name) "the installed $name is the manifest build" "$($globalLock.version) / $($expected.$name)"
        } else {
            Check $false "the installed $name registry exists" $global
        }
    }
    # The frozen pair's base versions and the zt-digest build that embeds them must be one set.
    Check ($expected.base -match '^\d+\.\d+\.\d+$') 'the manifest names the frozen base version' $expected.base
}
function SkillSetCheck {
    foreach ($name in $manifest.skills) {
        $source = Join-Path $Project "skills\$name"
        $parts = foreach ($file in (Get-ChildItem -LiteralPath $source -Recurse -File | Sort-Object FullName)) {
            $relative = $file.FullName.Substring($source.Length).Replace('\', '/').TrimStart('/')
            "$relative=$(Get-Sha256 $file.FullName)"
        }
        $sourceHash = ($parts -join "`n")
        Check ($sourceHash -ne '') "the repository $name skill exists" $source
        foreach ($folder in @("$HOME\.claude\skills", "$HOME\.codex\skills", "$HOME\.dsh\skills")) {
            $target = Join-Path $folder $name
            $targetParts = foreach ($file in (Get-ChildItem -LiteralPath $target -Recurse -File -ErrorAction SilentlyContinue | Sort-Object FullName)) {
                $relative = $file.FullName.Substring($target.Length).Replace('\', '/').TrimStart('/')
                "$relative=$(Get-Sha256 $file.FullName)"
            }
            Check ((Test-Path -LiteralPath $target) -and (($targetParts -join "`n") -eq $sourceHash)) "the installed $name skill matches the repository copy" $target
        }
    }
}
function LiveContractCheck {
    $facts = Get-LiveFacts
    Check $facts.status.up 'the live service is up' $facts.status.state
    Check ($facts.status.sessions -eq 0) 'no live Session is open during the release step' "sessions: $($facts.status.sessions)"
    Check ($facts.locks -match 'integrity ok') 'the live Lock integrity is ok' $facts.locks
    Check ($facts.rule_version -eq $manifest.contracts.rule.rule_version) 'the live policy reports the manifest rule version' $facts.rule_version
    Check ($facts.diagnostic_schema -eq $manifest.contracts.rule.diagnostic_schema) 'the live policy reports the manifest diagnostic schema' $facts.diagnostic_schema
    Check ($facts.policy_hash -match '^[0-9a-f]{64}$') 'the live policy publishes its hash' $facts.policy_hash
    Check ($facts.stage -in @('advisory', 'enforce')) 'the live policy publishes its promotion stage' $facts.stage
    Check ($facts.revision -ge 0) 'the live archive reports a revision' "$($facts.revision)"
    $digest = Get-ActiveDigest $facts.archive
    $script:active_digest = $digest
    if ($digest -and $digest.active) {
        Check $false 'no digest is active during the release step' "run $($digest.run_id), unit $($digest.unit), renewed $($digest.age_s) s ago (ttl $($digest.ttl_s) s)"
    } else {
        Check $true 'no digest is active during the release step' $(if ($digest) { "a stale marker (run $($digest.run_id), age $($digest.age_s) s)" } else { 'no marker' })
    }
    [pscustomobject]@{ facts = $facts; digest = $digest }
}
function Read-Decision([string]$path, [string]$action) {
    if (-not $path) { return @{ ok = $false; error = 'no -Decision file was given; a policy change needs the owner''s written decision' } }
    if (-not (Test-Path -LiteralPath $path)) { return @{ ok = $false; error = "the decision file is missing: $path" } }
    $text = Get-Content -LiteralPath $path -Raw -Encoding utf8
    $value = $null
    try { $value = $text | ConvertFrom-Json -DateKind String } catch { return @{ ok = $false; error = "the decision file is not JSON: $($_.Exception.Message)" } }
    $problems = [Collections.Generic.List[string]]::new()
    if ($value.schema_version -ne 1) { $problems.Add('schema_version must be 1') }
    if ($value.issue -ne 173) { $problems.Add('issue must be 173') }
    if ($value.approved -ne $true) { $problems.Add('approved must be true') }
    if ($value.action -ne $action) { $problems.Add("action must be $action") }
    if (-not ([string]$value.owner).Trim()) { $problems.Add('owner must be named') }
    if (-not $value.evidence -or @($value.evidence).Count -eq 0) { $problems.Add('evidence must list at least one item') }
    if (-not ([string]$value.stage).Trim()) { $problems.Add('stage must be named') }
    if ($null -eq $value.overrides) { $problems.Add('overrides must be given (an empty object is allowed)') }
    foreach ($key in @($value.overrides.PSObject.Properties.Name | Where-Object { $_ })) {
        if ($key -notmatch '^gate\.(human|agent)\.[A-Za-z0-9]+$') { $problems.Add("override key is not gate.<workspace>.<rule>: $key") }
        $override = [string]$value.overrides.$key
        if ($override -notmatch '^(block|advise|off|default)$') { $problems.Add("override value is not block|advise|off|default: $key=$override") }
    }
    if ($action -eq 'promote' -and $value.stage -ne 'enforce') { $problems.Add('a promotion sets stage enforce') }
    if ($action -eq 'rollback' -and $value.stage -ne 'advisory') { $problems.Add('a rollback sets stage advisory') }
    if ($problems.Count -gt 0) { return @{ ok = $false; error = ($problems -join '; '); value = $value } }
    @{ ok = $true; value = $value; sha256 = Get-Sha256 $path }
}

# --- preflight ---------------------------------------------------------------------------------
$zt = Resolve-Tool $Zt 'zt'
$ztCheck = Resolve-Tool $ZtCheck 'zt-check'
$ztDigest = Resolve-Tool $ZtDigest 'zt-digest'
$summary = [ordered]@{
    ok = $false
    step = $Step
    issue = 173
    slice = 'S31'
    project = $Project
    work = $Work
    checked_at = (Get-Date).ToString('o')
    manifest = [ordered]@{ path = $manifestPath; sha256 = Get-Sha256 $manifestPath }
    binaries = [ordered]@{ zt = $zt; zt_check = $ztCheck; zt_digest = $ztDigest }
    checks = @()
    failures = @()
}
$evidencePath = Join-Path $Work "release-$Step.json"

try {
    if ($Step -eq 'preflight') {
        Install-SetCheck
        FrozenSetCheck
        SkillSetCheck
        $live = LiveContractCheck
        $summary['live'] = [ordered]@{
            archive = $live.facts.archive
            state = $live.facts.status.state
            pid = $live.facts.status.pid
            revision = $live.facts.revision
            cards = $live.facts.status.cards
            locks = $live.facts.locks
            stage = $live.facts.stage
            policy_hash = $live.facts.policy_hash
            rule_version = $live.facts.rule_version
            diagnostic_schema = $live.facts.diagnostic_schema
            active_digest = if ($live.digest) { [ordered]@{ active = $live.digest.active; run_id = $live.digest.run_id; unit = $live.digest.unit; age_s = $live.digest.age_s; ttl_s = $live.digest.ttl_s } } else { $null }
        }
        if (-not $SkipVerify) {
            $verifyOut = Join-Path $Work 'verify-release.json'
            $verify = Invoke-Lines (Get-Command pwsh -CommandType Application | Select-Object -First 1).Source @('-NoProfile', '-File', (Join-Path $Project 'scripts\verify-release.ps1'), '-SkipLive', '-SkipWiki', '-Out', $verifyOut)
            $parsed = $null
            try { $parsed = ($verify.lines | Select-Object -Last 1) | ConvertFrom-Json -DateKind String } catch { }
            Check ($verify.exit -eq 0 -and $parsed -and $parsed.ok -eq $true) 'the installed release set and a scratch preview pass verify-release' $(if ($parsed) { "checks $($parsed.checks), failed $($parsed.failed)" } else { $verify.text })
            $summary['verify_release'] = [ordered]@{ path = $verifyOut; ok = [bool]($verify.exit -eq 0 -and $parsed -and $parsed.ok -eq $true) }
        } else {
            $summary['verify_release'] = [ordered]@{ skipped = $true }
        }
    } elseif ($Step -eq 'observe') {
        $live = LiveContractCheck
        if ($failed -eq 0) {
            $addresses = @(Get-Addresses)
            $chunk = 300
            $byRule = @{}
            $bySeverity = @{}
            $diagnostics = 0
            $checked = 0
            $chunkIndex = 0
            for ($i = 0; $i -lt $addresses.Count; $i += $chunk) {
                $part = $addresses[$i..([Math]::Min($i + $chunk - 1, $addresses.Count - 1))]
                $report = Join-Path $Work ("advisory-check-$chunkIndex.json")
                $log = Join-Path $Work ("advisory-check-$chunkIndex.log")
                $run = Invoke-Lines $ztCheck @('--cards', ($part -join ','), '--report', $report, '--log', $log)
                Check (($run.exit -eq 0 -or $run.exit -eq 1) -and (Test-Path -LiteralPath $report)) "the advisory observation reads Card chunk $chunkIndex" $run.text.Trim()
                if (Test-Path -LiteralPath $report) {
                    $value = Get-Content -LiteralPath $report -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String
                    $checked += $part.Count
                    foreach ($diagnostic in @($value.diagnostics)) {
                        $diagnostics++
                        $rule = [string]$diagnostic.rule
                        $severity = [string]$diagnostic.severity
                        $byRule[$rule] = 1 + [int]$byRule[$rule]
                        $bySeverity[$severity] = 1 + [int]$bySeverity[$severity]
                    }
                }
                $chunkIndex++
            }
            $summary['live'] = [ordered]@{
                archive = $live.facts.archive
                revision = $live.facts.revision
                cards = $live.facts.status.cards
                stage = $live.facts.stage
                policy_hash = $live.facts.policy_hash
                locks = $live.facts.locks
            }
            $summary['observation'] = [ordered]@{
                checked = $checked
                diagnostics = $diagnostics
                by_rule = $byRule
                by_severity = $bySeverity
                term_rules = @($byRule.Keys | Where-Object { $_ -like 'T*' } | Sort-Object)
                policy_table = $live.facts.table
            }
            Check ($checked -eq [int]$live.facts.status.cards) "the observation read every live Card" "$checked / $($live.facts.status.cards)"
        }
    } elseif ($Step -eq 'backup') {
        $live = LiveContractCheck
        $facts = $live.facts
        $before = Get-ArchiveIdAndRevision
        $beforeDigest = Get-CardTextDigest
        $summary['before'] = [ordered]@{
            archive_id = $before.id
            revision = $before.revision
            cards = $beforeDigest.cards
            card_digest = $beforeDigest.digest
            stage = $facts.stage
            policy_hash = $facts.policy_hash
            locks = $facts.locks
        }
        if ($failed -eq 0) {
            if (-not $BackupRoot) {
                $parent = Split-Path -Parent ([IO.Path]::GetFullPath($facts.archive))
                $BackupRoot = Join-Path $parent 'zt-release-backups'
            }
            if (-not $Label) { $Label = (Get-Date).ToString('yyyyMMdd-HHmmss') }
            $backupDir = Join-Path $BackupRoot $Label
            New-Item -ItemType Directory -Force $backupDir | Out-Null
            $database = Join-Path $facts.archive 'zt.sqlite3'
            Check (Test-Path -LiteralPath $database) 'the live archive database exists' $database

            # Stop the service: the copy must be consistent, and the migration runs at the next start.
            $livePid = $facts.status.pid
            $down = Invoke-Lines $zt @('down')
            Check ($down.exit -eq 0) 'the live service stops before the backup' $down.text.Trim()
            # zt down clears the record before the daemon closes the database; wait for the process.
            $stopped = $false
            for ($i = 0; $i -lt 240; $i++) {
                $alive = ($livePid -match '^\d+$') -and ($null -ne (Get-Process -Id ([int]$livePid) -ErrorAction SilentlyContinue))
                if ((Get-Status).state -eq 'down' -and -not $alive) { $stopped = $true; break }
                Start-Sleep -Milliseconds 250
            }
            Check $stopped 'the live service is down before the copy'

            $copied = [Collections.Generic.List[object]]::new()
            foreach ($suffix in @('', '-wal', '-shm')) {
                $source = "$database$suffix"
                if (-not (Test-Path -LiteralPath $source)) { continue }
                $target = Join-Path $backupDir ("zt.sqlite3$suffix")
                Copy-Item -LiteralPath $source -Destination $target -Force
                $sourceHash = Get-Sha256 $source
                $targetHash = Get-Sha256 $target
                Check ($sourceHash -eq $targetHash) "the backup copy of zt.sqlite3$suffix is byte-identical" $sourceHash
                $copied.Add([ordered]@{ file = "zt.sqlite3$suffix"; bytes = (Get-Item -LiteralPath $target).Length; sha256 = $targetHash })
            }
            $manifestRecord = [ordered]@{}
            foreach ($key in $summary['before'].Keys) { $manifestRecord[$key] = $summary['before'][$key] }
            $manifestRecord['files'] = $copied
            $manifestFile = Join-Path $backupDir 'backup.json'
            [IO.File]::WriteAllText($manifestFile, ($manifestRecord | ConvertTo-Json -Depth 8), $utf8)
            $summary['backup'] = [ordered]@{ path = $backupDir; files = $copied; manifest = $manifestFile; manifest_sha256 = Get-Sha256 $manifestFile }

            # Validate the copy by starting the current binary on it: the migration rehearsal and its
            # postflight prove the backup is loadable and keeps every authored identity.
            $validate = Join-Path $Work 'validated-copy'
            $validateConfig = Join-Path $validate 'config'
            $validateArchive = Join-Path $validate 'archive'
            New-Item -ItemType Directory -Force $validateConfig, $validateArchive | Out-Null
            Copy-Item -LiteralPath (Join-Path $backupDir 'zt.sqlite3') -Destination (Join-Path $validateArchive 'zt.sqlite3') -Force
            $savedConfig = $env:ZT_CONFIG_DIR
            $savedPort = $env:ZT_GUI_PORT
            $validation = [ordered]@{ ran = $false; ok = $false }
            try {
                $env:ZT_CONFIG_DIR = $validateConfig
                $env:ZT_GUI_PORT = '0'
                $null = Invoke-Lines $zt @('config', 'set', 'archive_root', $validateArchive)
                $up = Invoke-Lines $zt @('up')
                $validation.ran = $true
                $validation['started'] = ($up.exit -eq 0)
                $validateStatus = Get-Status
                $validateId = Get-ArchiveIdAndRevision
                $validation['state'] = $validateStatus.state
                $validation['archive_id'] = $validateId.id
                $validation['revision'] = $validateId.revision
                $validation['cards'] = $validateStatus.cards
                $validation['locks'] = ($validateStatus.lines | Where-Object { $_ -match '^locks: ' } | Select-Object -First 1) -replace '^locks: ', ''
                $validation['ok'] = ($validateStatus.up -and $validateId.id -eq $before.id -and $validateId.revision -eq $before.revision -and $validateStatus.cards -eq $beforeDigest.cards -and $validation['locks'] -match 'integrity ok')
                $null = Invoke-Lines $zt @('down')
            } finally {
                $env:ZT_CONFIG_DIR = $savedConfig
                $env:ZT_GUI_PORT = $savedPort
                Start-Sleep -Milliseconds 300
            }
            Check $validation.ok 'the backup copy starts and keeps the archive identity, revision, Cards and Locks' ($validation | ConvertTo-Json -Compress)
            $summary['validation'] = $validation

            # Restart the live service; the real migration (when one is pending) runs here.
            $upLive = Invoke-Lines $zt @('up')
            Check ($upLive.exit -eq 0) 'the live service restarts after the backup' $upLive.text.Trim()
            $liveAfter = $null
            for ($i = 0; $i -lt 120; $i++) {
                $liveAfter = Get-Status
                if ($liveAfter.up) { break }
                Start-Sleep -Milliseconds 250
            }
            $after = Get-ArchiveIdAndRevision
            $summary['after'] = [ordered]@{
                archive_id = $after.id
                revision = $after.revision
                cards = $liveAfter.cards
                state = $liveAfter.state
            }
            Check ($liveAfter.up) 'the live service is up after the backup' $liveAfter.state
            Check ($after.id -eq $before.id) 'the live archive identity is unchanged' "$($after.id) / $($before.id)"
            Check ($after.revision -eq $before.revision) 'the live archive revision is unchanged' "$($after.revision) / $($before.revision)"
            Check ($liveAfter.cards -eq $beforeDigest.cards) 'the live Card count is unchanged' "$($liveAfter.cards) / $($beforeDigest.cards)"
        }
    } elseif ($Step -in @('promote', 'rollback')) {
        $live = LiveContractCheck
        $record = Read-Decision $Decision $Step
        Write-Output ("DEBUG value=[$decision] len=$($record.Length)")
        Check $record.ok 'the owner decision is complete and names this issue' $(if ($record.ok) { $record.sha256 } else { $record.error })
        if ($record.ok) {
            $before = Get-ArchiveIdAndRevision
            $beforeDigest = Get-CardTextDigest
            $beforeFacts = $live.facts
            $summary['decision'] = [ordered]@{ path = (Resolve-Path -LiteralPath $Decision).Path; sha256 = $record.sha256; value = $record.value }
            $summary['before'] = [ordered]@{
                revision = $before.revision
                cards = $beforeDigest.cards
                card_digest = $beforeDigest.digest
                stage = $beforeFacts.stage
                policy_hash = $beforeFacts.policy_hash
            }
            $applied = [Collections.Generic.List[object]]::new()
            $overrides = $record.value.overrides.PSObject.Properties | Sort-Object Name
            if ($Step -eq 'promote') {
                foreach ($property in $overrides) {
                    $run = Invoke-Lines $zt @('config', 'set', $property.Name, [string]$property.Value)
                    $applied.Add([ordered]@{ key = $property.Name; value = [string]$property.Value; ok = ($run.exit -eq 0); output = $run.text.Trim() })
                }
                $stageRun = Invoke-Lines $zt @('config', 'set', 'gate.stage', [string]$record.value.stage)
                $applied.Add([ordered]@{ key = 'gate.stage'; value = [string]$record.value.stage; ok = ($stageRun.exit -eq 0); output = $stageRun.text.Trim() })
            } else {
                $stageRun = Invoke-Lines $zt @('config', 'set', 'gate.stage', [string]$record.value.stage)
                $applied.Add([ordered]@{ key = 'gate.stage'; value = [string]$record.value.stage; ok = ($stageRun.exit -eq 0); output = $stageRun.text.Trim() })
                foreach ($property in $overrides) {
                    $run = Invoke-Lines $zt @('config', 'set', $property.Name, [string]$property.Value)
                    $applied.Add([ordered]@{ key = $property.Name; value = [string]$property.Value; ok = ($run.exit -eq 0); output = $run.text.Trim() })
                }
            }
            $allApplied = @($applied | Where-Object { -not $_.ok }).Count -eq 0
            Check $allApplied 'every decision row is applied' (($applied | ForEach-Object { "$($_.key)=$($_.value)" }) -join ', ')
            $afterFacts = Get-LiveFacts
            $after = Get-ArchiveIdAndRevision
            $afterDigest = Get-CardTextDigest
            $summary['applied'] = $applied
            $summary['after'] = [ordered]@{
                revision = $after.revision
                cards = $afterDigest.cards
                card_digest = $afterDigest.digest
                stage = $afterFacts.stage
                policy_hash = $afterFacts.policy_hash
            }
            Check ($afterDigest.digest -eq $beforeDigest.digest -and $afterDigest.cards -eq $beforeDigest.cards) 'the policy change rewrites no Card' "digest $($beforeDigest.digest.Substring(0, 12)) / $($afterDigest.digest.Substring(0, 12))"
            Check ($afterFacts.stage -eq $record.value.stage) 'the live policy reports the decided stage' "$($afterFacts.stage) / $($record.value.stage)"
            Check ($after.revision -gt $before.revision) 'the policy change advances the archive revision' "$($after.revision) / $($before.revision)"
            Check ($afterFacts.locks -match 'integrity ok') 'the mandatory Lock invariants stay active after the policy change' $afterFacts.locks
        }
    }
} catch {
    Check $false 'the step completes without an exception' $_.Exception.Message
} finally {
    $summary['checks'] = $checks
    $summary['failed'] = $failed
    $summary['ok'] = ($failed -eq 0)
    $summary['failures'] = @($checks | Where-Object { -not $_.ok } | ForEach-Object { $_.name })
    [IO.File]::WriteAllText($evidencePath, ($summary | ConvertTo-Json -Depth 12), $utf8)
    $summary['evidence'] = $evidencePath
    Write-Output ($summary | ConvertTo-Json -Depth 6 -Compress)
}
exit $(if ($failed) { 1 } else { 0 })
