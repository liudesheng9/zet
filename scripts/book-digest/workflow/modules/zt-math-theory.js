// ==== module zt-math-theory v2 ====
// Theory (#178): after the link pass, so the cited premises are complete. One math-steps.ps1 drive job plans one
// DeepSeek job per four Theory Cards for their title, question, main results and objects; the apply step writes
// the dependency order, tools, users and the map.
defineModule({
  id: 'zt-math-theory', version: '2', requires: ['zt-math-lib'],
  summary: 'ZT math digest: the Theory Cards (question, main results, objects, dependency order, tools, map).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    // One job per four Theory Cards: the question, the main results and the objects.
    const plan = M.must('theory', await M.step('math-theory', 'theory', 'write'))
    await M.jobs('math-theory: write', plan, 'theory')
    // The applied Theory Cards, with their dependency order, tools, users and map.
    const applied = M.must('theory apply', await M.step('math-theory', 'theory-apply', 'apply'))
    log(`Theory: ${applied.theories} Theory Cards written (${applied.answered} answered by DeepSeek).`)
    return { status: 'done', theories: plan.theories, applied, deepseek: M.deepseek }
  },
})
// ==== end module zt-math-theory ====
