// ==== module zt-group-cycle v3 ====
// The Digest stage of a managed run: one ready group per chain. Each chain claims one `digest/gNNN@1` unit
// from the ledger's ready work, starts it (the snapshot is generated before any usable job manifest exists,
// R262), runs the one frozen DeepSeek job that start wrote, and records the checked completion with
// `group-finish` (R250/R251/R253). The claim is the chain's first command, so a chain that waits for a free
// manager slot never holds a stale lease. A unit that fails is recorded against its own attempt and claimed
// again by a later attempt of this chain; the stage is never rerun as a whole (R258).
// #92: the claim requests a lease that covers the frozen provider-run cap, so the group's six-minute job can
// never be re-readied under a healthy claim while it runs.
// #87 (R280): `group-start` reconciles a retained accepted plan against the archive receipts. When every call
// already committed, its job declares `expected_jobs: 0` and the rebound draft, so this chain runs no DeepSeek
// job at all and completes the group from the committed result.
defineModule({
  id: 'zt-group-cycle', version: '3', requires: ['zt-digest-lib'],
  summary: 'ZT digest: one verified group per manager chain, claimed from the ledger ready work (mode full), recovering a group whose accepted plan already committed.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    group: { type: 'number', required: true, description: 'the number of this chain\'s group item ({{item.n}})' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const label = `group ${p.group}`
    const work = `${Z.OUT}\\group-jobs`
    const retryable = (text) => /stale_claim|read_view_changed|lock_busy|sharing_violation|sqlite_busy|transport_closed|command_timeout|service_unavailable|provider_|missing_result|pending_jobs/.test(String(text || ''))
    for (let attempt = 1; attempt <= Z.MAX_ATTEMPTS; attempt++) {
      const steps = [
        // The bounded claim: one unit, a lease that covers the frozen run cap (#92), and a bounded
        // readiness wait after the first attempt.
        { name: 'claim', claim: true, command: `${Z.LEDGER} next -Out "${Z.OUT}" -Layer digest -Take 1 -Owner "${label} a${attempt}"${attempt > 1 ? ' -Wait 60' : ''} -Lease ${Z.CLAIM_LEASE}` },
        // The start publishes the snapshot and the frozen job manifest, then the chain runs that job. A
        // recovered group's manifest declares no job: its accepted plan already committed.
        { name: 'start', jobs: true, command: `${Z.DIGEST} group-start -Out "${Z.OUT}" -Group {{group}} -Token {{token}}` },
        // The checked completion: revalidates, applies and checks the persisted outputs against the draft.
        { name: 'finish', command: `${Z.DIGEST} group-finish -Out "${Z.OUT}" -Group {{group}} -Token {{token}} -Draft "{{draft}}"` },
      ]
      const chained = await Z.chain(`${label} attempt ${attempt}`, {
        dsh: work, skills: Z.DSH_SKILLS, effort: Z.DSH_EFFORT, phase: 'Digest', writes: work, steps,
      })
      const claim = Z.jsonOf(chained, 'claim')
      const unit = (claim && Array.isArray(claim.units) && claim.units[0]) || null
      if (!unit) return { status: 'idle', group: p.group, attempt, label }
      const start = Z.jsonOf(chained, 'start')
      const finish = Z.resultLine(chained, 'finish', { unit: unit.id, token: unit.token })
      const done = finish.parsed && finish.parsed.state === 'done' && !finish.faults.length
      if (done) {
        if (start && start.recovered) log(`${label}: ${unit.id} recovered from its committed receipts (${(start.recovery && start.recovery.receipts ? start.recovery.receipts.length : 0)} receipts, ${finish.parsed.disposition || 'checked'}); no DeepSeek job ran.`)
        else log(`${label}: ${unit.id} verified (${finish.parsed.disposition || 'checked'}, ${(finish.parsed.owned || []).length} owned Cards).`)
        return { status: 'done', group: p.group, unit: unit.id, disposition: finish.parsed.disposition, owned: (finish.parsed.owned || []).length, recovered: !!(start && start.recovered) }
      }
      // One unit's failure is recorded against its own claim, not turned into a whole-stage rerun.
      const startStep = Z.stepOf(chained, 'start')
      const fault = {
        schema_version: 1, unit: unit.id, input_hash: unit.input_hash || '',
        code: finish.faults.length ? 'check_failed' : 'missing_result',
        cause: (finish.faults[0] || (startStep && startStep.error) || 'the group did not complete').slice(0, 800),
      }
      const transport = ((Z.stepOf(chained, 'start') || {}).ok === false) || ((Z.stepOf(chained, 'finish') || {}).ok === false)
      const refused = !transport && finish.parsed && finish.parsed.state === 'failed'
      const klass = refused || (!transport && retryable(fault.cause) === false && finish.faults.length) ? 'content' : 'retryable'
      if (klass === 'retryable') fault.code = retryable(fault.cause) ? 'provider_transient' : 'missing_result'
      await Z.failUnit(`${label} fault`, unit.id, unit.token, klass, fault)
      log(`${label}: ${unit.id} ${klass} failure: ${fault.cause}`)
      if (klass === 'content') return { status: 'failed', group: p.group, unit: unit.id, error: fault.cause }
    }
    return { status: 'failed', group: p.group, error: `no verified completion after ${Z.MAX_ATTEMPTS} attempts` }
  },
})
// ==== end module zt-group-cycle ====
