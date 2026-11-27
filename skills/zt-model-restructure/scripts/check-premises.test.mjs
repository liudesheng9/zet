// node --test check-premises.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import { scan } from './check-premises.mjs'

const card = (title, body) => ({ title, body })

test('a card that names an equation a later card derives is a hit; pointers and its own proof are not', () => {
  const order = ['K/s', 'K/s|b', 'K/2|a', 'K/2|a|a', 'K/3|a']
  const cards = new Map([
    ['K/s', card('设定', '一句话：设定。\n\n**内容**\n1. 由式 (2)，参数 β 可以忽略。\n\n**为什么需要它**\n[[K/3|a]] 用这里的 (9) 求工资。')],
    ['K/s|b', card('设定：时序', '**内容**\n1. 三个阶段。')],
    ['K/2|a', card('结果：任务范围 (2)', '**结果**\n$$b=1/(n+1)\\quad(2)$$\n\n**证明**\n证明见 [[K/2|a|a]]，它推出 (4)。')],
    ['K/2|a|a', card('推导：效率 (4)', '**起点**\n- 依赖 [[K/2|a]] 的 (2)。\n\n**结果**\n$$\\lambda\\quad(4)$$')],
    ['K/3|a', card('推导：工资 (9)', '**起点**\n- 依赖 (4)。\n\n**结果**\n$$w\\quad(9)$$')],
  ])
  const hits = scan(order, cards)
  assert.deepEqual(hits.map((h) => [h.card, h.eq, h.derived_by]), [['K/s', '2', 'K/2|a']], JSON.stringify(hits))
  assert.match(hits[0].line, /由式 \(2\)/)
})

test('the notation card is a lookup table; an equation is derived where its display formula carries it', () => {
  const order = ['K/n', 'K/9|e', 'K/10|c', 'K/10|b']
  const cards = new Map([
    ['K/n', { title: '符号表', facet: '符号表与读法', body: '| G | 辅助函数 (33) |' }],
    ['K/9|e', card('对照：完整模型', '**内容**\n$$\\Lambda=G(\\theta) \\tag{33}$$')],
    ['K/10|c', card('结果：反事实', '**结果**\n用式 (33) 算出效应。')],
    ['K/10|b', card('推导：矩条件', '**起点**\n- 由式 (33) 解出 K。')],
  ])
  assert.deepEqual(scan(order, cards), [])
})

test('a premise named with a later card filed first in build order is caught even when it is linked', () => {
  const order = ['K/a', 'K/b']
  const cards = new Map([
    ['K/a', card('推导：错配 (3)', '**起点**\n- 起点公式：λ (4)\n\n**结果**\n$$m\\quad(3)$$')],
    ['K/b', card('推导：效率 (4)', '**结果**\n$$\\lambda\\quad(4)$$')],
  ])
  const hits = scan(order, cards, [{ from: 'K/a', to: 'K/b', kind: 'rests-on' }])
  assert.deepEqual(hits.map((h) => [h.card, h.eq, h.linked]), [['K/a', '4', true]])
})

test('equation numbers count only in the model and its imports: a legacy share from another model neither defines nor is scanned (#211)', () => {
  const order = ['K/9|x', 'K/a', 'K/b']
  const cards = new Map([
    ['K/9|x', card('推导：他模型的式 (2)', '**结果**\n$$y\quad(2)$$\n\n用 (5)。')],
    ['K/a', card('推导：使用', '**起点**\n- 由式 (2) 得到。')],
    ['K/b', card('推导：本模型的式 (2)', '**结果**\n$$z\quad(2)$$')],
  ])
  assert.deepEqual(scan(order, cards), [], 'without a scope the other model hides the use')
  const hits = scan(order, cards, [], new Set(['K/9|x']))
  assert.deepEqual(hits.map((h) => [h.card, h.eq, h.derived_by]), [['K/a', '2', 'K/b']], JSON.stringify(hits))
})

test('formal-v2: Criterion 说明 pointer is skipped without discarding inline later headings, appendix tags (A.1)/(B.3) produce positive hits, and only Result/Benchmark proof routes are exempted', () => {
  const order = ['K/s', 'K/s|b', 'K/s|c', 'K/1|a', 'K/1|a|a', 'K/1|a|a|1']
  const cards = new Map([
    ['K/s', { title: '设定：环境', facet: '设定', body: '**起点**\n原始设定。\n\n**设定**\n先引用 (2.1) 与 (A.1)。' }],
    ['K/s|b', { title: '设定：次级块', facet: '设定：次级块', body: '**起点**\n依赖 [[K/s]]。\n\n**设定**\n$$x = 1 \\tag{2.1}$$' }],
    ['K/s|c', { title: '均衡定义：竞争均衡', facet: '均衡定义：竞争均衡', body: '**起点**\n依赖 [[K/s]]。\n\n**均衡定义**\n价格出清。\n\n**说明**\n存在性见后文式 (A.1)。\n\n**作用域** 在式 (B.3) 的约束下成立。' }],
    ['K/1|a', { title: '结果：命题 1', facet: '结果：命题 1', body: '**陈述**\n满足 (A.1)。\n\n**依据**\n- 局部证明：见 [[K/1|a|a]]。' }],
    ['K/1|a|a', { title: '推导：附录 A', facet: '推导：附录 A', body: '**起点**\n先用了后一步的 (B.3)。\n\n**结果**\n$$y = 2 \\tag{A.1}$$' }],
    ['K/1|a|a|1', { title: '推导：附录 B', facet: '推导：附录 B', body: '**结果**\n$$z = 3 \\tag{B.3}$$' }],
  ])
  const hits = scan(order, cards)
  // 1) K/s references (2.1) [non-proof child] AND (A.1) [later appendix tag] -> both caught!
  // 2) K/s|c references (A.1) inside **说明** [skipped], BUT inline `**作用域** 在式 (B.3)...` is preserved and caught!
  // 3) K/1|a references (A.1) derived by its bound proof step K/1|a|a -> exempted!
  // 4) K/1|a|a (a 推导 step, NOT a Result/Benchmark) references (B.3) in its later descendant K/1|a|a|1 -> caught!
  assert.deepEqual(
    hits.map((h) => [h.card, h.eq, h.derived_by]),
    [
      ['K/s', '2.1', 'K/s|b'],
      ['K/s', 'A.1', 'K/1|a|a'],
      ['K/s|c', 'B.3', 'K/1|a|a|1'],
      ['K/1|a|a', 'B.3', 'K/1|a|a|1'],
    ],
    JSON.stringify(hits),
  )

  // 5) Unbound descendant OR descendant linked only in an external-proof clause under a Result is NOT exempted!
  const unboundOrder = ['K/1|a', 'K/1|a|a']
  const unboundCards = new Map([
    ['K/1|a', { title: '结果：命题 1', facet: '结果：命题 1', body: '**陈述**\n满足 (A.1)。\n\n**依据**\n- 外部证明：见原文 [4]（旁注 [[K/1|a|a]]）。' }],
    ['K/1|a|a', { title: '推导：未绑定附录', facet: '推导：未绑定附录', body: '**结果**\n$$y = 2 \\tag{A.1}$$' }],
  ])
  assert.deepEqual(scan(unboundOrder, unboundCards).map((h) => [h.card, h.eq, h.derived_by]), [['K/1|a', 'A.1', 'K/1|a|a']])

  // 6) Benchmark without **依据** DOES exempt its direct Side proof child (matching Rust proof_route_addresses)!
  const benchOrder = ['K/2|a', 'K/2|a|a']
  const benchCards = new Map([
    ['K/2|a', { title: '基准：无信号混同工资', facet: '基准：无信号混同工资', body: '**基准**\n由式 (B.2) 给出。' }],
    ['K/2|a|a', { title: '推导：混同工资', facet: '推导：混同工资', body: '**结果**\n$$w = 2 - q_1 \\tag{B.2}$$' }],
  ])
  assert.deepEqual(scan(benchOrder, benchCards), [])

  // 7) Exact Rust is_local_route_clause parity: bare `见 [[child]]` and `inline: 局部证明` are NOT exempted,
  //    while `局部例示：[[child]]` IS exempted!
  const parityCases = [
    ['见 [[K/3|a|a]]。', 1],
    ['- inline: 局部证明 [[K/3|a|a]]。', 1],
    ['- 局部例示：[[K/3|a|a]]。', 0],
  ]
  for (const [justLine, wantHits] of parityCases) {
    const pCards = new Map([
      ['K/3|a', { title: '结果：命题 3', facet: '结果：命题 3', body: `**陈述**\n满足 (C.1)。\n\n**依据**\n${justLine}` }],
      ['K/3|a|a', { title: '推导：证明', facet: '推导：证明', body: '**结果**\n$$v = 1 \\tag{C.1}$$' }],
    ])
    assert.equal(scan(['K/3|a', 'K/3|a|a'], pCards).length, wantHits, `clause: ${justLine}`)
  }
})
