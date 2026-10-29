// node --test check-settings.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import { audit } from './check-settings.mjs'

const S = 'K/4|a|a'
const row = (address, facet, title, line = 'side') => ({ address, facet, title, line })
const order = [
  row(S, '设定', '设定：参与者与时点', 'main'),
  row(S + '|a', '符号表与读法', '符号表：全模型的符号'),
  row(S + '|b', '设定：信息', '设定：两类信息不对称'),
  row(S + '|c', '推导：管理跨度项', '推导：管理跨度项'),
  row(S + '|1', '第 1 章：直觉', '第 1 章：分工减少错配', 'main'),
  row(S + '|1|a', '设定', '设定：异质工厂与工人'),
  row(S + '|2', '第 2 章：生产', '第 2 章：生产技术', 'main'),
  row(S + '|2|a', '结果：效率', '结果：专业化提高效率'),
  row(S + '|2|a|a', '任务配置', '设定：任务配置与任务范围 (2)'),
  row(S + '|2|a|a|1', '推导：效率 (4)', '推导：由错配得到效率 (4)'),
  row(S + '|3', '第 3 章：第二阶段', '第 3 章：第二阶段', 'main'),
  row(S + '|3|a', '本阶段的设定', '设定：第二阶段只选职业数量'),
  row(S + '|3|a|1', '利润 (11)', '利润：收入减成本'),
  row(S + '|3|b', '结果：最优性', '结果：利润单峰'),
]
const body = {
  [S + '|a']: '| 符号 | 含义 |\n|---|---|\n| $\\lambda$ | 工人效率 |\n| $H$ | 辅助量 |',
  [S + '|2|a|a|1']: '**步骤**\n1. 记 $H=\\eta+\\pi x$，对 $H$ 求导。\n2. 记 $Q(\\omega)=C-\\gamma H$。',
  [S + '|3|b']: '**本章设定**\n- 工厂已知第三阶段的解。\n\n**结果**\n利润单峰。\n- 假设 $\\gamma>\\sigma-1$，它保证单峰。\n- 由于假设 $\\gamma>\\sigma-1$，指数为正。',
}
const cards = new Map(order.map((r) => [r.address, { ...r, body: body[r.address] || '' }]))

test('every setting, assumption and notation lives in the setting part; nothing else does', () => {
  const r = audit({ build: { setting: S, order } }, cards)
  assert.deepEqual(r.outside.map((f) => f.card), [S + '|1|a', S + '|2|a|a', S + '|3|a'])
  // A setting's own derivation does not go with it: the derivation stays in its chapter.
  assert.deepEqual(r.outside.find((f) => f.card === S + '|3|a').leave, [S + '|3|a|1'])
  assert.deepEqual(r.inside.map((f) => f.card), [S + '|c'])
  assert.equal(r.ok, false)
})

test('an assumption stated in a chapter card is a fault; a use of one is advice; local notation must be in the notation card', () => {
  const r = audit({ build: { setting: S, order } }, cards)
  assert.deepEqual(r.stated.map((f) => [f.card, f.what]), [[S + '|3|b', '**本章设定**'], [S + '|3|b', '假设 $\\gamma>\\sigma-1$，它保证单峰。']])
  assert.deepEqual(r.uses.map((f) => f.card), [S + '|3|b'])
  // H is in the notation card; Q(ω) is not.
  assert.deepEqual(r.notation.map((f) => [f.card, f.symbol]), [[S + '|2|a|a|1', 'Q(\\omega)']])
})

const start = (deps) => '**起点**\n' + (deps.length ? deps.map((d) => `- 依赖 [[${d}]] 的设定。`).join('\n') : '- 原始设定：不依赖其他设定。') + '\n\n**内容**\n1. x'
const tree = (rows) => {
  const order = rows.map(([address, facet, parent, deps, body = '']) => ({ address, facet, title: facet, line: parent ? 'side' : 'main' }))
  const cards = new Map(rows.map(([address, facet, , deps, body = '']) => [address, { facet, title: facet, body: deps === null ? body : start(deps) + body }]))
  const parents = new Map(rows.map(([address, , parent]) => [address, parent]))
  const relations = rows.flatMap(([from, , , deps]) => (deps || []).map((to) => ({ from, to, kind: 'rests-on' })))
  return { m: { build: { setting: S, order }, relations }, cards, parents }
}

test('a setting part reads as a dependency tree: each setting declares its start, rests on its parent, and on nothing later', () => {
  const { m, cards, parents } = tree([
    [S, '设定', null, []],
    [S + '|a', '符号表与读法', S, null, '| 符号 | 含义 |\n| $L$ | 工人数 |\n| $b(\\omega)$ | 任务范围 |'],
    [S + '|b', '设定：任务配置 (2)', S, [S], '\n任务范围 $b(\\omega)$，$L$ 个工人。'],
    [S + '|b|1', '设定：信息', S + '|b', [S + '|b']],
    [S + '|b|2', '设定：加总', S + '|b|1', [S + '|b']],          // filed after the information, rests only on b
    [S + '|c', '设定：时序', S, [S + '|d']],                      // rests on a later setting
    [S + '|d', '设定：开放经济', S, null, '**内容**\n1. 两国。'],   // no start section
    [S + '|1', '第 1 章：直觉', S, [S]],
  ])
  const r = audit(m, cards, parents)
  assert.deepEqual(r.start.map((f) => f.card), [S + '|d'])
  assert.deepEqual(r.misfiled.map((f) => [f.card, f.parent]), [[S + '|b|2', S + '|b|1']])
  assert.deepEqual(r.order.map((f) => [f.card, f.premise]), [[S + '|c', S + '|d']])
  // The reading aid: the notation symbols each setting uses first, in reading order.
  assert.deepEqual(r.introduces.find((x) => x.card === S + '|b').symbols, ['L', 'b(\\omega)'])
  assert.equal(r.ok, false)
})

test('a clean model passes', () => {
  const keep = order.filter((o) => [S, S + '|a', S + '|b', S + '|1', S + '|2', S + '|2|a'].includes(o.address))
  const clean = new Map([...cards].map(([a, c]) => [a, isSettingRow(c) ? { ...c, body: start(a === S ? [] : [S]) } : c]))
  const r = audit({ build: { setting: S, order: keep }, relations: [{ from: S + '|b', to: S, kind: 'rests-on' }] }, clean, new Map(keep.map((o) => [o.address, o.address === S ? null : S])))
  assert.equal(r.ok, true, JSON.stringify(r))
})

function isSettingRow(c) { return /^设定/.test(c.facet) || /^设定/.test(c.title) }
