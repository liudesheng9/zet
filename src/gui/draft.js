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

  return { replyDraft, replySwapPrefix, replyDestination, checkNote, checkRefusal, locateFault };
});
