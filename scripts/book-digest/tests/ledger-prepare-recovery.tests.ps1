#Requires -Version 7.5
param([string]$Zt='', [string]$Digest=$env:ZT_DIGEST)
$ErrorActionPreference='Stop';$utf8=[Text.UTF8Encoding]::new($false)
$scratch=[IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-prepare-recovery-test-$PID"))
$tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {throw 'scratch escaped temp directory'}
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) {$env:PATH="$(Split-Path -Parent $Zt);$env:PATH"}
if (-not $Digest) {$Digest=(Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source}
$env:ZT_DIGEST=$Digest;$env:ZT_CONFIG_DIR=Join-Path $scratch 'config';$env:ZT_GUI_PORT='0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition,$message) {if(-not $condition){throw $message};Write-Output "ok: $message"}
function Run-Digest([string[]]$Arguments){$lines=@(& $Digest @Arguments 2>&1);if($LASTEXITCODE){throw "digest failed: $($lines -join ' ')"};return $lines[-1]|ConvertFrom-Json -AsHashtable}
function Refused($arguments,$text,$message){$lines=@(& $Digest @arguments 2>&1);$detail=$lines -join ' ';Assert ($LASTEXITCODE -ne 0 -and $detail.Contains($text)) "$message [$($detail.Substring(0,[Math]::Min(400,$detail.Length)))]"}
$injector=@'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-prepare-recovery-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.exec(process.argv[2]);db.close();
'@
function Mutate([string]$sql){& node --no-warnings -e $injector (Join-Path $out 'ledger.sqlite3') $sql;if($LASTEXITCODE){throw 'isolated registration injection failed'}}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null;zt up | Out-Null
    $source=Join-Path $scratch 'book.md';$out=Join-Path $scratch 'prepared'
    $parts=@("# 导论`n`n企业的融资约束影响生产选择。")
    for($i=2;$i -le 66;$i++){$parts+="# 第${i}章`n`n融资成本改变第${i}项投资选择。"}
    $parts+="# 参考`n`n不在范围。";[IO.File]::WriteAllText($source,($parts -join "`n`n"),$utf8)
    $original=[IO.File]::ReadAllBytes($source)
    $arguments=@('prepare','-Source',$source,'-Out',$out,'-CitationKey','RecoverPrepare','-BibTeX','@book{RecoverPrepare,title={Recover preparation}}','-StartHeading','导论','-EndHeading','参考','-MaxUnits','1','-MinUnitChars','1','-NoFigures','-Variant','econ','-Apply')
    $env:ZT_CHECK_ZT=Join-Path $PSScriptRoot 'seam-zt-mcp.cmd';$env:ZT_DROP_BATCH_REPLY_FLAG=Join-Path $scratch 'setup-reply-lost.flag'
    $lost=@(& $Digest @arguments 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and (Test-Path $env:ZT_DROP_BATCH_REPLY_FLAG) -and (Test-Path (Join-Path $out 'ledger.sqlite3'))) "an identified setup retains its ledger and run identity when the first committed chunk reply is lost [$($lost -join ' ')]"
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_DROP_BATCH_REPLY_FLAG
    $pending=Run-Digest @('ledger','status','-Out',$out)
    $prepare=@($pending.units | Where-Object id -EQ 'prepare/book@1')[0]
    Assert ($prepare.state -eq 'claimed' -and -not $pending.complete -and @($pending.units | Where-Object layer -EQ digest).Count -eq 66 -and @($pending.units | Where-Object {$_.layer -ne 'prepare' -and $_.attempts -ne 0}).Count -eq 0) 'the entire target exists before setup completion and dependency waiting spends no attempts'
    $tree=Invoke-Zt @(@{tool='get_tree';arguments=@{tree='RecoverPrepare'}})
    Assert ($tree.cards.Count -eq 64) 'one real 64-operation setup chunk commits atomically before its reply disappears'
    $manifestPath=Join-Path $out 'manifest.json';$manifest=Get-Content -LiteralPath $manifestPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $contract=$manifest.run_contract | ConvertTo-Json -Depth 35 -Compress
    $files=@($manifest.run_contract.artifacts.Values)+@($manifest.run_contract.groups)+@($manifest.run_contract.groups.sections)
    $hashes=@{};foreach($file in $files){$hashes[$file.path]=(Get-FileHash -LiteralPath $file.path -Algorithm SHA256).Hash}
    $plan=$prepare.progress.prepare_setup.plan.path
    $doneArgs=@('ledger','done','-Out',$out,'-Unit','prepare/book@1','-Token',"$($prepare.token)",'-Outputs',$plan)
    $renewed=Run-Digest @('ledger','renew','-Out',$out,'-Unit','prepare/book@1','-Token',"$($prepare.token)")
    Assert ($renewed.unit -eq 'prepare/book@1' -and $renewed.token -eq $prepare.token) 'a current pending preparation can renew before its tail anchors exist'
    $revision=(Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    [IO.File]::AppendAllText($source,"`n已改写。",$utf8)
    Refused $doneArgs 'source hash mismatch' 'setup recovery cannot reinterpret a changed source'
    [IO.File]::WriteAllBytes($source,$original)
    $setupPath=$manifest.run_contract.artifacts.setup_calls.path;$setupBytes=[IO.File]::ReadAllBytes($setupPath)
    [IO.File]::AppendAllText($setupPath," `n",$utf8)
    Refused $doneArgs 'setup' 'even a changed retained setup file must be rejected before any new archive write'
    [IO.File]::WriteAllBytes($setupPath,$setupBytes)
    Assert ((Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $revision) 'refused setup recovery leaves the committed prefix unchanged'
    $prefixCard=Invoke-Zt @(@{tool='get_card';arguments=@{at='RecoverPrepare/20'}})
    Invoke-Zt @(@{tool='edit_card';arguments=@{at='RecoverPrepare/20';body=($prefixCard.body+"`n`n用户修改。")}}) | Out-Null
    $changedRevision=(Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    Refused $doneArgs 'current output differs' 'recovery reconciles the committed prefix before creating unseen anchors'
    Assert ((Invoke-Zt @(@{tool='get_tree';arguments=@{tree='RecoverPrepare'}})).cards.Count -eq 64 -and (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $changedRevision) 'a conflicting committed Card leaves the unseen tail untouched'
    Invoke-Zt @(@{tool='edit_card';arguments=@{at='RecoverPrepare/20';body=$prefixCard.body}}) | Out-Null
    $revision=(Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    Mutate "DELETE FROM deps WHERE unit_id='digest/g066@1' AND needs_id='prepare/book@1'"
    Refused $doneArgs 'preparation registration' 'an incomplete prepared target cannot release workers or write the remaining anchors'
    Assert ((Invoke-Zt @(@{tool='get_tree';arguments=@{tree='RecoverPrepare'}})).cards.Count -eq 64) 'missing prerequisite registration blocks the unseen tail'
    Mutate "INSERT INTO deps(unit_id,needs_id,condition) VALUES('digest/g066@1','prepare/book@1','success')"
    $env:ZT_LEDGER_COMPLETE_FAULT='before_commit'
    Refused $doneArgs 'injected completion fault: before_commit' 'a ledger commit failure can follow the accepted archive setup without losing its receipts'
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    $interrupted=Run-Digest @('ledger','status','-Out',$out)
    Assert (@($interrupted.units|Where-Object id -EQ 'prepare/book@1')[0].state -eq 'claimed' -and @($interrupted.units|Where-Object id -EQ 'terms/book@1')[0].state -eq 'registered') 'failed completion rolls back ledger outputs, done state and worker release together'
    $env:ZT_LEDGER_COMPLETE_FAULT='after_commit'
    Refused $doneArgs 'injected completion fault: after_commit' 'lost acknowledgment after the ledger commit retains a verified done preparation'
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    $done=Run-Digest $doneArgs
    Assert ($done.state -eq 'done' -and $done.receipt_keys.Count -eq 67 -and $done.anchors.Count -eq 66) 'typed preparation recovery verifies the root and all anchors before completing the producer'
    Assert ((Invoke-Zt @(@{tool='get_tree';arguments=@{tree='RecoverPrepare'}})).cards.Count -eq 67 -and (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $revision+1) 'only the three unseen anchors are created without duplicating the committed prefix'
    $after=Get-Content -LiteralPath $manifestPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert (($after.run_contract | ConvertTo-Json -Depth 35 -Compress) -ceq $contract) 'recovery preserves the exact frozen preparation contract'
    foreach($file in $files){if((Get-FileHash -LiteralPath $file.path -Algorithm SHA256).Hash -cne $hashes[$file.path]){throw "frozen file changed: $($file.path)"}}
    Assert $true 'source sections, groups, prefixes and the accepted setup are not regenerated during recovery'
    $status=Run-Digest @('ledger','status','-Out',$out,'-Layer','prepare')
    Assert ($status.complete -and $status.run_id -eq $pending.run_id -and $status.units[0].token -eq $prepare.token) 'successful recovery retains the same run and claim identity'
    $again=Run-Digest $doneArgs
    Assert ($again.reused -and $again.state -eq 'done' -and (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $revision+1) 'repeating checked preparation completion is read-only'
    Mutate "DELETE FROM outputs WHERE unit_id='prepare/book@1' AND output_key='completion'"
    Refused $doneArgs 'preparation output registration' 'a done preparation cannot reuse missing completion registration'
    $damaged=Run-Digest @('ledger','status','-Out',$out,'-Layer','prepare')
    Assert (-not $damaged.complete -and $damaged.coverage_faults.code -contains 'preparation_outputs_mismatch') 'offline preparation coverage does not equate a done state with complete output evidence'
    $record=$done.completion
    $locator=$record.path.Replace("'","''");$recordJson=($record|ConvertTo-Json -Compress).Replace("'","''")
    Mutate "INSERT INTO outputs(unit_id,output_key,kind,locator,fingerprint,data_json) VALUES('prepare/book@1','completion','file','$locator','$($record.sha256)','$recordJson')"
    $next=Run-Digest @('ledger','next','-Out',$out,'-Layer','terms','-Take','1','-Owner','terms-manager')
    Assert ($next.units.Count -eq 1) 'only verified setup completion releases the normal Terms prerequisite'
    $wrongArgs=$arguments.Clone();$wrongArgs[4]=Join-Path $scratch 'wrong-key';$wrongArgs[6]='ExpectedKey';$wrongArgs[8]='@book{DifferentKey,title={Wrong key}}'
    Refused $wrongArgs 'Citation key' 'a differing BibTeX Citation key refuses before creating a foreign Literature tree'
    $trees=(Invoke-Zt @(@{tool='list_trees';arguments=@{}})).trees.tree
    Assert ('ExpectedKey' -notin $trees -and 'DifferentKey' -notin $trees) 'Citation key validation leaves neither the requested nor the unexpected tree'
} finally {
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_DROP_BATCH_REPLY_FLAG,Env:ZT_LEDGER_COMPLETE_FAULT -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
