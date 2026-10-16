#Requires -Version 7
<#
.SYNOPSIS
  The event layer of the economics digest (docs/zt-digest-framework.md): the Event Cards of a book, written
  after the digest runs (which no longer write them). A compiler cycle: the script plans, DeepSeek writes a
  JSON file per job and checks it with the precheck step, the script writes the Cards.

.DESCRIPTION
  -Staging is the digest's staging folder (manifest.json, events.json from prepare-book-digest.ps1, the
  owned-gNN.txt logs of the digest runs, sections\). -Work (default <Staging>\events) holds the job files,
  out\<job>.json (DeepSeek's answers), state.json, and created.txt. Every step prints compact JSON last.

    plan      groups the event candidates (the sentences of a group that name a year) by group, lists each
              group's own Idea Cards (address and title) and anchor, and writes one job file per
              -GroupsPerJob groups with its SELF-CHECK line. Jobs already applied are left out.
              Prints {"candidates","groups","jobs":[{"id","files","prompt"}]}.
    precheck  read-only: checks out\<Job>.json as apply would (fields, supports, role Link, existing Event
              Card) and runs zt-check on the new Event Cards as drafts. One line per fault, then faults: N.
    apply     for each answered job not yet applied: refuses an event with a fault (the precheck's faults,
              zt-check on the drafts included), adds the book's source
              and evidence role to an existing Event Card (existing), or creates the Event Card as a Side
              successor of its first supported Idea Card (type event), then links it from every supported
              Idea Card ("案例 [[event]]：title。"). Without -Apply it only validates.
    report    the counts of state.json.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('plan', 'precheck', 'apply', 'report')][string]$Step,
    [Parameter(Mandatory)][string]$Staging,
    [string]$Work = '',
    [string]$Job = '',
    [int]$GroupsPerJob = 3,
    [switch]$Apply
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
$Doc = Join-Path $PSScriptRoot 'dsh-event-instructions.zh.md'
$manifest = Get-Content -LiteralPath (Join-Path $Staging 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
$key = [string]$manifest.citation_key
if (-not $Work) { $Work = Join-Path $Staging 'events' }
$out = Join-Path $Work 'out'
New-Item -ItemType Directory -Force -Path $Work, $out | Out-Null
$stateFile = Join-Path $Work 'state.json'
$installedHelper = Join-Path $HOME '.codex\skills\zt\scripts\zt-mcp.ps1'
function Out-Last($object) { $object | ConvertTo-Json -Depth 10 -Compress }
function Get-State {
    if (Test-Path -LiteralPath $stateFile) { return Get-Content -LiteralPath $stateFile -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable }
    @{ applied = @{}; refused = @() }
}
function Save-State($s) { [IO.File]::WriteAllText($stateFile, ($s | ConvertTo-Json -Depth 10), $utf8) }
function Get-Field($e, $name) { if ($e -is [Collections.IDictionary] -and $e.Contains($name)) { [string]$e[$name] } else { '' } }

# A group's own Cards: the addresses its digest run logged, plus its anchor.
function Get-Owned([int]$n) {
    $file = Join-Path $Staging ('owned-g{0:d2}.txt' -f $n)
    $list = @(if (Test-Path -LiteralPath $file) { Get-Content -LiteralPath $file -Encoding utf8 | ForEach-Object { $_.Trim() } | Where-Object { $_ } })
    @($list + "$key/$n" | Select-Object -Unique)
}

# The body of a new Event Card: the fixed labels the E2 rule checks, then the evidence role.
function Get-EventBody($e) {
    @("时间：$(Get-Field $e 'time')", "地点：$(Get-Field $e 'place')", "参与者：$(Get-Field $e 'actors')",
        "经过：$(Get-Field $e 'course')", "来源：[[$key]] $(Get-Field $e 'section')", '', "证据作用：$(Get-Field $e 'role')") -join "`n"
}

# The faults of one answer file that apply would refuse: "<n> <rule> <text>" per fault (n: the event's place).
function Test-Events($events, [switch]$Lookup) {
    $faults = [Collections.Generic.List[string]]::new()
    $titles = @{}
    $i = 0
    foreach ($e in @($events)) {
        $i++
        $id = "event:$i"
        if ($e -isnot [Collections.IDictionary]) { $faults.Add("$id EV1 not an object"); continue }
        foreach ($f in 'title', 'time', 'place', 'actors', 'course', 'section', 'role') { if (-not (Get-Field $e $f).Trim()) { $faults.Add("$id EV1 missing field: $f") } }
        if ((Get-Field $e 'title') -match '[；\n]') { $faults.Add("$id EV1 the title has ； or a line break") }
        $n = [int](Get-Field $e 'group')
        if (-not @($manifest.groups | Where-Object { [int]$_.n -eq $n }).Count) { $faults.Add("$id EV2 no group $n in the book"); continue }
        $owned = Get-Owned $n
        $supports = @($e['supports'] | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ })
        if (-not $supports.Count) { $faults.Add("$id EV2 supports is empty") }
        foreach ($s in $supports) { if ($owned -notcontains $s) { $faults.Add("$id EV2 $s is not a Card of group $n") } }
        $role = Get-Field $e 'role'
        if ($supports.Count -and -not @($supports | Where-Object { $role.Contains("[[$_]]") }).Count) { $faults.Add("$id EV3 role links no Card of supports") }
        if ($role -and $role -notmatch '^本事件(例示|是|作为)') { $faults.Add("$id EV3 role does not start with 本事件例示") }
        $t = (Get-Field $e 'title').Trim()
        if ($t -and $titles.ContainsKey($t)) { $faults.Add("$id EV5 the same title as event:$($titles[$t])") } elseif ($t) { $titles[$t] = $i }
        $existing = (Get-Field $e 'existing').Trim()
        if ($existing -and $Lookup) {
            try { $card = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $existing } }))[0] } catch { $card = $null }
            if (-not $card) { $faults.Add("$id EV4 existing $existing is not a Card") }
            elseif ([string]$card.type -ne 'event') { $faults.Add("$id EV4 existing $existing is not an Event Card") }
        }
    }
    , $faults
}
# zt-check on the new Event Cards as drafts: the Card language rules and E1, E2 on what apply would write.
# A draft needs "category": zt-check applies the Card layer rules only to entries that carry one.
function Get-DraftFaults($events, $tag) {
    $drafts = @()
    $i = 0
    foreach ($e in @($events)) {
        $i++
        if ($e -is [Collections.IDictionary] -and -not (Get-Field $e 'existing').Trim()) {
            $drafts += [ordered]@{ address = "event:$i"; title = (Get-Field $e 'title'); body = (Get-EventBody $e); type = 'event'; category = 'event'; book = $key }
        }
    }
    if (-not $drafts.Count) { return , @() }
    $draftFile = Join-Path $Work "drafts-$tag.json"
    [IO.File]::WriteAllText($draftFile, (ConvertTo-Json -InputObject @($drafts) -Depth 5), $utf8)
    , @(Invoke-ZtCheck -Json $draftFile | Where-Object { $_ -notmatch '^faults: \d+$' })
}
function Read-Answer($file) {
    $raw = Get-Content -LiteralPath $file -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    @($raw.events)
}

if ($Step -eq 'plan') {
    $eventsFile = Join-Path $Staging 'events.json'
    $candidates = @(if (Test-Path -LiteralPath $eventsFile) { Get-Content -LiteralPath $eventsFile -Raw -Encoding utf8 | ConvertFrom-Json })
    $state = Get-State
    $byGroup = [ordered]@{}
    foreach ($c in $candidates) { $g = [string][int]$c.group; if (-not $byGroup.Contains($g)) { $byGroup[$g] = @() }; $byGroup[$g] += $c }
    $groups = @($byGroup.Keys | ForEach-Object { [int]$_ })
    # The titles of every group's own Cards, in one zt call.
    $owned = @{}
    $all = @()
    foreach ($n in $groups) { $owned[$n] = Get-Owned $n; $all += $owned[$n] }
    $cards = @{}
    if ($all.Count) {
        $got = @(Invoke-Zt @($all | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } }))
        for ($i = 0; $i -lt $all.Count -and $i -lt $got.Count; $i++) { $cards[$all[$i]] = $got[$i] }
    }
    $docCopy = Join-Path $Work 'dsh-event-instructions.zh.md'
    [IO.File]::WriteAllText($docCopy, (Get-Content -LiteralPath $Doc -Raw -Encoding utf8).Replace('{{KEY}}', $key).Replace('{{STAGING}}', $Staging).Replace('{{HELPER}}', $installedHelper), $utf8)
    $jobs = @()
    for ($j = 0; $j * $GroupsPerJob -lt $groups.Count; $j++) {
        $id = 'ev-{0:d2}' -f ($j + 1)
        if ($state.applied.ContainsKey($id)) { continue }
        $chunk = @($groups | Select-Object -Skip ($j * $GroupsPerJob) -First $GroupsPerJob)
        $answer = Join-Path $out "$id.json"
        $sb = [Text.StringBuilder]::new()
        [void]$sb.AppendLine("# 事件任务 $id：《$($manifest.book_title)》").AppendLine()
        [void]$sb.AppendLine("把结果写入 ``$answer``。").AppendLine()
        [void]$sb.AppendLine("SELF-CHECK: pwsh -NoProfile -File `"$PSCommandPath`" -Step precheck -Staging `"$Staging`" -Work `"$Work`" -Job $id").AppendLine()
        foreach ($n in $chunk) {
            $g = @($manifest.groups | Where-Object { [int]$_.n -eq $n })[0]
            [void]$sb.AppendLine("## 第 $n 组：$($g.chapter)：$(@($g.headings) -join '；')").AppendLine()
            [void]$sb.AppendLine("锚点：``$key/$n``").AppendLine().AppendLine('本组的观点卡（supports 只能取这些地址）：').AppendLine()
            foreach ($a in $owned[$n]) {
                $c = $cards[$a]
                if ($c -and [string]$c.type -in @('model', 'event', 'hub')) { continue }
                [void]$sb.AppendLine("- ``$a`` $(if ($c) { [string]$c.title } else { '' })")
            }
            [void]$sb.AppendLine().AppendLine('事件候选：').AppendLine()
            foreach ($c in $byGroup["$n"]) { [void]$sb.AppendLine("- ``$($c.src)`` $($c.text)") }
            [void]$sb.AppendLine()
        }
        $jobFile = Join-Path $Work "$id.md"
        [IO.File]::WriteAllText($jobFile, $sb.ToString(), $utf8)
        $jobs += [ordered]@{ id = $id; files = @($docCopy, $jobFile); prompt = '按以上说明写出这些组的事件卡 JSON 文件，运行 SELF-CHECK 改到 faults: 0。最后只回复一行 JSON。' }
    }
    Out-Last ([ordered]@{ candidates = $candidates.Count; groups = $groups.Count; jobs = $jobs })
    return
}

if ($Step -eq 'precheck') {
    if (-not $Job) { throw 'precheck needs -Job' }
    $file = Join-Path $out "$Job.json"
    if (-not (Test-Path -LiteralPath $file)) { "$Job EV0 no answer file $file"; 'faults: 1'; return }
    try { $events = Read-Answer $file } catch { "$Job EV0 the answer is not JSON: $($_.Exception.Message)"; 'faults: 1'; return }
    $faults = Test-Events $events -Lookup
    $checked = Get-DraftFaults $events $Job
    foreach ($f in $faults) { $f }
    foreach ($c in $checked) { $c }
    "faults: $($faults.Count + $checked.Count)"
    return
}

if ($Step -eq 'report') {
    $s = Get-State
    $jobs = @($s.applied.Values)
    Out-Last ([ordered]@{ jobs = $jobs.Count; created = (@($jobs | ForEach-Object { @($_.created) }) | Where-Object { $_ }).Count
            merged = (@($jobs | ForEach-Object { @($_.merged) }) | Where-Object { $_ }).Count; linked = [int](@($jobs | ForEach-Object { [int]$_.linked }) | Measure-Object -Sum).Sum
            refused = @($s.refused).Count; errors = @($jobs | Where-Object { $_.error } | ForEach-Object { "$($_.id): $($_.error)" }) })
    return
}

# --- apply -----------------------------------------------------------------------------------------
$state = Get-State
$summary = [ordered]@{ ok = $true; applied_jobs = 0; created = 0; merged = 0; linked = 0; refused = @(); pending_jobs = 0 }
foreach ($file in @(Get-ChildItem -LiteralPath $out -Filter 'ev-*.json' -File | Sort-Object Name)) {
    $id = $file.BaseName
    if ($state.applied.ContainsKey($id)) { continue }
    try { $events = Read-Answer $file.FullName } catch { $summary.refused += "${id}: the answer is not JSON"; continue }
    # Refuse what the self-check would report: the structure, and zt-check on the drafts (no unchecked text is written).
    $faults = @((Test-Events $events -Lookup) | ForEach-Object { $_ }) + @((Get-DraftFaults $events $id) | ForEach-Object { $_ })
    $bad = @{}
    foreach ($f in $faults) { if ($f -match '^event:(\d+) ' -and -not $bad.ContainsKey([int]$Matches[1])) { $bad[[int]$Matches[1]] = $f } }
    foreach ($b in $bad.Values) { $summary.refused += "${id} $b" }
    if (-not $Apply) { continue }
    $calls = @()
    $entry = [ordered]@{ id = $id; created = @(); merged = @(); linked = 0; error = '' }
    $links = [ordered]@{}   # Idea Card -> the lines that link its events
    $i = 0
    foreach ($e in $events) {
        $i++
        if ($bad.ContainsKey($i)) { continue }
        $supports = @($e['supports'] | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ })
        $existing = (Get-Field $e 'existing').Trim()
        if ($existing) {
            $card = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $existing } }))[0]
            $lines = [Collections.Generic.List[string]]::new([string[]](([string]$card.body) -split "`n"))
            $at = -1
            for ($k = 0; $k -lt $lines.Count; $k++) { if ($lines[$k] -match '^来源：') { $at = $k } }
            $source = "来源：[[$key]] $(Get-Field $e 'section')"
            if ($at -ge 0) { $lines.Insert($at + 1, $source) } else { $lines.Add($source) }
            $lines.Add("证据作用：$(Get-Field $e 'role')")
            $calls += @{ tool = 'edit_card'; arguments = @{ at = $existing; body = ($lines -join "`n") } }
            $ref = $existing
            $entry.merged += $existing
        } else {
            $calls += @{ tool = 'create_successor'; arguments = @{ at = $supports[0]; kind = 'side'; type = 'event'; title = (Get-Field $e 'title').Trim(); body = (Get-EventBody $e); src = (Get-Field $e 'src') } }
            $ref = "@@$($calls.Count - 1)"
        }
        $title = (Get-Field $e 'title').Trim().TrimEnd('。')
        foreach ($s in $supports) { if (-not $links.Contains($s)) { $links[$s] = @() }; $links[$s] += "案例 [[$ref]]：$title。" }
    }
    # Each supported Idea Card links its events once, in one edit.
    if ($links.Count) {
        $ideas = @(Invoke-Zt @($links.Keys | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } }))
        $k = 0
        foreach ($s in @($links.Keys)) {
            $body = [string]$ideas[$k].body
            $k++
            $add = @($links[$s] | Where-Object { -not $body.Contains(($_ -replace '^案例 (\[\[[^\]]+\]\]).*$', '$1')) })
            if (-not $add.Count) { continue }
            $calls += @{ tool = 'edit_card'; arguments = @{ at = $s; body = ($body.TrimEnd() + "`n`n" + ($add -join "`n")) } }
            $entry.linked += $add.Count
        }
    }
    if ($calls.Count) {
        $log = Join-Path $Work 'created.txt'
        $result = @(($calls | ConvertTo-Json -Depth 8 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Log $log)
        $made = @($result | Where-Object { $_ -match '^(\d+) ok ' } | ForEach-Object { [int]($_ -split ' ', 2)[0] })
        for ($c = 0; $c -lt $calls.Count; $c++) { if ($calls[$c].tool -eq 'create_successor' -and $made -contains $c) { $entry.created += (@($result | Where-Object { $_ -match "^$c ok " })[0] -split ' ok ', 2)[1].Trim() } }
        $err = @($result | Where-Object { $_ -match '^\d+ error ' })
        if ($LASTEXITCODE -ne 0 -or $err.Count) { $entry.error = (@($err) + "exit $LASTEXITCODE")[0]; $summary.ok = $false }
    }
    $state.applied[$id] = $entry
    $state.refused = @(@($state.refused) + @($bad.Values | ForEach-Object { "$id $_" }))
    Save-State $state
    $summary.applied_jobs++
    $summary.created += $entry.created.Count
    $summary.merged += $entry.merged.Count
    $summary.linked += $entry.linked
}
Out-Last $summary
