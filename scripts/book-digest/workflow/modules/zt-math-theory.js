// ==== module zt-math-theory v4 ====
// Theory (#178): after the link pass, so the cited premises are complete. One math-steps.ps1 drive job plans one
// DeepSeek job per four Theory Cards for their title, question, main results and objects; the apply step writes
// the dependency order, tools, users and the map.
// 3 (#180): the steps answer their counts (`theories`, `answered`), never a nested tool line.
// 4: the stage publishes the theory revision from the applied record (docs/math-digest-redesign-proposal.md).
defineModule({
  id: 'zt-math-theory', version: '4', requires: ['zt-math-lib'],
  summary: 'ZT math digest: the Theory Cards (question, main results, objects, dependency order, tools, map).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    const n = (v) => Number(v || 0)
    // One job per four Theory Cards: the question, the main results and the objects.
    const plan = M.must('theory', await M.step('math-theory', 'theory', 'write'))
    await M.jobs('math-theory: write', plan, 'theory')
    // The applied Theory Cards, with their dependency order, tools, users and map.
    const applied = M.must('theory apply', await M.step('math-theory', 'theory-apply', 'apply'))
    log(`Theory: ${n(applied.theories)} Theory Cards written (${n(applied.answered)} answered by DeepSeek).`)
    // 4: the stage publishes its own revision from the applied record (`finalize -Step stage -Layer theory`), so
    // `theory/book@1` settles and Organize, which follows it, can publish (thompson1914calculus, 2026-11-22).
    const publication = await M.Z.bookStep('math-theory: publish', 'publish', { Layer: 'theory', Applied: `${M.MATH_WORK}\\theory\\applied.json` })
    if (!publication || publication.ok !== true) throw new Error(`math theory publish: ${(publication && publication.error) || 'no output'}`)
    log(`Theory: publication ${publication.unit || ''} ${publication.state || ''}.`)
    return { status: 'done', theories: n(plan.theories), applied: { theories: n(applied.theories), answered: n(applied.answered) }, publication: { unit: publication.unit, state: publication.state, revision: publication.revision }, deepseek: M.deepseek }
  },
})
// ==== end module zt-math-theory ====
