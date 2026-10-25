// ==== module zt-digest-report v18 ====
// The run's result, in the shape zt-book-digest.js returned for each mode: a stage-only mode returns that
// stage's report; mode full (and link, and models with organize) returns the whole digest report. A
// variant's own mode (math) returns {mode, dsh_model, citation_key} with the variant's extra reports; in the
// other modes the extra reports join the whole report (e.g. math: {...}). 4.0.0 adds the effective settings
// (maxAttempts, dshRunCap), the Cards layer's gate, and a schedule summary of what every group chain and layer
// driver settled: verified, failed, retrying and open work, never a claim of work that no gate confirmed.
// 8 adds run_report: the verified ledger report (completeness, gaps, current placeholders, timings, exact
// per-run usage and the supported resume invocation), so a run's result carries the report's own facts
// instead of an agent's summary (#85, R331-R346). 9 adds finalize: the finite finalization's plan, one
// correction cycle, changed-Card manifest, refreshed publications and final audit (#86, R299/R300). 10 adds
// link_blocked: the groups the Link pass did not work because their digest unit is not done (#92). 11 adds
// link_gaps: the scheduled, skipped and gap counts and the L1/L2 quality of the gap-only Link pass, with each
// group's own outcome and actual provider runs (#110). 12 adds concepts and review: the frozen concept review
// of the run's own term table and the scoped restatement resolution, with their report ids, completeness and
// counts, so the integrated digest path reports what it reviewed and what it left pending (S23/#115).
// 15 (docs/literature-only-digest-design.md): a digest builds only the Literature tree, so the concept review,
// the restatement review of new Topic Cards, the hubs, the register and mode hubs leave the report; zt-idea-grow
// reports them. 16 adds next: the digest built the Literature tree only, so the whole report (and the log) names
// the follow-up run that grows its Topic Cards, hubs and register: zt-idea-grow with this book and a fresh folder.
// 17 (S24, #167) adds term_cards and term_link: each stage's truthful complete/degraded/skipped status with its
// ledger unit, plan, readiness/publication or apply counts and the reason it stopped, so a failed Term-card job or
// a blocked Term-link pass is visible coverage debt in the run's own result (R0428).
// 18 (S25, #168) adds the backfill modes term-cards and term-link: the pre-dispatch validation (Workspace, list,
// occupancy, source/sections fingerprints, Lock and conflicts) plus the Term stages' own result and the read-only
// audit, so one selected book's Term backfill reports its coverage and debt without a fresh digest.
defineModule({
  id: 'zt-digest-report', version: '18', requires: ['zt-digest-lib'],
  summary: 'ZT digest: assemble the run\'s result from the stage outputs, per mode, with the settings, the schedule summary and the verified ledger run report.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'any', description: 'the DeepSeek model from the source stage' },
    groups: { type: 'any', description: 'the group count' },
    digest: { type: 'any', description: 'the digest stage output (its per-group chains)' },
    cards: { type: 'any', description: 'the Cards layer output (the per-group offline checks)' },
    termCards: { type: 'any', description: 'the Term-card branch output: its units, waves, readiness and publication (S24, #167)' },
    termLink: { type: 'any', description: 'the deterministic Term-link pass output: its unit, plan, counts and no_op (S24, #167)' },
    models: { type: 'any', description: 'the output of the model layer (the models stage of a book type)' },
    terms: { type: 'any', description: 'the term table stage output' },
    prepare: { type: 'any', description: 'the prepare stage output: the backfill validation of modes term-cards and term-link (S25, #168)' },
    cardsBook: { type: 'any', description: 'the whole-book card cycle (mode cards)' },
    explain: { type: 'any', description: 'the explain stage output (mode explain)' },
    link: { type: 'any', description: 'the link pass output' },
    linkCheck: { type: 'any', description: 'the link check card cycle' },
    linkBlocked: { type: 'array', default: [], description: 'the groups the Link pass left blocked because their digest unit is not done (#92)' },
    cleanup: { type: 'any', description: 'the cleanup stage output, with the verified run report' },
    organize: { type: 'any', description: 'the Organize stage output' },
    figures: { type: 'any', description: 'the figures stage output' },
    bridge: { type: 'any', description: 'Bridge report or explicit apply summary' },
    audit: { type: 'any', description: 'the audit stage output' },
    finalize: { type: 'any', description: 'the finite finalization branch output (#86)' },
    extra: { type: 'object', default: {}, description: 'a variant\'s own reports, added to the result (e.g. {"math": {...}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const A = Z.A, MODE = p.mode, KEY = Z.KEY
    const dshModel = p.model || Z.DSH_MODEL
    const extra = p.extra || {}
    const or = (v, d) => (v === undefined ? d : v)
    // The verified ledger report every mode carries: the run's own completeness, gaps, placement, timings,
    // exact per-run usage and one supported resume. It never substitutes for a stage's own checked output.
    const runReport = (p.cleanup && p.cleanup.report) || null
    const runUsage = (p.cleanup && p.cleanup.usage) || (runReport && runReport.usage) || null
    // The group stage reports one entry per digest unit (blocks of up to eight per manager); an older
    // per-item stage output (items) is still read the same way.
    const digestReports = Array.isArray(p.digest && p.digest.items)
      ? p.digest.items.map(i => ({ group: (i.item && i.item.n) ?? null, ...(i.steps && i.steps.group ? i.steps.group : {}), status: i.ok ? ((i.steps && i.steps.group && i.steps.group.status) || 'done') : 'failed', errors: i.error || null }))
      : (p.digest && p.digest.reports) || []
    const layerSummary = (value) => value && value.dispositions
      ? { status: value.status, verified: value.dispositions.filter(d => d.status === 'done').length, failed: value.dispositions.filter(d => d.status === 'failed').length, retrying: value.dispositions.filter(d => d.status === 'retry').length, open: (value.gate && value.gate.counts && (value.gate.counts.open || 0)) || 0, complete: !!(value.gate && value.gate.complete) }
      : null
    // S24 (#167): the two Term stages' own truthful result. A stage that did not run is `skipped` with
    // its reason, never an absent or silently successful entry.
    const termSummary = (value, what) => !value
      ? { status: 'skipped', reason: `${what} did not run` }
      : {
        status: value.status || 'unknown',
        unit: value.unit || value.book_unit || null,
        pass: value.pass || null,
        round: value.round ?? null,
        plan: value.plan || (value.plan_revision !== undefined ? { revision: value.plan_revision, hash: value.plan_hash } : null),
        jobs: value.jobs ? { total: value.jobs.total || 0, done: value.jobs.done || 0, failed: (value.jobs.failed || []).map(f => f.id) } : null,
        readiness: value.readiness || null,
        publication: value.publication || null,
        completion: value.completion || null,
        counts: value.counts || null,
        no_op: value.no_op === true,
        blocked: (value.unresolved || []).slice(0, 32),
        reason: value.reason || value.error || null,
      }
    const schedule = {
      settings: { maxAttempts: Z.MAX_ATTEMPTS, dshRunCap: Z.DSH_RUN_CAP },
      digest: p.digest && (Array.isArray(p.digest.items) || Array.isArray(p.digest.reports)) ? { verified: digestReports.filter(r => r.status === 'done').length, failed: digestReports.filter(r => r.status !== 'done').length } : null,
      cards: layerSummary(p.cards),
      models: layerSummary(p.models),
      explain: layerSummary(p.explain),
      events: layerSummary((extra && extra.events) || null),
      findings: layerSummary((extra && extra.findings) || null),
      link: p.link ? { scheduled: p.link.scheduled ?? 0, skipped: p.link.skipped ?? 0, verified: p.link.verified ?? 0, failed: p.link.failed ?? 0, provider_runs: p.link.provider_runs ?? 0 } : null,
      term_cards: termSummary(p.termCards, 'the Term-card branch').status,
      term_link: termSummary(p.termLink, 'the Term-link pass').status,
    }
    if (MODE === 'bridge') return { mode: MODE, citation_key: KEY, bridge: or(p.bridge, { status: 'failed', error: 'Bridge stage produced no summary' }) }
    if (MODE === 'cards') return { mode: MODE, dsh_model: dshModel, citation_key: KEY, cards: or(p.cardsBook, null), schedule, run_report: runReport }
    // One whole-book card cycle (R296): book is that cycle's report, link the link check's, groups the per-group checks.
    const cardsReport = { book: or(p.cardsBook, null), link: or(p.linkCheck, null), groups: layerSummary(p.cards) }
    if (MODE === 'explain') return { mode: MODE, dsh_model: dshModel, citation_key: KEY, explain: or(p.explain, null), schedule, run_report: runReport }
    if (MODE === 'figures') return { mode: MODE, citation_key: KEY, figures: or(p.figures, null), run_report: runReport }
    if (MODE === 'audit') return { mode: MODE, citation_key: KEY || null, audit: or(p.audit, null), run_report: runReport }
    if (MODE === 'models' && !A.organize) return { mode: MODE, dsh_model: dshModel, book: A.bookTitle, citation_key: KEY, models: (p.models && p.models.models) || [], model_cleanup: (p.models && p.models.model_cleanup) || null, schedule, run_report: runReport }
    // S25 (#168): one selected book's Term backfill. The result carries the pre-dispatch validation it ran
    // (Workspace, list, occupancy, source/sections fingerprints, Lock and conflicts), each Term stage's own
    // truthful result and the read-only audit; it is never a fresh digest's report.
    if (MODE === 'term-cards' || MODE === 'term-link') {
      const backfill = (p.prepare && p.prepare.backfill) || null
      const tc = termSummary(p.termCards, 'the Term-card branch')
      const tl = termSummary(p.termLink, 'the Term-link pass')
      const conflicts = (backfill && backfill.conflicts) || []
      ctx.log(`Backfill ${MODE}: ${tc.status} term-cards, ${tl.status} term-link${backfill ? `; ${backfill.list ? backfill.list.rows : 0} Term row(s), ${backfill.tree.heads} published head(s)` : ''}${conflicts.length ? `, ${conflicts.length} conflict(s)` : ''}.`)
      return {
        mode: MODE,
        citation_key: KEY,
        backfill,
        term_cards: tc,
        term_link: tl,
        audit: or(p.audit, null),
        schedule,
        run_report: runReport,
      }
    }
    if (!['full', 'link', 'models', 'resume'].includes(MODE)) return { mode: MODE, dsh_model: dshModel, citation_key: KEY, ...extra, schedule }
    const modelReports = (p.models && p.models.models) || []
    const linkReports = (p.link && p.link.groups) || []
    const linkGaps = p.link
      ? { scheduled: p.link.scheduled ?? 0, skipped: p.link.skipped ?? 0, gaps: p.link.gap ?? 0, skipped_cards: p.link.skipped_cards ?? 0, linked: p.link.linked ?? 0, linked_from: p.link.linked_from ?? 0, already_linked: p.link.already_linked ?? 0, no_link: p.link.no_link ?? 0, provider_runs: p.link.provider_runs ?? 0, quality: p.link.quality || null }
      : null
    const groupCount = p.groups || 0
    const next = { workflow: 'zt-idea-grow', script: '.claude/workflows/zt-idea-grow.js', args: { book: KEY, out: '<a fresh work folder>' }, why: 'this digest built only the Literature tree; zt-idea-grow grows its Topic Cards, hubs and register' }
    ctx.log(`Next: run zt-idea-grow with book "${KEY}" and a fresh out folder: the digest wrote no Topic Card, Hub Card or register entry.`)
    if (p.termCards || p.termLink) {
      const tc = termSummary(p.termCards, 'the Term-card branch')
      const tl = termSummary(p.termLink, 'the Term-link pass')
      ctx.log(`Terms: term-cards ${tc.status}${tc.unit ? ` (${tc.unit})` : ''}${tc.reason ? `: ${tc.reason}` : ''}; term-link ${tl.status}${tl.unit ? ` (${tl.unit})` : ''}${tl.reason ? `: ${tl.reason}` : ''}.`)
    }
    return {
      mode: MODE, dsh_model: dshModel, book: A.bookTitle, citation_key: KEY, groups: groupCount, managers_per_pass: Z.toBlocks(Z.groupsOf(groupCount)).length,
      digest: digestReports.map(r => ({ group: r.group, status: r.status, unit: r.unit || null, disposition: r.disposition || null, owned: r.owned ?? null, errors: r.errors || null })),
      models: modelReports.map(r => ({ model: r.model, level: r.level, status: r.status, parts: Array.isArray(r.parts) ? r.parts.length : (Number(r.parts) || 0), relations: r.relations, kinds: r.kinds, charts: r.charts, derivations: r.derivations, split: r.split, moved: r.moved, shared: r.shared, tolerated: r.tolerated, style: r.style, accepted: r.accepted, dsh: r.dsh, issues: r.issues })),
      model_cleanup: (p.models && p.models.model_cleanup) || null,
      link: linkReports.map(r => ({ group: r.group, status: r.status, unit: r.unit || null, gaps: r.gaps ?? null, skipped: r.skipped ?? null, scheduled: r.scheduled ?? null, linked: r.linked ?? null, linked_from: r.linked_from ?? null, already_linked: r.already_linked ?? null, no_link: r.no_link ?? null, provider_runs: r.provider_runs ?? null, changed: r.changed || [], quality: r.quality || null, errors: r.errors || null })),
      link_gaps: linkGaps,
      link_blocked: (p.linkBlocked || []).map(b => ({ group: b.n, unit: b.unit, state: b.state, reason: b.reason })),
      terms: (p.terms && p.terms.report) || null, cards: cardsReport,
      term_cards: termSummary(p.termCards, 'the Term-card branch'),
      term_link: termSummary(p.termLink, 'the Term-link pass'),
      deepseek_usage: runUsage, cleanup: (p.cleanup && p.cleanup.cleanup) || null, run_report: runReport,
      organize: or(p.organize, null), figures: or(p.figures, null), bridge: or(p.bridge, MODE === 'full' ? (A.bridge === false ? { status: 'skipped' } : { status: 'failed', error: 'Bridge stage produced no summary' }) : null), audit: or(p.audit, null), finalize: or(p.finalize, null),
      schedule,
      next,
      ...extra,
    }
  },
})
// ==== end module zt-digest-report ====


