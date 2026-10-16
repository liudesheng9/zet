// ==== module zt-math-skeleton v1 ====
// After Prepare (mode full): math-compiler.ps1 finds every numbered statement (index), writes the math prompt
// files (docs: the shared instructions with the math supplements appended), and builds the skeleton (a unit Model
// Card per definition, axiom, and result, a Theory Card per chapter). A failed step fails the stage. Then, unless
// args.notation is false, one DeepSeek job writes the Book notation table (a failure is logged; the digest goes on).
defineModule({
  id: 'zt-math-skeleton', version: '1', requires: ['zt-math-lib'],
  summary: 'ZT math digest: statement index, math prompt files, unit and Theory Card skeleton, notation table.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    const reports = {}
    for (const [what, s, extra] of [['index', 'index', ''], ['docs', 'docs', ''], ['skeleton', 'skeleton', ' -Apply']]) {
      const done = await M.runMathScript(what, s, extra, 'Prepare')
      if (!done) throw new Error(`math ${what}: math-compiler.ps1 -Step ${s} failed`)
      reports[what] = done
    }
    log(`Math: ${reports.index.units} units (${JSON.stringify(reports.index.forms)}), ${reports.index.examples} examples, ${reports.index.exercises} exercises; skeleton ${reports.skeleton.created} units and ${reports.skeleton.theories} Theory Cards.`)
    if (M.A.notation !== false) {
      const nopts = { ...M.mathOpts('notation', 'Prepare') }
      await M.Z.drive('notation: plan', { ...nopts, command: M.mstep('notation') })
      reports.notation = zdLastJson(((await M.Z.drive('notation: apply', { ...nopts, command: M.mstep('notation-apply', ' -Apply'), cleanup: true })) || {}).output)
      log(reports.notation && reports.notation.ok ? `Notation table: ${reports.notation.symbols} symbols in ${reports.notation.card}.` : 'Notation table failed; the digest goes on without it.')
    }
    return reports
  },
})
// ==== end module zt-math-skeleton ====
