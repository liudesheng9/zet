#Requires -Version 7.5
# Public precheck against a hand-authored frozen plan. No service is started or needed.
$ErrorActionPreference='Stop'
$scratch=Join-Path ([IO.Path]::GetTempPath()) "bridge-validation-test-$PID"
$work=Join-Path $scratch 'bridge'
New-Item -ItemType Directory -Force $work,(Join-Path $work 'out'),(Join-Path $work 'tmp')|Out-Null
$env:ZT_CONFIG_DIR=Join-Path $scratch 'no-service'
$compiler=Join-Path $PSScriptRoot '..\bridge-compiler.ps1'
$utf8=[Text.UTF8Encoding]::new($false)
function Assert($ok,$text){if(-not $ok){throw $text};Write-Output "ok: $text"}
function Save($p,$v){[IO.File]::WriteAllText($p,(ConvertTo-Json -InputObject $v -Depth 50),$utf8)}
function Compile($a){$script:Lines=@(pwsh -NoProfile -File $compiler @a);$script:Code=$LASTEXITCODE;$Lines[-1]|ConvertFrom-Json -AsHashtable -DateKind String}
function Card($at,$book,$type='idea',$category='idea',$workspace='agent',$form='',$facet=''){
 @{address=$at;tree=($at -split '/')[0];workspace=$workspace;kind='regular';type=$type;category=$category;book=$book;form=$form;facet=$facet;emphasis=$null;level=$null;title='可独立理解的卡片';body='卡片正文。';links=@();kinds=@{};model=$null;fingerprint='sha256:fixture'}
}
$cards=[ordered]@{}
$cards['New2026/1|a']=Card 'New2026/1|a' 'New2026' 'model' 'model' 'agent' 'theorem'
$cards['New2026/1|b']=Card 'New2026/1|b' 'New2026' 'model' 'model' 'agent' 'definition'
$cards['0/1']=Card '0/1' 'New2026'
$cards['0/2']=Card '0/2' 'New2026' 'model' 'model'
$cards['Old2020/1|a']=Card 'Old2020/1|a' 'Old2020'
$cards['Old2020/1|b']=Card 'Old2020/1|b' 'Old2020' 'model' 'model'
$cards['Old2020/1|c']=Card 'Old2020/1|c' 'Old2020' 'model' 'part' 'agent' '' '推导'
$cards['Old2020/1|d']=Card 'Old2020/1|d' 'Old2020' 'model' 'model' 'agent' 'definition'
$cards['Old2020/1|e']=Card 'Old2020/1|e' 'Old2020' 'model' 'model' 'agent' 'theorem'
$cards['Old2020/1|f']=Card 'Old2020/1|f' 'Old2020' 'model' 'model' 'agent' 'theory'
$cards['Old2020/1|t']=Card 'Old2020/1|t' 'Old2020' 'idea' 'terms'
$cards['Old2020/1|t'].body="术语表`n| 术语 | 不使用的词 |`n| --- | --- |`n| 剩余控制权 | 控制权力 |"
$cards['Old2020/1|b|a']=Card 'Old2020/1|b|a' 'Old2020' 'model' 'part' 'agent' '' '设定'
$cards['Old2020/1|b|a'].body="术语表`n| 术语 | 不使用的词 |`n| --- | --- |`n| 净现值 | 净值 |"
$cards['Old2020/1|b'].model=@{build=@{setting='Old2020/1|b|a'}}
$cards['1/1']=Card '1/1' $null 'idea' 'idea' 'human'
$cards['2/1']=Card '2/1' $null
$cards['Old2020/1|z']=Card 'Old2020/1|z' 'Old2020'
$cards['Old2020/1|z'].links=@('0/1')
$cards['Old2020/1|event']=Card 'Old2020/1|event' 'Old2020' 'event' 'event'
$cards['Old2020/1|finding']=Card 'Old2020/1|finding' 'Old2020' 'finding' 'finding'
$cards['Old2020/1|notation']=Card 'Old2020/1|notation' 'Old2020'
$cards['Old2020/1|notation'].title='Notation for this book'
foreach($category in @('root','anchor','hub','structure')){$at="Old2020/2|$($category.Substring(0,1))";$cards[$at]=Card $at 'Old2020' 'idea' $category}
$sourceIds=@('New2026/1|a','New2026/1|b','0/1','0/2')
$targets=@($cards.Keys|Where-Object{$_ -cnotin $sourceIds})+@('New2026/1|b')
$plan=@{schema=1;citation_key='New2026';profile='math';options=@{max_sources=60;candidates=5;books=@('Old2020')};counts=@{eligible=4;selected=4;omitted=0};cards=$cards;book_of=@{};term_tables=@{Old2020='Old2020/1|t'};sources=@();jobs=@();no_candidates=@();omitted_sources=@()}
for($i=0;$i -lt $sourceIds.Count;$i++){$plan.sources+=@{address=$sourceIds[$i];candidates=@($targets|ForEach-Object{@{address=$_;via=@('fixture')}})};$plan.jobs+=@{id=('br-{0:D3}' -f ($i+1));sources=@($sourceIds[$i])}}
$planFile=Join-Path $work 'plan.json';Save $planFile $plan
# Independent implementation of the documented canonical JSON contract, not compiler internals.
node -e 'const f=require("fs"),c=require("crypto"),p=process.argv[1],v=JSON.parse(f.readFileSync(p));const sort=x=>Array.isArray(x)?x.map(sort):x&&typeof x==="object"?Object.fromEntries(Object.keys(x).sort().map(k=>[k,sort(x[k])])):x;v.plan_id="p-"+c.createHash("sha256").update(JSON.stringify(sort(v))).digest("hex");v.created_at="2026-10-16T00:00:00Z";f.writeFileSync(p,JSON.stringify(v))' $planFile
if($LASTEXITCODE){throw 'fixture hash failed'}
$plan=Get-Content -LiteralPath $planFile -Raw|ConvertFrom-Json -AsHashtable -DateKind String
function Check($name,$source,$carrier,$target,$word,$expected,$suffix='两张卡讨论相同的决策机制。',$gap=' '){
 $job=$plan.jobs[[Array]::IndexOf($sourceIds,$source)].id
 $a=@{schema=1;plan_id=$plan.plan_id;job=$job;sources=@(@{source=$source;none='';bridges=@(@{carrier=$carrier;target=$target;word=$word;sentence="$word$gap[[$target]]：$suffix";why='说明两张卡片的具体关系。'})})}
 Save (Join-Path $work "out\$job.json") $a
 $r=Compile @('-Step','precheck','-Staging',$scratch,'-CitationKey','New2026','-Job',$job)
 if($expected){Assert ($Code -eq 1 -and @($Lines|Where-Object{$_ -match "^$expected\b"}).Count) "$name is refused by $expected ($($Lines -join ' | '))"}
 else{Assert ($Code -eq 0 -and $r.ok) "$name passes without any archive connection ($($Lines -join ' | '))"}
}
Check 'human target' '0/1' '0/1' '1/1' '对应' ''
Check 'human carrier' 'New2026/1|a' '1/1' 'New2026/1|a' '依赖' 'BR3'
Check 'unattributed earlier carrier' 'New2026/1|a' '2/1' 'New2026/1|a' '依赖' 'BR4'
Check 'Model-head to part correspondence' '0/2' '0/2' 'Old2020/1|c' '对应' 'BR4'
Check 'Idea rests on part' '0/1' '0/1' 'Old2020/1|c' '依赖' 'BR4'
Check 'Model rests on ordinary Idea' '0/2' '0/2' 'Old2020/1|a' '依赖' 'BR4'
Check 'Idea nondependency to part' '0/1' '0/1' 'Old2020/1|c' '对照' ''
Check 'Model cites a unit head' '0/2' '0/2' 'Old2020/1|e' '依赖' ''
Check 'two Definition heads specialization' 'New2026/1|b' 'New2026/1|b' 'Old2020/1|d' '特例' ''
Check 'nondefinition specialization' '0/1' '0/1' 'Old2020/1|d' '特例' 'BR4'
Check 'Theory carrier' 'New2026/1|a' 'Old2020/1|f' 'New2026/1|a' '依赖' 'BR3'
Check 'Theory target' '0/1' '0/1' 'Old2020/1|f' '对应' 'BR3'
Check 'part carrier' 'New2026/1|a' 'Old2020/1|c' 'New2026/1|a' '依赖' 'BR3'
foreach($type in @('event','finding')){Check "earlier $type carrier" 'New2026/1|a' "Old2020/1|$type" 'New2026/1|a' '依赖' 'BR4'}
foreach($at in @('Old2020/1|t','Old2020/1|notation','Old2020/2|r','Old2020/2|a','Old2020/2|h','Old2020/2|s')){Check "navigation carrier $at" 'New2026/1|a' $at 'New2026/1|a' '依赖' 'BR3'}
foreach($at in @('Old2020/1|t','Old2020/2|r','Old2020/2|a','Old2020/2|h','Old2020/2|s')){Check "navigation target $at" '0/1' '0/1' $at '对应' 'BR3'}
Check 'own-book candidate' '0/1' '0/1' 'New2026/1|b' '对应' 'BR3'
Check 'earlier carrier uses its book terminology' 'New2026/1|a' 'Old2020/1|a' 'New2026/1|a' '依赖' 'BR9' '控制权力决定经济模型中的投资激励。'
Check 'model setting terminology has priority' 'New2026/1|a' 'Old2020/1|b' 'New2026/1|a' '依赖' 'BR9' '净值决定经济模型中的投资激励。'
Check 'model setting does not use a different book table' 'New2026/1|a' 'Old2020/1|b' 'New2026/1|a' '依赖' '' '控制权力决定经济模型中的投资激励。'
Check 'a Model sentence receives Link checks' '0/2' '0/2' 'Old2020/1|e' '对应' 'BR9' ''
Check 'unknown relation kind' '0/1' '0/1' 'Old2020/1|a' '引出' 'BR4'
Check 'unplanned target' '0/1' '0/1' '3/1' '对应' 'BR2'
Check 'an existing reverse Link' '0/1' '0/1' 'Old2020/1|z' '对应' 'BR7'
Check 'chart markup' '0/1' '0/1' 'Old2020/1|a' '对应' 'BR8' '<div>两张卡讨论相同的决策机制。</div>'
Check 'human target before publication' '0/1' '0/1' '1/1' '对应' ''
# Review fixes (2026-10-16).
Check 'a kind word right before the Link, with no space' '0/1' '0/1' 'Old2020/1|a' '对应' '' '两张卡讨论相同的决策机制。' ''
Check 'a Model Card carrier with an evidence word' '0/2' '0/2' 'Old2020/1|a' '印证' 'BR4'
Check 'a Model Card carrier with a correspondence' '0/2' '0/2' 'Old2020/1|a' '对应' ''
$both=@{schema=1;plan_id=$plan.plan_id;job='br-001';sources=@(@{source='New2026/1|a';none='';bridges=@(
 @{carrier='New2026/1|a';target='Old2020/1|a';word='对应';sentence='对应 [[Old2020/1|a]]：两张卡讨论相同的决策机制。';why='两张卡讨论相同的机制。'},
 @{carrier='Old2020/1|a';target='New2026/1|a';word='依赖';sentence='依赖 [[New2026/1|a]]：这张卡使用这个定理的结论。';why='旧书观点使用这个定理。'}
)})}
Save (Join-Path $work 'out\br-001.json') $both
$null=Compile @('-Step','precheck','-Staging',$scratch,'-CitationKey','New2026','-Job','br-001')
Assert ($Code -eq 1 -and @($Lines|Where-Object{$_ -match '^BR6 .*reverse pair'}).Count) "both directions of one pair are refused ($($Lines -join ' | '))"
$conflictWork=Join-Path $scratch 'conflicts'
New-Item -ItemType Directory -Force $conflictWork,(Join-Path $conflictWork 'out')|Out-Null
Copy-Item -LiteralPath $planFile -Destination (Join-Path $conflictWork 'plan.json')
Copy-Item -Path (Join-Path $work 'out\*.json') -Destination (Join-Path $conflictWork 'out')
$conflict=@{schema=1;plan_id=$plan.plan_id;job='br-003';sources=@(@{source='0/1';none='';bridges=@(
 @{carrier='0/1';target='Old2020/1|a';word='对应';sentence='对应 [[Old2020/1|a]]：两张卡讨论相同的决策机制。';why='两张卡讨论相同的控制权机制。'},
 @{carrier='0/1';target='Old2020/1|a';word='对照';sentence='对照 [[Old2020/1|a]]：两张卡使用不同的决策前提。';why='两张卡讨论不同的决策前提。'}
)})}
Save (Join-Path $conflictWork 'out\br-003.json') $conflict
$report=Compile @('-Step','report','-Staging',$scratch,'-Work',$conflictWork,'-CitationKey','New2026')
$published=Get-Content -LiteralPath (Join-Path $conflictWork 'proposals.json') -Raw|ConvertFrom-Json -AsHashtable -DateKind String
Assert (@($published.proposals|Where-Object source -CEQ '0/1').Count -eq 0 -and $report.status -ceq 'partial') 'competing sentences for the same directed pair publish neither executable proposal'
$missingWork=Join-Path $scratch 'missing';New-Item -ItemType Directory -Force $missingWork,(Join-Path $missingWork 'out')|Out-Null
Copy-Item -LiteralPath $planFile -Destination (Join-Path $missingWork 'plan.json')
$missing=Compile @('-Step','report','-Staging',$scratch,'-Work',$missingWork,'-CitationKey','New2026')
Assert (-not $missing.ok -and $missing.status -ceq 'failed' -and $missing.counts.failed_sources -eq 4 -and $missing.counts.none_sources -eq 0) 'every judge job missing is a failed report, not a claim of no relation'
Assert (-not(Test-Path -LiteralPath $env:ZT_CONFIG_DIR)) 'the entire validation fixture used no live or scratch service'
