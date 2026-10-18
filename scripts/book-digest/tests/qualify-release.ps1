#Requires -Version 7.5
<#
.SYNOPSIS
  Qualify the digest release: isolated latency, parity and fixed-local-work evidence for both variants.

.DESCRIPTION
  This is the release-qualification harness for the frozen economics and mathematics digest
  workflows (issue #89, R347-R356). It is not one of the `*.tests.ps1` suites the aggregate runner
  picks up: it needs a consistent copy of the representative archive and takes several minutes, so
  it is run explicitly at a release boundary.

    pwsh -NoProfile -File scripts\book-digest\tests\qualify-release.ps1 `
        -Archive D:\zt\zt.sqlite3 -Source <short book>.zh.md `
        -Out docs\digest-release-latency-evidence.json -Work D:\zt-target\qualification\issue89\q

  Per variant (econ, math), each on its own consistent copy of -Archive:

   1. Records the input facts: archive hash and size, `zt version`, the hashes of the installed
      `zt`/`zt-check`/`zt-digest`, and the full-parser Link parity of the copy
      (`zt status --check-links`).
   2. Prepares one short source with `-Variant <v> -Apply`, so the measurements run on a real
      prepared run bound to the isolated archive.
   3. Measures the named tool targets - `snapshot-topics`, Card `plan`/`check`/`report` and
      `prepare-link` - with one warm-up and `-Runs` measured invocations each, reporting median and
      p95. Each invocation runs through the public test seam, so its MCP calls, requested addresses
      and process count are recorded from the trace.
   4. Measures `model apply`: a 66-part model applied as bounded identified batches through the
      real `zt-digest call -OperationPrefix` helper, with batch sizes, the retained part map and
      the receipts' replay behaviour recorded.
   5. Measures an ordinary Card read and write on one warm `zt mcp` process, with reply bytes and
      the archive's own `last_write` row counts.
   6. Proves fixed local work (R355): the identical fixed edit and read on a control copy and on a
      copy that holds 256 unrelated Cards; the parsed-row and updated-row counts must not grow.
   7. Runs one bounded lock-contention trial (a live lock holder for 6.5 s) and a bounded
      concurrent workload (four parallel applies), recording the daemon's own queue/lock lines.

  Every phase re-checks the full parser (`zt status --check-links`): the derived Link index and the
  stored Reverse sections must equal the parser's result with zero faults. The run writes only to
  -Work; -Archive is read once, to copy it. No digest workflow, provider call or live Card write
  happens anywhere.

  It exits 1 when any assertion fails and always prints one compact JSON summary line last.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Archive,
    [Parameter(Mandatory)][string]$Source,
    [Parameter(Mandatory)][string]$Out,
    [string]$Work = '',
    [ValidateSet('econ', 'math')][string[]]$Variant = @('econ', 'math'),
    [int]$Runs = 10,
    [int]$Concurrency = 4,
    [string]$Zt = '',
    [string]$ZtDigest = '',
    [string]$CitationKey = 'Qualify2026',
    [string]$BookTitle = 'Release qualification book',
    [string]$BibTeX = '@book{Qualify2026,title={Release qualification book},author={Qualification, Ada},year={2026}}',
    [switch]$SkipFixedWork
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$utf8 = [Text.UTF8Encoding]::new($false)
$here = $PSScriptRoot
$repo = (Resolve-Path (Join-Path $here '..\..\..')).Path
$seam = Join-Path $here 'seam-zt-mcp.cmd'
if (-not $Work) { $Work = Join-Path ([IO.Path]::GetTempPath()) "zt-release-qualification-$PID" }
$Work = [IO.Path]::GetFullPath($Work)
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$allowedRoot = $Work.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
    $Work.StartsWith('D:\zt-target\', [StringComparison]::OrdinalIgnoreCase)
if (-not $allowedRoot) { throw "-Work must stay under the temporary folder or D:\zt-target; got $Work" }
$Archive = (Resolve-Path -LiteralPath $Archive).Path
$Source = (Resolve-Path -LiteralPath $Source).Path
if (-not (Test-Path -LiteralPath $seam)) { throw "the public test seam is missing: $seam" }
if (-not $Zt) { $Zt = (Get-Command zt -CommandType Application | Select-Object -First 1).Source }
if (-not $ZtDigest) { $ZtDigest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$ZtCheck = (Get-Command zt-check -ErrorAction SilentlyContinue).Source
if (-not $Zt -or -not $ZtDigest) { throw 'zt and zt-digest must be on PATH or given with -Zt/-ZtDigest' }
$env:PATH = "$(Split-Path -Parent $Zt);$env:PATH"
New-Item -ItemType Directory -Force $Work | Out-Null
$Out = if ([IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path $repo $Out }
New-Item -ItemType Directory -Force (Split-Path -Parent $Out) | Out-Null

$script:failed = 0
$script:checks = 0
$script:roots = [Collections.Generic.List[string]]::new()
$script:server = $null
function Assert($condition, $message) {
    $script:checks++
    if (-not $condition) { $script:failed++; Write-Output "FAIL: $message" } else { Write-Output "ok: $message" }
}
function Section($title) { Write-Output "`n## $title" }
function Get-Sha256([string]$path) {
    if (-not (Test-Path -LiteralPath $path)) { return '' }
    (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Get-Bytes([string]$path) { if (Test-Path -LiteralPath $path) { (Get-Item -LiteralPath $path).Length } else { 0 } }
function Get-TextBytes([string]$text) { [Text.Encoding]::UTF8.GetByteCount([string]$text) }
function Round1($value) { [Math]::Round([double]$value, 1) }
# Nearest-rank percentile over the measured samples (R348 asks for median and p95, not a mean).
function Get-Percentile($values, [double]$p) {
    $sorted = @($values | Sort-Object)
    if (-not $sorted.Count) { return $null }
    $rank = [Math]::Max(1, [Math]::Ceiling($p * $sorted.Count))
    Round1 $sorted[[int]$rank - 1]
}
function Get-Median($values) { Get-Percentile $values 0.5 }
function Invoke-Exe {
    param([string]$exe, [string[]]$arguments, [hashtable]$setenv = @{})
    $saved = @{}
    foreach ($k in $setenv.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k); Set-Item -Path "Env:$k" -Value $setenv[$k] }
    try {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $lines = @(& $exe @arguments 2>&1 | ForEach-Object { "$_" })
        $code = $LASTEXITCODE
        $sw.Stop()
    } finally {
        foreach ($k in $saved.Keys) {
            if ($null -eq $saved[$k]) { Remove-Item -Path "Env:$k" -ErrorAction SilentlyContinue }
            else { Set-Item -Path "Env:$k" -Value $saved[$k] }
        }
    }
    [pscustomobject]@{ exit = $code; ms = Round1 $sw.Elapsed.TotalMilliseconds; lines = $lines; last = (@($lines) | Select-Object -Last 1) }
}
function ConvertFrom-LastJson([string]$text) { try { ($text | ConvertFrom-Json -DateKind String) } catch { $null } }
# A read through `zt-digest call -Tool` prints `<call number> <json>`; a tool that fails prints a line.
function ConvertFrom-CallJson([string]$text) {
    $body = $text -replace '^\d+\s+', ''
    if (-not $body.TrimStart().StartsWith('{')) { return $null }
    try { ($body | ConvertFrom-Json -DateKind String) } catch { $null }
}
# `zt-digest call` prints `n ok <address>` or `n error <message>` per accepted call.
function Get-CallAddresses($lines) {
    @($lines | Where-Object { $_ -match '^\d+ ok (\S+)$' } | ForEach-Object { ($_ -replace '^\d+ ok ', '').Trim() })
}
function Get-CallErrors($lines) { @($lines | Where-Object { $_ -match '^\d+ error ' }) }
# One traced invocation of zt-digest: the public seam forwards to the real `zt mcp` and records
# every call (tool, process id, address count), so call and row counts are observed, not asserted.
function Invoke-Digest {
    param([string[]]$arguments, [string]$trace)
    if (Test-Path -LiteralPath $trace) { Remove-Item -LiteralPath $trace -Force }
    $r = Invoke-Exe -exe $ZtDigest -arguments $arguments -setenv @{ ZT_CHECK_ZT = $seam; ZT_TRACE_TOOLS = $trace }
    $calls = @()
    if (Test-Path -LiteralPath $trace) { $calls = @(Get-Content -LiteralPath $trace -Encoding utf8 | Where-Object { $_ } | ForEach-Object { $_ | ConvertFrom-Json }) }
    $byTool = [ordered]@{}
    foreach ($call in $calls) { $byTool[$call.tool] = 1 + [int]($byTool[$call.tool] ?? 0) }
    $r | Add-Member -NotePropertyName calls -NotePropertyValue $calls -Force
    $r | Add-Member -NotePropertyName calls_by_tool -NotePropertyValue $byTool -Force
    $r | Add-Member -NotePropertyName call_count -NotePropertyValue $calls.Count -Force
    $r | Add-Member -NotePropertyName mcp_processes -NotePropertyValue @(@($calls.process_id | Sort-Object -Unique)).Count -Force
    $r | Add-Member -NotePropertyName addresses -NotePropertyValue ([int](($calls | Measure-Object -Property address_count -Sum).Sum ?? 0)) -Force
    $r | Add-Member -NotePropertyName stdout_bytes -NotePropertyValue (Get-TextBytes ($r.lines -join "`n")) -Force
    $r
}
function Get-LinkCheck {
    $r = Invoke-Exe -exe $Zt -arguments @('status', '--check-links')
    $value = ConvertFrom-LastJson $r.last
    if ($null -eq $value) { throw "zt status --check-links printed no JSON: $($r.lines -join ' | ')" }
    $value
}
function Get-LogLines([string]$root) {
    $log = Join-Path $root 'zt.log'
    if (-not (Test-Path -LiteralPath $log)) { return @() }
    @(Get-Content -LiteralPath $log -Encoding utf8 | Where-Object { $_ })
}
function Select-LogLines($lines, [string]$pattern) { @($lines | Where-Object { $_ -match $pattern }) }
function New-Isolated([string]$root) {
    New-Item -ItemType Directory -Force (Join-Path $root 'archive') | Out-Null
    Copy-Item -LiteralPath $Archive -Destination (Join-Path $root 'archive\zt.sqlite3') -Force
    $script:roots.Add($root)
    $env:ZT_CONFIG_DIR = Join-Path $root 'config'
    $env:ZT_GUI_PORT = '0'
    $null = Invoke-Exe -exe $Zt -arguments @('config', 'set', 'archive_root', (Join-Path $root 'archive'))
    $up = Invoke-Exe -exe $Zt -arguments @('up')
    if ($up.exit -ne 0) { throw "zt up failed for $root : $($up.lines -join ' | ')" }
    if ($script:server) { Stop-WarmServer }
}
function Stop-Isolated([string]$root) {
    Set-IsolatedEnv $root
    if ($script:server) { Stop-WarmServer }
    $null = Invoke-Exe -exe $Zt -arguments @('down')
}
function Set-IsolatedEnv([string]$root) {
    $env:ZT_CONFIG_DIR = Join-Path $root 'config'
    $env:ZT_GUI_PORT = '0'
}
# One stable identity for a command's own output: absolute paths are replaced so that the same
# input gives the same hash on two copies of the archive.
function Get-ProjectionHash([string]$text, [string]$root) {
    $normalized = $text.Replace($root, '<root>').Replace($root.Replace('\', '/'), '<root>')
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($normalized))) -replace '-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
}

# ---------------------------------------------------------------------------------------------
# One warm `zt mcp` process for the ordinary read/write trials (R348/R353): a fresh process per
# call would measure process startup instead of archive work.
# ---------------------------------------------------------------------------------------------
function Start-WarmServer {
    if ($script:server) { return }
    $start = [Diagnostics.ProcessStartInfo]::new($Zt, 'mcp')
    $start.RedirectStandardInput = $true; $start.RedirectStandardOutput = $true; $start.RedirectStandardError = $true
    $start.UseShellExecute = $false; $start.CreateNoWindow = $true
    $start.StandardInputEncoding = [Text.UTF8Encoding]::new($false); $start.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $process = [Diagnostics.Process]::Start($start)
    $process.BeginErrorReadLine()
    $script:server = @{ process = $process; next = 1 }
    $null = Send-Warm @{ jsonrpc = '2.0'; id = 0; method = 'initialize'; params = @{ protocolVersion = '2025-06-18' } }
}
function Stop-WarmServer {
    if (-not $script:server) { return }
    $process = $script:server.process
    $script:server = $null
    try { $process.StandardInput.Close() } catch { }
    if (-not $process.WaitForExit(5000)) { try { $process.Kill() } catch { } }
    $process.Dispose()
}
function Send-Warm($message) {
    $script:server.process.StandardInput.WriteLine(($message | ConvertTo-Json -Depth 50 -Compress))
    $script:server.process.StandardInput.Flush()
}
function Receive-Warm([int]$id) {
    $stream = $script:server.process.StandardOutput
    while ($null -ne ($line = $stream.ReadLine())) {
        $reply = $line | ConvertFrom-Json -AsHashtable -DateKind String
        if ($reply.id -eq $id) { return @{ reply = $reply; bytes = (Get-TextBytes $line) } }
    }
    throw 'the warm zt mcp process closed'
}
function Invoke-Warm([string]$tool, $arguments) {
    $id = $script:server.next++
    $sw = [Diagnostics.Stopwatch]::StartNew()
    Send-Warm @{ jsonrpc = '2.0'; id = $id; method = 'tools/call'; params = @{ name = $tool; arguments = $arguments } }
    $got = Receive-Warm $id
    $sw.Stop()
    $reply = $got.reply
    $text = ($reply.result.content | ForEach-Object { $_.text }) -join "`n"
    $value = $null
    if ($text.TrimStart().StartsWith('{')) { $value = $text | ConvertFrom-Json -DateKind String }
    [pscustomobject]@{ ms = Round1 $sw.Elapsed.TotalMilliseconds; bytes = $got.bytes; failed = [bool]($reply.error -or $reply.result.isError); value = $value; text = $text }
}

# The live lock holder of the one contention trial: an OS-exclusive `zt.edit.lock` carrying the
# marker the ordinary path looks for, released after the declared hold.
$lockHelper = @'
param([string]$Root, [int]$HoldMs)
$path = Join-Path $Root 'zt.edit.lock'
Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
$file = [IO.File]::Open($path, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
try {
  $writer = [IO.StreamWriter]::new($file, [Text.UTF8Encoding]::new($false))
  $writer.Write("zt edit lock`npid $PID"); $writer.Flush()
  Start-Sleep -Milliseconds $HoldMs
  $writer.Dispose()
} finally { $file.Dispose() }
Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
'@
$lockHelperPath = Join-Path $Work 'hold-edit-lock.ps1'
[IO.File]::WriteAllText($lockHelperPath, $lockHelper, $utf8)
# The per-process runner of the bounded concurrent workload.
$concurrentRunner = @'
param([string]$Digest, [string]$File, [string]$Prefix, [string]$Trace, [string]$Seam, [string]$Out)
$env:ZT_CHECK_ZT = $Seam
$env:ZT_TRACE_TOOLS = $Trace
$lines = @(& $Digest call $File -OperationPrefix $Prefix 2>&1 | ForEach-Object { "$_" })
[IO.File]::WriteAllLines($Out, $lines, [Text.UTF8Encoding]::new($false))
exit $LASTEXITCODE
'@
$runnerPath = Join-Path $Work 'run-identified-apply.ps1'
[IO.File]::WriteAllText($runnerPath, $concurrentRunner, $utf8)

# A bounded identified-batch fixture: one Topic, one formal Model Card and 66 parts in one chain,
# plus the Model Card's structure list. 69 calls become 64+5 archive chunks.
function New-ModelCallFile([int]$n, [string]$title, [string]$path) {
    $calls = [Collections.Generic.List[object]]::new()
    $calls.Add(@{ tool = 'create_topic'; arguments = @{ title = $title; body = "第 $n 个限定模型的入口。" } })
    $calls.Add(@{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '限定模型'; type = 'model'; level = 'formal'; body = '模型的问题与机制。' } })
    for ($i = 2; $i -lt 68; $i++) {
        $previous = $i - 1
        $calls.Add(@{ tool = 'create_successor'; arguments = @{ at = "@@$previous"; kind = $(if ($i -eq 2) { 'side' } else { 'direct' }); type = 'model'; facet = $(if ($i -eq 2) { '设定' } else { "推导：步骤 $i" }); title = "步骤 $i"; body = "这一步依赖 [[@@$previous]]。" } })
    }
    $calls.Add(@{ tool = 'edit_card'; arguments = @{ at = '@@1'; body = (2..67 | ForEach-Object { "- [[@@$_]] 步骤 $_" }) -join "`n" } })
    [IO.File]::WriteAllText($path, ($calls.ToArray() | ConvertTo-Json -Depth 12 -AsArray), $utf8)
    $path
}
# 256 unrelated Cards: four Topics, one Direct successor each, then 63 Side successors per Topic.
function New-UnrelatedCallFile([string]$tag, [string]$path) {
    $calls = [Collections.Generic.List[object]]::new()
    for ($t = 1; $t -le 4; $t++) {
        $topic = $calls.Count
        $calls.Add(@{ tool = 'create_topic'; arguments = @{ title = "无关卡片 $tag $t"; body = "无关卡片组 $t。" } })
        $first = $calls.Count
        $calls.Add(@{ tool = 'create_successor'; arguments = @{ at = "@@$topic"; title = "无关卡片 $tag $t-00"; body = "无关正文 $tag $t-00。" } })
        for ($i = 1; $i -lt 64; $i++) {
            $calls.Add(@{ tool = 'create_successor'; arguments = @{ at = "@@$first"; kind = 'side'; title = ("无关卡片 $tag $t-{0:00}" -f $i); body = "无关正文 $tag $t-{0:00}。" -f $i } })
        }
    }
    [IO.File]::WriteAllText($path, ($calls.ToArray() | ConvertTo-Json -Depth 12 -AsArray), $utf8)
    $path
}

# ---------------------------------------------------------------------------------------------
# Input facts (R347): machine, release hashes, archive identity.
# ---------------------------------------------------------------------------------------------
Section 'inputs'
$facts = [ordered]@{
    generated_utc = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    host = [ordered]@{
        os = [Environment]::OSVersion.VersionString
        psversion = $PSVersionTable.PSVersion.ToString()
        processors = [Environment]::ProcessorCount
        machine = $env:COMPUTERNAME
    }
    archive = [ordered]@{ source = $Archive; source_sha256 = Get-Sha256 $Archive; source_bytes = Get-Bytes $Archive }
    source = [ordered]@{ path = $Source; sha256 = Get-Sha256 $Source; bytes = Get-Bytes $Source }
    binaries = [ordered]@{
        zt = [ordered]@{ path = $Zt; sha256 = Get-Sha256 $Zt; version = ((Invoke-Exe -exe $Zt -arguments @('version')).lines -join ' ') }
        zt_check = [ordered]@{ path = $ZtCheck; sha256 = Get-Sha256 $ZtCheck }
        zt_digest = [ordered]@{ path = $ZtDigest; sha256 = Get-Sha256 $ZtDigest; version = ((Invoke-Exe -exe $ZtDigest -arguments @('version')).lines -join ' ') }
    }
    runs = $Runs
    concurrency = $Concurrency
}
Assert ($facts.archive.source_bytes -gt 0) "the archive fixture exists ($($facts.archive.source_bytes) bytes, sha256 $($facts.archive.source_sha256.Substring(0, 12))...)"
Assert ($facts.binaries.zt.version -match '^zt ') "the measured backend is a zt binary: $($facts.binaries.zt.version)"
Assert ($facts.binaries.zt_digest.version -match '^zt-digest ') "the measured client is a zt-digest binary: $($facts.binaries.zt_digest.version)"

$results = [ordered]@{ variants = [ordered]@{} }
try {
    foreach ($v in $Variant) {
        $root = Join-Path $Work "variant-$v"
        Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
        New-Isolated $root
        $vroot = Join-Path $root 'archive'
        $staging = Join-Path $root 'staging'
        $cardsWork = Join-Path $root 'cards'
        Section "variant $v"
        $status = Invoke-Exe -exe $Zt -arguments @('status')
        Assert ((($status.lines -join ' ') -match 'state: up')) "the isolated $v service is up"
        $baseline = Get-LinkCheck
        Assert ($baseline.ok -eq $true -and @($baseline.faults).Count -eq 0) "the $v copy passes the full-parser Link check: $($baseline.cards) Cards, $($baseline.links) occurrences, revision $($baseline.archive_revision)"
        $baselineHash = Get-Sha256 (Join-Path $vroot 'zt.sqlite3')

        # --- the prepared run (one real -Apply per variant) ------------------------------------
        $prepare = Invoke-Exe -exe $ZtDigest -arguments @('prepare', '-Variant', $v, '-Source', $Source, '-Out', $staging, '-CitationKey', $CitationKey,
            '-BookTitle', $BookTitle, '-BibTeX', $BibTeX, '-MinUnitChars', '200', '-MaxUnits', '2', '-NoFigures', '-Apply')
        $manifest = ConvertFrom-LastJson $prepare.last
        Assert ($prepare.exit -eq 0 -and $null -ne $manifest) "prepare -Variant $v -Apply completes"
        $groups = if ($manifest) { @($manifest.groups).Count } else { 0 }
        Assert ($groups -ge 2) "the prepared $v run holds $groups groups"
        $afterPrepare = Get-LinkCheck
        Assert ($afterPrepare.ok -eq $true) "the archive still passes the full-parser Link check after prepare ($($afterPrepare.cards) Cards, revision $($afterPrepare.archive_revision))"

        # --- the named tool targets (R349-R351) ------------------------------------------------
        $commands = @(
            [pscustomobject]@{ name = 'snapshot-topics'; target = 1000; args = @('snapshot-topics', '-Out', (Join-Path $staging 'snapshot-qualification.md')) }
            [pscustomobject]@{ name = 'card-plan'; target = 10000; args = @('card', '-Step', 'plan', '-Book', $CitationKey, '-Work', $cardsWork, '-Staging', $staging, '-NoOrphans', '-Sample', '0') }
            [pscustomobject]@{ name = 'card-check'; target = 10000; args = @('card', '-Step', 'check', '-Work', $cardsWork) }
            [pscustomobject]@{ name = 'card-report'; target = 10000; args = @('card', '-Step', 'report', '-Work', $cardsWork) }
            [pscustomobject]@{ name = 'prepare-link'; target = 20000; args = @('prepare-link', '-Staging', $staging) }
        )
        $toolResults = [Collections.Generic.List[object]]::new()
        foreach ($command in $commands) {
            # The traced warm-up records the calls, requested addresses and process count; the
            # measured runs stay untraced so their latency is the command's, not the seam's.
            $trace = Join-Path $root ("trace-" + $command.name + ".jsonl")
            $warm = Invoke-Digest -arguments $command.args -trace $trace
            if ($warm.exit -ne 0) { throw "$($command.name) failed before its warm-up completed: $(($warm.lines | Select-Object -Last 2) -join ' | ')" }
            $samples = [Collections.Generic.List[double]]::new()
            $exits = [Collections.Generic.List[int]]::new()
            $last = $null
            for ($i = 0; $i -lt $Runs; $i++) {
                $run = Invoke-Exe -exe $ZtDigest -arguments $command.args
                $samples.Add($run.ms); $exits.Add($run.exit); $last = $run
            }
            $median = Get-Median $samples
            $p95 = Get-Percentile $samples 0.95
            $okExits = @($exits | Where-Object { $_ -eq 0 }).Count
            Assert ($okExits -eq $Runs) "$($command.name) exits 0 on all $Runs measured runs of the $v variant"
            Assert ($median -le $command.target) "$($command.name) median $median ms is inside its $($command.target) ms target ($v)"
            if ($command.name -eq 'snapshot-topics') {
                $snapshotFile = Join-Path $staging 'snapshot-qualification.md'
                $projection = Get-Sha256 $snapshotFile
                $artifactBytes = Get-Bytes $snapshotFile
            } else {
                $projection = Get-ProjectionHash ([string]$last.last) $root
                $artifactBytes = Get-TextBytes ([string]$last.last)
            }
            $toolResults.Add([ordered]@{
                    command = $command.name; target_ms = $command.target
                    samples_ms = @($samples); median_ms = $median; p95_ms = $p95
                    max_ms = ((($samples | Measure-Object -Maximum).Maximum))
                    exits = @($exits); calls = $warm.calls_by_tool; call_count = $warm.call_count
                    mcp_processes = $warm.mcp_processes; addresses_requested = $warm.addresses
                    stdout_bytes = $last.stdout_bytes; artifact_bytes = $artifactBytes; projection_sha256 = $projection
                    call_evidence = 'the traced warm-up; the measured runs are untraced so their latency is the command own'
                })
            Write-Output ("     {0,-16} median {1,8} ms  p95 {2,8} ms  calls {3,3}  addresses {4,5}  bytes {5}" -f $command.name, $median, $p95, $warm.call_count, $warm.addresses, $artifactBytes)
        }
        $afterTools = Get-LinkCheck
        Assert ($afterTools.ok -eq $true -and @($afterTools.faults).Count -eq 0) "the $v full-parser Link check still passes after the measured tool runs"
        Assert ($afterTools.archive_revision -eq $afterPrepare.archive_revision) "the measured read-only tool runs advance no archive revision ($($afterTools.archive_revision))"

        # --- model apply (R352) ----------------------------------------------------------------
        $warmFile = New-ModelCallFile 0 'Qualification model warm-up' (Join-Path $root 'model-warm.json')
        $warmApply = Invoke-Digest -arguments @('call', $warmFile, '-OperationPrefix', "qualification-$v/model-warm/1/build") -trace (Join-Path $root 'trace-model-warm.jsonl')
        if ($warmApply.exit -ne 0) { throw "the model-apply warm-up failed: $(($warmApply.lines | Select-Object -Last 2) -join ' | ')" }
        $revisionBefore = (Get-LinkCheck).archive_revision
        $applySamples = [Collections.Generic.List[double]]::new()
        $chunkShapes = [Collections.Generic.List[string]]::new()
        $applyRuns = [Collections.Generic.List[object]]::new()
        for ($i = 1; $i -le $Runs; $i++) {
            $file = New-ModelCallFile $i "Qualification model $v $i" (Join-Path $root 'model-calls.json')
            $trace = Join-Path $root ("trace-model-$i.jsonl")
            $run = Invoke-Digest -arguments @('call', $file, '-OperationPrefix', "qualification-$v/model-$i/1/build") -trace $trace
            $chunks = (@($run.calls | Where-Object { $_.tool -eq 'apply_batch' }) | ForEach-Object { $_.item_count })
            $chunkShapes.Add(($chunks -join ','))
            $applySamples.Add($run.ms)
            $applyRuns.Add([ordered]@{ run = $i; exit = $run.exit; ms = $run.ms; chunks = @($chunks); accepted = @(Get-CallAddresses $run.lines).Count })
        }
        $applyMedian = Get-Median $applySamples
        $applyP95 = Get-Percentile $applySamples 0.95
        Assert (@($applyRuns | Where-Object { $_.exit -eq 0 }).Count -eq $Runs) "model apply commits through the identified-batch helper on all $Runs runs ($v): chunks $(@($chunkShapes | Sort-Object -Unique) -join '|')"
        Assert ($applyMedian -le 30000) "model apply median $applyMedian ms is inside its 30000 ms target ($v)"
        Assert ((@($chunkShapes | Sort-Object -Unique) -join '|') -eq '64,5') "every model apply splits into the same two bounded archive chunks (64 + 5 items)"
        $revisionAfter = (Get-LinkCheck).archive_revision
        Assert (($revisionAfter - $revisionBefore) -eq (2 * $Runs)) "each model apply publishes exactly two revisions: $($revisionAfter - $revisionBefore) for $Runs applies"
        # Receipts are the commit authority: a complete replay publishes nothing and duplicates nothing.
        $replay = Invoke-Digest -arguments @('call', (Join-Path $root 'model-calls.json'), '-OperationPrefix', "qualification-$v/model-$Runs/1/build") -trace (Join-Path $root 'trace-model-replay.jsonl')
        $replayRevision = (Get-LinkCheck).archive_revision
        $acceptedAddresses = @(Get-CallAddresses $replay.lines)
        Assert ($replay.exit -eq 0 -and $replayRevision -eq $revisionAfter) "a complete replay of the accepted model plan publishes no revision and duplicates no Card"
        Assert ($acceptedAddresses.Count -eq 69) "the replayed plan returns the same 69 accepted addresses ($($acceptedAddresses.Count))"
        $modelCard = if ($acceptedAddresses.Count -ge 2) { $acceptedAddresses[1] } else { '' }
        $modelRead = Invoke-Digest -arguments @('call', '-Tool', 'get_model', '-Json', (ConvertTo-Json -Compress -InputObject @{ at = $modelCard })) -trace (Join-Path $root 'trace-model-read.jsonl')
        $model = ConvertFrom-CallJson ([string]$modelRead.last)
        Assert ($null -ne $model -and @($model.parts).Count -eq 66) "the applied model's part map is stable: 66 parts under $modelCard"
        $logLines = Get-LogLines $vroot
        $applyLockLines = @(Select-LogLines $logLines 'edit (wait|lock)|edit in progress')
        $applyLastWrite = (Get-LinkCheck).last_write

        # --- ordinary read/write on one warm process (R353) ------------------------------------
        $topicCall = Invoke-Digest -arguments @('call', '-Tool', 'create_topic', '-Json', ('{"title":"Qualification ordinary ' + $v + '","body":"限定读写的固定卡片。"}')) -trace (Join-Path $root 'trace-fixture.jsonl')
        $fixtureTopic = @(Get-CallAddresses $topicCall.lines)[0]
        $created = Invoke-Digest -arguments @('call', '-Tool', 'create_successor', '-Json', (ConvertTo-Json -Compress -InputObject @{ at = $fixtureTopic; title = 'Ordinary write target'; body = '第一版正文，用于限定读写测量。' })) -trace (Join-Path $root 'trace-fixture2.jsonl')
        $target = @(Get-CallAddresses $created.lines)[0]
        Start-WarmServer
        $null = Invoke-Warm 'get_card' @{ at = $target }
        $readSamples = [Collections.Generic.List[double]]::new(); $readBytes = [Collections.Generic.List[int]]::new()
        for ($i = 0; $i -lt $Runs; $i++) {
            $r = Invoke-Warm 'get_card' @{ at = $target }
            Assert (-not $r.failed) "ordinary Card read $i of $v succeeds"
            $readSamples.Add($r.ms); $readBytes.Add($r.bytes)
        }
        $writeSamples = [Collections.Generic.List[double]]::new(); $writeStats = @()
        for ($i = 0; $i -lt $Runs; $i++) {
            $body = if ($i % 2 -eq 0) { '第二版正文，用于限定读写测量。' } else { '第一版正文，用于限定读写测量。' }
            $r = Invoke-Warm 'edit_card' @{ at = $target; body = $body }
            Assert (-not $r.failed) "ordinary Card write $i of $v succeeds"
            $writeSamples.Add($r.ms)
            $writeStats = (Get-LinkCheck).last_write
        }
        $readMedian = Get-Median $readSamples; $readP95 = Get-Percentile $readSamples 0.95
        $writeMedian = Get-Median $writeSamples; $writeP95 = Get-Percentile $writeSamples 0.95
        Assert ($readMedian -le 60) "ordinary Card read median $readMedian ms is in the local-millisecond band ($v)"
        Assert ($writeMedian -le 120) "ordinary Card write median $writeMedian ms is in the local-millisecond band ($v)"
        Assert ($writeStats.indexed_cards -eq 1) "one ordinary write indexes exactly its one changed Card ($($writeStats.indexed_cards))"
        $afterWrites = Get-LinkCheck
        Assert ($afterWrites.ok -eq $true -and @($afterWrites.faults).Count -eq 0) "the $v full-parser Link check passes after the ordinary reads and writes"

        # --- one bounded contention trial: a live lock holder (R348 queue/lock evidence) --------
        $logMark = (Get-LogLines $vroot).Count
        $holder = Start-Process pwsh -ArgumentList @('-NoProfile', '-File', $lockHelperPath, '-Root', $vroot, '-HoldMs', '6500') -PassThru -WindowStyle Hidden
        Start-Sleep -Milliseconds 600
        $contendedWatch = [Diagnostics.Stopwatch]::StartNew()
        $contended = Invoke-Warm 'edit_card' @{ at = $target; body = '第三版正文，在一位真实的锁持有者之后写入。' }
        $contendedWatch.Stop()
        if (-not $holder.WaitForExit(20000)) { try { $holder.Kill() } catch { } }
        $contendedMs = Round1 $contendedWatch.Elapsed.TotalMilliseconds
        $waitLines = @((Get-LogLines $vroot) | Select-Object -Skip $logMark | Where-Object { $_ -match 'edit wait' })
        $waitLine = @($waitLines | Where-Object { $_ -match 'edit wait done' }) | Select-Object -Last 1
        $parsedWait = 0; $parsedHeld = 0
        if ($waitLine -and $waitLine -match 'queue_wait_ms=(\d+) held_ms=(\d+)') { $parsedWait = [int]$Matches[1]; $parsedHeld = [int]$Matches[2] }
        Assert (-not $contended.failed -and $contendedMs -ge 5000) "the ordinary write waited $contendedMs ms for the live lock holder and then committed"
        Assert ($parsedWait -ge 5000) "the daemon recorded the queue wait apart from the hold ($waitLine)"

        # --- a bounded concurrent workload: four parallel identified applies -------------------
        $jobs = @()
        for ($i = 1; $i -le $Concurrency; $i++) {
            $file = New-ModelCallFile (100 + $i) "Qualification concurrent $v $i" (Join-Path $root "concurrent-$i.json")
            $jobs += [pscustomobject]@{ file = $file; prefix = "qualification-$v/concurrent-$i/1/build"; trace = (Join-Path $root "trace-concurrent-$i.jsonl"); out = (Join-Path $root "concurrent-$i.out") }
        }
        $revisionBeforeConcurrent = (Get-LinkCheck).archive_revision
        $concurrentLogMark = (Get-LogLines $vroot).Count
        $processes = @($jobs | ForEach-Object {
                Start-Process pwsh -ArgumentList @('-NoProfile', '-File', $runnerPath, '-Digest', $ZtDigest, '-File', $_.file, '-Prefix', $_.prefix, '-Trace', $_.trace, '-Seam', $seam, '-Out', $_.out) -PassThru -WindowStyle Hidden
            })
        $deadline = (Get-Date).AddMinutes(4)
        while (@($processes | Where-Object { -not $_.HasExited }).Count -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200 }
        $codes = @($processes | ForEach-Object { if (-not $_.HasExited) { try { $_.Kill() } catch { } }; $_.WaitForExit(5000) | Out-Null; $_.ExitCode })
        $concurrentOk = @($codes | Where-Object { $_ -eq 0 }).Count
        $concurrencyLockLines = @((Get-LogLines $vroot) | Select-Object -Skip $concurrentLogMark | Where-Object { $_ -match 'edit (wait|lock)|edit in progress' })
        Assert ($concurrentOk -eq $Concurrency) "all $Concurrency concurrent identified applies commit without a lock failure (exit codes $($codes -join ','))"
        $afterConcurrent = Get-LinkCheck
        Assert (($afterConcurrent.archive_revision - $revisionBeforeConcurrent) -eq (2 * $Concurrency)) "the concurrent applies publish exactly two revisions each"
        Assert ($afterConcurrent.ok -eq $true -and @($afterConcurrent.faults).Count -eq 0) "the $v full-parser Link check passes after the concurrent workload"

        Stop-WarmServer
        $final = Get-LinkCheck
        Assert ($final.ok -eq $true) "the $v release copy ends with a passing full-parser Link check"
        $results.variants[$v] = [ordered]@{
            prepare = [ordered]@{
                exit = $prepare.exit; ms = $prepare.ms; groups = $groups; citation_key = $CitationKey
                manifest_sha256 = (Get-Sha256 (Join-Path $staging 'manifest.json'))
                ledger_sha256 = (Get-Sha256 (Join-Path $staging 'ledger.sqlite3'))
            }
            link_check = [ordered]@{
                before = $baseline; after_prepare = $afterPrepare; after_tools = $afterTools; final = $final
                archive_sha256_before = $baselineHash; archive_sha256_final = (Get-Sha256 (Join-Path $vroot 'zt.sqlite3'))
            }
            tools = @($toolResults)
            model_apply = [ordered]@{
                runs = @($applyRuns); samples_ms = @($applySamples); median_ms = $applyMedian; p95_ms = $applyP95
                max_ms = ((($applySamples | Measure-Object -Maximum).Maximum))
                chunk_shapes = @($chunkShapes); part_map = [ordered]@{ model = $modelCard; parts = $(if ($model) { @($model.parts).Count } else { 0 }) }
                revisions = ($revisionAfter - $revisionBefore); replay_revision_delta = ($replayRevision - $revisionAfter)
                accepted_replayed = $acceptedAddresses.Count
                last_write = $applyLastWrite
                lock_log_lines = $applyLockLines
            }
            ordinary = [ordered]@{
                target = $target
                read_ms = [ordered]@{ samples = @($readSamples); median_ms = $readMedian; p95_ms = $readP95; bytes = @($readBytes) }
                write_ms = [ordered]@{ samples = @($writeSamples); median_ms = $writeMedian; p95_ms = $writeP95; last_write = $writeStats }
            }
            contention = [ordered]@{ waited_ms = $contendedMs; committed = (-not $contended.failed); queue_wait_ms = $parsedWait; held_ms = $parsedHeld; log_line = $waitLine }
            concurrency = [ordered]@{ writes = $Concurrency; succeeded = $concurrentOk; exit_codes = @($codes); revisions = ($afterConcurrent.archive_revision - $revisionBeforeConcurrent); lock_lines = $concurrencyLockLines }
        }
        Stop-Isolated $root
    }

    # --- fixed local work against unrelated Cards (R355) ----------------------------------------
    if (-not $SkipFixedWork) {
        Section 'fixed local work (R355)'
        $copies = [ordered]@{}
        foreach ($name in @('control', 'big')) {
            $root = Join-Path $Work "fixed-$name"
            Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
            New-Isolated $root
            $vroot = Join-Path $root 'archive'
            $trace = Join-Path $root 'trace-fixture.jsonl'
            $fixture = @(
                @{ tool = 'create_topic'; arguments = @{ title = 'Qualification fixed work'; body = '限定局部工作的固定主题。' } }
                @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = 'Fixed write target'; body = '第一版正文，未链接任何卡片。' } }
                @{ tool = 'create_successor'; arguments = @{ at = '@@1'; kind = 'side'; title = 'Neighbour A'; body = '相邻卡片甲。' } }
                @{ tool = 'create_successor'; arguments = @{ at = '@@1'; kind = 'side'; title = 'Neighbour B'; body = '相邻卡片乙。' } }
            )
            $fixturePath = Join-Path $root 'fixture.json'
            [IO.File]::WriteAllText($fixturePath, ($fixture | ConvertTo-Json -Depth 8 -AsArray), $utf8)
            $fixtureRun = Invoke-Digest -arguments @('call', $fixturePath) -trace $trace
            if ($fixtureRun.exit -ne 0) { throw "the fixed fixture of $name did not build: $(($fixtureRun.lines | Select-Object -Last 2) -join ' | ')" }
            $addresses = @(Get-CallAddresses $fixtureRun.lines)
            $fixedCard = $addresses[1]
            $neighbours = @($addresses[2], $addresses[3])
            $unrelated = 0
            $unrelatedMs = 0
            if ($name -eq 'big') {
                $unrelatedPath = New-UnrelatedCallFile $name (Join-Path $root 'unrelated.json')
                $before = (Get-LinkCheck).archive_revision
                $unrelatedRun = Invoke-Digest -arguments @('call', $unrelatedPath) -trace (Join-Path $root 'trace-unrelated.jsonl')
                if ($unrelatedRun.exit -ne 0) { throw "the unrelated Cards of $name were not created: $(($unrelatedRun.lines | Select-Object -Last 2) -join ' | ')" }
                $unrelated = @(Get-CallAddresses $unrelatedRun.lines).Count
                $unrelatedMs = $unrelatedRun.ms
                Assert ($unrelated -eq 260) "the big copy holds $unrelated new addresses (4 Topics and 256 unrelated Cards)"
                Assert (((Get-LinkCheck).archive_revision) -gt $before) "the unrelated Cards published their own revisions"
            }
            Start-WarmServer
            $bodyA = "第二版正文，参照 [[$($neighbours[0])]] 与 [[$($neighbours[1])]]，只改动这一张卡。"
            $bodyB = "第三版正文，参照 [[$($neighbours[0])]] 与 [[$($neighbours[1])]]，措辞不同而链接不变。"
            $firstWrite = Invoke-Warm 'edit_card' @{ at = $fixedCard; body = $bodyA }
            $firstStats = (Get-LinkCheck).last_write
            $samples = [Collections.Generic.List[double]]::new()
            for ($i = 0; $i -lt $Runs; $i++) {
                $r = Invoke-Warm 'edit_card' @{ at = $fixedCard; body = $(if ($i % 2 -eq 0) { $bodyB } else { $bodyA }) }
                Assert (-not $r.failed) "the fixed edit $i on the $name copy succeeds"
                $samples.Add($r.ms)
            }
            $lastStats = (Get-LinkCheck).last_write
            $read = Invoke-Warm 'get_cards' @{ addresses = @($fixedCard) + $neighbours; view = 'text' }
            $single = Invoke-Warm 'get_card' @{ at = $fixedCard }
            Stop-WarmServer
            # The scoped reply carries the copy's own archive revision, so the parity check is on the
            # returned Card payload: the same three Cards with the same authored text and Links.
            $readPayload = if ($read.value) { Get-ProjectionHash (ConvertTo-Json -Compress -Depth 12 -InputObject @($read.value.cards)) $root } else { '' }
            $copies[$name] = [ordered]@{
                name = $name
                archive_cards = (Get-LinkCheck).cards
                fixed_card = $fixedCard; neighbours = $neighbours; unrelated_addresses = $unrelated; unrelated_ms = $unrelatedMs
                edit_ms = [ordered]@{ samples = @($samples); median_ms = (Get-Median $samples); p95_ms = (Get-Percentile $samples 0.95) }
                first_write = $firstStats; last_write = $lastStats
                get_cards_ms = $read.ms; get_cards_bytes = $read.bytes
                get_cards_payload_sha256 = $readPayload
                get_cards_count = $(if ($read.value) { @($read.value.cards).Count } else { 0 })
                get_card_ms = $single.ms; get_card_bytes = $single.bytes
                link_check_ok = (Get-LinkCheck).ok
            }
            Stop-Isolated $root
        }
        $control = $copies['control']; $big = $copies['big']
        Assert ($control.first_write.indexed_cards -eq 1 -and $big.first_write.indexed_cards -eq 1) "the first fixed edit indexes exactly one Card in both copies ($($control.first_write.indexed_cards) / $($big.first_write.indexed_cards))"
        Assert ($control.first_write.refreshed_cards -eq $big.first_write.refreshed_cards) "the refreshed-row count is the same in both copies ($($control.first_write.refreshed_cards) / $($big.first_write.refreshed_cards))"
        Assert ($control.last_write.indexed_cards -eq 1 -and $big.last_write.indexed_cards -eq 1) "a repeated fixed edit stays at one indexed Card ($($control.last_write.indexed_cards) / $($big.last_write.indexed_cards))"
        Assert ($control.last_write.refreshed_cards -eq $big.last_write.refreshed_cards) "the repeated fixed edit refreshes the same rows regardless of 256 unrelated Cards ($($control.last_write.refreshed_cards) / $($big.last_write.refreshed_cards))"
        Assert ($control.last_write.changed_reverse_sections -eq $big.last_write.changed_reverse_sections) "the changed-reverse-section count does not grow with the unrelated archive"
        Assert ($control.get_cards_payload_sha256 -eq $big.get_cards_payload_sha256 -and $control.get_cards_count -eq 3) "the fixed scoped read returns the same three Cards with the same payload on both copies ($($control.get_cards_count) Cards, $($control.get_cards_bytes) / $($big.get_cards_bytes) bytes)"
        Assert ($big.edit_ms.median_ms -le 150) "the fixed edit stays in the local-millisecond band on the big copy ($($big.edit_ms.median_ms) ms)"
        Assert ($big.link_check_ok -eq $true -and $control.link_check_ok -eq $true) 'both fixed-work copies end with a passing full-parser Link check'
        $results['fixed_local_work'] = [ordered]@{
            runs = $Runs
            control = $control
            big = $big
            parsed_rows_bounded = ($control.last_write.indexed_cards -eq $big.last_write.indexed_cards)
            updated_rows_bounded = ($control.last_write.refreshed_cards -eq $big.last_write.refreshed_cards)
            payload_identical = ($control.get_cards_payload_sha256 -eq $big.get_cards_payload_sha256)
            unrelated_cards = $big.unrelated_addresses
        }
    }
} catch {
    $where = "line $($_.InvocationInfo.ScriptLineNumber): $($_.InvocationInfo.Line.Trim())"
    Write-Output "FAIL: $($_.Exception.Message) at $where"
    $script:failed++
    $results['error'] = "$($_.Exception.Message) at $where"
} finally {
    if ($script:server) { Stop-WarmServer }
    foreach ($root in $script:roots) {
        Set-IsolatedEnv $root
        $null = Invoke-Exe -exe $Zt -arguments @('down')
    }
}

# ---------------------------------------------------------------------------------------------
# Evidence and verdict
# ---------------------------------------------------------------------------------------------
$summary = [ordered]@{
    checks = $script:checks
    failed = $script:failed
    variants = @($results.variants.Keys)
    generated_utc = $facts.generated_utc
}
$evidence = [ordered]@{
    schema_version = 1
    issue = 89
    scope = 'Isolated release qualification of both frozen digest variants: consistent archive copies, release hashes, warm-up and 10 measured runs with median/p95, call/row/byte/update/queue/lock evidence, full-parser parity and fixed-local-work scaling. No digest workflow, provider call or live Card write ran.'
    facts = $facts
    results = $results
    summary = $summary
}
[IO.File]::WriteAllText($Out, ($evidence | ConvertTo-Json -Depth 12), $utf8)
Write-Output "`n## evidence"
Write-Output "written: $Out"
Write-Output ("{0} checks, {1} failed" -f $script:checks, $script:failed)
Write-Output (([ordered]@{ ok = ($script:failed -eq 0); checks = $script:checks; failed = $script:failed; evidence = $Out; variants = @($results.variants.Keys) }) | ConvertTo-Json -Compress)
exit $(if ($script:failed) { 1 } else { 0 })







