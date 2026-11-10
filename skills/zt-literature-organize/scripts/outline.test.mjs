// node --test outline.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import { outline } from './outline.mjs'

const card = (address, parent, edge, type = 'idea') => ({ address, parent, edge, type, title: address, body: '' })

test('the top line runs on after a whole unit: Cards after a Term list that is not last are printed', () => {
  // becker2025division before its Organize (2026-11-10): the appendix groups 10–27 stood after the Term list 8
  // and the Cited list 9, and the outline stopped at the Term list.
  const list = [
    card('K', null, 'root'),
    card('K/1', 'K', 'direct'),
    card('K/2', 'K/1', 'direct', 'term'),
    card('K/2|a', 'K/2', 'side', 'term'),
    card('K/3', 'K/2', 'direct', 'cited'),
    card('K/4', 'K/3', 'direct'),
    card('K/4|a', 'K/4', 'side'),
  ]
  const lines = outline(list, 'K')
  assert.deepEqual(lines.map(l => l.trim().split(' ')[0]), ['K', '1', '2', '3', '4', '4|a'])
  assert.match(lines[2], /\[term 2\]/)
})
