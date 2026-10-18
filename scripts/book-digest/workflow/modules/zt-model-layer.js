// ==== module zt-model-layer v2 ====
// The model layer: the ledger's ready `models/gNNN-mIIII@1` work, released as the digest groups settle. Each
// claimed block of Model units is built by the existing model compiler chain (one Sonnet architect per batch,
// DeepSeek writers, the late clean-up sweep) and every unit is then recorded through `model-finish`, which
// re-derives the build from the compiler's own state and the authored Cards. A model the compiler could not
// finish stays open with its own fault, so the next attempt works only that unit (R235/R258). The layer gate
// is the stage's result.
defineModule({
  id: 'zt-model-layer', version: '2', requires: ['zt-model-lib'],
  summary: 'ZT digest model layer: the ledger\'s ready Model units built by the model compiler chain and recorded by model-finish.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
  },
  run: async (ctx, p) => {
    const M = zmlMake(ctx, p)
    const Z = M.Z
    const listed = await M.listModels(`-All -CitationKey ${Z.KEY}`, 'find models: whole book')
    if (!listed || !listed.ok) {
      // On an existing digest the listing is the stage's whole input; after a digest the run goes on.
      if (p.mode === 'models') throw new Error(`model listing failed: ${listed && listed.error}`)
      log(`Models: listing failed (${listed && listed.error}); no model is built.`)
      return { status: 'failed', models: [], model_cleanup: null, error: `listing failed: ${listed && listed.error}` }
    }
    log(`${listed.models.length} Model Cards in the digest of ${Z.KEY}.`)
    const result = await Z.layer('models', {
      layer: 'models', take: 4, rounds: Z.MAX_ATTEMPTS, phase: 'Models',
      work: async (units) => {
        const wanted = units.map(u => (u.inputs && (u.inputs.header || u.inputs.model)) || '').filter(Boolean)
        const models = listed.models.filter(m => wanted.includes(m.address))
        if (models.length < wanted.length) {
          log(`Models: ${wanted.length - models.length} claimed Model Cards are not in the current listing; they stay open.`)
        }
        const built = models.length ? await M.elaborateAll(models, `models ${units.map(u => u.id).join(', ')}`) : []
        M.logModels(built)
        const out = []
        for (const unit of units) {
          const at = (unit.inputs && (unit.inputs.header || unit.inputs.model)) || ''
          out.push(await Z.finishOrFault(`model ${at}`, unit.id, unit.token, unit.inputs, { model: at }))
        }
        return out
      },
    })
    const modelCleanup = await M.modelCleanup()
    log(`Models: ${result.status} — ${result.dispositions.filter(d => d.status === 'done').length} models checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    return { status: result.status, dispositions: result.dispositions, gate: result.gate, model_cleanup: modelCleanup }
  },
})
// ==== end module zt-model-layer ====
