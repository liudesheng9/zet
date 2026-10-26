// ==== module zt-link-pass v8 ====
// The Link pass over the receipt-owned gap set, in blocks. The layer driver runs the stage as script steps:
// `start` claims a block of up to eight `link/gNNN@1` units and runs each group's `link-start` (the frozen gap
// scope and the one job that writes the draft), the block's jobs run in Haiku managers, and `finish` records each
// checked completion with `link-finish` (R250/R251/R263). Every step is one `layer-steps.ps1` call run by a
// DeepSeek drive job, so no Claude agent relays a command and no Claude agent is spent per group.
// Only the gap set receives a Link: `link-finish` refuses a call on any other Card, and the frozen
// accepted plan keeps a lost reply recoverable and a changed body a conflict (D123, D124, AC08).
// A group whose gap set is empty still settles its unit, with no provider run at all (AC07):
// `link-start` declares expected_jobs 0 and the finish applies the empty checked draft.
// A group whose digest did not complete stays blocked and is passed through, never worked (#92).
// 8 (#175): the block is two script steps and one content round under Haiku managers; the relay-era block is gone.
defineModule({
  id: 'zt-link-pass', version: '8', requires: ['zt-digest-lib'],
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
    const deepseek = { deleted: 0, not_deleted: [] }
    const result = await Z.layerRun('link', {
      layer: 'link', take: Z.PER_MANAGER, rounds: Z.MAX_ATTEMPTS, phase: 'Link',
      dsh: work, writes: work, skills: Z.DSH_SKILLS, effort: Z.DSH_EFFORT, what: 'Link writers', deepseek,
      onFinish: (finished, label) => {
        for (const r of finished.results || []) {
          if (Number(r.jobs || 0) === 0) log(`${label}: ${r.unit} settled with no semantic gap and no provider run (${(r.counts && r.counts.already_linked) || 0} already linked).`)
        }
      },
    })
    const results = result.dispositions.map((d) => {
      const counts = d.counts || {}
      const ran = Number(d.jobs || 0)
      providerRuns += ran
      const gap = Number((p.gaps || {})[d.group]) || 0
      if (d.status !== 'done') return { group: Number(d.group) || null, status: d.status, unit: d.unit, gaps: gap, error: d.cause || d.code || d.status }
      return {
        group: d.group, status: 'done', unit: d.unit,
        gaps: counts.gaps ?? gap, skipped: counts.skipped ?? 0,
        scheduled: ran > 0 ? 1 : 0,
        linked: counts.linked ?? 0, linked_from: counts.linked_from ?? 0,
        already_linked: counts.already_linked ?? 0, no_link: counts.no_link ?? 0,
        provider_runs: ran, changed: d.changed || [],
        quality: d.quality || null,
      }
    }).sort((x, y) => (x.group || 0) - (y.group || 0))
    const blocked = (p.blocked || []).filter(b => b && !todo.some(g => g.n === b.n))
    if (blocked.length) log(`Link: ${blocked.length} group(s) stay blocked (their digest unit is not done): ${blocked.map(b => `group ${b.n}`).join(', ')}.`)
    const total = (name) => results.reduce((sum, r) => sum + (Number(r[name]) || 0), 0)
    const quality = { rules: 'L1,L2', faults: results.reduce((sum, r) => sum + (Number(r.quality && r.quality.faults) || 0), 0), checked: results.reduce((sum, r) => sum + (Number(r.quality && r.quality.checked) || 0), 0) }
    log(`Link: ${results.filter(r => r.status === 'done').length} of ${todo.length} group(s) verified; ${total('gaps')} gaps, ${total('skipped')} skipped Cards, ${total('linked')} written, ${total('linked_from')} linked from another Card, ${total('no_link')} none; ${providerRuns} provider run(s); L1/L2 faults ${quality.faults}.`)
    if (result.status === 'failed') {
      return { status: 'failed', error: result.error, groups: results, blocked, scheduled: scheduled.length, skipped: skipped.length, provider_runs: providerRuns, quality, verified: results.filter(r => r.status === 'done').length, failed: results.filter(r => r.status !== 'done').length }
    }
    return {
      groups: results, blocked,
      scheduled: scheduled.length, skipped: skipped.length,
      gap: total('gaps'), skipped_cards: total('skipped'),
      linked: total('linked'), linked_from: total('linked_from'), already_linked: total('already_linked'), no_link: total('no_link'),
      provider_runs: providerRuns, quality, deepseek,
      verified: results.filter(r => r.status === 'done').length,
      failed: results.filter(r => r.status !== 'done').length,
    }
  },
})
// ==== end module zt-link-pass ====
