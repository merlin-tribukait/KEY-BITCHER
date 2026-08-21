# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/) and this project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed
- Renamed the project from `ai-env-plugin` to **Key-Bitcher** (binary and
  CLI now `key-bitcher`, config file `key-bitcher.toml`, log
  `key-bitcher.log`, notifications `key-bitcher-notifications.md`). The old
  `plugin_config.toml` is still read as a fallback.
- GitHub Pages site is now the static Key-Bitcher documentation page
  (`docs/`, no Jekyll build).

### Added
- Background "docs keeper" hook (`.opencode/plugins/auto-docs.ts`) that spawns
  a sub-agent to update docs before git write commands, plus `AGENTS.md` with
  the house conventions the keeper follows.
- GitHub Pages documentation site (`docs/`) deployed from `main`.
- `SECURITY.md` with vulnerability-reporting and leak-handling guidance.
- Dependabot config for Cargo and GitHub Actions updates.
- gitleaks secret-scanning step in CI.
- `LICENSE` (MIT) and expanded `.env.example` template.

## [0.2.2] - 2026

### Changed
- Replaced AWS legacy TLS stack with a `rustls-ring` hyper-1.x connector
  (fixes the three Dependabot security alerts).
- `working.md` parser now handles `###` subsections, `NAME: value` and
  `NAME=value` forms, and code spans on plain lines.

### Added
- `validate` subcommand with `--source env|s3` dry-run checks.
- `wave-test` self-test for the Wave secret-store integration.
- `s3-versioning` bucket versioning check.
- Wave notifications, concurrent Wave import, log rotation, custom section
  mapping (`[sections]`).

## [0.2.0] - 2026

### Added
- Benchmark console output with per-provider ASCII logos and brand colors.
- `sync-secrets` now also imports secrets into Wave after a successful sync.
- Release workflow: builds release binaries for Ubuntu + Windows and publishes
  a GitHub release.
- Benchmark-before-import: tries all keys per provider, promotes the working
  key, notifies the user.
- IDE, console and system integrations (VS Code, VS, JetBrains, terminals).

## [0.1.0] - 2026

### Added
- Initial commit: S3/Backblaze secret sync to `.env` and Wave, plus CI.
