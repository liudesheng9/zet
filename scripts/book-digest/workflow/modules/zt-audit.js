// ==== module zt-audit v3 ====
// The audit: digest-audit.ps1, read-only, on one book (args.citationKey) or on every book of the agent Workspace.
// It changes no Card; it writes audit.json, audit.md, and audit-faults.txt in args.out.
defineModule({
  id: 'zt-audit', version: '3', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the read-only audit of the book\'s Cards (digest-audit.ps1).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    if (await Z.settledOnResume('audit', ['audit'])) return { status: 'done', reused: true }
    const { run, json: summary } = await Z.runScript('audit cards', 'Audit',
      `${Z.tool('digest-audit')}${Z.KEY ? ` -CitationKey ${Z.KEY}` : ''} -Out "${Z.OUT}"`,
      ' (it may take several minutes)')
    if (!summary) { log(`Audit failed: ${run && run.output}`); return { status: 'failed', error: run && run.output } }
    for (const b of summary.books || []) log(`Audit ${b.key}: ${b.cards} Cards in ${b.topic_trees} Topic trees; ${b.style_faults} Card language faults; ${b.layer_faults ?? 0} Card layer faults (${b.layer_advisory ?? 0} advisory); ${b.orphans} orphans; ${b.no_source} with no source; ${b.unkinded} with no kinded Link; ${b.not_self_sufficient} of ${b.models} models not self-sufficient; ${b.explained ?? 0} of ${b.derivations ?? 0} Derivation parts with 思路 and 解读; ${b.figures_missing} figures named but not shown.`)
    log(`Audit: ${summary.hubs_missing} missing hubs, ${summary.broken_links} broken Links in the Workspace. Report: ${Z.OUT}\\audit.md`)
    return { status: 'done', ...summary }
  },
})
// ==== end module zt-audit ====
