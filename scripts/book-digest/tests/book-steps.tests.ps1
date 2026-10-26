#Requires -Version 7.5
<#
.SYNOPSIS
  The book-level digest stages' deterministic steps (book-steps.ps1, #176): one script call runs a whole chain
  and prints one compact JSON line.

.DESCRIPTION
  A stand-in zt-digest answers the tool's commands from a small table, so the suite proves the step's own
  behaviour without an archive, a model or a provider:
  - `prepare` runs the full preparation, the one retained resume (with its summary and the digest gate
    fallback) or a backfill branch, and stops on the owner's typed refusal or a backfill conflict;
  - `link-prepare` returns the Link pass's gap set;
  - `publish` records a stage publication;
  - the card-cycle steps plan, apply and check the whole-book cycle's jobs;
  - the whole-book claim and its checked completion, with the ledger read that answers a cut line;
  - Organize's survey and apply, the skeleton's plan and apply, Figures, the Bridge, the audit;
  - the finalization's plan, changes, scope, refresh and final steps and its Term-link round;
  - the settlement's plan, apply (with the cleanup-finish fallback) and the verified run report.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$here = $PSScriptRoot
$script = Join-Path (Split-Path $here -Parent) 'book-steps.ps1'
$failed = 0
$ok = 0
function Assert([bool]$condition, [string]$name) {
    if ($condition) { $script:ok++; Write-Host "ok: $name" } else { $script:failed++; Write-Host "FAIL: $name" }
}
# One whole command line of the stand-in's call log, from its start.
function Has([string]$Text, [string]$Pattern) {
    return [bool](@("$Text".Split("`n")) | Where-Object { $_ -match "^$Pattern" })
}
$scratch = Join-Path ([IO.Path]::GetTempPath()) "book-steps-test-$PID"
if (Test-Path $scratch) { Remove-Item -Recurse -Force $scratch }
$tool = Join-Path $scratch 'fake-zt-digest.ps1'
$out = Join-Path $scratch 'out'
New-Item -ItemType Directory -Force $out | Out-Null
$calls = Join-Path $scratch 'calls.txt'
$count = Join-Path $scratch 'claim-count.txt'
$applyCount = Join-Path $scratch 'apply-count.txt'
$planCount = Join-Path $scratch 'plan-count.txt'
$toolBody = @'
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$ErrorActionPreference = 'Stop'
$args2 = $Rest
$flat = $args2 -join ' '
Add-Content -LiteralPath $env:BS_CALLS -Value $flat -Encoding utf8
function Out($Value) { $Value | ConvertTo-Json -Compress -Depth 12; exit 0 }
if ($env:BS_LOCK -and $flat -match $env:BS_LOCK) {
    Out @{ ok = $false; code = 'card_locked'; error = 'card_locked'; message = 'the Literature tree Key is locked by the owner'; tree = 'Key'; lock_id = 3 }
}
if ($env:BS_NO_LINE -and $flat -match $env:BS_NO_LINE) { Write-Output 'the tool printed no JSON line'; exit 1 }
$cmd = "$($args2[0]) $($args2[1])"
switch -Regex ($cmd) {
    '^prepare ' {
        if ($flat -match '-Sections|-Validate') {
            if ($env:BS_BACKFILL_CONFLICT) { Out @{ ok = $false; code = 'backfill_conflict'; error = 'the Term list holds a conflicting child'; book = 'Key'; conflicts = @(@{ code = 'occupying_children'; message = 'the Term list Key/1|a already holds 1 conflicting Side child' }); complete = $false } }
            Out @{ ok = $true; kind = 'backfill_preparation'; step = $(if ($flat -match '-Sections') { 'sections' } else { 'validate' }); book = 'Key'; list = @{ rows = 2 }; source = @{ sections = 2; reused = $true }; conflicts = @(); complete = $true }
        }
        Out @{ ok = $true; groups = 2; citation_key = 'Key'; out = $env:BS_OUT }
    }
    '^prepare-link ' { Out @{ pending = @(1); no_gap = @(2); blocked = @(); gaps = @{ '1' = 2 }; owned_total = 6; gap_total = 2; skipped_total = 4; unowned = 0; digest_state = 'ledger' } }
    '^ledger resume' {
        if ($env:BS_RESUME_STATUS) { Out @{ status = $env:BS_RESUME_STATUS; invocation = 2; resume_count = 1; reset = @('x'); reevaluated = @(); reclaimed = @(); closure = @{ downstream = @() }; settings = @{ max_attempts = 3; dsh_run_cap = 1200 } } }
        Out @{ status = 'resumed'; invocation = 2; resume_count = 1; reset = @('digest/g002@1'); reevaluated = @('cards/g002@1'); reclaimed = @(); closure = @{ downstream = @('x') }; settings = @{ max_attempts = 3; dsh_run_cap = 1200 } }
    }
    '^ledger status' {
        if ($flat -match '-Held') {
            if ($env:BS_HELD_NONE) { Out @{ units = @() } }
            Out @{ units = @(@{ id = 'cards-book/book@1'; token = 4; input_hash = 'ih-cb'; state = 'claimed'; inputs = @{} }) }
        }
        if ($flat -match '-Units') {
            $state = if ($env:BS_UNIT_STATE) { $env:BS_UNIT_STATE } else { 'done' }
            Out @{ units = @(@{ id = 'cards-book/book@1'; token = 1; input_hash = 'ih-cb'; state = $state }) }
        }
        if ($env:BS_STATUS_NO_LAYERS) { Out @{ settings = @{ max_attempts = 3; dsh_run_cap = 1200 }; citation_key = 'Key'; variant = 'econ'; units = @() } }
        Out @{ settings = @{ max_attempts = 3; dsh_run_cap = 1200 }; citation_key = 'Key'; variant = 'econ'; run_options = @{ models = $true }; layers = @{ digest = @{ total = 3; done = 2 }; cards = @{ total = 1 } } }
    }
    '^ledger gate' { Out @{ complete = $false; counts = @{ total = 3; done = 2; failed = 0; open = 1 } } }
    '^ledger next' {
        $n = 0
        if (Test-Path -LiteralPath $env:BS_COUNT) { $n = [int](Get-Content -LiteralPath $env:BS_COUNT -Raw) }
        $n++
        Set-Content -LiteralPath $env:BS_COUNT -Value $n
        if ($env:BS_CLAIM_FAIL -and $n -le [int]$env:BS_CLAIM_FAIL) { Write-Output 'zt-digest ledger: refused while checking preparation'; exit 1 }
        if ($env:BS_CLAIM_STRING) { Out @{ schema_version = 1; units = '(1 unit claimed)'; complete = $false } }
        if ($env:BS_NO_UNITS) { Out @{ schema_version = 1; layer = 'cards-book'; complete = $false; counts = @{ total = 1; open = 1 }; units = @() } }
        if ($flat -match '-Layer termlink') { Out @{ schema_version = 1; layer = 'termlink'; complete = $false; counts = @{ total = 1; open = 1 }; units = @(@{ id = 'termlink/final@1'; token = 1; input_hash = 'ih-tl'; inputs = @{} }) } }
        Out @{ schema_version = 1; layer = 'cards-book'; complete = $false; counts = @{ total = 1; open = 1 }; units = @(@{ id = 'cards-book/book@1'; token = 1; input_hash = 'ih-cb'; inputs = @{} }) }
    }
    '^ledger publish' { Out @{ unit = 'cards-book/book@1'; revision = 1; state = 'ready' } }
    '^ledger done' { Out @{ state = 'done'; unit = 'termlink/final@1' } }
    '^ledger fail' { if ($flat -match 'card_locked') { Out @{ state = 'failed' } } else { Out @{ state = 'retry' } } }
    '^ledger report' { Out @{ kind = 'run_report'; completeness = @{ complete = $false }; counts = @{}; gaps = @(); usage = @{} } }
    '^finalize ' {
        if ($flat -match '-Step stage') { Out @{ layer = "$($Matches[0])"; unit = 'organize/book@1'; state = 'done'; disposition = 'published' } }
        if ($flat -match '-Step plan') { Out @{ step = 'plan'; branch = 'fix'; audit = @{ unit = 'audit/book@1'; graph_revision = 7 }; faults = @{ total = 1 }; counts = @{ fix = 1 }; fix_cycle = @{ rules = @('R3'); cards = @('Key/1|a'); card_list = 'D:\x\fix-cards.txt' } } }
        if ($flat -match '-Step changes') { Out @{ step = 'changes'; finalized = $false; term_link = @{ applicable = $true; round = 1; scope = 'cards'; cards = @('Key/1|a') }; changes = 'D:\x\fix\state.json'; cycle = @{ report = 'D:\x\fix\report.json' }; refresh = @() } }
        if ($flat -match '-Step scope') { Out @{ step = 'scope'; term_link = @{ applicable = $true; round = 1; scope = 'cards'; cards = @('Key/1|a') }; cycle = @{ report = 'D:\x\fix\report.json' }; refresh = @(@{ layer = 'link-check'; applicable = $true }) } }
        if ($flat -match '-Step refresh') { Out @{ layer = 'link-check'; unit = 'link-check/book@2'; state = 'done'; changed_cards = 1 } }
        if ($flat -match '-Step final') { Out @{ step = 'final'; status = 'done'; graph_revision = 8; audit = @{ final = @{ unit = 'audit/book@2'; revision = 2 } }; faults = @{ residual = 0 }; infrastructure = @(); terms = @{ status = 'complete' } } }
        Out @{ ok = $true }
    }
    '^card ' {
        if ($flat -match '-Step plan') {
            $dir = Join-Path $env:BS_OUT 'cards'
            Out @{ book = 'Key'; picked = 4; with_faults = 1; review = 0; models = 1; orphans = 0; faults = 2; dir = $dir; lead = @{ fix = (Join-Path $dir 'card.md'); orphans = (Join-Path $dir 'orphan.md') }; prompts = @{ fix = 'pf'; orphans = 'po'; follow = 'pw' }; jobs = @(@{ id = 'fix-01'; kind = 'fix'; file = 'fix-01.md' }, @{ id = 'follow-02'; kind = 'follow'; file = 'follow-02.md'; continue = 'fix-01' }) }
        }
        if ($flat -match '-Step apply') {
            $n = 0
            if (Test-Path -LiteralPath $env:BS_APPLY_COUNT) { $n = [int](Get-Content -LiteralPath $env:BS_APPLY_COUNT -Raw) }
            $n++
            Set-Content -LiteralPath $env:BS_APPLY_COUNT -Value $n
            $pending = 0
            if ($env:BS_APPLY_PENDING -and $n -le [int]$env:BS_APPLY_PENDING) { $pending = 1 }
            Out @{ applied_jobs = 1; pending_jobs = $pending; retried_jobs = 0; waiting_jobs = @(); counts = @{ fix = 1 } }
        }
        if ($flat -match '-Step check') {
            $dir = Join-Path $env:BS_OUT 'cards'
            if ($env:BS_CHECK_NO_JOBS) { Out @{ faults = 0; orphans = 0; dir = $dir; lead = @{ fix = (Join-Path $dir 'card.md'); orphans = (Join-Path $dir 'orphan.md') }; prompts = @{ fix = 'pf'; orphans = 'po'; follow = 'pw' }; jobs = @() } }
            Out @{ faults = 1; orphans = 0; dir = $dir; lead = @{ fix = (Join-Path $dir 'card.md'); orphans = (Join-Path $dir 'orphan.md') }; prompts = @{ fix = 'pf'; orphans = 'po'; follow = 'pw' }; jobs = @(@{ id = 'fix-03'; kind = 'fix'; file = 'fix-03.md' }) }
        }
        if ($flat -match '-Step report') { Out @{ cards = 4; faults_before = 2; faults_after = 0; verdicts = @{ fix = 1 }; splits = 0; merges = 0; refused = 0; orphan_links = 0; orphans_after = 0 } }
        Out @{ ok = $true }
    }
    '^book-cards-finish ' {
        if ($env:BS_FINISH_NO_LINE) { Write-Output 'the tool printed no JSON line'; exit 1 }
        if ($env:BS_FINISH_FAIL) { Out @{ state = 'failed'; unit = 'cards-book/book@1'; error = 'the cycle report does not cover the book' } }
        Out @{ state = 'done'; unit = 'cards-book/book@1'; complete = $true; disposition = 'published' }
    }
    '^organize-survey ' { Out @{ survey = 'x'; topics = 1; pairs = 0 } }
    '^organize-apply ' { Out @{ merges = 0; moves = 0; refused = 0; notes = 2; thesis = $true } }
    '^skeleton ' {
        if ($flat -match '-Step plan') {
            Out @{ groups = 2; jobs = @(@{ id = 'skeleton'; files = @('a', 'b'); prompt = 'p' }) }
        }
        Out @{ ok = $true; thesis = '论点'; argument = 2; written = @{ structure = 'Key/1|b' } }
    }
    '^figures ' { Out @{ shown = 3; cards = 2; figures = 4; labelled = 1; literature = $true } }
    '^bridge ' {
        if ($flat -match '-Step plan') {
            Out @{ ok = $true; step = 'plan'; status = 'done'; plan_id = 'p-1'; report_id = 'r-1'; counts = @{ selected = 1; new = 1; earlier = 0 }; jobs = @(@{ id = 'br-001'; files = @('a'); prompt = 'p' }) }
        }
        if ($flat -match '-Step report') { Out @{ ok = $true; step = 'report'; status = 'done'; report_id = 'r-1'; counts = @{ selected = 1; new = 1; earlier = 0 } } }
        Out @{ ok = $true; step = 'apply'; status = 'done'; report_id = 'r-1'; applied = 1; publication = @{ state = 'done' } }
    }
    '^audit ' { Out @{ hubs_missing = 0; broken_links = 0; books = @(@{ key = 'Key'; cards = 9; style_faults = 0; orphans = 0; models = 0; not_self_sufficient = 0; figures_missing = 0 }) } }
    '^term-link ' {
        if ($flat -match '-Step plan') {
            $n = 0
            if (Test-Path -LiteralPath $env:BS_PLAN_COUNT) { $n = [int](Get-Content -LiteralPath $env:BS_PLAN_COUNT -Raw) }
            $n++
            Set-Content -LiteralPath $env:BS_PLAN_COUNT -Value $n
            $changed = $(if ($n -eq 1) { 1 } else { 0 })
            Out @{ ok = $true; step = 'plan'; unit = 'termlink/final@1'; plan_revision = 2; plan_hash = 'h2'; cards_changed = $changed; cards_refused = 0; missing_heads = 0; complete = $true }
        }
        Out @{ ok = $true; step = 'apply'; complete = $true; report = (Join-Path $env:BS_OUT 'term-link\apply.json'); counts = @{ cards_changed = 1; links_added = 2 }; no_op = $false; chunks = 1 }
    }
    '^cleanup-run ' {
        if ($flat -match '-Step plan') { Out @{ kind = 'cleanup_run'; step = 'plan'; deletes = 3; workspaces = 1; plan_file = (Join-Path $env:BS_OUT 'cleanup\settle-plan.json') } }
        if ($env:BS_CLEANUP_FAIL) { Write-Output 'the settle results do not answer the plan'; exit 1 }
        Out @{ kind = 'cleanup_run'; step = 'apply'; deleted = 3; absent = 0; failed_count = 0; workspaces_removed = 1; workspaces_kept = 0; ready = @{ kind = 'cleanup_ready'; claimed = 1; done = 1; partial = 0; retry_count = 0; failed_count = 0; book = @{ unit = 'cleanup/book@1'; ok = $true; status = 'complete'; cleanup_complete = $true; content_complete = $true } } }
    }
    '^cleanup-finish ' { Out @{ kind = 'cleanup_ready'; claimed = 1; done = 1; partial = 0; retry_count = 0; failed_count = 0; book = @{ unit = 'cleanup/book@1'; ok = $true; status = 'complete'; cleanup_complete = $true; content_complete = $true } } }
}
Out @{ ok = $true }
'@
Set-Content -LiteralPath $tool -Value $toolBody -Encoding utf8
$env:BS_CALLS = $calls
$env:BS_COUNT = $count
$env:BS_APPLY_COUNT = $applyCount
$env:BS_PLAN_COUNT = $planCount
$env:BS_OUT = $out

function Step([string]$name, [hashtable]$opts = @{}) {
    Remove-Item -LiteralPath $count -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $applyCount -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $planCount -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
    $json = if ($opts.Count) { $opts | ConvertTo-Json -Compress -Depth 8 } else { '{}' }
    $text = & pwsh -NoProfile -File $script -Step $name -Out $out -Json $json -Tool $tool 2>&1
    $code = $LASTEXITCODE
    $line = @($text | Where-Object { "$_".Trim().StartsWith('{') }) | Select-Object -Last 1
    $parsed = if ($line) { $line | ConvertFrom-Json -Depth 16 } else { $null }
    [pscustomobject]@{ code = $code; json = $parsed; text = (@($text) -join ' | '); calls = $(if (Test-Path -LiteralPath $calls) { Get-Content -LiteralPath $calls -Raw } else { '' }) }
}

# 1. prepare, mode full: one command prepares the book and its brief line is the step's answer.
$full = Step 'prepare' @{ Mode = 'full'; Source = 'S.md'; BibTeX = '@book{Key}'; BookTitle = '书'; CitationKey = 'Key'; MaxChars = 20000; MaxUnits = 3; Variant = 'econ'; MaxAttempts = 3; DshRunCap = 1200; RunOptions = '{"models":true}' }
Assert ($full.json -and $full.json.ok -eq $true -and $full.json.groups -eq 2 -and $full.json.citation_key -eq 'Key') "prepare full: the brief line carries the group count ($($full.text))"
Assert ((Has $full.calls 'prepare -Source S.md -BibTeX @book') -and $full.calls -match '-CitationKey Key -MaxChars 20000 -MaxUnits 3 -Variant econ -MaxAttempts 3 -DshRunCap 1200 -RunOptions' -and $full.calls -match '-Apply -Brief') 'prepare full: the step runs the one preparation command with the frozen settings'
$book = Step 'prepare' @{ Mode = 'full'; Book = '1234'; CitationKey = 'Key'; Variant = 'math' }
Assert ((Has $book.calls 'prepare -Book 1234 -Out ') -and $book.calls -notmatch ' -Source ') 'prepare full: a Scholar Archive Work replaces the explicit source'

# 2. prepare, mode resume: the one ledger resume and its summarized status give the retained groups.
$resume = Step 'prepare' @{ Mode = 'resume' }
Assert ($resume.json -and $resume.json.ok -eq $true -and $resume.json.groups -eq 3 -and $resume.json.resumed -eq $true -and $resume.json.citation_key -eq 'Key') "prepare resume: the retained group count and identity reach the stage ($($resume.text))"
Assert ((Has $resume.calls 'ledger resume -Out ') -and (Has $resume.calls 'ledger status -Out .* -Summary') -and -not (Has $resume.calls 'ledger gate ')) 'prepare resume: one resume and one summarized status, no gate when the summary carries the layers'
$env:BS_STATUS_NO_LAYERS = '1'
$fallback = Step 'prepare' @{ Mode = 'resume' }
$env:BS_STATUS_NO_LAYERS = ''
Assert ($fallback.json -and $fallback.json.groups -eq 3 -and (Has $fallback.calls 'ledger gate -Out .* -Layer digest -Brief')) "prepare resume: a summary without layer counts reads the digest gate ($($fallback.text))"
$env:BS_RESUME_STATUS = 'nothing_to_resume'
$nothing = Step 'prepare' @{ Mode = 'resume' }
$env:BS_RESUME_STATUS = ''
Assert ($nothing.json -and $nothing.json.ok -eq $true -and $nothing.json.resumed -eq $false -and $nothing.json.groups -eq 3) 'prepare resume: a run with nothing to resume still reports its retained groups'
$env:BS_NO_LINE = '^ledger resume'
$lost = Step 'prepare' @{ Mode = 'resume' }
$env:BS_NO_LINE = ''
Assert ($lost.json -and $lost.json.ok -eq $false -and $lost.json.step -eq 'resume' -and $lost.json.error) "prepare resume: a resume that answers no line fails with its reason ($($lost.text))"

# 3. prepare, the Term backfill branches: validate or re-stage only, and a conflict stops before any work.
$backfill = Step 'prepare' @{ Mode = 'term-cards' }
Assert ($backfill.json -and $backfill.json.ok -eq $true -and $backfill.json.book -eq 'Key' -and $backfill.json.list.rows -eq 2) "prepare backfill: the validation it bound is the step's answer ($($backfill.text))"
Assert (Has $backfill.calls 'prepare -Out .* -Sections -Brief') 'prepare backfill: term-cards re-stages the source sections'
$env:BS_BACKFILL_CONFLICT = '1'
$conflict = Step 'prepare' @{ Mode = 'term-link' }
$env:BS_BACKFILL_CONFLICT = ''
Assert ($conflict.json -and $conflict.json.ok -eq $false -and $conflict.json.step -eq 'prepare term-link' -and $conflict.json.error -match 'conflicting Side child') "prepare backfill: a conflict fails the step with its message ($($conflict.text))"
Assert (Has $conflict.calls 'prepare -Out .* -Validate -Brief') 'prepare backfill: term-link validates only'

# 4. prepare, the owner's Lock: the step keeps the archive's typed refusal.
$env:BS_LOCK = '^prepare '
$locked = Step 'prepare' @{ Mode = 'full'; Source = 'S.md'; BibTeX = '@book{Key}'; BookTitle = '书'; CitationKey = 'Key' }
$env:BS_LOCK = ''
Assert ($locked.json -and $locked.json.ok -eq $false -and $locked.json.code -eq 'card_locked' -and $locked.json.lock_id -eq 3 -and $locked.json.message -match 'locked by the owner') "prepare lock: the refusal, its sentence and its Lock id are the step's answer ($($locked.text))"
Assert ($locked.code -eq 0) 'prepare lock: a typed refusal is a JSON answer, not a crashed step'

# 5. link-prepare: one brief command returns the gap set.
$linkPrep = Step 'link-prepare'
Assert ($linkPrep.json -and $linkPrep.json.ok -eq $true -and @($linkPrep.json.pending).Count -eq 1 -and $linkPrep.json.gap_total -eq 2) "link-prepare: the pending groups and the gap total are the step's answer ($($linkPrep.text))"
Assert (Has $linkPrep.calls 'prepare-link -Staging .* -Brief') 'link-prepare: the step runs the one brief preparation command'

# 6. publish: one stage command records its own book revision and names its declared artifacts.
$publish = Step 'publish' @{ Layer = 'organize'; Decisions = 'D:\x\organize\decisions.json'; Applied = 'D:\x\organize\applied.json' }
Assert ($publish.json -and $publish.json.ok -eq $true -and $publish.json.state -eq 'done' -and $publish.json.unit -eq 'organize/book@1') "publish: the publication line is the step's answer ($($publish.text))"
Assert ((Has $publish.calls 'finalize -Step stage -Out .* -Layer organize -Decisions D:\\x\\organize\\decisions.json -Applied D:\\x\\organize\\applied.json')) 'publish: the stage, its layer and its declared artifacts are one command'
$ledgerPublish = Step 'publish' @{ Layer = 'cards-book'; Ledger = $true }
Assert ($ledgerPublish.json -and $ledgerPublish.json.ok -eq $true -and $ledgerPublish.json.unit -eq 'cards-book/book@1' -and (Has $ledgerPublish.calls 'ledger publish -Out .* -Layer cards-book')) "publish: the whole-book Card revision is the ledger's own publication ($($ledgerPublish.text))"

# 7. the card cycle: plan lists its jobs, apply drains the drafts and checks, finish reports.
$work = Join-Path $out 'cards'
$cardsStart = Step 'cards-start' @{ Work = $work; Book = 'Key'; Staging = $out; MaxCards = 5; Sample = 0.1; ModelReview = $true }
Assert ($cardsStart.json -and $cardsStart.json.ok -eq $true -and $cardsStart.json.picked -eq 4 -and $cardsStart.json.jobs -eq 2) "cards start: the plan's counts and job count are the step's answer ($($cardsStart.text))"
Assert ((Has $cardsStart.calls 'card -Step plan -Work .* -Compact -Book Key -Staging .* -MaxCards 5 -Sample 0.1 -ModelReview')) 'cards start: the cycle plan names the book, the cap and the sample'
$jobs = @(Get-Content -Raw (Join-Path $work 'out\jobs.json') | ConvertFrom-Json)
Assert ($jobs.Count -eq 2 -and $jobs[0].id -eq 'fix-01' -and $jobs[0].prompt -eq 'pf' -and $jobs[0].files.Count -eq 2) 'cards start: a fix job expands to its instruction file and its own file'
Assert ($jobs[1].id -eq 'follow-02' -and $jobs[1].continue -eq 'fix-01' -and $jobs[1].prompt -eq 'pw' -and $jobs[1].fallback.Count -eq 2) 'cards start: a follow job keeps its continue and its fallback'
$env:BS_APPLY_PENDING = '2'
$cardsApply = Step 'cards-apply' @{ Work = $work; MaxJobs = 30 }
$env:BS_APPLY_PENDING = ''
Assert ($cardsApply.json -and $cardsApply.json.ok -eq $true -and $cardsApply.json.applied -eq 3 -and $cardsApply.json.faults -eq 1 -and $cardsApply.json.jobs -eq 1) "cards apply: every pending share is applied, then the check lists its fix job ($($cardsApply.text))"
Assert (@(Get-Content -Raw (Join-Path $work 'out\jobs.json') | ConvertFrom-Json).Count -eq 1) 'cards apply: the check writes its own job file for the next content round'
$cardsFinish = Step 'cards-finish' @{ Work = $work; MaxJobs = 30 }
Assert ($cardsFinish.json -and $cardsFinish.json.ok -eq $true -and $cardsFinish.json.cards -eq 4 -and $cardsFinish.json.faults_after -eq 0) "cards finish: the cycle's report is the step's answer ($($cardsFinish.text))"
Assert ((Has $cardsFinish.calls 'card -Step report -Work ') -and (Has $cardsFinish.calls 'card -Step apply -Work .* -MaxJobs 30')) 'cards finish: the last drafts are applied before the report'

# 8. unit-fail records one claimed unit's fault, with the claim's own hash.
$fail = Step 'unit-fail' @{ Unit = 'cards-book/book@1'; Token = 3; InputHash = 'ih-cb'; Cause = 'the whole-book cycle did not complete'; Code = 'missing_result' }
Assert ($fail.json -and $fail.json.ok -eq $true -and $fail.json.state -eq 'retry') "unit-fail: the ledger's answer is the step's result ($($fail.text))"
Assert ((Has $fail.calls 'ledger fail -Out .* -Unit cards-book/book@1 -Token 3 -Fault') -and $fail.calls -match 'ih-cb' -and $fail.calls -match 'missing_result') 'unit-fail: the fault names the unit, its token, its hash and its code'

# 9. the whole-book revision: publish, claim, then the checked completion.
$bookStart = Step 'cards-book-start' @{ Wait = 300; Lease = 300 }
Assert ($bookStart.json -and $bookStart.json.ok -eq $true -and $bookStart.json.unit -eq 'cards-book/book@1' -and $bookStart.json.token -eq 1 -and $bookStart.json.input_hash -eq 'ih-cb') "cards-book start: the claimed revision is the step's answer ($($bookStart.text))"
Assert ((Has $bookStart.calls 'ledger publish -Out .* -Layer cards-book') -and (Has $bookStart.calls 'ledger next -Out .* -Layer cards-book -Take 1 -Owner cards-book claim -Wait 300 -Lease 300 -Brief')) 'cards-book start: one publish and one brief claim'
$env:BS_CLAIM_STRING = '1'
$lostClaim = Step 'cards-book-start' @{ Wait = 300; Lease = 300 }
$env:BS_CLAIM_STRING = ''
Assert ($lostClaim.json -and $lostClaim.json.unit -eq 'cards-book/book@1' -and (Has $lostClaim.calls 'ledger status -Out .* -Held cards-book claim')) "cards-book start: a cut claim line reads the units its owner holds ($($lostClaim.text))"
$env:BS_HELD_NONE = '1'
$env:BS_NO_UNITS = '1'
$emptyBook = Step 'cards-book-start' @{ Wait = 300; Lease = 300 }
$env:BS_HELD_NONE = ''
$env:BS_NO_UNITS = ''
Assert ($emptyBook.json -and $emptyBook.json.ok -eq $true -and $emptyBook.json.unit -eq $null -and $emptyBook.json.waiting -eq $true -and $emptyBook.json.gate) "cards-book start: a revision waiting for its Model reviews is an empty success ($($emptyBook.text))"
$finish = Step 'cards-book-finish' @{ Unit = 'cards-book/book@1'; Token = 1; InputHash = 'ih-cb'; Work = $work }
Assert ($finish.json -and $finish.json.ok -eq $true -and $finish.json.state -eq 'done') "cards-book finish: the checked completion is the step's answer ($($finish.text))"
$env:BS_FINISH_NO_LINE = '1'
$cutFinish = Step 'cards-book-finish' @{ Unit = 'cards-book/book@1'; Token = 1; InputHash = 'ih-cb'; Work = $work }
$env:BS_FINISH_NO_LINE = ''
Assert ($cutFinish.json -and $cutFinish.json.ok -eq $true -and $cutFinish.json.state -eq 'done' -and $cutFinish.json.from -eq 'ledger' -and (Has $cutFinish.calls 'ledger status -Out .* -Units cards-book/book@1')) "cards-book finish: a cut line asks the ledger, and a done unit stays done ($($cutFinish.text))"
$env:BS_FINISH_FAIL = '1'
$env:BS_UNIT_STATE = 'failed'
$badFinish = Step 'cards-book-finish' @{ Unit = 'cards-book/book@1'; Token = 2; InputHash = 'ih-cb'; Work = $work }
$env:BS_FINISH_FAIL = ''
$env:BS_UNIT_STATE = ''
Assert ($badFinish.json -and $badFinish.json.ok -eq $false -and $badFinish.json.state -eq 'retry' -and (Has $badFinish.calls 'ledger fail -Out .* -Unit cards-book/book@1 -Token 2 -Fault') -and $badFinish.calls -match 'ih-cb') "cards-book finish: a refused completion keeps its fault on its own unit ($($badFinish.text))"

# 10. Organize: the survey and the applied decisions.
$org = Join-Path $out 'organize'
$survey = Step 'organize-survey' @{ Work = $org }
Assert ($survey.json -and $survey.json.ok -eq $true -and $survey.json.pairs -eq 0 -and (Has $survey.calls 'organize-survey -Staging .* -Work .*organize')) "organize survey: its line is the step's answer ($($survey.text))"
$orgApply = Step 'organize-apply' @{ Work = $org }
Assert ($orgApply.json -and $orgApply.json.ok -eq $true -and $orgApply.json.notes -eq 2 -and (Has $orgApply.calls 'organize-apply -Work .*organize')) "organize apply: the count of notes is the step's answer ($($orgApply.text))"

# 11. the skeleton: its plan lists the one index job, its apply writes the Cards.
$sk = Join-Path $out 'skeleton'
$skStart = Step 'skeleton-start' @{ Work = $sk }
Assert ($skStart.json -and $skStart.json.ok -eq $true -and $skStart.json.groups -eq 2 -and $skStart.json.jobs -eq 1) "skeleton start: the plan's job count is the step's answer ($($skStart.text))"
$skJobs = @(Get-Content -Raw (Join-Path $sk 'out\jobs.json') | ConvertFrom-Json)
Assert ($skJobs.Count -eq 1 -and $skJobs[0].id -eq 'skeleton' -and $skJobs[0].files.Count -eq 2) 'skeleton start: its own job list reaches the content manager unchanged'
$skFinish = Step 'skeleton-finish' @{ Work = $sk; Decisions = (Join-Path $org 'decisions.json') }
Assert ($skFinish.json -and $skFinish.json.ok -eq $true -and $skFinish.json.thesis -eq '论点' -and (Has $skFinish.calls 'skeleton -Step apply -Staging .* -Work .*skeleton -Decisions .*decisions.json -Apply')) "skeleton finish: the applied skeleton is the step's answer ($($skFinish.text))"

# 12. Figures: one pass shows the book's figures and writes its report.
$fig = Step 'figures' @{ CitationKey = 'Key'; FileId = 77 }
Assert ($fig.json -and $fig.json.ok -eq $true -and $fig.json.shown -eq 3 -and $fig.json.literature -eq $true) "figures: the pass's summary is the step's answer ($($fig.text))"
Assert (Has $fig.calls 'figures -CitationKey Key -Staging .* -FileId 77 -Report .*figures-report.json -Apply') 'figures: one apply pass with its report and the chosen document'

# 13. the Bridge: plan, report and apply are one command each.
$br = Join-Path $out 'bridge'
$brPlan = Step 'bridge-start' @{ Work = $br; CitationKey = 'Key'; Profile = 'book'; MaxSources = 40; Candidates = 5; Books = 'Other' }
Assert ($brPlan.json -and $brPlan.json.ok -eq $true -and $brPlan.json.report_id -eq 'r-1' -and $brPlan.json.jobs -eq 1) "bridge start: the plan's report id and job count are the step's answer ($($brPlan.text))"
Assert (Has $brPlan.calls 'bridge -Step plan -Staging .* -CitationKey Key -Work .*bridge -Profile book -MaxSources 40 -Candidates 5 -Books Other') 'bridge start: the discovery limits and the book filter reach the plan'
$brReport = Step 'bridge-report' @{ Work = $br; CitationKey = 'Key' }
Assert ($brReport.json -and $brReport.json.ok -eq $true -and $brReport.json.report_id -eq 'r-1' -and (Has $brReport.calls 'bridge -Step report -Staging .* -CitationKey Key -Work .*bridge')) "bridge report: the frozen report is the step's answer ($($brReport.text))"
$brApply = Step 'bridge-apply' @{ Work = $br; CitationKey = 'Key'; ReportId = 'r-1'; Accept = 'all' }
Assert ($brApply.json -and $brApply.json.ok -eq $true -and $brApply.json.step -eq 'apply' -and (Has $brApply.calls 'bridge -Step apply -Staging .* -CitationKey Key -Work .*bridge -ReportId r-1 -Accept all -Apply')) "bridge apply: the applied report is the step's answer ($($brApply.text))"

# 14. the audit: one read-only pass on the book or on the whole Workspace.
$audit = Step 'audit' @{ CitationKey = 'Key' }
Assert ($audit.json -and $audit.json.ok -eq $true -and @($audit.json.books).Count -eq 1 -and (Has $audit.calls 'audit -CitationKey Key -Out ')) "audit: the read-only summary is the step's answer ($($audit.text))"
$wsAudit = Step 'audit' @{}
Assert ($wsAudit.json -and $wsAudit.json.ok -eq $true -and (Has $wsAudit.calls 'audit -Out ') -and $wsAudit.calls -notmatch '-CitationKey') 'audit: a Workspace audit names no Citation key'

# 15. the finite finalization: each step is the tool's own command and line.
$fPlan = Step 'finalize-plan'
Assert ($fPlan.json -and $fPlan.json.ok -eq $true -and $fPlan.json.branch -eq 'fix' -and $fPlan.json.fix_cycle.rules[0] -eq 'R3' -and (Has $fPlan.calls 'finalize -Step plan -Out ')) "finalize plan: the audit plan is the step's answer ($($fPlan.text))"
$fChanges = Step 'finalize-changes'
Assert ($fChanges.json -and $fChanges.json.ok -eq $true -and $fChanges.json.step -eq 'changes' -and $fChanges.json.term_link.round -eq 1) "finalize changes: the retained manifest is the step's answer ($($fChanges.text))"
$fScope = Step 'finalize-scope' @{ FixWork = 'D:\x\fix' }
Assert ($fScope.json -and $fScope.json.ok -eq $true -and $fScope.json.step -eq 'scope' -and (Has $fScope.calls 'finalize -Step scope -Out .* -FixWork D:\\x\\fix')) "finalize scope: the changed-Card manifest is the step's answer ($($fScope.text))"
$fRefresh = Step 'finalize-refresh' @{ Layer = 'link-check'; Report = 'D:\x\fix\report.json' }
Assert ($fRefresh.json -and $fRefresh.json.ok -eq $true -and $fRefresh.json.layer -eq 'link-check' -and (Has $fRefresh.calls 'finalize -Step refresh -Out .* -Layer link-check -Report D:\\x\\fix\\report.json')) "finalize refresh: the refreshed publication is the step's answer ($($fRefresh.text))"
$fFinal = Step 'finalize-final'
Assert ($fFinal.json -and $fFinal.json.ok -eq $true -and $fFinal.json.step -eq 'final' -and $fFinal.json.graph_revision -eq 8 -and (Has $fFinal.calls 'finalize -Step final -Out ')) "finalize final: the final audit is the step's answer ($($fFinal.text))"
$env:BS_LOCK = '^finalize '
$fLocked = Step 'finalize-final'
$env:BS_LOCK = ''
Assert ($fLocked.json -and $fLocked.json.ok -eq $false -and $fLocked.json.code -eq 'card_locked') 'finalize: a Lock on the final audit keeps the typed refusal'

# 16. the finalization's Term-link round: plan, apply under the claim, complete, plan again.
$tl = Step 'term-link' @{ Round = 1; Occurrences = 'first'; Cards = @('Key/1|a') }
Assert ($tl.json -and $tl.json.ok -eq $true -and $tl.json.status -eq 'done' -and $tl.json.unit -eq 'termlink/final@1' -and $tl.json.counts.cards_changed -eq 1 -and $tl.json.second.cards_changed -eq 0) "term-link: the completed round and the second plan are the step's answer ($($tl.text))"
Assert ((Has $tl.calls 'term-link -Step plan -Staging .* -Occurrences first -Round 1 -Cards Key/1\|a') -and (Has $tl.calls 'ledger next -Out .* -Layer termlink -Take 1 -Owner term-link -Wait 60 -Lease 3600') -and (Has $tl.calls 'term-link -Step apply -Staging .* -Occurrences first -Round 1 -Cards Key/1\|a -Token 1 -Apply') -and (Has $tl.calls 'ledger done -Out .* -Unit termlink/final@1 -Token 1 -Outputs .*apply.json')) 'term-link: the plan, the claim, the apply and the completion are one step'
$env:BS_NO_UNITS = '1'
$tlSkipped = Step 'term-link' @{ Round = 1; Occurrences = 'first'; Cards = @('Key/1|a') }
$env:BS_NO_UNITS = ''
Assert ($tlSkipped.json -and $tlSkipped.json.ok -eq $true -and $tlSkipped.json.status -eq 'skipped' -and $tlSkipped.json.reason) "term-link: a pass with no ready unit is skipped with its reason ($($tlSkipped.text))"

# 17. the settlement: its plan, its apply (with the cleanup-finish fallback) and the run report.
$clStart = Step 'cleanup-start'
Assert ($clStart.json -and $clStart.json.ok -eq $true -and $clStart.json.kind -eq 'cleanup_run' -and $clStart.json.plan_file -match 'cleanup\\settle-plan.json' -and $clStart.json.results_file -match 'cleanup\\settle-results.json') "cleanup start: the plan and its two paths are the step's answer ($($clStart.text))"
Assert (Has $clStart.calls 'cleanup-run -Out .* -Step plan') 'cleanup start: one plan command records the evidence'
$clApply = Step 'cleanup-apply'
Assert ($clApply.json -and $clApply.json.ok -eq $true -and $clApply.json.ready.done -eq 1 -and -not $clApply.json.fallback) "cleanup apply: the settled provider facts are the step's answer ($($clApply.text))"
Assert ((Has $clApply.calls 'cleanup-run -Out .* -Step apply -Owner cleanup-run') -and -not (Has $clApply.calls 'cleanup-finish ')) 'cleanup apply: the provider results settle the units, with no fallback'
$env:BS_CLEANUP_FAIL = '1'
$clFallback = Step 'cleanup-apply'
$env:BS_CLEANUP_FAIL = ''
Assert ($clFallback.json -and $clFallback.json.ok -eq $true -and $clFallback.json.fallback -eq $true -and $clFallback.json.ready.claimed -eq 1 -and (Has $clFallback.calls 'cleanup-finish -Out .* -Ready -Owner cleanup-run')) "cleanup apply: without the provider results the finished units still settle ($($clFallback.text))"
$clReport = Step 'cleanup-report'
Assert ($clReport.json -and $clReport.json.ok -eq $true -and $clReport.json.kind -eq 'run_report' -and (Has $clReport.calls 'ledger report -Out ')) "cleanup report: the verified run report is the step's answer ($($clReport.text))"

Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
Write-Host ''
Write-Host "book-steps: $ok passed, $failed failed"
if ($failed) { exit 1 } else { exit 0 }


