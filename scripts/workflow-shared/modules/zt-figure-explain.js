// ==== module zt-figure-explain v1 ====
// docs/figure-explanation-design.md. A DeepSeek agent looks at each figure and says what it shows; the explanation is
// saved beside the image in the Scholar Archive. The pass belongs to the image, not to a digest: no ledger, no Card, no
// staging. One start, one queue of Haiku managers, one finish; the start lists only the images with no explanation for
// their current bytes, so a rerun redoes nothing, and one more round takes what stayed open. The digest runs it before
// Prepare, so book.zh.md is written once with the explanations in it; the standalone workflow zt-figure-explain runs
// it over many documents (the backfill).
defineModule({
  id: 'zt-figure-explain', version: '1', requires: ['zt-workflow-lib'],
  summary: 'ZT figures: DeepSeek looks at each image and writes its explanation, saved beside the image in the Scholar Archive.',
  params: {
    fileIds: { type: 'any', description: 'the Markdown file ids of the documents (the standalone workflow)' },
    args: { type: 'any', description: 'a digest\'s args ({{args}}): its book, fileId or source names the one document' },
    root: { type: 'any', description: 'the existing folder the pass works under (the standalone workflow); a digest uses the parent of args.out' },
  },
  run: async (ctx, p) => {
    const A = p.args || {}
    const OUT = String(A.out || '').replace(/[\\/]+$/, '')
    const ROOT = String(p.root || OUT.replace(/[\\/][^\\/]*$/, '')).replace(/[\\/]+$/, '')
    const IDS = Array.isArray(p.fileIds) ? p.fileIds.join(',') : ''
    const TAG = IDS ? IDS.split(',').slice(0, 3).join('-') + (IDS.split(',').length > 3 ? `-and-${IDS.split(',').length - 3}` : '') : String(A.citationKey || 'book')
    const WORK = `${ROOT}\\_figure-explain\\${TAG}`
    const quote = (text) => `'${String(text).replace(/'/g, "''")}'`
    const named = IDS ? `-FileIds ${IDS}`
      : A.fileId ? `-FileId ${A.fileId}`
        : A.book ? `-Book ${quote(A.book)}${A.citationKey ? ` -CitationKey ${quote(A.citationKey)}` : ''}`
          : `-Source ${quote(A.source || '')}`
    const deepseek = {}
    const step = (label, command) => ztDrive(ctx, { label: `figure explanations: ${label}`, command, out: ROOT, cwd: ROOT, phase: ctx.phase, record: false, deepseek })
    const SCHEMA = {
      type: 'object',
      properties: { done: { type: 'number' }, failed: { type: 'array', items: { type: 'string' } }, not_deleted: { type: 'array', items: { type: 'string' } } },
      required: ['done', 'failed', 'not_deleted'],
    }
    // The job list is a file: a manager reads it and hands each job's images to deepseek_run.
    const manager = (label, file) => `You are a DeepSeek subagent manager for ZET figure readers (${label}). You start DeepSeek agents, wait for them and delete their conversations. You never write content and you never run a content command yourself. Keep your messages short.
Load the tools with one ToolSearch call: query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_cancel,mcp__deepseek__deepseek_delete".
JOBS: read the JSON array in "${file}" (Read tool) and run EVERY entry in it. Each entry has id, files, images, prompt and output.
1. Start every job in one message: mcp__deepseek__deepseek_run with cwd "${WORK}", prompt_files = the entry's files, images = the entry's images (copy the list exactly: the agent must see these images), prompt = the entry's prompt, skills [], model "${ZT_DRIVE_MODEL}", reasoning_effort "high", wait_seconds 0.
2. Follow each session until it ends, polling with mcp__deepseek__deepseek_wait (wait_seconds ${DSMP.pollSeconds}):
- "needs_response" with an approval: allow only a command that runs "zt-digest figures -Step explain-check", a read of a file under "${WORK}", and writing a JSON file under "${WORK}\\out"; reject anything else. A question: answer with custom text "按任务说明自行决定，不需要询问。" through mcp__deepseek__deepseek_respond.
- A session that ended in an error or without an answer gets ONE more attempt in a new conversation with the same arguments; then the job failed.
3. Delete every conversation you started (mcp__deepseek__deepseek_delete, never archive) as soon as you have its answer.
You have no terminal tabs, no Monitor loop and no general shell, and you never start another agent.
Return done (the number of jobs that ended), failed (the ids of the jobs that failed) and not_deleted (session ids that stayed open).`
    let line = null
    let saved = 0
    for (let round = 1; round <= 2; round++) {
      const started = await step(`start ${round}`, `zt-digest figures -Step explain-start -Work "${WORK}" ${named} -Model ${ZT_DRIVE_MODEL}`)
      if (started.ok !== true) return { status: 'failed', error: started.error || 'explain-start failed' }
      saved += Number(started.settled || 0)
      log(`Figure explanations: ${started.figures || 0} figures, ${started.settled || 0} answers settled, ${started.jobs || 0} reader jobs.`)
      if (!Number(started.jobs || 0)) return { status: 'done', figures: Number(started.figures || 0), saved, open: 0, not_deleted: deepseek.not_deleted || [] }
      const managers = []
      for (let k = 1; k <= Number(started.chunks || 0); k++) {
        const label = `figure explanations ${round}: read ${k} of ${started.chunks}`
        const file = String(started.file).replace(/\.json$/, `-${String(k).padStart(3, '0')}.json`)
        managers.push(dsmpSlot(() => ctx.agent(manager(label, file), { role: 'manager', label, phase: ctx.phase, schema: SCHEMA }))
          .then((r) => { for (const id of (r && r.not_deleted) || []) (deepseek.not_deleted = deepseek.not_deleted || []).push(id) })
          .catch((error) => log(`${label}: the manager answered no report (${String((error && error.message) || error).slice(0, 160)}); its images stay open.`)))
      }
      await Promise.all(managers)
      try { await dsmpSlot(() => dsCleanupWorkspaces([WORK], { label: `${ctx.stage}: workspace`, phase: ctx.phase })) } catch (error) { log(`Figure explanations: the workspace stays listed (${String((error && error.message) || error).slice(0, 120)}).`) }
      line = await step(`finish ${round}`, `zt-digest figures -Step explain-finish -Work "${WORK}" -Model ${ZT_DRIVE_MODEL}`)
      if (line.ok !== true) return { status: 'failed', error: line.error || 'explain-finish failed' }
      saved += Number(line.saved || 0)
      log(`Figure explanations: ${line.saved || 0} saved in the Scholar Archive, ${line.open || 0} open${line.refused ? `, ${line.refused} refused (${line.first_refused})` : ''}.`)
      if (!Number(line.open || 0)) return { status: 'done', saved, open: 0, not_deleted: deepseek.not_deleted || [] }
    }
    return { status: 'failed', error: `explanations_open: ${line.open} images have no explanation after two rounds` }
  },
})
// ==== end module zt-figure-explain ====
