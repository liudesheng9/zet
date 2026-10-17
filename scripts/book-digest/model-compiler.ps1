#Requires -Version 7.5
<#
.SYNOPSIS
  The deterministic half of the book digest's model compiler. Sonnet plans each model (plan.json);
  DeepSeek writes briefs, Card bodies, charts, Derivation parts, and reviews; this script does
  everything between them without a model: it builds skeletons, writes job files, applies splits,
  and reports. No step reads or writes Card prose except to place skeletons and Links.

  This script forwards to `zt-digest model` (tools/zt-digest, installed beside zt.exe by
  scripts/install-zt-digest.ps1), which does the work in one process. Call `zt-digest model`
  directly when you can: it takes the same parameters, writes the same files, and prints the same
  lines.

.DESCRIPTION
  One batch of models shares one work folder -Work (also the DeepSeek Harness cwd), with
  state.json and one sub-folder per model (<slug>: the address with / and | replaced by _).
  Every step prints compact JSON as its last line. A step that hands work to DeepSeek prints
  {"jobs":[{"id","files":[...],"prompt","continue"?}], ...}; "continue" names an earlier job
  whose conversation the job continues (its "files" then open the follow-up message).

    brief     -Models a,b,...   context pack per model (model-context) and one brief job each;
                                skips a model that is already built (unless -Rebuild or -Level
                                changes it).
    validate  -Plan <file>      checks one plan.json against the model; prints one fault per line
                                and `faults: N`. Changes nothing.
    apply                       for each model with a plan.json: validates it, builds the skeleton
                                (level, parts in build order, skeleton bodies with kinded Links,
                                the setting's symbol and term tables, the Model Card's 结构 list),
                                writes setting.md, and prints the fill jobs (round 1: Model Card,
                                setting, every part that is not a Derivation part, with charts).
    derive                      prints the Derivation jobs (round 2), with the filled premises
                                copied in.
    review [-Changed]           prints one review job per Derivation chain of the model's own parts
                                (with -Changed, only chains that the last split touched). A shared
                                part (`take`) is never reviewed, split, or fixed: it belongs to
                                its home model, which may be finished.
    split [-Need a=N;b=M]       applies review-*.json verdicts (split, fix) and composer requests
                                (`need` Cards for one part): inserts the new parts, re-points the
                                next member, updates the 结构 list, and prints follow-up jobs.
    report                      one report per model: parts, relations, charts, Derivation chains,
                                build faults, style faults, parts still `待写`, and how many
                                Derivation parts have both 思路 and 解读 (explained).

  Explain (docs/derivation-explanations-proposal.md): adds 思路, 说明 lines, and 解读 to finished
  Derivation parts. DeepSeek writes JSON; this script writes the Cards and never changes 起点, a
  step, or 结果. It never runs the brief, plan, fill, derive, or review of a finished model.
    explain -Models a,b,...     one job per chain (at most three parts) of each model's own
                                Derivation parts that lack 思路 or 解读; -MaxParts caps the parts.
    explain-check -Job <id>     the writer's own check (read-only): prints what explain-apply would
                                refuse in the job's result file, one line per part, `faults: N`
                                last. Faults the old body already has are listed as kept, not
                                counted. Each explain job file names it on its SELF-CHECK line.
    explain-apply               reads the jobs' JSON, inserts the sections, refuses a part whose
                                body changed since its job, whose core changed, or whose new body
                                has a fault that the old body does not have (a fault the old body
                                already has is kept and listed under `kept`); sends refused parts
                                back to the same conversation once, and prints those follow-up jobs.
    explain-report              per model: Derivation parts, explained, missing, refused.

  Parameters: -Step*, -Work*, -Staging, -Models, -Plan, -Need, -Changed, -Rebuild, -Level,
  -MaxSentences (12), -MaxChars (400), -MaxSteps (6), -MaxExplainSentences (16),
  -MaxExplainChars (600), -MaxParts (0), -BriefDoc, -FillDoc, -ReviewDoc, -Job, -File.

  A Derivation part has five sections: 思路, 起点, 步骤, 结果, 解读. In a chain of two or more parts,
  the first line of 思路 is the part's position (`第 2 张，共 3 张：从 … 推到 …。`); the tool
  writes it in apply and explain and rewrites it after every split.

  Structural changes (insert, move, delete) run one call at a time through the zt helper with
  -Moves, and every address in state.json follows each moved map.

  Mathematical units (zt-math-digest): -BriefDoc, -FillDoc, and -ReviewDoc replace the shared
  instruction files (the math workflow passes the shared file with its math supplement appended).
  A unit's first main-line part may be its premises (facet 前提), and a premise that is the Model
  Card of another unit is a cited premise, not a closure fault.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# `pwsh -File` and `&` hand `-Name:value` over as two arguments (a switch value as a Boolean) and a
# list as an array; the tool takes `-Name:value` and a comma-separated list.
$pass = [Collections.Generic.List[string]]::new()
foreach ($a in $args) {
    $last = if ($pass.Count) { $pass[$pass.Count - 1] } else { '' }
    if ($last -match '^-[A-Za-z]\w*:$') { $pass[$pass.Count - 1] = $last + $(if ($a -is [array]) { $a -join ',' } else { "$a" }) }
    elseif ($a -is [bool] -and $last -match '^-[A-Za-z]\w*$') { $pass[$pass.Count - 1] = "${last}:$a" }
    elseif ($a -is [array]) { $pass.Add($a -join ',') }
    else { $pass.Add("$a") }
}
& $exe model @pass
exit $LASTEXITCODE
