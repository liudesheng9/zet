# 找出数学对象，认领每一段原文（数学消化流程）

你为《{{TITLE}}》（{{KEY}}）读几个原文文件，做两件事：

1. 找出原文里的每一个数学对象：定义、公理、定理、命题、引理、推论、例、注、记号、习题、图，以及编者说明和版式杂项。
2. 认领每一段原文：每一段只属于一个对象，或者属于一段说明文字（exposition）。

本说明之后附上任务文件：几个原文文件，每行开头是行号和 `│`，行号不属于原文。只读任务文件，不读别的文件，不上网。

后面的阶段为每个定义和结果建一张单元卡，为每章的习题建一组习题，其余的说明文字写成笔记。读者在一个地方学一件事：所以每一段原文只能有一个归属。漏掉的段落会让书不完整，归属两次会让读者读两遍。

## 片段（fragment）

答案是一个片段列表。每个片段有一个 `role`：

| role | 用途 |
|---|---|
| `object` | 一个数学对象：它的陈述，以及写在它旁边的证明。 |
| `proof` | 一个证明，它的定理在别处陈述（例如本章附录里的证明）。 |
| `hint` | 一道习题的提示。 |
| `answer` | 一道习题的书中答案。 |
| `exposition` | 一段连续的说明文字：动机、过渡、历史、对读者的话。 |
| `dismiss` | 一行以编号开头、但不是一个对象的行（页眉、OCR 重复的一行）。 |

## object 的字段

- `kind`：以下之一，用英文小写：`definition`（定义）、`axiom`（公理）、`theorem`（定理）、`proposition`（命题）、`lemma`（引理）、`corollary`（推论）、`example`（例）、`remark`（注）、`notation`（记号）、`exercise`（习题）、`figure`（图）、`editorial`（编者或译者的说明）、`boilerplate`（版权页、目录等版式杂项）。书自己用了词时，按书的词选。书没有用词时，看内容：一般规则是结果，具体算例是例。书在正文里用“称为”“叫作”“记作”或粗体、斜体第一次说清一个概念（例如“若存在 M 使……，则称序列有界”），那一句是一个没有编号的 `definition`，不是说明文字。
- `label`：书印的编号，只写编号，如 `1.19`、`19`、`III`。书没有印编号时写空字符串 `""`。不要自己编号。习题照书印的样子写：书印 `2.26.` 就写 `2.26`。
- 一个编号下的几个小题或几个部分（(a)、(b)、(c)）是同一个对象：整段写成一个 `object`，不要为每个小题另写对象，`sublabel` 不写。
- `name`：中文短名，最多 20 字，如 `实数域的存在`。
- `spans`：陈述所在的原文范围，见下文。陈述从印着编号的那一行开始。
- `proof`：只用于定理、命题、引理、推论。
  - `status`：`inline`（证明就写在陈述旁边）、`deferred_in_slice`（证明在本次处理的这几章里的另一处，例如本章附录）、`source_omits`（书没有给证明）、`external`（书说证明见别的书，或见本次没有给出的后面章节，例如“见第 8 章”）。
  - `steps`：证明的各步，按原文顺序。每步有 `label`（书印的步名，如 `第 1 步`；没有时写 `""`）和 `spans`。`inline` 必须写步；没有分步的证明写成一步。`deferred_in_slice` 在这里不写步，由证明所在的地方写一个 `proof` 片段。
  - `reason`：`source_omits` 和 `external` 必须写一句原因。
  - 定义、公理、例、注、记号、习题不写 `proof`。
- `destination`：只用于 `example`、`remark`、`notation`。读者在哪里学它：
  - `{"to": "unit", "target": {"namespace": "theorem", "label": "1.20"}}`：它服务于那个结果，写进那个结果的单元。
  - `{"to": "narrative"}`：它写进本章的笔记。
- `relations`：可选。它和别的对象的关系，每项有 `rel` 和 `target`：
  - `cites`：它用到另一个对象。
  - `contains`：另一个对象（例如证明中间的一个例）嵌在它的范围里。
  - `illustrates`：一个例说明另一个对象。
  - `target` 写 `{"namespace": "theorem", "label": "1.19"}`。`namespace` 就是那个对象的 `kind`。不知道种类时只写 `label`。没有编号的对象写它的 `name`：`{"namespace": "example", "name": "取 x=1 的情形"}`，名字要和那个对象的 `name` 一字不差。另一章的对象，`1.19` 这样的编号已经说明章；别的编号加 `"chapter": "ch2"`。

## proof、hint、answer 的字段

- `target`：它属于哪个对象，写法同上：`{"namespace": "theorem", "label": "1.19"}`，习题写 `{"namespace": "exercise", "label": "2"}`。这里必须写编号，不能只写名字。
- 提示或答案只针对某一小题时，`target` 仍是整道习题；它的文字本身说明是哪一小题。
- `evidence`：本任务里一行原文，证明它属于那个对象。`{"section": "s05.md", "line": 3, "quote": "定理 1.19 的证明"}`。`quote` 必须原样抄自那一行，并且包含目标的编号。找不到这样的一行时，仍写出片段，`evidence` 写你能找到的最近的一行；检查会说明它没有对上。
- `proof` 写 `steps`（同上）；`hint` 和 `answer` 写 `spans`。
- 一个证明被分在几个任务里时，每个任务写自己看到的那几步，各写一个 `proof` 片段。

## exposition 与 dismiss

- `exposition`：`spans` 里只写一个范围，连续的几行。它不能包含印着对象编号的行，也不能跨章。说明文字很长时，分成几个 `exposition`。
- `dismiss`：`{"role": "dismiss", "section": "s03.md", "line": 23, "reason": "OCR 把 1.21 印了两遍"}`。只用于以编号开头、但不是对象的行。这一行的文字仍要归属：通常归一个 `editorial`。
- 自查会列出每一行以编号开头、却没有对象从这一行开始的地方：要么写一个从这一行开始、编号相同的对象，要么 `dismiss` 这一行。

## 范围（span）

`{"section": "s05.md", "line": 3, "end": 7, "quote": "定理 1.19 的证明"}`

- `section`：文件名。`line`、`end`：任务文件里的行号，`end` 是最后一行。
- `quote`：从这个范围里原样抄 4 到 20 个字，不加不改。范围里的字少于 4 个时，抄全部。
- 一行里有两个对象（例如习题后面紧跟提示），用字符位置把这一行分开：`col` 是开始的位置，`end_col` 是结束后的位置，从 0 数，按字符数，不按字节。例：`{"section": "s09.md", "line": 4, "end": 4, "end_col": 17, "quote": "2. 证明不存在"}` 和 `{"section": "s09.md", "line": 4, "end": 4, "col": 17, "quote": "提示：仿照"}`。
- 两个片段的范围不重叠。例外：一个对象用 `contains` 写明另一个对象嵌在它里面，并且外面的范围完全包住里面的对象。
- 标题行（`#` 开头）不用认领。以编号开头的标题行（如 `# 3.4 定理。`）是对象的第一行，要认领。

## 结果文件

用写文件工具把 JSON 写到任务文件给出的路径（只写这一个文件）。按原文顺序列出片段。反斜杠在 JSON 里写成 `\\`，双引号写成 `\"`。下面的例子来自一个虚构的章节，展示每种片段：

```json
{"schema": 2, "fragments": [
  {"role": "exposition", "spans": [{"section": "s01.md", "line": 3, "end": 5, "quote": "我们先回顾有理数"}]},
  {"role": "object", "kind": "definition", "label": "1.17", "name": "有序域", "spans": [{"section": "s01.md", "line": 7, "end": 9, "quote": "1.17 定义"}]},
  {"role": "object", "kind": "theorem", "label": "1.19", "name": "实数域的存在", "spans": [{"section": "s01.md", "line": 11, "end": 13, "quote": "1.19 定理"}],
   "proof": {"status": "deferred_in_slice"}, "relations": [{"rel": "cites", "target": {"namespace": "definition", "label": "1.17"}}]},
  {"role": "object", "kind": "theorem", "label": "1.20", "name": "阿基米德性质", "spans": [{"section": "s01.md", "line": 15, "end": 15, "quote": "1.20 定理"}],
   "proof": {"status": "inline", "steps": [{"label": "", "spans": [{"section": "s01.md", "line": 17, "end": 23, "quote": "证明。设结论不成立"}]}]},
   "relations": [{"rel": "contains", "target": {"namespace": "example", "name": "取 x=1 的情形"}}]},
  {"role": "object", "kind": "example", "label": "", "name": "取 x=1 的情形", "spans": [{"section": "s01.md", "line": 19, "end": 19, "quote": "例如取 x=1"}],
   "destination": {"to": "unit", "target": {"namespace": "theorem", "label": "1.20"}}},
  {"role": "object", "kind": "corollary", "label": "", "name": "根的乘积", "spans": [{"section": "s01.md", "line": 25, "end": 25, "quote": "推论。若 a 与 b"}],
   "proof": {"status": "inline", "steps": [{"label": "", "spans": [{"section": "s01.md", "line": 27, "end": 27, "quote": "证明。由唯一性"}]}]}},
  {"role": "object", "kind": "remark", "label": "1.22", "name": "小数", "spans": [{"section": "s01.md", "line": 29, "end": 31, "quote": "1.22 小数"}], "destination": {"to": "narrative"}},
  {"role": "proof", "target": {"namespace": "theorem", "label": "1.19"}, "evidence": {"section": "s02.md", "line": 3, "quote": "定理 1.19 的证明"},
   "steps": [{"label": "第 1 步", "spans": [{"section": "s02.md", "line": 3, "end": 9, "quote": "本附录给出定理 1.19"}]},
             {"label": "第 2 步", "spans": [{"section": "s02.md", "line": 11, "end": 15, "quote": "第 2 步 定义"}]}]},
  {"role": "object", "kind": "exercise", "label": "1", "name": "有理加无理", "spans": [{"section": "s03.md", "line": 3, "end": 3, "quote": "1. 若 r 为非零"}]},
  {"role": "object", "kind": "exercise", "label": "2", "name": "根号十二", "spans": [{"section": "s03.md", "line": 4, "end": 4, "end_col": 17, "quote": "2. 证明不存在"}]},
  {"role": "hint", "target": {"namespace": "exercise", "label": "2"}, "evidence": {"section": "s03.md", "line": 4, "quote": "2. 证明不存在"},
   "spans": [{"section": "s03.md", "line": 4, "end": 4, "col": 17, "quote": "提示：仿照"}]},
  {"role": "object", "kind": "exercise", "label": "3", "name": "闭包的三个性质", "spans": [{"section": "s03.md", "line": 6, "end": 9, "quote": "3. 设 E 是度量空间"}]},
  {"role": "hint", "target": {"namespace": "exercise", "label": "3"}, "evidence": {"section": "s03.md", "line": 10, "quote": "第 3 题 (c) 的提示"},
   "spans": [{"section": "s03.md", "line": 10, "end": 10, "quote": "第 3 题 (c) 的提示"}]},
  {"role": "answer", "target": {"namespace": "exercise", "label": "1"}, "evidence": {"section": "s04.md", "line": 3, "quote": "1. 答案"},
   "spans": [{"section": "s04.md", "line": 3, "end": 3, "quote": "1. 答案：r+x 若为有理数"}]},
  {"role": "object", "kind": "editorial", "label": "", "name": "OCR 重复的一行", "spans": [{"section": "s04.md", "line": 5, "end": 5, "quote": "3. 设 E 是度量空间"}]},
  {"role": "dismiss", "section": "s04.md", "line": 5, "reason": "OCR 把第 3 题的开头又印了一遍"}
]}
```

## 自查

写完后运行任务文件 `SELF-CHECK:` 行的命令，原样照抄。它每个问题输出一行，最后一行是 `faults: N`。其中 `is claimed by no fragment` 的行列出你还没有认领的原文：把每一段放进一个对象、一个证明步、一个提示或答案，或者一段 exposition。N 不是 0 时，按每一行改正文件，再运行一次。最多运行四次。

最后只回复一行 JSON：`{"fragments":N,"faults":M}`。N 是你写出的片段数，M 是最后一次自查的 faults 数。
