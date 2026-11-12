// node --test dialogue.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import { termTargets, linkTargets, section, headTerms, estimateTerms, otherSide, judgedKeys, unjudged, judgedFile, widen, pairsFor, messages, withCorrespondence, linksFromAnswers } from './dialogue.mjs'

test('Term links and plain Links are read apart', () => {
  const text = '社交技能[[社交技能@A/1|c|3]]与企业规模[[企业规模@A/1|c|9]]，见 [[A/5|c]]。'
  assert.deepEqual(termTargets(text), ['A/1|c|3', 'A/1|c|9'])
  assert.deepEqual(linkTargets(text), ['A/5|c'])
})

test('the estimand section is read up to the next label', () => {
  assert.equal(section('**问题**\n问。\n\n**估计对象**\n规模对社交技能要求的关联。\n\n**设计**\ncross-section：比较。', '估计对象'), '规模对社交技能要求的关联。')
  assert.equal(section('没有这一节。', '估计对象'), '')
})

test('the estimand Terms are the head title, **问题** and **估计对象**, since a Term is linked only where a Card first names it', () => {
  // ebrill2001modern/5|a: 增值税 is linked in **问题**, so **估计对象** names it as plain text. A Term the head links
  // in a later section (a regressor in **设计** or **主要估计**) is not the estimand: an estimate that names it
  // pairs on it as its own.
  const body = [
    '**问题**', '哪些因素决定[[增值税@E/23|a]]的收入。', '',
    '**估计对象**', '税率与增值税收入之比的弹性。', '',
    '**设计**', 'cross-section：比较[[开放度@E/24|c]]不同的国家。', '',
    '来源：[[E]] §4',
  ].join('\n')
  assert.deepEqual(headTerms({ title: '增值税收入的决定因素', body }), ['E/23|a'])
  assert.deepEqual(headTerms({ title: 't', body: '**估计对象**\n无术语。' }), [])
})

test('a pair needs a shared Term (or its zt-term-sync equivalent) and no Link either way', () => {
  const head = new Set()
  const ours = [{ address: 'A/5|c|a|3|a', title: '估计：规模', body: '**对应**\n对照 [[C/2|a|3|a]]：已经比较。', ...estimateTerms('规模', head, widen(['A/1|c|3'], new Map([['A/1|c|3', ['B/1|c|7']]]))) }]
  const theirs = [
    { address: 'B/4|b|a|3|a', title: '估计：社交', body: '社交[[社交@B/1|c|7]]', kind: 'estimate', own: new Set(['B/1|c|7']), head: new Set() },
    { address: 'C/2|a|3|a', title: '估计：已链', body: '社交[[社交@B/1|c|7]]', kind: 'estimate', own: new Set(['B/1|c|7']), head: new Set() },
    { address: 'D/3|c|a', title: '发现：无关', body: '价格[[价格@D/1|c|1]]', kind: 'finding', own: new Set(['D/1|c|1']), head: new Set() },
  ]
  const pairs = pairsFor(ours, theirs)
  assert.deepEqual(pairs.map((p) => [p.id, p.from, p.to, p.shared]), [['p1', 'A/5|c|a|3|a', 'B/4|b|a|3|a', ['B/1|c|7']]])
})

test('an estimate pairs on the Term of what it measures, not on the book-wide Term every estimate shares', () => {
  // ebrill2001modern/5|a against 4|d|a: both models are about 增值税 (E/23|a); only the openness estimates measure the
  // same thing. Pairing on 增值税 alone gave every estimate against every estimate (11 x 9 = 99 pairs).
  const head = new Set(['E/23|a'])
  const openness = { address: 'E/5|a|a|5|a', body: '', ...estimateTerms('贸易[[开放度@E/23|y]]越高，[[增值税@E/23|a]]收入越高', head, new Set(['E/23|y', 'E/23|a'])) }
  const general = { address: 'E/5|a|a|4|b', body: '', ...estimateTerms('高税率伴窄税基', head, new Set()) }
  assert.deepEqual([openness.specific, [...openness.terms]], [true, ['E/23|y']])
  assert.deepEqual([general.specific, [...general.terms]], [false, ['E/23|a']])
  const theirs = [
    // their estimate names 增值税 only through its head, and openness in its own text
    { address: 'E/4|d|a|a|7|a', body: '', kind: 'estimate', own: new Set(['E/23|y']), head: new Set(['E/23|a']) },
    { address: 'E/4|d|a|a|6|a', body: '', kind: 'estimate', own: new Set(['E/23|w']), head: new Set(['E/23|a']) },
  ]
  const pairs = pairsFor([openness, general], theirs)
  assert.deepEqual(pairs.map((p) => [p.from, p.to, p.shared]), [
    ['E/5|a|a|5|a', 'E/4|d|a|a|7|a', ['E/23|y']],
    ['E/5|a|a|4|b', 'E/4|d|a|a|7|a', ['E/23|a']],
    ['E/5|a|a|4|b', 'E/4|d|a|a|6|a', ['E/23|a']],
  ])
})

test('the other side comes from every Literature tree, not from a search capped at 200 rows', () => {
  // The box's model search returned 200 rows of parts and heads, and ebrill2001modern/5|a and hansen2021demand/5|c|e
  // were not among them.
  const trees = [
    { cards: [
      { address: 'E/4|d|a', type: 'model', level: 'empirical', head: null },
      { address: 'E/4|d|a|a', type: 'model', level: null, head: 'E/4|d|a' },
      { address: 'E/5|a', type: 'model', level: 'empirical', head: null },
      { address: 'E/11|b|a', type: 'model', level: 'formal', head: null },
      { address: 'E/4|d|h', type: 'finding', level: null, head: null },
    ] },
    { cards: [
      { address: 'H/5|c|e', type: 'model', level: 'empirical', head: null },
      { address: 'H/1|c', type: 'finding', level: null, head: null },
    ] },
  ]
  assert.deepEqual(otherSide(trees, 'E/5|a'), { heads: ['E/4|d|a', 'H/5|c|e'], findings: ['H/1|c'] })
})

test('a judged pair is never judged again, in either direction, whatever the verdict', () => {
  // ebrill2001modern/5|a and 4|d|a: the 108 pairs of one model are the mirror of 132 of the other; a pair judged
  // not the same estimand wrote no Link, so without the record every later survey paid the judge again.
  const record = judgedKeys([
    { from: 'E/5|a|a|4|b', to: 'E/4|d|a|a|4|a' },
    { from: 'E/5|a|a|5|a', to: 'H/1|c' },
  ])
  const pairs = [
    { id: 'p1', from: 'E/4|d|a|a|4|a', to: 'E/5|a|a|4|b' },
    { id: 'p2', from: 'E/5|a|a|5|a', to: 'H/1|c' },
    { id: 'p3', from: 'E/4|d|a|a|4|a', to: 'E/5|a|a|5|a' },
  ]
  assert.deepEqual(unjudged(pairs, record).map((p) => [p.id, p.from, p.to]), [['p1', 'E/4|d|a|a|4|a', 'E/5|a|a|5|a']])
})

test('a judged pair is known by its two statements, so a move that renames both Cards does not judge it again', () => {
  // liu2026managing 2026-11-11: the Literature organize renamed the estimates of 6|a after their first judging, and
  // the next survey proposed 102 judged pairs again under their new addresses.
  const record = judgedKeys([{ from: 'L/6|a|a|3|b', to: 'L/13|a|a|4|c', from_fp: 'f1', to_fp: 't1' }])
  const pairs = [
    { id: 'p1', from: 'L/4|b|a|a|3|b', to: 'L/6|a|a|4|c', from_fp: 'f1', to_fp: 't1' },
    { id: 'p2', from: 'L/6|a|a|4|c', to: 'L/4|b|a|a|3|b', from_fp: 't1', to_fp: 'f1' },
    { id: 'p3', from: 'L/4|b|a|a|3|b', to: 'L/6|a|a|4|d', from_fp: 'f1', to_fp: 't2' },
  ]
  assert.deepEqual(unjudged(pairs, record).map((p) => [p.id, p.to]), [['p1', 'L/6|a|a|4|d']])
  // A record without fingerprints (written before this rule) still answers by address.
  assert.equal(unjudged([{ id: 'p1', from: 'A', to: 'B', from_fp: 'x', to_fp: 'y' }], judgedKeys([{ from: 'B', to: 'A' }])).length, 0)
})

test('the judged record belongs to one box', () => {
  assert.notEqual(judgedFile({ ZT_CONFIG_DIR: 'D:\\zt-empirical-box\\config' }, 'C:\\h'), judgedFile({}, 'C:\\h'))
  assert.equal(judgedFile({}, 'C:\\h'), judgedFile({}, 'C:\\h'))
})

test('the judge gets messages of twelve pairs', () => {
  const pairs = Array.from({ length: 13 }, (_, k) => ({ id: `p${k + 1}`, from: 'A', to: 'B', kind: 'estimate', shared: ['T'] }))
  const m = messages(pairs, (at) => at)
  assert.equal(m.length, 2)
  assert.equal((m[0].match(/^## p/gm) || []).length, 12)
  assert.match(m[1], /^## p13/)
})

test('a judged Link goes into the **对应** section, before the source line, once', () => {
  const body = '规模大的企业更常要求社交技能。\n\n**估计**\n- 值：0.014。\n\n来源：[[A]] §5'
  const once = withCorrespondence(body, 'B/4|b|a|3|a', '对照 [[B/4|b|a|3|a]]：同一估计对象，方向一致。')
  assert.equal(once, '规模大的企业更常要求社交技能。\n\n**估计**\n- 值：0.014。\n\n**对应**\n对照 [[B/4|b|a|3|a]]：同一估计对象，方向一致。\n\n来源：[[A]] §5')
  assert.equal(withCorrespondence(once, 'B/4|b|a|3|a', 'x'), null)
  const twice = withCorrespondence(once, 'C/1', '反驳 [[C/1]]：符号相反。')
  assert.match(twice, /方向一致。\n反驳 \[\[C\/1\]\]：符号相反。\n\n来源/)
})

test('only a same-estimand verdict with a kind word becomes a Link; an estimate pair is written both ways', () => {
  const pairs = [
    { id: 'p1', from: 'A/1', to: 'B/1', kind: 'estimate' },
    { id: 'p2', from: 'A/1', to: 'C/1', kind: 'finding' },
    { id: 'p3', from: 'A/1', to: 'D/1', kind: 'estimate' },
    { id: 'p4', from: 'A/1', to: 'E/1', kind: 'estimate' },
    { id: 'p5', from: 'A/1', to: 'F/1', kind: 'finding' },
  ]
  const links = linksFromAnswers(pairs, [
    { pair: 'p1', same: true, verdict: 'agree', kind: '印证', reason: '两者都为正，在不确定性之内一致' },
    { pair: 'p2', same: true, verdict: 'disagree', kind: '反驳', reason: '符号相反。' },
    { pair: 'p3', same: false, verdict: 'not-comparable', kind: '对照', reason: '估计对象不同' },
    { pair: 'p4', same: true, verdict: 'agree', kind: '依赖', reason: '错误的关系词' },
    // the same estimand in other units: the judge's instructions give it 对照, and it is written
    { pair: 'p5', same: true, verdict: 'not-comparable', kind: '对照', reason: '同一估计对象，样本不同' },
  ])
  assert.deepEqual(links.map((l) => [l.pair, l.line, l.back]), [
    ['p1', '印证 [[B/1]]：两者都为正，在不确定性之内一致。', '印证 [[A/1]]：两者都为正，在不确定性之内一致。'],
    ['p2', '反驳 [[C/1]]：符号相反。', null],
    ['p5', '对照 [[F/1]]：同一估计对象，样本不同。', null],
  ])
})

test('a new survey leaves only its own message files; a model without an apply reads as no links', async () => {
  // ebrill2001modern/5|a on the live box (2026-11-09): a second survey of 7 messages left judge-8..10.md of the
  // first one beside them. hansen2021demand/5|c|e: no pair, so no apply, and check failed on a missing applied.json.
  const { writeMessages, readApplied } = await import('./dialogue.mjs')
  const fs = await import('node:fs')
  const os = await import('node:os')
  const path = await import('node:path')
  const out = fs.mkdtempSync(path.join(os.tmpdir(), 'dialogue-test-'))
  writeMessages(out, 'K/5|a', ['a', 'b', 'c'])
  const files = writeMessages(out, 'K/5|a', ['a'])
  assert.deepEqual(fs.readdirSync(out).filter((f) => f.startsWith('judge-')), ['judge-1.md'])
  assert.equal(files.length, 1)
  assert.deepEqual(readApplied(out).links, [])
  fs.rmSync(out, { recursive: true })
})

test('a Term most estimates of one model link (标准误, 设定) is the model\'s reporting frame and forms no pair', async () => {
  // dell2010persistent: 1,105 pairs among its own models, 483 on 标准误 (D/1|a|an) alone, ~98% on 标准误 with
  // other method Terms. Only a Term that tells one estimate apart from its siblings says what it measures.
  const { frameTerms } = await import('./dialogue.mjs')
  const se = 'D/1|a|an', hacienda = 'D/1|a|q', mita = 'D/1|a|a'
  const frame = frameTerms([[se], [se, hacienda], [se]])
  assert.deepEqual([...frame], [se])
  assert.deepEqual([...frameTerms([[se], [se]])], [], 'two estimates are too few to tell a frame')
  const head = new Set([mita])
  const ours = [
    { address: 'D/6|b|a|4|b', body: '', ...estimateTerms('', head, new Set([se, hacienda]), frame) },
    { address: 'D/6|b|a|4|c', body: '', ...estimateTerms('', head, new Set([se]), frame) },
  ]
  assert.deepEqual(ours.map((o) => [o.specific, [...o.terms]]), [[true, [hacienda]], [false, [mita]]])
  const theirs = [
    { address: 'D/7|b|a|4|b', body: '', kind: 'estimate', own: new Set([se]), frame: new Set([se]), head: new Set(['D/1|a|bf']) },
    { address: 'D/8|a|a|4|b', body: '', kind: 'estimate', own: new Set([se, hacienda]), frame: new Set([se]), head: new Set(['D/1|a|bj']) },
  ]
  assert.deepEqual(pairsFor(ours, theirs).map((p) => [p.from, p.to, p.shared]), [['D/6|b|a|4|b', 'D/8|a|a|4|b', [hacienda]]])
})
