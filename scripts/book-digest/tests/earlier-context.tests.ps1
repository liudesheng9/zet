#Requires -Version 7.5
# S05 (#98): the bounded earlier-Card context pack of a digest group, at its public seams.
# A scratch archive with twenty-five earlier Topics and a three-group book proves the published pack,
# its frozen evidence, the bounded re-read of a reply-limit refusal, and the one-retry rule for a
# read set that crosses archive revisions.
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
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    # Twenty-five earlier Topics, so a full twenty-Card pack is reachable and the read set is large.
    $topics = 1..25 | ForEach-Object { @{ tool='create_topic'; arguments=@{ title="剩余控制权与谈判 $_"; body="剩余控制权决定第 $_ 种谈判地位。控制权分配影响谈判结果。" } } }
    $null = Invoke-Zt $topics
    $titles = Invoke-Zt @(@{ tool='export_titles'; arguments=@{ window_start=0 } })
    Assert (@($titles.titles).Count -eq 25) 'twenty-five earlier Topics exist before the first group starts'
    $source=Join-Path $scratch 'book.md'; $options=Join-Path $scratch 'options.json'
    [IO.File]::WriteAllText($source,"# 导论`n`n剩余控制权决定谈判地位。控制权分配影响谈判的结果。`n`n## 债务与控制权`n`n债务把控制权交给债权人。债权人取得剩余控制权。`n`n## 所有权与剩余控制权`n`n所有权决定剩余控制权的归属。所有者取得谈判地位。`n`n# 参考`n未选择。",$utf8)
    [IO.File]::WriteAllText($options,'{"terms":false}',$utf8)
    $manifest=Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','Earlier','-BibTeX','@book{Earlier,title={Earlier book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-MaxUnits','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    Assert (@($manifest.groups).Count -eq 3) 'the prepared book holds three groups'
    $anchors=@($manifest.groups | ForEach-Object { "Earlier/$($_.n)" })
    $next=Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','3','-Owner','group-manager')
    $claims=@{}
    foreach ($unit in @($next.units)) { $claims[[int]([string]$unit.id -replace '^digest/g(\d+)@1$','$1')] = $unit }
    Assert ($claims.Keys.Count -eq 3) 'three digest groups are claimed in one step'

    $env:ZT_CHECK_ZT=Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_TRACE_TOOLS=Join-Path $scratch 'trace-1.jsonl'
    $g1=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claims[1].token)")
    Remove-Item Env:ZT_TRACE_TOOLS
    Assert ($g1.earlier_context.candidates -eq 20 -and $g1.earlier_context.matches -ge 20) 'group-start publishes a full twenty-Card earlier context'
    Assert ($g1.earlier_context.register -eq 'absent' -and $g1.earlier_context.archive_revision -eq $g1.snapshot_revision) 'the pack reports a missing register and the startup revision it was read at'
    $pack=Read-Pack $g1.earlier_context_path
    Assert ($pack.schema_version -eq 1 -and $pack.unit -eq 'digest/g001@1' -and $pack.group -eq 1 -and $pack.count -eq 20) 'the pack names its schema, unit, group and count'
    Assert (@($pack.candidates | Where-Object { $_.excerpt_chars -gt 800 }).Count -eq 0 -and $pack.excerpt_chars -le 16000) 'every excerpt is at most 800 characters and the pack at most 16000'
    Assert (@($pack.candidates | Where-Object { $_.locator.tool -ne 'get_card' -or $_.locator.arguments.at -ne $_.address }).Count -eq 0) 'each candidate carries the locator that reads all of it'
    Assert (@($pack.candidates | Where-Object { $_.text_sha256 -notmatch '^[0-9a-f]{64}$' -or $_.text_chars -lt 1 }).Count -eq 0) 'each candidate retains its actual content hash and size'
    Assert (@($pack.candidates | Where-Object { $_.workspace -ne 'agent' -or $_.destination -ne 'agent-card' -or -not $_.writable }).Count -eq 0) 'every offered candidate is an agent Card that a Link may target'
    Assert (@($pack.candidates | Where-Object { $_.address -in $anchors }).Count -eq 0) 'no unfinished group placeholder is offered as earlier thought'
    Assert (@($pack.catalog.excluded | Where-Object { $_ -in $anchors }).Count -eq 3) 'the pack names every unfinished placeholder it refused'
    Assert (@($pack.candidates | Where-Object { -not $_.ContainsKey('source_evidence') -or $null -eq $_.source_evidence }).Count -eq 0) 'every candidate reports its source evidence, empty when the Card names none'
    $prompt=[IO.File]::ReadAllText($g1.prompt)
    Assert ($prompt.Contains($g1.earlier_context_text) -and $prompt.Contains('早前上下文包')) 'the group prompt names the earlier-context pack it must read'
    $job=Get-Content -LiteralPath $g1.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert (@($job.jobs[0].files) -contains $g1.earlier_context_text -and $job.artifacts.earlier_context.path -eq $g1.earlier_context_path) 'the writer job hands the pack to the run'
    Assert ($job.inputs.earlier_context.sha256 -eq (Get-FileHash -LiteralPath $g1.earlier_context_path -Algorithm SHA256).Hash.ToLowerInvariant()) 'the pack is frozen in the start inputs by hash'
    $text=[IO.File]::ReadAllText($g1.earlier_context_text)
    Assert ($text.Contains('断言同一主张') -and $text.Contains('读全文') -and $text.Contains('人类工作区的卡只读')) 'the pack states its read-before-asserting and read-only rules'
    $before=(Get-FileHash -LiteralPath $g1.earlier_context_path -Algorithm SHA256).Hash
    $again=Run-Digest @('group-start','-Out',$out,'-Group','1','-Token',"$($claims[1].token)")
    Assert ($again.reused -and (Get-FileHash -LiteralPath $again.earlier_context_path -Algorithm SHA256).Hash -eq $before) 'a resumed start reuses the frozen pack byte for byte'

    $env:ZT_TRACE_TOOLS=Join-Path $scratch 'trace-2.jsonl'; $env:ZT_BULK_LIMIT='5'
    $g2=Run-Digest @('group-start','-Out',$out,'-Group','2','-Token',"$($claims[2].token)")
    Remove-Item Env:ZT_BULK_LIMIT,Env:ZT_TRACE_TOOLS
    $reads=@(Get-Content -LiteralPath (Join-Path $scratch 'trace-2.jsonl') | ForEach-Object { $_ | ConvertFrom-Json -AsHashtable } | Where-Object { $_.tool -eq 'get_cards' -and $_.address_count -gt 0 })
    $pack2=Read-Pack $g2.earlier_context_path
    Assert ($reads.Count -ge 8 -and $pack2.count -eq 20) 'a reply-limit refusal halves the earlier-Card read and still yields the full pack'

    $env:ZT_DRIFT_MIN_ADDRESSES='10'; $env:ZT_DRIFT_REVISION='all'
    $drift=@(& $Digest group-start -Out $out -Group 3 -Token $claims[3].token 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($drift -join ' ').Contains('earlier-Card context')) 'a read set that keeps crossing revisions is one changing-view fault'
    $env:ZT_DRIFT_REVISION='once'
    $g3=Run-Digest @('group-start','-Out',$out,'-Group','3','-Token',"$($claims[3].token)")
    Remove-Item Env:ZT_DRIFT_MIN_ADDRESSES,Env:ZT_DRIFT_REVISION
    $pack3=Read-Pack $g3.earlier_context_path
    Assert ($pack3.archive.revision -eq $g3.snapshot_revision -and $pack3.count -gt 0) 'one crossed read set is retried and the pack comes from one view'
    Assert ($pack3.archive.identity.id -eq $pack.archive.identity.id) 'the pack binds the archive identity it was read from'

    $live=Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})
    Assert ($live.archive_revision -eq $g1.snapshot_revision) 'publishing three packs wrote no Card'
} finally {
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_TRACE_TOOLS,Env:ZT_FAIL_EXPORT_TITLES,Env:ZT_BULK_LIMIT,Env:ZT_DRIFT_REVISION,Env:ZT_DRIFT_MIN_ADDRESSES,Env:ZT_DRIFT_AFTER -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
