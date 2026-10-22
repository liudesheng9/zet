#Requires -Version 7.5
# S05 (#98): the bounded earlier-Card context pack of a digest group, at its public seams.
# A scratch archive with twenty-five earlier control-rights Cards, a photosynthesis Literature tree
# with a Model head and part and a long body, a human Literature tree, an accepted register entry and
# a four-group
# book proves the published pack, its selection reason and budgets, the refused placeholders, the
# sparse and truncated cases, the bounded re-read of a reply-limit refusal, and the one-retry rule
# for a read set that crosses archive revisions.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "earlier-context-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST=$Digest; $env:ZT_CONFIG_DIR=Join-Path $scratch 'config'; $env:ZT_GUI_PORT='0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$out=Join-Path $scratch 'prepared'
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments) {
    $lines=@(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) { throw "digest failed: $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Read-Pack([string]$Path) { Get-Content -LiteralPath $Path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable }
function Read-Titles {
    $answer=Invoke-Zt @(@{ tool='export_titles'; arguments=@{ window_start=0 } })
    $map=@{}
    foreach ($row in @($answer.titles)) { $map[[string]$row.title]=[string]$row.address }
    return $map
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    # docs/literature-only-digest-design.md: the pack reads Literature trees only, so the earlier
    # thought is in earlier books' Literature trees. Twenty-five earlier control-rights Cards, so a full
    # twenty-Card pack is reachable and the read set is larger than the pack.
    $control = @(@{ tool='create_literature'; arguments=@{ bibtex='@book{Prior2020,title={Prior book}}'; body='更早的一本书。' } },
        @{ tool='create_successor'; arguments=@{ at='Prior2020'; title='第一组'; body='§1' } }) +
        @(1..25 | ForEach-Object { @{ tool='create_successor'; arguments=@{ at='Prior2020/1'; kind='side'; title="剩余控制权与谈判 $_"; body="剩余控制权决定第 $_ 种谈判地位。控制权分配影响谈判结果。" } } })
    $null = Invoke-Zt $control
    # A sparse group's earlier Cards: three photosynthesis Cards (one body longer than one excerpt)
    # and a Model head with its part.
    $photoLong = '光合作用把光能转成化学能。叶绿体完成这一转换，保存能量。' * 50
    $null = Invoke-Zt @(
        @{ tool='create_literature'; arguments=@{ bibtex='@book{Photo2019,title={Photo book}}'; body='讲植物的书。' } },
        @{ tool='create_successor'; arguments=@{ at='Photo2019'; title='第一组'; body='§1' } },
        @{ tool='create_successor'; arguments=@{ at='Photo2019/1'; kind='side'; title='光合作用与叶绿体'; body='光合作用把光能转成化学能。叶绿体完成这一转换。' } },
        @{ tool='create_successor'; arguments=@{ at='Photo2019/1'; kind='side'; title='光合作用的能量转换'; body='光合作用保存能量。叶绿体中的色素吸收光。' } },
        @{ tool='create_successor'; arguments=@{ at='Photo2019/1'; kind='side'; title='光合作用的长文观察'; body=$photoLong } },
        @{ tool='create_topic'; arguments=@{ title='索引'; body='本主题收放代理工作区的入口。' } }
    )
    $titles=Read-Titles
    $photoModel='Photo2019/1'; $controlModel='Prior2020/1'; $index=$titles['索引']
    # The photosynthesis Model head and part whose titles and bodies name group 4's own source.
    $null = Invoke-Zt @(@{ tool='create_successor'; arguments=@{ at=$photoModel; kind='side'; type='model'; level='concept'; title='光合作用如何把光能转成化学能'; body='问题：光合作用如何把光能转成化学能？叶绿体完成这一转换，保存能量。' } })
    $titles=Read-Titles; $photoHead=$titles['光合作用如何把光能转成化学能']
    $null = Invoke-Zt @(@{ tool='create_successor'; arguments=@{ at=$photoHead; kind='side'; type='model'; facet='假设'; title='假设：叶绿体完成能量转换'; body='假设叶绿体把光能转成化学能，保存能量。' } })
    # The control-rights Model head that the accepted register entry will name, and a part of it.
    $null = Invoke-Zt @(@{ tool='create_successor'; arguments=@{ at=$controlModel; kind='side'; type='model'; level='concept'; title='剩余控制权如何决定谈判地位'; body='问题：剩余控制权如何决定谈判地位？控制权分配影响谈判结果。' } })
    $titles=Read-Titles; $controlHead=$titles['剩余控制权如何决定谈判地位']
    $null = Invoke-Zt @(@{ tool='create_successor'; arguments=@{ at=$controlHead; kind='side'; type='model'; facet='假设'; title='假设：剩余控制权来自所有权'; body='假设剩余控制权来自所有权。' } })
    # A human Literature tree and its Idea: read-only earlier references, never a filing destination.
    $editor=Join-Path $scratch 'editor.ps1'
    [IO.File]::WriteAllText($editor,'param([string]$File); [IO.File]::WriteAllText($File,$env:EARLIER_HUMAN_BODY,[Text.UTF8Encoding]::new($false))',$utf8)
    $env:EDITOR="pwsh -NoProfile -File $editor"
    $env:EARLIER_HUMAN_BODY="@book{HumanNotes2018,title={Human notes}}`n"
    $humanBook=@(zt l --ws human 2>&1)
    $env:EARLIER_HUMAN_BODY="# 剩余控制权的人类观察`n`n人类作者认为剩余控制权决定谈判地位。`n"
    $humanTopic=@(zt n --at 'HumanNotes2018' 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and $humanBook[-1] -eq 'HumanNotes2018' -and $humanTopic[-1] -eq 'HumanNotes2018/1') 'a human Literature tree exists before the first group starts'
    $env:EARLIER_HUMAN_BODY="# 人类对剩余控制权的补充`n`n人类作者补充：控制权分配影响谈判结果。`n"
    $humanIdea=@(zt b --at $humanTopic[-1] 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and $humanIdea[-1] -eq 'HumanNotes2018/1|a') 'the human Workspace holds an earlier Idea beside it'
    $titles=Read-Titles
    Assert (@($titles.Keys | Where-Object { $_ -like '剩余控制权与谈判 *' }).Count -eq 25) 'twenty-five earlier control-rights Cards exist before the first group starts'
    $source=Join-Path $scratch 'book.md'; $options=Join-Path $scratch 'options.json'
    [IO.File]::WriteAllText($source,"# 导论`n`n剩余控制权决定谈判地位。控制权分配影响谈判的结果。`n`n## 债务与控制权`n`n债务把控制权交给债权人。债权人取得剩余控制权。`n`n## 所有权与剩余控制权`n`n所有权决定剩余控制权的归属。所有者取得谈判地位。`n`n## 光合作用`n`n光合作用把光能转成化学能。叶绿体完成这一转换，保存能量。`n`n# 参考`n未选择。",$utf8)
    [IO.File]::WriteAllText($options,'{"terms":false}',$utf8)
    $manifest=Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','Earlier','-BibTeX','@book{Earlier,title={Earlier book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-MaxUnits','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    Assert (@($manifest.groups).Count -eq 4) 'the prepared book holds four groups, one of them about photosynthesis'
    $anchors=@($manifest.groups | ForEach-Object { "Earlier/$($_.n)" })
    $next=Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','4','-Owner','group-manager')
    $claims=@{}
    foreach ($unit in @($next.units)) { $claims[[int]([string]$unit.id -replace '^digest/g(\d+)@1$','$1')] = $unit }
    Assert ($claims.Keys.Count -eq 4) 'four digest groups are claimed in one step'

    $env:ZT_CHECK_ZT=Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    # An earlier Card the read cannot deliver refuses the start before any pack or job is published.
    $env:ZT_BULK_MISSING='Prior2020/1|a'
    $refused=@(& $Digest group-start -Out $out -Group 2 -Token "$($claims[2].token)" 2>&1)
    Remove-Item Env:ZT_BULK_MISSING
    $claimFolder=Join-Path $out "group-jobs\g002\claim-$($claims[2].token)"
    Assert ($LASTEXITCODE -eq 1 -and ($refused -join ' ').Contains('incomplete Card read')) 'an earlier Card the read cannot deliver refuses the start'
    Assert (-not (Test-Path -LiteralPath (Join-Path $claimFolder 'earlier-context.json')) -and -not (Test-Path -LiteralPath (Join-Path $claimFolder 'job.json'))) 'a refused start publishes no pack and no writer job'

    # Group 1: a full pack, a human reference, and one title catalogue and body batch.
    $env:ZT_TRACE_TOOLS=Join-Path $scratch 'trace-1.jsonl'
    $g1=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claims[1].token)")
    Remove-Item Env:ZT_TRACE_TOOLS
    $pack=Read-Pack $g1.earlier_context_path
    Assert ($g1.earlier_context.candidates -eq 20 -and $g1.earlier_context.matches -gt 20) 'group-start publishes a full twenty-Card earlier context from more than twenty matches'
    Assert ($g1.earlier_context.register -eq 'absent' -and $g1.earlier_context.archive_revision -eq $g1.snapshot_revision) 'the pack reports a missing register and the startup revision it was read at'
    Assert ($pack.count -eq 20 -and $pack.omitted.below_selection -ge 1) 'the pack keeps twenty Cards and names the matches below them'
    Assert (@($pack.candidates | Where-Object { $_.workspace -eq 'human' }).Count -ge 1) 'an earlier human Card is offered as context'
    Assert (@($pack.candidates | Where-Object { $_.workspace -eq 'human' -and ($_.writable -or $_.destination -ne 'read-only-reference') }).Count -eq 0) 'every human candidate is a read-only reference, never a filing destination'
    Assert (@($pack.candidates | Where-Object { $_.workspace -eq 'agent' -and (-not $_.writable -or $_.destination -ne 'agent-card') }).Count -eq 0) 'every agent candidate is an agent Card that a Link may target'
    Assert (@($pack.candidates | Where-Object { $_.excerpt_chars -gt 800 }).Count -eq 0 -and $pack.excerpt_chars -le 16000) 'every excerpt is at most 800 characters and the pack at most 16000'
    Assert (@($pack.candidates | Where-Object { $_.locator.tool -ne 'get_card' -or $_.locator.arguments.at -ne $_.address }).Count -eq 0) 'each candidate carries the locator that reads all of it'
    Assert (@($pack.candidates | Where-Object { $_.text_sha256 -notmatch '^[0-9a-f]{64}$' -or $_.text_chars -lt 1 }).Count -eq 0) 'each candidate retains its actual content hash and size'
    Assert (@($pack.candidates | Where-Object { $_.address -in $anchors }).Count -eq 0) 'no unfinished group placeholder is offered as earlier thought'
    Assert (@($pack.catalog.excluded | Where-Object { $_ -in $anchors }).Count -eq 4) 'the pack names every unfinished placeholder it refused'
    Assert (@($pack.candidates | Where-Object { -not $_.ContainsKey('source_evidence') -or $null -eq $_.source_evidence }).Count -eq 0) 'every candidate reports its source evidence, empty when the Card names none'
    $prompt=[IO.File]::ReadAllText($g1.prompt)
    Assert ($prompt.Contains($g1.earlier_context_text) -and $prompt.Contains('早前上下文包')) 'the group prompt names the earlier-context pack it must read'
    $job=Get-Content -LiteralPath $g1.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert (@($job.jobs[0].files) -contains $g1.earlier_context_text -and $job.artifacts.earlier_context.path -eq $g1.earlier_context_path) 'the writer job hands the pack to the run'
    Assert ($job.inputs.earlier_context.sha256 -eq (Get-FileHash -LiteralPath $g1.earlier_context_path -Algorithm SHA256).Hash.ToLowerInvariant()) 'the pack is frozen in the start inputs by hash'
    $text=[IO.File]::ReadAllText($g1.earlier_context_text)
    Assert ($text.Contains('断言同一主张') -and $text.Contains('读全文') -and $text.Contains('人类工作区的卡只读')) 'the pack states its read-before-asserting and read-only rules'
    $trace=@(Get-Content -LiteralPath (Join-Path $scratch 'trace-1.jsonl') | ForEach-Object { $_ | ConvertFrom-Json -AsHashtable })
    Assert (@($trace | Where-Object tool -EQ 'export_titles').Count -eq 1 -and @($trace | Where-Object tool -EQ 'get_register').Count -eq 1) 'one title catalogue and one register read serve the whole pack'
    Assert (@($trace | Where-Object { $_.tool -eq 'get_cards' -and $_.address_count -gt 200 }).Count -eq 0) 'no earlier-Card read exceeds the 200-address bound'
    Assert (@($trace | Where-Object { $_.tool -eq 'get_cards' -and $_.address_count -ge 10 }).Count -eq 1) 'one bounded text batch reads every earlier-Card body'
    # The legacy dsh-instructions.zh.md is deleted (docs/literature-only-digest-design.md); the pack
    # text above states the read-the-whole-Card rule to the writer.
    $before=(Get-FileHash -LiteralPath $g1.earlier_context_path -Algorithm SHA256).Hash
    $again=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claims[1].token)")
    Assert ($again.reused -and (Get-FileHash -LiteralPath $again.earlier_context_path -Algorithm SHA256).Hash -eq $before) 'a resumed start reuses the frozen pack byte for byte'

    # The agent register, published after the first pack: its accepted term must lead the next pack.
    $registerBody="范围：代理工作区的入口。`n`n## 词条`n- 剩余控制权：[[$controlHead]]"
    $null = Invoke-Zt @(@{ tool='create_successor'; arguments=@{ at=$index; type='hub'; level='register'; title='索引：代理工作区的入口'; body=$registerBody } })
    $view=(Invoke-Zt @(@{ tool='get_register'; arguments=@{ workspace='agent' } }))[0]
    Assert ($view.register_missing -eq $false -and $view.entry_total -eq 1 -and @($view.entries[0].targets)[0].address -eq $controlHead) 'the scratch archive gains one accepted register entry with its target'

    # Group 2: a reply-limit refusal halves the earlier-Card read and the pack is still complete.
    $env:ZT_TRACE_TOOLS=Join-Path $scratch 'trace-2.jsonl'; $env:ZT_BULK_LIMIT='5'
    $g2=Run-Digest @('group-start','-Out',$out,'-Group','2','-Token',"$($claims[2].token)")
    Remove-Item Env:ZT_BULK_LIMIT,Env:ZT_TRACE_TOOLS
    $reads=@(Get-Content -LiteralPath (Join-Path $scratch 'trace-2.jsonl') | ForEach-Object { $_ | ConvertFrom-Json -AsHashtable } | Where-Object { $_.tool -eq 'get_cards' -and $_.address_count -gt 0 })
    $pack2=Read-Pack $g2.earlier_context_path
    Assert ($pack2.register.status -eq 'read' -and $pack2.candidates[0].address -eq $controlHead -and @($pack2.candidates[0].reason.register_terms) -contains '剩余控制权') 'an accepted register term puts its own target first and says why'
    Assert ($reads.Count -ge 8 -and $pack2.count -eq 20) 'a reply-limit refusal halves the earlier-Card read and still yields the full pack'

    # Group 3: one crossed read set is retried; a persistent one is one changing-view fault.
    $env:ZT_DRIFT_MIN_ADDRESSES='10'; $env:ZT_DRIFT_REVISION='all'
    $drift=@(& $Digest group-start -Out $out -Group 3 -Token $claims[3].token 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($drift -join ' ').Contains('earlier-Card context')) 'a read set that keeps crossing revisions is one changing-view fault'
    $env:ZT_DRIFT_REVISION='once'
    $g3=Run-Digest @('group-start','-Out',$out,'-Group','3','-Token',"$($claims[3].token)")
    Remove-Item Env:ZT_DRIFT_MIN_ADDRESSES,Env:ZT_DRIFT_REVISION
    $pack3=Read-Pack $g3.earlier_context_path
    Assert ($pack3.archive.revision -eq $g3.snapshot_revision -and $pack3.count -gt 0) 'one crossed read set is retried and the pack comes from one view'
    Assert ($pack3.archive.identity.id -eq $pack.archive.identity.id) 'the pack binds the archive identity it was read from'

    # Group 4: a sparse, truncated, model-part pack over a register that became ambiguous.
    $null = Invoke-Zt @(@{ tool='create_topic'; arguments=@{ title='第二个入口'; body='另一个入口的收放处。' } })
    $secondIndex=(Read-Titles)['第二个入口']
    $null = Invoke-Zt @(@{ tool='create_successor'; arguments=@{ at=$secondIndex; type='hub'; level='register'; title='索引：第二个入口'; body='范围：入口二。' } })
    $beforePack=(Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}}))[0].archive_revision
    $g4=Run-Digest @('group-start','-Out',$out,'-Group','4','-Token',"$($claims[4].token)")
    $pack4=Read-Pack $g4.earlier_context_path
    Assert ($pack4.register.status -eq 'ambiguous' -and @($pack4.faults | Where-Object code -eq 'register_unavailable').Count -eq 1) 'a duplicated register is reported as a fault and the pack still builds'
    Assert ($pack4.count -ge 3 -and $pack4.count -lt 10 -and -not $pack4.enough -and $pack4.sufficiency.detail -match 'sparse') 'a sparse earlier context is reported honestly and the quota is not padded'
    Assert (@($pack4.candidates | Where-Object { $_.address -in $anchors }).Count -eq 0) 'the sparse pack still refuses the unfinished placeholders'
    Assert (@($pack4.candidates | Where-Object { $_.title -like '剩余控制权*' }).Count -eq 0) 'an unrelated earlier Card never fills the sparse pack'
    $truncated=@($pack4.candidates | Where-Object { $_.truncated })
    Assert ($truncated.Count -ge 1 -and $truncated[0].excerpt_chars -eq 800 -and $truncated[0].text_chars -gt 800 -and $truncated[0].locator.arguments.at -eq $truncated[0].address) 'a shortened excerpt keeps its locator, its length and its truncation mark'
    $part=@($pack4.candidates | Where-Object { $_.type -eq 'model' -and $_.facet -eq '假设' })
    Assert ($part.Count -ge 1) 'an earlier Model part is offered beside regular Cards'
    $text4=[IO.File]::ReadAllText($g4.earlier_context_text)
    Assert ($text4.Contains('model·假设')) 'the rendered pack marks the Model part'
    Assert ($text4.Contains('前 800/')) 'the rendered pack marks the shortened excerpt'
    Assert ($pack4.archive.revision -eq $g4.snapshot_revision) 'the sparse pack binds its own startup revision'
    $archiveAfter=(Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}}))[0]
    Assert ($archiveAfter.archive_revision -eq $beforePack -and $pack4.archive.revision -eq $beforePack) 'publishing the four packs wrote no Card'
} finally {
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_TRACE_TOOLS,Env:ZT_FAIL_EXPORT_TITLES,Env:ZT_BULK_LIMIT,Env:ZT_BULK_MISSING,Env:ZT_DRIFT_REVISION,Env:ZT_DRIFT_MIN_ADDRESSES,Env:ZT_DRIFT_AFTER,Env:EARLIER_HUMAN_BODY,Env:EDITOR -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'

