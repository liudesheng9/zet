// ==== module zt-math-boundary v1 ====
// Boundary: DeepSeek writes hypothesis roles with counterexamples and contrast pairs as JSON; the script makes
// the parts; a second DeepSeek job checks each supplied one; the script deletes what it judged wrong (settle).
defineModule({
  id: 'zt-math-boundary', version: '1', requires: ['zt-math-lib'],
  summary: 'ZT math digest: hypothesis roles, counterexamples, contrast pairs (write → apply and review → settle).',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    const o = M.mathOpts('boundary', 'Boundary')
    const planned = await M.Z.drive('boundary: write', { ...o, command: M.mstep('boundary', ` -UnitsPerJob ${M.UNITS_PER_JOB} -Supply ${M.SUPPLY}`) })
    const plan = zdLastJson(planned && planned.output)
    if (!plan) { await M.Z.drive('boundary: clean up', { ...o, command: M.mstep('boundary-settle'), cleanup: true }); return { status: 'failed', error: (planned && planned.error) || 'no output' } }
    const applied = zdLastJson(((await M.Z.drive('boundary: apply and review', { ...o, command: M.mstep('boundary-apply', ` -Supply ${M.SUPPLY}`) })) || {}).output)
    const settled = zdLastJson(((await M.Z.drive('boundary: settle', { ...o, command: M.mstep('boundary-settle'), cleanup: true })) || {}).output)
    log(`Boundary: ${plan.units} units; ${applied ? `${applied.created} parts made, ${applied.skipped_supplied} supplied items skipped` : 'apply failed'}; ${settled ? `${settled.deleted} wrong parts deleted, ${settled.kept} supplied parts kept` : 'settle failed'}.`)
    return { status: applied && settled ? 'done' : 'failed', plan: { units: plan.units, jobs: (plan.jobs || []).length }, applied, settled }
  },
})
// ==== end module zt-math-boundary ====
