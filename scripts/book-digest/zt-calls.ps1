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
$script:ZtHelper = Join-Path $PSScriptRoot '..\..\skills\zt\scripts\zt-mcp.ps1' | Resolve-Path | ForEach-Object Path

function Invoke-Zt($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 10 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "zt-mcp failed: $($lines -join ' | ')" }
    foreach ($line in $lines) {
        if ($line -match '^\d+ (\{.*)$') { $Matches[1] | ConvertFrom-Json -DateKind String }
        elseif ($line -match '^\d+ error (.*)$') { throw "zt: $($Matches[1])" }
    }
}
