// node --test check.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import { unitChanges } from './check.mjs'

// Two whole units (Model parts with Side children) that share one title, as the same specification in two
// empirical models does (brandimarti2026reorganizing 4|f and 6|g, 2026-11-10).
const card = (address, parent, edge = 'side', extra = {}) => ({ address, parent, edge, type: 'idea', title: address, ...extra })
const unit = (address, parent, n) => [
  card(address, parent, 'side', { type: 'model', facet: '规格', title: '规格：基准' }),
  ...Array.from({ length: n }, (_, i) => card(`${address}|${'abcdefghij'[i]}`, address)),
]
const before = [card('K', null, 'root'), card('K/1', 'K', 'direct'), card('K/2', 'K/1', 'direct'), ...unit('K/1|a', 'K/1', 3), ...unit('K/2|a', 'K/2', 1)]

test('units with one title are compared as a set of sizes, so a move that reorders them is no fault', () => {
  // The same two units, now filed under the other top Card: sizes {3, 1} before and after.
  const after = [card('K', null, 'root'), card('K/1', 'K', 'direct'), card('K/2', 'K/1', 'direct'), ...unit('K/1|a', 'K/1', 1), ...unit('K/2|a', 'K/2', 3)]
  assert.deepEqual(unitChanges(before, after), [])
})

test('a unit that lost a Card is still a fault', () => {
  const after = [card('K', null, 'root'), card('K/1', 'K', 'direct'), card('K/2', 'K/1', 'direct'), ...unit('K/1|a', 'K/1', 2), ...unit('K/2|a', 'K/2', 1)]
  assert.equal(unitChanges(before, after).length, 1)
})
