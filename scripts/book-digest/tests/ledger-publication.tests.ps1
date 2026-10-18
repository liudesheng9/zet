#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-publication-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest; $env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) { throw "digest failed ($LASTEXITCODE): $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Refuses([string[]]$Arguments,[string]$Expected) {
    $lines = @(& $Digest @Arguments 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($lines -join ' ').Contains($Expected)) "refuses $Expected"
}
# The same Content-Hash the bridge compiler and the ledger use: SHA-256 over the
# compact canonical JSON of the object without its excluded keys.
function Get-ContentHash($value,[string[]]$except) {
    function Sort-Keys($v) {
        if ($v -is [System.Collections.IDictionary]) {
            $sorted = [ordered]@{}
            foreach ($key in ([string[]]$v.Keys | Sort-Object -CaseSensitive)) { $sorted[$key] = Sort-Keys $v[$key] }
            return $sorted
        }
        if ($v -is [System.Collections.IEnumerable] -and $v -isnot [string]) { return ,@($v | ForEach-Object { Sort-Keys $_ }) }
        return $v
    }
    $filtered = [ordered]@{}
    foreach ($key in $value.Keys) { if ($except -notcontains $key) { $filtered[$key] = $value[$key] } }
    $json = (Sort-Keys $filtered) | ConvertTo-Json -Compress -Depth 40
    $bytes = [Text.Encoding]::UTF8.GetBytes($json)
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
}
$inspect = @'
const {DatabaseSync}=require('node:sqlite');
const db=new DatabaseSync(process.argv[1],{readOnly:true});
console.log(JSON.stringify({
 units:db.prepare('select * from units order by id').all(),
 deps:db.prepare('select * from deps order by unit_id,needs_id').all(),
 outputs:db.prepare('select * from outputs order by unit_id,output_key').all(),
 events:db.prepare('select * from events order by seq').all()
}));db.close();
'@
function Evidence { (& node --no-warnings -e $inspect (Join-Path $out 'ledger.sqlite3')) }
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source,"# 导论`n`n融资约束改变生产选择。`n`n# 第二章`n`n资本投入改变产量。`n`n# 参考`n未选择。",$utf8)
    $options = Join-Path $scratch 'run-options.json'
    [IO.File]::WriteAllText($options,'{"terms":false,"models":false,"events":false,"findings":false,"hubs":false,"figures":false,"bridge":false}',$utf8)
    $out = Join-Path $scratch 'prepared'
    $manifest = Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','PublishBook','-BibTeX','@book{PublishBook,title={Publish book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-MaxUnits','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    $evidence = Evidence

    # One test at a time: an unchanged input manifest must derive the revision that
    # preparation already registered, and must not add a second one.
    $published = Run-Digest @('ledger','publish','-Out',$out,'-Layer','organize')
    Assert ($published.unit -ceq 'organize/book@1' -and $published.revision -eq 1 -and $published.reused -eq $true -and @($published.superseded).Count -eq 0) 'an unchanged publication manifest reuses the revision that preparation registered'
    Assert ((Evidence) -ceq $evidence) 'reusing a derived publication revision registers no new unit, dependency or event'

    # One producer now fails, so the book's publication input set really changed.
    $claim = Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','publication-manager')
    $fault = Join-Path $scratch 'fault.json'
    [IO.File]::WriteAllText($fault,(@{code='source_unavailable';unit=$claim.units[0].id;input_hash=$claim.units[0].input_hash;cause='the fixture refuses this group'} | ConvertTo-Json),$utf8)
    Run-Digest @('ledger','fail','-Out',$out,'-Unit',$claim.units[0].id,'-Token',"$($claim.units[0].token)",'-Class','content','-Fault',$fault) | Out-Null
    $next = Run-Digest @('ledger','publish','-Out',$out,'-Layer','organize')

    # The report names the latest required revision and says what it replaced.
    $report = Run-Digest @('ledger','status','-Out',$out)
    $publication = @($report.publications | Where-Object { $_.layer -eq 'organize' -and $_.key -eq 'book' })
    Assert ($publication.Count -eq 1 -and $publication[0].latest -ceq 'organize/book@2' -and $publication[0].revision -eq 2 -and $publication[0].current -eq $true) 'the report selects the latest required publication revision'
    Assert (@($publication[0].superseded | Where-Object { $_.unit -ceq 'organize/book@1' -and $_.state -eq 'failed' -and $_.fault.code -eq 'superseded' -and $_.fault.by -ceq 'organize/book@2' }).Count -eq 1) 'the report states that the earlier open revision was superseded and by which revision'

    # A superseded revision is terminal history, not failed work to be resumed.
    $superseded = @($report.units | Where-Object { $_.id -eq 'organize/book@1' })[0]
    Assert ($superseded.state -eq 'failed' -and $superseded.fault.code -eq 'superseded' -and $report.counts.superseded -eq 1 -and $report.counts.failed -eq 1) 'a superseded revision is terminal without being counted as a failure beside the real one'
    Assert ('organize/book@1' -notin $report.unresolved -and 'organize/book@1' -notin (Run-Digest @('ledger','rerun-list','-Out',$out)).units.id) 'superseded work is never offered to a resume'

    # Every producer of the book publication has now settled, so a fresh revision
    # describes a complete input set and is real claimable work.
    $second = Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','4','-Owner','publication-manager')
    foreach ($unit in $second.units) {
        [IO.File]::WriteAllText($fault,(@{code='source_unavailable';unit=$unit.id;input_hash=$unit.input_hash;cause='the fixture refuses this group'} | ConvertTo-Json),$utf8)
        Run-Digest @('ledger','fail','-Out',$out,'-Unit',$unit.id,'-Token',"$($unit.token)",'-Class','content','-Fault',$fault) | Out-Null
    }
    $settled = Run-Digest @('ledger','publish','-Out',$out,'-Layer','organize')
    Assert ($settled.unit -ceq 'organize/book@3' -and -not $settled.reused -and @($settled.dispositions).Count -eq 2 -and @($settled.contributions).Count -eq 0) 'a settled producer set derives the next publication revision from the settled dispositions'
    $claimable = Run-Digest @('ledger','next','-Out',$out,'-Layer','organize','-Take','4','-Owner','publication-manager')
    Assert ($claimable.units.Count -eq 1 -and $claimable.units[0].id -ceq 'organize/book@3' -and $claimable.units[0].inputs.logical_key -ceq 'book') 'a sealed publication whose producers settled becomes claimable work exactly once'

    # A checked revision accounts for the whole manifest it was registered with and
    # binds the compiler artifacts that carry its result.
    foreach ($name in @('decisions','applied')) {
        [IO.File]::WriteAllText((Join-Path $out "$name.json"),( @{ name = $name; fixture = $true } | ConvertTo-Json ),$utf8)
    }
    $claimed = $claimable.units[0]
    $plan = Join-Path $out 'publication-organize-book-3.json'
    [IO.File]::WriteAllText($plan,(@{
        schema_version = 1
        kind = 'publication_completion'
        unit = $claimed.id
        token = $claimed.token
        layer = 'organize'
        key = 'book'
        input_hash = $claimed.input_hash
        disposition = 'partial'
        consumed = @()
        unresolved = @($claimed.inputs.dispositions | ForEach-Object { @{ unit = $_.unit } })
        coverage = @()
        artifacts = @{ decisions = (Join-Path $out 'decisions.json'); applied = (Join-Path $out 'applied.json') }
    } | ConvertTo-Json -Depth 8),$utf8)
    $done = Run-Digest @('ledger','done','-Out',$out,'-Unit',$claimed.id,'-Token',"$($claimed.token)",'-Outputs',$plan)
    Assert ($done.state -eq 'done' -and $done.disposition -eq 'partial' -and @($done.consumed).Count -eq 0 -and @($done.unresolved).Count -eq 2 -and $done.artifacts.decisions.sha256 -match '^[0-9a-f]{64}$' -and $done.artifacts.applied.sha256 -match '^[0-9a-f]{64}$') 'a publication revision completes with every manifest entry accounted for and its compiler artifacts hash-bound'
    $repeat = Run-Digest @('ledger','done','-Out',$out,'-Unit',$claimed.id,'-Token',"$($claimed.token)",'-Outputs',$plan)
    Assert ($repeat.reused -eq $true -and $repeat.disposition -eq 'partial' -and $repeat.completion.sha256 -ceq $done.completion.sha256) 'repeating a completed publication revision is read-only and retains its evidence'

    # A skeleton revision must classify every prepared anchor; it may not invent
    # coverage for an anchor whose producer never became substantive work.
    $skeleton = Run-Digest @('ledger','publish','-Out',$out,'-Layer','skeleton')
    Assert ($skeleton.unit -ceq 'skeleton/book@2' -and -not $skeleton.reused) 'another publication layer derives its own revision from the same sorted manifest'
    $skeletonClaim = Run-Digest @('ledger','next','-Out',$out,'-Layer','skeleton','-Take','1','-Owner','publication-manager')
    Assert ($skeletonClaim.units.Count -eq 1 -and $skeletonClaim.units[0].id -ceq 'skeleton/book@2') 'the skeleton revision is claimable through the ledger'
    [IO.File]::WriteAllText((Join-Path $out 'skeleton.json'),(@{ fixture = 'skeleton' } | ConvertTo-Json),$utf8)
    $anchors = @($manifest.groups | ForEach-Object { $_.anchor })
    $skelUnit = $skeletonClaim.units[0]
    function Skeleton-Plan([string]$Outcome) {
        $planPath = Join-Path $out 'publication-skeleton-book-2.json'
        [IO.File]::WriteAllText($planPath,(@{
            schema_version = 1
            kind = 'publication_completion'
            unit = $skelUnit.id
            token = $skelUnit.token
            layer = 'skeleton'
            key = 'book'
            input_hash = $skelUnit.input_hash
            disposition = 'partial'
            consumed = @()
            unresolved = @($skelUnit.inputs.dispositions | ForEach-Object { @{ unit = $_.unit } })
            coverage = @($anchors | ForEach-Object { @{ ref = $_; outcome = $Outcome; reason = 'fixture' } })
            artifacts = @{ skeleton = (Join-Path $out 'skeleton.json') }
        } | ConvertTo-Json -Depth 8),$utf8)
        return $planPath
    }
    Refuses @('ledger','done','-Out',$out,'-Unit',$skelUnit.id,'-Token',"$($skelUnit.token)",'-Outputs',(Skeleton-Plan 'indexed')) 'its producer is unresolved'
    $skelDone = Run-Digest @('ledger','done','-Out',$out,'-Unit',$skelUnit.id,'-Token',"$($skelUnit.token)",'-Outputs',(Skeleton-Plan 'unresolved'))
    Assert ($skelDone.state -eq 'done' -and $skelDone.disposition -eq 'partial' -and @($skelDone.coverage).Count -eq $anchors.Count -and @($skelDone.coverage | Where-Object { $_.outcome -eq 'unresolved' -and $_.reason -and $_.producer }).Count -eq $anchors.Count) 'a skeleton revision completes with every prepared anchor explicitly unresolved instead of fabricated coverage'

    # A hub refresh is coalesced by touched tree, so it declares that tree with its
    # required upper hubs and Register instead of refreshing every tree at once.
    $hubOut = Join-Path $scratch 'hubs'
    $hubSource = Join-Path $scratch 'hubs.md'
    [IO.File]::WriteAllText($hubSource,"# 导论`n`n企业的融资约束改变投资选择。`n`n# 参考`n未选择。",$utf8)
    $hubOptions = Join-Path $scratch 'hubs-options.json'
    [IO.File]::WriteAllText($hubOptions,'{"terms":false,"models":false,"events":false,"findings":false,"figures":false,"bridge":false}',$utf8)
    $hubManifest = Run-Digest @('prepare','-Source',$hubSource,'-Out',$hubOut,'-CitationKey','HubTree','-BibTeX','@book{HubTree,title={Hub tree}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','econ','-RunOptions',$hubOptions,'-Apply')
    $hubClaim = (Run-Digest @('ledger','next','-Out',$hubOut,'-Layer','digest','-Take','1','-Owner','hub-manager')).units[0]
    $hubStart = Run-Digest @('group-start','-Out',$hubOut,'-Group','1','-Token',"$($hubClaim.token)")
    $hubJob = Get-Content -LiteralPath $hubStart.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    [IO.File]::WriteAllText($hubStart.draft,(@{
        schema_version = 1; unit = $hubClaim.id; token = $hubClaim.token; input_hash = $hubStart.input_hash; disposition = 'cards'
        calls = @(
            @{ tool = 'edit_card'; arguments = @{ at = 'HubTree/1'; title = '融资约束改变投资选择'; body = '融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。'; base_fingerprint = $hubJob.inputs.anchor.fingerprint } },
            @{ tool = 'create_successor'; arguments = @{ at = 'HubTree/1'; kind = 'side'; title = '贷款收紧限制企业投入'; body = '融资成本影响企业的生产选择。贷款收紧使企业减少设备投入。投入减少限制下一期的生产能力。来源：[[HubTree/1]]' } }
        )
    } | ConvertTo-Json -Depth 8),$utf8)
    $hubDone = Run-Digest @('group-finish','-Out',$hubOut,'-Group','1','-Token',"$($hubClaim.token)",'-Draft',$hubStart.draft)
    Assert ($hubDone.state -eq 'done' -and 'HubTree' -in $hubDone.trees) 'a completed producer records the trees it actually touched'

    $hubScope = Join-Path $scratch 'hub-scope.json'
    [IO.File]::WriteAllText($hubScope,'{"tree":"OtherTree","upper":[],"register":null}',$utf8)
    Refuses @('ledger','publish','-Out',$hubOut,'-Layer','hubs','-Key','HubTree','-Scope',$hubScope) 'differs from the requested key'
    [IO.File]::WriteAllText($hubScope,'{"tree":"HubTree","upper":["HubTree/1"],"register":null}',$utf8)
    $hubPublication = Run-Digest @('ledger','publish','-Out',$hubOut,'-Layer','hubs','-Key','HubTree','-Scope',$hubScope)
    Assert ($hubPublication.unit -ceq 'hubs/HubTree@1' -and $hubPublication.scope.tree -ceq 'HubTree' -and $hubPublication.scope.upper -contains 'HubTree/1' -and @($hubPublication.contributions).Count -eq 1 -and $hubPublication.contributions[0].unit -ceq 'digest/g001@1') 'a hub refresh coalesces the producers that touched its declared tree'
    [IO.File]::WriteAllText($hubScope,'{"tree":"HubTree","upper":["HubTree/1","HubTree/2"],"register":null}',$utf8)
    $changedScope = Run-Digest @('ledger','publish','-Out',$hubOut,'-Layer','hubs','-Key','HubTree','-Scope',$hubScope)
    Assert ($changedScope.unit -ceq 'hubs/HubTree@2' -and -not $changedScope.reused) 'a changed declared scope is a changed input and registers the next revision'
    [IO.File]::WriteAllText($hubScope,'{"tree":"OtherTree","upper":[],"register":null}',$utf8)
    $untouched = Run-Digest @('ledger','publish','-Out',$hubOut,'-Layer','hubs','-Key','OtherTree','-Scope',$hubScope)
    Assert ($untouched.unit -ceq 'hubs/OtherTree@1' -and @($untouched.contributions).Count -eq 0) 'a tree no producer touched has no contributing work of its own'

    # Every independent note suggestion becomes one work unit with its own outcome,
    # registered against the Card state the suggestion was made from.
    $suggestions = Join-Path $scratch 'suggestions.json'
    [IO.File]::WriteAllText($suggestions,'[{"id":"n01","card":"HubTree/1","note":"[[HubTree/1|a]] 依赖 融资约束"},{"id":"n02","card":"HubTree/1|a","note":"[[HubTree/1]] 承接 融资约束"},{"id":"n03","card":"HubTree/1","note":"[[MissingCard/9]] 对应 未知来源"}]',$utf8)
    $notes = Run-Digest @('ledger','notes','-Out',$hubOut,'-Suggestions',$suggestions)
    Assert (@($notes.units).Count -eq 3 -and @($notes.units | Where-Object { $_.unit -eq 'notes/n01@1' -and $_.state -eq 'registered' }).Count -eq 1 -and @($notes.units | Where-Object { $_.unit -eq 'notes/n03@1' }).Count -eq 1) 'each note suggestion registers its own readable logical unit'
    Assert (@($notes.units | Where-Object { -not $_.fingerprint -or -not $_.target }).Count -eq 0) 'a registered suggestion records the Card state and the Link it proposes'
    $again = Run-Digest @('ledger','notes','-Out',$hubOut,'-Suggestions',$suggestions)
    Assert ($again.reused -eq $true -and @((Run-Digest @('ledger','status','-Out',$hubOut,'-Layer','notes')).units).Count -eq 3) 'repeating the same suggestions registers no second unit'
    [IO.File]::WriteAllText($suggestions,'[{"id":"n01","card":"HubTree/1","note":"[[HubTree/2]] 印证 融资约束"}]',$utf8)
    Refuses @('ledger','notes','-Out',$hubOut,'-Suggestions',$suggestions) 'a registered suggestion key cannot be rebound to different content'

    # Each suggestion reaches exactly one outcome, checked against the archive: an
    # applied Link changed the Card, an already-present Link did not, and a refusal
    # carries its source or relationship reason.
    $noteClaim = Run-Digest @('ledger','next','-Out',$hubOut,'-Layer','notes','-Take','4','-Owner','notes-manager')
    Assert (@($noteClaim.units).Count -eq 3) 'every registered suggestion is claimable as its own work unit'
    function Note-Plan([string]$Id,[string]$Outcome,[string]$Reason) {
        $unit = @($notes.units | Where-Object { $_.unit -eq "notes/$Id@1" })[0]
        $claim = @($noteClaim.units | Where-Object { $_.id -eq "notes/$Id@1" })[0]
        $planPath = Join-Path $hubOut "note-$Id.json"
        [IO.File]::WriteAllText($planPath,(@{
            schema_version = 1
            kind = 'note_disposition'
            unit = $claim.id
            token = $claim.token
            outcome = $Outcome
            reason = $Reason
            suggestion = @{ id = $Id; card = $unit.card; note = $unit.note; target = $unit.target; fingerprint = $unit.fingerprint }
        } | ConvertTo-Json -Depth 8),$utf8)
        return $planPath
    }
    Refuses @('ledger','done','-Out',$hubOut,'-Unit','notes/n01@1','-Token',"$(@($noteClaim.units | Where-Object { $_.id -eq 'notes/n01@1' })[0].token)",'-Outputs',(Note-Plan 'n01' 'refused' '')) 'must give its source or relationship reason'
    $null = Invoke-Zt @(@{tool='apply_batch';arguments=@{items=@(
        @{op_key='qualification/notes/1/apply/n01';client_ref='n01';tool='edit_card';arguments=@{at='HubTree/1';body='融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。[[HubTree/1|a]] 依赖 融资约束';base_fingerprint=$hubDone.card_fingerprints['HubTree/1']}}
    )}})
    $n01 = Run-Digest @('ledger','done','-Out',$hubOut,'-Unit','notes/n01@1','-Token',"$(@($noteClaim.units | Where-Object { $_.id -eq 'notes/n01@1' })[0].token)",'-Outputs',(Note-Plan 'n01' 'applied' ''))
    Assert ($n01.state -eq 'done' -and $n01.outcome -eq 'applied' -and $n01.present -eq $true -and $n01.fingerprint -ne $n01.suggestion.fingerprint) 'an applied suggestion is a checked Link that changed its Card'
    Refuses @('ledger','done','-Out',$hubOut,'-Unit','notes/n02@1','-Token',"$(@($noteClaim.units | Where-Object { $_.id -eq 'notes/n02@1' })[0].token)",'-Outputs',(Note-Plan 'n02' 'applied' '')) 'without changing'
    $n02 = Run-Digest @('ledger','done','-Out',$hubOut,'-Unit','notes/n02@1','-Token',"$(@($noteClaim.units | Where-Object { $_.id -eq 'notes/n02@1' })[0].token)",'-Outputs',(Note-Plan 'n02' 'already_present' ''))
    $n03 = Run-Digest @('ledger','done','-Out',$hubOut,'-Unit','notes/n03@1','-Token',"$(@($noteClaim.units | Where-Object { $_.id -eq 'notes/n03@1' })[0].token)",'-Outputs',(Note-Plan 'n03' 'refused' '来源不支持该关系'))
    Assert ($n02.outcome -eq 'already_present' -and $n02.present -eq $true -and $n02.fingerprint -eq $n02.suggestion.fingerprint -and $n03.outcome -eq 'refused' -and $n03.reason -eq '来源不支持该关系' -and $n03.present -eq $false) 'every suggestion retains its own already-present or reasoned refusal outcome'

    # A recorded structural mapping updates the addresses this run published; an
    # unexplained move is a precise conflict, not a silently stale locator.
    $moves = Join-Path $scratch 'moves.json'
    $null = & $Digest call -Tool move_card -Json '{"at":"HubTree/1|a","to":"HubTree/2","confirm":"move"}' -Moves $moves 2>&1
    Assert ($LASTEXITCODE -eq 0 -and (Test-Path $moves) -and ((Get-Content -LiteralPath $moves -Raw) -match 'HubTree/1\|a')) 'a real move records its exact structural mapping'
    $emptyMoves = Join-Path $scratch 'no-moves.json'
    [IO.File]::WriteAllText($emptyMoves,'{}',$utf8)
    Refuses @('ledger','relocate','-Out',$hubOut,'-Moves',$emptyMoves) 'unreconciled_move'
    $relocated = Run-Digest @('ledger','relocate','-Out',$hubOut,'-Moves',$moves)
    Assert ($relocated.applied -ge 1 -and @($relocated.units).Count -ge 1 -and -not $relocated.reused) 'a recorded mapping updates every published Card locator it explains'
    $again = Run-Digest @('ledger','relocate','-Out',$hubOut,'-Moves',$moves)
    Assert ($again.applied -eq 0 -and $again.reused) 'reapplying the same recorded mapping changes no locator twice'
    $badMoves = Join-Path $scratch 'bad-moves.json'
    [IO.File]::WriteAllText($badMoves,'{"HubTree/1":"HubTree/99"}',$utf8)
    Refuses @('ledger','relocate','-Out',$hubOut,'-Moves',$badMoves) 'must name a Card that exists'

    # T15: a completion fault after the outputs are written rolls the whole
    # publication transition back, so a done revision never loses or half-gains work.
    $skelAgain = Run-Digest @('ledger','publish','-Out',$out,'-Layer','skeleton')
    $skeletonOut = Join-Path $scratch 'skeleton-second'
    $null = New-Item -ItemType Directory -Force $skeletonOut
    $secondSource = Join-Path $scratch 'second.md'
    [IO.File]::WriteAllText($secondSource,"# 导论`n`n# 参考`n未选择。",$utf8)
    $secondOptions = Join-Path $scratch 'second-options.json'
    [IO.File]::WriteAllText($secondOptions,'{"terms":false,"models":false,"events":false,"findings":false,"figures":false}',$utf8)
    $second = Run-Digest @('prepare','-Source',$secondSource,'-Out',$skeletonOut,'-CitationKey','FaultBook','-BibTeX','@book{FaultBook,title={Fault book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','econ','-RunOptions',$secondOptions,'-Apply')
    $group = (Run-Digest @('ledger','next','-Out',$skeletonOut,'-Layer','digest','-Take','1','-Owner','fault-manager')).units[0]
    $started = Run-Digest @('group-start','-Out',$skeletonOut,'-Group','1','-Token',"$($group.token)")
    [IO.File]::WriteAllText($started.draft,(@{
        schema_version = 1; unit = $group.id; token = $group.token; input_hash = $started.input_hash; disposition = 'no_cards'; calls = @()
    } | ConvertTo-Json -Depth 8),$utf8)
    $null = Run-Digest @('group-finish','-Out',$skeletonOut,'-Group','1','-Token',"$($group.token)",'-Draft',$started.draft)
    $faultPublication = Run-Digest @('ledger','publish','-Out',$skeletonOut,'-Layer','audit')
    $faultClaim = (Run-Digest @('ledger','next','-Out',$skeletonOut,'-Layer','audit','-Take','1','-Owner','fault-manager')).units[0]
    [IO.File]::WriteAllText((Join-Path $skeletonOut 'report.json'),(@{ fixture = 'audit' } | ConvertTo-Json),$utf8)
    $faultPlan = Join-Path $skeletonOut ("publication-{0}.json" -f ($faultClaim.id -replace '[/@]','-'))
    [IO.File]::WriteAllText($faultPlan,(@{
        schema_version = 1; kind = 'publication_completion'; unit = $faultClaim.id; token = $faultClaim.token
        layer = 'audit'; key = 'book'; input_hash = $faultClaim.input_hash
        disposition = 'published'; consumed = @($faultClaim.inputs.contributions | ForEach-Object { @{ unit = $_.unit; fingerprint = $_.fingerprint } })
        unresolved = @(); coverage = @(); artifacts = @{ report = (Join-Path $skeletonOut 'report.json') }
    } | ConvertTo-Json -Depth 8),$utf8)
    $env:ZT_LEDGER_COMPLETE_FAULT = 'after_outputs'
    Refuses @('ledger','done','-Out',$skeletonOut,'-Unit',$faultClaim.id,'-Token',"$($faultClaim.token)",'-Outputs',$faultPlan) 'injected completion fault'
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    $rolledBack = Run-Digest @('ledger','status','-Out',$skeletonOut,'-Layer','audit')
    $rolledBackUnit = @($rolledBack.units | Where-Object { $_.id -eq $faultClaim.id })[0]
    Assert ($rolledBackUnit.state -eq 'claimed' -and $rolledBackUnit.result -eq $null -and $rolledBack.counts.done -eq 0) 'a publication completion fault rolls its outputs, done state and evidence back together'
    $retried = Run-Digest @('ledger','done','-Out',$skeletonOut,'-Unit',$faultClaim.id,'-Token',"$($faultClaim.token)",'-Outputs',$faultPlan)
    Assert ($retried.state -eq 'done' -and $retried.disposition -eq 'published' -and $retried.unit -ceq $faultPublication.unit) 'the same publication claim completes after the fault is removed'

    # R229: a Bridge publication binds one immutable report and accounts for its
    # exact proposal set. An open proposal is a review decision, not missing work.
    $bridgeOut = $skeletonOut
    $bridge = Run-Digest @('ledger','publish','-Out',$bridgeOut,'-Layer','bridge')
    $bridgeClaim = (Run-Digest @('ledger','next','-Out',$bridgeOut,'-Layer','bridge','-Take','1','-Owner','bridge-manager')).units[0]
    $reportPath = Join-Path $bridgeOut 'bridge-report.json'
    [IO.File]::WriteAllText($reportPath,(@{
        schema = 1; citation_key = 'FaultBook'; created_at = '2026-01-01T00:00:00.0000000Z'
        proposals = @(
            @{ id = 'b001'; carrier = 'FaultBook/1'; status = 'open'; detail = 'Not accepted or applied.' },
            @{ id = 'b002'; carrier = 'FaultBook/1'; status = 'refused'; detail = 'no earlier book' }
        )
    } | ConvertTo-Json -Depth 8 -Compress),$utf8)
    $report = Get-Content -LiteralPath $reportPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $report['report_id'] = 'r-' + (Get-ContentHash $report @('report_id','created_at'))
    [IO.File]::WriteAllText($reportPath,($report | ConvertTo-Json -Depth 8 -Compress),$utf8)
    $bridgePlan = Join-Path $bridgeOut ("publication-{0}.json" -f ($bridgeClaim.id -replace '[/@]','-'))
    function Bridge-Plan($coverage) {
        [IO.File]::WriteAllText($bridgePlan,(@{
            schema_version = 1; kind = 'publication_completion'; unit = $bridgeClaim.id; token = $bridgeClaim.token
            layer = 'bridge'; key = 'book'; input_hash = $bridgeClaim.input_hash
            disposition = 'published'; consumed = @($bridgeClaim.inputs.contributions | ForEach-Object { @{ unit = $_.unit; fingerprint = $_.fingerprint } })
            unresolved = @(); coverage = $coverage; artifacts = @{ report = $reportPath }
        } | ConvertTo-Json -Depth 8),$utf8)
        return $bridgePlan
    }
    $intact = [IO.File]::ReadAllBytes($reportPath)
    $tampered = Get-Content -LiteralPath $reportPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $tampered['status'] = 'done'
    [IO.File]::WriteAllText($reportPath,($tampered | ConvertTo-Json -Depth 8 -Compress),$utf8)
    Refuses @('ledger','done','-Out',$bridgeOut,'-Unit',$bridgeClaim.id,'-Token',"$($bridgeClaim.token)",'-Outputs',(Bridge-Plan @(@{ref='b001';outcome='open';reason=''},@{ref='b002';outcome='refused';reason='no earlier book'}))) 'does not match its own content'
    [IO.File]::WriteAllBytes($reportPath,$intact)
    Refuses @('ledger','done','-Out',$bridgeOut,'-Unit',$bridgeClaim.id,'-Token',"$($bridgeClaim.token)",'-Outputs',(Bridge-Plan @(@{ref='b001';outcome='open';reason=''}))) 'classify every proposal in the report exactly once'
    $bridgeDone = Run-Digest @('ledger','done','-Out',$bridgeOut,'-Unit',$bridgeClaim.id,'-Token',"$($bridgeClaim.token)",'-Outputs',(Bridge-Plan @(@{ref='b001';outcome='open';reason=''},@{ref='b002';outcome='refused';reason='no earlier book'})))
    Assert ($bridgeDone.state -eq 'done' -and @($bridgeDone.coverage).Count -eq 2 -and @($bridgeDone.coverage | Where-Object { $_.ref -eq 'b001' -and $_.outcome -eq 'open' }).Count -eq 1) 'an immutable Bridge report completes with every proposal explicitly accepted, open or refused'

    # An excluded layer can never gain a publication revision, and a figures
    # revision must classify every recognized source reference it was given.
    Refuses @('ledger','publish','-Out',$bridgeOut,'-Layer','figures') 'excluded layer of this prepared run'
    $figureOut = Join-Path $scratch 'figures-enabled'
    $figureOptions = Join-Path $scratch 'figure-options.json'
    [IO.File]::WriteAllText($figureOptions,'{"terms":false,"models":false,"events":false,"findings":false,"bridge":false}',$utf8)
    $null = Run-Digest @('prepare','-Source',$secondSource,'-Out',$figureOut,'-CitationKey','FigureBook','-BibTeX','@book{FigureBook,title={Figure book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-Variant','econ','-RunOptions',$figureOptions,'-Apply')
    $figureGroup = (Run-Digest @('ledger','next','-Out',$figureOut,'-Layer','digest','-Take','1','-Owner','figure-manager')).units[0]
    [IO.File]::WriteAllText($fault,(@{code='source_unavailable';unit=$figureGroup.id;input_hash=$figureGroup.input_hash;cause='the fixture refuses this group'} | ConvertTo-Json),$utf8)
    $null = Run-Digest @('ledger','fail','-Out',$figureOut,'-Unit',$figureGroup.id,'-Token',"$($figureGroup.token)",'-Class','content','-Fault',$fault)
    $figurePublication = Run-Digest @('ledger','publish','-Out',$figureOut,'-Layer','figures')
    $figureClaim = (Run-Digest @('ledger','next','-Out',$figureOut,'-Layer','figures','-Take','1','-Owner','figure-manager')).units[0]
    [IO.File]::WriteAllText((Join-Path $figureOut 'figure-plan.json'),(@{ fixture = 'figures' } | ConvertTo-Json),$utf8)
    $figurePlanPath = Join-Path $figureOut ("publication-{0}.json" -f ($figureClaim.id -replace '[/@]','-'))
    function Figure-Plan($coverage) {
        [IO.File]::WriteAllText($figurePlanPath,(@{
            schema_version = 1; kind = 'publication_completion'; unit = $figureClaim.id; token = $figureClaim.token
            layer = 'figures'; key = 'book'; input_hash = $figureClaim.input_hash
            disposition = 'partial'; consumed = @(); unresolved = @($figureClaim.inputs.dispositions | ForEach-Object { @{ unit = $_.unit } })
            coverage = $coverage; artifacts = @{ report = (Join-Path $figureOut 'figure-plan.json') }
        } | ConvertTo-Json -Depth 8),$utf8)
        return $figurePlanPath
    }
    Refuses @('ledger','done','-Out',$figureOut,'-Unit',$figureClaim.id,'-Token',"$($figureClaim.token)",'-Outputs',(Figure-Plan @(@{ref='图1-1';outcome='shown';reason='';sha256=('0'*64);path='x.png'}))) 'classify every recognized reference exactly once'
    $figureDone = Run-Digest @('ledger','done','-Out',$figureOut,'-Unit',$figureClaim.id,'-Token',"$($figureClaim.token)",'-Outputs',(Figure-Plan @()))
    Assert ($figureDone.state -eq 'done' -and @($figureDone.coverage).Count -eq 0 -and $figureDone.unit -ceq $figurePublication.unit) 'a source with no recognized reference completes with an empty checked figure coverage'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
