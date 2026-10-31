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
