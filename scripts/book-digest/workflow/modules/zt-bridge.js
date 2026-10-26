// ==== module zt-bridge v10 ====
// Bounded cross-book discovery publishes a report and applies its new-carrier proposals; an identified selection
// (earlier carriers too) applies through guarded script edits in mode bridge.
// v9: a discovery applies every new-carrier proposal of its report itself (no user review); earlier carriers stay explicit.
// v8 (S8): the no-op cleanupDsh call leaves; the run-level cleanup settles the recorded conversations.
// v10 (#176, docs/workflow-module-structure-proposal.md): the plan, the report, the apply and the publication are
// book-steps.ps1 steps run by DeepSeek drive jobs; the judge jobs run in the layer driver's read-only content
// managers. No Claude agent runs a command.
defineModule({
  id: 'zt-bridge', version: '10', requires: ['zt-digest-lib'],
  summary: 'Bounded cross-book discovery publishes a report and applies its new-carrier proposals; an identified selection (earlier carriers too) applies through guarded script edits in mode bridge.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'run arguments' },
    mode: { type: 'string', required: true, description: 'run mode' },
    model: { type: 'any', description: 'resolved judge model; unused for apply' },
    profile: { type: 'string', required: true, description: 'book or math' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p), A = Z.A, work = `${Z.OUT}\\bridge`
    if (await Z.settledOnResume('bridge', ['bridge'])) return { status: 'done', reused: true }
    for (const [key, low, high] of [['bridgeMax', 1, 120], ['bridgeCandidates', 1, 8]]) if (A[key] !== undefined && (!Number.isInteger(A[key]) || A[key] < low || A[key] > high)) return { status: 'failed', error: `${key} must be an integer in ${low}..${high}.` }
    if ((A.bridgeBooks !== undefined && typeof A.bridgeBooks !== 'string') || (A.bridgeReport !== undefined && typeof A.bridgeReport !== 'string')) return { status: 'failed', error: 'bridgeBooks and bridgeReport must be strings.' }
    const q = v => `'${String(v).replace(/'/g, "''")}'`
    // The stage publishes its own book revision (`zt-digest finalize -Step stage`): the ledger binds its artifacts by
    // hash and checks the coverage it derives. On grossman2002integration trial 8 no stage published, so the revision
    // stayed ready and the run could never be content-complete.
    const publish = async (layer, more) => {
      const json = await Z.bookStep(`${layer}: publish`, 'publish', { Layer: layer, ...more })
      log(json && json.state ? `${layer}: publication ${json.unit || ''} ${json.state}${json.disposition ? ` (${json.disposition})` : ''}.` : `${layer}: the publication failed: ${(json && json.error) || 'no output'}`)
      return json || { layer, state: 'failed', error: 'no output' }
    }
    const selection = v => {
      if (v === undefined) return ''
      if (v === 'all') return 'all'
      if (!Array.isArray(v) || v.some(id => typeof id !== 'string' || !/^b[0-9]{3}$/.test(id)) || new Set(v).size !== v.length) throw Error('Bridge selection must be all or a unique array of proposal ids.')
      return v.join(',')
    }
    let accept, earlier
    try { accept = selection(A.bridgeApply); earlier = selection(A.bridgeApplyEarlier) } catch (e) { return { status: 'failed', error: String(e) } }
    const applying = !!(A.bridgeReport || accept || earlier)
    if (applying && (p.mode !== 'bridge' || !A.bridgeReport || !(accept || earlier)))
      return { status: 'failed', error: 'Bridge apply requires mode bridge, bridgeReport, and a nonempty acceptance section.' }
    const options = (more = {}) => ({ Work: work, CitationKey: Z.KEY, ...more })
    if (applying) {
      const result = await Z.bookStep('bridge: apply', 'bridge-apply', options({ ReportId: A.bridgeReport, ...(accept ? { Accept: accept } : {}), ...(earlier ? { AcceptEarlier: earlier } : {}) }))
      return result || { status: 'failed', error: 'no output' }
    }
    const jobs = { dsh: work, writes: `${work}\\out`, phase: 'Bridge', skills: ['zettelkasten', 'asd-ste100-zh'], effort: Z.DSH_EFFORT, readOnly: true, run: true }
    // A report freezes its plan and proposals. So publish only after a plan whose jobs all answered; otherwise keep the
    // folder unpublished, so a rerun of mode bridge on the same out resumes it.
    let plan, error
    try {
      plan = await Z.bookStep('bridge: plan', 'bridge-start', options({ Profile: p.profile, MaxSources: A.bridgeMax ?? 60, Candidates: A.bridgeCandidates ?? 5, ...(A.bridgeBooks ? { Books: A.bridgeBooks } : {}) }))
      if (!plan || plan.ok !== true) error = `Bridge planning failed: ${(plan && plan.error) || 'no output'}`
      else if (Number(plan.jobs || 0)) {
        const written = await Z.runJobs('bridge: plan write', plan.file, plan.jobs, jobs)
        const failed = (written || []).filter(r => r.status !== 'done').map(r => r.id)
        if (failed.length) error = `Bridge judge jobs failed: ${failed.join(', ')}. The report is not published; rerun mode bridge with the same out to resume.`
      }
    } catch (e) { error = error || String(e) }
    if (error) return { status: 'failed', error }
    const summary = plan && plan.ok === true ? await Z.bookStep('bridge: report', 'bridge-report', options()) : null
    if (!summary || summary.ok !== true || !summary.step || !summary.report_id) return { status: 'failed', error: (summary && summary.error) || 'No Bridge report summary (step and report_id missing).' }
    // v9: the stage applies every new-carrier proposal of its own report at once, so no review waits for the user
    // (acemoglu2007equilibrium: 67 proposals left open). The apply publishes the Bridge revision itself.
    // Earlier-carrier proposals write into another book's tree, usually locked: they stay for an explicit apply.
    if (Number((summary.counts || {}).new) > 0) {
      const result = await Z.bookStep('bridge: apply new', 'bridge-apply', options({ ReportId: summary.report_id, Accept: 'all' }))
      if (!result || result.status === 'failed') return { ...summary, status: 'failed', error: `Bridge apply failed: ${(result && (result.error || result.status)) || 'no detail'}`, applied: result }
      return { ...summary, applied: result, publication: result.publication }
    }
    const publication = await publish('bridge', { Report: `${work}\\proposals.json` })
    return { ...summary, publication }
  },
})
// ==== end module zt-bridge ====

