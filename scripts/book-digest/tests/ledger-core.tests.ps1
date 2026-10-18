#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-core-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
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
 tables:db.prepare("select name from sqlite_master where type='table' and name not like 'sqlite_%' order by name").all().map(x=>x.name),
 journal:db.prepare('pragma journal_mode').get().journal_mode,
 meta:db.prepare('select key,value from meta order by key').all(),
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
    [IO.File]::WriteAllText($source,"# 导论`n`n企业以资本生产商品，交易双方选择价格。`n`n# 第二章`n`n融资约束限制生产投入，从而改变交易。`n`n# 参考`n未选择。",$utf8)
    $out = Join-Path $scratch 'prepared'
    $manifest = Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','CoreBook','-BibTeX','@book{CoreBook,title={Core book}}','-StartHeading','导论','-EndHeading','参考','-MaxUnits','1','-MinUnitChars','1','-NoFigures','-NoEventCandidates','-Variant','econ','-Apply')
    $init = Run-Digest @('ledger','init','-Out',$out)
    Assert ($init.run_id -match '^[0-9a-f]{32}$' -and $init.schema_version -eq 1) 'init creates a concrete schema and random retained run identity'
    $status = Run-Digest @('ledger','status','-Out',$out)
    $groups = @($status.units | Where-Object layer -EQ digest)
    Assert ($groups.Count -eq $manifest.groups.Count -and $groups.Count -ge 2 -and @($groups | Where-Object state -EQ done).Count -eq 0) 'all prepared groups are registered and skeleton anchors never establish done coverage'
    Assert ($status.complete -eq $false -and @($status.units | Where-Object { $_.layer -eq 'prepare' -and $_.state -eq 'done' }).Count -eq 1) 'only checked preparation is done while the declared book target stays incomplete'
    Assert ('figures' -in $status.excluded_layers -and 'events' -notin $status.excluded_layers) 'disabled figures are explicit while disabling date extraction does not disable Event work'
    $evidence = Evidence
    $protectedGroup = [IO.File]::ReadAllBytes($manifest.groups[0].file)
    $repeatPrepare = @(& $Digest prepare -Source $source -Out $out -CitationKey CoreBook -BibTeX '@book{CoreBook,title={Core book}}' -Variant econ -Apply 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($repeatPrepare -join ' ').Contains('already initialized')) 'preparation refuses to regenerate an initialized output directory'
    Assert ([Convert]::ToBase64String([IO.File]::ReadAllBytes($manifest.groups[0].file)) -ceq [Convert]::ToBase64String($protectedGroup) -and (Evidence) -ceq $evidence) 'the early preparation guard preserves frozen files and ledger rows'
    $db = $evidence | ConvertFrom-Json -AsHashtable
    Assert (($db.tables -join ',') -ceq 'deps,events,meta,outputs,units' -and $db.journal -eq 'wal') 'one WAL database has exactly the five contracted tables'
    Assert (@($db.deps | Where-Object condition -EQ success).Count -gt 0 -and @($db.deps | Where-Object condition -EQ settled).Count -ge $groups.Count) 'root dependencies retain distinct success and settled semantics'
    $again = Run-Digest @('ledger','init','-Out',$out)
    Assert ($again.run_id -ceq $init.run_id -and -not $again.initialized -and (Evidence) -ceq $evidence) 'repeated init verifies the existing run without duplicating units or events'
    $sourceBytes = [IO.File]::ReadAllBytes($source)
    [IO.File]::AppendAllText($source,"`n改变了来源。",$utf8)
    Refuses @('ledger','init','-Out',$out) 'source hash mismatch'
    Refuses @('ledger','next','-Out',$out,'-Layer','terms','-Take','1','-Owner','mismatch-manager') 'source hash mismatch'
    Assert ((Evidence) -ceq $evidence) 'a source mismatch leaves every ledger row unchanged'
    [IO.File]::WriteAllBytes($source,$sourceBytes)
    $groupFile = $manifest.groups[0].file
    $groupBytes = [IO.File]::ReadAllBytes($groupFile)
    [IO.File]::AppendAllText($groupFile,"`n改变了小组输入。",$utf8)
    Refuses @('ledger','init','-Out',$out) 'group hash mismatch'
    Assert ((Evidence) -ceq $evidence) 'a changed prepared group cannot reinterpret an existing unit'
    [IO.File]::WriteAllBytes($groupFile,$groupBytes)
    $manifestPath = Join-Path $out 'manifest.json'
    $originalManifest = [IO.File]::ReadAllBytes($manifestPath)
    $changed = $manifest | ConvertTo-Json -Depth 70 | ConvertFrom-Json -AsHashtable
    $changed.run_contract.contracts.base_version = '999.0.0'
    [IO.File]::WriteAllText($manifestPath,($changed | ConvertTo-Json -Depth 70),$utf8)
    Refuses @('ledger','init','-Out',$out) 'incompatible workflow contract'
    Assert ((Evidence) -ceq $evidence) 'an incompatible workflow refuses before any ledger transition'
    [IO.File]::WriteAllBytes($manifestPath,$originalManifest)
    $changed.run_contract.contracts.base_version = $manifest.run_contract.contracts.base_version
    $changed.run_contract.archive_identity.id = '00000000000000000000000000000000'
    [IO.File]::WriteAllText($manifestPath,($changed | ConvertTo-Json -Depth 70),$utf8)
    Refuses @('ledger','init','-Out',$out) 'archive identity mismatch'
    Assert ((Evidence) -ceq $evidence) 'reusing the same path with another archive identity is refused'
    [IO.File]::WriteAllBytes($manifestPath,$originalManifest)
    $statusAgain = Run-Digest @('ledger','status','-Out',$out,'-Layer','digest')
    Assert ($statusAgain.units.Count -eq $groups.Count -and (Evidence) -ceq $evidence) 'status is a read-only layer-scoped projection of durable coverage'
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_IDENTITY_FAULT = 'after_read'
    Refuses @('ledger','next','-Out',$out,'-Layer','terms','-Take','1','-Owner','raced-manager') 'archive changed while checking preparation'
    Assert ((Evidence) -ceq $evidence) 'an identity change during the actual read cannot publish a claim'
    Remove-Item Env:ZT_IDENTITY_FAULT,Env:ZT_CHECK_ZT
    $invalid = @(& $Digest ledger status 2>&1)
    Assert ($LASTEXITCODE -eq 2 -and ($invalid -join ' ').Contains('-Out is required')) 'ledger commands require an explicit output directory and distinguish invalid invocation'
    zt down | Out-Null
    $env:ZT_CHECK_ZT = Join-Path $scratch 'unavailable-zt.exe'
    $offline = Run-Digest @('ledger','status','-Out',$out)
    Assert ($offline.run_id -eq $init.run_id -and (Evidence) -ceq $evidence) 'read-only ledger status remains available without an archive service or executable'
    Remove-Item Env:ZT_CHECK_ZT
} finally {
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_IDENTITY_FAULT -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
