#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
# R231/R322–R330 (#83): owned DeepSeek resources are recorded at creation, settled from the exact
# ledger evidence, and reported as content_complete apart from cleanup_complete.
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "cleanup-resources-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest; $env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments,[int]$ExpectedExit = 0) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE -ne $ExpectedExit) { throw "digest failed ($LASTEXITCODE expected $ExpectedExit): $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Fail-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    return [pscustomobject]@{ code = $LASTEXITCODE; text = ($lines -join ' ') }
}
function Save-Json([string]$Path,$Value) { [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 20 -Compress), $utf8) }
function Common-Args([string]$Key) {
    return @('-Source',$source,'-CitationKey',$Key,'-BibTeX',"@book{$Key,title={Cleanup book}}",'-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-MaxUnits','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply','-MaxAttempts','1')
}
$script:currentOut = ''
function Register([hashtable]$Record,[string]$Out) {
    $file = Join-Path $scratch ("register-" + [guid]::NewGuid().ToString('N') + '.json')
    Save-Json $file $Record
    return Run-Digest @('ledger','resource','-Out',$Out,'-Register',$file)
}
function Fail-Register([hashtable]$Record,[string]$Out) {
    $file = Join-Path $scratch ("register-" + [guid]::NewGuid().ToString('N') + '.json')
    Save-Json $file $Record
    return Fail-Digest @('ledger','resource','-Out',$Out,'-Register',$file)
}
function Observe([hashtable]$Record,[hashtable]$Fence = @{}) {
    $file = Join-Path $scratch ("observe-" + [guid]::NewGuid().ToString('N') + '.json')
    Save-Json $file $Record
    $more = @('ledger','resource','-Out',$script:currentOut,'-Observe',$file)
    if ($Fence.Unit) { $more += @('-Unit',$Fence.Unit,'-Token',"$($Fence.Token)") }
    return Run-Digest $more
}
function Fail-Observe([hashtable]$Record,[hashtable]$Fence = @{}) {
    $file = Join-Path $scratch ("observe-" + [guid]::NewGuid().ToString('N') + '.json')
    Save-Json $file $Record
    $more = @('ledger','resource','-Out',$script:currentOut,'-Observe',$file)
    if ($Fence.Unit) { $more += @('-Unit',$Fence.Unit,'-Token',"$($Fence.Token)") }
    return Fail-Digest $more
}
function Fail-Unit([string]$Unit,[int]$Token,[string]$Layer) {
    $file = Join-Path $scratch ("fault-" + [guid]::NewGuid().ToString('N') + '.json')
    $hash = @((Run-Digest @('ledger','status','-Out',$script:currentOut,'-Layer',$Layer)).units | Where-Object id -EQ $Unit)[0].input_hash
    Save-Json $file ([ordered]@{ schema_version = 1; unit = $Unit; input_hash = $hash; code = 'check_failed'; cause = "the $Layer content failed in the cleanup fixture" })
    return Run-Digest @('ledger','fail','-Out',$script:currentOut,'-Unit',$Unit,'-Token',"$Token",'-Class','content','-Fault',$file)
}
function Settle-Layer([string]$Layer) {
    for ($round = 0; $round -lt 6; $round++) {
        $claimed = Run-Digest @('ledger','next','-Out',$script:currentOut,'-Layer',$Layer,'-Take','8','-Owner','cleanup-fixture')
        $units = @($claimed.units)
        if ($units.Count -eq 0) { return }
        foreach ($unit in $units) { $null = Fail-Unit $unit.id $unit.token $Layer }
    }
}
function Evidence([string]$Out,[string]$Name,[string]$Text) {
    $dir = Join-Path $Out 'cleanup\evidence'
    New-Item -ItemType Directory -Force $dir | Out-Null
    $path = Join-Path $dir $Name
    New-Item -ItemType Directory -Force (Split-Path -Parent $path) | Out-Null
    [IO.File]::WriteAllText($path,$Text,$utf8)
    return $path
}
function Register-Resource([string]$Out,[string]$Unit,[int]$Token,[string]$Workspace,[object[]]$Conversations,[object[]]$ScratchEntries = @()) {
    $record = [ordered]@{
        schema_version = 1; unit = $Unit; token = $Token
        workspace = @{ id = $Workspace; path = $Workspace }
        conversations = $Conversations
    }
    if ($ScratchEntries.Count) { $record.scratch = $ScratchEntries }
    return Register $record $Out
}
function Finish-Conversation([string]$Out,[string]$ResourceId,[string]$Conversation,[hashtable]$Fence = @{}) {
    $null = Observe ([ordered]@{ schema_version = 1; op = 'terminal'; resource = $ResourceId; conversation = $Conversation; state = 'finished'; outcome = 'answered' }) $Fence
    $answer = Evidence $Out "$Conversation.answer.json" '{"group":1,"cards":2}'
    $null = Observe ([ordered]@{ schema_version = 1; op = 'evidence'; resource = $ResourceId; conversation = $Conversation; answer = $answer }) $Fence
    $null = Observe ([ordered]@{ schema_version = 1; op = 'usage'; resource = $ResourceId; usage = [ordered]@{ input_tokens = 1200; output_tokens = 300 } }) $Fence
    $null = Observe ([ordered]@{ schema_version = 1; op = 'deleted'; resource = $ResourceId; conversation = $Conversation; result = 'already_absent' }) $Fence
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source,"# 导论`n`n融资约束影响生产选择。`n`n# 第二章`n`n资本投入影响产量。`n`n# 参考`n未选择。",$utf8)
    $options = Join-Path $scratch 'run-options.json'
    [IO.File]::WriteAllText($options,'{"terms":false,"models":false,"events":false,"findings":false,"hubs":false,"figures":false,"bridge":false}',$utf8)
    $out = Join-Path $scratch 'prepared'
    $script:currentOut = $out
    $null = Run-Digest (@('prepare') + (Common-Args 'CleanupBook') + @('-Out',$out))
    $run = (Run-Digest @('ledger','status','-Out',$out)).run_id
    $groupClaims = @((Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','2','-Owner','cleanup-manager')).units)
    $first = $groupClaims[0]; $second = $groupClaims[1]
    Assert ($groupClaims.Count -eq 2 -and $first.id -eq 'digest/g001@1') 'the fixture claims two real digest groups'

    # --- R322: the returned ids are recorded at creation, labelled with the run, the unit and the token ---
    $workspace = Join-Path $out 'dsh'
    New-Item -ItemType Directory -Force $workspace | Out-Null
    $registered = Register-Resource $out $first.id $first.token $workspace @(@{ id = 'conv-1'; run = 'run-1' })
    $label = "zet-$run-digest-g001-1-t$($first.token)"
    Assert ($registered.cleanup_unit -eq "cleanup/digest-g001-1-t$($first.token)@1" -and $registered.label -eq $label) 'a created resource registers its cleanup unit and the run/unit/token label'
    Assert ($registered.resources.Count -eq 1 -and $registered.resources[0].conversations[0].id -eq 'conv-1' -and $registered.resources[0].workspace.id -eq $workspace) 'the returned workspace and conversation ids are recorded immediately'
    $resourceId = $registered.resources[0].id
    $listed = Run-Digest @('ledger','resource','-Out',$out,'-List')
    Assert ($listed.resources.Count -eq 1 -and $listed.resources[0].label -eq $label -and $listed.resources[0].cleanup_unit -eq $registered.cleanup_unit) 'the recorded resource is listed with its exact identifiers'
    $cleanup = @((Run-Digest @('ledger','status','-Out',$out,'-Layer','cleanup')).units | Where-Object { $_.id -eq $registered.cleanup_unit })[0]
    Assert ($cleanup.state -eq 'registered' -and $cleanup.inputs.owner -eq $first.id) 'the cleanup unit waits on the work that created the resource'
    $runLevelUnit = @((Run-Digest @('ledger','status','-Out',$out,'-Layer','cleanup')).units | Where-Object { $_.id -eq 'cleanup/book@1' })[0]
    Assert ($runLevelUnit.state -eq 'registered') 'the run-level cleanup waits for the last content layer'
    $deps = @((Run-Digest @('ledger','status','-Out',$out,'-Layer','cleanup')).dependencies | Where-Object { $_.unit -eq $registered.cleanup_unit })
    Assert ($deps.Count -eq 1 -and $deps[0].needs -eq $first.id -and $deps[0].condition -eq 'settled') 'the cleanup unit is released when its owner settles, not before'
    # grossman2002integration trial 19: the CLI counted its actions without -SweepEvidence and refused the sweep alone.
    $answers = Join-Path $out 'answers'; New-Item -ItemType Directory -Force $answers | Out-Null
    $swept = Run-Digest @('ledger','resource','-Out',$out,'-SweepEvidence',$answers)
    Assert ($null -ne $swept) 'ledger resource -SweepEvidence runs as an action of its own'

    # --- R329/T13: a scratch target resolves inside the staging root; a stale attempt cannot register ---
    $bad = Fail-Register ([ordered]@{ schema_version = 1; unit = $first.id; token = $first.token; workspace = @{ id = 'ws'; path = $workspace }; conversations = @(@{ id = 'conv-x' }); scratch = @(@{ path = (Join-Path $scratch 'outside') }) }) $out
    Assert ($bad.code -eq 1 -and $bad.text.Contains('outside the exact staging root')) 'a scratch target outside the staging root is refused at registration'
    $bad = Fail-Register ([ordered]@{ schema_version = 1; unit = $first.id; token = $first.token; workspace = @{ id = 'ws'; path = $workspace }; conversations = @(@{ id = 'conv-x' }); scratch = @(@{ path = (Join-Path $out 'notes') }) }) $out
    Assert ($bad.code -eq 1 -and $bad.text.Contains('retained `notes` subtree')) 'a retained subtree is never a scratch target'
    $bad = Fail-Register ([ordered]@{ schema_version = 1; unit = $first.id; token = $first.token; workspace = @{ id = 'ws'; path = $workspace }; conversations = @(@{ id = 'conv-x' }); scratch = @(@{ path = (Join-Path $out 'manifest.json') }) }) $out
    Assert ($bad.code -eq 1 -and $bad.text.Contains('top-level staging file')) 'a top-level staging file is not compiler scratch'
    $bad = Fail-Register ([ordered]@{ schema_version = 1; unit = $first.id; token = ($first.token + 1); workspace = @{ id = 'ws'; path = $workspace }; conversations = @(@{ id = 'conv-x' }) }) $out
    Assert ($bad.code -eq 1 -and $bad.text.Contains('stale_claim')) 'a superseded attempt token cannot register a resource (T13)'
    $bad = Fail-Register ([ordered]@{ schema_version = 1; unit = $first.id; token = $first.token; workspace = @{ id = 'ws'; path = $workspace }; conversations = @(@{ id = 'conv-x' }); scratch = @(@{ path = 'relative\scratch' }) }) $out
    Assert ($bad.code -eq 1 -and $bad.text.Contains('absolute path')) 'a scratch target must be an absolute path'

    # --- R328/R329: a completed model folder is removable only after its plan is retained elsewhere ---
    $batch = Join-Path $out 'models\batch1'
    New-Item -ItemType Directory -Force $batch | Out-Null
    $plan = Join-Path $batch 'plan.json'
    [IO.File]::WriteAllText($plan,'{"schema_version":1,"parts":2}',$utf8)
    [IO.File]::WriteAllText((Join-Path $batch 'notes.md'),'scratch notes',$utf8)
    $retainedPlan = Evidence $out 'models\batch1\plan.json' '{"schema_version":1,"parts":2}'
    $null = Register-Resource $out $first.id $first.token $workspace @(@{ id = 'conv-1' }) @(@{ path = $batch; retained = @(@{ from = $plan; path = $retainedPlan; sha256 = (Get-FileHash $plan -Algorithm SHA256).Hash.ToLower() }) })

    # --- R322: a lost creation response is adopted only through this run's own label ---
    $secondResource = Register-Resource $out $second.id $second.token $workspace @(@{ lost = $true })
    $lostResourceId = $secondResource.resources[0].id
    $secondLabel = "zet-$run-digest-g002-1-t$($second.token)"
    $bad = Fail-Observe ([ordered]@{ schema_version = 1; op = 'recovered'; resource = $lostResourceId; conversation = 'conv-3'; label = 'some-other-label' })
    Assert ($bad.code -eq 1 -and $bad.text.Contains('own resource label')) 'a lost creation response is not adopted from a foreign label'
    $recovered = Observe ([ordered]@{ schema_version = 1; op = 'recovered'; resource = $lostResourceId; conversation = 'conv-3'; label = $secondLabel })
    Assert ($recovered.resources[0].conversations[0].id -eq 'conv-3' -and $recovered.resources[0].conversations[0].lost -eq $false) 'the run label recovers the exact conversation id inside its own workspace'

    # --- R324/R325/R326: terminal state, retained evidence, usage and the exact deletion ---
    Finish-Conversation $out $resourceId 'conv-1'
    Finish-Conversation $out $lostResourceId 'conv-3'
    $facts = @((Run-Digest @('ledger','resource','-Out',$out,'-List')).resources | Where-Object { $_.id -eq $resourceId })
    Assert ($facts.Count -eq 1 -and $facts[0].conversations[0].terminal -and $facts[0].conversations[0].deleted -and $facts[0].usage.input_tokens -eq 1200) 'each recorded conversation carries its terminal state, retained answer and per-run usage'
    $outside = Join-Path $scratch 'outside.txt'
    [IO.File]::WriteAllText($outside,'evidence that must stay out of the durable path',$utf8)
    $bad = Fail-Observe ([ordered]@{ schema_version = 1; op = 'evidence'; resource = $resourceId; conversation = 'conv-1'; answer = $outside })
    Assert ($bad.code -eq 1 -and $bad.text.Contains('durable evidence path')) 'evidence outside the durable evidence path is refused'
    $bad = Fail-Observe ([ordered]@{ schema_version = 1; op = 'deleted'; resource = $resourceId; conversation = 'conv-9'; result = 'confirmed' })
    Assert ($bad.code -eq 1 -and $bad.text.Contains('unknown conversation')) 'an observation may not name a conversation this run never recorded'
    $bad = Fail-Observe ([ordered]@{ schema_version = 1; op = 'terminal'; resource = $resourceId; conversation = 'conv-1'; state = 'finished'; outcome = 'another' })
    Assert ($bad.code -eq 1 -and $bad.text.Contains('operation_conflict')) 'a second terminal observation cannot rewrite the recorded outcome'

    # --- R330: cleanup runs on a halted path; content and cleanup are reported apart ---
    $null = Fail-Unit $first.id $first.token 'digest'
    $null = Fail-Unit $second.id $second.token 'digest'
    $audit = @((Run-Digest @('ledger','next','-Out',$out,'-Layer','audit','-Take','1','-Owner','cleanup-fixture')).units)[0]
    $null = Fail-Unit $audit.id $audit.token 'audit'
    $cleanupClaims = @((Run-Digest @('ledger','next','-Out',$out,'-Layer','cleanup','-Take','8','-Owner','cleanup-agent')).units)
    $perUnit = @($cleanupClaims | Where-Object { $_.id -eq $registered.cleanup_unit })[0]
    $secondUnit = @($cleanupClaims | Where-Object { $_.id -eq $secondResource.cleanup_unit })[0]
    $runLevel = @($cleanupClaims | Where-Object { $_.id -eq 'cleanup/book@1' })[0]
    Assert ($cleanupClaims.Count -eq 3 -and $perUnit -and $secondUnit -and $runLevel) 'each owned resource and the shared workspace have a cleanup unit that the halted run still owes'
    # T13: while the cleanup unit is claimed, only its current token may record another fact.
    $bad = Fail-Observe ([ordered]@{ schema_version = 1; op = 'terminal'; resource = $resourceId; conversation = 'conv-1'; state = 'finished'; outcome = 'answered' })
    Assert ($bad.code -eq 1 -and $bad.text.Contains('claim_active')) 'a claimed cleanup unit refuses an unfenced observation'
    $bad = Fail-Observe ([ordered]@{ schema_version = 1; op = 'terminal'; resource = $resourceId; conversation = 'conv-1'; state = 'finished'; outcome = 'answered' }) @{ Unit = $perUnit.id; Token = ($perUnit.token + 1) }
    Assert ($bad.code -eq 1 -and $bad.text.Contains('stale_claim')) 'a late observation with an old token cannot apply'
    $fenced = Observe ([ordered]@{ schema_version = 1; op = 'terminal'; resource = $resourceId; conversation = 'conv-1'; state = 'finished'; outcome = 'answered' }) @{ Unit = $perUnit.id; Token = $perUnit.token }
    Assert ($fenced.resources[0].conversations[0].terminal) 'the current claim token records the observation'
    # The per-unit cleanup completes while the content of the run is incomplete.
    $checkedOnly = Run-Digest @('cleanup-finish','-Out',$out,'-Unit',$perUnit.id,'-Token',"$($perUnit.token)",'-CheckOnly')
    Assert ($checkedOnly.check_only -and -not $checkedOnly.applied -and (Test-Path $batch)) 'a preview verifies the cleanup without removing anything'
    $perDone = Run-Digest @('cleanup-finish','-Out',$out,'-Unit',$perUnit.id,'-Token',"$($perUnit.token)")
    Assert ($perDone.kind -eq 'cleanup_completion' -and $perDone.content_complete -eq $false -and $perDone.cleanup_complete -eq $true -and $perDone.status -eq 'partial') 'the per-unit cleanup completes while the content of the halted run stays incomplete'
    Assert (-not (Test-Path $batch) -and (Test-Path $retainedPlan) -and @($perDone.scratch.removed).Count -eq 1) 'the disposable model folder is removed while its retained plan stays under the durable evidence path'
    Assert (@($perDone.content.unresolved) -contains 'digest/g001@1' -and @($perDone.content.stale_publications).Count -gt 0) 'the completion names the unfinished content instead of calling the run complete'
    $replay = Run-Digest @('ledger','done','-Out',$out,'-Unit',$perUnit.id,'-Token',"$($perUnit.token)")
    Assert ($replay.reused -and $replay.cleanup_complete -and $replay.unit -eq $perUnit.id) 'ledger done dispatches the same cleanup verifier and replays the checked result'

    # --- R327: an unknown workspace member is reconciled, never removed wholesale ---
    $null = Observe ([ordered]@{ schema_version = 1; op = 'workspace'; resource = $resourceId; removed = $true }) @{ Unit = $runLevel.id; Token = $runLevel.token }
    $pending = Fail-Digest @('cleanup-finish','-Out',$out,'-Unit',$runLevel.id,'-Token',"$($runLevel.token)")
    Assert ($pending.code -eq 1 -and $pending.text.Contains('the owned workspace is not removed yet')) 'the shared workspace is reported removed only once every recorded resource agrees'
    $null = Observe ([ordered]@{ schema_version = 1; op = 'listing'; resource = $lostResourceId; conversations = @(); unknown = @(@{ id = 'conv-9'; title = 'the user''s own conversation' }) }) @{ Unit = $runLevel.id; Token = $runLevel.token }
    $early = Fail-Digest @('cleanup-finish','-Out',$out,'-Unit',$runLevel.id,'-Token',"$($runLevel.token)")
    Assert ($early.code -eq 1 -and $early.text.Contains('cleanup_workspace_removed_early')) 'a workspace reported removed while an unknown member remains is refused'
    $null = Observe ([ordered]@{ schema_version = 1; op = 'listing'; resource = $lostResourceId; conversations = @(); unknown = @() }) @{ Unit = $runLevel.id; Token = $runLevel.token }
    $pending = Fail-Digest @('cleanup-finish','-Out',$out,'-Unit',$runLevel.id,'-Token',"$($runLevel.token)")
    Assert ($pending.code -eq 1 -and $pending.text.Contains('the owned workspace is not removed yet')) 'the shared workspace waits for every recorded resource of the run'
    $null = Observe ([ordered]@{ schema_version = 1; op = 'workspace'; resource = $lostResourceId; removed = $true }) @{ Unit = $runLevel.id; Token = $runLevel.token }
    $runDone = Run-Digest @('cleanup-finish','-Out',$out,'-Unit',$runLevel.id,'-Token',"$($runLevel.token)")
    Assert ($runDone.cleanup_complete -eq $true -and $runDone.content_complete -eq $false -and @($runDone.resources).Count -eq 2) 'the run-level cleanup settles both owned workspaces and still reports the content as incomplete'
    # --- R330: a transient provider fact stays retryable without rebuilding content ---
    $transientOut = Join-Path $scratch 'transient'
    $script:currentOut = $transientOut
    $null = Run-Digest (@('prepare') + (Common-Args 'CleanupTransient') + @('-Out',$transientOut))
    $claim = @((Run-Digest @('ledger','next','-Out',$transientOut,'-Layer','digest','-Take','1','-Owner','cleanup-manager')).units)[0]
    $transient = Register-Resource $transientOut $claim.id $claim.token $workspace @(@{ id = 'conv-5' })
    $transientResource = $transient.resources[0].id
    $null = Fail-Unit $claim.id $claim.token 'digest'
    $cleanupClaim = @((Run-Digest @('ledger','next','-Out',$transientOut,'-Layer','cleanup','-Take','8','-Owner','cleanup-agent')).units | Where-Object { $_.id -eq $transient.cleanup_unit })[0]
    $active = Fail-Digest @('cleanup-finish','-Out',$transientOut,'-Unit',$cleanupClaim.id,'-Token',"$($cleanupClaim.token)")
    Assert ($active.code -eq 1 -and $active.text.Contains('not terminal yet')) 'an active conversation is observed terminal before any deletion is accepted'
    $fence = @{ Unit = $cleanupClaim.id; Token = $cleanupClaim.token }
    $null = Observe ([ordered]@{ schema_version = 1; op = 'terminal'; resource = $transientResource; conversation = 'conv-5'; state = 'error'; outcome = 'the provider failed' }) $fence
    $null = Observe ([ordered]@{ schema_version = 1; op = 'evidence'; resource = $transientResource; conversation = 'conv-5'; result = (Evidence $transientOut 'conv-5.transcript.md' 'provider error: rate limited') }) $fence
    $null = Observe ([ordered]@{ schema_version = 1; op = 'usage'; resource = $transientResource; usage = [ordered]@{ input_tokens = 40 } }) $fence
    $pendingDeletion = Fail-Digest @('cleanup-finish','-Out',$transientOut,'-Unit',$cleanupClaim.id,'-Token',"$($cleanupClaim.token)")
    Assert ($pendingDeletion.code -eq 1 -and $pendingDeletion.text.Contains('no confirmed deletion yet')) 'a recorded conversation without a deletion fact keeps the cleanup retryable'
    $null = Observe ([ordered]@{ schema_version = 1; op = 'delete_failed'; resource = $transientResource; conversation = 'conv-5'; code = 'network'; message = 'the provider did not answer' }) $fence
    $refused = Fail-Digest @('cleanup-finish','-Out',$transientOut,'-Unit',$cleanupClaim.id,'-Token',"$($cleanupClaim.token)")
    Assert ($refused.code -eq 1 -and $refused.text.Contains('cleanup_transient') -and $refused.text.Contains('network')) 'a failed provider deletion refuses as a retryable cleanup fault naming the exact cause'
    $stillClaimed = @((Run-Digest @('ledger','status','-Out',$transientOut,'-Layer','cleanup')).units | Where-Object { $_.id -eq $cleanupClaim.id })[0]
    Assert ($stillClaimed.state -eq 'claimed' -and $stillClaimed.result -eq $null) 'a refused cleanup keeps its claim and records no completion, so the content is never rebuilt'
    $null = Observe ([ordered]@{ schema_version = 1; op = 'deleted'; resource = $transientResource; conversation = 'conv-5'; result = 'confirmed' }) $fence
    $retried = Run-Digest @('cleanup-finish','-Out',$transientOut,'-Unit',$cleanupClaim.id,'-Token',"$($cleanupClaim.token)")
    Assert ($retried.cleanup_complete -eq $true -and $retried.status -eq 'partial') 'the retried cleanup settles on the confirmed deletion without touching content'

    # --- R323/R167: the cleanup layer closes only after every layer that can register a resource ---
    $script:currentOut = $out
    $before = Run-Digest @('ledger','gate','-Out',$out,'-Layer','cleanup') 3
    Assert (-not $before.sealed -and -not $before.settled) 'the cleanup layer stays open while another layer can still register a resource'
    $secondDone = Run-Digest @('cleanup-finish','-Out',$out,'-Unit',$secondUnit.id,'-Token',"$($secondUnit.token)")
    Assert ($secondDone.cleanup_complete -eq $true -and $secondDone.unit -eq $secondUnit.id) 'the second owned resource settles on its own unit after the shared workspace is removed'
    foreach ($layer in @('cards','link','cards-book','link-check','organize','skeleton','audit')) { Settle-Layer $layer }
    $after = Run-Digest @('ledger','gate','-Out',$out,'-Layer','cleanup')
    Assert ($after.sealed -and $after.settled -and $after.counts.done -eq 3) 'the cleanup layer settles on its three completed units once every other unit is terminal'
    Assert ($after.cleanup.content_complete -eq $false -and $after.cleanup.cleanup_complete -eq $true -and $after.cleanup.run_complete -eq $false) 'the run reports content_complete and cleanup_complete as separate facts'

    # --- R327: an unrecovered lost creation response is a reported reconciliation, not a silent success ---
    $reconcileOut = Join-Path $scratch 'reconcile'
    $script:currentOut = $reconcileOut
    $null = Run-Digest (@('prepare') + (Common-Args 'CleanupReconcile') + @('-Out',$reconcileOut))
    $claims = @((Run-Digest @('ledger','next','-Out',$reconcileOut,'-Layer','digest','-Take','2','-Owner','cleanup-manager')).units)
    $lost = Register-Resource $reconcileOut $claims[0].id $claims[0].token $workspace @(@{ lost = $true })
    $null = Observe ([ordered]@{ schema_version = 1; op = 'usage'; resource = $lost.resources[0].id; usage = [ordered]@{ input_tokens = 10 } })
    $runOwned = Register-Resource $reconcileOut 'run' 1 $workspace @(@{ id = 'conv-run' })
    Assert ($runOwned.cleanup_unit -eq 'cleanup/book@1' -and $runOwned.label -eq "zet-$((Run-Digest @('ledger','status','-Out',$reconcileOut)).run_id)-run-t1") 'a stage-wide conversation is owned by the run-level cleanup unit'
    $null = Observe ([ordered]@{ schema_version = 1; op = 'usage'; resource = $runOwned.resources[0].id; usage = [ordered]@{ input_tokens = 5 } })
    $guarded = Join-Path $reconcileOut 'models\guarded'
    New-Item -ItemType Directory -Force $guarded | Out-Null
    [IO.File]::WriteAllText((Join-Path $guarded 'plan.json'),'{"schema_version":1,"parts":1}',$utf8)
    $guardedResource = Register-Resource $reconcileOut $claims[1].id $claims[1].token $workspace @(@{ id = 'conv-6' }) @(@{ path = $guarded; retained = @() })
    Finish-Conversation $reconcileOut $guardedResource.resources[0].id 'conv-6'
    $null = Fail-Unit $claims[0].id $claims[0].token 'digest'
    $null = Fail-Unit $claims[1].id $claims[1].token 'digest'
    $reconcileClaims = @((Run-Digest @('ledger','next','-Out',$reconcileOut,'-Layer','cleanup','-Take','8','-Owner','cleanup-agent')).units)
    $lostUnit = @($reconcileClaims | Where-Object { $_.id -eq $lost.cleanup_unit })[0]
    $guardedUnit = @($reconcileClaims | Where-Object { $_.id -eq $guardedResource.cleanup_unit })[0]
    $refused = Fail-Digest @('cleanup-finish','-Out',$reconcileOut,'-Unit',$guardedUnit.id,'-Token',"$($guardedUnit.token)")
    Assert ($refused.code -eq 1 -and $refused.text.Contains('retained_material_missing') -and (Test-Path $guarded)) 'an accepted plan inside a scratch target is refused when no durable copy exists'
    $partial = Run-Digest @('cleanup-finish','-Out',$reconcileOut,'-Unit',$lostUnit.id,'-Token',"$($lostUnit.token)")
    Assert ($partial.cleanup_complete -eq $false -and $partial.status -eq 'partial' -and @($partial.reconciliation)[0].code -eq 'lost_creation_response') 'an unrecovered creation response completes as a reported reconciliation rather than a silent success'
    Assert ($partial.reconciliation[0].workspace -eq $workspace -and $partial.reconciliation[0].cleanup_unit -eq $lost.cleanup_unit) 'the reconciliation names the exact workspace and cleanup unit a human must reconcile'
    Write-Output 'ok: owned resources, cleanup evidence and scratch removal verified'
} finally {
    if (Test-Path $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue }
}
