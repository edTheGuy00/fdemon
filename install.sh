#!/bin/bash
# install.sh — Install or update the fdemon binary from GitHub Releases.
#
# Usage:
#   curl -fsSL https://fdemon.dev/install.sh | bash
#   curl -fsSL https://fdemon.dev/install.sh | bash -s -- --version 0.2.0
#   FDEMON_INSTALL_DIR=/usr/local/bin curl -fsSL ... | bash
#
# Options:
#   --version X.Y.Z   Install a specific version (default: latest)
#   --help            Print this help message

set -euo pipefail

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

REPO="edTheGuy00/fdemon"
BINARY_NAME="fdemon"
DEFAULT_INSTALL_DIR="${FDEMON_INSTALL_DIR:-$HOME/.local/bin}"
GITHUB_API="https://api.github.com/repos/${REPO}/releases/latest"
GITHUB_LATEST="https://github.com/${REPO}/releases/latest"
GITHUB_RELEASES="https://github.com/${REPO}/releases/download"
USER_AGENT="fdemon-install (+https://github.com/${REPO})"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

print_usage() {
    cat <<EOF
fdemon installer

USAGE:
    install.sh [OPTIONS]

OPTIONS:
    --version X.Y.Z   Install a specific version (default: latest release)
    --help            Print this help and exit

ENVIRONMENT:
    FDEMON_INSTALL_DIR   Override the install directory (default: \$HOME/.local/bin)

EXAMPLES:
    # Install the latest release
    curl -fsSL https://fdemon.dev/install.sh | bash

    # Install a specific version
    curl -fsSL https://fdemon.dev/install.sh | bash -s -- --version 0.2.0

    # Install to a custom directory
    FDEMON_INSTALL_DIR=/usr/local/bin curl -fsSL https://fdemon.dev/install.sh | bash
EOF
}

info() {
    printf '  \033[1;36minfo\033[0m  %s\n' "$*"
}

success() {
    printf '  \033[1;32mok\033[0m    %s\n' "$*"
}

warn() {
    printf '  \033[1;33mwarn\033[0m  %s\n' "$*" >&2
}

error() {
    printf '  \033[1;31merror\033[0m %s\n' "$*" >&2
    exit 1
}

# ---------------------------------------------------------------------------
# Temp dir + cleanup
# ---------------------------------------------------------------------------

TMPDIR_WORK=""

cleanup() {
    if [ -n "$TMPDIR_WORK" ] && [ -d "$TMPDIR_WORK" ]; then
        rm -rf "$TMPDIR_WORK"
    fi
}

trap cleanup EXIT

# ---------------------------------------------------------------------------
# Platform detection
# ---------------------------------------------------------------------------

detect_os() {
    local uname_s
    uname_s="$(uname -s)"
    case "$uname_s" in
        Darwin) echo "apple-darwin" ;;
        Linux)  echo "unknown-linux-gnu" ;;
        MINGW*|MSYS*|CYGWIN*)
            error "Windows is not supported by this script. Download the .zip directly from: https://github.com/${REPO}/releases"
            ;;
        *)
            error "Unsupported operating system: ${uname_s}"
            ;;
    esac
}

detect_arch() {
    local uname_m
    uname_m="$(uname -m)"
    case "$uname_m" in
        x86_64|amd64) echo "x86_64" ;;
        arm64|aarch64) echo "aarch64" ;;
        *)
            error "Unsupported architecture: ${uname_m}"
            ;;
    esac
}

# ---------------------------------------------------------------------------
# Dependency checks
# ---------------------------------------------------------------------------

require_cmd() {
    local cmd="$1"
    if ! command -v "$cmd" >/dev/null 2>&1; then
        error "Required command not found: ${cmd}. Please install it and try again."
    fi
}

# ---------------------------------------------------------------------------
# Resolve latest version
# ---------------------------------------------------------------------------

# Primary: follow the github.com releases/latest redirect.
#
# https://github.com/<repo>/releases/latest answers with a 302 whose final
# location is .../releases/tag/vX.Y.Z. Unlike api.github.com (60 requests/hour
# per IP for unauthenticated callers), this endpoint is NOT subject to the
# REST API rate limit — shared-NAT/VPN/mobile users were hitting 403s there
# (see https://github.com/edTheGuy00/fdemon/issues/41).
get_latest_version_redirect() {
    local final_url
    final_url="$(curl -fsSLI -A "$USER_AGENT" --retry 2 --retry-delay 1 \
        -o /dev/null -w '%{url_effective}' "$GITHUB_LATEST" 2>/dev/null)" || return 1
    case "$final_url" in
        */releases/tag/v*)
            printf '%s' "${final_url##*/releases/tag/v}"
            ;;
        *)
            # No redirect happened (e.g. no releases yet) — treat as failure.
            return 1
            ;;
    esac
}

# Fallback: GitHub REST API (rate-limited; kept for resilience if the
# redirect behavior ever changes).
get_latest_version_api() {
    curl -fsSL -A "$USER_AGENT" --retry 2 --retry-delay 1 "$GITHUB_API" 2>/dev/null \
        | grep '"tag_name"' \
        | sed -E 's/.*"v([^"]+)".*/\1/'
}

get_latest_version() {
    local version
    version="$(get_latest_version_redirect)" || version=""
    if [ -z "$version" ]; then
        version="$(get_latest_version_api)" || version=""
    fi
    printf '%s' "$version"
}

# ---------------------------------------------------------------------------
# Installed version check
# ---------------------------------------------------------------------------

get_installed_version() {
    if command -v "$BINARY_NAME" >/dev/null 2>&1; then
        "$BINARY_NAME" --version 2>/dev/null | awk '{print $2}' || true
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    local requested_version=""

    # Parse arguments
    while [ $# -gt 0 ]; do
        case "$1" in
            --version)
                shift
                if [ $# -eq 0 ]; then
                    error "--version requires an argument (e.g. --version 0.2.0)"
                fi
                requested_version="$1"
                shift
                ;;
            --help|-h)
                print_usage
                exit 0
                ;;
            *)
                error "Unknown argument: $1. Run with --help for usage."
                ;;
        esac
    done

    # Reject Windows early (uname may still exist under MSYS/Git Bash)
    local os_raw
    os_raw="$(uname -s)"
    case "$os_raw" in
        MINGW*|MSYS*|CYGWIN*)
            error "Windows is not supported by this script. Download the .zip directly from: https://github.com/${REPO}/releases"
            ;;
    esac

    # Check required tools
    require_cmd curl
    require_cmd tar

    # Resolve OS and arch
    local os arch target
    os="$(detect_os)"
    arch="$(detect_arch)"
    target="${arch}-${os}"

    info "Detected platform: ${target}"

    # Resolve target version
    local version
    if [ -n "$requested_version" ]; then
        version="$requested_version"
        info "Requested version: ${version}"
    else
        info "Resolving latest version from GitHub..."
        version="$(get_latest_version)"
        if [ -z "$version" ]; then
            warn "Could not resolve the latest version. Both lookups failed:"
            warn "  1. ${GITHUB_LATEST} (redirect)"
            warn "  2. ${GITHUB_API} (REST API — rate-limited to 60 req/hr per IP)"
            error "Failed to resolve latest version. Pass one explicitly, e.g. --version 0.5.0 — available versions: https://github.com/${REPO}/releases"
        fi
        info "Latest version: ${version}"
    fi

    # Check installed version
    local installed_version
    installed_version="$(get_installed_version)"

    if [ -n "$installed_version" ] && [ "$installed_version" = "$version" ]; then
        success "${BINARY_NAME} ${version} is already installed and up to date."
        exit 0
    fi

    if [ -n "$installed_version" ]; then
        info "Upgrading ${BINARY_NAME} from ${installed_version} to ${version}"
    else
        info "Installing ${BINARY_NAME} ${version}"
    fi

    # Construct download URL
    local archive_name="fdemon-v${version}-${target}.tar.gz"
    local url="${GITHUB_RELEASES}/v${version}/${archive_name}"

    info "Downloading: ${url}"

    # Create temp directory
    TMPDIR_WORK="$(mktemp -d)"
    local archive_path="${TMPDIR_WORK}/${archive_name}"

    # Download archive
    if ! curl -fsSL -A "$USER_AGENT" --retry 2 --retry-delay 1 --output "$archive_path" "$url"; then
        error "Download failed. Check that version ${version} exists for target ${target}: https://github.com/${REPO}/releases"
    fi

    # Verify download is non-empty
    if [ ! -s "$archive_path" ]; then
        error "Downloaded archive is empty. The release artifact may not exist for target ${target}."
    fi

    # Extract binary
    info "Extracting archive..."
    tar -xzf "$archive_path" -C "$TMPDIR_WORK"

    local extracted_binary="${TMPDIR_WORK}/${BINARY_NAME}"
    if [ ! -f "$extracted_binary" ]; then
        # Some archives may nest the binary under a directory
        extracted_binary="$(find "$TMPDIR_WORK" -type f -name "$BINARY_NAME" | head -1)"
        if [ -z "$extracted_binary" ]; then
            error "Binary '${BINARY_NAME}' not found in archive."
        fi
    fi

    # Ensure install directory exists
    local install_dir="${DEFAULT_INSTALL_DIR}"
    if [ ! -d "$install_dir" ]; then
        info "Creating install directory: ${install_dir}"
        mkdir -p "$install_dir"
    fi

    # Install binary with correct permissions
    local install_path="${install_dir}/${BINARY_NAME}"
    install -m755 "$extracted_binary" "$install_path"

    # Verify the installed binary works
    info "Verifying installation..."
    local verified_version
    verified_version="$("$install_path" --version 2>/dev/null | awk '{print $2}' || true)"
    if [ -z "$verified_version" ]; then
        error "Installed binary did not run successfully. Try running: ${install_path} --version"
    fi

    success "Installed ${BINARY_NAME} ${verified_version} to ${install_path}"

    # PATH hint
    if ! printf '%s' "$PATH" | tr ':' '\n' | grep -qx "$install_dir"; then
        echo ""
        warn "${install_dir} is not in your PATH."
        echo ""
        echo "    Add fdemon to your PATH by adding this to your shell profile"
        echo "    (~/.bashrc, ~/.zshrc, or ~/.profile):"
        echo ""
        echo "      export PATH=\"${install_dir}:\$PATH\""
        echo ""
        echo "    Then restart your shell or run:"
        echo "      source ~/.bashrc   # (or ~/.zshrc)"
        echo ""
    fi
}

main "$@"
