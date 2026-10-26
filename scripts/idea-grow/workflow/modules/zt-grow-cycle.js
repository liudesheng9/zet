// ==== module zt-grow-cycle v14 ====
// The idea-grow compiler cycle (docs/idea-grow-design.md). v12 (zt-idea-grow 4.0.0, docs/workflow-simplicity-proposal.md
// S1-S4, S6, S7, S9) is one loop with three branches. `zt-digest grow -Step next` runs every deterministic step it can
// (plan, topics, apply, links, check, the restatement and concept reviews, the Topic maintenance, the hubs and the
// register, the report) in the order v11 ran them, and stops at the next DeepSeek job set or at the end. Its line is
//   {"next":"jobs",…}  -> the jobs go to DSMP dsDispatch, then `next -After <token>` runs again;
//   {"next":"done",…}  -> the stage ends with that line as its report;
//   {"next":"fault",…} -> the stage stops (a `card_locked` fault with the owner's refusal; S6).
// The command decides every branch; no agent decides control flow (S3).
// v13 (#179, docs/workflow-module-structure-proposal.md): the chain is ONE script step. `grow-steps.ps1 -Step next`
// runs the command, checks its line (shape, FNV-1a checksum of the job set) and reads `grow -Step status` once when
// the line is unreadable (S2), then prints the checked line. A DeepSeek drive job runs that step through a Haiku
// manager (zt-workflow-lib's shared ztDrive); no Claude agent runs a single command, and this module names no relay
// helper. A step that answers {ok:false,…} is a failed step: the stage stops with its error.
// v14 (#180): a step's answer is at most 400 characters, so it never carries a job template. `next` expands a job
// set to `<work>\grow-jobs.json` and answers its count and path; one Haiku manager per eight jobs reads that file
// and runs its entries (the same work DSMP dsDispatch did, from a file the step wrote). The done line carries the
// report's counts; the fault line its code and cause.
defineModule({
  id: 'zt-grow-cycle', version: '14', requires: ['deepseek', '@dsmp', 'zt-workflow-lib'],
  summary: 'ZT idea grow: one loop over `zt-digest grow -Step next`, whose checked line one grow-steps.ps1 step (a DeepSeek drive job) prints (jobs -> one manager reading the job file, done, fault); the command plans, applies, checks, reviews, builds the hubs and the register and reports; a write the owner\'s Lock refused stops the stage with the typed card_locked fault.',
  params: {
    book: { type: 'string', description: 'one Literature tree to grow from (its Citation key)' },
    books: { type: 'array', description: 'the Literature trees to grow from; with neither book nor books, every Literature tree of the box' },
    work: { type: 'string', required: true, description: 'the work folder (also the DeepSeek workspace)' },
    instructions: { type: 'string', required: true, description: 'the prompt template dsh-grow-instructions.zh.md' },
    tool: { type: 'string', default: 'zt-digest', description: 'the zt-digest command or the path of a zt-digest binary' },
    ztConfig: { type: 'string', description: 'the ZT_CONFIG_DIR of a copy of the box with its own zt service (the run pins its work folder to it); empty: the live box' },
    from: { type: 'number', description: 'the first section number to grow' },
    to: { type: 'number', description: 'the last section number to grow' },
    model: { type: 'string', required: true, description: 'DeepSeek Harness model of the jobs (frozen: the official source)' },
    effort: { type: 'string', default: 'high', enum: ['high', 'max'], description: 'DeepSeek reasoning effort of every job, the hub jobs included (never below high)' },
    hubs: { type: 'boolean', default: true, description: 'run the hubs round (hub compiler and register) after the Cards are written' },
    register: { type: 'string', default: 'apply', enum: ['apply', 'off'], description: 'publish the rebuilt register (apply) or leave it (off)' },
    mode: { type: 'string', default: 'full', description: 'the run\'s mode ({{mode}}): full grows Topic Cards; hubs runs only the concept review, the hubs round and the register' },
    hubTrees: { type: 'string', description: 'only these Topic trees get hubs (comma-separated Topic numbers; a pilot)' },
    hubRebuild: { type: 'boolean', default: false, description: 'rewrite every hub, not only the hubs of trees that changed' },
    hubLimits: { type: 'any', description: 'hub limits that replace the survey\'s (an object; the hub compiler clamps them)' },
    restate: { type: 'boolean', default: true, description: 'run the restatement review of the run\'s new Topic Cards' },
    concepts: { type: 'boolean', default: true, description: 'the concept review of each source book\'s term table, read by the register plan as -Review (needs the hubs round and a register)' },
    organize: { type: 'boolean', default: false, description: 'the Topic maintenance round before the hubs: duplicate-Topic merges and misplaced-Card moves on the run\'s Topics' },
  },
  run: async (ctx, p) => {
    const q = (v) => `'${String(v).replace(/'/g, "''")}'`
    const W = String(p.work).replace(/[\\/]+$/, '')
    const tool = p.tool || 'zt-digest'
    const n = (v) => Number(v || 0)
    // The stage's one step script. The module holds its own default: the kernel passes only the params a blueprint gives.
    const STEPS = 'E:\\zet\\scripts\\idea-grow\\grow-steps.ps1'
    const options = {
      mode: p.mode === 'hubs' ? 'hubs' : 'full', books: Array.isArray(p.books) ? p.books.map(String).filter(Boolean) : [], book: p.book || '',
      instructions: p.instructions, tool, zt_config: p.ztConfig || '', model: p.model, effort: p.effort === 'max' ? 'max' : 'high',
      restate: p.restate !== false, concepts: p.concepts !== false, organize: p.organize === true, hubs: p.hubs !== false, register: p.register === 'off' ? 'off' : 'apply',
      hub_trees: p.hubTrees || '', hub_rebuild: p.hubRebuild === true, hub_limits: p.hubLimits && typeof p.hubLimits === 'object' ? p.hubLimits : null,
      ...(p.from != null ? { from: Number(p.from) } : {}), ...(p.to != null ? { to: Number(p.to) } : {}),
    }
    // The idea-grow run keeps no ledger: the drive job records no resource and only deletes its conversation.
    const deepseek = { deleted: 0, not_deleted: [] }
    // ONE `grow-steps.ps1 -Step next` chain as ONE DeepSeek drive job through a Haiku manager.
    const next = async (after) => {
      const parts = [`pwsh -NoProfile -File "${STEPS}"`, '-Step next', `-Work "${W}"`, `-Options ${q(JSON.stringify(options))}`]
      if (after != null) parts.push(`-After ${Number(after)}`)
      if (p.ztConfig) parts.push(`-ZtConfig ${q(p.ztConfig)}`)
      parts.push(`-Tool ${q(tool)}`)
      const json = await ztDrive(ctx, { label: `${ctx.stage}: next`, command: parts.join(' '), out: W, cwd: W, phase: ctx.phase, record: false, deepseek })
      if (json.ok === false) throw new Error(`grow: ${String(json.error || 'the next step failed').trim().slice(0, 800)}`)
      // Tree Lock (#140, S6): a fault whose code is the owner's typed refusal stops the stage with it.
      if (json.code === ZT_LOCK || json.error === ZT_LOCK) throw ztLockStop(`${ctx.stage}: ${json.round || 'next'}`, json)
      return json
    }
    // One round's job set: one Haiku manager per eight entries of the file the step wrote (#180). The manager reads
    // the entries and runs each one's `run` object exactly, checks its `expect`, retries once and deletes the
    // conversation; it answers one status per job.
    const RESULT = {
      type: 'object',
      properties: {
        ok: { type: 'boolean' },
        jobs: { type: 'array', items: { type: 'object', properties: {
          id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] },
          answer: { type: 'string' }, deleted: { type: 'boolean' },
        }, required: ['id', 'status'] } },
      },
      required: ['ok', 'jobs'],
    }
    const manager = (label, file, from, to) => dsmpSlot(() => ctx.agent(`You are a DeepSeek subagent manager for the ZET idea-grow round (${label}). You start DeepSeek agents, wait for them, collect their answers and delete their conversations. You never write content and never run commands yourself. Keep your messages short.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_cancel,mcp__deepseek__deepseek_delete".

JOBS: read the JSON array in "${file}" (Read tool) and run its entries ${from} through ${to - 1} (0-based), and only those. Each entry has id, run, expect and approve.
1. Start each entry: mcp__deepseek__deepseek_run with the entry's "run" object as the arguments exactly, plus wait_seconds 0. Change nothing else. Start independent jobs in one message.
2. Keep at most 8 jobs running at a time. Poll every open session with deepseek_wait (wait_seconds ${DSMP.pollSeconds}) until it is not running. A job asking a question: answer "按任务说明自行决定，不需要询问。". A job asking approval: use the entry's "approve" rule. A job running longer than ${DSMP.budgetMinutes} minutes: cancel it.
3. A job succeeds when its answer matches the entry's "expect" (a JavaScript regular expression). A job that failed or gave an empty answer gets ONE more attempt in a new conversation. Never a third.
4. Collect each job's final answer verbatim, at most 2000 characters; delete its conversation with mcp__deepseek__deepseek_delete (never archive).
5. Return ok=true and one entry per job: id, session_id, status ("done" or "failed"), answer, deleted.`, { role: 'manager', label, phase: ctx.phase, model: 'haiku', schema: RESULT }))
    const dispatch = async (line) => {
      const count = n(line.jobs)
      const label = `${ctx.stage} ${line.round || 'next'}`
      const managers = []
      for (let from = 0; from < count; from += DSMP.perManager) {
        const to = Math.min(count, from + DSMP.perManager)
        managers.push(manager(`${label} ${from + 1}-${to}`, line.file, from, to).then((r) => (r && r.jobs) || []).catch((error) => {
          log(`grow ${label}: a manager answered no report (${String((error && error.message) || error).slice(0, 160)}); its jobs stay unfinished.`)
          return []
        }))
      }
      return (await Promise.all(managers)).flat()
    }
    const out = { mode: options.mode, rounds: [], report: null, workspaces: [] }
    let started = false
    let after = null
    try {
      for (let step = 1; step <= 400; step++) {
        const line = await next(after)
        if (line.next === 'done') {
          out.report = line
          break
        }
        if (line.next === 'fault') {
          // S6: the one Lock check. The command stopped where it stood: the Cards it filed and the answers stay.
          if (line.code === ZT_LOCK) {
            log(`Grow: the owner's Lock refused a write at ${line.round}; the stage stops and keeps the Cards already filed.`)
            throw ztLockStop(`${ctx.stage}: ${line.round}`, line)
          }
          throw new Error(`grow stopped at ${line.round || 'next'}: ${line.code}: ${line.cause}`)
        }
        started = true
        log(`Grow: ${line.round}: ${n(line.jobs)} DeepSeek job(s).`)
        const results = await dispatch(line)
        const failed = results.filter((r) => !r || r.status !== 'done').map((r) => r && r.id)
        out.rounds.push({ round: line.round, jobs: n(line.jobs), ok: n(line.jobs) - failed.length, failed })
        after = line.token
      }
      if (!out.report) throw new Error('grow: next did not end within 400 steps')
      const r = out.report
      log(`Grow: ${r.mode === 'hubs' ? 'hubs' : `report ${r.report}: ${n(r.created)} Cards, Topic share ${r.share_before} → ${r.share_after}, ${n(r.faults_left)} Cards with faults left`}${n(r.hubs) ? `; hubs ${n(r.hubs)}, register ${r.register}` : ''}.`)
    } finally {
      if (started) out.workspaces = await dsCleanupWorkspaces([W], { ownWorkspaces: true, label: `${ctx.stage} cleanup`, phase: ctx.phase })
    }
    return out
  },
})
// ==== end module zt-grow-cycle ====
