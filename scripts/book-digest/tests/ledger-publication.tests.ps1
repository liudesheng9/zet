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
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
