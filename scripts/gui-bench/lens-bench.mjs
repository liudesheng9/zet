// Lens benchmark (#246): for every tree in a graph or tree answer, the largest Map lens over any
// focus and the focus that gives it. It runs the GUI's own layout module, so it measures what the
// Map would draw. It exits 1 when any lens is above the bound.
//
//   node lens-bench.mjs <graph.json | tree.json> [--bound 400] [--layout <layout.js>] [--every <n>]
//
// `graph.json` is a saved `/api/graph` answer (`{nodes}`), `tree.json` a saved `/api/tree` answer
// (`{root, cards}`). `--every n` measures every n-th Card of each tree instead of every Card.

import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

function parseArgs(argv) {
  const out = { file: null, bound: 400, layout: resolve(dirname(fileURLToPath(import.meta.url)), '../../src/gui/layout.js'), every: 1 };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--bound') out.bound = Number(argv[++i]);
    else if (arg === '--layout') out.layout = resolve(argv[++i]);
    else if (arg === '--every') out.every = Math.max(1, Number(argv[++i]));
    else if (!out.file) out.file = arg;
    else throw new Error(`unexpected argument ${arg}`);
  }
  if (!out.file) throw new Error('usage: node lens-bench.mjs <graph.json|tree.json> [--bound 400] [--layout <layout.js>] [--every <n>]');
  return out;
}

const args = parseArgs(process.argv.slice(2));
const L = createRequire(import.meta.url)(args.layout);
const answer = JSON.parse(readFileSync(args.file, 'utf8'));
const nodes = answer.nodes || answer.cards;
const trees = new Map();
for (const node of nodes) {
  if (!trees.has(node.tree)) trees.set(node.tree, []);
  trees.get(node.tree).push(node);
}
const rows = [];
for (const [tree, cards] of trees) {
  const root = cards.find((card) => card.edge === 'root');
  const outline = L.outline(cards, root && root.address);
  const started = performance.now();
  let worst = { size: 0, focus: null };
  let measured = 0;
  outline.order.forEach((focus, i) => {
    if (i % args.every) return;
    measured += 1;
    const size = L.focusLens(outline, focus, {}).nodes.length;
    if (size > worst.size) worst = { size, focus };
  });
  const whole = L.focusLens(outline, outline.root, { expandAll: true }).nodes.length;
  rows.push({ tree, cards: cards.length, measured, lens: worst.size, at: worst.focus, whole, ms: Math.round(performance.now() - started) });
}
rows.sort((a, b) => b.lens - a.lens);
const over = rows.filter((row) => row.lens > args.bound);
console.log(JSON.stringify({ bound: args.bound, ok: over.length === 0, trees: rows }));
process.exit(over.length ? 1 : 0);
