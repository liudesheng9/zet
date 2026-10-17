#Requires -Version 7.5
<#
.SYNOPSIS
  The deterministic half of the book digest's hub stage (docs/hub-cards-design.md). A Haiku hub
  manager chooses the limits from the survey; DeepSeek plans the fields and writes every hub as a
  JSON file; this script does everything between them without a model: it surveys the agent
  Workspace, writes the job files, builds and updates the Hub Cards, checks them with get_hub, and
  reports. No step writes hub prose of its own except fixed lines (范围, the register's lists).
  This script forwards to `zt-digest hub` (tools/zt-digest/src/hub.rs); callers should call the
  tool directly.

.DESCRIPTION
  -Work is the stage's folder and the DeepSeek Harness cwd. It holds survey.json, survey.md,
  limits.json, state.json, the job files, and out\ (the JSON files DeepSeek writes). Every step
  prints compact JSON as its last line. A step that hands work to DeepSeek prints
  {"jobs":[{"id","files":[...],"prompt","continue"?}], ...}.

    survey                 reads every agent Topic tree and the hub graph; writes survey.json and
                           survey.md (one line per Topic); prints the size statistics the hub
                           manager reads to choose the limits.
    plan -Limits <json>    clamps and saves the limits; picks the trees whose hubs are missing,
                           faulty, legacy (an Idea Card used as a hub), or whose range changed
                           (all with -Rebuild, only -Trees when given); writes one topic job per
                           -PerJob trees and one field job; prints the jobs.
    apply                  reads out\hub-<tree>.json and out\fields.json, drops members that do
                           not exist, creates or updates the line, topic, and field hubs and the
                           register (Topic 索引), and links each topic hub from its Topic body.
    check                  runs get_hub on every hub of the state with the limits; prints one
                           follow-up job per writer conversation whose hubs have faults.
    report                 list_hubs with the limits: counts, faults, missing hubs; writes
                           report.json.
    precheck -File <file>  read-only: judges one DeepSeek file (out\hub-<tree>.json or
                           out\fields.json) before its writer replies, the way apply builds it and
                           get_hub judges it, as far as that is known before the Cards exist; runs
                           zt-check (R2, R3, R4, R5, R7, R9) on the reasons and group
                           names. It writes no Card and no state. Prints one line per fault,
                           `<file or hub> <code> <text>`, then `faults: N` as its last line (not
                           JSON). Each job file names this command for each of its files
                           (`SELF-CHECK: zt-digest hub -Step precheck ...`).

  Parameters: -Step survey|plan|apply|check|report|precheck and -Work <folder> (required); -File,
  -Limits (JSON), -Trees (a,b,...), -PerJob 0, -Rebuild.

  This script forwards every argument to `zt-digest hub`, which does the work in one process with
  one `zt mcp` and zt-check in-process. It sets ZT_DIGEST_SCRIPTS to this folder: the jobs name the
  instruction file dsh-hub-instructions.zh.md in it. ZT_DIGEST names another zt-digest build.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# The folder of the instruction file the jobs name.
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
& $exe hub @pass
exit $LASTEXITCODE
