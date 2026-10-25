#Requires -Version 7.5
<#
.SYNOPSIS
  Install the zt skills globally for Claude Code, Codex, and DeepSeek Harness, and register the
  `zt mcp` server for Claude Code and Codex.

.DESCRIPTION
  Safe to re-run; it replaces only what it owns.
  - Copies each skills/<name> (zt, zettelkasten, zt-digest-protocol) to ~/.claude/skills/<name>
    and ~/.codex/skills/<name>, and links ~/.dsh/skills/<name> to the Codex copy for DeepSeek
    Harness.
  - Registers the stdio server `zt` (`<zt.exe> mcp`) in Claude Code's user scope (~/.claude.json),
    approves this repository's .mcp.json server, and allows the server's tools in
    ~/.claude/settings.json so every conversation can call them without a prompt. The server
    itself limits writes to the agent Workspace.
  - Registers the same server in Codex with `codex mcp add`.
  - Checks that the server answers an MCP handshake.
  Config files are backed up next to themselves (*.zt-backup) before they are changed.
  Conversations that are already open keep the servers they started with; start a new one.
#>
[CmdletBinding()]
param(
    [string]$ZtPath = (Get-Command zt -ErrorAction SilentlyContinue).Source
)
$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent $PSScriptRoot
$skillsSource = Join-Path $repo 'skills'
if (-not $ZtPath -or -not (Test-Path $ZtPath)) {
    throw 'zt is not installed; run `cargo install --path . --locked` in the zt repository first.'
}
$ZtPath = (Resolve-Path $ZtPath).Path
Write-Host "zt binary: $ZtPath ($(& $ZtPath version))"

function Backup($path) {
    if (Test-Path $path) { Copy-Item -LiteralPath $path -Destination "$path.zt-backup" -Force }
}

function Write-JsonAtomically($path, $value) {
    $temp = "$path.zt-tmp"
    $value | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $temp -Encoding utf8NoBOM
    $null = Get-Content -LiteralPath $temp -Raw | ConvertFrom-Json -DateKind String
    Move-Item -LiteralPath $temp -Destination $path -Force
}

function Set-Member($object, $name, $value) {
    if ($object.PSObject.Properties[$name]) { $object.$name = $value }
    else { $object | Add-Member -NotePropertyName $name -NotePropertyValue $value }
}

# --- skills ------------------------------------------------------------------------------------
foreach ($skill in Get-ChildItem -LiteralPath $skillsSource -Directory) {
    foreach ($home_ in @("$HOME\.claude\skills", "$HOME\.codex\skills")) {
        New-Item -ItemType Directory -Force -Path $home_ | Out-Null
        $target = Join-Path $home_ $skill.Name
        if (Test-Path $target) { Remove-Item -LiteralPath $target -Recurse -Force }
        Copy-Item -LiteralPath $skill.FullName -Destination $target -Recurse
        Write-Host "skill installed: $target"
    }
    # DeepSeek Harness reads ~/.dsh/skills, whose entries link to the Codex copies.
    $dshSkills = "$HOME\.dsh\skills"
    if (Test-Path "$HOME\.dsh") {
        New-Item -ItemType Directory -Force -Path $dshSkills | Out-Null
        $link = Join-Path $dshSkills $skill.Name
        $codexCopy = Join-Path "$HOME\.codex\skills" $skill.Name
        $existing = Get-Item -LiteralPath $link -Force -ErrorAction SilentlyContinue
        if ($existing -and -not $existing.LinkType) {
            throw "$link is a real directory, not a link; move it aside and re-run."
        }
        if ($existing) { $existing.Delete() }
        New-Item -ItemType Junction -Path $link -Target $codexCopy | Out-Null
        Write-Host "skill linked: $link -> $codexCopy"
    }
}
if (-not (Test-Path "$HOME\.dsh")) { Write-Warning '~/.dsh does not exist; skipped the DeepSeek Harness skills.' }

# --- Claude Code -------------------------------------------------------------------------------
$claudeJson = "$HOME\.claude.json"
if (Test-Path $claudeJson) {
    Backup $claudeJson
    $config = Get-Content -LiteralPath $claudeJson -Raw | ConvertFrom-Json -DateKind String
} else {
    $config = [pscustomobject]@{}
}
if (-not $config.PSObject.Properties['mcpServers']) { Set-Member $config 'mcpServers' ([pscustomobject]@{}) }
Set-Member $config.mcpServers 'zt' ([pscustomobject][ordered]@{
    type    = 'stdio'
    command = $ZtPath
    args    = @('mcp')
    env     = [pscustomobject]@{}
})
# Approve the repository's own .mcp.json entry, which names the same server.
$projectKey = $repo -replace '\\', '/'
$project = $config.projects.PSObject.Properties | Where-Object { $_.Name -ieq $projectKey } | Select-Object -First 1
if ($project) {
    $enabled = @($project.Value.enabledMcpjsonServers | Where-Object { $_ })
    if ($enabled -notcontains 'zt') { $enabled += 'zt' }
    Set-Member $project.Value 'enabledMcpjsonServers' $enabled
    Set-Member $project.Value 'disabledMcpjsonServers' @($project.Value.disabledMcpjsonServers | Where-Object { $_ -and $_ -ne 'zt' })
}
Write-JsonAtomically $claudeJson $config
Write-Host "Claude Code: user-scope MCP server 'zt' -> $ZtPath mcp"

$settingsPath = "$HOME\.claude\settings.json"
if (Test-Path $settingsPath) {
    Backup $settingsPath
    $settings = Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json -DateKind String
} else {
    $settings = [pscustomobject]@{}
}
if (-not $settings.PSObject.Properties['permissions']) { Set-Member $settings 'permissions' ([pscustomobject]@{}) }
$allow = @($settings.permissions.allow | Where-Object { $_ })
if ($allow -notcontains 'mcp__zt') { $allow += 'mcp__zt' }
Set-Member $settings.permissions 'allow' $allow
Write-JsonAtomically $settingsPath $settings
Write-Host 'Claude Code: allowed tools mcp__zt'

# --- Codex -------------------------------------------------------------------------------------
$codex = (Get-Command codex -ErrorAction SilentlyContinue).Source
if ($codex) {
    Backup "$HOME\.codex\config.toml"
    & $codex mcp remove zt *> $null
    & $codex mcp add zt -- $ZtPath mcp
    if ($LASTEXITCODE -ne 0) { throw 'codex mcp add failed' }
    Write-Host "Codex: MCP server 'zt' -> $ZtPath mcp"
} else {
    Write-Warning 'codex is not on PATH; skipped the Codex registration.'
}

# --- check -------------------------------------------------------------------------------------
$requests = @(
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}'
    '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
) -join "`n"
$replies = $requests | & $ZtPath mcp | ForEach-Object { $_ | ConvertFrom-Json -DateKind String }
$tools = ($replies | Where-Object { $_.id -eq 2 }).result.tools.name
if (-not $tools) { throw 'zt mcp did not answer the handshake' }
Write-Host "zt mcp answers with $($tools.Count) tools: $($tools -join ', ')"
Write-Host 'Done. Start a new Claude Code or Codex conversation to use the server.'
