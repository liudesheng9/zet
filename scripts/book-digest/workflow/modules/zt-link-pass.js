// ==== module zt-link-pass v1 ====
// The cross-part link pass: each Haiku manager starts up to linkPerManager DeepSeek runs at once (the profile's
// link prefix plus the group's link file); each run appends reasoned Links to the Cards its group owns.
defineModule({
  id: 'zt-link-pass', version: '1', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the cross-part link runs on the pending groups.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile)' },
    groups: { type: 'number', required: true, description: 'the group count from the prepare stage' },
    pending: { type: 'array', required: true, description: 'the pending group numbers from the link preparation' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const pending = Z.groupsOf(p.groups).filter(g => p.pending.includes(g.n))
    if (!pending.length) return { reports: [] }
    const { reports } = await Z.runStage('link', Z.toBlocks(pending, Z.LINK_PER_MANAGER))
    return { reports }
  },
})
// ==== end module zt-link-pass ====
