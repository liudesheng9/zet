#Requires -Version 7.5
<#
.SYNOPSIS
  The book's skeleton (docs/digest-card-layers-proposal.md, proposal 7): the Literature Card index,
  the structure Card, and the bodies of the book's Topics. The script builds each frame from the
  manifest and the trees; one DeepSeek job writes the short texts as JSON; Opus gives only the
  thesis (the structure Card's title).

  This script forwards to `zt-digest skeleton` (tools/zt-digest/src/skeleton.rs, installed beside
  zt.exe by scripts/install-zt-digest.ps1), which does the work in one process, with the audit
  in-process. Call `zt-digest skeleton` directly when you can: it takes the same parameters,
  writes the same files, and prints the same lines.

.DESCRIPTION
  -Staging is the digest's staging folder; -Work is the skeleton folder (job.md, out\skeleton.json).
  -Audit names an audit.json of the book (organize-survey writes one); without it the tool runs
  the audit into <Work>\audit. Every step prints compact JSON as its last line.

    plan     writes job.md: one line per group (anchor, chapter, anchor title and body), the
             book's Model Cards with their levels, its Event Cards, and the Topics where the book
             holds most Cards (root body and Card titles). Its SELF-CHECK line names
             `zt-digest skeleton -Step apply ...` without -Apply. Prints the job.
    apply    reads out\skeleton.json, checks it (every anchor, model, event, and Topic it names
             belongs to the book; every Link exists), and with -Apply writes:
               - the structure Card (a Side successor of <key>/1; title = -Thesis, or the thesis
                 in -Decisions, or the title it has): the argument by group, the models, the events;
               - the Literature Card body: one index line per group, a line linking the structure
                 Card, and the document line (原书文档：…) it had;
               - each Topic body, keeping every line that links a Hub Card.

  Parameters: -Step plan|apply, -Staging <folder>, and -Work <folder> (required), -Audit,
  -Thesis, -Decisions, -Apply. The tool reads dsh-skeleton-instructions.zh.md from
  ZT_DIGEST_SCRIPTS, which this script sets to its own folder. ZT_DIGEST names another zt-digest
  build.
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
& $exe skeleton @pass
exit $LASTEXITCODE
