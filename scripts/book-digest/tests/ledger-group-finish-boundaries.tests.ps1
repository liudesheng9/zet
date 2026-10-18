#Requires -Version 7.5
param([string]$Zt='', [string]$Digest=$env:ZT_DIGEST)
$ErrorActionPreference='Stop';$utf8=[Text.UTF8Encoding]::new($false)
$scratch=[IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-group-finish-test-boundaries-$PID"))
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
function Prepared($name,$body,$extra=@{}) {
    $source=Join-Path $scratch "$name.md";$options=Join-Path $scratch "$name-options.json";$out=Join-Path $scratch $name
    [IO.File]::WriteAllText($source,"# 导论`n`n$body`n`n# 参考`n未选择。",$utf8)
    $switches=@{terms=$false};foreach($key in $extra.Keys){$switches[$key]=$extra[$key]};Save-Json $options $switches
    $manifest=Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey',$name,'-BibTeX',"@book{$name,title={$name}}",'-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    $claim=(Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','boundary-manager')).units[0]
    $start=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claim.token)")
    $job=Get-Content -LiteralPath $start.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $draft=@{schema_version=1;unit=$claim.id;token=$claim.token;input_hash=$start.input_hash;disposition='cards';calls=@()}
    return @{out=$out;start=$start;claim=$claim;job=$job;draft=$draft;args=@('group-finish','-Out',$out,'-Group','1','-Token',"$($claim.token)",'-Draft',$start.draft)}
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null;zt up | Out-Null
    $p=Prepared 'ModelGroup' '企业的融资约束影响生产选择。'
    $anchor='ModelGroup/1'
    $p.draft.calls=@(
        @{tool='edit_card';arguments=@{at=$anchor;title='融资约束影响企业投资';body='融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。';base_fingerprint=$p.job.inputs.anchor.fingerprint}},
        @{tool='create_topic';arguments=@{title='企业融资机制'}},
        @{tool='create_successor';arguments=@{at='@@1';title='融资成本决定最优投入';type='model';level='concept';body='融资成本改变企业的投资选择。贷款收紧使企业减少设备投入。投入减少限制下一期的生产能力。来源：[[ModelGroup/1]]'}},
        @{tool='create_successor';arguments=@{at='@@2';kind='side';title='企业比较投资收益与融资成本';type='model';facet='设定';body='可以说，企业比较投资收益与融资成本。融资成本超过收益时，企业取消投资。来源：[[ModelGroup/1]]'}}
    )
    Save-Json $p.start.draft $p.draft
    $env:ZT_CHECK_ZT=Join-Path $PSScriptRoot 'seam-zt-mcp.cmd';$env:ZT_DRAFT_FAULT='cards'
    Refused ($p.args+@('-CheckOnly')) 'incomplete preview Card count' 'a partial native preview cannot create fabricated completion evidence'
    Remove-Item Env:ZT_DRAFT_FAULT,Env:ZT_CHECK_ZT
    Invoke-Zt @(@{tool='create_topic';arguments=@{title='企业融资机制'}}) | Out-Null
    Refused ($p.args+@('-CheckOnly')) 'Topic gate' 'identified groups retain the existing duplicate Topic title gate'
    $p.draft.calls[1].arguments.title='信贷市场的信息结构';Save-Json $p.start.draft $p.draft
    $env:ZT_LEDGER_COMPLETE_FAULT='after_commit'
    Refused $p.args 'injected completion fault' 'a lost post-commit reply leaves one complete producer and its model candidate'
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    $done=Run-Digest $p.args
    Assert ($done.reused -and @($done.children | Where-Object {$_ -like 'models/g001-m*@1'}).Count -eq 1 -and $done.children.Count -eq 5) 'a model header and its parts emit one stable candidate without duplicate downstream work'
    $view=Run-Digest @('ledger','status','-Out',$p.out)
    $model=@($view.units | Where-Object layer -EQ models)[0]
    Assert ($model.inputs.header -eq $done.substantive[0] -and $model.input_hash -match '^[0-9a-f]{64}$') 'the Model candidate binds a verified current header and immutable group evidence'
    Assert (@($done.deferred | Where-Object {$_.fault -match ' R9 ' -and $_.consumer -eq $model.id}).Count -eq 1) 'a Model part quality fault is deferred to its registered Model consumer'
    $again=Run-Digest ($p.args+@('-CheckOnly'))
    Assert ($again.check_only -and -not $again.applied -and $again.reused) 'CheckOnly remains read-only when verifying an already completed group'
    $disabled=Prepared 'UncompiledGroup' '企业的融资约束影响生产选择。' @{models=$false}
    $disabled.draft.calls=@(
        @{tool='edit_card';arguments=@{at='UncompiledGroup/1';title='融资约束影响企业投资';body='融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。';base_fingerprint=$disabled.job.inputs.anchor.fingerprint}},
        @{tool='create_successor';arguments=@{at='UncompiledGroup/1';kind='side';title='融资成本决定最优投入';type='model';level='concept';body='可以说，融资成本改变企业的投资选择。贷款收紧使企业减少设备投入。投入减少限制下一期的生产能力。来源：[[UncompiledGroup/1]]'}}
    )
    Save-Json $disabled.start.draft $disabled.draft
    Refused ($disabled.args+@('-CheckOnly')) 'no enabled Model consumer' 'a disabled Model layer cannot hide a hard fault behind an incapable Card consumer'
    $changed=Prepared 'ChangedDraftGroup' '企业的融资约束影响生产选择。'
    $changed.draft.calls=@(
        @{tool='edit_card';arguments=@{at='ChangedDraftGroup/1';title='融资约束影响企业投资';body='融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。';base_fingerprint=$changed.job.inputs.anchor.fingerprint}},
        @{tool='create_successor';arguments=@{at='ChangedDraftGroup/1';kind='side';title='贷款收紧限制企业投入';body='融资成本影响企业的生产选择。贷款收紧使企业减少设备投入。投入减少限制下一期的生产能力。来源：[[ChangedDraftGroup/1]]'}}
    )
    Save-Json $changed.start.draft $changed.draft
    $replacement=Join-Path $scratch 'late-draft.json'
    $changed.draft.calls[1].arguments.body+='企业推迟下一期的投资。';Save-Json $replacement $changed.draft
    $env:ZT_CHECK_ZT=Join-Path $PSScriptRoot 'seam-zt-mcp.cmd';$env:ZT_REPLACE_PREVIEW_INPUT=$changed.start.draft;$env:ZT_PREVIEW_REPLACEMENT_FILE=$replacement
    Refused $changed.args 'group draft hash mismatch' 'a late draft replacement cannot be hashed as if it were the checked input'
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_REPLACE_PREVIEW_INPUT,Env:ZT_PREVIEW_REPLACEMENT_FILE
    $reserved=Prepared 'ReservationGroup' '企业的融资约束影响生产选择。'
    $reserved.draft.calls=@(
        @{tool='edit_card';arguments=@{at='ReservationGroup/1';title='融资约束影响企业投资';body='融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。';base_fingerprint=$reserved.job.inputs.anchor.fingerprint}},
        @{tool='create_successor';arguments=@{at='ReservationGroup/1';kind='side';title='贷款收紧限制企业投入';body='融资成本影响企业的生产选择。贷款收紧使企业减少设备投入。投入减少限制下一期的生产能力。来源：[[ReservationGroup/1]]'}}
    )
    Save-Json $reserved.start.draft $reserved.draft
    $env:ZT_CHECK_ZT=Join-Path $PSScriptRoot 'seam-zt-mcp.cmd';$env:ZT_RESERVE_BATCH_PARENT='ReservationGroup/1';$env:ZT_RESERVE_BATCH_FLAG=Join-Path $scratch 'reserved-slot.json'
    $owned=Run-Digest $reserved.args
    $other=Get-Content -LiteralPath $env:ZT_RESERVE_BATCH_FLAG -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_RESERVE_BATCH_PARENT,Env:ZT_RESERVE_BATCH_FLAG
    $tree=Invoke-Zt @(@{tool='get_tree';arguments=@{tree='ReservationGroup'}})
    $actual=@($tree.cards | Where-Object title -EQ '贷款收紧限制企业投入')[0].address
    Assert ($owned.owned.Count -eq 2 -and $owned.owned -contains $actual -and $owned.owned -notcontains $other.address -and $owned.substantive[0] -eq $actual) 'completion derives ownership from real receipts after another writer reserves the previewed slot'
    $large=Prepared 'LargeGroup' '企业的融资约束影响生产选择。' @{models=$false;events=$false;findings=$false}
    $large.draft.calls=@(@{tool='edit_card';arguments=@{at='LargeGroup/1';title='融资约束影响企业投资';body='融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。';base_fingerprint=$large.job.inputs.anchor.fingerprint}})
    for($i=0;$i -lt 65;$i++) {$large.draft.calls+=@{tool='create_successor';arguments=@{at='LargeGroup/1';kind='side';title="融资条件影响第${i}项投入";body='融资成本影响企业的生产选择。贷款收紧使企业减少设备投入。投入减少限制下一期的生产能力。来源：[[LargeGroup/1]]'}}}
    Save-Json $large.start.draft $large.draft
    $env:ZT_CHECK_ZT=Join-Path $PSScriptRoot 'seam-zt-mcp.cmd';$env:ZT_DROP_BATCH_REPLY_FLAG=Join-Path $scratch 'large-reply-lost.flag'
    $lost=@(& $Digest @($large.args) 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and (Test-Path $env:ZT_DROP_BATCH_REPLY_FLAG) -and (Invoke-Zt @(@{tool='get_tree';arguments=@{tree='LargeGroup'}})).cards.Count -eq 65) 'the first real 64-operation chunk can commit while its reply is lost'
    Remove-Item Env:ZT_DROP_BATCH_REPLY_FLAG,Env:ZT_CHECK_ZT
    $revision=(Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    $recovered=Run-Digest $large.args
    Assert ($recovered.state -eq 'done' -and $recovered.substantive.Count -eq 65 -and $recovered.children.Count -eq 2 -and (Invoke-Zt @(@{tool='get_tree';arguments=@{tree='LargeGroup'}})).cards.Count -eq 67) 'a new process recovers the prefix and applies only the two missing operations'
    Assert ((Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $revision+1) 'only the unseen tail advances the revision during group recovery'
    $empty=Prepared 'EmptyGroup' '' @{events=$false;findings=$false}
    $empty.draft.disposition='no_cards';Save-Json $empty.start.draft $empty.draft
    $emptyBefore=(Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    $noCards=Run-Digest $empty.args
    Assert ($noCards.state -eq 'done' -and $noCards.disposition -eq 'no_cards' -and $noCards.no_cards.body_lines -eq 0 -and $noCards.substantive.Count -eq 0 -and -not $noCards.applied -and $noCards.self_check.count -eq 0 -and $noCards.self_check.advisories.Count -ge 1 -and (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $emptyBefore) 'a heading-only source has an explicit checked no_cards disposition and separate advisory observations without archive writes'
} finally {
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT,Env:ZT_CHECK_ZT,Env:ZT_DRAFT_FAULT,Env:ZT_DROP_BATCH_REPLY_FLAG,Env:ZT_REPLACE_PREVIEW_INPUT,Env:ZT_PREVIEW_REPLACEMENT_FILE,Env:ZT_RESERVE_BATCH_PARENT,Env:ZT_RESERVE_BATCH_FLAG -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
