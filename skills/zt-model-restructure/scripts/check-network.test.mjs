// node --test check-network.test.mjs   (ZT_LAYOUT may name the GUI's layout.js; default: this repo's)
import test from 'node:test'
import assert from 'node:assert/strict'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { analyse, loadLayout } from './check-network.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
process.env.ZT_LAYOUT ||= path.join(here, '..', '..', '..', 'src', 'gui', 'layout.js')

const H = 'K/4|a'
const child = (address, facet, proof) => ({ address, facet, title: facet + '：x', proof: proof ? { first: proof[0], steps: proof.length, addresses: proof } : null })
const block = (i, head, kind, facet, lead, children) => ({ id: 'b' + i, head, kind, facet, title: facet + '：标题', lead, members: 1 + children.length, part: null, chaptered: true, children })
const fixture = () => ({
  head: { address: H },
  build: { parts: [] },
  blocks: [
    block(0, 'K/4|a|a', 'setting', '设定', null, [child('K/4|a|a|a', '符号表与读法'), child('K/4|a|a|b', '设定：信息'), child('K/4|a|a|c', '推导：管理跨度项')]),
    block(1, 'K/4|a|a|1', 'chapter', '第 1 章：直觉', '很短。', [child('K/4|a|a|1|a', '结果：效率', ['K/4|a|a|1|a|a'])]),
    block(2, 'K/4|a|a|2', 'chapter', '第 2 章：生产', '多一个职业，边际成本降一点，管理跨度成本升一点。', [child('K/4|a|a|2|a', '结果：离散度', ['K/4|a|a|2|a|a'])]),
    block(3, 'K/4|a|a|3', 'chapter', '第 3 章：工资', null, []),
    block(4, 'K/4|a|a|4', 'chapter', '第 4 章：开放', '出口把市场放大，同一家工厂卖得更多，于是愿意多设职业。', [child('K/4|a|a|4|a', '局限')]),
  ],
  block_edges: [
    { from_block: 'b1', to_block: 'b0', kind: 'rests-on', count: 2, pairs: [] },
    { from_block: 'b2', to_block: 'b1', kind: 'rests-on', count: 3, pairs: [] },
    { from_block: 'b0', to_block: 'b2', kind: 'rests-on', count: 1, pairs: [{ from: 'K/4|a|a|b', to: 'K/4|a|a|2|a|a' }, { from: 'K/4|a|a|c', to: 'K/4|a|a|2|a' }] },
    { from_block: 'b4', to_block: 'b2', kind: 'rests-on', count: 1, pairs: [] },
    { from_block: 'b4', to_block: 'b1', kind: 'refers', count: 5, pairs: [] },
  ],
})

test('the network check reads the tree, the faults and their fixes the way the GUI draws them', async () => {
  const L = await loadLayout()
  const r = analyse(fixture(), L)
  assert.deepEqual(r.parent, { b0: null, b1: 'b0', b2: 'b1', b3: null, b4: 'b2' })
  assert.deepEqual(r.forward.map((f) => [f.from_block, f.to_block, f.cards]), [['b0', 'b2', ['K/4|a|a|b', 'K/4|a|a|c']]])
  const [setting, derivation] = r.moves
  assert.match(setting.fix, /K\/4\|a\|a\|b is a setting.*reversed.*keep it in the setting part/, 'a setting keeps its place; its relation is turned')
  assert.doesNotMatch(setting.fix, /file K\/4\|a\|a\|b under/)
  assert.match(derivation.fix, /move K\/4\|a\|a\|c after its latest premise.*K\/4\|a\|a\|2/, 'a derivation filed too early moves')
  assert.deepEqual(r.unanchored.map((u) => u.block), ['b3'])
  assert.deepEqual(r.leads.map((l) => [l.block, l.why]), [['b1', 'short'], ['b3', 'missing']])
  assert.deepEqual(r.empty.map((e) => e.block), ['b3', 'b4'], 'a chapter with no result card (advice)')
  assert.deepEqual(r.moves.map((x) => x.card), ['K/4|a|a|b', 'K/4|a|a|c'], 'one fix per Card')
  assert.equal(r.ok, false)
  assert.ok(r.lines.some((l) => /^ {2}1 /.test(l)) && r.lines.some((l) => /^ {6}4 /.test(l)), r.lines.join('\n'))
  assert.ok(r.threads.head > 0 && r.threads.head < 60, 'paths drawn with the Model Card open')
})

test('a clean spine passes', async () => {
  const L = await loadLayout()
  const m = fixture()
  m.blocks[1].lead = '错配越少，效率越高，代价是更多的管理跨度成本。'
  m.blocks[3].lead = '工资随效率走，离散度取决于绩效敏感度和监督的强弱。'
  m.blocks[3].children = [child('K/4|a|a|3|a', '结果：工资', ['K/4|a|a|3|a|a'])]
  // b4 keeps no result card: advice only, the network is still clean
  m.block_edges = m.block_edges.filter((e) => e.from_block !== 'b0')
  m.block_edges.push({ from_block: 'b3', to_block: 'b2', kind: 'rests-on', count: 1, pairs: [] })
  const r = analyse(m, L)
  assert.equal(r.ok, true, JSON.stringify(r, null, 1))
})

test('a part shared from another model is a premise block before the setting: resting on it is not forward', async () => {
  const L = await loadLayout()
  const m = fixture()
  m.blocks[1].lead = '错配越少，效率越高，代价是更多的管理跨度成本。'
  m.blocks[3].lead = '工资随效率走，离散度取决于绩效敏感度和监督的强弱。'
  m.block_edges = m.block_edges.filter((e) => e.from_block !== 'b0')
  m.block_edges.push({ from_block: 'b3', to_block: 'b2', kind: 'rests-on', count: 1, pairs: [] })
  m.blocks.unshift({ id: 'b5', head: H, kind: 'shared', facet: null, title: '共用前提', lead: null, members: 1, part: null, chaptered: true, children: [child('J/6|b|a|1', '结果：最优税')] })
  m.block_edges.push({ from_block: 'b1', to_block: 'b5', kind: 'rests-on', count: 1, pairs: [{ from: 'K/4|a|a|1|a', to: 'J/6|b|a|1' }] })
  const r = analyse(m, L)
  assert.deepEqual(r.forward, [])
  assert.ok(r.lines.some((l) => /^共用 共用前提/.test(l)), r.lines.join('\n'))
  assert.equal(r.ok, true, JSON.stringify(r, null, 1))
})
