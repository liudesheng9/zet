// ==== module zt-term-link v5 ====
// S24 (#167); v3 (#172); v4 (#175); v5 (#180): the run's whole-book Term-link pass as one DeepSeek drive job that a
// Haiku manager starts through the shared zt-workflow-lib helper `ztDrive`: `term-steps.ps1 -Step link` plans, applies
// under the claim (or, when the unit is blocked by an incomplete Term-card book, links the published prefix and
// leaves the unit open), and plans again so the report shows the second pass's changes (0 expected). The
// finalization's own correction round still uses the shared zt-term-link-lib.
// v5 (#180): the step answers one compact line whose fields are top-level counts (`cards_changed`,
// `links_added`, `second_cards_changed`, `second_cards_refused`); the module reads those and builds its report.
defineModule({
  id: 'zt-term-link', version: '5', requires: ['@dsmp', 'zt-workflow-lib'],
  summary: 'Term link: one DeepSeek drive job runs the deterministic Term-link pass (plan, apply, complete, second plan) and reports its result.',
  params: {
    out: { type: 'string', required: true, description: 'the prepared run\'s staging folder' },
    key: { type: 'string', required: true, description: 'the book\'s Citation key' },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    occurrences: { type: 'string', enum: ['first', 'all'], default: 'first', description: 'the occurrence policy (args.termLinkOccurrences)' },
    script: { type: 'string', default: 'E:\\zet\\scripts\\book-digest\\term-steps.ps1', description: 'the Term step script the drive job runs' },
  },
  run: async (ctx, p) => {
    // The kernel passes only the params a blueprint gives; the script path has one default.
    const DEFAULT_SCRIPT = 'E:\\zet\\scripts\\book-digest\\term-steps.ps1'
    const OUT = String(p.out).replace(/[\\/]+$/, '')
    const tail = (text) => String(text || '').trim().slice(0, 400)
    const n = (v) => Number(v || 0)
    const deepseek = { deleted: 0, not_deleted: [] }
    const command = `pwsh -NoProfile -File "${p.script || DEFAULT_SCRIPT}" -Step link -Out "${OUT}" -Occurrences ${p.occurrences}`
    const json = await ztDrive(ctx, { label: `${ctx.stage}: drive link`, step: 'link', command, out: OUT, cwd: `${OUT}\\term-link`, phase: ctx.phase, ledger: 'zt-digest ledger', deepseek })
    if (json.ok !== true) return { status: 'failed', book: p.key, error: tail(json.error || 'the link step failed'), deepseek }
    const counts = { cards_changed: n(json.cards_changed), links_added: n(json.links_added) }
    const second = { cards_changed: n(json.second_cards_changed), cards_refused: n(json.second_cards_refused) }
    log(`Term link: ${json.status}; ${counts.cards_changed} Card(s) changed; second plan changes ${second.cards_changed} Card(s).`)
    return { status: json.status, book: p.key, unit: json.unit || null, counts, second, reason: json.reason || null, deepseek }
  },
})
