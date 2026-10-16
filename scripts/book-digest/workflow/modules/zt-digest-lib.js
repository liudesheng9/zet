// ==== module zt-digest-lib v1 ====
// The shared machinery of the ZT digest workflows (doc/zt-digest-framework.md), ported from
// .claude/workflows/zt-book-digest.js: the settings read from the args, the digest and link managers, the
// compiler-step driver (drive), the model compiler (compileBatch, elaborateAll), and the card layer cycle
// (runCards). A library module: the stage modules call zdMake(ctx, params) and use what it returns.
// Every Haiku, Sonnet, and Opus agent goes through ctx.agent, so the blueprint's roles decide the models:
// manager (Haiku), architect (Sonnet), organizer (Opus). Managers share the DeepSeek Subagent Managed
// Protocol pool (dsmpSlot): at most 8 managers alive in the whole run.
const ZD_SCRIPTS = 'E:\\zet\\scripts\\book-digest'
const ZD_HELPER = 'C:\\Users\\chenx\\.codex\\skills\\zt\\scripts\\zt-mcp.ps1'
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
  const COMPILER = `${SCRIPTS}\\model-compiler.ps1`
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
  const HUB_COMPILER = `${SCRIPTS}\\hub-compiler.ps1`
  const HUB_EFFORT = atLeastHigh(A.hubEffort, 'hubEffort')
  const HUB_FIX_ROUNDS = Math.max(0, A.hubFixRounds ?? 1)
  const HUB_SKILLS = ['asd-ste100-zh']
  const CARD_CHECKS = MODE === 'cards' || ((MODE === 'full' || MODE === 'link') && A.cardChecks !== false)
  const CARD_COMPILER = `${SCRIPTS}\\card-compiler.ps1`
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
  a. Snapshot the slip-box (one PowerShell call): pwsh -NoProfile -File ${SCRIPTS}\\snapshot-topics.ps1 -Out "${g.snapshot}"
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
  - "needs_response": approve (decision "allow") a pwsh command that runs zt-mcp.ps1, and read-only reads or searches (Read, Get-Content, Select-String) of files under "${OUT}" or of "${A.source}"; reject anything that writes, moves or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
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

  // --- Models: the model compiler ------------------------------------------------------------------
  const MODEL_LIST = {
    type: 'object',
    properties: {
      ok: { type: 'boolean' }, error: { type: 'string' },
      models: { type: 'array', items: { type: 'object', properties: { address: { type: 'string' }, title: { type: 'string' }, level: { type: ['string', 'null'] }, parts: { type: 'number' }, issues: { type: 'number' }, faults: { type: 'number' } }, required: ['address'] } },
    },
    required: ['ok', 'models'],
  }
  const listModels = (selector, label) => zAgent(
    `List the Model Cards of a ZET book digest. Run this one PowerShell command and read its last output line, a compact JSON object {"models":[{"address","title","group","level","parts","issues","faults"}]}:
pwsh -NoProfile -File ${SCRIPTS}\\list-models.ps1 -Staging "${OUT}" ${selector}
If it fails, return ok=false with the error text and models=[]; do not retry with other options. Otherwise return ok=true and the models array exactly.`,
    { label, phase: 'Models', model: 'haiku', schema: MODEL_LIST },
  )
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
- "needs_response": approve (decision "allow") a pwsh command that runs zt-mcp.ps1 or card-style-check.ps1; a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}; and ${o.writes ? `writing JSON files under "${o.writes}"` : `writing the one file under "${o.dsh}" that the job's prompt names`}. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
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

  const PLANS = {
    type: 'object',
    properties: {
      plans: { type: 'array', items: { type: 'object', properties: {
        model: { type: 'string' }, status: { type: 'string', enum: ['planned', 'skipped', 'failed'] }, level: { type: 'string' },
        parts: { type: 'number' }, derivations: { type: 'number' }, chains: { type: 'number' },
        notes: { type: 'array', items: { type: 'string' } }, summary: { type: 'string' },
      }, required: ['model', 'status'] } },
    },
    required: ['plans'],
  }
  const slug = (address) => String(address).replace(/[\/|]/g, '_')
  const managerFolders = []
  let dshJobs = 0
  const compileBatch = async (batch, kind) => {
    const name = `m-${slug(batch[0].address)}`
    const dsh = `${MODEL_WORK}\\${name}`
    managerFolders.push(dsh)
    const tag = `${kind} ${batch[0].address}${batch.length > 1 ? ` +${batch.length - 1}` : ''}`
    const step = (s, extra = '') => `pwsh -NoProfile -File "${COMPILER}" -Step ${s} -Work "${dsh}" -Staging "${OUT}" ${LIMITS}${COMPILER_DOCS}${extra}`
    const sessions = {}
    const run = async (what, command, skills, effort, opts = {}) => {
      const r = await drive(`${what}: ${tag}`, { command, dsh, skills, effort, sessions: opts.continues ? sessions : null, cleanup: opts.cleanup })
      for (const x of (r && r.results) || []) {
        dshJobs++
        if (x.session_id) sessions[x.id] = x.session_id
      }
      if (!r || !r.ok) log(`${what} ${tag}: ${(r && r.error) || 'no report from its driver'}`)
      return r
    }
    const splitNeeds = async (driven) => {
      const need = answerLines(driven).filter(l => l.status === 'split' && l.address && l.need > 1).map(l => `${l.address}=${l.need}`)
      if (need.length) await run('split', step('split', ` -Need "${need.join(';')}"`), FILL_SKILLS, DERIVE_EFFORT, { continues: true })
    }
    const failed = (why) => batch.map(m => ({ model: m.address, status: 'failed', parts: [], charts: 0, issues: [why] }))
    // 1. Brief: the script writes each context pack; DeepSeek reads the source and writes brief.md.
    const brief = await run('brief', step('brief', ` -Models "${batch.map(m => m.address).join(',')}"${REBUILD ? ' -Rebuild' : ''}${LEVEL ? ` -Level ${LEVEL}` : ''}`), [], BRIEF_EFFORT)
    const briefed = zdLastJson(brief && brief.output)
    if (!briefed) return failed(`brief failed: ${(brief && brief.error) || 'no output'}`)
    const skipped = new Set(briefed.skipped || [])
    const todo = batch.filter(m => !skipped.has(m.address))
    if (!todo.length) return batch.map(m => ({ model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }))
    // 2. Plan: Sonnet reads the briefs and writes plan.json for each model; nothing else.
    const entry = (m, i) => `## Model ${i + 1}
MODEL: ${m.address} (${m.title || ''})
DIR: ${dsh}\\${slug(m.address)}
LEVEL NOW: ${m.level || 'none'}`
    const planned = await zAgent(
      `You are the Sonnet architect in the Models stage of the ZET digest of 《${A.bookTitle || KEY}》. Read your instructions first: ${ARCH_DOC}${ARCH_SUPPLEMENT ? `, then ${profile.architectSupplement.name || 'the supplement'} ${ARCH_SUPPLEMENT}` : ''} (Read tool). Then follow them for each model in turn. These models are near each other in the book: where one part serves two of them, plan it in the first and take it in from the second (\`<model>#<id>\`).
COMPILER: ${COMPILER}
WORK: ${dsh}
LIMITS: ${LIMITS}${REBUILD ? '\nREBUILD: rebuild each model even if it already has parts.' : ''}${LEVEL ? `\nLEVEL: ${LEVEL}` : ''}

${todo.map(entry).join('\n\n')}

Return plans: one entry per model, in this order.`,
      { label: `plan: ${tag}`, phase: 'Models', model: 'sonnet', schema: PLANS },
    )
    const plans = (planned && planned.plans) || []
    if (!plans.some(p => p.status === 'planned')) {
      await run('clean up', step('report'), [], DSH_EFFORT, { cleanup: true })
      return batch.map(m => skipped.has(m.address) ? { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
        : { model: m.address, status: (plans.find(p => p.model === m.address) || {}).status || 'failed', parts: [], charts: 0, issues: planned ? [] : ['no plan from the architect'] })
    }
    // 3. Fill, 4. derive and 5. review (only where the plans have Derivation parts).
    await splitNeeds(await run('fill', step('apply'), FILL_SKILLS, DSH_EFFORT))
    if (plans.some(p => p.status === 'planned' && p.derivations > 0)) {
      await splitNeeds(await run('derive', step('derive'), FILL_SKILLS, DERIVE_EFFORT))
      for (let round = 0; round < REVIEW_ROUNDS; round++) {
        const review = await run(`review ${round + 1}`, step('review', round ? ' -Changed' : ''), [], REVIEW_EFFORT)
        const verdicts = answerLines(review).reduce((s, l) => ({ split: s.split + (Number(l.split) || 0), fix: s.fix + (Number(l.fix) || 0) }), { split: 0, fix: 0 })
        log(`review ${round + 1} ${tag}: ${verdicts.split} parts to split, ${verdicts.fix} to fix`)
        if (!verdicts.split && !verdicts.fix) break
        await splitNeeds(await run(`split ${round + 1}`, step('split'), FILL_SKILLS, DERIVE_EFFORT, { continues: true }))
      }
    }
    // 6. Report (script) and clean-up.
    const reported = zdLastJson((await run('report', step('report'), [], DSH_EFFORT, { cleanup: true }) || {}).output)
    const reports = (reported && reported.models) || []
    return batch.map(m => {
      const r = reports.find(x => x.model === m.address)
      const pl = plans.find(x => x.model === m.address) || {}
      if (skipped.has(m.address)) return { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
      if (!r) return { model: m.address, status: 'failed', parts: [], charts: 0, issues: ['no report from the compiler'] }
      const faults = Object.entries(r.faults || {}).filter(([, n]) => n).map(([k, n]) => `${k} ${n}`)
      return {
        ...r,
        issues: [...(r.issues || []), ...(r.error ? [r.error] : []), ...faults, ...((r.todo || []).length ? [`still 待写: ${r.todo.join(', ')}`] : [])],
        notes: [...(r.notes || []), ...(pl.notes || [])], summary: pl.summary,
      }
    })
  }
  const groupNo = (m) => Number(m.group) || Number(String(m.address).split('/')[1].split('|')[0]) || 0
  const elaborateAll = async (models, label) => {
    const only = Array.isArray(A.only) && A.only.length ? new Set(A.only) : null
    // A model whose level the run changes is never skipped; a form the profile skips (math: theory) never goes in.
    const todo = models.filter(m => !SKIP_FORMS.includes(m.form)).filter(m => !only || only.has(m.address)).filter(m => REBUILD || (LEVEL && m.level !== LEVEL) || !(m.parts > 0 && !m.issues))
      .sort((a, b) => groupNo(a) - groupNo(b) || String(a.address).localeCompare(String(b.address)))
    if (todo.length < models.length) log(`${label}: ${models.length - todo.length} of ${models.length} models already have their parts; skipped.`)
    const concept = todo.filter(m => (LEVEL || m.level) === 'concept')
    const formal = todo.filter(m => (LEVEL || m.level) !== 'concept')
    const batches = []
    for (let i = 0; i < concept.length; i += PER_CONCEPT) batches.push({ kind: 'concept', models: concept.slice(i, i + PER_CONCEPT) })
    for (let i = 0; i < formal.length; i += PER_SONNET) batches.push({ kind: 'formal', models: formal.slice(i, i + PER_SONNET) })
    log(`${label}: ${todo.length} models (${concept.length} concept, ${formal.length} formal or undecided) in ${batches.length} batches, one Sonnet architect each; up to ${DSH_PER_BATCH} DeepSeek runs at a time per batch.`)
    const results = (await parallel(batches.map(b => () => compileBatch(b.models, b.kind).catch(error => { log(`batch ${b.models[0].address}: ${error}`); return null })))).flatMap((r, i) => r || batches[i].models.map(() => null))
    const ordered = batches.flatMap(b => b.models)
    return results.map((r, i) => r || { model: ordered[i].address, status: 'failed', parts: [], charts: 0, issues: ['no report from its batch'] })
  }
  const elaborateBlock = async (block, managed) => {
    const reports = (managed && managed.reports) || []
    const groups = block.map(g => g.n).filter(n => {
      const r = reports.find(x => x.group === n)
      return r && r.status === 'done' && r.models !== 0
    })
    if (!groups.length) return []
    const listed = await listModels(`-Groups ${groups.join(',')}`, `find models: groups ${groups[0]}-${groups[groups.length - 1]}`)
    if (!listed || !listed.ok) { log(`Models for groups ${groups.join(',')}: listing failed (${listed && listed.error})`); return [] }
    const reported = reports.filter(r => groups.includes(r.group)).reduce((s, r) => s + (r.models || 0), 0)
    if (reported && !listed.models.length) log(`Models for groups ${groups.join(',')}: the runs reported ${reported} Model Cards but the listing found none; the whole-book sweep will pick them up.`)
    return elaborateAll(listed.models, `groups ${groups[0]}-${groups[groups.length - 1]}`)
  }
  const kindTally = (reports) => {
    const tally = {}
    for (const r of reports) for (const [kind, n] of Object.entries(r.kinds || {})) tally[kind] = (tally[kind] || 0) + n
    return Object.entries(tally).sort((a, b) => b[1] - a[1]).map(([kind, n]) => `${kind} ${n}`).join(', ') || 'no kinds reported'
  }
  const logModels = (reports) => {
    const done = reports.filter(r => r.status === 'done')
    const sum = (field) => done.reduce((s, r) => s + (r[field] || 0), 0)
    const deriv = (field) => done.reduce((s, r) => s + ((r.derivations && r.derivations[field]) || 0), 0)
    log(`Build: ${deriv('total')} Derivation parts in ${deriv('chains')} chains of two or more (longest ${done.reduce((m, r) => Math.max(m, (r.derivations && r.derivations.longest) || 0), 0)}), ${sum('split')} parts split by review or for the Card size, ${done.reduce((s, r) => s + ((r.moved || []).length), 0)} Cards moved into ranges, ${sum('shared')} shared parts taken in, ${sum('tolerated')} tolerated premises, ${sum('style')} style faults left, ${done.reduce((s, r) => s + ((r.todo || []).length), 0)} parts still 待写; ${dshJobs} DeepSeek jobs.`)
    log(`Levels: ${done.filter(r => r.level === 'concept').length} concept models (level 1), ${done.filter(r => r.level === 'formal').length} formal models (level 2), ${done.filter(r => !r.level).length} without a level.`)
    log(`Models: ${done.length} structured (${done.reduce((s, r) => s + r.parts.length, 0)} parts, ${done.reduce((s, r) => s + r.parts.filter(pp => pp.emphasis === 'core').length, 0)} core, ${sum('relations')} relations (${kindTally(done)}), ${sum('neighbours')} neighbours, ${sum('loose')} loose, ${sum('charts')} charts), ${reports.filter(r => r.status === 'skipped').length} skipped, failed ${reports.filter(r => r.status === 'failed').map(r => r.model).join(', ') || 'none'}; open issues ${reports.reduce((s, r) => s + r.issues.length, 0)}.`)
  }
  // A batch's cleanup deletes its own DeepSeek conversations; this sweep deletes any a failed batch left.
  const modelCleanup = async () => {
    if (!managerFolders.length) return null
    const c = await zAgent(
      `Clean up after the batches of a ZET model stage (a batch deletes its own conversations; this catches a batch that failed). Load tools with ToolSearch query "select:mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_delete,mcp__deepseek__deepseek_remove_workspace".
For each folder below: deepseek_list with that cwd (a workspace that is not found needs nothing); deepseek_delete every conversation there that is not running, and count them; then, when none is running, deepseek_remove_workspace with that cwd and delete_conversations true. Never archive a conversation.
${managerFolders.join('\n')}
Then delete what is left of those folders with one PowerShell call: ${managerFolders.map(f => `Remove-Item -LiteralPath "${f}" -Recurse -Force -ErrorAction SilentlyContinue`).join('; ')}
Return late_deleted (the count) and folders (how many folders you handled).`,
      {
        label: 'cleanup model managers', phase: 'Models', model: 'haiku',
        schema: { type: 'object', properties: { late_deleted: { type: 'number' }, folders: { type: 'number' } }, required: ['late_deleted'] },
      },
    )
    if (c) log(`Model clean-up: ${c.late_deleted} conversations deleted late.`)
    return c
  }

  // --- Cards: the card layer cycle -----------------------------------------------------------------
  const runCards = async (label, work, extra = '', only = '') => {
    const cstep = (s, more = '') => `pwsh -NoProfile -File "${CARD_COMPILER}" -Step ${s} -Work "${work}" -Compact${more}`
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
    return { status: reported && applied ? 'done' : 'failed', label, plan, applied, report: reported, ...(applied ? {} : { error: 'apply failed: no output from card-compiler.ps1 -Step apply' }) }
  }
  const cardBlock = async (block) => {
    const groups = block.map(g => g.n).join(',')
    return runCards(`groups ${block[0].n}-${block[block.length - 1].n}`, `${OUT}\\cards-g${String(block[0].n).padStart(2, '0')}`, ` -Groups "${groups}" -Sample ${IDEA_SAMPLE} -NoOrphans`)
  }

  return {
    A, MODE, KEY, OUT, SCRIPTS, DSH_MODEL, DSH_EFFORT, MODELS, CARD_CHECKS, IDEA_SAMPLE, LIMITS, COMPILER, FILL_SKILLS,
    HUB_WORK, HUB_COMPILER, HUB_EFFORT, HUB_FIX_ROUNDS, HUB_SKILLS, PER_MANAGER, LINK_PER_MANAGER, raised,
    zAgent, runScript, groupsOf, toBlocks, runStage, listModels, drive, expandJobs, answerLines, elaborateAll, elaborateBlock,
    logModels, modelCleanup, runCards, cardBlock, dshJobs: () => dshJobs,
  }
}

defineModule({
  id: 'zt-digest-lib', version: '1', library: true, requires: ['@dsmp'],
  summary: 'Library: the ZT digest machinery (settings, managers, drive, model compiler, card cycle) for the zt-* stage modules.',
})
// ==== end module zt-digest-lib ====
