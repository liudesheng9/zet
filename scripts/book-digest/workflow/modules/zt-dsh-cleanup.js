// ==== module zt-dsh-cleanup v1 ====
// After the link pass: a Haiku agent deletes any DeepSeek conversation a manager left in the book's workspace,
// removes the workspace (the folder stays), deletes the per-run scratch files, and reads the usage ledger; the
// module reports the batch's DeepSeek usage against the reading taken at Prepare (or at the link preparation).
defineModule({
  id: 'zt-dsh-cleanup', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: delete leftover DeepSeek conversations, remove the workspace and scratch files, report usage.',
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
    const cleanup = await Z.zAgent(
      `Clean up after a DeepSeek Harness batch. Load tools with ToolSearch query "select:mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_delete,mcp__deepseek__deepseek_remove_workspace,mcp__deepseek__deepseek_usage".
1. deepseek_list with cwd "${OUT}". The managers deleted their own conversations; deepseek_delete every conversation still listed that is not running (a manager that failed left it), and count them. Never archive a conversation.
2. When none is running: deepseek_remove_workspace with cwd "${OUT}" and delete_conversations true (the folder and its files stay on disk); workspace_removed=true when it succeeds.
3. Delete the per-run scratch files with one PowerShell call and report the count it prints:
   $f = Get-ChildItem -LiteralPath "${OUT}" -File | Where-Object { $_.Name -like '*calls*.json' -or $_.Name -like 'snapshot-g*.md' -or $_.Name -like 'link-g*.md' -or $_.Name -like 'allow-g*.txt' }; $f | Remove-Item; $f.Count
   Delete nothing else (keep the book, prefixes, manifest, cards-all.md and owned-g*.txt).
4. deepseek_usage with days=1; sum each field over the rows for the last day listed.
Return late_deleted, workspace_removed, files_removed, usage_after.`,
      {
        label: 'cleanup DeepSeek', phase: 'Cleanup', model: 'haiku',
        schema: {
          type: 'object',
          properties: { late_deleted: { type: 'number' }, workspace_removed: { type: 'boolean' }, files_removed: { type: 'number' }, usage_after: ZD_USAGE },
          required: ['late_deleted', 'workspace_removed', 'files_removed'],
        },
      },
    )
    let usage = null
    const b = p.usageBefore || p.usageBeforeLink
    if (b && cleanup && cleanup.usage_after) {
      const a = cleanup.usage_after
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
