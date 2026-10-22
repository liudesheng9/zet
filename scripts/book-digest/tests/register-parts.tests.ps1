#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\register-parts.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# #93 S11 / issue #104: a register over 120 term rows publishes its rows into ordinary Idea Cards
# declared under 词条分册. The parts carry the accepted pinyin-initial ranges, a term with no
# accepted reading stays in 其他, the parts are published and verified before the root, and a
# retried publication reuses the receipts of the Cards it already created instead of duplicating
# them. A real scratch archive; the live archive is not touched.
param([string]$Zt = '', [string]$Digest = '', [string]$Check = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$scratch = Join-Path ([IO.Path]::GetTempPath()) "register-parts-test-$PID"
$archive = Join-Path $scratch 'archive'
$work = Join-Path $scratch 'work'
New-Item -ItemType Directory -Force $archive, $work, (Join-Path $work 'out') | Out-Null
function Newest($stamp, [string[]]$candidates) {
    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath $candidate) -and (Get-Item -LiteralPath $candidate).LastWriteTimeUtc -gt $stamp) { return $candidate }
    }
    ''
}
if (-not $Zt) {
    $installed = (Get-Command zt -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    $Zt = Newest (Get-Item (Join-Path $repo 'src\entry.rs')).LastWriteTimeUtc @(
        (Join-Path $repo 'target\debug\zt.exe'), (Join-Path $repo 'target\release\zt.exe'), $installed)
}
if (-not $Zt) { $Zt = (Get-Command zt -ErrorAction Stop).Source }
if (-not $Digest) { $Digest = $env:ZT_DIGEST }
if (-not $Digest) {
    $installedDigest = (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    $Digest = Newest (Get-Item (Join-Path $repo 'tools\zt-digest\src\register.rs')).LastWriteTimeUtc @(
        (Join-Path $repo 'tools\zt-digest\target\debug\zt-digest.exe'), 'D:\zt-digest-target\release\zt-digest.exe', $installedDigest)
}
if (-not $Digest) { $Digest = (Get-Command zt-digest -ErrorAction Stop).Source }
$bin = Join-Path $scratch 'bin'
New-Item -ItemType Directory -Force $bin | Out-Null
Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
# zt-check classifies a declared part as a navigation Card, so the check needs this build too.
if (-not $Check) {
    $installedCheck = (Get-Command zt-check -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    $Check = Newest (Get-Item (Join-Path $repo 'tools\zt-check\src\graph.rs')).LastWriteTimeUtc @(
        (Join-Path $repo 'tools\zt-check\target\debug\zt-check.exe'), (Join-Path $repo 'tools\zt-check\target\release\zt-check.exe'), $installedCheck)
}
if (-not $Check) { $Check = (Get-Command zt-check -ErrorAction Stop).Source }
Copy-Item -LiteralPath $Check -Destination (Join-Path $bin 'zt-check.exe')
$env:PATH = "$bin;$env:PATH"
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$wrapper = Join-Path $PSScriptRoot '..\register-compiler.ps1' | Resolve-Path | ForEach-Object Path
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Run([string]$step, [string[]]$more = @()) {
    $lines = @(pwsh -NoProfile -File $wrapper -Step $step -Work $work @more 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "$step failed: $($lines -join "`n")" }
    $lines
}
function Fails([string]$step, [string[]]$more = @()) {
    $lines = @(pwsh -NoProfile -File $wrapper -Step $step -Work $work @more 2>&1)
    [pscustomobject]@{ code = $LASTEXITCODE; text = ($lines -join "`n") }
}
function Last($lines) { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable -DateKind String }
function Register { @(Invoke-Zt @(@{ tool = 'get_register'; arguments = @{ workspace = 'agent' } }))[0] }
function Card([string]$at) { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] }
function Edit([string]$at, [string]$body) { $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = $at; body = $body } }) }
function Titles { @(Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{ workspace = 'agent' } }))[0].titles }
function Terms($rows) {
    $map = [ordered]@{}
    foreach ($row in @($rows)) {
        $map[$row.term] = @($row.targets | ForEach-Object { if ($_ -is [string]) { $_ } elseif ($_.exists) { $_.address } })
    }
    $map
}
# The accepted plan a `plan` step would stage for a split register: the parts in band order, each
# with its exact body and its call, and the root body naming each part by {{part:N}} until apply
# knows the address its receipt returned.
function NewSplitPlan([object[]]$parts, [string]$target, $facts, [string]$planId) {
    $shell = '范围：代理工作区的入口。每个词条指向一到三个实质卡。'
    $calls = [Collections.Generic.List[object]]::new()
    $calls.Add([ordered]@{ tool = 'create_topic'; arguments = [ordered]@{ title = '索引'; body = '本主题收放代理工作区的入口：索引卡与领域枢纽。' } })
    $calls.Add([ordered]@{ tool = 'create_successor'; arguments = [ordered]@{ at = '@@0'; type = 'hub'; level = 'register'; title = '索引：代理工作区的入口'; body = $shell } })
    $terms = [Collections.Generic.List[object]]::new()
    $planned = [Collections.Generic.List[object]]::new()
    $declarations = [Collections.Generic.List[string]]::new()
    for ($i = 0; $i -lt $parts.Count; $i++) {
        $part = $parts[$i]
        $rows = @($part.terms | ForEach-Object { "- $_：[[$target]]" })
        $body = "## 词条`n" + ($rows -join "`n")
        $calls.Add([ordered]@{ tool = 'create_successor'; arguments = [ordered]@{ at = '@@1'; kind = 'side'; title = "词条 $($part.label)"; body = $body } })
        $planned.Add([ordered]@{
                label = $part.label; bucket = $part.bucket; title = "词条 $($part.label)"
                address = $null; body = $body; terms = @($part.terms); call = ($calls.Count - 1); reused = $false
            })
        $declarations.Add("- [[{{part:$i}}]] $($part.label)")
        foreach ($term in $part.terms) {
            $terms.Add([ordered]@{ term = $term; targets = @($target); sources = @(); ambiguous = $false; overflow = 0; reviewed = @(); read_only_targets = @() })
        }
    }
    $rootBody = $shell + "`n`n## 词条分册`n`n" + ($declarations -join "`n")
    $accepted = @($parts | Where-Object { $_.bucket -ne 'other' } | ForEach-Object { @($_.terms) } | ForEach-Object { [ordered]@{ term = $_; reading = 'accepted'; initial = 'a' } })
    $unresolved = @($parts | Where-Object { $_.bucket -eq 'other' } | ForEach-Object { @($_.terms) } | ForEach-Object { [ordered]@{ term = $_; reading = ''; reason = 'missing' } })
    [ordered]@{
        schema_version  = 1
        archive         = [ordered]@{ identity = $facts.archive_identity; revision = $facts.archive_revision }
        register        = [ordered]@{ address = $null; missing = $true; entries = 0; complete = $false; parts = 0 }
        terms           = $terms
        unresolved      = @()
        faults          = @()
        excluded        = @()
        review          = $null
        readings        = [ordered]@{ accepted = $accepted; unresolved = $unresolved; count = $accepted.Count }
        parts           = $planned
        obsolete_parts  = @()
        terms_on_root   = $false
        root_before     = $shell
        body            = $rootBody
        calls           = $calls
        root_call       = [ordered]@{ tool = 'edit_card'; arguments = [ordered]@{ at = $null; body = $rootBody } }
        plan_id         = $planId
        created_at      = '2026-10-19T00:00:00.0000000Z'
    }
}
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    # One agent Card serves every term of the fixture.
    $null = Invoke-Zt @(
        @{ tool = 'create_topic'; arguments = @{ title = '分册的登记簿'; body = '分册的登记簿怎样读全？' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '分册的登记簿要有全部词条'; body = '每个词条都指向承载它的卡。' } }
    )
    $target = (Titles | Where-Object { $_.title -eq '分册的登记簿要有全部词条' } | Select-Object -First 1).address
    $facts = @(Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @(); view = 'text' } }))[0]
    # 121 terms of one Latin initial split at 120, two digit-initial terms, and five with no
    # accepted reading: four parts, 128 rows.
    $latin = @(1..121 | ForEach-Object { '词条A{0:D3}' -f $_ })
    $parts = @(
        [ordered]@{ label = 'A–F'; bucket = 'latin'; terms = @($latin[0..119]) },
        [ordered]@{ label = 'A–F(2)'; bucket = 'latin'; terms = @($latin[120]) },
        [ordered]@{ label = '0–9'; bucket = 'digit'; terms = @('词条9一', '词条9二') },
        [ordered]@{ label = '其他'; bucket = 'other'; terms = @('词条无读一', '词条无读二', '词条无读三', '词条无读四', '词条无读五') }
    )
    $count = 128
    $plan = NewSplitPlan $parts $target $facts 'rp-parts-test'
    $planned = Join-Path $work 'out\register-plan.json'
    [IO.File]::WriteAllText($planned, ($plan | ConvertTo-Json -Depth 12), $utf8)

    # apply: the parts are published first, then the root last, and every part is verified.
    $applied = Last (Run 'apply')
    Assert ($applied.register -and $applied.created -eq 2 -and $applied.publication -eq 'complete') 'apply: creates the Topic 索引, its register shell, and marks the publication complete'
    Assert ($applied.verified -eq $true -and $applied.complete -eq $true -and $applied.entries -eq $count) "apply: verifies all $count published terms"
    Assert (@($applied.parts).Count -eq 4) 'apply: reports the four published parts'
    $view = Register
    Assert ($view.register -eq $applied.register -and $view.complete -eq $true) 'get_register: the published register is complete'
    Assert ($view.entry_total -eq $count) "get_register: all $count rows are one catalog"
    $labels = @($view.parts | ForEach-Object { $_.label })
    Assert (($labels -join ',') -eq 'A–F,A–F(2),0–9,其他') "get_register: the declared parts carry the accepted ranges in order ($($labels -join ','))"
    Assert (@($view.parts | Where-Object { $_.place -ne 'ok' }).Count -eq 0) 'get_register: every part is an immediate Side successor of the register'
    $rows = @($view.parts | ForEach-Object { $_.entries })
    Assert (($rows -join ',') -eq '120,1,2,5') "get_register: a bucket over 120 rows splits into contiguous parts ($($rows -join ','))"
    Assert (@($view.duplicates).Count -eq 0 -and @($view.faults).Count -eq 0) 'get_register: the publication leaves no duplicate row and no fault'
    # The parts are ordinary Idea Cards holding the register's own row grammar, and the root
    # declares them by label.
    $partCard = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $view.parts[0].address } }))[0]
    Assert ($partCard.type -eq 'idea' -and $partCard.body -match '(?m)^## 词条$' -and $partCard.body -match [regex]::Escape("- 词条A001：[[$target]]")) 'a part is an ordinary Idea Card with 词条 rows'
    # The checker derives the register-part navigation category from the root declaration
    # (§8.5, D188): the part raises none of the substantive Idea faults (claim title, size,
    # source line), and its term rows are still read by the register's own entry check.
    $linted = @(& zt-check --addresses (@($view.parts | ForEach-Object { $_.address }) -join ',') 2>&1)
    Assert ($linted[-1] -eq 'faults: 0') "zt-check: the declared parts are navigation Cards, not Ideas ($($linted -join ' | '))"
    # The root declares them by label and keeps no term row of its own.
    $root = Card $applied.register
    Assert ($root.body -match '(?m)^## 词条分册$' -and $root.body -match [regex]::Escape("- [[$($view.parts[0].address)]] A–F") -and $root.body -notmatch '(?m)^## 词条$') 'the root declares its parts and keeps no term row of its own'
    # A term of a part names its target as a member of the register.
    $member = Card $target
    Assert (@($member.hubs | Where-Object { $_.address -eq $applied.register -and $_.term -eq '词条A121' }).Count -eq 1) 'a part row names its target as a register member'

    # check: the published layout, entries, labels and readings are the accepted plan.
    $checkLines = Run 'check'
    $checkOut = Last $checkLines
    Assert (@($checkOut.missing).Count -eq 0 -and @($checkOut.diverged).Count -eq 0 -and @($checkOut.extra).Count -eq 0) 'check: no term of any part is missing, diverged, or extra'
    Assert (@($checkOut.parts).Count -eq 4 -and @($checkOut.plan_parts).Count -eq 0) 'check: the four declared parts are the accepted layout'
    Assert ($checkOut.readings.accepted.Count -eq 123 -and $checkOut.readings.unresolved.Count -eq 5) 'check: the accepted readings and the 其他 bucket are reported'

    # a second apply over the same plan writes nothing.
    $titlesBefore = (Titles).Count
    $again = Last (Run 'apply')
    Assert ($again.wrote -eq 0 -and $again.changed -eq $false -and $again.publication -eq 'complete') 'apply again: no write, and the publication is still verified'
    Assert ((Titles).Count -eq $titlesBefore) 'apply again: no new Card'

    # An interrupted publication: the parts batch was committed, the root call was not. Retrying
    # the same accepted plan replays the created parts instead of creating them again (D189,
    # AC07): no Card is added and the same addresses come back.
    zt down 2>&1 | Out-Null
    $second = Join-Path $scratch 'archive2'
    New-Item -ItemType Directory -Force $second | Out-Null
    zt config set archive_root $second | Out-Null
    zt up | Out-Null
    $null = Invoke-Zt @(
        @{ tool = 'create_topic'; arguments = @{ title = '分册的登记簿'; body = '分册的登记簿怎样读全？' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '分册的登记簿要有全部词条'; body = '每个词条都指向承载它的卡。' } }
    )
    $facts = @(Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @(); view = 'text' } }))[0]
    $plan = NewSplitPlan $parts $target $facts 'rp-parts-test'
    [IO.File]::WriteAllText($planned, ($plan | ConvertTo-Json -Depth 12), $utf8)
    $partsFile = Join-Path $work 'out\parts-calls.json'
    [IO.File]::WriteAllText($partsFile, ($plan.calls | ConvertTo-Json -Depth 12 -AsArray), $utf8)
    # The parts batch alone: as if the process stopped right after it.
    $lines = @(& $env:ZT_DIGEST call -Open -File $partsFile -OperationPrefix 'register/parts-test/1/parts' 2>&1)
    Assert ($LASTEXITCODE -eq 0) "the staged parts batch commits ($($lines -join ' | '))"
    $beforeRetry = (Titles)
    $half = @(Invoke-Zt @(@{ tool = 'get_register'; arguments = @{ workspace = 'agent' } }))[0]
    Assert ($half.entry_total -eq 0 -and @($half.parts | Where-Object { $_.place -eq 'ok' }).Count -eq 0) 'interrupted: the shell register declares no part and serves no row'
    Assert (@($half.parts | Where-Object { $_.place -eq 'unlisted' }).Count -eq 4 -and @($half.faults | Where-Object { $_.fault -eq 'part-unlisted' }).Count -eq 4) 'interrupted: the four parts are reported as unlisted, not accepted'
    $repaired = Last (Run 'apply')
    Assert ($repaired.publication -eq 'complete' -and $repaired.replayed -ge 5) "apply retry: the committed parts are replayed, not created again (replayed=$($repaired.replayed))"
    Assert ((Titles).Count -eq $beforeRetry.Count) 'apply retry: the archive holds no duplicated part Card'
    $same = Register
    Assert (@($same.parts).Count -eq 4 -and $same.entry_total -eq $count -and $same.complete -eq $true) 'apply retry: the register declares its parts and serves all rows'
    $later = Last (Run 'apply')
    Assert ($later.wrote -eq 0 -and $later.publication -eq 'complete') 'apply after the retry: nothing more to write'
    $applied = $repaired
    $root = Card $applied.register

    # A declared part the register cannot own in its place obstructs a new plan: the step names the
    # fault and writes nothing (D182, AC05).
    $foreign = (Titles | Where-Object { $_.title -eq '别处的卡' } | Select-Object -First 1).address
    if (-not $foreign) {
        $null = Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{ at = $target; title = '别处的卡'; body = '不属于这张登记簿。' } })
        $foreign = (Titles | Where-Object { $_.title -eq '别处的卡' } | Select-Object -First 1).address
    }
    Edit $applied.register ($root.body + "`n- [[$foreign]] 误放`n")
    $obstructed = Fails 'plan'
    Assert ($obstructed.code -ne 0 -and $obstructed.text -match 'part-place') 'plan: a declared part that is not an immediate Side successor is refused, naming the fault'
    $kept = Get-Content -Raw -LiteralPath $planned | ConvertFrom-Json -AsHashtable -DateKind String
    Assert ($kept.plan_id -eq 'rp-parts-test') 'plan: a refused plan does not overwrite the accepted plan file'
    Edit $applied.register $root.body

    # A staged plan that keeps more than 120 rows on the root is not the accepted layout: apply
    # refuses it instead of publishing a large root (D179, AC01).
    $rootOnly = Get-Content -Raw -LiteralPath $planned | ConvertFrom-Json -AsHashtable -DateKind String
    $rootOnly.terms_on_root = $true
    [IO.File]::WriteAllText($planned, ($rootOnly | ConvertTo-Json -Depth 12), $utf8)
    $refused = Fails 'apply'
    Assert ($refused.code -ne 0 -and $refused.text -match '120') 'apply: a plan keeping more than 120 rows on the root is refused'
}
finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
