# Derivation Explanations: Proposal

Status (2026-10-15): phases 1 to 3 exist and pass their tests. Phase 4 (the pilot on `hart1995firms/11|b|a|4`) ran, and the user approved it. Phase 5 (the existing books) runs book by book, on the user's go-ahead for each book. See "As built" below.

A Derivation part (a Model part whose facet starts with `推导`) shows one step of a model's algebra. Today it holds three sections: **起点**, **步骤**, and **结果**. The steps have no errors, and a reader can check each line.

But the Card does not tell the reader three things. It does not say why the derivation takes this route. It does not explain a step that makes a choice. It does not say what the result means. A reader can follow the algebra and still not understand the model.

This proposal adds explanation to every Derivation part. It adds these things:

1. A **思路** section before **起点**: what the derivation shows and the route it takes.
2. A **说明** line under a step that makes a choice: a case split, an approximation, a change of variable, or a limit.
3. A **解读** section after **结果**: what the result means, and a special case that checks it. It also says what the next part of the model does with the result.
4. A **直觉** item in the brief, so that the writer uses the book's own intuition first.
5. Check rules for the new sections, and a new review question.
6. An **explain** step that adds the new sections to the Derivation parts that exist now, and changes no step.

## What the Derivation parts look like now

The agent Workspace holds 1,446 Derivation parts (counted on 2026-10-15). The numbers show the gap:

| Measure | Value |
| --- | --- |
| Parts with only the sections 起点, 步骤, 结果 | 1,376 (95%) |
| Parts with a word of intuition or meaning (思路, 直觉, 直观, 含义, 为什么, 意味着) | 53 (4%) |
| Numbered steps per part, median and maximum | 6 and 16 |
| Characters per part (with formulas), median | 1,016 |

Two examples from `hart1995firms` show the gap in detail:

- `hart1995firms/11|b|a|4` derives the participation constraint (5.10). Steps 3 to 5 split the work into the cases $f=1$ and $f<1$. No line says why the cases are necessary. The reason is the kink of $\min[c,P]$ at $c=P$. The Card also does not say that $w$ cancels, so the entrepreneur's wealth does not change the participation constraint.
- `hart1995firms/12|c|a|2` shows that the probability of a pivotal vote goes to zero. It uses Stirling's approximation in step 4 and gives no reason for it. It also does not state the rate: the probability falls like $1/\sqrt{N}$.

The cause is in the instructions. `dsh-fill-instructions.zh.md` gives a Derivation part only three sections and asks for "an equation and its reason" on each step line. `dsh-review-instructions.zh.md` checks only for errors, jumps, and density. No instruction asks for intuition, and no check finds that it is missing.

## Decisions

| No. | Question | Decision | Reason |
| --- | --- | --- | --- |
| D1 | Where does the explanation go? | **Into each Derivation part**, as new sections. There is no new Card. | A reader who opens one part must see why it exists. A separate intuition Card is one more Card to find, and a chain already has many Cards. |
| D2 | Which sections? | **思路** before 起点, **说明** under a step, **解读** after 结果. | Each answers one question: why this route, why this move, what the result means. |
| D3 | Which steps get a 说明 line? | **Any step that a reader may not see at once.** A step that makes a choice must get one. A mechanical step (add a term to both sides) needs only its reason. There is no fixed count. | The user wants more explanation. The explanation budget (D4) stops a part from becoming an essay. |
| D4 | Do the Card size limits change? | **Yes. The explanation gets its own budget**, separate from R13. 思路, 说明, and 解读 together hold at most 16 sentences and 600 characters. Formulas and Links do not count. R13 no longer counts these sections in a Derivation part. R14 (6 numbered steps) does not change. Two new workflow arguments set the budget: `maxExplainSentences` (16) and `maxExplainChars` (600). | The user wants looser limits and more explanation. The steps stay short and checkable. The explanation can grow without a split. Only the steps decide a split: a part over its explanation budget shortens the explanation and does not answer `split`. |
| D5 | Where does the intuition come from? | **The book first.** The briefer records the book's own intuition for each equation. The writer marks its own intuition with `（补：书中未写）`. | This is the rule that the steps follow with `（补：书中略去）`. A reader must see which words are the book's. |
| D6 | Does the Sonnet architect do more? | **Superseded (2026-10-27, #195) by §3.7 of [the one-conversation proposal](model-one-conversation-proposal.md): the architect does more.** `plan.json` carries an `explain` entry for every chain of Derivation parts (`route`, and each part's `shows`, `notes` and `meaning`); `plan-status` refuses a chain without it, and DeepSeek writes the text in the model's own conversation from that design. A script still writes the chain position into 思路. | The owner's decision of 2026-10-27: the explanations are part of the model job, and their design belongs to the one designer role. This document's other decisions (D1-D5, D7-D9) stand. |
| D7 | How does a chain read as one argument? | **The first part's 思路 names the route of the whole chain**: the cut results in order. Every later part's 思路 starts with its position, for example `第 2 张，共 3 张`. | The reader of any part sees where it sits in the chain. The script knows the chain, so this line costs no model. |
| D8 | How do the existing parts get the sections? | **A new `explain` step.** DeepSeek writes JSON. The script inserts the sections and checks that 起点, every step, and 结果 do not change. | This is the card-compiler pattern (decisions D11 and D12 of `digest-card-layers-proposal.md`). It cannot damage a checked derivation. |
| D9 | Does `explain` rebuild a model? | **No.** It adds sections to finished parts. It never runs the brief, the plan, the fill, or the review of a finished model. | Finished models stay as built (`digest-no-model-reruns`). |

## The new Derivation part

A Derivation part has five sections in this order. The new sections are 思路, the 说明 lines, and 解读.

| Section | Holds | Limit |
| --- | --- | --- |
| **思路** | What the part shows. The route: the key move and why it works. Why the obvious route does not work, when the book says so. In a chain, the position of the part. | 1 to 5 sentences |
| **起点** | As today: the premises with `依赖` Links. | no change |
| **步骤** | As today: one equation and its reason per line. Under a step that a reader may not see at once, one indented line `说明：…`. | R14: 6 steps. At most one 说明 line per step, each of 1 or 2 sentences. |
| **结果** | As today: the result equation with the book's number. | no change |
| **解读** | What the result means for the model. How the result changes when a parameter changes. One or more checks: a special case, a sign, or a limit. What the next part does with the result. | 1 to 6 sentences |

A 说明 line explains a step that a reader may not see at once. These steps must get one:

- a split into cases, and the condition that splits them
- an approximation or a bound, and why it is good enough
- a new variable or a new definition
- a limit, and the term that dominates it
- a step that uses a fact from outside the model

Other steps may get one. Examples are a step that joins two earlier results, and a step where a reader can misread the sign of an inequality. A step whose reason already says everything gets none.

The example below shows `hart1995firms/11|b|a|4` with the new sections. The steps that do not change are cut short. The intuition is the writer's own, so it carries the mark `（补：书中未写）`.

```
**思路**
E 只在还款后所得超过 $w$ 时参与。先求 E 还款后剩下的现金 $R$，再代入参与约束。$\min[c,P]$ 在 $c=P$ 处转折，所以分两种情形求 $R$。（补：书中未写）

**起点**
- 依赖设定 [[hart1995firms/11|b|a]]：E 参与，须所得高于初始财富 $w$。
- …

**步骤**
1. 参与约束：E 的所得高于 $w$，即 $R+fy_2>w$。
2. …
3. 情形 $f=1$：$c\ge P$ 且 $(1-f)L=0$，所以 $R=c-P=y_1-(K-w)+(1-f)L$。（补：书中略去）
   说明：不清算时，现金够还款，E 留下差额。
4. 情形 $f<1$：$c<P$，所以 $R=c-c=0$。（补：书中略去）
   说明：清算时，现金全部用来还款，E 不留现金。
5. …
   说明：C 只清算到收回 $P$ 为止，所以 E 的剩余为零，这一行与步骤 3 的式子相同。
6. 两种情形都有 $R=y_1-(K-w)+(1-f)L$。代入步骤 1，得 $y_1-(K-w)+(1-f)L+fy_2>w$。
   说明：两种情形得到同一个 $R$，所以后面的推导不再分情形。
7. 两边加 $K-w$，得 $y_1+fy_2+(1-f)L>K$。（补：书中略去）

**结果**
$$y_1+fy_2+(1-f)L>K \quad (5.10)$$

**解读**
(5.10) 说：计入部分清算后，项目的价值须超过成本 $K$。$w$ 在两边相消，所以 E 的财富不影响参与。E 的财富只影响融资条件 [[hart1995firms/11|b|a|5]]。清算越多，$y_2$ 的权重越小，$L$ 的权重越大。检验一：$f=1$ 时 (5.10) 化为 $y_1+y_2>K$，即普通的净现值为正。检验二：$f=0$ 时它化为 $y_1+L>K$，即立即清算也要收回成本。（补：书中未写）
```

The new prose has 13 sentences and about 270 characters without formulas and Links. It fits the explanation budget of D4 (16 sentences, 600 characters). Under the old R13 budget, this much explanation needed a split.

## Changes, file by file

### 1. Brief: the book's own intuition

`dsh-brief-instructions.zh.md` gets one new item per equation: **直觉**. It records the book's own words that explain the equation or its derivation, with file and lines. When the book gives none, the item says `书中没有`. The brief limit stays at 6,000 characters. The item holds line references and a short paraphrase, not quotes.

### 2. Fill and derive: the writer's instructions

`dsh-fill-instructions.zh.md` changes in four places:

1. The format block shows the five sections and the example above.
2. A new subsection "讲清推导" gives the rules: what 思路 and 解读 hold, and which steps get a 说明 line. It also gives the order of sources: the brief's 直觉 item, then the source lines, then the writer's own words with `（补：书中未写）`.
3. Rule 8 (keep the book's modality) applies to 解读. The writer does not state an economic claim that the result does not support.
4. A check in 解读 must be a computation from the result, for example a special case. It is not a new fact.

The writer answers `split` only when the steps do not fit (R14). When the explanation is over its budget, the writer shortens it. A split part gets its own 思路 and 解读.

### 3. Compiler: the skeleton and the chain position

`model-compiler.ps1` changes in three steps:

- **apply** writes the skeleton of a Derivation part with five sections. 思路 and 解读 hold `待写`. In a chain, 思路 starts with the position line, for example `第 2 张，共 3 张：从 $P=B$ 到 $B\le L$。`
- **split** writes the position line again for every part of a changed chain. A split changes the number of parts.
- **report** counts the parts that still hold `待写` in 思路 or 解读.

### 4. Check: new rules R15 to R17

`zt-check` gets three rules for Derivation parts. They apply to formal models and to models with no level, like R13 and R14.

| Rule | Checks | Fault |
| --- | --- | --- |
| R15 | The five sections are present, in order. | A section is missing, or the order is wrong. |
| R16 | 思路 has 1 to 5 sentences. 解读 has 1 to 6 sentences. Each sentence follows R2 (40 characters, formulas not counted). | Too many sentences, an empty section, or a long sentence. |
| R17 | At most one 说明 line under each numbered step, of 1 or 2 sentences, each at most 40 characters. 思路, 说明, and 解读 together hold at most `-MaxExplainSentences` sentences (16) and `-MaxExplainChars` characters (600). | A second 说明 line under a step, a long sentence, or an explanation over its budget. |

R13 no longer counts 思路, 说明, and 解读 in a Derivation part. A part with neither 思路 nor 解读 is an older part. It raises no R15 fault unless the check gets `-RequireExplain`. The writers and the `explain` step set it. The build report and `digest-audit.ps1` do not set it. The audit counts the parts with both sections per book, so the user sees the gap close.

### 5. Review: one new question

`dsh-review-instructions.zh.md` adds a fifth question to "逐张检查": **讲没讲清**.

- Does 思路 match the steps? A route that the steps do not take is a fault.
- Does each 说明 line explain a real choice?
- Does 解读 follow from 结果? Does its check give the right value? Does it state a claim that the result does not support?
- Is each sentence of intuition that the book does not state marked `（补：书中未写）`?

A fault here gives the verdict `fix`, with the section and the sentence in `reason`. It never gives `split`. The writer fixes it in the same conversation, as today.

### 6. Explain step: the parts that exist now

A new compiler step, `explain`, adds the sections to finished Derivation parts. It works like the card cycle:

1. **explain -Models …** lists the Derivation parts of each model that lack 思路 or 解读. It writes one job per chain, or per three parts of a long chain. A job holds the setting, the Model Card, the bodies of the parts, and the path of the context pack (`model-context.ps1`). The writer reads source lines from the pack. A job also holds the part before the chain and the part after it.
2. DeepSeek writes one JSON file per job: for each part, `idea` (思路), `notes` (step number to 说明 text), and `meaning` (解读).
3. **explain-apply** inserts the sections. It refuses a part when 起点, a step line, or 结果 differs from the stored text, or when R15 to R17 fail. A refused part goes back to the same conversation once.
4. **explain-report** gives the counts per model: explained, refused, and still missing.

The workflow gets a new mode, `explain`, with the arguments `only` (Model Card addresses) `explainMax` (most parts, for a pilot), and the budget arguments `maxExplainSentences` and `maxExplainChars` of D4. The jobs run under the DeepSeek Subagent Managed Protocol: at most 8 jobs per Haiku manager and at most 8 managers at once. The cleanup deletes the conversations when the step ends.

## Cost

New builds pay only for longer answers. A larger explanation budget gives longer answers, so the derive jobs cost more output tokens. The derive jobs and the review jobs do not change in number.

The `explain` step is a one-time cost for the existing parts. 1,446 parts at three parts per job give about 480 DeepSeek jobs and about 60 Haiku managers. The model compiler pilot cost about $0.01 per DeepSeek call (94 calls, $0.99). An `explain` job reads more text than a pilot call, so the total can be $5 to $15. The pilot below measures the real number before any whole-book run.

## Plan

| Phase | Work | Changes Cards? |
| --- | --- | --- |
| 1 | Instructions (brief, fill, review), R15 to R17 in the checker, audit counts. Tests on fixtures. | No |
| 2 | Compiler: five-section skeleton, chain position, report counts. Tests on a scratch archive (`model-compiler.tests.ps1`). | No |
| 3 | `explain` and `explain-apply`, mode `explain`. Tests on a scratch archive. | No |
| 4 | Pilot: mode `explain` on one model, `hart1995firms/11|b` (its Derivation parts only). Database backup first. The user reads every changed part. | Yes, about 5 parts |
| 5 | One book at a time, after the user approves the pilot. Audit before and after each book. | Yes |
| 6 | New digests write the sections in the derive round. No `explain` step is necessary for them. | Yes, in new digests |

Phases 4 and 5 change Cards in the slip-box. Each needs the user's go-ahead.

## As built

Phases 1 to 3 exist. The build differs from the plan above in these points:

| Point | As built |
| --- | --- |
| R15 on older parts | Silent unless the caller passes `-RequireExplain` (see section 4). The plan called the rules "advisory". |
| Source lines in an `explain` job | The job names the context pack, and the writer reads the source lines. A finished part has no `src` in a plan. |
| Instructions of the `explain` step | A new file, `dsh-explain-instructions.zh.md`. DeepSeek writes `idea`, `notes` (step number to text), and `meaning`, or `blocked`. |
| Position line | `第 1 张，共 n 张。全链依次推出 …。` for the first part, `第 k 张，共 n 张：从 … 推到 …。` for the others. It names each result's formula only. `apply`, `split`, and `explain` write it. A chain of one part gets none. The check does not count it. |
| A split part | It keeps its steps and their 说明 lines. Its 结果 becomes the first cut, and its 解读 becomes `待写`. The follow-up asks the writer to write it again. |
| `explain-apply` | It refuses a part that changed after its job and a part whose core would change. It also refuses a part with a fault that the old body does not have. It keeps a fault that the old body already has, and reports it without counting it. It stops with an error when the check does not run. A refused part goes back once. A second refusal is final. |
| `explain-check` | The writer's own check: the same test as `explain-apply`, without a write. Each job file names it on its `SELF-CHECK` line. |

The tests:

- `zt-check.tests.ps1`: R15 (a missing 解读), R16, and R17 (a second 说明 under one step, the budget). An older part is silent, and `-RequireExplain` flags it. R13 does not count the explanation.
- `model-compiler.tests.ps1`: the five-section skeleton and its position lines, a split that rewrites the positions and clears 解读, and the `explained` count. It also runs the `explain` cycle: plan, `explain-check`, apply with a refusal and a blocked part, the follow-up, and the report. The written parts pass R15 to R17, and 起点, the steps, and 结果 stay the same.
- `workflow-harness.mjs`: mode `explain` runs plan, apply, apply, report, and sends the refused part back to the same conversation.

## Verification

- The tests show that `explain-apply` refuses a changed step, a changed 起点, and a changed 结果.
- After the pilot, R15 to R17 report 0 faults on the pilot model. Every step line and every result equation stays the same. A script compares them with the backup.
- The reviewer's verdicts on the pilot chains are `ok`, or `fix` with a reason that the fix round closes.
- The user reads the pilot parts and judges whether a reader now understands the route and the result.

## Open questions

| No. | Question |
| --- | --- |
| Q1 | Are 思路, 说明, and 解读 the right labels? Other options are 直觉 and 含义. |
| Q2 | Should 解读 add a chart when the result is a curve or a region? A chart helps intuition, but it makes the part longer. |
| Q3 | Should the `explain` step run on every book, or only on books that the user reads often? |
| Q4 | Are 16 sentences and 600 characters the right explanation budget? The pilot shows how much of it the writers use. |
