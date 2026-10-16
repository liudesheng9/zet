// ==== module zt-cards-book v1 ====
// Mode "cards": the card layer cycle on the whole book, with orphans (card-compiler.ps1 plan → DeepSeek JSON →
// apply in shares → check → one fix round → report).
defineModule({
  id: 'zt-cards-book', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the card layer cycle on the whole book (mode cards).',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    return Z.runCards('whole book', `${Z.OUT}\\cards`, Z.IDEA_SAMPLE ? ` -Sample ${Z.IDEA_SAMPLE}` : '')
  },
})
// ==== end module zt-cards-book ====
