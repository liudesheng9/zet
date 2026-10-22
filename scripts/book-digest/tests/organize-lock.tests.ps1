#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\organize-lock.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# Tree Lock S16 (#139): the Organize stage reads a locked tree normally, previews the complete
# structural effect of every merge and move (mapping, into re-pointing, Side compaction), keeps a
# locked member and a pinned target where they are with a typed card_locked refusal, reports a
# decision that depends on a blocked one apart from a decision checked on its own facts, and stops
# the affected write when the owner locks a tree mid-run while it keeps the receipts it already
# committed. A scratch archive with its own service; every Lock is a real owner Lock taken through
# the GUI API. The live archive is never touched.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "organize-lock-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the temp directory' }
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
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$dir = Join-Path $PSScriptRoot '..' | Resolve-Path | ForEach-Object Path
. (Join-Path $dir 'zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Invoke-Tool([string[]]$arguments) {
    $lines = @(& $Digest @arguments 2>&1 | ForEach-Object { "$_" })
    [pscustomobject]@{ code = $LASTEXITCODE; lines = $lines; last = @($lines)[-1]; text = ($lines -join ' ') }
}
function Last($lines) { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable }
function Write-Calls($calls) {
    $out = @(($calls | ConvertTo-Json -Depth 8 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "zt: $out" }
    , @($out | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Get-Card($at) { try { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] } catch { $null } }
function Save-Json($path, $value) { New-Item -ItemType Directory -Force (Split-Path -Parent $path) | Out-Null; [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 10), $utf8) }
# The GUI of this scratch service: its URL and the token its page carries.
function Get-Gui {
    $line = @(@(zt gui 2>&1) | Where-Object { $_ -match '^zt gui: http' })[0]
    if (-not $line) { throw 'zt gui printed no URL' }
    $url = (($line -replace '^zt gui: ', '') -replace '/+$', '')
    $page = (Invoke-WebRequest -UseBasicParsing -Uri $url).Content
    $token = [regex]::Match($page, 'name="zt-token" content="([^"]+)"').Groups[1].Value
    if (-not $token) { throw 'the served page carries no token' }
    [pscustomobject]@{ url = $url; token = $token }
}
function Invoke-Gui($gui, [string]$path, $body) {
    Invoke-RestMethod -Method Post -Uri "$($gui.url)$path" -Headers @{ 'X-ZT-Token' = $gui.token } -ContentType 'application/json' -Body ($body | ConvertTo-Json -Depth 8)
}
function Lock-Tree([string]$root, [string]$note = 'reviewed') {
    $gui = Get-Gui
    $plan = Invoke-Gui $gui '/api/lock' @{ root = $root; note = $note }
    if (-not $plan.plan.plan_hash) { throw "the lock plan carries no plan_hash: $($plan | ConvertTo-Json -Depth 8 -Compress)" }
    $done = Invoke-Gui $gui '/api/lock' @{ root = $root; note = $note; confirm = 'lock'; plan_hash = $plan.plan.plan_hash }
    if (-not $done.applied) { throw "the owner lock was not applied: $($done | ConvertTo-Json -Depth 8 -Compress)" }
    $done.lock
}
function Blocked-For($applied, [string]$decision, [string]$address) {
    @($applied.blocked | Where-Object { $_.decision -eq $decision -and ($_.at -eq $address -or $_.drop -eq $address) })[0]
}

try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $key = 'OrgLock2024'
    $null = Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@book{$key, title={Organize Under Lock}, author={Hart, Oliver}, year={2024}}"; body = '旧索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = $key; kind = 'direct'; title = '所有权决定剩余控制权'; body = '§1 - 所有权' } }
    )
    Save-Json (Join-Path $staging 'manifest.json') @{ citation_key = $key; book_title = '锁定下的整理'; groups = @(@{ n = 1; chapter = '第一章'; headings = @('所有权') }) }

    # --- the workspaces the decisions act on ----------------------------------------------------
    $a = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '剩余控制权'; body = '本主题追问：谁在契约空白处做决定。' } }))[0] -replace '/0$', ''
    $b = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '剩余控制权的配置'; body = '本主题追问：控制权怎样配置。' } }))[0] -replace '/0$', ''
    $d = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '要合并的主题'; body = '本主题追问：这个主题会不会合并。' } }))[0] -replace '/0$', ''
    $e = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '开放的主题'; body = '本主题追问：这张卡能不能移动。' } }))[0] -replace '/0$', ''
    $locked = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '已锁定的主题'; body = '本主题追问：锁定的树怎样参与整理。' } }))[0] -replace '/0$', ''
    $lockedRoot = "$locked/0"
    $aCards = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$a/0"; title = '所有者在空白处做决定'; body = "依赖 [[$key/1]]：契约不完全。`n`n来源：[[$key]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '控制权带来谈判地位'; body = "依赖 [[@@0]]：所有者可以拒绝。`n`n来源：[[$key]] §1" } }
    )
    $bCards = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$b/0"; title = '配置决定投资激励'; body = "依赖 [[$($aCards[0])]]：控制权在谁手里。`n`n来源：[[$key]] §1" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '债权人在违约时接管'; body = "对照 [[@@0]]：控制权随状态转移。`n`n来源：[[$key]] §1" } }
    )
    $dCards = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$d/0"; title = '合并会搬走这张卡'; body = "依赖 [[$($bCards[0])]]：合并的对象。`n`n来源：[[$key]] §1" } }
    )
    $eCards = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$e/0"; title = '这张卡会移动到别的主题'; body = "依赖 [[$($aCards[0])]]：目标主题。`n`n来源：[[$key]] §1" } }
    )
    # A Card free of any pin: the plan may move it into the locked tree, and the Lock decides.
    $f = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '要移进锁定主题的主题'; body = '本主题追问：这张卡能不能移进锁定的树。' } }))[0] -replace '/0$', ''
    $fCards = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$f/0"; title = '这张卡想移进锁定的树'; body = "依赖 [[$($aCards[0])]]：目标主题。`n`n来源：[[$key]] §1" } }
    )
    # The locked tree's Card links A's second Card (pins it), the root of the Topic a merge would
    # delete (its into re-pointing would rewrite this Card), and the pinned Card of an open tree.
    $p = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '被锁定的树引用的主题'; body = '本主题追问：被锁定的树引用的卡能不能动。' } }))[0] -replace '/0$', ''
    $pCards = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$p/0"; title = '这张卡被锁定树引用'; body = '它被锁定的树引用，所以地址不能改。' } }
    )
    $lockedCards = Write-Calls @(
        @{ tool = 'create_successor'; arguments = @{ at = "$locked/0"; title = '锁定树里的引用卡'; body = "依赖 [[$($aCards[1])]]：这张卡在锁定的树里。`n`n对照 [[$d/0]]：合并会重指这里的链接。`n`n对照 [[$($pCards[0])]]：被锁定的树引用。" } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '锁定树里的第二张卡'; body = "对照 [[$key]]：第二张引用。`n`n来源：[[$key]] §1" } }
    )

    $lock = Lock-Tree $lockedRoot 'the owner reviews this tree'
    Assert ($lock.lock_id -gt 0 -and $lock.root -eq $lockedRoot) "the owner locks a whole Topic tree (lock $($lock.lock_id))"
    $pinnedBy = (Get-Card $pCards[0]).pinned_by
    Assert (@($pinnedBy).Count -eq 1 -and $pinnedBy[0] -eq $lockedRoot) "the locked tree pins the open Card: $($pinnedBy -join ', ')"

    # --- the survey reads the locked tree and marks it (R332) ------------------------------------
    $survey = Last @(pwsh -NoProfile -File (Join-Path $dir 'organize-survey.ps1') -Staging $staging -Work $work 2>&1)
    # A digest's survey names no Topic as the book's (docs/literature-only-digest-design.md); it still
    # marks every locked agent Topic as read-only context.
    Assert ($survey.topics -eq 0 -and $survey.locked -eq 1) "survey: the locked Topic is counted ($($survey.topics) Topics, $($survey.locked) locked)"
    $surveyJson = Get-Content (Join-Path $work 'survey.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($surveyJson.locks[$locked].lock_id -eq $lock.lock_id) "survey: the locked Topic carries its Lock facts ($($surveyJson.locks | ConvertTo-Json -Compress))"
    $surveyMd = Get-Content (Join-Path $work 'survey.md') -Raw -Encoding utf8
    Assert ($surveyMd -match "``$lockedRoot``" -and $surveyMd -match '已锁定') 'survey: survey.md marks the locked Topic as read-only context'

    # --- the preview: every effect of every decision, and the blocked ones typed -----------------
    Save-Json (Join-Path $work 'decisions.json') @{
        thesis = '剩余控制权的配置决定投资激励'
        merges = @(
            # The locked tree links the dropped root, and the root is a pinned target: the delete
            # would have to change a locked Card, so the whole merge waits.
            @{ drop = "$d/0"; keep = "$a/0"; reason = '合并把控制权收进同一个主题' },
            # An unlocked merge that runs.
            @{ drop = "$b/0"; keep = "$a/0"; reason = '两个主题讲同一个概念' }
        )
        moves  = @(
            @{ at = $lockedCards[0]; under = $aCards[0]; reason = '锁定树里的卡不能移动' },
            @{ at = $pCards[0]; under = $aCards[0]; reason = '被引用的卡不能移动' },
            @{ at = $fCards[0]; under = $lockedCards[0]; reason = '目标主题锁定，不能移入' },
            @{ at = $dCards[0]; under = $aCards[0]; reason = '它的主题被合并挡住' },
            @{ at = $eCards[0]; under = $aCards[0]; reason = '开放主题的卡移动到定义之下' }
        )
        notes  = @()
    }
    $before = @(Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{ workspace = 'agent' } })).titles
    $preview = Last @(& $Digest organize-apply -Work $work -DryRun -Open 2>&1 | ForEach-Object { "$_" })
    Assert ($preview.merges -eq 1 -and $preview.moves -eq 1 -and $preview.blocked -eq 5) "preview: one merge, one move, five blocked decisions ($($preview.merges), $($preview.moves), $($preview.blocked))"
    $previewFile = Get-Content (Join-Path $work 'applied.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($previewFile.dry_run -eq $true -and $previewFile.complete -eq $false) 'preview: the run is marked dry and incomplete'
    Assert (-not $previewFile.stopped) 'preview: nothing stopped the run'
    # Every decision carries its full structural effect: the final mapping, the into re-pointing and
    # the Side compaction (an organize delete is a tree root, so there is none).
    $mergePreview = @($previewFile.preview | Where-Object { $_.decision -eq 'merge' -and $_.drop -eq "$b/0" })[0]
    Assert ($null -ne $mergePreview -and $mergePreview.move.mapping.Count -ge 2 -and $mergePreview.move.to) "preview: the merging move names its full mapping ($($mergePreview.move.mapping.Count) entries to $($mergePreview.move.to))"
    Assert ($mergePreview.delete.at -eq "$b/0" -and $mergePreview.delete.into -eq "$a/0" -and $null -ne $mergePreview.delete.repointed_in -and @($mergePreview.delete.side_compaction).Count -eq 0) 'preview: the delete names its into re-pointing and its Side compaction'
    $movePreview = @($previewFile.preview | Where-Object { $_.decision -eq 'move' -and $_.at -eq $eCards[0] })[0]
    Assert ($null -ne $movePreview -and $movePreview.to -and @($movePreview.mapping.Keys).Count -eq 1 -and @($movePreview.side_compaction).Count -eq 0) "preview: the move names its mapping and Side compaction ($($movePreview.to))"
    $dMergeBlock = Blocked-For $previewFile 'merge' "$d/0"
    Assert ($null -ne $dMergeBlock -and $dMergeBlock.refusal.error -eq 'card_locked' -and $dMergeBlock.step -eq 'delete') 'preview: the merge whose delete would rewrite a locked Card is blocked with its typed refusal'
    $memberBlock = Blocked-For $previewFile 'move' $lockedCards[0]
    Assert ($memberBlock.refusal.error -eq 'card_locked' -and $memberBlock.refusal.reason -eq 'move' -and $memberBlock.refusal.tree -eq $lockedRoot -and @($memberBlock.refusal.cards) -contains $lockedCards[0]) "preview: a locked member is blocked as a move ($($memberBlock.refusal | ConvertTo-Json -Compress))"
    $pinBlock = Blocked-For $previewFile 'move' $pCards[0]
    Assert ($pinBlock.refusal.error -eq 'card_locked' -and $pinBlock.refusal.reason -eq 'pinned-move' -and $pinBlock.refusal.tree -eq $lockedRoot -and @($pinBlock.refusal.cards) -contains $pCards[0]) "preview: a pinned target is blocked as a pinned move ($($pinBlock.refusal | ConvertTo-Json -Compress))"
    $insideBlock = Blocked-For $previewFile 'move' $fCards[0]
    Assert ($insideBlock.refusal.error -eq 'card_locked' -and $insideBlock.refusal.reason -eq 'move-inside' -and $insideBlock.refusal.tree -eq $lockedRoot) "preview: a locked destination is blocked as move-inside ($($insideBlock.refusal | ConvertTo-Json -Compress))"
    $dependent = Blocked-For $previewFile 'move' $dCards[0]
    Assert ($dependent.dependent -eq $true -and $dependent.depends_on.drop -eq "$d/0") "preview: a decision inside a blocked merge is a dependent, not a checked plan ($($dependent | ConvertTo-Json -Depth 6 -Compress))"
    Assert ($dependent.refusal.blocked_by -eq 'dependent') 'preview: the dependent names the blocked decision it waits for'
    # The preview wrote nothing at all.
    $after = @(Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{ workspace = 'agent' } })).titles
    Assert ($after.Count -eq $before.Count) "preview: no Card was written ($($before.Count) before, $($after.Count) after)"
    Assert ((Get-Card "$d/0").title -eq '要合并的主题' -and (Get-Card $dCards[0]).title -eq '合并会搬走这张卡') 'preview: the blocked merge kept its Cards in place'

    # --- the real run: the permitted decisions apply, the blocked ones stay put -------------------
    $applied = Last @(& $Digest organize-apply -Work $work -Open 2>&1 | ForEach-Object { "$_" })
    Assert ($applied.merges -eq 1 -and $applied.moves -eq 1 -and $applied.blocked -eq 5) "apply: the permitted decisions run and the blocked ones are reported ($($applied.merges), $($applied.moves), $($applied.blocked))"
    $runFile = Get-Content (Join-Path $work 'applied.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($runFile.complete -eq $false -and -not $runFile.stopped) 'apply: the run is incomplete and did not stop'
    Assert (-not (Get-Card "$b/0")) 'apply: the unlocked merge deleted its dropped Topic root'
    $kept = Get-Card "$a/0"
    Assert ($kept.title -eq '剩余控制权') 'apply: the kept Topic root is unchanged'
    Assert ((Get-Card "$d/0").title -eq '要合并的主题' -and (Get-Card $dCards[0]).title -eq '合并会搬走这张卡') 'apply: the blocked merge moved and deleted nothing'
    Assert ((Get-Card $lockedCards[0]).title -eq '锁定树里的引用卡') 'apply: the locked member stayed at its address'
    Assert ((Get-Card $pCards[0]).title -eq '这张卡被锁定树引用') 'apply: the pinned target stayed at its address'
    $tree = @(Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $a } }))[0]
    Assert (@($tree.cards | Where-Object { $_.title -eq '这张卡会移动到别的主题' }).Count -eq 1) 'apply: the permitted move placed its Card'
    $log = Get-Content (Join-Path $work 'applied.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert (@($log.blocked | Where-Object { $_.refusal.reason -eq 'pinned-move' }).Count -eq 1 -and @($log.blocked | Where-Object { $_.refusal.reason -eq 'move-inside' }).Count -eq 1) 'apply: the recorded refusals keep their reasons'
    Assert ((Get-Card $lockedRoot).lock.lock_id -eq $lock.lock_id) 'apply: the Lock is still in force'

    # --- a tree locked mid-run stops the affected write and keeps the earlier receipt ------------
    $x = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '先移动的开放主题'; body = '本主题追问：中途锁定怎样停止。' } }))[0] -replace '/0$', ''
    $y = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = '后移动的开放主题'; body = '本主题追问：中途锁定怎样停止。' } }))[0] -replace '/0$', ''
    $xC = Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$x/0"; title = '先移动的卡片'; body = "依赖 [[$($aCards[0])]]：目标。`n`n来源：[[$key]] §1" } })
    $yC = Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$y/0"; title = '后移动的卡片'; body = "依赖 [[$($aCards[0])]]：目标。`n`n来源：[[$key]] §1" } })
    $raceWork = Join-Path $staging 'organize-race'
    New-Item -ItemType Directory -Force $raceWork | Out-Null
    Save-Json (Join-Path $raceWork 'decisions.json') @{
        thesis = ''
        merges = @()
        moves  = @(
            @{ at = $xC[0]; under = $aCards[0]; reason = '先移动的卡' },
            @{ at = $yC[0]; under = $aCards[0]; reason = '后移动的卡' }
        )
        notes  = @()
    }
    $seam = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $flag = Join-Path $scratch 'race-lock.flag'
    # The owner locks the destination tree after the third move_card call: the first decision's plan
    # and its write, then the second decision's plan. Its write meets the Lock that appeared between
    # the plan and the write.
    $env:ZT_CHECK_ZT = $seam
    $env:ZT_LOCK_TREE = "$a/0"
    $env:ZT_LOCK_AFTER_TOOL = 'move_card'
    $env:ZT_LOCK_AFTER_N = '3'
    $env:ZT_LOCK_FLAG = $flag
    $race = Invoke-Tool @('organize-apply', '-Work', $raceWork, '-Open')
    $env:ZT_CHECK_ZT = $null
    $env:ZT_LOCK_TREE = $null
    $env:ZT_LOCK_AFTER_TOOL = $null
    $env:ZT_LOCK_AFTER_N = $null
    $env:ZT_LOCK_FLAG = $null
    Assert ($race.code -eq 2) "mid-run: the run stops with the Lock exit code ($($race.code))"
    Assert (Test-Path -LiteralPath $flag) 'mid-run: the owner lock really happened during the run'
    $typed = @($race.lines | Where-Object { $_ -match '^\s*\{' } | ForEach-Object { try { $_ | ConvertFrom-Json } catch { } })
    $stoppedJson = @($typed | Where-Object { $_.error -eq 'card_locked' })[0]
    Assert ($null -ne $stoppedJson -and $stoppedJson.tree -eq "$a/0" -and @($stoppedJson.cards).Count -ge 1) "mid-run: the stopped run answers the typed card_locked JSON ($($race.text))"
    $raceLog = Get-Content (Join-Path $raceWork 'applied.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($raceLog.stopped -and $raceLog.stopped.refusal.error -eq 'card_locked' -and $raceLog.stopped.step -eq 'move') 'mid-run: applied.json keeps the stop as its typed refusal'
    Assert (@($raceLog.done).Count -eq 1 -and $raceLog.done[0].at -eq $xC[0]) "mid-run: the earlier move stays committed ($(@($raceLog.done).Count) done)"
    $movedNow = $raceLog.moved[$xC[0]]
    Assert ($movedNow -and (Get-Card $movedNow).title -eq '先移动的卡片' -and (Get-Card $yC[0]).title -eq '后移动的卡片') 'mid-run: only the affected write is missing from the tree'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
