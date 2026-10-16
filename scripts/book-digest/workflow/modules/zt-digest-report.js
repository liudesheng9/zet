// ==== module zt-digest-report v2 ====
// The run's result, in the shape zt-book-digest.js returned for each mode: a stage-only mode returns that
// stage's report; mode full (and link, and models or hubs with organize) returns the whole digest report. A
// variant's own mode (math) returns {mode, dsh_model, citation_key} with the variant's extra reports; in the
// other modes the extra reports join the whole report (e.g. math: {...}).
defineModule({
  id: 'zt-digest-report', version: '2', requires: ['zt-digest-lib'],
  summary: 'ZT digest: assemble the run\'s result from the stage outputs, per mode.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'any', description: 'the DeepSeek model from the source stage' },
    groups: { type: 'any', description: 'the group count' },
    digest: { type: 'any', description: 'the digest stage output' },
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
    if (MODE === 'cards') return { mode: MODE, dsh_model: dshModel, citation_key: KEY, cards: or(p.cardsBook, null) }
    if (MODE === 'explain') return { mode: MODE, dsh_model: dshModel, citation_key: KEY, explain: or(p.explain, null) }
    if (MODE === 'figures') return { mode: MODE, citation_key: KEY, figures: or(p.figures, null) }
    if (MODE === 'audit') return { mode: MODE, citation_key: KEY || null, audit: or(p.audit, null) }
    if (MODE === 'models' && !A.organize) return { mode: MODE, dsh_model: dshModel, book: A.bookTitle, citation_key: KEY, models: (p.models && p.models.models) || [], model_cleanup: (p.models && p.models.model_cleanup) || null }
    if (MODE === 'hubs' && !A.organize) return { mode: MODE, dsh_model: dshModel, hubs: or(p.hubsFirst, null) }
    if (!['full', 'link', 'models', 'hubs'].includes(MODE)) return { mode: MODE, dsh_model: dshModel, citation_key: KEY, ...extra }
    const digestReports = (p.digest && p.digest.reports) || []
    const modelReports = (p.models && p.models.models) || []
    const linkReports = (p.link && p.link.reports) || []
    const groupCount = p.groups || 0
    return {
      mode: MODE, dsh_model: dshModel, book: A.bookTitle, citation_key: KEY, groups: groupCount, managers_per_pass: Z.toBlocks(Z.groupsOf(groupCount)).length,
      digest: digestReports.map(r => ({ group: r.group, status: r.status, cards: r.cards, tool_calls: r.tool_calls ?? null })),
      models: modelReports.map(r => ({ model: r.model, level: r.level, status: r.status, parts: (r.parts || []).length, relations: r.relations, kinds: r.kinds, charts: r.charts, derivations: r.derivations, split: r.split, moved: r.moved, shared: r.shared, tolerated: r.tolerated, style: r.style, accepted: r.accepted, dsh: r.dsh, issues: r.issues })),
      model_cleanup: (p.models && p.models.model_cleanup) || null,
      link: linkReports.map(r => ({ group: r.group, status: r.status, cards_linked: r.cards, links: r.links, tool_calls: r.tool_calls ?? null, errors: r.errors })),
      terms: (p.terms && p.terms.report) || null, cards: { blocks: (p.digest && p.digest.cards) || [], link: or(p.linkCheck, null) },
      deepseek_usage: (p.cleanup && p.cleanup.usage) || null, cleanup: (p.cleanup && p.cleanup.cleanup) || null,
      organize: or(p.organize, null), figures: or(p.figures, null), hubs: or(p.hubs, or(p.hubsFirst, null)), audit: or(p.audit, null),
      ...extra,
    }
  },
})
// ==== end module zt-digest-report ====
