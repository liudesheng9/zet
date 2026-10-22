// ==== module zt-digest-prepare v11 ====
// Prepare (mode full): prepare-book-digest.ps1 -Apply writes the Chinese text, the section groups, the
// Literature tree with one anchor per group, and the shared prefixes, then binds the immutable run contract with
// the effective settings (maxAttempts, dshRunCap) and initializes the run ledger. No stage reads the
// account-wide usage ledger: run usage is recorded per provider run from that run's own result (R343-R346).
// args.book (a Scholar Archive Work id or exact title) replaces the explicit source,
// BibTeX and title: the tool resolves them through the read-only bridge, and an unresolved choice stops with its
// candidates. A resume (mode resume) prepares nothing and generates no file: it runs the one supported
// `ledger resume`, which verifies the frozen identity, reconciles stranded claims and retained results, opens
// invocation 2 and spends the one-rerun budget, then reads the retained group enumeration, settings and
// identity from the ledger (#87, R274-R285). Other modes take the group count from args.groups.
// The output also carries groupList, the per-group items the digest stage's chains are built from.
// 10 (Tree Lock S17, #140): a preparation the owner's Lock refused stops the run with the archive's own
// typed `card_locked` refusal (R326/R337), never as a generic preparation failure.
// 11 (docs/literature-only-digest-design.md): no hubs switch is frozen; the prepared run never has a hubs layer.
defineModule({
  id: 'zt-digest-prepare', version: '11', requires: ['zt-digest-lib'],
  summary: 'ZT digest: prepare the book (mode full, from args.book or the explicit source), resume the retained run (mode resume, one ledger resume) or take args.groups; output {groups, groupList}.',
  params: {
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    needGroups: { type: 'array', default: ['full', 'link'], description: 'modes that need a group count' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile): the prepared-run variant' },
    instructions: { type: 'any', description: 'the digest instruction files (comma-separated, from consts.profile): the base prompt and the supplements of the book type' },
    noEventCandidates: { type: 'any', description: 'true: the group files leave out the event candidates (the event layer reads events.json)' },
  },
  run: async (ctx, p) => {
    const A = p.args
    const Z = zdMake(ctx, { args: A, mode: p.mode })
    const groupList = (count) => Array.from({ length: count || 0 }, (_, i) => ({ n: i + 1, nn: String(i + 1).padStart(2, '0') }))
    let groups = A.groups || 0
    if (p.mode === 'resume') {
      // R278-R285: the resume command verifies the retained identity, reclaims a lost owner, opens
      // invocation 2, resets only the unresolved units and spends resume_count. It never prepares and
      // never regenerates a file; a changed source, group manifest, contract or archive is refused here.
      const resumed = await Z.runScript('resume run', 'Prepare', `${Z.LEDGER} resume -Out "${Z.OUT}"`, ' (the one supported resume of the retained prepared run)')
      const facts = resumed.json
      if (!facts || !['resumed', 'nothing_to_resume'].includes(facts.status)) {
        throw new Error(`mode resume needs a resumable prepared run ledger: ${(resumed.run && resumed.run.output) || 'no ledger resume'}`)
      }
      // -Summary: the units counted per layer and state. The full status of a long book (about 450 KB
      // for 171 groups) is more than one relayed output line can carry.
      const status = await Z.runScript('resume status', 'Prepare', `${Z.LEDGER} status -Out "${Z.OUT}" -Summary`, ' (the retained run ledger, summarized)')
      const view = status.json
      if (!view) throw new Error(`mode resume needs a prepared run ledger with its frozen inputs: ${(status.run && status.run.output) || 'no ledger status'}`)
      groups = (view.layers && view.layers.digest && view.layers.digest.total) || 0
      if (!groups) {
        // grossman2002integration trial 17: a relay returned the summary without `layers` (a `layers_open`
        // map of its own instead); the digest layer's own brief gate carries the retained group count.
        const digestGate = await Z.gate('resume digest groups', 'digest')
        groups = (digestGate && digestGate.counts && (digestGate.counts.total || digestGate.counts.declared)) || 0
        log(`Resume: the status line carried no layer counts; the digest gate gives ${groups} retained groups.`)
      }
      if (!groups) throw new Error('mode resume found no retained digest groups; prepare the book again')
      const settings = view.settings || {}
      if (facts.status === 'nothing_to_resume') {
        log(`Resume: no executable or failed unit remains (${groups} retained groups, invocation ${facts.invocation}); the retained coverage stays what the ledger reports, and no resume was spent.`)
      } else {
        const reset = (facts.reset || []).length
        const reevaluated = (facts.reevaluated || []).length
        log(`Resume: invocation ${facts.invocation}, resume_count ${facts.resume_count}; ${reset} unresolved unit(s) reset, ${reevaluated} blocked dependant(s) back to dependency evaluation, ${(facts.reclaimed || []).length} lost claim(s) reclaimed, ${((facts.closure && facts.closure.downstream) || []).length} unit(s) in the closure to drain. ${groups} retained groups, variant ${view.variant}, settings maxAttempts ${settings.max_attempts}, dshRunCap ${settings.dsh_run_cap} s.`)
      }
      return { groups, groupList: groupList(groups), resumed: facts.status === 'resumed', resume: facts, settings, citation_key: view.citation_key }
    }
    if (p.mode === 'full') {
      // args.book names the book in the Scholar Archive; the tool resolves its Markdown, BibTeX and
      // title, so the explicit arguments are left out rather than passed empty.
      const book = A.book ? `-Book '${A.book}'`
        : `-Source "${A.source}" -BibTeX '${A.bibtex}' -BookTitle '${A.bookTitle}'`
      // The layer switches the run declares are frozen into the prepared target: a layer this run
      // deliberately excludes is excluded in the ledger too, so no unit waits for work that will not come.
      const switches = {}
      for (const name of ['terms', 'models', 'events', 'findings', 'figures', 'bridge', 'notation', 'boundary', 'proof', 'theory']) {
        if (typeof A[name] === 'boolean') switches[name] = A[name]
      }
      const runOptions = Object.keys(switches).length ? ` -RunOptions '${JSON.stringify(switches)}'` : ''
      const prep = await Z.zAgent(
        `You prepare a book for a ZET digest. Do exactly these steps and nothing else.
1. Run this PowerShell command (one call) in the foreground with timeout_ms: 600000 and read its last output line, a compact JSON manifest:
${Z.tool('prepare-book-digest')} ${book} -Out "${Z.OUT}" -CitationKey ${Z.KEY} -MaxChars ${A.maxChars || 20000} -MaxUnits ${A.maxUnits || 3}${A.startHeading ? ` -StartHeading '${A.startHeading}'` : ''}${A.endHeading ? ` -EndHeading '${A.endHeading}'` : ''}${A.skipHeadings ? ` -SkipHeadings '${A.skipHeadings}'` : ''}${A.fileId ? ` -FileId ${A.fileId}` : ''}${p.instructions ? ` -Instructions '${p.instructions}'` : ''}${p.noEventCandidates ? ' -NoEventCandidates' : ''} -Variant ${p.profile && p.profile.book === 'mathematics' ? 'math' : 'econ'} -MaxAttempts ${Z.MAX_ATTEMPTS} -DshRunCap ${Z.DSH_RUN_CAP}${runOptions} -Apply
If it fails, return ok=false with the error text; do not retry with other options and do not delete anything. Exit code 3 with needs_selection means the last line names the Works or files to choose from: put that line in error, and the run halts for the choice; pass one Work id as args.book (or one file id as args.fileId) and run again.
Return groups = the number of entries in manifest.groups.`,
        {
          label: 'prepare book', phase: 'Prepare', model: 'haiku',
          schema: {
            type: 'object',
            properties: { ok: { type: 'boolean' }, error: { type: 'string' }, groups: { type: 'number' } },
            required: ['ok', 'groups'],
          },
        },
      )
      if (!prep || !prep.ok || !prep.groups) {
        log(`Preparation failed: ${prep && prep.error}`)
        // R326/R329/R337: preparation writes the Literature tree first, so a Lock on that tree stops the
        // run here with the archive's own typed refusal, never as a generic preparation failure.
        const lock = Z.lockRefusal((prep && prep.error) || '')
        if (lock) throw Z.lockStop('prepare', lock)
        throw new Error(`preparation failed: ${(prep && prep.error) || 'no report from the prepare agent'}`)
      }
      groups = prep.groups
    }
    if (!groups && (p.needGroups || ['full', 'link']).includes(p.mode)) throw new Error(`mode "${p.mode}" needs args.groups`)
    return { groups, groupList: groupList(groups), citation_key: Z.KEY }
  },
})
// ==== end module zt-digest-prepare ====
