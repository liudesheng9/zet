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
// as its own. Exit 0 with no hit, 1 otherwise.
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const eqIn = (s) => [...(s || '').matchAll(/[（(](\d{1,2}[a-c]?)[)）]/g)].map((x) => x[1])
const POINTERS = /\*\*(为什么需要它|解读|承上启下)\*\*[\s\S]*$/

const displays = (body) => [...(body || '').matchAll(/\$\$([\s\S]*?)\$\$/g)].map((x) => x[1])
const tags = (tex) => [...tex.matchAll(/\\tag\s*\{\s*(\d{1,2}[a-c]?)\s*\}/g)].map((x) => x[1])

/** Hits for a model: `order` is the build order (addresses), `cards` maps address -> {title, body, facet}. */
export function scan(order, cards, relations = []) {
  const pos = new Map(order.map((a, i) => [a, i]))
  const defs = new Map()
  for (const at of order) {
    const c = cards.get(at) || {}
    // Prose may use an equation anywhere (a result computed "with (33)"); only a title or a display
    // formula derives one.
    const named = [...eqIn(c.title), ...displays(c.body).flatMap((d) => [...eqIn(d), ...tags(d)])]
    for (const n of named) if (!defs.has(n)) defs.set(n, at)
  }
  const rests = new Set(relations.filter((r) => r.kind === 'rests-on').map((r) => `${r.from}>${r.to}`))
  const hits = []
  for (const at of order) {
    if (/^符号/.test(cards.get(at)?.facet || '')) continue
    const body = (cards.get(at)?.body || '').replace(POINTERS, '')
    for (const n of new Set(eqIn(body))) {
      const d = defs.get(n)
      if (!d || d === at || pos.get(d) <= pos.get(at) || d.startsWith(at + '|')) continue
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
  const hits = scan(order, byAt, m.relations)
  const sh = (a) => (a.startsWith(head) ? 'H' + a.slice(head.length) : a)
  for (const h of hits) console.log(`${sh(h.card)} names (${h.eq}), derived later by ${sh(h.derived_by)}: ${h.line.slice(0, 140)}`)
  console.log(hits.length ? `${hits.length} hits: read each; a use is fixed by moving cards, a pointer stays` : 'premises: none used before their card')
  process.exit(hits.length ? 1 : 0)
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()
