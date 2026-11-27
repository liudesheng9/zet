// node check-premises.mjs <modelHead>
// Finds dependencies the Links hide: a card that names an equation number "(n)" which a card
// later in the build derives. Such a card uses a later result, whatever its Links say. Fix it by
// moving: the card (or the sentence that uses the result) goes after the card that derives it,
// or the derivation goes first. Never by deleting the Link or dropping its cue.
// Each hit needs reading: a symbol table, a roadmap ("(20)–(22) give ...") or a pointer to the
// card that uses this one is not a use; the report lists them all and the reader decides.
// An equation's card: the earliest card in build order whose title names "(n)" or whose display
// formula carries it ($$ … (n) $$, \tag{n}); prose that only uses "(n)" derives nothing. The notation card
// (facet 符号…) is a lookup table and is skipped. Text after **为什么需要它**, **解读** or **承上启下** is ignored (those
// sections point to the cards that use this one). A card's own proof chain (filed under it) counts
// as its own. A part another model lends by a legacy （共用） share is that model's: its equations
// neither define nor count (#205); a part an **引入** line imports counts as the model's own.
// Exit 0 with no hit, 1 otherwise.
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const EQ_ID_SRC = String.raw`[A-Za-z]\.\d{1,2}[a-c]?|[A-Za-z]?\d{1,2}(?:\.\d{1,2})?[a-c]?`
const EQ_NUM = new RegExp(`[（(](${EQ_ID_SRC})[)）]`, 'g')
const eqIn = (s) => [...(s || '').matchAll(EQ_NUM)].map((x) => x[1])
const displays = (body) => [...(body || '').matchAll(/\$\$([\s\S]*?)\$\$/g)].map((x) => x[1])
const TAG_RE = new RegExp(`\\\\tag\\s*\\{\\s*(${EQ_ID_SRC})\\s*\\}`, 'g')
const tags = (tex) => [...tex.matchAll(TAG_RE)].map((x) => x[1])
const CRITERION = /^(?:判据|均衡定义|解概念)/
const RESULT_OWNER = /^(?:结果|基准)/
const PROOF_STEP = /^(?:推导|论证)/

/** Remove pointer-only sections individually (up to the next **Heading** — standalone or inline — or 来源：),
 * never discarding later substantive sections. On Criterion cards, **说明** is a forward navigation pointer by proposal §5.4. */
export function stripPointerSections(body, facet = '') {
  const skipNames = new Set(['为什么需要它', '解读', '承上启下'])
  if (CRITERION.test(facet || '')) skipNames.add('说明')
  const lines = (body || '').replace(/\r/g, '').split('\n')
  const out = []
  let skipping = false
  for (const line of lines) {
    const h = line.match(/^\s*\*\*([^*]+)\*\*/)
    if (h) {
      skipping = skipNames.has(h[1].trim())
      if (!skipping) out.push(line)
      continue
    }
    if (/^\s*来源[：:]/.test(line)) skipping = false
    if (!skipping) out.push(line)
  }
  return out.join('\n')
}

function extractSection(body, names) {
  const lines = (body || '').replace(/\r/g, '').split('\n')
  let inSec = false
  const out = []
  for (const line of lines) {
    const h = line.match(/^\s*\*\*([^*]+)\*\*(.*)$/)
    if (h) {
      inSec = names.has(h[1].trim())
      if (inSec && h[2].trim()) out.push(h[2])
      continue
    }
    if (/^\s*来源[：:]/.test(line)) inSec = false
    if (inSec) out.push(line)
  }
  return out.join('\n')
}

function isDirectSideChild(addr, parent) {
  if (!addr.startsWith(parent + '|')) return false
  const tail = addr.slice(parent.length + 1)
  return /^[a-z]+$/.test(tail)
}

function nextDirectAddress(addr) {
  const m = addr.match(/^(.*)\|(\d+)$/)
  if (m) return `${m[1]}|${Number(m[2]) + 1}`
  return `${addr}|1`
}

// Verbatim port of Rust `zt_check::formal::is_local_route_clause` (tools/zt-check/src/formal.rs:352).
function isLocalRouteClause(clause) {
  const t = (clause || '').trim()
  if (!t || t.includes('外部') || t.includes('（行内）') || t.includes('(行内)') || t.includes('inline:')) return false
  return (
    t.includes('局部证明') ||
    t.includes('局部论证') ||
    t.includes('局部构造') ||
    t.includes('局部图示') ||
    t.includes('局部例示') ||
    t.includes('局部数值') ||
    t.includes('启发式推导')
  )
}

/** Exact bound proof-route addresses matching Rust `src/model/formal.rs::proof_route_addresses`:
 * - Result/Benchmark with **依据**: only direct Side proof roots linked in non-external, non-inline local clauses,
 *   plus their contiguous Direct proof-step continuations.
 * - Legacy Result with **证明** (no **依据**): direct Side proof roots linked in **证明** + Direct continuations.
 * - Benchmark without **依据**: all direct Side proof children + their Direct proof-step continuations. */
function boundProofRoute(at, card, order, cards) {
  const isResult = /^结果/.test(card.facet || card.title || '')
  const isBenchmark = /^基准/.test(card.facet || card.title || '')
  if (!isResult && !isBenchmark) return new Set()
  const isProofNode = (addr) => {
    const c = cards.get(addr)
    return Boolean(c && PROOF_STEP.test(c.facet || c.title || ''))
  }
  const hasJust = /^\s*\*\*依据\*\*/m.test(card.body || '')
  const hasLegacyProof = !hasJust && /^\s*\*\*证明\*\*/m.test(card.body || '')
  const roots = []
  if (hasJust) {
    const sec = extractSection(card.body || '', new Set(['依据']))
    for (const line of sec.split('\n')) {
      for (const clause of line.split(/[；;]/)) {
        if (!isLocalRouteClause(clause)) continue
        for (const m of clause.matchAll(/\[\[([^\]@]+)\]\]/g)) {
          const target = m[1].trim()
          if (isDirectSideChild(target, at) && isProofNode(target) && !roots.includes(target)) roots.push(target)
        }
      }
    }
  } else if (hasLegacyProof) {
    const sec = extractSection(card.body || '', new Set(['证明']))
    for (const m of sec.matchAll(/\[\[([^\]@]+)\]\]/g)) {
      const target = m[1].trim()
      if (isDirectSideChild(target, at) && isProofNode(target) && !roots.includes(target)) roots.push(target)
    }
  } else if (isBenchmark) {
    for (const addr of order) {
      if (isDirectSideChild(addr, at) && isProofNode(addr) && !roots.includes(addr)) roots.push(addr)
    }
  }
  const out = new Set()
  for (const root of roots) {
    let cur = root
    while (cur && !out.has(cur) && isProofNode(cur)) {
      out.add(cur)
      cur = nextDirectAddress(cur)
    }
  }
  return out
}

/** Hits for a model: `order` is the build order (addresses), `cards` maps address -> {title, body, facet},
 * `foreign` holds the Cards of other models in the order (legacy shares), left out of the scan. */
export function scan(order, cards, relations = [], foreign = new Set()) {
  const pos = new Map(order.map((a, i) => [a, i]))
  const defs = new Map()
  for (const at of order) {
    if (foreign.has(at)) continue
    const c = cards.get(at) || {}
    // Prose may use an equation anywhere (a result computed "with (33)"); only a title or a display
    // formula derives one. Prefer local definitions over earlier imported ones if both define the same tag.
    const named = [...eqIn(c.title), ...displays(c.body).flatMap((d) => [...eqIn(d), ...tags(d)])]
    for (const n of named) if (!defs.has(n)) defs.set(n, at)
  }
  const rests = new Set(relations.filter((r) => r.kind === 'rests-on').map((r) => `${r.from}>${r.to}`))
  const hits = []
  for (const at of order) {
    const card = cards.get(at) || {}
    if (foreign.has(at) || /^符号/.test(card.facet || '')) continue
    const body = stripPointerSections(card.body || '', card.facet || '')
    const route = boundProofRoute(at, card, order, cards)
    for (const n of new Set(eqIn(body))) {
      const d = defs.get(n)
      if (!d || d === at || pos.get(d) <= pos.get(at)) continue
      if (route.has(d)) continue
      const line = body.split('\n').find((l) => l.includes(`(${n})`) || l.includes(`（${n}）`)) || ''
      hits.push({ card: at, eq: n, derived_by: d, linked: rests.has(`${at}>${d}`), line: line.trim() })
    }
  }
  return hits
}

async function main() {
  const head = process.argv[2]
  if (!head) { console.error('usage: node check-premises.mjs <modelHead>'); process.exit(2) }
  const { model, cards } = await import('./zt.mjs')
  const m = model(head)
  const order = m.build.order.map((o) => o.address)
  const byAt = new Map(cards(order).map((c) => [c.address, c]))
  const foreign = new Set(m.build.order.filter((o) => o.line === 'shared' && !o.import).map((o) => o.address))
  const hits = scan(order, byAt, m.relations, foreign)
  const sh = (a) => (a.startsWith(head) ? 'H' + a.slice(head.length) : a)
  for (const h of hits) console.log(`${sh(h.card)} names (${h.eq}), derived later by ${sh(h.derived_by)}: ${h.line.slice(0, 140)}`)
  console.log(hits.length ? `${hits.length} hits: read each; a use is fixed by moving cards, a pointer stays` : 'premises: none used before their card')
  process.exit(hits.length ? 1 : 0)
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()
