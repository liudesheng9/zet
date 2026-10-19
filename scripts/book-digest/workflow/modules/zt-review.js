// ==== module zt-review v1 ====
// The scoped restatement review of the run's own new Cards, placed where §13.1 orders it: inside the
// scoped Card cycle, after the whole-book Card cycle and before the gap-only Link pass. The review is
// #107's compiler (zt-digest restate); this module only drives it, and it creates no second job store:
// `prepare` reads the run's own staged `owned-g*.txt` manifests, freezes the bounded candidate
// evidence and prints the one judge job, `judge` validates that answer against the frozen evidence,
// `apply` carries each accepted rewrite through the archive's identified receipt path under this
// run's own operation prefix, and `report` renders the explicit per-finding statuses from the frozen
// files alone (it starts no `zt mcp`). The Card cycle's recorded merge verdicts are handed to `apply`
// as `-Merges`, so a merge the cycle proposed but no rewrite covers stays `pending-existing-card-change`
// — a recorded verdict is never counted as an applied merge (D115, D116). Nothing here rewrites an
// older Card: only the run's own new claims are candidates, and a changed one is a conflict, never
// permission to overwrite it (S23 / #115).
defineModule({
  id: 'zt-review', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the run\'s scoped restatement review and resolution (zt-digest restate prepare → judge → apply → report), recorded from the frozen files.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    cardsBook: { type: 'any', description: 'the whole-book Card cycle output (stages.cards-book.output): its recorded merge verdicts are the pending resolutions this review must not count as applied' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    // Where the restatement review keeps its files: the staging copies are the ones the Card cycle's
    // report reads (`restate-resolution.json`), so both live under the run's staging folder.
    const work = `${Z.OUT}\\review\\restate`
    const step = (name, more = '') => `${Z.DIGEST} restate -Step ${name} -Staging "${Z.OUT}" -Work "${work}"${more}`
    // A recorded merge verdict of the card cycle: applied only by a rewrite that covers its pair.
    const merges = p.cardsBook && p.cardsBook.cycle && p.cardsBook.cycle.status === 'done' ? ` -Merges "${Z.OUT}\\cards\\decisions.json"` : ''
    const chained = await Z.chain('restatement review', {
      dsh: work, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: 'Cards', writes: `${work}\\out`,
      steps: [
        // The run's own ownership receipts are the population; the tool refuses a list in which no
        // Card is a Topic Idea claim this review may compare.
        { name: 'prepare', command: step('prepare'), jobs: true },
        { name: 'judge', command: step('judge', ` -File "${work}\\out\\rj-01.json"`), optional: true },
        { name: 'apply', command: step('apply', ` -OperationPrefix "${Z.KEY}/review/restate"${merges}`), optional: true },
        { name: 'report', command: step('report'), optional: true },
      ],
      cleanup: true, keep: true,
    })
    const prepared = Z.jsonOf(chained, 'prepare')
    const judged = Z.jsonOf(chained, 'judge')
    const applied = Z.jsonOf(chained, 'apply')
    const reported = Z.jsonOf(chained, 'report')
    if (!prepared) {
      const cause = String((Z.stepOf(chained, 'prepare') || {}).error || 'no output from the restatement evidence').slice(0, 800)
      // The tool refuses a population in which no listed Card is a reviewable claim. That is a
      // bounded, honest state for a run that filed everything into earlier Topics, so it is reported
      // as skipped with the tool's own reason rather than as a broken stage.
      if (/no new Card .* is a Topic Idea Card|names no new Card/.test(cause)) {
        log(`Restatement review: ${cause}`)
        return { status: 'skipped', reason: cause, resolution: null }
      }
      log(`Restatement review failed: ${cause}`)
      return { status: 'failed', error: cause }
    }
    if (!reported) {
      const cause = String((Z.stepOf(chained, 'report') || {}).error || 'no output from the restatement report').slice(0, 800)
      log(`Restatement review failed: the report was not rendered (${cause}).`)
      return { status: 'failed', evidence: prepared, error: cause }
    }
    const counts = reported.counts || {}
    log(`Restatement review ${reported.report_id}: ${reported.status}${reported.complete ? '' : ' (incomplete)'}; ${counts.pairs ?? 0} pairs, ${counts.resolved_by_rewrite ?? 0} resolved by rewrite, ${counts.pending_existing_card_change ?? 0} pending an existing-Card change, ${counts.unverified ?? 0} unverified, ${counts.unresolved ?? 0} unresolved, ${counts.unjudged ?? 0} unjudged.`)
    if (!applied) log('Restatement review: no accepted rewrite was applied; the report keeps every finding at its recommended or pending state.')
    return {
      status: reported.status === 'resolved' ? 'done' : 'partial',
      evidence: {
        evidence_id: prepared.evidence_id, archive_revision: prepared.archive_revision,
        cards: prepared.cards, reviewable: prepared.reviewable, candidates: prepared.candidates, unread: prepared.unread,
      },
      judged: judged ? { judged_id: judged.judged_id, judged: judged.judged, unjudged: judged.unjudged } : null,
      applied: applied ? { prefix: applied.prefix, written: applied.written, complete: applied.complete, counts: applied.counts } : null,
      resolution: {
        report_id: reported.report_id, status: reported.status, complete: reported.complete,
        counts, written: reported.written ?? null,
      },
      resolution_path: `${Z.OUT}\\restate-resolution.json`,
      report_path: `${work}\\restate-report.json`,
    }
  },
})
// ==== end module zt-review ====
