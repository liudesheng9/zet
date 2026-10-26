// ==== module zt-math-lib v5 ====
// The shared code of the math stages (docs/math-cards-design.md): one script, math-steps.ps1, runs every
// deterministic chain of the math compiler in one process (#178); each chain is one DeepSeek drive job, run by a
// Haiku manager through the shared zt-workflow-lib helper `ztDrive`, and each content round (the notation table,
// the hypothesis roles, the proof reviews, the Theory answers) is DeepSeek jobs in Haiku managers through
// zt-digest-lib's `runJobs`. No Claude agent runs a single command: the module names no relay helper at all.
// Each stage has its own DeepSeek workspace under <out>\math\, and the run cleanup deletes the recorded
// conversations.
// v3 (P5): a math script step ran through the relay role (the zt-relay agent type).
// v4 (S8): the math script helper was the shared one (it had copied it without the Lock check), and the unread option remove: 'delete' left.
// v5 (#178, docs/workflow-module-structure-proposal.md): `step` runs one math-steps.ps1 chain as a DeepSeek drive
// job, `must` is the stage's stop on a failed chain, and `jobs` runs a step's content jobs in Haiku managers.
// The relay-era math helper and the compiler-step drive leave.
function zmMake(ctx, p) {
  const Z = zdMake(ctx, p)
  const A = Z.A
  const OUT = Z.OUT
  const MATH_WORK = `${OUT}\\math`
  // The math stages' one step script: every deterministic chain runs there, one process per step.
  const MATH_STEPS = p.mathScript || `${Z.SCRIPTS}\\math-steps.ps1`
  const SUPPLY = ['none', 'examples', 'examples+answers'].includes(A.supply) ? A.supply : 'examples'
  const UNITS_PER_JOB = Math.max(1, A.unitsPerJob || 4)
  const MAIN_RESULTS = Math.max(1, A.mainResults || 7)
  const deepseek = { deleted: 0, not_deleted: [] }
  const tail = (text) => String(text || '').trim().slice(0, 400)
  // One deterministic chain: one math-steps.ps1 call run by one DeepSeek drive job, answered by its one JSON
  // line. A step that printed no line at all is a transport fault (ztDrive throws); a line with `ok:false` is
  // the step's failure, which the caller reads and stops the stage with.
  const step = async (stage, name, label) => {
    const command = `pwsh -NoProfile -File "${MATH_STEPS}" -Step ${name} -Out "${OUT}" -UnitsPerJob ${UNITS_PER_JOB} -Supply ${SUPPLY} -MainResults ${MAIN_RESULTS}`
    const json = await ztDrive(ctx, { label: `${stage}: ${label}`, step: name, command, out: OUT, cwd: OUT, phase: ctx.phase, ledger: Z.LEDGER, deepseek })
    // Tree Lock (#140): a step whose answer carries the owner's typed refusal stops the stage with it.
    const lock = (json.code === Z.LOCK || json.error === Z.LOCK) ? json : ztLockRefusal(JSON.stringify(json))
    if (lock) throw ztLockStop(`${stage}: ${label}`, lock)
    return json
  }
  // The stage's stop: a failed chain halts it with the step's own reason.
  const must = (what, json) => {
    if (!json || json.ok !== true) throw new Error(`math ${what}: ${tail((json && json.error) || 'the step failed')}`)
    return json
  }
  // The content jobs a plan step listed: Haiku managers read the step's job file (at most eight jobs each),
  // record their conversations and delete them. `dir` is the compiler's own work folder of this round
  // (`notation`, `boundary`, `proof`, `theory`): the jobs write their answers under its `out`. The stages own no
  // ledger unit, so the jobs are the run's.
  const jobs = async (label, json, dir) => {
    const count = Number((json && json.jobs) || 0)
    if (!count) return []
    return Z.runJobs(label, json.file, count, {
      what: 'math compiler', dsh: `${MATH_WORK}\\${dir}`, writes: `${MATH_WORK}\\${dir}\\out`, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: ctx.phase, run: true,
    })
  }
  return { Z, A, MATH_WORK, MATH_STEPS, SUPPLY, UNITS_PER_JOB, MAIN_RESULTS, step, must, jobs, deepseek }
}

defineModule({
  id: 'zt-math-lib', version: '5', library: true, requires: ['zt-digest-lib'],
  summary: 'Library: one math-steps.ps1 step per deterministic chain of the zt-math-* stages, run as a DeepSeek drive job, and their DeepSeek content jobs in Haiku managers.',
})
// ==== end module zt-math-lib ====
