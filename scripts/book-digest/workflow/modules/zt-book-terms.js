// ==== module zt-book-terms v3 ====
// The book term table, as ready ledger work: the `terms/book@1` unit that every digest group waits for. The
// chain claims it, plans the candidates (book-terms finds the bold terms, definition sentences, bracketed
// original-language terms and frequent word groups), runs the one DeepSeek job that chooses the terms and the
// words not to use, and applies the table: the script writes terms.md and the Card 术语表 before any digest run,
// through archive receipts, and completes the unit itself with its checked evidence. A failure is recorded
// against the unit, so the groups that depend on it stay waiting instead of digesting without the table.
defineModule({
  id: 'zt-book-terms', version: '3', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the claimed Term table (terms/book@1): plan → DeepSeek → apply, completing the unit.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const termWork = `${Z.OUT}\\terms`
    const got = await Z.claim('terms claim', 'terms', 1, 0)
    if (!got.ok) return { status: 'failed', error: got.error, report: null }
    const unit = (got.units || [])[0]
    if (!unit) {
      log('Term table: no ready Terms unit (a disabled layer or a completed table); the digest goes on.')
      return { status: 'skipped', report: null, reason: 'no ready terms unit' }
    }
    const token = unit.token
    const tstep = (s, more = '') => `${Z.tool('book-terms')} -Step ${s} -Staging "${Z.OUT}" -Token ${token}${more}`
    const chained = await Z.chain('terms', {
      dsh: termWork, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: 'Prepare', writes: `${termWork}\\out`,
      steps: [
        { name: 'plan', command: tstep('plan'), jobs: true },
        { name: 'apply', command: tstep('apply', ' -Apply') },
      ],
      cleanup: true, keep: true,
    })
    const report = Z.jsonOf(chained, 'apply')
    const ok = report && (report.ok === true || report.state === 'done')
    if (!ok) {
      const cause = String((Z.stepOf(chained, 'apply') || {}).error || (Z.stepOf(chained, 'plan') || {}).error || 'no output from the Term table steps').slice(0, 800)
      await Z.failUnit('terms fault', unit.id, token, 'content', { schema_version: 1, unit: unit.id, input_hash: unit.input_hash || '', code: 'check_failed', cause })
      log(`Term table failed: ${cause}; the digest groups stay waiting on it.`)
      return { status: 'failed', report, error: cause }
    }
    log(`Term table: ${report.terms} terms (${report.with_avoid} with words not to use) in ${report.card}.`)
    return { status: 'done', unit: unit.id, report }
  },
})
// ==== end module zt-book-terms ====
