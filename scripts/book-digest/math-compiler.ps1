#Requires -Version 7
<#
.SYNOPSIS
  The deterministic half of the zt-math-digest workflow (doc/math-cards-design.md). DeepSeek writes
  every Card body and every JSON answer; this script finds the book's numbered statements, builds
  the skeleton of every mathematical unit and Theory Card, writes the job files, applies the
  answers, and checks the result. It never writes Card prose except skeletons, lists, and Links.

.DESCRIPTION
  -Staging is the digest's staging folder (manifest.json, sections\, groups\). The work folder is
  <Staging>\math. Every step prints compact JSON as its last line. A step that hands work to
  DeepSeek prints {"jobs":[{"id","files","prompt"}], ...}.

    index            finds every numbered environment in the book (定义, 公理, 定理, 命题, 引理, 推论,
                     例, 习题, and their English names), its lines, its proof, its name, and the
                     statements it cites; writes math\index.json and math\index.md. Reads files only.
    docs             writes the math prompt files: math-prefix.md (the shared digest prefix with
                     the math supplement), math-link-prefix.md, and math\brief.zh.md, fill.zh.md,
                     review.zh.md (each shared instruction file with its math supplement). The
                     shared files come first, so every rule of the book digest holds unchanged.
    skeleton         (-Apply) creates one unit Model Card per indexed definition, axiom, and result
                     (its Math form, its level, its source lines) as a Side successor of its group's
                     anchor, then one Theory Card per chapter that lists its units. Each unit joins
                     its group's owned-gNN.txt and src-gNN.txt, so the digest, the model stage, and
                     the link pass treat it like a Card the group wrote. Appends the units, examples,
                     and exercises of each group to its group file.
    notation         writes the notation job (symbol candidates); notation-apply (-Apply) writes the
                     Book notation table (符号表) as a Side successor of <key>/1.
    boundary         one job per -UnitsPerJob units: hypothesis roles with their counterexamples
                     for results, and the contrast pair for definitions that lack one.
    boundary-apply   creates the parts from math\boundary\out\b-NN.json (only book items under
                     -Supply none) and prints one review job per batch for the supplied items.
    boundary-settle  deletes each supplied part that a review (v-NN.json) judged wrong, with the
                     part that names it.
    proof            one job per -UnitsPerJob formal results: a DeepSeek reviewer checks the proof
                     plan, every proof step (证明：…), and the conclusion; the Derivation parts are
                     checked by the model compiler's review.
    proof-apply      writes each fixed body from math\proof\out\p-NN.json that keeps every Link of
                     the old body; refuses the others.
    theory           one job per four Theory Cards: the question, the main results (at most
                     -MainResults), and the objects.
    theory-apply     rewrites each Theory Card: 问题, 主要结果, 对象 from the answers; 结构 in
                     dependency order, 所用工具, 后续, and the dependency map from get_model.
    check            the math rules M1 to M18 on every unit and Theory Card of the book; writes
                     math-audit.json and math-audit.md in -Out (default <Staging>). Read-only.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('index', 'docs', 'skeleton', 'notation', 'notation-apply', 'boundary', 'boundary-apply', 'boundary-settle', 'proof', 'proof-apply', 'theory', 'theory-apply', 'check')][string]$Step,
    [Parameter(Mandatory)][string]$Staging,
    [string]$Out = '',
    [int]$UnitsPerJob = 4,
    [ValidateSet('none', 'examples', 'examples+answers')][string]$Supply = 'examples',
    [int]$MainResults = 7,
    [int]$CoreBudget = 3,
    [int]$MaxSymbols = 80,
    [switch]$Apply
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
$Work = Join-Path $Staging 'math'
New-Item -ItemType Directory -Force -Path $Work | Out-Null
function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String }
function Save-Text($path, $text) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
    [IO.File]::WriteAllText($path, $text, $utf8)
}
function Save-Json($path, $value) { Save-Text $path ($value | ConvertTo-Json -Depth 20) }
function Out-Last($object) { $object | ConvertTo-Json -Depth 20 -Compress }
$manifest = Read-Json (Join-Path $Staging 'manifest.json')
$key = [string]$manifest.citation_key
$IndexFile = Join-Path $Work 'index.json'
$UnitsFile = Join-Path $Work 'units.json'

# The forms: the Chinese word of a numbered environment, its Math form, and whether it is a unit.
$Forms = [ordered]@{
    '定义' = 'definition'; '公理' = 'axiom'; '定理' = 'theorem'; '命题' = 'proposition'; '引理' = 'lemma'; '推论' = 'corollary'
    'definition' = 'definition'; 'axiom' = 'axiom'; 'theorem' = 'theorem'; 'proposition' = 'proposition'; 'lemma' = 'lemma'; 'corollary' = 'corollary'
}
$FormZh = @{ theory = '理论'; definition = '定义'; axiom = '公理'; theorem = '定理'; proposition = '命题'; lemma = '引理'; corollary = '推论' }
$Results = @('theorem', 'proposition', 'lemma', 'corollary')
$UnitForms = @('definition', 'axiom') + $Results
$Num = '([0-9]+(?:[.．][0-9A-Za-z]+)*)'
$EnvRx = "^\s*(?:#{1,6}\s*)?(?:>\s*)?\**\s*(定义|公理|定理|命题|引理|推论|例子|例|习题|练习|Definition|Axiom|Theorem|Proposition|Lemma|Corollary|Example|Exercise)\s*$Num\s*\**\s*(?:[（(]([^）)\n]{1,30})[)）])?"
$ProofRx = '^\s*(?:#{1,6}\s*)?(?:>\s*)?\**\s*(证明|Proof)\s*\**\s*([:：.。]|\s|$)'
$RefRx = "(定义|公理|定理|命题|引理|推论|Definition|Axiom|Theorem|Proposition|Lemma|Corollary)\s*$Num"

# Writes through the zt helper; returns the addresses of the writes, in order.
function Invoke-Write($calls, [string]$log = '', [string]$srcLog = '') {
    if (-not @($calls).Count) { return , @() }
    $more = @()
    if ($log) { $more += @('-Log', $log) }
    if ($srcLog) { $more += @('-SrcLog', $srcLog) }
    $lines = @((@($calls) | ConvertTo-Json -Depth 10 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - @more)
    if ($LASTEXITCODE -ne 0) { throw "zt: $($lines -join ' | ')" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Get-Section($file, [int]$from, [int]$to, [int]$max = 80) {
    $path = Join-Path $Staging "sections\$file"
    if (-not (Test-Path -LiteralPath $path)) { return '' }
    $lines = @(Get-Content -LiteralPath $path -Encoding utf8)
    $to = [Math]::Min($to, $lines.Count)
    $from = [Math]::Max(1, $from)
    if ($to - $from + 1 -gt $max) { $to = $from + $max - 1 }
    ($lines[($from - 1)..($to - 1)] -join "`n")
}
function Get-Units { if (Test-Path -LiteralPath $UnitsFile) { Read-Json $UnitsFile } else { $null } }
function Get-Body($card) { [string]$card.body }
# A Card body with one section (a bold label or heading such as **结构**) replaced or added before 来源.
function Set-Section([string]$body, [string]$name, [string]$text) {
    $lines = [Collections.Generic.List[string]]::new()
    foreach ($l in ($body -split "`n")) { $lines.Add($l) }
    $isLabel = { param($l) $l -match '^\s*(\*\*([^*]+)\*\*|#{1,6}\s*(\S+))' }
    $labelOf = { param($l) if ($l -match '^\s*\*\*([^*]+)\*\*') { $Matches[1].Trim().TrimEnd('：', ':') } elseif ($l -match '^\s*#{1,6}\s*(.+)$') { $Matches[1].Trim().TrimEnd('：', ':') } else { '' } }
    $start = -1
    for ($i = 0; $i -lt $lines.Count; $i++) { if ((& $isLabel $lines[$i]) -and (& $labelOf $lines[$i]) -eq $name) { $start = $i; break } }
    if ($start -ge 0) {
        $end = $lines.Count
        for ($j = $start + 1; $j -lt $lines.Count; $j++) { if ((& $isLabel $lines[$j]) -or $lines[$j] -match '^\s*来源[：:]') { $end = $j; break } }
        $lines.RemoveRange($start, $end - $start)
        if ($text) { $lines.Insert($start, $text.TrimEnd() + "`n") }
    } elseif ($text) {
        $at = $lines.Count
        for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -match '^\s*来源[：:]') { $at = $i; break } }
        $lines.Insert($at, $text.TrimEnd() + "`n")
    }
    (($lines -join "`n") -replace "`n{3,}", "`n`n").Trim()
}
function Get-Zt($address, $tool = 'get_card') { @(Invoke-Zt @(@{ tool = $tool; arguments = @{ at = $address } }))[0] }

# --- index ---------------------------------------------------------------------------------------
if ($Step -eq 'index') {
    $units = [Collections.Generic.List[object]]::new()
    $others = [Collections.Generic.List[object]]::new()
    foreach ($g in @($manifest.groups)) {
        foreach ($file in @($g.sections)) {
            $path = Join-Path $Staging "sections\$file"
            if (-not (Test-Path -LiteralPath $path)) { continue }
            $lines = @(Get-Content -LiteralPath $path -Encoding utf8)
            $marks = @()
            for ($i = 0; $i -lt $lines.Count; $i++) {
                if ($lines[$i] -match $EnvRx) {
                    $marks += [ordered]@{ line = $i + 1; word = $Matches[1]; number = ($Matches[2] -replace '．', '.'); name = [string]$Matches[3] }
                }
            }
            for ($k = 0; $k -lt $marks.Count; $k++) {
                $m = $marks[$k]
                $end = if ($k + 1 -lt $marks.Count) { $marks[$k + 1].line - 1 } else { $lines.Count }
                # A heading ends the statement too, unless it is the statement's own heading.
                for ($j = $m.line; $j -lt $end; $j++) { if ($lines[$j] -match '^#{1,6}\s') { $end = $j; break } }
                $proof = 0
                for ($j = $m.line; $j -lt $end; $j++) { if ($lines[$j] -match $ProofRx) { $proof = $j + 1; break } }
                $text = ($lines[($m.line - 1)..([Math]::Max($m.line, $end) - 1)] -join "`n")
                $refs = @([regex]::Matches($text, $RefRx) | ForEach-Object { "$($_.Groups[1].Value) $($_.Groups[2].Value -replace '．', '.')" } |
                    Where-Object { $_ -ne "$($m.word) $($m.number)" } | Select-Object -Unique)
                $word = $m.word
                $form = $Forms[$word.ToLower()]
                $entry = [ordered]@{
                    id = "$word $($m.number)"; word = $word; number = $m.number; name = $m.name.Trim()
                    group = [int]$g.n; chapter = [string]$g.chapter; section = $file; line = $m.line; end = $end
                    proof = $proof; refs = $refs
                }
                if ($form) {
                    $entry.form = $form
                    $entry.level = if ($form -in $Results -and $proof) { 'formal' } else { 'concept' }
                    $units.Add($entry)
                } else {
                    $entry.kind = if ($word -match '^(例|例子|Example)$') { 'example' } else { 'exercise' }
                    # An example or exercise belongs beside the last statement before it.
                    $entry.after = if ($units.Count -and $units[$units.Count - 1].section -eq $file) { $units[$units.Count - 1].id } else { '' }
                    $others.Add($entry)
                }
            }
        }
    }
    # Resolve each reference to an indexed unit: by word and number, else by number alone.
    $byId = @{}
    $byNumber = @{}
    foreach ($u in $units) { $byId[$u.id] = $u.id; $n = $u.number; $byNumber[$n] = if ($byNumber.ContainsKey($n)) { '' } else { $u.id } }
    foreach ($u in $units) {
        $u.cites = @($u.refs | ForEach-Object {
                $num = ($_ -split ' ', 2)[1]
                if ($byId.ContainsKey($_)) { $byId[$_] } elseif ($byNumber[$num]) { $byNumber[$num] }
            } | Where-Object { $_ -and $_ -ne $u.id } | Select-Object -Unique)
    }
    Save-Json $IndexFile ([ordered]@{ units = @($units); others = @($others) })
    $md = [Text.StringBuilder]::new()
    [void]$md.AppendLine("# 数学单元索引：《$($manifest.book_title)》").AppendLine()
    [void]$md.AppendLine('| 编号 | 形式 | 层级 | 名称 | 原文 | 证明 | 引用 |').AppendLine('| --- | --- | --- | --- | --- | --- | --- |')
    foreach ($u in $units) { [void]$md.AppendLine("| $($u.id) | $($u.form) | $($u.level) | $($u.name) | ``sections/$($u.section):$($u.line)-$($u.end)`` | $(if ($u.proof) { ":$($u.proof)" } else { '—' }) | $(@($u.cites) -join '、') |") }
    Save-Text (Join-Path $Work 'index.md') $md.ToString()
    $counts = [ordered]@{}
    foreach ($u in $units) { $counts[$u.form] = 1 + [int]$counts[$u.form] }
    Out-Last ([ordered]@{ ok = $true; units = $units.Count; forms = $counts; examples = @($others | Where-Object { $_.kind -eq 'example' }).Count; exercises = @($others | Where-Object { $_.kind -eq 'exercise' }).Count; proofs = @($units | Where-Object { $_.proof }).Count; cites = (@($units | ForEach-Object { @($_.cites).Count }) | Measure-Object -Sum).Sum; file = $IndexFile })
    return
}

# --- docs ----------------------------------------------------------------------------------------
if ($Step -eq 'docs') {
    $fill = @{ KEY = $key; TITLE = [string]$manifest.book_title; STAGING = $Staging; INDEX = (Join-Path $Work 'index.md') }
    $render = {
        param($file)
        $text = Get-Content -LiteralPath (Join-Path $PSScriptRoot $file) -Raw -Encoding utf8
        foreach ($k in $fill.Keys) { $text = $text.Replace("{{$k}}", [string]$fill[$k]) }
        $text
    }
    $written = @()
    foreach ($pair in @(
            @([string]$manifest.shared_prefix, 'dsh-math-instructions.zh.md', (Join-Path $Staging 'math-prefix.md')),
            @([string]$manifest.link_prefix, 'dsh-math-link.zh.md', (Join-Path $Staging 'math-link-prefix.md')),
            @((Join-Path $PSScriptRoot 'dsh-brief-instructions.zh.md'), 'dsh-math-brief.zh.md', (Join-Path $Work 'brief.zh.md')),
            @((Join-Path $PSScriptRoot 'dsh-fill-instructions.zh.md'), 'dsh-math-fill.zh.md', (Join-Path $Work 'fill.zh.md')),
            @((Join-Path $PSScriptRoot 'dsh-review-instructions.zh.md'), 'dsh-math-review.zh.md', (Join-Path $Work 'review.zh.md')))) {
        $shared = Get-Content -LiteralPath $pair[0] -Raw -Encoding utf8
        Save-Text $pair[2] ($shared.TrimEnd() + "`n`n" + (& $render $pair[1]).TrimEnd() + "`n")
        $written += $pair[2]
    }
    Out-Last ([ordered]@{ ok = $true; files = $written })
    return
}

# --- skeleton --------------------------------------------------------------------------------------
if ($Step -eq 'skeleton') {
    if (-not (Test-Path -LiteralPath $IndexFile)) { throw 'run -Step index first' }
    $index = Read-Json $IndexFile
    $units = @($index.units)
    $done = Get-Units
    $made = @{}
    if ($done) { foreach ($u in @($done.units)) { if ($u.address) { $made[$u.id] = $u.address } } }
    $created = 0
    foreach ($g in @($manifest.groups)) {
        $mine = @($units | Where-Object { $_.group -eq [int]$g.n -and -not $made.ContainsKey($_.id) })
        if (-not $mine.Count -or -not $Apply) { continue }
        $id = 'g{0:d2}' -f [int]$g.n
        $calls = @($mine | ForEach-Object {
                $src = "sections/$($_.section):$($_.line)-$($_.end)"
                $title = "$($_.id)$(if ($_.name) { "（$($_.name)）" })：待写"
                @{ tool = 'create_successor'; arguments = @{ at = [string]$g.anchor; kind = 'side'; type = 'model'; form = $_.form; level = $_.level
                        title = $title; body = "**陈述**：待写。`n`n来源：[[$key]] $($_.id)（``$src``）"; src = $src } } })
        $addresses = Invoke-Write $calls (Join-Path $Staging "owned-$id.txt") (Join-Path $Staging "src-$id.txt")
        for ($i = 0; $i -lt $mine.Count; $i++) { $made[$mine[$i].id] = $addresses[$i]; $created++ }
    }
    foreach ($u in $units) { $u.address = $made[$u.id] }
    # One Theory Card per chapter that holds units, under the chapter's first anchor.
    $theories = @()
    if ($done) { $theories = @($done.theories) }
    $chapters = @($manifest.groups | ForEach-Object { [string]$_.chapter } | Select-Object -Unique)
    foreach ($chapter in $chapters) {
        $members = @($units | Where-Object { $_.chapter -eq $chapter -and $_.address })
        if (-not $members.Count -or @($theories | Where-Object { $_.chapter -eq $chapter }).Count) { continue }
        $anchor = [string](@($manifest.groups | Where-Object { [string]$_.chapter -eq $chapter })[0].anchor)
        $list = ($members | ForEach-Object { "- [[$($_.address)]] $($_.id)$(if ($_.name) { "（$($_.name)）" })" }) -join "`n"
        $body = "**问题**：待写。`n`n**主要结果**：待写。`n`n**对象**：待写。`n`n**结构**：`n$list`n`n来源：[[$key]] $chapter"
        $address = $null
        if ($Apply) {
            $address = (Invoke-Write @(@{ tool = 'create_successor'; arguments = @{ at = $anchor; kind = 'side'; type = 'model'; form = 'theory'; title = "理论：$chapter（待写）"; body = $body } }))[0]
        }
        $theories += [ordered]@{ chapter = $chapter; anchor = $anchor; address = $address; units = @($members | ForEach-Object { $_.address }) }
    }
    # Each group file names its units, examples, and exercises, so its digest run writes them.
    if ($Apply) {
        foreach ($g in @($manifest.groups)) {
            $mine = @($units | Where-Object { $_.group -eq [int]$g.n -and $_.address })
            $extra = @($index.others | Where-Object { $_.group -eq [int]$g.n })
            if (-not $mine.Count -and -not $extra.Count) { continue }
            $file = [string]$g.file
            $text = Get-Content -LiteralPath $file -Raw -Encoding utf8
            $marker = '## 本组的数学单元'
            if ($text.Contains($marker)) { continue }
            $sb = [Text.StringBuilder]::new()
            [void]$sb.AppendLine().AppendLine().AppendLine("$marker（脚本已建骨架，你写它们的模型卡正文）").AppendLine()
            foreach ($u in $mine) {
                $cites = @($u.cites | ForEach-Object { $c = $_; $a = $made[$c]; if ($a) { "$c [[$a]]" } else { $c } })
                [void]$sb.AppendLine("- [[$($u.address)]] $($u.id)$(if ($u.name) { "（$($u.name)）" })：$($u.form)，$($u.level)；原文 ``sections/$($u.section):$($u.line)-$($u.end)``$(if ($u.proof) { "，证明从第 $($u.proof) 行起" })$(if ($cites.Count) { "；引用 $($cites -join '、')" })")
            }
            $theory = @($theories | Where-Object { $_.chapter -eq [string]$g.chapter })[0]
            if ($theory -and $theory.address) { [void]$sb.AppendLine().AppendLine("本章的理论卡：[[$($theory.address)]]（理论阶段写它，你不改它）。") }
            if ($extra.Count) {
                [void]$sb.AppendLine().AppendLine('## 本组的例与习题（模型阶段把它们放进所属单元，你不另写卡片）').AppendLine()
                foreach ($e in $extra) { [void]$sb.AppendLine("- $($e.id)：$($e.kind)，``sections/$($e.section):$($e.line)-$($e.end)``$(if ($e.after) { "，属于 $($e.after)$(if ($made[$e.after]) { " [[$($made[$e.after])]]" })" })") }
            }
            Save-Text $file ($text.TrimEnd() + $sb.ToString())
        }
    }
    Save-Json $UnitsFile ([ordered]@{ key = $key; units = $units; others = @($index.others); theories = $theories })
    Out-Last ([ordered]@{ ok = $true; units = @($units | Where-Object { $_.address }).Count; created = $created; theories = @($theories | Where-Object { $_.address }).Count; applied = [bool]$Apply; file = $UnitsFile })
    return
}

# --- notation ------------------------------------------------------------------------------------
$NotationWork = Join-Path $Work 'notation'
if ($Step -eq 'notation') {
    New-Item -ItemType Directory -Force -Path (Join-Path $NotationWork 'out') | Out-Null
    $data = Get-Units
    $sb = [Text.StringBuilder]::new()
    [void]$sb.AppendLine("# 符号任务：《$($manifest.book_title)》").AppendLine()
    [void]$sb.AppendLine("把结果写入 ``$(Join-Path $NotationWork 'out\notation.json')``。").AppendLine()
    [void]$sb.AppendLine('## 定义与公理的原文开头（符号多在这里引入）').AppendLine()
    foreach ($u in @($data.units | Where-Object { $_.form -in 'definition', 'axiom' })) {
        $first = (Get-Section $u.section $u.line ([Math]::Min($u.end, $u.line + 5)) 6) -replace '\s+', ' '
        if ($first.Length -gt 220) { $first = $first.Substring(0, 220) + '…' }
        [void]$sb.AppendLine("- $($u.id)$(if ($u.address) { " [[$($u.address)]]" })：$first")
    }
    [void]$sb.AppendLine().AppendLine('## 引入记号的句子（记作、记为、表示、令）').AppendLine()
    $count = 0
    foreach ($file in @(Get-ChildItem -LiteralPath (Join-Path $Staging 'sections') -Filter 's*.md' -File | Sort-Object Name)) {
        $n = 0
        foreach ($line in Get-Content -LiteralPath $file.FullName -Encoding utf8) {
            $n++
            if ($count -ge 200) { break }
            if ($line -match '\$' -and $line -match '(记作|记为|记成|表示|令|设|定义为|denote)') {
                $clip = ($line -replace '\s+', ' ').Trim()
                if ($clip.Length -gt 160) { $clip = $clip.Substring(0, 160) + '…' }
                [void]$sb.AppendLine("- ``sections/$($file.Name):$n`` $clip")
                $count++
            }
        }
    }
    $job = Join-Path $NotationWork 'job.md'
    Save-Text $job $sb.ToString()
    $doc = Join-Path $NotationWork 'dsh-math-notation-instructions.zh.md'
    Save-Text $doc ((Get-Content -LiteralPath (Join-Path $PSScriptRoot 'dsh-math-notation-instructions.zh.md') -Raw -Encoding utf8).Replace('{{KEY}}', $key).Replace('{{STAGING}}', $Staging))
    Out-Last ([ordered]@{ candidates = $count; jobs = @([ordered]@{ id = 'notation'; files = @($doc, $job); prompt = '按以上说明选出本书的符号，写出 JSON 文件。最后只回复一行 JSON。' }) })
    return
}
if ($Step -eq 'notation-apply') {
    $file = Join-Path $NotationWork 'out\notation.json'
    if (-not (Test-Path -LiteralPath $file)) { Out-Last ([ordered]@{ ok = $false; error = 'no notation.json' }); return }
    $raw = Read-Json $file
    $data = Get-Units
    $byId = @{}
    if ($data) { foreach ($u in @($data.units)) { if ($u.address) { $byId[$u.id] = $u.address } } }
    $rows = @()
    $seen = @{}
    foreach ($s in @($raw.symbols)) {
        $symbol = ([string]$s.symbol).Trim()
        if (-not $symbol -or $seen.ContainsKey($symbol) -or $symbol -match '[|\n]') { continue }
        $seen[$symbol] = $true
        $unit = ([string]$s.unit).Trim()
        $where = if ($byId[$unit]) { "$unit [[$($byId[$unit])]]" } elseif ($unit) { $unit } else { '—' }
        $rows += "| $symbol | $(([string]$s.meaning).Trim() -replace '\|', '/') | $where |"
        if ($rows.Count -ge $MaxSymbols) { break }
    }
    $body = "本卡是《$($manifest.book_title)》的符号表。本书的单元用左列的符号表示中列的对象。证明可以依赖本表。`n`n符号表`n`n| 符号 | 含义 | 定义于 |`n| --- | --- | --- |`n$($rows -join "`n")`n`n来源：[[$key]]"
    $title = "符号表：《$($manifest.book_title)》的 $($rows.Count) 个符号"
    $address = $null
    if ($Apply) {
        $tree = @(Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $key } }))[0]
        $existing = @($tree.cards | Where-Object { [string]$_.title -match '^符号表' } | Select-Object -First 1)
        $call = if ($existing.Count) { @{ tool = 'edit_card'; arguments = @{ at = [string]$existing[0].address; title = $title; body = $body } } }
        else { @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = $title; body = $body } } }
        $address = (Invoke-Write @($call))[0]
    }
    Save-Text (Join-Path $Staging 'notation.md') $body
    Out-Last ([ordered]@{ ok = $true; symbols = $rows.Count; card = $address })
    return
}

# --- boundary --------------------------------------------------------------------------------------
$BoundaryWork = Join-Path $Work 'boundary'
$BoundaryState = Join-Path $BoundaryWork 'state.json'
function Get-UnitAddresses {
    $data = Get-Units
    if ($data) { return @($data.units | Where-Object { $_.address } | ForEach-Object { $_ }) }
    @()
}
if ($Step -eq 'boundary') {
    New-Item -ItemType Directory -Force -Path (Join-Path $BoundaryWork 'out') | Out-Null
    $todo = @()
    foreach ($u in @(Get-UnitAddresses | Where-Object { $_.form -in @('definition') + $Results })) {
        $model = Get-Zt $u.address 'get_model'
        if (-not $model -or -not $model.math) { continue }
        $facets = @($model.parts | ForEach-Object { [string]$_.facet })
        $need = @()
        if ($u.form -eq 'definition') {
            if (-not @($facets | Where-Object { $_ -match '^(例子|example)' }).Count) { $need += 'example' }
            if (-not @($facets | Where-Object { $_ -match '^(非例|non-example)' }).Count) { $need += 'non_example' }
        } else {
            $roles = @($facets | Where-Object { $_ -match '^假设作用[：:]\s*(H\d+)' } | ForEach-Object { if ($_ -match '(H\d+)') { $Matches[1] } })
            $missing = @(@($model.math.labels) | Where-Object { $_ -match '^H' -and $_ -notin $roles })
            if ($missing.Count) { $need += "hypotheses:$($missing -join ',')" }
        }
        if ($need.Count) { $todo += [ordered]@{ unit = $u; model = $model; need = $need } }
    }
    $jobs = @()
    $batches = [ordered]@{}
    $n = 0
    for ($i = 0; $i -lt $todo.Count; $i += $UnitsPerJob) {
        $n++
        $name = 'b-{0:d2}' -f $n
        $share = @($todo[$i..([Math]::Min($i + $UnitsPerJob, $todo.Count) - 1)])
        $sb = [Text.StringBuilder]::new()
        [void]$sb.AppendLine("# 边界任务 $name：《$($manifest.book_title)》").AppendLine()
        [void]$sb.AppendLine("把结果写入 ``$(Join-Path $BoundaryWork "out\$name.json")``。只读本文件，不读其他文件。补写许可：``$Supply``。").AppendLine()
        foreach ($t in $share) {
            $u = $t.unit
            [void]$sb.AppendLine("## [[$($u.address)]] $($u.id)（$($u.form)）").AppendLine()
            [void]$sb.AppendLine("要写：$($t.need -join '；')").AppendLine()
            [void]$sb.AppendLine('### 模型卡正文').AppendLine().AppendLine([string]$t.model.head.body).AppendLine()
            $steps = @($t.model.build.order | Where-Object { $_.line -eq 'main' } | ForEach-Object { "- [[$($_.address)]] $($_.facet)：$($_.title)" })
            if ($steps.Count) { [void]$sb.AppendLine('### 主线（证明的各步）').AppendLine().AppendLine($steps -join "`n").AppendLine() }
            $bodies = @($t.model.parts | Where-Object { $_.facet -match '^(证明|推导|前提)' } | ForEach-Object { "#### [[$($_.address)]] $($_.facet)`n$($_.body)" })
            if ($bodies.Count) { [void]$sb.AppendLine('### 证明分卡正文').AppendLine().AppendLine(($bodies -join "`n`n")).AppendLine() }
            [void]$sb.AppendLine('### 原文').AppendLine().AppendLine((Get-Section $u.section $u.line $u.end 60)).AppendLine()
        }
        $job = Join-Path $BoundaryWork "$name.md"
        Save-Text $job $sb.ToString()
        $batches[$name] = @($share | ForEach-Object { $_.unit.address })
        $jobs += [ordered]@{ id = $name; files = @((Join-Path $PSScriptRoot 'dsh-math-boundary-instructions.zh.md'), $job); prompt = "按以上说明写出 $name 的 JSON 文件。最后只回复一行 JSON。" }
    }
    Save-Json $BoundaryState ([ordered]@{ batches = $batches; created = [ordered]@{} })
    Out-Last ([ordered]@{ units = $todo.Count; jobs = $jobs })
    return
}
if ($Step -eq 'boundary-apply') {
    $state = Read-Json $BoundaryState
    $reviews = @()
    $made = 0
    $skipped = 0
    foreach ($name in @($state.batches.Keys)) {
        $file = Join-Path $BoundaryWork "out\$name.json"
        if (-not (Test-Path -LiteralPath $file)) { continue }
        $answer = Read-Json $file
        $created = @()
        foreach ($item in @($answer.items)) {
            $unit = [string]$item.unit
            if ($unit -notin @($state.batches[$name])) { continue }
            # Under supply none, only what the book gives.
            $pieces = @()
            if ($item.hypothesis) {
                $ce = $item.counterexample
                if ($ce -and $ce.title -and -not ($ce.supplied -and $Supply -eq 'none')) {
                    $mark = if ($ce.supplied -and ([string]$ce.body) -notmatch '补：书中未给') { '（补：书中未给）' } else { '' }
                    $pieces += [ordered]@{ facet = "反例：去掉 $($item.hypothesis)"; title = [string]$ce.title; body = "反例 [[$unit]]：$(([string]$ce.body).Trim())$mark"; supplied = [bool]$ce.supplied; role = $false }
                } elseif ($ce -and $ce.supplied) { $skipped++ }
                $used = if ($item.used_in) { "使用 $($item.hypothesis) 的步骤：[[$($item.used_in)]]。" } else { '' }
                $pieces += [ordered]@{ facet = "假设作用：$($item.hypothesis)"; title = [string]$item.title; body = "$(([string]$item.role).Trim())$(if ($used) { "`n`n$used" })"; supplied = $false; role = $true; hypothesis = [string]$item.hypothesis }
            }
            foreach ($kind in @('example', 'non_example')) {
                $e = $item[$kind]
                if (-not $e -or -not $e.title) { continue }
                if ($e.supplied -and $Supply -eq 'none') { $skipped++; continue }
                $mark = if ($e.supplied -and ([string]$e.body) -notmatch '补：书中未给') { '（补：书中未给）' } else { '' }
                $facet = if ($kind -eq 'example') { '例子' } else { '非例' }
                $word = if ($kind -eq 'example') { '例示' } else { '对照' }
                $pieces += [ordered]@{ facet = $facet; title = [string]$e.title; body = "$word [[$unit]]：$(([string]$e.body).Trim())$mark"; supplied = [bool]$e.supplied; role = $false; emphasis = 'core' }
            }
            # A counterexample first, so its hypothesis role can link it.
            $last = $null
            foreach ($p in $pieces) {
                $body = $p.body
                if ($p.role -and $last) { $body = "$body`n`n去掉 $($p.hypothesis) 的反例：[[$last]]。" }
                $args = @{ at = $unit; kind = 'side'; facet = $p.facet; title = $p.title; body = $body }
                if ($p.emphasis) { $args.emphasis = $p.emphasis }
                $address = (Invoke-Write @(@{ tool = 'create_successor'; arguments = $args }))[0]
                $made++
                $created += [ordered]@{ address = $address; unit = $unit; facet = $p.facet; supplied = $p.supplied; partner = $(if ($p.role) { $last } else { $null }) }
                $last = if ($p.facet -match '^反例') { $address } else { $null }
            }
        }
        $state.created[$name] = $created
        $supplied = @($created | Where-Object { $_.supplied })
        if ($supplied.Count) {
            $sb = [Text.StringBuilder]::new()
            [void]$sb.AppendLine("# 复核任务 $name").AppendLine()
            [void]$sb.AppendLine("把结论写入 ``$(Join-Path $BoundaryWork "out\v-$name.json")``。只读本文件。").AppendLine()
            foreach ($c in $supplied) {
                $unitCard = Get-Zt $c.unit
                $card = Get-Zt $c.address
                [void]$sb.AppendLine("## [[$($c.address)]] $($c.facet)：$($card.title)").AppendLine().AppendLine([string]$card.body).AppendLine()
                [void]$sb.AppendLine("### 所属单元 [[$($c.unit)]] $($unitCard.title)").AppendLine().AppendLine([string]$unitCard.body).AppendLine()
            }
            $job = Join-Path $BoundaryWork "v-$name.md"
            Save-Text $job $sb.ToString()
            $reviews += [ordered]@{ id = "v-$name"; files = @((Join-Path $PSScriptRoot 'dsh-math-boundary-review.zh.md'), $job); prompt = "按以上说明复核这些补写的分卡，写出 v-$name 的 JSON 文件。最后只回复一行 JSON。" }
        }
    }
    Save-Json $BoundaryState $state
    Out-Last ([ordered]@{ created = $made; skipped_supplied = $skipped; jobs = $reviews })
    return
}
# Folgezettel order as a sortable key: numbers by value, side letters by length then alphabet.
function Get-OrderKey([string]$address) {
    (($address -split '[/|]') | ForEach-Object { if ($_ -match '^\d+$') { 'N{0:d8}' -f [int]$_ } else { 'L{0:d3}{1}' -f $_.Length, $_ } }) -join '/'
}
if ($Step -eq 'boundary-settle') {
    $state = Read-Json $BoundaryState
    $deleted = @()
    $wrong = [ordered]@{}
    $kept = 0
    foreach ($name in @($state.created.Keys)) {
        $file = Join-Path $BoundaryWork "out\v-$name.json"
        if (-not (Test-Path -LiteralPath $file)) { continue }
        $verdicts = @((Read-Json $file).verdicts)
        foreach ($c in @($state.created[$name] | Where-Object { $_.supplied })) {
            $v = @($verdicts | Where-Object { [string]$_.address -eq $c.address })[0]
            if (-not $v -or [string]$v.verdict -ne 'wrong') { $kept++; continue }
            # The part that names a wrong counterexample goes with it.
            $wrong[$c.address] = [string]$v.reason
            foreach ($p in @($state.created[$name] | Where-Object { $_.partner -eq $c.address })) { $wrong[$p.address] = [string]$v.reason }
        }
    }
    # Delete the last Card first: deleting a Side successor renames its later siblings.
    foreach ($address in @($wrong.Keys | Sort-Object { Get-OrderKey $_ } -Descending)) {
        $plan = @(Invoke-Zt @(@{ tool = 'delete_card'; arguments = @{ at = $address } }))[0]
        if ($plan.confirm) { $null = Invoke-Zt @(@{ tool = 'delete_card'; arguments = @{ at = $address; confirm = [string]$plan.confirm } }) }
        $deleted += [ordered]@{ address = $address; reason = $wrong[$address] }
    }
    Out-Last ([ordered]@{ kept = $kept; deleted = $deleted.Count; parts = $deleted })
    return
}

# --- proof ---------------------------------------------------------------------------------------
$ProofWork = Join-Path $Work 'proof'
if ($Step -eq 'proof') {
    New-Item -ItemType Directory -Force -Path (Join-Path $ProofWork 'out') | Out-Null
    $todo = @()
    foreach ($u in @(Get-UnitAddresses | Where-Object { $_.form -in $Results })) {
        $model = Get-Zt $u.address 'get_model'
        if (-not $model -or $model.level -ne 'formal') { continue }
        $steps = @($model.build.order | Where-Object { $_.line -eq 'main' -and [string]$_.facet -match '^(前提|证明|结论|premises|proof|conclusion)' })
        if ($steps.Count) { $todo += [ordered]@{ unit = $u; model = $model; steps = $steps } }
    }
    $jobs = @()
    $n = 0
    for ($i = 0; $i -lt $todo.Count; $i += $UnitsPerJob) {
        $n++
        $name = 'p-{0:d2}' -f $n
        $share = @($todo[$i..([Math]::Min($i + $UnitsPerJob, $todo.Count) - 1)])
        $sb = [Text.StringBuilder]::new()
        [void]$sb.AppendLine("# 证明复核任务 $name").AppendLine()
        [void]$sb.AppendLine("把结果写入 ``$(Join-Path $ProofWork "out\$name.json")``。只读本文件。").AppendLine()
        foreach ($t in $share) {
            [void]$sb.AppendLine("## 单元 [[$($t.unit.address)]] $($t.model.head.title)").AppendLine().AppendLine([string]$t.model.head.body).AppendLine()
            foreach ($s in $t.steps) {
                $part = @($t.model.parts | Where-Object { $_.address -eq $s.address })[0]
                [void]$sb.AppendLine("### [[$($s.address)]] $($s.facet)：$($s.title)$(if ($s.emphasis -eq 'core') { '（关键步）' })").AppendLine().AppendLine([string]$part.body).AppendLine()
            }
        }
        $job = Join-Path $ProofWork "$name.md"
        Save-Text $job $sb.ToString()
        $jobs += [ordered]@{ id = $name; files = @((Join-Path $PSScriptRoot 'dsh-math-proof-review.zh.md'), $job); prompt = "按以上说明复核这些证明，写出 $name 的 JSON 文件。最后只回复一行 JSON。" }
    }
    Out-Last ([ordered]@{ units = $todo.Count; jobs = $jobs })
    return
}
if ($Step -eq 'proof-apply') {
    $fixed = 0; $refused = @(); $ok = 0
    foreach ($file in @(Get-ChildItem -LiteralPath (Join-Path $ProofWork 'out') -Filter 'p-*.json' -File -ErrorAction SilentlyContinue)) {
        foreach ($v in @((Read-Json $file.FullName).verdicts)) {
            if ([string]$v.verdict -ne 'fix') { $ok++; continue }
            $new = [string]$v.body
            if (-not $new.Trim()) { $refused += [ordered]@{ address = [string]$v.address; reason = "no body: $($v.reason)" }; continue }
            $card = Get-Zt ([string]$v.address)
            $old = @([regex]::Matches([string]$card.body, '\[\[([^\]]+)\]\]') | ForEach-Object { $_.Groups[1].Value } | Select-Object -Unique)
            $kept = @([regex]::Matches($new, '\[\[([^\]]+)\]\]') | ForEach-Object { $_.Groups[1].Value })
            $lost = @($old | Where-Object { $_ -notin $kept })
            if ($lost.Count) { $refused += [ordered]@{ address = [string]$v.address; reason = "drops Links: $($lost -join ', ')" }; continue }
            $null = Invoke-Write @(@{ tool = 'edit_card'; arguments = @{ at = [string]$v.address; body = $new } })
            $fixed++
        }
    }
    Out-Last ([ordered]@{ ok = $ok; fixed = $fixed; refused = $refused.Count; parts = $refused })
    return
}

# --- theory ----------------------------------------------------------------------------------------
$TheoryWork = Join-Path $Work 'theory'
function Get-Theories {
    $data = Get-Units
    if (-not $data) { return @() }
    @($data.theories | Where-Object { $_.address })
}
if ($Step -eq 'theory') {
    New-Item -ItemType Directory -Force -Path (Join-Path $TheoryWork 'out') | Out-Null
    $theories = Get-Theories
    $jobs = @()
    $n = 0
    for ($i = 0; $i -lt $theories.Count; $i += 4) {
        $n++
        $name = 't-{0:d2}' -f $n
        $share = @($theories[$i..([Math]::Min($i + 4, $theories.Count) - 1)])
        $sb = [Text.StringBuilder]::new()
        [void]$sb.AppendLine("# 理论任务 $name：《$($manifest.book_title)》").AppendLine()
        [void]$sb.AppendLine("把结果写入 ``$(Join-Path $TheoryWork "out\$name.json")``。只读本文件。主要结果至多 $MainResults 个。").AppendLine()
        foreach ($t in $share) {
            $model = Get-Zt $t.address 'get_model'
            [void]$sb.AppendLine("## 理论卡 [[$($t.address)]]：$($t.chapter)").AppendLine()
            [void]$sb.AppendLine('| 单元 | 形式 | 标题 | 被引次数 | 引用本章单元 |').AppendLine('| --- | --- | --- | --- | --- |')
            foreach ($u in @($model.math.order)) { [void]$sb.AppendLine("| [[$($u.address)]] | $($u.form) | $($u.title) | $($u.load) | $(@($u.cites) -join ' ') |") }
            [void]$sb.AppendLine()
            foreach ($u in @($model.math.order | Where-Object { $_.form -in 'theorem', 'definition' })) {
                $card = Get-Zt $u.address
                $statement = ([string]$card.body -split "`n" | Select-Object -First 8) -join "`n"
                [void]$sb.AppendLine("### [[$($u.address)]] $($u.title)").AppendLine().AppendLine($statement).AppendLine()
            }
        }
        $job = Join-Path $TheoryWork "$name.md"
        Save-Text $job $sb.ToString()
        $jobs += [ordered]@{ id = $name; files = @((Join-Path $PSScriptRoot 'dsh-math-theory-instructions.zh.md'), $job); prompt = "按以上说明写出 $name 的 JSON 文件。最后只回复一行 JSON。" }
    }
    Out-Last ([ordered]@{ theories = $theories.Count; jobs = $jobs })
    return
}
# The dependency map: one column per depth, each unit a box that opens its Card.
function Get-Map($order) {
    $units = @($order)
    if (-not $units.Count) { return '' }
    $depth = @{}
    foreach ($u in $units) {
        $d = 0
        foreach ($c in @($u.cites)) { if ($depth.ContainsKey($c)) { $d = [Math]::Max($d, $depth[$c] + 1) } }
        $depth[[string]$u.address] = $d
    }
    $columns = @{}
    $pos = @{}
    foreach ($u in $units) {
        $d = $depth[[string]$u.address]
        $row = [int]$columns[$d]
        $columns[$d] = $row + 1
        $pos[[string]$u.address] = @($d, $row)
    }
    $w = 170; $h = 46; $bw = 140; $bh = 34
    $cols = ($depth.Values | Measure-Object -Maximum).Maximum + 1
    $rows = ($columns.Values | Measure-Object -Maximum).Maximum
    $svg = [Text.StringBuilder]::new()
    [void]$svg.Append("<svg viewBox=`"0 0 $($cols * $w) $($rows * $h + 8)`" width=`"100%`" xmlns=`"http://www.w3.org/2000/svg`" font-size=`"11`" font-family=`"sans-serif`">")
    foreach ($u in $units) {
        $to = $pos[[string]$u.address]
        foreach ($c in @($u.cites)) {
            if (-not $pos.ContainsKey($c)) { continue }
            $from = $pos[$c]
            [void]$svg.Append("<line x1=`"$($from[0] * $w + $bw + 4)`" y1=`"$($from[1] * $h + 4 + $bh / 2)`" x2=`"$($to[0] * $w + 4)`" y2=`"$($to[1] * $h + 4 + $bh / 2)`" stroke=`"#999`" stroke-width=`"1`"/>")
        }
    }
    foreach ($u in $units) {
        $p = $pos[[string]$u.address]
        $x = $p[0] * $w + 4; $y = $p[1] * $h + 4
        $weight = [string]$u.weight
        $stroke = switch ($weight) { 'focus' { '#333' } 'support' { '#888' } default { '#bbb' } }
        $fill = if ($weight -eq 'focus') { '#fffbe6' } else { '#fff' }
        $label = ([string]$u.title -replace '[<>&"]', '')
        if ($label.Length -gt 11) { $label = $label.Substring(0, 11) + '…' }
        $zh = $FormZh[[string]$u.form]
        [void]$svg.Append("<g data-zt=`"$($u.address)`" style=`"cursor:pointer`"><rect x=`"$x`" y=`"$y`" width=`"$bw`" height=`"$bh`" rx=`"3`" fill=`"$fill`" stroke=`"$stroke`"/><text x=`"$($x + 6)`" y=`"$($y + 14)`" fill=`"#666`">$zh</text><text x=`"$($x + 6)`" y=`"$($y + 28)`" fill=`"#222`">$label</text></g>")
    }
    [void]$svg.Append('</svg>')
    "``````chart`n<figure>$($svg.ToString())<figcaption>依赖图：左边的单元先学，线指向引用它的单元。</figcaption></figure>`n``````"
}
if ($Step -eq 'theory-apply') {
    $theories = Get-Theories
    $answers = @{}
    foreach ($file in @(Get-ChildItem -LiteralPath (Join-Path $TheoryWork 'out') -Filter 't-*.json' -File -ErrorAction SilentlyContinue)) {
        foreach ($t in @((Read-Json $file.FullName).theories)) { $answers[[string]$t.address] = $t }
    }
    $written = 0
    foreach ($t in $theories) {
        $model = Get-Zt $t.address 'get_model'
        $card = Get-Zt $t.address
        $body = [string]$card.body
        $members = @($model.math.order | ForEach-Object { [string]$_.address })
        $a = $answers[[string]$t.address]
        if ($a) {
            if ($a.question) { $body = Set-Section $body '问题' "**问题**：$(([string]$a.question).Trim())" }
            $mains = @($a.main_results | Where-Object { [string]$_.address -in $members } | Select-Object -First $MainResults | ForEach-Object { "- [[$($_.address)]]：$(([string]$_.line).Trim())" })
            if ($mains.Count) { $body = Set-Section $body '主要结果' "**主要结果**：`n$($mains -join "`n")" }
            $objects = @($a.objects | Where-Object { [string]$_.address -in $members } | ForEach-Object { "- [[$($_.address)]]：$(([string]$_.line).Trim())" })
            if ($objects.Count) { $body = Set-Section $body '对象' "**对象**：`n$($objects -join "`n")" }
        }
        # Recompute after the main results changed: the weights depend on them.
        $title = if ($a -and $a.title) { ([string]$a.title).Trim() } else { $card.title -replace '（待写）$', '' }
        $null = Invoke-Write @(@{ tool = 'edit_card'; arguments = @{ at = $t.address; title = $title; body = $body } })
        $model = Get-Zt $t.address 'get_model'
        $order = @($model.math.order)
        $structure = "**结构**（依赖顺序）：`n" + (($order | ForEach-Object { "- [[$($_.address)]] $($FormZh[[string]$_.form])：$($_.title)" }) -join "`n")
        $body = Set-Section $body '结构' $structure
        $tools = @($model.math.tools | ForEach-Object { "- 依赖 [[$($_.address)]]：$($_.title)" })
        $body = Set-Section $body '所用工具' $(if ($tools.Count) { "**所用工具**：`n$($tools -join "`n")" } else { '' })
        $users = @($model.math.users | Where-Object { $_.theory } | ForEach-Object { "- 引出 [[$($_.theory.address)]]：$($_.theory.title)" })
        $body = Set-Section $body '后续' $(if ($users.Count) { "**后续**：`n$($users -join "`n")" } else { '' })
        $body = Set-Section $body '图' "**图**：`n`n$(Get-Map $order)"
        $null = Invoke-Write @(@{ tool = 'edit_card'; arguments = @{ at = $t.address; body = $body } })
        $written++
    }
    Out-Last ([ordered]@{ ok = $true; theories = $written; answered = $answers.Count })
    return
}

# --- check -----------------------------------------------------------------------------------------
if ($Step -eq 'check') {
    if (-not $Out) { $Out = $Staging }
    $tree = @(Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $key } }))[0]
    $heads = @($tree.cards | Where-Object { $_.type -eq 'model' -and -not $_.facet -and $_.form })
    $models = if ($heads.Count) { @(Invoke-Zt @($heads | ForEach-Object { @{ tool = 'get_model'; arguments = @{ at = $_.address } } })) } else { @() }
    $faults = [Collections.Generic.List[object]]::new()
    $add = { param($rule, $address, $text) $faults.Add([ordered]@{ rule = $rule; address = $address; text = $text }) }
    $methods = '直接|反证|逆否|归纳|构造|分情形|对角线|direct|contradiction|contrapositive|induction|construction|cases|diagonal'
    for ($i = 0; $i -lt $heads.Count; $i++) {
        $head = $heads[$i]
        $model = $models[$i]
        $form = [string]$head.form
        $address = [string]$head.address
        $card = Get-Zt $address
        $body = [string]$card.body
        if ($form -eq 'theory') {
            foreach ($f in @($model.math.faults)) { & $add 'M17' $address $f.fix }
            foreach ($c in @($model.math.cycle)) { & $add 'M4' $address $c.fix }
            if ($body -match '待写') { & $add 'M1' $address 'Theory Card still holds 待写' }
            continue
        }
        # M1: one claim in the title, no skeleton left.
        if ([string]$card.title -match '待写' -or $body -match '待写') { & $add 'M1' $address 'the unit still holds 待写' }
        if (([string]$card.title).Length -gt 40) { & $add 'M1' $address "title over 40 characters: $($card.title)" }
        # M2: labels.
        $labels = @($model.math.labels)
        if ($form -in $Results -and -not @($labels | Where-Object { $_ -match '^H' }).Count) { & $add 'M2' $address 'a result labels its hypotheses (H1), (H2), …' }
        if ($form -eq 'definition' -and -not @($labels | Where-Object { $_ -match '^C' }).Count) { & $add 'M2' $address 'a definition labels its clauses (C1), (C2), …' }
        # M3: reach-in.
        foreach ($r in @($model.build.reach_in)) { & $add 'M3' $address $r.fix }
        foreach ($c in @($model.build.closure)) { & $add 'M3' $address $c.fix }
        # M5: each hypothesis used in a proof part, or explained by its role part.
        $proof = @($model.parts | Where-Object { [string]$_.facet -match '^(证明|推导|结论|假设作用|proof|derivation|conclusion|hypothesis)' })
        if ($form -in $Results -and $head.level -eq 'formal' -and $proof.Count) {
            $text = ($proof | ForEach-Object { "$($_.title)`n$($_.body)" }) -join "`n"
            foreach ($h in @($labels | Where-Object { $_ -match '^H' })) { if ($text -notmatch "\b$h\b") { & $add 'M5' $address "hypothesis $h appears in no proof step and no 假设作用 part" } }
        }
        # M6, M7, M13, M14: the unit report.
        foreach ($f in @($model.math.faults)) {
            $rule = switch ([string]$f.fault) { 'no_example' { 'M6' } 'no_nonexample' { 'M6' } 'no_intuition' { 'M7' } 'no_plan' { 'M7' } 'over_budget' { 'M13' } default { 'M14' } }
            & $add $rule $address $f.fix
        }
        # M8: the proof plan names its method.
        foreach ($p in @($model.parts | Where-Object { [string]$_.facet -match '^(证明思路|proof plan)' })) {
            if ("$($p.title)`n$($p.body)" -notmatch $methods) { & $add 'M8' $p.address 'the proof plan names no method (直接, 反证, 逆否, 归纳, 构造, 分情形, 对角线)' }
        }
        # M9: a formal result states its symbols (the setting's 符号表) or cites the notation table.
        if ($form -in $Results -and $head.level -eq 'formal') {
            $all = ($model.parts | ForEach-Object { [string]$_.body }) -join "`n"
            if ($all -notmatch '符号表' -and @($model.build.cited | Where-Object { [string]$_.title -match '^符号表' }).Count -eq 0) { & $add 'M9' $address 'no symbol table in the premises and no Link to the notation table' }
        }
        # M10: a supplied part is marked.
        foreach ($p in @($model.parts | Where-Object { [string]$_.facet -match '^(反例|非例|例子)' })) {
            if ([string]$p.body -match '补' -and [string]$p.body -notmatch '（补：书中(未给|略去|未写)）') { & $add 'M10' $p.address 'a supplied item carries the mark （补：书中未给）' }
        }
        # M12: an exercise answer stands under ## 答案.
        foreach ($p in @($model.parts | Where-Object { [string]$_.facet -match '^(习题|exercise)' })) {
            if ([string]$p.body -match '答案' -and [string]$p.body -notmatch '(?m)^##\s*答案') { & $add 'M12' $p.address 'an exercise answer stands under ## 答案' }
        }
        # M16: the Model Card states the proof idea in one sentence.
        if ($body -match '(?s)\*\*思路\*\*[：:]?(.*?)(\n\s*\n|\*\*|$)') {
            $idea = ($Matches[1] -replace '\$[^$]*\$', 'x')
            if (@([regex]::Matches($idea, '[。！？]')).Count -gt 1) { & $add 'M16' $address 'the Model Card states the proof idea in more than one sentence' }
        }
    }
    $byRule = [ordered]@{}
    foreach ($f in $faults) { $byRule[$f.rule] = 1 + [int]$byRule[$f.rule] }
    $forms = [ordered]@{}
    foreach ($h in $heads) { $forms[[string]$h.form] = 1 + [int]$forms[[string]$h.form] }
    $report = [ordered]@{ key = $key; units = @($heads | Where-Object { $_.form -ne 'theory' }).Count; theories = @($heads | Where-Object { $_.form -eq 'theory' }).Count; forms = $forms; faults = $faults.Count; by_rule = $byRule; list = @($faults) }
    Save-Json (Join-Path $Out 'math-audit.json') $report
    $md = [Text.StringBuilder]::new()
    [void]$md.AppendLine("# 数学规则审计：$key").AppendLine()
    [void]$md.AppendLine("单元 $($report.units) 个，理论卡 $($report.theories) 张，问题 $($faults.Count) 处。").AppendLine()
    [void]$md.AppendLine('| 规则 | 数量 |').AppendLine('| --- | --- |')
    foreach ($k in $byRule.Keys) { [void]$md.AppendLine("| $k | $($byRule[$k]) |") }
    [void]$md.AppendLine().AppendLine('## 问题').AppendLine()
    foreach ($f in $faults) { [void]$md.AppendLine("- $($f.rule) [[$($f.address)]]：$($f.text)") }
    Save-Text (Join-Path $Out 'math-audit.md') $md.ToString()
    Out-Last ([ordered]@{ ok = $true; units = $report.units; theories = $report.theories; forms = $forms; faults = $faults.Count; by_rule = $byRule; report = (Join-Path $Out 'math-audit.md') })
    return
}
