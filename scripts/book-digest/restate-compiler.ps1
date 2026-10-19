#Requires -Version 7.5
<#
.SYNOPSIS
  Resolve new restatements without rewriting a finished model (issue #107,
  docs/luhmann-comparison-implementation-design.md §4.5 and §5.4). This script forwards to
  `zt-digest restate` (tools/zt-digest/src/restate.rs); callers may call the tool directly.

.DESCRIPTION
  -Staging is the prepared run's staging folder (manifest.json with the book's citation_key). -Cards
  names the run's new Topic Idea Cards, one address per line; they are the only Cards this review may
  rewrite. The work folder (default <Staging>\restate) holds everything else. Every step prints
  compact JSON as its last line; prepare also prints the judge job it wants written.

    prepare -Cards <f>   reads the new Cards, the book's anchors and Model Cards/parts, and earlier
                         Topic Idea Cards, and selects at most ten comparison pairs per new Card by
                         the current relatedness score, normalized title similarity and exact
                         shared-formula evidence. It freezes restate\evidence.json, the retained
                         endpoint bodies and the judge job restate\rj-01.md. Read calls only.
    judge -File <f>      validates one reviewer answer file against the frozen evidence and writes
                         restate\judged.json. Each pair is judged once; a `same` verdict names the
                         canonical statement, its scope, and the new Card's rewrite (or an explicit
                         pending existing-Card change). A refused or absent answer resolves nothing.
    apply -OperationPrefix <run/unit/rev/step>
                         writes the accepted new-Card rewrites through the archive's identified
                         receipt path with the authored base, reads every output back, checks it, and
                         records one status per finding: recommended, resolved-by-rewrite,
                         pending-existing-card-change (or unverified). -Merges names the Card cycle's
                         decisions.json; its recorded merge verdicts stay pending until a rewrite
                         covers them and are never counted as an applied merge.
    report               renders restate\restate-report.json and restate\restate-report.md from the
                         frozen files, with one verified and one pending example. It starts no client.

  Parameters: -Step prepare|judge|apply|report and -Staging <folder> (required); -Work, -Cards, -File,
  -OperationPrefix, -Merges, -MaxCandidates 10.

  This script forwards every argument to `zt-digest restate`, which does the work in one process with
  one `zt mcp`. It sets ZT_DIGEST_SCRIPTS to this folder so the job names the instruction file
  dsh-restate-instructions.zh.md in it. ZT_DIGEST names another zt-digest build.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# The folder of the instruction file the prepare job names.
$env:ZT_DIGEST_SCRIPTS = $PSScriptRoot
& $exe restate @args
exit $LASTEXITCODE
