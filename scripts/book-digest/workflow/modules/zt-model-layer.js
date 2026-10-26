// ==== module zt-model-layer v7 ====
// The model layer: the ledger's ready `models/gNNN-mIIII@1` work, released as the digest groups settle. Each
// claimed block of Model units is built by the existing model compiler chain (one Sonnet architect per batch,
// DeepSeek writers, the late clean-up sweep) and every unit is then recorded through `model-finish`, which
// re-derives the build from the compiler's own state and the authored Cards. A model the compiler could not
// finish stays open with its own fault, so the next attempt works only that unit (R235/R258). The layer gate
// is the stage's result.
// 6: the built units of a block are recorded through `model-finish` in ONE command (`zt-digest finish-batch`)
// instead of one relay per model (docs/digest-claude-token-proposal.md P1).
// 7 (#177, docs/workflow-module-structure-proposal.md): the layer is a straight line of model-steps.ps1 steps
// run by DeepSeek drive jobs. `start` releases a stopped run's claims, lists the Model Cards and claims a block
// of up to eight units; the block's models are built by brief, fill, derive, review, split and close steps with
// their content jobs; `finish` records the block's checked completions and reads the gate. No Claude agent
// relays a command and no JavaScript retries a step: a failed step stops the stage with its reason, per-unit
// outcomes are ledger data.
defineModule({
  id: 'zt-model-layer', version: '7', requires: ['zt-model-lib'],
  summary: 'ZT digest model layer: the ledger\'s ready Model units built by the model compiler chain and recorded by model-finish, as model-steps.ps1 steps run by DeepSeek drive jobs.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: "the run's mode ({{mode}})" },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
  },
  run: async (ctx, p) => {
    const M = zmlMake(ctx, p)
    const Z = M.Z
    // A block of up to eight Model units (the protocol's eight per manager): with up to eight formal models per
    // architect batch, a book's models are planned together, their references to each other resolve in-batch,
    // and no second block waits for the first (grossman2002integration trials 16-17: two blocks of four).
    const TAKE = 8
    const ROUNDS = Z.MAX_ATTEMPTS
    const deepseek = { deleted: 0, not_deleted: [] }
    const dispositions = []
    const addressOf = (unit) => (unit && unit.model) || ''
    let claimed = 0
    let idle = 0
    let gate = null
    let started = await M.start({ block: 1, take: TAKE, owner: 'models block 1' })
    if (!started.ok) {
      // On an existing digest the listing is the stage's whole input; after a digest the run goes on.
      if (p.mode === 'models') throw new Error(`model listing failed: ${started.error}`)
      log(`Models: listing failed (${started.error}); no model is built.`)
      return { status: 'failed', models: [], model_cleanup: null, error: `listing failed: ${started.error}` }
    }
    log(`${(started.models || []).length} Model Cards in the digest of ${Z.KEY}.`)
    for (let block = 1; block <= 200 && idle < ROUNDS; block++) {
      if (block > 1) {
        started = await M.start({ block, take: TAKE, owner: `models block ${block}` })
        if (!started.ok) {
          const cleanup = await M.modelCleanup()
          return { status: 'failed', dispositions, gate: started.gate || gate, error: started.error, model_cleanup: cleanup }
        }
      }
      gate = started.gate || gate
      const units = started.units || []
      if (!units.length) {
        idle += 1
        if (!gate || gate.complete === true) break
        if (!(gate.counts && Number(gate.counts.open || 0))) break
        continue
      }
      idle = 0
      claimed += units.length
      const listed = started.models || []
      log(`Models block ${block}: ${units.length} unit(s) claimed; ${listed.length} model(s) in the listing.`)
      const built = await M.elaborate(listed, units, 'models', { deepseek })
      M.logModels(built)
      // A model the compiler released to wait (or refused by model-deps) is not finished here: the ledger
      // holds it registered or failed, and it is claimed again once its producer is done.
      const settled = []
      const open = []
      for (const unit of units) {
        const at = addressOf(unit)
        const state = (built.find((b) => b && b.model === at) || {}).status
        if (state === 'waiting' || state === 'refused') {
          dispositions.push({ unit: unit.id, status: state === 'waiting' ? 'deferred' : 'failed', error: state })
          continue
        }
        if (at && !listed.some((m) => m.address === at)) {
          // The claim names a model the current listing does not hold; it stays open.
          open.push(unit.id)
          continue
        }
        settled.push(unit)
      }
      if (open.length) log(`Models: ${open.length} claimed Model Card(s) are not in the current listing; they stay open (${open.join(', ')}).`)
      const finished = await M.finish(settled, { deepseek, gate })
      if (!finished.ok) {
        const cleanup = await M.modelCleanup()
        return { status: 'failed', dispositions, gate: finished.gate || gate, error: finished.error, model_cleanup: cleanup }
      }
      for (const r of finished.results) {
        dispositions.push(r.status === 'done' || r.status === 'partial'
          ? { unit: r.unit, status: r.status, result: { unit: r.unit, state: 'done', disposition: r.disposition || null } }
          : { unit: r.unit, status: 'failed', error: r.cause || r.code || r.status })
      }
      gate = finished.gate || gate
      log(`Models block ${block}: ${finished.done.length} finished, ${finished.failed.length} not recorded.`)
    }
    const final = gate || { complete: false, counts: {} }
    const failed = dispositions.filter((d) => d.status === 'failed').length
    const unresolved = Number((final.counts && final.counts.open) || 0)
    const status = final.complete === true ? 'complete' : failed && !unresolved ? 'partial' : 'active'
    const modelCleanup = await M.modelCleanup()
    log(`Models: ${status} — ${dispositions.filter((d) => d.status === 'done').length} models checked${final.counts ? `, ${unresolved} open` : ''} (${claimed} unit(s) claimed, ${dispositions.filter((d) => d.status === 'deferred').length} deferred).`)
    return { status, dispositions, gate: final, model_cleanup: modelCleanup }
  },
})
// ==== end module zt-model-layer ====
