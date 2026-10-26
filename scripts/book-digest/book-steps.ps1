<#
.SYNOPSIS
  The book-level digest stages' deterministic command chains, one step per call (#176). A DeepSeek drive job runs
  one step and replies with its last line, so no Claude agent runs a single command.

  pwsh -NoProfile -File book-steps.ps1 -Step <step> -Out <staging> [-Json '<options>'] [-Tool zt-digest]

  prepare           the book preparation (mode full), the one supported resume (mode resume) or a Term backfill
                    (modes term-cards and term-link)                     -> {ok, groups, citation_key, ...}
  link-prepare      the Link pass's receipt-owned gap set               -> the tool's brief line
  publish           a stage's own book revision (`finalize -Step stage`) -> its publication line
  cards-start       plan the card cycle and list its jobs               -> cards\out\jobs.json
  cards-apply       apply every ready draft and run the check            -> cards\out\jobs.json
  cards-finish      apply every ready draft and report the cycle         -> the cycle's report line
  unit-fail         record one claimed unit's fault
  cards-book-start  publish and claim the whole-book Card revision
  cards-book-finish the whole-book cycle's checked completion
  organize-survey   Organize's survey
  organize-apply    record Organize's decisions
  skeleton-start    plan the Literature index and structure Card jobs
  skeleton-finish   apply the skeleton answer
  figures           show the book's figures in the Cards that name them
  bridge-start      plan the Bridge and list its judge jobs
  bridge-report     freeze the Bridge report
  bridge-apply      apply a Bridge report's proposals
  audit             the read-only audit of the book
  finalize-plan     the audit plan (at most one correction cycle)
  finalize-changes  the retained changed-Card manifest
  finalize-scope    record the one cycle's changed Cards
  finalize-refresh  refresh one dependency-scoped publication
  finalize-final    the final read-only audit
  term-link         the finalization's Term-link round (plan, apply, complete, second plan)
  cleanup-start     the settlement plan (every recorded id and workspace)
  cleanup-apply     record the provider results and settle the cleanup units (cleanup-finish -Ready without them)
  cleanup-report    the verified run report

  Every step prints one compact JSON line. A step a tool command failed answers {ok:false, step, error} and exits
  0: its caller reads the line and stops the stage. A command the owner's Lock refused answers the archive's typed
  refusal ({ok:false, code:"card_locked", ...}) unchanged.
#>
param(
    [Parameter(Mandatory)][ValidateSet(
        'prepare', 'link-prepare', 'publish', 'gate',
        'cards-start', 'cards-apply', 'cards-finish',
        'unit-fail', 'cards-book-start', 'cards-book-finish',
        'organize-survey', 'organize-apply', 'skeleton-start', 'skeleton-finish',
        'figures', 'bridge-start', 'bridge-report', 'bridge-apply',
        'audit', 'finalize-plan', 'finalize-changes', 'finalize-scope', 'finalize-refresh', 'finalize-final',
        'term-link', 'cleanup-start', 'cleanup-apply', 'cleanup-report')][string]$Step,
    [Parameter(Mandatory)][string]$Out,
    [string]$Json = '{}',
    [string]$Tool = 'zt-digest'
)
$ErrorActionPreference = 'Stop'
$Out = $Out.TrimEnd('\', '/')
$o = @{}
if ($Json -and $Json.Trim()) {
    $parsed = $Json | ConvertFrom-Json -AsHashtable -Depth 32
    if ($parsed) { $o = $parsed }
}
# One option of the step's -Json object, or its default when the option is absent or empty.
function Opt([string]$Name, $Default = $null) {
    if ($o.ContainsKey($Name) -and $null -ne $o[$Name] -and "$($o[$Name])" -ne '') { return $o[$Name] }
    return $Default
}
function Flag([string]$Name) { return ($o.ContainsKey($Name) -and $o[$Name] -eq $true) }
function At([string]$Value) { return $Value.TrimEnd('\', '/') }

# One command: its last JSON line (or $null), its exit code and its last lines.
function Run([string[]]$CommandArgs) {
    $text = & $Tool @CommandArgs 2>&1 | ForEach-Object { "$_" }
    $code = $LASTEXITCODE
    $line = @($text | Where-Object { $_.Trim().StartsWith('{') }) | Select-Object -Last 1
    $json = $null
    if ($line) { try { $json = $line | ConvertFrom-Json -Depth 64 } catch { $json = $null } }
    [pscustomobject]@{ code = $code; json = $json; tail = ((@($text) | Select-Object -Last 3) -join ' | ') }
}
function Save($Path, $Value) {
    $dir = Split-Path -Parent $Path
    if ($dir) { [void][System.IO.Directory]::CreateDirectory($dir) }
    $list = @($Value)
    $text = if ($list.Count) { $list | ConvertTo-Json -Depth 64 } else { '[]' }
    Set-Content -LiteralPath $Path -Value $text -Encoding utf8
}
function Tail([string]$Text) {
    $t = "$Text".Trim()
    if ($t.Length -gt 400) { return $t.Substring(0, 400) }
    return $t
}
function Reply($Value) {
    $text = $Value | ConvertTo-Json -Depth 16 -Compress
    Write-Output $text
    exit 0
}
# The owner's typed refusal of an answer (the archive's `card_locked` JSON).
function Lock-Refusal($Json, [string]$Text) {
    if ($Json -and ($Json.code -eq 'card_locked' -or $Json.error -eq 'card_locked')) { return $Json }
    foreach ($line in "$Text".Split("`n")) {
        $t = $line.Trim()
        if (-not $t.StartsWith('{')) { continue }
        try { $v = $t | ConvertFrom-Json -Depth 16 } catch { continue }
        if ($v.code -eq 'card_locked' -or $v.error -eq 'card_locked') { return $v }
    }
    return $null
}
# The step's answer for one tool line: its fields, with ok true unless the line says otherwise. A refused line
# keeps the archive's typed refusal and ok false.
function Reply-Line($Json, [hashtable]$Extra = @{}) {
    $out = @{}
    if ($Json -is [System.Collections.IDictionary]) {
        foreach ($k in $Json.Keys) { $out["$k"] = $Json[$k] }
    } elseif ($Json) {
        foreach ($p in $Json.PSObject.Properties) { $out[$p.Name] = $p.Value }
    }
    foreach ($k in $Extra.Keys) { $out[$k] = $Extra[$k] }
    $refusal = Lock-Refusal $Json ''
    if ($refusal) {
        $out['ok'] = $false
        if (-not $out.ContainsKey('error')) { $out['error'] = "$($refusal.error)" }
        Reply $out
    }
    if (-not $Json -or -not $out.ContainsKey('ok')) { $out['ok'] = $true }
    Reply $out
}
function Fail([string]$Name, [string]$Reason, $Json = $null) {
    $out = @{ ok = $false; step = $Name; error = (Tail $Reason) }
    if ($Json) {
        foreach ($k in @('code', 'message', 'refusal', 'lock_id', 'tree', 'conflicts')) {
            if ($null -ne $Json.$k) { $out[$k] = $Json.$k }
        }
    }
    Reply $out
}
function Gate([string]$Layer) {
    $g = Run @('ledger', 'gate', '-Out', $Out, '-Layer', $Layer, '-Brief')
    if ($g.json) { return $g.json }
    return @{ ok = $false; complete = $false; error = (Tail $g.tail) }
}
# The jobs a card-cycle plan or check printed: a compact list ({dir, lead, prompts, jobs:[{id, kind, file}]})
# expands to the exact files and prompt of every job, as the card compiler's own job list declares them.
function Expand-Cards($Json) {
    $jobs = @()
    if (-not $Json) { return @() }
    $dir = "$($Json.dir)"
    foreach ($j in @($Json.jobs)) {
        if (-not $j) { continue }
        $entry = [ordered]@{ id = "$($j.id)" }
        if ($j.files) {
            $entry.files = @($j.files)
            $entry.prompt = "$($j.prompt)"
        } elseif ($Json.lead -and $Json.prompts) {
            switch ("$($j.kind)") {
                'follow' {
                    $entry.files = @((Join-Path $dir "$($j.file)"))
                    $entry.prompt = "$($Json.prompts.follow)"
                    if ($j.continue) { $entry.continue = "$($j.continue)" }
                    $entry.fallback = @("$($Json.lead.fix)", (Join-Path $dir "$($j.file)"))
                }
                'orphans' {
                    $entry.files = @("$($Json.lead.orphans)", (Join-Path $dir "$($j.file)"))
                    $entry.prompt = "$($Json.prompts.orphans)"
                }
                default {
                    $entry.files = @("$($Json.lead.fix)", (Join-Path $dir "$($j.file)"))
                    $entry.prompt = "$($Json.prompts.fix)"
                }
            }
        } else { continue }
        $jobs += $entry
    }
    return @($jobs)
}
# The job file one content round reads: the plan's or the check's expanded list.
function Save-Jobs([string]$Work, $Json) {
    $jobs = @(Expand-Cards $Json)
    $file = Join-Path $Work 'out\jobs.json'
    Save $file $jobs
    return [pscustomobject]@{ count = $jobs.Count; file = $file }
}
# One unit's fault, as the ledger takes it: the claim's hash, the cause and a code only for a content refusal.
function Fail-Unit([string]$Unit, [int]$Token, [string]$Hash, [string]$Cause, [string]$Code = '') {
    $fault = [ordered]@{ schema_version = 1; unit = $Unit; input_hash = $Hash; cause = (Tail $Cause) }
    if ($Code) { $fault['code'] = $Code }
    $text = $fault | ConvertTo-Json -Compress
    $out = Run @('ledger', 'fail', '-Out', $Out, '-Unit', $Unit, '-Token', "$Token", '-Fault', $text)
    if ($out.json -and $out.json.state -eq 'failed') { return 'failed' }
    if ($out.json) { return 'retry' }
    if ($Code -eq 'check_failed' -or $Code -eq 'card_locked') { return 'failed' }
    return 'retry'
}

switch ($Step) {
    'prepare' {
        $mode = "$(Opt 'Mode' 'full')"
        $key = "$(Opt 'CitationKey' '')"
        if ($mode -eq 'resume') {
            # R278-R285: the one supported resume; the retained groups and identity come from the ledger's own
            # summary, and the digest gate answers a summary that carries no layer counts.
            $facts = Run @('ledger', 'resume', '-Out', $Out)
            if (-not $facts.json) { Fail 'resume' "the ledger resume printed no line: $($facts.tail)" }
            if (@('resumed', 'nothing_to_resume') -notcontains "$($facts.json.status)") {
                Fail 'resume' ("the retained run cannot resume: " + $(if ($facts.json.error) { "$($facts.json.error)" } else { $facts.tail })) $facts.json
            }
            $status = Run @('ledger', 'status', '-Out', $Out, '-Summary')
            if (-not $status.json) { Fail 'resume status' "the ledger summary printed no line: $($status.tail)" }
            $view = $status.json
            $groups = 0
            if ($view.layers -and $view.layers.digest) { $groups = [int]$view.layers.digest.total }
            if ($groups -le 0) {
                $gate = Gate 'digest'
                if ($gate.counts) {
                    $groups = [int]$gate.counts.total
                    if ($groups -le 0) { $groups = [int]$gate.counts.declared }
                }
            }
            if ($groups -le 0) { Fail 'resume' 'mode resume found no retained digest groups; prepare the book again' $view }
            Reply-Line @{ groups = $groups; resumed = ("$($facts.json.status)" -eq 'resumed'); resume = $facts.json; settings = $view.settings; variant = $view.variant; citation_key = $view.citation_key; run_options = $view.run_options } @{}
        }
        elseif ($mode -eq 'term-cards' -or $mode -eq 'term-link') {
            # S25 (#168): the backfill's own preparation. `-Sections` reuses or reconstructs the staged source
            # slices the Term-card branch reads; `-Validate` only inspects the already digested book.
            $flag = if ($mode -eq 'term-cards') { '-Sections' } else { '-Validate' }
            $r = Run @('prepare', '-Out', $Out, $flag, '-Brief')
            if (-not $r.json) { Fail "prepare $mode" "the backfill preparation printed no line: $($r.tail)" }
            if ($r.json.ok -ne $true) {
                $said = @($r.json.conflicts | ForEach-Object { "$($_.message)" }) -join '; '
                if (-not $said) { $said = "$($r.json.error)" }
                Fail "prepare $mode" $said $r.json
            }
            Reply-Line $r.json
        }
        else {
            # The full preparation: args.book resolves a Scholar Archive Work; otherwise the explicit source,
            # BibTeX key and title are given. The effective settings and the layer switches freeze here.
            if (Opt 'Book') {
                $bookArgs = @('-Book', "$(Opt 'Book')")
            } else {
                $bookArgs = @('-Source', "$(Opt 'Source')", '-BibTeX', "$(Opt 'BibTeX')", '-BookTitle', "$(Opt 'BookTitle')")
            }
            $argv = @('prepare') + $bookArgs + @('-Out', $Out, '-CitationKey', $key,
                '-MaxChars', "$(Opt 'MaxChars' 20000)", '-MaxUnits', "$(Opt 'MaxUnits' 3)")
            if (Opt 'StartHeading') { $argv += @('-StartHeading', "$(Opt 'StartHeading')") }
            if (Opt 'EndHeading') { $argv += @('-EndHeading', "$(Opt 'EndHeading')") }
            if (Opt 'SkipHeadings') { $argv += @('-SkipHeadings', "$(Opt 'SkipHeadings')") }
            if (Opt 'FileId') { $argv += @('-FileId', "$(Opt 'FileId')") }
            if (Opt 'Instructions') { $argv += @('-Instructions', "$(Opt 'Instructions')") }
            if (Flag 'NoEventCandidates') { $argv += '-NoEventCandidates' }
            $argv += @('-Variant', "$(Opt 'Variant' 'econ')", '-MaxAttempts', "$(Opt 'MaxAttempts' 3)", '-DshRunCap', "$(Opt 'DshRunCap' 1200)")
            if (Opt 'RunOptions') { $argv += @('-RunOptions', "$(Opt 'RunOptions')") }
            $argv += @('-Apply', '-Brief')
            $prepared = Run $argv
            if (-not $prepared.json) { Fail 'prepare book' "the preparation printed no line: $($prepared.tail)" }
            $refusal = Lock-Refusal $prepared.json $prepared.tail
            if ($refusal) { Reply-Line $prepared.json }
            if ($prepared.json.ok -ne $true -or [int]$prepared.json.groups -le 0) {
                Fail 'prepare book' ("preparation failed: " + $(if ($prepared.json.error) { "$($prepared.json.error)" } else { ($prepared.json | ConvertTo-Json -Depth 8 -Compress) })) $prepared.json
            }
            Reply-Line $prepared.json
        }
    }
    'gate' {
        Reply-Line (Gate "$(Opt 'Layer')")
    }
    'link-prepare' {
        $r = Run @('prepare-link', '-Staging', $Out, '-Brief')
        if (-not $r.json) { Fail 'prepare-link' "the Link preparation printed no line: $($r.tail)" }
        Reply-Line $r.json
    }
    'publish' {
        $layer = "$(Opt 'Layer')"
        # The whole-book Card revision is the ledger's own publication; every other stage publishes its revision
        # through `finalize -Step stage` with its declared artifacts.
        if (Flag 'Ledger') {
            $r = Run @('ledger', 'publish', '-Out', $Out, '-Layer', $layer)
            if (-not $r.json) { Fail "$layer publish" "the ledger publication printed no line: $($r.tail)" }
            Reply-Line $r.json
        }
        $argv = @('finalize', '-Step', 'stage', '-Out', $Out, '-Layer', $layer)
        if (Opt 'Report') { $argv += @('-Report', "$(Opt 'Report')") }
        if (Opt 'Decisions') { $argv += @('-Decisions', "$(Opt 'Decisions')") }
        if (Opt 'Applied') { $argv += @('-Applied', "$(Opt 'Applied')") }
        if (Opt 'Skeleton') { $argv += @('-Skeleton', "$(Opt 'Skeleton')") }
        if (Opt 'Cards') { $argv += @('-Cards', "$(Opt 'Cards')") }
        $r = Run $argv
        if (-not $r.json) { Fail "$layer publish" "the stage publication printed no line: $($r.tail)" }
        Reply-Line $r.json
    }
    'cards-start' {
        $work = At "$(Opt 'Work')"
        [void][System.IO.Directory]::CreateDirectory($work)
        $argv = @('card', '-Step', 'plan', '-Work', $work, '-Compact', '-Book', "$(Opt 'Book')", '-Staging', $Out)
        if (Opt 'Rules') { $argv += @('-Rules', "$(Opt 'Rules')") }
        if (Opt 'MaxCards') { $argv += @('-MaxCards', "$(Opt 'MaxCards')") }
        if (Opt 'Sample') { $argv += @('-Sample', "$(Opt 'Sample')") }
        if (Flag 'ModelReview') { $argv += '-ModelReview' }
        if (Opt 'Notes') { $argv += @('-Notes', "$(Opt 'Notes')") }
        if (Flag 'NoOrphans') { $argv += '-NoOrphans' }
        if (Opt 'Cards') { $argv += @('-Cards', "$(Opt 'Cards')") }
        $plan = Run $argv
        if (-not $plan.json) { Fail 'plan' "the card plan printed no line: $($plan.tail)" }
        if ($plan.json.ok -eq $false) { Fail 'plan' "$($plan.json.error)" $plan.json }
        $refusal = Lock-Refusal $plan.json $plan.tail
        if ($refusal) { Reply-Line $plan.json }
        $saved = Save-Jobs $work $plan.json
        Reply-Line $plan.json @{ jobs = $saved.count; file = $saved.file }
    }
    'cards-apply' {
        $work = At "$(Opt 'Work')"
        $max = [int]"$(Opt 'MaxJobs' 30)"
        $applied = 0
        $pending = 0
        for ($i = 1; $i -le 100; $i++) {
            $a = Run @('card', '-Step', 'apply', '-Work', $work, '-MaxJobs', "$max")
            if (-not $a.json) { Fail 'apply' "the card apply printed no line: $($a.tail)" }
            $refusal = Lock-Refusal $a.json $a.tail
            if ($refusal) { Reply-Line $a.json }
            $applied += [int]$a.json.applied_jobs
            $pending = [int]$a.json.pending_jobs
            if ($pending -le 0) { break }
        }
        $check = Run @('card', '-Step', 'check', '-Work', $work, '-Compact')
        if (-not $check.json) { Fail 'check' "the card check printed no line: $($check.tail)" }
        $refusal = Lock-Refusal $check.json $check.tail
        if ($refusal) { Reply-Line $check.json }
        $saved = Save-Jobs $work $check.json
        Reply-Line $check.json @{ applied = $applied; jobs = $saved.count; file = $saved.file }
    }
    'cards-finish' {
        $work = At "$(Opt 'Work')"
        $max = [int]"$(Opt 'MaxJobs' 30)"
        $applied = 0
        for ($i = 1; $i -le 100; $i++) {
            $a = Run @('card', '-Step', 'apply', '-Work', $work, '-MaxJobs', "$max")
            if (-not $a.json) { Fail 'apply' "the card apply printed no line: $($a.tail)" }
            $refusal = Lock-Refusal $a.json $a.tail
            if ($refusal) { Reply-Line $a.json }
            $applied += [int]$a.json.applied_jobs
            if ([int]$a.json.pending_jobs -le 0) { break }
        }
        $report = Run @('card', '-Step', 'report', '-Work', $work)
        if (-not $report.json) { Fail 'report' "the card report printed no line: $($report.tail)" }
        $refusal = Lock-Refusal $report.json $report.tail
        if ($refusal) { Reply-Line $report.json }
        Reply-Line $report.json @{ applied = $applied }
    }
    'unit-fail' {
        $unit = "$(Opt 'Unit')"
        $token = [int]"$(Opt 'Token' 0)"
        $status = Fail-Unit $unit $token "$(Opt 'InputHash' '')" "$(Opt 'Cause' '')" "$(Opt 'Code' '')"
        Reply @{ ok = $true; unit = $unit; state = $status }
    }
    'cards-book-start' {
        # The revision is derived from the present contributions; the same manifest reuses it.
        $published = Run @('ledger', 'publish', '-Out', $Out, '-Layer', 'cards-book')
        if (-not $published.json -or -not $published.json.unit) { Fail 'publish' "the cards-book revision was not registered: $($published.tail)" }
        $revision = "$($published.json.unit)"
        $wait = [int]"$(Opt 'Wait' 0)"
        $lease = [int]"$(Opt 'Lease' 300)"
        $claim = $null
        for ($try = 1; $try -le 3; $try++) {
            $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'cards-book', '-Take', '1', '-Owner', 'cards-book claim', '-Wait', "$wait", '-Lease', "$lease", '-Brief')
            if ($claim.json -or $claim.code -eq 0) { break }
        }
        $units = @()
        if ($claim.json -and $claim.json.units -is [array]) {
            $units = @($claim.json.units | Where-Object { $_ -and $_.id })
        } else {
            # A claim line the tool cut is never an empty claim: the units this owner holds come from the ledger.
            $held = Run @('ledger', 'status', '-Out', $Out, '-Held', 'cards-book claim')
            if ($held.json -and $held.json.units -is [array]) { $units = @($held.json.units | Where-Object { $_ -and $_.id }) }
        }
        $unit = @($units | Where-Object { $_.id -eq $revision }) | Select-Object -First 1
        if (-not $unit) { $unit = @($units) | Select-Object -First 1 }
        if (-not $unit) { Reply @{ ok = $true; unit = $null; revision = $revision; waiting = $true; gate = (Gate 'cards-book') } }
        Reply @{ ok = $true; unit = "$($unit.id)"; token = $unit.token; input_hash = "$($unit.input_hash)"; inputs = $unit.inputs; revision = $revision }
    }
    'cards-book-finish' {
        $unit = "$(Opt 'Unit')"
        $token = [int]"$(Opt 'Token' 0)"
        $work = At "$(Opt 'Work')"
        $argv = @('book-cards-finish', '-Out', $Out, '-Work', $work, '-Unit', $unit, '-Token', "$token")
        if (Flag 'CheckOnly') { $argv += '-CheckOnly' }
        $finish = Run $argv
        if ($finish.json -and ($finish.json.state -eq 'done' -or $finish.json.complete -eq $true)) { Reply-Line $finish.json }
        # A cut completion line asks the ledger before any fault: a unit it holds done is done.
        $read = Run @('ledger', 'status', '-Out', $Out, '-Units', $unit)
        $held = @(@($read.json.units) | Where-Object { $_ -and $_.id -eq $unit }) | Select-Object -First 1
        if ($held -and ($held.state -eq 'done' -or $held.state -eq 'noop')) {
            Reply @{ ok = $true; unit = $unit; state = "$($held.state)"; disposition = $held.disposition; from = 'ledger' }
        }
        $cause = if ($finish.json -and $finish.json.fault) { "$($finish.json.fault)" } elseif ($finish.json -and $finish.json.error) { "$($finish.json.error)" } else { $finish.tail }
        $status = Fail-Unit $unit $token "$(Opt 'InputHash' '')" $cause
        Reply @{ ok = $false; step = 'book-cards-finish'; unit = $unit; state = $status; error = (Tail $cause) }
    }
    'organize-survey' {
        $work = At "$(Opt 'Work')"
        $r = Run @('organize-survey', '-Staging', $Out, '-Work', $work)
        if (-not $r.json) { Fail 'organize survey' "the survey printed no line: $($r.tail)" }
        Reply-Line $r.json
    }
    'organize-apply' {
        $work = At "$(Opt 'Work')"
        $r = Run @('organize-apply', '-Work', $work)
        if (-not $r.json) { Fail 'organize apply' "the apply printed no line: $($r.tail)" }
        Reply-Line $r.json
    }
    'skeleton-start' {
        $work = At "$(Opt 'Work')"
        [void][System.IO.Directory]::CreateDirectory($work)
        $r = Run @('skeleton', '-Step', 'plan', '-Staging', $Out, '-Work', $work)
        if (-not $r.json) { Fail 'skeleton plan' "the skeleton plan printed no line: $($r.tail)" }
        if (Lock-Refusal $r.json $r.tail) { Reply-Line $r.json }
        $saved = Save-Jobs $work $r.json
        Reply-Line $r.json @{ jobs = $saved.count; file = $saved.file }
    }
    'skeleton-finish' {
        $work = At "$(Opt 'Work')"
        $argv = @('skeleton', '-Step', 'apply', '-Staging', $Out, '-Work', $work)
        if (Opt 'Decisions') { $argv += @('-Decisions', "$(Opt 'Decisions')") }
        $argv += '-Apply'
        $r = Run $argv
        if (-not $r.json) { Fail 'skeleton apply' "the skeleton apply printed no line: $($r.tail)" }
        Reply-Line $r.json
    }
    'figures' {
        $argv = @('figures', '-CitationKey', "$(Opt 'CitationKey')", '-Staging', $Out)
        if (Opt 'FileId') { $argv += @('-FileId', "$(Opt 'FileId')") }
        $argv += @('-Report', "$(At $Out)\figures-report.json", '-Apply')
        $r = Run $argv
        if (-not $r.json) { Fail 'show figures' "the figure pass printed no line: $($r.tail)" }
        Reply-Line $r.json
    }
    'bridge-start' {
        $work = At "$(Opt 'Work')"
        [void][System.IO.Directory]::CreateDirectory($work)
        $argv = @('bridge', '-Step', 'plan', '-Staging', $Out, '-CitationKey', "$(Opt 'CitationKey')", '-Work', $work,
            '-Profile', "$(Opt 'Profile')", '-MaxSources', "$(Opt 'MaxSources' 60)", '-Candidates', "$(Opt 'Candidates' 5)")
        if (Opt 'Books') { $argv += @('-Books', "$(Opt 'Books')") }
        $r = Run $argv
        if (-not $r.json) { Fail 'bridge plan' "the Bridge plan printed no line: $($r.tail)" }
        if (Lock-Refusal $r.json $r.tail) { Reply-Line $r.json }
        $saved = Save-Jobs $work $r.json
        Reply-Line $r.json @{ jobs = $saved.count; file = $saved.file }
    }
    'bridge-report' {
        $work = At "$(Opt 'Work')"
        $r = Run @('bridge', '-Step', 'report', '-Staging', $Out, '-CitationKey', "$(Opt 'CitationKey')", '-Work', $work)
        if (-not $r.json) { Fail 'bridge report' "the Bridge report printed no line: $($r.tail)" }
        Reply-Line $r.json
    }
    'bridge-apply' {
        $work = At "$(Opt 'Work')"
        $argv = @('bridge', '-Step', 'apply', '-Staging', $Out, '-CitationKey', "$(Opt 'CitationKey')", '-Work', $work,
            '-ReportId', "$(Opt 'ReportId')")
        if (Opt 'Accept') { $argv += @('-Accept', "$(Opt 'Accept')") }
        if (Opt 'AcceptEarlier') { $argv += @('-AcceptEarlier', "$(Opt 'AcceptEarlier')") }
        $argv += '-Apply'
        $r = Run $argv
        if (-not $r.json) { Fail 'bridge apply' "the Bridge apply printed no line: $($r.tail)" }
        Reply-Line $r.json
    }
    'audit' {
        $argv = @('audit')
        if (Opt 'CitationKey') { $argv += @('-CitationKey', "$(Opt 'CitationKey')") }
        $argv += @('-Out', $Out)
        $r = Run $argv
        if (-not $r.json) { Fail 'audit cards' "the audit printed no line: $($r.tail)" }
        Reply-Line $r.json
    }
    'finalize-plan' { $r = Run @('finalize', '-Step', 'plan', '-Out', $Out); if (-not $r.json) { Fail 'finalize plan' $r.tail } ; Reply-Line $r.json }
    'finalize-changes' { $r = Run @('finalize', '-Step', 'changes', '-Out', $Out); if (-not $r.json) { Fail 'finalize changes' $r.tail } ; Reply-Line $r.json }
    'finalize-scope' {
        $argv = @('finalize', '-Step', 'scope', '-Out', $Out)
        if (Opt 'FixWork') { $argv += @('-FixWork', "$(Opt 'FixWork')") }
        $r = Run $argv
        if (-not $r.json) { Fail 'finalize scope' $r.tail }
        Reply-Line $r.json
    }
    'finalize-refresh' {
        $argv = @('finalize', '-Step', 'refresh', '-Out', $Out, '-Layer', "$(Opt 'Layer')")
        if (Opt 'Report') { $argv += @('-Report', "$(Opt 'Report')") }
        $r = Run $argv
        if (-not $r.json) { Fail "finalize refresh $(Opt 'Layer')" $r.tail }
        Reply-Line $r.json
    }
    'finalize-final' { $r = Run @('finalize', '-Step', 'final', '-Out', $Out); if (-not $r.json) { Fail 'finalize final' $r.tail } ; Reply-Line $r.json }
    'term-link' {
        $round = [int]"$(Opt 'Round' 0)"
        $occ = "$(Opt 'Occurrences' 'first')"
        $cards = @(Opt 'Cards' @())
        $scoped = @()
        if ($round -gt 0) { $scoped += @('-Round', "$round") }
        if ($cards.Count) { $scoped += @('-Cards', ($cards -join ',')) }
        $plan1 = Run (@('term-link', '-Step', 'plan', '-Staging', $Out, '-Occurrences', $occ) + $scoped)
        if (-not $plan1.json) { Fail 'term-link plan' "the Term-link plan printed no line: $($plan1.tail)" }
        if ($plan1.json.ok -ne $true) { Fail 'term-link plan' "$($plan1.json.error)" $plan1.json }
        $plan = $plan1.json
        $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'termlink', '-Take', '1', '-Owner', 'term-link', '-Wait', '60', '-Lease', '3600')
        $units = @()
        if ($claim.json -and $claim.json.units -is [array]) { $units = @($claim.json.units | Where-Object { $_ -and "$($_.id)" -like 'termlink/*' }) }
        if (-not $units.Count -and -not $claim.json) { Fail 'term-link claim' "the claim printed no line: $($claim.tail)" }
        $unit = @($units) | Select-Object -First 1
        if (-not $unit) {
            # No ready unit: the pass is already complete, or the work it depends on is not done.
            $gate = Gate 'termlink'
            $reason = if ($gate.complete -eq $true) { 'the Term-link pass is already complete' } else { 'the Term-link unit is not ready: its dependencies are not done' }
            Reply @{ ok = $true; status = 'skipped'; reason = $reason; plan = $plan }
        }
        $applied = Run (@('term-link', '-Step', 'apply', '-Staging', $Out, '-Occurrences', $occ) + $scoped + @('-Token', "$($unit.token)", '-Apply'))
        $summary = $null
        if ($applied.json -and $applied.json.step -eq 'apply') { $summary = $applied.json }
        if (-not $summary -or $summary.complete -ne $true) {
            $said = if ($summary -and $summary.code) { "$($summary.code)" } else { $applied.tail }
            [void](Fail-Unit "$($unit.id)" ([int]$unit.token) "$($unit.input_hash)" "the Term-link pass stopped as $said" '')
            Reply @{ ok = $false; status = 'failed'; unit = "$($unit.id)"; error = (Tail "the Term-link pass stopped as $said") }
        }
        $report = if ($summary.report) { "$($summary.report)" } else { Join-Path $Out 'term-link\apply.json' }
        $done = Run @('ledger', 'done', '-Out', $Out, '-Unit', "$($unit.id)", '-Token', "$($unit.token)", '-Outputs', $report)
        if (-not $done.json -or "$($done.json.state)" -ne 'done') {
            [void](Fail-Unit "$($unit.id)" ([int]$unit.token) "$($unit.input_hash)" "the Term-link completion was refused: $($done.tail)" '')
            Reply @{ ok = $false; status = 'failed'; unit = "$($unit.id)"; error = (Tail "the Term-link completion was refused: $($done.tail)") }
        }
        $plan2 = Run (@('term-link', '-Step', 'plan', '-Staging', $Out, '-Occurrences', $occ) + $scoped)
        Reply @{ ok = $true; status = 'done'; unit = "$($unit.id)"; plan = $plan; counts = $summary.counts; no_op = ($summary.no_op -eq $true); completion = $done.json; second = $(if ($plan2.json) { @{ cards_changed = $plan2.json.cards_changed; cards_refused = $plan2.json.cards_refused } } else { $null }) }
    }
    'cleanup-start' {
        $r = Run @('cleanup-run', '-Out', $Out, '-Step', 'plan')
        if (-not $r.json) { Fail 'cleanup plan' "the cleanup plan printed no line: $($r.tail)" }
        Reply-Line $r.json @{ plan_file = (Join-Path $Out 'cleanup\settle-plan.json'); results_file = (Join-Path $Out 'cleanup\settle-results.json') }
    }
    'cleanup-apply' {
        $r = Run @('cleanup-run', '-Out', $Out, '-Step', 'apply', '-Owner', 'cleanup-run')
        if ($r.json) { Reply @{ ok = $true; fallback = $false; provider = $r.json; ready = $r.json.ready } }
        # Without the provider results (the settle tool is missing or failed), the units whose conversations are
        # already gone still settle, and the stage reports the provider batch as not run.
        $fallback = Run @('cleanup-finish', '-Out', $Out, '-Ready', '-Owner', 'cleanup-run')
        if (-not $fallback.json) { Fail 'cleanup apply' "neither the provider results nor the cleanup-finish line answered: $($r.tail) | $($fallback.tail)" }
        Reply @{ ok = $true; fallback = $true; provider = $null; provider_error = (Tail $r.tail); ready = $fallback.json }
    }
    'cleanup-report' {
        $r = Run @('ledger', 'report', '-Out', $Out)
        if (-not $r.json) { Fail 'report the run from the ledger' "the run report printed no line: $($r.tail)" }
        Reply-Line $r.json
    }
}
Fail $Step 'the step named no action'


