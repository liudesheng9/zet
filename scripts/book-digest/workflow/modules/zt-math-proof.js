// ==== module zt-math-proof v4 ====
// Proof (#178): one math-steps.ps1 drive job plans a DeepSeek reviewer per -UnitsPerJob formal results; the
// reviewers check every proof plan, proof step (证明：…) and conclusion; the apply step writes each fix that
// keeps its Links and refuses the others.
// 3 (#180): the apply answers its counts (`steps_ok`, `fixed`, `refused`) instead of a nested tool line.
defineModule({
  id: 'zt-math-proof', version: '4', requires: ['zt-math-lib'],
  summary: 'ZT math digest: the proof review of every proof plan, step, and conclusion.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    const n = (v) => Number(v || 0)
    // v4 (thompson1914calculus pilot, 2026-11-22): a resume starts from the ledger. Every obligation done: the stage
    // is reused. Open obligations whose evidence the compiler already wrote: the ledger settle alone. Planning again
    // would rebind a registered unit to new work, which the ledger refuses.
    const state = M.must('proof state', await M.step('math-proof', 'layer-state', 'state', '-Layer proof'))
    if (n(state.total) > 0 && n(state.open) === 0) {
      log(`Proof: all ${n(state.total)} obligations done; the stage is reused.`)
      return { status: 'done', reused: true, total: n(state.total), deepseek: M.deepseek }
    }
    if (n(state.total) > 0 && state.evidence === true) {
      const s = M.must('proof settle', await M.step('math-proof', 'settle', 'settle', '-Layer proof'))
      log(`Proof: resumed at the settle; ${n(s.settled)} obligations settled, ${n(s.settle_refused)} refused.`)
      return { status: 'done', resumed: 'settle', settled: n(s.settled), settle_refused: n(s.settle_refused), deepseek: M.deepseek }
    }
    // One job per -UnitsPerJob formal results: the proof plans, steps and conclusions.
    const plan = M.must('proof', await M.step('math-proof', 'proof', 'review'))
    await M.jobs('math-proof: review', plan, 'proof')
    // The reviewed fixes: a body that keeps every Link is written, the others are refused.
    const step = M.must('proof apply', await M.step('math-proof', 'proof-apply', 'apply'))
    const applied = { ok: n(step.steps_ok), fixed: n(step.fixed), refused: n(step.refused), parts: n(step.parts) }
    log(`Proof: ${n(plan.units)} results reviewed; ${applied.ok} steps ok, ${applied.fixed} fixed, ${applied.refused} refused.`)
    return { status: 'done', units: n(plan.units), applied, deepseek: M.deepseek }
  },
})
// ==== end module zt-math-proof ====
