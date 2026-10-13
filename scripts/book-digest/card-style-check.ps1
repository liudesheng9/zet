#Requires -Version 7
<#
.SYNOPSIS
  Check Card text against the Card language rules of the self-sufficient models proposal
  (ASD-STE100 adapted for Chinese). The script never changes text.

.DESCRIPTION
  Checks these rules mechanically, in titles and bodies. Formulas, Links, charts, code, tables,
  images, HTML, and quotations (lines that start with `>`) are not checked.

    R2  Sentence length: at most 40 characters in a description, at most 30 in a numbered step of
        a Derivation part (facet starts with 推导). Formulas and Links do not count. A sentence
        with no Chinese is counted in words: at most 25, or 20 in a step.
    R3  No `；` (and no `;`).
    R4  Active voice: a sentence with `被` names no actor. Use `被` only when the book names none.
    R5  A verb for an action: no `进行` / `加以` / `予以` / `作出` / `做出` before a verb.
    R6  One term, one meaning: a word the model's term table rejects (the setting's 术语表).
    R7  At most two `的` in one noun phrase: three `的` within 16 characters with no punctuation.
    R9  No hedge stacking and no praise words.
    R12 A title states one claim in at most 40 characters (formulas do not count), with no `；`.

  The term table is a Markdown table under a line that holds `术语表`, with the term in the
  first column and the words not to use in the second, separated by `、`:

    术语表
    | 术语 | 不用 |
    | --- | --- |
    | 清算 | 卖资产、变卖资产 |

  Input, one of:
    -Model <address>      every Card of that model (its Model Card and parts); the term table is
                          read from the model's setting (get_model build.setting).
    -Addresses <a,b,...>  those Cards; the term table comes from -Setting, or from the setting of
                          the model the first Card belongs to.
    -Json <file>          offline: a JSON array of {"address","title","body","facet","line"}; the
                          term table comes from the entry whose address is -Setting.

  A shared part (`line` "shared" in the build order) follows the term table of its home model, so
  R6 skips it here; its home model checks it.

  Output: one line per fault, `<address> R<n> <sentence>`, then `faults: <count>`.
#>
[CmdletBinding(DefaultParameterSetName = 'Model')]
param(
    [Parameter(ParameterSetName = 'Model', Mandatory)][string]$Model,
    [Parameter(ParameterSetName = 'Addresses', Mandatory)][string]$Addresses,
    [Parameter(ParameterSetName = 'Json', Mandatory)][string]$Json,
    [string]$Setting = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)

$cjk = '[㐀-鿿豈-﫿]'
$hedges = @(
    '值得注意的是', '需要注意的是', '在某种程度上', '某种意义上', '在一定程度上', '不难看出', '显而易见',
    '毋庸置疑', '众所周知', '可以说', '精妙', '巧妙', '深刻', '精彩', '极其重要', '非常重要', '十分重要', '优雅'
)

. (Join-Path $PSScriptRoot 'zt-calls.ps1')

# --- the Cards to check ----------------------------------------------------------------------------
$cards = @()
$script:otherTerms = @()
switch ($PSCmdlet.ParameterSetName) {
    'Json' {
        $cards = @(Get-Content -LiteralPath $Json -Raw -Encoding utf8 | ConvertFrom-Json -DateKind String)
        $script:otherTerms = @($cards | Where-Object { $_.line -eq 'shared' } | ForEach-Object { [string]$_.address })
    }
    'Model' {
        $structure = @(Invoke-Zt @(@{ tool = 'get_model'; arguments = @{ at = $Model } }))[0]
        if (-not $structure.head) { throw "``$Model`` belongs to no model" }
        $head = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $structure.head.address } }))[0]
        $cards = @([pscustomobject]@{ address = $head.address; title = $head.title; body = $head.body; facet = $null }) +
            @($structure.parts | ForEach-Object { [pscustomobject]@{ address = $_.address; title = $_.title; body = $_.body; facet = $_.facet } })
        if (-not $Setting -and $structure.build) { $Setting = [string]$structure.build.setting }
        # A shared part follows the term table of its home model, not this one.
        $script:otherTerms = @($structure.build.order | Where-Object { $_.line -eq 'shared' } | ForEach-Object { [string]$_.address })
    }
    'Addresses' {
        $list = @($Addresses -split '[,\s]+' | Where-Object { $_ })
        $read = @(Invoke-Zt @($list | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } }))
        $cards = @($read | ForEach-Object { [pscustomobject]@{ address = $_.address; title = $_.title; body = $_.body; facet = $_.facet } })
        if (-not $Setting -and $read.Count -and $read[0].model -and $read[0].model.build) { $Setting = [string]$read[0].model.build.setting }
    }
}

# --- the term table --------------------------------------------------------------------------------
function Read-Terms([string]$body) {
    $terms = [ordered]@{}
    $inTable = $false
    foreach ($line in ($body -split "`n")) {
        if ($line -match '术语表') { $inTable = $true; continue }
        if (-not $inTable) { continue }
        $t = $line.Trim()
        if (-not $t) { if ($terms.Count) { break } else { continue } }
        if (-not $t.StartsWith('|')) { if ($terms.Count) { break } else { continue } }
        $cells = @($t.Trim('|') -split '\|' | ForEach-Object { $_.Trim() })
        if ($cells.Count -lt 2 -or $cells[0] -match '^-+$|^:?-+:?$' -or $cells[0] -eq '术语') { continue }
        $avoid = @($cells[1] -split '[、,，/]' | ForEach-Object { $_.Trim(' ', '`', '“', '”') } |
            Where-Object { $_ -and $_ -notmatch '^[-—无]+$' })
        if ($avoid.Count) { $terms[$cells[0]] = $avoid }
    }
    $terms
}
$terms = [ordered]@{}
if ($Setting) {
    $settingCard = $cards | Where-Object { $_.address -eq $Setting } | Select-Object -First 1
    if (-not $settingCard -and $PSCmdlet.ParameterSetName -ne 'Json') {
        $settingCard = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $Setting } }))[0]
    }
    if ($settingCard) { $terms = Read-Terms ([string]$settingCard.body) }
}

# --- prose: what the rules apply to ----------------------------------------------------------------
# Strip what the rules do not cover; keep a placeholder so a formula still splits nothing.
function Get-Plain([string]$text) {
    $t = [regex]::Replace($text, '\$\$[\s\S]*?\$\$', 'F')
    $t = [regex]::Replace($t, '\$[^$\n]+\$', 'F')
    $t = [regex]::Replace($t, '\[\[[^\]]+\]\]', '')
    $t = [regex]::Replace($t, '`[^`\n]*`', 'C')
    $t = $t -replace '\*\*|__', ''
    $t
}
function Measure-Sentence([string]$sentence) {
    $plain = (Get-Plain $sentence) -replace '[\s“”"「」（）()《》]', ''
    $plain = $plain -replace 'F', '' -replace 'C', ''
    if ($sentence -match $cjk) { return @{ size = $plain.Length; unit = 'characters' } }
    return @{ size = @((Get-Plain $sentence) -split '\s+' | Where-Object { $_ -match '\w' }).Count; unit = 'words' }
}
# Lines of prose with whether each is a numbered step.
function Get-ProseLines([string]$body) {
    $out = [Collections.Generic.List[object]]::new()
    $fence = $false
    $display = $false
    foreach ($raw in ($body -split "`n")) {
        $line = $raw.TrimEnd()
        $trim = $line.Trim()
        if ($trim -match '^(```|~~~)') { $fence = -not $fence; continue }
        if ($fence) { continue }
        if ($trim -eq '$$') { $display = -not $display; continue }
        if ($display) { continue }
        if (-not $trim -or $trim.StartsWith('|') -or $trim.StartsWith('>') -or $trim.StartsWith('<') -or
            $trim.StartsWith('![') -or $trim -match '^#{1,6}\s' -or $trim -match '^\$\$.*\$\$$') { continue }
        $step = $trim -match '^\d+[.、)]\s*'
        $text = $trim -replace '^([-*+]|\d+[.、)])\s*', ''
        $out.Add([pscustomobject]@{ text = $text; step = $step })
    }
    $out
}
function Split-Sentences([string]$text) {
    # Hide formulas and Links so their punctuation splits nothing.
    $hidden = [Collections.Generic.List[string]]::new()
    $masked = [regex]::Replace($text, '\$\$[\s\S]*?\$\$|\$[^$\n]+\$|\[\[[^\]]+\]\]', {
        param($m)
        $hidden.Add($m.Value)
        "§$($hidden.Count - 1)§"
    })
    $parts = [regex]::Split($masked, '(?<=[。！？!?])|(?<=\.)\s+')
    foreach ($part in $parts) {
        $s = $part.Trim()
        if (-not $s) { continue }
        [regex]::Replace($s, '§(\d+)§', { param($m) $hidden[[int]$m.Groups[1].Value] })
    }
}

# --- the checks ------------------------------------------------------------------------------------
$faults = [Collections.Generic.List[string]]::new()
function Add-Fault($address, $rule, $sentence) {
    $clip = ($sentence -replace '\s+', ' ').Trim()
    if ($clip.Length -gt 90) { $clip = $clip.Substring(0, 89) + '…' }
    $faults.Add("$address R$rule $clip")
}
function Test-Sentence($address, [string]$sentence, [bool]$step) {
    $plain = Get-Plain $sentence
    $measure = Measure-Sentence $sentence
    $limit = if ($measure.unit -eq 'characters') { if ($step) { 30 } else { 40 } } else { if ($step) { 20 } else { 25 } }
    if ($measure.size -gt $limit) { Add-Fault $address 2 "($($measure.size) $($measure.unit) > $limit) $sentence" }
    if ($plain -match '[；;]') { Add-Fault $address 3 $sentence }
    if ($plain -match '被(?!告|保险|动|子)') { Add-Fault $address 4 $sentence }
    if ($plain -match "(进行|加以|予以|作出|做出)$cjk") { Add-Fault $address 5 $sentence }
    foreach ($term in $(if ($address -in $script:otherTerms) { @() } else { $terms.Keys })) {
        foreach ($word in $terms[$term]) {
            # A rejected word inside the term itself is the term, not the word.
            if ($plain.Replace($term, '').Contains($word)) {
                Add-Fault $address 6 "uses ``$word`` for ``$term``: $sentence"
            }
        }
    }
    if ($plain -match '的[^，。、：；！？,.;:!?\s的]{0,7}的[^，。、：；！？,.;:!?\s的]{0,7}的') { Add-Fault $address 7 $sentence }
    foreach ($word in $hedges) {
        if ($plain.Contains($word)) { Add-Fault $address 9 "``$word``: $sentence"; break }
    }
}

foreach ($card in $cards) {
    $address = [string]$card.address
    $title = [string]$card.title
    $derivation = ([string]$card.facet).StartsWith('推导') -or ([string]$card.facet) -match '^(?i)derivation'
    if ($title) {
        $measure = Measure-Sentence $title
        $limit = if ($measure.unit -eq 'characters') { 40 } else { 25 }
        if ($measure.size -gt $limit) { Add-Fault $address 12 "($($measure.size) $($measure.unit) > $limit) $title" }
        if ((Get-Plain $title) -match '[；;]') { Add-Fault $address 12 "two claims: $title" }
    }
    foreach ($line in Get-ProseLines ([string]$card.body)) {
        foreach ($sentence in Split-Sentences $line.text) {
            Test-Sentence $address $sentence ($derivation -and $line.step)
        }
    }
}
$faults | ForEach-Object { Write-Output $_ }
Write-Output "faults: $($faults.Count)"
