// node --test explain.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { changedParts, lastJson, keyOf, workOf, plan, assertWorkIdentity } from './explain.mjs'

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

test('plan refuses to wipe a non-empty directory not owned by explain.mjs or with unfinished jobs', () => {
  const foreign = fs.mkdtempSync(path.join(os.tmpdir(), 'zt-explain-foreign-'))
  try {
    fs.writeFileSync(path.join(foreign, 'keep.txt'), 'do not delete')
    assert.throws(() => plan('K/2|a', foreign), /refusing to wipe existing non-empty directory/)
    assert.equal(fs.readFileSync(path.join(foreign, 'keep.txt'), 'utf8'), 'do not delete')
  } finally {
    fs.rmSync(foreign, { recursive: true, force: true })
  }
  const owned = fs.mkdtempSync(path.join(os.tmpdir(), 'zt-explain-owned-'))
  const archA = { id: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', path: 'D:/box-a' }
  const archB = { id: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', path: 'D:/box-b' }
  try {
    // Empty object `{}` and head-only legacy marker must both fail identity checks.
    fs.writeFileSync(path.join(owned, '.zt-model-explain-work'), '{}\n')
    assert.throws(() => assertWorkIdentity('K/2|a', owned, archA), /missing head/)
    fs.writeFileSync(path.join(owned, '.zt-model-explain-work'), 'K/2|a\n')
    assert.throws(() => assertWorkIdentity('K/2|a', owned, archA), /no valid/)
    fs.writeFileSync(path.join(owned, '.zt-model-explain-work'), JSON.stringify({ head: 'K/2|a' }) + '\n')
    assert.throws(() => assertWorkIdentity('K/2|a', owned, archA), /lacks archive \{id, path\}/)

    // Valid marker bound to archA:
    fs.writeFileSync(path.join(owned, '.zt-model-explain-work'), JSON.stringify({ head: 'K/2|a', archive: archA }) + '\n')
    // Partial 2-part job (addresses=[A,B], done=[A]): must NOT be wiped!
    fs.writeFileSync(path.join(owned, 'state.json'), JSON.stringify({ explain: { jobs: { j1: { addresses: ['K/2|a|a|1|a', 'K/2|a|a|1|a|1'], done: ['K/2|a|a|1|a'] } } } }))
    assert.throws(() => plan('K/2|a', owned, { archive: archA }), /refusing to wipe unfinished explain state/)
    // Mismatched head marker: must refuse!
    assert.throws(() => plan('K/9|a', owned, { archive: archA }), /belongs to K\/2\|a, not K\/9\|a/)
    // Mismatched archive {id, path}: must refuse!
    assert.throws(() => plan('K/2|a', owned, { archive: archB }), /belongs to archive aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/)
    // Corrupted JSON state: must NOT fall through to rmSync!
    fs.writeFileSync(path.join(owned, 'state.json'), '{broken json')
    assert.throws(() => plan('K/2|a', owned, { archive: archA }), /refusing to wipe unreadable state\.json/)
  } finally {
    fs.rmSync(owned, { recursive: true, force: true })
  }
})
