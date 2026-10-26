// ==== module zt-cards-book v6 ====
// The whole-book Card cycle as ready ledger work (modes cards and full, R296/R247): publish the current
// `cards-book` revision from the settled digest, Model and Cards contributions, claim it, run the one cycle in
// its declared steps (plan → DeepSeek jobs → apply/check rounds → apply → report), and record it with
// `book-cards-finish`, which re-derives the deterministic pass and requires one reviewed owner per Model draft.
// The cycle's coverage gate waits for the delegated Model reviews to settle while model compilation never waits for
// it (R248). In full it also reviews the source of a Model draft no model chain owns.
// 5 (#176, docs/workflow-module-structure-proposal.md): the publication, the claim, the cycle's steps and the
// checked completion are book-steps.ps1 steps run by DeepSeek drive jobs; no Claude relay runs a command.
// 6 (live run 2026-10-26, staging5): the claim holds the layers' longest lease (LAYER_LEASE): a 300-second lease
// expired during the cycle and book-cards-finish refused it as stale_claim.
defineModule({
  id: 'zt-cards-book', version: '6', requires: ['zt-digest-lib'],
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
    // The publication revision is derived from the present contributions; the same manifest reuses it.
    const published = await Z.bookStep('cards publish', 'publish', { Layer: 'cards-book', Ledger: true })
    const unit = published && published.unit
    if (!unit) return { status: 'failed', error: (published && published.error) || 'the cards-book revision was not registered' }
    const got = await Z.bookStep('cards-book claim', 'cards-book-start', { Wait: 300, Lease: Z.LAYER_LEASE })
    if (!got || got.ok !== true) return { status: 'failed', error: (got && got.error) || 'the cards-book claim failed' }
    const claimed = got.unit ? { id: got.unit, token: got.token, input_hash: got.input_hash, inputs: got.inputs || {} } : null
    if (!claimed) return { status: 'active', error: `the ${unit} revision is waiting for its Model reviews to settle`, gate: got.gate }
    log(`Cards whole book: one cycle over ${Z.KEY} on ${claimed.id}${modelReview ? ', reviewing the source of every Model draft no model chain owns' : ''}.`)
    const cycle = await Z.cardCycle('whole book', `${Z.OUT}\\cards`, { sample: Z.IDEA_SAMPLE, modelReview })
    if (cycle.status !== 'done') {
      // v4 (S5): the code is the fact (no result); the ledger classifies it.
      await Z.bookStep('cards-book fault', 'unit-fail', {
        Unit: claimed.id, Token: claimed.token, InputHash: claimed.input_hash,
        Code: 'missing_result', Cause: String(cycle.error || 'the whole-book card cycle did not complete').slice(0, 800),
      })
      return { status: 'failed', ...cycle, unit: claimed.id }
    }
    const finished = await Z.bookStep('cards-book finish', 'cards-book-finish', { Unit: claimed.id, Token: claimed.token, InputHash: claimed.input_hash, Work: `${Z.OUT}\\cards` })
    if (!finished || finished.ok !== true) {
      log(`Cards whole book: the cycle ran but its checked completion failed: ${(finished && finished.error) || 'no output'}`)
      return { status: 'partial', cycle, unit: claimed.id, error: (finished && finished.error) || 'no output' }
    }
    return { status: 'done', cycle, unit: claimed.id, completion: finished }
  },
})
// ==== end module zt-cards-book ====

