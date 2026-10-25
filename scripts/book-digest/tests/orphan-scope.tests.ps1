#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST, [string]$Checker = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "orphan-scope-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the test temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Read-Json($path) { Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable }
function Audit($out) {
    $lines = @(& $Digest audit -CitationKey OrphanBook -Out $out -SkipModels 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "audit failed: $($lines -join ' | ')" }
    Read-Json (Join-Path $out 'audit.json')
}
$oldEditor = $env:EDITOR
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $null = Invoke-Zt @(
        @{tool='create_literature';arguments=@{bibtex='@book{OrphanBook,title={Orphan scope}}'}},
        @{tool='create_successor';arguments=@{at='OrphanBook';title='第一组'}},
        @{tool='create_successor';arguments=@{at='OrphanBook/1';kind='side';title='仅有来源的卡';body='来源：[[OrphanBook]]。'}},
        @{tool='create_successor';arguments=@{at='OrphanBook/1';kind='side';title='只有自身链接的卡';body='来源：[[OrphanBook]]。'}},
        @{tool='create_successor';arguments=@{at='OrphanBook/1';kind='side';title='父卡确实引用的卡';body='来源：[[OrphanBook]]。'}},
        @{tool='create_successor';arguments=@{at='OrphanBook/1';kind='side';title='人类工作区引用的卡';body='来源：[[OrphanBook]]。'}},
        @{tool='create_successor';arguments=@{at='OrphanBook/1';kind='side';title='2020 年的事件';type='event';body="时间：2020 年。`n地点：城市。`n参与者：企业。`n经过：企业关闭。`n来源：[[OrphanBook]]。"}},
        @{tool='create_successor';arguments=@{at='OrphanBook/1';kind='side';title='组导航';type='hub';level='line'}},
        @{tool='create_successor';arguments=@{at='OrphanBook/1';kind='side';title='术语表'}},
        @{tool='create_successor';arguments=@{at='OrphanBook/1';kind='side';title='符号表'}},
        @{tool='create_successor';arguments=@{at='OrphanBook/1';kind='side';title='全书结构';body='本卡是《Orphan scope》的结构卡。'}},
        @{tool='create_successor';arguments=@{at='OrphanBook/1';title='一个独立模型';type='model';level='concept'}},
        @{tool='create_successor';arguments=@{at='OrphanBook/2';kind='side';title='模型设定';type='model';facet='设定'}}
    )
    $null = Invoke-Zt @(
        @{tool='edit_card';arguments=@{at='OrphanBook/1|b';body='参照 [[OrphanBook/1|b]]，这里仅重复自身。来源：[[OrphanBook]]。'}},
        @{tool='edit_card';arguments=@{at='OrphanBook/1';body='参照 [[OrphanBook/1|c]]，后者给出了本组的具体论点。参照 [[OrphanBook/1|e]]，后者给出了本组的真实事件。'}}
    )
    $editor = Join-Path $scratch 'editor.ps1'
    [IO.File]::WriteAllText($editor,'param([string]$File); [IO.File]::WriteAllText($File,$env:ZT_ORPHAN_TEST_BODY,[Text.UTF8Encoding]::new($false))',$utf8)
    $env:EDITOR = "pwsh -NoProfile -File $editor"
    # docs/literature-only-digest-design.md: the Card cycle reads Literature trees only, so the human
    # inbound source is a human Literature-tree Card, not a human Topic.
    $env:ZT_ORPHAN_TEST_BODY = "@book{HumanNotes2020, title={Human notes}}`n"
    $humanBook = @(zt l --ws human 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and $humanBook[-1] -eq 'HumanNotes2020') "the owner CLI creates a human Literature tree: $($humanBook -join ' | ')"
    $env:ZT_ORPHAN_TEST_BODY = "# 人类作者的讨论`n`n参照 [[OrphanBook/1|d]]，后者给出了本次讨论的约束。`n"
    $human = @(zt n --at 'HumanNotes2020' 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and $human[-1] -eq 'HumanNotes2020/1') "the human inbound fixture uses the owner CLI: $($human -join ' | ')"
    $audit = Audit (Join-Path $scratch 'audit-before')
    $work = Join-Path $scratch 'card'
    $plan = @(& $Digest card -Step plan -Book OrphanBook -Work $work -Rules 'I5,E4,F4' -NoOrphans 2>&1)
    Assert ($LASTEXITCODE -eq 0) "the public Card plan reads the same graph: $($plan[-1])"
    $card = Read-Json (Join-Path $work 'before\audit.json')
    Assert ($audit.orphan_check.complete -and $card.orphan_check.complete) 'audit and Card checks publish complete explicit orphan coverage'
    $expected = 'OrphanBook/1|a,OrphanBook/1|b,OrphanBook/2,OrphanBook/2|a'
    Assert (($audit.orphan_check.list -join ',') -ceq $expected -and ($card.orphan_check.list -join ',') -ceq $expected) 'exact orphan sets exclude self-Links and outbound citations, and accept authored parent/human inbound Links'
    Assert ($audit.orphan_check.archive_revision -eq $card.orphan_check.archive_revision -and ($audit.orphan_check.targets -join ',') -ceq ($card.orphan_check.targets -join ',')) 'equal sets name the same target scope and graph revision'
    Assert (($card.orphan_check.source_workspaces -join ',') -eq 'human,agent' -and $card.orphan_check.target_workspace -eq 'agent') 'inbound sources span both Workspaces while repair targets remain agent Cards'
    Assert (($card.orphan_check.excluded.Keys | Sort-Object) -join ',' -eq 'anchor,hub,notation,root,structure,terms') 'roots, anchors, structure, term, notation and hub Cards are explicitly excluded'
    $faults = [IO.File]::ReadAllText((Join-Path $work 'before\audit-faults.txt'))
    Assert ($faults.Contains('OrphanBook/1|e E4') -and -not $faults.Contains('OrphanBook/1|e I5')) 'strong kinded inbound evidence stays separate from ordinary orphanhood'
    $null = Invoke-Zt @(@{tool='edit_card';arguments=@{at='OrphanBook/1';body='依赖 [[OrphanBook/1|a]]，后者给出了新的共同约束。参照 [[OrphanBook/1|c]]，后者给出了本组的具体论点。参照 [[OrphanBook/1|e]]，后者给出了本组的真实事件。'}})
    $env:ZT_ORPHAN_TEST_BODY = "# 企业退出限制了选择`n`n依赖 [[OrphanBook/1|e]]，后者给出了企业退出的真实事件。`n"
    $humanIdea = @(zt b --at 'HumanNotes2020/1' 2>&1 | Where-Object { $_ -notmatch '^check: ' })
    Assert ($LASTEXITCODE -eq 0 -and $humanIdea[-1] -eq 'HumanNotes2020/1|a') 'the owner CLI creates a human Idea carrier with a kinded Link'
    $laterAudit = Audit (Join-Path $scratch 'audit-after')
    $report = @(& $Digest card -Step report -Work $work 2>&1)
    Assert ($LASTEXITCODE -eq 0) "the Card report rereads the later graph: $($report[-1])"
    $laterCard = Read-Json (Join-Path $work 'after\audit.json')
    Assert ($laterCard.orphan_check.archive_revision -gt $card.orphan_check.archive_revision -and $laterAudit.orphan_check.archive_revision -eq $laterCard.orphan_check.archive_revision) 'later orphan results identify their changed graph revision'
    Assert (($laterCard.orphan_check.list -join ',') -ceq ($laterAudit.orphan_check.list -join ',') -and 'OrphanBook/1|a' -notin $laterCard.orphan_check.list) 'both commands reflect the exact resolved orphan after the authored inbound edit'
    Assert ('OrphanBook/1' -in $laterCard.orphan_check.changed_cards) 'the later Card report names authored changes since its before view'
    $laterFaults = [IO.File]::ReadAllText((Join-Path $work 'after\audit-faults.txt'))
    Assert (-not $laterFaults.Contains('OrphanBook/1|e E4')) 'the separate Idea-kinded statistic accepts evidence from either Workspace'
    $cardReport = Read-Json (Join-Path $work 'report.json')
    Assert ($cardReport.orphans_after -eq $laterCard.orphan_check.list.Count) 'the public Card summary uses the shared orphan set rather than the stronger layer fault count'
    $null = Invoke-Zt @(
        # docs/literature-only-digest-design.md: declared work is a Literature-tree Card; a Topic Card belongs to no book.
        @{tool='create_successor';arguments=@{at='OrphanBook/2';kind='side';title='尚未标明来源的观点';body='这张观点卡由本书的清单明确归属。'}}
    )
    $staging = Join-Path $scratch 'staging'
    New-Item -ItemType Directory -Force $staging | Out-Null
    [IO.File]::WriteAllText((Join-Path $staging 'owned-g01.txt'),"OrphanBook/2|b`n",$utf8)
    $declaredWork = Join-Path $scratch 'declared-card'
    $declaredPlan = @(& $Digest card -Step plan -Book OrphanBook -Work $declaredWork -Staging $staging -Rules I5 -NoOrphans 2>&1)
    Assert ($LASTEXITCODE -eq 0) 'the Card cycle includes declared Literature work'
    $declaredCard = Read-Json (Join-Path $declaredWork 'before\audit.json')
    $targets = Join-Path $scratch 'orphan-targets.txt'
    [IO.File]::WriteAllText($targets,($declaredCard.orphan_check.targets -join "`n"),$utf8)
    $declaredOut = Join-Path $scratch 'declared-audit'
    $declaredAuditLines = @(& $Digest audit -CitationKey OrphanBook -OrphanTargets $targets -Out $declaredOut -SkipModels 2>&1)
    Assert ($LASTEXITCODE -eq 0) "audit accepts the exact declared orphan target scope: $($declaredAuditLines[-1])"
    $declaredAudit = Read-Json (Join-Path $declaredOut 'audit.json')
    Assert ('OrphanBook/2|b' -in $declaredAudit.orphan_check.list -and ($declaredAudit.orphan_check.list -join ',') -ceq ($declaredCard.orphan_check.list -join ',') -and $declaredAudit.archive_revision -eq $declaredCard.archive_revision) 'explicitly equal scopes retain the same full orphan set, including unattributed owned work'
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_BULK_MISSING = 'HumanNotes2020/1|a'
    $failedOut = Join-Path $scratch 'incomplete-audit'
    $failedAudit = @(& $Digest audit -CitationKey OrphanBook -Out $failedOut -SkipModels 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and -not (Test-Path (Join-Path $failedOut 'audit.json'))) 'an unreadable human graph source cannot publish a complete orphan report'
    $failedWork = Join-Path $scratch 'incomplete-card'
    $failedCard = @(& $Digest card -Step plan -Book OrphanBook -Work $failedWork -Rules I5 -NoOrphans 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and -not (Test-Path (Join-Path $failedWork 'state.json'))) 'an unreadable required human Idea carrier cannot publish a zero-orphan Card plan'
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_BULK_MISSING
    for ($n=3; $n -le 8; $n++) {
        $null = Invoke-Zt @(@{tool='create_successor';arguments=@{at="OrphanBook/$($n-1)";title="第 $n 组"}})
    }
    $null = Invoke-Zt @(@{tool='create_successor';arguments=@{at='OrphanBook/1';kind='side';title='少量链接仍然陈述观点';body='见 [[OrphanBook/1]]、[[OrphanBook/2]]、[[OrphanBook/3]]。'}})
    if (-not $Checker) { $Checker = (Get-Command zt-check -CommandType Application | Select-Object -First 1).Source }
    $classification = @(& $Checker --cards 'OrphanBook/1|j' --book OrphanBook --rules I2 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and ($classification -join ' ').Contains('OrphanBook/1|j I2')) 'standalone checks use the same complete anchor count before classifying a structure Card'
} finally {
    $env:EDITOR = $oldEditor
    Remove-Item Env:ZT_ORPHAN_TEST_BODY -ErrorAction SilentlyContinue
    Remove-Item Env:ZT_CHECK_ZT,Env:ZT_BULK_MISSING -ErrorAction SilentlyContinue
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
