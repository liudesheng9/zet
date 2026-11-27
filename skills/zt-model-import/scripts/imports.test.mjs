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

test('plan: in an empirical model, a 单卡 import that only 假说 parts rest on is the prediction under test and stays 单卡', () => {
  // hansen2021demand/5|c|e and becker2025division/2|g|f on the live box (2026-11-09): the tested theory result
  // was proposed as 整块, which would take the whole proof of the prediction into the test (proposal §7, §9.4).
  const m = model({ level: 'empirical', relations: [{ from: H + '|a|1', to: 'K/9|a|a', kind: 'rests-on' }] })
  m.build.order = m.build.order.map((o) => (o.address === H + '|a|1' ? { ...o, role: 'hypothesis' } : o))
  assert.deepEqual(propose(m).changes.filter((c) => c.why === 'upgrade'), [])
  // A part of another role that rests on the result still needs its premises: the upgrade stands.
  m.build.order = m.build.order.map((o) => (o.address === H + '|a|1' ? { ...o, role: 'spec' } : o))
  assert.deepEqual(propose(m).changes.filter((c) => c.why === 'upgrade').map((c) => c.root), ['K/9|a|a'])
})

test('plan: a tested prediction stays 单卡 when another imported prediction rests on it', () => {
  // brandimarti2026reorganizing/7|e (2026-11-10): the hypothesis imports corollary A.5(i) and A.5(ii); A.5(ii)
  // rests on A.5(i) in its home model. Only this model's own parts decide whether a root is under test.
  const m = model({ level: 'empirical', relations: [
    { from: H + '|a|1', to: 'K/9|a|a', kind: 'rests-on' },
    { from: 'K/9|a|b', to: 'K/9|a|a', kind: 'rests-on' },
  ] })
  m.build.order = [...m.build.order.map((o) => (o.address === H + '|a|1' ? { ...o, role: 'hypothesis' } : o)),
    { address: 'K/9|a|b', line: 'shared', import: 'K/9|a|b', role: 'result' }]
  assert.deepEqual(propose(m).changes.filter((c) => c.why === 'upgrade'), [])
})

test('plan: in a formal model, a 校准 part importing an empirical 估计 stays 单卡 across premise, legacy, and upgrade paths', () => {
  const cal = H + '|a|2|a'
  const est = 'K/9|a|1|a'
  const m = {
    level: 'formal',
    head: { address: H, body: '' },
    build: {
      order: [
        { address: H + '|a', line: 'main', facet: '设定', role: 'setting' },
        { address: cal, line: 'side', facet: '校准：稳态识别', role: 'calibration' },
        { address: est, line: 'shared', import: est, facet: '估计：瑞士便利收益', home: 'K/9|a', home_level: 'empirical', home_role: 'estimate' },
        { address: 'K/8|a|1|a', line: 'shared', legacy: true, facet: '估计：旧估计', home: 'K/8|a', home_level: 'empirical', home_role: 'estimate' },
      ],
      closure: [
        { from: cal, to: 'K/6|a|1|a', note: '依赖实证估计', card: { address: 'K/6|a|1|a', title: '估计：287 bp', type: 'model', facet: '估计：基准', models: ['K/6|a'] } },
        { from: cal, to: 'K/6|a|1|a|a', note: '依赖检验', card: { address: 'K/6|a|1|a|a', title: '检验：稳健性', type: 'model', facet: '检验：稳健性', models: ['K/6|a'] } },
      ],
      tolerated: [
        { part: est, home: 'K/9|a', premise: { address: 'K/9|a|b', models: ['K/9|a'] } },
        { part: 'K/8|a|1|a', home: 'K/8|a', premise: { address: 'K/8|a|b', models: ['K/8|a'] } },
      ],
      false_share: [],
      import_target: [],
      import_cycle: [],
    },
    imports: [
      { root: est, title: '估计：瑞士便利收益', mode: 'card', home: 'K/9|a', members: [est], closure: [] },
      { root: 'K/8|a|1|a', title: '估计：旧估计', mode: 'legacy', home: 'K/8|a', members: ['K/8|a|1|a'], closure: [] },
    ],
    relations: [
      { from: cal, to: est, kind: 'rests-on' },
      { from: cal, to: 'K/8|a|1|a', kind: 'rests-on' },
      { from: cal, to: 'K/6|a|1|a', kind: 'rests-on' },
    ],
    neighbours: [],
  }
  const p = propose(m)
  // 1) Repeat-plan upgrade: stays 单卡 (no upgrade proposed)
  assert.deepEqual(p.changes.filter((c) => c.why === 'upgrade'), [])
  // 2) Legacy conversion: stays 单卡 despite tolerated premise in home model
  assert.deepEqual(p.changes.filter((c) => c.why === 'legacy').map((c) => [c.root, c.mode]), [['K/8|a|1|a', '单卡']])
  // 3) Outside premise (Rust-shaped: edge is ONLY in build.closure, NOT in m.relations): stays 单卡!
  const mRustClosureOnly = { ...m, relations: m.relations.filter((r) => r.to !== 'K/6|a|1|a') }
  const pRust = propose(mRustClosureOnly)
  assert.deepEqual(pRust.changes.filter((c) => c.why === 'premise' && c.root === 'K/6|a|1|a').map((c) => c.mode), ['单卡'])
  // 4) Empirical Hypothesis resting on a formal Setting is NOT protected as 单卡 (upgrades to 整块)
  const empOnSetting = model({
    level: 'empirical',
    relations: [{ from: H + '|a|1', to: 'K/9|a|a', kind: 'rests-on' }],
  })
  empOnSetting.build.order = empOnSetting.build.order.map((o) => {
    if (o.address === H + '|a|1') return { ...o, role: 'hypothesis' }
    if (o.address === 'K/9|a|a') return { ...o, facet: '设定：基础环境', home_level: 'formal', home_role: 'setting' }
    return o
  })
  assert.deepEqual(propose(empOnSetting).changes.filter((c) => c.why === 'upgrade').map((c) => c.root), ['K/9|a|a'])
  // 5) When a Setting (non-calibration) also rests on est, upgrade to 整块 IS proposed
  const mMixed = { ...m, relations: [...m.relations, { from: H + '|a', to: est, kind: 'rests-on' }] }
  assert.deepEqual(propose(mMixed).changes.filter((c) => c.why === 'upgrade').map((c) => c.root), [est])
  // 6) When a parent block encloses a protected 单卡 estimate AND another child, the parent 整块 is withheld into manual
  const mEnclosing = {
    level: 'formal',
    head: { address: H, body: '' },
    build: {
      order: [
        { address: H + '|a', line: 'main', facet: '设定', role: 'setting' },
        { address: cal, line: 'side', facet: '校准：稳态识别', role: 'calibration' },
      ],
      closure: [
        { from: H + '|a', to: 'K/6|a|1', note: '依赖章', card: { address: 'K/6|a|1', title: '第 1 章', type: 'model', facet: '第 1 章', models: ['K/6|a'] } },
        { from: cal, to: 'K/6|a|1|a', note: '依赖估计', card: { address: 'K/6|a|1|a', title: '估计：287 bp', type: 'model', facet: '估计：基准', home_level: 'empirical', home_role: 'estimate', models: ['K/6|a'] } },
        { from: H + '|a', to: 'K/6|a|1|b', note: '依赖表', card: { address: 'K/6|a|1|b', title: '表 1', type: 'model', facet: '表 1', models: ['K/6|a'] } },
      ],
      tolerated: [],
      false_share: [],
      import_target: [],
      import_cycle: [],
    },
    imports: [],
    relations: [],
    neighbours: [],
  }
  const pEnc = propose(mEnclosing)
  assert.equal(pEnc.changes.some((c) => c.root === 'K/6|a|1' && c.mode === '整块'), false, 'enclosing 整块 must be withheld on first plan')
  assert.ok(pEnc.manual.some((x) => x.kind === 'mixed singleton' && x.target === 'K/6|a|1'))

  // 7) Repeat plan after K/6|a|1|a has already been applied as 单卡: enclosing parent 整块 STILL withheld!
  const mRepeatEnc = {
    ...mEnclosing,
    build: {
      ...mEnclosing.build,
      order: [
        ...mEnclosing.build.order,
        { address: 'K/6|a|1|a', line: 'shared', import: 'K/6|a|1|a', facet: '估计：基准', home: 'K/6|a', home_level: 'empirical', home_role: 'estimate' },
      ],
      closure: mEnclosing.build.closure.filter((c) => c.to !== 'K/6|a|1|a'),
    },
    imports: [{ root: 'K/6|a|1|a', title: '估计：287 bp', mode: 'card', home: 'K/6|a', members: ['K/6|a|1|a'], closure: [] }],
    relations: [{ from: cal, to: 'K/6|a|1|a', kind: 'rests-on' }],
  }
  const pRep = propose(mRepeatEnc)
  assert.equal(pRep.changes.some((c) => c.root === 'K/6|a|1' && c.mode === '整块'), false, 'enclosing 整块 must stay withheld on repeat plan')
  assert.ok(pRep.manual.some((x) => x.kind === 'mixed singleton' && x.target === 'K/6|a|1'))

  // 8) When BOTH parent and estimate are already imported as 单卡, a tolerated premise on parent still withholds 整块 upgrade!
  const mBothImported = {
    ...mRepeatEnc,
    build: {
      ...mRepeatEnc.build,
      order: [
        ...mRepeatEnc.build.order,
        { address: 'K/6|a|1', line: 'shared', import: 'K/6|a|1', facet: '第 1 章', home: 'K/6|a', home_level: 'empirical', home_role: 'chapter' },
      ],
      closure: [],
      tolerated: [{ part: 'K/6|a|1', home: 'K/6|a', premise: { address: 'K/6|a|a', models: ['K/6|a'] } }],
    },
    imports: [
      ...mRepeatEnc.imports,
      { root: 'K/6|a|1', title: '第 1 章', mode: 'card', home: 'K/6|a', members: ['K/6|a|1'], closure: [] },
    ],
  }
  const pUpg = propose(mBothImported)
  assert.equal(pUpg.changes.some((c) => c.root === 'K/6|a|1' && c.mode === '整块'), false, 'parent upgrade to 整块 over protected estimate must be withheld')
  assert.ok(pUpg.manual.some((x) => x.kind === 'mixed singleton' && x.target === 'K/6|a|1'))

  // 9) When ONLY a parent and its protected child estimate are pending (parent itself would be 单卡, not 整块),
  //    the parent is NOT falsely withheld: both are proposed as separate 单卡 imports.
  const mParentOnly = {
    ...mEnclosing,
    build: {
      ...mEnclosing.build,
      closure: mEnclosing.build.closure.filter((c) => c.to !== 'K/6|a|1|b'),
    },
  }
  const pPO = propose(mParentOnly)
  assert.deepEqual(pPO.changes.map((c) => [c.root, c.mode]).sort(), [['K/6|a|1', '单卡'], ['K/6|a|1|a', '单卡']])
  assert.equal(pPO.manual.some((x) => x.kind === 'mixed singleton'), false)
})
