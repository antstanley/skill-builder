#!/bin/sh
# install.sh - Install skill-builder from GitHub releases
# Usage: curl -fsSL https://raw.githubusercontent.com/antstanley/skill-builder/main/install.sh | sh
#
# Environment variables:
#   SKILL_BUILDER_INSTALL_DIR - Override install directory
#   SKILL_BUILDER_VERSION     - Install a specific version (e.g. "v1.0.0")

set -eu

REPO="antstanley/skill-builder"
BINARY_NAME="sb"

detect_os() {
    os=$(uname -s)
    case "$os" in
        Linux)   echo "linux" ;;
        Darwin)  echo "macos" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *)
            echo "Error: unsupported operating system: $os" >&2
            return 1
            ;;
    esac
}

detect_arch() {
    arch=$(uname -m)
    case "$arch" in
        x86_64|amd64)    echo "x86_64" ;;
        aarch64|arm64)   echo "aarch64" ;;
        *)
            echo "Error: unsupported architecture: $arch" >&2
            return 1
            ;;
    esac
}

get_target() {
    _os="$1"
    _arch="$2"

    case "${_os}_${_arch}" in
        linux_x86_64)   echo "x86_64-linux-gnu" ;;
        linux_aarch64)  echo "aarch64-linux-gnu" ;;
        macos_x86_64)   echo "x86_64-apple-darwin" ;;
        macos_aarch64)  echo "aarch64-apple-darwin" ;;
        windows_x86_64) echo "x86_64-pc-windows-msvc" ;;
        *)
            echo "Error: unsupported platform: ${_os} ${_arch}" >&2
            return 1
            ;;
    esac
}

get_default_install_dir() {
    _os="$1"
    case "$_os" in
        linux)   echo "$HOME/.local/bin" ;;
        macos)   echo "/usr/local/bin" ;;
        windows) echo "$HOME/.local/bin" ;;
        *)       echo "$HOME/.local/bin" ;;
    esac
}

get_archive_ext() {
    _os="$1"
    case "$_os" in
        windows) echo "zip" ;;
        *)       echo "tar.gz" ;;
    esac
}

resolve_version() {
    if [ -n "${SKILL_BUILDER_VERSION:-}" ]; then
        echo "$SKILL_BUILDER_VERSION"
        return
    fi

    # Query GitHub API for latest release
    if command -v curl >/dev/null 2>&1; then
        tag=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | \
            grep '"tag_name"' | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/')
    elif command -v wget >/dev/null 2>&1; then
        tag=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | \
            grep '"tag_name"' | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/')
    else
        echo "Error: curl or wget is required" >&2
        return 1
    fi

    if [ -z "$tag" ]; then
        echo "Error: could not determine latest version" >&2
        return 1
    fi

    echo "$tag"
}

download_and_install() {
    _version="$1"
    _target="$2"
    _ext="$3"
    _install_dir="$4"

    _archive_name="skill-builder-${_target}.${_ext}"
    _url="https://github.com/${REPO}/releases/download/${_version}/${_archive_name}"

    echo "Downloading ${BINARY_NAME} ${_version} for ${_target}..."
    echo "  URL: ${_url}"

    # Create temp directory
    _tmpdir=$(mktemp -d)
    trap 'rm -rf "$_tmpdir"' EXIT

    # Download
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$_url" -o "${_tmpdir}/${_archive_name}"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$_url" -O "${_tmpdir}/${_archive_name}"
    else
        echo "Error: curl or wget is required" >&2
        return 1
    fi

    # Extract
    echo "Extracting..."
    case "$_ext" in
        tar.gz)
            tar -xzf "${_tmpdir}/${_archive_name}" -C "$_tmpdir"
            ;;
        zip)
            unzip -q "${_tmpdir}/${_archive_name}" -d "$_tmpdir"
            ;;
    esac

    # Install
    mkdir -p "$_install_dir"
    if [ -f "${_tmpdir}/${BINARY_NAME}" ]; then
        mv "${_tmpdir}/${BINARY_NAME}" "${_install_dir}/${BINARY_NAME}"
    elif [ -f "${_tmpdir}/${BINARY_NAME}.exe" ]; then
        mv "${_tmpdir}/${BINARY_NAME}.exe" "${_install_dir}/${BINARY_NAME}.exe"
    else
        echo "Error: binary not found in archive" >&2
        return 1
    fi

    chmod +x "${_install_dir}/${BINARY_NAME}" 2>/dev/null || true

    echo ""
    echo "Installed ${BINARY_NAME} to ${_install_dir}/${BINARY_NAME}"

    # Check if install dir is in PATH
    case ":${PATH}:" in
        *":${_install_dir}:"*) ;;
        *)
            echo ""
            echo "Note: ${_install_dir} is not in your PATH."
            echo "Add it with:"
            echo "  export PATH=\"${_install_dir}:\$PATH\""
            ;;
    esac
}

main() {
    echo "=== sb (skill-builder) installer ==="
    echo ""

    os=$(detect_os)
    arch=$(detect_arch)
    target=$(get_target "$os" "$arch")
    ext=$(get_archive_ext "$os")
    version=$(resolve_version)
    install_dir="${SKILL_BUILDER_INSTALL_DIR:-$(get_default_install_dir "$os")}"

    echo "  OS:          ${os}"
    echo "  Arch:        ${arch}"
    echo "  Target:      ${target}"
    echo "  Version:     ${version}"
    echo "  Install dir: ${install_dir}"
    echo ""

    download_and_install "$version" "$target" "$ext" "$install_dir"

    echo ""
    echo "Done! Run '${BINARY_NAME} --help' to get started."
}

# Support --test-mode for sourcing functions without running main
case "${1:-}" in
    --test-mode) ;;
    *) main ;;
esac
