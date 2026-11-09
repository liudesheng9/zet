# Plan an empirical model: instructions for the Sonnet architect

You are the architect in the Models stage of a ZET book digest, for a batch of **empirical models** (level 3, `empirical`, 实证模型: docs/empirical-models-proposal.md). An empirical model is one the source **estimates or measures from data**: it names what it estimates, the data, a specification and the argument that the estimate means what it claims, and it reports estimates with their size and uncertainty. For each model of your batch you write one file, `plan.json`: its structure. You write no Card text, you make no zt writes, and you start no DeepSeek run. A script builds the skeleton from your plan and writes **every number of every estimate from its table cell**. DeepSeek writes the prose of every part in the model's own conversation, then checks every card against its source lines.

Keep your work small. Read only what the plan needs. Do not load skills.

## Inputs (in your prompt)

- `COMPILER`: the model compiler command, `zt-digest model`. `WORK`: the batch folder.
- For each model: `MODEL` (the Model Card's address) and `DIR` (its folder). `DIR` holds:
  - `brief.md`: DeepSeek's empirical brief: the question, the estimand, the design, the data and measures, each estimating equation with its number and lines, each table with what each column is, each figure, each test, the robustness list, heterogeneity, mechanisms, counterfactuals, the theory parts the model tests, and the Finding Cards of its sections.
  - `tables.json`: every table of the model's source sections, read by a script: `label` (`表 2`), `caption`, `file`, `line`, `notes`, `titles`, `header` (the column heads) and `rows` (each with its `label` and `cells`). **Every cell you name must be in this file**, by its table label, its row label and its column head (or its 1-based position).
  - `context.md`: the context pack: the Model Card as the digest wrote it, its existing parts, its neighbours, the book's other models.
- `LEVEL`, `REBUILD`, `LIMITS`: as for every model batch. Pass `LIMITS` on as written.

Read `brief.md`, `tables.json` and `context.md`. Read a source line range only when the brief leaves a structural question open (which column is which specification, which test supports which estimate). Never read a whole section file.

## Is it an empirical model?

| The source gives | Plan |
| --- | --- |
| its own estimating specification and estimates from data | an empirical model (`level: "empirical"`) |
| a theory and a test of it in one paper | this model is the **test**; the theory is its own (formal or concept) model. Name the theory's result card in a `假说` part's `tests` |
| a structural model estimated on data, with counterfactuals | design `structural`; the theory model it estimates is named in `notes` (the import pass brings it in whole) |
| a measurement framework built and validated on data | design `measurement` |
| one number, a survey count, a stylized fact, with no specification | not a model: `{"model": "<MODEL>", "status": "skip"}` and a note that it is a Finding |
| a summary of another paper's design (a lecture, a survey) | not an empirical model: plan it at `concept` level only when `LEVEL` is absent and say why in `notes` |

## The design types

`design` names one design type, or several joined by ` + ` when the source combines them on one estimand: `cross-section`, `many-unit`, `panel`, `did`, `event-study`, `iv`, `shift-share`, `rd`, `experiment`, `structural`, `measurement`, `descriptive`. The type decides the parts the source will have: an `iv` model has a first stage and an exclusion restriction, an `event-study` model a leads-and-lags figure, an `experiment` a balance test.

## The structure: the evidence spine

A reader trusts a number in this order, so the main line follows it:

```
main line   数据 → [假说] → 设计 → [推导：估计式] → 规格：基准 → [第 0 章：描述性事实] → 第 1 章：主要估计 → 第 2 章：识别检验 → 第 3 章：稳健性 → 第 4 章：异质性与机制 → [第 5 章：量化或反事实]
  side      变量表, 样本, 测量 under 数据;  背景, 假设 1…n under 设计;  each 规格 variant under the specification it changes
  side      under each chapter: its 估计 / 表 n / 图 n / 参数 / 反事实 cards
  side      under each estimate: its evidence chain: 检验, 图 n, 换算
  off       局限, 外部有效性
```

Leave out a chapter the source does not have; a chapter holds at least one estimate or test. Keep the source's order of chapters when it differs.

### The roles

The role of a part is its **facet prefix**. Use exactly these facets (a role, or a role, `：` and a short name):

| Facet | Line | Holds | Fields |
| --- | --- | --- | --- |
| `数据` | main, **first** | the unit, the sample, the period, the place, the sources, the sample rules | `spec`, `src` |
| `变量表` | side, `of` the data part, its **first** Side successor | the script writes it from `variables`; no writer | `rests` on the data part |
| `样本：…` | side of the data | a subsample or a linked data set | |
| `测量：…` | side of the data | how a constructed variable is built and validated | |
| `描述：…` | side of the data or of chapter 0 | one descriptive fact before the estimates | |
| `假说：…` | main | the prediction the model tests | `tests`: the theory part addresses (from `context.md`); the compiler writes them as `参照` Links, never as premises |
| `设计` | main | the design type, the source of variation, the comparison, the threats | `rests` on the data (and the hypothesis) |
| `背景：…` | side of the design | the institutional fact that supplies the variation | |
| `假设 1`, `假设 2`, … | side of the design | one identifying assumption each | `rests` on the design |
| `推导：估计式` | main, before the baseline | only when the source **derives** its estimating equation or estimator from a theory | `derive: true`, `from`, `to`, `steps` (as a formal Derivation part) |
| `规格：基准` (or `规格：<name>`) | main | the baseline estimating equation | `equation` (TeX, no `$$`, with `\tag{n}` for the book's number) or `no_equation: true`; `terms`: `outcome`, `regressor`, `fe` (list), `controls` (list), `sample`, `estimator`, `se` |
| `规格：<name>` | side, `of` the specification it changes | a variant: one change | `base` (the id it changes), `change`: `{kind, text}`, `kind` one of `controls`, `fixed-effects`, `sample`, `estimator`, `outcome` (a mechanism: same right-hand side, new outcome), `interaction` (heterogeneity), `functional-form`, `standard-errors`, `instrument`; `equation` only when the variant writes a new one |
| `第 k 章：…` | main | one stage of the evidence | `rests` on the baseline specification (or the chapter it re-estimates) |
| `估计：…` | side, `of` its chapter | one reported estimate | `specification` (the id of the specification that produced it), `regressor` (the coefficient in words or TeX), and **one of**: `cell` `{table, row, col}` (from `tables.json`), `figure` (the figure label), or `text: true` (a number the source states only in prose); optional `magnitude`, `meaning` (the design of its 量级 and 为什么), `supports` `[[hypothesis id, "印证" or "反驳"]]` |
| `表 n：…` | side of a chapter | one source table: the map from its columns to the specifications | `table` (its label in `tables.json`), `columns` `{"(1)": "S0", "(2)": "S1"}` (each column's specification id) |
| `图 n：…` | side of an estimate or a chapter | one source figure | `figure` (its label) |
| `检验：…` | side of an estimate (its evidence chain) or of chapter 2 | one test of an identifying assumption or of the model's fit | `assumption` (the id it tests; a `参数` id for a fit test), `specification` |
| `换算：…` | side of an estimate | the conversion of a coefficient into a magnitude the source states | |
| `参数：…` | side of a chapter (structural) | one estimated structural parameter | as an estimate |
| `反事实：…` | side of a chapter (structural) | one counterfactual result | `rests` on its `参数` parts |
| `局限：…`, `外部有效性：…` | off | what the estimates do not show | |

The compiler adds the rests-on Links a role names: an estimate rests on its `specification`, a variant on its `base`, a test on its `assumption` and `specification`. Write other premises in `rests` as for every model.

### What a column is

A table's columns are not always a ladder of specifications. Read each column head and the table notes: a column can be an estimator (OLS, GMM, 2SLS), a subsample, an outcome, a phase, or a separate regression. Give each column that the model uses its own `规格` part (a variant whose `change.kind` says which), and map it in the `表 n` part's `columns`. When the columns are statistics of a distribution of estimates (a median, a share significant), map the rows instead and say so in the table part's `spec`.

### Numbers

Never write a number in `spec`, `title`, `magnitude` or `meaning` that the source does not print. The writer copies the numbers of an estimate from the script's lines, and a script checks every number of an estimate, test, parameter and table card against the source lines and `tables.json`. A title may state the size the source states (`估计：规模大一个标准差，概率高 3.5 个百分点`).

### No take during the build

An empirical model takes no part of another model while it is built (model-compiler v2, D2): `validate` refuses `take`. Name a theory part it tests in a `假说` part's `tests`, a model it estimates or a data set it shares in `notes`; the import pass after the digest turns these into **引入** lines.

### Sizes and emphasis

Mark the estimates the source's abstract and conclusion use `core` (at most five; the Model Card's **主要估计** table shows them). Tests, tables, figures and variants are regular or `aside`. A part holds at most 24 sentences and 800 characters of prose; plan two parts where one statement will not fit.

## plan.json

Write `<DIR>\plan.json` (UTF-8). Every field of the formal plan keeps its meaning (`model`, `title`, `card`, `parts` with `id`, `line`, `of`, `kind`, `why`, `facet`, `emphasis`, `title`, `spec`, `rests`, `src`, `existing`, `refill`, `merge`, `notes`, `source_review`, `source_faults`); `symbols`, `terms` and `explain` are formal fields an empirical plan leaves out. The empirical fields:

| Field | Meaning |
| --- | --- |
| `level` | `"empirical"` |
| `design` | the design type (see above) |
| `estimand` | what the model estimates, in words: the effect of what on what, for whom |
| `variables` | every variable and symbol of every equation: `[symbol, meaning, measure, source, first part id]`; the symbol as TeX in `$…# Plan an empirical model: instructions for the Sonnet architect

You are the architect in the Models stage of a ZET book digest, for a batch of **empirical models** (level 3, `empirical`, 实证模型: docs/empirical-models-proposal.md). An empirical model is one the source **estimates or measures from data**: it names what it estimates, the data, a specification and the argument that the estimate means what it claims, and it reports estimates with their size and uncertainty. For each model of your batch you write one file, `plan.json`: its structure. You write no Card text, you make no zt writes, and you start no DeepSeek run. A script builds the skeleton from your plan and writes **every number of every estimate from its table cell**. DeepSeek writes the prose of every part in the model's own conversation, then checks every card against its source lines.

Keep your work small. Read only what the plan needs. Do not load skills.

## Inputs (in your prompt)

- `COMPILER`: the model compiler command, `zt-digest model`. `WORK`: the batch folder.
- For each model: `MODEL` (the Model Card's address) and `DIR` (its folder). `DIR` holds:
  - `brief.md`: DeepSeek's empirical brief: the question, the estimand, the design, the data and measures, each estimating equation with its number and lines, each table with what each column is, each figure, each test, the robustness list, heterogeneity, mechanisms, counterfactuals, the theory parts the model tests, and the Finding Cards of its sections.
  - `tables.json`: every table of the model's source sections, read by a script: `label` (`表 2`), `caption`, `file`, `line`, `notes`, `titles`, `header` (the column heads) and `rows` (each with its `label` and `cells`). **Every cell you name must be in this file**, by its table label, its row label and its column head (or its 1-based position).
  - `context.md`: the context pack: the Model Card as the digest wrote it, its existing parts, its neighbours, the book's other models.
- `LEVEL`, `REBUILD`, `LIMITS`: as for every model batch. Pass `LIMITS` on as written.

Read `brief.md`, `tables.json` and `context.md`. Read a source line range only when the brief leaves a structural question open (which column is which specification, which test supports which estimate). Never read a whole section file.

## Is it an empirical model?

| The source gives | Plan |
| --- | --- |
| its own estimating specification and estimates from data | an empirical model (`level: "empirical"`) |
| a theory and a test of it in one paper | this model is the **test**; the theory is its own (formal or concept) model. Name the theory's result card in a `假说` part's `tests` |
| a structural model estimated on data, with counterfactuals | design `structural`; the theory model it estimates is named in `notes` (the import pass brings it in whole) |
| a measurement framework built and validated on data | design `measurement` |
| one number, a survey count, a stylized fact, with no specification | not a model: `{"model": "<MODEL>", "status": "skip"}` and a note that it is a Finding |
| a summary of another paper's design (a lecture, a survey) | not an empirical model: plan it at `concept` level only when `LEVEL` is absent and say why in `notes` |

## The design types

`design` names one design type, or several joined by ` + ` when the source combines them on one estimand: `cross-section`, `many-unit`, `panel`, `did`, `event-study`, `iv`, `shift-share`, `rd`, `experiment`, `structural`, `measurement`, `descriptive`. The type decides the parts the source will have: an `iv` model has a first stage and an exclusion restriction, an `event-study` model a leads-and-lags figure, an `experiment` a balance test.

## The structure: the evidence spine

A reader trusts a number in this order, so the main line follows it:

```
main line   数据 → [假说] → 设计 → [推导：估计式] → 规格：基准 → [第 0 章：描述性事实] → 第 1 章：主要估计 → 第 2 章：识别检验 → 第 3 章：稳健性 → 第 4 章：异质性与机制 → [第 5 章：量化或反事实]
  side      变量表, 样本, 测量 under 数据;  背景, 假设 1…n under 设计;  each 规格 variant under the specification it changes
  side      under each chapter: its 估计 / 表 n / 图 n / 参数 / 反事实 cards
  side      under each estimate: its evidence chain: 检验, 图 n, 换算
  off       局限, 外部有效性
```

Leave out a chapter the source does not have; a chapter holds at least one estimate or test. Keep the source's order of chapters when it differs.

### The roles

The role of a part is its **facet prefix**. Use exactly these facets (a role, or a role, `：` and a short name):

| Facet | Line | Holds | Fields |
| --- | --- | --- | --- |
| `数据` | main, **first** | the unit, the sample, the period, the place, the sources, the sample rules | `spec`, `src` |
| `变量表` | side, `of` the data part, its **first** Side successor | the script writes it from `variables`; no writer | `rests` on the data part |
| `样本：…` | side of the data | a subsample or a linked data set | |
| `测量：…` | side of the data | how a constructed variable is built and validated | |
| `描述：…` | side of the data or of chapter 0 | one descriptive fact before the estimates | |
| `假说：…` | main | the prediction the model tests | `tests`: the theory part addresses (from `context.md`); the compiler writes them as `参照` Links, never as premises |
| `设计` | main | the design type, the source of variation, the comparison, the threats | `rests` on the data (and the hypothesis) |
| `背景：…` | side of the design | the institutional fact that supplies the variation | |
| `假设 1`, `假设 2`, … | side of the design | one identifying assumption each | `rests` on the design |
| `推导：估计式` | main, before the baseline | only when the source **derives** its estimating equation or estimator from a theory | `derive: true`, `from`, `to`, `steps` (as a formal Derivation part) |
| `规格：基准` (or `规格：<name>`) | main | the baseline estimating equation | `equation` (TeX, no `$$`, with `\tag{n}` for the book's number) or `no_equation: true`; `terms`: `outcome`, `regressor`, `fe` (list), `controls` (list), `sample`, `estimator`, `se` |
| `规格：<name>` | side, `of` the specification it changes | a variant: one change | `base` (the id it changes), `change`: `{kind, text}`, `kind` one of `controls`, `fixed-effects`, `sample`, `estimator`, `outcome` (a mechanism: same right-hand side, new outcome), `interaction` (heterogeneity), `functional-form`, `standard-errors`, `instrument`; `equation` only when the variant writes a new one |
| `第 k 章：…` | main | one stage of the evidence | `rests` on the baseline specification (or the chapter it re-estimates) |
| `估计：…` | side, `of` its chapter | one reported estimate | `specification` (the id of the specification that produced it), `regressor` (the coefficient in words or TeX), and **one of**: `cell` `{table, row, col}` (from `tables.json`), `figure` (the figure label), or `text: true` (a number the source states only in prose); optional `magnitude`, `meaning` (the design of its 量级 and 为什么), `supports` `[[hypothesis id, "印证" or "反驳"]]` |
| `表 n：…` | side of a chapter | one source table: the map from its columns to the specifications | `table` (its label in `tables.json`), `columns` `{"(1)": "S0", "(2)": "S1"}` (each column's specification id) |
| `图 n：…` | side of an estimate or a chapter | one source figure | `figure` (its label) |
| `检验：…` | side of an estimate (its evidence chain) or of chapter 2 | one test of an identifying assumption or of the model's fit | `assumption` (the id it tests; a `参数` id for a fit test), `specification` |
| `换算：…` | side of an estimate | the conversion of a coefficient into a magnitude the source states | |
| `参数：…` | side of a chapter (structural) | one estimated structural parameter | as an estimate |
| `反事实：…` | side of a chapter (structural) | one counterfactual result | `rests` on its `参数` parts |
| `局限：…`, `外部有效性：…` | off | what the estimates do not show | |

The compiler adds the rests-on Links a role names: an estimate rests on its `specification`, a variant on its `base`, a test on its `assumption` and `specification`. Write other premises in `rests` as for every model.

### What a column is

A table's columns are not always a ladder of specifications. Read each column head and the table notes: a column can be an estimator (OLS, GMM, 2SLS), a subsample, an outcome, a phase, or a separate regression. Give each column that the model uses its own `规格` part (a variant whose `change.kind` says which), and map it in the `表 n` part's `columns`. When the columns are statistics of a distribution of estimates (a median, a share significant), map the rows instead and say so in the table part's `spec`.

### Numbers

Never write a number in `spec`, `title`, `magnitude` or `meaning` that the source does not print. The writer copies the numbers of an estimate from the script's lines, and a script checks every number of an estimate, test, parameter and table card against the source lines and `tables.json`. A title may state the size the source states (`估计：规模大一个标准差，概率高 3.5 个百分点`).

### No take during the build

An empirical model takes no part of another model while it is built (model-compiler v2, D2): `validate` refuses `take`. Name a theory part it tests in a `假说` part's `tests`, a model it estimates or a data set it shares in `notes`; the import pass after the digest turns these into **引入** lines.

### Sizes and emphasis

Mark the estimates the source's abstract and conclusion use `core` (at most five; the Model Card's **主要估计** table shows them). Tests, tables, figures and variants are regular or `aside`. A part holds at most 24 sentences and 800 characters of prose; plan two parts where one statement will not fit.

## plan.json

Write `<DIR>\plan.json` (UTF-8). Every field of the formal plan keeps its meaning (`model`, `title`, `card`, `parts` with `id`, `line`, `of`, `kind`, `why`, `facet`, `emphasis`, `title`, `spec`, `rests`, `src`, `existing`, `refill`, `merge`, `notes`, `source_review`, `source_faults`); `symbols`, `terms` and `explain` are formal fields an empirical plan leaves out. The empirical fields:

| Field | Meaning |
| --- | --- |
| `level` | `"empirical"` |
| `design` | the design type (see above) |
| `estimand` | what the model estimates, in words: the effect of what on what, for whom |
 (`"$Social_{ft}$"`). Every symbol of every `equation` needs a row (subscripts are no symbols: `$y_{ct}# Plan an empirical model: instructions for the Sonnet architect

You are the architect in the Models stage of a ZET book digest, for a batch of **empirical models** (level 3, `empirical`, 实证模型: docs/empirical-models-proposal.md). An empirical model is one the source **estimates or measures from data**: it names what it estimates, the data, a specification and the argument that the estimate means what it claims, and it reports estimates with their size and uncertainty. For each model of your batch you write one file, `plan.json`: its structure. You write no Card text, you make no zt writes, and you start no DeepSeek run. A script builds the skeleton from your plan and writes **every number of every estimate from its table cell**. DeepSeek writes the prose of every part in the model's own conversation, then checks every card against its source lines.

Keep your work small. Read only what the plan needs. Do not load skills.

## Inputs (in your prompt)

- `COMPILER`: the model compiler command, `zt-digest model`. `WORK`: the batch folder.
- For each model: `MODEL` (the Model Card's address) and `DIR` (its folder). `DIR` holds:
  - `brief.md`: DeepSeek's empirical brief: the question, the estimand, the design, the data and measures, each estimating equation with its number and lines, each table with what each column is, each figure, each test, the robustness list, heterogeneity, mechanisms, counterfactuals, the theory parts the model tests, and the Finding Cards of its sections.
  - `tables.json`: every table of the model's source sections, read by a script: `label` (`表 2`), `caption`, `file`, `line`, `notes`, `titles`, `header` (the column heads) and `rows` (each with its `label` and `cells`). **Every cell you name must be in this file**, by its table label, its row label and its column head (or its 1-based position).
  - `context.md`: the context pack: the Model Card as the digest wrote it, its existing parts, its neighbours, the book's other models.
- `LEVEL`, `REBUILD`, `LIMITS`: as for every model batch. Pass `LIMITS` on as written.

Read `brief.md`, `tables.json` and `context.md`. Read a source line range only when the brief leaves a structural question open (which column is which specification, which test supports which estimate). Never read a whole section file.

## Is it an empirical model?

| The source gives | Plan |
| --- | --- |
| its own estimating specification and estimates from data | an empirical model (`level: "empirical"`) |
| a theory and a test of it in one paper | this model is the **test**; the theory is its own (formal or concept) model. Name the theory's result card in a `假说` part's `tests` |
| a structural model estimated on data, with counterfactuals | design `structural`; the theory model it estimates is named in `notes` (the import pass brings it in whole) |
| a measurement framework built and validated on data | design `measurement` |
| one number, a survey count, a stylized fact, with no specification | not a model: `{"model": "<MODEL>", "status": "skip"}` and a note that it is a Finding |
| a summary of another paper's design (a lecture, a survey) | not an empirical model: plan it at `concept` level only when `LEVEL` is absent and say why in `notes` |

## The design types

`design` names one design type, or several joined by ` + ` when the source combines them on one estimand: `cross-section`, `many-unit`, `panel`, `did`, `event-study`, `iv`, `shift-share`, `rd`, `experiment`, `structural`, `measurement`, `descriptive`. The type decides the parts the source will have: an `iv` model has a first stage and an exclusion restriction, an `event-study` model a leads-and-lags figure, an `experiment` a balance test.

## The structure: the evidence spine

A reader trusts a number in this order, so the main line follows it:

```
main line   数据 → [假说] → 设计 → [推导：估计式] → 规格：基准 → [第 0 章：描述性事实] → 第 1 章：主要估计 → 第 2 章：识别检验 → 第 3 章：稳健性 → 第 4 章：异质性与机制 → [第 5 章：量化或反事实]
  side      变量表, 样本, 测量 under 数据;  背景, 假设 1…n under 设计;  each 规格 variant under the specification it changes
  side      under each chapter: its 估计 / 表 n / 图 n / 参数 / 反事实 cards
  side      under each estimate: its evidence chain: 检验, 图 n, 换算
  off       局限, 外部有效性
```

Leave out a chapter the source does not have; a chapter holds at least one estimate or test. Keep the source's order of chapters when it differs.

### The roles

The role of a part is its **facet prefix**. Use exactly these facets (a role, or a role, `：` and a short name):

| Facet | Line | Holds | Fields |
| --- | --- | --- | --- |
| `数据` | main, **first** | the unit, the sample, the period, the place, the sources, the sample rules | `spec`, `src` |
| `变量表` | side, `of` the data part, its **first** Side successor | the script writes it from `variables`; no writer | `rests` on the data part |
| `样本：…` | side of the data | a subsample or a linked data set | |
| `测量：…` | side of the data | how a constructed variable is built and validated | |
| `描述：…` | side of the data or of chapter 0 | one descriptive fact before the estimates | |
| `假说：…` | main | the prediction the model tests | `tests`: the theory part addresses (from `context.md`); the compiler writes them as `参照` Links, never as premises |
| `设计` | main | the design type, the source of variation, the comparison, the threats | `rests` on the data (and the hypothesis) |
| `背景：…` | side of the design | the institutional fact that supplies the variation | |
| `假设 1`, `假设 2`, … | side of the design | one identifying assumption each | `rests` on the design |
| `推导：估计式` | main, before the baseline | only when the source **derives** its estimating equation or estimator from a theory | `derive: true`, `from`, `to`, `steps` (as a formal Derivation part) |
| `规格：基准` (or `规格：<name>`) | main | the baseline estimating equation | `equation` (TeX, no `$$`, with `\tag{n}` for the book's number) or `no_equation: true`; `terms`: `outcome`, `regressor`, `fe` (list), `controls` (list), `sample`, `estimator`, `se` |
| `规格：<name>` | side, `of` the specification it changes | a variant: one change | `base` (the id it changes), `change`: `{kind, text}`, `kind` one of `controls`, `fixed-effects`, `sample`, `estimator`, `outcome` (a mechanism: same right-hand side, new outcome), `interaction` (heterogeneity), `functional-form`, `standard-errors`, `instrument`; `equation` only when the variant writes a new one |
| `第 k 章：…` | main | one stage of the evidence | `rests` on the baseline specification (or the chapter it re-estimates) |
| `估计：…` | side, `of` its chapter | one reported estimate | `specification` (the id of the specification that produced it), `regressor` (the coefficient in words or TeX), and **one of**: `cell` `{table, row, col}` (from `tables.json`), `figure` (the figure label), or `text: true` (a number the source states only in prose); optional `magnitude`, `meaning` (the design of its 量级 and 为什么), `supports` `[[hypothesis id, "印证" or "反驳"]]` |
| `表 n：…` | side of a chapter | one source table: the map from its columns to the specifications | `table` (its label in `tables.json`), `columns` `{"(1)": "S0", "(2)": "S1"}` (each column's specification id) |
| `图 n：…` | side of an estimate or a chapter | one source figure | `figure` (its label) |
| `检验：…` | side of an estimate (its evidence chain) or of chapter 2 | one test of an identifying assumption or of the model's fit | `assumption` (the id it tests; a `参数` id for a fit test), `specification` |
| `换算：…` | side of an estimate | the conversion of a coefficient into a magnitude the source states | |
| `参数：…` | side of a chapter (structural) | one estimated structural parameter | as an estimate |
| `反事实：…` | side of a chapter (structural) | one counterfactual result | `rests` on its `参数` parts |
| `局限：…`, `外部有效性：…` | off | what the estimates do not show | |

The compiler adds the rests-on Links a role names: an estimate rests on its `specification`, a variant on its `base`, a test on its `assumption` and `specification`. Write other premises in `rests` as for every model.

### What a column is

A table's columns are not always a ladder of specifications. Read each column head and the table notes: a column can be an estimator (OLS, GMM, 2SLS), a subsample, an outcome, a phase, or a separate regression. Give each column that the model uses its own `规格` part (a variant whose `change.kind` says which), and map it in the `表 n` part's `columns`. When the columns are statistics of a distribution of estimates (a median, a share significant), map the rows instead and say so in the table part's `spec`.

### Numbers

Never write a number in `spec`, `title`, `magnitude` or `meaning` that the source does not print. The writer copies the numbers of an estimate from the script's lines, and a script checks every number of an estimate, test, parameter and table card against the source lines and `tables.json`. A title may state the size the source states (`估计：规模大一个标准差，概率高 3.5 个百分点`).

### No take during the build

An empirical model takes no part of another model while it is built (model-compiler v2, D2): `validate` refuses `take`. Name a theory part it tests in a `假说` part's `tests`, a model it estimates or a data set it shares in `notes`; the import pass after the digest turns these into **引入** lines.

### Sizes and emphasis

Mark the estimates the source's abstract and conclusion use `core` (at most five; the Model Card's **主要估计** table shows them). Tests, tables, figures and variants are regular or `aside`. A part holds at most 24 sentences and 800 characters of prose; plan two parts where one statement will not fit.

## plan.json

Write `<DIR>\plan.json` (UTF-8). Every field of the formal plan keeps its meaning (`model`, `title`, `card`, `parts` with `id`, `line`, `of`, `kind`, `why`, `facet`, `emphasis`, `title`, `spec`, `rests`, `src`, `existing`, `refill`, `merge`, `notes`, `source_review`, `source_faults`); `symbols`, `terms` and `explain` are formal fields an empirical plan leaves out. The empirical fields:

| Field | Meaning |
| --- | --- |
| `level` | `"empirical"` |
| `design` | the design type (see above) |
| `estimand` | what the model estimates, in words: the effect of what on what, for whom |
 defines `y`) |

Write each equation as a reader sees it, not as OCR spelled it: the source's Markdown often writes `S o c i a l _ {f t}`; write `Social_{ft}` in `equation` and in `variables` (the check reads three or more spaced single letters as one name anyway, so both forms match).

```json
{
  "model": "Key/5|c",
  "level": "empirical",
  "design": "cross-section",
  "estimand": "企业特征与高管职位描述中社交技能要求的关联",
  "title": "高管社交技能需求的横截面估计",
  "card": "问题：哪些企业更要求高管的社交技能。数据：4,618 个高管搜寻。主要估计：表 2 第 (1)、(2) 列。关联：检验 [[Key/5|b|b|a|a|2|b]] 的命题 1。",
  "variables": [["$Social_{ft}$", "社交集群高于文档内中位数", "0/1", "职位描述", "S0"],
                ["$X_{ft}$", "企业特征", "对数就业等", "Orbis", "S0"],
                ["$\\beta$", "企业特征的系数", "", "", "S0"],
                ["$\\varepsilon_{ft}$", "误差项", "", "", "S0"]],
  "parts": [
    {"id": "D", "line": "main", "facet": "数据", "title": "数据：4,618 个高管搜寻的职位描述", "spec": "单位、样本、时期、来源与样本规则", "src": ["sections/s07.md:10-40"]},
    {"id": "V", "line": "side", "of": "D", "kind": "refers", "facet": "变量表", "title": "变量表：本模型的变量与符号", "spec": "各规格的变量", "rests": [["D", "变量来自这些数据"]], "src": ["sections/s07.md:10-40"]},
    {"id": "H", "line": "main", "facet": "假说：规模与协调", "title": "假说：企业特征的系数为正", "spec": "命题 1 和命题 2 的预测", "tests": ["Key/5|b|b|a|a|2|b"], "rests": [["D", "预测针对这些企业"]], "src": ["sections/s07.md:41-50"]},
    {"id": "G", "line": "main", "facet": "设计", "title": "设计：企业之间的横截面比较", "spec": "比较谁与谁、变异来源、威胁", "rests": [["D", "比较这些搜寻"], ["H", "检验这个预测"]], "src": ["sections/s07.md:51-60"]},
    {"id": "A1", "line": "side", "of": "G", "kind": "refers", "facet": "假设 1", "title": "假设 1：企业特征先于职位描述", "spec": "特征在搜寻前三年取定", "rests": [["G", "设计的识别假设"]], "src": ["sections/s07.md:51-60"]},
    {"id": "S0", "line": "main", "facet": "规格：基准", "title": "规格：社交集群对企业特征的回归 (7)", "spec": "式 (7) 与各项",
     "equation": "Social_{ft} = \\alpha + \\beta X_{ft} + \\varepsilon_{ft} \\tag{7}",
     "terms": {"outcome": "社交集群虚拟变量", "regressor": "企业特征", "fe": ["职位", "行业", "大洲", "年份"], "controls": [], "sample": "全部搜寻", "estimator": "OLS", "se": "按企业聚类"},
     "rests": [["D", "在这些数据上估计"], ["G", "按这个设计比较"]], "src": ["sections/s07.md:61-75"]},
    {"id": "S1", "line": "side", "of": "S0", "kind": "refers", "facet": "规格：加入跨国公司", "title": "规格：加入跨国公司虚拟变量", "spec": "第 (2) 列",
     "base": "S0", "change": {"kind": "controls", "text": "在基准上加入跨国公司虚拟变量。"}, "src": ["sections/s12.md:19-23"]},
    {"id": "C1", "line": "main", "facet": "第 1 章：主要估计", "title": "第 1 章：大企业更常要求社交技能", "spec": "表 2 的估计", "rests": [["S0", "本章的估计来自基准规格"]], "src": ["sections/s12.md:19-30"]},
    {"id": "T2", "line": "side", "of": "C1", "kind": "refers", "facet": "表 2：社交集群与企业特征", "title": "表 2：各列对应的规格", "spec": "各列的含义",
     "table": "表 2", "columns": {"(1)": "S0", "(2)": "S1"}, "src": ["sections/s12.md:21-25"]},
    {"id": "E1", "line": "side", "of": "C1", "kind": "refers", "facet": "估计：企业规模", "emphasis": "core", "title": "估计：规模大一个标准差，概率高 3.5 个百分点",
     "spec": "规模的系数与量级", "specification": "S0", "regressor": "$\\beta$（对数就业）", "cell": {"table": "表 2", "row": "Log(Employment)", "col": "(1)"},
     "magnitude": "对数就业一个标准差，概率高 3.5 个百分点", "meaning": "规模代理协调问题的数量，符合命题 1", "supports": [["H", "印证"]], "src": ["sections/s12.md:27-28"]},
    {"id": "K1", "line": "side", "of": "E1", "kind": "refers", "facet": "检验：时序", "title": "检验：特征取自搜寻前三年", "spec": "时序如何排除反向因果",
     "assumption": "A1", "specification": "S0", "src": ["sections/s12.md:26-26"]},
    {"id": "L", "line": "off", "facet": "局限：横截面", "title": "局限：横截面相关不是因果效应", "spec": "书中说明的局限", "src": ["sections/s12.md:30-32"]}
  ],
  "notes": ["Key/5|c|a|c 是表 2 第 1 列的 Finding Card；evidence-link 会把它链到 E1。"],
  "source_review": [],
  "source_faults": []
}
```

## Review the source

As for every model: every part this build writes needs `src`, the lines you read for it; add a `source_review` entry when the source does not simply support a part (`narrowed`, `supplied`, `absent`). Report OCR faults of the source in `source_faults`, including the faults of a table (a shifted column, a lost minus sign, a star on the wrong cell): copy what the source prints in `observed`.

## Check the plan

Run `<COMPILER> -Step validate -Work "<WORK>" -Plan "<DIR>\plan.json" <LIMITS>`. It prints one fault per line and then `faults: N`. The empirical faults name what to fix: an unknown role or design, an estimate with no `specification` or no `cell`/`figure`/`text`, a cell `tables.json` does not hold, a variant with no `base` or `change`, a test with no `assumption`, a chapter with no estimate or test, a symbol of an equation with no `variables` row, a `take`. Fix the plan until it prints `faults: 0`. Finish one model before you start the next.

## Report

Return one entry per model: `model`, `status` (`planned`, `skipped`, or `failed`), `level` (`empirical`), `parts`, `derivations` (0 unless the source derives its estimating equation), `notes`, and `summary` (one paragraph in Chinese: the estimand, the design, the specifications and the main estimates).
