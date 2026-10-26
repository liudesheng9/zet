// ==== module zt-link-check v3 ====
// The Link check and the orphan pass on the whole book. In mode "full" the block cycles have already fixed the
// book, so the check runs every hard rule (the link pass can add R2, R4, and I2 faults); on an older book
// (mode "link") it keeps to L1 and L2 unless args.cardRules says otherwise.
// v2: the stage publishes its own `link-check` revision from its cycle report (`zt-digest finalize -Step stage`).
// The finalization refreshes that revision only when its one correction cycle changed a Card, so on
// acemoglu2025nobel trial 1 (nothing to correct) link-check/book@1 stayed ready and the run was never content-complete.
// v3 (#176, docs/workflow-module-structure-proposal.md): the card cycle is its declared book-steps.ps1 steps (plan
// with its jobs, one apply/check step per declared correction round, then apply and report) and the publication is
// one more step; each runs as a DeepSeek drive job, so no Claude relay runs a command.
defineModule({
  id: 'zt-link-check', version: '3', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the card layer cycle on the new Links and the orphans after the link pass, published as the link-check revision.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\cards-link`
    const cycle = await Z.cardCycle('link check', work, { rules: p.mode === 'full' ? '' : (Z.A.cardRules || 'L1,L2') })
    if (!cycle || cycle.status !== 'done') return cycle
    const publication = await Z.bookStep('link-check: publish', 'publish', { Layer: 'link-check', Report: `${work}\\report.json` })
    log(publication && publication.state ? `link-check: publication ${publication.unit || ''} ${publication.state}${publication.disposition ? ` (${publication.disposition})` : ''}.` : `link-check: the publication failed: ${(publication && publication.error) || 'no output'}`)
    return { ...cycle, publication: publication || { layer: 'link-check', state: 'failed', error: 'no output' } }
  },
})
// ==== end module zt-link-check ====
