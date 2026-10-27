#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference='Stop'; $utf8=[Text.UTF8Encoding]::new($false)
$scratch=[IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-cards-test-$PID"))
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
    Assert ($LASTEXITCODE -ne 0 -and ($lines -join ' ').Contains($text)) "$message [$($lines -join ' ')]"
}
function Revision { (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision }
# One prepared economics book whose single group writes its anchor and one substantive
# Card. The Card must carry a kinded Link to a Card other than its parent so the group
# itself has no quality fault to defer (I4).
function New-PreparedBook([string]$key,[string]$createdBody,[string]$createdTitle='贷款收紧限制企业投入') {
    $out=Join-Path $scratch "prepared-$key"
    $source=Join-Path $scratch "book-$key.md"; $options=Join-Path $scratch "options-$key.json"
    [IO.File]::WriteAllText($source,"# 导论`n`n企业的融资约束影响生产选择。`n`n# 参考`n未选择。",$utf8)
    Save-Json $options @{terms=$false}
    $null=Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey',$key,'-BibTeX',"@book{$key,title={$key book}}",'-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    $claim=(Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','cards-manager')).units[0]
    $started=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claim.token)")
    $job=Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $draft=@{schema_version=1;unit=$claim.id;token=$claim.token;input_hash=$started.input_hash;disposition='cards';calls=@(
        @{tool='edit_card';arguments=@{at="$key/1";title='融资约束影响企业投资';body='融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。';base_fingerprint=$job.inputs.anchor.fingerprint}},
        @{tool='create_successor';arguments=@{at="$key/1";kind='side';title=$createdTitle;body=$createdBody}}
    )}
    Save-Json $started.draft $draft
    $group=Run-Digest @('group-finish','-Out',$out,'-Group','1','-Token',"$($claim.token)",'-Draft',$started.draft)
    return [pscustomobject]@{out=$out;key=$key;group=$group}
}
$cleanBody='融资成本影响企业的生产选择。贷款收紧使企业减少设备投入。依赖 [[CardsBook]]，后者给出全书的问题范围。来源：[[CardsBook/1]]'
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $book=New-PreparedBook 'CardsBook' $cleanBody
    $out=$book.out; $group=$book.group
    $owned=@($group.owned)
    Assert ($group.disposition -eq 'verified' -and $group.self_check.count -eq 0 -and $owned.Count -eq 2) 'the group completes its anchor and substantive Card with no deferred quality fault'
    $claim=(Run-Digest @('ledger','next','-Out',$out,'-Layer','cards','-Take','1','-Owner','cards-manager')).units[0]
    Assert ($claim.id -eq 'cards/g001@1' -and $claim.inputs.producer -eq 'digest/g001@1' -and $claim.inputs.owned.path -like '*owned-*' -and $claim.input_hash) 'the Cards claim freezes the owned enumeration the group registered'
    $modelsGate=Run-Digest @('ledger','gate','-Out',$out,'-Layer','models')
    Assert ($modelsGate.sealed -and $modelsGate.settled -and $modelsGate.counts.total -eq 0) 'a child layer with no items settles only after its producer enumeration closes'
    $finishArgs=@('cards-finish','-Out',$out,'-Group','1','-Token',"$($claim.token)")
    $checkArgs=$finishArgs+@('-CheckOnly')
    $revision=Revision
    $checked=Run-Digest $checkArgs
    Assert ($checked.check_only -and -not $checked.applied -and $checked.complete -and $checked.disposition -eq 'verified') 'a supplied offline check runs the real SELF-CHECK without writing'
    Assert (@($checked.owned).Count -eq 2 -and $checked.self_check.ran -and $checked.self_check.count -eq 0 -and $checked.archive_revision -eq $revision) 'the preview reports the checked owned set and revision'
    Assert (-not (Test-Path (Join-Path $out 'cards-check-g001.json')) -and -not (Test-Path (Join-Path $out 'cards-completion-g001.json'))) 'the offline check preview publishes no evidence'
    $open=Run-Digest @('ledger','status','-Out',$out,'-Layer','cards')
    Assert ($open.units[0].state -eq 'claimed' -and (Revision) -eq $revision) 'the offline check preview changes no unit state and no Card'
    $done=Run-Digest $finishArgs
    Assert ($done.schema_version -eq 1 -and $done.kind -eq 'cards_completion' -and $done.unit -eq 'cards/g001@1' -and $done.state -eq 'done' -and $done.complete -and -not $done.reused) 'the offline check completes the Cards/group unit with typed evidence'
    Assert ($done.disposition -eq 'verified' -and @($done.deferred).Count -eq 0) 'a group whose owned Cards pass the offline check completes as verified'
    Assert ($done.self_check.ran -and $done.self_check.count -eq 0 -and $done.self_check.reported_count -eq 0 -and $done.self_check.rules -eq 'R2,R3,R4,R5,R7,R9,R12,R18,R19,R20,R21,R22,I1,I2,I3,I4,I7,L1,L2,E1,E2') 'the SELF-CHECK runs the whole-book Card rules over the exact owned set'
    Assert (@($done.owned).Count -eq 2 -and @($done.owned | Where-Object {$_ -in $owned}).Count -eq 2) 'the completion enumerates exactly the group''s owned Cards'
    Assert (@($done.card_fingerprints.Keys).Count -eq 2 -and @($done.card_fingerprints.Keys | Where-Object {$_ -in $owned}).Count -eq 2) 'completion evidence records the checked authoring fingerprint of every owned Card'
    Assert ($done.archive_revision -eq $revision) 'the offline check reads one consistent archive revision'
    $check=Get-Content -LiteralPath (Join-Path $out 'cards-check-g001.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($check.unit -eq 'cards/g001@1' -and @($check.owned).Count -eq 2 -and $check.self_check.count -eq 0) 'the retained check evidence describes the checked owned set'
    Assert ((Test-Path (Join-Path $out 'cards-completion-g001.json')) -and $done.files.completion.sha256) 'the completion evidence is retained and hash-bound'
    $view=Run-Digest @('ledger','status','-Out',$out,'-Layer','cards')
    Assert ($view.units[0].state -eq 'done' -and $view.units[0].result.complete -and $view.units[0].result.disposition -eq 'verified') 'the ledger records the completion the evidence describes'
    $gate=Run-Digest @('ledger','gate','-Out',$out,'-Layer','cards')
    Assert ($gate.sealed -and $gate.settled -and -not $gate.complete -and $gate.counts.done -eq 1 -and @($gate.unresolved).Count -eq 0) 'the Cards layer gate settles on its done unit without claiming the whole run complete'
    Assert (@($gate.coverage_faults | Where-Object {$_.unit -eq 'cards/g001@1'}).Count -eq 0) 'the completed Cards unit leaves no coverage fault'
    $again=Run-Digest $finishArgs
    Assert ($again.reused -and $again.state -eq 'done' -and (Revision) -eq $revision) 'a repeated completion reuses the checked result without writing'
    $dispatch=Run-Digest @('ledger','done','-Out',$out,'-Unit','cards/g001@1','-Token',"$($claim.token)",'-Outputs',(Join-Path $out 'cards-completion-g001.json'))
    Assert ($dispatch.reused -and $dispatch.complete -and $dispatch.unit -eq 'cards/g001@1') 'ledger done dispatches the same deterministic Cards verifier'
    Refused @('ledger','done','-Out',$out,'-Unit','cards/g001@1','-Token',"$($claim.token)",'-Outputs',(Join-Path $out 'completion-g001.json')) 'completion evidence' 'ledger done refuses another unit''s evidence as Cards completion'

    # Guard: an owned Card that changed after the check makes the retained completion
    # stale rather than silently describing different Cards.
    $created=$owned[1]
    $createdBody=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$created}})).body
    Invoke-Zt @(@{tool='edit_card';arguments=@{at=$created;body="$createdBody`n`n后来的一条独立补充。"}}) | Out-Null
    Refused $finishArgs 'owned Cards changed' 'a later edit of an owned Card makes the completed offline check stale'
    Invoke-Zt @(@{tool='edit_card';arguments=@{at=$created;body=$createdBody}}) | Out-Null
    Assert ((Run-Digest $finishArgs).reused) 'restoring the checked Card recovers the same verified completion'
    Refused @('cards-finish','-Out',$out,'-Group','1','-Token',"$($claim.token+1)") 'stale_claim' 'a wrong claim token cannot complete the Cards unit'
    Refused @('cards-finish','-Out',$out,'-Group','9','-Token','1') 'unknown unit' 'an unregistered group has no Cards obligation'
    $ownedPath=$view.units[0].inputs.owned.path
    $ownedBytes=[IO.File]::ReadAllBytes($ownedPath)
    [IO.File]::AppendAllText($ownedPath,"CardsBook/999`n",$utf8)
    Refused $finishArgs 'owned enumeration changed' 'a changed owned enumeration cannot be checked as the group scope'
    [IO.File]::WriteAllBytes($ownedPath,$ownedBytes)
    Assert ((Run-Digest $finishArgs).reused) 'the restored owned enumeration recovers the same completion'

    # A group whose owned Cards carry a real quality fault (an open formula, R22, which keeps its
    # level on a Literature note since #184) hands that work to the book
    # Card cycle instead of claiming a clean result.
    $faultyBody='融资成本 $r 影响企业的生产选择，因为贷款收紧使企业减少设备投入。依赖 [[CardsFault]]，后者给出全书的问题范围。来源：[[CardsFault/1]]'
    $faultBook=New-PreparedBook 'CardsFault' $faultyBody '贷款收紧'
    $faultOut=$faultBook.out; $faultGroup=$faultBook.group
    Assert ($faultGroup.disposition -eq 'deferred' -and $faultGroup.self_check.count -eq 1) 'the group completes with its own fault deferred to the Cards obligation'
    $faultClaim=(Run-Digest @('ledger','next','-Out',$faultOut,'-Layer','cards','-Take','1','-Owner','cards-manager')).units[0]
    $faultArgs=@('cards-finish','-Out',$faultOut,'-Group','1','-Token',"$($faultClaim.token)")
    $faultCheckArgs=$faultArgs+@('-CheckOnly')
    $faultPreview=Run-Digest $faultCheckArgs
    Assert ($faultPreview.disposition -eq 'deferred' -and @($faultPreview.deferred).Count -eq 1 -and $faultPreview.deferred[0].consumer -eq 'cards-book/book@1') 'every hard fault of the offline check names the registered book Card cycle consumer'
    Assert ($faultPreview.deferred[0].fault.StartsWith("$($faultGroup.substantive[0]) R22 ") -and $faultPreview.self_check.count -eq 1) 'the deferred fault is the real reported line of an owned Card'
    function Set-UnitState([string]$Database,[string]$Unit,[string]$State) {
        $seed=@'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');
const p=path.resolve(process.argv[1]);const unit=process.argv[2];const state=process.argv[3];
const db=new DatabaseSync(p);const info=db.prepare('update units set state=? where id=?').run(state,unit);db.close();
'@
        & node --no-warnings -e $seed $Database $Unit $State
        if ($LASTEXITCODE) {throw 'isolated ledger state fixture failed'}
    }
    $faultLedger=Join-Path $faultOut 'ledger.sqlite3'
    Set-UnitState $faultLedger 'cards-book/book@1' 'failed'
    Refused $faultCheckArgs 'cannot repair' 'a terminal consumer cannot be registered as the repair path'
    Set-UnitState $faultLedger 'cards-book/book@1' 'registered'
    Set-UnitState $faultLedger 'digest/g001@1' 'failed'
    Refused $faultCheckArgs 'not a checked completion' 'a Cards obligation whose group producer failed is not a checked scope'
    Set-UnitState $faultLedger 'digest/g001@1' 'done'
    $faultCreated=$faultGroup.substantive[0]
    $faultCard=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$faultCreated}}))
    Invoke-Zt @(@{tool='edit_card';arguments=@{at=$faultCreated;title='术语表';body=$faultCard.body}}) | Out-Null
    Refused $faultCheckArgs 'no writable Card consumer' 'a fault the book Card cycle cannot repair cannot be deferred'
    Invoke-Zt @(@{tool='edit_card';arguments=@{at=$faultCreated;title=$faultCard.title;body=$faultCard.body}}) | Out-Null

    # A lost ledger commit leaves the retained evidence and the archive unchanged; the
    # retry commits the same checked completion exactly once.
    $env:ZT_LEDGER_COMPLETE_FAULT='after_outputs'
    Refused $faultArgs 'injected completion fault' 'an injected fault rolls the ledger completion back after its evidence is written'
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    Assert ((Run-Digest @('ledger','status','-Out',$faultOut,'-Layer','cards')).units[0].state -eq 'claimed') 'the injected fault leaves the Cards unit claimed'
    $faultDone=Run-Digest $faultArgs
    Assert ($faultDone.state -eq 'done' -and $faultDone.complete -and $faultDone.disposition -eq 'deferred' -and @($faultDone.deferred).Count -eq 1) 'the offline check completes as deferred with its registered consumer'
    $faultCheck=Get-Content -LiteralPath (Join-Path $faultOut 'cards-check-g001.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert (@($faultCheck.deferred).Count -eq 1 -and $faultCheck.self_check.count -eq 1) 'the retained evidence keeps the exact deferred fault'
    $faultLines=@($faultDone.self_check.lines); $faultHard=@($faultDone.self_check.faults); $faultAdvisories=@($faultDone.self_check.advisories)
    Assert ($faultLines.Count -eq $faultHard.Count + $faultAdvisories.Count -and $faultDone.self_check.reported_count -eq $faultHard.Count -and $faultAdvisories.Count -ge 1 -and @($faultAdvisories | Where-Object {$_ -notmatch '\(advisory\)'}).Count -eq 0) "an advisory is reported separately and never counted as a hard fault [$($faultLines.Count)/$($faultHard.Count)/$($faultAdvisories.Count)]"

    # Work emitted: the book Card cycle consumes the checked result as its next
    # publication revision instead of leaving an older revision current.
    $published=Run-Digest @('ledger','publish','-Out',$faultOut,'-Layer','cards-book')
    Assert (@($published.contributions | Where-Object {$_.unit -eq 'cards/g001@1'}).Count -eq 1 -and $published.revision -eq 2) 'the book Card-cycle revision consumes the completed Cards unit'
    Assert (@($published.superseded | Where-Object {$_ -eq 'cards-book/book@1'}).Count -eq 1) 'the older book Card-cycle revision is superseded rather than silently reused'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
