// node mask-links.test.mjs: the mask turns Links under the prefix into plain text and keeps every other Link.
import assert from 'node:assert/strict'
import { maskPattern } from './mask-links.mjs'

const re = maskPattern('becker2025division/4|a')
const body = '依赖 [[becker2025division/4|a|a|3]] 与 [[becker2025division/4|a]] 和 [[becker2025division/4|b]] 与 [[任务@becker2025division/1|a|c]] 及 [[becker2025division/4|ab]]'
assert.equal(
  body.replace(re, '$1'),
  '依赖 becker2025division/4|a|a|3 与 becker2025division/4|a 和 [[becker2025division/4|b]] 与 [[任务@becker2025division/1|a|c]] 及 [[becker2025division/4|ab]]',
)
console.log('ok')
