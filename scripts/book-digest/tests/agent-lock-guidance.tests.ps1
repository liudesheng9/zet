#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\agent-lock-guidance.tests.ps1 [-Zt <zt.exe>]
# Tree Lock S17 (#140): the agent-facing material states one Lock policy and nothing more. The MCP
# server's own initialize answer and the two skills say that a locked Card is a readable, linkable
# source, that new thought belongs in another unlocked tree, and that only the owner changes Locks.
# None of them names a Lock route, a token or a way to acquire another Access (R325/R326).
# The live archive is never touched and no Card is written.
param([string]$Zt = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "agent-lock-guidance-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
if (-not $Zt) { $Zt = (Get-Command zt -CommandType Application | Select-Object -First 1).Source }
if (-not $Zt) { throw 'no zt binary: pass -Zt <path> or put zt on PATH' }
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
New-Item -ItemType Directory -Force $env:ZT_CONFIG_DIR | Out-Null
# The server answers initialize before any Card work, but it asks for an archive root first.
$archive = Join-Path $scratch 'archive'
New-Item -ItemType Directory -Force $archive | Out-Null
$null = & $Zt config set archive_root $archive 2>&1
if ($LASTEXITCODE -ne 0) { throw 'could not configure the scratch archive root' }
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }

# The server's own words: one initialize request over stdio, then the answer.
function Initialize-Instructions([string]$exe) {
    $info = [Diagnostics.ProcessStartInfo]::new($exe)
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.ArgumentList.Add('mcp')
    $proc = [Diagnostics.Process]::new()
    $proc.StartInfo = $info
    $null = $proc.Start()
    $out = $proc.StandardOutput.ReadToEndAsync()
    $err = $proc.StandardError.ReadToEndAsync()
    $proc.StandardInput.WriteLine('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}')
    $proc.StandardInput.Close()
    if (-not $proc.WaitForExit(30000)) { $proc.Kill($true); throw 'zt mcp did not answer initialize within 30 s' }
    $text = $out.GetAwaiter().GetResult()
    $stderr = $err.GetAwaiter().GetResult()
    $proc.Dispose()
    $answer = @($text -split "`n" | Where-Object { $_.Trim() } | Select-Object -First 1)
    if (-not $answer) { throw "zt mcp printed no answer: $stderr" }
    ($answer | ConvertFrom-Json)
}

$reply = Initialize-Instructions $Zt
$instructions = [string]$reply.result.instructions
Assert ([bool]$instructions) "the MCP server answers initialize with its instructions ($($instructions.Length) characters)"
$lower = $instructions.ToLowerInvariant()
foreach ($phrase in @('`lock`', 'locked tree', 'read and link', 'another, unlocked tree', 'only the owner changes locks', '`card_locked`', 'planning error', 'do not retry')) {
    Assert ($lower.Contains($phrase)) "the MCP instructions say `"$phrase`""
}
# R325/R326: agent-facing material names no Lock route, token or second write path.
foreach ($refused in @('/api/lock', '/api/unlock', 'X-ZT-Token', 'zt-token', 'token=', 'unlock_card', 'lock_card')) {
    Assert (-not $instructions.Contains($refused)) "the MCP instructions never name $refused"
}

# The two skills: the same rule in each, and no route or token either.
$ztSkill = [IO.File]::ReadAllText((Join-Path $repo 'skills\zt\SKILL.md'))
$zkSkill = [IO.File]::ReadAllText((Join-Path $repo 'skills\zettelkasten\SKILL.md'))
foreach ($case in @(@{ name = 'zt'; text = $ztSkill }, @{ name = 'zettelkasten'; text = $zkSkill })) {
    $name = $case.name
    $text = $case.text
    # The Lock paragraph itself: the lines that state the Lock policy, not every line of the skill.
    $lockLines = @(($text -split "`r?`n") | Where-Object { $_ -match 'locked tree|card_locked|Card lock|changes Locks' })
    $lockText = $lockLines -join "`n"
    $lockLower = $lockText.ToLowerInvariant()
    Assert ([bool]$lockText) "$name skill has a Lock paragraph"
    Assert ($lockLower.Contains('locked tree')) "$name skill names a locked tree"
    Assert ($lockLower -match 'read' -and $lockLower -match 'link') "$name skill says a locked Card stays readable and linkable"
    Assert ($lockLower.Contains('another') -and $lockLower.Contains('unlocked tree')) "$name skill sends new thought to another unlocked tree"
    Assert ($lockLower.Contains('only the owner changes locks')) "$name skill says only the owner changes Locks"
    Assert ($lockLower.Contains('`card_locked`')) "$name skill names the typed card_locked error"
    Assert ($lockLower.Contains('do not retry') -or $lockLower.Contains('never retry')) "$name skill forbids retrying it as a fault"
    foreach ($refused in @('/api/lock', '/api/unlock', 'X-ZT-Token', 'zt-token', 'token=')) {
        Assert (-not $lockText.Contains($refused)) "$name skill never names $refused"
    }
    # A Lock never widens Access: the Lock paragraph offers no shell, GUI route or SQL way around it.
    Assert (-not ($lockLower -match 'zt\.sqlite3|sqlite3 |shell command|with the `zt` shell')) "$name skill offers no direct database or shell write around a Lock"
}

Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
if ($failed) { Write-Output "$failed check(s) failed"; exit 1 }
Write-Output 'agent Lock guidance: ok'
