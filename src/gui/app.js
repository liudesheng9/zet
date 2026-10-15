/* zt gui — a slip-box desk for ZT Cards. */
(() => {
  'use strict';

  // The service's token; a restarted service brings a new one (see heartbeat).
  let TOKEN = document.querySelector('meta[name="zt-token"]').content;

  // ---------------------------------------------------------------- helpers
  const $ = (selector, root = document) => root.querySelector(selector);

  function h(tag, props, ...kids) {
    const node = document.createElement(tag);
    for (const [key, value] of Object.entries(props || {})) {
      if (value == null || value === false) continue;
      if (key === 'class') node.className = value;
      else if (key === 'text') node.textContent = value;
      else if (key === 'html') node.innerHTML = value;
      else if (key.startsWith('on')) node.addEventListener(key.slice(2), value);
      else if (key === 'dataset') Object.assign(node.dataset, value);
      else node.setAttribute(key, value === true ? '' : value);
    }
    for (const kid of kids.flat(Infinity)) {
      if (kid == null || kid === false) continue;
      node.append(kid instanceof Node ? kid : String(kid));
    }
    return node;
  }

  function svg(tag, attrs) {
    const node = document.createElementNS('http://www.w3.org/2000/svg', tag);
    for (const [key, value] of Object.entries(attrs || {})) node.setAttribute(key, value);
    return node;
  }

  async function api(path, body) {
    const options = { headers: { 'X-ZT-Token': TOKEN } };
    // A change to the Cards may move or delete Cards on a Spread; the server rewrites the
    // Spreads it holds, so it must hold the latest ones first.
    if (body !== undefined && path !== 'spreads' && path !== 'render') await spreads.flush();
    if (body !== undefined) {
      options.method = 'POST';
      options.headers['Content-Type'] = 'application/json';
      options.body = JSON.stringify(body);
    }
    let response;
    try {
      response = await fetch('/api/' + path, options);
    } catch {
      throw new Error('the zt service is not responding');
    }
    let data = {};
    try { data = await response.json(); } catch { /* empty body */ }
    if (!response.ok) throw new Error(data.error || response.statusText);
    return data;
  }

  const enc = encodeURIComponent;
  const debounce = (fn, ms) => {
    let timer;
    return (...args) => { clearTimeout(timer); timer = setTimeout(() => fn(...args), ms); };
  };
  const store = {
    get(key) { try { return localStorage.getItem(key); } catch { return null; } },
    set(key, value) { try { localStorage.setItem(key, value); } catch { /* private mode */ } },
  };
  const isTyping = (target) =>
    target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable);

  function toast(message, kind) {
    const node = h('div', { class: 'toast' + (kind === 'err' ? ' err' : ''), text: message });
    $('#toasts').append(node);
    setTimeout(() => node.remove(), kind === 'err' ? 6000 : 3200);
  }
  const fail = (error) => toast(error.message || String(error), 'err');

  /** Mirrors `parent_location` in the backend. */
  function parentOf(address) {
    const slash = address.indexOf('/');
    if (slash < 0) return null;
    const tree = address.slice(0, slash);
    const segments = address.slice(slash + 1).split('|');
    if (segments.length === 1 && segments[0] === '0') return null;
    const last = segments[segments.length - 1];
    if (/^[a-z]+$/.test(last)) return tree + '/' + segments.slice(0, -1).join('|');
    const number = parseInt(last, 10);
    if (number > 1) {
      segments[segments.length - 1] = String(number - 1);
      return tree + '/' + segments.join('|');
    }
    if (segments.length > 1) return tree + '/' + segments.slice(0, -1).join('|');
    return /^\d+$/.test(tree) ? tree + '/0' : tree;
  }
  const cssVar = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

  // ------------------------------------------------------------------ state
  const state = {
    overview: null,
    nodes: [],            // every Card summary, for link completion and the sky
    byAddress: new Map(), // address -> Card summary
    inboundOf: new Map(), // address -> addresses of the Cards that link it
    childrenOf: new Map(), // address -> summaries of its Direct and Side successors
    fingerprint: '',
    pointer: null,        // Card address, or null for ROOT
    card: null,
    tree: null,
    outline: null,        // the current tree as an outline (layout.js)
    expandAll: store.get('zt-expand') === '1',
    mode: ['constellation', 'table'].includes(store.get('zt-mode')) ? store.get('zt-mode') : 'map',
    // The Workspace shown at ROOT and given new trees; it follows the Pointer.
    workspace: store.get('zt-workspace') === 'agent' ? 'agent' : 'human',
    editing: false,
    landing: false,       // a card carried from the search tray is landing in the reader
  };

  const WORKSPACES = {
    human: { label: 'Human', short: 'human', mcp: 'read-only for AI agents' },
    agent: { label: 'AI agent', short: 'agent', mcp: 'written by AI agents over MCP' },
  };
  const wsLabel = (id) => (WORKSPACES[id] || WORKSPACES.human).label;
  const nodeWorkspace = (address) => state.byAddress.get(address)?.workspace;

  /** Take a fresh list of Card summaries and index it. */
  function setNodes(nodes) {
    state.nodes = nodes;
    state.byAddress = new Map(nodes.map((node) => [node.address, node]));
    state.inboundOf = new Map();
    state.childrenOf = new Map();
    for (const node of nodes) {
      for (const target of node.links || []) {
        if (!state.inboundOf.has(target)) state.inboundOf.set(target, []);
        state.inboundOf.get(target).push(node.address);
      }
      if (node.parent && node.edge !== 'root') {
        if (!state.childrenOf.has(node.parent)) state.childrenOf.set(node.parent, []);
        state.childrenOf.get(node.parent).push(node);
      }
    }
  }

  // Card types: most Cards hold an idea; a Regular Card may record an event or present a model.
  const CARD_TYPES = {
    idea: { label: 'Idea', note: 'one idea or claim' },
    event: { label: 'Event', note: 'one real-world event: what, when, where, who, and the source' },
    model: { label: 'Model', note: 'one whole model: its question and what it shows, or one part of it' },
    hub: { label: 'Hub', note: 'a way into one subject: its Cards in named groups, each with what it adds' },
  };
  // A Hub Card is drawn as a guide card; its tab stands further left the wider its subject is.
  const HUB_LEVELS = {
    register: { label: 'register', note: 'the Workspace: key terms, each with one to three ways in' },
    field: { label: 'field', note: 'several Topic trees that share one subject' },
    topic: { label: 'topic', note: 'one Topic tree' },
    line: { label: 'line', note: 'one line of thought inside one tree' },
  };
  const hubClass = (card) => (card.type === 'hub' && HUB_LEVELS[card.level] ? ' hub-' + card.level : '');
  // A model is presented by its Model Card and by Model parts: filed under it or linked from it. A
  // part's facet is a short label its author chose; these common facets are only suggestions (mirrors
  // `COMMON_FACETS` in src/model.rs). An emphasis marks a part `core` or `aside`.
  const FACETS = {
    setting: { label: 'Setting', note: 'agents, assets, actions, and payoffs' },
    assumptions: { label: 'Assumptions', note: 'what the model assumes, and what each assumption buys' },
    timing: { label: 'Timing', note: 'the order of moves and who knows what when' },
    dynamics: { label: 'Dynamics', note: 'how the state evolves: laws of motion, best responses, feedback' },
    solution: { label: 'Solution', note: 'the equilibrium or optimum and how it is derived' },
    results: { label: 'Results', note: 'propositions and comparative statics' },
    intuition: { label: 'Intuition', note: 'why the results hold, in words' },
    limits: { label: 'Limits', note: 'where the model stops applying, and its critics' },
  };
  const EMPHASIS = {
    core: { label: 'Core', note: 'the heart of the model: read it first' },
    aside: { label: 'Aside', note: 'detail a reader may skip' },
  };
  // A Model Card carries its model's elaboration level (mirrors `Level` in src/model.rs): a concept
  // model is stated in words with no Derivation parts; a formal model is built self-sufficient.
  const LEVELS = {
    concept: { label: 'L1 concept', note: 'level 1, a concept model: the question, mechanism, and result in words; no Derivation parts' },
    formal: { label: 'L2 formal', note: 'level 2, a formal model: self-sufficient, built in order with Derivation parts for skipped steps' },
  };
  /** How a facet reads: a common facet by its English name, any other label as written. */
  const facetLabel = (facet) => FACETS[facet]?.label || facet || '';
  /** The rubber stamp an Event or Model Card carries; Idea Cards carry none. A Model part's
   *  stamp names its facet; a core part's stamp carries a dot; a Model Card's names its level. */
  const typeStamp = (type, title, facet, emphasis, level) => {
    if (!CARD_TYPES[type] || type === 'idea') return null;
    const part = type === 'model' && facet;
    const lvl = type === 'model' ? !part && LEVELS[level] : type === 'hub' && HUB_LEVELS[level];
    const note = part && [FACETS[facet]?.note, EMPHASIS[emphasis] && `${EMPHASIS[emphasis].label}: ${EMPHASIS[emphasis].note}`].filter(Boolean).join(' · ');
    return h('span', {
      class: 'type-stamp ' + type + (part ? ' part' : '') + (part && emphasis ? ' ' + emphasis : '') + (lvl ? ' level-' + level : ''),
      title: title || (part ? `Model part · ${facetLabel(facet)}${note ? ' — ' + note : ''}` : `${CARD_TYPES[type].label} Card — ${CARD_TYPES[type].note}${lvl ? ` · ${lvl.note}` : ''}`),
      text: part ? `${type} · ${facet}` : lvl ? `${type} · ${lvl.label}` : type,
    });
  };

  // ------------------------------------------------------------------ math
  // TeX renders with the bundled KaTeX: `$…$` and `$$…$$` in bodies arrive as math spans
  // from the server; titles are plain text, so their `$…$` runs are typeset here.
  const katexReady = () => typeof window.katex !== 'undefined';
  function typeset(root) {
    if (!katexReady()) return;
    for (const node of root.querySelectorAll('span.math-inline, span.math-display')) {
      if (node.dataset.typeset) continue;
      const tex = node.textContent;
      try {
        window.katex.render(tex, node, { displayMode: node.classList.contains('math-display'), throwOnError: false, output: 'htmlAndMathml' });
        node.dataset.typeset = '1';
        node.title = tex;
      } catch { /* leave the TeX as text */ }
    }
  }
  const titleCache = new Map();
  /** A title as nodes, with its `$…$` runs typeset. */
  function titleNodes(text) {
    if (!text || !text.includes('$') || !katexReady()) return [text];
    let html = titleCache.get(text);
    if (html === undefined) {
      const escape = (value) => value.replace(/[&<>"]/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[ch]));
      html = text.split(/(\$[^$]+\$)/).map((part) => {
        if (part.length > 2 && part.startsWith('$') && part.endsWith('$')) {
          try { return window.katex.renderToString(part.slice(1, -1), { throwOnError: false }); } catch { return escape(part); }
        }
        return escape(part);
      }).join('');
      if (titleCache.size > 4000) titleCache.clear();
      titleCache.set(text, html);
    }
    const holder = document.createElement('span');
    holder.innerHTML = html;
    return [...holder.childNodes];
  }

  // ---------------------------------------------------------------- charts
  // A Card's HTML chart runs in a sandboxed frame served by `/chart/<id>`; it can only post
  // its height and the Card a reader clicked.
  const theme = () => (document.documentElement.dataset.theme === 'dark'
    || (!document.documentElement.dataset.theme && matchMedia('(prefers-color-scheme: dark)').matches) ? 'dark' : 'light');
  function wireCharts(root) {
    for (const frame of root.querySelectorAll('iframe.zt-chart-frame[data-chart]')) {
      frame.src = `/chart/${frame.dataset.chart}?theme=${theme()}`;
    }
  }
  window.addEventListener('message', (event) => {
    const data = event.data;
    if (!data || typeof data !== 'object') return;
    const frame = [...document.querySelectorAll('iframe.zt-chart-frame')].find((item) => item.contentWindow === event.source);
    if (!frame) return;
    if (data.zt === 'height' && Number.isFinite(data.height)) {
      frame.style.height = Math.max(40, Math.min(4000, data.height)) + 'px';
    } else if (data.zt === 'go' && typeof data.target === 'string') {
      if (state.nodes.some((node) => node.address === data.target)) go(data.target);
      else toast(`card \`${data.target}\` does not exist`, 'err');
    }
  });

  function setWorkspace(id, { toRoot } = {}) {
    if (!WORKSPACES[id]) throw new Error(`unknown workspace \`${id}\`; expected human or agent`);
    state.workspace = id;
    store.set('zt-workspace', id);
    if (toRoot) go(null);
  }

  // --------------------------------------------------------------- commands
  // Same vocabulary and availability as the terminal Session.
  const COMMANDS = [
    ['go [<target>]', 'go to a Location or Citation key; bare go returns to ROOT', 'always'],
    ['root', 'return to ROOT', 'always'],
    ['ws [<workspace>]', 'list workspaces, or switch to the human or agent workspace ROOT', 'always'],
    ['up', 'go to the parent card; a tree root goes to ROOT', 'card'],
    ['back', 'go back to the card you were at before', 'always'],
    ['fwd', 'go forward again to the card you went back from', 'always'],
    ['ls', 'list Topics and Literature at ROOT, or the current tree', 'always'],
    ['t <title>', 'create a Topic in the current workspace', 'always'],
    ['l', 'create a Literature Card in the current workspace from one BibTeX entry', 'always'],
    ['n', 'create the direct successor', 'card'],
    ['b', 'create the next side successor', 'regular'],
    ['e', 'edit the current card', 'card'],
    ['del', 'delete the current card and its successors', 'card'],
    ['ins', "insert a new card in the current card's place; it and its successors move down", 'regular'],
    ['mv [alone] <new-location>', 'move the current card and its successors (alone: only the card; its chain closes up)', 'regular'],
    ['mv [alone] before <location>', "move the current card into another card's place; that card moves down", 'regular'],
    ['ty <idea|event|model|hub> [<facet>]', 'make the current card an Idea, Event, Model, or Hub Card; a facet (any short label, e.g. setting, dynamics, 均衡) makes it a Model part', 'regular'],
    ['lv <level|none>', 'set the level of the current Model Card (concept or formal) or Hub Card (line, topic, field, or register)', 'regular'],
    ['stats', 'count cards by kind', 'always'],
    ['status', 'show service status', 'always'],
    ['lsbk', 'list broken links', 'always'],
    ['help', 'show the commands available here', 'always'],
    ['q', 'close this window (the GUI keeps running with the service)', 'always'],
  ];
  const context = () => (!state.card ? 'root' : state.card.context === 'tree-root' ? 'tree-root' : 'regular');
  const available = (scope) =>
    scope === 'always' || (scope === 'card' && context() !== 'root') || (scope === 'regular' && context() === 'regular');

  async function runCommand(line) {
    const parts = line.trim().split(/\s+/).filter(Boolean);
    const [name, ...rest] = parts;
    if (!name) return;
    const needCard = () => {
      if (!state.card) throw new Error(`${name} needs a Card; use go <target> first`);
    };
    switch (name) {
      case 'q': return quit();
      case 'root': return go(null);
      case 'back':
        if (rest.length) throw new Error('usage: back');
        return nav.back();
      case 'fwd':
        if (rest.length) throw new Error('usage: fwd');
        return nav.forward();
      case 'ws':
        if (rest.length > 1) throw new Error('usage: ws [<workspace>]');
        if (!rest.length) return showWorkspaces();
        return setWorkspace(rest[0] === 'ai' ? 'agent' : rest[0].toLowerCase(), { toRoot: true });
      case 'go':
        if (rest.length > 1) throw new Error('usage: go [<target>]');
        if (!rest.length) return go(null);
        if (!state.nodes.some((node) => node.address === rest[0])) {
          await refresh();
          if (!state.nodes.some((node) => node.address === rest[0])) {
            throw new Error(`target \`${rest[0]}\` does not exist`);
          }
        }
        return go(rest[0]);
      case 'up':
        if (rest.length) throw new Error('usage: up');
        return state.pointer ? go(parentOf(state.pointer)) : undefined;
      case 'ls':
        if (!state.pointer) { openRail(true); return; }
        setMode('map'); return fitView();
      case 't': return newCard('topic', rest.join(' '));
      case 'l': return newLiterature();
      case 'n': needCard(); return newCard('direct');
      case 'b':
        needCard();
        if (context() !== 'regular') throw new Error(`side successors cannot start from a ${state.card.kind === 'literature' ? 'Literature' : 'Topic'} Card`);
        return newCard('side');
      case 'e': needCard(); return editCard();
      case 'del': needCard(); return deleteCard();
      case 'ins': needCard(); return insertCard();
      case 'mv': {
        needCard();
        const alone = rest[0] === 'alone';
        const args = alone ? rest.slice(1) : rest;
        if (args.length === 2 && args[0] === 'before') return moveCard({ before: args[1] }, alone);
        if (args.length === 1 && args[0] !== 'before') return moveCard({ to: args[0] }, alone);
        throw new Error('usage: mv [alone] <new-location> | mv [alone] before <location>');
      }
      case 'ty':
        needCard();
        if (rest.length < 1) throw new Error('usage: ty <idea|event|model|hub> [<facet>]');
        // A facet is the rest of the line, so it may hold spaces.
        return setCardType(rest[0].toLowerCase(), rest.slice(1).join(' '));
      case 'lv': {
        needCard();
        if (rest.length !== 1) throw new Error('usage: lv <level|none>');
        const none = rest[0] === 'none';
        const result = await api('edit', { at: state.card.address, level: none ? '' : rest[0] });
        return afterChange(result.address, none ? `${result.address} has no level` : `${result.address} is a ${rest[0]} ${state.card.type === 'hub' ? 'hub' : 'model'}`);
      }
      case 'stats': {
        const s = state.overview.stats;
        return toast(`total: ${s.total}\ntopics: ${s.topics}\nregular: ${s.regular}\nliterature: ${s.literature}`);
      }
      case 'status':
        return toast(`state: up\narchive_root: ${state.overview.archive_root}\ncards: ${state.overview.stats.total}`);
      case 'lsbk': return showBroken();
      case 'help': return showHelp();
      default:
        throw new Error(`unknown session command: ${name}`);
    }
  }

  // --------------------------------------------------------------- routing
  function go(address) {
    const hash = address ? '#/card/' + enc(address) : '#/';
    if (location.hash === hash) route();
    else location.hash = hash;
  }

  function route() {
    const match = location.hash.match(/^#\/card\/(.+)$/);
    const pointer = match ? decodeURIComponent(match[1]) : null;
    const moved = pointer !== state.pointer;
    state.pointer = pointer;
    nav.arrive(pointer);
    if (moved) finder.close();
    loadPointer().catch(fail);
  }

  async function loadPointer() {
    if (!state.pointer) {
      state.card = null;
      renderAll();
      return;
    }
    try {
      state.card = await api('card?at=' + enc(state.pointer));
    } catch (error) {
      // Another client removed this Card: fall back to its nearest ancestor.
      const known = new Set(state.nodes.map((node) => node.address));
      let ancestor = parentOf(state.pointer);
      while (ancestor && !known.has(ancestor)) ancestor = parentOf(ancestor);
      toast(`card \`${state.pointer}\` no longer exists`, 'err');
      if (!nav.skipMissing()) go(ancestor);
      return;
    }
    if (state.card.workspace && state.card.workspace !== state.workspace) setWorkspace(state.card.workspace);
    const treeId = state.card.tree;
    if (!state.tree || state.tree.tree !== treeId) {
      state.tree = await api('tree?id=' + enc(treeId));
      state.outline = ZtLayout.outline(state.tree.cards, state.tree.root);
    }
    renderAll();
  }

  /** Reload everything, e.g. after a change here or from the CLI. */
  async function refresh() {
    // The server may have moved Cards on a Spread: read the Spreads back, after saving ours.
    await spreads.flush();
    const [overview, graph, kept] = await Promise.all([api('overview'), api('graph'), api('spreads')]);
    spreads.take(kept);
    state.overview = overview;
    state.fingerprint = overview.fingerprint;
    setNodes(graph.nodes);
    state.tree = null;
    sky.invalidate();
    renderRail();
    renderTopbar();
    await loadPointer();
  }

  // --------------------------------------------------------------- history
  /**
   * Back and forward through the places the Pointer has been, as a file browser does
   * (mirrors the session's `back` / `fwd`). Every move of the Pointer is one browser history
   * entry, so the mouse's back button and Alt+← step through the same list. Each entry
   * carries its index; this tab remembers the address at each index, so the buttons can
   * name their step and the search tray can show the cards visited last.
   */
  const nav = (() => {
    let index = 0;
    let trail = [];
    let booted = false;
    let step = 0; // -1 or +1 while the Pointer arrives by back or forward, else 0
    try {
      const saved = JSON.parse(sessionStorage.getItem('zt-nav') || 'null');
      if (saved && Array.isArray(saved.trail)) trail = saved.trail;
    } catch { /* no session storage */ }
    const save = () => {
      try { sessionStorage.setItem('zt-nav', JSON.stringify({ trail })); } catch { /* private mode */ }
    };
    const label = (address) => {
      if (!address) return 'ROOT';
      const title = state.byAddress.get(address)?.title;
      return title ? `${address} · ${title}` : address;
    };
    const canBack = () => index > 0;
    const canFwd = () => index < trail.length - 1;

    /** Called by route() for every Pointer change. */
    function arrive(pointer) {
      const entry = Number.isInteger(history.state?.ztNav) ? history.state.ztNav : null;
      if (entry !== null) {
        step = booted ? Math.sign(entry - index) : 0;
        index = entry;
      } else if (!booted) {
        index = 0;
        trail = [];
        step = 0;
      } else {
        // A new move forgets the places back came from.
        index += 1;
        trail.length = index;
        step = 0;
      }
      booted = true;
      trail[index] = pointer;
      if (entry === null) history.replaceState({ ...(history.state || {}), ztNav: index }, '');
      save();
      render();
    }

    function render() {
      const back = $('#nav-back');
      const fwd = $('#nav-fwd');
      back.disabled = !canBack();
      fwd.disabled = !canFwd();
      back.title = canBack() ? `Back to ${label(trail[index - 1])} (Alt+←) · right-click for more` : 'Back (Alt+←)';
      fwd.title = canFwd() ? `Forward to ${label(trail[index + 1])} (Alt+→) · right-click for more` : 'Forward (Alt+→)';
    }

    const back = () => { if (canBack()) history.back(); };
    const forward = () => { if (canFwd()) history.forward(); };
    const jump = (to) => { if (to !== index && to >= 0 && to < trail.length) history.go(to - index); };

    /** A card that was deleted since: keep stepping the same way, if there is further to go. */
    function skipMissing() {
      if (step < 0 && canBack()) { history.back(); return true; }
      if (step > 0 && canFwd()) { history.forward(); return true; }
      return false;
    }

    /** The cards visited before the current one, latest first, each once. */
    function recent(limit) {
      const seen = new Set([trail[index]]);
      const list = [];
      for (let at = index - 1; at >= 0 && list.length < limit; at -= 1) {
        const address = trail[at];
        if (!address || seen.has(address) || !state.byAddress.has(address)) continue;
        seen.add(address);
        list.push(address);
      }
      return list;
    }

    /** Right-click (or hold) a button: the places in its direction, as a browser lists them. */
    let menu = null;
    function closeMenu() {
      if (menu) menu.remove();
      menu = null;
    }
    function showMenu(button, direction) {
      closeMenu();
      const rows = [];
      for (let at = index + direction, n = 0; at >= 0 && at < trail.length && n < 12; at += direction, n += 1) {
        const address = trail[at];
        const summary = address && state.byAddress.get(address);
        rows.push(h('button', { class: 'nav-row', role: 'menuitem', onclick: () => { closeMenu(); jump(at); } },
          h('span', { class: 'addr', text: address || 'ROOT' }),
          h('span', { class: 't' }, address ? titleNodes(summary?.title || '(deleted)') : 'all trees')));
      }
      if (!rows.length) return;
      const box = button.getBoundingClientRect();
      menu = h('div', { class: 'nav-menu', role: 'menu', style: `left:${Math.round(box.left)}px;top:${Math.round(box.bottom + 6)}px` },
        h('div', { class: 'nav-menu-head', text: direction < 0 ? 'Back to' : 'Forward to' }), rows);
      document.body.append(menu);
    }

    function install() {
      const wire = (button, direction, act) => {
        let held = null;
        button.addEventListener('click', act);
        button.addEventListener('contextmenu', (event) => { event.preventDefault(); showMenu(button, direction); });
        button.addEventListener('pointerdown', () => { held = setTimeout(() => showMenu(button, direction), 500); });
        for (const name of ['pointerup', 'pointerleave']) button.addEventListener(name, () => clearTimeout(held));
      };
      wire($('#nav-back'), -1, back);
      wire($('#nav-fwd'), 1, forward);
      document.addEventListener('pointerdown', (event) => { if (menu && !menu.contains(event.target)) closeMenu(); }, true);
      document.addEventListener('keydown', (event) => { if (event.key === 'Escape') closeMenu(); });
      window.addEventListener('blur', closeMenu);
    }

    return { arrive, back, forward, skipMissing, recent, install, get step() { return step; } };
  })();

  // ---------------------------------------------------------------- search
  /**
   * The search tray: a card-catalogue drawer pulled out over the desk. The hits stand in it
   * as slips behind one guide card per tree, the trees in the order of their best hit, a few
   * slips each with the rest folded. The slip you reach for lifts, and its whole card lies
   * beside the drawer; opening it carries that card to the reader. With no words in the
   * field, the drawer holds the cards you visited last.
   */
  const finder = (() => {
    const PER_TREE = 3;
    const TREES = 6;
    const filters = { ws: 'all', type: 'all' };
    const previews = new Map();
    let built = false;
    let isOpen = false;
    let data = null;
    let unfolded = new Set();
    let items = [];
    let selected = 0;
    let ticket = 0;
    let closing = null;
    let root, input, count, chips, hits, preview;

    const terms = () => input.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
    const rootOf = (tree) => (/^\d+$/.test(tree) ? tree + '/0' : tree);

    /** Text with every occurrence of the query's words marked. */
    function marked(text, words) {
      if (!text) return [];
      const lower = text.toLowerCase();
      const ranges = [];
      for (const word of words) {
        for (let at = lower.indexOf(word); at >= 0; at = lower.indexOf(word, at + word.length)) ranges.push([at, at + word.length]);
      }
      if (!ranges.length) return [text];
      ranges.sort((a, b) => a[0] - b[0]);
      const nodes = [];
      let from = 0;
      for (const [start, end] of ranges) {
        if (end <= from) continue;
        const begin = Math.max(start, from);
        if (begin > from) nodes.push(text.slice(from, begin));
        nodes.push(h('mark', { text: text.slice(begin, end) }));
        from = end;
      }
      nodes.push(text.slice(from));
      return nodes;
    }

    function build() {
      root = $('#finder');
      input = h('input', { type: 'search', placeholder: 'Search every card: titles, addresses, and text', autocomplete: 'off', spellcheck: 'false', 'aria-label': 'Search cards' });
      count = h('span', { class: 'finder-count' });
      chips = h('div', { class: 'finder-filters' });
      hits = h('div', { class: 'finder-hits', role: 'listbox', 'aria-label': 'Matching cards' });
      preview = h('div', { class: 'finder-preview', 'aria-live': 'polite' });
      root.append(
        h('div', { class: 'finder-scrim', onclick: close }),
        h('section', { class: 'finder-tray', role: 'dialog', 'aria-label': 'Search cards' },
          h('header', { class: 'finder-head' },
            h('span', { class: 'finder-lens', html: '<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/></svg>' }),
            input, count,
            h('button', { class: 'finder-close', title: 'Close (Esc)', onclick: close }, h('kbd', { text: 'Esc' }))),
          chips,
          h('div', { class: 'finder-body' }, hits, preview),
          h('footer', { class: 'finder-keys' },
            h('span', {}, h('kbd', { text: '↑' }), h('kbd', { text: '↓' }), ' choose'),
            h('span', {}, h('kbd', { text: '↵' }), ' open'),
            h('span', {}, h('kbd', { text: '⇧' }), h('kbd', { text: '↵' }), ' to the Table'),
            h('span', {}, h('kbd', { text: 'Esc' }), ' clear · close'))));
      input.addEventListener('input', () => { unfolded = new Set(); search(); });
      input.addEventListener('keydown', onKey);
      root.addEventListener('keydown', (event) => {
        if (event.key === 'Escape' && event.target !== input) { event.preventDefault(); close(); }
      });
      built = true;
    }

    function open(text) {
      if (!built) build();
      clearTimeout(closing);
      previews.clear();
      if (typeof text === 'string') input.value = text;
      isOpen = true;
      root.hidden = false;
      root.classList.remove('closing');
      openRail(false);
      unfolded = new Set();
      search.now();
      input.focus();
      input.select();
    }

    function close() {
      if (!isOpen) return;
      isOpen = false;
      root.classList.add('closing');
      closing = setTimeout(() => { root.hidden = true; root.classList.remove('closing'); }, reduceMotion() ? 0 : 170);
      if (document.activeElement && root.contains(document.activeElement)) document.activeElement.blur();
    }

    const search = (() => {
      const run = async () => {
        const mine = ++ticket;
        const words = input.value.trim();
        if (!words) { data = null; render(); return; }
        try {
          const answer = await api(`search?q=${enc(words)}&ws=${filters.ws}&type=${filters.type}`);
          if (mine !== ticket || !isOpen) return;
          data = answer;
          selected = 0;
          render();
        } catch (error) { fail(error); }
      };
      const later = debounce(run, 110);
      later.now = run;
      return later;
    })();

    function chip(group, value, label, n) {
      const on = filters[group] === value;
      return h('button', {
        class: 'fchip' + (on ? ' on' : '') + (value !== 'all' ? ' ' + value : ''),
        disabled: !on && n === 0,
        'aria-pressed': on ? 'true' : 'false',
        onclick: () => { filters[group] = value; unfolded = new Set(); search.now(); input.focus(); },
      }, h('span', { text: label }), n == null ? null : h('b', { text: String(n) }));
    }

    function renderChips() {
      if (!data) { chips.hidden = true; return; }
      chips.hidden = false;
      const ws = data.workspaces || {};
      const types = data.types || {};
      const sum = (counts) => Object.values(counts).reduce((total, n) => total + n, 0);
      chips.replaceChildren(
        h('div', { class: 'fgroup', role: 'group', 'aria-label': 'Workspace' },
          chip('ws', 'all', 'All', sum(ws)),
          chip('ws', 'human', wsLabel('human'), ws.human || 0),
          chip('ws', 'agent', wsLabel('agent'), ws.agent || 0)),
        h('div', { class: 'fgroup', role: 'group', 'aria-label': 'Card type' },
          chip('type', 'all', 'Any type', sum(types)),
          ...Object.keys(CARD_TYPES).map((type) => chip('type', type, CARD_TYPES[type].label, types[type] || 0))),
      );
    }

    /** A slip: the top of one index card, standing in the drawer. */
    function slip(hit, words) {
      const onTable = spreads.has(hit.address);
      const title = hit.title || '(untitled)';
      return h('div', {
        class: 'slip' + (hit.workspace === 'agent' ? ' agent-ws' : '') + (hit.type && hit.type !== 'idea' ? ' type-' + hit.type : ''),
        role: 'option', dataset: { address: hit.address },
        onclick: () => choose(hit.address),
        onmousemove: (event) => pointAt(event.currentTarget),
      },
        h('span', { class: 'slip-head' },
          h('span', { class: 'slip-title' }, title.includes('$') ? titleNodes(title) : marked(title, words)),
          typeStamp(hit.type, null, hit.facet, hit.emphasis, hit.level),
          h('span', { class: 'addr', text: hit.address })),
        hit.snippet ? h('span', { class: 'slip-text' }, marked(hit.snippet, words)) : null,
        h('button', {
          class: 'slip-pull' + (onTable ? ' on' : ''), tabindex: '-1',
          title: onTable ? 'On the Table: click to put it back (⇧↵)' : 'Lay it on the Table (⇧↵)',
          onclick: (event) => { event.stopPropagation(); pullHit(hit.address); },
        }, onTable ? '✓' : '+'),
      );
    }

    /** A guide card: the tree the slips behind it belong to. */
    function guide(tree, title, n, kind) {
      return h('header', { class: 'guide-card ' + kind },
        h('span', { class: 'addr', text: rootOf(tree) }),
        h('span', { class: 't' }, titleNodes(title || '(untitled)')),
        n == null ? null : h('span', { class: 'n', text: String(n) }));
    }

    function render() {
      renderChips();
      const words = terms();
      const nodes = [];
      if (!data) {
        count.textContent = '';
        const recent = nav.recent(8).map((address) => {
          const node = state.byAddress.get(address);
          return { ...node, snippet: node.excerpt };
        });
        if (recent.length) {
          nodes.push(h('section', { class: 'drawer-run recent' },
            h('header', { class: 'guide-card recent' }, h('span', { class: 't', text: 'Visited last' }), h('span', { class: 'n', text: 'Alt+← steps back' })),
            recent.map((hit) => slip(hit, []))));
        } else {
          nodes.push(h('div', { class: 'finder-empty' },
            h('p', { class: 'big', text: 'Look for a card by any words in it.' }),
            h('p', { text: 'Every word must appear in the title, the address, or the text. Titles count most.' })));
        }
      } else if (!data.results.length) {
        count.textContent = 'no cards';
        nodes.push(h('div', { class: 'finder-empty' },
          h('p', { class: 'big' }, 'No card holds ', h('q', { text: input.value.trim() }), '.'),
          h('p', { text: words.length > 1 ? 'Every word must appear; try fewer words.' : (filters.ws !== 'all' || filters.type !== 'all' ? 'A filter is on; try All.' : 'Try another spelling or a shorter word.') })));
      } else {
        const shown = data.results.length;
        count.textContent = data.total > shown ? `best ${shown} of ${data.total} cards` : `${data.total} card${data.total === 1 ? '' : 's'}`;
        const groups = new Map();
        for (const hit of data.results) {
          if (!groups.has(hit.tree)) groups.set(hit.tree, []);
          groups.get(hit.tree).push(hit);
        }
        const trees = [...groups.entries()];
        const allTrees = unfolded.has('*');
        trees.slice(0, allTrees ? trees.length : TREES).forEach(([tree, list]) => {
          const open = unfolded.has(tree);
          const kept = open ? list : list.slice(0, PER_TREE);
          nodes.push(h('section', { class: 'drawer-run' },
            guide(tree, list[0].tree_title, list.length, /^\d+$/.test(tree) ? 'topic' : 'literature'),
            kept.map((hit) => slip(hit, words)),
            list.length > kept.length ? h('button', {
              class: 'fold', tabindex: '-1', dataset: { fold: tree },
              onclick: () => unfold(tree), onmousemove: (event) => pointAt(event.currentTarget),
            }, `+${list.length - kept.length} more in this tree`) : null));
        });
        if (!allTrees && trees.length > TREES) {
          const rest = trees.slice(TREES);
          nodes.push(h('button', {
            class: 'fold trees', tabindex: '-1', dataset: { fold: '*' },
            onclick: () => unfold('*'), onmousemove: (event) => pointAt(event.currentTarget),
          }, `+${rest.length} more trees · ${rest.reduce((total, [, list]) => total + list.length, 0)} cards`));
        }
        if (data.total > shown) nodes.push(h('p', { class: 'finder-more', text: `${data.total - shown} weaker matches are not shown; add a word to narrow the search.` }));
      }
      hits.replaceChildren(...nodes);
      items = [...hits.querySelectorAll('.slip, .fold')];
      select(Math.min(selected, items.length - 1), { scroll: false });
      if (!items.length) preview.replaceChildren();
    }

    function unfold(tree) {
      const at = selected;
      unfolded.add(tree);
      render();
      select(at);
      input.focus();
    }

    function pointAt(item) {
      const at = items.indexOf(item);
      if (at >= 0 && at !== selected) select(at, { scroll: false });
    }

    function select(at, { scroll = true } = {}) {
      selected = Math.max(0, at);
      items.forEach((item, index) => {
        item.classList.toggle('on', index === selected);
        item.setAttribute('aria-selected', index === selected ? 'true' : 'false');
      });
      const item = items[selected];
      if (!item) return;
      if (scroll) item.scrollIntoView({ block: 'nearest' });
      if (item.dataset.address) showPreview(item.dataset.address);
    }

    const showPreview = (() => {
      /** The card's face; until its body arrives, the hit's own words stand in for it. */
      const draw = (card, partial, filled) => {
        const md = h('div', { class: 'md' });
        if (partial) md.append(h('p', { class: 'pending', text: card.snippet || card.excerpt || '' }));
        else if (card.body.trim()) md.innerHTML = card.html;
        else md.append(h('p', { class: 'empty', text: 'No body yet.' }));
        const kindLabel = { topic: 'Topic', literature: 'Literature', regular: 'Card' }[card.kind] || 'Card';
        const face = h('article', { class: 'card preview ' + card.kind + (partial ? ' partial' : '') + (filled ? ' filled' : '') + (card.workspace === 'agent' ? ' agent-ws' : '') + (card.type && card.type !== 'idea' ? ' type-' + card.type + hubClass(card) : ''), dataset: { address: card.address } },
          h('header', { class: 'card-head' },
            h('div', { class: 'card-meta' },
              h('span', { class: 'kind-tag ' + card.kind, text: kindLabel }),
              h('span', { class: 'ws-tag ' + card.workspace, text: wsLabel(card.workspace) }),
              typeStamp(card.type, null, card.facet, card.emphasis, card.level),
              h('span', { class: 'stamp', text: card.address })),
            h('h1', { class: 'title' }, titleNodes(card.title || '(untitled)'))),
          md);
        if (!partial) wireMarkdown(md);
        const onTable = spreads.has(card.address);
        preview.replaceChildren(face, h('div', { class: 'preview-actions' },
          h('button', { class: 'btn primary', onclick: () => choose(card.address) }, 'Open', h('kbd', { text: '↵' })),
          h('button', { class: 'btn', onclick: () => pullHit(card.address) }, onTable ? 'On the Table ✓' : 'To the Table', h('kbd', { text: '⇧↵' })),
          card.tree !== card.address && rootOf(card.tree) !== card.address
            ? h('span', { class: 'in-tree' }, 'in ', h('span', { class: 'addr', text: rootOf(card.tree) }), ' ', titleNodes(state.byAddress.get(rootOf(card.tree))?.title || '')) : null));
      };
      const load = debounce(async (address) => {
        try {
          const card = await api('card?at=' + enc(address));
          previews.set(address, card);
          if (preview.dataset.address === address && isOpen) draw(card, false, true);
        } catch { /* the card may be gone; its slip stays */ }
      }, 70);
      return (address, { redraw } = {}) => {
        if (preview.dataset.address === address && preview.firstChild && !redraw) return;
        preview.dataset.address = address;
        const card = previews.get(address);
        if (card) { draw(card); return; }
        const hit = data?.results.find((item) => item.address === address) || state.byAddress.get(address);
        if (hit) draw(hit, true);
        load(address);
      };
    })();

    function pullHit(address) {
      toggleTable(address);
      const on = spreads.has(address);
      for (const pull of hits.querySelectorAll(`.slip[data-address="${CSS.escape(address)}"] .slip-pull`)) {
        pull.classList.toggle('on', on);
        pull.textContent = on ? '✓' : '+';
      }
      if (preview.dataset.address === address) showPreview(address, { redraw: true });
    }

    /**
     * Open a card: the drawer slides away and the card in the hand travels to the reader,
     * where the same card lands, so the eye follows one object.
     */
    function choose(address) {
      const face = preview.querySelector(`.card[data-address="${CSS.escape(address)}"]`);
      const from = face?.getBoundingClientRect();
      const reader = $('#reader').getBoundingClientRect();
      const fly = from && from.width && reader.width > 200 && !reduceMotion();
      close();
      if (fly) {
        const ghost = face.cloneNode(true);
        ghost.classList.add('in-flight');
        Object.assign(ghost.style, { left: from.left + 'px', top: from.top + 'px', width: from.width + 'px', height: from.height + 'px' });
        document.body.append(ghost);
        const scale = Math.min(1.25, (reader.width - 52) / from.width);
        const dx = reader.left + 26 - from.left;
        const dy = reader.top + 28 - from.top;
        state.landing = true;
        ghost.animate([
          { transform: 'translate(0, 0) scale(1) rotate(0deg)', opacity: 1 },
          { transform: `translate(${dx * 0.55}px, ${dy * 0.55 - 18}px) scale(${(1 + scale) / 2}) rotate(-.6deg)`, opacity: 1, offset: 0.55 },
          { transform: `translate(${dx}px, ${dy}px) scale(${scale}) rotate(0deg)`, opacity: 1, offset: 0.86 },
          { transform: `translate(${dx}px, ${dy}px) scale(${scale})`, opacity: 0 },
        ], { duration: 460, easing: 'cubic-bezier(.3, .7, .3, 1)' }).finished.finally(() => ghost.remove());
      }
      go(address);
    }

    function onKey(event) {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        if (items.length) select((selected + (event.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length);
      } else if (event.key === 'Enter') {
        event.preventDefault();
        const item = items[selected];
        if (!item) return;
        if (item.dataset.fold) unfold(item.dataset.fold);
        else if (event.shiftKey) pullHit(item.dataset.address);
        else choose(item.dataset.address);
      } else if (event.key === 'Escape') {
        event.preventDefault();
        if (input.value) { input.value = ''; unfolded = new Set(); search.now(); } else close();
      }
    }

    return { open, close, get isOpen() { return isOpen; } };
  })();

  // ------------------------------------------------------------------- rail
  function renderWorkspaceSwitch() {
    const summaries = state.overview.workspaces || [];
    $('#workspace-switch').replaceChildren(...Object.keys(WORKSPACES).map((id) => {
      const summary = summaries.find((item) => item.id === id) || { topic_trees: 0, literature_trees: 0, cards: 0 };
      return h('button', {
        class: 'ws-tab ' + id + (state.workspace === id ? ' on' : ''),
        role: 'tab',
        'aria-selected': state.workspace === id ? 'true' : 'false',
        title: `${wsLabel(id)} workspace — ${WORKSPACES[id].mcp}`,
        onclick: () => setWorkspace(id, { toRoot: true }),
      },
        h('span', { class: 'ws-dot' }),
        h('span', { class: 'ws-name', text: wsLabel(id) }),
        h('span', { class: 'ws-count', text: String(summary.cards) }),
      );
    }));
    $('#workspace-note').textContent = state.workspace === 'agent'
      ? 'Trees AI agents write over MCP. Links may cross workspaces; cards never move between them.'
      : 'Your own trees. AI agents can read them over MCP but never change them.';
  }

  function renderRail() {
    renderWorkspaceSwitch();
    const trees = state.overview.trees.filter((tree) => (tree.workspace || 'human') === state.workspace);
    const drawer = (tree) => h('button', {
      class: 'drawer ' + tree.kind + (state.card && state.card.tree === tree.tree ? ' on' : ''),
      title: tree.title,
      onclick: () => { go(tree.root); openRail(false); },
    },
      h('span', { class: 'addr', text: tree.root }),
      h('span', { class: 't', text: tree.title || '(untitled)' }),
      h('span', { class: 'n', text: String(tree.count) }),
      tree.excerpt ? h('span', { class: 'x', text: tree.excerpt }) : null,
    );
    const fill = (id, kind, hint) => {
      const list = trees.filter((tree) => tree.kind === kind);
      $(id).replaceChildren(...(list.length ? list.map(drawer) : [h('div', { class: 'empty-hint', text: hint })]));
    };
    fill('#topic-drawers', 'topic', state.workspace === 'agent' ? 'No agent Topics yet.' : 'No Topics yet — press t.');
    fill('#literature-drawers', 'literature', state.workspace === 'agent' ? 'No agent Literature yet.' : 'No Literature yet — press l.');
    $('#archive-root').textContent = state.overview.archive_root;
  }

  function openRail(open) {
    $('#app').classList.toggle('rail-open', open ?? !$('#app').classList.contains('rail-open'));
  }

  // ----------------------------------------------------------------- topbar
  function renderTopbar() {
    const stats = state.overview.stats;
    $('#stats').textContent = `${stats.total} cards · ${stats.topics} topics · ${stats.literature} literature`;
    const pill = $('#broken-pill');
    pill.hidden = !state.overview.broken;
    pill.textContent = `${state.overview.broken} broken`;
    const trail = [
      h('button', {
        class: 'ws-tag ' + state.workspace, title: 'Switch workspace (w)',
        text: wsLabel(state.workspace),
        onclick: () => setWorkspace(state.workspace === 'agent' ? 'human' : 'agent', { toRoot: true }),
      }),
      h('a', { href: '#/', text: 'ROOT' }),
    ];
    if (state.pointer) {
      const chain = [];
      for (let at = state.pointer; at; at = parentOf(at)) chain.unshift(at);
      const shown = chain.length > 5 ? [chain[0], '…', ...chain.slice(-3)] : chain;
      for (const at of shown) {
        trail.push(h('span', { class: 'sep', text: '›' }));
        if (at === '…') trail.push(h('span', { class: 'sep', text: '…' }));
        else if (at === state.pointer) trail.push(h('span', { class: 'cur', text: at }));
        else trail.push(h('a', { href: '#/card/' + enc(at), text: at }));
      }
    }
    $('#trail').replaceChildren(...trail);
  }

  // ------------------------------------------------------------------ stage
  const camera = { map: { x: 40, y: 70, k: 1 }, table: { x: 0, y: 0, k: 1 }, constellation: { x: 0, y: 0, k: 1 } };
  const MODES = ['map', 'table', 'constellation'];
  /** The camera of the view that lays Cards on a plane: the Map's or the Table's. */
  const planeCamera = () => (state.mode === 'table' ? camera.table : camera.map);
  const reduceMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

  function setMode(mode) {
    state.mode = mode;
    store.set('zt-mode', mode);
    for (const button of document.querySelectorAll('.seg [data-mode]')) {
      button.classList.toggle('on', button.dataset.mode === mode);
    }
    renderStage();
  }

  function setExpandAll(on) {
    state.expandAll = on;
    store.set('zt-expand', on ? '1' : '0');
    renderStage();
  }

  function renderStage() {
    if (!state.overview) return;
    const inTree = state.mode === 'map' && state.card;
    const onTableMode = state.mode === 'table';
    $('#desk').hidden = !(state.mode === 'map' && !state.card);
    $('#plane').hidden = !inTree;
    $('#table-plane').hidden = !onTableMode;
    $('#table-tools').hidden = !onTableMode;
    $('#viewport').classList.toggle('table-mode', onTableMode);
    $('#app').classList.toggle('at-table', onTableMode);
    if (!onTableMode) { $('#table-empty').hidden = true; $('#table-hand').hidden = true; }
    $('#sky').hidden = state.mode !== 'constellation';
    $('#sky-label').hidden = true;
    $('#legend').hidden = !inTree;
    $('.zoom').hidden = state.mode === 'map' && !state.card;
    renderTableCount();
    const lens = $('#lens-toggle');
    lens.hidden = !inTree;
    lens.classList.toggle('on', state.expandAll);
    lens.title = state.expandAll ? 'Focus on the pointer and fold the rest (x)' : 'Unfold the whole tree (x)';
    lens.querySelector('span').textContent = state.expandAll ? 'Whole tree' : 'Focus';
    const title = $('#stage-title');
    if (state.mode === 'constellation') {
      title.textContent = `Constellation · ${state.overview.trees.length} trees · ${state.nodes.length} cards`;
      sky.show();
    } else if (onTableMode) {
      const count = tableView.render();
      title.textContent = count ? `On the Table · ${count} card${count === 1 ? '' : 's'}` : 'The Table';
    } else if (!state.card) {
      title.textContent = `The cabinet · ${wsLabel(state.workspace)} workspace`;
      renderDesk();
    } else {
      map.render();
      const tree = state.tree;
      const shown = map.shownCount();
      const total = tree.cards.length;
      title.textContent = `${tree.kind === 'literature' ? 'Literature' : 'Topic'} ${tree.tree} · ${tree.title} — `
        + (shown < total ? `${shown} of ${total} cards in view` : `${total} card${total === 1 ? '' : 's'}`);
    }
  }

  function renderDesk() {
    const agent = state.workspace === 'agent';
    const trees = state.overview.trees.filter((tree) => (tree.workspace || 'human') === state.workspace);
    const zone = (kind) => trees.filter((tree) => tree.kind === kind);
    const largest = Math.max(1, ...trees.map((tree) => tree.count));
    // A stack shows a stamp for each kind of non-Idea Card it holds, with a tally past one.
    const tally = (type, count) => (count
      ? h('span', { class: 'tally' }, typeStamp(type, `${count} ${CARD_TYPES[type].label} Card${count === 1 ? '' : 's'}`), count > 1 ? `×${count}` : null)
      : null);
    // A bigger tree is a thicker stack: up to four sheets under the face.
    const stack = (tree) => h('button', {
      class: 'stack ' + tree.kind, onclick: () => go(tree.root), title: tree.title,
    },
      tree.count > 6 ? h('span', { class: 'sheet s4' }) : null,
      tree.count > 2 ? h('span', { class: 'sheet s3' }) : null,
      tree.count > 1 ? h('span', { class: 'sheet s2' }) : null,
      h('span', { class: 'face' },
        h('span', { class: 'kind', text: `${tree.kind === 'literature' ? 'Literature' : 'Topic'} · ${tree.root}` }),
        h('span', { class: 't', text: tree.title || '(untitled)' }),
        tree.excerpt ? h('span', { class: 'x', text: tree.excerpt }) : null,
        h('span', { class: 'foot' },
          h('span', { class: 'types' },
            h('span', { class: 'count', text: `${tree.count} card${tree.count === 1 ? '' : 's'}` }),
            tally('event', tree.events),
            tally('model', tree.models),
          ),
          h('span', { class: 'gauge', title: `${tree.count} of the ${largest} cards in the largest tree` },
            h('i', { style: `width: ${Math.max(6, Math.round(100 * tree.count / largest))}%` })),
        ),
      ),
    );
    const newStack = (label, key, action) => h('button', { class: 'stack new', onclick: action },
      h('span', { class: 'face' }, h('span', { class: 'plus', text: '+' }), h('span', { text: label }), h('kbd', { text: key })));
    const zoneSection = (kind, label, newLabel, key, action) => [
      h('h2', { class: 'zone-head ' + kind }, h('span', { text: label }), h('span', { class: 'n', text: String(zone(kind).length) })),
      h('div', { class: 'desk-grid' }, zone(kind).map(stack), newStack(newLabel, key, action)),
    ];
    $('#desk').replaceChildren(
      h('div', { class: 'desk-intro' },
        h('span', { class: 'ws-tag ' + state.workspace, text: `${wsLabel(state.workspace)} workspace` }),
        h('h1', { text: agent
          ? (trees.length ? 'The agent’s slip-box' : 'The agent’s slip-box is empty')
          : (trees.length ? 'Your slip-box' : 'An empty slip-box') }),
        h('p', { text: agent
          ? 'AI agents grow these trees through the zt MCP server. Their cards may link to yours; yours stay read-only to them.'
          : (trees.length
            ? 'Every Topic and Literature Card roots its own tree. Open a stack to walk its cards.'
            : 'Start a Topic for your own ideas, or a Literature Card for a work you are reading.') }),
      ),
      ...zoneSection('topic', 'Topic zone', 'New Topic', 't', () => newCard('topic', '')),
      ...zoneSection('literature', 'Literature zone', 'New Literature', 'l', () => newLiterature()),
    );
  }

  // -------------------------------------------------------------------- map
  // The Map is a focus lens over the current tree (see layout.js). The Pointer card is
  // pulled out of the drawer; its path, parent, children, and run neighbours stand up as
  // cards; the rest of what is shown lies as slips; everything else is folded into "+N"
  // counts. Only the Pointer's Links are drawn. Each move of the Pointer re-runs the
  // layout and glides the cards to their new places.
  const map = (() => {
    const els = new Map();   // address -> node element
    const paths = new Map(); // edge key -> SVG path
    let drawn = new Map();   // address -> geometry on screen now
    let view = null;
    let hovered = null;
    let frame = 0;
    const cam = camera.map;

    const STAGE_TOP = 62;
    const lerp = (a, b, t) => a + (b - a) * t;

    function content(el, card, n, outline) {
      const external = card.links.filter((target) => !outline.card.has(target)).length;
      const signature = [n.lod, n.role, n.folded, card.title, card.type, card.facet, card.emphasis, card.level, card.inbound, external, card.excerpt].join('\u0001');
      el.className = ['node', n.lod, 'role-' + n.role,
        card.edge === 'root' ? 'root ' + card.kind : '',
        card.type && card.type !== 'idea' ? 'type-' + card.type + hubClass(card) : '',
        card.facet ? 'model-part' : '',
        n.folded ? 'folded' : '',
        state.linkedIn && state.linkedIn.has(card.address) ? 'in' : '',
        state.linkedOut && state.linkedOut.has(card.address) ? 'out' : '',
        spreads.has(card.address) ? 'on-table' : '',
      ].filter(Boolean).join(' ');
      if (el.dataset.sig === signature) return;
      el.dataset.sig = signature;
      el.title = card.title + (card.excerpt ? '\n\n' + card.excerpt : '');
      const fold = n.folded ? h('span', {
        class: 'badge fold', title: `${n.folded} more card${n.folded === 1 ? '' : 's'} filed behind — open this card to unfold them`,
        text: '+' + n.folded,
      }) : null;
      const title = h('span', { class: 't' + (card.title ? '' : ' untitled') }, card.title ? titleNodes(card.title) : 'untitled');
      const slash = card.address.indexOf('/');
      // Inside a Literature tree the long Citation key is known, so cards show the rest.
      const short = card.edge !== 'root' && slash > 4 ? '…' + card.address.slice(slash) : card.address;
      if (n.lod === 'slip') {
        el.replaceChildren(...[h('span', { class: 'addr', text: short }), title, typeStamp(card.type, null, card.facet, card.emphasis, card.level), fold].filter(Boolean));
        return;
      }
      el.replaceChildren(...[
        h('span', { class: 'top' },
          h('span', { class: 'addr', text: n.lod === 'focus' ? card.address : short }),
          h('span', { class: 'badges' },
            typeStamp(card.type, null, card.facet, card.emphasis, card.level),
            card.inbound ? h('span', { class: 'badge in', title: `Referred by ${card.inbound}`, text: '←' + card.inbound }) : null,
            external ? h('span', { class: 'badge', title: 'Links to other trees', text: '↗' + external }) : null,
            fold,
          ),
        ),
        title,
        n.lod === 'focus' && card.excerpt ? h('span', { class: 'x', text: card.excerpt }) : null,
      ].filter(Boolean));
    }

    /** Where the camera should be: a view that fits is centred and stays still; a larger
     *  one follows the Pointer, held a little left of centre and in the upper third. */
    function cameraTarget(k) {
      const viewport = $('#viewport');
      const vw = viewport.clientWidth; const vh = viewport.clientHeight;
      const b = view.bounds;
      const f = view.index.get(view.focus);
      const room = { w: vw - 64, h: vh - STAGE_TOP - 56 };
      const reach = 110; // Link arcs swing out to the right of the cards.
      let x; let y;
      if ((b.w + reach) * k <= room.w) x = 32 + (room.w - (b.w + reach) * k) / 2;
      else {
        // Show the right edge (the pointer's successors) and as much context to the left
        // as fits, but never push the pointer card itself out of view.
        const lo = vw - 32 - (f.x + f.w) * k;
        const hi = 32 - f.x * k;
        x = Math.min(40, Math.max(lo, Math.min(hi, vw - 32 - b.w * k)));
      }
      if (b.h * k <= room.h) y = STAGE_TOP + (room.h - b.h * k) / 2;
      else {
        y = STAGE_TOP + room.h * 0.36 - (f.y + f.h / 2) * k;
        y = Math.min(STAGE_TOP + 10, Math.max(vh - 56 - b.h * k, y));
      }
      return { x, y, k };
    }

    function paint(geometry) {
      for (const [address, g] of geometry) {
        const el = els.get(address);
        if (!el) continue;
        el.style.transform = `translate(${g.x}px, ${g.y}px)`;
        el.style.width = g.w + 'px';
        el.style.height = g.h + 'px';
      }
      const edges = $('#edges');
      const keep = new Set();
      const put = (key, d, cls) => {
        keep.add(key);
        let path = paths.get(key);
        if (!path) { path = svg('path', {}); paths.set(key, path); edges.append(path); }
        path.setAttribute('d', d);
        path.setAttribute('class', cls);
      };
      for (const edge of view.edges) {
        const a = geometry.get(edge.from); const b = geometry.get(edge.to);
        if (a && b) put('e' + edge.from + '>' + edge.to, ZtLayout.edgePath(edge, a, b), 'e-' + edge.kind + (edge.hot ? ' hot' : ''));
      }
      const focus = geometry.get(view.focus);
      if (focus) {
        const near = structural(view.focus);
        for (const address of state.linkedOut || []) {
          const b = geometry.get(address);
          if (b && !near.has(address)) put('o' + address, ZtLayout.linkArc(focus, b), 'e-link out');
        }
        for (const address of state.linkedIn || []) {
          const b = geometry.get(address);
          if (b && !near.has(address) && !state.linkedOut.has(address)) put('i' + address, ZtLayout.linkArc(b, focus), 'e-link in');
        }
      }
      const over = hovered && hovered !== view.focus ? geometry.get(hovered) : null;
      if (over) {
        const near = structural(hovered);
        for (const target of state.outline.card.get(hovered).links) {
          const b = geometry.get(target);
          if (b && target !== hovered && !near.has(target)) put('h' + target, ZtLayout.linkArc(over, b), 'e-link peek');
        }
      }
      for (const [key, path] of paths) if (!keep.has(key)) { path.remove(); paths.delete(key); }
    }

    /** Cards the tree structure already ties to `address`; a Link arc to one adds nothing. */
    function structural(address) {
      const outline = state.outline;
      const up = outline.up.get(address);
      const tied = new Set([...outline.kids.get(address), up, outline.card.get(address).parent]);
      for (const sibling of up != null ? outline.kids.get(up) : []) {
        if (outline.card.get(sibling).parent === address) tied.add(sibling);
      }
      return tied;
    }

    function glide(from, to, camFrom, camTo, ms) {
      const id = ++frame;
      const t0 = performance.now();
      const step = (now) => {
        if (id !== frame) return;
        const t = ms <= 0 || reduceMotion() ? 1 : Math.min(1, (now - t0) / ms);
        const e = 1 - Math.pow(1 - t, 3);
        const geometry = new Map();
        for (const [address, b] of to) {
          const a = from.get(address) || b;
          geometry.set(address, { x: lerp(a.x, b.x, e), y: lerp(a.y, b.y, e), w: lerp(a.w, b.w, e), h: lerp(a.h, b.h, e) });
        }
        drawn = geometry;
        paint(geometry);
        cam.x = lerp(camFrom.x, camTo.x, e); cam.y = lerp(camFrom.y, camTo.y, e); cam.k = lerp(camFrom.k, camTo.k, e);
        applyCamera();
        if (t < 1) requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    }

    function render() {
      const card = state.card;
      const outline = state.outline;
      if (!card || !outline) return;
      const inTree = (address) => outline.card.has(address) && address !== card.address;
      state.linkedOut = new Set(card.outbound.map((link) => link.address).filter(inTree));
      const inbound = card.inbound.map((link) => link.address).filter(inTree);
      // A Card that half the tree refers to (a tree root) would light up everything.
      state.linkedIn = new Set(inbound.length <= 16 ? inbound : []);
      const fresh = !view || view.tree !== state.tree.tree;
      view = ZtLayout.focusLens(outline, card.address, {
        links: [...state.linkedOut, ...state.linkedIn], expandAll: state.expandAll,
      });
      view.tree = state.tree.tree;

      const nodes = $('#nodes');
      const from = new Map(drawn);
      const to = new Map(view.nodes.map((n) => [n.address, { x: n.x, y: n.y, w: n.w, h: n.h }]));
      if (fresh) {
        for (const el of els.values()) el.remove();
        for (const path of paths.values()) path.remove();
        els.clear(); paths.clear(); from.clear();
      }
      for (const n of view.nodes) {
        let el = els.get(n.address);
        if (!el) {
          const address = n.address;
          el = h('button', {
            class: 'node',
            dataset: { address },
            onclick: (event) => {
              event.stopPropagation();
              // Shift-click lays the card on the Table, or takes it back, without leaving the Map.
              if (event.shiftKey) { toggleTable(address); render(); return; }
              go(address);
            },
            onpointerenter: () => { hovered = address; paint(drawn); },
            onpointerleave: () => { if (hovered === address) { hovered = null; paint(drawn); } },
          });
          el.classList.add('entering');
          els.set(address, el);
          nodes.append(el);
          // A newly shown card slides out from behind its nearest card already on screen.
          let at = outline.up.get(address);
          while (at != null && !from.has(at)) at = outline.up.get(at);
          const origin = at != null ? from.get(at) : null;
          if (origin) from.set(address, { x: origin.x, y: origin.y, w: n.w, h: n.h });
          setTimeout(() => el.classList.remove('entering'), 400);
        }
        content(el, outline.card.get(n.address), n, outline);
      }
      for (const [address, el] of els) {
        if (to.has(address)) continue;
        els.delete(address);
        el.classList.add('leaving');
        setTimeout(() => el.remove(), 240);
      }
      if (hovered && !to.has(hovered)) hovered = null;
      const edges = $('#edges');
      edges.setAttribute('width', view.bounds.w + 400);
      edges.setAttribute('height', view.bounds.h + 40);
      // Reading needs legible type: navigating out of a zoomed-out overview zooms back in.
      const k = fresh || cam.k < 0.8 ? 1 : cam.k;
      const target = cameraTarget(k);
      if (fresh) glide(to, to, target, target, 0);
      else glide(from, to, { ...cam }, target, 360);
    }

    return {
      render,
      shownCount: () => (view ? view.nodes.length : 0),
      fit() {
        if (!view) return;
        const viewport = $('#viewport');
        const room = { w: viewport.clientWidth - 64, h: viewport.clientHeight - STAGE_TOP - 56 };
        const k = Math.max(0.22, Math.min(1.1, room.w / (view.bounds.w + 110), room.h / view.bounds.h));
        glide(drawn, drawn, { ...cam }, cameraTarget(k), 300);
      },
      reset() { view = null; drawn = new Map(); },
    };
  })();

  function applyCamera() {
    const cam = camera.map;
    $('#plane').style.transform = `translate(${cam.x}px, ${cam.y}px) scale(${cam.k})`;
    const t = camera.table;
    $('#table-plane').style.transform = `translate(${t.x}px, ${t.y}px) scale(${t.k})`;
  }

  function fitView() {
    if (state.mode === 'constellation') sky.fit();
    else if (state.mode === 'table') tableView.fit(true, true);
    else map.fit();
  }

  function zoomAt(factor, px, py) {
    if (state.mode === 'constellation') { sky.zoom(factor, px, py); return; }
    const cam = planeCamera();
    const k = Math.max(0.2, Math.min(2.5, cam.k * factor));
    cam.x = px - (px - cam.x) * (k / cam.k);
    cam.y = py - (py - cam.y) * (k / cam.k);
    cam.k = k;
    applyCamera();
  }

  // A hand that carries a card or the view selects no words on its way: nothing in the views
  // is selectable (see app.css), and while a press that began in them lasts, nothing on the
  // page is.
  function installSteadyHand() {
    const viewport = $('#viewport');
    const release = () => document.body.classList.remove('holding');
    viewport.addEventListener('pointerdown', (event) => {
      if (event.button !== 0 || isTyping(event.target)) return;
      document.body.classList.add('holding');
      window.getSelection()?.removeAllRanges();
    }, { capture: true });
    window.addEventListener('pointerup', release, { capture: true });
    window.addEventListener('pointercancel', release, { capture: true });
    window.addEventListener('blur', release);
    document.addEventListener('selectstart', (event) => {
      if (document.body.classList.contains('holding')) event.preventDefault();
    });
    // Images and links in the views are never dragged out as files or URLs.
    viewport.addEventListener('dragstart', (event) => event.preventDefault());
  }

  function installPanZoom() {
    const viewport = $('#viewport');
    let drag = null;
    viewport.addEventListener('pointerdown', (event) => {
      if (event.button !== 0 || event.target.closest('.stack, .desk, .tcard, .reach, .table-hand, .table-empty')) return;
      // A press on a Map card opens it, unless the hand moves: then it pans the view too.
      drag = { x: event.clientX, y: event.clientY, moved: false, captured: false, pointer: event.pointerId };
    });
    viewport.addEventListener('pointermove', (event) => {
      // A press released outside the view, before it moved, left no pointerup here.
      if (drag && !drag.captured && event.buttons === 0) drag = null;
      if (!drag) { if (state.mode === 'constellation') sky.hover(event); return; }
      const dx = event.clientX - drag.x; const dy = event.clientY - drag.y;
      if (!drag.moved && Math.abs(dx) + Math.abs(dy) <= 3) return;
      if (!drag.captured) {
        // Captured only once it moves, so a click still reaches the card pressed.
        drag.captured = true;
        try { viewport.setPointerCapture(drag.pointer); } catch { /* a pointer the browser no longer tracks */ }
        viewport.classList.add('dragging');
      }
      drag.moved = true;
      drag.x = event.clientX; drag.y = event.clientY;
      if (state.mode === 'constellation') sky.pan(dx, dy);
      else { const cam = planeCamera(); cam.x += dx; cam.y += dy; applyCamera(); }
    });
    const end = (event) => {
      if (drag && !drag.moved && state.mode === 'constellation') sky.click(event);
      drag = null;
      viewport.classList.remove('dragging');
    };
    viewport.addEventListener('pointerup', end);
    viewport.addEventListener('pointercancel', () => { drag = null; viewport.classList.remove('dragging'); });
    viewport.addEventListener('pointerleave', () => { if (state.mode === 'constellation') sky.leave(); });
    viewport.addEventListener('wheel', (event) => {
      if (state.mode === 'map' && !state.card) return; // the desk scrolls normally
      event.preventDefault();
      const rect = viewport.getBoundingClientRect();
      if (event.ctrlKey || event.metaKey || state.mode === 'constellation') {
        zoomAt(Math.exp(-event.deltaY * 0.0022), event.clientX - rect.left, event.clientY - rect.top);
      } else {
        const cam = planeCamera();
        cam.x -= event.deltaX; cam.y -= event.deltaY; applyCamera();
      }
    }, { passive: false });
    for (const button of document.querySelectorAll('[data-zoom]')) {
      button.addEventListener('click', () => {
        const kind = button.dataset.zoom;
        if (kind === 'fit') return fitView();
        zoomAt(kind === 'in' ? 1.2 : 1 / 1.2, viewport.clientWidth / 2, viewport.clientHeight / 2);
      });
    }
  }

  // ---------------------------------------------------------------- spreads
  // A Spread is a set of Cards laid out together on the Table: which Cards, where each lies,
  // and which lies on top (the last listed). The archive keeps them (src/spread.rs) and moves
  // them with their Cards; every change here is saved a moment later.
  const spreads = (() => {
    let doc = { current: null, spreads: [] };
    let dirty = false;
    let timer = 0;
    let saving = Promise.resolve();
    const newId = () => Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
    const self = {
      get doc() { return doc; },
      now: () => doc.spreads.find((spread) => spread.id === doc.current) || null,
      /** The Spread on the Table, made when the first Card is laid down. */
      ensure() {
        let spread = self.now();
        if (!spread) {
          spread = { id: newId(), name: `Spread ${doc.spreads.length + 1}`, cards: [] };
          doc.spreads.push(spread);
          doc.current = spread.id;
        }
        return spread;
      },
      create(name) {
        const spread = { id: newId(), name, cards: [] };
        doc.spreads.push(spread);
        doc.current = spread.id;
        self.changed();
        return spread;
      },
      has: (address) => !!self.now()?.cards.some((card) => card.at === address),
      /** The Cards on the Table that still exist, in the order they lie (the last on top). */
      cards: () => (self.now()?.cards || []).filter((card) => state.byAddress.has(card.at)),
      changed() {
        dirty = true;
        clearTimeout(timer);
        timer = setTimeout(() => self.flush(), 450);
        renderTableCount();
      },
      flush() {
        clearTimeout(timer);
        if (!dirty) return saving;
        dirty = false;
        const body = JSON.parse(JSON.stringify(doc));
        saving = saving.then(() => api('spreads', body)).then(() => {}, fail);
        return saving;
      },
      take(kept) {
        if (dirty) return;
        doc = kept && Array.isArray(kept.spreads) ? kept : { current: null, spreads: [] };
      },
    };
    return self;
  })();

  function renderTableCount() {
    const count = spreads.cards().length;
    const badge = $('#table-count');
    badge.hidden = !count;
    badge.textContent = String(count);
    $('#spread-name').textContent = spreads.now()?.name || 'Spread';
    // Only the Reader's Table button changes; re-rendering the Reader would lose its scroll.
    const button = $('#reader .table-btn');
    if (button && state.card) {
      const on = spreads.has(state.card.address);
      button.firstChild.textContent = on ? 'On the Table ✓' : 'To the Table';
      button.title = on ? 'Put it back in the box' : 'Lay it on the Table beside the cards you are gathering (P: with every card it relates to)';
    }
  }

  /** Cards the open Card relates to: what it links, what links it, and its place in its tree. */
  function relatedTo(address) {
    const node = state.byAddress.get(address);
    const related = new Set(node ? node.links : []);
    for (const source of state.inboundOf.get(address) || []) related.add(source);
    if (node && node.edge !== 'root' && node.parent) related.add(node.parent);
    for (const child of state.childrenOf.get(address) || []) related.add(child.address);
    related.delete(address);
    return related;
  }

  /**
   * Lay Cards on the Table. Each lies in the nearest free place beside `near` (a Card on the
   * Table), else beside the Card held, else in the middle of the view. `from` is where a Card
   * comes from on the Table plane, so it slides out from there.
   */
  function pull(addresses, { near, from } = {}) {
    const spread = spreads.ensure();
    const lying = new Map(spread.cards.map((card) => [card.at, card]));
    const size = ZtLayout.TABLE;
    const anchorCard = lying.get(near) || lying.get(state.pointer) || spread.cards[spread.cards.length - 1];
    const want = anchorCard ? { x: anchorCard.x + size.w + size.gap, y: anchorCard.y } : tableView.middle();
    const added = [];
    for (const address of addresses) {
      if (!state.byAddress.has(address) || lying.has(address)) continue;
      const rects = spread.cards.map((card) => ({ x: card.x, y: card.y, w: size.w, h: size.h }));
      const spot = ZtLayout.freeSpot(rects, want, size);
      const card = { at: address, x: spot.x, y: spot.y };
      spread.cards.push(card);
      lying.set(address, card);
      if (from) tableView.arriveFrom(address, from);
      added.push(address);
    }
    if (added.length) spreads.changed();
    return added;
  }

  function putBack(address) {
    const spread = spreads.now();
    if (!spread) return;
    const at = spread.cards.findIndex((card) => card.at === address);
    if (at < 0) return;
    spread.cards.splice(at, 1);
    spreads.changed();
  }

  /** `p`: lay a Card on the Table, or put it back in the box. */
  function toggleTable(address) {
    if (!address) throw new Error('p needs a Card; open one first');
    if (spreads.has(address)) {
      putBack(address);
      toast(`${address} is back in the box`);
    } else {
      pull([address]);
      toast(`${address} lies on the Table · ${spreads.cards().length} card${spreads.cards().length === 1 ? '' : 's'}`);
    }
    renderStage();
  }

  /** `P`: lay a Card on the Table with every Card it relates to, gathered around it. */
  function spreadAround(address) {
    if (!address) throw new Error('P needs a Card; open one first');
    pull([address]);
    const added = pull([...relatedTo(address)], { near: address });
    tableView.arrange('gather', address, added);
    setMode('table');
    setTimeout(() => tableView.fit(), 160);
  }

  /** Write a Card that lists the Cards on the Table in reading order, filed after the Card held. */
  async function fileSpread() {
    const cards = spreads.cards();
    if (!cards.length) throw new Error('the Table is empty: lay some cards on it first');
    if (!state.card) throw new Error('open the card to file the new card after, then try again');
    const order = ZtLayout.readingOrder(cards.map((card) => card.at), (at) => state.byAddress.get(at)?.links || []);
    const body = order.map((at) => `- [[${at}]] ${state.byAddress.get(at)?.title || ''}`.trimEnd()).join('\n');
    const kind = state.card.context === 'regular' ? 'side' : 'direct';
    await newCard(kind, '', { title: spreads.now()?.name || '', body, onTable: true });
  }

  function showSpreads() {
    const doc = spreads.doc;
    const open = (spread, close) => {
      doc.current = spread.id;
      spreads.changed();
      close();
      tableView.reset();
      setMode('table');
    };
    listDialog(`Spreads · ${doc.spreads.length}`, (close) => [
      doc.spreads.length ? h('div', { class: 'spread-list' }, doc.spreads.map((spread) => h('div', { class: 'spread-row' + (spread.id === doc.current ? ' on' : '') },
        h('button', { class: 'spread-open', onclick: () => open(spread, close), title: 'Lay this spread on the Table' },
          h('span', { class: 't', text: spread.name || 'Untitled' }),
          h('span', { class: 'n', text: `${spread.cards.length} card${spread.cards.length === 1 ? '' : 's'}` })),
        h('button', { class: 'mini-btn', text: 'Rename', onclick: async () => {
          close();
          const name = await promptValue('Rename spread', 'Name:', spread.name);
          if (name) { spread.name = name; spreads.changed(); renderStage(); }
        } }),
        h('button', { class: 'mini-btn danger', text: 'Remove', onclick: () => {
          if (!window.confirm(`Remove the spread “${spread.name}”? Its cards stay filed in the slip-box.`)) return;
          doc.spreads.splice(doc.spreads.indexOf(spread), 1);
          if (doc.current === spread.id) doc.current = doc.spreads[0]?.id || null;
          spreads.changed();
          close();
          tableView.reset();
          renderStage();
        } }),
      ))) : h('p', { text: 'No spreads yet.' }),
      h('div', { class: 'actions' }, actionButton('New spread', null, async () => {
        close();
        const name = await promptValue('New spread', 'Name — what the cards on it are about:', `Spread ${doc.spreads.length + 1}`);
        if (name === null) return;
        spreads.create(name || `Spread ${doc.spreads.length + 1}`);
        tableView.reset();
        setMode('table');
      }, { primary: true })),
      h('p', { class: 'hint', text: 'A spread is a set of cards laid out together. It changes no card and survives restarts; “File as card” writes it into the slip-box as a card that lists them.' }),
    ]);
  }

  // ------------------------------------------------------------------ table
  // The Table is where a reader builds a set of related Cards, wherever each is filed. It
  // follows the desk: Cards lie at a slight angle where they were put down; the Card held is
  // squared up, lifted, and on top; the Cards it relates to keep their ink and their threads;
  // the rest recede. Its unlaid neighbours wait within reach, tucked under its edge, one click
  // from the Table. A Card picked up tilts with the hand, settles when laid down, and goes
  // back in the box when dropped there.
  const tableView = (() => {
    const els = new Map();
    const arrivals = new Map(); // address -> where on the plane a Card slides in from
    const cam = camera.table;
    const size = ZtLayout.TABLE;
    let shown = null;           // the Spread last laid out, by id
    let lastFocus = null;
    let hovered = null;
    let drag = null;
    let reachOpen = false;
    let current = { cards: [], focus: null, near: new Set() };

    /** A Card's angle at rest: a few degrees either way, the same every time. */
    const restAngle = (address) => {
      let hash = 7;
      for (const ch of address) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
      return ((hash % 1000) / 1000) * 3.4 - 1.7;
    };
    const place = (el, card, angle) => {
      el.style.transform = `translate(${card.x}px, ${card.y}px) rotate(${angle}deg)`;
    };
    const rectOf = (card) => ({ x: card.x, y: card.y, w: size.w, h: size.h });

    function content(el, address) {
      const node = state.byAddress.get(address);
      const signature = [node.title, node.type, node.facet, node.emphasis, node.level, node.excerpt, node.links.length, node.inbound, node.workspace].join('\u0001');
      if (el.dataset.sig === signature) return;
      el.dataset.sig = signature;
      el.title = node.title + (node.excerpt ? '\n\n' + node.excerpt : '');
      el.replaceChildren(
        h('span', { class: 'top' },
          h('span', { class: 'addr', text: address }),
          h('span', { class: 'badges' },
            typeStamp(node.type, null, node.facet, node.emphasis, node.level),
            node.workspace === 'agent' ? h('span', { class: 'ws-tag agent', text: 'agent' }) : null)),
        h('span', { class: 't' + (node.title ? '' : ' untitled') }, node.title ? titleNodes(node.title) : 'untitled'),
        node.excerpt ? h('span', { class: 'x' }, titleNodes(node.excerpt)) : null,
        h('span', { class: 'foot' },
          node.inbound ? h('span', { title: `Linked from ${node.inbound} card${node.inbound === 1 ? '' : 's'}`, text: '←' + node.inbound }) : null,
          node.links.length ? h('span', { title: `Links to ${node.links.length} card${node.links.length === 1 ? '' : 's'}`, text: '→' + node.links.length }) : null),
        h('button', {
          class: 'put-back', title: 'Put it back in the box', 'aria-label': `Put ${address} back in the box`,
          onpointerdown: (event) => event.stopPropagation(),
          onclick: (event) => { event.stopPropagation(); sendToBox(address); },
        }, '↩'),
      );
    }

    function make(address) {
      const node = state.byAddress.get(address);
      const el = h('div', {
        class: 'tcard' + (node.edge === 'root' ? ' root ' + node.kind : '') + (node.type && node.type !== 'idea' ? ' type-' + node.type + hubClass(node) : ''),
        dataset: { address }, role: 'button', tabindex: '0',
      });
      el.addEventListener('pointerdown', (event) => grab(event, address, el));
      el.addEventListener('pointerenter', () => { if (!drag) { hovered = address; paintThreads(); } });
      el.addEventListener('pointerleave', () => { if (hovered === address) { hovered = null; paintThreads(); } });
      el.addEventListener('keydown', (event) => { if (event.key === 'Enter') go(address); });
      return el;
    }

    /** A Card comes onto the Table: from where it was (a slip within reach), else dealt from above. */
    function enter(el, card, index) {
      const from = arrivals.get(card.at);
      arrivals.delete(card.at);
      el.classList.add('arriving');
      if (from) el.style.transform = `translate(${from.x}px, ${from.y}px) rotate(${from.angle || 0}deg) scale(.86)`;
      else el.style.transform = `translate(${card.x - 26}px, ${card.y - 54}px) rotate(${restAngle(card.at) - 7}deg) scale(1.06)`;
      el.style.transitionDelay = Math.min(index, 14) * 32 + 'ms';
      // Commit the starting place before moving on, so the move is a transition.
      void el.offsetWidth;
      el.classList.remove('arriving');
      place(el, card, card.at === current.focus ? 0 : restAngle(card.at));
      setTimeout(() => { el.style.transitionDelay = ''; }, 600);
    }

    function render() {
      const spread = spreads.now();
      if (!spread || shown !== spread.id) {
        for (const el of els.values()) el.remove();
        els.clear();
        lastFocus = null;
      }
      const fresh = !spread || shown !== spread.id;
      shown = spread ? spread.id : null;
      const cards = spreads.cards();
      const focus = state.pointer && cards.some((card) => card.at === state.pointer) ? state.pointer : null;
      const near = focus ? relatedTo(focus) : new Set();
      current = { cards, focus, near };
      if (focus !== lastFocus) reachOpen = false;
      const holder = $('#tcards');
      const keep = new Set();
      let entering = 0;
      cards.forEach((card, index) => {
        keep.add(card.at);
        let el = els.get(card.at);
        const isNew = !el;
        if (isNew) {
          el = make(card.at);
          els.set(card.at, el);
          holder.append(el);
        }
        content(el, card.at);
        el.classList.toggle('focus', card.at === focus);
        el.classList.toggle('near', near.has(card.at));
        el.classList.toggle('recede', !!focus && card.at !== focus && !near.has(card.at));
        // Paper lies in the order it was put down; the Card held lies on top of it all.
        el.style.zIndex = card.at === focus ? 900 : String(index + 1);
        if (isNew) enter(el, card, fresh ? index : entering++);
        else if (!drag || drag.at !== card.at) place(el, card, card.at === focus ? 0 : restAngle(card.at));
      });
      for (const [address, el] of els) {
        if (keep.has(address)) continue;
        els.delete(address);
        leave(el);
      }
      paintThreads();
      renderReach();
      renderHand();
      const empty = $('#table-empty');
      empty.hidden = cards.length > 0;
      if (!cards.length) renderEmpty(empty);
      if (fresh) fit(false);
      else if (focus && focus !== lastFocus) keepInView(focus);
      lastFocus = focus;
      applyCamera();
      return cards.length;
    }

    /** A Card put back slides toward the box and drops into it. */
    function leave(el) {
      const box = $('#table-box').getBoundingClientRect();
      const view = $('#viewport').getBoundingClientRect();
      const x = (box.left + box.width / 2 - view.left - cam.x) / cam.k - size.w / 2;
      const y = (box.top - view.top - cam.y) / cam.k;
      el.classList.add('leaving');
      el.style.transform = `translate(${x}px, ${y}px) rotate(-4deg) scale(.42)`;
      setTimeout(() => el.remove(), 460);
    }

    function sendToBox(address) {
      $('#table-box').classList.add('open');
      putBack(address);
      renderStage();
      setTimeout(() => $('#table-box').classList.remove('open'), 520);
    }

    function paintThreads() {
      const layer = $('#threads');
      const { cards, focus } = current;
      const lying = new Map(cards.map((card) => [card.at, card]));
      const groups = [];
      for (const card of cards) {
        const node = state.byAddress.get(card.at);
        for (const target of node.links) {
          if (target === card.at || !lying.has(target)) continue;
          const kind = (node.kinds && node.kinds[target]) || 'refers';
          const hot = !!focus && (card.at === focus || target === focus);
          const warm = !hot && !!hovered && (card.at === hovered || target === hovered);
          const back = (state.byAddress.get(target)?.links || []).includes(card.at) && card.at > target;
          const geometry = ZtLayout.thread(rectOf(card), rectOf(lying.get(target)), back);
          const group = svg('g', { class: 'thread kind-' + kind + (hot ? ' hot' : warm ? ' warm' : focus ? ' faint' : '') });
          const label = svg('title', {});
          label.textContent = `${card.at} ${KINDS[kind] ? KINDS[kind].fwd : 'refers to'} ${target}`;
          const tip = geometry.end;
          const a = geometry.angle;
          const arrow = `M ${tip.x - 8 * Math.cos(a - 0.45)} ${tip.y - 8 * Math.sin(a - 0.45)} L ${tip.x} ${tip.y} L ${tip.x - 8 * Math.cos(a + 0.45)} ${tip.y - 8 * Math.sin(a + 0.45)}`;
          group.append(label, svg('path', { d: geometry.d, class: 'line' }), svg('path', { d: arrow, class: 'tip' }));
          if ((hot || warm) && kind !== 'refers' && KINDS[kind]) {
            const word = svg('text', { x: geometry.mid.x, y: geometry.mid.y, class: 'word' });
            word.textContent = KINDS[kind].fwd;
            group.append(word);
          }
          groups.push([hot ? 2 : warm ? 1 : 0, group]);
        }
      }
      groups.sort((a, b) => a[0] - b[0]);
      layer.replaceChildren(...groups.map(([, group]) => group));
    }

    /** The Card held's neighbours not yet on the Table: kinded Links first, then plain ones, then its tree. */
    function candidates(focus) {
      const node = state.byAddress.get(focus);
      const list = [];
      const seen = new Set([focus]);
      const add = (at, way, label) => {
        if (seen.has(at) || !state.byAddress.has(at) || spreads.has(at)) return;
        seen.add(at);
        list.push({ at, way, label });
      };
      const kindOfLink = (from, to) => state.byAddress.get(from)?.kinds?.[to];
      const sources = state.inboundOf.get(focus) || [];
      for (const target of node.links) { const k = kindOfLink(focus, target); if (k && KINDS[k]) add(target, 'out', KINDS[k].fwd); }
      for (const source of sources) { const k = kindOfLink(source, focus); if (k && KINDS[k]) add(source, 'in', KINDS[k].back); }
      for (const target of node.links) add(target, 'out', 'links to');
      for (const source of sources) add(source, 'in', 'linked from');
      if (node.edge !== 'root' && node.parent) add(node.parent, 'tree', 'filed after');
      for (const child of state.childrenOf.get(focus) || []) add(child.address, 'tree', child.edge === 'side' ? 'branches to' : 'continued by');
      return list;
    }

    /**
     * Where the slips within reach lie: in the nearest gap beside the card held, right before
     * left, so they never cover a card on the Table; if no gap is near, against its right edge.
     */
    function reachOrigin(count) {
      const card = current.cards.find((item) => item.at === current.focus);
      if (!card) return null;
      const box = { w: 250, h: 24 + Math.min(count, 6) * 41 + (count > 6 ? 24 : 0), gap: 14 };
      const rects = current.cards.map((item) => ({ ...rectOf(item), w: size.w * (item.at === current.focus ? 1.07 : 1) }));
      const right = { x: card.x + size.w * 1.04 + 14, y: card.y + 6 };
      const spot = ZtLayout.freeSpot(rects, right, box);
      const far = Math.hypot(spot.x - right.x, spot.y - right.y) > size.w * 1.6;
      return far ? right : spot;
    }

    function renderReach() {
      const box = $('#reach');
      const focus = current.focus;
      const list = focus ? candidates(focus) : [];
      if (!list.length) { box.hidden = true; box.replaceChildren(); return; }
      const origin = reachOrigin(reachOpen ? list.length : Math.min(list.length, 7));
      box.hidden = false;
      box.style.transform = `translate(${origin.x}px, ${origin.y}px)`;
      const limit = reachOpen ? list.length : 6;
      const slip = (item, index) => h('button', {
        class: 'reach-slip way-' + item.way, style: `--i: ${index}`,
        title: `${item.label} ${item.at} — lay it on the Table`,
        onclick: (event) => {
          const top = event.currentTarget.offsetTop;
          pull([item.at], { near: focus, from: { x: origin.x, y: origin.y + top, angle: index % 2 ? 0.8 : -0.8 } });
          renderStage();
        },
      },
        h('span', { class: 'k', text: (item.way === 'in' ? '← ' : item.way === 'out' ? '→ ' : '') + item.label }),
        h('span', { class: 't' }, titleNodes(state.byAddress.get(item.at).title || item.at)),
        h('span', { class: 'addr', text: item.at }),
      );
      box.replaceChildren(
        h('div', { class: 'reach-head' },
          h('span', { text: `Within reach · ${list.length}` }),
          list.length > 1 ? h('button', {
            class: 'reach-all', title: 'Lay every card within reach on the Table',
            onclick: () => {
              const added = pull(list.map((item) => item.at), { near: focus, from: { x: origin.x, y: origin.y } });
              arrange('gather', focus, added);
            },
          }, 'Lay all') : null),
        ...list.slice(0, limit).map(slip),
        ...(list.length > limit ? [h('button', { class: 'reach-more', onclick: () => { reachOpen = true; renderReach(); } }, `+${list.length - limit} more`)] : []),
      );
    }

    /** The Card being read but not on the Table is in the reader's hand, ready to lay down. */
    function renderHand() {
      const hand = $('#table-hand');
      const card = state.card;
      if (!card || spreads.has(card.address)) { hand.hidden = true; return; }
      hand.hidden = false;
      hand.replaceChildren(
        h('span', { class: 'lbl', text: 'In hand' }),
        h('span', { class: 'addr', text: card.address }),
        h('span', { class: 't' }, titleNodes(card.title || '(untitled)')),
        h('span', { class: 'act' }, 'Lay it down', h('kbd', { text: 'p' })),
      );
      hand.onclick = () => {
        const from = middle();
        pull([card.address], { from: { x: from.x, y: from.y + 400 } });
        renderStage();
      };
    }

    function renderEmpty(empty) {
      empty.replaceChildren(
        h('h2', { text: spreads.now() ? `${spreads.now().name} is empty` : 'An empty Table' }),
        h('p', { text: 'Pull related cards out of the box and lay them side by side, wherever each is filed.' }),
        h('dl', {}, [
          ['p', 'lay the card you are reading on the Table — or put it back'],
          ['P', 'lay it down with every card it relates to'],
          ['Shift-click', 'a card in the Map, or + on a search hit'],
          ['within reach', 'click the card you hold: its neighbours wait beside it'],
        ].flatMap(([key, text]) => [h('dt', { text: key }), h('dd', { text })])),
      );
    }

    // ---- the hand: pick up, carry, lay down
    function grab(event, address, el) {
      if (event.button !== 0 || event.target.closest('.put-back')) return;
      event.stopPropagation();
      const card = spreads.now()?.cards.find((item) => item.at === address);
      if (!card) return;
      try { el.setPointerCapture(event.pointerId); } catch { /* a pointer the browser no longer tracks */ }
      drag = { at: address, card, sx: event.clientX, sy: event.clientY, ox: card.x, oy: card.y, moved: false, last: event.clientX, vx: 0, overBox: false };
      const box = $('#table-box');
      const move = (e) => {
        if (!drag.moved) {
          if (Math.hypot(e.clientX - drag.sx, e.clientY - drag.sy) < 4) return;
          drag.moved = true;
          el.classList.add('held');
          el.style.zIndex = 1000;
          box.classList.add('open');
          $('#reach').classList.add('away');
        }
        drag.vx = drag.vx * 0.72 + (e.clientX - drag.last) * 0.28;
        drag.last = e.clientX;
        card.x = drag.ox + (e.clientX - drag.sx) / cam.k;
        card.y = drag.oy + (e.clientY - drag.sy) / cam.k;
        // The card swings with the hand that carries it.
        place(el, card, Math.max(-9, Math.min(9, drag.vx * 0.9)));
        // Where the box stands once open (its layout box, without the slide), so a quick
        // throw lands in it too.
        const v = $('#viewport').getBoundingClientRect();
        const left = v.left + box.offsetLeft; const top = v.top + box.offsetTop;
        drag.overBox = e.clientX >= left - 10 && e.clientX <= left + box.offsetWidth + 10 && e.clientY >= top - 20;
        box.classList.toggle('over', drag.overBox);
        el.classList.toggle('to-box', drag.overBox);
        paintThreads();
      };
      const up = () => {
        el.removeEventListener('pointermove', move);
        el.removeEventListener('pointerup', up);
        el.removeEventListener('pointercancel', up);
        const done = drag;
        drag = null;
        box.classList.remove('open', 'over');
        $('#reach').classList.remove('away');
        el.classList.remove('held', 'to-box');
        if (!done.moved) { go(address); return; }
        if (done.overBox) { sendToBox(address); return; }
        // The card laid down last lies on top.
        const spread = spreads.now();
        spread.cards.splice(spread.cards.indexOf(card), 1);
        spread.cards.push(card);
        spreads.changed();
        render();
      };
      el.addEventListener('pointermove', move);
      el.addEventListener('pointerup', up);
      el.addEventListener('pointercancel', up);
    }

    // ---- camera
    let cameraFrame = 0;
    function glideCamera(target, ms) {
      const id = ++cameraFrame;
      if (document.hidden) { Object.assign(cam, target); applyCamera(); return; }
      const from = { ...cam };
      const t0 = performance.now();
      const step = (now) => {
        if (id !== cameraFrame) return;
        const t = ms <= 0 || reduceMotion() ? 1 : Math.min(1, (now - t0) / ms);
        const e = 1 - Math.pow(1 - t, 3);
        cam.x = from.x + (target.x - from.x) * e;
        cam.y = from.y + (target.y - from.y) * e;
        cam.k = from.k + (target.k - from.k) * e;
        applyCamera();
        if (t < 1) requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    }

    const STAGE_TOP = 62;
    // Below this scale a card's title is hard to read; the Table only goes there when asked.
    const LEGIBLE = 0.72;
    const reachWidth = () => (current.focus && !$('#reach').hidden ? 270 : 0);

    /**
     * Frame the Table. Asked for (`overview`), it fits every card, however small. Otherwise it
     * keeps the cards legible: if they do not all fit, it frames the card held (or the middle).
     */
    function fit(animate = true, overview = false) {
      const viewport = $('#viewport');
      const vw = viewport.clientWidth; const vh = viewport.clientHeight;
      const cards = spreads.cards();
      if (!cards.length) {
        const target = { x: vw / 2, y: STAGE_TOP + (vh - STAGE_TOP) / 2, k: 1 };
        if (animate) glideCamera(target, 320); else Object.assign(cam, target);
        return;
      }
      const xs = cards.map((card) => card.x); const ys = cards.map((card) => card.y);
      const b = { x: Math.min(...xs) - 20, y: Math.min(...ys) - 20, w: Math.max(...xs) - Math.min(...xs) + size.w + 40, h: Math.max(...ys) - Math.min(...ys) + size.h + 40 };
      const reach = reachWidth();
      const room = { w: vw - 64, h: vh - STAGE_TOP - 40 };
      const whole = Math.min(1.05, room.w / (b.w + reach), room.h / b.h);
      let target;
      if (overview || whole >= LEGIBLE) {
        const k = Math.max(0.2, whole);
        target = { k, x: 32 + (room.w - (b.w + reach) * k) / 2 - b.x * k, y: STAGE_TOP + (room.h - b.h * k) / 2 - b.y * k };
      } else {
        // Each way the cards fit, centre them; each way they do not, centre the card held,
        // but never past the last card.
        const held = cards.find((card) => card.at === current.focus);
        const k = LEGIBLE;
        const axis = (start, length, extent, low, mid) => {
          if (length * k <= extent) return low + (extent - length * k) / 2 - start * k;
          const centred = low + extent / 2 - mid * k;
          return Math.min(low - start * k, Math.max(low + extent - (start + length) * k, centred));
        };
        target = {
          k,
          x: axis(b.x, b.w + reach, room.w, 32, held ? held.x + (size.w + reach) / 2 : b.x + b.w / 2),
          y: axis(b.y, b.h, room.h, STAGE_TOP, held ? held.y + size.h / 2 : b.y + b.h / 2),
        };
      }
      if (animate) glideCamera(target, 340); else Object.assign(cam, target);
    }

    /** Bring the Card held (and what lies within its reach) into view, if it is not. */
    function keepInView(address) {
      const card = current.cards.find((item) => item.at === address);
      if (!card) return;
      const viewport = $('#viewport');
      const vw = viewport.clientWidth; const vh = viewport.clientHeight;
      const left = cam.x + card.x * cam.k; const top = cam.y + card.y * cam.k;
      const right = left + (size.w + reachWidth()) * cam.k;
      const bottom = top + size.h * cam.k;
      if (left >= 24 && right <= vw - 24 && top >= STAGE_TOP && bottom <= vh - 24) return;
      glideCamera({ k: cam.k, x: vw / 2 - (card.x + size.w / 2 + 120) * cam.k, y: STAGE_TOP + (vh - STAGE_TOP) * 0.42 - (card.y + size.h / 2) * cam.k }, 380);
    }

    /** The middle of the view, on the Table plane: where a Card goes on an empty Table. */
    function middle() {
      const viewport = $('#viewport');
      const vw = viewport.clientWidth || 800; const vh = viewport.clientHeight || 600;
      return { x: (vw / 2 - cam.x) / cam.k - size.w / 2, y: ((STAGE_TOP + vh) / 2 - cam.y) / cam.k - size.h / 2 };
    }

    /**
     * Rearrange the Cards: gathered around the one held, lined up in reading order, or stood by
     * tree. With `only`, just those Cards move (the rest stay where the reader put them). The
     * arrangement keeps the middle of the Cards it moves, and they slide one after another.
     */
    function arrange(kind, focusAt, only) {
      const spread = spreads.now();
      if (!spread) return;
      const cards = spreads.cards();
      if (!cards.length) return;
      const addresses = cards.map((card) => card.at);
      const linksOf = (at) => state.byAddress.get(at)?.links || [];
      let pos;
      if (kind === 'gather') {
        const focus = focusAt || (addresses.includes(state.pointer) ? state.pointer : addresses[addresses.length - 1]);
        const near = relatedTo(focus);
        const set = only ? [focus, ...only] : addresses;
        pos = ZtLayout.gather(set.filter((at, i) => set.indexOf(at) === i), focus, (at) => near.has(at), size);
        // Keep the Card held where it lies; its neighbours come to it.
        const held = cards.find((card) => card.at === focus);
        const at = pos.get(focus);
        if (held && at) {
          const dx = held.x - at.x; const dy = held.y - at.y;
          for (const p of pos.values()) { p.x += dx; p.y += dy; }
        }
        if (only) {
          // Only the newcomers move; put each in the free place nearest its ring spot.
          const fixed = cards.filter((card) => !only.includes(card.at)).map(rectOf);
          for (const address of only) {
            const want = pos.get(address);
            if (!want) continue;
            const spot = ZtLayout.freeSpot(fixed, want, size);
            pos.set(address, spot);
            fixed.push({ ...spot, w: size.w, h: size.h });
          }
          pos.delete(focus);
        }
      } else {
        pos = kind === 'line' ? ZtLayout.lineUp(ZtLayout.readingOrder(addresses, linksOf), size) : ZtLayout.byTree(addresses, size);
        const centre = (list) => ({ x: list.reduce((s, p) => s + p.x, 0) / list.length, y: list.reduce((s, p) => s + p.y, 0) / list.length });
        const was = centre(cards); const now = centre([...pos.values()]);
        for (const p of pos.values()) { p.x += was.x - now.x; p.y += was.y - now.y; }
        // Lined up or stood in a drawer, the cards lie in that order: the later on top.
        const rank = new Map([...pos.keys()].map((at, i) => [at, i]));
        spread.cards.sort((a, b) => (rank.get(a.at) ?? -1) - (rank.get(b.at) ?? -1));
      }
      let i = 0;
      for (const card of spread.cards) {
        const p = pos.get(card.at);
        if (!p) continue;
        card.x = p.x; card.y = p.y;
        const el = els.get(card.at);
        if (el) {
          el.style.transitionDelay = Math.min(i, 20) * 22 + 'ms';
          setTimeout(() => { el.style.transitionDelay = ''; }, 900);
        }
        i += 1;
      }
      spreads.changed();
      if (state.mode === 'table') {
        render();
        setTimeout(() => fit(true), 120);
      }
    }

    return {
      render,
      fit,
      middle,
      arrange,
      arriveFrom(address, from) { arrivals.set(address, from); },
      reset() { shown = null; reachOpen = false; },
    };
  })();

  // ---------------------------------------------------------- constellation
  // Every tree as a radial outline, the trees set around one ring (layout.js), and the
  // Links between two trees bundled into one arc whose weight is their count. Attention
  // goes to the Pointer's tree (or, at ROOT, the open Workspace); the rest recedes. Single
  // Links are drawn only for the Pointer and the hovered card.
  const sky = (() => {
    let layout = null;
    let hovered = null;
    let hoveredTree = null;
    let userMoved = false;
    const cam = camera.constellation;
    const canvas = () => $('#sky');

    const hue = (tree) => {
      const index = state.overview ? state.overview.trees.findIndex((entry) => entry.tree === tree) : -1;
      if (index >= 0) return (24 + index * 137.508) % 360;
      let hash = 0;
      for (const ch of tree) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
      return hash % 360;
    };

    function build() {
      layout = ZtLayout.constellation(state.nodes, state.overview ? state.overview.trees : []);
      layout.dots = state.nodes.map((node) => ({
        ...node, ...layout.pos.get(node.address),
        hue: hue(node.tree),
        r: node.edge === 'root' ? 5.5 : 2.3 + Math.min(node.inbound, 12) * 0.3,
      }));
      layout.byAddress = new Map(layout.dots.map((dot) => [dot.address, dot]));
      layout.byTree = new Map(layout.trees.map((tree) => [tree.id, tree]));
      layout.extent = layout.R + Math.max(...layout.trees.map((tree) => tree.r), 0) + 150;
    }

    function resize() {
      const c = canvas();
      const dpr = window.devicePixelRatio || 1;
      const w = c.clientWidth; const hh = c.clientHeight;
      if (c.width !== Math.round(w * dpr) || c.height !== Math.round(hh * dpr)) {
        c.width = Math.round(w * dpr); c.height = Math.round(hh * dpr);
      }
      return { w, h: hh, dpr };
    }

    /** A Link between two cards: a curve that bows toward the middle of the ring. */
    function arc(ctx, a, b) {
      ctx.moveTo(a.x, a.y);
      const bow = a.tree === b.tree ? 0.75 : 0.35;
      ctx.quadraticCurveTo((a.x + b.x) / 2 * bow, (a.y + b.y) / 2 * bow, b.x, b.y);
    }

    function draw() {
      if (!layout || state.mode !== 'constellation') return;
      const c = canvas();
      const { w, h: hh, dpr } = resize();
      const ctx = c.getContext('2d');
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.clearRect(0, 0, c.width, c.height);
      ctx.setTransform(dpr * cam.k, 0, 0, dpr * cam.k, dpr * (w / 2 + cam.x), dpr * (hh / 2 + cam.y));
      const px = 1 / cam.k;
      const dark = document.documentElement.dataset.theme === 'dark'
        || (!document.documentElement.dataset.theme && matchMedia('(prefers-color-scheme: dark)').matches);
      const light = dark ? 64 : 40;
      const ink = cssVar('--ink'); const faint = cssVar('--ink-faint'); const link = cssVar('--link');
      const paper = cssVar('--paper');
      const focus = state.pointer ? layout.byAddress.get(state.pointer) : null;
      const focusTree = focus ? focus.tree : null;
      const lit = (id) => {
        if (hoveredTree) return id === hoveredTree || id === focusTree;
        if (focusTree) return id === focusTree;
        return (layout.byTree.get(id).workspace || 'human') === state.workspace;
      };

      // Bundled Links between trees.
      ctx.lineCap = 'round';
      for (const bundle of layout.bundles) {
        const a = layout.byTree.get(bundle.a); const b = layout.byTree.get(bundle.b);
        const touch = [hoveredTree, focusTree].some((id) => id && (id === bundle.a || id === bundle.b));
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.quadraticCurveTo((a.x + b.x) * 0.16, (a.y + b.y) * 0.16, b.x, b.y);
        ctx.strokeStyle = touch ? link : faint;
        ctx.globalAlpha = touch ? 0.42 : (focusTree || hoveredTree ? 0.07 : 0.16);
        ctx.lineWidth = Math.max(px, 1 + 1.7 * Math.log2(1 + bundle.count));
        ctx.stroke();
      }

      // Each tree's disc and its outline edges.
      for (const tree of layout.trees) {
        const on = lit(tree.id);
        ctx.globalAlpha = on ? 0.9 : 0.5;
        ctx.beginPath();
        ctx.arc(tree.x, tree.y, tree.r, 0, Math.PI * 2);
        ctx.fillStyle = paper;
        ctx.fill();
        ctx.lineWidth = (on ? 1.4 : 1) * px;
        ctx.strokeStyle = cssVar(tree.workspace === 'agent' ? '--agent' : '--human');
        ctx.globalAlpha = on ? 0.55 : 0.2;
        ctx.stroke();
        ctx.beginPath();
        for (const [address, p] of tree.local) {
          const up = tree.outline.up.get(address);
          if (up == null) continue;
          const q = tree.local.get(up);
          ctx.moveTo(tree.x + q.x, tree.y + q.y);
          ctx.lineTo(tree.x + p.x, tree.y + p.y);
        }
        ctx.strokeStyle = `hsl(${hue(tree.id)} 40% ${light}%)`;
        ctx.globalAlpha = on ? 0.5 : 0.16;
        ctx.lineWidth = 1.1 * px;
        ctx.stroke();
      }

      // Cards.
      for (const dot of layout.dots) {
        ctx.globalAlpha = lit(dot.tree) ? 1 : 0.32;
        ctx.beginPath();
        ctx.arc(dot.x, dot.y, dot.r, 0, Math.PI * 2);
        ctx.fillStyle = `hsl(${dot.hue} ${dot.kind === 'literature' ? 62 : 52}% ${light}%)`;
        ctx.fill();
        if (dot.edge === 'root') { ctx.lineWidth = 2 * px; ctx.strokeStyle = paper; ctx.stroke(); }
      }

      // Single Links: the Pointer's in blue, the hovered card's in ink.
      const spray = (from, color, alpha) => {
        if (!from) return;
        ctx.beginPath();
        for (const target of from.links) {
          const to = layout.byAddress.get(target);
          if (to && to !== from) arc(ctx, from, to);
        }
        ctx.strokeStyle = color; ctx.globalAlpha = alpha; ctx.lineWidth = 1.3 * px;
        ctx.stroke();
        for (const target of from.links) {
          const to = layout.byAddress.get(target);
          if (!to || to === from) continue;
          ctx.beginPath();
          ctx.arc(to.x, to.y, to.r + 2.5 * px, 0, Math.PI * 2);
          ctx.lineWidth = 1.4 * px; ctx.strokeStyle = color; ctx.globalAlpha = Math.min(1, alpha + 0.2);
          ctx.stroke();
        }
      };
      spray(focus, link, 0.75);
      if (hovered && hovered !== focus) spray(hovered, ink, 0.55);
      if (focus) {
        ctx.globalAlpha = 1;
        ctx.beginPath();
        ctx.arc(focus.x, focus.y, focus.r + 5 * px, 0, Math.PI * 2);
        ctx.lineWidth = 2.5 * px;
        ctx.strokeStyle = cssVar('--pointer');
        ctx.stroke();
      }

      // Tree labels sit outside the ring, so they never cover a disc.
      ctx.textBaseline = 'middle';
      for (const tree of layout.trees) {
        const on = lit(tree.id);
        const dx = Math.cos(tree.angle); const dy = Math.sin(tree.angle);
        const x = tree.x + dx * (tree.r + 12 * px); const y = tree.y + dy * (tree.r + 12 * px);
        ctx.textAlign = Math.abs(dx) < 0.3 ? 'center' : dx > 0 ? 'left' : 'right';
        const title = tree.title || tree.id;
        const label = title.length > 18 ? title.slice(0, 17) + '…' : title;
        ctx.font = `${on ? 600 : 400} ${13 * px}px ${cssVar('--serif')}`;
        ctx.fillStyle = on ? ink : faint;
        ctx.globalAlpha = on ? 0.95 : 0.7;
        const ly = y + (Math.abs(dx) < 0.3 ? dy * 6 * px : 0);
        ctx.fillText(label, x, ly);
        ctx.font = `${10.5 * px}px ${cssVar('--mono')}`;
        ctx.fillStyle = faint;
        ctx.fillText(`${tree.id} · ${tree.count}`, x, ly + 15 * px);
      }

      // The two Workspaces, named inside the ring at the middle of their arcs.
      ctx.textAlign = 'center';
      for (const id of ['human', 'agent']) {
        const own = layout.trees.filter((tree) => (tree.workspace || 'human') === id);
        if (!own.length || !layout.R) continue;
        const sx = own.reduce((s, t) => s + Math.cos(t.angle), 0);
        const sy = own.reduce((s, t) => s + Math.sin(t.angle), 0);
        const len = Math.hypot(sx, sy) || 1;
        const inner = Math.max(40, layout.R - Math.max(...own.map((t) => t.r)) - 46);
        ctx.font = `700 ${10.5 * px}px ${cssVar('--sans')}`;
        ctx.fillStyle = cssVar(id === 'agent' ? '--agent' : '--human');
        ctx.globalAlpha = id === state.workspace ? 0.9 : 0.5;
        ctx.fillText(`${wsLabel(id).toUpperCase()} · ${own.reduce((s, t) => s + t.count, 0)}`, sx / len * inner, sy / len * inner);
      }
      ctx.globalAlpha = 1;
    }

    function screenToWorld(event) {
      const rect = canvas().getBoundingClientRect();
      return {
        x: (event.clientX - rect.left - rect.width / 2 - cam.x) / cam.k,
        y: (event.clientY - rect.top - rect.height / 2 - cam.y) / cam.k,
      };
    }

    function pick(event) {
      if (!layout) return { dot: null, tree: null };
      const p = screenToWorld(event);
      let dot = null;
      let best = (11 / cam.k) ** 2;
      for (const node of layout.dots) {
        const d = (node.x - p.x) ** 2 + (node.y - p.y) ** 2;
        if (d < Math.max(best, node.r * node.r)) { dot = node; best = d; }
      }
      const tree = layout.trees.find((t) => (t.x - p.x) ** 2 + (t.y - p.y) ** 2 <= (t.r + 6) ** 2) || null;
      return { dot, tree };
    }

    const self = {
      invalidate() { if (layout) build(); },
      show() {
        if (!layout) build();
        if (!userMoved) self.fit();
        draw();
      },
      draw,
      fit() {
        if (!layout) return;
        const c = canvas();
        cam.k = Math.max(0.15, Math.min(2.2, c.clientWidth / (2 * layout.extent), (c.clientHeight - 60) / (2 * layout.extent)));
        cam.x = 0;
        cam.y = 20;
        draw();
      },
      pan(dx, dy) { userMoved = true; cam.x += dx; cam.y += dy; draw(); },
      zoom(factor, px, py) {
        userMoved = true;
        const c = canvas();
        const ox = px - c.clientWidth / 2; const oy = py - c.clientHeight / 2;
        const k = Math.max(0.15, Math.min(6, cam.k * factor));
        cam.x = ox - (ox - cam.x) * (k / cam.k);
        cam.y = oy - (oy - cam.y) * (k / cam.k);
        cam.k = k;
        draw();
      },
      hover(event) {
        const { dot, tree } = pick(event);
        const label = $('#sky-label');
        const treeId = dot ? null : tree ? tree.id : null;
        if (dot !== hovered || treeId !== hoveredTree) { hovered = dot; hoveredTree = treeId; draw(); }
        if (!dot && !tree) { label.hidden = true; canvas().style.cursor = ''; return; }
        const rect = canvas().getBoundingClientRect();
        const at = dot || tree;
        label.hidden = false;
        label.replaceChildren(...(dot
          ? [h('span', { class: 'addr', text: dot.address + '  ' }), dot.title || '(untitled)']
          : [h('span', { class: 'addr', text: tree.id + '  ' }), `${tree.title || '(untitled)'} · ${tree.count} cards`]));
        label.style.left = (rect.width / 2 + cam.x + at.x * cam.k) + 'px';
        label.style.top = (rect.height / 2 + cam.y + (at.y - (dot ? dot.r : tree.r)) * cam.k) + 'px';
        canvas().style.cursor = 'pointer';
      },
      leave() {
        if (hovered || hoveredTree) { hovered = null; hoveredTree = null; draw(); }
        $('#sky-label').hidden = true;
      },
      click(event) {
        const { dot, tree } = pick(event);
        if (dot) go(dot.address);
        else if (tree) go(tree.root);
      },
    };
    return self;
  })();

  // ----------------------------------------------------------------- reader
  function chip(link, extraClass) {
    const cross = !link.broken && link.workspace && state.card && link.workspace !== state.card.workspace;
    return h('button', {
      class: 'chip' + (link.broken ? ' broken' : '') + (cross ? ' cross ' + link.workspace : '') + (extraClass ? ' ' + extraClass : ''),
      title: link.broken ? 'Broken link' : cross ? `${link.title} · ${wsLabel(link.workspace)} workspace` : link.title,
      onclick: link.broken ? null : () => go(link.address),
    }, h('span', { class: 'addr', text: link.address }), h('span', { class: 't' }, link.broken ? 'missing' : titleNodes(link.title || '(untitled)')));
  }

  function actionButton(label, key, handler, options = {}) {
    return h('button', {
      class: 'btn' + (options.primary ? ' primary' : '') + (options.danger ? ' danger' : '') + (options.cls ? ' ' + options.cls : ''),
      disabled: options.disabled,
      title: options.title,
      onclick: () => Promise.resolve().then(handler).catch(fail),
    }, label, key ? h('kbd', { text: key }) : null);
  }

  function renderReader() {
    const reader = $('#reader');
    const card = state.card;
    if (!card) {
      const summary = (state.overview.workspaces || []).find((item) => item.id === state.workspace)
        || { cards: 0, topic_trees: 0, literature_trees: 0 };
      reader.replaceChildren(h('div', { class: 'welcome' },
        h('h2', { text: 'ROOT' }),
        h('p', {}, h('span', { class: 'ws-tag ' + state.workspace, text: `${wsLabel(state.workspace)} workspace` })),
        h('p', { text: `${summary.cards} cards in ${summary.topic_trees} Topic trees and ${summary.literature_trees} Literature trees. Pick a stack on the desk, a drawer in the cabinet, or type a command above.` }),
        h('dl', {}, [
          ['go <target>', 'open a Location or Citation key'],
          ['t <title>', 'start a Topic'],
          ['l', 'add a Literature Card from BibTeX'],
          [': · / or Ctrl+K', 'command bar · search every card'],
          ['Alt+← →', 'back · forward through the cards you visited'],
          ['↑ ↓ →', 'walk parent · direct · side in the map'],
          ['x', 'unfold the whole tree, or fold it back around the pointer'],
          ['p · P', 'lay the card on the Table · with every card it relates to'],
          ['v', 'switch Map / Table / Constellation'],
          ['w · ws', 'switch the human / agent workspace'],
        ].flatMap(([key, text]) => [h('dt', { text: key }), h('dd', { text })]),
        ),
        h('div', { class: 'actions' },
          actionButton('New Topic', 't', () => newCard('topic', ''), { primary: true }),
          actionButton('New Literature', 'l', () => newLiterature()),
        ),
      ));
      return;
    }
    const kindLabel = { topic: 'Topic', literature: 'Literature', regular: 'Card' }[card.kind];
    const md = h('div', { class: 'md' });
    if (card.body.trim()) md.innerHTML = card.html;
    else md.append(h('p', { class: 'empty', text: 'No body yet — press e to write in Markdown.' }));
    wireMarkdown(md);
    const regular = card.context === 'regular';
    // The article is the physical index card; links and tools lie on the desk below it.
    // Back brings the card in from the left, forward from the right; one carried from the
    // search tray is already in the hand and just lands.
    const arrival = state.landing ? ' landed' : nav.step < 0 ? ' from-back' : nav.step > 0 ? ' from-fwd' : '';
    state.landing = false;
    reader.replaceChildren(h('article', { class: 'card' + arrival + ' ' + card.kind + (card.workspace === 'agent' ? ' agent-ws' : '') + (card.type && card.type !== 'idea' ? ' type-' + card.type + hubClass(card) : '') + (card.model ? ' in-model' : '') },
      card.model ? modelTabs(card) : null,
      card.type === 'hub' ? h('div', { class: 'guide-tab' + hubClass(card), title: `${(HUB_LEVELS[card.level] || {}).label || 'hub'} hub — ${(HUB_LEVELS[card.level] || {}).note || 'set its level with lv'}`, text: (HUB_LEVELS[card.level] || {}).label || 'hub' }) : null,
      h('header', { class: 'card-head' },
        h('div', { class: 'card-meta' },
          h('span', { class: 'kind-tag ' + card.kind, text: kindLabel }),
          h('span', { class: 'ws-tag ' + card.workspace, title: `${wsLabel(card.workspace)} workspace — ${(WORKSPACES[card.workspace] || WORKSPACES.human).mcp}`, text: wsLabel(card.workspace) }),
          typeStamp(card.type, null, card.facet, card.emphasis, card.level),
          h('button', {
            class: 'stamp', title: 'Copy address', text: card.address,
            onclick: () => navigator.clipboard?.writeText(card.address).then(() => toast(`copied ${card.address}`), () => {}),
          }),
        ),
        card.hubs && card.hubs.length ? h('nav', { class: 'hub-climb', 'aria-label': 'Hubs that list this card' }, card.hubs.map((hub) => h('button', {
          class: 'hub-chip' + (HUB_LEVELS[hub.level] ? ' hub-' + hub.level : ''),
          title: `${hub.level || ''} hub ${hub.address} — ${hub.title}`,
          onclick: () => go(hub.address),
        }, h('span', { class: 'lvl', text: hub.level || 'hub' }), h('span', { class: 't' }, titleNodes(hub.title || hub.address))))) : null,
        h('h1', { class: 'title' }, titleNodes(card.title || '(untitled)')),
      ),
      md,
    ), h('div', { class: 'card-tools' },
      card.model ? modelBoard(card) : null,
      card.kind === 'literature' ? h('details', { class: 'biblio' },
        h('summary', { text: `BibTeX · ${card.citation_key}` }),
        h('pre', { text: card.bibtex })) : null,
      card.inbound.length ? [h('div', { class: 'section-label', text: `Referred by · ${card.inbound.length}` }), h('div', { class: 'chips' }, card.inbound.map((link) => chip(link)))] : null,
      card.outbound.length ? [h('div', { class: 'section-label', text: `Links out · ${card.outbound.length}` }), h('div', { class: 'chips' }, card.outbound.map((link) => chip(link)))] : null,
      h('div', { class: 'actions' },
        actionButton(spreads.has(card.address) ? 'On the Table ✓' : 'To the Table', 'p', () => toggleTable(card.address), {
          title: spreads.has(card.address) ? 'Put it back in the box' : 'Lay it on the Table beside the cards you are gathering (P: with every card it relates to)',
          cls: 'table-btn',
        }),
        actionButton('Edit', 'e', editCard, { primary: true }),
        actionButton('Direct', 'n', () => newCard('direct'), { title: 'New direct successor' }),
        actionButton('Side', 'b', () => newCard('side'), { disabled: !regular, title: regular ? 'New side successor' : 'Tree roots have no side successors' }),
        actionButton('Insert', 'i', insertCard, { disabled: !regular, title: regular ? 'Insert a new card in this place; this card and its successors move down' : 'Nothing goes before a tree root' }),
        actionButton('Move', 'm', () => moveCard(), { disabled: !regular, title: regular ? 'Move with successors, alone, or into another card’s place' : 'Tree roots cannot move' }),
        actionButton('Delete', 'del', deleteCard, { danger: true }),
      ),
      h('details', { class: 'raw-toggle' }, h('summary', { text: 'Stored Markdown' }), h('pre', { text: card.text })),
    ));
    reader.scrollTop = 0;
  }

  /**
   * The model's members in build order (`build.order`: the main line from the setting, each
   * example beside its step, a shared part just before the first step that rests on it, then
   * the parts outside the build), each with its part fields and its `line`. A model read
   * without a build report keeps the order its Model Card names.
   */
  function buildOrder(model) {
    const order = model.build && model.build.order;
    if (!order) return model.parts;
    const byAddress = new Map(model.parts.map((part) => [part.address, part]));
    return order.map((entry) => ({ ...(byAddress.get(entry.address) || {}), ...entry }));
  }

  /**
   * The divider tabs along the top of a Model Card or Model part: the Model Card, then one tab
   * per part in build order, named by its facet. The open Card's tab stands up; core tabs are
   * inked, aside tabs are narrow and pale, a shared part's tab is dashed and marked ⧉, a
   * Derivation part's tab is a slim slip, and the parts outside the build stand apart.
   */
  function modelTabs(card) {
    const model = card.model;
    const head = model.head;
    const tab = (label, address, here, title, extra) => h(address ? 'button' : 'span', {
      class: 'model-tab' + (here ? ' here' : '') + (extra ? ' ' + extra : ''),
      title,
      onclick: address && !here ? () => go(address) : null,
    }, label);
    const tabs = [head
      ? tab('Model', head.address, head.address === card.address, `Model Card ${head.address} — ${head.title}`, 'head')
      : tab('Model', null, false, 'This part belongs to no model: file it under a Model Card, or link it from one', 'head missing')];
    // A long step may take a chain of Derivation parts: each tab names its place in the chain.
    const stepOf = new Map();
    for (const chain of (model.build && model.build.chains) || []) {
      if (chain.parts.length > 1) chain.parts.forEach((address, at) => stepOf.set(address, ` · derivation step ${at + 1} of ${chain.parts.length}`));
    }
    let apart = false;
    for (const part of buildOrder(model)) {
      const line = part.line || '';
      const extra = [part.emphasis, line, part.derivation ? 'derivation' : null,
        line === 'off' && !apart ? 'apart' : null].filter(Boolean).join(' ');
      if (line === 'off') apart = true;
      const where = (line === 'shared' ? ` · shared from ${part.home || 'another model'}`
        : line === 'side' ? ` · beside ${part.from}` : line === 'off' ? ' · outside the build' : '') + (stepOf.get(part.address) || '');
      tabs.push(tab((line === 'shared' ? '⧉ ' : '') + facetLabel(part.facet), part.address, part.address === card.address,
        `${facetLabel(part.facet)}${part.emphasis ? ` (${part.emphasis})` : ''}${where} · ${part.address} — ${part.title}`, extra));
    }
    return h('nav', { class: 'model-tabs', 'aria-label': 'Model structure' }, tabs);
  }

  /** A part's title without a leading `facet：` its author wrote for the Folgezettel map. */
  const bareTitle = (title) => (title || '').replace(/^[^：:$]{1,10}[：:]\s*/, '') || title || '(untitled)';

  /**
   * What a Link says, read by ZT from the words its author wrote before it (mirrors
   * `RelationKind` in src/model.rs): how it reads from the Card that writes it (`fwd`) and
   * from the Card it names (`back`). Only `rests-on` decides where a part lies on the board.
   */
  const KINDS = {
    'rests-on': { fwd: 'rests on', back: 'carries' },
    illustrates: { fwd: 'illustrates', back: 'illustrated by' },
    contrasts: { fwd: 'contrasts with', back: 'contrasted by' },
    relaxes: { fwd: 'relaxes', back: 'relaxed by' },
    corresponds: { fwd: 'corresponds to', back: 'mirrored by' },
    objects: { fwd: 'objects to', back: 'objected to by' },
    refers: { fwd: 'refers to', back: 'referred to by' },
  };
  const kindOf = (relation) => (KINDS[relation.kind] ? relation.kind : 'rests-on');

  /**
   * The model on the desk below its card. The Model Card is the way in; its parts lie below
   * as small index cards, each under the parts it rests on, with a thread for every relation:
   * a `rests on` thread runs down to the part that rests on it, and every other kind
   * (illustrates, contrasts, relaxes, corresponds, objects, refers) is a dashed arc beside the
   * two cards, named by its kind. The board is laid out for a reader's attention: the open part
   * is lifted and fully titled, the parts it relates to stay inked with their threads drawn and
   * their reasons listed, core parts stay legible, and every other part recedes to its label.
   * On the Model Card itself, the core parts lead. Loose parts lie apart in a tray. Below, the
   * Cards outside the model that the open Card relates to, and the other models a shared part
   * serves, keep the model open to the rest of the slip-box. One pass over the parts.
   */
  function modelBoard(card) {
    const model = card.model;
    if (!model.head) {
      return h('div', { class: 'model-issues' }, model.issues.map((issue) => h('p', { text: issue })));
    }
    const parts = model.parts;
    const here = card.address;
    const onHead = here === model.head.address;
    // A Derivation part carries a step between two parts: on the board it is that thread, made
    // visible; a chain of them is the thread in several short lengths.
    const derivations = new Set(((model.build && model.build.order) || []).filter((entry) => entry.derivation).map((entry) => entry.address));
    const order = new Map(parts.map((part, at) => [part.address, at]));
    const facetOf = new Map(parts.map((part) => [part.address, part.facet]));
    const loose = new Set(model.loose || []);
    const relations = (model.relations || []).filter((r) => order.has(r.from) && order.has(r.to));
    const restsOn = new Map(parts.map((part) => [part.address, []]));
    const supports = new Map(parts.map((part) => [part.address, []]));
    const besides = new Map(parts.map((part) => [part.address, []]));
    for (const relation of relations) {
      if (kindOf(relation) === 'rests-on') {
        restsOn.get(relation.from).push(relation);
        supports.get(relation.to).push(relation);
      } else {
        besides.get(relation.from).push(relation);
        besides.get(relation.to).push(relation);
      }
    }

    // Depth: one row below the deepest part a part rests on; a Link that closes a cycle is
    // left out of the depth (its thread is still drawn). Other kinds leave the rows alone.
    const depth = new Map();
    const visiting = new Set();
    const depthOf = (address) => {
      if (depth.has(address)) return depth.get(address);
      visiting.add(address);
      let d = 0;
      for (const relation of restsOn.get(address)) {
        if (!visiting.has(relation.to)) d = Math.max(d, depthOf(relation.to) + 1);
      }
      visiting.delete(address);
      depth.set(address, d);
      return d;
    };
    const rows = [];
    for (const part of parts) {
      if (!loose.has(part.address)) (rows[depthOf(part.address)] ||= []).push(part);
    }
    // Within a row, a part sits under the parts it rests on (their mean slot), else in the
    // order its Model Card names it.
    const slot = new Map();
    for (const row of rows.filter(Boolean)) {
      const key = (part) => {
        const xs = restsOn.get(part.address).map((r) => slot.get(r.to)).filter((x) => x !== undefined);
        return xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : (order.get(part.address) + 0.5) / parts.length;
      };
      const keys = new Map(row.map((part) => [part.address, key(part)]));
      row.sort((a, b) => keys.get(a.address) - keys.get(b.address) || order.get(a.address) - order.get(b.address));
      row.forEach((part, at) => slot.set(part.address, (at + 0.5) / row.length));
    }

    // Attention: what the reader has open, what it relates to, what its author called core.
    const near = new Set();
    if (!onHead) {
      for (const r of restsOn.get(here) || []) near.add(r.to);
      for (const r of supports.get(here) || []) near.add(r.from);
      for (const r of besides.get(here) || []) near.add(r.from === here ? r.to : r.from);
    }
    const level = (part) => {
      if (part.address === here) return 'here';
      if (onHead) {
        if (part.emphasis === 'core') return 'near';
        return part.emphasis === 'aside' ? 'far' : 'mid';
      }
      if (near.has(part.address)) return 'near';
      return part.emphasis === 'core' ? 'mid' : 'far';
    };
    const isHot = (relation) => (onHead
      ? level(parts[order.get(relation.from)]) === 'near' && level(parts[order.get(relation.to)]) === 'near'
      : relation.from === here || relation.to === here);

    const slips = new Map();
    const slip = (part) => {
      const at = level(part);
      // A part its Model Card took in by a Link, filed elsewhere in the slip-box.
      const joined = part.filed === false;
      const also = (part.also || []).map((other) => other.address).join(', ');
      const node = h(part.address === here ? 'div' : 'button', {
        class: `mb-slip ${at}${part.emphasis ? ' ' + part.emphasis : ''}${joined ? ' joined' : ''}${also ? ' shared' : ''}${derivations.has(part.address) ? ' derivation' : ''}`,
        title: `${facetLabel(part.facet)}${part.emphasis ? ` · ${part.emphasis}` : ''} · ${part.address}${joined ? ' (filed elsewhere, joined by Link)' : ''}${also ? `\nalso in ${also}` : ''}\n${part.title}`,
        'aria-current': part.address === here ? 'true' : null,
        onclick: part.address === here ? null : () => go(part.address),
      },
        h('span', { class: 'mb-facet', text: facetLabel(part.facet) }),
        h('span', { class: 'mb-title' }, titleNodes(bareTitle(part.title))),
      );
      slips.set(part.address, node);
      return node;
    };
    const headSlip = h(onHead ? 'div' : 'button', {
      class: 'mb-head' + (onHead ? ' here' : ''),
      title: `Model Card ${model.head.address}\n${model.head.title}`,
      onclick: onHead ? null : () => go(model.head.address),
    }, h('span', { class: 'mb-facet', text: 'Model' }), h('span', { class: 'mb-title' }, titleNodes(model.head.title)));

    const threads = svg('svg', { class: 'mb-threads', 'aria-hidden': 'true' });
    const board = h('div', { class: 'model-board' + (onHead ? ' on-head' : '') },
      threads,
      headSlip,
      rows.filter(Boolean).map((row) => h('div', { class: 'mb-row' + (row.length > 3 ? ' wide' : '') }, row.map(slip))),
      loose.size ? h('div', { class: 'mb-loose' },
        h('span', { class: 'mb-tray', text: 'Loose', title: 'Parts that relate to no other part of the model' }),
        parts.filter((part) => loose.has(part.address)).map(slip)) : null,
    );
    // A `rests on` thread runs from a part down to the parts that rest on it; any other kind,
    // or a Link against the grain, is an arc beside the two cards. Faint threads first, so the
    // ones that matter now lie on top.
    const draw = () => {
      const box = board.getBoundingClientRect();
      if (!box.width) return;
      threads.setAttribute('width', box.width);
      threads.setAttribute('height', box.height);
      threads.setAttribute('viewBox', `0 0 ${box.width} ${box.height}`);
      threads.replaceChildren();
      const place = (address) => {
        const r = slips.get(address)?.getBoundingClientRect();
        return r && { l: r.left - box.left, r: r.right - box.left, t: r.top - box.top, b: r.bottom - box.top, x: r.left - box.left + r.width / 2, y: r.top - box.top + r.height / 2 };
      };
      const drawn = relations
        .filter((r) => !loose.has(r.from) && !loose.has(r.to))
        .map((r) => ({ r, hot: isHot(r) }))
        .sort((a, b) => Number(a.hot) - Number(b.hot));
      for (const { r, hot } of drawn) {
        const kind = kindOf(r);
        const from = place(r.to);
        const to = place(r.from);
        if (!from || !to) continue;
        let d;
        let tip;
        let mark = null;
        if (kind === 'rests-on' && from.b + 4 <= to.t) {
          const mid = (from.b + to.t) / 2;
          d = `M ${from.x} ${from.b} C ${from.x} ${mid}, ${to.x} ${mid}, ${to.x} ${to.t - 2}`;
          tip = `M ${to.x - 3.5} ${to.t - 7} L ${to.x} ${to.t - 2} L ${to.x + 3.5} ${to.t - 7}`;
        } else {
          const side = Math.max(from.r, to.r) + 16;
          d = `M ${from.r} ${from.y} C ${side} ${from.y}, ${side} ${to.y}, ${to.r + 2} ${to.y}`;
          tip = `M ${to.r + 7} ${to.y - 3.5} L ${to.r + 2} ${to.y} L ${to.r + 7} ${to.y + 3.5}`;
          mark = { x: side - 4, y: (from.y + to.y) / 2 };
        }
        const group = svg('g', { class: 'mb-thread' + (kind === 'rests-on' ? '' : ' beside kind-' + kind) + (hot ? ' hot' : '') });
        const hit = svg('path', { d, class: 'hit' });
        const label = svg('title', {});
        label.textContent = `${facetLabel(facetOf.get(r.from))} ${KINDS[kind].fwd} ${facetLabel(facetOf.get(r.to))}: ${r.note}`;
        hit.append(label);
        group.append(svg('path', { d, class: 'line' }), svg('path', { d: tip, class: 'tip' }), hit);
        if (mark && hot && kind !== 'rests-on') {
          const word = svg('text', { x: mark.x, y: mark.y, class: 'word' });
          word.textContent = KINDS[kind].fwd;
          group.append(word);
        }
        threads.append(group);
      }
    };
    // Draw once the board is on the page (measuring forces layout, even in a background tab,
    // where resize notifications wait for the next rendered frame), and again whenever it
    // resizes. The threads are absolutely placed and never resize the board, so this cannot loop.
    setTimeout(draw);
    new ResizeObserver(draw).observe(board);

    // The reasons behind the open part's relations, in its own and its neighbours' words:
    // what it rests on, what rests on it, then its other relations.
    const rank = { up: 0, down: 1, side: 2 };
    const reasons = onHead ? [] : relations
      .filter((r) => r.from === here || r.to === here)
      .map((r) => {
        const kind = kindOf(r);
        const forward = r.from === here;
        return {
          way: kind === 'rests-on' ? (forward ? 'up' : 'down') : 'side',
          at: forward ? r.to : r.from,
          note: r.note,
          label: forward ? KINDS[kind].fwd : KINDS[kind].back,
        };
      })
      .sort((a, b) => rank[a.way] - rank[b.way]);
    const glyph = { up: '↑', down: '↓', side: '↔', in: '←', out: '→' };

    // Beyond the model: the Cards outside it that the open Card links or is linked by,
    // announced kinds first, the plain references after.
    const focus = onHead ? model.head.address : here;
    const beyond = (model.neighbours || [])
      .filter((n) => (n.way === 'in' ? n.to : n.from) === focus)
      .map((n, at) => ({ n, at }))
      .sort((a, b) => Number(a.n.kind === 'refers') - Number(b.n.kind === 'refers') || a.at - b.at)
      .map(({ n }) => n);
    const outsideRow = (n) => {
      const kind = KINDS[n.kind] ? n.kind : 'refers';
      return h('button', {
        class: 'mb-reason beyond ' + n.way + (kind === 'refers' ? ' plain' : ''),
        onclick: () => go(n.card.address),
        title: `${n.card.address} — ${n.card.title}\n${n.note}`,
      },
        h('span', { class: 'way', text: glyph[n.way] }),
        h('span', { class: 'mb-kind', text: n.way === 'out' ? KINDS[kind].fwd : KINDS[kind].back }),
        h('span', { class: 'note' }, titleNodes(n.card.title || n.card.address),
          n.card.type !== 'idea' ? typeStamp(n.card.type, null, n.card.facet, null, n.card.level) : null),
      );
    };
    const shown = beyond.slice(0, 6);
    const folded = beyond.slice(6);
    const others = onHead ? [] : (model.models || []).slice(1);
    const kept = parts.length - loose.size;
    return [
      h('div', { class: 'section-label', text: [
        `Model · ${parts.length} part${parts.length === 1 ? '' : 's'}`,
        relations.length ? `${relations.length} relation${relations.length === 1 ? '' : 's'}` : null,
        loose.size && kept ? `${loose.size} loose` : null,
        model.discussion ? `discussed by ${model.discussion}` : null,
      ].filter(Boolean).join(' · ') }),
      others.length ? h('div', { class: 'mb-also' },
        h('span', { class: 'mb-tray', text: 'Also in' }),
        others.map((other) => h('button', { class: 'chip', title: `Model Card ${other.address}`, onclick: () => go(other.address) }, titleNodes(other.title || other.address)))) : null,
      board,
      reasons.length ? h('div', { class: 'mb-reasons', 'aria-label': 'Why this part relates to the others' },
        reasons.map((reason) => h('button', { class: 'mb-reason ' + reason.way, onclick: () => go(reason.at), title: `${reason.label} ${reason.at}` },
          h('span', { class: 'way', text: glyph[reason.way] }),
          h('span', { class: 'mb-kind', text: reason.label }),
          h('span', { class: 'note' }, h('span', { class: 'mb-facet', text: facetLabel(facetOf.get(reason.at)) }), ' ', titleNodes(reason.note)),
        ))) : null,
      beyond.length ? h('div', { class: 'mb-reasons mb-beyond', 'aria-label': 'Cards outside the model' },
        h('div', { class: 'mb-tray', text: `Beyond the model · ${beyond.length}` }),
        shown.map(outsideRow),
        folded.length ? h('details', { class: 'mb-more' },
          h('summary', { text: `+${folded.length} more` }),
          folded.map(outsideRow)) : null) : null,
      model.issues && model.issues.length ? h('div', { class: 'model-issues', title: 'A part that belongs to no model' },
        model.issues.map((issue) => h('p', { text: issue }))) : null,
      buildFaults(model),
    ];
  }

  /**
   * What keeps a reader from building the model from its own Cards, from `build`: premises
   * outside the model, steps that rest on later steps, and steps with no path back to the
   * setting. Each fault names its fix and opens the Card it starts from. Tolerated premises of
   * shared parts are counted, not listed: they stay in their home model.
   */
  function buildFaults(model) {
    const build = model.build;
    if (!build) return null;
    const rows = [
      ...(build.closure || []).map((f) => ({ at: f.from, kind: 'outside', text: f.fix })),
      ...(build.forward || []).map((f) => ({ at: f.from, kind: 'forward', text: f.fix })),
      ...(build.unanchored || []).map((f) => ({ at: f.address, kind: 'unanchored', text: f.fix })),
    ];
    const tolerated = (build.tolerated || []).length;
    if (!rows.length) {
      return h('div', { class: 'mb-build ok', title: 'Every premise is a member, and every step rests on earlier steps' },
        `Self-sufficient${tolerated ? ` · ${tolerated} premise${tolerated === 1 ? '' : 's'} kept in a shared part’s home model` : ''}`);
    }
    return h('details', { class: 'mb-build' },
      h('summary', { text: `Build · ${rows.length} fault${rows.length === 1 ? '' : 's'}${tolerated ? ` · ${tolerated} tolerated` : ''}` }),
      rows.map((row) => h('button', { class: 'mb-reason fault ' + row.kind, onclick: () => go(row.at), title: row.at },
        h('span', { class: 'mb-kind', text: row.kind }),
        h('span', { class: 'note' }, titleNodes(row.text)))));
  }

  function wireMarkdown(root) {
    typeset(root);
    wireCharts(root);
    // Figures: a print clipped to the card. One the archive lacks recedes to a quiet gap.
    for (const figure of root.querySelectorAll('.zt-figure')) {
      const image = figure.querySelector('img');
      const lost = () => {
        figure.classList.add('missing');
        if (image) image.replaceWith(h('span', { class: 'zt-figure-gap', text: 'figure not in this archive' }));
      };
      if (!figure.dataset.artifact || !image) { lost(); continue; }
      image.addEventListener('error', lost, { once: true });
      if (image.complete && image.naturalWidth === 0 && image.src) lost();
      image.addEventListener('click', () => showFigure(figure.dataset.artifact, image.alt));
      image.title = 'Open figure';
    }
    // Link chips carry their target's title, which may hold TeX.
    for (const title of root.querySelectorAll('.zt-link-title')) {
      if (title.textContent.includes('$')) title.replaceChildren(...titleNodes(title.textContent));
    }
    for (const anchor of root.querySelectorAll('a[href]')) {
      if (anchor.classList.contains('zt-link')) {
        const targetWorkspace = nodeWorkspace(anchor.dataset.target);
        const here = state.card ? state.card.workspace : state.workspace;
        if (targetWorkspace && targetWorkspace !== here) {
          anchor.classList.add('cross', targetWorkspace);
          anchor.title = `${wsLabel(targetWorkspace)} workspace`;
        }
        anchor.addEventListener('click', (event) => { event.preventDefault(); go(anchor.dataset.target); });
      } else {
        anchor.target = '_blank';
        anchor.rel = 'noopener noreferrer';
      }
    }
  }

  function renderAll() {
    renderTopbar();
    renderRail();
    renderStage();
    renderReader();
  }

  // ---------------------------------------------------------------- dialogs
  const layers = [];

  /** Open a dialog layer; dialogs stack, so a confirmation can sit over the editor. */
  function openOverlay(dialog) {
    const overlay = $('#overlay');
    const layer = h('div', { class: 'layer' }, dialog);
    overlay.append(layer);
    overlay.hidden = false;
    layers.push(layer);
    state.editing = true;
    return () => {
      const index = layers.indexOf(layer);
      if (index < 0) return;
      layers.splice(index, 1);
      layer.remove();
      overlay.hidden = !layers.length;
      state.editing = layers.length > 0;
    };
  }

  /** Type-to-confirm, exactly like the CLI's `type ... to confirm` prompt. */
  function confirmPlan(title, lines, expected) {
    return new Promise((resolve) => {
      const input = h('input', { autocomplete: 'off', spellcheck: 'false', 'aria-label': 'Confirmation' });
      const ok = h('button', { class: 'btn primary', disabled: true, text: 'Confirm' });
      const cancel = h('button', { class: 'btn', text: 'Cancel' });
      input.addEventListener('input', () => { ok.disabled = input.value.trim() !== expected; });
      input.addEventListener('keydown', (event) => {
        if (event.key === 'Enter' && !ok.disabled) ok.click();
        if (event.key === 'Escape') cancel.click();
      });
      const close = openOverlay(h('div', { class: 'sheet-dlg narrow', role: 'dialog' },
        h('div', { class: 'dlg-head' }, h('h3', { text: title })),
        h('div', { class: 'confirm-body' },
          h('pre', { text: lines.join('\n') }),
          h('p', {}, 'Type ', h('code', { text: expected }), ' to confirm:'),
          input,
        ),
        h('div', { class: 'dlg-foot' }, h('span', { class: 'msg' }), cancel, ok),
      ));
      ok.addEventListener('click', () => { close(); resolve(true); });
      cancel.addEventListener('click', () => { close(); resolve(false); });
      input.focus();
    });
  }

  function promptValue(title, label, initial) {
    return new Promise((resolve) => {
      const input = h('input', { value: initial || '', autocomplete: 'off', spellcheck: 'false' });
      const ok = h('button', { class: 'btn primary', text: 'Continue' });
      const cancel = h('button', { class: 'btn', text: 'Cancel' });
      const close = openOverlay(h('div', { class: 'sheet-dlg narrow', role: 'dialog' },
        h('div', { class: 'dlg-head' }, h('h3', { text: title })),
        h('div', { class: 'confirm-body field' }, h('p', { text: label }), input),
        h('div', { class: 'dlg-foot' }, h('span', { class: 'msg' }), cancel, ok),
      ));
      input.addEventListener('keydown', (event) => {
        if (event.key === 'Enter') ok.click();
        if (event.key === 'Escape') cancel.click();
      });
      ok.addEventListener('click', () => { close(); resolve(input.value.trim()); });
      cancel.addEventListener('click', () => { close(); resolve(null); });
      input.focus();
      input.select();
    });
  }

  function listDialog(title, rows) {
    let close = null;
    const content = typeof rows === 'function' ? rows(() => close()) : rows;
    close = openOverlay(h('div', { class: 'sheet-dlg narrow', role: 'dialog' },
      h('div', { class: 'dlg-head' }, h('h3', { text: title })),
      h('div', { class: 'confirm-body' }, content),
      h('div', { class: 'dlg-foot' }, h('span', { class: 'msg' }), h('button', { class: 'btn primary', text: 'Close', onclick: () => close() })),
    ));
  }

  function showWorkspaces() {
    const summaries = state.overview.workspaces || [];
    listDialog('Workspaces', (close) => [
      h('div', { class: 'ws-list' }, Object.keys(WORKSPACES).map((id) => {
        const summary = summaries.find((item) => item.id === id) || { topic_trees: 0, literature_trees: 0, cards: 0 };
        return h('button', {
          class: 'ws-row ' + id + (state.workspace === id ? ' on' : ''),
          onclick: () => { close(); setWorkspace(id, { toRoot: true }); },
        },
          h('span', { class: 'ws-tag ' + id, text: wsLabel(id) }),
          h('span', { class: 't', text: `${summary.topic_trees} Topic trees · ${summary.literature_trees} Literature trees · ${summary.cards} cards` }),
          h('span', { class: 'x', text: WORKSPACES[id].mcp }),
        );
      })),
      h('p', { class: 'hint', text: 'Links may cross workspaces; cards and trees never move between them.' }),
    ]);
  }

  async function showBroken() {
    const { broken } = await api('broken');
    listDialog(`Broken links · ${broken.length}`, (close) => (broken.length
      ? h('div', { class: 'chips' }, broken.map((item) => h('button', {
        class: 'chip broken', title: item.line,
        onclick: () => { close(); go(item.source); },
      }, h('span', { class: 'addr', text: item.source }), h('span', { class: 't', text: `${item.title} → ${item.target}` }))))
      : h('p', { text: 'no broken links' })));
  }

  function showHelp() {
    const rows = COMMANDS.filter(([, , scope]) => available(scope));
    listDialog('Commands', [
      h('p', { text: 'The command bar speaks the Session’s language:' }),
      h('pre', { text: rows.map(([syntax, desc]) => `${syntax.padEnd(30)} ${desc}`).join('\n') }),
      h('p', { text: 'Keys: : command · / or Ctrl+K search · Alt+← Alt+→ back forward · g go · n b e m i · Del delete · u up · ↑↓→ walk · v view · x unfold · p table · P table with relations · w workspace · f fit · Esc close' }),
    ]);
  }

  // ---------------------------------------------------------------- figures
  /** One figure, large, over a dimmed desk: the only thing to look at. */
  function showFigure(sha, caption) {
    const source = h('p', { class: 'figure-source' });
    const close = openOverlay(h('div', { class: 'figure-view', role: 'dialog', 'aria-label': 'Figure', onclick: (event) => { if (event.target === event.currentTarget) close(); } },
      h('img', { src: '/artifact/' + sha, alt: caption || '' }),
      caption ? h('p', { class: 'figure-caption' }, titleNodes(caption)) : null,
      source,
      h('button', { class: 'btn', text: 'Close', onclick: () => close() }),
    ));
    const onKey = (event) => { if (event.key === 'Escape') { close(); document.removeEventListener('keydown', onKey, true); } };
    document.addEventListener('keydown', onKey, true);
    api('artifact?sha=' + enc(sha)).then((info) => {
      const from = info.source || {};
      if (from.work_id) source.textContent = `From Scholar Archive · Work ${from.work_id} · ${from.path || ''}`;
    }).catch(() => {});
  }

  /**
   * Choose a figure of a Scholar Archive Work and return its Markdown. The
   * search starts from the Literature tree being written in, so the likely
   * figures come first; everything else stays one search away.
   */
  function pickFigure() {
    return new Promise((resolve) => {
      const tree = state.card ? state.card.address.split('/')[0] : '';
      const rootNode = state.nodes.find((node) => node.address === tree && node.kind === 'literature');
      const query = h('input', { type: 'search', placeholder: 'Search Works by title or author', autocomplete: 'off', spellcheck: 'false', value: rootNode ? rootNode.title || '' : '' });
      const works = h('div', { class: 'figure-works' });
      const figures = h('div', { class: 'figure-grid' }, h('p', { class: 'figure-hint', text: 'Choose a Work to see its figures.' }));
      const msg = h('span', { class: 'msg' });
      let done = false;
      const finish = (markdown) => { if (done) return; done = true; close(); resolve(markdown); };
      const close = openOverlay(h('div', { class: 'sheet-dlg figure-pick', role: 'dialog' },
        h('div', { class: 'dlg-head' }, h('h3', { text: 'Figure from Scholar Archive' })),
        h('div', { class: 'figure-pick-body' }, h('div', { class: 'figure-side' }, query, works), figures),
        h('div', { class: 'dlg-foot' }, msg, h('button', { class: 'btn', text: 'Cancel', onclick: () => finish(null) })),
      ));
      let searchTicket = 0;
      const search = debounce(async () => {
        const ticket = ++searchTicket;
        try {
          const found = await api('scholar/works?q=' + enc(query.value.trim()));
          if (ticket !== searchTicket) return;
          if (!found.works.length && query.value.trim() && rootNode && query.value === rootNode.title) {
            query.value = ''; search(); return;  // No exact title match: show every Work with documents.
          }
          works.replaceChildren(...(found.works.length ? found.works.map((work) => h('button', {
            class: 'figure-work', onclick: (event) => {
              for (const other of works.querySelectorAll('.on')) other.classList.remove('on');
              event.currentTarget.classList.add('on');
              openWork(work);
            },
          }, h('span', { class: 't', text: work.title || `Work ${work.work_id}` }), h('span', { class: 'y', text: [work.year, (work.authors || '').split(',')[0]].filter(Boolean).join(' · ') })))
            : [h('p', { class: 'figure-hint', text: 'No Work with Markdown documents matches.' })]));
          if (found.works.length === 1) works.querySelector('.figure-work').click();
        } catch (error) { msg.textContent = error.message; msg.className = 'msg err'; }
      }, 220);
      let workTicket = 0;
      async function openWork(work) {
        const ticket = ++workTicket;
        figures.replaceChildren(h('p', { class: 'figure-hint', text: 'Reading figures…' }));
        try {
          const listing = await api('scholar/artifacts?work_id=' + work.work_id);
          if (ticket !== workTicket) return;
          const shown = listing.artifacts.filter((item) => item.shown && item.availability === 'present');
          const items = shown.length ? shown : listing.artifacts.filter((item) => item.availability === 'present');
          if (!items.length) { figures.replaceChildren(h('p', { class: 'figure-hint', text: 'This Work\'s Markdown shows no figures.' })); return; }
          figures.replaceChildren(...items.map((item) => {
            const frame = h('span', { class: 'figure-thumb' });
            const cell = h('button', { class: 'figure-cell', title: item.path, onclick: async () => {
              cell.disabled = true;
              try {
                const kept = await api('scholar/artifact', { file_id: item.file_id, path: item.path });
                finish(kept.markdown);
              } catch (error) { msg.textContent = error.message; msg.className = 'msg err'; cell.disabled = false; }
            } }, frame, h('span', { class: 'figure-cell-caption' }, h('b', { text: String(item.number) }), ' ', item.caption || 'no caption'));
            preview(item, frame, ticket);
            return cell;
          }));
          msg.textContent = `${items.length} figure${items.length === 1 ? '' : 's'} · choose one to show it in the Card`;
          msg.className = 'msg';
        } catch (error) { figures.replaceChildren(h('p', { class: 'figure-hint err', text: error.message })); }
      }
      // Thumbnails load one after another, so the first figures appear first.
      let queue = Promise.resolve();
      function preview(item, frame, ticket) {
        queue = queue.then(async () => {
          if (ticket !== workTicket || done) return;
          try {
            const shot = await api('scholar/preview', { file_id: item.file_id, path: item.path });
            frame.replaceChildren(h('img', { src: shot.data_url, alt: '' }));
          } catch { frame.textContent = '—'; }
        });
      }
      query.addEventListener('input', search);
      query.addEventListener('keydown', (event) => { if (event.key === 'Escape') { event.preventDefault(); finish(null); } });
      search();
      query.focus();
    });
  }

  // ----------------------------------------------------------- the editor
  /**
   * The writing desk: raw Markdown on the left, rendered on the right.
   * `save(text)` persists and returns the address to open afterwards.
   */
  function openEditor({ heading, initial, literature, bibtex, save, bibtexCheck, cardType, facet, emphasis, level }) {
    return new Promise((resolve) => {
      // Regular Cards choose a Card type; Topic and Literature Cards are always ideas.
      // A Model Card is the entry point of its model, or one of its parts when it has a facet: any
      // short label, with the common facets offered as suggestions. A part may be core or aside.
      let chosenType = cardType;
      let chosenFacet = facet || '';
      let chosenEmphasis = emphasis || '';
      let chosenLevel = level || '';
      const facetInput = cardType ? h('input', {
        class: 'facet-input', 'aria-label': 'Model facet', list: 'zt-facets', maxlength: 32, spellcheck: 'false',
        placeholder: 'whole model', value: chosenFacet,
        title: 'Empty: the Model Card of a whole model. A label (设定, 均衡, a worked example…): a Model part, which belongs to the Model Card it is filed under and to every Model Card that links it',
        oninput: (event) => { chosenFacet = event.target.value.trim(); dirty = true; emphasisPick.hidden = !chosenFacet; levelPick.hidden = !!chosenFacet; update(); },
      }) : null;
      const emphasisPick = cardType ? h('select', {
        class: 'emphasis-pick', 'aria-label': 'Emphasis', hidden: !chosenFacet,
        title: 'How much of a reader\'s attention this part asks for',
        onchange: (event) => { chosenEmphasis = event.target.value; dirty = true; update(); },
      },
        h('option', { value: '', text: 'regular' }),
        Object.entries(EMPHASIS).map(([id, info]) => h('option', { value: id, text: id, title: info.note })),
      ) : null;
      if (emphasisPick) emphasisPick.value = chosenEmphasis;
      // A whole model (no facet) carries its elaboration level instead of an emphasis.
      const levelPick = cardType ? h('select', {
        class: 'emphasis-pick level-pick', 'aria-label': 'Elaboration level', hidden: !!chosenFacet,
        title: 'How fully the model is elaborated: L1 concept (in words, no Derivation parts) or L2 formal (self-sufficient, with Derivation parts)',
        onchange: (event) => { chosenLevel = event.target.value; dirty = true; update(); },
      },
        h('option', { value: '', text: 'no level' }),
        Object.entries(LEVELS).map(([id, info]) => h('option', { value: id, text: info.label, title: info.note })),
      ) : null;
      if (levelPick) levelPick.value = chosenLevel;
      const facetPick = cardType ? h('span', { class: 'facet-pick', hidden: cardType !== 'model' },
        h('span', { class: 'lbl', text: 'Part' }), facetInput, emphasisPick, levelPick,
        h('datalist', { id: 'zt-facets' }, Object.entries(FACETS).map(([id, info]) => h('option', { value: id, label: info.note }))),
      ) : null;
      const typePick = cardType ? h('div', { class: 'type-pick', role: 'radiogroup', 'aria-label': 'Card type' },
        h('span', { class: 'lbl', text: 'Type' }),
        Object.entries(CARD_TYPES).map(([id, info]) => h('button', {
          class: id + (id === cardType ? ' on' : ''), role: 'radio', 'aria-checked': id === cardType ? 'true' : 'false',
          title: `${info.label} Card — ${info.note}`, text: info.label, dataset: { type: id },
          onclick: (event) => {
            chosenType = id;
            dirty = dirty || id !== cardType;
            for (const button of event.currentTarget.parentNode.querySelectorAll('button')) {
              button.classList.toggle('on', button.dataset.type === id);
              button.setAttribute('aria-checked', button.dataset.type === id ? 'true' : 'false');
            }
            facetPick.hidden = id !== 'model';
            update();
          },
        })),
        facetPick,
      ) : null;
      const text = h('textarea', { spellcheck: 'true', 'aria-label': 'Card Markdown' });
      text.value = initial;
      const bib = bibtex !== undefined
        ? h('textarea', { class: 'bib', spellcheck: 'false', 'aria-label': 'BibTeX', placeholder: '@book{Smith2024,\n  title = {Example Book}\n}' })
        : null;
      if (bib) bib.value = bibtex;
      const preview = h('div', { class: 'preview md' });
      const msg = h('span', { class: 'msg' });
      const saveBtn = h('button', { class: 'btn primary' }, 'Save', h('kbd', { text: 'Ctrl S' }));
      const cancelBtn = h('button', { class: 'btn', text: 'Cancel' });
      const completion = h('div', { class: 'command-menu', hidden: true });
      let bibTitle = null;
      let dirty = false;

      const setMsg = (message, kind) => { msg.textContent = message || ''; msg.className = 'msg' + (kind ? ' ' + kind : ''); };
      const update = debounce(async () => {
        try {
          if (bib) {
            if (bib.value.trim()) {
              try {
                const checked = await bibtexCheck(bib.value);
                bibTitle = checked.title;
                setMsg(checked.note || `Citation key ${checked.citation_key}`, checked.warn ? 'err' : 'ok');
              } catch (error) {
                bibTitle = null;
                setMsg(error.message, 'err');
              }
            } else { bibTitle = null; setMsg('Paste or type one BibTeX entry.'); }
          }
          const rendered = await api('render', { text: text.value, literature: !!literature });
          const title = literature ? (bibTitle ?? literature.title ?? '') : rendered.title;
          preview.innerHTML = rendered.html;
          preview.prepend(h('h1', { class: 'title' + (title ? '' : ' untitled') }, title ? titleNodes(title) : 'Untitled — start with “# Title”'));
          const stamp = chosenType && typeStamp(chosenType, null, chosenType === 'model' ? chosenFacet : null, chosenFacet ? chosenEmphasis : null, chosenFacet ? null : chosenLevel);
          if (stamp) preview.prepend(h('div', { class: 'card-meta' }, stamp));
          wireMarkdown(preview);
          if (!bib || !msg.classList.contains('err')) setMsg(rendered.error || (literature ? 'The title comes from BibTeX.' : ''), rendered.error ? 'err' : null);
        } catch (error) { setMsg(error.message, 'err'); }
      }, 160);

      const close = openOverlay(h('div', { class: 'sheet-dlg', role: 'dialog' },
        h('div', { class: 'dlg-head' }, h('h3', { text: heading }), typePick),
        h('div', { class: 'dlg-body' },
          h('div', { class: 'pane' },
            bib ? h('div', { class: 'pane-label' }, h('span', { text: 'BibTeX metadata' })) : null,
            bib,
            h('div', { class: 'pane-label' }, h('span', { text: 'Markdown' }), h('span', {},
              h('button', { class: 'figure-btn', type: 'button', title: 'Show a figure of a Scholar Archive Work in this Card', onclick: async () => {
                const markdown = await pickFigure();
                if (!markdown) { text.focus(); return; }
                const at = text.selectionStart;
                const before = text.value.slice(0, at);
                const lead = !before || before.endsWith('\n\n') ? '' : before.endsWith('\n') ? '\n' : '\n\n';
                text.setRangeText(lead + markdown + '\n\n', at, text.selectionEnd, 'end');
                text.focus();
                dirty = true;
                update();
              } }, '＋ Figure'),
              ' · [[ links · text below the reverse-links marker is generated')),
            text,
          ),
          h('div', { class: 'pane' }, h('div', { class: 'pane-label' }, h('span', { text: 'Preview' })), preview),
        ),
        h('div', { class: 'dlg-foot' }, msg, cancelBtn, saveBtn),
        completion,
      ));

      const finish = (address) => { close(); resolve(address); };
      const doSave = async () => {
        saveBtn.disabled = true;
        try {
          const address = await save(text.value, bib ? bib.value : undefined, chosenType, chosenType === 'model' ? chosenFacet : '', chosenType === 'model' && chosenFacet ? chosenEmphasis : '', chosenType === 'model' && !chosenFacet ? chosenLevel : '');
          if (address !== undefined) finish(address);
        } catch (error) {
          setMsg(error.message, 'err');
        } finally { saveBtn.disabled = false; }
      };
      const doCancel = () => {
        if (dirty && !window.confirm('Discard your changes to this card?')) return;
        finish(null);
      };
      saveBtn.addEventListener('click', doSave);
      cancelBtn.addEventListener('click', doCancel);

      // [[ link completion
      let options = [];
      let selected = 0;
      const hideCompletion = () => { completion.hidden = true; options = []; };
      const showCompletion = () => {
        const before = text.value.slice(0, text.selectionStart);
        const match = before.match(/\[\[([^\]\s]*)$/);
        if (!match) return hideCompletion();
        const query = match[1].toLowerCase();
        options = state.nodes
          .filter((node) => node.address.toLowerCase().includes(query) || (node.title || '').toLowerCase().includes(query))
          .slice(0, 8);
        if (!options.length) return hideCompletion();
        selected = Math.min(selected, options.length - 1);
        completion.replaceChildren(h('div', { class: 'group', text: 'Link to card' }), ...options.map((node, index) => h('button', {
          class: index === selected ? 'on' : '',
          onmousedown: (event) => { event.preventDefault(); accept(node, match[1].length); },
        }, h('span', { class: 'syntax', text: node.address }), h('span', { class: 'desc', text: node.title || '(untitled)' }))));
        const rect = text.getBoundingClientRect();
        completion.style.position = 'fixed';
        completion.style.left = rect.left + 16 + 'px';
        completion.style.top = 'auto';
        completion.style.bottom = (window.innerHeight - rect.bottom + 12) + 'px';
        completion.style.width = '340px';
        completion.style.right = 'auto';
        completion.hidden = false;
      };
      const accept = (node, typed) => {
        const at = text.selectionStart;
        const after = text.value.slice(at);
        const closing = after.startsWith(']]') ? '' : ']]';
        text.setRangeText(node.address + closing, at - typed, at, 'end');
        if (!closing) text.selectionStart = text.selectionEnd = at - typed + node.address.length + 2;
        hideCompletion();
        dirty = true;
        update();
      };

      text.addEventListener('input', () => { dirty = true; selected = 0; showCompletion(); update(); });
      text.addEventListener('blur', () => setTimeout(hideCompletion, 120));
      if (bib) bib.addEventListener('input', () => { dirty = true; update(); });
      const keys = (event) => {
        if (!completion.hidden && event.target === text) {
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault();
            selected = (selected + (event.key === 'ArrowDown' ? 1 : options.length - 1)) % options.length;
            showCompletion();
            return;
          }
          if (event.key === 'Enter' || event.key === 'Tab') {
            event.preventDefault();
            const match = text.value.slice(0, text.selectionStart).match(/\[\[([^\]\s]*)$/);
            accept(options[selected], match ? match[1].length : 0);
            return;
          }
          if (event.key === 'Escape') { event.preventDefault(); hideCompletion(); return; }
        }
        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') { event.preventDefault(); doSave(); }
        else if (event.key === 'Escape') { event.preventDefault(); doCancel(); }
        else if (event.key === 'Tab' && event.target.tagName === 'TEXTAREA' && !event.shiftKey) {
          event.preventDefault();
          event.target.setRangeText('  ', event.target.selectionStart, event.target.selectionEnd, 'end');
        }
      };
      text.addEventListener('keydown', keys);
      if (bib) bib.addEventListener('keydown', keys);
      update();
      (bib && !bib.value ? bib : text).focus();
      if (!bib) {
        // Put the caret after the title heading, ready to type.
        const firstLine = text.value.indexOf('\n');
        const caret = text.value.startsWith('# \n') ? 2 : firstLine < 0 ? text.value.length : Math.min(text.value.length, firstLine + 2);
        text.setSelectionRange(caret, caret);
      }
    });
  }

  // ------------------------------------------------------------- mutations
  async function afterChange(address, message) {
    if (message) toast(message);
    await refresh();
    go(address);
  }

  /** A new Card; `preset` may give its title and body, and lay it on the Table once saved. */
  async function newCard(kind, title, preset = {}) {
    if (kind !== 'topic' && !state.card) throw new Error(`${kind === 'side' ? 'b' : 'n'} needs a Card; use go <target> first`);
    const parent = state.card;
    if (kind === 'direct' && parent.direct) throw new Error(`direct successor already exists: ${parent.direct.address}`);
    if (kind === 'side' && parent.context !== 'regular') {
      throw new Error(`side successors cannot start from a ${parent.kind === 'literature' ? 'Literature' : 'Topic'} Card`);
    }
    const heading = kind === 'topic' ? `New Topic · ${wsLabel(state.workspace)} workspace`
      : kind === 'direct' ? `New direct successor of ${parent.address}` : `New side successor of ${parent.address}`;
    // Same template as the terminal: heading, blank line, body line, marker.
    const initial = `# ${kind === 'topic' ? title || '' : preset.title || ''}\n\n${preset.body ? preset.body + '\n' : ''}\n<!-- zt:reverse-links -->\n`;
    const address = await openEditor({
      heading,
      initial,
      // Every successor is a Regular Card, so it may be an Event or Model Card.
      cardType: kind === 'topic' ? null : 'idea',
      save: (text, _bibtex, type, facet, emphasis, level) => api('create', { kind, at: parent?.address, text, workspace: state.workspace, type: type || undefined, facet: facet || undefined, emphasis: emphasis || undefined, level: level || undefined }).then((result) => result.address),
    });
    if (!address) return;
    await afterChange(address, `created ${address}`);
    if (preset.onTable) { pull([address]); renderStage(); }
  }

  async function newLiterature() {
    const address = await openEditor({
      heading: `New Literature Card · ${wsLabel(state.workspace)} workspace`,
      initial: '',
      literature: {},
      bibtex: '',
      bibtexCheck: async (bibtex) => {
        const checked = await api('literature/check', { bibtex });
        if (checked.exists) return { ...checked, warn: true, note: `Literature Card \`${checked.citation_key}\` already exists` };
        return { ...checked, note: `Citation key ${checked.citation_key} · “${checked.title}”` };
      },
      save: (text, bibtex) => api('literature', { bibtex, text, workspace: state.workspace }).then((result) => result.address),
    });
    if (address) await afterChange(address, `created ${address}`);
  }

  async function editCard() {
    const card = state.card;
    if (!card) throw new Error('e needs a Card; use go <target> first');
    if (card.kind === 'literature') {
      const part = await choosePart();
      if (part === 'metadata') return editMetadata(card);
      if (part !== 'text') return;
    }
    const address = await openEditor({
      heading: `Edit ${card.address}`,
      initial: card.text,
      literature: card.kind === 'literature' ? { title: card.title } : null,
      cardType: card.context === 'regular' ? card.type || 'idea' : null,
      facet: card.facet,
      emphasis: card.emphasis,
      level: card.level,
      save: (text, _bibtex, type, facet, emphasis, level) => api('edit', {
        at: card.address, text, base: card.text, type: type || undefined,
        facet: type === 'model' ? facet || '' : undefined, emphasis: type === 'model' ? emphasis || '' : undefined,
        level: type === 'model' && !facet ? level || '' : undefined,
      }).then((result) => result.address),
    });
    if (address) await afterChange(address, `saved ${address}`);
  }

  /** `ty <idea|event|model|hub> [<facet>]`: change only the current Regular Card's type and facet. */
  async function setCardType(type, facet) {
    const card = state.card;
    if (!CARD_TYPES[type]) throw new Error(`unknown card type \`${type}\`; expected idea, event, model, or hub`);
    if (card.context !== 'regular') throw new Error('only Regular Cards can be Event, Model, or Hub Cards');
    if (facet && type !== 'model') throw new Error('only Model Cards can have a Model facet');
    const result = await api('edit', { at: card.address, type, facet: type === 'model' ? facet || '' : undefined });
    await afterChange(result.address, facet
      ? `${result.address} is the ${facetLabel(facet)} part of its model`
      : `${result.address} is ${type === 'idea' ? 'an Idea' : type === 'event' ? 'an Event' : type === 'hub' ? 'a Hub' : 'a Model'} Card`);
  }

  function choosePart() {
    return new Promise((resolve) => {
      let open = true;
      const pick = (part) => { open = false; close(); resolve(part); };
      const close = openOverlay(h('div', { class: 'sheet-dlg narrow', role: 'dialog' },
        h('div', { class: 'dlg-head' }, h('h3', { text: 'Edit Literature Card' })),
        h('div', { class: 'confirm-body' },
          h('p', { text: 'Which part do you want to edit?' }),
          h('div', { class: 'actions' },
            actionButton('Card text (Markdown)', 't', () => pick('text'), { primary: true }),
            actionButton('BibTeX metadata', 'm', () => pick('metadata')),
          ),
        ),
        h('div', { class: 'dlg-foot' }, h('span', { class: 'msg' }), h('button', { class: 'btn', text: 'Cancel', onclick: () => pick(null) })),
      ));
      const onKey = (event) => {
        if (!open) { document.removeEventListener('keydown', onKey, true); return; }
        const key = event.key.toLowerCase();
        if (key === 't' || key === 'm' || key === 'escape') {
          event.preventDefault(); event.stopPropagation();
          document.removeEventListener('keydown', onKey, true);
          pick(key === 't' ? 'text' : key === 'm' ? 'metadata' : null);
        }
      };
      document.addEventListener('keydown', onKey, true);
    });
  }

  async function editMetadata(card) {
    const address = await openEditor({
      heading: `Edit metadata · ${card.address}`,
      initial: card.text,
      literature: { title: card.title },
      bibtex: card.bibtex,
      bibtexCheck: async (bibtex) => {
        const checked = await api('literature/check', { bibtex });
        if (checked.citation_key !== card.citation_key) {
          return { ...checked, warn: checked.exists, note: checked.exists
            ? `Citation key \`${checked.citation_key}\` conflicts with an existing Literature Card`
            : `Renames ${card.citation_key} → ${checked.citation_key} (the whole tree and every Link)` };
        }
        return { ...checked, note: `Citation key ${checked.citation_key}` };
      },
      save: async (_text, bibtex) => {
        let result = await api('metadata', { at: card.address, bibtex, base: card.text });
        if (result.confirm) {
          const ok = await confirmPlan('Rename Citation key', result.lines, result.confirm);
          if (!ok) return undefined;
          result = await api('metadata', { at: card.address, bibtex, base: card.text, confirm: result.confirm });
        }
        return result.address;
      },
    });
    if (address) await afterChange(address, `saved ${address}`);
  }

  async function deleteCard() {
    const card = state.card;
    if (!card) throw new Error('del needs a Card; use go <target> first');
    const plan = await api('delete', { at: card.address });
    const ok = await confirmPlan(`Delete ${card.address}`, plan.lines, plan.confirm);
    if (!ok) return;
    const result = await api('delete', { at: card.address, confirm: plan.confirm });
    const moved = Object.entries(result.moved || {});
    await afterChange(result.address || null,
      `deleted: ${result.deleted}` + (moved.length ? `\ncompacted:\n${moved.map(([a, b]) => `${a} -> ${b}`).join('\n')}` : ''));
  }

  /**
   * Move the open Card: to a free Location (`{ to }`) or into another Card's place
   * (`{ before }`, which moves that Card down), with its successors or `alone` (its Direct
   * successors close the gap). Without a target, ask: `0/3|a`, `before 0/2`, `alone before 0/2`.
   */
  async function moveCard(target, alone = false) {
    const card = state.card;
    if (!card) throw new Error('mv needs a Card; use go <target> first');
    if (card.context !== 'regular') throw new Error(card.kind === 'literature'
      ? 'Literature Card cannot be moved; edit metadata to change its Citation key'
      : 'Topic Cards cannot be moved');
    let where = target;
    if (!where) {
      const answer = await promptValue(`Move ${card.address}`,
        'New Location, or "before <location>" to take that card’s place. Start with "alone" to leave the successors behind:', card.address);
      if (!answer) return;
      const words = answer.trim().split(/\s+/);
      alone = words[0] === 'alone';
      const rest = alone ? words.slice(1) : words;
      if (rest.length === 2 && rest[0] === 'before') where = { before: rest[1] };
      else if (rest.length === 1 && rest[0] !== 'before') where = { to: rest[0] };
      else throw new Error('expected <new-location>, before <location>, or alone before <location>');
    }
    const request = { at: card.address, ...where, alone: alone || undefined };
    const plan = await api('move', request);
    const ok = await confirmPlan(`Move ${card.address}`, plan.lines, plan.confirm);
    if (!ok) return;
    const result = await api('move', { ...request, confirm: plan.confirm });
    await afterChange(result.address, `moved: ${Object.keys(result.moved).length}`);
  }

  /**
   * Insert a new Card in the open Card's place: confirm the renumbering first (the open Card and
   * its successors move down one place, and it becomes the new Card's Direct successor), then write
   * the new Card. Its Links name today's addresses and follow the move.
   */
  async function insertCard() {
    const card = state.card;
    if (!card) throw new Error('ins needs a Card; use go <target> first');
    if (card.context !== 'regular') throw new Error('cannot insert before a tree root; insert before its Direct successor instead');
    const plan = await api('insert', { before: card.address, text: '# (new card)\n' });
    const ok = await confirmPlan(`Insert before ${card.address}`, plan.lines, plan.confirm);
    if (!ok) return;
    const inModel = card.type === 'model';
    const address = await openEditor({
      heading: `New card in the place of ${card.address} (it moves down)`,
      initial: '# \n\n\n<!-- zt:reverse-links -->\n',
      cardType: inModel ? 'model' : 'idea',
      facet: inModel && card.facet ? '推导' : '',
      emphasis: inModel && card.facet ? 'aside' : '',
      save: (text, _bibtex, type, facet, emphasis) => api('insert', {
        before: card.address, text, confirm: plan.confirm,
        type: type || undefined, facet: facet || undefined, emphasis: emphasis || undefined,
      }).then((result) => result.address),
    });
    if (address) await afterChange(address, `inserted ${address}`);
  }

  // The GUI runs inside the zt service, so closing a window stops nothing; `zt down` does.
  async function quit() {
    await spreads.flush();
    window.close();
    // A browser tab the page did not open may refuse to close.
    toast('Close this tab to leave; the GUI keeps running until `zt down`.');
  }

  // ----------------------------------------------------------- command bar
  function installCommandBar() {
    const input = $('#command-input');
    const menu = $('#command-menu');
    let items = [];
    let selected = 0;

    const suggestions = () => {
      const value = input.value;
      const trimmed = value.trimStart();
      const list = [];
      const goMatch = trimmed.match(/^go\s+(\S*)$/);
      const mvMatch = trimmed.match(/^mv\s+(\S*)$/);
      if (goMatch) {
        const query = goMatch[1].toLowerCase();
        for (const node of state.nodes) {
          if (list.length >= 9) break;
          if (node.address.toLowerCase().startsWith(query) || (query && (node.title || '').toLowerCase().includes(query))) {
            list.push({ syntax: node.address, desc: node.title || '(untitled)', run: `go ${node.address}`, group: 'Cards' });
          }
        }
      } else if (!mvMatch) {
        const word = trimmed.split(/\s/)[0] || '';
        for (const [syntax, desc, scope] of COMMANDS) {
          if (!available(scope)) continue;
          const name = syntax.split(' ')[0];
          if (!word || name.startsWith(word)) {
            const takesArg = syntax.includes('<');
            list.push({ syntax, desc, group: 'Commands', fill: takesArg && !trimmed.includes(' ') ? name + ' ' : null, run: takesArg ? null : name });
          }
        }
        if (trimmed && !list.some((item) => item.syntax.split(' ')[0] === word) && trimmed.length > 1) {
          list.push({ syntax: `search “${trimmed}”`, desc: 'find cards by text', group: 'Search', search: trimmed });
        }
      }
      return list;
    };

    const render = () => {
      items = suggestions();
      if (!items.length || document.activeElement !== input) { menu.hidden = true; return; }
      selected = Math.min(selected, items.length - 1);
      let lastGroup = null;
      const nodes = [];
      items.forEach((item, index) => {
        if (item.group !== lastGroup) { nodes.push(h('div', { class: 'group', text: item.group })); lastGroup = item.group; }
        nodes.push(h('button', {
          class: index === selected ? 'on' : '', role: 'option',
          onmousedown: (event) => { event.preventDefault(); choose(item); },
        }, h('span', { class: 'syntax', text: item.syntax }), h('span', { class: 'desc', text: item.desc })));
      });
      menu.replaceChildren(...nodes);
      menu.hidden = false;
    };

    const execute = (line) => {
      input.value = '';
      menu.hidden = true;
      input.blur();
      Promise.resolve().then(() => runCommand(line)).catch(fail);
    };

    const choose = (item) => {
      if (item.search !== undefined) {
        input.value = '';
        menu.hidden = true;
        input.blur();
        finder.open(item.search);
        return;
      }
      if (item.fill) { input.value = item.fill; selected = 0; render(); return; }
      execute(item.run || input.value);
    };

    input.addEventListener('input', () => { selected = 0; render(); });
    input.addEventListener('focus', render);
    input.addEventListener('blur', () => setTimeout(() => { menu.hidden = true; }, 100));
    input.addEventListener('keydown', (event) => {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        if (!items.length) return;
        selected = (selected + (event.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length;
        render();
      } else if (event.key === 'Tab' && items.length) {
        event.preventDefault();
        const item = items[selected];
        input.value = item.fill || item.run || input.value;
        render();
      } else if (event.key === 'Enter') {
        event.preventDefault();
        const typed = input.value.trim();
        const item = items[selected];
        const word = typed.split(/\s+/)[0];
        const exact = COMMANDS.some(([syntax]) => syntax.split(' ')[0] === word);
        if (typed && (exact || !item)) execute(typed);
        else if (item) choose(item);
      } else if (event.key === 'Escape') {
        input.value = '';
        input.blur();
      }
    });
  }

  // ------------------------------------------------------------- keyboard
  function installKeys() {
    document.addEventListener('keydown', (event) => {
      if (state.editing || finder.isOpen) return;
      // Ctrl+K searches from anywhere, as in most desktop apps.
      if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        finder.open();
        return;
      }
      // Alt+← / Alt+→ step back and forward, as in a file browser.
      if (event.altKey && !event.ctrlKey && !event.metaKey && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
        if (isTyping(event.target)) return;
        event.preventDefault();
        if (event.key === 'ArrowLeft') nav.back(); else nav.forward();
        return;
      }
      if (isTyping(event.target) || event.ctrlKey || event.metaKey || event.altKey) return;
      const card = state.card;
      const run = (fn) => { event.preventDefault(); Promise.resolve().then(fn).catch(fail); };
      switch (event.key) {
        case ':': return run(() => $('#command-input').focus());
        case '/': return run(() => finder.open());
        case 'Backspace': return run(() => nav.back());
        case 'g': return run(() => { const input = $('#command-input'); input.value = 'go '; input.focus(); input.dispatchEvent(new Event('input')); });
        case 'n': return run(() => runCommand('n'));
        case 'b': return run(() => runCommand('b'));
        case 'e': return run(() => runCommand('e'));
        case 't': return run(() => newCard('topic', ''));
        case 'l': return run(() => newLiterature());
        case 'm': return card ? run(() => moveCard()) : undefined;
        case 'i': return card && card.context === 'regular' ? run(insertCard) : undefined;
        case 'Delete': return card ? run(deleteCard) : undefined;
        case 'u': case 'ArrowUp': case 'ArrowLeft':
          return card ? run(() => go(card.parent ? card.parent.address : null)) : undefined;
        case 'ArrowDown': return card?.direct ? run(() => go(card.direct.address)) : undefined;
        case 'ArrowRight': return card?.sides.length ? run(() => go(card.sides[0].address)) : undefined;
        case 'v': return run(() => setMode(MODES[(MODES.indexOf(state.mode) + 1) % MODES.length]));
        case 'p': case 'P':
          if (!card) return undefined;
          return run(() => (event.shiftKey ? spreadAround(card.address) : toggleTable(card.address)));
        case 'w': return run(() => setWorkspace(state.workspace === 'agent' ? 'human' : 'agent', { toRoot: true }));
        case 'f': return run(() => fitView());
        case 'x': return state.card && state.mode === 'map' ? run(() => setExpandAll(!state.expandAll)) : undefined;
        case '?': return run(showHelp);
        case 'Escape': return run(() => openRail(false));
        default: return undefined;
      }
    });
  }

  // ---------------------------------------------------------------- theme
  function installTheme() {
    const saved = store.get('zt-theme');
    if (saved === 'light' || saved === 'dark') document.documentElement.dataset.theme = saved;
    $('#theme-toggle').addEventListener('click', () => {
      const current = document.documentElement.dataset.theme
        || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
      const next = current === 'dark' ? 'light' : 'dark';
      document.documentElement.dataset.theme = next;
      store.set('zt-theme', next);
      sky.draw();
      wireCharts(document);
    });
  }

  // ------------------------------------------------------------- service
  // The GUI lives inside the zt service. When the service goes down this window goes too,
  // unless the writing desk holds text: then it says so and keeps the text. When a service
  // is up again it answers with a new token, which the page takes up.
  const service = { down: false, notice: null };

  async function heartbeat() {
    let response;
    try {
      response = await fetch('/api/ping', { headers: { 'X-ZT-Token': TOKEN }, cache: 'no-store' });
    } catch {
      serviceDown();
      return;
    }
    if (response.status !== 403 && !service.down) return;
    if (!state.editing) { location.reload(); return; }
    try {
      const page = await (await fetch('/', { cache: 'no-store' })).text();
      const token = new DOMParser().parseFromString(page, 'text/html')
        .querySelector('meta[name="zt-token"]');
      if (token) TOKEN = token.content;
    } catch { return; }
    service.down = false;
    if (service.notice) service.notice.remove();
    service.notice = null;
    toast('The zt service is up again; you can save.');
  }

  function serviceDown() {
    if (service.down) return;
    service.down = true;
    if (!state.editing) window.close();
    service.notice = state.editing
      ? h('div', { class: 'service-down', role: 'alert',
        text: 'The zt service stopped. Your text is kept here: copy it, or save once `zt up` brings the service back.' })
      : h('div', { class: 'stopped' }, h('div', {},
        h('h2', { text: 'zt service stopped' }),
        h('p', { text: 'The GUI goes down with the service. This page comes back when `zt up` starts it again.' })));
    document.body.append(service.notice);
  }

  // ------------------------------------------------------------------ boot
  async function watchForChanges() {
    if (document.hidden || state.editing) return;
    try {
      const overview = await api('overview');
      if (overview.fingerprint !== state.fingerprint) await refresh();
    } catch { /* the server may be restarting */ }
  }

  async function boot() {
    installTheme();
    installSteadyHand();
    installPanZoom();
    installCommandBar();
    installKeys();
    for (const button of document.querySelectorAll('.seg [data-mode]')) {
      button.addEventListener('click', () => setMode(button.dataset.mode));
    }
    setMode(state.mode);
    $('#rail-toggle').addEventListener('click', () => openRail());
    $('#lens-toggle').addEventListener('click', () => setExpandAll(!state.expandAll));
    $('#spread-pick').addEventListener('click', () => showSpreads());
    $('#file-spread').addEventListener('click', () => fileSpread().catch(fail));
    for (const button of document.querySelectorAll('[data-arrange]')) {
      button.addEventListener('click', () => tableView.arrange(button.dataset.arrange));
    }
    $('#quit').addEventListener('click', () => quit().catch(fail));
    $('#broken-pill').addEventListener('click', () => showBroken().catch(fail));
    $('#search-open').addEventListener('click', () => finder.open());
    nav.install();
    for (const button of document.querySelectorAll('[data-action]')) {
      button.addEventListener('click', () => (button.dataset.action === 'new-topic' ? newCard('topic', '') : newLiterature()).catch(fail));
    }
    window.addEventListener('hashchange', route);
    window.addEventListener('resize', debounce(() => renderStage(), 120));
    window.addEventListener('focus', () => watchForChanges());
    setInterval(watchForChanges, 4000);
    setInterval(heartbeat, 2000);
    try {
      const [overview, graph, kept] = await Promise.all([api('overview'), api('graph'), api('spreads')]);
      state.overview = overview;
      state.fingerprint = overview.fingerprint;
      setNodes(graph.nodes);
      spreads.take(kept);
      route();
    } catch (error) {
      fail(error);
    }
  }

  boot();
})();
