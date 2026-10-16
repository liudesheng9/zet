#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\requires-version.tests.ps1
# A script that uses ConvertFrom-Json -DateKind (PowerShell 7.5), or dot-sources zt-calls.ps1 (which does),
# must declare #Requires -Version 7.5, so an older pwsh refuses it at the start instead of failing part way.
$ErrorActionPreference = 'Stop'
$repo = Join-Path $PSScriptRoot '..\..\..' | Resolve-Path | ForEach-Object Path
$failed = 0
$scripts = @(Get-ChildItem -Path (Join-Path $repo 'scripts'), (Join-Path $repo 'skills') -Recurse -Filter '*.ps1' -File |
        Where-Object { $_.FullName -notmatch '[\\/]tests[\\/]' })
foreach ($file in $scripts) {
    $text = Get-Content -LiteralPath $file.FullName -Raw -Encoding utf8
    $needs = $text -match '-DateKind' -or $text -match "zt-calls\.ps1'\)"
    if (-not $needs) { continue }
    $declared = [regex]::Match($text, '(?m)^#Requires -Version (\d+(?:\.\d+)?)')
    $relative = $file.FullName.Substring($repo.Length + 1)
    if (-not $declared.Success -or [version]($declared.Groups[1].Value + $(if ($declared.Groups[1].Value -notmatch '\.') { '.0' })) -lt [version]'7.5') {
        Write-Output "FAIL: $relative needs PowerShell 7.5 but declares $(if ($declared.Success) { $declared.Value } else { 'nothing' })"
        $failed++
    } else {
        Write-Output "ok: $relative declares 7.5"
    }
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
