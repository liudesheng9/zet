// ==== module zt-math-theory v1 ====
// Theory: after the link pass, so the cited premises are complete. DeepSeek writes each Theory Card's title,
// question, main results, and objects; the script writes the dependency order, tools, users, and the map.
defineModule({
  id: 'zt-math-theory', version: '1', requires: ['zt-math-lib'],
  summary: 'ZT math digest: the Theory Cards (question, main results, objects, dependency order, tools, map).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    const o = M.mathOpts('theory', 'Theory')
    const planned = await M.Z.drive('theory: write', { ...o, command: M.mstep('theory', ` -MainResults ${M.MAIN_RESULTS}`) })
    const plan = zdLastJson(planned && planned.output)
    const applied = zdLastJson(((await M.Z.drive('theory: apply', { ...o, command: M.mstep('theory-apply', ` -MainResults ${M.MAIN_RESULTS}`), cleanup: true })) || {}).output)
    log(`Theory: ${applied ? `${applied.theories} Theory Cards written (${applied.answered} answered by DeepSeek)` : 'apply failed'}.`)
    return { status: plan && applied ? 'done' : 'failed', theories: plan && plan.theories, applied }
  },
})
// ==== end module zt-math-theory ====
