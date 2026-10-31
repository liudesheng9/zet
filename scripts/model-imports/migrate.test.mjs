// node --test scripts/model-imports/migrate.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import { planModel, rewriteBody } from './migrate.mjs'

// One borrower's data as get_model reports it (#213, I8 of #205).
const borrower = (over = {}) => ({
  head: 'K/7|a',
  locked: false,
  body: '问题：B？\n\n**结构**\n\n- 设定 [[K/7|a|a]]（核心）：设定\n- 共用 [[K/2|a|a]]（共用）：A 的设定\n- 共用 [[K/2|a|a|b]]（共用）：A 的时点\n- 共用 [[K/3|a|a|2]]（共用）：C 的一步\n\n**关联**\n\n- 对照 [[K/4|a|a|1]]：比较\n\n来源：[[K]]',
  imports: [
    { root: 'K/2|a|a', mode: 'legacy', home: 'K/2|a' },
    { root: 'K/2|a|a|b', mode: 'legacy', home: 'K/2|a' },
    { root: 'K/3|a|a|2', mode: 'legacy', home: 'K/3|a' },
  ],
  tolerated: [{ part: 'K/3|a|a|2', home: 'K/3|a', premise: { address: 'K/3|a|a|1', models: ['K/3|a'] } }],
  false_share: [{ target: 'K/4|a|a|1' }],
  relations: [{ from: 'K/7|a|a', to: 'K/2|a|a', kind: 'corresponds' }, { from: 'K/7|a|a', to: 'K/2|a|a|b', kind: 'rests-on' }],
  own: ['K/7|a|a'],
  ...over,
})

test('a legacy share becomes 整块 when the borrower shares more of its block or the closure would bring a tolerated premise in; 单卡 otherwise', () => {
  const p = planModel(borrower())
  assert.deepEqual(p.lines.map((l) => [l.root, l.mode]), [['K/2|a|a', '整块'], ['K/3|a|a|2', '整块']])
  assert.deepEqual(p.lines[0].absorbed, ['K/2|a|a|b'], 'a share inside another share\'s block is absorbed')
  assert.match(p.lines[0].reason, /block/)
  assert.match(p.lines[1].reason, /tolerated/)
  const single = planModel(borrower({ imports: [{ root: 'K/3|a|a|2', mode: 'legacy', home: 'K/3|a' }], tolerated: [] }))
  assert.deepEqual(single.lines.map((l) => [l.root, l.mode]), [['K/3|a|a|2', '单卡']])
})

test('false shares and possible copies are listed, never changed; a locked tree is skipped', () => {
  const p = planModel(borrower())
  assert.deepEqual(p.falseShares, ['K/4|a|a|1'])
  assert.deepEqual(p.copies, [{ part: 'K/7|a|a', shared: 'K/2|a|a' }], 'an own part that only corresponds to a shared Card may restate it')
  const locked = planModel(borrower({ locked: true }))
  assert.equal(locked.skip, 'locked')
  assert.equal(locked.body, null)
})

test('the new body: the 共用 lines leave 结构, the imports stand in **引入** before it with their own text; the rest stays', () => {
  const p = planModel(borrower())
  assert.equal(
    p.body,
    '问题：B？\n\n**引入**\n\n- 整块 [[K/2|a|a]]：A 的设定\n- 整块 [[K/3|a|a|2]]：C 的一步\n\n**结构**\n\n- 设定 [[K/7|a|a]]（核心）：设定\n\n**关联**\n\n- 对照 [[K/4|a|a|1]]：比较\n\n来源：[[K]]'
  )
  // An existing 引入 section keeps its lines; a migrated root it already names is not added twice.
  const merged = rewriteBody('问题\n\n**引入**\n\n- 单卡 [[K/9|a]]：旧\n\n**结构**\n\n- 共用 [[K/9|a]]（共用）：重复\n- 共用 [[K/8|a]]（共用）：新', [
    { root: 'K/9|a', mode: '单卡', text: '重复', absorbed: [] },
    { root: 'K/8|a', mode: '单卡', text: '新', absorbed: [] },
  ])
  assert.equal(merged, '问题\n\n**引入**\n\n- 单卡 [[K/9|a]]：旧\n- 单卡 [[K/8|a]]：新\n\n**结构**')
})

test('a model without legacy shares has nothing to do', () => {
  const p = planModel(borrower({ imports: [{ root: 'K/2|a|a', mode: 'block', home: 'K/2|a' }], false_share: [], relations: [] }))
  assert.deepEqual(p.lines, [])
  assert.equal(p.body, null)
})

test('a tolerated premise outside the home model does not make a 整块 line: the closure stays in the home model (box-copy run, 8|a)', () => {
  const p = planModel(borrower({
    imports: [{ root: 'K/3|a|a|2', mode: 'legacy', home: 'K/3|a' }],
    tolerated: [{ part: 'K/3|a|a|2', home: 'K/3|a', premise: { address: 'K/5|a|a|1', models: ['K/5|a'] } }],
  }))
  assert.deepEqual(p.lines.map((l) => [l.root, l.mode]), [['K/3|a|a|2', '单卡']])
})
