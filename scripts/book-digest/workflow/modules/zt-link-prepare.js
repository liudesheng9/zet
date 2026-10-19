// ==== module zt-link-prepare v5 ====
// The Link pass preparation: prepare-link-pass.ps1 computes each completed group's semantic gap set
// from its own write receipts (§5.5; D118-D124, #110). Only the gap set becomes a Link job; a Card
// that already carries a valid kinded Link outside its group is skipped and reported. A group whose
// digest unit is not `done` is not scheduled: its allow-list is empty (#92).
// Output: pending (the groups with gaps), no_gap (the groups whose gap set is empty; their Link
// unit is still settled, without a provider run), blocked, and the public counts. Fails the stage
// when the script fails. No stage reads the account-wide usage ledger: run usage is recorded per
// provider run from that run's own result (R343-R346).
defineModule({
  id: 'zt-link-prepare', version: '5', requires: ['zt-digest-lib'],
  summary: 'ZT digest: prepare the receipt-owned semantic gap set of the Link pass (prepare-link-pass.ps1); output the scheduled, gap-free and blocked groups.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const linkPrep = await Z.zAgent(
      `Prepare the receipt-owned semantic gap set of a ZET book digest Link pass. Do exactly this step.
1. Run this PowerShell command (one call): ${Z.tool('prepare-link-pass')} -Staging "${Z.OUT}"
   On a large book it takes several minutes. Run it in the foreground with timeout_ms: 600000 and wait for it; never in the background. If it fails because the ZT service is not up, run \`zt up\` once and run the command again. Its last line is JSON {"pending":[n…],"no_gap":[n…],"blocked":[{"n","unit","state","reason"}],"groups":[{"n","owned","done","digest","blocked","gaps","skipped","position"}],"owned_total","gap_total","skipped_total","pending_count","no_gap_count","scheduled","unowned","digest_state","cards_all"}. If it fails, return ok=false with the error.
Return ok, pending (the "pending" array exactly), no_gap (the "no_gap" array exactly), blocked (the "blocked" array exactly, with the same fields), gaps (an object mapping each group number to its "gaps" count from "groups"), gap_total, skipped_total, owned_total, unowned.`,
      {
        label: 'prepare link pass', phase: 'Link', model: 'haiku',
        schema: {
          type: 'object',
          properties: {
            ok: { type: 'boolean' }, error: { type: 'string' }, pending: { type: 'array', items: { type: 'number' } },
            no_gap: { type: 'array', items: { type: 'number' } },
            blocked: { type: 'array', items: { type: 'object' } },
            gaps: { type: 'object' },
            gap_total: { type: 'number' }, skipped_total: { type: 'number' }, owned_total: { type: 'number' }, unowned: { type: 'number' },
          },
          required: ['ok', 'pending'],
        },
      },
    )
    if (!linkPrep || !linkPrep.ok) {
      log(`Link preparation failed: ${linkPrep && linkPrep.error}`)
      throw new Error(`link preparation failed: ${(linkPrep && linkPrep.error) || 'no report'}`)
    }
    const inRun = (n) => n >= 1 && n <= p.groups
    const pending = linkPrep.pending.filter(inRun)
    const noGap = (linkPrep.no_gap || []).filter(inRun)
    const blocked = (linkPrep.blocked || []).filter(b => b && inRun(b.n))
    const gaps = {}
    for (const g of linkPrep.groups || []) if (g && inRun(g.n)) gaps[g.n] = g.gaps
    const gapTotal = linkPrep.gap_total ?? Object.values(gaps).reduce((sum, n) => sum + (n || 0), 0)
    log(`Link pass: ${gapTotal} semantic gap(s) on ${pending.length} of ${p.groups} groups; ${noGap.length} group(s) need no job; ${linkPrep.skipped_total ?? 0} owned Card(s) skipped.`)
    if (blocked.length) {
      log(`Link pass: ${blocked.length} group(s) blocked because their digest did not complete: ${blocked.map(b => `group ${b.n} (${b.unit} ${b.state})`).join(', ')}. No link job runs for them.`)
    }
    return {
      pending, no_gap: noGap, blocked, gaps,
      gap_total: gapTotal, skipped_total: linkPrep.skipped_total ?? 0,
      owned_total: linkPrep.owned_total ?? 0, unowned: linkPrep.unowned ?? 0,
      scheduled: pending.length, skipped: noGap.length,
      digest_state: linkPrep.digest_state || null,
    }
  },
})
// ==== end module zt-link-prepare ====
