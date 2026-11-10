// node --test check-settings.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import { audit, notationSymbols } from './check-settings.mjs'

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

test('a definition written with := names the symbol before it, without the colon', () => {
  const defs = new Map(cards)
  defs.set(S + '|2|a|a|1', { ...cards.get(S + '|2|a|a|1'), body: '**步骤**\n1. 记 $H:=\\eta+\\pi x$，对 $H$ 求导。\n2. 记 $Q(\\omega):=C-\\gamma H$。' })
  const r = audit({ build: { setting: S, order } }, defs)
  assert.deepEqual(r.notation.map((f) => [f.card, f.symbol]), [[S + '|2|a|a|1', 'Q(\\omega)']])
})

const start = (deps) => '**起点**\n' + (deps.length ? deps.map((d) => `- 依赖 [[${d}]] 的设定。`).join('\n') : '- 原始设定：不依赖其他设定。') + '\n\n**设定**\n1. x'
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

test('notation symbols: split outside brackets, TeX spaces dropped, cut at = and \\in', () => {
  assert.deepEqual(notationSymbols('| 符号 | 含义 |\n| --- | --- |\n| $L,\\ Z=(Z_1,\\dots,Z_N)$ | 要素 |\n| $\\lambda(i,\\omega)$、$\\theta\\in\\Theta$ | x |'), ['L', 'Z', '\\lambda(i,\\omega)', '\\theta'])
})

test('a clean model passes', () => {
  const keep = order.filter((o) => [S, S + '|a', S + '|b', S + '|1', S + '|2', S + '|2|a'].includes(o.address))
  // The notation card lists only symbols the model uses.
  const clean = new Map([...cards].map(([a, c]) => [a, isSettingRow(c) ? { ...c, body: start(a === S ? [] : [S]) } : a === S + '|a' ? { ...c, body: '' } : c]))
  const r = audit({ build: { setting: S, order: keep }, relations: [{ from: S + '|b', to: S, kind: 'rests-on' }] }, clean, new Map(keep.map((o) => [o.address, o.address === S ? null : S])))
  assert.equal(r.ok, true, JSON.stringify(r))
})

function isSettingRow(c) { return /^设定/.test(c.facet) || /^设定/.test(c.title) }

test('a setting shared from another model is not filed outside: its home model checks where it is filed', () => {
  const shared = [...order, row('K/9|a|a|b', '设定：现金流量规则', '规则：资金流入视同销售', 'shared')]
  const cs = new Map([...cards, ['K/9|a|a|b', { ...shared.at(-1), body: '' }]])
  const r = audit({ build: { setting: S, order: shared } }, cs)
  assert.ok(!r.outside.some((f) => f.card === 'K/9|a|a|b'))
})

test('imported notation counts as defined; borrowed Cards are not audited; 沿用 … 的全部符号 replaces an own notation card (#211)', () => {
  const H = 'K/7|a'
  const own = H + '|a'
  const rows = [
    { address: 'K/4|a|a', facet: '设定', title: '设定：A', line: 'shared', import: 'K/4|a|a' },
    { address: 'K/4|a|a|a', facet: '符号表与读法', title: '符号表：A', line: 'shared', import: 'K/4|a|a' },
    { address: 'K/4|a|a|1', facet: '推导：A 的一步', title: '推导：A 的一步', line: 'shared', legacy: true },
    { address: own, facet: '设定', title: '设定：B', line: 'main' },
    { address: own + '|1', facet: '推导：B 的一步', title: '推导：B 的一步', line: 'main' },
  ]
  const bodies = {
    'K/4|a|a|a': '| 符号 | 含义 |\n|---|---|\n| $x$ | 产量 |',
    'K/4|a|a|1': '**本章设定**\n- 他模型自己的设定。\n记 $Z$ 为他模型的量。',
    [own + '|1']: '记 $x$ 为产量。',
    [own]: '**起点**\n- 原始设定。',
  }
  const cards = new Map(rows.map((r) => [r.address, { ...r, body: bodies[r.address] || '' }]))
  const r = audit({ build: { setting: own, order: rows } }, cards)
  assert.deepEqual(r.missing, [], 'an imported notation card serves the borrower')
  assert.deepEqual(r.notation, [], 'x is defined by the imported notation card; a borrowed card is not audited')
  assert.deepEqual(r.stated, [], 'a borrowed card states its own home model\'s settings')
  // Without the import, a 沿用 line in the setting replaces the own notation card.
  const bare = rows.filter((x) => x.line !== 'shared')
  const bind = new Map([...cards].filter(([a]) => !a.startsWith('K/4')))
  bind.set(own, { ...bind.get(own), body: '**起点**\n- 原始设定。\n\n沿用 [[K/4|a|a]] 的全部符号。' })
  assert.deepEqual(audit({ build: { setting: own, order: bare } }, bind).missing, [])
  bind.set(own, { ...bind.get(own), body: '**起点**\n- 原始设定。' })
  assert.equal(audit({ build: { setting: own, order: bare } }, bind).missing.length, 1)
})

// The stricter pass (2026-11-03): placement, symbol coverage, notation rows, card format, assumptions.
const H = 'K/2|a'
const P = H + '|a'
const strict = (rows, extra = {}) => {
  const order = rows.map(([address, facet, title, , line = 'side']) => ({ address, facet, title, line }))
  const cards = new Map(rows.map(([address, facet, title, body]) => [address, { facet, title, body }]))
  const parents = new Map(rows.map(([address]) => [address, address === P ? H : address.slice(0, address.lastIndexOf('|'))]))
  const relations = rows.flatMap(([from, , , body]) => [...(body || '').matchAll(/依赖 \[\[([^\]]+)\]\]/g)].map((x) => ({ from, to: x[1], kind: 'rests-on' })))
  return audit({ head: { address: extra.head || H }, build: { setting: extra.setting || P, order }, relations }, cards, parents)
}
const NOTE = '| 符号 | 含义 | 首次出现 |\n| --- | --- | --- |\n| $L$ | 工人数 | [[' + P + ']] |\n| $\\bar Z$ | 资本总量 | [[' + P + '|b]] |\n| $\\theta,\\Theta$ | 技术 | [[' + P + '|b]] |'
const good = [
  [P, '设定', '设定：工人与资本', '**起点**\n- 原始设定：x。\n\n**设定**\n1. 有 $L$ 个工人。', 'main'],
  [P + '|a', '符号表与读法', '符号表与读法', NOTE],
  [P + '|b', '设定：技术', '设定：技术与资本', '**起点**\n- 依赖 [[' + P + ']] 的设定：工人。\n\n**设定**\n1. 资本总量为 $\\bar Z$。\n2. 技术 $\\theta\\in\\Theta$。\n\n**说明**\n技术由企业选。'],
  [P + '|c', '假设 1', '假设 1：凹性', '**起点**\n- 依赖 [[' + P + '|b]] 的设定：技术。\n\n**假设**\n1. $L$ 与 $\\theta$ 上严格凹。'],
  [P + '|1', '第 1 章：直觉', '第 1 章：直觉', '在[[假设 1@' + P + '|c]]下，解唯一。', 'main'],
]

test('a clean strict model passes', () => {
  const r = strict(good)
  assert.equal(r.ok, true, JSON.stringify(r, null, 1))
})

test('place: the setting card is the first Side of the Model Card, the notation card the first Side of the setting card', () => {
  const r = strict(good.map(([a, ...x]) => [a.replace(P, H + '|b'), ...x.map((v) => typeof v === 'string' ? v.replaceAll(P, H + '|b') : v)]), { setting: H + '|b' })
  assert.deepEqual(r.place.map((f) => f.card), [H + '|b'])
  const moved = good.map(([a, f, t, b, l]) => [a === P + '|a' ? P + '|d' : a, f, t, b, l])
  assert.deepEqual(strict(moved).place.map((f) => f.card), [P + '|d'])
})

test('uncovered: every symbol a setting uses is in the notation card; indices in sub- and superscripts and \\text are not symbols', () => {
  const rows = good.map((r) => r[0] === P + '|b' ? [r[0], r[1], r[2], r[3] + '\n3. 产出 $Y^i=A_Z\\,\\bar Z^{\\alpha}$，$\\text{cost}$，$\\frac{dY}{dZ}$，$\\theta\'\\in\\mathbb R^K$。'] : r)
  const r = strict(rows)
  assert.deepEqual(r.uncovered.map((f) => [f.card, f.symbols]), [[P + '|b', ['Y', 'A', 'Z']]])
})

test('unused and first: a notation row names symbols some model card uses, and its first card is where the reader meets it', () => {
  const note = NOTE + '\n| $\\omega$ | 不再使用 | [[' + P + '|b]] |\n| $\\bar Z$ | 重复 | [[' + P + ']] |'
  const r = strict(good.map((r) => r[0] === P + '|a' ? [r[0], r[1], r[2], note] : r))
  assert.deepEqual(r.unused.map((f) => f.symbol), ['\\omega'])
  assert.deepEqual(r.first.map((f) => [f.symbol, f.listed, f.reads]), [['\\bar Z', P, P + '|b']])
})

test('format: 起点 first, then 设定 (假设 for an assumption), then 说明; no local symbol or term table; no 依赖 outside 起点; title names the kind', () => {
  const rows = good.map((r) => {
    if (r[0] === P + '|b') return [r[0], r[1], '技术与资本', '**起点**\n- 依赖 [[' + P + ']] 的设定。\n\n资本总量为 $\\bar Z$。\n\n依赖 [[' + P + ']]：工人。\n\n| 符号 | 含义 |\n| --- | --- |\n| $\\bar Z$ | 资本 |']
    if (r[0] === P + '|c') return [r[0], r[1], r[2], '**起点**\n- 依赖 [[' + P + '|b]] 的设定。\n\n**设定**\n1. 凹。']
    return r
  })
  const r = strict(rows)
  const what = (a) => r.format.filter((f) => f.card === a).map((f) => f.what)
  assert.deepEqual(what(P + '|b'), ['title', 'section', 'depends', 'table'])
  assert.deepEqual(what(P + '|c'), ['section'])
})

test('repeat: one assumption is stated in one card; cite: a card that names an assumption links its card', () => {
  const rows = [...good.slice(0, 4),
    [P + '|c|1', '假设 1：变体', '假设 1：在变体中', '**起点**\n- 依赖 [[' + P + '|c]] 的设定。\n\n**假设**\n1. $L$ 与 $\\theta$ 上严格凹。'],
    [P + '|1', '第 1 章：直觉', '第 1 章：直觉', '在假设 1 下，解唯一。\n由[[假设 1@' + P + '|c]]，解唯一。', 'main']]
  const r = strict(rows)
  assert.deepEqual(r.repeat.map((f) => [f.card, f.other]), [[P + '|c|1', P + '|c']])
  assert.deepEqual(r.cite.map((f) => [f.card, f.name]), [[P + '|1', '假设 1']])
  // A weaker assumption with its own number runs parallel to the one it weakens: not a repeat.
  const weaker = rows.map((x) => x[0] === P + '|c|1' ? [x[0], '假设 2', '假设 2：只对要素', x[3]] : x)
  assert.deepEqual(strict(weaker).repeat, [])
})

test('balance advice: an overloaded setting and a one-line setting; a one-line assumption stays its own card', () => {
  const many = '**起点**\n- 依赖 [[' + P + ']] 的设定。\n\n**设定**\n' + Array.from({ length: 11 }, (_, i) => `${i + 1}. 第 ${i} 条，$L$。`).join('\n')
  const one = '**起点**\n- 依赖 [[' + P + ']] 的设定。\n\n**设定**\n1. 时点是长期，$L$ 不变。'
  const r = strict([...good.map((r) => r[0] === P + '|b' ? [r[0], r[1], r[2], many] : r), [P + '|d', '设定：时点', '设定：长期', one]])
  assert.deepEqual(r.balance.map((f) => [f.card, f.what]), [[P + '|b', 'heavy'], [P + '|d', 'light']])
  assert.equal(r.format.length, 0)
})

test('an empirical model: the data tree and the specification ladder (docs/empirical-models-proposal.md §9.2)', async () => {
  const { auditEmpirical } = await import('./check-settings.mjs')
  const H = 'K/5|c'
  const o = (address, role, line = 'side') => ({ address, role, line, facet: role, title: role })
  const m = {
    level: 'empirical', head: { address: H },
    build: {
      order: [o(H + '|a', 'data', 'main'), o(H + '|a|a', 'variables'), o(H + '|a|1', 'design', 'main'), o(H + '|a|1|a', 'assumption'),
        o(H + '|a|2', 'spec', 'main'), o(H + '|a|2|a', 'spec'), o(H + '|a|3', 'chapter', 'main'), o(H + '|a|3|a', 'estimate'), o(H + '|a|3|b', 'spec'), o(H + '|a|4', 'assumption')],
      variable_uncovered: [{ address: H + '|a|2', symbol: 'Z', fix: 'Z has no row' }],
      estimate_unbound: [], spec_unbound: [], test_unbound: [], empirical_shape: [],
    },
  }
  const cards = new Map([
    [H + '|a|a', { body: '| 变量 | 含义 |\n| --- | --- |\n| $y_{it}$ | 结果 |\n| $\\beta$ | 系数 |\n| $W$ | 未用 |' }],
    [H + '|a|2', { body: '**估计式**\n$$ y_{it} = \\beta X_{it} + Z $$' }],
    [H + '|a|2|a', { body: '**相对基准的改动**\n加入控制变量。' }],
    [H + '|a|3|b', { body: '**估计式**\n书中没有写出估计式。' }],
  ])
  const r = auditEmpirical(m, cards)
  assert.equal(r.place.length, 0)
  assert.deepEqual(r.uncovered.map((x) => x.symbol), ['Z'])
  assert.deepEqual(r.unused.map((x) => x.symbol), ['W'])
  assert.deepEqual(r.variant_change.map((x) => x.card), [H + '|a|3|b'])
  assert.deepEqual(r.spec_in_chapter.map((x) => x.card), [H + '|a|3|b'])
  assert.deepEqual(r.assumption.map((x) => x.card), [H + '|a|4'])
  assert.equal(r.ok, false)
})

test('an empirical model: a Greek letter written in prose uses its 变量表 row', async () => {
  // becker2025division/6|g on the live box (2026-11-09): the 参数 cards write σ, γ and θ in prose, and the
  // check called the rows $\sigma$, $\gamma$ and $\hat \theta$ unused.
  const { auditEmpirical } = await import('./check-settings.mjs')
  const H = 'K/6|g'
  const o = (address, role, line = 'side') => ({ address, role, line, facet: role, title: role })
  const m = {
    level: 'empirical', head: { address: H },
    build: { order: [o(H + '|a', 'data', 'main'), o(H + '|a|a', 'variables'), o(H + '|a|4|c', 'parameter'), o(H + '|a|4|d', 'parameter')], variable_uncovered: [] },
  }
  const cards = new Map([
    [H + '|a|a', { body: '| 变量 | 含义 |\n| --- | --- |\n| $\\sigma$ | 弹性 |\n| $\\hat \\theta$ | 截断点 |\n| $\\eta$ | 未用 |' }],
    [H + '|a|4|c', { body: '需求弹性 σ 的估计值是 5.666。' }],
    [H + '|a|4|d', { body: '截断点 ln θ0* 的估计值。' }],
  ])
  assert.deepEqual(auditEmpirical(m, cards).unused.map((x) => x.symbol), ['\\eta'])
})

test('an empirical model: the data tree names its premises first, and every assumption is a Side of the design', async () => {
  // hansen2021demand/5|c|e on the live box (2026-11-09): the folded 测量 cards had no **起点**, and |a|d rested on
  // its later sibling |a|g. ebrill2001modern/5|a: 假设 2 stood as the Direct successor of 假设 1.
  const { auditEmpirical } = await import('./check-settings.mjs')
  const H = 'K/5|c'
  const o = (address, role, line = 'side') => ({ address, role, line, facet: role, title: role })
  const m = {
    level: 'empirical', head: { address: H },
    build: {
      order: [o(H + '|a', 'data', 'main'), o(H + '|a|a', 'variables'), o(H + '|a|b', 'sample'), o(H + '|a|c', 'measure'), o(H + '|a|d', 'measure'),
        o(H + '|a|1', 'design', 'main'), o(H + '|a|1|a', 'assumption'), o(H + '|a|1|a|1', 'assumption'), o(H + '|a|1|b', 'assumption')],
      variable_uncovered: [],
    },
  }
  const cards = new Map([
    [H + '|a|a', { body: '| 变量 | 含义 |\n| --- | --- |' }],
    [H + '|a|b', { body: '子样本。\n\n**起点**\n- 依赖 [[K/5|c|a]]：这些数据的子样本。' }],
    [H + '|a|c', { body: '一个测量。\n依赖 [[K/5|c|a|d]]：用到那套度量。' }],
    [H + '|a|d', { body: '度量框架。\n\n**起点**\n- 依赖 [[K/5|c|a]]：读这些文本。' }],
  ])
  const r = auditEmpirical(m, cards)
  assert.deepEqual(r.start.map((x) => x.card), [H + '|a|c'])
  assert.deepEqual(r.order.map((x) => x.card), [H + '|a|c'])
  assert.deepEqual(r.assumption.map((x) => x.card), [H + '|a|1|a|1'])
  assert.equal(r.ok, false)
})

test('first: a bare symbol is not met inside a longer row symbol of the same notation card (β inside β_R)', () => {
  const note = NOTE + '\n| $\beta_R$ | 链条系数 | [[' + P + '|b]] |\n| $\beta$ | 第一阶段系数 | [[' + P + '|c]] |'
  const rows = good.map((r) => {
    if (r[0] === P + '|a') return [r[0], r[1], r[2], note]
    if (r[0] === P + '|b') return [r[0], r[1], r[2], r[3] + '\n3. 链条 $C=\beta_R L$。']
    if (r[0] === P + '|c') return [r[0], r[1], r[2], r[3] + '\n2. 第一阶段 $L=\beta\theta$。']
    return r
  })
  const r = strict(rows)
  assert.deepEqual(r.first, [], JSON.stringify(r.first))
  assert.deepEqual(r.unused, [])
  assert.ok(!JSON.stringify(r.introduces.find((x) => (x.card || x.address) === P + '|b') || {}).includes('\\beta"'), JSON.stringify(r.introduces))
})

test('an empirical model: a card another model lends (an imported block) is audited in its home model, not here', async () => {
  const { auditEmpirical } = await import('./check-settings.mjs')
  const H = 'K/5|b'
  const o = (address, role, line = 'side', extra = {}) => ({ address, role, line, facet: role, title: role, ...extra })
  const m = {
    level: 'empirical', head: { address: H },
    build: {
      order: [o('K/5|a|a', 'setting', 'shared', { import: 'K/5|a|a|1|a' }), o('K/5|a|a|c', 'assumption', 'shared', { import: 'K/5|a|a|1|a' }),
        o(H + '|a', 'data', 'main'), o(H + '|a|a', 'variables'), o(H + '|a|1', 'design', 'main'), o(H + '|a|1|a', 'assumption'),
        o(H + '|a|2', 'spec', 'main'), o(H + '|a|3', 'chapter', 'main')],
      estimate_unbound: [], spec_unbound: [], test_unbound: [], empirical_shape: [],
    },
  }
  const cards = new Map([[H + '|a|a', { body: '| 变量 | 含义 |\n| --- | --- |' }]])
  const r = auditEmpirical(m, cards)
  assert.deepEqual(r.assumption, [], JSON.stringify(r.assumption))
})
