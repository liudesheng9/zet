// ==== module zt-issue-relay v1 ====
// Develops GitHub issues one at a time, each in its own DeepSeek Harness conversation that a Haiku manager drives.
// The DeepSeek agent works on its own (it has a shell). When it needs something it cannot do, it asks: a
// question or approval (status needs_response) or a turn that ends "STATUS: ASK". Then the main agent (the
// executor role) does what was asked, and the manager sends the result back so the DeepSeek agent continues.
// An issue ends on "STATUS: DONE" or "STATUS: NEEDS-HUMAN", or when a declared limit is reached. Then the manager
// deletes the conversation (never archives it) and a runner records the repository and deploy state.
// Issues run in order, never in parallel: they share one checkout, one service, and one installed binary.
// DeepSeek Subagent Managed Protocol: managers are Haiku and take a slot of the script-wide pool (dsmpSlot);
// each manager drives exactly one DeepSeek conversation; deletions go into dsmpLog; the workspace is removed at
// the end only when it holds no conversations (the user's own conversations there are kept).

const ZIR_STATE = {
  type: 'object',
  properties: {
    session_id: { type: 'string', description: 'the DeepSeek session_id ("" if none was started)' },
    status: { type: 'string', enum: ['running', 'finished', 'needs_response', 'failed', 'cancelled', 'error'] },
    answer: { type: 'string', description: 'the last 6000 characters of the agent answer, verbatim ("" if none)' },
    pending: {
      type: 'array', description: 'each pending item when status is needs_response',
      items: { type: 'object', properties: {
        request_id: { type: 'string' },
        kind: { type: 'string', description: '"question" or "approval"' },
        detail: { type: 'string', description: 'the whole pending item as JSON text, verbatim' } },
        required: ['request_id', 'kind', 'detail'] },
    },
    error: { type: 'string', description: 'the error text of a failed call, else ""' },
    context_pct: { type: 'number', description: 'context.used_pct of the last result, else 0' },
    waits: { type: 'integer', description: 'how many deepseek_wait calls you made' },
  },
  required: ['session_id', 'status', 'answer', 'pending', 'error'],
}

const ZIR_DELETED = {
  type: 'object',
  properties: { deleted: { type: 'boolean' }, cancelled: { type: 'boolean' }, note: { type: 'string' } },
  required: ['deleted', 'note'],
}

const ZIR_EXEC = {
  type: 'object',
  properties: {
    outcome: { type: 'string', enum: ['done', 'refused', 'failed'], description: 'done: you did what was asked; refused: it needs the user; failed: you tried and it did not work' },
    result: { type: 'string', description: 'what the DeepSeek agent needs: what you ran, exit codes, key output, paths of long logs; at most 3500 characters' },
    answers: { type: 'array', description: 'for a question: one answer per question id', items: { type: 'object', properties: { id: { type: 'string' }, text: { type: 'string' } }, required: ['id', 'text'] } },
    decision: { type: 'string', enum: ['allow', 'reject', 'none'], description: 'for an approval: allow or reject the tool call; else none' },
  },
  required: ['outcome', 'result', 'answers', 'decision'],
}

// The declared defaults; the kernel passes only the params a blueprint gives.
const ZIR_DEFAULTS = { skills: [], maxRounds: 40, waitSeconds: 900, waitsPerManager: 12, maxWaitCalls: 8, managerRole: 'manager', executorRole: 'executor', runnerRole: 'runner', cleanupWorkspace: true }

const ZIR_TOOLS = 'select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_prompt,mcp__deepseek__deepseek_cancel,mcp__deepseek__deepseek_delete'

// The last STATUS line of an answer: DONE, NEEDS-HUMAN or ASK (else null).
function zirStatus(answer) {
  const all = [...String(answer || '').matchAll(/STATUS:\s*(DONE|NEEDS-HUMAN|ASK)\b[ \t]*[—:-]*[ \t]*([^\n]*)/g)]
  if (!all.length) return { kind: null, text: '' }
  const m = all[all.length - 1]
  return { kind: m[1], text: m[2].trim() }
}

function zirManagerPrompt(op, p, waitMax) {
  const loop = `WAIT LOOP: while the status is "running", call mcp__deepseek__deepseek_wait with the session_id and wait_seconds ${p.waitSeconds}. Make at most ${waitMax} wait calls; if the status is still "running" after them, stop and return status "running".`
  const ret = `Return: session_id, status (the status of the last result; "error" if a call failed), answer (the last 6000 characters of the agent's answer, copied verbatim; "" if there is none), pending (when the status is "needs_response": one entry per pending item with its request_id, its kind, and the whole item as JSON text in detail), error (the error text, else ""), context_pct (context.used_pct of the last result, else 0), waits (how many wait calls you made).`
  const head = `You manage one DeepSeek agent conversation through the deepseek bridge. You relay only: you never answer its questions yourself, never approve or reject anything on your own, never edit files, and never run shell commands. Keep your messages short.

First load the tools: ToolSearch with query "${ZIR_TOOLS}".
`
  if (op.op === 'start') return `${head}
1. Call mcp__deepseek__deepseek_run with exactly these arguments (change nothing):
${JSON.stringify({ ...op.run, wait_seconds: p.waitSeconds }, null, 1)}
2. ${loop}
3. ${ret}`
  if (op.op === 'wait') return `${head}
The conversation is session_id ${op.session_id}. Its status was "running".
1. ${loop}
2. ${ret}`
  if (op.op === 'respond') return `${head}
The conversation is session_id ${op.session_id}. It is paused on the pending items below. The main agent decided each RESPONSE.

RESPONSES (${op.responses.length}):
${JSON.stringify(op.responses, null, 1)}

1. For each RESPONSE in order, call mcp__deepseek__deepseek_respond with session_id ${op.session_id}, its request_id, wait_seconds ${p.waitSeconds}, and its "decision" (for an approval) or its "answers" (for a question), copied verbatim. If a result is "needs_response" with a request_id that is not in RESPONSES, stop and return that state.
2. ${loop}
3. ${ret}`
  if (op.op === 'prompt') return `${head}
The conversation is session_id ${op.session_id}. Its last turn has finished.
1. Call mcp__deepseek__deepseek_prompt with session_id ${op.session_id}, wait_seconds ${p.waitSeconds}, and this prompt, copied verbatim between the markers (without the markers):
<<<PROMPT
${op.text}
PROMPT>>>
2. ${loop}
3. ${ret}`
  throw new Error(`zt-issue-relay: unknown manager op ${op.op}`)
}

async function zirManage(ctx, p, op, label) {
  return dsmpSlot(() => ctx.agent(zirManagerPrompt(op, p, p.waitsPerManager), { role: p.managerRole, schema: ZIR_STATE, label }))
}

async function zirDelete(ctx, p, sessionId, label) {
  if (!sessionId) return { deleted: false, note: 'no session' }
  const r = await dsmpSlot(() => ctx.agent(`You remove one finished DeepSeek conversation. Keep your messages short.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_cancel,mcp__deepseek__deepseek_delete".

1. Call mcp__deepseek__deepseek_wait with session_id ${sessionId} and wait_seconds 0 to read its status.
2. If the status is "running": call mcp__deepseek__deepseek_cancel with session_id ${sessionId}, then wait again (wait_seconds 60) until it is not "running".
3. Call mcp__deepseek__deepseek_delete with session_id ${sessionId}. Never call deepseek_archive. If the delete is refused, do not retry it.
Return deleted (true when the delete succeeded), cancelled (true when you cancelled it), and note (the refusal text, else "").`,
    { role: p.managerRole, schema: ZIR_DELETED, label }))
  if (!r) return { deleted: false, note: 'the manager returned nothing' }
  if (r.deleted) dsmpLog.deleted += 1
  else dsmpLog.notDeleted.push(`${sessionId}: ${r.note || 'not deleted'}`)
  return r
}

async function zirExecute(ctx, p, issue, ask, label) {
  const what = ask.kind === 'message'
    ? `It ended its turn with this message; its request is at the end ("STATUS: ASK — ..."):\n<<<ASK\n${ask.text}\nASK>>>`
    : ask.kind === 'question'
      ? `It asked this question (a pending item of the deepseek bridge; answer each question id):\n<<<ASK\n${ask.detail}\nASK>>>`
      : `It asks for approval of this tool call (a pending item of the deepseek bridge):\n<<<ASK\n${ask.detail}\nASK>>>\nDecide allow or reject. Allow only a call that serves the issue and that you would run yourself under your rules.`
  return ctx.agent(`A DeepSeek agent is developing GitHub issue #${issue.number} ("${issue.title}", ${issue.state}) of ${p.repo} in ${p.cwd}. It asked you, the main agent, for something. ${what}

Do what it asks, within your rules, then return outcome, result, answers and decision.
- For a question: put your full reply in answers (one entry per question id; text at most 3500 characters) and the same reply in result.
- For an approval: set decision and say why in result.
- For a message: put your reply in result; answers is [] and decision is "none".`,
    { role: p.executorRole, schema: ZIR_EXEC, label })
}

const zirCut = (s, n) => (String(s || '').length > n ? String(s).slice(0, n) + `\n[... cut at ${n} characters]` : String(s || ''))

async function zirIssue(ctx, p, issue, k, total) {
  const tag = `#${issue.number}`
  const lab = s => `${ctx.label}: ${tag} ${s}`
  const prompt = `${p.taskPrompt}\n\n${p.protocol}`.split('{N}').join(String(issue.number)).split('{REPO}').join(p.repo).split('{CWD}').join(p.cwd)
  const run = { cwd: p.cwd, prompt, model: p.model, reasoning_effort: p.effort, ...(p.skills && p.skills.length ? { skills: p.skills } : {}) }
  ctx.log(`issue ${tag} (${k + 1}/${total}): ${issue.title} [${issue.state}]`)
  const trail = []
  let st = await zirManage(ctx, p, { op: 'start', run }, lab('manager start'))
  let session = (st && st.session_id) || ''
  let rounds = 0, asks = 0, waitCalls = 0, resumes = 0, outcome = null, summary = ''
  while (!outcome) {
    if (!st) { outcome = 'failed'; summary = 'the manager returned nothing'; break }
    if (st.session_id) session = st.session_id
    if (st.status === 'running') {
      if (++waitCalls > p.maxWaitCalls) { outcome = 'timeout'; summary = `still running after ${p.maxWaitCalls} extra wait rounds`; break }
      st = await zirManage(ctx, p, { op: 'wait', session_id: session }, lab(`manager wait ${waitCalls}`))
      continue
    }
    if (st.status === 'finished') {
      const s = zirStatus(st.answer)
      if (s.kind === 'DONE') { outcome = 'done'; summary = s.text; break }
      if (s.kind === 'NEEDS-HUMAN') { outcome = 'needs-human'; summary = s.text; break }
    }
    if (rounds >= p.maxRounds) { outcome = 'max-rounds'; summary = `${p.maxRounds} rounds without STATUS: DONE`; break }
    rounds++
    if (st.status === 'needs_response') {
      const responses = []
      for (const item of st.pending || []) {
        asks++
        const ex = await zirExecute(ctx, p, issue, { kind: item.kind === 'approval' ? 'approval' : 'question', detail: item.detail }, lab(`main agent ${asks}`))
        trail.push({ round: rounds, kind: item.kind, outcome: ex ? ex.outcome : 'failed', result: zirCut(ex && ex.result, 600) })
        if (item.kind === 'approval') responses.push({ request_id: item.request_id, decision: ex && ex.decision === 'allow' ? 'allow' : 'reject' })
        else {
          let ids = []
          try { ids = (JSON.parse(item.detail).questions || []).map(q => q.id) } catch (e) { ids = [] }
          const given = (ex && ex.answers) || []
          const text = id => zirCut(((given.find(a => a.id === id) || given[0] || {}).text) || (ex && ex.result) || 'The main agent returned nothing; continue with your best judgment.', 3800)
          responses.push({ request_id: item.request_id, answers: (ids.length ? ids : given.map(a => a.id)).map(id => ({ id, custom: text(id) })) })
        }
      }
      if (!responses.length) { outcome = 'failed'; summary = 'needs_response without pending items'; break }
      st = await zirManage(ctx, p, { op: 'respond', session_id: session, responses }, lab(`manager respond ${rounds}`))
      continue
    }
    if (st.status === 'finished') {
      const s = zirStatus(st.answer)
      let text
      if (s.kind === 'ASK') {
        asks++
        const ex = await zirExecute(ctx, p, issue, { kind: 'message', text: st.answer }, lab(`main agent ${asks}`))
        trail.push({ round: rounds, kind: 'message', outcome: ex ? ex.outcome : 'failed', result: zirCut(ex && ex.result, 600) })
        text = `RESULT FROM THE MAIN AGENT (${ex ? ex.outcome : 'failed'}):\n${zirCut((ex && ex.result) || 'The main agent returned nothing.', 3800)}\n\n${p.continueText}`
      } else text = p.nudgeText
      st = await zirManage(ctx, p, { op: 'prompt', session_id: session, text }, lab(`manager prompt ${rounds}`))
      continue
    }
    // failed, cancelled or error: one more prompt to the same conversation, then give up
    if (session && resumes < 1) {
      resumes++
      trail.push({ round: rounds, kind: 'resume', outcome: st.status, result: zirCut(st.error, 600) })
      st = await zirManage(ctx, p, { op: 'prompt', session_id: session, text: p.resumeText }, lab(`manager resume ${rounds}`))
      continue
    }
    outcome = 'failed'; summary = `${st.status}: ${zirCut(st.error, 600)}`
  }
  const del = await zirDelete(ctx, p, session, lab('manager delete'))
  let repo = null
  try {
    repo = await wfpScript(ctx, { command: p.repoCheck.split('{N}').join(String(issue.number)), cwd: p.cwd, role: p.runnerRole, label: lab('repo state'), timeoutMs: 300000 })
  } catch (e) { repo = { ok: false, error: String(e && e.message || e) } }
  const res = {
    number: issue.number, title: issue.title, stateBefore: issue.state, outcome, summary: zirCut(summary, 1500),
    rounds, asks, session, deleted: !!(del && del.deleted), lastAnswer: outcome === 'done' ? '' : zirCut(st && st.answer, 2000),
    trail, repo,
  }
  ctx.log(`issue ${tag}: ${outcome}${summary ? ' — ' + zirCut(summary, 200) : ''}`)
  return res
}

defineModule({
  id: 'zt-issue-relay', version: '1', requires: ['@dsmp', 'script'],
  summary: 'Develop GitHub issues one at a time: a Haiku manager drives one DeepSeek conversation per issue; the main agent fulfills its asks.',
  params: {
    issues: { type: 'array', required: true, description: 'issue records {number, title, state} in the order to develop' },
    repo: { type: 'string', required: true, description: 'owner/name on GitHub' },
    cwd: { type: 'string', required: true, description: 'the checkout the DeepSeek agent works in (its workspace)' },
    model: { type: 'string', required: true, description: 'the DeepSeek model, provider/model' },
    effort: { type: 'string', required: true, description: 'the DeepSeek reasoning effort' },
    skills: { type: 'array', default: [], description: 'DeepSeek Harness skills for the first message' },
    taskPrompt: { type: 'string', required: true, description: 'the task; {N} is replaced by the issue number' },
    protocol: { type: 'string', required: true, description: 'the relay rules added after the task; {REPO} and {CWD} are replaced' },
    continueText: { type: 'string', required: true, description: 'added after a main-agent result sent by prompt' },
    nudgeText: { type: 'string', required: true, description: 'sent when a turn ends without a STATUS line' },
    resumeText: { type: 'string', required: true, description: 'sent once after a failed or cancelled turn' },
    repoCheck: { type: 'string', required: true, description: 'the command that prints the repository and deploy state; {N} is the issue number' },
    maxRounds: { type: 'number', default: 40, min: 1, max: 200, description: 'most relay rounds (asks, answers, nudges) per issue' },
    waitSeconds: { type: 'number', default: 900, min: 60, max: 3600, description: 'wait_seconds of each bridge call' },
    waitsPerManager: { type: 'number', default: 12, min: 1, max: 40, description: 'most deepseek_wait calls in one manager agent' },
    maxWaitCalls: { type: 'number', default: 8, min: 0, max: 100, description: 'most extra manager agents that only wait, per issue' },
    managerRole: { type: 'string', default: 'manager', role: true, description: 'the Haiku relay role' },
    executorRole: { type: 'string', default: 'executor', role: true, description: 'the main agent role' },
    runnerRole: { type: 'string', default: 'runner', role: true, description: 'the role that runs the repo check' },
    cleanupWorkspace: { type: 'boolean', default: true, description: 'remove the workspace from the DeepSeek Desktop at the end when it holds no conversations' },
  },
  run: async (ctx, params) => {
    const p = { ...ZIR_DEFAULTS, ...Object.fromEntries(Object.entries(params).filter(([, v]) => v !== undefined && v !== null)) }
    const issues = wfpClone(p.issues)
    const results = []
    for (const [k, issue] of issues.entries()) results.push(await zirIssue(ctx, p, issue, k, issues.length))
    let workspace = null
    if (p.cleanupWorkspace && issues.length) workspace = (await dsCleanupWorkspaces([p.cwd], { ownWorkspaces: false, label: `${ctx.label}: workspace cleanup`, phase: ctx.phase }))[0] || null
    const count = o => results.filter(r => r.outcome === o).length
    return {
      issues: results, total: results.length, done: count('done'), needsHuman: count('needs-human'),
      notDone: results.filter(r => r.outcome !== 'done').map(r => `#${r.number} ${r.outcome}`),
      repoNotOk: results.filter(r => !(r.repo && r.repo.ok)).map(r => `#${r.number}`),
      notDeleted: results.filter(r => r.session && !r.deleted).map(r => r.session),
      workspace,
    }
  },
})
// ==== end module zt-issue-relay ====
