// ==== module zt-model-lib v15 ====
// The model layer's machinery (docs/zt-digest-framework.md), moved from zt-digest-lib: the model list and the
// model compiler per batch: DeepSeek brief → Sonnet architect plan → DeepSeek fill, derive, review, split →
// report and clean-up. A book type installs the model layer with its profile (economics: the economic
// architect; mathematics: the math supplements and no Theory Card). zmlMake(ctx, params) returns start,
// elaborate, finish, logModels, modelCleanup.
// 9 (docs/digest-claude-token-proposal.md P1/P7): a batch registers its models' references with ONE `model-deps -Units`
// command, and a close round that sends nothing back writes the batch report in the same process (`-Step close -Report`).
// 10 (docs/workflow-simplicity-proposal.md S1, S10): whether a batch fills only or derives and reviews is read from
// the plans the architect wrote (`model -Step plan-status`), never from its answer.
// 11 (#177, docs/workflow-module-structure-proposal.md): every deterministic chain is ONE `model-steps.ps1` step
// run by a DeepSeek drive job (zt-workflow-lib's shared `ztDrive`); the content jobs (brief, fill, derive,
// review, split, close) run in Haiku managers through zt-digest-lib's `runJobs`; the Sonnet architect stays the
// one designer role. The module relays no command and reads no file: each step answers one JSON line.
// 12 (#180): a step's line is at most 400 characters, so the module reads counts, packed lists and file paths,
// never per-unit rows. The architect batching moved into the `start` step (it answers the plan as
// `concept:a,b|formal:c`), a plan is a packed `address:status` list, and a batch's report answers the models'
// statuses instead of their rows. The per-model rows stay in the compiler's own `report.json` and the step's
// detail file; the run report's per-model facts come from the archive (`run_report.placeholders.models`).
// 13 (live run 2026-10-26, staging4): the fill line of six formal models named each one twice in full and overflowed
// its 400 characters; the overflow answered {ok, detail}, the module read a fill with no jobs and six models stayed
// unwritten. The steps now name models without the book key (`2|a`) and the module adds it back (`full`); the
// brief's skipped models travel the same way (a list, no longer a count). An overflow is now a failed step.
// 14 (#194, docs/model-one-conversation-proposal.md §7): one DeepSeek conversation per model. The brief job
// (`<slug>:brief`) starts it; the compiler's fill, derive, review, split and close jobs name `continue` for it,
// so the brief round's sessions are kept and every later content round gets the session table. The math variant
// (`profile.independentReview`) reviews in the reviewer's own conversation (`<slug>:review`), and the review
// step is told so with `-IndependentReview`.
// 15 (#195, §3.7): after the close rounds the batch runs the explain round: one job per model of the batch that
// still has a Derivation part without 思路 or 解读, continuing the model's conversation, carrying the Sonnet
// architect's `explain` design from plan.json. `explain-apply` writes the accepted text and lists a fix job for
// a refused part, which the same conversation works (the compiler allows three rounds in all). `finish-batch`
// then records the Explain unit of a model whose explanations the batch wrote; a model whose explanation stays
// open leaves its unit ready for the explain stage.
function zmlMake(ctx, p) {
  const Z = zdMake(ctx, p)
  const {
    A, KEY, OUT, SCRIPTS, DSH_EFFORT, LIMITS, FILL_SKILLS, profile, ARCH_DOC, ARCH_SUPPLEMENT, MODEL_WORK, PER_SONNET,
    PER_CONCEPT, DSH_PER_BATCH, REBUILD, LEVEL, BRIEF_EFFORT, DERIVE_EFFORT, REVIEW_EFFORT, REVIEW_ROUNDS, SKIP_FORMS,
    zAgent, answerLines, runJobs, psQuote, LEDGER,
  } = Z
  // The model stage's one step script: every deterministic chain runs there, one process per step.
  const MODEL_STEPS = p.modelScript || `${SCRIPTS}\\model-steps.ps1`
  const phase = 'Models'
  const tail = (text) => String(text || '').trim().slice(0, 400)
  const n = (v) => Number(v || 0)
  // A packed `a:b;c:d` list of pairs (#180).
  const pairs = (text, sep = ';') => String(text || '').split(sep).filter(Boolean).map((entry) => {
    const at = entry.indexOf(':')
    return at < 0 ? [entry, ''] : [entry.slice(0, at), entry.slice(at + 1)]
  })
  // A model the step lines name without its book key (`2|a`), as its full address (`Key/2|a`).
  const full = (at) => { const t = String(at || '').trim(); return !t || t.includes('/') ? t : `${KEY}/${t}` }
  const fullPairs = (text) => pairs(text).map(([at, v]) => [full(at), v])
  // One deterministic step: the model-steps.ps1 call a DeepSeek drive job runs and answers with its one JSON line.
  // A failed step is returned for the module to stop with; the owner's typed refusal stops the stage here.
  const modelStep = async (label, step, o = {}) => {
    const parts = [`pwsh -NoProfile -File "${MODEL_STEPS}"`, `-Step ${step}`, `-Out "${OUT}"`]
    if (o.work) parts.push(`-Work "${o.work}"`)
    if (o.models && o.models.length) parts.push(`-Models "${o.models.join(',')}"`)
    if (o.units && o.units.length) parts.push(`-Units "${o.units.map((u) => `${u.id}:${u.token}`).join(',')}"`)
    if (o.citationKey) parts.push(`-CitationKey "${o.citationKey}"`)
    if (o.need) parts.push(`-Need ${psQuote(o.need)}`)
    if (o.level) parts.push(`-Level ${o.level}`)
    if (o.rebuild) parts.push('-Rebuild')
    if (o.changed) parts.push('-Changed')
    if (o.take) parts.push(`-Take ${o.take}`)
    if (o.owner) parts.push(`-Owner ${psQuote(o.owner)}`)
    if (o.only && o.only.length) parts.push(`-Only "${o.only.join(',')}"`)
    if (o.skipForms && o.skipForms.length) parts.push(`-SkipForms "${o.skipForms.join(',')}"`)
    if (o.perConcept) parts.push(`-PerConcept ${o.perConcept}`)
    if (o.perSonnet) parts.push(`-PerSonnet ${o.perSonnet}`)
    // #194: the math variant's reviewer opens its own conversation; the compiler names it `<slug>:review`.
    if (o.independentReview) parts.push('-IndependentReview')
    // #195: the batch's explain round, in the models' own conversations, from the architect's design.
    if (o.batch) parts.push('-Batch')
    if (o.limits) parts.push(o.limits)
    // The math variant's own instruction files: every compiler command of that variant takes them.
    for (const [flag, file] of Object.entries(profile.compilerDocs || {})) parts.push(`-${flag} "${OUT}\\${file}"`)
    if (o.tool) parts.push(`-Tool ${o.tool}`)
    const json = await ztDrive(ctx, { label, step, command: parts.join(' '), out: OUT, cwd: o.cwd || OUT, phase, ledger: LEDGER, deepseek: o.deepseek })
    const lock = ztLockRefusal(JSON.stringify(json || {})) || ((json && (json.code === Z.LOCK || json.error === Z.LOCK)) ? json : null)
    if (lock) throw ztLockStop(label, lock.refusal || lock)
    const gate = json && (json.gate_complete !== undefined || json.gate_open !== undefined)
      ? { complete: json.gate_complete === true, counts: { open: n(json.gate_open) } }
      : (json && json.gate ? json.gate : null)
    if (!json || json.ok !== true) return { ok: false, error: tail((json && json.error) || `the ${step} step failed`), gate, raw: json }
    return { ...json, gate }
  }
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
  // The block's start step: release the claims a stopped run left, list the Model Cards, claim up to `take`
  // ready units, and answer the block's architect batches as `concept:<addresses>|formal:<addresses>` (#180: the
  // batching is the step's, because a module reads no listing row).
  const start = async (o = {}) => modelStep(`find models: block ${o.block || 1}`, 'start', {
    citationKey: KEY, take: o.take || 8, owner: o.owner || 'models', cwd: o.cwd, deepseek: o.deepseek,
    only: A.only, skipForms: SKIP_FORMS, perConcept: PER_CONCEPT, perSonnet: PER_SONNET, level: LEVEL, rebuild: REBUILD,
  })
  // One batch of up to eight models: brief -> the Sonnet architect's plan -> fill -> derive -> review and
  // split rounds -> the close rounds -> the report. Every command is one step; every job round is one or more
  // Haiku managers reading the step's job file.
  const compileBatch = async (batch, o = {}) => {
    const addresses = batch.models
    const name = `m-${slug(addresses[0])}`
    const dsh = `${MODEL_WORK}\\${name}`
    managerFolders.push(dsh)
    const tag = `${addresses[0]}${addresses.length > 1 ? ` +${addresses.length - 1}` : ''}`
    const sessions = new Map()
    const table = () => [...sessions].map(([id, session]) => `${id} ${session}`).join('\n')
    const step = (what, s, more = {}) => modelStep(`${what}: ${tag}`, s, { work: dsh, cwd: dsh, ...more, deepseek: o.deepseek })
    // The step's content jobs: one or more Haiku managers, at most eight jobs each. A job that names
    // "continue" reuses the conversation its earlier job started (the table of this batch's own jobs).
    const content = async (what, file, count, effort) => {
      const total = Number(count || 0)
      if (!total) return []
      const results = await runJobs(`${what}: ${tag}`, file, total, {
        what: 'model compiler', dsh, writes: dsh, skills: FILL_SKILLS, effort: effort || DSH_EFFORT, phase, run: true, keep: true, table: table(),
      })
      for (const r of results) { dshJobs++; if (r && r.id && r.session_id) sessions.set(r.id, r.session_id) }
      return results
    }
    const failed = (why) => addresses.map((model) => ({ model, status: 'failed', issues: [why] }))
    // 1. Brief: the script writes each context pack; DeepSeek reads the source and writes brief.md.
    const brief = await step('brief', 'brief', { models: addresses, rebuild: REBUILD, level: LEVEL, limits: LIMITS })
    if (!brief.ok) return failed(`brief failed: ${brief.error}`)
    if (Number(brief.jobs || 0)) await content('brief', brief.file, brief.jobs, BRIEF_EFFORT)
    const skipped = new Set((Array.isArray(brief.skipped) ? brief.skipped : String(brief.skipped || '').split(',')).map(full).filter(Boolean))
    const todo = addresses.filter((at) => !skipped.has(at))
    if (!todo.length) return addresses.map((model) => ({ model, status: 'skipped' }))
    // 2. Plan: Sonnet reads the briefs and writes plan.json for each model; nothing else.
    const levelOf = (at) => (LEVEL || (batch.kind === 'concept' ? 'concept' : 'formal'))
    const entry = (at, i) => `## Model ${i + 1}
MODEL: ${at}
DIR: ${dsh}\\${slug(at)}
LEVEL NOW: ${levelOf(at)}`
    const planned = await zAgent(
      `You are the Sonnet architect in the Models stage of the ZET digest of 《${A.bookTitle || KEY}》. Read your instructions first: ${ARCH_DOC}${ARCH_SUPPLEMENT ? `, then ${profile.architectSupplement.name || 'the supplement'} ${ARCH_SUPPLEMENT}` : ''} (Read tool). Then follow them for each model in turn. These models are near each other in the book: where one part serves two of them, plan it in the first and take it in from the second (\`<model>#<id>\`). Each model's own brief is \`brief.md\` in its DIR; read it for the book's wording and the model's title.
COMPILER: ${Z.tool('model-compiler')}
WORK: ${dsh}
LIMITS: ${LIMITS}${REBUILD ? '\nREBUILD: rebuild each model even if it already has parts.' : ''}${LEVEL ? `\nLEVEL: ${LEVEL}` : ''}

${todo.map(entry).join('\n\n')}

Return plans: one entry per model, in this order.`,
      { label: `plan: ${tag}`, phase, model: 'sonnet', schema: PLANS },
    )
    // 3. Fill: plan-status, the batch's model-deps references, the defer of the models they release, and the
    // fill, in ONE step. S10: the plans as the architect wrote them (plan.json per model), read by the compiler;
    // the answer only adds notes.
    const fill = await step('fill', 'fill', { models: todo, limits: LIMITS })
    if (!fill.ok) return failed(`fill failed: ${fill.error}`)
    const said = (planned && planned.plans) || []
    const statusOf = new Map(fullPairs(fill.statuses))
    const plans = todo.map((model) => ({ ...(said.find((y) => y.model === model) || {}), model, status: statusOf.get(model) || 'failed', level: levelOf(model) }))
    const deferred = new Map(fullPairs(fill.deferred).map(([at, why]) => [at, why || 'refused']))
    if (deferred.size) log(`${tag}: ${[...deferred].map(([at, why]) => `${at} ${why === 'waiting' ? 'waits for the model it takes a part from' : 'has a reference the ledger refused'}`).join('; ')}.`)
    const outcome = (at) => ({ model: at, status: deferred.get(at), issues: [deferred.get(at) === 'waiting' ? 'waits for the model it takes a part from' : 'a reference to another model\'s part was refused'] })
    if (!plans.some((x) => x.status === 'planned' && !deferred.has(x.model))) {
      log(`${tag}: no model of the batch is planned; the batch builds nothing.`)
      return addresses.map((m) => skipped.has(m) ? { model: m, status: 'skipped' }
        : deferred.has(m) ? outcome(m)
          : { model: m, status: 'failed' })
    }
    // A composer that needs more parts for one part asks through its answer; the split step takes the need.
    const splitNeeds = async (results) => {
      const need = answerLines({ results }).filter((l) => l.status === 'split' && l.address && l.need > 1).map((l) => `${l.address}=${l.need}`)
      if (!need.length) return
      const s = await step('split', 'split', { need: need.join(';') })
      if (!s.ok) { log(`split needs ${tag}: ${s.error}`); return }
      if (Number(s.jobs || 0)) await content('split', s.file, s.jobs, DERIVE_EFFORT)
    }
    // 4. The fill, 5. derive and 6. review (only where the plans have Derivation parts).
    const written = await content('fill', fill.file, fill.jobs, DSH_EFFORT)
    await splitNeeds(written)
    if (fill.derive) {
      const derive = await step('derive', 'derive')
      if (!derive.ok) return failed(`derive failed: ${derive.error}`)
      await splitNeeds(await content('derive', derive.file, derive.jobs, DERIVE_EFFORT))
      for (let round = 0; round < REVIEW_ROUNDS; round++) {
        const review = await step(`review ${round + 1}`, 'review', { changed: round > 0, independentReview: !!profile.independentReview })
        if (!review.ok) return failed(`review failed: ${review.error}`)
        if (!Number(review.jobs || 0)) break
        const verdicts = answerLines({ results: await content(`review ${round + 1}`, review.file, review.jobs, REVIEW_EFFORT) })
          .reduce((s, l) => ({ split: s.split + (Number(l.split) || 0), fix: s.fix + (Number(l.fix) || 0) }), { split: 0, fix: 0 })
        log(`review ${round + 1} ${tag}: ${verdicts.split} parts to split, ${verdicts.fix} to fix`)
        if (!verdicts.split && !verdicts.fix) break
        const split = await step(`split ${round + 1}`, 'split')
        if (!split.ok) return failed(`split failed: ${split.error}`)
        await splitNeeds(await content(`split ${round + 1}`, split.file, split.jobs, DERIVE_EFFORT))
      }
    }
    // 7. Close: the build faults the compiler still sees go back once, as 修正 notes, to the conversations that
    // wrote those Cards; the round that sends nothing back also writes the batch report (-Report). At most
    // CLOSE_ROUNDS rounds: a writer can answer a note and leave the fault in place.
    const CLOSE_ROUNDS = 3
    let reported = null
    for (let round = 0; round < CLOSE_ROUNDS; round++) {
      const close = await step(round ? `close ${round + 1}` : 'close', 'close')
      if (!close.ok) return failed(`close failed: ${close.error}`)
      // #180: the close round's report is its models' packed statuses, never their rows.
      if (Number(close.report_models || 0)) reported = new Map(fullPairs(close.statuses))
      if (!close.faults) break
      log(`close ${round + 1} ${tag}: ${close.faults} build fault(s) sent back in ${Number(close.jobs || 0)} follow-up(s)${Number(close.unowned || 0) ? `, ${close.unowned} in Cards no job of this batch wrote` : ''}`)
      if (!Number(close.jobs || 0)) break
      await content(`close ${round + 1}`, close.file, close.jobs, DERIVE_EFFORT)
    }
    // 7.5 Explain (#195, §3.7): after the close rounds the batch's models write 思路, 说明 and 解读 in
    // their own conversations, from the architect's explain design. A refused part goes back to the
    // same conversation; the compiler allows three rounds in all, and explain-finish later records the
    // unit (finish-batch, or the explain stage when a part stays refused).
    let explained = 0
    if (fill.derive) {
      const planned = await step('explain', 'explain', { models: todo, batch: true })
      if (!planned.ok) return failed(`explain failed: ${planned.error}`)
      if (Number(planned.jobs || 0)) await content('explain', planned.file, planned.jobs, DSH_EFFORT)
      for (let round = 0; round < 3; round++) {
        const applied = await step(round ? `explain-apply ${round + 1}` : 'explain-apply', 'explain-apply')
        if (!applied.ok) return failed(`explain failed: ${applied.error}`)
        explained += Number(applied.applied || 0)
        if (!Number(applied.jobs || 0)) break
        await content(`explain fix ${round + 1}`, applied.file, applied.jobs, DSH_EFFORT)
      }
      log(`explain ${tag}: ${explained} Derivation part(s) written in the models' own conversations, ${Number(planned.parts || 0)} planned.`)
    }
    // 8. Report, when the last close round sent follow-ups and wrote none.
    let statuses = reported
    if (!statuses) {
      const report = await step('report', 'report')
      if (!report.ok) return failed(`report failed: ${report.error}`)
      statuses = new Map(fullPairs(report.statuses))
    }
    return addresses.map((m) => {
      if (skipped.has(m)) return { model: m, status: 'skipped' }
      if (deferred.has(m)) return outcome(m)
      if (!statuses.has(m)) return { model: m, status: 'failed', issues: ['no report from the compiler'] }
      return { model: m, status: statuses.get(m), level: levelOf(m) }
    })
  }
  // The block's models, in the batches the start step planned: a model whose level the run changes is never
  // skipped; a form the profile skips (math: theory) never goes in; a model already built with no issue is left
  // out. The batches travel in the start line's `plan` (#180).
  const elaborate = async (started, label, o = {}) => {
    const plan = String((started && started.plan) || '')
    const batches = plan.split(';').filter(Boolean).map((entry) => {
      const at = entry.indexOf(':')
      return { kind: entry.slice(0, at), models: entry.slice(at + 1).split(',').filter(Boolean).map(full) }
    }).filter((b) => b.models.length)
    if (!batches.length) {
      log(`${label}: no model of the block is planned; the block builds nothing.`)
      return []
    }
    log(`${label}: ${batches.reduce((sum, b) => sum + b.models.length, 0)} models in ${batches.length} batches, one Sonnet architect each; up to ${DSH_PER_BATCH} DeepSeek runs at a time per batch.`)
    const results = (await parallel(batches.map((b) => () => compileBatch(b, o).catch((error) => { log(`batch ${b.models[0]}: ${error}`); return null })))).flatMap((r, i) => r || batches[i].models.map((model) => ({ model, status: 'failed' })))
    return results
  }
  // One block's checked completions: the finish step runs each held unit's own verifier in ONE command and
  // records a unit it cannot finish against that unit alone. The ledger stays the judge.
  const finish = async (o = {}) => {
    const r = await modelStep('models: finish', 'finish', { ...o, deepseek: o.deepseek })
    if (!r.ok) return { ok: false, error: r.error, done: 0, failed: 0, partial: 0, gate: r.gate }
    return { ok: true, done: n(r.done), failed: n(r.failed), partial: n(r.partial), gate: r.gate }
  }
  // #180: the stage's log from the batch results (a status each), never a per-model row.
  const logModels = (reports) => {
    const done = reports.filter((r) => r.status === 'done')
    log(`Models: ${done.length} structured, ${reports.filter((r) => r.status === 'skipped').length} skipped, failed ${reports.filter((r) => r.status === 'failed').map((r) => r.model).join(', ') || 'none'}; ${done.filter((r) => r.level === 'concept').length} concept, ${done.filter((r) => r.level !== 'concept').length} formal; ${dshJobs} DeepSeek jobs.`)
  }
  // R323: the model stage no longer deletes conversations or folders itself. Every batch conversation was
  // recorded when the provider returned its id, so the run's cleanup stage deletes exactly those ids after
  // the owning work settles; a batch folder is disposable scratch only when its accepted plan is retained
  // first (R328/R329). This resolves that the sweep owes nothing.
  const modelCleanup = async () => {
    if (!managerFolders.length) return null
    log(`Model clean-up: ${managerFolders.length} batch folders stay until the run-level cleanup settles their recorded conversations.`)
    return { late_deleted: 0, folders: managerFolders.length, deferred: true }
  }

  return { Z, start, elaborate, finish, logModels, modelCleanup, dshJobs: () => dshJobs }
}

defineModule({
  id: 'zt-model-lib', version: '15', library: true, requires: ['zt-digest-lib'],
  summary: 'Library: the model layer\'s model-steps.ps1 steps (list, claim and batch; brief, fill, derive, review, split, close, report, explain, explain-apply, finish), the Sonnet architect, and the model compiler\'s report and clean-up.',
})
// ==== end module zt-model-lib ====



