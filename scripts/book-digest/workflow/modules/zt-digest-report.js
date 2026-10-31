// ==== module zt-digest-report v23 ====
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
// 19 (S26, #169) adds term_link.final: the finite finalization's own round (generation, scope, unit, status and
// counts), read from the final report's ledger facts, and the final audit's Term quality under `terms`, so a run
// that corrected Cards reports whether their Term links were restored and audit-ran never reads as all-terms-passed.
// 20 (#180): the stage outputs it assembles carry counts only (a step line is at most 400 characters and a module
// reads no unit list), so `digest`, `link`, `link_blocked` and the Term summaries report totals and how many groups
// were verified, skipped or blocked; the per-unit facts stay in the ledger's run report (`run_report`).
// 21 (#188): every mode's result carries `coverage` (units, covered, skipped, uncovered, pct), measured by the one
// read-only `coverage` step (zt-digest coverage -Step) after the run's writers settled, so an operator sees how
// much of the source the Literature notes cover and a later mode cards run can fill the gaps it names.
// 22 (#188): a variant's own mode (`events`, `findings`, `math`) also carries that coverage, so a measured count is
// never thrown away; a successful measurement of zero units is `coverage` 0/0/0/0/100, not "not measured" (the
// step line omits zero fields, so its `ok:true` is the measurement's own answer).
// 23 (#221): the Cited list and the Cited Cards. Every whole report and mode cited-cards carry `cited` (the list:
// rows, unparsed, refused, its Card) and `cited_cards` (jobs, publication, reason), and the schedule names both
// statuses, so a book whose reference list did not publish in full reports that debt.
defineModule({
  id: 'zt-digest-report', version: '23', requires: ['zt-digest-lib'],
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
    citedList: { type: 'any', description: 'the Cited list stage output: rows, unparsed, refused and its Card (#221)' },
    citedCards: { type: 'any', description: 'the Cited Card stage output: its jobs and publication (#221)' },
    models: { type: 'any', description: 'the output of the model layer (the models stage of a book type)' },
    terms: { type: 'any', description: 'the term table stage output' },
    prepare: { type: 'any', description: 'the prepare stage output: the backfill validation of modes term-cards and term-link (S25, #168)' },
    cardsBook: { type: 'any', description: 'the whole-book card cycle (mode cards)' },
    explain: { type: 'any', description: 'the explain stage output (mode explain)' },
    link: { type: 'any', description: 'the link pass output' },
    linkCheck: { type: 'any', description: 'the link check card cycle' },
    linkBlocked: { type: 'number', default: 0, description: 'how many groups the Link pass left blocked because their digest unit is not done (#92)' },
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
    // #180: a layer answers counts (verified and failed units, the gate), never per-unit rows.
    const digestSummary = p.digest
      ? { groups: p.digest.groups ?? null, claimed: p.digest.claimed ?? 0, verified: p.digest.verified ?? 0, failed: p.digest.failed ?? 0 }
      : null
    const layerSummary = (value) => value && (value.verified !== undefined || value.failed !== undefined || value.gate)
      ? { status: value.status, verified: value.verified ?? 0, failed: value.failed ?? 0, open: (value.gate && value.gate.counts && (value.gate.counts.open || 0)) || 0, complete: !!(value.gate && value.gate.complete) }
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
        jobs: value.jobs ? { total: value.jobs.total || 0, done: value.jobs.done || 0, failed: Number(value.jobs.failed || 0) } : null,
        readiness: value.readiness || null,
        publication: value.publication || null,
        completion: value.completion || null,
        counts: value.counts || null,
        no_op: value.no_op === true,
        blocked: Number(value.not_ready || 0),
        reason: value.reason || value.error || null,
      }
    // #221: the Cited stages' own result, never a guess: skipped when the stage did not run.
    const citedSummary = (value) => !value
      ? { status: 'skipped' }
      : {
        status: value.status || 'unknown',
        rows: value.rows ?? (value.publication && value.publication.rows) ?? null,
        list: value.list || null,
        unparsed: Number(value.unparsed || 0),
        refused: Number(value.refused || 0),
        jobs: value.jobs ? { total: value.jobs.total || 0, done: value.jobs.done || 0, failed: Number(value.jobs.failed || 0) } : null,
        publication: value.publication || null,
        blocked: Number(value.not_ready || 0),
        reason: value.reason || value.error || null,
      }
    const schedule = {
      settings: { maxAttempts: Z.MAX_ATTEMPTS, dshRunCap: Z.DSH_RUN_CAP },
      digest: digestSummary ? { verified: digestSummary.verified, failed: digestSummary.failed } : null,
      cards: layerSummary(p.cards),
      models: layerSummary(p.models),
      explain: layerSummary(p.explain),
      events: layerSummary((extra && extra.events) || null),
      findings: layerSummary((extra && extra.findings) || null),
      link: p.link ? { scheduled: p.link.scheduled ?? 0, skipped: p.link.skipped ?? 0, verified: p.link.verified ?? 0, failed: p.link.failed ?? 0, provider_runs: p.link.provider_runs ?? 0 } : null,
      term_cards: termSummary(p.termCards, 'the Term-card branch').status,
      term_link: termSummary(p.termLink, 'the Term-link pass').status,
      term_link_final: (p.finalize && p.finalize.final && p.finalize.final.term_link && p.finalize.final.term_link.status) || null,
      cited: citedSummary(p.citedList).status,
      cited_cards: citedSummary(p.citedCards).status,
    }
    // S26 (#169): the finite finalization's own Term-link round and the final audit's Term quality,
    // both read from the final report's ledger facts, never inferred from the whole-book pass.
    const termLinkSummary = (value, finalize) => {
      const summary = termSummary(value, 'the Term-link pass')
      const round = finalize && finalize.final && finalize.final.term_link
      if (round) {
        summary.final = {
          applicable: round.applicable !== false,
          round: round.round ?? null,
          scope: round.scope || null,
          status: round.status || 'unknown',
          unit: round.unit || null,
          state: round.state || null,
          counts: round.counts || null,
          no_op: round.no_op === true,
          reason: round.reason || null,
        }
      }
      if (finalize && finalize.final && finalize.final.terms) summary.audit_terms = finalize.final.terms
      return summary
    }
    // #188: the run's Literature-note coverage, one read-only measurement step. Its one drive job settles
    // itself; a failed measurement is reported as no coverage with its reason, never as a failed stage: the
    // audit lists the gaps as findings, and a later `mode cards` run fills them. The step line leaves zero
    // fields out (#180), so its own `ok:true` is the measurement's answer: 0 units is 0/0/0/0/100.
    let coverage = null, coverageError = null
    try {
      const measured = await Z.bookStep('report: coverage', 'coverage')
      if (measured && measured.ok === true) {
        const count = (v) => Number(v || 0)
        coverage = {
          units: count(measured.units), covered: count(measured.covered), skipped: count(measured.skipped),
          uncovered: count(measured.uncovered), pct: count(measured.pct),
        }
        log(`Coverage: ${coverage.covered} of ${coverage.units} unit(s) covered, ${coverage.skipped} skipped, ${coverage.uncovered} uncovered (${coverage.pct}%).`)
      } else {
        coverageError = String((measured && measured.error) || 'the coverage step printed no line')
        log(`Coverage: not measured: ${coverageError}`)
      }
    } catch (error) {
      coverageError = String((error && error.message) || error)
      log(`Coverage: not measured: ${coverageError}`)
    }
    const coverageField = coverage ? { coverage } : { coverage: null, coverage_error: coverageError }
    if (MODE === 'bridge') return { mode: MODE, citation_key: KEY, bridge: or(p.bridge, { status: 'failed', error: 'Bridge stage produced no summary' }), ...coverageField }
    if (MODE === 'cards') return { mode: MODE, dsh_model: dshModel, citation_key: KEY, cards: or(p.cardsBook, null), schedule, run_report: runReport, ...coverageField }
    // One whole-book card cycle (R296): book is that cycle's report, link the link check's, groups the per-group checks.
    const cardsReport = { book: or(p.cardsBook, null), link: or(p.linkCheck, null), groups: layerSummary(p.cards) }
    if (MODE === 'explain') return { mode: MODE, dsh_model: dshModel, citation_key: KEY, explain: or(p.explain, null), schedule, run_report: runReport, ...coverageField }
    if (MODE === 'figures') return { mode: MODE, citation_key: KEY, figures: or(p.figures, null), run_report: runReport, ...coverageField }
    if (MODE === 'audit') return { mode: MODE, citation_key: KEY || null, audit: or(p.audit, null), run_report: runReport, ...coverageField }
    if (MODE === 'models' && !A.organize) return { mode: MODE, dsh_model: dshModel, book: A.bookTitle, citation_key: KEY, models: (p.models && p.models.models) || [], model_cleanup: (p.models && p.models.model_cleanup) || null, schedule, run_report: runReport, ...coverageField }
    // S25 (#168): one selected book's Term backfill. The result carries the pre-dispatch validation it ran
    // (Workspace, list, occupancy, source/sections fingerprints, Lock and conflicts), each Term stage's own
    // truthful result and the read-only audit; it is never a fresh digest's report.
    if (MODE === 'term-cards' || MODE === 'term-link') {
      const backfill = (p.prepare && p.prepare.backfill) || null
      const tc = termSummary(p.termCards, 'the Term-card branch')
      const tl = termLinkSummary(p.termLink, p.finalize)
      const conflicts = Number((backfill && backfill.conflicts) || 0)
      ctx.log(`Backfill ${MODE}: ${tc.status} term-cards, ${tl.status} term-link${backfill ? `; ${Number(backfill.rows || 0)} Term row(s), ${Number(backfill.heads || 0)} published head(s)` : ''}${conflicts ? `, ${conflicts} conflict(s)` : ''}.`)
      return {
        mode: MODE,
        citation_key: KEY,
        backfill,
        term_cards: tc,
        term_link: tl,
        audit: or(p.audit, null),
        schedule,
        run_report: runReport,
        ...coverageField,
      }
    }
    // #221: one selected book's Cited backfill: the list, its Cards, the Term-link pass after them and the audit.
    if (MODE === 'cited-cards') {
      const cl = citedSummary(p.citedList)
      const cc = citedSummary(p.citedCards)
      ctx.log(`Backfill cited-cards: ${cl.status} list (${cl.rows ?? 0} row(s)), ${cc.status} Cited Cards.`)
      return { mode: MODE, citation_key: KEY, cited: cl, cited_cards: cc, term_link: termLinkSummary(p.termLink, p.finalize), audit: or(p.audit, null), schedule, run_report: runReport, ...coverageField }
    }
    if (!['full', 'link', 'models', 'resume'].includes(MODE)) return { mode: MODE, dsh_model: dshModel, citation_key: KEY, ...extra, schedule, ...coverageField }
    const modelReports = (p.models && p.models.models) || []
    // #180: the Link pass answers the run's own totals, never per-group rows.
    const linkGaps = p.link
      ? { scheduled: p.link.scheduled ?? 0, skipped: p.link.skipped ?? 0, gaps: p.link.gap ?? 0, skipped_cards: p.link.skipped_cards ?? 0, linked: p.link.linked ?? 0, linked_from: p.link.linked_from ?? 0, already_linked: p.link.already_linked ?? 0, no_link: p.link.no_link ?? 0, changed: p.link.changed ?? 0, provider_runs: p.link.provider_runs ?? 0, verified: p.link.verified ?? 0, failed: p.link.failed ?? 0, blocked: p.link.blocked ?? 0, quality: p.link.quality || null }
      : null
    const groupCount = p.groups || 0
    const next = { workflow: 'zt-idea-grow', script: '.claude/workflows/zt-idea-grow.js', args: { book: KEY, out: '<a fresh work folder>' }, why: 'this digest built only the Literature tree; zt-idea-grow grows its Topic Cards, hubs and register' }
    ctx.log(`Next: run zt-idea-grow with book "${KEY}" and a fresh out folder: the digest wrote no Topic Card, Hub Card or register entry.`)
    if (p.termCards || p.termLink) {
      const tc = termSummary(p.termCards, 'the Term-card branch')
      const tl = termLinkSummary(p.termLink, p.finalize)
      ctx.log(`Terms: term-cards ${tc.status}${tc.unit ? ` (${tc.unit})` : ''}${tc.reason ? `: ${tc.reason}` : ''}; term-link ${tl.status}${tl.unit ? ` (${tl.unit})` : ''}${tl.reason ? `: ${tl.reason}` : ''}${tl.final ? `; final round ${tl.final.round} ${tl.final.status} (${tl.final.scope})` : ''}.`)
    }
    return {
      mode: MODE, dsh_model: dshModel, book: A.bookTitle, citation_key: KEY, groups: groupCount, managers_per_pass: Z.toBlocks(Z.groupsOf(groupCount)).length,
      digest: digestSummary,
      models: modelReports.map(r => ({ model: r.model, level: r.level, status: r.status, parts: Array.isArray(r.parts) ? r.parts.length : (Number(r.parts) || 0), relations: r.relations, kinds: r.kinds, charts: r.charts, derivations: r.derivations, split: r.split, moved: r.moved, shared: r.shared, tolerated: r.tolerated, style: r.style, accepted: r.accepted, dsh: r.dsh, issues: r.issues })),
      model_cleanup: (p.models && p.models.model_cleanup) || null,
      link: linkGaps ? { scheduled: linkGaps.scheduled, skipped: linkGaps.skipped, verified: linkGaps.verified, failed: linkGaps.failed, provider_runs: linkGaps.provider_runs } : null,
      link_gaps: linkGaps,
      link_blocked: Number(p.linkBlocked || 0),
      terms: (p.terms && p.terms.report) || null, cards: cardsReport,
      term_cards: termSummary(p.termCards, 'the Term-card branch'),
      term_link: termLinkSummary(p.termLink, p.finalize),
      cited: citedSummary(p.citedList),
      cited_cards: citedSummary(p.citedCards),
      deepseek_usage: runUsage, cleanup: (p.cleanup && p.cleanup.cleanup) || null, run_report: runReport,
      organize: or(p.organize, null), figures: or(p.figures, null), bridge: or(p.bridge, MODE === 'full' ? (A.bridge === false ? { status: 'skipped' } : { status: 'failed', error: 'Bridge stage produced no summary' }) : null), audit: or(p.audit, null), finalize: or(p.finalize, null),
      schedule,
      next,
      ...coverageField,
      ...extra,
    }
  },
})
// ==== end module zt-digest-report ====





