// ==== module zt-concepts v1 ====
// The scoped concept review of one prepared run, placed where §13.1 orders it: after the Terms stage
// and before the first group writer, so the run freezes its concept decisions before it writes a Card.
// The review itself is #97's compiler (zt-digest concepts); this module only drives it, and it adds no
// second store: `propose` derives the proposals from the run's own book term table (one concept per
// term row, the book's preferred and rejected spellings), `prepare` freezes the bounded candidate
// evidence, one DeepSeek judge answers it, `judge` validates that answer against the frozen evidence
// and `report` renders the immutable review from the frozen files alone — the report step starts no
// `zt mcp` and writes no Card. The report and its ids travel in this stage's output, so the hub
// stage's register plan can read it as evidence and the run report can name what was reviewed.
// A run without a term table proposes no concept and says why: the stage is `skipped` with its
// reason, never a clean review of an empty set (S23 / #115).
defineModule({
  id: 'zt-concepts', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the frozen concept review of the run\'s own term table (zt-digest concepts propose → prepare → judge → report).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const work = `${Z.OUT}\\review\\concepts`
    const step = (name, more = '') => `${Z.DIGEST} concepts -Step ${name} -Staging "${Z.OUT}" -Work "${work}"${more}`
    const proposed = await Z.runScript('concepts: propose', 'Prepare', step('propose'), ' (it reads the run\'s own term table and freezes the proposals; it writes no Card)')
    const facts = proposed.json
    if (!facts || facts.ok !== true) {
      return { status: 'failed', error: (proposed.run && proposed.run.output) || 'the concept proposals were not derived' }
    }
    if (!facts.proposals) {
      log(`Concept review: ${facts.reason}`)
      return { status: 'skipped', reason: facts.reason, proposals: 0, term_rows: facts.term_rows ?? 0, review: null }
    }
    // One manager runs the frozen evidence, the one judge job it prints, the validated answer and the
    // report. The judge is optional: a refused or absent answer leaves pairs unjudged, which the
    // report names as `partial`; the report never needs the answer to exist.
    const chained = await Z.chain('concept review', {
      dsh: work, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: 'Prepare', writes: `${work}\\out`,
      steps: [
        { name: 'prepare', command: step('prepare'), jobs: true },
        { name: 'judge', command: step('judge', ` -File "${work}\\out\\cj-01.json"`), optional: true },
        { name: 'report', command: step('report') },
      ],
      cleanup: true, keep: true,
    })
    const prepared = Z.jsonOf(chained, 'prepare')
    const judged = Z.jsonOf(chained, 'judge')
    const reported = Z.jsonOf(chained, 'report')
    if (!prepared) {
      const cause = String((Z.stepOf(chained, 'prepare') || {}).error || 'no output from the concept evidence').slice(0, 800)
      log(`Concept review failed: ${cause}`)
      return { status: 'failed', proposals: facts.proposals, error: cause }
    }
    if (!reported) {
      const cause = String((Z.stepOf(chained, 'report') || {}).error || 'no output from the concept report').slice(0, 800)
      log(`Concept review failed: the report was not rendered (${cause}).`)
      return { status: 'failed', proposals: facts.proposals, evidence: prepared, error: cause }
    }
    log(`Concept review ${reported.report_id}: ${reported.status}${reported.complete ? '' : ' (partial)'}; ${reported.findings} findings over ${facts.proposals} proposed concepts, ${reported.judged ?? 0} judged, ${reported.unjudged ?? 0} unjudged, ${reported.unread ?? 0} unread.`)
    if (judged) log(`Concept review: ${judged.judged} judgments accepted (${judged.pairs} pairs), ${judged.unjudged} unjudged.`)
    else log('Concept review: no accepted judge answer; every pair stays unjudged and the report is partial.')
    return {
      status: reported.status === 'partial' ? 'partial' : 'done',
      proposals: facts.proposals,
      term_rows: facts.term_rows ?? 0,
      evidence: {
        evidence_id: prepared.evidence_id, archive_revision: prepared.archive_revision,
        topic_candidates: prepared.topic_candidates, restatement_candidates: prepared.restatement_candidates, unread: prepared.unread,
      },
      judged: judged ? { judged_id: judged.judged_id, judged: judged.judged, pairs: judged.pairs, unjudged: judged.unjudged } : null,
      review: {
        report_id: reported.report_id, status: reported.status, complete: reported.complete,
        findings: reported.findings, judged: reported.judged, unjudged: reported.unjudged, unread: reported.unread,
        written: reported.written ?? reported.paths ?? null,
      },
      report_path: `${Z.OUT}\\concept-review.json`,
      markdown_path: `${Z.OUT}\\concept-review.md`,
    }
  },
})
// ==== end module zt-concepts ====
