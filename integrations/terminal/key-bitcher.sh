# key-bitcher - bash/zsh wrapper for Key-Bitcher.
#
# Install (add to ~/.bashrc or ~/.zshrc):
#   . /path/to/key-bitcher/integrations/terminal/key-bitcher.sh
#
# Commands:
#   key-bitcher              run the full auto flow (sync + wave import + benchmark)
#   key-bitcher sync-secrets
#   key-bitcher benchmark
#   key-bitcher wave-import
#   kb-sync                  short alias for sync-secrets
#   kb-bench                 short alias for benchmark
#   kb-auto                  short alias for the auto flow
#   kb-env                   load .env values into the current shell

_key_bitcher_find_root() {
  local dir
  dir="$(dirname "${BASH_SOURCE[0]:-${(%):-%x}}")"
  while [ -n "$dir" ] && [ ! -f "$dir/Cargo.toml" ]; do
    dir="$(dirname "$dir")"
  done
  [ -n "$dir" ] && printf '%s' "$dir"
}

KEY_BITCHER_ROOT="$(_key_bitcher_find_root)"
KEY_BITCHER_BIN="${KEY_BITCHER_ROOT}/target/release/key-bitcher"

key-bitcher() {
  if [ ! -x "$KEY_BITCHER_BIN" ]; then
    echo "key-bitcher binary not found at $KEY_BITCHER_BIN. Run 'cargo build --release' in $KEY_BITCHER_ROOT" >&2
    return 1
  fi
  (cd "$KEY_BITCHER_ROOT" && "$KEY_BITCHER_BIN" "$@")
}

kb-auto() { key-bitcher; }
kb-sync() { key-bitcher sync-secrets "$@"; }
kb-bench() { key-bitcher benchmark "$@"; }

kb-env() {
  local env_file="${KEY_BITCHER_ROOT}/.env"
  if [ ! -f "$env_file" ]; then
    echo ".env not found at $env_file" >&2
    return 1
  fi
  set -a
  # shellcheck disable=SC1090
  source "$env_file"
  set +a
}
