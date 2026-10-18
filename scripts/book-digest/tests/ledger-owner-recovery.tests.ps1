#Requires -Version 7.5
# R176/R178/R181/R182/R184/R186: a claim outlives the manager that took it, an expired owner is
# reclaimed only under the unit guard with a fenced token, and a registered external run must be
# cancelled and observed terminal before a replacement attempt may start. Every step uses the real
# zt-digest commands, the real zt mcp/checker boundary and the isolated temporary archive.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference='Stop'; $utf8=[Text.UTF8Encoding]::new($false)
$scratch=[IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-owner-test-$PID"))
$tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {throw 'scratch escaped temp directory'}
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) {$env:PATH="$(Split-Path -Parent $Zt);$env:PATH"}
if (-not $Digest) {$Digest=(Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source}
$env:ZT_DIGEST=$Digest; $env:ZT_CONFIG_DIR=Join-Path $scratch 'config'; $env:ZT_GUI_PORT='0'
$seam=Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
function Assert($condition,$message) {if (-not $condition) {throw $message}; Write-Output "ok: $message"}
function Run-Digest([string[]]$Arguments) {
    $lines=@(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) {throw "digest failed: $($lines -join ' ')"}
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Command-Output([string[]]$Arguments) {
    $lines=@(& $Digest @Arguments 2>&1)
    return [pscustomobject]@{code=$LASTEXITCODE;text=($lines -join ' ')}
}
function Refused([string[]]$Arguments,[string]$Text,[string]$Message) {
    $result=Command-Output $Arguments
    Assert ($result.code -ne 0 -and $result.text.Contains($Text)) "$Message [$($result.text)]"
}
function Save-Json($path,$value) {[IO.File]::WriteAllText($path,($value | ConvertTo-Json -Depth 30),$utf8)}
function Start-Digest([string[]]$Arguments) {
    $psi=[Diagnostics.ProcessStartInfo]::new($Digest)
    $psi.UseShellExecute=$false; $psi.CreateNoWindow=$true
    $psi.RedirectStandardOutput=$true; $psi.RedirectStandardError=$true
    foreach($arg in $Arguments) {$psi.ArgumentList.Add($arg)}
    return [Diagnostics.Process]::Start($psi)
}
function Complete-Digest($process,[int]$TimeoutMs=120000) {
    if (-not $process.WaitForExit($TimeoutMs)) {$process.Kill($true); throw 'background digest process timed out'}
    $stdout=$process.StandardOutput.ReadToEnd(); $stderr=$process.StandardError.ReadToEnd()
    $code=$process.ExitCode; $process.Dispose()
    if ($code) {throw "background digest failed ($code): $stdout $stderr"}
    return ($stdout -split "`n" | Where-Object {$_.Trim()} | Select-Object -Last 1) | ConvertFrom-Json -AsHashtable
}
function Ledger-Rows([string]$out,[string]$sql) {
    $script=@'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
const rel=path.relative(os.tmpdir(),p);
if(rel.startsWith('..')||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-owner-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);const rows=db.prepare(process.argv[2]).all();db.close();console.log(JSON.stringify(rows));
'@
    $json=& node --no-warnings -e $script (Join-Path $out 'ledger.sqlite3') $sql
    if ($LASTEXITCODE) {throw 'isolated ledger read failed'}
    return @($json | ConvertFrom-Json -AsHashtable)
}
function Expire-Claim([string]$out,[string]$unit,[long]$token) {
    $script=@'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
const rel=path.relative(os.tmpdir(),p);
if(rel.startsWith('..')||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-owner-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);
const changed=db.prepare("update units set lease_until=? where id=? and claim_token=? and state='claimed'").run(Math.floor(Date.now()/1000)-1,process.argv[2],Number(process.argv[3]));
db.close();if(changed.changes!==1)throw new Error('the current claim was not expired');
'@
    & node --no-warnings -e $script (Join-Path $out 'ledger.sqlite3') $unit "$token"
    if ($LASTEXITCODE) {throw 'isolated lease expiry injection failed'}
}
function Unit-Of([string]$out,[string]$id) {
    $layer=($id -split '/')[0]
    return @((Run-Digest @('ledger','status','-Out',$out,'-Layer',$layer)).units | Where-Object id -EQ $id)[0]
}
function New-PreparedBook([string]$key) {
    $out=Join-Path $scratch "prepared-$key"
    $source=Join-Path $scratch "book-$key.md"; $options=Join-Path $scratch "options-$key.json"
    [IO.File]::WriteAllText($source,"# 导论`n`n企业的融资约束影响生产选择。`n`n# 参考`n未选择。",$utf8)
    Save-Json $options @{terms=$false}
    $null=Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey',$key,'-BibTeX',"@book{$key,title={$key book}}",'-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    return $out
}
function Complete-Group([string]$out,[string]$key) {
    $claim=(Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','group-manager')).units[0]
    $started=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claim.token)")
    $job=Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $draft=@{schema_version=1;unit=$claim.id;token=$claim.token;input_hash=$started.input_hash;disposition='cards';calls=@(
        @{tool='edit_card';arguments=@{at="$key/1";title='融资约束影响企业投资';body='融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。';base_fingerprint=$job.inputs.anchor.fingerprint}},
        @{tool='create_successor';arguments=@{at="$key/1";kind='side';title='贷款收紧限制企业投入';body='融资成本影响企业的生产选择。贷款收紧使企业减少设备投入。依赖 [[OwnerBook]]，后者给出全书的问题范围。来源：[[OwnerBook/1]]'}}
    )}
    Save-Json $started.draft $draft
    return (Run-Digest @('group-finish','-Out',$out,'-Group','1','-Token',"$($claim.token)",'-Draft',$started.draft))
}
$locked=$null
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    # --- T12, manager half: the claim survives its manager and a separate compiler completes it.
    $outA=New-PreparedBook 'OwnerBook'
    $group=Complete-Group $outA 'OwnerBook'
    Assert ($group.disposition -eq 'verified') 'the group completes and registers its Cards child'
    $cardsClaim=(Run-Digest @('ledger','next','-Out',$outA,'-Layer','cards','-Take','1','-Owner','dead-manager')).units[0]
    Assert ($cardsClaim.id -eq 'cards/g001@1') 'the Cards unit is claimable after its producer completed'
    # The claiming process has exited: the manager is gone, the claim is durable.
    $compiler=Start-Digest @('cards-finish','-Out',$outA,'-Group','1','-Token',"$($cardsClaim.token)")
    $duplicate=Run-Digest @('ledger','next','-Out',$outA,'-Layer','cards','-Take','1','-Owner','second-manager')
    $finished=Complete-Digest $compiler
    Assert ($duplicate.units.Count -eq 0) 'a second manager receives no duplicate claim while the dead manager''s compiler works'
    Assert ($finished.state -eq 'done' -and $finished.token -eq $cardsClaim.token -and -not $finished.reused) 'the compiler completes the claimed unit under the dead manager''s exact token'
    $cardsRow=Unit-Of $outA 'cards/g001@1'
    Assert ($cardsRow.state -eq 'done' -and $cardsRow.attempts -eq 1 -and $cardsRow.owner -eq 'dead-manager' -and $cardsRow.token -eq $cardsClaim.token) 'completion keeps the original owner, one attempt and one token without a replacement claim'

    # --- R178: a long command renews its own lease from an internal heartbeat.
    $outB=New-PreparedBook 'GuardBook'
    $slowFlag=Join-Path $scratch 'slow.started'
    $env:ZT_LEDGER_LEASE_SECONDS='2'; $env:ZT_LEDGER_HEARTBEAT_MS='200'
    $env:ZT_CHECK_ZT=$seam; $env:ZT_SLOW_TOOL='export_titles'; $env:ZT_SLOW_MS='4000'; $env:ZT_SLOW_FLAG=$slowFlag
    $claimB=(Run-Digest @('ledger','next','-Out',$outB,'-Layer','digest','-Take','1','-Owner','vanishing-manager')).units[0]
    $startedCompiler=Start-Digest @('group-start','-Out',$outB,'-Group','1','-Token',"$($claimB.token)")
    $compilerPid=$startedCompiler.Id
    $deadline=(Get-Date).AddSeconds(30)
    while (-not (Test-Path $slowFlag) -and (Get-Date) -lt $deadline) {Start-Sleep -Milliseconds 100}
    Assert (Test-Path $slowFlag) 'the slowed command reached its bounded archive read while holding the claim'
    $whileRunning=Run-Digest @('ledger','next','-Out',$outB,'-Layer','digest','-Take','1','-Owner','impatient-manager')
    $startedResult=Complete-Digest $startedCompiler
    Assert ($whileRunning.units.Count -eq 0) 'a live renewed lease offers no second attempt to another manager'
    Assert ($startedResult.unit -eq 'digest/g001@1' -and $startedResult.token -eq $claimB.token -and -not $startedResult.reused) 'the long command outlives a lease shorter than its own work and finishes under the same token'
    $renewals=@(Ledger-Rows $outB "select event,facts_json from events where unit_id='digest/g001@1' and event='renewed' order by seq")
    $heartbeats=@($renewals | Where-Object {$_.facts_json.Contains('"heartbeat"')})
    $facts=@($heartbeats | ForEach-Object {$_.facts_json | ConvertFrom-Json -AsHashtable})
    $pids=@($facts | ForEach-Object {$_.heartbeat.pid} | Sort-Object -Unique)
    Assert ($heartbeats.Count -ge 3 -and $pids.Count -eq 1 -and $pids[0] -eq $compilerPid) 'each renewal records a heartbeat fact with the compiler process id, not an agent assertion'
    $lastLease=($facts | ForEach-Object {$_.lease_until} | Measure-Object -Maximum).Maximum
    Assert ($lastLease -gt $claimB.lease_until) 'the heartbeat extends the actual lease beyond the claim it started with'
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_SLOW_TOOL,Env:ZT_SLOW_MS,Env:ZT_SLOW_FLAG -ErrorAction SilentlyContinue
    Remove-Item Env:ZT_LEDGER_LEASE_SECONDS,Env:ZT_LEDGER_HEARTBEAT_MS -ErrorAction SilentlyContinue

    # --- T13: an expired owner is reclaimed only under the unit guard and a fenced token.
    Expire-Claim $outB 'digest/g001@1' $claimB.token
    $guardJson='"digest/g001@1"'
    $guardHash=-join ([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($guardJson)) | ForEach-Object {$_.ToString('x2')})
    $guardPath=Join-Path (Join-Path $outB 'locks') "$guardHash.lock"
    Assert (Test-Path $guardPath) 'the per-unit guard file named by the reclaim contract exists for the claimed unit'
    # A live writer holds the OS guard: reclaim must not revoke it in the middle of an apply.
    $locked=[IO.File]::Open($guardPath,'Open','ReadWrite','None')
    Refused @('ledger','reclaim','-Out',$outB,'-Unit','digest/g001@1') 'lock_busy' 'reclaim refuses while another writer holds the per-unit guard'
    $afterContention=Unit-Of $outB 'digest/g001@1'
    Assert ($afterContention.state -eq 'claimed' -and $afterContention.token -eq $claimB.token) 'a refused reclaim changes no claim'
    $locked.Dispose(); $locked=$null
    $reclaimed=Run-Digest @('ledger','reclaim','-Out',$outB,'-Unit','digest/g001@1')
    Assert ($reclaimed.reclaimed.Count -eq 1 -and $reclaimed.reclaimed[0].unit -eq 'digest/g001@1' -and $reclaimed.reclaimed[0].previous_owner -eq 'vanishing-manager' -and $reclaimed.reclaimed[0].previous_token -eq $claimB.token) 'an expired claim is reclaimed after the guard is released, naming the lost owner and token'
    $reclaimedToken=$reclaimed.reclaimed[0].token
    Assert ($reclaimedToken -gt $claimB.token) 'reclaim fences the old owner by moving the claim token forward'
    $readyRow=Unit-Of $outB 'digest/g001@1'
    Assert ($readyRow.state -eq 'ready' -and $null -eq $readyRow.owner -and $readyRow.attempts -eq 1) 'the reclaimed unit is ready with no owner and no extra attempt'
    $fault=Join-Path $scratch 'fault.json'
    [IO.File]::WriteAllText($fault,(@{code='pending_jobs';unit='digest/g001@1';input_hash=$readyRow.input_hash;cause='the lost owner may have left an external run'} | ConvertTo-Json),$utf8)
    Refused @('ledger','renew','-Out',$outB,'-Unit','digest/g001@1','-Token',"$($claimB.token)") 'stale_claim' 'the lost owner cannot renew its old token'
    Refused @('ledger','fail','-Out',$outB,'-Unit','digest/g001@1','-Token',"$($claimB.token)",'-Class','retryable','-Fault',$fault) 'stale_claim' 'the lost owner cannot report a fault under its old token'
    $replacement=(Run-Digest @('ledger','next','-Out',$outB,'-Layer','digest','-Take','1','-Owner','replacement-manager')).units[0]
    Assert ($replacement.id -eq 'digest/g001@1' -and $replacement.token -gt $reclaimedToken -and (Unit-Of $outB 'digest/g001@1').attempts -eq 2) 'the next attempt claims the reclaimed unit with a newer token and its own attempt'
    $lateDraft=Join-Path $outB ('group-jobs\g001\claim-'+$claimB.token+'\draft.json')
    Refused @('group-finish','-Out',$outB,'-Group','1','-Token',"$($replacement.token)",'-Draft',$lateDraft) 'group-start binding' 'a draft job frozen for the lost owner cannot apply under the replacement attempt'

    # --- R184/R186: a registered external run is cancelled and observed before a replacement.
    $register=Join-Path $scratch 'run-1.json'; $observe=Join-Path $scratch 'run-1-observed.json'
    Save-Json $register @{id='run-1';kind='deepseek';pid=4242;facts=@{prompt='group-1';model='deepseek-chat'}}
    $registered=Run-Digest @('ledger','job','-Out',$outB,'-Unit','digest/g001@1','-Token',"$($replacement.token)",'-Register',$register)
    Assert ($registered.action -eq 'register' -and $registered.jobs.Count -eq 1 -and $registered.jobs[0].id -eq 'run-1' -and $registered.jobs[0].state -eq 'running' -and $registered.jobs[0].token -eq $replacement.token) 'a manager registers its external run under the current claim token'
    Expire-Claim $outB 'digest/g001@1' $replacement.token
    $blockedByJob=Command-Output @('ledger','reclaim','-Out',$outB,'-Unit','digest/g001@1')
    Assert ($blockedByJob.code -ne 0 -and $blockedByJob.text.Contains('jobs_running') -and $blockedByJob.text.Contains('run-1')) 'a registered run still running prevents any replacement attempt and is named'
    $cancelled=Run-Digest @('ledger','job','-Out',$outB,'-Unit','digest/g001@1','-Cancel','run-1')
    Assert ($cancelled.jobs[0].cancel_requested_at -and $cancelled.jobs[0].state -eq 'running') 'a cancellation request is recorded while the run is not yet terminal'
    Refused @('ledger','reclaim','-Out',$outB,'-Unit','digest/g001@1') 'jobs_running' 'a requested cancellation is not an observed terminal state'
    Save-Json $observe @{id='run-1';state='terminal';outcome='cancelled';facts=@{observed_by='reclaimer'}}
    $observed=Run-Digest @('ledger','job','-Out',$outB,'-Unit','digest/g001@1','-Observe',$observe)
    Assert ($observed.jobs[0].state -eq 'terminal' -and $observed.jobs[0].outcome -eq 'cancelled' -and $observed.jobs[0].observed_at) 'the terminal observation is a recorded durable fact'
    $missingJob=Join-Path $scratch 'missing-job.json'
    Save-Json $missingJob @{id='never-registered';state='terminal';outcome='finished'}
    Refused @('ledger','job','-Out',$outB,'-Unit','digest/g001@1','-Observe',$missingJob) 'unknown job' 'observing a run that was never registered is refused'
    $runningAgain=Join-Path $scratch 'run-1-running.json'
    Save-Json $runningAgain @{id='run-1';state='running'}
    Refused @('ledger','job','-Out',$outB,'-Unit','digest/g001@1','-Observe',$runningAgain) 'terminal' 'a terminal run cannot return to running'
    $reclaimedAgain=Run-Digest @('ledger','reclaim','-Out',$outB,'-Unit','digest/g001@1')
    Assert ($reclaimedAgain.reclaimed.Count -eq 1 -and $reclaimedAgain.reclaimed[0].token -gt $replacement.token) 'after the terminal observation the same guarded reclaim succeeds and fences the old token'
    Refused @('ledger','job','-Out',$outB,'-Unit','digest/g001@1','-Token',"$($replacement.token)",'-Register',$register) 'stale_claim' 'the old owner cannot register another run under its fenced token'
    $nextClaim=(Run-Digest @('ledger','next','-Out',$outB,'-Layer','digest','-Take','1','-Owner','finishing-manager')).units[0]
    Save-Json $register @{id='run-2';kind='deepseek';pid=4343}
    $null=Run-Digest @('ledger','job','-Out',$outB,'-Unit','digest/g001@1','-Token',"$($nextClaim.token)",'-Register',$register)
    Refused @('group-finish','-Out',$outB,'-Group','1','-Token',"$($nextClaim.token)",'-Draft',$lateDraft,'-CheckOnly') 'jobs_running' 'no completion, not even a preview, is established while a registered run is pending'
    Save-Json $observe @{id='run-2';state='terminal';outcome='finished'}
    $null=Run-Digest @('ledger','job','-Out',$outB,'-Unit','digest/g001@1','-Observe',$observe)
    $afterObserve=Command-Output @('group-finish','-Out',$outB,'-Group','1','-Token',"$($nextClaim.token)",'-Draft',$lateDraft,'-CheckOnly')
    Assert ($afterObserve.code -ne 0 -and $afterObserve.text.Contains('group-start binding') -and -not $afterObserve.text.Contains('jobs_running')) 'once the run is terminal the pending-job gate no longer refuses the attempt'
} finally {
    if ($locked) {$locked.Dispose()}
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_SLOW_TOOL,Env:ZT_SLOW_MS,Env:ZT_SLOW_FLAG,Env:ZT_LEDGER_LEASE_SECONDS,Env:ZT_LEDGER_HEARTBEAT_MS -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
