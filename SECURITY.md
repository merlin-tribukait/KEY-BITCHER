# Security Policy

## Reporting a vulnerability

**Do not open a public issue for a live secret.** Secrets that end up in this
repository are burned the moment they leak.

If you find a bug that could leak or mishandle credentials, please report it
privately:

- Email: merlin_felix_@hotmail.com
- Or open an issue **without pasting any key material** describing the
  vulnerable code path and how it can be triggered.

## Handling leaked secrets

If a key was committed or pasted anywhere public:

1. **Revoke it immediately** at the provider's dashboard — do not "fix the
   commit", the key is already compromised.
2. Remove it from the bucket (`key-bitcher.toml` → `secrets/ai-keys.json`).
3. Open an issue to track rotation (see `.github/ISSUE_TEMPLATE` / the
   [security tracking issue](https://github.com/merlin-tribukait/KEY-BITCHER/issues)).

## Scope

The tool itself stores secrets in two places:

- **`.env`** (local) — plaintext, must be gitignored (it is) and
  permission-restricted (see `docs/security.md`).
- **S3 bucket** (`wave-secrets-bucket`) — the canonical store. Bucket access
  keys are scoped to the bucket; versioning is enabled for recovery.

## Supported versions

Only the latest release is actively patched. Reports against current `main`
are prioritized.

## CI

Every push is scanned with [gitleaks](https://github.com/gitleaks/gitleaks);
a non-zero finding fails the build. If your PR trips it, assume the matched
value is burned and rotate it.
