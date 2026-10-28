# Plan a mathematical unit: supplement for the Sonnet architect

Read this after `model-architect.md`. Every rule there still holds. This file adds the rules for a **unit**: a Model Card whose `form` (in `context.md` and `get_model`) is `definition`, `axiom`, `theorem`, `proposition`, `lemma`, or `corollary`. A Theory Card (`form: theory`) never reaches you. A Model Card without a form is an ordinary model: plan it as before.

## Level

- A result that the book proves: `formal`.
- A result that the book states without proof, or proves in one line: `concept`. The Model Card states it, and its 思路 sentence gives the idea or says `书中未给出证明`.
- A definition or an axiom: `concept`.

## The facets

Use only these facets. A colon and a short name may follow (`证明：x0 存在`).

| Facet | Line | Emphasis | Holds |
| --- | --- | --- | --- |
| `前提` | main, first | none | the objects, the hypotheses with their labels (H1, H2), the symbols |
| `证明思路` | main, second | `core` | the method, the key idea, the crux step by name, the top-level steps |
| `证明：…` | main | `core` for the crux, else none | one top-level step, with a title that states what the step proves |
| `推导：…` | main | `aside` | a Derivation part, as in `model-architect.md` |
| `结论` | main, last (optional) | none | the step that closes the proof |
| `直觉`, `图示` | off | `core` for one of them | why the result should hold, with a chart when a picture helps |
| `例子` | off | `core` for a definition's first, else none | an object that meets the definition or the theorem, with its computation |
| `非例` | off | `core` for a definition's first, else none | an object that fails exactly one clause, named by its label |
| `反例：…` | off | none | an object that shows a hypothesis is necessary, or that the converse fails |
| `假设作用：Hi` | off | none | the step that uses Hi, with a Link to the counterexample |
| `等价刻画` | off | none | a Link to the result that proves an equivalent form |
| `另证`, `习题`, `历史`, `记号` | off | `aside` | a second proof, an exercise from the book, the history, the notation |

The boundary pass writes the `假设作用` and `反例` parts that the book does not give. Plan them only when the book gives them.

## Attention

- **At most three `core` parts per unit.** The Model Card is the fourth chunk. A definition's first `例子` and first `非例` count as one.
- **One crux step.** In a formal proof of three or more steps, mark the one `证明：…` step that carries the idea `core`. A proof of eight or more steps may have two. Name the crux step in the `证明思路` spec.
- For a theorem, the three are normally `证明思路`, the crux step, and `直觉`. For a definition, the contrast pair and `直觉`.
- Derivation parts, `另证`, `习题`, `历史`, and `记号` are `aside`.

## The proof

- The main line is the proof at three levels. First `证明思路`. Then one `证明：…` part per top-level step. Then Derivation parts for each step the book skips or compresses.
- A chain of `推导：…` parts also needs the `explain` entry of `model-architect.md` ("Design the explanations of each chain"): the chain's route and each part's shows, notes and meaning. `validate` refuses a chain without it.
- Write each `证明：…` title so that the main-line titles, read in order, state the proof in outline.
- A step that two proofs need is a lemma. Do not plan it as a shared part. Name it in `notes` ("the step … of … is a lemma that … also needs").

## Premises

- A premise that is another unit's **statement** is a cited premise: put its Model Card address in `rests` (`["Key/4|d", "用到介值定理"]`). `validate` accepts it. Never rest on a part of another unit (a reach-in).
- The book's notation table (title 符号表…) may be a premise.
- A formal unit still needs `symbols` and `terms` (the 前提 part's tables). Take the symbols from the book's notation table where it has them.

## Examples and exercises

The context names the book's examples and exercises that follow the unit (`本组的例与习题`). Plan each example as an `例子` part and each exercise as a `习题` part, with its `src` lines. An exercise answer stands under `## 答案` only when the book gives one.

## The Model Card

Your `card` field says what the Model Card holds. It holds **动机** (one to three sentences), **陈述** with labelled hypotheses and a conclusion, and **思路** in one sentence. A definition holds **定义** with labelled clauses in place of **陈述**. The writer maps the job's 问题 and 概要 to these sections.
