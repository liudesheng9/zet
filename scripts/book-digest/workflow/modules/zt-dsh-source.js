// ==== module zt-dsh-source v2 ====
// The DeepSeek source. v2 (docs/workflow-simplicity-proposal.md S1): the source is frozen at the official one (the
// user's rule, 2026-10-20: never OpenCode Go), so no Haiku probe runs and no argument chooses it. The stage reports the
// model and logs the DeepSeek efforts raised to high.
defineModule({
  id: 'zt-dsh-source', version: '2', requires: ['zt-digest-lib'],
  summary: 'ZT digest: report the frozen DeepSeek source (the official one) and its model.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const A = p.args
    const Z = zdMake(ctx, { args: A, mode: p.mode })
    if (Z.raised.length) log(`DeepSeek efforts raised to high (the user wants at least medium; V4.1 Flash has no medium): ${Z.raised.join(', ')}.`)
    const { source, model } = zdDefaultModel()
    log(`DeepSeek source: ${source} (${model}).`)
    return { source, model }
  },
})
// ==== end module zt-dsh-source ====
