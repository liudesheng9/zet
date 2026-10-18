#Requires -Version 7.5
# Bounded lock waits and daemon restart diagnostics (#84, R314-R321, T10/T27).
#
#   pwsh -NoProfile -File scripts\book-digest\tests\service-health.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
#
# Every case runs on a scratch archive under the temp folder with its own ZT_CONFIG_DIR, so it
# never touches the live archive. The suite drives the public command boundary: real `zt`, real
# `zt mcp` children, the real edit lock and the real daemon.
param([string]$Zt='', [string]$Digest=$env:ZT_DIGEST)
$ErrorActionPreference='Stop';$utf8=[Text.UTF8Encoding]::new($false)
$scratch=[IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "service-health-test-$PID"))
$tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if(-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'scratch escaped temp directory'}
New-Item -ItemType Directory -Force $scratch | Out-Null
if($Zt){$env:PATH="$(Split-Path -Parent $Zt);$env:PATH"}
$env:ZT_GUI_PORT='0'
if(-not $Digest){$Digest=(Get-Command zt-digest -CommandType Application|Select-Object -First 1).Source}
$env:ZT_CONFIG_DIR=Join-Path $scratch 'config';New-Item -ItemType Directory -Force $env:ZT_CONFIG_DIR|Out-Null
$archive=Join-Path $scratch 'archive';$staging=Join-Path $scratch 'staging'
New-Item -ItemType Directory -Force $staging|Out-Null
$log=Join-Path $archive 'zt.log'
function Assert($condition,$message){if(-not $condition){throw $message};Write-Output "ok: $message"}
function Digest([string[]]$arguments){
    $lines=@(& $Digest @arguments 2>&1|ForEach-Object{"$_"})
    $script:Code=$LASTEXITCODE
    $script:Text=($lines -join "`n")
    if($script:Text.Length){Write-Output $script:Text}
    return $script:Text
}
function DigestJson([string[]]$arguments){$text=Digest $arguments;$text.Split("`n")[-1].Trim()|ConvertFrom-Json -AsHashtable -DateKind String}
function LogText{if(Test-Path -LiteralPath $log){Get-Content -LiteralPath $log -Raw}else{''}}
function Wait-Until([scriptblock]$condition,[string]$what,[int]$seconds=20){
    $until=(Get-Date).AddSeconds($seconds)
    while((Get-Date) -lt $until){if(& $condition){return $null};Start-Sleep -Milliseconds 50}
    throw "timed out waiting until $what"
}
function ServicePid{((zt status|Where-Object{$_ -match '^pid: '}) -split ': ',2)[1]}
function Hold-Lock([string]$path,[string]$ready,[int]$seconds){
    $job=Start-Job -ScriptBlock {
        param($path,$ready,$seconds)
        $marker="zt edit lock`npid $PID`n"
        [IO.File]::WriteAllText($path,$marker,[Text.UTF8Encoding]::new($false))
        # Opened without delete sharing: a writer cannot remove the stale file (os error 32) but
        # can still read the marker, so the holder stays observable.
        $handle=[IO.File]::Open($path,'Open','ReadWrite','ReadWrite')
        [IO.File]::WriteAllText($ready,[string]$PID,[Text.UTF8Encoding]::new($false))
        Start-Sleep -Seconds $seconds
        $handle.Dispose()
    } -ArgumentList $path,$ready,$seconds
    $job
}
$failure=$null
try{
    zt config set archive_root $archive|Out-Null
    zt up|Out-Null
    Assert ((zt status)-contains 'state: up') 'the scratch service is up'
    $first=ServicePid
    Assert ($first -match '^\d+$') 'the service record names its daemon pid'

    # --- T10: a contended batch write waits within its own patience and logs one wait and one completion
    Assert (-not $env:ZT_EDIT_PATIENCE) 'the calling environment declares no edit patience: the child MCP must get its own'
    $lockPath=Join-Path $archive 'zt.edit.lock'
    $ready=Join-Path $scratch 'lock-held.txt'
    $holder=Hold-Lock $lockPath $ready 8
    Wait-Until {Test-Path -LiteralPath $ready} 'the lock holder is in place'
    $holderPid=(Get-Content -LiteralPath $ready -Raw).Trim()
    $before=(LogText)
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $out=Digest @('call','-Tool','create_topic','-Json','{"title":"Contended topic"}')
    $watch.Stop()
    Assert ($script:Code -eq 0) "a contended batch write commits after the holder releases: $out"
    Assert ($watch.Elapsed.TotalSeconds -ge 5) 'the write waited for the live holder instead of removing its file'
    $added=(LogText).Substring($before.Length)
    $waits=@($added -split "`n"|Where-Object{$_ -match 'edit wait: '})
    Assert ($waits.Count -eq 1) "exactly one wait event, never one line per poll: $($waits -join ' / ')"
    Assert ($waits[0] -match "op=create_topic") 'the wait event names the operation that waited'
    Assert ($waits[0] -match "holder=pid:$holderPid") 'the wait event names the observable holder pid'
    Assert ($waits[0] -match 'waiter_pid=\d+') 'the wait event names the waiting process'
    Assert ($waits[0] -match 'patience_s=180') 'the child MCP received the digest batch patience of 180 seconds (R315)'
    $elapsedMs=[int](($waits[0] -split 'elapsed_ms=')[1] -split ' ')[0]
    Assert ($elapsedMs -ge 5000) 'the wait event is written only after five seconds'
    $done=@($added -split "`n"|Where-Object{$_ -match 'edit wait done: '})
    Assert ($done.Count -eq 1) "exactly one completion for the wait: $($done -join ' / ')"
    Assert ($done[0] -match 'outcome=acquired') 'the completion records that the write acquired the lock'
    $queueMs=[int](($done[0] -split 'queue_wait_ms=')[1] -split ' ')[0]
    Assert ($queueMs -ge 5000) 'the completion records the queue wait (R317)'
    Assert ($done[0] -match 'held_ms=\d+') 'the completion records the lock-held duration (R317)'
    Assert (-not (Test-Path -LiteralPath $lockPath)) 'the released lock leaves no file behind'
    zt status|Out-Null
    Assert ($LASTEXITCODE -eq 0) 'the archive is still healthy after the contended write'
    Wait-Job $holder -Timeout 30|Out-Null;Remove-Job $holder -Force

    # --- T10: a fault is not waited out as if it were a busy lock
    $guard=Join-Path $archive 'zt.edit.guard'
    Remove-Item -LiteralPath $guard -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Path $guard|Out-Null
    $before=(LogText)
    $watch.Restart()
    $faulted=Digest @('call','-Tool','create_topic','-Json','{"title":"Never written"}')
    $watch.Stop()
    Assert ($script:Code -ne 0) "an unusable lock file is a fault: $faulted"
    Assert ($faulted -notmatch 'edit in progress') 'an access fault is never reported as a busy lock'
    Assert ($faulted -match 'zt\.edit\.guard') 'the fault names the lock file it could not use'
    Assert ($watch.Elapsed.TotalSeconds -lt 10) 'a fault fails at once instead of waiting out a patience'
    Assert (-not ((LogText).Substring($before.Length) -match 'edit wait: ')) 'a fault logs no wait event'
    Remove-Item -LiteralPath $guard -Force -Recurse

    # --- T27: the daemon disappears, the stage health check waits for the restart and reconnects
    $prefix='run/service-health/1/create'
    $firstOut=Digest @('call','-Tool','create_topic','-Json','{"title":"Survivor"}','-OperationPrefix',$prefix)
    Assert ($script:Code -eq 0) "the receipt-identified write commits: $firstOut"
    $address=($firstOut -split "`n")[0] -replace '^0 ok ',''
    $revision=((zt status|Where-Object{$_ -match '^revision: '}) -split ': ',2)[1]
    Assert ($address -match '^\d+' -and $revision -match '^\d+$') "the write returned address $address at revision $revision"
    $gone=[int](ServicePid)
    Stop-Process -Id $gone -Force
    Wait-Until {-not (Get-Process -Id $gone -ErrorAction SilentlyContinue)} 'the force-killed daemon is gone'
    Assert (Test-Path -LiteralPath (Join-Path $archive 'zt.pid')) 'a force kill leaves the service record behind'
    $waited=DigestJson @('service-wait','-Wait','60','-Out',$staging)
    # --- R318: a staging-stage command records the archive's active-run marker
    $marker=Join-Path $archive 'zt.digest.run'
    Assert (Test-Path -LiteralPath $marker) 'a digest command records an active-run marker in the archive'
    $facts=Get-Content -LiteralPath $marker -Raw|ConvertFrom-Json -AsHashtable
    Assert ($facts.schema_version -eq 1 -and $facts.ttl_s -eq 300) 'the marker carries its schema and a finite lifetime'
    Assert (($facts.renewed -as [long]) -gt ([DateTimeOffset]::UtcNow.ToUnixTimeSeconds()-120)) 'the marker is fresh'
    Assert ($facts.run_id -and $facts.out -eq $staging) 'the marker names its run and the staging folder of the command'
    Assert ($facts.pid -match '^\d+$') 'the marker names the command process that renewed it'
    Assert ($script:Code -eq 0) 'the stage health check succeeds after the restart'
    Assert ($waited.ok -eq $true -and $waited.state -eq 'up') 'the health check reports the service up'
    $restarted=ServicePid
    Assert ($restarted -ne $first) 'the restart is a new daemon process'
    Assert ($waited.pid -eq $restarted) 'the health check reports the daemon that serves the stage'
    Assert ($waited.reconnected -eq $true) 'the health check reconnects one client before the stage continues'
    Assert ($waited.archive_revision -eq $revision) 'the reconnected view is the same archive revision'
    Assert ($waited.last_exit_abnormal -eq $true) 'the health check reports the unexplained disappearance'
    $restarts=@((LogText) -split "`n"|Where-Object{$_ -match 'service restarted: daemon '})
    Assert ($restarts.Count -eq 1) 'exactly one restart is recorded: no repeated daemons'
    Assert ($restarts[0] -match "daemon $first ") 'the restart names the daemon that disappeared'
    $abnormal=@((LogText) -split "`n"|Where-Object{$_ -match 'daemon abnormal exit: '})
    Assert ($abnormal.Count -eq 1) 'the next start records the abnormal disappearance once'
    Assert ($abnormal[0] -match "pid=$first ") 'the abnormal record keeps the missing daemon pid'
    Assert ($abnormal[0] -match 'exit_info=none' -and $abnormal[0] -match 'cause=unknown') 'no OS exit evidence is claimed for a force kill (R319)'
    Assert ($abnormal[0] -match 'version=' -and $abnormal[0] -match 'archive=' -and $abnormal[0] -match 'revision=') 'the abnormal record keeps version, archive and revision'
    $starts=@((LogText) -split "`n"|Where-Object{$_ -match 'daemon started '})
    Assert ($starts.Count -eq 2) "the restart started exactly one new daemon: $($starts.Count) starts"
    # The same receipt keys replay after the restart: committed writes are reconciled, not repeated.
    $again=Digest @('call','-Tool','create_topic','-Json','{"title":"Survivor"}','-OperationPrefix',$prefix)
    Assert ($script:Code -eq 0) "the retried write replays its receipt: $again"
    Assert (($again -split "`n")[0] -eq "0 ok $address") 'the replay returns the original address instead of creating a second Card'
    Assert (((zt status|Where-Object{$_ -match '^revision: '}) -split ': ',2)[1] -eq $revision) 'the replay publishes no new revision'
    zt status|Out-Null
    Assert ($LASTEXITCODE -eq 0) 'the archive stays healthy after the restart and replay'

    # --- T27: a service that stays down is one infrastructure fault, not a new daemon
    zt down|Out-Null
    Wait-Until {((zt status)-contains 'state: down')} 'the service is down'
    $before=(LogText)
    $down=DigestJson @('service-wait','-Wait','2','-Out',$staging)
    Assert ($script:Code -eq 4) 'a service that stays down is an infrastructure fault (exit 4)'
    Assert ($down.ok -eq $false -and $down.code -eq 'infrastructure_fault') 'the fault is typed'
    Assert ($down.error -match 'still down') 'the fault names the service that did not come up'
    Assert (-not ((LogText).Substring($before.Length) -match 'daemon started')) 'no daemon is launched for a service that stays down'
    Assert (-not (Test-Path -LiteralPath (Join-Path $archive 'zt.pid'))) 'the stopped service keeps no record'
}catch{
    $failure=$_
}finally{
    Get-Job|Remove-Job -Force -ErrorAction SilentlyContinue
    if(Test-Path -LiteralPath $archive){
        zt down 2>&1|Out-Null
        Remove-Item -LiteralPath (Join-Path $archive 'zt.pid') -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Milliseconds 300
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
if($failure){throw $failure}
Write-Output 'all passed'
