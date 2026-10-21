#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference='Stop'; $utf8=[Text.UTF8Encoding]::new($false)
$scratch=[IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-link-test-$PID"))
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
function Revision { (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision }
function BodyOf($at) { (Invoke-Zt @(@{tool='get_card';arguments=@{at=$at}})).body }
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $source=Join-Path $scratch 'book.md'; $options=Join-Path $scratch 'options.json'
    [IO.File]::WriteAllText($source,"# 导论`n`n企业的融资约束影响生产选择。`n`n# 参考`n未选择。",$utf8)
    Save-Json $options @{terms=$false}
    $null=Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','LinkBook','-BibTeX','@book{LinkBook,title={Link book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    $claim=(Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','link-manager')).units[0]
    $started=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claim.token)")
    $job=Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $body='融资成本影响企业的生产选择。贷款收紧使企业减少设备投入。投入减少限制下一期的生产能力。来源：[[LinkBook/1]]'
    $groupDraft=@{schema_version=1;unit=$claim.id;token=$claim.token;input_hash=$started.input_hash;disposition='cards';calls=@(
        @{tool='edit_card';arguments=@{at='LinkBook/1';title='融资约束影响企业投资';body='融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。';base_fingerprint=$job.inputs.anchor.fingerprint}},
        @{tool='create_successor';arguments=@{at='LinkBook/1';kind='side';title='贷款收紧限制企业投入';body=$body}}
    )}
    Save-Json $started.draft $groupDraft
    $group=Run-Digest @('group-finish','-Out',$out,'-Group','1','-Token',"$($claim.token)",'-Draft',$started.draft)
    # A semantic Link needs a target outside the group; the anchor and the book are source places (#110).
    $null=Invoke-Zt @(@{tool='create_topic';arguments=@{title='融资约束';body='一个已有主题。'}},@{tool='create_successor';arguments=@{at='0/0';title='融资成本影响投资';body='融资成本影响企业的投资选择。'}})
    $owned=@($group.owned)
    $created=$group.substantive[0]
    Assert ($owned.Count -eq 2 -and $owned -contains 'LinkBook/1' -and $owned -contains $created) 'the group completion enumerates its anchor and substantive owned Card'
    $unready=(Run-Digest @('ledger','next','-Out',$out,'-Layer','link','-Take','1','-Owner','link-manager')).units
    Assert (@($unready).Count -eq 0) 'the Link unit waits for the Cards/group obligation the group emitted'
    $cardsClaim=(Run-Digest @('ledger','next','-Out',$out,'-Layer','cards','-Take','1','-Owner','link-manager')).units[0]
    $cardsDone=Run-Digest @('cards-finish','-Out',$out,'-Group','1','-Token',"$($cardsClaim.token)")
    Assert ($cardsDone.state -eq 'done' -and $cardsDone.complete -and $cardsDone.disposition -eq 'deferred' -and $cardsDone.deferred[0].consumer -eq 'cards-book/book@1') 'the Cards/group obligation completes through its own offline check'
    $linkClaim=(Run-Digest @('ledger','next','-Out',$out,'-Layer','link','-Take','1','-Owner','link-manager')).units[0]
    Assert ($linkClaim.id -eq 'link/g001@1' -and $linkClaim.inputs.owned.path -like '*owned-*' -and $linkClaim.input_hash) 'the claim freezes the exact owned enumeration of the Link unit'
    $paragraph='依赖 [[0/1]]，后者给出这家企业的融资约束。'
    $linkDraft=@{schema_version=1;unit='link/g001@1';token=$linkClaim.token;input_hash=$linkClaim.input_hash;calls=@(
        @{tool='edit_card';arguments=@{at=$created;base_receipt=$group.receipt_keys[1];owned_paragraph=@{text=$paragraph}}}
    );dispositions=@(@{at='LinkBook/1';outcome='no_link';reason='锚点只承接本组的写作分支，它的 Link 由全书卡片周期决定。'})}
    $draftPath=Join-Path $out 'link-g001-draft.json'
    Save-Json $draftPath $linkDraft
    $finishArgs=@('link-finish','-Out',$out,'-Group','1','-Token',"$($linkClaim.token)",'-Draft',$draftPath)
    $checkArgs=$finishArgs+@('-CheckOnly')
    $before=Revision
    $checked=Run-Digest $checkArgs
    Assert ($checked.check_only -and -not $checked.applied -and $checked.complete -and $checked.owned.Count -eq 2 -and $checked.outcomes.Count -eq 2 -and $checked.self_check.ran -and $checked.self_check.count -eq 0) 'a complete supplied Link draft is checked against every owned Card and the Link rules'
    Assert ((Revision) -eq $before -and (BodyOf $created) -eq $body) 'the draft check writes no Card and publishes no revision'
    $open=Run-Digest @('ledger','status','-Out',$out,'-Layer','link')
    Assert ($open.units[0].state -eq 'claimed' -and -not (Test-Path (Join-Path $out 'link-accepted-g001.json'))) 'the draft check creates no accepted plan or completion transition'

    # Defensive guards: a silent omission, an invented Card, a duplicate outcome, a
    # raw edit and an unkinded or unreasonable paragraph are all refused.
    $savedDraft=[IO.File]::ReadAllText($draftPath,$utf8)
    function Copy-Draft { $savedDraft | ConvertFrom-Json -AsHashtable }
    $omitted=Copy-Draft; $omitted.dispositions=@()
    Save-Json $draftPath $omitted
    Refused $checkArgs 'no outcome for the gap Card' 'a gap Card without a Link or a checked disposition cannot complete the unit'
    $invented=Copy-Draft; $invented.dispositions=@($invented.dispositions)+@(@{at='LinkBook/999';outcome='no_link';reason='编造的卡片。'})
    Save-Json $draftPath $invented
    Refused $checkArgs 'not an owned Card of this Link unit' 'a disposition cannot invent a Card outside the frozen owned enumeration'
    $doubled=Copy-Draft; $doubled.dispositions=@($doubled.dispositions)+@(@{at=$created;outcome='already_linked';reason='同一张卡片有两个结果。'})
    Save-Json $draftPath $doubled
    Refused $checkArgs 'both an owned paragraph and a disposition' 'one owned Card carries exactly one outcome'
    $unkinded=Copy-Draft; $unkinded.calls=@(@{tool='edit_card';arguments=@{at=$created;base_receipt=$group.receipt_keys[1];owned_paragraph=@{text='参见 [[LinkBook/1]]。'}}})
    Save-Json $draftPath $unkinded
    Refused $checkArgs 'no kinded Link' 'an owned paragraph whose Link reads as a plain reference is refused'
    $noreason=Copy-Draft; $noreason.calls=@(@{tool='edit_card';arguments=@{at=$created;base_receipt=$group.receipt_keys[1];owned_paragraph=@{text='依赖 [[0/1]]。'}}})
    Save-Json $draftPath $noreason
    Refused $checkArgs 'L2' 'the Link SELF-CHECK refuses a Link with no reason'
    $falseClaim=Copy-Draft; $falseClaim.calls=@(); $falseClaim.dispositions=@(
        @{at='LinkBook/1';outcome='already_linked';reason='锚点已有 Link。'},
        @{at=$created;outcome='no_link';reason='本卡暂时没有目标。'})
    Save-Json $draftPath $falseClaim
    Refused $checkArgs 'holds no linked' 'a Card claimed as already_linked must really carry its required Link'
    $rawEdit=Copy-Draft; $rawEdit.calls=@(@{tool='edit_card';arguments=@{at=$created;body='直接改正文。';base_receipt=$group.receipt_keys[1]}})
    Save-Json $draftPath $rawEdit
    Refused $checkArgs 'owned paragraph' 'a Link revision cannot rewrite a Card body outside its owned paragraph'
    $noBase=Copy-Draft; $noBase.calls=@(@{tool='edit_card';arguments=@{at=$created;owned_paragraph=@{text=$paragraph}}})
    Save-Json $draftPath $noBase
    Refused $checkArgs 'authored base' 'an owned paragraph needs an authored base'
    $unknown=Copy-Draft; $unknown.extra='x'
    Save-Json $draftPath $unknown
    Refused $checkArgs 'unknown Link draft field' 'an unknown draft field is refused instead of ignored'
    $wrongHash=Copy-Draft; $wrongHash.input_hash=('0'*64)
    Save-Json $draftPath $wrongHash
    Refused $checkArgs 'binding' 'a draft for another unit input cannot pass by supplying valid calls'
    $badToken=Copy-Draft; $badToken.token=$badToken.token+1
    Save-Json $draftPath $badToken
    Refused $checkArgs 'binding' 'a draft for another claim token is refused'
    [IO.File]::WriteAllText($draftPath,$savedDraft,$utf8)
    Assert ((BodyOf $created) -eq $body -and (Revision) -eq $before) 'every refused draft leaves the archive unchanged'
    $anchorBody=BodyOf 'LinkBook/1'
    Invoke-Zt @(@{tool='edit_card';arguments=@{at='LinkBook/1';body="$anchorBody`n`n参见 [[$created]]，本组的写作分支从这里开始。"}}) | Out-Null
    Refused $checkArgs 'L1' 'an unkinded Link in an owned Card the draft does not write is the Link unit''s fault too'
    Invoke-Zt @(@{tool='edit_card';arguments=@{at='LinkBook/1';body=$anchorBody}}) | Out-Null
    $before=Revision

    # A lost ledger commit is recovered from the retained accepted plan and the
    # archive receipts: the paragraph is already committed and must not duplicate.
    $env:ZT_LEDGER_COMPLETE_FAULT='after_outputs'
    Refused $finishArgs 'injected completion fault' 'an injected fault rolls the ledger completion back after the archive write'
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    Assert ((Run-Digest @('ledger','status','-Out',$out,'-Layer','link')).units[0].state -eq 'claimed') 'the injected fault leaves the Link unit claimed'
    $changedDraft=Copy-Draft
    $changedDraft.calls[0].arguments.owned_paragraph.text='依赖 [[0/1]]，后者限定这家企业可选的融资方式。'
    Save-Json $draftPath $changedDraft
    Refused $finishArgs 'accepted_plan_changed' 'a different draft cannot replace the accepted plan after its first write'
    [IO.File]::WriteAllText($draftPath,$savedDraft,$utf8)
    $done=Run-Digest $finishArgs
    Assert ($done.state -eq 'done' -and $done.complete -and $done.applied -and $done.receipt_keys.Count -eq 1 -and @($done.changed).Count -eq 1 -and $done.changed[0] -eq $created) 'the accepted Link draft applies through the identified receipts and records its changed Cards'
    Assert ((BodyOf $created) -eq "$body`n`n$paragraph") 'the persisted Card gains exactly the receipt-owned paragraph'
    Assert ((Revision) -eq $before+1) 'one Link paragraph publishes exactly one archive revision'
    $view=Run-Digest @('ledger','status','-Out',$out,'-Layer','link')
    Assert ($view.units[0].state -eq 'done' -and $view.units[0].result.complete) 'the Link unit records its completion in the ledger'
    Assert (@($done.outcomes | Where-Object {$_.outcome -eq 'linked'}).Count -eq 1 -and @($done.outcomes | Where-Object {$_.outcome -eq 'no_link'}).Count -eq 1 -and @($done.outcomes.at | Where-Object {$_ -in $owned}).Count -eq 2) 'completion evidence enumerates every owned Card with its checked outcome'
    Assert ((Test-Path (Join-Path $out 'link-coverage-g001.json')) -and (Test-Path (Join-Path $out 'link-self-check-g001.json')) -and (Test-Path (Join-Path $out 'link-accepted-g001.json'))) 'the coverage, SELF-CHECK and accepted plan are retained as completion evidence'
    $again=Run-Digest $finishArgs
    Assert ($again.reused -and $again.state -eq 'done' -and (Revision) -eq $before+1) 'a lost final reply can recover the same checked Link completion'
    $recheck=Run-Digest $checkArgs
    Assert ($recheck.reused -and $recheck.check_only -and -not $recheck.applied -and $recheck.complete) 'a done Link unit reports its checked completion without writing'
    $ownedPath=$view.units[0].inputs.owned.path
    $ownedBytes=[IO.File]::ReadAllBytes($ownedPath)
    [IO.File]::AppendAllText($ownedPath,"LinkBook/999`n",$utf8)
    Refused $finishArgs 'owned enumeration changed' 'completion replay refuses a changed owned enumeration'
    [IO.File]::WriteAllBytes($ownedPath,$ownedBytes)
    Assert ((Run-Digest $finishArgs).reused) 'the restored owned enumeration recovers the same completion'

    # Work emitted: a changed publication input manifest derives the Link-check
    # revision that consumes this unit's checked result.
    $published=Run-Digest @('ledger','publish','-Out',$out,'-Layer','link-check')
    Assert (@($published.contributions | Where-Object {$_.unit -eq 'link/g001@1'}).Count -eq 1 -and $published.revision -eq 2) 'the Link-check revision consumes the completed Link unit'
    Assert (@($published.superseded | Where-Object {$_ -eq 'link-check/book@1'}).Count -eq 1) 'the older Link-check revision is superseded rather than silently reused'
    Invoke-Zt @(@{tool='edit_card';arguments=@{at=$created;body=$body}}) | Out-Null
    Refused $finishArgs 'archive receipt verification failed' 'a later edit of a changed Link Card makes the completion stale'
    Assert ((Run-Digest @('ledger','status','-Out',$out,'-Layer','link')).units[0].state -eq 'done') 'the stale evidence does not rewrite the recorded unit state'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'

