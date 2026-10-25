// ==== module zt-term-link v2 ====
// S24 (#167), S26 (#169): the deterministic Term-link stage. The pass itself (claim the pass's
// ledger unit, freeze the plan, apply it under the claim, complete the unit only from the apply
// report) is the shared `zt-term-link-lib`, because the finite finalization runs the same pass for
// its own correction round before the final read-only audit. This stage is the run's whole-book
// pass after every Card writer and the Bridge; a blocked pass is reported and audit/report still
// run, so coverage debt is visible instead of a linked book that was never linked (R0428).
defineModule({
  id: 'zt-term-link', version: '2', requires: ['zt-term-link-lib'],
  summary: 'Term link: claim the run\'s Term-link unit, freeze the deterministic plan, apply it under the claim and complete the unit from the apply report; a blocked or failed pass stays incomplete and is reported, never silently linked.',
  params: {
    out: { type: 'string', required: true, description: 'the prepared run\'s staging folder (the compiler\'s -Staging)' },
    key: { type: 'string', required: true, description: 'the book\'s Citation key' },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    occurrences: { type: 'string', enum: ['first', 'all'], default: 'first', description: 'the occurrence policy: first or all (args.termLinkOccurrences)' },
    cards: { type: 'array', default: [], description: 'a finalization round\'s exact changed Cards (empty for the whole-book pass)' },
    round: { type: 'number', description: 'the finalization correction round the -Cards scope serves, or the whole-book round when no Cards are given' },
    tool: { type: 'string', default: 'zt-digest', description: 'the zt-digest command, or the path of a zt-digest binary' },
    relay: { type: 'string', default: 'relay', role: true, description: 'the declared role that relays the compiler commands (the zt-relay agent type)' },
  },
  run: async (ctx, p) => ztTermLinkPass(ctx, p),
})
// ==== end module zt-term-link ====
