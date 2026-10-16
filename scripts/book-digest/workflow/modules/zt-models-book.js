// ==== module zt-models-book v1 ====
// Mode "models": the model compiler on every Model Card of an existing digest (or those in args.only; with
// args.rebuild, also on models that already have parts), then the late clean-up of the model batches.
defineModule({
  id: 'zt-models-book', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the model compiler on the whole book (mode models).',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const listed = await Z.listModels(`-All -CitationKey ${Z.KEY}`, 'find models: whole book')
    if (!listed || !listed.ok) throw new Error(`model listing failed: ${listed && listed.error}`)
    log(`${listed.models.length} Model Cards in the digest of ${Z.KEY}.`)
    const models = await Z.elaborateAll(listed.models, 'whole book')
    Z.logModels(models)
    const modelCleanup = await Z.modelCleanup()
    return { models, model_cleanup: modelCleanup }
  },
})
// ==== end module zt-models-book ====
