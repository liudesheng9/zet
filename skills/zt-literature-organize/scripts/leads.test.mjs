import test from 'node:test'
import assert from 'node:assert/strict'
import { linkTerms, demoteItems } from './leads.mjs'

const terms = [['税收', 'k/9|a'], ['税收归宿', 'k/9|b'], ['归宿', 'k/9|c']]

test('the longest term wins and each term links once', () => {
  assert.equal(linkTerms('税收归宿决定税收的去向，税收归宿。', terms), '[[税收归宿@k/9|b]]决定[[税收@k/9|a]]的去向，税收[[归宿@k/9|c]]。')
})

test('Links, formulas and the source line stay as they are', () => {
  const body = '见 [[k/3]] 与 $税收$。\n\n来源：[[k]] §税收'
  assert.equal(linkTerms(body, terms), body)
})

// k/3 is a chapter Card on the top line; its Side children are sections, each a Direct chain of notes.
const now = [
  { address: 'k/3', parent: 'k/2', edge: 'direct' },
  { address: 'k/4', parent: 'k/3', edge: 'direct' },
  { address: 'k/3|a', parent: 'k/3', edge: 'side' },
  { address: 'k/3|a|1', parent: 'k/3|a', edge: 'direct' },
  { address: 'k/3|b', parent: 'k/3', edge: 'side' },
  { address: 'k/3|b|1', parent: 'k/3|b', edge: 'direct' },
  { address: 'k/3|b|2', parent: 'k/3|b|1', edge: 'direct' },
  { address: 'k/3|c', parent: 'k/3', edge: 'side' },
  { address: 'k/3|d', parent: 'k/3', edge: 'side' },
  { address: 'k/3|d|1', parent: 'k/3|d', edge: 'direct' },
]
const moves = items => items.filter(i => i.tool === 'move_span').map(i => i.arguments)

test('a Side child moved up to the summary takes its Direct chain along', () => {
  const m = moves(demoteItems({ now, lead: 'k/3', title: 't', body: 'b', keep: ['k/3|a'], first: ['k/3|d'], run: 'r' }))
  assert.deepEqual(m.slice(1), [
    { from: 'k/3|d', through: 'k/3|d|1', side_of: '@@s' },
    { from: 'k/3', side_of: '@@s' },
    { from: 'k/3|b', through: 'k/3|b|2', side_of: '@@s' },
    { from: 'k/3|c', side_of: '@@s' },
  ])
})

test('a lead below the top line takes its own Direct chain along', () => {
  const m = moves(demoteItems({ now, lead: 'k/3|b', title: 't', body: 'b', run: 'r' }))
  assert.deepEqual(m.slice(1), [{ from: 'k/3|b', through: 'k/3|b|2', side_of: '@@s' }])
})
