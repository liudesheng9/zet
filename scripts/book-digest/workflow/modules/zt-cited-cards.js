// ==== module zt-cited-cards v2 ====
// The Cited literature branch (#221, docs/cited-literature-cards-proposal.md): Haiku managers and DeepSeek agents
// only, the shape of zt-term-cards v8 without a review round. Every deterministic step is one `cited-steps.ps1`
// call that a DeepSeek agent runs (a "drive" job, the shared zt-workflow-lib helper `ztDrive`), so no Claude agent
// relays a single command. Two stages use it:
//   list   (stage `cited`): drive list (parse the whole source's reference list, index the citing lines) ->
//          the normalize jobs for the entries the parser could not split -> drive list-finish (the Cited list Card
//          under the claimed cited/book@1, completed from the apply report)
//   cards  (stage `cited-cards`): drive start (plan, claim every ready job unit, list the jobs) -> writers (each
//          reviews how its works are used in the WHOLE source and self-checks; at most eight per manager,
//          managers in parallel) -> drive finish (complete each job from the readiness result, fail the rest;
//          publish the ready prefix in row order; the book unit completes only when every row is published)
// Every conversation is deleted after its answer is collected; the workspace registration is removed at the end.
// v2 (pilot 2026-10-31): `source` names the whole bilingual source for a backfill, whose adopted staging names the
// Chinese-only book.zh.md; the list step passes it as -Source and the plan freezes it for the Cards.
defineModule({
  id: 'zt-cited-cards', version: '2', requires: ['@dsmp', 'zt-workflow-lib'],
  summary: 'Cited list and Cited Cards: Haiku managers run DeepSeek drive jobs for the deterministic steps and DeepSeek writers for the content; each unit completes only from the compiler\'s own artifacts.',
  params: {
    out: { type: 'string', required: true, description: 'the prepared run\'s staging folder' },
    key: { type: 'string', required: true, description: 'the book\'s Citation key' },
    part: { type: 'string', required: true, description: '`list` (the Cited list) or `cards` (the Cited Cards)' },
    perJob: { type: 'number', min: 1, max: 8, description: 'works per writer job (the compiler\'s default is 4)' },
    heading: { type: 'string', description: 'the reference-list heading when it is not References, Bibliography or 参考文献' },
    source: { type: 'string', description: 'the whole bilingual source when the staging manifest names another file (an adopted backfill)' },
    script: { type: 'string', default: 'E:\\zet\\scripts\\book-digest\\cited-steps.ps1', description: 'the Cited step script the drive jobs run' },
  },
  run: async (ctx, p) => {
    // The kernel passes only the params a blueprint gives; the script path has one default.
    const DEFAULT_SCRIPT = 'E:\\zet\\scripts\\book-digest\\cited-steps.ps1'
    const OUT = String(p.out).replace(/[\\/]+$/, '')
    const WORK = p.part === 'list' ? `${OUT}\\cited` : `${OUT}\\cited-cards`
    const phase = ctx.phase
    const tail = (text) => String(text || '').trim().slice(0, 400)
    const report = { status: 'done', book: p.key, part: p.part, jobs: { total: 0, done: 0, failed: 0 }, deepseek: { deleted: 0, not_deleted: [] } }

    // One manager runs the DeepSeek jobs listed in entries [from, to) of a job file.
    const RESULT = {
      type: 'object',
      properties: {
        ok: { type: 'boolean' },
        jobs: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] }, answer: { type: 'string' }, deleted: { type: 'boolean' } }, required: ['id', 'status'] } },
      },
      required: ['ok', 'jobs'],
    }
    const manager = (label, source) => dsmpSlot(() => ctx.agent(`You are a DeepSeek subagent manager for ZET Cited Cards (${label}). You start DeepSeek agents, wait for them, collect their answers and delete their conversations. You never write content and never run commands yourself. Keep your messages short.

First load the tools: ToolSearch with query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_cancel,mcp__deepseek__deepseek_delete".

JOBS: ${source}
Each job has id, files, prompt, skills, model, effort.

1. Start every job in ONE message: mcp__deepseek__deepseek_run with cwd "${WORK}", prompt_files = its "files", prompt = its "prompt", skills = its "skills", model = its "model", reasoning_effort = its "effort", wait_seconds 0.
2. Keep at most 8 jobs running at a time. Until no job is running: call mcp__deepseek__deepseek_wait (wait_seconds ${DSMP.pollSeconds}) on one running job, then mcp__deepseek__deepseek_wait (wait_seconds 0) on the others. A job asking a question: answer "按任务说明自行决定，不需要询问。". A job asking approval for a command, or for reading or writing a file under "${OUT}" or the book's source file its job file names: allow; anything else: reject. A job running longer than ${DSMP.budgetMinutes} minutes: cancel it.
3. A job that failed or gave an empty answer gets ONE more attempt in a new conversation. Never a third.
4. When a job ends, delete its conversation with mcp__deepseek__deepseek_delete. Never archive.
5. Use the session_id that deepseek_run returned, never a word in its place. A step whose job list is empty starts no run: never send a placeholder prompt.
6. You have no terminal tabs, no Monitor loop, no session handoff and no general shell: never start another agent, never hand the work on, never poll with Start-Sleep or any other timer, and wait only with deepseek_wait. Any command you are given to run is run in the foreground with timeout_ms: 600000.
7. Return ok=true and one entry per job: id, session_id, status ("done" or "failed"), answer (its final answer verbatim, at most 2000 characters), deleted.`, { role: 'manager', label: `${ctx.stage}: ${label}`, phase, schema: RESULT }))

    const collect = (run) => {
      const jobs = (run && run.jobs) || []
      for (const j of jobs) { if (j.deleted === true) report.deepseek.deleted++; else if (j.session_id) report.deepseek.not_deleted.push(j.session_id) }
      return jobs
    }
    // A deterministic step: one DeepSeek drive job runs one script call and answers with its last line (the shared
    // zt-workflow-lib helper ztDrive). A step that answers no JSON line or `ok:false` stops the stage with its reason.
    const drive = async (step) => {
      const extra = `${p.perJob ? ` -PerJob ${p.perJob}` : ''}${p.heading ? ` -Heading "${p.heading}"` : ''}${p.source && step === 'list' ? ` -Source "${p.source}"` : ''}`
      const command = `pwsh -NoProfile -File "${p.script || DEFAULT_SCRIPT}" -Step ${step} -Out "${OUT}"${extra}`
      const json = await ztDrive(ctx, { label: `${ctx.stage}: drive ${step}`, step, command, out: OUT, cwd: WORK, phase, ledger: 'zt-digest ledger', deepseek: report.deepseek })
      if (json.ok !== true) throw new Error(`cited ${step}: ${tail(json.error || 'the step failed')}`)
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
      if (p.part === 'list') {
        const listed = await drive('list')
        report.rows = Number(listed.rows || 0)
        report.unparsed = Number(listed.unparsed || 0)
        if (listed.jobs > 0) {
          report.jobs.total = listed.jobs
          const ran = await runFile('normalize', listed.file, listed.jobs)
          report.jobs.failed = ran.filter((j) => j.status !== 'done').length
          report.jobs.done = ran.length - report.jobs.failed
        }
        if (report.rows === 0) {
          report.status = 'skipped'
          report.reason = 'the source has no reference list'
        } else {
          const finished = await drive('list-finish')
          report.list = finished.card || null
          report.refused = Number(finished.refused || 0)
          report.unit = finished.unit || null
        }
      } else {
        const started = await drive('start')
        report.jobs.total = started.jobs
        report.plan_revision = started.plan_revision
        if (started.jobs > 0) {
          const written = await runFile('write', started.file, started.jobs)
          report.writer_failures = written.filter((j) => j.status !== 'done').length
        }
        const finished = await drive('finish')
        report.jobs.done = Number(finished.done || 0) || 0
        report.not_ready = Number(finished.not_ready || 0)
        report.publication = { complete: finished.complete === true, rows: Number(finished.rows || 0), committed: Number(finished.committed || 0) }
        report.book_state = finished.book
        if (finished.book !== 'done') {
          report.status = 'failed'
          report.error = `the Cited Card book is incomplete: ${report.not_ready} row(s) not ready; ${report.publication.committed} of ${report.publication.rows} row(s) are published${finished.publish_error ? `; the publication failed: ${finished.publish_error}` : ''}`
        }
      }
    } catch (e) {
      report.status = 'failed'
      report.error = tail(e && e.message)
    } finally {
      try { await dsmpSlot(() => dsCleanupWorkspaces([WORK], { label: `${ctx.stage}: workspace`, phase })) } catch (e) { report.deepseek.not_deleted.push(`workspace: ${tail(e && e.message)}`) }
    }
    log(`Cited ${p.part}: ${report.status}${p.part === 'list' ? `, ${report.rows || 0} row(s)` : `, ${report.jobs.done}/${report.jobs.total} job(s)`}${report.error ? ` — ${report.error}` : ''}.`)
    return report
  },
})
// ==== end module zt-cited-cards ====
