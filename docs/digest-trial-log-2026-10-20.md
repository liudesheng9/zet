# zt-econ-digest trial log: grossman2002integration (2026-10-20 to 2026-10-21)

The book is *Integration versus Outsourcing in Industry Equilibrium* (Grossman & Helpman; Scholar Archive file 75), digested from `一、引言` to `参考文献` into 8 groups, with DeepSeek V4.1 Flash from the official source at high effort. Trials 1–15 are recorded in the memory note `digest-test-loop-goal`. This log covers trials 16–20.

## Outcomes

| Trial | Versions | Outcome |
|---|---|---|
| 16 | 5.9.3 / zt-digest 0.5.0 | Digest, events, findings and cards 8/8; all 8 models built with 0 build faults. Stopped at model-finish: `7|a` and `8|a` were refused. |
| 17 | 5.9.3 → 5.9.6 | Digest, events, findings and cards 8/8; models 1–4 done. The second models claim came back without its units, so models 5–8 were never built. Resumed: the relay rewrote the resume summary, a Link block manager's answer failed, 6|a was refused (a writer kept a 沿用 rests-on sentence), and a relay refused a step after the user wrote "accelerate it". Stopped. |
| 18 | 5.9.6 | Stopped by the user to install the parallel model stage. |
| 19 | 5.9.7 → 5.9.9 | Relays refused cards-finish, findings-finish and ledger fail after "stop trial 18 … start trial 19". Resumed; the resume skipped the events, findings, cards-book and Bridge stages. Ran to the end, but with 3 models, link/g008, cards-book and the run-level cleanup failed. |
| 20 | 5.10.0 → 5.10.2 / zt-digest 0.5.3 | Every content stage done in one invocation: 8/8 digest, events, findings, cards and Link; all 7 models in one parallel block, with 0 build faults after two close rounds; explain 3/3; whole-book cycle, Organize, Figures, Bridge, Hubs, audit and finalize. The cleanup could not settle (a relay dropped the workspace paths); fixed and resumed. Final report: 0 failed and 0 open units (7 superseded), `cleanup_complete: true`, `content_complete: false`. |

Trial 20's only remaining gap is figure coverage. The report names 6 book figures that no Card names: 图 III, IV, V and VII, and two figures the source gives no label. This limit has been known since trial 10 and is still open.

## Fixes

zt-digest:
- **0.5.1:** a planned relation to a split part is authored by a Link to any of its pieces (`split_pieces`; trial 16).
- **0.5.2:** the close note for a closure fault names every word that reads as rests-on (trial 17, 6|a).
- **0.5.3:**
  - A reference to another model's split part accepts any of its pieces (`reference_pieces`; trial 19, 3|a).
  - An owned manifest that a card split extended keeps its frozen prefix (trial 19, link/g008).
  - `ledger resource` counts `-SweepEvidence` as an action (trial 19).

Workflow (base / econ):
- **4.9.4:** a claim line without a `units` array is unreadable, never an empty claim (trial 17).
- **4.9.5:** resume takes the group count from the digest gate when the relayed summary lacks its layer counts (trial 17).
- **4.9.6:** a block manager that throws is a block with no report, never a halted stage (trial 17).
- **4.9.7:** the model layer claims blocks of 8 and one architect batch plans up to 8 formal models (the user's parallelism request; quality steps unchanged).
- **4.9.8:** every relay prompt says that its command is a step of the run the user started, and that later messages don't cancel it (trials 17–19).
- **4.9.9:** the cards-book, Bridge, events and findings stages run in mode resume (trial 19).
- **4.10.0:**
  - Close runs up to three rounds.
  - The resume samples Idea Cards.
  - A fault takes the ledger's input hash.
  - The restatement apply uses a four-part prefix (trials 17–19).
- **4.10.1:** the cleanup knows a workspace by path or id (trial 20).
- **4.10.2:** on resume, a stage whose ledger layers are complete reuses its work (trial 20: the resume had rerun Organize).

Every fix has a harness case or a unit test. The full gate passed at each install: cargo test (328), the framework harness (0 failed) and run-all.

## Card review (trial 20)

- **Structure:** one Literature tree of 143 Cards. 8 group anchors chained by argument, at most 6 side successors each. 7 models: 1 concept and 6 formal, with 2|a as the base model and the other formal models taking its shared parts in instead of re-deriving them. 3 Topic trees (企业边界, 敲竹杠问题, 外部选择) and an index Topic with a field hub and the register.
- **Form:** no unbalanced `$`, no duplicate titles, and no Idea Card filed under a Model Card. Every Idea Card except the glossary (1|a) and the structure Card (1|g) carries a kinded outbound Link.
- **Models:** all 7 models pass model-finish with 0 closure, forward, unanchored or beyond-level faults. Derivations total 1–9 per formal model.
- **Open points:**
  - The 8 group anchors carry no 来源 line.
  - One restatement pair stays unverified.
  - The figure coverage gap above.

## Still open

- On resume, the concept review, the restatement review and the link-check card cycle have no ledger unit, so they rerun; the restatement apply's fixed revision `1` then collides with the first review's receipts. They should reuse their retained reports.
- Figure coverage: either name 图 III, IV, V and VII on the Cards that discuss them, or stop counting a source figure with no label as a content gap.
- The model-writer self-check (the user's method 2), beyond the three close rounds.
