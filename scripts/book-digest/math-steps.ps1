#Requires -Version 7.5
<#
.SYNOPSIS
  The mathematics stages' deterministic command chains, one step per call (#178). A DeepSeek job runs one step
  and replies with its last line, so no Claude relay runs a single command.

  pwsh -NoProfile -File math-steps.ps1 -Step <step> -Out <staging> [-UnitsPerJob 4] [-Supply examples] [-MainResults 7]

  index           the statement finder's DeepSeek jobs over the prepared sections   -> math\drive\index-jobs.json
  skeleton        the index from the finder's checked answers, the math prompt files and the unit and Theory
                  Card skeleton, in one process                                    -> {index, docs, skeleton}
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
    [Parameter(Mandatory)][ValidateSet('index', 'skeleton', 'notation', 'notation-apply', 'boundary', 'boundary-apply', 'boundary-settle', 'proof', 'proof-apply', 'theory', 'theory-apply', 'check')][string]$Step,
    [Parameter(Mandatory)][string]$Out,
    [ValidateRange(1, 64)][int]$UnitsPerJob = 4,
    [ValidateSet('none', 'examples', 'examples+answers')][string]$Supply = 'examples',
    [ValidateRange(1, 64)][int]$MainResults = 7,
    [ValidateRange(2000, 200000)][int]$IndexChars = 30000,
    [string]$Tool = 'zt-digest'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
. (Join-Path $PSScriptRoot 'step-line.ps1')
$Out = $Out.TrimEnd('\', '/')
$Work = Join-Path $Out 'math'
$Drive = Join-Path $Work 'drive'
New-Item -ItemType Directory -Force $Drive | Out-Null
$LineName = $Step

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
    # -InputObject keeps a one-entry list an array (piping unwraps it to a bare object; see book-steps.ps1 Save).
    $text = if ($list.Count) { ConvertTo-Json -InputObject $list -Depth 64 } else { '[]' }
    Set-Content -LiteralPath $Path -Value $text -Encoding utf8
}
# The step's answer: its compact line and the detail file beside it (#180).
function Reply($Value, $Detail = $null) { Send-StepLine -Line $Value -Detail $Detail -Name $LineName -Out $Work }
function Fail([string]$Where, $Result) { Send-StepFail -Step $Where -Reason "$($Result.tail)" -Name $LineName -Out $Work }
# How many entries a list-typed field names; 0 when the tool printed none.
function Count-Of($Value) {
    if ($null -eq $Value) { return 0 }
    return @($Value).Count
}
# One math compiler step of this staging folder.
function Math([string]$StepName, [string[]]$Extra = @()) { Run (@('math', '-Step', $StepName, '-Staging', $Out) + $Extra) }
# Bug D (docs/math-digest-redesign-proposal.md): each math step settles its own ledger units, so the run
# report and the stage outputs tell the same story. Every ready unit of the layer is claimed and completed with
# the layer's evidence file, page by page (the CLI claims at most 64 at once); a unit the ledger refuses (a changed
# archive, an unmet proof) is failed (retryable) for the next resume and named in `refused`. A claim the CLI refuses,
# or a missing evidence file, is an `error`: it never reads as a layer with nothing to settle. A unit that is not
# ready yet (its dependencies are open) is not claimed and stays for the step that meets them.
function Settle([string]$Layer, [string]$Evidence) {
    $result = [ordered]@{ layer = $Layer; settled = 0; refused = @(); error = $null }
    if (-not (Test-Path -LiteralPath $Evidence)) { $result.error = "no evidence file $Evidence"; return $result }
    $seen = @{}
    while ($true) {
        $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', $Layer, '-Take', '64', '-Brief', '-Owner', "math-steps $Layer")
        if ($claim.code -ne 0 -or -not $claim.json -or -not ($claim.json.PSObject.Properties.Name -contains 'units')) {
            $result.error = "the $Layer claim failed: $($claim.tail)"
            return $result
        }
        $page = @($claim.json.units | Where-Object { $_ -and $_.id })
        if (-not $page.Count) { break }
        $fresh = 0
        foreach ($u in $page) {
            $id = "$($u.id)"
            # The ledger claimed every unit it returned. A unit refused earlier in this drain became claimable
            # again: its claim is failed again with the same reason, never left held (Astra round 6).
            if ($seen.ContainsKey($id)) {
                $failed = Run @('ledger', 'fail', '-Out', $Out, '-Unit', $id, '-Token', "$($u.token)", '-Class', 'retryable', '-Fault', "math_settle_refused: $($seen[$id])")
                if ($failed.code -ne 0 -and -not $result.error) { $result.error = "the $Layer unit $id could not be failed: $($failed.tail)" }
                continue
            }
            $fresh++
            $done = Run @('ledger', 'done', '-Out', $Out, '-Unit', $id, '-Token', "$($u.token)", '-Outputs', $Evidence)
            if ($done.code -eq 0 -and $done.json) {
                $seen[$id] = ''
                $result.settled++
            } else {
                $why = ("$($done.tail)" -replace '\s+', ' ').Trim()
                if ($why.Length -gt 300) { $why = $why.Substring(0, 300) }
                $seen[$id] = $why
                $result.refused += "${id}: $why"
                $failed = Run @('ledger', 'fail', '-Out', $Out, '-Unit', $id, '-Token', "$($u.token)", '-Class', 'retryable', '-Fault', "math_settle_refused: $why")
                if ($failed.code -ne 0 -and -not $result.error) { $result.error = "the $Layer unit $id could not be failed: $($failed.tail)" }
            }
        }
        # A failed `ledger fail` stops the drain only after every unit of this claimed page is closed.
        if ($result.error) { return $result }
        # A page of repeats only that is not full holds no unseen unit: the drain is over. A full page of repeats
        # may hide unseen units behind them; the next page shows them (each repeat spends one attempt).
        if (-not $fresh -and $page.Count -lt 64) { break }
    }
    return $result
}
# One step's answer with its settlement: an error stops the step; the counts go on the line, the refusals in the detail.
function Reply-Settled($Line, $Settle, $Detail) {
    if ($Settle.error) { Send-StepFail -Step "$($Settle.layer) settle" -Reason $Settle.error -Name $LineName -Out $Work -Detail $Detail }
    $Line.settled = [int]$Settle.settled
    $Line.settle_refused = @($Settle.refused).Count
    $full = [ordered]@{ step = $Detail; settle = $Settle }
    Reply $Line $full
}
# The jobs one plan command printed, saved for the stage's content managers.
function Save-Jobs([string]$Name, $Result) {
    $jobs = @($Result.json.jobs | Where-Object { $_ })
    $file = Join-Path $Drive $Name
    Save $file $jobs
    return [pscustomobject]@{ count = $jobs.Count; file = $file }
}

switch ($Step) {
    'index' {
        # The statement finder (owner, 2026-11-21): DeepSeek reads every prepared section with its line numbers and
        # lists the book's definitions, results, examples and exercises, numbered or not. One job per -IndexChars.
        $r = Math 'index' @('-IndexChars', "$IndexChars")
        if (-not $r.json -or $r.json.ok -ne $true) { Fail 'index' $r }
        $saved = Save-Jobs 'index-jobs.json' $r
        Reply @{ ok = $true; sections = [int]$r.json.sections; chars = [int]$r.json.chars; jobs = $saved.count; file = $saved.file } $r.json
    }
    'skeleton' {
        # One process: the index from the finder's checked answers, the prompt files the model stage reads, and the
        # unit and Theory Card skeleton. The line carries their counts; each command's own line is the detail.
        $index = Math 'index-apply'
        if (-not $index.json -or $index.json.ok -ne $true) { Fail 'index' $index }
        $docs = Math 'docs'
        if (-not $docs.json -or $docs.json.ok -ne $true) { Fail 'docs' $docs }
        $skeleton = Math 'skeleton' @('-Apply')
        if (-not $skeleton.json -or $skeleton.json.ok -ne $true) { Fail 'skeleton' $skeleton }
        $ix = $index.json
        $sk = $skeleton.json
        # The three book layers settle in their dependency order: index, docs, then the skeleton.
        $settle = [ordered]@{ layer = 'math-index, math-docs, math-skeleton'; settled = 0; refused = @(); error = $null }
        foreach ($pair in @(@('math-index', 'index.json'), @('math-docs', 'docs.json'), @('math-skeleton', 'units.json'))) {
            $one = Settle $pair[0] (Join-Path $Work $pair[1])
            $settle.settled += $one.settled
            $settle.refused += @($one.refused)
            if ($one.error) { $settle.error = $one.error; $settle.layer = $pair[0]; break }
        }
        # Bug B: the settled skeleton registers one model build per allocated unit, so the model stage (and a mode
        # math repair) builds every unit; idempotent on a replay.
        if (-not $settle.error) {
            $reg = Run @('ledger', 'math', '-Out', $Out)
            if ($reg.code -ne 0) { $settle.error = "the unit builds were not registered: $($reg.tail)"; $settle.layer = 'models' }
        }
        Reply-Settled ([ordered]@{
            ok = $true
            index_units = [int]$ix.units; index_forms = (Keys-Of $ix.forms); index_examples = [int]$ix.examples; index_exercises = [int]$ix.exercises
            docs_files = (Count-Of $docs.json.files)
            # A replay allocates nothing new: `skeleton_units` counts the allocated units, `skeleton_created` the new ones (Bug H).
            skeleton_units = [int]$sk.units; skeleton_created = [int]$sk.created; skeleton_theories = [int]$sk.theories
        }) $settle @{ index = $ix; docs = $docs.json; skeleton = $sk }
    }
    'notation' {
        $r = Math 'notation'
        if (-not $r.json) { Fail 'notation' $r }
        $saved = Save-Jobs 'notation-jobs.json' $r
        Reply @{ ok = $true; candidates = [int]$r.json.candidates; jobs = $saved.count; file = $saved.file } $r.json
    }
    'notation-apply' {
        # A refused table (no answer file) keeps the tool's ok:false and reason, so the caller can go on without it.
        $r = Math 'notation-apply' @('-Apply')
        if (-not $r.json) { Fail 'notation-apply' $r }
        # A refused table (no answer file) keeps the tool's ok:false and reason, so the caller can go on without it.
        $line = [ordered]@{ ok = ($r.json.ok -eq $true); symbols = [int]$r.json.symbols; card = "$($r.json.card)"; error = "$($r.json.error)"; disposition = "$($r.json.disposition)" }
        if ($r.json.ok -ne $true) { Reply $line $r.json }
        Reply-Settled $line (Settle 'notation' (Join-Path $Work 'notation\applied.json')) $r.json
    }
    'boundary' {
        $r = Math 'boundary' @('-UnitsPerJob', "$UnitsPerJob", '-Supply', $Supply)
        if (-not $r.json) { Fail 'boundary' $r }
        $saved = Save-Jobs 'boundary-jobs.json' $r
        # The plan's units become ledger obligations (R237) before any job runs.
        $reg = Run @('ledger', 'math', '-Out', $Out)
        if ($reg.code -ne 0) { Fail 'boundary register' $reg }
        Reply @{ ok = $true; units = [int]$r.json.units; jobs = $saved.count; file = $saved.file } $r.json
    }
    'boundary-apply' {
        $r = Math 'boundary-apply' @('-Supply', $Supply)
        if (-not $r.json) { Fail 'boundary-apply' $r }
        $saved = Save-Jobs 'boundary-review-jobs.json' $r
        Reply @{ ok = $true; created = [int]$r.json.created; skipped_supplied = [int]$r.json.skipped_supplied; jobs = $saved.count; file = $saved.file } $r.json
    }
    'boundary-settle' {
        $r = Math 'boundary-settle'
        if (-not $r.json) { Fail 'boundary-settle' $r }
        Reply-Settled ([ordered]@{ ok = $true; kept = [int]$r.json.kept; deleted = [int]$r.json.deleted; unreviewed = [int]$r.json.unreviewed }) (Settle 'boundary' (Join-Path $Work 'boundary\settled.json')) $r.json
    }
    'proof' {
        $r = Math 'proof' @('-UnitsPerJob', "$UnitsPerJob")
        if (-not $r.json) { Fail 'proof' $r }
        $saved = Save-Jobs 'proof-jobs.json' $r
        # Reviewed and unmet proofs become ledger obligations (R240, Bug C) before any job runs.
        $reg = Run @('ledger', 'math', '-Out', $Out)
        if ($reg.code -ne 0) { Fail 'proof register' $reg }
        Reply @{ ok = $true; units = [int]$r.json.units; unmet = [int]$r.json.unmet; jobs = $saved.count; file = $saved.file } $r.json
    }
    'proof-apply' {
        # The tool counts its clean steps in `ok`, so its line is the nested answer, not this step's envelope.
        # The tool counts its clean steps in `ok`; this step's own ok stays a boolean, and its counts are flat.
        $r = Math 'proof-apply'
        if (-not $r.json) { Fail 'proof-apply' $r }
        # The reviewed proofs settle; an unmet one waits on its unit's build and stays a gap of the run (Bug C).
        Reply-Settled ([ordered]@{ ok = $true; steps_ok = [int]$r.json.ok; fixed = [int]$r.json.fixed; refused = [int]$r.json.refused; parts = (Count-Of $r.json.parts) }) (Settle 'proof' (Join-Path $Work 'proof\applied.json')) $r.json
    }
    'theory' {
        $r = Math 'theory' @('-MainResults', "$MainResults")
        if (-not $r.json) { Fail 'theory' $r }
        $saved = Save-Jobs 'theory-jobs.json' $r
        Reply @{ ok = $true; theories = [int]$r.json.theories; jobs = $saved.count; file = $saved.file } $r.json
    }
    'theory-apply' {
        $r = Math 'theory-apply' @('-MainResults', "$MainResults")
        if (-not $r.json) { Fail 'theory-apply' $r }
        Reply @{ ok = ($r.json.ok -ne $false); theories = [int]$r.json.theories; answered = [int]$r.json.answered } $r.json
    }
    'check' {
        $r = Math 'check' @('-Out', $Out)
        if (-not $r.json) { Fail 'check' $r }
        Reply-Settled ([ordered]@{ ok = ($r.json.ok -ne $false); units = [int]$r.json.units; theories = [int]$r.json.theories; faults = [int]$r.json.faults; rules = (Keys-Of $r.json.by_rule); report = "$($r.json.report)" }) (Settle 'math-check' (Join-Path $Out 'math-audit.json')) $r.json
    }
}


