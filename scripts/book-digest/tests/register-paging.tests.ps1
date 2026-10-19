#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\register-paging.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# S10 / issue #101, S11 / issue #104: a register larger than one `get_register` page is one
# catalog. Its rows live in 词条分册 parts, published and verified before the root; `-Step apply`
# verifies every page at one revision, `-Step check` diffs every page, and a second apply writes
# nothing. A real scratch archive; the live archive is not touched.
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
function Last($lines) { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable -DateKind String }
function Titles { @(Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{ workspace = 'agent' } }))[0].titles }
function Terms($rows) {
    $map = [ordered]@{}
    foreach ($row in @($rows)) {
        $map[$row.term] = @($row.targets | ForEach-Object { if ($_ -is 'string') { $_ } elseif ($_.exists) { $_.address } })
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
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '分页的登记簿要有全部词条'; body = '一次读全所有页，才谈得上完整。' } }
    )
    $target = (Titles | Where-Object { $_.title -eq '分页的登记簿要有全部词条' } | Select-Object -First 1).address
    # The accepted plan is the accepted scope: 1,005 rows in nine declared parts, its own plan id.
    $facts = @(Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @(); view = 'text' } }))[0]
    $count = 1005
    $terms = [Collections.Generic.List[object]]::new()
    $parts = [Collections.Generic.List[object]]::new()
    $calls = [Collections.Generic.List[object]]::new()
    $shell = '范围：一页装不下的登记簿。'
    $calls.Add([ordered]@{ tool = 'create_topic'; arguments = [ordered]@{ title = '索引'; body = '本主题收放代理工作区的入口：索引卡与领域枢纽。' } })
    $calls.Add([ordered]@{ tool = 'create_successor'; arguments = [ordered]@{ at = '@@0'; type = 'hub'; level = 'register'; title = '索引：代理工作区的入口'; body = $shell } })
    $declarations = [Collections.Generic.List[string]]::new()
    $part = 0
    for ($start = 1; $start -le $count; $start += 120) {
        $end = [Math]::Min($start + 119, $count)
        $names = @($start..$end | ForEach-Object { '术语{0:D4}' -f $_ })
        $body = "## 词条`n" + (($names | ForEach-Object { "- $_：[[$target]]" }) -join "`n")
        $label = if ($part -eq 0) { 'A–F' } else { 'A–F({0})' -f ($part + 1) }
        $calls.Add([ordered]@{ tool = 'create_successor'; arguments = [ordered]@{ at = '@@1'; kind = 'side'; title = "词条 $label"; body = $body } })
        $parts.Add([ordered]@{ label = $label; bucket = 'latin'; title = "词条 $label"; address = $null; body = $body; terms = $names; call = ($calls.Count - 1); reused = $false })
        $declarations.Add("- [[{{part:$part}}]] $label")
        foreach ($name in $names) {
            $terms.Add([ordered]@{ term = $name; targets = @($target); sources = @(); ambiguous = $false; overflow = 0; reviewed = @(); read_only_targets = @() })
        }
        $part++
    }
    $rootBody = $shell + "`n`n## 词条分册`n`n" + ($declarations -join "`n")
    $plan = [ordered]@{
        schema_version = 1
        archive        = [ordered]@{ identity = $facts.archive_identity; revision = $facts.archive_revision }
        register       = [ordered]@{ address = $null; missing = $true; entries = 0; complete = $false; parts = 0 }
        terms          = $terms
        unresolved     = @()
        faults         = @()
        excluded       = @()
        review         = $null
        readings       = [ordered]@{ accepted = @(); unresolved = @(); count = 0 }
        parts          = $parts
        obsolete_parts = @()
        terms_on_root  = $false
        root_before    = $shell
        body           = $rootBody
        calls          = $calls
        root_call      = [ordered]@{ tool = 'edit_card'; arguments = [ordered]@{ at = $null; body = $rootBody } }
        plan_id        = 'rp-paging-test'
        created_at     = '2026-10-19T00:00:00.0000000Z'
    }
    $planned = Join-Path $work 'out\register-plan.json'
    [IO.File]::WriteAllText($planned, ($plan | ConvertTo-Json -Depth 12), $utf8)

    # apply: the published register is verified through every page, not only the first.
    $applied = Last (Run 'apply')
    Assert ($applied.register -and $applied.created -eq 2 -and $applied.publication -eq 'complete') 'apply: creates the Topic 索引 and its register Hub Card'
    Assert ($applied.verified -eq $true -and $applied.complete -eq $true) 'apply: verifies the exact register through every page'
    Assert ($applied.entries -eq $count) "apply: reports all $count published terms ($($applied.entries))"
    Assert (@($applied.parts).Count -eq 9) "apply: reports the nine published parts ($(@($applied.parts).Count))"
    $first = @(Invoke-Zt @(@{ tool = 'get_register'; arguments = @{ workspace = 'agent' } }))[0]
    Assert ($first.entry_total -eq $count -and @($first.entries).Count -eq 200 -and $first.entry_next_offset -eq 200) 'get_register: the first page holds 200 rows and names the next page'
    $second = @(Invoke-Zt @(@{ tool = 'get_register'; arguments = @{ workspace = 'agent'; entry_offset = 1000 } }))[0]
    Assert (@($second.entries).Count -eq 5 -and $null -eq $second.entry_next_offset) 'get_register: the last page holds the remaining rows and ends the catalog'
    Assert ($second.archive_revision -eq $first.archive_revision -and $second.entry_total -eq $first.entry_total) 'get_register: every page declares one revision and one total'
    Assert ($first.entries[0].term -eq '术语0001' -and $second.entries[0].term -eq '术语1001') 'get_register: the pages continue one catalog in order'
    Assert (@($first.parts).Count -eq 9 -and $first.parts[8].label -eq 'A–F(9)') 'get_register: the nine parts are declared with their accepted ranges'

    # check: every published term of every page matches the staged plan.
    $check = Last (Run 'check')
    Assert (@($check.missing).Count -eq 0 -and @($check.diverged).Count -eq 0 -and @($check.extra).Count -eq 0) 'check: no term of any page is missing, diverged, or extra'
    Assert ($check.entries -eq $count -and $check.complete -eq $true) "check: reads all $count published terms ($($check.entries))"
    Assert (@($check.parts).Count -eq 9 -and @($check.plan_parts).Count -eq 0) 'check: the nine declared parts are the accepted layout'

    # a second apply writes nothing and keeps the plan id.
    $again = Last (Run 'apply')
    Assert ($again.wrote -eq 0 -and $again.changed -eq $false -and $again.entries -eq $count) 'apply again: no new Card, no changed body, every term still verified'
    Assert ($again.plan_id -eq $applied.plan_id) 'apply again: the same plan id'
}
finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
