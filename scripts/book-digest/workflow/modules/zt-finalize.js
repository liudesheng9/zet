// ==== module zt-finalize v1 ====
// The declared finite finalization branch (R299/R300, #86): the read-only audit plan, at most ONE
// correction cycle scoped to the plan's Cards, the exact changed-Card/touched-tree manifest, the
// dependency-scoped refresh of the affected publications this branch re-derives, and the final
// read-only audit published at the new graph revision. The branch is a declared sequence, never a
// loop: the plan refuses a second cycle, and a stage-owned publication names its own stage instead
// of being republished from an artifact this branch did not re-derive. A residual hard fault leaves
// the result partial, with the fault named, and never republishes the audit that scheduled it.
defineModule({
  id: 'zt-finalize', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the finite finalization (audit plan, one scoped correction cycle, the changed-Card manifest, dependency-scoped publication refresh, final read-only audit).',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const step = (name, more = '') => `${Z.DIGEST} finalize -Step ${name} -Out "${Z.OUT}"${more}`
    const plan = await Z.runScript('finalize plan', 'Finalize', step('plan'), ' (the audit plan: at most one correction cycle)')
    if (!plan.json) return { status: 'failed', error: (plan.run && plan.run.output) || 'the audit plan was not derived' }
    const facts = plan.json
    const counts = facts.counts || {}
    log(`Finalize: audit ${facts.audit.unit} at graph revision ${facts.audit.graph_revision}; ${facts.faults.total} faults (${counts.fix || 0} correctable, ${counts.conflict || 0} conflicting, ${counts.blocked || 0} blocked, ${counts.advisory || 0} advisory); branch ${facts.branch}.`)
    let cycle = null
    let scope = null
    const refreshed = []
    if (facts.branch === 'fix') {
      const work = `${Z.OUT}\\finalize\\fix`
      const rules = (facts.fix_cycle.rules || []).join(',')
      const cards = facts.fix_cycle.card_list ? ` -Cards "${facts.fix_cycle.card_list}"` : ''
      cycle = await Z.runCards('final correction', work, cards, rules, { fixRounds: 1 })
      if (cycle.status !== 'done') {
        return { status: 'partial', plan: facts, cycle, error: `the one correction cycle did not complete: ${cycle.error || 'no report'}` }
      }
      const scoped = await Z.runScript('finalize scope', 'Finalize', step('scope', ` -FixWork "${work}"`), ' (the exact changed-Card manifest of the one cycle)')
      if (!scoped.json) return { status: 'partial', plan: facts, cycle, error: (scoped.run && scoped.run.output) || 'the changed-Card manifest was not recorded' }
      scope = scoped.json
      for (const entry of (scope.refresh || [])) {
        if (!entry.applicable || entry.layer === 'audit') continue
        const more = ` -Layer ${entry.layer} -Report "${work}\\report.json"`
        const done = await Z.runScript(`finalize refresh ${entry.layer}`, 'Finalize', step('refresh', more))
        refreshed.push(done.json
          ? { layer: entry.layer, unit: done.json.unit, revision: done.json.revision, state: done.json.state, changed_cards: done.json.changed_cards }
          : { layer: entry.layer, error: (done.run && done.run.output) || 'the refresh did not complete' })
        if (!done.json) log(`Finalize: the ${entry.layer} refresh did not complete: ${(done.run && done.run.output) || 'no output'}.`)
      }
      const owned = (scope.refresh || []).filter(e => e.stage_owned).map(e => e.layer)
      if (owned.length) log(`Finalize: ${owned.join(', ')} keep their own stage as the owner of that refresh; the changed-Card manifest is recorded for it.`)
    }
    const final = await Z.runScript('finalize final', 'Finalize', step('final'), ' (the final read-only audit at the new revision)')
    if (!final.json) return { status: 'failed', plan: facts, cycle, scope, refreshed, error: (final.run && final.run.output) || 'the final audit was not published' }
    const report = final.json
    log(`Finalize: ${report.status} at graph revision ${report.graph_revision}; final audit ${report.audit.final.unit} (revision ${report.audit.final.revision}); ${report.faults.residual} residual hard/source faults, ${report.infrastructure.length} infrastructure faults.`)
    return {
      status: report.status === 'done' ? 'done' : 'partial',
      branch: facts.branch, plan: facts, cycle, scope, refreshed, final: report,
    }
  },
})
// ==== end module zt-finalize ====
