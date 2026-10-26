// ==== module zt-link-prepare v8 ====
// v6 (S1): one relay runs prepare-link -Brief and the module reads its line; the Haiku agent that copied the arrays
// back (and ran `zt up` itself) leaves: zt-digest starts and waits for the service.
// v7 (#176, docs/workflow-module-structure-proposal.md): the stage's one command is a book-steps.ps1 step
// (`link-prepare`) run by a DeepSeek drive job; no Claude relay runs a command.
// v8 (#180): the step answers counts of the prepared gap set, not its group arrays. The scheduling is the
// ledger's anyway (the Link units are claimed from it), so the stage reports how many groups are scheduled,
// gap-free and blocked, the gap total and the digest state; the group lists stay in the step's detail file.
// The Link pass preparation: prepare-link-pass computes each completed group's semantic gap set
// from its own write receipts (§5.5; D118-D124, #110). Only the gap set becomes a Link job; a Card
// that already carries a valid kinded Link outside its group is skipped and reported. A group whose
// digest unit is not `done` is not scheduled: its allow-list is empty (#92).
// Fails the stage when the step fails. No stage reads the account-wide usage ledger: run usage is recorded per
// provider run from that run's own result (R343-R346).
defineModule({
  id: 'zt-link-prepare', version: '8', requires: ['zt-digest-lib'],
  summary: 'ZT digest: prepare the receipt-owned semantic gap set of the Link pass (one book-steps.ps1 step); output the scheduled, gap-free and blocked group counts.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const line = await Z.bookStep('prepare link pass', 'link-prepare')
    if (!line || line.ok !== true || line.pending === undefined) {
      const error = (line && line.error) || 'no report'
      log(`Link preparation failed: ${error}`)
      throw new Error(`link preparation failed: ${error}`)
    }
    const n = (v) => Number(v || 0)
    const pending = n(line.pending)
    const noGap = n(line.no_gap)
    const blocked = n(line.blocked)
    const gapTotal = n(line.gap_total)
    log(`Link pass: ${gapTotal} semantic gap(s) on ${pending} of ${p.groups} groups; ${noGap} group(s) need no job; ${n(line.skipped_total)} owned Card(s) skipped.`)
    if (blocked) log(`Link pass: ${blocked} group(s) blocked because their digest did not complete. No link job runs for them.`)
    return {
      pending, no_gap: noGap, blocked, gaps: gapTotal,
      gap_total: gapTotal, skipped_total: n(line.skipped_total),
      owned_total: n(line.owned_total), unowned: n(line.unowned),
      scheduled: pending, skipped: noGap,
      digest_state: line.digest_state || null,
    }
  },
})
// ==== end module zt-link-prepare ====
