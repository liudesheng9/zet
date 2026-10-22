#Requires -Version 7.5
param([string]$Zt = '')
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "bridge-compiler-test-$PID"
$archive = Join-Path $scratch 'archive'
$staging = Join-Path $scratch 'staging'
New-Item -ItemType Directory -Force -Path $staging | Out-Null
if ($Zt) {
    $bin=Join-Path $scratch 'bin';New-Item -ItemType Directory -Force $bin|Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH="$bin;$env:PATH"
}
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$compiler = Join-Path $PSScriptRoot '..\bridge-compiler.ps1'
# zt-digest starts `<ZT_CHECK_ZT> mcp`: the seam forwards to zt mcp and injects a fault or a race.
$seam = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
function Assert($condition, [string]$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Compile($arguments) {
    $lines = @(& pwsh -NoProfile -File $compiler @arguments 2>&1 | ForEach-Object { [string]$_ })
    $script:CompilerExit = $LASTEXITCODE
    try { $result = $lines[-1] | ConvertFrom-Json -AsHashtable -DateKind String } catch { throw "compiler returned no final JSON: $($lines -join ' | ')" }
    $result
}
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $made = Write-Cards @(
        @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{New2026,title={New Book},year={2026}}' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'New2026'; title = '第一组'; body = '§1 控制权' } },
        @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{Old2020,title={Old Book},year={2020}}' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Old2020'; title = '第一组'; body = '§1 控制权' } },
        # docs/literature-only-digest-design.md: Bridge sources, targets and carriers are Literature-tree
        # Cards, so the source is in the book's own tree and the target in the earlier book's tree.
        @{ tool = 'create_successor'; arguments = @{ at = 'New2026/1'; kind = 'side'; title = '控制权决定债权人的议价能力'; body = "债权人掌握控制权后决定清算。控制权决定事后议价。`n`n来源：[[New2026]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Old2020/1'; kind = 'side'; title = '控制权决定企业的议价能力'; body = "企业掌握控制权后决定投资。控制权决定事后议价。`n`n来源：[[Old2020]] §1" } }
    )
    $source = $made[4]; $target = $made[5]
    $before = @(Invoke-Zt @(@{ tool='get_card'; arguments=@{at=$source} }, @{tool='get_card';arguments=@{at=$target}}))
    $result = Compile @('-Step','plan','-Staging',$staging,'-CitationKey','New2026','-MaxSources',1,'-Candidates',1)
    Assert ($CompilerExit -eq 0 -and $result.ok -and @($result.jobs).Count -eq 1) "an existing book produces one bounded candidate job without old section files ($($result | ConvertTo-Json -Depth 5 -Compress))"
    $plan = Get-Content -LiteralPath (Join-Path $staging 'bridge\plan.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    Assert ($plan.sources[0].address -ceq $source -and $plan.sources[0].candidates[0].address -ceq $target -and $plan.cards[$source].fingerprint -match '^sha256:[a-f0-9]{64}$') 'the plan freezes the selected source and eligible earlier-book target'
    $after = @(Invoke-Zt @(@{ tool='get_card'; arguments=@{at=$source} }, @{tool='get_card';arguments=@{at=$target}}))
    Assert ($before[0].body -ceq $after[0].body -and $before[1].body -ceq $after[1].body) 'planning changes no Card body'

    $termCards = Write-Cards @(
        @{tool='create_successor'; arguments=@{at='New2026/1';kind='side';title='术语表：本书';body="术语表`n`n| 术语 | 不使用的词 |`n| --- | --- |`n| 事后议价 | 事后谈判 |`n| 不动点定理 | 不动点法则 |"}},
        @{tool='create_successor'; arguments=@{at='Old2020/1';kind='side';title='事后议价压低专用性投资';body="专用性投资在事后被敲竹杠。投资方预期这一点，于是少投资。`n`n来源：[[Old2020]] §2"}},
        @{tool='create_successor'; arguments=@{at='Old2020/1';kind='side';title='不动点定理保证均衡存在';body="连续映射有不动点。均衡可以由不动点构造。`n`n来源：[[Old2020]] §3"}}
    )
    $termWork = Join-Path $staging 'term-bridge'
    $result = Compile @('-Step','plan','-Staging',$staging,'-Work',$termWork,'-CitationKey','New2026','-MaxSources',1,'-Candidates',1)
    $termPlan = Get-Content -LiteralPath (Join-Path $termWork 'plan.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    Assert (@($termPlan.sources[0].candidates | Where-Object { $_.address -ceq $termCards[1] -and 'term' -cin $_.via }).Count -eq 1) 'a book term in the source adds a target beyond the similarity candidate'
    Assert (-not @($termPlan.sources[0].candidates | Where-Object { $_.address -ceq $termCards[2] }).Count) 'a book term that the source does not use offers no target (term matches are per source)'
    $termAnswer=@{schema=1;plan_id=$termPlan.plan_id;job='br-001';sources=@(@{source=$source;none='';bridges=@(
        @{carrier=$source;target=$target;word='对应';sentence="对应 [[$target]]：两处都由控制权决定事后议价。";why='两张卡讨论相同的控制权机制。'},
        @{carrier=$source;target=$termCards[1];word='对照';sentence="对照 [[$($termCards[1])]]：本卡讨论控制权的配置，那张卡讨论事后议价对投资的影响。";why='一个讨论控制权配置，一个讨论事后议价的投资后果。'}
    )})}
    [IO.File]::WriteAllText((Join-Path $termWork 'out\br-001.json'),(ConvertTo-Json -InputObject $termAnswer -Depth 20),[Text.UTF8Encoding]::new($false))
    $termReport=Compile @('-Step','report','-Staging',$staging,'-Work',$termWork,'-CitationKey','New2026')
    Assert ($termReport.ok -and $termReport.counts.proposals -eq 2) 'one assigned source can publish two individually checked proposals'

    $sentence = "对应 [[$target]]：两处都由控制权决定事后议价。"
    $answer = @{schema=1;plan_id=$plan.plan_id;job='br-001';sources=@(@{source=$source;bridges=@(@{carrier=$source;target=$target;word='对应';sentence=$sentence;why='两张卡片讨论同一控制权机制。'});none=''})}
    $answerFile = Join-Path $staging 'bridge\out\br-001.json'
    [IO.File]::WriteAllText($answerFile, (ConvertTo-Json -InputObject $answer -Depth 15), [Text.UTF8Encoding]::new($false))
    $result = Compile @('-Step','precheck','-Staging',$staging,'-CitationKey','New2026','-Job','br-001')
    Assert ($CompilerExit -eq 0 -and $result.faults -eq 0 -and $result.ok) "a valid frozen source-candidate answer passes the public precheck ($($result | ConvertTo-Json -Compress))"
    $resume=Compile @('-Step','plan','-Staging',$staging,'-CitationKey','New2026','-MaxSources',1,'-Candidates',1)
    Assert ($resume.plan_id -ceq $plan.plan_id -and @($resume.jobs).Count -eq 0) 'resuming a checked judge answer retains its snapshot and dispatches no completed job'

    $result = Compile @('-Step','report','-Staging',$staging,'-CitationKey','New2026')
    Assert ($CompilerExit -eq 0 -and $result.ok -and $result.report_id -match '^r-[a-f0-9]{64}$' -and $result.counts.proposals -eq 1) "a checked answer becomes an identified read-only review report ($($result | ConvertTo-Json -Compress))"
    $publishedFile = Join-Path $staging 'bridge\proposals.json'
    $published = Get-Content -LiteralPath $publishedFile -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    Assert ($published.proposals[0].id -ceq 'b001' -and $published.proposals[0].sentence -ceq $sentence -and $published.cards[$target].body -ceq $before[1].body) 'published review data retains exact text and self-contained endpoint snapshots'

    $publishedHash = (Get-FileHash -LiteralPath $publishedFile -Algorithm SHA256).Hash
    $goodAnswer = ConvertTo-Json -InputObject $answer -Depth 15 -Compress
    $invalidCases = @(
        @{name='unplanned target';change={param($a) $a.sources[0].bridges[0].target='missing/1'}},
        @{name='unknown kind';change={param($a) $a.sources[0].bridges[0].word='引出'}},
        @{name='two Links';change={param($a) $a.sources[0].bridges[0].sentence += " 对应 [[$target]]：这句话重复同一关系。"}},
        @{name='third Bridge';change={param($a) $a.sources[0].bridges += @($a.sources[0].bridges[0],$a.sources[0].bridges[0])}},
        @{name='missing source';change={param($a) $a.sources=@()}},
        @{name='reason too short';change={param($a) $a.sources[0].bridges[0].sentence="对应 [[$target]]。"}},
        @{name='multiline sentence';change={param($a) $a.sources[0].bridges[0].sentence += "`n另一行。"}},
        @{name='wrong plan';change={param($a) $a.plan_id='p-wrong'}}
    )
    foreach ($case in $invalidCases) {
        $invalid = $goodAnswer | ConvertFrom-Json -AsHashtable -DateKind String
        & $case.change $invalid
        [IO.File]::WriteAllText($answerFile,(ConvertTo-Json -InputObject $invalid -Depth 15),[Text.UTF8Encoding]::new($false))
        $result=Compile @('-Step','precheck','-Staging',$staging,'-CitationKey','New2026','-Job','br-001')
        Assert ($CompilerExit -eq 1 -and -not $result.ok -and $result.faults -gt 0) "precheck refuses $($case.name)"
    }
    $rerender = Compile @('-Step','report','-Staging',$staging,'-CitationKey','New2026')
    Assert ($rerender.report_id -ceq $published.report_id -and (Get-FileHash -LiteralPath $publishedFile -Algorithm SHA256).Hash -ceq $publishedHash) 'late job edits cannot replace published sentences or report identity'
    [IO.File]::WriteAllText($answerFile,'{not-json',[Text.UTF8Encoding]::new($false))
    $result=Compile @('-Step','precheck','-Staging',$staging,'-CitationKey','New2026','-Job','br-001')
    Assert ($CompilerExit -eq 1 -and -not $result.ok -and $result.faults -gt 0) 'malformed judge JSON is an input refusal rather than an operational failure'
    [IO.File]::WriteAllText($answerFile,$goodAnswer,[Text.UTF8Encoding]::new($false))

    Write-Cards @(@{tool='edit_card';arguments=@{at=$target;title='并行阶段修改了旧卡标题'}})|Out-Null
    $changed=Compile @('-Step','apply','-Staging',$staging,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001')
    Assert (-not $changed.ok -and $changed.outcomes[0].status -ceq 'stale') 'a changed target title is stale even when its body is unchanged'
    Write-Cards @(@{tool='edit_card';arguments=@{at=$target;title=$before[1].title}},@{tool='create_successor';arguments=@{at=$target;kind='side';title='另一张卡引用旧卡';body="来源：[[$target]]"}})|Out-Null
    $reverseOnly=Compile @('-Step','apply','-Staging',$staging,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001')
    Assert ($reverseOnly.ok -and $reverseOnly.outcomes[0].status -ceq 'ready') 'generated Reverse-link and inbound-count churn does not invalidate a reviewed endpoint'
    $linkedWork=Join-Path $staging 'externally-linked';New-Item -ItemType Directory -Force $linkedWork,(Join-Path $linkedWork 'tmp')|Out-Null
    Copy-Item -LiteralPath $publishedFile -Destination (Join-Path $linkedWork 'proposals.json')
    $externalBody=$before[0].body+"`n`n对照 [[$target]]：这是另一位作者留下的独立说明。"
    Write-Cards @(@{tool='edit_card';arguments=@{at=$source;body=$externalBody}})|Out-Null
    $linked=Compile @('-Step','apply','-Staging',$staging,'-Work',$linkedWork,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001','-Apply')
    $preserved=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$source}}))[0]
    Assert ($linked.ok -and $linked.outcomes[0].status -ceq 'already_linked' -and $preserved.body -ceq $externalBody -and -not $linked.changed_carriers.Count) 'an external existing Link preserves its prose and is not claimed as an applied explanation'
    Write-Cards @(@{tool='edit_card';arguments=@{at=$source;body=$before[0].body}})|Out-Null
    $preview = Compile @('-Step','apply','-Staging',$staging,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001')
    Assert ($CompilerExit -eq 0 -and $preview.ok -and $preview.preview -and $preview.outcomes[0].status -ceq 'ready' -and -not (Test-Path -LiteralPath (Join-Path $staging 'bridge\state.json'))) "an identified accepted selection can be previewed without creating apply state ($($preview | ConvertTo-Json -Depth 5 -Compress))"
    foreach($bad in @(@{report='r-other';ids='b001'},@{report=$published.report_id;ids='b999'},@{report=$published.report_id;ids='b001,b001'},@{report=$published.report_id;ids='true'})){
        $denied=Compile @('-Step','apply','-Staging',$staging,'-CitationKey','New2026','-ReportId',$bad.report,'-Accept',$bad.ids,'-Apply')
        Assert ($CompilerExit -eq 1 -and -not $denied.ok -and -not (Test-Path -LiteralPath (Join-Path $staging 'bridge\state.json'))) "invalid report or selection $($bad.ids) creates no apply journal"
    }
    $held=[IO.File]::Open((Join-Path $staging 'bridge\apply.lock'),[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
    try{$contended=Compile @('-Step','apply','-Staging',$staging,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001','-Apply')
        Assert ($CompilerExit -eq 1 -and -not $contended.ok -and -not(Test-Path -LiteralPath (Join-Path $staging 'bridge\state.json'))) 'a second process cannot apply while the report lock handle is held'
    }finally{$held.Dispose()}

    $applied = Compile @('-Step','apply','-Staging',$staging,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001','-Apply')
    Assert ($CompilerExit -eq 0 -and $applied.ok -and $applied.outcomes[0].status -ceq 'verified') "explicit acceptance writes and verifies the selected agent carrier ($($applied | ConvertTo-Json -Depth 5 -Compress))"
    $actual = @(Invoke-Zt @(@{tool='get_card';arguments=@{at=$source}},@{tool='get_card';arguments=@{at=$target}}))
    Assert ($actual[0].body.Contains("跨书关联：$sentence") -and $actual[1].body -ceq $before[1].body -and @($actual[1].inbound | Where-Object address -CEQ $source).Count -eq 1) 'only the accepted carrier body changes and the earlier target gains a generated Reverse link'
    $stateFile = Join-Path $staging 'bridge\state.json'
    $state = Get-Content -LiteralPath $stateFile -Raw | ConvertFrom-Json -AsHashtable -DateKind String
    # Reconstruct the durable state of a process that saved intent but did not confirm its edit.
    Write-Cards @(@{tool='edit_card';arguments=@{at=$source;body=$before[0].body}}) | Out-Null
    $state.attempts[0].status='written_unverified';$state.outcomes.b001.status='written_unverified'
    [IO.File]::WriteAllText($stateFile,(ConvertTo-Json -InputObject $state -Depth 30),[Text.UTF8Encoding]::new($false))
    $retry = Compile @('-Step','apply','-Staging',$staging,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001','-Apply')
    Assert ($retry.ok -and $retry.outcomes[0].status -ceq 'verified') 'an unconfirmed edit still at its expected body retries its saved intent'
    $current = (Invoke-Zt @(@{tool='get_card';arguments=@{at=$source}}))[0]
    $state = Get-Content -LiteralPath $stateFile -Raw | ConvertFrom-Json -AsHashtable -DateKind String
    $state.attempts[0].status='prepared';$state.outcomes.b001.status='prepared'
    [IO.File]::WriteAllText($stateFile,(ConvertTo-Json -InputObject $state -Depth 30),[Text.UTF8Encoding]::new($false))
    $recovered = Compile @('-Step','apply','-Staging',$staging,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001','-Apply')
    $repeated = Compile @('-Step','apply','-Staging',$staging,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001','-Apply')
    $final = (Invoke-Zt @(@{tool='get_card';arguments=@{at=$source}}))[0]
    Assert ($recovered.ok -and $repeated.ok -and $final.text -ceq $current.text -and [regex]::Matches($final.body,[regex]::Escape($sentence)).Count -eq 1) 'recovery after persistence and repeated accepted apply append no duplicate'

    Write-Cards @(@{tool='edit_card';arguments=@{at=$source;body=$before[0].body}})|Out-Null
    $first=Compile @('-Step','apply','-Staging',$staging,'-Work',$termWork,'-CitationKey','New2026','-ReportId',$termReport.report_id,'-Accept','b001','-Apply')
    $second=Compile @('-Step','apply','-Staging',$staging,'-Work',$termWork,'-CitationKey','New2026','-ReportId',$termReport.report_id,'-Accept','b002','-Apply')
    $again=Compile @('-Step','apply','-Staging',$staging,'-Work',$termWork,'-CitationKey','New2026','-ReportId',$termReport.report_id,'-Accept','all','-Apply')
    $allBody=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$source}}))[0].body
    Assert ($first.ok -and $second.ok -and $again.ok -and @($again.outcomes|Where-Object status -CEQ 'verified').Count -eq 2 -and [regex]::Matches($allBody,'跨书关联：').Count -eq 2) 'additional accepted ids use journal-proven carrier state and all remains idempotent'

    # A confirmed write followed by a failed verification read must remain visible.
    Write-Cards @(@{tool='edit_card';arguments=@{at=$source;body=$before[0].body}})|Out-Null
    # zt-digest reads and writes in-process: the seam stands in for its zt mcp (ZT_CHECK_ZT).
    $env:ZT_CHECK_ZT=$seam;$env:ZT_FAIL_AFTER_EDIT=$source;$env:ZT_FAIL_CARD=$target;$env:ZT_FAIL_FLAG=Join-Path $scratch 'written.flag'
    $faultWork=Join-Path $staging 'written-review';New-Item -ItemType Directory -Force $faultWork,(Join-Path $faultWork 'tmp')|Out-Null
    Copy-Item -LiteralPath $publishedFile -Destination (Join-Path $faultWork 'proposals.json')
    $unverified=Compile @('-Step','apply','-Staging',$staging,'-Work',$faultWork,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001','-Apply')
    $written=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$source}}))[0]
    Assert (-not $unverified.ok -and $unverified.outcomes[0].status -ceq 'written_unverified' -and $source -cin $unverified.changed_carriers -and $written.body.Contains("跨书关联：$sentence")) 'failed post-write verification names the persisted carrier and retains written_unverified'
    Remove-Item -LiteralPath $env:ZT_FAIL_FLAG
    $reverified=Compile @('-Step','apply','-Staging',$staging,'-Work',$faultWork,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001','-Apply')
    $reread=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$source}}))[0]
    Assert ($reverified.ok -and $reverified.outcomes[0].status -ceq 'verified' -and $reread.body -ceq $written.body) 'a later invocation completes verification without another append'
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_FAIL_AFTER_EDIT,Env:ZT_FAIL_CARD,Env:ZT_FAIL_FLAG

    # The zt mcp seam injects a real concurrent edit after the compiler read.
    Write-Cards @(@{tool='edit_card';arguments=@{at=$source;body=$before[0].body}})|Out-Null
    $env:ZT_CHECK_ZT=$seam;$env:ZT_RACE_TARGET=$source;$env:ZT_RACE_FLAG=Join-Path $scratch 'raced';$env:ZT_RACE_LINE='并行编辑保留这句话。'
    $raceWork=Join-Path $staging 'race-review';New-Item -ItemType Directory -Force $raceWork,(Join-Path $raceWork 'tmp')|Out-Null
    Copy-Item -LiteralPath $publishedFile -Destination (Join-Path $raceWork 'proposals.json')
    $raced=Compile @('-Step','apply','-Staging',$staging,'-Work',$raceWork,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001','-Apply')
    $raceBody=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$source}}))[0].body
    Assert (-not $raced.ok -and $raced.outcomes[0].status -ceq 'conflict' -and $raceBody.Contains($env:ZT_RACE_LINE) -and -not $raceBody.Contains('跨书关联：')) 'a raw-base race preserves concurrent prose and reports conflict without an overwrite'
    $stale=Compile @('-Step','apply','-Staging',$staging,'-Work',$raceWork,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001','-Apply')
    Assert (-not $stale.ok -and $stale.outcomes[0].status -cin @('stale','conflict')) 'retry never adopts unreviewed concurrent prose as a fresh base'
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_RACE_TARGET,Env:ZT_RACE_FLAG,Env:ZT_RACE_LINE
    # A Literature Card cannot leave its tree (docs/literature-only-digest-design.md): move it within the book's tree.
    Write-Cards @(@{tool='move_card';arguments=@{at=$source;to='New2026/1|c'}},@{tool='move_card';arguments=@{at=$source;to='New2026/1|c';confirm='move'}})|Out-Null
    $movedWork=Join-Path $staging 'moved-review';New-Item -ItemType Directory -Force $movedWork,(Join-Path $movedWork 'tmp')|Out-Null
    Copy-Item -LiteralPath $publishedFile -Destination (Join-Path $movedWork 'proposals.json')
    $moved=Compile @('-Step','apply','-Staging',$staging,'-Work',$movedWork,'-CitationKey','New2026','-ReportId',$published.report_id,'-Accept','b001')
    Assert (-not $moved.ok -and $moved.outcomes[0].status -ceq 'stale' -and $moved.outcomes[0].detail -match 'missing endpoint') 'a moved source is refused at its accepted address without title relocation'
} finally {
    zt down | Out-Null
}
