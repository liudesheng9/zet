// ==== module zt-cards-layer v4 ====
// The Cards layer of a managed run: one deterministic offline check per verified group (`cards/gNNN@1`).
// `group-finish` registers the check; `layer-steps.ps1 -Step start` claims a block of the ready checks and
// records each one through `zt-digest finish-batch -Layer cards`, which re-reads the group's owned Cards, runs
// the shared checker in-process and hands every hard fault to a registered repair consumer. The step is one
// script call run by a DeepSeek drive job: no Claude agent claims or finishes a unit, and the layer stops
// when a claim finds no ready check. It never writes a Card and never starts a content job for a check that is
// not ready (R252/R254/R257).
// 2: the claim and the checked finishes of a block run as ONE command (`zt-digest finish-batch -Layer cards`), so a
// block costs one command instead of a claim and one command per group (docs/digest-claude-token-proposal.md P1/P2).
// 3 (#175): that command is the `start` step of `layer-steps.ps1`, run by a DeepSeek drive job under a Haiku manager.
// 4 (#180): a step answers counts (`done`, `failed`), never the block's per-unit results.
defineModule({
  id: 'zt-cards-layer', version: '4', requires: ['zt-digest-lib'],
  summary: 'ZT digest: the per-group offline Card checks, claimed from the ledger ready work and recorded by cards-finish.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    const result = await Z.layerRun('cards', {
      layer: 'cards', take: 4, rounds: Z.MAX_ATTEMPTS, phase: 'Cards', jobs: false,
    })
    const verified = Number(result.verified || 0)
    const failed = Number(result.failed || 0)
    if (result.status === 'failed') return { status: 'failed', error: result.error, verified, failed, gate: result.gate }
    log(`Cards: ${result.status} — ${verified} verified, ${failed} failed${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    return { status: result.status, verified, failed, gate: result.gate }
  },
})
// ==== end module zt-cards-layer ====
