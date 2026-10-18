#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
# R273/R267/R268 (#82): the persisted run settings, the bounded readiness wait and the provider cap facts.
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-settings-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest; $env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) { throw "digest failed: $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Fail-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    return [pscustomobject]@{ code = $LASTEXITCODE; text = ($lines -join ' ') }
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source,"# 导论`n`n融资约束影响生产选择。`n`n# 第二章`n`n资本投入影响产量。`n`n# 参考`n未选择。",$utf8)
    $options = Join-Path $scratch 'run-options.json'
    [IO.File]::WriteAllText($options,'{"terms":false,"models":false,"events":false,"findings":false,"hubs":false,"figures":false,"bridge":false}',$utf8)
    function Common-Args([string]$Key) {
        return @('-Source',$source,'-CitationKey',$Key,'-BibTeX',"@book{$Key,title={Settings book}}",'-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-MaxUnits','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    }
    $common = Common-Args 'SettingsBook'

    # --- the two bounded settings, refused outside their finite bounds before anything is written ---
    foreach ($case in @(
        @{ args = @('-MaxAttempts','0'); expect = '-MaxAttempts'; name = 'attempts-too-small' },
        @{ args = @('-MaxAttempts','11'); expect = '-MaxAttempts'; name = 'attempts-too-large' },
        @{ args = @('-DshRunCap','59'); expect = '-DshRunCap'; name = 'cap-too-small' },
        @{ args = @('-DshRunCap','3601'); expect = '-DshRunCap'; name = 'cap-too-large' },
        @{ args = @('-DshRunCap','1.5'); expect = '-DshRunCap'; name = 'cap-not-whole' }
    )) {
        $bad = Join-Path $scratch $case.name
        $r = Fail-Digest (@('prepare') + $common + $case.args + @('-Out',$bad))
        Assert ($r.code -eq 2 -and $r.text.Contains($case.expect)) "an out-of-range setting refuses before preparing ($($case.name))"
        Assert (-not (Test-Path $bad)) "an out-of-range setting publishes no prepared files ($($case.name))"
    }
    $noVariant = Fail-Digest (@('prepare','-Source',$source,'-Out',(Join-Path $scratch 'legacy'),'-CitationKey','Legacy','-BibTeX','@book{Legacy}','-MaxAttempts','4'))
    Assert ($noVariant.code -eq 1 -and $noVariant.text.Contains('require Variant and Apply')) 'the bounded settings belong to a managed prepared run (Variant and Apply)'

    # --- the effective defaults and explicit values are persisted in the immutable contract ---
    $out = Join-Path $scratch 'prepared'
    $manifest = Run-Digest (@('prepare') + $common + @('-Out',$out,'-MaxAttempts','5','-DshRunCap','600'))
    Assert ($manifest.run_contract.settings.max_attempts -eq 5 -and $manifest.run_contract.settings.dsh_run_cap -eq 600) 'preparation persists the effective maxAttempts and dshRunCap in the run contract'
    $status = Run-Digest @('ledger','status','-Out',$out)
    Assert ($status.settings.max_attempts -eq 5 -and $status.settings.dsh_run_cap -eq 600) 'ledger status reports the frozen settings, so a resume reuses them'
    $defaultOut = Join-Path $scratch 'defaults'
    $defaults = Run-Digest (@('prepare') + (Common-Args 'SettingsDefaults') + @('-Out',$defaultOut))
    Assert ($defaults.run_contract.settings.max_attempts -eq 3 -and $defaults.run_contract.settings.dsh_run_cap -eq 1200) 'omitted settings persist the documented defaults (3 attempts, 1200 seconds)'
    $badResume = Fail-Digest (@('prepare') + $common + @('-Out',$out,'-MaxAttempts','2'))
    Assert ($badResume.code -eq 1 -and $badResume.text.Contains('already initialized')) 'a second preparation with different settings cannot replace the frozen run'

    # --- the bounded readiness wait keeps a claim call open only until its deadline ---
    $taken = Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','64','-Owner','settings-manager')
    Assert ($taken.units.Count -ge 1) 'the readiness fixture claims every ready group before the wait'
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $wait = Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','settings-waiter','-Wait','2')
    $watch.Stop()

    Assert ($wait.units.Count -eq 0 -and $watch.Elapsed.TotalSeconds -ge 1 -and $watch.Elapsed.TotalSeconds -lt 30) "ledger next -Wait waits for readiness inside the tool, not for ever ($([int]$watch.Elapsed.TotalSeconds) s)"
    $badWait = Fail-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','settings-waiter','-Wait','601')
    Assert ($badWait.code -eq 2 -and $badWait.text.Contains('-Wait')) 'the readiness wait has a finite declared bound (0..600 seconds)'

    # --- the provider cap is measured from the registered start and drives cancel then retry ---
    $token = $taken.units[0].token
    $started = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds() - 700
    $registration = Join-Path $scratch 'provider-run.json'
    [IO.File]::WriteAllText($registration,(@{ id = 'dsh-run-1'; kind = 'deepseek'; started_at = $started } | ConvertTo-Json -Compress),$utf8)
    $registered = Run-Digest @('ledger','job','-Out',$out,'-Unit','digest/g001@1','-Token',"$token",'-Register',$registration)
    Assert ($registered.jobs[0].cap_seconds -eq 600 -and $registered.jobs[0].started_at -eq $started) 'a provider run registers the frozen cap and the actual provider start'
    Assert ($registered.jobs[0].cap_exceeded -eq $true -and $registered.jobs[0].action -eq 'cancel') 'a run past its cap is reported with the action cancel'
    $cancelled = Run-Digest @('ledger','job','-Out',$out,'-Unit','digest/g001@1','-Cancel','dsh-run-1')
    Assert ($cancelled.jobs[0].action -eq 'await_terminal' -and $cancelled.jobs[0].state -eq 'running') 'a requested cancellation waits for its terminal observation'
    $blocked = Fail-Digest @('ledger','reclaim','-Out',$out,'-Unit','digest/g001@1')
    Assert ($blocked.code -eq 1 -and ($blocked.text.Contains('claim_active') -or $blocked.text.Contains('jobs_running'))) 'a cancellation still awaiting its terminal state never grants a replacement claim'
    $observation = Join-Path $scratch 'provider-terminal.json'
    [IO.File]::WriteAllText($observation,(@{ id = 'dsh-run-1'; state = 'terminal'; outcome = 'cancelled' } | ConvertTo-Json -Compress),$utf8)
    $observed = Run-Digest @('ledger','job','-Out',$out,'-Unit','digest/g001@1','-Observe',$observation)
    Assert ($observed.jobs[0].state -eq 'terminal' -and $observed.jobs[0].outcome -eq 'cancelled') 'the terminal observation settles the run'
    $faultFile = Join-Path $scratch 'provider-cap-fault.json'
    [IO.File]::WriteAllText($faultFile,(@{ schema_version = 1; unit = 'digest/g001@1'; input_hash = $taken.units[0].input_hash; code = 'provider_cap'; cause = 'the external run passed the frozen cap and was cancelled' } | ConvertTo-Json -Compress),$utf8)
    $failed = Run-Digest @('ledger','fail','-Out',$out,'-Unit','digest/g001@1','-Token',"$token",'-Class','retryable','-Fault',$faultFile)
    Assert ($failed.state -eq 'retry') 'a cancelled provider run records the transient provider cap fault and becomes retryable'
    # --- the per-unit attempt budget is the prepared setting (R258/R273) ---
    $tightOut = Join-Path $scratch 'tight'
    $null = Run-Digest (@('prepare') + (Common-Args 'SettingsTight') + @('-Out',$tightOut,'-MaxAttempts','1'))
    $tightClaim = Run-Digest @('ledger','next','-Out',$tightOut,'-Layer','digest','-Take','1','-Owner','settings-tight')
    $tightToken = $tightClaim.units[0].token
    $tightFault = Join-Path $scratch 'tight-fault.json'
    [IO.File]::WriteAllText($tightFault,(@{ schema_version = 1; unit = 'digest/g001@1'; input_hash = $tightClaim.units[0].input_hash; code = 'provider_cap'; cause = 'the prepared budget allows one attempt' } | ConvertTo-Json -Compress),$utf8)
    $spent = Run-Digest @('ledger','fail','-Out',$tightOut,'-Unit','digest/g001@1','-Token',"$tightToken",'-Class','retryable','-Fault',$tightFault)
    Assert ($spent.state -eq 'failed') 'with maxAttempts 1 a retryable failure is terminal: the ledger spends the prepared attempt budget'
    Write-Output 'ok: ledger settings, readiness wait and provider cap verified'
} finally {
    if (Test-Path $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue }
}