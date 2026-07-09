#!/usr/bin/env bash
# Install Cortex CLI from GitHub Releases into ~/.local/bin (no sudo).
#
# Platforms: Linux and macOS only (no Windows).
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/CallumBicknell/cortex/main/scripts/install.sh | sh
#
# Env:
#   CORTEX_VERSION      Pin a release tag (e.g. v0.2.0). Default: latest.
#   CORTEX_INSTALL_DIR  Install directory (default: ~/.local/bin).
#   CORTEX_REPO         owner/name (default: CallumBicknell/cortex).
#
# Asset names match .github/workflows/release.yml:
#   cortex-${TAG}-${target}.tar.gz  (unix)
set -euo pipefail

REPO="${CORTEX_REPO:-CallumBicknell/cortex}"
INSTALL_DIR="${CORTEX_INSTALL_DIR:-${HOME}/.local/bin}"
VERSION="${CORTEX_VERSION:-}"

info() { printf '==> %s\n' "$*"; }
err()  { printf 'error: %s\n' "$*" >&2; exit 1; }

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || err "missing required command: $1"
}

detect_target() {
  local os arch
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"
  case "$os" in
    linux)  os_part="unknown-linux-gnu" ;;
    darwin) os_part="apple-darwin" ;;
    *) err "unsupported OS: $os (use cargo install --git https://github.com/${REPO} for now)" ;;
  esac
  case "$arch" in
    x86_64|amd64) arch_part="x86_64" ;;
    aarch64|arm64) arch_part="aarch64" ;;
    *) err "unsupported arch: $arch" ;;
  esac
  echo "${arch_part}-${os_part}"
}

download() {
  local url="$1" dest="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$dest"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$dest" "$url"
  else
    err "need curl or wget"
  fi
}

resolve_tag() {
  if [[ -n "$VERSION" ]]; then
    echo "$VERSION"
    return
  fi
  need_cmd curl
  # Prefer GitHub API; fall back to redirect Location on releases/latest.
  local tag
  tag="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' \
    | head -n1)"
  if [[ -z "$tag" ]]; then
    tag="$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
      "https://github.com/${REPO}/releases/latest" \
      | sed 's#.*/##')"
  fi
  [[ -n "$tag" ]] || err "could not resolve latest release tag for ${REPO}"
  echo "$tag"
}

main() {
  need_cmd tar
  need_cmd uname
  local target tag asset url tmp stage bin
  target="$(detect_target)"
  tag="$(resolve_tag)"
  # Normalize: allow CORTEX_VERSION=0.2.0 or v0.2.0
  case "$tag" in
    v*) ;;
    *) tag="v${tag}" ;;
  esac

  asset="cortex-${tag}-${target}.tar.gz"
  url="https://github.com/${REPO}/releases/download/${tag}/${asset}"
  info "installing Cortex ${tag} (${target})"
  info "from ${url}"

  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  if ! download "$url" "${tmp}/${asset}"; then
    err "download failed — is ${tag} published? Try: cargo install --git https://github.com/${REPO} --locked --bin cortex"
  fi

  tar -xzf "${tmp}/${asset}" -C "$tmp"
  # Archive layout: cortex-${tag}-${target}/cortex
  stage="${tmp}/cortex-${tag}-${target}"
  if [[ -x "${stage}/cortex" ]]; then
    bin="${stage}/cortex"
  else
    bin="$(find "$tmp" -type f -name cortex | head -n1)"
  fi
  [[ -n "$bin" && -f "$bin" ]] || err "cortex binary not found in archive"

  mkdir -p "$INSTALL_DIR"
  install -m 755 "$bin" "${INSTALL_DIR}/cortex"
  info "installed ${INSTALL_DIR}/cortex"

  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
      info "add to PATH (zsh/bash):"
      printf '    export PATH="%s:$PATH"\n' "$INSTALL_DIR"
      info "add that line to ~/.bashrc or ~/.zshrc"
      ;;
  esac

  if "${INSTALL_DIR}/cortex" setup >/dev/null 2>&1; then
    info "ran: cortex setup"
  else
    info "run after PATH is set: cortex setup"
  fi

  "${INSTALL_DIR}/cortex" --version 2>/dev/null || true
  info "done. Try: cortex doctor"
}

main "$@"
