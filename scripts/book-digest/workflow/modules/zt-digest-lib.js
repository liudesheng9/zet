// ==== module zt-digest-lib v11 ====
// The shared machinery of the ZT digest workflows (docs/zt-digest-framework.md), first ported from the
// hand-written zt-book-digest.js (since removed): the settings read from the args, the digest and link managers, the
// compiler-step driver (drive), the ready-work driver (chain), and the card layer cycle (runCards). The model
// compiler is zt-model-lib (the model layer, which each book type installs). A library module: the stage modules
// call zdMake(ctx, params) and use what it returns.
// One card cycle per book, not one per block (R296): `runCards` runs the whole-book cycle, and its
// `-ModelReview` option lets that cycle own the source review of a Model draft no model chain elaborates (R247).
// 11 adds `fixRounds`: the finalization's one correction cycle is declared, not inherited from the pilot
// setting (#86, R299).
// Every Haiku, Sonnet, and Opus agent goes through ctx.agent, so the blueprint's roles decide the models:
// manager (Haiku), architect (Sonnet), organizer (Opus). Managers share the DeepSeek Subagent Managed
// Protocol pool (dsmpSlot): at most 8 managers alive in the whole run.
// The ready-work schedule (9.3/9.4, #82): the ledger owns readiness. A driver claims ready units in bounded
// blocks (`ledger next`), runs each unit's compiler chain in ONE manager's conversation (`chain`), records the
// result through that family's checked finish command, and seals with the layer gate. Retries are per unit
// (`maxAttempts`), never a whole-stage rerun. No module reads a clock: the bounded readiness wait lives in
// zt-digest (`ledger next -Wait`).
const ZD_SCRIPTS = 'E:\\zet\\scripts\\book-digest'
// The digest scripts ported to zt-digest (tools/zt-digest, one fast binary): script name → tool. zdTool(name)
// is the command that runs a script: `zt-digest <tool>` once it is ported, else pwsh with the script. The
// helper is `zt-digest call` (ZD_CALL); the old helper path zt-mcp.ps1 forwards to it.
const ZD_DIGEST = 'zt-digest'
const ZD_TOOLS = {
  'digest-audit': 'audit', 'card-compiler': 'card', 'model-compiler': 'model', 'model-context': 'model-context', 'list-models': 'list-models',
  'hub-compiler': 'hub', 'bridge-compiler': 'bridge', 'math-compiler': 'math', 'event-compiler': 'event', 'finding-compiler': 'finding',
  'book-terms': 'terms', 'book-skeleton': 'skeleton', 'organize-survey': 'organize-survey', 'organize-apply': 'organize-apply',
  'attach-figures': 'figures', 'prepare-book-digest': 'prepare', 'prepare-link-pass': 'prepare-link', 'snapshot-topics': 'snapshot-topics',
}
const ZD_CALL = `${ZD_DIGEST} call`
const ZD_LEDGER = `${ZD_DIGEST} ledger`
// R265: every manager's compiler command carries this explicit timeout (milliseconds).
const ZD_COMMAND_TIMEOUT_MS = 600000
// R269: a write-capable compiler command gives its child `zt mcp` this many seconds of edit patience.
const ZD_EDIT_PATIENCE = 180
function zdTool(name, scripts = ZD_SCRIPTS) {
  return ZD_TOOLS[name] ? `${ZD_DIGEST} ${ZD_TOOLS[name]}` : `pwsh -NoProfile -File "${scripts}\\${name}.ps1"`
}
const ZD_DSH_SOURCES = {
  official: 'deepseek-official/deepseek-flash',
  opencode: 'opencode-go/deepseek-v4.1-flash',
}
const ZD_PER_MANAGER_MAX = 8
const ZD_MAX_MANAGERS = 8
// R343–R346: run usage is per provider run, recorded from that run's own result. This framework never
// reads the account-wide usage ledger: the difference of two rolling totals is not a run's cost (I22/R344).
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
  // R297: the deterministic Idea sample of the one whole-book card cycle (modes cards and full).
  const IDEA_SAMPLE = Math.max(0, Math.min(1, A.ideaSample ?? ((MODE === 'full' || MODE === 'cards') ? 0.1 : 0)))
  const CARD_SKILLS = ['asd-ste100-zh']
  const APPLY_SHARE = Math.max(1, A.applyShare || 30)
  const DSH_MANAGERS = Math.min(ZD_MAX_MANAGERS, Math.max(1, A.dshManagers || 8))
  const LINK_PER_MANAGER = Math.min(ZD_PER_MANAGER_MAX, Math.max(PER_MANAGER, A.linkPerManager || 8))
  // R273: only the two bounded run settings are exposed; the tool keeps leases, batch
  // sizes and heartbeat intervals as versioned defaults. Both are persisted at
  // preparation, so a resume reuses the frozen effective values.
  const MAX_ATTEMPTS = Math.max(1, Math.min(10, A.maxAttempts ?? 3))
  const DSH_RUN_CAP = Math.max(60, Math.min(3600, A.dshRunCap ?? 1200))
  // The one runner prompt line every manager gets: no general shell, no timers, no handoff (R260).
  const RUNNER = `You have only these tools: the bounded PowerShell command named below (always with timeout_ms: ${ZD_COMMAND_TIMEOUT_MS}), file reads and writes under the paths named here, and the DeepSeek Harness tools. You have no terminal tabs, no Monitor loop, no session handoff and no general shell: never start another agent, never hand the work on, never poll with Start-Sleep or any other timer, and wait only with deepseek_wait. Do not set environment variables: the tools keep their own bounds.`

  // Every agent through the blueprint's roles: Haiku is "manager", Sonnet "architect", Opus "organizer".
  const zAgent = (prompt, o) => ctx.agent(prompt, {
    role: o.model === 'sonnet' ? 'architect' : o.model === 'opus' ? 'organizer' : 'manager',
    label: o.label, phase: o.phase, schema: o.schema,
  })
  // One Haiku agent runs one script command and returns its last JSON line (or null).
  const runScript = async (label, phaseName, command, note = '', o = {}) => {
    const run = await zAgent(
      `Run this PowerShell command once in the foreground with timeout_ms: ${ZD_COMMAND_TIMEOUT_MS} and wait for it${note}; do nothing else:
${command}
Return ok (true when it exits without an error) and output = its last output line exactly (a compact JSON object), or the error text when it fails. Do not retry.`,
      { label, phase: phaseName, model: 'haiku', schema: ZD_RUN_SCHEMA },
    )
    // A gate prints its JSON and then exits 3 while the layer still has open work, so a caller that asked
    // for it still reads a typed result instead of only prose.
    const json = run && (run.ok || o.always) ? zdLastJson(run.output) : null
    return { run, json }
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
      usage: { type: 'object', description: 'the numbers the run\'s own result reported for its tokens or cost' },
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
${RUNNER}
Load all tools with one ToolSearch call: query "${DSH_TOOLS}".

${S.parallel ? 'Run the groups below at the same time: start every run in one message (all deepseek_run calls together), then follow each one until it ends. The runs do not depend on each other.' : 'For each group below, in order, finish it completely before starting the next:'}
${block.map(S.steps).join('\n')}

Keep at most ${S.parallel ? Math.min(ZD_PER_MANAGER_MAX, block.length) : 1} DeepSeek run(s) going at a time; never start more than ${ZD_PER_MANAGER_MAX} in one manager. Pass every deepseek_run value unchanged: the identical leading prompt files, prompt, skills, model and effort keep the DSH prompt cache warm. You send the skills: put them in the "skills" field of the call, exactly as given. Do not paste a skill's text into the prompt and do not write it to a prompt file. If a call fails because a skill is not found, stop and mark the group "failed" with that error; do not start the run without its skills.
Then follow "status" until the run ends:
  - "running": deepseek_wait (session_id, wait_seconds 900).
  - "needs_response": approve (decision "allow") a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1), the self-check command zt-check (read-only), and read-only reads or searches (Read, Get-Content, Select-String) of files under "${OUT}" or of "${A.source}"; reject anything that writes, moves or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
  - "error", or "finished" without a one-line JSON answer: once, deepseek_prompt (same session, wait_seconds 900, the same skills as the run) with "按 resume 行修正失败的调用后重跑，最后只回复一行 JSON。"; if it still fails, mark the group "failed".
  - "finished": parse its JSON ${S.answer}.
  Then clean up: after you have its answer (or its error), deepseek_delete (session_id); deleted=true when the delete succeeds. Never archive a conversation, and never delete one whose status is "running".
  Record tool_calls = the sum of the counts in the last turn report's tools_used, and context_used = context.used.
  When the last turn report or the run's own result states this run's token usage or cost, copy those numbers into the result's usage object (for example {"input_tokens": 1200, "output_tokens": 300}). Never call deepseek_usage or subtract account-wide totals: a difference of two rolling totals is not this run's cost.

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
      results: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] }, answer: { type: 'string' }, usage: { type: 'object' } }, required: ['id', 'status'] } },
      deleted: { type: 'number' },
    },
    required: ['ok', 'results'],
  }
  // One Haiku driver runs one compiler step and the DeepSeek jobs it prints. o: command, dsh (the DSH
  // workspace), skills, effort, sessions (continued jobs), cleanup (the last round), phase, writes, keep.
  const driveAgent = (label, o, part) => {
    const table = Object.entries(o.sessions || {}).map(([job, session]) => `${job} ${session}`).join('\n')
    const head = `You drive DeepSeek Harness (DSH) jobs for a ZET compiler script. You do not read the book and you do not write Cards: a script and DSH do. Keep your messages short.
${RUNNER}
Load the tools with one ToolSearch call: query "${DRIVE_TOOLS}".
`
    if (part === 'command') {
      return zAgent(`${head}
Run this one PowerShell command in the foreground with timeout_ms: ${ZD_COMMAND_TIMEOUT_MS} and wait for it:
${o.command}
Its last output line is compact JSON; it may be long (tens of kilobytes). Put that line, unchanged and whole, in output: never shorten it, summarize it, or save it to a file instead. Do not run the jobs it lists: other managers run them. If the command fails, return ok=false with the error text; do not retry it with other options. Return ok, output, and results=[].`,
        { label, phase: o.phase || 'Models', model: 'haiku', schema: DRIVE })
    }
    if (part === 'cleanup') {
      // R323/R327: a step no longer deletes its conversations. Every created conversation was recorded
      // when the provider returned its id, and the run's cleanup stage deletes exactly those ids after
      // the owning work settled. Nothing is removed on a title match or on an agent's guess.
      return { ok: true, results: [], deleted: 0, deferred: true }
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
- "needs_response": approve (decision "allow") ${o.readOnly ? `only these, and refuse every other compiler step and every zt write: the Bridge compiler (${ZD_DIGEST} bridge, or bridge-compiler.ps1) with -Step precheck (never -Apply); the check zt-check (read-only); a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1) with -Allow "${o.dsh}\\read-only.allow.txt" (an empty allow-list: reads only); a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"; and writing JSON files under "${o.writes}"` : `a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1), or the check zt-check (read-only); a read-only self-check of a draft: the card, hub, event, or finding compiler (${ZD_DIGEST} card, hub, event, or finding, or card-compiler.ps1, hub-compiler.ps1, event-compiler.ps1, finding-compiler.ps1) with -Step precheck, the model compiler (${ZD_DIGEST} model, or model-compiler.ps1) with -Step explain-check, or the book skeleton (${ZD_DIGEST} skeleton, or book-skeleton.ps1) with -Step apply and without -Apply; a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}; and ${o.writes ? `writing JSON files under "${o.writes}"` : `writing the one file under "${o.dsh}" that the job's prompt names`}`}. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
- "error", or "finished" without JSON lines: once, deepseek_prompt (same session, the same skills, wait_seconds 600) with "按 resume 行修正失败的调用后重跑，最后只回复 JSON 行。"; if it still fails, the job failed.
- "finished": answer = its final answer text, unchanged (the JSON lines).
Do not close, archive, or delete conversations: later rounds continue them, and the step's cleanup agent deletes them.
When the run's own result or its last turn report states this run's token usage or cost, copy those numbers into that result's usage object. Never call deepseek_usage or subtract account-wide totals: a difference of two rolling totals is not this run's cost.
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
  // DSH_PER_BATCH jobs, all under dsmpSlot). The run ids are recorded at once; the run-level cleanup
  // later deletes exactly those ids (R322/R323).
  const drive = async (label, o) => {
    const step = await driveAgent(label, o, 'command')
    if (!step || !step.ok) {
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
    // R322: the provider returned these ids, so record them now, against the work that owns them.
    if (results.length) {
      const recorded = await registerResources(`${label}: record the run ids`, resourceOwner(o), o.dsh, results)
      if (!recorded.ok) log(`${label}: ${recorded.error}`)
    }
    return { ok: true, output: step.output, results, deleted: 0 }
  }
  const answerLines = (driven) => ((driven && driven.results) || []).flatMap(r => String(r.answer || '').split('\n').map(zdJsonIn).filter(Boolean))

  // --- The ready-work driver (9.3, R250-R258): claim a bounded block, run each unit's chain in one manager ----
  // One manager per chain: it runs the chain's commands and DeepSeek jobs in its own conversation (R250/R251).
  // The chain is declared here, frozen in the blueprint's module versions, never improvised at run time.
  const CHAIN = {
    type: 'object',
    properties: {
      steps: {
        type: 'array',
        items: {
          type: 'object',
          properties: { name: { type: 'string' }, ok: { type: 'boolean' }, exit: { type: 'number' }, output: { type: 'string' }, error: { type: 'string' } },
          required: ['name', 'ok'],
        },
      },
      results: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            id: { type: 'string' }, step: { type: 'string' }, session_id: { type: 'string' },
            status: { type: 'string', enum: ['done', 'failed'] }, answer: { type: 'string' },
            usage: { type: 'object' },
          },
          required: ['id', 'status'],
        },
      },
    },
    required: ['steps', 'results'],
  }
  // The steps a chain runs: { name, command, jobs?, repeat? }. `jobs` collects the DeepSeek jobs the step
  // prints (the default). `repeat` runs the same command again while its JSON still reports work, at most
  // `max` times; that is the bounded correction path, not an unbounded loop. A command may hold
  // {{unit}}, {{token}}, {{group}} and {{draft}}; they come from what the chain has already read: the claim
  // step's `units[0]` (its `id`, `token` and the number in `gNNN`) and a start step's `draft`.
  const chainPrompt = (label, o) => {
    const steps = o.steps || []
    const listed = steps.map((s, i) => {
      const lines = [`${i + 1}. ${s.name}: run this command in the foreground with timeout_ms: ${ZD_COMMAND_TIMEOUT_MS} and wait for it, then read its last output line (compact JSON, it may be tens of kilobytes; keep it whole):`, `   ${s.command}`]
      if (s.claim) lines.push(`   This is the claim step. If its JSON has an empty "units" array, stop the chain here: run no later step and no job, and return the steps you ran. Otherwise remember its "units"[0].id, "units"[0].token and the group number in that id (the digits in gNNN) for the commands below.`)
      if (s.jobs) lines.push(`   When its JSON has a nonempty "jobs" array, run those jobs before the next step (the job rules below). When it names a "job" file instead (a group start), read that file and run the jobs it declares (its "jobs" entries name files and prompt; its "expected_jobs" must equal the number you run).`)
      else lines.push(`   It starts no DeepSeek job by itself.`)
      if (s.repeat) lines.push(`   Repeat this same command while its JSON still reports ${s.repeat.path} ${s.repeat.op || '>'} ${s.repeat.value}, at most ${s.repeat.max || 100} times, and record the last JSON in this step's output.`)
      if (s.optional) lines.push(`   A failure here is recorded and the chain goes on with the next step.`)
      return lines.join('\n')
    }).join('\n')
    return `You run one compiler chain of a ZET book digest: ${label}. A script and DeepSeek Harness (DSH) do the work; you only run the bounded commands and the DSH jobs they print. Keep your messages short.
${RUNNER}
Load the DSH tools with one ToolSearch call: query "${DRIVE_TOOLS}".
The chain is fixed. Run every step in this order and nothing else. A command may contain {{unit}}, {{token}}, {{group}} or {{draft}}: replace each with the value the chain has already read (the claim's unit id, its token, the digits of that id's gNNN, and a start step's "draft").
CHAIN PLAN: ${JSON.stringify({ dsh: o.dsh, steps: steps.map(s => ({ name: s.name, command: s.command, claim: !!s.claim, jobs: !!s.jobs, repeat: s.repeat || null, optional: !!s.optional })) })}
${listed}
Jobs (for every step that prints them):
- A compact list (the card compiler's -Compact) is { dir, lead: { fix, orphans }, prompts: { fix, orphans, follow }, jobs: [{ id, kind, file, continue? }] }. Expand each entry: kind "fix" → files [lead.fix, dir\file], prompt prompts.fix; kind "orphans" → files [lead.orphans, dir\file], prompt prompts.orphans; kind "follow" → files [dir\file], prompt prompts.follow, and continue = its "continue" id. Any other entry already names files and prompt.
- No continue: deepseek_run with cwd "${o.dsh}", prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills || [])}, model "${DSH_MODEL}", reasoning_effort "${o.effort || DSH_EFFORT}", wait_seconds 0.
- continue: reuse the session_id of the earlier job of this chain with that id (from this chain's own results); without one, deepseek_run with prompt_files = fallback (or files when it names none).
Keep at most ${DSH_PER_BATCH} jobs running at a time; start independent jobs in one message and start the next job when one ends.
Follow "status" until each job ends:
- "running": deepseek_wait (session_id, wait_seconds 600).
- "needs_response": approve (decision "allow") ${o.readOnly ? `only these, and refuse every other compiler step and every zt write: the Bridge compiler (${ZD_DIGEST} bridge, or bridge-compiler.ps1) with -Step precheck (never -Apply); the check zt-check (read-only); a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1) with -Allow "${o.dsh}\\read-only.allow.txt" (an empty allow-list: reads only); a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"; and writing JSON files under "${o.writes}"` : `a command that runs the zt helper (${ZD_CALL}, or pwsh with zt-mcp.ps1), or the check zt-check (read-only); a read-only self-check of a draft: the card, hub, event, or finding compiler (${ZD_DIGEST} card, hub, event, or finding, or card-compiler.ps1, hub-compiler.ps1, event-compiler.ps1, finding-compiler.ps1) with -Step precheck, the model compiler (${ZD_DIGEST} model, or model-compiler.ps1) with -Step explain-check, or the book skeleton (${ZD_DIGEST} skeleton, or book-skeleton.ps1) with -Step apply and without -Apply; a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}; and ${o.writes ? `writing JSON files under "${o.writes}"` : `writing the one file under "${o.dsh}" that the job's prompt names`}`}. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
- "error", or "finished" without JSON lines: once, deepseek_prompt (same session, the same skills, wait_seconds 600) with "按 resume 行修正失败的调用后重跑，最后只回复 JSON 行。"; if it still fails, the job failed.
- "finished": answer = its final answer text, unchanged (the JSON lines).
Do not close, archive, or delete conversations: later rounds continue them, and the step's cleanup deletes them.
When the run's own result or its last turn report states this run's token usage or cost, copy those numbers into that result's usage object. Never call deepseek_usage or subtract account-wide totals: a difference of two rolling totals is not this run's cost.
Return steps: one entry per step with name, ok (true when it exited without an error), exit (its exit code when known), output (its last line unchanged) or error (the failure text); and results: one entry per job with id, step, session_id, status ("done" or "failed"), and answer.`
  }
  // Runs one declared chain in ONE manager. o: label, dsh, skills, effort, phase, writes, steps, sessions, cleanup, keep.
  const chain = async (label, o) => {
    const run = await dsmpSlot(() => zAgent(chainPrompt(label, o), {
      label, phase: o.phase || 'Models', model: 'haiku', schema: CHAIN,
    }))
    const driven = run || { steps: [], results: [] }
    const steps = Array.isArray(driven.steps) ? driven.steps : []
    const results = Array.isArray(driven.results) ? driven.results : []
    // R322: a claim step inside the chain names the unit that owns its conversations; a block of units
    // claimed before the chain names its own owner; a stage-wide chain is the run's own work.
    // A declared claim step (the chain's own first step) names the unit that owns these conversations.
    const claimStep = (o.steps || []).find((s) => s.claim)
    const claimRun = claimStep ? steps.find((s) => s.name === claimStep.name) : null
    const claimed = claimRun ? ((zdLastJson(claimRun.output) || {}).units || []) : []
    const owner = claimed[0] && claimed[0].id ? { unit: claimed[0].id, token: claimed[0].token } : resourceOwner(o)
    if (results.length) {
      const recorded = await registerResources(`${label}: record the run ids`, owner, o.dsh, results)
      if (!recorded.ok) log(`${label}: ${recorded.error}`)
    }
    return { ok: steps.every(s => s.ok), steps, results, deleted: 0, raw: driven, unit: owner === 'run' ? 'run' : owner.unit }
  }
  const stepOf = (driven, name) => ((driven && driven.steps) || []).find(s => s.name === name) || null
  const jsonOf = (driven, name) => zdLastJson((stepOf(driven, name) || {}).output)
  // R271/R272: one compact JSON object on the last line, with the expected job count, the unit and the
  // input hash checked before a run trusts it. `jobs: []` is a success only with an explicit no-op reason.
  const resultLine = (driven, name, o = {}) => {
    const step = stepOf(driven, name)
    const parsed = zdLastJson(step && step.output)
    const faults = []
    if (!step) faults.push(`the chain has no step ${name}`)
    else if (!step.ok) faults.push(`${name} failed: ${step.error || 'nonzero exit'}`)
    else if (!parsed) faults.push(`${name} printed no JSON line`)
    if (parsed && o.unit && (o.unit !== parsed.unit) && (parsed.unit !== undefined)) faults.push(`${name} answered unit ${parsed.unit}, not ${o.unit}`)
    if (parsed && o.token !== undefined && parsed.token !== undefined && parsed.token !== o.token) faults.push(`${name} answered token ${parsed.token}, not ${o.token}`)
    if (parsed && o.inputHash && typeof parsed.input_hash === 'string' && parsed.input_hash !== o.inputHash) faults.push(`${name} answered input ${parsed.input_hash}, not ${o.inputHash}`)
    if (parsed && o.expectedJobs !== undefined) {
      const declared = parsed.expected_jobs
      const jobs = Array.isArray(parsed.jobs) ? parsed.jobs.length : Array.isArray(parsed.results) ? parsed.results.length : null
      if (declared === undefined) faults.push(`${name} did not declare expected_jobs`)
      else if (declared !== o.expectedJobs) faults.push(`${name} declared ${declared} expected jobs, not ${o.expectedJobs}`)
      else if (declared === 0 && jobs !== 0) faults.push(`${name} declared no jobs but printed ${jobs}`)
      if (declared === 0 && !parsed.no_op && !parsed.no_op_reason && !parsed.reason) faults.push(`${name} declared no jobs without a no-op reason`)
    }
    if (parsed && parsed.pending_jobs > 0) faults.push(`${name} left ${parsed.pending_jobs} jobs pending`)
    return { parsed, faults }
  }
  // The phase a ledger-only call is filed under: a declared stage title of the base blueprint.
  const LEDGER_PHASE = 'Digest'
  // The phase of the run-level cleanup: the base declares one `Cleanup` stage.
  const CLEANUP_PHASE = 'Cleanup'
  // The ledger adapter: every call is one bounded command in a manager's own conversation. The ledger owns
  // readiness, attempts and completion evidence; a module never reads a clock or a file to decide it (R270).
  const ledgerCommand = (args) => `${ZD_LEDGER} ${args}`
  // Claims ready units of one layer in a bounded block. `wait` is the tool-side readiness wait (seconds).
  const claim = async (label, layer, take, wait = 0) => {
    const out = await runScript(label, LEDGER_PHASE, ledgerCommand(`next -Out "${OUT}" -Layer ${layer} -Take ${take} -Owner "${label}"${wait ? ` -Wait ${wait}` : ''}`))
    const parsed = out.json
    if (!parsed) return { ok: false, error: (out.run && out.run.output) || 'no output from ledger next', units: [] }
    return { ok: true, units: parsed.units || [], layer: parsed.layer, settled: parsed.settled, complete: parsed.complete, counts: parsed.counts }
  }
  const gate = async (label, layer) => {
    const out = await runScript(label, LEDGER_PHASE, ledgerCommand(`gate -Out "${OUT}" -Layer ${layer}`), ' (exit 3 means the layer still has open work)', { always: true })
    return out.json || { ok: false, error: (out.run && out.run.output) || 'no output from ledger gate' }
  }
  const renew = (label, unit, token) => runScript(label, LEDGER_PHASE, ledgerCommand(`renew -Out "${OUT}" -Unit "${unit}" -Token ${token}`))
  const failUnit = (label, unit, token, klass, fault) => runScript(label, LEDGER_PHASE,
    `${ZD_LEDGER} fail -Out "${OUT}" -Unit "${unit}" -Token ${token} -Class ${klass} -Fault ${psQuote(JSON.stringify(fault))}`)
  // The registered provider run of one unit: register (with the frozen cap), observe, cancel, list (R267/R268).
  const jobOf = (label, unit, action, more = '') => runScript(label, LEDGER_PHASE, ledgerCommand(`job -Out "${OUT}" -Unit "${unit}" ${action}${more}`))
  // A PowerShell single-quoted literal (the tool's -Fault and -Json values are JSON text).
  function psQuote(text) {
    return `'${String(text).replace(/'/g, "''")}'`
  }
  // The family finish command of one claimed unit: the checked verifier that records the completion.
  const finishOf = async (label, unit, token, extra = {}, o = {}) => {
    const number = /\/g(\d+)@1$/.exec(unit)
    const model = o.model || (o.inputs && (o.inputs.model || o.inputs.header))
    let command = ''
    if (/^digest\//.test(unit)) command = `${ZD_DIGEST} group-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token} -Draft ${psQuote(o.draft || '')}`
    else if (/^cards\//.test(unit)) command = `${ZD_DIGEST} cards-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token}`
    else if (/^events\//.test(unit)) command = `${ZD_DIGEST} events-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token}`
    else if (/^findings\//.test(unit)) command = `${ZD_DIGEST} findings-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token}`
    else if (/^link\//.test(unit)) command = `${ZD_DIGEST} link-finish -Out "${OUT}" -Group ${Number(number[1])} -Token ${token} -Draft ${psQuote(o.draft || '')}`
    else if (/^models\//.test(unit)) command = `${ZD_DIGEST} model-finish -Out "${OUT}" -Model ${psQuote(model)} -Token ${token}`
    else if (/^explain\//.test(unit)) command = `${ZD_DIGEST} explain-finish -Out "${OUT}" -Model ${psQuote(model)} -Token ${token}`
    else if (/^cards-book\//.test(unit)) command = `${ZD_DIGEST} book-cards-finish -Out "${OUT}" -Work ${psQuote(extra.work || `${OUT}\\cards`)} -Unit "${unit}" -Token ${token}`
    else if (/^terms\//.test(unit)) command = `${ZD_LEDGER} done -Out "${OUT}" -Unit "${unit}" -Token ${token} -Outputs ${psQuote(`${OUT}\\terms\\completion.json`)}`
    else if (/^cleanup\//.test(unit)) command = `${ZD_DIGEST} cleanup-finish -Out "${OUT}" -Unit "${unit}" -Token ${token}`
    else return { ok: false, error: `no checked finish command for ${unit}` }
    if (extra.checkOnly) command += ' -CheckOnly'
    const out = await runScript(label, o.phase || LEDGER_PHASE, command, ' (a checked completion; never retry it with other options)')
    return { ok: !!(out.run && out.run.ok), json: out.json, command, error: out.run && out.run.output }
  }
  // One claimed unit's checked finish. A failure is recorded against that unit's own attempt
  // (`ledger fail`): a transient transport or lease fault schedules the retry, a checked content
  // failure keeps the unit failed and visible. Never a whole-stage rerun (R258).
  const finishOrFault = async (label, unit, token, inputs, extra = {}) => {
    const at = `${label}: ${unit}`
    const got = await finishOf(at, unit, token, { ...extra, inputs }, { inputs })
    const done = got.ok && got.json && (got.json.state === 'done' || got.json.complete === true)
    if (done) return { unit, status: got.json.status === 'partial' ? 'partial' : 'done', result: got.json }
    const cause = String((got.error && got.error.message) || got.error || (got.json && (got.json.fault || got.json.error)) || 'the checked completion did not complete').slice(0, 800)
    const retryable = /stale_claim|read_view_changed|lock_busy|sharing_violation|sqlite_busy|transport_closed|command_timeout|service_unavailable|provider_|missing_result|pending_jobs|cleanup_transient/.test(cause)
    const fault = {
      schema_version: 1, unit, input_hash: (inputs && inputs.input_hash) || '',
      code: retryable ? 'provider_transient' : 'check_failed', cause,
    }
    await failUnit(`${at} fault`, unit, token, retryable ? 'retryable' : 'content', fault)
    return { unit, status: retryable ? 'retry' : 'failed', error: cause, command: got.command }
  }
  // --- Owned DeepSeek resources (R322–R330) -----------------------------------------------------------
  // Every created conversation is recorded the moment the provider returns its id, against the work that
  // created it: the unit a chain claimed, or the run-level cleanup unit for a stage-wide chain. The
  // run-level cleanup deletes exactly the recorded ids; nothing is deleted by title.
  const RESOURCES = `${OUT}\\resources`
  // A monotone counter names the record files: workflow scripts have no clock and no randomness.
  let resourceSeq = 0
  const resourceFile = (stem) => `${RESOURCES}\\${stem}-${++resourceSeq}.json`
  const resourceOwner = (o = {}) => {
    if (o.unit) return { unit: o.unit, token: Number(o.token) || 0 }
    return 'run'
  }
  // R343–R345: exact per-run usage. A manager copies the numbers the provider attached to that run's own
  // result; the ledger deduplicates them by run id, so reading one result twice or continuing a run never
  // adds its cost twice. The account-wide usage ledger is never read here: the difference of two rolling
  // totals is not a run's cost (R344).
  const measuredUsage = (value) => {
    if (!value || typeof value !== 'object') return null
    const usage = {}
    for (const [name, number] of Object.entries(value)) {
      if (typeof number === 'number' && Number.isFinite(number) && number >= 0) usage[name] = number
    }
    return Object.keys(usage).length ? usage : null
  }
  const registerResources = async (label, owner, dsh, results) => {
    const items = (results || []).filter((r) => r && typeof r.session_id === 'string' && r.session_id.trim())
    if (!items.length) return { ok: true, recorded: 0 }
    const unit = owner === 'run' ? 'run' : owner.unit
    const token = owner === 'run' ? 1 : owner.token
    if (unit !== 'run' && !token) return { ok: false, recorded: 0, error: 'a recorded run needs the attempt token of its unit' }
    const file = resourceFile(`${String(unit).replace(/[^A-Za-z0-9._-]+/g, '-')}-t${token}`)
    // R325: the accepted answer travels with the id, so the command retains it under the durable
    // evidence path at once; a failed run keeps an explicit line instead of an empty claim.
    const conversations = items.map((r) => ({
      id: r.session_id,
      answer: typeof r.answer === 'string' && r.answer.trim() ? r.answer : `no answer: ${r.status || 'failed'}`,
    }))
    const record = { schema_version: 1, unit, token, workspace: { id: dsh, path: dsh }, conversations }
    const command = `New-Item -ItemType Directory -Force -LiteralPath "${RESOURCES}" | Out-Null; Set-Content -LiteralPath "${file}" -Value ${psQuote(JSON.stringify(record))} -Encoding utf8; ${ZD_DIGEST} ledger resource -Out "${OUT}" -Unit "${unit}" -Token ${token} -Register "${file}"`
    const out = await runScript(label, LEDGER_PHASE, command)
    const parsed = out.json
    if (!parsed || !parsed.cleanup_unit) {
      return { ok: false, recorded: 0, error: String((out.run && out.run.output) || 'the created run ids were not recorded') }
    }
    // The provider's own per-run numbers, when the manager had them. A run without them stays
    // missing coverage in the report; nothing is estimated from the account total.
    const resourceId = (parsed.resources || [])[0] && parsed.resources[0].id
    let usageRuns = 0
    if (resourceId) {
      for (const r of items) {
        const usage = measuredUsage(r.usage)
        if (!usage) continue
        const got = await observeResource(`${label}: usage ${r.session_id}`, {
          op: 'usage', resource: resourceId, conversation: r.session_id, usage,
          ...(r.usage_currency ? { currency: r.usage_currency } : {}),
        })
        if (got.ok) usageRuns += 1
      }
    }
    return { ok: true, recorded: items.length, cleanup_unit: parsed.cleanup_unit, label: parsed.label, usage_runs: usageRuns }
  }
  // R331: the run report. It reads the ledger and verifies the current placeholder state, writes
  // run-report.json and run-report.md, and prints the same facts ledger status reports.
  const ledgerReport = async (label) => {
    const out = await runScript(label, CLEANUP_PHASE, ledgerCommand(`report -Out "${OUT}"`), ' (it writes run-report.json and run-report.md)')
    if (!out.json) return { ok: false, error: String((out.run && out.run.output) || 'no output from ledger report') }
    return { ok: true, json: out.json }
  }
  // The recorded resources of this run, exactly as the ledger holds them (never a workspace listing alone).
  const resourceList = async (label, unit = '') => {
    const out = await runScript(label, LEDGER_PHASE, ledgerCommand(`resource -Out "${OUT}" -List${unit ? ` -Unit "${unit}"` : ''}`))
    if (!out.json) return { ok: false, error: String((out.run && out.run.output) || 'no output from ledger resource'), resources: [] }
    return { ok: true, resources: out.json.resources || [] }
  }
  // Record one provider fact about a recorded resource. A claimed cleanup unit fences the observation
  // with its own token, so a late fact cannot apply to another attempt (T13).
  const observeResource = async (label, record, unit = '', token = 0) => {
    const file = resourceFile('observe')
    const body = { schema_version: 1, ...record }
    const fence = unit && token ? ` -Unit "${unit}" -Token ${token}` : ''
    const command = `New-Item -ItemType Directory -Force -LiteralPath "${RESOURCES}" | Out-Null; Set-Content -LiteralPath "${file}" -Value ${psQuote(JSON.stringify(body))} -Encoding utf8; ${ZD_DIGEST} ledger resource -Out "${OUT}" -Observe "${file}"${fence}`
    const out = await runScript(label, LEDGER_PHASE, command)
    return { ok: !!(out.run && out.run.ok) && !!out.json, json: out.json, error: String((out.run && out.run.output) || '') }
  }
  // The checked completion of one cleanup unit; a partial completion records what a human must reconcile.
  const finishCleanup = async (label, unit, token, checkOnly = false) => {
    const command = `${ZD_DIGEST} cleanup-finish -Out "${OUT}" -Unit "${unit}" -Token ${token}${checkOnly ? ' -CheckOnly' : ''}`
    const out = await runScript(label, CLEANUP_PHASE, command, ' (the checked cleanup completion; never retry it with other options)')
    return { ok: !!(out.run && out.run.ok), json: out.json, command, error: String((out.run && out.run.output) || '') }
  }
  // One unit's full attempt: claim is done by the caller; the work runs; the family finish records it.
  const drain = async (layerName, unit, token, inputs, work) => {
    let outcome
    try {
      outcome = await work({ unit, token, inputs, at: `${layerName}: ${unit}` })
    } catch (error) {
      outcome = { ok: false, retryable: true, error: String((error && error.message) || error) }
    }
    if (outcome && outcome.ok) {
      const finished = await finishOrFault(`${layerName} ${unit}`, unit, token, inputs, outcome.extra || {})
      return { ...finished, result: (finished.result || outcome.result) }
    }
    const retryable = outcome ? outcome.retryable !== false : true
    const fault = {
      schema_version: 1, unit, input_hash: (inputs && inputs.input_hash) || '',
      code: (outcome && outcome.code) || (retryable ? 'provider_transient' : 'check_failed'),
      cause: String((outcome && outcome.error) || 'the unit did not complete').slice(0, 800),
    }
    await failUnit(`${layerName}: ${unit} fault`, unit, token, retryable ? 'retryable' : 'content', fault)
    return { unit, status: retryable ? 'retry' : 'failed', error: fault.cause }
  }
  // The driver of one layer (R252-R257): claim ready units in bounded blocks, work each unit's own chain,
  // record every result, and settle with the layer gate. `wait` is the tool-side readiness wait in seconds;
  // `producer` names the layer whose settled enumeration means no further unit can appear. Returns
  // complete/partial/active only after the gate says so.
  const layer = async (name, o) => {
    const layerName = o.layer || name
    const take = Math.max(1, Math.min(64, o.take || 1))
    const rounds = Math.max(1, Math.min(12, o.rounds || MAX_ATTEMPTS))
    const dispositions = []
    let claimedCount = 0
    let gateResult = null
    for (let round = 1; round <= rounds; round++) {
      const got = await claim(`${name} claim ${round}`, layerName, take, o.wait === undefined ? 0 : o.wait)
      if (!got.ok) return { status: 'failed', layer: layerName, error: got.error, dispositions }
      const units = got.units || []
      if (units.length) {
        claimedCount += units.length
        const results = await o.work(units)
        for (const r of results) dispositions.push(r)
        continue
      }
      // Nothing ready: settle only on the gate's own facts, never on an empty queue.
      gateResult = await gate(`${name} gate ${round}`, layerName)
      if (gateResult.complete === true) return { status: 'complete', layer: layerName, gate: gateResult, dispositions, claimed: claimedCount }
      const open = (gateResult.counts && (gateResult.counts.open || 0)) || 0
      if (!o.producer && open === 0) {
        // A settled layer keeps an explicit partial result when a unit failed; an unsettled one is active.
        const failed = dispositions.filter(d => d.status === 'failed').length
        return { status: failed ? 'partial' : 'active', layer: layerName, gate: gateResult, dispositions, claimed: claimedCount }
      }
      if (o.producer) {
        const produced = await gate(`${name} producer ${round}`, o.producer)
        if (produced.settled === true) break
      }
      if (o.wait === undefined) break
    }
    gateResult = await gate(`${name} gate`, layerName)
    const failed = dispositions.filter(d => d.status === 'failed').length
    const open = (gateResult.counts && (gateResult.counts.open || 0)) || 0
    return {
      status: gateResult.complete === true ? 'complete' : failed && !open ? 'partial' : 'active',
      layer: layerName, gate: gateResult, dispositions, claimed: claimedCount,
    }
  }
  // --- Cards: the card layer cycle -----------------------------------------------------------------
  // One cycle over one scope, run as ONE manager's chain: plan → the jobs it prints → apply in shares
  // until none is pending → check → its follow-up jobs → apply → report (R250/R251). The bounded
  // correction path is declared here, not improvised. o.modelReview: this whole-book cycle also reviews
  // the Model drafts no model chain owns (R247); a draft an active chain owns reuses that chain's review.
  const runCards = async (label, work, extra = '', only = '', o = {}) => {
    const cstep = (s, more = '') => `${CARD_COMPILER} -Step ${s} -Work "${work}" -Compact${more}`
    const rules = only ? ` -Rules "${only}"` : A.cardRules ? ` -Rules "${A.cardRules}"` : ''
    const max = A.cardMax ? ` -MaxCards ${A.cardMax}` : ''
    const models = o.modelReview ? ' -ModelReview' : ''
    const applyStep = (n) => ({ name: `apply ${n}`, command: cstep('apply', ` -MaxJobs ${APPLY_SHARE}`), repeat: { path: 'pending_jobs', value: 0, max: 100 } })
    // R299: the finalization spends its one declared correction cycle, so it passes fixRounds 1
    // whatever the run's cardFixRounds pilot setting is.
    const rounds = o.fixRounds === undefined ? CARD_FIX_ROUNDS : Math.max(0, Math.min(12, o.fixRounds))
    const steps = [{ name: 'plan', command: cstep('plan', ` -Book ${KEY} -Staging "${OUT}"${rules}${max}${models}${extra}`), jobs: true }, applyStep(1)]
    for (let round = 1; round <= rounds; round++) {
      steps.push({ name: `check ${round}`, command: cstep('check'), jobs: true })
      steps.push(applyStep(round + 1))
    }
    steps.push({ name: 'report', command: cstep('report'), optional: true })
    const chained = await chain(`cards ${label}`, {
      dsh: work, skills: CARD_SKILLS, effort: DSH_EFFORT, phase: 'Cards', writes: `${work}\\out`, steps, cleanup: true, keep: true,
    })
    const rawPlan = jsonOf(chained, 'plan')
    const plan = rawPlan ? { ...rawPlan, jobs: expandJobs(rawPlan) } : null
    const applied = steps.filter(s => /^apply /.test(s.name)).map(s => jsonOf(chained, s.name)).filter(Boolean).pop() || null
    const reported = jsonOf(chained, 'report')
    const jobs = (chained.results || []).length
    if (!plan) {
      log(`cards ${label}: plan failed: ${((stepOf(chained, 'plan') || {}).error) || 'no output'}.`)
      return { status: 'failed', label, error: `plan failed: ${((stepOf(chained, 'plan') || {}).error) || 'no output'}` }
    }
    log(`Cards ${label}: ${plan.picked} Cards of the book; ${plan.with_faults} with ${plan.faults} faults, ${plan.review} for review, ${plan.orphans} orphans; ${plan.jobs.length} DeepSeek jobs.`)
    const checked = steps.filter(s => /^check /.test(s.name)).map(s => jsonOf(chained, s.name)).filter(Boolean)
    for (const c of checked) log(`Cards ${label}: ${c.faults} faults left, ${(c.jobs || []).length} follow-up jobs.`)
    if (reported) log(`Cards ${label}: faults ${reported.faults_before} → ${reported.faults_after} on ${reported.cards} Cards; verdicts ${JSON.stringify(reported.verdicts)}; ${reported.splits} splits, ${reported.merges} merge proposals, ${reported.refused} refused; ${reported.orphan_links} orphan Links (${reported.orphans_after} orphans left); ${jobs} DeepSeek jobs.`)
    const applyFailed = steps.filter(s => /^apply /.test(s.name)).some(s => jsonOf(chained, s.name) === null)
    if (applyFailed) log(`Cards ${label}: an apply step failed; the cycle is marked failed.`)
    return { status: reported && !applyFailed ? 'done' : 'failed', label, plan, applied, report: reported, ...(applyFailed ? { error: 'apply failed: no output from the card compiler -Step apply' } : {}) }
  }
  return {
    A, MODE, KEY, OUT, SCRIPTS, DSH_MODEL, DSH_EFFORT, MODELS, CARD_CHECKS, IDEA_SAMPLE, LIMITS, COMPILER, FILL_SKILLS,
    HUB_WORK, HUB_COMPILER, HUB_EFFORT, HUB_FIX_ROUNDS, HUB_SKILLS, PER_MANAGER, LINK_PER_MANAGER, raised,
    MAX_ATTEMPTS, DSH_RUN_CAP, RUNNER, COMMAND_TIMEOUT_MS: ZD_COMMAND_TIMEOUT_MS,
    // The command prefixes and skills a stage module builds a chain with.
    LEDGER: ZD_LEDGER, DIGEST: ZD_DIGEST, CALL: ZD_CALL, DSH_SKILLS,
    zAgent, runScript, groupsOf, toBlocks, runStage, drive, expandJobs, answerLines,
    // The ready-work driver (9.3): one manager per declared chain, plus the ledger adapter.
    chain, stepOf, jsonOf, resultLine, claim, gate, renew, failUnit, jobOf, finishOf, finishOrFault, drain, layer, psQuote,
    // The command that runs a digest script: zt-digest <tool> once ported, else pwsh with the script.
    tool: name => zdTool(name, SCRIPTS),
    // The cleanup round alone: delete every conversation of o.dsh and its workspace, even when no command ran.
    cleanupDsh: () => ({ ok: true, results: [], deleted: 0, deferred: true }),
    // Owned provider resources (R322-R330): record the returned ids, observe the provider's facts, settle the unit.
    registerResources, observeResource, resourceList, finishCleanup, resourceOwner, CLEANUP_PHASE, ledgerReport,
    runCards, profile, ARCH_DOC, ARCH_SUPPLEMENT, MODEL_WORK, PER_SONNET, PER_CONCEPT, DSH_PER_BATCH, REBUILD, LEVEL,
    BRIEF_EFFORT, DERIVE_EFFORT, REVIEW_EFFORT, REVIEW_ROUNDS, COMPILER_DOCS, SKIP_FORMS,
  }
}

defineModule({
  id: 'zt-digest-lib', version: '11', library: true, requires: ['@dsmp'],
  summary: 'Library: the ZT digest machinery (settings, managers, one-manager chains, the ledger ready-work driver, the model compiler, the card cycle) for the zt-* stage modules.',
})
// ==== end module zt-digest-lib ====

