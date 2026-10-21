#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-claims-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments,[int]$ExpectedExit = 0) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE -ne $ExpectedExit) { throw "digest failed ($LASTEXITCODE expected $ExpectedExit): $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Refuses([string[]]$Arguments,[string]$Expected,[int]$ExitCode=1) {
    $lines = @(& $Digest @Arguments 2>&1)
    Assert ($LASTEXITCODE -eq $ExitCode -and ($lines -join ' ').Contains($Expected)) "refuses $Expected"
}
function Start-Claim([string]$Owner) {
    $psi = [Diagnostics.ProcessStartInfo]::new($Digest)
    $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true
    $psi.RedirectStandardOutput = $true; $psi.RedirectStandardError = $true
    foreach ($arg in @('ledger','next','-Out',$out,'-Layer','terms','-Take','1','-Owner',$Owner)) { $psi.ArgumentList.Add($arg) }
    return [Diagnostics.Process]::Start($psi)
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source,"# 导论`n`n资本约束改变生产选择。`n`n# 第二章`n`n市场价格影响资本投入。`n`n# 参考`n未选择。",$utf8)
    $out = Join-Path $scratch 'prepared'
    $manifest = Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','ClaimBook','-BibTeX','@book{ClaimBook,title={Claim book}}','-StartHeading','导论','-EndHeading','参考','-MaxUnits','1','-MinUnitChars','1','-NoFigures','-Variant','econ','-Apply')
    $prepared = Run-Digest @('ledger','gate','-Out',$out,'-Layer','prepare')
    Assert ($prepared.sealed -and $prepared.settled -and $prepared.complete) 'the checked preparation gate is settled and complete'
    $waiting = Run-Digest @('ledger','gate','-Out',$out,'-Layer','digest') 3
    Assert (-not $waiting.settled -and -not $waiting.complete -and $waiting.unresolved.Count -eq $manifest.groups.Count) 'unwritten groups keep their sealed target open with exact unresolved ids'
    $empty = Run-Digest @('ledger','gate','-Out',$out,'-Layer','events') 3
    Assert (-not $empty.sealed -and -not $empty.complete -and $empty.counts.total -eq 0) 'an empty dynamic queue is not a sealed layer'
    $excluded = Run-Digest @('ledger','gate','-Out',$out,'-Layer','figures')
    Assert ($excluded.excluded -and $excluded.settled -and -not $excluded.complete) 'an explicit exclusion is distinct from successful work'
    $notThisVariant = Run-Digest @('ledger','gate','-Out',$out,'-Layer','proof')
    Assert ($notThisVariant.excluded -and -not $notThisVariant.complete) 'mathematical obligations are explicit exclusions in an economics target'
    $workers = @((Start-Claim 'manager-a'),(Start-Claim 'manager-b'))
    $claims = @()
    foreach ($worker in $workers) {
        # Read both pipes while the claim runs: a reply larger than the pipe buffer must never
        # deadlock the child behind the parent's WaitForExit.
        $stdoutTask = $worker.StandardOutput.ReadToEndAsync()
        $stderrTask = $worker.StandardError.ReadToEndAsync()
        if (-not $worker.WaitForExit(30000)) { $worker.Kill($true); throw 'claim process timed out' }
        $stdout = $stdoutTask.Result; $stderr = $stderrTask.Result
        if ($worker.ExitCode) { throw "claim failed: $stderr" }
        $claims += @((ConvertFrom-Json -InputObject $stdout -AsHashtable).units)
        $worker.Dispose()
    }
    Assert ($claims.Count -eq 1 -and $claims[0].id -eq 'terms/book@1' -and $claims[0].token -gt 0) 'two actual processes receive exactly one current claim for the same queue'
    $claim = $claims[0]
    # R318: the claiming command refreshed the archive's active-run marker for this prepared run.
    $marker = Join-Path (Join-Path $scratch 'archive') 'zt.digest.run'
    Assert (Test-Path -LiteralPath $marker) 'a claimed unit refreshes the archive active-run marker'
    $markerFacts = Get-Content -LiteralPath $marker -Raw | ConvertFrom-Json -AsHashtable
    $runId = (Run-Digest @('ledger','status','-Out',$out)).run_id
    Assert ($markerFacts.run_id -eq $runId -and $markerFacts.out -eq $out) 'the marker names the run and staging folder whose command renewed it'
    Assert ($markerFacts.ttl_s -eq 300 -and $markerFacts.pid -gt 0) 'the marker carries a finite lifetime and the renewing process'
    $status = Run-Digest @('ledger','status','-Out',$out,'-Layer','terms')
    Assert ($status.units[0].state -eq 'claimed' -and $status.units[0].owner -eq $claim.owner -and $status.units[0].attempts -eq 1 -and $claim.input_hash -eq $status.units[0].input_hash -and $status.units[0].lease_until -eq $claim.lease_until) 'claim ownership, input hash, lease and attempt reservation are durable'
    $renew = Run-Digest @('ledger','renew','-Out',$out,'-Unit',$claim.id,'-Token',"$($claim.token)")
    Assert ($renew.token -eq $claim.token -and $renew.lease_until -ge $claim.lease_until) 'renew extends only the current claimed token'
    Refuses @('ledger','renew','-Out',$out,'-Unit',$claim.id,'-Token',"$($claim.token+1)") 'stale_claim'
    $fault = Join-Path $scratch 'fault.json'
    [IO.File]::WriteAllText($fault,(@{code='access_refused';unit=$claim.id;input_hash=$claim.input_hash;cause='the fixture refuses this access'} | ConvertTo-Json),$utf8)
    Refuses @('ledger','fail','-Out',$out,'-Unit',$claim.id,'-Token',"$($claim.token)",'-Class','retryable','-Fault',$fault) 'fault classification mismatch'
    Refuses @('ledger','fail','-Out',$out,'-Unit',$claim.id,'-Token',"$($claim.token+1)",'-Class','operator','-Fault',$fault) 'stale_claim'
    Refuses @('ledger','fail','-Out',$out,'-Unit',$claim.id,'-Token',"$($claim.token+1)",'-Class','operator','-Fault',(@{code='access_refused';unit=$claim.id;input_hash=$claim.input_hash;cause='a stale token with an inline fault document'} | ConvertTo-Json -Compress)) 'stale_claim'
    $failed = Run-Digest @('ledger','fail','-Out',$out,'-Unit',$claim.id,'-Token',"$($claim.token)",'-Class','operator','-Fault',$fault)
    Assert ($failed.state -eq 'failed' -and $failed.fault.code -eq 'access_refused') 'permission faults are final even when presented through a transport caller'
    $partial = Run-Digest @('ledger','gate','-Out',$out,'-Layer','digest')
    Assert ($partial.sealed -and $partial.settled -and -not $partial.complete -and $partial.counts.blocked -eq $manifest.groups.Count -and $partial.counts.open -eq 0) 'a final prerequisite failure settles the digest target as partial with blocked groups'
    $blocked = Run-Digest @('ledger','status','-Out',$out,'-Layer','digest')
    Assert (@($blocked.units | Where-Object { $_.fault.code -eq 'dependency_failed' -and 'terms/book@1' -in $_.fault.prerequisites }).Count -eq $manifest.groups.Count -and @($blocked.units | Where-Object attempts -NE 0).Count -eq 0) 'blocked groups name their failed prerequisite without consuming attempts'
    $rerun = Run-Digest @('ledger','rerun-list','-Out',$out)
    Assert ('terms/book@1' -in $rerun.unresolved -and 'prepare/book@1' -notin $rerun.unresolved -and 'digest/g001@1' -in $rerun.downstream) 'rerun coverage includes unresolved dependencies and downstream work while preserving done builds'
    $none = Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','2','-Owner','manager-c')
    Assert ($none.units.Count -eq 0 -and -not $none.complete) 'zero claims cannot be interpreted as complete digest coverage'
    Refuses @('ledger','renew','-Out',$out,'-Unit',$claim.id,'-Token',"$($claim.token)") 'stale_claim'
    Refuses @('ledger','next','-Out',$out,'-Layer','terms','-Take','1.5','-Owner','manager-d') 'positive integer' 2
    $secondPrerequisite = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-claims-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.exec('pragma foreign_keys=on');
db.exec("begin;insert into units(id,layer,logical_key,revision,state,inputs_json,input_hash,created_by,created_at) select 'models/other@1','models','other',1,'registered',inputs_json,input_hash,'prepare/book@1',created_at from units where id='terms/book@1';insert into deps values('models/other@1','prepare/book@1','success');insert into deps values('digest/g001@1','models/other@1','success');commit");db.close();
'@
    & node --no-warnings -e $secondPrerequisite (Join-Path $out 'ledger.sqlite3')
    if ($LASTEXITCODE) { throw 'isolated prerequisite fixture failed' }
    $other = Run-Digest @('ledger','next','-Out',$out,'-Layer','models','-Take','1','-Owner','other-manager')
    Assert ($other.units.Count -eq 1) 'independent ready work remains claimable after another prerequisite fails'
    $other = $other.units[0]
    # The frozen workflow modules pass the fault document as JSON text (zt-digest-lib.js ailUnit),
    # while the tests and the runbook may name a file: both forms must record the same failure.
    $otherFault = (@{code='invalid_plan';unit=$other.id;input_hash=$other.input_hash;cause='the independent fixture plan is invalid'} | ConvertTo-Json -Compress)
    $otherFailed = Run-Digest @('ledger','fail','-Out',$out,'-Unit',$other.id,'-Token',"$($other.token)",'-Class','content','-Fault',$otherFault)
    Assert ($otherFailed.state -eq 'failed' -and $otherFailed.fault.code -eq 'invalid_plan') 'a fault document given as JSON text records the same failure as a file'
    $blockedAgain = Run-Digest @('ledger','status','-Out',$out,'-Layer','digest')
    $firstGroup = @($blockedAgain.units | Where-Object id -EQ 'digest/g001@1')[0]
    Assert ($firstGroup.fault.prerequisites.Count -eq 2 -and 'terms/book@1' -in $firstGroup.fault.prerequisites -and 'models/other@1' -in $firstGroup.fault.prerequisites) 'an already blocked unit updates its exact prerequisite list when another producer fails'
    $damage = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-claims-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.exec('pragma foreign_keys=on');let appendOnly=false,foreignKeys=false;
try{db.exec('delete from events')}catch(e){appendOnly=String(e).includes('append-only')}
try{db.exec("insert into deps values('not-a-unit','prepare/book@1','success')")}catch(e){foreignKeys=String(e).includes('FOREIGN KEY')}
// A publication unit now carries real readiness history, so removing it removes its
// own events too, and every dependency that named it as its prerequisite (the run-level
// cleanup waits on the audit revision, R323). The append-only guard is restored before the
// ledger is read back.
db.exec("DROP TRIGGER events_no_delete");
db.exec("begin;delete from deps where unit_id='audit/book@1' or needs_id='audit/book@1';delete from events where unit_id='audit/book@1';delete from units where id='audit/book@1';commit");
db.exec("CREATE TRIGGER events_no_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT,'events are append-only'); END");
db.close();console.log(JSON.stringify({appendOnly,foreignKeys}));
'@
    $constraints = & node --no-warnings -e $damage (Join-Path $out 'ledger.sqlite3') | ConvertFrom-Json -AsHashtable
    if ($LASTEXITCODE) { throw 'isolated ledger fault injection failed' }
    Assert ($constraints.appendOnly -and $constraints.foreignKeys) 'SQLite refuses event deletion and dependencies without registered endpoints'
    $damaged = Run-Digest @('ledger','gate','-Out',$out,'-Layer','audit') 3
    Assert (@($damaged.coverage_faults | Where-Object unit -EQ 'audit/book@1').Count -eq 1 -and -not $damaged.complete) 'a missing required book publication is an exact coverage fault even with no queued rows'
    $out = Join-Path $scratch 'retry-prepared'
    $math = Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','RetryBook','-BibTeX','@book{RetryBook,title={Retry book}}','-StartHeading','导论','-EndHeading','参考','-MaxUnits','1','-MinUnitChars','1','-NoFigures','-Variant','math','-Apply')
    $mathStatus = Run-Digest @('ledger','status','-Out',$out)
    Assert ('events' -in $mathStatus.excluded_layers -and 'proof' -in $mathStatus.enabled_layers -and @($mathStatus.units | Where-Object layer -EQ 'notation').Count -eq 1) 'a mathematics preparation declares its required notation work and excludes economics-only layers'
    $tokens = @()
    for ($attempt=1; $attempt -le 3; $attempt++) {
        $ready = Run-Digest @('ledger','next','-Out',$out,'-Layer','terms','-Take','1','-Owner',"retry-manager-$attempt")
        Assert ($ready.units.Count -eq 1) "actual retry attempt $attempt receives one claim"
        $claim = $ready.units[0]; $tokens += $claim.token
        if ($attempt -gt 1) { Refuses @('ledger','renew','-Out',$out,'-Unit',$claim.id,'-Token',"$($tokens[0])") 'stale_claim' }
        $retryCode = if ($attempt -eq 2) { 'read_view_changed' } else { 'pending_jobs' }
        [IO.File]::WriteAllText($fault,(@{code=$retryCode;unit=$claim.id;input_hash=$claim.input_hash;cause='registered fixture work is still pending'} | ConvertTo-Json),$utf8)
        $retry = Run-Digest @('ledger','fail','-Out',$out,'-Unit',$claim.id,'-Token',"$($claim.token)",'-Class','retryable','-Fault',$fault)
        if ($attempt -lt 3) {
            $remaining = $retry.retry_after-[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
            $baseDelay = if ($attempt -eq 1) { 5 } else { 20 }
            Assert ($retry.state -eq 'retry' -and $remaining -ge $baseDelay-1 -and $remaining -le $baseDelay+1) "attempt $attempt uses the fixed $baseDelay-second backoff with bounded jitter"
            $tooSoon = Run-Digest @('ledger','next','-Out',$out,'-Layer','terms','-Take','1','-Owner','premature-manager')
            Assert ($tooSoon.units.Count -eq 0) 'backoff waiting cannot consume another attempt'
            $remaining = [Math]::Max(0,$retry.retry_after-[DateTimeOffset]::UtcNow.ToUnixTimeSeconds())
            Start-Sleep -Seconds ($remaining+0.15)
        } else {
            Assert ($retry.state -eq 'failed' -and $null -eq $retry.retry_after) 'the third actual attempt exhausts the retry budget for this invocation'
        }
    }
    $exhausted = Run-Digest @('ledger','status','-Out',$out,'-Layer','terms')
    Assert ($exhausted.units[0].attempts -eq 3 -and ($tokens | Select-Object -Unique).Count -eq 3 -and $tokens[2] -gt $tokens[0]) 'retries retain exact historical attempt count and monotonically increasing tokens'
    $noFourth = Run-Digest @('ledger','next','-Out',$out,'-Layer','terms','-Take','1','-Owner','fourth-manager')
    Assert ($noFourth.units.Count -eq 0 -and -not $noFourth.complete) 'an exhausted failed unit cannot silently receive a fourth attempt'
    $out = Join-Path $scratch 'expired-prepared'
    $expiredManifest = Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','ExpiredBook','-BibTeX','@book{ExpiredBook,title={Expired book}}','-StartHeading','导论','-EndHeading','参考','-MaxUnits','1','-MinUnitChars','1','-NoFigures','-Variant','econ','-Apply')
    $claimed = Run-Digest @('ledger','next','-Out',$out,'-Layer','terms','-Take','1','-Owner','expired-manager')
    $claim = $claimed.units[0]
    $expire = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-claims-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);const changed=db.prepare("update units set lease_until=? where id=? and claim_token=? and state='claimed'").run(Math.floor(Date.now()/1000)-1,process.argv[2],Number(process.argv[3]));
db.close();if(changed.changes!==1)throw new Error('the current claim was not expired');
'@
    & node --no-warnings -e $expire (Join-Path $out 'ledger.sqlite3') $claim.id "$($claim.token)"
    if ($LASTEXITCODE) { throw 'isolated lease expiry injection failed' }
    Refuses @('ledger','renew','-Out',$out,'-Unit',$claim.id,'-Token',"$($claim.token)") 'stale_claim'
    [IO.File]::WriteAllText($fault,(@{code='pending_jobs';unit=$claim.id;input_hash=$claim.input_hash;cause='expired attempt cannot report'} | ConvertTo-Json),$utf8)
    Refuses @('ledger','fail','-Out',$out,'-Unit',$claim.id,'-Token',"$($claim.token)",'-Class','retryable','-Fault',$fault) 'stale_claim'
    $notReclaimed = Run-Digest @('ledger','next','-Out',$out,'-Layer','terms','-Take','1','-Owner','replacement-manager')
    $expiredGate = Run-Digest @('ledger','gate','-Out',$out,'-Layer','terms') 3
    Assert ($notReclaimed.units.Count -eq 0 -and $expiredGate.counts.open -eq 1 -and -not $expiredGate.complete) 'expiry alone cannot start a replacement before the guarded reclaim'
    $reclaimed = Run-Digest @('ledger','reclaim','-Out',$out,'-Unit',$claim.id)
    Assert ($reclaimed.reclaimed.Count -eq 1 -and $reclaimed.reclaimed[0].previous_owner -eq 'expired-manager' -and $reclaimed.reclaimed[0].token -gt $claim.token) 'only the guarded reclaim of the expired owner offers the next attempt and fences the old token'
    $replacement = Run-Digest @('ledger','next','-Out',$out,'-Layer','terms','-Take','1','-Owner','replacement-manager')
    Assert ($replacement.units.Count -eq 1 -and $replacement.units[0].id -eq $claim.id -and $replacement.units[0].token -gt $reclaimed.reclaimed[0].token) 'the replacement attempt needs a newer token than the reclaimed claim'

    # #92: a claim that starts a provider run asks for a lease that covers the run's whole cap, so a
    # healthy job that outlives the 120-second base lease keeps its original claim.
    $leaseOut = Join-Path $scratch 'lease-prepared'
    $null = Run-Digest @('prepare','-Source',$source,'-Out',$leaseOut,'-CitationKey','LeaseBook','-BibTeX','@book{LeaseBook,title={Lease book}}','-StartHeading','导论','-EndHeading','参考','-MaxUnits','1','-MinUnitChars','1','-NoFigures','-Variant','econ','-Apply')
    $savedLease = $env:ZT_LEDGER_LEASE_SECONDS
    $env:ZT_LEDGER_LEASE_SECONDS = '2'
    try {
        $slow = Run-Digest @('ledger','next','-Out',$leaseOut,'-Layer','terms','-Take','1','-Owner','slow-job','-Lease','60')
        $slowClaim = $slow.units[0]
        Assert ($slowClaim.lease_seconds -ge 60) "a claim can request the lease its provider run needs ($($slowClaim.lease_seconds) s over a 2 s base)"
        Start-Sleep -Seconds 3
        $during = Run-Digest @('ledger','status','-Out',$leaseOut,'-Layer','terms')
        Assert ($during.units[0].state -eq 'claimed' -and $during.units[0].token -eq $slowClaim.token) 'the claim outlives the base lease without any renewal'
        $renewed = Run-Digest @('ledger','renew','-Out',$leaseOut,'-Unit',$slowClaim.id,'-Token',"$($slowClaim.token)")
        Assert ($renewed.token -eq $slowClaim.token -and $renewed.lease_until -ge $slowClaim.lease_until) 'the original token still renews its claim after the base lease passed'
        Refuses @('ledger','reclaim','-Out',$leaseOut,'-Unit',$slowClaim.id) 'claim_active'
        Assert $true 'no guarded reclaim can revoke a claim whose lease covers its job'
        # The control: the same guarded reclaim does take a claim whose lease really expired, so the
        # refusal above is the lease, not a dead code path.
        & node --no-warnings -e $expire (Join-Path $leaseOut 'ledger.sqlite3') $slowClaim.id "$($slowClaim.token)"
        if ($LASTEXITCODE) { throw 'isolated lease expiry injection failed' }
        $expiredLease = Run-Digest @('ledger','reclaim','-Out',$leaseOut,'-Unit',$slowClaim.id)
        Assert (@($expiredLease.reclaimed).Count -eq 1 -and $expiredLease.reclaimed[0].previous_token -eq $slowClaim.token) 'an expired claim is the only one the guarded reclaim takes'
        # A stopped run's claims need not wait out their lease: -Release takes a live-lease claim once
        # nothing happened to it for the quiet window, and fences the old token like any reclaim.
        $held = (Run-Digest @('ledger','next','-Out',$leaseOut,'-Layer','terms','-Take','1','-Owner','stopped-run','-Lease','600')).units[0]
        Refuses @('ledger','reclaim','-Out',$leaseOut,'-Unit',$held.id,'-Release') 'claim_recent'
        $savedQuiet = $env:ZT_LEDGER_RELEASE_QUIET_SECONDS
        $env:ZT_LEDGER_RELEASE_QUIET_SECONDS = '1'
        try {
            Start-Sleep -Seconds 2
            $released = Run-Digest @('ledger','reclaim','-Out',$leaseOut,'-Release')
            Assert (@($released.reclaimed).Count -eq 1 -and $released.reclaimed[0].previous_token -eq $held.token -and $released.reclaimed[0].released) 'a quiet claim of a stopped run is released before its lease ends'
            Refuses @('ledger','renew','-Out',$leaseOut,'-Unit',$held.id,'-Token',"$($held.token)") 'stale_claim'
        } finally {
            $env:ZT_LEDGER_RELEASE_QUIET_SECONDS = $savedQuiet
        }
        # A long step keeps its claims current with one bounded command: `ledger hold` renews each listed claim
        # every -Every seconds and releases one that is no longer held (here a fenced, reclaimed token).
        $kept = (Run-Digest @('ledger','next','-Out',$leaseOut,'-Layer','terms','-Take','1','-Owner','long-step','-Lease','60')).units[0]
        $hold = Run-Digest @('ledger','hold','-Out',$leaseOut,'-Units',"$($kept.id):$($kept.token),$($held.id):$($held.token)",'-Seconds','3','-Every','1')
        Assert (@($hold.held).Count -eq 1 -and $hold.held[0].unit -eq $kept.id -and $hold.renewals -ge 2) "a hold renews the claim it still holds (renewals $($hold.renewals))"
        Assert (@($hold.released).Count -eq 1 -and $hold.released[0].unit -eq $held.id -and $hold.released[0].token -eq $held.token) 'a hold releases a claim whose token is no longer current'
        $afterHold = Run-Digest @('ledger','status','-Out',$leaseOut,'-Units',$kept.id)
        Assert ($afterHold.units[0].state -eq 'claimed' -and $afterHold.units[0].token -eq $kept.token) 'the held claim is still the same claim'
        Refuses @('ledger','hold','-Out',$leaseOut,'-Units',"$($kept.id)") 'each entry is <unit id>:<token>'
        Refuses @('ledger','hold','-Out',$leaseOut,'-Units',"$($kept.id):$($kept.token)",'-Seconds','9999') '-Seconds must be a whole number of seconds from 1 through 580'
        Refuses @('ledger','next','-Out',$leaseOut,'-Layer','terms','-Take','1','-Owner','out-of-range','-Lease','0') '-Lease must be a whole number of seconds from 1 through 7200' 2
        Refuses @('ledger','next','-Out',$leaseOut,'-Layer','terms','-Take','1','-Owner','out-of-range','-Lease','99999') '-Lease must be a whole number of seconds from 1 through 7200' 2
    } finally {
        $env:ZT_LEDGER_LEASE_SECONDS = $savedLease
    }
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
