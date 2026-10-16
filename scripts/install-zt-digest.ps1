#Requires -Version 7
<#
.SYNOPSIS
  Build tools\zt-digest (release) and install zt-digest.exe next to zt.exe, so every agent finds it
  on PATH: Claude sessions, workflow agents, and DeepSeek Harness runs.

.DESCRIPTION
  zt-digest is a crate of its own (it takes tools\zt-check as a library): building it never builds
  zt, and installing it never stops the zt service. Its target folder is D:\zt-digest-target (E: has little free space). A zt-digest.exe
  that a running digest step holds open is renamed aside first (zt-digest.exe.old-<time>).

  Run: pwsh -NoProfile -File scripts\install-zt-digest.ps1
#>
[CmdletBinding()]
param(
    [string]$TargetDir = 'D:\zt-digest-target',
    # Default: the folder of the zt on PATH (~\.cargo\bin).
    [string]$BinDir = ''
)
$ErrorActionPreference = 'Stop'
$crate = Join-Path $PSScriptRoot '..\tools\zt-digest' | Resolve-Path | ForEach-Object Path
if (-not $BinDir) { $BinDir = Split-Path -Parent (Get-Command zt -ErrorAction Stop).Source }

$env:CARGO_TARGET_DIR = $TargetDir
Push-Location $crate
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
} finally { Pop-Location }

$built = Join-Path $TargetDir 'release\zt-digest.exe'
$target = Join-Path $BinDir 'zt-digest.exe'
if (Test-Path -LiteralPath $target) {
    try { Remove-Item -LiteralPath $target -Force }
    catch { Rename-Item -LiteralPath $target -NewName "zt-digest.exe.old-$(Get-Date -Format yyyyMMdd-HHmmss)" }
}
Copy-Item -LiteralPath $built -Destination $target
$found = (Get-Command zt-digest -ErrorAction Stop).Source
Write-Output "installed $target (on PATH: $found)"
