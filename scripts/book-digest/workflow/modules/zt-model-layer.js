// ==== module zt-model-layer v1 ====
// The model layer: after the digest (mode full) or on an existing digest (mode models), the model compiler on
// every Model Card of the book that still needs its parts (or those in args.only; with args.rebuild, all), then
// the late clean-up of the model batches. The book type's profile decides the architect's supplement, the
// compiler's extra instruction files, and the Model Card forms left out (math: Theory Cards).
defineModule({
  id: 'zt-model-layer', version: '1', requires: ['zt-model-lib'],
  summary: 'ZT digest model layer: the model compiler on the book\'s Model Cards (after the digest, or mode models).',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
  },
  run: async (ctx, p) => {
    const M = zmlMake(ctx, p)
    const listed = await M.listModels(`-All -CitationKey ${M.Z.KEY}`, 'find models: whole book')
    if (!listed || !listed.ok) {
      // On an existing digest the listing is the stage's whole input; after a digest the run goes on.
      if (p.mode === 'models') throw new Error(`model listing failed: ${listed && listed.error}`)
      log(`Models: listing failed (${listed && listed.error}); no model is built.`)
      return { models: [], model_cleanup: null, error: `listing failed: ${listed && listed.error}` }
    }
    log(`${listed.models.length} Model Cards in the digest of ${M.Z.KEY}.`)
    const models = await M.elaborateAll(listed.models, 'whole book')
    M.logModels(models)
    const modelCleanup = await M.modelCleanup()
    return { models, model_cleanup: modelCleanup }
  },
})
// ==== end module zt-model-layer ====
