// ==== module zt-math-boundary v3 ====
// Boundary (#178): one math-steps.ps1 drive job plans the DeepSeek jobs that write hypothesis roles with
// counterexamples and contrast pairs as JSON; the content managers run them; the apply step makes the parts and
// lists one review job per supplied batch; the reviewers run; the settle step deletes what a review judged wrong.
// 3 (#180): each step answers its own counts (`units`, `created`, `kept`, `deleted`), never a nested tool line.
defineModule({
  id: 'zt-math-boundary', version: '3', requires: ['zt-math-lib'],
  summary: 'ZT math digest: hypothesis roles, counterexamples, contrast pairs (write → apply and review → settle).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    const n = (v) => Number(v || 0)
    // One job per -UnitsPerJob units: the hypothesis roles and the definitions' contrast pairs.
    const plan = M.must('boundary', await M.step('math-boundary', 'boundary', 'write'))
    await M.jobs('math-boundary: write', plan, 'boundary')
    // The parts from the answers, and one independent review job per batch of supplied items.
    const applied = M.must('boundary apply', await M.step('math-boundary', 'boundary-apply', 'apply'))
    await M.jobs('math-boundary: review', applied, 'boundary')
    // The supplied parts a review judged wrong are deleted, each with the part that names it.
    const settled = M.must('boundary settle', await M.step('math-boundary', 'boundary-settle', 'settle'))
    log(`Boundary: ${n(plan.units)} units; ${n(applied.created)} parts made, ${n(applied.skipped_supplied)} supplied items skipped; ${n(settled.deleted)} wrong parts deleted, ${n(settled.kept)} supplied parts kept.`)
    return {
      status: 'done',
      plan: { units: n(plan.units), jobs: n(plan.jobs) },
      applied: { created: n(applied.created), skipped_supplied: n(applied.skipped_supplied), jobs: n(applied.jobs) },
      settled: { kept: n(settled.kept), deleted: n(settled.deleted), unreviewed: n(settled.unreviewed) },
      deepseek: M.deepseek,
    }
  },
})
// ==== end module zt-math-boundary ====
