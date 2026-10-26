// ==== module zt-math-skeleton v2 ====
// After Prepare (mode full): math-steps.ps1's skeleton step runs the statement index, writes the math prompt
// files (docs: the shared instructions with the math supplements appended) and builds the skeleton (a unit Model
// Card per definition, axiom, and result, a Theory Card per chapter) in ONE process (#178). Then, unless
// args.notation is false, one DeepSeek job writes the Book notation table and a second step applies it; a failed
// notation table is logged and the digest goes on (P8).
defineModule({
  id: 'zt-math-skeleton', version: '2', requires: ['zt-math-lib'],
  summary: 'ZT math digest: statement index, math prompt files, unit and Theory Card skeleton, notation table.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    // One chain, one drive job: index, the prompt files, and the applied skeleton.
    const start = M.must('skeleton', await M.step('math-skeleton', 'skeleton', 'start'))
    log(`Math: ${start.index.units} units (${JSON.stringify(start.index.forms)}), ${start.index.examples} examples, ${start.index.exercises} exercises; skeleton ${start.skeleton.created} units and ${start.skeleton.theories} Theory Cards.`)
    const reports = { index: start.index, docs: start.docs, skeleton: start.skeleton }
    if (M.A.notation !== false) {
      // The notation table: one DeepSeek job over the symbol candidates, then the step that writes the table.
      const plan = await M.step('math-skeleton', 'notation', 'notation plan')
      if (plan.ok === true) await M.jobs('math-skeleton: notation', plan, 'notation')
      const applied = await M.step('math-skeleton', 'notation-apply', 'notation apply')
      reports.notation = applied
      log(applied.ok === true ? `Notation table: ${applied.symbols} symbols in ${applied.card}.` : 'Notation table failed; the digest goes on without it.')
    }
    return { ...reports, deepseek: M.deepseek }
  },
})
// ==== end module zt-math-skeleton ====
