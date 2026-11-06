// ==== module zt-figures v11 ====
// 11 (2026-11-06, session history of 30 readers): 21 of them met `10-4 is not a label` 348 times, because a printed number
// was cut at its dash and readers were pushed to `10`. A printed number is read whole, and a reader copies each figure's
// own id (`F12`), never a printed number.
// 10 (2026-11-06): a reader job holds at most 80 Cards in reading order. Three top-level sections made jobs of 410 Cards
// on atkinson2015lectures, and those found half the pairs that an 80 to 110 Card job finds on the same Cards.
// 9 (2026-11-05, the owner's order to lower the cost): a reader job carries three whole groups, and the reader works from
// its message alone (no file, no image, two passes in thought, one write, one check). The one structure stands: one
// start, one queue of managers, one finish.
// 8 (docs/figure-explanation-design.md): the reader's list carries each figure's explanation from the Scholar Archive, and
// a figure with no printed label joins it under an id. A figure with no explanation stops the start step
// (explanations_open): zt-figure-explain makes them.
// 7 (2026-11-04, the owner's design): the one structure. The start step writes the book's figure list and one reader job
// per group; the managers run one queue of DeepSeek readers, each naming every figure related to each Card of its group,
// cited in the text or not; the finish step puts each figure in every Card related to it. One figure may stand in
// hundreds of Cards, because that helps the reader.
// The book's figures in its Cards: attach-figures puts every figure a Card names but does not show yet into that
// Card, and links the book's documents from the Literature Card. It runs after Organize, which rewrites bodies.
// 6 (#176, docs/workflow-module-structure-proposal.md): the figure pass and the stage's own publication are two
// book-steps.ps1 steps run by DeepSeek drive jobs; no Claude relay runs a command.
defineModule({
  id: 'zt-figures', version: '11', requires: ['zt-digest-lib'],
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
    const listed = await Z.bookStep('figures: list', 'figures-start', options)
    if (!listed || listed.ok !== true) return { status: 'failed', error: (listed && listed.error) || 'no figure list' }
    log(`Figures: ${listed.figures || 0} figures in the list, ${listed.jobs || 0} reader jobs.`)
    if (Number(listed.jobs || 0)) {
      await Z.runJobs('figures: read', listed.file, listed.jobs, { dsh: `${Z.OUT}\\figures`, writes: `${Z.OUT}\\figures\\out`, phase: 'Figures', skills: [], effort: Z.DSH_EFFORT, what: 'figure readers', run: true, chunks: Number(listed.chunks || 0) })
    }
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
