#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-options-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest; $env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
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
    [IO.File]::WriteAllText($source,"# 导论`n`n融资约束影响生产选择。`n`n# 第二章`n`n资本投入影响产量。`n`n# 参考`n未选择。",$utf8)
    $options = Join-Path $scratch 'run-options.json'
    [IO.File]::WriteAllText($options,'{"terms":false,"models":false,"events":false,"findings":false,"hubs":false,"figures":false,"bridge":false}',$utf8)
    $manifest = Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','OptionsBook','-BibTeX','@book{OptionsBook,title={Options book}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-MaxUnits','1','-NoFigures','-Variant','econ','-RunOptions',$options,'-Apply')
    Assert ($manifest.run_contract.options.run_options.terms -eq $false -and $manifest.run_contract.options.run_options.models -eq $false) 'reliable preparation freezes effective declared run-layer choices'
    $status = Run-Digest @('ledger','status','-Out',$out)
    Assert ('terms' -in $status.excluded_layers -and 'models' -in $status.excluded_layers -and 'explain' -in $status.excluded_layers -and 'bridge' -in $status.excluded_layers -and $status.run_options.terms -eq $false -and $status.citation_key -eq 'OptionsBook') 'excluded work, frozen choices and model-derived explanations are explicit in the declared target'
    Assert (@($status.units | Where-Object layer -In @('terms','bridge')).Count -eq 0 -and @($status.units | Where-Object state -EQ done).Count -eq 1) 'disabled obligations do not become fabricated successful units'
    $next = Run-Digest @('ledger','next','-Out',$out,'-Layer','digest','-Take','1','-Owner','options-manager')
    Assert ($next.units.Count -eq 1 -and $next.units[0].id -eq 'digest/g001@1') 'explicitly disabled Terms do not block a real registered group claim'
    $excluded = Run-Digest @('ledger','gate','-Out',$out,'-Layer','terms')
    Assert ($excluded.excluded -and $excluded.settled -and -not $excluded.complete) 'a disabled Terms gate reports exclusion instead of completion'
    [IO.File]::WriteAllText($options,'{"terms":true}',$utf8)
    Remove-Item -LiteralPath $options
    $init = Run-Digest @('ledger','init','-Out',$out)
    Assert ($init.run_id -eq $status.run_id -and -not $init.initialized) 'resume uses the retained effective choices without reconstructing an options file'
    $path = Join-Path $out 'manifest.json'; $original = [IO.File]::ReadAllBytes($path)
    $manifest.run_contract.options.run_options.terms = $true
    [IO.File]::WriteAllText($path,($manifest | ConvertTo-Json -Depth 70),$utf8)
    $changed = @(& $Digest ledger init -Out $out 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($changed -join ' ').Contains('immutable preparation mismatch')) 'a layer cannot be enabled or disabled by changing the frozen manifest during recovery'
    [IO.File]::WriteAllBytes($path,$original)
    foreach ($case in @(
        @{json='{"audit":false}';error='unknown run option';name='core-check'},
        @{json='{"terms":"false"}';error='must be boolean';name='wrong-type'},
        @{json='{"terms":false,"terms":true}';error='duplicate JSON key';name='duplicate'},
        @{json='{"proof":true}';error='not available for econ';name='wrong-variant'},
        @{json='{"figures":true}';error='conflicts with NoFigures';name='figure-conflict'}
    )) {
        [IO.File]::WriteAllText($options,$case.json,$utf8)
        $badOut = Join-Path $scratch $case.name; $key="Bad-$($case.name)"
        $bad = @(& $Digest prepare -Source $source -Out $badOut -CitationKey $key -BibTeX "@book{$key,title={Bad fixture}}" -MinUnitChars 1 -NoFigures -Variant econ -RunOptions $options -Apply 2>&1)
        Assert ($LASTEXITCODE -eq 1 -and ($bad -join ' ').Contains($case.error)) "invalid declared choice refuses $($case.name)"
        Assert (-not (Test-Path $badOut) -and $key -notin (Invoke-Zt @(@{tool='list_trees';arguments=@{}})).trees.tree) 'invalid run options publish neither prepared files nor archive setup'
    }
    $hideAudit = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-options-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);const target=JSON.parse(db.prepare("select value from meta where key='target'").get().value);
target.enabled=target.enabled.filter(x=>x!=='audit');target.excluded.push('audit');db.prepare("update meta set value=? where key='target'").run(JSON.stringify(target));db.close();
'@
    & node --no-warnings -e $hideAudit (Join-Path $out 'ledger.sqlite3')
    if ($LASTEXITCODE) { throw 'isolated declared-target injection failed' }
    foreach ($command in @('init','status')) {
        $hidden = @(& $Digest ledger $command -Out $out 2>&1)
        Assert ($LASTEXITCODE -eq 1 -and ($hidden -join ' ').Contains('declared target mismatch')) "$command refuses a metadata change that would hide required work"
    }
    $out = Join-Path $scratch 'legacy-prepared'
    $legacy = Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','LegacyOptions','-BibTeX','@book{LegacyOptions,title={Legacy options}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','econ','-Apply')
    $before = Run-Digest @('ledger','status','-Out',$out)
    # Simulate the already-published schema-1 contract before optional choices existed.
    $legacyShape = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');const fs=require('node:fs');const crypto=require('node:crypto');
const p=path.resolve(process.argv[1]);if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-options-test-'))throw new Error('not the isolated ledger');
const canonical=v=>Array.isArray(v)?v.map(canonical):v&&typeof v==='object'?Object.fromEntries(Object.keys(v).sort().map(k=>[k,canonical(v[k])])):v;
const pack=v=>JSON.stringify(canonical(v));const hash=v=>crypto.createHash('sha256').update(pack(v)).digest('hex');
const manifestFile=path.join(path.dirname(p),'manifest.json');const manifest=JSON.parse(fs.readFileSync(manifestFile,'utf8'));
delete manifest.run_contract.options.run_options;fs.writeFileSync(manifestFile,JSON.stringify(manifest));
const contract=manifest.run_contract;const fingerprint=hash(contract);const db=new DatabaseSync(p);db.exec('pragma foreign_keys=on;begin');
const meta=db.prepare('update meta set value=? where key=?');meta.run(pack(contract),'preparation');meta.run(pack(fingerprint),'preparation_hash');meta.run(pack(contract.options),'effective_options');
const update=db.prepare('update units set inputs_json=?,input_hash=? where id=?');
for(const unit of db.prepare('select id,inputs_json from units').all()){const input=JSON.parse(unit.inputs_json);if(Object.hasOwn(input,'preparation_hash')){input.preparation_hash=fingerprint;update.run(pack(input),hash(input),unit.id)}}
const output=db.prepare("select data_json from outputs where unit_id='prepare/book@1'").get();const facts=JSON.parse(output.data_json);facts.preparation_hash=fingerprint;
db.prepare("update outputs set fingerprint=?,data_json=? where unit_id='prepare/book@1'").run(fingerprint,pack(facts));db.exec('commit');db.close();
'@
    & node --no-warnings -e $legacyShape (Join-Path $out 'ledger.sqlite3')
    if ($LASTEXITCODE) { throw 'legacy optional-field fixture failed' }
    $compatible = Run-Digest @('ledger','init','-Out',$out)
    $oldStatus = Run-Digest @('ledger','status','-Out',$out)
    Assert ($compatible.run_id -eq $before.run_id -and -not $compatible.initialized -and $oldStatus.run_options.terms -eq $true -and $oldStatus.run_options.figures -eq $false) 'a compatible earlier schema-1 ledger retains its run identity and original default target'
    $out = Join-Path $scratch 'math-prepared'
    [IO.File]::WriteAllText($options,'{"terms":false,"notation":false,"boundary":false,"proof":false,"theory":false}',$utf8)
    $math = Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey','MathOptions','-BibTeX','@book{MathOptions,title={Math options}}','-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-NoFigures','-Variant','math','-RunOptions',$options,'-Apply')
    $mathStatus = Run-Digest @('ledger','status','-Out',$out)
    Assert ('notation' -in $mathStatus.excluded_layers -and 'proof' -in $mathStatus.excluded_layers -and 'theory' -in $mathStatus.excluded_layers -and 'math-check' -in $mathStatus.enabled_layers) 'mathematical choices exclude their declared layers while retaining required final checks'
    Assert (@($mathStatus.units | Where-Object layer -In @('notation','theory','terms')).Count -eq 0 -and $mathStatus.coverage_faults.Count -eq 0) 'disabled mathematical root obligations create neither successful rows nor false missing-root faults'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
