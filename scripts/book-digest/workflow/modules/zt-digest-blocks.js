// ==== module zt-digest-blocks v1 ====
// The Digest stage (mode full). One Haiku manager per block of perManager groups drives the DeepSeek digest runs
// (the profile's digest prefix plus the group file). As each block finishes, its Model Cards go to the model
// compiler and its Idea Cards, Event Cards, and anchors go to the card layer cycle, side by side (a pipeline, not
// a barrier). Then a whole-book sweep compiles any model a block missed, and a Haiku agent deletes what a failed
// model batch left. Fails the stage when no group is done (a host fault: zt or DeepSeek Harness down).
defineModule({
  id: 'zt-digest-blocks', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: digest blocks with per-block models and card cycles, then the model sweep (mode full).',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile): prefixes, compiler docs, architect supplement, skipped forms' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const groups = Z.groupsOf(p.groups)
    const blocks = Z.toBlocks(groups)
    log(`${p.groups} groups in ${Z.KEY}; ${blocks.length} Haiku managers, up to ${Z.PER_MANAGER} DeepSeek runs each${Z.MODELS ? '; each block\'s Model Cards go to the model compiler as soon as the block finishes' : ''}.`)
    const cardReports = []
    // A block's models and its card cycle run side by side: the cycle never writes a Model Card or part.
    const afterBlock = (Z.MODELS || Z.CARD_CHECKS) ? async (block, managed) => {
      const [models, cards] = await Promise.all([
        Z.MODELS ? Z.elaborateBlock(block, managed) : [],
        Z.CARD_CHECKS ? Z.cardBlock(block).catch(error => ({ status: 'failed', error: String(error) })) : null,
      ])
      if (cards) cardReports.push(cards)
      return models
    } : null
    const { reports, extras } = await Z.runStage('digest', blocks, afterBlock)
    let modelReports = extras
    if (!reports.some(r => r.status === 'done')) {
      const why = [...new Set(reports.map(r => r.errors).filter(Boolean))].slice(0, 3).join(' | ')
      log(`Digest: every group failed; the run stops before the link pass. ${why}`)
      throw new Error(`every digest group failed: ${why}`)
    }
    // Sweep the whole book: a block whose listing failed or missed a Model Card would leave it unelaborated.
    if (Z.MODELS) {
      const listed = await Z.listModels(`-All -CitationKey ${Z.KEY}`, 'find models: sweep')
      if (!listed || !listed.ok) log(`Model sweep: listing failed (${listed && listed.error})`)
      else {
        const covered = new Set(modelReports.filter(r => r && r.status === 'done').map(r => r.model))
        const missed = listed.models.filter(m => !covered.has(m.address))
        if (missed.length) modelReports = modelReports.concat(await Z.elaborateAll(missed, 'sweep'))
      }
      Z.logModels(modelReports)
    }
    const modelCleanup = Z.MODELS ? await Z.modelCleanup() : null
    return { reports, models: modelReports, cards: cardReports, model_cleanup: modelCleanup, managers: blocks.length }
  },
})
// ==== end module zt-digest-blocks ====
