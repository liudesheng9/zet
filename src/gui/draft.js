/* zt gui draft — pure, DOM-free text rules for the writing desk.
 *
 * The Reply desk supplies a relation word and a Link to the agent Card, then the reader writes
 * around that skeleton. These rules keep the skeleton in the draft without touching a sentence
 * the reader composed. `layout.js` exports the same way, so this module runs in the page and
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

  return { replyDraft, replySwapPrefix, replyDestination };
});
