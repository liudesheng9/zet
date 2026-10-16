## Parent

#1

## What to build

Implement the core Card domain engine for Locations, successor generation, and stored Card text parsing/validation. This slice should make it possible to validate all structural inputs independently of any one UI surface.

## Acceptance criteria

- [ ] Valid Topic Card Locations are exactly `<topic-id>/0`.
- [ ] Valid regular Card Locations are `<topic-id>/<positive-number>` followed by zero or more `|<positive-number>` or `|<side-label>` segments.
- [ ] Topic IDs reject leading zeroes except the Topic ID `0`.
- [ ] Numeric successor segments are positive decimal numbers without leading zeroes.
- [ ] Side labels use lowercase Excel-style labels: `a..z`, then `aa`, `ab`, and so on.
- [ ] Topic root numbers are monotonic and are not reused after Topic deletion.
- [ ] `zt n` successor generation follows the locked rules for Topic Cards, regular Cards ending in numbers, and regular Cards ending in side labels.
- [ ] `zt b` successor generation appends the next available side label and is invalid on Topic Cards.
- [ ] Card text parsing requires exactly three sections separated by lines containing exactly `<--->`.
- [ ] Card titles are non-empty single-line text and may contain punctuation and spaces.
- [ ] Topic descriptions reject link macros.
- [ ] Regular Card bodies accept link macros in the form `[[location]]`.
- [ ] Link macros do not support display text.
- [ ] Invalid Location strings, invalid link macros, malformed Card text, and invalid titles are reported with clear validation errors.

## Blocked by

#2
