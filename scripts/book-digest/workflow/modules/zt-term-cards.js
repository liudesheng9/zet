// ==== module zt-term-cards v1 ====
// S20 (#164): run the Term-card compiler's writer, reviewer and fix jobs on the shared DeepSeek
// Subagent Managed Protocol (DSMP) pool (design 12.4; S20 / #164). The compiler (zt-digest
// term-cards, S18/S19) prints the exact JSON jobs; this module only
// schedules them, so no agent decides what a term is or writes Card prose.
//
// The schedule (R0335): the writer jobs are processed in waves of at most four. A wave keeps its four
// writer conversations (retained, never archived), runs the compiler's check, obtains the independent
// reviews of the mathematical drafts (at most four reviewer conversations alongside the writers),
// sends the one permitted fix back to each writer's own conversation through deepseek_prompt, checks
// and re-reviews any changed draft, and deletes the wave's conversations before the next wave starts.
// Four retained writers + four reviewers = the protocol's eight conversations; no manager can hold
// eight writers and then look for a reviewer slot, so the shared 64-slot pool cannot deadlock.
//
// Every manager call takes one dsmpSlot from the script-wide pool (at most eight managers alive), and
// the runtime policy is the installed protocol's own: 240 minutes per conversation, nonblocking polls
// of all open sessions, at most 90 seconds blocking on one, one provider retry, one semantic fix.
// A conversation is registered, its answer evidence is written into the local staging folder, and its
// deletion is recorded, so the run's own cleanup can verify the provider resources. The workspace
// registration is removed when the branch is done (success or failure); the staging files stay.
//
// The stage graph, the ledger units and the full-mode dependencies belong to S24 (#167): this module
// returns the branch's report and writes no ledger unit of its own.
defineModule({
  id: 'zt-term-cards', version: '1', requires: ['@dsmp', 'zt-workflow-lib'],
  summary: 'Term cards: schedule the compiler\'s writer jobs in waves of four on the shared DSMP pool, review the mathematical drafts, send the one fix back to each writer\'s own conversation, delete every conversation and remove the workspace registration while keeping the staging files.',
  params: {
    out: { type: 'string', required: true, description: 'the prepared run\'s staging folder (the compiler\'s -Staging)' },
    key: { type: 'string', required: true, description: 'the book\'s Citation key' },
    tool: { type: 'string', default: 'zt-digest', description: 'the zt-digest command, or the path of a zt-digest binary' },
    perJob: { type: 'number', min: 1, max: 8, description: 'concept terms per writer job (the compiler\'s default is 4)' },
    mathPerJob: { type: 'number', min: 1, max: 8, description: 'math terms per writer job (the compiler\'s default is 1)' },
    maxStepsPerCard: { type: 'number', min: 1, max: 64, description: 'steps per rendered continuation part (the compiler\'s default is 12)' },
    maxProse: { type: 'number', min: 1, max: 20000, description: 'prose budget of a Card (the compiler\'s default is 1600)' },
    relay: { type: 'string', default: 'relay', role: true, description: 'the declared role that relays the compiler commands (the zt-relay agent type)' },
  },
  run: async (ctx, p) => {
    // The four constants this module reconciles against the protocol it was built with (R0332/R0334).
    const WANTED = { perManager: 8, maxManagers: 8, pollSeconds: 90, budgetMinutes: 240 }
    const WAVE = 4 // R0335: at most four live writers, and at most four reviewers alongside them.

    const q = (v) => `'${String(v).replace(/'/g, "''")}'`
    const OUT = String(p.out).replace(/[\\/]+$/, '')
    const WORK = `${OUT}\\term-cards`
    const ANSWERS = `${WORK}\\answers`
    const exe = p.tool && p.tool !== 'zt-digest' ? `& ${q(p.tool)}` : 'zt-digest'
    const phase = ctx.phase
    const chunk = (list, size) => { const out = []; for (let i = 0; i < list.length; i += size) out.push(list.slice(i, i + size)); return out }
    // The review batches of one wave (R0335): at most four reviewer conversations, and one grouped
    // writer job's promoted terms are reviewed one after another, never side by side.
    const reviewBatches = (jobs) => {
      const batches = []
      let left = jobs.slice()
      while (left.length) {
        const batch = []
        const seen = new Set()
        const rest = []
        for (const job of left) {
          const writer = String(job.id).replace(/-r\d+-review$/, '')
          if (batch.length < WAVE && !seen.has(writer)) { seen.add(writer); batch.push(job) } else rest.push(job)
        }
        batches.push(batch)
        left = rest
      }
      return batches
    }
    const tail = (text) => String(text || '').trim().slice(0, 400)

    // R0332/AC08: the runtime policy is the installed protocol's own. A changed protocol is reported
    // explicitly, never silently stretched by this workflow.
    const changed = Object.entries(WANTED).filter(([k, v]) => DSMP[k] !== v)
    if (changed.length) {
      return { status: 'failed', error: `the managed protocol changed (${changed.map(([k, v]) => `${k} ${DSMP[k]} ≠ ${v}`).join(', ')}); reconcile the term-card waves before running`, book: p.key, work: WORK }
    }

    const notes = { registered: 0, evidence: 0, deleted: 0, not_deleted: [], workspaces: [] }
    const sessions = {} // a retained writer's job id -> its conversation id
    const report = { status: 'done', book: p.key, work: WORK, jobs: { total: 0, done: 0, failed: [] }, waves: [], reviews: { planned: 0, approved: 0, refused: 0 }, fixes: { planned: 0, final: 0 }, readiness: { counts: {}, complete: false }, deepseek: notes }
    let error = ''
    let lock = null
    let readiness = { counts: {}, complete: false }

    // One compiler command through the one relay. Its last JSON line is the step's answer.
    const command = async (label, args) => {
      const got = await ztRelay(ctx, { command: `${exe} term-cards ${args}`, label: `${ctx.stage}: ${label}`, phase, role: p.relay || 'relay', run: `the digest run the user started for "${OUT}"`, note: ' (it writes only the staging folder)' })
      const line = got.json
      if (!line || line.ok !== true) throw new Error(`term-cards ${label}: ${tail((got.run && got.run.output) || 'no output')}`)
      return line
    }
    const bookkeeping = (line) => { if (line && line.counts) readiness = { counts: line.counts, complete: line.complete === true } }
    // The final review state of each term, keyed by job and row: a later review-plan repeats the
    // terms already approved, so only a changed state is counted (R0322: one verdict per binding).
    const reviewStates = new Map()
    const countReviews = (line) => {
      for (const notice of (line && line.reviews) || []) {
        const key = `${notice.job}#${notice.row}`
        const before = reviewStates.get(key)
        if (before === notice.status) continue
        if (before && report.reviews[before] !== undefined) report.reviews[before]--
        if (report.reviews[notice.status] !== undefined) report.reviews[notice.status]++
        reviewStates.set(key, notice.status)
      }
    }

    // The record commands each manager runs itself, at the moment the fact exists (R0322-R0325).
    const register = `${exe} ledger resource -Out ${q(OUT)} -Unit run -RegisterId <session_id> -Workspace ${q(WORK)}`
    const evidence = (file) => `${exe} ledger resource -Out ${q(OUT)} -Unit run -EvidenceFor <session_id> -Evidence ${q(file)}`
    const settle = `${exe} ledger resource -Out ${q(OUT)} -Unit run -Settle <session_id> -State finished -Deleted confirmed`

    // The manager prompt: the DSMP v1.1 loop (one retry, 240 min, 90 s polls) with the one extension
    // the Term-card chain needs: a writer conversation survives until its wave's fix/review
    // opportunity is complete (R0330).
    const managerPrompt = (kind, jobs) => {
      const table = Object.entries(sessions).map(([job, session]) => `${job} ${session}`).join('\n')
      return `You are a DeepSeek subagent manager for ZET Term Cards (${kind}). You start DeepSeek agents, collect their answers, and delete the conversations the jobs tell you to delete. You do not write Card prose or JSON yourself: the DeepSeek content jobs do. Keep your messages short.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_prompt,mcp__deepseek__deepseek_delete".

JOBS (${jobs.length}):
${JSON.stringify(jobs, null, 1)}

1. Start, in ONE message. For a job without "continue": mcp__deepseek__deepseek_run with cwd "${WORK}", prompt_files = its "files", prompt = its "prompt", skills = its "skills", model = its "model", reasoning_effort = its "effort", wait_seconds 0. For a job with "continue", take that job id's session_id from this table (job id, then session_id):
${table || '(empty)'}
   With a session: mcp__deepseek__deepseek_prompt with that session_id, prompt_files = its "files", prompt = its "prompt", skills = its "skills", reasoning_effort = its "effort", wait_seconds 0. Without a session: mcp__deepseek__deepseek_run with prompt_files = its "fallback", and the same other fields.
   The moment a deepseek_run returns a session_id, record it: ${register}
2. Collect loop. A session is OPEN until you have collected it in step 4. Repeat until no session is open:
   a. Poll, in ONE message: call mcp__deepseek__deepseek_wait with wait_seconds 0 for EVERY open session.
   b. Act on every result before waiting again:
      - finished, failed or cancelled: collect it now (steps 3, 4 and 5).
      - needs_response with an approval: allow only a read of a file under "${WORK}", and writing the one file the job's own prompt names ("${WORK}\\out" or "${WORK}\\review\\answers"). Reject everything else: shell commands, every zt command, deletes, network, installs, and files outside "${WORK}". Answer a question with custom text "按任务说明自行决定，不需要询问。".
      - running longer than ${DSMP.budgetMinutes} minutes: mcp__deepseek__deepseek_cancel it. It is collected as a timeout and gets no retry.
   c. Block: if any session is still running, call mcp__deepseek__deepseek_wait with wait_seconds ${DSMP.pollSeconds} on exactly ONE running session (the one started first). Never use a larger wait and never block on two sessions.
3. Check: a job succeeds when its status is finished and its answer is not empty. A failed job gets ONE more attempt, in a new conversation: deepseek_run with prompt_files = its "fallback" (or its "files" when it has none) and the same other fields. Never start a third attempt. A timeout gets no retry.
4. Collect: copy each job's final answer verbatim, at most 4000 characters, into "${ANSWERS}\\<session_id>.txt" (the line "no answer: <status>" when it gave none), then record it: ${evidence(`${ANSWERS}\\<session_id>.txt`)}
5. Delete: when the job's "delete_after_collect" is true, call mcp__deepseek__deepseek_delete with its session_id and record it: ${settle}
   Never call deepseek_archive and never call deepseek_remove_workspace. A job whose delete_after_collect is false stays open: its wave deletes it after the fix/review opportunity.
   A record command that fails does not stop the work: return registered/evidence/deleted false for that job and go on.
6. Return ok=true and one entry per job: id, session_id, status ("done" or "failed"), answer, registered, evidence, deleted. Copy answers verbatim; never summarize.`
    }

    const RESULT = {
      type: 'object',
      properties: {
        ok: { type: 'boolean' }, error: { type: 'string' }, deleted: { type: 'number' },
        not_deleted: { type: 'array', items: { type: 'string' } },
        jobs: { type: 'array', items: { type: 'object', properties: {
          id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] },
          answer: { type: 'string' }, registered: { type: 'boolean' }, evidence: { type: 'boolean' }, deleted: { type: 'boolean' },
        }, required: ['id', 'status'] } },
      },
      required: ['ok', 'jobs'],
    }
    // One manager call takes one slot of the script-wide pool: the term branch and the digest branch
    // share the same eight managers (R0327/R0328).
    const manager = async (kind, jobs) => {
      if (!jobs.length) return []
      const run = await dsmpSlot(() => ctx.agent(managerPrompt(kind, jobs), { role: 'manager', label: `${ctx.stage}: ${kind}`, phase, schema: RESULT }))
      if (!run || !Array.isArray(run.jobs)) throw new Error(`term-cards: the ${kind} manager returned no report`)
      for (const entry of run.jobs) {
        if (entry.registered === true) notes.registered++
        if (entry.evidence === true) notes.evidence++
        if (entry.deleted === true) notes.deleted++
      }
      if ((run.not_deleted || []).length) notes.not_deleted.push(...run.not_deleted)
      // A job the manager did not answer is a failed job, never a silent success.
      return jobs.map((job) => (run.jobs || []).find((entry) => entry.id === job.id) || { id: job.id, status: 'failed', answer: 'the manager returned no entry' })
    }

    // The settle of one wave: delete the writer conversations that the protocol is not allowed to
    // keep, after the review/fix opportunity, and record every outcome (R0330).
    const SETTLE = {
      type: 'object',
      properties: {
        ok: { type: 'boolean' }, error: { type: 'string' }, deleted: { type: 'number' },
        not_deleted: { type: 'array', items: { type: 'string' } },
        sessions: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, session_id: { type: 'string' }, deleted: { type: 'boolean' } }, required: ['id', 'session_id', 'deleted'] } },
      },
      required: ['ok', 'sessions'],
    }
    const settlePrompt = (list) => `You are a DeepSeek subagent manager for ZET Term Cards (settle a wave). Delete the conversations of one finished wave and record every deletion. Never archive a conversation and never remove a workspace.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_delete".

SESSIONS (${list.length}):
${JSON.stringify(list.map(([id, session_id]) => ({ id, session_id })))}

For each session in order: call mcp__deepseek__deepseek_delete with its session_id. If a delete is refused, do not retry it and record the reason. Then record the outcome once for that session: ${settle} (or ${exe} ledger resource -Out ${q(OUT)} -Unit run -Settle <session_id> -State finished -Deleted failed -Message '<reason>').
Return ok=true, deleted (how many), not_deleted ("session_id: reason" for each), and one entry per session: id, session_id, deleted.`
    const settleWave = async (label, ids) => {
      const list = ids.filter((id) => sessions[id]).map((id) => [id, sessions[id]])
      if (!list.length) return
      let run = null
      try {
        run = await dsmpSlot(() => ctx.agent(settlePrompt(list), { role: 'manager', label: `${ctx.stage}: ${label}`, phase, schema: SETTLE }))
      } catch (e) {
        // The run-level cleanup still knows the registered ids; a settle that throws is reported.
        notes.not_deleted.push(`${label}: ${tail(e && e.message)}`)
        return
      }
      if (!run || !Array.isArray(run.sessions)) { notes.not_deleted.push(`${label}: the settle manager returned no report`); return }
      notes.deleted += Number(run.deleted) || 0
      if (Array.isArray(run.not_deleted)) notes.not_deleted.push(...run.not_deleted)
      // A conversation the settle manager did not delete stays registered: the run cleanup retries it.
      for (const [id, session] of list) {
        const entry = run.sessions.find((s) => s.session_id === session)
        if (!entry || entry.deleted !== true) notes.not_deleted.push(`${session}: not deleted`)
        delete sessions[id]
      }
    }

    const planOptions = [[p.perJob, '-PerJob'], [p.mathPerJob, '-MathPerJob'], [p.maxStepsPerCard, '-MaxStepsPerCard'], [p.maxProse, '-MaxProse']]
      .filter(([value]) => value !== undefined).map(([value, flag]) => ` ${flag} ${Number(value)}`).join('')

    try {
      const planned = await command('plan', `-Step plan -Staging ${q(OUT)}${planOptions}`)
      report.plan_revision = planned.revision
      report.plan_hash = planned.plan_hash
      const listed = await command('jobs', `-Step jobs -Staging ${q(OUT)}`)
      const jobs = Array.isArray(listed.jobs) ? listed.jobs : []
      report.jobs.total = jobs.length
      report.plan_revision = listed.plan_revision || report.plan_revision
      report.plan_hash = listed.plan_hash || report.plan_hash
      if (!jobs.length) {
        // A plan with no writer job is no Term branch: never report a successful zero-term book
        // (design 14.2). The stage graph of S24 (#167) decides skipped versus failed for it.
        report.readiness = readiness
        report.status = 'failed'
        report.error = 'the plan has no writer jobs (no usable Term rows)'
        report.staging = WORK
        return report
      }
      const waves = chunk(jobs, WAVE)
      for (let index = 0; index < waves.length && !error && !lock; index++) {
        const wave = waves[index]
        const ids = wave.map((job) => job.id)
        const row = { wave: index + 1, jobs: ids, reviews: 0, fixes: 0 }
        report.waves.push(row)
        log(`Term cards: wave ${index + 1}/${waves.length}: ${ids.length} writer job(s) ${ids.join(', ')}.`)
        try {
          // The writer conversations of this wave are retained (delete_after_collect false). A failed
          // writer's conversation is retained too, so the wave's settle deletes it as well.
          const written = await manager('write', wave)
          for (const entry of written) {
            if (entry.session_id) sessions[entry.id] = entry.session_id
            if (entry.status === 'done') report.jobs.done++
            else {
              report.jobs.failed.push({ id: entry.id, answer: tail(entry.answer) })
              if (!entry.session_id) notes.not_deleted.push(`${entry.id}: no session id returned`)
            }
          }
          if (report.jobs.failed.length) throw new Error(`term-cards: writer job(s) failed: ${report.jobs.failed.map((f) => f.id).join(', ')}`)
          bookkeeping(await command('check', `-Step check -Staging ${q(OUT)}`))

          // The independent review of every mathematical draft, in waves of at most four, alongside
          // the four retained writers.
          const reviews = async () => {
            const line = await command('review-plan', `-Step review-plan -Staging ${q(OUT)}`)
            bookkeeping(line)
            countReviews(line)
            for (const group of reviewBatches((line.jobs || []).filter((job) => job.delete_after_collect === true))) {
              row.reviews += group.length
              report.reviews.planned += group.length
              await manager('review', group)
            }
          }
          await reviews()

          // One finite fix per writer job, sent back to that writer's own conversation; a changed
          // draft is checked and re-reviewed.
          const fixed = await command('fix-plan', `-Step fix-plan -Staging ${q(OUT)}`)
          bookkeeping(fixed)
          report.fixes.final += (fixed.final || []).length
          const fixes = (fixed.jobs || []).filter((job) => job.continue)
          report.fixes.planned += fixes.length
          for (const group of chunk(fixes, WAVE)) {
            row.fixes += group.length
            await manager('fix', group)
          }
          if (fixes.length) {
            bookkeeping(await command('check', `-Step check -Staging ${q(OUT)}`))
            await reviews()
          }
        } catch (e) {
          if (e && e.lock_refusal) lock = e
          error = error || tail(e && e.message) || 'term-cards: the wave failed'
        } finally {
          // The wave is complete or stopped: delete its retained writers before the next wave is
          // admitted (R0330/R0335). Every staged file stays on disk.
          await settleWave(`wave ${index + 1}`, ids)
        }
      }
    } catch (e) {
      if (e && e.lock_refusal) lock = e
      error = error || tail(e && e.message) || 'term-cards: the branch failed'
    } finally {
      // R0331: remove the workspace registration once the branch's work and continuations are done;
      // removing the registration never deletes the staging folder. The local files stay for audit.
      if (notes.registered) {
        try {
          // The workspace removal is one more manager of the shared pool, never a second path.
          const folders = await dsmpSlot(() => dsCleanupWorkspaces([WORK], { label: `${ctx.stage}: workspace`, phase }))
          notes.workspaces.push(...(folders || []))
        } catch (e) {
          notes.not_deleted.push(`workspace: ${tail(e && e.message)}`)
        }
      }
    }

    report.readiness = readiness
    if (lock) { report.status = 'failed'; report.error = `card_locked: ${tail(lock.message)}`; report.staging = WORK }
    else if (error) { report.status = 'failed'; report.error = error; report.staging = WORK }
    else if (report.jobs.failed.length) { report.status = 'failed'; report.error = `writer job(s) failed: ${report.jobs.failed.map((f) => f.id).join(', ')}`; report.staging = WORK }
    if (notes.not_deleted.length) log(`Term cards: ${notes.not_deleted.length} conversation(s) not deleted by the branch; the run cleanup still holds their ids.`)
    log(`Term cards: ${report.jobs.done}/${report.jobs.total} writer job(s), ${report.reviews.planned} review(s), ${report.fixes.planned} fix prompt(s), ${notes.deleted} conversation(s) deleted, ${notes.workspaces.length} workspace(s) removed.`)
    if (lock) throw lock
    return report
  },
})
// ==== end module zt-term-cards ====
