// Tests of copy-tree.mjs's tree comparison (no zt process runs).
//   node scripts/book-digest/tests/copy-tree.tests.mjs
import path from 'node:path';
import assert from 'node:assert/strict';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const { compare, gapFiller, createArgs, withoutOutsideLinks } = await import(pathToFileURL(path.join(here, '..', 'copy-tree.mjs')).href);

const reverse = (links) => (links.length ? `\n\n<!-- zt:reverse-links -->\n## Reverse links\n\n${links.map((a) => `- [[${a}]] t ${a}`).join('\n')}\n` : '');
const card = (address, body, inbound) => ({ address, edge: 'direct', parent: 'k', title: address, type: 'idea', body, text: `# ${address}\n\n${body}${reverse(inbound)}`, inbound: inbound.length, links: [], kinds: {} });

let passed = 0;
const test = (name, fn) => { try { fn(); passed++; console.log(`ok   ${name}`); } catch (e) { console.log(`FAIL ${name}\n     ${e.message}`); process.exitCode = 1; } };

test('identical trees have no difference', () => {
  const r = compare([card('k/1', 'a', ['k/2'])], [card('k/1', 'a', ['k/2'])], 'k');
  assert.deepEqual(r.differences, []);
  assert.deepEqual(r.outside_inbound, []);
});

test('Links into the tree from another tree are not differences: a one-tree copy cannot hold them', () => {
  // Live: k/1 is linked from k/2 and from another book's Bridge Card; the copy has only k/2.
  const r = compare([card('k/1', 'a', ['ebrill2001modern/6|a', 'k/2'])], [card('k/1', 'a', ['k/2'])], 'k');
  assert.deepEqual(r.differences, []);
  assert.deepEqual(r.outside_inbound, [{ to: 'k/1', from: 'ebrill2001modern/6|a' }]);
});

test('a Card whose only inbound Link is from outside matches a copy with no reverse-links section', () => {
  const r = compare([card('k/1', 'a', ['other/1'])], [card('k/1', 'a', [])], 'k');
  assert.deepEqual(r.differences, []);
  assert.equal(r.outside_inbound.length, 1);
});

test('a missing in-tree inbound Link is still a difference', () => {
  const r = compare([card('k/1', 'a', ['k/2', 'other/1'])], [card('k/1', 'a', [])], 'k');
  assert.deepEqual(r.differences.map((d) => d.field), ['text (with generated reverse links)', 'inbound']);
});

test('a slot the live tree left empty (a deleted Card) is filled by a placeholder, never by another source Card', () => {
  const source = new Set(['k/1', 'k/1|b', 'k/1|c'])
  const c = { address: 'k/1|b', parent: 'k/1' }
  assert.equal(gapFiller('k/1|a', c, source), true)
  assert.equal(gapFiller('k/1|c', c, source), false, 'a slot another source Card owns is a misallocation')
  assert.equal(gapFiller('k/2|a', c, source), false, 'another parent')
  assert.equal(gapFiller('k/1|b', c, source), false, 'the right slot')
})

test('a Term Card is created with its term name, so Term links find their head', () => {
  const a = createArgs({ title: '术语：高管职位', edge: 'side', type: 'term', term: '高管职位', body: '术语：高管职位' }, 'k/7')
  assert.equal(a.type, 'term')
  assert.equal(a.term, '高管职位')
})

test('a Link to another tree stays plain text in a one-tree copy; a Link inside the tree and a Term link stay', () => {
  assert.equal(withoutOutsideLinks('见 [[k/1|a]]、[[other/3|b]] 与 [[概念@k/7|h]]、[[概念@o/7|h]]，来源 [[k]]。', 'k'), '见 [[k/1|a]]、other/3|b 与 [[概念@k/7|h]]、概念，来源 [[k]]。')
})

test('a changed body is still a difference', () => {
  const r = compare([card('k/1', 'a', [])], [card('k/1', 'b', [])], 'k');
  assert.ok(r.differences.some((d) => d.field === 'body'));
});

console.log(`${passed} passed${process.exitCode ? ', some failed' : ''}`);
