/* zt portable export — the service, answered in the page.
 *
 * A portable export runs the real GUI (`app.js`, `app.css`, every view) with no service behind it.
 * The page's one way out is `api()`; this module answers the same `/api/*` routes from `cards.json`,
 * in the same shapes, and refuses what needs the service with one sentence the page shows. It is
 * DOM-free, and like `layout.js` it also loads as CommonJS, for its tests.
 */
((root, factory) => {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.ZtPortableRules = api;
})(typeof self !== 'undefined' ? self : this, () => {
  'use strict';

  const FORMAT = 'zt-portable-export';
  const VERSIONS = [2, 3, 4];
  const INBOUND_HEAD = 50;
  const SEARCH_LIMIT = 80;
  const SNIPPET = 40;

  /** The one sentence every unsupported function shows. */
  const notice = (what) => `${what} is not available in the portable version.`;
  const elsewhere = (address) => `Card \`${address}\` is not in this portable export.`;

  // What each route that needs the service is called on screen.
  const REFUSED = {
    ask: 'Ask the box',
    batch: 'Changing the tree',
    create: 'Writing a Card',
    delete: 'Deleting a Card',
    edit: 'Editing a Card',
    export: 'Exporting a tree',
    'hub/check': 'Filing a hub',
    'hub/draft': 'Filing a hub',
    insert: 'Inserting a Card',
    literature: 'Writing a Literature Card',
    'literature/check': 'Writing a Literature Card',
    'literature/type': 'Changing the Literature type',
    locks: 'Tree Locks',
    metadata: 'Editing a Card',
    move: 'Moving a Card',
    render: 'The writing desk',
    reply: 'Reply',
    shutdown: 'Stopping the service',
    stop: 'Stopping the service',
  };

  /** Why a parsed file is no export this reader opens, or `null` when it is one. */
  function refusal(data) {
    if (!data || data.format !== FORMAT) return 'This file is not a ZET export.';
    if (!VERSIONS.includes(data.version) || (data.version === 3 && !data.connection_tree)) {
      return `This export has version ${data.version}; this page reads versions 2, 3, and 4.`;
    }
    if (!Array.isArray(data.cards) || !data.cards.length) return 'This export holds no Cards.';
    return null;
  }

  /** The body of an authored text: everything under its `# title` line. */
  function bodyOf(text) {
    return (text || '').replace(/^\s*#[^\n]*\n*/, '').trim();
  }

  /** Stored text as a reader sees it: a Term link reads as its label. */
  function visibleText(text) {
    return bodyOf(text).replace(/\[\[([^\[\]\n@]*)@[^\[\]\n]*\]\]/g, '$1');
  }

  /** Everything the routes read, made once when an export opens. */
  function open(data) {
    const byAddress = new Map(data.cards.map((card) => [card.address, card]));
    const outside = new Map((data.outside || []).map((row) => [row.address, row]));
    const direct = new Map();
    const sides = new Map();
    const refers = new Map();
    const uses = new Map();
    const add = (map, key, value) => {
      if (!map.has(key)) map.set(key, []);
      map.get(key).push(value);
    };
    for (const card of data.cards) {
      if (card.edge === 'side') add(sides, card.parent, card.address);
      else if (card.edge === 'direct') direct.set(card.parent, card.address);
      const terms = new Set(card.term_use_targets || []);
      for (const target of card.links || []) {
        if (target !== card.address && byAddress.has(target)) add(terms.has(target) ? uses : refers, target, card.address);
      }
    }
    const inboundCount = (address) => new Set([...(refers.get(address) || []), ...(uses.get(address) || [])]).size;
    const summaries = data.cards.map((card) => {
      const { text, html, bibtex, hubs, broken, models, model, ...summary } = card;
      // A Card can keep its own Workspace when it appears in another tree.
      return { ...summary, tree: data.tree.id, workspace: card.workspace || data.tree.workspace, inbound: inboundCount(card.address), lock: null, pinned_by: [] };
    });
    const search = data.cards.map((card) => {
      const text = visibleText(card.text);
      return { card, text, foldedTitle: (card.title || '').toLowerCase(), foldedText: text.toLowerCase() };
    });
    return { data, byAddress, outside, direct, sides, refers, uses, summaries, search, spreads: { current: null, spreads: [] } };
  }

  function reference(ix, address) {
    const card = ix.byAddress.get(address);
    const title = card ? card.title : ix.outside.get(address)?.title ?? '';
    return { address, title, workspace: card ? card.workspace || ix.data.tree.workspace : null, lock: null, pinned_by: [] };
  }

  function outbound(ix, card) {
    const terms = new Set(card.term_use_targets || []);
    const live = (card.links || []).map((address) => {
      const { title, workspace } = reference(ix, address);
      const row = { address, title, workspace, broken: false };
      if (terms.has(address)) {
        row.kind = 'term';
        row.label = ix.byAddress.get(address)?.term ?? row.title;
      }
      return row;
    });
    const broken = (card.broken || []).map((address) => ({ address, title: '', workspace: null, broken: true }));
    return [...live, ...broken];
  }

  function termFacts(ix, card) {
    if (card.type !== 'term') return null;
    const list = card.term_list && ix.byAddress.has(card.term_list)
      ? { address: card.term_list, title: ix.byAddress.get(card.term_list).title }
      : null;
    return { role: card.term_role || 'unfiled', list };
  }

  /** The model answer of one Card: each model is stored once, and only this order is the Card's. */
  function modelOf(ix, card, via) {
    if (card.model !== undefined) return card.model;
    const heads = card.models;
    if (!heads || !heads.length) return null;
    const chosen = heads.includes(via) ? via : heads[0];
    const stored = ix.data.models?.[chosen];
    if (!stored) return null;
    const order = [chosen, ...heads.filter((head) => head !== chosen)];
    return {
      ...stored,
      models: order.map((head) => ({ address: head, title: ix.byAddress.get(head)?.title ?? '' })),
      facet: card.facet ?? null,
      emphasis: card.emphasis ?? null,
    };
  }

  function cardAnswer(ix, address, query) {
    const card = ix.byAddress.get(address);
    if (!card) return { status: 400, body: { error: elsewhere(address) } };
    const refer = ix.refers.get(address) || [];
    const used = ix.uses.get(address) || [];
    const inboundAll = [...new Set([...refer, ...used])].sort();
    const all = query.get('inbound') === 'all';
    const head = (list) => (all ? list : list.slice(0, INBOUND_HEAD));
    const sides = ix.sides.get(address) || [];
    const answer = {
      address,
      tree: ix.data.tree.id,
      workspace: card.workspace || ix.data.tree.workspace,
      kind: card.kind,
      type: card.type,
      term: card.term ?? null,
      term_facts: termFacts(ix, card),
      facet: card.facet ?? null,
      emphasis: card.emphasis ?? null,
      level: card.level ?? null,
      form: card.form ?? null,
      model: modelOf(ix, card, query.get('via')),
      hubs: card.hubs || [],
      context: card.edge === 'root' ? 'tree-root' : 'regular',
      title: card.title,
      body: bodyOf(card.text),
      authored_fingerprint: '',
      restatement: Boolean(card.restatement),
      statement_fingerprint: '',
      html: card.html,
      bibtex: card.bibtex ?? null,
      citation_key: card.kind === 'literature' ? address : null,
      literature_type: card.literature_type ?? null,
      parent: ix.byAddress.has(card.parent) ? reference(ix, card.parent) : null,
      direct: ix.direct.has(address) ? reference(ix, ix.direct.get(address)) : null,
      sides: (query.get('sides') === 'all' ? sides : sides.slice(0, 1)).map((at) => reference(ix, at)),
      sides_count: sides.length,
      inbound: head(inboundAll).map((at) => reference(ix, at)),
      inbound_count: inboundAll.length,
      inbound_near: inboundAll.length <= 16 ? inboundAll : null,
      reverse_links: head(refer).map((at) => reference(ix, at)),
      reverse_count: refer.length,
      term_uses: head(used).map((at) => ({ ...reference(ix, at), label: card.term ?? card.title })),
      term_use_count: used.length,
      outbound: outbound(ix, card),
      lock: null,
      pinned_by: [],
      writable: true,
    };
    if (query.get('text')) answer.text = card.text;
    return { status: 200, body: answer };
  }

  function treeRow(ix) {
    const { tree, cards } = ix.data;
    const count = (type) => cards.filter((card) => card.type === type).length;
    return {
      ...tree,
      tree: tree.id,
      literature_type: cards[0].literature_type ?? null,
      count: cards.length,
      events: count('event'),
      findings: count('finding'),
      terms: count('term'),
      models: cards.filter((card) => card.type === 'model' && !card.facet).length,
      excerpt: cards[0].excerpt || '',
      locked: false,
      fingerprint: 'portable',
    };
  }

  function overview(ix) {
    const { tree, cards, exported, broken } = ix.data;
    const literature = tree.kind === 'literature';
    const workspace = (id) => ({
      id,
      label: id === 'human' ? 'Human' : 'Agent',
      topic_trees: id === tree.workspace && !literature ? 1 : 0,
      literature_trees: id === tree.workspace && literature ? 1 : 0,
      cards: id === tree.workspace ? cards.length : 0,
      agent_writable: false,
    });
    return {
      fingerprint: 'portable',
      revision: exported?.archive_revision ?? 0,
      archive_root: `portable export · ${tree.title}`,
      stats: { total: cards.length, topics: literature ? 0 : 1, regular: cards.length - 1, literature: literature ? 1 : 0 },
      broken: (broken || []).length,
      workspaces: [workspace('human'), workspace('agent')],
      trees: [treeRow(ix)],
      locks: 'portable',
      features: { search: true, ask: true, reply: true },
    };
  }

  function snippet(text, at) {
    const start = Math.max(0, at - SNIPPET);
    const line = text.slice(start, at + SNIPPET * 2).replace(/\s+/g, ' ').trim();
    return `${start > 0 ? '…' : ''}${line}${at + SNIPPET * 2 < text.length ? '…' : ''}`;
  }

  /**
   * The search tray's answer. Every word must appear: Cards whose title holds them all come first,
   * then the rest, each in reading order; the Workspace and type filters narrow the rows, and the
   * counts beside them keep showing what each filter would find. With no register to rank by, both
   * orders of the tray read this list.
   */
  function search(ix, query) {
    const words = (query.get('q') || '').toLowerCase().split(/\s+/).filter(Boolean);
    const mode = 'plain';
    const none = { mode, results: [], total: 0, workspaces: {}, types: {} };
    if (!words.length) return none;
    const wanted = query.get('type');
    const titles = [];
    const bodies = [];
    const types = {};
    const row = (card, field, excerpt) => ({
      address: card.address,
      title: card.title,
      kind: card.kind,
      type: card.type,
      facet: card.facet ?? null,
      emphasis: card.emphasis ?? null,
      level: card.level ?? null,
      form: card.form ?? null,
      workspace: card.workspace || ix.data.tree.workspace,
      tree: ix.data.tree.id,
      tree_title: ix.data.tree.title,
      field,
      score: 0,
      snippet: excerpt,
      lock: null,
      pinned_by: [],
    });
    for (const { card, text, foldedTitle, foldedText } of ix.search) {
      const inTitle = words.every((word) => foldedTitle.includes(word));
      const inBody = !inTitle && words.every((word) => foldedTitle.includes(word) || foldedText.includes(word));
      if (!inTitle && !inBody) continue;
      types[card.type] = (types[card.type] || 0) + 1;
      if (wanted && wanted !== 'all' && card.type !== wanted) continue;
      if (inTitle) titles.push(row(card, 'title', ''));
      else bodies.push(row(card, 'body', snippet(text, Math.max(0, foldedText.indexOf(words[0])))));
    }
    const all = [...titles, ...bodies];
    const seen = Object.values(types).reduce((sum, n) => sum + n, 0);
    return { mode, results: all.slice(0, SEARCH_LIMIT), total: all.length, workspaces: { [ix.data.tree.workspace]: seen }, types };
  }

  /**
   * The read-only Coordinate view of a v4 tree or v3 Connection tree. Each occurrence has a
   * package-local Coordinate ID, while all occurrences of a live Card share its Card data.
   */
  function connectionTree(ix) {
    const ct = ix.data.connection_tree;
    const stations = ix.data.coordinates ?? ct?.stations;
    if (!stations) return null;
    if (ix.connection) return ix.connection;
    const meta = ix.data.tree;
    const handle = ct?.handle ?? meta.id;
    const role = (address) => /^~/.test(address || '') ? 'connection' : /^\d+(?:\/|$)/.test(address || '') ? 'topic' : 'literature';
    const ordinals = new Map();
    const coordinates = stations.map((s) => {
      const id = s.coord_id ?? s.station ?? s.placement_id;
      const parent = s.parent ?? s.parent_coord_id ?? s.parent_id ?? null;
      let ordinal = null;
      if (s.edge === 'side') {
        const n = (ordinals.get(parent) || 0) + 1;
        ordinals.set(parent, n);
        ordinal = n;
      }
      const missing = s.state === 'missing';
      const slot = s.slot ?? ordinal;
      const home = missing ? null : ix.byAddress.get(s.address);
      const workspace = s.home_workspace === undefined ? (home?.workspace ?? meta.workspace) : s.home_workspace;
      return {
        station: id, coord_id: id, placement_id: id, coord: s.coord,
        card_id: s.card_id ?? (missing ? `missing:${id}` : s.address),
        edge: s.edge, parent, parent_coord_id: parent, parent_id: parent, slot, ordinal: slot,
        is_filing: s.is_filing ?? (s.edge === 'root' || s.origin === 'native'),
        gloss: s.gloss ?? null, host: handle, tree: handle, tree_id: handle, tree_role: meta.kind, host_workspace: meta.workspace,
        address: s.address, title: s.title, state: s.state, status: missing ? 'missing' : 'live',
        home_address: s.address, home_role: role(s.address),
        home_workspace: workspace, origin: s.origin || 'mount',
        can_edit_card: false, can_edit_coordinate: false, can_edit_placement: false, citation: `[[${s.address}]]`,
        bookmark: `place:${id}`, offline_state: missing ? (s.detail || 'Missing mount (deleted in archive)') : 'in-package',
      };
    });
    const byId = new Map(coordinates.map((p) => [p.coord_id, p]));
    ix.byCoordId = byId;
    ix.byCoordPath = new Map(coordinates.map((p) => [p.coord, p]));
    const occurrences = new Map();
    for (const p of coordinates) occurrences.set(p.card_id, (occurrences.get(p.card_id) || 0) + 1);
    for (const p of coordinates) {
      p.places = occurrences.get(p.card_id);
      p.repeat_count = p.places;
    }
    const end = (id) => {
      const p = byId.get(id) || {};
      return { coord_id: id, placement_id: id, coord: p.coord, card_id: p.card_id, title: p.title, status: p.status };
    };
    const connections = (ix.data.connections ?? ct?.connections ?? []).map((c, i) => {
      const from = end(c.from);
      const to = end(c.to);
      const predicate = c.relation_label || c.relation_kind || (c.direction === 'directed' ? 'connects to' : '');
      const sentence = c.direction === 'directed' ? `${from.title} ${predicate} ${to.title}` : (predicate ? `${from.title} — ${predicate} — ${to.title}` : `${from.title} — ${to.title}`);
      return { connection_id: `c${i + 1}`, direction: c.direction, from, to, relation_label: c.relation_label || null, relation_kind: c.relation_kind || null, note: c.note || null, provenance: c.provenance, sentence, state: c.state };
    });
    const rootId = coordinates.find((p) => p.edge === 'root')?.coord_id ?? null;
    const native = coordinates.filter((p) => p.is_filing && p.edge !== 'root').length;
    const mounted = coordinates.filter((p) => !p.is_filing && p.edge !== 'root').length;
    const untitled = meta.kind === 'connection' && meta.title === handle;
    const tree = {
      // An untitled root (its title is its own handle) reads as untitled offline too (D2).
      tree: handle, tree_id: handle, tree_role: meta.kind, title: untitled ? '' : meta.title, untitled,
      workspace: meta.workspace, purpose: ct?.purpose ?? null,
      root_coord_id: rootId, root_placement_id: rootId, coordinates, placements: coordinates,
      total_coordinates: coordinates.length, total_placements: coordinates.length,
      archive_revision: ix.data.exported?.archive_revision ?? 0,
      connections, mounted, native, missing: coordinates.filter((p) => p.status === 'missing').length,
      locked: false, can_edit_tree: false, counts: `${mounted} mounted · ${native} native · ${connections.length} connections`,
    };
    ix.connection = tree;
    return tree;
  }

  /** The read tools of the station view, answered from the package; writes are not offline. */
  function connectionAnswer(ix, request) {
    const tool = request && request.tool;
    const args = (request && request.args) || {};
    const ok = (value) => ({ status: 200, body: value });
    if (tool === 'list_side_mounts') {
      return ok({ at: args.at, mounts: [], retired: true });
    }
    const isConnection = Boolean(ix.data.connection_tree) || ix.data.tree.kind === 'connection';
    if (tool === 'list_connection_trees' && !isConnection) return ok({ trees: [] });
    const tree = connectionTree(ix);
    if (tool === 'list_coordinates' || tool === 'list_placements') {
      const at = String(args.card_id ?? (args.target_coord_id ? `place:${args.target_coord_id}` : args.at) ?? '');
      const bookmark = at.startsWith('place:');
      const resolvedCardId = bookmark
        ? ix.byCoordId?.get(at.slice(6))?.card_id
        : ix.byCoordPath?.get(at)?.card_id ?? at;
      const coordinates = (tree?.coordinates || []).filter((p) =>
        p.card_id === resolvedCardId || (!bookmark && p.status === 'live' && p.home_address === at));
      const [first] = coordinates;
      const home = first ? (first.status === 'live' ? first.home_address : null) : (ix.byAddress.has(at) ? at : null);
      if (tool === 'list_placements' && !first) return ok({ placements: [], home });
      return ok({
        card_id: first?.card_id, home, placements: coordinates, coordinates, places: coordinates.length,
        filing_coord_id: coordinates.find((p) => p.is_filing)?.coord_id ?? null,
        archive_revision: tree?.archive_revision ?? ix.data.exported?.archive_revision ?? 0,
      });
    }
    if (!tree || (tool === 'get_connection_tree' && !isConnection)) {
      return { status: 400, body: { error: tool === 'get_tree_coordinates' ? 'This export holds no Coordinate data.' : 'This export holds no Connection tree.' } };
    }
    switch (tool) {
      case 'list_connection_trees': {
        const { coordinates, placements, connections, ...summary } = tree;
        return ok({ trees: [{ ...summary, connections: connections.length }] });
      }
      case 'get_connection_tree':
      case 'get_tree_coordinates':
        return ok(tree);
      case 'list_connections': {
        const focus = args.coord_id ?? args.placement_id;
        const mine = tree.connections.filter((c) => !focus || c.from.coord_id === focus || c.to.coord_id === focus);
        return ok({ tree: tree.tree, connections: mine });
      }
      default:
        return { status: 409, body: { error: notice('This function'), portable: true } };
    }
  }

  const locksEmpty = () => ({
    locks: [],
    events: [],
    integrity: { faulted: false, faults: [], latched: [] },
  });

  /** The answer to one `/api/*` request: `{ status, body }`, exactly as the service sends it. */
  function respond(ix, method, url, body) {
    const parsed = new URL(url, 'http://portable.invalid');
    const route = parsed.pathname.replace(/^\/api\//, '');
    const query = parsed.searchParams;
    const ok = (value) => ({ status: 200, body: value });
    if (method === 'GET') {
      switch (route) {
        case 'ping': return ok({ ok: true });
        case 'overview': return ok(overview(ix));
        case 'graph': return ok({ nodes: ix.summaries });
        case 'tree': {
          const { tree } = ix.data;
          if (query.get('id') !== tree.id) return { status: 400, body: { error: elsewhere(query.get('id')) } };
          const answer = { tree: tree.id, root: tree.root, workspace: tree.workspace, kind: tree.kind, title: tree.title, cards: ix.summaries };
          if (ix.data.version === 4) {
            const view = connectionTree(ix);
            if (view) Object.assign(answer, { coordinates: view.coordinates, connections: view.connections, archive_revision: view.archive_revision });
          }
          return ok(answer);
        }
        case 'card': return cardAnswer(ix, query.get('at'), query);
        case 'search': return ok(search(ix, query));
        case 'spreads': return ok(ix.spreads);
        case 'locks': return ok(locksEmpty());
        case 'broken': return ok({ broken: ix.data.broken || [] });
        case 'distant': return ok({ at: query.get('at'), candidate: null, source: null, excluded: [] });
        case 'artifact': {
          const sha = query.get('sha');
          return ok({ sha256: sha, held: Boolean(ix.data.artifacts?.[sha]), source: null, in_document: null });
        }
        default:
      }
    }
    if (method === 'POST' && route === 'connection') return connectionAnswer(ix, body);
    if (method === 'POST' && route === 'spreads') {
      ix.spreads = body;
      return ok(body);
    }
    const what = REFUSED[route] ?? (route.startsWith('scholar/') ? 'The Scholar Archive' : 'This function');
    return { status: 409, body: { error: notice(what), portable: true } };
  }

  // What Card HTML may hold: the tags the renderer writes. A chart's page lives in its own frame,
  // so no script, style, form or foreign content has a place here.
  const TAGS = new Set(['a', 'b', 'blockquote', 'br', 'code', 'dd', 'del', 'div', 'dl', 'dt', 'em', 'figure', 'h1',
    'h2', 'h3', 'h4', 'h5', 'h6', 'hr', 'i', 'iframe', 'img', 'input', 'ins', 'kbd', 'li', 'mark', 'ol', 'p', 'pre',
    's', 'small', 'span', 'strong', 'sub', 'sup', 'table', 'tbody', 'td', 'tfoot', 'th', 'thead', 'tr', 'u', 'ul']);
  const ATTRIBUTES = new Set(['class', 'title', 'data-artifact', 'data-chart', 'data-target', 'data-reason']);

  const keepTag = (tag) => TAGS.has(tag.toLowerCase());

  /** Whether one attribute of Card HTML stays. Nothing here can carry a script or an address out. */
  function keepAttr(tag, name, value) {
    if (name === 'href') return tag === 'a' && (value.startsWith('#/card/') || value.startsWith('#/place/'));
    if (name === 'alt' || name === 'loading') return tag === 'img' || (name === 'loading' && tag === 'iframe');
    if (name === 'style') return /^text-align:\s*(left|right|center)$/.test(value);
    if (name === 'start') return tag === 'ol';
    if (name === 'type') return tag === 'input' && value === 'checkbox';
    if (name === 'disabled' || name === 'checked') return tag === 'input';
    return ATTRIBUTES.has(name);
  }

  /** What the GUI reads off a Term link: its target from the route, its meaning from the hint. */
  function linkFacts(href, title) {
    const prefix = '#/card/';
    if (!href || !href.startsWith(prefix)) return null;
    let target;
    try {
      target = decodeURIComponent(href.slice(prefix.length));
    } catch {
      return null;
    }
    const suffix = ` · ${target}`;
    return { target, meaning: title && title.endsWith(suffix) ? title.slice(0, -suffix.length) : '' };
  }

  /** The `data:` address of a stored figure, or `null` for anything that is not plainly an image. */
  function figureSource(artifact) {
    if (!artifact || !/^image\/(png|jpeg|gif|webp|svg\+xml)$/.test(artifact.type)) return null;
    if (!/^[A-Za-z0-9+/]+=*$/.test(artifact.base64 || '')) return null;
    return `data:${artifact.type};base64,${artifact.base64}`;
  }

  /**
   * A stored chart page as it opens: its theme and its scripts filled in, as `serve_chart_page`
   * does on the service. A chart page in the export names these only as placeholders, so seventy
   * charts carry no seventy copies of KaTeX and its fonts.
   */
  function chartDocument(page, theme, parts) {
    const fill = (text, mark, value) => {
      const at = text.indexOf(mark);
      return at < 0 ? text : text.slice(0, at) + value + text.slice(at + mark.length);
    };
    // This file is inlined into the page, so its text must not read as a tag that ends its script.
    const themed = fill(page, '__ZT_THEME__', theme);
    // A frame paints its own canvas in the dark theme, so it takes the card's paper.
    const paper = '<style>html { background: var(--zt-paper) !important; }</style>';
    const framed = fill(themed, '<\!--zt:frame-->', `<script>${parts.frame}<\/script>${paper}`);
    return fill(framed, '<\!--zt:katex-->', `<style>${parts.katexCss}</style>\n<script>${parts.katexJs}<\/script>`);
  }

  return {
    refusal, open, respond, notice, elsewhere, bodyOf, visibleText, keepTag, keepAttr, linkFacts, figureSource, chartDocument,
  };
});
