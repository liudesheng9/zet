// ==== module zt-model-lib v11 ====
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
    if (o.limits) parts.push(o.limits)
    // The math variant's own instruction files: every compiler command of that variant takes them.
    for (const [flag, file] of Object.entries(profile.compilerDocs || {})) parts.push(`-${flag} "${OUT}\\${file}"`)
    if (o.tool) parts.push(`-Tool ${o.tool}`)
    const json = await ztDrive(ctx, { label, step, command: parts.join(' '), out: OUT, cwd: o.cwd || OUT, phase, ledger: LEDGER, deepseek: o.deepseek })
    const lock = ztLockRefusal(JSON.stringify(json || {})) || ((json && (json.code === Z.LOCK || json.error === Z.LOCK)) ? json : null)
    if (lock) throw ztLockStop(label, lock.refusal || lock)
    if (!json || json.ok !== true) return { ok: false, error: tail((json && json.error) || `the ${step} step failed`), gate: json && json.gate, raw: json }
    return json
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
  // The address a claimed Model unit names (the compiler's own reading of a claim's inputs).
  const addressOf = (unit) => (unit && unit.model) || ''
  const slug = (address) => String(address).replace(/[\/|]/g, '_')
  const managerFolders = []
  let dshJobs = 0
  // The block's start step: release the claims a stopped run left, list the Model Cards, claim up to `take`
  // ready units and name the models they build.
  const start = async (o = {}) => modelStep(`find models: block ${o.block || 1}`, 'start', {
    citationKey: KEY, take: o.take || 8, owner: o.owner || 'models', cwd: o.cwd, deepseek: o.deepseek,
  })
  // One batch of up to eight models: brief -> the Sonnet architect's plan -> fill -> derive -> review and
  // split rounds -> the close rounds -> the report. Every command is one step; every job round is one or more
  // Haiku managers reading the step's job file.
  const compileBatch = async (batch, units, o = {}) => {
    const name = `m-${slug(batch[0].address)}`
    const dsh = `${MODEL_WORK}\\${name}`
    managerFolders.push(dsh)
    const tag = `${batch[0].address}${batch.length > 1 ? ` +${batch.length - 1}` : ''}`
    const claims = {}
    for (const u of units) { const at = addressOf(u); if (at) claims[at] = u }
    const sessions = new Map()
    const table = () => [...sessions].map(([id, session]) => `${id} ${session}`).join('\n')
    const step = (what, s, more = {}) => modelStep(`${what}: ${tag}`, s, { work: dsh, cwd: dsh, ...more, deepseek: o.deepseek })
    // The step's content jobs: one or more Haiku managers, at most eight jobs each. A job that names
    // "continue" reuses the conversation its earlier job started (the table of this batch's own jobs).
    const content = async (what, file, count, effort) => {
      const n = Number(count || 0)
      if (!n) return []
      const results = await runJobs(`${what}: ${tag}`, file, n, {
        what: 'model compiler', dsh, writes: dsh, skills: FILL_SKILLS, effort: effort || DSH_EFFORT, phase, run: true, keep: true, table: table(),
      })
      for (const r of results) { dshJobs++; if (r && r.id && r.session_id) sessions.set(r.id, r.session_id) }
      return results
    }
    const failed = (why) => batch.map((m) => ({ model: m.address, status: 'failed', parts: [], charts: 0, issues: [why] }))
    // 1. Brief: the script writes each context pack; DeepSeek reads the source and writes brief.md.
    const brief = await step('brief', 'brief', { models: batch.map((m) => m.address), rebuild: REBUILD, level: LEVEL, limits: LIMITS })
    if (!brief.ok) return failed(`brief failed: ${brief.error}`)
    if (Number(brief.jobs || 0)) await content('brief', brief.file, brief.jobs, BRIEF_EFFORT)
    const skipped = new Set(brief.skipped || [])
    const todo = batch.filter((m) => !skipped.has(m.address))
    if (!todo.length) return batch.map((m) => ({ model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }))
    // 2. Plan: Sonnet reads the briefs and writes plan.json for each model; nothing else.
    const entry = (m, i) => `## Model ${i + 1}
MODEL: ${m.address} (${m.title || ''})
DIR: ${dsh}\\${slug(m.address)}
LEVEL NOW: ${m.level || 'none'}`
    const planned = await zAgent(
      `You are the Sonnet architect in the Models stage of the ZET digest of 《${A.bookTitle || KEY}》. Read your instructions first: ${ARCH_DOC}${ARCH_SUPPLEMENT ? `, then ${profile.architectSupplement.name || 'the supplement'} ${ARCH_SUPPLEMENT}` : ''} (Read tool). Then follow them for each model in turn. These models are near each other in the book: where one part serves two of them, plan it in the first and take it in from the second (\`<model>#<id>\`).
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
    const fillUnits = todo.filter((m) => claims[m.address]).map((m) => claims[m.address])
    const fill = await step('fill', 'fill', { models: todo.map((m) => m.address), units: fillUnits, limits: LIMITS })
    if (!fill.ok) return failed(`fill failed: ${fill.error}`)
    const said = (planned && planned.plans) || []
    const plans = (fill.plans || []).map((x) => ({ ...(said.find((y) => y.model === x.model) || {}), model: x.model, status: x.status, level: x.level, parts: x.parts, derivations: x.derivations }))
    const deferred = new Map((fill.deferred || []).map((d) => [d.model, d.why]))
    if (deferred.size) log(`${tag}: ${[...deferred].map(([at, why]) => `${at} ${why === 'waiting' ? 'waits for the model it takes a part from' : 'has a reference the ledger refused'}`).join('; ')}.`)
    const outcome = (m) => ({ model: m.address, status: deferred.get(m.address), parts: [], charts: 0, issues: [deferred.get(m.address) === 'waiting' ? 'waits for the model it takes a part from' : 'a reference to another model\'s part was refused'] })
    if (!plans.some((x) => x.status === 'planned' && !deferred.has(x.model))) {
      log(`${tag}: no model of the batch is planned; the batch builds nothing.`)
      return batch.map((m) => skipped.has(m.address) ? { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
        : deferred.has(m.address) ? outcome(m)
          : { model: m.address, status: (plans.find((x) => x.model === m.address) || {}).status || 'failed', parts: [], charts: 0, issues: [] })
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
        const review = await step(`review ${round + 1}`, 'review', { changed: round > 0 })
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
    let closeReport = null
    for (let round = 0; round < CLOSE_ROUNDS; round++) {
      const close = await step(round ? `close ${round + 1}` : 'close', 'close')
      if (!close.ok) return failed(`close failed: ${close.error}`)
      if (close.report && Array.isArray(close.report.models)) closeReport = close.report
      if (!close.faults) break
      log(`close ${round + 1} ${tag}: ${close.faults} build fault(s) sent back in ${Number(close.jobs || 0)} follow-up(s)${(close.unowned || []).length ? `, ${close.unowned.length} in Cards no job of this batch wrote` : ''}`)
      if (!Number(close.jobs || 0)) break
      await content(`close ${round + 1}`, close.file, close.jobs, DERIVE_EFFORT)
    }
    // 8. Report, when the last close round sent follow-ups and wrote none.
    let reports = (closeReport && closeReport.models) || []
    if (!reports.length) {
      const report = await step('report', 'report')
      if (!report.ok) return failed(`report failed: ${report.error}`)
      reports = report.models || []
    }
    return batch.map((m) => {
      const r = reports.find((x) => x.model === m.address)
      const pl = plans.find((x) => x.model === m.address) || {}
      if (skipped.has(m.address)) return { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
      if (deferred.has(m.address)) return outcome(m)
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
  // The block's models, in batches: a model whose level the run changes is never skipped; a form the profile
  // skips (math: theory) never goes in; a model already built with no issue is left out.
  const elaborate = async (models, units, label, o = {}) => {
    const only = Array.isArray(A.only) && A.only.length ? new Set(A.only) : null
    const todo = models.filter((m) => !SKIP_FORMS.includes(m.form)).filter((m) => !only || only.has(m.address)).filter((m) => REBUILD || (LEVEL && m.level !== LEVEL) || !(m.parts > 0 && !m.issues))
      .sort((a, b) => groupNo(a) - groupNo(b) || String(a.address).localeCompare(String(b.address)))
    if (todo.length < models.length) log(`${label}: ${models.length - todo.length} of ${models.length} models already have their parts; skipped.`)
    const concept = todo.filter((m) => (LEVEL || m.level) === 'concept')
    const formal = todo.filter((m) => (LEVEL || m.level) !== 'concept')
    const batches = []
    for (let i = 0; i < concept.length; i += PER_CONCEPT) batches.push({ kind: 'concept', models: concept.slice(i, i + PER_CONCEPT) })
    for (let i = 0; i < formal.length; i += PER_SONNET) batches.push({ kind: 'formal', models: formal.slice(i, i + PER_SONNET) })
    log(`${label}: ${todo.length} models (${concept.length} concept, ${formal.length} formal or undecided) in ${batches.length} batches, one Sonnet architect each; up to ${DSH_PER_BATCH} DeepSeek runs at a time per batch.`)
    const results = (await parallel(batches.map((b) => () => compileBatch(b.models, units, o).catch((error) => { log(`batch ${b.models[0].address}: ${error}`); return null })))).flatMap((r, i) => r || batches[i].models.map(() => null))
    const ordered = batches.flatMap((b) => b.models)
    return results.map((r, i) => r || { model: ordered[i].address, status: 'failed', parts: [], charts: 0, issues: ['no report from its batch'] })
  }
  // One block's checked completions: the finish step runs each held unit's own verifier in ONE command and
  // records a unit it cannot finish against that unit alone. The ledger stays the judge.
  const finish = async (units, o = {}) => {
    if (!units.length) return { ok: true, results: [], done: [], failed: [], gate: o.gate || null }
    const r = await modelStep(`models: finish ${units.map((u) => u.id).join(', ')}`, 'finish', { units, deepseek: o.deepseek })
    if (!r.ok) return { ok: false, error: r.error, results: [], done: [], failed: [], gate: r.gate }
    return { ok: true, results: r.results || [], done: r.done || [], failed: r.failed || [], gate: r.gate }
  }
  const kindTally = (reports) => {
    const tally = {}
    for (const r of reports) for (const [kind, n] of Object.entries(r.kinds || {})) tally[kind] = (tally[kind] || 0) + n
    return Object.entries(tally).sort((a, b) => b[1] - a[1]).map(([kind, n]) => `${kind} ${n}`).join(', ') || 'no kinds reported'
  }
  // The compiler's report line gives counts (parts, core, moved); an older line gave lists. Either reads.
  const nOf = (v) => Array.isArray(v) ? v.length : (Number(v) || 0)
  const coreOf = (r) => r.core != null ? (Number(r.core) || 0) : Array.isArray(r.parts) ? r.parts.filter((pp) => pp && pp.emphasis === 'core').length : 0
  const logModels = (reports) => {
    const done = reports.filter((r) => r.status === 'done')
    const sum = (field) => done.reduce((s, r) => s + (r[field] || 0), 0)
    const deriv = (field) => done.reduce((s, r) => s + ((r.derivations && r.derivations[field]) || 0), 0)
    log(`Build: ${deriv('total')} Derivation parts in ${deriv('chains')} chains of two or more (longest ${done.reduce((m, r) => Math.max(m, (r.derivations && r.derivations.longest) || 0), 0)}), ${sum('split')} parts split by review or for the Card size, ${done.reduce((s, r) => s + nOf(r.moved), 0)} Cards moved into ranges, ${sum('shared')} shared parts taken in, ${sum('tolerated')} tolerated premises, ${sum('style')} style faults left, ${done.reduce((s, r) => s + ((r.todo || []).length), 0)} parts still 待写; ${dshJobs} DeepSeek jobs.`)
    log(`Levels: ${done.filter((r) => r.level === 'concept').length} concept models (level 1), ${done.filter((r) => r.level === 'formal').length} formal models (level 2), ${done.filter((r) => !r.level).length} without a level.`)
    log(`Models: ${done.length} structured (${done.reduce((s, r) => s + nOf(r.parts), 0)} parts, ${done.reduce((s, r) => s + coreOf(r), 0)} core, ${sum('relations')} relations (${kindTally(done)}), ${sum('neighbours')} neighbours, ${sum('loose')} loose, ${sum('charts')} charts), ${reports.filter((r) => r.status === 'skipped').length} skipped, failed ${reports.filter((r) => r.status === 'failed').map((r) => r.model).join(', ') || 'none'}; open issues ${reports.reduce((s, r) => s + nOf(r.issues), 0)}.`)
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
  id: 'zt-model-lib', version: '11', library: true, requires: ['zt-digest-lib'],
  summary: 'Library: the model layer\'s model-steps.ps1 steps (list and claim, brief, fill, derive, review, split, close, report, finish), the Sonnet architect, and the model compiler\'s report and clean-up.',
})
// ==== end module zt-model-lib ====
