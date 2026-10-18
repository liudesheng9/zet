// ==== module zt-math-proof v1 ====
// Proof: a DeepSeek reviewer checks every proof plan, proof step, and conclusion; the script writes each fix
// that keeps its Links (proof-apply), and the last step deletes the conversations.
defineModule({
  id: 'zt-math-proof', version: '1', requires: ['zt-math-lib'],
  summary: 'ZT math digest: the proof review of every proof plan, step, and conclusion.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    const o = M.mathOpts('proof', 'Proof')
    const planned = await M.Z.drive('proof: review', { ...o, command: M.mstep('proof', ` -UnitsPerJob ${M.UNITS_PER_JOB}`) })
    const plan = zdLastJson(planned && planned.output)
    const applied = zdLastJson(((await M.Z.drive('proof: apply', { ...o, command: M.mstep('proof-apply'), cleanup: true })) || {}).output)
    log(`Proof: ${plan ? plan.units : '?'} results reviewed; ${applied ? `${applied.ok} steps ok, ${applied.fixed} fixed, ${applied.refused} refused` : 'apply failed'}.`)
    return { status: plan && applied ? 'done' : 'failed', units: plan && plan.units, applied }
  },
})
// ==== end module zt-math-proof ====
