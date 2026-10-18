// ==== module zt-math-check v1 ====
// The math check: math-compiler.ps1 -Step check reports rules M1 to M18 on every unit and Theory Card
// (math-audit.json, math-audit.md in args.out). Read-only.
defineModule({
  id: 'zt-math-check', version: '1', requires: ['zt-math-lib'],
  summary: 'ZT math digest: rules M1 to M18 on every unit and Theory Card (read-only).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const M = zmMake(ctx, p)
    const out = await M.runMathScript('check', 'check', ` -Out "${M.Z.OUT}"`, 'Audit')
    return out || { status: 'failed', error: 'math-compiler.ps1 -Step check failed' }
  },
})
// ==== end module zt-math-check ====
