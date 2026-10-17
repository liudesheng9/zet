#Requires -Version 7.5
<#
.SYNOPSIS
  The empirical-results layer of the economics digest (docs/zt-digest-framework.md, docs/finding-cards-design.md):
  the Finding Cards of a book, one per empirical result, written after the digest. A compiler cycle: the script
  plans, DeepSeek writes a JSON file per job and checks it with the precheck step, the script writes the Cards.
  Needs zt 0.20.0 (the Card type finding). This script forwards to `zt-digest finding`
  (tools/zt-digest/src/finding.rs); callers should call the tool directly.

.DESCRIPTION
  -Staging is the digest's staging folder (manifest.json with each group's section files, the owned-gNN.txt
  logs of the digest runs, sections\). -Work (default <Staging>\findings) holds the job files, out\<job>.json
  (DeepSeek's answers), state.json, and created.txt. Every step prints compact JSON last.

    plan      finds the result candidates of each group: the lines of its sections that name a number and a
              cue of empirical work (数据, 样本, 调查, 访谈, 回归, 实验, 统计, 显著, 系数, %, …), at most
              -MaxCandidates per group; lists each group's own Idea Cards and anchor; writes one job file per
              -GroupsPerJob groups with its SELF-CHECK line. Groups with no candidate and jobs already applied
              are left out. Prints {"candidates","groups","jobs":[{"id","files","prompt"}]}.
    precheck  read-only: checks out\<Job>.json as apply would (fields, supports, the role Link) and runs zt-check
              on the Finding Cards as drafts (F1, F2, F3, the Card language). One line per fault, then faults: N.
    apply     for each answered job not yet applied: refuses a finding with a fault (the precheck's faults,
              zt-check included), creates the Finding Card (type finding) as a Side successor of its group's
              anchor, and links it from every supported Idea Card ("印证 [[finding]]：title。"). Without -Apply it
              only validates.
              It writes in two helper runs: the Cards first, then the Link edits, which read the Idea Cards again
              and carry base, so an Idea Card a parallel stage changed in between is refused, not overwritten. A job
              whose helper run fails part way is not applied: state.json keeps it under partial (the Cards made so
              far and the error), and the next apply reuses those Cards and writes what is missing.
    report    the counts of state.json.

  Parameters: -Step plan|precheck|apply|report and -Staging <folder> (required); -Work, -Job (precheck),
  -GroupsPerJob 3, -MaxCandidates 16, -Apply.

  This script forwards every argument to `zt-digest finding`, which does the work in one process with one
  `zt mcp` and zt-check in-process. Each job file's SELF-CHECK line names
  `zt-digest finding -Step precheck -Staging "<staging>" -Work "<work>" -Job <id>`. The script sets
  ZT_DIGEST_SCRIPTS to this folder: the tool reads dsh-finding-instructions.zh.md from it. ZT_DIGEST names
  another zt-digest build.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# The folder of the instruction file.
$env:ZT_DIGEST_SCRIPTS = $PSScriptRoot
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
& $exe finding @pass
exit $LASTEXITCODE
