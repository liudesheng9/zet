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

  /**
   * A Card summary's Links, split for a graph view (S16, #158; design 15.2). A Term use is a
   * vocabulary reference, not a reasoning Link: `A -> B` is a Term edge iff `A.term_use_targets`
   * names `B`. `links` keeps every ordinary edge: a target the Card names only through Term uses
   * is left out, while one it also names with an announced relation (`kinds`) stays, its Term use
   * then being one more edge. `terms` lists the Term edges, and only when `showTerms` is on, so a
   * default view draws none. Nodes are never touched: this decides only which edges are drawn.
   */
  function edgeSplit(node, showTerms = false) {
    const all = (node && node.links) || [];
    const used = new Set((node && node.term_use_targets) || []);
    const kinds = (node && node.kinds) || {};
    const links = all.filter((target) => !used.has(target) || Object.prototype.hasOwnProperty.call(kinds, target));
    const terms = showTerms ? all.filter((target, i) => used.has(target) && all.indexOf(target) === i) : [];
    return { links, terms };
  }

  /** Whether `node` reaches `target` with a Term use (the edge a default view leaves out). */
  const isTermEdge = (node, target) => !!(node && node.term_use_targets && node.term_use_targets.includes(target));

  /**
   * The Map's edges at its focus. `outbound` and `inbound` are the in-tree addresses the focus
   * links and is linked from; `summaryOf` gives a Card summary. `lens` is what the focus lens
   * shows, the same with Term edges on or off, so pressing the switch moves no Card; `out` and
   * `in` are the ordinary arcs; `termOut` and `termIn` the Term arcs, empty unless `showTerms`.
   * Inbound arcs are left out when more than 16 Cards link the focus (a tree root would light
   * up everything).
   */
  function focusEdges(summaryOf, focus, outbound, inbound, showTerms = false) {
    const inboundShown = inbound.length <= 16 ? inbound : [];
    const own = edgeSplit(summaryOf(focus), true);
    return {
      lens: [...outbound, ...inboundShown],
      out: outbound.filter((at) => own.links.includes(at)),
      in: inboundShown.filter((at) => edgeSplit(summaryOf(at), false).links.includes(focus)),
      termOut: showTerms ? outbound.filter((at) => own.terms.includes(at)) : [],
      termIn: showTerms ? inboundShown.filter((at) => isTermEdge(summaryOf(at), focus)) : [],
    };
  }

  /**
   * SVG path for a Link from the focus: an arc that leaves and enters on the right. `wider`
   * bows it further out, so a Term edge beside an ordinary one between the same Cards stays apart.
   */
  function linkArc(a, b, wider = 0) {
    const ax = a.x + a.w; const ay = a.y + a.h / 2;
    const bx = b.x + b.w; const by = b.y + b.h / 2;
    const reach = 40 + wider + Math.min(220, Math.abs(by - ay) * 0.22);
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
    // Links between trees, bundled into one arc per pair of trees. Ordinary Links and Term uses
    // are bundled apart (S16, #158): the view draws `termBundles` only when Term edges are shown,
    // and the ring itself never depends on either.
    const bundles = new Map();
    const termBundles = new Map();
    const bundle = (into, node, target) => {
      const other = pos.get(target);
      if (!other || other.tree === node.tree) return;
      const key = node.tree < other.tree ? node.tree + '\u0000' + other.tree : other.tree + '\u0000' + node.tree;
      into.set(key, (into.get(key) || 0) + 1);
    };
    for (const node of nodes) {
      const split = edgeSplit(node, true);
      for (const target of split.links) bundle(bundles, node, target);
      for (const target of split.terms) bundle(termBundles, node, target);
    }
    const listed = (map) => [...map].map(([key, count]) => { const [a, b] = key.split('\u0000'); return { a, b, count }; });
    return { trees, pos, R, bundles: listed(bundles), termBundles: listed(termBundles) };
  }

  // ------------------------------------------------------------ the Table
  // The Table is a free working surface: a reader lays Cards from anywhere in the slip-box on
  // it, side by side. Positions are the top-left corners of cards of one size; a reader moves
  // cards by hand, and these helpers only suggest places (all at most quadratic in the few
  // dozen cards a Table holds).
  const TABLE = { w: 236, h: 150, gap: 30 };

  /** Folgezettel order: by tree (Topic numbers first, by value), then segment by segment. */
  function addressOrder(a, b) {
    const split = (address) => {
      const at = address.indexOf('/');
      return at < 0 ? [address, []] : [address.slice(0, at), address.slice(at + 1).split('|')];
    };
    const [ta, sa] = split(a);
    const [tb, sb] = split(b);
    if (ta !== tb) {
      const na = /^\d+$/.test(ta); const nb = /^\d+$/.test(tb);
      if (na && nb) return Number(ta) - Number(tb);
      if (na !== nb) return na ? -1 : 1;
      return ta < tb ? -1 : 1;
    }
    for (let i = 0; i < Math.max(sa.length, sb.length); i += 1) {
      if (i >= sa.length) return -1;
      if (i >= sb.length) return 1;
      const x = sa[i]; const y = sb[i];
      if (x === y) continue;
      const nx = /^\d+$/.test(x); const ny = /^\d+$/.test(y);
      if (nx && ny) return Number(x) - Number(y);
      if (nx !== ny) return nx ? -1 : 1;
      return sideRank('|' + x) - sideRank('|' + y);
    }
    return 0;
  }

  const overlaps = (a, b, pad) =>
    a.x < b.x + b.w + pad && b.x < a.x + a.w + pad && a.y < b.y + b.h + pad && b.y < a.y + a.h + pad;

  /**
   * The nearest free place to `want` (a top-left corner) for one more card, searched ring by
   * ring on a half-card grid; `rects` are the cards already lying there. Places on the same
   * row come first, as a hand lays a card beside the last.
   */
  function freeSpot(rects, want, size = TABLE) {
    const sx = (size.w + size.gap) / 2;
    const sy = (size.h + size.gap) / 2;
    const fits = (x, y) => !rects.some((r) => overlaps({ x, y, w: size.w, h: size.h }, r, size.gap * 0.6));
    for (let ring = 0; ring < 40; ring += 1) {
      const spots = [];
      for (let i = -ring; i <= ring; i += 1) {
        for (let j = -ring; j <= ring; j += 1) {
          if (Math.max(Math.abs(i), Math.abs(j)) === ring) spots.push({ x: want.x + i * sx, y: want.y + j * sy });
        }
      }
      // Right and below before left and above, as a hand lays out cards.
      const cost = (p) => Math.hypot(p.x - want.x, (p.y - want.y) * 1.6) * (p.x < want.x ? 1.3 : 1) * (p.y < want.y ? 1.15 : 1);
      spots.sort((p, q) => cost(p) - cost(q));
      for (const p of spots) if (fits(p.x, p.y)) return p;
    }
    return { x: want.x, y: want.y };
  }

  /**
   * An order to read a set of Cards in: each Card that no other Card on the set links starts a
   * chain, and a chain follows Links depth first; Folgezettel order breaks ties.
   */
  function readingOrder(addresses, linksOf) {
    const on = new Set(addresses);
    const sorted = [...on].sort(addressOrder);
    const into = new Map(sorted.map((address) => [address, 0]));
    for (const address of sorted) {
      for (const target of linksOf(address)) if (on.has(target) && target !== address) into.set(target, into.get(target) + 1);
    }
    const order = [];
    const seen = new Set();
    const walk = (address) => {
      if (seen.has(address)) return;
      seen.add(address);
      order.push(address);
      for (const target of linksOf(address)) if (on.has(target)) walk(target);
    };
    for (const address of sorted) if (!into.get(address)) walk(address);
    for (const address of sorted) walk(address);
    return order;
  }

  /** Cards in rows, left to right, in the order given: an argument laid out to write from. */
  function lineUp(order, size = TABLE) {
    const perRow = Math.max(3, Math.min(6, Math.ceil(Math.sqrt(order.length * 1.5))));
    return new Map(order.map((address, i) => [address, {
      x: (i % perRow) * (size.w + size.gap),
      y: Math.floor(i / perRow) * (size.h + size.gap),
    }]));
  }

  /**
   * The card held in the middle and the others in the cells of a grid around it, nearest cells
   * first: the cards it relates to take the cells beside it, the rest lie further out. Beside
   * comes before above and below, and the cells go round clockwise from the right, so a set
   * stays compact enough to read at full size.
   */
  function gather(addresses, focus, isNear, size = TABLE) {
    const pos = new Map();
    const cw = size.w + size.gap;
    const ch = size.h + size.gap;
    const held = addresses.includes(focus);
    const others = addresses.filter((address) => address !== focus);
    const ordered = [...others.filter((address) => isNear(address)), ...others.filter((address) => !isNear(address))];
    if (held) pos.set(focus, { x: -size.w / 2, y: -size.h / 2 });
    const reach = Math.ceil(Math.sqrt(ordered.length + 1)) + 1;
    const cells = [];
    for (let i = -reach; i <= reach; i += 1) {
      for (let j = -reach; j <= reach; j += 1) {
        if (held && i === 0 && j === 0) continue;
        const turn = (Math.atan2(j, i) + Math.PI * 2) % (Math.PI * 2);
        cells.push({ i, j, d: Math.round(Math.hypot(i * cw, j * ch * 1.6)), turn });
      }
    }
    cells.sort((a, b) => a.d - b.d || a.turn - b.turn);
    ordered.forEach((address, k) => {
      const cell = cells[k];
      pos.set(address, { x: cell.i * cw - size.w / 2, y: cell.j * ch - size.h / 2 });
    });
    return pos;
  }

  /**
   * Cards by tree, one column per tree, each column shingled in Folgezettel order the way
   * cards stand in a drawer: each card covers the one before it below its title.
   */
  function byTree(addresses, size = TABLE) {
    const trees = new Map();
    for (const address of [...addresses].sort(addressOrder)) {
      const tree = address.includes('/') ? address.slice(0, address.indexOf('/')) : address;
      if (!trees.has(tree)) trees.set(tree, []);
      trees.get(tree).push(address);
    }
    const pos = new Map();
    let column = 0;
    for (const list of trees.values()) {
      list.forEach((address, i) => pos.set(address, { x: column * (size.w + size.gap * 1.4), y: i * 78 }));
      column += 1;
    }
    return pos;
  }

  /** Where the line from the centre of `r` toward `p` leaves `r`. */
  function edgePoint(r, p) {
    const cx = r.x + r.w / 2; const cy = r.y + r.h / 2;
    const dx = p.x - cx; const dy = p.y - cy;
    const s = Math.min(dx ? (r.w / 2) / Math.abs(dx) : Infinity, dy ? (r.h / 2) / Math.abs(dy) : Infinity, 1);
    return { x: cx + dx * s, y: cy + dy * s };
  }

  /**
   * A thread from card `a` to card `b`: from edge to edge, sagging a little as a string does
   * under its own weight; a thread back from `b` to `a` sags the other way. Returns its path,
   * its middle (for a label), and its end and direction (for a tip).
   */
  function thread(a, b, back = false) {
    const ca = { x: a.x + a.w / 2, y: a.y + a.h / 2 };
    const cb = { x: b.x + b.w / 2, y: b.y + b.h / 2 };
    const pa = edgePoint(a, cb);
    const pb = edgePoint(b, ca);
    const dx = pb.x - pa.x; const dy = pb.y - pa.y;
    const len = Math.hypot(dx, dy) || 1;
    const sag = Math.min(46, 10 + len * 0.1) * (back ? -1 : 1);
    // Gravity pulls down; a nearly vertical thread bows sideways instead.
    const nx = Math.abs(dx) / len; const ny = 1 - nx;
    const cx = (pa.x + pb.x) / 2 + sag * ny * 0.6;
    const cy = (pa.y + pb.y) / 2 + sag * nx;
    return {
      d: `M ${pa.x} ${pa.y} Q ${cx} ${cy} ${pb.x} ${pb.y}`,
      mid: { x: (pa.x + 2 * cx + pb.x) / 4, y: (pa.y + 2 * cy + pb.y) / 4 },
      end: pb,
      angle: Math.atan2(pb.y - cy, pb.x - cx),
    };
  }

  /**
   * The theory board: a Theory Card's units laid out by dependency. `units` come in
   * dependency order (`math.order`), each with its `form` and the units of the theory it
   * `cites`. A unit lies one row below the deepest unit it cites, under the mean slot of
   * those units (ties keep the dependency order). An axiom lies on the bottom row: it holds
   * the theory up. A corollary that follows from one result alone, and that no unit cites,
   * hangs under that result instead of taking a slot. Linear in units and citations.
   * Returns { rows: [[address]], base: [address], hung: Map(address -> [address]) }.
   */
  function theoryRows(units) {
    const index = new Map(units.map((unit, at) => [unit.address, at]));
    const cited = new Set();
    for (const unit of units) for (const premise of unit.cites || []) cited.add(premise);
    const formOf = new Map(units.map((unit) => [unit.address, unit.form]));
    const base = units.filter((unit) => unit.form === 'axiom').map((unit) => unit.address);
    const hung = new Map();
    const hanging = new Set();
    for (const unit of units) {
      const premises = (unit.cites || []).filter((premise) => index.has(premise));
      const host = premises[0];
      if (unit.form !== 'corollary' || premises.length !== 1 || cited.has(unit.address)) continue;
      if (formOf.get(host) === 'axiom' || hanging.has(host)) continue;
      if (!hung.has(host)) hung.set(host, []);
      hung.get(host).push(unit.address);
      hanging.add(unit.address);
    }
    const depth = new Map();
    const rows = [];
    for (const unit of units) {
      if (hanging.has(unit.address) || formOf.get(unit.address) === 'axiom') continue;
      let d = 0;
      for (const premise of unit.cites || []) if (depth.has(premise)) d = Math.max(d, depth.get(premise) + 1);
      depth.set(unit.address, d);
      (rows[d] ||= []).push(unit);
    }
    const slot = new Map();
    const placed = rows.filter(Boolean).map((row) => {
      const key = new Map(row.map((unit) => {
        const xs = (unit.cites || []).map((premise) => slot.get(premise)).filter((x) => x !== undefined);
        return [unit.address, xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : (index.get(unit.address) + 0.5) / units.length];
      }));
      row.sort((a, b) => key.get(a.address) - key.get(b.address) || index.get(a.address) - index.get(b.address));
      row.forEach((unit, at) => slot.set(unit.address, (at + 0.5) / row.length));
      return row.map((unit) => unit.address);
    });
    return { rows: placed, base, hung };
  }

  /**
   * The labels a text names: hypotheses `H1`, `H2`, ... and clauses `C1`, `C2`, ..., as
   * `(H1)`, `（H1）`, or a bare `H1` that no letter or digit touches.
   */
  function labelsIn(text) {
    const found = new Set();
    for (const match of String(text || '').matchAll(/(?<![A-Za-z0-9])([HC][0-9]+)(?![0-9])/g)) found.add(match[1]);
    return found;
  }

  /**
   * The stations of a unit in one reading mode, from its study order (`math.study`: the
   * Learn passes, then the recall pass). Learn takes every station but the recall pass, and
   * the proof plan's station (`isPlan`) shows the tools of every step it plans. Review recalls
   * the statement, then each Why and Boundary station face down, then the exercises. Look
   * up is one station: the statement alone, with every tool its proof cites.
   */
  function studyStations(study, mode, head, isPlan = () => false) {
    const learn = (study || []).filter((station) => station.pass !== 'recall');
    const tools = [];
    const seen = new Set();
    for (const station of learn) {
      for (const tool of station.tools || []) {
        if (!seen.has(tool.address)) { seen.add(tool.address); tools.push(tool); }
      }
    }
    const blank = { companions: [], details: [], tools: [] };
    if (mode === 'lookup') return [{ ...blank, pass: 'look up', focus: [head], tools, lookup: true }];
    if (mode === 'review') {
      const recall = (study || []).filter((station) => station.pass === 'recall');
      const statement = recall.find((station) => station.focus.includes(head)) || { ...blank, pass: 'recall', focus: [head] };
      const middle = learn.filter((station) => station.pass === 'why' || station.pass === 'boundary');
      const exercises = recall.filter((station) => station !== statement);
      return [statement, ...middle, ...exercises].map((station) => ({ ...blank, ...station, face_down: true }));
    }
    return learn.map((station) => (station.focus.some(isPlan) && station.pass === 'why'
      ? { ...blank, ...station, tools: tools.length ? tools : station.tools || [] }
      : { ...blank, ...station }));
  }

  /**
   * The stations of a whole theory in one run, from its theory report (`math.study`,
   * `math.tools`). Learn: Orient (the Theory Card, its first main results beside it), then for
   * each main result its prerequisite path and the result itself, each unit by its own Meet,
   * Why, and Boundary stations, then Connect (the Theory Card, the theories that use it, its
   * tools as tabs) and Recall (each main result face down). Review: each main result's own
   * Review stations. Look up: the Theory Card with its first main results. `unitStations(unit)`
   * gives a unit's stations in the same mode. Each station names its `unit` (the theory's own
   * stations name the Theory Card).
   */
  function theoryRun(math, head, mode, unitStations) {
    const study = (math && math.study) || [];
    const pass = (name) => study.find((station) => station.pass === name);
    const blank = { companions: [], details: [], tools: [] };
    const own = (station) => ({ ...blank, ...station, unit: head });
    const mains = (pass('recall') || {}).focus || [];
    const units = (list, keep) => list.flatMap((unit) => unitStations(unit)
      .filter((station) => !keep || keep.includes(station.pass))
      .map((station) => ({ ...blank, ...station, unit })));
    if (mode === 'lookup') return [own({ pass: 'look up', focus: [head], companions: mains.slice(0, 3), lookup: true })];
    if (mode === 'review') return units(mains);
    const run = [];
    const orient = pass('orient');
    if (orient) run.push(own({ pass: 'orient', focus: orient.focus, companions: orient.companions || [] }));
    for (const result of (pass('path') || {}).results || []) {
      run.push(...units([...result.path, result.result], ['meet', 'why', 'boundary']));
    }
    const connect = pass('connect');
    if (connect) {
      const tools = (math.tools || []).filter((tool) => (connect.tools || []).includes(tool.address));
      run.push(own({ pass: 'connect', focus: connect.focus, companions: (connect.users || []).filter(Boolean).slice(0, 3), tools }));
    }
    run.push(...mains.map((at) => own({ pass: 'recall', focus: [at], face_down: true })));
    return run;
  }
  /**
   * The model board in blocks (#200, #201; docs/model-board-blocks-proposal.md §3a). `blocks` are
   * the model's blocks in reading order (`id` at least), `edges` its `block_edges`, `focus` the open
   * block's id or null (the Model Card itself is open).
   *
   * Attention: the focus block; its previous and next block and the two blocks it shares the most
   * Links with are `support`; every other block is `far`. With no focus every block is `near`.
   * Ribbons: one per block pair and kind (`refers` only with `options.refers`), each in a lane of the
   * right gutter; a ribbon never shares a lane with one whose span overlaps it, shorter spans
   * inside. `up` marks a rests-on ribbon from a block to a later one (the forward fault); `hot` one
   * that touches the focus. Ports: per Card, the blocks its Links leave to (`out`) or come from (`in`).
   *
   * With `options.backbone` and no focus (the Model Card is open), the overview draws only each
   * block's heaviest rests-on ribbon to an earlier block and every `up` ribbon; `folded` counts
   * the rest. `options.hover` (a block id) draws that block's ribbons instead and moves no tray.
   * `unanchored`: the chapter blocks after the first with no rests-on edge to an earlier block.
   * Each ribbon has a `fromSlot` and a `toSlot`: its rank among the ribbons that meet that tray,
   * so two ribbons never leave or reach a tray at one point.
   * Linear in blocks plus edges, apart from the lane search over the lanes in use.
   */
  function blockLayout(blocks, edges, focus, options = {}) {
    const at = new Map(blocks.map((b, i) => [b.id, i]));
    const shown = edges.filter((e) => at.has(e.from_block) && at.has(e.to_block) && (options.refers || e.kind !== 'refers'));
    const f = focus != null && at.has(focus) ? at.get(focus) : null;
    // With `focusOnly`, an open block (or the block pointed at) draws only its own ribbons: the
    // others would compete with it. With no focus, the backbone stands for the whole model.
    const lens = f != null ? f : options.hover != null && at.has(options.hover) ? at.get(options.hover) : null;
    const isUp = (e) => e.kind === 'rests-on' && at.get(e.from_block) < at.get(e.to_block);
    let drawn = shown;
    if (options.focusOnly && lens != null) drawn = shown.filter((e) => at.get(e.from_block) === lens || at.get(e.to_block) === lens);
    else if (options.backbone && f == null) {
      const best = new Map();
      for (const e of shown) {
        if (e.kind !== 'rests-on' || isUp(e)) continue;
        const was = best.get(e.from_block);
        if (!was || e.count > was.count || (e.count === was.count && at.get(e.to_block) > at.get(was.to_block))) best.set(e.from_block, e);
      }
      const keep = new Set(best.values());
      drawn = shown.filter((e) => keep.has(e) || isUp(e));
    }
    const anchored = new Set(edges.filter((e) => e.kind === 'rests-on' && at.has(e.from_block) && at.has(e.to_block) && !isUp(e) && e.from_block !== e.to_block).map((e) => e.from_block));
    const unanchored = blocks.filter((b, i) => i > 0 && b.kind === 'chapter' && !anchored.has(b.id)).map((b) => b.id);
    const support = new Set();
    if (f != null) {
      if (f > 0) support.add(f - 1);
      if (f < blocks.length - 1) support.add(f + 1);
      const weight = new Map();
      for (const e of shown) {
        const a = at.get(e.from_block);
        const b = at.get(e.to_block);
        const other = a === f ? b : b === f ? a : null;
        if (other != null && other !== f) weight.set(other, (weight.get(other) || 0) + e.count);
      }
      [...weight.entries()].filter(([i]) => !support.has(i)).sort((x, y) => y[1] - x[1] || x[0] - y[0]).slice(0, 2).forEach(([i]) => support.add(i));
    }
    const trays = blocks.map((b, i) => ({ id: b.id, level: f == null ? 'near' : i === f ? 'focus' : support.has(i) ? 'support' : 'far' }));
    const lanes = [];
    const ribbons = drawn
      .map((e) => {
        const a = at.get(e.from_block);
        const b = at.get(e.to_block);
        return { ...e, lo: Math.min(a, b), hi: Math.max(a, b), up: isUp(e), hot: lens != null && (a === lens || b === lens) };
      })
      .sort((x, y) => (x.hi - x.lo) - (y.hi - y.lo) || x.lo - y.lo)
      .map((r) => {
        let lane = 0;
        while ((lanes[lane] || []).some(([lo, hi]) => r.lo < hi && lo < r.hi)) lane += 1;
        (lanes[lane] ||= []).push([r.lo, r.hi]);
        return { ...r, lane };
      });
    const ports = {};
    const add = (card, dir, block) => {
      const list = (ports[card] ||= []);
      const found = list.find((p) => p.dir === dir && p.block === block);
      if (found) found.count += 1;
      else list.push({ dir, block, count: 1 });
    };
    for (const e of shown) for (const p of e.pairs || []) { add(p.from, 'out', e.to_block); add(p.to, 'in', e.from_block); }
    // Slots: where several ribbons meet one tray, the outer lane meets it lowest.
    const meet = new Map();
    for (const r of ribbons) for (const end of [r.from_block, r.to_block]) (meet.get(end) || meet.set(end, []).get(end)).push(r);
    for (const list of meet.values()) list.sort((x, y) => x.lane - y.lane);
    for (const r of ribbons) {
      r.fromSlot = meet.get(r.from_block).indexOf(r);
      r.toSlot = meet.get(r.to_block).indexOf(r);
    }
    return { trays, ribbons, ports, lanes: lanes.length, folded: shown.length - drawn.length, unanchored };
  }

  /**
   * The two-block view of one ribbon (#201): the Cards of the block rested on (`to`) on the left,
   * the Cards that rest on them on the right, each once in Folgezettel order, and per Card pair one
   * thread `[left index, right index]`.
   */
  /**
   * The model network's tree (#200, #201): each block's parent is the earlier block it rests on
   * most (ties to the later block); a block with none is a root. `children` lists each parent's
   * blocks in reading order. Plain references and rests-on Links to later blocks never make a parent.
   */
  function blockTree(blocks, edges) {
    const at = new Map(blocks.map((b, i) => [b.id, i]));
    const best = new Map();
    for (const e of edges) {
      if (e.kind !== 'rests-on' || !at.has(e.from_block) || !at.has(e.to_block) || at.get(e.to_block) >= at.get(e.from_block)) continue;
      const was = best.get(e.from_block);
      if (!was || e.count > was.count || (e.count === was.count && at.get(e.to_block) > at.get(was.to_block))) best.set(e.from_block, e);
    }
    const parent = {};
    const children = {};
    const roots = [];
    for (const b of blocks) {
      const p = best.get(b.id);
      parent[b.id] = p ? p.to_block : null;
      if (p) (children[p.to_block] ||= []).push(b.id);
      else roots.push(b.id);
    }
    return { parent, children, roots };
  }

  /**
   * A tidy tree laid out top to bottom (#200, #201): one layer per depth, as tall as its tallest
   * card; siblings left to right in the given order, each subtree packed against the one before it
   * by their contours (layer by layer, so a small subtree tucks in beside a wide one); a parent
   * centred over its first and last child; roots side by side. `size(id)` gives `{w, h}`. Returns
   * each card's box `{x, y, w, h, cx, depth}` and the whole width and height. Cards times layers.
   */
  function treeLayout(roots, children, size, options = {}) {
    const gapX = options.gapX ?? 14;
    const gapY = options.gapY ?? 34;
    const kids = (id) => children[id] || [];
    const depth = {};
    const layerH = [];
    // Each subtree is laid out on its own, x relative to its left edge, with its contour: the
    // leftmost and rightmost x it holds on each layer below its root.
    const sub = (id, d) => {
      depth[id] = d;
      const s = size(id);
      layerH[d] = Math.max(layerH[d] || 0, s.h);
      const list = kids(id).map((c) => sub(c, d + 1));
      const xs = { [id]: 0 };
      let left = [];
      let right = [];
      let cx;
      if (!list.length) {
        cx = s.w / 2;
      } else {
        const offsets = [];
        list.forEach((t, i) => {
          let off = 0;
          if (i) for (let k = 0; k < Math.min(right.length, t.left.length); k += 1) off = Math.max(off, right[k] + gapX - t.left[k]);
          offsets.push(off);
          for (let k = 0; k < t.left.length; k += 1) {
            left[k] = left[k] === undefined ? t.left[k] + off : Math.min(left[k], t.left[k] + off);
            right[k] = right[k] === undefined ? t.right[k] + off : Math.max(right[k], t.right[k] + off);
          }
          for (const [n, x] of Object.entries(t.xs)) xs[n] = x + off;
        });
        cx = (list[0].cx + offsets[0] + list[list.length - 1].cx + offsets[list.length - 1]) / 2;
      }
      xs[id] = cx - s.w / 2;
      left = [xs[id], ...left];
      right = [xs[id] + s.w, ...right];
      // Normalise: the subtree's leftmost point is 0.
      const min = Math.min(...left);
      if (min) {
        for (const n of Object.keys(xs)) xs[n] -= min;
        left = left.map((x) => x - min);
        right = right.map((x) => x - min);
        cx -= min;
      }
      return { xs, left, right, cx };
    };
    const forest = { xs: {}, left: [], right: [] };
    roots.forEach((r, i) => {
      const t = sub(r, 0);
      let off = 0;
      if (i) for (let k = 0; k < Math.min(forest.right.length, t.left.length); k += 1) off = Math.max(off, forest.right[k] + gapX * 2 - t.left[k]);
      for (let k = 0; k < t.left.length; k += 1) {
        forest.left[k] = forest.left[k] === undefined ? t.left[k] + off : Math.min(forest.left[k], t.left[k] + off);
        forest.right[k] = forest.right[k] === undefined ? t.right[k] + off : Math.max(forest.right[k], t.right[k] + off);
      }
      for (const [n, x] of Object.entries(t.xs)) forest.xs[n] = x + off;
    });
    const top = [];
    layerH.reduce((y, h, d) => { top[d] = y; return y + h + gapY; }, 0);
    const pos = {};
    for (const [id, x] of Object.entries(forest.xs)) {
      const s = size(id);
      const d = depth[id];
      pos[id] = { x, y: top[d] + (layerH[d] - s.h) / 2, w: s.w, h: s.h, cx: x + s.w / 2, depth: d };
    }
    const width = forest.right.length ? Math.max(...forest.right) - Math.min(0, ...forest.left) : 0;
    const height = layerH.length ? top[layerH.length - 1] + layerH[layerH.length - 1] : 0;
    return { pos, width, height };
  }

  /**
   * A thread of the model network from `p1` (on its premise's edge) to `p2` (on its dependent's
   * edge). With no card in the layers between them it is one curve. Otherwise it leaves into the
   * gap beside its premise (`half` the layer gap away), runs along the free corridor between the
   * cards `between` (the one nearest its own line, `shift` apart from its neighbours), and enters
   * its dependent from the gap beside it, corners rounded. Returns `{ d, mid, lane }`.
   */
  function threadPath(p1, p2, between, options = {}) {
    const half = options.half ?? 17;
    const down = p1.y < p2.y;
    if (!between.length) {
      const k = Math.max(18, Math.abs(p2.y - p1.y) * 0.45) * (down ? 1 : -1);
      return { d: `M ${p1.x} ${p1.y} C ${p1.x} ${p1.y + k} ${p2.x} ${p2.y - k} ${p2.x} ${p2.y}`, mid: { x: (p1.x + p2.x) / 2, y: (p1.y + p2.y) / 2 }, lane: null, points: [[p1.x, p1.y], [p2.x, p2.y]] };
    }
    const spans = between.map((b) => [b.x - 6, b.x + b.w + 6]).sort((a, b) => a[0] - b[0]);
    const merged = [];
    for (const s of spans) {
      const last = merged[merged.length - 1];
      if (last && s[0] <= last[1]) last[1] = Math.max(last[1], s[1]);
      else merged.push([...s]);
    }
    const free = [merged[0][0] - 8, merged[merged.length - 1][1] + 8];
    for (let i = 1; i < merged.length; i += 1) if (merged[i][0] - merged[i - 1][1] >= 8) free.push((merged[i][0] + merged[i - 1][1]) / 2);
    const line = (p1.x + p2.x) / 2;
    const lane = free.sort((a, b) => Math.abs(a - line) - Math.abs(b - line) || b - a)[0] + (options.shift || 0);
    const g1 = p1.y + (down ? half : -half);
    const g2 = p2.y + (down ? -half : half);
    const pts = [[p1.x, p1.y], [p1.x, g1], [lane, g1], [lane, g2], [p2.x, g2], [p2.x, p2.y]]
      .filter((p, i, all) => i === 0 || p[0] !== all[i - 1][0] || p[1] !== all[i - 1][1]);
    const points = pts.map((p) => [...p]);
    let d = `M ${pts[0][0]} ${pts[0][1]}`;
    const seg = (a, b) => (a[0] === b[0] ? `V ${b[1]}` : `H ${b[0]}`);
    for (let i = 1; i < pts.length - 1; i += 1) {
      const [a, c, b] = [pts[i - 1], pts[i], pts[i + 1]];
      const r = Math.min(10, Math.hypot(c[0] - a[0], c[1] - a[1]) / 2, Math.hypot(b[0] - c[0], b[1] - c[1]) / 2);
      const before = [c[0] - Math.sign(c[0] - a[0]) * r, c[1] - Math.sign(c[1] - a[1]) * r];
      const after = [c[0] + Math.sign(b[0] - c[0]) * r, c[1] + Math.sign(b[1] - c[1]) * r];
      d += ` ${seg(a, before)} Q ${c[0]} ${c[1]} ${after[0]} ${after[1]}`;
      pts[i] = after;
    }
    d += ` ${seg(pts[pts.length - 2], pts[pts.length - 1])}`;
    return { d, mid: { x: lane, y: (g1 + g2) / 2 }, lane, points };
  }

  /**
   * How far a pinned card leans about its pin (#201): a small angle from its address, -1.2° to
   * 1.2°, so the board looks pinned by hand and the same card always hangs the same way.
   */
  function tilt(address) {
    let hash = 2166136261;
    for (const ch of String(address)) {
      hash ^= ch.codePointAt(0);
      hash = Math.imul(hash, 16777619) >>> 0;
    }
    return Math.round(((hash % 2401) / 1000 - 1.2) * 100) / 100;
  }

  /**
   * A transit line along an orthogonal route (#201): straight legs, and each corner rounded by
   * `radius`, or by half the shorter leg beside it when that is less, as on a metro map.
   */
  function roundedPath(points, radius) {
    const pts = points.map((p) => [...p]);
    const seg = (a, b) => (a[0] === b[0] ? `V ${b[1]}` : `H ${b[0]}`);
    let d = `M ${pts[0][0]} ${pts[0][1]}`;
    for (let i = 1; i < pts.length - 1; i += 1) {
      const [a, c, b] = [pts[i - 1], pts[i], pts[i + 1]];
      const r = Math.min(radius, Math.hypot(c[0] - a[0], c[1] - a[1]) / 2, Math.hypot(b[0] - c[0], b[1] - c[1]) / 2);
      const before = [c[0] - Math.sign(c[0] - a[0]) * r, c[1] - Math.sign(c[1] - a[1]) * r];
      const after = [c[0] + Math.sign(b[0] - c[0]) * r, c[1] + Math.sign(b[1] - c[1]) * r];
      d += ` ${seg(a, before)} Q ${c[0]} ${c[1]} ${after[0]} ${after[1]}`;
      pts[i] = after;
    }
    return `${d} ${seg(pts[pts.length - 2], pts[pts.length - 1])}`;
  }

  function bridgeLayout(edge) {
    const pairs = edge.pairs || [];
    const left = [...new Set(pairs.map((p) => p.to))].sort(addressOrder);
    const right = [...new Set(pairs.map((p) => p.from))].sort(addressOrder);
    const li = new Map(left.map((a, i) => [a, i]));
    const ri = new Map(right.map((a, i) => [a, i]));
    return { left, right, threads: pairs.map((p) => [li.get(p.to), ri.get(p.from)]) };
  }

  return {
    blockLayout, bridgeLayout, blockTree, treeLayout, threadPath, roundedPath, tilt,
    sideRank, outline, pathTo, focusLens, edgePath, elbow, edgeSplit, isTermEdge, focusEdges, linkArc, constellation, LOD, GUTTER,
    TABLE, addressOrder, freeSpot, readingOrder, lineUp, gather, byTree, thread,
    theoryRows, labelsIn, studyStations, theoryRun,
  };
});
