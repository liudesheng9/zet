// ==== module zt-model-lib v8 ====
// The model layer's machinery (docs/zt-digest-framework.md), moved from zt-digest-lib: the model list
// (list-models.ps1) and the model compiler per batch: DeepSeek brief → Sonnet architect plan → DeepSeek fill,
// derive, review, split → report and clean-up (model-compiler.ps1 does every structural step). A book type
// installs the model layer with its profile (economics: the economic architect; mathematics: the math
// supplements and no Theory Card). zmlMake(ctx, params) returns listModels, elaborateAll, logModels, modelCleanup.
function zmlMake(ctx, p) {
  const Z = zdMake(ctx, p)
  const { A, KEY, OUT, SCRIPTS, DSH_EFFORT, LIMITS, COMPILER, FILL_SKILLS, profile, ARCH_DOC, ARCH_SUPPLEMENT, MODEL_WORK, PER_SONNET,
    PER_CONCEPT, DSH_PER_BATCH, REBUILD, LEVEL, BRIEF_EFFORT, DERIVE_EFFORT, REVIEW_EFFORT, REVIEW_ROUNDS, COMPILER_DOCS, SKIP_FORMS,
    zAgent, drive, answerLines } = Z
  const MODEL_LIST = {
    type: 'object',
    properties: {
      ok: { type: 'boolean' }, error: { type: 'string' },
      models: { type: 'array', items: { type: 'object', properties: { address: { type: 'string' }, title: { type: 'string' }, level: { type: ['string', 'null'] }, parts: { type: 'number' }, issues: { type: 'number' }, faults: { type: 'number' } }, required: ['address'] } },
    },
    required: ['ok', 'models'],
  }
  const listModels = (selector, label) => zAgent(
    `List the Model Cards of a ZET book digest. Run this one PowerShell command in the foreground with timeout_ms: 600000 and read its last output line, a compact JSON object {"models":[{"address","title","group","level","parts","issues","faults"}]}:
${Z.tool('list-models')} -Staging "${OUT}" ${selector}
If it fails, return ok=false with the error text and models=[]; do not retry with other options. Otherwise return ok=true and the models array exactly.`,
    { label, phase: 'Models', model: 'haiku', schema: MODEL_LIST },
  )

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
  const compileBatch = async (batch, kind, claims = null) => {
    const name = `m-${slug(batch[0].address)}`
    const dsh = `${MODEL_WORK}\\${name}`
    managerFolders.push(dsh)
    const tag = `${kind} ${batch[0].address}${batch.length > 1 ? ` +${batch.length - 1}` : ''}`
    const step = (s, extra = '') => `${COMPILER} -Step ${s} -Work "${dsh}" -Staging "${OUT}" ${LIMITS}${COMPILER_DOCS}${extra}`
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
    // R233/R234: a plan may take in or rest on a part of a model outside this batch (`X#A`); the compiler resolves
    // it only after `zt-digest model-deps` has registered the producer and written the part's checked address
    // into this batch's state. Each planned model with a claim registers its references before the fill; a
    // model whose producer is not done yet is released to wait (the ledger readies it again once the producer
    // is done), and a reference the ledger refuses is recorded as that unit's content fault by model-deps itself.
    // Nothing called it before (grossman2002integration trial 8: batch 2 took `2|a#DA` from batch 1 and all
    // three of its models failed at the fill with "model … has no part DA yet").
    const deferred = new Map()
    if (claims) {
      const planned = todo.filter(m => (plans.find(p => p.model === m.address) || {}).status === 'planned' && claims[m.address])
      const registered = await Promise.all(planned.map(m => Z.runScript(`model-deps: ${m.address}`, 'Models',
        `${Z.DIGEST} model-deps -Out "${OUT}" -Model ${Z.psQuote(m.address)} -Token ${claims[m.address].token}`,
        ' (it registers the model\'s references to parts of other models; it writes no Card)')))
      registered.forEach((r, i) => {
        const at = planned[i].address
        const line = r.json
        if (line && line.ok && line.waiting) deferred.set(at, 'waiting')
        else if (line && line.ok === false) deferred.set(at, 'refused')
        else if (!line) log(`model-deps ${at}: no readable line (${String((r.run && r.run.output) || 'no output').slice(0, 200)})`)
      })
      if (deferred.size) log(`${tag}: ${[...deferred].map(([at, why]) => `${at} ${why === 'waiting' ? 'waits for the model it takes a part from' : 'has a reference the ledger refused'}`).join('; ')}.`)
    }
    // A released or refused model leaves the batch before the fill (`-Step defer`): no later step writes or
    // reports it, and its plan is set aside so the batch that claims it again plans it afresh. On
    // grossman2002integration trial 9 the fill held jobs for all four models after three were released to wait.
    const outcome = (m) => ({ model: m.address, status: deferred.get(m.address), parts: [], charts: 0, issues: [deferred.get(m.address) === 'waiting' ? 'waits for the model it takes a part from' : 'a reference to another model\'s part was refused'] })
    if (deferred.size) {
      const dropped = await Z.runScript(`defer: ${tag}`, 'Models', step('defer', ` -Models "${[...deferred.keys()].join(',')}"`), ' (it takes the waiting models out of this batch; it writes no Card)')
      if (!(dropped.json && dropped.json.ok)) {
        log(`defer ${tag}: the waiting models could not leave the batch, so nothing is filled (${String((dropped.run && dropped.run.output) || 'no output').slice(0, 200)})`)
        await run('clean up', step('report'), [], DSH_EFFORT, { cleanup: true })
        return batch.map(m => skipped.has(m.address) ? { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
          : deferred.has(m.address) ? outcome(m) : { model: m.address, status: 'failed', parts: [], charts: 0, issues: ['the batch could not set aside its waiting models before the fill'] })
      }
      if (!todo.some(m => !deferred.has(m.address) && (plans.find(p => p.model === m.address) || {}).status === 'planned')) {
        await run('clean up', step('report'), [], DSH_EFFORT, { cleanup: true })
        return batch.map(m => skipped.has(m.address) ? { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
          : deferred.has(m.address) ? outcome(m) : { model: m.address, status: (plans.find(p => p.model === m.address) || {}).status || 'failed', parts: [], charts: 0, issues: [] })
      }
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
    // 6. Close: the build faults the compiler still sees (closure, forward, reach-in, unanchored) go back
    // once, as 修正 notes, to the conversations that wrote those Cards; model-finish refuses a model that
    // keeps any (grossman2002integration trial 6: all four models were refused, and a concept model has no
    // review round that could have caught them).
    // Close runs again while its last round sent notes and faults remain, at most CLOSE_ROUNDS rounds: a writer
    // can answer a note and leave the fault in place (trials 17-19: 6|a kept a rests-on sentence to another model,
    // 7|a a malformed marker, and model-finish refused both after the one close round).
    const CLOSE_ROUNDS = 3
    for (let round = 0; round < CLOSE_ROUNDS; round++) {
      const closed = await run(round ? `close ${round + 1}` : 'close', step('close'), FILL_SKILLS, DERIVE_EFFORT, { continues: true })
      const closeLine = zdLastJson(closed && closed.output)
      if (!closeLine || !closeLine.faults) break
      log(`close ${round + 1} ${tag}: ${closeLine.faults} build fault(s) sent back in ${(closeLine.jobs || []).length} follow-up(s)${(closeLine.unowned || []).length ? `, ${closeLine.unowned.length} in Cards no job of this batch wrote` : ''}`)
      if (!(closeLine.jobs || []).length) break
    }
    // 7. Report (script) and clean-up.
    const reported = zdLastJson((await run('report', step('report'), [], DSH_EFFORT, { cleanup: true }) || {}).output)
    const reports = (reported && reported.models) || []
    return batch.map(m => {
      const r = reports.find(x => x.model === m.address)
      const pl = plans.find(x => x.model === m.address) || {}
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
  const elaborateAll = async (models, label, claims = null) => {
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
    const results = (await parallel(batches.map(b => () => compileBatch(b.models, b.kind, claims).catch(error => { log(`batch ${b.models[0].address}: ${error}`); return null })))).flatMap((r, i) => r || batches[i].models.map(() => null))
    const ordered = batches.flatMap(b => b.models)
    return results.map((r, i) => r || { model: ordered[i].address, status: 'failed', parts: [], charts: 0, issues: ['no report from its batch'] })
  }
  const kindTally = (reports) => {
    const tally = {}
    for (const r of reports) for (const [kind, n] of Object.entries(r.kinds || {})) tally[kind] = (tally[kind] || 0) + n
    return Object.entries(tally).sort((a, b) => b[1] - a[1]).map(([kind, n]) => `${kind} ${n}`).join(', ') || 'no kinds reported'
  }
  // The compiler's report line gives counts (parts, core, moved); an older line gave lists. Either reads.
  const nOf = (v) => Array.isArray(v) ? v.length : (Number(v) || 0)
  const coreOf = (r) => r.core != null ? (Number(r.core) || 0) : Array.isArray(r.parts) ? r.parts.filter(pp => pp && pp.emphasis === 'core').length : 0
  const logModels = (reports) => {
    const done = reports.filter(r => r.status === 'done')
    const sum = (field) => done.reduce((s, r) => s + (r[field] || 0), 0)
    const deriv = (field) => done.reduce((s, r) => s + ((r.derivations && r.derivations[field]) || 0), 0)
    log(`Build: ${deriv('total')} Derivation parts in ${deriv('chains')} chains of two or more (longest ${done.reduce((m, r) => Math.max(m, (r.derivations && r.derivations.longest) || 0), 0)}), ${sum('split')} parts split by review or for the Card size, ${done.reduce((s, r) => s + nOf(r.moved), 0)} Cards moved into ranges, ${sum('shared')} shared parts taken in, ${sum('tolerated')} tolerated premises, ${sum('style')} style faults left, ${done.reduce((s, r) => s + ((r.todo || []).length), 0)} parts still 待写; ${dshJobs} DeepSeek jobs.`)
    log(`Levels: ${done.filter(r => r.level === 'concept').length} concept models (level 1), ${done.filter(r => r.level === 'formal').length} formal models (level 2), ${done.filter(r => !r.level).length} without a level.`)
    log(`Models: ${done.length} structured (${done.reduce((s, r) => s + nOf(r.parts), 0)} parts, ${done.reduce((s, r) => s + coreOf(r), 0)} core, ${sum('relations')} relations (${kindTally(done)}), ${sum('neighbours')} neighbours, ${sum('loose')} loose, ${sum('charts')} charts), ${reports.filter(r => r.status === 'skipped').length} skipped, failed ${reports.filter(r => r.status === 'failed').map(r => r.model).join(', ') || 'none'}; open issues ${reports.reduce((s, r) => s + nOf(r.issues), 0)}.`)
  }
  // R323: the model stage no longer deletes conversations or folders itself. Every batch conversation
  // was recorded when the provider returned its id, so the run's cleanup stage deletes exactly those
  // ids after the owning work settles; a batch folder is disposable scratch only when its accepted
  // plan is retained first (R328/R329). This resolves that the sweep owes nothing.
  const modelCleanup = async () => {
    if (!managerFolders.length) return null
    log(`Model clean-up: ${managerFolders.length} batch folders stay until the run-level cleanup settles their recorded conversations.`)
    return { late_deleted: 0, folders: managerFolders.length, deferred: true }
  }


  return { Z, listModels, elaborateAll, logModels, modelCleanup, dshJobs: () => dshJobs }
}

defineModule({
  id: 'zt-model-lib', version: '8', library: true, requires: ['zt-digest-lib'],
  summary: 'Library: the model list and the model compiler (brief, architect plan, fill, derive, review, split) for the model layer.',
})
// ==== end module zt-model-lib ====
