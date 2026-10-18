#Requires -Version 7.5
param([string]$Zt='', [string]$Digest=$env:ZT_DIGEST, [string]$Checker=$env:ZT_CHECK, [string]$Python=$env:ZT_TEST_PYTHON)
$ErrorActionPreference='Stop';$utf8=[Text.UTF8Encoding]::new($false)
$scratch=[IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "mcp-lifecycle-test-$PID"))
$tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if(-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'scratch escaped temp directory'}
New-Item -ItemType Directory -Force $scratch | Out-Null
if($Zt){$env:PATH="$(Split-Path -Parent $Zt);$env:PATH"}
$env:ZT_GUI_PORT='0'
if(-not $Digest){$Digest=(Get-Command zt-digest -CommandType Application|Select-Object -First 1).Source}
if(-not $Checker){$Checker=(Get-Command zt-check -CommandType Application|Select-Object -First 1).Source}
$env:ZT_CONFIG_DIR=Join-Path $scratch 'config';New-Item -ItemType Directory -Force $env:ZT_CONFIG_DIR|Out-Null
$env:ZT_MCP_TEST_TIMEOUT_MS='1000';$env:ZT_MCP_TEST_SHUTDOWN_MS='300'
$server=Join-Path $scratch 'server.mjs';$launcher=Join-Path $scratch 'server.cmd'
$fixture=@'
import fs from 'node:fs';import readline from 'node:readline';import {spawn} from 'node:child_process';
const mode=process.env.ZT_MCP_LIFECYCLE_MODE;
if(process.argv.includes('--child')){setInterval(()=>{},1000);}else{
  const child=spawn(process.execPath,[process.argv[1],'--child'],{stdio:['ignore','inherit','ignore']});
  fs.writeFileSync(process.env.ZT_MCP_PID_FILE,JSON.stringify({parent:process.pid,child:child.pid}));
  setInterval(()=>{},1000);
  const reply=(id,value)=>process.stdout.write(JSON.stringify({jsonrpc:'2.0',id,result:value})+'\n');
  const input=readline.createInterface({input:process.stdin});
  input.on('line',line=>{
    const m=JSON.parse(line);if(m.id===undefined)return;
    if(m.method==='initialize'){
      if(mode==='initialize_hang')return;
      reply(m.id,{protocolVersion:'2025-06-18',capabilities:{tools:{}},serverInfo:{name:'owned-lifecycle-fixture',version:'1'}});
      if(mode==='write_hang'){input.pause();process.stdin.pause();}
    }else if(mode==='request_hang'){}else if(mode==='noise_hang'){
      setInterval(()=>reply(-100,{noise:true}),20);
    }else reply(m.id,{content:[{type:'text',text:'{"ok":true}'}]});
  });
}
'@
[IO.File]::WriteAllText($server,$fixture,$utf8)
[IO.File]::WriteAllText($launcher,"@node `"$server`" %*`r`n",$utf8)
$env:ZT_CHECK_ZT=$launcher
function Assert($condition,$message){if(-not $condition){throw $message};Write-Output "ok: $message"}
function Run-Case([string]$mode,[string]$exe,[string[]]$arguments){
    $env:ZT_MCP_LIFECYCLE_MODE=$mode;$env:ZT_MCP_PID_FILE=Join-Path $scratch "$mode-pids.json"
    $info=[Diagnostics.ProcessStartInfo]::new($exe);$info.UseShellExecute=$false;$info.CreateNoWindow=$true
    foreach($arg in $arguments){$info.ArgumentList.Add($arg)}
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    $proc=[Diagnostics.Process]::new();$proc.StartInfo=$info
    $watch=[Diagnostics.Stopwatch]::StartNew();$null=$proc.Start()
    $output=$proc.StandardOutput.ReadToEndAsync();$errorOutput=$proc.StandardError.ReadToEndAsync()
    try{
        $finished=$proc.WaitForExit(6000)
        Assert $finished "$mode returns within the public command deadline"
        $text=$output.GetAwaiter().GetResult()+$errorOutput.GetAwaiter().GetResult()
        if($mode -eq 'shutdown_hang'){Assert ($proc.ExitCode -eq 0 -and $text.Contains('ok')) 'a valid reply remains successful when the owned server refuses stdin shutdown'}
        else{Assert ($proc.ExitCode -ne 0 -and $text.Contains('command_timeout')) "$mode reports a typed timeout"}
        $pids=Get-Content -LiteralPath $env:ZT_MCP_PID_FILE -Raw|ConvertFrom-Json
        Assert (-not (Get-Process -Id $pids.parent,$pids.child -ErrorAction SilentlyContinue)) "$mode reaps its owned server and descendant before returning"
        Assert (-not $control.HasExited) "$mode preserves an independently started process"
    }finally{
        if(-not $proc.HasExited){$proc.Kill($true);$proc.WaitForExit(3000)|Out-Null}
        $proc.Dispose()
        if(Test-Path $env:ZT_MCP_PID_FILE){
            $pids=Get-Content -LiteralPath $env:ZT_MCP_PID_FILE -Raw|ConvertFrom-Json
            foreach($id in @($pids.parent,$pids.child)){
                $candidate=Get-CimInstance Win32_Process -Filter "ProcessId=$id" -ErrorAction SilentlyContinue
                if($candidate.CommandLine -and $candidate.CommandLine.Contains($server)){Stop-Process -Id $id -Force -ErrorAction SilentlyContinue}
            }
        }
    }
}
$controlInfo=[Diagnostics.ProcessStartInfo]::new((Get-Command node -CommandType Application|Select-Object -First 1).Source)
$controlInfo.UseShellExecute=$false;$controlInfo.CreateNoWindow=$true;$controlInfo.ArgumentList.Add($server);$controlInfo.ArgumentList.Add('--child')
$control=[Diagnostics.Process]::Start($controlInfo)
try{
    Run-Case 'request_hang' $Digest @('call','-Tool','get_cards','-Json','{"addresses":[],"view":"text"}')
    Run-Case 'initialize_hang' $Digest @('call','-Tool','get_cards','-Json','{"addresses":[],"view":"text"}')
    Run-Case 'noise_hang' $Digest @('call','-Tool','get_cards','-Json','{"addresses":[],"view":"text"}')
    $large=Join-Path $scratch 'large-calls.json'
    [IO.File]::WriteAllText($large,('[{"tool":"create_topic","arguments":{"workspace":"agent","title":"blocked pipe","body":"'+('a'*(2*1024*1024))+'"}}]'),$utf8)
    Run-Case 'write_hang' $Digest @('call',$large)
    Run-Case 'shutdown_hang' $Digest @('call','-Tool','get_cards','-Json','{"addresses":[],"view":"text"}')
    Run-Case 'request_hang' $Checker @('--addresses','MissingFixtureCard')
    $env:ZT_MCP_LIFECYCLE_MODE='request_hang';$env:ZT_MCP_TEST_TIMEOUT_MS='10000'
    $env:ZT_MCP_PID_FILE=Join-Path $scratch 'parent-death-pids.json'
    $deathInfo=[Diagnostics.ProcessStartInfo]::new($Digest);$deathInfo.UseShellExecute=$false;$deathInfo.CreateNoWindow=$true
    foreach($arg in @('call','-Tool','get_cards','-Json','{"addresses":[],"view":"text"}')){$deathInfo.ArgumentList.Add($arg)}
    $owner=[Diagnostics.Process]::Start($deathInfo)
    try{
        $until=[DateTimeOffset]::UtcNow.AddSeconds(4)
        while(-not (Test-Path $env:ZT_MCP_PID_FILE) -and [DateTimeOffset]::UtcNow -lt $until){Start-Sleep -Milliseconds 20}
        Assert (Test-Path $env:ZT_MCP_PID_FILE) 'the parent-death fixture observes its live owned MCP descendants'
        $pids=Get-Content -LiteralPath $env:ZT_MCP_PID_FILE -Raw|ConvertFrom-Json
        $owner.Kill();$owner.WaitForExit(3000)|Out-Null
        $until=[DateTimeOffset]::UtcNow.AddSeconds(3)
        while((Get-Process -Id $pids.parent,$pids.child -ErrorAction SilentlyContinue) -and [DateTimeOffset]::UtcNow -lt $until){Start-Sleep -Milliseconds 20}
        Assert (-not (Get-Process -Id $pids.parent,$pids.child -ErrorAction SilentlyContinue)) 'killing only the compiler terminates its contained MCP tree'
        Assert (-not $control.HasExited) 'compiler death preserves the unrelated control process'
    }finally{
        if(-not $owner.HasExited){$owner.Kill($true);$owner.WaitForExit(3000)|Out-Null};$owner.Dispose()
        $env:ZT_MCP_TEST_TIMEOUT_MS='1000'
    }
    if($Zt){
        Remove-Item Env:ZT_CHECK_ZT
        & $Zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
        $up=@(& $Zt up 2>&1)
        $daemonId=[int]([regex]::Match(($up -join ' '),'service started pid: (\d+)').Groups[1].Value)
        Assert ($LASTEXITCODE -eq 0 -and $daemonId -gt 0) 'the fixture starts its own isolated archive service'
        $daemon=Get-Process -Id $daemonId
        Assert ([IO.Path]::GetFullPath($daemon.Path) -eq [IO.Path]::GetFullPath($Zt)) 'the stale-service fixture identifies its own candidate executable'
        $daemon.Kill();$daemon.WaitForExit(3000)|Out-Null;$daemon.Dispose()
        $lines=@(& $Digest call -Tool get_cards -Json '{"addresses":[],"view":"text"}' 2>&1)
        Assert ($LASTEXITCODE -eq 0) "the owned real MCP restarts its disappeared isolated daemon [$($lines -join ' ')]"
        $restartedId=[int](Get-Content -LiteralPath (Join-Path $scratch 'archive/zt.pid') -Raw)
        Assert ($restartedId -ne $daemonId -and (Get-Process -Id $restartedId -ErrorAction SilentlyContinue)) 'the restarted daemon is still alive before any status call can repair it'
        $status=@(& $Zt status 2>&1)
        Assert ($LASTEXITCODE -eq 0 -and ($status -join ' ').Contains('state: up')) 'closing an owned MCP does not terminate the independent archive daemon it started'
        foreach($point in @('before','after')){
            $calls=Join-Path $scratch "timeout-$point.json"
            [IO.File]::WriteAllText($calls,('[{"tool":"create_topic","arguments":{"workspace":"agent","title":"Timeout '+$point+'","body":"Actual receipt recovery."}}]'),$utf8)
            $prefix="qualification/timeout-$point/1/apply"
            $view=@(& $Digest call -Tool get_cards -Json '{"addresses":[],"view":"text"}' 2>&1)
            if($LASTEXITCODE){throw ($view -join ' ')}
            $beforeRevision=($view[-1].Substring(2)|ConvertFrom-Json).archive_revision
            $env:ZT_CHECK_ZT=Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
            $flag=Join-Path $scratch "hung-$point.flag"
            if($point -eq 'before'){$env:ZT_HANG_BATCH_BEFORE_WRITE_FLAG=$flag}else{$env:ZT_HANG_BATCH_REPLY_FLAG=$flag}
            $lost=@(& $Digest call $calls -OperationPrefix $prefix 2>&1)
            Assert ($LASTEXITCODE -ne 0 -and (Test-Path $flag) -and ($lost -join ' ').Contains('command_timeout') -and ($lost -join ' ').Contains('cleanup_confirmed=true')) "a real $point-commit batch timeout confirms owned cleanup"
            $owned=Get-Content -LiteralPath $flag -Raw|ConvertFrom-Json
            Assert (-not (Get-Process -Id $owned.proxy,$owned.writer -ErrorAction SilentlyContinue)) 'the real proxy and archive writer are both gone before retry'
            Remove-Item Env:ZT_CHECK_ZT,Env:ZT_HANG_BATCH_BEFORE_WRITE_FLAG,Env:ZT_HANG_BATCH_REPLY_FLAG -ErrorAction SilentlyContinue
            $view=@(& $Digest call -Tool get_cards -Json '{"addresses":[],"view":"text"}' 2>&1)
            $afterRevision=($view[-1].Substring(2)|ConvertFrom-Json).archive_revision
            Assert ($afterRevision -eq $beforeRevision+$(if($point -eq 'after'){1}else{0})) "$point-commit timeout retains the actual committed archive state"
            $retry=@(& $Digest call $calls -OperationPrefix $prefix 2>&1)
            Assert ($LASTEXITCODE -eq 0 -and ($retry -join ' ').Contains(' ok ')) 'the same accepted operation recovers its original receipt or performs the unseen write once'
            $again=@(& $Digest call $calls -OperationPrefix $prefix 2>&1)
            Assert ($LASTEXITCODE -eq 0 -and ($again -join ' ') -eq ($retry -join ' ')) 'receipt replay preserves the allocated address after a timeout'
            $view=@(& $Digest call -Tool get_cards -Json '{"addresses":[],"view":"text"}' 2>&1)
            Assert (($view[-1].Substring(2)|ConvertFrom-Json).archive_revision -eq $beforeRevision+1) 'timeout recovery publishes exactly one archive revision'
            Assert (Get-Process -Id $restartedId -ErrorAction SilentlyContinue) 'archive writer cleanup preserves the independent daemon'
        }
        $archiveScript=Join-Path $scratch 'archive-service.mjs';$archiveCommand=Join-Path $scratch "archive service's launcher.cmd"
        $env:ZT_ARCHIVE_TEST_PID_FILE=Join-Path $scratch 'archive-service.pid'
        $listener=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0);$listener.Start();$env:ZT_ARCHIVE_TEST_PORT="$($listener.LocalEndpoint.Port)";$listener.Stop()
        $archiveFixture=@'
import fs from 'node:fs';import http from 'node:http';import {spawn,spawnSync} from 'node:child_process';
const marker=process.env.ZT_ARCHIVE_TEST_PID_FILE;const port=Number(process.env.ZT_ARCHIVE_TEST_PORT);
if(process.argv.includes('--server')){
  http.createServer((q,s)=>{s.writeHead(500,{'content-type':'application/json'});s.end('{"error":"unsigned isolated fixture"}');}).listen(port,'127.0.0.1',()=>fs.writeFileSync(marker,String(process.pid)));
}else if(process.argv.includes('start')){
  if(process.env.ZT_ARCHIVE_TEST_PYTHON){
    const check=spawnSync(process.env.ZT_ARCHIVE_TEST_PYTHON,['-c','import sys; raise SystemExit(23 if sys.stdout is None else 0)'],{stdio:'inherit'});
    if(check.status!==0)process.exit(check.status||23);
  }
  const child=spawn(process.execPath,[process.argv[1],'--server'],{detached:true,stdio:'ignore'});child.unref();
  const until=Date.now()+2000;while(!fs.existsSync(marker)&&Date.now()<until)await new Promise(r=>setTimeout(r,10));
  if(!fs.existsSync(marker))process.exit(1);
}else console.log(JSON.stringify({client:'zet',key_id:'test-only',secret:'1'.repeat(64),url:`http://127.0.0.1:${port}`}));
'@
        [IO.File]::WriteAllText($archiveScript,$archiveFixture,$utf8)
        [IO.File]::WriteAllText($archiveCommand,"@node `"$archiveScript`" %*`r`n",$utf8)
        if($Python){$env:ZT_ARCHIVE_TEST_PYTHON=$Python}else{Remove-Item Env:ZT_ARCHIVE_TEST_PYTHON -ErrorAction SilentlyContinue}
        $env:ZT_SCHOLAR_ARCHIVE=$archiveCommand;$env:ZT_MCP_TEST_TIMEOUT_MS='5000'
        $bridgeInfo=[Diagnostics.ProcessStartInfo]::new($Digest);$bridgeInfo.UseShellExecute=$false;$bridgeInfo.CreateNoWindow=$true
        $bridgeInfo.RedirectStandardOutput=$true;$bridgeInfo.RedirectStandardError=$true
        foreach($arg in @('call','-Tool','scholar_archive_status','-Json','{}')){$bridgeInfo.ArgumentList.Add($arg)}
        $bridge=[Diagnostics.Process]::Start($bridgeInfo);$bridgeOut=$bridge.StandardOutput.ReadToEndAsync();$bridgeErr=$bridge.StandardError.ReadToEndAsync()
        try{
            Assert ($bridge.WaitForExit(8000)) 'shared-service startup returns within its public deadline'
            Assert ($bridgeOut.Wait(2000) -and $bridgeErr.Wait(2000)) 'the independent service cannot retain client capture pipes'
            Assert ($bridge.ExitCode -ne 0 -and (Test-Path $env:ZT_ARCHIVE_TEST_PID_FILE)) 'the isolated bridge starts its fixture service and rejects its unsigned response'
        }finally{if(-not $bridge.HasExited){$bridge.Kill($true);$bridge.WaitForExit(3000)|Out-Null};$bridge.Dispose()}
        $archiveId=[int](Get-Content -LiteralPath $env:ZT_ARCHIVE_TEST_PID_FILE -Raw)
        Assert (Get-Process -Id $archiveId -ErrorAction SilentlyContinue) 'MCP shutdown preserves the independent Scholar Archive service it started'
    }
}finally{
    if($Zt){& $Zt down 2>&1|Out-Null}
    if($env:ZT_ARCHIVE_TEST_PID_FILE -and (Test-Path $env:ZT_ARCHIVE_TEST_PID_FILE)){
        $id=[int](Get-Content -LiteralPath $env:ZT_ARCHIVE_TEST_PID_FILE -Raw)
        $candidate=Get-CimInstance Win32_Process -Filter "ProcessId=$id" -ErrorAction SilentlyContinue
        if($candidate.CommandLine -and $candidate.CommandLine.Contains($archiveScript)){Stop-Process -Id $id -Force -ErrorAction SilentlyContinue}
    }
    if(-not $control.HasExited){$control.Kill();$control.WaitForExit(3000)|Out-Null};$control.Dispose()
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_MCP_LIFECYCLE_MODE,Env:ZT_MCP_PID_FILE,Env:ZT_MCP_TEST_TIMEOUT_MS,Env:ZT_MCP_TEST_SHUTDOWN_MS,Env:ZT_HANG_BATCH_BEFORE_WRITE_FLAG,Env:ZT_HANG_BATCH_REPLY_FLAG -ErrorAction SilentlyContinue
    Remove-Item Env:ZT_SCHOLAR_ARCHIVE,Env:ZT_ARCHIVE_TEST_PID_FILE,Env:ZT_ARCHIVE_TEST_PORT,Env:ZT_ARCHIVE_TEST_PYTHON -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
