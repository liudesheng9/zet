#Requires -Version 7.5
<#
.SYNOPSIS
  The concept register of one Workspace: a small Hub Card of level register whose 词条 rows are
  terms with one to three substantive Cards (#93 S10, #101, S11, #104,
  docs/luhmann-comparison-implementation-design.md §4.1 and §8.3-8.5). This script forwards to
  `zt-digest register` (tools/zt-digest/src/register.rs); callers should call the tool directly.

.DESCRIPTION
  -Work is the stage's folder. It holds out\register-plan.json, the accepted plan, and by default
  out\register-readings.json, the accepted pinyin readings. Every step prints compact JSON as its
  last line.

    plan [-Review <concept-review.json>] [-Readings <register-readings.json>]
                           reads every agent book term table (through zt-check's own row parser)
                           and the model-head names, reads the reviewed concept report and the
                           accepted pinyin readings as read-only evidence when they are named, and
                           stages out\register-plan.json: one term per accepted spelling with one to
                           three substantive targets (a definition or statement, then a whole model,
                           then a standalone Topic claim), the accepted readings and the ones it
                           could not classify, the unresolved source rows, the exact part bodies and
                           root body, the identified calls that publish them, and the id of the
                           reviewed report it read. A reviewed entry in the human Workspace is a
                           read-only target; a reviewed entry no Card holds is an excluded stale
                           review.
    apply                  publishes the staged parts first (each through its own identified call,
                           verified by their exact bodies), then the root last, with the addresses
                           the receipts returned. Retrying the same accepted plan replays the Cards
                           it already created instead of duplicating them. It then reads every page
                           of the register back through get_register at one revision and marks the
                           publication complete only after the exact root, parts, targets, and
                           revision agree with the plan. Running it again over a finished plan
                           writes nothing.
    check                  reads every page and compares the staged plan with the published
                           entries, parts, labels, and readings: the terms that are missing,
                           diverged, or extra, and the parts whose label or address moved.

  A register over 120 term rows puts its rows into ordinary Idea Cards declared under 词条分册,
  one per accepted pinyin-initial band (A–F, G–L, M–R, S–Z, then 0–9 and 其他), each holding at most
  120 rows and split contiguously when a band is longer. The readings file is
  {"schema_version":1,"readings":{"学习":{"reading":"xué xí","initial":"x"}}}: a term with no
  reading, a null initial, or "uncertain": true stays in 其他 and in the unresolved-reading report.
  This step checks the reading's shape only; it never transliterates. Part addresses are reused by
  label, then by position, so a rebuild keeps them; a part the plan no longer needs stays declared
  and empty, pending a separate deletion plan. A register whose declared parts are moved, missing,
  a Hub, listed twice, nested, or unlisted is refused before any write.

  Parameters: -Step plan|apply|check and -Work <folder> (required); -Review <file>;
  -Readings <file>.

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
