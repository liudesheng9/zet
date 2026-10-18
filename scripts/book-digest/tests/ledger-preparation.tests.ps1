#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-preparation-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the test temp directory' }
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
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source,"# 导论`r`n`r`n企业用资本生产商品，交易双方选择产量和价格。`r`n`r`n# 第二章`r`n`r`n融资约束会限制企业的生产投入，从而改变市场交易。`r`n`r`n# 参考`r`n不在选定范围。",$utf8)
    $out = Join-Path $scratch 'prepared'
    $prepared = @(& $Digest prepare -Source $source -Out $out -CitationKey LedgerBook -BibTeX '@book{LedgerBook,title={Ledger book}}' -BookTitle 'Ledger book' -StartHeading '导论' -EndHeading '参考' -MaxUnits 1 -MaxChars 2000 -MinUnitChars 1 -NoFigures -NoEventCandidates -Variant econ -Apply 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "prepare failed: $($prepared[-1])" }
    Assert $true 'the real preparation declares its run contract'
    $manifest = Get-Content (Join-Path $out 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $contract = $manifest.run_contract
    $statusLines = @(& $Digest ledger status -Out $out 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and (Test-Path (Join-Path $out 'ledger.sqlite3'))) 'successful reliable preparation initializes its durable ledger before returning'
    Assert ($contract.schema_version -eq 1 -and $contract.variant -eq 'econ' -and $manifest.groups.Count -ge 2) 'the prepared target has an explicit schema, variant and complete group enumeration'
    $archive = Invoke-Zt @(@{tool='get_cards';arguments=@{addresses=@();view='text'}})
    Assert ($contract.archive_identity.id -eq $archive.archive_identity.id -and $contract.archive_identity.path -eq $archive.archive_identity.path) 'preparation binds the actual archive identity and resolved path'
    Assert ($contract.source.sha256 -ceq (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant() -and $contract.source.path.EndsWith('book.md')) 'source identity hashes the exact original bytes, including CRLF'
    Assert ($contract.options.max_units -eq 1 -and $contract.options.max_chars -eq 2000 -and $contract.options.no_figures -and $contract.options.no_event_candidates -and $contract.options.book_title -eq 'Ledger book' -and $contract.options.bibtex_sha256 -match '^[0-9a-f]{64}$') 'effective preparation and bibliographic inputs are frozen explicitly'
    $base = Get-Content (Join-Path $PSScriptRoot '..\workflow\zt-digest-base.blueprint.json') -Raw -Encoding utf8 | ConvertFrom-Json
    $econ = Get-Content (Join-Path $PSScriptRoot '..\workflow\zt-econ-digest.blueprint.json') -Raw -Encoding utf8 | ConvertFrom-Json
    Assert ($contract.contracts.base_version -eq $base.version -and $contract.contracts.variant_version -eq $econ.version -and $contract.contracts.artifact_schema -eq 1) 'workflow and artifact contract versions are derived from the actual frozen sources'
    Assert ($contract.groups.Count -eq $manifest.groups.Count -and $contract.group_manifest_hash -match '^[0-9a-f]{64}$') 'every selected group participates in the frozen manifest hash'
    foreach ($group in $contract.groups) {
        Assert ($group.sha256 -ceq (Get-FileHash -LiteralPath $group.path -Algorithm SHA256).Hash.ToLowerInvariant()) "group $($group.n) binds the actual prepared file"
    }
    # The frozen workflow modules pass -RunOptions as JSON text (zt-digest-prepare.js writes
    # `-RunOptions '{"terms":false,...}'`), while a hand-run prepare may name a file. Both forms must
    # prepare the same run contract, so a workflow's declared layer switches cannot be dropped.
    $inlineOut = Join-Path $scratch 'inline-prepared'
    $switches = '{"terms":false,"events":false,"findings":false,"figures":false}'
    $inline = @(& $Digest prepare -Source $source -Out $inlineOut -CitationKey InlineBook -BibTeX '@book{InlineBook,title={Inline book}}' -BookTitle 'Inline book' -StartHeading '导论' -EndHeading '参考' -MaxUnits 1 -MinUnitChars 1 -Variant econ -RunOptions $switches -Apply 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and (Test-Path (Join-Path $inlineOut 'manifest.json'))) "a prepare given -RunOptions as JSON text binds the declared switches: $($inline[-1])"
    $inlineManifest = Get-Content (Join-Path $inlineOut 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $inlineOptions = $inlineManifest.run_contract.options.run_options
    Assert (-not $inlineOptions.terms -and -not $inlineOptions.events -and -not $inlineOptions.findings -and -not $inlineOptions.figures) 'the declared switches are frozen into the run contract'
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    foreach ($fault in @('missing','incomplete')) {
        $env:ZT_IDENTITY_FAULT = $fault
        $faultOut = Join-Path $scratch "identity-$fault"
        $key = "FaultBook-$fault"
        $bad = @(& $Digest prepare -Source $source -Out $faultOut -CitationKey $key -BibTeX "@book{$key,title={Fault fixture}}" -StartHeading '导论' -EndHeading '参考' -MinUnitChars 1 -NoFigures -Variant econ -Apply 2>&1)
        Assert ($LASTEXITCODE -ne 0 -and ($bad -join ' ').Contains('incomplete archive identity')) "$fault identity refuses preparation explicitly: $($bad[-1])"
        Assert (-not (Test-Path (Join-Path $faultOut 'manifest.json')) -and $key -notin (Invoke-Zt @(@{tool='list_trees';arguments=@{}})).trees.tree) 'invalid identity publishes neither a manifest nor archive setup writes'
    }
    Remove-Item Env:ZT_IDENTITY_FAULT,Env:ZT_CHECK_ZT
    $atomicSource = Join-Path $scratch 'atomic-book.md'
    [IO.File]::WriteAllBytes($atomicSource,[IO.File]::ReadAllBytes($source))
    $atomicOut = Join-Path $scratch 'atomic-prepared'
    $arguments = @('prepare','-Source',$atomicSource,'-Out',$atomicOut,'-CitationKey','AtomicBook','-BibTeX','@book{AtomicBook,title={Atomic book}}','-StartHeading','导论','-EndHeading','参考','-MaxUnits','1','-MinUnitChars','1','-NoFigures')
    $initial = @(& $Digest @arguments 2>&1)
    Assert ($LASTEXITCODE -eq 0) 'the legacy artifact-only preparation remains available'
    $artifact = Join-Path $atomicOut 'book.zh.md'
    $oldText = [IO.File]::ReadAllText($artifact)
    $reader = [IO.File]::Open($artifact,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    try {
        [IO.File]::WriteAllText($atomicSource,[IO.File]::ReadAllText($atomicSource).Replace('企业用资本生产商品','这是新的完整版本。企业用资本生产商品'),$utf8)
        $updated = @(& $Digest @arguments 2>&1)
        Assert ($LASTEXITCODE -ne 0 -and [IO.File]::ReadAllText($artifact) -ceq $oldText) 'a blocked publication reports failure and preserves the complete old artifact'
        $textReader = [IO.StreamReader]::new($reader,$utf8,$true,1024,$true)
        try { $retained = $textReader.ReadToEnd() } finally { $textReader.Dispose() }
        Assert ($retained -ceq $oldText) 'an existing reader retains the complete previous version after the refused replacement'
    } finally { $reader.Dispose() }
    $retried = @(& $Digest @arguments 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and [IO.File]::ReadAllText($artifact).Contains('新的完整版本')) 'retry publishes the complete replacement after the reader closes'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item Env:ZT_IDENTITY_FAULT,Env:ZT_CHECK_ZT -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
