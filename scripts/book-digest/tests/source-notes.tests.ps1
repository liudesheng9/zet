#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\source-notes.tests.ps1 [-Digest <zt-digest.exe>]
# Drives the chapter-endnote preparation (DG19, #80) through its real command boundary on a scratch
# archive: `prepare-book-digest.ps1` -> `zt-digest prepare` on a bilingual book whose 注释 section
# stands outside the digest range, and `zt-digest source-faults` / `zt-digest model -Step validate`
# for the retained source-fault report. The notes are recorded in the manifest with their original
# source line spans, hashes and note ids, reach each group file and model context pack, and never
# create a group or a Card. The live archive is not touched.
param([string]$Digest = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "source-notes-test-$PID"
$scratch = [IO.Path]::GetFullPath($scratch)
$testTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($testTempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch path escaped the intended test temp directory' }
$archive = Join-Path $scratch 'archive'
New-Item -ItemType Directory -Force $archive | Out-Null
# One build for the direct tool calls and for the forwarders (`ZT_DIGEST`): -Digest, else the
# environment's, else the installed one.
$digestExe = if ($Digest) { (Resolve-Path -LiteralPath $Digest).Path }
    elseif ($env:ZT_DIGEST) { (Resolve-Path -LiteralPath $env:ZT_DIGEST).Path }
    else { (Get-Command zt-digest -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source }
$env:ZT_DIGEST = $digestExe
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$here = Join-Path $PSScriptRoot '..' | Resolve-Path | ForEach-Object Path
. (Join-Path $here 'zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Invoke-Script($name, [string[]]$arguments) {
    $lines = @(pwsh -NoProfile -File (Join-Path $here $name) @arguments 2>&1 | ForEach-Object { "$_" })
    [pscustomobject]@{ code = $LASTEXITCODE; lines = $lines; last = @($lines)[-1] }
}
function Invoke-Digest([string[]]$arguments) {
    $lines = @(& $digestExe @arguments 2>&1 | ForEach-Object { "$_" })
    [pscustomobject]@{ code = $LASTEXITCODE; lines = $lines; last = @($lines)[-1] }
}

# A note long enough to stand alone: 300 characters of Chinese prose, larger than -NoteMaxChars.
$longNote = ('敲竹杠的例子说明专用性投资的收益在事后会被对方分享。' * 12)
$book = @"
# Preface

Some English preface.

# 导论

## Intro.1 为什么有企业

Firms exist because markets fail.

企业存在是因为市场失灵。

# 第一章 契约

## 1.1 不完全契约

Contracts are incomplete.

契约不完全，Hart (1995) 讨论了剩余控制权。

## 1.2 敲竹杠

Hold-up.

敲竹杠问题使专用性投资不足，参见注 [1.4]。

# 第二章 企业边界

## 2.1 边界问题

The boundary follows complementary assets.

企业边界由资产互补性决定。

# 注释

## 第一章 契约

[1.1] 契约不完全：见 Hart 1995 第二章。

[1.3] 敲竹杠：这里给出专用性投资的定义。

## 第二章 企业边界

[2.1] 资产互补性：见 Williamson 1985。

[2.2] $longNote

## 集中注释

[1.5] 跨章注释一。

[2.3] 跨章注释二。

# 参考文献

文献。
"@

try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source, $book, $utf8)
    $sourceHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant()
    $staging = Join-Path $scratch 'staging'
    $prepare = @('-Source', $source, '-Out', $staging, '-CitationKey', 'Test2021', '-BibTeX', '@book{Test2021, title={Firms and Contracts}}',
        '-BookTitle', '企业与契约', '-StartHeading', '导论', '-EndHeading', '注释', '-MinUnitChars', '20', '-MaxUnits', '1',
        '-NoFigures', '-NoteMaxChars', '200')

    # --- the notes section is extracted outside the digest range -------------------------------------
    $run = Invoke-Script 'prepare-book-digest.ps1' $prepare
    Assert ($run.code -eq 0) "prepare runs: $($run.lines -join ' | ')"
    $manifest = Get-Content -LiteralPath (Join-Path $staging 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
    $chapters = @($manifest.notes.chapters)
    Assert (($chapters | ForEach-Object { $_.chapter }) -join ' / ' -eq '导论 / 第一章 契约 / 第二章 企业边界') "the notes record every chapter of the digest in order: $(($chapters | ForEach-Object { $_.chapter }) -join ' / ')"
    Assert ($chapters[0].present -eq $false -and @($chapters[0].files).Count -eq 0) 'a chapter without endnotes has no note file'
    Assert ($chapters[1].present -eq $true -and ($chapters[1].chapter_id -eq 'c02')) "the first chapter with notes is c02: $($chapters[1].chapter_id)"
    $c02 = Join-Path $staging 'notes\c02.md'
    Assert (Test-Path -LiteralPath $c02) "the notes of 第一章 are written to $c02"
    $c02Text = Get-Content -LiteralPath $c02 -Raw -Encoding utf8
    Assert ($c02Text.Contains('[1.1] 契约不完全：见 Hart 1995 第二章。') -and $c02Text.Contains('[1.3] 敲竹杠：这里给出专用性投资的定义。')) 'the note file keeps the original note text'
    Assert (-not $c02Text.Contains('[2.1] 资产互补性')) 'a chapter note file holds only its own chapter'
    Assert (($chapters[1].note_ids -join ',') -eq '1.1,1.3') "the manifest records the chapter's note ids: $($chapters[1].note_ids -join ',')"
    $spans = @($chapters[1].spans)
    $sourceLines = [IO.File]::ReadAllText($source) -split "`r?`n"
    $recorded = ($spans | ForEach-Object { $sourceLines[($_.start_line - 1)..($_.end_line - 1)] -join "`n" }) -join "`n"
    Assert ($spans.Count -ge 1 -and $recorded.Contains('[1.1] 契约不完全') -and $recorded.Contains('[1.3] 敲竹杠')) "the manifest's original line spans point at the note text (lines $(($spans | ForEach-Object { "$($_.start_line)-$($_.end_line)" }) -join ','))"
    Assert ($chapters[1].sha256 -match '^[0-9a-f]{64}$' -and $chapters[1].sha256 -eq $chapters[1].files[0].sha256) 'the chapter notes carry their content hash'
    Assert ($manifest.source_sha256 -eq $sourceHash) 'the manifest binds the prepared source hash'
    Assert (@($manifest.groups).Count -eq 4) 'the notes create no digest group'
    Assert (@($manifest.notes.faults).Count -ge 2) 'the manifest retains the notes faults it found'
    $status = (zt status 2>&1) -join "`n"
    Assert ($status -match 'cards: 0') "the notes create no Card: $(($status -split "`n" | Where-Object { $_ -match 'cards:' }) -join ' ')"
    $bookText = Get-Content -LiteralPath (Join-Path $staging 'book.zh.md') -Raw -Encoding utf8
    Assert (-not $bookText.Contains('[1.1]') -and -not $bookText.Contains('集中注释')) 'the digest text stays the main range: the notes are not digested'

    # --- every group and model context gets its chapter's notes --------------------------------------
    $g02 = Get-Content -LiteralPath (Join-Path $staging 'groups\g02.md') -Raw -Encoding utf8
    $g03 = Get-Content -LiteralPath (Join-Path $staging 'groups\g03.md') -Raw -Encoding utf8
    $g01 = Get-Content -LiteralPath (Join-Path $staging 'groups\g01.md') -Raw -Encoding utf8
    $g04 = Get-Content -LiteralPath (Join-Path $staging 'groups\g04.md') -Raw -Encoding utf8
    Assert ($g02.Contains('## 本章注释（只读参考文本') -and $g02.Contains('[1.1] 契约不完全：见 Hart 1995 第二章。')) 'the group file carries its chapter notes as inline read-only text'
    Assert ($g03.Contains('[1.3] 敲竹杠：这里给出专用性投资的定义。')) 'another group of the same chapter carries the same chapter notes'
    Assert (-not $g01.Contains('## 本章注释')) 'a chapter without endnotes adds no notes block'
    Assert ($g02.Contains('缺失的注释：本章正文引用了注释 1.4')) 'a cited note with no definition stays visible in the group file'
    Assert ($g04.Contains('## 本章注释') -and $g04.Contains('c03-1.md') -and $g04.Contains('c03-2.md') -and $g04.Contains('注释 2.1') -and $g04.Contains('注释 2.2')) 'oversized notes are referenced by labelled part files with their note ids'
    Assert (-not $g04.Contains('敲竹杠的例子说明专用性投资的收益')) 'oversized notes are not silently truncated into the group file'
    $longNoteFile = Get-Content -LiteralPath (Join-Path $staging 'notes\c03-2.md') -Raw -Encoding utf8
    Assert ($longNoteFile.Contains('[2.2] ' + $longNote)) 'a note larger than the budget stays whole in its own labelled part'

    # --- the model context pack of a group's model carries the same chapter notes ---------------------
    $null = Invoke-Zt @(
        @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{Test2021x, title={企业与契约}}' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2021x'; title = '第一章：契约' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2021x/1'; kind = 'side'; type = 'model'; level = 'formal'; title = '契约模型：剩余控制权决定投资'; body = "问题：契约不完全时谁做决定？`n`n来源：原书第一章 1.1 节。" } }
    )
    [IO.File]::WriteAllText((Join-Path $staging 'owned-g02.txt'), "Test2021x/1|a`n", $utf8)
    $contextFile = Join-Path $staging 'models\context.md'
    New-Item -ItemType Directory -Force (Split-Path -Parent $contextFile) | Out-Null
    $context = Invoke-Digest @('model-context', '-Staging', $staging, '-Model', 'Test2021x/1|a', '-Out', $contextFile)
    Assert ($context.code -eq 0) "model-context runs: $($context.lines -join ' | ')"
    $contextText = Get-Content -LiteralPath $contextFile -Raw -Encoding utf8
    Assert ($contextText.Contains('## 本章注释（只读参考文本') -and $contextText.Contains('[1.3] 敲竹杠：这里给出专用性投资的定义。')) 'the model context pack carries its chapter notes before the draft source review'
    Assert ($contextText.Contains('缺失的注释：本章正文引用了注释 1.4')) 'the model context pack keeps the chapter missing notes visible'

    # --- the source-fault report: prepared faults, then the architects', one entry per location -----
    $report = Join-Path $staging 'source-faults.md'
    $prepared = Invoke-Digest @('source-faults', '-Staging', $staging)
    Assert ($prepared.code -eq 0 -and (Test-Path -LiteralPath $report)) "the preparation renders its source-fault report: $($prepared.lines -join ' | ')"
    $reportText = Get-Content -LiteralPath $report -Raw -Encoding utf8
    Assert ($reportText -match '集中注释' -and $reportText -match '1\.4') 'the preparation faults and the missing note are in the report'
    Assert ($reportText -notmatch 'A\.66|A\.69a|A\.53') 'the report claims no historical fault that the current evidence does not support'
    $firstHash = (Get-FileHash -LiteralPath $report -Algorithm SHA256).Hash
    $render = Invoke-Digest @('source-faults', '-Staging', $staging)
    Assert ($render.code -eq 0 -and (Get-FileHash -LiteralPath $report -Algorithm SHA256).Hash -eq $firstHash) 'the report renders deterministically from the retained record'

    # Two architects' observations at one location deduplicate into one entry that keeps the original.
    $sourceLines = [IO.File]::ReadAllText($source) -split "`r?`n"
    $a66Line = [Array]::IndexOf($sourceLines, '契约不完全，Hart (1995) 讨论了剩余控制权。') + 1
    $a53Line = [Array]::IndexOf($sourceLines, '企业边界由资产互补性决定。') + 1
    $observedA66 = $sourceLines[$a66Line - 1]
    $observedA53 = $sourceLines[$a53Line - 1]
    $attempts = @(
        @{ start_line = $a66Line; end_line = $a66Line; chapter = '第一章 契约'; equation = '(A.66)';
            observed = $observedA66; proposed = '分母应为 $1+\rho$，印刷漏了 $\rho$。'; confidence = 'high'; affected = @('Test2021/2|a') },
        @{ start_line = $a66Line; end_line = $a66Line; chapter = '第一章 契约'; equation = '(A.66)';
            observed = $observedA66; proposed = '分母应为 $1+\rho$（另一次观察同意）。'; confidence = 'medium'; affected = @('Test2021/2|b') },
        @{ start_line = $a53Line; end_line = $a53Line; chapter = '第二章 企业边界'; equation = '(A.53)';
            observed = $observedA53; proposed = '符号应为负号。'; confidence = 'low'; affected = @() }
    )
    $attemptsFile = Join-Path $scratch 'faults-a.json'
    [IO.File]::WriteAllText($attemptsFile, (@{ faults = $attempts } | ConvertTo-Json -Depth 6), $utf8)
    $recorded = Invoke-Digest @('source-faults', '-Staging', $staging, '-Json', $attemptsFile, '-Origin', 'models/b1/plan.json')
    Assert ($recorded.code -eq 0) "an architect's observations are recorded: $($recorded.lines -join ' | ')"
    $summary = $recorded.last | ConvertFrom-Json
    Assert ($summary.recorded -eq 3 -and $summary.source_faults -eq 2) "one fault record per source location: $(($summary | ConvertTo-Json -Compress))"
    $store = Get-Content -LiteralPath (Join-Path $staging 'source-faults.json') -Raw -Encoding utf8 | ConvertFrom-Json
    $fault = @($store.faults | Where-Object { $_.kind -eq 'source' -and $_.equation -eq '(A.66)' })
    Assert ($fault.Count -eq 1 -and $fault[0].observed -eq $observedA66) 'the deduplicated entry preserves the original observation'
    Assert (($fault[0].affected -join ',') -eq 'Test2021/2|a,Test2021/2|b' -and @($fault[0].attempts).Count -eq 2) "the attempts merge, keeping both affected Cards: $(($fault[0].affected -join ',')), attempts $(@($fault[0].attempts).Count)"
    $reportText = Get-Content -LiteralPath $report -Raw -Encoding utf8
    Assert ($reportText -match '\(A\.66\)' -and $reportText.Contains($observedA66) -and $reportText.Contains('Test2021/2|a') -and $reportText.Contains('Test2021/2|b')) 'the report binds the location, the observed text and every affected Card'
    Assert (-not $reportText.Contains('A.69a')) 'a fault nobody observed is still absent from the report'
    Assert ($reportText.Contains('(A.53)')) 'a fault the current source supports is reported'

    # A record that names another source, or an observation the source does not print, is refused.
    $wrong = @(@{ start_line = $a66Line; end_line = $a66Line; chapter = '第一章 契约'; equation = '(A.66)';
        observed = $observedA66; proposed = '改。'; confidence = 'high'; affected = @(); source_sha256 = ('0' * 64) })
    $wrongFile = Join-Path $scratch 'faults-wrong.json'
    [IO.File]::WriteAllText($wrongFile, ($wrong | ConvertTo-Json -Depth 5 -AsArray), $utf8)
    $beforeHash = (Get-FileHash -LiteralPath (Join-Path $staging 'source-faults.json') -Algorithm SHA256).Hash
    $refused = Invoke-Digest @('source-faults', '-Staging', $staging, '-Json', $wrongFile)
    Assert ($refused.code -ne 0 -and ($refused.lines -join ' ') -match 'source' -and (Get-FileHash -LiteralPath (Join-Path $staging 'source-faults.json') -Algorithm SHA256).Hash -eq $beforeHash) 'a fault bound to another source hash is refused without changing the record'
    $badConfidence = @(@{ start_line = $a66Line; end_line = $a66Line; chapter = '第一章 契约'; equation = '(A.66)';
        observed = $observedA66; proposed = '改。'; confidence = 'certain'; affected = @() })
    $badFile = Join-Path $scratch 'faults-confidence.json'
    [IO.File]::WriteAllText($badFile, ($badConfidence | ConvertTo-Json -Depth 5 -AsArray), $utf8)
    $bad = Invoke-Digest @('source-faults', '-Staging', $staging, '-Json', $badFile)
    Assert ($bad.code -ne 0 -and ($bad.lines -join ' ') -match 'confidence') 'a confidence outside high, medium or low is refused'

    # An observation the source does not print at its span is a visible limitation, not a fault.
    $unsupportedFile = Join-Path $scratch 'faults-unsupported.json'
    $unsupported = @(@{ start_line = $a53Line; end_line = $a53Line; chapter = '第二章 企业边界'; equation = '(A.70)';
        observed = '这一行原文里根本没有的话。'; proposed = '随便改。'; confidence = 'low'; affected = @() })
    [IO.File]::WriteAllText($unsupportedFile, ($unsupported | ConvertTo-Json -Depth 5 -AsArray), $utf8)
    $recorded = Invoke-Digest @('source-faults', '-Staging', $staging, '-Json', $unsupportedFile)
    $summary = $recorded.last | ConvertFrom-Json
    Assert ($recorded.code -eq 0 -and $summary.unsupported -eq 1) "an unverifiable observation is recorded as unsupported: $(($summary | ConvertTo-Json -Compress))"
    $reportText = Get-Content -LiteralPath $report -Raw -Encoding utf8
    Assert ($reportText.Contains('(A.70)') -and $reportText -match '未能核对|未核对') 'the unsupported observation stays visible in its own section, not as a source fault'
    $sourceHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant()
    Assert ($sourceHash -eq $manifest.source_sha256) 'nothing in this slice edits the Scholar Archive copy'

    # A verified correction is kept apart from a proposed one, and only its own evidence makes it verified.
    $verifiedFile = Join-Path $scratch 'faults-verified.json'
    $verified = @(@{ start_line = $a53Line; end_line = $a53Line; chapter = '第二章 企业边界'; equation = '(A.53)';
        observed = $observedA53; proposed = '负号是印刷错误。'; confidence = 'high'; affected = @('Test2021x/1|a');
        verified = @{ text = '原文符号应为 $-\beta$。'; evidence = '纸本第 203 页与英文版第 188 页一致。' } })
    [IO.File]::WriteAllText($verifiedFile, ($verified | ConvertTo-Json -Depth 6 -AsArray), $utf8)
    $recorded = Invoke-Digest @('source-faults', '-Staging', $staging, '-Json', $verifiedFile, '-Origin', 'review/verified.json')
    $store = Get-Content -LiteralPath (Join-Path $staging 'source-faults.json') -Raw -Encoding utf8 | ConvertFrom-Json
    $f53 = @($store.faults | Where-Object { $_.kind -eq 'source' -and $_.equation -eq '(A.53)' })
    Assert ($recorded.code -eq 0 -and $f53.Count -eq 1 -and $f53[0].status -eq 'verified' -and $f53[0].verified.evidence -and $f53[0].proposed -eq '负号是印刷错误。') 'a verified correction stays apart from its proposed interpretation in one record'
    $reportText = Get-Content -LiteralPath $report -Raw -Encoding utf8
    Assert ($reportText -match '已核实的修正（证据：纸本第 203 页' -and $reportText -match '同时保留的建议：负号是印刷错误。') 'the report distinguishes the verified correction from the proposal'
    $claimFile = Join-Path $scratch 'faults-claim.json'
    $claim = @(@{ start_line = $a53Line; end_line = $a53Line; chapter = '第二章 企业边界'; equation = '(A.53)';
        observed = $observedA53; proposed = '改。'; confidence = 'high'; affected = @(); verified = @{ text = '应为 $-\beta$。' } })
    [IO.File]::WriteAllText($claimFile, ($claim | ConvertTo-Json -Depth 6 -AsArray), $utf8)
    $claimRun = Invoke-Digest @('source-faults', '-Staging', $staging, '-Json', $claimFile)
    Assert ($claimRun.code -ne 0 -and ($claimRun.lines -join ' ') -match 'evidence') 'a verified correction without its evidence is refused'

    # Notes still inside the digest range are a preparation fault, not a silent double digest.
    $insideOut = Join-Path $scratch 'staging-inside'
    $inside = Invoke-Digest @('prepare', '-Source', $source, '-Out', $insideOut, '-CitationKey', 'Test2021i',
        '-BibTeX', '@book{Test2021i, title={Firms}}', '-StartHeading', '导论', '-EndHeading:', '-MinUnitChars', '20',
        '-MaxUnits', '1', '-NoFigures')
    Assert ($inside.code -eq 0) "prepare without an end heading runs: $($inside.lines -join ' | ')"
    $insideManifest = Get-Content -LiteralPath (Join-Path $insideOut 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
    $insideKinds = @($insideManifest.notes.faults | ForEach-Object { $_.kind })
    Assert ('notes_inside_range' -in $insideKinds -and $insideManifest.notes.section.present -eq $false) "notes inside the range are reported, not extracted twice: $($insideKinds -join ',')"
    $insideReport = Get-Content -LiteralPath (Join-Path $insideOut 'source-faults.md') -Raw -Encoding utf8
    Assert ($insideReport.Contains('注释') -and $insideReport -match 'digest range') 'the inside-range fault is in the preparation report'


    # --- the architect's plan carries the faults: the compiler boundary records them ----------------
    $work = Join-Path $staging 'models\b1'
    $modelDir = Join-Path $work 'Test2021x_1_a'
    New-Item -ItemType Directory -Force $modelDir | Out-Null
    $planFile = Join-Path $modelDir 'plan.json'
    $plan = [ordered]@{
        model = 'Test2021x/1|a'; level = 'concept'; card = '问题：契约不完全时谁做决定；结论：剩余控制权决定投资。'; parts = @()
        source_faults = @(@{ start_line = $a66Line; end_line = $a66Line; chapter = '第一章 契约'; equation = '(A.66)';
            observed = $observedA66; proposed = '第三次观察：分母漏了一项。'; confidence = 'low'; affected = @('Test2021x/1|a') })
    }
    [IO.File]::WriteAllText($planFile, ($plan | ConvertTo-Json -Depth 8), $utf8)
    $validate = Invoke-Digest @('model', '-Step', 'validate', '-Work', $work, '-Staging', $staging, '-Plan', $planFile)
    Assert ($validate.code -eq 0 -and @($validate.lines)[-1] -eq 'faults: 0') "the compiler validates the plan and still ends with faults: 0: $($validate.lines -join ' | ')"
    Assert (($validate.lines -join ' ') -match 'source-faults: 1') 'the compiler reports what it recorded'
    $store = Get-Content -LiteralPath (Join-Path $staging 'source-faults.json') -Raw -Encoding utf8 | ConvertFrom-Json
    $fault = @($store.faults | Where-Object { $_.kind -eq 'source' -and $_.equation -eq '(A.66)' })
    Assert ($fault.Count -eq 1 -and @($fault[0].attempts).Count -eq 3 -and $fault[0].observed -eq $observedA66) "the compiler records into the same deduplicated store, keeping the original observation: $(@($fault[0].attempts).Count) attempts"
    Assert ($fault[0].affected -contains 'Test2021x/1|a' -and $fault[0].confidence -eq 'low') 'the plan entry merges its Card and its latest confidence'
    # The architect validates without -Staging (as its instructions say); the run folder above -Work is found.
    $plan.source_faults = @(@{ start_line = $a53Line; end_line = $a53Line; chapter = '第二章 企业边界'; equation = '(A.53)';
        observed = $observedA53; proposed = '同一处符号应为负。'; confidence = 'medium'; affected = @('Test2021x/1|a') })
    [IO.File]::WriteAllText($planFile, ($plan | ConvertTo-Json -Depth 8), $utf8)
    $bare = Invoke-Digest @('model', '-Step', 'validate', '-Work', $work, '-Plan', $planFile)
    $store = Get-Content -LiteralPath (Join-Path $staging 'source-faults.json') -Raw -Encoding utf8 | ConvertFrom-Json
    Assert ($bare.code -eq 0 -and @($bare.lines)[-1] -eq 'faults: 0' -and @($store.faults | Where-Object { $_.equation -eq '(A.53)' }).Count -eq 1) 'a validate without -Staging still finds the prepared run folder above -Work'
    # A malformed entry is a plan fault the architect must fix, and it is not recorded.
    $plan.source_faults = @(@{ start_line = $a66Line; end_line = $a66Line; chapter = '第一章 契约'; equation = '(A.66)';
        observed = $observedA66; proposed = '改。'; confidence = 'certain'; affected = @() })
    [IO.File]::WriteAllText($planFile, ($plan | ConvertTo-Json -Depth 8), $utf8)
    $broken = Invoke-Digest @('model', '-Step', 'validate', '-Work', $work, '-Staging', $staging, '-Plan', $planFile)
    Assert ($broken.code -eq 0 -and @($broken.lines)[-1] -eq 'faults: 1' -and ($broken.lines -join ' ') -match 'source_faults: .*confidence') "a malformed source fault is a plan fault: $($broken.lines -join ' | ')"
    $storeAgain = Get-Content -LiteralPath (Join-Path $staging 'source-faults.json') -Raw -Encoding utf8 | ConvertFrom-Json
    Assert (@($storeAgain.faults).Count -eq @($store.faults).Count -and @($storeAgain.faults | Where-Object { $_.equation -eq '(A.66)' })[0].confidence -eq 'low') 'a refused plan changes nothing in the retained record'
    $plan.source_faults = @()
    [IO.File]::WriteAllText($planFile, ($plan | ConvertTo-Json -Depth 8), $utf8)

} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
if ($failed) { Write-Output "$failed failed"; exit 1 }
Write-Output 'all passed'
