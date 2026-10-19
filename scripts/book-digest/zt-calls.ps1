#Requires -Version 7.5
<#
.SYNOPSIS
  Call zt MCP tools from a book-digest script. Dot-source it: . (Join-Path $PSScriptRoot 'zt-calls.ps1')

.DESCRIPTION
  Invoke-Zt runs an array of calls through the zt helper (skills\zt\scripts\zt-mcp.ps1) and returns
  the JSON result of each read call, in order. It throws when the helper fails or a call returns
  `N error`.

  The helper writes UTF-8. PowerShell decodes a child's output with [Console]::OutputEncoding,
  which is the console's code page (GBK on this machine) unless a script sets it. Decoded as GBK,
  a Chinese excerpt can swallow a quote and break the JSON, so this file sets UTF-8 for every
  script that loads it.
#>
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)

# zt-check (tools/zt-check, installed by scripts/install-zt-check.ps1) is the one Card quality
# check: the Card language/structural rules R2 to R21 and the Card layer rules I, E, F, L. Invoke-ZtCheck takes
# the parameter names the scripts use (-Json, -Model, -Addresses, -Cards, -Log, -Book, -Rules,
# -Setting, -Level, -RequireExplain, -SkipShared, -MaxSentences, ...) and returns zt-check's
# lines: one per fault, then `faults: N`. It throws when zt-check does not run.
$script:ZtCheck = (Get-Command zt-check -ErrorAction SilentlyContinue).Source
function Invoke-ZtCheck {
    param(
        [string]$Json = '', [string]$Model = '', [string]$Addresses = '', [string]$Cards = '', [string]$Log = '',
        [string]$Book = '', [string]$Rules = '', [string]$Setting = '', [string]$Level = '',
        [switch]$RequireExplain, [switch]$SkipShared,
        [int]$MaxSentences = 12, [int]$MaxChars = 400, [int]$MaxSteps = 6, [int]$MaxExplainSentences = 16,
        [int]$MaxExplainChars = 600, [int]$MinIdeaChars = 40, [int]$MaxIdeaChars = 300, [int]$MinReason = 8
    )
    if (-not $script:ZtCheck) { throw 'zt-check is not installed: run scripts\install-zt-check.ps1' }
    $a = if ($Json) { @('--json', $Json) } elseif ($Model) { @('--model', $Model) } elseif ($Addresses) { @('--addresses', $Addresses) } else { @('--cards', $Cards) }
    if ($Log) { $a += @('--log', $Log) }
    if ($Book) { $a += @('--book', $Book) }
    if ($Rules) { $a += @('--rules', $Rules) }
    if ($Setting) { $a += @('--setting', $Setting) }
    if ($Level) { $a += @('--level', $Level) }
    if ($RequireExplain) { $a += '--require-explain' }
    if ($SkipShared) { $a += '--skip-shared' }
    $a += @('--max-sentences', $MaxSentences, '--max-chars', $MaxChars, '--max-steps', $MaxSteps,
        '--max-explain-sentences', $MaxExplainSentences, '--max-explain-chars', $MaxExplainChars,
        '--min-idea-chars', $MinIdeaChars, '--max-idea-chars', $MaxIdeaChars, '--min-reason', $MinReason)
    $out = @(& $script:ZtCheck @a 2>&1 | ForEach-Object { [string]$_ })
    if (-not @($out | Where-Object { $_ -match '^faults: \d+$' }).Count) { throw "zt-check did not run: $($out -join ' | ')" }
    $out
}

$script:ZtHelper =Join-Path $PSScriptRoot '..\..\skills\zt\scripts\zt-mcp.ps1' | Resolve-Path | ForEach-Object Path

# Direct reads. Starting the helper costs a new pwsh and a new `zt mcp` per call (about 0.8 s). A
# batch of read-only calls goes instead to one long-lived `zt mcp` per PowerShell process (per
# runspace), with the helper's read output and its stop-at-first-error rule. Writes always go
# through the helper: it keeps the allow-list, `@@N`, the logs, and the Topic gate. The direct path
# is used only when the helper file carries the line `zt-calls: direct reads`, so a replacement
# helper (a test's fault or race seam) still gets every call. ZT_CALLS_DIRECT=0 turns it off.
$script:ZtReadTools = @('list_workspaces', 'list_trees', 'get_tree', 'get_card', 'get_model', 'get_hub', 'list_hubs', 'list_links', 'list_broken_links', 'search_cards')
$script:ZtDirectOk = [bool](Select-String -LiteralPath $script:ZtHelper -SimpleMatch 'zt-calls: direct reads' -Quiet)
$script:ZtServer = $null
function Close-ZtServer {
    $server = $script:ZtServer.process
    $script:ZtServer = $null
    if ($server) { try { $server.StandardInput.Close(); if (-not $server.WaitForExit(3000)) { $server.Kill() } } catch { } }
}
function Start-ZtServer {
    $zt = (Get-Command zt -ErrorAction Stop).Source
    $start = [Diagnostics.ProcessStartInfo]::new($zt, 'mcp')
    $start.RedirectStandardInput = $true; $start.RedirectStandardOutput = $true; $start.RedirectStandardError = $true
    $start.UseShellExecute = $false
    $start.StandardInputEncoding = [Text.UTF8Encoding]::new($false); $start.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $server = [Diagnostics.Process]::Start($start)
    # stderr is not read; a full stderr pipe must not block the server.
    $server.BeginErrorReadLine()
    $script:ZtServer = @{ process = $server; config = [string]$env:ZT_CONFIG_DIR; next = 1 }
    Send-ZtServer @{ jsonrpc = '2.0'; id = 0; method = 'initialize'; params = @{ protocolVersion = '2025-06-18' } }
    $null = Receive-ZtServer 0
    Send-ZtServer @{ jsonrpc = '2.0'; method = 'notifications/initialized' }
}
function Send-ZtServer($message) {
    $server = $script:ZtServer.process
    if ($server.HasExited) { throw [IO.IOException]::new('zt mcp exited') }
    $server.StandardInput.WriteLine(($message | ConvertTo-Json -Depth 50 -Compress))
    $server.StandardInput.Flush()
}
function Receive-ZtServer($id) {
    $server = $script:ZtServer.process
    while ($null -ne ($line = $server.StandardOutput.ReadLine())) {
        $reply = $line | ConvertFrom-Json -AsHashtable -DateKind String
        if ($reply.id -eq $id) { return $reply }
    }
    throw [IO.IOException]::new('zt mcp closed')
}
function Invoke-ZtDirect($calls) {
    if (-not $script:ZtServer -or $script:ZtServer.process.HasExited -or $script:ZtServer.config -cne [string]$env:ZT_CONFIG_DIR) { Close-ZtServer; Start-ZtServer }
    $values = [Collections.Generic.List[object]]::new()
    for ($i = 0; $i -lt $calls.Count; $i++) {
        $id = $script:ZtServer.next++
        Send-ZtServer @{ jsonrpc = '2.0'; id = $id; method = 'tools/call'; params = @{ name = $calls[$i].tool; arguments = ($calls[$i].arguments ?? @{}) } }
        $reply = Receive-ZtServer $id
        $text = ($reply.result.content | ForEach-Object { $_.text }) -join "`n"
        if ($reply.error -or $reply.result.isError) {
            $message = $reply.error.message ?? $text
            try { $message = ($message | ConvertFrom-Json -AsHashtable).error ?? $message } catch { }
            throw "zt-mcp failed: $i error $($message -replace '\s*\r?\n\s*', ' ')"
        }
        # As the helper: a read prints its result as compact JSON, and Invoke-Zt returns objects only.
        if ($text.TrimStart().StartsWith('{')) { $values.Add(($text | ConvertFrom-Json -DateKind String)) }
    }
    , $values.ToArray()
}

function Invoke-Zt($calls) {
    $calls = @($calls)
    if ($script:ZtDirectOk -and $env:ZT_CALLS_DIRECT -ne '0' -and $calls.Count -and
        -not @($calls | Where-Object { $_.tool -cnotin $script:ZtReadTools }).Count -and
        (ConvertTo-Json -InputObject $calls -Depth 10 -Compress) -notmatch '@@\d') {
        # A read changes nothing, so a batch that hit a dead server runs again on a new one.
        try { $values = Invoke-ZtDirect $calls } catch [IO.IOException] { Close-ZtServer; $values = Invoke-ZtDirect $calls }
        return $values
    }
    $lines = @(($calls | ConvertTo-Json -Depth 10 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "zt-mcp failed: $($lines -join ' | ')" }
    foreach ($line in $lines) {
        if ($line -match '^\d+ (\{.*)$') { $Matches[1] | ConvertFrom-Json -DateKind String }
        elseif ($line -match '^\d+ error (.*)$') { throw "zt: $($Matches[1])" }
    }
}
