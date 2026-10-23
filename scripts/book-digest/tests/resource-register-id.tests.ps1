#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
# R322/R325: the manager that ran a DeepSeek job records its id itself, with plain arguments
# (`ledger resource -RegisterId`), and later names a file that holds the job's answer
# (`-EvidenceFor -Evidence`). No agent writes a JSON record that restates what another agent saw.
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "resource-register-id-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest; $env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments,[int]$ExpectedExit = 0) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE -ne $ExpectedExit) { throw "digest failed ($LASTEXITCODE expected $ExpectedExit): $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
# The record commands answer one short line (`record_brief`); the recorded state is read back with the listing.
function View([string]$Path) {
    return @((Run-Digest @('ledger','resource','-Out',$out,'-List')).resources | Where-Object { $_.workspace.path -eq $Path })[0]
}
function Fail-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    return [pscustomobject]@{ code = $LASTEXITCODE; text = ($lines -join ' ') }
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source,"# 导论`n`n融资约束影响生产选择。`n`n# 第二章`n`n资本投入影响产量。`n`n# 参考`n未选择。",$utf8)
    $options = Join-Path $scratch 'run-options.json'
    [IO.File]::WriteAllText($options,'{"terms":false,"models":false,"events":false,"findings":false,"hubs":false,"figures":false,"bridge":false}',$utf8)
    $out = Join-Path $scratch 'prepared'
    $null = Run-Digest @('prepare','-Source',$source,'-CitationKey','IdBook','-BibTeX','@book{IdBook,title={Id book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-MaxUnits','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply','-MaxAttempts','1','-Out',$out)
    $unit = @((Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','block-manager')).units)[0]
    $workspace = Join-Path $out 'group-jobs'
    New-Item -ItemType Directory -Force $workspace | Out-Null

    # The id alone, at creation: zt-digest builds the record from plain arguments.
    $first = Run-Digest @('ledger','resource','-Out',$out,'-Unit',$unit.id,'-Token',"$($unit.token)",'-RegisterId','conv-a','-Workspace',$workspace)
    Assert ($first.recorded -eq $true -and -not $first.ContainsKey('resources')) 'a record command answers one short line, never the resource views'
    $resource = View $workspace
    Assert ($first.cleanup_unit -eq "cleanup/digest-g001-1-t$($unit.token)@1" -and @($resource.conversations).Count -eq 1 -and $resource.conversations[0].id -eq 'conv-a' -and $null -eq $resource.conversations[0].evidence) 'a manager registers a created id with plain arguments, before any answer exists'
    $again = Run-Digest @('ledger','resource','-Out',$out,'-Unit',$unit.id,'-Token',"$($unit.token)",'-RegisterId','conv-b','-Workspace',$workspace)
    $resource = View $workspace
    Assert (@($resource.conversations).Count -eq 2) 'a second id of the same workspace joins the same resource'
    $repeat = Run-Digest @('ledger','resource','-Out',$out,'-Unit',$unit.id,'-Token',"$($unit.token)",'-RegisterId','conv-a','-Workspace',$workspace)
    Assert (@((View $workspace).conversations).Count -eq 2) 'registering a known id again changes nothing'

    # The answer as a file the manager names: zt-digest copies it into the retained evidence.
    $answer = Join-Path $workspace 'answer-conv-a.txt'
    [IO.File]::WriteAllText($answer,'{"group":1,"cards":3,"errors":"无"}',$utf8)
    $evidence = Run-Digest @('ledger','resource','-Out',$out,'-EvidenceFor','conv-a','-Evidence',$answer,'-Usage','{"input_tokens":1200,"output_tokens":300}')
    $conv = @((View $workspace).conversations | Where-Object { $_.id -eq 'conv-a' })[0]
    Assert ($conv.evidence.answer.sha256.Length -eq 64 -and (Test-Path -LiteralPath $conv.evidence.answer.path)) 'the named answer file becomes retained evidence with its hash'
    Assert ([IO.File]::ReadAllText($conv.evidence.answer.path) -eq '{"group":1,"cards":3,"errors":"无"}') 'the retained evidence holds the answer text unchanged'
    Assert ($conv.usage.total.input_tokens -eq 1200 -and $conv.usage.total.output_tokens -eq 300) 'the usage numbers the manager read are recorded for that conversation'
    # Evidence without usage records the usage as unavailable (R345), so cleanup-finish has a retained record
    # (grossman2002integration trial 10: no manager passed -Usage and every cleanup unit was refused).
    $answerB = Join-Path $workspace 'answer-conv-b.txt'
    [IO.File]::WriteAllText($answerB,'{"group":1,"cards":2}',$utf8)
    $null = Run-Digest @('ledger','resource','-Out',$out,'-EvidenceFor','conv-b','-Evidence',$answerB)
    $convB = @((View $workspace).conversations | Where-Object { $_.id -eq 'conv-b' })[0]
    Assert ($convB.evidence.answer.sha256.Length -eq 64 -and $convB.usage.unavailable -match 'no token usage') 'evidence without usage records the usage as unavailable'
    $null = Run-Digest @('ledger','resource','-Out',$out,'-EvidenceFor','conv-a','-Evidence',$answer)
    $conv = @((View $workspace).conversations | Where-Object { $_.id -eq 'conv-a' })[0]
    Assert ($conv.usage.total.input_tokens -eq 1200) 'later evidence without usage keeps a recorded usage reading'

    # Refusals: an unknown id, a file outside the run, a superseded token.
    $bad = Fail-Digest @('ledger','resource','-Out',$out,'-EvidenceFor','conv-zzz','-Evidence',$answer)
    Assert ($bad.code -eq 1 -and $bad.text.Contains('conv-zzz')) 'evidence for an id no resource recorded is refused'
    $outside = Join-Path $scratch 'outside.txt'
    [IO.File]::WriteAllText($outside,'x',$utf8)
    $bad = Fail-Digest @('ledger','resource','-Out',$out,'-EvidenceFor','conv-b','-Evidence',$outside)
    Assert ($bad.code -eq 1 -and $bad.text.Contains('inside the prepared output directory')) 'an evidence file outside the run is refused'
    $bad = Fail-Digest @('ledger','resource','-Out',$out,'-Unit',$unit.id,'-Token',"$($unit.token + 1)",'-RegisterId','conv-c','-Workspace',$workspace)
    Assert ($bad.code -eq 1 -and $bad.text.Contains('stale_claim')) 'a superseded token cannot register an id'
    # ebrill2001modern trial 1: an explain manager "checked" the command with -RegisterId PENDING before any run
    # existed, and the cleanup verifier refused the placeholder as a conversation without evidence.
    foreach ($placeholder in @('PENDING','placeholder','<session_id>','TBD','conv a')) {
        $bad = Fail-Digest @('ledger','resource','-Out',$out,'-Unit',$unit.id,'-Token',"$($unit.token)",'-RegisterId',$placeholder,'-Workspace',$workspace)
        Assert ($bad.code -eq 1 -and $bad.text.Contains('not a conversation id')) "a placeholder id ($placeholder) is refused"
    }
    Assert (@((View $workspace).conversations).Count -eq 2) 'a refused placeholder records nothing'
    # The cleanup agent records what it did itself, with plain arguments, right after it acted.
    $settled = Run-Digest @('ledger','resource','-Out',$out,'-Settle','conv-a','-State','finished','-Deleted','confirmed')
    $conv = @((View $workspace).conversations | Where-Object { $_.id -eq 'conv-a' })[0]
    Assert ($conv.terminal -and $conv.deleted -eq $true -and $conv.delete -eq 'confirmed') 'a deleted conversation is recorded terminal and deleted by one plain command'
    $failed = Run-Digest @('ledger','resource','-Out',$out,'-Settle','conv-b','-State','finished','-Deleted','failed','-Message','open in another window')
    $conv = @((View $workspace).conversations | Where-Object { $_.id -eq 'conv-b' })[0]
    Assert ($conv.terminal -and $conv.deleted -eq $false -and $conv.delete -eq 'failed') 'a refused deletion is recorded as failed, not as deleted'
    $bad = Fail-Digest @('ledger','resource','-Out',$out,'-Settle','conv-a','-State','finished','-Deleted','maybe')
    Assert ($bad.code -ne 0 -and $bad.text.Contains('-Deleted')) 'an unknown deletion result is refused'
    $ws = Run-Digest @('ledger','resource','-Out',$out,'-SettleWorkspace',$workspace,'-Removed','true','-Remaining','conv-b','-Unknown','stray-1')
    Assert ($ws.recorded -eq $true) 'the workspace record answers one short line'
    $res = View $workspace
    Assert ($res.workspace_removed -eq $true -and @($res.listing.conversations).Count -eq 1 -and $res.listing.conversations[0].id -eq 'conv-b' -and $res.unknown[0].id -eq 'stray-1') 'the workspace listing and its removal are recorded by one plain command'
    $bad = Fail-Digest @('ledger','resource','-Out',$out,'-SettleWorkspace',(Join-Path $out 'nowhere'),'-Removed','true')
    Assert ($bad.code -eq 1 -and $bad.text.Contains('no recorded resource')) 'a workspace no resource recorded is refused'
    # ebrill2001modern trial 1: the cleanup stage sent its faults with an empty input_hash and `ledger fail` refused
    # them, so the failed cleanup units stayed claimed. The token fences the attempt; the ledger names the hash.
    $hash = @((Run-Digest @('ledger','status','-Out',$out,'-Layer','digest')).units | Where-Object id -EQ $unit.id)[0].input_hash
    $faulted = Run-Digest @('ledger','fail','-Out',$out,'-Unit',$unit.id,'-Token',"$($unit.token)",'-Class','retryable','-Fault','{"schema_version":1,"unit":"digest/g001@1","input_hash":"","code":"provider_transient","cause":"a fault without its hash"}')
    Assert ($faulted.fault.input_hash -eq $hash -and $faulted.state -in @('retry','failed')) 'a fault without its input hash takes the claim''s own hash'
    Write-Output 'ok: id registration and file evidence verified'
} finally {
    zt down 2>&1 | Out-Null
    if (Test-Path $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue }
}
