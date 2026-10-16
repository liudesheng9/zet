#Requires -Version 7.5
<#
.SYNOPSIS
  Run every book-digest test in parallel: each *.tests.ps1 file and each workflow harness.

.DESCRIPTION
  The test files are independent. Each makes its own scratch archive under the temp folder, with its
  own ZT_CONFIG_DIR and ZT_GUI_PORT=0, so they can run at the same time. The runner starts up to
  -Throttle of them at once, writes each one's output to <Logs>\<name>.log, prints one line per
  test (exit code, seconds, ok count), and exits 1 when any test fails.

    pwsh -NoProfile -File scripts\book-digest\tests\run-all.ps1
    pwsh -NoProfile -File scripts\book-digest\tests\run-all.ps1 -Only bridge-compiler,digest-audit
    pwsh -NoProfile -File scripts\book-digest\tests\run-all.ps1 -Zt D:\zt-target\debug\zt.exe

  -Only takes test names (with or without .tests.ps1 / .mjs) as one comma-separated string or a list.
  -Zt is passed to each test file that declares a -Zt parameter (a zt build to test against).
  -Sequential runs one test at a time, as before.
#>
[CmdletBinding()]
param(
    [string[]]$Only = @(),
    [int]$Throttle = [Math]::Max(2, [Math]::Min(8, [Environment]::ProcessorCount)),
    [string]$Zt = '',
    [string]$Logs = '',
    [switch]$Sequential
)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$repo = (Resolve-Path (Join-Path $here '..\..\..')).Path
if (-not $Logs) { $Logs = Join-Path ([IO.Path]::GetTempPath()) "zt-digest-tests-$PID" }
New-Item -ItemType Directory -Force -Path $Logs | Out-Null
$names = @($Only | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ } | ForEach-Object { $_ -replace '\.tests\.ps1$', '' -replace '\.mjs$', '' })

$items = @()
foreach ($f in Get-ChildItem -LiteralPath $here -Filter '*.tests.ps1' | Sort-Object Name) {
    $declaresZt = [bool](Select-String -LiteralPath $f.FullName -Pattern '^\s*param\(.*\$Zt\b' -Quiet)
    $items += [pscustomobject]@{ name = $f.Name; key = ($f.Name -replace '\.tests\.ps1$', ''); kind = 'ps1'; path = $f.FullName; zt = $declaresZt }
}
foreach ($f in Get-ChildItem -LiteralPath $here -Filter '*harness.mjs' | Sort-Object Name) {
    $items += [pscustomobject]@{ name = $f.Name; key = ($f.Name -replace '\.mjs$', ''); kind = 'node'; path = $f.FullName; zt = $false }
}
if ($names.Count) {
    $unknown = @($names | Where-Object { $_ -notin $items.key })
    if ($unknown.Count) { throw "unknown tests: $($unknown -join ', ')" }
    $items = @($items | Where-Object { $_.key -in $names })
}

$run = {
    param($item, $repo, $logs, $zt)
    $log = Join-Path $logs "$($item.name).log"
    $sw = [Diagnostics.Stopwatch]::StartNew()
    Push-Location $repo
    try {
        if ($item.kind -eq 'ps1') {
            $arguments = @('-NoProfile', '-File', $item.path)
            if ($zt -and $item.zt) { $arguments += @('-Zt', $zt) }
            & pwsh @arguments *> $log
        } else {
            & node $item.path *> $log
        }
        $code = $LASTEXITCODE
    } finally { Pop-Location }
    $text = @(Get-Content -LiteralPath $log -ErrorAction SilentlyContinue)
    [pscustomobject]@{
        test = $item.name; exit = $code; seconds = [int]$sw.Elapsed.TotalSeconds
        ok = @($text | Where-Object { $_ -match '^ok\b' }).Count
        tail = $(if ($code) { (@($text | Select-Object -Last 3) -join ' | ') } else { '' })
    }
}

$wall = [Diagnostics.Stopwatch]::StartNew()
$results = if ($Sequential) {
    foreach ($item in $items) { & $run $item $repo $Logs $Zt }
} else {
    $source = $run.ToString()
    $items | ForEach-Object -ThrottleLimit $Throttle -Parallel {
        $block = [scriptblock]::Create($using:source)
        & $block $_ $using:repo $using:Logs $using:Zt
    }
}
$results = @($results | Sort-Object test)
foreach ($r in $results) { '{0,-42} exit={1} {2,4}s ok={3}{4}' -f $r.test, $r.exit, $r.seconds, $r.ok, $(if ($r.tail) { "  $($r.tail)" } else { '' }) }
$failed = @($results | Where-Object { $_.exit -ne 0 })
"{0} tests, {1} failed, {2} s wall clock. Logs: {3}" -f $results.Count, $failed.Count, [int]$wall.Elapsed.TotalSeconds, $Logs
exit $(if ($failed.Count) { 1 } else { 0 })
