// ==== module zt-dsh-cleanup v11 ====
// R231/R322–R330: the run's cleanup is settled from ledger evidence, never from a workspace listing's titles.
// v11 (2026-10-23): no model reads, relays or decides any cleanup fact. One Haiku agent makes three calls:
// `zt-digest cleanup-run -Step plan` records the evidence already on disk and writes the plan file (every
// recorded id still to delete, every recorded workspace not yet removed); the bridge's `deepseek_settle` runs
// that plan in the MCP server that holds the conversations open (it closes each first) and writes the results
// file; `cleanup-run -Step apply` records every fact and settles every ready cleanup unit through its checked
// verifier. No id list passes through the agent: it sees two paths and three short lines. The stage had spent
// a Haiku agent per 40 conversations, per 8 workspaces and (until v10) per cleanup unit, about 150 agents one
// after another on ebrill2001modern trial 1. A workspace that still lists a conversation the run did not record
// is kept and reported (that member is the user's); a refused deletion stays a retryable provider fact.
// The run report (#85, R331–R346) is rendered here, after cleanup settles: every stage of the run has
// finished, so the report describes the whole invocation. Its usage section is the exact per-run usage
// the ledger recorded from each run's own result; this module never reads or differences the account-wide
// usage ledger, because the difference of two rolling totals is not a run's cost (R344).
defineModule({
  id: 'zt-dsh-cleanup', version: '11', requires: ['zt-digest-lib'],
  summary: 'ZT digest: settle every recorded DeepSeek conversation and workspace from ledger evidence, then render the verified run report.',
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
      properties: { plan: { type: 'string' }, settle: { type: 'string' }, ok: { type: 'boolean' }, output: { type: 'string' } },
      required: ['ok', 'output'],
    }
    const step = (name) => `${Z.DIGEST} cleanup-run -Out "${OUT}" -Step ${name}${name === 'apply' ? ' -Owner "cleanup-run"' : ''}`
    const got = await dsmpSlot(() => Z.zAgent(
      `Settle the DeepSeek Harness conversations and workspaces of one digest run. A script decides every id; you make three calls and copy short lines. Never delete, list or remove anything yourself, and never call deepseek_delete or deepseek_remove_workspace.
${Z.RUNNER}
1. Run this command once in the foreground with timeout_ms: ${Z.COMMAND_TIMEOUT_MS} and read its last line (JSON; it changes no Card):
   ${step('plan')}
2. Load the tool with one ToolSearch call: query "select:mcp__deepseek__deepseek_settle". Call deepseek_settle once with plan_file "${PLAN}" and results_file "${RESULTS}". Put its answer, as one line, in settle. If the tool is missing or fails, put the error in settle and skip to step 4.
3. Run this command once with timeout_ms: ${Z.COMMAND_TIMEOUT_MS}; it records the results and settles the cleanup units (it changes no Card):
   ${step('apply')}
   Return ok (true when it exits without an error) and output = its last line exactly. Stop here.
4. Only when step 2 failed: run ${Z.DIGEST} cleanup-finish -Out "${OUT}" -Ready -Owner "cleanup-run" once with timeout_ms: ${Z.COMMAND_TIMEOUT_MS} (it settles the units whose conversations are already gone) and return ok and output = its last line exactly.
Return plan = the last line of step 1.`,
      { label: 'cleanup: settle the run', phase: 'Cleanup', model: 'haiku', schema: SETTLED },
    ))
    const line = got && got.ok ? zdLastJson(got.output) : null
    const ran = line && line.kind === 'cleanup_run' ? line : null
    const ready = ran ? ran.ready : line && line.kind === 'cleanup_ready' ? line : null
    if (!ran) log(`Cleanup: the provider batch did not run (${String((got && (got.settle || got.output)) || 'no answer').slice(0, 240)}).`)
    else log(`Cleanup: deleted ${ran.deleted}, already gone ${ran.absent}, failed ${ran.failed_count}; workspaces removed ${ran.workspaces_removed}, kept ${ran.workspaces_kept}${ran.unknown_count ? `; ${ran.unknown_count} conversation(s) the run did not record stay: ${JSON.stringify(ran.unknown)}` : ''}${ran.unrecorded_count ? `; ${ran.unrecorded_count} fact(s) not recorded: ${JSON.stringify(ran.unrecorded)}` : ''}.`)
    const settled = []
    if (ready) {
      const open = (ready.retry || []).concat(ready.failed || [])
      log(`Cleanup: ${ready.done} of ${ready.claimed} claimed unit(s) settled (${ready.partial} partial); ${ready.retry_count} retryable, ${ready.failed_count} failed${open.length ? `: ${JSON.stringify(open.slice(0, 4))}` : ''}.`)
      for (const unit of open) settled.push({ unit: unit.unit, status: unit.state === 'retry' ? 'retry' : 'failed', error: unit.cause })
      if (ready.book && ready.book.unit) settled.push(ready.book.ok ? { unit: ready.book.unit, status: ready.book.status || 'done', json: ready.book } : { unit: ready.book.unit, status: 'retry', error: ready.book.cause })
    }
    const run = settled.find((s) => s.unit === 'cleanup/book@1' && s.json) || null
    const cleanup = {
      provider: ran ? { deleted: ran.deleted, absent: ran.absent, failed: ran.failed_count, workspaces_removed: ran.workspaces_removed, workspaces_kept: ran.workspaces_kept, unknown: ran.unknown_count, unrecorded: ran.unrecorded_count } : null,
      settled: ready ? { claimed: ready.claimed, done: ready.done, partial: ready.partial, retry: ready.retry_count, failed: ready.failed_count } : null,
      units: settled.map((s) => ({ unit: s.unit, status: s.status, cleanup_complete: s.json ? s.json.cleanup_complete : null, content_complete: s.json ? s.json.content_complete : null, error: s.error })),
      workspace_removed: ran ? ran.workspaces_kept === 0 && !(ran.failed || []).some((f) => f.workspace) : false,
      files_removed: run && run.json ? Number(run.json.scratch_removed) || 0 : 0,
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
