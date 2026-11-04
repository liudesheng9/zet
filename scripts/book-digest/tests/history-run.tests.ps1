#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\history-run.tests.ps1
# zt-history-digest on a real scratch archive: the real `zt-digest prepare -Variant history`, the prompt the
# writer receives, the layers the ledger holds, the two SELF-CHECK commands and the group finish. It starts no
# provider. The live archive is never touched: ZT_CONFIG_DIR names a scratch box.
param([string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$OutputEncoding = $utf8
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "history-run-test-$PID"))
New-Item -ItemType Directory -Force $scratch | Out-Null
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
function Assert($condition, $message) { if (-not $condition) { throw "FAIL: $message" }; Write-Output "ok: $message" }
function Run-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1)
    if ($LASTEXITCODE) { throw "digest failed: $($lines -join ' ')" }
    return $lines[-1] | ConvertFrom-Json -AsHashtable
}
$instructions = 'dsh-digest-base.zh.md,dsh-history-cards.zh.md'
$common = @('-BibTeX', '@book{HistBook,title={History book}}', '-BookTitle', 'History book', '-StartHeading', '导论', '-EndHeading', '参考', '-MinUnitChars', '1', '-NoFigures', '-Instructions', $instructions, '-NoEventCandidates', '-Variant', 'history', '-Apply')
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source, "# 导论`n`n1648 年的账册记载，行会控制了城内的呢绒贸易。`n`n# 参考`n未选择。", $utf8)

    # A true switch of a layer the variant leaves out is refused before a Card is written.
    $refused = @(& $Digest prepare -Source $source -Out (Join-Path $scratch 'refused') -CitationKey HistRefused @common -RunOptions '{"models":true}' 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($refused -join ' ').Contains('not available for history')) 'prepare refuses models true for a history run'

    $out = Join-Path $scratch 'prepared'
    $null = Run-Digest (@('prepare', '-Source', $source, '-Out', $out, '-CitationKey', 'HistBook', '-RunOptions', '{"terms":false}') + $common)
    $manifest = Get-Content (Join-Path $out 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $contract = $manifest.run_contract
    $history = Get-Content (Join-Path $PSScriptRoot '..\workflow\zt-history-digest.blueprint.json') -Raw -Encoding utf8 | ConvertFrom-Json
    Assert ($contract.variant -eq 'history' -and $contract.contracts.variant_version -eq $history.version) 'the prepared run binds variant history and its blueprint version'
    $options = $contract.options.run_options
    Assert (-not $options.models -and -not $options.events -and -not $options.findings -and $options.termCards -and $options.termLink) 'the run options switch the three layers off and keep the Term layers'

    # The prompt the writer really receives.
    $prefix = [IO.File]::ReadAllText((Join-Path $out 'shared-prefix.md'))
    Assert ($prefix.Contains('不写模型卡') -and $prefix.Contains('史料')) 'the writer prompt carries the history supplement'
    Assert (-not $prefix.Contains('"type":"model"') -and -not $prefix.Contains('dsh-model-cards') -and -not $prefix.Contains('该推导见')) 'the writer prompt carries no Model Card rule'

    # The ledger drives the run: the digest layer has work, and the left-out layers cannot gain any.
    $status = Run-Digest @('ledger', 'status', '-Out', $out)
    $layers = ($status.units | ForEach-Object { $_.layer } | Select-Object -Unique)
    Assert ('digest' -in $layers) 'the ledger holds digest units'
    Assert (-not ($layers | Where-Object { $_ -in @('models', 'explain', 'events', 'findings') })) "the ledger holds no Model, Event or Finding unit ($($layers -join ','))"

    # A run can take more than one owner's stop: each recorded grant adds one resume.
    $first = Run-Digest @('ledger', 'resume-grant', '-Out', $out, '-Reason', 'first stop')
    $second = Run-Digest @('ledger', 'resume-grant', '-Out', $out, '-Reason', 'second stop')
    Assert ($first.grants -eq 1 -and $second.grants -eq 2) 'each resume grant is recorded'

    $next = Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-Take', '1', '-Owner', 'history-test')
    $claim = $next.units[0]
    $started = Run-Digest @('group-start', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)")
    $section = [IO.File]::ReadAllLines((Join-Path $out 'sections\s01.md'))
    $line = @(1..$section.Count | Where-Object { $section[$_ - 1] -match '行会' })[0]
    $base = ([IO.File]::ReadAllText($started.prompt) | Select-String -Pattern 'base_fingerprint[^0-9a-f]+([0-9a-f]{16,})').Matches[0].Groups[1].Value
    $note = @{ tool = 'create_successor'; arguments = @{ at = 'HistBook/1'; kind = 'side'; title = '行会控制呢绒贸易'; body = "1648 年的账册记载，行会控制了城内的呢绒贸易。`n`n来源：[[HistBook]] §导论"; src = "sections/s01.md:$line-$line" } }
    $anchor = @{ tool = 'edit_card'; arguments = @{ at = 'HistBook/1'; title = '行会与贸易'; body = "本部分说明行会怎样控制贸易。`n`n来源：[[HistBook]] 导论`n`n在全书中的位置：本书的引言部分。"; base_fingerprint = $base } }
    $write = { param($calls) [IO.File]::WriteAllText($started.draft, (@{ schema_version = 1; unit = $claim.id; token = $claim.token; input_hash = $started.input_hash; disposition = 'cards'; calls = $calls } | ConvertTo-Json -Depth 10), $utf8) }

    & $write @($note, $anchor)
    $covered = @(& $Digest coverage -Out $out -Group 1 -Draft $started.draft 2>&1)
    Assert ($covered[-1] -eq 'faults: 0') "a draft of plain notes passes the coverage SELF-CHECK ($($covered -join ' | '))"
    $checked = @(& $Digest group-finish -Out $out -Group 1 -Token $claim.token -Draft $started.draft -CheckOnly 2>&1)
    Assert ($checked[-1] -eq 'faults: 0') "a draft of plain notes passes the Card SELF-CHECK ($($checked -join ' | '))"

    foreach ($kind in @('model', 'event', 'finding')) {
        $typed = @{ tool = 'create_successor'; arguments = @{ at = 'HistBook/1'; kind = 'side'; type = $kind; title = '行会控制呢绒贸易'; body = "1648 年的账册记载，行会控制了城内的呢绒贸易。`n`n来源：[[HistBook]] §导论"; src = "sections/s01.md:$line-$line" } }
        & $write @($typed, $anchor)
        $faulted = @(& $Digest coverage -Out $out -Group 1 -Draft $started.draft 2>&1)
        Assert (($faulted -join ' ').Contains(' C5 ') -and $faulted[-1] -eq 'faults: 1') "the coverage SELF-CHECK reports a drafted $kind Card as C5"
    }
    # The last draft holds a Finding Card; a Model Card goes to the group finish itself.
    $typed = @{ tool = 'create_successor'; arguments = @{ at = 'HistBook/1'; kind = 'side'; type = 'model'; level = 'concept'; title = '行会控制呢绒贸易'; body = "1648 年的账册记载，行会控制了城内的呢绒贸易。`n`n来源：[[HistBook]] §导论"; src = "sections/s01.md:$line-$line" } }
    & $write @($typed, $anchor)
    $finish = @(& $Digest group-finish -Out $out -Group 1 -Token $claim.token -Draft $started.draft 2>&1)
    Assert ($LASTEXITCODE -ne 0 -or ($finish -join ' ').Contains('reading notes only')) "the group finish refuses a Model Card ($($finish[-1]))"
    Assert (($finish -join ' ').Contains('reading notes only')) 'the refusal names the reason'

    # The Figures stage on the real archive: a Card that never names a figure receives it through a reader's answer.
    & $write @($note, $anchor)
    $null = Run-Digest @('group-finish', '-Out', $out, '-Group', '1', '-Token', "$($claim.token)", '-Draft', $started.draft)
    $store = Join-Path (Join-Path $scratch 'archive') 'artifacts'
    New-Item -ItemType Directory -Force $store | Out-Null
    $bytes = [byte[]](0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A) + [Text.Encoding]::ASCII.GetBytes('fixture')
    $sha = -join ([Security.Cryptography.SHA256]::HashData($bytes) | ForEach-Object { $_.ToString('x2') })
    [IO.File]::WriteAllBytes((Join-Path $store "$sha.png"), $bytes)
    $figures = Join-Path $scratch 'figures.json'
    [IO.File]::WriteAllText($figures, (@{ figures = @(@{ name = 'a.jpg'; sha256 = $sha; path = 'a.jpg'; labels = @('7.8'); alt = '图 7.8'; line = "![图 7.8](artifact:$sha)" }) } | ConvertTo-Json -Depth 6), $utf8)
    $plan = Run-Digest @('figures', '-Step', 'plan', '-CitationKey', 'HistBook', '-Staging', $out, '-FiguresFile', $figures)
    Assert ($plan.figures -eq 1 -and $plan.jobs -eq 1 -and $plan.chunks -eq 1) "the figure plan lists the figure and one reader job ($($plan | ConvertTo-Json -Compress))"
    $jobs = @(Get-Content -LiteralPath (Join-Path $out 'figures\jobs-001.json') -Raw -Encoding utf8 | ConvertFrom-Json)
    Assert ($jobs.Count -eq 1 -and $jobs[0].files.Count -eq 3) 'the manager job file lists the job with its instructions, list and Cards'
    Assert ([IO.File]::ReadAllText($jobs[0].files[1]).Contains('7.8')) 'the figure list names the figure'
    $task = [IO.File]::ReadAllText($jobs[0].files[2])
    Assert ($task.Contains('行会控制呢绒贸易') -and -not $task.Contains('术语表')) 'the job holds the notes of the group'
    $card = ([regex]::Match($task, '## `([^`]+)`')).Groups[1].Value
    $answer = Join-Path $out 'figures\out\g001.json'
    [IO.File]::WriteAllText($answer, (@{ links = @(@{ card = 'HistBook/9'; figures = @('9.9') }) } | ConvertTo-Json -Depth 6), $utf8)
    $bad = @(& $Digest figures -Step precheck -CitationKey HistBook -Staging $out -Job g001 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and $bad[-1] -eq 'faults: 2') "the reader's SELF-CHECK refuses a Card and a label it does not know ($($bad -join ' | '))"
    [IO.File]::WriteAllText($answer, (@{ links = @(@{ card = $card; figures = @('7.8') }) } | ConvertTo-Json -Depth 6), $utf8)
    $good = @(& $Digest figures -Step precheck -CitationKey HistBook -Staging $out -Job g001 2>&1)
    Assert ($LASTEXITCODE -eq 0 -and $good[-1] -eq 'faults: 0') "the reader's SELF-CHECK passes ($($good -join ' | '))"
    $report = Join-Path $out 'figures-report.json'
    $null = Run-Digest @('figures', '-CitationKey', 'HistBook', '-Staging', $out, '-FiguresFile', $figures, '-NoDocuments', '-Report', $report, '-Apply')
    . (Join-Path $PSScriptRoot '..\zt-calls.ps1')
    $shown = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $card } })
    Assert ("$($shown.body)".Contains("artifact:$sha")) 'the Card that never names the figure now shows it'
    Assert ("$($shown.body)".IndexOf("artifact:$sha") -lt "$($shown.body)".IndexOf('来源：')) 'the figure stands before the source line'
    $null = Run-Digest @('figures', '-CitationKey', 'HistBook', '-Staging', $out, '-FiguresFile', $figures, '-NoDocuments', '-Report', $report, '-Apply')
    $again = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $card } })
    Assert ("$($again.body)" -ceq "$($shown.body)") 'a second pass changes nothing'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'history run: ok'
