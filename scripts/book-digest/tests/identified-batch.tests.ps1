#Requires -Version 7.5
# Real helper/MCP boundary; a process dies after a committed chunk but before its reply.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "identified-batch-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the intended temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $calls = [Collections.Generic.List[object]]::new()
    $calls.Add(@{tool='create_topic';arguments=@{title='Identified model build'}})
    $calls.Add(@{tool='create_successor';arguments=@{at='@@0';title='A model';type='model';level='formal';body='Question and mechanism.'}})
    for ($i=2; $i -lt 68; $i++) {
        $previous = $i-1
        $calls.Add(@{tool='create_successor';arguments=@{at="@@$previous";kind=$(if ($i -eq 2) {'side'} else {'direct'});type='model';facet=$(if ($i -eq 2) {'设定'} else {"推导：步骤 $i"});title="Part $i";body="This checked part depends on [[@@$previous]]."}})
    }
    $calls.Add(@{tool='edit_card';arguments=@{at='@@1';body=(2..67 | ForEach-Object { "- [[@@$_]] Part $_" }) -join "`n"}})
    $inputFile = Join-Path $scratch 'accepted-calls.json'
    [IO.File]::WriteAllText($inputFile,($calls.ToArray() | ConvertTo-Json -Depth 12 -AsArray),$utf8)
    $trace = Join-Path $scratch 'calls.jsonl'
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_TRACE_TOOLS = $trace
    $env:ZT_DROP_BATCH_REPLY_FLAG = Join-Path $scratch 'committed-without-reply.flag'
    $prefix = 'qualification/model-large/1/build'
    $lost = @(& $Digest call $inputFile -OperationPrefix $prefix 2>&1)
    $lostExit = $LASTEXITCODE
    $treeList = Invoke-Zt @(@{tool='list_trees';arguments=@{}})
    $partialCount = 0
    if (@($treeList.trees).Count) { $partialCount = (Invoke-Zt @(@{tool='get_tree';arguments=@{tree='0'}})).cards.Count }
    Assert ($lostExit -ne 0 -and $partialCount -eq 64 -and (Test-Path -LiteralPath $env:ZT_DROP_BATCH_REPLY_FLAG)) 'the public helper loses its reply after one 64-item archive chunk commits'
    $env:ZT_TRACE_TOOLS = Join-Path $scratch 'retry-calls.jsonl'
    $retry = @(& $Digest call $inputFile -OperationPrefix $prefix 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and $retry.Count -eq 69) "retry completes the exact accepted calls: $($retry[-1])"
    $tree = Invoke-Zt @(@{tool='get_tree';arguments=@{tree='0'}})
    Assert ($tree.cards.Count -eq 68) 'replay restores original addresses without duplicating the committed prefix'
    $model = Invoke-Zt @(@{tool='get_model';arguments=@{at='0/1'}})
    Assert ($model.parts.Count -eq 66) 'cross-chunk local references retain the complete 66-part model'
    $revision = (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    Assert ($revision -eq 2) 'two committed chunks publish two revisions; the replayed prefix publishes none'
    $env:ZT_TRACE_TOOLS = Join-Path $scratch 'replay-calls.jsonl'
    $replay = @(& $Digest call $inputFile -OperationPrefix $prefix 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($replay -join "`n") -eq ($retry -join "`n")) 'a later new process returns every original allocated address'
    Assert ((Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $revision) 'a complete replay makes no Card revision'
    $requests = @(Get-Content -LiteralPath (Join-Path $scratch 'retry-calls.jsonl') | ForEach-Object { $_ | ConvertFrom-Json })
    $writes = @($requests | Where-Object tool -eq 'apply_batch')
    Assert (($writes.item_count -join ',') -eq '64,5' -and @($writes.process_id | Sort-Object -Unique).Count -eq 1) 'the resumed helper uses two deterministic bounded batches through one MCP process'
    $null = Invoke-Zt @(@{tool='edit_card';arguments=@{at='0/1|a';body='Independent edit after the accepted build.'}})
    $changed = @(& $Digest call $inputFile -OperationPrefix $prefix 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($changed -join ' ').Contains('identified output changed')) 'replayed receipts cannot claim completion after an output was independently edited'
    Assert ((Invoke-Zt @(@{tool='get_card';arguments=@{at='0/1|a'}})).body -eq 'Independent edit after the accepted build.') 'verification leaves the independent edit intact'
    $null = Invoke-Zt @(
        @{tool='create_literature';arguments=@{bibtex='@book{ReceiptTerms,title={Reliable terms}}'}},
        @{tool='create_successor';arguments=@{at='ReceiptTerms';title='First group'}}
    )
    $staging = Join-Path $scratch 'terms-staging'
    $termsOut = Join-Path $staging 'terms\out'
    New-Item -ItemType Directory -Force $termsOut | Out-Null
    [IO.File]::WriteAllText((Join-Path $staging 'manifest.json'),' {"citation_key":"ReceiptTerms","book_title":"Reliable terms"}',$utf8)
    [IO.File]::WriteAllText((Join-Path $termsOut 'terms.json'),' {"terms":[{"term":"资本","avoid":["资金"],"meaning":"生产要素","section":"§1"}]}',$utf8)
    $env:ZT_DROP_BATCH_REPLY_FLAG = Join-Path $scratch 'terms-committed-without-reply.flag'
    $termsPrefix = 'qualification/terms/1/apply'
    $termsLost = @(& $Digest terms -Step apply -Staging $staging -Apply -OperationPrefix $termsPrefix 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and (Invoke-Zt @(@{tool='get_tree';arguments=@{tree='ReceiptTerms'}})).cards.Count -eq 3) 'the actual terms compiler commits its accepted plan before losing the MCP reply'
    $termsRetry = @(& $Digest terms -Step apply -Staging $staging -Apply -OperationPrefix $termsPrefix 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($termsRetry[-1] | ConvertFrom-Json).card -eq 'ReceiptTerms/1|a') 'the compiler replays its retained create rather than switching to a different edit request'
    Assert ((Invoke-Zt @(@{tool='get_tree';arguments=@{tree='ReceiptTerms'}})).cards.Count -eq 3) 'compiler recovery retains exactly one term table'
    $null = Invoke-Zt @(@{tool='edit_card';arguments=@{at='ReceiptTerms/1|a';body='Independent term-table correction.'}})
    $termsChanged = @(& $Digest terms -Step apply -Staging $staging -Apply -OperationPrefix $termsPrefix 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($termsChanged -join ' ').Contains('identified output changed')) 'the compiler rereads receipt outputs before reporting completion'
    [IO.File]::WriteAllText((Join-Path $termsOut 'terms.json'),' {"terms":[{"term":"资本","avoid":["资金"],"meaning":"新接受的定义","section":"§2"}]}',$utf8)
    $conflictingPlan = @(& $Digest terms -Step apply -Staging $staging -Apply -OperationPrefix $termsPrefix 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($conflictingPlan -join ' ').Contains('another accepted terms plan')) 'changed input requires a newly accepted plan revision'
    $newPlan = @(& $Digest terms -Step apply -Staging $staging -Apply -OperationPrefix 'qualification/terms/2/apply' 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($newPlan[-1] | ConvertFrom-Json).card -eq 'ReceiptTerms/1|a') 'a newly accepted revision applies a guarded edit to the existing term table'
    Assert ((Invoke-Zt @(@{tool='get_card';arguments=@{at='ReceiptTerms/1|a'}})).body.Contains('新接受的定义')) 'the persisted compiler output is reread and verified'
    $null = Invoke-Zt @(@{tool='create_successor';arguments=@{at='ReceiptTerms/1';title='Independent inbound carrier'}})
    [IO.File]::WriteAllText((Join-Path $termsOut 'terms.json'),' {"terms":[{"term":"资本","meaning":"反向链接变化后的定义","section":"§3"}]}',$utf8)
    $env:ZT_RACE_AFTER_READ = '1'
    $env:ZT_RACE_TARGET = 'ReceiptTerms/2'
    $env:ZT_RACE_FLAG = Join-Path $scratch 'new-inbound-link.flag'
    $env:ZT_RACE_LINE = '参照 [[ReceiptTerms/1|a]]，这里给出独立的术语背景。'
    $inboundChanged = @(& $Digest terms -Step apply -Staging $staging -Apply -OperationPrefix 'qualification/terms/3/apply' 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and (Test-Path -LiteralPath $env:ZT_RACE_FLAG)) 'an unrelated inbound Link between plan read and apply does not invalidate the authored base'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item Env:ZT_DROP_BATCH_REPLY_FLAG -ErrorAction SilentlyContinue
    Remove-Item Env:ZT_CHECK_ZT -ErrorAction SilentlyContinue
    Remove-Item Env:ZT_TRACE_TOOLS -ErrorAction SilentlyContinue
    Remove-Item Env:ZT_RACE_AFTER_READ,Env:ZT_RACE_TARGET,Env:ZT_RACE_FLAG,Env:ZT_RACE_LINE -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
