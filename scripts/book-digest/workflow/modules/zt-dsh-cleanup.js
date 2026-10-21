// ==== module zt-dsh-cleanup v9 ====
// R231/R322–R330: the run's cleanup is settled from ledger evidence, never from a workspace listing's
// titles. The module reads every resource this run recorded and hands the conversations not yet settled to
// cleanup agents in chunks (at most eight agents at once). Each agent deletes exactly its recorded ids and
// records each fact itself, right after it acted, with a plain `ledger resource -Settle` command; one agent
// per workspace batch then records the workspace listing and its removal with `-SettleWorkspace` (an
// unknown member is reported, never removed). No relay restates another agent's observation. Every cleanup
// unit then settles through its checked verifier. The accepted answers were
// retained when the ids were recorded; a disposable model folder is removed by the verifier only after
// its accepted plan sits under the durable evidence path. A partial cleanup keeps the run partial; a
// provider fault stays retryable and rebuilds no content.
// The run report (#85, R331–R346) is rendered here, after cleanup settles: every stage of the run has
// finished, so the report describes the whole invocation. Its usage section is the exact per-run usage
// the ledger recorded from each run's own result; this module never reads or differences the account-wide
// usage ledger, because the difference of two rolling totals is not a run's cost (R344).
defineModule({
  id: 'zt-dsh-cleanup', version: '9', requires: ['zt-digest-lib'],
  summary: 'ZT digest: settle every recorded DeepSeek conversation and workspace from ledger evidence, then render the verified run report.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const OUT = Z.OUT
    // The recorded conversations come as the brief listing (`ledger resource -List -Brief`): per workspace, the
    // recorded ids, the ones still to delete and the ones without evidence. The full listing was about 52 KB on
    // grossman2002integration trial 8; its relay answered in prose, and the cleanup settled nothing and rendered
    // no run report.
    // No id list passes through a relay (zt-dsh-cleanup v8): the stage reads the counts per workspace, the ledger records
    // the missing evidence itself, and each agent reads the ids of its own workspace from the ledger. On
    // grossman2002integration trial 11 the relay abridged the brief listing's id arrays and left out the two model
    // workspaces, so their 83 conversations were never deleted and the run-level cleanup could not settle.
    const listed = await Z.runScript('cleanup: list the recorded resources', 'Cleanup', `${Z.LEDGER} resource -Out "${OUT}" -List -Counts`, ' (it counts the recorded DeepSeek conversations per workspace; it changes nothing)')
    const brief = listed.json && Array.isArray(listed.json.workspaces) ? listed.json : null
    if (!brief) return { status: 'failed', cleanup: null, usage: null, report: null, error: String((listed.run && listed.run.output) || 'no readable resource listing') }
    // A workspace's id is its folder path; a relay that dropped `path` (trial 20) still names each workspace by its id,
    // and dropping those left the stage nothing to settle, so the run-level cleanup refused every unit.
    const spaces = brief.workspaces.filter((w) => w && (w.path || w.id)).map((w) => ({ ...w, path: w.path || w.id }))
    const workspaces = spaces.map((w) => w.id || w.path)
    let report = { workspaces: [] }
    const LEDGER = Z.LEDGER
    const SETTLE_CHUNK = 40
    const count = (w, key) => Number(w[key]) || 0
    // A manager may write a job's answer to <out>\answers and skip its `-EvidenceFor` record (trial 6: two of
    // three review jobs), and the cleanup verifier refuses a conversation with no evidence
    // (`cleanup_evidence_missing`). Before anything is deleted, one bounded command per 40 ids records the
    // evidence of every such conversation from its own answer file (a deleted one too: the verifier asks every recorded conversation for its evidence); a conversation with no answer file stays
    // without evidence and the verifier names it.
    if (spaces.some((w) => count(w, 'bare') > 0)) {
      const swept = await Z.runScript('cleanup: evidence', 'Cleanup', `${LEDGER} resource -Out "${OUT}" -SweepEvidence "${Z.ANSWERS}"`, ' (it records answers already on disk; it changes no Card)')
      const s = swept.json || {}
      log(`Cleanup: recorded the evidence of ${s.recorded || 0} conversation(s) from their answer files${s.missing ? `; ${s.missing} without an answer file: ${(s.first_missing || []).join(', ')}` : ''}.`)
    }
    // One agent per 40 open conversations of a workspace; it reads that workspace's ids from the ledger itself.
    const chunks = []
    for (const w of spaces) for (let from = 0; from < count(w, 'open'); from += SETTLE_CHUNK) chunks.push({ path: w.path, from, to: Math.min(count(w, 'open'), from + SETTLE_CHUNK) - 1 })
    const SETTLED = {
      type: 'object',
      properties: { settled: { type: 'number' }, unrecorded: { type: 'array', items: { type: 'string' } } },
      required: ['settled'],
    }
    const settledChunks = await Promise.all(chunks.map((chunk, k) => dsmpSlot(() => Z.zAgent(
      `Settle these DeepSeek Harness (DSH) conversations of one digest run. Work only with the recorded ids you read in step 0: never delete a conversation because its title looks like this run's, and never delete one the ledger does not list.
Load the tools with one ToolSearch call: query "select:mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_cancel,mcp__deepseek__deepseek_delete".
Workspace: "${chunk.path}". Your share: the entries ${chunk.from} to ${chunk.to} (counting from 0) of its open ids.
0. Run this command (it changes nothing): ${LEDGER} resource -Out "${OUT}" -List -Workspace "${chunk.path}". Its last line is JSON; take workspace.open and keep only the entries ${chunk.from} to ${chunk.to}. These are your ids.
1. deepseek_list with cwd = that workspace path (once). If the workspace is not found, every id is absent.
2. For each id, in order:
   - Listed and "running": deepseek_cancel (session_id), then delete it as below with -State cancelled.
   - Listed: deepseek_delete (session_id). Then record it with this command (it changes no Card): ${LEDGER} resource -Out "${OUT}" -Settle <id> -State <finished or error, as listed> -Deleted confirmed
   - Not listed: record it: ${LEDGER} resource -Out "${OUT}" -Settle <id> -State absent -Deleted already_absent
   - deepseek_delete fails (try at most twice): record it: ${LEDGER} resource -Out "${OUT}" -Settle <id> -State finished -Deleted failed -Message '<the error, without quotes>'
   Never archive a conversation.
Return settled = how many of your ids you recorded, and unrecorded = the ids whose record command failed (usually none).`,
      { label: `cleanup chunk ${k + 1} of ${chunks.length}`, phase: 'Cleanup', model: 'haiku', schema: SETTLED },
    ))))
    const unrecorded = settledChunks.flatMap((got, k) => (got && typeof got.settled === 'number' ? (got.unrecorded || []) : [`${chunks[k].path} entries ${chunks[k].from}–${chunks[k].to}`]))
    if (unrecorded.length) log(`Cleanup: ${unrecorded.length} conversation(s) or shares without a recorded settlement: ${unrecorded.slice(0, 12).join(', ')}${unrecorded.length > 12 ? ', …' : ''}`)
    // The workspaces: list each one after its deletions, remove it only when nothing recorded or unknown
    // remains, and record the listing and the removal with one plain command per workspace.
    const paths = spaces.map((w) => w.path)
    const WORKSPACES = {
      type: 'object',
      properties: { workspaces: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, removed: { type: 'boolean' }, unknown: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, title: { type: 'string' } }, required: ['id'] } }, remaining: { type: 'array', items: { type: 'string' } }, recorded: { type: 'boolean' } }, required: ['id', 'removed'] } } },
      required: ['workspaces'],
    }
    const batches = []
    for (let i = 0; i < paths.length; i += 8) batches.push(paths.slice(i, i + 8))
    const listedBatches = await Promise.all(batches.map((batch, k) => dsmpSlot(() => Z.zAgent(
      `Settle the DeepSeek Harness (DSH) workspaces of one digest run, after their recorded conversations were deleted.
Load the tools with one ToolSearch call: query "select:mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_remove_workspace".
Workspaces (JSON): ${JSON.stringify(batch)}
For each workspace:
0. Run this command (it changes nothing): ${LEDGER} resource -Out "${OUT}" -List -Workspace "<path>". Its last line is JSON; workspace.recorded holds the ids this run recorded there.
1. deepseek_list with cwd = that path. remaining = the recorded ids still listed; unknown = every listed conversation that is NOT recorded there (id and title): those belong to the user and must stay.
2. When remaining and unknown are both empty: deepseek_remove_workspace with cwd = that path and delete_conversations true. removed = true only when it succeeded. Otherwise do not remove it; removed = false.
3. Record it with this command (it changes no Card): ${LEDGER} resource -Out "${OUT}" -SettleWorkspace "<path>" -Removed <true or false> -Remaining '<remaining ids, comma-separated>' -Unknown '<unknown ids, comma-separated>'
Return workspaces: one entry per workspace with id (its path), removed, unknown, remaining and recorded (true when the record command succeeded). Do not read or sum account-wide usage totals.`,
      { label: `cleanup workspaces ${k + 1} of ${batches.length}`, phase: 'Cleanup', model: 'haiku', schema: WORKSPACES },
    ))))
    report = { workspaces: listedBatches.flatMap((got) => (got && got.workspaces) || []) }
    // Every cleanup unit this run recorded: claim the ready ones in bounded rounds, settle each through
    // its own checked verifier, and record a retryable fault when a provider fact still blocks it.
    // The rounds go on until no ready unit is left or every unit a claim offers has already failed three times
    // here: on grossman2002integration trial 10 three rounds of eight claims went to the same failing units and the
    // eight link cleanup units were never claimed. The book unit settles last, after the units it reports on.
    const settled = []
    const tries = new Map()
    for (let round = 0; round < 60; round++) {
      const claimed = await Z.claim(`cleanup: claim round ${round + 1}`, 'cleanup', 8, 0)
      const units = (claimed.units || []).slice().sort((a, b) => (a.id === 'cleanup/book@1') - (b.id === 'cleanup/book@1'))
      if (!units.length || units.every((u) => (tries.get(u.id) || 0) >= 3)) break
      for (const unit of units) {
        tries.set(unit.id, (tries.get(unit.id) || 0) + 1)
        const done = await Z.finishCleanup(`cleanup ${unit.id}`, unit.id, unit.token)
        if (done.ok) {
          settled.push({ unit: unit.id, status: (done.json && done.json.status) || 'done', json: done.json })
          continue
        }
        await Z.failUnit(`cleanup: ${unit.id} fault`, unit.id, unit.token, 'retryable', {
          schema_version: 1, unit: unit.id, input_hash: unit.input_hash || '', code: 'provider_transient',
          cause: String(done.error || 'the checked cleanup did not complete').slice(0, 800),
        })
        settled.push({ unit: unit.id, status: 'retry', error: done.error })
      }
    }
    const last = [...new Map(settled.map((s) => [s.unit, s])).values()]
    settled.length = 0
    settled.push(...last)
    const run = settled.find((s) => s.unit === 'cleanup/book@1' && s.json) || settled.find((s) => s.json) || null
    const cleanup = {
      resources: brief.resources,
      workspaces: workspaces.length,
      units: settled.map((s) => ({ unit: s.unit, status: s.status, cleanup_complete: s.json ? s.json.cleanup_complete : null, content_complete: s.json ? s.json.content_complete : null, reconciliation: s.json ? s.json.reconciliation : null })),
      workspace_removed: (report.workspaces || []).every((w) => w.removed === true),
      late_deleted: (report.workspaces || []).reduce((n, w) => n + (w.conversations || []).filter((c) => c.deleted === 'confirmed').length, 0),
      files_removed: run && run.json && run.json.scratch ? (run.json.scratch.removed || []).length : 0,
      content_complete: run ? run.json.content_complete : null,
      cleanup_complete: run ? run.json.cleanup_complete : null,
      status: run ? run.json.status : 'unsettled',
    }
    if (cleanup.status === 'partial') log(`Cleanup is partial: ${JSON.stringify((run.json && run.json.partial_reasons) || [])}`)
    // R331: the verified run report. Every stage has settled, so the report describes this invocation:
    // completeness, gaps, current placeholders, timings and exact per-run usage. A failed render leaves the
    // run partial and names why; it never substitutes an agent's summary for the report.
    const rendered = await Z.ledgerReport('report the run from the ledger')
    let facts = null
    if (!rendered.ok) {
      log(`The run report failed: ${rendered.error}`)
    } else {
      facts = rendered.json
      const c = facts.completeness || {}
      const u = facts.usage || {}
      log(`Run report: settled ${c.settled}, content ${c.content_complete}, quality ${c.quality_complete}, cleanup ${c.cleanup_complete}, complete ${c.complete}; ${(facts.gaps || []).length} gap(s); placeholders ${facts.placeholders && facts.placeholders.count}; usage ${u.known_runs} run(s) known, ${u.missing_runs} missing. Report: ${OUT}\\run-report.md`)
    }
    const usage = facts ? facts.usage : null
    return {
      status: facts ? (facts.completeness && facts.completeness.complete ? 'complete' : 'partial') : (cleanup.status === 'complete' ? 'partial' : cleanup.status),
      cleanup,
      usage,
      report: facts ? {
        json: (facts.report && facts.report['run-report.json'] && facts.report['run-report.json'].path) || `${OUT}\\run-report.json`,
        markdown: (facts.report && facts.report['run-report.md'] && facts.report['run-report.md'].path) || `${OUT}\\run-report.md`,
        completeness: facts.completeness,
        counts: facts.counts,
        gaps: facts.gaps,
        placeholders: facts.placeholders,
        timing: facts.timing,
        resume: facts.resume,
        bridge: facts.bridge,
        publications: facts.publications,
      } : null,
      error: facts ? undefined : rendered.error,
    }
  },
})
// ==== end module zt-dsh-cleanup ====
