#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
# R331–R346 (#85): the run report states verified coverage, current placeholders and exact
# per-run usage from the ledger and verified reads, and never from an agent's prose.
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-report-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest; $env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments,[int]$ExpectedExit = 0) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE -ne $ExpectedExit) { throw "digest failed ($LASTEXITCODE expected $ExpectedExit): $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
# A command whose JSON may print over several lines (prepare prints its manifest pretty).
function Run-Json([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "digest failed ($LASTEXITCODE): $($lines -join ' ')" }
    return ($lines -join "`n") | ConvertFrom-Json -AsHashtable
}
function Fail-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    return [pscustomobject]@{ code = $LASTEXITCODE; text = ($lines -join ' ') }
}
function Save-Json([string]$Path,$Value) { [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 20 -Compress), $utf8) }
# One helper call: its answer line is "N ok <address>" or "N <json>"; this fixture needs the address.
function Call-Tool([string]$Tool,[hashtable]$Arguments) {
    $lines = @(& $Digest call -Tool $Tool -Json ($Arguments | ConvertTo-Json -Depth 8 -Compress) 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "call $Tool failed: $($lines -join ' ')" }
    $text = ($lines -join "`n")
    if ($text -match 'ok\s+(\S+)') { return $Matches[1] }
    throw "call $Tool returned no address: $text"
}
function Report([string]$Out) { return Run-Digest @('ledger','report','-Out',$Out) }
function Gap([object]$Report,[string]$Kind,[string]$Needle) {
    return @($Report.gaps | Where-Object { $_.kind -eq $Kind -and (($_.unit + ' ' + $_.address + ' ' + $_.reason) -like "*$Needle*") })
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source,"# 导论`n`n融资约束影响生产选择。`n`n# 第二章`n`n资本投入影响产量。`n`n# 参考`n未选择。",$utf8)
    $options = Join-Path $scratch 'run-options.json'
    [IO.File]::WriteAllText($options,'{"terms":false,"models":false,"events":false,"findings":false,"figures":false,"bridge":false}',$utf8)
    $out = Join-Path $scratch 'prepared'
    $key = 'ReportBook'
    $null = Run-Json (@('prepare') + @('-Source',$source,'-CitationKey',$key,'-BibTeX',"@book{$key,title={Report book}}",'-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-MaxUnits','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply','-MaxAttempts','1') + @('-Out',$out))
    $manifest = Get-Content -LiteralPath (Join-Path $out 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $status = Run-Digest @('ledger','status','-Out',$out)

    # --- R331/R332: the report exists, states its identity, and re-renders idempotently -------
    $first = Report $out
    foreach ($name in 'run-report.json','run-report.md') {
        Assert ((Test-Path -LiteralPath (Join-Path $out $name)) -and $first.report[$name].sha256 -match '^[0-9a-f]{64}$') "the report writes $name with its content hash"
    }
    Assert ($first.identity.run_id -ceq $status.run_id -and $first.identity.invocation -eq 1 -and $first.identity.resume_count -eq 0) 'the report carries the run identity, its invocation and the resume count'
    Assert ($first.identity.source.sha256 -ceq $manifest.source_sha256 -and $first.identity.range.group_manifest_hash -ceq $manifest.run_contract.group_manifest_hash) 'the report carries the source hash and the frozen range hash'
    Assert ($first.identity.workflow.base_version -ceq $manifest.run_contract.contracts.base_version -and $first.identity.workflow.variant_version -ceq $manifest.run_contract.contracts.variant_version) 'the report names the frozen workflow contract versions'
    Assert ($first.identity.binaries.zt_digest -and $first.identity.binaries.zt_check -and $first.identity.binaries.zt) 'the report names the binary versions it was produced with'
    Assert ($first.identity.archive.id -ceq $manifest.run_contract.archive_identity.id -and $first.identity.archive.verified -eq $true -and $null -ne $first.identity.archive.revision) 'the report names the archive identity and the revision its verified read used'
    Assert (($first.identity.enabled_layers -contains 'digest') -and ($first.identity.enabled_layers -contains 'cleanup')) 'the report names the effective enabled layers'
    Assert ($first.counts.declared -eq 9 -and $first.counts.per_layer.prepare.done -eq 1 -and $first.counts.per_layer.digest.declared -eq 2) 'the report counts declared and done work per layer'
    foreach ($field in 'settled','content_complete','quality_complete','cleanup_complete','complete') {
        Assert ($first.completeness.ContainsKey($field)) "the report keeps $field as its own fact"
    }
    Assert ($first.completeness.complete -eq $false -and $first.completeness.content_complete -eq $false) 'an unfinished run is not reported complete'
    $skeletonPublication = @($first.publications | Where-Object { $_.layer -eq 'skeleton' })[0]
    Assert ($skeletonPublication.revision -eq 1 -and $skeletonPublication.current -eq $true) 'the report lists the latest required publication revision as current'
    Assert (@($first.counts.invocations).Count -eq 1 -and $first.counts.invocations[0].invocation -eq 1) 'the report keeps the invocation history separate from resume work'
    $markdown = [IO.File]::ReadAllText((Join-Path $out 'run-report.md'))
    Assert ($markdown -match '# Run report' -and $markdown -match 'content_complete' -and $markdown -match 'DeepSeek usage') 'the human report renders the same facts'
    $beforeRevision = (zt status 2>&1 | Select-String -Pattern '^revision: (\d+)').Matches.Groups[1].Value
    $mdBytes = [IO.File]::ReadAllBytes((Join-Path $out 'run-report.md'))
    $jsonBytes = [IO.File]::ReadAllBytes((Join-Path $out 'run-report.json'))
    $again = Report $out
    Assert ($again.report['run-report.md'].changed -eq $false -and $again.report['run-report.json'].changed -eq $false) 're-rendering an unchanged run writes nothing'
    Assert ([Convert]::ToBase64String($mdBytes) -ceq [Convert]::ToBase64String([IO.File]::ReadAllBytes((Join-Path $out 'run-report.md'))) -and [Convert]::ToBase64String($jsonBytes) -ceq [Convert]::ToBase64String([IO.File]::ReadAllBytes((Join-Path $out 'run-report.json')))) 'the second render leaves byte-identical report files'
    $afterRevision = (zt status 2>&1 | Select-String -Pattern '^revision: (\d+)').Matches.Groups[1].Value
    Assert ($beforeRevision -eq $afterRevision) 'rendering the report changes no Card and no archive revision'

    # --- R336: current placeholders come from verified reads, and a failed read is unknown -----
    $modelAddress = "$key/1"
    $headAddress = Call-Tool 'create_successor' @{ at = "$key/1"; kind = 'side'; title = '融资约束条件下的投入选择'; type = 'model'; level = 'formal'; body = '待写' }
    $partAddress = Call-Tool 'create_successor' @{ at = $headAddress; kind = 'side'; title = '企业按融资成本安排投入'; type = 'model'; facet = '设定'; body = '待写' }
    $verified = Report $out
    Assert ($verified.placeholders.status -eq 'verified' -and $verified.placeholders.count -eq 2) 'the report counts the current placeholder parts from a verified read'
    Assert ((@($verified.placeholders.models[0].placeholders) -contains $headAddress) -and (@($verified.placeholders.models[0].placeholders) -contains $partAddress)) 'the report names the exact placeholder addresses and parts'
    Assert (@(Gap $verified 'placeholder' $partAddress).Count -eq 1) 'each placeholder part is a named gap'
    Assert ($verified.completeness.quality_complete -eq $false) 'a run with a placeholder is not quality-complete'
    $seam = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $realZt = $env:ZT_CHECK_ZT
    $env:ZT_CHECK_ZT = $seam; $env:ZT_FAIL_TREE = $key
    $unread = Report $out
    $env:ZT_CHECK_ZT = $realZt; Remove-Item Env:ZT_FAIL_TREE -ErrorAction SilentlyContinue
    Assert ($unread.placeholders.status -eq 'unknown' -and $null -eq $unread.placeholders.count -and $unread.placeholders.reason) 'a failed placeholder read is unknown, never a zero count'
    Assert ($unread.completeness.quality_complete -eq $false -and $unread.completeness.complete -eq $false) 'an unknown placeholder count cannot certify quality'

    # --- R335/R337: gaps and pending jobs name their exact unit, address and time --------------
    $groupOne = @((Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','2','-Owner','report-manager')).units)
    $first = $groupOne[0]; $second = $groupOne[1]
    $stamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $jobFile = Join-Path $scratch 'job.json'
    Save-Json $jobFile ([ordered]@{ schema_version = 1; id = 'dsh-run-1'; kind = 'dsh'; started_at = ($stamp - 60) })
    $null = Run-Digest @('ledger','job','-Out',$out,'-Unit',$second.id,'-Token',"$($second.token)",'-Register',$jobFile)
    $observed = Join-Path $scratch 'observe-job.json'
    Save-Json $observed ([ordered]@{ schema_version = 1; id = 'dsh-run-1'; state = 'terminal'; outcome = 'answered' })
    $null = Run-Digest @('ledger','job','-Out',$out,'-Unit',$second.id,'-Observe',$observed)
    $stillRunning = Join-Path $scratch 'job-2.json'
    Save-Json $stillRunning ([ordered]@{ schema_version = 1; id = 'dsh-run-2'; kind = 'dsh'; started_at = $stamp })
    $null = Run-Digest @('ledger','job','-Out',$out,'-Unit',$second.id,'-Token',"$($second.token)",'-Register',$stillRunning)
    $fault = Join-Path $scratch 'fault.json'
    $hash = @($status.units | Where-Object { $_.id -eq $first.id })[0].input_hash
    Save-Json $fault ([ordered]@{ schema_version = 1; unit = $first.id; input_hash = $hash; code = 'check_failed'; cause = 'the group draft held an unsourced claim' })
    $null = Run-Digest @('ledger','fail','-Out',$out,'-Unit',$first.id,'-Token',"$($first.token)",'-Class','content','-Fault',$fault)
    $gapped = Report $out
    Assert (@(Gap $gapped 'group_gap' $first.id).Count -eq 1 -and @(Gap $gapped 'group_gap' 'unsourced claim').Count -eq 1) 'a failed group is a gap naming its unit and its recorded reason'
    Assert (@(Gap $gapped 'pending_job' 'dsh-run-2').Count -eq 1) 'an external run that never reached a terminal state is a named pending-job gap'
    Assert ($gapped.counts.per_layer.digest.failed -eq 1 -and $gapped.counts.per_layer.digest.claimed -eq 1) 'the report separates failed from still-claimed work'
    Assert ($gapped.timing.external_run_seconds -ge 59 -and $gapped.timing.parallel_stages_not_summed -eq $true) 'the report measures external-run time and does not add parallel stage durations'
    Assert ($gapped.timing.critical_path.units.Count -ge 1 -and $null -ne $gapped.timing.critical_path.seconds) 'the report states one critical path rather than a sum of parallel stages'
    Assert (@(Gap $gapped 'cleanup' 'cleanup').Count -ge 1 -and $gapped.completeness.cleanup_complete -eq $false) 'unfinished cleanup is reported and keeps the run incomplete'

    # --- R343–R346: exact per-run usage, deduplicated, with explicit missing coverage ---------
    $workspace = Join-Path $out 'dsh'
    New-Item -ItemType Directory -Force $workspace | Out-Null
    $register = Join-Path $scratch 'resource.json'
    Save-Json $register ([ordered]@{
        schema_version = 1; unit = $second.id; token = $second.token
        workspace = @{ id = $workspace; path = $workspace }
        conversations = @(
            @{ id = 'conv-1'; run = 'run-1' }, @{ id = 'conv-2'; run = 'run-2' },
            @{ id = 'conv-3'; run = 'run-3' }, @{ id = 'conv-4'; run = 'run-4' }
        )
    })
    $registered = Run-Digest @('ledger','resource','-Out',$out,'-Register',$register)
    $resourceId = $registered.resources[0].id
    function Observe([hashtable]$Record) {
        $file = Join-Path $scratch ("observe-" + [guid]::NewGuid().ToString('N') + '.json')
        Save-Json $file $Record
        return Run-Digest @('ledger','resource','-Out',$out,'-Observe',$file)
    }
    function Fail-Observe([hashtable]$Record) {
        $file = Join-Path $scratch ("observe-" + [guid]::NewGuid().ToString('N') + '.json')
        Save-Json $file $Record
        return Fail-Digest @('ledger','resource','-Out',$out,'-Observe',$file)
    }
    $null = Observe @{ schema_version = 1; op = 'usage'; resource = $resourceId; conversation = 'conv-1'; usage = [ordered]@{ input_tokens = 100; cost = 1.5 } }
    $null = Observe @{ schema_version = 1; op = 'usage'; resource = $resourceId; conversation = 'conv-1'; usage = [ordered]@{ input_tokens = 100; cost = 1.5 } }
    $null = Observe @{ schema_version = 1; op = 'usage'; resource = $resourceId; conversation = 'conv-2'; request = 'req-1'; usage = [ordered]@{ output_tokens = 10 } }
    $null = Observe @{ schema_version = 1; op = 'usage'; resource = $resourceId; conversation = 'conv-2'; request = 'req-1'; usage = [ordered]@{ output_tokens = 10 } }
    $null = Observe @{ schema_version = 1; op = 'usage'; resource = $resourceId; conversation = 'conv-2'; request = 'req-2'; usage = [ordered]@{ output_tokens = 5 } }
    $null = Observe @{ schema_version = 1; op = 'usage'; resource = $resourceId; conversation = 'conv-3'; state = 'cancelled'; usage = [ordered]@{ output_tokens = 7 }; estimate = [ordered]@{ cost = 0.25 } }
    $null = Observe @{ schema_version = 1; op = 'usage_unavailable'; resource = $resourceId; conversation = 'conv-4'; reason = 'account_wide_only' }
    $null = Observe @{ schema_version = 1; op = 'usage'; resource = $resourceId; usage = [ordered]@{ cost = 4.0 } }
    $null = Observe @{ schema_version = 1; op = 'usage'; resource = $resourceId; usage = [ordered]@{ cost = 1.0 } }
    $negative = Fail-Observe @{ schema_version = 1; op = 'usage'; resource = $resourceId; conversation = 'conv-1'; usage = [ordered]@{ cost = -1.5 } }
    Assert ($negative.code -ne 0 -and $negative.text -match 'non-negative') 'a negative provider charge is refused as a data fault, not stored'
    $usage = Report $out
    Assert ($usage.usage.known_runs -eq 3 -and $usage.usage.missing_runs -eq 1) 'the report counts three known runs and one missing rather than inventing a total'
    Assert ($usage.usage.totals.input_tokens -eq 100 -and $usage.usage.totals.output_tokens -eq 22) 'one run read twice counts once, and a continuation adds only its new requests'
    Assert ($usage.usage.totals.cost -eq 1.5) 'a cancelled run with recorded usage counts, and an estimate stays out of the measured charge'
    Assert ($usage.usage.estimates.cost -eq 0.25) 'an estimate is reported separately and labelled'
    Assert ($usage.usage.status -eq 'partial' -and @($usage.usage.missing)[0].reason -eq 'account_wide_only') 'missing per-run usage is explicit coverage, never a complete claim'
    Assert ($usage.usage.by_phase.initial.runs -eq 3 -and $usage.usage.by_phase.resume.runs -eq 0) 'the report separates initial work from a resume'
    Assert (@($usage.usage.resource_aggregates).Count -eq 1 -and $usage.usage.resource_aggregates[0].usage.cost -eq 1) 'a rolling resource total stays a labelled aggregate and never becomes the run cost'
    Assert ($usage.usage.totals.cost -ge 0) 'no differenced account total can make a negative run cost'

    # --- R333/R334: a deliberate no-op is explicit evidence, not silent success ---------------
    $noopOut = Join-Path $scratch 'NoTermsCandidates'
    $noopSource = Join-Path $scratch 'noop.md'
    [IO.File]::WriteAllText($noopSource,"# 导论`n`n读者按章节顺序阅读本书。`n`n# 参考`n`n未选择。",$utf8)
    $null = Run-Json @('prepare','-Source',$noopSource,'-Out',$noopOut,'-CitationKey','NoTermsCandidates','-BibTeX','@book{NoTermsCandidates,title={No terms}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','math','-Apply')
    $termsClaim = @((Run-Digest @('ledger','next','-Out',$noopOut,'-Layer','terms','-Take','1','-Owner','terms-manager')).units)[0]
    $termsPlan = Run-Digest @('terms','-Step','plan','-Staging',$noopOut,'-Token',"$($termsClaim.token)",'-MaxTerms','5000','-MaxCandidates','20000')
    Assert ($termsPlan.expected_jobs -eq 0 -and $termsPlan.no_op.reason -eq 'no_terms_candidates') 'a zero-candidate Terms enumeration is an explicit checked no-op, not an empty agent answer'
    $termsDone = Run-Digest @('terms','-Step','apply','-Staging',$noopOut,'-Token',"$($termsClaim.token)",'-Apply')
    Assert ($termsDone.state -eq 'done' -and $termsDone.disposition -eq 'no_terms') 'the checked no-op completion records its own disposition'
    $noop = Report $noopOut
    Assert ($noop.counts.per_layer.terms.noop -eq 1 -and $noop.counts.noop -eq 1 -and $noop.counts.per_layer.terms.done -eq 1) 'the report counts the deliberate no-op apart from done work'
    Assert ($noop.completeness.content_complete -eq $false) 'a run with unresolved groups is still not content-complete'

    # --- R340: the supported resume invocation is printed, and never as a bare guess ----------
    $resume = Report $out
    Assert ($resume.resume.available -eq $true -and $resume.resume.invocation -match 'zt-econ-digest\.js' -and $resume.resume.invocation -match 'mode: "resume"') 'the report prints the supported resume invocation while the one resume is unused'
    Assert ($resume.resume.invocation -notmatch '\\\\\?\\') 'the printed invocation names the staging folder in the form a reader can run'
    $emptyRepo = Join-Path $scratch 'empty-repo'
    New-Item -ItemType Directory -Force $emptyRepo | Out-Null
    $savedRepo = $env:ZT_DIGEST_REPO
    $env:ZT_DIGEST_REPO = $emptyRepo
    $noWorkflow = Report $out
    $env:ZT_DIGEST_REPO = $savedRepo
    Assert ($noWorkflow.resume.available -eq $false -and $noWorkflow.resume.reason -match 'not found') 'an uninstalled variant workflow is reported instead of an invocation that cannot run'

    # --- the report is the same facts the concise status view reports -------------------------
    $concise = Run-Digest @('ledger','status','-Out',$out,'-Verify')
    Assert ($concise.completeness.content_complete -eq $resume.completeness.content_complete -and $concise.completeness.complete -eq $resume.completeness.complete) 'ledger status is the concise view of the same completeness facts'
    Assert ($concise.placeholders.status -eq 'verified' -and $concise.placeholders.count -eq $resume.placeholders.count) 'the verified status view counts the same current placeholders'
    Assert ($concise.usage.known_runs -eq $resume.usage.known_runs -and $concise.usage.missing_runs -eq $resume.usage.missing_runs) 'the concise view reports the same per-run usage coverage'
    Assert ($concise.resume.invocation -ceq $resume.resume.invocation) 'the concise view prints the same resume invocation'
    $plain = Run-Digest @('ledger','status','-Out',$out)
    Assert ($plain.placeholders.status -eq 'not_read' -and $null -eq $plain.placeholders.count) 'a ledger-only status states that it verified no placeholder count'
} finally {
    Remove-Item Env:ZT_DIGEST_REPO -ErrorAction SilentlyContinue
    try { zt down 2>&1 | Out-Null } catch { }
    if (Test-Path $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue }
}
