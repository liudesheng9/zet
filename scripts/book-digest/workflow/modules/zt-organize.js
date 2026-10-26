// ==== module zt-organize v9 ====
// Organize: organize-survey lists what to decide; Opus writes only decisions.json (the thesis and Card notes);
// organize-apply records that no merge or move is decided; a card cycle hands the notes to DeepSeek;
// book-skeleton and one DeepSeek job write the Literature Card index and the structure Card.
// v6 (docs/literature-only-digest-design.md): a digest writes only the book's Literature tree. Topic merges, Topic
// Card moves and Topic bodies are zt-idea-grow's, so Opus decides none of them, the skeleton writes no Topic body,
// and the old single Opus job (args.organize "legacy") is gone.
// v7 (pilot 2026-10-23): the decision reads run the helper with -Allow on the survey's empty read-only list: reads
// only, and a path in the staging folder, so a run on a box copy (zt-box.txt) reads that box, not the live one.
// v8 (S10): whether the notes cycle runs is read from decisions.json by organize-apply (its `notes` count), never
// from the decider's answer; the unread option remove: 'delete' leaves.
// v9 (#176, docs/workflow-module-structure-proposal.md): the survey, the apply, the notes card cycle, the
// skeleton's plan and apply and both publications are book-steps.ps1 steps run by DeepSeek drive jobs, and the
// DeepSeek note writers and skeleton writer are content managers of the layer driver. Opus still makes the one
// approved decision; no Claude agent runs a command.
defineModule({
  id: 'zt-organize', version: '9', requires: ['zt-digest-lib'],
  summary: 'ZT digest: Organize on the Literature tree only (survey → Opus thesis and notes → apply → notes card cycle → skeleton: the Literature index and the structure Card).',
  params: {
    key: { type: 'any', description: "the run's Citation key from the prepared run (stages.prepare.output.citation_key): in mode full args.citationKey, in mode resume the ledger's retained key (#87)" },
    args: { type: 'object', required: true, description: 'the run\'s args ({{args}})' },
    mode: { type: 'string', required: true, description: 'the run\'s mode ({{mode}})' },
    model: { type: 'string', description: 'the DeepSeek model from the source stage' },
    groups: { type: 'number', description: 'the group count from the prepare stage' },
    digest: { type: 'any', description: 'the digest group reports (mode full)' },
    link: { type: 'any', description: 'the link group reports (other modes)' },
    models: { type: 'any', description: 'the model reports of the digest stage (mode full)' },
    modelsBook: { type: 'any', description: 'the model reports of the models stage (mode models)' },
  },
  run: async (ctx, p) => {
    const Z = zdMake(ctx, p)
    if (await Z.settledOnResume('organize', ['organize', 'skeleton'])) return { status: 'done', reused: true, publications: {} }
    const A = Z.A, KEY = Z.KEY, OUT = Z.OUT, MODE = Z.MODE
    const groupCount = p.groups || 0
    const modelReports = p.models || p.modelsBook || []
    const modelLines = modelReports.filter(r => r.status === 'done').map(r => `- [[${r.model}]]${r.level === 'concept' ? '（概念模型）' : r.level === 'formal' ? '（形式模型）' : ''}：${Array.isArray(r.parts) ? r.parts.map(pp => `${pp.facet}${pp.emphasis === 'core' ? '（核心）' : ''} [[${pp.address}]]`).join('、') : `${Number(r.parts) || 0} 张分卡（${Number(r.core) || 0} 张核心，见模型卡的结构列表）`}${(r.issues || []).length ? `；未解决：${r.issues.join('；')}` : ''}${r.notes && r.notes.length ? `；建议：${r.notes.join('；')}` : ''}`).join('\n')
    const groupLines = ((MODE === 'full' ? p.digest : p.link) || []).map(r => `- 第 ${r.group} 组 [[${KEY}/${r.group}]]：${r.status}${r.cards ? `，${r.cards} 张` : ''}${r.models ? `，模型卡 ${r.models}` : ''}${r.events ? `，事件卡 ${r.events}` : ''}${r.errors && r.errors !== '无' ? `；问题：${r.errors}` : ''}`).join('\n')
    const ORG = `${OUT}\\organize`
    // The stage publishes its own book revision (`zt-digest finalize -Step stage`): the ledger binds its artifacts by
    // hash and checks the coverage it derives. On grossman2002integration trial 8 no stage published, so the revision
    // stayed ready and the run could never be content-complete.
    const publish = async (layer, more) => {
      const json = await Z.bookStep(`${layer}: publish`, 'publish', { Layer: layer, ...more })
      log(json && json.state ? `${layer}: publication ${json.unit || ''} ${json.state}${json.disposition ? ` (${json.disposition})` : ''}.` : `${layer}: the publication failed: ${(json && json.error) || 'no output'}`)
      return json || { layer, state: 'failed', error: 'no output' }
    }
    // 1. The survey: a script lists what Opus must decide.
    const survey = await Z.bookStep('organize: survey', 'organize-survey', { Work: ORG })
    if (!survey || survey.ok !== true) log(`Organize: the survey failed (${(survey && survey.error) || 'no output'}).`)
    // 2. Opus decides; it writes decisions.json and nothing else.
    const decided = survey && survey.ok === true ? await Z.zAgent(
      `You decide the structure of the ZET digest of 《${A.bookTitle || KEY}》 (Literature Card [[${KEY}]], anchors [[${KEY}/1]] … [[${KEY}/${groupCount}]]). Scripts and DeepSeek do every edit; you only decide. You make no zt write.
Read ${ORG}\\survey.md first (Read tool). It lists the Card merge proposals, the models with faults, and broken Links (it may also list Topics and hubs: they are not yours; ignore them). The digest writes only the book's Literature tree: Topic trees, Topic bodies, hubs and the register belong to the zt-idea-grow workflow. When a decision needs a Card's text, read it through the helper (read-only calls only: get_card, get_tree, get_model, search_cards), always with -Allow "${ORG}\\read-only.allow.txt" (the survey's empty list: reads only), e.g.
@'
[{"tool":"get_tree","arguments":{"tree":"${KEY}"}},{"tool":"get_card","arguments":{"at":"${KEY}/1"}}]
'@ | ${ZD_CALL} - -Allow "${ORG}\\read-only.allow.txt"
${modelLines ? `The architect's notes on the book's models (Cards outside a model that should link a part of it):\n${modelLines}\n` : ''}
Decide, then write ${ORG}\\decisions.json (UTF-8, Write tool) with exactly these fields:
- "thesis": the book's central thesis as one Chinese sentence of at most 40 characters. It becomes the title of the structure Card.
- "merges": [] (always empty: the digest merges no Topic).
- "moves": [] (always empty: the digest moves no Card).
- "notes": Card-level work for DeepSeek, [{"address": "<Card>", "note": "<one Chinese sentence: what to change>"}]: a Card that should link a model part (the architect's notes), a Card whose Link reads as a plain reference but states a real relation, a Card that should link an Event Card. Only Idea Cards, Event Cards, and anchors of the book's Literature tree [[${KEY}]]; never a Topic-tree Card, and never a note that asks for a Link to a Topic-tree Card.
Use an empty list where nothing needs doing. Do not plan Topics or Hub Cards. Do not rewrite the Literature Card or the structure Card: a script and DeepSeek write them after your decisions.
Return thesis, notes (the count), and report: a short Chinese report of what you decided and what you leave for the user.`,
      {
        label: 'organize: decisions', phase: 'Organize', model: 'opus',
        schema: { type: 'object', properties: { thesis: { type: 'string' }, merges: { type: 'number' }, moves: { type: 'number' }, notes: { type: 'number' }, report: { type: 'string' } }, required: ['thesis', 'report'] },
      },
    ) : null
    // 3. The apply script records the (empty) merges and moves; in the Literature scope it could move no Topic Card.
    const moved = decided ? await Z.bookStep('organize: apply', 'organize-apply', { Work: ORG }) : null
    if (decided) log(`Organize: thesis "${decided.thesis}"; ${moved ? `${moved.merges} merges, ${moved.moves} moves, ${moved.refused} refused, ${moved.notes || 0} notes in decisions.json${moved.thesis === false ? ' (no thesis written)' : ''}` : 'apply failed (no output)'}.`)
    // 4. The notes go to DeepSeek as a card cycle: every hard rule in mode "full" (the script adds N1), else N1.
    const notes = moved && Number(moved.notes) > 0
      ? await Z.cardCycle('organize notes', `${OUT}\\cards-notes`, { notes: `${ORG}\\decisions.json`, noOrphans: true, rules: MODE === 'full' ? '' : 'N1' })
      : null
    // 4a. Organize publishes its revision once its decisions are applied and its notes are disposed.
    const organizePublished = moved ? await publish('organize', { Decisions: `${ORG}\\decisions.json`, Applied: `${ORG}\\applied.json` }) : null
    // 5. The skeleton: the Literature Card index and the structure Card.
    const SK = `${OUT}\\skeleton`
    const planned = await Z.bookStep('organize: skeleton', 'skeleton-start', { Work: SK })
    if (planned && planned.ok === true && Number(planned.jobs || 0)) {
      await Z.runJobs('organize: skeleton write', planned.file, planned.jobs, { dsh: SK, writes: `${SK}\\out`, phase: 'Organize', skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, what: 'skeleton writer', run: true })
    }
    const sk = planned && planned.ok === true
      ? await Z.bookStep('organize: skeleton apply', 'skeleton-finish', { Work: SK, Decisions: `${ORG}\\decisions.json` })
      : null
    log(sk && sk.ok === true ? `Organize: structure Card ${sk.written && sk.written.structure} ("${sk.thesis}"), ${sk.argument} argument lines, ${sk.models} models, ${sk.events} events.` : `Organize: the skeleton failed: ${JSON.stringify((sk && (sk.problems || sk.error)) || 'no output')}`)
    const skeletonPublished = sk && sk.ok === true ? await publish('skeleton', { Skeleton: `${SK}\\out\\skeleton.json` }) : null
    return { survey, decisions: decided, applied: moved, notes: notes && notes.report, skeleton: sk, publications: { organize: organizePublished, skeleton: skeletonPublished } }
  },
})
// ==== end module zt-organize ====
