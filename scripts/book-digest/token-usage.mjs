// The Claude Code tokens a digest Workflow run spent, per agent kind, from its agent transcripts: the measurement
// of docs/digest-claude-token-proposal.md (gate 5, "Measured saving"). It only reads.
//
//   node scripts/book-digest/token-usage.mjs <session>\subagents\workflows [<run id> ...]
//
// <session> is ~/.claude/projects/<project>/<session id> of the session that ran the Workflow. With run ids, only
// those runs are read. It prints one JSON object: per run the agents and weighted tokens by kind/model and by
// phase, the relay commands by verb, the 25 costliest agents, and the relay averages (the first-request cache
// write per relay is the P5 measurement).
//
// Weighted tokens = input + 1.25 × cache write + 0.1 × cache read + 5 × output, in units of each model's own input
// price. A request is counted once (by request id). Kinds: relay (task starts "Run this PowerShell command once"),
// cleanup-agent, and manager (every other agent; the model column tells Haiku, Sonnet and Opus apart).
import fs from 'node:fs'
import path from 'node:path'

const [root, ...only] = process.argv.slice(2)
if (!root) {
  console.error('usage: node token-usage.mjs <session>\\subagents\\workflows [<run id> ...]')
  process.exit(2)
}
const weighted = (r) => Math.round(r.input + 1.25 * r.write + 0.1 * r.read + 5 * r.output)
const kindOf = (prompt, label) => {
  if (/^\s*Run this PowerShell command once/.test(prompt)) return 'relay'
  if (/^cleanup chunk|^cleanup workspaces|^cleanup: settle the run/.test(label)) return 'cleanup-agent'
  return 'manager'
}
// The task text sits after the harness preamble: the first user text that names the computed task.
const firstPrompt = (lines) => {
  let first = ''
  for (const j of lines) {
    if (j.type !== 'user' || !j.message) continue
    const c = j.message.content
    const texts = typeof c === 'string' ? [c] : (Array.isArray(c) ? c.filter((x) => x.type === 'text').map((x) => x.text) : [])
    for (const t of texts) {
      const m = t.match(/computed task text follows:\s*\n([\s\S]*)/)
      if (m) return m[1].replace(/^ {2}/gm, '')
      if (!first && !/^<system-reminder>/.test(t.trim())) first = t
    }
  }
  return first
}
const rows = []
for (const run of fs.readdirSync(root)) {
  const dir = path.join(root, run)
  if (!fs.statSync(dir).isDirectory() || (only.length && !only.includes(run))) continue
  for (const m of fs.readdirSync(dir).filter((f) => f.endsWith('.meta.json'))) {
    const id = m.replace('.meta.json', '')
    const meta = JSON.parse(fs.readFileSync(path.join(dir, m), 'utf8'))
    const file = path.join(dir, `${id}.jsonl`)
    if (!fs.existsSync(file)) continue
    const lines = fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => { try { return JSON.parse(l) } catch { return {} } })
    const seen = new Set()
    const r = { run, id, label: meta.description || '', model: meta.model || '', phase: meta.workflowPhase || '', requests: 0, tools: 0, input: 0, write: 0, read: 0, output: 0, firstWrite: null }
    for (const j of lines) {
      if (j.type === 'assistant' && j.message && Array.isArray(j.message.content)) r.tools += j.message.content.filter((x) => x.type === 'tool_use').length
      const u = j.message && j.message.usage
      if (!u) continue
      const key = j.requestId || j.message.id
      if (seen.has(key)) continue
      seen.add(key)
      r.requests += 1
      r.input += u.input_tokens || 0
      r.write += u.cache_creation_input_tokens || 0
      r.read += u.cache_read_input_tokens || 0
      r.output += u.output_tokens || 0
      if (r.firstWrite === null) r.firstWrite = (u.cache_creation_input_tokens || 0) + (u.cache_read_input_tokens || 0) + (u.input_tokens || 0)
    }
    const prompt = firstPrompt(lines)
    r.kind = kindOf(prompt, r.label)
    r.command = r.kind === 'relay' ? (prompt.split('\n')[1] || '').slice(0, 160) : ''
    r.weighted = weighted(r)
    rows.push(r)
  }
}
const group = (list, keyf) => {
  const g = {}
  for (const r of list) {
    const k = keyf(r)
    const t = (g[k] = g[k] || { agents: 0, requests: 0, weighted: 0 })
    t.agents += 1
    t.requests += r.requests
    t.weighted += r.weighted
  }
  return Object.fromEntries(Object.entries(g).sort((a, b) => b[1].weighted - a[1].weighted))
}
const verb = (c) => {
  const m = c.match(/zt-digest ([a-z-]+)(?: ([a-z-]+))?/)
  if (!m) return c.slice(0, 40)
  const step = (c.match(/-Step ([a-z-]+)/) || [])[1]
  return [m[1], m[1] === 'ledger' || m[1] === 'call' ? m[2] : step].filter(Boolean).join(' ')
}
const out = { runs: {} }
for (const run of [...new Set(rows.map((r) => r.run))]) {
  const list = rows.filter((r) => r.run === run)
  const relays = list.filter((r) => r.kind === 'relay')
  out.runs[run] = {
    agents: list.length,
    weighted: list.reduce((n, r) => n + r.weighted, 0),
    byKind: group(list, (r) => `${r.kind}/${r.model}`),
    byPhase: group(list, (r) => r.phase),
    relayVerbs: group(relays, (r) => verb(r.command)),
    relay: {
      agents: relays.length,
      weightedPerAgent: Math.round(relays.reduce((n, r) => n + r.weighted, 0) / Math.max(1, relays.length)),
      requestsPerAgent: Number((relays.reduce((n, r) => n + r.requests, 0) / Math.max(1, relays.length)).toFixed(2)),
      firstRequestContext: Math.round(relays.reduce((n, r) => n + (r.firstWrite || 0), 0) / Math.max(1, relays.length)),
    },
    top: list.slice().sort((a, b) => b.weighted - a.weighted).slice(0, 25).map((r) => ({ label: r.label, kind: r.kind, model: r.model, requests: r.requests, weighted: r.weighted })),
  }
}
console.log(JSON.stringify(out, null, 1))
