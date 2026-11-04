import test from 'node:test'
import assert from 'node:assert/strict'
import { parseOps } from './batch.mjs'

test('moves, spans and reorders become batch items', () => {
  const ops = parseOps('# plan\nmove 9 side_of 8\nmove 2|a..2|a|3 after 7\n\nreorder 4 4|b 9|c 4|a\n', 'k')
  assert.deepEqual(ops[0], { tool: 'move_span', arguments: { from: 'k/9', side_of: 'k/8' } })
  assert.deepEqual(ops[1], { tool: 'move_span', arguments: { from: 'k/2|a', through: 'k/2|a|3', after: 'k/7' } })
  assert.deepEqual(ops[2], { tool: 'reorder', arguments: { parent: 'k/4', side: ['k/4|b', 'k/9|c', 'k/4|a'] } })
})

test('a malformed line names itself', () => {
  assert.throws(() => parseOps('move 9 under 8', 'k'), /line 1/)
})
