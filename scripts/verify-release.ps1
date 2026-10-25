#Requires -Version 7.5
<#
.SYNOPSIS
  Verify the candidate release set: the installed binaries, both frozen digest variants, the
  installed agent skills, the running service's metadata, and a scratch advisory/hard-invariant
  check, all against the release manifest.

.DESCRIPTION
  S28 (#170). This is the installed-and-running half of the release verification seam; the fast
  static half is `scripts/book-digest/tests/release-set.tests.ps1`, which this script runs too.

    pwsh -NoProfile -File scripts\verify-release.ps1
    pwsh -NoProfile -File scripts\verify-release.ps1 -SkipLive        # no daemon / MCP reads
    pwsh -NoProfile -File scripts\verify-release.ps1 -Out evidence.json

  It never writes a Card in the live archive and never changes the live config. The scratch check
  uses its own temporary config folder and archive and stops its own daemon. Prints one compact
  JSON summary as its last line and exits 1 when any check fails.
#>
[CmdletBinding()]
param(
    [string]$Project = '',
    [string]$Zt = '',
    [string]$ZtCheck = '',
    [string]$ZtDigest = '',
    [string]$Out = '',
    [switch]$SkipStatic,
    [switch]$SkipLive,
    [switch]$SkipScratch,
    [switch]$SkipSkills,
    [switch]$SkipWiki
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
$checks = [Collections.Generic.List[object]]::new()
$failed = 0
function Check($condition, [string]$name, [string]$detail = '') {
    $ok = [bool]$condition
    if (-not $ok) { $script:failed++ }
    $script:checks.Add([ordered]@{ name = $name; ok = $ok; detail = $detail })
    if ($ok) { Write-Output "ok: $name$(if ($detail) { " ($detail)" })" } else { Write-Output "FAIL: $name$(if ($detail) { " ($detail)" })" }
}
function Get-Sha256([string]$path) {
    if (-not (Test-Path -LiteralPath $path)) { return '' }
    (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Resolve-Tool([string]$given, [string]$name) {
    if ($given) { return (Resolve-Path -LiteralPath $given).Path }
    $found = Get-Command $name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($found) { return $found.Source }
    return ''
}
function Invoke-Lines([string]$exe, [string[]]$arguments) {
    $lines = @(& $exe @arguments 2>&1 | ForEach-Object { [string]$_ })
    [pscustomobject]@{ exit = $LASTEXITCODE; lines = $lines; text = ($lines -join "`n") }
}
function Get-TreeHash([string]$dir) {
    if (-not (Test-Path -LiteralPath $dir)) { return '' }
    $parts = foreach ($file in (Get-ChildItem -LiteralPath $dir -Recurse -File | Sort-Object FullName)) {
        $relative = $file.FullName.Substring($dir.Length).Replace('\', '/').TrimStart('/')
        "$relative=$(Get-Sha256 $file.FullName)"
    }
    ($parts -join "`n")
}
# The published archive path can carry Rust's canonicalize prefix; compare the meaningful path.
function ConvertTo-NormalPath([string]$path) {
    if (-not $path) { return '' }
    ($path -replace '^\\\\\?\\', '') -replace '\\+$', ''
}
function Get-McpInitialize([string]$exe) {
    $info = [Diagnostics.ProcessStartInfo]::new($exe)
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.ArgumentList.Add('mcp')
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    $null = $process.Start()
    $out = $process.StandardOutput.ReadToEndAsync()
    $err = $process.StandardError.ReadToEndAsync()
    $process.StandardInput.WriteLine('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}')
    $process.StandardInput.Close()
    if (-not $process.WaitForExit(30000)) { try { $process.Kill($true) } catch { }; throw 'zt mcp did not answer initialize within 30 s' }
    $text = $out.GetAwaiter().GetResult()
    $stderr = $err.GetAwaiter().GetResult()
    $process.Dispose()
    $line = @($text -split "`n" | Where-Object { $_.Trim() } | Select-Object -First 1)
    if (-not $line) { throw "zt mcp printed no initialize answer: $stderr" }
    $line | ConvertFrom-Json -DateKind String
}

# --- static half -------------------------------------------------------------------------------
$static = [ordered]@{ ran = $false; ok = $false; failed = $null; tail = @() }
$staticScript = Join-Path $Project 'scripts\book-digest\tests\release-set.tests.ps1'
if (-not $SkipStatic) {
    $arguments = @('-NoProfile', '-File', $staticScript)
    if ($Zt) { $arguments += @('-Zt', $Zt) }
    $run = & pwsh @arguments 2>&1 | ForEach-Object { [string]$_ }
    $code = $LASTEXITCODE
    $static.ran = $true
    $static.ok = ($code -eq 0)
    $static.failed = $code
    $static.tail = @($run | Select-Object -Last 2)
    Check $static.ok 'the static release-set checks pass' ($static.tail -join ' | ')
}

$zt = Resolve-Tool $Zt 'zt'
$ztCheck = Resolve-Tool $ZtCheck 'zt-check'
$ztDigest = Resolve-Tool $ZtDigest 'zt-digest'
if (-not $zt -or -not $ztCheck -or -not $ztDigest) { throw 'zt, zt-check and zt-digest must be installed (on PATH or given with -Zt/-ZtCheck/-ZtDigest)' }
Check (Test-Path -LiteralPath $zt) 'the zt executable is installed' $zt
Check (Test-Path -LiteralPath $ztCheck) 'the zt-check executable is installed' $ztCheck
Check (Test-Path -LiteralPath $ztDigest) 'the zt-digest executable is installed' $ztDigest

$executable = $manifest.contracts.executable
$versionRun = Invoke-Lines $zt @('version', '--json')
$version = $null
if ($versionRun.exit -eq 0 -and $versionRun.lines.Count -eq 1 -and $versionRun.lines[0].TrimStart().StartsWith('{')) {
    $version = $versionRun.lines[0] | ConvertFrom-Json -DateKind String
}
Check ($null -ne $version) 'zt version --json answers one JSON object'
if ($version) {
    Check ($version.zt -eq $executable.zt) 'the installed zt version is the manifest executable' "$($version.zt)"
    Check ($version.checker -eq $executable.zt_check) 'the installed zt carries the manifest checker' "$($version.checker)"
    Check ($version.rule_version -eq $manifest.contracts.rule.rule_version -and $version.diagnostic_schema -eq $manifest.contracts.rule.diagnostic_schema -and $version.applicability_version -eq $manifest.contracts.rule.applicability_version) 'the installed zt publishes the manifest rule contract'
    Check ($version.schema -eq $manifest.contracts.archive_schema.generation) 'the installed zt publishes the manifest archive schema' "$($version.schema)"
    Check ($version.capabilities.term_links -eq $manifest.capabilities.term_links -and $version.capabilities.term_cards -eq $manifest.capabilities.term_cards) 'the installed zt publishes the Term capabilities'
}
$checkVersion = Invoke-Lines $ztCheck @('--version')
Check ($checkVersion.text -match "^zt-check $([regex]::Escape($executable.zt_check))$") 'the installed zt-check version is the manifest checker' $checkVersion.text.Trim()
$digestVersion = Invoke-Lines $ztDigest @('version')
Check ($digestVersion.text -match "^zt-digest $([regex]::Escape($executable.zt_digest))$") 'the installed zt-digest version is the manifest tool' $digestVersion.text.Trim()
$help = Invoke-Lines $zt @('help')
Check ($help.text.Contains('zt version --json') -and $help.text.Contains('zt config policy')) 'zt help names the version contract and the policy command'

$hashes = [ordered]@{
    zt        = Get-Sha256 $zt
    zt_check  = Get-Sha256 $ztCheck
    zt_digest = Get-Sha256 $ztDigest
}

# --- the frozen workflows ----------------------------------------------------------------------
$assembler = Join-Path $HOME '.claude\skills\workflow-framework-protocol\tools\assemble.mjs'
$workflows = [ordered]@{}
if (Test-Path -LiteralPath $assembler) {
    foreach ($name in @('zt-econ-digest', 'zt-math-digest')) {
        $run = Invoke-Lines (Get-Command node -CommandType Application | Select-Object -First 1).Source @($assembler, 'verify', $name, '--scope', 'project', '--project', $Project)
        $answer = $null
        try { $answer = $run.lines[-1] | ConvertFrom-Json -DateKind String } catch { }
        $expected = $manifest.contracts.workflow.$name
        $ok = $run.exit -eq 0 -and $answer -and $answer.ok -eq $true -and $answer.version -eq $expected -and @($answer.problems).Count -eq 0
        Check $ok "the installed $name verifies against its frozen lock" $(if ($answer) { "v$($answer.version); $($answer.problems -join '; ')" } else { $run.text })
        $lock = Get-Content -LiteralPath (Join-Path $Project ".claude\workflow-framework\$name\lock.json") -Raw | ConvertFrom-Json -DateKind String
        $workflows[$name] = [ordered]@{ version = $lock.version; sha256 = $lock.sha256; verified = [bool]$ok }
    }
} else {
    Check $false 'the workflow-framework assembler is installed' $assembler
}

# --- the installed skills ----------------------------------------------------------------------
$skills = [ordered]@{}
if (-not $SkipSkills) {
    foreach ($name in $manifest.skills) {
        $source = Join-Path $Project "skills\$name"
        $sourceHash = Get-TreeHash $source
        $copies = [ordered]@{}
        foreach ($folder in @("$HOME\.claude\skills", "$HOME\.codex\skills")) {
            $target = Join-Path $folder $name
            $targetHash = Get-TreeHash $target
            $ok = ($sourceHash -ne '' -and $sourceHash -eq $targetHash)
            Check $ok "the installed $name skill matches the repository copy" $target
            $copies[$folder] = [ordered]@{ exists = (Test-Path -LiteralPath $target); match = [bool]$ok }
        }
        $dshLink = Join-Path "$HOME\.dsh\skills" $name
        $dshOk = (Test-Path -LiteralPath $dshLink) -and ((Get-TreeHash $dshLink) -eq $sourceHash)
        Check $dshOk "the DeepSeek Harness $name skill resolves to the repository copy" $dshLink
        $skills[$name] = [ordered]@{ source_sha256 = $sourceHash; copies = $copies; dsh = [bool]$dshOk }
    }
}

# --- the running service -----------------------------------------------------------------------
$service = [ordered]@{ checked = $false }
if (-not $SkipLive) {
    $service.checked = $true
    $status = Invoke-Lines $zt @('status')
    $statusText = $status.text
    $up = $statusText -match '(?m)^state: up$'
    Check $up 'the live service is up' $(($statusText -split "`n" | Where-Object { $_ -match '^pid: ' }) -join ' ')
    $daemon = [regex]::Match($statusText, '(?m)^daemon: version=(\S+) ')
    Check ($daemon.Success -and $daemon.Groups[1].Value -eq $executable.zt) 'the running daemon is the manifest executable' $(if ($daemon.Success) { $daemon.Groups[1].Value })
    $revisionLine = [regex]::Match($statusText, '(?m)^revision: (\d+)$')
    $revision = if ($revisionLine.Success) { [int64]$revisionLine.Groups[1].Value } else { -1 }
    Check ($revision -ge 0) 'the live archive reports a revision' "$revision"
    Check ($statusText -match '(?m)^locks: .*integrity ok') 'the live Lock integrity is ok' (($statusText -split "`n" | Where-Object { $_ -match '^locks: ' }) -join ' ')
    Check ($statusText -match '(?m)^sessions: 0$') 'no live Session is open during the release check'

    $policy = Invoke-Lines $zt @('config', 'policy')
    $policyLines = @{}
    foreach ($line in $policy.lines) {
        if ($line -match '^([^=]+)=(.*)$') { $policyLines[$Matches[1]] = $Matches[2] }
    }
    Check ($policyLines['gate_stage'] -match '^advisory\b') 'the live archive runs the advisory gate stage' $policyLines['gate_stage']
    Check ($policyLines['rule_version'] -eq $manifest.contracts.rule.rule_version) 'the live policy reports the manifest rule version' $policyLines['rule_version']
    Check ($policyLines['diagnostic_schema'] -eq $manifest.contracts.rule.diagnostic_schema) 'the live policy reports the manifest diagnostic schema' $policyLines['diagnostic_schema']
    Check ($policyLines['policy_hash'] -match '^[0-9a-f]{64}$') 'the live policy publishes its hash' $policyLines['policy_hash']

    $initialize = $null
    try { $initialize = Get-McpInitialize $zt } catch { Check $false 'zt mcp answers initialize' $_.Exception.Message }
    if ($initialize) {
        Check ($initialize.result.serverInfo.version -eq $executable.zt) 'the MCP server reports the manifest executable' $initialize.result.serverInfo.version
        Check ($initialize.result.zt_check_version -eq $executable.zt_check) 'the MCP server carries the manifest checker' $initialize.result.zt_check_version
        Check ($initialize.result.check_contract.rule_version -eq $manifest.contracts.rule.rule_version -and $initialize.result.check_contract.diagnostic_schema -eq $manifest.contracts.rule.diagnostic_schema -and $initialize.result.check_contract.applicability_version -eq $manifest.contracts.rule.applicability_version) 'the MCP initialize contract is the manifest rule contract'
    }

    $read = Invoke-Lines $ztDigest @('call', '-Tool', 'get_cards', '-Json', '{"addresses":[],"view":"text"}')
    $contract = $null
    if ($read.exit -eq 0 -and $read.lines.Count -ge 1) {
        $body = $read.lines[-1] -replace '^\d+\s+', ''
        try { $contract = $body | ConvertFrom-Json -DateKind String } catch { }
    }
    Check ($null -ne $contract -and $null -ne $contract.check_contract) 'the live service publishes its check contract on a read'
    if ($contract -and $contract.check_contract) {
        $live = $contract.check_contract
        Check ($live.rule_version -eq $manifest.contracts.rule.rule_version -and $live.diagnostic_schema -eq $manifest.contracts.rule.diagnostic_schema -and $live.applicability_version -eq $manifest.contracts.rule.applicability_version) 'the live check contract is the manifest rule contract'
        Check ($live.checker_version -eq $executable.zt_check) 'the live contract names the manifest checker' $live.checker_version
        Check ($live.capabilities.term_links -eq $manifest.capabilities.term_links -and $live.capabilities.term_cards -eq $manifest.capabilities.term_cards) 'the live contract publishes the Term capabilities'
        Check ($live.stage -eq $manifest.policy.default_stage) 'the live contract reports the advisory stage' $live.stage
        Check ($live.archive.id -match '^[0-9a-f]{32}$') 'the live contract names the archive identity' $live.archive.id
        $statusSqlite = [regex]::Match($statusText, '(?m)^sqlite: (.+)$').Groups[1].Value.Trim()
        $statusRoot = if ($statusSqlite) { Split-Path -Parent $statusSqlite } else { '' }
        Check ((ConvertTo-NormalPath $live.archive.path) -ieq (ConvertTo-NormalPath $statusRoot)) 'the live contract names the configured archive' $live.archive.path
        Check ([int64]$live.archive.revision -eq $revision) 'the live read and the status name one archive revision' "$($live.archive.revision) / $revision"
        $service['archive_id'] = $live.archive.id
        $service['archive_path'] = $live.archive.path
        $service['revision'] = $revision
        $service['rule_version'] = $live.rule_version
        $service['checker_version'] = $live.checker_version
        $service['stage'] = $live.stage
        $service['cards'] = [regex]::Match($statusText, '(?m)^cards: (\d+)$').Groups[1].Value
    }
}

# --- a scratch archive: the advisory default and one mandatory invariant -----------------------
$scratchResult = [ordered]@{ ran = $false }
if (-not $SkipScratch) {
    $scratchResult.ran = $true
    $root = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "zt-release-verify-$PID"))
    $savedConfig = $env:ZT_CONFIG_DIR
    $savedPort = $env:ZT_GUI_PORT
    try {
        New-Item -ItemType Directory -Force $root | Out-Null
        $env:ZT_CONFIG_DIR = Join-Path $root 'config'
        $env:ZT_GUI_PORT = '0'
        New-Item -ItemType Directory -Force $env:ZT_CONFIG_DIR | Out-Null
        $archive = Join-Path $root 'archive'
        $null = Invoke-Lines $zt @('config', 'set', 'archive_root', $archive)
        $up = Invoke-Lines $zt @('up')
        Check ($up.exit -eq 0) 'the scratch service starts' $up.text.Trim()
        $scratchPolicy = Invoke-Lines $zt @('config', 'policy')
        $scratchStage = [regex]::Match($scratchPolicy.text, '(?m)^gate_stage=(\S+)').Groups[1].Value
        Check ($scratchStage -eq 'advisory') 'a fresh archive defaults to the advisory gate stage' $scratchStage

        # A quality fault (the Card language's semicolon) is accepted and stored under advisory.
        $topic = Invoke-Lines $ztDigest @('call', '-Open', '-Tool', 'create_topic', '-Json', '{"title":"发布检查","body":"这次发布检查的主题。"}')
        $topicAddress = ($topic.lines | Where-Object { $_ -match '^\d+ ok \S+$' } | Select-Object -First 1) -replace '^\d+ ok ', ''
        Check ($topic.exit -eq 0 -and $topicAddress) 'the scratch archive accepts a Topic write' $topicAddress
        if ($topicAddress) {
            $body = '这条正文用了分号；advisory 阶段仍应存储它。'
            $draft = @{ at = $topicAddress; title = '发布检查：一条被建议修改的卡片'; body = $body } | ConvertTo-Json -Compress
            $advised = Invoke-Lines $ztDigest @('call', '-Open', '-Tool', 'create_successor', '-Json', $draft)
            $advisedAddress = ($advised.lines | Where-Object { $_ -match '^\d+ ok \S+$' } | Select-Object -First 1) -replace '^\d+ ok ', ''
            Check ($advised.exit -eq 0 -and $advisedAddress) 'advisory stores a quality-fault write instead of refusing it' $advisedAddress
            if ($advisedAddress) {
                $read = Invoke-Lines $ztDigest @('call', '-Tool', 'get_card', '-Json', (@{ at = $advisedAddress } | ConvertTo-Json -Compress))
                $stored = $false
                if ($read.exit -eq 0) {
                    $card = ($read.lines[-1] -replace '^\d+\s+', '') | ConvertFrom-Json -DateKind String
                    $stored = $card.body -eq $body
                }
                Check $stored 'the advised Card keeps its exact text' $advisedAddress
            }
            # A new Term use must target a Term head. That target grammar is a mandatory Link
            # invariant, so the same advisory archive refuses it with no Card or revision: the
            # new quality rules advise, the existing hard invariants stay active.
            $before = [int64]([regex]::Match((Invoke-Lines $zt @('status')).text, '(?m)^revision: (\d+)$').Groups[1].Value)
            $wrongTarget = @{ at = $advisedAddress; kind = 'side'; title = '发布检查：错误术语链接的卡片'; body = '这一句使用了 [[均衡@0/0]] 这个术语。' } | ConvertTo-Json -Compress
            $refused = Invoke-Lines $ztDigest @('call', '-Open', '-Tool', 'create_successor', '-Json', $wrongTarget)
            Check ($refused.exit -ne 0 -and $refused.text -match 'invalid Term link macro target') 'advisory still refuses a Term use that does not target a Term head' ($refused.text.Trim())
            $after = [int64]([regex]::Match((Invoke-Lines $zt @('status')).text, '(?m)^revision: (\d+)$').Groups[1].Value)
            Check ($after -eq $before) 'the refused Term use leaves the archive revision unchanged' "$before / $after"
        }
        $down = Invoke-Lines $zt @('down')
        Check ($down.exit -eq 0) 'the scratch service stops' $down.text.Trim()
        $scratchResult['ok'] = ($script:failed -eq 0)
    } finally {
        $env:ZT_CONFIG_DIR = $savedConfig
        $env:ZT_GUI_PORT = $savedPort
        Start-Sleep -Milliseconds 300
        Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# --- the wiki ----------------------------------------------------------------------------------
$wiki = [ordered]@{ ran = $false }
if (-not $SkipWiki) {
    $wikiScript = Join-Path $HOME '.claude\skills\project-wiki\scripts\wiki.mjs'
    if (Test-Path -LiteralPath $wikiScript) {
        $wiki.ran = $true
        $run = Invoke-Lines (Get-Command node -CommandType Application | Select-Object -First 1).Source @($wikiScript, 'check')
        $upToDate = $run.exit -eq 0 -and $run.text -match 'the wiki is current'
        Check $upToDate 'the developer wiki is current with the code' (($run.lines | Select-Object -Last 1) -join '')
        $wiki['ok'] = $upToDate
    } else {
        Check $false 'the project-wiki checker is installed' $wikiScript
    }
}

$summary = [ordered]@{
    ok = ($failed -eq 0)
    issue = $manifest.issue
    slice = $manifest.slice
    manifest = $manifestPath
    checks = $checks.Count
    failed = $failed
    contracts = $manifest.contracts
    capabilities = $manifest.capabilities
    policy = $manifest.policy
    binaries = [ordered]@{ zt = $zt; zt_check = $ztCheck; zt_digest = $ztDigest; sha256 = $hashes }
    workflows = $workflows
    skills = $skills
    static = $static
    service = $service
    scratch = $scratchResult
    wiki = $wiki
    failures = @($checks | Where-Object { -not $_.ok } | ForEach-Object { $_.name })
}
if ($Out) {
    $outPath = if ([IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path $Project $Out }
    $outParent = Split-Path -Parent $outPath
    if ($outParent) { New-Item -ItemType Directory -Force $outParent | Out-Null }
    [IO.File]::WriteAllText($outPath, ($summary | ConvertTo-Json -Depth 12), $utf8)
    Write-Output "evidence: $outPath"
}
Write-Output ($summary | ConvertTo-Json -Depth 6 -Compress)
exit $(if ($failed) { 1 } else { 0 })
