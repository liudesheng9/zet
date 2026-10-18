#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\ledger-adopt.tests.ps1
# R410-R417 (#88): legacy adoption. The command reads a legacy staging folder, classifies its
# evidence read-only, and writes the ledger only on -Apply. A skeleton anchor is never digest
# success, an adopted unit carries a dated read-only verification instead of a pretended historic
# SELF-CHECK, a verified model stays built and only its missing explanation becomes a unit, and a
# folder without enough source/group evidence names the missing scope and writes nothing.
# One scratch archive; the live archive is never touched.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "ledger-adopt-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest; $env:ZT_CONFIG_DIR = Join-Path $scratch 'config'; $env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
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
function Unit($out, [string]$id) { return @((Run-Digest @('ledger', 'status', '-Out', $out)).units | Where-Object { $_.id -eq $id })[0] }
# The durable ledger rows, so a refused or read-only operation can be shown to write nothing.
$inspect = @'
const {DatabaseSync}=require('node:sqlite');const path=require('node:path');const os=require('node:os');
const p=path.resolve(process.argv[1]);
if(!p.startsWith(path.resolve(os.tmpdir())+path.sep)||!path.basename(path.dirname(path.dirname(p))).startsWith('ledger-adopt-test-'))throw new Error('not the isolated ledger');
const db=new DatabaseSync(p,{readOnly:true});
console.log(JSON.stringify({meta:db.prepare('select key,value from meta order by key').all(),units:db.prepare('select * from units order by id').all(),events:db.prepare('select * from events order by seq').all(),outputs:db.prepare('select * from outputs order by unit_id,output_key').all()}));db.close();
'@
function Evidence($out) { return (& node --no-warnings -e $inspect (Join-Path $out 'ledger.sqlite3')) }
function Tree($key) { return (Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $key } }))[0] }
function Address-Of($tree, [string]$title) { return @($tree.cards | Where-Object { $_.title -eq $title })[0].address }
$archive = Join-Path $scratch 'archive'
$book = Join-Path $scratch 'book.md'
$key = 'AdoptBook'
try {
    zt config set archive_root $archive | Out-Null; zt up | Out-Null
    [IO.File]::WriteAllText($book, "# 第一章`n`n融资约束改变企业的生产选择。`n`n# 第二章`n`n贷款收紧使企业减少设备投入。`n`n# 第三章`n`n投入减少限制下一期的生产能力。`n`n# 参考`n未选择。", $utf8)
    $options = Join-Path $scratch 'options.json'; Save-Json $options @{ terms = $false }

    # --- the fixture: a staging folder as the legacy run left it ------------------------------
    # `prepare -Apply` writes the true legacy artifacts (groups, sections, index, book, figures).
    # The test then deletes everything the ledger era added, so the folder predates the ledger.
    $legacy = Join-Path $scratch 'legacy'
    $null = Run-Digest @('prepare', '-Source', $book, '-Out', $legacy, '-CitationKey', $key, '-BibTeX', "@book{$key,title={Adopt book}}", '-StartHeading', '第一章', '-EndHeading', '参考', '-MinUnitChars', '1', '-MaxUnits', '1', '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')
    foreach ($name in @('manifest.json', 'ledger.sqlite3', 'events.json', 'shared-prefix.md', 'link-prefix.md', 'setup-calls.json', 'preparation.json', 'source-faults.md', 'source-faults.json')) {
        $path = Join-Path $legacy $name
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
    }
    $legacy = (Resolve-Path $legacy).Path
    Assert (-not (Test-Path (Join-Path $legacy 'manifest.json'))) 'the fixture is a legacy staging folder with no prepared manifest'
    $anchor1 = "$key/1"; $anchor2 = "$key/2"; $anchor3 = "$key/3"

    # The digest wrote group 1 and group 3; group 2 keeps its skeleton anchor only.
    $model1 = '融资成本与投入的模型'
    $null = Invoke-Zt @(
        @{ tool = 'edit_card'; arguments = @{ at = $anchor1; title = '融资约束改变企业的生产选择'; body = '融资成本影响企业的投资；贷款收紧减少设备投入。来源：[[AdoptBook]]' } },
        @{ tool = 'create_successor'; arguments = @{ at = $anchor1; kind = 'side'; title = '融资成本决定企业投入规模'; body = '融资成本决定企业的设备投入。来源：[[AdoptBook/1]]' } },
        @{ tool = 'create_successor'; arguments = @{ at = $anchor1; kind = 'side'; title = $model1; body = "问题：融资成本如何改变投入。`n`n**结构**`n`n- 设定（核心）：企业以外部资金购买设备"; type = 'model' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@2'; kind = 'side'; title = '设定：企业以外部资金购买设备'; body = '企业以外部资金购买设备，投入随融资成本下降。'; type = 'model'; facet = '设定' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@3'; kind = 'side'; title = '推导：投入随融资成本下降'; body = '推导：融资成本上升使投入下降。`n`n结果：投入随融资成本反向变化。'; type = 'model'; facet = '推导：投入下降' } },
        @{ tool = 'create_successor'; arguments = @{ at = $anchor1; kind = 'side'; title = '术语表：融资约束与投入'; body = "- 融资约束：企业投资受外部资金限制的状态。`n- 投入：企业当期购买的设备。" } },
        @{ tool = 'edit_card'; arguments = @{ at = $anchor3; title = '投入下降压低下一期生产能力'; body = '投入减少限制下一期的生产能力。来源：[[AdoptBook]]' } },
        @{ tool = 'create_successor'; arguments = @{ at = $anchor3; kind = 'side'; title = '生产能力约束企业的下一期产出'; body = '生产能力决定下一期产出。来源：[[AdoptBook/3]]' } },
        @{ tool = 'create_successor'; arguments = @{ at = $anchor3; kind = 'side'; title = '未完成的模型'; body = "问题：生产能力如何限制产出。`n`n**结构**`n`n- 设定（核心）：下一期生产能力已定"; type = 'model' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@8'; kind = 'side'; title = '设定：下一期生产能力已定'; body = '待写'; type = 'model'; facet = '设定' } }
    )
    $tree = Tree $key
    $modelAddress = Address-Of $tree $model1
    $builtModelAddress = Address-Of $tree '未完成的模型'
    Assert ($modelAddress.StartsWith("$anchor1|") -and $builtModelAddress.StartsWith("$anchor3|")) 'the seeded models are filed under their group anchors'
    $group1Cards = @($tree.cards | Where-Object { $_.address -eq $anchor1 -or $_.address.StartsWith("$anchor1|") } | ForEach-Object { $_.address })
    Assert ($group1Cards.Count -ge 5) 'group 1 holds its anchor and the Cards it wrote'

    # The owned and source logs the legacy digest helper wrote.
    [IO.File]::WriteAllText((Join-Path $legacy 'owned-g01.txt'), (($group1Cards -join "`n") + "`n"), $utf8)
    [IO.File]::WriteAllText((Join-Path $legacy 'src-g01.txt'), "$anchor1`t第一章 融资约束改变企业的生产选择`n", $utf8)
    [IO.File]::WriteAllText((Join-Path $legacy 'terms.md'), "# 术语表`n`n- 融资约束：企业投资受外部资金限制的状态。`n", $utf8)

    # The retained model compiler state: one built model with its accepted plan and a note that
    # names a Link, plus a prose note the notes cycle cannot dispose of.
    $batch = Join-Path $legacy 'models\m-AdoptBook_1_c'
    $slug = $modelAddress.Replace('/', '_').Replace('|', '_')
    New-Item -ItemType Directory -Force (Join-Path $batch $slug) | Out-Null
    $plan = Join-Path $batch "$slug\plan.json"
    Save-Json $plan @{
        model = $modelAddress; schema_version = 1; status = 'planned'; level = 'concept'
        parts = @(@{ id = 'p1'; title = '设定：企业以外部资金购买设备' }, @{ id = 'p2'; title = '推导：投入随融资成本下降'; derive = $true })
        notes = @('建议补充 承接 [[AdoptBook/2]]：本条说明融资成本的作用。')
    }
    $planHash = (Get-FileHash -LiteralPath $plan -Algorithm SHA256).Hash.ToLowerInvariant()
    Save-Json (Join-Path $batch 'state.json') @{
        order = @($modelAddress); seq = 1
        models = @{ $modelAddress = @{
            status = 'built'; plan_hash = $planHash; plan = @{ path = $plan; sha256 = $planHash }
            notes = @('not merged: 该部分的相邻卡片仍有后继。')
        } }
    }

    # --- R410: `ledger init` never fabricates a legacy folder's history -----------------------
    $archiveIdentity = (Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @(); view = 'text' } }))[0].archive_identity

    # --- R412/R413: the read-only plan --------------------------------------------------------
    $planFile = Join-Path $legacy 'adoption\plan.json'
    $plan = Run-Digest @('ledger', 'adopt', '-Out', $legacy, '-Variant', 'econ')
    Assert ($plan.kind -eq 'adoption_plan' -and $plan.status -eq 'ready') 'the adoption plan is ready for a folder with source and group evidence'
    Assert ($plan.citation_key -eq $key -and $plan.identity.citation_key_source -eq 'sections/index.md' -and $plan.identity.contract.origin -eq 'adopted') 'the plan resolves the Citation key from the retained index and records the contract as adopted'
    Assert ($plan.archive.id -eq $archiveIdentity.id -and $plan.identity.archive_identity.id -eq $archiveIdentity.id) 'the plan binds the live archive identity'
    Assert ($plan.evidence.source.sha256 -eq (Get-FileHash -LiteralPath (Join-Path $legacy 'book.zh.md') -Algorithm SHA256).Hash.ToLowerInvariant()) 'the plan records the frozen source by its actual hash'
    Assert ($plan.groups.Count -eq 3 -and $plan.evidence.groups -eq 3) 'every prepared group is in the plan'
    $g1 = @($plan.groups | Where-Object n -EQ 1)[0]; $g2 = @($plan.groups | Where-Object n -EQ 2)[0]; $g3 = @($plan.groups | Where-Object n -EQ 3)[0]
    Assert ($g1.state -eq 'verified' -and $g1.owned_source -eq 'owned_log' -and $g1.log_entries -eq $group1Cards.Count) 'a digested group verifies from its owned log and its substantive Cards'
    # T14: the skeleton anchor alone is never digest success.
    Assert ($g2.state -eq 'incomplete' -and $g2.reason.Contains('skeleton anchor') -and $g2.substantive -eq 0) 'a group with a skeleton anchor and no substantive Card stays incomplete'
    Assert ($g3.state -eq 'verified' -and $g3.owned_source -eq 'reconstructed') 'a group without an owned log verifies from the Cards its anchor holds now'
    Assert (@($plan.classification.verified | Where-Object unit -EQ 'digest/g001@1').Count -eq 1 -and @($plan.classification.incomplete | Where-Object unit -EQ 'digest/g002@1').Count -eq 1) 'the classification names the verified and the incomplete group'
    $verifiedModels = @($plan.models | Where-Object state -EQ 'verified')
    $unfinishedModels = @($plan.models | Where-Object state -EQ 'incomplete')
    Assert ($verifiedModels.Count -eq 1 -and $verifiedModels[0].address -eq $modelAddress -and $verifiedModels[0].plan_identity -eq 'retained_plan') 'the built model verifies read-only and keeps its retained plan'
    Assert ($unfinishedModels.Count -eq 1 -and $unfinishedModels[0].address -eq $builtModelAddress -and @($unfinishedModels[0].placeholders).Count -eq 1) 'the model whose part still holds 待写 is incomplete with its exact placeholder'
    Assert (@($plan.classification.incomplete | Where-Object kind -EQ 'explain').Count -eq 1) 'the verified model with an unexplained Derivation part gets an explanation obligation'
    Assert ($plan.notes.Count -eq 1 -and $plan.notes[0].card -eq $modelAddress) 'the retained note that names a Link becomes a recovered note suggestion'
    Assert (@($plan.classification.unknown | Where-Object { $_.what -eq 'retained model note' })[0].count -eq 1) 'a retained note that names no Link is a recorded fact, not an invented suggestion'
    Assert (@($plan.classification.unknown | Where-Object { $_.what -eq 'Model source review' }).Count -ge 1) 'a model without a retained source review is a named provenance gap'
    Assert (@($plan.targeted | Where-Object unit -EQ 'digest/g002@1').Count -eq 1 -and @($plan.targeted | Where-Object unit -EQ "models/adopted-$($builtModelAddress.Replace('/','-').Replace('|','-'))@1").Count -eq 1) 'every missing group and unfinished model is a targeted obligation'
    Assert (@($plan.proposal.units | Where-Object { $_.unit -eq 'digest/g001@1' })[0].state -eq 'done' -and @($plan.proposal.units | Where-Object { $_.unit -eq 'digest/g002@1' })[0].state -eq 'registered') 'the proposal adopts the verified unit and registers the incomplete one'
    Assert (-not (Test-Path -LiteralPath (Join-Path $legacy 'ledger.sqlite3')) -and -not (Test-Path -LiteralPath (Join-Path $legacy 'manifest.json'))) 'a read-only plan writes no ledger and no prepared manifest'
    Assert ((Test-Path -LiteralPath $planFile) -and $plan.plan.sha256 -eq (Get-FileHash -LiteralPath $planFile -Algorithm SHA256).Hash.ToLowerInvariant()) 'the plan is a separately identifiable artifact with its own hash'
    Assert (@($plan.derived | Where-Object artifact -EQ 'shared-prefix.md').Count -eq 1 -and @($plan.derived | Where-Object artifact -EQ 'events.json').Count -eq 1) 'the plan names every artifact it derives instead of inventing them'

    # --- R417: not enough evidence names the missing scope and writes nothing ------------------
    $empty = Join-Path $scratch 'empty-legacy'
    New-Item -ItemType Directory -Force (Join-Path $empty 'groups') | Out-Null
    [IO.File]::WriteAllText((Join-Path $empty 'groups\g01.md'), "# 旧组`n`n旧文本。", $utf8)
    $missing = Run-Digest @('ledger', 'adopt', '-Out', $empty, '-Variant', 'econ') 3
    Assert ($missing.status -eq 'insufficient_evidence' -and @($missing.missing)[0].what.Contains('legacy preparation index')) 'a folder without the preparation index names the missing scope'
    Refused @('ledger', 'adopt', '-Out', $empty, '-Variant', 'econ', '-Apply') 'insufficient_evidence' 'applying an insufficient plan writes no ledger'
    Assert (-not (Test-Path (Join-Path $empty 'ledger.sqlite3')) -and -not (Test-Path (Join-Path $empty 'manifest.json'))) 'the refused apply leaves no ledger and no manifest'
    # An index that names another book is not this folder's evidence either.
    $foreign = Join-Path $scratch 'foreign'
    New-Item -ItemType Directory -Force $foreign | Out-Null
    Copy-Item -LiteralPath (Join-Path $legacy 'book.zh.md') -Destination $foreign
    Copy-Item -LiteralPath (Join-Path $legacy 'sections') -Destination $foreign -Recurse
    Copy-Item -LiteralPath (Join-Path $legacy 'groups') -Destination $foreign -Recurse
    $indexPath = Join-Path $foreign 'sections\index.md'
    [IO.File]::WriteAllText($indexPath, ([IO.File]::ReadAllText($indexPath) -replace "$key/", 'AnotherBook/'), $utf8)
    Refused @('ledger', 'adopt', '-Out', $foreign, '-Variant', 'econ') 'insufficient_evidence' 'an index naming a book the archive does not hold is insufficient evidence'

    # --- R416/R413: apply writes the ledger from that plan ------------------------------------
    $applied = Run-Digest @('ledger', 'adopt', '-Out', $legacy, '-Variant', 'econ', '-Apply')
    Assert ($applied.status -eq 'applied' -and $applied.kind -eq 'adoption_applied' -and $applied.resume_count -eq 0) 'apply writes the adopted ledger from the plan'
    Assert ($applied.plan.sha256 -eq $plan.plan.sha256 -and (Test-Path -LiteralPath $applied.manifest.path) -and (Test-Path -LiteralPath (Join-Path $legacy 'adoption\applied.json'))) 'the plan, the prepared manifest and the applied record are separate artifacts'
    $status = Run-Digest @('ledger', 'status', '-Out', $legacy)
    $done = Unit $legacy 'digest/g001@1'
    Assert ($done.state -eq 'done' -and $done.result.kind -eq 'group_adoption' -and $done.result.disposition -eq 'adopted') 'the verified group is imported as an adoption, not as a historic completion'
    Assert ($done.result.verification.mode -eq 'read_only' -and $done.result.verification.tool -eq $plan.tool -and $done.result.verification.archive_revision -ne $null) 'the adopted unit carries a dated, versioned read-only verification'
    Assert ($done.result.evidence.owned_source -eq 'owned_log' -and $done.result.card_fingerprints.ContainsKey($anchor1)) 'the evidence names the owned log and every verified Card fingerprint'
    Assert ((Unit $legacy 'digest/g002@1').state -eq 'ready' -and (Unit $legacy 'digest/g003@1').state -eq 'done') 'the incomplete group is open work while the reconstructed group is adopted'
    Assert ((Unit $legacy 'terms/book@1').state -eq 'done' -and (Unit $legacy 'terms/book@1').result.kind -eq 'terms_adoption') 'a retained term table with its Card adopts the Terms unit'
    $adoptedModel = Unit $legacy $verifiedModels[0].unit
    Assert ($adoptedModel.state -eq 'done' -and $adoptedModel.result.kind -eq 'model_adoption' -and $adoptedModel.result.parts -eq 2) 'a verified built model stays built instead of being rebuilt'
    Assert (@($adoptedModel.result.card_fingerprints.Keys).Count -ge 3 -and $adoptedModel.result.source_review.retained -eq $false) 'the model verification records its actual Cards and claims no retained review'
    $unfinishedUnit = $unfinishedModels[0].unit
    Assert ((Unit $legacy $unfinishedUnit).state -eq 'ready' -and (Unit $legacy $unfinishedUnit).inputs.kind -eq 'model_adoption_need') 'the model with 待写 parts is a targeted obligation, not a rebuild of the others'
    $explainUnit = @($plan.classification.incomplete | Where-Object kind -EQ 'explain')[0].unit
    Assert ((Unit $legacy $explainUnit).state -eq 'ready' -and (Unit $legacy $explainUnit).inputs.model -eq $modelAddress) 'the missing explanation is a targeted unit beside the adopted model'
    $noteUnit = "notes/$($plan.notes[0].id)@1"
    Assert ((Unit $legacy $noteUnit).state -eq 'ready' -and (Unit $legacy $noteUnit).inputs.card -eq $modelAddress) 'the recovered note suggestion is a registered unit'
    $ledger = (Evidence $legacy) | ConvertFrom-Json -AsHashtable
    $adoptedEvents = @($ledger.events | Where-Object { $_.event -eq 'adopted' })
    $registeredEvents = @($ledger.events | Where-Object { $_.event -eq 'registered' })
    Assert ($adoptedEvents.Count -ge 4 -and $registeredEvents.Count -ge 2) 'the ledger records adopted and registered events'
    Assert (@($adoptedEvents | Where-Object { $_.unit_id -eq 'digest/g001@1' })[0].facts_json.Contains('owned_log')) 'the adopted event names its evidence'
    Assert (@($status.units | Where-Object { $_.layer -eq 'audit' -or $_.layer -eq 'cards-book' }).Count -ge 2) 'the book-level downstream obligations are registered as open units'

    # --- idempotence: the same proposal re-applies without changing a row ---------------------
    $evidence = Evidence $legacy
    $reapply = Run-Digest @('ledger', 'adopt', '-Out', $legacy, '-Variant', 'econ', '-Apply')
    Assert ($reapply.status -eq 'already_applied' -and (Evidence $legacy) -ceq $evidence) 'a repeated apply of the same proposal changes no ledger row'
    Refused @('ledger', 'adopt', '-Out', $legacy, '-Variant', 'math', '-Apply') 'adoption_variant_changed' 'an adopted folder is never re-adopted under another variant'
    $again = Run-Digest @('ledger', 'adopt', '-Out', $legacy, '-Variant', 'econ')
    Assert ($again.status -eq 'already_applied' -and $again.kind -eq 'adoption_plan' -and $again.adoption.plan.proposal_hash -eq $plan.proposal_hash) 'a read-only adoption of an adopted folder reports its recorded adoption'

    # --- T17: one resume drains the unresolved closure and keeps the adopted build ------------
    $before = Unit $legacy 'digest/g001@1'
    $modelBefore = Unit $legacy $verifiedModels[0].unit
    $resumed = Run-Digest @('ledger', 'resume', '-Out', $legacy)
    Assert ($resumed.status -eq 'resumed' -and $resumed.invocation -eq 2 -and $resumed.resume_count -eq 1) 'the adopted run resumes with one invocation'
    $after = Unit $legacy 'digest/g001@1'
    $modelAfter = Unit $legacy $verifiedModels[0].unit
    Assert ($after.state -eq 'done' -and $after.completed_at -ceq $before.completed_at -and $after.result_json -ceq $before.result_json) 'an adopted group keeps its exact result and completion time across the resume'
    Assert ($modelAfter.state -eq 'done' -and $modelAfter.result_json -ceq $modelBefore.result_json) 'the adopted model keeps its verification across the resume'
    Assert (@($resumed.reset).Count -eq 0 -and (Unit $legacy 'digest/g002@1').state -eq 'ready' -and (Unit $legacy 'digest/g002@1').attempts -eq 0) 'the resume keeps the adopted units terminal and leaves the unfinished group ready without a spent attempt'
    # The adopted model's explanation unit accepts its read-only producer record.
    $claim = (Run-Digest @('ledger', 'next', '-Out', $legacy, '-Layer', 'explain', '-Take', '1', '-Owner', 'explain-manager')).units[0]
    Assert ($claim.id -eq $explainUnit) 'the explanation obligation is claimable beside the adopted model'
    Refused @('explain-finish', '-Out', $legacy, '-Model', $modelAddress, '-Token', "$($claim.token)", '-CheckOnly') 'explanation_missing' 'the explanation unit accepts the adopted model as its checked producer and names the missing sections'
    $report = Run-Digest @('ledger', 'report', '-Out', $legacy)
    Assert ($report.identity.citation_key -eq $key -and $report.resume.available -eq $false) 'the run report reads the adopted identity and the spent resume'
    Refused @('ledger', 'resume', '-Out', $legacy) 'resume_used' 'the one supported resume is spent exactly once'

    # --- T21: identity is verified before any write -------------------------------------------
    $evidence = Evidence $legacy
    $bookBytes = [IO.File]::ReadAllBytes((Join-Path $legacy 'book.zh.md'))
    [IO.File]::AppendAllText((Join-Path $legacy 'book.zh.md'), "`n已改写。", $utf8)
    Refused @('ledger', 'report', '-Out', $legacy) 'source hash mismatch' 'a changed frozen source is refused before the report writes'
    Refused @('ledger', 'next', '-Out', $legacy, '-Layer', 'digest', '-Take', '1', '-Owner', 'manager') 'source hash mismatch' 'a changed frozen source is refused before a claim'
    [IO.File]::WriteAllBytes((Join-Path $legacy 'book.zh.md'), $bookBytes)
    Assert ((Evidence $legacy) -ceq $evidence) 'the refused command left every ledger row byte-identical'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
