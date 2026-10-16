#Requires -Version 7
<#
.SYNOPSIS
  The empirical-results layer of the economics digest (docs/zt-digest-framework.md, docs/finding-cards-design.md):
  the Finding Cards of a book, one per empirical result, written after the digest. A compiler cycle: the script
  plans, DeepSeek writes a JSON file per job and checks it with the precheck step, the script writes the Cards.
  Needs zt 0.20.0 (the Card type finding) and a zt-check with the F rules.

.DESCRIPTION
  -Staging is the digest's staging folder (manifest.json with each group's section files, the owned-gNN.txt
  logs of the digest runs, sections\). -Work (default <Staging>\findings) holds the job files, out\<job>.json
  (DeepSeek's answers), state.json, and created.txt. Every step prints compact JSON last.

    plan      finds the result candidates of each group: the lines of its sections that name a number and a
              cue of empirical work (数据, 样本, 调查, 访谈, 回归, 实验, 统计, 显著, 系数, %, …), at most
              -MaxCandidates per group; lists each group's own Idea Cards and anchor; writes one job file per
              -GroupsPerJob groups with its SELF-CHECK line. Groups with no candidate and jobs already applied
              are left out. Prints {"candidates","groups","jobs":[{"id","files","prompt"}]}.
    precheck  read-only: checks out\<Job>.json as apply would (fields, supports, the role Link) and runs zt-check
              on the Finding Cards as drafts (F1, F2, F3, the Card language). One line per fault, then faults: N.
    apply     for each answered job not yet applied: refuses a finding with a fault (the precheck's faults,
              zt-check included), creates the Finding Card (type finding) as a Side successor of its group's
              anchor, and links it from every supported Idea Card ("印证 [[finding]]：title。"). Without -Apply it
              only validates.
    report    the counts of state.json.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('plan', 'precheck', 'apply', 'report')][string]$Step,
    [Parameter(Mandatory)][string]$Staging,
    [string]$Work = '',
    [string]$Job = '',
    [int]$GroupsPerJob = 3,
    [int]$MaxCandidates = 16,
    [switch]$Apply
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
$Doc = Join-Path $PSScriptRoot 'dsh-finding-instructions.zh.md'
$manifest = Get-Content -LiteralPath (Join-Path $Staging 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
$key = [string]$manifest.citation_key
if (-not $Work) { $Work = Join-Path $Staging 'findings' }
$out = Join-Path $Work 'out'
New-Item -ItemType Directory -Force -Path $Work, $out | Out-Null
$stateFile = Join-Path $Work 'state.json'
$installedHelper = Join-Path $HOME '.codex\skills\zt\scripts\zt-mcp.ps1'
$fields = 'title', 'measure', 'data', 'method', 'result', 'limits', 'section', 'role'
# A cue of empirical work, and a number, in the same line.
$cue = '(数据|样本|调查|问卷|访谈|受访|回归|估计|系数|显著|实验|统计|观测|面板|普查|抽样|比例|百分之|[%％])'
function Out-Last($object) { $object | ConvertTo-Json -Depth 10 -Compress }
function Get-State {
    if (Test-Path -LiteralPath $stateFile) { return Get-Content -LiteralPath $stateFile -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable }
    @{ applied = @{}; refused = @() }
}
function Save-State($s) { [IO.File]::WriteAllText($stateFile, ($s | ConvertTo-Json -Depth 10), $utf8) }
function Get-Field($e, $name) { if ($e -is [Collections.IDictionary] -and $e.Contains($name)) { [string]$e[$name] } else { '' } }
function Get-Owned([int]$n) {
    $file = Join-Path $Staging ('owned-g{0:d2}.txt' -f $n)
    $list = @(if (Test-Path -LiteralPath $file) { Get-Content -LiteralPath $file -Encoding utf8 | ForEach-Object { $_.Trim() } | Where-Object { $_ } })
    @($list + "$key/$n" | Select-Object -Unique)
}
# The body of a Finding Card: the six labels the F1 rule checks (docs/finding-cards-design.md), then the role.
function Get-FindingBody($e) {
    $n = [int](Get-Field $e 'group')
    @("测量：$(Get-Field $e 'measure')", "数据：$(Get-Field $e 'data')", "方法：$(Get-Field $e 'method')", "结果：$(Get-Field $e 'result')",
        "局限：$(Get-Field $e 'limits')", "来源：[[$key/$n]] $(Get-Field $e 'section')", '', (Get-Field $e 'role')) -join "`n"
}
function Test-Findings($findings) {
    $faults = [Collections.Generic.List[string]]::new()
    $titles = @{}
    $i = 0
    foreach ($e in @($findings)) {
        $i++
        $id = "finding:$i"
        if ($e -isnot [Collections.IDictionary]) { $faults.Add("$id FD1 not an object"); continue }
        foreach ($f in $fields) { if (-not (Get-Field $e $f).Trim()) { $faults.Add("$id FD1 missing field: $f") } }
        foreach ($f in $fields) { if ((Get-Field $e $f) -match "`n") { $faults.Add("$id FD1 $f has a line break: one line per label") } }
        $n = [int](Get-Field $e 'group')
        if (-not @($manifest.groups | Where-Object { [int]$_.n -eq $n }).Count) { $faults.Add("$id FD2 no group $n in the book"); continue }
        $owned = Get-Owned $n
        $supports = @($e['supports'] | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ })
        if (-not $supports.Count) { $faults.Add("$id FD2 supports is empty") }
        foreach ($s in $supports) { if ($owned -notcontains $s) { $faults.Add("$id FD2 $s is not a Card of group $n") } }
        $role = Get-Field $e 'role'
        if ($supports.Count -and -not @($supports | Where-Object { $role.Contains("[[$_]]") }).Count) { $faults.Add("$id FD3 role links no Card of supports") }
        if ($role -and $role -notmatch '^这一结果(印证|支持|例示)') { $faults.Add("$id FD3 role does not start with 这一结果印证") }
        $t = (Get-Field $e 'title').Trim()
        if ($t -and $titles.ContainsKey($t)) { $faults.Add("$id FD4 the same title as finding:$($titles[$t])") } elseif ($t) { $titles[$t] = $i }
    }
    , $faults
}
# zt-check on the Finding Cards as drafts. A draft needs "category" (the Card layer rules apply only then) and
# "book" (F2 then wants a Link to the book or one of its anchors).
function Get-DraftFaults($findings, $tag) {
    $drafts = @()
    $i = 0
    foreach ($e in @($findings)) {
        $i++
        if ($e -is [Collections.IDictionary]) { $drafts += [ordered]@{ address = "finding:$i"; title = (Get-Field $e 'title'); body = (Get-FindingBody $e); type = 'finding'; category = 'finding'; book = $key } }
    }
    if (-not $drafts.Count) { return , @() }
    $draftFile = Join-Path $Work "drafts-$tag.json"
    [IO.File]::WriteAllText($draftFile, (ConvertTo-Json -InputObject @($drafts) -Depth 5), $utf8)
    , @(Invoke-ZtCheck -Json $draftFile | Where-Object { $_ -notmatch '^faults: \d+$' })
}
function Read-Answer($file) { @((Get-Content -LiteralPath $file -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable).findings) }

if ($Step -eq 'plan') {
    $state = Get-State
    $byGroup = [ordered]@{}
    $total = 0
    foreach ($g in @($manifest.groups)) {
        $list = @()
        foreach ($name in @($g.sections)) {
            $path = Join-Path $Staging "sections\$name"
            if (-not (Test-Path -LiteralPath $path)) { continue }
            $no = 0
            foreach ($line in Get-Content -LiteralPath $path -Encoding utf8) {
                $no++
                if ($list.Count -ge $MaxCandidates) { break }
                if ($line -notmatch '[㐀-鿿]' -or $line -match '^\s*(!\[|\||#)') { continue }
                if ($line -match $cue -and $line -match '\d') {
                    $clip = ($line -replace '\s+', ' ').Trim()
                    if ($clip.Length -gt 140) { $clip = $clip.Substring(0, 140) + '…' }
                    $list += [ordered]@{ src = "sections/${name}:$no"; text = $clip }
                }
            }
        }
        if ($list.Count) { $byGroup[[string][int]$g.n] = $list; $total += $list.Count }
    }
    $groups = @($byGroup.Keys | ForEach-Object { [int]$_ })
    $owned = @{}
    $all = @()
    foreach ($n in $groups) { $owned[$n] = Get-Owned $n; $all += $owned[$n] }
    $cards = @{}
    if ($all.Count) {
        $got = @(Invoke-Zt @($all | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } }))
        for ($i = 0; $i -lt $all.Count -and $i -lt $got.Count; $i++) { $cards[$all[$i]] = $got[$i] }
    }
    $docCopy = Join-Path $Work 'dsh-finding-instructions.zh.md'
    [IO.File]::WriteAllText($docCopy, (Get-Content -LiteralPath $Doc -Raw -Encoding utf8).Replace('{{KEY}}', $key).Replace('{{STAGING}}', $Staging).Replace('{{HELPER}}', $installedHelper), $utf8)
    $jobs = @()
    for ($j = 0; $j * $GroupsPerJob -lt $groups.Count; $j++) {
        $id = 'fd-{0:d2}' -f ($j + 1)
        if ($state.applied.ContainsKey($id)) { continue }
        $chunk = @($groups | Select-Object -Skip ($j * $GroupsPerJob) -First $GroupsPerJob)
        $sb = [Text.StringBuilder]::new()
        [void]$sb.AppendLine("# 实证结果任务 $id：《$($manifest.book_title)》").AppendLine()
        [void]$sb.AppendLine("把结果写入 ``$(Join-Path $out "$id.json")``。").AppendLine()
        [void]$sb.AppendLine("SELF-CHECK: pwsh -NoProfile -File `"$PSCommandPath`" -Step precheck -Staging `"$Staging`" -Work `"$Work`" -Job $id").AppendLine()
        foreach ($n in $chunk) {
            $g = @($manifest.groups | Where-Object { [int]$_.n -eq $n })[0]
            [void]$sb.AppendLine("## 第 $n 组：$($g.chapter)：$(@($g.headings) -join '；')").AppendLine()
            [void]$sb.AppendLine("锚点：``$key/$n``（结果卡放在它的侧后继上，来源行链接它）").AppendLine().AppendLine('本组的观点卡（supports 只能取这些地址）：').AppendLine()
            foreach ($a in $owned[$n]) {
                $c = $cards[$a]
                if ($c -and [string]$c.type -in @('model', 'event', 'hub', 'finding')) { continue }
                [void]$sb.AppendLine("- ``$a`` $(if ($c) { [string]$c.title } else { '' })")
            }
            [void]$sb.AppendLine().AppendLine('结果候选：').AppendLine()
            foreach ($c in $byGroup["$n"]) { [void]$sb.AppendLine("- ``$($c.src)`` $($c.text)") }
            [void]$sb.AppendLine()
        }
        $jobFile = Join-Path $Work "$id.md"
        [IO.File]::WriteAllText($jobFile, $sb.ToString(), $utf8)
        $jobs += [ordered]@{ id = $id; files = @($docCopy, $jobFile); prompt = '按以上说明写出这些组的实证结果卡 JSON 文件，运行 SELF-CHECK 改到 faults: 0。最后只回复一行 JSON。' }
    }
    Out-Last ([ordered]@{ candidates = $total; groups = $groups.Count; jobs = $jobs })
    return
}

if ($Step -eq 'precheck') {
    if (-not $Job) { throw 'precheck needs -Job' }
    $file = Join-Path $out "$Job.json"
    if (-not (Test-Path -LiteralPath $file)) { "$Job FD0 no answer file $file"; 'faults: 1'; return }
    try { $findings = Read-Answer $file } catch { "$Job FD0 the answer is not JSON: $($_.Exception.Message)"; 'faults: 1'; return }
    $faults = Test-Findings $findings
    $checked = Get-DraftFaults $findings $Job
    foreach ($f in $faults) { $f }
    foreach ($c in $checked) { $c }
    "faults: $($faults.Count + $checked.Count)"
    return
}

if ($Step -eq 'report') {
    $s = Get-State
    $jobs = @($s.applied.Values)
    Out-Last ([ordered]@{ jobs = $jobs.Count; created = (@($jobs | ForEach-Object { @($_.created) }) | Where-Object { $_ }).Count
            linked = [int](@($jobs | ForEach-Object { [int]$_.linked }) | Measure-Object -Sum).Sum
            refused = @($s.refused).Count; errors = @($jobs | Where-Object { $_.error } | ForEach-Object { "$($_.id): $($_.error)" }) })
    return
}

# --- apply -----------------------------------------------------------------------------------------
$state = Get-State
$summary = [ordered]@{ ok = $true; applied_jobs = 0; created = 0; linked = 0; refused = @(); pending_jobs = 0 }
foreach ($file in @(Get-ChildItem -LiteralPath $out -Filter 'fd-*.json' -File | Sort-Object Name)) {
    $id = $file.BaseName
    if ($state.applied.ContainsKey($id)) { continue }
    try { $findings = Read-Answer $file.FullName } catch { $summary.refused += "${id}: the answer is not JSON"; continue }
    # Refuse what the self-check would report: no unchecked text is written.
    $faults = @((Test-Findings $findings) | ForEach-Object { $_ }) + @((Get-DraftFaults $findings $id) | ForEach-Object { $_ })
    $bad = @{}
    foreach ($f in $faults) { if ($f -match '^finding:(\d+) ' -and -not $bad.ContainsKey([int]$Matches[1])) { $bad[[int]$Matches[1]] = $f } }
    foreach ($b in $bad.Values) { $summary.refused += "${id} $b" }
    if (-not $Apply) { continue }
    $calls = @()
    $entry = [ordered]@{ id = $id; created = @(); linked = 0; error = '' }
    $links = [ordered]@{}
    $i = 0
    foreach ($e in $findings) {
        $i++
        if ($bad.ContainsKey($i)) { continue }
        $n = [int](Get-Field $e 'group')
        $calls += @{ tool = 'create_successor'; arguments = @{ at = "$key/$n"; kind = 'side'; type = 'finding'; title = (Get-Field $e 'title').Trim(); body = (Get-FindingBody $e); src = (Get-Field $e 'src') } }
        $ref = "@@$($calls.Count - 1)"
        $title = (Get-Field $e 'title').Trim().TrimEnd('。')
        foreach ($s in @($e['supports'] | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ })) {
            if (-not $links.Contains($s)) { $links[$s] = @() }
            $links[$s] += "印证 [[$ref]]：$title。"
        }
    }
    if ($links.Count) {
        $ideas = @(Invoke-Zt @($links.Keys | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } }))
        $k = 0
        foreach ($s in @($links.Keys)) {
            $body = [string]$ideas[$k].body
            $k++
            $calls += @{ tool = 'edit_card'; arguments = @{ at = $s; body = ($body.TrimEnd() + "`n`n" + ($links[$s] -join "`n")) } }
            $entry.linked += $links[$s].Count
        }
    }
    if ($calls.Count) {
        $log = Join-Path $Work 'created.txt'
        $result = @(($calls | ConvertTo-Json -Depth 8 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Log $log)
        for ($c = 0; $c -lt $calls.Count; $c++) {
            $line = @($result | Where-Object { $_ -match "^$c ok " })
            if ($calls[$c].tool -eq 'create_successor' -and $line.Count) { $entry.created += ($line[0] -split ' ok ', 2)[1].Trim() }
        }
        $err = @($result | Where-Object { $_ -match '^\d+ error ' })
        if ($LASTEXITCODE -ne 0 -or $err.Count) { $entry.error = (@($err) + "exit $LASTEXITCODE")[0]; $summary.ok = $false }
    }
    $state.applied[$id] = $entry
    $state.refused = @(@($state.refused) + @($bad.Values | ForEach-Object { "$id $_" }))
    Save-State $state
    $summary.applied_jobs++
    $summary.created += $entry.created.Count
    $summary.linked += $entry.linked
}
Out-Last $summary
