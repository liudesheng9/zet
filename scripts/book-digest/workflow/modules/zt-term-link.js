// ==== module zt-term-link v1 ====
// S24 (#167): the deterministic Term-link pass of one book, after every Card writer and the Bridge
// (design 13.6-13.7, 14.1-14.2). It owns no content decision: it claims the pass's ledger unit,
// runs the compiler's frozen `plan` (which registers that unit in a prepared run), applies the exact
// plan under the claim, and completes the unit only from the compiler's own apply report. The
// whole-book pass is `termlink/book@1`; a finalization correction round is `termlink/final@R`, so a
// later correction can never be suppressed by a reusable unit (R0426/R0427).
//
// The ledger is the scheduler and the truth. A pass whose Term-card book unit or Card writers are
// not done is not ready: the module reports it `skipped` with the ledger's unresolved units, so
// audit and report still run and show the coverage debt, and a failed or partial publication can
// never be represented as a linked book (R0428). A diagnostic `-Cards` subset without a round owns
// no unit at all (R0429). A stopped or incomplete apply records its fault on the unit and leaves it
// incomplete; the run's report shows it, and a resume retries only that unit.
defineModule({
  id: 'zt-term-link', version: '1', requires: ['zt-workflow-lib'],
  summary: 'Term link: claim the run\'s Term-link unit, freeze the deterministic plan, apply it under the claim and complete the unit from the apply report; a blocked or failed pass stays incomplete and is reported, never silently linked.',
  params: {
    out: { type: 'string', required: true, description: 'the prepared run\'s staging folder (the compiler\'s -Staging)' },
    key: { type: 'string', required: true, description: 'the book\'s Citation key' },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    occurrences: { type: 'string', enum: ['first', 'all'], default: 'first', description: 'the occurrence policy: first or all (args.termLinkOccurrences)' },
    cards: { type: 'array', default: [], description: 'a finalization round\'s exact changed Cards (empty for the whole-book pass)' },
    round: { type: 'number', description: 'the finalization correction round the -Cards scope serves' },
    tool: { type: 'string', default: 'zt-digest', description: 'the zt-digest command, or the path of a zt-digest binary' },
    relay: { type: 'string', default: 'relay', role: true, description: 'the declared role that relays the compiler commands (the zt-relay agent type)' },
  },
  run: async (ctx, p) => {
    const q = (v) => `'${String(v).replace(/'/g, "''")}'`
    const OUT = String(p.out).replace(/[\\/]+$/, '')
    const WORK = `${OUT}\\term-link`
    const exe = p.tool && p.tool !== 'zt-digest' ? `& ${q(p.tool)}` : 'zt-digest'
    const phase = ctx.phase
    const tail = (text) => String(text || '').trim().slice(0, 400)
    const cards = Array.isArray(p.cards) ? p.cards.map(String).filter(Boolean) : []
    const round = Number.isFinite(Number(p.round)) && Number(p.round) > 0 ? Number(p.round) : null
    if (round !== null && !cards.length) return { status: 'failed', error: `finalization round ${round} names no changed Card`, book: p.key, work: WORK }
    if (round === null && cards.length) return { status: 'skipped', reason: 'a diagnostic -Cards subset without a round owns no ledger unit; run the stage without -Cards for the book pass', book: p.key, work: WORK }
    const scoped = cards.length ? ` -Cards ${q(cards.join(','))}${round !== null ? ` -Round ${round}` : ''}` : ''
    const report = { status: 'done', book: p.key, work: WORK, unit: null, pass: round === null ? 'book' : 'final', round, cards: cards.length, plan: null, counts: null, no_op: false, unresolved: [], staging: WORK }

    // One compiler command through the one relay; its last JSON line is the step's answer. `always`
    // reads the line of a step that exits non-zero too: the archive's typed stop is its last line.
    const command = async (label, args, always = false) => {
      const got = await ztRelay(ctx, { command: `${exe} term-link ${args}`, label: `${ctx.stage}: ${label}`, phase, role: p.relay || 'relay', run: `the digest run the user started for "${OUT}"`, note: ' (it writes only the staging folder)', always })
      return got
    }
    const ledger = async (label, args, always = false) => {
      const got = await ztRelay(ctx, { command: `${exe} ledger ${args}`, label: `${ctx.stage}: ${label}`, phase, role: p.relay || 'relay', run: `the digest run the user started for "${OUT}"`, note: ' (it reads or records the run ledger only)', always, lockStops: false })
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
        log(`Term link: skipped — ${report.reason}.`)
        return report
      }
      report.unit = unit.id
      report.token = unit.token
      log(`Term link: ${unit.id} claimed; ${p.occurrences} occurrence(s)${cards.length ? `, ${cards.length} Card(s)` : ', whole book'}.`)

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
  },
})
// ==== end module zt-term-link ====
