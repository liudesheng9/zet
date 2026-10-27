#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\book-cards.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# R217/R247/R296-R298: one whole-book Card cycle with source-review coverage and explicit quality
# outcomes. Two prepared books on one scratch archive (the live archive is never touched):
#   * the models layer is excluded, so the cycle itself owns the source review of every Model
#     draft; its checked coverage completes the cards-book revision, and a residual hard fault
#     stays a named obligation instead of a waived one;
#   * the models layer is enabled, so a failed model chain becomes precise unfinished-chain
#     coverage instead of a second review of the same draft.
# The architect's checked source-review artifact is exercised at the model compiler boundary.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'; $utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "book-cards-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest; $env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
$env:ZT_DIGEST_SCRIPTS = Join-Path $PSScriptRoot '..' | Resolve-Path | ForEach-Object Path
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition, $message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) { throw "digest failed: $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Save-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 30), $utf8) }
function Save-Text($path, $text) { [IO.File]::WriteAllText($path, $text, $utf8) }
function Refused($arguments, $text, $message) {
    $lines = @(& $Digest @arguments 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($lines -join ' ').Contains($text)) "$message [$($lines -join ' ')]"
}
function State($work) { Get-Content -LiteralPath (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable }

# One prepared economics book whose single group writes its anchor, one substantive Idea Card and
# one Model draft. $models = $false excludes the model layer from the declared target, so no model
# chain owns the Model draft and the whole-book card cycle must review it itself.
function New-PreparedBook([string]$key, [bool]$models, [switch]$Faulty) {
    $out = Join-Path $scratch "prepared-$key"
    $source = Join-Path $scratch "book-$key.md"
    $options = Join-Path $scratch "options-$key.json"
    Save-Text $source "# 导论`n`n融资约束影响企业的生产选择。贷款收紧使企业减少设备投入。`n`n企业家用清算价值决定借款上限。`n`n# 参考`n未选择。"
    $flags = @{ terms = $false }
    if (-not $models) { $flags.models = $false }
    Save-Json $options $flags
    if ($Faulty) {
        # An open formula (R22) keeps its blocking level on a Literature note since #184, while
        # the Idea-card sentence rules (R2 to R5, R7) only advise there.
        $ideaBody = "融资成本 `$r 影响企业的生产选择，因为贷款收紧使企业减少设备投入。依赖 [[$key]]，后者给出全书的问题范围。来源：[[$key/1]]"
    } else {
        $ideaBody = "融资成本影响企业的生产选择。贷款收紧使企业减少设备投入。依赖 [[$key]]，后者给出全书的问题范围。来源：[[$key/1]]"
    }
    $modelBody = '问题：企业家能借到多少？概要：投资者只认清算价值。'
    $null = Run-Digest @('prepare', '-Source', $source, '-Out', $out, '-CitationKey', $key, '-BibTeX', "@book{$key,title={$key book}}", '-StartHeading', '导论', '-EndHeading', '参考', '-MinUnitChars', '1', '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')
    $claim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'digest-manager')).units[0]
    $started = Run-Digest @('group-start', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)")
    $job = Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $calls = @(
        @{ tool = 'edit_card'; arguments = @{ at = "$key/1"; title = '融资约束影响企业投资'; body = '融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。'; base_fingerprint = $job.inputs.anchor.fingerprint } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = '贷款收紧使企业减少设备投入'; body = $ideaBody } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; type = 'model'; level = 'concept'; title = '清算价值决定借款上限的模型'; body = $modelBody } }
    )
    $draft = @{ schema_version = 1; unit = $claim.id; token = $claim.token; input_hash = $started.input_hash; disposition = 'cards'; calls = $calls }
    Save-Json $started.draft $draft
    $group = Run-Digest @('group-finish', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)", '-Draft', $started.draft)
    $cardsClaim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'cards', '-Take', '1', '-Owner', 'cards-manager')).units[0]
    $null = Run-Digest @('cards-finish', '-Out', $out, '-Group', '1', '-Token', "$($cardsClaim.token)")
    return [pscustomobject]@{ out = $out; key = $key; group = $group }
}

# One whole-book Card cycle over the prepared staging: plan, the DeepSeek answers stand in as JSON
# files with verdict "ok", apply, report. Returns the cycle's work folder, plan, state and report.
function Invoke-CardCycle([string]$out, [string]$key, [string]$work, [int]$seed, [double]$sample, [switch]$ModelReview) {
    $planArgs = @('card', '-Step', 'plan', '-Work', $work, '-Book', $key, '-Staging', $out, '-Seed', "$seed", '-Sample', "$sample", '-NoOrphans')
    if ($ModelReview) { $planArgs += '-ModelReview' }
    $plan = Run-Digest $planArgs
    $state = State $work
    foreach ($job in @($state.jobs.Values)) {
        $answers = @(foreach ($at in @($job.cards)) { @{ address = $at; verdict = 'ok'; note = '对照原文核对无误。' } })
        Save-Json $job.out @{ cards = $answers }
    }
    $applied = Run-Digest @('card', '-Step', 'apply', '-Work', $work)
    $report = Run-Digest @('card', '-Step', 'report', '-Work', $work)
    return [pscustomobject]@{ work = $work; plan = $plan; state = $state; applied = $applied; report = $report }
}

try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null

    # ==== A. models excluded: the cycle owns every Model draft's source review ====
    $book = New-PreparedBook 'CycleBook' $false
    $out = $book.out; $key = $book.key
    Assert ($book.group.state -eq 'done' -and @($book.group.owned).Count -eq 3) "the group writes its anchor, one Idea Card and one Model draft [$($book.group.owned -join ', ')]"
    $models = Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'models')
    Assert (@($models.units).Count -eq 0) 'an excluded model layer registers no model unit for the Model draft'

    $cycle = Invoke-CardCycle $out $key (Join-Path $out 'cards') 7 0.5 -ModelReview
    $report = $cycle.report
    Assert ($report.kind -eq 'cards_cycle' -and $report.unit_kind -eq 'book' -and $report.seed -eq 7) "the whole-book cycle reports its own scope and seed [$($report.kind)/$($report.unit_kind)/$($report.seed)]"
    $modelDraft = @($book.group.owned | Where-Object { $_ -match '\|b$' })[0]
    Assert (@($report.reviews | Where-Object { $_.address -eq $modelDraft -and $_.kind -eq 'model' }).Count -eq 1) 'the cycle reviews the Model draft no model chain owns'
    $modelReview = @($report.reviews | Where-Object { $_.address -eq $modelDraft })[0]
    Assert ($modelReview.verdict -eq 'ok' -and @($modelReview.spans).Count -eq 1 -and $modelReview.fingerprint) 'the Model review records its verdict, source span and the reviewed Card fingerprint'
    Assert ($report.sample.proves -eq 'sample' -and $report.sample.seed -eq 7 -and $report.sample.population -ge 1 -and $report.sample.eligible -ge 1) 'the Idea sample records its population, eligible set, seed and that it is a sample'
    Assert ($report.sample.size -eq $report.sample.target -and @($report.sample.addresses).Count -eq $report.sample.size -and $report.sample.covered -eq $report.sample.groups) "the sample covers every group with an eligible Card and records the exact addresses [$($report.sample | ConvertTo-Json -Compress -Depth 5)]"
    Assert (@($report.sample.addresses | Where-Object { @($report.sample.spans[$_]).Count -eq 0 }).Count -eq 0) 'every sampled Card records its reviewed source span'
    Assert (@($report.sample.by_group | Where-Object { $_.eligible -gt 0 -and @($_.picked).Count -eq 0 }).Count -eq 0) 'no group with an eligible Card is left out of the sample'
    Assert ($report.faults_after -eq 0 -and $report.quality_complete) 'the cycle reports a clean deterministic pass on the whole book'

    $published = Run-Digest @('ledger', 'publish', '-Out', $out, '-Layer', 'cards-book')
    Assert ($published.revision -eq 2 -and @($published.contributions | Where-Object { $_.unit -eq 'cards/g001@1' }).Count -eq 1) 'the cards-book revision consumes the completed Cards/group unit'
    $unit = $published.unit
    $claim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'cards-book', '-Take', '1', '-Owner', 'cards-manager')).units[0]
    Assert ($claim.id -eq $unit -and $claim.token) 'the cards-book revision is claimable once its inputs settle'
    $finishArgs = @('book-cards-finish', '-Out', $out, '-Work', $cycle.work, '-Token', "$($claim.token)")
    $preview = Run-Digest ($finishArgs + @('-CheckOnly'))
    Assert ($preview.check_only -and -not $preview.applied -and $preview.disposition -eq 'published') 'the checked completion previews the coverage without writing'
    Assert (@($preview.coverage | Where-Object { $_.kind -eq 'model' -and $_.ref -eq $modelDraft -and $_.owner -eq 'cycle' -and $_.outcome -eq 'reviewed' }).Count -eq 1) 'every Model draft has one reviewed owner'
    Assert (@($preview.coverage | Where-Object { $_.kind -eq 'sample' -and $_.proves -eq 'sample' }).Count -eq 1) 'the completion records the Idea sample as a sample'
    Assert (-not (Test-Path (Join-Path $out 'publication-cards-book-book-2.json'))) 'the preview publishes no completion evidence'
    $done = Run-Digest $finishArgs
    Assert ($done.state -eq 'done' -and $done.complete -and $done.layer -eq 'cards-book' -and $done.revision -eq 2) 'the whole-book Card cycle completes its ledger revision'
    $residual = @($done.coverage | Where-Object { $_.kind -eq 'fault' })
    Assert ($residual.Count -eq 0) "a clean cycle leaves no residual quality obligation [$($residual | ForEach-Object { $_.ref })]"
    Assert ((Test-Path (Join-Path $out 'publication-cards-book-book-2.json')) -and $done.artifacts.report.sha256) 'the completion retains its report and evidence'
    $again = Run-Digest $finishArgs
    Assert ($again.reused -and $again.state -eq 'done') 'a repeated completion reuses the checked revision'
    Refused @('book-cards-finish', '-Out', $out, '-Work', $cycle.work, '-Token', "$($claim.token + 1)") 'stale_claim' 'a wrong claim token cannot complete the cards-book revision'
    Refused @('book-cards-finish', '-Out', $out, '-Work', (Join-Path $out 'nowhere'), '-Token', "$($claim.token)") 'left no report' 'a missing cycle report is not a checked cycle'
    $faultsPath = Join-Path $cycle.work 'after\audit-faults.txt'
    $restore = Get-Content -LiteralPath $faultsPath -Raw -Encoding utf8
    Save-Text $faultsPath ("$restore`n$key/1 R2 invented fault line")
    Refused @('book-cards-finish', '-Out', $out, '-Work', $cycle.work, '-Token', "$($claim.token)") 'does not describe the checked Cards' 'a hand-written fault list cannot replace the deterministic pass'
    Save-Text $faultsPath $restore
    $ideaCard = @($report.sample.addresses)[0]
    $before = (Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $ideaCard } }))
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $ideaCard; body = "$($before.body)`n`n后来的一条独立补充。" } }) | Out-Null
    Refused @('book-cards-finish', '-Out', $out, '-Work', $cycle.work, '-Token', "$($claim.token)") 'changed after the cycle' 'a Card edited after the cycle makes the checked coverage stale'
    Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $ideaCard; body = $before.body } }) | Out-Null
    Assert (@((Run-Digest ($finishArgs + @('-CheckOnly'))).coverage | Where-Object { $_.kind -eq 'sample' }).Count -eq 1) 'restoring the reviewed Card recovers the same coverage'

    # A cycle that reviewed no Model draft cannot claim the book revision.
    $blindWork = Join-Path $out 'cards-blind'
    $null = Invoke-CardCycle $out $key $blindWork 7 0.5
    Refused @('book-cards-finish', '-Out', $out, '-Work', $blindWork, '-Token', "$($claim.token)") 'reviewed no source for the Model draft' 'a cycle that reviewed no Model draft has no coverage to claim'

    # A residual hard fault is an explicit obligation, never a silent pass.
    $faultBook = New-PreparedBook 'CycleFault' $false -Faulty
    $faultOut = $faultBook.out; $faultKey = $faultBook.key
    $faultCycle = Invoke-CardCycle $faultOut $faultKey (Join-Path $faultOut 'cards') 3 0.5 -ModelReview
    Assert ($faultCycle.report.faults_before -ge 1 -and $faultCycle.report.faults_after -ge 1) "the cycle saw the group's real fault and could not invent a clean pass [$($faultCycle.report.faults_before) -> $($faultCycle.report.faults_after)]"
    $null = Run-Digest @('ledger', 'publish', '-Out', $faultOut, '-Layer', 'cards-book')
    $faultClaim = (Run-Digest @('ledger', 'next', '-Out', $faultOut, '-Layer', 'cards-book', '-Take', '1', '-Owner', 'cards-manager')).units[0]
    $faultDone = Run-Digest @('book-cards-finish', '-Out', $faultOut, '-Work', $faultCycle.work, '-Token', "$($faultClaim.token)")
    $obligations = @($faultDone.coverage | Where-Object { $_.kind -eq 'fault' })
    Assert ($faultDone.state -eq 'done' -and $obligations.Count -ge 1) 'a residual hard fault completes as a named obligation, not a waived one'
    Assert (@($obligations | Where-Object { $_.outcome -eq 'unresolved' -and $_.obligation -eq 'final-fix' -and $_.rule -match '^[RIEL]\d+$' }).Count -eq $obligations.Count) 'each residual fault names its Card, rule, unresolved outcome and the final-fix obligation'

    # ==== B. models enabled: a failed chain is a precise unfinished-chain obligation ====
    $chainBook = New-PreparedBook 'ChainBook' $true
    $chainOut = $chainBook.out; $chainKey = $chainBook.key
    $chainModels = Run-Digest @('ledger', 'status', '-Out', $chainOut, '-Layer', 'models')
    Assert (@($chainModels.units).Count -eq 1 -and $chainModels.units[0].state -in @('registered','ready')) 'an enabled model layer registers one model unit for the Model draft'
    $chainCycle = Invoke-CardCycle $chainOut $chainKey (Join-Path $chainOut 'cards') 5 0.5
    Assert (@($chainCycle.report.reviews | Where-Object { $_.kind -eq 'model' }).Count -eq 0 -and -not $chainCycle.report.model_review) 'a cycle that does not own the models reviews none of them twice'
    $null = Run-Digest @('ledger', 'publish', '-Out', $chainOut, '-Layer', 'cards-book')
    $blocked = Run-Digest @('ledger', 'next', '-Out', $chainOut, '-Layer', 'cards-book', '-Take', '1', '-Owner', 'cards-manager')
    Assert (@($blocked.units).Count -eq 0) 'the coverage gate waits for the delegated model review to settle'
    $modelUnit = $chainModels.units[0].id
    $modelClaim = (Run-Digest @('ledger', 'next', '-Out', $chainOut, '-Layer', 'models', '-Take', '1', '-Owner', 'model-manager')).units[0]
    Assert ($modelClaim.id -eq $modelUnit -and $modelClaim.token) 'the model chain claims its own registered unit'
    $faultFile = Join-Path $chainOut 'model-fault.json'
    Save-Json $faultFile @{ code = 'unsupported_claim'; unit = $modelUnit; input_hash = $modelClaim.input_hash; cause = '来源在这一段没有给出融资上限的推导。' }
    $null = Run-Digest @('ledger', 'fail', '-Out', $chainOut, '-Unit', $modelUnit, '-Token', "$($modelClaim.token)", '-Class', 'content', '-Fault', $faultFile)
    $chainClaim = (Run-Digest @('ledger', 'next', '-Out', $chainOut, '-Layer', 'cards-book', '-Take', '1', '-Owner', 'cards-manager')).units[0]
    Assert ($chainClaim.id -like 'cards-book/book@*') 'a settled failed chain lets the coverage gate run'
    $chainDone = Run-Digest @('book-cards-finish', '-Out', $chainOut, '-Work', $chainCycle.work, '-Token', "$($chainClaim.token)")
    $chainModel = @($chainBook.group.owned | Where-Object { $_ -match '\|b$' })[0]
    $chainCoverage = @($chainDone.coverage | Where-Object { $_.kind -eq 'model' -and $_.ref -eq $chainModel })
    Assert ($chainCoverage.Count -eq 1 -and $chainCoverage[0].owner -eq 'chain' -and $chainCoverage[0].outcome -eq 'unfinished_chain') 'a failed model chain becomes precise unfinished-chain coverage instead of a second review'
    Assert ($chainCoverage[0].unit -eq $modelUnit -and $chainDone.disposition -eq 'partial') 'the unfinished chain names its unit and leaves the revision partial'
    Assert (@($chainDone.unresolved | Where-Object { $_.unit -eq $modelUnit }).Count -eq 1) 'the unsettled model unit is named as the unresolved input of the revision'

    # ==== C. the architect's checked source-review artifact (R247, R297) ====
    $reviewWork = Join-Path $chainOut 'models\m-review'
    $step = { param($s, [string[]]$more = @()) Run-Digest (@('model', '-Step', $s, '-Work', $reviewWork, '-Staging', $chainOut) + $more) }
    $null = & $step 'brief' @('-Models', $chainModel)
    $slug = $chainModel -replace '[\\/|]', '_'
    $dir = Join-Path $reviewWork $slug
    $planFile = Join-Path $dir 'plan.json'
    $plan = [ordered]@{
        model = $chainModel; level = 'concept'; card = '问题、机制、结论'
        parts = @(
            [ordered]@{ id = 'S'; line = 'main'; facet = '设定'; emphasis = 'core'; title = '设定：企业家与投资者'; spec = '参与者与时点'; src = @('sections/s01.md:1-2') },
            [ordered]@{ id = 'P'; line = 'main'; facet = '结论'; emphasis = 'core'; title = '结论：清算价值决定借款上限'; spec = '结论'; rests = @(, @('S', '用到设定')) }
        )
    }
    Save-Json $planFile $plan
    $validate = @(& $Digest @('model', '-Step', 'validate', '-Work', $reviewWork, '-Staging', $chainOut, '-Plan', $planFile) 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($validate -join "`n") -match 'source-review: incomplete' -and ($validate -join "`n") -match 'source_review: part `P` has no reviewed source span' -and $validate[-1] -ne 'faults: 0') "a written part without a source span is a plan fault [$($validate -join ' | ')]"
    $plan.parts[1].src = @('sections/s01.md:3-4')
    Save-Json $planFile $plan
    $validate = @(& $Digest @('model', '-Step', 'validate', '-Work', $reviewWork, '-Staging', $chainOut, '-Plan', $planFile) 2>&1)
    Assert ($validate[-1] -eq 'faults: 0' -and ($validate -join "`n") -match 'source-review: complete 2 parts') "a plan whose written parts carry source spans records a complete review [$($validate -join ' | ')]"
    $artifact = Get-Content -LiteralPath (Join-Path $dir 'source-review.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($artifact.kind -eq 'model_source_review' -and $artifact.model -eq $chainModel -and $artifact.complete -and $artifact.plan.sha256) 'the artifact binds its model, its accepted plan hash and completeness'
    Assert (@($artifact.parts).Count -eq 2 -and @($artifact.parts | Where-Object { $_.outcome -eq 'supported' -and @($_.spans).Count -eq 1 }).Count -eq 2) 'each reviewed part records its outcome and its source span'
    $plan.source_review = @(@{ part = 'P'; outcome = 'supplied'; span = 'sections/s01.md:3-4' })
    Save-Json $planFile $plan
    $validate = @(& $Digest @('model', '-Step', 'validate', '-Work', $reviewWork, '-Staging', $chainOut, '-Plan', $planFile) 2>&1)
    Assert (($validate -join "`n") -match 'supplied step of `P` needs one note' -and $validate[-1] -ne 'faults: 0') "a supplied step without a note is refused [$($validate -join ' | ')]"
    $plan.source_review = @(@{ part = 'P'; outcome = 'supplied'; note = '命题的等价形式由补写给出。'; span = 'sections/s01.md:3-4' })
    Save-Json $planFile $plan
    $validate = @(& $Digest @('model', '-Step', 'validate', '-Work', $reviewWork, '-Staging', $chainOut, '-Plan', $planFile) 2>&1)
    $artifact = Get-Content -LiteralPath (Join-Path $dir 'source-review.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($validate[-1] -eq 'faults: 0' -and @($artifact.parts | Where-Object { $_.outcome -eq 'supplied' -and $_.note }).Count -eq 1) 'a supplied part is recorded with its note and keeps its span'
    $plan.source_review = @(@{ part = 'P'; outcome = 'guessed'; span = 'sections/s01.md:3-4' })
    Save-Json $planFile $plan
    $validate = @(& $Digest @('model', '-Step', 'validate', '-Work', $reviewWork, '-Staging', $chainOut, '-Plan', $planFile) 2>&1)
    Assert (($validate -join "`n") -match 'has outcome `guessed`' -and $validate[-1] -ne 'faults: 0') "an unknown review outcome is refused [$($validate -join ' | ')]"
    $plan.source_review = @(@{ part = 'P'; outcome = 'supported'; span = 'sections/never.md:1-2' })
    Save-Json $planFile $plan
    $validate = @(& $Digest @('model', '-Step', 'validate', '-Work', $reviewWork, '-Staging', $chainOut, '-Plan', $planFile) 2>&1)
    Assert (($validate -join "`n") -match 'which the prepared source does not hold') "a span the prepared source does not hold is refused [$($validate -join ' | ')]"

    # The established supplied-step markers are the only labels the card cycle accepts. The job is
    # the one that holds the sampled Idea Card; every other Card of it answers `ok`.
    $markerIdea = @($cycle.report.sample.addresses)[0]
    $markerJob = @($cycle.state.jobs.Keys | Where-Object { $markerIdea -in @($cycle.state.jobs[$_].cards) })[0]
    $markerRest = @(@($cycle.state.jobs[$markerJob].cards) | Where-Object { $_ -ne $markerIdea } | ForEach-Object { @{ address = $_; verdict = 'ok'; note = '对照原文核对无误。' } })
    $markerDraft = Join-Path $scratch 'marker-draft.json'
    Save-Json $markerDraft @{ cards = @(@{ address = $markerIdea; verdict = 'unfaithful'; title = '贷款收紧使企业减少设备投入'; body = '贷款收紧使企业减少设备投入。这一步（补充：由我补上）得到结论。' }) + $markerRest }
    $pre = @(& $Digest @('card', '-Step', 'precheck', '-Work', $cycle.work, '-Job', "$markerJob", '-File', $markerDraft) 2>&1)
    Assert (($pre -join "`n") -match 'supplied-step marker must be exactly' -and $pre[-1] -ne 'faults: 0') "a malformed supplied-step marker is refused [$($pre -join ' | ')]"
    Save-Json $markerDraft @{ cards = @(@{ address = $markerIdea; verdict = 'unfaithful'; title = '贷款收紧使企业减少设备投入'; body = '贷款收紧使企业减少设备投入。（补：书中略去）这一句由读者验证。依赖 [[CycleBook]]，后者给出全书的问题范围。来源：[[CycleBook/1]]' }) + $markerRest }
    $pre = @(& $Digest @('card', '-Step', 'precheck', '-Work', $cycle.work, '-Job', "$markerJob", '-File', $markerDraft) 2>&1)
    Assert ($pre[-1] -eq 'faults: 0') "the established marker passes the precheck [$($pre -join ' | ')]"
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'





