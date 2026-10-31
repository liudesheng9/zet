// ==== module zt-digest-prepare v20 ====
// Prepare (mode full): prepare-book-digest writes the Chinese text, the section groups, the Literature tree with one
// anchor per group, and the shared prefixes, then binds the immutable run contract with the effective settings
// (maxAttempts, dshRunCap) and initializes the run ledger. No stage reads the account-wide usage ledger: run usage is
// recorded per provider run from that run's own result (R343-R346).
// args.book (a Scholar Archive Work id or exact title) replaces the explicit source, BibTeX and title: the tool resolves
// them through the read-only bridge, and an unresolved choice stops with its candidates. A resume (mode resume)
// prepares nothing and generates no file: it runs the one supported `ledger resume`, which verifies the frozen
// identity, reconciles stranded claims and retained results, opens invocation 2 and spends the one-rerun budget, then
// reads the retained group enumeration, settings and identity from the ledger (#87, R274-R285). Other modes take the
// group count from args.groups.
// The output also carries groupList, the per-group items the digest stage's chains are built from.
// 10 (Tree Lock S17, #140): a preparation the owner's Lock refused stops the run with the archive's own typed
// `card_locked` refusal (R326/R337), never as a generic preparation failure.
// 11 (docs/literature-only-digest-design.md): no hubs switch is frozen; the prepared run never has a hubs layer.
// 13 (S24, #167): the Term-card branch and the Term-link pass freeze their switches too (termCards, termLink), so a
// layer this run disables is excluded in the prepared target and no unit waits for work that will not come (R0430).
// 14 (S25, #168): modes term-cards and term-link are the backfill's own preparation. They run the one existing
// zt-digest prepare with -Sections (reuse or reconstruct the staged source slices) or -Validate (inspect the book and
// the staging only), read the book's Citation key from the prepared run's manifest and stop before any provider work on
// a Lock or a conflict. -Sections creates no root, anchor, group, model or narrative Card (R0435); neither mode
// reruns an ordinary digest or model stage (R0436).
// 16 (#176, docs/workflow-module-structure-proposal.md): the stage runs its whole chain as one book-steps.ps1 step
// (prepare: the full preparation, the one resume with its summary and digest-gate fallback, or a backfill branch),
// run by a DeepSeek drive job; no Claude relay runs a command.
// 18 (#181): the stage takes the fresh preparation's Citation key from its own `key` param (the blueprint passes
// args.citationKey) instead of zdMake reading args.citationKey: the prepared run it creates is the one source every
// later stage reads the key from (stages.prepare.output.citation_key), so a resume that names only `out` still
// hands each stage the book's key.
// 19 (live run 2026-10-27, becker2025division): the fresh preparation's drive job runs in the staging folder's
// parent, because the staging folder does not exist until this step creates it and deepseek_run refuses a missing cwd.
// 20 (#221): mode cited-cards is the Cited backfill of one already digested book; like term-link it runs the
// preparation with -Validate only, so it inspects the book and its staging and starts no provider work.
defineModule({
  id: 'zt-digest-prepare', version: '20', requires: ['zt-digest-lib'],
  summary: 'ZT digest: prepare the book (mode full, from args.book or the explicit source), resume the retained run (mode resume, one ledger resume), validate or re-stage one already digested book for a Term backfill (modes term-cards and term-link) or take args.groups; output {groups, groupList, citation_key, backfill?}.',
  params: {
    key: { type: 'any', description: "a fresh preparation's Citation key (args.citationKey): the prepared run this stage creates is what every later stage reads it from (#181)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    needGroups: { type: 'array', default: ['full', 'link'], description: 'modes that need a group count' },
    profile: { type: 'object', required: true, description: 'the book profile (consts.profile): the prepared-run variant' },
    instructions: { type: 'any', description: 'the digest instruction files (comma-separated, from consts.profile): the base prompt and the supplements of the book type' },
    noEventCandidates: { type: 'any', description: 'true: the group files leave out the event candidates (the event layer reads events.json)' },
  },
  run: async (ctx, p) => {
    const A = p.args
    const Z = zdMake(ctx, { args: A, mode: p.mode, key: p.key })
    const groupList = (count) => Array.from({ length: count || 0 }, (_, i) => ({ n: i + 1, nn: String(i + 1).padStart(2, '0') }))
    let groups = A.groups || 0
    if (p.mode === 'resume') {
      // R278-R285: the step runs the one supported resume and its summarized status; the stage reads the retained
      // group enumeration and identity, and a summary without layer counts is answered by the digest gate inside
      // the step.
      const resumed = await Z.bookStep('prepare resume', 'prepare', { Mode: 'resume' })
      if (!resumed || resumed.ok !== true) throw new Error(`mode resume needs a resumable prepared run ledger: ${(resumed && resumed.error) || 'no ledger resume'}`)
      // #180: the resume step answers the retained facts as flat counts; its full tool lines are its detail file.
      const n = (v) => Number(v || 0)
      const view = { citation_key: resumed.citation_key, variant: resumed.variant }
      groups = n(resumed.groups)
      if (!groups) throw new Error('mode resume found no retained digest groups; prepare the book again')
      const settings = { max_attempts: n(resumed.max_attempts), dsh_run_cap: n(resumed.dsh_run_cap) }
      if (!resumed.resumed) {
        log(`Resume: no executable or failed unit remains (${groups} retained groups, invocation ${n(resumed.invocation)}); the retained coverage stays what the ledger reports, and no resume was spent.`)
      } else {
        log(`Resume: invocation ${n(resumed.invocation)}, resume_count ${n(resumed.resume_count)}; ${n(resumed.reset)} unresolved unit(s) reset, ${n(resumed.reevaluated)} blocked dependant(s) back to dependency evaluation, ${n(resumed.reclaimed)} lost claim(s) reclaimed, ${n(resumed.downstream)} unit(s) in the closure to drain. ${groups} retained groups, variant ${view.variant}, settings maxAttempts ${settings.max_attempts}, dshRunCap ${settings.dsh_run_cap} s.`)
      }
      // v15 (S30 pilot, #172): the prepared run's frozen layer switches. A resume gets only `out`, so the
      // Term stages read termCards/termLink here instead of their args defaults (true).
      // #180: the resume step names the frozen switches a layer switched off, as one short list.
      const off = String(resumed.off || '').split(',').filter(Boolean)
      const runOptions = off.length ? Object.fromEntries(off.map((k) => [k, false])) : null
      return { groups, groupList: groupList(groups), resumed: resumed.resumed === true, resume: null, settings, citation_key: view.citation_key, run_options: runOptions }
    }
    // #221: mode cited-cards is a backfill too; like term-link it only validates the book and its staging.
    if (p.mode === 'term-cards' || p.mode === 'term-link' || p.mode === 'cited-cards') {
      // S25 (#168): the backfill's own preparation. `-Sections` reuses or reconstructs the staged source slices the
      // Term-card branch reads; `-Validate` only inspects the already digested book, because the deterministic
      // Term-link pass needs no source and starts no provider probe (R0436). Both write no Card, read the Citation
      // key from the prepared run's own manifest, and stop before any provider work on a Lock or a conflict.
      const line = await Z.bookStep(`prepare backfill: ${p.mode}`, 'prepare', { Mode: p.mode })
      if (!line || line.ok !== true) {
        throw new Error(`the backfill of ${(line && line.book) || 'the book'} cannot start: ${(line && line.error) || 'no output'}`)
      }
      const n = (v) => Number(v || 0)
      log(`Backfill (${line.step}): ${line.book} — ${n(line.sections)} section(s) ${line.reused ? 'reused' : 'reconstructed'}${n(line.rebuilt) ? ` (${n(line.rebuilt)})` : ''}; ${n(line.rows)} Term row(s)${line.legacy ? ' in the legacy table' : ''}; ${n(line.heads)} published head(s), ${n(line.parts)} part(s), ${n(line.occupying)} occupying; ${n(line.conflicts)} conflict(s).`)
      return { groups: 0, groupList: [], citation_key: line.book, backfill: line }
    }
    if (p.mode === 'full') {
      // The layer switches the run declares are frozen into the prepared target: a layer this run deliberately
      // excludes is excluded in the ledger too, so no unit waits for work that will not come.
      const switches = {}
      for (const name of ['terms', 'models', 'events', 'findings', 'figures', 'bridge', 'notation', 'boundary', 'proof', 'theory', 'termCards', 'termLink']) {
        if (typeof A[name] === 'boolean') switches[name] = A[name]
      }
      const options = {
        Mode: 'full',
        CitationKey: Z.KEY,
        MaxChars: A.maxChars || 20000,
        MaxUnits: A.maxUnits || 3,
        Variant: p.profile && p.profile.book === 'mathematics' ? 'math' : 'econ',
        MaxAttempts: Z.MAX_ATTEMPTS,
        DshRunCap: Z.DSH_RUN_CAP,
      }
      // args.book names the book in the Scholar Archive; the tool resolves its Markdown, BibTeX and title, so the
      // explicit arguments are left out rather than passed empty.
      if (A.book) options.Book = A.book
      else { options.Source = A.source; options.BibTeX = A.bibtex; options.BookTitle = A.bookTitle }
      if (A.startHeading) options.StartHeading = A.startHeading
      if (A.endHeading) options.EndHeading = A.endHeading
      if (A.skipHeadings) options.SkipHeadings = A.skipHeadings
      if (A.fileId) options.FileId = A.fileId
      if (p.instructions) options.Instructions = p.instructions
      if (p.noEventCandidates) options.NoEventCandidates = true
      if (Object.keys(switches).length) options.RunOptions = JSON.stringify(switches)
      // v12 (S1): the preparation ends with one short line (the group count), which a Haiku agent had counted out of
      // the whole manifest. Exit 3 with needs_selection names the Works or files to choose from: the run halts for the
      // choice (pass one Work id as args.book, or one file id as args.fileId).
      const prepared = await Z.bookStep('prepare book', 'prepare', options, { workspace: String(Z.OUT).replace(/[\\/]+$/, '').replace(/[\\/][^\\/]*$/, '') })
      const prep = prepared && prepared.ok === true && Number(prepared.groups) > 0
        ? { ok: true, groups: Number(prepared.groups) }
        : { ok: false, error: prepared ? JSON.stringify(prepared) : 'no output' }
      if (!prep || !prep.ok || !prep.groups) {
        log(`Preparation failed: ${prep && prep.error}`)
        // R326/R329/R337: preparation writes the Literature tree first, so a Lock on that tree stops the run here with
        // the archive's own typed refusal, never as a generic preparation failure.
        const lock = (prepared && (prepared.code === Z.LOCK || prepared.error === Z.LOCK)) ? prepared : Z.lockRefusal(JSON.stringify(prepared || {}))
        if (lock) throw Z.lockStop('prepare', lock)
        throw new Error(`preparation failed: ${(prep && prep.error) || 'no report from the prepare step'}`)
      }
      groups = prep.groups
    }
    if (!groups && (p.needGroups || ['full', 'link']).includes(p.mode)) throw new Error(`mode "${p.mode}" needs args.groups`)
    return { groups, groupList: groupList(groups), citation_key: Z.KEY }
  },
})
// ==== end module zt-digest-prepare ====



