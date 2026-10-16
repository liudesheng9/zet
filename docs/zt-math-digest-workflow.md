# The zt-math-digest workflow

`zt-math-digest` digests a mathematics textbook from the Scholar Archive. It writes into the agent Workspace. The file `.claude/workflows/zt-math-digest.js` holds it. It is the digest base (`zt-digest-workflow.md`) with math stages added. Every shared stage keeps its standard. The shared stages are the card cycles, the link pass, the link check, Organize, Figures, the hub stage, and the audit. All follow the same Card language. The DeepSeek Subagent Managed Protocol holds for every stage.

The current workflow is the frozen framework build (math 2.2.2, base 2.1.2). Edit `scripts/book-digest/workflow/zt-math-digest.blueprint.json` and its source modules, then assemble/install/verify at project scope as described in [the framework guide](zt-digest-framework.md). The legacy generator targets `.claude/workflows-legacy/zt-math-digest.js`; never run it against the installed framework artifact.

Bridge discovery runs after Figures and before Hubs. The math profile selects theory main results/objects, definitions, and units with load at least two; it retrieves literal theorem names as well as similarity candidates. An earlier economics Idea or Model head can cite a new unit through a separately accepted earlier-carrier proposal. It never enrolls a proof part. Mode bridge and its report-bound apply arguments follow [the shared operator contract](zt-digest-framework.md#bridge-discovery-and-apply). Source tests and actual live acceptance are separate gates.

## What it adds to the book digest

| Stage | When | Who | What it does |
| --- | --- | --- | --- |
| **Index** | after Prepare | script | Finds every numbered 定义, 公理, 定理, 命题, 引理, 推论, 例, and 习题 (and the English names). For each: its lines, its proof, its name, and the statements it cites. Writes `math\index.json` and `math\index.md`. |
| **Docs** | after Index | script | Writes the math prompts: `math-prefix.md`, `math-link-prefix.md`, and `math\brief.zh.md`, `fill.zh.md`, `review.zh.md`. Each is the shared file followed by its math supplement. |
| **Skeleton** | after Docs | script | Creates a unit Model Card (form, level, source lines) per definition, axiom, and result under its group's anchor, and a Theory Card per chapter. Each unit joins its group's `owned-gNN.txt`, so the model stage and the link pass treat it as the group's Card. Appends the units, examples, and exercises to each group file. |
| **Notation** | after Skeleton | one DeepSeek job, script | Writes the Book notation table (`符号表…`), a Side successor of `<key>/1`. |
| **Digest** | as in the book digest | DeepSeek | Each run reads the math prefix and writes its units' statements (动机, 陈述 with labels, 思路), besides its Idea, Event, and Model Cards. |
| **Models** | as in the book digest | model compiler | The architect reads `model-architect-math.md` too. The brief, fill, and review use the math files. A Theory Card is never compiled. |
| **Boundary** | after the models | DeepSeek writes, DeepSeek checks, script | Hypothesis roles with counterexamples for each result, and the contrast pair for each definition that lacks one. A second DeepSeek job checks each supplied item. The script deletes each wrong one with the role that names it. |
| **Proof** | after Boundary | DeepSeek, script | Reviews every proof plan, proof step, and conclusion. The script writes each fix that keeps every Link of the old body. |
| **Theory** | after the link check | DeepSeek, script | DeepSeek writes each Theory Card's title, question, main results, and objects. The script writes the dependency order (`结构`), the tools (`所用工具`), the users (`后续`), and the dependency map chart. |
| **Math check** | after the audit | script | Rules M1 to M18 on every unit and Theory Card: `math-audit.json` and `math-audit.md` in `out`. Read-only. |

Every math stage runs its DeepSeek jobs through `drive`, the book digest's own driver. A Haiku manager runs at most 8 jobs. At most 8 managers run at once in the whole run. Each stage's last step deletes its conversations and its DeepSeek workspace (`math\notation`, `math\boundary`, `math\proof`, `math\theory`).

## Arguments

All arguments of the base (`zt-digest-workflow.md`) hold. These are new:

| Argument | Meaning | Default |
| --- | --- | --- |
| `mode` | adds `math`: only Boundary, Proof, Theory, and the math check on an existing math digest | `full` |
| `supply` | `none`, `examples`, or `examples+answers`: what DeepSeek may add that the book does not give. Every supplied item carries `（补：书中未给）`. | `examples` |
| `boundary`, `proof`, `theory`, `notation` | `false` skips that stage | `true` |
| `unitsPerJob` | units per boundary or proof job | 4 |
| `mainResults` | the most main results per Theory Card | 7 |

For a textbook, pass `skipHeadings: "参考文献,习题"` only when you do not want the exercises: the math workflow files each exercise beside its unit.

## Running it

The workflow uses the scripts in `E:\zet\scripts\book-digest`. Merge the `math-cards` branch and deploy zt 0.19.0 first: the math stages need the `form` argument and the math reports of `get_model`.

```
Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-math-digest.js", args: {
  source: "<the book's Chinese bilingual Markdown>", out: "D:\\zt-digest\\<key>",
  citationKey: "<key>", bibtex: "@book{<key>, title={…}}", bookTitle: "<中文书名>",
  startHeading: "<first chapter heading>", endHeading: "参考" } })
```

Pass an args object, not free text. Run a new workflow file by `scriptPath` in the session that edited it. A full digest changes the slip-box: take a database backup first, and ask the user before the run.
