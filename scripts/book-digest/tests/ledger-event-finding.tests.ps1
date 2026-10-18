#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
# The Events/Findings layer unit (R220, R221, T15, T18): prepared economics books whose groups
# release Events and Findings work. Every completion goes through the real compiler commands
# (`zt-digest event|finding -Step plan|apply`) and the real unit commands (`events-finish`,
# `findings-finish`, `ledger done`) on a temporary archive.
$ErrorActionPreference='Stop'; $utf8=[Text.UTF8Encoding]::new($false)
$scratch=[IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-event-finding-test-$PID"))
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
function Save-Text($path,$text) {[IO.File]::WriteAllText($path,$text,$utf8)}
function Refused([string[]]$Arguments,[string]$text,$message) {
    $lines=@(& $Digest @Arguments 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($lines -join ' ').Contains($text)) "$message [$($lines -join ' ')]"
}
function Revision { (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision }
function Exists($path) { [IO.File]::Exists($path) }
function Read-Json($path) { [IO.File]::ReadAllText($path) | ConvertFrom-Json -AsHashtable }
function BodyOf($at) { (Invoke-Zt @(@{tool='get_card';arguments=@{at=$at}})).body }
function Set-Body($at,$body) { $null=Invoke-Zt @(@{tool='edit_card';arguments=@{at=$at;body=$body}}) }
# A write batch through the helper, as the compilers write: the addresses of its create calls.
function Write-Zt($calls) {
    $out=@(($calls | ConvertTo-Json -Depth 10 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) {throw "zt: $($out -join ' | ')"}
    , @($out | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Ledger-Inspect([string]$BookOut,[string]$script) {
    $guard=@'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-event-finding-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);
'@
    $result=& node --no-warnings -e ($guard+$script) (Join-Path $BookOut 'ledger.sqlite3')
    if ($LASTEXITCODE) {throw 'isolated ledger inspection failed'}
    return $result | ConvertFrom-Json
}
# One prepared economics book of three chapters: chapters 1 and 2 hold dated and empirical
# sentences, chapter 3 holds neither, so its group has a legitimate zero-candidate no-op.
function New-Book([string]$key) {
    $bookOut=Join-Path $scratch "prepared-$key"
    $source=Join-Path $scratch "book-$key.md"; $options=Join-Path $scratch "options-$key.json"
    Save-Text $source "# 第一章`n`n1976 年，团队生产理论出现。监督者减少团队偷懒。`n`n一项覆盖 1200 家企业的调查显示，42% 的企业设有监督者。`n`n# 第二章`n`n1919 年，通用汽车与费雪公司签订十年合同。长期契约约束供应商抬价。`n`n作者访谈了 48 家企业，其中 31 家推迟投资。`n`n# 第三章`n`n企业可以选择一体化。契约也能替代一体化。`n`n# 参考`n未选择。"
    Save-Json $options @{terms=$false}
    $null=Run-Digest @('prepare','-Source',$source,'-Out',$bookOut,'-CitationKey',$key,'-BibTeX',"@book{$key,title={$key book}}",'-StartHeading','第一章','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    $ideas=@{}
    foreach ($g in 1..3) {
        $claim=(Run-Digest @('ledger','next','-Out',$bookOut,'-Layer','digest','-Take','1','-Owner','event-finding-manager')).units[0]
        $started=Run-Digest @('group-start','-Out',$bookOut,'-Group',"$g",'-Token',"$($claim.token)")
        $job=Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
        $at="$key/$g"
        $draft=@{schema_version=1;unit=$claim.id;token=$claim.token;input_hash=$started.input_hash;disposition='cards';calls=@(
            @{tool='edit_card';arguments=@{at=$at;title="第 $g 组说明的机制";body="第 $g 组说明了一个机制。企业因此调整生产。";base_fingerprint=$job.inputs.anchor.fingerprint}},
            @{tool='create_successor';arguments=@{at=$at;kind='side';title="第 $g 组的观点卡";body="第 $g 组给出一个论点。企业因此调整行为。来源：[[$at]]"}}
        )}
        Save-Json $started.draft $draft
        $done=Run-Digest @('group-finish','-Out',$bookOut,'-Group',"$g",'-Token',"$($claim.token)",'-Draft',$started.draft)
        $ideas[$g]=@($done.owned | Where-Object { $_ -like "$at|*" })[0]
    }
    return [pscustomobject]@{ Out=$bookOut; Key=$key; Ideas=$ideas }
}
function Claim([string]$bookOut,[string]$layer) {
    $claim=(Run-Digest @('ledger','next','-Out',$bookOut,'-Layer',$layer,'-Take','1','-Owner',"$layer-manager")).units[0]
    if (-not $claim) {throw "no ready $layer unit"}
    return $claim
}
function Event-Answer($group,$idea,$src,$section,$title,$course) {
    return @{group=$group;src=$src;title=$title;time='1976 年';place='美国';actors='阿尔钦、德姆塞茨';course=$course;section=$section;supports=@($idea);role="本事件例示 [[$idea]] 的论点：监督减少偷懒。";existing=''}
}
function Finding-Answer($group,$idea,$src,$section,$title,$result,$limits) {
    return @{group=$group;src=$src;title=$title;measure='企业采用监督者的比例。';data='1200 家企业的调查。';method='问卷调查。';result=$result;limits=$limits;section=$section;supports=@($idea);role="这一结果印证 [[$idea]] 的判断：监督减少偷懒。"}
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $book=New-Book 'SettleBook'
    $out=$book.Out; $key=$book.Key; $ideas=$book.Ideas
    Assert (@($ideas.Keys).Count -eq 3 -and $ideas[1] -like "$key/1|*" -and $ideas[3] -like "$key/3|*") "the prepared book releases one substantive Card per group ($($ideas[1]), $($ideas[3]))"

    # --- The accepted candidate/job manifest ------------------------------------------------
    $planned=Run-Digest @('event','-Step','plan','-Staging',$out,'-GroupsPerJob','1')
    Assert ($planned.candidates -eq 2 -and $planned.groups -eq 2 -and @($planned.jobs).Count -eq 2 -and $planned.expected_jobs -eq 2) "plan: two dated candidates of two groups in two jobs ($($planned.candidates), $($planned.groups))"
    $planPath=Join-Path $out 'events\plan.json'
    $plan=Get-Content -LiteralPath $planPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($plan.layer -eq 'events' -and $plan.tool -eq 'event' -and $plan.jobs[0].id -eq 'ev-01' -and $plan.jobs[0].groups[0] -eq 1 -and $plan.jobs[1].id -eq 'ev-02' -and $plan.jobs[1].groups[0] -eq 2) 'plan: the accepted manifest names each job and the groups it covers'
    Assert ($plan.candidates.g001[0].id -eq 'g001/c001' -and $plan.candidates.g001[0].src -eq 'sections/s01.md:3' -and $plan.candidates.g002[0].id -eq 'g002/c001' -and -not $plan.candidates.ContainsKey('g003')) 'plan: candidates carry stable ids and source spans, and the candidate-less group is not invented'
    Assert ($plan.candidate_source.kind -eq 'artifact' -and $plan.candidate_source.path -like '*events.json' -and $plan.candidate_source.sha256.Length -eq 64) 'plan: the manifest names the prepared events artifact'

    # --- T18: candidates with no applied job never report done -------------------------------
    $eventsClaim=Claim $out 'events'
    Refused @('events-finish','-Out',$out,'-Group','1','-Token',"$($eventsClaim.token)",'-CheckOnly') 'was never applied' 'an unapplied job with declared candidates cannot complete (T18: empty job list)'
    Refused @('events-finish','-Out',$out,'-Group','1','-Token','999','-CheckOnly') 'stale_claim' 'a wrong token cannot complete the layer'
    Assert (-not (Test-Path (Join-Path $out 'events-completion-g001.json'))) 'a refused check writes no completion evidence'

    # --- Applying the answers keeps the candidate manifest ---------------------------------
    $answers=@(
        (Event-Answer 1 $ideas[1] 'sections/s01.md:3' '§1' '1976 年阿尔钦和德姆塞茨发表团队生产理论' '两人发表团队生产理论。'),
        (Event-Answer 2 $ideas[2] 'sections/s02.md:3' '§2' '1919 年通用汽车与费雪公司签订十年合同' '两家公司签订十年合同。')
    )
    Save-Json (Join-Path $out 'events\out\ev-01.json') @{events=@($answers[0])}
    Save-Json (Join-Path $out 'events\out\ev-02.json') @{events=@($answers[1])}
    $applied=Run-Digest @('event','-Step','apply','-Staging',$out,'-Apply')
    Assert ($applied.ok -and $applied.created -eq 2 -and $applied.linked -eq 2 -and $applied.pending_jobs -eq 0) "apply: two Event Cards and their Links ($($applied | ConvertTo-Json -Compress))"
    $state=Get-Content -LiteralPath (Join-Path $out 'events\state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $entry=$state.applied['ev-01'].entries[0]
    Assert ($state.applied['ev-01'].answer.sha256 -and $entry.candidate -eq 'g001/c001' -and $entry.src -eq 'sections/s01.md:3' -and $entry.card -eq $state.applied['ev-01'].created[0] -and $entry.supports[0] -eq $ideas[1]) 'apply: the accepted job keeps its answer, candidate, Card and supports'

    # --- The checked completion --------------------------------------------------------------
    $before=Revision
    $checked=Run-Digest @('events-finish','-Out',$out,'-Group','1','-Token',"$($eventsClaim.token)",'-CheckOnly')
    Assert ($checked.check_only -and -not $checked.applied -and $checked.disposition -eq 'verified' -and @($checked.candidates).Count -eq 1 -and $checked.candidates[0].outcome -eq 'card' -and @($checked.accepted).Count -eq 1 -and $checked.self_check.rules -eq 'E1,E2,E5' -and $checked.self_check.count -eq 0) "CheckOnly reports the checked accepted candidate and passes the Event rules ($($checked.disposition), $($checked.self_check.rules))"
    Assert ((Revision) -eq $before -and -not (Test-Path (Join-Path $out 'events-completion-g001.json')) -and -not (Test-Path (Join-Path $out 'events-list-g001.json'))) 'CheckOnly writes nothing'
    Refused @('ledger','done','-Out',$out,'-Unit','events/g001@1','-Token',"$($eventsClaim.token)",'-Outputs',(Join-Path $out 'manifest.json')) 'completion evidence' 'ledger done cannot accept an arbitrary file as Events completion evidence'
    $done=Run-Digest @('events-finish','-Out',$out,'-Group','1','-Token',"$($eventsClaim.token)")
    Assert ($done.state -eq 'done' -and $done.complete -and $done.disposition -eq 'verified' -and $done.reused -eq $false -and $done.trees -contains $key) 'the Events unit completes as verified with its touched tree'
    Assert ((@($done.files.Keys | Sort-Object) -join ',') -eq 'check,completion,inputs,list,source' -and (Exists $done.files.list.path) -and (Exists $done.files.source.path) -and (Exists $done.files.inputs.path) -and (Exists $done.files.check.path)) 'the completion registers its list, source, Link/Organize inputs and check evidence'
    $list=Read-Json $done.files.list.path
    Assert ($list.candidates[0].outcome -eq 'card' -and $list.candidates[0].card -eq $done.accepted[0].card -and $list.jobs[0].answer.sha256 -eq $state.applied['ev-01'].answer.sha256) 'the Event list records the accepted candidate, its Card and the applied answer'
    $inputs=Read-Json $done.files.inputs.path
    Assert ($inputs.organize.anchor -eq "$key/1" -and $inputs.link.links[0].kind -eq '案例' -and $inputs.link.links[0].idea -eq $ideas[1] -and $inputs.hubs.touched_trees -contains $key) 'the registered inputs name the Organize anchor, the kinded Link and the touched hub tree'
    $unit=@((Run-Digest @('ledger','status','-Out',$out,'-Layer','events')).units | Where-Object id -EQ 'events/g001@1')
    Assert ($unit.Count -eq 1 -and $unit[0].state -eq 'done' -and $unit[0].result.complete -and @($unit[0].result.jobs).Count -eq 1) 'the ledger records the done Events unit with its job evidence'
    $again=Run-Digest @('events-finish','-Out',$out,'-Group','1','-Token',"$($eventsClaim.token)")
    Assert ($again.reused -and $again.state -eq 'done' -and $again.accepted[0].card -eq $done.accepted[0].card) 'a repeated completion reuses the same checked result without new work'

    # --- Findings: one checked result with source, data, method and limits -------------------
    $findingsPlan=Run-Digest @('finding','-Step','plan','-Staging',$out,'-GroupsPerJob','1')
    Assert ($findingsPlan.candidates -eq 2 -and $findingsPlan.expected_jobs -eq 2) "finding plan: two result candidates in two jobs ($($findingsPlan.candidates))"
    $findingAnswers=@(
        (Finding-Answer 1 $ideas[1] 'sections/s01.md:5' '§1' '42% 的受访企业设有监督者' '42% 的企业采用监督者。' '原书未写明抽样方法。'),
        (Finding-Answer 2 $ideas[2] 'sections/s02.md:5' '§2' '48 家受访企业中 31 家推迟投资' '31 家企业推迟投资。' '原因由企业自报。')
    )
    Save-Json (Join-Path $out 'findings\out\fd-01.json') @{findings=@($findingAnswers[0])}
    Save-Json (Join-Path $out 'findings\out\fd-02.json') @{findings=@($findingAnswers[1])}
    $findingsApplied=Run-Digest @('finding','-Step','apply','-Staging',$out,'-Apply')
    Assert ($findingsApplied.ok -and $findingsApplied.created -eq 2 -and $findingsApplied.linked -eq 2 -and $findingsApplied.pending_jobs -eq 0) "finding apply: two Finding Cards and their Links ($($findingsApplied | ConvertTo-Json -Compress))"
    $findingsState=Get-Content -LiteralPath (Join-Path $out 'findings\state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $findingCard=$findingsState.applied['fd-01'].created[0]
    $findingClaim=Claim $out 'findings'
    $findingChecked=Run-Digest @('findings-finish','-Out',$out,'-Group','1','-Token',"$($findingClaim.token)",'-CheckOnly')
    Assert ($findingChecked.disposition -eq 'verified' -and $findingChecked.self_check.rules -eq 'F1,F2,F3,F4' -and $findingChecked.self_check.count -eq 0 -and $findingChecked.accepted[0].card -eq $findingCard) 'CheckOnly passes the Finding rules for the accepted result'
    $findingBody=BodyOf $findingCard
    $ideaBody=BodyOf $ideas[1]
    Set-Body $findingCard ($findingBody -replace '(?m)^局限：.*$', '局限：')
    Refused @('findings-finish','-Out',$out,'-Group','1','-Token',"$($findingClaim.token)",'-CheckOnly') '局限' 'a Finding Card without its limits cannot complete (T18: missing evidence)'
    Set-Body $findingCard ($findingBody -replace '(?m)^结果：.*$', "结果：`n  - 31 家企业推迟投资。`n  - 12 家企业取消投资。")
    Refused @('findings-finish','-Out',$out,'-Group','1','-Token',"$($findingClaim.token)",'-CheckOnly') 'F3' 'a Finding Card holding two results cannot complete (one result per accepted Finding)'
    Set-Body $findingCard $findingBody
    Set-Body $ideas[1] ($ideaBody -replace '(?m)^印证 \[\[.*$', '')
    Refused @('findings-finish','-Out',$out,'-Group','1','-Token',"$($findingClaim.token)",'-CheckOnly') 'holds no' 'an accepted Finding without its kinded Link from the supported Card cannot complete'
    Set-Body $ideas[1] $ideaBody
    $findingDone=Run-Digest @('findings-finish','-Out',$out,'-Group','1','-Token',"$($findingClaim.token)")
    Assert ($findingDone.state -eq 'done' -and $findingDone.disposition -eq 'verified' -and @($findingDone.accepted).Count -eq 1 -and $findingDone.self_check.rules -eq 'F1,F2,F3,F4') 'the Findings unit completes as verified after the Cards are restored'
    Assert ((@($findingDone.files.Keys | Sort-Object) -join ',') -eq 'check,completion,inputs,list,source' -and (Exists $findingDone.files.inputs.path)) 'the Findings completion registers its list, source and inputs atomically'

    # --- A missing or changed manifest, or a changed answer, never reports done --------------
    $second=Claim $out 'events'
    $planBytes=[IO.File]::ReadAllBytes($planPath)
    Move-Item -LiteralPath $planPath -Destination "$planPath.aside"
    Refused @('events-finish','-Out',$out,'-Group','2','-Token',"$($second.token)",'-CheckOnly') 'missing accepted events candidate manifest' 'a missing candidate manifest cannot complete (T18: missing JSON)'
    Move-Item -LiteralPath "$planPath.aside" -Destination $planPath
    $changed=$plan | ConvertTo-Json -Depth 30 | ConvertFrom-Json -AsHashtable
    $changed.candidates.g002[0].src='sections/s02.md:99'
    Save-Json $planPath $changed
    Refused @('events-finish','-Out',$out,'-Group','2','-Token',"$($second.token)",'-CheckOnly') 'no longer describes group 2' 'a changed candidate manifest cannot complete (T18: wrong source span)'
    $emptyJobs=$plan | ConvertTo-Json -Depth 30 | ConvertFrom-Json -AsHashtable
    $emptyJobs.jobs=@()
    Save-Json $planPath $emptyJobs
    Refused @('events-finish','-Out',$out,'-Group','2','-Token',"$($second.token)",'-CheckOnly') 'cover' 'a candidate manifest with no planned job cannot complete (T18: unexpected empty job list)'
    [IO.File]::WriteAllBytes($planPath,$planBytes)
    $answerPath=Join-Path $out 'events\out\ev-02.json'; $answerBytes=[IO.File]::ReadAllBytes($answerPath)
    $tampered=@{events=@($answers[1])}; $tampered.events[0].title='1919 年通用汽车收购费雪公司'
    Save-Json $answerPath $tampered
    Refused @('events-finish','-Out',$out,'-Group','2','-Token',"$($second.token)",'-CheckOnly') 'changed after it was accepted' 'a changed answer cannot complete (T18: wrong accepted manifest)'
    [IO.File]::WriteAllBytes($answerPath,$answerBytes)
    $secondDone=Run-Digest @('events-finish','-Out',$out,'-Group','2','-Token',"$($second.token)")
    Assert ($secondDone.state -eq 'done' -and $secondDone.disposition -eq 'verified' -and @($secondDone.accepted).Count -eq 1) 'the restored manifest and answer complete the second Events unit'
    $secondFindingClaim=Claim $out 'findings'
    $secondFinding=Run-Digest @('findings-finish','-Out',$out,'-Group','2','-Token',"$($secondFindingClaim.token)")
    Assert ($secondFinding.state -eq 'done' -and $secondFinding.disposition -eq 'verified') 'the second applied Findings unit completes'

    # --- A changed accepted Card makes the retained completion stale ------------------------
    $eventCard=$done.accepted[0].card; $eventBody=BodyOf $eventCard
    Set-Body $eventCard ($eventBody -replace '(?m)^经过：.*$','经过：')
    Refused @('events-finish','-Out',$out,'-Group','1','-Token',"$($eventsClaim.token)") '经过' 'an Event Card without its labels makes the retained completion stale (T18: missing Event rules evidence)'
    Set-Body $eventCard $eventBody

    # --- Offline status reconciles the done unit's required outputs --------------------------
    $null=Ledger-Inspect $out 'db.prepare("delete from outputs where unit_id=? and output_key=?").run("events/g001@1","check");console.log(JSON.stringify({deleted:1}));'
    $gate=Run-Digest @('ledger','status','-Out',$out,'-Layer','events')
    Assert (@($gate.coverage_faults | Where-Object { $_.code -eq 'producer_outputs_mismatch' -and $_.unit -eq 'events/g001@1' -and @($_.missing) -contains 'check' }).Count -eq 1) 'offline status names a lost Events output registration'

    # --- Zero candidates is an explicit checked disposition, never a silent no-op -----------
    $emptyClaim=Claim $out 'events'
    $empty=Run-Digest @('events-finish','-Out',$out,'-Group','3','-Token',"$($emptyClaim.token)")
    Assert ($empty.state -eq 'done' -and $empty.disposition -eq 'no_candidates' -and @($empty.candidates).Count -eq 0 -and @($empty.accepted).Count -eq 0 -and @($empty.jobs).Count -eq 0) 'a group with no dated candidate completes with an explicit zero-candidate disposition'
    $emptyList=Read-Json $empty.files.list.path
    Assert ($emptyList.disposition -eq 'no_candidates' -and $emptyList.candidate_source.kind -eq 'artifact') 'the zero-candidate list keeps the enumeration source it checked'
    $emptyFindingClaim=Claim $out 'findings'
    $emptyFinding=Run-Digest @('findings-finish','-Out',$out,'-Group','3','-Token',"$($emptyFindingClaim.token)")
    Assert ($emptyFinding.state -eq 'done' -and $emptyFinding.disposition -eq 'no_candidates' -and $emptyFinding.candidate_source.kind -eq 'sections') 'a group with no result candidate completes with an explicit zero-candidate disposition'

    # --- One refused group never suppresses another; a revised answer recovers the job -------
    $other=New-Book 'SettleBook2'
    $out2=$other.Out; $key2=$other.Key; $ideas2=$other.Ideas
    $null=Run-Digest @('event','-Step','plan','-Staging',$out2,'-GroupsPerJob','1')
    $good=Event-Answer 1 $ideas2[1] 'sections/s01.md:3' '§1' '1976 年第二册报告团队生产理论' '两人发表团队生产理论。'
    $bad=$good.Clone(); $bad.group=2; $bad.src='sections/s02.md:3'; $bad.title='第 2 组；坏标题'; $bad.supports=@($ideas2[2]); $bad.role="本事件例示 [[$($ideas2[2])]] 的论点：长期契约约束抬价。"
    Save-Json (Join-Path $out2 'events\out\ev-01.json') @{events=@($good)}
    Save-Json (Join-Path $out2 'events\out\ev-02.json') @{events=@($bad)}
    $firstApply=Run-Digest @('event','-Step','apply','-Staging',$out2,'-Apply')
    Assert (-not $firstApply.ok -or @($firstApply.refused).Count -ge 1) "apply: an entry that breaks the Event rules is refused, not written ($($firstApply | ConvertTo-Json -Compress))"
    $firstClaim=Claim $out2 'events'
    $firstDone=Run-Digest @('events-finish','-Out',$out2,'-Group','1','-Token',"$($firstClaim.token)")
    Assert ($firstDone.state -eq 'done' -and $firstDone.disposition -eq 'verified') 'the group with accepted Cards completes while the other group holds a refused entry (one failure does not suppress another)'
    $secondClaim=Claim $out2 'events'
    Refused @('events-finish','-Out',$out2,'-Group','2','-Token',"$($secondClaim.token)",'-CheckOnly') 'refused' 'the group whose only entry was refused cannot complete (T18: refused work is not a no-op)'
    $fixed=$good.Clone(); $fixed.group=2; $fixed.src='sections/s02.md:3'; $fixed.title='1919 年第二册记录通用汽车的长期合同'; $fixed.course='两家公司签订十年合同。'
    $fixed.section='§2'; $fixed.supports=@($ideas2[2]); $fixed.role="本事件例示 [[$($ideas2[2])]] 的论点：长期契约约束抬价。"
    Save-Json (Join-Path $out2 'events\out\ev-02.json') @{events=@($fixed)}
    $retried=Run-Digest @('event','-Step','apply','-Staging',$out2,'-Apply')
    Assert ($retried.ok -and $retried.applied_jobs -eq 1 -and $retried.created -eq 1 -and $retried.refused.Count -eq 0) "the revised answer re-opens its applied job and writes the missing Card ($($retried | ConvertTo-Json -Compress))"
    $secondDone=Run-Digest @('events-finish','-Out',$out2,'-Group','2','-Token',"$($secondClaim.token)")
    $found=@(@(Invoke-Zt @(@{tool='search_cards';arguments=@{query='1919 年第二册记录通用汽车的长期合同';type='event'}}))[0].results)
    Assert ($secondDone.state -eq 'done' -and @($secondDone.accepted).Count -eq 1 -and $found.Count -eq 1) 'the recovered group completes with one Event Card, not a duplicate'

    # --- Retry recovers a pending Finding job without a duplicate Card ----------------------
    $null=Run-Digest @('finding','-Step','plan','-Staging',$out2,'-GroupsPerJob','1')
    $finding1=Finding-Answer 1 $ideas2[1] 'sections/s01.md:5' '§1' '42% 的第二册受访企业设有监督者' '42% 的企业采用监督者。' '原书未写明抽样方法。'
    $finding2=Finding-Answer 2 $ideas2[2] 'sections/s02.md:5' '§2' '第二册访谈的 31 家企业推迟投资' '31 家企业推迟投资。' '原因由企业自报。'
    Save-Json (Join-Path $out2 'findings\out\fd-01.json') @{findings=@($finding1)}
    Save-Json (Join-Path $out2 'findings\out\fd-02.json') @{findings=@($finding2)}
    $env:ZT_RACE_TARGET=$ideas2[1]
    $env:ZT_RACE_FLAG=Join-Path $scratch 'raced-findings'
    $env:ZT_RACE_LINE='事件阶段在这里补了一句。'
    $raced=Run-Digest @('finding','-Step','apply','-Staging',$out2,'-Apply')
    Assert (-not $raced.ok -and $raced.pending_jobs -eq 1) "a helper run that fails part way leaves its Finding job pending ($($raced | ConvertTo-Json -Compress))"
    $racingClaim=Claim $out2 'findings'
    Refused @('findings-finish','-Out',$out2,'-Group','1','-Token',"$($racingClaim.token)",'-CheckOnly') 'pending' 'a pending Finding job cannot complete (T18: pending jobs above zero)'
    $env:ZT_RACE_TARGET=''
    $retry=Run-Digest @('finding','-Step','apply','-Staging',$out2,'-Apply')
    Assert ($retry.ok -and $retry.applied_jobs -eq 1 -and $retry.created -eq 0 -and $retry.linked -eq 1 -and $retry.pending_jobs -eq 0) "the retry applies the pending job and reuses the Card it made ($($retry | ConvertTo-Json -Compress))"
    $retriedFinding=Run-Digest @('findings-finish','-Out',$out2,'-Group','1','-Token',"$($racingClaim.token)")
    $foundFinding=@(@(Invoke-Zt @(@{tool='search_cards';arguments=@{query='42% 的第二册受访企业设有监督者';type='finding'}}))[0].results)
    Assert ($retriedFinding.state -eq 'done' -and @($retriedFinding.accepted).Count -eq 1 -and (BodyOf $ideas2[1]).Contains($env:ZT_RACE_LINE) -and $foundFinding.Count -eq 1) 'the retried job completes with one Card, the parallel line and the real Link'

    # --- T15: completion evidence and outputs commit together or not at all ------------------
    $atomicClaim=Claim $out2 'findings'
    foreach ($point in @('after_outputs','before_commit')) {
        $env:ZT_LEDGER_COMPLETE_FAULT=$point
        Refused @('findings-finish','-Out',$out2,'-Group','2','-Token',"$($atomicClaim.token)") 'injected completion fault' "an injected $point fault rolls back the Findings completion"
        $rolled=Ledger-Inspect $out2 'console.log(JSON.stringify({state:db.prepare("select state from units where id=?").get("findings/g002@1").state,outputs:db.prepare("select count(*) n from outputs where unit_id=?").get("findings/g002@1").n,events:db.prepare("select count(*) n from events where unit_id=? and event=?").get("findings/g002@1","completed").n,producer:db.prepare("select state from units where id=?").get("digest/g002@1").state,producerOutputs:db.prepare("select count(*) n from outputs where unit_id=?").get("digest/g002@1").n}));'
        Assert ($rolled.state -eq 'claimed' -and $rolled.outputs -eq 0 -and $rolled.events -eq 0 -and $rolled.producer -eq 'done' -and $rolled.producerOutputs -gt 0) "the $point fault leaves the unit claimed, no outputs and the done producer's work intact (T15)"
    }
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    $final=Run-Digest @('findings-finish','-Out',$out2,'-Group','2','-Token',"$($atomicClaim.token)")
    Assert ($final.state -eq 'done' -and $final.disposition -eq 'verified' -and @($final.files.Keys).Count -eq 5) 'the same claim completes after the injected faults are cleared'
    $reuse=Run-Digest @('findings-finish','-Out',$out2,'-Group','2','-Token',"$($atomicClaim.token)")
    Assert ($reuse.reused -and $reuse.state -eq 'done') 'the recovered completion is reusable without new outputs'

    # --- A merged Event Card and an applied answer that accepted nothing --------------------
    # A third book merges its first group's event into an existing Event Card of another book
    # and its second group's answer accepts none of the two declared candidates. Both are
    # applied answers, so they complete with `verified` (merged) and `no_cards` (ruled out).
    $third=New-Book 'SettleBook3'
    $out3=$third.Out; $key3=$third.Key; $ideas3=$third.Ideas
    $foreign=Write-Zt @(@{tool='create_literature';arguments=@{workspace='agent';bibtex='@book{ForeignBook,title={Foreign book}}';body='另一本书。'}})
    $foreignEvent=Write-Zt @(@{tool='create_successor';arguments=@{at=$foreign[0];kind='direct';type='event';title='1911 年某公司登记成立';body="时间：1911 年`n地点：美国`n参与者：某公司`n经过：公司登记成立。`n来源：[[ForeignBook]] §1`n`n证据作用：本事件例示一处。"}})
    $null=Run-Digest @('event','-Step','plan','-Staging',$out3,'-GroupsPerJob','1')
    $merged=Event-Answer 1 $ideas3[1] 'sections/s01.md:3' '§1' '1976 年第二册报告团队生产理论' '两人发表团队生产理论。'
    $merged.existing=$foreignEvent[0]; $merged.role="本事件例示 [[$($ideas3[1])]] 的论点：监督减少偷懒。"
    Save-Json (Join-Path $out3 'events\out\ev-01.json') @{events=@($merged)}
    Save-Json (Join-Path $out3 'events\out\ev-02.json') @{events=@()}
    $ruledOut=Run-Digest @('event','-Step','apply','-Staging',$out3,'-Apply')
    Assert ($ruledOut.ok -and $ruledOut.applied_jobs -eq 2 -and $ruledOut.created -eq 0 -and $ruledOut.merged -eq 1 -and $ruledOut.pending_jobs -eq 0) "apply: one merged Event and one applied answer with no entry ($($ruledOut | ConvertTo-Json -Compress))"
    $mergeClaim=Claim $out3 'events'
    $mergeDone=Run-Digest @('events-finish','-Out',$out3,'-Group','1','-Token',"$($mergeClaim.token)")
    Assert ($mergeDone.state -eq 'done' -and $mergeDone.disposition -eq 'verified' -and $mergeDone.accepted[0].merged -eq $true -and $mergeDone.accepted[0].card -eq $foreignEvent[0] -and (BodyOf $foreignEvent[0]).Contains("来源：[[$key3]] §1")) 'a merged Event Card completes as verified with this book''s source link'
    $emptyClaim=Claim $out3 'events'
    $emptyAnswer=Run-Digest @('events-finish','-Out',$out3,'-Group','2','-Token',"$($emptyClaim.token)")
    Assert ($emptyAnswer.state -eq 'done' -and $emptyAnswer.disposition -eq 'no_cards' -and $emptyAnswer.candidates[0].outcome -eq 'not_accepted' -and @($emptyAnswer.accepted).Count -eq 0 -and @($emptyAnswer.jobs).Count -eq 1) 'an applied answer that accepts none of its declared candidates completes as an explicit no_cards disposition'
} finally {
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT, Env:ZT_RACE_TARGET, Env:ZT_RACE_FLAG, Env:ZT_RACE_LINE -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
