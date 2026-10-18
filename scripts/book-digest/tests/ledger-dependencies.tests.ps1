#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-dependencies-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
$out = Join-Path $scratch 'prepared'
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) { throw "digest failed: $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source,"# 导论`n`n融资约束影响生产选择。`n`n# 参考`n未选择。",$utf8)
    $manifest = Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','DependencyBook','-BibTeX','@book{DependencyBook,title={Dependency book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','econ','-Apply')
    # A stopped/local ledger fixture supplies pending graph nodes, not domain completion evidence.
    $fixture = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');const crypto=require('node:crypto');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-dependencies-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.exec('pragma foreign_keys=on;begin');
const add=db.prepare("insert into units(id,layer,logical_key,revision,state,inputs_json,input_hash,created_by,created_at) values(?,'models',?,1,'registered',?,?,'prepare/book@1',?)");
for(const key of ['a','b','c','waiting','free']){const input=JSON.stringify({model:key});add.run('models/'+key+'@1',key,input,crypto.createHash('sha256').update(input).digest('hex'),new Date().toISOString())}
const dep=db.prepare('insert into deps values(?,?,?)');
dep.run('models/a@1','models/b@1','success');dep.run('models/b@1','models/a@1','success');dep.run('models/waiting@1','models/a@1','success');dep.run('models/free@1','prepare/book@1','success');
dep.run('models/a@1','models/c@1','success');dep.run('models/c@1','models/b@1','success');
db.exec('commit');db.close();
'@
    & node --no-warnings -e $fixture (Join-Path $out 'ledger.sqlite3')
    if ($LASTEXITCODE) { throw 'isolated dependency fixture failed' }
    $next = Run-Digest @('ledger','next','-Out',$out,'-Layer','models','-Take','4','-Owner','graph-manager')
    Assert ($next.units.Count -eq 1 -and $next.units[0].id -eq 'models/free@1') 'a dependency cycle does not prevent an independent unit from being claimed'
    $status = Run-Digest @('ledger','status','-Out',$out,'-Layer','models')
    $cycle = @($status.units | Where-Object id -In @('models/a@1','models/b@1','models/c@1'))
    Assert ($cycle.Count -eq 3 -and @($cycle | Where-Object { $_.state -eq 'failed' -and $_.fault.code -eq 'dependency_cycle' -and $_.attempts -eq 0 }).Count -eq 3) 'every producer in overlapping cycles becomes an explicit content fault without spending attempts'
    foreach ($unit in $cycle) {
        $validEdges = $true
        for ($i=0; $i -lt $unit.fault.cycle.Count-1; $i++) {
            $from=$unit.fault.cycle[$i]; $to=$unit.fault.cycle[$i+1]
            if (@($status.dependencies | Where-Object { $_.unit -eq $from -and $_.needs -eq $to }).Count -ne 1) { $validEdges=$false }
        }
        Assert ($validEdges -and $unit.fault.cycle.Count -ge 3 -and $unit.fault.cycle[0] -eq $unit.fault.cycle[-1] -and $unit.id -in $unit.fault.cycle -and 'models/a@1' -in $unit.fault.cycle -and 'models/b@1' -in $unit.fault.cycle) "the exact closed dependency cycle is named for $($unit.id)"
    }
    $waiting = @($status.units | Where-Object id -EQ 'models/waiting@1')[0]
    Assert ($waiting.state -eq 'blocked' -and $waiting.attempts -eq 0 -and $waiting.fault.prerequisites[0] -eq 'models/a@1') 'a dependant outside the cycle is blocked with its exact prerequisite'
    $again = Run-Digest @('ledger','next','-Out',$out,'-Layer','models','-Take','4','-Owner','graph-manager-2')
    Assert ($again.units.Count -eq 0 -and -not $again.complete) 'cyclic work cannot become an endless ready/retry queue or empty-queue completion'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
