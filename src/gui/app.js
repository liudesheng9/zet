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
      else if (key === 'dataset') for (const [name, item] of Object.entries(value)) { if (item != null) node.dataset[name] = item; }
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
    if (!response.ok) {
      // A Tree Lock refusal (409) keeps its structured facts: the message leads with its code, and
      // the error carries the refusal, so a dialog can name the Locked tree and keep its draft.
      const error = new Error(ZtLock.refusalText(data.refusal, data.error) || data.error || response.statusText);
      error.status = response.status;
      if (data.refusal) error.refusal = data.refusal;
      throw error;
    }
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

  /** A short note in the corner; `action` adds one button to it (`{label, run}`). */
  function toast(message, kind, action) {
    const node = h('div', { class: 'toast' + (kind === 'err' ? ' err' : kind === 'warn' ? ' warn' : ''), text: message });
    if (action) {
      node.append(h('button', {
        class: 'toast-act', type: 'button', text: action.label,
        onclick: () => { node.remove(); action.run(); },
      }));
    }
    $('#toasts').append(node);
    setTimeout(() => node.remove(), action ? 9000 : kind === 'err' || kind === 'warn' ? 6000 : 3200);
  }
  /**
   * Show an error. A write the Lock state refuses (the page's own gate, S05 #128, or a server
   * refusal) names its tree, so the note carries the way to that tree's lock panel.
   */
  const fail = (error) => {
    const tree = error && (error.gate ? error.gate.tree : error.refusal ? ZtLock.refusalTree(error.refusal) : null);
    toast((error && error.message) || String(error), 'err',
      tree ? { label: 'Lock panel', run: () => lockPanel(tree).catch(fail) } : null);
  };

  /**
   * Refuse a write the open Card's Lock state does not allow (S05, #128), before any dialog
   * opens: throws the explanation, which names the Locked tree (or the pinning trees and their
   * source Cards). Every entry point — button, key, command bar, menu — reaches its write through
   * a function that calls this, so they all honour the same state. The server still decides.
   */
  function gateWrite(card, action) {
    const gate = ZtLock.writeGate(card, action, state.locks);
    if (gate.allowed) return;
    const error = new Error(gate.why);
    error.gate = gate;
    throw error;
  }

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
    // The reader features the service has on (S28, #121): `/api/overview.features`, taken at boot
    // and on every overview read, so a switch shows on the next 4 s poll without a reload.
    features: ZtFeatures.read(null),
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
    // The last `GET /api/locks` answer (S01, #124): every Lock, its history and the integrity
    // result. Read at boot, when the lock panel opens, and after each Lock action.
    locks: null,
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

  // Card types: most Cards hold an idea; a Regular Card may record an event, state an empirical
  // finding, present a model, or open a subject.
  const CARD_TYPES = {
    idea: { label: 'Idea', note: 'one idea or claim' },
    event: { label: 'Event', note: 'one real-world event: what, when, where, who, and the source' },
    finding: { label: 'Finding', note: 'one empirical result: what was measured, the data, the method, the result, its limits, and the source' },
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
  // A Model Card may state one mathematical unit (mirrors `MathForm` in src/math.rs).
  const MATH_FORMS = {
    theory: { note: 'a Theory Card: the way into one theory; it takes in whole units in dependency order' },
    definition: { note: 'a definition: its examples, non-examples, and intuition give it meaning' },
    axiom: { note: 'an axiom: assumed, not proved' },
    theorem: { note: 'a theorem: a main result with its proof' },
    proposition: { note: 'a proposition: a result of the second rank' },
    lemma: { note: 'a lemma: a step that a proof cites; met just in time' },
    corollary: { note: 'a corollary: a result that follows from a theorem' },
  };
  /** How a facet reads: a common facet by its English name, any other label as written. */
  const facetLabel = (facet) => FACETS[facet]?.label || facet || '';
  /** The rubber stamp an Event, Finding, Model, or Hub Card carries; Idea Cards carry none. A Model part's
   *  stamp names its facet; a core part's stamp carries a dot; a Model Card's names its level. */
  const typeStamp = (type, title, facet, emphasis, level, form) => {
    if (!CARD_TYPES[type] || type === 'idea') return null;
    const part = type === 'model' && facet;
    const lvl = type === 'model' ? !part && LEVELS[level] : type === 'hub' && HUB_LEVELS[level];
    // A Model Card that states a mathematical unit names its form, then its level.
    const unit = type === 'model' && !part && MATH_FORMS[form];
    if (unit) {
      return h('span', {
        class: 'type-stamp model form-' + form + (lvl ? ' level-' + level : ''),
        title: title || `${form} — ${unit.note}${lvl ? ` · ${lvl.note}` : ''}`,
        text: lvl ? `${form} · ${lvl.label}` : form,
      });
    }
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
    ['reply', 'answer the open agent Card with a human Card: 印证/反驳/放宽/对应 then [[the agent card]]', 'agent-card'],
    ['ask [<question>]', 'ask the box: the cards that answer a question lie on a temporary Table until you Keep or Cancel them', 'always'],
    ['del', 'delete the current card and its successors', 'card'],
    ['ins', "insert a new card in the current card's place; it and its successors move down", 'regular'],
    ['mv [alone] <new-location>', 'move the current card and its successors (alone: only the card; its chain closes up)', 'regular'],
    ['mv [alone] before <location>', "move the current card into another card's place; that card moves down", 'regular'],
    ['ty <idea|event|finding|model|hub> [<facet>]', 'make the current card an Idea, Event, Finding, Model, or Hub Card; a facet (any short label, e.g. setting, dynamics, 均衡) makes it a Model part', 'regular'],
    ['lv <level|none>', 'set the level of the current Model Card (concept or formal) or Hub Card (line, topic, field, or register)', 'regular'],
    ['mf <form|none>', 'set the Math form of the current Model Card: theory, definition, axiom, theorem, proposition, lemma, or corollary', 'regular'],
    ['stats', 'count cards by kind', 'always'],
    ['status', 'show service status', 'always'],
    ['lsbk', 'list broken links', 'always'],
    ['help', 'show the commands available here', 'always'],
    ['q', 'close this window (the GUI keeps running with the service)', 'always'],
  ];
  const context = () => (!state.card ? 'root' : state.card.context === 'tree-root' ? 'tree-root' : 'regular');
  const available = (scope) =>
    scope === 'always' || (scope === 'card' && context() !== 'root') || (scope === 'regular' && context() === 'regular')
    || (scope === 'agent-card' && context() !== 'root' && !!state.card && state.card.workspace === 'agent');
  // A command of a reader feature the service has off (`ask`, `reply`) is not offered (S28, #121).
  const offered = (syntax, scope) => available(scope)
    && ZtFeatures.allows(state.features, ZtFeatures.forCommand(syntax.split(' ')[0]));

  async function runCommand(line) {
    const parts = line.trim().split(/\s+/).filter(Boolean);
    const [name, ...rest] = parts;
    if (!name) return;
    const feature = ZtFeatures.forCommand(name);
    if (!ZtFeatures.allows(state.features, feature)) throw new Error(ZtFeatures.offNote(feature));
    const needCard = () => {
      if (!state.card) throw new Error(`${name} needs a Card; use go <target> first`);
    };
    // S05 (#128): a write word on a Card whose Lock state refuses it says why before its own usage
    // check; the write's function checks the same gate again.
    if (state.card && ZtLock.COMMAND_WRITES[name]) gateWrite(state.card, ZtLock.COMMAND_WRITES[name]);
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
      case 'reply': return replyTo();
      case 'ask': return askBox(rest.join(' '));
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
        if (rest.length < 1) throw new Error('usage: ty <idea|event|finding|model|hub> [<facet>]');
        // A facet is the rest of the line, so it may hold spaces.
        return setCardType(rest[0].toLowerCase(), rest.slice(1).join(' '));
      case 'lv': {
        needCard();
        if (rest.length !== 1) throw new Error('usage: lv <level|none>');
        gateWrite(state.card, 'level');
        const none = rest[0] === 'none';
        const result = await api('edit', { at: state.card.address, level: none ? '' : rest[0] });
        return afterChange(result.address, none ? `${result.address} has no level` : `${result.address} is a ${rest[0]} ${state.card.type === 'hub' ? 'hub' : 'model'}`);
      }
      case 'mf': {
        needCard();
        if (rest.length !== 1) throw new Error('usage: mf <form|none>');
        gateWrite(state.card, 'form');
        const none = rest[0] === 'none';
        const result = await api('edit', { at: state.card.address, form: none ? '' : rest[0] });
        return afterChange(result.address, none ? `${result.address} states no mathematical unit` : `${result.address} is a ${rest[0]}`);
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
    // The Locks come with every refresh (S03, #126): a Lock, unlock, note or recorded fault changes
    // the archive revision and so the fingerprint the 4 s watch compares, even when no Card text
    // changed; reading them here repaints the cabinet, the root's lock line and the fault pill. A
    // failed lock read never stops the refresh, as at boot.
    const [overview, graph, kept, locks] = await Promise.all([api('overview'), api('graph'), api('spreads'), api('locks').catch(() => null)]);
    spreads.take(kept);
    state.overview = overview;
    state.fingerprint = overview.fingerprint;
    state.locks = locks;
    paintLocks();
    applyFeatures(overview);
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
    const filters = { ws: 'all', type: 'all', mode: 'entry' };
    const previews = new Map();
    let built = false;
    let isOpen = false;
    let data = null;
    let unfolded = new Set();
    // The rows whose folded statements are shown: a fold is a display choice of this tray, never a
    // stored view of the archive.
    const foldedOpen = new Set();
    const clearFolds = () => { unfolded = new Set(); foldedOpen.clear(); };
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
      input.addEventListener('input', () => { clearFolds(); search(); });
      input.addEventListener('keydown', onKey);
      root.addEventListener('keydown', (event) => {
        if (event.key === 'Escape' && event.target !== input) { event.preventDefault(); close(); }
      });
      built = true;
    }

    function open(text) {
      // Search turned off (`gui_search off`, S28 #121): the tray opens nothing and asks nothing.
      if (!ZtFeatures.allows(state.features, 'search')) return;
      if (!built) build();
      clearTimeout(closing);
      previews.clear();
      if (typeof text === 'string') input.value = text;
      isOpen = true;
      root.hidden = false;
      root.classList.remove('closing');
      openRail(false);
      clearFolds();
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
          const answer = await api(`search?q=${enc(words)}&ws=${filters.ws}&type=${filters.type}&mode=${filters.mode}`);
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
        onclick: () => { filters[group] = value; clearFolds(); search.now(); input.focus(); },
      }, h('span', { text: label }), n == null ? null : h('b', { text: String(n) }));
    }

    function renderChips() {
      if (!data) { chips.hidden = true; return; }
      chips.hidden = false;
      const ws = data.workspaces || {};
      const types = data.types || {};
      const sum = (counts) => Object.values(counts).reduce((total, n) => total + n, 0);
      chips.replaceChildren(
        h('div', { class: 'fgroup', role: 'group', 'aria-label': 'Search order' },
          chip('mode', 'entry', 'Entry points'),
          chip('mode', 'plain', 'All matches')),
        h('div', { class: 'fgroup', role: 'group', 'aria-label': 'Workspace' },
          chip('ws', 'all', 'All', sum(ws)),
          chip('ws', 'human', wsLabel('human'), ws.human || 0),
          chip('ws', 'agent', wsLabel('agent'), ws.agent || 0)),
        h('div', { class: 'fgroup', role: 'group', 'aria-label': 'Card type' },
          chip('type', 'all', 'Any type', sum(types)),
          ...Object.keys(CARD_TYPES).map((type) => chip('type', type, CARD_TYPES[type].label, types[type] || 0))),
      );
    }

    /** The role a ranked row names: a register entry point, a hub way in, or an ordinary hit. */
    function roleLabel(hit) {
      if (hit.role === 'entry') return hit.matched_term ? `entry · ${hit.matched_term}` : 'entry';
      if (hit.role === 'member') return 'hub line';
      if (hit.role !== 'hit') return '';
      if (hit.rank_tier === 1) return 'address';
      if (hit.rank_tier === 4) return 'model';
      if (hit.rank_tier === 5) return 'title';
      return '';
    }

    /** A slip: the top of one index card, standing in the drawer. */
    function slip(hit, words) {
      const onTable = spreads.has(hit.address);
      const title = hit.title || '(untitled)';
      const role = hit.role ? roleLabel(hit) : '';
      return h('div', {
        class: 'fslip' + (hit.workspace === 'agent' ? ' agent-ws' : '') + (hit.type && hit.type !== 'idea' ? ' type-' + hit.type : '') + (hit.role ? ' ' + hit.role : '') + (hit.folded ? ' folded' : ''),
        role: 'option', dataset: { address: hit.address },
        onclick: () => choose(hit.address),
        onmousemove: (event) => pointAt(event.currentTarget),
      },
        h('span', { class: 'fslip-head' },
          role ? h('span', { class: 'fslip-role ' + hit.role, text: role }) : null,
          h('span', { class: 'fslip-title' }, title.includes('$') ? titleNodes(title) : marked(title, words)),
          typeStamp(hit.type, null, hit.facet, hit.emphasis, hit.level, hit.form),
          h('span', { class: 'addr', text: hit.address })),
        hit.snippet ? h('span', { class: 'fslip-text' }, marked(hit.snippet, words)) : null,
        // The row shows the statement it stands for: a matching member whose canonical Card the
        // filters removed still names that Card here (D209, S14-AC05).
        hit.representative && hit.representative !== hit.address
          ? h('button', {
              class: 'fslip-canon', tabindex: '-1', role: 'option',
              dataset: { address: hit.representative },
              title: 'Open the Card that holds this statement',
              onclick: (event) => { event.stopPropagation(); choose(hit.representative); },
              onmousemove: (event) => pointAt(event.currentTarget),
            }, h('span', { text: 'canonical ' }), h('span', { class: 'addr', text: hit.representative }))
          : null,
        hit.members && hit.members.length > 1 ? foldMarker(hit, words) : null,
        hit.reason && (hit.role === 'entry' || hit.role === 'member') ? h('span', { class: 'fslip-why', text: hit.reason }) : null,
        h('button', {
          class: 'fslip-pull' + (onTable ? ' on' : ''), tabindex: '-1',
          title: onTable ? 'On the Table: click to put it back (⇧↵)' : 'Lay it on the Table (⇧↵)',
          onclick: (event) => { event.stopPropagation(); pullHit(hit.address); },
        }, onTable ? '✓' : '+'),
      );
    }

    /**
     * The `+N` one row folds: every Card that states the same confirmed statement, with what each is
     * and whether it matched. A fold hides rows, never Cards: opening a member opens that Card.
     */
    function foldMarker(hit, words) {
      const members = hit.members || [];
      const behind = Math.max(hit.folded || 0, members.length - 1);
      const open = foldedOpen.has(hit.address);
      return h('div', { class: 'fslip-fold' },
        h('button', {
          class: 'ffold fslip-fold-toggle', tabindex: '-1',
          'aria-expanded': open ? 'true' : 'false',
          dataset: { fold: hit.address },
          title: open ? 'Hide the Cards that state the same statement' : 'Show every Card that states the same statement',
          onclick: (event) => { event.stopPropagation(); toggleMembers(hit.address); },
          onmousemove: (event) => pointAt(event.currentTarget),
        }, open ? '− the same statement' : `+${behind} more stating the same statement`),
        open ? h('div', { class: 'fslip-members' },
          members.map((member) => memberLine(member, hit, words))) : null);
    }

    /** One member of a folded row: its address, its title, and what it is in the statement. */
    function memberLine(member, hit, words) {
      const tags = [member.canonical ? 'statement' : (member.statement === 'entry' ? 'entry' : 'same statement')];
      if (member.matched) tags.push(member.kept ? 'matched' : 'filtered out');
      if (member.address === hit.address) tags.push('this row');
      const title = member.title || '(untitled)';
      return h('div', {
        class: 'fslip-member' + (member.canonical ? ' canonical' : '') + (member.address === hit.address ? ' here' : ''),
        role: 'option', dataset: { address: member.address },
        onclick: (event) => { event.stopPropagation(); choose(member.address); },
        onmousemove: (event) => pointAt(event.currentTarget),
      },
        h('span', { class: 'addr', text: member.address }),
        h('span', { class: 'fslip-member-title' }, title.includes('$') ? titleNodes(title) : marked(title, words)),
        h('span', { class: 'fslip-member-tags', text: tags.join(' · ') }));
    }

    /** Show or hide the Cards behind one folded row, without asking the server again. */
    function toggleMembers(at) {
      const keep = selected;
      if (foldedOpen.has(at)) foldedOpen.delete(at);
      else foldedOpen.add(at);
      render();
      select(Math.min(keep, items.length - 1));
      input.focus();
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
        if (data.mode === 'entry' && data.register_missing) {
          nodes.push(h('p', { class: 'finder-note', text: 'No concept register in this archive: there are no entry points to rank.' }));
        }
        nodes.push(h('div', { class: 'finder-empty' },
          h('p', { class: 'big' }, 'No card holds ', h('q', { text: input.value.trim() }), '.'),
          h('p', { text: words.length > 1 ? 'Every word must appear; try fewer words.' : (filters.ws !== 'all' || filters.type !== 'all' ? 'A filter is on; try All.' : 'Try another spelling or a shorter word.') })));
      } else if (data.mode === 'entry') {
        // Ways in first: one ranked list with roles, never regrouped by tree (D210). Rows and Cards
        // are counted apart, because one row can stand for several Cards that state one statement.
        const shown = data.results.length;
        const rows = data.row_total == null ? data.total : data.row_total;
        const cards = data.total;
        const folded = data.folded_total || 0;
        count.textContent = rows > shown
          ? `best ${shown} of ${rows} rows · ${cards} cards`
          : (folded > 0 ? `${rows} rows · ${cards} cards` : `${cards} card${cards === 1 ? '' : 's'}`);
        if (data.register_missing) {
          nodes.push(h('p', { class: 'finder-note', text: 'No concept register in this archive: these are the ordinary matches, by Card title and text.' }));
        } else if ((data.register_faults || []).length) {
          const faults = data.register_faults.length;
          nodes.push(h('p', { class: 'finder-note', text: `The register has ${faults} fault${faults === 1 ? '' : 's'}; the rows they mark are left out of the entry points.` }));
        }
        nodes.push(h('section', { class: 'drawer-run ranked' }, data.results.map((hit) => slip(hit, words))));
        if (folded > 0) {
          nodes.push(h('p', { class: 'finder-note', text: `${folded} more card${folded === 1 ? '' : 's'} state a statement that a row here already holds; open the row's + to reach ${folded === 1 ? 'it' : 'them'}.` }));
        }
        if (rows > shown) nodes.push(h('p', { class: 'finder-more', text: `${rows - shown} weaker rows are not shown; add a word to narrow the search.` }));
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
              class: 'ffold', tabindex: '-1', dataset: { ffold: tree },
              onclick: () => unfold(tree), onmousemove: (event) => pointAt(event.currentTarget),
            }, `+${list.length - kept.length} more in this tree`) : null));
        });
        if (!allTrees && trees.length > TREES) {
          const rest = trees.slice(TREES);
          nodes.push(h('button', {
            class: 'ffold trees', tabindex: '-1', dataset: { ffold: '*' },
            onclick: () => unfold('*'), onmousemove: (event) => pointAt(event.currentTarget),
          }, `+${rest.length} more trees · ${rest.reduce((total, [, list]) => total + list.length, 0)} cards`));
        }
        if (data.total > shown) nodes.push(h('p', { class: 'finder-more', text: `${data.total - shown} weaker matches are not shown; add a word to narrow the search.` }));
      }
      hits.replaceChildren(...nodes);
      items = [...hits.querySelectorAll('.fslip, .ffold, .fslip-member, .fslip-canon')];
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
        const face = h('article', { class: 'card in-hand ' + card.kind + (partial ? ' partial' : '') + (filled ? ' filled' : '') + (card.workspace === 'agent' ? ' agent-ws' : '') + (card.type && card.type !== 'idea' ? ' type-' + card.type + hubClass(card) : ''), dataset: { address: card.address } },
          h('header', { class: 'card-head' },
            h('div', { class: 'card-meta' },
              h('span', { class: 'kind-tag ' + card.kind, text: kindLabel }),
              h('span', { class: 'ws-tag ' + card.workspace, text: wsLabel(card.workspace) }),
              typeStamp(card.type, null, card.facet, card.emphasis, card.level, card.form),
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
      for (const pull of hits.querySelectorAll(`.fslip[data-address="${CSS.escape(address)}"] .fslip-pull`)) {
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
        if (item.dataset.ffold) unfold(item.dataset.ffold);
        else if (item.dataset.fold) toggleMembers(item.dataset.fold);
        else if (event.shiftKey) pullHit(item.dataset.address);
        else choose(item.dataset.address);
      } else if (event.key === 'Escape') {
        event.preventDefault();
        if (input.value) { input.value = ''; clearFolds(); search.now(); } else close();
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
    // A Locked tree keeps its drawer and its place (S03, #126); its count tab says `locked` in words,
    // and a paper strap with its seal holds the drawer shut (S05, #128). Right-click, a long press,
    // Shift+F10 or the menu key opens the tree menu: Open tree, then Lock tree… or the Lock's panel,
    // note and unlock.
    const drawer = (tree) => {
      const lock = tree.locked ? ZtLock.activeLock(state.locks, tree.root) : null;
      const el = h('button', {
        class: 'drawer ' + tree.kind + (tree.locked ? ' locked' : '') + (state.card && state.card.tree === tree.tree ? ' on' : ''),
        title: (tree.locked
          ? `${tree.title || tree.root} — locked tree${lock ? ` (seal ${ZtLock.sealShort(lock.seal)})` : ''}: readable and linkable; its Cards wait for an unlock`
          : tree.title) + ' · right-click for the tree menu',
        'aria-haspopup': 'menu',
        onclick: () => { go(tree.root); openRail(false); },
      },
        h('span', { class: 'addr', text: tree.root }),
        h('span', { class: 't', text: tree.title || '(untitled)' }),
        h('span', { class: 'n', text: tree.locked ? `${tree.count} · locked` : String(tree.count) }),
        tree.excerpt ? h('span', { class: 'x', text: tree.excerpt }) : null,
        lockBand('drawer', tree, lock),
      );
      treeMenu.wire(el, tree);
      return el;
    };
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
    // While a question's proposal lies on the Table, the ask bar takes the stage row: the title and
    // the Table's tools stand down so nothing overlaps the tabs or the zoom.
    const previewing = spreads.previewing();
    $('#stage-title').hidden = onTableMode && previewing;
    $('#table-tools').hidden = !onTableMode || previewing;
    $('#viewport').classList.toggle('table-mode', onTableMode);
    $('#app').classList.toggle('at-table', onTableMode);
    if (!onTableMode) { $('#table-empty').hidden = true; $('#table-hand').hidden = true; $('#ask-bar').hidden = true; }
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
      renderAskBar();
      title.textContent = spreads.previewing()
        ? (count ? `Ask · ${count} proposed card${count === 1 ? '' : 's'} on a temporary Table` : 'Ask · nothing justified')
        : (count ? `On the Table · ${count} card${count === 1 ? '' : 's'}` : 'The Table');
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
    // A bigger tree is a thicker stack: up to four sheets under the face. A Locked tree's stack wears
    // a paper band with its seal across every sheet and the root's face (S05, #128); the band says
    // `locked` and the seal's 12 characters in words. The stack opens the same tree menu as its drawer.
    const stack = (tree) => treeMenu.wire(h('button', {
      class: 'stack ' + tree.kind + (tree.locked ? ' locked' : ''), onclick: () => go(tree.root),
      title: `${tree.title}${tree.locked ? ' — locked tree' : ''} · right-click for the tree menu`, 'aria-haspopup': 'menu',
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
            tally('finding', tree.findings),
            tally('model', tree.models),
          ),
          h('span', { class: 'gauge', title: `${tree.count} of the ${largest} cards in the largest tree` },
            h('i', { style: `width: ${Math.max(6, Math.round(100 * tree.count / largest))}%` })),
        ),
      ),
      lockBand('desk', tree, tree.locked ? ZtLock.activeLock(state.locks, tree.root) : null),
    ), tree);
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
        el.replaceChildren(...[h('span', { class: 'addr', text: short }), title, typeStamp(card.type, null, card.facet, card.emphasis, card.level, card.form), fold].filter(Boolean));
        return;
      }
      el.replaceChildren(...[
        h('span', { class: 'top' },
          h('span', { class: 'addr', text: n.lod === 'focus' ? card.address : short }),
          h('span', { class: 'badges' },
            typeStamp(card.type, null, card.facet, card.emphasis, card.level, card.form),
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
      if (event.button !== 0 || event.target.closest('.stack, .desk, .tcard, .reach, .table-hand, .table-empty, .ask-bar')) return;
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
  // them with their Cards; every change here is saved a moment later. A question's proposal is a
  // temporary Table instead (`preview`): it decks the proposed Cards without touching the saved
  // document, so Cancel puts the previous Spread back and Keep is the one action that saves.
  const spreads = (() => {
    let doc = { current: null, spreads: [] };
    let preview = null;
    let dirty = false;
    let timer = 0;
    let saving = Promise.resolve();
    const newId = () => Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
    const self = {
      get doc() { return doc; },
      now: () => preview || doc.spreads.find((spread) => spread.id === doc.current) || null,
      newId,
      /** Whether the Table is a question's temporary Table rather than a saved Spread. */
      previewing: () => !!preview,
      /** The Card a proposal holds: the temporary Table squares it up and lifts it. */
      held: () => (preview ? preview.held : null),
      /** Deck a question's proposal on the temporary Table; nothing is saved. */
      begin(proposal) {
        preview = proposal;
        renderTableCount();
        tableView.refreshReach();
      },
      /** Put the previous Spread back: the saved document was never touched. */
      end() {
        if (!preview) return;
        preview = null;
        renderTableCount();
        tableView.refreshReach();
      },
      /**
       * Keep the temporary Table as the ordinary Spread document `kept` says, and save it now. The
       * answer resolves once the archive has it; a failed save leaves the Spread on the Table to be
       * saved again with the next Table change (its local document is still the reader's).
       */
      adopt(kept) {
        preview = null;
        doc = kept && Array.isArray(kept.spreads) ? kept : { current: null, spreads: [] };
        clearTimeout(timer);
        dirty = false;
        const body = JSON.parse(JSON.stringify(doc));
        const sent = saving.catch(() => {}).then(() => api('spreads', body));
        saving = sent.then(
          (saved) => { doc = saved; },
          () => { dirty = true; },
        );
        return sent;
      },
      /** The Spread on the Table, made when the first Card is laid down. */
      ensure() {
        if (preview) return preview;
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
        // A proposal is a temporary Table: its Cards and places live in the preview, never in the
        // saved document, so nothing is written for them.
        if (preview) { renderTableCount(); return; }
        dirty = true;
        clearTimeout(timer);
        timer = setTimeout(() => self.flush(), 450);
        renderTableCount();
        // The Cards on the Table are the distant slot's exclusions: a new set is a new read.
        tableView.refreshReach();
        requestDistant();
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

  // ------------------------------------------------------------------- ask
  // `Ask the box` (S16, #116; design §9.5): one question becomes one unsaved Spread proposal —
  // the Cards the box can justify, each with its reason. They lie on a temporary Table: the held
  // Card is squared up and lifted, the others keep the reasons beside them. Keep saves one ordinary
  // Spread through the same `/api/spreads` path as every other Spread; Cancel puts the previous
  // Spread back. A failed question and a cancelled proposal write nothing, and before Keep each
  // proposed endpoint is re-read so a changed or vanished Card is a visible choice, never a silent
  // substitution.

  /** Ask for the question, unless the reader typed one after the command. */
  function askQuestionDialog(initial) {
    return new Promise((resolve) => {
      const input = h('input', {
        value: initial || '', maxlength: '512', autocomplete: 'off', spellcheck: 'false',
        'aria-label': 'What do you want to ask the box?',
      });
      const ask = h('button', { class: 'btn primary', text: 'Ask' });
      const cancel = h('button', { class: 'btn', text: 'Cancel' });
      const msg = h('span', { class: 'ask-msg' });
      const close = openOverlay(h('div', { class: 'sheet-dlg narrow', role: 'dialog', 'aria-label': 'Ask the box' },
        h('div', { class: 'dlg-head' }, h('h3', { text: 'Ask the box' })),
        h('div', { class: 'confirm-body field' },
          h('p', { text: 'Ask what you want to know. The box proposes the cards it can justify, with the reason for each, on a temporary Table — nothing is saved until you Keep.' }),
          input,
          msg),
        h('div', { class: 'dlg-foot' }, h('span', { class: 'msg' }), cancel, ask),
      ));
      const submit = () => {
        const checked = ZtAsk.askQuestion(input.value);
        if (!checked.ok) {
          msg.textContent = checked.error;
          msg.className = 'ask-msg err';
          input.focus();
          return;
        }
        close();
        resolve(checked.question);
      };
      input.addEventListener('keydown', (event) => {
        if (event.key === 'Enter') { event.preventDefault(); submit(); }
        if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(); resolve(null); }
      });
      ask.addEventListener('click', submit);
      cancel.addEventListener('click', () => { close(); resolve(null); });
      input.focus();
      input.select();
    });
  }

  /** The question's own Table: the reasons, the sparse/truncated news, and Keep/Cancel. */
  function renderAskBar() {
    const bar = $('#ask-bar');
    const preview = spreads.now();
    const on = spreads.previewing() && !!preview;
    bar.hidden = !on;
    if (!on) return;
    const terms = preview.matched_terms.length
      ? `matched ${preview.matched_terms.join(' · ')}`
      : 'no accepted term matched';
    const ambiguous = preview.ambiguous_terms.length ? ` · ${preview.ambiguous_terms.join(' / ')} has several senses` : '';
    $('#ask-q').textContent = preview.question;
    $('#ask-q').title = preview.question;
    $('#ask-terms').textContent = terms + ambiguous;
    $('#ask-terms').title = terms + ambiguous;
    const notes = [];
    if (preview.fallback) notes.push('low confidence: these share character bigrams with the question');
    if (preview.sparse) notes.push('sparse: fewer than five cards are justified');
    if (preview.truncated) notes.push('bounded: the walk stopped at its limit, so this is not the whole box');
    if (!preview.cards.length) notes.push('nothing to keep; ask differently');
    $('#ask-note').textContent = notes.join(' · ');
    $('#ask-note').title = notes.join('\n');
    $('#ask-note').hidden = !notes.length;
    $('#ask-keep').disabled = !preview.cards.length;
    $('#ask-keep').title = preview.cards.length
      ? 'Keep: save these cards as a new Spread (y)'
      : 'Nothing was justified; there is nothing to keep';
  }

  /** Ask one question and show its proposal on the temporary Table. A failed question writes nothing. */
  async function askBox(asked) {
    if (!ZtFeatures.allows(state.features, 'ask')) throw new Error(ZtFeatures.offNote('ask'));
    let question = asked == null ? '' : String(asked).trim();
    if (!question) {
      question = await askQuestionDialog('');
      if (!question) return;
    }
    const checked = ZtAsk.askQuestion(question);
    if (!checked.ok) throw new Error(checked.error);
    let proposal;
    try {
      proposal = await api('ask', { question: checked.question });
    } catch (error) {
      toast(`the question could not be answered: ${error.message || error}`, 'err');
      return;
    }
    spreads.begin(ZtAsk.askPreview(proposal));
    tableView.reset();
    setMode('table');
    setTimeout(() => tableView.fit(true), 80);
  }

  /** Cancel the proposal: the Spread the reader had is still the saved document. */
  function cancelAskSpread() {
    if (!spreads.previewing()) return;
    spreads.end();
    tableView.reset();
    renderStage();
    toast('the proposal is gone; the previous Spread is back on the Table');
  }

  /** The changed or vanished Cards of a proposal, as the reader must see them before Keep. */
  function askChangedDialog(changed) {
    return new Promise((resolve) => {
      const rows = [
        ...changed.changed.map((card) => h('li', {},
          h('span', { class: 'addr', text: card.at }), ' ',
          h('span', { class: 'then', text: card.then || '(untitled)' }), ' → ',
          h('span', { class: 'now', text: card.now || '(untitled)' }))),
        ...changed.missing.map((card) => h('li', {},
          h('span', { class: 'addr', text: card.at }), ' ',
          h('span', { class: 'then', text: card.then || '(untitled)' }), h('span', { class: 'gone', text: ' — gone' }))),
      ];
      const keep = h('button', { class: 'btn primary', text: changed.missing.length ? 'Keep the rest' : 'Keep anyway' });
      const back = h('button', { class: 'btn', text: 'Back to the Table' });
      const close = openOverlay(h('div', { class: 'sheet-dlg narrow', role: 'dialog', 'aria-label': 'The proposed cards changed' },
        h('div', { class: 'dlg-head' }, h('h3', { text: 'The cards changed since the question' })),
        h('div', { class: 'confirm-body' },
          h('p', { text: 'The question answered with these cards, but they are not what they were. Keep saves them as they are now; a card that is gone leaves the Spread. Nothing is saved until you choose.' }),
          h('ul', { class: 'ask-changed' }, rows)),
        h('div', { class: 'dlg-foot' }, h('span', { class: 'msg' }), back, keep),
      ));
      // Esc is the way back, as in every other dialog: it never keeps anything. It stops here, so
      // the page's own Esc (closing a pull-out or the cabinet) does not also fire as the dialog goes.
      const onKey = (event) => {
        if (event.key !== 'Escape') return;
        event.preventDefault();
        event.stopPropagation();
        finish(false);
      };
      const finish = (value) => {
        document.removeEventListener('keydown', onKey, true);
        close();
        resolve(value);
      };
      document.addEventListener('keydown', onKey, true);
      keep.addEventListener('click', () => finish(true));
      back.addEventListener('click', () => finish(false));
      back.focus();
    });
  }

  /**
   * Keep the proposal as one ordinary named Spread. Just before the existing save path runs, every
   * proposed endpoint is read again: an authored fingerprint that moved (or a Card that is gone)
   * is shown with its fresh context, and only the reader's visible choice keeps it.
   */
  async function keepAskSpread() {
    if (!spreads.previewing()) throw new Error('there is no proposal to keep; use ask first');
    const preview = spreads.now();
    if (!preview.cards.length) throw new Error('the question found no card to keep; cancel it and ask differently');
    const fresh = {};
    await Promise.all(preview.cards.map(async (card) => {
      try {
        fresh[card.at] = await api('card?at=' + enc(card.at));
      } catch {
        fresh[card.at] = { error: true };
      }
    }));
    let keep = preview;
    const changed = ZtAsk.askChanged(preview, fresh);
    if (changed.changed.length || changed.missing.length) {
      // Show the fresh context: the changed Cards, and the Table of what still stands.
      await refresh();
      if (!(await askChangedDialog(changed))) return;
      keep = ZtAsk.askWithout(preview, changed.missing.map((card) => card.at));
      if (!keep.cards.length) throw new Error('every proposed card is gone; ask again');
    }
    const kept = ZtAsk.askKeep(spreads.doc, keep, spreads.newId());
    try {
      await spreads.adopt(kept);
    } catch (error) {
      // The archive did not take the document: the Spread stays on the Table and a later Table
      // change saves it again, but the reader is not told it was kept.
      renderStage();
      toast(`the Spread is on the Table but the archive did not save it yet: ${error.message || error}`, 'err');
      return;
    }
    tableView.reset();
    renderStage();
    const keptSpread = spreads.now();
    toast(`kept “${keptSpread.name || 'Spread'}” · ${spreads.cards().length} card${spreads.cards().length === 1 ? '' : 's'}`);
  }

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
    if (spreads.previewing()) {
      throw new Error('the Table holds a question’s proposal: Keep or Cancel it before laying other cards');
    }
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
    if (spreads.previewing()) {
      throw new Error('the proposal is a temporary Table: Keep or Cancel it to change the Spread');
    }
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
    if (spreads.previewing()) {
      throw new Error('the Table holds a question’s proposal: Keep or Cancel it before changing the Spread');
    }
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

  /**
   * File the Table as a Card: a Spread kept from a question becomes one agent Hub Card draft
   * (S17, #118, design §9.6), and any other Spread becomes the list Card this command always wrote.
   * Neither writes anything here: the hub draft needs its own Save in the desk, and Cancel leaves
   * no Card behind.
   */
  async function fileSpread() {
    if (spreads.previewing()) {
      throw new Error('a question’s proposal is not a saved Spread yet: Keep it, then file it as a card');
    }
    const cards = spreads.cards();
    if (!cards.length) throw new Error('the Table is empty: lay some cards on it first');
    const spread = spreads.now();
    if (spread && spread.question) return fileAskSpread(spread, cards);
    if (!state.card) throw new Error('open the card to file the new card after, then try again');
    const order = ZtLayout.readingOrder(cards.map((card) => card.at), (at) => state.byAddress.get(at)?.links || []);
    const body = order.map((at) => `- [[${at}]] ${state.byAddress.get(at)?.title || ''}`.trimEnd()).join('\n');
    const kind = state.card.context === 'regular' ? 'side' : 'direct';
    await newCard(kind, '', { title: spread?.name || '', body, onTable: true });
  }

  /**
   * The reasons one kept question's answer gave its Cards, by address (S17, #118, S17-AC01). The
   * reasons are transient page state while the answer is on the Table, so a Spread kept in an
   * earlier session reads its own question again — a read that writes nothing. A failed read
   * leaves the drafted lines without reasons, which the precheck then reports.
   */
  async function askReasons(question, addresses) {
    const wanted = new Set(addresses);
    // Ask turned off: the draft keeps its lines without reasons and the precheck reports them.
    if (!ZtFeatures.allows(state.features, 'ask')) return {};
    try {
      const proposal = await api('ask', { question });
      const reasons = {};
      for (const card of proposal.cards || []) {
        if (wanted.has(card.at) && card.reason) reasons[card.at] = card.reason;
      }
      return reasons;
    } catch {
      return {};
    }
  }

  /**
   * File a Spread kept from a question as an agent Hub Card draft (S17, #118). The plan comes from
   * `POST /api/hub/draft`, which reads the archive and writes nothing: it names where the draft is
   * filed and at what level, the Markdown that opens the desk — the question, the Spread's Links,
   * and the reason the answer gave each — and every legal destination. The desk's own Save writes
   * the Card; Cancel writes nothing.
   */
  async function fileAskSpread(spread, cards) {
    const addresses = cards.map((card) => card.at);
    const reasons = await askReasons(spread.question, addresses);
    let plan;
    try {
      plan = await api('hub/draft', { cards: addresses, question: spread.question, title: spread.name, reasons });
    } catch (error) {
      toast(`the hub draft could not be made: ${error.message || error}`, 'err');
      return;
    }
    const hub = {
      level: plan.level,
      destination: plan.destination,
      choices: plan.choices || [],
      missing: plan.missing,
      note: plan.note,
      check: (text, level, how, at) => api('hub/check', { text, level, how, at }),
    };
    // The hub is filed with the one existing write route: a create as a side successor of the
    // destination, or an edit of the hub a rewrite started from, guarded by its own text.
    const address = await openEditor({
      heading: `Hub draft · ${spread.name || 'Spread'} · agent workspace`,
      initial: plan.draft.text,
      cardType: 'hub',
      hub,
      save: async (text) => {
        const placement = hub.chosen;
        if (!placement || !placement.at) throw new Error('choose where the hub is filed first');
        if (placement.how === 'edit') {
          const saved = await api('edit', { at: placement.at, text, base: placement.base });
          return saved.address;
        }
        const saved = await api('create', { kind: 'side', at: placement.at, text, type: 'hub', level: placement.level });
        return saved.address;
      },
    });
    if (!address) return;
    await afterChange(address, `filed the hub draft as ${address}`);
  }

  function showSpreads() {
    if (spreads.previewing()) { toast('the Table holds a question’s proposal: Keep or Cancel it first', 'err'); return; }
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
    let questionOpen = false;   // the kept question's slip shows every line
    const QUESTION_GAP = 14;    // between the question slip and the top row of cards
    const QUESTION_WIDTH = 640; // a readable line, however wide the Spread lies
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
            typeStamp(node.type, null, node.facet, node.emphasis, node.level, node.form),
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
      // A question's proposal holds the answer's entry Card; an ordinary Spread holds the Pointer.
      const aimed = spreads.previewing() ? spreads.held() : state.pointer;
      const focus = aimed && cards.some((card) => card.at === aimed) ? aimed : null;
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
        // A proposed Card says why the box holds it, in the excerpt's place: the explanation is
        // what the reader is judging on the temporary Table.
        const proposed = spreads.previewing();
        el.classList.toggle('proposed', proposed);
        const why = proposed ? ZtAsk.askReason(spreads.now(), card.at) : '';
        let note = el.querySelector('.why');
        if (why) {
          if (!note) { note = h('span', { class: 'why' }); el.append(note); }
          if (note.textContent !== why) note.textContent = why;
        } else if (note) note.remove();
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
      paintQuestion(cards);
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

    /**
     * A Spread kept from a question keeps that whole question on the Table (S24, #120; CONTEXT
     * "Spread"): a pencilled slip at the head of its cards, on the Table plane, so it moves with
     * them and grows upward from above the top row instead of covering one. It is a heading, not a
     * card — no stock, faint ink — so the held Card keeps the eye. A long question folds to three
     * lines and the slip unfolds it; a proposal still names its question in the ask bar instead.
     */
    function paintQuestion(cards) {
      const slip = $('#spread-question');
      const spread = spreads.now();
      const question = !spreads.previewing() && cards.length && spread && spread.question ? spread.question : '';
      if (!question) {
        slip.hidden = true;
        slip.dataset.q = '';
        return;
      }
      if (slip.dataset.q !== question) {
        slip.dataset.q = question;
        questionOpen = false;
        $('#spread-question-text').textContent = question;
        slip.title = question;
        // The question settles onto the Table as the Spread is kept or opened.
        slip.classList.remove('settling');
        void slip.offsetWidth;
        slip.classList.add('settling');
      }
      const xs = cards.map((card) => card.x);
      const left = Math.min(...xs);
      const span = Math.max(...xs) + size.w - left;
      slip.hidden = false;
      slip.style.left = left + 'px';
      slip.style.top = (Math.min(...cards.map((card) => card.y)) - QUESTION_GAP) + 'px';
      slip.style.width = Math.min(Math.max(span, size.w * 2 + size.gap), QUESTION_WIDTH) + 'px';
      slip.classList.toggle('open', questionOpen);
      slip.setAttribute('aria-expanded', String(questionOpen));
    }

    /** The room the question slip takes above the cards, in Table units (0 when none shows). */
    function questionRoom() {
      const slip = $('#spread-question');
      return slip.hidden ? 0 : slip.offsetHeight + QUESTION_GAP;
    }

    $('#spread-question').addEventListener('click', () => {
      questionOpen = !questionOpen;
      paintQuestion(spreads.cards());
    });

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
      if (spreads.previewing()) {
        toast('the proposal is a temporary Table: Keep or Cancel it to change the Spread', 'err');
        return;
      }
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
    function reachOrigin(count, extra = 0) {
      const card = current.cards.find((item) => item.at === current.focus);
      if (!card) return null;
      const box = { w: 250, h: 24 + Math.min(count, 6) * 41 + (count > 6 ? 24 : 0) + extra, gap: 14 };
      const rects = current.cards.map((item) => ({ ...rectOf(item), w: size.w * (item.at === current.focus ? 1.07 : 1) }));
      const right = { x: card.x + size.w * 1.04 + 14, y: card.y + 6 };
      const spot = ZtLayout.freeSpot(rects, right, box);
      const far = Math.hypot(spot.x - right.x, spot.y - right.y) > size.w * 1.6;
      return far ? right : spot;
    }

    function renderReach() {
      const box = $('#reach');
      // A question's proposal is a closed set: no neighbour slips, no distant slot, nothing that
      // would offer to lay another Card beside it before the reader has kept or cancelled it.
      if (spreads.previewing()) { box.hidden = true; box.replaceChildren(); return; }
      const focus = current.focus;
      const list = focus ? candidates(focus) : [];
      // The distant Card shares the box: it is another way out of the same held Card, not a tray.
      const away = focus ? !!distantValue(focus) : false;
      if (!list.length && !away) { box.hidden = true; box.replaceChildren(); return; }
      const origin = reachOrigin(reachOpen ? list.length : Math.min(list.length, 7), away ? 104 : 0);
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
          h('span', { text: list.length ? `Within reach · ${list.length}` : 'Within reach' }),
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
      // The slot sits under the slips, faint until the reader points at it or tabs into it.
      box.append(distantSlot('table'));
      paintDistant();
    }

    /** The Card being read but not on the Table is in the reader's hand, ready to lay down. */
    function renderHand() {
      const hand = $('#table-hand');
      const card = state.card;
      if (spreads.previewing() || !card || spreads.has(card.address)) { hand.hidden = true; return; }
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
      if (spreads.previewing()) {
        empty.replaceChildren(
          h('h2', { text: 'The question found nothing to keep' }),
          h('p', { text: 'The box could not justify a card from this question. Cancel it, then ask again — naming the term you mean helps.' }),
          h('dl', {}, [['Cancel', 'put the previous Spread back on the Table']]
            .flatMap(([key, text]) => [h('dt', { text: key }), h('dd', { text })])),
        );
        return;
      }
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
      // The kept question lies above the top row: the view takes it in with the cards.
      const asked = questionRoom();
      b.y -= asked;
      b.h += asked;
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
      /** The Card the Table holds (the Pointer while it lies there), for the distant slot. */
      focus() { return current.focus; },
      /** Re-draw only the slips within reach, so a distant answer arriving adds no motion. */
      refreshReach() { if (state.mode === 'table') renderReach(); },
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

  // ----------------------------------------------------------- distant card
  // One low-salience slot beside the reader's Links and the Table's neighbours (S19, #108, design
  // §10.2). Its subject is the Card the reader has open or holds: the service names at most one
  // Card of another book that shares an accepted register term or a model name, with one line of
  // evidence. It stays faint until the reader points at it or tabs into it, and it never competes
  // with the held Card's ink, place, or motion. Opening, laying, or replying writes no Link: Reply
  // opens the ordinary human draft for an agent Card, and laying a Card is the Table's own action.
  // The answer is cached per read (Card, archive fingerprint, Table) so a repaint asks nothing.
  const distant = { key: null, value: null, pending: null, poll: null };

  /** The Table's Cards and the archive read an answer belongs to. */
  function distantKey(at) {
    const lying = (spreads.now()?.cards || []).map((card) => card.at).sort().join(',');
    return [at, state.fingerprint || '', lying, bridgeReport()].join('|');
  }

  /** Ask for the open Card's one distant Card; an answer for the same read is reused. */
  function requestDistant() {
    const card = state.card;
    if (!card) {
      distant.key = null;
      distant.value = null;
      distant.pending = null;
      return;
    }
    // A tree root is navigation: it has neither the slot nor evidence to ask about.
    if (card.context !== 'regular') return;
    const key = distantKey(card.address);
    if (distant.key === key || distant.pending === key) return;
    distant.pending = key;
    const lying = spreads.cards().map((item) => item.at);
    const report = bridgeReport();
    api('distant?at=' + enc(card.address) + (lying.length ? '&table=' + lying.map(enc).join(',') : '') + (report ? '&report=' + encodeURIComponent(report) : '')).then((value) => {
      if (distant.pending !== key) return;
      distant.pending = null;
      distant.key = key;
      distant.value = value && value.at === card.address ? value : null;
      paintDistant();
      // The helper may answer after this paint: ask once more shortly while it is pending.
      if (value && value.report && value.report.status === 'pending') scheduleBridgePoll(card.address);
    }, () => {
      // An unreadable suggestion leaves the reader as it is: no slot and no message (AC07).
      if (distant.pending !== key) return;
      distant.pending = null;
      distant.key = key;
      distant.value = null;
      paintDistant();
    });
  }

  /** The distant Card of one Card, when the answer belongs to this read of it. */
  function distantValue(at) {
    const value = distant.value;
    return value && value.at === at && value.candidate ? value.candidate : null;
  }

  /** The one Card of the slot: its address and title, one line of evidence, and its actions. */
  function distantBody(at) {
    const candidate = distantValue(at);
    if (!candidate) return null;
    const source = candidate.source === 'bridge_report' ? 'bridge report'
      : candidate.source === 'model_name' ? 'model name' : 'register term';
    const agent = candidate.workspace === 'agent';
    return h('div', {
      class: 'distant', role: 'note', 'aria-label': `Distant card ${candidate.address}`,
    },
      h('div', { class: 'distant-head' },
        h('span', { class: 'lbl', text: 'Distant' }),
        h('span', { class: 'kind', text: source })),
      h('button', {
        class: 'distant-open', title: `${candidate.title} · ${candidate.address} — open it`,
        onclick: () => go(candidate.address),
      },
        h('span', { class: 't' }, titleNodes(candidate.title || '(untitled)')),
        h('span', { class: 'addr', text: candidate.address })),
      h('p', { class: 'distant-why', text: candidate.reason }),
      h('div', { class: 'distant-actions' },
        h('button', {
          class: 'distant-lay', text: 'Lay on the Table',
          title: 'Lay it beside this Card; it writes no Link',
          onclick: () => { toggleTable(candidate.address); },
        }),
        agent && ZtFeatures.allows(state.features, 'reply') ? h('button', {
          class: 'distant-reply', text: 'Reply',
          title: 'Answer this agent Card with a human Card; the draft opens empty',
          onclick: () => replyTo(candidate).catch(fail),
        }) : null),
    );
  }

  /** The slot that holds one distant Card; the two views create it, paintDistant fills it. */
  function distantSlot(where) {
    return h('div', { class: 'distant-slot', 'data-distant': where, hidden: true });
  }

  /** Fill every distant slot from the cached answer, and hide a slot that has none. */
  function paintDistant() {
    for (const slot of document.querySelectorAll('[data-distant]')) {
      const at = slot.dataset.distant === 'reader' ? state.card?.address : tableView.focus();
      const body = at ? distantBody(at) : null;
      slot.replaceChildren(...(body ? [body] : []));
      slot.hidden = !body;
    }
    paintBridgeStatus();
  }

  // ------------------------------------------------------- bridge report (S20, #112)
  // The reader may select one existing immutable Bridge report. Its exact path stays in this
  // browser session only, never in the archive: the service projects that one file read-only
  // through the installed helper, caches the answer by report hash and archive revision, and the
  // distant slot prefers its still-current proposals. A missing report, a missing helper, a
  // changed endpoint, or a timeout leaves the term or model suggestion and the Card usable, and
  // one small line says what happened. Nothing here applies a Bridge: acceptance still names the
  // report id and its exact proposal ids in the digest compiler.
  const BRIDGE_SELECTION = 'zt-bridge-report';

  /** The report path the reader selected in this browser session, or an empty string. */
  function bridgeReport() {
    try { return sessionStorage.getItem(BRIDGE_SELECTION) || ''; } catch { return ''; }
  }

  /** Keep the selected path for this browser session and read the slot again. */
  function setBridgeReport(path) {
    try {
      if (path) sessionStorage.setItem(BRIDGE_SELECTION, path);
      else sessionStorage.removeItem(BRIDGE_SELECTION);
    } catch { /* private mode */ }
    stopBridgePoll();
    distant.key = null;
    distant.value = null;
    distant.pending = null;
    paintDistant();
    requestDistant();
  }

  /** The reader chooses one existing report; an empty answer stops using one. */
  async function useBridgeReport() {
    const path = await promptValue('Use a Bridge report',
      'The exact path of an existing proposals.json. The service reads only that file and never scans a folder for a newer report. Leave it empty to stop using a report.',
      bridgeReport());
    if (path === null) return;
    setBridgeReport(path.trim());
  }

  /** The selected report's status for one Card, when the last read carries one. */
  function bridgeStatus(at) {
    const value = distant.value;
    return value && value.at === at && value.report ? value.report : null;
  }

  /** One line for the selected report: what the helper answered, and what it means for the slot. */
  function bridgeStatusText(report) {
    if (!report) return '';
    const count = report.candidates || 0;
    const first = (report.errors || [])[0];
    switch (report.status) {
      case 'pending': return 'Bridge report: reading the selected report…';
      case 'done': return `Bridge report ${report.report_id}: ${count} current proposal${count === 1 ? '' : 's'}`;
      case 'stale': return `Bridge report ${report.report_id}: its endpoints changed, so no proposal is current`;
      case 'not_in_report': return `Bridge report ${report.report_id}: no proposal names this Card`;
      case 'missing_report': return 'Bridge report not found at the selected path; showing term and model evidence.';
      case 'missing_helper': return 'Bridge report helper is not installed; showing term and model evidence.';
      case 'timeout': return 'Bridge report helper took longer than ten seconds; showing term and model evidence.';
      case 'invalid_report': return `Bridge report is not a valid immutable report${first ? ': ' + first : ''}`;
      default: return `Bridge report could not be read${first ? ': ' + first : ''}`;
    }
  }

  /** The one small line under the slot; it carries the clear action and never a candidate tray. */
  function bridgeStatusSlot() {
    return h('div', { class: 'bridge-status', 'data-bridge-status': 'reader', role: 'status', 'aria-live': 'polite', hidden: true });
  }

  /** Fill the report status line; the helper may answer after the slot was first painted. */
  function paintBridgeStatus() {
    for (const slot of document.querySelectorAll('[data-bridge-status]')) {
      const at = state.card?.address;
      const text = at ? bridgeStatusText(bridgeStatus(at)) : '';
      slot.hidden = !text;
      slot.replaceChildren(...(text ? [
        h('span', { class: 't', text }),
        h('button', {
          class: 'bridge-clear', title: 'Stop using this Bridge report; nothing in the archive changes',
          text: '×', onclick: () => setBridgeReport(''),
        }),
      ] : []));
    }
  }

  /** Stop a scheduled re-read of a pending helper answer. */
  function stopBridgePoll() {
    if (distant.poll) clearTimeout(distant.poll);
    distant.poll = null;
  }

  /** Ask once more shortly while the installed helper is still working; never per repaint. */
  function scheduleBridgePoll(at) {
    stopBridgePoll();
    distant.poll = setTimeout(() => {
      distant.poll = null;
      if (!state.card || state.card.address !== at || !bridgeReport()) return;
      const report = bridgeStatus(at);
      if (!report || report.status !== 'pending') return;
      distant.key = null;
      requestDistant();
    }, 700);
  }

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
          ['s', 'study a mathematical unit on the desk, one station at a time'],
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
    wireUnitReader(md, card);
    const regular = card.context === 'regular';
    // S05 (#128): a write the Card's Lock state refuses stays in its place but disabled, its title
    // naming the Locked tree (or the pinning trees and their source Cards); the lock line under
    // the actions says it in words and offers the lock panel. Reading, linking, the Table and
    // Reply are never gated.
    const writeOpts = (action, base = {}) => {
      const gate = ZtLock.writeGate(card, action, state.locks);
      return gate.allowed ? base : { ...base, disabled: true, title: gate.why, cls: [base.cls, 'gated'].filter(Boolean).join(' ') };
    };
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
          typeStamp(card.type, null, card.facet, card.emphasis, card.level, card.form),
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
      isTheory(card.model) ? mathTools(card) : null,
      card.model && !(isTheory(card.model) && !card.model.parts.length) ? modelBoard(card) : null,
      isUnit(card.model) ? mathTools(card) : null,
      card.kind === 'literature' ? h('details', { class: 'biblio' },
        h('summary', { text: `BibTeX · ${card.citation_key}` }),
        h('pre', { text: card.bibtex })) : null,
      card.inbound.length ? [h('div', { class: 'section-label', text: `Referred by · ${card.inbound.length}` }), h('div', { class: 'chips' }, card.inbound.map((link) => chip(link)))] : null,
      card.outbound.length ? [h('div', { class: 'section-label', text: `Links out · ${card.outbound.length}` }), h('div', { class: 'chips' }, card.outbound.map((link) => chip(link)))] : null,
      // One distant Card beside the neighbours, faint until focus or hover (S19, #108), and the
      // selected Bridge report's one-line status (S20, #112).
      card.context === 'regular' ? distantSlot('reader') : null,
      card.context === 'regular' ? bridgeStatusSlot() : null,
      h('div', { class: 'actions' },
        actionButton(spreads.has(card.address) ? 'On the Table ✓' : 'To the Table', 'p', () => toggleTable(card.address), {
          title: spreads.has(card.address) ? 'Put it back in the box' : 'Lay it on the Table beside the cards you are gathering (P: with every card it relates to)',
          cls: 'table-btn',
        }),
        isStudied(card) ? actionButton('Study', 's', () => study.open(card.address), {
          title: isUnit(card.model) ? 'Take the unit out onto the study desk: one station at a time, in the reading mode you chose' : 'Study the whole theory on the desk: orient, each main result after what it needs, connect, recall',
          cls: 'study-btn',
        }) : null,
        actionButton('Edit', 'e', editCard, writeOpts('edit', { primary: true })),
        card.workspace === 'agent' && ZtFeatures.allows(state.features, 'reply') ? actionButton('Reply', 'r', replyTo, {
          title: 'Answer this agent Card with a human Card: 印证, 反驳, 放宽, or 对应, then the Link back',
          cls: 'reply-btn',
        }) : null,
        card.context === 'regular' ? actionButton('Bridge report', null, useBridgeReport, {
          title: 'Use one existing Bridge report as the distant slot’s first source; it is read only and no proposal is applied',
          cls: 'bridge-btn',
        }) : null,
        actionButton('Direct', 'n', () => newCard('direct'), writeOpts('direct', { title: 'New direct successor' })),
        actionButton('Side', 'b', () => newCard('side'), regular ? writeOpts('side', { title: 'New side successor' }) : { disabled: true, title: 'Tree roots have no side successors' }),
        actionButton('Insert', 'i', insertCard, regular ? writeOpts('insert', { title: 'Insert a new card in this place; this card and its successors move down' }) : { disabled: true, title: 'Nothing goes before a tree root' }),
        actionButton('Move', 'm', () => moveCard(), regular ? writeOpts('move', { title: 'Move with successors, alone, or into another card’s place' }) : { disabled: true, title: 'Tree roots cannot move' }),
        actionButton('Delete', 'del', deleteCard, writeOpts('delete', { danger: true })),
      ),
      card.context === 'tree-root' ? lockLine(card) : null,
      cardLockLine(card),
      h('details', { class: 'raw-toggle' }, h('summary', { text: 'Stored Markdown' }), h('pre', { text: card.text })),
    ));
    reader.scrollTop = 0;
    // Paint the distant slot from the answer already read for this Card, and ask for one when the
    // read has none yet: the request resolves on its own and repaints only the slot. A tree root
    // is navigation and has no slot, so requestDistant asks nothing for it.
    paintDistant();
    requestDistant();
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
    specializes: { fwd: 'specializes', back: 'specialized by' },
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
        dataset: { at: part.address },
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
          n.card.type !== 'idea' ? typeStamp(n.card.type, null, n.card.facet, null, n.card.level, n.card.form) : null),
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

  // ------------------------------------------------------------- math units
  // A Model Card with a Math form states one unit of a theory, or a whole theory (src/math.rs).
  // The reader meets a unit in one of three reading modes: Learn (first study, one station at
  // a time), Review (the focus cards lie face down: recall, then turn over), and Look up (the
  // statement and its labels, as a tool for another proof). get_model's `math` holds the study
  // order and the theory's dependency order; these views only lay them out.
  const READING = {
    learn: { label: 'Learn', key: '1', note: 'first study: one station at a time; proof steps lie face down until you turn them' },
    review: { label: 'Review', key: '2', note: 'recall: the focus cards lie face down; recall each one, then turn it over' },
    lookup: { label: 'Look up', key: '3', note: 'use the unit as a tool: its statement and labels only' },
  };
  const PASSES = { meet: 'Meet', why: 'Why', boundary: 'Boundary', connect: 'Connect', recall: 'Recall', 'look up': 'Look up' };
  state.reading = READING[store.get('zt-reading')] ? store.get('zt-reading') : 'learn';
  const setReading = (mode) => { state.reading = mode; store.set('zt-reading', mode); };
  const isUnit = (model) => Boolean(model && model.head && model.math && model.form && model.form !== 'theory');
  const isTheory = (model) => Boolean(model && model.head && model.math && model.form === 'theory');
  /** A unit or a Theory Card: what the study desk opens on. */
  const isStudied = (card) => Boolean(card && (isUnit(card.model) || (isTheory(card.model) && card.model.head.address === card.address)));

  // The Cards of a unit, read once per change of the archive.
  const cardCache = { fingerprint: null, cards: new Map() };
  function cardAt(address) {
    if (cardCache.fingerprint !== state.fingerprint) {
      cardCache.fingerprint = state.fingerprint;
      cardCache.cards.clear();
    }
    let card = cardCache.cards.get(address);
    if (!card) {
      card = api('card?at=' + enc(address));
      card.catch(() => cardCache.cards.delete(address));
      cardCache.cards.set(address, card);
    }
    return card;
  }
  /** The labels (H1, C2, ...) a Card names, in its facet, title, or body. */
  const labelsOf = (card) => ZtLayout.labelsIn(`${card.facet || ''} ${card.title || ''} ${card.body || ''}`);
  /**
   * The labels each Card of a unit uses: the ones it names, and the ones of a hypothesis role
   * (`假设作用：H2`) that links it, so a counterexample lifts with the hypothesis it shows.
   * Returns address -> labels joined by spaces.
   */
  function unitLabels(cards) {
    const labels = new Map(cards.map((card) => [card.address, labelsOf(card)]));
    for (const card of cards) {
      if (!/^(假设作用|hypothesis role)/i.test(card.facet || '')) continue;
      for (const link of card.outbound || []) {
        const target = labels.get(link.address);
        if (target) for (const label of labels.get(card.address)) target.add(label);
      }
    }
    return new Map([...labels].map(([address, set]) => [address, [...set].join(' ')]));
  }

  // A unit's Model Card opens its sections with bold labels: `**动机**：`, `**陈述**：`.
  const STATEMENT = ['陈述', '定义', '公理', 'statement', 'definition', 'axiom'];
  const MOTIVE = ['动机', 'motive', 'motivation'];
  const NOT_STATEMENT = [...MOTIVE, '思路', '结构', '来源', 'idea', 'structure', 'source'];
  /** The top-level blocks of a rendered body, grouped by the bold label that opens a section. */
  function sectionsOf(md) {
    const sections = [{ name: '', blocks: [] }];
    for (const block of md.children) {
      const lead = block.firstElementChild;
      const after = lead && block.firstChild === lead && lead.tagName === 'STRONG'
        ? block.textContent.slice(lead.textContent.length).trimStart() : '';
      if (/^[：:]/.test(after)) sections.push({ name: lead.textContent.trim().toLowerCase(), blocks: [] });
      sections[sections.length - 1].blocks.push(block);
    }
    return sections;
  }
  /** Keep only a body's statement (`statement`) or its motive (`motive`); false when it has none. */
  function keepSections(md, which) {
    const sections = sectionsOf(md);
    let keep = sections.filter((section) => (which === 'motive' ? MOTIVE : STATEMENT).includes(section.name));
    if (!keep.length && which === 'statement') keep = sections.filter((section) => !NOT_STATEMENT.includes(section.name));
    const kept = new Set(keep.flatMap((section) => section.blocks));
    for (const block of [...md.children]) if (!kept.has(block)) block.remove();
    return kept.size > 0;
  }

  /** Turn each `(H1)` or `（C2）` of a rendered body into a label chip. */
  function chipLabels(root) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode: (node) => (node.parentElement.closest('.math-inline, .math-display, .katex, code, pre, .lchip')
        ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
    });
    const found = [];
    while (walker.nextNode()) found.push(walker.currentNode);
    for (const node of found) {
      const parts = node.textContent.split(/[(（]([HC][0-9]+)[)）]/);
      if (parts.length < 2) continue;
      node.replaceWith(...parts.map((part, at) => (at % 2
        ? h('span', { class: 'lchip', tabindex: '0', dataset: { label: part }, title: `${part}: point at it to lift the Cards that use it`, text: part })
        : part)).filter((part) => part !== ''));
    }
  }

  /**
   * Label chips lift the Cards that use them: pointing at `H1` lifts, inside `scope`, every
   * element whose `data-labels` names H1 (a proof step that uses it, its counterexample). `ready`
   * may first fill in the labels (the Cards are read on the first point).
   */
  function wireLabels(listen, { scope = () => listen, ready, onLift } = {}) {
    // The Card that holds the chip the reader points at stays where it is.
    const lift = (label, on, from) => {
      for (const chip of scope().querySelectorAll(`.lchip[data-label="${label}"]`)) chip.classList.toggle('on', on);
      for (const node of scope().querySelectorAll('[data-labels]')) {
        if (node.dataset.labels.split(' ').includes(label)) node.classList.toggle('lchip-lift', on && !node.contains(from));
      }
      onLift?.(label, on);
    };
    let pointed = null;
    const enter = (event) => {
      const chip = event.target.closest?.('.lchip');
      if (!chip || pointed === chip.dataset.label) return;
      pointed = chip.dataset.label;
      Promise.resolve(ready?.()).then(() => { if (pointed === chip.dataset.label) lift(pointed, true, chip); });
    };
    const leave = (event) => {
      const chip = event.target.closest?.('.lchip');
      if (!chip || !pointed) return;
      lift(pointed, false);
      pointed = null;
    };
    listen.addEventListener('pointerover', enter);
    listen.addEventListener('pointerout', leave);
    listen.addEventListener('focusin', enter);
    listen.addEventListener('focusout', leave);
  }

  /** On a unit's Model Card in the reader: chips on its statement lift the model board's slips. */
  function wireUnitReader(root, card) {
    const model = card.model;
    if (!isUnit(model) || model.head.address !== card.address) return;
    chipLabels(root);
    let filled = null;
    const ready = () => (filled ||= Promise.all(model.parts.map((part) => cardAt(part.address).catch(() => null))).then((parts) => {
      for (const [address, labels] of unitLabels(parts.filter(Boolean))) {
        const slip = document.querySelector(`.mb-slip[data-at="${CSS.escape(address)}"]`);
        if (slip && labels) slip.dataset.labels = labels;
      }
    }));
    wireLabels(root, { scope: () => document.querySelector('#reader') || root, ready });
  }

  // ------------------------------------------------------------ tool tabs
  // A cited premise is a tab on the edge of the desk or the board. Opening it brings the unit
  // half out of the box in Look up mode: its statement and chips only. A tab held down drops
  // the unit back when let go; a tab clicked keeps it out until it is clicked again or Esc.
  const pullout = { node: null, address: null, held: 0 };
  function toolTab(tool, extra) {
    const tab = h('button', {
      class: 'ttab' + (tool.just_in_time ? ' new' : '') + (extra ? ' ' + extra : ''),
      title: `${tool.form || 'unit'} ${tool.address} — ${tool.title}\nPull it out of the box (hold to peek)`,
      dataset: { at: tool.address },
    },
      h('span', { class: 'ttab-form', text: tool.form || 'unit' }),
      h('span', { class: 'ttab-title' }, titleNodes(bareTitle(tool.title || tool.address))),
      tool.just_in_time ? h('span', { class: 'ttab-new', text: 'new' }) : null);
    tab.addEventListener('pointerdown', (event) => {
      if (event.button !== 0) return;
      pullout.held = performance.now();
      if (pullout.address === tool.address) { dropBack(); pullout.held = 0; return; }
      pullOut(tool.address, tab).catch(fail);
    });
    tab.addEventListener('pointerup', () => { if (pullout.held && performance.now() - pullout.held > 350) dropBack(); pullout.held = 0; });
    tab.addEventListener('keydown', (event) => {
      if (event.key !== 'Enter' && event.key !== ' ') return;
      event.preventDefault();
      event.stopPropagation();
      if (pullout.address === tool.address) dropBack(); else pullOut(tool.address, tab).catch(fail);
    });
    return tab;
  }

  /** Bring a unit (or any Card of a unit) half out of the box beside `from`: Look up mode. */
  async function pullOut(address, from) {
    dropBack(true);
    pullout.address = address;
    const card = await cardAt(address);
    if (pullout.address !== address) return;
    const unitHead = card.model && card.model.head && card.model.head.address === address && isUnit(card.model);
    const md = h('div', { class: 'md' });
    md.innerHTML = card.html;
    // A unit is a tool by its statement; any other Card shows its whole body.
    if (unitHead) keepSections(md, 'statement');
    if (unitHead) chipLabels(md);
    wireMarkdown(md);
    const node = h('aside', { class: 'pullout', role: 'dialog', 'aria-label': `Look up ${address}` },
      h('div', { class: 'pullout-card' },
        h('header', { class: 'pullout-head' },
          unitHead ? typeStamp(card.type, null, null, null, card.level, card.form) : h('span', { class: 'sd-facet', text: facetLabel(card.facet) || card.type }),
          h('button', { class: 'stamp', title: 'Open this Card in the reader', text: address, onclick: () => { study.close(); dropBack(); go(address); } }),
          h('button', { class: 'pullout-close', title: 'Drop it back into the box (Esc)', 'aria-label': 'Drop back', text: '×', onclick: () => dropBack() })),
        h('h3', { class: 'pullout-title' }, titleNodes(card.title || address)),
        study.canEnter(card) ? h('button', {
          class: 'btn pullout-study', title: 'Study its proof here, then come back to the step',
          onclick: () => {
            const rect = node.querySelector('.pullout-card').getBoundingClientRect();
            dropBack(true);
            study.enter(address, rect).catch(fail);
          },
        }, 'Study its proof', h('span', { 'aria-hidden': 'true', text: '→' })) : null,
        md),
      h('div', { class: 'pullout-lip', 'aria-hidden': 'true' }));
    // It rises out of the box under the tab that holds it.
    const rect = from?.getBoundingClientRect();
    const width = Math.min(460, window.innerWidth - 32);
    const left = rect ? Math.max(16, Math.min(window.innerWidth - width - 16, rect.left + rect.width / 2 - width / 2)) : (window.innerWidth - width) / 2;
    node.style.setProperty('--pull-left', left + 'px');
    node.style.setProperty('--pull-width', width + 'px');
    document.body.append(node);
    wireLabels(node);
    pullout.node = node;
    for (const tab of document.querySelectorAll('.ttab')) tab.classList.toggle('out', tab.dataset.at === address);
    requestAnimationFrame(() => node.classList.add('up'));
  }
  function dropBack(now) {
    const node = pullout.node;
    pullout.node = null;
    pullout.address = null;
    for (const tab of document.querySelectorAll('.ttab.out')) tab.classList.remove('out');
    if (!node) return;
    if (now) { node.remove(); return; }
    node.classList.remove('up');
    setTimeout(() => node.remove(), 320);
  }

  // ---------------------------------------------------------- theory board
  /**
   * A Theory Card's board: its units laid out by dependency (layout.js `theoryRows`), each a
   * card whose size is its weight: a focus unit (a main result, a theory object, a unit many
   * cite) is a full card, a support unit a smaller card, a lemma a slim slip on the thread
   * between the results it connects, a corollary hung under its theorem, an axiom on the
   * bottom row. Tools from other theories are tabs on the left edge; the theories that use it,
   * tabs on the right. Ink follows the degree of interest: the open unit is lifted, the units
   * it cites and the units that cite it stay inked, the main results and objects keep their
   * ink, and the rest recede to their form and title. Pointing at a unit opens it for a
   * moment. In Review mode the main results lie face down: recall each from its title.
   */
  function theoryBoard(theory, openUnit) {
    const math = theory.model.math;
    const units = math.order || [];
    const byAddress = new Map(units.map((unit) => [unit.address, unit]));
    const users = new Map(units.map((unit) => [unit.address, []]));
    for (const unit of units) for (const premise of unit.cites || []) users.get(premise)?.push(unit.address);
    const { rows, base, hung } = ZtLayout.theoryRows(units);
    const review = state.reading === 'review';
    const nodes = new Map();
    const keep = (unit) => unit.main || unit.object || unit.weight === 'focus';
    const ink = (unit, open) => {
      if (unit.address === open) return 'here';
      if (open && ((byAddress.get(open)?.cites || []).includes(unit.address) || users.get(open)?.includes(unit.address))) return 'near';
      return keep(unit) ? 'keep' : 'far';
    };
    const shape = (unit) => (unit.form === 'lemma' && unit.weight === 'detail' ? 'slim' : unit.weight === 'focus' ? 'full' : 'small');

    const unitNode = (address, hangs) => {
      const unit = byAddress.get(address);
      const down = review && unit.main;
      const lvl = LEVELS[unit.level];
      const node = h('button', {
        class: `tb-unit ${hangs ? 'small hung' : shape(unit)}${down ? ' down' : ''}`,
        dataset: { at: address },
        title: `${unit.form}${lvl ? ' · ' + lvl.label : ''} · ${address}${unit.main ? ' · main result' : unit.object ? ' · theory object' : ''}${unit.load ? ` · cited by ${unit.load}` : ''}\n${unit.title}${down ? '\nFace down: recall the statement, then click to turn it over' : ''}`,
      },
        h('span', { class: 'tb-head' },
          h('span', { class: 'tb-form', text: unit.form }),
          unit.main ? h('span', { class: 'tb-role', text: 'main' }) : unit.object ? h('span', { class: 'tb-role', text: 'object' }) : null,
          unit.load >= 2 ? h('span', { class: 'tb-load', title: `${unit.load} units and models cite it`, text: `用于 ${unit.load}` }) : null),
        h('span', { class: 'tb-title' }, titleNodes(bareTitle(unit.title))),
        down ? h('span', { class: 'tb-statement', hidden: true }) : null);
      node.addEventListener('click', () => {
        // Face down: the first click turns it over to show the statement; the next opens it.
        if (node.classList.contains('down')) {
          node.classList.remove('down');
          node.classList.add('turned');
          cardAt(address).then((card) => {
            const md = h('div', { class: 'md' });
            md.innerHTML = card.html;
            keepSections(md, 'statement');
            chipLabels(md);
            wireMarkdown(md);
            const slot = node.querySelector('.tb-statement');
            slot.replaceChildren(md);
            slot.hidden = false;
          }).catch(fail);
          return;
        }
        go(address);
      });
      nodes.set(address, node);
      return node;
    };
    const cell = (address) => h('div', { class: 'tb-cell' }, unitNode(address),
      (hung.get(address) || []).map((corollary) => unitNode(corollary, true)));
    const threads = svg('svg', { class: 'tb-threads', 'aria-hidden': 'true' });
    const toolsEdge = (math.tools || []).length ? h('div', { class: 'tb-edge tools', 'aria-label': 'Tools from other theories' },
      h('span', { class: 'mb-tray', text: 'Tools' }),
      math.tools.map((tool) => toolTab(tool))) : null;
    const usersEdge = (math.users || []).filter((user) => user.theory).length ? h('div', { class: 'tb-edge users', 'aria-label': 'Theories that use this one' },
      h('span', { class: 'mb-tray', text: '后续' }),
      math.users.filter((user) => user.theory).map((user) => h('button', {
        class: 'ttab user', title: `${user.theory.address} — ${user.theory.title}\ncites ${user.units.map((u) => u.address).join(', ')}`,
        onclick: () => go(user.theory.address),
      }, h('span', { class: 'ttab-form', text: 'theory' }), h('span', { class: 'ttab-title' }, titleNodes(user.theory.title))))) : null;
    const field = h('div', { class: 'tb-field' },
      threads,
      rows.map((row) => h('div', { class: 'tb-row' }, row.map(cell))),
      base.length ? h('div', { class: 'tb-row base', title: 'Axioms hold the theory up' }, base.map(cell)) : null);
    const board = h('div', { class: 'theory-board' + (review ? ' review' : '') }, toolsEdge, field, usersEdge);

    // Ink: the open unit, its premises and users, the main results and objects; the rest recedes.
    let open = openUnit || null;
    const paint = (focus) => {
      for (const [address, node] of nodes) {
        node.classList.remove('here', 'near', 'keep', 'far');
        node.classList.add(ink(byAddress.get(address), focus));
      }
      for (const group of threads.children) {
        const hot = focus && (group.dataset.from === focus || group.dataset.to === focus);
        group.classList.toggle('hot', Boolean(hot));
        group.classList.toggle('quiet', Boolean(focus) && !hot);
      }
    };
    field.addEventListener('pointerover', (event) => {
      const node = event.target.closest('.tb-unit');
      if (node) paint(node.dataset.at);
    });
    field.addEventListener('pointerleave', () => paint(open));

    // A thread runs from a premise down to the unit that cites it; from an axiom, up.
    const draw = () => {
      const box = field.getBoundingClientRect();
      if (!box.width) return;
      threads.setAttribute('width', box.width);
      threads.setAttribute('height', box.height);
      threads.setAttribute('viewBox', `0 0 ${box.width} ${box.height}`);
      threads.replaceChildren();
      const place = (address) => {
        const r = nodes.get(address)?.getBoundingClientRect();
        return r && { x: r.left - box.left + r.width / 2, t: r.top - box.top, b: r.bottom - box.top };
      };
      for (const unit of units) {
        if ([...hung.values()].some((list) => list.includes(unit.address))) continue;
        for (const premise of unit.cites || []) {
          const from = place(premise);
          const to = place(unit.address);
          if (!from || !to) continue;
          const up = from.t > to.b;
          const y1 = up ? from.t : from.b;
          const y2 = up ? to.b + 2 : to.t - 2;
          const mid = (y1 + y2) / 2;
          const group = svg('g', { class: 'tb-thread' });
          group.dataset.from = premise;
          group.dataset.to = unit.address;
          const d = `M ${from.x} ${y1} C ${from.x} ${mid}, ${to.x} ${mid}, ${to.x} ${y2}`;
          const s = up ? 1 : -1;
          group.append(svg('path', { d, class: 'line' }), svg('path', { d: `M ${to.x - 3.5} ${y2 + 5 * s} L ${to.x} ${y2} L ${to.x + 3.5} ${y2 + 5 * s}`, class: 'tip' }));
          threads.append(group);
        }
      }
      paint(open);
    };
    setTimeout(draw);
    new ResizeObserver(draw).observe(field);
    paint(open);
    return board;
  }

  /** The Learn path of a theory: each main result after the units it needs that came before. */
  function theoryPath(theory) {
    const math = theory.model.math;
    const byAddress = new Map((math.order || []).map((unit) => [unit.address, unit]));
    const pass = (math.study || []).find((station) => station.pass === 'path');
    if (!pass || !pass.results.length) return null;
    const stop = (address, main) => {
      const unit = byAddress.get(address) || { form: '', title: address };
      return h('button', {
        class: 'tb-stop' + (main ? ' main' : ''),
        title: `Study ${unit.form} ${address} — ${unit.title}`,
        onclick: () => study.open(theory.address, 'learn', { unit: address }).catch(fail),
      }, h('span', { class: 'tb-form', text: unit.form }), h('span', { class: 'tb-title' }, titleNodes(bareTitle(unit.title))));
    };
    return h('div', { class: 'tb-path', 'aria-label': 'Study path' },
      h('div', { class: 'mb-tray', text: 'Study path · each main result after what it needs' }),
      pass.results.map((result) => h('div', { class: 'tb-route' },
        result.path.map((address) => [stop(address), h('span', { class: 'tb-arrow', 'aria-hidden': 'true', text: '→' })]),
        stop(result.result, true))));
  }

  /** The reading-mode switch: one setting for the study desk and the theory board. */
  function readingSwitch(onChange) {
    return h('div', { class: 'seg sd-modes', role: 'tablist', 'aria-label': 'Reading mode' },
      Object.entries(READING).map(([mode, info]) => h('button', {
        role: 'tab', class: state.reading === mode ? 'on' : null, 'aria-selected': state.reading === mode ? 'true' : 'false',
        title: `${info.label} (${info.key}) — ${info.note}`,
        onclick: () => { if (state.reading !== mode) { setReading(mode); onChange(mode); } },
      }, info.label)));
  }

  /** The math block under a Theory Card or a unit in the reader. */
  function mathTools(card) {
    const model = card.model;
    if (isTheory(model) && model.head.address === card.address) {
      const holder = h('div', { class: 'tb-wrap' });
      const fill = () => holder.replaceChildren(...[
        h('div', { class: 'tb-bar' },
          h('div', { class: 'section-label', text: `Theory · ${model.math.units} unit${model.math.units === 1 ? '' : 's'} in dependency order` }),
          readingSwitch(fill)),
        theoryBoard(card, null),
        theoryPath(card),
        theoryFaults(model.math),
      ].filter(Boolean));
      fill();
      return holder;
    }
    if (!isUnit(model)) return null;
    const theories = model.math.theories || [];
    const holder = h('div', { class: 'tb-wrap' });
    if (theories.length) {
      // The theory around this unit, folded: it opens on this unit.
      const fold = h('details', { class: 'tb-fold' }, h('summary', {},
        h('span', { class: 'mb-tray', text: 'In theory' }), ' ', titleNodes(theories[0].title)));
      fold.open = store.get('zt-theory-fold') === '1';
      let filled = false;
      const fill = () => {
        if (filled || !fold.open) return;
        filled = true;
        cardAt(theories[0].address).then((theory) => {
          if (isTheory(theory.model)) fold.append(theoryBoard(theory, model.head.address));
        }).catch(fail);
      };
      fold.addEventListener('toggle', () => { store.set('zt-theory-fold', fold.open ? '1' : '0'); fill(); });
      fill();
      holder.append(fold);
    }
    return holder;
  }

  function theoryFaults(math) {
    const rows = [...(math.faults || []).map((f) => f.fix), ...(math.cycle || []).map((f) => f.fix), ...(math.forward || []).map((f) => f.note)];
    if (!rows.length) return null;
    return h('details', { class: 'mb-build' }, h('summary', { text: `Theory · ${rows.length} fault${rows.length === 1 ? '' : 's'}` }),
      rows.map((text) => h('p', { class: 'mb-reason fault' }, h('span', { class: 'note' }, titleNodes(text)))));
  }

  // ----------------------------------------------------------- study desk
  /**
   * The study desk: a unit's Cards taken out of the box onto a desk, one station at a time
   * (get_model's `math.study`). The focus Card lies in the centre, squared up and lifted; at
   * most three companions lie beside it in full ink; the Derivation slips of a step wait under
   * it; every other Card of the unit lies in a "+N" stack at the edge; the premises its proof
   * cites are tabs on the bottom edge. Moving to the next station slides the Cards that stay
   * to their new places, brings new ones out of the stack, and puts the others back on it. A
   * face-down Card shows its back with its title only: recall it, then turn it over.
   *
   * A run is one unit, or a whole theory (layout.js `theoryRun`: orient, each main result's
   * prerequisite path, connect, recall). From a step, the reader may study a lemma it cites:
   * the lemma's own stations take the desk, and its last station leads back to the same step.
   */
  const study = (() => {
    const desk = {
      node: null, run: null, units: new Map(), stations: [], at: 0, turned: new Set(),
      cards: new Map(), labels: new Map(), fan: false, nest: [],
    };
    const isOpen = () => Boolean(desk.node);
    const station = () => desk.stations[desk.at];

    async function read(addresses) {
      const missing = [...new Set(addresses)].filter((at) => at && !desk.cards.has(at));
      const cards = await Promise.all(missing.map((at) => cardAt(at).catch(() => null)));
      for (const card of cards) if (card) desk.cards.set(card.address, card);
    }

    /** Read a unit: its Model Card, its parts, and the Cards its stations lay beside them. */
    async function readUnit(head) {
      if (desk.units.has(head)) return desk.units.get(head);
      const card = desk.cards.get(head) || await cardAt(head).catch(() => null);
      if (!card || !isUnit(card.model) || card.model.head.address !== head) return null;
      desk.cards.set(head, card);
      const model = card.model;
      const members = [head, ...model.parts.map((part) => part.address)];
      const beside = (model.math.study || []).flatMap((s) => [...s.focus, ...s.companions, ...(s.details || [])]);
      await read([...members, ...beside]);
      for (const [address, labels] of unitLabels(members.map((at) => desk.cards.get(at)).filter(Boolean))) desk.labels.set(address, labels);
      const facetOf = new Map(model.parts.map((part) => [part.address, part.facet || '']));
      const unit = { head, model, members, isPlan: (at) => /^(证明思路|proof plan)/i.test(facetOf.get(at) || '') };
      desk.units.set(head, unit);
      return unit;
    }
    const unitStations = (head, mode) => {
      const unit = desk.units.get(head);
      return unit ? ZtLayout.studyStations(unit.model.math.study, mode, head, unit.isPlan).map((s) => ({ ...s, unit: head })) : [];
    };

    /**
     * Open the desk on a unit (any of its Cards) or a Theory Card. `options.unit` starts a
     * theory run at the first station of that unit.
     */
    async function open(address, mode, options = {}) {
      const card = await cardAt(address);
      const model = card.model;
      const theory = isTheory(model) && model.head.address === address;
      if (!theory && !isUnit(model)) { toast(`${address} is in no mathematical unit or theory`, 'err'); return; }
      if (mode) setReading(mode);
      Object.assign(desk, { units: new Map(), cards: new Map(), labels: new Map(), turned: new Set(), fan: false, nest: [] });
      if (theory) {
        desk.cards.set(address, card);
        desk.run = { kind: 'theory', head: address, math: model.math };
        await Promise.all((model.math.order || []).map((unit) => readUnit(unit.address)));
        const connect = (model.math.study || []).find((s) => s.pass === 'connect');
        await read((connect?.users || []).filter(Boolean));
      } else {
        const unit = await readUnit(model.head.address);
        desk.run = { kind: 'unit', head: unit.head };
      }
      plan();
      const at = options.unit
        ? desk.stations.findIndex((s) => s.unit === options.unit)
        : address === desk.run.head ? 0 : desk.stations.findIndex((s) => s.focus.includes(address));
      if (at > 0) desk.at = at;
      if (!desk.node) {
        desk.node = h('section', { class: 'sd', role: 'dialog', 'aria-label': 'Study desk' });
        document.body.append(desk.node);
        // Pointing at a label also lifts the Cards in the stack that use it.
        wireLabels(desk.node, { onLift: (label, on) => {
          const stack = desk.node?.querySelector('.sd-fold');
          if (!stack) return;
          const any = [...stack.querySelectorAll('.sd-fslip')].some((slip) => (slip.dataset.labels || '').split(' ').includes(label));
          stack.classList.toggle('peek', on && any);
        } });
        requestAnimationFrame(() => desk.node?.classList.add('open'));
      }
      render(null);
    }

    function plan() {
      const mode = state.reading;
      desk.nest = [];
      desk.stations = desk.run.kind === 'theory'
        ? ZtLayout.theoryRun(desk.run.math, desk.run.head, mode, (unit) => unitStations(unit, mode))
        : unitStations(desk.run.head, mode);
      desk.at = 0;
    }

    /** A lemma the open step cites can be studied in place: not the unit on the desk, not one already open. */
    function canEnter(card) {
      if (!isOpen() || !card || card.form !== 'lemma' || !isUnit(card.model) || card.model.head.address !== card.address) return false;
      const open = new Set([station()?.unit, ...desk.nest.map((frame) => frame.unit)]);
      return !open.has(card.address);
    }

    /** Step into a lemma's own short study order; the Cards come out of the pulled-out lemma. */
    async function enter(head, from) {
      const unit = await readUnit(head);
      const stations = unit ? unitStations(head, state.reading).filter((s) => s.pass !== 'connect') : [];
      if (!stations.length) { toast(`${head} has no study order`, 'err'); return; }
      const before = measure(from);
      desk.nest.push({ stations: desk.stations, at: desk.at, unit: station().unit, lemma: head });
      desk.stations = stations;
      desk.at = 0;
      desk.fan = false;
      render(before);
    }
    /** Back to the step the lemma was opened from; the lemma's Cards go back under its tab. */
    function leave() {
      const frame = desk.nest.pop();
      if (!frame) return;
      const before = measure(null, frame.lemma);
      desk.stations = frame.stations;
      desk.at = frame.at;
      desk.fan = false;
      render(before);
    }

    function close() {
      const node = desk.node;
      if (!node) return;
      desk.node = null;
      dropBack();
      node.classList.remove('open');
      setTimeout(() => node.remove(), 260);
    }

    function go_(at) {
      if (at < 0 || at >= desk.stations.length || at === desk.at) return;
      const before = measure();
      desk.at = at;
      desk.fan = false;
      render(before);
    }
    const forward = () => (desk.at === desk.stations.length - 1 && desk.nest.length ? leave() : go_(desk.at + 1));
    const backward = () => (desk.at === 0 && desk.nest.length ? leave() : go_(desk.at - 1));

    const turnKey = (address) => `${state.reading}:${desk.nest.length}:${desk.at}:${address}`;
    function turn(address) {
      desk.turned.add(turnKey(address));
      for (const node of desk.node.querySelectorAll(`.sd-card[data-at="${CSS.escape(address)}"].down`)) node.classList.remove('down');
    }
    const isDown = (s, address) => s.face_down && s.focus.includes(address) && !desk.turned.has(turnKey(address));

    /** One Card as an index card on the desk. `as`: focus or companion. */
    function deskCard(address, as, s) {
      const card = desk.cards.get(address);
      if (!card) return null;
      // A Model Card of a unit or a theory: this station's own, or another lying beside it as a tool.
      const head = Boolean(card.model && card.model.head && card.model.head.address === address && card.form);
      const own = address === s.unit;
      const md = h('div', { class: 'md' });
      md.innerHTML = card.html;
      let lean = false;
      if (head) {
        // The statement as a tool, the motive alone before the statement is met, or the whole Model Card.
        const meetsMotive = own && as === 'companion' && s.pass === 'meet'
          && !desk.stations.slice(0, desk.at).some((other) => other.unit === s.unit && other.focus.includes(address));
        if (s.lookup || !own || (as === 'companion' && !meetsMotive)) lean = !keepSections(md, 'statement');
        else if (meetsMotive) lean = !keepSections(md, 'motive');
        chipLabels(md);
      }
      wireMarkdown(md);
      const down = as === 'focus' && isDown(s, address);
      const labels = head ? '' : desk.labels.get(address) || [...labelsOf(card)].join(' ');
      const crux = s.crux && as === 'focus';
      const facet = head ? null : card.facet;
      const back = facet && /^(假设作用|hypothesis role)/i.test(facet) && labels
        ? `Why ${labels.split(' ')[0]}?` : null;
      const node = h('article', {
        class: `sd-card ${as}${head ? ' head' : ''}${down ? ' down' : ''}${crux ? ' crux' : ''}${card.emphasis === 'core' ? ' core' : ''}`,
        dataset: { at: address, labels: labels || null },
      },
        h('div', { class: 'sd-flip' },
          h('div', { class: 'sd-face front' },
            h('header', { class: 'sd-head' },
              head ? typeStamp(card.type, null, null, null, card.level, card.form) : h('span', { class: 'sd-facet', text: facetLabel(facet) }),
              crux ? h('span', { class: 'sd-crux', title: 'The crux step: the idea of the proof', text: 'crux' }) : null,
              labels && !head ? h('span', { class: 'sd-uses', title: 'The labels this Card uses' }, labels.split(' ').map((label) => h('span', { class: 'lchip small', tabindex: '0', dataset: { label }, text: label }))) : null,
              h('button', { class: 'stamp', title: 'Open this Card in the reader', text: address, onclick: () => { close(); go(address); } })),
            h('h3', { class: 'sd-title' }, titleNodes(card.title || address)),
            lean && as !== 'focus' ? null : md),
          down ? h('div', { class: 'sd-face back', 'aria-label': 'Face down' },
            h('span', { class: 'sd-back-title' }, back || titleNodes(head ? card.title : bareTitle(card.title))),
            h('span', { class: 'sd-back-hint', text: head ? 'Recall the hypotheses and the conclusion, then turn it over' : 'Recall it, then turn it over' }),
            h('kbd', { text: 'Space' })) : null));
      if (down) node.addEventListener('click', (event) => { if (!event.target.closest('button')) turn(address); });
      return node;
    }

    /** Where the run stands: the run, the units the reader stepped in from, the station's unit. */
    function trail(s) {
      const units = [desk.run.head];
      for (const frame of desk.nest) if (frame.unit !== units[units.length - 1]) units.push(frame.unit);
      if (s.unit !== units[units.length - 1]) units.push(s.unit);
      return h('div', { class: 'sd-unit' }, units.map((address, at) => {
        const card = desk.cards.get(address);
        if (!card) return null;
        const last = at === units.length - 1;
        return [at ? h('span', { class: 'sd-crumb-sep', 'aria-hidden': 'true', text: '›' }) : null,
          h('span', { class: 'sd-crumb' + (last ? ' here' : '') }, typeStamp(card.type, null, null, null, card.level, card.form),
            h('span', { class: 'sd-unit-title' }, titleNodes(at ? bareTitle(card.title) : card.title)))];
      }));
    }

    /** The track: one group of dots per pass, or, in a theory run, per unit on its path. */
    function track() {
      const theoryRun = desk.run.kind === 'theory' && !desk.nest.length;
      const keyOf = (s) => (theoryRun && s.unit !== desk.run.head ? 'u:' + s.unit : 'p:' + s.pass);
      const groups = [];
      desk.stations.forEach((s, at) => {
        if (!groups.length || groups[groups.length - 1].key !== keyOf(s)) groups.push({ key: keyOf(s), first: s, stops: [] });
        groups[groups.length - 1].stops.push(at);
      });
      const nameOf = (group) => {
        if (group.key.startsWith('p:')) return h('span', { class: 'sd-pass-name', text: PASSES[group.first.pass] || group.first.pass });
        const card = desk.cards.get(group.first.unit);
        return h('span', { class: 'sd-pass-name unit', title: card ? `${card.form} ${card.address} — ${card.title}` : group.first.unit },
          h('b', { text: card?.form || 'unit' }), ' ', card ? titleNodes(bareTitle(card.title)) : null);
      };
      const lemma = desk.nest.length ? desk.cards.get(desk.nest[desk.nest.length - 1].lemma) : null;
      return h('nav', { class: 'sd-track' + (theoryRun ? ' run' : ''), 'aria-label': 'Stations' },
        lemma ? h('span', { class: 'sd-pass sd-inset', title: 'Studying a lemma the step cites; its last station leads back to the step' },
          h('span', { class: 'sd-pass-name', text: 'Lemma' })) : null,
        groups.map((group) => h('span', { class: 'sd-pass' + (group.stops.includes(desk.at) ? ' on' : '') },
          nameOf(group),
          group.stops.map((at) => h('button', {
            class: 'sd-dot' + (at === desk.at ? ' on' : at < desk.at ? ' past' : '') + (desk.stations[at].crux ? ' crux' : ''),
            title: `${PASSES[desk.stations[at].pass] || desk.stations[at].pass} · ${desk.stations[at].focus.map((a) => facetLabel(desk.cards.get(a)?.facet) || (a === desk.stations[at].unit ? 'statement' : bareTitle(desk.cards.get(a)?.title || a))).join(' + ')}`,
            'aria-current': at === desk.at ? 'step' : null,
            onclick: () => go_(at),
          })))));
    }

    function render(before) {
      const s = station();
      const unit = desk.units.get(s.unit);
      const shown = new Set([...s.focus, ...s.companions, ...(s.details || [])]);
      // The rest of the station's unit lies in the stack; at a theory station, the theory's other units.
      const all = unit ? unit.members : (desk.run.math?.order || []).map((u) => u.address);
      const folded = all.filter((at) => !shown.has(at) && desk.cards.has(at));
      const companions = s.companions.map((at) => deskCard(at, 'companion', s)).filter(Boolean);
      const left = companions.filter((_, at) => at % 2 === 1);
      const right = companions.filter((_, at) => at % 2 === 0);

      const fold = folded.length ? h('div', { class: 'sd-fold' + (desk.fan ? ' fanned' : '') },
        h('div', { class: 'sd-fan' }, folded.map((at) => {
          const card = desk.cards.get(at);
          const head = card.form && card.model?.head?.address === at;
          return h('button', {
            class: 'sd-fslip', dataset: { at, labels: head ? null : desk.labels.get(at) || null },
            title: `${at} — ${card.title}\nPull it out of the stack`,
            onclick: (event) => pullOut(at, event.currentTarget).catch(fail),
          }, h('span', { class: 'sd-facet', text: head ? card.form : facetLabel(card.facet) }), h('span', { class: 'sd-fslip-title' }, titleNodes(bareTitle(card.title))));
        })),
        h('button', {
          class: 'sd-stack', title: `${folded.length} more Card${folded.length === 1 ? '' : 's'} ${unit ? 'of this unit' : 'of this theory'}, face down in a stack`,
          onclick: () => { desk.fan = !desk.fan; desk.node.querySelector('.sd-fold').classList.toggle('fanned', desk.fan); },
        }, h('span', { text: `+${folded.length}` }))) : null;

      const focus = s.focus.map((at) => deskCard(at, 'focus', s)).filter(Boolean);
      const details = (s.details || []).map((at) => {
        const card = desk.cards.get(at);
        if (!card) return null;
        return h('details', { class: 'sd-slip', dataset: { at, labels: desk.labels.get(at) || null } },
          h('summary', {}, h('span', { class: 'sd-facet', text: facetLabel(card.facet) }), ' ', titleNodes(bareTitle(card.title))),
          (() => { const md = h('div', { class: 'md' }); md.innerHTML = card.html; wireMarkdown(md); return md; })());
      }).filter(Boolean);
      const anyDown = focus.some((node) => node.classList.contains('down'));
      const frame = desk.nest[desk.nest.length - 1];
      const stepCard = frame && desk.cards.get(frame.stations[frame.at].focus[0]);
      const last = desk.at === desk.stations.length - 1;

      desk.node.replaceChildren(
        h('header', { class: 'sd-bar' },
          trail(s),
          frame ? h('button', {
            class: 'btn sd-return', title: `Back to the step ${stepCard ? bareTitle(stepCard.title) : ''} (Esc)`, onclick: leave,
          }, '↩ Back to the step') : null,
          readingSwitch(() => { plan(); render(null); }),
          h('button', { class: 'icon-btn sd-close', title: 'Put the cards back in the box (Esc)', 'aria-label': 'Close the study desk', onclick: close },
            h('span', { 'aria-hidden': 'true', text: '×' }))),
        track(),
        h('div', { class: 'sd-surface' },
          h('div', { class: 'sd-spread' + (focus.length > 1 ? ' pair' : '') },
            h('div', { class: 'sd-side left' }, left),
            h('div', { class: 'sd-center' }, h('div', { class: 'sd-focus' }, focus), details.length ? h('div', { class: 'sd-slips' }, details) : null),
            h('div', { class: 'sd-side right' }, right)),
          fold),
        h('footer', { class: 'sd-foot' },
          h('button', { class: 'btn', disabled: desk.at === 0 && !frame, onclick: backward }, desk.at === 0 && frame ? '↩ Back to the step' : '← Back'),
          s.tools && s.tools.length ? h('div', { class: 'sd-tabs', 'aria-label': 'Cited premises' }, s.tools.map((tool) => toolTab(tool))) : h('div', { class: 'sd-tabs' }),
          anyDown ? h('button', { class: 'btn', onclick: () => s.focus.forEach(turn) }, 'Turn over', h('kbd', { text: 'Space' })) : null,
          h('button', { class: 'btn primary', disabled: last && !frame, onclick: forward }, last && frame ? 'Back to the step ↩' : 'Next →')),
      );
      // The statement's chips cue the labels the focus uses, and the clause a non-example fails.
      const cued = new Set();
      const cue = (at) => (desk.labels.get(at) || '').split(' ').forEach((label) => label && cued.add(label));
      for (const at of s.focus) if (at !== s.unit) cue(at);
      for (const at of s.companions) if (/^(非例|反例|non-example|counterexample)/i.test(desk.cards.get(at)?.facet || '')) cue(at);
      for (const chip of desk.node.querySelectorAll('.sd-card.head .md .lchip')) chip.classList.toggle('cue', cued.has(chip.dataset.label));
      if (before) slideFrom(before);
    }

    // Motion: the Cards that stay slide to their new places; new Cards come off the stack (or
    // out of the lemma the reader pulled out); the Cards that leave go back onto the stack (or
    // under the lemma's tab). FLIP.
    function measure(origin, sinkAt) {
      const rects = new Map();
      for (const node of desk.node.querySelectorAll('.sd-card')) rects.set(node.dataset.at, { rect: node.getBoundingClientRect(), node });
      const stack = desk.node.querySelector('.sd-stack');
      return { rects, stack: stack ? stack.getBoundingClientRect() : null, origin, sinkAt };
    }
    function slideFrom(before) {
      const stack = desk.node.querySelector('.sd-stack')?.getBoundingClientRect() || before.stack;
      const source = before.origin || stack;
      const tab = before.sinkAt && desk.node.querySelector(`.sd-tabs .ttab[data-at="${CSS.escape(before.sinkAt)}"]`);
      const sink = tab ? tab.getBoundingClientRect() : stack;
      const stays = new Set();
      for (const node of desk.node.querySelectorAll('.sd-card')) {
        const now = node.getBoundingClientRect();
        const was = before.rects.get(node.dataset.at)?.rect;
        stays.add(node.dataset.at);
        const from = was || source;
        if (!from) continue;
        const scale = was ? 1 : before.origin ? Math.min(1, before.origin.width / now.width) : 0.35;
        node.animate([
          { transform: `translate(${from.left - now.left}px, ${from.top - now.top}px) scale(${scale})`, opacity: was ? 1 : before.origin ? 0.6 : 0, transformOrigin: 'top left' },
          { transform: 'none', opacity: 1, transformOrigin: 'top left' },
        ], { duration: 380, easing: 'cubic-bezier(.3, .7, .25, 1)' });
      }
      if (!sink) return;
      for (const [address, { rect, node }] of before.rects) {
        if (stays.has(address)) continue;
        const ghost = node.cloneNode(true);
        ghost.classList.add('sd-ghost');
        Object.assign(ghost.style, { left: rect.left + 'px', top: rect.top + 'px', width: rect.width + 'px', height: rect.height + 'px' });
        document.body.append(ghost);
        const motion = ghost.animate([
          { transform: 'none', opacity: 1 },
          { transform: `translate(${sink.left - rect.left}px, ${sink.top - rect.top}px) scale(.25)`, opacity: 0 },
        ], { duration: 340, easing: 'cubic-bezier(.4, 0, .6, 1)', fill: 'forwards' });
        motion.onfinish = () => ghost.remove();
      }
    }

    function key(event) {
      if (event.key === 'Escape') {
        event.preventDefault();
        if (pullout.node) dropBack(); else if (desk.nest.length) leave(); else close();
        return true;
      }
      if (isTyping(event.target) || event.ctrlKey || event.metaKey || event.altKey) return false;
      switch (event.key) {
        case 'ArrowRight': case 'PageDown': case 'j': event.preventDefault(); forward(); return true;
        case 'ArrowLeft': case 'PageUp': case 'k': event.preventDefault(); backward(); return true;
        case ' ': case 'Enter':
          if (event.target.closest?.('button, summary, .ttab')) return false;
          event.preventDefault();
          station().focus.forEach(turn);
          return true;
        default: {
          const mode = Object.keys(READING).find((one) => READING[one].key === event.key);
          if (mode) {
            event.preventDefault();
            setReading(mode);
            plan();
            render(null);
          }
          return true;
        }
      }
    }

    return { open, close, key, isOpen, canEnter, enter };
  })();

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

  // ------------------------------------------------------------ tree locks
  // S01 (#124): the owner freezes a whole tree, edits a Lock's note, and unlocks it here and only
  // here. Every action is two steps: the route answers a plan, the owner reviews it and types the
  // confirmation word, and the confirmed request sends the plan's `plan_hash` back. The server
  // recomputes the plan under the edit lock; a stale hash answers the renewed plan and writes
  // nothing. Cancel sends nothing. The pure rules are `ZtLock` (`lock.js`).

  /** Read every Lock and the integrity result, then repaint the cabinet's lock button and the fault pill. */
  async function loadLocks() {
    try {
      state.locks = await api('locks');
    } catch (error) {
      state.locks = null;
      throw error;
    } finally {
      paintLocks();
    }
    return state.locks;
  }

  /** The cabinet's Locks button counts the active Locks; one page-level pill names integrity faults. */
  function paintLocks() {
    const read = ZtLock.panel(state.locks);
    const open = $('#locks-open');
    open.textContent = read.active.length ? `Locks · ${read.active.length}` : 'Locks';
    const pill = $('#lock-pill');
    const line = ZtLock.faultLine(state.locks);
    pill.hidden = !line;
    pill.textContent = line || '';
    bands.take(state.locks);
  }

  /** The seal stamp: the first 12 characters of the digest, struck through when a fault was found. */
  function sealStamp(seal, broken) {
    return h('span', {
      class: 'lock-seal' + (broken ? ' broken' : ''),
      title: (broken ? 'Integrity fault: the tree no longer matches this seal. ' : '') + `Full seal: ${seal}`,
    }, h('span', { class: 'lbl', text: broken ? 'seal broken' : 'seal' }), h('span', { class: 'v', text: ZtLock.sealShort(seal) }));
  }

  /** The full digest, folded: it is evidence, not something to read every time. */
  function digestFold(seal) {
    return h('details', { class: 'lock-fold' },
      h('summary', { text: 'Full seal (SHA-256)' }),
      h('code', { class: 'lock-digest', text: seal || '' }));
  }

  /** A folded list: the summary counts it, the rows show only when opened. */
  function lockFold(label, rows, extra) {
    if (!rows.length) return null;
    return h('details', { class: 'lock-fold' + (extra ? ' ' + extra : '') },
      h('summary', { text: `${label} · ${rows.length}` }),
      h('ul', { class: 'lock-list' }, rows));
  }

  const pinRows = (groups) => groups.map((pin) => h('li', {},
    h('span', { class: 'a', text: pin.target }), h('span', { class: 'arrow', text: '←' }),
    h('span', { class: 'b', text: pin.sources.join(', ') })));

  /**
   * The Broken Links a plan or a Lock's retained `lock` event names (`ZtLock.warningsFold`), as one
   * folded warning list, every row kept (S04, #127). `null` when there are none.
   */
  function warningFold(fold) {
    if (!fold) return null;
    return h('details', { class: 'lock-fold warn' },
      h('summary', { text: fold.label }),
      h('ul', { class: 'lock-list' }, fold.rows.map((warning) => h('li', {},
        h('span', { class: 'a', text: warning.source }), h('span', { class: 'arrow', text: '→' }),
        h('span', { class: 'b broken', text: `${warning.target} (missing)` })))));
  }

  const dateOf = (at) => String(at || '').replace('T', ' ').replace(/:\d\d(\.\d+)?Z$/, ' UTC');

  /**
   * The tree as an index card in the dialog: what the owner freezes. Its address stamp sits in the
   * corner, the seal beside the count, and the rest folds away below.
   */
  function lockCardView({ root, title, kind, workspace, count, pins, seal, broken, note, since, lockId, until, noteless }) {
    return h('div', { class: 'lock-card' + (kind === 'literature' ? ' literature' : ' topic') },
      h('div', { class: 'lock-card-head' },
        h('span', { class: 'kind-tag ' + (kind || ''), text: `${kind === 'literature' ? 'Literature' : 'Topic'} tree` }),
        h('span', { class: 'ws-tag ' + workspace, text: wsLabel(workspace) }),
        h('span', { class: 'lock-addr', text: root })),
      h('div', { class: 'lock-card-body' },
        h('div', { class: 'lock-title' }, titleNodes(title || root)),
        h('div', { class: 'lock-facts' },
          h('span', { class: 'lock-count' }, h('b', { text: String(count) }), ` card${count === 1 ? '' : 's'}`),
          h('span', { class: 'lock-sub', text: `${pins} pinned target${pins === 1 ? '' : 's'}` }),
          // S06 (#129): a relock is a new promise, so the card names which Lock it shows.
          lockId != null ? h('span', { class: 'lock-sub', text: `lock ${lockId}` }) : null,
          since ? h('span', { class: 'lock-sub', text: `locked ${dateOf(since)}` }) : null,
          // S04 (#127): an ended Lock is the same card, dated at both ends.
          until ? h('span', { class: 'lock-sub', text: `unlocked ${dateOf(until)}` }) : null,
          sealStamp(seal, broken)),
        note ? h('p', { class: 'lock-note-text', text: note })
          : noteless ? h('p', { class: 'lock-note-text lock-note-empty', text: 'no note' }) : null));
  }

  /**
   * The two-step dialog every Lock action shares. `body(note)` builds the route's request; the
   * plan is fetched on open, again on "Review again" after the note changed, and replaced by the
   * renewed plan a stale confirmation answers. The note field and the typed word are never cleared
   * by a refusal. Resolves with the committed answer (`lock`, and `log_error` when the after-commit
   * `zt.log` mirror failed, S04 #127), or `null` on Cancel (nothing is sent).
   */
  function lockDialog({ action, path, heading, verb, body, note: initialNote, noteField, view }) {
    return new Promise((resolve) => {
      let plan = null;
      let busy = false;
      let closed = false;
      const summary = h('div', { class: 'lock-plan' }, h('p', { class: 'hint', text: 'Reading the plan…' }));
      const note = noteField ? h('textarea', {
        class: 'lock-note', rows: '3', spellcheck: 'false', 'aria-label': 'Lock note',
        placeholder: 'Why this tree is frozen (optional, at most 500 characters)',
      }) : null;
      if (note) note.value = initialNote || '';
      const counter = h('span', { class: 'lock-counter' });
      const review = h('button', { class: 'btn', type: 'button', text: 'Review again', hidden: true });
      const input = h('input', { autocomplete: 'off', spellcheck: 'false', 'aria-label': 'Confirmation' });
      const ok = h('button', { class: 'btn primary' + (action === 'unlock' ? ' danger-fill' : ''), type: 'button', disabled: true, text: verb });
      const cancel = h('button', { class: 'btn', type: 'button', text: 'Cancel' });
      const msg = h('span', { class: 'msg' });
      const setMsg = (text, kind) => { msg.textContent = text || ''; msg.className = 'msg' + (kind ? ' ' + kind : ''); };
      const dialog = h('div', { class: 'sheet-dlg narrow lock-dlg', role: 'dialog', 'aria-label': heading },
        h('div', { class: 'dlg-head' }, h('h3', { text: heading })),
        h('div', { class: 'confirm-body lock-body' },
          summary,
          note ? h('label', { class: 'lock-note-field' },
            h('span', { class: 'lock-note-label' }, h('span', { text: action === 'note' ? 'New note' : 'Note' }), counter),
            note) : null,
          h('div', { class: 'lock-confirm' },
            h('p', {}, 'Type ', h('code', { text: action }), ` to ${verb.toLowerCase()}:`),
            input),
        ),
        h('div', { class: 'dlg-foot' }, msg, review, cancel, ok),
      );
      const close = openOverlay(dialog);
      const noteValue = () => (note ? note.value : null);
      const sync = () => {
        if (closed) return;
        const checked = ZtLock.noteCheck(noteValue());
        if (note) {
          counter.textContent = `${checked.count} / ${ZtLock.NOTE_LIMIT}`;
          counter.classList.toggle('over', !checked.ok);
        }
        const fresh = ZtLock.planCurrent(plan, action, noteValue());
        dialog.classList.toggle('stale', !!plan && !fresh);
        review.hidden = !plan || fresh || !checked.ok;
        ok.disabled = busy || !fresh || input.value.trim() !== action;
      };
      const show = (answer) => {
        plan = answer.plan;
        // replaceChildren would print a folded-away `null` as text, so empty folds are dropped.
        summary.replaceChildren(...view(plan, answer.lines || []).filter(Boolean));
      };
      const fetchPlan = async () => {
        const checked = ZtLock.noteCheck(noteValue());
        if (!checked.ok) { setMsg(checked.error, 'err'); sync(); return; }
        busy = true;
        sync();
        try {
          show(await api(path, body(noteValue())));
          setMsg('');
        } catch (error) {
          setMsg(error.message, 'err');
        } finally { busy = false; sync(); }
      };
      const finish = (done) => { closed = true; close(); resolve(done); };
      ok.addEventListener('click', async () => {
        if (ok.disabled) return;
        busy = true;
        sync();
        try {
          const answer = await api(path, { ...body(noteValue()), confirm: action, plan_hash: plan.plan_hash });
          if (ZtLock.answer(answer) === 'applied') { finish(answer); return; }
          // The tree, the Lock or the archive moved since this plan: nothing was written. The
          // renewed plan replaces the old one; the note stays as typed and the owner reviews again.
          show(answer);
          input.value = '';
          setMsg('The plan changed since you read it, so nothing was written. Review the new plan and confirm again.', 'err');
        } catch (error) {
          setMsg(error.message, 'err');
        } finally { busy = false; sync(); }
      });
      cancel.addEventListener('click', () => finish(null));
      review.addEventListener('click', () => fetchPlan());
      input.addEventListener('input', sync);
      if (note) {
        note.addEventListener('input', () => {
          sync();
          if (plan && !ZtLock.planCurrent(plan, action, noteValue())) {
            const checked = ZtLock.noteCheck(noteValue());
            setMsg(checked.ok ? 'The note changed: review the plan again before you confirm.' : checked.error, 'err');
          } else setMsg('');
        });
      }
      dialog.addEventListener('keydown', (event) => {
        if (event.key === 'Escape') { event.preventDefault(); cancel.click(); }
        if (event.key === 'Enter' && event.target === input) { event.preventDefault(); ok.click(); }
      });
      sync();
      fetchPlan().then(() => { if (!closed) (note || input).focus(); });
    });
  }

  /** The Lock plan: the tree card, then what the promise covers, folded by degree of interest. */
  function lockPlanView(plan) {
    const regular = plan.regular_count;
    return [
      lockCardView({
        root: plan.root, title: plan.title, kind: plan.zone, workspace: plan.workspace,
        count: plan.member_count, pins: plan.pin_count, seal: plan.seal,
      }),
      h('p', { class: 'lock-what', text: `Freezes the root and ${regular} regular card${regular === 1 ? '' : 's'}: no edit, move, insert, delete or new card in this tree, for you as for agents, until you unlock it here. Its pinned targets keep their addresses.` }),
      h('p', { class: 'lock-limit', text: 'Pins keep a target\'s address and existence only: its meaning, and any external document, artifact or cache availability, stay outside the seal promise.' }),
      digestFold(plan.seal),
      lockFold('Pinned targets', pinRows(ZtLock.pinGroups(plan))),
      warningFold(ZtLock.warningsFold(plan.warnings, 'plan')),
      lockFold('Members', plan.members.map((member) => h('li', {},
        h('span', { class: 'a', text: member.address }),
        h('span', { class: 'b fp', title: member.fingerprint, text: String(member.fingerprint).slice(0, 12) })))),
    ];
  }

  /** An existing Lock's plan (unlock or note): the same tree card, built from the Lock's facts. */
  function existingLockView(plan, extra) {
    const card = state.byAddress.get(plan.root);
    // A latched fault breaks the seal exactly as a current one does (S06, #129).
    const broken = Boolean(ZtLock.panel(state.locks).broken[plan.lock_id]);
    return [
      lockCardView({
        root: plan.root, title: card ? card.title : plan.root, kind: card ? card.kind : (/^\d+\//.test(plan.root) ? 'topic' : 'literature'),
        workspace: plan.workspace, count: plan.member_count, pins: plan.pin_count, seal: plan.seal, broken,
        note: plan.previous_note, since: plan.locked_at, lockId: plan.lock_id,
      }),
      ...extra,
      digestFold(plan.seal),
    ];
  }

  /**
   * After a committed Lock action, the normal refresh (S05, #128, R222): it reads the overview,
   * the Cards and the Locks again, so the cabinet's drawer, the desk's stack band and every open
   * copy of the tree's Cards show the new state. An open writing desk keeps its draft: a refresh
   * repaints the page under the dialogs and never closes one. A committed answer whose `zt.log`
   * mirror failed adds one separate warning line (`ZtLock.logWarning`, S04 #127, R105): the action
   * stays committed, so the first line still says what was done.
   */
  async function afterLock(message, answer) {
    toast(message);
    const warning = ZtLock.logWarning(answer);
    if (warning) toast(warning, 'warn');
    await refresh().catch(fail);
  }

  /** Lock tree: review the whole tree's plan, then freeze it. */
  async function lockTree(rootAddress) {
    const done = await lockDialog({
      action: 'lock', path: 'lock', verb: 'Lock tree', noteField: true, note: '',
      heading: `Lock tree ${rootAddress}`,
      body: (note) => ({ root: rootAddress, note: ZtLock.noteCheck(note).note }),
      view: (plan) => lockPlanView(plan),
    });
    const lock = done && done.lock;
    if (lock) await afterLock(`locked ${lock.root} · seal ${ZtLock.sealShort(lock.seal)}`, done);
    return lock || null;
  }

  /** Unlock tree: review exactly this Lock's release, then end it. */
  async function unlockTree(lock) {
    const done = await lockDialog({
      action: 'unlock', path: 'unlock', verb: 'Unlock tree',
      heading: `Unlock tree ${lock.root}`,
      body: () => ({ lock_id: lock.lock_id, tree: lock.root }),
      view: (plan) => existingLockView(plan, [
        h('p', { class: 'lock-what', text: `Ends lock ${plan.lock_id} only. Its ${plan.member_count} cards can be edited, moved and deleted again, the tree takes new cards, and its ${plan.pin_count} pinned target${plan.pin_count === 1 ? '' : 's'} are released unless another Lock pins them. The history stays in the archive.` }),
      ]),
    });
    const ended = done && done.lock;
    if (ended) await afterLock(`unlocked ${ended.root}`, done);
    return ended || null;
  }

  /** Edit lock note: a note change has its own confirmation and never reseals the tree. */
  async function editLockNote(lock) {
    const done = await lockDialog({
      action: 'note', path: 'lock/note', verb: 'Change note', noteField: true, note: lock.note || '',
      heading: `Lock note on ${lock.root}`,
      body: (note) => ({ lock_id: lock.lock_id, tree: lock.root, note: ZtLock.noteCheck(note).note }),
      view: (plan) => existingLockView(plan, [
        h('p', { class: 'lock-what', text: 'Changes only the note. The members, the pins and the seal stay as they are.' }),
      ]),
    });
    const noted = done && done.lock;
    if (noted) await afterLock(`note changed on ${noted.root}`, done);
    return noted || null;
  }

  /**
   * The lock panel: every active Lock as a card with its seal, faults and actions, and the ended
   * Locks folded below. Opening it is an integrity check point (the server records new faults).
   * A fault is shown beside its Lock; it hides nothing and never updates the shown seal.
   */
  async function lockPanel(focusRoot) {
    const body = h('div', { class: 'confirm-body lock-panel-body' }, h('p', { class: 'hint', text: 'Reading the locks…' }));
    const status = h('span', { class: 'lock-status' });
    let close = null;
    close = openOverlay(h('div', { class: 'sheet-dlg lock-panel', role: 'dialog', 'aria-label': 'Tree locks' },
      h('div', { class: 'dlg-head' }, h('h3', { text: 'Tree locks' }), status),
      body,
      h('div', { class: 'dlg-foot' }, h('span', { class: 'msg' }), h('button', { class: 'btn primary', text: 'Close', onclick: () => close() })),
    ));
    const act = (fn) => () => fn().then((done) => { if (done) paint(); }).catch(fail);
    // S06 (#129): one row per fault, current or latched; a latch names when it was recorded and
    // recedes a step below a fault the last comparison still sees.
    const faultRows = (faults) => faults.map((fault) => {
      const said = ZtLock.faultText(fault);
      return h('li', { class: fault.current ? 'fault-now' : 'fault-latched', title: [fault.expected && `expected ${fault.expected}`, fault.actual && `found ${fault.actual}`].filter(Boolean).join(' · ') || null },
        h('span', { class: 'a', text: said.kind }),
        said.where ? h('span', { class: 'b', text: said.where }) : null,
        h('span', { class: 'fault-when', text: said.when }));
    });
    const history = (view) => lockFold('History', view.history.map((event) => h('li', { class: 'ev ' + event.action, text: event.text })), 'lock-scroll');
    // S04 (#127): every Lock, active or ended, is the same index card with the same folds — its
    // note, seal and full digest, its Broken-link warnings, its members, its pins grouped by
    // target with their source addresses, and its whole history — each list whole and scrolling
    // in place. An ended Lock reads the snapshot its `lock` event captured, names its root by the
    // captured address (never today's Card there, R104), recedes a step and offers no action.
    // Reading, the members, the pins and the whole history stay on a faulted Lock; the seal shown
    // is the one the owner reviewed (`lock.seal`), never one computed from damaged content.
    const entry = (lock, faults, broken) => {
      const view = ZtLock.entryView(lock, false);
      const card = view.ended ? null : state.byAddress.get(lock.root);
      const run = {
        open: () => { close(); go(lock.root); },
        note: act(() => editLockNote(lock)),
        unlock: act(() => unlockTree(lock)),
      };
      const actions = ZtLock.entryActions(lock, Boolean(card)).map((item) => h('button', {
        class: 'btn' + (item.danger ? ' danger' : ''), text: item.label, dataset: { act: item.id }, onclick: run[item.id],
      }));
      return h('section', { class: 'lock-entry' + (view.ended ? ' lock-ended' : '') + (broken ? ' faulted' : '') + (!view.ended && lock.root === focusRoot ? ' focus' : ''), dataset: { lock: String(lock.lock_id) } },
        lockCardView({
          root: lock.root, title: card ? card.title : view.ended ? lock.root : `${lock.root} (root missing)`, kind: card ? card.kind : (/^\d+\//.test(lock.root) ? 'topic' : 'literature'),
          workspace: lock.workspace, count: lock.member_count, pins: view.pins.length,
          seal: lock.seal, broken, note: lock.note, since: lock.locked_at, lockId: lock.lock_id,
          until: lock.unlocked_at, noteless: true,
        }),
        faults.length ? h('ul', { class: 'lock-list lock-faults' }, faultRows(faults)) : null,
        view.ended && view.faultsRecorded
          ? h('p', { class: 'lock-recorded', text: `${view.faultsRecorded} integrity fault${view.faultsRecorded === 1 ? '' : 's'} recorded` }) : null,
        actions.length ? h('div', { class: 'lock-entry-actions' }, actions) : null,
        digestFold(lock.seal),
        warningFold(view.warnings),
        lockFold('Members', view.members.map((member) => h('li', {},
          h('span', { class: 'a', text: member.address }),
          h('span', { class: 'b fp', title: member.fingerprint, text: member.fingerprint.slice(0, 12) }))), 'lock-scroll'),
        lockFold('Pinned targets', pinRows(view.pins), 'lock-scroll'),
        history(view),
      );
    };
    async function paint() {
      try {
        await loadLocks();
      } catch (error) {
        body.replaceChildren(h('p', { class: 'msg err', text: error.message }));
        return;
      }
      const read = ZtLock.panel(state.locks);
      const faultsOf = (lock) => read.faultsOf[lock.lock_id] || [];
      status.className = 'lock-status' + (read.ok ? ' ok' : ' bad');
      status.textContent = read.ok ? 'all intact' : ZtLock.faultLine(state.locks);
      body.replaceChildren(...[
        read.loose.length ? h('ul', { class: 'lock-list lock-faults' }, faultRows(read.loose)) : null,
        read.active.length
          ? read.active.map((lock) => entry(lock, faultsOf(lock), Boolean(read.broken[lock.lock_id])))
          : h('p', { class: 'hint', text: 'No tree is locked. Open a Topic or Literature root and choose Lock tree to freeze it.' }),
        read.ended.length ? h('details', { class: 'lock-fold lock-history' },
          h('summary', { text: `Ended locks · ${read.ended.length}` }),
          h('div', { class: 'lock-ended-list' }, read.ended.map((lock) => entry(lock, faultsOf(lock), false)))) : null,
      ].flat().filter(Boolean));
      const focus = body.querySelector('.lock-entry.focus');
      if (focus) focus.scrollIntoView({ block: 'nearest' });
    }
    await paint();
  }

  /**
   * When a Locked tree's band moves (S05, #128). `paintLocks` hands every new Locks read to `take`;
   * a band that arrived with it is wrapped round the drawer or stack by motion the first time each
   * one is painted, and a released band slides off once. Any other repaint shows the band still.
   * The first read of the page sets the baseline, so nothing moves at boot.
   */
  const bands = {
    was: null,
    now: null,
    played: new Set(),
    painting: new Set(),
    frame: null,
    take(locks) {
      if (!locks) return;
      this.was = this.now;
      this.now = new Set(ZtLock.panel(locks).active.map((lock) => lock.root));
      this.played = new Set();
    },
    motion(where, root) {
      if (!this.was || !this.now) return null;
      const move = this.now.has(root) && !this.was.has(root) ? 'on' : !this.now.has(root) && this.was.has(root) ? 'off' : null;
      const key = `${where} ${root}`;
      if (!move || this.played.has(key)) return null;
      // A refresh may paint the rail twice before the screen shows it: the motion counts as played
      // only once a frame has been drawn, so the band that reaches the screen still moves.
      this.painting.add(key);
      this.frame = this.frame || requestAnimationFrame(() => {
        for (const done of this.painting) this.played.add(done);
        this.painting.clear();
        this.frame = null;
      });
      return move;
    },
  };

  /**
   * The paper band of a Locked tree: on the desk a band across the whole stack with `locked` and
   * the seal's 12 characters and a wax seal on the root's face; in the cabinet a strap across the
   * drawer's end (its count tab already says `locked`). A released band slides off, then goes.
   */
  function lockBand(where, tree, lock) {
    const motion = bands.motion(where, tree.root);
    // Without motion a released band has no slide to finish; it is simply gone.
    if (!tree.locked && (motion !== 'off' || reduceMotion())) return null;
    // `band-*` names only: `.drawer`, `.desk`, `.on` and `.off` are taken by the cabinet and desk.
    const band = h('span', { class: `lock-band band-${where}` + (motion ? ` band-${motion}` : ''), 'aria-hidden': where === 'drawer' ? 'true' : null },
      where === 'desk' ? [
        h('span', { class: 'lbl', text: 'locked' }),
        lock ? h('span', { class: 'v', text: `seal ${ZtLock.sealShort(lock.seal)}` }) : null,
      ] : null,
      h('span', { class: 'wax', 'aria-hidden': 'true' }));
    if (motion === 'off') {
      // A page that draws no frames (a hidden window) never ends the animation; the timer still
      // takes the released band away.
      band.addEventListener('animationend', () => band.remove());
      setTimeout(() => band.remove(), 600);
    }
    return band;
  }

  /**
   * One tree's cabinet menu (S05, #128): Open tree, then Lock tree…, or the active Lock's panel,
   * note and unlock — the same dialogs the root's lock line and the panel open. It opens on a
   * drawer or a desk stack by right-click, a long press, Shift+F10 or the menu key, beside the
   * pointer, in the history menu's paper; arrows walk it, Esc and a click elsewhere close it.
   */
  const treeMenu = (() => {
    let menu = null;
    const close = () => { if (menu) menu.remove(); menu = null; };
    document.addEventListener('pointerdown', (event) => { if (menu && !menu.contains(event.target)) close(); }, true);
    window.addEventListener('blur', close);

    async function run(id, tree) {
      if (id === 'open') { go(tree.root); openRail(false); return; }
      if (id === 'lock') { await lockTree(tree.root); return; }
      if (id === 'panel') { await lockPanel(tree.root); return; }
      // Note and unlock act on the exact active Lock, read fresh if the page has none in hand.
      let lock = ZtLock.activeLock(state.locks, tree.root);
      if (!lock) lock = ZtLock.activeLock(await loadLocks(), tree.root);
      if (!lock) throw new Error(`tree ${tree.root} is not locked any more`);
      if (id === 'note') await editLockNote(lock);
      else if (id === 'unlock') await unlockTree(lock);
    }

    function open(tree, x, y) {
      close();
      const rows = ZtLock.treeActions(tree.root, tree.locked, state.locks).map((item) => h('button', {
        class: 'nav-row' + (item.danger ? ' danger' : ''), role: 'menuitem', dataset: { act: item.id },
        onclick: () => { close(); run(item.id, tree).catch(fail); },
      }, h('span', { class: 't', text: item.label }), item.note ? h('span', { class: 'addr', text: item.note }) : null));
      menu = h('div', { class: 'nav-menu tree-menu', role: 'menu', 'aria-label': `Tree ${tree.root}` },
        h('div', { class: 'nav-menu-head', text: `${tree.kind === 'literature' ? 'Literature' : 'Topic'} ${tree.root}${tree.locked ? ' · locked' : ''}` }),
        rows);
      menu.addEventListener('keydown', (event) => {
        const items = [...menu.querySelectorAll('.nav-row')];
        const at = items.indexOf(document.activeElement);
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
          // The menu's arrows walk the menu only, never the Map behind it.
          event.preventDefault();
          event.stopPropagation();
          items[(at + (event.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length].focus();
        } else if (event.key === 'Escape' || event.key === 'Tab') {
          event.preventDefault();
          event.stopPropagation();
          close();
        }
      });
      document.body.append(menu);
      // Beside the pointer, kept inside the window.
      const box = menu.getBoundingClientRect();
      menu.style.left = `${Math.round(Math.max(8, Math.min(x, innerWidth - box.width - 8)))}px`;
      menu.style.top = `${Math.round(Math.max(8, Math.min(y, innerHeight - box.height - 8)))}px`;
      menu.querySelector('.nav-row').focus();
    }

    /** Give one drawer or stack its menu; answers the element. */
    function wire(el, tree) {
      let held = null;
      let opened = false;
      const at = () => { const box = el.getBoundingClientRect(); return [box.left + 14, box.bottom + 4]; };
      el.addEventListener('contextmenu', (event) => { event.preventDefault(); open(tree, event.clientX, event.clientY); });
      el.addEventListener('keydown', (event) => {
        if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) { event.preventDefault(); open(tree, ...at()); }
      });
      el.addEventListener('pointerdown', (event) => {
        if (event.button !== 0) return;
        opened = false;
        held = setTimeout(() => { opened = true; open(tree, ...at()); }, 500);
      });
      for (const name of ['pointerup', 'pointerleave', 'pointercancel']) el.addEventListener(name, () => clearTimeout(held));
      // A long press opened the menu: the click that ends it does not also open the tree.
      el.addEventListener('click', (event) => { if (opened) { opened = false; event.stopImmediatePropagation(); event.preventDefault(); } }, true);
      return el;
    }

    return { wire, close, open };
  })();

  /** The tree root's lock line in the reader: its state in words, and the one action that fits it. */
  function lockLine(card) {
    if (!state.locks) {
      return h('div', { class: 'lock-line' },
        actionButton('Lock tree…', null, () => lockTree(card.address), { cls: 'lock-btn', title: 'Review this whole tree, then freeze it' }));
    }
    const lock = ZtLock.activeLock(state.locks, card.address);
    if (!lock) {
      return h('div', { class: 'lock-line' },
        h('span', { class: 'lock-state', text: 'Unlocked tree' }),
        actionButton('Lock tree…', null, () => lockTree(card.address), { cls: 'lock-btn', title: 'Review this whole tree, then freeze it' }));
    }
    const broken = Boolean(ZtLock.panel(state.locks).broken[lock.lock_id]);
    return h('div', { class: 'lock-line locked' },
      h('span', { class: 'lock-state', title: 'Edit, type, move, delete, insert and new cards wait for an unlock; reading, linking, the Table and Reply stay open', text: `Locked since ${dateOf(lock.locked_at)} · writing waits for an unlock` }),
      sealStamp(lock.seal, broken),
      actionButton('Lock panel', null, () => lockPanel(card.address), { cls: 'lock-btn' }),
      actionButton('Unlock tree…', null, () => unlockTree(lock), { cls: 'lock-btn', danger: true }));
  }

  /**
   * A Card's own Lock facts under its actions, quiet (S03, #126): the Locked tree it belongs to and
   * since when, and the Locked trees that pin it. The server's `lock` and `pinned_by` come with the
   * Card itself, so they repaint with every refresh. A tree root's own Lock is its lock line above.
   */
  function cardLockLine(card) {
    const facts = ZtLock.cardState(card);
    if (!facts) return null;
    const member = facts.locked && !facts.root;
    if (!member && !facts.pinnedBy.length) return null;
    // A quiet link to one Card: the Locked root, or a source Card that holds a pinning Link.
    const quiet = (address) => h('button', { class: 'lock-open', text: address, title: `Open ${address}`, onclick: () => go(address) });
    // S05 (#128): the pinning Links' source Cards, from the Locks the page last read.
    const holders = facts.pinnedBy.length ? ZtLock.pinHolders(state.locks, card.address, facts.pinnedBy) : [];
    return h('div', { class: 'lock-line card-lock' },
      member ? h('span', { class: 'lock-state', title: facts.lockedWhy },
        'Locked with tree ', quiet(facts.tree),
        facts.since ? ` since ${dateOf(facts.since)}` : '',
        ' · writing waits for its unlock') : null,
      member ? actionButton('Lock panel', null, () => lockPanel(facts.tree), { cls: 'lock-btn', title: `Open the lock panel at tree ${facts.tree}; Unlock tree is there` }) : null,
      holders.length ? h('span', { class: 'lock-pinned', title: facts.pinnedWhy },
        `Pinned by locked ${holders.length === 1 ? 'tree' : 'trees'} `,
        holders.flatMap((holder, index) => [index ? '; ' : null, quiet(holder.tree),
          holder.sources.length ? [' via ', holder.sources.flatMap((source, at) => [at ? ', ' : null, quiet(source)])] : null]),
        ' · move, insert and delete wait; editing stays open') : null);
  }

  function showHelp() {
    const rows = COMMANDS.filter(([syntax, , scope]) => offered(syntax, scope));
    const on = (feature) => ZtFeatures.allows(state.features, feature);
    const keys = [': command', on('search') ? '/ or Ctrl+K search' : null, 'Alt+← Alt+→ back forward', 'g go',
      on('reply') ? 'n b e r m i' : 'n b e m i', 'Del delete', 'u up', '↑↓→ walk', 'v view', 'x unfold', 'p table',
      'P table with relations', 's study a mathematical unit (on the desk: ← → stations, Space turn over, 1 2 3 Learn Review Look up)',
      'w workspace', 'f fit', on('ask') ? 'ask the box, then y keeps the proposed Table and Esc cancels it' : null, 'Esc close'];
    listDialog('Commands', [
      h('p', { text: 'The command bar speaks the Session’s language:' }),
      h('pre', { text: rows.map(([syntax, desc]) => `${syntax.padEnd(30)} ${desc}`).join('\n') }),
      h('p', { text: 'Keys: ' + keys.filter(Boolean).join(' · ') }),
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
      if (!from.work_id) return;
      source.append(`From Scholar Archive · Work ${from.work_id} · `);
      // The figure's own place in the book it came from: the print goes back to its source.
      source.append(info.in_document
        ? h('a', { href: info.in_document, target: '_blank', rel: 'noopener noreferrer', text: 'open it in the book' })
        : (from.path || ''));
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
      const figures = h('div', { class: 'figure-grid' }, h('p', { class: 'figure-hint', text: 'Choose a Work to see its documents and figures.' }));
      const msg = h('span', { class: 'msg' });
      let done = false;
      const finish = (markdown) => { if (done) return; done = true; close(); resolve(markdown); };
      const close = openOverlay(h('div', { class: 'sheet-dlg figure-pick', role: 'dialog' },
        h('div', { class: 'dlg-head' }, h('h3', { text: 'From Scholar Archive' })),
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
        figures.replaceChildren(h('p', { class: 'figure-hint', text: 'Reading documents and figures…' }));
        try {
          const [listing, documents] = await Promise.all([
            api('scholar/artifacts?work_id=' + work.work_id),
            api('scholar/files?work_id=' + work.work_id).catch(() => ({ files: [] })),
          ]);
          if (ticket !== workTicket) return;
          const shown = listing.artifacts.filter((item) => item.shown && item.availability === 'present');
          const items = shown.length ? shown : listing.artifacts.filter((item) => item.availability === 'present');
          // The Work's documents sit above its figures: a Card links a whole document, or shows one figure.
          const shelf = documents.files.length ? h('div', { class: 'figure-docs' },
            h('span', { class: 'figure-docs-label', text: 'Link a document' }),
            documents.files.map((file) => h('button', {
              class: 'figure-doc', title: file.path, onclick: () => finish(file.markdown),
            }, h('span', { class: 'kind', text: file.kind === 'pdf' ? 'PDF' : file.bilingual ? 'MD 中英' : 'MD' }), h('span', { class: 'n', text: file.name })))) : null;
          if (!items.length) { figures.replaceChildren(...[shelf, h('p', { class: 'figure-hint', text: 'This Work\'s Markdown shows no figures.' })].filter(Boolean)); return; }
          figures.replaceChildren(...[shelf].filter(Boolean), ...items.map((item) => {
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
          msg.textContent = `${items.length} figure${items.length === 1 ? '' : 's'} · choose one to show it in the Card, or link a document`;
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
   *
   * `hub` opens the desk as the composer of one agent Hub Card draft (S17, #118, design §9.6): the
   * type is Hub and the Workspace is the agent's whatever the cabinet shows, the placement
   * controls choose the Hub level and the Card the draft is filed under, and the hub precheck runs
   * with every edit, so Save is offered only for a draft that would pass it. `hub.chosen` holds
   * the placement the reader chose, for the caller's own Save.
   */
  function openEditor({ heading, initial, literature, bibtex, save, bibtexCheck, cardType, facet, emphasis, level, reply, hub }) {
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
      const typePick = cardType && !hub ? h('div', { class: 'type-pick', role: 'radiogroup', 'aria-label': 'Card type' },        h('span', { class: 'lbl', text: 'Type' }),
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
      // Reply: the relation selector rewrites only the supplied prefix, never the reader's own
      // sentences, and the destination selector names the human Topic the reply is filed in.
      const replyBar = reply ? (() => {
        const { target, destinations, draft, kinds } = reply;
        const prefixOf = (index) => `${kinds[index][0]} [[${target.address}]]：`;
        const relationPick = h('select', {
          class: 'reply-kind', 'aria-label': 'Relation to the agent Card',
          title: 'The relation word supplied before the Link; your own sentences stay as you wrote them',
          onchange: (event) => {
            const index = Number(event.target.value);
            // The draft module owns the placement rule: replace the supplied prefix in place, or put
            // it back after the title when the reader removed it.
            text.value = ZtDraft.replySwapPrefix(text.value, prefixOf(draft.relation), prefixOf(index));
            draft.relation = index;
            dirty = true;
            update();
          },
        }, kinds.map(([word, note], index) => h('option', { value: String(index), text: word, title: note })));
        // S05 (#128): a human Topic inside a Locked tree takes no reply. It stays listed, marked
        // `locked` and not choosable; when it is the Topic the reader chose last, it stays chosen and
        // the note says why, so the page never silently files the reply somewhere else.
        const paintDestination = () => {
          const locked = draft.destination ? ZtLock.destinationLock(draft.destination) : null;
          draft.note.textContent = locked ? locked.why : '';
          draft.note.className = 'reply-note' + (locked ? ' err dest-locked' : '');
          return locked;
        };
        const destinationPick = h('select', {
          class: 'reply-dest', 'aria-label': 'Destination Topic',
          title: 'The human Topic this reply is filed in: a side successor of its last card, or the first direct successor of an empty Topic',
          onchange: (event) => {
            draft.destinationIndex = Number(event.target.value);
            draft.destination = destinations[draft.destinationIndex] || null;
            if (!paintDestination()) setMsg(draft.destination ? `filed in ${draft.destination.title} (${draft.destination.address})` : `filed in a new Topic ${reply.fallback}`);
          },
        },
          destinations.map((topic, index) => {
            const locked = ZtLock.destinationLock(topic);
            return h('option', {
              value: String(index), disabled: Boolean(locked), title: locked ? locked.why : null,
              text: `${topic.title || '(untitled)'} · ${topic.address}${locked ? ' · locked' : ''}`,
            });
          }),
          h('option', { value: '-1', text: `new Topic ${reply.fallback}` }),
        );
        destinationPick.value = String(draft.destination ? Math.max(0, destinations.indexOf(draft.destination)) : -1);
        paintDestination();
        relationPick.value = String(draft.relation);
        return h('div', { class: 'reply-bar' },
          h('span', { class: 'lbl', text: `Reply to ${target.address}` }),
          relationPick,
          h('span', { class: 'lbl', text: 'in' }),
          destinationPick,
          draft.note,
        );
      })() : null;
      // S17 (#118, design §9.6): a Spread filed as a Hub Card fixes the type and the Workspace and
      // leaves the placement to these controls. A rewrite goes back to the one hub it was drafted
      // from and nowhere else; a new draft may be filed at any destination a create allows.
      const hubOptions = !hub ? [] : hub.destination && hub.destination.how === 'edit'
        ? [hub.destination]
        : (hub.choices || []).filter((choice) => choice.how === 'side');
      const hubLevels = [...new Set(hubOptions.map((choice) => choice.level))];
      const hubPlaced = hub ? {
        level: hub.destination ? hub.destination.level : (hubLevels[0] || ''),
        how: hub.destination ? hub.destination.how : 'side',
        at: hub.destination ? hub.destination.at : '',
        base: hub.destination ? hub.destination.base || null : null,
      } : null;
      if (hub) hub.chosen = hubPlaced;
      const hubDestinations = () => hubOptions.filter((choice) => choice.level === hubPlaced.level);
      const hubLevelPick = hub ? h('select', {
        class: 'hub-level', 'aria-label': 'Hub level',
        title: 'How wide the subject is: one line of thought, one Topic tree, or several Topic trees',
        onchange: (event) => {
          hubPlaced.level = event.target.value;
          const [first] = hubDestinations();
          hubPlaced.at = first ? first.at : '';
          hubPlaced.base = first && first.base ? first.base : null;
          fillDestinations();
          dirty = true;
          update();
        },
      }, hubLevels.map((level) => h('option', { value: level, text: level }))) : null;
      const hubDestPick = hub ? h('select', {
        class: 'hub-dest', 'aria-label': 'Destination of the hub draft',
        title: 'The Card the hub is filed under: a side successor of a Card of the agent Workspace',
        onchange: (event) => {
          if (event.target.value === '') {
            hubPlaced.at = '';
            hubPlaced.base = null;
          } else {
            const choice = hubDestinations()[Number(event.target.value)];
            hubPlaced.at = choice ? choice.at : '';
            hubPlaced.base = choice && choice.base ? choice.base : null;
          }
          dirty = true;
          update();
        },
      }) : null;
      const hubFaults = hub ? h('div', { class: 'hub-faults', 'aria-live': 'polite' }) : null;
      // `keepEmpty` starts the select on its "where is it filed?" placeholder, which is how a plan
      // with no destination — a missing register — asks the reader for one inside the editor.
      const fillDestinations = (keepEmpty = false) => {
        if (!hubDestPick) return;
        const options = hubDestinations();
        const placeholder = keepEmpty ? [h('option', { value: '', text: '— where is it filed? —' })] : [];
        let index = options.findIndex((choice) => choice.at === hubPlaced.at);
        if (index < 0 && !keepEmpty && options.length) {
          hubPlaced.at = options[0].at;
          hubPlaced.base = options[0].base || null;
          index = 0;
        }
        hubDestPick.replaceChildren(...placeholder, ...(options.length
          ? options.map((choice, at) => h('option', { value: String(at), text: `${choice.title || '(untitled)'} · ${choice.at}`, title: choice.note || '' }))
          : [h('option', { value: '', text: 'no destination at this level' })]));
        hubDestPick.value = index >= 0 ? String(index) : '';
      };
      // Only the hub draft has a placement: every other desk (Edit, a new Card, Insert, Reply) opens
      // with `hubPlaced` null and must not read it (S24, #120: reading it broke every other desk).
      if (hub) {
        hubLevelPick.value = hubPlaced.level;
        fillDestinations(!hubPlaced.at);
      }
      const hubBar = hub ? h('div', { class: 'hub-bar' },
        h('span', { class: 'lbl', text: 'Hub Card' }),
        h('span', { class: 'hub-ws', text: 'agent workspace', title: 'A filed Spread becomes an agent Workspace Hub Card, whatever Workspace the cabinet shows; filing a Spread itself creates no Card' }),
        h('span', { class: 'lbl', text: 'level' }), hubLevelPick,
        h('span', { class: 'lbl', text: 'under' }), hubDestPick,
        h('span', { class: 'hub-note', text: hub.note || '' }),
      ) : null;
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
      // S17: the last hub precheck answer. Its faults are painted under the Markdown pane and Save
      // waits for a draft that would pass, so a hub is never filed with a fault the report names.
      let hubCheck = null;
      const paintHubCheck = () => {
        if (!hubFaults) return;
        const faults = (hubCheck && hubCheck.faults) || [];
        hubFaults.replaceChildren(...(faults.length
          ? faults.map((fault) => h('div', { class: 'hub-fault', dataset: { fault: fault.fault } },
            h('span', { class: 'f', text: fault.fault }), h('span', { class: 'd', text: fault.detail })))
          : [h('div', { class: 'hub-ok', text: hubCheck ? 'the hub precheck passes' : 'checking…' })]));
        saveBtn.disabled = faults.length > 0 || !hubCheck;
        if (faults.length) setMsg(`${faults.length} hub fault${faults.length === 1 ? '' : 's'} to fix before Save`, 'err');
      };
      const hubPrecheck = async () => {
        if (!hub) return;
        if (!hubPlaced.at) {
          hubCheck = {
            ok: false,
            faults: [{
              fault: 'destination',
              detail: !hubOptions.length
                ? 'the agent Workspace has no Card a hub can be filed under: create an agent Topic or publish the register first'
                : hub.missing === 'register'
                  ? 'the agent Workspace has no register, so a field hub has nowhere to hang; choose a destination for a topic or line hub'
                  : 'choose where the hub is filed',
            }],
          };
          paintHubCheck();
          return;
        }
        try {
          hubCheck = await hub.check(text.value, hubPlaced.level, hubPlaced.how, hubPlaced.at);
        } catch (error) {
          hubCheck = { ok: false, faults: [{ fault: 'destination', detail: error.message || String(error) }] };
        }
        paintHubCheck();
      };
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
          await hubPrecheck();
        } catch (error) { setMsg(error.message, 'err'); }
      }, 160);

      const close = openOverlay(h('div', { class: 'sheet-dlg', role: 'dialog' },
        h('div', { class: 'dlg-head' }, h('h3', { text: heading }), typePick),
        replyBar,
        hubBar,
        h('div', { class: 'dlg-body' },
          h('div', { class: 'pane' + (hub ? ' hub-pane' : '') },
            bib ? h('div', { class: 'pane-label' }, h('span', { text: 'BibTeX metadata' })) : null,
            bib,
            h('div', { class: 'pane-label' }, h('span', { text: 'Markdown' }), h('span', {},
              h('button', { class: 'figure-btn', type: 'button', title: 'Show a figure of a Scholar Archive Work in this Card, or link one of its documents', onclick: async () => {
                const markdown = await pickFigure();
                if (!markdown) { text.focus(); return; }
                const at = text.selectionStart;
                const before = text.value.slice(0, at);
                const lead = !before || before.endsWith('\n\n') ? '' : before.endsWith('\n') ? '\n' : '\n\n';
                text.setRangeText(lead + markdown + '\n\n', at, text.selectionEnd, 'end');
                text.focus();
                dirty = true;
                update();
              } }, '＋ Figure or document'),
              ' · [[ links · text below the reverse-links marker is generated')),
            text,
            hub ? h('div', { class: 'hub-check' }, h('div', { class: 'pane-label' }, h('span', { text: 'Hub precheck' })), hubFaults) : null,
          ),
          h('div', { class: 'pane' }, h('div', { class: 'pane-label' }, h('span', { text: 'Preview' })), preview),
        ),
        h('div', { class: 'dlg-foot' }, msg, cancelBtn, saveBtn),
        completion,
      ));

      const finish = (address) => { close(); resolve(address); };
      const doSave = async () => {
        if (hub && !(hubCheck && hubCheck.ok)) {
          setMsg('the hub precheck must pass before Save; fix the named faults', 'err');
          return;
        }
        saveBtn.disabled = true;
        try {
          const address = await save(text.value, bib ? bib.value : undefined, chosenType, chosenType === 'model' ? chosenFacet : '', chosenType === 'model' && chosenFacet ? chosenEmphasis : '', chosenType === 'model' && !chosenFacet ? chosenLevel : '');
          if (address !== undefined) finish(address);
        } catch (error) {
          // The draft stays in the desk: a refused Save (a Tree Lock refusal included) only says why.
          setMsg(error.message, 'err');
          if (error.refusal) {
            msg.append(' ', h('button', {
              class: 'lock-open', type: 'button', text: 'Lock panel',
              title: 'Open the lock panel over this desk; your text stays here',
              // The panel opens at the refused tree's Lock (S05, #128), its Unlock tree in reach.
              onclick: () => lockPanel(ZtLock.refusalTree(error.refusal)).catch(fail),
            }));
          }
        } finally { saveBtn.disabled = hub ? !(hubCheck && hubCheck.ok) : false; }
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
    if (kind !== 'topic') gateWrite(parent, kind);
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

  // A Reply answers one agent Card from the human Workspace (S18, #99). The four words are the
  // Relation kinds a Link can announce; the selector supplies the prefix, and changing it never
  // rewrites text the reader composed. The reply is filed in a human Topic, never under the agent
  // Card: its Link and the generated Reverse link carry the connection across the Workspaces.
  const REPLY_KINDS = [
    ['印证', 'the card shows or supports this'],
    ['反驳', 'this counters the card'],
    ['放宽', 'this relaxes one of its assumptions'],
    ['对应', 'the same mechanism holds elsewhere'],
  ];
  const REPLY_DEFAULT_TOPIC = '回应';

  /** The human Topic the reader replied in last, while it still exists. */
  function replyRecentTopic() {
    const wanted = store.get('zt-reply-topic');
    if (!wanted) return null;
    return (state.nodes || []).find((node) => node.address === wanted && node.kind === 'topic' && node.workspace === 'human') || null;
  }

  /** The human Topics a reply may be filed in, the last used one first. */
  function replyDestinations() {
    const recent = replyRecentTopic();
    const topics = (state.nodes || [])
      .filter((node) => node.kind === 'topic' && node.workspace === 'human')
      .sort((a, b) => String(a.address).localeCompare(String(b.address), undefined, { numeric: true }));
    const list = [];
    if (recent) list.push(recent);
    for (const topic of topics) if (!list.some((seen) => seen.address === topic.address)) list.push(topic);
    return list;
  }

  /**
   * Reply to one agent Card: the Card the reader has open, or the distant Card the slot names.
   * The human Card is written in the writing desk and filed in a human Topic.
   */
  async function replyTo(card = state.card) {
    if (!ZtFeatures.allows(state.features, 'reply')) throw new Error(ZtFeatures.offNote('reply'));
    if (!card) throw new Error('r needs a Card; use go <target> first');
    if (card.workspace !== 'agent') {
      throw new Error('Reply answers an agent Card; open one of the agent workspace first (w switches workspace)');
    }
    if (!card.authored_fingerprint) throw new Error('this Card view carries no authored fingerprint; reload the page and try again');
    const target = { address: card.address, title: card.title || '', fingerprint: card.authored_fingerprint };
    const destinations = replyDestinations();
    // The last Topic used wins; with none, the desk offers the default Topic or creating it on Save.
    const recent = replyRecentTopic();
    const chosen = ZtDraft.replyDestination(destinations, recent ? recent.address : null, REPLY_DEFAULT_TOPIC);
    const draft = {
      destination: chosen,
      destinationIndex: chosen ? destinations.indexOf(chosen) : -1,
      relation: 0,
      note: h('span', { class: 'reply-note' }),
    };
    const address = await openEditor({
      heading: 'Reply · human workspace',
      initial: ZtDraft.replyDraft(REPLY_KINDS[draft.relation][0], target.address),
      cardType: null,
      reply: { target, destinations, draft, kinds: REPLY_KINDS, fallback: REPLY_DEFAULT_TOPIC },
      save: async (text) => {
        const result = await api('reply', {
          at: target.address,
          base: target.fingerprint,
          text,
          topic: draft.destination ? draft.destination.address : undefined,
        });
        if (result.saved) {
          // Remember the destination only after the save succeeded, so a refused reply leaves the
          // previous preference alone.
          store.set('zt-reply-topic', result.destination.topic);
          return result.address;
        }
        // A refusal keeps the draft: show the fresh context the reader must decide against, take its
        // new fingerprint so a renewed Save is checked against the Card they now see, and stop.
        if (result.target && result.reason === 'target_changed') {
          target.fingerprint = result.target.authored_fingerprint;
          target.title = result.target.title;
        }
        draft.note.textContent = result.detail || 'the reply was not saved';
        draft.note.className = 'reply-note err';
        // A destination locked since the desk opened (S05, #128): the draft and the chosen Topic
        // stay as they are; the refusal names the Locked tree, so the desk offers its lock panel.
        // Nothing retries on its own and nothing is unlocked.
        const error = new Error(result.detail || 'the reply was not saved');
        if (result.reason === 'locked_destination' && result.refusal) error.refusal = result.refusal;
        throw error;
      },
    });
    if (!address) return;
    await afterChange(address, `replied to ${target.address}`);
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
    gateWrite(card, 'edit');
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

  /** `ty <idea|event|finding|model|hub> [<facet>]`: change only the current Regular Card's type and facet. */
  async function setCardType(type, facet) {
    const card = state.card;
    if (!CARD_TYPES[type]) throw new Error(`unknown card type \`${type}\`; expected idea, event, finding, model, or hub`);
    if (card.context !== 'regular') throw new Error('only Regular Cards can be Event, Finding, Model, or Hub Cards');
    if (facet && type !== 'model') throw new Error('only Model Cards can have a Model facet');
    gateWrite(card, 'type');
    const result = await api('edit', { at: card.address, type, facet: type === 'model' ? facet || '' : undefined });
    await afterChange(result.address, facet
      ? `${result.address} is the ${facetLabel(facet)} part of its model`
      : `${result.address} is ${type === 'idea' ? 'an Idea' : type === 'event' ? 'an Event' : type === 'finding' ? 'a Finding' : type === 'hub' ? 'a Hub' : 'a Model'} Card`);
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
    gateWrite(card, 'bibtex');
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
    gateWrite(card, 'delete');
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
    gateWrite(card, 'move');
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
    gateWrite(card, 'insert');
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
          if (!offered(syntax, scope)) continue;
          const name = syntax.split(' ')[0];
          if (!word || name.startsWith(word)) {
            const takesArg = syntax.includes('<');
            // S05 (#128): a write the open Card's Lock state refuses stays listed, marked and
            // explained; choosing it shows why and offers the tree's lock panel.
            const write = ZtLock.COMMAND_WRITES[name];
            const gate = write && state.card ? ZtLock.writeGate(state.card, write, state.locks) : null;
            const gated = gate && !gate.allowed ? gate : null;
            list.push({
              syntax, desc: gated ? `unavailable — ${gated.short}` : desc, gated: gated ? gated.why : null, gate: gated, group: 'Commands',
              fill: takesArg && !trimmed.includes(' ') && !gated ? name + ' ' : null, run: takesArg && !gated ? null : name,
            });
          }
        }
        if (ZtFeatures.allows(state.features, 'search') && trimmed && !list.some((item) => item.syntax.split(' ')[0] === word) && trimmed.length > 1) {
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
          class: (index === selected ? 'on' : '') + (item.gated ? ' gated' : ''), role: 'option',
          title: item.gated, 'aria-disabled': item.gated ? 'true' : null,
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
      if (item.gate) {
        // A gated write says why at once, whatever arguments it would take.
        input.value = '';
        menu.hidden = true;
        input.blur();
        fail(Object.assign(new Error(item.gate.why), { gate: item.gate }));
        return;
      }
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
      // The study desk takes the keys while it lies open.
      if (study.isOpen()) { study.key(event); return; }
      // A key of a reader feature the service has off (`/`, Ctrl+K, `r`) does nothing (S28, #121).
      if (!ZtFeatures.allows(state.features, ZtFeatures.forKey(event.key, event.ctrlKey || event.metaKey))) return;
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
        case 'r': return card && card.workspace === 'agent' ? run(replyTo) : undefined;
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
        case 'y': return spreads.previewing() ? run(keepAskSpread) : undefined;
        case 'f': return run(() => fitView());
        case 'x': return state.card && state.mode === 'map' ? run(() => setExpandAll(!state.expandAll)) : undefined;
        case '?': return run(showHelp);
        case 's': return isStudied(card) ? run(() => study.open(card.address)) : undefined;
        case 'Escape': return run(() => (spreads.previewing() ? cancelAskSpread() : pullout.node ? dropBack() : openRail(false)));
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

  // ------------------------------------------------------------- features
  // S28 (#121): `zt config set gui_search|gui_ask|gui_reply off` turns one reader feature off. The
  // page takes the states from every overview read and removes that feature's controls whole, so
  // nothing stands empty where they were; Cards, the Map and the Table stay as they are.
  const COMMAND_PLACEHOLDER = $('#command-input').getAttribute('placeholder') || '';

  /** Show only the controls of the features that are on. */
  function paintFeatures() {
    const on = (feature) => ZtFeatures.allows(state.features, feature);
    $('#search-open').closest('.rail-search').hidden = !on('search');
    $('#ask-open').hidden = !on('ask');
    $('#command-input').setAttribute('placeholder', on('search') ? COMMAND_PLACEHOLDER : COMMAND_PLACEHOLDER.replace(/\s*— or search$/, ''));
  }

  /** Take the feature states of one overview answer; repaint only when a switch moved. */
  function applyFeatures(overview) {
    const next = ZtFeatures.read(overview);
    if (!ZtFeatures.changed(state.features, next)) return;
    const replyMoved = state.features.reply !== next.reply;
    state.features = next;
    paintFeatures();
    if (!next.search && finder.isOpen) finder.close();
    if (replyMoved && state.card) { renderReader(); paintDistant(); }
  }

  // ------------------------------------------------------------------ boot
  async function watchForChanges() {
    if (document.hidden || state.editing) return;
    try {
      const overview = await api('overview');
      // A switch turned off or back on shows on this poll, even when no Card changed.
      applyFeatures(overview);
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
    $('#ask-open').addEventListener('click', () => askBox('').catch(fail));
    $('#ask-keep').addEventListener('click', () => keepAskSpread().catch(fail));
    $('#ask-cancel').addEventListener('click', () => cancelAskSpread());
    for (const button of document.querySelectorAll('[data-arrange]')) {
      button.addEventListener('click', () => tableView.arrange(button.dataset.arrange));
    }
    $('#quit').addEventListener('click', () => quit().catch(fail));
    $('#broken-pill').addEventListener('click', () => showBroken().catch(fail));
    $('#lock-pill').addEventListener('click', () => lockPanel().catch(fail));
    $('#locks-open').addEventListener('click', () => lockPanel().catch(fail));
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
      // The Locks come with the first read, so a locked root opens with its state; a failed lock
      // read never keeps the page from booting (the panel says why when it is opened).
      const [overview, graph, kept, locks] = await Promise.all([api('overview'), api('graph'), api('spreads'), api('locks').catch(() => null)]);
      state.overview = overview;
      state.fingerprint = overview.fingerprint;
      applyFeatures(overview);
      setNodes(graph.nodes);
      spreads.take(kept);
      state.locks = locks;
      paintLocks();
      route();
    } catch (error) {
      fail(error);
    }
  }

  boot();
})();
