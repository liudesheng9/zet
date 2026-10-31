// node --test explain.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import path from 'node:path'
import { changedParts, lastJson, keyOf, workOf } from './explain.mjs'

test('changed parts: every done address of every job, without the refused and the blocked ones', () => {
  const state = {
    explain: {
      jobs: {
        'K-2-a:x1': { done: ['K/2|a|a|1|a', 'K/2|a|a|1|a|1', 'K/2|a|a|1|a|2'] },
        'K-2-a:x2': { done: ['K/2|a|a|3|a', 'K/2|a|a|1|a'] },
      },
      refused: { 'K/2|a': ['K/2|a|a|1|a|1: 步骤改了'] },
      blocked: { 'K/2|a': ['K/2|a|a|3|a: card_locked'] },
    },
  }
  assert.deepEqual(changedParts(state), ['K/2|a|a|1|a', 'K/2|a|a|1|a|2'])
})

test('changed parts: a plan that found nothing changes nothing', () => {
  assert.deepEqual(changedParts({ explain: { jobs: {}, refused: {}, blocked: {} } }), [])
  assert.deepEqual(changedParts({}), [])
})

test('last JSON line: the compiler prints notes before its one JSON line', () => {
  assert.deepEqual(lastJson('note\r\n{"jobs":[],"parts":0.0}\r\n\r\n'), { jobs: [], parts: 0 })
  assert.throws(() => lastJson('no json here'), /no JSON line/)
})

test('the book and the work folder of a Model Card', () => {
  assert.equal(keyOf('hart1995firms/11|a|7'), 'hart1995firms')
  assert.equal(path.basename(workOf('hart1995firms/11|a|7')), 'hart1995firms-11-a-7')
  assert.equal(workOf('K/2|a', 'D:\\w'), 'D:\\w')
})
