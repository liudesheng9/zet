import test from 'node:test'
import assert from 'node:assert/strict'
import { linkTerms } from './leads.mjs'

const terms = [['税收', 'k/9|a'], ['税收归宿', 'k/9|b'], ['归宿', 'k/9|c']]

test('the longest term wins and each term links once', () => {
  assert.equal(linkTerms('税收归宿决定税收的去向，税收归宿。', terms), '[[税收归宿@k/9|b]]决定[[税收@k/9|a]]的去向，税收[[归宿@k/9|c]]。')
})

test('Links, formulas and the source line stay as they are', () => {
  const body = '见 [[k/3]] 与 $税收$。\n\n来源：[[k]] §税收'
  assert.equal(linkTerms(body, terms), body)
})
