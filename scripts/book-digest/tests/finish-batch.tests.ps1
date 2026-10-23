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

    # The arguments: at most eight units, each named once, of a layer the batch finishes; the claim form takes
    # its own layer, take and owner.
    $nine=(1..9 | ForEach-Object {"cards/g00$($_)@1:1"}) -join ','
    Refused @('finish-batch','-Out','X','-Units',$nine) 'at most 8' 'a batch holds at most eight units'
    Refused @('finish-batch','-Out','X','-Units','cards/g001@1:1,cards/g001@1:2') 'twice' 'a batch names each unit once'
    Refused @('finish-batch','-Out','X','-Units','digest/g001@1:1') 'finishes cards, events, findings, models, explain units only' 'a digest unit keeps its block manager''s finish'
    Refused @('finish-batch','-Out','X','-Units','cards/g001@1:1','-Layer','cards') 'takes no -Layer' 'a named batch claims nothing'
    Refused @('finish-batch','-Out','X','-Layer','link','-Owner','o') '-Layer must be one of' 'the claim form finishes only the batch layers'
    Refused @('model-deps','-Out','X','-Units','cards/g001@1:1') 'Model units' 'model-deps -Units takes Model units only'

    $out=New-PreparedBook 'BatchBook'
    # The claim form: one command claims the ready Cards unit and records its checked finish.
    $line=Run-Digest @('finish-batch','-Out',$out,'-Layer','cards','-Take','8','-Owner','cards claim 1','-Lease','1500')
    Assert ($line.kind -eq 'finish_batch' -and $line.claimed -eq 1 -and $line.done -eq 1 -and $line.layer -eq 'cards') 'the claim form claims the ready Cards unit and finishes it in one command'
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
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
