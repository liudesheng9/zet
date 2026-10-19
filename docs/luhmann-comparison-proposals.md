# ZT and Luhmann's Zettelkasten: Practice, Archive Evidence, and Proposals

Status: analysis and proposals (2026-10-18), with the user's decisions of 2026-10-18 (see "Decisions"): 13 proposals kept, P6 and P10 dropped. Nothing in this document changes code, Cards, or skills. Each proposal that the user keeps needs its own design or issue before work starts.

This document is the second comparison of ZT with Niklas Luhmann's Zettelkasten. The first, [slip-box-luhmann-comparison.md](slip-box-luhmann-comparison.md) (decisions of 2026-10-16), compared the **mechanics**: addresses, branching, insertion, Links with reasons, the register, hubs. It accepted or rejected 25 practices (P1 to P25), and this document does not reopen them. This document compares **what the practice produces**: it measures the live agent Workspace and asks where ZT spends the user's time, where Links fail to relate, and where a Card cannot be read alone. The proposals aim at three goals:

1. **Efficiency**: the user's time to find, read, and build on Cards, and the cost to produce them.
2. **Relatedness**: Link quality, kinded Links, hub and keyword access, connections between books.
3. **Comprehensibility**: Cards that a reader understands alone, titles, context, and the Topic entry points.

The Link-scope fault in the digest is filed as [liudesheng9/zet#92](https://github.com/liudesheng9/zet/issues/92). This document leaves it out.

## Decisions

The user chose on 2026-10-18 which proposals to keep. **Kept** means that the proposal goes on to a design or an issue. **Dropped** means that ZT does not follow it. A Dropped proposal stays in this document as a record of the analysis.

| No. | Proposal | Group | Decision (2026-10-18) |
| --- | --- | --- | --- |
| P1 | The Topic test: only claims that hold without the book go into a Topic tree | A. Filing and production | Kept |
| P2 | One statement, one home | A. Filing and production | Kept |
| P3 | File new Cards at the lines of earlier books | A. Filing and production | Kept |
| P4 | Link while writing; the link pass fills only the gaps | A. Filing and production | Kept |
| P5 | One Topic per concept, across books | B. Relatedness | Kept |
| P6 | Fewer, stronger Links | B. Relatedness | Dropped |
| P7 | Hubs where books meet | B. Relatedness | Kept |
| P8 | A concept register | B. Relatedness | Kept |
| P9 | A standalone check: no book-local numbers, no "there" | C. Comprehensibility | Kept |
| P10 | A short Literature Card and the book's footprint | C. Comprehensibility | Dropped |
| P11 | Titles that tell Cards apart | C. Comprehensibility | Kept |
| P12 | Ask the box: a question becomes a Spread | D. Working with the box | Kept |
| P13 | Entry-first search | D. Working with the box | Kept |
| P14 | Reply slips: the user's Card beside an agent Card | D. Working with the box | Kept |
| P15 | One distant Card within reach | D. Working with the box | Kept |

## Sources

### Luhmann's practice

| Key | Source | Use |
| --- | --- | --- |
| L81 | Luhmann, N. (1981). "Kommunikation mit Zettelkästen. Ein Erfahrungsbericht." In H. Baier, H. M. Kepplinger, K. Reumann (eds.), *Öffentliche Meinung und sozialer Wandel*, pp. 222–228. Westdeutscher Verlag. doi:10.1007/978-3-322-87749-9_19. Read in M. Kuehn's English translation, "Communicating with Slip Boxes" (sections I to IV); German page numbers as Schmidt cites them. | His own account. |
| ZK9/8 | Luhmann, ZK II slips 9/8 to 9/8j ("Zettelkasten"), probably written in 1980/81 for L81 (editorial note of the archive). Read through the archive's portal and API. | His working notes on the box. |
| L87 | Luhmann, N. (1987). "Biographie, Attitüden, Zettelkasten." In *Archimedes und wir*, pp. 125–155. Merve. Read only as S15 and S18 quote it. | Reading toward the box; the time it cost. |
| NLA-ZK | Niklas Luhmann-Archiv, Universität Bielefeld: "Der Zettelkasten Niklas Luhmanns" (text by J. F. K. Schmidt), portal version 25 (2026-05-23), §§1–4. niklas-luhmann-archiv.de/nachlass/zettelkasten | The newest counts. |
| NLA-Tut | Niklas Luhmann-Archiv: "Tutorial zum digitalen Zettelkasten", portal version 25. | Strands and the subject overview. |
| NLA-SW | ZK II register slips ZK_2_SW2d_001 and ZK_2_SW2d_050, read through the archive API; entries counted for this document. | Addresses per keyword. |
| S13 | Schmidt, J. F. K. (2013). "Der Nachlass Niklas Luhmanns – eine erste Sichtung: Zettelkasten und Manuskripte." *Soziale Systeme* 19(1), 167–183, §§2.1–2.5. | Size, links, register, reading notes. |
| S15 | Schmidt, J. F. K. (2015). "Der Zettelkasten Niklas Luhmanns als Überraschungsgenerator." In the exhibition volume *Serendipity*, pp. 152–167. | Links, register, surprise. |
| S18 | Schmidt, J. F. K. (2018). "Niklas Luhmann's Card Index: The Fabrication of Serendipity." *Sociologica* 12(1), 53–60. doi:10.6092/issn.1971-8853/8350. A shortened version of Schmidt (2016), "Niklas Luhmann's Card Index: Thinking Tool, Communication Partner, Publication Machine", in A. Cevolini (ed.), *Forgetting Machines*, Brill, pp. 289–311, which this research could not open. | Sections, link types, hubs, register, revision. |
| HCD | [hub-cards-design.md](hub-cards-design.md), section "What Luhmann did": counts made from the archive's TEI data on 2026-10-14. | Collective-reference and register counts. |

Each claim below names its source key and section. Paraphrases only. Where the sources give different numbers, the text gives the range. The full research notes, with the URLs, are kept outside the repository with this session group's notes.

### ZT

- The design: `CONTEXT.md`, `README.md`, `docs/wiki/` (block docs), `skills/zettelkasten/SKILL.md` and its `references/luhmann-and-practice.md`, `skills/zt/SKILL.md`, `docs/zt-digest-framework.md`, and the digest prompts `scripts/book-digest/dsh-digest-base.zh.md`, `dsh-link-instructions.zh.md`, `dsh-hub-instructions.zh.md`, `dsh-bridge-instructions.zh.md`, `dsh-card-instructions.zh.md`, `dsh-orphan-instructions.zh.md`.
- The live archive, read only through the zt MCP tools on 2026-10-18 (archive revision 0): `list_workspaces`, `list_trees`, `export_titles`, `list_hubs`, `get_hub`, `get_tree` (hart1995firms with bodies, stiglitz2014learning without bodies, and the Topic trees 113, 161, 254), `get_cards` (the six Literature Cards, and a seeded random sample of 200 Topic Idea Cards), `search_cards`, `list_links`. mascolell1995microeconomic was read by Card and by search only: its tree exceeds the 8 MiB reply limit.

## What Luhmann did

### Numbering and branching

- **A fixed place, no subject order.** Each slip gets a number at the left of its first line and a fixed place that never changes. An order by topics and sub-topics would bind the writer for decades, so the fixed place needs no system (L81 §II, p. 223f.).
- **Growth inwards.** 57/12 continues as 57/13. A word or thought on 57/12 continues on 57/12a, 57/12b, and these branch again to 57/12a1. One slip can have several connection points, marked in red. Between two slips first written one after the other, several hundred later slips can stand (L81 §II.1, p. 224; S13 §2.2; NLA-ZK §3.4).
- **Local fit only.** A new slip goes beside the slip it fits locally. So its place says nothing about its weight: "autopoiesis", a central concept, sits at 21/3d26g1i (NLA-ZK §3.3; S18 §3; S15 §2).
- **Sections.** ZK I (about 1951 to 1962, about 22,000 to 24,000 slips) has 108 small sections such as state, planning, power, role. ZK II (1963 to 1997, about 67,000 slips) has 11 broad, problem-oriented top-level sections with about 100 subsections and deep internal branching. Luhmann made no table of contents for ZK II (NLA-ZK §§3.1–3.3; S13 §2.2; S18 §3 and fn. 15).

### Links (Verweise)

- **Why he linked.** Because numbers are fixed, a slip can take any number of references. References solve the problem of multiple storage: when a slip has several candidate places, pick one and record the others as links. He wrote the back-link on the target at once (L81 §II.2, p. 224).
- **How many.** About 18,000 to 20,000 links in ZK I and 25,000 to 30,000 in ZK II: nearly one link per slip in ZK I and one per two slips in ZK II. These are estimates from samples (S13 §2.3; NLA-ZK §3.5; S18 §5, p. 57).
- **Kinds.** Near links to an inserted side thought; outline links (capital letters) at the head of a line; distant single links, often from one word, to an entirely different context; and collective references (NLA-ZK §3.5; S18 §5).
- **Links cut across topics.** Links must not only gather material under collective concepts. They must pull material out from under them (ZK9/8b1). A distant link is a short cut across the order (S15 §3).
- **Upkeep.** He wrote links when he made a slip, and he kept adding links to old slips when new slips called for them. The box cost him more time than writing books (S18 §5, p. 58; L87 as quoted in S15 §1).

### The keyword register (Schlagwortregister)

- **Why.** Without a systematic order a writer cannot remember the numbers, so a register points to fixed numbers (L81 §II.3, p. 224f.).
- **Size.** About 1,250 entries for ZK I. For ZK II, four versions, the last on 244 cards with about 3,200 entries (NLA-ZK §3.2c; S13 §2.1c; S18 §6). HCD counts 3,016 terms in version 4.
- **Few places per term.** One to three, at most about four places per term (S13 §2.4; S18 §6, p. 58; NLA-ZK §3.2c). HCD counts 1.30 places per term, and 78.8% of the terms name one card. NLA-SW counts a mean of about 1.5 on two register slips.
- **Entry points, not a concordance.** The register names where to enter. The link network reaches the rest. It aims to be complete in its terms, not in its places (ZK9/8b; S13 §2.4; S18 §6, pp. 58–59). Schmidt's spot check found that some important slips are reached only by detours or not at all (S13 fn. 17).

### Collective references and outline slips

- **Sammelverweise.** At the head of a topic block a slip lists up to 25 references, usually with the keyword beside each number, often to distant places. Handwriting shows that these slips grew bit by bit (NLA-ZK §3.5b; S13 §2.3b; S18 §5b). Schmidt calls them hubs: nodes with far more links than average (S18 §5, p. 58).
- **Outline slips.** Capital-letter references at the head of a long line, close to a table of contents (S18 §5a).
- **No hub of hubs.** Above the hubs there were only the section numbers and the register (HCD).

### The bibliographic box and literature notes

- **A separate box.** A work actually read gets a bibliographic slip in a separate box. The numbers of the slips that came from the work can go on it, so the bibliography becomes a second way in (L81 §II.3, p. 225). He did not do this systematically (NLA-ZK §3.2b).
- **Minimal reading notes.** In ZK II about 11,000 single-title slips and about 7,000 notes with bibliographic data on their back (NLA-ZK §3.2b; older sources say 15,000 to 16,000). About 35% of the bibliographic slips carry a short note in keywords with page numbers on the back; often a whole book fits on one slip (S13 §2.1b; NLA-ZK §3.2b).
- **Reading toward the box.** While reading he asked what could be used for slips already written (L87, quoted in S18 §2). Slip 9/8d says to read with an eye on the box, "nicht: Exzerpte!" (ZK9/8d). In the evening he turned the day's notes into new slips, not copied as written (S18 §2, p. 55).

### The slip and its text

- **Format.** DIN A6 (he says "Oktavformat"), thin paper, front side only (L81 §II; NLA-ZK §3.1).
- **Own theses.** The slips record his own theses, concepts, and questions as well as results of reading. Over time they became more compact and thesis-like (S15 §1; S18 §2; NLA-ZK §3.2a). Excerpts are only for remarkable wording (L81 §III).
- **"One idea per slip" is a later reading.** Luhmann never states it. A branch can start from one word, one slip has several connection points, and a thought can run over several slips (L81 §II.1; NLA-Tut §2.3).

### The box as partner, second memory, surprise

- **A partner.** Communication needs partners that can surprise each other, so the box needs an open organization that no topic binds (L81 §I, p. 222f.). It is a second memory, an alter ego with no overall order, so it gets a life of its own (L81 §II, p. 225; ZK9/8).
- **Critical mass.** The box takes years to reach the inner complexity it needs; until then it is a container (L81 §III). Schmidt dates critical mass to the early or mid 1970s (S13 §2.5).
- **Surprise.** Activating the link network makes more available than the writer had in mind, because the box's internal horizons of comparison differ from the search pattern (L81 §III; ZK9/8e). Entering at a keyword with a research question opens a large web; without a question the reader gets lost (S15 §4; S18 §6).
- **Writing is not copying.** The box is fruitful only as relations between relations, at the moment of use (L81 §III).

### Revisiting and extending

- **Extend by insertion, judge later.** A note need not be finished before filing; later notes and use decide its value (S18 §2; ZK9/8a2, "Klärgrube").
- **No rewriting.** Errors and dead ends were answered by later slips. The old slips stayed (S18 §7, p. 59; NLA-ZK §4).
- **Upkeep.** Later work on old slips was added links and back-links, growing collective references, and four rebuilt registers. Some small regions were never touched again: Schmidt's "black holes" (S18 §§5–6).
- **Rate.** About 90,000 slips in about 45 years: roughly 2,000 a year, five or six a day. The slips are undated, so this is an average from the totals (estimate from NLA-ZK and S13 counts; S13 fn. 14).

## What the archive shows

All numbers come from the reads listed under "Sources". "Sample" means the seeded random sample of 200 Topic Idea Cards (no roots, no hubs).

| Measure | Value | Evidence |
| --- | --- | --- |
| Agent Workspace size | 10,887 Cards: 3,546 in Topic trees (208 Topics), 7,341 (67%) in 6 Literature trees | `list_workspaces`, `list_trees` |
| Human Workspace size | 8 Cards in 3 Topics and 1 Literature tree | `list_trees workspace=human` |
| Links between the Workspaces | 8, all from agent Cards to human Cards (for example `52/2|b` → `0/1|a`). No human Card links an agent Card. | `list_links cross_workspace_only` |
| Cards per book | hart1995firms 374, stiglitz2014learning 1,565, mascolell1995microeconomic 5,092 in the Literature trees | `list_trees` |
| Model share of a Literature tree | Hart: 250 of 373 Regular Cards are Model Cards or parts (41 Derivation parts). Stiglitz: 1,223 of 1,564 (330 Derivation parts). | `get_tree` |
| Topic trees by book | 207 Topics cite a book. 193 cite one book only. 14 cite two, and in 7 of them the second book gives one or two Cards. Stiglitz fills 79 Topics, Mas-Colell 118, Hart 11. | Reverse links of each Literature Card |
| One concept, two Topics | 161 均衡的多重性 (Mas-Colell) and 254 多重均衡 (Stiglitz); 166 次优福利问题 and 263 扭曲与次优; 125 非凸性 and 252 学习的非凸性; 120 策略性进入阻止 and 222 进入阻遏; 179 学习 and 214 学习型经济. No Card of 161 links a Card of 254, and the topic hub `161/1|b` lists only Mas-Colell Cards in its `邻近` group. | `get_tree 161`, `get_tree 254`, `get_hub 161/1|b` |
| Topic gate | Refuses a new Topic only when its title has a character-bigram Dice similarity of 0.8 or more with an agent Topic. 均衡的多重性 and 多重均衡 score 0.5. | `docs/wiki/zt-digest.md` (`-TopicGate`) |
| Topic Cards that rest on the book | 150 of the sample (75%) rest on a Card of a Literature tree; 193 (97%) have a kinded Link into a Literature tree. Every Card of Topic 254 rests on `stiglitz2014learning/109|a`. | Sample, `get_tree 254` |
| Book-local numbers in Topic Cards | 110 of the sample (55%) name an equation, section, or proposition of the book (式 (11), 命题 3.G.3, 11.D.1, 第 6 章, 本节). 71 of 3,338 Topic titles do, for example `113/1|d|b` "11.D.1 的市场方案要求产权明确且主体众多" and `45/1|a` "短期债务在模型 1 触发清算，在模型 3 只是阻止扩张". `113/1|e|a|a|a|a` reads in full "式 (12) 含加成项。式 (11) 不含加成项。" | Sample, `export_titles`, `get_tree 113` |
| Deixis | 144 of the sample (72%) point with 那里, 那张卡, or 该卡 ("there", "that Card") instead of naming the referent. In Hart, 129 of 373 bodies do. | Sample, `get_tree hart1995firms` |
| Appended Link paragraphs | 122 of the sample (61%) end with a `关联：` paragraph that the link pass appended. The anchor `mascolell1995microeconomic/19` holds two `在全书中的位置` paragraphs. | Sample, `get_cards` |
| Links per Card | Idea Cards in Literature trees: median 5 (Hart) and 8 (Stiglitz) Links out, 90th percentile 13. Model Cards: median 19 (Hart) and 10 (Stiglitz). `mascolell1995microeconomic/19|a` has 29. | `get_tree` |
| Symmetric Link pairs | A links B and B links A: 169 pairs in Hart, 281 in Stiglitz, for example `80/1` ↔ `88/2|a`. | `get_tree` |
| Links between books | From the Hart Literature tree, 1 Link of about 2,090 goes to another book's Literature tree; from Stiglitz, 19 of about 6,374. In the sample, 6 Cards (3%) cite the Literature trees of more than one book. | `get_tree`, sample |
| One statement, many Cards | The Slutsky equation is stated in `88/2|a`, `80/1`, `81/1|d`, the anchor `mascolell1995microeconomic/19`, the Model Card `mascolell1995microeconomic/19|a`, and its parts `…/19|a|a|3` and `…/19|a|a|4`. `81/1|d` says itself that it "对应" the model part with the same definition. | `search_cards 斯卢茨基`, `get_cards` |
| Register | `196/1`: 29 field hubs and 78 keywords. Each keyword names a topic hub or a Topic root (`79/0`, `98/0`, `118/0`, `142/0`, `250/0`, `254/0`, `268/0`); the keywords repeat the Topic titles. No keyword names a Card in a Literature tree. `get_hub` reports 7,488 Cards that no hub reaches. | `get_card 196/1`, `list_hubs` |
| Hubs | 270: 161 topic, 79 line, 29 field, 1 register; no faults. The topic hub `161/1|b` lists all 7 Cards of its tree. 33 hubs have exactly the title of the Card they are filed under (for example `66/1|a` and its hub `66/1|a|g`). | `list_hubs`, `get_hub`, `export_titles` |
| Literature Card | The body of `stiglitz2014learning` has 13,534 characters (the group index); Mas-Colell 2,613; Hart 1,105. | `get_cards` |
| Card size | Topic Idea Cards in the sample: 195, 261, and 365 characters of prose at the 10th, 50th, and 90th percentiles (formulas and Links removed). | Sample |

## Comparison

Each row names a Luhmann practice, what ZT does, and whether the difference helps or hurts. "Decided" marks a difference that the user settled on 2026-10-15 or 2026-10-16; this document does not reopen it. A reference such as "2026-10-16: P1" names a practice of [slip-box-luhmann-comparison.md](slip-box-luhmann-comparison.md); an arrow (→ P3) names a proposal of this document.

| No. | Luhmann | ZT today | Evidence | Effect |
| --- | --- | --- | --- | --- |
| C1 | A fixed number that never changes (L81 §II). | Locations move; ZT rewrites every Link. | `move_card`, `insert_card`; 0 Broken links | Decided (2026-10-16: P1, P5). Good inside ZT. |
| C2 | Growth inwards: continue (57/13) or branch (57/12a) at any slip (L81 §II.1). | Direct and Side successors, `insert_card`. | — | Same (2026-10-16: P3, P4). |
| C3 | No subject order. ZK II moved from 108 small fields to 11 broad blocks with deep branching (NLA-ZK §3.3; S18 §3). | Each digest opens Topics named by a concept, mostly for its own book. 208 Topics; 193 of the 207 that cite a book cite only one. | "Topic trees by book" | **Hurts relatedness.** In practice ZT is close to ZK I's small fields, cut again per book. → P3, P5 |
| C4 | Local fit: a new slip goes beside the slip it continues, from any earlier reading (S18 §3). | The digest files under "the most fitting Card of the snapshot", but mostly beside its own book's Cards. Topic 254 is the Stiglitz appendix in order (`254/1` to `254/6`). A good case exists: `113/1|e|a` (Stiglitz) branches from `113/1|e` (Mas-Colell). | `get_tree 254`, `get_tree 113` | **Mixed.** The rule is right; the inputs make it book-local. → P3 |
| C5 | Multiple storage is solved by links: one slip, other places as links (L81 §II.2; ZK9/8b2). | One home per Card in principle (2026-10-16: P13). In practice one statement is written again in several Cards. | Slutsky equation in 7 Cards | **Hurts efficiency.** → P2 |
| C6 | About one link per slip (ZK I) or one per two slips (ZK II) (S18 §5). | Idea Cards median 5 to 8 Links out, 90th percentile 13; Model Cards 10 to 19, up to 29. | "Links per Card" | **Mixed.** More paths for agents, weaker signal per Link for a reader. → P6 (dropped) |
| C7 | Links are written in the text at the point they serve, often from one word; distant links cut across topics (NLA-ZK §3.5; ZK9/8b1). | A kind word before each Link (stronger than Luhmann; 2026-10-16: P14). But 61% of Topic Cards end with an appended `关联：` paragraph, and Links stay inside the book (Hart: 1 of about 2,090 to another book's Literature tree). | Sample; `get_tree` | **Hurts relatedness.** → P3, P4 |
| C8 | Back-links written at once by hand (L81 §II.2). | Reverse links are generated (2026-10-16: P15). Authors still write the back-link: 169 symmetric pairs in Hart, 281 in Stiglitz. | `get_tree` | **Redundant.** The written back-link adds nothing that the Reverse links do not show. → P6 (dropped) |
| C9 | A register of about 3,200 terms, one to four places each, complete in its terms (S18 §6; NLA-ZK §3.2c). | `196/1`: 78 keywords that repeat Topic titles and point to hubs or Topic roots; no entry into the Literature trees, which hold 67% of the Cards. | `get_card 196/1` | **Hurts access.** The form is decided (2026-10-16: P18); the coverage is not. → P8 |
| C10 | Collective references of up to 25 references, mostly into other sections, grown over time (NLA-ZK §3.5b; HCD). | 161 topic hubs, one per tree by policy, at most 25 lines and one to five neighbours. `161/1|b` lists all 7 Cards of its tree, and its neighbours come from the same book. | `get_hub 161/1|b` | **Partly.** The form follows Luhmann; the policy makes many tables of contents. → P7 |
| C11 | Outline slips at the head of a line (S18 §5a). | Line hubs and structure Cards (2026-10-16: P20). 33 hubs carry the exact title of the Card they are filed under. | `export_titles` | Same, with a naming fault. → P11 |
| C12 | No hub of hubs (HCD). | Field hubs and the register list hubs. | `list_hubs` | Decided (2026-10-16: P21). |
| C13 | A separate bibliographic box; minimal reading notes, often a whole book on one slip (L81 §II.3; S13 §2.1b; NLA-ZK §3.2b). | A Literature Card and a Literature tree per book (2026-10-16: P11, P12). The Literature trees hold 7,341 of 10,887 agent Cards; the Stiglitz Literature Card body has 13,534 characters. | `list_trees`, `get_cards` | **Inverted proportion.** Whole-book coverage is decided (2026-10-16: P25); the size of the entry Card is not. → P10 (dropped) |
| C14 | Slips are written in a second step, toward existing slips, as the writer's own theses; "nicht: Exzerpte!" (ZK9/8d; S18 §2). | 75% of sampled Topic Cards rest on a Card of their book's Literature tree; 55% use the book's own numbers; 72% point with "there". | Sample | **Hurts comprehensibility.** Many Topic Cards are book notes filed in Topic trees. → P1, P9 |
| C15 | Own words, own style (L81 §III; S15 §1). | Agent paraphrase in the Card language. | — | Decided (2026-10-16: P9, P10). |
| C16 | Slip size: DIN A6, front only (NLA-ZK §3.1). | Topic Idea Cards: median 261 characters of prose. | Sample | Same. |
| C17 | The box is a partner through the writer's own notes, and it surprises (L81 §§I–III). | The agent Workspace has 10,887 Cards; the human Workspace has 8, and no human Card links an agent Card. Cross-book Links are rare (3% of the sample). The Bridge stage is report-only (G2). | `list_trees`, `list_links` | **Hurts.** A partner for the agents, not yet for the user. → P12, P14, P15 |
| C18 | Query: enter at a keyword with a question, then follow the links (S15 §4; S18 §6). | `search_cards` is an unranked substring search; "斯卢茨基" returns more than 40 hits (hubs, restatements, roots, a register line). The GUI search tray groups hits by tree. | `search_cards` | **Hurts efficiency.** → P8, P12, P13 |
| C19 | Never rewrite; answer an old slip with a new one (S18 §7). | Agents rewrite Cards in place (`edit_card`, the card fix cycle); the Bridge apply appends a paragraph to an earlier Card after the user's go-ahead. | `docs/zt-digest-framework.md` | **Neutral for agent Cards** (a checked copy of a book may be corrected). For the user's own thought the Luhmann way fits. → P14 |
| C20 | Upkeep: add links to old slips as new slips arrive (S18 §5). | Finished digests stay as built; earlier books get new Links only through an approved Bridge apply. | `docs/zt-digest-framework.md` | **Trade-off.** ZT protects finished books, so new books must reach old ones by placement and hubs. → P3, P5, P7 |
| C21 | Black holes: unlinked regions are lost (S18 §5). | Orphan, unreached, and broken-Link reports (2026-10-16: P17). Hubs do not reach the Literature trees (7,488 unreached). | `list_hubs` | ZT detects better; the register gap remains. → P8 |
| C22 | About 2,000 slips a year by one writer who also reads them (estimate). | Six books gave 10,887 agent Cards; one user reads them. | `list_workspaces` | **The core efficiency problem.** The box outgrows any reading budget, so entry, selection, and attention carry the load. → P2, P8, P12, P13 |
| C23 | Publications from relations between slips, drafted apart (L81 §III; NLA-ZK §3.2d). | The Table, Spreads, structure Cards (2026-10-16: P23). | — | Same. A Spread is still laid by hand. → P12 |

## Proposals

Fifteen proposals in four groups; the user kept thirteen and dropped P6 and P10 (see "Decisions"). Each gives the Luhmann practice with its source, the ZT gap with evidence, the change, the expected benefit, the cost and risk, and how to measure it.

Rules every proposal keeps:

- A change to existing Cards is a report first. Cards change only after the user's go-ahead. Finished models stay as built.
- A digest change goes into a new frozen Workflow version through the workflow pre-flight agent, and it runs first as a pilot on one book.
- The human Workspace stays the user's: no proposal lets an agent write a human Card.

### A. Filing and production

#### P1. The Topic test: only claims that hold without the book go into a Topic tree

**Decision (2026-10-18): Kept.**

- **Luhmann.** Box slips were written in a second step, toward the slips already in the box, as the writer's theses. Reading notes stayed minimal and apart, on the bibliographic slip (ZK9/8d; S18 §2; S13 §2.1b).
- **Gap.** 150 of the 200 sampled Topic Idea Cards (75%) rest on a Card of their book's Literature tree. All six Cards of Topic 254 rest on `stiglitz2014learning/109|a` and retell the book's appendix in order; `254/3` starts "把 (A.3) 与 (A.4) 画在同一平面上". The digest prompt sends most Cards to Topic trees ("多数卡片放这里"), so the book's derivation steps land there too.
- **Change.** Digest prompt (`dsh-digest-base.zh.md`, section 放置): a Card goes into a Topic tree only when its claim holds without the book's model, numbering, or neighbours, and it rests on no Card of its own book's Literature tree (an `例示`, `对应`, or `对照` Link to a Model Card stays allowed). Any other Card goes under its anchor in the Literature tree, beside the model it explains. zt-check: a new advice rule for a Topic Card with a rests-on Link into its own source's Literature tree. Organize: a report of existing Topic Cards that fail the test, with a proposed home.
- **Benefit.** Topic trees hold claims a reader can carry between books. Book-bound steps sit next to their model, where the model board shows them.
- **Cost and risk.** A prompt section, one zt-check rule, one report. Moves of existing Cards need the go-ahead. Risk: thinner Topic trees, and some true claims need a model premise; the allowed kinds keep those.
- **Measure.** Share of Topic Idea Cards with a rests-on Link into their own book's Literature tree: 75% in the sample now. Target for a new digest: under 30%.

#### P2. One statement, one home

**Decision (2026-10-18): Kept.**

- **Luhmann.** Multiple storage is solved by links, not by copies: file the slip once and record its other places as links (L81 §II.2; ZK9/8b2).
- **Gap.** The Slutsky equation is stated in seven Cards: `88/2|a`, `80/1`, `81/1|d`, `mascolell1995microeconomic/19`, `/19|a`, `/19|a|a|3`, `/19|a|a|4`. `81/1|d` itself says that it corresponds to the model part with the same definition. A reader meets the claim several times and cannot tell which Card carries it.
- **Change.** A restatement check in the zt-digest audit: compare each new Topic Idea Card with the Model parts and anchors of its own book and with the Topic Cards of earlier books, by title, body, and shared formula. Send each pair to the card cycle, which already has the verdict `merge`: keep the Card with the better home and turn the other into a sentence with a Link, or into a Card that says what the statement means outside the model. Prompt: a Topic Card does not restate a model part; it states what the part means elsewhere and links it.
- **Benefit.** Less to read, one Card to cite, fewer Cards per book.
- **Cost and risk.** A similarity step and a report. Risk: a merge of two Cards that differ in scope (a statement and its reading). Report only until the threshold is tuned on one book.
- **Measure.** Restatement pairs per 100 Cards in the audit; Cards per section group.

#### P3. File new Cards at the lines of earlier books

**Decision (2026-10-18): Kept.**

- **Luhmann.** A new slip goes beside the slip it continues, whatever reading it came from; the box grows inwards; he read with the slips already written in mind (L81 §II.1; S18 §§2–3; L87 as quoted in S18 §2).
- **Gap.** 193 of 207 Topics cite one book only. In 7 of the 14 shared Topics the second book gives one or two Cards. The digest writer gets a snapshot of titles, and files beside its own book. `113/1|e|a` (a Stiglitz Card under the Mas-Colell Card `113/1|e`) shows the wanted result.
- **Change.** Prepare gives each digest group the 10 to 20 most similar Cards of earlier books, with excerpts (the retrieval that the link pass already uses, run before the digest). Prompt: when a Card continues, qualifies, or contradicts a Card of an earlier book, file it as that Card's Side successor with the kind word in its first sentence; open a Topic only when no Card of any book fits. The audit reports the share of new Topic Cards filed in trees that another book opened. A successor changes no earlier Card.
- **Benefit.** Books meet at filing time. Topic trees become places where several books talk.
- **Cost and risk.** Retrieval exists (`relatedness.ps1`); a prompt change and a new Workflow version. Risk: wrong placements and deep side chains in old trees; the Topic test (P1) and the card cycle check them.
- **Measure.** Share of new Topic Cards filed in another book's tree (near 0 now); Topics that cite two or more books (14 of 207 now).

#### P4. Link while writing; the link pass fills only the gaps

**Decision (2026-10-18): Kept.**

- **Luhmann.** He wrote links when he made a slip, in the text where they served; later upkeep added links where new slips called for them (L81 §II.2; NLA-ZK §3.5; S18 §5).
- **Gap.** 122 of the 200 sampled Topic Cards (61%) end with an appended `关联：` paragraph from the link pass, which asks for "1–3 条跨部分链接" per Card. In Hart, 274 Links sit in such paragraphs. The anchor `mascolell1995microeconomic/19` holds two `在全书中的位置` paragraphs. A tail of Links reads as a list, not as an argument, and the link pass costs one full DeepSeek stage per book.
- **Change.** The digest writer gets the cross-part candidates before it writes and puts each Link into the sentence it serves. The link pass then runs only for Cards that end the digest with no kinded Link to another part, and for orphans; it edits the sentence that needs the Link instead of appending a paragraph. An anchor keeps one `在全书中的位置` paragraph: the link pass replaces it.
- **Benefit.** Links read as reasons. Most Cards skip one DeepSeek stage.
- **Cost and risk.** Prompt and workflow changes; a longer writer prompt. Risk: writers ignore the candidates; the link check (L1, L2) still gates.
- **Measure.** Share of Cards with an appended Link paragraph (61% now); DeepSeek tokens per Card; L1 and L2 faults per 100 Cards.

### B. Relatedness

#### P5. One Topic per concept, across books

**Decision (2026-10-18): Kept.**

- **Luhmann.** ZK II replaced ZK I's 108 small fields with 11 broad blocks and deep internal branching; links must pull material out from under collective concepts (NLA-ZK §3.3; S18 §3; ZK9/8b1).
- **Gap.** Five concepts have two Topics, one per book: 161 均衡的多重性 and 254 多重均衡; 166 次优福利问题 and 263 扭曲与次优; 125 非凸性 and 252 学习的非凸性; 120 策略性进入阻止 and 222 进入阻遏; 179 学习 and 214 学习型经济. No Card of 161 links a Card of 254. The Topic gate compares titles by character bigrams (Dice 0.8 or more); 均衡的多重性 and 多重均衡 score 0.5.
- **Change.** (a) A concept gate in `zt-digest call`: before `create_topic`, compare the new Topic's title and question with each agent Topic's title, its body question, the register terms, and the words not to use in the book term tables; a DeepSeek judge decides "same concept" for the near cases; a refusal names the Topic to file under. (b) A merge report for the existing pairs: move the smaller tree under the best Card of the larger (`move_card`), or keep both and join them by a hub (P7). Report only.
- **Benefit.** One place per concept. The register, hubs, and search point to one tree, and books meet there.
- **Cost and risk.** A change to the zt-digest gate and a judge job; a move plan. Risk: two meanings under one word (179 学习 may not be 214's learning economy); the judge keeps them apart when the meanings differ.
- **Measure.** Concept pairs with two Topics (5 found by hand now; a full scan is part of the report); Topics cited by two or more books.

#### P6. Fewer, stronger Links

**Decision (2026-10-18): Dropped.** The analysis stays as a record.

- **Luhmann.** About one link per slip in ZK I and one per two slips in ZK II; distant links were few and chosen (S18 §5; NLA-ZK §3.5).
- **Gap.** Idea Cards in Literature trees have a median of 5 (Hart) and 8 (Stiglitz) Links out, 13 at the 90th percentile; Model Cards up to 29 (`mascolell1995microeconomic/19|a`). Authors write back-links that the Reverse links already show: 169 symmetric pairs in Hart, 281 in Stiglitz (`80/1` ↔ `88/2|a`).
- **Change.** Skill and prompts: an Idea Card names at most about four kinded Links outside its own line, the ones its argument uses. A Link that already exists in the other direction with the same kind is not written again. zt-check: L3 (repeat) also reports symmetric pairs, as advice. The link pass looks for the one best Link per Card. The Model Card's 结构 list is exempt, because its Links define membership.
- **Benefit.** Each Link carries more meaning. Shorter chip rows in the reader; clearer threads on the Table and in the Constellation.
- **Cost and risk.** Prompt and rule changes. Risk: losing useful paths; apply it to new digests only, as advice, not as a fault.
- **Measure.** Links out per Idea Card; symmetric pairs per 1,000 Links; share of `refers` Links.

#### P7. Hubs where books meet

**Decision (2026-10-18): Kept.**

- **Luhmann.** A collective reference lists up to 25 references, mostly to distant places, and grows over time; such hubs open large parts of the box from one point (NLA-ZK §3.5b; S18 §5; HCD).
- **Gap.** 161 topic hubs, one for each tree of 7 or more Cards. `161/1|b` lists all 7 Cards of its tree; its three neighbours (175, 173, 169) come from the same book, and Topic 254 on the same concept is absent.
- **Change.** Hub compiler and `hub.rs` advice: (a) a concept hub for each concept that two or more books treat (from P5's scan): it lists the Cards of each book with the kind of relation between them; (b) a topic hub's `邻近` group must name a Card of another book when one exists (new advice `one-book`); (c) a tree that a topic hub would only copy (`mirror`) gets no topic hub; its Topic body names the entry Cards.
- **Benefit.** Hubs become crossings between books, not tables of contents.
- **Cost and risk.** Policy and compiler changes, a hub check. Risk: where a concept hub belongs (it spans trees); the field level or a topic hub with a wide `邻近` group can hold it, and the design decides.
- **Measure.** Share of hub lines that point into another book; hubs with the `mirror` advice.

#### P8. A concept register

**Decision (2026-10-18): Kept.**

- **Luhmann.** About 3,200 terms, each with one to four places, about 1.3 to 1.5 on average: the places where a term's treatment starts. Complete in its terms, kept current, rebuilt four times (S18 §6; NLA-ZK §3.2c; S13 §2.4; HCD; NLA-SW).
- **Gap.** `196/1` has 78 keywords. They repeat Topic titles and point to hubs or Topic roots (`79/0`, `98/0`, `118/0`). No keyword names a Card of a Literature tree, which holds 67% of the Cards; 7,488 Cards are unreached. Yet each book already has a term table (`术语表…`) that lists its concepts and the words not to use.
- **Change.** The hub compiler builds the keyword part of the register from the book term tables and the Model Card names: each term with one to three Card addresses (the Card that defines it, the Model Card, the strongest Topic Card), across books. Hundreds of terms, not tens. When the list outgrows one Card, it splits into register parts by initial letter (pinyin), each a Side successor of `196/1`; Luhmann's register filled 244 cards. Search (P13) and Ask the box (P12) start from it.
- **Benefit.** A reader enters at a concept, not at a book or a Topic name. The Literature trees get ways in.
- **Cost and risk.** A compiler change and a register layout. Risk: a long register; it stays an entry list (one to three places), and the compiler rebuilds it, so it cannot go stale.
- **Measure.** Number of terms; share of Cards within two Links of a register entry; time to the first useful Card in a task test.

### C. Comprehensibility

#### P9. A standalone check: no book-local numbers, no "there"

**Decision (2026-10-18): Kept.**

- **Luhmann.** Box slips are theses for the box, read away from the source; the box is fruitful only at a high level of generalization (S15 §1; NLA-ZK §3.2a; L81 §III).
- **Gap.** 110 of 200 sampled Topic Cards (55%) name the book's equation, section, or proposition numbers; 144 (72%) point with 那里, 那张卡, or 该卡. 71 Topic titles carry book numbers (`45/1|a` "短期债务在模型 1 触发清算，在模型 3 只是阻止扩张"). `113/1|e|a|a|a|a` reads in full "式 (12) 含加成项。式 (11) 不含加成项。".
- **Change.** zt-check, for Topic-tree Cards and for every title: a rule for a book-local reference without its content (式 (n), 命题 x.y, 第 n 章, 本节, 模型 n), and a rule for deixis without a named referent in the same sentence. The card fix cycle rewrites: name the model, state the proposition by its content, say what the equation says. Literature-tree Cards may keep the book's numbers as labels (as L7 does for math units) but also name the content.
- **Benefit.** A Topic Card reads alone; the reader follows fewer Links to understand one Card.
- **Cost and risk.** Two rules and a fix pass for new digests; existing Cards through the card cycle only after the go-ahead. Risk: false hits (第一福利定理 holds "第"); the rules need tuning on one book.
- **Measure.** Share of Topic Cards with each fault (55% and 72% now).

#### P10. A short Literature Card and the book's footprint

**Decision (2026-10-18): Dropped.** The analysis stays as a record.

- **Luhmann.** One slip per title in a separate box; reading notes minimal, often a whole book on one slip; slip numbers on it make the bibliography a way in (L81 §II.3; S13 §2.1b; NLA-ZK §3.2b).
- **Gap.** The body of `stiglitz2014learning` has 13,534 characters (one line per group); it lists 1,445 Reverse links. It does not work as a way in.
- **Change.** The Literature Card body keeps the BibTeX data, the book's thesis in one or two sentences, the Link to its structure Card, and one line per chapter with its anchor; the section index stays on the anchors, where it already is. The GUI reader shows the book's footprint from the Reverse links: the Topics its Cards went to, with counts, inked by size.
- **Benefit.** The book's entry fits one screen, and the user sees where the book lives in the box.
- **Cost and risk.** A prompt or compiler change for the Literature Card; one GUI panel. Low risk.
- **Measure.** Literature Card length; steps from the Literature Card to a Topic Card of the book.

#### P11. Titles that tell Cards apart

**Decision (2026-10-18): Kept.**

- **Luhmann.** The register and the collective references wrote the concept beside each number, so each entry named what it led to (NLA-ZK §3.5b; HCD).
- **Gap.** 33 hubs carry the exact title of the Card they are filed under (`66/1|a` and its hub `66/1|a|g`); one pair of Idea Cards has the same title (`70/3`, `91/1|b`). Search results and Reverse links then show two entries with one name.
- **Change.** The hub compiler refuses a hub title equal to the title of its anchor (a new `get_hub` fault). Rule I6 (near-duplicate title) runs across the Workspace, not only inside one book.
- **Benefit.** Every entry in a list is distinct.
- **Cost and risk.** Small. Low risk.
- **Measure.** Exact and near-duplicate titles in the agent Workspace (34 exact now).

### D. Working with the box

#### P12. Ask the box: a question becomes a Spread

**Decision (2026-10-18): Kept.**

- **Luhmann.** Enter at a keyword with a research question, then follow the links; without a question the reader gets lost (S15 §4; S18 §6; L81 §III).
- **Gap.** The user reads a box of 10,887 agent Cards that grows by a book per run, against Luhmann's 2,000 slips a year. To build a set for a question, the user searches by substring and lays each Card on the Table by hand.
- **Change.** A GUI command and an MCP tool: a question → its register terms → their entry Cards → the kinded Links one or two steps out, across trees → a Spread of five to nine Cards, with the best entry held and threads named by kind. "File as card" (built) turns a kept Spread into a hub draft in the agent Workspace. The retrieval is deterministic; an agent may refine it.
- **Benefit.** Less time from a question to a set worth reading. Set-building becomes one step, and the Table already inks by attention.
- **Cost and risk.** Retrieval code (search plus a Link walk) and a GUI command. Risk: weak results while search is substring only; P8 and P13 come first.
- **Measure.** Time to a Spread the user keeps; share of gathered Cards the user removes.

#### P13. Entry-first search

**Decision (2026-10-18): Kept.**

- **Luhmann.** The register gave few places per term, the entry places; the links did the rest (S18 §6).
- **Gap.** `search_cards` "斯卢茨基" returns more than 40 hits in no useful order: hubs, restatements, Topic roots, one register line. The GUI tray groups them by tree.
- **Change.** Rank hits: register term, then hub lines, then Model Cards, then Cards whose title matches, then bodies. Fold a restatement cluster (P2) into one row with "+N". Show each hit's role (hub, model, finding, Topic). `search_cards` gets a ranked mode; the plain order stays available.
- **Benefit.** The first screen holds the ways in.
- **Cost and risk.** A change to the Rust search and the tray. Risk: a ranking that hides a hit the user wanted; the plain mode stays.
- **Measure.** Rank of the register's target Card for its term; steps to the first Card the user keeps.

#### P14. Reply slips: the user's Card beside an agent Card

**Decision (2026-10-18): Kept.**

- **Luhmann.** The box is a partner through the writer's own notes; a later thought answers an old slip by a new slip at its place, and the old slip stays (L81 §§I–II; S18 §§2, 7).
- **Gap.** The human Workspace has 8 Cards, and no human Card links an agent Card. To answer an agent Card, the user must pick a human tree, a place, and type a Link.
- **Change.** In the GUI reader, a Reply action (one key) opens the writing desk for a new human Card in the human Topic used last, or in a default Topic for replies. The body starts with the Link and a kind word that the user picks (`印证`, `反驳`, `放宽`, `对应`). The agent Card's Reverse links then show the reply. Only the user writes it.
- **Benefit.** The user's own thought grows at the point of reading, and the box starts to answer the user, not only the agents. Agents can read the replies as signals of interest.
- **Cost and risk.** A writing-desk change in the GUI. Risk: a cluttered reply Topic; the user can move replies later.
- **Measure.** Human Cards per week; Links from human Cards to agent Cards (0 now).

#### P15. One distant Card within reach

**Decision (2026-10-18): Kept.**

- **Luhmann.** Surprise comes from distant links and from the gap between the search pattern and the box's own horizons (L81 §§I, III; S18 §5).
- **Gap.** "Within reach" on the Table lists the held Card's own Links and its tree. Links between books are rare (6 of 200 sampled Topic Cards cite two books), so a reader seldom meets another book. Bridge reports stay in local files.
- **Change.** Beside the neighbours, the Table and the reader show at most one distant Card: a Card of another book that shares a term-table term or a model name with the held Card, or the target of a Bridge proposal, with one line on why. It stays faint until the reader points at it. Accepting it goes through a reply slip (P14) or a Bridge apply.
- **Benefit.** Surprise at reading time, with no new Link written in advance.
- **Cost and risk.** A small server query and a GUI slot. Risk: noise; one Card only, and faint.
- **Measure.** Share of distant Cards that the user opens or lays on the Table.

## Suggested order

| Step | Proposals | Why this order | Changes Cards? |
| --- | --- | --- | --- |
| 1 | Baseline: the measures above as a repeatable audit report | Every later step is judged against it. | No |
| 2 | P9, P11 (rules in zt-check and `get_hub`) | Cheap, and they make the faults visible. | No |
| 3 | P1, P2, P3, P4 on one pilot book | The digest prompts change together in one new Workflow version. | New pilot Cards only |
| 4 | P5, P7, P8 (concept gate, merge report, crossing hubs, register) | They need the concept scan, and the register needs P5's one-place rule. | Reports; moves after the go-ahead |
| 5 | P13, then P12, P15, P14 (GUI and MCP) | Ask the box rests on the register and the ranked search. | No agent Card; P14 lets the user write human Cards |

## Out of scope

- The digest Link-scope fault: [liudesheng9/zet#92](https://github.com/liudesheng9/zet/issues/92).
- The practices decided on 2026-10-15 and 2026-10-16 (P1 to P25 of [slip-box-luhmann-comparison.md](slip-box-luhmann-comparison.md), L1 to L7 of [math-cards-luhmann-comparison.md](math-cards-luhmann-comparison.md)): fixed numbers, slips that never move, one free-form slip type, own words in the agent Workspace, the writer's own style, back references by hand, the hub of hubs, and digests that file the whole book.
- Model, unit, and derivation structure (the model compiler, Derivation parts, Learn mode): this document measures how Topic Cards lean on models, not how models are built.
