#Requires -Version 7.5
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
              It writes in two helper runs: the Cards first, then the Link edits, which read the Idea Cards again
              and carry base, so an Idea Card a parallel stage changed in between is refused, not overwritten. A job
              whose helper run fails part way is not applied: state.json keeps it under partial (the Cards made so
              far and the error), and the next apply reuses those Cards and writes what is missing.
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
# applied: the jobs written in full; partial: the jobs whose helper run failed part way (the Cards made so far, the error).
function Get-State {
    $s = if (Test-Path -LiteralPath $stateFile) { Get-Content -LiteralPath $stateFile -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable } else { @{ applied = @{}; refused = @() } }
    if (-not $s.Contains('partial')) { $s.partial = @{} }
    $s
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
            refused = @($s.refused).Count; pending = $s.partial.Count
            errors = @(@($jobs | Where-Object { $_.error } | ForEach-Object { "$($_.id): $($_.error)" }) + @($s.partial.Keys | ForEach-Object { "$($_): $($s.partial[$_].error)" })) })
    return
}

# --- apply -----------------------------------------------------------------------------------------
# Runs the zt helper on $calls; returns the result lines and sets $entry.error when the run fails part way.
function Invoke-Helper($calls, $entry) {
    $log = Join-Path $Work 'created.txt'
    $result = @(($calls | ConvertTo-Json -Depth 8 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Log $log)
    $exit = $LASTEXITCODE
    $err = @($result | Where-Object { $_ -match '^\d+ error ' })
    if ($exit -ne 0 -or $err.Count) { $entry.error = (@($err) + "exit $exit")[0] }
    , $result
}
function Test-Ok($result, [int]$c) { [bool]@($result | Where-Object { $_ -match "^$c ok " }).Count }

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
    # A run of this job that failed part way left the Event Cards it made (event place -> address): reuse them,
    # so the rerun makes no Card twice and writes the Links that run missed.
    $partial = if ($state.partial.Contains($id)) { $state.partial[$id] } else { @{ made = @{}; linked = 0 } }
    $made = @{}
    foreach ($p in @($partial.made.Keys)) { $made[[string]$p] = [string]$partial.made[$p] }
    $entry = [ordered]@{ id = $id; created = @(); merged = @(); linked = [int]$partial.linked; error = '' }
    $new = 0

    # First run: merge into existing Event Cards and create the new ones.
    $calls = @()
    $createdBy = @{}   # call index -> event place
    $refs = [ordered]@{}   # event place -> its Event Card (an address, or the place of a Card still to make)
    $i = 0
    foreach ($e in $events) {
        $i++
        if ($bad.ContainsKey($i)) { continue }
        $supports = @($e['supports'] | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ })
        $existing = (Get-Field $e 'existing').Trim()
        if ($existing) {
            $card = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $existing } }))[0]
            $source = "来源：[[$key]] $(Get-Field $e 'section')"
            $role = "证据作用：$(Get-Field $e 'role')"
            # A failed run may have merged it already.
            if (-not (([string]$card.body).Contains($source) -and ([string]$card.body).Contains($role))) {
                $lines = [Collections.Generic.List[string]]::new([string[]](([string]$card.body) -split "`n"))
                $at = -1
                for ($k = 0; $k -lt $lines.Count; $k++) { if ($lines[$k] -match '^来源：') { $at = $k } }
                if ($at -ge 0) { $lines.Insert($at + 1, $source) } else { $lines.Add($source) }
                $lines.Add($role)
                $calls += @{ tool = 'edit_card'; arguments = @{ at = $existing; body = ($lines -join "`n"); base = [string]$card.text } }
            }
            $refs["$i"] = $existing
            $entry.merged += $existing
        } elseif (-not $made.ContainsKey("$i")) {
            $calls += @{ tool = 'create_successor'; arguments = @{ at = $supports[0]; kind = 'side'; type = 'event'; title = (Get-Field $e 'title').Trim(); body = (Get-EventBody $e); src = (Get-Field $e 'src') } }
            $createdBy[$calls.Count - 1] = $i
        }
    }
    if ($calls.Count) {
        $result = Invoke-Helper $calls $entry
        foreach ($c in @($createdBy.Keys)) {
            $line = @($result | Where-Object { $_ -match "^$c ok " })
            if ($line.Count) { $made["$($createdBy[$c])"] = ($line[0] -split ' ok ', 2)[1].Trim(); $new++ }
        }
    }

    # Second run: each supported Idea Card links its events once, in one edit. It reads the Cards after the first
    # run (a new Event Card changes their Reverse links), and each edit carries the text it was built on (base),
    # so a Card another stage changed in between is refused, not overwritten; the rerun reads it again.
    if (-not $entry.error) {
        $links = [ordered]@{}   # Idea Card -> the lines that link its events
        $i = 0
        foreach ($e in $events) {
            $i++
            if ($bad.ContainsKey($i)) { continue }
            $ref = if ($refs.Contains("$i")) { $refs["$i"] } else { $made["$i"] }
            $title = (Get-Field $e 'title').Trim().TrimEnd('。')
            foreach ($s in @($e['supports'] | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ })) {
                if (-not $links.Contains($s)) { $links[$s] = @() }
                $links[$s] += "案例 [[$ref]]：$title。"
            }
        }
        $calls = @()
        $linkCount = @{}   # call index -> Links that edit adds
        if ($links.Count) {
            $ideas = @(Invoke-Zt @($links.Keys | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } }))
            $k = 0
            foreach ($s in @($links.Keys)) {
                $body = [string]$ideas[$k].body
                $text = [string]$ideas[$k].text
                $k++
                $add = @($links[$s] | Where-Object { -not $body.Contains(($_ -replace '^案例 (\[\[[^\]]+\]\]).*$', '$1')) })
                if (-not $add.Count) { continue }
                $calls += @{ tool = 'edit_card'; arguments = @{ at = $s; body = ($body.TrimEnd() + "`n`n" + ($add -join "`n")); base = $text } }
                $linkCount[$calls.Count - 1] = $add.Count
            }
        }
        if ($calls.Count) {
            $result = Invoke-Helper $calls $entry
            foreach ($c in @($linkCount.Keys)) { if (Test-Ok $result $c) { $entry.linked += $linkCount[$c] } }
        }
    }

    $entry.created = @($made.Keys | Sort-Object { [int]$_ } | ForEach-Object { $made[$_] })
    $summary.created += $new
    if ($entry.error) {
        # Not applied: the rerun tries the job again with the Cards made so far.
        $state.partial[$id] = [ordered]@{ made = $made; linked = $entry.linked; error = $entry.error }
        $summary.ok = $false
        $summary.pending_jobs++
    } else {
        $state.applied[$id] = $entry
        $state.partial.Remove($id)
        $state.refused = @(@($state.refused) + @($bad.Values | ForEach-Object { "$id $_" }))
        $summary.applied_jobs++
        $summary.merged += $entry.merged.Count
        $summary.linked += $entry.linked
    }
    Save-State $state
}
Out-Last $summary
