// ==== module zt-dsh-source v1 ====
// The DeepSeek source: with dshSource "auto", a Haiku probe asks each source of DeepSeek V4.1 Flash for one
// short answer and the run uses the first that gives it, the official source before OpenCode Go. Fails the
// stage when neither answers. Also logs the DeepSeek efforts raised to high.
defineModule({
  id: 'zt-dsh-source', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: choose the DeepSeek source (probe official, then OpenCode Go) and report the model.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const A = p.args
    const Z = zdMake(ctx, { args: A, mode: p.mode })
    if (Z.raised.length) log(`DeepSeek efforts raised to high (the user wants at least medium; V4.1 Flash has no medium): ${Z.raised.join(', ')}.`)
    const { source, model } = zdDefaultModel(A)
    if (source !== 'auto') {
      log(`DeepSeek source: ${source} (${model}).`)
      return { source, model }
    }
    const probeDir = `${Z.OUT}\\dsh-probe`
    const probe = await Z.zAgent(
      `You check which DeepSeek Harness source of DeepSeek V4.1 Flash answers now. Do exactly these steps; keep your messages short.
Load the tools with one ToolSearch call: query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_delete,mcp__deepseek__deepseek_remove_workspace".
1. One PowerShell call: New-Item -ItemType Directory -Force -Path "${probeDir}" | Out-Null
2. Try the sources in this order and stop at the first that answers:
   a. official: model "${ZD_DSH_SOURCES.official}"
   b. opencode: model "${ZD_DSH_SOURCES.opencode}"
   For each: deepseek_run with cwd "${probeDir}", model as given, reasoning_effort "high", prompt "只回复 ok", wait_seconds 180. If the status is "running", deepseek_wait once (wait_seconds 180). The source answers when the status is "finished" and the answer contains "ok". An error, a "running" status after the wait, or any other answer means it does not: record the error or status text for it. Then deepseek_delete that session_id.
3. deepseek_remove_workspace with cwd "${probeDir}" and delete_conversations true. Then one PowerShell call: Remove-Item -LiteralPath "${probeDir}" -Recurse -Force -ErrorAction SilentlyContinue
Return source ("official", "opencode", or "none") and errors (one short line per source that failed, else "").`,
      {
        label: 'choose DeepSeek source', phase: 'Prepare', model: 'haiku',
        schema: { type: 'object', properties: { source: { type: 'string', enum: ['official', 'opencode', 'none'] }, errors: { type: 'string' } }, required: ['source'] },
      },
    )
    if (!probe || !ZD_DSH_SOURCES[probe.source]) {
      log(`No DeepSeek source answered: ${(probe && probe.errors) || 'no report from the probe'}`)
      throw new Error('neither the DeepSeek official source nor OpenCode Go answered')
    }
    const chosen = ZD_DSH_SOURCES[probe.source]
    log(`DeepSeek source: ${probe.source} (${chosen})${probe.errors ? `; ${probe.errors}` : ''}.`)
    return { source: probe.source, model: chosen, errors: probe.errors || '' }
  },
})
// ==== end module zt-dsh-source ====
