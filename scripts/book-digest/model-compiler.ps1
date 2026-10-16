#Requires -Version 7.5
<#
.SYNOPSIS
  The deterministic half of the book digest's model compiler. Sonnet plans each model (plan.json);
  DeepSeek writes briefs, Card bodies, charts, Derivation parts, and reviews; this script does
  everything between them without a model: it builds skeletons, writes job files, applies splits,
  and reports. No step reads or writes Card prose except to place skeletons and Links.

.DESCRIPTION
  One batch of models shares one work folder -Work (also the DeepSeek Harness cwd), with
  state.json and one sub-folder per model (<slug>: the address with / and | replaced by _).
  Every step prints compact JSON as its last line. A step that hands work to DeepSeek prints
  {"jobs":[{"id","files":[...],"prompt","continue"?}], ...}; "continue" names an earlier job
  whose conversation the job continues (its "files" then open the follow-up message).

    brief     -Models a,b,...   context pack per model (model-context.ps1) and one brief job each;
                                skips a model that is already built (unless -Rebuild or -Level
                                changes it).
    validate  -Plan <file>      checks one plan.json against the model; prints one fault per line
                                and `faults: N`. Changes nothing.
    apply                       for each model with a plan.json: validates it, builds the skeleton
                                (level, parts in build order, skeleton bodies with kinded Links,
                                the setting's symbol and term tables, the Model Card's 结构 list),
                                writes setting.md, and prints the fill jobs (round 1: Model Card,
                                setting, every part that is not a Derivation part, with charts).
    derive                      prints the Derivation jobs (round 2), with the filled premises
                                copied in.
    review [-Changed]           prints one review job per Derivation chain of the model's own parts
                                (with -Changed, only chains that the last split touched). A shared
                                part (`take`) is never reviewed, split, or fixed: it belongs to
                                its home model, which may be finished.
    split [-Need a=N;b=M]       applies review-*.json verdicts (split, fix) and composer requests
                                (`need` Cards for one part): inserts the new parts, re-points the
                                next member, updates the 结构 list, and prints follow-up jobs.
    report                      one report per model: parts, relations, charts, Derivation chains,
                                build faults, style faults, parts still `待写`, and how many
                                Derivation parts have both 思路 and 解读 (explained).

  Explain (docs/derivation-explanations-proposal.md): adds 思路, 说明 lines, and 解读 to finished
  Derivation parts. DeepSeek writes JSON; this script writes the Cards and never changes 起点, a
  step, or 结果. It never runs the brief, plan, fill, derive, or review of a finished model.
    explain -Models a,b,...     one job per chain (at most three parts) of each model's own
                                Derivation parts that lack 思路 or 解读; -MaxParts caps the parts.
    explain-check -Job <id>     the writer's own check (read-only): prints what explain-apply would
                                refuse in the job's result file, one line per part, `faults: N`
                                last. Faults the old body already has are listed as kept, not
                                counted. Each explain job file names it on its SELF-CHECK line.
    explain-apply               reads the jobs' JSON, inserts the sections, refuses a part whose
                                body changed since its job, whose core changed, or whose new body
                                has a fault that the old body does not have (a fault the old body
                                already has is kept and listed under `kept`); sends refused parts
                                back to the same conversation once, and prints those follow-up jobs.
    explain-report              per model: Derivation parts, explained, missing, refused.

  A Derivation part has five sections: 思路, 起点, 步骤, 结果, 解读. In a chain of two or more parts,
  the first line of 思路 is the part's position (`第 2 张，共 3 张：从 … 推到 …。`); the script
  writes it in apply and explain and rewrites it after every split.

  Structural changes (insert, move, delete) run one call at a time through the zt helper with
  -Moves, and every address in state.json follows each moved map.

  Mathematical units (zt-math-digest): -BriefDoc, -FillDoc, and -ReviewDoc replace the shared
  instruction files (the math workflow passes the shared file with its math supplement appended).
  A unit's first main-line part may be its premises (facet 前提), and a premise that is the Model
  Card of another unit is a cited premise, not a closure fault.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('brief', 'validate', 'apply', 'derive', 'review', 'split', 'report', 'explain', 'explain-check', 'explain-apply', 'explain-report')][string]$Step,
    [Parameter(Mandatory)][string]$Work,
    [string]$Staging = '',
    [string]$Models = '',
    [string]$Plan = '',
    [string]$Need = '',
    [switch]$Changed,
    [switch]$Rebuild,
    [string]$Level = '',
    [int]$MaxSentences = 12,
    [int]$MaxChars = 400,
    [int]$MaxSteps = 6,
    [int]$MaxExplainSentences = 16,
    [int]$MaxExplainChars = 600,
    [int]$MaxParts = 0,
    [string]$BriefDoc = '',
    [string]$FillDoc = '',
    [string]$ReviewDoc = '',
    # explain-check: the job and its result file (default: the job's own result file)
    [string]$Job = '',
    [string]$File = ''
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
$Helper = $script:ZtHelper
# The writers' own check: zt-check (tools/zt-check), on PATH.
$Check = 'zt-check'
$Docs = @{
    brief  = Join-Path $PSScriptRoot 'dsh-brief-instructions.zh.md'
    fill   = Join-Path $PSScriptRoot 'dsh-fill-instructions.zh.md'
    review = Join-Path $PSScriptRoot 'dsh-review-instructions.zh.md'
    explain = Join-Path $PSScriptRoot 'dsh-explain-instructions.zh.md'
}
if ($BriefDoc) { $Docs.brief = $BriefDoc }
if ($FillDoc) { $Docs.fill = $FillDoc }
if ($ReviewDoc) { $Docs.review = $ReviewDoc }
# A premise that states a mathematical unit (a Model Card with a unit form) is cited, not taken in.
$UnitForms = @('definition', 'axiom', 'theorem', 'proposition', 'lemma', 'corollary')
$UnitCache = @{}
function Test-Unit($address) {
    if (-not $UnitCache.ContainsKey($address)) {
        $card = try { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $address } }))[0] } catch { $null }
        $UnitCache[$address] = [bool]($card -and $card.type -eq 'model' -and -not $card.facet -and [string]$card.form -in $UnitForms)
    }
    $UnitCache[$address]
}
$Explain = "-MaxExplainSentences $MaxExplainSentences -MaxExplainChars $MaxExplainChars"
# Writers check their own new Cards, so a Derivation part must have its 思路 and 解读 (R15).
$Limits = "--max-sentences $MaxSentences --max-chars $MaxChars --max-steps $MaxSteps --max-explain-sentences $MaxExplainSentences --max-explain-chars $MaxExplainChars --require-explain"
$Words = @{ rests = '依赖'; substitutes = '代入'; illustrates = '例示'; contrasts = '对照'; relaxes = '放宽'; corresponds = '对应'; objects = '反驳'; refers = '见' }
New-Item -ItemType Directory -Force -Path $Work | Out-Null
$StateFile = Join-Path $Work 'state.json'

# --- state -----------------------------------------------------------------------------------------
function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String }
function Save-Text($path, $text) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
    [IO.File]::WriteAllText($path, $text, $utf8)
}
function Save-State { Save-Text $StateFile ($State | ConvertTo-Json -Depth 20) }
$State = if (Test-Path -LiteralPath $StateFile) { Read-Json $StateFile } else { [ordered]@{ order = @(); models = [ordered]@{}; jobs = [ordered]@{}; rereview = @(); reviews = @(); seq = 0 } }
function Get-Slug($address) { $address -replace '[\\/|]', '_' }
function Get-Dir($model) { Join-Path $Work (Get-Slug $model) }
function Out-Last($object) { $object | ConvertTo-Json -Depth 20 -Compress }

# Every address the state holds follows a moved map: part ids, takes, and job targets.
function Update-Moved($map) {
    if (-not $map -or -not $map.Count) { return }
    foreach ($m in $State.models.Values) {
        foreach ($id in @($m.ids.Keys)) { $a = [string]$m.ids[$id]; if ($a -and $map.Contains($a)) { $m.ids[$id] = $map[$a] } }
    }
    foreach ($job in $State.jobs.Values) {
        $job.addresses = @($job.addresses | ForEach-Object { if ($_ -and $map.Contains($_)) { $map[$_] } else { $_ } })
    }
}

# Writes through the helper. A batch of creates and edits runs in one helper call; an insert, move,
# or delete must be the only call, because it renumbers Cards.
function Invoke-Write($calls) {
    $moves = Join-Path $Work 'moves.tmp'
    Remove-Item -LiteralPath $moves -ErrorAction SilentlyContinue
    $lines = @(($calls | ConvertTo-Json -Depth 10 -AsArray) | pwsh -NoProfile -File $Helper - -Moves $moves)
    $failed = $LASTEXITCODE -ne 0
    if (Test-Path -LiteralPath $moves) {
        foreach ($line in Get-Content -LiteralPath $moves -Encoding utf8) { if ($line.Trim()) { Update-Moved ($line | ConvertFrom-Json -AsHashtable).moved } }
    }
    $addresses = @()
    foreach ($line in $lines) {
        if ($line -match '^\d+ ok (.+)$') { $addresses += $Matches[1].Trim() }
        elseif ($line -match '^\d+ error (.*)$') { throw "zt: $($Matches[1])" }
    }
    if ($failed) { throw "zt helper failed: $($lines -join ' | ')" }
    , $addresses
}
function Invoke-Edits($calls) { if (@($calls).Count) { $null = Invoke-Write @($calls) } }

# --- Locations -------------------------------------------------------------------------------------
function Get-DirectSlot($address) {
    if ($address -match '^(.*[/|])(\d+)$') { return "$($Matches[1])$([int]$Matches[2] + 1)" }
    "$address|1"
}
function Get-TreeAddresses($address) {
    $tree = ($address -split '/', 2)[0]
    $set = [Collections.Generic.HashSet[string]]::new()
    foreach ($card in @((Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $tree } })).cards)) { [void]$set.Add($card.address) }
    , $set
}
function Get-FreeSide($parent, $set) {
    foreach ($c in [char[]]'abcdefghijklmnopqrstuvwxyz') { if (-not $set.Contains("$parent|$c")) { return "$parent|$c" } }
    throw "no free side slot under $parent"
}
function Test-HasSuccessors($address, $set) {
    if ($set.Contains((Get-DirectSlot $address))) { return $true }
    foreach ($a in $set) { if ($a.StartsWith("$address|")) { return $true } }
    $false
}

# --- plans -----------------------------------------------------------------------------------------
function Get-Ref($part) { if ($part.existing) { 'existing' } elseif ($part.take) { 'take' } elseif ($part.move) { 'move' } else { 'new' } }
function Get-Line($part) { if ($part.take) { 'shared' } elseif ($part.line) { $part.line } else { 'main' } }
function Test-Derive($part) { [bool]$part.derive }

# A reference is a plan id, `<other model>#<id>` (a model planned earlier in this batch), or an address.
function Resolve-Ref($ref, $model) {
    $ref = [string]$ref
    if ($ref -match '^(.+)#(.+)$') {
        $other = $State.models[$Matches[1]]
        if ($other -and $other.ids.Contains($Matches[2])) { return $other.ids[$Matches[2]] }
        throw "reference ${ref}: model $($Matches[1]) has no part $($Matches[2]) yet"
    }
    $m = $State.models[$model]
    if ($m.ids.Contains($ref)) {
        $value = [string]$m.ids[$ref]
        # a take of a part that another model of the batch planned
        if ($value -match '#') { return Resolve-Ref $value $model }
        return $value
    }
    $ref
}

function Test-Plan($plan, $structure, $model) {
    $faults = [Collections.Generic.List[string]]::new()
    $add = { param($text) $faults.Add($text) }
    if ($plan.model -ne $model) { & $add "plan.model is '$($plan.model)', expected '$model'" }
    if ($plan.status -eq 'skip') { return , $faults }
    if ($plan.level -notin 'concept', 'formal') { & $add 'level must be concept or formal' }
    # The Model Card job and a concept model's setting.md carry nothing else about the model.
    if (-not "$($plan.card)".Trim()) { & $add 'card is missing: what the Model Card must say (the question, the summary, the outside links to keep)' }
    $parts = @($plan.parts | Where-Object { $null -ne $_ })
    $ids = @{}
    $index = 0
    $memberAddresses = [Collections.Generic.HashSet[string]]::new()
    foreach ($p in @($structure.parts | Where-Object { $null -ne $_ })) { [void]$memberAddresses.Add($p.address) }
    foreach ($p in $parts) {
        $index++
        if (-not $p.id) { & $add "part $index has no id"; continue }
        if ($ids.ContainsKey($p.id)) { & $add "id $($p.id) is used twice" }
        $ids[$p.id] = $index
        foreach ($label in @($p.figure | Where-Object { $_ })) {
            if (-not (Get-FigureLine $label)) { & $add "part $($p.id): figure '$label' is not a book figure ZET holds (figures.json lists them); draw it with a chart instead" }
        }
        $kinds = @(@('existing', 'take', 'move') | Where-Object { $p[$_] })
        if ($kinds.Count -gt 1) { & $add "$($p.id): give only one of existing, take, move" }
        $line = Get-Line $p
        if ($line -notin 'main', 'side', 'off', 'shared') { & $add "$($p.id): line must be main, side, or off" }
        if ($line -eq 'side' -and -not $p.of) { & $add "$($p.id): a side part needs of (the part it belongs beside)" }
        if ((Get-Ref $p) -eq 'new') {
            if (-not $p.facet) { & $add "$($p.id): a new part needs a facet" }
            elseif ($p.facet.Length -gt 32) { & $add "$($p.id): facet is longer than 32 characters" }
            if (-not $p.title) { & $add "$($p.id): a new part needs a title" }
            if (-not $p.spec -and -not (Test-Derive $p)) { & $add "$($p.id): a new part needs spec (what it must state)" }
        }
        if (Test-Derive $p) {
            if ($plan.level -eq 'concept') { & $add "$($p.id): a concept model has no Derivation parts" }
            if ($line -ne 'main') { & $add "$($p.id): a Derivation part stands on the main line" }
            if ($p.facet -and -not $p.facet.StartsWith('推导')) { & $add "$($p.id): a Derivation part's facet starts with 推导" }
            if (-not $p.to) { & $add "$($p.id): a Derivation part needs to (its result equation)" }
            if ($p.steps -and $p.steps -gt $MaxSteps) { & $add "$($p.id): $($p.steps) steps is over MaxSteps $MaxSteps; make it a chain of Derivation parts" }
        }
        if ($p.existing -and -not $memberAddresses.Contains($p.existing)) { & $add "$($p.id): existing $($p.existing) is not a part of this model" }
    }
    $main = @($parts | Where-Object { (Get-Line $_) -eq 'main' })
    for ($k = 0; $k -lt $main.Count; $k++) {
        $p = $main[$k]
        if ((Test-Derive $p) -and -not $p.from -and -not @($p.rests | Where-Object { $null -ne $_ }).Count -and -not ($k -gt 0 -and (Test-Derive $main[$k - 1]))) {
            & $add "$($p.id): the first Derivation part of a chain needs from or rests (its starting point)"
        }
    }
    if ($plan.level -eq 'formal') {
        if (-not $main.Count) { & $add 'a formal model needs a main line' }
        elseif ($main[0].facet -notmatch '设定|前提|premises' -and -not $main[0].existing) { & $add 'the first main-line part is the setting (facet 设定; a mathematical unit: 前提)' }
        if (-not @($plan.symbols | Where-Object { $null -ne $_ }).Count) { & $add 'a formal model needs symbols (the setting''s symbol table)' }
        if (-not @($plan.terms | Where-Object { $null -ne $_ }).Count) { & $add 'a formal model needs terms (the setting''s term table)' }
    }
    if ($plan.level -eq 'concept' -and @($parts | Where-Object { -not $_.take }).Count -gt 4) { & $add 'a concept model has at most four parts' }
    # premises: earlier in the build order, and members
    $position = @{}
    for ($k = 0; $k -lt $main.Count; $k++) { $position[$main[$k].id] = $k }
    foreach ($p in $parts) {
        if ($p.of -and -not $ids.ContainsKey([string]$p.of) -and -not $memberAddresses.Contains([string]$p.of)) { & $add "$($p.id): of '$($p.of)' is not a part of this plan" }
        foreach ($r in @($p.rests | Where-Object { $null -ne $_ })) {
            $ref = [string]@($r)[0]
            if (-not $ref) { continue }
            if ($ids.ContainsKey($ref)) {
                if ($position.ContainsKey($p.id) -and $position.ContainsKey($ref) -and $position[$ref] -ge $position[$p.id]) { & $add "forward: $($p.id) rests on $ref, which comes later in the build order" }
            } elseif ($ref -match '#') {
                $other = ($ref -split '#')[0]
                if ($other -notin @($State.order | Where-Object { $null -ne $_ }) -or [array]::IndexOf(@($State.order | Where-Object { $null -ne $_ }), $other) -ge [array]::IndexOf(@($State.order | Where-Object { $null -ne $_ }), $model)) { & $add "$($p.id): $ref must name a model planned earlier in this batch" }
            } elseif (-not $memberAddresses.Contains($ref) -and $ref -ne $model -and -not (Test-Unit $ref)) {
                & $add "closure: $($p.id) rests on $ref, which is outside the model; take it in, move it in, or restate it as a new part"
            }
        }
    }
    # every filed part is accounted for, and kept main-line parts keep their order
    $listed = [Collections.Generic.HashSet[string]]::new()
    foreach ($p in $parts) { if ($p.existing) { [void]$listed.Add($p.existing) } }
    foreach ($m in @($plan.merge | Where-Object { $null -ne $_ })) { if ($m.from) { [void]$listed.Add($m.from) } }
    foreach ($p in @($structure.parts | Where-Object { $_.filed -ne $false })) {
        if (-not $listed.Contains($p.address)) { & $add "existing part $($p.address) ($($p.facet)) is not in the plan: list it with existing, or merge it" }
    }
    $currentMain = @($structure.build.order | Where-Object { $_.line -eq 'main' } | ForEach-Object { $_.address })
    $keptMain = @($main | Where-Object { $_.existing } | ForEach-Object { $_.existing })
    $inOrder = @($currentMain | Where-Object { $_ -in $keptMain })
    if (($inOrder -join ',') -ne ($keptMain -join ',')) { & $add "kept main-line parts must stay in their order: $($inOrder -join ', ')" }
    if (-not $currentMain.Count -and @($structure.parts | Where-Object { $null -ne $_ }).Count -and @($main | Where-Object { -not $_.existing }).Count) {
        & $add 'the model has parts but no main line: list the part that should become the setting as existing, line main'
    }
    foreach ($m in @($plan.merge | Where-Object { $null -ne $_ })) {
        if (-not $ids.ContainsKey([string]$m.into) -and -not $memberAddresses.Contains([string]$m.into)) { & $add "merge into '$($m.into)' is not a part" }
    }
    , $faults
}

# --- skeleton bodies -------------------------------------------------------------------------------
function Format-Link($r, $model, [string]$default = 'rests') {
    $items = @($r)
    $target = Resolve-Ref $items[0] $model
    $why = if ($items.Count -gt 1) { [string]$items[1] } else { '' }
    $kind = if ($items.Count -gt 2 -and $items[2]) { [string]$items[2] } else { $default }
    $word = if ($Words.ContainsKey($kind)) { $Words[$kind] } else { $kind }
    "$word [[$target]]$(if ($why) { "：$($why.TrimEnd('。'))" })。"
}
function Get-Tables($plan) {
    $t = [Text.StringBuilder]::new()
    [void]$t.AppendLine('符号表').AppendLine().AppendLine('| 符号 | 含义 | 首次出现 |').AppendLine('| --- | --- | --- |')
    foreach ($s in @($plan.symbols | Where-Object { $null -ne $_ })) { $s = @($s); [void]$t.AppendLine("| $($s[0]) | $($s[1]) | $(if ($s.Count -gt 2 -and $s[2]) { $s[2] } else { '本卡' }) |") }
    [void]$t.AppendLine().AppendLine('术语表').AppendLine().AppendLine('| 术语 | 不用 |').AppendLine('| --- | --- |')
    foreach ($s in @($plan.terms | Where-Object { $null -ne $_ })) { $s = @($s); [void]$t.AppendLine("| $($s[0]) | $(@($s | Select-Object -Skip 1) -join '、') |") }
    $t.ToString().TrimEnd()
}
# --- Derivation explanations: chain position lines -------------------------------------------------
# The formula a 结果 states: its last display formula, else its first inline one, else its first
# sentence (cut to 30 characters).
function Get-ResultFormula([string]$result) {
    $display = @([regex]::Matches($result, '\$\$[\s\S]+?\$\$'))
    if ($display.Count) { return $display[$display.Count - 1].Value }
    $inline = [regex]::Match($result, '\$[^$\n]+\$')
    if ($inline.Success) { return $inline.Value }
    $first = (($result -split '[。！？\n]')[0]).Trim()
    if ($first.Length -gt 30) { $first = $first.Substring(0, 30) + '…' }
    $first
}
# A display formula ($$…$$) as an inline one, for a sentence.
function Get-Inline([string]$formula) {
    $f = $formula.Trim()
    if ($f -match '^\$\$([\s\S]*)\$\$$') { $f = '$' + $Matches[1].Trim() + '$' }
    $f
}
# The first line of a chain member's 思路: its place in the chain and what it derives. $tos holds
# each member's result in chain order; a chain of one part gets no line.
function Format-ChainLine([int]$k, [int]$n, $tos) {
    if ($n -lt 2) { return '' }
    # A 结果 may hold sentences besides its formula: the line names the formula only.
    $tos = @(@($tos) | ForEach-Object { Get-ResultFormula ([string]$_) })
    if ($k -eq 1) { return "第 1 张，共 $n 张。全链依次推出 $(@($tos | ForEach-Object { Get-Inline ([string]$_) }) -join '、')。" }
    "第 $k 张，共 $n 张：从 $(Get-Inline ([string]$tos[$k - 2])) 推到 $(Get-Inline ([string]$tos[$k - 1]))。"
}
# Plan id -> @{ k; n; tos }: the runs of neighbouring Derivation parts on the plan's main line.
function Get-PlanChains($plan) {
    $out = @{}
    $run = @()
    foreach ($p in @(@($plan.parts | Where-Object { $null -ne $_ -and (Get-Line $_) -eq 'main' }) + $null)) {
        if ($p -and (Test-Derive $p)) { $run += $p; continue }
        for ($i = 0; $i -lt $run.Count; $i++) { $out[[string]$run[$i].id] = @{ k = $i + 1; n = $run.Count; tos = @($run | ForEach-Object { [string]$_.to }) } }
        $run = @()
    }
    $out
}
# The sections of a body as an ordered list of @{ head; lines }; text before the first heading has
# head ''. A heading is a line `**思路**` (or 起点, 步骤, 结果, 解读).
function Split-Sections([string]$body) {
    $out = [Collections.Generic.List[object]]::new()
    $cur = @{ head = ''; lines = [Collections.Generic.List[string]]::new() }
    foreach ($line in (($body -replace "`r", '') -split "`n")) {
        if ($line -match '^\s*\*\*(思路|起点|步骤|结果|解读)\*\*\s*$') {
            $out.Add($cur)
            $cur = @{ head = $Matches[1]; lines = [Collections.Generic.List[string]]::new() }
            continue
        }
        $cur.lines.Add($line)
    }
    $out.Add($cur)
    $out
}
function Join-Sections($sections) {
    $parts = foreach ($s in $sections) {
        $text = ($s.lines -join "`n").Trim()
        if ($s.head) { "**$($s.head)**$(if ($text) { "`n$text" })" } elseif ($text) { $text }
    }
    ($parts -join "`n`n").Trim()
}
# Writes the position line at the top of 思路 (replaces an old one, or removes it for a chain of
# one). A body with no 思路 is left as it is.
function Set-ChainLine([string]$body, [string]$line) {
    $sections = Split-Sections $body
    $idea = $sections | Where-Object { $_.head -eq '思路' } | Select-Object -First 1
    if (-not $idea) { return $body }
    $rest = @($idea.lines | Where-Object { $_.Trim() })
    if ($rest.Count -and $rest[0].Trim() -match '^第 \d+ 张，共 \d+ 张') { $rest = @($rest | Select-Object -Skip 1) }
    $idea.lines = [Collections.Generic.List[string]]::new([string[]]@(@($line | Where-Object { $_ }) + $rest))
    Join-Sections $sections
}
# The result of a Derivation part: the 结果 section only, without a 来源 line.
function Get-Result($body) {
    $r = (Split-Sections ([string]$body)) | Where-Object { $_.head -eq '结果' } | Select-Object -First 1
    if ($r) { (@($r.lines | Where-Object { $_.Trim() -notmatch '^来源\s*[：:]' }) -join "`n").Trim() } else { '' }
}
# Rewrites the position lines of every chain of a model's own parts, from the Cards themselves.
function Update-ChainLines($model) {
    $s = Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $model } })
    $bodies = @{}
    foreach ($p in @($s.parts | Where-Object { $null -ne $_ })) { $bodies[[string]$p.address] = [string]$p.body }
    $shared = @($s.build.order | Where-Object { $_.line -eq 'shared' } | ForEach-Object { [string]$_.address })
    $edits = @()
    foreach ($c in @($s.build.chains | Where-Object { $null -ne $_ })) {
        $parts = @($c.parts | Where-Object { $_ -notin $shared } | ForEach-Object { [string]$_ })
        $tos = @($parts | ForEach-Object { Get-Result $bodies[$_] })
        for ($k = 1; $k -le $parts.Count; $k++) {
            $old = $bodies[$parts[$k - 1]]
            $new = Set-ChainLine $old (Format-ChainLine $k $parts.Count $tos)
            if ($new -ne $old.Trim()) { $edits += @{ tool = 'edit_card'; arguments = @{ at = $parts[$k - 1]; body = $new } } }
        }
    }
    Invoke-Edits $edits
}
function Get-Skeleton($part, $plan, $model, $previous) {
    $links = @(@($part.rests | Where-Object { $null -ne $_ }) | Where-Object { @($_)[0] } | ForEach-Object { Format-Link $_ $model })
    if ($part.of) {
        $kind = if ($part.kind) { $part.kind } else { 'illustrates' }
        $links = @(Format-Link @($part.of, $part.why, $kind) $model) + $links
    }
    if (Test-Derive $part) {
        $start = @($links | ForEach-Object { "- $_" })
        if ($previous -and (Test-Derive $previous)) { $start = @("- 依赖上一步 [[$(Resolve-Ref $previous.id $model)]] 的结果：$($previous.to)。") + $start }
        if ($part.from) { $start += "- 从 $($part.from) 出发。" }
        $c = (Get-PlanChains $plan)[[string]$part.id]
        $position = if ($c) { Format-ChainLine $c.k $c.n $c.tos } else { '' }
        $idea = (@($position, '待写') | Where-Object { $_ }) -join "`n"
        return "**思路**`n$idea`n`n**起点**`n$($start -join "`n")`n`n**步骤**`n待写`n`n**结果**`n$($part.to)`n`n**解读**`n待写"
    }
    $body = (@($links) + @('', '待写')) -join "`n"
    if ($plan.level -eq 'formal' -and $part.facet -match '设定' -and (Get-Line $part) -eq 'main') { $body += "`n`n$(Get-Tables $plan)" }
    $body.Trim()
}
# The Model Card's 结构 list: one line per member in build order, shared parts last with their use.
function Get-StructureList($plan, $model) {
    $lines = foreach ($p in @($plan.parts | Where-Object { $null -ne $_ })) {
        $address = Resolve-Ref $p.id $model
        $m = $State.models[$model]
        if ($p.take) { "- $(if ($p.facet) { $p.facet } else { '共用' }) [[$address]]（共用）：$($p.use)" }
        else { "- $($m.facets[$p.id]) [[$address]]$(if ($p.emphasis -eq 'core') { '（核心）' })：$($m.titles[$p.id])" }
    }
    "**结构**`n`n$($lines -join "`n")"
}
function Set-Section($body, $heading, $content) {
    $lines = [Collections.Generic.List[string]]::new([string[]]@(($body -replace "`r", '') -split "`n"))
    $start = -1
    for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -match "^\s*(\*\*$heading\*\*|#+\s*$heading|$heading[：:])") { $start = $i; break } }
    if ($start -lt 0) {
        $at = $lines.Count
        for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -match '^\s*(\*\*(关联|来源)\*\*|#+\s*(关联|来源)|(关联|来源)[：:])') { $at = $i; break } }
        $lines.InsertRange($at, [string[]]@(($content -split "`n") + ''))
        return ($lines -join "`n").Trim()
    }
    $end = $lines.Count
    for ($i = $start + 1; $i -lt $lines.Count; $i++) { if ($lines[$i] -match '^\s*(\*\*[^*]+\*\*\s*$|#+\s|\S{2,6}[：:]\s*$)') { $end = $i; break } }
    $lines.RemoveRange($start, $end - $start)
    $lines.InsertRange($start, [string[]]@(($content -split "`n") + ''))
    ($lines -join "`n").Trim()
}

# --- job files -------------------------------------------------------------------------------------
function Get-SourceLines($refs) {
    $out = [Text.StringBuilder]::new()
    $budget = 12000
    foreach ($ref in @($refs)) {
        if ($ref -notmatch '^(.+?):(\d+)(?:-(\d+))?$') { continue }
        $file = $Matches[1]; $from = [int]$Matches[2]; $to = if ($Matches[3]) { [int]$Matches[3] } else { $from }
        $path = if ([IO.Path]::IsPathRooted($file)) { $file } else { Join-Path $Staging $file }
        if (-not (Test-Path -LiteralPath $path)) { [void]$out.AppendLine("（找不到 $path）"); continue }
        $text = (@(Get-Content -LiteralPath $path -Encoding utf8 | Select-Object -Skip ($from - 1) -First ($to - $from + 1)) -join "`n")
        if ($text.Length -gt $budget) { $text = $text.Substring(0, [Math]::Max(0, $budget)) + '……' }
        $budget -= $text.Length
        [void]$out.AppendLine("``$path`` 第 $from–$to 行：").AppendLine('```text').AppendLine($text).AppendLine('```')
        if ($budget -le 0) { break }
    }
    $out.ToString()
}
function New-Job($model, $round, $ids, $text, $prompt) {
    $State.seq++
    $id = "$(Get-Slug $model):$round$('{0:d2}' -f $State.seq)"
    $dir = Get-Dir $model
    $file = Join-Path $dir "job-$($round)$('{0:d2}' -f $State.seq).md"
    $allow = [IO.Path]::ChangeExtension($file, '.allow.txt')
    $addresses = @($ids | ForEach-Object { Resolve-Ref $_ $model })
    Save-Text $allow (($addresses -join "`n") + "`n")
    $head = "# 任务 $id`n`nHELPER: $Helper`nCHECK: $Check`nALLOW: $allow`nLIMITS: $Limits`n模型卡: [[$model]]（$(if ($State.models[$model].level -eq 'concept') { '概念模型' } else { '形式模型' })）`n`n"
    Save-Text $file ($head + $text)
    $State.jobs[$id] = [ordered]@{ model = $model; round = $round; ids = @($ids); addresses = $addresses; allow = $allow; file = $file }
    [ordered]@{ id = $id; files = @($Docs.fill, (Join-Path $dir 'setting.md'), $file); prompt = $prompt }
}
function Get-Premises($part, $model, [switch]$Bodies) {
    $lines = foreach ($r in @($part.rests | Where-Object { $null -ne $_ })) {
        $items = @($r)
        if (-not $items[0]) { continue }
        $address = Resolve-Ref $items[0] $model
        "- [[$address]]：$(if ($items.Count -gt 1) { $items[1] })"
    }
    if (-not $Bodies -or -not @($part.rests | Where-Object { $null -ne $_ }).Count) { return ($lines -join "`n") }
    $addresses = @(@($part.rests | Where-Object { $null -ne $_ }) | ForEach-Object { Resolve-Ref @($_)[0] $model })
    $cards = @(Invoke-Zt @($addresses | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } }))
    $text = ($lines -join "`n") + "`n`n前提卡的正文（已写好，供你引用其中的式子）：`n"
    foreach ($c in $cards) {
        $b = [string]$c.body
        if ($b.Length -gt 1500) { $b = $b.Substring(0, 1500) + '……' }
        $text += "`n#### [[$($c.address)]] $($c.title)`n$b`n"
    }
    $text
}
# The Markdown of a book figure by its label (`2.F.1`, `IV`), from the figures.json that
# prepare-book-digest.ps1 writes; $null when the staging folder has no such figure.
$script:FigureLines = $null
function Get-FigureLine([string]$label) {
    if ($null -eq $script:FigureLines) {
        $script:FigureLines = @{}
        # Without -Staging (the architect's validate), the nearest folder above -Work that holds figures.json:
        # the workflow's work folder is <staging>\models\<batch>.
        $file = if ($Staging) { Join-Path $Staging 'figures.json' } else { '' }
        if (-not $Staging) {
            $dir = Split-Path -Parent $Work
            while ($dir -and -not (Test-Path -LiteralPath (Join-Path $dir 'figures.json'))) { $dir = Split-Path -Parent $dir }
            if ($dir) { $file = Join-Path $dir 'figures.json' }
        }
        if ($file -and (Test-Path -LiteralPath $file)) {
            foreach ($figure in (Get-Content -LiteralPath $file -Raw -Encoding utf8 | ConvertFrom-Json).figures) {
                foreach ($l in @($figure.labels)) { if (-not $script:FigureLines.ContainsKey($l)) { $script:FigureLines[$l] = $figure.line } }
            }
        }
    }
    $key = ($label -replace '^\s*(图|Figure|Fig\.)\s*', '').Trim()
    return $script:FigureLines[$key]
}
function Get-PartBlock($n, $part, $model, [switch]$Bodies) {
    $address = Resolve-Ref $part.id $model
    $b = [Text.StringBuilder]::new()
    [void]$b.AppendLine("## 分卡 $n：[[$address]]").AppendLine()
    [void]$b.AppendLine("- 分面：$($part.facet)$(if ($part.emphasis -eq 'core') { '（核心）' } elseif ($part.emphasis -eq 'aside') { '（旁注）' })")
    [void]$b.AppendLine("- 标题：$($part.title)")
    if ($part.spec) { [void]$b.AppendLine("- 要陈述的内容：$($part.spec)") }
    if (Test-Derive $part) {
        [void]$b.AppendLine("- 推导：从起点写到结果 $($part.to)$(if ($part.steps) { "，约 $($part.steps) 步" })。骨架里已写好起点和结果。")
        [void]$b.AppendLine('- 解释：写思路、步骤下的说明和解读（见“讲清推导”）。思路的位置行原样保留。')
        # A chain member starts from the result of the part before it, which may sit in another job.
        $main = @($State.models[$model].main | Where-Object { $null -ne $_ })
        $k = [array]::IndexOf($main, [string]$part.id)
        $before = if ($k -gt 0) { Get-PlanPart $model $main[$k - 1] } else { $null }
        if ($before -and (Test-Derive $before) -and $before.to) { [void]$b.AppendLine("- 起点：上一步 [[$(Resolve-Ref $before.id $model)]] 的结果 $($before.to)") }
        if ($part.from) { [void]$b.AppendLine("- 从 $($part.from) 出发。") }
    }
    $premises = Get-Premises $part $model -Bodies:$Bodies
    if ($premises) { [void]$b.AppendLine("- 前提：").AppendLine($premises) }
    if ($part.of) { [void]$b.AppendLine("- 它站在 [[$(Resolve-Ref $part.of $model)]] 旁边（$(if ($part.kind) { $part.kind } else { 'illustrates' })）。") }
    if ($part.chart) { [void]$b.AppendLine("- 图表：$($part.chart)") }
    foreach ($label in @($part.figure | Where-Object { $_ })) {
        $line = Get-FigureLine $label
        if ($line) { [void]$b.AppendLine("- 书中的原图（把这一行原样放进正文，单独成段）：``$line``") }
        else { [void]$b.AppendLine("- 书中的图 $label：ZET 没有它的原图，按原文用图表画出。") }
    }
    $was = if ($State.models[$model].old) { $State.models[$model].old[[string]$part.id] }
    if ($was) {
        if ($was.Length -gt 6000) { $was = $was.Substring(0, 6000) + '……' }
        [void]$b.AppendLine().AppendLine('### 本卡原来的正文（按新骨架和要陈述的内容改写，可用的式子、例子和图表留用）').AppendLine($was)
    }
    if ($part.old) { [void]$b.AppendLine().AppendLine('### 已有内容（并入本卡，按新骨架改写）').AppendLine($part.old) }
    $src = Get-SourceLines @($part.src | Where-Object { $null -ne $_ })
    if ($src) { [void]$b.AppendLine().AppendLine('### 原文').AppendLine($src) }
    $b.ToString()
}

# --- steps -----------------------------------------------------------------------------------------
function Step-Brief {
    $jobs = @(); $skipped = @()
    foreach ($model in @($Models -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
        $structure = Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $model } })
        $built = @($structure.parts | Where-Object { $null -ne $_ }).Count -and $structure.build.self_sufficient -and -not @($structure.build.beyond_level | Where-Object { $null -ne $_ }).Count
        if ($built -and -not $Rebuild -and (-not $Level -or $structure.level -eq $Level)) { $skipped += $model; continue }
        $dir = Get-Dir $model
        if ($model -notin @($State.order | Where-Object { $null -ne $_ })) { $State.order = @($State.order | Where-Object { $null -ne $_ }) + $model }
        $State.models[$model] = [ordered]@{ level = $structure.level; status = 'brief'; ids = [ordered]@{}; titles = [ordered]@{}; derive = @(); main = @(); split = 0 }
        $context = Join-Path $dir 'context.md'
        $null = pwsh -NoProfile -File (Join-Path $PSScriptRoot 'model-context.ps1') -Staging $Staging -Model $model -Out $context
        if ($LASTEXITCODE -ne 0) { throw "model-context.ps1 failed for $model" }
        $readOnly = Join-Path $dir 'read-only.allow.txt'
        Save-Text $readOnly ''
        $brief = Join-Path $dir 'brief.md'
        $job = Join-Path $dir 'brief-job.md'
        Save-Text $job "# 简报任务：[[$model]]`n`nHELPER: $Helper（只读：每条命令末尾加 -Allow `"$readOnly`"）`n上下文包: $context`n简报文件: $brief`n现在的层级: $(if ($structure.level) { $structure.level } else { '未定' })$(if ($Level) { "`n本次指定层级: $Level" })`n分卡数: $(@($structure.parts | Where-Object { $null -ne $_ }).Count)`n"
        $jobs += [ordered]@{ id = "$(Get-Slug $model):brief"; files = @($Docs.brief, $job); prompt = "按以上说明写出简报文件 $brief，最后只回复一行 JSON。" }
    }
    Save-State
    Out-Last ([ordered]@{ jobs = $jobs; skipped = $skipped })
}

function Step-Validate {
    $p = Read-Json $Plan
    $structure = Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $p.model } })
    if (-not $State.models.Contains($p.model)) { $State.order = @($State.order | Where-Object { $null -ne $_ }) + $p.model; $State.models[$p.model] = [ordered]@{ ids = [ordered]@{}; titles = [ordered]@{} } }
    $faults = Test-Plan $p $structure $p.model
    $faults | ForEach-Object { Write-Output $_ }
    Write-Output "faults: $($faults.Count)"
}

function Build-Model($model) {
    $m = $State.models[$model]
    $dir = Get-Dir $model
    $plan = Read-Json (Join-Path $dir 'plan.json')
    $structure = Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $model } })
    $faults = Test-Plan $plan $structure $model
    if ($faults.Count) { $m.status = 'failed'; $m.error = "plan faults: $($faults -join '; ')"; return @() }
    if ($plan.status -eq 'skip') { $m.status = 'skipped'; return @() }
    $m.level = $plan.level
    $parts = @($plan.parts | Where-Object { $null -ne $_ })
    $m.ids = [ordered]@{}
    $m.titles = [ordered]@{}
    $m.facets = [ordered]@{}
    $known = @{}
    foreach ($s in @($structure.parts | Where-Object { $null -ne $_ })) { $known[$s.address] = $s }
    foreach ($p in $parts) {
        $ref = Get-Ref $p
        if ($ref -ne 'new') { $m.ids[$p.id] = [string]$p[$ref] }
        $was = if ($p.existing) { $known[$p.existing] } else { $null }
        $m.titles[$p.id] = if ($p.title) { $p.title } elseif ($was) { $was.title } else { '' }
        $m.facets[$p.id] = if ($p.facet) { $p.facet } elseif ($was) { $was.facet } else { '' }
    }
    # Material of merged parts goes into the job of the part that takes it in.
    foreach ($mg in @($plan.merge | Where-Object { $null -ne $_ })) {
        $card = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $mg.from } })
        $into = $parts | Where-Object { $_.id -eq $mg.into } | Select-Object -First 1
        if ($into) { $into.old = (@($into.old, "[[$($mg.from)]] $($card.title)`n$($card.body)") | Where-Object { $_ }) -join "`n`n" }
    }
    $edit = @{ at = $model; level = $plan.level }
    if ($plan.title) { $edit.title = $plan.title }
    Invoke-Edits @(@{ tool = 'edit_card'; arguments = $edit })

    # Main line, in build order. A new part goes before the next kept part, or after the last one.
    $main = @($parts | Where-Object { (Get-Line $_) -eq 'main' })
    $previous = $null
    for ($k = 0; $k -lt $main.Count; $k++) {
        $p = $main[$k]
        $anchor = $main | Select-Object -Skip ($k + 1) | Where-Object { $_.existing } | Select-Object -First 1
        $anchorAddress = if ($anchor) { $m.ids[$anchor.id] } else { $null }
        $fields = @{ title = $p.title; facet = $p.facet; type = 'model'; body = '待写' }
        if ($p.emphasis) { $fields.emphasis = $p.emphasis } elseif (Test-Derive $p) { $fields.emphasis = 'aside' }
        if ($p.existing) { $previous = $p; continue }
        if ($p.move) {
            $target = if ($anchorAddress) { @{ before = $anchorAddress } }
                elseif ($previous) { @{ to = (Get-DirectSlot $m.ids[$previous.id]) } }
                else { @{ to = (Get-FreeSide $model (Get-TreeAddresses $model)) } }
            $set = Get-TreeAddresses $m.ids[$p.id]
            $alone = -not @($set | Where-Object { $_.StartsWith("$($m.ids[$p.id])|") }).Count
            $null = Invoke-Write @(@{ tool = 'move_card'; arguments = $target + @{ at = $m.ids[$p.id]; alone = $alone; confirm = 'move' } })
            $m.moved = @($m.moved | Where-Object { $null -ne $_ }) + "$($p.move) -> $($m.ids[$p.id])"
        } elseif ($anchorAddress) {
            $address = (Invoke-Write @(@{ tool = 'insert_card'; arguments = $fields + @{ before = $anchorAddress; confirm = 'insert' } }))[0]
            $m.ids[$p.id] = $address
        } elseif ($previous) {
            $m.ids[$p.id] = (Invoke-Write @(@{ tool = 'create_successor'; arguments = $fields + @{ at = $m.ids[$previous.id]; kind = 'direct' } }))[0]
        } else {
            $m.ids[$p.id] = (Invoke-Write @(@{ tool = 'create_successor'; arguments = $fields + @{ at = $model; kind = 'side' } }))[0]
        }
        $previous = $p
    }
    # Side parts beside their step, then off parts beside the Model Card.
    foreach ($p in @($parts | Where-Object { (Get-Line $_) -in 'side', 'off' })) {
        $parent = if ((Get-Line $p) -eq 'side') { Resolve-Ref $p.of $model } else { $model }
        if ($p.existing) { continue }
        if ($p.move) {
            $null = Invoke-Write @(@{ tool = 'move_card'; arguments = @{ at = $m.ids[$p.id]; to = (Get-FreeSide $parent (Get-TreeAddresses $model)); confirm = 'move' } })
            $m.moved = @($m.moved | Where-Object { $null -ne $_ }) + "$($p.move) -> $($m.ids[$p.id])"
            continue
        }
        $fields = @{ at = $parent; kind = 'side'; title = $p.title; facet = $p.facet; type = 'model'; body = '待写' }
        if ($p.emphasis) { $fields.emphasis = $p.emphasis }
        $m.ids[$p.id] = (Invoke-Write @(@{ tool = 'create_successor'; arguments = $fields }))[0]
    }
    # Merged parts: only a part without successors can go (a delete takes its successors with it).
    foreach ($mg in @($plan.merge | Where-Object { $null -ne $_ })) {
        $set = Get-TreeAddresses $model
        if (Test-HasSuccessors $mg.from $set) { $m.notes = @($m.notes | Where-Object { $null -ne $_ }) + "not merged: $($mg.from) has successors"; continue }
        $null = Invoke-Write @(@{ tool = 'delete_card'; arguments = @{ at = $mg.from; into = (Resolve-Ref $mg.into $model); confirm = 'delete' } })
    }
    # A refilled part loses its body to the skeleton: keep the old body for the job that rewrites it.
    $m.old = [ordered]@{}
    $refill = @($parts | Where-Object { $_.existing -and $_.refill })
    if ($refill.Count) {
        $cards = @(Invoke-Zt @($refill | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = (Resolve-Ref $_.id $model) } } }))
        for ($k = 0; $k -lt $refill.Count; $k++) { if ("$($cards[$k].body)".Trim() -and $cards[$k].body -notmatch '^\s*待写\s*$') { $m.old[$refill[$k].id] = [string]$cards[$k].body } }
    }
    # Skeleton bodies, now that every address is final.
    $edits = @()
    $prev = $null
    foreach ($p in $parts) {
        if ($p.take) { continue }
        $address = Resolve-Ref $p.id $model
        $change = @{ at = $address }
        if ($p.existing -or $p.move) {
            foreach ($f in 'title', 'facet', 'emphasis') { if ($p[$f]) { $change[$f] = $p[$f] } }
            if ($p.refill) { $change.body = Get-Skeleton $p $plan $model $prev }
            if ($change.Count -gt 1) { $edits += @{ tool = 'edit_card'; arguments = $change } }
        } else {
            $edits += @{ tool = 'edit_card'; arguments = @{ at = $address; body = (Get-Skeleton $p $plan $model $prev) } }
        }
        if ((Get-Line $p) -eq 'main') { $prev = $p }
    }
    $card = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })
    $edits += @{ tool = 'edit_card'; arguments = @{ at = $model; body = (Set-Section $card.body '结构' (Get-StructureList $plan $model)) } }
    Invoke-Edits $edits
    $m.main = @($main | ForEach-Object { $_.id })
    $m.derive = @($parts | Where-Object { Test-Derive $_ } | ForEach-Object { $_.id })
    $m.status = 'built'

    # setting.md opens every DeepSeek job of this model, after the shared instructions.
    $setting = $main | Select-Object -First 1
    $settingText = if ($plan.level -eq 'formal') {
        "# 本模型的设定：[[$(Resolve-Ref $setting.id $model)]] $($setting.title)`n`n$($setting.spec)`n`n$(Get-Tables $plan)`n"
    } else { "# 本模型（概念模型）：[[$model]]`n`n$($plan.card)`n" }
    Save-Text (Join-Path $dir 'setting.md') $settingText

    # Round 1: the Model Card, then every part that is not a Derivation part, at most three per job.
    $jobs = @()
    $outside = @($card.outbound | Where-Object { -not $_.broken -and $_.address -notin @($m.ids.Values) } | ForEach-Object { "- [[$($_.address)]] $($_.title)" })
    $cardText = "## 模型卡：[[$model]]`n`n- 要写：问题、概要（三到五句）、结构、关联、来源。保留 **结构** 一节原样。`n- 内容：$($plan.card)`n$(if ($outside) { "- 保留这些模型外的链接，每个放进一句说明关系的话：`n$($outside -join "`n")`n" })$(if ($plan.level -eq 'concept') { "- 概念模型：用文字说清问题、机制和结论，不写推导。`n" })"
    # The writer rewrites the Model Card from its current body; the 结构 section is the script's.
    $current = (Set-Section ([string]$card.body) '结构' '').Trim()
    if ($current) {
        if ($current.Length -gt 4000) { $current = $current.Substring(0, 4000) + '……' }
        $cardText += "`n### 模型卡现在的正文（结构一节除外）`n$current`n"
    }
    $jobs += New-Job $model 'f' @() $cardText '按以上说明完成你的任务，最后每张卡回复一行 JSON。'
    $State.jobs[$jobs[-1].id].addresses = @($model)
    Save-Text $State.jobs[$jobs[-1].id].allow "$model`n"
    $todo = @($parts | Where-Object { -not $_.take -and -not (Test-Derive $_) -and (-not ($_.existing -or $_.move) -or $_.refill) })
    for ($i = 0; $i -lt $todo.Count; $i += 3) {
        $group = @($todo | Select-Object -Skip $i -First 3)
        $n = 0
        $text = @($group | ForEach-Object { $n++; Get-PartBlock $n $_ $model }) -join "`n"
        $jobs += New-Job $model 'f' @($group | ForEach-Object { $_.id }) $text '按以上说明完成你的任务，最后每张分卡回复一行 JSON。'
    }
    , $jobs
}

function Step-Apply {
    $jobs = @(); $models = @()
    foreach ($model in @($State.order | Where-Object { $null -ne $_ })) {
        $m = $State.models[$model]
        if (-not (Test-Path -LiteralPath (Join-Path (Get-Dir $model) 'plan.json')) -or $m.status -notin 'brief', 'planned') { continue }
        try { $jobs += Build-Model $model }
        catch { $m.status = 'failed'; $m.error = "$_" }
        $models += [ordered]@{ model = $model; status = $m.status; error = $m.error }
        Save-State
    }
    Save-State
    Out-Last ([ordered]@{ jobs = $jobs; models = $models })
}

function Get-PlanPart($model, $id) {
    $plan = Read-Json (Join-Path (Get-Dir $model) 'plan.json')
    $part = @($plan.parts | Where-Object { $null -ne $_ }) | Where-Object { $_.id -eq $id } | Select-Object -First 1
    if ($part) { return $part }
    # a part a split added: it continues its origin
    $added = $State.models[$model].added
    if ($added) { $added[$id] }
}

function Step-Derive {
    $jobs = @()
    foreach ($model in @($State.order | Where-Object { $null -ne $_ })) {
        $m = $State.models[$model]
        if ($m.status -ne 'built' -or -not @($m.derive | Where-Object { $null -ne $_ }).Count) { continue }
        # Chains in build order; one job holds at most three neighbouring parts of one chain. Each
        # part has its fixed 起点 and 结果, so the jobs of one chain run at the same time.
        $chains = [Collections.Generic.List[object]]::new()
        $chain = @()
        foreach ($id in @($m.main | Where-Object { $null -ne $_ }) + '') {
            $p = if ($id) { Get-PlanPart $model $id } else { $null }
            if ($p -and (Test-Derive $p)) { $chain += $p; continue }
            if ($chain.Count) { $chains.Add($chain); $chain = @() }
        }
        foreach ($c in $chains) {
            for ($i = 0; $i -lt $c.Count; $i += 3) {
                $group = @($c | Select-Object -Skip $i -First 3)
                $n = 0
                $text = @($group | ForEach-Object { $n++; Get-PartBlock $n $_ $model -Bodies }) -join "`n"
                $jobs += New-Job $model 'd' @($group | ForEach-Object { $_.id }) $text '按以上说明完成你的推导分卡，最后每张分卡回复一行 JSON。'
            }
        }
        $m.status = 'derived'
    }
    Save-State
    Out-Last ([ordered]@{ jobs = $jobs })
}

function Step-Review {
    $jobs = @()
    foreach ($model in @($State.order | Where-Object { $null -ne $_ })) {
        $m = $State.models[$model]
        if ($m.status -notin 'derived', 'reviewed' -or -not @($m.derive | Where-Object { $null -ne $_ }).Count) { continue }
        $structure = Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $model } })
        $bodies = @{}
        foreach ($p in @($structure.parts | Where-Object { $null -ne $_ })) { $bodies[$p.address] = $p }
        # Not $changed: PowerShell names are case-insensitive, and it would overwrite the -Changed switch.
        $touched = @($State.rereview | Where-Object { $null -ne $_ } | ForEach-Object { Resolve-Ref $_ $model })
        # Only the model's own Derivation parts: a shared part's chain belongs to its home model.
        $own = @($m.ids.Keys | Where-Object { $p = Get-PlanPart $model $_; -not ($p -and $p.take) } | ForEach-Object { [string]$m.ids[$_] })
        foreach ($c in @($structure.build.chains | Where-Object { $null -ne $_ })) {
            $parts = @($c.parts | Where-Object { $_ -in $own })
            if (-not $parts.Count) { continue }
            if ($Changed -and -not @($parts | Where-Object { $_ -in $touched }).Count) { continue }
            for ($i = 0; $i -lt $parts.Count; $i += 6) {
                $group = @($parts | Select-Object -Skip $i -First 6)
                $State.seq++
                $n = '{0:d2}' -f $State.seq
                $out = Join-Path (Get-Dir $model) "review-$n.json"
                $text = [Text.StringBuilder]::new()
                [void]$text.AppendLine("# 复核任务 $(Get-Slug $model):r$n`n`n结果文件: $out`nMaxSteps: $MaxSteps`n模型卡: [[$model]]`n")
                foreach ($around in @(@{ label = '链之前的分卡（推导从它出发）'; at = $c.after }, @{ label = '链之后的分卡（它用推导的结果）'; at = $c.before })) {
                    if ($around.at -and $bodies[$around.at]) { [void]$text.AppendLine("## $($around.label)：[[$($around.at)]] $($bodies[$around.at].title)`n`n$($bodies[$around.at].body)`n") }
                }
                foreach ($a in $group) { [void]$text.AppendLine("## 推导分卡 [[$a]] $($bodies[$a].title)`n`n$($bodies[$a].body)`n") }
                $file = Join-Path (Get-Dir $model) "review-$n.md"
                Save-Text $file $text.ToString()
                $jobs += [ordered]@{ id = "$(Get-Slug $model):r$n"; files = @($Docs.review, (Join-Path (Get-Dir $model) 'setting.md'), $file); prompt = "按以上说明复核这些推导分卡，把结果写进 $out，最后只回复一行 JSON。" }
            }
        }
        $m.status = 'reviewed'
    }
    $State.rereview = @()
    Save-State
    Out-Last ([ordered]@{ jobs = $jobs })
}

# The model and plan id of a part this batch built. A shared part (`take`) belongs to another
# model, which may be finished: the batch never splits or fixes it.
function Find-Owner($address) {
    foreach ($model in @($State.order | Where-Object { $null -ne $_ })) {
        $m = $State.models[$model]
        foreach ($id in @($m.ids.Keys)) {
            if ($m.ids[$id] -ne $address) { continue }
            $p = Get-PlanPart $model $id
            if ($p -and $p.take) { continue }
            return @{ model = $model; id = $id }
        }
    }
    $null
}
function Find-Job($model, $id) {
    $found = $null
    foreach ($key in @($State.jobs.Keys)) { $j = $State.jobs[$key]; if ($j.model -eq $model -and $id -in @($j.ids)) { $found = $key } }
    $found
}

function Split-Part($model, $id, [int]$count, $cuts, $reason) {
    $m = $State.models[$model]
    $p = Get-PlanPart $model $id
    $address = $m.ids[$id]
    $card = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $address } })
    $derive = [bool]($card.facet -and $card.facet.StartsWith('推导'))
    $final = if ($derive) { Get-Result $card.body } else { '' }
    $mainIndex = [array]::IndexOf(@($m.main | Where-Object { $null -ne $_ }), $id)
    $next = if ($mainIndex -ge 0 -and $mainIndex + 1 -lt @($m.main | Where-Object { $null -ne $_ }).Count) { @($m.main | Where-Object { $null -ne $_ })[$mainIndex + 1] } else { $null }
    if (-not $m.added) { $m.added = [ordered]@{} }
    $new = @()
    $prevId = $id
    for ($k = 2; $k -le $count; $k++) {
        $newId = "$id~$k"
        $facet = "$($card.facet)（$k/$count）"
        if ($facet.Length -gt 32) { $facet = $facet.Substring(0, 32) }
        $fields = @{ title = "$($card.title)（$k/$count）"; facet = $facet; type = 'model'; body = '待写' }
        if ($card.emphasis) { $fields.emphasis = $card.emphasis }
        $address = if ($next) {
            (Invoke-Write @(@{ tool = 'insert_card'; arguments = $fields + @{ before = $m.ids[$next]; confirm = 'insert' } }))[0]
        } else {
            $at = $m.ids[$prevId]
            $slot = Get-DirectSlot $at
            $set = Get-TreeAddresses $model
            if ($set.Contains($slot)) { (Invoke-Write @(@{ tool = 'insert_card'; arguments = $fields + @{ before = $slot; confirm = 'insert' } }))[0] }
            else { (Invoke-Write @(@{ tool = 'create_successor'; arguments = $fields + @{ at = $at; kind = 'direct' } }))[0] }
        }
        $m.ids[$newId] = $address
        $m.titles[$newId] = $fields.title
        $from = if (@($cuts | Where-Object { $null -ne $_ })[$k - 2]) { @($cuts | Where-Object { $null -ne $_ })[$k - 2] } else { '（上一张卡选定的中间结论）' }
        $to = if ($k -eq $count) { $final } elseif (@($cuts | Where-Object { $null -ne $_ })[$k - 1]) { @($cuts | Where-Object { $null -ne $_ })[$k - 1] } else { '（由你选定的中间结论）' }
        # The position line of 思路 comes from Update-ChainLines once every new part exists.
        $start = if ($derive) { "**思路**`n待写`n`n**起点**`n- 依赖上一步 [[$($m.ids[$prevId])]] 的结果：$from。`n`n**步骤**`n待写`n`n**结果**`n$to`n`n**解读**`n待写" } else { "依赖上一步 [[$($m.ids[$prevId])]]：本卡接着上一张卡写。`n`n待写" }
        $m.added[$newId] = [ordered]@{ id = $newId; facet = $facet; title = $fields.title; derive = $derive; to = $to; line = 'main'; spec = "接着 [[$($m.ids[$prevId])]] 写。" }
        Invoke-Edits @(@{ tool = 'edit_card'; arguments = @{ at = $address; body = $start } })
        $new += $newId
        $prevId = $newId
        if ($mainIndex -ge 0) { $list = [Collections.Generic.List[string]]::new([string[]]@($m.main | Where-Object { $null -ne $_ })); $list.Insert($mainIndex + $k - 1, $newId); $m.main = @($list) }
        if ($derive) { $m.derive = @($m.derive | Where-Object { $null -ne $_ }) + $newId }
    }
    $edits = @()
    if ($derive -and @($cuts | Where-Object { $null -ne $_ }).Count) {
        # The part now ends at the first cut: its 结果 is the cut, and its 解读 must be written again.
        $sections = Split-Sections ([string](Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $m.ids[$id] } })).body)
        foreach ($sec in $sections) {
            if ($sec.head -eq '结果') { $sec.lines = [Collections.Generic.List[string]]::new([string[]]@(@($cuts | Where-Object { $null -ne $_ })[0])) }
            if ($sec.head -eq '解读') { $sec.lines = [Collections.Generic.List[string]]::new([string[]]@('待写')) }
        }
        $edits += @{ tool = 'edit_card'; arguments = @{ at = $m.ids[$id]; body = (Join-Sections $sections) } }
    }
    if ($derive -and $next) {
        $nextCard = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $m.ids[$next] } })
        $edits += @{ tool = 'edit_card'; arguments = @{ at = $m.ids[$next]; body = $nextCard.body.Replace("[[$($m.ids[$id])]]", "[[$($m.ids[$new[-1]])]]") } }
    }
    # the 结构 list names the new parts right after the part they continue
    $head = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })
    $lines = [Collections.Generic.List[string]]::new([string[]]@(($head.body -replace "`r", '') -split "`n"))
    $at = -1
    for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i].Contains("[[$($m.ids[$id])]]") -and $lines[$i] -match '^\s*-') { $at = $i } }
    if ($at -ge 0) {
        $lines.InsertRange($at + 1, [string[]]@($new | ForEach-Object { "- $($m.added[$_].facet) [[$($m.ids[$_])]]：$($m.added[$_].title)" }))
        $edits += @{ tool = 'edit_card'; arguments = @{ at = $model; body = ($lines -join "`n") } }
    }
    Invoke-Edits $edits
    $m.split++
    $State.rereview = @($State.rereview | Where-Object { $null -ne $_ }) + @(@($id) + $new | ForEach-Object { "$model#$_" })
    # the job that wrote the part continues with the new parts
    $job = Find-Job $model $id
    if ($job) {
        $State.jobs[$job].ids = @($State.jobs[$job].ids | Where-Object { $null -ne $_ }) + $new
        $State.jobs[$job].addresses = @($State.jobs[$job].ids | ForEach-Object { Resolve-Ref $_ $model })
        Save-Text $State.jobs[$job].allow ((@($State.jobs[$job].addresses | Where-Object { $null -ne $_ }) -join "`n") + "`n")
    }
    $text = "## 拆分：[[$($m.ids[$id])]] 拆成 $count 张$(if ($reason) { "（$reason）" })`n`n"
    $text += "- [[$($m.ids[$id])]]：只写到 $(if ($derive -and @($cuts | Where-Object { $null -ne $_ }).Count) { @($cuts | Where-Object { $null -ne $_ })[0] } else { '你选的中间结论' })。$(if ($derive) { '它的解读已清空：按新的结果重写。' })`n"
    if ($derive) { $text += "- 每张推导分卡写自己的思路、说明和解读。思路的位置行由脚本写好，原样保留。`n" }
    foreach ($n in $new) { $text += "- [[$($m.ids[$n])]]：$($m.added[$n].title)，从上一张的结果写到 $(if ($m.added[$n].to) { $m.added[$n].to } else { '你选的中间结论' })。标题可改成本卡的结论。`n" }
    @{ job = $job; text = $text; ids = @(@($id) + $new) }
}

function Step-Split {
    $follow = [ordered]@{}   # job id -> what its conversation must do next
    $orphans = @()
    $add = {
        param($r)
        $key = if ($r.job) { $r.job } else { "new:$($r.model)" }
        $was = $follow[$key]
        $follow[$key] = [ordered]@{ model = $r.model; text = "$(if ($was) { $was.text })$($r.text)`n"; ids = @(@(if ($was) { $was.ids }) + $r.ids | Select-Object -Unique) }
    }
    # Name every target by its model and id first: a split renumbers the addresses after it.
    $actions = @()
    foreach ($pair in @($Need -split ';' | Where-Object { $_.Trim() })) {
        $address, $count = $pair -split '=', 2
        $owner = Find-Owner $address.Trim()
        if ($owner) { $actions += @{ owner = $owner; verdict = 'split'; parts = [int]$count; cuts = @(); reason = '写不下' } } else { $orphans += $address }
    }
    foreach ($model in @($State.order | Where-Object { $null -ne $_ })) {
        foreach ($file in @(Get-ChildItem -LiteralPath (Get-Dir $model) -Filter 'review-*.json' -File -ErrorAction SilentlyContinue)) {
            if ($file.FullName -in @($State.reviews | Where-Object { $null -ne $_ })) { continue }
            $State.reviews = @($State.reviews | Where-Object { $null -ne $_ }) + $file.FullName
            $verdicts = @(try { Read-Json $file.FullName } catch { $orphans += "$($file.Name): $_"; @() })
            foreach ($v in $verdicts) {
                if ($v.verdict -notin 'split', 'fix') { continue }
                $owner = Find-Owner $v.address
                if ($owner) { $actions += @{ owner = $owner; verdict = $v.verdict; parts = [int]$v.parts; cuts = @($v.cuts | Where-Object { $null -ne $_ }); reason = $v.reason } } else { $orphans += $v.address }
            }
        }
    }
    $resplit = @()
    foreach ($a in $actions) {
        $o = $a.owner
        if ($a.verdict -eq 'split' -and $a.parts -ge 2) {
            $r = Split-Part $o.model $o.id $a.parts $a.cuts $a.reason
            $r.model = $o.model
            $resplit += $o.model
            & $add $r
        } else {
            $State.rereview = @($State.rereview | Where-Object { $null -ne $_ }) + "$($o.model)#$($o.id)"
            & $add @{ model = $o.model; job = (Find-Job $o.model $o.id); text = "## 修正：[[$(Resolve-Ref $o.id $o.model)]]`n`n$($a.reason)`n"; ids = @($o.id) }
        }
    }
    # A split changes the length of its chain: every member gets its new position line.
    foreach ($model in @($resplit | Select-Object -Unique)) { Update-ChainLines $model }
    $jobs = @()
    foreach ($key in @($follow.Keys)) {
        $f = $follow[$key]
        $State.seq++
        $file = Join-Path (Get-Dir $f.model) "follow-$('{0:d2}' -f $State.seq).md"
        $allow = if ($key -like 'new:*') { $null } else { $State.jobs[$key].allow }
        if (-not $allow) {
            $n = 0
            $blocks = @($f.ids | ForEach-Object { $p = Get-PlanPart $f.model $_; if ($p) { $n++; Get-PartBlock $n $p $f.model -Bodies } }) -join "`n"
            $jobs += New-Job $f.model 'd' $f.ids "$($f.text)`n$blocks" '按以上说明完成你的分卡，最后每张分卡回复一行 JSON。'
            continue
        }
        Save-Text $file "# 补充任务（接着你刚才的分卡写）`n`nALLOW 已更新：$allow`n`n$($f.text)`n写完后照常运行 CHECK（地址加上新分卡）。"
        # fallback: the files of a fresh conversation, when the one that wrote the part is gone
        $fallback = @($Docs.fill, (Join-Path (Get-Dir $f.model) 'setting.md'), $State.jobs[$key].file, $file)
        $jobs += [ordered]@{ id = "$key+$('{0:d2}' -f $State.seq)"; continue = $key; files = @($file); fallback = $fallback; prompt = '按以上补充说明修改和续写你的分卡，最后每张分卡回复一行 JSON。' }
    }
    Save-State
    Out-Last ([ordered]@{ jobs = $jobs; orphans = $orphans })
}

function Step-Report {
    $reports = @()
    foreach ($model in @($State.order | Where-Object { $null -ne $_ })) {
        $m = $State.models[$model]
        $s = Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $model } })
        $style = 0
        if (@($s.parts | Where-Object { $null -ne $_ }).Count) {
            # Shared parts belong to their home models, which this batch does not change.
            $lines = @(Invoke-ZtCheck -Model $model -SkipShared -MaxSentences $MaxSentences -MaxChars $MaxChars -MaxSteps $MaxSteps -MaxExplainSentences $MaxExplainSentences -MaxExplainChars $MaxExplainChars)
            $last = [string]($lines | Where-Object { [string]$_ -match '^faults: \d+' } | Select-Object -Last 1)
            $style = if ($last) { [int]($last -replace '^faults: ', '') } else { -1 }
        }
        $kinds = [ordered]@{}
        foreach ($r in @($s.relations | Where-Object { $null -ne $_ })) { $kinds[$r.kind] = [int]$kinds[$r.kind] + 1 }
        $order = @($s.build.order | Where-Object { $null -ne $_ })
        $chains = @($s.build.chains | Where-Object { $null -ne $_ })
        $reports += [ordered]@{
            model = $model; level = $s.level
            status = if ($m.status -in 'failed', 'skipped') { $m.status } elseif ($m.status -eq 'brief') { 'failed' } else { 'done' }
            error = $m.error
            parts = @($order | ForEach-Object { [ordered]@{ address = $_.address; facet = $_.facet; emphasis = $_.emphasis; line = $_.line } })
            relations = @($s.relations | Where-Object { $null -ne $_ }).Count; kinds = $kinds
            neighbours = @($s.neighbours | Where-Object { $null -ne $_ }).Count; loose = @($s.loose | Where-Object { $null -ne $_ }).Count
            charts = @($s.parts | Where-Object { $_.body -match '```chart' }).Count
            todo = @($s.parts | Where-Object { $_.body -match '(^|\n)\s*待写\s*($|\n)' } | ForEach-Object { $_.address })
            derivations = [ordered]@{ total = @($order | Where-Object { $_.derivation -and $_.line -ne 'shared' }).Count; explained = @(Get-Explained $s).Count; chains = @($chains | Where-Object { $_.length -ge 2 }).Count; longest = [int](@($chains | ForEach-Object { [int]$_.length }) + @(0) | Measure-Object -Maximum).Maximum }
            split = [int]$m.split; moved = @($m.moved | Where-Object { $null -ne $_ }); shared = @($order | Where-Object { $_.line -eq 'shared' }).Count
            tolerated = @($s.build.tolerated | Where-Object { $null -ne $_ }).Count; style = $style
            faults = [ordered]@{ closure = @($s.build.closure | Where-Object { $null -ne $_ }).Count; forward = @($s.build.forward | Where-Object { $null -ne $_ }).Count; unanchored = @($s.build.unanchored | Where-Object { $null -ne $_ }).Count; beyond_level = @($s.build.beyond_level | Where-Object { $null -ne $_ }).Count }
            issues = @($s.issues | ForEach-Object { if ($_ -is [string]) { $_ } else { $_ | ConvertTo-Json -Compress } })
            notes = @($m.notes | Where-Object { $null -ne $_ })
        }
    }
    Out-Last ([ordered]@{ models = $reports })
}

# --- Explain: 思路, 说明, and 解读 for finished Derivation parts -------------------------------------
# The text of a section without the position line and 待写; '' when it says nothing yet.
function Get-SectionText($sections, [string]$head) {
    $s = $sections | Where-Object { $_.head -eq $head } | Select-Object -First 1
    if (-not $s) { return '' }
    (@($s.lines | Where-Object { $_.Trim() -and $_.Trim() -ne '待写' -and $_.Trim() -notmatch '^第 \d+ 张，共 \d+ 张' }) -join "`n").Trim()
}
function Test-Explained([string]$body) {
    $sections = Split-Sections $body
    [bool]((Get-SectionText $sections '思路') -and (Get-SectionText $sections '解读'))
}
# The model's own Derivation parts (shared parts belong to their home models), in build order.
function Get-OwnDerivations($s) {
    @($s.build.order | Where-Object { $_.derivation -and $_.line -ne 'shared' } | ForEach-Object { [string]$_.address })
}
function Get-Explained($s) {
    $bodies = @{}
    foreach ($p in @($s.parts | Where-Object { $null -ne $_ })) { $bodies[[string]$p.address] = [string]$p.body }
    @(Get-OwnDerivations $s | Where-Object { Test-Explained $bodies[$_] })
}
# What explain-apply must never change: 起点, the step lines, 结果, and any other text, line by line.
# The 来源 line counts too: Add-Explanation moves it after 解读, so it reads back inside 解读.
function Get-Core([string]$body) {
    $lines = foreach ($sec in (Split-Sections $body)) {
        $explanation = $sec.head -in '思路', '解读'
        foreach ($l in $sec.lines) {
            if (-not $l.Trim()) { continue }
            if ($explanation) { if ($l.Trim() -match '^来源\s*[：:]') { $l.Trim() }; continue }
            if ($l.Trim() -notmatch '^说明\s*[：:]') { $l.Trim() }
        }
    }
    $lines -join "`n"
}
# The body with the new 思路 (after the position line), 说明 lines, and 解读. Old 思路, 解读, and
# 说明 lines go. A 来源 line at the end of 结果 stays the last line, after 解读.
function Add-Explanation([string]$body, [string]$idea, $notes, [string]$meaning, [string]$position) {
    $sections = [Collections.Generic.List[object]]::new()
    foreach ($sec in (Split-Sections $body)) { if ($sec.head -notin '思路', '解读') { $sections.Add($sec) } }
    $steps = $sections | Where-Object { $_.head -eq '步骤' } | Select-Object -First 1
    if ($steps) {
        $out = [Collections.Generic.List[string]]::new()
        foreach ($line in $steps.lines) {
            if ($line.Trim() -match '^说明\s*[：:]') { continue }
            $out.Add($line)
            if ($notes -and $line -match '^\s*(\d+)[.、)]') {
                $note = [string]$notes[$Matches[1]]
                if ($note.Trim()) { $out.Add("   说明：$($note.Trim() -replace '^说明\s*[：:]\s*', '')") }
            }
        }
        $steps.lines = $out
    }
    $ideaLines = [Collections.Generic.List[string]]::new([string[]]@(@($position, $idea.Trim()) | Where-Object { $_ }))
    $at = 0
    for ($i = 0; $i -lt $sections.Count; $i++) { if ($sections[$i].head -eq '起点') { $at = $i; break } }
    if ($at -eq 0 -and $sections.Count -and -not $sections[0].head) { $at = 1 }
    $sections.Insert($at, @{ head = '思路'; lines = $ideaLines })
    $result = -1
    for ($i = 0; $i -lt $sections.Count; $i++) { if ($sections[$i].head -eq '结果') { $result = $i } }
    $tail = @()
    if ($result -ge 0) {
        $keep = [Collections.Generic.List[string]]::new([string[]]@($sections[$result].lines))
        while ($keep.Count -and -not $keep[$keep.Count - 1].Trim()) { $keep.RemoveAt($keep.Count - 1) }
        if ($keep.Count -and $keep[$keep.Count - 1].Trim() -match '^来源\s*[：:]') { $tail = @($keep[$keep.Count - 1]); $keep.RemoveAt($keep.Count - 1) }
        $sections[$result].lines = $keep
    }
    $where = if ($result -ge 0) { $result + 1 } else { $sections.Count }
    $sections.Insert($where, @{ head = '解读'; lines = [Collections.Generic.List[string]]::new([string[]]@($meaning.Trim())) })
    if ($tail.Count) { $sections.Insert($where + 1, @{ head = ''; lines = [Collections.Generic.List[string]]::new([string[]]$tail) }) }
    Join-Sections $sections
}
# The command a writer runs to check its own result file (the SELF-CHECK line of its job file).
function Get-ExplainCheckCommand([string]$id) {
    "pwsh -NoProfile -File `"$PSCommandPath`" -Step explain-check -Work `"$Work`" -Job `"$id`" -MaxSentences $MaxSentences -MaxChars $MaxChars -MaxSteps $MaxSteps $Explain"
}
function Get-ExplainState {
    if (-not $State.explain) { $State.explain = [ordered]@{ models = @(); settings = [ordered]@{}; jobs = [ordered]@{}; refused = [ordered]@{}; blocked = [ordered]@{} } }
    $State.explain
}

function Step-Explain {
    $X = Get-ExplainState
    $jobs = @()
    $left = if ($MaxParts -gt 0) { $MaxParts } else { [int]::MaxValue }
    $pending = @{}
    foreach ($j in $X.jobs.Values) { foreach ($a in @($j.addresses | Where-Object { $_ -notin @($j.done) })) { $pending[$a] = $true } }
    foreach ($model in @($Models -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
        if ($left -le 0) { break }
        $s = Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $model } })
        if (-not $s.head -or $s.level -eq 'concept') { continue }
        $bodies = @{}; $titles = @{}
        foreach ($p in @($s.parts | Where-Object { $null -ne $_ })) { $bodies[[string]$p.address] = [string]$p.body; $titles[[string]$p.address] = [string]$p.title }
        $own = @(Get-OwnDerivations $s)
        $missing = @($own | Where-Object { -not (Test-Explained $bodies[$_]) -and -not $pending.ContainsKey($_) })
        if (-not $missing.Count) { continue }
        $dir = Get-Dir $model
        if ($model -notin @($X.models)) { $X.models = @(@($X.models) + $model) }
        $X.settings[$model] = [string]$s.build.setting
        $setting = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $s.build.setting } })
        Save-Text (Join-Path $dir 'setting.md') "# 本模型的设定：[[$($setting.address)]] $($setting.title)`n`n$($setting.body)`n"
        $context = '无'
        if ($Staging) {
            $context = Join-Path $dir 'context.md'
            $null = pwsh -NoProfile -File (Join-Path $PSScriptRoot 'model-context.ps1') -Staging $Staging -Model $model -Out $context
            if ($LASTEXITCODE -ne 0) { $context = '无' }
        }
        $head = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $model } })
        $headBody = [string]$head.body
        if ($headBody.Length -gt 3000) { $headBody = $headBody.Substring(0, 3000) + '……' }
        # Groups: up to three neighbouring parts of one chain, with the members around them.
        $groups = [Collections.Generic.List[object]]::new()
        $covered = @{}
        foreach ($c in @($s.build.chains | Where-Object { $null -ne $_ })) {
            $parts = @($c.parts | Where-Object { $_ -in $own } | ForEach-Object { [string]$_ })
            $tos = @($parts | ForEach-Object { Get-Result $bodies[$_] })
            $todo = @($parts | Where-Object { $_ -in $missing })
            for ($i = 0; $i -lt $todo.Count; $i += 3) {
                $g = @($todo | Select-Object -Skip $i -First 3)
                $groups.Add(@{ parts = $g; chain = $parts; tos = $tos; after = $c.after; before = $c.before })
                foreach ($a in $g) { $covered[$a] = $true }
            }
        }
        foreach ($a in @($missing | Where-Object { -not $covered.ContainsKey($_) })) { $groups.Add(@{ parts = @($a); chain = @($a); tos = @(Get-Result $bodies[$a]); after = $null; before = $null }) }
        foreach ($g in $groups) {
            if ($left -le 0) { break }
            $g.parts = @($g.parts | Select-Object -First $left)
            $left -= $g.parts.Count
            $State.seq++
            $id = "$(Get-Slug $model):x$('{0:d2}' -f $State.seq)"
            $out = Join-Path (Join-Path $Work 'out') "explain-$('{0:d2}' -f $State.seq).json"
            $file = Join-Path $dir "explain-$('{0:d2}' -f $State.seq).md"
            $text = [Text.StringBuilder]::new()
            [void]$text.AppendLine("# 讲解任务 $id`n`n结果文件: $out`nSELF-CHECK: $(Get-ExplainCheckCommand $id)`n上下文包: $context`n模型卡: [[$model]] $($head.title)`n`n## 模型卡的正文`n`n$headBody`n")
            if ($g.after -and $bodies[$g.after]) { [void]$text.AppendLine("## 链之前的分卡（推导从它出发）：[[$($g.after)]] $($titles[$g.after])`n`n$($bodies[$g.after])`n") }
            $positions = [ordered]@{}
            foreach ($a in $g.parts) {
                $k = [array]::IndexOf(@($g.chain), $a) + 1
                $positions[$a] = Format-ChainLine $k @($g.chain).Count $g.tos
                [void]$text.AppendLine("## 推导分卡 [[$a]] $($titles[$a])$(if (@($g.chain).Count -ge 2) { "（第 $k 张，共 $(@($g.chain).Count) 张）" })`n`n$($bodies[$a])`n")
            }
            if ($g.before -and $bodies[$g.before]) { [void]$text.AppendLine("## 链之后的分卡（它用推导的结果）：[[$($g.before)]] $($titles[$g.before])`n`n$($bodies[$g.before])`n") }
            Save-Text $file $text.ToString()
            $X.jobs[$id] = [ordered]@{ model = $model; file = $file; out = $out; addresses = @($g.parts); done = @(); round = 1
                bodies = [ordered]@{}; positions = $positions }
            foreach ($a in $g.parts) { $X.jobs[$id].bodies[$a] = $bodies[$a] }
            $jobs += [ordered]@{ id = $id; files = @($Docs.explain, (Join-Path $dir 'setting.md'), $file); prompt = "按以上说明写出结果文件 $out，最后只回复一行 JSON。" }
        }
    }
    New-Item -ItemType Directory -Force -Path (Join-Path $Work 'out') | Out-Null
    Save-State
    Out-Last ([ordered]@{ jobs = $jobs; parts = @($jobs | ForEach-Object { @($X.jobs[$_.id].addresses).Count } | Measure-Object -Sum).Sum })
}

# What explain-apply would do with one job's result file, without a write: the new bodies that
# pass, and why each other part does not. explain-check (the writer's own check) and explain-apply
# share it, so the writer sees exactly what the apply step will refuse.
function Test-ExplainJob([string]$id, [string]$file) {
    $X = Get-ExplainState
    $j = $X.jobs[$id]
    if (-not $j) { throw "no explain job $id" }
    $r = @{ faults = [ordered]@{}; candidates = @(); blocked = @(); refused = @(); kept = @() }
    $todo = @($j.addresses | Where-Object { $_ -notin @($j.done) })
    if (-not $todo.Count) { return $r }
    $entries = @()
    if (Test-Path -LiteralPath $file) {
        try { $entries = @(Read-Json $file) } catch { foreach ($a in $todo) { $r.faults[$a] = "the result file is not a JSON array: $_" } }
    } else { foreach ($a in $todo) { $r.faults[$a] = 'no result file' } }
    $cards = @{}
    foreach ($c in @(Invoke-Zt @($todo | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } }))) { $cards[[string]$c.address] = $c }
    foreach ($a in $todo) {
        if ($r.faults.Contains($a)) { continue }
        $e = $entries | Where-Object { $_ -and [string]$_.address -eq $a } | Select-Object -First 1
        if (-not $e) { $r.faults[$a] = 'no entry for this part in the result file'; continue }
        if ($e.blocked) { $r.blocked += "$($a): $($e.blocked)"; continue }
        $card = $cards[$a]
        if (([string]$card.body).Trim() -ne ([string]$j.bodies[$a]).Trim()) { $r.refused += "$($a): the part changed after its job"; continue }
        if (-not ([string]$e.idea).Trim() -or -not ([string]$e.meaning).Trim()) { $r.faults[$a] = 'idea and meaning must not be empty'; continue }
        $new = Add-Explanation ([string]$card.body) ([string]$e.idea) $e.notes ([string]$e.meaning) ([string]$j.positions[$a])
        if ((Get-Core $new) -ne (Get-Core ([string]$card.body))) { $r.refused += "$($a): inserting the explanation would change 起点, a step, or 结果"; continue }
        $r.candidates += [ordered]@{ address = $a; title = [string]$card.title; facet = [string]$card.facet; body = $new; old = [string]$card.body }
    }
    if (-not $r.candidates.Count) { return $r }
    # The new bodies pass R2 to R17; the setting gives the term table (R6). Only a fault the
    # explanation adds counts: a fault the old body already has (a long derivation, a passive
    # sentence) stays as it is and is reported as kept, not as the writer's.
    $setting = [string]$X.settings[$j.model]
    $sc = if ($setting) { Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $setting } }) }
    $styleFaults = {
        param($items, [string]$name, [bool]$explain)
        $toCheck = @($items)
        if ($setting) { $toCheck += [ordered]@{ address = $setting; title = [string]$sc.title; body = [string]$sc.body; facet = [string]$sc.facet } }
        $entriesFile = Join-Path ([IO.Path]::GetTempPath()) "explain-$PID-$name"
        Save-Text $entriesFile (ConvertTo-Json -InputObject @($toCheck) -Depth 5)
        $checkArgs = @{ Json = $entriesFile; MaxSentences = $MaxSentences; MaxChars = $MaxChars; MaxSteps = $MaxSteps; MaxExplainSentences = $MaxExplainSentences; MaxExplainChars = $MaxExplainChars; RequireExplain = $explain }
        if ($setting) { $checkArgs.Setting = $setting }
        # Invoke-ZtCheck throws when zt-check does not run: nothing unchecked is written.
        try { $checked = @(Invoke-ZtCheck @checkArgs) } finally { Remove-Item -LiteralPath $entriesFile -ErrorAction SilentlyContinue }
        $out = @{}
        foreach ($line in $checked) {
            if ([string]$line -match '^(\S+) (R\d+ .*)$' -and $Matches[1] -ne $setting) {
                if (-not $out.ContainsKey($Matches[1])) { $out[$Matches[1]] = [Collections.Generic.List[string]]::new() }
                $out[$Matches[1]].Add($Matches[2])
            }
        }
        $out
    }
    $old = & $styleFaults @($r.candidates | ForEach-Object { [ordered]@{ address = $_.address; title = $_.title; body = $_.old; facet = $_.facet } }) 'old.json' $false
    $all = & $styleFaults @($r.candidates | ForEach-Object { [ordered]@{ address = $_.address; title = $_.title; body = $_.body; facet = $_.facet } }) 'new.json' $true
    $pass = @()
    foreach ($c in $r.candidates) {
        $a = $c.address
        $before = [Collections.Generic.List[string]]::new()
        if ($old.ContainsKey($a)) { foreach ($f in $old[$a]) { $before.Add($f) } }
        $now = if ($all.ContainsKey($a)) { @($all[$a]) } else { @() }
        $added = @(foreach ($f in $now) { if (-not $before.Remove($f)) { $f } })
        $was = @($now | Where-Object { $_ -notin $added })
        if ($was.Count) { $r.kept += "$($a): $($was -join ' | ')" }
        if ($added.Count) { $r.faults[$a] = $added -join ' | ' } else { $pass += $c }
    }
    $r.candidates = $pass
    $r
}

# The writer's own check of its result file: what explain-apply would refuse, as fault lines.
function Step-ExplainCheck {
    $X = Get-ExplainState
    if (-not $Job -or -not $X.jobs.Contains($Job)) { throw "explain-check needs -Job <an explain job id>" }
    $file = if ($File) { $File } else { [string]$X.jobs[$Job].out }
    $r = Test-ExplainJob $Job $file
    foreach ($a in @($r.faults.Keys)) { Write-Output "$a explain $($r.faults[$a])" }
    foreach ($t in $r.refused) { Write-Output ($t -replace '^(\S+): ', '$1 refused ') }
    foreach ($t in $r.blocked) { Write-Output "(blocked, not counted) $t" }
    foreach ($t in $r.kept) { Write-Output "(kept: the old body has these faults; not yours, not counted) $t" }
    foreach ($c in $r.candidates) { Write-Output "$($c.address) ok" }
    Write-Output "faults: $($r.faults.Count + $r.refused.Count)"
}

function Step-ExplainApply {
    $X = Get-ExplainState
    $applied = @(); $refused = @(); $blocked = @(); $jobs = @(); $edits = @(); $done = @(); $kept = @()
    foreach ($id in @($X.jobs.Keys)) {
        $j = $X.jobs[$id]
        if (-not @($j.addresses | Where-Object { $_ -notin @($j.done) }).Count) { continue }
        $r = Test-ExplainJob $id ([string]$j.out)
        $kept += $r.kept
        foreach ($t in $r.blocked) { $blocked += @{ model = $j.model; text = $t }; $done += @{ job = $id; address = ($t -split ': ', 2)[0] } }
        foreach ($t in $r.refused) { $refused += @{ model = $j.model; text = $t }; $done += @{ job = $id; address = ($t -split ': ', 2)[0] } }
        foreach ($c in $r.candidates) {
            $edits += @{ tool = 'edit_card'; arguments = @{ at = $c.address; body = $c.body } }
            $applied += $c.address
            $done += @{ job = $id; address = $c.address }
        }
        $faults = $r.faults
        if (-not $faults.Count) { continue }
        if ([int]$j.round -ge 2) {
            foreach ($a in @($faults.Keys)) { $refused += @{ model = $j.model; text = "$($a): $($faults[$a])" }; $done += @{ job = $id; address = $a } }
            continue
        }
        # Once: the same conversation rewrites the refused parts into a new result file.
        $j.round = 2
        $State.seq++
        $j.out = Join-Path (Join-Path $Work 'out') "explain-$('{0:d2}' -f $State.seq).json"
        $follow = Join-Path (Get-Dir $j.model) "explain-follow-$('{0:d2}' -f $State.seq).md"
        $lines = @($faults.Keys | ForEach-Object { "- [[$_]]：$($faults[$_])" })
        Save-Text $follow "# 补充任务（改正这些分卡的解释）`n`n新的结果文件: $($j.out)`nSELF-CHECK: $(Get-ExplainCheckCommand $id)`n`n脚本没有写入下面的分卡。改正每张分卡的问题，只为它们写新的结果文件。写完后运行 SELF-CHECK，直到最后一行是 ``faults: 0``：`n`n$($lines -join "`n")`n"
        $jobs += [ordered]@{ id = "$id+2"; continue = $id; files = @($follow); fallback = @($Docs.explain, (Join-Path (Get-Dir $j.model) 'setting.md'), $j.file, $follow); prompt = '按以上补充说明改正，写出新的结果文件，最后只回复一行 JSON。' }
    }
    for ($i = 0; $i -lt $edits.Count; $i += 25) { Invoke-Edits @($edits | Select-Object -Skip $i -First 25) }
    foreach ($d in $done) { $X.jobs[$d.job].done = @(@($X.jobs[$d.job].done) + $d.address) }
    foreach ($r in $refused) { $X.refused[$r.model] = @(@($X.refused[$r.model] | Where-Object { $_ }) + $r.text) }
    foreach ($b in $blocked) { $X.blocked[$b.model] = @(@($X.blocked[$b.model] | Where-Object { $_ }) + $b.text) }
    Save-State
    Out-Last ([ordered]@{ applied = $applied.Count; refused = @($refused | ForEach-Object { $_.text }); blocked = @($blocked | ForEach-Object { $_.text }); kept = @($kept); jobs = $jobs })
}

function Step-ExplainReport {
    $X = Get-ExplainState
    $reports = @()
    foreach ($model in @($X.models)) {
        $s = Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $model } })
        $own = @(Get-OwnDerivations $s)
        $explained = @(Get-Explained $s)
        $reports += [ordered]@{
            model = $model; derivations = $own.Count; explained = $explained.Count
            missing = @($own | Where-Object { $_ -notin $explained })
            refused = @($X.refused[$model] | Where-Object { $_ }); blocked = @($X.blocked[$model] | Where-Object { $_ })
        }
    }
    Out-Last ([ordered]@{ models = $reports })
}

switch ($Step) {
    'explain' { Step-Explain }
    'explain-check' { Step-ExplainCheck }
    'explain-apply' { Step-ExplainApply }
    'explain-report' { Step-ExplainReport }
    'brief' { Step-Brief }
    'validate' { Step-Validate }
    'apply' { Step-Apply }
    'derive' { Step-Derive }
    'review' { Step-Review }
    'split' { Step-Split }
    'report' { Step-Report }
}
