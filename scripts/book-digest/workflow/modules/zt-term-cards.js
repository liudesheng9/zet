// ==== module zt-term-cards v2 ====
// S20 (#164) built the branch; S24 (#167) makes it ledger work. The Term-card compiler's writer,
// reviewer and fix jobs run on the shared DeepSeek Subagent Managed Protocol (DSMP) pool (design
// 12.4), and the branch is the run's own `termcards/tNNN@<plan revision>` job units plus its
// `termcards/book@<plan revision>` unit (design 14.2, R0423/R0424). The compiler (zt-digest
// term-cards) prints the exact JSON jobs and registers the units at `plan`; this module schedules
// them and completes them only from the compiler's own checked artifacts.
//
// The schedule (R0335): ready job units are claimed in waves of at most four. A wave runs its
// writers (retained, never archived), the compiler's check, the independent reviews of the
// mathematical drafts (at most four reviewer conversations alongside the writers), the one permitted
// fix in each writer's own conversation, and the recheck/re-review of a changed draft; each job unit
// then completes from `check/readiness.json`, which binds the job schema, the shared check, the
// required reviews and the fix disposition. The wave's conversations are deleted before the next
// wave is admitted. Four retained writers + four reviewers = the protocol's eight conversations; no
// manager holds eight writers and then looks for a reviewer slot, so the shared pool cannot deadlock.
//
// The book unit completes only from `report.json` when every required row's head and parts are
// committed and read back. A partial prefix, a refused row or a failed writer leaves it incomplete,
// so the Term-link pass stays blocked and the audit/report show the coverage debt (R0428). No unit is
// ever marked done to unblock a later stage.
//
// Every manager call takes one dsmpSlot from the script-wide pool (at most eight managers alive), and
// the runtime policy is the installed protocol's own: 240 minutes per conversation, nonblocking polls
// of all open sessions, at most 90 seconds blocking on one, one provider retry, one semantic fix.
// A conversation is registered, its answer evidence is written into the local staging folder, and its
// deletion is recorded, so the run's own cleanup can verify the provider resources. The workspace
// registration is removed when the branch is done (success or failure); the staging files stay.
defineModule({
  id: 'zt-term-cards', version: '2', requires: ['@dsmp', 'zt-workflow-lib'],
  summary: 'Term cards: claim the run\'s Term-card job units in waves of four, schedule their writers, reviews and fixes on the shared DSMP pool, complete each unit from the compiler\'s readiness result and the book unit from its publication report, and leave a failed or partial book incomplete.',
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
    const MAX_WAVES = 200 // a bounded claim loop: one wave can never be claimed for ever.

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
    const report = { status: 'done', book: p.key, work: WORK, ledger: false, jobs: { total: 0, done: 0, failed: [] }, waves: [], reviews: { planned: 0, approved: 0, refused: 0 }, fixes: { planned: 0, final: 0 }, readiness: { counts: {}, complete: false }, book_unit: null, publication: null, deepseek: notes }
    let error = ''
    let lock = null
    let readiness = { counts: {}, complete: false }
    let bookUnit = null // { id, token, input_hash }

    // One compiler command through the one relay. Its last JSON line is the step's answer.
    const command = async (label, args, always = false) => {
      const got = await ztRelay(ctx, { command: `${exe} term-cards ${args}`, label: `${ctx.stage}: ${label}`, phase, role: p.relay || 'relay', run: `the digest run the user started for "${OUT}"`, note: ' (it writes only the staging folder)', always })
      return got
    }
    const ledger = async (label, args, always = false) => {
      const got = await ztRelay(ctx, { command: `${exe} ledger ${args}`, label: `${ctx.stage}: ${label}`, phase, role: p.relay || 'relay', run: `the digest run the user started for "${OUT}"`, note: ' (it reads or records the run ledger only)', always, lockStops: false })
      return { ok: !!(got.run && got.run.ok), line: got.json, output: (got.run && got.run.output) || '' }
    }
    // Record one unit's content fault: the ledger classifies the code from its cause (S5), and a
    // card_check refusal keeps the archive's own code.
    const fault = async (label, unit, code, cause) => {
      const document = { schema_version: 1, unit: unit.id, input_hash: unit.input_hash, cause: tail(cause) }
      if (code) document.code = code
      const got = await ledger(label, `fail -Out "${OUT}" -Unit "${unit.id}" -Token ${unit.token} -Fault ${q(JSON.stringify(document))}`)
      if (!got.ok) log(`Term cards: the ledger refused the fault of ${unit.id}: ${tail(got.output)}`)
      return got
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
   Register only the session_id deepseek_run returned, never a word in its place: never invent an id, a PENDING placeholder or a job name. A step with no job starts no run: never send a placeholder prompt.
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
      .filter(([value]) => value !== undefined && value !== null).map(([value, flag]) => ` ${flag} ${Number(value)}`).join('')

    // One wave: write, check, review, fix once, recheck and re-review, then (in a prepared run)
    // complete each claimed unit from the readiness result that says every row of that job is ready.
    const runWave = async (index, units, given) => {
      const ids = units ? units.map((unit) => String(unit.inputs.job.id)) : (given || []).map((job) => job.id)
      const row = { wave: index + 1, jobs: ids, reviews: 0, fixes: 0, completed: [] }
      report.waves.push(row)
      try {
        let wave = []
        if (units) {
          const listed = await command('jobs', `-Step jobs -Staging ${q(OUT)} -Tokens ${q(units.map((unit) => `${unit.inputs.job.id}:${unit.token}`).join(','))}`)
          const line = listed.json
          if (!line || line.ok !== true || !Array.isArray(line.jobs)) throw new Error(`term-cards jobs: ${tail((listed.run && listed.run.output) || 'no output')}`)
          wave = line.jobs
        } else {
          const listed = await command('jobs', `-Step jobs -Staging ${q(OUT)} -Jobs ${q(ids.join(','))}`)
          const line = listed.json
          if (!line || line.ok !== true || !Array.isArray(line.jobs)) throw new Error(`term-cards jobs: ${tail((listed.run && listed.run.output) || 'no output')}`)
          wave = line.jobs
        }
        // The writer conversations of this wave are retained (delete_after_collect false). A failed
        // writer's conversation is retained too, so the wave's settle deletes it as well.
        const written = await manager('write', wave)
        const failed = []
        for (const entry of written) {
          if (entry.session_id) sessions[entry.id] = entry.session_id
          if (entry.status === 'done') report.jobs.done++
          else {
            report.jobs.failed.push({ id: entry.id, answer: tail(entry.answer) })
            failed.push(entry.id)
            if (!entry.session_id) notes.not_deleted.push(`${entry.id}: no session id returned`)
          }
        }
        if (failed.length) {
          for (const unit of (units || []).filter((unit) => failed.includes(String(unit.inputs.job.id)))) {
            const answer = ((report.jobs.failed.find((f) => f.id === String(unit.inputs.job.id)) || {}).answer) || ''
            await fault('writer fault', unit, '', `the writer job ${unit.inputs.job.id} failed: ${answer}`)
          }
          throw new Error(`term-cards: writer job(s) failed: ${failed.join(', ')}`)
        }
        bookkeeping((await command('check', `-Step check -Staging ${q(OUT)}`)).json)

        // The independent review of every mathematical draft, in waves of at most four, alongside
        // the four retained writers.
        const reviews = async () => {
          const got = await command('review-plan', `-Step review-plan -Staging ${q(OUT)}`)
          const line = got.json
          if (!line || line.ok !== true) throw new Error(`term-cards review-plan: ${tail((got.run && got.run.output) || 'no output')}`)
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
        const fixLine = fixed.json
        if (!fixLine || fixLine.ok !== true) throw new Error(`term-cards fix-plan: ${tail((fixed.run && fixed.run.output) || 'no output')}`)
        bookkeeping(fixLine)
        report.fixes.final += (fixLine.final || []).length
        const fixes = (fixLine.jobs || []).filter((job) => job.continue)
        report.fixes.planned += fixes.length
        for (const group of chunk(fixes, WAVE)) {
          row.fixes += group.length
          await manager('fix', group)
        }
        if (fixes.length) {
          bookkeeping((await command('check', `-Step check -Staging ${q(OUT)}`)).json)
          await reviews()
        }

        // Complete each claimed unit from the readiness result: a job whose every row is ready is
        // the unit's proof (R0423); a job that is not ready keeps its unit open and is named.
        const notReady = []
        for (const unit of units || []) {
          const done = await ledger('complete', `done -Out "${OUT}" -Unit "${unit.id}" -Token ${unit.token} -Outputs "${WORK}\\check\\readiness.json"`)
          if (done.ok && done.line && done.line.state === 'done') {
            row.completed.push(unit.id)
            continue
          }
          notReady.push(String(unit.inputs.job.id))
          await fault('readiness fault', unit, '', `the job ${unit.inputs.job.id} is not ready: ${tail(done.output)}`)
        }
        if (notReady.length) throw new Error(`term-cards: job(s) not ready after the permitted fix: ${notReady.join(', ')}`)
      } finally {
        // The wave is complete or stopped: delete its retained writers before the next wave is
        // admitted (R0330/R0335). Every staged file stays on disk.
        await settleWave(`wave ${index + 1}`, ids)
      }
    }

    try {
      const planned = await command('plan', `-Step plan -Staging ${q(OUT)}${planOptions}`)
      const planLine = planned.json
      if (!planLine || planLine.ok !== true) throw new Error(`term-cards plan: ${tail((planned.run && planned.run.output) || 'no output')}`)
      report.plan_revision = planLine.revision
      report.plan_hash = planLine.plan_hash
      const units = planLine.units || null
      report.book_unit = units && units.book ? units.book : null
      report.ledger = !!(units && (units.book || (units.jobs && Object.keys(units.jobs).length)))

      if (!report.ledger) {
        // No prepared run ledger (a fixture or a diagnostic branch): the compiler lists its plan's
        // jobs, and the branch reports the branch's own result without a ledger unit.
        const listed = await command('jobs', `-Step jobs -Staging ${q(OUT)}`)
        const jobs = (listed.json && listed.json.jobs) || []
        report.jobs.total = jobs.length
        if (!jobs.length) {
          report.readiness = readiness
          report.status = 'failed'
          report.error = 'the plan has no writer jobs (no usable Term rows)'
          report.staging = WORK
          return report
        }
        for (let index = 0; index < chunk(jobs, WAVE).length && !error && !lock; index++) {
          const wave = chunk(jobs, WAVE)[index]
          try {
            await runWave(index, null, wave)
          } catch (e) {
            error = error || tail(e && e.message) || 'term-cards: the wave failed'
          }
        }
      } else {
        report.jobs.total = Object.keys(units.jobs || {}).length
        let claimedWaves = 0
        while (!error && !lock && claimedWaves < MAX_WAVES && !bookUnit) {
          const claimed = await ledger('claim', `next -Out "${OUT}" -Layer termcards -Take ${WAVE} -Owner "term cards wave ${claimedWaves + 1}" -Lease 3600`)
          if (!claimed.ok || !claimed.line) { error = `the Term-card claim returned no answer: ${tail(claimed.output)}`; break }
          const held = (claimed.line.units || []).filter((unit) => unit && typeof unit.id === 'string')
          // The book unit sorts before the job units (logical key `book`): when every job is done, the
          // claim offers it alone, and the branch goes straight to its publication under this claim.
          const book = held.find((unit) => unit.id.startsWith('termcards/book@'))
          if (book) {
            bookUnit = book
            report.book_unit = book.id
          }
          const ready = held.filter((unit) => unit.id.startsWith('termcards/') && !unit.id.startsWith('termcards/book@') && unit.inputs && unit.inputs.job && unit.inputs.job.id)
          if (!ready.length) break
          claimedWaves++
          await runWave(claimedWaves - 1, ready)
        }
        if (claimedWaves >= MAX_WAVES) error = error || `the Term-card branch claimed ${MAX_WAVES} waves without settling`
        if (!error && !lock && !bookUnit) {
          const claimed = await ledger('claim', `next -Out "${OUT}" -Layer termcards -Take 1 -Owner "term cards book" -Wait 60 -Lease 3600`)
          const unit = ((claimed.line && claimed.line.units) || []).find((u) => u && typeof u.id === 'string' && u.id.startsWith('termcards/book@'))
          if (!unit) {
            report.readiness = readiness
            report.status = 'failed'
            report.error = 'the Term-card book unit is not ready: a writer job or the Term table it waits on is not done'
            report.staging = WORK
            return report
          }
          bookUnit = unit
          report.book_unit = unit.id
        }
      }
      if (!error && !lock) {
        const applied = await command('apply', `-Step apply -Staging ${q(OUT)}${bookUnit ? ` -Token ${bookUnit.token}` : ''} -Apply`)
        const line = applied.json
        const ok = !!(applied.run && applied.run.ok) && !!line && line.ok === true && line.complete === true
        report.publication = line ? { complete: line.complete === true, rows: line.rows, committed: line.committed, ready_unpublished: line.ready_unpublished, refused: line.refused, pending: line.pending, conflicted: line.conflicted, staged: line.staged, stop: line.stop } : null
        if (!ok) {
          const stop = (line && (line.code || (line.stop && line.stop.code))) || 'incomplete'
          if (bookUnit) await fault('publication fault', bookUnit, '', `the Term-card publication is not complete (${stop}): ${tail((line && line.error) || (applied.run && applied.run.output) || 'no output')}`)
          report.status = 'failed'
          report.error = `the Term-card publication is not complete (${stop}); the book unit stays incomplete`
        } else if (bookUnit) {
          const completed = await ledger('complete', `done -Out "${OUT}" -Unit "${bookUnit.id}" -Token ${bookUnit.token} -Outputs "${WORK}\\report.json"`)
          if (!completed.ok || !completed.line || completed.line.state !== 'done') {
            await fault('book fault', bookUnit, '', `the Term-card book completion was refused: ${tail(completed.output)}`)
            report.status = 'failed'
            report.error = `the Term-card book unit could not complete: ${tail(completed.output)}`
          } else {
            report.completion = completed.line
          }
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
