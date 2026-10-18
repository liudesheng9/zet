// ==== module zt-link-check v1 ====
// The Link check and the orphan pass on the whole book. In mode "full" the block cycles have already fixed the
// book, so the check runs every hard rule (the link pass can add R2, R4, and I2 faults); on an older book
// (mode "link") it keeps to L1 and L2 unless args.cardRules says otherwise.
defineModule({
  id: 'zt-link-check', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the card layer cycle on the new Links and the orphans after the link pass.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    return Z.runCards('link check', `${Z.OUT}\\cards-link`, '', p.mode === 'full' ? '' : (Z.A.cardRules || 'L1,L2'))
  },
})
// ==== end module zt-link-check ====
