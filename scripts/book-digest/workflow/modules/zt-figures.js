// ==== module zt-figures v6 ====
// The book's figures in its Cards: attach-figures puts every figure a Card names but does not show yet into that
// Card, and links the book's documents from the Literature Card. It runs after Organize, which rewrites bodies.
// 6 (#176, docs/workflow-module-structure-proposal.md): the figure pass and the stage's own publication are two
// book-steps.ps1 steps run by DeepSeek drive jobs; no Claude relay runs a command.
defineModule({
  id: 'zt-figures', version: '6', requires: ['zt-digest-lib'],
  summary: 'ZT digest: show the book\'s own figures in the Cards that name them (one book-steps.ps1 step).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    if (await Z.settledOnResume('figures', ['figures'])) return { status: 'done', reused: true }
    const A = Z.A
    const options = { CitationKey: Z.KEY }
    if (A.fileId) options.FileId = A.fileId
    const summary = await Z.bookStep('show figures', 'figures', options)
    log(summary && summary.ok === true ? `Figures: ${summary.shown} figures shown in ${summary.cards} Cards (${summary.figures} kept, ${summary.labelled} labelled); documents ${summary.literature ? 'linked' : 'already linked'}.` : `Figures failed: ${(summary && summary.error) || 'no output'}`)
    if (!summary || summary.ok !== true) return { status: 'failed', error: (summary && summary.error) || 'no output' }
    // The stage publishes its own book revision (`zt-digest finalize -Step stage`): the ledger binds its artifacts
    // by hash and checks the coverage it derives.
    const publication = await Z.bookStep('figures: publish', 'publish', { Layer: 'figures', Report: `${Z.OUT}\\figures-report.json` })
    log(publication && publication.state ? `figures: publication ${publication.unit || ''} ${publication.state}${publication.disposition ? ` (${publication.disposition})` : ''}.` : `figures: the publication failed: ${(publication && publication.error) || 'no output'}.`)
    return { status: 'done', ...summary, ok: undefined, publication }
  },
})
// ==== end module zt-figures ====
