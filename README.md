# Key-Bitcher

CLI tool that manages AI API keys and environment variables for local and
cloud-based coding environments. It syncs secrets from a private
S3-compatible bucket (Backblaze B2) into your `.env`, imports them into the
[Wave](https://waveterm.dev) secret store, validates/benchmarks the providers,
and self-heals expired keys.

> [!WARNING]
> This tool moves real API keys around. Read
> [`docs/security.md`](https://merlin-tribukait.github.io/KEY-BITCHER/#/security)
> before using it on a shared machine.

## Why

Developers accumulate dozens of AI-provider keys (OpenAI, Anthropic, Google,
Mistral, OpenRouter, NVIDIA, ...). They end up duplicated across laptops,
`.env` files, terminals, and cloud agents, and they expire without notice.
Key-Bitcher gives you **one source of truth** (a private bucket) and pushes
validated keys everywhere your tools need them:

- **`sync-secrets`** — pull the canonical key set from the bucket into `.env`.
- **`wave-import`** — import the keys into the Wave secret store.
- **`benchmark`** — verify each provider still accepts your keys (detects 401s).
- **`import-md`** — parse a `working.md` handout into structured secrets and
  upload them, so pasting a chat summary becomes a reproducible, audited action.
- **`validate`** — dry-run checks that the bucket or `.env` matches expectations.
- **`rust-setup`** — one-shot environment bootstrap (installs the toolchain,
  cargo tools, and the shell integrations).

## Installation

You need a Rust toolchain (1.70+).

```powershell
# PowerShell (recommended)
.\build.ps1 -Profile release -Test
# or, from source:
cargo build --release
```

The binary is `target\release\key-bitcher.exe`. The `integrations/` folder
contains shell wrappers (`key-bitcher.cmd`, `key-bitcher.ps1`,
`key-bitcher.sh`) and setup scripts (`install-path.ps1`, `env-loader.ps1`)
that put the tool and its env file on your PATH and load `.env` automatically
per-terminal.

## Quick start

1. Copy `.env.example` to `.env` and fill in at least:
   - `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` / `AWS_REGION` (bucket access)
   - the AI provider keys you want to sync
2. Point the tool at the same bucket as your team:

   ```toml
   # key-bitcher.toml
   [s3]
   bucket = "wave-secrets-bucket"
   region = "eu-central-003"
   object_key = "secrets/ai-keys.json"
   ```

3. Pull your keys and check they work:

   ```powershell
   key-bitcher sync-secrets        # bucket -> .env
   key-bitcher benchmark           # ping each provider, surface 401s
   ```

4. Run with **no arguments** for the full flow:
   `sync-secrets` → `wave-import` → `benchmark`.

## Usage

```
key-bitcher [OPTIONS] [COMMAND]
```

### Global flags

| Flag | Description |
| --- | --- |
| `--config <PATH>` | Config file (default `./key-bitcher.toml`) |
| `--auto-sync` | Enable the automatic flow flag for sync commands |
| `--debug`, `-d` | Verbose logging |
| `--log-file <PATH>` | Log file (default `key-bitcher.log`, rotated) |

### Commands

| Command | What it does | When you need it |
| --- | --- | --- |
| *(none)* | Auto flow: `sync-secrets` → `wave-import` → `benchmark` | Daily refresh |
| `sync-secrets` | Fetch secrets from S3 into `.env` (`--dry-run` to preview, `--print-exports` for shell `export`s) | After bucket changes, new machine |
| `benchmark` | Ping configured models, report per-key status | Check for expired keys |
| `wave-import` | Import `.env` keys into the Wave secret store | Feeding Wave-terminal agents |
| `import-md --file X` | Parse a `working.md` handout into secrets, upload in-memory | Reproducing a chat handout |
| `upload --file X` | Upload a secrets JSON to the bucket | Publishing a new canonical key set |
| `list` | List secret *names* in the bucket (values never shown) | Safe inventory / shared-screen checks |
| `secure [--paths ...]` | Restrict `.env` / `secrets.json` to the current user | Hardening a shared machine |
| `validate --source env\|s3` | Dry-run checks on `.env` or the bucket | CI / pre-flight |
| `wave-test` | Self-test the Wave secret-store integration | Debugging `wave-import` |
| `s3-versioning` | Show/verify bucket versioning status | Backup safety check |
| `s3-test` | Connectivity test against the bucket | Debugging S3 setup |
| `create-bucket` | Create the bucket if it does not exist | First-time setup |
| `rust-setup` | Install toolchain + cargo tools + integrations | One-shot bootstrap |

See [`docs/commands.md`](https://merlin-tribukait.github.io/KEY-BITCHER/#/commands)
for full reference and examples.

## Configuration

`key-bitcher.toml` sections: `[s3]`, `[secrets]`, `[wave]`, `[logging]`,
`[benchmark]`, `[sections]`. The `[sections]` table lets you map custom
`working.md` headings to env-var names. For backward compatibility the legacy
`plugin_config.toml` is still read if `key-bitcher.toml` is absent. See
[`docs/configuration.md`](https://merlin-tribukait.github.io/KEY-BITCHER/#/configuration).

## Project layout

```
├── src/
│   ├── main.rs        # CLI entry point
│   ├── config.rs      # key-bitcher.toml + LoggingConfig + section map
│   ├── secrets.rs     # secrets.json model, serialization, key logic
│   ├── workingmd.rs   # working.md parser (section -> keys)
│   ├── s3.rs          # S3/B2 client (rustls), upload/download, versioning
│   ├── benchmark.rs   # provider health checks
│   ├── rust_setup.rs  # environment bootstrap
│   └── logging.rs     # rotating file logger
├── integrations/      # shell wrappers + IDE integration files
├── docs/              # GitHub Pages documentation (static site)
└── key-bitcher.toml   # runtime config
```

## Development

```powershell
.\build.ps1 -Profile release -Clippy -Test -Fmt
```

CI runs `cargo fmt --check`, `cargo clippy --release -- -D warnings`,
`cargo test`, and a [gitleaks](https://github.com/gitleaks/gitleaks) secret
scan on every push. Dependabot keeps Cargo and GitHub Actions updated.

## License

[MIT](LICENSE)
