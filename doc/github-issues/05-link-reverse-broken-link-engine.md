## Parent

#1

## What to build

Implement link parsing, Reverse link generation, Broken link handling, and `zt lsbk`. Link relationships must stay in Card text and generated Reverse link text, not as structural SQLite link rows.

## Acceptance criteria

- [ ] Regular Cards may link to regular Cards and Topic Cards using `[[location]]`.
- [ ] Topic descriptions cannot contain outbound link macros.
- [ ] Topic Cards can receive Reverse links.
- [ ] Save rejects newly introduced Broken links.
- [ ] Save allows existing Broken links to remain when the edit does not introduce new Broken links.
- [ ] Reverse-link sections are regenerated after every successful save, delete, or move.
- [ ] Reverse-link regeneration recomputes the Reverse link section for all Cards.
- [ ] Links inside generated Reverse link sections are ignored when computing Reverse links.
- [ ] Multiple links from one source Card to the same target produce one Reverse link line for that source Card.
- [ ] Reverse link lines use the locked wording: `This note has been referred by note [[<source-location>]] <source-title>`.
- [ ] Reverse links are generated text inside the Card `text` field and are not stored as structural SQLite link data.
- [ ] `zt lsbk` lists Broken link macros as a shell command while service is up.
- [ ] `zt lsbk` is available in the Session command bar.
- [ ] Each Broken link listing shows source Card Location, source Card title, broken target Location, and the matching source text line.

## Blocked by

#4
