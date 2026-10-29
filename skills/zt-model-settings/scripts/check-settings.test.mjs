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

test('a clean model passes', () => {
  const keep = order.filter((o) => [S, S + '|a', S + '|b', S + '|1', S + '|2', S + '|2|a'].includes(o.address))
  const r = audit({ build: { setting: S, order: keep } }, cards)
  assert.equal(r.ok, true, JSON.stringify(r))
})
