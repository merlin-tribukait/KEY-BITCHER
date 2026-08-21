# Key-Bitcher project conventions

This file guides AI agents working in this repository. The `.opencode/plugins/auto-docs.ts`
plugin spawns a background "docs keeper" sub-agent before git write commands
(commit/push/merge/rebase/...). That sub-agent follows these rules:

## House style

- Keep a Changelog format in `CHANGELOG.md`; unreleased, user-visible changes
  go under `## [Unreleased]`, one concise bullet per change.
- Markdown documents stay plain and wrapped at ~80 columns.
- The GitHub Pages site in `docs/` is a static site (no Jekyll build). Pages
  are `index.html` plus one `*.html` per topic (`commands.html`,
  `config.html`, `security.html`, ...). If `docs/_build.py` exists with its
  template inputs, use it; otherwise edit the HTML directly.
- Docs HTML uses the `.docs-content` reveal classes (`html.js` + `.revealed`)
  — keep reveal-prefixed rules intact in `docs.css`; bare `.revealed` rules
  lose the specificity fight and hide content.
- The generated banner links (`🪙 KEY-GOBLIN HOARD ↗`, `📚 ROADMAP`) appear on
  every page via the template / shared header markup.

## Doc-keeper rules

- Update README.md, CHANGELOG.md, SECURITY.md, `.env.example`, and `docs/`
  when the code changes (commands, flags, config keys, env vars, behavior).
- Never commit, push, amend, reset, or destroy anything. Stage your changes
  with `git add <file>` so the pending commit picks them up.
- Never read or print secrets (`.env`, `example_s3_secrets.json` contents).
- Keep edits minimal, in existing style, no wholesale reformatting.
- Work without asking questions or waiting for approval; append a one-line
  `[auto-docs]` summary to `ai-env-plugin.log` when done.
