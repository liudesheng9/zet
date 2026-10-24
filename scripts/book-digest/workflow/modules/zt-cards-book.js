// ==== module zt-cards-book v4 ====
// The whole-book Card cycle as ready ledger work (modes cards and full, R296/R247): publish the current
// `cards-book` revision from the settled digest, Model and Cards contributions, claim it, run the one cycle in
// ONE manager chain (plan → DeepSeek JSON → apply in shares → check → one fix round → report), and record it
// with `book-cards-finish`, which re-derives the deterministic pass and requires one reviewed owner per Model
// draft. The cycle's coverage gate waits for the delegated Model reviews to settle while model compilation
// never waits for it (R248). In full it also reviews the source of a Model draft no model chain owns.
defineModule({
  id: 'zt-cards-book', version: '4', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the claimed whole-book card cycle (modes cards and full), recorded by book-cards-finish.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    // In mode cards no model chain runs in this invocation; in full the chain exists unless args.models is false.
    const modelReview = Z.MODE === 'cards' || Z.A.models === false
    const sample = Z.IDEA_SAMPLE ? ` -Sample ${Z.IDEA_SAMPLE}` : ''
    // The publication revision is derived from the present contributions; the same manifest reuses it.
    const published = await Z.runScript('cards publish', 'Cards', `${Z.LEDGER} publish -Out "${Z.OUT}" -Layer cards-book`, ' (the same present inputs reuse the registered revision)')
    const unit = published.json && published.json.unit
    if (!unit) return { status: 'failed', error: (published.run && published.run.output) || 'the cards-book revision was not registered' }
    const got = await Z.claim('cards-book claim', 'cards-book', 1, 300)
    if (!got.ok) return { status: 'failed', error: got.error }
    const claimed = (got.units || []).find(u => u.id === unit) || (got.units || [])[0]
    if (!claimed) return { status: 'active', error: `the ${unit} revision is waiting for its Model reviews to settle`, gate: got }
    log(`Cards whole book: one cycle over ${Z.KEY} on ${claimed.id}${modelReview ? ', reviewing the source of every Model draft no model chain owns' : ''}.`)
    const cycle = await Z.runCards('whole book', `${Z.OUT}\\cards`, sample, '', { modelReview })
    if (cycle.status !== 'done') {
      // v4 (S5): the code is the fact (no result); the ledger classifies it.
      await Z.failUnit('cards-book fault', claimed.id, claimed.token, {
        schema_version: 1, unit: claimed.id, input_hash: claimed.input_hash || '',
        code: 'missing_result', cause: String(cycle.error || 'the whole-book card cycle did not complete').slice(0, 800),
      })
      return { status: 'failed', ...cycle, unit: claimed.id }
    }
    const finished = await Z.finishOrFault('cards-book finish', claimed.id, claimed.token, claimed.inputs, { work: `${Z.OUT}\\cards` })
    if (finished.status !== 'done') {
      log(`Cards whole book: the cycle ran but its checked completion failed: ${finished.error}`)
      return { status: 'partial', cycle, unit: claimed.id, error: finished.error }
    }
    return { status: 'done', cycle, unit: claimed.id, completion: finished.result }
  },
})
// ==== end module zt-cards-book ====
