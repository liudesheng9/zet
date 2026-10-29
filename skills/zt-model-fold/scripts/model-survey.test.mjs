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
  assert.deepEqual(g[0].evidence, ['K/3|a -> K/2|a: rests-on 8, shared 5', 'K/4|a -> K/3|a: shared 2'])
  // Kinship and comparison stay visible as questions for the reader, not as joins.
  assert.deepEqual(g[2].ask, ['K/7|a -> K/6|a: kin 1, compare 3'])
})
