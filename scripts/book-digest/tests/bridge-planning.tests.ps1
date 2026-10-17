#Requires -Version 7.5
$ErrorActionPreference='Stop'
$scratch=Join-Path ([IO.Path]::GetTempPath()) "bridge-planning-test-$PID"
$staging=Join-Path $scratch 'staging';New-Item -ItemType Directory -Force $staging|Out-Null
$env:ZT_CONFIG_DIR=Join-Path $scratch 'config';$env:ZT_GUI_PORT='0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$compiler=Join-Path $PSScriptRoot '..\bridge-compiler.ps1'
function Assert($ok,$text){if(-not $ok){throw $text};Write-Output "ok: $text"}
function Calls($calls){$lines=@((ConvertTo-Json -InputObject @($calls) -Depth 20)|pwsh -NoProfile -File $script:ZtHelper -);if($LASTEXITCODE){throw "$lines"};,@($lines|Where-Object{$_ -match '^\d+ ok '}|ForEach-Object{($_ -split ' ok ',2)[1].Trim()})}
function Compile($a){$lines=@(pwsh -NoProfile -File $compiler @a);$script:Code=$LASTEXITCODE;$lines[-1]|ConvertFrom-Json -AsHashtable -DateKind String}
try{
 zt config set archive_root (Join-Path $scratch 'archive')|Out-Null;zt up|Out-Null
 $calls=@(
  @{tool='create_literature';arguments=@{bibtex='@book{New2026,title={New},year={2026}}'}},
  @{tool='create_successor';arguments=@{at='New2026';title='第一组';body='§1 控制权'}},
  @{tool='create_literature';arguments=@{bibtex='@book{Old2020,title={Old},year={2020}}'}},
  @{tool='create_successor';arguments=@{at='Old2020';title='第一组';body='§1 控制权'}},
  @{tool='create_topic';arguments=@{title='控制权';body='控制权与投资'}}
 )
 for($i=0;$i -lt 121;$i++){$calls+=@{tool='create_successor';arguments=@{at="@@$($calls.Count-1)";title="控制权影响投资 $i";body='控制权改变投资的激励。来源：[[New2026]] §1'}}}
 for($i=0;$i -lt 12;$i++){$calls+=@{tool='create_successor';arguments=@{at=$(if($i){"@@$($calls.Count-1)"}else{'Old2020/1'});kind=$(if($i){'direct'}else{'side'});title="控制权改变投资 $i";body='企业根据控制权决定投资。'}}}
 foreach($level in @('formal','concept','')){$a=@{at='New2026/1';kind='side';type='model';title="控制权模型 $level";body='控制权影响投资的模型。'};if($level){$a.level=$level};$calls+=@{tool='create_successor';arguments=$a}}
 Calls $calls|Out-Null
 $default=Compile @('-Step','plan','-Staging',$staging,'-CitationKey','New2026')
 $plan=Get-Content -LiteralPath (Join-Path $staging 'bridge\plan.json') -Raw|ConvertFrom-Json -AsHashtable -DateKind String
 Assert ($Code -eq 0 -and $default.selected -eq 60 -and $plan.counts.eligible -eq 124 -and $plan.counts.omitted -eq 64 -and $plan.jobs.Count -eq 5) 'default selection is 60 sources, with explicit omissions and five twelve-source jobs'
 Assert ($plan.cards[$plan.sources[0].address].level -ceq 'formal' -and $plan.cards[$plan.sources[1].address].level -ceq 'concept' -and -not $plan.cards[$plan.sources[2].address].level) 'formal, concept and unclassified Model heads precede Topic Ideas'
 Assert (@($plan.sources|Where-Object{$_.candidates.Count -gt 7}).Count -eq 0 -and @($plan.jobs|Where-Object{$_.sources.Count -gt 12}).Count -eq 0) 'default candidate and job caps remain bounded'
 $jobPath=Join-Path $staging 'bridge\jobs\br-001.md';$jobText=Get-Content -LiteralPath $jobPath -Raw -Encoding utf8
 Remove-Item -LiteralPath $jobPath,(Join-Path $staging 'bridge\dsh-bridge-instructions.zh.md'),(Join-Path $staging 'bridge\read-only.allow.txt')
 $resumed=Compile @('-Step','plan','-Staging',$staging,'-CitationKey','New2026')
 Assert ($resumed.ok -and $resumed.plan_id -ceq $plan.plan_id -and (Test-Path -LiteralPath $jobPath) -and (Get-Content -LiteralPath $jobPath -Raw -Encoding utf8) -ceq $jobText -and (Test-Path -LiteralPath (Join-Path $staging 'bridge\dsh-bridge-instructions.zh.md')) -and (Test-Path -LiteralPath (Join-Path $staging 'bridge\read-only.allow.txt'))) 'an interrupted plan render recreates missing job inputs from the same frozen snapshot'
 $max=Compile @('-Step','plan','-Staging',$staging,'-Work',(Join-Path $staging 'maximum'),'-CitationKey','New2026','-MaxSources',120,'-Candidates',8)
 Assert ($max.ok -and $max.selected -eq 120 -and $max.jobs.Count -eq 10) 'the maximum selection dispatches ten jobs rather than exceeding the manager bound'
 $emptyWork=Join-Path $staging 'math-empty'
 $empty=Compile @('-Step','plan','-Staging',$staging,'-Work',$emptyWork,'-CitationKey','New2026','-Profile','math')
 $emptyReport=Compile @('-Step','report','-Staging',$staging,'-Work',$emptyWork,'-CitationKey','New2026')
 $emptyData=Get-Content -LiteralPath (Join-Path $emptyWork 'proposals.json') -Raw|ConvertFrom-Json -AsHashtable -DateKind String
 Assert ($empty.ok -and $empty.selected -eq 0 -and $emptyReport.ok -and $emptyData.empty_reason -match 'No qualifying' -and $emptyData.read_coverage.complete) 'no qualifying math unit produces a successful empty report with an explicit reason and read coverage'
 foreach($option in @(@('-MaxSources','0'),@('-MaxSources','121'),@('-MaxSources','1.5'),@('-Candidates','0'),@('-Candidates','9'),@('-Candidates','2.5'))){
  $bad=Compile (@('-Step','plan','-Staging',$staging,'-CitationKey','New2026')+$option)
  Assert ($Code -eq 1 -and -not $bad.ok) "invalid bound $option is rejected"
 }
 $partialWork=Join-Path $staging 'partial-review';New-Item -ItemType Directory -Force $partialWork,(Join-Path $partialWork 'out')|Out-Null
 Copy-Item -LiteralPath (Join-Path $staging 'bridge\plan.json') -Destination (Join-Path $partialWork 'plan.json')
 $picked=@($plan.sources[0].address,$plan.sources[1].address)
 foreach($job in $plan.jobs){
  $answer=@{schema=1;plan_id=$plan.plan_id;job=$job.id;sources=@()}
  foreach($at in $job.sources){
   $bridges=@();$none='本任务没有确定的跨书关系。'
   if($at -cin $picked){$target=@($plan.sources|Where-Object address -CEQ $at)[0].candidates[0].address;$bridges=@(@{carrier=$at;target=$target;word='对应';sentence="对应 [[$target]]：两处都由控制权解释投资激励。";why='两张卡讨论同一个控制权机制。'});$none=''}
   $answer.sources+=@{source=$at;bridges=$bridges;none=$none}
  }
  [IO.File]::WriteAllText((Join-Path $partialWork "out\$($job.id).json"),(ConvertTo-Json -InputObject $answer -Depth 20),[Text.UTF8Encoding]::new($false))
 }
 $report=Compile @('-Step','report','-Staging',$staging,'-Work',$partialWork,'-CitationKey','New2026')
 Assert ($report.ok -and $report.counts.proposals -eq 2 -and $report.counts.none_sources -eq 58) 'all selected source outcomes reconcile before the two-carrier recovery fixture'
 $raceCopy=Join-Path $scratch 'race\scripts\book-digest';$raceHelper=Join-Path $scratch 'race\skills\zt\scripts'
 New-Item -ItemType Directory -Force $raceCopy,$raceHelper|Out-Null
 Copy-Item -Path (Join-Path $PSScriptRoot '..\*.ps1'),(Join-Path $PSScriptRoot '..\*.md') -Destination $raceCopy
 Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'racing-zt-mcp.ps1') -Destination (Join-Path $raceHelper 'zt-mcp.ps1')
 $env:ZT_REAL_HELPER=$script:ZtHelper;$env:ZT_RACE_TARGET=$picked[1];$env:ZT_RACE_FLAG=Join-Path $scratch 'raced';$env:ZT_RACE_LINE='并行阶段修改第二张卡片。'
 $compiler=Join-Path $raceCopy 'bridge-compiler.ps1'
 $partial=Compile @('-Step','apply','-Staging',$staging,'-Work',$partialWork,'-CitationKey','New2026','-ReportId',$report.report_id,'-Accept','all','-Apply')
 Assert (-not $partial.ok -and $partial.status -ceq 'partial' -and @($partial.outcomes|Where-Object status -CEQ 'verified').Count -eq 1 -and @($partial.outcomes|Where-Object status -CEQ 'conflict').Count -eq 1) 'one successful carrier remains verified when the later carrier conflicts'
 $first=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$picked[0]}}))[0].text
 $retry=Compile @('-Step','apply','-Staging',$staging,'-Work',$partialWork,'-CitationKey','New2026','-ReportId',$report.report_id,'-Accept','all','-Apply')
 $after=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$picked[0]}}))[0].text
 Assert (-not $retry.ok -and $first -ceq $after -and @($retry.outcomes|Where-Object status -CEQ 'verified').Count -eq 1) 'a partial retry preserves the first carrier without duplicating or losing progress'
 $copy=Join-Path $scratch 'fault\scripts\book-digest';$helperDir=Join-Path $scratch 'fault\skills\zt\scripts'
 New-Item -ItemType Directory -Force $copy,$helperDir|Out-Null
 Copy-Item -Path (Join-Path $PSScriptRoot '..\*.ps1'),(Join-Path $PSScriptRoot '..\*.md') -Destination $copy
 Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'faulting-zt-mcp.ps1') -Destination (Join-Path $helperDir 'zt-mcp.ps1')
 $env:ZT_REAL_HELPER=$script:ZtHelper;$env:ZT_FAIL_TREE='Old2020';$compiler=Join-Path $copy 'bridge-compiler.ps1'
 $failedWork=Join-Path $staging 'failed'
 $failed=Compile @('-Step','plan','-Staging',$staging,'-Work',$failedWork,'-CitationKey','New2026')
 Assert ($Code -eq 2 -and -not $failed.ok -and -not(Test-Path -LiteralPath (Join-Path $failedWork 'plan.json'))) 'a required tree read failure publishes no empty snapshot or judge jobs'
 # The audit (zt-digest audit) reads through its own zt mcp, not through the helper, so the faulting
 # helper cannot fail its tree reads. The rule "incomplete external reads leave semantic isolation
 # unknown rather than true" is the Rust test audit::tests::an_incomplete_read_leaves_the_island_unknown.
 # Here the audit reads every tree, so the graph is complete.
 $auditOut=Join-Path $scratch 'audit'
 $lines=@(pwsh -NoProfile -File (Join-Path $copy 'digest-audit.ps1') -CitationKey New2026 -Out $auditOut -SkipModels)
 if($LASTEXITCODE){throw "audit failed: $lines"}
 $audit=Get-Content -LiteralPath (Join-Path $auditOut 'audit.json') -Raw|ConvertFrom-Json -AsHashtable -DateKind String
 Assert ($audit.books[0].bridges.complete -eq $true) 'the audit reads every tree through its own zt mcp, so its graph is complete'
}finally{zt down|Out-Null}
