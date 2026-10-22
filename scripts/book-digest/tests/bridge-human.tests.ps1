#Requires -Version 7.5
# -Python is accepted for old command lines and not used: the seed runs on node:sqlite.
param([string]$Python = '')
# Human Cards are seeded only into a newly created, stopped, isolated fixture database.
# All behavior under test then uses the public agent MCP/compiler interfaces.
$ErrorActionPreference='Stop'
$scratch=Join-Path ([IO.Path]::GetTempPath()) "bridge-human-test-$PID"
$archive=Join-Path $scratch 'archive';$staging=Join-Path $scratch 'staging'
New-Item -ItemType Directory -Force $staging|Out-Null
$env:ZT_CONFIG_DIR=Join-Path $scratch 'config';$env:ZT_GUI_PORT='0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$compiler=Join-Path $PSScriptRoot '..\bridge-compiler.ps1'
function Assert($ok,$text){if(-not $ok){throw $text};Write-Output "ok: $text"}
function Calls($calls){$lines=@((ConvertTo-Json -InputObject @($calls) -Depth 20)|pwsh -NoProfile -File $script:ZtHelper - -Open);if($LASTEXITCODE){throw "$lines"};,@($lines|Where-Object{$_ -match '^\d+ ok '}|ForEach-Object{($_ -split ' ok ',2)[1].Trim()})}
function Compile($a){$lines=@(pwsh -NoProfile -File $compiler @a);$script:Code=$LASTEXITCODE;$lines[-1]|ConvertFrom-Json -AsHashtable -DateKind String}
function Audit($out){$lines=@(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -CitationKey New2026 -Out $out -SkipModels);if($LASTEXITCODE){throw "$lines"};Get-Content -LiteralPath (Join-Path $out 'audit.json') -Raw|ConvertFrom-Json -AsHashtable -DateKind String}
try{
 zt config set archive_root $archive|Out-Null;zt up|Out-Null;zt down|Out-Null
 # node:sqlite (Node 22.5+) seeds the stopped scratch database.
 $seed=@'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os')
const p=path.resolve(process.argv[2])
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('bridge-human-test-'))throw new Error('not a scratch archive: '+p)
const c=new DatabaseSync(p)
if(c.prepare('select count(*) as n from cards').get().n!==0)throw new Error('scratch archive is not empty')
const nn=String.fromCharCode(10).repeat(2)
// docs/literature-only-digest-design.md: Bridge targets are Literature-tree Cards, so the human
// target sits in a human Literature tree, and the human Card that links the book links a Card of
// the book's own Literature tree (the source's Direct successor, created below).
c.prepare("insert into cards(location,citation_key,is_topic,is_lit,bibtex,text,workspace,card_type) values(null,'Hum1999',0,1,'@book{Hum1999,title={Human},year={1999}}','# Human','human','idea')").run()
const add=c.prepare("insert into cards(location,is_topic,is_lit,text,workspace,card_type) values(?,0,0,?,'human','idea')")
for(const [at,text] of [['Hum1999/1','# 第一组'+nn+'§1 控制权'],['Hum1999/1|a','# 人类卡片讨论控制权'+nn+'控制权决定投资的激励。'],['Hum1999/1|b','# 人类卡片比较控制权'+nn+'对应 [[New2026/1|a|1]]：两处都讨论控制权决定投资的激励。']])add.run(at,text)
c.close()
'@
 $seedFile=Join-Path $scratch 'seed.cjs';[IO.File]::WriteAllText($seedFile,$seed,[Text.UTF8Encoding]::new($false))
 & node $seedFile (Join-Path $archive 'zt.sqlite3')
 if($LASTEXITCODE){throw 'human fixture seed failed'}
 zt up|Out-Null
 # Direct fixture SQL bypasses ordinary writes; rebuild its derived Link index.
 zt reindex|Out-Null
 if($LASTEXITCODE){throw 'human fixture reindex failed'}
 $made=Calls @(
  @{tool='create_literature';arguments=@{bibtex='@book{New2026,title={New},year={2026}}'}},
  @{tool='create_successor';arguments=@{at='New2026';title='第一组';body='§1 控制权'}},
  @{tool='create_successor';arguments=@{at='New2026/1';kind='side';title='控制权影响投资';body='控制权决定投资的激励。来源：[[New2026]] §1'}}
 )
 $source=$made[2];$target='Hum1999/1|a'
 $before=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$target}}))[0]
 $r=Compile @('-Step','plan','-Staging',$staging,'-CitationKey','New2026')
 $work=Join-Path $staging 'bridge';$plan=Get-Content -LiteralPath (Join-Path $work 'plan.json') -Raw|ConvertFrom-Json -AsHashtable -DateKind String
 Assert ($r.ok -and $plan.cards[$target].workspace -ceq 'human' -and $target -cin @($plan.sources[0].candidates|ForEach-Object address)) 'the planner can offer a substantive human target without attributing it to an agent book'
 $answer=@{schema=1;plan_id=$plan.plan_id;job='br-001';sources=@(@{source=$source;none='';bridges=@(@{carrier=$source;target=$target;word='对应';sentence="对应 [[$target]]：两张卡都由控制权解释投资激励。";why='两张卡讨论同一个控制权机制。'})})}
 [IO.File]::WriteAllText((Join-Path $work 'out\br-001.json'),(ConvertTo-Json -InputObject $answer -Depth 20),[Text.UTF8Encoding]::new($false))
 $report=Compile @('-Step','report','-Staging',$staging,'-CitationKey','New2026')
 $audit=Audit (Join-Path $scratch 'before-audit')
 Assert ($report.ok -and $audit.books[0].bridges.complete -and $audit.books[0].bridges.island -and $audit.books[0].bridges.out.human -eq 0) 'a proposed-only Bridge is absent from the complete actual-Link audit'
 $applied=Compile @('-Step','apply','-Staging',$staging,'-CitationKey','New2026','-ReportId',$report.report_id,'-Accept','all','-Apply')
 $after=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$target}}))[0]
 Assert ($applied.ok -and $applied.outcomes[0].status -ceq 'verified' -and $before.body -ceq $after.body -and $source -cin @($after.inbound|ForEach-Object address)) 'accepted agent prose links a human target whose body stays read-only while Reverse links update'
 Calls @(
  @{tool='create_successor';arguments=@{at=$source;title='控制权的另一个结果';body='控制权改变退出选择。来源：[[New2026]] §2'}},
  @{tool='create_topic';arguments=@{title='未归属的比较';body='比较不同机制'}},
  @{tool='create_successor';arguments=@{at='@@1';title='不同机制影响投资';body="对照 [[$source]]：这张卡使用不同的决策前提。"}}
 )|Out-Null
 $audit=Audit (Join-Path $scratch 'after-audit')
 Assert ($audit.books[0].bridges.out.human -eq 1 -and $audit.books[0].bridges.in.human -eq 1 -and $audit.books[0].bridges.in.unattributed -eq 1 -and -not $audit.books[0].bridges.island) 'a filtered audit includes actual human and unattributed pairs in the correct incoming and outgoing buckets'
}finally{zt down|Out-Null}
