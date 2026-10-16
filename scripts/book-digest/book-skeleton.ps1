#Requires -Version 7
<#
.SYNOPSIS
  The book's skeleton (docs/digest-card-layers-proposal.md, proposal 7): the Literature Card index,
  the structure Card, and the bodies of the book's Topics. The script builds each frame from the
  manifest and the trees; one DeepSeek job writes the short texts as JSON; Opus gives only the
  thesis (the structure Card's title).

.DESCRIPTION
  -Staging is the digest's staging folder; -Work is the skeleton folder (job.md, out\skeleton.json).
  -Audit names an audit.json of the book (organize-survey.ps1 writes one); without it the script
  runs digest-audit.ps1. Every step prints compact JSON as its last line.

    plan     writes job.md: one line per group (anchor, chapter, anchor title and body), the
             book's Model Cards with their levels, its Event Cards, and the Topics where the book
             holds most Cards (root body and Card titles). Prints the job.
    apply    reads out\skeleton.json, checks it (every anchor, model, event, and Topic it names
             belongs to the book; every Link exists), and with -Apply writes:
               - the structure Card (a Side successor of <key>/1; title = -Thesis, or the thesis
                 in -Decisions, or the title it has): the argument by group, the models, the events;
               - the Literature Card body: one index line per group, a line linking the structure
                 Card, and the document line (原书文档：…) it had;
               - each Topic body, keeping every line that links a Hub Card.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('plan', 'apply')][string]$Step,
    [Parameter(Mandatory)][string]$Staging,
    [Parameter(Mandatory)][string]$Work,
    [string]$Audit = '',
    [string]$Thesis = '',
    [string]$Decisions = '',
    [switch]$Apply
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
$Doc = Join-Path $PSScriptRoot 'dsh-skeleton-instructions.zh.md'
$manifest = Get-Content -LiteralPath (Join-Path $Staging 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
$key = [string]$manifest.citation_key
$title = [string]$manifest.book_title
$out = Join-Path $Work 'out'
New-Item -ItemType Directory -Force -Path $Work, $out | Out-Null
function Out-Last($object) { $object | ConvertTo-Json -Depth 10 -Compress }
$Marker = "本卡是《$title》的结构卡：按阅读顺序列出全书的论证、模型与事件。"

# --- what the book holds -------------------------------------------------------------------------
if (-not $Audit) {
    $Audit = Join-Path $Work 'audit\audit.json'
    $lines = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot 'digest-audit.ps1') -CitationKey $key -Out (Split-Path -Parent $Audit) 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "digest-audit failed: $($lines -join ' | ')" }
}
$auditJson = Get-Content -LiteralPath $Audit -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
$book = @($auditJson.books)[0]
$cards = @(Get-Content -LiteralPath (Join-Path (Split-Path -Parent $Audit) 'audit-cards.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable)
$known = [Collections.Generic.HashSet[string]]::new()
foreach ($a in Get-Content -LiteralPath (Join-Path (Split-Path -Parent $Audit) 'audit-addresses.txt') -Encoding utf8) { if ($a) { [void]$known.Add($a.Trim()) } }
$groups = @($manifest.groups)
$anchors = @($groups | ForEach-Object { "$key/$($_.n)" })
$models = @($book.models.list | Where-Object { $_.address })
$events = @($cards | Where-Object { $_.category -eq 'event' -and $_.book -eq $key })
$topicRows = @($book.topics.list | Where-Object { [int]$_.cards * 2 -gt [int]$_.size })

if ($Step -eq 'plan') {
    $read = @(Invoke-Zt @(@($anchors) + @($topicRows | ForEach-Object { "$($_.tree)/0" }) | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } }))
    $byAddress = @{}
    foreach ($c in $read) { $byAddress[[string]$c.address] = $c }
    $sb = [Text.StringBuilder]::new()
    [void]$sb.AppendLine("# 骨架任务：《$title》").AppendLine().AppendLine("把结果写入 ``$(Join-Path $out 'skeleton.json')``。").AppendLine()
    [void]$sb.AppendLine('## 各组（锚点卡）').AppendLine()
    foreach ($g in $groups) {
        $a = "$key/$($g.n)"
        $c = $byAddress[$a]
        $body = (([string]$c.body) -replace '\s+', ' ').Trim()
        if ($body.Length -gt 600) { $body = $body.Substring(0, 600) + '…' }
        [void]$sb.AppendLine("### ``$a``（$($g.chapter)：$(@($g.headings) -join '；')）").AppendLine().AppendLine("标题：$($c.title)").AppendLine().AppendLine("正文：$body").AppendLine()
    }
    [void]$sb.AppendLine('## 模型卡').AppendLine()
    if ($models.Count) { foreach ($m in $models) { [void]$sb.AppendLine("- ``$($m.address)``（$(if ($m.level -eq 'concept') { '概念模型' } elseif ($m.level -eq 'formal') { '形式模型' } else { '未定层级' })）$($m.title)") } } else { [void]$sb.AppendLine('（没有）') }
    [void]$sb.AppendLine().AppendLine('## 事件卡').AppendLine()
    if ($events.Count) {
        foreach ($e in $events) {
            $targets = @($e.links | Select-Object -First 6 | ForEach-Object { '`' + $_ + '`' }) -join '、'
            [void]$sb.AppendLine("- ``$($e.address)`` $($e.title)；它链接：$targets")
        }
    } else { [void]$sb.AppendLine('（没有）') }
    [void]$sb.AppendLine().AppendLine('## 本书为主的主题').AppendLine()
    foreach ($t in $topicRows) {
        $root = "$($t.tree)/0"
        $c = $byAddress[$root]
        [void]$sb.AppendLine("### ``$root`` $($t.title)（$($t.size) 张卡）").AppendLine().AppendLine("现在的正文：$((([string]$c.body) -replace '\s+', ' ').Trim())").AppendLine().AppendLine('树中的卡片：').AppendLine()
        foreach ($x in @($cards | Where-Object { [string]$_.tree -eq [string]$t.tree -and $_.category -in @('idea', 'event', 'model') } | Sort-Object { [string]$_.address } | Select-Object -First 12)) { [void]$sb.AppendLine("- ``$($x.address)`` $($x.title)") }
        [void]$sb.AppendLine()
    }
    # apply without -Apply checks the JSON and writes nothing: the writer's own check.
    [void]$sb.AppendLine("SELF-CHECK: ``pwsh -NoProfile -File `"$PSCommandPath`" -Step apply -Staging `"$Staging`" -Work `"$Work`"``（不要加 -Apply）")
    $job = Join-Path $Work 'job.md'
    [IO.File]::WriteAllText($job, $sb.ToString(), $utf8)
    $docCopy = Join-Path $Work (Split-Path -Leaf $Doc)
    [IO.File]::WriteAllText($docCopy, (Get-Content -LiteralPath $Doc -Raw -Encoding utf8).Replace('{{KEY}}', $key), $utf8)
    Out-Last ([ordered]@{ groups = $groups.Count; models = $models.Count; events = $events.Count; topics = $topicRows.Count; jobs = @([ordered]@{ id = 'skeleton'; files = @($docCopy, $job); prompt = '按以上说明写出骨架 JSON 文件。最后只回复一行 JSON。' }) })
    return
}

# --- apply ---------------------------------------------------------------------------------------
$file = Join-Path $out 'skeleton.json'
if (-not (Test-Path -LiteralPath $file)) { Out-Last ([ordered]@{ ok = $false; error = 'no skeleton.json' }); return }
$sk = Get-Content -LiteralPath $file -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
$problems = [Collections.Generic.List[string]]::new()
function Get-Links([string]$text) { @([regex]::Matches($text, '\[\[([^\]]+)\]\]') | ForEach-Object { $_.Groups[1].Value.Trim() }) }
function Test-Text([string]$where, [string]$text) {
    foreach ($l in Get-Links $text) { if (-not $known.Contains($l)) { $problems.Add("$($where): [[$l]] does not exist") } }
    if ($text -match '[；;]') { $problems.Add("$($where): a semicolon") }
}
$says = @{}
foreach ($c in @($sk.chapters)) { if ([string]$c.anchor -in $anchors -and ([string]$c.says).Trim()) { $says[[string]$c.anchor] = ([string]$c.says).Trim(); Test-Text $c.anchor $c.says } }
$argument = @()
foreach ($a in @($sk.argument)) {
    if ([string]$a.anchor -notin $anchors) { $problems.Add("argument: ``$($a.anchor)`` is no anchor"); continue }
    $keyCards = @(@($a.key_cards) | ForEach-Object { [string]$_ } | Where-Object { $known.Contains($_) } | Select-Object -First 3)
    Test-Text $a.anchor $a.adds
    $argument += [ordered]@{ anchor = [string]$a.anchor; adds = ([string]$a.adds).Trim(); key = $keyCards }
}
$modelSet = @{}
foreach ($m in $models) { $modelSet[[string]$m.address] = $m }
$modelLines = @()
foreach ($m in @($sk.models)) {
    if (-not $modelSet.ContainsKey([string]$m.address)) { $problems.Add("models: ``$($m.address)`` is no Model Card of the book"); continue }
    Test-Text $m.address $m.shows
    $level = if ($modelSet[[string]$m.address].level -eq 'concept') { '概念模型' } elseif ($modelSet[[string]$m.address].level -eq 'formal') { '形式模型' } else { '模型' }
    $later = @(@($m.built_on_by) + @($m.builds_on) | ForEach-Object { [string]$_ } | Where-Object { $modelSet.ContainsKey($_) } | Select-Object -Unique)
    $tail = if ($later.Count) { " 依赖它的模型：$(@($later | ForEach-Object { "[[$_]]" }) -join '、')。" } else { '' }
    $modelLines += "- [[$($m.address)]]（$level）：$(([string]$m.shows).Trim())$tail"
}
$eventSet = @{}
foreach ($e in $events) { $eventSet[[string]$e.address] = $true }
$eventLines = @()
foreach ($e in @($sk.events)) {
    if (-not $eventSet.ContainsKey([string]$e.address)) { $problems.Add("events: ``$($e.address)`` is no Event Card of the book"); continue }
    $supports = @(@($e.supports) | ForEach-Object { [string]$_ } | Where-Object { $known.Contains($_) } | Select-Object -First 3)
    $eventLines += "- [[$($e.address)]]：例示 $(if ($supports.Count) { @($supports | ForEach-Object { "[[$_]]" }) -join '、' } else { '本书的论证' }) 的观点。"
}
$topicSet = @{}
foreach ($t in $topicRows) { $topicSet["$($t.tree)/0"] = [string]$t.tree }
$topicBodies = [ordered]@{}
foreach ($t in @($sk.topics)) {
    $root = [string]$t.root
    if (-not $topicSet.ContainsKey($root)) { $problems.Add("topics: ``$root`` is no Topic of the book"); continue }
    $body = ([string]$t.body).Trim()
    $inTree = @(Get-Links $body | Where-Object { ($_ -split '/', 2)[0] -eq $topicSet[$root] })
    if (-not $inTree.Count) { $problems.Add("topics: ``$root`` links no entry Card of its tree"); continue }
    Test-Text $root $body
    $topicBodies[$root] = $body
}
$bad = @($problems | Where-Object { $_ -match 'does not exist|semicolon' })
if ($bad.Count) { Out-Last ([ordered]@{ ok = $false; problems = @($problems) }); return }

# The thesis: -Thesis, else decisions.json, else the title the structure Card has.
if (-not $Thesis -and $Decisions -and (Test-Path -LiteralPath $Decisions)) { $Thesis = [string](Get-Content -LiteralPath $Decisions -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable).thesis }
$tree = @(Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $key } }))[0]
$structure = @($tree.cards | Where-Object { [string]$_.excerpt -match ('^本卡是《.*》的结构卡') } | Select-Object -First 1)
# A structure Card of an older digest has no marker line: a Side successor of <key>/1 that links at
# least half of the anchors (and is no anchor, model, event, or term table).
if (-not $structure.Count) {
    $structure = @($tree.cards | Where-Object {
            [string]$_.parent -eq "$key/1" -and [string]$_.edge -eq 'side' -and $_.type -eq 'idea' -and [string]$_.title -notmatch '^术语表' -and
            @(@($_.links) | Where-Object { [string]$_ -in $anchors }).Count * 2 -ge $anchors.Count
        } | Select-Object -First 1)
}
if (-not $Thesis.Trim()) { $Thesis = if ($structure.Count) { [string]$structure[0].title } else { "《$title》的论证结构" } }
$sb = [Text.StringBuilder]::new()
[void]$sb.AppendLine($Marker).AppendLine().AppendLine('## 论证').AppendLine()
foreach ($a in ($argument | Sort-Object { [int](($_.anchor -split '/')[-1]) })) {
    $keys = if ($a.key.Count) { " 关键卡片：$(@($a.key | ForEach-Object { "[[$_]]" }) -join '、')。" } else { '' }
    [void]$sb.AppendLine("- [[$($a.anchor)]]：$($a.adds)$keys")
}
if ($modelLines.Count) { [void]$sb.AppendLine().AppendLine('## 模型').AppendLine(); foreach ($l in $modelLines) { [void]$sb.AppendLine($l) } }
if ($eventLines.Count) { [void]$sb.AppendLine().AppendLine('## 事件').AppendLine(); foreach ($l in $eventLines) { [void]$sb.AppendLine($l) } }
[void]$sb.AppendLine().AppendLine("来源：[[$key]]")
$structureBody = $sb.ToString().TrimEnd()
$written = [ordered]@{}
if ($Apply) {
    $call = if ($structure.Count) { @{ tool = 'edit_card'; arguments = @{ at = [string]$structure[0].address; title = $Thesis; body = $structureBody } } }
    else { @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = $Thesis; body = $structureBody } } }
    $lines = @((@($call) | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "zt: $($lines -join ' | ')" }
    $structureAt = (@($lines | Where-Object { $_ -match '^\d+ ok ' })[0] -split ' ok ', 2)[1].Trim()
    $written.structure = $structureAt
    # The Literature Card: the index, the structure Card, and the document line it had.
    $root = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $key } }))[0]
    $docs = @(([string]$root.body) -split "`n" | Where-Object { $_ -match '^\s*原书文档' })
    $index = [Text.StringBuilder]::new()
    [void]$index.AppendLine("《$title》的索引：每组一行，写出它讲了什么，再链接它的锚点卡。").AppendLine()
    foreach ($g in $groups) {
        $a = "$key/$($g.n)"
        $text = if ($says.ContainsKey($a)) { $says[$a] } else { @($g.headings) -join '；' }
        [void]$index.AppendLine("- 第 $($g.n) 组（$($g.chapter)）：$text → [[$a]]")
    }
    [void]$index.AppendLine().AppendLine("全书的论证结构见 [[$structureAt]]。")
    $terms = @($tree.cards | Where-Object { [string]$_.title -match '^术语表' } | Select-Object -First 1)
    if ($terms.Count) { [void]$index.AppendLine("本书的术语见 [[$($terms[0].address)]]。") }
    if ($docs.Count) { [void]$index.AppendLine().AppendLine(($docs -join "`n")) }
    $calls = @(@{ tool = 'edit_card'; arguments = @{ at = $key; body = $index.ToString().TrimEnd() } })
    # Topic bodies: keep every line that links a Hub Card of the tree.
    foreach ($r in @($topicBodies.Keys)) {
        $tc = @(Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $topicSet[$r] } }))[0]
        $hubs = @($tc.cards | Where-Object { $_.type -eq 'hub' } | ForEach-Object { [string]$_.address })
        $old = [string](@($tc.cards | Where-Object { [string]$_.address -eq $r })[0]).excerpt
        $rootCard = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $r } }))[0]
        $keepLines = @(([string]$rootCard.body) -split "`n" | Where-Object { $line = $_; @(Get-Links $line | Where-Object { $_ -in $hubs }).Count })
        $body = $topicBodies[$r] + $(if ($keepLines.Count) { "`n`n" + ($keepLines -join "`n") } else { '' })
        $calls += @{ tool = 'edit_card'; arguments = @{ at = $r; body = $body } }
    }
    $lines = @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "zt: $($lines -join ' | ')" }
    $written.literature = $key
    $written.topics = @($topicBodies.Keys)
}
Out-Last ([ordered]@{ ok = $true; thesis = $Thesis; argument = $argument.Count; models = $modelLines.Count; events = $eventLines.Count; topics = $topicBodies.Count; problems = @($problems); written = $written })
