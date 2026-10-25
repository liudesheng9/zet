#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\zt-check.tests.ps1
# Checks zt-check (the one Card quality check) against a fixture with one fault for each rule it checks, and
# text it must leave alone (formulas, Links, tables, quotations, charts).
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$fixture = Join-Path $PSScriptRoot 'card-style-fixture.json'
$lines = @(Invoke-ZtCheck -Json $fixture -Setting 'K/1|a|a')
$expected = @(
    'K/1|a|a|1 R12 (49 characters > 40) 清算比例：E 付不出 P 时'
    'K/1|a|a|1 R3 E 在时点 1 付不出去；E 能部分付出。'
    'K/1|a|a|1 R4 资产被清算。'
    'K/1|a|a|1 R5 双方进行谈判。'
    'K/1|a|a|1 R7 E 在时点 1 的用来还债的现金的上限是 $P$。'
    'K/1|a|a|1 R6 (advisory) uses `卖资产` for `清算`: 值得注意的是，C 会卖资产。'
    'K/1|a|a|1 R9 `值得注意的是`: 值得注意的是，C 会卖资产。'
    'K/1|a|a|1 R6 (advisory) uses `还款额` for `可承诺的还款`: 可承诺的还款等于 $B$，还款额不会更高。'
    'K/1|a|a|1 R2 (50 characters > 40) 这一句很长很长'
    'K/1|a|a|2 R2 (34 characters > 30) 代入 $P=B$ 以后'
    'K/1|a|a|3 R3 The creditor cannot verify the return; it cannot enforce a payment from it either.'
    'K/1|a|a|4 R13 (13 sentences > 12) split the part'
    'K/1|a|a|5 R14 (7 steps > 6) split the derivation into a chain of Derivation parts'
    'Other/1|a|a R21 (advisory) unnamed referent `这张卡` with no named Link in its sentence: 这张卡属于另一个模型。'
    'faults: 11'
)
$failed = 0
if ($lines.Count -ne $expected.Count) {
    Write-Output "expected $($expected.Count) lines, got $($lines.Count):"
    $lines | ForEach-Object { Write-Output "  $_" }
    $failed++
}
for ($i = 0; $i -lt [Math]::Min($lines.Count, $expected.Count); $i++) {
    if (-not $lines[$i].StartsWith($expected[$i])) {
        Write-Output "line $i`n  expected: $($expected[$i])`n  got:      $($lines[$i])"
        $failed++
    }
}
# The setting's own tables, the quotation, chart, and formulas, and a shared part's words under
# another model's term table raise nothing. The shared part's one bare deictic is the pilot's
# advice (R21), which its own text earns.
foreach ($line in $lines) {
    $sharedAdvice = $line.StartsWith('Other/') -and $line.Contains(' R21 (advisory) ')
    if (($line -match '引文|图里|清算价值；|^Other/') -and -not $sharedAdvice) { Write-Output "checked text it must skip: $line"; $failed++ }
}
# R13 counts prose characters too, against -MaxChars; a Derivation part's steps count under R14 only.
$small = Join-Path ([IO.Path]::GetTempPath()) "card-style-size-$PID.json"
@(
    [pscustomobject]@{ address = 'S/1|a|a'; facet = '设定'; title = '设定'; body = 'E 有一个项目，项目在时点 2 产生收益。C 出资，换取时点 1 的还款。E 可以挪用一部分收益。' }
    [pscustomobject]@{ address = 'S/1|a|a|1'; facet = '推导'; title = '推导'; body = "1. E 在时点 1 还款以后，手里剩下项目的收益。`n2. E 在时点 2 挪用一部分收益，留下其余部分。`n3. 两边加上初始财富，得到参与约束。" }
) | ConvertTo-Json | Set-Content -LiteralPath $small -Encoding utf8
$size = @(Invoke-ZtCheck -Json $small -MaxChars 30 -MaxSteps 2)
# A concept model keeps its few parts whole: no size faults.
$concept = @(Invoke-ZtCheck -Json $small -MaxChars 30 -MaxSteps 2 -Level concept)
Remove-Item -LiteralPath $small
if ($concept[-1] -ne 'faults: 0') { Write-Output "a concept model must raise no size fault:"; $concept | ForEach-Object { Write-Output "  $_" }; $failed++ }
$expectedSize = @(
    'S/1|a|a R13 ('
    'S/1|a|a|1 R14 (3 steps > 2)'
    'faults: 2'
)
if ($size.Count -ne $expectedSize.Count) { Write-Output "size check: expected $($expectedSize.Count) lines, got:"; $size | ForEach-Object { Write-Output "  $_" }; $failed++ }
else {
    for ($i = 0; $i -lt $size.Count; $i++) {
        if (-not $size[$i].StartsWith($expectedSize[$i])) { Write-Output "size line $i`n  expected: $($expectedSize[$i])`n  got:      $($size[$i])"; $failed++ }
    }
    if ($size[0] -notmatch 'characters > 30') { Write-Output "size check: R13 must count characters: $($size[0])"; $failed++ }
}
# R15 to R17: the explanation of a Derivation part (docs/derivation-explanations-proposal.md).
$steps = "**步骤**`n1. 参与约束：E 的所得高于 `$w`$，即 `$R+fy_2>w`$。`n2. 情形 `$f=1`$：`$R=c-P`$。`n   说明：不清算时，现金够还款，E 留下差额。`n3. 两边加 `$K-w`$，得 `$y_1+fy_2>K`$。"
$good = "**思路**`nE 只在还款后所得超过 `$w`$ 时参与。先求 E 还款后剩下的现金 `$R`$。`n`n**起点**`n- 依赖设定 [[X/1|a|a]]：E 参与，须所得高于 `$w`$。`n`n$steps`n`n**结果**`n`$`$y_1+fy_2>K`$`$`n`n**解读**`n`$w`$ 在两边相消，所以 E 的财富不影响参与。检验：`$f=1`$ 时它是普通的净现值条件。"
$long = (1..17 | ForEach-Object { "第 $_ 句解释推导的一步。" }) -join ''
$explainCards = @(
    [pscustomobject]@{ address = 'X/1|a|a|1'; facet = '推导：参与约束'; title = '推导：参与约束'; body = $good }
    [pscustomobject]@{ address = 'X/1|a|a|2'; facet = '推导：缺解读'; title = '推导：缺解读'; body = ($good -replace '(?s)\*\*解读\*\*.*$', '').Trim() }
    [pscustomobject]@{ address = 'X/1|a|a|3'; facet = '推导：两行说明'; title = '推导：两行说明'; body = $good.Replace("留下差额。", "留下差额。`n   说明：这一行多余。") }
    [pscustomobject]@{ address = 'X/1|a|a|4'; facet = '推导：太长'; title = '推导：太长'; body = $good.Replace("**解读**`n", "**解读**`n$long`n") }
    [pscustomobject]@{ address = 'X/1|a|a|5'; facet = '推导：旧格式'; title = '推导：旧格式'; body = "**起点**`n- 依赖设定 [[X/1|a|a]]。`n`n$steps`n`n**结果**`n`$`$y_1+fy_2>K`$`$" }
)
$explainFile = Join-Path ([IO.Path]::GetTempPath()) "card-style-explain-$PID.json"
$explainCards | ConvertTo-Json | Set-Content -LiteralPath $explainFile -Encoding utf8
$plainRun = @(Invoke-ZtCheck -Json $explainFile)
$strictRun = @(Invoke-ZtCheck -Json $explainFile -RequireExplain)
# The explanation does not count under R13: 12 sentences of 思路 and 解读 fit a -MaxSentences 3 part.
$r13Run = @(Invoke-ZtCheck -Json $explainFile -MaxSentences 3 -MaxExplainSentences 40 -MaxExplainChars 2000)
Remove-Item -LiteralPath $explainFile
$expectedExplain = @(
    'X/1|a|a|2 R15 sections 思路, 起点, 步骤, 结果; want 思路, 起点, 步骤, 结果, 解读'
    'X/1|a|a|3 R17 a second 说明 line under step 2'
    'X/1|a|a|4 R16 (解读 has 19 sentences > 6)'
    'X/1|a|a|4 R17 (22 explanation sentences > 16)'
    'faults: 4'
)
if ($plainRun.Count -ne $expectedExplain.Count) { Write-Output "explain check: expected $($expectedExplain.Count) lines, got:"; $plainRun | ForEach-Object { Write-Output "  $_" }; $failed++ }
else {
    for ($i = 0; $i -lt $plainRun.Count; $i++) {
        if (-not $plainRun[$i].StartsWith($expectedExplain[$i])) { Write-Output "explain line $i`n  expected: $($expectedExplain[$i])`n  got:      $($plainRun[$i])"; $failed++ }
    }
}
if (-not @($strictRun | Where-Object { $_ -like 'X/1|a|a|5 R15 sections 起点, 步骤, 结果*' }).Count -or $strictRun[-1] -ne 'faults: 5') {
    Write-Output "explain check: -RequireExplain must flag the older part:"; $strictRun | ForEach-Object { Write-Output "  $_" }; $failed++
}
if (@($r13Run | Where-Object { $_ -match '^X/1\|a\|a\|[1-4] R13 ' }).Count) {
    Write-Output "explain check: R13 must not count the explanation:"; $r13Run | ForEach-Object { Write-Output "  $_" }; $failed++
}

# --- S06 (#148): the exact draft, the status document, and the exit contract ----------------------
# An offline draft is honest about what it did not cover: its lines stay complete for
# the families it declares, and the process exits 2 rather than passing by omission.
$checker = (Get-Command zt-check -CommandType Application | Select-Object -First 1).Source
# The rule version this checker uses is the one its own status document publishes (S27, #166): a
# draft binds the contract it was checked under, never a frozen literal that drifts from the rules.
$ruleStatusPath = Join-Path ([IO.Path]::GetTempPath()) "zt-check-rule-$PID.json"
$null = @(& $checker --json $fixture --setting 'K/1|a|a' --report $ruleStatusPath 2>&1)
if ($LASTEXITCODE -ne 2) { Write-Output "exit: an offline draft must exit 2, got $LASTEXITCODE"; $failed++ }
if (-not (Test-Path -LiteralPath $ruleStatusPath)) { throw 'the offline draft wrote no status document' }
$ruleVersion = [string](Get-Content -LiteralPath $ruleStatusPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable).contract.rule_version
Remove-Item -LiteralPath $ruleStatusPath -Force -ErrorAction SilentlyContinue
if (-not $ruleVersion) { throw 'the status document names no rule version' }

$scratch = Join-Path ([IO.Path]::GetTempPath()) "zt-check-contract-$PID"
New-Item -ItemType Directory -Force $scratch | Out-Null
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
try {
    $null = @(zt config set archive_root (Join-Path $scratch 'archive') 2>&1)
    $null = @(zt up 2>&1)
    $null = Invoke-Zt @(@{tool='create_topic';arguments=@{title='校验主题'}})
    # A write answers `N ok <address>`; the read names the stored row.
    $topic = @(@(Invoke-Zt @(@{tool='export_titles';arguments=@{}}))[0].titles | Where-Object { $_.title -eq '校验主题' })[0]
    $cleanText = "# 税负上升会减少交易数量`n`n税负上升使买卖双方减少交易数量。均衡价格上升，市场参与者也随之改变。我们可以观察成交量的下降。"
    $draftPath = Join-Path $scratch 'draft.json'
    $reportPath = Join-Path $scratch 'status.json'
    $policyHash = ((& zt config show) | Where-Object { $_ -like 'policy_hash=*' }) -replace '^policy_hash=', ''
    $contextCard = [ordered]@{ address = $topic.address; title = $topic.title; workspace = 'agent' }
    function Write-Draft($cards, $families) {
        [ordered]@{
            rule_version = $ruleVersion
            policy_hash = $policyHash
            cards = $cards
            context = [ordered]@{ cards = @($contextCard); complete_for = $families }
        } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $draftPath -Encoding utf8
    }
    $all = @('envelope', 'local', 'graph', 'placement', 'policy')

    # A complete, clean draft: exit 0, the status names the contract it was checked under.
    Write-Draft @([ordered]@{ address = 'D/1'; type = 'idea'; text = $cleanText }) $all
    $out = @(& $checker --draft $draftPath --report $reportPath 2>&1)
    if ($LASTEXITCODE -ne 0 -or $out[-1] -ne 'faults: 0') { Write-Output "draft: a complete clean draft must exit 0 with faults: 0 ($($LASTEXITCODE): $($out -join ' | '))"; $failed++ }
    $status = Get-Content -LiteralPath $reportPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    if ($status.mode -ne 'draft_bundle' -or $status.verdict -ne 'pass' -or -not $status.complete -or @($status.missing).Count) { Write-Output "status: a complete draft must say so ($($status | ConvertTo-Json -Compress -Depth 6))"; $failed++ }
    if ($status.policy_source -ne 'built_in' -or $status.contract.rule_version -ne $ruleVersion -or $status.contract.policy_hash -ne $policyHash) { Write-Output 'status: the draft binds the built-in policy it declared'; $failed++ }

    # A complete draft with a blocking fault: exit 1, the blocker in faults: N.
    Write-Draft @([ordered]@{ address = 'D/2'; type = 'idea'; text = "# 税负落在买方`n`n税负落在买方；卖方只承担一小部分。" }) $all
    $out = @(& $checker --draft $draftPath --report $reportPath 2>&1)
    if ($LASTEXITCODE -ne 1 -or $out[-1] -ne 'faults: 2') { Write-Output "draft: a complete draft with blockers must exit 1 ($($LASTEXITCODE): $($out -join ' | '))"; $failed++ }
    $status = Get-Content -LiteralPath $reportPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    if ($status.verdict -ne 'block' -or $status.blocking -ne 2 -or @($status.diagnostics).Count -ne 3) { Write-Output "status: the draft counts its blocking diagnostics ($($status | ConvertTo-Json -Compress -Depth 8))"; $failed++ }

    # An undeclared family stays missing: the answer is incomplete, never a pass.
    Write-Draft @([ordered]@{ address = 'D/1'; type = 'idea'; text = $cleanText }) @('envelope', 'local')
    $out = @(& $checker --draft $draftPath --report $reportPath 2>&1)
    $status = Get-Content -LiteralPath $reportPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    if ($LASTEXITCODE -ne 2 -or ($status.missing -join ',') -ne 'graph,placement,policy') { Write-Output "status: an undeclared family is missing ($(($status.missing -join ',')), exit $LASTEXITCODE)"; $failed++ }

    # A stale rule binding is refused, with a message, and never reported as a pass.
    [ordered]@{ rule_version = '0'; cards = @([ordered]@{ address = 'D/1'; text = $cleanText }) } |
        ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $draftPath -Encoding utf8
    $err = @(& $checker --draft $draftPath 2>&1)
    if ($LASTEXITCODE -ne 2 -or ($err -join ' ') -notmatch 'stale check contract') { Write-Output "draft: a stale rule version is refused ($($LASTEXITCODE): $($err -join ' | '))"; $failed++ }

    # A live check reads the running service's contract: the policy hash is the archive's,
    # the status names the gate stage, and an explicit limit that differs is incomplete.
    $null = Invoke-Zt @(@{tool='create_successor';arguments=@{at=$topic.address;title='税负落在买方';body='税负落在买方；卖方只承担一小部分。'}})
    $written = @(@(Invoke-Zt @(@{tool='export_titles';arguments=@{}}))[0].titles | Where-Object { $_.title -eq '税负落在买方' })[0]
    $out = @(& $checker --cards $written.address --report $reportPath 2>&1)
    if ($LASTEXITCODE -ne 1 -or $out[-1] -notmatch '^faults: [1-9]') { Write-Output "live: a faulting live Card exits 1 ($($LASTEXITCODE): $($out -join ' | '))"; $failed++ }
    $status = Get-Content -LiteralPath $reportPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    if ($status.mode -ne 'live' -or $status.policy_source -ne 'archive' -or $status.contract.policy_hash -ne $policyHash -or $status.gate.stage -ne 'advisory' -or $status.gate.would_refuse) { Write-Output "live: the live status names the archive contract ($($status | ConvertTo-Json -Compress -Depth 6))"; $failed++ }
    $out = @(& $checker --cards $written.address --max-chars 100 --report $reportPath 2>&1)
    $status = Get-Content -LiteralPath $reportPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    if ($LASTEXITCODE -ne 2 -or ($status.missing -join ',') -ne 'limits' -or ($status.reasons -join ' ') -notmatch 'max_chars') { Write-Output "live: a limit outside the contract is incomplete ($($LASTEXITCODE): $(($status.missing -join ',')))"; $failed++ }
    $out = @(& $checker --cards $written.address --workspace human --report $reportPath 2>&1)
    $status = Get-Content -LiteralPath $reportPath -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    if ($LASTEXITCODE -ne 2 -or ($status.missing -join ',') -ne 'workspace' -or ($status.reasons -join ' ') -notmatch 'cannot reclassify a stored Card') { Write-Output "live: a conflicting Workspace flag is refused ($($LASTEXITCODE): $(($status.missing -join ',')))"; $failed++ }
} finally {
    $null = @(zt down 2>&1)
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item Env:ZT_CONFIG_DIR, Env:ZT_GUI_PORT -ErrorAction SilentlyContinue
}

if ($failed) { Write-Output "FAILED: $failed"; exit 1 }
Write-Output 'zt-check: all checks passed'
