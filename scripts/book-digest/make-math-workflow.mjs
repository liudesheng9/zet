// Run: node scripts/book-digest/make-math-workflow.mjs
// Writes .claude/workflows/zt-math-digest.js from .claude/workflows/zt-book-digest.js. The math
// workflow is the book digest with math stages added: every shared stage (the DeepSeek Subagent
// Managed Protocol, the digest and link managers, the model compiler, the card cycles, the link
// check, Organize, Figures, Hubs, and the Audit) is the book digest's own text, so its standards
// stay identical. Rerun this after any change to zt-book-digest.js; the harness
// (tests/math-workflow-harness.mjs) fails when the two drift apart.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const workflows = path.join(here, '..', '..', '.claude', 'workflows')
let s = fs.readFileSync(path.join(workflows, 'zt-book-digest.js'), 'utf8')
const crlf = s.includes('\r\n')
s = s.replace(/\r\n/g, '\n')
const edits = []
const rep = (what, a, b) => {
  const n = s.split(a).length - 1
  if (n !== 1) throw new Error(`${what}: expected one match, found ${n}`)
  s = s.split(a).join(b)
  edits.push(what)
}
const between = (what, start, end, text) => {
  const i = s.indexOf(start)
  const j = s.indexOf(end, i)
  if (i < 0 || j < 0) throw new Error(`${what}: markers not found`)
  s = s.slice(0, i) + text + s.slice(j)
  edits.push(what)
}

// 1. The meta block: name, description, and the math phases.
between('meta', 'export const meta = {', '// args:', `export const meta = {
  name: 'zt-math-digest',
  description: 'Digest a Chinese-translated mathematics textbook into ZET Cards (docs/math-cards-design.md): the zt-book-digest workflow with the same Card, Link, hub, and DeepSeek standards, plus mathematical units. A script finds every numbered definition, axiom, theorem, proposition, lemma, and corollary, builds a unit Model Card (its Math form and level) and a Theory Card per chapter, and a DeepSeek job writes the Book notation table; the digest writes each unit\\'s statement; the model compiler builds each unit with the math supplements (premises, proof plan, proof steps, Derivation parts, intuition, examples) under the attention budget; a boundary pass writes hypothesis roles, counterexamples, and contrast pairs, and a second DeepSeek job checks each supplied one; a proof review checks every proof step; after the link pass a theory pass writes each Theory Card (question, main results, objects, dependency order, tools, map); and a math check reports rules M1 to M18 beside the audit',
  whenToUse: 'Turn a Scholar Archive mathematics textbook (Chinese bilingual Markdown with numbered definitions and theorems) into an agent Literature tree of mathematical units and Theory Cards; mode "math" re-runs only the boundary, proof, theory, and check stages on an existing math digest; the other modes are those of zt-book-digest.',
  phases: [
    { title: 'Prepare', detail: 'Chinese-only book text, section groups, anchors; then math-compiler.ps1 finds every numbered statement (index), writes the math prompt files (docs: the shared instructions with the math supplements appended), and builds the skeleton (a unit Model Card per definition, axiom, and result, a Theory Card per chapter); one DeepSeek job writes the Book notation table; then the book term table as in zt-book-digest', model: 'haiku' },
    { title: 'Digest', detail: 'each Haiku manager drives its block of DeepSeek runs with the math prefix: each run writes its units\\' statements and its Idea, Event, and Model Cards', model: 'haiku' },
    { title: 'Models', detail: 'the model compiler builds every unit and model; the architect reads model-architect.md and model-architect-math.md, and the briefs, fills, and reviews use the math supplements', model: 'sonnet' },
    { title: 'Cards', detail: 'the card layer cycle of zt-book-digest, unchanged', model: 'haiku' },
    { title: 'Boundary', detail: 'math-compiler.ps1 boundary: DeepSeek writes hypothesis roles with counterexamples and contrast pairs as JSON; the script makes the parts; a second DeepSeek job checks each supplied one; the script deletes what it judged wrong', model: 'haiku' },
    { title: 'Proof', detail: 'math-compiler.ps1 proof: a DeepSeek reviewer checks every proof plan, proof step, and conclusion; the script writes each fix that keeps its Links', model: 'haiku' },
    { title: 'Link', detail: 'the link pass of zt-book-digest with the math link supplement', model: 'haiku' },
    { title: 'Theory', detail: 'math-compiler.ps1 theory: DeepSeek writes each Theory Card\\'s title, question, main results, and objects; the script writes the dependency order, tools, users, and the dependency map', model: 'haiku' },
    { title: 'Cleanup', detail: 'as in zt-book-digest', model: 'haiku' },
    { title: 'Organize', detail: 'as in zt-book-digest', model: 'opus' },
    { title: 'Figures', detail: 'as in zt-book-digest', model: 'haiku' },
    { title: 'Hubs', detail: 'as in zt-book-digest', model: 'haiku' },
    { title: 'Audit', detail: 'digest-audit.ps1 as in zt-book-digest, then math-compiler.ps1 check: rules M1 to M18 on every unit and Theory Card (math-audit.json, math-audit.md)', model: 'haiku' },
  ],
}

// zt-math-digest is generated from zt-book-digest.js by scripts/book-digest/make-math-workflow.mjs.
// Edit zt-book-digest.js or the generator, not this file. Its args are those of zt-book-digest and:
//   mode ('math': only Boundary, Proof, Theory, and the math check on an existing digest),
//   boundary, proof, theory (true: run that math stage), supply ('none' | 'examples' |
//   'examples+answers': what DeepSeek may add that the book does not give; default 'examples'),
//   unitsPerJob (4: units per boundary or proof job), mainResults (7: main results per Theory
//   Card), coreBudget (3: core parts per unit), notation (true: write the Book notation table).
`)

// 2. The math prompts: the shared prefixes with the math supplement appended (math-compiler.ps1 -Step docs).
rep('digest prefix', 'const DIGEST_PREFIX = `${OUT}\\\\shared-prefix.md`', 'const DIGEST_PREFIX = `${OUT}\\\\math-prefix.md`')
rep('link prefix', 'const LINK_PREFIX = `${OUT}\\\\link-prefix.md`', 'const LINK_PREFIX = `${OUT}\\\\math-link-prefix.md`')

// 3. The math settings, after the last shared setting.
rep('math settings', 'const APPLY_SHARE = Math.max(1, A.applyShare || 30)\n', `const APPLY_SHARE = Math.max(1, A.applyShare || 30)
// --- math settings (zt-math-digest only) ---------------------------------------------------------
const MATH_COMPILER = \`\${SCRIPTS}\\\\math-compiler.ps1\`
const ARCH_MATH_DOC = \`\${SCRIPTS}\\\\model-architect-math.md\`
const MATH_WORK = \`\${OUT}\\\\math\`
const MATH_DOC_ARGS = \` -BriefDoc "\${MATH_WORK}\\\\brief.zh.md" -FillDoc "\${MATH_WORK}\\\\fill.zh.md" -ReviewDoc "\${MATH_WORK}\\\\review.zh.md"\`
const SUPPLY = ['none', 'examples', 'examples+answers'].includes(A.supply) ? A.supply : 'examples'
const UNITS_PER_JOB = Math.max(1, A.unitsPerJob || 4)
const MAIN_RESULTS = Math.max(1, A.mainResults || 7)
const MATH_MODE = MODE === 'full' || MODE === 'math'
const BOUNDARY = MATH_MODE && A.boundary !== false
const PROOF = MATH_MODE && A.proof !== false
const THEORY = MATH_MODE && A.theory !== false
const NOTATION = MODE === 'full' && A.notation !== false
`)

// 4. The model compiler takes the math instruction files; the architect reads the math supplement.
rep('compiler docs', '-Work "${dsh}" -Staging "${OUT}" ${LIMITS}${extra}`', '-Work "${dsh}" -Staging "${OUT}" ${LIMITS}${MATH_DOC_ARGS}${extra}`')
rep('architect doc', 'Read your instructions first: ${ARCH_DOC} (Read tool).', 'Read your instructions first: ${ARCH_DOC}, then the math supplement ${ARCH_MATH_DOC} (Read tool).')
// Theory Cards are written by the theory pass, never by the model compiler.
rep('no theory in the compiler', 'const todo = models.filter(m => !only || only.has(m.address))', 'const todo = models.filter(m => m.form !== \'theory\').filter(m => !only || only.has(m.address))')

// 5. Modes: "math" needs no groups.
rep('math mode needs no groups', "if (!groupCount && MODE !== 'models' && MODE !== 'hubs'", "if (!groupCount && MODE !== 'math' && MODE !== 'models' && MODE !== 'hubs'")

// 6. Prepare: the statement index, the prompt files, the skeleton, and the notation table.
rep('math prepare', '// The book term table (proposal 2)', `// The math skeleton (zt-math-digest only): after Prepare, before the term table and the digest.
if (MODE === 'full') {
  for (const [what, s, extra] of [['index', 'index', ''], ['docs', 'docs', ''], ['skeleton', 'skeleton', ' -Apply']]) {
    const done = await runMathScript(what, s, extra, 'Prepare')
    if (!done) return { stage: \`math \${what}\`, error: \`math-compiler.ps1 -Step \${s} failed\` }
    mathReports[what] = done
  }
  log(\`Math: \${mathReports.index.units} units (\${JSON.stringify(mathReports.index.forms)}), \${mathReports.index.examples} examples, \${mathReports.index.exercises} exercises; skeleton \${mathReports.skeleton.created} units and \${mathReports.skeleton.theories} Theory Cards.\`)
  if (NOTATION) {
    const nopts = { dsh: \`\${MATH_WORK}\\\\notation\`, skills: ['asd-ste100-zh'], effort: DSH_EFFORT, phase: 'Prepare', writes: \`\${MATH_WORK}\\\\notation\\\\out\`, remove: 'delete', keep: true }
    await drive('notation: plan', { ...nopts, command: mstep('notation') })
    mathReports.notation = lastJson(((await drive('notation: apply', { ...nopts, command: mstep('notation-apply', ' -Apply'), cleanup: true })) || {}).output)
    log(mathReports.notation && mathReports.notation.ok ? \`Notation table: \${mathReports.notation.symbols} symbols in \${mathReports.notation.card}.\` : 'Notation table failed; the digest goes on without it.')
  }
}
// The book term table (proposal 2)`)

// 7. The math stages: Boundary and Proof before the link pass, Theory after the link check.
rep('math stages', 'const NO_LINK = MODE === \'models\' || MODE === \'hubs\'\n', `// --- Math: Boundary and Proof (zt-math-digest only) ------------------------------------------------
if (BOUNDARY) mathReports.boundary = await runBoundary()
if (PROOF) mathReports.proof = await runProof()
if (MODE === 'math') {
  if (THEORY) mathReports.theory = await runTheory()
  mathReports.check = await runMathScript('check', 'check', \` -Out "\${OUT}"\`, 'Audit')
  return { mode: MODE, dsh_model: DSH_MODEL, citation_key: KEY, math: mathReports }
}
const NO_LINK = MODE === 'models' || MODE === 'hubs'
`)
rep('theory after the link check', '// --- Cleanup -------------------------------------------------------------------------------------\n', `// --- Math: Theory (zt-math-digest only): after the link pass, so the cited premises are complete ---
if (THEORY && !NO_LINK) mathReports.theory = await runTheory()

// --- Cleanup -------------------------------------------------------------------------------------
`)

// 8. The math check after the audit, and the math reports in the result.
rep('math check', 'const audit = AUDIT ? await runAudit() : null\n', `const audit = AUDIT ? await runAudit() : null
if (AUDIT && KEY) mathReports.check = await runMathScript('check', 'check', \` -Out "\${OUT}"\`, 'Audit')
`)
rep('math result', '  deepseek_usage: dshUsage, cleanup, organize, figures, hubs: hubReport, audit,\n}', '  deepseek_usage: dshUsage, cleanup, organize, figures, hubs: hubReport, audit, math: mathReports,\n}')

// 9. The math stage functions, before the card layer cycle.
rep('math functions', '// --- Cards: the card layer cycle -----------------------------------------------------------------\n', `// --- Math stages (zt-math-digest only) -------------------------------------------------------------
// math-compiler.ps1 does every structural step; DeepSeek writes JSON files and Card bodies; Haiku
// drivers relay the jobs under the DeepSeek Subagent Managed Protocol (drive, above). Each stage has
// its own DeepSeek workspace under math\\, and its last step's cleanup deletes the conversations.
const mathReports = {}
const mstep = (s, more = '') => \`pwsh -NoProfile -File "\${MATH_COMPILER}" -Step \${s} -Staging "\${OUT}"\${more}\`
async function runMathScript(what, s, extra, phaseName) {
  const run = await agent(
    \`Run this PowerShell command once in the foreground with timeout 600000 and wait for it; do nothing else:
\${mstep(s, extra)}
Return ok (true when it exits without an error) and output = its last output line exactly (a compact JSON object), or the error text when it fails. Do not retry.\`,
    { label: \`math: \${what}\`, phase: phaseName, model: 'haiku', effort: 'low', schema: { type: 'object', properties: { ok: { type: 'boolean' }, output: { type: 'string' } }, required: ['ok', 'output'] } },
  )
  const out = run && run.ok ? lastJson(run.output) : null
  if (!out) log(\`math \${what} failed: \${run && run.output}\`)
  return out
}
const mathOpts = (stage, phaseName) => ({ dsh: \`\${MATH_WORK}\\\\\${stage}\`, skills: ['asd-ste100-zh'], effort: DSH_EFFORT, phase: phaseName, writes: \`\${MATH_WORK}\\\\\${stage}\\\\out\`, remove: 'delete', keep: true })
async function runBoundary() {
  phase('Boundary')
  const o = mathOpts('boundary', 'Boundary')
  const planned = await drive('boundary: write', { ...o, command: mstep('boundary', \` -UnitsPerJob \${UNITS_PER_JOB} -Supply \${SUPPLY}\`) })
  const plan = lastJson(planned && planned.output)
  if (!plan) { await drive('boundary: clean up', { ...o, command: mstep('boundary-settle'), cleanup: true }); return { status: 'failed', error: (planned && planned.error) || 'no output' } }
  const applied = lastJson(((await drive('boundary: apply and review', { ...o, command: mstep('boundary-apply', \` -Supply \${SUPPLY}\`) })) || {}).output)
  const settled = lastJson(((await drive('boundary: settle', { ...o, command: mstep('boundary-settle'), cleanup: true })) || {}).output)
  log(\`Boundary: \${plan.units} units; \${applied ? \`\${applied.created} parts made, \${applied.skipped_supplied} supplied items skipped\` : 'apply failed'}; \${settled ? \`\${settled.deleted} wrong parts deleted, \${settled.kept} supplied parts kept\` : 'settle failed'}.\`)
  return { status: applied && settled ? 'done' : 'failed', plan: { units: plan.units, jobs: (plan.jobs || []).length }, applied, settled }
}
async function runProof() {
  phase('Proof')
  const o = mathOpts('proof', 'Proof')
  const planned = await drive('proof: review', { ...o, command: mstep('proof', \` -UnitsPerJob \${UNITS_PER_JOB}\`) })
  const plan = lastJson(planned && planned.output)
  const applied = lastJson(((await drive('proof: apply', { ...o, command: mstep('proof-apply'), cleanup: true })) || {}).output)
  log(\`Proof: \${plan ? plan.units : '?'} results reviewed; \${applied ? \`\${applied.ok} steps ok, \${applied.fixed} fixed, \${applied.refused} refused\` : 'apply failed'}.\`)
  return { status: plan && applied ? 'done' : 'failed', units: plan && plan.units, applied }
}
async function runTheory() {
  phase('Theory')
  const o = mathOpts('theory', 'Theory')
  const planned = await drive('theory: write', { ...o, command: mstep('theory', \` -MainResults \${MAIN_RESULTS}\`) })
  const plan = lastJson(planned && planned.output)
  const applied = lastJson(((await drive('theory: apply', { ...o, command: mstep('theory-apply', \` -MainResults \${MAIN_RESULTS}\`), cleanup: true })) || {}).output)
  log(\`Theory: \${applied ? \`\${applied.theories} Theory Cards written (\${applied.answered} answered by DeepSeek)\` : 'apply failed'}.\`)
  return { status: plan && applied ? 'done' : 'failed', theories: plan && plan.theories, applied }
}

// --- Cards: the card layer cycle -----------------------------------------------------------------
`)

if (crlf) s = s.replace(/\n/g, '\r\n')
const target = path.join(workflows, 'zt-math-digest.js')
// --check: compare with the file on disk instead of writing it (the harness runs this).
if (process.argv.includes('--check')) {
  const disk = fs.existsSync(target) ? fs.readFileSync(target, 'utf8').replace(/\r\n/g, '\n') : ''
  const same = disk === s.replace(/\r\n/g, '\n')
  console.log(same ? 'zt-math-digest.js is up to date' : 'zt-math-digest.js is out of date: run node scripts/book-digest/make-math-workflow.mjs')
  process.exit(same ? 0 : 1)
}
fs.writeFileSync(target, s)
console.log(`zt-math-digest.js: ${edits.length} edits (${edits.join(', ')})`)
