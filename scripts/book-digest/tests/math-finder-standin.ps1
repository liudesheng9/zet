#Requires -Version 7
# The stand-in for the statement finder's DeepSeek jobs (owner, 2026-11-21: DeepSeek finds the
# statements, no pattern does). Dot-source it, run `math -Step index`, then call
# Write-FinderAnswers with the statements a finder would list. Each entry names its kind, label,
# name and a `quote` of its first line; `proofQuote` and `endQuote` find the proof line and the last
# line after it. The stand-in finds the line numbers in the sections of the job that holds the quote
# and writes one answer file per planned job, so `index-apply` builds the index from them.
function Get-FinderLine([string[]]$Lines, [string]$Quote, [int]$After = 0) {
    $q = $Quote -replace '\s', ''
    for ($i = $After; $i -lt $Lines.Count; $i++) {
        if (($Lines[$i] -replace '\s', '').Contains($q)) { return $i + 1 }
    }
    return 0
}

function Write-FinderAnswers([string]$Staging, [object[]]$Entries) {
    $utf8 = [Text.UTF8Encoding]::new($false)
    $work = Join-Path $Staging 'math\index'
    $plan = Get-Content -Raw -LiteralPath (Join-Path $work 'plan.json') | ConvertFrom-Json
    $placed = @{}
    foreach ($job in @($plan.jobs)) {
        $statements = [Collections.Generic.List[object]]::new()
        foreach ($e in $Entries) {
            foreach ($file in @($job.sections)) {
                $lines = @(Get-Content -LiteralPath (Join-Path $Staging "sections\$file") -Encoding utf8)
                $line = Get-FinderLine $lines $e.quote
                if (-not $line) { continue }
                $proof = if ($e.proofQuote) { Get-FinderLine $lines $e.proofQuote $line } else { 0 }
                $end = if ($e.endQuote) { Get-FinderLine $lines $e.endQuote $line } elseif ($proof) { $proof } else { $line }
                $statements.Add([ordered]@{
                        kind = $e.kind; label = "$($e.label)"; name = $e.name; section = $file
                        line = $line; end = $end; proof = $proof; quote = $e.quote
                        cites = @($e.cites | Where-Object { $_ }); belongs_to = "$($e.belongs_to)"
                    })
                $placed[$e.name] = $true
                break
            }
        }
        $answer = [ordered]@{ statements = $statements.ToArray() }
        [IO.File]::WriteAllText((Join-Path $work "out\$($job.id).json"), ($answer | ConvertTo-Json -Depth 8), $utf8)
    }
    $missing = @($Entries | Where-Object { -not $placed[$_.name] } | ForEach-Object name)
    if ($missing.Count) { throw "the finder stand-in found no line for: $($missing -join ', ')" }
}
