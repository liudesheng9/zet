// ==== module zt-audit v6 ====
// The audit: digest-audit, read-only, on one book (args.citationKey) or on every book of the agent Workspace.
// It changes no Card; it writes audit.json, audit.md, and audit-faults.txt in args.out.
// 4 (S26, #169): the audit publishes the per-book and Workspace Term quality beside its own completion, so the
// stage log and the run report show Term coverage debt without reading the whole audit report.
// 5 (#176): the audit's one command is a book-steps.ps1 step (`audit`) run by a DeepSeek drive job.
// 6 (#180): the step answers the audit's counts (the per-book rows stay in its detail file), so the stage logs one
// truthful summary line and returns the same counts.
defineModule({
  id: 'zt-audit', version: '6', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the read-only audit of the book\'s Cards (one book-steps.ps1 step), with its Term coverage and quality.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    if (await Z.settledOnResume('audit', ['audit'])) return { status: 'done', reused: true }
    const summary = await Z.bookStep('audit cards', 'audit', Z.KEY ? { CitationKey: Z.KEY } : {})
    if (!summary || summary.ok !== true) {
      const error = (summary && summary.error) || 'no output'
      log(`Audit failed: ${error}`)
      return { status: 'failed', error }
    }
    const n = (v) => Number(v || 0)
    log(`Audit: ${n(summary.books)} book(s), ${n(summary.cards)} Cards; ${n(summary.style_faults)} Card language faults; ${n(summary.layer_faults)} Card layer faults; ${n(summary.orphans)} orphans; ${n(summary.no_source)} with no source; ${n(summary.unkinded)} with no kinded Link; ${n(summary.not_self_sufficient)} of ${n(summary.models)} models not self-sufficient; ${n(summary.explained)} of ${n(summary.derivations)} Derivation parts with 思路 and 解读; ${n(summary.figures_missing)} figures named but not shown. Terms ${summary.terms || 'unknown'} (${n(summary.term_published)}/${n(summary.term_rows)} rows, ${n(summary.term_uses)} uses, ${n(summary.term_faults)} T faults).`)
    log(`Audit: ${n(summary.hubs_missing)} missing hubs, ${n(summary.broken_links)} broken Links in the Workspace. Report: ${Z.OUT}\\audit.md`)
    return { status: 'done', ...summary, ok: undefined }
  },
})
// ==== end module zt-audit ====
