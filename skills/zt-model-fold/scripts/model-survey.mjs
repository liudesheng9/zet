// node model-survey.mjs <citationKey>
// Lists every Model Card of one Literature tree (its level, parts, question and setting), the Links
// between Model Cards, and the candidate groups: Model Cards joined by rests-on Links or shared parts.
// A candidate group is a hypothesis about the book's models, never the decision: the skill decides
// each group by the nature of the models (the question, the primitives), not by the size of the book.
//   structural  rests-on Links and shared parts: one card builds on the other's cards (joins a group)
//   kin         relaxes, corresponds: the same framework seen from another side (does not join; read it)
//   compare     contrasts, illustrates, refers: a comparison or a pointer (does not join)
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const KIN = new Set(['relaxes', 'corresponds'])
const COMPARE = new Set(['contrasts', 'illustrates', 'refers'])

/** Candidate groups: connected components over the structural links, in the book's order. */
export function propose(heads, links) {
  const root = new Map(heads.map((h) => [h, h]))
  const find = (h) => (root.get(h) === h ? h : find(root.get(h)))
  for (const l of links) if (l.rests + l.shared > 0) root.set(find(l.from), find(l.to))
  const groups = new Map()
  for (const h of heads) {
    const r = find(h)
    if (!groups.has(r)) groups.set(r, [])
    groups.get(r).push(h)
  }
  const describe = (l, kinds) => `${l.from} -> ${l.to}: ${kinds.filter(([, n]) => n).map(([k, n]) => `${k} ${n}`).join(', ')}`
  return [...groups.values()].map((hs) => {
    const inside = (l) => hs.includes(l.from)
    return {
      heads: hs,
      evidence: links.filter((l) => inside(l) && l.rests + l.shared > 0).map((l) => describe(l, [['rests-on', l.rests], ['shared', l.shared]])),
      ask: links.filter((l) => inside(l) && l.rests + l.shared === 0 && l.kin + l.compare > 0).map((l) => describe(l, [['kin', l.kin], ['compare', l.compare]])),
    }
  })
}

async function main() {
  const key = process.argv[2]
  if (!key) { console.error('usage: node model-survey.mjs <citationKey>'); process.exit(2) }
  const { reads, model } = await import('./zt.mjs')
  const rows = reads([{ tool: 'search_cards', arguments: { type: 'model', query: key, limit: 200 } }])[0].results
  const heads = rows.filter((r) => !r.facet && r.address.startsWith(key + '/')).map((r) => r.address)
  const owner = (a) => heads.filter((h) => a === h || a.startsWith(h + '|')).sort((x, y) => y.length - x.length)[0]
  const sh = (a) => a.slice(key.length + 1)
  const pairs = new Map()
  const pair = (from, to) => {
    const k = from + '>' + to
    if (!pairs.has(k)) pairs.set(k, { from, to, rests: 0, shared: 0, kin: 0, compare: 0 })
    return pairs.get(k)
  }
  console.log(`${heads.length} Model Card(s) in ${key}`)
  for (const h of heads) {
    const m = model(h)
    const order = m.build.order || []
    for (const o of order) if (o.line === 'shared') { const w = owner(o.address); if (w && w !== h) pair(h, w).shared++ }
    for (const n of m.neighbours || []) {
      if (n.way !== 'out') continue
      const w = owner(n.to)
      if (!w || w === h) continue
      if (n.kind === 'rests-on') pair(h, w).rests++
      else if (KIN.has(n.kind)) pair(h, w).kin++
      else if (COMPARE.has(n.kind)) pair(h, w).compare++
    }
    const question = (m.head.body || '').split('\n').find((l) => l.trim() && !l.startsWith('**')) || ''
    const setting = order.find((o) => o.address === m.build.setting)
    console.log(`${sh(h).padEnd(8)} ${String(m.level).padEnd(7)} parts ${String(order.length).padStart(3)}  ${m.head.title}`)
    console.log(`         question: ${question.trim().slice(0, 120)}`)
    if (setting && setting.address !== h) console.log(`         setting:  ${setting.title}`)
  }
  const groups = propose(heads, [...pairs.values()])
  console.log(`\nCandidate groups (joined by rests-on Links or shared parts): ${groups.length}`)
  groups.forEach((g, i) => {
    console.log(`G${i + 1}: ${g.heads.map(sh).join(' ')}`)
    for (const e of g.evidence) console.log(`    builds on  ${e.replaceAll(key + '/', '')}`)
    for (const a of g.ask) console.log(`    read       ${a.replaceAll(key + '/', '')}`)
  })
  console.log('\nDecide each group by the nature of its models: one model shares the question and the primitives (agents, choices, technology, timing) and continues them; a Model Card that only borrows a result, compares, or changes the question or the primitives is a model of its own. The size of the book decides nothing.')
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()
