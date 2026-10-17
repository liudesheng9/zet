#Requires -Version 7.5
<#
.SYNOPSIS
  The deterministic half of the zt-math-digest workflow (docs/math-cards-design.md). DeepSeek writes
  every Card body and every JSON answer; this script finds the book's numbered statements, builds
  the skeleton of every mathematical unit and Theory Card, writes the job files, applies the
  answers, and checks the result. It never writes Card prose except skeletons, lists, and Links.

  This script forwards to `zt-digest math` (tools/zt-digest/src/math.rs, installed beside zt.exe by
  scripts/install-zt-digest.ps1), which does the work in one process with one `zt mcp`. Call
  `zt-digest math` directly when you can: it takes the same parameters, writes the same files, and
  prints the same lines.

.DESCRIPTION
  -Staging is the digest's staging folder (manifest.json, sections\, groups\). The work folder is
  <Staging>\math. Every step prints compact JSON as its last line. A step that hands work to
  DeepSeek prints {"jobs":[{"id","files","prompt"}], ...}.

    index            finds every numbered environment in the book (定义, 公理, 定理, 命题, 引理, 推论,
                     例, 习题, and their English names), its lines, its proof, its name, and the
                     statements it cites; writes math\index.json and math\index.md. Reads files only.
    docs             writes the math prompt files: math-prefix.md (the shared digest prefix with
                     the math supplement), math-link-prefix.md, and math\brief.zh.md, fill.zh.md,
                     review.zh.md (each shared instruction file with its math supplement). The
                     shared files come first, so every rule of the book digest holds unchanged.
    skeleton         (-Apply) creates one unit Model Card per indexed definition, axiom, and result
                     (its Math form, its level, its source lines) as a Side successor of its group's
                     anchor, then one Theory Card per chapter that lists its units. Each unit joins
                     its group's owned-gNN.txt and src-gNN.txt, so the digest, the model stage, and
                     the link pass treat it like a Card the group wrote. Appends the units, examples,
                     and exercises of each group to its group file.
    notation         writes the notation job (symbol candidates); notation-apply (-Apply) writes the
                     Book notation table (符号表) as a Side successor of <key>/1.
    boundary         one job per -UnitsPerJob units: hypothesis roles with their counterexamples
                     for results, and the contrast pair for definitions that lack one.
    boundary-apply   creates the parts from math\boundary\out\b-NN.json (only book items under
                     -Supply none) and prints one review job per batch for the supplied items.
    boundary-settle  deletes each supplied part that a review (v-NN.json) judged wrong, with the
                     part that names it.
    proof            one job per -UnitsPerJob formal results: a DeepSeek reviewer checks the proof
                     plan, every proof step (证明：…), and the conclusion; the Derivation parts are
                     checked by the model compiler's review.
    proof-apply      writes each fixed body from math\proof\out\p-NN.json that keeps every Link of
                     the old body; refuses the others.
    theory           one job per four Theory Cards: the question, the main results (at most
                     -MainResults), and the objects.
    theory-apply     rewrites each Theory Card: 问题, 主要结果, 对象 from the answers; 结构 in
                     dependency order, 所用工具, 后续, and the dependency map from get_model.
    check            the math rules M1 to M18 on every unit and Theory Card of the book; writes
                     math-audit.json and math-audit.md in -Out (default <Staging>). Read-only.

  Parameters: -Step* (one of the steps above), -Staging*, -Out (<Staging>, for check),
  -UnitsPerJob (4), -Supply (examples; none, examples+answers), -MainResults (7), -CoreBudget (3,
  not used by any step), -MaxSymbols (80), -Apply.

  This script forwards every argument to `zt-digest math`. It sets ZT_DIGEST_SCRIPTS to this
  folder: the tool reads the dsh-math-*.zh.md and shared instruction files from it and names them
  in the job files by this path. ZT_DIGEST names another zt-digest build.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# The folder of the instruction files (the job files name them by this path).
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
& $exe math @pass
exit $LASTEXITCODE
