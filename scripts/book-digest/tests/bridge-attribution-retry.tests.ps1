#Requires -Version 7.5
$ErrorActionPreference='Stop'
$scratch=Join-Path ([IO.Path]::GetTempPath()) "bridge-attribution-test-$PID"
$staging=Join-Path $scratch 'staging';New-Item -ItemType Directory -Force $staging|Out-Null
$env:ZT_CONFIG_DIR=Join-Path $scratch 'config';$env:ZT_GUI_PORT='0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$compiler=Join-Path $PSScriptRoot '..\bridge-compiler.ps1'
function Calls($calls){$lines=@((ConvertTo-Json -InputObject @($calls) -Depth 20)|pwsh -NoProfile -File $script:ZtHelper -);if($LASTEXITCODE){throw "$lines"};,@($lines|Where-Object{$_ -match '^\d+ ok '}|ForEach-Object{($_ -split ' ok ',2)[1].Trim()})}
function Compile($a){$lines=@(pwsh -NoProfile -File $compiler @a);if($LASTEXITCODE){throw "$lines"};$lines[-1]|ConvertFrom-Json -AsHashtable -DateKind String}
try{
 zt config set archive_root (Join-Path $scratch 'archive')|Out-Null;zt up|Out-Null
 $made=Calls @(
  @{tool='create_literature';arguments=@{bibtex='@book{Znew2026,title={New},year={2026}}'}},
  @{tool='create_successor';arguments=@{at='Znew2026';title='第一组';body='§1 控制权'}},
  @{tool='create_successor';arguments=@{at='Znew2026/1';kind='side';title='控制权的文献笔记';body='控制权改变投资激励。'}},
  @{tool='create_literature';arguments=@{bibtex='@book{Aold2020,title={Old},year={2020}}'}},
  @{tool='create_successor';arguments=@{at='Aold2020';title='第一组';body='§1 控制权'}},
  @{tool='create_successor';arguments=@{at='Aold2020/1';kind='side';title='控制权影响投资';body='控制权通过收益分配改变投资激励。'}},
  @{tool='create_topic';arguments=@{title='控制权';body='控制权与投资'}},
  @{tool='create_successor';arguments=@{at='@@6';title='控制权改变投资激励';body='控制权改变投资激励。来源：[[@@2]]'}}
 )
 $source=$made[7];$target=$made[5]
 $planned=Compile @('-Step','plan','-Staging',$staging,'-CitationKey','Znew2026','-Candidates',1)
 $work=Join-Path $staging 'bridge';$plan=Get-Content -LiteralPath (Join-Path $work 'plan.json') -Raw|ConvertFrom-Json -AsHashtable -DateKind String
 if($plan.cards[$source].book -cne 'Znew2026' -or $target -cnotin @($plan.sources[0].candidates|ForEach-Object address)){throw 'weak-attribution fixture is invalid'}
 $answer=@{schema=1;plan_id=$planned.plan_id;job='br-001';sources=@(@{source=$source;none='';bridges=@(@{carrier=$source;target=$target;word='对应';sentence="对应 [[$target]]：两张卡都由控制权解释投资激励。";why='两张卡讨论同一个控制权机制。'})})}
 [IO.File]::WriteAllText((Join-Path $work 'out\br-001.json'),(ConvertTo-Json -InputObject $answer -Depth 20),[Text.UTF8Encoding]::new($false))
 $report=Compile @('-Step','report','-Staging',$staging,'-CitationKey','Znew2026')
 $applied=Compile @('-Step','apply','-Staging',$staging,'-CitationKey','Znew2026','-ReportId',$report.report_id,'-Accept','all','-Apply')
 $before=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$source}}))[0]
 $again=Compile @('-Step','apply','-Staging',$staging,'-CitationKey','Znew2026','-ReportId',$report.report_id,'-Accept','all','-Apply')
 $after=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$source}}))[0]
 if(-not $applied.ok -or -not $again.ok -or $again.outcomes[0].status -cne 'verified' -or $before.text -cne $after.text){throw "an accepted paragraph that changes inferred book must remain idempotent: $($again|ConvertTo-Json -Depth 8 -Compress)"}
 'ok: a journal-proven accepted paragraph may change inferred attribution without invalidating repeat apply'
}finally{zt down|Out-Null}
