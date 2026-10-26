// ==== module zt-audit v5 ====
// The audit: digest-audit, read-only, on one book (args.citationKey) or on every book of the agent Workspace.
// It changes no Card; it writes audit.json, audit.md, and audit-faults.txt in args.out.
// 4 (S26, #169): the audit publishes the per-book and Workspace Term quality beside its own completion, so the
// stage log and the run report show Term coverage debt without reading the whole audit report, and "the audit ran"
// never reads as "all terms passed" (R0441/R0442).
// 5 (#176, docs/workflow-module-structure-proposal.md): the audit's one command is a book-steps.ps1 step (`audit`)
// run by a DeepSeek drive job; the resume reads its own gate step. No Claude relay runs a command.
defineModule({
  id: 'zt-audit', version: '5', requires: ['zt-digest-lib'],
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
    for (const b of summary.books || []) log(`Audit ${b.key}: ${b.cards} Cards in ${b.topic_trees} Topic trees; ${b.style_faults} Card language faults; ${b.layer_faults ?? 0} Card layer faults (${b.layer_advisory ?? 0} advisory); ${b.orphans} orphans; ${b.no_source} with no source; ${b.unkinded} with no kinded Link; ${b.not_self_sufficient} of ${b.models} models not self-sufficient; ${b.explained ?? 0} of ${b.derivations ?? 0} Derivation parts with 思路 and 解读; ${b.figures_missing} figures named but not shown; terms ${b.terms ?? 'unknown'} (${b.term_published ?? 0}/${b.term_rows ?? 0} rows, ${b.term_uses ?? 0} uses, ${b.term_faults ?? 0} T faults).`)
    const terms = summary.terms || {}
    log(`Audit: ${summary.hubs_missing} missing hubs, ${summary.broken_links} broken Links in the Workspace; Term quality ${terms.status ?? 'unknown'} (${terms.published ?? 0}/${terms.rows ?? 0} rows, ${terms.heads ?? 0} heads, ${terms.uses ?? 0} uses, ${terms.faults ?? 0} T faults). Report: ${Z.OUT}\\audit.md`)
    return { status: 'done', ...summary, ok: undefined }
  },
})
// ==== end module zt-audit ====
