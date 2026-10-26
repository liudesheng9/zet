// ==== module zt-book-terms v6 ====
// 4 (S5): a fault names its code and leaves the class to the ledger.
// 5 (#175): the chain is two script steps run by DeepSeek drive jobs under Haiku managers.
// 6 (#180): the finish step answers the table's counts (`terms`, `card`) instead of a nested report.
// The book term table, as ready ledger work: the `terms/book@1` unit that every digest group waits for. The
// layer driver runs the stage as script steps: `start` claims the one ready terms unit and runs the candidates
// plan (book-terms finds the bold terms, definition sentences, bracketed original-language terms and frequent
// word groups), the one DeepSeek job that chooses the terms and the words not to use runs in a Haiku manager, and
// `finish` applies the table: the script writes terms.md and the Card 术语表 before any digest run, through
// archive receipts, and completes the unit itself with its checked evidence. A failure is recorded against the
// unit, so the groups that depend on it stay waiting instead of digesting without the table.
defineModule({
  id: 'zt-book-terms', version: '6', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the claimed Term table (terms/book@1): plan → DeepSeek → apply, completing the unit, in two script steps run by DeepSeek drive jobs.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const termWork = `${Z.OUT}\\terms`
    const result = await Z.layerRun('terms', {
      layer: 'terms', take: 1, rounds: 1, phase: 'Prepare',
      dsh: termWork, writes: `${termWork}\\out`, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, what: 'the Term table writer',
    })
    if (result.status === 'failed') return { status: 'failed', error: result.error, report: null }
    const finish = result.finish || {}
    const report = { terms: Number(finish.terms || 0), with_avoid: Number(finish.with_avoid || 0), card: finish.card || null }
    if (!result.claimed) {
      log('Term table: no ready Terms unit (a disabled layer or a completed table); the digest goes on.')
      return { status: 'skipped', report: null, reason: 'no ready terms unit' }
    }
    if (!Number(result.verified || 0)) {
      const cause = String(result.error || 'the Term table steps gave no output').slice(0, 800)
      log(`Term table failed: ${cause}; the digest groups stay waiting on it.`)
      return { status: 'failed', report, error: cause }
    }
    log(`Term table: ${report.terms} terms (${report.with_avoid} with words not to use) in ${report.card}.`)
    return { status: 'done', report }
  },
})
// ==== end module zt-book-terms ====
