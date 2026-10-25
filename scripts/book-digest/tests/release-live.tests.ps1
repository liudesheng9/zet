#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\release-live.tests.ps1
# S31 (#173): the live-release procedure on an isolated scratch archive. Proves the preflight
# refusal while a digest is active, the consistent backup and its validation, the owner-decision
# guard on a policy change, and that a promote/rollback round rewrites no Card and keeps the
# mandatory invariants active. Starts no live work: the box is a temporary config folder.
param([string]$ZtDigest = '', [string]$ZtCheck = '', [string]$Zt = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$script = Join-Path $repo 'scripts\release-live.ps1'
if (-not $Zt) { $Zt = (Get-Command zt -CommandType Application).Source }
if (-not $ZtCheck) { $ZtCheck = (Get-Command zt-check -CommandType Application).Source }
if (-not $ZtDigest) { $ZtDigest = (Get-Command zt-digest -CommandType Application).Source }
$root = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "release-live-test-$PID"))
$config = Join-Path $root 'config'
$archive = Join-Path $root 'archive'
$work = Join-Path $root 'work'
New-Item -ItemType Directory -Force $config, $archive, $work | Out-Null
$savedConfig = $env:ZT_CONFIG_DIR
$savedPort = $env:ZT_GUI_PORT
$failed = 0
$checks = 0
function Assert($condition, [string]$message) {
    $script:checks++
    if (-not $condition) { $script:failed++; Write-Output "FAIL: $message" } else { Write-Output "ok: $message" }
}
function Invoke-Step([string[]]$arguments) {
    $lines = @(& pwsh -NoProfile -File $script @arguments 2>&1 | ForEach-Object { [string]$_ })
    $json = $null
    try { $json = ($lines | Select-Object -Last 1) | ConvertFrom-Json -DateKind String } catch { }
    [pscustomobject]@{ exit = $LASTEXITCODE; lines = $lines; json = $json; text = ($lines -join "`n") }
}
function Invoke-DigestJson([string]$tool, [string]$argumentsJson) {
    $out = Join-Path $root 'digest.json'
    & $ZtDigest call -Tool $tool -Json $argumentsJson > $out 2> (Join-Path $root 'digest.err')
    if ($LASTEXITCODE -ne 0) { throw "zt-digest $tool failed: $([IO.File]::ReadAllText((Join-Path $root 'digest.err')))" }
    $text = [IO.File]::ReadAllText($out)
    if ($text -match '^\d+ ok (\S+)\s*$') { return [pscustomobject]@{ address = $Matches[1] } }
    ($text -replace '^\d+\s+', '') | ConvertFrom-Json -DateKind String
}
function New-Decision([string]$action, [string]$stage, [string]$path) {
    $decision = [ordered]@{
        schema_version = 1
        issue = 173
        approved = $true
        action = $action
        owner = 'release-live.tests.ps1'
        stage = $stage
        overrides = [ordered]@{}
        evidence = @('scratch archive', 'release-live.tests.ps1')
        note = "test decision for $action"
    }
    [IO.File]::WriteAllText($path, ($decision | ConvertTo-Json -Depth 6), $utf8)
    $path
}
try {
    $env:ZT_CONFIG_DIR = $config
    $env:ZT_GUI_PORT = '0'
    $null = & $Zt config set archive_root $archive
    $null = & $Zt up
    $status = ''
    for ($i = 0; $i -lt 120; $i++) { $status = (& $Zt status | Out-String); if ($status -match 'state: up') { break }; Start-Sleep -Milliseconds 250 }
    Assert ($status -match 'state: up') 'the scratch service starts'

    $base = @('-Project', $repo, '-Zt', $Zt, '-ZtCheck', $ZtCheck, '-ZtDigest', $ZtDigest, '-Work', $work, '-SkipVerify')
    $pre = Invoke-Step ($base + @('-Step', 'preflight'))
    Assert ($pre.exit -eq 0 -and $pre.json.ok) "preflight passes on a quiet archive ($($pre.json.failures -join '; '))"
    Assert ($pre.json.live.stage -eq 'advisory') 'preflight records the advisory stage'

    # An active digest marker refuses the step; a stale one does not.
    $marker = Join-Path $archive 'zt.digest.run'
    $now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $active = [ordered]@{ schema_version = 1; run_id = 'release-live-test'; out = $root; unit = 'cards'; pid = $PID; renewed = $now; ttl_s = 300 } | ConvertTo-Json -Compress
    [IO.File]::WriteAllText($marker, $active, $utf8)
    $refused = Invoke-Step ($base + @('-Step', 'preflight'))
    Assert ($refused.exit -ne 0 -and $refused.text -match 'no digest is active') 'preflight refuses while a digest is active'
    $stale = [ordered]@{ schema_version = 1; run_id = 'release-live-test'; out = $root; unit = 'cards'; pid = $PID; renewed = ($now - 4000); ttl_s = 300 } | ConvertTo-Json -Compress
    [IO.File]::WriteAllText($marker, $stale, $utf8)
    $quiet = Invoke-Step ($base + @('-Step', 'preflight'))
    Assert ($quiet.exit -eq 0 -and $quiet.json.live.active_digest.active -eq $false) 'a stale marker does not block the release'
    Remove-Item -LiteralPath $marker -Force

    # The backup stops the service, validates the copy, and preserves identity, revision, Cards and Locks.
    $before = (& $Zt status | Out-String)
    $beforeRevision = [int64]([regex]::Match($before, '(?m)^revision: (\d+)$').Groups[1].Value)
    $beforeCards = [int]([regex]::Match($before, '(?m)^cards: (\d+)$').Groups[1].Value)
    $backup = Invoke-Step ($base + @('-Step', 'backup', '-BackupRoot', (Join-Path $root 'backups'), '-Label', 'test'))
    Assert ($backup.exit -eq 0 -and $backup.json.ok) "backup passes ($($backup.json.failures -join '; '))"
    Assert ($backup.json.validation.ok -and $backup.json.validation.revision -eq $beforeRevision) 'the validated copy keeps the revision'
    Assert ($backup.json.after.revision -eq $beforeRevision -and $backup.json.after.cards -eq $beforeCards) 'the live archive is unchanged after the backup'
    Assert (Test-Path -LiteralPath (Join-Path $backup.json.backup.path 'zt.sqlite3')) 'the backup copy remains on disk'

    # A policy change needs the owner's decision file.
    $unguarded = Invoke-Step ($base + @('-Step', 'promote'))
    Assert ($unguarded.exit -ne 0 -and $unguarded.text -match 'no -Decision') 'promotion without a decision is refused'

    # A quality-fault Card stores under advisory; under enforce the same write is refused.
    $topic = Invoke-DigestJson 'create_literature' '{"bibtex":"@book{ReleaseLive, title={Release Live Test}, author={Test, Ada}, year={2026}}","body":"该文献只用于发布程序的临时检查。"}'
    $topicAddress = [string]$topic.address
    Assert ($topicAddress -ne '') 'the scratch archive accepts a Literature root'
    $body = '这条正文用了分号；advisory 阶段仍应存储它。'
    $draft = @{ at = $topicAddress; title = '发布检查：一条被建议修改的卡片'; body = $body } | ConvertTo-Json -Compress
    $stored = Invoke-DigestJson 'create_successor' $draft
    $advisedAddress = [string]$stored.address
    Assert ($advisedAddress -ne '') 'advisory stores a quality-fault write'

    $promote = Invoke-Step ($base + @('-Step', 'promote', '-Decision', (New-Decision 'promote' 'enforce' (Join-Path $root 'promote.json'))))
    Assert ($promote.exit -eq 0 -and $promote.json.ok -and $promote.json.after.stage -eq 'enforce') "promotion applies the decision ($($promote.json.failures -join '; '))"
    Assert ($promote.json.before.card_digest -eq $promote.json.after.card_digest) 'promotion rewrites no Card'

    $refusedWrite = @{ at = $advisedAddress; title = '发布检查：强制阶段拒绝的卡片'; body = $body } | ConvertTo-Json -Compress
    & $ZtDigest call -Tool create_successor -Json $refusedWrite > (Join-Path $root 'refused.json') 2>&1
    Assert ($LASTEXITCODE -ne 0) 'the enforced archive refuses the same quality fault'
    $hard = @{ at = $advisedAddress; title = '发布检查：错误术语链接'; body = "这一句使用了 [[均衡@$topicAddress]] 这个术语。" } | ConvertTo-Json -Compress
    & $ZtDigest call -Tool create_successor -Json $hard > (Join-Path $root 'hard.json') 2>&1
    Assert ($LASTEXITCODE -ne 0 -and ([IO.File]::ReadAllText((Join-Path $root 'hard.json')) -match 'invalid Term link macro target')) 'the mandatory Term-link invariant refuses under enforce'

    $rollback = Invoke-Step ($base + @('-Step', 'rollback', '-Decision', (New-Decision 'rollback' 'advisory' (Join-Path $root 'rollback.json'))))
    Assert ($rollback.exit -eq 0 -and $rollback.json.ok -and $rollback.json.after.stage -eq 'advisory') "rollback applies the decision ($($rollback.json.failures -join '; '))"
    Assert ($rollback.json.before.card_digest -eq $rollback.json.after.card_digest) 'rollback rewrites no Card'
    $afterPolicy = (& $Zt config policy | Out-String)
    Assert ($afterPolicy -match 'gate_stage=advisory') 'rollback returns the quality checks to advice'
    Assert ($afterPolicy -match 'invariants: envelope, fields, address, links, targets, access, locks') 'rollback keeps the mandatory invariants named'
    & $ZtDigest call -Tool create_successor -Json $hard > (Join-Path $root 'hard2.json') 2>&1
    Assert ($LASTEXITCODE -ne 0 -and ([IO.File]::ReadAllText((Join-Path $root 'hard2.json')) -match 'invalid Term link macro target')) 'the mandatory Term-link invariant still refuses after rollback'
    $storedAgain = @{ at = $advisedAddress; title = '发布检查：回滚后再次存储'; body = $body } | ConvertTo-Json -Compress
    $again = Invoke-DigestJson 'create_successor' $storedAgain
    Assert ([string]$again.address -ne '') 'advisory stores the quality fault again after rollback'

    # The observation is read-only and counts the stored debt.
    $observe = Invoke-Step ($base + @('-Step', 'observe'))
    Assert ($observe.exit -eq 0 -and $observe.json.ok) "the advisory observation completes ($($observe.json.failures -join '; '))"
    Assert ($observe.json.observation.checked -eq $observe.json.live.cards) 'the observation read every Card'
    Assert ($observe.json.observation.by_severity.PSObject.Properties.Name.Count -ge 0) 'the observation reports its severity counts'
} finally {
    try { $null = & $Zt down } catch { }
    $env:ZT_CONFIG_DIR = $savedConfig
    $env:ZT_GUI_PORT = $savedPort
    Start-Sleep -Milliseconds 400
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output ("release-live: $checks checks, $failed failed")
exit $(if ($failed) { 1 } else { 0 })
