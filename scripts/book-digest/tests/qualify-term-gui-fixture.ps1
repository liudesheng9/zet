#Requires -Version 7.5
<#
.SYNOPSIS
  Start one scratch archive holding the S29 (#171) Term-reading GUI fixture, and hold it for a real
  browser check.

.DESCRIPTION
  The S29-AC08 check inspects the running page for the Term reader: a Term use as a quiet label
  beside an ordinary Link, a broken Term use explained without hover, a cross-Workspace Link, a
  Term head as an explanation Card with its collapsed uses, and the writing desk's completion of
  Term names and ordinary citations. It also inspects light/dark and hover/focus states.

  This script builds that fixture through the public `zt mcp` tools on a scratch archive, starts
  the service, and prints the GUI URL and the token. It never touches the live archive.

    pwsh -NoProfile -File scripts\book-digest\tests\qualify-term-gui-fixture.ps1 -Action start
    pwsh -NoProfile -File scripts\book-digest\tests\qualify-term-gui-fixture.ps1 -Action url
    pwsh -NoProfile -File scripts\book-digest\tests\qualify-term-gui-fixture.ps1 -Action stop

  -Action start -Reset rebuilds the scratch archive.

  The fixture holds: one agent book (`TermBook2026`) with a four-row Term list and three heads
  (均衡, 生产函数, 学习); an agent Idea Card that uses 均衡 twice and links one other Card with an
  ordinary reasoned Link; a broken ordinary Link; a head (弹性) deleted after one use so the use
  stays as a labelled broken Term use; and a human Workspace Card that links into the agent book,
  so the cross-Workspace ink is visible.
#>
[CmdletBinding()]
param(
    [ValidateSet('start', 'url', 'stop')][string]$Action = 'start',
    [string]$Archive = (Join-Path ([IO.Path]::GetTempPath()) 'zt-term-gui-fixture'),
    [string]$Zt = '',
    [switch]$Reset,
    [switch]$NoHold
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$config = Join-Path (Split-Path -Parent $Archive) 'zt-term-gui-fixture-config'
$env:ZT_CONFIG_DIR = $config
$env:ZT_GUI_PORT = '0'
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')

function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}

function Wait-Down {
    for ($i = 0; $i -lt 50; $i++) {
        $status = @(zt status 2>&1)
        if (-not ($status -match '^state: up')) { return }
        Start-Sleep -Milliseconds 100
    }
    throw 'the service did not stop'
}

function Get-Gui {
    $out = @(zt gui 2>&1)
    for ($i = 0; $i -lt 30 -and $LASTEXITCODE -ne 0; $i++) {
        Start-Sleep -Milliseconds 200
        $out = @(zt gui 2>&1)
    }
    if ($LASTEXITCODE -ne 0) { throw "the service is not up: $($out -join ' ')" }
    $line = @($out | Where-Object { $_ -match '^zt gui: http' })[0]
    if (-not $line) { throw "zt gui printed no URL: $($out -join ' ')" }
    $url = ($line -replace '^zt gui: ', '').Trim()
    $page = (Invoke-WebRequest -Uri $url -UseBasicParsing).Content
    $token = [regex]::Match($page, 'name="zt-token" content="([^"]+)"').Groups[1].Value
    if (-not $token) { throw 'the served page carries no token' }
    [pscustomobject]@{ url = $url; token = $token }
}

function Show-Checks($gui, $addresses) {
    $gui | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Archive 'gui-fixture.json') -Encoding utf8
    $addresses | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Archive 'gui-fixture-addresses.json') -Encoding utf8
    Write-Output "gui: $($gui.url)"
    Write-Output "token: $($gui.token)"
    Write-Output "archive: $Archive"
    Write-Output "config: $config"
    Write-Output 'fixture Cards:'
    foreach ($entry in $addresses.GetEnumerator()) { Write-Output ("  {0,-10} {1}" -f $entry.Key, $entry.Value) }
    Write-Output @'
what the page must show (issue #171, S29-AC08 / R0550):
  1. Term use as a quiet label: open the agent Card 笔记 = 均衡是重复博弈的稳定结果. Both uses of
     均衡 render as the bare word (never [[均衡@…]]), are quieter than the ordinary Link 依赖…, and
     hover/focus marks them without turning the paragraph into a table of links. Clicking a label
     opens its Term head 均衡.
  2. Ordinary Link distinction: the ordinary reasoned Link on the same Card renders as a Link with
     its arrow/target, its Reason kind visible, and hover/focus states distinct from a Term label.
  3. Broken Term use: open the Card 弹性… Deleting the head left the use as a labelled, marked
     reference; the page says it is broken without hover (a reader who cannot hover still learns),
     and no address is shown.
  4. Broken ordinary Link: the Card 含断链的笔记 shows its broken Link marked as broken, and the
     Broken-links reader lists it.
  5. Cross-Workspace: open the human Card 读记 = 读 [[TermBook2026/1]] …, which links into the agent
     book. The Link carries the agent Workspace's ink, both in the human Card and in the agent
     target's Reverse section.
  6. Term head as an explanation Card: open 均衡. Its sections (释义, 区别, 来源) read as one
     explanation Card, and its ## Term uses section lists the using Cards collapsed.
  7. Writing desk completion: open the desk on the agent Card 笔记 and type [[均. The completion
     offers the Term 均衡 (and, with [[TermB, the ordinary citation TermBook2026). The inserted
     form differs: a Term name completes to [[label@address]], an ordinary Card to [[address]].
  8. Light and dark: repeat 1, 2, 6 in both themes and tab through the focus ring. No Term label or
     broken marker loses contrast, and the held Card keeps more ink than a secondary panel.
'@
}

switch ($Action) {
    'stop' {
        zt down | Out-Null
        Write-Output "service stopped: $Archive"
        break
    }
    'url' {
        $gui = Get-Gui
        Show-Checks $gui (Get-Content -LiteralPath (Join-Path $Archive 'gui-fixture-addresses.json') -Raw | ConvertFrom-Json -AsHashtable)
        break
    }
}
if ($Action -eq 'start') {
    New-Item -ItemType Directory -Force -Path $Archive, $config | Out-Null
    if ($Reset) {
        zt down 2>$null | Out-Null
        Wait-Down
        Get-ChildItem -LiteralPath $Archive -Force | Remove-Item -Recurse -Force
    }
    zt config set archive_root $Archive | Out-Null
    zt up | Out-Null
    $existing = @(Invoke-Zt @(@{ tool = 'search_cards'; arguments = @{ query = '均衡' } }) | ForEach-Object { $_.results })
    if ($existing.Count -gt 0) {
        $gui = Get-Gui
        Show-Checks $gui (Get-Content -LiteralPath (Join-Path $Archive 'gui-fixture-addresses.json') -Raw | ConvertFrom-Json -AsHashtable)
    } else {
        $table = @'
## 术语表

| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |
| 弹性 | 弹性系数 | 价格变化的反应程度 | §2.3 |
| 生产函数 | 产量函数 | 投入与产出的关系 | §3.1 |
| 学习 | 学习效应 | 经验带来的效率提高 | §4.2 |
'@
        $book = Write-Cards @(
            @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{TermBook2026, title={Term Book 2026}}'; body = '# Term Book 2026

一本说明术语的书。' } }
            @{ tool = 'create_successor'; arguments = @{ at = 'TermBook2026'; title = '第一章：均衡'; body = '生产函数影响均衡。均衡是稳定的。' } }
        )
        $anchor = $book[-1]
        $list = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{
            at = $anchor; kind = 'side'; type = 'term'; title = '术语表：TermBook2026 的 4 个术语'; body = $table
        } }))[-1]
        function New-Head($term, $definition) {
            (Write-Cards @(@{ tool = 'create_successor'; arguments = @{
                at = $list; kind = 'side'; type = 'term'; term = $term; title = $term
                body = "术语：$term`n类别：概念`n首次定义：§1.1`n`n## 释义`n`n$definition`n`n## 区别`n`n- 与 生产函数：不同，均衡说的是策略。`n`n来源：[[TermBook2026]] §1.1"
            } }))[-1]
        }
        $balance = New-Head '均衡' '两个参与者都不愿单方面改变策略，因此价格与数量都不再移动。'
        $production = New-Head '生产函数' '生产函数是投入与产出的关系，它决定供给曲线。'
        $learning = New-Head '学习' '经验带来效率提高，因此同一投入得到更多产出。'
        $elastic = New-Head '弹性' '弹性是价格变化时数量变化的反应程度。'

        $topic = (Write-Cards @(@{ tool = 'create_topic'; arguments = @{ title = '笔记'; body = '术语读者的笔记。' } }))[-1]
        $other = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{
            at = $topic; title = '重复博弈的稳定策略'; body = '这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。'
        } }))[-1]
        $note = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{
            at = $other; title = '均衡是重复博弈的稳定结果'
            body = "这一段说明背景并给出充分理由。[[均衡@$balance]] 是稳定结果，[[均衡@$balance]] 在重复博弈里仍然成立。它让读者看到条件。依赖 [[$other]]：这条理由说明选择。`n`n来源：[[TermBook2026]] §1.1"
        } }))[-1]
        $doomed = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{
            at = $other; kind = 'side'; title = '会被删除的卡'; body = '这一段说明背景并给出充分理由。'
        } }))[-1]
        $brokenNote = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{
            at = $note; title = '含断链的笔记'
            body = "这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。对应 [[$doomed]]：这条理由说明选择。"
        } }))[-1]
        $elasticNote = (Write-Cards @(@{ tool = 'create_successor'; arguments = @{
            at = $brokenNote; title = '弹性决定税负落在谁身上'
            body = "这一段说明背景并给出充分理由。[[弹性@$elastic]] 决定税负落在谁身上。它让读者看到条件。"
        } }))[-1]
        # Delete the 弹性 head after its use, so the use stays as a labelled broken Term use, and
        # delete the linked Card, so the ordinary Link stays broken.
        Invoke-Zt @(
            @{ tool = 'delete_card'; arguments = @{ at = $elastic; confirm = 'delete' } }
            @{ tool = 'delete_card'; arguments = @{ at = $doomed; confirm = 'delete' } }
        ) | Out-Null

        # A human Workspace Card that links into the agent book: the cross-Workspace ink. The MCP
        # server has agent Access only, so the human Workspace is written through the GUI's own API.
        $gui = Get-Gui
        $headers = @{ 'X-ZT-Token' = $gui.token }
        $human = Invoke-RestMethod -Uri ($gui.url + 'api/create') -Method Post -Headers $headers -ContentType 'application/json' -Body (@{
            kind = 'topic'; workspace = 'human'; text = "# 读记`n`n人的读记。`n"
        } | ConvertTo-Json)
        $humanTopic = $human.address
        $humanCard = Invoke-RestMethod -Uri ($gui.url + 'api/create') -Method Post -Headers $headers -ContentType 'application/json' -Body (@{
            kind = 'direct'; at = $humanTopic
            text = "# 读 TermBook2026 的第一章`n`n这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。读 [[$anchor]]：这条理由说明选择。`n"
        } | ConvertTo-Json)
        $humanNote = $humanCard.address

        $addresses = [ordered]@{
            book = 'TermBook2026'; anchor = $anchor; list = $list
            balance = $balance; production = $production; learning = $learning
            topic = $topic; other = $other; note = $note; brokenNote = $brokenNote; elasticNote = $elasticNote
            humanTopic = $humanTopic; humanNote = $humanNote
        }
        Show-Checks $gui $addresses
    }
    if (-not $NoHold) {
        Write-Output 'the service stays up; press Enter to leave it running and exit this script.'
        $null = Read-Host
    }
}
