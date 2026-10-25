// ==== module zt-term-link-lib v2 ====
// The deterministic Term-link pass of one book (S24, #167; S26, #169), shared by the `term-link`
// stage and the finite finalization. It owns no content decision: it claims the pass's ledger
// unit, runs the compiler's frozen `plan` (which registers that unit in a prepared run), applies
// the exact plan under the claim, and completes the unit only from the compiler's own apply
// report. The whole-book pass is `termlink/book@1`; a finalization correction round is
// `termlink/final@R` with the round's changed Cards, or the whole book when the round changed the
// Term mapping (R0439), so a later correction can never be suppressed by a reusable unit
// (R0438/R0439).
//
// The ledger is the scheduler and the truth. A pass whose Term-card book unit or Card writers are
// not done is not ready: the caller gets `skipped` with the ledger's unresolved units, so audit and
// report still run and show the coverage debt, and a failed or partial publication can never be
// represented as a linked book (R0428). A diagnostic `-Cards` subset without a round owns no unit
// at all (R0429). A stopped or incomplete apply records its fault on the unit and leaves it
// incomplete; the run's report shows it, and a resume retries only that unit.
async function ztTermLinkPass(ctx, p) {
  const q = (v) => `'${String(v).replace(/'/g, "''")}'`
  const OUT = String(p.out).replace(/[\\/]+$/, '')
  const WORK = `${OUT}\\term-link`
  const exe = p.tool && p.tool !== 'zt-digest' ? `& ${q(p.tool)}` : 'zt-digest'
  const phase = ctx.phase
  const tag = p.stage || ctx.stage
  const tail = (text) => String(text || '').trim().slice(0, 400)
  const cards = Array.isArray(p.cards) ? p.cards.map(String).filter(Boolean) : []
  const round = Number.isFinite(Number(p.round)) && Number(p.round) > 0 ? Number(p.round) : null
  if (round === null && cards.length) return { status: 'skipped', reason: 'a diagnostic -Cards subset without a round owns no ledger unit; run the stage without -Cards for the book pass', book: p.key, work: WORK }
  // S26 (#169): a round without -Cards is the whole-book replanning a changed Term mapping
  // requires; every other pass names its exact Card subset (or none, for the whole book).
  const scoped = `${round !== null ? ` -Round ${round}` : ''}${cards.length ? ` -Cards ${q(cards.join(','))}` : ''}`
  const report = { status: 'done', book: p.key, work: WORK, unit: null, pass: round === null ? 'book' : 'final', round, scope: cards.length ? 'cards' : 'book', cards: cards.length, plan: null, counts: null, no_op: false, unresolved: [], staging: WORK }

  // One compiler command through the one relay; its last JSON line is the step's answer. `always`
  // reads the line of a step that exits non-zero too: the archive's typed stop is its last line.
  const command = async (label, args, always = false) => {
    const got = await ztRelay(ctx, { command: `${exe} term-link ${args}`, label: `${tag}: ${label}`, phase, role: p.relay || 'relay', run: `the digest run the user started for "${OUT}"`, note: ' (it writes only the staging folder)', always })
    return got
  }
  const ledger = async (label, args, always = false) => {
    const got = await ztRelay(ctx, { command: `${exe} ledger ${args}`, label: `${tag}: ${label}`, phase, role: p.relay || 'relay', run: `the digest run the user started for "${OUT}"`, note: ' (it reads or records the run ledger only)', always, lockStops: false })
    return { ok: !!(got.run && got.run.ok), line: got.json, output: (got.run && got.run.output) || '' }
  }
  // Record one unit's content fault and leave it incomplete: the ledger's own classifier reads the
  // code from the cause (S5), so a card_check refusal keeps its code.
  const fault = async (label, unit, code, cause) => {
    const document = { schema_version: 1, unit: unit.id, input_hash: unit.input_hash, cause: tail(cause) }
    if (code) document.code = code
    const got = await ledger(label, `fail -Out "${OUT}" -Unit "${unit.id}" -Token ${unit.token} -Fault ${q(JSON.stringify(document))}`)
    if (!got.ok) log(`Term link: the ledger refused the fault of ${unit.id}: ${tail(got.output)}`)
    return got
  }

  let unit = null
  try {
    // The plan comes first: in a prepared run it registers this pass's unit (`termlink/book@1`, or
    // `termlink/final@R` for a finalization round). The claim then reads the unit the dependency
    // ledger has made ready; a blocked pass has no ready unit and is reported, never forced.
    const planned = await command('plan', `-Step plan -Staging ${q(OUT)} -Occurrences ${p.occurrences}${scoped}`)
    const plan = planned.json
    if (!plan || plan.ok !== true) {
      report.status = 'failed'
      report.error = `term-link plan: ${tail((planned.run && planned.run.output) || 'no output')}`
      return report
    }
    report.plan = { revision: plan.plan_revision, hash: plan.plan_hash, cards_changed: plan.cards_changed, cards_refused: plan.cards_refused, missing_heads: plan.missing_heads, complete: plan.complete }
    if (plan.unit) report.unit = plan.unit

    const claimed = await ledger('claim', `next -Out "${OUT}" -Layer termlink -Take 1 -Owner "term-link" -Wait 60 -Lease 3600`)
    if (!claimed.ok || !claimed.line || !Array.isArray(claimed.line.units)) {
      report.status = 'failed'
      report.error = `the Term-link claim returned no units: ${tail(claimed.output)}`
      return report
    }
    unit = claimed.line.units.find((u) => u && typeof u.id === 'string' && u.id.startsWith('termlink/')) || null
    if (!unit) {
      // No ready unit: the pass is already complete, or the work it depends on is not.
      const gate = await ledger('gate', `gate -Out "${OUT}" -Layer termlink -Brief`, true)
      const view = gate.line || {}
      report.unresolved = Array.isArray(view.unresolved) ? view.unresolved : []
      const counts = view.counts || {}
      if (view.complete === true) {
        report.status = 'skipped'
        report.reason = 'the Term-link pass is already complete'
        log('Term link: the run\'s Term-link pass is already complete; nothing to do.')
        return report
      }
      report.status = 'skipped'
      report.reason = `the Term-link unit is not ready: ${report.unresolved.length} unresolved unit(s) (${counts.failed || 0} failed, ${counts.blocked || 0} blocked); a failed Term-card book or Card writer never becomes a linked book`
      if (round === null) {
        // v2 (S30 pilot, #172): the whole-book pass still links the Terms that are published. The stage
        // runs after every Card writer and the Term-card branch, so no writer changes a body under it.
        // It applies the frozen plan without a claim: the unit stays incomplete (R0343: never done
        // over a partial book), and a later complete book runs the claimed pass, which is idempotent.
        const applied = await command('prefix apply', `-Step apply -Staging ${q(OUT)} -Occurrences ${p.occurrences} -Apply`, true)
        const summary = applied.json && applied.json.step === 'apply' ? applied.json : null
        if (summary) {
          report.counts = summary.counts || null
          report.no_op = summary.no_op === true
          report.chunks = summary.chunks
        }
        if (!(applied.run && applied.run.ok) || !summary || summary.complete !== true) {
          const detail = applied.json && applied.json.step !== 'apply' ? applied.json : null
          report.status = 'failed'
          report.code = (summary && summary.code) || (detail && (detail.code || detail.error)) || 'incomplete'
          report.error = `the prefix Term-link pass stopped as ${report.code}; ${report.reason}`
          log(`Term link: failed — ${report.error}.`)
          return report
        }
        report.status = 'partial'
        report.prefix = true
        report.reason = `${report.reason}; the published prefix is linked (${(report.counts && report.counts.cards_changed) || 0} Card(s) changed) and the unit stays incomplete`
        log(`Term link: partial — ${report.reason}.`)
        return report
      }
      log(`Term link: skipped — ${report.reason}.`)
      return report
    }
    report.unit = unit.id
    report.token = unit.token
    log(`Term link: ${unit.id} claimed; ${p.occurrences} occurrence(s)${round !== null ? `, round ${round}` : ''}${cards.length ? `, ${cards.length} Card(s)` : ', whole book'}.`)

    const applied = await command('apply', `-Step apply -Staging ${q(OUT)} -Occurrences ${p.occurrences}${scoped} -Token ${unit.token} -Apply`, true)
    const summary = applied.json && applied.json.step === 'apply' ? applied.json : null
    const ok = !!(applied.run && applied.run.ok) && !!summary && summary.complete === true
    if (summary) {
      report.counts = summary.counts || null
      report.no_op = summary.no_op === true
      report.chunks = summary.chunks
    }
    if (!ok) {
      const detail = applied.json && applied.json.step !== 'apply' ? applied.json : null
      const stop = (summary && summary.code) || (detail && (detail.code || detail.error)) || 'incomplete'
      await fault('apply fault', unit, stop === 'card_check' ? 'card_check' : '', `the Term-link pass stopped as ${stop}: ${tail((applied.run && applied.run.output) || 'no output')}`)
      report.status = 'failed'
      report.code = stop
      report.error = `the Term-link pass stopped as ${stop}; its unit stays incomplete`
      log(`Term link: failed — ${report.error}.`)
      return report
    }

    const completed = await ledger('complete', `done -Out "${OUT}" -Unit "${unit.id}" -Token ${unit.token} -Outputs "${summary.report || `${WORK}\\apply.json`}"`)
    if (!completed.ok || !completed.line || completed.line.state !== 'done') {
      await fault('complete fault', unit, '', `the Term-link completion was refused: ${tail(completed.output)}`)
      report.status = 'failed'
      report.error = `the Term-link unit could not complete: ${tail(completed.output)}`
      return report
    }
    report.status = 'done'
    report.completion = completed.line
    report.counts = report.counts || completed.line.counts || null
    log(`Term link: ${unit.id} complete — ${(report.counts && report.counts.cards_changed) || 0} Card(s) changed, ${(report.counts && report.counts.links_added) || 0} link(s) added${report.no_op ? ', proven no-op' : ''}.`)
    return report
  } catch (e) {
    if (e && e.lock_refusal) {
      // The owner's Lock stops the pass: the unit keeps no completion and the run keeps its debt.
      if (unit) await fault('lock fault', unit, 'card_locked', e.message)
      throw e
    }
    report.status = 'failed'
    report.error = tail((e && e.message) || 'the Term-link pass failed')
    if (unit) await fault('branch fault', unit, '', report.error)
    return report
  }
}

defineModule({
  id: 'zt-term-link-lib', version: '2', library: true, requires: ['zt-workflow-lib'],
  summary: 'Library: the deterministic Term-link pass (claim the pass unit, freeze the plan, apply under the claim, complete only from the apply report) for the Term-link stage and the finite finalization.',
})
// ==== end module zt-term-link-lib ====
