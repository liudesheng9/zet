#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\register-human-target.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# S10 / issue #101: a human Card may be a register target only when the concept review selects it
# explicitly (design §8.3, D174). The register names it; the publication never writes the human
# Workspace and the human Card's authored text never changes. A reviewed entry that no longer
# exists is an excluded stale review, not an invented address. A real scratch archive; the live
# archive is not touched.
param([string]$Zt = '', [string]$Digest = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$scratch = Join-Path ([IO.Path]::GetTempPath()) "register-human-target-test-$PID"
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
function Staged { Get-Content -Raw -LiteralPath (Join-Path $work 'out\register-plan.json') | ConvertFrom-Json -AsHashtable -DateKind String }
function Titles([string]$workspace) { @(Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{ workspace = $workspace } }))[0].titles }
function Card([string]$at) { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] }
function Terms($rows) {
    $map = [ordered]@{}
    foreach ($row in @($rows)) {
        $map[$row.term] = @($row.targets | ForEach-Object { if ($_ -is [string]) { $_ } elseif ($_.exists) { $_.address } })
    }
    $map
}
function Gui {
    $url = (& zt gui) -split "`n" | Select-Object -First 1
    $port = [int]([regex]::Match($url, '127\.0\.0\.1:(\d+)').Groups[1].Value)
    $page = (Invoke-WebRequest -Uri "http://127.0.0.1:$port/" -UseBasicParsing).Content
    $token = [regex]::Match($page, 'name="zt-token" content="([0-9a-f]{32})"').Groups[1].Value
    @{ port = $port; token = $token }
}
function Gui-Create($gui, $body) {
    Invoke-RestMethod -Uri "http://127.0.0.1:$($gui.port)/api/create" -Method Post -Headers @{ 'X-ZT-Token' = $gui.token } -ContentType 'application/json' -Body ($body | ConvertTo-Json -Compress)
}
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    # One agent book whose term table names terms no agent Card carries.
    $book = @(Invoke-Zt @(
            @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{H2026, title={产权复核}, author={Owner, A.}, year={2026}}'; body = '全书索引。' } },
            @{ tool = 'create_successor'; arguments = @{ at = 'H2026'; kind = 'direct'; title = '第 1 组：契约未尽处的决定'; body = '§1 契约未尽处的决定' } }
        ))
    $anchor = (Titles 'agent' | Where-Object { $_.title -eq '第 1 组：契约未尽处的决定' } | Select-Object -First 1).address
    $tableBody = @'
本卡是《产权复核》的术语表。

术语表

| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 产权 | 所有权 | 谁拥有企业并取得剩余 | §1.2 |
| 剩余控制权 | 控制权 | 谁在契约未尽处做决定 | §1.4 |
'@
    $null = Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{ at = $anchor; kind = 'side'; title = '术语表：产权复核'; body = $tableBody } })
    # The person's own Cards, made through the owner's GUI: only that route writes the human
    # Workspace. The Card's title never names a term as an agent candidate would.
    $gui = Gui
    $humanTopic = Gui-Create $gui @{ kind = 'topic'; workspace = 'human'; text = "# 人的产权笔记`n`n人自己的问题：谁拥有企业？`n" }
    $humanCard = Gui-Create $gui @{ kind = 'direct'; at = $humanTopic.address; text = "# 所有权的归属`n`n剩余控制权归所有者，收益也归他。`n" }
    $authored = Card $humanCard.address
    $humanCount = @(Titles 'human').Count
    $agentCount = @(Titles 'agent').Count
    $facts = @(Invoke-Zt @(@{ tool = 'get_cards'; arguments = @{ addresses = @(); view = 'text' } }))[0]
    $reviewPath = Join-Path $scratch 'concept-review.json'
    $review = [ordered]@{
        schema_version = 1
        report_id      = 'r-' + ('b' * 64)
        created_at     = '2026-10-19T00:00:00.0000000Z'
        book           = [ordered]@{ citation_key = 'H2026'; title = '产权复核' }
        archive        = $facts.archive_identity
        status         = 'complete'
        completeness   = [ordered]@{ complete = $true }
        evidence_path  = 'e-2'
        proposals      = @(
            [ordered]@{ id = 'p001'; title = '产权' },
            [ordered]@{ id = 'p002'; title = '剩余控制权' }
        )
        findings       = @(
            [ordered]@{ id = 'c001'; kind = 'topic-concept'; proposal = 'p001'; candidate = $humanTopic.address; entry = $humanCard.address; verdict = 'same'; scope = '所有权'; reason = '同一个问题。' },
            [ordered]@{ id = 'c002'; kind = 'topic-concept'; proposal = 'p002'; candidate = $humanCard.address; entry = '99/9'; verdict = 'same'; scope = '剩余控制权'; reason = '审查时指向的卡已经不在。' }
        )
    }
    [IO.File]::WriteAllText($reviewPath, ($review | ConvertTo-Json -Depth 10), $utf8)

    # Without the review, a human Card is never an automatic candidate: the terms stay unresolved.
    $plain = Last (Run 'plan')
    $plainStaged = Staged
    Assert (@($plainStaged.terms).Count -eq 0 -and @($plainStaged.unresolved).Count -eq 2) 'plan without a review: no human Card is a target, and both source rows are unresolved'
    Assert (@($plainStaged.unresolved | Where-Object { $_.term -eq '产权' }).Count -eq 1) 'plan without a review: the human Card is never picked automatically'

    # The review selects the human Card explicitly: it is a read-only target of the term.
    $plan = Last (Run 'plan' @('-Review', $reviewPath))
    $staged = Staged
    $terms = Terms $staged.terms
    Assert (@($terms['产权']).Count -eq 1 -and $terms['产权'][0] -eq $humanCard.address) 'plan: the reviewed human Card is the term target'
    Assert (@($terms['所有权']).Count -eq 1 -and $terms['所有权'][0] -eq $humanCard.address) 'plan: the book word 不用 names the same human target'
    $readOnly = @($staged.terms | Where-Object { $_.term -eq '产权' })[0].read_only_targets
    Assert (@($readOnly).Count -eq 1 -and $readOnly[0] -eq $humanCard.address) 'plan: the human target is recorded as read-only'
    Assert ($plan.read_only -eq 2) "plan: reports the read-only targets of both accepted spellings ($($plan.read_only))"
    Assert (@($staged.excluded).Count -eq 1 -and $staged.excluded[0].address -eq '99/9' -and $staged.excluded[0].reason -eq 'reviewed_entry_missing') 'plan: a reviewed entry that no card holds is excluded as a stale review'
    Assert (@($staged.unresolved | Where-Object { $_.term -eq '剩余控制权' }).Count -eq 1) 'plan: the stale review leaves its term unresolved'

    # apply: the register names the human Card; nothing in the human Workspace is written.
    $applied = Last (Run 'apply')
    Assert ($applied.created -eq 2 -and $applied.entries -eq 2 -and $applied.read_only -eq 2) "apply: creates the register and names the read-only target ($($applied.entries) entries, $($applied.read_only) read-only)"
    Assert ($applied.verified -eq $true -and $applied.complete -eq $true) 'apply: verifies the published terms'
    $register = Card $applied.register
    Assert ($register.body -match [regex]::Escape("- 产权：[[$($humanCard.address)]]")) 'the register body names the human Card'
    $published = @(Invoke-Zt @(@{ tool = 'get_register'; arguments = @{ workspace = 'agent' } }))[0]
    $entries = Terms $published.entries
    Assert ($entries['产权'][0] -eq $humanCard.address) 'get_register: the human Card is the published target'
    $target = @($published.entries | Where-Object { $_.term -eq '产权' })[0].targets[0]
    Assert ($target.workspace -eq 'human' -and $target.exists -eq $true) 'get_register: the target is a human Card'
    $after = Card $humanCard.address
    Assert ($after.title -eq $authored.title -and $after.body -eq $authored.body) 'apply: the human Card keeps its authored title and body'
    Assert (@(Titles 'human').Count -eq $humanCount) 'apply: the human Workspace gained no Card'
    Assert (@(Titles 'agent').Count -eq $agentCount + 2) 'apply: the agent Workspace gained only the Topic 索引 and the register'

    # check: the published register matches the staged plan, and the read-only target stays named.
    $check = Last (Run 'check')
    Assert (@($check.missing).Count -eq 0 -and @($check.diverged).Count -eq 0 -and @($check.extra).Count -eq 0 -and $check.complete -eq $true) 'check: the published register matches the staged plan'
    Assert ($check.read_only -eq 2) 'check: the plan still names its read-only target'
    $again = Last (Run 'apply')
    Assert ($again.created -eq 0 -and $again.changed -eq $false -and $again.plan_id -eq $applied.plan_id) 'apply again: no new Card, no changed body, the same plan id'
}
finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
