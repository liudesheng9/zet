// ==== module zt-digest-blocks v3 ====
// The Digest stage (mode full). One Haiku manager per block of perManager groups drives the DeepSeek digest runs
// (the profile's digest prefix plus the group file). The group work is content only: the cheap per-group deterministic
// check is the group completion's own offline check (R216, #70), and the one whole-book card cycle runs as its own
// stage after this one (R296) instead of a cycle per block. Model Cards, Event Cards, and other layers are a book
// type's own stages after this one (docs/zt-digest-framework.md). Fails the stage when no group is done (a host
// fault: zt or DeepSeek Harness down).
defineModule({
  id: 'zt-digest-blocks', version: '3', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the digest blocks, with the whole-book card cycle left to its own stage (mode full).',
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
    log(`${p.groups} groups in ${Z.KEY}; ${blocks.length} Haiku managers, up to ${Z.PER_MANAGER} DeepSeek runs each.`)
    const { reports } = await Z.runStage('digest', blocks)
    if (!reports.some(r => r.status === 'done')) {
      const why = [...new Set(reports.map(r => r.errors).filter(Boolean))].slice(0, 3).join(' | ')
      log(`Digest: every group failed; the run stops before the card cycle and the link pass. ${why}`)
      throw new Error(`every digest group failed: ${why}`)
    }
    return { reports, managers: blocks.length }
  },
})
// ==== end module zt-digest-blocks ====
