#Requires -Version 7.5
$ErrorActionPreference='Stop'
$scratch=Join-Path ([IO.Path]::GetTempPath()) "bridge-math-test-$PID"
$staging=Join-Path $scratch 'staging'
if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force }
New-Item -ItemType Directory -Force $staging | Out-Null
$env:ZT_CONFIG_DIR=Join-Path $scratch 'config';$env:ZT_GUI_PORT='0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$compiler=Join-Path $PSScriptRoot '..\bridge-compiler.ps1'
function Assert($ok,$text){if(-not $ok){throw $text};Write-Output "ok: $text"}
function Calls($calls){$lines=@((ConvertTo-Json -InputObject @($calls) -Depth 20)|pwsh -NoProfile -File $script:ZtHelper -);if($LASTEXITCODE){throw "$lines"};,@($lines|Where-Object{$_ -match '^\d+ ok '}|ForEach-Object{($_ -split ' ok ',2)[1].Trim()})}
function Compile($a){$lines=@(pwsh -NoProfile -File $compiler @a);$script:Code=$LASTEXITCODE;$lines[-1]|ConvertFrom-Json -AsHashtable -DateKind String}
function Save($path,$value){[IO.File]::WriteAllText($path,(ConvertTo-Json -InputObject $value -Depth 30),[Text.UTF8Encoding]::new($false))}
try{
 zt config set archive_root (Join-Path $scratch 'archive')|Out-Null;zt up|Out-Null
 $made=Calls @(
  @{tool='create_literature';arguments=@{bibtex='@book{Math2026,title={Math},year={2026}}'}},
  @{tool='create_successor';arguments=@{at='Math2026';title='第一组';body='§1 连续映射'}},
  @{tool='create_successor';arguments=@{at='Math2026/1';kind='side';type='model';form='theorem';title='不动点定理';body='连续映射在紧凸集上有不动点。'}},
  @{tool='create_successor';arguments=@{at='Math2026/1';kind='side';type='model';form='definition';title='紧集';body='每个开覆盖都有有限子覆盖。'}},
  @{tool='create_successor';arguments=@{at='Math2026/1';kind='side';type='model';form='theorem';title='未被使用的结果';body='这是没有引用者的结果。'}},
  @{tool='create_successor';arguments=@{at='Math2026/1';kind='side';type='model';form='theory';title='不动点理论';body="**主要结果**：[[@@2]]`n`n**对象**：[[@@3]]"}},
  @{tool='create_literature';arguments=@{bibtex='@book{Econ2020,title={Econ},year={2020}}'}},
  @{tool='create_successor';arguments=@{at='Econ2020';title='第一组';body='§1 均衡'}},
  @{tool='create_successor';arguments=@{at='Econ2020/1';kind='side';title='不动点定理保证经济均衡存在';body='经济模型把均衡表示成连续映射的不动点。'}},
  @{tool='create_successor';arguments=@{at='Math2026/1';kind='side';title='普通想法不属于数学单元';body='这是本书的普通想法。'}},
  @{tool='create_successor';arguments=@{at='Econ2020/1';kind='side';title='不动点定理也给出博弈均衡';body='博弈的最优反应是连续映射，它的不动点是均衡。'}}
 )
 Calls @(
  @{tool='edit_card';arguments=@{at=$made[8];type='model';level='concept'}},
  @{tool='create_successor';arguments=@{at=$made[8];kind='side';type='model';facet='设定';title='均衡模型的设定';body='经济模型使用连续映射表示市场。'}},
  @{tool='create_successor';arguments=@{at=$made[2];kind='side';type='model';facet='前提';title='定理的前提';body='映射连续，定义域紧且凸。'}}
 )|Out-Null
 $result=Compile @('-Step','plan','-Staging',$staging,'-CitationKey','Math2026','-Profile','math')
 Assert ($result.ok -and $result.selected -eq 2) "math selects theory main results and definitions, excluding unused results and ordinary Ideas ($($result|ConvertTo-Json -Compress))"
 $work=Join-Path $staging 'bridge';$plan=Get-Content -LiteralPath (Join-Path $work 'plan.json') -Raw|ConvertFrom-Json -AsHashtable -DateKind String
 $answer=@{schema=1;plan_id=$plan.plan_id;job='br-001';sources=@()}
 foreach($source in $plan.jobs[0].sources){
  $bridges=@(if($source -ceq $made[2]){
    @{carrier=$made[8];target=$source;word='依赖';sentence="依赖 [[$source]]：连续映射在紧凸集上的不动点保证经济均衡存在。";why='经济均衡的存在性使用该数学定理。'}
    @{carrier=$source;target=$made[10];word='对应';sentence="对应 [[$($made[10])]]：博弈均衡的存在性使用同一个不动点结构。";why='两处的不动点结构相同，应用的对象不同。'}
  })
  $answer.sources+=@{source=$source;bridges=@($bridges);none=$(if($bridges.Count){''}else{'没有独立可说明的跨书关系。'})}
 }
 Save (Join-Path $work 'out\br-001.json') $answer
 $check=Compile @('-Step','precheck','-Staging',$staging,'-CitationKey','Math2026','-Job','br-001')
 Assert ($Code -eq 0 -and $check.ok) "an earlier economics Model head can cite a new mathematical theorem ($($check|ConvertTo-Json -Compress))"
 $report=Compile @('-Step','report','-Staging',$staging,'-CitationKey','Math2026')
 Assert ($report.counts.earlier -eq 1 -and $report.counts.new -eq 1) 'the dependency and correspondence are published in separate acceptance sections'
 $published=Get-Content -LiteralPath (Join-Path $work 'proposals.json') -Raw|ConvertFrom-Json -AsHashtable -DateKind String
 $earlierId=@($published.proposals|Where-Object section -CEQ 'earlier')[0].id
 $bad=Compile @('-Step','apply','-Staging',$staging,'-CitationKey','Math2026','-ReportId',$report.report_id,'-Accept',$earlierId,'-Apply')
 Assert ($Code -eq 1 -and -not $bad.ok) 'new-carrier selection cannot authorize an earlier-carrier id'
 $before=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$made[2]}}))[0]
 $earlierBefore=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$made[8]}}))[0]
 $apply=Compile @('-Step','apply','-Staging',$staging,'-CitationKey','Math2026','-ReportId',$report.report_id,'-AcceptEarlier','all','-Apply')
 $after=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$made[2]}}))[0]
 Assert ($apply.ok -and @($apply.outcomes|Where-Object status -CEQ 'verified').Count -eq 1 -and $before.body -ceq $after.body -and $after.model.math.load -eq 1) "separate earlier acceptance changes only the economics carrier and increases the theorem load ($($apply|ConvertTo-Json -Depth 6 -Compress))"
 $earlierAfter=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$made[8]}}))[0]
 Assert (@($before.model.parts).Count -eq 1 -and @($earlierBefore.model.parts).Count -eq 1 -and ($before.model.parts.address -join ',') -ceq ($after.model.parts.address -join ',') -and ($earlierBefore.model.parts.address -join ',') -ceq ($earlierAfter.model.parts.address -join ',')) 'an actual dependency preserves the existing parts of both Model heads'
 $new=Compile @('-Step','apply','-Staging',$staging,'-CitationKey','Math2026','-ReportId',$report.report_id,'-Accept','all','-Apply')
 Assert ($new.ok -and @($new.outcomes|Where-Object status -CEQ 'verified').Count -eq 2) 'another proposal can target a carrier already changed by the same verified report'
}finally{zt down|Out-Null}
