#Requires -Version 7
<#
.SYNOPSIS
  The book term table of a digest (docs/digest-card-layers-proposal.md, proposal 2). The script
  finds the candidate terms; DeepSeek chooses the terms and the words not to use as a JSON file;
  the script writes the table. Rule R6 (one term, one meaning) then checks every Card of the book
  against it (digest-audit.ps1 reads a Card of the Literature tree whose title starts with 术语表).

.DESCRIPTION
  -Staging is the digest's staging folder (manifest.json, book.zh.md, sections\). The job and its
  output live in <Staging>\terms\. Every step prints compact JSON as its last line.

    plan     writes terms\job.md: the section headings, the bold terms of the book, and the
             sentences that define a term (称为, 叫做, 定义为, 是指, 即所谓), each with its section
             file and line. Prints {"jobs":[{"id","files","prompt"}]}.
    apply    reads terms\out\terms.json, keeps at most -MaxTerms valid entries (a term, and words
             not to use that differ from it), writes <Staging>\terms.md, and creates or rewrites the
             Card 术语表 as a Side successor of <key>/1 (with -Apply). Prints the term count and the
             Card address.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('plan', 'apply')][string]$Step,
    [Parameter(Mandatory)][string]$Staging,
    [int]$MaxTerms = 60,
    [int]$MaxCandidates = 160,
    [switch]$Apply
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
$utf8 = [Text.UTF8Encoding]::new($false)
$Doc = Join-Path $PSScriptRoot 'dsh-terms-instructions.zh.md'
$manifest = Get-Content -LiteralPath (Join-Path $Staging 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
$key = [string]$manifest.citation_key
$work = Join-Path $Staging 'terms'
$out = Join-Path $work 'out'
New-Item -ItemType Directory -Force -Path $work, $out | Out-Null
function Out-Last($object) { $object | ConvertTo-Json -Depth 10 -Compress }

if ($Step -eq 'plan') {
    $sb = [Text.StringBuilder]::new()
    [void]$sb.AppendLine("# 术语任务：《$($manifest.book_title)》").AppendLine()
    [void]$sb.AppendLine("把结果写入 ``$(Join-Path $out 'terms.json')``。").AppendLine()
    [void]$sb.AppendLine('## 小节标题').AppendLine()
    foreach ($g in @($manifest.groups)) { [void]$sb.AppendLine("- 第 $($g.n) 组：$($g.chapter)：$(@($g.headings) -join '；')") }
    [void]$sb.AppendLine()
    $bold = [ordered]@{}
    $defs = [Collections.Generic.List[string]]::new()
    foreach ($file in @(Get-ChildItem -LiteralPath (Join-Path $Staging 'sections') -Filter 's*.md' -File | Sort-Object Name)) {
        $n = 0
        foreach ($line in Get-Content -LiteralPath $file.FullName -Encoding utf8) {
            $n++
            foreach ($m in [regex]::Matches($line, '\*\*([^*\n]{2,16})\*\*')) {
                $t = $m.Groups[1].Value.Trim()
                if ($t -match '[㐀-鿿]' -and -not $bold.Contains($t)) { $bold[$t] = "sections/$($file.Name):$n" }
            }
            if ($defs.Count -lt $MaxCandidates -and $line -match '[㐀-鿿]' -and $line -match '(称为|叫做|定义为|是指|即所谓|我们把)') {
                $clip = ($line -replace '\s+', ' ').Trim()
                if ($clip.Length -gt 140) { $clip = $clip.Substring(0, 140) + '…' }
                $defs.Add("- ``sections/$($file.Name):$n`` $clip")
            }
        }
    }
    # A translation often keeps the original term in brackets: 剩余控制权（residual control rights）.
    $glossed = [ordered]@{}
    $grams = @{}
    foreach ($file in @(Get-ChildItem -LiteralPath (Join-Path $Staging 'sections') -Filter 's*.md' -File | Sort-Object Name)) {
        $text = Get-Content -LiteralPath $file.FullName -Raw -Encoding utf8
        foreach ($m in [regex]::Matches($text, '([㐀-鿿]{2,10})\s*[（(]\s*([A-Za-z][A-Za-z \-]{2,40}?)\s*[)）]')) {
            $t = $m.Groups[1].Value
            if (-not $glossed.Contains($t)) { $glossed[$t] = $m.Groups[2].Value.Trim() }
        }
        # Frequent word groups: runs of 2 to 7 Chinese characters.
        foreach ($run in [regex]::Matches(([regex]::Replace($text, '\$[^$]*\$|\[[^\]]*\]|\([^)]*\)', ' ')), '[㐀-鿿]{2,}')) {
            $r = $run.Value
            for ($n = 2; $n -le 7; $n++) { for ($i = 0; $i + $n -le $r.Length; $i++) { $g = $r.Substring($i, $n); $grams[$g] = 1 + [int]$grams[$g] } }
        }
    }
    # Keep a group that a longer group does not cover (a longer group with 80% of its count covers it).
    $frequent = @($grams.GetEnumerator() | Where-Object { $_.Value -ge 6 } | Sort-Object { - $_.Value })
    $kept = @()
    foreach ($g in $frequent) {
        $covered = @($frequent | Where-Object { $_.Key.Length -gt $g.Key.Length -and $_.Key.Contains($g.Key) -and $_.Value -ge 0.8 * $g.Value }).Count
        if (-not $covered -and $g.Key -notmatch '的$|^的' -and $g.Key -notmatch '^(我们|这个|一个|因为|所以|如果|但是|可以|没有|就是|之间|以及|这种|它们|他们|其中|通过|对于|由于|这些|那么|因此|而且|并且|不是|还是|已经|只有)') { $kept += $g }
        if ($kept.Count -ge 60) { break }
    }
    [void]$sb.AppendLine('## 带原文的词（中文术语与括号中的英文）').AppendLine()
    if ($glossed.Count) { foreach ($t in @($glossed.Keys | Select-Object -First $MaxCandidates)) { [void]$sb.AppendLine("- $t（$($glossed[$t])）") } } else { [void]$sb.AppendLine('（没有）') }
    [void]$sb.AppendLine().AppendLine('## 高频词组（出现次数）').AppendLine()
    if ($kept.Count) { [void]$sb.AppendLine((@($kept | ForEach-Object { "$($_.Key)（$($_.Value)）" }) -join '、')) } else { [void]$sb.AppendLine('（没有）') }
    [void]$sb.AppendLine()
    [void]$sb.AppendLine('## 书中加粗的词').AppendLine()
    foreach ($t in @($bold.Keys | Select-Object -First $MaxCandidates)) { [void]$sb.AppendLine("- $t（``$($bold[$t])``）") }
    if (-not $bold.Count) { [void]$sb.AppendLine('（没有）') }
    [void]$sb.AppendLine().AppendLine('## 定义术语的句子').AppendLine()
    if ($defs.Count) { foreach ($d in $defs) { [void]$sb.AppendLine($d) } } else { [void]$sb.AppendLine('（没有）') }
    $job = Join-Path $work 'job.md'
    [IO.File]::WriteAllText($job, $sb.ToString(), $utf8)
    $docCopy = Join-Path $work 'dsh-terms-instructions.zh.md'
    [IO.File]::WriteAllText($docCopy, (Get-Content -LiteralPath $Doc -Raw -Encoding utf8).Replace('{{KEY}}', $key).Replace('{{STAGING}}', $Staging), $utf8)
    Out-Last ([ordered]@{ bold = $bold.Count; definitions = $defs.Count; jobs = @([ordered]@{ id = 'terms'; files = @($docCopy, $job); prompt = '按以上说明选出本书的术语，写出 JSON 文件。最后只回复一行 JSON。' }) })
    return
}

# --- apply ---------------------------------------------------------------------------------------
$file = Join-Path $out 'terms.json'
if (-not (Test-Path -LiteralPath $file)) { Out-Last ([ordered]@{ ok = $false; error = 'no terms.json' }); return }
$raw = Get-Content -LiteralPath $file -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
$seen = @{}
$terms = @()
foreach ($t in @($raw.terms)) {
    $term = ([string]$t.term).Trim()
    if (-not $term -or $seen.ContainsKey($term) -or $term -match '[|`\n]') { continue }
    $avoid = @(@($t.avoid) | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ -and $_ -ne $term -and $_ -notmatch '[|`、\n]' -and -not $term.Contains($_) } | Select-Object -Unique)
    $seen[$term] = $true
    $terms += [ordered]@{ term = $term; avoid = $avoid; section = ([string]$t.section).Trim(); meaning = ([string]$t.meaning).Trim() }
    if ($terms.Count -ge $MaxTerms) { break }
}
$table = [Text.StringBuilder]::new()
[void]$table.AppendLine("本卡是《$($manifest.book_title)》的术语表。本书的卡片用左列的术语，不用右列的词。检查脚本按本表检查规则 R6（一个术语一个意思）。").AppendLine()
[void]$table.AppendLine('术语表').AppendLine().AppendLine('| 术语 | 不用 | 意思 | 首次定义 |').AppendLine('| --- | --- | --- | --- |')
foreach ($t in $terms) { [void]$table.AppendLine("| $($t.term) | $(if ($t.avoid.Count) { $t.avoid -join '、' } else { '—' }) | $($t.meaning -replace '\|', '/') | $($t.section) |") }
[void]$table.AppendLine().AppendLine("来源：[[$key]]")
$body = $table.ToString().TrimEnd()
[IO.File]::WriteAllText((Join-Path $Staging 'terms.md'), $body, $utf8)
$title = "术语表：《$($manifest.book_title)》的 $($terms.Count) 个术语"
$address = $null
if ($Apply) {
    $tree = @(Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $key } }))[0]
    $existing = @($tree.cards | Where-Object { [string]$_.title -match '^术语表' } | Select-Object -First 1)
    $call = if ($existing.Count) { @{ tool = 'edit_card'; arguments = @{ at = [string]$existing[0].address; title = $title; body = $body } } }
    else { @{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = $title; body = $body } } }
    $lines = @((@($call) | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "zt: $($lines -join ' | ')" }
    $address = (@($lines | Where-Object { $_ -match '^\d+ ok ' })[0] -split ' ok ', 2)[1].Trim()
}
Out-Last ([ordered]@{ ok = $true; terms = $terms.Count; with_avoid = @($terms | Where-Object { $_.avoid.Count }).Count; card = $address; file = (Join-Path $Staging 'terms.md') })
