// node check-network.mjs <modelHead> [--json]
// Checks a model the way the GUI's model network (zt 0.44+) draws it, with the GUI's own layout
// code: each block (a main-line card with everything filed under it) hangs under the earlier
// block it rests on most. Prints that tree and every fault the network shows a reader:
//   forward     a card rests on a later block (a red thread): a derivation moves after its latest
//               premise; a setting stays and its reversed relation is turned the right way
//   unanchored  a chapter rests on no earlier block (a root with a red mark)
//   parts       a **分部** line that starts off the main line
//   lead        a chapter's 直觉 opening, the first line of the open block, is missing or short
//   empty       (advice) a chapter with no result card; right for an intuition or summary chapter
// and the shape: depth, the widest layer, and the threads drawn with the Model Card open.
// The layout code is the running service's /layout.js (ZT_GUI_PORT, default 4717), or ZT_LAYOUT.
// Exit 0 when the network is clean, 1 when it shows a fault.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)

/** The GUI's layout module: ZT_LAYOUT (a file), else the running service's /layout.js. */
export async function loadLayout() {
  if (process.env.ZT_LAYOUT) return require(path.resolve(process.env.ZT_LAYOUT))
  const port = process.env.ZT_GUI_PORT || '4717'
  const res = await fetch(`http://127.0.0.1:${port}/layout.js`)
  if (!res.ok) throw new Error(`no /layout.js on port ${port} (start the zt service, or set ZT_LAYOUT)`)
  const file = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'zt-layout-')), 'layout.js')
  fs.writeFileSync(file, await res.text())
  return require(file)
}

const NODE = { w: 124, h: 66 }
const LEAD_MIN = 20

/** The network of a model (`get_model` output) as the GUI draws it, with its faults and fixes. */
export function analyse(m, L) {
  const blocks = m.blocks || []
  const edges = m.block_edges || []
  const at = new Map(blocks.map((b, i) => [b.id, i]))
  const byId = new Map(blocks.map((b) => [b.id, b]))
  const facetOf = new Map([...blocks.map((b) => [b.head, b.facet]), ...blocks.flatMap((b) => b.children.map((c) => [c.address, c.facet]))])
  const name = (id) => {
    const b = byId.get(id)
    if (!b) return id
    if (b.kind === 'aside') return '旁注'
    const label = (b.facet || '').split(/[：:]/)[0].trim()
    const n = label.match(/^第\s*(\d+)\s*章$/)
    return n ? n[1] : label || b.head
  }
  const tree = L.blockTree(blocks, edges)
  const layout = L.blockLayout(blocks, edges, null, { focusOnly: true, backbone: true })

  // Forward: rests-on Links from a block to a later one, grouped by the Card that writes them.
  const forward = []
  for (const e of edges) {
    if (e.kind !== 'rests-on' || !(at.get(e.from_block) < at.get(e.to_block))) continue
    const cards = [...new Set((e.pairs || []).map((p) => p.from))]
    const fixes = cards.map((card) => {
      // The latest block among everything this card rests on decides where it can stand.
      const premises = edges.filter((x) => x.kind === 'rests-on' && x.from_block === e.from_block).flatMap((x) => (x.pairs || []).filter((p) => p.from === card).map(() => x.to_block))
      const latest = premises.sort((a, b) => at.get(b) - at.get(a))[0] || e.to_block
      // A setting is a primitive: it rests on nothing derived later, so the relation is reversed.
      if (/^设定/.test(facetOf.get(card) || '')) {
        return `${card} is a setting: a setting rests on nothing derived later, so this relation is reversed. If the later card (in ${byId.get(latest).head}, chapter ${name(latest)}) works with the setting's variable or definition, let it rest on ${card} and let the setting only point to it; keep it in the setting part. A derived fact the setting states moves to the card that derives it`
      }
      return `move ${card} after its latest premise: file it under ${byId.get(latest).head} (chapter ${name(latest)}) or later, with every card that rests on it`
    })
    forward.push({ from_block: e.from_block, to_block: e.to_block, count: e.count, cards, pairs: e.pairs || [], fixes, fix: fixes.join(' | ') || `block ${name(e.from_block)} rests on the later block ${name(e.to_block)}` })
  }
  const unanchored = layout.unanchored.map((id) => ({ block: id, fix: `chapter ${name(id)} rests on no earlier block: add a 依赖 line from ${byId.get(id).head} to the chapter it builds on` }))
  const chapters = blocks.filter((b) => b.kind === 'chapter')
  const leads = chapters
    .map((b) => (!b.lead ? { block: b.id, why: 'missing' } : [...b.lead].length < LEAD_MIN ? { block: b.id, why: 'short', lead: b.lead } : null))
    .filter(Boolean)
    .map((l) => ({ ...l, fix: l.why === 'missing' ? `give ${byId.get(l.block).head} a **直觉** section: its first sentence opens the block` : `the 直觉 of ${byId.get(l.block).head} opens with "${l.lead}"; state the mechanism in its first sentence (${LEAD_MIN}+ characters)` }))
  const isResult = (c) => /^结果/.test(c.facet || '')
  const empty = chapters.filter((b) => !b.children.some(isResult)).map((b) => ({ block: b.id, fix: `chapter ${name(b.id)} has no result card (facet 结果：…): its block shows no result` }))
  const parts = (m.build && m.build.parts) || []
  // One move per Card, however many later blocks it rests on.
  const moves = []
  for (const f of forward) for (const [i, card] of f.cards.entries()) {
    if (moves.some((x) => x.card === card)) continue
    moves.push({ card, fix: f.fixes[i] })
  }

  // Shape: the tree laid out at the board's card size; threads drawn with the Model Card open.
  const lay = L.treeLayout(tree.roots, tree.children, () => NODE, { gapX: 14, gapY: 34 })
  const depth = Math.max(0, ...Object.values(lay.pos).map((p) => p.depth)) + 1
  const lines = []
  const walk = (id, d) => {
    const b = byId.get(id)
    const marks = [
      forward.some((f) => f.from_block === id) ? 'FORWARD' : null,
      unanchored.some((u) => u.block === id) ? 'UNANCHORED' : null,
      empty.some((x) => x.block === id) ? 'no result' : null,
    ].filter(Boolean)
    lines.push(`${'  '.repeat(d)}${name(id)} ${b.kind === 'aside' ? '旁注与共用' : b.title} [${b.members} 张, ${b.children.filter(isResult).length} 结果]${marks.length ? ' <- ' + marks.join(', ') : ''}`)
    for (const c of tree.children[id] || []) walk(c, d + 1)
  }
  tree.roots.forEach((r) => walk(r, 0))
  const threads = { head: layout.ribbons.length * 2 + 3 }
  return {
    parent: tree.parent, lines, forward, moves, unanchored, parts, leads, empty,
    shape: { blocks: blocks.length, depth, width: Math.round(lay.width), fits: lay.width <= 520 },
    threads,
    ok: !forward.length && !unanchored.length && !parts.length && !leads.length,
  }
}

async function main() {
  const head = process.argv[2]
  if (!head) { console.error('usage: node check-network.mjs <modelHead> [--json]'); process.exit(2) }
  const { model } = await import('./zt.mjs')
  const m = model(head)
  if (!m.blocks || m.blocks.length < 2) { console.error(`${head}: no blocks (zt 0.43+ and a model with a main line)`); process.exit(2) }
  const r = analyse(m, await loadLayout())
  if (process.argv.includes('--json')) { console.log(JSON.stringify(r, null, 1)); process.exit(r.ok ? 0 : 1) }
  console.log(`network of ${head}: ${r.shape.blocks} blocks, ${r.shape.depth} layers, ${r.shape.width}px wide${r.shape.fits ? '' : ' (scaled down on the board)'}, ${r.threads.head} paths with the Model Card open`)
  for (const l of r.lines) console.log('  ' + l)
  for (const [k, list] of [['forward', r.moves], ['unanchored', r.unanchored], ['parts', r.parts], ['lead', r.leads], ['advice', r.empty]]) {
    for (const f of list) console.log(`${k}: ${f.fix}`)
  }
  console.log(r.ok ? 'network: clean' : 'network: faults above')
  process.exit(r.ok ? 0 : 1)
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()
