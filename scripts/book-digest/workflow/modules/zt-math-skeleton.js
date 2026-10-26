// ==== module zt-math-skeleton v3 ====
// After Prepare (mode full): math-steps.ps1's skeleton step runs the statement index, writes the math prompt
// files (docs: the shared instructions with the math supplements appended) and builds the skeleton (a unit Model
// Card per definition, axiom, and result, a Theory Card per chapter) in ONE process (#178). Then, unless
// args.notation is false, one DeepSeek job writes the Book notation table and a second step applies it; a failed
// notation table is logged and the digest goes on (P8).
// 3 (#180): the steps answer counts (`index_units`, `index_forms`, `docs_files`, `skeleton_units`, `symbols`),
// so the stage reports them and keeps no nested tool line.
defineModule({
  id: 'zt-math-skeleton', version: '3', requires: ['zt-math-lib'],
  summary: 'ZT math digest: statement index, math prompt files, unit and Theory Card skeleton, notation table.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    const n = (v) => Number(v || 0)
    // One chain, one drive job: index, the prompt files, and the applied skeleton. Its line carries their counts.
    const start = M.must('skeleton', await M.step('math-skeleton', 'skeleton', 'start'))
    const index = { units: n(start.index_units), forms: n(start.index_forms), examples: n(start.index_examples), exercises: n(start.index_exercises) }
    const docs = { files: n(start.docs_files) }
    const skeleton = { units: n(start.skeleton_units), theories: n(start.skeleton_theories) }
    log(`Math: ${index.units} units (${index.forms} forms), ${index.examples} examples, ${index.exercises} exercises; skeleton ${skeleton.units} units and ${skeleton.theories} Theory Cards.`)
    const reports = { index, docs, skeleton }
    if (M.A.notation !== false) {
      // The notation table: one DeepSeek job over the symbol candidates, then the step that writes the table.
      const plan = await M.step('math-skeleton', 'notation', 'notation plan')
      if (plan.ok === true) await M.jobs('math-skeleton: notation', plan, 'notation')
      const applied = await M.step('math-skeleton', 'notation-apply', 'notation apply')
      reports.notation = { ok: applied.ok === true, symbols: n(applied.symbols), card: applied.card || null }
      log(applied.ok === true ? `Notation table: ${n(applied.symbols)} symbols in ${applied.card}.` : 'Notation table failed; the digest goes on without it.')
    }
    return { ...reports, deepseek: M.deepseek }
  },
})
// ==== end module zt-math-skeleton ====
