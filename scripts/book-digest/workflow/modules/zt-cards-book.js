// ==== module zt-cards-book v2 ====
// Mode "cards": the card layer cycle on the whole book, with orphans (matrix: plan → DeepSeek JSON → apply in
// shares → check → one fix round → report). Mode "full": the same one whole-book cycle (R296), after the digest
// runs; it also reviews the source of every Model draft no model chain elaborates (R247), which is the case when
// the model layer is switched off for this run. A draft an active model chain owns keeps that chain's review.
defineModule({
  id: 'zt-cards-book', version: '2', requires: ['zt-digest-lib'],
  summary: 'ZT digest: one whole-book card cycle (modes cards and full), with the Model review it owns.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    // In mode cards no model chain runs in this invocation; in full the chain exists unless args.models is false.
    const modelReview = Z.MODE === 'cards' || Z.A.models === false
    const sample = Z.IDEA_SAMPLE ? ` -Sample ${Z.IDEA_SAMPLE}` : ''
    log(`Cards whole book: one cycle over ${Z.KEY}${modelReview ? ', reviewing the source of every Model draft no model chain owns' : ''}.`)
    return Z.runCards('whole book', `${Z.OUT}\\cards`, sample, '', { modelReview })
  },
})
// ==== end module zt-cards-book ====
