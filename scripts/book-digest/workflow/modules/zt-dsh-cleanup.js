// ==== module zt-dsh-cleanup v13 ====
// R231/R322–R330: the run's cleanup is settled from ledger evidence, never from a workspace listing's titles.
// v11 (2026-10-23): no model reads, relays or decides any cleanup fact. `zt-digest cleanup-run -Step plan` records
// the evidence already on disk and writes the plan file (every recorded id still to delete, every recorded workspace
// not yet removed); the bridge's `deepseek_settle` runs that plan in the MCP server that holds the conversations open
// (it closes each first) and writes the results file; `cleanup-run -Step apply` records every fact and settles every
// ready cleanup unit through its checked verifier. The run report is rendered after cleanup settles.
// v12 (#176, docs/workflow-module-structure-proposal.md): the plan, the apply (with the `cleanup-finish -Ready`
// fallback when the settle tool is missing or its results are unusable) and the run report are book-steps.ps1 steps
// run by DeepSeek drive jobs. One Haiku manager makes the one call no script can: `deepseek_settle` with the plan
// file and the results file. No Claude agent runs a command, so no id list passes through an agent: it sees two paths
// and one answer.
// A workspace that still lists a conversation the run did not record is kept and reported (that member is the
// user's); a refused deletion stays a retryable provider fact.
// The run report (#85, R331–R346) is rendered here, after cleanup settles: every stage of the run has finished, so the
// report describes the whole invocation. Its usage section is the exact per-run usage the ledger recorded from each
// run's own result; this module never reads or differences the account-wide usage ledger, because the difference of
// two rolling totals is not a run's cost (R344).
defineModule({
  id: 'zt-dsh-cleanup', version: '13', requires: ['zt-digest-lib'],
  summary: 'ZT digest: settle every recorded DeepSeek conversation and workspace from ledger evidence (one settle call, two steps), then render the verified run report.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const OUT = Z.OUT
    const PLAN = `${OUT}\\cleanup\\settle-plan.json`
    const RESULTS = `${OUT}\\cleanup\\settle-results.json`
    const SETTLED = {
      type: 'object',
      properties: { settle: { type: 'string' }, ok: { type: 'boolean' } },
      required: ['settle'],
    }
    // 1. The plan: every recorded id still open and every recorded workspace not yet removed.
    const planned = await Z.bookStep('cleanup: plan', 'cleanup-start')
    if (!planned || planned.ok !== true) {
      log(`Cleanup: the plan did not run (${(planned && planned.error) || 'no output'}).`)
      return { status: 'partial', cleanup: { status: 'unsettled' }, error: (planned && planned.error) || 'the cleanup plan did not run' }
    }
    // 2. The one provider call no script makes: `deepseek_settle` runs the plan inside the MCP server that holds the
    //    conversations open. The manager copies one short line; it deletes nothing and reads no id list.
    const got = await dsmpSlot(() => Z.zAgent(
      `Settle the DeepSeek Harness conversations and workspaces of one digest run. A script decides every id and has already written the plan. You make ONE call and copy its short answer. Never delete, list or remove anything yourself, and never call deepseek_delete or deepseek_remove_workspace.
1. Load the tool with one ToolSearch call: query "select:mcp__deepseek__deepseek_settle". Call deepseek_settle once with plan_file "${PLAN}" and results_file "${RESULTS}". Put its answer, as one line, in settle. If the tool is missing or fails, put the error in settle.
2. Return settle = that one line, and ok = true when the call answered.`,
      { label: 'cleanup: settle the run', phase: 'Cleanup', model: 'haiku', schema: SETTLED },
    ))
    if (!got || !got.ok) log(`Cleanup: the provider batch did not run (${String((got && got.settle) || 'no answer').slice(0, 240)}).`)
    // 3. Record the results and settle every ready cleanup unit; without usable results the units whose conversations
    //    are already gone still settle through `cleanup-finish -Ready`.
    const applied = await Z.bookStep('cleanup: apply', 'cleanup-apply')
    if (!applied || applied.ok !== true) {
      log(`Cleanup: the settlement did not run (${(applied && applied.error) || 'no output'}).`)
      return { status: 'partial', cleanup: { status: 'unsettled' }, error: (applied && applied.error) || 'the cleanup settlement did not run' }
    }
    // #180: the apply step answers the provider and settle facts as flat counts.
    const n = (v) => Number(v || 0)
    const ran = applied.fallback !== true ? true : null
    if (!ran) log(`Cleanup: the provider batch did not run (${String(applied.provider_error || 'the settle results were not usable').slice(0, 240)}).`)
    else log(`Cleanup: deleted ${n(applied.deleted)}, already gone ${n(applied.absent)}, failed ${n(applied.failed)}; workspaces removed ${n(applied.workspaces_removed)}, kept ${n(applied.workspaces_kept)}${n(applied.unknown) ? `; ${n(applied.unknown)} conversation(s) the run did not record stay` : ''}${n(applied.unrecorded) ? `; ${n(applied.unrecorded)} fact(s) not recorded` : ''}.`)
    log(`Cleanup: ${n(applied.ready_done)} of ${n(applied.ready_claimed)} claimed unit(s) settled (${n(applied.ready_partial)} partial); ${n(applied.ready_retry)} retryable, ${n(applied.ready_failed)} failed.`)
    const cleanup = {
      provider: ran ? { deleted: n(applied.deleted), absent: n(applied.absent), failed: n(applied.failed), workspaces_removed: n(applied.workspaces_removed), workspaces_kept: n(applied.workspaces_kept), unknown: n(applied.unknown), unrecorded: n(applied.unrecorded) } : null,
      settled: { claimed: n(applied.ready_claimed), done: n(applied.ready_done), partial: n(applied.ready_partial), retry: n(applied.ready_retry), failed: n(applied.ready_failed) },
      units: [],
      workspace_removed: ran ? n(applied.workspaces_kept) === 0 : false,
      files_removed: 0,
      content_complete: null,
      cleanup_complete: n(applied.ready_failed) === 0 ? true : null,
      status: applied.book_status || (n(applied.ready_failed) ? 'partial' : 'complete'),
    }
    // 4. R331: the verified run report. Every stage has settled, so the report describes this invocation:
    //    completeness, gaps, current placeholders, timings and exact per-run usage. A failed render leaves the run
    //    partial and names why; it never substitutes an agent's summary for the report.
    const facts = await Z.bookStep('report the run from the ledger', 'cleanup-report')
    let report = null
    if (!facts || facts.ok !== true) {
      log(`The run report failed: ${(facts && facts.error) || 'no output'}`)
    } else {
      // #180: the report step answers its verified facts as counts and paths; the whole report is its detail file
      // and the rendered run-report.json / run-report.md under the staging folder.
      const u = String(facts.usage || '').split(',')
      const usage = { status: u[0] || 'unknown', source: u[1] || 'unknown', known_runs: n(u[2]), missing_runs: n(u[3]), missing: [], totals: { input_tokens: n(u[4]), output_tokens: n(u[5]) } }
      report = {
        json: `${OUT}\\run-report.json`,
        markdown: `${OUT}\\run-report.md`,
        completeness: { settled: facts.settled === true, content_complete: facts.content_complete === true, quality_complete: facts.quality_complete === true, cleanup_complete: facts.cleanup_complete === true, complete: facts.complete === true },
        counts: { declared: n(facts.declared), done: n(facts.done), noop: 0, failed: n(facts.failed), open: n(facts.open) },
        gaps: n(facts.gaps),
        placeholders: { count: n(facts.placeholders) },
        timing: null,
        resume: { available: facts.resume_available === true },
        bridge: null,
        publications: n(facts.publications),
        usage,
      }
      log(`Run report: settled ${report.completeness.settled}, content ${report.completeness.content_complete}, quality ${report.completeness.quality_complete}, cleanup ${report.completeness.cleanup_complete}, complete ${report.completeness.complete}; ${report.gaps} gap(s); placeholders ${report.placeholders.count}; usage ${usage.known_runs} run(s) known, ${usage.missing_runs} missing. Report: ${OUT}\\run-report.md`)
    }
    const usage = report ? report.usage : null
    return {
      status: report ? (report.completeness.complete ? 'complete' : 'partial') : (cleanup.status === 'complete' ? 'partial' : cleanup.status),
      cleanup,
      usage,
      report,
      error: report ? undefined : ((facts && facts.error) || 'the run report did not render'),
    }
  },
})
// ==== end module zt-dsh-cleanup ====

