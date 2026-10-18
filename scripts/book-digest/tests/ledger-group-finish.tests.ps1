#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference='Stop'; $utf8=[Text.UTF8Encoding]::new($false)
$scratch=[IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-group-finish-test-$PID"))
$tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {throw 'scratch escaped temp directory'}
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) {$env:PATH="$(Split-Path -Parent $Zt);$env:PATH"}
if (-not $Digest) {$Digest=(Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source}
$env:ZT_DIGEST=$Digest; $env:ZT_CONFIG_DIR=Join-Path $scratch 'config'; $env:ZT_GUI_PORT='0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$out=Join-Path $scratch 'prepared'
function Assert($condition,$message) {if (-not $condition) {throw $message}; Write-Output "ok: $message"}
function Run-Digest([string[]]$Arguments) {
    $lines=@(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) {throw "digest failed: $($lines -join ' ')"}
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Save-Json($path,$value) {[IO.File]::WriteAllText($path,($value | ConvertTo-Json -Depth 30),$utf8)}
function Refused($arguments,$text,$message) {
    $lines=@(& $Digest @arguments 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($lines -join ' ').Contains($text)) "$message [$($lines -join ' ')]"
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $source=Join-Path $scratch 'book.md'; $options=Join-Path $scratch 'options.json'
    [IO.File]::WriteAllText($source,"# 导论`n`n企业的融资约束影响生产选择。`n`n# 参考`n未选择。",$utf8)
    Save-Json $options @{terms=$false}
    $manifest=Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','FinishBook','-BibTeX','@book{FinishBook,title={Finish book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    $claim=(Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','finish-manager')).units[0]
    $started=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claim.token)")
    $job=Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $draft=@{schema_version=1;unit=$claim.id;token=$claim.token;input_hash=$started.input_hash;disposition='cards';calls=@()}
    Save-Json $started.draft $draft
    $args=@('group-finish','-Out',$out,'-Group','1','-Token',"$($claim.token)",'-Draft',$started.draft)
    Refused ($args+@('-CheckOnly')) 'substantive' 'an empty answer and skeleton anchor cannot establish group coverage'
    $draft.disposition='no_cards'; Save-Json $started.draft $draft
    Refused ($args+@('-CheckOnly')) 'source' 'no_cards is refused when the selected immutable source contains prose'
    $draft.disposition='cards'
    $body='融资成本影响企业的生产选择。贷款收紧使企业减少设备投入。投入减少限制下一期的生产能力。来源：[[FinishBook/1]]'
    $draft.calls=@(
        @{tool='edit_card';arguments=@{at='FinishBook/1';title='融资约束影响企业投资';body='融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。';base_fingerprint=$job.inputs.anchor.fingerprint}},
        @{tool='create_successor';arguments=@{at='FinishBook/1';kind='side';title='贷款收紧限制企业投入';body=$body}}
    )
    $ordinaryCalls=$draft.calls
    $draft.calls=@(@{tool='edit_card';arguments=@{at='FinishBook/1';title='融资约束影响企业投资';type='model';level='concept';body=$body;base_fingerprint=$job.inputs.anchor.fingerprint}})
    Save-Json $started.draft $draft
    Refused ($args+@('-CheckOnly')) 'substantive' 'retagging only the anchor as a Model cannot substitute for an additional substantive Card'
    $draft.calls=$ordinaryCalls
    Save-Json $started.draft $draft
    $before=(Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    $checked=Run-Digest ($args+@('-CheckOnly'))
    Assert ($checked.check_only -and -not $checked.applied -and $checked.complete -and $checked.owned.Count -eq 2 -and $checked.substantive.Count -eq 1 -and $checked.self_check.ran) 'CheckOnly validates real proposed Cards and runs the checker'
    $open=Run-Digest @('ledger','status','-Out',$out,'-Layer','digest')
    Assert ($open.units[0].state -eq 'claimed' -and -not $open.complete -and (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $before -and -not (Test-Path (Join-Path $out 'accepted-g001.json'))) 'CheckOnly creates no Cards, receipts, accepted plan or completion transition'
    $small=$draft.calls
    for ($i=1;$i -lt 65;$i++) {$draft.calls+=@{tool='create_successor';arguments=@{at='FinishBook/1';kind='side';title="融资条件影响第${i}项投入";body=$body}}}
    Save-Json $started.draft $draft
    $large=Run-Digest ($args+@('-CheckOnly'))
    Assert ($large.owned.Count -eq 66 -and $large.substantive.Count -eq 65 -and (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $before) 'a complete multi-chunk draft is checked together without a larger write transaction'
    $draft.calls=$small
    $draft.token++; Save-Json $started.draft $draft
    Refused ($args+@('-CheckOnly')) 'binding' 'a draft with the wrong token cannot pass by supplying valid calls'
    $draft.token--; $draft.calls[1].arguments.body="$body`n`n$body"; Save-Json $started.draft $draft
    Refused ($args+@('-CheckOnly')) 'R18' 'exact paragraph corruption is rejected before the archive changes'
    $draft.calls[1].arguments.body=$body
    $draft.calls+=@{tool='edit_card';arguments=@{at='FinishBook';body='越界编辑';base_fingerprint=('a'*64)}}
    Save-Json $started.draft $draft
    Refused ($args+@('-CheckOnly')) 'owned' 'group drafts cannot edit the Literature root outside their ownership'
    $draft.calls=$draft.calls[0..1]; $draft.calls[1].arguments.kind='direct'; Save-Json $started.draft $draft
    Refused ($args+@('-CheckOnly')) 'Side' 'new Cards cannot consume another group direct-anchor location'
    $draft.calls[1].arguments.kind='side'; Save-Json $started.draft $draft
    $draft.calls[1].arguments.at='@@0'; $draft.calls[1].arguments.kind='direct'; Save-Json $started.draft $draft
    Refused ($args+@('-CheckOnly')) 'Side' 'an alias of the edited anchor cannot bypass the Side branch rule'
    $draft.calls[1].arguments.at='FinishBook/1'; $draft.calls[1].arguments.kind='side'; Save-Json $started.draft $draft
    foreach ($point in @('after_outputs','before_children','after_children','before_commit')) {
        $env:ZT_LEDGER_COMPLETE_FAULT=$point
        Refused $args 'injected completion fault' "an injected $point fault rolls back the whole ledger completion"
    }
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    $inspect=@'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-group-finish-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);console.log(JSON.stringify({children:db.prepare("select count(*) n from units where created_by='digest/g001@1'").get().n,outputs:db.prepare("select count(*) n from outputs where unit_id='digest/g001@1'").get().n,events:db.prepare("select count(*) n from events where unit_id='digest/g001@1' and event='completed'").get().n}));db.close();
'@
    $rolled=& node --no-warnings -e $inspect (Join-Path $out 'ledger.sqlite3') | ConvertFrom-Json
    $open=Run-Digest @('ledger','status','-Out',$out,'-Layer','digest')
    Assert ($rolled.children -eq 0 -and $rolled.outputs -eq 0 -and $rolled.events -eq 0 -and $open.units[0].state -eq 'claimed') 'producer state, outputs, completion event and every child share one rollback boundary'
    $after=(Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    Assert ($after -eq $before+1 -and (Invoke-Zt @(@{tool='get_tree';arguments=@{tree='FinishBook'}})).cards.Count -eq 3) 'archive receipt commits survive a later ledger fault without duplicating Cards'
    $done=Run-Digest @('ledger','done','-Out',$out,'-Unit',$claim.id,'-Token',"$($claim.token)",'-Outputs',(Join-Path $out 'completion-g001.json'))
    Assert ($done.state -eq 'done' -and $done.substantive.Count -eq 1 -and $done.children.Count -eq 4 -and $done.self_check.ran) 'retry replays accepted operations and atomically completes the verified group and its four enabled child obligations'
    Assert ((Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $after) 'completion recovery does not advance the archive revision'
    $view=Run-Digest @('ledger','status','-Out',$out)
    Assert (@($view.units | Where-Object {$_.id -in @('cards/g001@1','events/g001@1','findings/g001@1','link/g001@1')}).Count -eq 4 -and -not $view.complete) 'actual downstream units keep the whole run open after digest completion'
    $owned=[IO.File]::ReadAllLines((Join-Path $out 'owned-g01.txt'))
    Assert ($owned.Count -eq 2 -and $owned -contains 'FinishBook/1' -and $owned -contains $done.substantive[0]) 'the existing owned manifest is published from actual receipt-verified addresses'
    Refused @('ledger','done','-Out',$out,'-Unit',$claim.id,'-Token',"$($claim.token)",'-Outputs',$options) 'completion evidence' 'ledger done cannot accept an arbitrary address or option file as completion evidence'
    $again=Run-Digest $args
    Assert ($again.reused -and $again.state -eq 'done' -and $again.children.Count -eq 4) 'a lost final reply can recover the same checked completion without new children'
    $ownedPath=Join-Path $out 'owned-g01.txt'; $ownedBytes=[IO.File]::ReadAllBytes($ownedPath)
    [IO.File]::AppendAllText($ownedPath,"FinishBook/999`n",$utf8)
    Refused $args 'group completion evidence hash mismatch' 'completion replay refuses a changed owned manifest'
    [IO.File]::WriteAllBytes($ownedPath,$ownedBytes)
    $forget=@'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-group-finish-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.prepare("update units set input_hash=? where id='findings/g001@1'").run('0'.repeat(64));db.close();
'@
    & node --no-warnings -e $forget (Join-Path $out 'ledger.sqlite3')
    if ($LASTEXITCODE) {throw 'isolated changed-child fixture failed'}
    $broken=Run-Digest @('ledger','status','-Out',$out,'-Layer','digest')
    Assert (-not $broken.complete -and @($broken.coverage_faults | Where-Object {$_.code -eq 'emitted_unit_missing_or_changed' -and $_.unit -eq 'findings/g001@1'}).Count -eq 1) 'offline gates reconcile the accepted child manifest and name a changed child input'
    Refused $args 'registered child contracts differ' 'a done producer cannot hide a changed downstream obligation'
    $missingOutput=@'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-group-finish-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.prepare("update units set input_hash=? where id='findings/g001@1'").run(process.argv[2]);db.exec("delete from outputs where unit_id='digest/g001@1' and output_key='self_check'");db.close();
'@
    $hash=@($view.units | Where-Object id -EQ 'findings/g001@1')[0].input_hash
    & node --no-warnings -e $missingOutput (Join-Path $out 'ledger.sqlite3') $hash
    if ($LASTEXITCODE) {throw 'isolated missing-output fixture failed'}
    $broken=Run-Digest @('ledger','status','-Out',$out,'-Layer','digest')
    Assert (-not $broken.complete -and @($broken.coverage_faults | Where-Object {$_.code -eq 'producer_outputs_mismatch' -and $_.missing -contains 'self_check'}).Count -eq 1) 'offline gates name a missing required SELF-CHECK output registration'
    Refused $args 'required output registration' 'completion replay cannot pass using an incomplete ledger output set'
} finally {
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
