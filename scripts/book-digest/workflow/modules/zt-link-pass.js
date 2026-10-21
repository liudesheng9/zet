// ==== module zt-link-pass v7 ====
// The Link pass over the receipt-owned gap set, in blocks: one Haiku manager claims up to eight
// `link/gNNN@1` units, runs `link-start` for each (the frozen gap scope and the one job that writes the
// draft), runs their jobs side by side, and records each checked completion with `link-finish`
// (R250/R251/R263). At most eight lanes run at once, each claiming block after block until none is ready.
// Only the gap set receives a Link: `link-finish` refuses a call on any other Card, and the frozen
// accepted plan keeps a lost reply recoverable and a changed body a conflict (D123, D124, AC08).
// A group whose gap set is empty still settles its unit, with no provider run at all (AC07):
// `link-start` declares expected_jobs 0 and the chain applies the empty checked draft.
// A group whose digest did not complete stays blocked and is passed through, never worked (#92).
defineModule({
  id: 'zt-link-pass', version: '7', requires: ['zt-digest-lib'],
  summary: 'ZT digest: verified Link units in blocks of up to eight per manager, for every scheduled or gap-free group, with the gap counts and L1/L2 quality recorded.',
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
    const scheduled = Z.groupsOf(p.groups).filter(g => (p.pending || []).includes(g.n))
    const skipped = Z.groupsOf(p.groups).filter(g => (p.noGap || []).includes(g.n) && !(p.pending || []).includes(g.n))
    const todo = [...scheduled, ...skipped]
    const work = `${Z.OUT}\\link-jobs`
    let providerRuns = 0
    const latest = new Map()
    // One held Link unit's outcome: the ledger decides (zt-digest-lib blockSettle); the manager's finish line
    // adds the counts and the quality when it came back whole.
    const settle = async (u, label) => {
      const gap = Number((p.gaps || {})[u.group]) || 0
      const start = zdLastJson(u.start && u.start.output)
      const ran = u.jobs.length
      providerRuns += ran
      const got = await Z.blockSettle(u, label)
      if (got.status !== 'done') return { group: u.group, status: got.status, unit: u.unit, gaps: gap, error: got.error }
      const counts = got.parsed.counts || {}
      if (start && start.expected_jobs === 0) log(`${label}: ${u.unit} settled with no semantic gap and no provider run (${counts.already_linked ?? 0} already linked).`)
      return {
        group: u.group, status: 'done', unit: u.unit,
        gaps: counts.gaps ?? gap, skipped: counts.skipped ?? 0,
        scheduled: (start && start.expected_jobs === 0) ? 0 : 1,
        linked: counts.linked ?? 0, linked_from: counts.linked_from ?? 0,
        already_linked: counts.already_linked ?? 0, no_link: counts.no_link ?? 0,
        provider_runs: ran, changed: got.parsed.changed || [],
        quality: got.parsed.quality || null,
      }
    }
    const lanes = Math.max(1, Math.min(8, Math.ceil(todo.length / Z.BLOCK_TAKE)))
    const maxBlocks = Math.ceil(todo.length / Z.BLOCK_TAKE) * Z.MAX_ATTEMPTS + 1
    const lane = async (n) => {
      for (let round = 1; round <= maxBlocks; round++) {
        const label = `link lane ${n} block ${round}`
        const got = await Z.block(label, {
          dsh: work, skills: Z.DSH_SKILLS, effort: Z.DSH_EFFORT, phase: 'Link', writes: work,
          claim: `${Z.LEDGER} next -Out "${Z.OUT}" -Layer link -Take ${Z.BLOCK_TAKE} -Brief -Owner "${label}"${round > 1 ? ' -Wait 60' : ''} -Lease ${Z.CLAIM_LEASE}`,
          start: `${Z.DIGEST} link-start -Out "${Z.OUT}" -Group {{group}} -Token {{token}}`,
          finish: `${Z.DIGEST} link-finish -Out "${Z.OUT}" -Group {{group}} -Token {{token}} -Draft "{{draft}}"`,
        })
        if (!got.claimed.length) {
          // A manager that threw before its claim ran holds nothing: take the next round (trial 17).
          if (got.thrown && round < maxBlocks) continue
          if (!got.ok) log(`${label}: the claim did not run: ${got.error || 'no claim line'}`)
          return
        }
        for (const u of got.units) latest.set(u.unit, await settle(u, label))
      }
    }
    if (todo.length) await Promise.all(Array.from({ length: lanes }, (_, i) => lane(i + 1)))
    const results = [...latest.values()].sort((x, y) => (x.group || 0) - (y.group || 0))
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
