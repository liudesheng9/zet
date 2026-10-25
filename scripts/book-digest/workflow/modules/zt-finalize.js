// ==== module zt-finalize v3 ====
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
defineModule({
  id: 'zt-finalize', version: '3', requires: ['zt-digest-lib', 'zt-term-link-lib'],
  summary: 'ZT digest: the finite finalization (audit plan, one scoped correction cycle, the changed-Card manifest, the round\'s Term-link pass, dependency-scoped publication refresh, final read-only audit).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const step = (name, more = '') => `${Z.DIGEST} finalize -Step ${name} -Out "${Z.OUT}"${more}`
    let plan = null
    let cycle = null
    let scope = null
    let recovered = false
    if (await Z.settledOnResume('finalize', ['audit', 'link-check'])) {
      // A resume after the scheduling audit: the branch either already published its final audit
      // (reuse it) or stopped before the correction cycle/final audit (continue from the recorded
      // manifest). Re-entering the cycle would either be refused as spent or redo finished writes.
      const retained = await Z.runScript('finalize changes', 'Finalize', step('changes'), ' (the retained changed-Card manifest)', { always: true })
      if (retained.json && retained.json.finalized === true) {
        log('Finalize: the branch already published its final audit; the resume reuses it.')
        return { status: 'done', reused: true }
      }
      if (retained.json && retained.json.term_link) {
        scope = retained.json
        recovered = true
        log(`Finalize: the resume continues the recorded correction cycle from ${scope.changes}; it does not re-enter the cycle.`)
      }
    }
    if (!recovered) {
      const planned = await Z.runScript('finalize plan', 'Finalize', step('plan'), ' (the audit plan: at most one correction cycle)')
      if (!planned.json) return { status: 'failed', error: (planned.run && planned.run.output) || 'the audit plan was not derived' }
      plan = planned.json
      const counts = plan.counts || {}
      log(`Finalize: audit ${plan.audit.unit} at graph revision ${plan.audit.graph_revision}; ${plan.faults.total} faults (${counts.fix || 0} correctable, ${counts.conflict || 0} conflicting, ${counts.blocked || 0} blocked, ${counts.advisory || 0} advisory); branch ${plan.branch}.`)
      if (plan.branch === 'fix') {
        const work = `${Z.OUT}\\finalize\\fix`
        const rules = (plan.fix_cycle.rules || []).join(',')
        const cards = plan.fix_cycle.card_list ? ` -Cards "${plan.fix_cycle.card_list}"` : ''
        cycle = await Z.runCards('final correction', work, cards, rules, { fixRounds: 1 })
        if (cycle.status !== 'done') {
          return { status: 'partial', plan, cycle, error: `the one correction cycle did not complete: ${cycle.error || 'no report'}` }
        }
        // The changed-Card manifest is recorded once, with the round's Term-link scope.
        const scoped = await Z.runScript('finalize scope', 'Finalize', step('scope', ` -FixWork "${work}"`), ' (the exact changed-Card manifest of the one cycle)')
        if (!scoped.json) return { status: 'partial', plan, cycle, error: (scoped.run && scoped.run.output) || 'the changed-Card manifest was not recorded' }
        scope = scoped.json
      }
    }
    const refreshed = []
    // The cycle's report artifact is named by the immutable manifest, so a recovered branch
    // refreshes from the same artifact a fresh run would.
    const cycleReport = scope && scope.cycle && scope.cycle.report ? String(scope.cycle.report) : ''
    for (const entry of (scope ? scope.refresh : []) || []) {
      if (!entry.applicable || entry.layer === 'audit') continue
      if (!cycleReport) {
        log(`Finalize: the ${entry.layer} refresh has no retained cycle report to read; its own stage owns that content.`)
        continue
      }
      const more = ` -Layer ${entry.layer} -Report "${cycleReport}"`
      const done = await Z.runScript(`finalize refresh ${entry.layer}`, 'Finalize', step('refresh', more))
      refreshed.push(done.json
        ? { layer: entry.layer, unit: done.json.unit, revision: done.json.revision, state: done.json.state, changed_cards: done.json.changed_cards }
        : { layer: entry.layer, error: (done.run && done.run.output) || 'the refresh did not complete' })
      if (!done.json) log(`Finalize: the ${entry.layer} refresh did not complete: ${(done.run && done.run.output) || 'no output'}.`)
    }
    const owned = ((scope && scope.refresh) || []).filter(e => e.stage_owned).map(e => e.layer)
    if (owned.length) log(`Finalize: ${owned.join(', ')} keep their own stage as the owner of that refresh; the changed-Card manifest is recorded for it.`)

    // S26 (#169): the round named by the immutable manifest relinks the Cards the correction
    // actually changed (or the whole book when it changed the Term mapping) before the final audit.
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
        relay: 'relay',
        stage: 'zt-finalize',
      })
      log(`Finalize: Term-link round ${round.round} (${round.scope}) is ${termLink.status}${termLink.unit ? ` (${termLink.unit})` : ''}${termLink.error || termLink.reason ? `: ${termLink.error || termLink.reason}` : ''}.`)
      if (termLink.status !== 'done') log('Finalize: the final audit still runs; the run reports the Term coverage debt.')
    } else if (round) {
      log(`Finalize: no Term-link round: ${round.reason || round.scope}.`)
    }

    const final = await Z.runScript('finalize final', 'Finalize', step('final'), ' (the final read-only audit at the new revision)')
    if (!final.json) return { status: 'failed', plan, cycle, scope, refreshed, term_link: termLink, error: (final.run && final.run.output) || 'the final audit was not published' }
    const report = final.json
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
