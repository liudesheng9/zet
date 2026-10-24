// ==== module zt-math-lib v4 ====
// The shared code of the math stages (docs/math-cards-design.md): math-compiler.ps1 does every structural step;
// DeepSeek writes JSON files and Card bodies; Haiku drivers relay the jobs under the DeepSeek Subagent Managed
// Protocol (drive, from zt-digest-lib). Each stage has its own DeepSeek workspace under <out>\math\, and its last
// step's cleanup deletes the conversations.
// v3 (P5): a math script step is relayed by the relay role (the zt-relay agent type), as runScript is.
// v4 (S8): runMathScript is runScript (it had copied it without the Lock check), and the unread option remove: 'delete' leaves.
function zmMake(ctx, p) {
  const Z = zdMake(ctx, p)
  const A = Z.A
  const MATH_WORK = `${Z.OUT}\\math`
  const MATH_COMPILER = Z.tool('math-compiler')
  const SUPPLY = ['none', 'examples', 'examples+answers'].includes(A.supply) ? A.supply : 'examples'
  const UNITS_PER_JOB = Math.max(1, A.unitsPerJob || 4)
  const MAIN_RESULTS = Math.max(1, A.mainResults || 7)
  const mstep = (s, more = '') => `${MATH_COMPILER} -Step ${s} -Staging "${Z.OUT}"${more}`
  // One script step through the relay (runScript, with its Lock check); its last JSON line, or null (logged) on a failure.
  const runMathScript = async (what, s, extra, phaseName) => {
    const { run, json } = await Z.runScript(`math: ${what}`, phaseName, mstep(s, extra))
    if (!json) log(`math ${what} failed: ${run && run.output}`)
    return json
  }
  const mathOpts = (stage, phaseName) => ({ dsh: `${MATH_WORK}\\${stage}`, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: phaseName, writes: `${MATH_WORK}\\${stage}\\out`, keep: true })
  return { Z, A, MATH_WORK, SUPPLY, UNITS_PER_JOB, MAIN_RESULTS, mstep, runMathScript, mathOpts }
}

defineModule({
  id: 'zt-math-lib', version: '4', library: true, requires: ['zt-digest-lib'],
  summary: 'Library: math-compiler.ps1 steps and DeepSeek options for the zt-math-* stage modules.',
})
// ==== end module zt-math-lib ====
