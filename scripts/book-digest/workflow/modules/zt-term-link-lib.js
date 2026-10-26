// ==== module zt-term-link-lib v3 ====
// The deterministic Term-link pass of one book's finalization round (S24, #167; S26, #169), shared by the finite
// finalization. It owns no content decision: one book-steps.ps1 step (`term-link`) freezes the round's plan, claims
// the pass's ledger unit, applies the exact plan under the claim, completes the unit only from the compiler's own
// apply report and plans again, so the report shows the second pass's changes. A finalization correction round is
// `termlink/final@R` with the round's changed Cards, or the whole book when the round changed the Term mapping
// (R0439), so a later correction can never be suppressed by a reusable unit (R0438/R0439).
//
// The ledger is the scheduler and the truth. A pass whose Term-card book unit or Card writers are not done has no
// ready unit: the step answers `skipped` with the ledger's own gate, so audit and report still run and show the
// coverage debt, and a failed or partial publication can never be represented as a linked book (R0428). A stopped or
// incomplete apply records its fault on the unit inside the step and leaves it incomplete; the run's report shows it,
// and a resume retries only that unit.
//
// 3 (#176, docs/workflow-module-structure-proposal.md): the pass is one DeepSeek drive job that runs one
// book-steps.ps1 step and answers its one JSON line. The relay-era whole-book prefix path leaves with the relay: the
// whole-book pass is `zt-term-link`'s term-steps.ps1 step; this library serves the finalization round alone.
const ZT_TERM_LINK_SCRIPT = 'E:\\zet\\scripts\\book-digest\\book-steps.ps1'
async function ztTermLinkPass(ctx, p) {
  const OUT = String(p.out).replace(/[\\/]+$/, '')
  const WORK = `${OUT}\\term-link`
  const phase = ctx.phase
  const tag = p.stage || ctx.stage
  const tail = (text) => String(text || '').trim().slice(0, 400)
  const cards = Array.isArray(p.cards) ? p.cards.map(String).filter(Boolean) : []
  const round = Number.isFinite(Number(p.round)) && Number(p.round) > 0 ? Number(p.round) : null
  const report = { status: 'done', book: p.key, work: WORK, unit: null, pass: 'final', round, scope: cards.length ? 'cards' : 'book', cards: cards.length, plan: null, counts: null, no_op: false, unresolved: [], staging: WORK }
  if (round === null && cards.length) {
    report.status = 'skipped'
    report.reason = 'a diagnostic -Cards subset without a round owns no ledger unit; run the stage without -Cards for the book pass'
    return report
  }
  const options = { Round: round, Occurrences: p.occurrences || 'first' }
  if (cards.length) options.Cards = cards
  const deepseek = p.deepseek || { deleted: 0, not_deleted: [] }
  const command = `pwsh -NoProfile -File "${p.script || ZT_TERM_LINK_SCRIPT}" -Step term-link -Out "${OUT}" -Json '${JSON.stringify(options).replace(/'/g, "''")}'`
  let json
  try {
    json = await ztDrive(ctx, { label: `${tag}: term-link round`, step: 'term-link', command, out: OUT, cwd: WORK, phase, ledger: 'zt-digest ledger', deepseek })
  } catch (error) {
    report.status = 'failed'
    report.error = tail((error && error.message) || 'the Term-link pass failed')
    return report
  }
  report.plan = json.plan ? { revision: json.plan.plan_revision, hash: json.plan.plan_hash, cards_changed: json.plan.cards_changed, cards_refused: json.plan.cards_refused, missing_heads: json.plan.missing_heads, complete: json.plan.complete } : null
  report.unit = json.unit || (json.plan && json.plan.unit) || null
  report.counts = json.counts || null
  report.no_op = json.no_op === true
  report.completion = json.completion || null
  report.second = json.second || null
  if (json.status === 'done') {
    report.status = 'done'
    log(`Term link: ${report.unit || 'the round'} complete — ${(report.counts && report.counts.cards_changed) || 0} Card(s) changed, ${(report.counts && report.counts.links_added) || 0} link(s) added${report.no_op ? ', proven no-op' : ''}.`)
    return report
  }
  if (json.status === 'skipped') {
    report.status = 'skipped'
    report.reason = json.reason || 'the Term-link unit is not ready'
    log(`Term link: skipped — ${report.reason}.`)
    return report
  }
  report.status = 'failed'
  report.code = json.code || null
  report.error = tail(json.error || 'the Term-link pass did not complete')
  if (json.completion && json.completion.state) report.state = json.completion.state
  log(`Term link: failed — ${report.error}.`)
  return report
}

defineModule({
  id: 'zt-term-link-lib', version: '3', library: true, requires: ['zt-workflow-lib'],
  summary: 'Library: the finalization\'s deterministic Term-link round (one book-steps.ps1 step: plan, claim, apply, complete, second plan) as one DeepSeek drive job.',
})
// ==== end module zt-term-link-lib ====
