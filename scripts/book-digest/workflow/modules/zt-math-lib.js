// ==== module zt-math-lib v1 ====
// The shared code of the math stages (doc/math-cards-design.md): math-compiler.ps1 does every structural step;
// DeepSeek writes JSON files and Card bodies; Haiku drivers relay the jobs under the DeepSeek Subagent Managed
// Protocol (drive, from zt-digest-lib). Each stage has its own DeepSeek workspace under <out>\math\, and its last
// step's cleanup deletes the conversations.
function zmMake(ctx, p) {
  const Z = zdMake(ctx, p)
  const A = Z.A
  const MATH_WORK = `${Z.OUT}\\math`
  const MATH_COMPILER = `${Z.SCRIPTS}\\math-compiler.ps1`
  const SUPPLY = ['none', 'examples', 'examples+answers'].includes(A.supply) ? A.supply : 'examples'
  const UNITS_PER_JOB = Math.max(1, A.unitsPerJob || 4)
  const MAIN_RESULTS = Math.max(1, A.mainResults || 7)
  const mstep = (s, more = '') => `pwsh -NoProfile -File "${MATH_COMPILER}" -Step ${s} -Staging "${Z.OUT}"${more}`
  // One script step relayed by Haiku; its last JSON line, or null (logged) on a failure.
  const runMathScript = async (what, s, extra, phaseName) => {
    const run = await Z.zAgent(
      `Run this PowerShell command once in the foreground with timeout 600000 and wait for it; do nothing else:
${mstep(s, extra)}
Return ok (true when it exits without an error) and output = its last output line exactly (a compact JSON object), or the error text when it fails. Do not retry.`,
      { label: `math: ${what}`, phase: phaseName, model: 'haiku', schema: ZD_RUN_SCHEMA },
    )
    const out = run && run.ok ? zdLastJson(run.output) : null
    if (!out) log(`math ${what} failed: ${run && run.output}`)
    return out
  }
  const mathOpts = (stage, phaseName) => ({ dsh: `${MATH_WORK}\\${stage}`, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: phaseName, writes: `${MATH_WORK}\\${stage}\\out`, remove: 'delete', keep: true })
  return { Z, A, MATH_WORK, SUPPLY, UNITS_PER_JOB, MAIN_RESULTS, mstep, runMathScript, mathOpts }
}

defineModule({
  id: 'zt-math-lib', version: '1', library: true, requires: ['zt-digest-lib'],
  summary: 'Library: math-compiler.ps1 steps and DeepSeek options for the zt-math-* stage modules.',
})
// ==== end module zt-math-lib ====
