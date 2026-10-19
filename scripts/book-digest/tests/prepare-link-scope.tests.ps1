#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\prepare-link-scope.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
<#
.SYNOPSIS
  The Link pass ownership boundary of a prepared run (issue #92).

.DESCRIPTION
  prepare-link owns a Card of this book when this group's own receipt names it. The regression of
  #92 is a digest that did not complete: its groups had no receipt, so the old fallback inferred
  ownership from any body that mentions a `§1.2`-style section, and the Link pass then wrote into
  Cards of other books and of existing Topic trees.

  This suite drives the real commands on one prepared economics run of the pilot source (3 groups):
    * with no completed digest every group is `blocked`, its allow-list is empty and its link file
      says so, and no Card is offered to a link job;
    * a foreign book's Card that cites `来源：[[<key>]] §1.2` and an existing Topic tree's Card that
      names `§2.1` are never owned, before or after a group completes;
    * a completed group (group-start -> draft -> group-finish) becomes pending and owns exactly its
      receipts, its anchor and its own Literature-tree Cards;
    * a group whose digest unit failed stays blocked with an empty allow-list even when an
      `owned-gNN.txt` naming a foreign Card is present.

  It starts no provider run and no workflow, and it does not touch the live archive.
#>
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "prepare-link-scope-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$here = Join-Path $PSScriptRoot '..' | Resolve-Path | ForEach-Object Path
$repo = Join-Path $PSScriptRoot '..\..\..' | Resolve-Path | ForEach-Object Path
. (Join-Path $here 'zt-calls.ps1')
$out = Join-Path $scratch 'prepared'
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1 | ForEach-Object { "$_" })
    $json = $null
    if ($lines.Count) { try { $json = $lines[-1] | ConvertFrom-Json -AsHashtable -DateKind String } catch { } }
    [pscustomobject]@{ exit = $LASTEXITCODE; text = ($lines -join ' | '); json = $json }
}

try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $source = Join-Path $repo 'docs\digest-pilot-short-source.zh.md'
    if (-not (Test-Path -LiteralPath $source)) { throw "the pilot source is missing: $source" }
    # The workflow's own prepare form (the frozen prepare stage's JSON text switches).
    $prepared = Digest @('prepare', '-Variant', 'econ', '-Source', $source, '-Out', $out, '-CitationKey', 'ScopeBook',
        '-BookTitle', 'Link scope book', '-BibTeX', '@book{ScopeBook,title={Link scope book},author={Scope, Ada},year={2026}}',
        '-RunOptions', '{"terms":false}', '-Apply')
    $groups = if ($prepared.json) { @($prepared.json.groups).Count } else { 0 }
    Assert ($prepared.exit -eq 0 -and $groups -eq 3) "prepare binds the pilot source with three groups ($groups)"
    Assert (Test-Path -LiteralPath (Join-Path $out 'ledger.sqlite3')) 'the prepared run holds its own ledger'
    if ($groups -ne 3) { throw 'the suite needs the three-group prepared run' }

    # --- foreign Cards: another book's Card that cites this book, and an existing Topic tree's Card
    $foreignBook = Invoke-Zt @(@{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{Other2020,title={Another book}}'; body = '另一本书的书卡。' } })
    $null = Invoke-Zt @(
        @{ tool = 'create_successor'; arguments = @{ at = 'Other2020'; kind = 'direct'; title = '风险与租金'; body = "风险分担说明租金。`n`n来源：[[ScopeBook]] §1.2" } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Other2020/1'; kind = 'side'; title = '企业的边界'; body = "企业边界由成本决定。`n`n来源：[[ScopeBook]] §2.1" } },
        @{ tool = 'create_topic'; arguments = @{ title = '旧主题'; body = '一个早已存在的主题。' } },
        @{ tool = 'create_successor'; arguments = @{ at = '0/0'; title = '旧主题下的卡片'; body = "这张卡片属于旧主题。`n`n§1.2 与 §2.1 都无关。" } }
    )
    Assert ((Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = 'Other2020/1|a' } })).title -eq '企业的边界') 'a second book and an existing Topic tree are created in the archive'

    # --- 1. no completed digest: no group gets a link job
    $first = Digest @('prepare-link', '-Staging', $out)
    $summary = $first.json
    Assert ($first.exit -eq 0 -and $summary -and $summary.digest_state -eq 'ledger') "prepare-link reads the run's ledger ($($summary.digest_state))"
    Assert (@($summary.pending).Count -eq 0) "no group is pending before its digest completes ($(@($summary.pending) -join ', '))"
    Assert (@($summary.blocked).Count -eq 3 -and (@($summary.blocked | ForEach-Object { $_.n }) -join ',') -eq '1,2,3') "all three groups are reported blocked ($((@($summary.blocked | ForEach-Object { "$($_.n):$($_.state)" })) -join ', '))"
    $emptyAllows = @(1..3 | Where-Object { @(Get-Content -LiteralPath (Join-Path $out "allow-g0$_.txt") -Encoding utf8 | Where-Object { $_ -ne '' }).Count -eq 0 })
    Assert ($emptyAllows.Count -eq 3) "every blocked group's allow-list is empty ($($emptyAllows -join ', '))"
    $blockedLink = Get-Content -LiteralPath (Join-Path $out 'link-g01.md') -Raw -Encoding utf8
    Assert ($blockedLink.Contains('digest 未完成') -and -not $blockedLink.Contains('### `')) 'a blocked group''s link file offers no Card'

    # --- 2. one group completes through its real lifecycle
    $claim = (Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'scope-manager')).json.units[0]
    Assert ($claim.id -eq 'digest/g001@1') "the run's first ready group is claimable ($($claim.id))"
    $started = Digest @('group-start', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)")
    $job = Get-Content -LiteralPath $started.json.job -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $draft = @{
        schema_version = 1; unit = $claim.id; token = $claim.token; input_hash = $started.json.input_hash; disposition = 'cards'
        calls = @(
            @{ tool = 'edit_card'; arguments = @{ at = 'ScopeBook/1'; title = '企业因市场失灵而出现'; body = '企业替代市场。企业的边界由交易成本决定，市场失灵使企业出现。'; base_fingerprint = $job.inputs.anchor.fingerprint } },
            @{ tool = 'create_successor'; arguments = @{ at = 'ScopeBook/1'; kind = 'side'; title = '市场失灵使企业替代市场'; body = '市场失灵使企业替代市场。交易成本决定企业的边界。企业内部协调减少交易成本。来源：[[ScopeBook/1]]' } }
        )
    }
    [IO.File]::WriteAllText($started.json.draft, ($draft | ConvertTo-Json -Depth 20), $utf8)
    $finished = Digest @('group-finish', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)", '-Draft', $started.json.draft)
    Assert ($finished.exit -eq 0 -and $finished.json.state -eq 'done') "group 1 completes through group-start and group-finish ($($finished.json.state))"
    $created = @($finished.json.substantive)[0]
    Assert ($created -and $created -ne 'ScopeBook/1') "the group owns its new Card ($created)"

    # --- 3. the completed group is the only pending one, and owns exactly its receipts
    $second = Digest @('prepare-link', '-Staging', $out)
    $summary = $second.json
    Assert ((@($summary.pending) -join ',') -eq '1') "only the completed group is pending ($(@($summary.pending) -join ', '))"
    $allow1 = @(Get-Content -LiteralPath (Join-Path $out 'allow-g01.txt') -Encoding utf8 | Where-Object { $_ -ne '' })
    Assert (($allow1 -join ',') -eq "ScopeBook/1,$created" -or ($allow1 -join ',') -eq "$created,ScopeBook/1") "the completed group owns its anchor and its own Card ($($allow1 -join ', '))"
    Assert (($allow1 -notcontains 'Other2020') -and ($allow1 -notcontains 'Other2020/1') -and ($allow1 -notcontains 'Other2020/1|a')) 'no Card of another book is owned'
    Assert (-not ($allow1 | Where-Object { $_ -like '0/*' })) 'no Card of an existing Topic tree is owned'
    Assert (@($summary.blocked).Count -eq 2) "the two unfinished groups stay blocked ($(@($summary.blocked | ForEach-Object { $_.n }) -join ', '))"

    # --- 4. a failed digest group stays blocked, whatever an old receipt says
    $claim2 = (Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'scope-manager')).json.units[0]
    Assert ($claim2.id -eq 'digest/g002@1') "the next group is claimable ($($claim2.id))"
    $fault = @{ schema_version = 1; unit = $claim2.id; input_hash = $claim2.input_hash; code = 'check_failed'; cause = 'test: the group fails once so the link scope is checked against a failed digest' }
    $failure = Digest @('ledger', 'fail', '-Out', $out, '-Unit', $claim2.id, '-Token', "$($claim2.token)", '-Class', 'content', '-Fault', ($fault | ConvertTo-Json -Depth 6 -Compress))
    Assert ($failure.exit -eq 0 -and $failure.json.state -eq 'failed') "group 2 records a content failure ($($failure.json.state))"
    # An owned log naming a foreign Card is not a completion: the ledger state decides.
    [IO.File]::WriteAllText((Join-Path $out 'owned-g002.txt'), "Other2020/1`n0/1`n", $utf8)
    $third = Digest @('prepare-link', '-Staging', $out)
    $summary = $third.json
    $blocked2 = @($summary.blocked) | Where-Object { $_.n -eq 2 }
    Assert ($blocked2 -and $blocked2.state -eq 'failed') "the failed group is blocked by its ledger state ($($blocked2.state))"
    $allow2 = @(Get-Content -LiteralPath (Join-Path $out 'allow-g02.txt') -Encoding utf8 | Where-Object { $_ -ne '' })
    Assert ($allow2.Count -eq 0) "a failed group's allow-list stays empty even with an owned log ($($allow2 -join ', '))"
    Assert ((@($summary.pending) -join ',') -eq '1') "the failed group is not pending ($(@($summary.pending) -join ', '))"
    Assert ((@($summary.blocked | ForEach-Object { $_.n }) -join ',') -eq '2,3') "the blocked closure names groups 2 and 3 ($(@($summary.blocked | ForEach-Object { $_.n }) -join ', '))"

    # --- 5. a finished link pass takes its group off the pending list
    [IO.File]::WriteAllText((Join-Path $out 'linked-g01.txt'), "done`n", $utf8)
    $fourth = Digest @('prepare-link', '-Staging', $out)
    Assert (@($fourth.json.pending).Count -eq 0 -and $fourth.json.groups[0].done -eq $true) "linked-gNN.txt takes a completed group off the pending list (pending $(@($fourth.json.pending) -join ','); done $($fourth.json.groups[0].done); groups $(@($fourth.json.groups | ForEach-Object { "$($_.n):$($_.digest):$($_.done)" }) -join ' '))"
    $cardsAll = Get-Content -LiteralPath (Join-Path $scratch 'prepared\cards-all.md') -Raw -Encoding utf8
    Assert ($cardsAll.Contains('Other2020') -and $cardsAll.Contains('旧主题下的卡片')) 'cards-all.md still lists every Card of both Workspaces'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
if ($failed) { Write-Output "$failed failed"; exit 1 }
Write-Output 'all passed'
