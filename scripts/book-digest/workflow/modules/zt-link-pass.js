// ==== module zt-link-pass v4 ====
// The Link pass over the receipt-owned gap set: one chain per group. Each chain claims the group's
// `link/gNNN@1` unit, runs `link-start` (the frozen gap scope and the one job that writes the
// draft), runs that job, and records the checked completion with `link-finish` (R250/R251/R263).
// Only the gap set receives a Link: `link-finish` refuses a call on any other Card, and the frozen
// accepted plan keeps a lost reply recoverable and a changed body a conflict (D123, D124, AC08).
// A group whose gap set is empty still settles its unit, with no provider run at all (AC07):
// `link-start` declares expected_jobs 0 and the chain applies the empty checked draft.
// A group whose digest did not complete stays blocked and is passed through, never worked (#92).
defineModule({
  id: 'zt-link-pass', version: '4', requires: ['zt-digest-lib'],
  summary: 'ZT digest: one verified Link chain per scheduled or gap-free group, with the gap counts and L1/L2 quality recorded.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
    pending: { type: 'array', required: true, description: 'the group numbers whose semantic gap set is not empty (from the link preparation)' },
    noGap: { type: 'array', default: [], description: 'the group numbers whose semantic gap set is empty; their Link unit is settled without a provider run' },
    gaps: { type: 'object', default: {}, description: 'the semantic gap count per group from the link preparation' },
    blocked: { type: 'array', default: [], description: 'the groups the link preparation blocked because their digest did not complete' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const retryable = (text) => /stale_claim|read_view_changed|lock_busy|sharing_violation|sqlite_busy|transport_closed|command_timeout|service_unavailable|provider_|missing_result|pending_jobs/.test(String(text || ''))
    const scheduled = Z.groupsOf(p.groups).filter(g => (p.pending || []).includes(g.n))
    const skipped = Z.groupsOf(p.groups).filter(g => (p.noGap || []).includes(g.n) && !(p.pending || []).includes(g.n))
    const todo = [...scheduled, ...skipped]
    const work = `${Z.OUT}\\link-jobs`
    let providerRuns = 0
    const one = async (g) => {
      const label = `link group ${g.n}`
      const gap = Number((p.gaps || {})[g.n]) || 0
      for (let attempt = 1; attempt <= Z.MAX_ATTEMPTS; attempt++) {
        const steps = [
          { name: 'claim', claim: true, command: `${Z.LEDGER} next -Out "${Z.OUT}" -Layer link -Take 1 -Owner "${label} a${attempt}"${attempt > 1 ? ' -Wait 60' : ''} -Lease ${Z.CLAIM_LEASE}` },
          { name: 'start', jobs: true, command: `${Z.DIGEST} link-start -Out "${Z.OUT}" -Group {{group}} -Token {{token}}` },
          { name: 'finish', command: `${Z.DIGEST} link-finish -Out "${Z.OUT}" -Group {{group}} -Token {{token}} -Draft "{{draft}}"` },
        ]
        const chained = await Z.chain(`${label} attempt ${attempt}`, {
          dsh: work, skills: Z.DSH_SKILLS, effort: Z.DSH_EFFORT, phase: 'Link', writes: work, steps,
        })
        const claim = Z.jsonOf(chained, 'claim')
        const unit = (claim && Array.isArray(claim.units) && claim.units[0]) || null
        if (!unit) return { group: g.n, status: 'idle', gaps: gap }
        const start = Z.jsonOf(chained, 'start')
        const finish = Z.resultLine(chained, 'finish', { unit: unit.id, token: unit.token })
        const ran = (chained.results || []).length
        providerRuns += ran
        const done = finish.parsed && finish.parsed.state === 'done' && !finish.faults.length
        if (done) {
          const counts = finish.parsed.counts || {}
          if (start && start.expected_jobs === 0) log(`${label}: ${unit.id} settled with no semantic gap and no provider run (${counts.already_linked ?? 0} already linked).`)
          else log(`${label}: ${unit.id} verified (${counts.linked ?? 0} written, ${counts.linked_from ?? 0} linked from another Card, ${counts.no_link ?? 0} none; ${finish.parsed.quality ? `${finish.parsed.quality.rules} faults ${finish.parsed.quality.faults}` : 'checked'}).`)
          return {
            group: g.n, status: 'done', unit: unit.id,
            gaps: counts.gaps ?? gap, skipped: counts.skipped ?? 0,
            scheduled: (start && start.expected_jobs === 0) ? 0 : 1,
            linked: counts.linked ?? 0, linked_from: counts.linked_from ?? 0,
            already_linked: counts.already_linked ?? 0, no_link: counts.no_link ?? 0,
            provider_runs: ran, changed: finish.parsed.changed || [],
            quality: finish.parsed.quality || null,
          }
        }
        const startStep = Z.stepOf(chained, 'start')
        const fault = {
          schema_version: 1, unit: unit.id, input_hash: unit.input_hash || '',
          code: finish.faults.length ? 'check_failed' : 'missing_result',
          cause: (finish.faults[0] || (startStep && startStep.error) || 'the Link unit did not complete').slice(0, 800),
        }
        const transport = ((Z.stepOf(chained, 'claim') || {}).ok === false) || ((startStep || {}).ok === false) || ((Z.stepOf(chained, 'finish') || {}).ok === false)
        const klass = !transport && retryable(fault.cause) === false ? 'content' : 'retryable'
        if (klass === 'retryable') fault.code = retryable(fault.cause) ? 'provider_transient' : 'missing_result'
        await Z.failUnit(`${label} fault`, unit.id, unit.token, klass, fault)
        log(`${label}: ${unit.id} ${klass} failure: ${fault.cause}`)
        if (klass === 'content') return { group: g.n, status: 'failed', unit: unit.id, gaps: gap, error: fault.cause }
      }
      return { group: g.n, status: 'failed', gaps: gap, error: `no verified completion after ${Z.MAX_ATTEMPTS} attempts` }
    }
    const results = await Promise.all(todo.map(one))
    const blocked = (p.blocked || []).filter(b => b && !todo.some(g => g.n === b.n))
    if (blocked.length) log(`Link: ${blocked.length} group(s) stay blocked (their digest unit is not done): ${blocked.map(b => `group ${b.n}`).join(', ')}.`)
    const total = (name) => results.reduce((sum, r) => sum + (Number(r[name]) || 0), 0)
    const quality = { rules: 'L1,L2', faults: results.reduce((sum, r) => sum + (Number(r.quality && r.quality.faults) || 0), 0), checked: results.reduce((sum, r) => sum + (Number(r.quality && r.quality.checked) || 0), 0) }
    log(`Link: ${results.filter(r => r.status === 'done').length} of ${todo.length} group(s) verified; ${total('gaps')} gaps, ${total('skipped')} skipped Cards, ${total('linked')} written, ${total('linked_from')} linked from another Card, ${total('no_link')} none; ${providerRuns} provider run(s); L1/L2 faults ${quality.faults}.`)
    return {
      groups: results, blocked,
      scheduled: scheduled.length, skipped: skipped.length,
      gap: total('gaps'), skipped_cards: total('skipped'),
      linked: total('linked'), linked_from: total('linked_from'), already_linked: total('already_linked'), no_link: total('no_link'),
      provider_runs: providerRuns, quality,
      verified: results.filter(r => r.status === 'done').length,
      failed: results.filter(r => r.status !== 'done').length,
    }
  },
})
// ==== end module zt-link-pass ====
