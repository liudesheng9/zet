#Requires -Version 7.5
param([string]$Zt='', [string]$Digest=$env:ZT_DIGEST)
$ErrorActionPreference='Stop';$utf8=[Text.UTF8Encoding]::new($false)
$scratch=[IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-terms-test-$PID"))
$tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {throw 'scratch escaped temp directory'}
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) {$env:PATH="$(Split-Path -Parent $Zt);$env:PATH"}
if (-not $Digest) {$Digest=(Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source}
$env:ZT_DIGEST=$Digest;$env:ZT_CONFIG_DIR=Join-Path $scratch 'config';$env:ZT_GUI_PORT='0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition,$message) {if (-not $condition){throw $message};Write-Output "ok: $message"}
function Run-Digest([string[]]$Arguments) {
    $lines=@(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE){throw "digest failed: $($lines -join ' ')"}
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Save-Json($path,$value){[IO.File]::WriteAllText($path,($value | ConvertTo-Json -Depth 40),$utf8)}
function Refused($arguments,$text,$message){$lines=@(& $Digest @arguments 2>&1);Assert ($LASTEXITCODE -ne 0 -and ($lines -join ' ').Contains($text)) "$message [$($lines -join ' ')]"}
function Prepared($name,$body,$variant='econ') {
    $source=Join-Path $scratch "$name.md";$out=Join-Path $scratch $name
    [IO.File]::WriteAllText($source,"# 导论`n`n$body`n`n# 参考`n`n未选择。",$utf8)
    $manifest=Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey',$name,'-BibTeX',"@book{$name,title={$name}}",'-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant',$variant,'-Apply')
    return $out
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null;zt up | Out-Null
    $out=Prepared 'DefaultTerms' '企业把融资成本的差额称为**外部融资溢价**。融资成本影响企业的投资选择。'
    Refused @('terms','-Step','plan','-Staging',$out) '-Token' 'an initialized ledger cannot be planned through an unclaimed Terms command'
    Refused @('terms','-Step','apply','-Staging',$out,'-Apply') '-Token' 'an initialized ledger cannot use the unkeyed legacy Terms writer'
    $waiting=Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','group-manager')
    Assert ($waiting.units.Count -eq 0) 'the default enabled Terms prerequisite prevents premature group claims'
    $claim=(Run-Digest @('ledger','next','-Out',$out,'-Layer','terms','-Take','1','-Owner','terms-manager')).units[0]
    $plan=Run-Digest @('terms','-Step','plan','-Staging',$out,'-Token',"$($claim.token)")
    Assert ($plan.schema_version -eq 1 -and $plan.unit -eq 'terms/book@1' -and $plan.token -eq $claim.token -and $plan.expected_jobs -eq 1 -and $plan.jobs.Count -eq 1 -and $plan.input_hash -match '^[0-9a-f]{64}$') 'claimed Terms planning freezes one real job and its exact current binding'
    $planHash=(Get-FileHash -LiteralPath $plan.job -Algorithm SHA256).Hash
    $replan=Run-Digest @('terms','-Step','plan','-Staging',$out,'-Token',"$($claim.token)")
    Assert ($replan.reused -and $replan.input_hash -eq $plan.input_hash -and (Get-FileHash -LiteralPath $plan.job -Algorithm SHA256).Hash -eq $planHash) 'repeated Terms planning preserves its actual frozen job and parameters'
    Refused @('terms','-Step','plan','-Staging',$out,'-Token',"$($claim.token)",'-MaxTerms','61') 'frozen' 'a claimed Terms run cannot silently change its accepted limits'
    $args=@('terms','-Step','apply','-Staging',$out,'-Token',"$($claim.token)",'-Apply')
    $job=Get-Content -LiteralPath $plan.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $prompt=$job.artifacts.prompt.path;$promptBytes=[IO.File]::ReadAllBytes($prompt)
    [IO.File]::AppendAllText($prompt,"`n已改写。",$utf8)
    Refused $args 'Terms plan artifact hash mismatch' 'an edited frozen Terms prompt cannot supply completion evidence'
    [IO.File]::WriteAllBytes($prompt,$promptBytes)
    Refused $args 'missing Terms answer' 'an unproduced expected Terms job cannot complete'
    Save-Json $plan.answer @{}
    Refused $args 'binding' 'an empty JSON object cannot stand in for a successful Terms answer'
    $answer=@{schema_version=1;unit=$plan.unit;token=$plan.token;input_hash=$plan.input_hash;disposition='terms';terms=@(@{term='外部融资溢价';avoid=@('融资差额');meaning='外部融资成本与内部融资成本的差额';section='sections/s01.md:3'})}
    $bad=$answer.Clone();$bad.terms=@(@{term='外部融资溢价';avoid=@();section='sections/s01.md:3'});Save-Json $plan.answer $bad
    Refused $args 'meaning' 'a malformed selected row cannot silently disappear during Terms normalization'
    $cross=$answer.Clone();$cross.terms=@($answer.terms[0],@{term='融资差额';avoid=@();meaning='两种融资成本的差额';section='sections/s01.md:3'});Save-Json $plan.answer $cross
    Refused $args 'another selected term' 'the Terms table cannot reject a different term that it simultaneously declares canonical'
    [IO.File]::WriteAllBytes($plan.answer,[byte[]]@(255,123,125))
    Refused $args 'strict UTF-8' 'claimed answer bytes must be valid UTF-8 rather than replacement-decoded prose'
    $json=$answer | ConvertTo-Json -Depth 20 -Compress
    [IO.File]::WriteAllText($plan.answer,$json.Replace('"terms":','"unit":"terms/book@1","terms":'),$utf8)
    Refused $args 'duplicate JSON key' 'ambiguous duplicate bindings are refused before Terms apply'
    $noTerms=$answer.Clone();$noTerms.disposition='no_terms';$noTerms.terms=@();Save-Json $plan.answer $noTerms
    Refused $args 'candidate' 'a no-terms assertion cannot hide enumerated source candidates'
    Save-Json $plan.answer $answer
    $env:ZT_CHECK_ZT=Join-Path $PSScriptRoot 'seam-zt-mcp.cmd';$env:ZT_FAIL_BATCH_BEFORE_WRITE='1'
    Refused $args 'injected failure before the archive write' 'a pre-write process fault retains an accepted Terms plan without committing a Card'
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_FAIL_BATCH_BEFORE_WRITE
    $acceptedPath=Join-Path $out 'terms/accepted-ledger.json';$acceptedBytes=[IO.File]::ReadAllBytes($acceptedPath)
    $accepted=Get-Content -LiteralPath $acceptedPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $accepted.calls=@(@{tool='create_topic';arguments=@{title=$accepted.input.title;body=$accepted.input.body}});Save-Json $acceptedPath $accepted
    Refused $args 'accepted Terms calls' 'a corrupted retained plan cannot redirect the term table into a Topic root'
    [IO.File]::WriteAllBytes($acceptedPath,$acceptedBytes)
    $before=(Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    $env:ZT_CHECK_ZT=Join-Path $PSScriptRoot 'seam-zt-mcp.cmd';$env:ZT_DROP_BATCH_REPLY_FLAG=Join-Path $scratch 'terms-reply-lost.flag'
    $lost=@(& $Digest @args 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and (Test-Path $env:ZT_DROP_BATCH_REPLY_FLAG) -and (Invoke-Zt @(@{tool='get_tree';arguments=@{tree='DefaultTerms'}})).cards.Count -eq 3) 'the Terms archive write can commit while its MCP reply is lost'
    Remove-Item Env:ZT_DROP_BATCH_REPLY_FLAG,Env:ZT_CHECK_ZT
    $open=Run-Digest @('ledger','status','-Out',$out,'-Layer','terms')
    Assert ($open.units[0].state -eq 'claimed' -and -not $open.complete) 'a historical archive receipt alone cannot mark the Terms producer done'
    foreach ($point in @('after_outputs','before_commit')) {
        $env:ZT_LEDGER_COMPLETE_FAULT=$point
        Refused $args 'injected completion fault' "a Terms $point failure leaves the completion transaction uncommitted"
    }
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    $inspect=@'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-terms-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);console.log(JSON.stringify({outputs:db.prepare("select count(*) n from outputs where unit_id='terms/book@1'").get().n,completed:db.prepare("select count(*) n from events where unit_id='terms/book@1' and event='completed'").get().n}));db.close();
'@
    $facts=& node --no-warnings -e $inspect (Join-Path $out 'ledger.sqlite3') | ConvertFrom-Json
    $stillWaiting=Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','group-manager')
    Assert ($facts.outputs -eq 0 -and $facts.completed -eq 0 -and $stillWaiting.units.Count -eq 0) 'Terms state, outputs and completion event roll back together while groups keep waiting'
    $done=Run-Digest @('ledger','done','-Out',$out,'-Unit','terms/book@1','-Token',"$($claim.token)",'-Outputs',(Join-Path $out 'terms/completion.json'))
    Assert ($done.state -eq 'done' -and $done.disposition -eq 'terms' -and $done.terms -eq 1 -and $done.receipt_keys.Count -eq 1 -and $done.self_check.ran) 'recovery verifies the actual Terms table, receipt and checker before completing the producer'
    Assert ((Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $before+1) 'receipt recovery never creates a duplicate Terms Card or revision'
    $card=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$done.card}}))
    Assert ($card.title.StartsWith('术语表') -and $card.parent.address -eq 'DefaultTerms/1' -and $card.body.Contains('| 外部融资溢价 | 融资差额 |')) 'the verified Terms Card is the actual Side successor and contains the rendered row'
    $next=Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','group-manager')
    $groupStatus=Run-Digest @('ledger','status','-Out',$out,'-Layer','digest')
    Assert ($next.units.Count -eq 1 -and $groupStatus.units[0].attempts -eq 1) 'a verified default Terms completion releases its group without dependency-wait attempts'
    $started=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($next.units[0].token)")
    $job=Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($job.artifacts.terms.sha256 -eq (Get-FileHash -LiteralPath (Join-Path $out 'terms.md') -Algorithm SHA256).Hash.ToLowerInvariant()) 'default group startup freezes the actual verified Terms output'
    $again=Run-Digest $args
    Assert ($again.reused -and $again.card -eq $done.card -and (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $before+1) 'the same completed Terms claim verifies read-only on repeat'
    $typed=Run-Digest @('ledger','done','-Out',$out,'-Unit','terms/book@1','-Token',"$($claim.token)",'-Outputs',(Join-Path $out 'terms/completion.json'))
    Assert ($typed.reused -and $typed.state -eq 'done') 'ledger done dispatches the same deterministic Terms verifier'
    Refused @('terms','-Step','apply','-Staging',$out,'-Token',"$($claim.token+1)",'-Apply') 'stale_claim' 'an obsolete Terms token cannot publish or complete work'
    $receiptTool=@'
const {DatabaseSync}=require('node:sqlite');const fs=require('node:fs');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-terms-test-'))throw new Error('not the isolated archive');
const db=new DatabaseSync(p);const key=process.argv[2],backup=process.argv[3];
if(process.argv[4]==='delete'){const row=db.prepare('select * from applied_ops where op_key=?').get(key);if(!row)throw new Error('missing fixture receipt');fs.writeFileSync(backup,JSON.stringify(row));db.prepare('delete from applied_ops where op_key=?').run(key);}
else{const r=JSON.parse(fs.readFileSync(backup,'utf8'));db.prepare('insert into applied_ops(op_key,request_hash,result_json,committed_at) values(?,?,?,?)').run(r.op_key,r.request_hash,r.result_json,r.committed_at);}
db.close();
'@
    $backup=Join-Path $scratch 'receipt-row.json';$archiveDb=Join-Path $scratch 'archive/zt.sqlite3'
    & node --no-warnings -e $receiptTool $archiveDb $done.receipt_keys[0] $backup delete
    if ($LASTEXITCODE) {throw 'isolated receipt removal failed'}
    Refused $args 'missing retained archive receipt' 'matching Card text cannot replace the archive commit authority after a receipt is lost'
    Assert ((Invoke-Zt @(@{tool='get_tree';arguments=@{tree='DefaultTerms'}})).cards.Count -eq 3 -and (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $before+1) 'receipt verification rolls back its simulation instead of creating a replacement Card'
    & node --no-warnings -e $receiptTool $archiveDb $done.receipt_keys[0] $backup restore
    if ($LASTEXITCODE) {throw 'isolated receipt restoration failed'}
    $emptyOut=Prepared 'NoTermsCandidates' '读者按章节顺序阅读本书。' 'math'
    $emptyClaim=(Run-Digest @('ledger','next','-Out',$emptyOut,'-Layer','terms','-Take','1','-Owner','terms-manager')).units[0]
    $emptyPlan=Run-Digest @('terms','-Step','plan','-Staging',$emptyOut,'-Token',"$($emptyClaim.token)",'-MaxTerms','5000','-MaxCandidates','20000')
    $emptyJob=Get-Content -LiteralPath $emptyPlan.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($emptyJob.inputs.parameters.max_terms -eq 5000 -and $emptyJob.inputs.parameters.max_candidates -eq 20000) 'claimed options preserve explicit positive budgets beyond defaults while native byte limits still apply'
    Assert ($emptyPlan.expected_jobs -eq 0 -and $emptyPlan.jobs.Count -eq 0 -and $emptyPlan.candidate_count -eq 0 -and $emptyPlan.no_op.reason -eq 'no_terms_candidates') 'zero Terms candidates are an explicit compiler enumeration rather than an empty agent answer'
    $emptyBefore=(Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    $env:ZT_LEDGER_COMPLETE_FAULT='after_commit'
    Refused @('terms','-Step','apply','-Staging',$emptyOut,'-Token',"$($emptyClaim.token)",'-Apply') 'injected completion fault' 'a lost post-commit no-terms reply leaves one verifiable completion'
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    $emptyDone=Run-Digest @('terms','-Step','apply','-Staging',$emptyOut,'-Token',"$($emptyClaim.token)",'-Apply')
    Assert ($emptyDone.state -eq 'done' -and $emptyDone.disposition -eq 'no_terms' -and $emptyDone.terms -eq 0 -and -not $emptyDone.card -and $emptyDone.receipt_keys.Count -eq 0 -and (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $emptyBefore) 'an enumerated no-terms result publishes checked context without inventing a Card write'
    $emptyNext=Run-Digest @('ledger','next','-Out',$emptyOut,'-Layer','digest','-Take','1','-Owner','group-manager')
    Assert ($emptyNext.units.Count -eq 1) 'the explicit no-terms completion also releases a normally enabled Terms prerequisite'
    $missing=@'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-terms-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.exec("delete from outputs where unit_id='terms/book@1' and output_key='terms'");db.close();
'@
    & node --no-warnings -e $missing (Join-Path $emptyOut 'ledger.sqlite3')
    if ($LASTEXITCODE) {throw 'isolated Terms output removal failed'}
    $broken=Run-Digest @('ledger','status','-Out',$emptyOut,'-Layer','terms')
    Assert (-not $broken.complete -and @($broken.coverage_faults | Where-Object {$_.unit -eq 'terms/book@1' -and $_.code -eq 'producer_outputs_mismatch' -and $_.missing -contains 'terms'}).Count -eq 1) 'offline Terms gates name a missing required context output instead of trusting done alone'
} finally {
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_DROP_BATCH_REPLY_FLAG,Env:ZT_LEDGER_COMPLETE_FAULT,Env:ZT_FAIL_BATCH_BEFORE_WRITE -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
