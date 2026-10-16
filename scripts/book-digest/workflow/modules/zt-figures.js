// ==== module zt-figures v1 ====
// The book's figures in its Cards: attach-figures.ps1 puts every figure a Card names but does not show yet into
// that Card, and links the book's documents from the Literature Card. It runs after Organize, which rewrites bodies.
defineModule({
  id: 'zt-figures', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: show the book\'s own figures in the Cards that name them (attach-figures.ps1).',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const A = Z.A
    const { run, json: summary } = await Z.runScript('show figures', 'Figures',
      `pwsh -NoProfile -File ${Z.SCRIPTS}\\attach-figures.ps1 -CitationKey ${Z.KEY} -Staging "${Z.OUT}"${A.fileId ? ` -FileId ${A.fileId}` : ''} -Report "${Z.OUT}\\figures-report.json" -Apply`,
      ' (it may take several minutes)')
    log(summary ? `Figures: ${summary.shown} figures shown in ${summary.cards} Cards (${summary.figures} kept, ${summary.labelled} labelled); documents ${summary.literature ? 'linked' : 'already linked'}.` : `Figures failed: ${run && run.output}`)
    return summary ? { status: 'done', ...summary } : { status: 'failed', error: run && run.output }
  },
})
// ==== end module zt-figures ====
