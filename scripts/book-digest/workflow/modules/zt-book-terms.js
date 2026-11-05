// ==== module zt-book-terms v9 ====
// 9 (2026-11-05): the block size is gone; the layer driver names no block.
// 4 (S5): a fault names its code and leaves the class to the ledger.
// 5 (#175): the chain is two script steps run by DeepSeek drive jobs under Haiku managers.
// 6 (#180): the finish step answers the table's counts (`terms`, `card`) instead of a nested report.
// 7 (the owner, 2026-11-01): no upper bound on the number of terms, and the one writer becomes explorers and a
// merge. `start` splits the book into parts of about 30,000 characters and lists one DeepSeek explorer job per
// part; the explorers run in Haiku managers (at most 8 per manager, the managers in parallel); `apply` (terms
// -Step merge) checks every explorer answer and lists the one DeepSeek merge job, run by a Haiku manager, that
// writes the table's answer; `finish` applies it as before.
// 8 (test box run 2026-11-01): the report reads the finish line that applied the table from `layerRun`'s
// `finishes` (it read a `finish` field the driver never returns, so a 259-term table was reported as 0 terms and
// no Card).
// The book term table, as ready ledger work: the `terms/book@1` unit that every digest group waits for. The
// layer driver runs the stage as script steps: `start` claims the one ready terms unit and runs the candidates
// plan of each explorer part (the bold terms, definition sentences, bracketed original-language terms and frequent
// word groups of the part's sections), the explorer jobs propose terms and the words not to use, `apply` gives
// every proposal to the one merge job that writes the final list, and `finish` applies the table: the script
// writes terms.md and the Card 术语表 before any digest run, through archive receipts, and completes the unit
// itself with its checked evidence. A failure is recorded against the unit, so the groups that depend on it stay
// waiting instead of digesting without the table.
defineModule({
  id: 'zt-book-terms', version: '9', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the claimed Term table (terms/book@1), uncapped: plan → DeepSeek explorers (one per part of the book) → merge → one DeepSeek merge job → apply, completing the unit, in three script steps run by DeepSeek drive jobs.',
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
      layer: 'terms', rounds: 1, phase: 'Prepare', apply: true,
      dsh: termWork, writes: `${termWork}\\out`, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, what: 'the Term table explorers and merge',
      onApply: (merged) => log(`Term table: ${Number(merged.parts || 0)} explorer parts proposed ${Number(merged.rows || 0)} rows (${Number(merged.distinct || 0)} distinct words); one merge job writes the list.`),
    })
    if (result.status === 'failed') return { status: 'failed', error: result.error, report: null }
    const finish = (result.finishes || []).find((f) => f && f.card) || {}
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
