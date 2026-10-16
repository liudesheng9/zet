// ==== module zt-digest-blocks v1 ====
// The Digest stage (mode full). One Haiku manager per block of perManager groups drives the DeepSeek digest runs
// (the profile's digest prefix plus the group file). As each block finishes, its Idea Cards and anchors go to
// the card layer cycle (a pipeline, not a barrier). Model Cards, Event Cards, and other layers are a book type's
// own stages after this one (docs/zt-digest-framework.md). Fails the stage when no group is done (a host fault:
// zt or DeepSeek Harness down).
defineModule({
  id: 'zt-digest-blocks', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the digest blocks, each block\'s card cycle as soon as it finishes (mode full).',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile): the digest prefix' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const groups = Z.groupsOf(p.groups)
    const blocks = Z.toBlocks(groups)
    log(`${p.groups} groups in ${Z.KEY}; ${blocks.length} Haiku managers, up to ${Z.PER_MANAGER} DeepSeek runs each${Z.CARD_CHECKS ? '; each block\'s Cards go to the card cycle as soon as the block finishes' : ''}.`)
    const cardReports = []
    const afterBlock = Z.CARD_CHECKS ? async (block) => {
      cardReports.push(await Z.cardBlock(block).catch(error => ({ status: 'failed', error: String(error) })))
      return []
    } : null
    const { reports } = await Z.runStage('digest', blocks, afterBlock)
    if (!reports.some(r => r.status === 'done')) {
      const why = [...new Set(reports.map(r => r.errors).filter(Boolean))].slice(0, 3).join(' | ')
      log(`Digest: every group failed; the run stops before the link pass. ${why}`)
      throw new Error(`every digest group failed: ${why}`)
    }
    return { reports, cards: cardReports, managers: blocks.length }
  },
})
// ==== end module zt-digest-blocks ====
