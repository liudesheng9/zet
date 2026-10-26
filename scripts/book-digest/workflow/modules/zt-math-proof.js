// ==== module zt-math-proof v2 ====
// Proof (#178): one math-steps.ps1 drive job plans a DeepSeek reviewer per -UnitsPerJob formal results; the
// reviewers check every proof plan, proof step (证明：…) and conclusion; the apply step writes each fix that
// keeps its Links and refuses the others.
defineModule({
  id: 'zt-math-proof', version: '2', requires: ['zt-math-lib'],
  summary: 'ZT math digest: the proof review of every proof plan, step, and conclusion.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    // One job per -UnitsPerJob formal results: the proof plans, steps and conclusions.
    const plan = M.must('proof', await M.step('math-proof', 'proof', 'review'))
    await M.jobs('math-proof: review', plan, 'proof')
    // The reviewed fixes: a body that keeps every Link is written, the others are refused.
    const step = M.must('proof apply', await M.step('math-proof', 'proof-apply', 'apply'))
    const applied = step.applied || {}
    log(`Proof: ${plan.units} results reviewed; ${applied.ok} steps ok, ${applied.fixed} fixed, ${applied.refused} refused.`)
    return { status: 'done', units: plan.units, applied, deepseek: M.deepseek }
  },
})
// ==== end module zt-math-proof ====
