// ==== module zt-digest-report v7 ====
// The run's result, in the shape zt-book-digest.js returned for each mode: a stage-only mode returns that
// stage's report; mode full (and link, and models or hubs with organize) returns the whole digest report. A
// variant's own mode (math) returns {mode, dsh_model, citation_key} with the variant's extra reports; in the
// other modes the extra reports join the whole report (e.g. math: {...}). 4.0.0 adds the effective settings
// (maxAttempts, dshRunCap), the Cards layer's gate, and a schedule summary of what every group chain and layer
// driver settled: verified, failed, retrying and open work, never a claim of work that no gate confirmed.
defineModule({
  id: 'zt-digest-report', version: '7', requires: ['zt-digest-lib'],
  summary: 'ZT digest: assemble the run\'s result from the stage outputs, per mode, with the settings and the schedule summary.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'any', description: 'the DeepSeek model from the source stage' },
    groups: { type: 'any', description: 'the group count' },
    digest: { type: 'any', description: 'the digest stage output (its per-group chains)' },
    cards: { type: 'any', description: 'the Cards layer output (the per-group offline checks)' },
    models: { type: 'any', description: 'the output of the model layer (the models stage of a book type)' },
    terms: { type: 'any', description: 'the term table stage output' },
    cardsBook: { type: 'any', description: 'the whole-book card cycle (mode cards)' },
    explain: { type: 'any', description: 'the explain stage output (mode explain)' },
    hubsFirst: { type: 'any', description: 'the hub stage output in mode hubs' },
    link: { type: 'any', description: 'the link pass output' },
    linkCheck: { type: 'any', description: 'the link check card cycle' },
    cleanup: { type: 'any', description: 'the cleanup stage output' },
    organize: { type: 'any', description: 'the Organize stage output' },
    figures: { type: 'any', description: 'the figures stage output' },
    bridge: { type: 'any', description: 'Bridge report or explicit apply summary' },
    hubs: { type: 'any', description: 'the hub stage output' },
    audit: { type: 'any', description: 'the audit stage output' },
    extra: { type: 'object', default: {}, description: 'a variant\'s own reports, added to the result (e.g. {"math": {...}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const A = Z.A, MODE = p.mode, KEY = Z.KEY
    const dshModel = p.model || Z.DSH_MODEL
    const extra = p.extra || {}
    const or = (v, d) => (v === undefined ? d : v)
    // The group stage is a per-item chain pipeline now: each item's Digest chain is its own report.
    const digestReports = Array.isArray(p.digest && p.digest.items)
      ? p.digest.items.map(i => ({ group: (i.item && i.item.n) ?? null, ...(i.steps && i.steps.group ? i.steps.group : {}), status: i.ok ? ((i.steps && i.steps.group && i.steps.group.status) || 'done') : 'failed', errors: i.error || null }))
      : (p.digest && p.digest.reports) || []
    const layerSummary = (value) => value && value.dispositions
      ? { status: value.status, verified: value.dispositions.filter(d => d.status === 'done').length, failed: value.dispositions.filter(d => d.status === 'failed').length, retrying: value.dispositions.filter(d => d.status === 'retry').length, open: (value.gate && value.gate.counts && (value.gate.counts.open || 0)) || 0, complete: !!(value.gate && value.gate.complete) }
      : null
    const schedule = {
      settings: { maxAttempts: Z.MAX_ATTEMPTS, dshRunCap: Z.DSH_RUN_CAP },
      digest: Array.isArray(p.digest && p.digest.items) ? { verified: digestReports.filter(r => r.status === 'done').length, failed: digestReports.filter(r => r.status !== 'done').length } : null,
      cards: layerSummary(p.cards),
      models: layerSummary(p.models),
      explain: layerSummary(p.explain),
      events: layerSummary((extra && extra.events) || null),
      findings: layerSummary((extra && extra.findings) || null),
    }
    if (MODE === 'bridge') return { mode: MODE, citation_key: KEY, bridge: or(p.bridge, { status: 'failed', error: 'Bridge stage produced no summary' }) }
    if (MODE === 'cards') return { mode: MODE, dsh_model: dshModel, citation_key: KEY, cards: or(p.cardsBook, null), schedule }
    // One whole-book card cycle (R296): book is that cycle's report, link the link check's, groups the per-group checks.
    const cardsReport = { book: or(p.cardsBook, null), link: or(p.linkCheck, null), groups: layerSummary(p.cards) }
    if (MODE === 'explain') return { mode: MODE, dsh_model: dshModel, citation_key: KEY, explain: or(p.explain, null), schedule }
    if (MODE === 'figures') return { mode: MODE, citation_key: KEY, figures: or(p.figures, null) }
    if (MODE === 'audit') return { mode: MODE, citation_key: KEY || null, audit: or(p.audit, null) }
    if (MODE === 'models' && !A.organize) return { mode: MODE, dsh_model: dshModel, book: A.bookTitle, citation_key: KEY, models: (p.models && p.models.models) || [], model_cleanup: (p.models && p.models.model_cleanup) || null, schedule }
    if (MODE === 'hubs' && !A.organize) return { mode: MODE, dsh_model: dshModel, hubs: or(p.hubsFirst, null) }
    if (!['full', 'link', 'models', 'hubs', 'resume'].includes(MODE)) return { mode: MODE, dsh_model: dshModel, citation_key: KEY, ...extra, schedule }
    const modelReports = (p.models && p.models.models) || []
    const linkReports = (p.link && p.link.reports) || []
    const groupCount = p.groups || 0
    return {
      mode: MODE, dsh_model: dshModel, book: A.bookTitle, citation_key: KEY, groups: groupCount, managers_per_pass: Z.toBlocks(Z.groupsOf(groupCount)).length,
      digest: digestReports.map(r => ({ group: r.group, status: r.status, unit: r.unit || null, disposition: r.disposition || null, owned: r.owned ?? null, errors: r.errors || null })),
      models: modelReports.map(r => ({ model: r.model, level: r.level, status: r.status, parts: (r.parts || []).length, relations: r.relations, kinds: r.kinds, charts: r.charts, derivations: r.derivations, split: r.split, moved: r.moved, shared: r.shared, tolerated: r.tolerated, style: r.style, accepted: r.accepted, dsh: r.dsh, issues: r.issues })),
      model_cleanup: (p.models && p.models.model_cleanup) || null,
      link: linkReports.map(r => ({ group: r.group, status: r.status, cards_linked: r.cards, links: r.links, tool_calls: r.tool_calls ?? null, errors: r.errors })),
      terms: (p.terms && p.terms.report) || null, cards: cardsReport,
      deepseek_usage: (p.cleanup && p.cleanup.usage) || null, cleanup: (p.cleanup && p.cleanup.cleanup) || null,
      organize: or(p.organize, null), figures: or(p.figures, null), bridge: or(p.bridge, MODE === 'full' ? (A.bridge === false ? { status: 'skipped' } : { status: 'failed', error: 'Bridge stage produced no summary' }) : null), hubs: or(p.hubs, or(p.hubsFirst, null)), audit: or(p.audit, null),
      schedule,
      ...extra,
    }
  },
})
// ==== end module zt-digest-report ====
