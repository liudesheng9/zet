## Agent skills

### Issue tracker

Issues and PRDs are tracked in GitHub Issues for `liudesheng9/zet` using the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the canonical triage label vocabulary configured for this repository. See `docs/agents/triage-labels.md`.

### Domain docs

This is a single-context repository with one root `CONTEXT.md`. See `docs/agents/domain.md`.

## Wiki

The developer wiki is `docs/wiki/`: one doc per functional block, listed in `docs/wiki/README.md`. Read the block doc before you change a block. When you change code, update its block doc in the same commit (project-wiki skill); `node ~/.claude/skills/project-wiki/scripts/wiki.mjs check` lists stale blocks.
