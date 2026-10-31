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
// `plan` checks that every Card of the model outside `keep` is placed exactly once. `run` parks those
// Cards onto a parking chain (each branch with its Direct chain in one move, deepest first), then places
// them one by one top-down; it freezes the starting tree in <spec>.tree.json and saves progress in
// <spec>.state.json after every move, so a second `run` resumes at the next move. A failed move is retried
// once (after MOVER_RETRY_MS, default 3000) before the run stops.
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
const stateFile = specFile.replace(/\.json$/, '') + '.state.json'
const saved = fs.existsSync(stateFile) ? JSON.parse(fs.readFileSync(stateFile, 'utf8')) : null
// Planner 2 parks each branch in one move: a branch starts at a detached Card filed as a Side, or under a
// kept Card, and runs down its Direct chain. Branches park deepest first, so each one is bare (no Side
// successor left) when it moves, and the parking chain is one Direct chain of bare Cards as before.
// Planner 1 (a state file without `planner`, from an older mover) parks Card by Card; a resume keeps it.
const planner = saved && !saved.planner ? 1 : 2
const isDetached = a => inModel(a) && !keep(a)
const parentOf = Object.fromEntries(tree.map(c => [c.address, c.parent]))
const directKid = a => (kids[a] || []).find(k => /^\d+$/.test(k.slice(k.lastIndexOf('|') + 1)))
const isStart = a => isDetached(a) && (!isDetached(parentOf[a]) || !/^\d+$/.test(a.slice(a.lastIndexOf('|') + 1)))
const below = a => (kids[a] || []).flatMap(k => [k, ...below(k)])
const chainOf = a => { const c = [a]; let d; while ((d = directKid(c[c.length - 1])) && isDetached(d)) c.push(d); return c }
const covered = new Set()
if (planner === 2) for (const c of tree) if (isStart(c.address) && !below(c.address).some(k => !isDetached(k))) chainOf(c.address).slice(1).forEach(k => covered.add(k))
const plan = []
const visit = a => {
  for (const k of kids[a] || []) visit(k)
  if (!isDetached(a) || covered.has(a)) return
  plan.push(planner === 2 && isStart(a) && !below(a).some(k => !isDetached(k)) ? { park: a, last: chainOf(a).at(-1) } : { park: a })
}
visit(H)
const place = (n, slot) => { plan.push({ id: n.id, slot }); n.sides.forEach((s, i) => place(s, { side: n.id, i })); if (n.next) place(n.next, { direct: n.id }) }
let prev = full(spec.after)
for (const m of main) { place(m, { direct: prev }); prev = m.id }

const st = saved || { planner, cur: Object.fromEntries([...detach, full(spec.after)].map(a => [a, a])), step: 0, tail: null }
// One retry after a pause, then stop with the move named (a resume starts at that move).
const retryMs = Number(process.env.MOVER_RETRY_MS ?? 3000)
const moveOnce = args => {
  try { return move(args) } catch (e) {
    console.error(`move ${st.step + 1}/${plan.length} failed, retrying once: ${String(e.message).slice(0, 200)}`)
    if (retryMs > 0) Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, retryMs)
    try { return move(args) } catch (e2) {
      console.error(`move ${st.step + 1}/${plan.length} failed twice: ${JSON.stringify(args)}; run the same command again to resume at this move`)
      throw e2
    }
  }
}
const nextDirect = a => { const s = a.split('|'); const t = s[s.length - 1]; return /^\d+$/.test(t) ? [...s.slice(0, -1), String(+t + 1)].join('|') : a + '|1' }
const letters = 'abcdefghijklmnopqrstuvwxyz'
for (; st.step < plan.length; st.step++) {
  const p = plan[st.step]
  let id, to, alone
  if (p.park) { id = p.park; to = st.tail ? nextDirect(st.cur[st.tail]) : full(spec.park); alone = false }
  else { id = p.id; alone = true; to = p.slot.side ? `${st.cur[p.slot.side]}|${letters[p.slot.i]}` : nextDirect(st.cur[p.slot.direct]) }
  const map = moveOnce({ at: st.cur[id], to, ...(alone ? { alone: true } : {}) })
  for (const k of Object.keys(st.cur)) st.cur[k] = map[st.cur[k]] || st.cur[k]
  if (p.park) st.tail = p.last || id
  // Save the index of the NEXT move: a resume must not repeat the move just made.
  fs.writeFileSync(stateFile, JSON.stringify({ ...st, step: st.step + 1 }))
  if (st.step % 20 === 0) console.log(`${st.step}/${plan.length} ${p.park ? 'park' : 'place'} ${rel(id)} -> ${st.cur[id]}`)
}
console.log(`done: ${plan.length} moves`)
