/* zt gui layout — pure, DOM-free geometry for the Map and the Constellation.
 *
 * Both views read a tree as an outline. A Card's outline children are the runs of its
 * Side successors (a run is a Card and its chain of Direct successors); a tree root's
 * outline children are its own Direct run. So outline depth counts letter segments in
 * the address, and the Direct successors of a Card are its outline siblings.
 *
 * Every function here is linear in the number of Cards, so the views re-layout on each
 * move of the Pointer without a simulation.
 */
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.ZtLayout = api;
})(typeof self !== 'undefined' ? self : this, () => {
  'use strict';

  /** The rank of a Side successor's letter label: a=1, z=26, aa=27. */
  const sideRank = (address) => {
    const label = address.slice(address.lastIndexOf('|') + 1);
    return [...label].reduce((acc, ch) => acc * 26 + ch.charCodeAt(0) - 96, 0);
  };

  /**
   * Build the outline of one tree from Card summaries (`address`, `parent`, `edge`).
   * Returns { root, card, kids, up, depth, size, order } where `kids` lists outline
   * children in reading order, `up` is the outline parent, and `size` counts a Card's
   * outline descendants.
   */
  function outline(cards, rootAddress) {
    const card = new Map(cards.map((c) => [c.address, c]));
    const direct = new Map();
    const sides = new Map();
    for (const c of cards) {
      if (!c.parent || !card.has(c.parent)) continue;
      if (c.edge === 'side') {
        if (!sides.has(c.parent)) sides.set(c.parent, []);
        sides.get(c.parent).push(c.address);
      } else direct.set(c.parent, c.address);
    }
    for (const list of sides.values()) list.sort((a, b) => sideRank(a) - sideRank(b));
    const run = (start) => {
      const list = [];
      for (let at = start; at; at = direct.get(at)) list.push(at);
      return list;
    };
    const kids = new Map();
    const up = new Map();
    const depth = new Map();
    const order = [];
    const root = rootAddress && card.has(rootAddress) ? rootAddress : cards[0] && cards[0].address;
    const visit = (address, d, parent) => {
      depth.set(address, d);
      up.set(address, parent);
      order.push(address);
      const children = (card.get(address).edge === 'root' || address === root
        ? (direct.has(address) ? run(direct.get(address)) : [])
        : (sides.get(address) || []).flatMap(run)).filter((child) => !depth.has(child));
      kids.set(address, children);
      for (const child of children) visit(child, d + 1, address);
    };
    // Anything the address rules cannot reach (malformed input) hangs off the root.
    if (root) visit(root, 0, null);
    for (const c of cards) {
      if (depth.has(c.address)) continue;
      kids.get(root).push(c.address);
      visit(c.address, 1, root);
    }
    const size = new Map();
    for (let i = order.length - 1; i >= 0; i -= 1) {
      const address = order[i];
      size.set(address, kids.get(address).reduce((sum, k) => sum + 1 + size.get(k), 0));
    }
    return { root, card, kids, up, depth, size, order };
  }

  /** The outline path from the root down to `address`, inclusive. */
  function pathTo(tree, address) {
    const path = [];
    for (let at = address; at != null; at = tree.up.get(at)) path.unshift(at);
    return path;
  }

  // Sizes of the three levels of detail. A focus Card is pulled out of the drawer, near
  // Cards stand up in it, and slips show only their top edge.
  const LOD = {
    focus: { w: 304, h: 136 },
    near: { w: 236, h: 76 },
    slip: { w: 236, h: 30 },
  };
  const GAP_Y = { focus: 16, near: 12, slip: 6 };
  const GUTTER = 56;

  /**
   * The Map's focus lens. Given the outline, the Pointer, and the Pointer's in-tree Links,
   * decide which Cards are shown, at what level of detail, and where.
   *
   * Shown: the path from the root to the focus, every outline child of each Card on that
   * path (so the top level and the focus's siblings are always visible), the focus's
   * grandchildren, and the focus's in-tree Link targets with their own paths. Everything
   * else is folded into a "+N" count on its nearest shown ancestor.
   *
   * Returns { nodes: [{address, lod, x, y, w, h, folded, role}], edges: [...], bounds }.
   */
  function focusLens(tree, focus, options = {}) {
    const { links = [], expandAll = false } = options;
    if (!tree.card.has(focus)) focus = tree.root;
    const shown = new Set();
    const path = pathTo(tree, focus);
    const onPath = new Set(path);
    const show = (address) => { for (const at of pathTo(tree, address)) shown.add(at); };
    if (expandAll) for (const address of tree.order) shown.add(address);
    for (const at of path) {
      shown.add(at);
      for (const kid of tree.kids.get(at)) shown.add(kid);
    }
    // Grandchildren too, unless the children alone already fill the lens.
    if (tree.kids.get(focus).length <= 14) {
      for (const kid of tree.kids.get(focus)) for (const grand of tree.kids.get(kid)) shown.add(grand);
    }
    const linked = new Set(links.filter((address) => tree.card.has(address) && address !== focus));
    for (const address of linked) show(address);

    // Level of detail: the focus; its parent, path, children, and neighbours in its run
    // stand up as near Cards; the rest of what is shown lies as slips.
    const parent = tree.up.get(focus);
    const siblings = parent != null ? tree.kids.get(parent) : [];
    const at = siblings.indexOf(focus);
    const near = new Set(path);
    const children = tree.kids.get(focus);
    if (children.length <= 14) for (const kid of children) near.add(kid);
    if (at > 0) near.add(siblings[at - 1]);
    if (at >= 0 && at < siblings.length - 1) near.add(siblings[at + 1]);
    const lodOf = (address) => (address === focus ? 'focus' : near.has(address) ? 'near' : 'slip');

    const nodes = [];
    const index = new Map();
    const columns = [];
    // The root's own run of Direct successors continues the root's column downward, as
    // Direct successors do everywhere; each level of Side successors adds a column.
    const columnOf = (address) => Math.max(0, tree.depth.get(address) - 1);
    // First pass: each column is as wide as the widest Card shown in it.
    for (const address of tree.order) {
      if (!shown.has(address)) continue;
      const c = columnOf(address);
      columns[c] = Math.max(columns[c] || 0, LOD[lodOf(address)].w);
    }
    const colX = [0];
    for (let c = 1; c < columns.length; c += 1) colX[c] = colX[c - 1] + (columns[c - 1] || LOD.near.w) + GUTTER;

    // Second pass: a tidy outline. A Card's shown children start level with it in the next
    // column and stack downward; its next sibling goes below the lower of the two.
    let maxX = 0;
    const place = (address, y) => {
      const lod = lodOf(address);
      const { w, h } = LOD[lod];
      const d = tree.depth.get(address);
      const kids = tree.kids.get(address);
      const shownKids = kids.filter((kid) => shown.has(kid));
      let folded = 0;
      for (const kid of kids) if (!shown.has(kid)) folded += 1 + tree.size.get(kid);
      const node = {
        address, lod, x: colX[columnOf(address)], y, w, h, depth: d, folded,
        role: address === focus ? 'focus' : onPath.has(address) ? 'path' : linked.has(address) ? 'linked' : 'context',
      };
      index.set(address, node);
      nodes.push(node);
      maxX = Math.max(maxX, node.x + w);
      let bottom = y + h;
      let childY = d === 0 ? y + h + GAP_Y[lod] : y;
      for (const kid of shownKids) {
        childY = place(kid, childY);
        bottom = Math.max(bottom, childY);
        childY += GAP_Y[lodOf(kid)];
      }
      return bottom;
    };
    const maxY = place(tree.root, 0);

    // Structure edges follow the addresses: a Direct successor hangs below its predecessor
    // in the same column, a Side successor branches right from its Card. A Card whose
    // predecessor is folded away hangs from its outline parent instead. The address chain
    // from the root down to the focus is marked hot.
    const chain = new Set();
    for (let at = focus; at != null; at = tree.card.get(at).parent) {
      chain.add(at);
      if (!tree.card.has(tree.card.get(at).parent)) break;
    }
    const edges = [];
    for (const node of nodes) {
      const card = tree.card.get(node.address);
      if (card.edge === 'root') continue;
      const from = index.has(card.parent) ? card.parent : tree.up.get(node.address);
      if (from == null || !index.has(from)) continue;
      const kind = card.edge === 'side' ? 'side' : 'direct';
      edges.push({
        from, to: node.address, kind,
        shape: index.get(from).x === node.x ? 'down' : 'elbow',
        hot: chain.has(node.address) && chain.has(from),
      });
    }
    return { nodes, index, edges, focus, bounds: { w: maxX, h: maxY } };
  }

  /** SVG path for a structure edge between two placed Cards (see `focusLens`). */
  function edgePath(edge, p, c) {
    if (edge.shape !== 'down') return elbow(p, c);
    const x = p.x + 16;
    return `M ${x} ${p.y + p.h} V ${c.y}`;
  }

  /**
   * An elbow: out of the parent's right edge at its header line, down the gutter, and
   * into the child's left edge.
   */
  function elbow(p, c) {
    const y1 = p.y + Math.min(15, p.h / 2);
    const y2 = c.y + Math.min(15, c.h / 2);
    const gx = c.x - GUTTER / 2;
    if (Math.abs(y2 - y1) < 1) return `M ${p.x + p.w} ${y1} H ${c.x}`;
    const r = Math.min(10, Math.abs(y2 - y1) / 2, GUTTER / 4);
    return `M ${p.x + p.w} ${y1} H ${gx - r} Q ${gx} ${y1} ${gx} ${y1 + r} V ${y2 - r} Q ${gx} ${y2} ${gx + r} ${y2} H ${c.x}`;
  }

  /** SVG path for a Link from the focus: an arc that leaves and enters on the right. */
  function linkArc(a, b) {
    const ax = a.x + a.w; const ay = a.y + a.h / 2;
    const bx = b.x + b.w; const by = b.y + b.h / 2;
    const reach = 40 + Math.min(220, Math.abs(by - ay) * 0.22);
    const cx = Math.max(ax, bx) + reach;
    return `M ${ax} ${ay} C ${cx} ${ay} ${cx} ${by} ${bx} ${by}`;
  }

  /**
   * The Constellation: every tree as a radial outline (root at the centre, outline depth
   * as the radius), and the trees set around one ring, the human Workspace's first.
   * Returns { trees: [{id, x, y, r, ...}], pos: Map(address -> {x, y, depth, tree}), bundles }.
   */
  function constellation(nodes, treeInfo) {
    const byTree = new Map();
    for (const node of nodes) {
      if (!byTree.has(node.tree)) byTree.set(node.tree, []);
      byTree.get(node.tree).push(node);
    }
    const info = new Map((treeInfo || []).map((t) => [t.tree, t]));
    const ids = [...byTree.keys()].sort((a, b) => {
      const wa = (info.get(a) || {}).workspace === 'agent' ? 1 : 0;
      const wb = (info.get(b) || {}).workspace === 'agent' ? 1 : 0;
      if (wa !== wb) return wa - wb;
      const ka = (info.get(a) || {}).kind === 'literature' ? 1 : 0;
      const kb = (info.get(b) || {}).kind === 'literature' ? 1 : 0;
      if (ka !== kb) return ka - kb;
      const na = Number(a); const nb = Number(b);
      if (Number.isFinite(na) && Number.isFinite(nb)) return na - nb;
      return a < b ? -1 : a > b ? 1 : 0;
    });
    const pos = new Map();
    const trees = [];
    for (const id of ids) {
      const cards = byTree.get(id);
      const root = (info.get(id) || {}).root || (cards.find((c) => c.edge === 'root') || cards[0]).address;
      const t = outline(cards, root);
      // Leaves share the circle, so a wide level never crowds one sector.
      const leaves = new Map();
      for (let i = t.order.length - 1; i >= 0; i -= 1) {
        const address = t.order[i];
        const kids = t.kids.get(address);
        leaves.set(address, kids.length ? kids.reduce((sum, k) => sum + leaves.get(k), 0) : 1);
      }
      const maxDepth = Math.max(...t.order.map((a) => t.depth.get(a)));
      const ring = Math.max(16, Math.min(46, 7 * Math.sqrt(cards.length) / Math.max(1, maxDepth) * 1.6));
      const local = new Map();
      const spread = (address, a0, a1) => {
        const d = t.depth.get(address);
        const angle = (a0 + a1) / 2;
        local.set(address, { x: Math.cos(angle) * d * ring, y: Math.sin(angle) * d * ring, depth: d, angle });
        let start = a0;
        const total = leaves.get(address);
        for (const kid of t.kids.get(address)) {
          const span = (a1 - a0) * leaves.get(kid) / total;
          spread(kid, start, start + span);
          start += span;
        }
      };
      spread(t.root, -Math.PI / 2, Math.PI * 1.5);
      const r = Math.max(26, maxDepth * ring + 18);
      trees.push({ id, root: t.root, r, local, count: cards.length, outline: t, ...info.get(id) });
    }
    // Set the trees around a ring, each taking an arc as long as its diameter.
    const gap = 46;
    const circumference = trees.reduce((sum, t) => sum + t.r * 2 + gap, 0);
    const R = trees.length > 1 ? Math.max(circumference / (2 * Math.PI), trees[0].r + 40) : 0;
    let along = 0;
    for (const t of trees) {
      const angle = -Math.PI / 2 + ((along + t.r + gap / 2) / circumference) * Math.PI * 2;
      along += t.r * 2 + gap;
      t.x = Math.cos(angle) * R;
      t.y = Math.sin(angle) * R;
      t.angle = angle;
      for (const [address, p] of t.local) pos.set(address, { x: t.x + p.x, y: t.y + p.y, depth: p.depth, tree: t.id });
    }
    // Links between trees, bundled into one arc per pair of trees.
    const bundles = new Map();
    for (const node of nodes) {
      for (const target of node.links || []) {
        const other = pos.get(target);
        if (!other || other.tree === node.tree) continue;
        const key = node.tree < other.tree ? node.tree + '\u0000' + other.tree : other.tree + '\u0000' + node.tree;
        bundles.set(key, (bundles.get(key) || 0) + 1);
      }
    }
    return {
      trees, pos, R,
      bundles: [...bundles].map(([key, count]) => { const [a, b] = key.split('\u0000'); return { a, b, count }; }),
    };
  }

  return { sideRank, outline, pathTo, focusLens, edgePath, elbow, linkArc, constellation, LOD, GUTTER };
});
