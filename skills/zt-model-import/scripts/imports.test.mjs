// node --test imports.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import { propose, rewrite, audit } from './imports.mjs'

// One model as get_model reports it: H rests on parts of A (K/2|a) and has a legacy share of C.
const H = 'K/7|a'
const model = (over = {}) => ({
  head: { address: H, body: '' },
  build: {
    order: [
      { address: 'K/3|a|a|2', line: 'shared', legacy: true },
      { address: H + '|a', line: 'main' },
      { address: H + '|a|1', line: 'main' },
      { address: 'K/9|a|a', line: 'shared', import: 'K/9|a|a' },
    ],
    closure: [
      { from: H + '|a', to: 'K/2|a|a', note: '依赖 A 的设定', card: { address: 'K/2|a|a', title: '设定：A', type: 'model', facet: '设定', models: ['K/2|a'] } },
      { from: H + '|a|1', to: 'K/2|a|a|b', note: '依赖 A 的时点', card: { address: 'K/2|a|a|b', title: '设定：时点', type: 'model', facet: '设定：时点', models: ['K/2|a'] } },
      { from: H + '|a|1', to: 'K/2|a', note: '依赖整个 A', card: { address: 'K/2|a', title: '模型 A', type: 'model', facet: null, models: ['K/2|a'] } },
      { from: H + '|a|1', to: 'K/1', note: '依赖一张想法卡', card: { address: 'K/1', title: '想法', type: 'idea', facet: null, models: [] } },
    ],
    tolerated: [
      { part: 'K/9|a|a', home: 'K/9|a', premise: { address: 'K/9|a|a|1', models: ['K/9|a'] } },
      { part: 'K/3|a|a|2', home: 'K/3|a', premise: { address: 'K/5|a|a', models: ['K/5|a'] } },
    ],
    false_share: [{ target: 'K/4|a|a|1', note: '对照 B 的一步' }, { target: 'K/6|a|a', note: '对照 D' }],
    import_target: [],
    import_cycle: [],
  },
  imports: [
    { root: 'K/9|a|a', title: '设定：E', mode: 'card', home: 'K/9|a', members: ['K/9|a|a'], closure: [] },
    { root: 'K/3|a|a|2', title: '推导：C 的一步', mode: 'legacy', home: 'K/3|a', members: ['K/3|a|a|2'], closure: [] },
  ],
  relations: [{ from: H + '|a|1', to: 'K/4|a|a|1', kind: 'rests-on' }],
  neighbours: [{ way: 'out', from: H + '|a', to: 'K/8|a|a', kind: 'corresponds', card: { address: 'K/8|a|a', type: 'model', facet: '设定' } }],
  ...over,
})

test('plan: an outside premise in another model is imported, one 整块 line when several lie in one block (#205 skill)', () => {
  const p = propose(model())
  const imports = p.changes.filter((c) => c.why === 'premise')
  assert.deepEqual(imports.map((c) => [c.root, c.mode, c.absorbed]), [['K/2|a|a', '整块', ['K/2|a|a|b']]])
  assert.equal(imports[0].text, '设定：A')
  // A Link to a whole Model Card and a premise that is no model's part are named, never imported here.
  assert.deepEqual(p.manual.map((m) => m.kind).sort(), ['copy', 'not a part', 'whole model'])
})

test('plan: a 单卡 import whose tolerated premise lies in its home model becomes 整块; a premise elsewhere does not', () => {
  const p = propose(model())
  assert.deepEqual(p.changes.filter((c) => c.why === 'upgrade').map((c) => [c.root, c.mode]), [['K/9|a|a', '整块']])
})

test('plan: a legacy share becomes an import line; a false share that a part rests on is imported, a comparison is left to the reader', () => {
  const p = propose(model())
  assert.deepEqual(p.changes.filter((c) => c.why === 'legacy').map((c) => [c.root, c.mode]), [['K/3|a|a|2', '单卡']])
  assert.deepEqual(p.changes.filter((c) => c.why === 'false share').map((c) => [c.root, c.mode]), [['K/4|a|a|1', '单卡']])
  assert.deepEqual(p.advice.map((a) => a.target), ['K/6|a|a'])
})

test('plan: a model with nothing to import proposes nothing', () => {
  const clean = model({ build: { order: [], closure: [], tolerated: [], false_share: [], import_target: [], import_cycle: [] }, imports: [], relations: [], neighbours: [] })
  const p = propose(clean)
  assert.deepEqual([p.changes, p.manual, p.advice], [[], [], []])
})

test('apply: the body gains its 引入 lines, an upgrade rewrites the mode word, absorbed lines and legacy （共用） lines go', () => {
  const body = '问题：H？\n\n**引入**\n\n- 单卡 [[K/9|a|a]]：E 的设定\n- 单卡 [[K/2|a|a|b]]：A 的时点\n\n**结构**\n\n- 设定 [[K/7|a|a]]（核心）：设定\n- 共用 [[K/3|a|a|2]]（共用）：C 的一步\n\n来源：[[K]]'
  const out = rewrite(body, [
    { root: 'K/2|a|a', mode: '整块', text: '设定：A', absorbed: ['K/2|a|a|b'] },
    { root: 'K/9|a|a', mode: '整块', text: '', absorbed: [] },
    { root: 'K/3|a|a|2', mode: '单卡', text: 'C 的一步', absorbed: [] },
  ])
  assert.equal(out, '问题：H？\n\n**引入**\n\n- 整块 [[K/9|a|a]]：E 的设定\n- 整块 [[K/2|a|a]]：设定：A\n- 单卡 [[K/3|a|a|2]]：C 的一步\n\n**结构**\n\n- 设定 [[K/7|a|a]]（核心）：设定\n\n来源：[[K]]')
  // Without a section, it is made before 结构.
  assert.equal(rewrite('问题\n\n**结构**\n\n- a', [{ root: 'K/2|a|a', mode: '单卡', text: 'x', absorbed: [] }]), '问题\n\n**引入**\n\n- 单卡 [[K/2|a|a]]：x\n\n**结构**\n\n- a')
})

test('check: import faults, premises in other models and legacy lines fail; tolerated, false shares and copies are advice', () => {
  const bad = audit(model())
  assert.equal(bad.ok, false)
  assert.deepEqual([...new Set(bad.faults.map((f) => f.kind))], ['outside premise', 'legacy share'])
  const good = audit(model({
    build: { order: [], closure: [], tolerated: [{ part: 'K/9|a|a', home: 'K/9|a', premise: { address: 'K/5|a', models: ['K/5|a'] } }], false_share: [], import_target: [], import_cycle: [] },
    imports: [{ root: 'K/9|a|a', mode: 'block', home: 'K/9|a', members: ['K/9|a|a'], closure: [] }],
    neighbours: [],
  }))
  assert.equal(good.ok, true)
  assert.deepEqual(good.advice.map((a) => a.kind), ['tolerated'])
})
