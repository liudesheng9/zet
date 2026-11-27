/* zt gui connections — pure, DOM-free rules for Connection trees (关联树), Topic-side mounts and the
 * Dependency Lens in the page (docs/multi-tree-coordinates-proposal.md §9.1).
 *
 * The page reads and writes through `POST /api/connection` ({tool, args}: the MCP tool names) as
 * the owner. These rules decide how the answers read: the outline of a tree's stations (the Direct
 * chain of the root as top-level peers, Side branches indented), which tree connections the Map
 * shows for the focused station (anti-hairball default; the full overlay is opt-in), the plain
 * provenance label of every edge ("In this tree · ~7" vs "In the Card · BookB/5|a|2"), directed
 * assertions as sentences, the citation and bookmark a copy action writes, and the station route.
 * The server recomputes every fact; nothing here is authoritative. Like lock.js this module runs
 * in the page and under Node.
 */
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.ZtConnections = api;
})(typeof self !== 'undefined' ? self : this, () => {
  'use strict';

  const GLOSS_LIMIT = 240;
  const LABEL_LIMIT = 32;
  const NOTE_LIMIT = 400;
  const DIRECTIONAL = ['rests-on', 'illustrates', 'relaxes', 'objects', 'specializes'];
  const KINDS = ['rests-on', 'illustrates', 'contrasts', 'relaxes', 'corresponds', 'objects', 'specializes', 'refers'];

  /**
   * The stations of one tree in reading order with their depth: the root at 0, the root's Direct
   * chain as top-level peers at 1, each Side branch one deeper than its parent and its own Direct
   * chain at the same depth. `stations` are `get_connection_tree` placements (any order).
   */
  function outline(stations) {
    const byId = new Map(stations.map((s) => [s.placement_id, s]));
    const direct = new Map();
    const sides = new Map();
    let rootId = null;
    for (const s of stations) {
      if (s.edge === 'root') rootId = s.placement_id;
      else if (s.edge === 'direct') direct.set(s.parent_id, s.placement_id);
      else if (s.edge === 'side') {
        if (!sides.has(s.parent_id)) sides.set(s.parent_id, []);
        sides.get(s.parent_id).push(s);
      }
    }
    for (const list of sides.values()) list.sort((a, b) => (a.ordinal || 0) - (b.ordinal || 0));
    const rows = [];
    const seen = new Set();
    const walkChain = (id, depth) => {
      // A Direct chain stays at one depth; each station's Side branches go one deeper.
      let at = id;
      while (at && !seen.has(at)) {
        seen.add(at);
        const station = byId.get(at);
        if (!station) break;
        rows.push({ ...station, depth });
        for (const side of sides.get(at) || []) walkChain(side.placement_id, depth + 1);
        at = direct.get(at);
      }
    };
    if (rootId) {
      seen.add(rootId);
      rows.push({ ...byId.get(rootId), depth: 0 });
      walkChain(direct.get(rootId), 1);
    }
    // Anything unreachable (should not exist) still shows, last, so nothing is hidden.
    for (const s of stations) if (!seen.has(s.placement_id)) rows.push({ ...s, depth: 1, orphan: true });
    return rows;
  }

  /** How many connections touch each station. */
  function counts(connections) {
    const n = new Map();
    for (const c of connections || []) {
      for (const end of [c.from && c.from.placement_id, c.to && c.to.placement_id]) {
        if (end) n.set(end, (n.get(end) || 0) + 1);
      }
    }
    return n;
  }

  /**
   * The connections the Map draws: by default only those incident to the focused station; with
   * `overlay` all of them. Each carries its role relative to the focus.
   */
  function focusEdges(connections, focusId, overlay) {
    const out = [];
    for (const c of connections || []) {
      const from = c.from && c.from.placement_id;
      const to = c.to && c.to.placement_id;
      const touches = from === focusId || to === focusId;
      if (!overlay && !touches) continue;
      let role = 'other';
      if (touches) role = c.direction === 'neutral' ? 'neutral' : (from === focusId ? 'outgoing' : 'incoming');
      out.push({ ...c, role });
    }
    return out;
  }

  /** Split one station's connections into incoming, outgoing and neutral lists. */
  function panel(connections, focusId) {
    const lists = { incoming: [], outgoing: [], neutral: [] };
    for (const c of focusEdges(connections, focusId, false)) lists[c.role].push(c);
    return lists;
  }

  /** The plain provenance label of an edge drawn on the Map or in the Reader. */
  function provenance(edge) {
    if (edge && edge.provenance) return edge.provenance;
    if (edge && edge.carrier) return `In the Card · ${edge.carrier}`;
    return 'In this tree';
  }

  /** A directed assertion reads as a sentence; a neutral pair has no arrow and no claim. */
  function sentence(edge) {
    const from = (edge.from && edge.from.title) || '';
    const to = (edge.to && edge.to.title) || '';
    const predicate = edge.relation_label || edge.relation_kind || (edge.direction === 'directed' ? 'connects to' : '');
    if (edge.direction === 'directed') return `${from} ${predicate} ${to}`.replace(/\s+/g, ' ').trim();
    return predicate ? `${from} — ${predicate} — ${to}` : `${from} — ${to}`;
  }

  /** The labels already used in a tree, most used first, for suggestion. */
  function labelSuggestions(connections) {
    const n = new Map();
    for (const c of connections || []) if (c.relation_label) n.set(c.relation_label, (n.get(c.relation_label) || 0) + 1);
    return [...n.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).map(([label]) => label);
  }

  /**
   * Check a new connection the way the server does, before sending it: a label of 1–32 Unicode
   * scalar values on one line, a note of at most 400, no `[[…]]`, and a directional kind needs a
   * direction. Returns `{ ok, error }`.
   */
  function checkConnection(input) {
    const label = input.relation_label == null ? '' : String(input.relation_label).trim();
    const note = input.note == null ? '' : String(input.note).trim();
    if (Array.from(label).length > LABEL_LIMIT) return { ok: false, error: `a label is at most ${LABEL_LIMIT} characters` };
    if (/[\r\n]/.test(label)) return { ok: false, error: 'a label is one line' };
    if (Array.from(note).length > NOTE_LIMIT) return { ok: false, error: `a note is at most ${NOTE_LIMIT} characters` };
    if (label.includes('[[') || note.includes('[[')) return { ok: false, error: 'labels and notes are plain text; cite Cards in a Card body' };
    if (input.relation_kind && !KINDS.includes(input.relation_kind)) return { ok: false, error: `unknown kind ${input.relation_kind}` };
    if (input.relation_kind && DIRECTIONAL.includes(input.relation_kind) && input.direction !== 'directed') {
      return { ok: false, error: `${input.relation_kind} has a direction: choose directed` };
    }
    return { ok: true, error: null };
  }

  /** A gloss as typed: at most 240 scalar values on one line, no Links; blank clears it. */
  function checkGloss(raw) {
    const text = raw == null ? '' : String(raw).trim();
    if (!text) return { ok: true, gloss: null };
    if (Array.from(text).length > GLOSS_LIMIT) return { ok: false, error: `a gloss is at most ${GLOSS_LIMIT} characters` };
    if (/[\r\n]/.test(text)) return { ok: false, error: 'a gloss is one line' };
    if (text.includes('[[')) return { ok: false, error: 'a gloss is plain text and creates no Link' };
    return { ok: true, gloss: text };
  }

  /** The two copy actions: the Card's citation (handle when it has one) and the station bookmark. */
  function citation(station) {
    return station && station.citation ? station.citation : `[[${station.home_address}]]`;
  }

  function bookmark(station) {
    const label = (station.title || station.coord || 'station').replace(/[\[\]]/g, '');
    return `[${label}](place:${station.placement_id})`;
  }

  /** The station route: an occurrence is addressed by its placement ID, never by its coordinate. */
  function stationHash(placementId) {
    return '#/place/' + encodeURIComponent(placementId);
  }

  function treeHash(handle) {
    return '#/conn/' + encodeURIComponent(handle);
  }

  /** Parse a page hash into `{ kind: 'place'|'conn'|'card'|'root', value }`. */
  function parseHash(hash) {
    const m = /^#\/(place|conn|card)\/(.+)$/.exec(hash || '');
    if (!m) return { kind: 'root', value: null };
    return { kind: m[1], value: decodeURIComponent(m[2]) };
  }

  /** The portal line of a mounted tree root: one node, never expanded. */
  function portalLabel(station) {
    if (!station || !station.portal) return null;
    const what = station.portal.role === 'literature' ? 'Book' : station.portal.role === 'connection' ? 'Connection tree' : 'Topic';
    return `${what} · ${station.portal.cards} cards`;
  }

  /** The gloss-review lines of a plan answer, for the dialog. */
  function glossReview(plan) {
    return ((plan && plan.effects && plan.effects.gloss_review) || []).map((r) => ({
      placement_id: r.placement_id, coord: r.coord, gloss: r.gloss,
    }));
  }

  /** Mark the exact invoking macro in a carrier body: [before, macro, after] by UTF-8 byte range. */
  function markByBytes(body, range) {
    const bytes = new TextEncoder().encode(body);
    const dec = new TextDecoder();
    const [start, end] = range;
    return [dec.decode(bytes.slice(0, start)), dec.decode(bytes.slice(start, end)), dec.decode(bytes.slice(end))];
  }

  // ------------------------------------------------------------ one Map for every tree role
  // A Connection tree is drawn on the same desk as a Topic or Literature tree: its stations are
  // index cards in the same focus lens. Cards pulled from different drawers keep the colour of their
  // drawer: each home tree has one hue, the same in every view, from a palette of ten that stay
  // apart at a glance and leave out the pointer's gold and the Link's blue. A Connection tree's own
  // Cards wear the Literature colour (docs/connection-trees-free-proposal.md D6). In one page no two
  // trees share a hue (up to nine).

  /** The tree a Card lives in: `~7` for `~7` and `~7#4`, else the part before the first `/`. */
  function treeOf(address) {
    const text = String(address || '');
    const handle = text.match(/^~\d+/);
    return handle ? handle[0] : text.split('/')[0];
  }

  /** A tree's role from its id: a handle, a Topic number, or a Citation key. */
  function roleOf(tree) {
    const text = String(tree || '');
    return text.startsWith('~') ? 'connection' : /^\d/.test(text) ? 'topic' : 'literature';
  }

  // OKLCH hues: lime, green, teal, cyan, sky, indigo, violet, plum, rose. Gold (the pointer, ~85)
  // and blue (a Link, ~255) stay out, and so do the red and orange beside the Literature colour
  // (~36), which is a Connection tree's own: no mounted tree is mistaken for the tree's own Cards.
  const PALETTE = [112, 140, 164, 190, 214, 280, 300, 328, 354];
  const OWN_HUE = 36;

  /** A tree's place in the palette: a hash of its id, the same in every view. */
  function paletteSlot(tree) {
    let hash = 2166136261;
    for (const ch of String(tree || '')) {
      hash ^= ch.codePointAt(0);
      hash = Math.imul(hash, 16777619) >>> 0;
    }
    return hash % PALETTE.length;
  }

  // One hue per tree for the whole page: the Map's card stock, its colour key, the stamps and the
  // chips all read it here, so a tree never shows in two colours. A tree keeps the first hue it is
  // given; a tree whose hash meets a hue already given takes the next free one (past nine trees two
  // share one). A Connection tree wears the Literature colour.
  const given = new Map();

  /** The hue (OKLCH degrees) of one tree in this page. */
  function treeHue(tree, role) {
    if ((role || roleOf(tree)) === 'connection') return OWN_HUE;
    const id = String(tree || '');
    if (given.has(id)) return given.get(id);
    const taken = new Set(given.values());
    const start = paletteSlot(id);
    let hue = PALETTE[start];
    for (let step = 1; step < PALETTE.length && taken.has(hue); step += 1) hue = PALETTE[(start + step) % PALETTE.length];
    given.set(id, hue);
    return hue;
  }

  /**
   * A Connection tree as the Map draws it: one Card summary per station, keyed by coordinate, with
   * the outline fields (`parent`, `edge`), the tree connections as `links` (a neutral connection
   * runs both ways) and `inbound` counts, and `station`: the home address, home tree, role, hue,
   * origin, status, gloss and portal of what stands there. `tree` is a `get_connection_tree` answer.
   */
  function mapTree(tree) {
    const stations = (tree && tree.placements) || [];
    const byId = new Map(stations.map((s) => [s.placement_id, s]));
    const coordOf = (id) => (byId.has(id) ? byId.get(id).coord : null);
    const out = new Map();
    const inn = new Map();
    const add = (map, key, value) => {
      if (!map.has(key)) map.set(key, []);
      if (!map.get(key).includes(value)) map.get(key).push(value);
    };
    for (const e of (tree && tree.connections) || []) {
      const a = coordOf(e.from && e.from.placement_id);
      const b = coordOf(e.to && e.to.placement_id);
      if (!a || !b || a === b) continue;
      add(out, a, b); add(inn, b, a);
      if (e.direction !== 'directed') { add(out, b, a); add(inn, a, b); }
    }
    const own = tree && tree.tree;
    let root = null;
    const byCoord = new Map();
    const cards = stations.map((s) => {
      if (s.edge === 'root') root = s.coord;
      const home = s.home_address || '';
      const homeTree = s.edge === 'root' ? own : treeOf(home);
      const role = s.edge === 'root' ? 'connection' : roleOf(homeTree);
      byCoord.set(s.coord, s.placement_id);
      return {
        address: s.coord,
        parent: s.parent_id ? coordOf(s.parent_id) : null,
        edge: s.edge,
        // An untitled root (its title is its own handle) reads as untitled (D2).
        title: s.edge === 'root' && (tree.untitled || s.title === own) ? '' : (s.title || home),
        type: s.type || 'idea',
        excerpt: s.excerpt || '',
        links: out.get(s.coord) || [],
        kinds: {},
        inbound: (inn.get(s.coord) || []).length,
        kind: s.edge === 'root' ? 'connection' : undefined,
        station: {
          id: s.placement_id, home, tree: homeTree, role, hue: treeHue(homeTree, role),
          origin: s.edge === 'root' ? 'root' : s.origin || 'mount', status: s.status || 'live',
          gloss: s.gloss || '', portal: s.portal || null, places: s.places || 0,
        },
      };
    });
    return { tree: 'conn:' + own, handle: own, kind: 'connection', title: (tree && tree.title) || own, root, cards, byCoord, inbound: inn };
  }

  /** The focus the Map lays out at one station: its coordinate and its connections as Links. */
  function focusCard(map, coord) {
    const card = map.cards.find((c) => c.address === coord) || map.cards.find((c) => c.address === map.root);
    const address = card ? card.address : coord;
    const near = map.inbound.get(address) || [];
    return {
      address, tree: map.tree, model: null,
      outbound: ((card && card.links) || []).map((at) => ({ address: at })),
      inbound: near.map((at) => ({ address: at })),
      inbound_near: near,
    };
  }

  // ------------------------------------------------------------ the projection (D4)
  // A Connection tree is a structure in a higher dimension; the Topic or Literature tree being read
  // is a plane. `crossings` (the tree answer's field, `connection::crossings`) names, per crossing
  // tree, its stations in the plane, its lines in the plane and its exits. These rules choose what
  // the Map draws: the lines that touch the focus or the hovered Card, or every line with the
  // Crossings switch on, each line once even when two trees draw the same pair.

  /** Every projection line of every crossing tree, each with its tree and a key unique in the page. */
  function projectionLines(crossings) {
    const out = [];
    for (const [tree, cross] of Object.entries(crossings || {})) {
      for (const line of (cross && cross.lines) || []) {
        if (!line.from || !line.to || line.from === line.to) continue;
        const id = line.kind === 'connection' ? line.connection_id : `${line.from_coord}>${line.to_coord}`;
        out.push({ ...line, tree, title: (cross && cross.title) || '', key: `x${tree}|${line.kind}|${id}` });
      }
    }
    return out;
  }

  /**
   * The projection lines indexed once per tree answer: every line with its `lane` (0, 1, … among
   * the lines that join the same two Cards, whichever way, so parallel assertions and two trees
   * over one pair lie side by side) and the lines at each address.
   */
  function crossingIndex(crossings) {
    const all = projectionLines(crossings);
    const lanes = new Map();
    const byEnd = new Map();
    for (const line of all) {
      const pair = [line.from, line.to].sort().join('\u0001');
      line.lane = lanes.get(pair) || 0;
      lanes.set(pair, line.lane + 1);
      for (const end of [line.from, line.to]) {
        if (!byEnd.has(end)) byEnd.set(end, []);
        byEnd.get(end).push(line);
      }
    }
    return { all, byEnd };
  }

  /**
   * The projection lines the Map draws now: with `all` every line; otherwise those that touch the
   * focused or the hovered address, each once. Takes the `crossings` field or its `crossingIndex`.
   */
  function visibleCrossings(crossings, focus, hovered, all) {
    const index = crossings && crossings.byEnd ? crossings : crossingIndex(crossings);
    if (all) return index.all;
    const out = new Set([...(index.byEnd.get(focus) || []), ...(hovered ? index.byEnd.get(hovered) || [] : [])]);
    return index.all.filter((line) => out.has(line));
  }

  /** How a projection line reads in its tooltip: its tree, what it is, and its hops outside. */
  function crossingText(line) {
    const tree = line.title ? `${line.tree} · ${line.title}` : line.tree;
    if (line.kind === 'connection') {
      const word = line.relation_label || line.relation_kind || (line.direction === 'directed' ? 'connects to' : 'connected');
      return `In ${tree}: ${line.from_coord} ${line.direction === 'directed' ? '→' : '—'} ${line.to_coord} · ${word}`;
    }
    const how = line.edge === 'side' ? 'a Side branch of' : 'after';
    const through = line.hops ? ` · through ${line.hops} station${line.hops === 1 ? '' : 's'} outside this tree` : '';
    return `In ${tree}: ${line.to_coord} stands ${how} ${line.from_coord}${through}`;
  }

  /** The crossing trees that leave the plane at one address: [{ tree, n }] by tree. */
  function exitsAt(crossings, address) {
    const out = [];
    for (const [tree, cross] of Object.entries(crossings || {})) {
      const n = cross && cross.exits && cross.exits[address];
      if (n) out.push({ tree, n });
    }
    return out.sort((a, b) => a.tree.localeCompare(b.tree, undefined, { numeric: true }));
  }

  /** A tree's name in a list: its title, or its handle when the root is untitled. */
  function treeLabel(tree) {
    if (!tree) return '';
    return tree.untitled || !tree.title ? `${tree.tree} (untitled)` : tree.title;
  }

  /** Several Cards typed into one Mount: split on spaces, commas, semicolons and new lines. */
  function mountTargets(text) {
    const seen = new Set();
    const out = [];
    for (const part of String(text || '').split(/[\s,;，；]+/)) {
      const at = part.trim();
      if (at && !seen.has(at)) { seen.add(at); out.push(at); }
    }
    return out;
  }

  return {
    GLOSS_LIMIT, LABEL_LIMIT, NOTE_LIMIT, KINDS, DIRECTIONAL,
    projectionLines, crossingIndex, visibleCrossings, crossingText, exitsAt, treeLabel, mountTargets,
    outline, counts, focusEdges, panel, provenance, sentence, labelSuggestions,
    checkConnection, checkGloss, citation, bookmark, stationHash, treeHash, parseHash,
    portalLabel, glossReview, markByBytes,
    PALETTE, OWN_HUE, treeOf, roleOf, treeHue, mapTree, focusCard,
  };
});
