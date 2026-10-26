// ==== module zt-term-cards v7 ====
// The Term-card branch (S20 #164, S24 #167; v4 #172; v5 #175; v6 #180): Haiku managers and DeepSeek agents only.
// v7 (live run 2026-10-26, staging5): a publication that printed no line is named in the error (`publish_error`).
// Every deterministic step is one `term-steps.ps1` call that a DeepSeek agent runs (a "drive" job, the shared
// zt-workflow-lib helper `ztDrive`), so no Claude agent relays a single command. The flow is linear:
//   start (plan, claim every ready job unit, list the jobs) -> writers (all jobs, at most eight per
//   manager, managers in parallel) -> review (check + review plan) -> reviewers -> fix (one fix per
//   job, in a new conversation) -> fixers -> review again -> reviewers -> finish (complete each job
//   from the readiness result, fail the rest; publish the book, or its ready prefix when a job failed).
// A job that is not ready fails only its own unit; the book unit completes only when every row is
// published (R0343). Every conversation is deleted after its answer is collected; the workspace
// registration is removed at the end (DSMP).
// v6 (#180): a step answers one compact line whose fields are top-level counts and paths (`ztDrive` reads
// only its last line, and only with a boolean top-level `ok`); the module reads counts, never a unit list.
defineModule({
  id: 'zt-term-cards', version: '7', requires: ['@dsmp', 'zt-workflow-lib'],
  summary: 'Term cards: Haiku managers run DeepSeek drive jobs for the deterministic steps and DeepSeek writers, reviewers and fixers for the content; each job completes from the readiness result and the book from its publication report.',
  params: {
    out: { type: 'string', required: true, description: 'the prepared run\'s staging folder' },
    key: { type: 'string', required: true, description: 'the book\'s Citation key' },
    tool: { type: 'string', default: 'zt-digest', description: 'the zt-digest command' },
    perJob: { type: 'number', min: 1, max: 8, description: 'concept terms per writer job (the compiler\'s default is 4)' },
    mathPerJob: { type: 'number', min: 1, max: 8, description: 'math terms per writer job (the compiler\'s default is 1)' },
    script: { type: 'string', default: 'E:\\zet\\scripts\\book-digest\\term-steps.ps1', description: 'the Term step script the drive jobs run' },
  },
  run: async (ctx, p) => {
    // The kernel passes only the params a blueprint gives; the script path has one default.
    const DEFAULT_SCRIPT = 'E:\\zet\\scripts\\book-digest\\term-steps.ps1'
    const OUT = String(p.out).replace(/[\\/]+$/, '')
    const WORK = `${OUT}\\term-cards`
    const DRIVE = `${WORK}\\drive`
    const MODEL = 'deepseek-official/deepseek-flash'
    const phase = ctx.phase
    const tail = (text) => String(text || '').trim().slice(0, 400)
    const report = { status: 'done', book: p.key, work: WORK, jobs: { total: 0, done: 0, failed: 0 }, reviews: 0, fixes: 0, publication: null, deepseek: { deleted: 0, not_deleted: [] } }

    // One manager runs the DeepSeek jobs listed inline, or entries [from, to) of a job file.
    const RESULT = {
      type: 'object',
      properties: {
        ok: { type: 'boolean' },
        jobs: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] }, answer: { type: 'string' }, deleted: { type: 'boolean' } }, required: ['id', 'status'] } },
      },
      required: ['ok', 'jobs'],
    }
    const manager = (label, source) => dsmpSlot(() => ctx.agent(`You are a DeepSeek subagent manager for ZET Term Cards (${label}). You start DeepSeek agents, wait for them, collect their answers and delete their conversations. You never write content and never run commands yourself. Keep your messages short.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_cancel,mcp__deepseek__deepseek_delete".

JOBS: ${source}
Each job has id, files, prompt, skills, model, effort.

1. Start every job in ONE message: mcp__deepseek__deepseek_run with cwd "${WORK}", prompt_files = its "files", prompt = its "prompt", skills = its "skills", model = its "model", reasoning_effort = its "effort", wait_seconds 0.
2. Keep at most 8 jobs running at a time. Until no job is running: call mcp__deepseek__deepseek_wait (wait_seconds ${DSMP.pollSeconds}) on one running job, then mcp__deepseek__deepseek_wait (wait_seconds 0) on the others. A job asking a question: answer "按任务说明自行决定，不需要询问。". A job asking approval for a command or a file under "${OUT}": allow; anything else: reject. A job running longer than ${DSMP.budgetMinutes} minutes: cancel it.
3. A job that failed or gave an empty answer gets ONE more attempt in a new conversation. Never a third.
4. When a job ends, delete its conversation with mcp__deepseek__deepseek_delete. Never archive.
5. Use the session_id that deepseek_run returned, never a word in its place. A step whose job list is empty starts no run: never send a placeholder prompt.
5. You have no terminal tabs, no Monitor loop, no session handoff and no general shell: never start another agent, never hand the work on, never poll with Start-Sleep or any other timer, and wait only with deepseek_wait. Any command you are given to run is run in the foreground with timeout_ms: 600000.
6. Return ok=true and one entry per job: id, session_id, status ("done" or "failed"), answer (its final answer verbatim, at most 2000 characters), deleted.`, { role: 'manager', label: `${ctx.stage}: ${label}`, phase, schema: RESULT }))

    const collect = (run) => {
      const jobs = (run && run.jobs) || []
      for (const j of jobs) { if (j.deleted === true) report.deepseek.deleted++; else if (j.session_id) report.deepseek.not_deleted.push(j.session_id) }
      return jobs
    }
    // A deterministic step: one DeepSeek drive job runs one script call and answers with its last line
    // (the shared zt-workflow-lib helper ztDrive). A step that answers no JSON line stops the branch; a step
    // that answers `ok:false` is the branch's failed reason.
    const drive = async (step) => {
      const command = `pwsh -NoProfile -File "${p.script || DEFAULT_SCRIPT}" -Step ${step} -Out "${OUT}"${p.perJob ? ` -PerJob ${p.perJob}` : ''}${p.mathPerJob ? ` -MathPerJob ${p.mathPerJob}` : ''}`
      const json = await ztDrive(ctx, { label: `${ctx.stage}: drive ${step}`, step, command, out: OUT, cwd: WORK, phase, ledger: 'zt-digest ledger', deepseek: report.deepseek })
      if (json.ok !== true) throw new Error(`term-cards ${step}: ${tail(json.error || 'the step failed')}`)
      return json
    }
    // Content jobs from a job file the script wrote: at most eight per manager, managers in parallel.
    const runFile = async (label, file, count) => {
      const managers = []
      for (let from = 0; from < count; from += DSMP.perManager) {
        const to = Math.min(count, from + DSMP.perManager)
        managers.push(manager(`${label} ${from + 1}-${to}`, `read the JSON array in "${file}" (Read tool) and run its entries ${from} through ${to - 1} (0-based), and only those.`).then(collect))
      }
      return (await Promise.all(managers)).flat()
    }

    try {
      const started = await drive('start')
      report.jobs.total = started.jobs
      report.plan_revision = started.plan_revision
      if (started.jobs > 0) {
        const written = await runFile('write', `${DRIVE}\\jobs.json`, started.jobs)
        report.writer_failures = written.filter((j) => j.status !== 'done').length
        const review = async () => {
          const r = await drive('review')
          report.reviews += r.reviews
          if (r.reviews > 0) await runFile('review', `${DRIVE}\\reviews.json`, r.reviews)
        }
        await review()
        const fix = await drive('fix')
        report.fixes = fix.fixes
        if (fix.fixes > 0) {
          await runFile('fix', `${DRIVE}\\fixes.json`, fix.fixes)
          await review()
        }
      }
      const finished = await drive('finish')
      report.jobs.done = Number(finished.done || 0) || 0
      report.not_ready = Number(finished.failed || 0) || 0
      report.publication = { complete: finished.complete === true, rows: Number(finished.rows || 0), committed: Number(finished.committed || 0), ready_unpublished: Number(finished.ready_unpublished || 0) }
      report.book_state = finished.book
      if (finished.book !== 'done') {
        report.status = 'failed'
        report.error = `the Term-card book is incomplete: ${report.not_ready} job(s) not ready; the ready prefix (${report.publication.committed} row(s)) is published${finished.publish_error ? `; the publication failed: ${finished.publish_error}` : ''}`
      }
    } catch (e) {
      report.status = 'failed'
      report.error = tail(e && e.message)
    } finally {
      try { await dsmpSlot(() => dsCleanupWorkspaces([WORK], { label: `${ctx.stage}: workspace`, phase })) } catch (e) { report.deepseek.not_deleted.push(`workspace: ${tail(e && e.message)}`) }
    }
    log(`Term cards: ${report.jobs.done}/${report.jobs.total} job(s) done, ${report.reviews} review(s), ${report.fixes} fix(es); ${report.status}${report.error ? ` — ${report.error}` : ''}.`)
    return report
  },
})
// ==== end module zt-term-cards ====




