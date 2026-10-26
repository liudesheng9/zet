// ==== module zt-math-check v2 ====
// The math check (#178): one math-steps.ps1 drive job runs math-compiler.ps1 -Step check, which reports rules
// M1 to M18 on every unit and Theory Card (math-audit.json, math-audit.md in args.out). Read-only.
defineModule({
  id: 'zt-math-check', version: '2', requires: ['zt-math-lib'],
  summary: 'ZT math digest: rules M1 to M18 on every unit and Theory Card (read-only).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    return M.must('check', await M.step('math-check', 'check', 'check'))
  },
})
// ==== end module zt-math-check ====
