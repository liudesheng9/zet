#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\organize.tests.ps1 [-Zt <zt.exe>]
# Drives the new Organize stage on a scratch archive: organize-survey.ps1 finds the duplicate
# Topics, organize-apply.ps1 merges them and moves a Card (and refuses a Model Card and a Literature
# Card), and book-skeleton.ps1 (plan, apply) writes the structure Card, the Literature Card index,
# and a Topic body that keeps its hub line, standing in for Opus's decisions.json and DeepSeek's
# skeleton.json.
param([string]$Zt = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$scratch = Join-Path ([IO.Path]::GetTempPath()) "organize-test-$PID"
$archive = Join-Path $scratch 'archive'
$staging = Join-Path $scratch 'staging'
$work = Join-Path $staging 'organize'
New-Item -ItemType Directory -Force $archive, $staging | Out-Null
if ($Zt) {
    $bin = Join-Path $scratch 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$dir = Join-Path $PSScriptRoot '..' | Resolve-Path | ForEach-Object Path
. (Join-Path $dir 'zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Last($lines) { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable }
function Write-Calls($calls) {
    $out = @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "zt: $out" }
    , @($out | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Get-Card($at) { try { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] } catch { $null } }
function Save-Json($path, $value) { New-Item -ItemType Directory -Force (Split-Path -Parent $path) | Out-Null; [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 10), $utf8) }
$key = 'Org2024'
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $null = Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@book{$key, title={Organize}, author={Hart, Oliver}, year={2024}}"; body = "旧索引。`n`n原书文档：[原书 PDF](scholar:85)" } },
        @{ tool = 'create_successor'; arguments = @{ at = $key; kind = 'direct'; title = '所有权决定剩余控制权'; body = '§1 - 所有权' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'direct'; title = '债务把控制权交给债权人'; body = '§2 - 债务' } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; type = 'model'; level = 'concept'; title = '不完全契约模型'; body = "问题：谁拥有资产？`n`n来源：[[$key]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = "$key/2"; kind = 'side'; type = 'event'; title = '1926 年通用汽车收购费雪车身公司'; body = "时间：1926 年。`n来源：[[$key]] §2" } }
    )
    $model = "$key/1|a"
    $event = "$key/2|a"
    $a = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '剩余控制权'; body = '旧正文。' } }))[0] -replace '/0$', ''
    $b = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '剩余控制权的配置'; body = '旧正文。' } }))[0] -replace '/0$', ''
    $aCards = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$a/0"; title = '所有者在空白处做决定'; body = "依赖 [[$key/1]]：契约不完全。`n`n来源：[[$key]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '控制权带来谈判地位'; body = "依赖 [[@@0]]：所有者可以拒绝。`n`n来源：[[$key]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; kind = 'side'; type = 'hub'; level = 'topic'; title = '剩余控制权的入口'; body = "## 入口`n`n- [[@@0]]：定义。" } }
    )
    $hub = $aCards[2]
    $null = Write-Calls @(@{ tool = 'edit_card'; arguments = @{ at = "$a/0"; body = "旧正文。`n`n枢纽：[[$hub]]" } })
    $bCards = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$b/0"; title = '配置决定投资激励'; body = "依赖 [[$($aCards[0])]]：控制权在谁手里。`n`n来源：[[$key]] §2" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '债权人在违约时接管'; body = "对照 [[@@0]]：控制权随状态转移。`n`n来源：[[$key]] §2" } }
    )
    Save-Json (Join-Path $staging 'manifest.json') @{ citation_key = $key; book_title = '组织测试'; groups = @(@{ n = 1; chapter = '第一章'; headings = @('所有权') }, @{ n = 2; chapter = '第二章'; headings = @('债务') }) }

    # --- survey ---
    $survey = Last @(pwsh -NoProfile -File (Join-Path $dir 'organize-survey.ps1') -Staging $staging -Work $work 2>&1)
    Assert ($survey.topics -eq 2 -and $survey.pairs -ge 1) "survey: two Topics and a duplicate pair ($($survey.topics), $($survey.pairs))"
    $md = Get-Content (Join-Path $work 'survey.md') -Raw -Encoding utf8
    Assert ($md -match "``$a/0`` 剩余控制权" -and $md -match "``$b/0`` 剩余控制权的配置") 'survey: survey.md names both Topics of the pair'

    # --- Opus stand-in: merge B into A, move a Card, and three moves that must be refused ---
    Save-Json (Join-Path $work 'decisions.json') @{
        thesis  = '剩余控制权的配置决定企业的边界与融资'
        merges  = @(@{ drop = "$b/0"; keep = "$a/0"; under = $aCards[1]; reason = '同一个概念' })
        moves   = @(
            @{ at = $bCards[1]; under = $aCards[0]; reason = '它属于定义之下' },
            @{ at = $model; under = $aCards[0]; reason = '不能移动模型卡' },
            @{ at = "$key/2"; under = $aCards[0]; reason = '不能移出文献树' },
            @{ at = "$a/0"; under = $aCards[0]; reason = '根卡只能合并' }
        )
        notes   = @(@{ address = $bCards[1]; note = '链接事件卡。' })
    }
    $applied = Last @(pwsh -NoProfile -File (Join-Path $dir 'organize-apply.ps1') -Work $work 2>&1)
    Assert ($applied.merges -eq 1 -and $applied.moves -eq 1 -and $applied.refused -eq 3) "apply: one merge, one move, three refusals ($($applied.merges), $($applied.moves), $($applied.refused))"
    Assert (-not (Get-Card "$b/0")) 'apply: the dropped Topic root is gone'
    $tree = @(Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $a } }))[0]
    $titles = @($tree.cards | ForEach-Object { [string]$_.title })
    Assert ('配置决定投资激励' -in $titles -and '债权人在违约时接管' -in $titles) 'apply: the dropped Topic''s Cards now live in the kept Topic'
    $moved = @($tree.cards | Where-Object { $_.title -eq '债权人在违约时接管' })[0]
    Assert ([string]$moved.parent -eq $aCards[0]) "apply: the moved Card sits under its new parent ($($moved.address) under $($moved.parent))"
    $log = Get-Content (Join-Path $work 'applied.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert (@($log.refused | Where-Object { $_.reason -match 'model Card' }).Count -eq 1 -and @($log.refused | Where-Object { $_.reason -match 'Literature tree' }).Count -eq 1 -and @($log.refused | Where-Object { $_.reason -match 'only by a merge' }).Count -eq 1) "apply: the refusals name their reasons ($(@($log.refused | ForEach-Object { $_.reason }) -join ' | '))"
    Assert ((Get-Card $model).title -eq '不完全契约模型') 'apply: the Model Card did not move'
    $dec = Get-Content (Join-Path $work 'decisions.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert (@($dec.notes)[0].address -eq [string]$moved.address) "apply: a note follows its Card to the new address ($(@($dec.notes)[0].address))"

    # --- skeleton ---
    $sw = Join-Path $staging 'skeleton'
    $planLines = @(pwsh -NoProfile -File (Join-Path $dir "book-skeleton.ps1") -Step plan -Staging $staging -Work $sw 2>&1)
    if ($LASTEXITCODE) { Write-Output ($planLines -join "`n") }
    $plan = Last $planLines
    Assert ($plan.groups -eq 2 -and $plan.models -eq 1 -and $plan.events -eq 1 -and $plan.topics -eq 1) "skeleton plan: 2 groups, 1 model, 1 event, 1 Topic ($($plan.groups), $($plan.models), $($plan.events), $($plan.topics))"
    $jobText = Get-Content (Join-Path $sw 'job.md') -Raw -Encoding utf8
    Assert ($jobText.Contains("SELF-CHECK: ``zt-digest skeleton -Step apply -Staging `"$staging`" -Work `"$sw`"``（不要加 -Apply）")) 'skeleton plan: the SELF-CHECK line names zt-digest skeleton, without -Apply'
    $entry = @($tree.cards | Where-Object { $_.address -eq $aCards[0] })[0].address
    Save-Json (Join-Path $sw 'out\skeleton.json') @{
        chapters = @(@{ anchor = "$key/1"; says = '所有权决定剩余控制权。' }, @{ anchor = "$key/2"; says = '债务在违约时转移控制权。' })
        argument = @(@{ anchor = "$key/1"; adds = '本部分定义剩余控制权。'; key_cards = @($entry) }, @{ anchor = "$key/2"; adds = '本部分把控制权用于融资。'; key_cards = @('99/9') })
        models   = @(@{ address = $model; shows = '所有权配置决定投资激励。'; built_on_by = @() })
        events   = @(@{ address = $event; supports = @($entry) })
        topics   = @(@{ root = "$a/0"; body = "本主题追问：谁决定契约没写的事。依赖 [[$entry]] 的定义：所有者决定。" })
    }
    $sk = Last @(pwsh -NoProfile -File (Join-Path $dir 'book-skeleton.ps1') -Step apply -Staging $staging -Work $sw -Decisions (Join-Path $work 'decisions.json') -Apply 2>&1)
    Assert ($sk.ok -and $sk.written.structure) "skeleton apply: writes the structure Card ($($sk.written.structure))"
    $structure = Get-Card $sk.written.structure
    Assert ($structure.title -eq '剩余控制权的配置决定企业的边界与融资' -and [string]$structure.parent.address -eq "$key/1") 'skeleton apply: the thesis is the title; the Card is a Side successor of the first anchor'
    Assert ($structure.body -match '## 模型' -and $structure.body -match "\[\[$([regex]::Escape($model))\]\]（概念模型）" -and $structure.body -match '## 事件') 'skeleton apply: the structure Card lists the models with their level and the events'
    Assert ($structure.body -notmatch '99/9') 'skeleton apply: a key Card that does not exist is dropped'
    $lit = Get-Card $key
    Assert ($lit.body -match "第 1 组（第一章）：所有权决定剩余控制权。 → \[\[$key/1\]\]" -and $lit.body -match '原书文档：\[原书 PDF\]\(scholar:85\)' -and $lit.body -match [regex]::Escape("[[$($sk.written.structure)]]")) 'skeleton apply: the Literature Card holds the index, the structure Link, and its document line'
    $root = Get-Card "$a/0"
    Assert ($root.body -match '本主题追问' -and $root.body -match [regex]::Escape("枢纽：[[$hub]]")) "skeleton apply: the Topic body is new and keeps its hub line ($($root.body -replace "
", ' / '))"
    $again = Last @(pwsh -NoProfile -File (Join-Path $dir 'book-skeleton.ps1') -Step apply -Staging $staging -Work $sw -Thesis '第二个论点' -Apply 2>&1)
    Assert ($again.written.structure -eq $sk.written.structure -and (Get-Card $sk.written.structure).title -eq '第二个论点') 'skeleton apply: a second run rewrites the same structure Card'

    # The audit reads the structure Card as its own category, not as an Idea Card.
    $out = Join-Path $scratch 'audit'
    $null = @(pwsh -NoProfile -File (Join-Path $dir 'digest-audit.ps1') -CitationKey $key -Out $out 2>&1)
    $rep = Get-Content (Join-Path $out 'audit.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert (@($rep.books)[0].cards_by_category.structure -eq 1) 'audit: the structure Card is its own category'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
