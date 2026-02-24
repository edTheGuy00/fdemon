## Task: Create Version-Aware Install Script

**Objective**: Create an `install.sh` script that installs or updates the `fdemon` binary from GitHub Releases. The script detects the OS and architecture, checks the currently installed version (if any), and only downloads if a newer version is available.

**Depends on**: 01-version-cli-flag, 04-release-workflow

**Estimated Time**: 3-4 hours

### Scope

- `install.sh` (**NEW**): Install/update script at workspace root

### Details

#### Usage

Fresh install:
```bash
curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash
```

Install specific version:
```bash
curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash -s -- --version 0.2.0
```

Update check (re-running the same command):
```bash
curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash
# If fdemon 0.1.0 is already installed and 0.1.0 is the latest, prints:
#   fdemon 0.1.0 is already installed and up to date.
# If 0.2.0 is available, installs it.
```

Custom install directory:
```bash
FDEMON_INSTALL_DIR=/usr/local/bin curl -fsSL ... | bash
```

#### Script flow

```
1. Parse arguments (--version, --help)
2. Detect OS (uname -s) → Darwin, Linux
3. Detect architecture (uname -m) → x86_64, arm64/aarch64
4. Map to Rust target triple
5. Resolve target version:
   a. If --version specified, use that
   b. Otherwise, query GitHub API for latest release tag
6. Check installed version:
   a. If fdemon is in PATH, run `fdemon --version`
   b. Parse version from output (e.g., "fdemon 0.1.0" → "0.1.0")
   c. If installed version == target version, print "up to date" and exit
7. Download archive from GitHub Releases
8. Extract binary
9. Install to $FDEMON_INSTALL_DIR (default: $HOME/.local/bin)
10. Verify installation (run fdemon --version)
11. Check if install dir is in PATH, show hint if not
12. Cleanup temp directory
```

#### OS and architecture mapping

```bash
case "$(uname -s)" in
    Darwin) os="apple-darwin" ;;
    Linux)  os="unknown-linux-gnu" ;;
    *)      error "Unsupported OS: $(uname -s)" ;;
esac

case "$(uname -m)" in
    x86_64|amd64)    arch="x86_64" ;;
    arm64|aarch64)   arch="aarch64" ;;
    *)               error "Unsupported architecture: $(uname -m)" ;;
esac

target="${arch}-${os}"
```

#### GitHub API version resolution

```bash
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/edTheGuy00/fdemon/releases/latest" \
        | grep '"tag_name"' \
        | sed -E 's/.*"v([^"]+)".*/\1/'
}
```

This avoids jq as a dependency — uses grep + sed which are universally available.

#### Version comparison

```bash
check_installed_version() {
    if command -v fdemon >/dev/null 2>&1; then
        # Also check the install directory specifically
        local installed_path
        installed_path="$(command -v fdemon)"
        local installed_version
        installed_version="$(fdemon --version 2>/dev/null | awk '{print $2}')"
        echo "$installed_version"
    fi
}
```

If the installed version matches the target version, print a message and exit 0 (success — not an error).

#### Download URL construction

```bash
url="https://github.com/edTheGuy00/fdemon/releases/download/v${version}/fdemon-v${version}-${target}.tar.gz"
```

#### Safety features

- `set -euo pipefail` — exit on error, undefined vars, pipe failures
- `mktemp -d` for temporary download directory
- `trap cleanup EXIT` — always clean up temp dir
- `install -m755` — set correct permissions
- Verify download succeeded (check HTTP status, file size > 0)
- Verify binary works after install (`fdemon --version`)

#### Default install directory

```bash
install_dir="${FDEMON_INSTALL_DIR:-$HOME/.local/bin}"
```

`$HOME/.local/bin` is the XDG standard user binary directory, doesn't require sudo, and is already in PATH on most modern Linux distributions and macOS (when using Homebrew-style setups).

#### PATH hint

```bash
if ! echo "$PATH" | tr ':' '\n' | grep -qx "$install_dir"; then
    echo ""
    echo "  Add fdemon to your PATH by adding this to your shell profile:"
    echo ""
    echo "    export PATH=\"$install_dir:\$PATH\""
    echo ""
    echo "  Then restart your shell or run: source ~/.bashrc (or ~/.zshrc)"
fi
```

### Acceptance Criteria

1. `install.sh` is executable (`chmod +x`) and starts with `#!/bin/bash`
2. Script uses `set -euo pipefail` for safety
3. Detects macOS (Darwin) and Linux correctly
4. Detects x86_64 and arm64/aarch64 architectures
5. Maps OS + arch to the correct Rust target triple (must match release workflow artifact names)
6. Resolves latest version from GitHub API when `--version` is not specified
7. Accepts `--version X.Y.Z` to install a specific version
8. When `fdemon` is already installed at the target version, prints "up to date" and exits cleanly (exit 0)
9. When `fdemon` is not installed or is an older version, downloads and installs the new version
10. Installs to `$HOME/.local/bin` by default, overridable via `$FDEMON_INSTALL_DIR`
11. Creates install directory if it doesn't exist (`mkdir -p`)
12. Cleans up temporary directory on exit (including on failure)
13. Shows PATH hint if install directory is not in `$PATH`
14. Prints the installed version after successful install
15. `--help` flag prints usage information
16. Windows is NOT supported by this script (Windows users download directly from releases) — print a clear error if detected

### Testing

Manual testing scenarios:

```bash
# Test on macOS (local development machine)
bash install.sh --help
bash install.sh --version 0.1.0

# Test version detection
fdemon --version  # Should print "fdemon 0.1.0"
bash install.sh   # Should print "already up to date"

# Test custom install dir
FDEMON_INSTALL_DIR=/tmp/test-fdemon bash install.sh --version 0.1.0
/tmp/test-fdemon/fdemon --version

# Shellcheck validation
shellcheck install.sh
```

### Notes

- The script does NOT support Windows — `install.sh` is a bash script for macOS/Linux only. Windows users should download the `.zip` directly from GitHub Releases
- The GitHub API rate limit for unauthenticated requests is 60/hour — sufficient for install scripts but could be an issue in CI. The `--version` flag bypasses the API call entirely
- `curl` is required — virtually universal on macOS and Linux. The script should check for its presence
- `tar` is required for extraction — also universal
- No dependency on `jq`, `python`, or other optional tools — just `curl`, `tar`, `grep`, `sed`, `awk`
- The script should work in both `bash` and when piped via `curl ... | bash` (no interactive prompts)
- The artifact naming must exactly match what the release workflow produces (task 04): `fdemon-v{VERSION}-{TARGET}.tar.gz`

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `install.sh` | **NEW** — 297-line version-aware install/update script |

### Notable Decisions/Tradeoffs

1. **No jq dependency**: Uses `grep` + `sed` + `awk` for JSON parsing and version extraction — maximizes portability at the cost of fragility if GitHub API response format changes.
2. **`install -m755`**: Uses `install` command instead of `cp` + `chmod` — atomic permission-setting in a single operation.
3. **Windows rejection at two points**: Early `uname -s` check in `main()` before any work, plus the `detect_os()` function — belt and suspenders for Windows detection under MSYS/Git Bash.
4. **`find` fallback for binary extraction**: After `tar -xzf`, checks for the binary at the expected flat path first, then falls back to `find` — handles both flat and nested archive layouts.

### Testing Performed

- `bash -n install.sh` — Syntax check passed
- `bash install.sh --help` — Prints usage information
- `bash install.sh --version` (no arg) — Prints error requesting argument
- Script is executable (`-rwxr-xr-x`)

### Risks/Limitations

1. **GitHub API rate limiting**: Unauthenticated requests limited to 60/hour — the `--version` flag bypasses the API call entirely as a workaround.
2. **No checksum verification**: The script downloads and installs without verifying SHA256 checksums against `checksums-sha256.txt` — acceptable for v1 but could be added later.
3. **grep+sed JSON parsing**: Fragile if GitHub changes their API response format — unlikely but not impossible.
