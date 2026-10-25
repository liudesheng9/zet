/* zt gui draft — pure, DOM-free text rules for the writing desk.
 *
 * The Reply desk supplies a relation word and a Link to the agent Card, then the reader writes
 * around that skeleton. These rules keep the skeleton in the draft without touching a sentence
 * the reader composed. Since S04 (#146) the module also reads a write's Card check: the save
 * note of an accepted write and the fault lines of a refused one. `layout.js` exports the same way, so this module runs in the page and
 * under Node.
 */
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.ZtDraft = api;
})(typeof self !== 'undefined' ? self : this, () => {
  'use strict';

  /** The marker the generated Reverse link section follows; the reader never writes it. */
  const MARKER = '<!-- zt:reverse-links -->';

  /** The text the writing desk opens with: an empty claim title, then the supplied prefix. */
  const replyDraft = (word, target) => `# \n\n${word} [[${target}]]：\n\n${MARKER}\n`;

  /**
   * Replace the supplied prefix in place, wherever the reader moved it, and return the new text.
   * When the reader removed it, put the chosen prefix back at the start of the body — after the
   * title heading, never above it. The title and every composed sentence stay as they are.
   */
  const replySwapPrefix = (value, before, after) => {
    const at = value.indexOf(before);
    if (at >= 0) return value.slice(0, at) + after + value.slice(at + before.length);
    const line = value.indexOf('\n');
    if (line < 0) return value ? `${value}\n\n${after}\n\n` : `${after}\n\n`;
    const blank = value[line + 1] === '\n';
    const head = value.slice(0, blank ? line + 2 : line + 1);
    const rest = value.slice(blank ? line + 2 : line + 1);
    return `${head}${blank ? '' : '\n'}${after}\n\n${rest}`;
  };

  /**
   * The human Topic a reply is filed in: the one the reader used last while it still exists, else
   * the Topic titled with the default (`回应`), else `null` — the desk then offers to create the
   * default Topic on Save. A vanished preference is never followed by address.
   */
  const replyDestination = (topics, recent, defaultTitle) => {
    const list = topics || [];
    const used = recent ? list.find((topic) => topic.address === recent) : null;
    if (used) return used;
    return list.find((topic) => String(topic.title || '').trim() === defaultTitle) || null;
  };

  // ------------------------------------------------------------- the write check (S04, #146)
  // Every authored write passes the archive's Card check. A saved write answers `warnings`
  // (`{rule, fault, message, legacy}`) and `check {complete, rule_version, policy_hash, stage}`; a
  // blocking check answers the typed `card_check` (or `check_incomplete`) object instead. These
  // rules turn both into what the desk and the save note show, most important first.

  /** How many advice lines the save note shows before it folds the rest under `+N`. */
  const NOTE_SHOWN = 3;

  /** One line of text: white space collapsed, at most `limit` characters with `…`. */
  const oneLine = (value, limit) => {
    const text = String(value == null ? '' : value).replace(/\s+/g, ' ').trim();
    const chars = Array.from(text);
    return chars.length > limit ? chars.slice(0, limit - 1).join('') + '…' : text;
  };

  /**
   * The save note of one accepted write, or `null` when the answer ran no check. New advice comes
   * before the advice the Card already carried (`legacy`), so the eye lands on what this save added.
   */
  const checkNote = (result) => {
    if (!result || !Array.isArray(result.warnings) || !result.check || typeof result.check !== 'object') return null;
    const line = (warning) => ({
      rule: String(warning.rule || ''),
      fault: String(warning.fault || ''),
      text: oneLine(warning.message, 140),
      legacy: !!warning.legacy,
    });
    const fresh = result.warnings.filter((warning) => warning && !warning.legacy).map(line);
    const legacy = result.warnings.filter((warning) => warning && warning.legacy).map(line);
    const lines = [...fresh, ...legacy];
    const stage = result.check.stage || null;
    const parts = [];
    if (!lines.length) parts.push('no advice');
    else {
      parts.push(`${lines.length} advice`);
      if (legacy.length) parts.push(fresh.length ? `${fresh.length} new, ${legacy.length} already there` : 'all already there');
    }
    return {
      stage,
      fresh: fresh.length,
      legacy: legacy.length,
      summary: `check${stage ? ' · ' + stage : ''} · ${parts.join(' · ')}`,
      versions: `zt-check ${result.check.rule_version || '?'} · policy ${String(result.check.policy_hash || '?').slice(0, 12)}`,
      lines,
      shown: lines.slice(0, NOTE_SHOWN),
      folded: lines.slice(NOTE_SHOWN),
    };
  };

  /**
   * A refused write's lines, or `null` when the error is not a Card check. Blocking faults come
   * first and carry the fragment to find in the draft; advice follows, to be folded. A check that
   * could not finish has no lines, only its reason.
   */
  const checkRefusal = (check) => {
    if (!check || typeof check !== 'object') return null;
    if (check.error === 'check_incomplete') {
      return {
        incomplete: true,
        blocking: [],
        advice: [],
        headline: 'The check could not finish, so nothing was saved; the draft is kept. Save again later.',
        detail: oneLine(check.message || check.reason || '', 200),
      };
    }
    if (check.error !== 'card_check') return null;
    const diagnostics = Array.isArray(check.diagnostics) ? check.diagnostics.filter(Boolean) : [];
    const lines = diagnostics.length
      ? diagnostics.map((diagnostic) => ({
        rule: String(diagnostic.rule || ''),
        fault: String(diagnostic.fault || ''),
        text: oneLine(diagnostic.message, 160),
        find: String(diagnostic.key || diagnostic.message || ''),
        message: String(diagnostic.message || ''),
        blocking: diagnostic.severity === 'block' && !diagnostic.legacy,
        legacy: !!diagnostic.legacy,
      }))
      // An answer without structured diagnostics still carries its human lines `<address> <rule> <text>`.
      : (Array.isArray(check.faults) ? check.faults : []).map((fault) => {
        const words = String(fault).split(' ');
        const advisory = words[2] === '(advisory)';
        const text = words.slice(advisory ? 3 : 2).join(' ');
        return { rule: words[1] || '', fault: '', text: oneLine(text, 160), find: text.replace(/…$/, ''), message: text, blocking: !advisory, legacy: false };
      });
    let blocking = lines.filter((line) => line.blocking);
    let advice = lines.filter((line) => !line.blocking);
    // A refusal always names what blocked it; if no line is marked so, every line is shown as blocking.
    if (!blocking.length && Number(check.blocking) > 0) { blocking = advice; advice = []; }
    const count = Number(check.blocking) || blocking.length;
    return {
      incomplete: false,
      blocking,
      advice,
      headline: `${count} blocking fault${count === 1 ? '' : 's'}: nothing was saved and the draft is kept. Click a line to find it, fix it, then Save again.`,
      detail: '',
    };
  };

  /**
   * Where a fault line's fragment sits in the draft: `{start, end}`, or `null` when the draft does
   * not hold it as written (the check reads the plain sentence, the draft may hold its Markdown).
   * Tries the full fragment, the sentence after a measure prefix (`… > 60: <sentence>`), then the
   * fragment's first clause.
   */
  const locateFault = (text, line) => {
    const value = String(text || '');
    if (!line || !value) return null;
    const tries = [];
    const add = (candidate) => {
      const trimmed = String(candidate || '').trim();
      if (trimmed && !tries.includes(trimmed)) tries.push(trimmed);
    };
    add(line.find);
    for (const source of [line.find, line.message]) {
      const raw = String(source || '');
      const colon = raw.search(/[:：] ?/);
      if (colon >= 0) add(raw.slice(colon + 1));
      const paren = raw.match(/^\([^)]*\)\s*(.+)$/);
      if (paren) add(paren[1]);
    }
    add(line.message);
    for (const candidate of [...tries]) {
      const clause = candidate.split(/[，,；;。.!?！？]/)[0];
      if (Array.from(clause.trim()).length >= 4) add(clause);
    }
    for (const candidate of tries) {
      const start = value.indexOf(candidate);
      if (start >= 0) return { start, end: start + candidate.length };
    }
    return null;
  };

  // ------------------------------------------------------------- Term Cards (S14, #161)
  // A Term Card is the sixth Card type: with no name it is a Term list, with a name it defines
  // that term. The desk's term-name field and its `[[` completion follow these rules; the server
  // (`src/term.rs`) stays the authority and refuses what they let through.

  /** The longest term name and Term-use label (mirrors `MAX_TERM_NAME`): Unicode scalar values. */
  const TERM_NAME_MAX = 40;

  /**
   * Why a typed term name cannot be saved, or `null` when it can. An empty name is no fault: it
   * makes the Card a Term list. Mirrors `term::normalize_name`.
   */
  const termNameIssue = (name) => {
    const value = String(name || '').trim();
    if (!value) return null;
    if (/[\r\n]/.test(value)) return 'a term name must be one line';
    if (Array.from(value).length > TERM_NAME_MAX) return `a term name holds at most ${TERM_NAME_MAX} characters`;
    if (/[[\]@|`]/.test(value)) return 'a term name cannot hold [ ] @ | or `';
    return null;
  };

  /**
   * The `term` field one desk save sends. Only a Term Card sends one, so no other type ever
   * carries a name (retyping a Term Card away clears its name on the server). A create or an
   * insert sends a name only when there is one (none: a Term list); an edit always sends it, so an
   * emptied field clears the name and the Card becomes a Term list.
   */
  const termField = (type, name, how) => {
    if (type !== 'term') return undefined;
    const value = String(name || '').trim();
    if (how === 'edit') return value;
    return value || undefined;
  };

  /** The text typed after an unclosed `[[` before the caret, or `null` when there is none. */
  const linkTyped = (before) => {
    const match = String(before || '').match(/\[\[([^[\]\n]{0,80})$/);
    return match ? match[1] : null;
  };

  /** Natural address order: `2/10` after `2/9`. */
  const byAddress = (a, b) => String(a.address).localeCompare(String(b.address), undefined, { numeric: true });

  /**
   * The rows `[[` completion offers for the typed text, in two groups.
   *
   * - `term` rows: a Term use. Only a Term head (a named Term Card filed as a Side successor of a
   *   list: `term_role` `card`) can be used, so only heads are offered, searched by their term
   *   name: an exact name first, then a name that starts with the text, then one that holds it.
   *   Each row names its book (the tree and its root title) and its address, so two heads of one
   *   name are told apart, and inserts `name@address` exactly: the head's own name and address.
   *   Typing `word@` first keeps the reader's own word as the label and narrows to the heads whose
   *   name and word overlap, the text after `@` searching their addresses and trees.
   * - `link` rows: an ordinary reasoned Link `[[address]]`, by address, title, or term name. A
   *   head offered as a Term use is always offered here too, first, since a claim about a
   *   definition is a reasoned Link to it and not a use of the word.
   *
   * `limit` bounds the Link group; Term uses get at most half of it.
   */
  const linkCompletions = (nodes, typed, limit = 8) => {
    const list = Array.isArray(nodes) ? nodes : [];
    const raw = String(typed || '');
    const at = raw.indexOf('@');
    const word = at >= 0 ? raw.slice(0, at).trim() : null;
    const query = (at >= 0 ? raw.slice(at + 1) : raw).trim().toLowerCase();
    const books = new Map();
    for (const node of list) if (node.edge === 'root') books.set(node.tree, node.title || '');
    const book = (node) => ({ tree: node.tree || '', title: books.get(node.tree) || '' });
    const heads = list.filter((node) => node.type === 'term' && node.term_role === 'card' && node.term);
    const rank = (name, text) => (name === text ? 0 : name.startsWith(text) ? 1 : 2);
    let used = [];
    if (word !== null) {
      const w = word.toLowerCase();
      used = heads
        .filter((node) => {
          const name = node.term.toLowerCase();
          return (!w || name.includes(w) || w.includes(name))
            && (!query || node.address.toLowerCase().includes(query) || String(node.tree || '').toLowerCase().includes(query));
        })
        .map((node) => ({ node, order: w ? rank(node.term.toLowerCase(), w) : 2 }));
    } else if (query) {
      used = heads
        .filter((node) => node.term.toLowerCase().includes(query))
        .map((node) => ({ node, order: rank(node.term.toLowerCase(), query) }));
    }
    used.sort((a, b) => a.order - b.order || byAddress(a.node, b.node));
    const termRows = used.slice(0, Math.max(1, Math.floor(limit / 2))).map(({ node }) => {
      const label = word || node.term;
      return { kind: 'term', address: node.address, term: node.term, label, insert: `${label}@${node.address}`, title: node.title || '', book: book(node) };
    });
    if (word !== null) return termRows;
    const seen = new Set();
    const linkRows = [];
    const cite = (node) => {
      if (!node || seen.has(node.address) || linkRows.length >= limit) return;
      seen.add(node.address);
      linkRows.push({ kind: 'link', address: node.address, term: node.term || null, label: null, insert: node.address, title: node.title || '', book: book(node) });
    };
    for (const row of termRows) cite(list.find((node) => node.address === row.address));
    for (const node of list) {
      if (node.address.toLowerCase().includes(query)
        || String(node.title || '').toLowerCase().includes(query)
        || (node.term && node.term.toLowerCase().includes(query))) cite(node);
    }
    return [...termRows, ...linkRows];
  };

  return { replyDraft, replySwapPrefix, replyDestination, checkNote, checkRefusal, locateFault, TERM_NAME_MAX, termNameIssue, termField, linkTyped, linkCompletions };
});
