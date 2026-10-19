#Requires -Version 7.5
<#
.SYNOPSIS
  The concept register of one Workspace: a small Hub Card of level register whose 词条 rows are
  terms with one to three substantive Cards (#93 S10, issue #101,
  docs/luhmann-comparison-implementation-design.md §4.1 and §8.3-8.5). This script forwards to
  `zt-digest register` (tools/zt-digest/src/register.rs); callers should call the tool directly.

.DESCRIPTION
  -Work is the stage's folder. It holds out\register-plan.json, the accepted plan. Every step
  prints compact JSON as its last line.

    plan [-Review <concept-review.json>]
                           reads every agent book term table (through zt-check's own row parser)
                           and the model-head names, reads the reviewed concept report as
                           read-only evidence when one is named, and stages
                           out\register-plan.json: one term per accepted spelling with one to
                           three substantive targets (a definition or statement, then a whole
                           model, then a standalone Topic claim), the terms of one book row
                           grouped by their accepted spellings, the unresolved source rows, the
                           exact register body, and the id of the reviewed report it read. A
                           reviewed entry in the human Workspace is a read-only target; a
                           reviewed entry no Card holds is an excluded stale review. A register
                           that already declares 词条分册 parts is refused: publishing parts is
                           S11 (#104).
    apply                  publishes the staged body: creates the Topic 索引 and the register
                           Hub Card when the Workspace has none, otherwise edits that one
                           register, keeping every group it does not own (the field-hub
                           navigation among them). It then reads every page of the register
                           back through get_register at one revision and verifies every term and
                           target. Running it again over the same plan writes nothing.
    check                  reads every page and compares the staged plan with the published
                           entries: the terms that are missing, diverged, or extra.

  Parameters: -Step plan|apply|check and -Work <folder> (required); -Review <file>.

  This script forwards every argument to `zt-digest register`, which does the work in one process
  with one `zt mcp`. ZT_DIGEST names another zt-digest build.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
& $exe register @args
exit $LASTEXITCODE
