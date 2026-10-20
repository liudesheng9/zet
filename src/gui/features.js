/* zt gui features — pure, DOM-free rules for the three reader feature switches (S28, #121).
 *
 * `zt config set gui_search|gui_ask|gui_reply on|off` turns one reader feature off in the GUI;
 * `GET /api/overview` reports the states as `features:{search,ask,reply}`, read fresh on every
 * answer. These rules decide what the page takes from that answer, which command and which key
 * belong to which feature, and what the page says when the reader names a feature that is off.
 * `draft.js` and `ask.js` export the same way, so this module runs in the page and under Node.
 */
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.ZtFeatures = api;
})(typeof self !== 'undefined' ? self : this, () => {
  'use strict';

  /** The three switchable reader features and the config key of each. */
  const KEYS = { search: 'gui_search', ask: 'gui_ask', reply: 'gui_reply' };
  const NAMES = { search: 'Search', ask: 'Ask the box', reply: 'Reply' };

  /**
   * The page's feature state from one overview answer. A feature is off only when the service
   * says exactly `false`; a missing `features` object (an older service) or a missing or odd value
   * leaves it on, as the service's own config line does.
   */
  function read(overview) {
    const given = overview && typeof overview.features === 'object' && overview.features ? overview.features : {};
    const state = {};
    for (const name of Object.keys(KEYS)) state[name] = given[name] !== false;
    return state;
  }

  /** Whether two feature states differ, so the page repaints only on a real switch. */
  function changed(before, after) {
    return Object.keys(KEYS).some((name) => !!(before && before[name]) !== !!(after && after[name]));
  }

  /** The feature a command-bar word belongs to (`ask`, `reply`), else `null`. */
  function forCommand(word) {
    if (word === 'ask') return 'ask';
    if (word === 'reply') return 'reply';
    return null;
  }

  /**
   * The feature a key belongs to: `/` and Ctrl+K (or Cmd+K) open search, `r` replies; any other
   * key, and `r` with a modifier (Ctrl+R reloads), belongs to none.
   */
  function forKey(key, modified) {
    const lower = String(key || '').toLowerCase();
    if (modified) return lower === 'k' ? 'search' : null;
    if (key === '/') return 'search';
    if (key === 'r') return 'reply';
    return null;
  }

  /** Whether the state allows a feature; `null` (no feature) is always allowed. */
  function allows(state, feature) {
    if (!feature) return true;
    return !state || state[feature] !== false;
  }

  /** What the page says when the reader names a feature that is off, with the command that turns it on. */
  function offNote(feature) {
    return `${NAMES[feature] || feature} is turned off in this GUI; \`zt config set ${KEYS[feature] || feature} on\` turns it back on`;
  }

  return { KEYS, read, changed, forCommand, forKey, allows, offNote };
});
