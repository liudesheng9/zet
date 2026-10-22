#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\math-compiler.tests.ps1 [-Zt <zt.exe>]
# Drives math-compiler.ps1 through index, docs, skeleton, notation, boundary, proof, theory, and
# check on a scratch archive (ZT_CONFIG_DIR, its own service) and a two-chapter fixture book,
# standing in for DeepSeek's Card bodies and JSON files. It also checks that model-compiler.ps1
# accepts a unit's plan that cites another unit. The live archive is not touched. -Zt runs a zt
# build that is not installed yet (it is copied to the scratch folder and put first on PATH).
param([string]$Zt = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "math-compiler-test-$PID"
$archive = Join-Path $scratch 'archive'
$staging = Join-Path $scratch 'staging'
New-Item -ItemType Directory -Force $archive, (Join-Path $staging 'sections'), (Join-Path $staging 'groups') | Out-Null
if ($Zt) {
    $bin = Join-Path $scratch 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$compiler = Join-Path $PSScriptRoot '..\math-compiler.ps1' | Resolve-Path | ForEach-Object Path
$modelCompiler = Join-Path $PSScriptRoot '..\model-compiler.ps1' | Resolve-Path | ForEach-Object Path
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Run($step, [string[]]$more = @()) {
    $lines = @(pwsh -NoProfile -File $compiler -Step $step -Staging $staging @more 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "$step failed: $($lines -join "`n")" }
    $lines
}
function Last($lines) { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable }
function Write-Calls($calls) {
    $out = @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "zt: $out" }
    , @($out | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Save-Text($path, $text) { New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null; [IO.File]::WriteAllText($path, $text, $utf8) }
function Save-Json($path, $value) { Save-Text $path ($value | ConvertTo-Json -Depth 10) }
function Read-Model($at) { @(Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $at } }))[0] }
function Read-Card($at) { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] }
$key = 'Abbott2015'
try {
    # --- the fixture book: two chapters, one group each ------------------------------------------
    Save-Text (Join-Path $staging 'sections\s01.md') @'
# 第 1 章 实数

## 1.3 完备性

**公理 1.3.3**（完备性公理）有上界的非空实数集有上确界。

例 1.3.4 集合 $\{x: x^2<2\}$ 有上确界 $\sqrt2$。
'@
    Save-Text (Join-Path $staging 'sections\s02.md') @'
# 第 4 章 函数极限与连续

## 4.3 连续函数

**定义 4.3.1**（连续）设 $f:A\to\mathbb{R}$，$x_0\in A$。若对每个 $\varepsilon>0$ 都存在 $\delta>0$，则称 $f$ 在 $x_0$ 连续。记作 $f\in C(A)$。

**引理 4.4.2** 若 $f$ 在 $x_0$ 连续且 $f(x_0)<c$，则在 $x_0$ 附近 $f<c$。

证明：由定义 4.3.1 取 $\varepsilon=c-f(x_0)$。

**定理 4.5.1**（介值定理）设 $f$ 在 $[a,b]$ 上连续，$f(a)<c<f(b)$。则存在 $x_0$ 使 $f(x_0)=c$。

证明：令 $S=\{x: f(x)<c\}$。由公理 1.3.3，$S$ 有上确界。由引理 4.4.2 排除 $f(x_0)<c$。

**推论 4.5.2** 奇次多项式至少有一个实根。

证明：由定理 4.5.1。

习题 4.5.3 证明 $x^3-x-1$ 在 $[1,2]$ 上有零点。
'@
    foreach ($n in 1, 2) { Save-Text (Join-Path $staging ('groups\g{0:d2}.md' -f $n)) "# 你的组：第 $n 组`n" }
    Save-Text (Join-Path $staging 'shared-prefix.md') "# 任务说明（所有组共用）`n"
    Save-Text (Join-Path $staging 'link-prefix.md') "# 连接说明`n"
    Save-Json (Join-Path $staging 'manifest.json') ([ordered]@{
            citation_key = $key; book_title = '理解分析'; staging = $staging
            shared_prefix = (Join-Path $staging 'shared-prefix.md'); link_prefix = (Join-Path $staging 'link-prefix.md')
            groups = @(
                [ordered]@{ n = 1; id = 'g01'; anchor = "$key/1"; chapter = '第 1 章 实数'; headings = @('1.3 完备性'); sections = @('s01.md'); file = (Join-Path $staging 'groups\g01.md') },
                [ordered]@{ n = 2; id = 'g02'; anchor = "$key/2"; chapter = '第 4 章 函数极限与连续'; headings = @('4.3 连续函数'); sections = @('s02.md'); file = (Join-Path $staging 'groups\g02.md') })
        })

    # --- index (files only) ------------------------------------------------------------------------
    $index = Last (Run 'index')
    Assert ($index.units -eq 5) 'index: five units'
    Assert ($index.forms.theorem -eq 1 -and $index.forms.lemma -eq 1 -and $index.forms.axiom -eq 1) 'index: the forms'
    Assert ($index.examples -eq 1 -and $index.exercises -eq 1) 'index: one example and one exercise'
    $data = Get-Content -Raw (Join-Path $staging 'math\index.json') | ConvertFrom-Json -AsHashtable
    $thm = @($data.units | Where-Object { $_.id -eq '定理 4.5.1' })[0]
    Assert ($thm.name -eq '介值定理' -and $thm.level -eq 'formal' -and $thm.proof -gt 0) 'index: the theorem has its name and a proof'
    Assert ((@($thm.cites) -join ',') -eq '公理 1.3.3,引理 4.4.2') 'index: the theorem cites the axiom and the lemma'
    $ax = @($data.units | Where-Object { $_.id -eq '公理 1.3.3' })[0]
    Assert ($ax.level -eq 'concept') 'index: an axiom is a concept unit'
    Assert ((@($data.others | Where-Object { $_.id -eq '习题 4.5.3' })[0]).after -eq '推论 4.5.2') 'index: the exercise follows the corollary'

    # --- docs --------------------------------------------------------------------------------------
    $docs = Last (Run 'docs')
    $prefix = Get-Content -Raw (Join-Path $staging 'math-prefix.md')
    Assert ($prefix.StartsWith('# 任务说明（所有组共用）') -and $prefix -match '数学书补充说明') 'docs: the math prefix is the shared prefix plus the supplement'
    Assert ((Get-Content -Raw (Join-Path $staging 'math\fill.zh.md')) -match '填写模型分卡[\s\S]*填写数学单元') 'docs: the fill instructions keep the shared rules first'

    # --- skeleton (scratch archive) ------------------------------------------------------------------
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $null = Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@book{$key, title={Understanding Analysis}}" } },
        @{ tool = 'create_successor'; arguments = @{ at = $key; title = '第 1 组' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; title = '第 2 组' } })
    $sk = Last (Run 'skeleton' @('-Apply'))
    Assert ($sk.units -eq 5 -and $sk.theories -eq 2) 'skeleton: five units and two Theory Cards'
    $units = (Get-Content -Raw (Join-Path $staging 'math\units.json') | ConvertFrom-Json -AsHashtable)
    $at = @{}
    foreach ($u in $units.units) { $at[$u.id] = $u.address }
    $theory4 = @($units.theories | Where-Object { $_.chapter -match '第 4 章' })[0].address
    $theory1 = @($units.theories | Where-Object { $_.chapter -match '第 1 章' })[0].address
    Assert ($at['公理 1.3.3'] -eq "$key/1|a" -and $at['定理 4.5.1'] -eq "$key/2|c") 'skeleton: units are Side successors of their anchors in book order'
    $card = Read-Card $at['定理 4.5.1']
    Assert ($card.form -eq 'theorem' -and $card.level -eq 'formal' -and $card.body -match 'sections/s02.md') 'skeleton: the theorem has its form, level, and source'
    Assert ((Get-Content -Raw (Join-Path $staging 'owned-g02.txt')) -match [regex]::Escape($at['定理 4.5.1'])) 'skeleton: the unit joins its group''s owned Cards'
    $g2 = Get-Content -Raw (Join-Path $staging 'groups\g02.md')
    Assert ($g2 -match '本组的数学单元' -and $g2 -match [regex]::Escape("公理 1.3.3 [[$($at['公理 1.3.3'])]]") -and $g2 -match '习题 4.5.3') 'skeleton: the group file lists its units, citations, and exercises'
    Assert ((Read-Card $theory4).form -eq 'theory') 'skeleton: a Theory Card per chapter'
    $again = Last (Run 'skeleton' @('-Apply'))
    Assert ($again.created -eq 0 -and $again.theories -eq 2) 'skeleton: a rerun makes nothing twice'

    # --- the digest and model stage, in place of DeepSeek and the model compiler ----------------------
    $def = $at['定义 4.3.1']; $lem = $at['引理 4.4.2']; $th = $at['定理 4.5.1']; $cor = $at['推论 4.5.2']; $axiom = $at['公理 1.3.3']
    $null = Write-Calls @(
        @{ tool = 'edit_card'; arguments = @{ at = $axiom; title = '完备性公理：有上界的非空实数集有上确界'; body = "**公理**：`n- (C1) 有上界的非空实数集有上确界。`n`n来源：[[$key]] 公理 1.3.3" } },
        @{ tool = 'edit_card'; arguments = @{ at = $def; title = '连续：ε-δ 条件成立'; body = "**定义**：`n- (C1) `$x_0\in A`$。`n- (C2) 对每个 `$\varepsilon>0`$ 都存在 `$\delta>0`$。`n`n来源：[[$key]] 定义 4.3.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = $def; kind = 'side'; facet = '例子'; emphasis = 'core'; title = '例子：x^2 处处连续'; body = "例示 [[$def]]。" } },
        @{ tool = 'create_successor'; arguments = @{ at = $def; kind = 'side'; facet = '非例'; emphasis = 'core'; title = '非例：符号函数在 0 处跳跃'; body = '(C2) 不成立。' } },
        @{ tool = 'create_successor'; arguments = @{ at = $def; kind = 'side'; facet = '直觉'; emphasis = 'core'; title = '直觉：图像不断开'; body = '图像不断开。' } },
        @{ tool = 'edit_card'; arguments = @{ at = $lem; title = '引理 4.4.2：连续函数在一点附近保号'; body = "**陈述**：`n- (H1) f 在 x0 连续，f(x0)<c。`n- 结论：x0 附近 f<c。`n`n来源：[[$key]] 引理 4.4.2" } },
        @{ tool = 'create_successor'; arguments = @{ at = $lem; kind = 'side'; facet = '前提'; title = '前提：f 在 x0 连续'; body = '(H1)。' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@6'; facet = '证明：取 ε'; title = '证明：取 ε=c-f(x0) 得到 δ'; body = "依赖 [[@@6]]。按定义 [[$def]] 取 ε。由 (H1)。" } },
        @{ tool = 'edit_card'; arguments = @{ at = $th; title = '介值定理：连续函数取遍中间值'; body = "**陈述**：`n- (H1) f 在 [a,b] 上连续。`n- (H2) f(a)<c<f(b)。`n- 结论：存在 x0 使 f(x0)=c。`n`n**思路**：取上确界，再用反证法排除两种情形。`n`n来源：[[$key]] 定理 4.5.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = $th; kind = 'side'; facet = '前提'; title = '前提：f 连续，f(a)<c<f(b)'; body = '(H1) 与 (H2)。' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@9'; facet = '证明思路'; emphasis = 'core'; title = '证明思路：取上确界再反证'; body = "依赖 [[@@9]]。方法：反证。" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@10'; facet = '证明：x0 存在'; title = '证明：S 有上确界 x0'; body = "依赖 [[@@10]]。依赖 [[$axiom]]：S 有上确界。由 (H2)。" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@11'; facet = '证明：排除小于'; emphasis = 'core'; title = '证明：f(x0)<c 不成立'; body = "依赖 [[@@11]]。依赖 [[$lem]]：右侧仍小于 c。由 (H1)。" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@12'; facet = '证明：排除大于'; title = '证明：f(x0)>c 不成立'; body = "依赖 [[@@12]]。" } },
        @{ tool = 'create_successor'; arguments = @{ at = $th; kind = 'side'; facet = '直觉'; emphasis = 'core'; title = '直觉：图像必须穿过直线'; body = '图像从下方走到上方。' } },
        @{ tool = 'edit_card'; arguments = @{ at = $cor; body = "**陈述**：`n- (H1) p 是奇次实系数多项式。`n- 结论：p 有实根。`n`n依赖 [[$th]]。`n`n来源：[[$key]] 推论 4.5.2" } })
    $model = Read-Model $th
    Assert (@($model.build.cited).Count -eq 2 -and @($model.build.closure).Count -eq 0) 'model: the theorem cites the axiom and the lemma, with no closure fault'

    # --- model-compiler accepts a unit's plan that cites another unit -------------------------------
    $work = Join-Path $scratch 'mwork'
    $dir = Join-Path $work ($cor -replace '[\\/|]', '_')
    Save-Json (Join-Path $dir 'plan.json') ([ordered]@{
            model = $cor; level = 'formal'; card = '动机：奇次多项式为何有实根。'
            symbols = @(, @('$p$', '奇次实系数多项式')); terms = @(, @('实根', '实零点'))
            parts = @(
                [ordered]@{ id = 'P'; line = 'main'; facet = '前提'; title = '前提：p 是奇次多项式'; spec = 'p 的次数与系数'; rests = @(, @($th, '用到介值定理')); src = @('sections/s01.md:1-2') },
                [ordered]@{ id = 'S'; line = 'main'; facet = '证明：两端变号'; emphasis = 'core'; title = '证明：p 在两端变号'; spec = '极限'; rests = @(, @('P', '用到前提')); src = @('sections/s01.md:3-4') })
        })
    $validated = @(pwsh -NoProfile -File $modelCompiler -Step validate -Work $work -Plan (Join-Path $dir 'plan.json') 2>&1)
    Assert (($validated | Select-Object -Last 1) -eq 'faults: 0') "model-compiler: a plan that starts with 前提 and cites a unit validates ($($validated -join ' | '))"

    # --- notation ---------------------------------------------------------------------------------------
    $nplan = Last (Run 'notation')
    Assert ($nplan.candidates -ge 1 -and @($nplan.jobs).Count -eq 1) 'notation: one job with candidates'
    Assert ((Get-Content -Raw $nplan.jobs[0].files[1]) -match '记作') 'notation: the job lists the sentence that introduces a symbol'
    Save-Json (Join-Path $staging 'math\notation\out\notation.json') @{ symbols = @(@{ symbol = '$C(A)$'; meaning = 'A 上的连续函数'; unit = '定义 4.3.1' }) }
    $napplied = Last (Run 'notation-apply' @('-Apply'))
    Assert ($napplied.symbols -eq 1 -and (Read-Card $napplied.card).body -match [regex]::Escape("定义 4.3.1 [[$def]]")) 'notation: the table links the defining unit'

    # --- boundary ----------------------------------------------------------------------------------------
    $bplan = Last (Run 'boundary')
    Assert ($bplan.units -eq 3 -and @($bplan.jobs).Count -eq 1) 'boundary: the theorem, the lemma, and the corollary need hypothesis roles'
    $bjob = Get-Content -Raw $bplan.jobs[0].files[1]
    Assert ($bjob -match 'hypotheses:H1,H2' -and $bjob -notmatch [regex]::Escape("[[$def]] 定义")) 'boundary: the definition with a contrast pair needs nothing'
    $steps = @($model.build.order | Where-Object { $_.line -eq 'main' } | ForEach-Object { $_.address })
    Save-Json (Join-Path $staging 'math\boundary\out\b-01.json') @{ items = @(
            @{ unit = $th; hypothesis = 'H1'; title = '假设作用：去掉连续性，函数可以跳过 c'; role = '步骤三用到连续性。'; used_in = $steps[3]; counterexample = @{ title = '反例：符号函数跳过 1/2'; body = '取符号函数。'; supplied = $true } },
            @{ unit = $th; hypothesis = 'H2'; title = '假设作用：c 须在两端值之间'; role = '步骤一用到它。'; used_in = $steps[2]; counterexample = $null },
            @{ unit = $lem; hypothesis = 'H1'; title = '假设作用：保号要连续'; role = '证明取 ε 时用到。'; used_in = ''; counterexample = @{ title = '反例：跳跃函数不保号'; body = '书中的例子。'; supplied = $false } }) }
    $bapplied = Last (Run 'boundary-apply' @('-Supply', 'examples'))
    Assert ($bapplied.created -eq 5 -and @($bapplied.jobs).Count -eq 1) 'boundary-apply: five parts and one review job for the supplied one'
    $parts = @((Read-Model $th).parts)
    $role = @($parts | Where-Object { $_.facet -eq '假设作用：H1' })[0]
    $ce = @($parts | Where-Object { $_.facet -eq '反例：去掉 H1' })[0]
    Assert ($role -and $ce -and $role.body -match [regex]::Escape("[[$($ce.address)]]") -and $ce.body -match '（补：书中未给）') 'boundary-apply: the role links its counterexample, which carries the mark'
    Save-Json (Join-Path $staging 'math\boundary\out\v-b-01.json') @{ verdicts = @(@{ address = $ce.address; verdict = 'wrong'; reason = '符号函数在 [-1,1] 上取不到 1/2，但它不满足 H2 的端点条件。' }) }
    $settled = Last (Run 'boundary-settle')
    Assert ($settled.deleted -eq 2) 'boundary-settle: the wrong counterexample goes, with the role that names it'
    $facets = @((Read-Model $th).parts | ForEach-Object { $_.facet })
    Assert (-not ($facets -contains '反例：去掉 H1') -and ($facets -contains '假设作用：H2')) 'boundary-settle: the other parts stay'
    Assert (-not (@(Invoke-Zt @(@{ tool = 'list_broken_links'; arguments = @{ workspace = 'agent' } }))[0].broken).Count) 'boundary-settle: no broken Link is left'

    # --- proof --------------------------------------------------------------------------------------------
    $pplan = Last (Run 'proof')
    Assert ($pplan.units -eq 2 -and @($pplan.jobs).Count -eq 1) 'proof: the lemma and the theorem have proof steps'
    $pjob = Get-Content -Raw $pplan.jobs[0].files[1]
    Assert ($pjob -match '（关键步）') 'proof: the job marks the crux step'
    $last = @((Read-Model $th).build.order | Where-Object { $_.line -eq 'main' })[-1].address
    $old = (Read-Card $last).body
    Save-Json (Join-Path $staging 'math\proof\out\p-01.json') @{ verdicts = @(
            @{ address = $last; verdict = 'fix'; reason = '补出 x0>a。'; body = "$old`n由 (H2)，x0>a。" },
            @{ address = $steps[2]; verdict = 'fix'; reason = '删掉链接。'; body = '没有链接的正文。' }) }
    $papplied = Last (Run 'proof-apply')
    Assert ($papplied.fixed -eq 1 -and $papplied.refused -eq 1) 'proof-apply: writes a fix that keeps its Links and refuses one that drops them'
    Assert ((Read-Card $last).body -match 'x0>a') 'proof-apply: the fixed body is written'

    # --- theory ---------------------------------------------------------------------------------------------
    $tplan = Last (Run 'theory')
    Assert ($tplan.theories -eq 2 -and @($tplan.jobs).Count -eq 1) 'theory: one job for both Theory Cards'
    Save-Json (Join-Path $staging 'math\theory\out\t-01.json') @{ theories = @(
            @{ address = $theory4; title = '连续函数理论：连续性使函数取遍中间值'; question = '连续函数有什么整体性质？'
                main_results = @(@{ address = $th; line = '闭区间上的连续函数取遍中间值。' }, @{ address = $axiom; line = '不是本章的单元。' })
                objects = @(@{ address = $def; line = '一点的连续性。' }) }) }
    $tapplied = Last (Run 'theory-apply')
    Assert ($tapplied.theories -eq 2) 'theory-apply: both Theory Cards rewritten'
    $tc = Read-Card $theory4
    $order = @("[[$def]]", "[[$lem]]", "[[$th]]", "[[$cor]]" | ForEach-Object { $tc.body.IndexOf($_, $tc.body.IndexOf('**结构**')) })
    Assert ($tc.title -eq '连续函数理论：连续性使函数取遍中间值' -and $tc.body -match '连续函数有什么整体性质') 'theory-apply: the title and question'
    Assert ($tc.body -match [regex]::Escape("**主要结果**：`n- [[$th]]") -and $tc.body -notmatch [regex]::Escape("[[$axiom]]：不是本章")) 'theory-apply: main results only from the theory''s units'
    Assert (($order -join ',') -eq (($order | Sort-Object) -join ',') -and $order[0] -gt 0) 'theory-apply: 结构 lists the units in dependency order'
    Assert ($tc.body -match '\*\*所用工具\*\*' -and $tc.body -match [regex]::Escape("依赖 [[$axiom]]")) 'theory-apply: the axiom of chapter 1 is a tool'
    Assert ($tc.body -match '```chart' -and $tc.body -match [regex]::Escape("data-zt=`"$th`"")) 'theory-apply: the dependency map opens the units'
    $tm = (Read-Model $theory4).math
    Assert (@($tm.main_results).Count -eq 1 -and @($tm.objects).Count -eq 1 -and @($tm.cycle).Count -eq 0) 'theory-apply: ZT reads the main results and the objects'
    $t1 = Read-Card $theory1
    Assert ($t1.body -match '\*\*后续\*\*' -and $t1.body -match [regex]::Escape("[[$theory4]]")) 'theory-apply: chapter 1 names chapter 4 as its user'

    # --- check ----------------------------------------------------------------------------------------------
    $checked = Last (Run 'check')
    Assert ($checked.units -eq 5 -and $checked.theories -eq 2) 'check: five units and two Theory Cards'
    Assert ($checked.by_rule.M1 -ge 1) 'check: a unit still holding 待写 is an M1 fault'
    $audit = Get-Content -Raw (Join-Path $staging 'math-audit.json') | ConvertFrom-Json -AsHashtable
    Assert (@($audit.list | Where-Object { $_.rule -eq 'M1' -and $_.address -eq $cor }).Count -ge 1) 'check: the corollary title still holds 待写'
    Assert (-not @($audit.list | Where-Object { $_.rule -eq 'M5' -and $_.address -eq $th }).Count) 'check: every hypothesis of the theorem is used'
    Assert (@($audit.list | Where-Object { $_.rule -eq 'M8' }).Count -eq 0) 'check: the proof plans name their method'
    Assert (Test-Path -LiteralPath (Join-Path $staging 'math-audit.md')) 'check: math-audit.md written'
} finally {
    zt down 2>$null | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
