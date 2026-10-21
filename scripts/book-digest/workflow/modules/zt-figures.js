// ==== module zt-figures v4 ====
// The book's figures in its Cards: attach-figures.ps1 puts every figure a Card names but does not show yet into
// that Card, and links the book's documents from the Literature Card. It runs after Organize, which rewrites bodies.
defineModule({
  id: 'zt-figures', version: '4', requires: ['zt-digest-lib'],
  summary: 'ZT digest: show the book\'s own figures in the Cards that name them (attach-figures.ps1).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    if (await Z.settledOnResume('figures', ['figures'])) return { status: 'done', reused: true }
    const A = Z.A
    // The stage publishes its own book revision (`zt-digest finalize -Step stage`): the ledger binds its artifacts
    // by hash and checks the coverage it derives. On grossman2002integration trial 8 no stage published, so the
    // revision stayed ready and the run could never be content-complete.
    const publish = async (layer, more) => {
      const { run, json } = await Z.runScript(`${layer}: publish`, 'Figures', `${Z.DIGEST} finalize -Step stage -Out "${Z.OUT}" -Layer ${layer}${more}`, ' (it records the stage publication in the ledger)')
      log(json && json.state ? `${layer}: publication ${json.unit || ''} ${json.state}${json.disposition ? ` (${json.disposition})` : ''}.` : `${layer}: the publication failed: ${(run && run.output) || 'no output'}`)
      return json || { layer, state: 'failed', error: (run && run.output) || 'no output' }
    }
    const { run, json: summary } = await Z.runScript('show figures', 'Figures',
      `${Z.tool('attach-figures')} -CitationKey ${Z.KEY} -Staging "${Z.OUT}"${A.fileId ? ` -FileId ${A.fileId}` : ''} -Report "${Z.OUT}\\figures-report.json" -Apply`,
      ' (it may take several minutes)')
    log(summary ? `Figures: ${summary.shown} figures shown in ${summary.cards} Cards (${summary.figures} kept, ${summary.labelled} labelled); documents ${summary.literature ? 'linked' : 'already linked'}.` : `Figures failed: ${run && run.output}`)
    if (!summary) return { status: 'failed', error: run && run.output }
    const publication = await publish('figures', ` -Report "${Z.OUT}\\figures-report.json"`)
    return { status: 'done', ...summary, publication }
  },
})
// ==== end module zt-figures ====
