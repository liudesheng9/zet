// ==== module zt-digest-lib v5 ====
// The shared machinery of the ZT digest workflows (docs/zt-digest-framework.md), first ported from the
// hand-written zt-book-digest.js (since removed): the settings read from the args, the digest and link managers, the
// compiler-step driver (drive), and the card layer cycle (runCards). The model compiler is zt-model-lib (the
// model layer, which each book type installs). A library module: the stage modules call zdMake(ctx, params) and use what it returns.
// Every Haiku, Sonnet, and Opus agent goes through ctx.agent, so the blueprint's roles decide the models:
// manager (Haiku), architect (Sonnet), organizer (Opus). Managers share the DeepSeek Subagent Managed
// Protocol pool (dsmpSlot): at most 8 managers alive in the whole run.
const ZD_SCRIPTS = 'E:\\zet\\scripts\\book-digest'
// The digest scripts ported to zt-digest (tools/zt-digest, one fast binary): script name → tool. zdTool(name)
// is the command that runs a script: `zt-digest <tool>` once it is ported, else pwsh with the script. The
// helper is `zt-digest call` (ZD_CALL); the old helper path zt-mcp.ps1 forwards to it.
const ZD_DIGEST = 'zt-digest'
const ZD_TOOLS = { 'digest-audit': 'audit', 'card-compiler': 'card', 'model-compiler': 'model', 'model-context': 'model-context', 'list-models': 'list-models' }
const ZD_CALL = `${ZD_DIGEST} call`
function zdTool(name, scripts = ZD_SCRIPTS) {
  return ZD_TOOLS[name] ? `${ZD_DIGEST} ${ZD_TOOLS[name]}` : `pwsh -NoProfile -File "${scripts}\\${name}.ps1"`
}
const ZD_DSH_SOURCES = {
  official: 'deepseek-official/deepseek-flash',
  opencode: 'opencode-go/deepseek-v4.1-flash',
}
const ZD_PER_MANAGER_MAX = 8
const ZD_MAX_MANAGERS = 8
const ZD_USAGE = {
  type: 'object',
  properties: {
    input_cache_miss: { type: 'number' }, cached_read: { type: 'number' },
    output: { type: 'number' }, cost: { type: 'number' }, calls: { type: 'number' },
  },
  required: ['input_cache_miss', 'cached_read', 'output', 'cost', 'calls'],
}
const ZD_USAGE_STEP = 'Load the DeepSeek tools with ToolSearch query "select:mcp__deepseek__deepseek_usage" and call deepseek_usage with days=1. Sum each field over the rows for the last day listed.'
const ZD_RUN_SCHEMA = { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' } }, required: ['ok', 'output'] }

// The JSON object in a line, also after prose on the same line ("…保留原样。{"address":…}").
function zdJsonIn(line) {
  for (let i = line.indexOf('{'); i >= 0; i = line.indexOf('{', i + 1)) {
    try { const v = JSON.parse(line.slice(i).trim()); if (v && typeof v === 'object') return v } catch (e) { }
  }
  return null
}
// The compiler's JSON line: the last line holding a JSON object.
function zdLastJson(text) {
  for (const line of String(text || '').trim().split('\n').reverse()) { const v = zdJsonIn(line); if (v) return v }
  return null
}

// The DeepSeek source model before (or without) the probe: the given model, the given source, or the official one.
function zdDefaultModel(A) {
  const source = A.dshModel ? 'model' : (['official', 'opencode'].includes(A.dshSource) ? A.dshSource : 'auto')
  return { source, model: A.dshModel || ZD_DSH_SOURCES[source] || ZD_DSH_SOURCES.official }
}

function zdMake(ctx, p) {
  const A = p.args || {}
  const MODE = p.mode || 'full'
  const KEY = A.citationKey
  const OUT = A.out
  const profile = p.profile || {}
  const SCRIPTS = ZD_SCRIPTS
  const DSH_MODEL = p.model || zdDefaultModel(A).model
  const DIGEST_PREFIX = `${OUT}\\${profile.digestPrefix || 'shared-prefix.md'}`
  const LINK_PREFIX = `${OUT}\\${profile.linkPrefix || 'link-prefix.md'}`
  // Extra instruction files of the model compiler (math: -BriefDoc, -FillDoc, -ReviewDoc), in the order given.
  const COMPILER_DOCS = Object.entries(profile.compilerDocs || {}).map(([k, f]) => ` -${k} "${OUT}\\${f}"`).join('')
  const SKIP_FORMS = profile.skipForms || []
  const PER_MANAGER = Math.min(ZD_PER_MANAGER_MAX, A.perManager || 5)
  const raised = []
  const atLeastHigh = (value, name) => {
    if (value === 'high' || value === 'max') return value
    if (value) raised.push(`${name} ${value} → high`)
    return 'high'
  }
  const DSH_EFFORT = atLeastHigh(A.dshEffort, 'dshEffort')
  const MODELS = A.models !== false
  const ARCH_DOC = `${SCRIPTS}\\model-architect.md`
  const ARCH_SUPPLEMENT = profile.architectSupplement ? `${SCRIPTS}\\${profile.architectSupplement.file}` : null
  const COMPILER = zdTool('model-compiler', SCRIPTS)
  const MODEL_WORK = `${OUT}\\models`
  const PER_SONNET = Math.max(1, A.perSonnet || 6)
  const DSH_PER_BATCH = Math.min(ZD_PER_MANAGER_MAX, Math.max(1, A.dshPerBatch || A.dshPerSonnet || 8))
  const REBUILD = !!(A.rebuild || A.redoModels)
  const LEVEL = ['concept', 'formal'].includes(A.level) ? A.level : null
  const PER_CONCEPT = Math.max(1, A.perConcept || 10)
  const BRIEF_EFFORT = atLeastHigh(A.briefEffort, 'briefEffort')
  const DERIVE_EFFORT = atLeastHigh(A.deriveEffort, 'deriveEffort')
  const REVIEW_EFFORT = atLeastHigh(A.reviewEffort, 'reviewEffort')
  const REVIEW_ROUNDS = Math.max(0, A.reviewRounds ?? 2)
  const LIMITS = `-MaxSentences ${A.maxSentences || 12} -MaxChars ${A.maxPartChars || 400} -MaxSteps ${A.maxSteps || 6} -MaxExplainSentences ${A.maxExplainSentences || 16} -MaxExplainChars ${A.maxExplainChars || 600}`
  const DSH_SKILLS = ['zettelkasten', 'asd-ste100-zh']
  const FILL_SKILLS = ['asd-ste100-zh']
  const DSH_TOOLS = 'select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_prompt,mcp__deepseek__deepseek_delete'
  const DRIVE_TOOLS = `${DSH_TOOLS},mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_remove_workspace`
  const HUB_WORK = A.hubWork || 'D:\\zt-digest\\hubs'
  const HUB_COMPILER = zdTool('hub-compiler', SCRIPTS)
  const HUB_EFFORT = atLeastHigh(A.hubEffort, 'hubEffort')
  const HUB_FIX_ROUNDS = Math.max(0, A.hubFixRounds ?? 1)
  const HUB_SKILLS = ['asd-ste100-zh']
  const CARD_CHECKS = MODE === 'cards' || ((MODE === 'full' || MODE === 'link') && A.cardChecks !== false)
  const CARD_COMPILER = zdTool('card-compiler', SCRIPTS)
  const CARD_FIX_ROUNDS = Math.max(0, A.cardFixRounds ?? 1)
  const IDEA_SAMPLE = Math.max(0, Math.min(1, A.ideaSample ?? (MODE === 'full' ? 0.1 : 0)))
  const CARD_SKILLS = ['asd-ste100-zh']
  const APPLY_SHARE = Math.max(1, A.applyShare || 30)
  const DSH_MANAGERS = Math.min(ZD_MAX_MANAGERS, Math.max(1, A.dshManagers || 8))
  const LINK_PER_MANAGER = Math.min(ZD_PER_MANAGER_MAX, Math.max(PER_MANAGER, A.linkPerManager || 8))

  // Every agent through the blueprint's roles: Haiku is "manager", Sonnet "architect", Opus "organizer".
  const zAgent = (prompt, o) => ctx.agent(prompt, {
    role: o.model === 'sonnet' ? 'architect' : o.model === 'opus' ? 'organizer' : 'manager',
    label: o.label, phase: o.phase, schema: o.schema,
  })
  // One Haiku agent runs one script command and returns its last JSON line (or null).
  const runScript = async (label, phaseName, command, note = '') => {
    const run = await zAgent(
      `Run this PowerShell command once in the foreground with timeout 600000 and wait for it${note}; do nothing else:
${command}
Return ok (true when it exits without an error) and output = its last output line exactly (a compact JSON object), or the error text when it fails. Do not retry.`,
      { label, phase: phaseName, model: 'haiku', schema: ZD_RUN_SCHEMA },
    )
    return { run, json: run && run.ok ? zdLastJson(run.output) : null }
  }

  const groupsOf = (count) => Array.from({ length: count || 0 }, (_, i) => {
    const n = i + 1
    const nn = String(n).padStart(2, '0')
    return { n, nn, file: `${OUT}\\groups\\g${nn}.md`, snapshot: `${OUT}\\snapshot-g${nn}.md`, link: `${OUT}\\link-g${nn}.md` }
  })
  const toBlocks = (list, size = PER_MANAGER) => {
    const out = []
    for (let i = 0; i < list.length; i += size) out.push(list.slice(i, i + size))
    return out
  }

  // --- one Haiku manager per block -----------------------------------------------------------------
  const REPORT = {
    type: 'object',
    properties: {
      group: { type: 'number' }, status: { type: 'string', enum: ['done', 'failed'] },
      cards: { type: 'number' }, models: { type: 'number' }, events: { type: 'number' }, links: { type: 'number' }, topics: { type: 'array', items: { type: 'string' } },
      errors: { type: 'string' }, deleted: { type: 'boolean' },
      tool_calls: { type: 'number' }, context_used: { type: 'number' },
    },
    required: ['group', 'status', 'errors', 'deleted'],
  }
  const STAGES = {
    digest: {
      phase: 'Digest',
      steps: g => `Group ${g.n}:
  a. Snapshot the slip-box (one PowerShell call): ${zdTool('snapshot-topics', SCRIPTS)} -Out "${g.snapshot}"
  b. deepseek_run with exactly: cwd "${OUT}"; prompt_files ["${DIGEST_PREFIX}", "${g.file}"] (the book, its index and the snapshot stay on disk; DSH reads what it needs); prompt "按以上说明完成你的组，只回复一行 JSON。"; skills ${JSON.stringify(DSH_SKILLS)}; model "${DSH_MODEL}"; reasoning_effort "${DSH_EFFORT}"; wait_seconds 900.`,
      answer: '{"group","cards","models","events","topics","anchor","errors"}',
      fields: 'cards (Cards written), models (Model Cards), events (Event Cards), topics (as "地址 主题名")',
    },
    link: {
      phase: 'Link',
      parallel: true,
      steps: g => `Group ${g.n}:
  deepseek_run with exactly: cwd "${OUT}"; prompt_files ["${LINK_PREFIX}", "${g.link}"]; prompt "按以上说明完成你的组的连接，只回复一行 JSON。"; skills ${JSON.stringify(DSH_SKILLS)}; model "${DSH_MODEL}"; reasoning_effort "${DSH_EFFORT}"; wait_seconds 0.
  When it finished with status "done", mark it (one PowerShell call): Set-Content -LiteralPath "${OUT}\\linked-g${g.nn}.txt" -Value done`,
      answer: '{"group","cards_linked","links_added","errors"}',
      fields: 'cards (= cards_linked), links (= links_added)',
    },
  }
  const manage = (block, b, stage) => {
    const S = STAGES[stage]
    return dsmpSlot(() => zAgent(
      `You manage ${block.length} DeepSeek Harness (DSH) runs of a ZET book digest (${stage} pass), ${S.parallel ? 'all at the same time' : 'one after another'}. DSH reads the book and changes the Cards; you only start, supervise, and clean up each run. Keep your messages short.
Load all tools with one ToolSearch call: query "${DSH_TOOLS}".

${S.parallel ? 'Run the groups below at the same time: start every run in one message (all deepseek_run calls together), then follow each one until it ends. The runs do not depend on each other.' : 'For each group below, in order, finish it completely before starting the next:'}
${block.map(S.steps).join('\n')}

Pass every deepseek_run value unchanged: the identical leading prompt files, prompt, skills, model and effort keep the DSH prompt cache warm. You send the skills: put them in the "skills" field of the call, exactly as given. Do not paste a skill's text into the prompt and do not write it to a prompt file. If a call fails because a skill is not found, stop and mark the group "failed" with that error; do not start the run without its skills.
Then follow "status" until the run ends:
  - "running": deepseek_wait (session_id, wait_seconds 900).
  - "needs_response": approve (decision "allow") a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1), the self-check command zt-check (read-only), and read-only reads or searches (Read, Get-Content, Select-String) of files under "${OUT}" or of "${A.source}"; reject anything that writes, moves or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
  - "error", or "finished" without a one-line JSON answer: once, deepseek_prompt (same session, wait_seconds 900, the same skills as the run) with "按 resume 行修正失败的调用后重跑，最后只回复一行 JSON。"; if it still fails, mark the group "failed".
  - "finished": parse its JSON ${S.answer}.
  Then clean up: after you have its answer (or its error), deepseek_delete (session_id); deleted=true when the delete succeeds. Never archive a conversation, and never delete one whose status is "running".
  Record tool_calls = the sum of the counts in the last turn report's tools_used, and context_used = context.used.

Return reports: one entry per group in this block with group, status, ${S.fields}, errors, deleted, tool_calls, context_used.`,
      {
        label: `${stage} manager ${b + 1}: groups ${block[0].n}-${block[block.length - 1].n}`, phase: S.phase, model: 'haiku',
        schema: { type: 'object', properties: { reports: { type: 'array', items: REPORT } }, required: ['reports'] },
      },
    ))
  }
  // A stage runs its blocks in parallel; with `after`, each block goes on to `after` as soon as its
  // own manager returns (a pipeline, not a barrier), so later stages overlap slower blocks.
  const runStage = async (stage, stageBlocks, after) => {
    const stages = [(block, _item, b) => manage(block, b, stage)]
    if (after) {
      stages.push((managed, block) => Promise.resolve(after(block, managed))
        .catch(error => { log(`${stage}: follow-up of groups ${block[0].n}-${block[block.length - 1].n} failed: ${error}`); return [] })
        .then(extra => ({ managed, extra })))
    }
    const results = await pipeline(stageBlocks, ...stages)
    const managed = results.map(r => (after ? r && r.managed : r))
    const extras = after ? results.flatMap(r => (r && r.extra) || []) : []
    const reports = []
    stageBlocks.forEach((block, b) => {
      const got = (managed[b] && managed[b].reports) || []
      for (const g of block) {
        reports.push(got.find(r => r.group === g.n) ||
          { group: g.n, status: 'failed', cards: 0, links: 0, topics: [], errors: 'no report from its manager', deleted: false })
      }
    })
    const failed = reports.filter(r => r.status !== 'done').map(r => r.group)
    const calls = reports.map(r => r.tool_calls).filter(x => typeof x === 'number')
    log(`${stage}: ${reports.reduce((s, r) => s + (r.cards || 0), 0)} cards, ${reports.reduce((s, r) => s + (r.links || 0), 0)} links; tool calls per group ${calls.join(', ') || 'n/a'}; failed ${failed.join(', ') || 'none'}`)
    return { reports, extras }
  }

  // --- The compiler-step driver (drive): one Haiku agent runs a step, managers run its DeepSeek jobs ---
  const DRIVE = {
    type: 'object',
    properties: {
      ok: { type: 'boolean' }, error: { type: 'string' }, output: { type: 'string' },
      results: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] }, answer: { type: 'string' } }, required: ['id', 'status'] } },
      deleted: { type: 'number' },
    },
    required: ['ok', 'results'],
  }
  // One Haiku driver runs one compiler step and the DeepSeek jobs it prints. o: command, dsh (the DSH
  // workspace), skills, effort, sessions (continued jobs), cleanup (the last round), phase, writes, keep.
  const driveAgent = (label, o, part) => {
    const table = Object.entries(o.sessions || {}).map(([job, session]) => `${job} ${session}`).join('\n')
    const removal = `deepseek_list with cwd "${o.dsh}"; deepseek_delete every conversation listed there that is not running, and count them in deleted; then, when none is running, deepseek_remove_workspace with cwd "${o.dsh}" and delete_conversations true. Never archive a conversation`
    const head = `You drive DeepSeek Harness (DSH) jobs for a ZET compiler script. You do not read the book and you do not write Cards: a script and DSH do. Keep your messages short.
Load the tools with one ToolSearch call: query "${DRIVE_TOOLS}".
`
    if (part === 'command') {
      return zAgent(`${head}
Run this one PowerShell command in the foreground with timeout 600000 and wait for it:
${o.command}
Its last output line is compact JSON; it may be long (tens of kilobytes). Put that line, unchanged and whole, in output: never shorten it, summarize it, or save it to a file instead. Do not run the jobs it lists: other managers run them. If the command fails, return ok=false with the error text; do not retry it with other options. Return ok, output, and results=[].`,
        { label, phase: o.phase || 'Models', model: 'haiku', schema: DRIVE })
    }
    if (part === 'cleanup') {
      return zAgent(`${head}
Clean up: ${removal}${o.keep ? '. Keep the folder and its files.' : `; then one PowerShell call: Remove-Item -LiteralPath "${o.dsh}" -Recurse -Force -ErrorAction SilentlyContinue`}
Return ok=true, results=[], and deleted.`,
        { label, phase: o.phase || 'Models', model: 'haiku', schema: DRIVE })
    }
    const jobs = o.jobs || []
    return dsmpSlot(() => zAgent(`${head}
Run every job below. A job has id, files, prompt, and sometimes continue and fallback. Other managers run the other jobs of this step at the same time; run only these.
${JSON.stringify(jobs)}
- No continue: deepseek_run with cwd "${o.dsh}", prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills)}, model "${DSH_MODEL}", reasoning_effort "${o.effort}", wait_seconds 0.
- continue: find the session of the job id named by continue in this table (job id, then session_id):
${table || '(empty)'}
  With a session: deepseek_prompt with that session_id, prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills)}, reasoning_effort "${o.effort}", wait_seconds 0. Without one: deepseek_run as above, with prompt_files = fallback.
Keep at most ${DSH_PER_BATCH} jobs running at a time; start the next job when one ends. Start independent jobs in one message.
Follow "status" until each job ends:
- "running": deepseek_wait (session_id, wait_seconds 600).
- "needs_response": approve (decision "allow") ${o.readOnly ? `only these, and refuse every other compiler step and every zt write: bridge-compiler.ps1 with -Step precheck (never -Apply); the check zt-check (read-only); a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1) with -Allow "${o.dsh}\\read-only.allow.txt" (an empty allow-list: reads only); a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"; and writing JSON files under "${o.writes}"` : `a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1), or the check zt-check (read-only); a read-only self-check of a draft: the card compiler (${ZD_DIGEST} card, or card-compiler.ps1), hub-compiler.ps1, event-compiler.ps1, or finding-compiler.ps1 with -Step precheck, the model compiler (${ZD_DIGEST} model, or model-compiler.ps1) with -Step explain-check, or book-skeleton.ps1 with -Step apply and without -Apply; a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}; and ${o.writes ? `writing JSON files under "${o.writes}"` : `writing the one file under "${o.dsh}" that the job's prompt names`}`}. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
- "error", or "finished" without JSON lines: once, deepseek_prompt (same session, the same skills, wait_seconds 600) with "按 resume 行修正失败的调用后重跑，最后只回复 JSON 行。"; if it still fails, the job failed.
- "finished": answer = its final answer text, unchanged (the JSON lines).
Do not close, archive, or delete conversations: later rounds continue them, and the step's cleanup agent deletes them.
Return ok=true, output="", and results: one entry per job with id, session_id, status ("done" or "failed"), and answer.`,
      { label, phase: o.phase || 'Models', model: 'haiku', schema: DRIVE }))
  }
  // A step's jobs. A compact list (card-compiler.ps1 -Compact) expands to { id, files, prompt, continue?, fallback? }.
  const expandJobs = (parsed) => {
    const list = (parsed && Array.isArray(parsed.jobs)) ? parsed.jobs : []
    if (!parsed || !parsed.lead || !parsed.prompts) return list
    const at = (name) => `${parsed.dir}\\${name}`
    return list.map(j => {
      if (j.files) return j
      if (j.kind === 'follow') return { id: j.id, continue: j.continue, files: [at(j.file)], fallback: [parsed.lead.fix, at(j.file)], prompt: parsed.prompts.follow }
      const lead = j.kind === 'orphans' ? parsed.lead.orphans : parsed.lead.fix
      return { id: j.id, files: [lead, at(j.file)], prompt: parsed.prompts[j.kind] }
    })
  }
  // Runs one compiler step: the command, its jobs spread over up to DSH_MANAGERS managers (one per
  // DSH_PER_BATCH jobs, all under dsmpSlot), then the cleanup when o.cleanup.
  const drive = async (label, o) => {
    const step = await driveAgent(label, o, 'command')
    if (!step || !step.ok) {
      if (o.cleanup) await driveAgent(`${label}: clean up`, o, 'cleanup')
      return { ok: false, error: (step && step.error) || 'no report from the step', output: (step && step.output) || '', results: [] }
    }
    const parsed = zdLastJson(step.output)
    const jobs = expandJobs(parsed)
    let results = []
    if (jobs.length) {
      const shares = []
      for (let i = 0; i < jobs.length; i += DSH_PER_BATCH) shares.push(jobs.slice(i, i + DSH_PER_BATCH))
      const managers = shares.length
      const done = new Array(managers)
      let next = 0
      const lane = async () => {
        while (next < managers) {
          const i = next++
          done[i] = await driveAgent(managers > 1 ? `${label} [${i + 1}/${managers}]` : label, { ...o, jobs: shares[i] }, 'jobs')
            .catch(error => ({ ok: false, error: String(error), results: [] }))
        }
      }
      await Promise.all(Array.from({ length: Math.min(DSH_MANAGERS, managers) }, lane))
      results = done.flatMap((d, i) => {
        const got = (d && d.results) || []
        const missing = shares[i].filter(j => !got.some(r => r.id === j.id)).map(j => ({ id: j.id, status: 'failed', answer: '' }))
        return [...got, ...missing]
      })
      const failedManagers = done.filter(d => !d || !d.ok).length
      if (failedManagers) log(`${label}: ${failedManagers} of ${managers} managers reported a failure.`)
    }
    let deleted = 0
    if (o.cleanup) {
      const c = await driveAgent(`${label}: clean up`, o, 'cleanup')
      deleted = (c && c.deleted) || 0
    }
    return { ok: true, output: step.output, results, deleted }
  }
  const answerLines = (driven) => ((driven && driven.results) || []).flatMap(r => String(r.answer || '').split('\n').map(zdJsonIn).filter(Boolean))

  // --- Cards: the card layer cycle -----------------------------------------------------------------
  const runCards = async (label, work, extra = '', only = '') => {
    const cstep = (s, more = '') => `${CARD_COMPILER} -Step ${s} -Work "${work}" -Compact${more}`
    const sessions = {}
    let jobs = 0
    const crun = async (what, command, opts = {}) => {
      const r = await drive(`cards ${label}: ${what}`, {
        command, dsh: work, skills: CARD_SKILLS, effort: DSH_EFFORT, sessions: opts.continues ? sessions : null,
        cleanup: opts.cleanup, phase: 'Cards', writes: `${work}\\out`, remove: 'delete', keep: true,
      })
      for (const x of (r && r.results) || []) {
        jobs++
        if (x.session_id) sessions[x.id] = x.session_id
      }
      if (!r || !r.ok) log(`cards ${label} ${what}: ${(r && r.error) || 'no report from its driver'}`)
      return r
    }
    const rules = only ? ` -Rules "${only}"` : A.cardRules ? ` -Rules "${A.cardRules}"` : ''
    const max = A.cardMax ? ` -MaxCards ${A.cardMax}` : ''
    const planned = await crun('plan', cstep('plan', ` -Book ${KEY} -Staging "${OUT}"${rules}${max}${extra}`))
    const plan = zdLastJson(planned && planned.output)
    if (plan) plan.jobs = expandJobs(plan)
    if (!plan) return { status: 'failed', label, error: `plan failed: ${(planned && planned.error) || 'no output'}` }
    log(`Cards ${label}: ${plan.picked} Cards of the book; ${plan.with_faults} with ${plan.faults} faults, ${plan.review} for review, ${plan.orphans} orphans; ${plan.jobs.length} DeepSeek jobs.`)
    if (!plan.jobs.length) {
      const reported = zdLastJson(((await crun('report', cstep('report'), { cleanup: true })) || {}).output)
      return { status: 'done', label, plan, report: reported }
    }
    // Apply in shares of APPLY_SHARE jobs until none is pending (one call stays inside the 10-minute limit).
    const applyAll = async (what) => {
      let r = null
      for (let i = 0; i < 100; i++) {
        r = zdLastJson(((await crun(i ? `${what}.${i + 1}` : what, cstep('apply', ` -MaxJobs ${APPLY_SHARE}`))) || {}).output)
        if (!r || !r.pending_jobs) return r
      }
      return r
    }
    let applied = await applyAll('apply 1')
    for (let round = 1; round <= CARD_FIX_ROUNDS; round++) {
      const checked = zdLastJson(((await crun(`check ${round}`, cstep('check'), { continues: true })) || {}).output)
      log(`Cards ${label} check ${round}: ${checked ? `${checked.faults} faults left, ${checked.jobs.length} follow-up jobs` : 'no output'}.`)
      if (!checked || !checked.jobs.length) break
      applied = await applyAll(`apply ${round + 1}`)
    }
    const reported = zdLastJson(((await crun('report', cstep('report'), { cleanup: true })) || {}).output)
    if (!applied) log(`Cards ${label}: an apply step failed; the cycle is marked failed.`)
    if (reported) log(`Cards ${label}: faults ${reported.faults_before} → ${reported.faults_after} on ${reported.cards} Cards; verdicts ${JSON.stringify(reported.verdicts)}; ${reported.splits} splits, ${reported.merges} merge proposals, ${reported.refused} refused; ${reported.orphan_links} orphan Links (${reported.orphans_after} orphans left); ${jobs} DeepSeek jobs.`)
    return { status: reported && applied ? 'done' : 'failed', label, plan, applied, report: reported, ...(applied ? {} : { error: 'apply failed: no output from the card compiler -Step apply' }) }
  }
  const cardBlock = async (block) => {
    const groups = block.map(g => g.n).join(',')
    return runCards(`groups ${block[0].n}-${block[block.length - 1].n}`, `${OUT}\\cards-g${String(block[0].n).padStart(2, '0')}`, ` -Groups "${groups}" -Sample ${IDEA_SAMPLE} -NoOrphans`)
  }

  return {
    A, MODE, KEY, OUT, SCRIPTS, DSH_MODEL, DSH_EFFORT, MODELS, CARD_CHECKS, IDEA_SAMPLE, LIMITS, COMPILER, FILL_SKILLS,
    HUB_WORK, HUB_COMPILER, HUB_EFFORT, HUB_FIX_ROUNDS, HUB_SKILLS, PER_MANAGER, LINK_PER_MANAGER, raised,
    zAgent, runScript, groupsOf, toBlocks, runStage, drive, expandJobs, answerLines,
    // The command that runs a digest script: zt-digest <tool> once ported, else pwsh with the script.
    tool: name => zdTool(name, SCRIPTS),
    // The cleanup round alone: delete every conversation of o.dsh and its workspace, even when no command ran.
    cleanupDsh: (label, o) => driveAgent(label, o, 'cleanup'),
    runCards, cardBlock, profile, ARCH_DOC, ARCH_SUPPLEMENT, MODEL_WORK, PER_SONNET, PER_CONCEPT, DSH_PER_BATCH, REBUILD, LEVEL,
    BRIEF_EFFORT, DERIVE_EFFORT, REVIEW_EFFORT, REVIEW_ROUNDS, COMPILER_DOCS, SKIP_FORMS,
  }
}

defineModule({
  id: 'zt-digest-lib', version: '5', library: true, requires: ['@dsmp'],
  summary: 'Library: the ZT digest machinery (settings, managers, drive, model compiler, card cycle) for the zt-* stage modules.',
})
// ==== end module zt-digest-lib ====
