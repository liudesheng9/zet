#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "owned-paragraph-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the intended test temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Save-Calls($path,$calls) { [IO.File]::WriteAllText($path,($calls | ConvertTo-Json -Depth 12 -AsArray),$utf8) }
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $body = "旧稿被保留。`n`n`$`$`nx^2+y^2=z^2`n`$`$`n`n![Figure](https://example.test/figure.png)"
    $null = Invoke-Zt @(@{tool='apply_batch';arguments=@{items=@(
        @{op_key='qualification/link/1/create/topic';client_ref='topic';tool='create_topic';arguments=@{title='Owned paragraphs'}},
        @{op_key='qualification/link/1/create/target';client_ref='target';tool='create_successor';arguments=@{at='@@topic';title='Constraint'}},
        @{op_key='qualification/link/1/create/source';client_ref='source';tool='create_successor';arguments=@{at='@@target';title='A claim';body=$body}}
    )}})
    $allow = Join-Path $scratch 'owned.txt'
    [IO.File]::WriteAllText($allow,"0/2`n",$utf8)
    $input = Join-Path $scratch 'link.json'
    $paragraph = '依赖 [[0/1]]，后者给出共同的约束。'
    Save-Calls $input @(@{tool='edit_card';arguments=@{at='0/2';base_receipt='qualification/link/1/create/source';owned_paragraph=@{text=$paragraph}}})
    $applied = @(& $Digest call $input -OperationPrefix 'qualification/link/1/apply' -Allow $allow 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and $applied[-1] -eq '0 ok 0/2') "identified Link writes retain the source allow-list: $($applied -join ' | ')"
    $before = Invoke-Zt @(@{tool='get_card';arguments=@{at='0/2'}})
    Assert ($before.body -eq "$body`n`n$paragraph") 'the helper appends only its owned paragraph'
    $replacement = '特例 [[0/1]]，本卡讨论较窄的约束情形。'
    Save-Calls $input @(@{tool='edit_card';arguments=@{at='0/2';base_receipt='qualification/link/1/apply/item-000000';owned_paragraph=@{text=$replacement;replaces_receipt='qualification/link/1/apply/item-000000'}}})
    $corrected = @(& $Digest call $input -OperationPrefix 'qualification/link/2/apply' -Allow $allow 2>&1)
    Assert ($LASTEXITCODE -eq 0) "the helper applies a receipt-bound correction: $($corrected -join ' | ')"
    $after = Invoke-Zt @(@{tool='get_card';arguments=@{at='0/2'}})
    Assert ($after.text -eq $before.text.Replace($paragraph,$replacement)) 'formulas, figure references and independent prose remain byte-preserved'
    $revision = (Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision
    Save-Calls $input @(@{tool='edit_card';arguments=@{at='0/2';base_receipt='qualification/link/2/apply/item-000000';owned_paragraph=@{text='参见 [[0/1]]。';replaces_receipt='qualification/link/2/apply/item-000000'}}})
    $badDraft = @(& $Digest call $input -OperationPrefix 'qualification/link/7/apply' -Allow $allow -CheckOnly 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and @($badDraft | Where-Object { $_ -match '^0/2 L[12] ' }).Count -gt 0) "the draft check runs the existing kind and reason rules: $($badDraft -join ' | ')"
    Assert ((Invoke-Zt @(@{tool='get_card';arguments=@{at='0/2'}})).text -eq $after.text) 'failed draft checks preserve all stored Card bytes'
    $draftText = '依赖 [[0/1]]，后者限制了本卡的可行选择。'
    Save-Calls $input @(@{tool='edit_card';arguments=@{at='0/2';base_receipt='qualification/link/2/apply/item-000000';owned_paragraph=@{text=$draftText;replaces_receipt='qualification/link/2/apply/item-000000'}}})
    $goodDraft = @(& $Digest call $input -OperationPrefix 'qualification/link/7/apply' -Allow $allow -CheckOnly 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and $goodDraft[-1] -eq 'faults: 0' -and ($goodDraft -join ' ').Contains('not_applied')) "a checked owned paragraph uses draft Links and excludes independent prose from its language check: $($goodDraft -join ' | ')"
    Assert ((Invoke-Zt @(@{tool='get_card';arguments=@{at='0/2'}})).text -eq $after.text) 'successful draft checks preserve all stored Card bytes'
    Assert ((Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})).archive_revision -eq $revision) 'draft validation publishes no archive revision'
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    foreach ($fault in @('paragraphs','cards','complete')) {
        $env:ZT_DRAFT_FAULT = $fault
        $incomplete = @(& $Digest call $input -OperationPrefix 'qualification/link/7/apply' -Allow $allow -CheckOnly 2>&1)
        Assert ($LASTEXITCODE -ne 0 -and ($incomplete -join ' ').Contains('incomplete Link draft') -and 'faults: 0' -notin $incomplete) "missing or incomplete $fault cannot become a clean draft check"
    }
    Remove-Item Env:ZT_DRAFT_FAULT,Env:ZT_CHECK_ZT
    $noPrefix = @(& $Digest call $input -Allow $allow -CheckOnly 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($noPrefix -join ' ').Contains('CheckOnly requires')) 'CheckOnly cannot fall through to the legacy writer'
    $applyDraft = @(& $Digest call $input -OperationPrefix 'qualification/link/7/apply' -Allow $allow 2>&1)
    Assert ($LASTEXITCODE -eq 0) "the same accepted draft can subsequently apply: $($applyDraft -join ' | ')"
    Assert ((Invoke-Zt @(@{tool='get_card';arguments=@{at='0/2'}})).text -eq $after.text.Replace($replacement,$draftText)) 'apply changes exactly the checked owned paragraph'
    Save-Calls $input @(@{tool='edit_card';arguments=@{at='0/1';base_receipt='qualification/link/1/create/target';owned_paragraph=@{text='依赖 [[0/2]]，后者给出共同的约束。'}}})
    $outside = @(& $Digest call $input -OperationPrefix 'qualification/link/3/apply' -Allow $allow 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($outside -join ' ').Contains('not allowed')) 'the identified helper refuses a source outside the owned manifest'
    Assert ((Invoke-Zt @(@{tool='get_card';arguments=@{at='0/1'}})).body -eq '') 'a denied source remains unchanged'
    Save-Calls $input @(
        @{tool='create_successor';arguments=@{at='0/2';title='Must not be created'}},
        @{tool='edit_card';arguments=@{at='@@0/1';body='Guessed child address.'}}
    )
    $deniedPlan = @(& $Digest call $input -OperationPrefix 'qualification/link/4/apply' -Allow $allow 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($deniedPlan -join ' ').Contains('not allowed')) 'the complete allow-list plan is checked before any writes; guessed descendant references are refused'
    Assert ((Invoke-Zt @(@{tool='get_tree';arguments=@{tree='0'}})).cards.Count -eq 3) 'a rejected later source prevents the earlier creation'
    Save-Calls $input @(
        @{tool='create_successor';arguments=@{at='0/2';title='Allowed child'}},
        @{tool='edit_card';arguments=@{at='@@0';body='An exact earlier result is within the accepted scope.'}}
    )
    $allowedChild = @(& $Digest call $input -OperationPrefix 'qualification/link/5/apply' -Allow $allow 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and $allowedChild[-1] -eq '1 ok 0/3') 'exact earlier results retain scope and automatic receipt bases'
    $byteCalls = @()
    for ($i=0; $i -lt 3; $i++) {
        $base = if ($i -eq 0) {'qualification/link/7/apply/item-000000'} else {"qualification/link/6/apply/item-$('{0:d6}' -f ($i-1))"}
        $byteCalls += @{tool='edit_card';arguments=@{at='0/2';base_receipt=$base;owned_paragraph=@{text=("$i " + ('a' * 400000) + ' 依赖 [[0/1]]，后者给出共同的约束。')}}}
    }
    Save-Calls $input $byteCalls
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_TRACE_TOOLS = Join-Path $scratch 'byte-chunks.jsonl'
    $large = @(& $Digest call $input -OperationPrefix 'qualification/link/6/apply' -Allow $allow 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and $large[-1] -eq '2 ok 0/2') "nested paragraph bytes are chunked without losing receipt dependencies: $($large[-1])"
    $writes = @(Get-Content $env:ZT_TRACE_TOOLS | ForEach-Object { $_ | ConvertFrom-Json } | Where-Object tool -eq 'apply_batch')
    Assert (($writes.item_count -join ',') -eq '2,1') 'the 1 MiB input bound includes nested owned paragraph text'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_TRACE_TOOLS,Env:ZT_DRAFT_FAULT -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
