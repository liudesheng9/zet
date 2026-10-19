/* zt gui ask — pure, DOM-free rules for an answered question and the Spread kept from it.
 *
 * The page asks `POST /api/ask`, shows the proposal on a temporary Table, and keeps it through
 * the ordinary `/api/spreads` save only when the reader chooses Keep. These rules decide what the
 * question may be, what the temporary Table holds, what Keep writes, and which proposed endpoints
 * changed since the answer. `draft.js` and `layout.js` export the same way, so this module runs in
 * the page and under Node.
 */
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.ZtAsk = api;
})(typeof self !== 'undefined' ? self : this, () => {
  'use strict';

  /** The question path's own bound (`ASK_QUESTION_MAX_CHARS`, D228), counted as characters. */
  const QUESTION_MAX = 512;
  /** The Spread schema's name limit (D233). */
  const NAME_MAX = 80;

  /** A string's characters, as Rust's `chars()` counts them: one per code point, never half a pair. */
  const characters = (value) => Array.from(String(value == null ? '' : value));

  /** The display name of a kept Spread: trimmed, then cut to the schema's 80 characters. */
  const askName = (value) => characters(String(value == null ? '' : value).trim()).slice(0, NAME_MAX).join('');

  /**
   * The question a reader typed, checked exactly as `AskArgs::from_args` checks it: nonblank and at
   * most 512 Unicode characters. The answer carries the trimmed question and the display name the
   * kept Spread takes, so the whole question survives in `question` while only the name is cut.
   */
  const askQuestion = (raw) => {
    const text = String(raw == null ? '' : raw).trim();
    if (!text) {
      return { ok: false, error: 'type a nonblank question to ask the box' };
    }
    const count = characters(text).length;
    if (count > QUESTION_MAX) {
      return { ok: false, error: `a question is at most ${QUESTION_MAX} characters; this one has ${count}` };
    }
    return { ok: true, question: text, name: askName(text) };
  };

  /** A number a Spread can keep: finite, inside the schema's own range, rounded like the server. */
  const coord = (value) => {
    const number = Number(value);
    if (!Number.isFinite(number)) return 0;
    return Math.round(Math.max(-1.0e6, Math.min(1.0e6, number)) * 10) / 10;
  };

  /**
   * The temporary Table one answered question decks (D232, D234): the proposed Cards at the places
   * the answer suggested, each with the reason it was chosen and the authored fingerprint the Keep
   * check re-reads. Its `id` names the question, so answering another question decks another Table
   * and asking the same one gives the same Table. Nothing here is a saved Spread, and the answer's
   * own `held` Card is the one the Table squares up and lifts.
   */
  const askPreview = (proposal) => {
    if (!proposal || typeof proposal !== 'object') {
      throw new Error('the ask answer is not a proposal');
    }
    const spread = proposal.spread && typeof proposal.spread === 'object' ? proposal.spread : {};
    const question = String(spread.question || proposal.question || '');
    const byAddress = new Map();
    for (const card of Array.isArray(proposal.cards) ? proposal.cards : []) {
      if (card && card.at) byAddress.set(card.at, card);
    }
    const cards = (Array.isArray(spread.cards) ? spread.cards : [])
      .filter((card) => card && card.at)
      .map((card) => {
        const known = byAddress.get(card.at) || {};
        return {
          at: String(card.at),
          x: coord(card.x),
          y: coord(card.y),
          title: String(known.title || ''),
          reason: String(known.reason || ''),
          fingerprint: String(known.authored_fingerprint || ''),
        };
      });
    return {
      id: 'ask:' + question,
      name: askName(spread.name || question),
      question,
      held: proposal.held || null,
      cards,
      sparse: !!proposal.sparse,
      truncated: !!proposal.truncated,
      fallback: proposal.fallback || null,
      matched_terms: Array.isArray(proposal.matched_terms) ? proposal.matched_terms : [],
      ambiguous_terms: Array.isArray(proposal.ambiguous_terms) ? proposal.ambiguous_terms : [],
      counts: proposal.counts && typeof proposal.counts === 'object' ? proposal.counts : {},
    };
  };

  /** Why the temporary Table holds one Card; a Card the answer explained with nothing has none. */
  const askReason = (preview, at) => {
    const card = (preview && preview.cards ? preview.cards : []).find((held) => held.at === at);
    return card ? card.reason : '';
  };

  /**
   * The Spread document Keep saves (D232, D233): a new value holding every earlier Spread as it
   * was, this question's one ordinary Spread — its id, its display name, its whole question, its
   * Cards and places — and that Spread current. The temporary Table's reasons and fingerprints stay
   * transient, so they are not written into the document.
   */
  const askKeep = (doc, preview, id) => {
    if (!preview || !Array.isArray(preview.cards)) {
      throw new Error('there is no proposal to keep; ask the box first');
    }
    const spreads = (doc && Array.isArray(doc.spreads) ? doc.spreads : []).map((spread) => ({ ...spread }));
    const spread = {
      id: String(id),
      name: askName(preview.name || preview.question || ''),
      cards: preview.cards.map((card) => ({ at: card.at, x: coord(card.x), y: coord(card.y) })),
    };
    if (preview.question) spread.question = preview.question;
    spreads.push(spread);
    return { current: spread.id, spreads };
  };

  /**
   * What the Keep check found (D234): each proposed Card's authored fingerprint compared with the
   * Card at that address now. `changed` names a Card whose authored text moved — including a new
   * occupant of an old address, which is never treated as the Card the question selected — `missing`
   * a Card that is gone, and `unchanged` the rest. A proposal that captured no fingerprint cannot be
   * trusted silently, so it counts as changed.
   */
  const askChanged = (preview, fresh) => {
    const changed = [];
    const missing = [];
    const unchanged = [];
    for (const card of (preview && preview.cards) || []) {
      const now = fresh ? fresh[card.at] : null;
      if (!now || now.error) {
        missing.push({ at: card.at, then: card.title || '' });
      } else if (!card.fingerprint || now.authored_fingerprint !== card.fingerprint) {
        changed.push({ at: card.at, then: card.title || '', now: String(now.title || '') });
      } else {
        unchanged.push(card.at);
      }
    }
    return { changed, missing, unchanged };
  };

  /** The same temporary Table without the named Cards: Keep saves only what the reader chose. */
  const askWithout = (preview, addresses) => {
    const drop = new Set(Array.isArray(addresses) ? addresses : []);
    return {
      ...preview,
      cards: ((preview && preview.cards) || []).filter((card) => !drop.has(card.at)),
    };
  };

  return { askQuestion, askName, askPreview, askReason, askKeep, askChanged, askWithout };
});
