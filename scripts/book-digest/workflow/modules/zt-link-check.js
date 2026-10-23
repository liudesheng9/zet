// ==== module zt-link-check v2 ====
// The Link check and the orphan pass on the whole book. In mode "full" the block cycles have already fixed the
// book, so the check runs every hard rule (the link pass can add R2, R4, and I2 faults); on an older book
// (mode "link") it keeps to L1 and L2 unless args.cardRules says otherwise.
// v2: the stage publishes its own `link-check` revision from its cycle report (`zt-digest finalize -Step stage`).
// The finalization refreshes that revision only when its one correction cycle changed a Card, so on
// acemoglu2025nobel trial 1 (nothing to correct) link-check/book@1 stayed ready and the run was never content-complete.
defineModule({
  id: 'zt-link-check', version: '2', requires: ['zt-digest-lib'],
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
    const cycle = await Z.runCards('link check', work, '', p.mode === 'full' ? '' : (Z.A.cardRules || 'L1,L2'))
    if (!cycle || cycle.status !== 'done') return cycle
    const { run, json } = await Z.runScript('link-check: publish', 'Link', `${Z.DIGEST} finalize -Step stage -Out "${Z.OUT}" -Layer link-check -Report "${work}\\report.json"`, ' (it records the stage publication in the ledger)')
    log(json && json.state ? `link-check: publication ${json.unit || ''} ${json.state}${json.disposition ? ` (${json.disposition})` : ''}.` : `link-check: the publication failed: ${(run && run.output) || 'no output'}`)
    return { ...cycle, publication: json || { layer: 'link-check', state: 'failed', error: (run && run.output) || 'no output' } }
  },
})
// ==== end module zt-link-check ====
