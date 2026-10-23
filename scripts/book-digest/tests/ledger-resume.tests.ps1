#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\ledger-resume.tests.ps1
# R278-R285 (#87): one resume of a prepared run. The command verifies the persisted identity,
# reconciles stranded claims and retained receipts, starts invocation 2, resets only the selected
# unresolved units, returns blocked dependants to dependency evaluation and records resume_count.
# A group whose accepted plan already committed is recovered from its archive receipts without
# rebuilding its Cards. One scratch archive; the live archive is never touched.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-resume-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest; $env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$out = Join-Path $scratch 'prepared'
function Assert($condition, $message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments, [int]$ExpectedExit = 0) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE -ne $ExpectedExit) { throw "digest failed ($LASTEXITCODE expected $ExpectedExit): $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
function Refused([string[]]$Arguments, [string]$Text, [string]$Message) {
    $lines = @(& $Digest @Arguments 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($lines -join ' ').Contains($Text)) "$message [$($lines -join ' ')]"
}
function Save-Json($Path, $Value) { [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 30), $utf8) }
function Unit([string]$id) { return @((Run-Digest @('ledger', 'status', '-Out', $out)).units | Where-Object { $_.id -eq $id })[0] }
function Revision { return (Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @(); view = 'text' } })).archive_revision }
# The durable ledger rows, so a refused resume can be shown to write nothing at all.
$inspect = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-resume-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p,{readOnly:true});
console.log(JSON.stringify({meta:db.prepare('select key,value from meta order by key').all(),units:db.prepare('select * from units order by id').all(),events:db.prepare('select * from events order by seq').all(),outputs:db.prepare('select * from outputs order by unit_id,output_key').all()}));db.close();
'@
function Evidence { return (& node --no-warnings -e $inspect (Join-Path $out 'ledger.sqlite3')) }
# An isolated ledger mutation, scoped by the same scratch guard (never the live archive).
$mutator = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-resume-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.exec('pragma foreign_keys=on');db.exec(process.argv[2]);db.close();
'@
function Mutate([string]$Sql) { & node --no-warnings -e $mutator (Join-Path $out 'ledger.sqlite3') $Sql; if ($LASTEXITCODE) { throw 'isolated ledger mutation failed' } }
function Group-Draft($claim, $started, $calls) { return @{ schema_version = 1; unit = $claim.id; token = $claim.token; input_hash = $started.input_hash; disposition = 'cards'; calls = $calls } }
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null; zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source, "# 第一章`n`n融资约束改变企业的生产选择。`n`n# 第二章`n`n贷款收紧使企业减少设备投入。`n`n# 第三章`n`n投入减少限制下一期的生产能力。`n`n# 参考`n未选择。", $utf8)
    $options = Join-Path $scratch 'options.json'; Save-Json $options @{ terms = $false }
    $key = 'ResumeBook'
    $null = Run-Digest @('prepare', '-Source', $source, '-Out', $out, '-CitationKey', $key, '-BibTeX', "@book{$key,title={Resume book}}", '-StartHeading', '第一章', '-EndHeading', '参考', '-MinUnitChars', '1', '-MaxUnits', '1', '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')

    # --- AC1/T21: a legacy staging folder is adoption work, not an inferable resume ----------
    $legacy = Join-Path $scratch 'legacy-staging'
    New-Item -ItemType Directory -Force (Join-Path $legacy 'groups') | Out-Null
    [IO.File]::WriteAllText((Join-Path $legacy 'groups\g01.md'), "# 旧组`n`n旧文本。", $utf8)
    Refused @('ledger', 'resume', '-Out', $legacy) 'legacy staging' 'a staging folder without a prepared manifest is legacy adoption, never an inferred resume'
    $manifestCopy = Join-Path $scratch 'manifest-only'
    New-Item -ItemType Directory -Force $manifestCopy | Out-Null
    Copy-Item -LiteralPath (Join-Path $out 'manifest.json') -Destination (Join-Path $manifestCopy 'manifest.json')
    Refused @('ledger', 'resume', '-Out', $manifestCopy) 'no prepared run ledger' 'a manifest without the run ledger cannot be resumed'

    # --- AC1/T21: identity, contract and archive are verified before any write ----------------
    $evidence = Evidence
    $sourceBytes = [IO.File]::ReadAllBytes($source)
    [IO.File]::AppendAllText($source, "`n已改写。", $utf8)
    Refused @('ledger', 'resume', '-Out', $out) 'source hash mismatch' 'a changed source is refused before the resume writes'
    [IO.File]::WriteAllBytes($source, $sourceBytes)
    $groupFile = Join-Path $out 'groups\g01.md'; $groupBytes = [IO.File]::ReadAllBytes($groupFile)
    [IO.File]::AppendAllText($groupFile, "`n改变了小组输入。", $utf8)
    Refused @('ledger', 'resume', '-Out', $out) 'group hash mismatch' 'a changed prepared group is refused before the resume writes'
    [IO.File]::WriteAllBytes($groupFile, $groupBytes)
    $manifestPath = Join-Path $out 'manifest.json'
    $originalManifest = [IO.File]::ReadAllBytes($manifestPath)
    $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $manifest.run_contract.contracts.base_version = '0.0.1'
    [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 70), $utf8)
    Refused @('ledger', 'resume', '-Out', $out) 'incompatible workflow contract' 'an incompatible workflow contract is refused before the resume writes'
    [IO.File]::WriteAllBytes($manifestPath, $originalManifest)
    $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $manifest.run_contract.archive_identity.id = '00000000000000000000000000000000'
    [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 70), $utf8)
    Refused @('ledger', 'resume', '-Out', $out) 'archive identity mismatch' 'another archive identity at the same path is refused before the resume writes'
    [IO.File]::WriteAllBytes($manifestPath, $originalManifest)
    Assert ((Evidence) -ceq $evidence) 'every refused resume leaves every ledger row byte-identical'

    # --- the fixture: one done group, one applied-but-lost group, one content failure --------
    $claim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'done-manager')).units[0]
    Assert ($claim.id -eq 'digest/g001@1') 'the first prepared group is the first claim'
    $started = Run-Digest @('group-start', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)")
    $job = Get-Content -LiteralPath $started.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $body = '融资成本影响企业的生产选择。贷款收紧使企业减少设备投入。来源：[[ResumeBook/1]]'
    $draft = Group-Draft $claim $started @(
        @{ tool = 'edit_card'; arguments = @{ at = "$key/1"; title = '融资约束影响企业投资'; body = '融资约束改变企业的投资选择。企业减少设备投入，生产能力随之下降。'; base_fingerprint = $job.inputs.anchor.fingerprint } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = '贷款收紧限制企业投入'; body = $body } }
    )
    Save-Json $started.draft $draft
    $done = Run-Digest @('group-finish', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)", '-Draft', $started.draft)
    Assert ($done.state -eq 'done' -and @($done.children).Count -eq 4) 'the first group completes with its checked children registered'
    $doneBefore = Unit 'digest/g001@1'
    $revisionBefore = Revision

    # Concurrent lanes move the archive revision while a command checks its preparation: the check reads
    # its view again instead of failing, and refuses only when every read raced.
    # A renew of a token nobody holds passes the check and is refused for the token alone.
    $probe = @('ledger', 'renew', '-Out', $out, '-Unit', 'digest/g002@1', '-Token', '999')
    $env:ZT_LEDGER_VIEW_RACE = '4'
    $raced = @(& $Digest @probe 2>&1) -join ' '
    Assert ($LASTEXITCODE -ne 0 -and -not $raced.Contains('archive changed')) "a preparation check that races four times reads its view again and passes [$raced]"
    $env:ZT_LEDGER_VIEW_RACE = '5'
    Refused $probe 'archive changed while checking preparation' 'a preparation check whose every read races is refused'
    Remove-Item Env:ZT_LEDGER_VIEW_RACE

    # Group 2 accepts its plan and commits its Cards, then the ledger completion is lost.
    $lostClaim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'lost-manager')).units[0]
    Assert ($lostClaim.id -eq 'digest/g002@1') 'the second group is claimed next'
    $lostStart = Run-Digest @('group-start', '-Out', $out, '-Group', '2', '-Token', "$($lostClaim.token)")
    $lostJob = Get-Content -LiteralPath $lostStart.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $lostDraft = Group-Draft $lostClaim $lostStart @(
        @{ tool = 'edit_card'; arguments = @{ at = "$key/2"; title = '贷款收紧减少设备投入'; body = '贷款收紧使企业减少设备投入，融资成本决定投入规模。'; base_fingerprint = $lostJob.inputs.anchor.fingerprint } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/2"; kind = 'side'; title = '融资成本决定投入规模'; body = '融资成本上升使企业缩小设备投资。投入规模随融资成本下降。来源：[[ResumeBook/2]]'; src = '第二节：融资成本与设备投资' } }
    )
    Save-Json $lostStart.draft $lostDraft
    $env:ZT_LEDGER_COMPLETE_FAULT = 'before_commit'
    Refused @('group-finish', '-Out', $out, '-Group', '2', '-Token', "$($lostClaim.token)", '-Draft', $lostStart.draft) 'injected completion fault' 'the lost reply commits the Cards but rolls back the ledger completion'
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT
    Assert ((Unit 'digest/g002@1').state -eq 'claimed' -and (Test-Path -LiteralPath (Join-Path $out 'accepted-g002.json')) -and (Revision) -gt $revisionBefore) 'the retained accepted plan and the committed Cards survive the lost ledger completion'

    # A Cards failure in the first group leaves its Link obligation blocked at dependency evaluation.
    $cardsClaim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'cards', '-Take', '1', '-Owner', 'cards-manager')).units[0]
    Assert ($cardsClaim.id -eq 'cards/g001@1') 'the first group registers its Cards obligation'
    $faultPath = Join-Path $scratch 'fault.json'
    Save-Json $faultPath @{ schema_version = 1; unit = $cardsClaim.id; input_hash = $cardsClaim.input_hash; code = 'check_failed'; cause = 'the first group holds one unchecked Card' }
    $cardsFailed = Run-Digest @('ledger', 'fail', '-Out', $out, '-Unit', $cardsClaim.id, '-Token', "$($cardsClaim.token)", '-Class', 'content', '-Fault', $faultPath)
    Assert ($cardsFailed.state -eq 'failed' -and (Unit 'link/g001@1').state -eq 'blocked') 'the Cards failure blocks the Link obligation of the same group'

    # Group 3 records a content failure of its own: more than one layer starts unresolved.
    $badClaim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'bad-manager')).units[0]
    Assert ($badClaim.id -eq 'digest/g003@1') 'the third group is claimed next'
    Save-Json $faultPath @{ schema_version = 1; unit = $badClaim.id; input_hash = $badClaim.input_hash; code = 'check_failed'; cause = 'the third group draft holds an unsourced claim' }
    $failed = Run-Digest @('ledger', 'fail', '-Out', $out, '-Unit', $badClaim.id, '-Token', "$($badClaim.token)", '-Class', 'content', '-Fault', $faultPath)
    Assert ($failed.state -eq 'failed' -and (Unit 'digest/g003@1').state -eq 'failed') 'a second layer records its own content failure'

    # --- AC1/R279: a live owner cannot be resumed over --------------------------------
    $evidence = Evidence
    Refused @('ledger', 'resume', '-Out', $out) 'run_active' 'a live claim of another invocation is refused before any ownership change'
    Assert ((Evidence) -ceq $evidence -and (Unit 'digest/g002@1').state -eq 'claimed') 'the refused resume leaves the live claim and every ledger row unchanged'

    # The owner is lost: its lease expires, as it does after a crashed manager.
    Mutate "update units set lease_until=cast(strftime('%s','now') as integer)-1 where id='digest/g002@1' and claim_token=$($lostClaim.token) and state='claimed'"

    # --- AC2/AC3/AC5: one resume opens invocation 2 -----------------------------------------
    $evidence = Evidence
    $resumed = Run-Digest @('ledger', 'resume', '-Out', $out)
    Assert ($resumed.status -eq 'resumed' -and $resumed.resumed -eq $true -and $resumed.invocation -eq 2 -and $resumed.resume_count -eq 1) 'one resume opens invocation 2 and records resume_count 1'
    Assert ($resumed.run_id -eq (Run-Digest @('ledger', 'status', '-Out', $out)).run_id -and $resumed.citation_key -eq $key -and $resumed.settings.max_attempts -eq 3) 'the resume reads the prepared identity, Citation key and effective settings instead of new arguments'
    Assert (@($resumed.reclaimed | Where-Object unit -EQ 'digest/g002@1').Count -eq 1 -and @($resumed.reset | Where-Object unit -EQ 'digest/g002@1').Count -eq 1) 'the stranded claim is reclaimed and its unresolved attempt budget is reset'
    Assert (@($resumed.reset | Where-Object unit -EQ 'digest/g003@1').Count -eq 1 -and @($resumed.reset | Where-Object unit -EQ 'cards/g001@1').Count -eq 1) 'every failed unit of both unfinished layers is reset'
    Assert (@($resumed.reevaluated | Where-Object unit -EQ 'link/g001@1').Count -eq 1 -and (Unit 'link/g001@1').attempts -eq 0) 'the blocked dependant returns to dependency evaluation without spending an attempt'
    Assert (@($resumed.reset | Where-Object unit -EQ 'digest/g001@1').Count -eq 0 -and (@($resumed.retained.units) -contains 'digest/g001@1')) 'a done build is retained and never reset by the resume'
    $doneAfter = Unit 'digest/g001@1'
    Assert ($doneAfter.state -eq 'done' -and $doneAfter.attempts -eq $doneBefore.attempts -and $doneAfter.result_json -ceq $doneBefore.result_json -and $doneAfter.completed_at -ceq $doneBefore.completed_at) 'the neighbouring done completion keeps its exact result, attempt count and time'
    Assert ((Unit 'digest/g002@1').state -eq 'ready' -and (Unit 'digest/g002@1').attempts -eq 0 -and @($resumed.ready | Where-Object layer -EQ 'digest')[0].count -eq 2) 'the reclaimed group is eligible again with no spent attempt, beside the reset group'
    $history = Run-Digest @('ledger', 'status', '-Out', $out)
    $invocationOne = @($history.counts.invocations | Where-Object invocation -EQ 1)[0]
    $invocationTwo = @($history.counts.invocations | Where-Object invocation -EQ 2)[0]
    Assert ($invocationOne.claimed -ge 4 -and $invocationOne.completed -ge 1 -and $invocationTwo -and [int]$invocationTwo.claimed -eq 0) 'the attempt history is retained under invocation 1 while invocation 2 starts without a duplicate claim'
    Assert ((Unit 'digest/g003@1').fault.code -eq 'check_failed' -and (Unit 'cards/g001@1').fault.code -eq 'check_failed') 'the recorded content faults stay visible across the resume'
    Assert ('digest/g002@1' -in $resumed.closure.prerequisites -and 'digest/g003@1' -in $resumed.closure.prerequisites -and 'cards/g001@1' -in $resumed.closure.prerequisites -and 'link/g001@1' -in $resumed.closure.downstream) 'the resume publishes the unresolved prerequisite and downstream closure it will drain'

    # --- AC5: the one supported resume cannot be spent twice ---------------------------------
    $evidence = Evidence
    Refused @('ledger', 'resume', '-Out', $out) 'resume_used' 'a second resume is refused because the one-rerun policy is spent'
    Assert ((Evidence) -ceq $evidence) 'the refused second resume writes nothing'

    # --- AC2/R280: the retained accepted plan completes from archive receipts ---------------
    $receiptRevision = Revision
    $recoveredClaim = (Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'recovery-manager')).units[0]
    Assert ($recoveredClaim.id -eq 'digest/g002@1' -and $recoveredClaim.token -gt $lostClaim.token) 'the recovered group is claimed with a newer token than the lost attempt'
    $recoveredStart = Run-Digest @('group-start', '-Out', $out, '-Group', '2', '-Token', "$($recoveredClaim.token)")
    $recoveredJob = Get-Content -LiteralPath $recoveredStart.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($recoveredStart.recovered -eq $true -and $recoveredStart.reused -eq $true -and $recoveredJob.expected_jobs -eq 0 -and @($recoveredJob.jobs).Count -eq 0) 'an accepted plan whose receipts already committed starts no DeepSeek job'
    Assert ($recoveredJob.recovery.reason -eq 'receipts_committed' -and @($recoveredJob.recovery.receipts).Count -eq 2 -and (Split-Path -Leaf $recoveredJob.recovery.plan) -eq 'accepted-g002.json' -and (Test-Path -LiteralPath $recoveredJob.recovery.plan)) 'the recovery names the retained plan and every committed receipt it reconciles'
    $recoveredDraft = Get-Content -LiteralPath $recoveredStart.draft -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($recoveredDraft.token -eq $recoveredClaim.token -and $recoveredDraft.input_hash -eq $recoveredStart.input_hash -and $recoveredDraft.calls.Count -eq 2) 'the rebound draft carries the current claim token and the retained calls'
    Assert ($recoveredDraft.calls[1].arguments.src -eq '第二节：融资成本与设备投资') 'the rebound draft restores the source note the accepted plan keeps apart from its calls'
    $recovered = Run-Digest @('group-finish', '-Out', $out, '-Group', '2', '-Token', "$($recoveredClaim.token)", '-Draft', $recoveredStart.draft)
    Assert ($recovered.state -eq 'done' -and @($recovered.owned).Count -eq 2 -and (Revision) -eq $receiptRevision) 'the recovered group completes from its committed receipts without writing another Card'
    $tree = Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $key } })
    Assert (@($tree.cards | Where-Object { $_.title -eq '贷款收紧减少设备投入' }).Count -eq 1 -and @($tree.cards | Where-Object { $_.title -eq '融资约束影响企业投资' }).Count -eq 1) 'recovery duplicates no Card and leaves the completed neighbour untouched'

    # --- T20/AC5: persistent failures are a manual worklist, never another resume -----------
    $report = Run-Digest @('ledger', 'report', '-Out', $out)
    Assert ($report.identity.invocation -eq 2 -and $report.identity.resume_count -eq 1) 'the run report states the second invocation and the spent resume'
    Assert ($report.resume.available -eq $false -and @($report.resume.manual).Count -ge 1) 'after the resume the report lists manual decisions instead of another automatic resume'
    $manualUnits = @($report.gaps | Where-Object { $_.unit -eq 'digest/g003@1' })
    Assert ($manualUnits.Count -eq 1 -and $manualUnits[0].code -eq 'check_failed') 'the persistent failure stays a named gap with its recorded cause'
    # --- R284: a resume of a run with no executable work changes nothing and spends nothing -----
    $idleOut = Join-Path $scratch 'idle'
    $null = Run-Digest @('prepare', '-Source', $source, '-Out', $idleOut, '-CitationKey', 'IdleBook', '-BibTeX', '@book{IdleBook,title={Idle book}}', '-StartHeading', '第一章', '-EndHeading', '参考', '-MinUnitChars', '1', '-MaxUnits', '1', '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')
    $idleInspect = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-resume-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p);db.exec("update units set state='done',completed_at=coalesce(completed_at,datetime('now')),result_json=coalesce(result_json,'{\"disposition\":\"verified\"}') where state!='done'");db.close();
'@
    & node --no-warnings -e $idleInspect (Join-Path $idleOut 'ledger.sqlite3'); if ($LASTEXITCODE) { throw 'isolated idle fixture failed' }
    $idleEvidence = (& node --no-warnings -e $inspect (Join-Path $idleOut 'ledger.sqlite3'))
    $idle = Run-Digest @('ledger', 'resume', '-Out', $idleOut)
    Assert ($idle.status -eq 'nothing_to_resume' -and $idle.resumed -eq $false -and $idle.invocation -eq 1 -and $idle.resume_count -eq 0) 'a run with no executable work reports nothing_to_resume and spends no invocation'
    Assert (@($idle.coverage_faults).Count -ge 1) 'the settled-but-incomplete coverage stays reported instead of becoming a new attempt'
    Assert (((& node --no-warnings -e $inspect (Join-Path $idleOut 'ledger.sqlite3'))) -ceq $idleEvidence) 'the no-work resume leaves every ledger row byte-identical'
} finally {
    Remove-Item Env:ZT_LEDGER_COMPLETE_FAULT -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
