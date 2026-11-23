#Requires -Version 7
# The stand-in for the statement finder's DeepSeek jobs (owner, 2026-11-21: DeepSeek finds the
# statements, no pattern does), on answer schema 2 (docs/math-digest-p1a-design.md §2). Dot-source
# it, run `math -Step index`, then call Write-FinderAnswers with the statements a finder would list.
# Each entry names its kind, label, name and a `quote` of its first line; `proofQuote` and `endQuote`
# find the proof line and the last line after it. The stand-in writes one object fragment per entry
# found in a job's sections (the statement up to the proof, the proof as one inline step), adds a
# `contains` relation where one entry's lines enclose another's, then runs the job's real SELF-CHECK
# and declares every passage it lists as unclaimed as an exposition, as the finder is told to.
function Get-FinderLine([string[]]$Lines, [string]$Quote, [int]$After = 0) {
    $q = $Quote -replace '\s', ''
    for ($i = $After; $i -lt $Lines.Count; $i++) {
        if (($Lines[$i] -replace '\s', '').Contains($q)) { return $i + 1 }
    }
    return 0
}

function Invoke-FinderCheck([string]$Staging, [string]$Job) {
    if ($env:ZT_DIGEST) {
        $lines = @(& $env:ZT_DIGEST math -Step index-check -Staging $Staging -Job $Job 2>&1)
    } else {
        $compiler = Join-Path $PSScriptRoot '..\math-compiler.ps1'
        $lines = @(pwsh -NoProfile -File $compiler -Step index-check -Staging $Staging -Job $Job 2>&1)
    }
    , @($lines | ForEach-Object { "$_" })
}

function Get-QuoteOf([string]$Text) {
    # A quote from inside the span: its text without whitespace, at most 20 characters; a span
    # shorter than four characters is quoted whole.
    $squeezed = $Text -replace '\s', ''
    if ($squeezed.Length -le 20) { return $squeezed }
    return $squeezed.Substring(0, 20)
}

function Write-FinderAnswers([string]$Staging, [object[]]$Entries) {
    $utf8 = [Text.UTF8Encoding]::new($false)
    $work = Join-Path $Staging 'math\index'
    $plan = Get-Content -Raw -LiteralPath (Join-Path $work 'plan.json') | ConvertFrom-Json
    $placed = @{}
    $results = @('theorem', 'proposition', 'lemma', 'corollary')
    foreach ($job in @($plan.jobs)) {
        $fragments = [Collections.Generic.List[object]]::new()
        $ranges = [Collections.Generic.List[object]]::new()
        foreach ($e in $Entries) {
            foreach ($file in @($job.sections)) {
                $lines = @(Get-Content -LiteralPath (Join-Path $Staging "sections\$file") -Encoding utf8)
                $line = Get-FinderLine $lines $e.quote
                if (-not $line) { continue }
                $proof = if ($e.proofQuote) { Get-FinderLine $lines $e.proofQuote $line } else { 0 }
                $end = if ($e.endQuote) { Get-FinderLine $lines $e.endQuote $line } elseif ($proof) { $proof } else { $line }
                $statementEnd = if ($proof) { $proof - 1 } else { $end }
                while ($statementEnd -gt $line -and -not $lines[$statementEnd - 1].Trim()) { $statementEnd-- }
                $fragment = [ordered]@{
                    role = 'object'; kind = $e.kind; label = "$($e.label)"; name = $e.name
                    spans = @([ordered]@{ section = $file; line = $line; end = $statementEnd; quote = $e.quote })
                }
                if ($results -contains $e.kind) {
                    if ($proof) {
                        $fragment.proof = [ordered]@{ status = 'inline'; steps = @([ordered]@{ label = ''; spans = @([ordered]@{ section = $file; line = $proof; end = $end; quote = $e.proofQuote }) }) }
                    } else {
                        $fragment.proof = [ordered]@{ status = 'source_omits'; reason = 'the book gives no proof' }
                    }
                }
                if ($e.kind -in @('example', 'remark', 'notation')) {
                    $fragment.destination = if ($e.belongs_to) { [ordered]@{ to = 'unit'; target = [ordered]@{ label = "$($e.belongs_to)" } } } else { [ordered]@{ to = 'narrative' } }
                }
                $rels = @($e.cites | Where-Object { $_ } | ForEach-Object { [ordered]@{ rel = 'cites'; target = [ordered]@{ label = "$_" } } })
                if ($rels.Count) { $fragment.relations = $rels }
                $fragments.Add($fragment)
                $ranges.Add([pscustomobject]@{ fragment = $fragment; file = $file; line = $line; end = $end; kind = $e.kind; label = "$($e.label)"; name = $e.name })
                $placed[$e.name] = $true
                break
            }
        }
        # One entry's lines that enclose another's: the outer one contains it.
        foreach ($outer in $ranges) {
            foreach ($inner in $ranges) {
                if ($outer -eq $inner -or $outer.file -ne $inner.file) { continue }
                if ($outer.line -le $inner.line -and $inner.end -le $outer.end -and -not ($outer.line -eq $inner.line -and $outer.end -eq $inner.end)) {
                    $target = if ($inner.label) { [ordered]@{ namespace = $inner.kind; label = $inner.label } } else { [ordered]@{ namespace = $inner.kind; name = $inner.name } }
                    $outer.fragment.relations = @($outer.fragment.relations | Where-Object { $_ }) + @([ordered]@{ rel = 'contains'; target = $target })
                }
            }
        }
        $answerFile = Join-Path $work "out\$($job.id).json"
        $answer = [ordered]@{ schema = 2; fragments = $fragments.ToArray() }
        [IO.File]::WriteAllText($answerFile, ($answer | ConvertTo-Json -Depth 12), $utf8)
        # The passages no fragment claims become expositions, from the SELF-CHECK's own list.
        $faults = Invoke-FinderCheck $Staging $job.id
        foreach ($f in $faults) {
            if ($f -match '^(\S+?):(\d+):(\d+)-(\d+):(\d+) \(\w+\) is claimed by no fragment') {
                $file = $Matches[1]; $l1 = [int]$Matches[2]; $c1 = [int]$Matches[3]; $l2 = [int]$Matches[4]; $c2 = [int]$Matches[5]
                $lines = @(Get-Content -LiteralPath (Join-Path $Staging "sections\$file") -Encoding utf8)
                $text = if ($l1 -eq $l2) { $lines[$l1 - 1].Substring($c1, $c2 - $c1) } else { $lines[$l1 - 1].Substring($c1) }
                $fragments.Add([ordered]@{ role = 'exposition'; spans = @([ordered]@{ section = $file; line = $l1; end = $l2; col = $c1; end_col = $c2; quote = (Get-QuoteOf $text) }) })
            } elseif ($f -match '^(\S+?):(\d+)-(\d+) \(\w+\) is claimed by no fragment') {
                $file = $Matches[1]; $l1 = [int]$Matches[2]; $l2 = [int]$Matches[3]
                $lines = @(Get-Content -LiteralPath (Join-Path $Staging "sections\$file") -Encoding utf8)
                $text = ($lines[($l1 - 1)..($l2 - 1)] -join "`n")
                $fragments.Add([ordered]@{ role = 'exposition'; spans = @([ordered]@{ section = $file; line = $l1; end = $l2; quote = (Get-QuoteOf $text) }) })
            }
        }
        $answer = [ordered]@{ schema = 2; fragments = $fragments.ToArray() }
        [IO.File]::WriteAllText($answerFile, ($answer | ConvertTo-Json -Depth 12), $utf8)
    }
    $missing = @($Entries | Where-Object { -not $placed[$_.name] } | ForEach-Object name)
    if ($missing.Count) { throw "the finder stand-in found no line for: $($missing -join ', ')" }
}
