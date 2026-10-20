#Requires -Version 7.5
<#
.SYNOPSIS
  Start one scratch archive holding the S24 (#120) GUI qualification fixture, and hold it for a
  real browser check.

.DESCRIPTION
  The GUIS-05/GUIS-06 checks of issue #120 inspect the running page: the search tray, the Ask
  preview with Keep/Cancel, filing a kept Spread as a card, the reader Reply and its changed-target
  error, the Table's one distant slot, and the light/dark/focus states — with the editor, an input,
  the command bar and a modal open, where no keyboard shortcut may fire.

  This script builds that fixture through the public `zt mcp` tools on a scratch archive, starts the
  service, and prints the GUI URL and the token. It never touches the live archive.

    pwsh -NoProfile -File scripts\book-digest\tests\qualify-gui-fixture.ps1 -Action start
    pwsh -NoProfile -File scripts\book-digest\tests\qualify-gui-fixture.ps1 -Action url
    pwsh -NoProfile -File scripts\book-digest\tests\qualify-gui-fixture.ps1 -Action touch
    pwsh -NoProfile -File scripts\book-digest\tests\qualify-gui-fixture.ps1 -Action stop

  `-Action touch` edits book B's Card after the page opened it, so the reader's Reply save shows the
  changed-target error and keeps the draft. `-Action start -Reset` rebuilds the scratch archive.

  The fixture holds: two books that link each other, an Idea Topic whose Idea Card carries a model
  with one part and a line hub, the concept register with a three-target term (`多重均衡`) and a
  two-target term (`学习`), and an empty human Workspace, so a Reply creates the default Topic.
#>
[CmdletBinding()]
param(
    [ValidateSet('start', 'url', 'touch', 'stop')][string]$Action = 'start',
    [string]$Archive = (Join-Path ([IO.Path]::GetTempPath()) 'zt-gui-fixture'),
    [string]$Zt = '',
    [switch]$Reset,
    [switch]$NoHold
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$config = Join-Path (Split-Path -Parent $Archive) 'zt-gui-fixture-config'
$env:ZT_CONFIG_DIR = $config
$env:ZT_GUI_PORT = '0'
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')

function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}

function Get-Gui {
    $out = @(zt gui 2>&1)
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
what the page must show (issue #120, S24-AC05/AC06):
  1. search tray: type 多重均衡 in the search box and open the tray. The register entry rows come
     first (its three targets: book A's Card, book B's Card, the side Idea Card), then the hub
     member line, then the body hits; the held Card stays the visual focus and the tray does not
     become a second stage.
  2. Ask: type a question (例如 为什么策略互补会产生多重均衡？), preview the proposal, Keep it. The
     Table shows the kept Spread and its whole question; Cancel keeps the Table as it was.
  3. file as card: from the kept Spread, open the file-as-card draft. The draft names the agent
     register as the destination, the desk shows the Markdown, and Cancel writes nothing.
  4. reader Reply: open book B's Card, open the Reply desk, type a reply that starts with a relation
     word and the Card's Link (例如 印证 [[bookB/1]]：同一机制在另一本书里也成立。). Before saving,
     run `-Action touch` in another terminal, then save: the error names the changed Card, the draft
     and its destination stay visible, and nothing is written. Reload and save again: one human Card
     in the 回应 Topic.
  5. distant slot: open the Idea Card 干中学提高生产率 (0/1). The Table shows exactly one distant
     Card (book B's 策略互补), it never expands into a second recommendation tray, and opening it
     writes no Link.
  6. light/dark/focus: repeat 1, 2 and 4 in both themes, tab through the focus ring, and confirm the
     held Card keeps more ink than any secondary panel.
  7. shortcuts: with the editor open, an input focused, the command bar open and a modal shown, the
     view shortcuts (t, /, space, f, g) must not fire.
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
    'touch' {
        $addresses = Get-Content -LiteralPath (Join-Path $Archive 'gui-fixture-addresses.json') -Raw | ConvertFrom-Json -AsHashtable
        $result = Write-Cards @(@{ tool = 'edit_card'; arguments = @{
            at = $addresses.bookB
            body = "策略互补解释多重均衡。一个市场可以有两个稳定价格。`n`n来源：[[bookB]]"
        } })
        Write-Output "edited (the next Reply save must report target_changed): $($result -join ' ')"
        break
    }
}
if ($Action -eq 'start') {
    New-Item -ItemType Directory -Force -Path $Archive, $config | Out-Null
    if ($Reset) {
        zt down 2>$null | Out-Null
        Get-ChildItem -LiteralPath $Archive -Force | Remove-Item -Recurse -Force
    }
    zt config set archive_root $Archive | Out-Null
    zt up | Out-Null
    $existing = @(Invoke-Zt @(@{ tool = 'search_cards'; arguments = @{ query = '多重均衡' } }) | ForEach-Object { $_.results })
    if ($existing.Count -gt 0) {
        $gui = Get-Gui
        Show-Checks $gui (Get-Content -LiteralPath (Join-Path $Archive 'gui-fixture-addresses.json') -Raw | ConvertFrom-Json -AsHashtable)
    } else {
        $bookA = Write-Cards @(
            @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{bookA, title={Book A}}'; body = '该书。' } }
            @{ tool = 'create_successor'; arguments = @{ at = 'bookA'; title = '多重均衡的陈述'; body = '多重均衡来自策略互补。' } }
        )
        $bookB = Write-Cards @(
            @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{bookB, title={Book B}}'; body = '该书。' } }
            @{ tool = 'create_successor'; arguments = @{ at = 'bookB'; title = '策略互补'; body = '策略互补解释多重均衡。' } }
        )
        $bookACard, $bookBCard = $bookA[-1], $bookB[-1]
        Write-Cards @(
            @{ tool = 'edit_card'; arguments = @{ at = $bookACard; body = "多重均衡来自策略互补。对应 [[$bookBCard]]。`n`n来源：[[bookA]]" } }
            @{ tool = 'edit_card'; arguments = @{ at = $bookBCard; body = "策略互补解释多重均衡。对照 [[$bookACard]]。`n`n来源：[[bookB]]" } }
        ) | Out-Null
        $topic = Write-Cards @(@{ tool = 'create_topic'; arguments = @{ title = '概念'; body = '本主题收放跨书的同一机制。' } })
        $root = $topic[-1]
        $idea = Write-Cards @(
            @{ tool = 'create_successor'; arguments = @{ at = $root; title = '干中学提高生产率'; body = "累计产量提高时单位成本下降。`n`n来源：[[bookA]]" } }
        )
        $ideaCard = $idea[-1]
        $side = Write-Cards @(
            @{ tool = 'create_successor'; arguments = @{ at = $ideaCard; kind = 'side'; title = '规模报酬也产生多重均衡'; body = '规模报酬递增时，厂商的最优选择不唯一。' } }
        )
        $sideCard = $side[-1]
        $model = Write-Cards @(
            @{ tool = 'create_successor'; arguments = @{ at = $ideaCard; type = 'model'; level = 'concept'; title = '策略互补模型'; body = '本模型追问：策略互补为什么产生多重均衡？' } }
        )
        $modelCard = $model[-1]
        Write-Cards @(
            @{ tool = 'create_successor'; arguments = @{ at = $modelCard; type = 'model'; facet = '设定'; emphasis = 'core'; title = '策略互补的设定'; body = "依赖 [[$bookACard]]：多重均衡来自策略互补。" } }
        ) | Out-Null
        $hub = Write-Cards @(
            @{ tool = 'create_successor'; arguments = @{ at = $modelCard; kind = 'side'; type = 'hub'; level = 'line'; title = '策略互补这条线'; body = "## 主线`n- [[$ideaCard]]：干中学提高生产率。`n- [[$bookBCard]]：策略互补解释多重均衡。" } }
        )
        $index = Write-Cards @(@{ tool = 'create_topic'; arguments = @{ title = '索引'; body = '本主题收放代理工作区的入口。' } })
        $register = Write-Cards @(
            @{ tool = 'create_successor'; arguments = @{ at = $index[-1]; type = 'hub'; level = 'register'; title = '索引：代理工作区的入口'; body = "## 词条`n- 多重均衡：[[$bookACard]] [[$bookBCard]] [[$sideCard]]`n- 学习：[[$ideaCard]] [[$bookBCard]]" } }
        )
        # One kept Spread, so the Table is not empty before the reader keeps one from Ask.
        $spreadDoc = @{ current = 'fixture'; spreads = @(@{ id = 'fixture'; name = '资格检查'; question = '这套卡说明什么？'; cards = @(@{ at = $ideaCard; x = 0; y = 0 }, @{ at = $bookBCard; x = 236; y = 0 }) }) }
        $gui = Get-Gui
        Invoke-WebRequest -Uri ($gui.url + 'api/spreads') -Method Post -UseBasicParsing -Headers @{ 'X-ZT-Token' = $gui.token } -ContentType 'application/json' -Body ($spreadDoc | ConvertTo-Json -Depth 10) | Out-Null
        $addresses = [ordered]@{
            bookA = $bookACard; bookB = $bookBCard; topic = $root; idea = $ideaCard; side = $sideCard
            model = $modelCard; hub = $hub[-1]; register = $register[-1]
        }
        Show-Checks $gui $addresses
    }
    if (-not $NoHold) {
        Write-Output 'the service stays up; press Enter to leave it running and exit this script.'
        $null = Read-Host
    }
}
