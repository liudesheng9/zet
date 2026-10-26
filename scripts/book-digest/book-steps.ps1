<#
.SYNOPSIS
  The book-level digest stages' deterministic command chains, one step per call (#176, #180). A DeepSeek drive job
  runs one step and replies with its last line, so no Claude agent runs a single command.

  pwsh -NoProfile -File book-steps.ps1 -Step <step> -Out <staging> [-Json '<options>'] [-Tool zt-digest]

  prepare           the book preparation (mode full), the one supported resume (mode resume) or a Term backfill
                    (modes term-cards and term-link)                     -> {ok, groups, citation_key, ...}
  link-prepare      the Link pass's receipt-owned gap set counts        -> {ok, pending, no_gap, blocked, ...}
  publish           a stage's own book revision (`finalize -Step stage`) -> its publication counts
  cards-start       plan the card cycle and list its jobs               -> cards\out\jobs.json
  cards-apply       apply every ready draft and run the check            -> cards\out\jobs.json
  cards-finish      apply every ready draft and report the cycle         -> the cycle's flat counts
  unit-fail         record one claimed unit's fault                      -> {ok, unit, state}
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

  #180: each step prints ONE compact JSON line: `ok` first, then the counts and file paths the module reads, at
  most 400 characters. Its full data (the tool's own line, the per-card or per-book rows, the report body) goes
  to <staging>\drive-lines\<step>.json, named in the line as `detail`; a module reads no nested object and no
  list from a line. A step a tool command failed answers {ok:false, step, error} and exits 0: its caller reads
  the line and stops the stage. A command the owner's Lock refused answers the archive's typed refusal
  ({ok:false, code:"card_locked", message, tree, lock_id, ...}) with its refusal in the detail file.
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
. (Join-Path $PSScriptRoot 'step-line.ps1')
$Out = $Out.TrimEnd('\', '/')
$LineName = $Step
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
# The book's Citation key: the step's own option, else the prepared run's manifest.json. A resume names only `out`,
# so this is the one place its stages find the book's identity, and an empty -CitationKey or -Book never reaches
# the tool (#181).
function Book-Key([string]$Value) {
    $v = "$Value".Trim()
    if ($v) { return $v }
    $manifest = Join-Path $Out 'manifest.json'
    if (Test-Path -LiteralPath $manifest) {
        try {
            $doc = Get-Content -LiteralPath $manifest -Raw -Encoding utf8 | ConvertFrom-Json -Depth 8
            if ($doc -and "$($doc.citation_key)".Trim()) { return "$($doc.citation_key)".Trim() }
        } catch { }
    }
    return $v
}

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
# The step's answer: its compact line and the detail file beside it (#180).
function Reply($Value, $Detail = $null) { Send-StepLine -Line $Value -Detail $Detail -Name $LineName -Out $Out }
# A failed step: one `{ok:false, step, error}` line; the full tool output is in the detail file.
function Fail([string]$Name, [string]$Reason, $Json = $null) { Send-StepFail -Step $Name -Reason $Reason -Name $LineName -Out $Out -Detail $Json }
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
# A refusal as the compact line's flat fields: its sentence, tree and Lock id; the whole refusal is in the detail.
function Refuse($Json, [string]$Text) {
    $said = Lock-Refusal $Json $Text
    if (-not $said) { return $null }
    $line = @{ ok = $false; code = 'card_locked'; error = "card_locked: $($said.message)" }
    foreach ($k in @('message', 'tree', 'lock_id', 'unit')) { if ($null -ne $said.$k) { $line[$k] = "$($said.$k)" } }
    return [pscustomobject]@{ line = $line; raw = $said }
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
# The stage names a list of `-Json` refresh entries: the applicable stages to refresh and the ones whose own
# stage owns that content (#180: a name list, never the entries themselves).
function Names($Entries, [string]$Field = 'layer') {
    return (@(@($Entries) | Where-Object { $_ -and $_.$Field } | ForEach-Object { "$($_.$Field)" }) -join ',')
}
function Refresh-Names($Entries) {
    $todo = @(@($Entries) | Where-Object { $_ -and $_.applicable -eq $true -and "$($_.layer)" -ne 'audit' })
    $owned = @($todo | Where-Object { $_.stage_owned -eq $true })
    return [pscustomobject]@{
        refresh = (@($todo | ForEach-Object { "$($_.layer)" }) -join ',')
        owned = (@($owned | ForEach-Object { "$($_.layer)" }) -join ',')
    }
}

switch ($Step) {
    'prepare' {
        $mode = "$(Opt 'Mode' 'full')"
        $key = Book-Key "$(Opt 'CitationKey' '')"
        if ($mode -eq 'resume') {
            # R278-R285: the one supported resume; the retained groups and identity come from the ledger's own
            # summary, and the digest gate answers a summary that carries no layer counts.
            $facts = Run @('ledger', 'resume', '-Out', $Out)
            if (-not $facts.json) { Fail 'resume' "the ledger resume printed no line: $($facts.tail)"; break }
            if (@('resumed', 'nothing_to_resume') -notcontains "$($facts.json.status)") {
                Fail 'resume' ("the retained run cannot resume: " + $(if ($facts.json.error) { "$($facts.json.error)" } else { $facts.tail })) $facts.json
                break
            }
            $status = Run @('ledger', 'status', '-Out', $Out, '-Summary')
            if (-not $status.json) { Fail 'resume status' "the ledger summary printed no line: $($status.tail)"; break }
            $view = $status.json
            $groups = 0
            if ($view.layers -and $view.layers.digest) { $groups = [int]$view.layers.digest.total }
            if ($groups -le 0) {
                $gate = Run @('ledger', 'gate', '-Out', $Out, '-Layer', 'digest', '-Brief')
                if ($gate.json -and $gate.json.counts) {
                    $groups = [int]$gate.json.counts.total
                    if ($groups -le 0) { $groups = [int]$gate.json.counts.declared }
                }
            }
            if ($groups -le 0) { Fail 'resume' 'mode resume found no retained digest groups; prepare the book again' $view; break }
            $resume = $facts.json
            Reply @{
                ok = $true; groups = $groups; resumed = ("$($resume.status)" -eq 'resumed')
                citation_key = "$($view.citation_key)"; variant = "$($view.variant)"
                invocation = [int]$resume.invocation; resume_count = [int]$resume.resume_count
                reset = (Count-Of $resume.reset); reevaluated = (Count-Of $resume.reevaluated); reclaimed = (Count-Of $resume.reclaimed)
                downstream = (Count-Of $resume.closure.downstream)
                max_attempts = [int]$view.settings.max_attempts; dsh_run_cap = [int]$view.settings.dsh_run_cap
                off = (@($view.run_options.PSObject.Properties | Where-Object { $_.Value -eq $false } | ForEach-Object { $_.Name }) -join ',')
            } @{ resume = $resume; settings = $view.settings; run_options = $view.run_options; status = $view }
            break
        }
        if ($mode -eq 'term-cards' -or $mode -eq 'term-link') {
            # S25 (#168): the backfill's own preparation. `-Sections` reuses or reconstructs the staged source
            # slices the Term-card branch reads; `-Validate` only inspects the already digested book.
            $flag = if ($mode -eq 'term-cards') { '-Sections' } else { '-Validate' }
            $r = Run @('prepare', '-Out', $Out, $flag, '-Brief')
            if (-not $r.json) { Fail "prepare $mode" "the backfill preparation printed no line: $($r.tail)"; break }
            if ($r.json.ok -ne $true) {
                $said = @($r.json.conflicts | ForEach-Object { "$($_.message)" }) -join '; '
                if (-not $said) { $said = "$($r.json.error)" }
                Fail "prepare $mode" $said $r.json
                break
            }
            $b = $r.json
            Reply @{
                ok = $true; step = "prepare $mode"; book = "$($b.book)"
                sections = (Count-Of $b.source.sections); reused = ($b.source.reused -eq $true); rebuilt = (Count-Of $b.source.rebuilt)
                rows = [int]$b.list.rows; legacy = ($b.list.legacy -eq $true)
                heads = [int]$b.tree.heads; parts = [int]$b.tree.parts; occupying = (Count-Of $b.tree.occupying)
                conflicts = (Count-Of $b.conflicts)
            } @{ tool = $b; source = $b.source; list = $b.list; tree = $b.tree; conflicts = $b.conflicts }
            break
        }
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
        if (-not $prepared.json) { Fail 'prepare book' "the preparation printed no line: $($prepared.tail)"; break }
        $refusal = Refuse $prepared.json $prepared.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        if ($prepared.json.ok -ne $true -or [int]$prepared.json.groups -le 0) {
            Fail 'prepare book' ("preparation failed: " + $(if ($prepared.json.error) { "$($prepared.json.error)" } else { ($prepared.json | ConvertTo-Json -Depth 8 -Compress) })) $prepared.json
            break
        }
        $p = $prepared.json
        Reply @{ ok = $true; groups = [int]$p.groups; citation_key = "$($p.citation_key)" } $p
    }
    'gate' {
        $g = Run @('ledger', 'gate', '-Out', $Out, '-Layer', "$(Opt 'Layer')", '-Brief')
        $refusal = Refuse $g.json $g.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        if (-not $g.json) { Fail 'gate' "the ledger gate printed no line: $($g.tail)"; break }
        Reply @{ ok = ($g.json.ok -ne $false); complete = ($g.json.complete -eq $true); total = [int]$g.json.counts.total; done = [int]$g.json.counts.done; open = [int]$g.json.counts.open } $g.json
    }
    'link-prepare' {
        $r = Run @('prepare-link', '-Staging', $Out, '-Brief')
        if (-not $r.json) { Fail 'prepare-link' "the Link preparation printed no line: $($r.tail)"; break }
        $l = $r.json
        Reply @{
            ok = $true; pending = (Count-Of $l.pending); no_gap = (Count-Of $l.no_gap); blocked = (Count-Of $l.blocked)
            gaps = [int]$l.gap_total; gap_total = [int]$l.gap_total; skipped_total = [int]$l.skipped_total
            owned_total = [int]$l.owned_total; unowned = [int]$l.unowned; digest_state = "$($l.digest_state)"
        } $l
    }
    'publish' {
        $layer = "$(Opt 'Layer')"
        # The whole-book Card revision is the ledger's own publication; every other stage publishes its revision
        # through `finalize -Step stage` with its declared artifacts.
        if (Flag 'Ledger') {
            $r = Run @('ledger', 'publish', '-Out', $Out, '-Layer', $layer)
            if (-not $r.json) { Fail "$layer publish" "the ledger publication printed no line: $($r.tail)"; break }
            $u = $r.json
            Reply @{ ok = $true; layer = $layer; unit = "$($u.unit)"; revision = [int]$u.revision; state = 'ready'; complete = $false } $u
            break
        }
        $argv = @('finalize', '-Step', 'stage', '-Out', $Out, '-Layer', $layer)
        if (Opt 'Report') { $argv += @('-Report', "$(Opt 'Report')") }
        if (Opt 'Decisions') { $argv += @('-Decisions', "$(Opt 'Decisions')") }
        if (Opt 'Applied') { $argv += @('-Applied', "$(Opt 'Applied')") }
        if (Opt 'Skeleton') { $argv += @('-Skeleton', "$(Opt 'Skeleton')") }
        if (Opt 'Cards') { $argv += @('-Cards', "$(Opt 'Cards')") }
        $r = Run $argv
        if (-not $r.json) { Fail "$layer publish" "the stage publication printed no line: $($r.tail)"; break }
        $refusal = Refuse $r.json $r.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $p = $r.json
        Reply @{ ok = $true; layer = $layer; unit = "$($p.unit)"; revision = [int]$p.revision; state = "$($p.state)"; disposition = "$($p.disposition)"; complete = ($p.complete -eq $true); changed_cards = [int]$p.changed_cards } $p
    }
    'cards-start' {
        $work = At "$(Opt 'Work')"
        [void][System.IO.Directory]::CreateDirectory($work)
        $argv = @('card', '-Step', 'plan', '-Work', $work, '-Compact', '-Book', (Book-Key "$(Opt 'Book')"), '-Staging', $Out)
        if (Opt 'Rules') { $argv += @('-Rules', "$(Opt 'Rules')") }
        if (Opt 'MaxCards') { $argv += @('-MaxCards', "$(Opt 'MaxCards')") }
        if (Opt 'Sample') { $argv += @('-Sample', "$(Opt 'Sample')") }
        if (Flag 'ModelReview') { $argv += '-ModelReview' }
        if (Opt 'Notes') { $argv += @('-Notes', "$(Opt 'Notes')") }
        if (Flag 'NoOrphans') { $argv += '-NoOrphans' }
        if (Opt 'Cards') { $argv += @('-Cards', "$(Opt 'Cards')") }
        $plan = Run $argv
        if (-not $plan.json) { Fail 'plan' "the card plan printed no line: $($plan.tail)"; break }
        if ($plan.json.ok -eq $false) { Fail 'plan' "$($plan.json.error)" $plan.json; break }
        $refusal = Refuse $plan.json $plan.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $saved = Save-Jobs $work $plan.json
        $p = $plan.json
        Reply @{ ok = $true; picked = [int]$p.picked; with_faults = [int]$p.with_faults; faults = [int]$p.faults; review = [int]$p.review; orphans = [int]$p.orphans; jobs = $saved.count; file = $saved.file } $p
    }
    'cards-apply' {
        $work = At "$(Opt 'Work')"
        $max = [int]"$(Opt 'MaxJobs' 30)"
        $applied = 0
        $pending = 0
        for ($i = 1; $i -le 100; $i++) {
            $a = Run @('card', '-Step', 'apply', '-Work', $work, '-MaxJobs', "$max")
            if (-not $a.json) { Fail 'apply' "the card apply printed no line: $($a.tail)"; break }
            $refusal = Refuse $a.json $a.tail
            if ($refusal) { Reply $refusal.line $refusal.raw; break }
            $applied += [int]$a.json.applied_jobs
            $pending = [int]$a.json.pending_jobs
            if ($pending -le 0) { break }
        }
        $check = Run @('card', '-Step', 'check', '-Work', $work, '-Compact')
        if (-not $check.json) { Fail 'check' "the card check printed no line: $($check.tail)"; break }
        $refusal = Refuse $check.json $check.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $saved = Save-Jobs $work $check.json
        $c = $check.json
        Reply @{ ok = $true; applied = $applied; faults = [int]$c.faults; orphans = [int]$c.orphans; jobs = $saved.count; file = $saved.file } @{ check = $c; pending = $pending }
    }
    'cards-finish' {
        $work = At "$(Opt 'Work')"
        $max = [int]"$(Opt 'MaxJobs' 30)"
        $applied = 0
        for ($i = 1; $i -le 100; $i++) {
            $a = Run @('card', '-Step', 'apply', '-Work', $work, '-MaxJobs', "$max")
            if (-not $a.json) { Fail 'apply' "the card apply printed no line: $($a.tail)"; break }
            $refusal = Refuse $a.json $a.tail
            if ($refusal) { Reply $refusal.line $refusal.raw; break }
            $applied += [int]$a.json.applied_jobs
            if ([int]$a.json.pending_jobs -le 0) { break }
        }
        $report = Run @('card', '-Step', 'report', '-Work', $work)
        if (-not $report.json) { Fail 'report' "the card report printed no line: $($report.tail)"; break }
        $refusal = Refuse $report.json $report.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $r = $report.json
        Reply @{
            ok = $true; applied = $applied; cards = [int]$r.cards; picked = [int]$r.picked
            faults_before = [int]$r.faults_before; faults_after = [int]$r.faults_after
            splits = [int]$r.splits; merges = [int]$r.merges; refused = @($r.refused).Count
            orphan_links = [int]$r.orphan_links; orphans_after = [int]$r.orphans_after
            quality_complete = ($r.quality_complete -eq $true)
        } $r
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
        if (-not $published.json -or -not $published.json.unit) { Fail 'publish' "the cards-book revision was not registered: $($published.tail)"; break }
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
        if (-not $unit) {
            $g = Run @('ledger', 'gate', '-Out', $Out, '-Layer', 'cards-book', '-Brief')
            Reply @{ ok = $true; unit = $null; revision = $revision; waiting = $true; open = [int]$g.json.counts.open } @{ gate = $g.json; claim = $claim.json }
            break
        }
        Reply @{ ok = $true; unit = "$($unit.id)"; token = [int]$unit.token; input_hash = "$($unit.input_hash)"; revision = $revision } @{ unit = $unit; claim = $claim.json }
    }
    'cards-book-finish' {
        $unit = "$(Opt 'Unit')"
        $token = [int]"$(Opt 'Token' 0)"
        $work = At "$(Opt 'Work')"
        $argv = @('book-cards-finish', '-Out', $Out, '-Work', $work, '-Unit', $unit, '-Token', "$token")
        if (Flag 'CheckOnly') { $argv += '-CheckOnly' }
        $finish = Run $argv
        if ($finish.json -and ($finish.json.state -eq 'done' -or $finish.json.complete -eq $true)) {
            $f = $finish.json
            Reply @{ ok = $true; unit = $unit; state = "$($f.state)"; disposition = "$($f.disposition)"; complete = ($f.complete -eq $true); from = "$($f.from)" } $f
            break
        }
        # A cut completion line asks the ledger before any fault: a unit it holds done is done.
        $read = Run @('ledger', 'status', '-Out', $Out, '-Units', $unit)
        $held = @(@($read.json.units) | Where-Object { $_ -and $_.id -eq $unit }) | Select-Object -First 1
        if ($held -and ($held.state -eq 'done' -or $held.state -eq 'noop')) {
            Reply @{ ok = $true; unit = $unit; state = "$($held.state)"; disposition = "$($held.disposition)"; from = 'ledger' } @{ held = $held; finish = $finish.json }
            break
        }
        $cause = if ($finish.json -and $finish.json.fault) { "$($finish.json.fault)" } elseif ($finish.json -and $finish.json.error) { "$($finish.json.error)" } else { $finish.tail }
        $status = Fail-Unit $unit $token "$(Opt 'InputHash' '')" $cause
        Reply @{ ok = $false; step = 'book-cards-finish'; unit = $unit; state = $status; error = (Tail $cause) } @{ finish = $finish.json; held = $held }
    }
    'organize-survey' {
        $work = At "$(Opt 'Work')"
        $r = Run @('organize-survey', '-Staging', $Out, '-Work', $work)
        if (-not $r.json) { Fail 'organize survey' "the survey printed no line: $($r.tail)"; break }
        $s = $r.json
        $refusal = Refuse $r.json $r.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        Reply @{ ok = $true; report = "$($s.report)"; merges = (Count-Of $s.merges); moves = (Count-Of $s.moves); issues = (Count-Of $s.issues) } $s
    }
    'organize-apply' {
        $work = At "$(Opt 'Work')"
        $r = Run @('organize-apply', '-Work', $work)
        if (-not $r.json) { Fail 'organize apply' "the apply printed no line: $($r.tail)"; break }
        $a = $r.json
        Reply @{ ok = $true; merges = [int]$a.merges; moves = [int]$a.moves; refused = [int]$a.refused; notes = [int]$a.notes; thesis = ($a.thesis -ne $false) } $a
    }
    'skeleton-start' {
        $work = At "$(Opt 'Work')"
        [void][System.IO.Directory]::CreateDirectory($work)
        $r = Run @('skeleton', '-Step', 'plan', '-Staging', $Out, '-Work', $work)
        if (-not $r.json) { Fail 'skeleton plan' "the skeleton plan printed no line: $($r.tail)"; break }
        $refusal = Refuse $r.json $r.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $saved = Save-Jobs $work $r.json
        Reply @{ ok = $true; jobs = $saved.count; file = $saved.file } $r.json
    }
    'skeleton-finish' {
        $work = At "$(Opt 'Work')"
        $argv = @('skeleton', '-Step', 'apply', '-Staging', $Out, '-Work', $work)
        if (Opt 'Decisions') { $argv += @('-Decisions', "$(Opt 'Decisions')") }
        $argv += '-Apply'
        $r = Run $argv
        if (-not $r.json) { Fail 'skeleton apply' "the skeleton apply printed no line: $($r.tail)"; break }
        $refusal = Refuse $r.json $r.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $s = $r.json
        Reply @{ ok = ($s.ok -ne $false); structure = "$($s.written.structure)"; thesis = "$($s.thesis)"; argument = [int]$s.argument; models = [int]$s.models; events = [int]$s.events; problems = (Count-Of $s.problems) } $s
    }
    'figures' {
        $argv = @('figures', '-CitationKey', (Book-Key "$(Opt 'CitationKey')"), '-Staging', $Out)
        if (Opt 'FileId') { $argv += @('-FileId', "$(Opt 'FileId')") }
        $argv += @('-Report', "$(At $Out)\figures-report.json", '-Apply')
        $r = Run $argv
        if (-not $r.json) { Fail 'show figures' "the figure pass printed no line: $($r.tail)"; break }
        $f = $r.json
        Reply @{ ok = ($f.ok -ne $false); shown = [int]$f.shown; cards = [int]$f.cards; figures = [int]$f.figures; labelled = [int]$f.labelled; literature = ($f.literature -eq $true); missing = (Count-Of $f.missing) } $f
    }
    'bridge-start' {
        $work = At "$(Opt 'Work')"
        [void][System.IO.Directory]::CreateDirectory($work)
        $argv = @('bridge', '-Step', 'plan', '-Staging', $Out, '-CitationKey', (Book-Key "$(Opt 'CitationKey')"), '-Work', $work,
            '-Profile', "$(Opt 'Profile')", '-MaxSources', "$(Opt 'MaxSources' 60)", '-Candidates', "$(Opt 'Candidates' 5)")
        if (Opt 'Books') { $argv += @('-Books', "$(Opt 'Books')") }
        $r = Run $argv
        if (-not $r.json) { Fail 'bridge plan' "the Bridge plan printed no line: $($r.tail)"; break }
        $refusal = Refuse $r.json $r.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $saved = Save-Jobs $work $r.json
        Reply @{ ok = ($r.json.ok -ne $false); jobs = $saved.count; file = $saved.file } $r.json
    }
    'bridge-report' {
        $work = At "$(Opt 'Work')"
        $r = Run @('bridge', '-Step', 'report', '-Staging', $Out, '-CitationKey', (Book-Key "$(Opt 'CitationKey')"), '-Work', $work)
        if (-not $r.json) { Fail 'bridge report' "the Bridge report printed no line: $($r.tail)"; break }
        $b = $r.json
        Reply @{ ok = ($b.ok -ne $false); step = "$($b.step)"; report_id = "$($b.report_id)"; proposals = (Count-Of $b.proposals); new = [int]$b.counts.new; earlier = [int]$b.counts.earlier; refused = [int]$b.counts.refused } $b
    }
    'bridge-apply' {
        $work = At "$(Opt 'Work')"
        $argv = @('bridge', '-Step', 'apply', '-Staging', $Out, '-CitationKey', (Book-Key "$(Opt 'CitationKey')"), '-Work', $work,
            '-ReportId', "$(Opt 'ReportId')")
        if (Opt 'Accept') { $argv += @('-Accept', "$(Opt 'Accept')") }
        if (Opt 'AcceptEarlier') { $argv += @('-AcceptEarlier', "$(Opt 'AcceptEarlier')") }
        $argv += '-Apply'
        $r = Run $argv
        if (-not $r.json) { Fail 'bridge apply' "the Bridge apply printed no line: $($r.tail)"; break }
        $refusal = Refuse $r.json $r.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $a = $r.json
        Reply @{ ok = ($a.ok -ne $false); status = "$($a.status)"; applied = [int]$a.applied; refused = (Count-Of $a.refused); unit = "$($a.publication.unit)"; state = "$($a.publication.state)" } $a
    }
    'audit' {
        $argv = @('audit')
        $auditKey = Book-Key "$(Opt 'CitationKey')"
        if ($auditKey) { $argv += @('-CitationKey', $auditKey) }
        $argv += @('-Out', $Out)
        $r = Run $argv
        if (-not $r.json) { Fail 'audit cards' "the audit printed no line: $($r.tail)"; break }
        $a = $r.json
        $books = @($a.books)
        $sum = { param($name) (@($books | ForEach-Object { [int]$_.$name }) | Measure-Object -Sum).Sum }
        $terms = $a.terms
        Reply @{
            ok = ($a.ok -ne $false); books = $books.Count
            cards = [int](& $sum 'cards'); style_faults = [int](& $sum 'style_faults'); layer_faults = [int](& $sum 'layer_faults')
            orphans = [int](& $sum 'orphans'); no_source = [int](& $sum 'no_source'); unkinded = [int](& $sum 'unkinded')
            not_self_sufficient = [int](& $sum 'not_self_sufficient'); models = [int](& $sum 'models')
            explained = [int](& $sum 'explained'); derivations = [int](& $sum 'derivations'); figures_missing = [int](& $sum 'figures_missing')
            terms = "$($terms.status)"; term_published = [int]$terms.published; term_rows = [int]$terms.rows
            term_heads = [int]$terms.heads; term_uses = [int]$terms.uses; term_faults = [int]$terms.faults
            hubs_missing = [int]$a.hubs_missing; broken_links = [int]$a.broken_links
        } $a
    }
    'finalize-plan' {
        $r = Run @('finalize', '-Step', 'plan', '-Out', $Out)
        if (-not $r.json) { Fail 'finalize plan' $r.tail; break }
        $refusal = Refuse $r.json $r.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $p = $r.json
        $names = Refresh-Names $p.refresh
        Reply @{
            ok = $true; step = 'plan'; branch = "$($p.branch)"; audit_unit = "$($p.audit.unit)"; audit_revision = [int]$p.audit.revision
            graph_revision = [int]$p.graph_revision; faults = [int]$p.faults.total; hard = [int]$p.faults.hard; advisory = [int]$p.faults.advisory
            fix = [int]$p.counts.fix; conflict = [int]$p.counts.conflict; blocked = [int]$p.counts.blocked; stale = [int]$p.counts.stale
            cycle_permitted = [int]$p.fix_cycle.permitted; cycle_used = [int]$p.fix_cycle.used
            cycle_cards = (Count-Of $p.fix_cycle.cards); cycle_rules = ((@($p.fix_cycle.rules | Where-Object { $_ }) -join ',')); cycle_card_list = "$($p.fix_cycle.card_list)"
            refresh = $names.refresh; owned = $names.owned
        } $p
    }
    'finalize-changes' {
        $r = Run @('finalize', '-Step', 'changes', '-Out', $Out)
        if (-not $r.json) { Fail 'finalize changes' $r.tail; break }
        $refusal = Refuse $r.json $r.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $c = $r.json
        $names = Refresh-Names $c.refresh
        Reply @{
            ok = $true; step = 'changes'; finalized = ($c.finalized -eq $true); graph_revision = [int]$c.graph_revision
            cards = [int]$c.counts.cards; trees = [int]$c.counts.trees
            changes = "$($c.changes)"; cycle_report = "$($c.cycle.report)"
            term_applicable = ($c.term_link.applicable -eq $true); term_scope = "$($c.term_link.scope)"; term_round = [int]$c.term_link.round
            term_cards = (Count-Of $c.term_link.cards); term_reason = "$($c.term_link.reason)"
            refresh = $names.refresh; owned = $names.owned
        } $c
    }
    'finalize-scope' {
        $argv = @('finalize', '-Step', 'scope', '-Out', $Out)
        if (Opt 'FixWork') { $argv += @('-FixWork', "$(Opt 'FixWork')") }
        $r = Run $argv
        if (-not $r.json) { Fail 'finalize scope' $r.tail; break }
        $refusal = Refuse $r.json $r.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $c = $r.json
        $names = Refresh-Names $c.refresh
        Reply @{
            ok = $true; step = 'scope'; graph_revision = [int]$c.graph_revision
            cards = [int]$c.counts.cards; trees = [int]$c.counts.trees
            changes = "$($c.changes)"; cycle_report = "$($c.cycle.report)"
            term_applicable = ($c.term_link.applicable -eq $true); term_scope = "$($c.term_link.scope)"; term_round = [int]$c.term_link.round
            term_cards = (Count-Of $c.term_link.cards); term_reason = "$($c.term_link.reason)"
            refresh = $names.refresh; owned = $names.owned
        } $c
    }
    'finalize-refresh' {
        $argv = @('finalize', '-Step', 'refresh', '-Out', $Out, '-Layer', "$(Opt 'Layer')")
        if (Opt 'Report') { $argv += @('-Report', "$(Opt 'Report')") }
        $r = Run $argv
        if (-not $r.json) { Fail "finalize refresh $(Opt 'Layer')" $r.tail; break }
        $refusal = Refuse $r.json $r.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $f = $r.json
        Reply @{ ok = ($f.ok -ne $false); layer = "$($f.layer)"; unit = "$($f.unit)"; revision = [int]$f.revision; state = "$($f.state)"; complete = ($f.complete -eq $true); changed_cards = [int]$f.changed_cards } $f
    }
    'finalize-final' {
        $r = Run @('finalize', '-Step', 'final', '-Out', $Out)
        if (-not $r.json) { Fail 'finalize final' $r.tail; break }
        $refusal = Refuse $r.json $r.tail
        if ($refusal) { Reply $refusal.line $refusal.raw; break }
        $f = $r.json
        $t = $f.terms
        $round = $f.term_link
        Reply @{
            ok = ($f.ok -ne $false); status = "$($f.status)"; branch = "$($f.branch)"; graph_revision = [int]$f.graph_revision
            audit_unit = "$($f.audit.final.unit)"; audit_revision = [int]$f.audit.final.revision
            scheduled_unit = "$($f.audit.scheduled.unit)"; scheduled_revision = [int]$f.audit.scheduled.revision
            residual = [int]$f.faults.residual; infrastructure = (Count-Of $f.infrastructure)
            changes_cards = [int]$f.changes.cards; changes_trees = [int]$f.changes.trees
            term_applicable = ($round.applicable -ne $false); term_status = "$($round.status)"; term_round = [int]$round.round
            term_scope = "$($round.scope)"; term_unit = "$($round.unit)"; term_state = "$($round.state)"; term_reason = "$($round.reason)"
            term_no_op = ($round.no_op -eq $true); term_cards_changed = [int]$round.counts.cards_changed; term_links_added = [int]$round.counts.links_added
            terms_status = "$($t.status)"; terms_published = [int]$t.published; terms_rows = [int]$t.rows; terms_heads = [int]$t.heads
            terms_uses = [int]$t.uses; terms_faults = [int]$t.faults; terms_complete = ($t.complete -eq $true)
        } $f
    }
    'term-link' {
        $round = [int]"$(Opt 'Round' 0)"
        $occ = "$(Opt 'Occurrences' 'first')"
        $cards = @(Opt 'Cards' @())
        # #180: a finalization round's Cards come from the immutable manifest this step reads, never from a line.
        if (-not $cards.Count -and $round -gt 0) {
            $manifest = Join-Path $Out 'finalize\changes.json'
            if (Test-Path -LiteralPath $manifest) {
                $doc = Get-Content -LiteralPath $manifest -Raw -Encoding utf8 | ConvertFrom-Json -Depth 32
                $cards = @($doc.term_link.cards | Where-Object { $_ })
            }
        }
        $scoped = @()
        if ($round -gt 0) { $scoped += @('-Round', "$round") }
        if ($cards.Count) { $scoped += @('-Cards', ($cards -join ',')) }
        $plan1 = Run (@('term-link', '-Step', 'plan', '-Staging', $Out, '-Occurrences', $occ) + $scoped)
        if (-not $plan1.json) { Fail 'term-link plan' "the Term-link plan printed no line: $($plan1.tail)"; break }
        if ($plan1.json.ok -ne $true) { Fail 'term-link plan' "$($plan1.json.error)" $plan1.json; break }
        $plan = $plan1.json
        $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'termlink', '-Take', '1', '-Owner', 'term-link', '-Wait', '60', '-Lease', '3600')
        $units = @()
        if ($claim.json -and $claim.json.units -is [array]) { $units = @($claim.json.units | Where-Object { $_ -and "$($_.id)" -like 'termlink/*' }) }
        if (-not $units.Count -and -not $claim.json) { Fail 'term-link claim' "the claim printed no line: $($claim.tail)"; break }
        $unit = @($units) | Select-Object -First 1
        if (-not $unit) {
            # No ready unit: the pass is already complete, or the work it depends on is not done.
            $gate = Run @('ledger', 'gate', '-Out', $Out, '-Layer', 'termlink', '-Brief')
            $reason = if ($gate.json -and $gate.json.complete -eq $true) { 'the Term-link pass is already complete' } else { 'the Term-link unit is not ready: its dependencies are not done' }
            Reply @{ ok = $true; status = 'skipped'; reason = $reason; plan_revision = [int]$plan.plan_revision } @{ plan = $plan; gate = $gate.json }
            break
        }
        $applied = Run (@('term-link', '-Step', 'apply', '-Staging', $Out, '-Occurrences', $occ) + $scoped + @('-Token', "$($unit.token)", '-Apply'))
        $summary = $null
        if ($applied.json -and $applied.json.step -eq 'apply') { $summary = $applied.json }
        if (-not $summary -or $summary.complete -ne $true) {
            $said = if ($summary -and $summary.code) { "$($summary.code)" } else { $applied.tail }
            [void](Fail-Unit "$($unit.id)" ([int]$unit.token) "$($unit.input_hash)" "the Term-link pass stopped as $said" '')
            Reply @{ ok = $false; status = 'failed'; unit = "$($unit.id)"; error = (Tail "the Term-link pass stopped as $said") } @{ apply = $applied.json; plan = $plan }
            break
        }
        $report = if ($summary.report) { "$($summary.report)" } else { Join-Path $Out 'term-link\apply.json' }
        $done = Run @('ledger', 'done', '-Out', $Out, '-Unit', "$($unit.id)", '-Token', "$($unit.token)", '-Outputs', $report)
        if (-not $done.json -or "$($done.json.state)" -ne 'done') {
            [void](Fail-Unit "$($unit.id)" ([int]$unit.token) "$($unit.input_hash)" "the Term-link completion was refused: $($done.tail)" '')
            Reply @{ ok = $false; status = 'failed'; unit = "$($unit.id)"; error = (Tail "the Term-link completion was refused: $($done.tail)") } @{ done = $done.json; apply = $summary }
            break
        }
        $plan2 = Run (@('term-link', '-Step', 'plan', '-Staging', $Out, '-Occurrences', $occ) + $scoped)
        Reply @{
            ok = $true; status = 'done'; unit = "$($unit.id)"
            cards_changed = [int]$summary.counts.cards_changed; links_added = [int]$summary.counts.links_added
            no_op = ($summary.no_op -eq $true); complete = ($summary.complete -eq $true)
            second_cards_changed = [int]$plan2.json.cards_changed; second_cards_refused = [int]$plan2.json.cards_refused
        } @{ plan = $plan; counts = $summary.counts; completion = $done.json; second = @{ cards_changed = $plan2.json.cards_changed; cards_refused = $plan2.json.cards_refused }; apply = $summary }
    }
    'cleanup-start' {
        $r = Run @('cleanup-run', '-Out', $Out, '-Step', 'plan')
        if (-not $r.json) { Fail 'cleanup plan' "the cleanup plan printed no line: $($r.tail)"; break }
        $p = $r.json
        Reply @{ ok = $true; deletes = [int]$p.deletes; workspaces = [int]$p.workspaces; evidence_recorded = [int]$p.evidence_recorded; evidence_missing = [int]$p.evidence_missing; plan_file = (Join-Path $Out 'cleanup\settle-plan.json'); results_file = (Join-Path $Out 'cleanup\settle-results.json') } $p
    }
    'cleanup-apply' {
        $r = Run @('cleanup-run', '-Out', $Out, '-Step', 'apply', '-Owner', 'cleanup-run')
        if ($r.json) {
            $p = $r.json
            $ready = $p.ready
            Reply @{
                ok = $true; fallback = $false; deleted = [int]$p.deleted; absent = [int]$p.absent; workspaces_removed = [int]$p.workspaces_removed
                workspaces_kept = [int]$p.workspaces_kept; failed = [int]$p.failed_count; unknown = [int]$p.unknown_count; unrecorded = [int]$p.unrecorded_count
                ready_claimed = [int]$ready.claimed; ready_done = [int]$ready.done; ready_partial = [int]$ready.partial
                ready_retry = [int]$ready.retry_count; ready_failed = [int]$ready.failed_count
                book_unit = "$($ready.book.unit)"; book_ok = ($ready.book.ok -eq $true); book_status = "$($ready.book.status)"
            } @{ provider = $p }
            break
        }
        # Without the provider results (the settle tool is missing or failed), the units whose conversations are
        # already gone still settle, and the stage reports the provider batch as not run.
        $fallback = Run @('cleanup-finish', '-Out', $Out, '-Ready', '-Owner', 'cleanup-run')
        if (-not $fallback.json) { Fail 'cleanup apply' "neither the provider results nor the cleanup-finish line answered: $($r.tail) | $($fallback.tail)"; break }
        $ready = $fallback.json
        Reply @{
            ok = $true; fallback = $true; provider_error = (Tail $r.tail)
            ready_claimed = [int]$ready.claimed; ready_done = [int]$ready.done; ready_partial = [int]$ready.partial
            ready_retry = [int]$ready.retry_count; ready_failed = [int]$ready.failed_count
            book_unit = "$($ready.book.unit)"; book_ok = ($ready.book.ok -eq $true); book_status = "$($ready.book.status)"
        } @{ ready = $ready; provider = $null }
    }
    'cleanup-report' {
        $r = Run @('ledger', 'report', '-Out', $Out)
        if (-not $r.json) { Fail 'report the run from the ledger' "the run report printed no line: $($r.tail)"; break }
        $f = $r.json
        $c = $f.completeness
        $n = $f.counts
        $u = $f.usage
        # The usage facts as one short string (#180): status, source, known runs, missing runs, input and output
        # tokens ("complete,per-run,1,0,100,20"). The whole report is the detail file.
        $usage = (@("$($u.status)", "$($u.source)", [int]$u.known_runs, [int]$u.missing_runs, [int]$u.totals.input_tokens, [int]$u.totals.output_tokens) -join ',')
        Reply @{
            ok = $true
            settled = ($c.settled -eq $true); content_complete = ($c.content_complete -eq $true); quality_complete = ($c.quality_complete -eq $true)
            cleanup_complete = ($c.cleanup_complete -eq $true); complete = ($c.complete -eq $true)
            declared = [int]$n.declared; done = [int]$n.done; failed = [int]$n.failed; open = [int]$n.open
            gaps = (Count-Of $f.gaps); placeholders = [int]$f.placeholders.count; resume_available = ($f.resume.available -eq $true)
            usage = $usage
        } $f
    }
}
Fail $Step 'the step named no action'









