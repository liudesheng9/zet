// ==== module zt-finalize v4 ====
// The declared finite finalization branch (R299/R300, #86; S26, #169): the read-only audit plan, at
// most ONE correction cycle scoped to the plan's Cards, the exact changed-Card/touched-tree
// manifest, a Term-link round for the Cards that cycle actually changed (the whole book when the
// correction changed the Term mapping), the dependency-scoped refresh of the affected publications
// this branch re-derives, and the final read-only audit published at the new graph revision. The
// branch is a declared sequence, never a loop: the plan refuses a second cycle, and a stage-owned
// publication names its own stage instead of being republished from an artifact this branch did not
// re-derive. A residual hard fault leaves the result partial, with the fault named, and never
// republishes the audit that scheduled it.
//
// 3 (S26, #169): after `scope`, the round named in the immutable manifest relinks its Cards (the
// shared `zt-term-link-lib` pass, owned by `termlink/final@R`) before the final audit and the
// mathematics check, so a correction that dropped a Term link never becomes the book's final text
// (R0438/R0439/R0440). A resume that finds the correction cycle already recorded but the final
// audit not yet published continues from the retained manifest (`zt-digest finalize -Step
// changes`) instead of re-entering the one permitted cycle or skipping the relink.
// 4 (#176, docs/workflow-module-structure-proposal.md): the plan, the retained manifest, the correction cycle's
// steps, the scope, the refreshes and the final audit are book-steps.ps1 steps run by DeepSeek drive jobs, and the
// Term-link round is zt-term-link-lib's one drive job. No Claude agent runs a command.
defineModule({
  id: 'zt-finalize', version: '4', requires: ['zt-digest-lib', 'zt-term-link-lib'],
  summary: 'ZT digest: the finite finalization (audit plan, one scoped correction cycle, the changed-Card manifest, the round\'s Term-link pass, dependency-scoped publication refresh, final read-only audit).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    let plan = null
    let cycle = null
    let scope = null
    let recovered = false
    if (await Z.settledOnResume('finalize', ['audit', 'link-check'])) {
      // A resume after the scheduling audit: the branch either already published its final audit (reuse it) or
      // stopped before the correction cycle/final audit (continue from the recorded manifest). Re-entering the cycle
      // would either be refused as spent or redo finished writes.
      const retained = await Z.bookStep('finalize changes', 'finalize-changes')
      if (retained && retained.ok === true && retained.finalized === true) {
        log('Finalize: the branch already published its final audit; the resume reuses it.')
        return { status: 'done', reused: true }
      }
      if (retained && retained.ok === true && retained.term_link) {
        scope = retained
        recovered = true
        log(`Finalize: the resume continues the recorded correction cycle from ${scope.changes}; it does not re-enter the cycle.`)
      }
    }
    if (!recovered) {
      const planned = await Z.bookStep('finalize plan', 'finalize-plan')
      if (!planned || planned.ok !== true || !planned.audit) return { status: 'failed', error: (planned && planned.error) || 'the audit plan was not derived' }
      plan = planned
      const counts = plan.counts || {}
      log(`Finalize: audit ${plan.audit.unit} at graph revision ${plan.audit.graph_revision}; ${plan.faults.total} faults (${counts.fix || 0} correctable, ${counts.conflict || 0} conflicting, ${counts.blocked || 0} blocked, ${counts.advisory || 0} advisory); branch ${plan.branch}.`)
      if (plan.branch === 'fix') {
        const work = `${Z.OUT}\\finalize\\fix`
        const cards = plan.fix_cycle.card_list || ''
        const rules = (plan.fix_cycle.rules || []).join(',')
        cycle = await Z.cardCycle('final correction', work, { cards, rules, fixRounds: 1 })
        if (cycle.status !== 'done') {
          return { status: 'partial', plan, cycle, error: `the one correction cycle did not complete: ${cycle.error || 'no report'}` }
        }
        // The changed-Card manifest is recorded once, with the round's Term-link scope.
        const scoped = await Z.bookStep('finalize scope', 'finalize-scope', { FixWork: work })
        if (!scoped || scoped.ok !== true) return { status: 'partial', plan, cycle, error: (scoped && scoped.error) || 'the changed-Card manifest was not recorded' }
        scope = scoped
      }
    }
    const refreshed = []
    // The cycle's report artifact is named by the immutable manifest, so a recovered branch refreshes from the same
    // artifact a fresh run would.
    const cycleReport = scope && scope.cycle && scope.cycle.report ? String(scope.cycle.report) : ''
    for (const entry of (scope ? scope.refresh : []) || []) {
      if (!entry.applicable || entry.layer === 'audit') continue
      if (!cycleReport) {
        log(`Finalize: the ${entry.layer} refresh has no retained cycle report to read; its own stage owns that content.`)
        continue
      }
      const done = await Z.bookStep(`finalize refresh ${entry.layer}`, 'finalize-refresh', { Layer: entry.layer, Report: cycleReport })
      refreshed.push(done && done.ok === true
        ? { layer: entry.layer, unit: done.unit, revision: done.revision, state: done.state, changed_cards: done.changed_cards }
        : { layer: entry.layer, error: (done && done.error) || 'the refresh did not complete' })
      if (!done || done.ok !== true) log(`Finalize: the ${entry.layer} refresh did not complete: ${(done && done.error) || 'no output'}.`)
    }
    const owned = ((scope && scope.refresh) || []).filter(e => e.stage_owned).map(e => e.layer)
    if (owned.length) log(`Finalize: ${owned.join(', ')} keep their own stage as the owner of that refresh; the changed-Card manifest is recorded for it.`)

    // S26 (#169): the round named by the immutable manifest relinks the Cards the correction actually changed (or the
    // whole book when it changed the Term mapping) before the final audit.
    let termLink = null
    const round = scope && scope.term_link ? scope.term_link : null
    if (round && round.applicable === true && round.scope !== 'none') {
      const occurrences = (Z.A && Z.A.termLinkOccurrences) || 'first'
      termLink = await ztTermLinkPass(ctx, {
        out: Z.OUT,
        key: Z.KEY,
        args: Z.A,
        mode: p.mode,
        occurrences,
        cards: round.scope === 'cards' ? (round.cards || []) : [],
        round: round.round,
        tool: 'zt-digest',
        stage: 'zt-finalize',
      })
      log(`Finalize: Term-link round ${round.round} (${round.scope}) is ${termLink.status}${termLink.unit ? ` (${termLink.unit})` : ''}${termLink.error || termLink.reason ? `: ${termLink.error || termLink.reason}` : ''}.`)
      if (termLink.status !== 'done') log('Finalize: the final audit still runs; the run reports the Term coverage debt.')
    } else if (round) {
      log(`Finalize: no Term-link round: ${round.reason || round.scope}.`)
    }

    const report = await Z.bookStep('finalize final', 'finalize-final')
    if (!report || report.ok !== true) return { status: 'failed', plan, cycle, scope, refreshed, term_link: termLink, error: (report && report.error) || 'the final audit was not published' }
    log(`Finalize: ${report.status} at graph revision ${report.graph_revision}; final audit ${report.audit.final.unit} (revision ${report.audit.final.revision}); ${report.faults.residual} residual hard/source faults, ${report.infrastructure.length} infrastructure faults; terms ${(report.terms && report.terms.status) || 'unknown'}.`)
    return {
      status: report.status === 'done' ? 'done' : 'partial',
      branch: plan ? plan.branch : report.branch,
      plan,
      cycle,
      scope,
      recovered,
      refreshed,
      term_link: termLink,
      final: report,
    }
  },
})
// ==== end module zt-finalize ====
