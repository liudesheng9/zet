// ==== module zt-dsh-cleanup v2 ====
// R231/R322–R330: the run's cleanup is settled from ledger evidence, never from a workspace listing's
// titles. The module reads every resource this run recorded, asks one Haiku agent to delete exactly those
// conversation ids (an unknown member is reported, never removed wholesale), records each provider fact
// in the ledger, and settles every cleanup unit through its checked verifier. The accepted answers were
// retained when the ids were recorded; a disposable model folder is removed by the verifier only after
// its accepted plan sits under the durable evidence path. A partial cleanup keeps the run partial; a
// provider fault stays retryable and rebuilds no content.
defineModule({
  id: 'zt-dsh-cleanup', version: '2', requires: ['zt-digest-lib'],
  summary: 'ZT digest: settle every recorded DeepSeek conversation and workspace from ledger evidence, then report usage.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    usageBefore: { type: 'any', description: 'the usage reading from Prepare (mode full)' },
    usageBeforeLink: { type: 'any', description: 'the usage reading from the link preparation (mode link)' },
    groups: { type: 'any', description: 'the group count (digest runs in mode full)' },
    linkRuns: { type: 'any', description: 'the link run reports, for the per-run averages' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const OUT = Z.OUT
    const listed = await Z.resourceList('cleanup: list the recorded resources')
    if (!listed.ok) return { status: 'failed', cleanup: null, usage: null, error: listed.error }
    const resources = listed.resources || []
    const workspaces = [...new Set(resources.map((r) => r.workspace && r.workspace.id).filter(Boolean))]
    let report = { workspaces: [] }
    if (resources.length) {
      const plan = resources.map((r) => ({
        resource: r.id,
        cleanup_unit: r.cleanup_unit,
        workspace: r.workspace,
        conversations: (r.conversations || []).map((c) => ({ id: c.id, lost: !!c.lost })),
      }))
      const asked = await Z.zAgent(
        `Settle the DeepSeek Harness (DSH) conversations this digest run recorded. Work only with the recorded ids below: never delete a conversation because its title looks like this run's, and never delete one that is not recorded here.
Load the tools with one ToolSearch call: query "select:mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_delete,mcp__deepseek__deepseek_remove_workspace,mcp__deepseek__deepseek_usage".
For each workspace below, in order:
1. deepseek_list with cwd = that workspace path. If the workspace is not found, treat every recorded conversation of it as absent.
2. For each recorded conversation id of that workspace:
   - Listed and "running": deepseek_wait (session_id, wait_seconds 600) once, then delete it.
   - Listed and finished or error: deepseek_delete (session_id). deleted="confirmed". Its state and outcome are what deepseek_list reported.
   - Not listed: deleted="already_absent"; state="absent".
   - deepseek_delete fails: deleted="failed" and record the error text in outcome. Never retry it more than twice.
   Never archive a conversation. Count every conversation the listing shows that is NOT recorded for that workspace as "unknown" (id and title, if shown): those belong to the user and must stay.
3. When no recorded id of that workspace remains and there is no unknown member, deepseek_remove_workspace with cwd = that workspace path and delete_conversations true. removed=true only when it succeeded. Otherwise removed=false.
Recorded resources (JSON):
${JSON.stringify(plan)}
Then deepseek_usage with days=1 and sum each field over the rows for the last day listed (usage_after).
Return one entry per workspace with its id, removed, unknown (the unrecorded members) and conversations (id, state, outcome, deleted), plus usage_after.`,
        {
          label: 'cleanup DeepSeek', phase: 'Cleanup', model: 'haiku',
          schema: {
            type: 'object',
            properties: {
              workspaces: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    id: { type: 'string' },
                    removed: { type: 'boolean' },
                    unknown: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, title: { type: 'string' } }, required: ['id'] } },
                    conversations: {
                      type: 'array',
                      items: {
                        type: 'object',
                        properties: { id: { type: 'string' }, state: { type: 'string' }, outcome: { type: 'string' }, deleted: { type: 'string' } },
                        required: ['id', 'deleted'],
                      },
                    },
                  },
                  required: ['id', 'removed', 'conversations'],
                },
              },
              usage_after: ZD_USAGE,
            },
            required: ['workspaces'],
          },
        },
      )
      if (!asked) return { status: 'failed', cleanup: null, usage: null, error: 'no report from the cleanup agent' }
      report = asked
      // Record the provider's own facts, then settle each cleanup unit through its checked verifier.
      const seen = new Map()
      for (const workspace of asked.workspaces || []) {
        for (const conversation of workspace.conversations || []) seen.set(conversation.id, { workspace: workspace.id, ...conversation })
      }
      for (const resource of resources) {
        for (const conversation of resource.conversations || []) {
          const said = seen.get(conversation.id) || { state: 'absent', deleted: 'already_absent', outcome: '' }
          const state = String(said.state || 'absent').toLowerCase()
          await Z.observeResource(`cleanup: terminal ${conversation.id}`, {
            op: 'terminal', resource: resource.id, conversation: conversation.id,
            state: ['finished', 'error', 'cancelled', 'failed', 'absent'].includes(state) ? state : 'finished',
            outcome: String(said.outcome || state).slice(0, 180),
          })
          const deleted = String(said.deleted || '').toLowerCase()
          if (deleted === 'failed') {
            await Z.observeResource(`cleanup: failed deletion ${conversation.id}`, {
              op: 'delete_failed', resource: resource.id, conversation: conversation.id,
              code: 'provider', message: String(said.outcome || 'the provider refused the deletion').slice(0, 300),
            })
          } else {
            await Z.observeResource(`cleanup: delete ${conversation.id}`, {
              op: 'deleted', resource: resource.id, conversation: conversation.id,
              result: deleted === 'confirmed' ? 'confirmed' : 'already_absent',
            })
          }
        }
        const workspace = (asked.workspaces || []).find((w) => w.id === resource.workspace.id) || { removed: false, unknown: [] }
        await Z.observeResource(`cleanup: workspace ${resource.workspace.id}`, {
          op: 'listing', resource: resource.id,
          conversations: (workspace.conversations || []).map((c) => ({ id: c.id, state: String(c.state || 'unknown') })),
          unknown: workspace.unknown || [],
        })
        const b = p.usageBefore || p.usageBeforeLink
        const usage = asked.usage_after && b
          ? {
              input_cache_miss: Math.max(0, asked.usage_after.input_cache_miss - b.input_cache_miss),
              cached_read: Math.max(0, asked.usage_after.cached_read - b.cached_read),
              output: Math.max(0, asked.usage_after.output - b.output),
              calls: Math.max(0, asked.usage_after.calls - b.calls),
            }
          : { calls: 0 }
        await Z.observeResource(`cleanup: usage ${resource.id}`, { op: 'usage', resource: resource.id, usage })
        await Z.observeResource(`cleanup: workspace removed ${resource.workspace.id}`, {
          op: 'workspace', resource: resource.id, removed: workspace.removed === true,
        })
      }
    }
    // Every cleanup unit this run recorded: claim the ready ones in bounded rounds, settle each through
    // its own checked verifier, and record a retryable fault when a provider fact still blocks it.
    const settled = []
    for (let round = 0; round < 3; round++) {
      const claimed = await Z.claim(`cleanup: claim round ${round + 1}`, 'cleanup', 8, 0)
      const units = (claimed.units || []).slice().sort((a, b) => (a.id === 'cleanup/book@1') - (b.id === 'cleanup/book@1'))
      if (!units.length) break
      for (const unit of units) {
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
    const run = settled.find((s) => s.unit === 'cleanup/book@1' && s.json) || settled.find((s) => s.json) || null
    const cleanup = {
      resources: resources.length,
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
    let usage = null
    const b = p.usageBefore || p.usageBeforeLink
    if (b && report.usage_after) {
      const a = report.usage_after
      const miss = a.input_cache_miss - b.input_cache_miss, hit = a.cached_read - b.cached_read
      const runs = Math.max(1, (p.mode === 'full' ? (p.groups || 0) : 0) + (p.linkRuns || []).length)
      usage = {
        input_cache_miss: miss, cached_read: hit, output: a.output - b.output,
        calls: a.calls - b.calls, cost: Math.round((a.cost - b.cost) * 10000) / 10000,
        cache_hit_rate: miss + hit > 0 ? Math.round((hit / (miss + hit)) * 1000) / 1000 : null,
        per_run: { input: Math.round((miss + hit) / runs), output: Math.round((a.output - b.output) / runs), calls: Math.round(((a.calls - b.calls) / runs) * 10) / 10 },
      }
      log(`DeepSeek usage for this batch: ${JSON.stringify(usage)}`)
    }
    return { cleanup, usage }
  },
})
// ==== end module zt-dsh-cleanup ====
