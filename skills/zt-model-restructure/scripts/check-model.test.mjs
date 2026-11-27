// node --test check-model.test.mjs
import test from 'node:test'
import assert from 'node:assert/strict'
import { evaluate, validateModelPayload } from './check-model.mjs'

const baseFormal = (buildOver = {}) => ({
  level: 'formal',
  form: null,
  head: { address: 'K/2|a' },
  build: {
    order: [{ address: 'K/2|a|a', line: 'main', facet: '设定' }],
    closure: [],
    reach_in: [],
    forward: [],
    unanchored: [],
    parts: [],
    beyond_level: [],
    import_target: [],
    import_cycle: [],
    formal_shape: [],
    setting_dependency: [],
    criterion_unbound: [],
    result_unbound: [],
    proof_unbound: [],
    experiment_unbound: [],
    symbol_uncovered: [],
    self_sufficient: true,
    formal_structured: true,
    ...buildOver,
  },
})

test('Stage 1 (fold) passes on a freshly folded model with Settings/Restructure/Import faults deferred', () => {
  const m = baseFormal({
    self_sufficient: false,
    formal_structured: false,
    closure: [{ from: 'K/2|a|a|1', to: 'K/1|a|1', card: { type: 'model', facet: '结果：命题 1', models: ['K/1|a'] }, fix: 'import K/1|a|1' }],
    setting_dependency: [{ address: 'K/2|a|a|1|a', fix: 'move setting' }],
    result_unbound: [{ address: 'K/2|a|a|2|a', fix: 'add source' }],
  })
  const fold = evaluate(m, 'fold')
  assert.equal(fold.ok, true, JSON.stringify(fold))
  assert.equal(fold.deferred.length, 3)

  // A non-model closure or reach_in fault DOES block Stage 1.
  const badFold = evaluate(baseFormal({ reach_in: [{ from: 'K/2|a|a', to: 'K/1|a|a', fix: 'reach_in' }] }), 'fold')
  assert.equal(badFold.ok, false)
})

test('Stage 2 (settings) blocks on setting_dependency and notation formal_shape, defers result_unbound and cross-model closure', () => {
  const pendingLater = baseFormal({
    self_sufficient: false,
    formal_structured: false,
    closure: [{ from: 'K/2|a|a|1', to: 'K/1|a|1', card: { type: 'model', facet: '结果：命题 1', models: ['K/1|a'] }, fix: 'import K/1|a|1' }],
    result_unbound: [{ address: 'K/2|a|a|2|a', fix: 'add source' }],
  })
  assert.equal(evaluate(pendingLater, 'settings').ok, true)

  const badSettings = baseFormal({
    formal_structured: false,
    setting_dependency: [{ address: 'K/2|a|a|1|a', fix: 'setting under chapter' }],
  })
  assert.equal(evaluate(badSettings, 'settings').ok, false)

  // P1-6: non-head formal_shape with lacks="符号表与读法" must NOT defer, and Stage 2 empirical
  // blocks on spec_unbound / non-estimate empirical_shape while deferring Stage 3 estimate_unbound.
  const brokenLender = baseFormal({
    closure: [{ from: 'K/2|a|a', to: 'K/1|a|a|a', card: { type: 'model', facet: '符号表与读法', home_role: 'notation', models: ['K/1|a'] }, fix: 'import notation' }],
    formal_shape: [{ address: 'K/2|a|a|a', lacks: '符号表与读法', fix: '沿用 target invalid' }],
  })
  assert.equal(evaluate(brokenLender, 'settings').ok, false)

  const empS2Bad = {
    level: 'empirical',
    head: { address: 'K/5|a' },
    build: {
      order: [{ address: 'K/5|a|a', line: 'main', role: 'data' }],
      empirical_shape: [{ address: 'K/5|a', lacks: '变量表', fix: 'missing 变量表' }],
      spec_unbound: [{ address: 'K/5|a|a|2', fix: 'spec needs design' }],
      self_sufficient: true,
      evidence_bound: false,
    },
  }
  assert.equal(evaluate(empS2Bad, 'settings').ok, false)
  assert.equal(evaluate(empS2Bad, 'settings').blocking.length, 2)

  const empS3Deferred = {
    level: 'empirical',
    head: { address: 'K/5|a' },
    build: {
      order: [{ address: 'K/5|a|a', line: 'main', role: 'data' }],
      empirical_shape: [{ address: 'K/5|a', lacks: '估计', fix: 'zero 估计' }],
      estimate_unbound: [{ address: 'K/5|a|a|3|a', fix: 'bind estimate' }],
      test_unbound: [{ address: 'K/5|a|a|3|b', fix: 'bind test' }],
      self_sufficient: true,
      evidence_bound: false,
    },
  }
  assert.equal(evaluate(empS3Deferred, 'settings').ok, true)
})

test('Stage 3 (restructure) blocks on local formal faults and defers cross-model closure (and criterion_unbound only when a pending Criterion import exists)', () => {
  const pendingCritImport = baseFormal({
    self_sufficient: false,
    closure: [{ from: 'K/2|a|a|1', to: 'K/1|a|b', card: { type: 'model', facet: '均衡定义：竞争均衡', home_role: 'criterion', models: ['K/1|a'] }, fix: 'import K/1|a|b' }],
    criterion_unbound: [{ address: 'K/2|a', fix: 'needs imported criterion' }],
  })
  assert.equal(evaluate(pendingCritImport, 'restructure').ok, true)

  // A pending import of a non-Criterion (e.g. a Result) must NOT defer criterion_unbound.
  const pendingNonCrit = baseFormal({
    self_sufficient: false,
    closure: [{ from: 'K/2|a|a|1', to: 'K/1|a|1', card: { type: 'model', facet: '结果：命题 1', home_role: 'result', models: ['K/1|a'] }, fix: 'import K/1|a|1' }],
    criterion_unbound: [{ address: 'K/2|a', fix: 'no criterion anywhere' }],
  })
  assert.equal(evaluate(pendingNonCrit, 'restructure').ok, false)

  // Without pending cross-model closure, criterion_unbound DOES block Stage 3.
  const localMissingCrit = baseFormal({
    formal_structured: false,
    criterion_unbound: [{ address: 'K/2|a', fix: 'no criterion anywhere' }],
  })
  assert.equal(evaluate(localMissingCrit, 'restructure').ok, false)

  const badResult = baseFormal({
    formal_structured: false,
    result_unbound: [{ address: 'K/2|a|a|2|a', fix: 'missing justification' }],
  })
  assert.equal(evaluate(badResult, 'restructure').ok, false)
})

test('Stage 4 (final) requires strict self_sufficient === true, formal_structured === true, and zero build.parts division faults', () => {
  assert.equal(evaluate(baseFormal(), 'final').ok, true)
  // Missing formal_structured (undefined from an older service) must NOT silently pass final acceptance.
  const missingFlag = baseFormal({ formal_structured: undefined })
  const res = evaluate(missingFlag, 'final')
  assert.equal(res.ok, false)
  assert.ok(res.blocking.some((b) => b.key === 'formal_structured'))

  // Invalid 分部 (build.parts fault) must fail final even when self_sufficient and formal_structured are true.
  const badParts = baseFormal({ parts: [{ line: '第一部分', fix: 'starts off main line' }] })
  const resParts = evaluate(badParts, 'final')
  assert.equal(resParts.ok, false)
  assert.ok(resParts.blocking.some((b) => b.key === 'parts'))

  // H8: validateModelPayload rejects missing build, missing build.parts, missing/contradictory level, or non-string form.
  assert.throws(() => validateModelPayload({ head: { address: 'K/2|a' } }, 'K/2|a'))
  const noParts = baseFormal()
  delete noParts.build.parts
  assert.throws(() => validateModelPayload(noParts, 'K/2|a'))
  const noLevel = baseFormal({ level: 'formal', formal_structured: false })
  delete noLevel.level
  assert.throws(() => validateModelPayload(noLevel, 'K/2|a'))
  const conflictLevel = baseFormal({ level: 'empirical' })
  assert.throws(() => validateModelPayload(conflictLevel, 'K/2|a'))
  const badForm = baseFormal({ formal_structured: false })
  badForm.form = true
  assert.throws(() => validateModelPayload(badForm, 'K/2|a'))
  const typoForm = baseFormal({ formal_structured: false })
  typoForm.form = 'typo'
  assert.throws(() => validateModelPayload(typoForm, 'K/2|a'))
  const typoLevel = baseFormal()
  typoLevel.level = 'forml'
  assert.throws(() => validateModelPayload(typoLevel, 'K/2|a'))
  assert.doesNotThrow(() => validateModelPayload(baseFormal(), 'K/2|a'))

  // M4: book key containing "zero" with missing design must NOT defer as Stage 3.
  const zeroBook = {
    level: 'empirical',
    head: { address: 'zero2020/1' },
    build: {
      order: [{ address: 'zero2020/1|a', line: 'main', role: 'data' }],
      empirical_shape: [{ address: 'zero2020/1', lacks: '设计', fix: 'zero2020/1 holds no design to identify the estimate' }],
      self_sufficient: true,
      evidence_bound: false,
    },
  }
  assert.equal(evaluate(zeroBook, 'settings').ok, false)
})
