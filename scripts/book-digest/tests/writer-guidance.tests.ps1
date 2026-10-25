#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\writer-guidance.tests.ps1
# S27 (#166, design 14.5): every writer and correction prompt carries the same concise guidance —
# preserve a Term macro or write the visible term plainly, never manufacture an address or swap in
# a synonym, run the exact SELF-CHECK before submit (a run with no `faults:` line is no check),
# treat a `card_check` refusal as content for the current finite fix cycle, and treat `card_locked`
# as a planning stop. The block is one text: this test compares it verbatim, so no prompt drifts
# into its own wording and no writer loses a rule.
# The live archive is never touched and no Card is written.
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }

# The one block. Its five bullets are R0444 (preserve the term), R0445 (no invented address or
# synonym), R0446 (the exact SELF-CHECK), R0447 (card_check is content) and R0448 (card_locked is a
# planning stop).
$block = @'
## 通用写入规则（每个写卡的任务都适用）

- **保留术语写法。** 正文里已有的术语链接 `[[词@地址]]` 原样保留；要提到一个已有术语、而正文没有现成链接时，写术语本身的文字，不要自己造链接。
- **不编造、不绕过。** 不要编造地址或链接目标，也不要为了让检查通过而换同义词、换关系词或改掉事实里的用词。
- **先自查。** 任务文件给出自查命令时（`SELF-CHECK:` 行，或 `CHECK:` 与 `LIMITS:` 两行），原样照抄，写完（或改完）就运行；最后一行是 `faults: 0` 才算通过，最多运行三次。命令没有打印 `faults:` 行（或任务文件指定的状态行）时，视为没有自查。
- **`card_check` 是内容问题。** 写入被拒绝为 `card_check` 时，拒绝里的每个故障行就是要改的内容：按它们改写后再写一次，仍在这一轮限定的修正次数以内。它不是脚本、服务或网络故障，不要原样重试。
- **`card_locked` 是选址决定。** 写入被拒绝为 `card_locked` 时不要重试，不要换 Access，不要改用别的写入路径或直接改数据库：保留已写好的答案，按任务文件的要求回复，并写明被拒绝的地址。
'@ -replace "`r`n", "`n"

# Every prompt that writes or corrects Card text. A supplement appended to one of these (the
# dsh-math-* digest, link, fill and review files appended by `math-compiler -Step docs`, and
# dsh-model-cards appended by prepare) inherits the block from its base, so it is not listed twice.
$writers = @(
    'dsh-digest-base.zh.md',
    'dsh-link-instructions.zh.md',
    'dsh-card-instructions.zh.md',
    'dsh-orphan-instructions.zh.md',
    'dsh-fill-instructions.zh.md',
    'dsh-review-instructions.zh.md',
    'dsh-explain-instructions.zh.md',
    'dsh-hub-instructions.zh.md',
    'dsh-event-instructions.zh.md',
    'dsh-finding-instructions.zh.md',
    'dsh-terms-instructions.zh.md',
    'dsh-skeleton-instructions.zh.md',
    'dsh-term-card-instructions.zh.md',
    'dsh-bridge-instructions.zh.md',
    'dsh-restate-instructions.zh.md',
    'dsh-math-boundary-instructions.zh.md',
    'dsh-math-proof-review.zh.md',
    'dsh-math-notation-instructions.zh.md',
    'dsh-math-theory-instructions.zh.md'
)
foreach ($name in $writers) {
    $path = Join-Path $repo "scripts\book-digest\$name"
    Assert (Test-Path -LiteralPath $path) "$name exists"
    $text = ([IO.File]::ReadAllText($path)) -replace "`r`n", "`n"
    Assert ($text.Contains($block)) "$name carries the exact shared writer guidance"
}
# The idea-grow writer is a direct-writing workflow of its own (it writes Topic-tree Cards through
# `zt-digest grow`), so it carries the same block (R0449: direct writers handle gate refusals).
$grow = ([IO.File]::ReadAllText((Join-Path $repo 'scripts\idea-grow\dsh-grow-instructions.zh.md'))) -replace "`r`n", "`n"
Assert ($grow.Contains($block)) 'the idea-grow writer carries the exact shared writer guidance'

# Each bullet states the rule, once: a prompt that paraphrases the block would fail above, and the
# rules are also checked one by one here so a later edit that removes one names the rule it broke.
foreach ($case in @(
        @{ rule = 'R0444 a Term macro is preserved or the term is written plainly'; text = '术语链接 `[[词@地址]]` 原样保留' },
        @{ rule = 'R0445 no invented address or synonym'; text = '不要编造地址或链接目标' },
        @{ rule = 'R0446 the exact SELF-CHECK, and no faults line is no check'; text = '视为没有自查' },
        @{ rule = 'R0447 card_check is content for the finite fix cycle'; text = '`card_check` 是内容问题' },
        @{ rule = 'R0448 card_locked is a planning stop'; text = '`card_locked` 是选址决定' }
    )) {
    Assert ($block.Contains($case.text)) "the block states $($case.rule)"
}

# The Term-card writer's own job line and prompt (S18/S19/S21 chain, S27): the job names its exact
# read-only precheck, and the prompt tells the writer where the self-check is.
$termPrompt = ([IO.File]::ReadAllText((Join-Path $repo 'scripts\book-digest\dsh-term-card-instructions.zh.md'))) -replace "`r`n", "`n"
Assert ($termPrompt.Contains('SELF-CHECK')) 'the Term-card prompt names its SELF-CHECK line'
Assert ($termPrompt.Contains('faults: 0')) 'the Term-card prompt states the faults: 0 goal'
Assert ($termPrompt.Contains('precheck')) 'the Term-card prompt names the precheck step of its job'

# Every prompt that already carried a SELF-CHECK still names the command it runs; the shared block
# does not replace a prompt's own concrete command (the block itself is removed first).
foreach ($name in @(
        'dsh-card-instructions.zh.md', 'dsh-orphan-instructions.zh.md', 'dsh-hub-instructions.zh.md',
        'dsh-event-instructions.zh.md', 'dsh-finding-instructions.zh.md', 'dsh-skeleton-instructions.zh.md',
        'dsh-bridge-instructions.zh.md', 'dsh-restate-instructions.zh.md', 'dsh-explain-instructions.zh.md',
        'dsh-link-instructions.zh.md'
    )) {
    $text = (([IO.File]::ReadAllText((Join-Path $repo "scripts\book-digest\$name"))) -replace "`r`n", "`n").Replace($block, '')
    Assert ($text.Contains('SELF-CHECK')) "$name still names its own SELF-CHECK command"
}

if ($failed) { Write-Output "$failed check(s) failed"; exit 1 }
Write-Output 'writer guidance: ok'
