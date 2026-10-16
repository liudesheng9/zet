# Luhmann and practitioners: the basis of the zettelkasten skill

Gathered on 2026-10-10 for the ZT `zettelkasten` skill. Each rule in `SKILL.md` traces to a finding below. Quotes are short paraphrases unless marked.

ZT's accepted practices and decisions: [comparison](https://github.com/liudesheng9/zet/blob/main/docs/slip-box-luhmann-comparison.md), [design](https://github.com/liudesheng9/zet/blob/main/docs/slip-box-luhmann-design.md), and [implementation specification](https://github.com/liudesheng9/zet/blob/main/docs/slip-box-luhmann-implementation-design.md).

Source quality: Luhmann's essay (Kuehn translation), Matuschak's notes, and the zettelkasten.de posts were read directly. Schmidt's articles could not be opened (the PDF is protected and the HTML and archive pages render by script), so the archive facts come from the Bielefeld digitization talk and from secondary reports, and the register's "one or two numbers per keyword" is Ahrens citing Schmidt (2013, p. 171). Doto's book and most practitioner reports are known through summaries and forum posts, not controlled studies.

## Luhmann's own account (1981)

Niklas Luhmann, "Kommunikation mit Zettelkästen. Ein Erfahrungsbericht" (1981), in *Öffentliche Meinung und sozialer Wandel*; English "Communicating with Slip Boxes", trans. Manfred Kuehn.

- **Fixed number, no topical order.** Each slip gets a number at the top left that never changes; position is set by the number. A subject hierarchy would bind him to one structure for decades. → ZT Locations; *Placement* is about conversation, not category.
- **Internal branching.** A new slip attaches to any existing slip, even to one word in it: 57/12 continues to 57/13, or branches to 57/12a, 57/12a1. → Direct successor (continue the line) and Side successor (comment on one Card).
- **References solve "multiple storage".** Add as many references between slips as needed; where a slip is stored matters little once it is linked from elsewhere. → One home per Card, every other context as a Link.
- **Unconnected slips are lost.** A slip gets its value from its links; an unconnected one is found again only by chance. → At least one Link besides the parent.
- **Keyword register** pointing to fixed numbers, as entry points. → Topic bodies name one to three entry Cards.
- **A separate bibliographic box** of works actually read, linked from the slips about them. → Literature Cards in their own Zone.
- **Own thoughts, few excerpts.** Record your own deliberations; excerpt only remarkable formulations. → *Own words*.
- **Communication partner and surprise.** Communication needs partners who can surprise each other. The box surprises when it is open (not thematically narrow) and densely, ad-hoc connected; query it by activating its link network. → Converse first; look for Links between distant trees.
- **Writing is not copying.** Publications are not assembled by copying slips; productive use happens when relations between relations are established. → Structure Cards; write the text fresh.

Sources: [Kuehn translation](http://luhmann.surge.sh/communicating-with-slip-boxes); [zettelkasten.de on the translations](https://zettelkasten.de/posts/luhmann-essays-online/); [side-by-side 2025 translation](https://members.wolfram.com/billw/2025-04-12-luhman.html).

## The archive (Johannes F. K. Schmidt, Bielefeld)

- Two collections: ZK I (about 1951 to 1962) with about 23,000 slips and 1,800 bibliographic entries; ZK II (about 1963 to 1996) with about 66,000 slips, 16,000 bibliographic entries, and a keyword register of about 3,200 entries. Over 90,000 slips in total.
- Register entries listed one or two slip numbers, rarely more (Schmidt 2013, p. 171, as reported by Ahrens): an entry point, not a full index. Links between slips mattered more than references from the register.
- Hubs: slips carrying long lists of references to the core slips of a subject. Structure slips: outlines resembling an article's or book's table of contents, opening a major line of thought.
- Slips were short and written as reformulations; bibliographic slips carried page references on the back.
- Luhmann said working with the box took more of his time than writing the books.

Sources: Schmidt, "Niklas Luhmann's Card Index: Thinking Tool, Communication Partner, Publication Machine", in Cevolini (ed.), *Forgetting Machines* (Brill 2016), pp. 289–311; Schmidt, "Niklas Luhmann's Card Index: The Fabrication of Serendipity", *Sociologica* 12(1) 2018, [doi:10.6092/issn.1971-8853/8350](https://sociologica.unibo.it/article/view/8350); [Bielefeld digitization talk (2018)](https://uni-bielefeld.de/einrichtungen/bits/ueber-uns/knowledge-transfer/2018/2018-04-19_Vortrag_Das_Digitalisierungsprojekt_Luhmann-Zettelkasten.pdf); [Niklas Luhmann-Archiv](https://niklas-luhmann-archiv.de); ["The Money Is in the Hubs"](https://zettelkasten.de/posts/zettelkasten-hubs/); [overview of the two boxes](https://www.ernestchiang.com/en/posts/2025/niklas-luhmann-original-zettelkasten-method/); [Ahrens notes via Andy Matuschak](https://notes.andymatuschak.org/How_to_Take_Smart_Notes_-_Ahrens).

## Practitioners

- **Sönke Ahrens, *How to Take Smart Notes* (2017).** Fleeting notes are temporary; literature notes capture a source's idea briefly in your own words with its reference; permanent notes are one idea each, understandable in isolation, compared with and linked to existing notes before filing. → Literature Card vs Topic tree Card; *Standalone*. [Summary](https://www.ybrikman.com/blog/2023/08/10/how-to-take-smart-notes/).
- **Andy Matuschak, evergreen notes.** Atomic, concept-oriented, densely linked, associative rather than hierarchical; titles as complete claims, so a title works like an API for the note. Atomicity is a trade-off: over-split notes fragment the link network. → *Claim title*; split when a Card holds two ideas, not into fragments. [Evergreen notes](https://notes.andymatuschak.org/Evergreen_notes); [titles are like APIs](https://notes.andymatuschak.org/Evergreen_note_titles_are_like_APIs); [should be atomic](https://notes.andymatuschak.org/Evergreen_notes_should_be_atomic).
- **Christian Tietze and Sascha Fast (zettelkasten.de).** The Collector's Fallacy: knowing about is not knowing; shorten the read-and-process cycle (Tietze later confessed years-old unprocessed backlogs). Atomicity is a guiding principle and the *outcome* of working notes over time, not an entry requirement. Folgezettel is one way to make a link, not the essence; placement matters little when links carry intent. → *Collector's fallacy*; *Maintenance*. [Collector's fallacy](https://zettelkasten.de/posts/collectors-fallacy/); [confession](https://zettelkasten.de/posts/collectors-fallacy-confession/); [atomicity](https://zettelkasten.de/posts/principle-of-atomicity-difference-between-principle-and-implementation/); [Folgezettel](https://zettelkasten.de/posts/luhmann-folgezettel-truth/).
- **Bob Doto, *A System for Writing* (2024).** Capture → main notes → connections → hub notes, structure notes and indexes → writing. Folgezettel forces you to connect each imported idea at the moment of filing. → The filing steps; placement next to the Card a thought talks to.
- **Forum experience.** Explain why two notes are linked, or years later the link means nothing; tags only suggest relatedness. Connection notes (a third note explaining the relation of two others) keep the originals unchanged. Long-term users report retroactive relinking of old notes and moving toward minimalism (fewer sources in parallel). → *Links* carry reasons; *Maintenance*. [Forum: link reasons](https://forum.zettelkasten.de/discussion/comment/9767/); [link context](https://forum.zettelkasten.de/discussion/comment/15390/); [practice reports](https://forum.zettelkasten.de/discussion/comment/24393/).
- **Failure reports.** Hundreds of neatly linked notes did not produce understanding when needed (Eva Keiffenheim); over-splitting produced notes that were titles with almost no body (Obsidian forum). → Process into claims; keep Cards whole enough to stand alone. [Keiffenheim](https://evakeiffenheim.substack.com/p/the-fatal-flaw-in-most-personal-knowledge); [atomicity debate](https://forum.obsidian.md/t/debating-the-usefulness-of-atomic-notes-a-novel-pragmatic-obsidian-based-approach-to-pkm-strategies/38077).

## Mapping to ZET

The rules in `SKILL.md` follow ZET's actual Location scheme (`docs/zt-note-system-design.md`, `src/main.rs`), not Luhmann's.

- **Fixed slip number → Location**, with a different grammar. Luhmann alternated digits and letters inside one token (`21/3d7a7`); ZET joins segments with `|` (`3/2|c|4`), and every letter segment is a branch, every number a step.
- **Continue a sequence → Direct successor**, one per Card. ZET can also insert a missing step with `insert_card before`. This renumbers the line and rewrites its Links. Use a Side successor for a different thought beside a step.
- **Branch at a slip (57/12a) → Side successor**, ordered `a…z, aa…`. Luhmann's numbers were permanent; deleting a ZET Side successor moves its later siblings down a letter, and ZT rewrites the Links.
- **Section number (the leading 21 in 21/3d7) → Topic `<n>/0`**, whose only successor is `<n>/1`. A Topic cannot branch at its root, so `<n>/1` is the doorway and branches start there. Topic numbers are shared by both Workspaces and never reused.
- **Fixed placement → movable placement.** `move_card` carries a Card with its subtree to another free slot; Luhmann never moved a slip.
- **Reference and handwritten back reference → `[[address]]` Link and generated Reverse links.**
- **Keyword register → no register;** `list_trees`, `search_cards` (substring), and Topic bodies with Links to one to three entry Cards.
- **Hub and structure slips → hub and structure Cards**, ordinary Regular Cards whose bodies list Links with reasons; a whole-Topic hub sits as a Side successor of `<n>/1`.
- **Bibliographic box with page references → Literature Card** (BibTeX plus a page index), rooted at its Citation key, whose successors `Key/1`… never leave its tree.
- **One owner's box → two Workspaces.** Agents grow only the agent Workspace and continue the user's lines by Link.
