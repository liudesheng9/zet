// ==== module zt-term-link v3 ====
// S24 (#167); v3 (#172): the run's whole-book Term-link pass as one DeepSeek drive job that a Haiku
// manager starts: `term-steps.ps1 -Step link` plans, applies under the claim (or, when the unit is
// blocked by an incomplete Term-card book, links the published prefix and leaves the unit open), and
// plans again so the report shows the second pass's changes (0 expected). The finalization's own
// correction round still uses the shared zt-term-link-lib.
defineModule({
  id: 'zt-term-link', version: '3', requires: ['@dsmp'],
  summary: 'Term link: one DeepSeek drive job runs the deterministic Term-link pass (plan, apply, complete, second plan) and reports its result.',
  params: {
    out: { type: 'string', required: true, description: 'the prepared run\'s staging folder' },
    key: { type: 'string', required: true, description: 'the book\'s Citation key' },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    occurrences: { type: 'string', enum: ['first', 'all'], default: 'first', description: 'the occurrence policy (args.termLinkOccurrences)' },
    script: { type: 'string', default: 'E:\\zet\\scripts\\book-digest\\term-steps.ps1', description: 'the Term step script the drive job runs' },
  },
  run: async (ctx, p) => {
    // The kernel passes only the params a blueprint gives; the script path has one default.
    const DEFAULT_SCRIPT = 'E:\\zet\\scripts\\book-digest\\term-steps.ps1'
    const OUT = String(p.out).replace(/[\\/]+$/, '')
    const tail = (text) => String(text || '').trim().slice(0, 400)
    const command = `pwsh -NoProfile -File "${p.script || DEFAULT_SCRIPT}" -Step link -Out "${OUT}" -Occurrences ${p.occurrences}`
    const job = { id: 'drive-link', files: [], prompt: `在 PowerShell 里原样运行下面这一条命令，不改写它，不运行别的命令，不修改任何文件。命令结束后，只回复它输出的最后一行（一行 JSON），不加任何别的字。\n\n${command}`, skills: [], model: 'deepseek-official/deepseek-flash', effort: 'high' }
    const SCHEMA = { type: 'object', properties: { answer: { type: 'string' }, deleted: { type: 'boolean' } }, required: ['answer'] }
    const r = await dsmpSlot(() => ctx.agent(`You are a DeepSeek subagent manager for the ZET Term-link pass. You never run commands yourself. Keep your messages short.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_delete".

1. Start this job: mcp__deepseek__deepseek_run with cwd "${OUT}\\term-link", prompt ${JSON.stringify(job.prompt)}, model "${job.model}", reasoning_effort "high", wait_seconds 0.
2. Wait with mcp__deepseek__deepseek_wait (wait_seconds ${DSMP.pollSeconds}) until it ends. A question: answer "按任务说明自行决定，不需要询问。". An approval for that command: allow.
3. If it failed or gave an empty answer, run it ONCE more in a new conversation.
4. Delete the conversation with mcp__deepseek__deepseek_delete (never archive).
5. Return answer (its final answer verbatim) and deleted.`, { role: 'manager', label: `${ctx.stage}: drive link`, phase: ctx.phase, schema: SCHEMA }))
    const line = String((r && r.answer) || '').trim().split('\n').filter((l) => l.trim().startsWith('{')).pop()
    let json = null
    try { json = line ? JSON.parse(line) : null } catch { json = null }
    if (!json || json.ok !== true) return { status: 'failed', book: p.key, error: tail((json && json.error) || (r && r.answer) || 'no answer') }
    log(`Term link: ${json.status}; ${(json.counts && json.counts.cards_changed) || 0} Card(s) changed; second plan changes ${(json.second && json.second.cards_changed) ?? '?'} Card(s).`)
    return { status: json.status, book: p.key, unit: json.unit || null, counts: json.counts || null, second: json.second || null, reason: json.reason || null, deepseek: { deleted: r && r.deleted ? 1 : 0 } }
  },
})
// ==== end module zt-term-link ====
