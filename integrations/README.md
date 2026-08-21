# Integrations

Make Key-Bitcher usable from your favorite IDE, console, or system shell.

The binary is built with `cargo build --release` and ends up at
`target/release/key-bitcher` (`.exe` on Windows). The integrations assume it
exists and is run from the project root (where `key-bitcher.toml` and `.env`
live).

## Subcommands

| Command                | What it does                                          |
| ---------------------- | ----------------------------------------------------- |
| *(no argument)*        | auto: sync-secrets + wave-import + benchmark           |
| `sync-secrets`         | fetch secrets from S3 and update `.env`               |
| `benchmark`            | ping the configured models with the synced keys       |
| `wave-import`          | import `.env` keys into the Wave secret store         |
| `import-md --file X`   | parse a working.md file, upload to S3 (in memory)     |
| `upload --file X`      | upload a secrets JSON file to S3                      |
| `list`                 | list secret names in the bucket (values never shown)  |
| `secure`               | restrict `.env` / `secrets.json` to the current user  |
| `validate --source env\|s3` | dry-run checks on `.env` or the bucket            |
| `s3-test`, `create-bucket`, `rust-setup` | diagnostics / setup          |

Global flags: `--debug` (verbose logging), `--log-file` (append-only log, never
truncated between runs), `--config`.

## Visual Studio Code

- `.vscode/tasks.json` - tasks: `key-bitcher: build`, `clippy`, `test`, `fmt`,
  `sync-secrets`, `benchmark`, `wave-import`, `auto`.
- `.vscode/extensions.json` - recommended Rust extensions.

Run via `Ctrl+Shift+P > Tasks: Run Task > key-bitcher: sync-secrets`.

## Visual Studio 2022+

- `.vs/tasks.vs.json` - tasks shown under `View > Other Windows > Task Runner
  Explorer` when the folder is opened (Folder View). Build/test/clippy run
  through `cargo`; the plugin commands run the release binary directly.

## JetBrains (RustRover / CLion / IntelliJ + Rust plugin)

- `.idea/runConfigurations/*.run.xml` - run configurations:
  `key-bitcher: build`, `test`, `clippy`, `sync-secrets`, `benchmark`,
  `wave-import`, `auto`. They use `cargo run --release` so the binary is built
  automatically. Pick one from the run configuration dropdown.

## Consoles / terminals

- `integrations/terminal/key-bitcher.ps1` - PowerShell wrapper.
  Add to your profile: `. C:\path\to\key-bitcher\integrations\terminal\key-bitcher.ps1`
  Then: `key-bitcher`, `key-bitcher sync-secrets`, `Export-KeyBitcherEnv` (load `.env` into
  the session), alias `kb`.
- `integrations/terminal/key-bitcher.sh` - bash/zsh wrapper.
  Add to `~/.bashrc` / `~/.zshrc`: `. /path/to/key-bitcher/integrations/terminal/key-bitcher.sh`
  Then: `key-bitcher`, `kb-sync`, `kb-bench`, `kb-env`.
- `integrations/terminal/key-bitcher.cmd` - cmd.exe / Windows Terminal shim.
  Put it on your PATH or run `key-bitcher sync-secrets` from anywhere.

## System setup

- `integrations/system/install-path.ps1` - copies the wrappers into a `bin\`
  folder and adds it to the user PATH, so `key-bitcher` works from any new console.
- `integrations/system/env-loader.ps1` - loads the project `.env` into the
  current process (or, with `-Persist`, at user scope).

All wrappers locate the project root by walking up until they find
`Cargo.toml`, so they keep working even if copied into `bin\`.
