// ==== module zt-link-pass v9 ====
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
// 9 (#180): a step answers the block's totals (gaps, linked, linked_from, already_linked, no_link, changed, L1/L2
// faults), never per-group rows; the per-group outcomes are the ledger's run report. The report carries the
// run's own totals and how many groups were scheduled, gap-free, verified and blocked.
defineModule({
  id: 'zt-link-pass', version: '9', requires: ['zt-digest-lib'],
  summary: 'ZT digest: verified Link units in blocks of up to eight per manager, for every scheduled or gap-free group, with the gap totals and L1/L2 quality recorded.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
    pending: { type: 'number', default: 0, description: 'the number of groups whose semantic gap set is not empty (from the link preparation)' },
    noGap: { type: 'number', default: 0, description: 'the number of groups whose semantic gap set is empty; their Link unit is settled without a provider run' },
    blocked: { type: 'number', default: 0, description: 'the number of groups the link preparation blocked because their digest did not complete' },
    gaps: { type: 'number', default: 0, description: 'the semantic gap count of the prepared gap set (from the link preparation)' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\link-jobs`
    const deepseek = { deleted: 0, not_deleted: [] }
    const result = await Z.layerRun('link', {
      layer: 'link', take: Z.PER_MANAGER, rounds: Z.MAX_ATTEMPTS, phase: 'Link',
      dsh: work, writes: work, skills: Z.DSH_SKILLS, effort: Z.DSH_EFFORT, what: 'Link writers', deepseek,
    })
    const t = result.totals || {}
    const quality = { rules: 'L1,L2', faults: Number(t.link_quality_faults || 0), checked: Number(t.link_quality_checked || 0) }
    const summary = {
      scheduled: Number(p.pending || 0), skipped: Number(p.noGap || 0),
      gap: Number(p.gaps || 0), skipped_cards: Number(t.link_skipped || 0),
      linked: Number(t.link_linked || 0), linked_from: Number(t.link_linked_from || 0),
      already_linked: Number(t.link_already_linked || 0), no_link: Number(t.link_no_link || 0),
      changed: Number(t.link_changed || 0), provider_runs: Number(t.link_scheduled || 0),
      verified: Number(result.verified || 0), failed: Number(result.failed || 0),
      blocked: Number(p.blocked || 0), quality,
    }
    log(`Link: ${summary.verified} of ${Number(p.pending || 0) + Number(p.noGap || 0)} group(s) verified; ${summary.gap} gaps, ${summary.skipped_cards} skipped Cards, ${summary.linked} written, ${summary.linked_from} linked from another Card, ${summary.no_link} none; ${summary.provider_runs} provider run(s); L1/L2 faults ${quality.faults}.`)
    if (summary.blocked) log(`Link: ${summary.blocked} group(s) stay blocked (their digest unit is not done).`)
    if (result.status === 'failed') return { status: 'failed', error: result.error, ...summary, deepseek }
    return { status: result.status, ...summary, deepseek }
  },
})
// ==== end module zt-link-pass ====
