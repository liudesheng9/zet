// node tree-mover.mjs <spec.json> plan | run
//
// Rebuilds the inner structure of one model to a target tree. Every Card is named by its address when
// the run began, written relative to the head as `H|…`; the mover follows each Card through the rename
// map of every move, so later addresses never need to be predicted.
//
// spec.json:
// {
//   "head": "Key/4|a",                      the Model Card
//   "keep": ["H|a"],                         Cards that stay where they are (the setting, normally)
//   "keepSubtrees": ["H|a|a", "H|a|b"],      Cards that stay with everything filed under them (the setting's sides)
//   "after": "H|a",                          the kept Card the main line continues from
//   "park": "H|z",                           a free Side slot of a Card that stays (the parking chain starts there)
//   "main": [ node, … ]                      the main line after `after`, in order
// }
// node = "H|…"  or  { "id": "H|…", "sides": [node, …], "next": node }
//   sides: the Card's Side branches in order; next: its Direct successor (a chain is nested `next`s).
//   ["H|x", "H|y", "H|z"] as a node is shorthand for a Direct chain x → y → z.
//
// `plan` checks that every Card of the model outside `keep` is placed exactly once. `run` detaches those
// Cards leaf-first onto a parking chain, then places them top-down; it freezes the starting tree in
// <spec>.tree.json and saves progress in <spec>.state.json after every move, so a second `run` resumes.
import fs from 'node:fs'
import { tree as readTree, move } from './zt.mjs'

const [specFile, mode = 'plan'] = process.argv.slice(2)
if (!specFile) { console.error('usage: node tree-mover.mjs <spec.json> plan|run'); process.exit(2) }
const spec = JSON.parse(fs.readFileSync(specFile, 'utf8'))
const H = spec.head
const full = s => (s === 'H' ? H : s.startsWith('H|') ? H + s.slice(1) : s)
const rel = a => (a.startsWith(H) ? 'H' + a.slice(H.length) : a)

const norm = node => {
  if (typeof node === 'string') return { id: full(node), sides: [], next: null }
  if (Array.isArray(node)) return node.map(norm).reduceRight((next, n) => ({ ...n, next }), null)
  return { id: full(node.id), sides: (node.sides || []).map(norm), next: node.next ? norm(node.next) : null }
}
const main = spec.main.map(norm)

const treeFile = specFile.replace(/\.json$/, '') + '.tree.json'
const tree = fs.existsSync(treeFile) ? JSON.parse(fs.readFileSync(treeFile, 'utf8')) : readTree(H.split('/')[0]).map(c => ({ address: c.address, parent: c.parent }))
const keepExact = new Set((spec.keep || []).map(full))
const keepUnder = (spec.keepSubtrees || []).map(full)
const keep = a => a === H || keepExact.has(a) || keepUnder.some(s => a === s || a.startsWith(s + '|'))
const inModel = a => a === H || a.startsWith(H + '|')
const detach = tree.filter(c => inModel(c.address) && !keep(c.address)).map(c => c.address)

const ids = []
const walk = n => { if (!n) return; ids.push(n.id); n.sides.forEach(walk); walk(n.next) }
main.forEach(walk)
const want = new Set(ids)
const missing = detach.filter(a => !want.has(a)).map(rel), unknown = ids.filter(a => !detach.includes(a)).map(rel), twice = ids.filter((x, i) => ids.indexOf(x) !== i).map(rel)
console.log(`detach ${detach.length}, placed ${ids.length}`)
if (missing.length) console.log('not placed:', missing.join(' '))
if (unknown.length) console.log('placed but not detached (kept or not in the model):', unknown.join(' '))
if (twice.length) console.log('placed twice:', twice.join(' '))
if (missing.length || unknown.length || twice.length) process.exit(1)
if (mode !== 'run') process.exit(0)
if (!fs.existsSync(treeFile)) fs.writeFileSync(treeFile, JSON.stringify(tree))

// The order of moves: detach leaf-first, then place top-down.
const kids = {}
for (const c of tree) if (c.parent) (kids[c.parent] ||= []).push(c.address)
const plan = []
const visit = a => { for (const k of kids[a] || []) visit(k); if (inModel(a) && !keep(a)) plan.push({ park: a }) }
visit(H)
const place = (n, slot) => { plan.push({ id: n.id, slot }); n.sides.forEach((s, i) => place(s, { side: n.id, i })); if (n.next) place(n.next, { direct: n.id }) }
let prev = full(spec.after)
for (const m of main) { place(m, { direct: prev }); prev = m.id }

const stateFile = specFile.replace(/\.json$/, '') + '.state.json'
const st = fs.existsSync(stateFile) ? JSON.parse(fs.readFileSync(stateFile, 'utf8')) : { cur: Object.fromEntries([...detach, full(spec.after)].map(a => [a, a])), step: 0, tail: null }
const nextDirect = a => { const s = a.split('|'); const t = s[s.length - 1]; return /^\d+$/.test(t) ? [...s.slice(0, -1), String(+t + 1)].join('|') : a + '|1' }
const letters = 'abcdefghijklmnopqrstuvwxyz'
for (; st.step < plan.length; st.step++) {
  const p = plan[st.step]
  let id, to, alone
  if (p.park) { id = p.park; to = st.tail ? nextDirect(st.cur[st.tail]) : full(spec.park); alone = false }
  else { id = p.id; alone = true; to = p.slot.side ? `${st.cur[p.slot.side]}|${letters[p.slot.i]}` : nextDirect(st.cur[p.slot.direct]) }
  const map = move({ at: st.cur[id], to, ...(alone ? { alone: true } : {}) })
  for (const k of Object.keys(st.cur)) st.cur[k] = map[st.cur[k]] || st.cur[k]
  if (p.park) st.tail = id
  fs.writeFileSync(stateFile, JSON.stringify(st))
  if (st.step % 20 === 0) console.log(`${st.step}/${plan.length} ${p.park ? 'park' : 'place'} ${rel(id)} -> ${st.cur[id]}`)
}
console.log(`done: ${plan.length} moves`)
