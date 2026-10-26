#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\grow-steps.tests.ps1
# The idea-grow stage's one step script, scripts/idea-grow/grow-steps.ps1 (#179), against a stand-in zt-digest:
# no archive, model or provider runs. The stand-in answers from a queue of lines per -Step and records every call,
# so the test checks the argument passing, the checksum guard, the one status recovery, the repeated-set guard and
# the failure answers.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$here = $PSScriptRoot
$repo = (Resolve-Path (Join-Path $here '..\..\..')).Path
$steps = Join-Path $repo 'scripts\idea-grow\grow-steps.ps1'
$scratch = Join-Path ([IO.Path]::GetTempPath()) "grow-steps-test-$PID"
$work = Join-Path $scratch 'work'
$tool = Join-Path $scratch 'zt-digest.ps1'
New-Item -ItemType Directory -Force -Path $work | Out-Null
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }

# FNV-1a over the Unicode scalar values, computed here a second way: the checksum a jobs line carries must match
# the same text the script hashes.
function Fnv([string]$text) {
    [uint32]$h = 2166136261
    for ($i = 0; $i -lt $text.Length; $i++) {
        $cp = [char]::ConvertToUtf32($text, $i)
        if ([char]::IsHighSurrogate($text[$i])) { $i++ }
        $h = [uint32]((([uint64]($h -bxor [uint32]$cp)) * [uint64]16777619) % 4294967296)
    }
    '{0:x8}' -f $h
}
function Job-Sum($ids, $job) {
    $files = (@($job.files) -join "`n")
    $parts = @((@($ids) -join ','), $job.cwd, $files, $job.prompt, $job.approve, $job.expect)
    Fnv ($parts -join "`n")
}
function New-Job([string]$family = 'grow') {
    [pscustomobject]@{
        family = $family; cwd = $work
        files = @((Join-Path $work 'instructions.zh.md'), (Join-Path $work 'jobs\{id}.md'))
        prompt = '完成任务 {id}。第 {id} 节。「引号」'; skills = @('zettelkasten', 'asd-ste100-zh')
        model = 'deepseek-official/deepseek-flash'; effort = 'high'; expect = '(^|\n)(OK|FLAG:)'
        approve = 'allow only: the precheck for {id}; reads under the work folder'
    }
}
function Jobs-Line([string]$round, [int]$token, [string[]]$ids, $job = $null) {
    if (-not $job) { $job = New-Job }
    $sum = Job-Sum $ids $job
    [pscustomobject]@{ next = 'jobs'; round = $round; token = $token; ids = @($ids); job = $job; sum = $sum } | ConvertTo-Json -Depth 8 -Compress
}
# One queued answer: the line the stand-in prints and the exit code it ends with.
function Entry([string]$out, [int]$code = 0) { [pscustomobject]@{ out = $out; exit = $code } | ConvertTo-Json -Compress }
function Set-Queue([string]$step, [object[]]$entries) {
    $lines = @($entries | ForEach-Object { if ($_ -is [string]) { $_ } else { $_ | ConvertTo-Json -Compress } })
    Set-Content -LiteralPath (Join-Path $scratch "$step-lines.txt") -Value $lines -Encoding utf8
    Set-Content -LiteralPath (Join-Path $scratch "$step-index.txt") -Value '0' -Encoding utf8
    Remove-Item -LiteralPath (Join-Path $scratch 'calls.jsonl') -ErrorAction SilentlyContinue
}
function Calls {
    $file = Join-Path $scratch 'calls.jsonl'
    if (-not (Test-Path -LiteralPath $file)) { return @() }
    @(Get-Content -LiteralPath $file -Encoding utf8 | ForEach-Object { $_ | ConvertFrom-Json -AsHashtable })
}
function Invoke-Next([string[]]$More = @()) {
    $all = @('-NoProfile', '-File', $steps, '-Step', 'next', '-Work', $work, '-Options', '{"mode":"full","books":[]}', '-Tool', $tool) + $More
    $lines = @(& pwsh @all 2>&1 | ForEach-Object { [string]$_ })
    $last = @($lines)[-1]
    $json = $null
    if ($last -and $last.Trim().StartsWith('{')) { try { $json = $last | ConvertFrom-Json -AsHashtable } catch { $json = $null } }
    [pscustomobject]@{ code = $LASTEXITCODE; lines = $lines; last = $last; json = $json }
}

# The stand-in zt-digest: logs the call, answers the next queued line of that -Step, and exits with its code.
Set-Content -LiteralPath $tool -Encoding utf8 -Value @'
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$here = $PSScriptRoot
$call = [ordered]@{ args = @($Rest); zt_config = "$env:ZT_CONFIG_DIR" }
$call | ConvertTo-Json -Compress -Depth 4 | Add-Content -LiteralPath (Join-Path $here 'calls.jsonl') -Encoding utf8
$at = [Array]::IndexOf($Rest, '-Step')
$step = if ($at -ge 0 -and $at + 1 -lt $Rest.Count) { $Rest[$at + 1] } else { 'unknown' }
$queue = Join-Path $here "$step-lines.txt"
$state = Join-Path $here "$step-index.txt"
$lines = @()
if (Test-Path -LiteralPath $queue) { $lines = @(Get-Content -LiteralPath $queue -Encoding utf8) }
$n = if (Test-Path -LiteralPath $state) { [int](Get-Content -LiteralPath $state -Raw) } else { 0 }
Set-Content -LiteralPath $state -Value ([string]($n + 1)) -Encoding utf8
if ($n -lt $lines.Count) {
    $entry = $lines[$n] | ConvertFrom-Json
    if ($entry.out) { Write-Output $entry.out }
    exit ([int]$entry.exit)
}
exit 0
'@

# --- 1. The done line passes through unchanged: one next call, the options and the work folder as given ---------
$done = '{"next":"done","mode":"full","created":4,"topics":1,"faults_left":0,"share_before":0.1,"share_after":0.5,"report":"' + (Join-Path $work 'grow-report.md').Replace('\', '\\') + '"}'
Set-Queue 'next' @(Entry $done)
$r = Invoke-Next
Assert ($r.code -eq 0 -and $r.json -and $r.json.next -eq 'done' -and $r.json.created -eq 4 -and $r.json.share_after -eq 0.5) "next: the done line is printed as it came ($($r.last))"
$calls = @(Calls)
Assert ($calls.Count -eq 1 -and ($calls[0].args -join ' ') -eq "grow -Step next -Work $work -Options {""mode"":""full"",""books"":[]}") "next: one grow -Step next call with the work folder and the options ($((@($calls) | ForEach-Object { $_.args -join ' ' }) -join ' | '))"
Assert (-not $calls[0].args.Contains('-After')) 'next: no -After on the first call'

# --- 2. A jobs line is checked and passed on: its ids, the token, the template and the checksum -----------------
$job = New-Job
$set = Jobs-Line 'ideas-jobs' 3 @('gw-001', 'gw-002') $job
Set-Queue 'next' @(Entry $set)
$r = Invoke-Next @('-After', '2')
Assert ($r.json.next -eq 'jobs' -and (@($r.json.ids) -join ',') -eq 'gw-001,gw-002' -and $r.json.token -eq 3 -and $r.json.job.model -eq 'deepseek-official/deepseek-flash') "next: a checked jobs line is passed on with its ids and template ($($r.last))"
Assert ($r.json.job.files[1] -eq (Join-Path $work 'jobs\{id}.md') -and $r.json.job.prompt.Contains('「引号」')) 'next: the template keeps its paths and its non-ASCII text'
Assert ((@(Calls))[0].args -join ' ' -match ' -After 2$') "next: -After reaches the command ($((@(Calls))[0].args -join ' '))"

# --- 3. A jobs line a relay cut (or rewrote) is never dispatched: the status is read once ------------------------
$cut = $set.Substring(0, [Math]::Min(80, $set.Length - 1))
Set-Queue 'next' @(Entry $cut)
Set-Queue 'status' @(Entry $set)
$r = Invoke-Next
Assert ($r.json.next -eq 'jobs' -and (@($r.json.ids) -join ',') -eq 'gw-001,gw-002') "next: a cut jobs line is read again from the status ($($r.last))"
Assert (@(Calls).Count -eq 2 -and (@(Calls))[1].args -join ' ' -eq "grow -Step status -Work $work -Options {""mode"":""full"",""books"":[]}") "next: exactly one status read follows the unreadable line ($((@(Calls) | ForEach-Object { $_.args[2] }) -join ','))"
$rewritten = (Jobs-Line 'ideas-jobs' 3 @('gw-001') $job) -replace '"gw-001"', '"gw-001","gw-009"'
Set-Queue 'next' @(Entry $rewritten)
Set-Queue 'status' @(Entry $set)
$r = Invoke-Next
Assert (@($r.json.ids) -join ',' -eq 'gw-001,gw-002') "next: a jobs line whose checksum fails is never dispatched; the status line is ($($r.last))"

# --- 4. Two unreadable lines answer one ok:false line and exit 0 ------------------------------------------------
Set-Queue 'next' @(Entry 'Command ran fine.')
Set-Queue 'status' @(Entry 'still prose', 1)
$r = Invoke-Next
Assert ($r.code -eq 0 -and $r.json.ok -eq $false -and $r.json.step -eq 'next' -and $r.json.error -match 'Command ran fine') "next: a second unreadable line answers ok:false with the tool's tail and exits 0 ($($r.last))"

# --- 5. A status line that only repeats the confirmed job set asks next again, so nothing is dispatched twice ---
$advanced = Jobs-Line 'links-jobs' 4 @('gl-001') $job
Set-Queue 'next' @(Entry $set)
Set-Queue 'status' @((Entry $set), (Entry $set))
$r = Invoke-Next @('-After', '3')
# The queue holds one next line; the second next call takes the same entry (the queue is exhausted and the stand-in
# prints nothing), so this checks the ask, then the stop: a repeated set must not be answered as a dispatch.
$nextCalls = @(Calls | Where-Object { $_.args[2] -eq 'next' })
Assert ($nextCalls.Count -eq 3 -and ($nextCalls | ForEach-Object { $_.args -join ' ' } | Where-Object { $_ -match ' -After 3$' }).Count -eq 3) "next: the status's repeated set asks next -After 3 again ($($nextCalls.Count) calls)"
Assert ($r.json.ok -eq $false -and $r.json.error -match 'kept answering the dispatched job set 3') "next: a set that never advances stops with its reason instead of dispatching ($($r.last))"
Set-Queue 'next' @((Entry $set), (Entry $advanced))
Set-Queue 'status' @(Entry $set)
$r = Invoke-Next @('-After', '3')
Assert ($r.json.next -eq 'jobs' -and $r.json.token -eq 4 -and (@($r.json.ids) -join ',') -eq 'gl-001') "next: the second ask gets the next round's job set ($($r.last))"

# --- 6. A fault line passes through with its typed fields; the box config reaches the command ------------------
$fault = '{"next":"fault","code":"card_locked","round":"apply","cause":"the owner locked topic 4/0","tree":"4/0","lock_id":3}'
Set-Queue 'next' @(Entry $fault)
$r = Invoke-Next @('-ZtConfig', 'D:\box\config')
Assert ($r.json.next -eq 'fault' -and $r.json.code -eq 'card_locked' -and $r.json.tree -eq '4/0' -and $r.json.lock_id -eq 3) "next: a fault line passes through with its tree and Lock id ($($r.last))"
$boxCalls = @(Calls)
Assert ($boxCalls[0].zt_config -eq 'D:\box\config') "next: a box copy's ZT_CONFIG_DIR reaches the tool ($($boxCalls[0].zt_config))"

# --- 7. A tool that fails with no line, then a good status line, still answers ----------------------------------
Set-Queue 'next' @(Entry '' 1)
Set-Queue 'status' @(Entry $set)
$r = Invoke-Next
Assert ($r.json.next -eq 'jobs' -and (@($r.json.ids) -join ',') -eq 'gw-001,gw-002') "next: a failed command is recovered from the status once ($($r.last))"

# --- 8. The step script is a straight line: only the one step, and no relay --------------------------------------------------
$text = Get-Content -LiteralPath $steps -Raw -Encoding utf8
Assert ($text -match "ValidateSet\('next'\)") 'grow-steps.ps1: the one step is next'
Assert (-not ($text -match 'ztRelay|runScript|wfpScript')) 'grow-steps.ps1: no relay helper is named'

Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
Write-Output $(if ($failed) { "$failed check(s) FAILED" } else { 'all checks passed' })
exit $(if ($failed) { 1 } else { 0 })
