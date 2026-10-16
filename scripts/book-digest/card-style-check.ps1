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
    R13 Card size: a Model part (a Card with a facet) holds at most -MaxSentences sentences (12)
        and -MaxChars characters (400) of prose. Formulas, Links, tables, charts, and quotations do
        not count. In a Derivation part, the numbered steps count under R14 instead, and its
        explanation (思路, 说明, 解读) counts under R17. A part over the limit splits into two or
        more parts.
    R14 A Derivation part holds at most -MaxSteps numbered steps (6). A longer step splits into a
        chain of Derivation parts: each one starts from the result of the one before.
    R15 A Derivation part that has a 思路 or a 解读 section holds the five sections 思路, 起点, 步骤,
        结果, 解读, in this order (doc/derivation-explanations-proposal.md). A part with neither is
        an older part: it raises no fault, unless -RequireExplain is set (new builds and the
        explain step set it).
    R16 思路 holds 1 to 5 sentences and 解读 1 to 6. Each sentence follows R2 (40 characters).
    R17 At most one 说明 line under each numbered step, of 1 or 2 sentences. The explanation (思路,
        说明, and 解读 together) holds at most -MaxExplainSentences sentences (16) and
        -MaxExplainChars characters (600). Formulas and Links do not count. Over the budget, the
        writer shortens the explanation: only the steps (R14) decide a split.
    R13 to R17 apply to a formal model (Elaboration level 2) and to a model with no level. A
    concept model (level 1) states itself in the Model Card and at most four parts, so its parts
    may be long. The level comes from get_model (-Model, -Addresses) or from -Level (-Json).

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
  R6 skips it here; its home model checks it. With -SkipShared (-Model only), shared parts are not
  checked at all: a build report counts only the Cards its own model owns.

  Card layer rules (doc/digest-card-layers-proposal.md). They apply only to -Json entries that
  name their `category` (idea, anchor, event; other categories get the R rules only), so the
  model compiler's checks do not change. An entry may also carry `links` (every Link target),
  `kinds` (the targets ZT reads a Relation kind for), `parent`, `inbound` (the addresses that
  link to it), `inbound_kinded` (how many Idea Cards link to it with a kind word), `book` (its
  Citation key), and `tree`. A rule whose data an entry lacks is skipped. Rules marked
  (advisory) are heuristics: they name a candidate for a review, not a sure fault.
    I1  (idea, anchor; advisory) The title states a claim: a title of 6 characters or fewer with
        no verb is a fault.
    I2  (idea) The body holds -MinIdeaChars to -MaxIdeaChars characters of prose (40 to 300).
        Over is a split candidate, under is a merge candidate.
    I3  (idea, event) A Card of a book has a source line: a Link to its Literature Card or to
        one of its anchors (`<key>/<n>`).
    I4  (idea) The Card has a kinded Link to a Card other than its parent.
    I5  (idea, event) A Card other than its parent links to the Card.
    I6  (idea; advisory) No other Card of the same tree has a very similar title (character
        bigram similarity 0.8 or more).
    E1  (event) The title holds a date: a year at least.
    E2  (event) The body holds the labels 时间, 地点, 参与者, 经过, and 来源 (`时间：…`).
    E4  (event) At least one Idea Card links to it with a kind word.
    E5  (event; advisory) No other Event Card has the same year and a similar title.
    L1  (idea, anchor, event) ZT reads a Relation kind for every Link, except a source line or a
        place in the book (a Literature Card or an anchor) and a Link after 引出 in its sentence
        (it points the other way: the linked Card's 承接 carries the relation). One line per Card
        names the Links read as "refers".
    L2  (idea, anchor, event) The sentence of a Link holds at least -MinReason characters (8)
        besides its Links and formulas. A source line (`来源：…`) is exempt.
    L3  (idea, anchor, event; advisory) A Card links the same target in two or more paragraphs.

  Output: one line per fault, `<address> <rule> <sentence>` (R2 … R14, I1 … L3), then
  `faults: <count>`.
#>
[CmdletBinding(DefaultParameterSetName = 'Model')]
param(
    [Parameter(ParameterSetName = 'Model', Mandatory)][string]$Model,
    [Parameter(ParameterSetName = 'Addresses', Mandatory)][string]$Addresses,
    [Parameter(ParameterSetName = 'Json', Mandatory)][string]$Json,
    [string]$Setting = '',
    [int]$MaxSentences = 12,
    [int]$MaxChars = 400,
    [int]$MaxSteps = 6,
    [int]$MaxExplainSentences = 16,
    [int]$MaxExplainChars = 600,
    [switch]$RequireExplain,
    [ValidateSet('', 'concept', 'formal')][string]$Level = '',
    [switch]$SkipShared,
    [int]$MinIdeaChars = 40,
    [int]$MaxIdeaChars = 300,
    [int]$MinReason = 8
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
        if (-not $Level) { $Level = [string]$structure.level }
        # A shared part follows the term table of its home model, not this one.
        $script:otherTerms = @($structure.build.order | Where-Object { $_.line -eq 'shared' } | ForEach-Object { [string]$_.address })
        if ($SkipShared) { $cards = @($cards | Where-Object { $_.address -notin $script:otherTerms }) }
    }
    'Addresses' {
        $list = @($Addresses -split '[,\s]+' | Where-Object { $_ })
        $read = @(Invoke-Zt @($list | ForEach-Object { @{ tool = 'get_card'; arguments = @{ at = $_ } } }))
        $cards = @($read | ForEach-Object { [pscustomobject]@{ address = $_.address; title = $_.title; body = $_.body; facet = $_.facet } })
        if (-not $Setting -and $read.Count -and $read[0].model -and $read[0].model.build) { $Setting = [string]$read[0].model.build.setting }
        if (-not $Level -and $read.Count -and $read[0].model) { $Level = [string]$read[0].model.level }
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
    # `被收购方`, `被投资方`: a party, not a passive (and a 被 that names the party has its actor).
    if ($plain -match "被(?!告|保险|动|子|$cjk{1,2}方)") { Add-Fault $address 4 $sentence }
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

$proseChars = @{}
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
    $sentences = 0
    $chars = 0
    $steps = 0
    # A Derivation part: its sections, the explanation (思路, 说明, 解读), and the 说明 lines per step.
    $section = ''
    $order = [Collections.Generic.List[string]]::new()
    $explain = @{ sentences = 0; chars = 0; 思路 = 0; 解读 = 0 }
    $notes = @{}
    foreach ($line in Get-ProseLines ([string]$card.body)) {
        # Get-ProseLines reads the first `*` of `**思路**` as a list marker.
        if ($derivation -and $line.text -match '^\*{0,2}(思路|起点|步骤|结果|解读)\*\*\s*$') {
            $section = $Matches[1]
            $order.Add($section)
            continue
        }
        $isStep = $derivation -and $line.step
        if ($isStep) { $steps++ }
        $isNote = $derivation -and -not $isStep -and $section -eq '步骤' -and $line.text -match '^说明\s*[：:]'
        $text = if ($isNote) { $line.text -replace '^说明\s*[：:]\s*', '' } else { $line.text }
        # The 来源 line after 解读 is not part of the explanation.
        $isExplain = $isNote -or ($derivation -and $section -in '思路', '解读' -and $line.text -notmatch '^来源\s*[：:]')
        if ($isNote) {
            if (-not $steps) { Add-Fault $address 17 "a 说明 line before the first step: $text" }
            elseif ($notes.ContainsKey($steps)) { Add-Fault $address 17 "a second 说明 line under step $($steps): $text" }
            $notes[$steps] = 0
        }
        foreach ($sentence in Split-Sentences $text) {
            Test-Sentence $address $sentence $isStep
            if ($isStep) { continue }
            $measure = Measure-Sentence $sentence
            $size = if ($measure.unit -eq 'characters') { $measure.size } else { 0 }
            if ($isExplain) {
                $explain.sentences++
                $explain.chars += $size
                if ($isNote) { $notes[$steps]++ } elseif ($sentence.Trim() -ne '待写') { $explain[$section]++ }
            } else {
                $sentences++
                $chars += $size
            }
        }
    }
    # R15 to R17: a formal model's Derivation part with an explanation, or any one with -RequireExplain.
    if ($derivation -and $Level -ne 'concept') {
        $has = @($order | Where-Object { $_ -in '思路', '解读' }).Count
        if ($has -or $RequireExplain) {
            $want = @('思路', '起点', '步骤', '结果', '解读')
            if (($order -join ',') -ne ($want -join ',')) { Add-Fault $address 15 "sections $(if ($order.Count) { $order -join ', ' } else { 'none' }); want $($want -join ', ')" }
            if ($order -contains '思路' -and $explain['思路'] -gt 5) { Add-Fault $address 16 "(思路 has $($explain['思路']) sentences > 5)" }
            if ($order -contains '解读' -and $explain['解读'] -gt 6) { Add-Fault $address 16 "(解读 has $($explain['解读']) sentences > 6)" }
            foreach ($k in @($notes.Keys | Sort-Object)) { if ($notes[$k] -gt 2) { Add-Fault $address 17 "(说明 under step $k has $($notes[$k]) sentences > 2)" } }
            if ($explain.sentences -gt $MaxExplainSentences) { Add-Fault $address 17 "($($explain.sentences) explanation sentences > $MaxExplainSentences) shorten the explanation" }
            elseif ($explain.chars -gt $MaxExplainChars) { Add-Fault $address 17 "($($explain.chars) explanation characters > $MaxExplainChars) shorten the explanation" }
        }
    }
    $proseChars[$address] = $chars
    # Card size: only a Model part of a formal model; the Model Card is the model's entry point,
    # and a concept model keeps its few parts whole.
    if ($Level -eq 'concept') { continue }
    if ([string]$card.facet) {
        if ($sentences -gt $MaxSentences) { Add-Fault $address 13 "($sentences sentences > $MaxSentences) split the part into two or more parts" }
        elseif ($chars -gt $MaxChars) { Add-Fault $address 13 "($chars characters > $MaxChars) split the part into two or more parts" }
    }
    if ($steps -gt $MaxSteps) { Add-Fault $address 14 "($steps steps > $MaxSteps) split the derivation into a chain of Derivation parts" }
}

# --- Card layer rules (I, E, L): only for entries that name their category -------------------------
function Add-Layer($address, [string]$rule, [string]$text) {
    $clip = ($text -replace '\s+', ' ').Trim()
    if ($clip.Length -gt 90) { $clip = $clip.Substring(0, 89) + '…' }
    $faults.Add("$address $rule $clip")
}
# Character bigrams of a title without formulas and Links: the base of a title similarity.
function Get-Bigrams([string]$text) {
    $plain = ((Get-Plain $text) -replace 'F|C', '') -replace '[^\p{L}\p{N}]', ''
    $set = [Collections.Generic.HashSet[string]]::new()
    for ($i = 0; $i + 1 -lt $plain.Length; $i++) { [void]$set.Add($plain.Substring($i, 2)) }
    , $set
}
function Get-Similarity($a, $b) {
    if (-not $a.Count -or -not $b.Count) { return 0 }
    $common = 0
    foreach ($x in $a) { if ($b.Contains($x)) { $common++ } }
    2.0 * $common / ($a.Count + $b.Count)
}
# A source line or a place in the book: a Literature Card (`hart1995firms`) or an anchor
# (`hart1995firms/4`). Topic trees are numbered, so `12/3` is no anchor.
function Test-BookPlace([string]$target) { $target -match '^[^\d/|][^/|]*$' -or $target -match '^[^\d/|][^/|]*/\d+$' }
# A Link that 引出 announces in its sentence: it points the other way, so the linked Card's
# 承接 carries the relation.
function Test-LeadsTo([string]$body, [string]$target) {
    foreach ($m in [regex]::Matches($body, '\[\[' + [regex]::Escape($target) + '\]\]')) {
        $start = $body.LastIndexOfAny([char[]]"。！？!?；;`n", [Math]::Max(0, $m.Index - 1)) + 1
        if ($body.Substring($start, $m.Index - $start).Contains('引出')) { return $true }
    }
    $false
}
$claimVerb = '是|会|能|让|使|要|不|没|把|被|决定|导致|取决|依赖|提高|降低|减少|增加|等于|大于|小于|高于|低于|来自|需要|带来|引起|产生|改变|保证|排除|允许|阻止|证明|说明|表明'
$layer = @($cards | Where-Object { $_.PSObject.Properties['category'] -and $_.category })
foreach ($card in $layer) {
    $address = [string]$card.address
    $category = [string]$card.category
    if ($category -notin @('idea', 'anchor', 'event')) { continue }
    $title = [string]$card.title
    $body = [string]$card.body
    $has = { param($name) [bool]$card.PSObject.Properties[$name] }
    $parent = [string]$card.parent
    $links = @($card.links | Where-Object { $_ } | ForEach-Object { [string]$_ })
    $kinds = @($card.kinds | Where-Object { $_ } | ForEach-Object { [string]$_ })
    $titlePlain = ((Get-Plain $title) -replace 'F|C', '') -replace '\s', ''
    if ($category -in @('idea', 'anchor') -and $titlePlain.Length -le 6 -and $titlePlain -notmatch $claimVerb) {
        Add-Layer $address 'I1' "(advisory) the title states no claim: $title"
    }
    if ($category -eq 'idea') {
        $size = [int]$proseChars[$address]
        if ($size -gt $MaxIdeaChars) { Add-Layer $address 'I2' "($size characters > $MaxIdeaChars) split candidate" }
        elseif ($size -lt $MinIdeaChars) { Add-Layer $address 'I2' "($size characters < $MinIdeaChars) merge candidate" }
    }
    if ($category -in @('idea', 'event') -and (& $has 'book') -and $card.book -and (& $has 'links')) {
        $book = [string]$card.book
        $source = @($links | Where-Object { $_ -eq $book -or $_ -match ('^' + [regex]::Escape($book) + '/\d+$') })
        if (-not $source.Count) { Add-Layer $address 'I3' "no source line: no Link to [[$book]] or one of its anchors" }
    }
    if ($category -eq 'idea' -and (& $has 'kinds')) {
        if (-not @($kinds | Where-Object { $_ -ne $parent }).Count) { Add-Layer $address 'I4' 'no kinded Link to a Card other than the parent' }
    }
    if ($category -in @('idea', 'event') -and (& $has 'inbound')) {
        if (-not @($card.inbound | Where-Object { $_ -and [string]$_ -ne $parent }).Count) { Add-Layer $address 'I5' 'no Card other than the parent links here' }
    }
    if ($category -eq 'event') {
        if ($titlePlain -notmatch '(1[0-9]{3}|20[0-9]{2})|[0-9一二三四五六七八九十]+\s*(年|世纪)') { Add-Layer $address 'E1' "no date in the title: $title" }
        $labels = @('时间', '地点', '参与者', '经过', '来源') | Where-Object { $body -notmatch ('(?m)^\s*[-*]?\s*(\*\*)?' + $_ + '(\*\*)?\s*[：:]') }
        if ($labels.Count) { Add-Layer $address 'E2' "missing labels: $($labels -join '、')" }
        if ((& $has 'inbound_kinded') -and [int]$card.inbound_kinded -eq 0) { Add-Layer $address 'E4' 'no Idea Card links it with a kind word' }
    }
    if (& $has 'links') {
        $refers = @($links | Where-Object { $_ -notin $kinds -and -not (Test-BookPlace $_) -and -not (Test-LeadsTo $body $_) } | Select-Object -Unique)
        if ($refers.Count) {
            $shown = @($refers | Select-Object -First 5 | ForEach-Object { "[[$_]]" }) -join ' '
            Add-Layer $address 'L1' "($($refers.Count) Links read as refers) $shown"
        }
    }
    # L2 and L3 read the body: the sentence of each Link, and the paragraphs it appears in.
    $paragraphs = @{}
    $p = 0
    foreach ($raw in ($body -split "`n")) {
        $trim = $raw.Trim()
        if (-not $trim) { $p++; continue }
        foreach ($m in [regex]::Matches($trim, '\[\[([^\]]+)\]\]')) {
            $target = $m.Groups[1].Value.Trim()
            if (-not $paragraphs.ContainsKey($target)) { $paragraphs[$target] = [Collections.Generic.HashSet[int]]::new() }
            [void]$paragraphs[$target].Add($p)
        }
    }
    foreach ($line in Get-ProseLines $body) {
        foreach ($sentence in Split-Sentences $line.text) {
            if ($sentence -notmatch '\[\[') { continue }
            if ($sentence -match '^\s*(来源|出处|Source)\s*[：:]') { continue }
            $reason = ((Get-Plain $sentence) -replace 'F|C', '') -replace '[\s\p{P}\p{S}]', ''
            # A formula states the reason too (`代入 [[…]] 的定义得 $MRTS=…$`).
            if ($reason.Length -lt $MinReason -and $sentence -notmatch '\$[^$\n]+\$') { Add-Layer $address 'L2' "($($reason.Length) characters < $MinReason) a Link with no reason: $sentence" }
        }
    }
    foreach ($target in @($paragraphs.Keys | Sort-Object)) {
        if ($paragraphs[$target].Count -ge 2) { Add-Layer $address 'L3' "(advisory) [[$target]] is linked in $($paragraphs[$target].Count) paragraphs" }
    }
}
# I6 and E5 compare Cards with each other.
foreach ($group in @($layer | Where-Object { $_.category -eq 'idea' -and $_.PSObject.Properties['tree'] -and $_.tree } | Group-Object { [string]$_.tree })) {
    $items = @($group.Group | ForEach-Object { [pscustomobject]@{ address = [string]$_.address; bigrams = (Get-Bigrams ([string]$_.title)) } })
    for ($i = 0; $i -lt $items.Count; $i++) {
        for ($j = $i + 1; $j -lt $items.Count; $j++) {
            if ((Get-Similarity $items[$i].bigrams $items[$j].bigrams) -ge 0.8) {
                Add-Layer $items[$j].address 'I6' "(advisory) a title very similar to [[$($items[$i].address)]]"
            }
        }
    }
}
$events = @($layer | Where-Object { $_.category -eq 'event' } | ForEach-Object {
    $year = if ([string]$_.title -match '(1[0-9]{3}|20[0-9]{2})') { $Matches[1] } else { '' }
    [pscustomobject]@{ address = [string]$_.address; year = $year; bigrams = (Get-Bigrams ([string]$_.title)) }
})
for ($i = 0; $i -lt $events.Count; $i++) {
    for ($j = $i + 1; $j -lt $events.Count; $j++) {
        if ($events[$i].year -and $events[$i].year -eq $events[$j].year -and (Get-Similarity $events[$i].bigrams $events[$j].bigrams) -ge 0.5) {
            Add-Layer $events[$j].address 'E5' "(advisory) the same event as [[$($events[$i].address)]]?"
        }
    }
}
$faults | ForEach-Object { Write-Output $_ }
Write-Output "faults: $($faults.Count)"
