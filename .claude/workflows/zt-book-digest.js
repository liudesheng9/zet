export const meta = {
  name: 'zt-book-digest',
  description: 'Digest a Chinese-translated book into ZET Cards: Haiku managers drive DeepSeek Harness runs that write idea, event, and Model Cards for their sections; as each block finishes, the model compiler builds every new model to its elaboration level: DeepSeek briefs the source, a Sonnet architect only plans each model (level, parts in build order, where Derivation parts go and how many), a script builds the skeleton, DeepSeek writes the Cards and charts, then the Derivation parts, and a DeepSeek reviewer splits any derivation that jumps or is too dense; a link pass ties the parts of the book together, Opus organizes the structure, attach-figures.ps1 shows the book\'s own figures (kept from the Scholar Archive) in every Card that names one and links the book\'s PDF and Markdown from its Literature Card, the hub stage builds the Hub Cards of the whole agent Workspace (a Haiku hub manager chooses the limits from a survey, DeepSeek plans the fields and writes every hub, hub-compiler.ps1 builds and checks them), and last digest-audit.ps1 writes a read-only audit of the book\'s Cards',
  whenToUse: 'Turn a Scholar Archive book (Chinese bilingual Markdown, digested as Chinese only) into an agent Literature tree plus Topic trees in the ZET slip-box; mode "link" re-runs only the cross-part link pass on an existing digest; mode "models" re-runs only the model stage on its Model Cards (with rebuild: true, also on models that already have parts); mode "hubs" runs only the hub stage on the whole agent Workspace (needs only out); mode "figures" only shows the book\'s figures in the Cards that name them and links its documents (needs citationKey and out, the digest\'s staging folder); mode "audit" only writes the read-only audit (audit.json, audit.md) of one book (citationKey) or of every book in the agent Workspace (no citationKey) into out.',
  phases: [
    { title: 'Prepare', detail: 'Chinese-only book text, section groups, agent Literature tree with anchors, event candidates (sentences with a year) in each group file; then the book term table: book-terms.ps1 lists the candidate terms, one DeepSeek job chooses the terms and the words not to use, the script writes terms.md and the Card 术语表', model: 'haiku' },
    { title: 'Digest', detail: 'each Haiku manager drives its block of DeepSeek runs; each run gets its own sections inline, searches the rest of the book on disk, and writes Cards for its sections, one whole Model Card per model', model: 'haiku' },
    { title: 'Models', detail: 'the model compiler, per batch of models as soon as a digest block finishes: DeepSeek writes a brief of each model\'s source; one Sonnet architect writes each plan.json (level, parts in build order, premises, where Derivation parts go and how many) and nothing else; model-compiler.ps1 builds the skeleton and the job files; Haiku drivers relay DeepSeek rounds: fill (Model Card, setting, parts, charts), derive (the Derivation parts), review (a reviewer splits a jumping or dense derivation into two or more parts, the script inserts them, the writer continues); the script reports each model and the driver archives its conversations', model: 'sonnet' },
    { title: 'Cards', detail: 'the card layer cycle (card-compiler.ps1): after each digest block, the script checks the block\'s Idea Cards, Event Cards, and anchor (Card language and the I, E, L rules), DeepSeek rewrites the Cards with faults and reviews a sample against the source as JSON files, the script checks each rewrite (every Link, figure, and formula kept) and writes it, checks again, and sends what is left back once; after the link pass, the same cycle checks the new Links (L1, L2) and links each orphan Card from a related Card; mode "cards" runs the whole cycle on an existing digest', model: 'haiku' },
    { title: 'Link', detail: 'each DeepSeek run gets only the Cards it owns, searches the book and the Card index on disk, and appends reasoned cross-part Links', model: 'haiku' },
    { title: 'Cleanup', detail: 'Haiku archives conversations, removes the DeepSeek workspace and scratch files', model: 'haiku' },
    { title: 'Organize', detail: 'organize-survey.ps1 lists what to decide (duplicate Topics, small Topics, merge proposals, model faults, notes); Opus writes only decisions.json (the thesis, Topic merges, moves, Card notes); organize-apply.ps1 carries out the merges and moves; a card cycle hands the notes to DeepSeek; book-skeleton.ps1 and one DeepSeek job write the Literature Card index, the structure Card, and the Topic bodies; organize "legacy" runs the old single Opus job', model: 'opus' },
    { title: 'Figures', detail: 'attach-figures.ps1 puts each book figure ZET keeps (![图 2.C.1](artifact:…)) into every Card of the book that names it but does not show it yet, and links the book\'s PDF and Markdown (scholar:<file_id>) from the Literature Card', model: 'haiku' },
    { title: 'Hubs', detail: 'the hub compiler on the whole agent Workspace: hub-compiler.ps1 surveys every Topic tree; a Haiku hub manager chooses the limits (when a tree or branch needs a hub, how large a hub may be) from the survey; Haiku drivers start DeepSeek jobs in the hub workspace: one field planner (fields and register terms) and topic writers (topic and line hubs as JSON); the script builds the Hub Cards, links them from Topic bodies and the register, checks every hub with get_hub, and sends faults back to the writer once; the last driver deletes the DeepSeek conversations', model: 'haiku' },
    { title: 'Audit', detail: 'digest-audit.ps1, read-only: per book, the Cards by category, the Topic trees, the Card language faults per rule (card-style-check.ps1), orphans, Cards with no source line or no kinded Link, models that are not self-sufficient, and figures named but not shown; for the Workspace, missing hubs and broken Links. It writes audit.json, audit.md, and audit-faults.txt in out', model: 'haiku' },
  ],
}

// args: { mode ('full' | 'link' | 'models' | 'hubs' | 'figures' | 'audit'), source, out, citationKey, bibtex, bookTitle,
//         fileId (the book Markdown's Scholar Archive file id; read from a source path such as
//         ...\scholar-archive\files\83\... when not given): its figures are kept in ZET and shown in the Cards,
//         figures (mode "full": true; false skips the Figures stage),
//         startHeading ('导论'), endHeading ('参考'): the Chinese headings where the digested text starts and stops,
//         skipHeadings (e.g. '参考文献,习题': headings whose units are left out, such as each chapter's references
//         and exercises; default none),
//         groups (modes 'link' and 'models': the existing group count), organize (modes 'link' and 'models': default false;
//         "legacy": the old single Opus job instead of survey, decisions, apply, notes, and skeleton),
//         perManager (5), maxChars (20000), maxUnits (3),
//         dshSource ('auto' | 'official' | 'opencode': where DeepSeek V4.1 Flash runs; 'auto' checks the DeepSeek
//         official source first and takes the OpenCode Go source when the official one does not answer),
//         dshModel (a DeepSeek Harness model that overrides dshSource), dshEffort ('high': the digest, link,
//         and fill runs; every DeepSeek effort is high or max, see atLeastHigh),
//         models (true: run the model stage), rebuild (false: also rebuild models that already have parts;
//         redoModels is the old name), perSonnet (6: formal models per batch, one Sonnet architect each;
//         neighbouring models of the book share a batch), perConcept (10: concept models per batch),
//         dshPerBatch (8: DeepSeek runs one Haiku manager keeps going at the same time; dshPerSonnet is the old name),
//         dshManagers (8: the most Haiku managers that run the DeepSeek jobs of one compiler step in parallel;
//         a step gets one manager per dshPerBatch jobs),
//         linkPerManager (8: the link runs one Haiku manager starts at once),
//         applyShare (30: the jobs one apply call of a card cycle writes; apply repeats until none is pending),
//         only (mode "models": the Model Card addresses to work on, e.g. a pilot; default all),
//         level ('concept' | 'formal': build every model of the run to this elaboration level; default each
//         model's own level, which DeepSeek proposes and the Sonnet architect confirms),
//         briefEffort ('high'), deriveEffort ('high'), reviewEffort ('high'): DeepSeek effort of the brief,
//         the Derivation parts and their follow-ups, and the review (dshEffort is the fill's),
//         reviewRounds (2: review, split, review again; 0 skips the review), architectEffort (Sonnet's effort),
//         maxSentences (12), maxPartChars (400), maxSteps (6): the Card size of a formal model's part;
//         a part over a limit splits, and a long derivation step becomes a chain of Derivation parts,
//         hubs (mode "full": true; mode "link": false): run the hub stage after the link pass,
//         hubWork ('D:\\zt-digest\\hubs': the hub stage's folder and DeepSeek workspace; its state.json
//         remembers each tree's hubs, so a later run rewrites only trees that changed),
//         hubLimits (an object that overrides the limits the Haiku hub manager chooses, e.g. { topic_min: 8 }),
//         hubTrees ('130,122': only these Topic trees, e.g. a pilot), hubRebuild (false: rewrite every hub),
//         hubEffort ('high': DeepSeek effort of the hub writers and the field planner; the user wants at least medium), hubFixRounds (1: check, fix, apply again),
//         terms (mode "full": true; false skips the book term table at Prepare),
//         audit (mode "full": true; false skips the Audit stage; read-only, see doc/digest-card-layers-proposal.md),
//         cardChecks (modes "full" and "link": true; false skips the card layer cycle), ideaSample (0.1: the share
//         of a block's Idea Cards without faults that DeepSeek reviews against the source; mode "cards": 0),
//         cardRules (the rules the cycle fixes; default all hard rules), cardFixRounds (1: check, fix, apply again),
//         cardMax (a pilot: at most this many Cards with faults and this many orphans) }
//
// How the stages hand work on:
//   Prepare (figures kept) ─▶ Digest block 1 (Haiku ▸ DeepSeek ×5) ─▶ Models for block 1 (model compiler, per batch) ─┐
//           ─▶ Digest block 2 (Haiku ▸ DeepSeek ×5) ─▶ Models for block 2 (model compiler, per batch) ─┼─▶ Link ─▶ Cleanup ─▶ Organize (Opus) ─▶ Figures ─▶ Hubs
//           ─▶ …                                                                                      ─┘
// Blocks run in parallel; a block's models are compiled while later blocks are still digesting.
//
// The model compiler, per batch (model-compiler.ps1 does every step marked "script"):
//   brief (script: context packs ▸ DeepSeek: brief.md) ─▶ plan (Sonnet: plan.json, validated by script)
//   ─▶ fill (script: skeleton, jobs ▸ DeepSeek: Model Card, setting, parts, charts)
//   ─▶ derive (script: jobs with filled premises ▸ DeepSeek: Derivation parts)
//   ─▶ review ×reviewRounds (script: one job per chain ▸ DeepSeek: verdicts ▸ script: splits ▸ DeepSeek: follow-ups)
//   ─▶ report (script) and clean-up. Haiku only relays DeepSeek jobs; Sonnet only plans.
//
// The hub compiler, once per run on the whole agent Workspace (hub-compiler.ps1 does every "script" step):
//   survey (script ▸ Haiku hub manager: the limits) ─▶ plan (script: jobs ▸ DeepSeek: fields.json and
//   hub-<tree>.json) ─▶ apply (script: Hub Cards, Topic bodies, register) ─▶ check (script: get_hub ▸
//   DeepSeek: fixes) ─▶ apply ─▶ report (script) and clean-up: the conversations are deleted, the
//   folder and its state.json stay for the next run.
const A =args && typeof args === 'object' ? args : {}
const MODE = A.mode || 'full'
// The args are an object, never free text: without these fields the stages would run on "undefined".
const missing = ['out', ...(MODE !== 'hubs' && MODE !== 'audit' ? ['citationKey'] : []), ...(MODE === 'full' ? ['source', 'bibtex', 'bookTitle'] : [])].filter(k => !A[k])
if (missing.length) return { stage: 'args', error: `args must be an object with ${missing.join(', ')}; got ${typeof args === 'string' ? 'a string' : JSON.stringify(args)}` }
const KEY = A.citationKey
const OUT = A.out
const DIGEST_PREFIX = `${OUT}\\shared-prefix.md`
const LINK_PREFIX = `${OUT}\\link-prefix.md`
const PER_MANAGER = A.perManager || 5
// DeepSeek V4.1 Flash at the two sources DeepSeek Harness offers (deepseek_models lists them). The
// official source is the DeepSeek platform ("DeepSeek-V41-Flash"); OpenCode Go serves the same model.
const DSH_SOURCES = {
  official: 'deepseek-official/deepseek-flash',
  opencode: 'opencode-go/deepseek-v4.1-flash',
}
const DSH_SOURCE = A.dshModel ? 'model' : (['official', 'opencode'].includes(A.dshSource) ? A.dshSource : 'auto')
// Set before the first DeepSeek run: the given model, the given source, or the source the probe found.
let DSH_MODEL = A.dshModel || DSH_SOURCES[DSH_SOURCE] || DSH_SOURCES.official
// The user wants every DeepSeek run at medium effort or more. V4.1 Flash offers off, low, high, and max at
// both sources, with no medium, so every effort is high or max: a lower value is raised to high.
const raised = []
const atLeastHigh = (value, name) => {
  if (value === 'high' || value === 'max') return value
  if (value) raised.push(`${name} ${value} → high`)
  return 'high'
}
const DSH_EFFORT = atLeastHigh(A.dshEffort, 'dshEffort')
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
const BRIEF_EFFORT = atLeastHigh(A.briefEffort, 'briefEffort')
const DERIVE_EFFORT = atLeastHigh(A.deriveEffort, 'deriveEffort')
const REVIEW_EFFORT = atLeastHigh(A.reviewEffort, 'reviewEffort')
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
const DRIVE_TOOLS = `${DSH_TOOLS},mcp__deepseek__deepseek_list,mcp__deepseek__deepseek_remove_workspace,mcp__deepseek__deepseek_delete`
// The hub stage: the Haiku hub manager chooses the limits; DeepSeek plans and writes; the script builds.
const HUBS = MODE === 'hubs' || (MODE === 'full' && A.hubs !== false) || (MODE === 'link' && A.hubs === true)
const HUB_WORK = A.hubWork || 'D:\\zt-digest\\hubs'
const HUB_COMPILER = `${SCRIPTS}\\hub-compiler.ps1`
const HUB_EFFORT = atLeastHigh(A.hubEffort, 'hubEffort')
const HUB_FIX_ROUNDS = Math.max(0, A.hubFixRounds ?? 1)
const HUB_SKILLS = ['asd-ste100-zh']
const AUDIT = MODE === 'audit' || (MODE === 'full' && A.audit !== false)
// The card layer cycle (doc/digest-card-layers-proposal.md, phases 3 and 4): the script checks and
// writes; DeepSeek rewrites and reviews as JSON files; Haiku drivers relay the jobs.
const CARD_CHECKS = MODE === 'cards' || ((MODE === 'full' || MODE === 'link') && A.cardChecks !== false)
const CARD_COMPILER = `${SCRIPTS}\\card-compiler.ps1`
const CARD_FIX_ROUNDS = Math.max(0, A.cardFixRounds ?? 1)
const IDEA_SAMPLE = Math.max(0, Math.min(1, A.ideaSample ?? (MODE === 'full' ? 0.1 : 0)))
const CARD_SKILLS = ['asd-ste100-zh']
// The jobs one apply call writes (A.applyShare): about 8 Cards each.
const APPLY_SHARE = Math.max(1, A.applyShare || 30)

// --- Audit: a read-only report on the book's Cards (phase 1 of doc/digest-card-layers-proposal.md) --
// A script, relayed by Haiku. It changes no Card. Mode "audit" runs only this stage.
async function runAudit() {
  phase('Audit')
  const run = await agent(
    `Run this PowerShell command once in the foreground with timeout 600000 and wait for it (it may take several minutes); do nothing else:
pwsh -NoProfile -File ${SCRIPTS}\\digest-audit.ps1${KEY ? ` -CitationKey ${KEY}` : ''} -Out "${OUT}"
Return ok (true when it exits without an error) and output = its last output line exactly (a compact JSON object), or the error text when it fails. Do not retry.`,
    { label: 'audit cards', phase: 'Audit', model: 'haiku', effort: 'low', schema: { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' } }, required: ['ok', 'output'] } },
  )
  const summary = run && run.ok ? lastJson(run.output) : null
  if (!summary) { log(`Audit failed: ${run && run.output}`); return { status: 'failed', error: run && run.output } }
  for (const b of summary.books || []) log(`Audit ${b.key}: ${b.cards} Cards in ${b.topic_trees} Topic trees; ${b.style_faults} Card language faults; ${b.layer_faults ?? 0} Card layer faults (${b.layer_advisory ?? 0} advisory); ${b.orphans} orphans; ${b.no_source} with no source; ${b.unkinded} with no kinded Link; ${b.not_self_sufficient} of ${b.models} models not self-sufficient; ${b.figures_missing} figures named but not shown.`)
  log(`Audit: ${summary.hubs_missing} missing hubs, ${summary.broken_links} broken Links in the Workspace. Report: ${OUT}\\audit.md`)
  return { status: 'done', ...summary }
}

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
if (raised.length) log(`DeepSeek efforts raised to high (the user wants at least medium; V4.1 Flash has no medium): ${raised.join(', ')}.`)
// The DeepSeek source: with dshSource 'auto', a Haiku probe asks each source for one short answer and
// the run uses the first that gives it, the official source before OpenCode Go.
if (DSH_SOURCE === 'auto' && MODE !== 'figures' && MODE !== 'audit') {
  const probeDir = `${OUT}\\dsh-probe`
  const probe = await agent(
    `You check which DeepSeek Harness source of DeepSeek V4.1 Flash answers now. Do exactly these steps; keep your messages short.
Load the tools with one ToolSearch call: query "select:mcp__deepseek__deepseek_run,mcp__deepseek__deepseek_wait,mcp__deepseek__deepseek_delete,mcp__deepseek__deepseek_remove_workspace".
1. One PowerShell call: New-Item -ItemType Directory -Force -Path "${probeDir}" | Out-Null
2. Try the sources in this order and stop at the first that answers:
   a. official: model "${DSH_SOURCES.official}"
   b. opencode: model "${DSH_SOURCES.opencode}"
   For each: deepseek_run with cwd "${probeDir}", model as given, reasoning_effort "high", prompt "只回复 ok", wait_seconds 180. If the status is "running", deepseek_wait once (wait_seconds 180). The source answers when the status is "finished" and the answer contains "ok". An error, a "running" status after the wait, or any other answer means it does not: record the error or status text for it. Then deepseek_delete that session_id.
3. deepseek_remove_workspace with cwd "${probeDir}" and delete_conversations true. Then one PowerShell call: Remove-Item -LiteralPath "${probeDir}" -Recurse -Force -ErrorAction SilentlyContinue
Return source ("official", "opencode", or "none") and errors (one short line per source that failed, else "").`,
    {
      label: 'choose DeepSeek source', phase: 'Prepare', model: 'haiku', effort: 'low',
      schema: { type: 'object', properties: { source: { type: 'string', enum: ['official', 'opencode', 'none'] }, errors: { type: 'string' } }, required: ['source'] },
    },
  )
  if (!probe || !DSH_SOURCES[probe.source]) {
    log(`No DeepSeek source answered: ${(probe && probe.errors) || 'no report from the probe'}`)
    return { stage: 'source', error: 'neither the DeepSeek official source nor OpenCode Go answered', probe }
  }
  DSH_MODEL = DSH_SOURCES[probe.source]
  log(`DeepSeek source: ${probe.source} (${DSH_MODEL})${probe.errors ? `; ${probe.errors}` : ''}.`)
} else {
  log(`DeepSeek source: ${DSH_SOURCE} (${DSH_MODEL}).`)
}
let groupCount = A.groups || 0
let usageBefore = null
if (MODE === 'full') {
  const prep = await agent(
    `You prepare a book for a ZET digest. Do exactly these steps and nothing else.
1. Run this PowerShell command (one call) and read its last output line, a compact JSON manifest:
pwsh -NoProfile -File ${SCRIPTS}\\prepare-book-digest.ps1 -Source "${A.source}" -Out "${OUT}" -CitationKey ${KEY} -BibTeX '${A.bibtex}' -BookTitle '${A.bookTitle}' -MaxChars ${A.maxChars || 20000} -MaxUnits ${A.maxUnits || 3}${A.startHeading ? ` -StartHeading '${A.startHeading}'` : ''}${A.endHeading ? ` -EndHeading '${A.endHeading}'` : ''}${A.skipHeadings ? ` -SkipHeadings '${A.skipHeadings}'` : ''}${A.fileId ? ` -FileId ${A.fileId}` : ''} -Apply
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
if (!groupCount && MODE !== 'models' && MODE !== 'hubs' && MODE !== 'figures' && MODE !== 'audit' && MODE !== 'cards') return { stage: 'prepare', error: 'mode "link" needs args.groups' }

const groups = Array.from({ length: groupCount }, (_, i) => {
  const n = i + 1
  const nn = String(n).padStart(2, '0')
  return { n, nn, file: `${OUT}\\groups\\g${nn}.md`, snapshot: `${OUT}\\snapshot-g${nn}.md`, link: `${OUT}\\link-g${nn}.md` }
})
// Contiguous blocks in book order: managers run in parallel on different chapters (few duplicate
// Topics), and inside a block each run starts after its predecessor and sees its new Cards.
const toBlocks = (list, size = PER_MANAGER) => {
  const out = []
  for (let i = 0; i < list.length; i += size) out.push(list.slice(i, i + size))
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
    parallel: true,
    steps: g => `Group ${g.n}:
  deepseek_run with exactly: cwd "${OUT}"; prompt_files ["${LINK_PREFIX}", "${g.link}"]; prompt "按以上说明完成你的组的连接，只回复一行 JSON。"; skills ${JSON.stringify(DSH_SKILLS)}; model "${DSH_MODEL}"; reasoning_effort "${DSH_EFFORT}"; wait_seconds 0.
  When it finished with status "done", mark it (one PowerShell call): Set-Content -LiteralPath "${OUT}\\linked-g${g.nn}.txt" -Value done`,
    answer: '{"group","cards_linked","links_added","errors"}',
    fields: 'cards (= cards_linked), links (= links_added)',
  },
}

const manage = (block, b, stage) => {
  const S = STAGES[stage]
  return agent(
    `You manage ${block.length} DeepSeek Harness (DSH) runs of a ZET book digest (${stage} pass), ${S.parallel ? 'all at the same time' : 'one after another'}. DSH reads the book and changes the Cards; you only start, supervise, and clean up each run. Keep your messages short.
Load all tools with one ToolSearch call: query "${DSH_TOOLS}".

${S.parallel ? 'Run the groups below at the same time: start every run in one message (all deepseek_run calls together), then follow each one until it ends. The runs do not depend on each other.' : 'For each group below, in order, finish it completely before starting the next:'}
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
// o: command, dsh (the DSH workspace), skills, effort, sessions (continued jobs), cleanup (the last
// round: end the conversations, remove the workspace), phase ('Models'), writes (a folder whose files
// a job may write; default: the one file its prompt names), remove ('archive' | 'delete': what cleanup
// does with the conversations), keep (true: cleanup keeps the folder, e.g. the hub stage's state).
//
// A step runs in up to three kinds of Haiku agent: one runs the script command; its DeepSeek jobs go
// to one manager per DSH_PER_BATCH (8) jobs, at most DSH_MANAGERS (8), and each manager keeps
// DSH_PER_BATCH jobs running at a time; one agent cleans up. The user set this rule: 1-8 jobs need one
// manager, 9-16 two, and so on; from 57 jobs on, 8 managers. A manager costs Haiku tokens, and one
// more manager does not make a DeepSeek job faster once each manager is full.
const DSH_MANAGERS = Math.max(1, A.dshManagers || 8)
const driveAgent = (label, o, part) => {
  const table = Object.entries(o.sessions || {}).map(([job, session]) => `${job} ${session}`).join('\n')
  const removal = o.remove === 'delete'
    ? `deepseek_list with cwd "${o.dsh}"; deepseek_delete every conversation listed there, and count them in archived; then deepseek_remove_workspace with cwd "${o.dsh}" and delete_conversations true`
    : `deepseek_list with cwd "${o.dsh}"; deepseek_close and deepseek_archive every conversation there that is not archived yet, and count them in archived; deepseek_remove_workspace with cwd "${o.dsh}"`
  const head = `You drive DeepSeek Harness (DSH) jobs for a ZET compiler script. You do not read the book and you do not write Cards: a script and DSH do. Keep your messages short.
Load the tools with one ToolSearch call: query "${DRIVE_TOOLS}".
`
  if (part === 'command') {
    return agent(`${head}
Run this one PowerShell command in the foreground with timeout 600000 and wait for it:
${o.command}
Its last output line is compact JSON; it may be long (tens of kilobytes). Put that line, unchanged and whole, in output: never shorten it, summarize it, or save it to a file instead. Do not run the jobs it lists: other managers run them. If the command fails, return ok=false with the error text; do not retry it with other options. Return ok, output, and results=[].`,
      { label, phase: o.phase || 'Models', model: 'haiku', effort: 'low', schema: DRIVE })
  }
  if (part === 'cleanup') {
    return agent(`${head}
Clean up: ${removal}${o.keep ? '. Keep the folder and its files.' : `; then one PowerShell call: Remove-Item -LiteralPath "${o.dsh}" -Recurse -Force -ErrorAction SilentlyContinue`}
Return ok=true, results=[], and archived.`,
      { label, phase: o.phase || 'Models', model: 'haiku', effort: 'low', schema: DRIVE })
  }
  const jobs = o.jobs || []
  return agent(`${head}
Run every job below. A job has id, files, prompt, and sometimes continue and fallback. Other managers run the other jobs of this step at the same time; run only these.
${JSON.stringify(jobs)}
- No continue: deepseek_run with cwd "${o.dsh}", prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills)}, model "${DSH_MODEL}", reasoning_effort "${o.effort}", wait_seconds 0.
- continue: find the session of the job id named by continue in this table (job id, then session_id):
${table || '(empty)'}
  With a session: deepseek_prompt with that session_id, prompt_files = files, prompt = prompt, skills ${JSON.stringify(o.skills)}, reasoning_effort "${o.effort}", wait_seconds 0. Without one: deepseek_run as above, with prompt_files = fallback.
Keep at most ${DSH_PER_BATCH} jobs running at a time; start the next job when one ends. Start independent jobs in one message.
Follow "status" until each job ends:
- "running": deepseek_wait (session_id, wait_seconds 600).
- "needs_response": approve (decision "allow") a pwsh command that runs zt-mcp.ps1 or card-style-check.ps1; a read-only read (Read, Get-Content, Select-String) of a file under "${OUT}"${A.source ? ` or of "${A.source}"` : ''}; and ${o.writes ? `writing JSON files under "${o.writes}"` : `writing the one file under "${o.dsh}" that the job's prompt names`}. Reject anything else that writes, moves, or deletes files. Answer a question with custom text "按任务说明自行决定，不需要询问。".
- "error", or "finished" without JSON lines: once, deepseek_prompt (same session, the same skills, wait_seconds 600) with "按 resume 行修正失败的调用后重跑，最后只回复 JSON 行。"; if it still fails, the job failed.
- "finished": answer = its final answer text, unchanged (the JSON lines).
Do not close or archive conversations: later rounds continue them, and a cleanup agent ends them.
Return ok=true, output="", and results: one entry per job with id, session_id, status ("done" or "failed"), and answer.`,
    { label, phase: o.phase || 'Models', model: 'haiku', effort: 'low', schema: DRIVE })
}
// A step's jobs. A compact list (card-compiler.ps1 -Compact) names each job's file and kind and gives
// the folder, the instruction files, and the prompts once; it expands to { id, files, prompt,
// continue?, fallback? } here, so the step agent echoes a short line even for hundreds of jobs.
const expandJobs = (parsed) => {
  const list = (parsed && Array.isArray(parsed.jobs)) ? parsed.jobs : []
  if (!parsed || !parsed.lead || !parsed.prompts) return list
  const at = (name) => `${parsed.dir}\\${name}`
  return list.map(j => {
    if (j.files) return j
    if (j.kind === 'follow') return { id: j.id, continue: j.continue, files: [at(j.file)], fallback: [parsed.lead.fix, at(j.file)], prompt: parsed.prompts.follow }
    const lead = j.kind === 'orphans' ? parsed.lead.orphans : parsed.lead.fix
    return { id: j.id, files: [lead, at(j.file)], prompt: parsed.prompts[j.kind] }
  })
}
// Runs one compiler step: the command, its jobs spread over up to DSH_MANAGERS managers, then the
// cleanup when o.cleanup. Returns { ok, error, output, results, archived } like one driver did.
const drive = async (label, o) => {
  const step = await driveAgent(label, o, 'command')
  if (!step || !step.ok) {
    if (o.cleanup) await driveAgent(`${label}: clean up`, o, 'cleanup')
    return { ok: false, error: (step && step.error) || 'no report from the step', output: (step && step.output) || '', results: [] }
  }
  const parsed = lastJson(step.output)
  const jobs = expandJobs(parsed)
  let results = []
  if (jobs.length) {
    const managers = Math.min(DSH_MANAGERS, Math.ceil(jobs.length / DSH_PER_BATCH))
    const shares = Array.from({ length: managers }, () => [])
    jobs.forEach((job, i) => shares[i % managers].push(job))
    const done = await parallel(shares.map((share, i) => () =>
      driveAgent(managers > 1 ? `${label} [${i + 1}/${managers}]` : label, { ...o, jobs: share }, 'jobs')
        .catch(error => ({ ok: false, error: String(error), results: [] }))))
    results = done.flatMap((d, i) => {
      const got = (d && d.results) || []
      // A job a manager did not report is a failed job, so a later round can retry it.
      const missing = shares[i].filter(j => !got.some(r => r.id === j.id)).map(j => ({ id: j.id, status: 'failed', answer: '' }))
      return [...got, ...missing]
    })
    const failedManagers = done.filter(d => !d || !d.ok).length
    if (failedManagers) log(`${label}: ${failedManagers} of ${managers} managers reported a failure.`)
  }
  let archived = 0
  if (o.cleanup) {
    const c = await driveAgent(`${label}: clean up`, o, 'cleanup')
    archived = (c && c.archived) || 0
  }
  return { ok: true, output: step.output, results, archived }
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

// --- Cards: the card layer cycle -----------------------------------------------------------------
// card-compiler.ps1 does every structural step; DeepSeek writes JSON files; Haiku drivers relay the
// jobs. work is the cycle's folder and DeepSeek workspace; it keeps state.json and report.json.
const runCards = async (label, work, extra = '', only = '') => {
  const cstep = (s, more = '') => `pwsh -NoProfile -File "${CARD_COMPILER}" -Step ${s} -Work "${work}" -Compact${more}`
  const sessions = {}
  let jobs = 0
  const crun = async (what, command, opts = {}) => {
    const r = await drive(`cards ${label}: ${what}`, {
      command, dsh: work, skills: CARD_SKILLS, effort: DSH_EFFORT, sessions: opts.continues ? sessions : null,
      cleanup: opts.cleanup, phase: 'Cards', writes: `${work}\\out`, remove: 'delete', keep: true,
    })
    for (const x of (r && r.results) || []) {
      jobs++
      if (x.session_id) sessions[x.id] = x.session_id
    }
    if (!r || !r.ok) log(`cards ${label} ${what}: ${(r && r.error) || 'no report from its driver'}`)
    return r
  }
  const rules = only ? ` -Rules "${only}"` : A.cardRules ? ` -Rules "${A.cardRules}"` : ''
  const max = A.cardMax ? ` -MaxCards ${A.cardMax}` : ''
  const planned = await crun('plan', cstep('plan', ` -Book ${KEY} -Staging "${OUT}"${rules}${max}${extra}`))
  const plan = lastJson(planned && planned.output)
  if (plan) plan.jobs = expandJobs(plan)
  if (!plan) return { status: 'failed', label, error: `plan failed: ${(planned && planned.error) || 'no output'}` }
  log(`Cards ${label}: ${plan.picked} Cards of the book; ${plan.with_faults} with ${plan.faults} faults, ${plan.review} for review, ${plan.orphans} orphans; ${plan.jobs.length} DeepSeek jobs.`)
  if (!plan.jobs.length) {
    const reported = lastJson(((await crun('report', cstep('report'), { cleanup: true })) || {}).output)
    return { status: 'done', label, plan, report: reported }
  }
  // Apply in shares of APPLY_SHARE jobs until none is pending: one call writes a few hundred Cards
  // at most, so it ends inside the step agent's 10-minute command limit (mascolell1995microeconomic:
  // 226 jobs in one call ran past it, and the agent's end stopped the write part way).
  const applyAll = async (what) => {
    let r = null
    for (let i = 0; i < 100; i++) {
      r = lastJson(((await crun(i ? `${what}.${i + 1}` : what, cstep('apply', ` -MaxJobs ${APPLY_SHARE}`))) || {}).output)
      if (!r || !r.pending_jobs) return r
    }
    return r
  }
  let applied = await applyAll('apply 1')
  for (let round = 1; round <= CARD_FIX_ROUNDS; round++) {
    const checked = lastJson(((await crun(`check ${round}`, cstep('check'), { continues: true })) || {}).output)
    log(`Cards ${label} check ${round}: ${checked ? `${checked.faults} faults left, ${checked.jobs.length} follow-up jobs` : 'no output'}.`)
    if (!checked || !checked.jobs.length) break
    applied = await applyAll(`apply ${round + 1}`)
  }
  const reported = lastJson(((await crun('report', cstep('report'), { cleanup: true })) || {}).output)
  if (!applied) log(`Cards ${label}: an apply step failed; the cycle is marked failed.`)
  if (reported) log(`Cards ${label}: faults ${reported.faults_before} → ${reported.faults_after} on ${reported.cards} Cards; verdicts ${JSON.stringify(reported.verdicts)}; ${reported.splits} splits, ${reported.merges} merge proposals, ${reported.refused} refused; ${reported.orphan_links} orphan Links (${reported.orphans_after} orphans left); ${jobs} DeepSeek jobs.`)
  return { status: reported && applied ? 'done' : 'failed', label, plan, applied, report: reported, ...(applied ? {} : { error: 'apply failed: no output from card-compiler.ps1 -Step apply' }) }
}
// After a digest block: its Idea Cards, Event Cards, and anchors (orphans wait for the link pass).
const cardBlock = async (block) => {
  const groups = block.map(g => g.n).join(',')
  return runCards(`groups ${block[0].n}-${block[block.length - 1].n}`, `${OUT}\\cards-g${String(block[0].n).padStart(2, '0')}`, ` -Groups "${groups}" -Sample ${IDEA_SAMPLE} -NoOrphans`)
}
if (MODE === 'cards') return { mode: MODE, dsh_model: DSH_MODEL, citation_key: KEY, cards: await runCards('whole book', `${OUT}\\cards`, IDEA_SAMPLE ? ` -Sample ${IDEA_SAMPLE}` : '') }

let digestReports = []
const cardReports = []
let modelReports = []
// The book term table (proposal 2): book-terms.ps1 finds the candidates, one DeepSeek job chooses the
// terms and the words not to use, the script writes terms.md and the Card 术语表 before any digest run.
const TERMS = MODE === 'full' && A.terms !== false
let termReport = null
if (TERMS) {
  const termWork = `${OUT}\\terms`
  const tstep = (s, more = '') => `pwsh -NoProfile -File "${SCRIPTS}\\book-terms.ps1" -Step ${s} -Staging "${OUT}"${more}`
  const opts = { dsh: termWork, skills: ['asd-ste100-zh'], effort: DSH_EFFORT, phase: 'Prepare', writes: `${termWork}\\out`, remove: 'delete', keep: true }
  const planned = await drive('terms: plan', { ...opts, command: tstep('plan') })
  const applied = await drive('terms: apply', { ...opts, command: tstep('apply', ' -Apply'), cleanup: true })
  termReport = lastJson(applied && applied.output)
  log(termReport && termReport.ok ? `Term table: ${termReport.terms} terms (${termReport.with_avoid} with words not to use) in ${termReport.card}.` : `Term table failed: ${(applied && applied.error) || (planned && planned.error) || 'no output'}; the digest goes on without it.`)
}
if (MODE === 'full') {
  phase('Digest')
  log(`${groupCount} groups in ${KEY}; ${blocks.length} Haiku managers, up to ${PER_MANAGER} DeepSeek runs each${MODELS ? '; each block\'s Model Cards go to the model compiler as soon as the block finishes' : ''}.`)
  // A block's models and its card cycle run side by side: the cycle never writes a Model Card or part.
  const afterBlock = (MODELS || CARD_CHECKS) ? async (block, managed) => {
    const [models, cards] = await Promise.all([
      MODELS ? elaborateBlock(block, managed) : [],
      CARD_CHECKS ? cardBlock(block).catch(error => ({ status: 'failed', error: String(error) })) : null,
    ])
    if (cards) cardReports.push(cards)
    return models
  } : null
  digestReports = await runStage('digest', blocks, afterBlock)
  modelReports = digestReports.extras
  // No group done means a host fault (zt not on PATH, DeepSeek Harness unable to start pwsh), not a
  // book fault: linking, organizing, and hubs on an empty book only spend tokens.
  if (!digestReports.some(r => r.status === 'done')) {
    const why = [...new Set(digestReports.map(r => r.errors).filter(Boolean))].slice(0, 3).join(' | ')
    log(`Digest: every group failed; the run stops before the link pass. ${why}`)
    return { mode: MODE, stage: 'digest', error: `every digest group failed: ${why}`, citation_key: KEY, digest: digestReports, terms: termReport, cards: { blocks: cardReports } }
  }
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
if (MODE === 'models' && !A.organize) return { mode: MODE, dsh_model: DSH_MODEL, book: A.bookTitle, citation_key: KEY, models: modelReports, model_cleanup: modelCleanup }

// --- Hubs: the hub compiler builds the Hub Cards of the whole agent Workspace ------------------------
// A Haiku hub manager reads the survey and chooses the limits; hub-compiler.ps1 does every structural
// step; DeepSeek plans the fields and writes each hub as JSON; Haiku drivers relay the DeepSeek jobs.
const HUB_LIMITS = {
  type: 'object',
  properties: {
    ok: { type: 'boolean' }, error: { type: 'string' }, survey: { type: 'string' },
    limits: { type: 'object', properties: {
      topic_min: { type: 'number' }, line_min: { type: 'number' }, max_lines: { type: 'number' }, max_groups: { type: 'number' },
      min_reason: { type: 'number' }, neighbour_links: { type: 'number' }, field_min: { type: 'number' }, field_max: { type: 'number' }, per_job: { type: 'number' },
    }, required: ['topic_min', 'line_min', 'max_lines', 'max_groups', 'min_reason', 'neighbour_links', 'field_min', 'field_max', 'per_job'] },
    reasons: { type: 'string' },
  },
  required: ['ok', 'limits'],
}
const runHubs = async () => {
  phase('Hubs')
  const hstep = (s, extra = '') => `pwsh -NoProfile -File "${HUB_COMPILER}" -Step ${s} -Work "${HUB_WORK}"${extra}`
  const manager = await agent(
    `You are the hub manager of a ZET slip-box. Hub Cards are the ways into a subject: a topic hub for each large Topic tree, a line hub for each large branch, field hubs over related Topics, and one register. A script builds them; DeepSeek writes them. You choose the limits that decide which hubs the policy requires and how large a hub may be. You read no Card.
1. Run this one PowerShell command in the foreground with timeout 600000 and wait for it:
${hstep('survey')}
   Its last output line is compact JSON: topics, tree_cards, branch_cards, and neighbour_links (each a distribution: n, min, p25, median, p75, p90, max), hubs (counts per level), legacy (Idea Cards used as hubs), register. Put that line, unchanged, in survey. If the command fails, return ok=false with the error text; do not retry with other options.
2. Choose the limits from the distributions with these rules, and round to whole numbers:
   - topic_min (a Topic tree with this many Cards needs a topic hub): tree_cards.p25, but at least 5 and at most 12. A tree smaller than most trees needs no map.
   - line_min (a branch with this many Cards needs a line hub): branch_cards.p90, but at least 6, at most 20, and never below topic_min.
   - max_lines (the most member lines of one hub): tree_cards.p75, but at least 15 and at most 25. Luhmann's collective reference cards held at most about 25 references.
   - max_groups: 7; 5 when max_lines is below 18.
   - min_reason (the fewest characters of a line's reason): 8.
   - neighbour_links (two trees with this many Links between them are neighbours): neighbour_links.median, but at least 2 and at most 6; 3 when n is 0.
   - field_min: 3. field_max (the most Topics in one field): the square root of topics, but at least 6 and at most 15.
   - per_job (Topic trees per DeepSeek writer job): 6 when tree_cards.median is 15 or less, 4 when it is 30 or less, else 2.
3. Return ok=true, survey, limits, and reasons: one short sentence per limit that names the number from the survey it came from.`,
    { label: 'hub manager: survey and limits', phase: 'Hubs', model: 'haiku', effort: 'low', schema: HUB_LIMITS },
  )
  if (!manager || !manager.ok) {
    log(`Hubs: the survey failed (${manager && manager.error}).`)
    return { status: 'failed', error: (manager && manager.error) || 'no report from the hub manager' }
  }
  const limits = { ...manager.limits, ...(A.hubLimits && typeof A.hubLimits === 'object' ? A.hubLimits : {}) }
  log(`Hub limits from the Haiku hub manager: ${JSON.stringify(limits)}. ${manager.reasons || ''}`)
  const sessions = {}
  let jobs = 0
  const hrun = async (what, command, opts = {}) => {
    const r = await drive(`hubs ${what}`, {
      command, dsh: HUB_WORK, skills: HUB_SKILLS, effort: HUB_EFFORT, sessions: opts.continues ? sessions : null,
      cleanup: opts.cleanup, phase: 'Hubs', writes: `${HUB_WORK}\\out`, remove: 'delete', keep: true,
    })
    for (const x of (r && r.results) || []) {
      jobs++
      if (x.session_id) sessions[x.id] = x.session_id
    }
    if (!r || !r.ok) log(`hubs ${what}: ${(r && r.error) || 'no report from its driver'}`)
    return r
  }
  const limitArg = JSON.stringify(limits).replace(/'/g, "''")
  const written = await hrun('write', hstep('plan', ` -Limits '${limitArg}'${A.hubTrees ? ` -Trees "${A.hubTrees}"` : ''}${A.hubRebuild ? ' -Rebuild' : ''}`))
  const plan = lastJson(written && written.output)
  if (!plan) return { status: 'failed', limits, error: `plan failed: ${(written && written.error) || 'no output'}` }
  const failedJobs = ((written && written.results) || []).filter(x => x.status !== 'done').map(x => x.id)
  log(`Hubs: ${plan.tree_count ?? (plan.trees || []).length} Topic trees to write in ${plan.jobs.length} DeepSeek jobs (one is the field planner)${failedJobs.length ? `; failed jobs ${failedJobs.join(', ')}` : ''}.`)
  let applied = lastJson(((await hrun('apply 1', hstep('apply'))) || {}).output)
  let checked = null
  for (let round = 1; round <= HUB_FIX_ROUNDS; round++) {
    checked = lastJson(((await hrun(`check ${round}`, hstep('check'), { continues: true })) || {}).output)
    log(`Hubs check ${round}: ${checked ? `${checked.hubs} hubs, ${checked.faults} faults, ${checked.jobs.length} fix jobs` : 'no output'}.`)
    if (!checked || !checked.jobs.length) break
    applied = lastJson(((await hrun(`apply ${round + 1}`, hstep('apply'))) || {}).output)
  }
  const reported = lastJson(((await hrun('report', hstep('report'), { cleanup: true })) || {}).output)
  log(`Hubs: ${JSON.stringify((reported && reported.counts) || {})}; faults ${JSON.stringify((reported && reported.faults) || {})}; ${reported ? reported.mirror : '?'} mirror hubs; ${reported ? reported.missing_count : '?'} required hubs still missing; ${jobs} DeepSeek jobs.`)
  // A crashed apply writes no hub of this run (acemoglu2000franchise: Topics 197 and 199 got none).
  if (!applied) log('Hubs: an apply step failed; the stage is marked failed.')
  return { status: reported && applied ? 'done' : 'failed', limits, reasons: manager.reasons, trees: plan.tree_count ?? (plan.trees || []).length, applied, report: reported, ...(applied ? {} : { error: 'apply failed: no output from hub-compiler.ps1 -Step apply' }) }
}
// --- Figures -------------------------------------------------------------------------------------
// The book's figures in its Cards: a script, relayed by Haiku. The digest and model writers paste the
// figures they discuss; this puts every figure a Card names but does not show yet into that Card, and
// links the book's documents from the Literature Card. It runs after Organize, which rewrites bodies.
const FIGURES = MODE === 'figures' || (MODE === 'full' && A.figures !== false)
async function runFigures() {
  phase('Figures')
  const run = await agent(
    `Run this PowerShell command once and wait for it (it may take several minutes); do nothing else:
pwsh -NoProfile -File ${SCRIPTS}\\attach-figures.ps1 -CitationKey ${KEY} -Staging "${OUT}"${A.fileId ? ` -FileId ${A.fileId}` : ''} -Report "${OUT}\\figures-report.json" -Apply
Return ok (true when it exits without an error) and output = its last output line exactly (a compact JSON object), or the error text when it fails. Do not retry.`,
    { label: 'show figures', phase: 'Figures', model: 'haiku', effort: 'low', schema: { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' } }, required: ['ok', 'output'] } },
  )
  const summary = run && run.ok ? lastJson(run.output) : null
  log(summary ? `Figures: ${summary.shown} figures shown in ${summary.cards} Cards (${summary.figures} kept, ${summary.labelled} labelled); documents ${summary.literature ? 'linked' : 'already linked'}.` : `Figures failed: ${run && run.output}`)
  return summary ? { status: 'done', ...summary } : { status: 'failed', error: run && run.output }
}
if (MODE === 'figures') return { mode: MODE, citation_key: KEY, figures: await runFigures() }
if (MODE === 'audit') return { mode: MODE, citation_key: KEY || null, audit: await runAudit() }

let hubReport = null
if (MODE === 'hubs') {
  hubReport = await runHubs()
  if (!A.organize) return { mode: MODE, dsh_model: DSH_MODEL, hubs: hubReport }
}

const NO_LINK = MODE === 'models' || MODE === 'hubs'
// --- Link ----------------------------------------------------------------------------------------
if (!NO_LINK) phase('Link')
const linkPrep = NO_LINK ? { ok: true, pending: [] } : await agent(
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
if (!NO_LINK) log(`Link pass: ${pending.length} of ${groupCount} groups pending; ${linkPrep.owned_total} owned Cards; ${linkPrep.unowned} unowned (hubs, structure Cards, Model parts).`)
// A link run depends on no other: each Haiku manager starts LINK_PER_MANAGER runs at once.
const LINK_PER_MANAGER = Math.max(PER_MANAGER, A.linkPerManager || 8)
const linkReports = pending.length ? await runStage('link', toBlocks(pending, LINK_PER_MANAGER)) : []
// The Link check and the orphan pass on the whole book (proposal 6). In mode "full" the block cycles
// have already fixed the book, so the check runs every hard rule: the link pass can add R2, R4, and
// I2 faults to the Cards it extends. On an older book (mode "link") it keeps to L1 and L2.
const linkCards = !NO_LINK && CARD_CHECKS ? await runCards('link check', `${OUT}\\cards-link`, '', MODE === 'full' ? '' : (A.cardRules || 'L1,L2')) : null

// --- Cleanup -------------------------------------------------------------------------------------
if (!NO_LINK) phase('Cleanup')
const cleanup = NO_LINK ? null : await agent(
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
  const ORG = `${OUT}\\organize`
  if (A.organize === 'legacy') {
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
  3. Rewrite every agent Topic body: 2–3 sentences on the question it pursues, then Links to its 1–3 best entry Cards. Keep a line that links a Hub Card.
  4. Do not write, rewrite, or retype Hub Cards (type hub), and do not write an Idea Card that lists a subject's Cards: the hub stage runs after you and builds every topic, line, and field hub and the register from the trees you leave. When you merge two Topics, leave their Hub Cards where they are; the hub stage merges them.
  5. Write the book's structure Card as a Side successor of [[${KEY}/1]]: title = the book's central thesis as one sentence; body = the argument in reading order, one line per part linking its anchor and its 1–2 key Topic Cards, each line saying what it contributes and how it follows from the previous part; then a paragraph listing the book's Model Cards (search_cards with type "model") in order, one line each on what the model shows, its level (概念模型 or 形式模型), and which later models build on it, and a paragraph listing its Event Cards (type "event") with the ideas each supports.
  5a. Read each model's build report: run get_model on every Model Card and read \`level\` and \`build\`. Each Model Card has an elaboration level: \`concept\` (level 1: the Model Card and at most four parts in words, no Derivation parts) or \`formal\` (level 2: a self-sufficient model). A formal model is self-sufficient when \`build.closure\`, \`build.forward\`, and \`build.unanchored\` are empty; a concept model also needs an empty \`build.beyond_level\`; tolerated premises of shared parts are not faults. Do not change a model's level; report a level that looks wrong. Re-point each outside Card that a model manager named in its notes (a Card whose content a new part of the model now states): that Card links the new part, with its kind word. Check that each model with shared parts takes them in through its Model Card's 结构 list. Do not insert or move Cards inside a model's range.
  5b. Check Model and Event Cards: each model the book sets out has exactly one Model Card (merge duplicates; set type "model" on a Card that states a whole model but is typed idea); run get_model on every Model Card and fix each reported issue (a part that belongs to no model: link it from its Model Card's 结构 list), but keep the structure the architect planned (its facets, core and aside parts, kinded relations, and loose parts). A part belongs to the Model Card it is filed under and to every Model Card that links it, so where two models of the book share a part (a correspondence, a common setting), link it from the second Model Card's 结构 list rather than writing a duplicate. Every Card that derives, interprets, applies, extends, or criticises a model links to its Model Card, or better to the Model part it concerns (a critique of an assumption links the part that states it), and the word right before the Link names the kind of relation, which ZT reads: 依赖/基于 (rests on), 例示/案例 (illustrates), 对照/区别 (contrasts), 放宽/推广 (relaxes), 对应/类比 (corresponds), 反驳/批评 (objects). Read each model's neighbours in get_model, rewrite those that read "refers" but state a real relation so the kind word stands right before the Link, then search for discussions that miss the model; every Card that uses a real-world event as evidence links to its Event Card.
  6. Rewrite the Literature Card [[${KEY}]] body (edit_card body only) as the book's index: one line per chapter, "第N章 章名 - 讲了什么 → [[anchor]]", then a line linking the structure Card. Keep its last line "原书文档：…" (Document links such as [原书 PDF](scholar:85) that open the book's PDF and Markdown) exactly as it is.
  6a. A line \`![图 2.C.1](artifact:<sha256>)\` in a Card shows the book's own figure; when you rewrite or merge a Card, keep each such line exactly, next to the text that discusses the figure.
  7. Check the cross-part Links the DeepSeek agents added: keep those whose sentence states a real relationship and reason; rewrite vague ones. Add Links still missing between Topics that share a mechanism, and to related human Cards (search_cards with workspace "human"). Remove a "# 标题" line that repeats the title at the top of a body.
  8. Check list_broken_links and fix any in the agent Workspace.
  Return a short Chinese report: the final agent Topics (address and name), merges and moves done, the structure Card address, link counts added or rewritten, and anything left for the user.`,
      { label: 'organize card structure', phase: 'Organize', model: 'opus' },
    )
  } else {
    // 1. The survey: a script lists what Opus must decide (doc/digest-card-layers-proposal.md, proposal 8).
    const surveyed = await agent(
      `Run this PowerShell command once in the foreground with timeout 600000 and wait for it; do nothing else:
pwsh -NoProfile -File "${SCRIPTS}\\organize-survey.ps1" -Staging "${OUT}" -Work "${ORG}"
Return ok (true when it exits without an error) and output = its last output line exactly, or the error text. Do not retry.`,
      { label: 'organize: survey', phase: 'Organize', model: 'haiku', effort: 'low', schema: { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' } }, required: ['ok', 'output'] } },
    )
    const survey = surveyed && surveyed.ok ? lastJson(surveyed.output) : null
    if (!survey) log(`Organize: the survey failed (${surveyed && surveyed.output}).`)
    // 2. Opus decides; it writes decisions.json and nothing else.
    const decided = survey ? await agent(
      `You decide the structure of the ZET digest of 《${A.bookTitle}》 (Literature Card [[${KEY}]], anchors [[${KEY}/1]] … [[${KEY}/${groupCount}]]). Scripts and DeepSeek do every edit; you only decide. You make no zt write.
Read ${ORG}\\survey.md first (Read tool). It lists the book's Topics, the duplicate candidates, small Topics, Card merge proposals, the models with faults, broken Links, and missing hubs. When a decision needs a Card's text, read it through the helper (read-only calls only: get_card, get_tree, get_model, search_cards), e.g.
@'
[{"tool":"get_tree","arguments":{"tree":"62"}},{"tool":"get_card","arguments":{"at":"62/1"}}]
'@ | pwsh -NoProfile -File "${HELPER}" -
${modelLines ? `The architect's notes on the book's models (Cards outside a model that should link a part of it):\n${modelLines}\n` : ''}
Decide, then write ${ORG}\\decisions.json (UTF-8, Write tool) with exactly these fields:
- "thesis": the book's central thesis as one Chinese sentence of at most 40 characters. It becomes the title of the structure Card.
- "merges": Topic merges, [{"drop": "<n>/0", "keep": "<m>/0", "under": "<a Card of the kept Topic>", "reason": "…"}]. Merge only two agent Topics that pursue the same question. "under" is the Card under which the dropped Topic's Cards fit best (default: the kept Topic's first Card).
- "moves": Cards that sit in the wrong place, [{"at": "<Card>", "under": "<new parent>", "reason": "…"}]. Only Idea and Event Cards of Topic trees; never a Model Card, a Model part, a Hub Card, or a Literature tree Card.
- "notes": Card-level work for DeepSeek, [{"address": "<Card>", "note": "<one Chinese sentence: what to change>"}]: a Card that should link a model part (the architect's notes), a Card whose Link reads as a plain reference but states a real relation, a Card that should link an Event Card. Only Idea Cards, Event Cards, and anchors.
Use empty lists where nothing needs doing. Do not plan Hub Cards: the hub stage runs after you. Do not rewrite the Literature Card, the structure Card, or Topic bodies: a script and DeepSeek write them after your decisions.
Return thesis, merges, moves, notes (the counts), and report: a short Chinese report of what you decided and what you leave for the user.`,
      {
        label: 'organize: decisions', phase: 'Organize', model: 'opus',
        schema: { type: 'object', properties: { thesis: { type: 'string' }, merges: { type: 'number' }, moves: { type: 'number' }, notes: { type: 'number' }, report: { type: 'string' } }, required: ['thesis', 'report'] },
      },
    ) : null
    // 3. A script carries out the merges and moves.
    const applied = decided ? await agent(
      `Run this PowerShell command once in the foreground with timeout 600000 and wait for it; do nothing else:
pwsh -NoProfile -File "${SCRIPTS}\\organize-apply.ps1" -Work "${ORG}"
Return ok and output = its last output line exactly, or the error text. Do not retry.`,
      { label: 'organize: apply', phase: 'Organize', model: 'haiku', effort: 'low', schema: { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' } }, required: ['ok', 'output'] } },
    ) : null
    const moved = applied && applied.ok ? lastJson(applied.output) : null
    if (decided) log(`Organize: thesis "${decided.thesis}"; ${moved ? `${moved.merges} merges, ${moved.moves} moves, ${moved.refused} refused` : `apply failed (${applied && applied.output})`}; ${decided.notes || 0} notes.`)
    // 4. The notes go to DeepSeek as a card cycle. In mode "full" it checks every hard rule (the script
    // adds N1), so a note that makes a Card too long or adds a bare Link is fixed in the same cycle;
    // on an older book it keeps to N1.
    const notes = decided && decided.notes ? await runCards('organize notes', `${OUT}\\cards-notes`, ` -Notes "${ORG}\\decisions.json" -NoOrphans`, MODE === 'full' ? '' : 'N1') : null
    // 5. The skeleton: the Literature Card index, the structure Card, the Topic bodies.
    const SK = `${OUT}\\skeleton`
    const kstep = (s, more = '') => `pwsh -NoProfile -File "${SCRIPTS}\\book-skeleton.ps1" -Step ${s} -Staging "${OUT}" -Work "${SK}"${more}`
    const kopts = { dsh: SK, skills: ['asd-ste100-zh'], effort: DSH_EFFORT, phase: 'Organize', writes: `${SK}\\out`, remove: 'delete', keep: true }
    await drive('organize: skeleton', { ...kopts, command: kstep('plan') })
    const sk = lastJson(((await drive('organize: skeleton apply', { ...kopts, command: kstep('apply', ` -Decisions "${ORG}\\decisions.json" -Apply`), cleanup: true })) || {}).output)
    log(sk && sk.ok ? `Organize: structure Card ${sk.written && sk.written.structure} ("${sk.thesis}"), ${sk.argument} argument lines, ${sk.models} models, ${sk.events} events, ${sk.topics} Topic bodies.` : `Organize: the skeleton failed: ${JSON.stringify((sk && sk.problems) || 'no output')}`)
    organize = { survey, decisions: decided, applied: moved, notes: notes && notes.report, skeleton: sk }
  }
}
const figures = FIGURES && MODE !== 'figures' ? await runFigures() : null
// The hub stage runs last, on the trees as Organize left them.
if (HUBS && MODE !== 'hubs') hubReport = await runHubs()
// The audit runs last, on the Cards as every other stage left them.
const audit = AUDIT ? await runAudit() : null

return {
  mode: MODE, dsh_model: DSH_MODEL, book: A.bookTitle, citation_key: KEY, groups: groupCount, managers_per_pass: blocks.length,
  digest: digestReports.map(r => ({ group: r.group, status: r.status, cards: r.cards, tool_calls: r.tool_calls ?? null })),
  models: modelReports.map(r => ({ model: r.model, level: r.level, status: r.status, parts: r.parts.length, relations: r.relations, kinds: r.kinds, charts: r.charts, derivations: r.derivations, split: r.split, moved: r.moved, shared: r.shared, tolerated: r.tolerated, style: r.style, accepted: r.accepted, dsh: r.dsh, issues: r.issues })),
  model_cleanup: modelCleanup,
  link: linkReports.map(r => ({ group: r.group, status: r.status, cards_linked: r.cards, links: r.links, tool_calls: r.tool_calls ?? null, errors: r.errors })),
  terms: termReport, cards: { blocks: cardReports, link: linkCards },
  deepseek_usage: dshUsage, cleanup, organize, figures, hubs: hubReport, audit,
}
