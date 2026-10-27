#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\card-review-fill.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# #187 (L4): the card layer's one review, fix and fill job per group. On a scratch archive and a
# prepared staging folder: the plan writes one job per group with every Card's title, body, recorded
# span and the text of those lines, the group's uncovered units and C3 numbers, and two SELF-CHECK
# lines; a file-only `add` span is refused by precheck; apply files each `add` as a Side successor of
# the note before the gap, appends its span to the group's source log, records the skips, and the
# coverage command counts both. The live archive is never touched.
param([string]$Zt = '', [string]$Digest = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$scratch = Join-Path ([IO.Path]::GetTempPath()) "card-review-fill-$PID"
$archive = Join-Path $scratch 'archive'
$staging = Join-Path $scratch 'staging'
$work = Join-Path $staging 'cards'
New-Item -ItemType Directory -Force $archive, $staging, $work | Out-Null
if ($Zt) {
    $bin = Join-Path $scratch 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$digest = if ($Digest) { $Digest } else { (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Save-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 12), $utf8) }
function Run-Digest([string[]]$argv) { @(& $digest @argv 2>&1 | ForEach-Object { [string]$_ }) }
function Last($lines) { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable }
function Get-Card($at) { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] }
function Write-Calls($calls) {
    $out = @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "zt: $($out -join ' | ')" }
    , @($out | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $key = 'Key2024'
    # Six paragraphs, one line each: unit i is at line 2i-1.
    $paragraphs = 1..6 | ForEach-Object { "第 $_ 段的正文内容，长度超过二十个字，是一个段落单元。" }
    New-Item -ItemType Directory -Force (Join-Path $staging 'sections') | Out-Null
    [IO.File]::WriteAllText((Join-Path $staging 'sections\s01.md'), (($paragraphs -join "`n`n") + "`n"), $utf8)
    Save-Json (Join-Path $staging 'manifest.json') @{ schema_version = 1; citation_key = $key; groups = @(@{ n = 1; id = 'g01'; anchor = "$key/1"; chapter = '第一章'; headings = @('§1.1'); sections = @('s01.md') }) }
    $made = Write-Calls @(
            @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@book{$key, title={Test Book}, author={Hart, Oliver}, year={2024}}"; body = '全书索引。' } },
            @{ tool = 'create_successor'; arguments = @{ at = $key; kind = 'direct'; title = '第 1 组：所有权与剩余控制权'; body = '§1.1 不完全契约' } },
            @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = '所有者在契约空白处决定资产用法'
                    body = "契约无法写明所有情形。所有者决定空白处的资产用法。这位作者还给出 12 个案例。`n`n来源：[[$key]] §1.1" } },
            @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = '剩余控制权提高谈判地位'
                    body = "剩余控制权提高所有者的事后谈判地位。`n`n来源：[[$key]] §1.1" } }
        )
    $tree, $anchor, $n1, $n2 = $made
    # n1 covers unit 1; n2's log entry names only the file (C2), so it covers nothing.
    [IO.File]::WriteAllText((Join-Path $staging 'src-g01.txt'), "$n1`tsections/s01.md:1-2`n$n2`tsections/s01.md`n", $utf8)

    # --- plan: one review-and-fill job for the group ---
    $plan = Last (Run-Digest @('card', '-Step', 'plan', '-Work', $work, '-Compact', '-Book', $key, '-Staging', $staging, '-NoOrphans'))
    $jobs = @($plan.jobs | Where-Object { $_.kind -eq 'fix' })
    Assert ($jobs.Count -eq 1) "plan: one fix job for the one group ($($jobs.Count))"
    $state = Get-Content -LiteralPath (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $job = $state.jobs[$jobs[0].id]
    Assert ($job.group -eq 1 -and @($job.cards).Count -eq 3) "plan: the job names its group and holds the group's Cards with the anchor ($(@($job.cards) -join ', '))"
    Assert (@($job.fills).Count -ge 4) "plan: the job carries the group's uncovered units ($(@($job.fills).Count))"
    $task = Get-Content -LiteralPath $job.file -Raw -Encoding utf8
    $precheck = "(?m)^SELF-CHECK: zt-digest card -Step precheck -Work ""$([regex]::Escape((Resolve-Path $work).Path))"" -Job $([regex]::Escape($jobs[0].id))\s*$"
    $coverage = "(?m)^SELF-CHECK: zt-digest coverage -Out ""$([regex]::Escape((Resolve-Path $staging).Path))"" -Group 1 -Draft "".*$([regex]::Escape($jobs[0].id))\.json""\s*$"
    Assert ($task -match $precheck) 'plan: the task file names its Card precheck'
    Assert ($task -match $coverage) 'plan: the task file names the coverage self-check of the group'
    Assert ($task -match "记录来源行：``sections/s01.md:1-2``" -and $task -match '### 原文行' -and $task -match '第 1 段的正文内容') 'plan: each Card gets its recorded span and the text of those lines'
    Assert ($task -match '## 未覆盖的段落' -and $task -match '作为 `' + [regex]::Escape($n1) + '` 的侧后继' -and $task -match '第 3 段的正文内容') 'plan: the uncovered units carry their text and their planned parent'
    Assert ($task -match '## C3' -and $task -match ([regex]::Escape($n1) + '`：12')) 'plan: the C3 numbers are listed for the group'

    # --- precheck: an add whose span names only a file is refused ---
    $draftPath = $job.out
    $base = @(@{ address = $anchor; verdict = 'ok'; note = '锚点的小结与原文一致。' },
        @{ address = $n1; verdict = 'ok'; note = '逐句对照原文，没有发现问题。' },
        @{ address = $n2; verdict = 'ok'; span = 'sections/s01.md:3-4'; note = '补上可用的来源行。' })
    $adds = @(@{ title = '契约空白处的决定权带来谈判地位'; body = "所有者能拒绝合作，因此有谈判地位。`n`n来源：[[$key]] §1.1"; span = 'sections/s01.md:7-8' },
        @{ title = '所有权决定事前投资'; body = "事前投资取决于事后剩余的分配。`n`n来源：[[$key]] §1.1"; span = 'sections/s01.md:11-12' })
    $skips = @(@{ unit = 'sections/s01.md:5-6'; reason = '版式' }, @{ unit = 'sections/s01.md:9-10'; reason = '版式' })
    $fileOnly = @{ title = '只有文件的 span'; body = "所有者能拒绝合作。`n`n来源：[[$key]] §1.1"; span = 'sections/s01.md' }
    Save-Json $draftPath @{ cards = $base; adds = @($fileOnly); skips = @() }
    $pre = Run-Digest @('card', '-Step', 'precheck', '-Work', $work, '-Job', $jobs[0].id)
    Assert (@($pre | Where-Object { $_ -match 'refuse' -and $_ -match 'names only a file' }).Count -eq 1 -and $pre[-1] -match '^faults: [1-9]') "precheck: an add with a file-only span is refused ($($pre -join ' | '))"
    Save-Json $draftPath @{ cards = $base; adds = $adds; skips = @(@{ unit = 'sections/s01.md:5-6'; reason = '背景' }) }
    $pre = Run-Digest @('card', '-Step', 'precheck', '-Work', $work, '-Job', $jobs[0].id)
    Assert (@($pre | Where-Object { $_ -match '背景' -and $_ -match 'not one of' }).Count -eq 1) "precheck: a reason outside the list is refused ($($pre -join ' | '))"
    Save-Json $draftPath @{ cards = $base; adds = $adds; skips = $skips }
    $pre = Run-Digest @('card', '-Step', 'precheck', '-Work', $work, '-Job', $jobs[0].id)
    Assert ($pre[-1] -eq 'faults: 0') "precheck: a clean review-and-fill draft gives faults: 0 ($($pre -join ' | '))"

    # --- apply: the adds become Side successors, their spans join the source log, the skips are recorded ---
    $applied = Last (Run-Digest @('card', '-Step', 'apply', '-Work', $work))
    Assert ($applied.counts.add -eq 2 -and $applied.counts.skip -eq 2) "apply: two Cards added and two units skipped ($($applied | ConvertTo-Json -Compress))"
    $sides = @((Get-Card $n1).sides | Where-Object { $_.title -eq '契约空白处的决定权带来谈判地位' -or $_.title -eq '所有权决定事前投资' })
    Assert ($sides.Count -eq 2) "apply: both fill Cards are Side successors of the note before the gap ($($sides.title -join ', '))"
    $log = Get-Content -LiteralPath (Join-Path $staging 'src-g01.txt') -Raw -Encoding utf8
    $first = @($sides | Where-Object { $_.title -eq '契约空白处的决定权带来谈判地位' })[0]
    $second = @($sides | Where-Object { $_.title -eq '所有权决定事前投资' })[0]
    Assert ($log -match [regex]::Escape($first.address) + "`tsections/s01.md:7-8" -and $log -match [regex]::Escape($second.address) + "`tsections/s01.md:11-12" -and $log -match [regex]::Escape($n2) + "`tsections/s01.md:3-4") 'apply: the added Cards and the corrected span join the group source log'
    Assert (-not ($log -match "(?m)^" + [regex]::Escape($n2) + "`tsections/s01.md\s*$")) 'apply: the file-only span is replaced, not kept'
    $skipsFile = Get-Content -LiteralPath (Join-Path $staging 'coverage-skips-g01.json') -Raw -Encoding utf8 | ConvertFrom-Json
    Assert (@($skipsFile.skips).Count -eq 2 -and @($skipsFile.skips | Where-Object { $_.reason -eq '版式' }).Count -eq 2) 'apply: the skips are written where the coverage command reads them'

    # --- coverage: the run's measurement counts the added Cards and the skips ---
    $cov = Last (Run-Digest @('coverage', '-Out', $staging, '-Step'))
    Assert ($cov.units -eq 6 -and $cov.covered -eq 4 -and $cov.skipped -eq 2 -and $cov.uncovered -eq 0) "coverage: four units covered (the note, the corrected note and two adds) and two skipped ($($cov | ConvertTo-Json -Compress))"
    Assert ($cov.pct -eq 100) "coverage: the group is fully covered ($($cov.pct)%)"

    # --- a fix keeps every Link of the rewritten Card ---
    $plan2 = Last (Run-Digest @('card', '-Step', 'plan', '-Work', (Join-Path $scratch 'again'), '-Book', $key, '-Staging', $staging))
    $state2 = Get-Content -LiteralPath (Join-Path $scratch 'again\state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $job2 = @($state2.jobs.Keys | Where-Object { $state2.jobs[$_].kind -eq 'fix' })[0]
    $target = @($state2.jobs[$job2].cards)[0]
    $current = Get-Card $target
    Save-Json $state2.jobs[$job2].out @{ cards = @(@{ address = $target; verdict = 'fix'; title = $current.title; body = '改写时丢掉了全部 Link。' }) }
    $refused = Last (Run-Digest @('card', '-Step', 'apply', '-Work', (Join-Path $scratch 'again')))
    Assert ([int]$refused.counts.refused -ge 1) "apply: a fix that loses a Link is refused ($($refused.counts | ConvertTo-Json -Compress))"
    # --- #191: one group job carries the group's review, fix, fill and its orphan Links ---
    $orphan = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = $anchor; kind = 'side'; title = '没有链接的笔记'
                    body = '这一段没有链接到任何卡片，也没有记录来源行。' } }))[0]
    $bothWork = Join-Path $scratch 'both'
    $both = Last (Run-Digest @('card', '-Step', 'plan', '-Work', $bothWork, '-Compact', '-Book', $key, '-Staging', $staging))
    $bothJobs = @($both.jobs)
    Assert ($bothJobs.Count -eq 1 -and $bothJobs[0].kind -eq 'both') "plan: one job holds the group's Cards and its orphan Links ($(@($bothJobs | ForEach-Object { "$($_.id):$($_.kind)" }) -join ', '))"
    Assert (@($bothJobs | Where-Object { $_.id -like 'orphans-*' }).Count -eq 0) 'plan: no separate orphan job for a group that has a job'
    Assert ($both.prompts.both -and $both.lead.fix -and $both.lead.orphans) 'plan: the compact list names both instruction files and the one combined prompt'
    $bothState = Get-Content -LiteralPath (Join-Path $bothWork 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $bothJob = $bothState.jobs[$bothJobs[0].id]
    Assert (@($bothJob.orphans) -contains $orphan -and @($bothJob.candidates[$orphan]).Count -ge 1) "plan: the job names its orphan and its candidates ($(@($bothJob.orphans) -join ', '))"
    $bothTask = Get-Content -LiteralPath $bothJob.file -Raw -Encoding utf8
    Assert ($bothTask -match '## 本组孤立卡片' -and $bothTask -match [regex]::Escape($orphan) -and $bothTask -match '候选卡片') 'plan: the task file carries the orphan-link section'
    $bothPrecheck = "(?m)^SELF-CHECK: zt-digest card -Step precheck -Work ""$([regex]::Escape((Resolve-Path $bothWork).Path))"" -Job $([regex]::Escape($bothJobs[0].id))\s*$"
    Assert ($bothTask -match $bothPrecheck -and $bothTask -match 'SELF-CHECK: zt-digest coverage') 'plan: the one job keeps both SELF-CHECK lines'
    # A group draft answers for every orphan it names: `links` or `none` per orphan, in the same JSON.
    $bothDraft = $bothJob.out
    $okCards = @(@($bothJob.cards) | ForEach-Object { @{ address = $_; verdict = 'ok'; note = '与原文一致。' } })
    $otherOrphans = @(@($bothJob.orphans) | Where-Object { $_ -ne $orphan })
    Save-Json $bothDraft @{ cards = $okCards; adds = @(); skips = @(); links = @(); none = @() }
    $bothPre = Run-Digest @('card', '-Step', 'precheck', '-Work', $bothWork, '-Job', $bothJobs[0].id)
    Assert (@($bothPre | Where-Object { $_ -match [regex]::Escape($orphan) -and $_ -match 'no entry for this orphan' }).Count -eq 1) "precheck: the group job answers for its orphan Link ($($bothPre -join ' | '))"
    $from = @($bothJob.candidates[$orphan])[0]
    Save-Json $bothDraft @{ cards = $okCards; adds = @(); skips = @()
        links = @(@{ to = $orphan; from = $from; text = "对照 [[$orphan]]：这里讨论的是没有链接的笔记。" })
        none = @($otherOrphans | ForEach-Object { @{ orphan = $_; note = '本组的候选卡片都不合适。' } }) }
    $bothPre = @(Run-Digest @('card', '-Step', 'precheck', '-Work', $bothWork, '-Job', $bothJobs[0].id))
    Assert ($bothPre[-1] -eq 'faults: 0') "precheck: a group draft with a clean orphan Link gives faults: 0 ($($bothPre -join ' | '))"
    # Apply writes the group's orphan Link in the same job.
    $bothApplied = Last (Run-Digest @('card', '-Step', 'apply', '-Work', $bothWork))
    Assert ($bothApplied.counts.orphan_links -eq 1) "apply: the group job writes its orphan Link ($($bothApplied.counts | ConvertTo-Json -Compress))"
    Assert ((Get-Card $from).body -match "对照 \[\[$([regex]::Escape($orphan))\]\]") 'apply: the candidate links the orphan'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
