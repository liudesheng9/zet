// ==== module zt-book-terms v1 ====
// The book term table: book-terms.ps1 finds the candidates, one DeepSeek job chooses the terms and the words
// not to use, the script writes terms.md and the Card 术语表 before any digest run. A failure is logged and the
// digest goes on without the table (the output's report is then null).
defineModule({
  id: 'zt-book-terms', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the book term table (book-terms.ps1 plan → DeepSeek → apply).',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const termWork = `${Z.OUT}\\terms`
    const tstep = (s, more = '') => `pwsh -NoProfile -File "${Z.SCRIPTS}\\book-terms.ps1" -Step ${s} -Staging "${Z.OUT}"${more}`
    const opts = { dsh: termWork, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: 'Prepare', writes: `${termWork}\\out`, remove: 'delete', keep: true }
    const planned = await Z.drive('terms: plan', { ...opts, command: tstep('plan') })
    const applied = await Z.drive('terms: apply', { ...opts, command: tstep('apply', ' -Apply'), cleanup: true })
    const report = zdLastJson(applied && applied.output)
    log(report && report.ok ? `Term table: ${report.terms} terms (${report.with_avoid} with words not to use) in ${report.card}.` : `Term table failed: ${(applied && applied.error) || (planned && planned.error) || 'no output'}; the digest goes on without it.`)
    return { report }
  },
})
// ==== end module zt-book-terms ====
