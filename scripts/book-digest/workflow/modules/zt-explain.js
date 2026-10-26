// ==== module zt-explain v8 ====
// Explanations of finished Derivation parts (R219/R301/R302): the ledger's ready `explain/<model-key>@1` work,
// which a checked Model completion registers. The layer driver runs the stage as script steps: `start` claims a
// block of ready explanation units and runs the explanation plan (one job per chain), the block's jobs run in
// Haiku managers, `apply` writes 思路/说明/解读 through `explain-apply` (which refuses a changed 起点/step/结果 and
// lists one fix job per refused part, run in that part's own conversation), and `finish` reports and records
// every unit through `explain-finish`, which re-reads the parts and refuses a part that still lacks text. Every
// step is one `layer-steps.ps1` call run by a DeepSeek drive job: no Claude agent relays a command and no Claude
// agent is spent per model or per finish. The block's conversations stay open across its two content rounds and
// are deleted by the run cleanup. Runs in mode explain and after the model layer in mode full.
// 6: the chain ends with the block's checked finishes in one command (`zt-digest finish-batch`), run by the chain's own
// manager; a unit its line does not answer is finished by one relay, then on its own (docs/digest-claude-token-proposal.md P1/P2).
// 7 (S8): the claimed models, whatever args.only says (the two branches were identical).
// 8 (#175): the block is three script steps and two content rounds, run by DeepSeek drive jobs under Haiku managers.
defineModule({
  id: 'zt-explain', version: '8', requires: ['zt-model-lib'],
  summary: 'ZT digest: the ledger\'s ready explanations of finished Derivation parts, written in two DeepSeek content rounds and recorded by explain-finish.',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
  },
  run: async (ctx, p) => {
    const M = zmlMake(ctx, p)
    const Z = M.Z
    const A = Z.A
    const work = `${Z.OUT}\\explain`
    const tally = { parts: 0, refused: 0 }
    const result = await Z.layerRun('explain', {
      // The block's conversations stay open for the second content round; the run cleanup deletes them.
      layer: 'explain', take: 4, rounds: Z.MAX_ATTEMPTS, phase: 'Models', jobs: true, keep: true,
      dsh: work, writes: `${work}\\out`, skills: Z.FILL_SKILLS, effort: Z.DSH_EFFORT, what: 'explanation writers',
      limits: Z.LIMITS, maxParts: A.explainMax || 0, apply: true,
      onStart: (started) => { tally.parts += Number((started.plan && started.plan.parts) || 0) },
      onApply: (applied) => { tally.refused += Number(applied.refused || 0) },
      onFinish: (finished) => {
        const report = (finished.mid || []).find((m) => m.step === 'report') || {}
        log(`Explain: ${report.models || 0} models reported.`)
      },
    })
    log(`Explain: ${tally.parts} Derivation part(s) planned; ${tally.refused} refused; ${result.status} — ${result.dispositions.filter(d => d.status === 'done').length} models checked${result.gate && result.gate.counts ? `, ${result.gate.counts.open || 0} open` : ''}.`)
    if (result.status === 'failed') return { status: 'failed', error: result.error, dispositions: result.dispositions, gate: result.gate }
    return { status: result.status, dispositions: result.dispositions, gate: result.gate }
  },
})
// ==== end module zt-explain ====
