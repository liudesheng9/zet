// ==== module zt-organize v6 ====
// Organize: organize-survey.ps1 lists what to decide; Opus writes only decisions.json (the thesis and Card
// notes); organize-apply.ps1 records that no merge or move is decided; a card cycle hands the notes to DeepSeek;
// book-skeleton.ps1 and one DeepSeek job write the Literature Card index and the structure Card.
// v6 (docs/literature-only-digest-design.md): a digest writes only the book's Literature tree. Topic merges,
// Topic Card moves and Topic bodies are zt-idea-grow's, so Opus decides none of them, the skeleton writes no
// Topic body, and the old single Opus job (args.organize "legacy") is gone.
defineModule({
  id: 'zt-organize', version: '6', requires: ['zt-digest-lib'],
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
    const A = Z.A, KEY = Z.KEY, OUT = Z.OUT, SCRIPTS = Z.SCRIPTS, MODE = Z.MODE
    const groupCount = p.groups || 0
    const modelReports = p.models || p.modelsBook || []
    const modelLines = modelReports.filter(r => r.status === 'done').map(r => `- [[${r.model}]]${r.level === 'concept' ? '（概念模型）' : r.level === 'formal' ? '（形式模型）' : ''}：${Array.isArray(r.parts) ? r.parts.map(pp => `${pp.facet}${pp.emphasis === 'core' ? '（核心）' : ''} [[${pp.address}]]`).join('、') : `${Number(r.parts) || 0} 张分卡（${Number(r.core) || 0} 张核心，见模型卡的结构列表）`}${(r.issues || []).length ? `；未解决：${r.issues.join('；')}` : ''}${r.notes && r.notes.length ? `；建议：${r.notes.join('；')}` : ''}`).join('\n')
    const groupLines = ((MODE === 'full' ? p.digest : p.link) || []).map(r => `- 第 ${r.group} 组 [[${KEY}/${r.group}]]：${r.status}${r.cards ? `，${r.cards} 张` : ''}${r.models ? `，模型卡 ${r.models}` : ''}${r.events ? `，事件卡 ${r.events}` : ''}${r.errors && r.errors !== '无' ? `；问题：${r.errors}` : ''}`).join('\n')
    const ORG = `${OUT}\\organize`
    // The stage publishes its own book revision (`zt-digest finalize -Step stage`): the ledger binds its artifacts
    // by hash and checks the coverage it derives. On grossman2002integration trial 8 no stage published, so the
    // revision stayed ready and the run could never be content-complete.
    const publish = async (layer, more) => {
      const { run, json } = await Z.runScript(`${layer}: publish`, 'Organize', `${Z.DIGEST} finalize -Step stage -Out "${Z.OUT}" -Layer ${layer}${more}`, ' (it records the stage publication in the ledger)')
      log(json && json.state ? `${layer}: publication ${json.unit || ''} ${json.state}${json.disposition ? ` (${json.disposition})` : ''}.` : `${layer}: the publication failed: ${(run && run.output) || 'no output'}`)
      return json || { layer, state: 'failed', error: (run && run.output) || 'no output' }
    }
    // 1. The survey: a script lists what Opus must decide.
    const surveyed = await Z.runScript('organize: survey', 'Organize', `${Z.tool('organize-survey')} -Staging "${OUT}" -Work "${ORG}"`)
    const survey = surveyed.json
    if (!survey) log(`Organize: the survey failed (${surveyed.run && surveyed.run.output}).`)
    // 2. Opus decides; it writes decisions.json and nothing else.
    const decided = survey ? await Z.zAgent(
      `You decide the structure of the ZET digest of 《${A.bookTitle}》 (Literature Card [[${KEY}]], anchors [[${KEY}/1]] … [[${KEY}/${groupCount}]]). Scripts and DeepSeek do every edit; you only decide. You make no zt write.
Read ${ORG}\\survey.md first (Read tool). It lists the Card merge proposals, the models with faults, and broken Links (it may also list Topics and hubs: they are not yours; ignore them). The digest writes only the book's Literature tree: Topic trees, Topic bodies, hubs and the register belong to the zt-idea-grow workflow. When a decision needs a Card's text, read it through the helper (read-only calls only: get_card, get_tree, get_model, search_cards), e.g.
@'
[{"tool":"get_tree","arguments":{"tree":"${KEY}"}},{"tool":"get_card","arguments":{"at":"${KEY}/1"}}]
'@ | ${ZD_CALL} -
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
    const applied = decided ? await Z.runScript('organize: apply', 'Organize', `${Z.tool('organize-apply')} -Work "${ORG}"`) : null
    const moved = applied ? applied.json : null
    if (decided) log(`Organize: thesis "${decided.thesis}"; ${moved ? `${moved.merges} merges, ${moved.moves} moves, ${moved.refused} refused` : `apply failed (${applied && applied.run && applied.run.output})`}; ${decided.notes || 0} notes.`)
    // 4. The notes go to DeepSeek as a card cycle: every hard rule in mode "full" (the script adds N1), else N1.
    const notes = decided && decided.notes ? await Z.runCards('organize notes', `${OUT}\\cards-notes`, ` -Notes "${ORG}\\decisions.json" -NoOrphans`, MODE === 'full' ? '' : 'N1') : null
    // 4a. Organize publishes its revision once its decisions are applied and its notes are disposed.
    const organizePublished = moved ? await publish('organize', ` -Decisions "${ORG}\\decisions.json" -Applied "${ORG}\\applied.json"`) : null
    // 5. The skeleton: the Literature Card index and the structure Card.
    const SK = `${OUT}\\skeleton`
    const kstep = (s, more = '') => `${Z.tool('book-skeleton')} -Step ${s} -Staging "${OUT}" -Work "${SK}"${more}`
    const kopts = { dsh: SK, skills: ['asd-ste100-zh'], effort: Z.DSH_EFFORT, phase: 'Organize', writes: `${SK}\\out`, remove: 'delete', keep: true }
    await Z.drive('organize: skeleton', { ...kopts, command: kstep('plan') })
    const sk = zdLastJson(((await Z.drive('organize: skeleton apply', { ...kopts, command: kstep('apply', ` -Decisions "${ORG}\\decisions.json" -Apply`), cleanup: true })) || {}).output)
    log(sk && sk.ok ? `Organize: structure Card ${sk.written && sk.written.structure} ("${sk.thesis}"), ${sk.argument} argument lines, ${sk.models} models, ${sk.events} events.` : `Organize: the skeleton failed: ${JSON.stringify((sk && sk.problems) || 'no output')}`)
    const skeletonPublished = sk && sk.ok ? await publish('skeleton', ` -Skeleton "${SK}\\out\\skeleton.json"`) : null
    return { survey, decisions: decided, applied: moved, notes: notes && notes.report, skeleton: sk, publications: { organize: organizePublished, skeleton: skeletonPublished } }
  },
})
// ==== end module zt-organize ====
