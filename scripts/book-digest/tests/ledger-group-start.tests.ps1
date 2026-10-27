#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-group-start-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
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
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $source=Join-Path $scratch 'book.md'; $options=Join-Path $scratch 'options.json'
    [IO.File]::WriteAllText($source,"# 导论`n`n企业的融资约束影响生产选择。`n`n# 参考`n未选择。",$utf8)
    [IO.File]::WriteAllText($options,'{"terms":false}',$utf8)
    $manifest=Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','StartBook','-BibTeX','@book{StartBook,title={Start book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    $preparationGuards=@(Get-ChildItem -LiteralPath (Join-Path $out 'locks') -Filter '*.lock' | ForEach-Object FullName)
    $next=Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','group-manager')
    $claim=$next.units[0]
    $shortLease=@'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-group-start-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.prepare("update units set lease_until=? where id=? and claim_token=? and state='claimed'").run(Math.floor(Date.now()/1000)+30,process.argv[2],Number(process.argv[3]));db.close();
'@
    & node --no-warnings -e $shortLease (Join-Path $out 'ledger.sqlite3') $claim.id "$($claim.token)"
    if ($LASTEXITCODE) { throw 'short lease fixture failed' }
    $env:ZT_CHECK_ZT=Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'; $env:ZT_FAIL_EXPORT_TITLES='1'
    $failed=@(& $Digest group-start -Out $out -Group 1 -Token $claim.token 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($failed -join ' ').Contains('unavailable title export')) 'a failed fresh snapshot refuses a usable group job'
    Remove-Item Env:ZT_FAIL_EXPORT_TITLES
    $stillOpen=Run-Digest @('ledger','status','-Out',$out,'-Layer','digest')
    Assert ($stillOpen.units[0].state -eq 'claimed' -and -not $stillOpen.units[0].progress.group_start -and -not $stillOpen.complete) 'a skeleton anchor plus a failed snapshot cannot establish group coverage'
    # docs/literature-only-digest-design.md: the startup snapshot lists Literature trees only, so the post-preparation context is another book.
    $made=Invoke-Zt @(@{tool='create_literature';arguments=@{bibtex='@book{Later2021,title={启动时新增文献}}';body='这是准备完成之后才写入的上下文。'}})
    $env:ZT_TRACE_TOOLS=Join-Path $scratch 'trace.jsonl'
    $started=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claim.token)")
    Assert ($started.unit -eq $claim.id -and $started.token -eq $claim.token -and $started.input_hash -match '^[0-9a-f]{64}$' -and -not $started.reused) 'group-start binds the actual claim to a frozen job input hash'
    $active=Run-Digest @('ledger','status','-Out',$out,'-Layer','digest')
    Assert ($started.lease_until -ge [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()+110 -and $active.units[0].lease_until -eq $started.lease_until -and $active.units[0].attempts -eq 1 -and $active.units[0].state -eq 'claimed' -and -not $active.complete) 'startup renews a nearly expired claim without spending another attempt or completing the group'
    $job=Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($job.expected_jobs -eq 1 -and $job.jobs.Count -eq 1 -and $job.mode -eq 'draft_only' -and $job.jobs[0].output -eq $started.draft -and $job.jobs[0].permissions.archive_write -eq $false) 'the job manifest explicitly requests one draft file with no archive write permission'
    Assert ([IO.File]::ReadAllText($started.snapshot).Contains('启动时新增文献') -and [IO.File]::ReadAllText($started.snapshot).Contains('人类工作区（只读')) 'the startup snapshot includes post-preparation context and marks human Cards read-only'
    Assert ([IO.File]::ReadAllText($started.prompt).Contains('企业的融资约束影响生产选择') -and [IO.File]::ReadAllText($started.prompt).Contains($started.input_hash)) 'the prompt contains the selected source and exact draft binding'
    $trace=@(Get-Content $env:ZT_TRACE_TOOLS | ForEach-Object {$_ | ConvertFrom-Json -AsHashtable})
    Assert (@($trace | Where-Object tool -EQ export_titles).Count -eq 1 -and @($trace.process_id | Select-Object -Unique).Count -eq 1) 'one fresh title export shares the command MCP process'
    Remove-Item Env:ZT_TRACE_TOOLS
    # S28 (#186): the group task file carries the double SELF-CHECK — the Card check (zt-check over
    # the draft's preview Cards, Literature-note profile) and the coverage check. Both run on the
    # group's own draft before it is applied, and both answer fault lines then `faults: N`.
    $task=[IO.File]::ReadAllText($started.prompt)
    $selfChecks=@($task -split "`r?`n" | Where-Object { $_ -match '^SELF-CHECK: ' })
    Assert ($selfChecks.Count -eq 2 -and $selfChecks[0] -match '^SELF-CHECK: zt-digest group-finish .* -Group 1 -Token \d+ -Draft ".*draft\.json" -CheckOnly$' -and $selfChecks[1] -match '^SELF-CHECK: zt-digest coverage .* -Group 1 -Draft ".*draft\.json"$') "the group task file carries the double SELF-CHECK (the Card check and coverage) ($($selfChecks -join ' | '))"
    $section=[IO.File]::ReadAllLines((Join-Path $out 'sections\s01.md'))
    $unitLine=@(1..$section.Count | Where-Object { $section[$_-1] -match '企业的融资约束影响生产选择' })[0]
    $draft=@{ schema_version=1; unit=$claim.id; token=$claim.token; input_hash=$started.input_hash; disposition='cards'; calls=@(
        @{ tool='create_successor'; arguments=@{ at='StartBook/1'; kind='side'; title='融资约束改变企业的生产选择'; body="企业的融资约束影响生产选择。`n`n来源：[[StartBook]] §导论"; src="sections/s01.md:$unitLine-$unitLine" } },
        @{ tool='edit_card'; arguments=@{ at='StartBook/1'; title='融资约束与生产选择'; body="本部分说明融资约束如何改变生产选择。`n`n来源：[[StartBook]] 导论`n`n在全书中的位置：本书的引言部分。"; base_fingerprint=[string]$job.inputs.anchor.fingerprint } }
    ) }
    [IO.File]::WriteAllText($started.draft, ($draft | ConvertTo-Json -Depth 10), $utf8)
    $cardCheck=@(& $Digest group-finish -Out $out -Group 1 -Token $claim.token -Draft $started.draft -CheckOnly 2>&1)
    $covered=@(& $Digest coverage -Out $out -Group 1 -Draft $started.draft 2>&1)
    Assert ($cardCheck[-1] -eq 'faults: 0' -and $covered[-1] -eq 'faults: 0') "both SELF-CHECK commands pass on a draft that covers its unit ($($cardCheck[-1]) / $($covered[-1]))"
    $draft.calls[0].arguments.src='sections/s01.md'
    [IO.File]::WriteAllText($started.draft, ($draft | ConvertTo-Json -Depth 10), $utf8)
    $fileOnly=@(& $Digest coverage -Out $out -Group 1 -Draft $started.draft 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($fileOnly -join ' ') -match 'C2' -and $fileOnly[-1] -ne 'faults: 0') "a file-only span is a coverage fault the writer must fix ($($fileOnly[-1]))"
    $draft.calls[0].arguments.src="sections/s01.md:$unitLine-$unitLine"
    [IO.File]::WriteAllText($started.draft, ($draft | ConvertTo-Json -Depth 10), $utf8)
    $archiveAfter=Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})
    Assert ($archiveAfter.archive_revision -eq $started.snapshot_revision) 'startup publishes input evidence without changing Cards or their archive revision'
    $beforeHash=(Get-FileHash -LiteralPath $started.job -Algorithm SHA256).Hash
    $again=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claim.token)")
    Assert ($again.reused -and $again.input_hash -ceq $started.input_hash -and (Get-FileHash -LiteralPath $started.job -Algorithm SHA256).Hash -ceq $beforeHash) 'repeating the same accepted start retains its exact frozen files and inputs'
    $guardPaths=@(Get-ChildItem -LiteralPath (Join-Path $out 'locks') -Filter '*.lock' | Where-Object FullName -NotIn $preparationGuards)
    Assert ($guardPaths.Count -eq 1) 'the group publishes a persistent per-unit OS guard location'
    $held=[IO.File]::Open($guardPaths[0].FullName,[IO.FileMode]::Open,[IO.FileAccess]::ReadWrite,[IO.FileShare]::ReadWrite)
    try {
        $held.Lock(0,1)
        $busy=@(& $Digest group-start -Out $out -Group 1 -Token $claim.token 2>&1)
        Assert ($LASTEXITCODE -eq 1 -and ($busy -join ' ').Contains('lock_busy')) 'another process holding the OS guard prevents concurrent input publication'
        $stale=@(& $Digest group-start -Out $out -Group 1 -Token ($claim.token+1) 2>&1)
        Assert ($LASTEXITCODE -eq 1 -and ($stale -join ' ').Contains('stale_claim')) 'a stale token is diagnosed before an unrelated active guard can mask it'
        $fault=Join-Path $scratch 'busy-fault.json'
        [IO.File]::WriteAllText($fault,(@{code='provider_transient';unit=$claim.id;input_hash=$claim.input_hash;cause='fixture failure while writer is protected'} | ConvertTo-Json),$utf8)
        $busy=@(& $Digest ledger fail -Out $out -Unit $claim.id -Token $claim.token -Class retryable -Fault $fault 2>&1)
        Assert ($LASTEXITCODE -eq 1 -and ($busy -join ' ').Contains('lock_busy')) 'failure cannot revoke the protected unit while its OS guard is held'
    } finally { $held.Dispose() }
    $released=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claim.token)")
    Assert ($released.reused -and $released.input_hash -eq $started.input_hash) 'a released guard file is not mistaken for an active owner'
    $wrong=@(& $Digest group-start -Out $out -Group 1 -Token ($claim.token+1) 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($wrong -join ' ').Contains('stale_claim')) 'a stale startup token cannot replace the accepted job'
    [IO.File]::AppendAllText($started.prompt,"`n改变了任务。",$utf8)
    $changed=@(& $Digest group-start -Out $out -Group 1 -Token $claim.token 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($changed -join ' ').Contains('group job artifact hash mismatch')) 'an edited accepted prompt is refused rather than silently reinterpreted'
} finally {
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_TRACE_TOOLS,Env:ZT_FAIL_EXPORT_TITLES -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
