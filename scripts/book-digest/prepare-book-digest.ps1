#Requires -Version 7
<#
.SYNOPSIS
  Prepare a book for the ZT digest workflows: Chinese-only text, section groups, the shared
  DeepSeek instructions, and (with -Apply) the agent Literature tree with one anchor Card per group.

.DESCRIPTION
  - Reads a Chinese bilingual Markdown book (each English block followed by its Chinese
    translation) and keeps the Chinese: a block without CJK characters is dropped when the next
    block is its translation. Formulas, tables, and unpaired blocks are kept.
  - Keeps the book's figures: every image of the Markdown (-FileId, its Scholar Archive file id,
    read from a ZET cache path such as ...\scholar-archive\files\83\... when not given) of at
    least -MinFigureBytes is taken into ZET's artifact store through the bridge (`zt scholar
    keep-figures`), and its image line becomes `![图 2.C.1](artifact:<sha256>)`, which a Card can
    paste to show the figure. The caption lines under it stay; the same figure repeated in the
    next blocks (the bilingual copy) is written once. figures.json lists every figure with its
    labels. Images below the size, and every image under -NoFigures, are dropped.
  - Splits at `#` and `##` headings from -StartHeading up to -EndHeading, merges tiny units into
    the next one, and groups one or two consecutive units of the same chapter per DeepSeek run
    (at most -MaxChars characters per group unless a single unit is larger). Units under a heading
    in -SkipHeadings (comma-separated, e.g. a chapter's '参考文献,习题') are left out up to the next
    other heading.
  - Writes to -Out: book.zh.md, groups/gNN.md, shared-prefix.md (identical for every run, so the
    DeepSeek prompt cache can reuse it), and manifest.json.
  - -Instructions (comma-separated instruction files of this folder, default dsh-instructions.zh.md)
    builds shared-prefix.md: the first file, with each further file inserted before its `## 回复`
    section. The framework digests pass dsh-digest-base.zh.md plus their book type's files
    (docs/zt-digest-framework.md). -NoEventCandidates leaves the event candidates out of the group
    files (events.json still lists them, for the event layer).
  - -Apply creates, through `zt mcp` with agent Access, the Literature Card -CitationKey from
    -BibTeX and the anchor line <key>/1 .. <key>/N, one per group. It refuses if the tree exists.
    The Literature Card's body links the Work's PDF and Markdown documents with Document links
    (`[原书 PDF](scholar:<file_id>)`), which the GUI opens.
  - -Book <Work id|title> resolves the book through the read-only Scholar Archive bridge instead
    of taking -Source, -BibTeX and -BookTitle: the Work (a Work id resolves exactly; a title only
    on one exact match), its Chinese/bilingual Markdown from the saved file metadata (a bilingual
    or `chinese_bilingual_markdown` file wins; several files of the same preference need -FileId),
    the BibTeX entry from the saved metadata (the Citation key is generated when -CitationKey is
    not given, and a taken key is refused), and the heading range (the proposal starts after the
    front matter, ends at the back matter, and skips exercises and references). The resolved
    source path, hash, key and range, with every excluded section, are written to
    `<Out>\preparation.json` and recorded in `manifest.json` as `book_resolution` before any group
    work. A choice that stays open (an ambiguous title or several suitable files) writes only the
    preparation result, prints it as its last line and exits 3; no Card is written. A given -Source,
    -FileId, -BibTeX or -BookTitle that disagrees with the resolution is an input error, and a
    bridge that cannot supply the metadata or the document fails with the missing fact and accepts
    the explicit arguments as the fallback.
  Prints the manifest as compact JSON.

  Parameters: -Source, -Out, -CitationKey, -BibTeX (required unless -Book resolves them); -Book,
  -BookTitle '', -StartHeading 导论, -EndHeading 参考, -SkipHeadings '', -MaxChars 20000,
  -MaxUnits 3, -MinUnitChars 1000, -FileId 0, -MinFigureBytes 3000, -MaxEventCandidates 12,
  -Instructions dsh-instructions.zh.md, -NoEventCandidates, -NoFigures, -Apply.

  This script forwards every argument to `zt-digest prepare` (tools/zt-digest/src/prepare.rs),
  which does the work in one process with one `zt mcp`; callers should call the tool directly. It
  sets ZT_DIGEST_SCRIPTS to this folder: the tool reads the instruction files (-Instructions and
  dsh-link-instructions.zh.md) from it. ZT_DIGEST names another zt-digest build.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# The folder of the instruction files.
$env:ZT_DIGEST_SCRIPTS = $PSScriptRoot
# `pwsh -File` and `&` hand `-Name:value` over as two arguments (a switch value as a Boolean); the
# tool takes `-Name:value`.
$pass = [Collections.Generic.List[string]]::new()
foreach ($a in $args) {
    $last = if ($pass.Count) { $pass[$pass.Count - 1] } else { '' }
    if ($last -match '^-[A-Za-z]\w*:$') { $pass[$pass.Count - 1] = $last + $(if ($a -is [array]) { $a -join ',' } else { "$a" }) }
    elseif ($a -is [bool] -and $last -match '^-[A-Za-z]\w*$') { $pass[$pass.Count - 1] = "${last}:$a" }
    elseif ($a -is [array]) { $pass.Add($a -join ',') }
    else { $pass.Add("$a") }
}
& $exe prepare @pass
exit $LASTEXITCODE
