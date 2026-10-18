#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\book-preparation.tests.ps1 [-Zt <zt.exe>] [-ZtDigest <zt-digest.exe>]
# Drives `prepare -Book` (a Scholar Archive Work id or title) through prepare-book-digest.ps1 on a
# scratch archive. The Scholar Archive itself is not touched: the seam-zt-mcp fixture answers
# `scholar_archive_read` and `scholar_archive_file`, so the test proves exactly which bridge calls
# the resolution makes and what it does with them.
#   - a Work id resolves the Work, its Chinese/bilingual Markdown, its BibTeX and the heading
#     range; the preparation result and the manifest freeze them before any group work;
#   - a title resolves only on one exact match, and an ambiguous or inexact title returns the
#     candidates (with their files) instead of writing group work;
#   - several Markdown files of the same preference need -FileId, and the bilingual file wins over
#     the aligned OCR one;
#   - a given -Source, -FileId, -BibTeX or -BookTitle that disagrees with the resolution is an
#     input error; a Citation key is generated from metadata after the collision check;
#   - a bridge that cannot supply metadata or the document is a specific fault that names the
#     explicit-source fallback;
#   - explicit range arguments override the proposal, and a heading that does not exist fails;
#   - -Apply and the econ variant build the Literature tree from the resolved document.
param([string]$Zt = '', [string]$ZtDigest = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "book-preparation-test-$PID"
$scratch = [IO.Path]::GetFullPath($scratch)
$testTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($testTempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch path escaped the intended test temp directory' }
$archive = Join-Path $scratch 'archive'
New-Item -ItemType Directory -Force $archive | Out-Null
if ($Zt) {
    $bin = Join-Path $scratch 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
if ($ZtDigest) { $env:ZT_DIGEST = $ZtDigest }
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$env:ZT_SCHOLAR_FIXTURE = Join-Path $scratch 'scholar-fixture.json'
$env:ZT_TRACE_TOOLS = Join-Path $scratch 'tools.jsonl'
$env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
$here = Join-Path $PSScriptRoot '..' | Resolve-Path | ForEach-Object Path
. (Join-Path $here 'zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Invoke-Prepare([string[]]$arguments) {
    $lines = @(pwsh -NoProfile -File (Join-Path $here 'prepare-book-digest.ps1') @arguments 2>&1 | ForEach-Object { "$_" })
    $code = $LASTEXITCODE
    $result = $null
    try { $result = @($lines)[-1] | ConvertFrom-Json -DateKind String } catch { }
    [pscustomobject]@{ code = $code; lines = $lines; last = @($lines)[-1]; result = $result }
}
function Reset-Trace { [IO.File]::WriteAllText($env:ZT_TRACE_TOOLS, '', $utf8) }
function Trace { @(Get-Content -LiteralPath $env:ZT_TRACE_TOOLS -Encoding utf8 -ErrorAction SilentlyContinue | Where-Object { $_ } | ForEach-Object { $_ | ConvertFrom-Json }) }
function Assert-NoGroupWork($staging, $label) {
    Assert (-not (Test-Path (Join-Path $staging 'manifest.json')) -and -not (Test-Path (Join-Path $staging 'book.zh.md')) -and
        -not (Test-Path (Join-Path $staging 'groups')) -and -not (Test-Path (Join-Path $staging 'sections')) -and
        -not (Test-Path (Join-Path $staging 'setup-calls.json'))) "$label writes no group work"
}
function Save-Fixture([hashtable]$fixture) {
    [IO.File]::WriteAllText($env:ZT_SCHOLAR_FIXTURE, ($fixture | ConvertTo-Json -Depth 20), $utf8)
}

# A bilingual book: each English block is followed by its Chinese translation, so prepare keeps the
# Chinese. Preface is front matter, 习题 is skipped, 参考文献 is back matter.
$book = @'
# Preface

Some preface.

# 导论

## Intro.1 Why firms

Firms exist because markets fail.

企业存在是因为市场失灵。1937 年科斯提出了这个问题。

# 第一章 契约

## 1.1 不完全契约

Contracts are incomplete.

契约不完全，Hart (1995) 讨论了剩余控制权。

## 习题

练习一。

# 第二章 市场

## 2.1 市场失灵

Markets fail.

市场失灵需要企业。

# 参考文献

文献。
'@
# The same structure for the second book, with its own author and year.
$book21 = @'
# 前言

前言材料。

# 导论

## 导论一

第一章的内容。

这是第二本书的导论。

# 第一章 主题

## 1.1 小节

正文内容。

这是第二本书的第一章。

# 参考文献

文献。
'@
$library = Join-Path $scratch 'library'
New-Item -ItemType Directory -Force $library | Out-Null
$doc20 = Join-Path $library 'w20.zh-bilingual.md'
$aligned20 = Join-Path $library 'w20_aligned.md'
$doc21 = Join-Path $library 'w21.zh-bilingual.md'
[IO.File]::WriteAllText($doc20, $book, $utf8)
[IO.File]::WriteAllText($aligned20, ($book -replace '企业存在是因为市场失灵', '企业替代市场'), $utf8)
[IO.File]::WriteAllText($doc21, $book21, $utf8)

$works = @{
    '20' = @{ id = 20; title = 'Democracy does cause growth'; authors = 'D Acemoglu, S Naidu, P Restrepo, JA Robinson'; year = 2019; venue = 'NBER Working Papers'; doi = $null; source_fields = @{ author = 'Daron Acemoglu and Suresh Naidu and Pascual Restrepo and James A Robinson' } }
    '5445' = @{ id = 5445; title = 'Democracy does cause growth'; authors = 'D Acemoglu, S Naidu'; year = 2019; venue = 'NBER Working Papers'; doi = $null }
    '21' = @{ id = 21; title = '第二本书'; authors = '张三, 李四'; year = 2021; venue = '示例出版社'; doi = $null }
    '22' = @{ id = 22; title = 'Only a PDF'; authors = 'R Author'; year = 2001; venue = 'Some Press'; doi = $null }
    '23' = @{ id = 23; title = 'Unreadable files'; authors = 'R Author'; year = 2002; venue = 'Some Press'; doi = $null }
}
$search = @{
    'democracy does cause growth' = @(20, 5445)
    'democracy' = @(20, 5445)
    '第二本书' = @(21)
    'the missing book' = @()
}
$files = @{
    '20' = @(
        @{ file_id = 43; kind = 'pdf'; relative_path = 'nber/w20.pdf'; display_role = 'original_pdf'; shown = $true },
        @{ file_id = 44; kind = 'markdown'; relative_path = 'nber/w20_raw_ocr.md'; display_role = 'ocr_markdown'; shown = $false; availability = 'missing' },
        @{ file_id = 41; relative_path = 'nber/w20.zh-bilingual.md'; display_role = 'chinese_bilingual_markdown'; shown = $true },
        @{ file_id = 42; relative_path = 'nber/w20_aligned.md'; display_role = 'ocr_markdown'; shown = $false }
    )
    '5445' = @(@{ file_id = 50; relative_path = 'nber/w5445.md'; display_role = 'ocr_markdown'; shown = $true })
    '21' = @(
        @{ file_id = 51; kind = 'pdf'; relative_path = 'press/w21.pdf'; display_role = 'original_pdf'; shown = $true },
        @{ file_id = 52; relative_path = 'press/w21_aligned.md'; display_role = 'ocr_markdown'; shown = $false },
        @{ file_id = 53; relative_path = 'press/w21.zh-bilingual.md'; display_role = 'ocr_markdown'; shown = $false }
    )
    '22' = @(@{ file_id = 54; kind = 'pdf'; relative_path = 'press/w22.pdf'; display_role = 'original_pdf'; shown = $true })
    '23' = @(@{ file_id = 55; relative_path = 'press/w23.md'; display_role = 'ocr_markdown'; shown = $true })
}
$documents = @{
    '41' = @{ name = 'w20.zh-bilingual.md'; relative_path = 'nber/w20.zh-bilingual.md'; path = $doc20 }
    '42' = @{ name = 'w20_aligned.md'; relative_path = 'nber/w20_aligned.md'; path = $aligned20 }
    '44' = @{ name = 'w20_raw_ocr.md'; relative_path = 'nber/w20_raw_ocr.md'; path = $doc20 }
    '52' = @{ name = 'w21_aligned.md'; relative_path = 'press/w21_aligned.md'; path = $doc21 }
    '53' = @{ name = 'w21.zh-bilingual.md'; relative_path = 'press/w21.zh-bilingual.md'; path = $doc21 }
    '55' = @{ name = 'w23.md'; relative_path = 'press/w23.md'; path = $doc21 }
    '56' = @{ name = 'w21_notes.md'; relative_path = 'press/w21_notes.md'; path = $doc21 }
}
$fixture = @{ works = $works; search = $search; files = $files; documents = $documents; errors = @{} }
$bookSha = (Get-FileHash -LiteralPath $doc20 -Algorithm SHA256).Hash.ToLowerInvariant()

try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    Save-Fixture $fixture

    # --- a Work id resolves the whole book ---------------------------------------------------------
    Reset-Trace
    $staging = Join-Path $scratch 'staging'
    $run = Invoke-Prepare @('-Book', '20', '-Out', $staging, '-CitationKey', 'TestBook', '-MaxUnits', '1', '-MinUnitChars', '20', '-NoFigures')
    Assert ($run.code -eq 0) "a Work id prepares the book: $($run.lines -join ' | ')"
    $manifest = $run.result
    $preparationPath = Join-Path $staging 'preparation.json'
    $preparation = Get-Content -LiteralPath $preparationPath -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String
    Assert ($manifest.citation_key -eq 'TestBook' -and $manifest.file_id -eq 41 -and $manifest.source -eq $doc20) 'the manifest names the resolved key, file and source'
    Assert ($preparation.status -eq 'ready' -and $preparation.work.id -eq 20 -and $preparation.file.file_id -eq 41 -and $preparation.file.preference -eq 'chinese_bilingual_markdown') "the preparation result resolves the Work and its bilingual Markdown: $($preparation.file.preference)"
    Assert ($preparation.headings.start.heading -eq '导论' -and $preparation.headings.end.heading -eq '参考文献' -and ($preparation.headings.skip -join ',') -eq '习题') "the proposal starts after the front matter, ends at the back matter, and skips 习题: $(($preparation.headings.top | ForEach-Object { $_.heading }) -join ' / ')"
    $excluded = @($preparation.headings.excluded | ForEach-Object { "$($_.heading):$($_.disposition)" }) -join ','
    Assert ($excluded -eq 'Preface:before_start,习题:skipped,参考文献:after_end') "every excluded section is persisted with its reason: $excluded"
    Assert ($preparation.headings.source_sha256 -eq $bookSha -and $preparation.frozen.source_sha256 -eq $bookSha) 'the source hash is frozen in the preparation result'
    Assert ($preparation.citation.key -eq 'TestBook' -and $preparation.citation.source -eq 'given' -and $preparation.citation.entry_type -eq 'book') 'a given Citation key is the selected entry key'
    Assert ($preparation.citation.bibtex -like '@book{TestBook,*' -and $preparation.citation.bibtex.Contains('author = {Daron Acemoglu and Suresh Naidu and Pascual Restrepo and James A Robinson}') -and $preparation.citation.bibtex.Contains('publisher = {NBER Working Papers}')) 'BibTeX is generated from the saved metadata with the selected key'
    Assert ($manifest.book_resolution.headings.excluded -eq 3 -and $manifest.book_resolution.preparation -eq $preparationPath) 'the manifest keeps the compact resolution record and points at the preparation result'
    Assert ($manifest.groups.Count -eq 3) "the book splits into three groups: $($manifest.groups.Count)"
    $zh = Get-Content -LiteralPath (Join-Path $staging 'book.zh.md') -Raw -Encoding utf8
    $text = Get-Content -LiteralPath $manifest.shared_prefix -Raw -Encoding utf8
    Assert ($zh.Contains('企业存在是因为市场失灵') -and -not $zh.Contains('Firms exist') -and -not $zh.Contains('练习一') -and $zh.Contains('市场失灵需要企业')) 'book.zh.md keeps the Chinese, drops the English, the skipped 习题 and the back matter'
    Assert ($text.Contains('[[TestBook]]') -and $text.Contains('《Democracy does cause growth》')) 'the shared prefix names the resolved key and title'
    $trace = Trace
    $scholarTrace = @($trace | Where-Object { $_.tool -in @('scholar_archive_read', 'scholar_archive_file') })
    Assert ((@($scholarTrace | Where-Object { $_.scholar_tool -eq 'get_work' -and $_.work_id -eq 20 }).Count -eq 1) -and (@($scholarTrace | Where-Object { $_.scholar_tool -eq 'list_work_files' }).Count -eq 1) -and (@($scholarTrace | Where-Object { $_.tool -eq 'scholar_archive_file' -and $_.file_id -eq 41 }).Count -eq 1)) 'resolution reads the Work, its file list and the one document through the read-only bridge'
    Assert (@($trace | Where-Object { $_.tool -in @('create_literature', 'create_successor', 'create_topic', 'apply_batch', 'edit_card') }).Count -eq 0) 'a preparation without -Apply writes no Card'
    Assert (((@($trace | ForEach-Object { $_.tool }) | Sort-Object -Unique) -join ',') -eq 'get_card,scholar_archive_file,scholar_archive_read') 'resolution uses only the read-only bridge and the Citation-key probe, never a Scholar Archive write'

    # --- one exact title match resolves, as a Work id does ---------------------------------------
    $oneTitle = Invoke-Prepare @('-Book', '第二本书', '-Out', (Join-Path $scratch 'onetitle'), '-CitationKey', 'OneTitle', '-MinUnitChars', '20', '-NoFigures')
    Assert ($oneTitle.code -eq 0 -and $oneTitle.result.book_resolution.work.id -eq 21 -and $oneTitle.result.book_resolution.requested.kind -eq 'title') 'one exact title match resolves the Work'

    # --- the generated key and the collision check -------------------------------------------------
    $generated = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'generated'), '-MaxUnits', '1', '-MinUnitChars', '20', '-NoFigures')
    $generatedPreparation = Get-Content -LiteralPath (Join-Path $scratch 'generated\preparation.json') -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String
    Assert ($generated.code -eq 0 -and $generated.result.citation_key -eq 'Acemoglu2019' -and $generatedPreparation.citation.source -eq 'generated') "the key is generated from the first author and the year: $($generated.result.citation_key)"
    Assert ($generatedPreparation.citation.bibtex.StartsWith('@book{Acemoglu2019,')) 'the generated key is the generated entry key'
    $null = Invoke-Zt @(@{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{Acemoglu2019, title={A taken key}}'; body = '已占用。' } })
    $clash = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'clash'), '-NoFigures')
    Assert ($clash.code -eq 1 -and ($clash.lines -join ' ') -match 'Citation key `Acemoglu2019` already names a Card') 'a generated key that is taken is refused after the collision check'
    $other = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'clash2'), '-CitationKey', 'TestBook', '-MinUnitChars', '20', '-NoFigures')
    Assert ($other.code -eq 0 -and $other.result.citation_key -eq 'TestBook') 'another Citation key prepares beside a taken one'
    $badKey = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'clash3'), '-CitationKey', '1Bad Key', '-NoFigures')
    Assert ($badKey.code -eq 1 -and ($badKey.lines -join ' ') -match 'invalid Citation key') 'a Citation key the archive would refuse is an input error'

    # --- an ambiguous or inexact title returns candidates, not group work --------------------------
    $ambiguous = Join-Path $scratch 'ambiguous'
    $byTitle = Invoke-Prepare @('-Book', 'Democracy does cause growth', '-Out', $ambiguous, '-NoFigures')
    $result = $byTitle.result
    Assert ($byTitle.code -eq 3 -and $result.status -eq 'needs_selection' -and $result.selections[0].reason -eq 'ambiguous_title') "two Works with the title are a selection: $($result.selections[0].message)"
    Assert ((@($result.selections[0].candidates | ForEach-Object { $_.id }) -join ',') -eq '20,5445' -and @($result.selections[0].candidates[0].files).Count -eq 4) 'the candidates name their ids and files'
    Assert ((Get-Content -LiteralPath (Join-Path $ambiguous 'preparation.json') -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String).status -eq 'needs_selection') 'the read-only preparation result is persisted'
    Assert-NoGroupWork $ambiguous 'an ambiguous title'
    $inexact = Invoke-Prepare @('-Book', 'democracy', '-Out', (Join-Path $scratch 'inexact'), '-NoFigures')
    Assert ($inexact.code -eq 3 -and $inexact.result.selections[0].message -match 'no Work is titled exactly') 'an inexact title lists the matches instead of picking one'
    $missing = Invoke-Prepare @('-Book', 'the missing book', '-Out', (Join-Path $scratch 'missing'), '-NoFigures')
    Assert ($missing.code -eq 1 -and ($missing.lines -join ' ') -match 'has no Work matching `the missing book`' -and ($missing.lines -join ' ') -match '-Source') 'an unmatched title is a fault that names the explicit-source fallback'

    # --- several suitable Markdown files need a selection; the bilingual one wins -------------------
    $severalFixture = $fixture.Clone()
    $severalFixture.files = @{} + $files
    $severalFixture.files['21'] = @(
        @{ file_id = 51; kind = 'pdf'; relative_path = 'press/w21.pdf'; display_role = 'original_pdf'; shown = $true },
        @{ file_id = 52; relative_path = 'press/w21_aligned.md'; display_role = 'ocr_markdown'; shown = $false },
        @{ file_id = 56; relative_path = 'press/w21_notes.md'; display_role = 'ocr_markdown'; shown = $false }
    )
    $bilingual = Invoke-Prepare @('-Book', '21', '-Out', (Join-Path $scratch 'bilingual'), '-CitationKey', 'Second2021', '-MinUnitChars', '20', '-NoFigures')
    Assert ($bilingual.code -eq 0 -and $bilingual.result.file_id -eq 53 -and (Get-Content -LiteralPath (Join-Path $scratch 'bilingual\preparation.json') -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String).file.preference -eq 'bilingual_name') 'a bilingual file name wins over the aligned OCR Markdown'
    Save-Fixture $severalFixture
    $several = Invoke-Prepare @('-Book', '21', '-Out', (Join-Path $scratch 'several'), '-NoFigures')
    Assert ($several.code -eq 3 -and $several.result.selections[0].reason -eq 'several_markdown_files') "two Markdown files of the same preference are a selection: $($several.result.selections[0].message)"
    Assert (((@($several.result.selections[0].candidates | Where-Object { $_.selected } | ForEach-Object { $_.file_id }) -join ',')) -eq '52,56') 'the selection names the files of the best preference'
    Assert-NoGroupWork (Join-Path $scratch 'several') 'several Markdown files'
    $chosen = Invoke-Prepare @('-Book', '21', '-Out', (Join-Path $scratch 'chosen'), '-CitationKey', 'Second2021', '-FileId', '56', '-MinUnitChars', '20', '-NoFigures')
    Assert ($chosen.code -eq 0 -and $chosen.result.file_id -eq 56) '-FileId selects one of the offered files'
    $notMarkdown = Invoke-Prepare @('-Book', '21', '-Out', (Join-Path $scratch 'notmarkdown'), '-FileId', '51', '-NoFigures')
    Assert ($notMarkdown.code -eq 1 -and ($notMarkdown.lines -join ' ') -match 'is not a present Markdown document') '-FileId of the PDF is refused'
    $foreign = Invoke-Prepare @('-Book', '21', '-Out', (Join-Path $scratch 'foreign'), '-FileId', '41', '-NoFigures')
    Assert ($foreign.code -eq 1 -and ($foreign.lines -join ' ') -match "file 41 is not one of Work 21's linked present files") '-FileId of another Work is refused'
    Save-Fixture $fixture

    # --- explicit values that disagree with the resolution are input errors ------------------------
    $sameSource = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'samesource'), '-CitationKey', 'SameSource', '-Source', $doc20, '-MinUnitChars', '20', '-NoFigures')
    Assert ($sameSource.code -eq 0 -and $sameSource.result.source -eq $doc20) 'the same -Source as the resolved document is accepted'
    $wrongSource = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'wrongsource'), '-CitationKey', 'WrongSource', '-Source', $aligned20, '-NoFigures')
    Assert ($wrongSource.code -eq 1 -and ($wrongSource.lines -join ' ') -match 'is not the Markdown of Work 20') 'a different -Source is an input error'
    $sameBibtex = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'samebibtex'), '-CitationKey', 'TestBook', '-BibTeX', "@book{TestBook, author={Daron Acemoglu and Suresh Naidu and Pascual Restrepo and James A Robinson}, title={Democracy does cause growth}, year={2019}, publisher={NBER Working Papers}}", '-BookTitle', 'Democracy does cause growth', '-MinUnitChars', '20', '-NoFigures')
    Assert ($sameBibtex.code -eq 0) 'the entry the resolution generates is accepted as -BibTeX, in any whitespace'
    $wrongBibtex = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'wrongbibtex'), '-CitationKey', 'TestBook', '-BibTeX', '@book{TestBook, title={Another book}}', '-NoFigures')
    Assert ($wrongBibtex.code -eq 1 -and ($wrongBibtex.lines -join ' ') -match 'differs from the entry Work 20') 'a different -BibTeX is an input error'
    $wrongTitle = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'wrongtitle'), '-CitationKey', 'TestBook', '-BookTitle', '另一本书', '-NoFigures')
    Assert ($wrongTitle.code -eq 1 -and ($wrongTitle.lines -join ' ') -match "differs from Work 20's title") 'a different -BookTitle is an input error'

    # --- explicit range arguments override the proposal --------------------------------------------
    $range = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'range'), '-CitationKey', 'RangeBook', '-StartHeading', '第一章 契约', '-EndHeading', '第二章 市场', '-SkipHeadings', '习题', '-MinUnitChars', '20', '-NoFigures')
    $rangePreparation = Get-Content -LiteralPath (Join-Path $scratch 'range\preparation.json') -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String
    Assert ($range.code -eq 0 -and $rangePreparation.headings.start.explicit -and $rangePreparation.headings.start.heading -eq '第一章 契约' -and $rangePreparation.headings.end.heading -eq '第二章 市场') 'explicit range arguments become the frozen choice'
    Assert ((@($rangePreparation.headings.excluded | ForEach-Object { "$($_.heading):$($_.disposition)" }) -join ',') -eq 'Preface:before_start,导论:before_start,Intro.1 Why firms:before_start,习题:skipped,第二章 市场:after_end,2.1 市场失灵:after_end,参考文献:after_end') "the excluded sections follow the chosen range: $(@($rangePreparation.headings.excluded | ForEach-Object { "$($_.heading):$($_.disposition)" }) -join ',')"
    $noHeading = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'noheading'), '-CitationKey', 'NoHeadingBook', '-StartHeading', '第九章 不存在', '-NoFigures')
    Assert ($noHeading.code -eq 1 -and ($noHeading.lines -join ' ') -match 'no heading "第九章 不存在" found') 'an explicit heading the document does not have is a fault'

    # --- the bridge cannot supply metadata or the document ------------------------------------------
    $noWork = Invoke-Prepare @('-Book', '999', '-Out', (Join-Path $scratch 'nowork'), '-NoFigures')
    Assert ($noWork.code -eq 1 -and ($noWork.lines -join ' ') -match 'has no Work 999' -and ($noWork.lines -join ' ') -match '-Source') 'a missing Work id is a specific fault with the explicit fallback'
    $noMarkdown = Invoke-Prepare @('-Book', '22', '-Out', (Join-Path $scratch 'nomarkdown'), '-NoFigures')
    Assert ($noMarkdown.code -eq 1 -and ($noMarkdown.lines -join ' ') -match 'has no present Markdown document' -and ($noMarkdown.lines -join ' ') -match 'w22.pdf') 'a Work with only a PDF names the files and the fallback'
    $brokenList = $fixture.Clone()
    $brokenList.errors = @{ work_files = @(23) }
    Save-Fixture $brokenList
    $listFault = Invoke-Prepare @('-Book', '23', '-Out', (Join-Path $scratch 'listfault'), '-NoFigures')
    Assert ($listFault.code -eq 1 -and ($listFault.lines -join ' ') -match 'list_work_files') 'an unavailable file list is a specific bridge fault'
    $brokenDocument = $fixture.Clone()
    $brokenDocument.errors = @{ file = @(41) }
    Save-Fixture $brokenDocument
    $documentFault = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'documentfault'), '-CitationKey', 'DocFault', '-NoFigures')
    Assert ($documentFault.code -eq 1 -and ($documentFault.lines -join ' ') -match 'cannot deliver file 41 of Work 20' -and ($documentFault.lines -join ' ') -match '-Source') 'an undeliverable document is a specific fault with the explicit fallback'
    Save-Fixture $fixture

    # --- -Apply and the variant build the Literature tree from the resolved document ---------------
    $applied = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'applied'), '-CitationKey', 'AppliedBook', '-MaxUnits', '1', '-MinUnitChars', '20', '-NoFigures', '-Apply')
    Assert ($applied.code -eq 0 -and @($applied.result.created).Count -eq 4 -and $applied.result.created[0] -eq '0 ok AppliedBook') "the resolved book -Apply creates the Literature tree and its anchors: $($applied.result.created -join ', ')"
    $anchor = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = 'AppliedBook/3' } })
    Assert ($anchor.title -eq '第 3 组：第二章 市场；2.1 市场失灵') "the anchor names the resolved group: $($anchor.title)"
    $variant = Invoke-Prepare @('-Book', '20', '-Out', (Join-Path $scratch 'variant'), '-CitationKey', 'VariantBook', '-MaxUnits', '1', '-MinUnitChars', '20', '-Variant', 'econ', '-NoFigures', '-Apply')
    Assert ($variant.code -eq 0 -and $variant.result.run_contract.source.file_id -eq 41 -and $variant.result.run_contract.source.sha256 -eq $bookSha) 'the variant contract freezes the resolved file id and source hash'
    Assert (Test-Path (Join-Path $scratch 'variant\ledger.sqlite3')) 'the variant registers its preparation in the ledger'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
if ($failed) { Write-Output "$failed failed"; exit 1 }
Write-Output 'all passed'
