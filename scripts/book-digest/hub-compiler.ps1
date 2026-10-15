#Requires -Version 7
<#
.SYNOPSIS
  The deterministic half of the book digest's hub stage (doc/hub-cards-design.md). A Haiku hub
  manager chooses the limits from the survey; DeepSeek plans the fields and writes every hub as a
  JSON file; this script does everything between them without a model: it surveys the agent
  Workspace, writes the job files, builds and updates the Hub Cards, checks them with get_hub, and
  reports. No step writes hub prose of its own except fixed lines (范围, the register's lists).

.DESCRIPTION
  -Work is the stage's folder and the DeepSeek Harness cwd. It holds survey.json, survey.md,
  limits.json, state.json, the job files, and out\ (the JSON files DeepSeek writes). Every step
  prints compact JSON as its last line. A step that hands work to DeepSeek prints
  {"jobs":[{"id","files":[...],"prompt","continue"?}], ...}.

    survey                 reads every agent Topic tree and the hub graph; writes survey.json and
                           survey.md (one line per Topic); prints the size statistics the hub
                           manager reads to choose the limits.
    plan -Limits <json>    clamps and saves the limits; picks the trees whose hubs are missing,
                           faulty, legacy (an Idea Card used as a hub), or whose range changed
                           (all with -Rebuild, only -Trees when given); writes one topic job per
                           -PerJob trees and one field job; prints the jobs.
    apply                  reads out\hub-<tree>.json and out\fields.json, drops members that do
                           not exist, creates or updates the line, topic, and field hubs and the
                           register (Topic 索引), and links each topic hub from its Topic body.
    check                  runs get_hub on every hub of the state with the limits; prints one
                           follow-up job per writer conversation whose hubs have faults.
    report                 list_hubs with the limits: counts, faults, missing hubs; writes
                           report.json.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('survey', 'plan', 'apply', 'check', 'report')][string]$Step,
    [Parameter(Mandatory)][string]$Work,
    [string]$Limits = '',
    [string]$Trees = '',
    [int]$PerJob = 0,
    [switch]$Rebuild
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
$Helper = $script:ZtHelper
$Doc = Join-Path $PSScriptRoot 'dsh-hub-instructions.zh.md'
$Out = Join-Path $Work 'out'
New-Item -ItemType Directory -Force -Path $Work, $Out | Out-Null
$StateFile = Join-Path $Work 'state.json'
$Kinds = @{ '依赖' = 1; '基于' = 1; '例示' = 1; '案例' = 1; '对照' = 1; '区别' = 1; '放宽' = 1; '推广' = 1; '对应' = 1; '类比' = 1; '反驳' = 1; '批评' = 1 }

function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String }
function Save-Text($path, $text) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
    [IO.File]::WriteAllText($path, $text, $utf8)
}
function Out-Last($object) { $object | ConvertTo-Json -Depth 20 -Compress }
$State = if (Test-Path -LiteralPath $StateFile) { Read-Json $StateFile } else { [ordered]@{ trees = [ordered]@{}; fields = [ordered]@{}; register = $null; jobs = [ordered]@{}; seq = 0 } }
function Save-State { Save-Text $StateFile ($State | ConvertTo-Json -Depth 20) }

# Writes through the helper, in one batch; returns the address of each write in order.
function Invoke-Write($calls) {
    if (-not @($calls).Count) { return , @() }
    $lines = @(($calls | ConvertTo-Json -Depth 10 -AsArray) | pwsh -NoProfile -File $Helper -)
    $failed = $LASTEXITCODE -ne 0
    $addresses = @()
    foreach ($line in $lines) {
        if ($line -match '^\d+ ok (.+)$') { $addresses += $Matches[1].Trim() }
        elseif ($line -match '^\d+ error (.*)$') { throw "zt: $($Matches[1])" }
    }
    if ($failed) { throw "zt helper failed: $($lines -join ' | ')" }
    , $addresses
}

function Get-Quantiles($values) {
    $sorted = @($values | Sort-Object)
    if (-not $sorted.Count) { return [ordered]@{ n = 0 } }
    $at = { param($q) $sorted[[Math]::Min($sorted.Count - 1, [int][Math]::Floor($q * ($sorted.Count - 1) + 0.5))] }
    [ordered]@{ n = $sorted.Count; min = $sorted[0]; p25 = & $at 0.25; median = & $at 0.5; p75 = & $at 0.75; p90 = & $at 0.9; max = $sorted[-1] }
}

function Read-Limits {
    $file = Join-Path $Work 'limits.json'
    if (Test-Path -LiteralPath $file) { return Read-Json $file }
    [ordered]@{ topic_min = 7; line_min = 12; max_lines = 25; max_groups = 7; min_reason = 8; neighbour_links = 3; field_min = 3; field_max = 12; per_job = 5 }
}
function Get-LimitArgs($limits) {
    $args = [ordered]@{}
    foreach ($name in 'topic_min', 'line_min', 'max_lines', 'max_groups', 'min_reason', 'neighbour_links') { $args[$name] = [int]$limits[$name] }
    $args
}

# --- survey ------------------------------------------------------------------------------------------
function Step-Survey {
    $trees = @((Invoke-Zt @(@{ tool = 'list_trees'; arguments = @{ workspace = 'agent'; zone = 'topic' } })).trees)
    $results = @(Invoke-Zt @($trees | ForEach-Object { @{ tool = 'get_tree'; arguments = @{ tree = $_.tree } } }))
    $graph = Invoke-Zt @(@{ tool = 'list_hubs'; arguments = @{ workspace = 'agent' } })
    # Links between Topic trees, both ways, and the Cards of the other tree that each pair's Links
    # touch. A hub's own Links are not evidence that two subjects meet.
    $pair = @{}
    $touched = @{}
    $titles = @{}
    foreach ($t in $results) { foreach ($card in $t.cards) { $titles[$card.address] = $card.title } }
    foreach ($t in $results) {
        foreach ($card in @($t.cards | Where-Object { $_.type -ne 'hub' })) {
            $from = ($card.address -split '/', 2)[0]
            foreach ($target in @($card.links)) {
                $to = ($target -split '/', 2)[0]
                if ($to -eq $from -or $to -notmatch '^\d+$' -or $target -match '/0$') { continue }
                foreach ($key in "$from>$to", "$to>$from") { $pair[$key] = 1 + [int]$pair[$key] }
                foreach ($k in @(@("$from>$to", $target), @("$to>$from", $card.address))) {
                    if (-not $touched.ContainsKey($k[0])) { $touched[$k[0]] = @{} }
                    $touched[$k[0]][$k[1]] = 1 + [int]$touched[$k[0]][$k[1]]
                }
            }
        }
    }
    $survey = [ordered]@{ trees = [ordered]@{}; register = $null }
    $treeSizes = @(); $branchSizes = @(); $pairCounts = @()
    foreach ($t in $results) {
        $tree = [string]$t.tree
        $cards = @($t.cards | Where-Object { $_.edge -ne 'root' })
        # A legacy hub: an Idea Card under <n>/1 (or next to it) with many Links, written before Hub Cards.
        $legacy = @($cards | Where-Object { $_.type -eq 'idea' -and ($_.parent -eq "$tree/1" -or $_.address -eq "$tree/2") -and @($_.links).Count -ge 8 -and $_.excerpt -match '枢纽|核心卡片|清单' } | ForEach-Object { $_.address })
        $content = @($cards | Where-Object { $_.type -ne 'hub' -and $_.address -notin $legacy })
        $hubs = @($cards | Where-Object { $_.type -eq 'hub' } | ForEach-Object { [ordered]@{ address = $_.address; level = $_.level; title = $_.title } })
        if ($hubs | Where-Object { $_.level -eq 'register' }) { $survey.register = @($hubs | Where-Object { $_.level -eq 'register' })[0].address; continue }
        if (-not $content.Count) { continue }
        $branches = @()
        foreach ($b in @($content | Where-Object { $_.type -eq 'idea' -and $_.edge -eq 'side' })) {
            $inside = @($content | Where-Object { $_.address -eq $b.address -or $_.address.StartsWith("$($b.address)|") })
            $models = @($inside | Where-Object { $_.type -eq 'model' -and -not $_.facet }).Count
            $branchSizes += $inside.Count
            if ($inside.Count -ge 3 -or $models -ge 2) { $branches += [ordered]@{ anchor = $b.address; cards = $inside.Count; models = $models; title = $b.title } }
        }
        $neighbours = @()
        foreach ($key in @($pair.Keys | Where-Object { $_.StartsWith("$tree>") })) {
            $other = $key.Substring($tree.Length + 1)
            $cardsThere = @($touched[$key].GetEnumerator() | Sort-Object { - $_.Value } | Select-Object -First 3 | ForEach-Object { [ordered]@{ address = $_.Key; title = $titles[$_.Key] } })
            $neighbours += [ordered]@{ tree = $other; links = [int]$pair[$key]; cards = $cardsThere }
        }
        $neighbours = @($neighbours | Sort-Object { - $_.links } | Select-Object -First 8)
        foreach ($n in $neighbours) { if ([string]$n.tree -match '^\d+$' -and [int]$n.tree -gt [int]$tree) { $pairCounts += $n.links } }
        $root = @($t.cards | Where-Object { $_.edge -eq 'root' })[0]
        $treeSizes += $content.Count
        $hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($utf8.GetBytes((@($content | ForEach-Object { "$($_.address)`t$($_.title)" }) -join "`n")))).Substring(0, 16)
        $survey.trees[$tree] = [ordered]@{
            tree = $tree; title = $t.title; excerpt = $root.excerpt; cards = $content.Count
            models = @($content | Where-Object { $_.type -eq 'model' -and -not $_.facet }).Count
            hubs = $hubs; legacy = $legacy; branches = $branches; neighbours = $neighbours; hash = $hash
        }
    }
    Save-Text (Join-Path $Work 'survey.json') ($survey | ConvertTo-Json -Depth 12)
    $md = [Text.StringBuilder]::new()
    [void]$md.AppendLine('# 代理工作区的主题（每行一个主题）').AppendLine()
    foreach ($s in $survey.trees.Values) {
        $near = (@($s.neighbours | Select-Object -First 4 | ForEach-Object { "$($_.tree)($($_.links))" }) -join ' ')
        [void]$md.AppendLine("- $($s.tree) $($s.title)（$($s.cards) 张，模型 $($s.models)）：$($s.excerpt) | 邻近：$near")
    }
    Save-Text (Join-Path $Work 'survey.md') $md.ToString()
    Out-Last ([ordered]@{
            topics = $survey.trees.Count
            tree_cards = Get-Quantiles $treeSizes
            branch_cards = Get-Quantiles $branchSizes
            neighbour_links = Get-Quantiles $pairCounts
            hubs = $graph.counts
            legacy = @($survey.trees.Values | ForEach-Object { $_.legacy } | Where-Object { $_ }).Count
            register = $survey.register
            survey = (Join-Path $Work 'survey.json')
        })
}

# --- plan --------------------------------------------------------------------------------------------
function Get-Clamped($value, $default, $min, $max) {
    $v = if ($null -eq $value -or "$value" -eq '') { $default } else { [int]$value }
    [Math]::Max($min, [Math]::Min($max, $v))
}
function Step-Plan {
    $given = if ($Limits) { $Limits | ConvertFrom-Json -AsHashtable } else { @{} }
    $limits = [ordered]@{
        topic_min = Get-Clamped $given.topic_min 7 4 30
        line_min = Get-Clamped $given.line_min 12 5 40
        max_lines = Get-Clamped $given.max_lines 25 12 30
        max_groups = Get-Clamped $given.max_groups 7 3 9
        min_reason = Get-Clamped $given.min_reason 8 4 30
        neighbour_links = Get-Clamped $given.neighbour_links 3 2 20
        field_min = Get-Clamped $given.field_min 3 2 8
        field_max = Get-Clamped $given.field_max 12 4 25
        per_job = Get-Clamped ($(if ($PerJob) { $PerJob } else { $given.per_job })) 5 1 12
    }
    Save-Text (Join-Path $Work 'limits.json') ($limits | ConvertTo-Json)
    # A new plan starts clean: the jobs and DeepSeek files of an earlier run are not applied again.
    $State.jobs = [ordered]@{}
    Get-ChildItem -LiteralPath $Out -File -ErrorAction SilentlyContinue | Remove-Item -Force
    Get-ChildItem -LiteralPath $Work -File -Filter 'job-*.md' -ErrorAction SilentlyContinue | Remove-Item -Force
    Get-ChildItem -LiteralPath $Work -File -Filter 'fix-*.md' -ErrorAction SilentlyContinue | Remove-Item -Force
    $survey = Read-Json (Join-Path $Work 'survey.json')
    $graph = Invoke-Zt @(@{ tool = 'list_hubs'; arguments = (Get-LimitArgs $limits) + @{ workspace = 'agent' } })
    $faulty = @{}
    foreach ($h in @($graph.hubs)) { if (@($h.faults).Count -or $h.mirror) { $faulty[[string]$h.tree] = $true } }
    $missing = @{}
    foreach ($m in @($graph.missing)) { if ($m.tree) { $missing[[string]$m.tree] = @($missing[[string]$m.tree]) + , $m } }
    $only = @($Trees -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    $picked = @()
    foreach ($s in $survey.trees.Values) {
        $tree = [string]$s.tree
        if ($only.Count -and $tree -notin $only) { continue }
        $needs = $s.cards -ge $limits.topic_min -or @($s.branches | Where-Object { $_.cards -ge $limits.line_min -or $_.models -ge 2 }).Count
        if (-not $needs) { continue }
        $changed = -not $State.trees.Contains($tree) -or $State.trees[$tree].hash -ne $s.hash
        if ($Rebuild -or $only.Count -or $missing.ContainsKey($tree) -or $faulty.ContainsKey($tree) -or @($s.legacy).Count -or $changed) { $picked += $s }
    }
    # The cards of each picked tree, for its job file.
    $treeData = @{}
    if ($picked.Count) {
        $read = @(Invoke-Zt @($picked | ForEach-Object { @{ tool = 'get_tree'; arguments = @{ tree = [string]$_.tree } } }))
        foreach ($t in $read) { $treeData[[string]$t.tree] = $t }
    }
    $jobs = @()
    for ($i = 0; $i -lt $picked.Count; $i += $limits.per_job) {
        $chunk = @($picked[$i..([Math]::Min($picked.Count, $i + $limits.per_job) - 1)])
        $State.seq++
        $id = 't{0:d2}' -f $State.seq
        $file = Join-Path $Work "job-$id.md"
        $text = [Text.StringBuilder]::new()
        [void]$text.AppendLine("# 任务 $id：主题枢纽与线索枢纽").AppendLine()
        [void]$text.AppendLine("HELPER: $Helper").AppendLine("ALLOW: $(Join-Path $Work 'none.allow.txt')").AppendLine("OUT: $Out").AppendLine("LIMITS: $(($limits | ConvertTo-Json -Compress))").AppendLine()
        foreach ($s in $chunk) {
            $tree = [string]$s.tree
            $t = $treeData[$tree]
            [void]$text.AppendLine("## 主题 $tree $($s.title)").AppendLine()
            [void]$text.AppendLine("输出文件：$(Join-Path $Out "hub-$tree.json")").AppendLine("Topic 卡 [[$tree/0]]：$($s.excerpt)").AppendLine()
            $existing = @($s.hubs | Where-Object { $_.level -eq 'topic' }) + @($s.legacy | ForEach-Object { @{ address = $_; level = 'legacy' } })
            if ($existing.Count) { [void]$text.AppendLine("现有主题枢纽（改写它，不要另写）：$((@($existing | ForEach-Object { "[[$($_.address)]]" }) -join '、'))") }
            $required = @($s.branches | Where-Object { $_.cards -ge $limits.line_min -or $_.models -ge 2 })
            if ($required.Count) {
                [void]$text.AppendLine('必须写线索枢纽的分支：')
                foreach ($b in $required) { [void]$text.AppendLine("- [[$($b.anchor)]] $($b.title)（$($b.cards) 张，模型 $($b.models)）") }
            }
            [void]$text.AppendLine().AppendLine('本树卡片（地址 | 类型 | 标题 | 摘要）：')
            foreach ($c in @($t.cards | Where-Object { $_.edge -ne 'root' -and $_.type -ne 'hub' -and -not ($_.address -in @($s.legacy)) })) {
                $type = if ($c.type -eq 'model' -and $c.facet) { "模型部件·$($c.facet)" } elseif ($c.type -eq 'model') { '模型卡' } elseif ($c.type -eq 'event') { '事件卡' } else { '观点卡' }
                $excerpt = [string]$c.excerpt; if ($excerpt.Length -gt 90) { $excerpt = $excerpt.Substring(0, 90) + '…' }
                [void]$text.AppendLine("- $($c.address) | $type | $($c.title) | $excerpt")
            }
            if (@($s.neighbours).Count) {
                [void]$text.AppendLine().AppendLine('邻近主题（链接数）：')
                foreach ($n in @($s.neighbours | Where-Object { $_.links -ge $limits.neighbour_links } | Select-Object -First 5)) {
                    $title = if ($survey.trees.Contains([string]$n.tree)) { $survey.trees[[string]$n.tree].title } else { '' }
                    $touchedCards = (@($n.cards | ForEach-Object { "[[$($_.address)]] $($_.title)" }) -join '；')
                    [void]$text.AppendLine("- $($n.tree) $title（$($n.links) 条）：$touchedCards")
                }
            }
            [void]$text.AppendLine()
        }
        Save-Text $file $text.ToString()
        $State.jobs[$id] = [ordered]@{ kind = 'topic'; trees = @($chunk | ForEach-Object { [string]$_.tree }); file = $file }
        $jobs += [ordered]@{ id = $id; files = @($Doc, $file); prompt = '按以上说明完成本任务的每个主题：写出它的枢纽 JSON 文件。最后只回复一行 JSON。' }
    }
    Save-Text (Join-Path $Work 'none.allow.txt') "`n"
    # One field job: every Topic of the Workspace, on one line each.
    if (-not $only.Count -or $Rebuild) {
        $State.seq++
        $id = 'f{0:d2}' -f $State.seq
        $file = Join-Path $Work "job-$id.md"
        $fieldsNow = if ($State.fields.Count) { "现有领域枢纽（保留它们的名字，可以增删成员）：$((@($State.fields.Keys) -join '、'))`n" } else { '' }
        Save-Text $file ("# 任务 ${id}：领域枢纽与索引`n`nOUT: $(Join-Path $Out 'fields.json')`nLIMITS: $(($limits | ConvertTo-Json -Compress))`n$fieldsNow`n主题清单：$(Join-Path $Work 'survey.md')`n")
        $State.jobs[$id] = [ordered]@{ kind = 'field'; file = $file }
        $jobs += [ordered]@{ id = $id; files = @($Doc, $file); prompt = '按以上说明读主题清单，写出领域与索引的 JSON 文件。最后只回复一行 JSON。' }
    }
    Save-State
    Out-Last ([ordered]@{ tree_count = $picked.Count; jobs = $jobs; trees = @($picked | ForEach-Object { [string]$_.tree }); limits = $limits })
}

# --- apply -------------------------------------------------------------------------------------------
$Cards = $null
function Get-Exists($address) {
    if (-not $script:Cards) {
        $script:Cards = @{}
        $trees = @((Invoke-Zt @(@{ tool = 'list_trees'; arguments = @{ workspace = 'agent' } })).trees)
        foreach ($t in @(Invoke-Zt @($trees | ForEach-Object { @{ tool = 'get_tree'; arguments = @{ tree = $_.tree } } }))) {
            foreach ($c in $t.cards) { $script:Cards[$c.address] = $c }
        }
    }
    $script:Cards.ContainsKey($address)
}
function Format-Item($ref, $reason, $resolve, [string]$kind = '') {
    $address = & $resolve $ref
    if (-not $address) { return $null }
    # A small Topic has no topic hub, so `tree:<n>` may resolve to its root, which is not a hub.
    $isHub = $ref -match '^(line|field):' -or ($ref -match '^tree:' -and $address -notmatch '/0$')
    $words = if ($kind -and $Kinds.ContainsKey($kind)) { "$kind " } else { '' }
    "- $words[[$address]]$(if ($isHub) { '（枢纽）' } else { '' })：$([string]$reason)"
}
function Format-Body($scope, $groups, $neighbours, $resolve, [ref]$dropped) {
    $out = [Text.StringBuilder]::new()
    if ($scope) { [void]$out.AppendLine("范围：$scope").AppendLine() }
    foreach ($g in @($groups)) {
        $lines = @()
        foreach ($item in @($g.items)) {
            $line = Format-Item ([string]$item.ref) $item.reason $resolve
            if ($line) { $lines += $line } else { $dropped.Value += [string]$item.ref }
        }
        if ($lines.Count) { [void]$out.AppendLine("## $($g.name)"); foreach ($l in $lines) { [void]$out.AppendLine($l) }; [void]$out.AppendLine() }
    }
    $near = @()
    foreach ($item in @($neighbours)) {
        $line = Format-Item ([string]$item.ref) $item.reason $resolve ([string]$item.kind)
        if ($line) { $near += $line } else { $dropped.Value += [string]$item.ref }
    }
    if ($near.Count) { [void]$out.AppendLine('## 邻近'); foreach ($l in $near) { [void]$out.AppendLine($l) } }
    $out.ToString().TrimEnd()
}
function Step-Apply {
    $survey = Read-Json (Join-Path $Work 'survey.json')
    $created = 0; $updated = 0; $dropped = @()
    $topicHub = @{}
    foreach ($tree in @($State.trees.Keys)) { if ($State.trees[$tree].topic) { $topicHub[$tree] = $State.trees[$tree].topic } }
    foreach ($s in $survey.trees.Values) { foreach ($h in @($s.hubs | Where-Object { $_.level -eq 'topic' })) { $topicHub[[string]$s.tree] = $h.address } }
    $files = @(Get-ChildItem -LiteralPath $Out -Filter 'hub-*.json' -ErrorAction SilentlyContinue)
    foreach ($f in $files) {
        try { $plan = Read-Json $f.FullName } catch { $dropped += "$($f.Name): not JSON"; continue }
        $tree = [string]$plan.tree
        if (-not $survey.trees.Contains($tree)) { $dropped += "$($f.Name): unknown tree $tree"; continue }
        $s = $survey.trees[$tree]
        $entry = if ($State.trees.Contains($tree)) { $State.trees[$tree] } else { [ordered]@{ topic = $null; lines = [ordered]@{}; hash = '' } }
        if (-not $entry.lines) { $entry.lines = [ordered]@{} }
        # 1. Line hubs: one Side successor of each anchor.
        $creates = @(); $pending = @()
        foreach ($line in @($plan.lines)) {
            $anchor = [string]$line.anchor
            if (-not (Get-Exists $anchor)) { $dropped += "line:$anchor"; continue }
            if (-not $entry.lines.Contains($anchor)) {
                $creates += @{ tool = 'create_successor'; arguments = @{ at = $anchor; kind = 'side'; type = 'hub'; level = 'line'; title = [string]$line.title; body = '范围：待写。' } }
                $pending += $anchor
            }
        }
        $made = Invoke-Write $creates
        for ($i = 0; $i -lt $pending.Count; $i++) { $entry.lines[$pending[$i]] = $made[$i]; $script:Cards[$made[$i]] = @{ address = $made[$i] }; $created++ }
        # 2. The topic hub: rewrite the existing or legacy one, or create it under <n>/1.
        $topic = if ($entry.topic -and (Get-Exists $entry.topic)) { $entry.topic } elseif ($topicHub[$tree]) { $topicHub[$tree] } elseif (@($s.legacy).Count) { @($s.legacy)[0] } else { $null }
        if (-not $topic) {
            $topic = (Invoke-Write @(@{ tool = 'create_successor'; arguments = @{ at = "$tree/1"; kind = 'side'; type = 'hub'; level = 'topic'; title = [string]$plan.topic.title; body = '范围：待写。' } }))[0]
            $script:Cards[$topic] = @{ address = $topic }
            $created++
        }
        $entry.topic = $topic
        $topicHub[$tree] = $topic
        $resolve = {
            param($ref)
            if ($ref -match '^line:(.+)$') { return $entry.lines[$Matches[1]] }
            if ($ref -match '^tree:(.+)$') { $t = $Matches[1]; if ($topicHub[$t]) { return $topicHub[$t] } ; if (Get-Exists "$t/0") { return "$t/0" } ; return $null }
            if (Get-Exists $ref) { return $ref }
            $null
        }.GetNewClosure()
        $edits = @()
        foreach ($line in @($plan.lines)) {
            $anchor = [string]$line.anchor
            if (-not $entry.lines.Contains($anchor)) { continue }
            $d = @(); $body = Format-Body ([string]$line.scope) $line.groups $line.neighbours $resolve ([ref]$d); $dropped += $d
            $edits += @{ tool = 'edit_card'; arguments = @{ at = $entry.lines[$anchor]; title = [string]$line.title; body = $body; type = 'hub'; level = 'line' } }
        }
        $d = @(); $body = Format-Body ([string]$plan.topic.scope) $plan.topic.groups $plan.topic.neighbours $resolve ([ref]$d); $dropped += $d
        # Every line hub of the tree is reached from the topic hub (H9), directly or through the line hub
        # of an outer branch; the script adds one group for those the writer left out.
        $lineBodies = (@($edits | ForEach-Object { $_.arguments.body }) -join "`n")
        $left = @($entry.lines.Keys | Where-Object {
                $hub = $entry.lines[$_]
                $body -notmatch [regex]::Escape("[[$hub]]") -and $lineBodies -notmatch [regex]::Escape("[[$hub]]")
            })
        if ($left.Count) {
            $body += "`n`n## 线索`n" + ((@($left | ForEach-Object { "- [[$($entry.lines[$_])]]（枢纽）：从 [[$_]] 起的一支。" })) -join "`n")
        }
        $edits += @{ tool = 'edit_card'; arguments = @{ at = $topic; title = [string]$plan.topic.title; body = $body; type = 'hub'; level = 'topic' } }
        $updated += $edits.Count
        $null = Invoke-Write $edits
        # 3. The Topic body links its topic hub (H7).
        $root = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = "$tree/0" } }))
        if ($root.body -notmatch [regex]::Escape("[[$topic]]")) {
            $null = Invoke-Write @(@{ tool = 'edit_card'; arguments = @{ at = "$tree/0"; body = ($root.body.TrimEnd() + "`n`n本主题的枢纽卡：[[$topic]]。") } })
        }
        $entry.hash = $s.hash
        $State.trees[$tree] = $entry
        Save-State
    }
    # 4. Fields and the register, in the Topic 索引.
    $fieldsFile = Join-Path $Out 'fields.json'
    $register = if ($State.register -and (Get-Exists $State.register)) { $State.register } elseif ($survey.register) { $survey.register } else { $null }
    if (-not $register -and ($files.Count -or (Test-Path -LiteralPath $fieldsFile))) {
        $index = (Invoke-Write @(@{ tool = 'create_topic'; arguments = @{ title = '索引'; body = '本主题收放代理工作区的入口：索引卡与领域枢纽。' } }))[0]
        $register = (Invoke-Write @(@{ tool = 'create_successor'; arguments = @{ at = $index; type = 'hub'; level = 'register'; title = '索引：代理工作区的入口'; body = '范围：待写。' } }))[0]
        $script:Cards[$register] = @{ address = $register }
        $created += 2
    }
    $State.register = $register
    $fields = if (Test-Path -LiteralPath $fieldsFile) { try { Read-Json $fieldsFile } catch { $dropped += 'fields.json: not JSON'; @{} } } else { @{} }
    $inField = @{}
    foreach ($field in @($fields.fields | Where-Object { $_ })) {
        $name = [string]$field.name
        $members = @($field.groups | ForEach-Object { @($_.items) } | Where-Object { $topicHub[[string]$_.tree] -or (Get-Exists "$([string]$_.tree)/0") })
        if ($members.Count -lt 2) { $dropped += "field:$name (fewer than 2 Topics)"; continue }
        if (-not ($State.fields.Contains($name) -and (Get-Exists $State.fields[$name]))) {
            $State.fields[$name] = (Invoke-Write @(@{ tool = 'create_successor'; arguments = @{ at = $register; kind = 'side'; type = 'hub'; level = 'field'; title = [string]$field.title; body = '范围：待写。' } }))[0]
            $script:Cards[$State.fields[$name]] = @{ address = $State.fields[$name] }
            $created++
        }
        foreach ($m in $members) { $inField[[string]$m.tree] = $true }
    }
    $resolveField = {
        param($ref)
        if ($ref -match '^tree:(.+)$') { $t = $Matches[1]; if ($topicHub[$t]) { return $topicHub[$t] }; if (Get-Exists "$t/0") { return "$t/0" }; return $null }
        if ($ref -match '^field:(.+)$') { return $State.fields[$Matches[1]] }
        if (Get-Exists $ref) { return $ref }
        $null
    }.GetNewClosure()
    $edits = @()
    foreach ($field in @($fields.fields | Where-Object { $_ })) {
        $name = [string]$field.name
        if (-not $State.fields.Contains($name)) { continue }
        $groups = @($field.groups | ForEach-Object { @{ name = $_.name; items = @($_.items | ForEach-Object { @{ ref = "tree:$($_.tree)"; reason = $_.reason } }) } })
        $d = @(); $body = Format-Body ([string]$field.scope) $groups @() $resolveField ([ref]$d); $dropped += $d
        $edits += @{ tool = 'edit_card'; arguments = @{ at = $State.fields[$name]; title = [string]$field.title; body = $body } }
    }
    if ($register) {
        $out = [Text.StringBuilder]::new()
        [void]$out.AppendLine('范围：代理工作区的全部入口。每个关键词指向一到三个枢纽。').AppendLine()
        if ($State.fields.Count) {
            [void]$out.AppendLine('## 领域')
            foreach ($name in $State.fields.Keys) { [void]$out.AppendLine("- ${name}：[[$($State.fields[$name])]]") }
            [void]$out.AppendLine()
        }
        $terms = @()
        foreach ($t in @($fields.register | Where-Object { $_ })) {
            $targets = @($t.refs | ForEach-Object { & $resolveField ([string]$_) } | Where-Object { $_ } | Select-Object -Unique -First 3)
            if ($targets.Count) { $terms += "- $($t.term)：$((@($targets | ForEach-Object { "[[$_]]" }) -join '、'))" }
        }
        if ($terms.Count) { [void]$out.AppendLine('## 关键词'); foreach ($l in $terms) { [void]$out.AppendLine($l) }; [void]$out.AppendLine() }
        $loose = @($topicHub.Keys | Where-Object { -not $inField[$_] } | Sort-Object { [int]$_ })
        if ($loose.Count) {
            [void]$out.AppendLine('## 未归入领域的主题')
            foreach ($t in $loose) { [void]$out.AppendLine("- $($survey.trees[$t].title)：[[$($topicHub[$t])]]") }
        }
        $edits += @{ tool = 'edit_card'; arguments = @{ at = $register; body = $out.ToString().TrimEnd() } }
    }
    $null = Invoke-Write $edits
    $updated += $edits.Count
    Save-State
    Out-Last ([ordered]@{ created = $created; updated = $updated; trees = $State.trees.Count; fields = $State.fields.Count; register = $register; dropped = @($dropped | Select-Object -First 40) })
}

# --- check -------------------------------------------------------------------------------------------
function Step-Check {
    $limits = Read-Limits
    $hubs = @()
    foreach ($tree in $State.trees.Keys) {
        $e = $State.trees[$tree]
        if ($e.topic) { $hubs += @{ address = $e.topic; tree = $tree } }
        foreach ($a in $e.lines.Values) { $hubs += @{ address = $a; tree = $tree } }
    }
    foreach ($a in $State.fields.Values) { $hubs += @{ address = $a; tree = 'field' } }
    if ($State.register) { $hubs += @{ address = $State.register; tree = 'field' } }
    $args = Get-LimitArgs $limits
    $reports = if ($hubs.Count) { @(Invoke-Zt @($hubs | ForEach-Object { @{ tool = 'get_hub'; arguments = $args + @{ at = $_.address } } })) } else { @() }
    $byTree = [ordered]@{}
    $total = 0
    for ($i = 0; $i -lt $hubs.Count; $i++) {
        # A missing upper hub is the field job's or the register's work, not the writer's.
        $faults = @($reports[$i].faults | Where-Object { -not ($_.fault -eq 'unanchored' -and $reports[$i].level -eq 'topic' -and $_.detail -notmatch 'Topic body') })
        if ($reports[$i].mirror) { $faults += @{ fault = 'mirror'; detail = '只按地址顺序抄了本树，没有按主题分组，也没有树外的卡' } }
        if (-not $faults.Count) { continue }
        $total += $faults.Count
        $key = [string]$hubs[$i].tree
        if (-not $byTree.Contains($key)) { $byTree[$key] = @() }
        $byTree[$key] += "- [[$($hubs[$i].address)]]（$($reports[$i].level)）：$((@($faults | ForEach-Object { "$($_.fault)：$($_.detail)" }) -join '；'))"
    }
    $jobs = @()
    foreach ($id in @($State.jobs.Keys)) {
        $job = $State.jobs[$id]
        $keys = if ($job.kind -eq 'topic') { @($job.trees) } else { @('field') }
        $lines = @($keys | Where-Object { $byTree.Contains($_) } | ForEach-Object { $byTree[$_] })
        if (-not $lines.Count) { continue }
        $file = Join-Path $Work "fix-$id-$($State.seq).md"
        Save-Text $file ("# 修正 $id`n`nZT 的 get_hub 报告了下列问题。按说明修改对应的 JSON 文件（只改有问题的部分），再回复一行 JSON。`n`n$($lines -join "`n")`n")
        $jobs += [ordered]@{ id = "$id-fix"; continue = $id; files = @($file); fallback = @($Doc, $job.file, $file); prompt = '按以上问题修正 JSON 文件。最后只回复一行 JSON。' }
    }
    Save-State
    Out-Last ([ordered]@{ jobs = $jobs; hubs = $hubs.Count; faults = $total })
}

# --- report ------------------------------------------------------------------------------------------
function Step-Report {
    $limits = Read-Limits
    $graph = Invoke-Zt @(@{ tool = 'list_hubs'; arguments = (Get-LimitArgs $limits) + @{ workspace = 'agent' } })
    $faults = @{}
    foreach ($h in @($graph.hubs)) { foreach ($f in @($h.faults)) { $faults[$f] = 1 + [int]$faults[$f] } }
    $report = [ordered]@{
        counts = $graph.counts; faults = $faults; mirror = @($graph.hubs | Where-Object { $_.mirror }).Count
        missing = @($graph.missing | Select-Object -First 30); missing_count = @($graph.missing).Count
        register = $State.register; fields = $State.fields; limits = $limits
    }
    Save-Text (Join-Path $Work 'report.json') ($report | ConvertTo-Json -Depth 10)
    Out-Last $report
}

switch ($Step) {
    'survey' { Step-Survey }
    'plan' { Step-Plan }
    'apply' { Step-Apply }
    'check' { Step-Check }
    'report' { Step-Report }
}
