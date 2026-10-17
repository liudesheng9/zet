#Requires -Version 7.5
<#
.SYNOPSIS
  The book term table of a digest (docs/digest-card-layers-proposal.md, proposal 2). The script
  finds the candidate terms; DeepSeek chooses the terms and the words not to use as a JSON file;
  the script writes the table. Rule R6 (one term, one meaning) then checks every Card of the book
  against it (the audit reads a Card of the Literature tree whose title starts with 术语表).

  This script forwards to `zt-digest terms` (tools/zt-digest/src/terms.rs, installed beside zt.exe
  by scripts/install-zt-digest.ps1), which does the work in one process. Call `zt-digest terms`
  directly when you can: it takes the same parameters, writes the same files, and prints the same
  lines.

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

  Parameters: -Step plan|apply and -Staging <folder> (required), -MaxTerms 60, -MaxCandidates 160,
  -Apply. The tool reads dsh-terms-instructions.zh.md from ZT_DIGEST_SCRIPTS, which this script
  sets to its own folder. ZT_DIGEST names another zt-digest build.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# The folder of the instruction file.
$env:ZT_DIGEST_SCRIPTS = $PSScriptRoot
# `pwsh -File` and `&` hand `-Name:value` over as two arguments (a switch value as a Boolean); the
# tool takes `-Name:value`.
$pass = [Collections.Generic.List[string]]::new()
foreach ($a in $args) {
    $last = if ($pass.Count) { $pass[$pass.Count - 1] } else { '' }
    if ($last -match '^-[A-Za-z]\w*:$') { $pass[$pass.Count - 1] = $last + $(if ($a -is [array]) { $a -join ',' } else { "$a" }) }
    elseif ($a -is [bool] -and $last -match '^-[A-Za-z]\w*$') { $pass[$pass.Count - 1] = "${last}:$a" }
    elseif ($a -is [array]) { $pass.Add($a -join ',') }
    else { $pass.Add("$a") }
}
& $exe terms @pass
exit $LASTEXITCODE
