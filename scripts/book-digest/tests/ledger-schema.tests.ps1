#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-schema-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
$out = Join-Path $scratch 'prepared'
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
$schemaInjector = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-schema-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.exec(process.argv[2]);db.close();
'@
function Mutate([string]$SQL) { & node --no-warnings -e $schemaInjector (Join-Path $out 'ledger.sqlite3') $SQL; if ($LASTEXITCODE) { throw 'isolated schema fault injection failed' } }
function Refuses([string]$Expected) {
    $lines = @(& $Digest ledger init -Out $out 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($lines -join ' ').Contains($Expected)) "init refuses $Expected"
    $lines = @(& $Digest ledger status -Out $out 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($lines -join ' ').Contains($Expected)) "offline status reports $Expected without repairing evidence"
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source,"# 导论`n`n融资约束影响生产选择。`n`n# 参考`n未选择。",$utf8)
    $prepared = @(& $Digest prepare -Source $source -Out $out -CitationKey SchemaBook -BibTeX '@book{SchemaBook,title={Schema book}}' -StartHeading '导论' -EndHeading '参考' -MinUnitChars 1 -NoFigures -Variant econ -Apply 2>&1)
    if ($LASTEXITCODE) { throw "prepare failed: $($prepared -join ' ')" }
    Mutate 'PRAGMA user_version=2'
    Refuses 'incompatible ledger schema'
    Mutate 'PRAGMA user_version=1'
    Mutate 'DROP INDEX units_ready'
    Refuses 'ledger index mismatch'
    Mutate 'CREATE INDEX units_ready ON units(layer,state,retry_after)'
    Mutate 'DROP TRIGGER events_no_delete'
    Refuses 'ledger event protection mismatch'
    Mutate "CREATE TRIGGER events_no_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT,'events are append-only'); END"
    Mutate 'CREATE TABLE unexpected_jobs(id TEXT PRIMARY KEY)'
    Refuses 'ledger table mismatch'
    Mutate 'DROP TABLE unexpected_jobs'
    Mutate 'PRAGMA journal_mode=delete'
    Refuses 'ledger journal mode mismatch'
    Mutate 'PRAGMA journal_mode=wal'
    $healthy = @(& $Digest ledger init -Out $out 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($healthy[-1] | ConvertFrom-Json).initialized -eq $false) 'repairing only the injected schema faults restores verification of the original run'
    Mutate 'BEGIN; ALTER TABLE deps RENAME TO old_deps; CREATE TABLE deps(unit_id TEXT,needs_id TEXT,condition TEXT,PRIMARY KEY(unit_id,needs_id)); INSERT INTO deps SELECT * FROM old_deps; DROP TABLE old_deps; CREATE INDEX deps_needs ON deps(needs_id); COMMIT'
    Refuses 'ledger foreign key mismatch'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
