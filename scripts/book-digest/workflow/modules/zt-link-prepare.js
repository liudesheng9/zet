// ==== module zt-link-prepare v7 ====
// v6 (S1): one relay runs prepare-link -Brief and the module reads its line; the Haiku agent that copied the arrays
// back (and ran `zt up` itself) leaves: zt-digest starts and waits for the service.
// v7 (#176, docs/workflow-module-structure-proposal.md): the stage's one command is a book-steps.ps1 step
// (`link-prepare`) run by a DeepSeek drive job; no Claude relay runs a command.
// The Link pass preparation: prepare-link-pass computes each completed group's semantic gap set
// from its own write receipts (§5.5; D118-D124, #110). Only the gap set becomes a Link job; a Card
// that already carries a valid kinded Link outside its group is skipped and reported. A group whose
// digest unit is not `done` is not scheduled: its allow-list is empty (#92).
// Output: pending (the groups with gaps), no_gap (the groups whose gap set is empty; their Link
// unit is still settled, without a provider run), blocked, and the public counts. Fails the stage
// when the step fails. No stage reads the account-wide usage ledger: run usage is recorded per
// provider run from that run's own result (R343-R346).
defineModule({
  id: 'zt-link-prepare', version: '7', requires: ['zt-digest-lib'],
  summary: 'ZT digest: prepare the receipt-owned semantic gap set of the Link pass (one book-steps.ps1 step); output the scheduled, gap-free and blocked groups.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const line = await Z.bookStep('prepare link pass', 'link-prepare')
    if (!line || line.ok !== true || !Array.isArray(line.pending)) {
      const error = (line && line.error) || 'no report'
      log(`Link preparation failed: ${error}`)
      throw new Error(`link preparation failed: ${error}`)
    }
    const inRun = (n) => n >= 1 && n <= p.groups
    const pending = line.pending.filter(inRun)
    const noGap = (line.no_gap || []).filter(inRun)
    const blocked = (line.blocked || []).filter(b => b && inRun(b.n))
    const gaps = {}
    for (const [n, count] of Object.entries(line.gaps || {})) if (inRun(Number(n))) gaps[Number(n)] = count
    const gapTotal = line.gap_total ?? Object.values(gaps).reduce((sum, n) => sum + (n || 0), 0)
    log(`Link pass: ${gapTotal} semantic gap(s) on ${pending.length} of ${p.groups} groups; ${noGap.length} group(s) need no job; ${line.skipped_total ?? 0} owned Card(s) skipped.`)
    if (blocked.length) {
      log(`Link pass: ${blocked.length} group(s) blocked because their digest did not complete: ${blocked.map(b => `group ${b.n} (${b.unit} ${b.state})`).join(', ')}. No link job runs for them.`)
    }
    return {
      pending, no_gap: noGap, blocked, gaps,
      gap_total: gapTotal, skipped_total: line.skipped_total ?? 0,
      owned_total: line.owned_total ?? 0, unowned: line.unowned ?? 0,
      scheduled: pending.length, skipped: noGap.length,
      digest_state: line.digest_state || null,
    }
  },
})
// ==== end module zt-link-prepare ====
