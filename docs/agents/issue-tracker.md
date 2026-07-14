# Issue tracker: GitHub

Issues and PRDs for this repository live in GitHub Issues at `liudesheng9/zet`. Use the `gh` CLI for all tracker operations.

## Conventions

- Create an issue with `gh issue create` and a complete Markdown body.
- Read an issue and its comments with `gh issue view <number> --comments`.
- List work with `gh issue list` and request JSON fields when machine-readable output is useful.
- Update labels with `gh issue edit <number> --add-label` or `--remove-label`.
- Add implementation or verification evidence with `gh issue comment`.
- Close completed work with `gh issue close` only after its acceptance criteria are verified.
- Infer the repository from the local `origin` remote when commands run inside this clone.

## Publishing

When an engineering skill says to publish a PRD, plan, or issue to the tracker, create a GitHub issue in `liudesheng9/zet`.
