#Requires -Version 7.5
<#
.SYNOPSIS
  The mathematics stages' deterministic command chains, one step per call (#178). A DeepSeek job runs one step
  and replies with its last line, so no Claude relay runs a single command.

  pwsh -NoProfile -File math-steps.ps1 -Step <step> -Out <staging> [-UnitsPerJob 4] [-Supply examples] [-MainResults 7]

  skeleton        the statement index, the math prompt files and the unit and Theory Card skeleton, in one
                  process                                                          -> {index, docs, skeleton}
  notation        the notation job (symbol candidates)                             -> math\drive\notation-jobs.json
  notation-apply  the Book notation table from the answer                          -> {ok, symbols, card}
  boundary        one job per -UnitsPerJob units: hypothesis roles and contrast pairs
                                                                                   -> math\drive\boundary-jobs.json
  boundary-apply  the parts from the answers, and one review job per supplied batch
                                                                                   -> math\drive\boundary-review-jobs.json
  boundary-settle delete each supplied part a review judged wrong
  proof           one proof-review job per -UnitsPerJob formal results             -> math\drive\proof-jobs.json
  proof-apply     write each fixed proof body that keeps every Link
  theory          one job per four Theory Cards                                    -> math\drive\theory-jobs.json
  theory-apply    rewrite each Theory Card from the answers
  check           the M1-M18 math rules on every unit and Theory Card (read-only)  -> math-audit.json

  Each step prints one compact JSON line. A step a tool command failed answers {ok:false, step, error} and
  exits 0: its caller reads the line and stops the stage with its reason. Files stay under <staging>\math\drive.
#>
param(
    [Parameter(Mandatory)][ValidateSet('skeleton', 'notation', 'notation-apply', 'boundary', 'boundary-apply', 'boundary-settle', 'proof', 'proof-apply', 'theory', 'theory-apply', 'check')][string]$Step,
    [Parameter(Mandatory)][string]$Out,
    [ValidateRange(1, 64)][int]$UnitsPerJob = 4,
    [ValidateSet('none', 'examples', 'examples+answers')][string]$Supply = 'examples',
    [ValidateRange(1, 64)][int]$MainResults = 7,
    [string]$Tool = 'zt-digest'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$Out = $Out.TrimEnd('\', '/')
$Drive = Join-Path (Join-Path $Out 'math') 'drive'
New-Item -ItemType Directory -Force $Drive | Out-Null

# One command: its last JSON line (or $null), its exit code and the tail of its output.
function Run([string[]]$CommandArgs) {
    $text = & $Tool @CommandArgs 2>&1 | ForEach-Object { "$_" }
    $code = $LASTEXITCODE
    $line = @($text | Where-Object { $_.Trim().StartsWith('{') }) | Select-Object -Last 1
    $json = $null
    if ($line) { try { $json = $line | ConvertFrom-Json -Depth 64 } catch { $json = $null } }
    [pscustomobject]@{ code = $code; json = $json; tail = (@($text) | Select-Object -Last 3) -join ' | ' }
}
function Save($Path, $Value) {
    $list = @($Value)
    $text = if ($list.Count) { $list | ConvertTo-Json -Depth 64 } else { '[]' }
    Set-Content -LiteralPath $Path -Value $text -Encoding utf8
}
function Reply($Value) { $Value | ConvertTo-Json -Depth 32 -Compress; exit 0 }
function Fail([string]$Where, $Result) { Reply @{ ok = $false; step = $Where; error = "$($Result.tail)" } }
# The fields of a tool line as one ordered answer.
function Fields($Json) {
    $out = [ordered]@{}
    if ($Json -is [System.Collections.IDictionary]) { foreach ($k in $Json.Keys) { $out["$k"] = $Json[$k] } }
    elseif ($Json) { foreach ($p in $Json.PSObject.Properties) { $out[$p.Name] = $p.Value } }
    return $out
}
# The step's answer: the tool line's own fields, plus this step's, with ok true unless the tool said otherwise.
function Reply-Fields($Json, [hashtable]$Extra = @{}) {
    $out = Fields $Json
    foreach ($k in $Extra.Keys) { $out[$k] = $Extra[$k] }
    if (-not $out.Contains('ok')) { $out['ok'] = $true }
    Reply $out
}
# One math compiler step of this staging folder.
function Math([string]$StepName, [string[]]$Extra = @()) { Run (@('math', '-Step', $StepName, '-Staging', $Out) + $Extra) }
# The jobs one plan command printed, saved for the stage's content managers.
function Save-Jobs([string]$Name, $Result) {
    $jobs = @($Result.json.jobs | Where-Object { $_ })
    $file = Join-Path $Drive $Name
    Save $file $jobs
    return [pscustomobject]@{ count = $jobs.Count; file = $file }
}

switch ($Step) {
    'skeleton' {
        # One process: the statement index, the prompt files the model stage reads, and the unit and Theory Card
        # skeleton. Each command's own line is kept, because the report reads all three.
        $index = Math 'index'
        if (-not $index.json -or $index.json.ok -ne $true) { Fail 'index' $index }
        $docs = Math 'docs'
        if (-not $docs.json -or $docs.json.ok -ne $true) { Fail 'docs' $docs }
        $skeleton = Math 'skeleton' @('-Apply')
        if (-not $skeleton.json -or $skeleton.json.ok -ne $true) { Fail 'skeleton' $skeleton }
        Reply @{ ok = $true; index = $index.json; docs = $docs.json; skeleton = $skeleton.json }
    }
    'notation' {
        $r = Math 'notation'
        if (-not $r.json) { Fail 'notation' $r }
        $saved = Save-Jobs 'notation-jobs.json' $r
        Reply @{ ok = $true; candidates = $r.json.candidates; jobs = $saved.count; file = $saved.file }
    }
    'notation-apply' {
        # A refused table (no answer file) keeps the tool's ok:false and reason, so the caller can go on without it.
        $r = Math 'notation-apply' @('-Apply')
        if (-not $r.json) { Fail 'notation-apply' $r }
        Reply-Fields $r.json
    }
    'boundary' {
        $r = Math 'boundary' @('-UnitsPerJob', "$UnitsPerJob", '-Supply', $Supply)
        if (-not $r.json) { Fail 'boundary' $r }
        $saved = Save-Jobs 'boundary-jobs.json' $r
        Reply @{ ok = $true; units = $r.json.units; jobs = $saved.count; file = $saved.file }
    }
    'boundary-apply' {
        $r = Math 'boundary-apply' @('-Supply', $Supply)
        if (-not $r.json) { Fail 'boundary-apply' $r }
        $saved = Save-Jobs 'boundary-review-jobs.json' $r
        Reply @{ ok = $true; created = $r.json.created; skipped_supplied = $r.json.skipped_supplied; jobs = $saved.count; file = $saved.file }
    }
    'boundary-settle' {
        $r = Math 'boundary-settle'
        if (-not $r.json) { Fail 'boundary-settle' $r }
        Reply @{ ok = $true; kept = $r.json.kept; deleted = $r.json.deleted; unreviewed = $r.json.unreviewed }
    }
    'proof' {
        $r = Math 'proof' @('-UnitsPerJob', "$UnitsPerJob")
        if (-not $r.json) { Fail 'proof' $r }
        $saved = Save-Jobs 'proof-jobs.json' $r
        Reply @{ ok = $true; units = $r.json.units; jobs = $saved.count; file = $saved.file }
    }
    'proof-apply' {
        # The tool counts its clean steps in `ok`, so its line is the nested answer, not this step's envelope.
        $r = Math 'proof-apply'
        if (-not $r.json) { Fail 'proof-apply' $r }
        Reply @{ ok = $true; applied = $r.json }
    }
    'theory' {
        $r = Math 'theory' @('-MainResults', "$MainResults")
        if (-not $r.json) { Fail 'theory' $r }
        $saved = Save-Jobs 'theory-jobs.json' $r
        Reply @{ ok = $true; theories = $r.json.theories; jobs = $saved.count; file = $saved.file }
    }
    'theory-apply' {
        $r = Math 'theory-apply' @('-MainResults', "$MainResults")
        if (-not $r.json) { Fail 'theory-apply' $r }
        Reply-Fields $r.json
    }
    'check' {
        $r = Math 'check' @('-Out', $Out)
        if (-not $r.json) { Fail 'check' $r }
        Reply-Fields $r.json
    }
}
