#Requires -Version 7.5
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST, [string]$Checker = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "content-corruption-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
if (-not $scratch.StartsWith($tempRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the test temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
if (-not $Checker) { $Checker = (Get-Command zt-check -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition,$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Check-Entries($entries) {
    $file = Join-Path $scratch 'entries.json'
    [IO.File]::WriteAllText($file,($entries | ConvertTo-Json -Depth 12 -AsArray),$utf8)
    $lines = @(& $Checker --json $file 2>&1)
    # An offline draft is an incomplete evaluation by design (exit 2: placement needs a
    # service preview), so the `faults:` line, not the exit code, is the evidence that
    # the check completed; an invalid invocation prints no such line.
    if (-not @($lines | Where-Object { $_ -match '^faults: \d+$' }).Count) { throw "checker did not run: $($lines -join ' | ')" }
    ,$lines
}
try {
    $title = '税率提高会限制交易'
    $paragraph = '税率上升使交易收缩。买方承担部分成本。'
    $duplicate = Check-Entries @(@{address='1/1';title=$title;body="$paragraph`n`n$paragraph`n`n$paragraph"})
    $r18 = @($duplicate | Where-Object { $_ -match '^1/1 R18 ' })
    Assert ($r18.Count -eq 2 -and $r18[0].Contains('1') -and $r18[0].Contains('3') -and $r18[1].Contains('5')) "exact duplicate paragraphs report every repeated location: $($r18 -join ' | ')"
    $multi = Check-Entries @(@{address='1/2';title=$title;body="  第一行陈述交易收缩。`r`n第二行陈述成本分担。  `r`n`r`n第一行陈述交易收缩。`n第二行陈述成本分担。"})
    Assert (@($multi | Where-Object { $_ -match '^1/2 R18 ' }).Count -eq 1) 'only line endings and outer paragraph whitespace are normalized'
    $sourceProse = Check-Entries @(@{address='1/6';title=$title;body="来源：原书。`n$paragraph`n`n来源：原书。`n$paragraph"})
    Assert (@($sourceProse | Where-Object { $_ -match '^1/6 R18 ' }).Count -eq 1) 'a source label cannot hide a repeated substantive prose paragraph'
    $near = Check-Entries @(@{address='1/3';title=$title;body="本卡明确陈述一个可检验的机制。`n`n本卡明确陈述 一个可检验的机制。`n`n向向前看。`n`n价价变化。"})
    Assert (@($near | Where-Object { $_ -match ' R1[89] ' }).Count -eq 0) 'interior whitespace and uncertain repeated characters are not fuzzy corruption rules'
    $continuation = Check-Entries @(@{address='1/7';title=$title;body="税率改变交易选择。`n    这里给出第一种条件。`n`n税率改变交易选择。`n    这里给出第二种条件。"})
    Assert (@($continuation | Where-Object { $_ -match ' R18 ' }).Count -eq 0) 'indented continuation text remains part of the exact prose paragraph'
    $titles = Check-Entries @(@{address='1/4';title=$title;body="独立的正文。`n`n# $title`n`n# $title ###"})
    $r19 = @($titles | Where-Object { $_ -match '^1/4 R19 ' })
    Assert ($r19.Count -eq 2 -and $r19[0].Contains('3') -and $r19[1].Contains('5')) 'every authored repeated title names its body line'
    $excluded = @'
```text
PARAGRAPH

PARAGRAPH
# TITLE
```

~~~mermaid
PARAGRAPH

PARAGRAPH
# TITLE
~~~

$$
PARAGRAPH

PARAGRAPH
# TITLE
$$

\[
PARAGRAPH

PARAGRAPH
# TITLE
\]

<blockquote>
PARAGRAPH

PARAGRAPH
# TITLE
</blockquote>

<blockquote>
<blockquote>
引用的开头。
</blockquote>
PARAGRAPH

PARAGRAPH
# TITLE
</blockquote>

\begin{align*}
PARAGRAPH

PARAGRAPH
# TITLE
\end{align*}

> PARAGRAPH
PARAGRAPH

> PARAGRAPH
# TITLE

“PARAGRAPH”

“PARAGRAPH”

$x+y=z$

$x+y=z$

    PARAGRAPH

    PARAGRAPH
    # TITLE

<!-- zt:reverse-links -->
PARAGRAPH

PARAGRAPH
# TITLE
'@
    $excluded = $excluded.Replace('PARAGRAPH',$paragraph).Replace('TITLE',$title).Replace('\\[','\[').Replace('\\]','\]')
    $clean = Check-Entries @(@{address='1/5';title=$title;body=$excluded})
    Assert (@($clean | Where-Object { $_ -match ' R1[89] ' }).Count -eq 0) 'code/chart fences, formulas, explicit quotations and generated Reverse content are excluded'
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $body = "$paragraph`n`n$paragraph`n`n# $title`n`n来源：[[CorruptionBook]]。"
    $null = Invoke-Zt @(
        @{tool='create_literature';arguments=@{bibtex='@book{CorruptionBook,title={Corruption checks}}'}},
        @{tool='create_successor';arguments=@{at='CorruptionBook';title='第一组'}},
        @{tool='create_successor';arguments=@{at='CorruptionBook/1';kind='side';title=$title;body=$body}}
    )
    $stored = Invoke-Zt @(@{tool='get_card';arguments=@{at='CorruptionBook/1|a'}})
    $direct = Check-Entries @(@{address=$stored.address;title=$stored.title;body=$stored.body})
    $out = Join-Path $scratch 'audit'
    $audit = @(& $Digest audit -CitationKey CorruptionBook -Out $out -SkipModels 2>&1)
    Assert ($LASTEXITCODE -eq 0) "the actual audit runs the shared corruption rules: $($audit[-1])"
    $auditFaults = @(Get-Content (Join-Path $out 'audit-faults.txt') | Where-Object { $_ -match '^CorruptionBook/1\|a R1[89] ' })
    $directFaults = @($direct | Where-Object { $_ -match '^CorruptionBook/1\|a R1[89] ' })
    Assert ($directFaults.Count -eq 2 -and ($auditFaults -join "`n") -ceq ($directFaults -join "`n")) 'the audit and standalone checker report identical corruption faults and locations'
    $report = Get-Content (Join-Path $out 'audit.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ($report.books[0].style.by_rule.R18 -eq 1 -and $report.books[0].style.by_rule.R19 -eq 1) 'the audit counts the same duplicate paragraph and title faults'
    Assert ((Invoke-Zt @(@{tool='get_card';arguments=@{at='CorruptionBook/1|a'}})).body -eq $body) 'corruption checks do not rewrite authored content'
    $work = Join-Path $scratch 'card-work'
    $plan = @(& $Digest card -Step plan -Book CorruptionBook -Work $work -NoOrphans -Sample 0 2>&1)
    Assert ($LASTEXITCODE -eq 0) "the ordinary Card cycle plans corruption checks: $($plan[-1])"
    $state = Get-Content (Join-Path $work 'state.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    Assert ('R18' -in $state.before['CorruptionBook/1|a'] -and 'R19' -in $state.before['CorruptionBook/1|a']) 'the default Card cycle includes both corruption rules in its repair work'
    $null = Invoke-Zt @(@{tool='apply_batch';arguments=@{items=@(
        @{op_key='corruption/link/1/create/topic';client_ref='topic';tool='create_topic';arguments=@{title='Draft quality'}},
        @{op_key='corruption/link/1/create/source';client_ref='source';tool='create_successor';arguments=@{at='@@topic';title=$title;body='原有正文保持独立。'}}
    )}})
    $draftFile = Join-Path $scratch 'link-draft.json'
    $draft = @(@{tool='edit_card';arguments=@{at='0/1';base_receipt='corruption/link/1/create/source';owned_paragraph=@{text="# $title"}}})
    [IO.File]::WriteAllText($draftFile,($draft | ConvertTo-Json -Depth 12 -AsArray),$utf8)
    $linkCheck = @(& $Digest call -Open $draftFile -OperationPrefix 'corruption/link/1/apply' -CheckOnly 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($linkCheck -join ' ').Contains('0/1 R19')) 'Link draft checking retains the real Card title and the shared title-corruption rule'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'all passed'
