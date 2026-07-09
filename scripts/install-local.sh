#!/usr/bin/env bash
# Build cortex from this repo and install to ~/.local/bin (or CORTEX_INSTALL_DIR).
# Use after local development so `cortex` works from any directory.
#
#   ./scripts/install-local.sh
#   make install
#   just install
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_DIR="${CORTEX_INSTALL_DIR:-${HOME}/.local/bin}"
BIN_NAME="cortex"
PROFILE="${CORTEX_BUILD_PROFILE:-release}"

info() { printf '==> %s\n' "$*"; }

cd "$ROOT"

if [[ "$PROFILE" == "release" ]]; then
  info "building release cortex-cli…"
  cargo build --release -p cortex-cli
  SRC="$ROOT/target/release/$BIN_NAME"
else
  info "building debug cortex-cli…"
  cargo build -p cortex-cli
  SRC="$ROOT/target/debug/$BIN_NAME"
fi

[[ -x "$SRC" ]] || { echo "error: binary not found at $SRC" >&2; exit 1; }

mkdir -p "$INSTALL_DIR"
install -m 755 "$SRC" "$INSTALL_DIR/$BIN_NAME"
info "installed $INSTALL_DIR/$BIN_NAME"

case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    info "note: $INSTALL_DIR is not on PATH — add:"
    printf '    export PATH="%s:$PATH"\n' "$INSTALL_DIR"
    ;;
esac

# Quiet home bootstrap so doctor/models work out of the box.
if [[ -x "$INSTALL_DIR/$BIN_NAME" ]]; then
  "$INSTALL_DIR/$BIN_NAME" setup >/dev/null 2>&1 || true
  info "version: $("$INSTALL_DIR/$BIN_NAME" --version 2>/dev/null || true)"
fi

info "done. try: cortex doctor"
