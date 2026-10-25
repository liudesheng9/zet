#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\release-set.tests.ps1 [-Zt <zt.exe>]
# S28 (#170, S28-AC01/AC02/AC04/AC05/AC06): the candidate release set is one compatible set.
# This static test reads the manifest, the crates, the source blueprints, the installed frozen
# registry and the shipped guidance (skills, both READMEs, CONTEXT.md), and fails when one place
# names a different version, when an installed script is not the build its lock records, when a
# new workflow version reused or dropped frozen history, or when the Term guidance is missing.
# It starts no service and writes no Card; scripts\verify-release.ps1 adds the installed-and-running
# half (binaries, installed skills, the live daemon's metadata, a scratch gate check, the wiki).
param([string]$Zt = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$failed = 0
$checks = 0
function Assert($condition, $message) {
    $script:checks++
    if (-not $condition) { $script:failed++; Write-Output "FAIL: $message" } else { Write-Output "ok: $message" }
}
function Read-Json([string]$path) {
    if (-not (Test-Path -LiteralPath $path)) { throw "missing file: $path" }
    Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String
}
function Get-Sha256([string]$path) {
    if (-not (Test-Path -LiteralPath $path)) { return '' }
    (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Get-Text([string]$relative) {
    $path = Join-Path $repo $relative
    if (-not (Test-Path -LiteralPath $path)) { throw "missing file: $path" }
    ([IO.File]::ReadAllText($path)) -replace "`r`n", "`n"
}
# The first `version = "..."` after `[package]`; the dependency tables below also carry version =.
function Get-CrateVersion([string]$relative) {
    $package = $false
    foreach ($line in ((Get-Text $relative) -split "`n")) {
        $line = $line.Trim()
        if ($line -eq '[package]') { $package = $true; continue }
        if ($package -and $line.StartsWith('[')) { break }
        if ($package -and $line -match '^version = "([^"]+)"$') { return $Matches[1] }
    }
    throw "no [package] version in $relative"
}
function Get-LockVersion([string]$relative, [string]$package) {
    $lines = (Get-Text $relative) -split "`n"
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i].Trim() -ne "name = `"$package`"") { continue }
        for ($j = $i + 1; $j -lt $lines.Count -and $j -lt $i + 4; $j++) {
            if ($lines[$j].Trim() -match '^version = "([^"]+)"$') { return $Matches[1] }
        }
    }
    return ''
}
function Get-Semver([string]$value) {
    if ($value -notmatch '^(\d+)\.(\d+)\.(\d+)$') { throw "not a semantic version: $value" }
    [pscustomobject]@{ major = [int]$Matches[1]; minor = [int]$Matches[2]; patch = [int]$Matches[3]; text = $value }
}
function Compare-Semver($left, $right) {
    foreach ($part in @('major', 'minor', 'patch')) {
        if ($left.$part -ne $right.$part) { return $left.$part.CompareTo($right.$part) }
    }
    return 0
}

$manifestPath = Join-Path $repo 'docs\term-cards-links-and-check-gate-release-manifest.json'
$manifest = Read-Json $manifestPath
Assert ($manifest.schema_version -eq 1 -and $manifest.issue -eq 170 -and $manifest.slice -eq 'S28') 'the manifest names this issue and slice'

# --- the four contracts, against the code that carries them -----------------------------------
$executable = $manifest.contracts.executable
Assert ((Get-CrateVersion 'Cargo.toml') -eq $executable.zt) "Cargo.toml carries zt $($executable.zt)"
Assert ((Get-CrateVersion 'tools\zt-check\Cargo.toml') -eq $executable.zt_check) "zt-check Cargo.toml carries $($executable.zt_check)"
Assert ((Get-CrateVersion 'tools\zt-digest\Cargo.toml') -eq $executable.zt_digest) "zt-digest Cargo.toml carries $($executable.zt_digest)"
Assert ((Get-LockVersion 'Cargo.lock' 'zt-check') -eq $executable.zt_check) "Cargo.lock resolves zt-check $($executable.zt_check)"
Assert ((Get-LockVersion 'tools\zt-digest\Cargo.lock' 'zt-check') -eq $executable.zt_check) "zt-digest's lock resolves the same zt-check $($executable.zt_check)"

$rule = $manifest.contracts.rule
$contract = Get-Text 'tools\zt-check\src\contract.rs'
foreach ($pair in @(@{ key = 'RULE_VERSION'; value = $rule.rule_version }, @{ key = 'DIAGNOSTIC_SCHEMA'; value = $rule.diagnostic_schema }, @{ key = 'APPLICABILITY_VERSION'; value = $rule.applicability_version })) {
    Assert ($contract -match "pub const $($pair.key): &str = `"$([regex]::Escape($pair.value))`";") "contract.rs publishes $($pair.key) $($pair.value)"
}
$schema = $manifest.contracts.archive_schema
$treeLock = Get-Text 'src\tree_lock.rs'
Assert ($treeLock -match "pub\(crate\) const SCHEMA_VERSION: &str = `"$([regex]::Escape($schema.generation))`";") "tree_lock.rs installs generation $($schema.generation)"
Assert ($schema.card_format -eq 'markdown' -and $schema.term_field -eq 'term_name') 'the manifest names the markdown format and the Term field'
Assert ($schema.supported_read_generations -join ',' -eq '1,2' -and $schema.older_binary_refuses -eq '2') 'the compatibility floor is generation 2 and generations 1-2 are read'

$policy = $manifest.policy
Assert ($policy.default_stage -eq 'advisory') 'the release ships the advisory gate stage'
Assert ($policy.mandatory_invariants_active -eq $true) 'the mandatory invariants stay active'
Assert ($policy.promotion -eq 'owner') 'only the owner promotes the stage'
$gate = Get-Text 'src\gate.rs'
Assert ($gate -match "an absent row is the rollout default, advisory") 'gate.rs names advisory as the absent-row default'
Assert ($gate -match '(?s)pub\(crate\) fn current_stage\(conn: &Connection\) -> Result<Stage> \{.*?Ok\(Stage::Advisory\)') 'current_stage reads an absent row as Stage::Advisory'

# --- the installed binary publishes the same manifest -----------------------------------------
if (-not $Zt) { $Zt = (Get-Command zt -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $Zt) { throw 'no zt binary: pass -Zt <path> or put zt on PATH' }
$published = (& $Zt version --json 2>&1 | Out-String).Trim()
Assert ($LASTEXITCODE -eq 0 -and $published.StartsWith('{')) "the installed zt publishes its version contract ($Zt)"
if ($published.StartsWith('{')) {
    $version = $published | ConvertFrom-Json -DateKind String
    Assert ($version.zt -eq $executable.zt) "the installed zt is $($executable.zt) ($($version.zt))"
    Assert ($version.checker -eq $executable.zt_check) "it carries checker $($executable.zt_check) ($($version.checker))"
    Assert ($version.rule_version -eq $rule.rule_version -and $version.diagnostic_schema -eq $rule.diagnostic_schema -and $version.applicability_version -eq $rule.applicability_version) 'it publishes the same rule contract'
    Assert ($version.schema -eq $schema.generation) "it publishes archive schema $($schema.generation)"
    Assert ($version.capabilities.term_links -eq $manifest.capabilities.term_links -and $version.capabilities.term_cards -eq $manifest.capabilities.term_cards) 'it publishes the Term capabilities'
}
Assert ((& $Zt help 2>&1 | Out-String) -match 'zt version --json') 'zt help names zt version --json'
foreach ($tool in @(@{ name = 'zt-check'; command = '--version'; version = $executable.zt_check }, @{ name = 'zt-digest'; command = 'version'; version = $executable.zt_digest })) {
    $exe = (Get-Command $tool.name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    Assert ([bool]$exe) "$($tool.name) is installed beside zt"
    if ($exe) {
        $printed = (& $exe $tool.command 2>&1 | Out-String).Trim()
        Assert ($printed -match "^$([regex]::Escape($tool.name)) $([regex]::Escape($tool.version))$") "$($tool.name) is $($tool.version) ($printed)"
    }
}

# --- both frozen variants are the manifest builds, assembled from the current sources ----------
$workflow = $manifest.contracts.workflow
$baseSource = Read-Json (Join-Path $repo 'scripts\book-digest\workflow\zt-digest-base.blueprint.json')
Assert ($baseSource.kind -eq 'base' -and $baseSource.version -eq $workflow.base) "the base source is $($workflow.base)"
$retained = $manifest.retained_workflows
foreach ($name in @('zt-econ-digest', 'zt-math-digest')) {
    $expected = $workflow.$name
    $source = Read-Json (Join-Path $repo "scripts\book-digest\workflow\$name.blueprint.json")
    Assert ($source.version -eq $expected -and $source.base -eq './zt-digest-base.blueprint.json') "the $name source is $expected on the release base"
    $registry = Join-Path $repo ".claude\workflow-framework\$name"
    $lock = Read-Json (Join-Path $registry 'lock.json')
    Assert ($lock.name -eq $name -and $lock.version -eq $expected -and $lock.protocol -eq 'workflow-framework-protocol/1') "the $name lock is $expected"
    $installed = Join-Path $repo (($lock.script -replace '/', '\'))
    Assert ((Get-Sha256 $installed) -eq $lock.sha256) "the installed $name script is the build its lock records"
    Assert ((Get-Text (($lock.script -replace '/', '\'))) -match "(?m)^\s*// $name v$([regex]::Escape($expected)): built by the workflow-framework-protocol") "the installed $name script names its release version"
    $composed = Read-Json (Join-Path $registry 'blueprint.json')
    Assert ($composed.version -eq $expected -and $composed.composedFrom.base -eq 'zt-digest-base' -and $composed.composedFrom.version -eq $workflow.base) "$name was composed from the $($workflow.base) base"
    foreach ($entry in @($lock.sources)) {
        Assert ((Get-Sha256 (Join-Path $repo ($entry.file -replace '/', '\'))) -eq $entry.sha256) "the lock records the current source $($entry.file)"
    }
    $old = Get-Semver $retained.$name
    $new = Get-Semver $expected
    Assert ((Compare-Semver $new $old) -gt 0) "$name rose from $($old.text) to $($new.text)"
    foreach ($version in @($retained.$name, $expected)) {
        $historyScript = Join-Path $registry "history\$version\$name.js"
        Assert (Test-Path -LiteralPath $historyScript) "$name keeps its frozen $version build in history"
    }
    Assert ((Get-Sha256 (Join-Path $registry "history\$expected\$name.js")) -eq $lock.sha256) "$name's history entry under $expected is the installed build"
}

# --- the guidance ships with the set ----------------------------------------------------------
$shipped = @(Get-ChildItem -LiteralPath (Join-Path $repo 'skills') -Directory | Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'SKILL.md') } | ForEach-Object Name | Sort-Object)
Assert ((@($manifest.skills | Sort-Object) -join ',') -eq ($shipped -join ',')) "the manifest names every installable skill ($($shipped -join ', '))"

$expectations = @(
    @{ file = 'skills\zt\SKILL.md'; phrases = @('Term Cards', '`get_terms`', '`[[label@address]]`', 'term_links: "label"', '`card_check`', 'advice', 'Relation kind') },
    @{ file = 'skills\zettelkasten\SKILL.md'; phrases = @('Term Cards and Term uses', '`type: term`', '`[[label@address]]`', '自动链接：否', 'raw text', 'Relation kind is `term`') },
    @{ file = 'skills\zt-digest-protocol\SKILL.md'; phrases = @('term-cards', 'term-link', 'T1–T10', '`card_check`', '`card_locked`', 'termCards', 'termLink', 'termLinkOccurrences') },
    @{ file = 'skills\zt-digest-protocol\references\scripts.md'; phrases = @('zt-digest term-cards', 'zt-digest term-link', '--draft', 'T1–T10', '`terms` section') },
    @{ file = 'README.md'; phrases = @('Term Cards, Term lists and Term uses', 'Term use', 'Relation kind', 'zt config policy', 'gate.stage', 'term-cards', 'term-link', 'zt version --json', 'release-manifest.json', 'Card check and policy') },
    @{ file = 'README.zh-CN.md'; phrases = @('Term Card', '术语表卡', 'Term use', 'zt config policy', 'gate.stage', 'term-cards', 'term-link', 'zt version --json') }
)
foreach ($case in $expectations) {
    $text = Get-Text $case.file
    foreach ($phrase in $case.phrases) { Assert ($text.Contains($phrase)) "$($case.file) states `"$phrase`"" }
}
$glossary = Get-Text 'CONTEXT.md'
foreach ($entry in @('**Card type**', '**Relation kind**', '**Book term table**', '**Term Card type**', '**Term list Card**', '**Term Card**', '**Term part**', '**Term name**', '**Term link**', '**Term use**', '**Term edge**', '**Term rule**', '**Card-check policy**', '**Write gate**', '**Gate stage**')) {
    Assert ($glossary.Contains($entry)) "CONTEXT.md defines $entry"
}

Write-Output "failed: $failed"
if ($failed) { exit 1 }
Write-Output "release set: ok ($checks checks)"
