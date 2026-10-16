#Requires -Version 7
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
# check: the Card language rules R2 to R17 and the Card layer rules I, E, L. Invoke-ZtCheck takes
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

function Invoke-Zt($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 10 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "zt-mcp failed: $($lines -join ' | ')" }
    foreach ($line in $lines) {
        if ($line -match '^\d+ (\{.*)$') { $Matches[1] | ConvertFrom-Json -DateKind String }
        elseif ($line -match '^\d+ error (.*)$') { throw "zt: $($Matches[1])" }
    }
}
