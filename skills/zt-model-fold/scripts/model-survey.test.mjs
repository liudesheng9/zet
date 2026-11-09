// node --test model-survey.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import { propose } from './model-survey.mjs'

test('candidate groups join Model Cards by rests-on Links and shared parts only; comparisons and kinship do not join', () => {
  const heads = ['K/2|a', 'K/3|a', 'K/4|a', 'K/6|a', 'K/7|a', 'K/9|a']
  const links = [
    { from: 'K/3|a', to: 'K/2|a', rests: 8, shared: 5, kin: 0, compare: 0 },
    { from: 'K/4|a', to: 'K/3|a', rests: 0, shared: 2, kin: 0, compare: 0 },
    { from: 'K/7|a', to: 'K/6|a', rests: 0, shared: 0, kin: 1, compare: 3 },   // relaxes / contrasts: not a join
    { from: 'K/9|a', to: 'K/2|a', rests: 0, shared: 0, kin: 0, compare: 4 },
  ]
  const g = propose(heads, links)
  assert.deepEqual(g.map((x) => x.heads), [['K/2|a', 'K/3|a', 'K/4|a'], ['K/6|a'], ['K/7|a'], ['K/9|a']])
  assert.deepEqual(g[0].evidence, ['K/3|a -> K/2|a: rests-on 8, legacy share 5', 'K/4|a -> K/3|a: legacy share 2'])
  // Kinship and comparison stay visible as questions for the reader, not as joins.
  assert.deepEqual(g[2].ask, ['K/7|a -> K/6|a: kin 1, compare 3'])
})

test('an import (引入) never joins a group; a legacy （共用） share still does, named as one (#211)', () => {
  const heads = ['K/2|a', 'K/3|a', 'K/5|a']
  const links = [
    { from: 'K/3|a', to: 'K/2|a', rests: 0, shared: 0, imports: 4, kin: 0, compare: 0 },
    { from: 'K/5|a', to: 'K/2|a', rests: 0, shared: 2, imports: 0, kin: 0, compare: 0 },
  ]
  const g = propose(heads, links)
  assert.deepEqual(g.map((x) => x.heads), [['K/2|a', 'K/5|a'], ['K/3|a']])
  assert.deepEqual(g[0].evidence, ['K/5|a -> K/2|a: legacy share 2'])
  // An import is intended sharing between two models: listed to read, never a join.
  assert.deepEqual(g[1].ask, ['K/3|a -> K/2|a: imports 4'])
})

test('modelHeads reads every Model Card head from get_tree rows, past search_cards\' 200-result cap', async () => {
  const { modelHeads } = await import('./model-survey.mjs')
  const rows = [
    { address: 'K', type: 'literature' },
    { address: 'K/1|a', type: 'idea', facet: null },
    ...Array.from({ length: 250 }, (_, i) => ({ address: `K/${i + 2}|a`, type: 'model', facet: null })),
    { address: 'K/2|a|a', type: 'model', facet: '设定' },
  ]
  const heads = modelHeads(rows, 'K')
  assert.equal(heads.length, 250)
  assert.ok(!heads.includes('K/2|a|a'))
})

test('a false share (a Model Card Link to another model\'s part) is named as one and never joins; a legacy share still joins', async () => {
  const { sharedKind } = await import('./model-survey.mjs')
  const falseTargets = new Set(['K/2|a|a|1'])
  assert.equal(sharedKind({ address: 'K/2|a|a|1', line: 'shared', legacy: true }, falseTargets), 'false share')
  assert.equal(sharedKind({ address: 'K/2|a|a|2', line: 'shared', legacy: true }, falseTargets), 'legacy share')
  assert.equal(sharedKind({ address: 'K/2|a|a|3', line: 'shared', import: true }, falseTargets), 'import')
  const g = propose(['K/2|a', 'K/3|a', 'K/5|a'], [
    { from: 'K/3|a', to: 'K/2|a', rests: 0, shared: 0, imports: 0, falseShares: 1, kin: 0, compare: 0 },
    { from: 'K/5|a', to: 'K/2|a', rests: 0, shared: 1, imports: 0, kin: 0, compare: 0 },
  ])
  assert.deepEqual(g.map((x) => x.heads), [['K/2|a', 'K/5|a'], ['K/3|a']])
  assert.deepEqual(g[1].ask, ['K/3|a -> K/2|a: false share 1'])
  // Beside a rests-on Link, the false share is named in the evidence line but adds no join of its own.
  const h = propose(['K/2|a', 'K/3|a'], [{ from: 'K/3|a', to: 'K/2|a', rests: 1, shared: 0, imports: 0, falseShares: 1, kin: 0, compare: 0 }])
  assert.deepEqual(h[0].evidence, ['K/3|a -> K/2|a: rests-on 1, false share 1'])
})

test('a candidate group that mixes an empirical model with a formal or concept one is flagged: a theory and its test are two models', async () => {
  const { mixedLevels } = await import('./model-survey.mjs')
  const level = new Map([['K/2|a', 'formal'], ['K/3|a', 'empirical'], ['K/4|a', 'empirical'], ['K/5|a', 'concept']])
  assert.equal(mixedLevels(['K/2|a', 'K/3|a'], level), 'levels mixed (empirical + formal): a theory and its test, or a theory and its estimation, are two models joined by an import; fold only cards of one level')
  assert.equal(mixedLevels(['K/3|a', 'K/4|a'], level), null)
  assert.equal(mixedLevels(['K/2|a', 'K/5|a'], level), null)
})
