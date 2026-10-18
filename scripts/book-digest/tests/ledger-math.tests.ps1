#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\ledger-math.tests.ps1 [-Zt <zt.exe>]
# The mathematics variant's ledger adapter: a real `prepare -Variant math -Apply` on a scratch
# archive and a two-chapter fixture book, the real `zt-digest math` steps standing in for DeepSeek's
# bodies and JSON answers, and `zt-digest ledger` on the same staging folder. It checks the
# obligations DG16 names: every indexed statement accounted for, notation before dependent work,
# one tracked boundary candidate at a time, and Theory/math-check publication revisions.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-math-test-$PID"))
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
    if ($LASTEXITCODE) { throw "digest failed ($LASTEXITCODE): $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Refuses([string[]]$Arguments,[string]$Expected) {
    $lines = @(& $Digest @Arguments 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($lines -join ' ').Contains($Expected)) "refuses $Expected"
}
function Math([string]$Step,[string[]]$more = @()) { Run-Digest (@('math','-Step',$Step,'-Staging',$out) + $more) }
function Claim([string]$Layer) { Run-Digest @('ledger','next','-Out',$out,'-Layer',$Layer,'-Take','4','-Owner',"math-$Layer") }
function Complete([string]$Layer,[string]$Unit,[string]$Outputs) {
    $claim = Claim $Layer
    $mine = @($claim.units | Where-Object id -CEQ $Unit)
    if ($mine.Count -ne 1) { throw "cannot claim $Unit (got $((@($claim.units | ForEach-Object id)) -join ', '))" }
    Run-Digest @('ledger','done','-Out',$out,'-Unit',$Unit,'-Token',$mine[0].token,'-Outputs',$Outputs)
}
function Write-Calls($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "zt: $($lines -join ' | ')" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Save-Json($path, $value) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
    [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 10), $utf8)
}
function Read-Card($at) { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] }
function Read-Model($at) { @(Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $at } }))[0] }
$key = 'Abbott2015'
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source,@'
# 导论

本书讲实数与连续。

# 第 1 章 实数

**公理 1.3.3**（完备性公理）有上界的非空实数集有上确界。

# 第 2 章 函数极限与连续

**定义 4.3.1**（连续）设 $f:A\to\mathbb{R}$，$x_0\in A$。若对每个 $\varepsilon>0$ 都存在 $\delta>0$，则称 $f$ 在 $x_0$ 连续。记作 $f\in C(A)$。

**定理 4.5.1**（介值定理）设 $f$ 在 $[a,b]$ 上连续，$f(a)<c<f(b)$。则存在 $x_0$ 使 $f(x_0)=c$。

证明：令 $S=\{x: f(x)<c\}$。由公理 1.3.3，$S$ 有上确界。

# 参考

未选择。
'@,$utf8)
    $null = Run-Digest @('prepare','-Source',$source,'-Out',$out,'-CitationKey',$key,'-BibTeX',"@book{$key,title={Understanding Analysis}}",'-StartHeading','导论','-EndHeading','参考','-MinUnitChars','1','-MaxUnits','1','-NoFigures','-Variant','math','-Apply')

    # --- index, docs and the skeleton: allocation is not a built unit -------------------------------
    $index = Math 'index'
    Assert ($index.units -eq 3 -and $index.forms.axiom -eq 1 -and $index.forms.definition -eq 1 -and $index.forms.theorem -eq 1) 'the index finds the book statements'
    $null = Math 'docs'
    $sk = Math 'skeleton' @('-Apply')
    Assert ($sk.units -eq 3 -and $sk.theories -eq 2) 'the skeleton allocates three units and two Theory Cards'
    $units = (Get-Content -Raw (Join-Path $out 'math\units.json') | ConvertFrom-Json -AsHashtable)
    $at = @{}
    foreach ($u in $units.units) { $at[$u.id] = $u.address }
    $axiom = $at['公理 1.3.3']; $def = $at['定义 4.3.1']; $th = $at['定理 4.5.1']
    $theory1 = @($units.theories | Where-Object { $_.chapter -match '第 1 章' })[0].address
    $theory2 = @($units.theories | Where-Object { $_.chapter -match '第 2 章' })[0].address
    Assert ((Read-Card $th).title -match '待写' -and (Read-Card $theory2).title -match '待写') 'allocation leaves the statement and the Theory Card open: both still hold 待写'

    # The digest and model stages, in place of DeepSeek and the model compiler.
    $null = Write-Calls @(
        @{ tool = 'edit_card'; arguments = @{ at = $axiom; title = '完备性公理：有上界的非空实数集有上确界'; body = "**公理**：`n- (C1) 有上界的非空实数集有上确界。`n`n来源：[[$key]] 公理 1.3.3" } },
        @{ tool = 'edit_card'; arguments = @{ at = $def; title = '连续：ε-δ 条件成立'; body = "**定义**：`n- (C1) `$x_0\in A`$。`n- (C2) 对每个 `$\varepsilon>0`$ 都存在 `$\delta>0`$。`n`n来源：[[$key]] 定义 4.3.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = $def; kind = 'side'; facet = '例子'; emphasis = 'core'; title = '例子：x^2 处处连续'; body = "例示 [[$def]]。" } },
        @{ tool = 'create_successor'; arguments = @{ at = $def; kind = 'side'; facet = '直觉'; emphasis = 'core'; title = '直觉：图像不断开'; body = '图像不断开。' } },
        @{ tool = 'edit_card'; arguments = @{ at = $th; title = '介值定理：连续函数取遍中间值'; body = "**陈述**：`n- (H1) f 在 [a,b] 上连续。`n- (H2) f(a)<c<f(b)。`n- 结论：存在 x0 使 f(x0)=c。`n`n**思路**：取上确界，再用反证法排除两种情形。`n`n来源：[[$key]] 定理 4.5.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = $th; kind = 'side'; facet = '前提'; title = '前提：f 连续，f(a)<c<f(b)'; body = '(H1) 与 (H2)。' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@5'; facet = '证明思路'; emphasis = 'core'; title = '证明思路：取上确界再反证'; body = "依赖 [[@@5]]。方法：反证。" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@6'; facet = '证明：S 有上确界 x0'; title = '证明：S 有上确界 x0'; body = "依赖 [[@@6]]。依赖 [[$axiom]]：S 有上确界。由 (H2)。" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@7'; facet = '证明：f(x0)<c 不成立'; emphasis = 'core'; title = '证明：f(x0)<c 不成立'; body = "依赖 [[@@7]]。由 (H1)。" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@8'; facet = '证明：f(x0)>c 不成立'; title = '证明：f(x0)>c 不成立'; body = "依赖 [[@@8]]。" } },
        @{ tool = 'create_successor'; arguments = @{ at = $th; kind = 'side'; facet = '直觉'; emphasis = 'core'; title = '直觉：图像必须穿过直线'; body = '图像从下方走到上方。' } })

    # --- notation: applied and available before the work that depends on it -------------------------
    $nplan = Math 'notation'
    Assert ($nplan.candidates -ge 1) 'the notation job enumerates the symbol candidates'
    Save-Json (Join-Path $out 'math\notation\out\notation.json') @{ symbols = @(@{ symbol = '$C(A)$'; meaning = 'A 上的连续函数'; unit = '定义 4.3.1' }) }
    $napplied = Math 'notation-apply' @('-Apply')
    Assert ($napplied.symbols -eq 1 -and $napplied.card) 'notation-apply writes the Book notation table'

    # --- boundary and proof plans register one obligation per mathematical unit ---------------------
    $bplan = Math 'boundary'
    Assert ($bplan.units -eq 2) 'the boundary plan names the units that need hypothesis roles'
    $registered = Run-Digest @('ledger','math','-Out',$out)
    Assert ($registered.reused -eq $false -and @($registered.units | Where-Object layer -eq 'boundary').Count -eq 2) 'the boundary plan registers one ledger obligation per planned unit'
    $status = Run-Digest @('ledger','status','-Out',$out,'-Layer','boundary')
    $role = "boundary/$th@1"
    Assert (@($status.dependencies | Where-Object { $_.unit -eq $role -and $_.needs -eq 'notation/book@1' }).Count -eq 1) 'notation is a prerequisite of the boundary work that depends on it'
    Assert (@($status.dependencies | Where-Object { $_.unit -eq $role -and $_.needs -eq 'math-skeleton/book@1' }).Count -eq 1) 'the skeleton is a prerequisite of its unit obligations'
    $again = Run-Digest @('ledger','math','-Out',$out)
    Assert ($again.reused -eq $true -and @($again.units).Count -eq @($registered.units).Count) 'the same plan registers no second obligation'
    $declared = Run-Digest @('ledger','status','-Out',$out)
    Assert (('events' -in $declared.excluded_layers) -and ('findings' -in $declared.excluded_layers) -and (@($declared.units | Where-Object { $_.layer -in @('events','findings') }).Count -eq 0)) 'the mathematics profile declares no Event or Finding layer and registers none of their work'

    # --- a done model build stays done while its own obligations are open ---------------------------
    # A stopped-local fixture supplies the finished Model unit (#72 owns its own verifier); the
    # boundary and proof obligations are real registered work beside it.
    $modelUnit = 'models/g003-m0001@1'
    $modelFixture = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');const crypto=require('node:crypto');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-math-test-'))throw new Error('not the isolated ledger');
const input=JSON.stringify({model:'Abbott2015/3|b'});
const db=new DatabaseSync(p);db.prepare("insert into units(id,layer,logical_key,revision,state,inputs_json,input_hash,created_by,created_at,completed_at,result_json) values(?,'models',?,'1','done',?,?,'prepare/book@1',?,?,'{\"disposition\":\"verified\"}')").run(process.argv[2],process.argv[2],input,crypto.createHash('sha256').update(input).digest('hex'),new Date().toISOString(),new Date().toISOString());db.close();
'@
    & node --no-warnings -e $modelFixture (Join-Path $out 'ledger.sqlite3') $modelUnit
    if ($LASTEXITCODE) { throw 'isolated Model fixture failed' }
    $modelRow = (& node --no-warnings -e "const {DatabaseSync}=require('node:sqlite');const db=new DatabaseSync(process.argv[1],{readOnly:true});console.log(JSON.stringify(db.prepare('select * from units where id=?').get(process.argv[2])));db.close();" (Join-Path $out 'ledger.sqlite3') $modelUnit)
    $rerun = Run-Digest @('ledger','rerun-list','-Out',$out)
    Assert ($modelUnit -notin $rerun.units.id -and $modelUnit -notin $rerun.unresolved -and "boundary/$th@1" -in $rerun.units.id) 'a done model build is not offered to a resume while its boundary obligation is'
    $modelBefore = (Read-Model $th) | ConvertTo-Json -Depth 8 -Compress

    # --- the indexed statement index must account for the prepared sections -------------------------
    $indexFile = Join-Path $out 'math\index.json'
    $good = [IO.File]::ReadAllBytes($indexFile)
    $drifted = (Get-Content -Raw $indexFile | ConvertFrom-Json -AsHashtable)
    $drifted.units = @($drifted.units | Select-Object -First 2)
    [IO.File]::WriteAllText($indexFile, ($drifted | ConvertTo-Json -Depth 20), $utf8)
    $indexClaim = Claim 'math-index'
    $refused = @(& $Digest ledger done -Out $out -Unit math-index/book@1 -Token $indexClaim.units[0].token -Outputs $indexFile 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($refused -join ' ').Contains('does not match the prepared sections')) 'an index that leaves a prepared statement out cannot complete'
    [IO.File]::WriteAllBytes($indexFile, $good)
    $indexDone = Run-Digest @('ledger','done','-Out',$out,'-Unit','math-index/book@1','-Token',$indexClaim.units[0].token,'-Outputs',$indexFile)
    Assert ($indexDone.state -eq 'done' -and $indexDone.units -eq $index.units -and $indexDone.others -eq ($index.examples + $index.exercises)) 'the completed index accounts for every indexed statement of the prepared sections'
    $docs = Complete 'math-docs' 'math-docs/book@1' (Join-Path $out 'math\docs.json')
    Assert ($docs.state -eq 'done' -and $docs.files -eq 5) 'the math prompt files are re-derived from the shared instruction files'

    # --- skeleton allocation is not a built mathematical unit ---------------------------------------
    $groupFile = Join-Path $out 'groups\g03.md'
    $groupBytes = [IO.File]::ReadAllBytes($groupFile)
    $groupText = [Text.Encoding]::UTF8.GetString($groupBytes)
    $atMarker = $groupText.IndexOf('## 本组的数学单元')
    [IO.File]::WriteAllText($groupFile, $groupText.Substring(0, $atMarker), $utf8)
    $skeletonClaim = Claim 'math-skeleton'
    $unlisted = @(& $Digest ledger done -Out $out -Unit math-skeleton/book@1 -Token $skeletonClaim.units[0].token -Outputs (Join-Path $out 'math\units.json') 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($unlisted -join ' ').Contains('carries no mathematical unit list')) 'a group whose unit list is gone cannot be accounted for'
    [IO.File]::WriteAllBytes($groupFile, $groupBytes)
    $skeleton = Run-Digest @('ledger','done','-Out',$out,'-Unit','math-skeleton/book@1','-Token',$skeletonClaim.units[0].token,'-Outputs',(Join-Path $out 'math\units.json'))
    Assert ($skeleton.state -eq 'done' -and $skeleton.disposition -eq 'allocated' -and $skeleton.indexed -eq 3 -and $skeleton.allocated -eq 3 -and $skeleton.theories -eq 2) 'the skeleton accounts for every indexed statement and Theory Card'
    Assert (@($skeleton.groups).Count -eq 3 -and @($skeleton.groups | Where-Object { $_.units -gt 0 }).Count -eq 2) 'each prepared group file carries the unit list of its own units'
    $open = Run-Digest @('ledger','status','-Out',$out)
    Assert (@($open.units | Where-Object { $_.layer -eq 'boundary' -and $_.state -ne 'done' }).Count -eq 2) 'a done skeleton leaves every mathematical unit obligation open'

    # --- required notation is available before the work that depends on it --------------------------
    $blocked = Claim 'boundary'
    Assert (@($blocked.units).Count -eq 0) 'boundary work is not claimable before the notation table is applied'
    [IO.File]::WriteAllText((Join-Path $out 'math\notation\out\notation.json'),'{"symbols":[{"symbol":"$C(A)$","meaning":"别的含义","unit":"定义 4.3.1"}]}',$utf8)
    $notationClaim = Claim 'notation'
    $wrong = @(& $Digest ledger done -Out $out -Unit notation/book@1 -Token $notationClaim.units[0].token -Outputs (Join-Path $out 'math\notation\applied.json') 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($wrong -join ' ').Contains('does not match the answer')) 'a notation table that is not what the retained answer produces cannot complete'
    Save-Json (Join-Path $out 'math\notation\out\notation.json') @{ symbols = @(@{ symbol = '$C(A)$'; meaning = 'A 上的连续函数'; unit = '定义 4.3.1' }) }
    $null = Math 'notation-apply' @('-Apply')
    $notation = Run-Digest @('ledger','done','-Out',$out,'-Unit','notation/book@1','-Token',$notationClaim.units[0].token,'-Outputs',(Join-Path $out 'math\notation\applied.json'))
    Assert ($notation.state -eq 'done' -and $notation.symbols -eq 1 -and $notation.available -eq $true -and $notation.card) 'the applied notation table records its checked symbols and their Card'
    $ready = Claim 'boundary'
    Assert (@($ready.units).Count -eq 2) 'notation applied makes the dependent boundary work claimable'
    $theoremUnit = "boundary/$th@1"
    $definitionUnit = "boundary/$def@1"
    $theoremToken = @($ready.units | Where-Object id -CEQ $theoremUnit)[0].token
    $definitionToken = @($ready.units | Where-Object id -CEQ $definitionUnit)[0].token

    # --- one tracked candidate at a time: supply, apply, independent review, settle -----------------
    $steps = @((Read-Model $th).build.order | Where-Object { $_.line -eq 'main' } | ForEach-Object { $_.address })
    Save-Json (Join-Path $out 'math\boundary\out\b-01.json') @{ items = @(
            @{ unit = $th; hypothesis = 'H1'; title = '假设作用：去掉连续性，函数可以跳过 c'; role = '步骤三用到连续性。'; used_in = $steps[3]; counterexample = @{ title = '反例：符号函数跳过 1/2'; body = '取符号函数。'; supplied = $true } },
            @{ unit = $th; hypothesis = 'H2'; title = '假设作用：c 须在两端值之间'; role = '步骤一用到它。'; used_in = $steps[2]; counterexample = $null },
            @{ unit = $def; non_example = @{ title = '非例：符号函数在 0 处跳跃'; body = '不满足 (C2)。'; supplied = $true } }) }
    $bapplied = Math 'boundary-apply' @('-Supply','examples')
    Assert ($bapplied.created -eq 4 -and @($bapplied.jobs).Count -eq 1) 'boundary-apply creates the parts and one review job for the supplied ones'
    $parts = @((Read-Model $th).parts)
    $role = @($parts | Where-Object { $_.facet -eq '假设作用：H1' })[0].address
    $ce = @($parts | Where-Object { $_.facet -eq '反例：去掉 H1' })[0].address
    $keptRole = @($parts | Where-Object { $_.facet -eq '假设作用：H2' })[0].address
    $nonExample = @((Read-Model $def).parts | Where-Object { $_.facet -eq '非例' })[0].address
    # No independent review yet: the supplied parts are applied but not settled.
    $unsettled = Math 'boundary-settle'
    Assert ($unsettled.unreviewed -eq 2 -and $unsettled.deleted -eq 0) 'an unanswered review leaves every supplied candidate unreviewed and removes nothing'
    $noReview = @(& $Digest ledger done -Out $out -Unit $theoremUnit -Token $theoremToken -Outputs (Join-Path $out 'math\boundary\settled.json') 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($noReview -join ' ').Contains('has no independent review')) "a supplied candidate without its independent review cannot settle ($($noReview -join ' '))"
    Save-Json (Join-Path $out 'math\boundary\out\v-b-01.json') @{ verdicts = @(
            @{ address = $ce; verdict = 'wrong'; reason = '符号函数在 [-1,1] 上取不到 1/2，但它不满足 H2 的端点条件。' },
            @{ address = $nonExample; verdict = 'ok' }) }
    $settled = Math 'boundary-settle'
    Assert ($settled.deleted -eq 2 -and $settled.kept -eq 1) 'the rejected supplied part goes with the role that names it'
    $facets = @((Read-Model $th).parts | ForEach-Object { $_.facet })
    Assert (-not ($facets -contains '反例：去掉 H1') -and -not ($facets -contains '假设作用：H1') -and ($facets -contains '假设作用：H2')) 'only the rejected pair left the model'
    # A record that hides one deletion behind the count cannot complete.
    $settledFile = Join-Path $out 'math\boundary\settled.json'
    $goodSettled = [IO.File]::ReadAllBytes($settledFile)
    $hidden = (Get-Content -Raw $settledFile | ConvertFrom-Json -AsHashtable)
    $hidden.deleted = @($hidden.deleted | Where-Object { $_.address -ne $role })
    [IO.File]::WriteAllText($settledFile, ($hidden | ConvertTo-Json -Depth 20), $utf8)
    $hiddenRefused = @(& $Digest ledger done -Out $out -Unit $theoremUnit -Token $theoremToken -Outputs $settledFile 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($hiddenRefused -join ' ').Contains('was not deleted through the confirmed path')) "a rejected part cannot disappear behind a batch count ($($hiddenRefused -join ' '))"
    [IO.File]::WriteAllBytes($settledFile, $goodSettled)
    $theoremDone = Run-Digest @('ledger','done','-Out',$out,'-Unit',$theoremUnit,'-Token',$theoremToken,'-Outputs',$settledFile)
    Assert ($theoremDone.state -eq 'done' -and $theoremDone.candidates -eq 3 -and $theoremDone.kept -eq 1 -and @($theoremDone.removed).Count -eq 2) 'the settled boundary unit accounts for every candidate it created'
    $defDone = Run-Digest @('ledger','done','-Out',$out,'-Unit',$definitionUnit,'-Token',$definitionToken,'-Outputs',$settledFile)
    Assert ($defDone.state -eq 'done' -and $defDone.kept -eq 1 -and @($defDone.removed).Count -eq 0) 'a kept supplied contrast is settled without a deletion'

    # --- every selected formal result gets its proof review and accepted fixes ----------------------
    $pplan = Math 'proof'
    Assert ($pplan.units -eq 1) 'the proof plan names the formal result with proof steps'
    $proofRegistered = Run-Digest @('ledger','math','-Out',$out)
    Assert (@($proofRegistered.units | Where-Object layer -eq 'proof').Count -eq 1) 'the proof plan registers its own ledger obligation'
    function Review([hashtable]$fixes) {
        $list = @()
        foreach ($s in $script:steps) {
            $list += $(if ($fixes.ContainsKey($s)) { $fixes[$s] } else { @{ address = $s; verdict = 'ok' } })
        }
        Save-Json (Join-Path $out 'math\proof\out\p-01.json') @{ verdicts = $list }
    }
    $last = @($steps)[-1]
    $old = (Read-Card $last).body
    Review @{ $last = @{ address = $last; verdict = 'fix'; reason = '补出 x0>a。'; body = "$old`n由 (H2)，x0>a。" } }
    $papplied = Math 'proof-apply'
    Assert ($papplied.fixed -eq 1 -and $papplied.refused -eq 0) 'proof-apply writes the accepted correction'
    $proofUnit = "proof/$th@1"
    $proofReady = Claim 'proof'
    $proofToken = @($proofReady.units | Where-Object id -CEQ $proofUnit)[0].token
    $proofFile = Join-Path $out 'math\proof\applied.json'
    # A reviewer that answers nothing about one selected step leaves it unreviewed.
    Save-Json (Join-Path $out 'math\proof\out\p-01.json') @{ verdicts = @($steps | Where-Object { $_ -ne $steps[2] } | ForEach-Object { @{ address = $_; verdict = 'ok' } }) }
    $null = Math 'proof-apply'
    $incomplete = @(& $Digest ledger done -Out $out -Unit $proofUnit -Token $proofToken -Outputs $proofFile 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($incomplete -join ' ').Contains('was not reviewed')) "a proof step nobody reviewed cannot complete the unit ($($incomplete -join ' '))"
    Review @{
        $steps[2] = @{ address = $steps[2]; verdict = 'fix'; reason = '删掉链接。'; body = '没有链接的正文。' }
        $last = @{ address = $last; verdict = 'fix'; reason = '补出 x0>a。'; body = "$old`n由 (H2)，x0>a。" } }
    $papplied = Math 'proof-apply'
    Assert ($papplied.fixed -eq 1 -and $papplied.refused -eq 1) 'a correction that drops a Link is refused while the rest are applied'
    $refusal = @(& $Digest ledger done -Out $out -Unit $proofUnit -Token $proofToken -Outputs $proofFile 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($refusal -join ' ').Contains('remains a named obligation')) "an unresolved refusal stays visible instead of completing ($($refusal -join ' '))"
    # The reviewer now accepts a Link-preserving correction for that step, so nothing is open.
    $stepBody = (Read-Card $steps[2]).body
    Review @{ $steps[2] = @{ address = $steps[2]; verdict = 'fix'; reason = '保留链接，补出理由。'; body = "$stepBody`n由 (H1)。" } }
    $papplied = Math 'proof-apply'
    Assert ($papplied.fixed -eq 1 -and $papplied.refused -eq 0) 'the second round accepts only the Link-preserving correction'
    $proofDone = Run-Digest @('ledger','done','-Out',$out,'-Unit',$proofUnit,'-Token',$proofToken,'-Outputs',$proofFile)
    Assert ($proofDone.state -eq 'done' -and $proofDone.steps -eq @($steps).Count -and $proofDone.fixed -eq 1) 'the completed proof unit accounts for every reviewed step'

    # --- the theory revision follows link-check and rebuilds every Theory Card summary ---------------
    $tplan = Math 'theory'
    Assert ($tplan.theories -eq 2 -and @($tplan.jobs).Count -eq 1) 'the theory plan names both Theory Cards'
    Save-Json (Join-Path $out 'math\theory\out\t-01.json') @{ theories = @(
            @{ address = $theory2; title = '连续函数理论：连续性使函数取遍中间值'; question = '连续函数有什么整体性质？'
                main_results = @(@{ address = $th; line = '闭区间上的连续函数取遍中间值。' })
                objects = @(@{ address = $def; line = '一点的连续性。' }) }) }
    $tapplied = Math 'theory-apply' @('-Apply')
    Assert ($tapplied.theories -eq 2) 'theory-apply rebuilds both Theory Cards'
    $theoryApplied = Join-Path $out 'math\theory\applied.json'
    $appliedRecord = (Get-Content -Raw $theoryApplied | ConvertFrom-Json -AsHashtable)
    $record2 = @($appliedRecord.theories | Where-Object { $_.address -ceq $theory2 })[0]
    $model2 = Read-Model $theory2
    $expectedMembers = @($model2.math.order).Count
    Assert (@($record2['members']).Count + @($record2['tools']).Count -eq $expectedMembers) 'the applied theory record accounts for the units its model holds'
    # The publications the theory revision consumes are supplied as done fixtures; their own
    # verifiers are covered by ledger-publication.tests.ps1.
    $upstream = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-math-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.exec('pragma foreign_keys=on;begin');
const done=db.prepare("update units set state='done',completed_at=?,result_json='{\"disposition\":\"published\"}' where id=?");
for(const id of ['digest/g001@1','digest/g002@1','digest/g003@1','link-check/book@1'])done.run(new Date().toISOString(),id);
db.exec('commit');db.close();
'@
    & node --no-warnings -e $upstream (Join-Path $out 'ledger.sqlite3')
    if ($LASTEXITCODE) { throw 'isolated upstream fixture failed' }
    $published = Run-Digest @('ledger','publish','-Out',$out,'-Layer','theory')
    Assert ($published.unit -ceq 'theory/book@2' -and $published.reused -eq $false -and 'theory/book@1' -in $published.superseded) 'a changed input set derives the next theory revision and supersedes the registered one'
    $theoryContribution = @($published.contributions | ForEach-Object { $_.unit })
    Assert (($theoryContribution -contains 'link-check/book@1') -and ($theoryContribution -contains 'models/g003-m0001@1')) 'the theory revision consumes the link-check revision and the changed units'
    $organize = Run-Digest @('ledger','status','-Out',$out,'-Layer','organize')
    Assert (@($organize.dependencies | Where-Object { $_.unit -eq 'organize/book@1' -and $_.needs -eq 'theory/book@1' -and $_.condition -eq 'settled' }).Count -eq 1) 'Organize is the tail publication that follows the theory revision'
    $theoryReady = Claim 'theory'
    $theoryUnit = @($theoryReady.units)[0].id
    $theoryToken = @($theoryReady.units)[0].token
    $theoryInputHash = @($theoryReady.units)[0].input_hash
    $theoryEvidence = Join-Path $out 'publication-theory-book-2.json'
    $consumed = @(@($theoryReady.units)[0].inputs.contributions | ForEach-Object { @{ unit = $_.unit; fingerprint = $_.fingerprint } })
    $coverage = @(@{ ref = $theory1; outcome = 'rebuilt'; reason = '' }, @{ ref = $theory2; outcome = 'rebuilt'; reason = '' })
    function Theory-Evidence($plan) { Save-Json $theoryEvidence $plan; $null = $theoryEvidence }
    Theory-Evidence ([ordered]@{ schema_version = 1; kind = 'publication_completion'; unit = $theoryUnit; token = $theoryToken; layer = 'theory'; key = 'book'; input_hash = $theoryInputHash; disposition = 'published'; consumed = $consumed; unresolved = @(); artifacts = @{ applied = $theoryApplied }; coverage = @(@{ ref = $theory2; outcome = 'rebuilt'; reason = '' }) })
    $missing = @(& $Digest ledger done -Out $out -Unit $theoryUnit -Token $theoryToken -Outputs $theoryEvidence 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($missing -join ' ').Contains('must classify every Theory Card exactly once')) "a theory revision must account for every Theory Card ($($missing -join ' '))"
    # A summary that no longer accounts for the theory's units cannot complete.
    $staleRecord = (Get-Content -Raw $theoryApplied | ConvertFrom-Json -AsHashtable)
    foreach ($record in $staleRecord.theories) { if ($record.address -ceq $theory2) { $record.members = @($record.members | Select-Object -First 1) } }
    [IO.File]::WriteAllText($theoryApplied, ($staleRecord | ConvertTo-Json -Depth 20), $utf8)
    Theory-Evidence ([ordered]@{ schema_version = 1; kind = 'publication_completion'; unit = $theoryUnit; token = $theoryToken; layer = 'theory'; key = 'book'; input_hash = $theoryInputHash; disposition = 'published'; consumed = $consumed; unresolved = @(); artifacts = @{ applied = $theoryApplied }; coverage = $coverage })
    $stale = @(& $Digest ledger done -Out $out -Unit $theoryUnit -Token $theoryToken -Outputs $theoryEvidence 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($stale -join ' ').Contains('does not account for its model''s current units')) "a summary the theory's units no longer match cannot complete ($($stale -join ' '))"
    # Rebuilding after that change makes the summary current again.
    $null = Math 'theory-apply' @('-Apply')
    $theoryDone = Run-Digest @('ledger','done','-Out',$out,'-Unit',$theoryUnit,'-Token',$theoryToken,'-Outputs',$theoryEvidence)
    $rebuiltUnits = @((Read-Model $theory2).math.order).Count
    Assert ($theoryDone.state -eq 'done' -and @($theoryDone.coverage).Count -eq 2 -and @($theoryDone.coverage | Where-Object { $_.ref -ceq $theory2 -and ($_.members + $_.tools) -eq $rebuiltUnits }).Count -eq 1) ("the rebuilt theory revision completes with every Theory Card's current summary recorded (" + ($theoryDone.coverage | ConvertTo-Json -Depth 6 -Compress) + ")")
    # A changed unit derives the next revision and makes the completed one terminal history.
    $anotherModel = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');const crypto=require('node:crypto');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-math-test-'))throw new Error('not the isolated ledger');
const input=JSON.stringify({model:'Abbott2015/3|a'});
const db=new DatabaseSync(p);db.prepare("insert into units(id,layer,logical_key,revision,state,inputs_json,input_hash,created_by,created_at,completed_at,result_json) values(?,'models',?,'1','done',?,?,'prepare/book@1',?,?,'{\"disposition\":\"verified\"}')").run(process.argv[2],process.argv[2],input,crypto.createHash('sha256').update(input).digest('hex'),new Date().toISOString(),new Date().toISOString());db.close();
'@
    & node --no-warnings -e $anotherModel (Join-Path $out 'ledger.sqlite3') 'models/g003-m0002@1'
    if ($LASTEXITCODE) { throw 'isolated second Model fixture failed' }
    $republished = Run-Digest @('ledger','publish','-Out',$out,'-Layer','theory')
    $theoryStatus = Run-Digest @('ledger','status','-Out',$out,'-Layer','theory')
    $theoryPublication = @($theoryStatus.publications | Where-Object { $_.layer -eq 'theory' })[0]
    Assert ($republished.revision -eq 3 -and $republished.unit -ceq 'theory/book@3' -and @($theoryPublication.superseded | Where-Object { $_.unit -eq 'theory/book@2' -and $_.state -eq 'done' }).Count -eq 1) 'a changed unit derives the next theory revision and names the completed one as its superseded history'
    Assert (@($theoryStatus.units | Where-Object { $_.id -eq 'theory/book@2' -and $_.state -eq 'done' }).Count -eq 1) 'the superseded theory revision keeps its verified completion as history'

    # --- the M1-M18 check describes the revision the audit published --------------------------------
    $null = Math 'check' @('-Out',$out)
    $blockedCheck = Claim 'math-check'
    Assert (@($blockedCheck.units).Count -eq 0) 'the final math check waits for the audit revision'
    # A stopped-local fixture supplies the published audit revision; the check itself runs for real.
    $auditFixture = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-math-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.prepare("update units set state='done',completed_at=?,result_json='{\"disposition\":\"published\"}' where id='audit/book@1'").run(new Date().toISOString());db.close();
'@
    & node --no-warnings -e $auditFixture (Join-Path $out 'ledger.sqlite3')
    if ($LASTEXITCODE) { throw 'isolated audit fixture failed' }
    $checkClaim = Claim 'math-check'
    $checkUnit = @($checkClaim.units)[0].id
    $checkToken = @($checkClaim.units)[0].token
    $checkFile = Join-Path $out 'math-audit.json'
    $auditBytes = [IO.File]::ReadAllBytes($checkFile)
    $stale = (Get-Content -Raw $checkFile | ConvertFrom-Json -AsHashtable)
    $stale.units = $stale.units + 1
    [IO.File]::WriteAllText($checkFile, ($stale | ConvertTo-Json -Depth 20), $utf8)
    $staleRefused = @(& $Digest ledger done -Out $out -Unit $checkUnit -Token $checkToken -Outputs $checkFile 2>&1)
    Assert ($LASTEXITCODE -eq 1 -and ($staleRefused -join ' ').Contains('not the M1–M18 report of the current revision')) "a stale math audit cannot complete the final check ($($staleRefused -join ' '))"
    [IO.File]::WriteAllBytes($checkFile, $auditBytes)
    $checkDone = Run-Digest @('ledger','done','-Out',$out,'-Unit',$checkUnit,'-Token',$checkToken,'-Outputs',$checkFile)
    Assert ($checkDone.state -eq 'done' -and $checkDone.audit -ceq 'audit/book@1' -and $checkDone.audit_revision -eq 1 -and $checkDone.units -eq 3 -and $checkDone.theories -eq 2) 'the final math check names the audit revision its M1-M18 report describes'
    Assert ($checkDone.faults -ge 1 -and $checkDone.by_rule.M1 -ge 1) 'the final check reports the rules the book still breaks instead of claiming a clean pass'

    # --- the drained obligations never rebuilt the model unit ---------------------------------------
    $modelAfter = (& node --no-warnings -e "const {DatabaseSync}=require('node:sqlite');const db=new DatabaseSync(process.argv[1],{readOnly:true});console.log(JSON.stringify(db.prepare('select * from units where id=?').get(process.argv[2])));db.close();" (Join-Path $out 'ledger.sqlite3') $modelUnit)
    Assert ($modelAfter -ceq $modelRow) 'draining the boundary and proof obligations changed no fact of the done model unit'
    $drained = Run-Digest @('ledger','rerun-list','-Out',$out)
    Assert ($modelUnit -notin $drained.units.id -and "boundary/$th@1" -notin $drained.units.id -and "proof/$th@1" -notin $drained.units.id) 'completed obligations leave the resume list while the done model build stays out of it'
    $modelNow = (Read-Model $th) | ConvertTo-Json -Depth 8 -Compress
    Assert ($modelNow -ne $modelBefore -and (Read-Card $th).title -eq '介值定理：连续函数取遍中间值') 'the work changed the model parts, not the unit Card the build owns'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
