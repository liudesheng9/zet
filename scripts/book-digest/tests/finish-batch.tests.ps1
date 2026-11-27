#Requires -Version 7.5
# zt-digest finish-batch (docs/digest-claude-token-proposal.md P1/P2/P4): a block's checked finishes in one
# process. Each unit keeps its own verifier and claim token; a unit that cannot finish is recorded against that
# unit alone; the printed line is short and the full results go to <out>\finish\.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference='Stop'; $utf8=[Text.UTF8Encoding]::new($false)
$scratch=[IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "finish-batch-test-$PID"))
$tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {throw 'scratch escaped temp directory'}
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) {$env:PATH="$(Split-Path -Parent $Zt);$env:PATH"}
if (-not $Digest) {$Digest=(Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source}
$env:ZT_DIGEST=$Digest; $env:ZT_CONFIG_DIR=Join-Path $scratch 'config'; $env:ZT_GUI_PORT='0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition,$message) {if (-not $condition) {throw $message}; Write-Output "ok: $message"}
function Run-Digest([string[]]$Arguments) {
    $lines=@(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) {throw "digest failed: $($lines -join ' ')"}
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Save-Json($path,$value) {[IO.File]::WriteAllText($path,($value | ConvertTo-Json -Depth 30),$utf8)}
function Refused($arguments,$text,$message) {
    $lines=@(& $Digest @arguments 2>&1)
    Assert ($LASTEXITCODE -eq 2 -and ($lines -join ' ').Contains($text)) "$message [$($lines -join ' ')]"
}
# One prepared economics book whose single group writes its anchor and one substantive Card (as cards-finish.tests.ps1).
function New-PreparedBook([string]$key) {
    $out=Join-Path $scratch "prepared-$key"
    $source=Join-Path $scratch "book-$key.md"; $options=Join-Path $scratch "options-$key.json"
    [IO.File]::WriteAllText($source,"# 导论`n`n企业的融资约束影响生产选择。`n`n# 参考`n未选择。",$utf8)
    Save-Json $options @{terms=$false}
    $null=Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey',$key,'-BibTeX',"@book{$key,title={$key book}}",'-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    $claim=(Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','batch-manager')).units[0]
    $started=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claim.token)")
    $job=Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $body="融资成本影响企业的生产选择。贷款收紧使企业减少设备投入。依赖 [[$key]]，后者给出全书的问题范围。来源：[[$key/1]]"
    $draft=@{schema_version=1;unit=$claim.id;token=$claim.token;input_hash=$started.input_hash;disposition='cards';calls=@(
        @{tool='edit_card';arguments=@{at="$key/1";title='融资约束影响企业投资';body='融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。';base_fingerprint=$job.inputs.anchor.fingerprint}},
        @{tool='create_successor';arguments=@{at="$key/1";kind='side';title='贷款收紧限制企业投入';body=$body}}
    )}
    Save-Json $started.draft $draft
    $null=Run-Digest @('group-finish','-Out',$out,'-Group','1','-Token',"$($claim.token)",'-Draft',$started.draft)
    return $out
}
function State($out,$unit) {(Run-Digest @('ledger','status','-Out',$out,'-Units',$unit)).units[0]}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null

    # The arguments: each unit named once, of a layer the batch finishes (no arbitrary eight-unit cap);
    # the claim form takes its own layer, take or -All, and owner.
    Refused @('finish-batch','-Out','X','-Units','cards/g001@1:1,cards/g001@1:2') 'twice' 'a batch names each unit once'
    Refused @('finish-batch','-Out','X','-Units','digest/g001@1:1') 'finishes cards, events, findings, models, explain units only' 'a digest unit keeps its own batch finish'
    Refused @('finish-batch','-Out','X','-Units','cards/g001@1:1','-Layer','cards') 'takes no -Layer' 'a named batch claims nothing'
    Refused @('finish-batch','-Out','X','-Layer','link','-Owner','o') '-Layer must be one of' 'the claim form finishes only the batch layers'
    Refused @('finish-batch','-Out','X','-Layer','cards','-All','-Take','8','-Owner','o') 'mutually exclusive' '-All and -Take are mutually exclusive'
    Refused @('model-deps','-Out','X','-Units','cards/g001@1:1') 'Model units' 'model-deps -Units takes Model units only'

    # Batch argument validation for group-start/finish, link-start/finish, and ledger done/next/reclaim.
    Refused @('group-start','-Out','X','-Units','digest/g1@1:1') 'canonical' 'group-start rejects noncanonical group id'
    Refused @('group-start','-Out','X','-Units','digest/g001@1:1,digest/g001@1:2') 'twice' 'group-start rejects duplicate unit ids'
    Refused @('link-finish','-Out','X','-Units','digest/g001@1:1') 'canonical' 'link-finish rejects wrong layer'
    Refused @('ledger','done','-Out','X','-Units','termcards/book@1:1','-Outputs','r.json') 'job units only' 'ledger done batch rejects book unit'
    Refused @('ledger','done','-Out','X','-Units','termcards/t001@1:1,citedcards/t001@1:1','-Outputs','r.json') 'mix' 'ledger done batch rejects mixed families'
    Refused @('ledger','next','-Out','X','-Layer','termcards','-All','-Take','8','-Owner','o') 'mutually exclusive' 'ledger next rejects -All with -Take'
    Refused @('ledger','reclaim','-Out','X','-Layer','termcards','-All','-Take','8') 'mutually exclusive' 'ledger reclaim rejects -All with -Take'

    $out=New-PreparedBook 'BatchBook'
    # Batch finish accepts >8 units (9-unit test: 1 real unit + 8 unknown units settle cleanly in one call without an 8-unit cap).
    $nineList=@('cards/g001@1:1') + (2..9 | ForEach-Object {"cards/g00$($_)@1:1"})
    $nineRes=Run-Digest @('finish-batch','-Out',$out,'-Units',($nineList -join ','))
    Assert ($nineRes.kind -eq 'finish_batch' -and $nineRes.units -eq 9) "finish-batch accepts 9+ units in one command (got $($nineRes.units))"
    # The claim form with -All: one command drains every ready Cards unit and records its checked finish.
    $line=Run-Digest @('finish-batch','-Out',$out,'-Layer','cards','-All','-Owner','cards claim 1','-Lease','1500')
    Assert ($line.kind -eq 'finish_batch' -and $line.claimed -eq 1 -and $line.done -eq 1 -and $line.layer -eq 'cards' -and $line.settled -eq $true -and $line.counts.open -eq 0) "the -All claim form claims and finishes every ready Cards unit in one command with post-finish gate [$($line | ConvertTo-Json -Depth 4 -Compress)]"
    $entry=$line.results[0]
    Assert ($entry.unit -eq 'cards/g001@1' -and $entry.status -eq 'done' -and $entry.disposition -eq 'verified' -and $entry.token -gt 0) "the line names the unit, its token, its status and its disposition [$($entry | ConvertTo-Json -Compress)]"
    Assert ((State $out 'cards/g001@1').state -eq 'done') 'the ledger records the completion the batch reports'
    $raw=@(& $Digest finish-batch -Out $out -Units "cards/g001@1:$($entry.token)" 2>&1)[-1]
    Assert ($raw.Length -lt 600) "the printed line is short ($($raw.Length) characters)"
    $full=Get-Content -LiteralPath $line.file -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($full.units[0].result.kind -eq 'cards_completion' -and $full.units[0].result.files.completion.sha256) 'the full completion evidence is written to the batch file'

    # A repeated batch is idempotent: the done unit stays done, from its own reuse or from the ledger.
    $again=Run-Digest @('finish-batch','-Out',$out,'-Units',"cards/g001@1:$($entry.token)")
    Assert ($again.done -eq 1 -and $again.results[0].status -eq 'done') 'a repeated batch finish reuses the done unit'
    $stale=Run-Digest @('finish-batch','-Out',$out,'-Units',"cards/g001@1:$($entry.token + 7)")
    Assert ($stale.results[0].status -eq 'done' -and (State $out 'cards/g001@1').state -eq 'done') 'a stale token never faults a unit the ledger holds done'
    $empty=Run-Digest @('finish-batch','-Out',$out,'-Layer','cards','-Owner','cards claim 2')
    Assert ($empty.claimed -eq 0 -and @($empty.results).Count -eq 0) 'a claim that finds nothing ready finishes nothing'

    # Every other ready unit the group released: each line outcome matches the ledger, and a unit that could not
    # finish has its fault recorded against it by the batch itself.
    foreach ($layer in @('events','findings','models')) {
        $claimed=@((Run-Digest @('ledger','next','-Out',$out,'-Layer',$layer,'-Take','8','-Owner',"$layer claim 1")).units)
        if (-not $claimed.Count) { Write-Output "ok: no $layer unit is ready in this fixture"; continue }
        $units=($claimed | ForEach-Object {"$($_.id):$($_.token)"}) -join ','
        $batch=Run-Digest @('finish-batch','-Out',$out,'-Units',$units)
        foreach ($r in @($batch.results)) {
            $held=State $out $r.unit
            $agrees=switch ($r.status) { 'done' {$held.state -eq 'done'} 'partial' {$held.state -eq 'done'} 'failed' {$held.state -eq 'failed'} 'retry' {$held.state -in @('retry','ready')} default {$false} }
            Assert ($agrees -and ($r.status -in @('done','partial') -or $r.recorded)) "$($r.unit): the batch outcome $($r.status) is the ledger's ($($held.state)) and a fault is recorded"
        }
    }
    # End-to-end P0-1 + P0-2 batch test on a two-group book: reclaim -Layer -All, next -All, group-start -UnitsFile,
    # group-finish -UnitsFile, finish-batch -Layer cards -All, link-start -UnitsFile, link-finish -UnitsFile.
    $out2=Join-Path $scratch 'prepared-BatchMulti'
    $src2=Join-Path $scratch 'book-BatchMulti.md'; $opt2=Join-Path $scratch 'options-BatchMulti.json'
    [IO.File]::WriteAllText($src2,"# 第一章`n`n企业的融资约束限制资本投入。`n`n# 第二章`n`n银行信贷扩张缓解中小企业的融资摩擦。`n`n# 参考`n未选择。",$utf8)
    Save-Json $opt2 @{terms=$false}
    $null=Run-Digest @('prepare','-Source',$src2,'-Out',$out2,'-CitationKey','BatchMulti','-BibTeX','@book{BatchMulti,title={Batch multi book}}','-StartHeading','第一章','-EndHeading','参考','-MaxUnits','1','-MinUnitChars','1','-NoFigures','-Variant','econ','-RunOptions',$opt2,'-Apply')
    $claimedAll=Run-Digest @('ledger','next','-Out',$out2,'-Layer','digest','-All','-Owner','batch-multi')
    Assert (@($claimedAll.units).Count -eq 2) "ledger next -All claims both digest groups in one transaction (got $(@($claimedAll.units).Count))"
    $relAll=Run-Digest @('ledger','reclaim','-Out',$out2,'-Layer','digest','-All','-Release')
    Assert (@($relAll.reclaimed).Count -eq 0 -and @($relAll.skipped).Count -eq 2 -and $relAll.skipped[0].reason -eq 'claim_recent') 'ledger reclaim -Layer -All -Release respects the quiet window on fresh claims'
    $uFile=Join-Path $scratch 'multi-digest-units.json'
    Save-Json $uFile @($claimedAll.units)
    $gsBatch=Run-Digest @('group-start','-Out',$out2,'-UnitsFile',$uFile)
    Assert ($gsBatch.kind -eq 'group_start_batch' -and $gsBatch.ok -eq $true -and $gsBatch.started -eq 2 -and (Test-Path -LiteralPath $gsBatch.file)) "group-start -UnitsFile starts both groups in one process [$($gsBatch | ConvertTo-Json -Depth 4 -Compress)]"
    foreach ($r in @($gsBatch.results)) {
        $n=[int]$r.group
        $job=Get-Content -LiteralPath $r.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
        $body="第${n}组证据表明融资约束影响企业投入。贷款收紧使企业减少设备投入。依赖 [[BatchMulti]]，后者给出全书的问题范围。来源：[[BatchMulti/$n]]"
        $draft=@{schema_version=1;unit=$r.unit;token=$r.token;input_hash=$r.input_hash;disposition='cards';calls=@(
            @{tool='edit_card';arguments=@{at="BatchMulti/$n";title="第${n}组融资约束分析";body="融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。";base_fingerprint=$job.inputs.anchor.fingerprint}},
            @{tool='create_successor';arguments=@{at="BatchMulti/$n";kind='side';title="第${n}组贷款收紧限制投入";body=$body}}
        )}
        Save-Json $r.draft $draft
    }
    $gfBatch=Run-Digest @('group-finish','-Out',$out2,'-UnitsFile',$uFile)
    Assert ($gfBatch.kind -eq 'digest_finish_batch' -and $gfBatch.ok -eq $true -and $gfBatch.done -eq 2 -and $gfBatch.failed -eq 0) "group-finish -UnitsFile finishes both groups in one process [$($gfBatch | ConvertTo-Json -Depth 4 -Compress)]"
    $cardsAll=Run-Digest @('finish-batch','-Out',$out2,'-Layer','cards','-All','-Owner','cards-multi','-Lease','1500')
    Assert ($cardsAll.kind -eq 'finish_batch' -and $cardsAll.claimed -eq 2 -and $cardsAll.done -eq 2 -and $cardsAll.settled -eq $true -and $cardsAll.counts.open -eq 0) "finish-batch -Layer cards -All drains both groups in one command"
    $null=Run-Digest @('prepare-link','-Staging',$out2,'-Brief')
    $linkAll=Run-Digest @('ledger','next','-Out',$out2,'-Layer','link','-All','-Owner','link-multi')
    Assert (@($linkAll.units).Count -eq 2) "ledger next -Layer link -All claims both link groups"
    $luFile=Join-Path $scratch 'multi-link-units.json'
    Save-Json $luFile @($linkAll.units)
    $lsBatch=Run-Digest @('link-start','-Out',$out2,'-UnitsFile',$luFile)
    Assert ($lsBatch.kind -eq 'link_start_batch' -and $lsBatch.ok -eq $true -and $lsBatch.started -eq 2) "link-start -UnitsFile starts both link groups in one process [$($lsBatch | ConvertTo-Json -Depth 4 -Compress)]"
    foreach ($r in @($lsBatch.results)) {
        if (-not (Test-Path -LiteralPath $r.draft)) {
            $sj=Get-Content -LiteralPath $r.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
            $disps=@($sj.gap | ForEach-Object { @{at=$_.at;outcome='no_link';reason='测试样例无跨组连接。'} })
            Save-Json $r.draft @{schema_version=1;unit=$r.unit;token=$r.token;input_hash=$r.input_hash;calls=@();dispositions=$disps}
        }
    }
    $lfBatch=Run-Digest @('link-finish','-Out',$out2,'-UnitsFile',$luFile)
    Assert ($lfBatch.kind -eq 'link_finish_batch' -and $lfBatch.ok -eq $true -and $lfBatch.done -eq 2 -and $lfBatch.failed -eq 0) "link-finish -UnitsFile finishes both link groups in one process [$($lfBatch | ConvertTo-Json -Depth 4 -Compress)]"
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
