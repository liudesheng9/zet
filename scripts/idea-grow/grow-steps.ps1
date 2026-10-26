#Requires -Version 7.5
<#
.SYNOPSIS
  The idea-grow stage's deterministic command chain, one step per call (#179). A DeepSeek drive job runs the step
  and replies with its last line, so no Claude relay runs a single command.

  pwsh -NoProfile -File grow-steps.ps1 -Step next -Work <work folder> -Options '<json>' [-After <token>]
                                      [-ZtConfig <folder>] [-Tool zt-digest]

  next   runs `zt-digest grow -Step next` once and prints its checked JSON line. A line that is not one of the
         three shapes, or whose job set fails its checksum, is read again ONCE from `grow -Step status` (the
         command's own memory, S2). A status line that only repeats the job set this call already confirmed asks
         `next -After <token>` again, so the driver never dispatches one set twice; a second unreadable line, or a
         state that still names the confirmed set, answers {ok:false, step:"next", error:…}.

  Every answer is one compact JSON line and exits 0: {"next":"jobs",…}, {"next":"done",…}, {"next":"fault",…} or
  this step's {ok:false, step:"next", error:…}. The caller reads the line and stops with its reason.
#>
param(
    [Parameter(Mandatory)][ValidateSet('next')][string]$Step,
    [Parameter(Mandatory)][string]$Work,
    [Parameter(Mandatory)][string]$Options,
    [int]$After = -1,
    [string]$ZtConfig = '',
    [string]$Tool = 'zt-digest'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$Work = $Work.TrimEnd('\', '/')
# A copy of the box runs its own zt service: the step, and every child process it starts, names its config.
if ($ZtConfig) { $env:ZT_CONFIG_DIR = $ZtConfig }

# One command: its exit code, its last JSON line (or $null) and the tail of its output.
function Run([string[]]$CommandArgs) {
    $text = & $Tool @CommandArgs 2>&1 | ForEach-Object { "$_" }
    $code = $LASTEXITCODE
    $line = @($text | Where-Object { $_.Trim().StartsWith('{') }) | Select-Object -Last 1
    $json = $null
    if ($line) { try { $json = $line | ConvertFrom-Json -Depth 64 } catch { $json = $null } }
    [pscustomobject]@{ code = $code; json = $json; tail = (@($text) | Select-Object -Last 3) -join ' | ' }
}
function Reply($Value) { $Value | ConvertTo-Json -Depth 32 -Compress; exit 0 }
function Fail([string]$Where, $Result) { Reply @{ ok = $false; step = $Where; error = (@($Result.tail) | Where-Object { $_ }) -join ' | ' } }

# FNV-1a over the Unicode scalar values, as hex (grow_next.rs `fnv`; zt-grow-cycle v12 hashed the same text in
# JavaScript). A jobs line a relay cut or rewrote fails this checksum and is never dispatched.
function Fnv([string]$text) {
    [uint32]$h = 2166136261
    for ($i = 0; $i -lt $text.Length; $i++) {
        $cp = [char]::ConvertToUtf32($text, $i)
        if ([char]::IsHighSurrogate($text[$i])) { $i++ }
        $h = [uint32]((([uint64]($h -bxor [uint32]$cp)) * [uint64]16777619) % 4294967296)
    }
    '{0:x8}' -f $h
}
# The text a job set's checksum covers: the ids, then each template field, one per line.
function Job-Sum($json) {
    $job = $json.job
    $ids = (@($json.ids) | ForEach-Object { [string]$_ }) -join ','
    $files = ''
    if ($job -and $job.files -is [System.Array]) { $files = (@($job.files) | ForEach-Object { [string]$_ }) -join "`n" }
    $parts = @($ids, [string]$job.cwd, $files, [string]$job.prompt, [string]$job.approve, [string]$job.expect)
    Fnv ($parts -join "`n")
}
# The tool's line, when it is one the driver can act on; $null when it is not.
function Read-Line($json) {
    if (-not $json -or $json.next -notin @('jobs', 'done', 'fault')) { return $null }
    if ($json.next -ne 'jobs') { return $json }
    $ids = @($json.ids | Where-Object { $_ })
    $job = $json.job
    $shape = $ids.Count -gt 0 -and $job -and $job.files -is [System.Array] -and $job.prompt -is [string] -and $job.approve -is [string] -and [int]$json.token -gt 0
    if (-not $shape -or (Job-Sum $json) -ne [string]$json.sum) { return $null }
    return $json
}
# The command the step runs: the next chain, or its own memory when the line was unreadable.
function Grow([string]$StepName, [string[]]$Extra = @()) { Run (@('grow', '-Step', $StepName, '-Work', $Work, '-Options', $Options) + $Extra) }

switch ($Step) {
    'next' {
        $nextArgs = @()
        if ($After -ge 0) { $nextArgs = @('-After', [string]$After) }
        # One read, one status recovery; then, at most, two more asks when the status only repeats the set this
        # call already confirmed. Three answers naming that set mean the command is not advancing: stop.
        for ($attempt = 1; $attempt -le 3; $attempt++) {
            $ran = Grow 'next' $nextArgs
            $line = Read-Line $ran.json
            if (-not $line) {
                $status = Grow 'status'
                $line = Read-Line $status.json
                if (-not $line) { Fail 'next' ([pscustomobject]@{ tail = (@($ran.tail, $status.tail) | Where-Object { $_ }) -join ' | ' }) }
            }
            if ($line.next -eq 'jobs' -and $After -ge 0 -and [int]$line.token -eq $After) {
                if ($attempt -lt 3) { continue }
                Reply @{ ok = $false; step = 'next'; error = "next kept answering the dispatched job set $After" }
            }
            Reply $line
        }
    }
}
