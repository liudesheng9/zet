// node --test tree-mover.test.mjs
// Runs the real tree-mover.mjs in a temp folder beside a fake zt.mjs (no live box). The fake simulates zt's
// moves on a small tree: a non-alone move carries the Card with all its successors; an alone move needs a
// Card without Side successors, and its Direct successor takes its place; the target slot must be free.
// It returns the rename map of every Card, logs every move, and can fail the move named by FAIL_AT.
import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))

const FAKE_ZT = String.raw`import fs from 'node:fs'
const dir = process.env.SIM_DIR
const simFile = dir + '/sim.json', log = dir + '/moves.log'
// Each node: parent id, edge 'direct' | 'side', letter (for a side). The root K/1|a has a fixed address.
const START = JSON.parse(process.env.SIM_START)
const load = () => (fs.existsSync(simFile) ? JSON.parse(fs.readFileSync(simFile, 'utf8')) : START)
const isNum = s => /^\d+$/.test(s)
export function addresses(nodes) {
  const out = {}
  const addr = id => {
    if (out[id]) return out[id]
    const n = nodes[id]
    if (!n.parent) return (out[id] = id)
    const p = addr(n.parent)
    if (n.edge === 'side') return (out[id] = p + '|' + n.letter)
    const s = p.split('|'), t = s[s.length - 1]
    return (out[id] = isNum(t) ? [...s.slice(0, -1), String(+t + 1)].join('|') : p + '|1')
  }
  for (const id of Object.keys(nodes)) addr(id)
  return out
}
export const tree = () => { const n = load(), a = addresses(n); return Object.keys(n).map(id => ({ address: a[id], parent: n[id].parent ? a[n[id].parent] : null })) }
export function move(args) {
  const done = fs.existsSync(log) ? fs.readFileSync(log, 'utf8').split('\n').filter(Boolean).length : 0
  if (String(done) === process.env.FAIL_AT) {
    const once = dir + '/failed.once'
    if (!process.env.FAIL_ONCE || !fs.existsSync(once)) { fs.writeFileSync(once, '1'); throw new Error('fake move failure') }
  }
  const nodes = load(), before = addresses(nodes)
  const byAddr = Object.fromEntries(Object.entries(before).map(([id, a]) => [a, id]))
  const id = byAddr[args.at]
  if (!id) throw new Error('no card at ' + args.at)
  if (byAddr[args.to]) throw new Error('slot taken ' + args.to)
  const s = args.to.split('|'), t = s[s.length - 1]
  const edge = isNum(t) ? 'direct' : 'side'
  const parentAddr = edge === 'side' ? s.slice(0, -1).join('|') : +t === 1 ? s.slice(0, -1).join('|') : [...s.slice(0, -1), String(+t - 1)].join('|')
  const parent = byAddr[parentAddr]
  if (!parent) throw new Error('no parent ' + parentAddr)
  const kids = c => Object.keys(nodes).filter(k => nodes[k].parent === c)
  const under = c => [c, ...kids(c).flatMap(under)]
  if (edge === 'direct' && kids(parent).some(k => nodes[k].edge === 'direct' && k !== id)) throw new Error('direct slot taken ' + args.to)
  if (args.alone) {
    if (kids(id).some(k => nodes[k].edge === 'side')) throw new Error('alone with side successors ' + args.at)
    const d = kids(id).find(k => nodes[k].edge === 'direct')
    if (d) Object.assign(nodes[d], { parent: nodes[id].parent, edge: nodes[id].edge, letter: nodes[id].letter })
  } else if (under(id).includes(parent)) throw new Error('move target parent does not exist outside the moved subtree')
  Object.assign(nodes[id], { parent, edge, letter: edge === 'side' ? t : undefined })
  fs.writeFileSync(simFile, JSON.stringify(nodes))
  fs.appendFileSync(log, JSON.stringify(args) + '\n')
  const after = addresses(nodes), map = {}
  for (const k of Object.keys(nodes)) if (before[k] !== after[k]) map[before[k]] = after[k]
  return map
}
`

// The model K/1|a: setting K/1|a|a (kept) with its notation K/1|a|a|a (kept subtree), two chapter placeholders
// K/1|a|a|b, K/1|a|a|c, and a main line 1 → 2 → 3 → 4 → 5, where 2 has a side note and 5 a side chain.
const n = (parent, edge, letter) => ({ parent, edge, letter })
const START = {
  'K/1|a': n(null),
  'K/1|a|a': n('K/1|a', 'side', 'a'),
  'K/1|a|a|a': n('K/1|a|a', 'side', 'a'),
  'K/1|a|a|b': n('K/1|a|a', 'side', 'b'),
  'K/1|a|a|c': n('K/1|a|a', 'side', 'c'),
  'K/1|a|a|1': n('K/1|a|a', 'direct'),
  'K/1|a|a|2': n('K/1|a|a|1', 'direct'),
  'K/1|a|a|2|a': n('K/1|a|a|2', 'side', 'a'),
  'K/1|a|a|3': n('K/1|a|a|2', 'direct'),
  'K/1|a|a|4': n('K/1|a|a|3', 'direct'),
  'K/1|a|a|5': n('K/1|a|a|4', 'direct'),
  'K/1|a|a|5|a': n('K/1|a|a|5', 'side', 'a'),
  'K/1|a|a|5|a|1': n('K/1|a|a|5|a', 'direct'),
}
// Target: chapter b (result 2 with its proof chain 1, and the note), chapter c (result 5 with chain 3 → 4,
// then its side chain).
const SPEC = {
  head: 'K/1|a', keep: ['H|a'], keepSubtrees: ['H|a|a'], after: 'H|a', park: 'H|a|d',
  main: [
    { id: 'H|a|b', sides: [{ id: 'H|a|2', sides: ['H|a|1', 'H|a|2|a'] }] },
    { id: 'H|a|c', sides: [{ id: 'H|a|5', sides: [['H|a|3', 'H|a|4'], ['H|a|5|a', 'H|a|5|a|1']] }] },
  ],
}
// The tree the spec describes, as parent / edge / letter per original Card.
const WANT = {
  'K/1|a|a|b': n('K/1|a|a', 'direct'), 'K/1|a|a|c': n('K/1|a|a|b', 'direct'),
  'K/1|a|a|2': n('K/1|a|a|b', 'side', 'a'), 'K/1|a|a|1': n('K/1|a|a|2', 'side', 'a'), 'K/1|a|a|2|a': n('K/1|a|a|2', 'side', 'b'),
  'K/1|a|a|5': n('K/1|a|a|c', 'side', 'a'), 'K/1|a|a|3': n('K/1|a|a|5', 'side', 'a'), 'K/1|a|a|4': n('K/1|a|a|3', 'direct'),
  'K/1|a|a|5|a': n('K/1|a|a|5', 'side', 'b'), 'K/1|a|a|5|a|1': n('K/1|a|a|5|a', 'direct'),
  'K/1|a|a|a': n('K/1|a|a', 'side', 'a'),
}

function setup() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'tree-mover-test-'))
  fs.copyFileSync(path.join(here, 'tree-mover.mjs'), path.join(dir, 'tree-mover.mjs'))
  fs.writeFileSync(path.join(dir, 'zt.mjs'), FAKE_ZT)
  fs.writeFileSync(path.join(dir, 'spec.json'), JSON.stringify(SPEC))
  return dir
}
const run = (dir, env = {}) => spawnSync(process.execPath, ['tree-mover.mjs', 'spec.json', 'run'],
  { cwd: dir, env: { ...process.env, SIM_DIR: dir, SIM_START: JSON.stringify(START), MOVER_RETRY_MS: '0', ...env }, encoding: 'utf8' })
const moves = dir => fs.readFileSync(path.join(dir, 'moves.log'), 'utf8').split('\n').filter(Boolean).map(l => JSON.parse(l))
const sim = dir => JSON.parse(fs.readFileSync(path.join(dir, 'sim.json'), 'utf8'))
function assertTarget(dir) {
  const s = sim(dir)
  for (const [id, w] of Object.entries(WANT)) assert.deepEqual({ parent: s[id].parent, edge: s[id].edge, letter: s[id].letter }, w, id)
}

test('a run builds the target tree, parking each branch with its Direct chain in one move', () => {
  const dir = setup()
  const r = run(dir)
  assert.equal(r.status, 0, r.stderr)
  assertTarget(dir)
  // 10 Cards placed one by one; parking takes 5 moves (the main chain 1–5, the two Side branches 2|a and
  // 5|a with its chain, the two placeholders), not one per Card.
  const parks = moves(dir).filter(m => !m.alone).length
  assert.equal(parks, 5)
  assert.equal(moves(dir).length, 15)
})

test('a resume starts at the next move: the move made just before the stop is not repeated', () => {
  const clean = setup()
  run(clean)
  const expected = moves(clean)
  const dir = setup()
  const first = run(dir, { FAIL_AT: '2' }) // moves 0 and 1 succeed; move 2 fails twice (the retry too)
  assert.notEqual(first.status, 0)
  assert.match(first.stderr, /failed twice/)
  assert.equal(moves(dir).length, 2)
  assert.equal(JSON.parse(fs.readFileSync(path.join(dir, 'spec.state.json'), 'utf8')).step, 2)
  const second = run(dir)
  assert.equal(second.status, 0, second.stderr)
  assert.deepEqual(moves(dir), expected)
  assertTarget(dir)
})

test('a failed move is retried once before the run stops', () => {
  const dir = setup()
  const r = run(dir, { FAIL_AT: '3', FAIL_ONCE: '1' })
  assert.equal(r.status, 0, r.stderr)
  assertTarget(dir)
})

test('a state file from the older mover (no planner field) keeps the per-Card parking it started with', () => {
  const dir = setup()
  const ids = Object.keys(START).filter(a => a.startsWith('K/1|a|a|') && a !== 'K/1|a|a|a')
  fs.writeFileSync(path.join(dir, 'spec.state.json'), JSON.stringify({ cur: Object.fromEntries([...ids, 'K/1|a|a'].map(a => [a, a])), step: 0, tail: null }))
  const r = run(dir)
  assert.equal(r.status, 0, r.stderr)
  assertTarget(dir)
  assert.equal(moves(dir).filter(m => !m.alone).length, 10) // one park per Card
})
