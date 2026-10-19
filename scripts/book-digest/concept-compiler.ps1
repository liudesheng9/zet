#Requires -Version 7.5
<#
.SYNOPSIS
  The concept, restatement and filing review of one prepared digest run (issue #97,
  docs/luhmann-comparison-implementation-design.md §4.2 and §4.4). Read-only: every step writes
  staging files and no Card. This script forwards to `zt-digest concepts`
  (tools/zt-digest/src/concepts.rs); callers should call the tool directly.

.DESCRIPTION
  -Staging is the prepared run's staging folder (manifest.json with the book's citation_key). The
  caller freezes the proposed concepts in <Staging>\concepts\proposals.json; the work folder
  (default <Staging>\concepts) holds everything else. Every step prints compact JSON as its last
  line; the prepare step also prints the judge job it wants written.

    prepare             reads the book's term table through zt-check's own parser (术语, 不用,
                        意思, 首次定义), the agent Topic questions, the Idea Card statements, and
                        the representative Cards of the shortlist. It selects at most ten Topic
                        candidates per proposed concept and at most ten restatement candidates per
                        proposed statement, deduplicated by unordered pair, and freezes
                        concepts\evidence.json, concepts\endpoints\ and the judge job
                        concepts\cj-01.md. It makes read calls only, through one `zt mcp`.
    judge -File <f>      validates one judge answer file against the frozen evidence and writes
                        concepts\judged.json: schema, duplicate keys, unknown verdicts,
                        contradictory outcomes for one pair, missing evidence, unread endpoints.
                        A refused or absent answer leaves those pairs unjudged; it never becomes a
                        create or merge decision.
    report              renders concept-review.json and concept-review.md under -Staging from the
                        frozen files alone. It starts no client, so report generation cannot write
                        a Card. The same evidence and answers give the same report id and change
                        no file.
    decision -File <f>   validates a reviewed gate decision against the committed report: the
                        exact report id, known finding ids, the finding's own verdict, one of the
                        remedies the report supports, and the reviewed authored fingerprints. It
                        writes concepts\decision.json and no Card.

  Parameters: -Step prepare|judge|report|decision and -Staging <folder> (required); -Work, -File,
  -MaxTopics 10, -MaxRestatements 10.

  This script forwards every argument to `zt-digest concepts`, which does the work in one process
  with one `zt mcp`. It sets ZT_DIGEST_SCRIPTS to this folder so the job names the instruction
  file dsh-concept-instructions.zh.md in it. ZT_DIGEST names another zt-digest build.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# The folder of the instruction file the prepare job names.
$env:ZT_DIGEST_SCRIPTS = $PSScriptRoot
& $exe concepts @args
exit $LASTEXITCODE
