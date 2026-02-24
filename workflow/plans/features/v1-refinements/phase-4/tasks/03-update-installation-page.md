## Task: Update Installation Page

**Objective**: Replace the "coming soon" placeholder on the installation page with real installation instructions covering the curl install script, platform-specific downloads, and build-from-source instructions.

**Depends on**: None (Phase 3 has already created `install.sh` and `release.yml`)

### Scope

- `website/src/pages/docs/installation.rs`: Rewrite the full page content

### Details

The current `installation.rs` is only 20 lines with a yellow warning banner saying "Pre-built binaries are coming soon!" and a simple build-from-source code block. Now that Phase 3 has created the install script and release workflow, this page needs to be updated with real instructions.

#### Page Structure

**1. Title + intro paragraph**
- "Installation"
- Brief intro: "Flutter Demon can be installed via the install script (recommended), downloaded as a pre-built binary, or built from source."

**2. Quick Install section** (primary, most prominent)
- Green-tinted callout or highlighted section
- One-liner for macOS and Linux:
  ```
  curl -fsSL https://raw.githubusercontent.com/edTheGuy00/flutter-demon/main/install.sh | bash
  ```
- Use the existing `<CodeBlock>` component
- Note: "This downloads the latest release binary for your platform and installs it to `$HOME/.local/bin`."

**3. Specifying a Version**
- Show how to install a specific version:
  ```
  curl -fsSL https://raw.githubusercontent.com/edTheGuy00/flutter-demon/main/install.sh | bash -s -- --version 0.1.0
  ```

**4. Custom Install Directory**
- `FDEMON_INSTALL_DIR` environment variable:
  ```
  FDEMON_INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/edTheGuy00/flutter-demon/main/install.sh | bash
  ```

**5. Supported Platforms table**

| Platform | Architecture | Target |
|----------|-------------|--------|
| macOS | Intel (x86_64) | `x86_64-apple-darwin` |
| macOS | Apple Silicon (aarch64) | `aarch64-apple-darwin` |
| Linux | x86_64 | `x86_64-unknown-linux-gnu` |
| Linux | ARM64 (aarch64) | `aarch64-unknown-linux-gnu` |
| Windows | x86_64 | `x86_64-pc-windows-msvc` |

**6. Manual Download (Windows)**
- "For Windows, download the `.zip` archive from the [GitHub Releases](https://github.com/edTheGuy00/flutter-demon/releases) page."
- Extract and add `fdemon.exe` to your PATH

**7. Build from Source** (existing section, keep and enhance)
- Requirements: Rust 1.70+, Flutter SDK (for running, not building)
- Code block:
  ```
  git clone https://github.com/edTheGuy00/flutter-demon.git
  cd flutter-demon
  cargo build --release
  # Binary is at ./target/release/fdemon
  ```

**8. Verifying Installation**
- Code block:
  ```
  fdemon --version
  ```
- Expected output: `fdemon 0.1.0` (or current version)

**9. PATH Setup hint**
- Blue callout: "If `fdemon` is not found after installation, ensure `$HOME/.local/bin` is in your PATH. Add `export PATH=\"$HOME/.local/bin:$PATH\"` to your shell profile (`~/.bashrc`, `~/.zshrc`, etc.)."

#### Remove the yellow warning banner
The "coming soon" banner must be removed entirely.

### Acceptance Criteria

1. The yellow "coming soon" warning banner is removed
2. The curl one-liner install command is prominently displayed
3. Version-specific and custom directory installation options are documented
4. Supported platforms table shows all 5 targets
5. Windows download instructions link to GitHub Releases
6. Build from source instructions remain with enhanced detail
7. A verification step shows `fdemon --version`
8. PATH setup hint is included
9. Website compiles: `cd website && trunk build`

### Testing

- Visual verification: `cd website && trunk serve` then navigate to `/docs/installation`
- Verify all code blocks render correctly via `<CodeBlock>` component
- Verify the page is comprehensive but not overwhelming

### Notes

- Use the existing `CodeBlock` component from `crate::components::code_block::CodeBlock`
- Follow the same styling patterns as other doc pages: `Section` headings with blue indicators, tables with `bg-slate-900` headers, callout boxes with colored borders
- The GitHub repo URL is `https://github.com/edTheGuy00/flutter-demon` (from `install.sh` in the plan)
- Do NOT define local `Section` or `KeyRow` helpers — the installation page can use plain `<h2>` tags or define its own `Section` helper matching the devtools.rs pattern
