#Requires -Version 7.5
<#
.SYNOPSIS
  The mathematics stages' deterministic steps (math-steps.ps1, #178): one script call runs a whole chain and
  prints one compact JSON line.

.DESCRIPTION
  A stand-in zt-digest answers the math compiler's commands from a small table, so the suite proves the step's
  own behaviour without an archive, a book or a provider:
  - `skeleton` runs the statement index, the math prompt files and the skeleton in ONE call and answers each
    command's own line;
  - `notation`, `boundary`, `proof` and `theory` each run their one plan command and save the jobs the stage's
    content managers read under <staging>\math\drive;
  - `notation-apply`, `boundary-apply`, `boundary-settle`, `proof-apply` and `theory-apply` run their one apply
    command and answer its line (the proof apply's counted `ok` stays nested, never the envelope's own `ok`);
  - the sizing options reach the commands that take them;
  - `check` is the read-only M1-M18 report;
  - a failed command is one `ok:false` line with its step, never a half-written step.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$here = $PSScriptRoot
$script = Join-Path (Split-Path $here -Parent) 'math-steps.ps1'
$failed = 0
$ok = 0
function Assert([bool]$condition, [string]$name) {
    if ($condition) { $script:ok++; Write-Host "ok: $name" } else { $script:failed++; Write-Host "FAIL: $name" }
}
$scratch = Join-Path ([IO.Path]::GetTempPath()) "math-steps-test-$PID"
if (Test-Path $scratch) { Remove-Item -Recurse -Force $scratch }
$tool = Join-Path $scratch 'fake-zt-digest.ps1'
$out = Join-Path $scratch 'out'
New-Item -ItemType Directory -Force $out | Out-Null
$calls = Join-Path $scratch 'calls.txt'
$toolBody = @'
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$ErrorActionPreference = 'Stop'
$flat = $Rest -join ' '
Add-Content -LiteralPath $env:MAS_CALLS -Value $flat -Encoding utf8
$cmd = "$($Rest[0]) $($Rest[1])"
$null = $flat -match '-Step (\S+)'
if ($Matches[1]) { $cmd = "$($Rest[0]) $($Matches[1])" }
function Emit($Value) { $Value | ConvertTo-Json -Compress -Depth 16; exit 0 }
if ($env:MAS_FAIL -and $flat -match $env:MAS_FAIL) { Write-Output "zt-digest math: the compiler refused"; exit 1 }
switch -Regex ($cmd) {
    '^math index' { Emit @{ ok = $true; units = 5; forms = @{ theorem = 1; lemma = 1 }; examples = 1; exercises = 2; proofs = 2; cites = 3; file = 'D:\x\Key\math\index.json' } }
    '^math docs' { Emit @{ ok = $true; files = @('D:\x\Key\math\brief.zh.md', 'D:\x\Key\math\fill.zh.md') } }
    '^math skeleton' { Emit @{ ok = $true; units = 5; created = 5; theories = 2; applied = ($flat -match ' -Apply'); file = 'D:\x\Key\math\units.json' } }
    '^math notation-apply' {
        if ($env:MAS_NO_ANSWER -eq '1') { Emit @{ ok = $false; error = 'no notation.json' } }
        Emit @{ ok = $true; symbols = 3; card = 'Key/1|c' }
    }
    '^math notation' { Emit @{ candidates = 3; jobs = @(@{ id = 'notation'; files = @('D:\x\Key\math\notation-prefix.md'); prompt = 'p' }) } }
    '^math boundary-apply' { Emit @{ created = 3; skipped_supplied = 0; jobs = @(@{ id = 'v-b-01'; files = @('D:\x\Key\math\review-prefix.md'); prompt = 'p' }) } }
    '^math boundary-settle' { Emit @{ kept = 1; deleted = 0; unreviewed = 0; parts = @() } }
    '^math boundary' { Emit @{ units = 2; jobs = @(@{ id = 'b-01'; files = @('D:\x\Key\math\boundary-prefix.md'); prompt = 'p' }) } }
    '^math proof-apply' { Emit @{ ok = 3; fixed = 1; refused = 0; parts = @() } }
    '^math proof' { Emit @{ units = 2; jobs = @(@{ id = 'p-01'; files = @('D:\x\Key\math\proof-prefix.md'); prompt = 'p' }) } }
    '^math theory-apply' { Emit @{ ok = $true; theories = 2; answered = 2 } }
    '^math theory' { Emit @{ theories = 2; jobs = @(@{ id = 't-01'; files = @('D:\x\Key\math\theory-prefix.md'); prompt = 'p' }) } }
    '^math check' { Emit @{ ok = $true; units = 5; theories = 2; forms = @{ theorem = 1 }; faults = 0; by_rule = @{}; report = 'D:\x\Key\math-audit.md' } }
}
Emit @{ ok = $true }
'@
Set-Content -LiteralPath $tool -Value $toolBody -Encoding utf8
$env:MAS_CALLS = $calls

$script:callsMade = 0
# #180: every step of the script answers one compact line: at most 400 characters, a boolean top-level `ok`, and
# the detail file it names exists.
function Assert-Line([object]$r, [string]$name) {
    Assert ($r.last.Length -gt 0 -and $r.last.Length -le 400) "compact ${name}: the last line is at most 400 characters ($($r.last.Length))"
    Assert ($r.json -and $r.json.ok -is [bool]) "compact ${name}: the last line has a boolean top-level ok"
    Assert ($r.json -and $r.json.detail -and (Test-Path -LiteralPath $r.json.detail)) "compact ${name}: the line names its detail file and it exists"
}
function Step([string[]]$argv) {
    $text = & pwsh -NoProfile -File $script @argv 2>&1
    $code = $LASTEXITCODE
    $lines = @($text | ForEach-Object { "$_".Trim() } | Where-Object { $_ })
    $line = if ($lines.Count) { $lines[-1] } else { '' }
    $json = if ($line.StartsWith('{')) { $line | ConvertFrom-Json -Depth 16 } else { $null }
    $r = [pscustomobject]@{ code = $code; json = $json; last = $line; text = (@($text) -join ' | ') }
    $script:callsMade++
    if ($script:callsMade -gt 1) { Assert-Line $r "step $script:callsMade" }
    $r
}
$common = @('-Out', $out, '-Tool', $tool)
function Calls() { if (Test-Path -LiteralPath $calls) { Get-Content -LiteralPath $calls -Raw } else { '' } }

# 1. skeleton: index, docs and the skeleton apply, in ONE call; the answer carries each command's own line.
Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
$skeleton = Step (@('-Step', 'skeleton') + $common)
$skeletonCalls = Calls
Assert ($skeleton.code -eq 0 -and $skeleton.json -and $skeleton.json.ok -eq $true) "skeleton: the statement chain answers one ok line ($($skeleton.text))"
Assert-Line $skeleton 'skeleton'
Assert ($skeleton.json.index_units -eq 5 -and $skeleton.json.index_forms -eq 2 -and $skeleton.json.index_examples -eq 1 -and $skeleton.json.index_exercises -eq 2) 'skeleton: the index line keeps its unit, form, example and exercise counts'
Assert ($skeleton.json.docs_files -eq 2 -and $skeleton.json.skeleton_units -eq 5 -and $skeleton.json.skeleton_theories -eq 2) 'skeleton: the prompt files and the applied skeleton keep their own counts'
Assert ($skeletonCalls -match 'math -Step index -Staging ' -and $skeletonCalls -match 'math -Step docs -Staging ' -and $skeletonCalls -match 'math -Step skeleton -Staging .* -Apply') 'skeleton: one process runs index, docs and the applied skeleton'

# 2. notation: the plan command's jobs are saved for the stage's content manager.
$notation = Step (@('-Step', 'notation') + $common)
$notationJobs = Join-Path $out 'math\drive\notation-jobs.json'
Assert ($notation.json -and $notation.json.ok -eq $true -and $notation.json.candidates -eq 3 -and $notation.json.jobs -eq 1) "notation: the symbol candidates and one job ($($notation.text))"
Assert ($notation.json.file -eq $notationJobs -and (Test-Path -LiteralPath $notationJobs)) 'notation: the job list is saved under math\drive'
$saved = Get-Content -Raw $notationJobs | ConvertFrom-Json
Assert (@($saved).Count -eq 1 -and @($saved)[0].id -eq 'notation' -and @($saved)[0].files[0] -like '*notation-prefix*') 'notation: the saved job keeps the compiler''s own files'
$notationApply = Step (@('-Step', 'notation-apply') + $common)
Assert ($notationApply.json -and $notationApply.json.ok -eq $true -and $notationApply.json.symbols -eq 3 -and $notationApply.json.card -eq 'Key/1|c') "notation-apply: the applied table answers its symbols and Card ($($notationApply.text))"
Assert ((Calls) -match 'math -Step notation-apply -Staging .* -Apply') 'notation-apply: the table is written (-Apply), not planned'

# 3. boundary: the plan, then the apply that creates the parts and lists the review jobs, then the settle.
Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
$boundary = Step (@('-Step', 'boundary', '-UnitsPerJob', '6', '-Supply', 'none') + $common)
$boundaryCalls = Calls
Assert ($boundary.json -and $boundary.json.ok -eq $true -and $boundary.json.units -eq 2 -and $boundary.json.jobs -eq 1) "boundary: the planned units and one job ($($boundary.text))"
Assert ($boundaryCalls -match 'math -Step boundary -Staging .* -UnitsPerJob 6 -Supply none') 'boundary: the per-job size and the supply policy reach the plan'
$boundaryApply = Step (@('-Step', 'boundary-apply', '-Supply', 'examples') + $common)
$reviewJobs = Join-Path $out 'math\drive\boundary-review-jobs.json'
Assert ($boundaryApply.json -and $boundaryApply.json.ok -eq $true -and $boundaryApply.json.created -eq 3 -and -not $boundaryApply.json.skipped_supplied -and $boundaryApply.json.jobs -eq 1 -and $boundaryApply.json.file -eq $reviewJobs) "boundary-apply: the parts and one review job ($($boundaryApply.text))"
$boundarySettle = Step (@('-Step', 'boundary-settle') + $common)
Assert ($boundarySettle.json -and $boundarySettle.json.ok -eq $true -and $boundarySettle.json.kept -eq 1 -and -not $boundarySettle.json.deleted) "boundary-settle: the reviewed parts settle ($($boundarySettle.text))"

# 4. proof: the review plan, and the apply whose counted ok stays nested under applied.
$proof = Step (@('-Step', 'proof', '-UnitsPerJob', '2') + $common)
$proofJobs = Join-Path $out 'math\drive\proof-jobs.json'
Assert ($proof.json -and $proof.json.ok -eq $true -and $proof.json.units -eq 2 -and $proof.json.jobs -eq 1 -and $proof.json.file -eq $proofJobs) "proof: two results reviewed by one job ($($proof.text))"
Assert ((Calls) -match 'math -Step proof -Staging .* -UnitsPerJob 2') 'proof: the per-job size reaches the review plan'
$proofApply = Step (@('-Step', 'proof-apply') + $common)
Assert ($proofApply.json -and $proofApply.json.ok -eq $true -and $proofApply.json.steps_ok -eq 3 -and $proofApply.json.fixed -eq 1 -and -not $proofApply.json.refused) "proof-apply: the counted ok stays nested, so the envelope stays a boolean ($($proofApply.text))"

# 5. theory: the write plan with the main-results limit, and the applied cards.
$theory = Step (@('-Step', 'theory', '-MainResults', '9') + $common)
$theoryJobs = Join-Path $out 'math\drive\theory-jobs.json'
Assert ($theory.json -and $theory.json.ok -eq $true -and $theory.json.theories -eq 2 -and $theory.json.jobs -eq 1 -and $theory.json.file -eq $theoryJobs) "theory: two Theory Cards and one job ($($theory.text))"
Assert ((Calls) -match 'math -Step theory -Staging .* -MainResults 9') 'theory: the main-results limit reaches the plan'
$theoryApply = Step (@('-Step', 'theory-apply', '-MainResults', '9') + $common)
Assert ($theoryApply.json -and $theoryApply.json.ok -eq $true -and $theoryApply.json.theories -eq 2 -and $theoryApply.json.answered -eq 2) "theory-apply: the rewritten cards and the answers ($($theoryApply.text))"
Assert ((Calls) -match 'math -Step theory-apply -Staging .* -MainResults 9') 'theory-apply: the main-results limit reaches the apply'

# 6. check: the read-only rules report, written where -Out names it.
Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
$check = Step (@('-Step', 'check') + $common)
$checkCalls = Calls
Assert ($check.json -and $check.json.ok -eq $true -and $check.json.units -eq 5 -and $check.json.theories -eq 2 -and -not $check.json.faults) "check: the rules report answers its counts ($($check.text))"
Assert ($checkCalls -match "math -Step check -Staging $([regex]::Escape($out)) -Out $([regex]::Escape($out))") 'check: the audit files are written in the staging folder'

# 7. a plan that prints no JSON line is one failed step, never a half-written one.
$env:MAS_FAIL = 'math -Step boundary'
$bad = Step (@('-Step', 'boundary') + $common)
$env:MAS_FAIL = ''
Assert ($bad.code -eq 0 -and $bad.json -and $bad.json.ok -eq $false -and $bad.json.step -eq 'boundary') "fail: a failed plan answers one ok:false line with its step ($($bad.text))"
$env:MAS_FAIL = 'math -Step index'
$badSkeleton = Step (@('-Step', 'skeleton') + $common)
$env:MAS_FAIL = ''
Assert ($badSkeleton.code -eq 0 -and $badSkeleton.json -and $badSkeleton.json.ok -eq $false -and $badSkeleton.json.step -eq 'index') "fail: the statement chain names the command that failed ($($badSkeleton.text))"
$env:MAS_NO_ANSWER = '1'
$refused = Step (@('-Step', 'notation-apply') + $common)
$env:MAS_NO_ANSWER = ''
Assert ($refused.json -and $refused.json.ok -eq $false -and $refused.json.error -eq 'no notation.json') "notation-apply: a refused table answers ok:false with the tool's reason, so the digest can go on ($($refused.text))"

# 8. the step's defaults hold when the module passes no sizing option.
Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
$defaults = Step (@('-Step', 'boundary') + $common)
$defaultCalls = Calls
Assert ($defaults.json -and $defaults.json.ok -eq $true -and $defaultCalls -match 'math -Step boundary -Staging .* -UnitsPerJob 4 -Supply examples') 'defaults: the script holds its own per-job size (4) and supply policy (examples)'

Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
Write-Host ''
Write-Host "math-steps: $ok passed, $failed failed"
if ($failed) { exit 1 } else { exit 0 }



