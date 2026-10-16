#Requires -Version 7
<#
.SYNOPSIS
  The deterministic half of the card layer cycle (doc/digest-card-layers-proposal.md, proposals 3,
  4, and 6): check the Idea Cards, Event Cards, and anchors of one book, hand their faults to
  DeepSeek as jobs, check what DeepSeek writes, apply it, check again, and report. DeepSeek writes
  only JSON files; this script makes every zt write. It never writes a Model Card, a Model part,
  a Hub Card, or a Topic root.

.DESCRIPTION
  -Work is the cycle's folder and the DeepSeek Harness cwd. It holds state.json, the job files,
  out\ (the JSON files DeepSeek writes), before\ and after\ (digest-audit.ps1 runs), report.json,
  and decisions.json. Every step prints compact JSON as its last line. A step that hands work to
  DeepSeek prints {"jobs":[{"id","files":[...],"prompt","continue"?,"fallback"?}], ...}.

    plan     runs digest-audit.ps1 on -Book; picks its Idea Cards, Event Cards, and anchors (only
             the Cards of -Groups when given: owned-gNN.txt in -Staging and the anchors); writes
             one fix job per -PerJob Cards with faults of -Rules, a review job for a -Sample of
             the Cards without faults (only with -Staging: the reviewer reads the source), and one
             orphan job per -PerJob Cards that no Card other than the parent links (I5, E4).
    apply    reads out\fix-*.json and out\orphans-*.json of the jobs not applied yet. A rewrite
             must keep every Link, figure, and formula of the Card, and the Card must be unchanged
             since the plan. It writes the rewrites (edit_card with base), the splits (a Side
             successor), and the orphan Links (append_card on a candidate, with a kind word before
             the Link). merge verdicts go to decisions.json; a refusal goes to the next check.
    check    runs digest-audit.ps1 again; prints one follow-up job per fix job whose Cards still
             have faults of -Rules or were refused. The follow-up continues the job's conversation.
    report   runs digest-audit.ps1 again; compares the faults of the picked Cards before and
             after, per rule; writes report.json and decisions.json.
    precheck reads the JSON file of one job (-Job; -File, or the job's `out` in state.json) and
             checks it the way apply and the audit will, without a write: no zt call, no change to
             state.json. Every job file names this command in its `SELF-CHECK:` line, so DeepSeek
             checks its draft before it replies. A fix entry: a known verdict, an address of the
             job, a writable category, the rewrite test (every Link, figure, and formula kept, no
             Link to an unknown Card), then the Card language and layer rules of -Rules on the new
             title and body and on each split Card (zt-check --json, with the Link kinds
             as ZT reads them). Rules that need the whole Workspace are left out: I5, E4, I6, E5,
             and L3 (advisory). An orphan entry: the orphan and the candidate belong to the job, the
             text links the orphan with a kind word before the Link, no semicolon, no Link to an
             unknown Card, and L2 (with the R rules and L1 of -Rules) on the sentence. Output: one
             line per fault, `<address> <rule or check> <text>`, then `faults: N` as the last line.
             The check `refuse` names what apply refuses; a split Card is `<address>+split<n>`; an
             orphan Link is `<orphan><-<candidate>`.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('plan', 'apply', 'check', 'report', 'precheck')][string]$Step,
    [Parameter(Mandatory)][string]$Work,
    [string]$Book = '',
    [string]$Staging = '',
    [string]$Groups = '',
    [int]$PerJob = 8,
    [double]$Sample = 0,
    [int]$Seed = 1,
    # R6 is left out: the book term table must be checked before the cycle rewrites Cards by it.
    [string]$Rules = 'R2,R3,R4,R5,R7,R9,R12,I1,I2,I3,I4,L1,L2,E1,E2',
    [int]$MaxCards = 0,
    [switch]$NoOrphans,
    # Organize notes: a JSON array of {"address","note"}, or decisions.json with a "notes" field.
    # Each note is a fault N1 of its Card.
    [string]$Notes = '',
    # Compact job lists: {id, kind, file, continue?} plus "lead" and "prompts" once, for a book with
    # hundreds of jobs (the workflow expands them). Without it the output is as before.
    [switch]$Compact,
    # apply: at most this many jobs per call (0: all); the output's pending_jobs counts the rest.
    [int]$MaxJobs = 0,
    # precheck: the job whose JSON file is checked, and that file (default: the job's `out`).
    [string]$Job = '',
    [string]$File = ''
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
$Helper = $script:ZtHelper
$Audit = Join-Path $PSScriptRoot 'digest-audit.ps1'
$FixDoc = Join-Path $PSScriptRoot 'dsh-card-instructions.zh.md'
$OrphanDoc = Join-Path $PSScriptRoot 'dsh-orphan-instructions.zh.md'
$OutDir = Join-Path $Work 'out'
# precheck writes nothing, not even the folders.
if ($Step -ne 'precheck') { New-Item -ItemType Directory -Force -Path $Work, $OutDir | Out-Null }
$StateFile = Join-Path $Work 'state.json'
# The words before a Link that ZT reads as a Relation kind (src/model.rs RELATION_KINDS).
$Cues = @('依赖', '依据', '基于', '建立在', '取自', '沿用', '承接', '用到', '借用', '代入', '推导自', '由此推出', '按定义',
    '例示', '举例', '例子', '示例', '演示', '例证', '实例', '案例', '印证', '对照', '对比', '相比', '不同于', '区别', '相反', '逆命题',
    '放宽', '放松', '推广', '扩展', '拓展', '一般化', '对应', '类比', '同构', '相当于', '等价', '参照',
    '反驳', '批评', '质疑', '反例', '否定', '挑战', '特例', '特殊情形')
# The English cues of the same table. ZT matches them in lower case, never inside a word.
$CuesEn = @('substitut', 'rests on', 'builds on', 'depends on', 'relies on', 'derived from', 'follows from', 'uses', 'cites',
    'by definition', 'by theorem', 'by lemma', 'by proposition', 'by corollary', 'by axiom',
    'illustrat', 'example of', 'instance of', 'for example', 'e.g.', 'contrast', 'unlike', 'compared with', 'compare', 'versus',
    'differs from', 'relax', 'extend', 'generaliz', 'correspond', 'maps onto', 'analog', 'equivalent', 'mirrors',
    'objection', 'objects to', 'critici', 'refut', 'counterexample', 'challeng', 'special case', 'specializ')
$Writable = @('idea', 'event', 'anchor')

function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String }
function Save-Text($path, $text) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
    [IO.File]::WriteAllText($path, $text, $utf8)
}
function Out-Last($object) { $object | ConvertTo-Json -Depth 20 -Compress }
# The jobs of a step in compact form: the instruction file and the prompt of each kind are given once.
function Get-CompactJobs($jobs) {
    $fixDoc = Join-Path $Work (Split-Path -Leaf $FixDoc)
    $orphanDoc = Join-Path $Work (Split-Path -Leaf $OrphanDoc)
    $prompts = [ordered]@{}
    $list = foreach ($j in @($jobs)) {
        $kind = if ($j.continue) { 'follow' } elseif ($j.id -like 'orphans-*') { 'orphans' } else { 'fix' }
        if (-not $prompts.Contains($kind)) { $prompts[$kind] = $j.prompt }
        $c = [ordered]@{ id = $j.id; kind = $kind; file = Split-Path -Leaf @($j.files)[-1] }
        if ($j.continue) { $c.continue = $j.continue }
        $c
    }
    [ordered]@{ dir = $Work; lead = [ordered]@{ fix = $fixDoc; orphans = $orphanDoc }; prompts = $prompts; jobs = @($list) }
}
# The line of a job file that names the job's precheck command, with absolute paths.
function Get-SelfCheck([string]$id) {
    $dir = (Resolve-Path -LiteralPath $Work).Path
    "SELF-CHECK: pwsh -NoProfile -File ""$PSCommandPath"" -Step precheck -Work ""$dir"" -Job $id"
}
function Add-SelfCheck([Text.StringBuilder]$sb, [string]$id) {
    [void]$sb.AppendLine('写完 JSON 文件后，按说明的“自查”一节运行这条自查命令：').AppendLine().AppendLine((Get-SelfCheck $id)).AppendLine()
}
$State = if (Test-Path -LiteralPath $StateFile) { Read-Json $StateFile } else { $null }
function Save-State { Save-Text $StateFile ($State | ConvertTo-Json -Depth 20) }

# --- the audit -----------------------------------------------------------------------------------
function Invoke-Audit([string]$dir) {
    $lines = @(pwsh -NoProfile -File $Audit -CitationKey $State.book -Out $dir -SkipModels 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "digest-audit failed: $($lines -join ' | ')" }
    $faults = @{}
    foreach ($line in (Get-Content -LiteralPath (Join-Path $dir 'audit-faults.txt') -Encoding utf8)) {
        $p = $line -split ' ', 3
        if ($p.Count -lt 3) { continue }
        if (-not $faults.ContainsKey($p[0])) { $faults[$p[0]] = [Collections.Generic.List[object]]::new() }
        $faults[$p[0]].Add(@{ rule = $p[1]; text = $p[2] })
    }
    $cards = @{}
    foreach ($c in @(Read-Json (Join-Path $dir 'audit-cards.json'))) { $cards[[string]$c.address] = $c }
    @{ faults = $faults; cards = $cards }
}
function Get-RuleList { @($State.rules -split '[,\s]+' | Where-Object { $_ }) }
function Get-HardFaults($audit, [string]$address) {
    $rules = Get-RuleList
    @($(if ($audit.faults.ContainsKey($address)) { $audit.faults[$address] } else { @() }) | Where-Object { $_.rule -in $rules })
}

# --- writes --------------------------------------------------------------------------------------
# Writes through the helper, one batch; when a call fails, each call again alone, so one refusal
# stops nothing else. Returns one entry per call: the address, or the error.
function Invoke-Writes($calls) {
    $calls = @($calls)
    $results = [object[]]::new($calls.Count)
    $start = 0
    # The helper stops at the first failed call: record it, then go on after it. The calls before
    # it are done and are never sent again.
    while ($start -lt $calls.Count) {
        $batch = @($calls[$start..($calls.Count - 1)])
        $lines = @(($batch | ConvertTo-Json -Depth 10 -AsArray) | pwsh -NoProfile -File $Helper -)
        $failedAt = -1
        foreach ($line in $lines) {
            if ($line -match '^(\d+) ok (.+)$') { $results[$start + [int]$Matches[1]] = @{ ok = $true; address = $Matches[2].Trim() } }
            elseif ($line -match '^(\d+) error (.*)$') { $failedAt = [int]$Matches[1]; $results[$start + $failedAt] = @{ ok = $false; error = $Matches[2] } }
        }
        if ($failedAt -lt 0) {
            for ($k = $start; $k -lt $calls.Count; $k++) { if (-not $results[$k]) { $results[$k] = @{ ok = $false; error = "no result: $($lines -join ' | ')" } } }
            break
        }
        $start += $failedAt + 1
    }
    , $results
}
# A Card a split made belongs to the group that owns the Card it came from (owned-gNN.txt), so
# the link pass and list-models.ps1 see it.
function Add-Owned([string]$from, [string]$new) {
    if (-not $State.staging) { return }
    foreach ($file in @(Get-ChildItem -LiteralPath $State.staging -Filter 'owned-g*.txt' -File -ErrorAction SilentlyContinue)) {
        if (@(Get-Content -LiteralPath $file.FullName -Encoding utf8 | ForEach-Object { $_.Trim() }) -contains $from) {
            [IO.File]::AppendAllText($file.FullName, "$new`n", $utf8)
            return
        }
    }
}
function Get-Known {
    if (-not $script:known) {
        $script:known = [Collections.Generic.HashSet[string]]::new()
        foreach ($a in Get-Content -LiteralPath (Join-Path $Work 'before\audit-addresses.txt') -Encoding utf8) { if ($a) { [void]$script:known.Add($a.Trim()) } }
    }
    , $script:known
}

# --- what a rewrite must keep --------------------------------------------------------------------
function Get-Keep([string]$text) {
    $links = @([regex]::Matches($text, '\[\[([^\]]+)\]\]') | ForEach-Object { $_.Groups[1].Value.Trim() })
    $figures = @([regex]::Matches($text, '!\[[^\]]*\]\(artifact:[0-9a-fA-F]+\)') | ForEach-Object Value)
    $display = @([regex]::Matches($text, '\$\$[\s\S]*?\$\$') | ForEach-Object { ($_.Value -replace '\s', '') })
    $inline = @([regex]::Matches(([regex]::Replace($text, '\$\$[\s\S]*?\$\$', ' ')), '\$[^$\n]+\$') | ForEach-Object { ($_.Value -replace '\s', '') })
    @{ links = @($links | Select-Object -Unique); figures = @($figures | Select-Object -Unique); formulas = @(@($display) + @($inline) | Select-Object -Unique) }
}
function Test-Rewrite($old, $entry) {
    $title = [string]$entry.title
    $body = [string]$entry.body
    if (-not $title.Trim() -or -not $body.Trim()) { return 'the title or the body is empty' }
    if ($body.TrimStart() -match '^#\s') { return 'the body starts with a # heading' }
    $new = "$title`n$body"
    if ($entry.verdict -eq 'split') {
        $parts = @($entry.split)
        if (-not $parts.Count) { return 'split names no new Card' }
        foreach ($p in $parts) {
            if (-not ([string]$p.title).Trim() -or -not ([string]$p.body).Trim()) { return 'a split Card has no title or body' }
            $new += "`n$($p.title)`n$($p.body)"
        }
    }
    $keep = Get-Keep "$($old.title)`n$($old.body)"
    $now = Get-Keep $new
    $lost = @($keep.links | Where-Object { $_ -notin $now.links })
    # An Organize note that names other Links may replace a wrongly cited Card: the rewrite may drop
    # as many Links as it adds of those the note names, and no more.
    $noteText = if ($State.notes) { [string]$State.notes[[string]$entry.address] } else { '' }
    $named = @([regex]::Matches($noteText, '\[\[([^\]]+)\]\]') | ForEach-Object { $_.Groups[1].Value.Trim() })
    $added = @($named | Where-Object { $_ -in $now.links -and $_ -notin $keep.links } | Select-Object -Unique)
    if ($lost.Count -and $lost.Count -le $added.Count) { $lost = @() }
    if ($lost.Count) { return "lost Links: $(@($lost | ForEach-Object { "[[$_]]" }) -join ' ')" }
    $made = @($now.links | Where-Object { $_ -notin $keep.links -and -not (Get-Known).Contains($_) })
    if ($made.Count) { return "Links to Cards that do not exist: $(@($made | ForEach-Object { "[[$_]]" }) -join ' ')" }
    if (@($keep.figures | Where-Object { $_ -notin $now.figures }).Count) { return 'lost a figure line' }
    $lostFormulas = @($keep.formulas | Where-Object { $_ -notin $now.formulas })
    if ($lostFormulas.Count) { return "changed formulas: $($lostFormulas[0])" }
    $null
}
# The kind word before a Link in its sentence, as ZT reads it.
function Test-Kinded([string]$text, [string]$target) {
    $at = $text.IndexOf("[[$target")
    if ($at -lt 0) { return $false }
    $start = [Math]::Max($text.LastIndexOfAny([char[]]'。！？!?', [Math]::Max(0, $at - 1)) + 1, 0)
    $before = $text.Substring($start, $at - $start)
    foreach ($cue in $Cues) { if ($before.Contains($cue)) { return $true } }
    $false
}
# The Link targets of a body that ZT reads a Relation kind for, as get_card `kinds` gives them
# (src/model.rs read_links): the cue that ends last in the words before the Link, in its sentence
# and after an earlier Link of that sentence, decides the kind. A Link with no cue there takes the
# kind of that earlier Link. 引出 is a plain reference.
function Get-LinkKinds([string]$body) {
    $kinds = [Collections.Generic.List[string]]::new()
    $lower = $body.ToLowerInvariant()
    $prevEnd = -1
    $prevKinded = $false
    foreach ($m in [regex]::Matches($body, '\[\[([^\]]+)\]\]')) {
        $start = 0
        for ($i = $m.Index - 1; $i -ge 0; $i--) {
            $ch = $body[$i]
            if ($ch -in [char[]]'。！？!?；;' -or $ch -eq "`n" -or ($ch -eq '.' -and $i + 1 -lt $body.Length -and [char]::IsWhiteSpace($body[$i + 1]))) { $start = $i + 1; break }
        }
        $shared = $prevEnd -gt $start
        $from = if ($shared) { $prevEnd } else { $start }
        $words = $lower.Substring($from, $m.Index - $from)
        # The cue that ends last; of two that end at one place, the longer.
        $best = $null
        foreach ($cue in @($Cues + $CuesEn + @('引出'))) {
            $word = $cue -match '^[a-z]'
            $at = $words.IndexOf($cue, [StringComparison]::Ordinal)
            while ($at -ge 0) {
                if (-not ($word -and $at -gt 0 -and $words[$at - 1] -match '[A-Za-z0-9]')) {
                    $end = $at + $cue.Length
                    if (-not $best -or $end -gt $best.end -or ($end -eq $best.end -and $cue.Length -gt $best.cue.Length)) { $best = @{ end = $end; cue = $cue } }
                }
                $at = $words.IndexOf($cue, $at + 1, [StringComparison]::Ordinal)
            }
        }
        $kinded = if ($best) { $best.cue -ne '引出' } elseif ($shared) { $prevKinded } else { $false }
        $target = $m.Groups[1].Value.Trim()
        if ($kinded -and -not $kinds.Contains($target)) { $kinds.Add($target) }
        $prevEnd = $m.Index + $m.Length
        $prevKinded = $kinded
    }
    $kinds.ToArray()
}

# --- plan ----------------------------------------------------------------------------------------
function Format-Card($card, $faults, [string]$parentTitle, [string]$source) {
    # An empty fault list arrives as $null, and @($null) holds one item.
    $faults = @($faults | Where-Object { $_ })
    $sb = [Text.StringBuilder]::new()
    [void]$sb.AppendLine("## 卡片 ``$($card.address)``（$($card.category)）").AppendLine()
    if ($card.parent) { [void]$sb.AppendLine("- 父卡：``$($card.parent)`` $parentTitle") }
    if ($source) { [void]$sb.AppendLine("- 原文：$source") }
    if (@($faults).Count) {
        [void]$sb.AppendLine('- 问题：')
        foreach ($f in $faults) { [void]$sb.AppendLine("  - $($f.rule) $($f.text)") }
    } else {
        [void]$sb.AppendLine('- 问题：检查脚本没有找到问题。这是抽查：对照原文核实这张卡（ok、fix、split、merge 或 unfaithful）。')
    }
    [void]$sb.AppendLine().AppendLine('### 标题').AppendLine().AppendLine([string]$card.title).AppendLine()
    [void]$sb.AppendLine('### 正文').AppendLine().AppendLine('````markdown').AppendLine(([string]$card.body).TrimEnd()).AppendLine('````').AppendLine()
    $sb.ToString()
}
function Get-SourceHint($card) {
    if (-not $State.staging) { return '' }
    $src = $State.src[[string]$card.address]
    if ($src) { return "``$(Join-Path $State.staging $src)``（只读这几行）" }
    if ([string]$card.body -match '§\s?([0-9A-Za-z.]+)') {
        return "小节 §$($Matches[1])：在 ``$(Join-Path $State.staging 'sections')`` 中用 Select-String 找到它，只读相关段落"
    }
    "在 ``$(Join-Path $State.staging 'sections')`` 中用 Select-String 找到这张卡的出处，只读相关段落"
}
# One orphan job per 2 x -PerJob orphans: each orphan with its best candidates (relatedness.ps1)
# among the Idea Cards, Event Cards, and anchors, never its parent or a Card that links it already.
function New-OrphanJobs($audit, $orphans) {
    $made = @()
    if (-not @($orphans).Count) { return }
    . (Join-Path $PSScriptRoot 'relatedness.ps1')
    $docs = @($audit.cards.Values | Where-Object { $_.category -in @('idea', 'event', 'anchor') } | ForEach-Object {
            @{ id = [string]$_.address; text = "$($_.title) $($_.title) $($_.body)" } })
    $model = New-Relatedness $docs 80
    for ($i = 0; $i -lt $orphans.Count; $i += 2 * $PerJob) {
        $chunk = @($orphans[$i..([Math]::Min($i + 2 * $PerJob - 1, $orphans.Count - 1))])
        $State.seq++
        $id = 'orphans-{0:d2}' -f $State.seq
        $out = Join-Path $OutDir "$id.json"
        $sb = [Text.StringBuilder]::new()
        [void]$sb.AppendLine("# 任务 $id").AppendLine().AppendLine("把结果写入 ``$out``。本任务共 $($chunk.Count) 张孤立卡片。").AppendLine()
        Add-SelfCheck $sb $id
        $candidates = [ordered]@{}
        foreach ($o in $chunk) {
            $a = [string]$o.address
            $isEvent = $o.category -eq 'event'
            $linking = @($audit.cards.Values | Where-Object { @($_.links) -contains $a } | ForEach-Object { [string]$_.address })
            $cands = @(Find-Related $model $a 5 {
                    param($id)
                    $id -eq [string]$o.parent -or $id -in $linking -or -not $audit.cards.ContainsKey($id) -or
                    $audit.cards[$id].category -notin $Writable -or ($isEvent -and $audit.cards[$id].category -ne 'idea')
                })
            $candidates[$a] = $cands
            [void]$sb.AppendLine("## 孤立卡片 ``$a``（$($o.category)）").AppendLine()
            [void]$sb.AppendLine("### $($o.title)").AppendLine().AppendLine('````markdown').AppendLine(([string]$o.body).TrimEnd()).AppendLine('````').AppendLine()
            [void]$sb.AppendLine('候选卡片：').AppendLine()
            foreach ($cand in $cands) {
                $cb = (([string]$audit.cards[$cand].body) -replace '\s+', ' ').Trim()
                if ($cb.Length -gt 160) { $cb = $cb.Substring(0, 160) + '…' }
                [void]$sb.AppendLine("- ``$cand`` $($audit.cards[$cand].title)：$cb")
            }
            [void]$sb.AppendLine()
        }
        $file = Join-Path $Work "$id.md"
        Save-Text $file $sb.ToString()
        $State.jobs[$id] = [ordered]@{ kind = 'orphans'; file = $file; out = $out; cards = @($chunk | ForEach-Object { [string]$_.address }); candidates = $candidates; applied = $false }
        $made += [ordered]@{ id = $id; files = @((Join-Path $Work (Split-Path -Leaf $OrphanDoc)), $file); prompt = '按以上说明为每张孤立卡片选出链接，写出 JSON 文件。最后只回复一行 JSON。' }
    }
    $made
}
function Step-Plan {
    if (-not $Book) { throw 'plan needs -Book' }
    $script:State = [ordered]@{
        book = $Book; staging = $Staging; groups = $Groups; rules = $Rules; sample = $Sample
        jobs = [ordered]@{}; seq = 0; picked = @(); before = [ordered]@{}; plan = [ordered]@{}; no_orphans = [bool]$NoOrphans
        applied = [ordered]@{}; refused = [ordered]@{}; merges = @(); splits = @(); orphan_links = @(); orphan_none = @()
        src = [ordered]@{}; notes = [ordered]@{}
    }
    $audit = Invoke-Audit (Join-Path $Work 'before')
    if ($Notes -and (Test-Path -LiteralPath $Notes)) {
        # A list of notes, or Organize's decisions.json with its "notes" field.
        $raw = Get-Content -LiteralPath $Notes -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
        # (One note alone may arrive as an object, not a list.)
        $list = if ($raw -is [Collections.IDictionary]) { if ($raw.Contains('notes')) { @($raw.notes) } else { @($raw) } } else { @($raw) }
        foreach ($n in $list) {
            $a = [string]$n.address
            if (-not $a -or -not ([string]$n.note).Trim()) { continue }
            if (-not $audit.faults.ContainsKey($a)) { $audit.faults[$a] = [Collections.Generic.List[object]]::new() }
            $audit.faults[$a].Add(@{ rule = 'N1'; text = ([string]$n.note).Trim() })
            $State.notes[$a] = ([string]$State.notes[$a] + ' ' + ([string]$n.note).Trim()).Trim()
        }
        if ('N1' -notin ($State.rules -split '[,\s]+')) { $State.rules = "$($State.rules),N1" }
    }
    $owned = $null
    if ($Groups) {
        if (-not $Staging) { throw '-Groups needs -Staging (owned-gNN.txt)' }
        $owned = [Collections.Generic.HashSet[string]]::new()
        foreach ($n in @($Groups -split '[,\s]+' | Where-Object { $_ } | ForEach-Object { [int]$_ })) {
            [void]$owned.Add("$Book/$n")
            $file = Join-Path $Staging ('owned-g{0:d2}.txt' -f $n)
            if (Test-Path -LiteralPath $file) { foreach ($a in Get-Content -LiteralPath $file -Encoding utf8) { if ($a.Trim()) { [void]$owned.Add($a.Trim()) } } }
        }
    }
    if ($Staging) {
        foreach ($file in @(Get-ChildItem -LiteralPath $Staging -Filter 'src-g*.txt' -File -ErrorAction SilentlyContinue)) {
            foreach ($line in Get-Content -LiteralPath $file.FullName -Encoding utf8) {
                $p = $line -split "`t", 2
                if ($p.Count -eq 2 -and $p[1].Trim()) { $State.src[$p[0].Trim()] = $p[1].Trim() }
            }
        }
    }
    $picked = @($audit.cards.Values | Where-Object {
            $_.book -eq $Book -and $_.category -in $Writable -and (-not $owned -or $owned.Contains([string]$_.address))
        } | Sort-Object { [string]$_.address })
    $withFaults = @($picked | Where-Object { @(Get-HardFaults $audit ([string]$_.address)).Count })
    if ($MaxCards -gt 0) { $withFaults = @($withFaults | Select-Object -First $MaxCards) }
    $review = @()
    if ($Sample -gt 0 -and $Staging) {
        $clean = @($picked | Where-Object { $_.category -eq 'idea' -and -not @(Get-HardFaults $audit ([string]$_.address)).Count })
        $count = [Math]::Min($clean.Count, [int][Math]::Ceiling($Sample * $clean.Count))
        $rng = [Random]::new($Seed)
        $review = @($clean | Sort-Object { $rng.Next() } | Select-Object -First $count | Sort-Object { [string]$_.address })
    }
    $orphans = @()
    if (-not $NoOrphans) {
        $orphans = @($picked | Where-Object { $_.category -in @('idea', 'event') -and @($(if ($audit.faults.ContainsKey([string]$_.address)) { $audit.faults[[string]$_.address] } else { @() }) | Where-Object { $_.rule -in @('I5', 'E4') }).Count })
        if ($MaxCards -gt 0) { $orphans = @($orphans | Select-Object -First $MaxCards) }
    }
    foreach ($c in @($withFaults + $review + $orphans)) {
        $a = [string]$c.address
        $State.plan[$a] = [ordered]@{ title = [string]$c.title; body = [string]$c.body; category = [string]$c.category }
        $State.before[$a] = @(Get-HardFaults $audit $a | ForEach-Object { $_.rule })
    }
    $State.picked = @($picked | ForEach-Object { [string]$_.address })
    $title = { param($a) if ($a -and $audit.cards.ContainsKey($a)) { [string]$audit.cards[$a].title } else { '' } }
    $jobs = @()
    $fixCards = @(@($withFaults | ForEach-Object { @{ card = $_; review = $false } }) + @($review | ForEach-Object { @{ card = $_; review = $true } }))
    for ($i = 0; $i -lt $fixCards.Count; $i += $PerJob) {
        $chunk = @($fixCards[$i..([Math]::Min($i + $PerJob - 1, $fixCards.Count - 1))])
        $State.seq++
        $id = 'fix-{0:d2}' -f $State.seq
        $out = Join-Path $OutDir "$id.json"
        $sb = [Text.StringBuilder]::new()
        [void]$sb.AppendLine("# 任务 $id").AppendLine()
        [void]$sb.AppendLine("把结果写入 ``$out``。本任务共 $($chunk.Count) 张卡。").AppendLine()
        Add-SelfCheck $sb $id
        foreach ($x in $chunk) {
            $a = [string]$x.card.address
            $source = if ($x.review -or @(Get-HardFaults $audit $a | Where-Object { $_.rule -in @('I3', 'E1', 'E2') }).Count) { Get-SourceHint $x.card } else { '' }
            [void]$sb.Append((Format-Card $x.card (Get-HardFaults $audit $a) (& $title ([string]$x.card.parent)) $source))
        }
        $file = Join-Path $Work "$id.md"
        Save-Text $file $sb.ToString()
        $State.jobs[$id] = [ordered]@{ kind = 'fix'; file = $file; out = $out; cards = @($chunk | ForEach-Object { [string]$_.card.address }); applied = $false }
        $jobs += [ordered]@{ id = $id; files = @($FixDoc, $file); prompt = '按以上说明处理任务文件中的每张卡，写出 JSON 文件。最后只回复一行 JSON。' }
    }
    $jobs += @(New-OrphanJobs $audit $orphans)
    # The instruction files name the book.
    foreach ($doc in @($FixDoc, $OrphanDoc)) {
        $text = (Get-Content -LiteralPath $doc -Raw -Encoding utf8).Replace('{{KEY}}', $Book)
        Save-Text (Join-Path $Work (Split-Path -Leaf $doc)) $text
    }
    foreach ($j in $jobs) { $j.files[0] = Join-Path $Work (Split-Path -Leaf $j.files[0]) }
    Save-State
    $planOut = [ordered]@{
            book = $Book; picked = $picked.Count; with_faults = $withFaults.Count; review = $review.Count; orphans = $orphans.Count
            faults = [int](@($withFaults | ForEach-Object { @(Get-HardFaults $audit ([string]$_.address)).Count }) | Measure-Object -Sum).Sum
            jobs = $jobs
        }
    if ($Compact) { $c = Get-CompactJobs $jobs; foreach ($k in @('dir', 'lead', 'prompts', 'jobs')) { $planOut[$k] = $c[$k] } }
    Out-Last $planOut
}

# --- apply ---------------------------------------------------------------------------------------
function Step-Apply {
    if (-not $State) { throw 'run plan first' }
    $pending = @($State.jobs.Keys | Where-Object { -not $State.jobs[$_].applied -and (Test-Path -LiteralPath $State.jobs[$_].out) })
    $missing = @($State.jobs.Keys | Where-Object { -not $State.jobs[$_].applied -and -not (Test-Path -LiteralPath $State.jobs[$_].out) })
    # A write zt refused (a conflict, a lost connection) is tried again: the job's JSON still holds it.
    $retry = @{}
    foreach ($a in @($State.refused.Keys)) {
        $r = $State.refused[$a]
        if ([string]$r.reason -like 'zt:*' -and $a -notmatch '<-' -and $State.jobs.Contains([string]$r.job) -and $State.jobs[[string]$r.job].kind -eq 'fix') {
            if (-not $retry.ContainsKey([string]$r.job)) { $retry[[string]$r.job] = [Collections.Generic.HashSet[string]]::new() }
            [void]$retry[[string]$r.job].Add($a)
        }
    }
    $toApply = @(@($pending) + @($retry.Keys | Where-Object { $_ -notin $pending }))
    # -MaxJobs: a share of the jobs per call, so one call stays inside a caller's time limit; the
    # caller repeats apply until pending_jobs is 0.
    $later = @()
    if ($MaxJobs -gt 0 -and $toApply.Count -gt $MaxJobs) { $later = @($toApply[$MaxJobs..($toApply.Count - 1)]); $toApply = @($toApply[0..($MaxJobs - 1)]) }
    $addresses =@($toApply | ForEach-Object { @($State.jobs[$_].cards) + @($(if ($State.jobs[$_].candidates) { $State.jobs[$_].candidates.Values | ForEach-Object { $_ } })) } | Select-Object -Unique)
    $current = @{}
    if ($addresses.Count) {
        $read = @(Invoke-Zt @($addresses | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } }))
        for ($i = 0; $i -lt $addresses.Count; $i++) { $current[$addresses[$i]] = $read[$i] }
    }
    $counts = [ordered]@{ fix = 0; split = 0; unfaithful = 0; ok = 0; merge = 0; refused = 0; retried = 0; orphan_links = 0; orphan_none = 0; unread = 0 }
    foreach ($id in $toApply) {
        $job = $State.jobs[$id]
        $only = if ($id -in $pending) { $null } else { $retry[$id] }
        try { $result = Read-Json $job.out } catch { $State.refused["job:$id"] = [ordered]@{ job = $id; reason = "unreadable JSON: $($_.Exception.Message)" }; $counts.unread++; continue }
        if ($job.kind -eq 'fix') {
            $edits = @()
            $entries = @{}
            $lateSplits = @()
            foreach ($entry in @($result.cards)) {
                $a = [string]$entry.address
                if ($a -notin @($job.cards)) { continue }
                if ($only -and -not $only.Contains($a)) { continue }
                if ($only) { $State.refused.Remove($a); $counts.retried++ }
                $verdict = [string]$entry.verdict
                $old = $State.plan[$a]
                $refuse = {
                    param($reason)
                    $State.refused[$a] = [ordered]@{ job = $id; reason = $reason }
                    $counts.refused++
                }
                if ($verdict -eq 'ok') { $counts.ok++; $State.applied[$a] = [ordered]@{ job = $id; verdict = 'ok'; note = [string]$entry.note }; continue }
                if ($verdict -eq 'merge') {
                    $counts.merge++
                    $State.merges += [ordered]@{ address = $a; merge_with = [string]$entry.merge_with; note = [string]$entry.note }
                    $State.applied[$a] = [ordered]@{ job = $id; verdict = 'merge' }
                    continue
                }
                if ($verdict -notin @('fix', 'split', 'unfaithful')) { & $refuse "unknown verdict ``$verdict``"; continue }
                if ($old.category -notin $Writable) { & $refuse "category $($old.category) is not writable"; continue }
                # The title and the body decide a change. The stored text also holds the generated
                # Reverse links, which every write to a linking Card changes.
                $now = $current[$a]
                # Already written (a run stopped part way): record it. A split Card that run did not
                # make yet is made now; one it made (a Side successor with that title) is not made again.
                if ($now -and [string]$now.title -eq ([string]$entry.title).Trim() -and ([string]$now.body).TrimEnd() -eq ([string]$entry.body).TrimEnd()) {
                    $State.applied[$a] = [ordered]@{ job = $id; verdict = $verdict; note = [string]$entry.note }
                    if ($verdict -eq 'split' -and -not @($State.splits | Where-Object { $_.from -eq $a -and $_.job -eq $id }).Count) {
                        $sideTitles = @($now.sides | ForEach-Object { ([string]$_.title).Trim() })
                        foreach ($p in @($entry.split)) { if (([string]$p.title).Trim() -notin $sideTitles) { $lateSplits += @{ at = $a; part = $p } } }
                    }
                    continue
                }
                if (-not $now -or [string]$now.title -ne $old.title -or ([string]$now.body).TrimEnd() -ne ([string]$old.body).TrimEnd()) { & $refuse 'the Card changed since the plan'; continue }
                $why = Test-Rewrite $old $entry
                if ($why) { & $refuse $why; continue }
                $edits += @{ tool = 'edit_card'; arguments = @{ at = $a; title = ([string]$entry.title).Trim(); body = ([string]$entry.body).TrimEnd() } }
                $entries[$a] = $entry
            }
            # The edits first; a split Card only for an edit that is written, and only once.
            $done = Invoke-Writes $edits
            $creates = @()
            $from = @()
            for ($k = 0; $k -lt $edits.Count; $k++) {
                $a = [string]$edits[$k].arguments.at
                $entry = $entries[$a]
                $r = $done[$k]
                if (-not $r.ok) { $State.refused[$a] = [ordered]@{ job = $id; reason = "zt: $($r.error)" }; $counts.refused++; continue }
                $State.applied[$a] = [ordered]@{ job = $id; verdict = [string]$entry.verdict; note = [string]$entry.note }
                if ($entry.verdict -eq 'unfaithful') { $counts.unfaithful++ } else { $counts.fix++ }
                if ($entry.verdict -eq 'split' -and -not @($State.splits | Where-Object { $_.from -eq $a -and $_.job -eq $id }).Count) {
                    foreach ($p in @($entry.split)) {
                        $creates += @{ tool = 'create_successor'; arguments = @{ at = $a; kind = 'side'; title = ([string]$p.title).Trim(); body = ([string]$p.body).TrimEnd() } }
                        $from += $a
                    }
                }
            }
            foreach ($s in $lateSplits) {
                $creates += @{ tool = 'create_successor'; arguments = @{ at = $s.at; kind = 'side'; title = ([string]$s.part.title).Trim(); body = ([string]$s.part.body).TrimEnd() } }
                $from += $s.at
            }
            $made = Invoke-Writes $creates
            for ($k = 0; $k -lt $creates.Count; $k++) {
                if ($made[$k].ok) { $State.splits += [ordered]@{ from = $from[$k]; address = $made[$k].address; job = $id }; $counts.split++; Add-Owned $from[$k] $made[$k].address }
                else { $State.refused["split:$($from[$k]):$k"] = [ordered]@{ job = $id; reason = "zt: $($made[$k].error)" }; $counts.refused++ }
            }
        } else {
            $calls = @()
            $what = @()
            foreach ($l in @($result.links)) {
                $to = [string]$l.to
                $from = [string]$l.from
                $text = ([string]$l.text).Trim()
                if ($to -notin @($job.cards)) { continue }
                $reason = $null
                if ($from -notin @($job.candidates[$to])) { $reason = "``$from`` is not a candidate" }
                elseif (-not $text.Contains("[[$to]]")) { $reason = 'the text does not link the orphan' }
                elseif (-not (Test-Kinded $text $to)) { $reason = 'no kind word before the Link' }
                elseif ($text -match '[；;]') { $reason = 'a semicolon' }
                elseif (@([regex]::Matches($text, '\[\[([^\]]+)\]\]') | Where-Object { -not (Get-Known).Contains($_.Groups[1].Value) -and $_.Groups[1].Value -ne $to }).Count) { $reason = 'a Link to a Card that does not exist' }
                if ($reason) { $State.refused["$to<-$from"] = [ordered]@{ job = $id; reason = $reason }; $counts.refused++; continue }
                # Already appended by a run that stopped part way: record it, do not append it twice.
                if ($current[$from] -and ([string]$current[$from].body).Contains($text)) { $State.orphan_links += [ordered]@{ to = $to; from = $from }; $counts.orphan_links++; continue }
                $calls += @{ tool = 'append_card'; arguments = @{ at = $from; text = $text } }
                $what += @{ to = $to; from = $from }
            }
            foreach ($n in @($result.none)) { $State.orphan_none += [ordered]@{ orphan = [string]$n.orphan; note = [string]$n.note }; $counts.orphan_none++ }
            $done = Invoke-Writes $calls
            for ($k = 0; $k -lt $what.Count; $k++) {
                $r = if ($k -lt $done.Count) { $done[$k] } else { @{ ok = $false; error = 'no result' } }
                if ($r.ok) { $State.orphan_links += [ordered]@{ to = $what[$k].to; from = $what[$k].from }; $counts.orphan_links++ }
                else { $State.refused["$($what[$k].to)<-$($what[$k].from)"] = [ordered]@{ job = $id; reason = "zt: $($r.error)" }; $counts.refused++ }
            }
        }
        $job.applied = $true
        # After every job: a run stopped part way loses at most the job it was writing.
        Save-State
    }
    Save-State
    Out-Last ([ordered]@{ applied_jobs = $toApply.Count; pending_jobs = $later.Count; retried_jobs = @($retry.Keys).Count; waiting_jobs = $missing; counts = $counts })
}

# --- check ---------------------------------------------------------------------------------------
function Step-Check {
    if (-not $State) { throw 'run plan first' }
    $audit = Invoke-Audit (Join-Path $Work 'after')
    $jobs = @()
    $total = 0
    foreach ($id in @($State.jobs.Keys)) {
        $job = $State.jobs[$id]
        if ($job.kind -ne 'fix' -or -not $job.applied -or $job.followed) { continue }
        $open = @()
        # The job's Cards and the Cards its splits made: a new Card is checked like the others.
        foreach ($a in @(@($job.cards) + @($State.splits | Where-Object { $_.job -eq $id } | ForEach-Object { [string]$_.address }))) {
            $left = @(Get-HardFaults $audit $a)
            $refused = $State.refused[$a]
            if (-not $left.Count -and -not $refused) { continue }
            if ($State.applied[$a] -and $State.applied[$a].verdict -in @('ok', 'merge') -and -not $refused) { continue }
            $open += @{ address = $a; faults = $left; refused = $refused }
        }
        if (-not $open.Count) { continue }
        $total += [int](@($open | ForEach-Object { @($_.faults).Count }) | Measure-Object -Sum).Sum
        $fid = "$id-r2"
        $out = Join-Path $OutDir "$fid.json"
        $sb = [Text.StringBuilder]::new()
        [void]$sb.AppendLine("# 任务 $fid（第二轮）").AppendLine()
        [void]$sb.AppendLine("检查脚本核对了你写入的卡片。下面的卡片仍有问题，或你的改写被拒绝。只修正这些卡片，把结果写入 ``$out``（格式同前）。").AppendLine()
        # The follow-up has its own id and file: its own self-check line, not the first job's.
        Add-SelfCheck $sb $fid
        foreach ($o in $open) {
            $card = $audit.cards[$o.address]
            if (-not $card) { continue }
            $State.plan[$o.address] = [ordered]@{ title = [string]$card.title; body = [string]$card.body; category = [string]$card.category }
            $faults = @($o.faults)
            if ($o.refused) { $faults += @{ rule = '拒绝'; text = "脚本拒绝了上一轮的改写：$($o.refused.reason)" } }
            [void]$sb.Append((Format-Card $card $faults '' ''))
        }
        $file = Join-Path $Work "$fid.md"
        Save-Text $file $sb.ToString()
        $job.followed = $true
        $State.jobs[$fid] = [ordered]@{ kind = 'fix'; file = $file; out = $out; cards = @($open | ForEach-Object { $_.address }); applied = $false; followed = $true }
        foreach ($o in $open) { if ($State.refused.Contains($o.address)) { $State.refused.Remove($o.address) } }
        $jobs += [ordered]@{ id = $fid; continue = $id; files = @($file); fallback = @((Join-Path $Work (Split-Path -Leaf $FixDoc)), $file); prompt = '按以上问题修正这些卡片，写出 JSON 文件。最后只回复一行 JSON。' }
    }
    Save-State
    # A Card that took an orphan Link got new text: check it too. Its faults go to a fresh fix job
    # (the orphan conversation answers in another format).
    $linked = @($State.orphan_links | Where-Object { -not $_.checked } | ForEach-Object { [string]$_.from } | Select-Object -Unique)
    foreach ($l in @($State.orphan_links)) { $l.checked = $true }
    $open = @($linked | Where-Object { $audit.cards.ContainsKey($_) -and @(Get-HardFaults $audit $_).Count })
    if ($open.Count) {
        $State.seq++
        $fid = 'fix-{0:d2}' -f $State.seq
        $out = Join-Path $OutDir "$fid.json"
        $sb = [Text.StringBuilder]::new()
        [void]$sb.AppendLine("# 任务 $fid").AppendLine().AppendLine("把结果写入 ``$out``。本任务共 $($open.Count) 张卡：它们刚追加了一句链接，检查脚本在其中找到问题。").AppendLine()
        Add-SelfCheck $sb $fid
        foreach ($a in $open) {
            $card = $audit.cards[$a]
            $State.plan[$a] = [ordered]@{ title = [string]$card.title; body = [string]$card.body; category = [string]$card.category }
            if (-not $State.before.Contains($a)) { $State.before[$a] = @(Get-HardFaults $audit $a | ForEach-Object { $_.rule }) }
            [void]$sb.Append((Format-Card $card (Get-HardFaults $audit $a) '' ''))
            $total += @(Get-HardFaults $audit $a).Count
        }
        $file = Join-Path $Work "$fid.md"
        Save-Text $file $sb.ToString()
        $State.jobs[$fid] = [ordered]@{ kind = 'fix'; file = $file; out = $out; cards = $open; applied = $false; followed = $true }
        $jobs += [ordered]@{ id = $fid; files = @((Join-Path $Work (Split-Path -Leaf $FixDoc)), $file); prompt = '按以上说明处理任务文件中的每张卡，写出 JSON 文件。最后只回复一行 JSON。' }
    }
    # Orphans the plan did not see: mostly Cards a split made. They get an orphan job of their own.
    $orphans = @()
    if (-not $State.no_orphans) {
        $handled = [Collections.Generic.HashSet[string]]::new()
        foreach ($j in @($State.jobs.Values | Where-Object { $_.kind -eq 'orphans' })) { foreach ($a in @($j.cards)) { [void]$handled.Add([string]$a) } }
        $scope = [Collections.Generic.HashSet[string]]::new()
        foreach ($a in @(@($State.picked) + @($State.splits | ForEach-Object { [string]$_.address }))) { [void]$scope.Add([string]$a) }
        $orphans = @($audit.cards.Values | Where-Object {
                $scope.Contains([string]$_.address) -and -not $handled.Contains([string]$_.address) -and $_.category -in @('idea', 'event') -and
                @($(if ($audit.faults.ContainsKey([string]$_.address)) { $audit.faults[[string]$_.address] } else { @() }) | Where-Object { $_.rule -in @('I5', 'E4') }).Count
            } | Sort-Object { [string]$_.address })
        foreach ($o in $orphans) { if (-not $State.before.Contains([string]$o.address)) { $State.before[[string]$o.address] = @() } }
        $jobs += @(New-OrphanJobs $audit $orphans)
    }
    Save-State
    $checkOut = [ordered]@{ jobs = $jobs; faults = $total; orphans = $orphans.Count }
    if ($Compact) { $c = Get-CompactJobs $jobs; foreach ($k in @('dir', 'lead', 'prompts', 'jobs')) { $checkOut[$k] = $c[$k] } }
    Out-Last $checkOut
}

# --- report --------------------------------------------------------------------------------------
function Step-Report {
    if (-not $State) { throw 'run plan first' }
    $audit = Invoke-Audit (Join-Path $Work 'after')
    $rules = Get-RuleList
    $before = [ordered]@{}
    $after = [ordered]@{}
    foreach ($a in @($State.before.Keys)) {
        foreach ($r in @($State.before[$a])) { $before[$r] = 1 + [int]$before[$r] }
        foreach ($f in @(Get-HardFaults $audit $a)) { $after[$f.rule] = 1 + [int]$after[$f.rule] }
    }
    $orphansAfter = @($State.before.Keys | Where-Object { @($(if ($audit.faults.ContainsKey($_)) { $audit.faults[$_] } else { @() }) | Where-Object { $_.rule -in @('I5', 'E4') }).Count })
    $verdicts = [ordered]@{}
    foreach ($v in @($State.applied.Values)) { $verdicts[$v.verdict] = 1 + [int]$verdicts[$v.verdict] }
    $report = [ordered]@{
        book = $State.book; groups = $State.groups; rules = $rules
        cards = @($State.before.Keys).Count; picked = @($State.picked).Count
        faults_before = [int](@($before.Values) | Measure-Object -Sum).Sum; faults_after = [int](@($after.Values) | Measure-Object -Sum).Sum
        by_rule_before = $before; by_rule_after = $after
        verdicts = $verdicts; splits = @($State.splits).Count; merges = @($State.merges).Count
        refused = @($State.refused.Keys | ForEach-Object { [ordered]@{ address = $_; job = $State.refused[$_].job; reason = $State.refused[$_].reason } })
        orphan_links = @($State.orphan_links).Count; orphan_none = @($State.orphan_none).Count; orphans_after = $orphansAfter.Count
        job_count = @($State.jobs.Keys).Count
    }
    Save-Text (Join-Path $Work 'report.json') ($report | ConvertTo-Json -Depth 10)
    Save-Text (Join-Path $Work 'decisions.json') ([ordered]@{ merges = @($State.merges); orphan_none = @($State.orphan_none); splits = @($State.splits) } | ConvertTo-Json -Depth 10)
    $short = [ordered]@{}
    foreach ($k in $report.Keys) { if ($k -ne 'refused') { $short[$k] = $report[$k] } }
    $short.refused = @($report.refused).Count
    Out-Last $short
}

# --- precheck ------------------------------------------------------------------------------------
# Rules that need the whole Workspace (who links a Card, the other titles of its tree, a Link in
# two paragraphs is advisory): only the audit checks them.
$WholeWorkspace = @('I5', 'E4', 'I6', 'E5', 'L3')
# The audit's view of each Card (category, book, parent): the later run wins.
function Get-AuditCards {
    $cards = @{}
    foreach ($dir in @('before', 'after')) {
        $f = Join-Path $Work "$dir\audit-cards.json"
        if (Test-Path -LiteralPath $f) { foreach ($c in @(Read-Json $f)) { $cards[[string]$c.address] = $c } }
    }
    $cards
}
# A --json entry of zt-check, with the Links and the kinded Links as zt will read them.
function New-CheckEntry([string]$address, [string]$title, [string]$body, [string]$category, [string]$book, [string]$parent) {
    [ordered]@{
        address = $address; title = $title; body = $body; category = $category; book = $book; parent = $parent
        links = @([regex]::Matches($body, '\[\[([^\]]+)\]\]') | ForEach-Object { $_.Groups[1].Value.Trim() } | Select-Object -Unique)
        kinds = @(Get-LinkKinds $body)
    }
}
# Runs zt-check on the entries (and the term table entry for R6, when given); returns
# the faults of the entries as {address, rule, text}.
function Invoke-StyleCheck($entries, $term) {
    $want = @($entries | ForEach-Object { [string]$_.address })
    $all = @($entries) + @($(if ($term) { $term }))
    $tmp = Join-Path ([IO.Path]::GetTempPath()) "card-precheck-$PID-$([guid]::NewGuid().ToString('N')).json"
    try {
        [IO.File]::WriteAllText($tmp, (ConvertTo-Json -InputObject @($all) -Depth 5), $utf8)
        $splat = @{ Json = $tmp }
        if ($term) { $splat.Setting = [string]$term.address }
        foreach ($line in @(Invoke-ZtCheck @splat)) {
            $p = ([string]$line) -split ' ', 3
            if ($p.Count -eq 3 -and $p[0] -in $want -and $p[1] -match '^[RIEL]\d+$') { @{ address = $p[0]; rule = $p[1]; text = $p[2] } }
        }
    } finally { Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue }
}
# A fix job's draft: what apply refuses, then the rules of -Rules on each new text.
function Test-FixDraft($theJob, $result, $add) {
    if (-not $result.Contains('cards')) { & $add $Job 'json' 'the file holds no "cards" list'; return }
    $rules = @(Get-RuleList | Where-Object { $_ -notin $WholeWorkspace })
    $info = Get-AuditCards
    $entries = [Collections.Generic.List[object]]::new()
    $seen = @{}
    foreach ($entry in @($result.cards | Where-Object { $_ })) {
        $a = [string]$entry.address
        if ($a -notin @($theJob.cards)) { & $add $(if ($a) { $a } else { '-' }) 'refuse' "``$a`` is not a Card of job $Job"; continue }
        if ($seen.ContainsKey($a)) { & $add $a 'entry' 'a second entry for this Card: write one entry per Card'; continue }
        $seen[$a] = $true
        $verdict = [string]$entry.verdict
        if ($verdict -eq 'ok') { continue }
        if ($verdict -eq 'merge') { if (-not ([string]$entry.merge_with).Trim()) { & $add $a 'entry' 'merge names no merge_with' }; continue }
        if ($verdict -notin @('fix', 'split', 'unfaithful')) { & $add $a 'refuse' "unknown verdict ``$verdict``"; continue }
        $old = $State.plan[$a]
        if ($old.category -notin $Writable) { & $add $a 'refuse' "category $($old.category) is not writable"; continue }
        $why = Test-Rewrite $old $entry
        if ($why) { & $add $a 'refuse' $why }
        $card = $info[$a]
        $book = if ($card -and $card.book) { [string]$card.book } else { [string]$State.book }
        $parent = if ($card) { [string]$card.parent } else { '' }
        $title = ([string]$entry.title).Trim()
        $body = ([string]$entry.body).TrimEnd()
        if ($title -and $body.Trim()) { $entries.Add((New-CheckEntry $a $title $body ([string]$old.category) $book $parent)) }
        if ($verdict -eq 'split') {
            # A split Card is an Idea Card filed as a Side successor of this Card: its parent.
            $n = 0
            foreach ($p in @($entry.split | Where-Object { $_ })) {
                $n++
                $pt = ([string]$p.title).Trim()
                $pb = ([string]$p.body).TrimEnd()
                if ($pt -and $pb.Trim()) { $entries.Add((New-CheckEntry "$a+split$n" $pt $pb 'idea' $book $a)) }
            }
        }
    }
    foreach ($a in @($theJob.cards)) { if (-not $seen.ContainsKey([string]$a)) { & $add $a 'entry' 'no entry for this Card: write one entry per Card' } }
    if (-not $entries.Count) { return }
    $term = $null
    if ('R6' -in $rules) {
        $t = @($info.Values | Where-Object { [string]$_.tree -eq [string]$State.book -and [string]$_.title -match '^术语表' } | Select-Object -First 1)
        if ($t.Count) { $term = [ordered]@{ address = [string]$t[0].address; title = [string]$t[0].title; body = [string]$t[0].body } }
    }
    foreach ($f in @(Invoke-StyleCheck $entries $term)) { if ($f.rule -in $rules) { & $add $f.address $f.rule $f.text } }
}
# An orphan job's draft: what apply refuses, then L2 (and the R rules and L1 of -Rules) on each
# sentence. A semicolon is refused, so R3 is not named twice.
function Test-OrphanDraft($theJob, $result, $add) {
    $rules = @(@(Get-RuleList | Where-Object { ($_ -match '^R\d+$' -and $_ -ne 'R3') -or $_ -eq 'L1' }) + @('L2'))
    $entries = [Collections.Generic.List[object]]::new()
    $done = @{}
    foreach ($l in @($result.links | Where-Object { $_ })) {
        $to = [string]$l.to
        $from = [string]$l.from
        $text = ([string]$l.text).Trim()
        $at = "$to<-$from"
        if ($to -notin @($theJob.cards)) { & $add $at 'refuse' "``$to`` is not an orphan of job $Job"; continue }
        $done[$to] = $true
        if ($from -notin @($theJob.candidates[$to])) { & $add $at 'refuse' "``$from`` is not a candidate: choose one of $(@($theJob.candidates[$to]) -join ', ')" }
        if (-not $text.Contains("[[$to]]")) { & $add $at 'refuse' 'the text does not link the orphan' }
        elseif (-not (Test-Kinded $text $to)) { & $add $at 'refuse' 'no kind word before the Link' }
        if ($text -match '[；;]') { & $add $at 'refuse' 'a semicolon' }
        $unknown = @([regex]::Matches($text, '\[\[([^\]]+)\]\]') | ForEach-Object { $_.Groups[1].Value } | Where-Object { -not (Get-Known).Contains($_) -and $_ -ne $to })
        if ($unknown.Count) { & $add $at 'refuse' "a Link to a Card that does not exist: $(@($unknown | ForEach-Object { "[[$_]]" }) -join ' ')" }
        # The sentence alone, as an anchor: no I2 on one sentence; I1 on the empty title is dropped.
        if ($text) { $entries.Add((New-CheckEntry $at '' $text 'anchor' ([string]$State.book) '')) }
    }
    foreach ($n in @($result.none | Where-Object { $_ })) {
        $o = [string]$n.orphan
        if ($o -notin @($theJob.cards)) { & $add $(if ($o) { $o } else { '-' }) 'entry' "``$o`` is not an orphan of job $Job" } else { $done[$o] = $true }
    }
    foreach ($a in @($theJob.cards)) { if (-not $done.ContainsKey([string]$a)) { & $add $a 'entry' 'no entry for this orphan: put it in "links" or in "none"' } }
    if (-not $entries.Count) { return }
    foreach ($f in @(Invoke-StyleCheck $entries $null)) { if ($f.rule -in $rules) { & $add $f.address $f.rule $f.text } }
}
function Step-Precheck {
    $faults = [Collections.Generic.List[string]]::new()
    $add = { param($address, $rule, $text) $faults.Add("$address $rule $(([string]$text -replace '\s+', ' ').Trim())") }
    $theJob = if ($State -and $Job -and $State.jobs.Contains($Job)) { $State.jobs[$Job] } else { $null }
    if (-not $theJob) { & $add $(if ($Job) { $Job } else { '-' }) 'job' "no job ``$Job`` in $StateFile" }
    else {
        $path = if ($File) { $File } else { [string]$theJob.out }
        $result = $null
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { & $add $Job 'json' "no JSON file at $path" }
        else {
            try {
                $result = Read-Json $path
                if ($null -eq $result) { & $add $Job 'json' "the file $path is empty" }
                elseif ($result -isnot [Collections.IDictionary]) { & $add $Job 'json' "the file $path holds no JSON object"; $result = $null }
            } catch { & $add $Job 'json' "unreadable JSON in ${path}: $($_.Exception.Message)" }
        }
        if ($result) { if ($theJob.kind -eq 'fix') { Test-FixDraft $theJob $result $add } else { Test-OrphanDraft $theJob $result $add } }
    }
    $faults | ForEach-Object { Write-Output $_ }
    Write-Output "faults: $($faults.Count)"
}

switch ($Step) {
    'plan' { Step-Plan }
    'apply' { Step-Apply }
    'check' { Step-Check }
    'report' { Step-Report }
    'precheck' { Step-Precheck }
}
