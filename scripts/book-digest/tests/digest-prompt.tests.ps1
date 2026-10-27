#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\digest-prompt.tests.ps1
# S28 (#186): the digest group job writes Literature notes. The prompt must not carry the Idea-card
# standard (a fixed Card count, the 80–200 character body, one Card one idea, the STE sentence rules,
# the required relation word before a Link), and it must carry the Literature-note rules (one note per
# passage, a 60-character title, fluent prose in the author's order, （解释）, src line ranges, the
# direct reading thread, the anchor layout, the five style rules) plus the double SELF-CHECK (the Card
# check and the coverage check). The Link pass reads the same Link rule for the sentence it writes.
# The live archive is never touched and no Card is written.
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Prompt([string]$name) { ([IO.File]::ReadAllText((Join-Path $repo "scripts\book-digest\$name"))) -replace "`r`n", "`n" }

$digest = Prompt 'dsh-digest-base.zh.md'

# The Idea-card standard is gone from the digest prompt.
foreach ($case in @(
        @{ rule = 'the fixed 6–12 Card count'; text = '6–12' },
        @{ rule = 'the 80–200 character body'; text = '80–200' },
        @{ rule = 'the one-Card-one-idea rule'; text = '一卡一观点' },
        @{ rule = 'the STE skill instruction'; text = '先按该技能写' },
        @{ rule = 'the 40-character sentence rule'; text = '最多 40 字' },
        @{ rule = 'the one-sentence-one-fact rule'; text = '一句只说一件事' },
        @{ rule = 'the no-semicolon rule'; text = '不用分号' },
        @{ rule = 'the passive-voice rule'; text = '主动语态' },
        @{ rule = 'the two-的 noun phrase rule'; text = '最多两个“的”' },
        @{ rule = 'the seven-command budget'; text = '最多 7 条命令' },
        @{ rule = 'the required relation word before a Link'; text = '关系词紧贴' },
        @{ rule = 'the relation-in-the-first-sentence rule'; text = '第一句写出关系' },
        @{ rule = 'the few-and-refined rule'; text = '宁少而精' }
    )) {
    Assert (-not $digest.Contains($case.text)) "the digest prompt drops $($case.rule)"
}

# The Literature-note standard is present.
foreach ($case in @(
        @{ rule = 'one note per passage'; text = '每段写一张笔记' },
        @{ rule = 'the 200–1,200 character body'; text = '200–1,200' },
        @{ rule = 'the 60-character title'; text = '最多 60 字' },
        @{ rule = 'the fluent prose in the author order'; text = '按作者顺序' },
        @{ rule = 'the attributions, hedges, numbers and caveats'; text = '归属' },
        @{ rule = 'an explanation the source does not give'; text = '（解释）' },
        @{ rule = 'the line-range src'; text = 'src' },
        @{ rule = 'the reading thread as Direct successors'; text = '直接后继' },
        @{ rule = 'an example or digression as a Side successor'; text = '侧后继' },
        @{ rule = 'the anchor summary and section index'; text = '小节索引' },
        @{ rule = 'the anchor position paragraph'; text = '在全书中的位置' },
        @{ rule = 'the book term table'; text = 'terms.md' },
        @{ rule = 'connectives instead of short-sentence lists'; text = '连接词' },
        @{ rule = 'numbered lists only for three or more parallel steps'; text = '三个以上' },
        @{ rule = 'formulas, quotations and figure lines verbatim'; text = '逐字' },
        @{ rule = 'the STE skill is not used'; text = '不使用' }
    )) {
    Assert ($digest.Contains($case.text)) "the digest prompt states $($case.rule)"
}

# The double SELF-CHECK: one Card check and one coverage check, both read by the writer.
Assert ($digest.Contains('SELF-CHECK')) 'the digest prompt names its SELF-CHECK lines'
Assert ($digest.Contains('coverage') -and $digest.Contains('faults: 0') -and $digest.Contains('最多运行三次')) 'the digest prompt fixes both checks to faults: 0, at most three runs'
Assert ($digest.Contains('C1') -and $digest.Contains('C2')) 'the digest prompt names the coverage fault codes the writer reads'
Assert ($digest.Contains('`src`') -and $digest.Contains('行号')) 'the digest prompt requires a line-range src on every call'

# The draft is the write path: the job writes one JSON draft and calls no zt tool of its own.
Assert ($digest.Contains('JSON 草稿') -and $digest.Contains('calls') -and $digest.Contains('@@N')) 'the digest prompt asks for the JSON draft with call proposals'
Assert (-not $digest.Contains('zt-digest call -')) 'the digest prompt no longer teaches a direct zt call'

# The Link pass reads the same sentence rule: the connection is stated in the sentence the Link
# serves. It keeps the archive's Relation-kind mechanism, which decides whether a gap is closed.
$link = Prompt 'dsh-link-instructions.zh.md'
Assert ($link.Contains('Link 写在使用它意思的那句话里')) 'the link prompt writes the Link in the sentence it serves'
Assert (-not $link.Contains('关系词紧贴在 Link 前')) 'the link prompt drops the required relation word before the Link'
Assert ($link.Contains('句子说出') -or $link.Contains('这句话说出')) 'the link prompt requires the sentence to state the connection'

if ($failed) { Write-Output "$failed check(s) failed"; exit 1 }
Write-Output 'digest prompt: ok'
