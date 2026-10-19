#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\register-paging.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# S10 / issue #101: a register larger than one `get_register` page is one catalog. `-Step apply`
# publishes it and verifies every page at one revision; `-Step check` diffs every page; a second
# apply writes nothing. A register that declares 词条分册 parts is refused before any write,
# because publishing parts is S11 (#104). A real scratch archive; the live archive is not touched.
param([string]$Zt = '', [string]$Digest = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$scratch = Join-Path ([IO.Path]::GetTempPath()) "register-paging-test-$PID"
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
function Fails([string]$step) {
    $lines = @(pwsh -NoProfile -File $wrapper -Step $step -Work $work 2>&1)
    [pscustomobject]@{ code = $LASTEXITCODE; text = ($lines -join "`n") }
}
function Last($lines) { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable -DateKind String }
function Titles { @(Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{ workspace = 'agent' } }))[0].titles }
function Card([string]$at) { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] }
function Terms($rows) {
    $map = [ordered]@{}
    foreach ($row in @($rows)) {
        $map[$row.term] = @($row.targets | ForEach-Object { if ($_ -is [string]) { $_ } elseif ($_.exists) { $_.address } })
    }
    $map
}
function Same([string[]]$a, [string[]]$b) { @(Compare-Object @($a) @($b)).Count -eq 0 }
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    # One agent Card serves every term: a register may name the same Card under more terms.
    $null = Invoke-Zt @(
        @{ tool = 'create_topic'; arguments = @{ title = '分页的登记簿'; body = '一页装不下的登记簿怎样读全？' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '分页的登记簿要有全部条目'; body = '一次读全所有页，才谈得上完整。' } }
    )
    $target = (Titles | Where-Object { $_.title -eq '分页的登记簿要有全部条目' } | Select-Object -First 1).address
    # The staged plan is the accepted scope: its terms, its body, and its own plan id.
    $facts = @(Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @(); view = 'text' } }))[0]
    $count = 1005
    $terms = [System.Collections.Generic.List[object]]::new()
    $rows = [System.Collections.Generic.List[string]]::new()
    for ($i = 1; $i -le $count; $i++) {
        $term = ('术语{0:D4}' -f $i)
        $terms.Add([ordered]@{ term = $term; targets = @($target); sources = @(); ambiguous = $false; overflow = 0; reviewed = @(); read_only_targets = @() })
        $rows.Add("- $term：[[$target]]")
    }
    $body = "范围：一页装不下的登记簿。`n`n## 词条`n" + ($rows -join "`n")
    $plan = [ordered]@{
        schema_version = 1
        archive        = [ordered]@{ identity = $facts.archive_identity; revision = $facts.archive_revision }
        register       = [ordered]@{ address = $null; missing = $true; entries = 0; complete = $false }
        terms          = $terms
        unresolved     = @()
        faults         = @()
        excluded       = @()
        review         = $null
        body           = $body
        plan_id        = 'rp-paging-test'
        created_at     = '2026-10-19T00:00:00.0000000Z'
    }
    $planned = Join-Path $work 'out\register-plan.json'
    [IO.File]::WriteAllText($planned, ($plan | ConvertTo-Json -Depth 10), $utf8)

    # apply: the published register is verified through every page, not only the first.
    $applied = Last (Run 'apply')
    Assert ($applied.register -and $applied.created -eq 2) 'apply: creates the Topic 索引 and its register Hub Card'
    Assert ($applied.verified -eq $true -and $applied.complete -eq $true) 'apply: verifies the exact register through every page'
    Assert ($applied.entries -eq $count) "apply: reports all $count published terms ($($applied.entries))"
    $first = @(Invoke-Zt @(@{ tool = 'get_register'; arguments = @{ workspace = 'agent' } }))[0]
    Assert ($first.entry_total -eq $count -and @($first.entries).Count -eq 200 -and $first.entry_next_offset -eq 200) 'get_register: the first page holds 200 rows and names the next page'
    $second = @(Invoke-Zt @(@{ tool = 'get_register'; arguments = @{ workspace = 'agent'; entry_offset = 1000 } }))[0]
    Assert (@($second.entries).Count -eq 5 -and $null -eq $second.entry_next_offset) 'get_register: the last page holds the remaining rows and ends the catalog'
    Assert ($second.archive_revision -eq $first.archive_revision -and $second.entry_total -eq $first.entry_total) 'get_register: every page declares one revision and one total'
    Assert ($first.entries[0].term -eq '术语0001' -and $second.entries[0].term -eq '术语1001') 'get_register: the pages continue one catalog in order'

    # check: every published term of every page matches the staged plan.
    $check = Last (Run 'check')
    Assert (@($check.missing).Count -eq 0 -and @($check.diverged).Count -eq 0 -and @($check.extra).Count -eq 0) 'check: no term of any page is missing, diverged, or extra'
    Assert ($check.entries -eq $count -and $check.complete -eq $true) "check: reads all $count published terms ($($check.entries))"

    # a second apply writes nothing and keeps the plan id.
    $again = Last (Run 'apply')
    Assert ($again.created -eq 0 -and $again.changed -eq $false -and $again.entries -eq $count) 'apply again: no new Card, no changed body, every term still verified'
    Assert ($again.plan_id -eq $applied.plan_id) 'apply again: the same plan id'

    # A register that declares 词条分册 parts is S11's (#104): refused before any write, not
    # published and then reported as a failure.
    $register = $applied.register
    $null = Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{
                at    = $register
                kind  = 'side'
                title = '词条分册：分页'
                body  = "- 分册词：[[$target]]`n"
            } })
    $part = (Titles | Where-Object { $_.title -eq '词条分册：分页' } | Select-Object -First 1).address
    $card = Card $register
    $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{
                at   = $register
                body = "$($card.body)`n`n## 词条分册`n- [[$part]] 分页`n"
            } })
    $before = @(Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @($register); view = 'text' } }))[0]
    $refusedPlan = Fails 'plan'
    Assert ($refusedPlan.code -ne 0 -and $refusedPlan.text -match 'S11') 'plan: a register with declared parts is refused, and the refusal names S11 (#104)'
    $kept = Get-Content -Raw -LiteralPath $planned | ConvertFrom-Json -AsHashtable -DateKind String
    Assert ($kept.plan_id -eq 'rp-paging-test') 'plan: a refused plan does not overwrite the accepted plan file'
    $refusedApply = Fails 'apply'
    Assert ($refusedApply.code -ne 0 -and $refusedApply.text -match 'S11') 'apply: the same register is refused before the write'
    $after = @(Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @($register); view = 'text' } }))[0]
    Assert ($after.cards[0].text -eq $before.cards[0].text) 'apply: the refused publication left the register body byte-identical'
}
finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
