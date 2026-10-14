export const meta = {
  name: 'zt-book-digest',
  description: 'Digest a Chinese-translated book into ZET Cards: Haiku managers drive DeepSeek Harness runs that write idea, event, and Model Cards for their sections; as each block finishes, the model compiler builds every new model to its elaboration level: DeepSeek briefs the source, a Sonnet architect only plans each model (level, parts in build order, where Derivation parts go and how many), a script builds the skeleton, DeepSeek writes the Cards and charts, then the Derivation parts, and a DeepSeek reviewer splits any derivation that jumps or is too dense; a link pass ties the parts of the book together, then Opus organizes the structure',
  whenToUse: 'Turn a Scholar Archive book (Chinese bilingual Markdown, digested as Chinese only) into an agent Literature tree plus Topic trees in the ZET slip-box; mode "link" re-runs only the cross-part link pass on an existing digest; mode "models" re-runs only the model stage on its Model Cards (with rebuild: true, also on models that already have parts).',
  phases: [
    { title: 'Prepare', detail: 'Chinese-only book text, section groups, agent Literature tree with anchors', model: 'haiku' },
    { title: 'Digest', detail: 'each Haiku manager drives its block of DeepSeek runs; each run gets its own sections inline, searches the rest of the book on disk, and writes Cards for its sections, one whole Model Card per model', model: 'haiku' },
    { title: 'Models', detail: 'the model compiler, per batch of models as soon as a digest block finishes: DeepSeek writes a brief of each model\'s source; one Sonnet architect writes each plan.json (level, parts in build order, premises, where Derivation parts go and how many) and nothing else; model-compiler.ps1 builds the skeleton and the job files; Haiku drivers relay DeepSeek rounds: fill (Model Card, setting, parts, charts), derive (the Derivation parts), review (a reviewer splits a jumping or dense derivation into two or more parts, the script inserts them, the writer continues); the script reports each model and the driver archives its conversations', model: 'sonnet' },
    { title: 'Link', detail: 'each DeepSeek run gets only the Cards it owns, searches the book and the Card index on disk, and appends reasoned cross-part Links', model: 'haiku' },
    { title: 'Cleanup', detail: 'Haiku archives conversations, removes the DeepSeek workspace and scratch files', model: 'haiku' },
    { title: 'Organize', detail: 'Opus merges Topics, writes hubs, entry points, the book structure Card, and links', model: 'opus' },
  ],
}

// args: { mode ('full' | 'link' | 'models'), source, out, citationKey, bibtex, bookTitle,
//         startHeading ('导论'), endHeading ('参考'): the Chinese headings where the digested text starts and stops,
//         skipHeadings (e.g. '参考文献,习题': headings whose units are left out, such as each chapter's references
//         and exercises; default none),
//         groups (modes 'link' and 'models': the existing group count), organize (modes 'link' and 'models': default false),
//         perManager (5), maxChars (20000), maxUnits (3),
//         dshModel ('deepseek-official/deepseek-flash'), dshEffort ('low'),
//         models (true: run the model stage), rebuild (false: also rebuild models that already have parts;
//         redoModels is the old name), perSonnet (6: formal models per batch, one Sonnet architect each;
//         neighbouring models of the book share a batch), perConcept (10: concept models per batch),
//         dshPerBatch (8: DeepSeek runs a batch keeps going at the same time; dshPerSonnet is the old name),
//         only (mode "models": the Model Card addresses to work on, e.g. a pilot; default all),
//         level ('concept' | 'formal': build every model of the run to this elaboration level; default each
//         model's own level, which DeepSeek proposes and the Sonnet architect confirms),
//         briefEffort ('medium'), deriveEffort ('high'), reviewEffort ('high'): DeepSeek effort of the brief,
//         the Derivation parts and their follow-ups, and the review (dshEffort is the fill's),
//         reviewRounds (2: review, split, review again; 0 skips the review), architectEffort (Sonnet's effort),
//         maxSentences (12), maxPartChars (400), maxSteps (6): the Card size of a formal model's part;
//         a part over a limit splits, and a long derivation step becomes a chain of Derivation parts }
//
// How the stages hand work on:
//   Prepare ─▶ Digest block 1 (Haiku ▸ DeepSeek ×5) ─▶ Models for block 1 (model compiler, per batch) ─┐
//           ─▶ Digest block 2 (Haiku ▸ DeepSeek ×5) ─▶ Models for block 2 (model compiler, per batch) ─┼─▶ Link ─▶ Cleanup ─▶ Organize (Opus)
//           ─▶ …                                                                                      ─┘
// Blocks run in parallel; a block's models are compiled while later blocks are still digesting.
//
// The model compiler, per batch (model-compiler.ps1 does every step marked "script"):
//   brief (script: context packs ▸ DeepSeek: brief.md) ─▶ plan (Sonnet: plan.json, validated by script)
//   ─▶ fill (script: skeleton, jobs ▸ DeepSeek: Model Card, setting, parts, charts)
//   ─▶ derive (script: jobs with filled premises ▸ DeepSeek: Derivation parts)
//   ─▶ review ×reviewRounds (script: one job per chain ▸ DeepSeek: verdicts ▸ script: splits ▸ DeepSeek: follow-ups)
//   ─▶ report (script) and clean-up. Haiku only relays DeepSeek jobs; Sonnet only plans.
const A = args && typeof args === 'object' ? args : {}
const MODE = A.mode || 'full'
// The args are an object, never free text: without these fields the stages would run on "undefined".
const missing = ['citationKey', 'out', ...(MODE === 'full' ? ['source', 'bibtex', 'bookTitle'] : [])].filter(k => !A[k])
if (missing.length) return { stage: 'args', error: `args must be an object with ${missing.join(', ')}; got ${typeof args === 'string' ? 'a string' : JSON.stringify(args)}` }
const KEY = A.citationKey
const OUT = A.out
const DIGEST_PREFIX = `${OUT}\\shared-prefix.md`
const LINK_PREFIX = `${OUT}\\link-prefix.md`
const PER_MANAGER = A.perManager || 5
const DSH_MODEL = A.dshModel || 'deepseek-official/deepseek-flash'
const DSH_EFFORT = A.dshEffort || 'low'
const SCRIPTS = 'E:\\zet\\scripts\\book-digest'
const MODELS = A.models !== false
const ARCH_DOC = `${SCRIPTS}\\model-architect.md`
const COMPILER = `${SCRIPTS}\\model-compiler.ps1`
const MODEL_WORK = `${OUT}\\models`
const PER_SONNET = Math.max(1, A.perSonnet || 6)
const DSH_PER_BATCH = Math.max(1, A.dshPerBatch || A.dshPerSonnet || 8)
const REBUILD = !!(A.rebuild || A.redoModels)
// The elaboration level: a concept model (level 1) is stated in words, with no Derivation parts; a
// formal model (level 2) is built self-sufficient. Unset: each model's own level.
const LEVEL = ['concept', 'formal'].includes(A.level) ? A.level : null
const PER_CONCEPT = Math.max(1, A.perConcept || 10)
const BRIEF_EFFORT = A.briefEffort || 'medium'
const DERIVE_EFFORT = A.deriveEffort || 'high'
const REVIEW_EFFORT = A.reviewEffort || 'high'
const REVIEW_ROUNDS = Math.max(0, A.reviewRounds ?? 2)
// Card size limits, passed to card-style-check.ps1 (R13, R14): the size of a Card, not the length of
// a derivation, decides how many Derivation parts carry a step.
const LIMITS = `-MaxSentences ${A.maxSentences || 12} -MaxChars ${A.maxPartChars || 400} -MaxSteps ${A.maxSteps || 6}`
const CHECK = `${SCRIPTS}\\card-style-check.ps1`
const HELPER = 'C:\\Users\\chenx\\.codex\\skills\\zt\\scripts\\zt-mcp.ps1'
// Skills the manager passes to DeepSeek Harness in the `skills` field of deepseek_run and deepseek_prompt.
// Both skills are installed in DeepSeek Harness (deepseek_skills lists them). No prompt file carries a copy of a skill.
const DSH_SKILLS = ['zettelkasten', 'asd-ste100-zh']
const FILL_SKILLS = ['asd-ste100-zh']
const DSH_TOOLS ='select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_respond,mcp__deepseek__deepseek_prompt,mcp__deepseek__deepseek_close,mcp__deepseek__deepseek_archive'
const DRIVE_TOOLS = `${DSH_TOOLS},mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_remove_workspace`

const USAGE = {
  type: 'object',
  properties: {
    input_cache_miss: { type: 'number' }, cached_read: { type: 'number' },
    output: { type: 'number' }, cost: { type: 'number' }, calls: { type: 'number' },
  },
  required: ['input_cache_miss', 'cached_read', 'output', 'cost', 'calls'],
}
const usageStep = 'Load the DeepSeek tools with ToolSearch query "select:mcp__deepseek__deepseek_usage" and call deepseek_usage with days=1. Sum each field over the rows for the last day listed.'

// --- Prepare ---------------------------------------------------------------------------------
phase('Prepare')
let groupCount = A.groups || 0
let usageBefore = null
if (MODE === 'full') {
  const prep = await agent(
    `You prepare a book for a ZET digest. Do exactly these steps and nothing else.
1. Run this PowerShell command (one call) and read its last output line, a compact JSON manifest:
pwsh -NoProfile -File ${SCRIPTS}\\prepare-book-digest.ps1 -Source "${A.source}" -Out "${OUT}" -CitationKey ${KEY} -BibTeX '${A.bibtex}' -BookTitle '${A.bookTitle}' -MaxChars ${A.maxChars || 20000} -MaxUnits ${A.maxUnits || 3}${A.startHeading ? ` -StartHeading '${A.startHeading}'` : ''}${A.endHeading ? ` -EndHeading '${A.endHeading}'` : ''}${A.skipHeadings ? ` -SkipHeadings '${A.skipHeadings}'` : ''} -Apply
If it fails, return ok=false with the error text; do not retry with other options and do not delete anything.
2. ${usageStep}
Return groups = the number of entries in manifest.groups and usage_before = the summed usage.`,
    {
      label: 'prepare book', phase: 'Prepare', model: 'haiku', effort: 'low',
      schema: {
        type: 'object',
        properties: { ok: { type: 'boolean' }, error: { type: 'string' }, groups: { type: 'number' }, usage_before: USAGE },
        required: ['ok', 'groups'],
      },
    },
  )
  if (!prep || !prep.ok || !prep.groups) {
    log(`Preparation failed: ${prep && prep.error}`)
    return { stage: 'prepare', prep }
  }
  groupCount = prep.groups
  usageBefore = prep.usage_before
}
if (!groupCount && MODE !== 'models') return { stage: 'prepare', error: 'mode "link" needs args.groups' }

const groups = Array.from({ length: groupCount }, (_, i) => {
  const n = i + 1
  const nn = String(n).padStart(2, '0')
  return { n, nn, file: `${OUT}\\groups\\g${nn}.md`, snapshot: `${OUT}\\snapshot-g${nn}.md`, link: `${OUT}\\link-g${nn}.md` }
})
// Contiguous blocks in book order: managers run in parallel on different chapters (few duplicate
// Topics), and inside a block each run starts after its predecessor and sees its new Cards.
const toBlocks = list => {
  const out = []
  for (let i = 0; i < list.length; i += PER_MANAGER) out.push(list.slice(i, i + PER_MANAGER))
  return out
}
const blocks = toBlocks(groups)

// --- one Haiku manager per block -----------------------------------------------------------------
const REPORT = {
  type: 'object',
  properties: {
    group: { type: 'number' }, status: { type: 'string', enum: ['done', 'failed'] },
    cards: { type: 'number' }, models: { type: 'number' }, events: { type: 'number' }, links: { type: 'number' }, topics: { type: 'array', items: { type: 'string' } },
    errors: { type: 'string' }, archived: { type: 'boolean' },
    tool_calls: { type: 'number' }, context_used: { type: 'number' },
  },
  required: ['group', 'status', 'errors', 'archived'],
}

const STAGES = {
  digest: {
    phase: 'Digest',
    steps: g => `Group ${g.n}:
  a. Snapshot the slip-box (one PowerShell call): pwsh -NoProfile -File ${SCRIPTS}\\snapshot-topics.ps1 -Out "${g.snapshot}"
  b. deepseek_run with exactly: cwd "${OUT}"; prompt_files ["${DIGEST_PREFIX}", "${g.file}"] (the book, its index and the snapshot stay on disk; DSH reads what it needs); prompt "按以上说明完成你的组，只回复一行 JSON。"; skills ${JSON.stringify(DSH_SKILLS)}; model "${DSH_MODEL}"; reasoning_effort "${DSH_EFFORT}"; wait_seconds 900.`,
    answer: '{"group","cards","models","events","topics","anchor","errors"}',
    fields: 'cards (Cards written), models (Model Cards), events (Event Cards), topics (as "地址 主题名")',
  },
  link: {
    phase: 'Link',
    steps: g => `Group ${g.n}:
  deepseek_run with exactly: cwd "${OUT}"; prompt_files ["${LINK_PREFIX}", "${g.link}"]; prompt "按以上说明完成你的组的连接，只回复一行 JSON。"; skills ${JSON.stringify(DSH_SKILLS)}; model "${DSH_MODEL}"; reasoning_effort "${DSH_EFFORT}"; wait_seconds 900.
  When it finished with status "done", mark it (one PowerShell call): Set-Content -LiteralPath "${OUT}\\linked-g${g.nn}.txt" -Value done`,
    answer: '{"group","cards_linked","links_added","errors"}',
    fields: 'cards (= cards_linked), links (= links_added)',
  },
}

const manage = (block, b, stage) => {
  const S = STAGES[stage]
  return agent(
    `You manage ${block.length} DeepSeek Harness (DSH) runs of a ZET book digest (${stage} pass), one after another. DSH reads the book and changes the Cards; you only start, supervise, and clean up each run. Keep your messages short.
Load all tools with one ToolSearch call: query "${DSH_TOOLS}".

For each group below, in order, finish it completely before starting the next:
${block.map(S.steps).join('\n')}

Pass every deepseek_run value unchanged: the identical leading prompt files, prompt, skills, model and effort keep the DSH prompt cache warm. You send the skills: put them in the "skills" field of the call, exactly as given. Do not paste a skill's text into the prompt and do not write it to a prompt file. If a call fails because a skill is not found, stop and mark the group "failed" with that error; do not start the run without its skills.
Then follow "status" until the run ends:
  - "running": deepseek_wait (session_id, wait_seconds 900).
  - "needs_response": approve (decision "allow") a pwsh command that runs zt-mcp.ps1, and read-only reads or searches (Read, Get-Content, Select-String) of files under "${OUT}" or of "${A.source}"; reject anything that writes, moves or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
  - "error", or "finished" without a one-line JSON answer: once, deepseek_prompt (same session, wait_seconds 900, the same skills as the run) with "按 resume 行修正失败的调用后重跑，最后只回复一行 JSON。"; if it still fails, mark the group "failed".
  - "finished": parse its JSON ${S.answer}.
  Then clean up: deepseek_close and deepseek_archive (session_id); archived=true when archiving succeeds.
  Record tool_calls = the sum of the counts in the last turn report's tools_used, and context_used = context.used.

Return reports: one entry per group in this block with group, status, ${S.fields}, errors, archived, tool_calls, context_used.`,
    {
      label: `${stage} manager ${b + 1}: groups ${block[0].n}-${block[block.length - 1].n}`, phase: S.phase, model: 'haiku', effort: 'low',
      schema: { type: 'object', properties: { reports: { type: 'array', items: REPORT } }, required: ['reports'] },
    },
  )
}

// --- Models: the model compiler turns each new Model Card into a structured model ---------------------
// Sonnet only plans (plan.json); model-compiler.ps1 does every structural step; DeepSeek writes,
// draws, derives, and reviews; Haiku only relays DeepSeek jobs between them.
const MODEL_LIST = {
  type: 'object',
  properties: {
    ok: { type: 'boolean' }, error: { type: 'string' },
    models: { type: 'array', items: { type: 'object', properties: { address: { type: 'string' }, title: { type: 'string' }, level: { type: ['string', 'null'] }, parts: { type: 'number' }, issues: { type: 'number' }, faults: { type: 'number' } }, required: ['address'] } },
  },
  required: ['ok', 'models'],
}
const listModels = (selector, label) => agent(
  `List the Model Cards of a ZET book digest. Run this one PowerShell command and read its last output line, a compact JSON object {"models":[{"address","title","group","level","parts","issues","faults"}]}:
pwsh -NoProfile -File ${SCRIPTS}\\list-models.ps1 -Staging "${OUT}" ${selector}
If it fails, return ok=false with the error text and models=[]; do not retry with other options. Otherwise return ok=true and the models array exactly.`,
  { label, phase: 'Models', model: 'haiku', effort: 'low', schema: MODEL_LIST },
)

// One Haiku driver runs one compiler step and the DeepSeek jobs it prints. It never reads the book
// or a Card; the step's JSON names every file, prompt, and conversation to continue.
const DRIVE = {
  type: 'object',
  properties: {
    ok: { type: 'boolean' }, error: { type: 'string' }, output: { type: 'string' },
    results: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, session_id: { type: 'string' }, status: { type: 'string', enum: ['done', 'failed'] }, answer: { type: 'string' } }, required: ['id', 'status'] } },
    archived: { type: 'number' },
  },
  required: ['ok', 'results'],
}
const drive = (label, o) => {
  const table = Object.entries(o.sessions || {}).map(([job, session]) => `${job} ${session}`).join('\n')
  return agent(
    `You drive DeepSeek Harness (DSH) jobs for the ZET model compiler. You do not read the book and you do not write Cards: a script and DSH do. Keep your messages short.
Load the tools with one ToolSearch call: query "${DRIVE_TOOLS}".

1. Run this one PowerShell command in the foreground with timeout 600000 and wait for it:
${o.command}
   Its last output line is compact JSON. Put that line, unchanged, in output. If the command fails, return ok=false with the error text and results=[]; do not retry it with other options. If the JSON has no "jobs" or they are empty, ${o.cleanup ? 'go to step 3' : 'return ok=true and results=[] now'}.
2. Run every job in "jobs". A job has id, files, prompt, and sometimes continue and fallback.
   - No continue: deepseek_run with cwd "${o.dsh}", prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills)}, model "${DSH_MODEL}", reasoning_effort "${o.effort}", wait_seconds 0.
   - continue: find the session of the job id named by continue in this table (job id, then session_id):
${table || '(empty)'}
     With a session: deepseek_prompt with that session_id, prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills)}, reasoning_effort "${o.effort}", wait_seconds 0. Without one: deepseek_run as above, with prompt_files = fallback.
   Keep at most ${DSH_PER_BATCH} jobs running at a time; start the next job when one ends. Make independent calls in one message.
   Follow "status" until each job ends:
   - "running": deepseek_wait (session_id, wait_seconds 600).
   - "needs_response": approve (decision "allow") a pwsh command that runs zt-mcp.ps1 or card-style-check.ps1; a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}; and writing the one file under "${o.dsh}" that the job's prompt names. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
   - "error", or "finished" without JSON lines: once, deepseek_prompt (same session, the same skills, wait_seconds 600) with "按 resume 行修正失败的调用后重跑，最后只回复 JSON 行。"; if it still fails, the job failed.
   - "finished": answer = its final answer text, unchanged (the JSON lines).
   ${o.cleanup ? 'Do not close conversations before step 3.' : 'Do not close or archive conversations: later rounds continue them.'}
${o.cleanup ? `3. Clean up: deepseek_list with cwd "${o.dsh}"; deepseek_close and deepseek_archive every conversation there that is not archived yet, and count them in archived; deepseek_remove_workspace with cwd "${o.dsh}"; then one PowerShell call: Remove-Item -LiteralPath "${o.dsh}" -Recurse -Force -ErrorAction SilentlyContinue
` : ''}Return ok, output, and results: one entry per job with id, session_id, status ("done" or "failed"), and answer.`,
    { label, phase: 'Models', model: 'haiku', effort: 'low', schema: DRIVE },
  )
}
// The JSON object in a line, also after prose on the same line ("…保留原样。{"address":…}"):
// DeepSeek does not always put its JSON on a line of its own.
const jsonIn = (line) => {
  for (let i = line.indexOf('{'); i >= 0; i = line.indexOf('{', i + 1)) {
    try { const v = JSON.parse(line.slice(i).trim()); if (v && typeof v === 'object') return v } catch {}
  }
  return null
}
// The compiler's JSON line: the last line holding a JSON object. A driver may wrap it in prose
// ("Report JSON: {…}") although it is told to pass the line unchanged.
const lastJson = (text) => {
  for (const line of String(text || '').trim().split('\n').reverse()) { const v = jsonIn(line); if (v) return v }
  return null
}
// The JSON lines of every answer, e.g. {"address","status":"split","need":3} or {"ok":2,"split":1}.
const answerLines = (driven) => ((driven && driven.results) || []).flatMap(r => String(r.answer || '').split('\n').map(jsonIn).filter(Boolean))

const PLANS = {
  type: 'object',
  properties: {
    plans: { type: 'array', items: { type: 'object', properties: {
      model: { type: 'string' }, status: { type: 'string', enum: ['planned', 'skipped', 'failed'] }, level: { type: 'string' },
      parts: { type: 'number' }, derivations: { type: 'number' }, chains: { type: 'number' },
      notes: { type: 'array', items: { type: 'string' } }, summary: { type: 'string' },
    }, required: ['model', 'status'] } },
  },
  required: ['plans'],
}
const slug = (address) => String(address).replace(/[\/|]/g, '_')
const managerFolders = []
let dshJobs = 0
const compileBatch = async (batch, kind) => {
  const name = `m-${slug(batch[0].address)}`
  const dsh = `${MODEL_WORK}\\${name}`
  managerFolders.push(dsh)
  const tag = `${kind} ${batch[0].address}${batch.length > 1 ? ` +${batch.length - 1}` : ''}`
  const step = (s, extra = '') => `pwsh -NoProfile -File "${COMPILER}" -Step ${s} -Work "${dsh}" -Staging "${OUT}" ${LIMITS}${extra}`
  const sessions = {}
  const run = async (what, command, skills, effort, opts = {}) => {
    const r = await drive(`${what}: ${tag}`, { command, dsh, skills, effort, sessions: opts.continues ? sessions : null, cleanup: opts.cleanup })
    for (const x of (r && r.results) || []) {
      dshJobs++
      if (x.session_id) sessions[x.id] = x.session_id
    }
    if (!r || !r.ok) log(`${what} ${tag}: ${(r && r.error) || 'no report from its driver'}`)
    return r
  }
  // A writer that cannot fit its part answers `split` with the number of Cards it needs.
  const splitNeeds = async (driven) => {
    const need = answerLines(driven).filter(l => l.status === 'split' && l.address && l.need > 1).map(l => `${l.address}=${l.need}`)
    if (need.length) await run('split', step('split', ` -Need "${need.join(';')}"`), FILL_SKILLS, DERIVE_EFFORT, { continues: true })
  }
  const failed = (why) => batch.map(m => ({ model: m.address, status: 'failed', parts: [], charts: 0, issues: [why] }))

  // 1. Brief: the script writes each context pack; DeepSeek reads the source and writes brief.md.
  const brief = await run('brief', step('brief', ` -Models "${batch.map(m => m.address).join(',')}"${REBUILD ? ' -Rebuild' : ''}${LEVEL ? ` -Level ${LEVEL}` : ''}`), [], BRIEF_EFFORT)
  const briefed = lastJson(brief && brief.output)
  if (!briefed) return failed(`brief failed: ${(brief && brief.error) || 'no output'}`)
  const skipped = new Set(briefed.skipped || [])
  const todo = batch.filter(m => !skipped.has(m.address))
  if (!todo.length) return batch.map(m => ({ model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }))

  // 2. Plan: Sonnet reads the briefs and writes plan.json for each model; nothing else.
  const entry = (m, i) => `## Model ${i + 1}
MODEL: ${m.address} (${m.title || ''})
DIR: ${dsh}\\${slug(m.address)}
LEVEL NOW: ${m.level || 'none'}`
  const planned = await agent(
    `You are the Sonnet architect in the Models stage of the ZET digest of 《${A.bookTitle || KEY}》. Read your instructions first: ${ARCH_DOC} (Read tool). Then follow them for each model in turn. These models are near each other in the book: where one part serves two of them, plan it in the first and take it in from the second (\`<model>#<id>\`).
COMPILER: ${COMPILER}
WORK: ${dsh}
LIMITS: ${LIMITS}${REBUILD ? '\nREBUILD: rebuild each model even if it already has parts.' : ''}${LEVEL ? `\nLEVEL: ${LEVEL}` : ''}

${todo.map(entry).join('\n\n')}

Return plans: one entry per model, in this order.`,
    { label: `plan: ${tag}`, phase: 'Models', model: 'sonnet', ...(A.architectEffort ? { effort: A.architectEffort } : {}), schema: PLANS },
  )
  const plans = (planned && planned.plans) || []
  if (!plans.some(p => p.status === 'planned')) {
    await run('clean up', step('report'), [], DSH_EFFORT, { cleanup: true })
    return batch.map(m => skipped.has(m.address) ? { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
      : { model: m.address, status: (plans.find(p => p.model === m.address) || {}).status || 'failed', parts: [], charts: 0, issues: planned ? [] : ['no plan from the architect'] })
  }

  // 3. Fill: the script builds every skeleton; DeepSeek writes the Model Card, setting, parts, charts.
  await splitNeeds(await run('fill', step('apply'), FILL_SKILLS, DSH_EFFORT))
  // 4. Derive and 5. review, only where the plans have Derivation parts.
  if (plans.some(p => p.status === 'planned' && p.derivations > 0)) {
    await splitNeeds(await run('derive', step('derive'), FILL_SKILLS, DERIVE_EFFORT))
    for (let round = 0; round < REVIEW_ROUNDS; round++) {
      const review = await run(`review ${round + 1}`, step('review', round ? ' -Changed' : ''), [], REVIEW_EFFORT)
      const verdicts = answerLines(review).reduce((s, l) => ({ split: s.split + (Number(l.split) || 0), fix: s.fix + (Number(l.fix) || 0) }), { split: 0, fix: 0 })
      log(`review ${round + 1} ${tag}: ${verdicts.split} parts to split, ${verdicts.fix} to fix`)
      if (!verdicts.split && !verdicts.fix) break
      await splitNeeds(await run(`split ${round + 1}`, step('split'), FILL_SKILLS, DERIVE_EFFORT, { continues: true }))
    }
  }
  // 6. Report (script) and clean-up.
  const reported = lastJson((await run('report', step('report'), [], DSH_EFFORT, { cleanup: true }) || {}).output)
  const reports = (reported && reported.models) || []
  return batch.map(m => {
    const r = reports.find(x => x.model === m.address)
    const p = plans.find(x => x.model === m.address) || {}
    if (skipped.has(m.address)) return { model: m.address, status: 'skipped', parts: [], charts: 0, issues: [] }
    if (!r) return { model: m.address, status: 'failed', parts: [], charts: 0, issues: ['no report from the compiler'] }
    const faults = Object.entries(r.faults || {}).filter(([, n]) => n).map(([k, n]) => `${k} ${n}`)
    return {
      ...r,
      issues: [...(r.issues || []), ...(r.error ? [r.error] : []), ...faults, ...((r.todo || []).length ? [`still 待写: ${r.todo.join(', ')}`] : [])],
      notes: [...(r.notes || []), ...(p.notes || [])], summary: p.summary,
    }
  })
}
const groupNo = (m) => Number(m.group) || Number(String(m.address).split('/')[1].split('|')[0]) || 0
const elaborateAll = async (models, label) => {
  const only = Array.isArray(A.only) && A.only.length ? new Set(A.only) : null
  // A model whose level the run changes is never skipped.
  const todo = models.filter(m => !only || only.has(m.address)).filter(m => REBUILD || (LEVEL && m.level !== LEVEL) || !(m.parts > 0 && !m.issues))
    .sort((a, b) => groupNo(a) - groupNo(b) || String(a.address).localeCompare(String(b.address)))
  if (todo.length < models.length) log(`${label}: ${models.length - todo.length} of ${models.length} models already have their parts; skipped.`)
  // Concept models are light (no Derivation round), so one batch takes more of them. A model with no
  // level yet goes with the formal ones: its architect decides.
  const concept = todo.filter(m => (LEVEL || m.level) === 'concept')
  const formal = todo.filter(m => (LEVEL || m.level) !== 'concept')
  const batches = []
  for (let i = 0; i < concept.length; i += PER_CONCEPT) batches.push({ kind: 'concept', models: concept.slice(i, i + PER_CONCEPT) })
  for (let i = 0; i < formal.length; i += PER_SONNET) batches.push({ kind: 'formal', models: formal.slice(i, i + PER_SONNET) })
  log(`${label}: ${todo.length} models (${concept.length} concept, ${formal.length} formal or undecided) in ${batches.length} batches, one Sonnet architect each; up to ${DSH_PER_BATCH} DeepSeek runs at a time per batch.`)
  const results = (await parallel(batches.map(b => () => compileBatch(b.models, b.kind).catch(error => { log(`batch ${b.models[0].address}: ${error}`); return null })))).flatMap((r, i) => r || batches[i].models.map(() => null))
  const ordered = batches.flatMap(b => b.models)
  return results.map((r, i) => r || { model: ordered[i].address, status: 'failed', parts: [], charts: 0, issues: ['no report from its batch'] })
}
// After a digest block: the Model Cards its DeepSeek runs wrote go to the model compiler.
const elaborateBlock = async (block, managed) => {
  const reports = (managed && managed.reports) || []
  const groups = block.map(g => g.n).filter(n => {
    const r = reports.find(x => x.group === n)
    return r && r.status === 'done' && r.models !== 0
  })
  if (!groups.length) return []
  const listed = await listModels(`-Groups ${groups.join(',')}`, `find models: groups ${groups[0]}-${groups[groups.length - 1]}`)
  if (!listed || !listed.ok) { log(`Models for groups ${groups.join(',')}: listing failed (${listed && listed.error})`); return [] }
  const reported = reports.filter(r => groups.includes(r.group)).reduce((s, r) => s + (r.models || 0), 0)
  if (reported && !listed.models.length) log(`Models for groups ${groups.join(',')}: the runs reported ${reported} Model Cards but the listing found none; the whole-book sweep will pick them up.`)
  return elaborateAll(listed.models, `groups ${groups[0]}-${groups[groups.length - 1]}`)
}

// A stage runs its blocks in parallel; with `after`, each block goes on to `after` as soon as its
// own manager returns (a pipeline, not a barrier), so later stages overlap slower blocks.
const runStage = async (stage, stageBlocks, after) => {
  const stages = [(block, _item, b) => manage(block, b, stage)]
  if (after) {
    stages.push((managed, block) => Promise.resolve(after(block, managed))
      .catch(error => { log(`${stage}: follow-up of groups ${block[0].n}-${block[block.length - 1].n} failed: ${error}`); return [] })
      .then(extra => ({ managed, extra })))
  }
  const results = await pipeline(stageBlocks, ...stages)
  const managed = results.map(r => (after ? r && r.managed : r))
  const extras = after ? results.flatMap(r => (r && r.extra) || []) : []
  const reports = []
  stageBlocks.forEach((block, b) => {
    const got = (managed[b] && managed[b].reports) || []
    for (const g of block) {
      reports.push(got.find(r => r.group === g.n) ||
        { group: g.n, status: 'failed', cards: 0, links: 0, topics: [], errors: 'no report from its manager', archived: false })
    }
  })
  const failed = reports.filter(r => r.status !== 'done').map(r => r.group)
  const calls = reports.map(r => r.tool_calls).filter(x => typeof x === 'number')
  log(`${stage}: ${reports.reduce((s, r) => s + (r.cards || 0), 0)} cards, ${reports.reduce((s, r) => s + (r.links || 0), 0)} links; tool calls per group ${calls.join(', ') || 'n/a'}; failed ${failed.join(', ') || 'none'}`)
  reports.extras = extras
  return reports
}

// How many relations of each kind the model agents wrote, e.g. "rests-on 12, illustrates 3".
const kindTally = (reports) => {
  const tally = {}
  for (const r of reports) for (const [kind, n] of Object.entries(r.kinds || {})) tally[kind] = (tally[kind] || 0) + n
  return Object.entries(tally).sort((a, b) => b[1] - a[1]).map(([kind, n]) => `${kind} ${n}`).join(', ') || 'no kinds reported'
}
const logModels = (reports) => {
  const done = reports.filter(r => r.status === 'done')
  const sum = (field) => done.reduce((s, r) => s + (r[field] || 0), 0)
  const deriv = (field) => done.reduce((s, r) => s + ((r.derivations && r.derivations[field]) || 0), 0)
  log(`Build: ${deriv('total')} Derivation parts in ${deriv('chains')} chains of two or more (longest ${done.reduce((m, r) => Math.max(m, (r.derivations && r.derivations.longest) || 0), 0)}), ${sum('split')} parts split by review or for the Card size, ${done.reduce((s, r) => s + ((r.moved || []).length), 0)} Cards moved into ranges, ${sum('shared')} shared parts taken in, ${sum('tolerated')} tolerated premises, ${sum('style')} style faults left, ${done.reduce((s, r) => s + ((r.todo || []).length), 0)} parts still 待写; ${dshJobs} DeepSeek jobs.`)
  log(`Levels: ${done.filter(r => r.level === 'concept').length} concept models (level 1), ${done.filter(r => r.level === 'formal').length} formal models (level 2), ${done.filter(r => !r.level).length} without a level.`)
  log(`Models: ${done.length} structured (${done.reduce((s, r) => s + r.parts.length, 0)} parts, ${done.reduce((s, r) => s + r.parts.filter(p => p.emphasis === 'core').length, 0)} core, ${sum('relations')} relations (${kindTally(done)}), ${sum('neighbours')} neighbours, ${sum('loose')} loose, ${sum('charts')} charts), ${reports.filter(r => r.status === 'skipped').length} skipped, failed ${reports.filter(r => r.status === 'failed').map(r => r.model).join(', ') || 'none'}; open issues ${reports.reduce((s, r) => s + r.issues.length, 0)}.`)
}

let digestReports = []
let modelReports = []
if (MODE === 'full') {
  phase('Digest')
  log(`${groupCount} groups in ${KEY}; ${blocks.length} Haiku managers, up to ${PER_MANAGER} DeepSeek runs each${MODELS ? '; each block\'s Model Cards go to the model compiler as soon as the block finishes' : ''}.`)
  digestReports = await runStage('digest', blocks, MODELS ? elaborateBlock : null)
  modelReports = digestReports.extras
  // Sweep the whole book before linking: a block whose listing failed or missed a Model Card
  // would otherwise leave it unelaborated without any error.
  if (MODELS) {
    const listed = await listModels(`-All -CitationKey ${KEY}`, 'find models: sweep')
    if (!listed || !listed.ok) log(`Model sweep: listing failed (${listed && listed.error})`)
    else {
      const covered = new Set(modelReports.filter(r => r && r.status === 'done').map(r => r.model))
      const missed = listed.models.filter(m => !covered.has(m.address))
      if (missed.length) modelReports = modelReports.concat(await elaborateAll(missed, 'sweep'))
    }
    logModels(modelReports)
  }
}
if (MODE === 'models') {
  phase('Models')
  const listed = await listModels(`-All -CitationKey ${KEY}`, 'find models: whole book')
  if (!listed || !listed.ok) return { stage: 'models', error: listed && listed.error }
  log(`${listed.models.length} Model Cards in the digest of ${KEY}.`)
  modelReports = await elaborateAll(listed.models, 'whole book')
  logModels(modelReports)
}
// A manager archives its own DeepSeek conversations; this sweep archives any a failed manager left open.
const modelCleanup = (MODELS || MODE === 'models') && managerFolders.length ? await agent(
  `Clean up after the batches of a ZET model stage (a batch archives its own conversations; this catches a batch that failed). Load tools with ToolSearch query "select:mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_archive,mcp__deepseek__deepseek_close,mcp__deepseek__deepseek_remove_workspace".
For each folder below: deepseek_list with that cwd; close and archive every conversation there that is not archived yet, and count them; then deepseek_remove_workspace with that cwd (it removes only the sidebar entry).
${managerFolders.join('\n')}
Then delete what is left of those folders with one PowerShell call: ${managerFolders.map(f => `Remove-Item -LiteralPath "${f}" -Recurse -Force -ErrorAction SilentlyContinue`).join('; ')}
Return late_archived (the count) and folders (how many folders you handled).`,
  {
    label: 'cleanup model managers', phase: 'Models', model: 'haiku', effort: 'low',
    schema: { type: 'object', properties: { late_archived: { type: 'number' }, folders: { type: 'number' } }, required: ['late_archived'] },
  },
) : null
if (modelCleanup) log(`Model clean-up: ${modelCleanup.late_archived} conversations archived late.`)
if (MODE === 'models' && !A.organize) return { mode: MODE, book: A.bookTitle, citation_key: KEY, models: modelReports, model_cleanup: modelCleanup }

// --- Link ----------------------------------------------------------------------------------------
if (MODE !== 'models') phase('Link')
const linkPrep = MODE === 'models' ? { ok: true, pending: [] } : await agent(
  `Prepare the cross-part link pass of a ZET book digest. Do exactly these steps.
1. Run this PowerShell command (one call): pwsh -NoProfile -File ${SCRIPTS}\\prepare-link-pass.ps1 -Staging "${OUT}"
   On a large book it takes several minutes. Run it in the foreground with timeout 600000 and wait for it; never in the background. If it fails because the ZT service is not up, run \`zt up\` once and run the command again. Its last line is JSON {"pending":[n…],"groups":[{"n","owned","done"}],"unowned","cards_all"}. If it fails, return ok=false with the error.
2. ${MODE === 'link' ? usageStep : 'Skip this step.'}
Return ok, pending (the "pending" array exactly), owned_total (sum of owned), unowned${MODE === 'link' ? ', usage_before' : ''}.`,
  {
    label: 'prepare link pass', phase: 'Link', model: 'haiku', effort: 'low',
    schema: {
      type: 'object',
      properties: {
        ok: { type: 'boolean' }, error: { type: 'string' }, pending: { type: 'array', items: { type: 'number' } },
        owned_total: { type: 'number' }, unowned: { type: 'number' }, usage_before: USAGE,
      },
      required: ['ok', 'pending'],
    },
  },
)
if (!linkPrep || !linkPrep.ok) {
  log(`Link preparation failed: ${linkPrep && linkPrep.error}`)
  return { stage: 'link-prepare', linkPrep, digestReports }
}
if (MODE === 'link') usageBefore = linkPrep.usage_before
// Groups whose anchor already states its place in the book are skipped, so a rerun links only the rest.
const pending = groups.filter(g => linkPrep.pending.includes(g.n))
if (MODE !== 'models') log(`Link pass: ${pending.length} of ${groupCount} groups pending; ${linkPrep.owned_total} owned Cards; ${linkPrep.unowned} unowned (hubs, structure Cards, Model parts).`)
const linkReports = pending.length ? await runStage('link', toBlocks(pending)) : []

// --- Cleanup -------------------------------------------------------------------------------------
if (MODE !== 'models') phase('Cleanup')
const cleanup = MODE === 'models' ? null : await agent(
  `Clean up after a DeepSeek Harness batch. Load tools with ToolSearch query "select:mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_archive,mcp__deepseek__deepseek_close,mcp__deepseek__deepseek_remove_workspace,mcp__deepseek__deepseek_usage".
1. deepseek_list with cwd "${OUT}". Close and archive every conversation there that is not archived yet; count them.
2. deepseek_remove_workspace with cwd "${OUT}" (removes only the sidebar entry; history is kept).
3. Delete the per-run scratch files with one PowerShell call and report the count it prints:
   $f = Get-ChildItem -LiteralPath "${OUT}" -File | Where-Object { $_.Name -like '*calls*.json' -or $_.Name -like 'snapshot-g*.md' -or $_.Name -like 'link-g*.md' -or $_.Name -like 'allow-g*.txt' }; $f | Remove-Item; $f.Count
   Delete nothing else (keep the book, prefixes, manifest, cards-all.md and owned-g*.txt).
4. deepseek_usage with days=1; sum each field over the rows for the last day listed.
Return late_archived, workspace_removed, files_removed, usage_after.`,
  {
    label: 'cleanup DeepSeek', phase: 'Cleanup', model: 'haiku', effort: 'low',
    schema: {
      type: 'object',
      properties: { late_archived: { type: 'number' }, workspace_removed: { type: 'boolean' }, files_removed: { type: 'number' }, usage_after: USAGE },
      required: ['late_archived', 'workspace_removed', 'files_removed'],
    },
  },
)

let dshUsage = null
if (usageBefore && cleanup && cleanup.usage_after) {
  const b = usageBefore, a = cleanup.usage_after
  const miss = a.input_cache_miss - b.input_cache_miss, hit = a.cached_read - b.cached_read
  const runs = Math.max(1, (MODE === 'full' ? groupCount : 0) + linkReports.length)
  dshUsage = {
    input_cache_miss: miss, cached_read: hit, output: a.output - b.output,
    calls: a.calls - b.calls, cost: Math.round((a.cost - b.cost) * 10000) / 10000,
    cache_hit_rate: miss + hit > 0 ? Math.round((hit / (miss + hit)) * 1000) / 1000 : null,
    per_run: { input: Math.round((miss + hit) / runs), output: Math.round((a.output - b.output) / runs), calls: Math.round(((a.calls - b.calls) / runs) * 10) / 10 },
  }
  log(`DeepSeek usage for this batch: ${JSON.stringify(dshUsage)}`)
}

// --- Organize ------------------------------------------------------------------------------------
let organize = null
if (MODE === 'full' || A.organize) {
  const modelLines = modelReports.filter(r => r.status === 'done').map(r => `- [[${r.model}]]${r.level === 'concept' ? '（概念模型）' : r.level === 'formal' ? '（形式模型）' : ''}：${r.parts.map(p => `${p.facet}${p.emphasis === 'core' ? '（核心）' : ''} [[${p.address}]]`).join('、')}${r.issues.length ? `；未解决：${r.issues.join('；')}` : ''}${r.notes && r.notes.length ? `；建议：${r.notes.join('；')}` : ''}`).join('\n')
  phase('Organize')
  const groupLines = (MODE === 'full' ? digestReports : linkReports).map(r => `- 第 ${r.group} 组 [[${KEY}/${r.group}]]：${r.status}${r.cards ? `，${r.cards} 张` : ''}${r.models ? `，模型卡 ${r.models}` : ''}${r.events ? `，事件卡 ${r.events}` : ''}${r.topics && r.topics.length ? `；新主题：${r.topics.join('、')}` : ''}${r.errors && r.errors !== '无' ? `；问题：${r.errors}` : ''}`).join('\n')
  organize = await agent(
    `You are the final coordinator of a ZET book digest. DeepSeek agents have digested 《${A.bookTitle}》 group by group into the agent Workspace of the ZET slip-box, then added reasoned cross-part Links to their own Cards. Your job is to organize the whole card structure. Use the zettelkasten and zt skills (load them with the Skill tool first). Make every zt call through the helper script, not the mcp__zt__* tools (the conversation's MCP server may be an older zt): write a JSON array of calls and run it in one PowerShell command, e.g.
@'
[{"tool":"list_trees","arguments":{"workspace":"agent"}},{"tool":"search_cards","arguments":{"type":"model","workspace":"agent"}}]
'@ | pwsh -NoProfile -File "${HELPER}" -
Each output line is "N <json>", "N ok <address>", or "N error <reason>" (then a resume line); "@@N" in an argument is the address returned by call N. Batch many reads or writes per command. delete_card and move_card return a plan first; call again with the confirm text. Write all Card text in Chinese. Change only the agent Workspace; human Cards are read-only but you may link to them.

Book: Literature Card [[${KEY}]] (agent Workspace, Literature Zone). Its Direct-successor line [[${KEY}/1]] … [[${KEY}/${groupCount}]] holds one anchor Card per group, in book order; each anchor holds that part's central claim, a section index, and its place in the whole book.
Group reports:
${groupLines}
${modelLines ? `Structured models (a Sonnet architect planned each model's parts, facets, core parts, and kinded relations, and DeepSeek wrote them with charts; act on the architect's notes for Cards outside the model and for other Model Cards that should link a part to share it):\n${modelLines}\n` : ''}
Work in this order, reading cheaply (list_trees, get_tree excerpts; get_card only when you need a body):
1. Survey every agent tree (list_trees with workspace "agent", then get_tree each).
2. Merge duplicate or overlapping Topics: keep the clearer Topic, move_card the other's first Card (with its subtree) to the best free slot in the kept tree (plan first, then confirm with "move"), then delete the emptied Topic root (plan, then confirm with its address). Fix clearly misplaced Cards the same way. Never delete a Card that still has content.
3. Rewrite every agent Topic body: 2–3 sentences on the question it pursues, then Links to its 1–3 best entry Cards.
4. For a Topic or subject with about seven or more Cards, write a hub Card as a Side successor of <n>/1 that lists Links to the core Cards with one line each on what they contribute, and Link to it from the Topic body.
5. Write the book's structure Card as a Side successor of [[${KEY}/1]]: title = the book's central thesis as one sentence; body = the argument in reading order, one line per part linking its anchor and its 1–2 key Topic Cards, each line saying what it contributes and how it follows from the previous part; then a paragraph listing the book's Model Cards (search_cards with type "model") in order, one line each on what the model shows, its level (概念模型 or 形式模型), and which later models build on it, and a paragraph listing its Event Cards (type "event") with the ideas each supports.
5a. Read each model's build report: run get_model on every Model Card and read \`level\` and \`build\`. Each Model Card has an elaboration level: \`concept\` (level 1: the Model Card and at most four parts in words, no Derivation parts) or \`formal\` (level 2: a self-sufficient model). A formal model is self-sufficient when \`build.closure\`, \`build.forward\`, and \`build.unanchored\` are empty; a concept model also needs an empty \`build.beyond_level\`; tolerated premises of shared parts are not faults. Do not change a model's level; report a level that looks wrong. Re-point each outside Card that a model manager named in its notes (a Card whose content a new part of the model now states): that Card links the new part, with its kind word. Check that each model with shared parts takes them in through its Model Card's 结构 list. Do not insert or move Cards inside a model's range.
5b. Check Model and Event Cards: each model the book sets out has exactly one Model Card (merge duplicates; set type "model" on a Card that states a whole model but is typed idea); run get_model on every Model Card and fix each reported issue (a part that belongs to no model: link it from its Model Card's 结构 list), but keep the structure the architect planned (its facets, core and aside parts, kinded relations, and loose parts). A part belongs to the Model Card it is filed under and to every Model Card that links it, so where two models of the book share a part (a correspondence, a common setting), link it from the second Model Card's 结构 list rather than writing a duplicate. Every Card that derives, interprets, applies, extends, or criticises a model links to its Model Card, or better to the Model part it concerns (a critique of an assumption links the part that states it), and the word right before the Link names the kind of relation, which ZT reads: 依赖/基于 (rests on), 例示/案例 (illustrates), 对照/区别 (contrasts), 放宽/推广 (relaxes), 对应/类比 (corresponds), 反驳/批评 (objects). Read each model's neighbours in get_model, rewrite those that read "refers" but state a real relation so the kind word stands right before the Link, then search for discussions that miss the model; every Card that uses a real-world event as evidence links to its Event Card.
6. Rewrite the Literature Card [[${KEY}]] body (edit_card body only) as the book's index: one line per chapter, "第N章 章名 - 讲了什么 → [[anchor]]", then a line linking the structure Card.
7. Check the cross-part Links the DeepSeek agents added: keep those whose sentence states a real relationship and reason; rewrite vague ones. Add Links still missing between Topics that share a mechanism, and to related human Cards (search_cards with workspace "human"). Remove a "# 标题" line that repeats the title at the top of a body.
8. Check list_broken_links and fix any in the agent Workspace.
Return a short Chinese report: the final agent Topics (address and name), merges and moves done, hubs created, the structure Card address, link counts added or rewritten, and anything left for the user.`,
    { label: 'organize card structure', phase: 'Organize', model: 'opus' },
  )
}

return {
  mode: MODE, book: A.bookTitle, citation_key: KEY, groups: groupCount, managers_per_pass: blocks.length,
  digest: digestReports.map(r => ({ group: r.group, status: r.status, cards: r.cards, tool_calls: r.tool_calls ?? null })),
  models: modelReports.map(r => ({ model: r.model, level: r.level, status: r.status, parts: r.parts.length, relations: r.relations, kinds: r.kinds, charts: r.charts, derivations: r.derivations, split: r.split, moved: r.moved, shared: r.shared, tolerated: r.tolerated, style: r.style, accepted: r.accepted, dsh: r.dsh, issues: r.issues })),
  model_cleanup: modelCleanup,
  link: linkReports.map(r => ({ group: r.group, status: r.status, cards_linked: r.cards, links: r.links, tool_calls: r.tool_calls ?? null, errors: r.errors })),
  deepseek_usage: dshUsage, cleanup, organize,
}
