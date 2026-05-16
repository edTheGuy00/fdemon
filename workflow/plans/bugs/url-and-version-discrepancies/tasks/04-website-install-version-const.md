## Task: Auto-Populate Current Version on Website Installation Page

**Objective**: Extend `website/build.rs` to read the workspace `Cargo.toml` version at build time and emit a `FDEMON_VERSION` constant. Replace the two literal `0.1.0` references on the installation page with that constant so the docs auto-update with every release.

**Depends on**: None

**Estimated Time**: 45 minutes

### Scope

**Files Modified (Write):**
- `website/build.rs`: Add a function that reads `../Cargo.toml`, extracts `[workspace.package].version`, and emits a `version_generated.rs` include file containing `pub const FDEMON_VERSION: &str = "X.Y.Z";`.
- `website/src/data.rs`: Add a re-export of the generated `FDEMON_VERSION` constant via `include!(concat!(env!("OUT_DIR"), "/version_generated.rs"));`.
- `website/src/pages/docs/installation.rs`: Replace the two `0.1.0` literals (lines 31 and 149) with interpolation of `FDEMON_VERSION`.

**Files Read (Dependencies):**
- `Cargo.toml` (workspace root) — source of the version string (`version = "0.5.2"` at the time of writing).
- `website/build.rs` existing structure — established pattern for reading external files and emitting generated Rust source.

### Details

#### Step 1 — `website/build.rs`

The current `main()` already reads `changelog.json` from `CARGO_MANIFEST_DIR` and writes `changelog_generated.rs` to `OUT_DIR`. Add a parallel flow that reads `../Cargo.toml` and emits `version_generated.rs`. Concrete shape:

```rust
fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    // ── Existing changelog generation (unchanged) ────────────────────────
    let json_path = Path::new(&manifest_dir).join("changelog.json");
    let changelog_out = Path::new(&out_dir).join("changelog_generated.rs");
    println!("cargo::rerun-if-changed=changelog.json");
    // ... existing changelog code unchanged ...

    // ── New: emit FDEMON_VERSION constant from workspace Cargo.toml ──────
    let workspace_cargo = Path::new(&manifest_dir).join("..").join("Cargo.toml");
    let version_out = Path::new(&out_dir).join("version_generated.rs");
    println!("cargo::rerun-if-changed=../Cargo.toml");

    let version = read_workspace_version(&workspace_cargo).unwrap_or_else(|err| {
        println!("cargo:warning=could not read workspace version ({err}); falling back to 'unknown'");
        "unknown".to_string()
    });
    fs::write(
        &version_out,
        format!("pub const FDEMON_VERSION: &str = \"{}\";\n", escape(&version)),
    )
    .expect("failed to write version_generated.rs");
}

fn read_workspace_version(path: &Path) -> Result<String, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    // Minimal scan — avoids pulling in `toml` as a new build dependency.
    // The workspace Cargo.toml has a single `version = "X.Y.Z"` under `[workspace.package]`.
    let mut in_workspace_package = false;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_workspace_package = line == "[workspace.package]";
            continue;
        }
        if in_workspace_package {
            if let Some(rest) = line.strip_prefix("version") {
                let rest = rest.trim_start();
                if let Some(eq_rest) = rest.strip_prefix('=') {
                    let value = eq_rest.trim().trim_matches('"');
                    if !value.is_empty() {
                        return Ok(value.to_string());
                    }
                }
            }
        }
    }
    Err("could not find [workspace.package] version".into())
}
```

Notes on the parser:
- The existing `build.rs` already pulls in `serde` + `serde_json` as build deps for the changelog; adding a `toml` build dep would work but is extra surface area for one string. The hand-rolled scanner is ~15 LoC and only needs to handle the project's own well-formed `Cargo.toml`.
- If the scan fails for any reason (file missing, manual edit broke the format), the build falls back to `"unknown"` rather than panicking — keeps `trunk build` working in unusual environments and surfaces a warning so the issue is visible.

#### Step 2 — `website/src/data.rs`

Add at the top of the file (after the existing `use` statements):

```rust
include!(concat!(env!("OUT_DIR"), "/version_generated.rs"));
```

This makes `pub const FDEMON_VERSION: &str` available for re-export from the `data` module. Verify the constant is visible from the installation page by adding `use crate::data::FDEMON_VERSION;` in that file.

#### Step 3 — `website/src/pages/docs/installation.rs`

Two changes:

**Line 31 (before):**

```rust
<CodeBlock code="curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash -s -- --version 0.1.0" />
```

**Line 31 (after):**

```rust
<CodeBlock code=format!(
    "curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash -s -- --version {}",
    FDEMON_VERSION
) />
```

(Or use Leptos string interpolation equivalent — match the style of other dynamic strings in the file.)

**Line 149 (before):**

```rust
"Expected output: "<code class="text-blue-400">"fdemon 0.1.0"</code>" (or the installed version)."
```

**Line 149 (after):**

```rust
"Expected output: "<code class="text-blue-400">{format!("fdemon {}", FDEMON_VERSION)}</code>" (or the installed version)."
```

Add `use crate::data::FDEMON_VERSION;` near the top of the file.

### Acceptance Criteria

1. `website/build.rs` writes `version_generated.rs` containing `pub const FDEMON_VERSION: &str = "0.5.2";` (or whatever the current workspace version is) into `OUT_DIR`.
2. `cargo:rerun-if-changed=../Cargo.toml` directive ensures the constant rebuilds when the workspace version bumps.
3. If `../Cargo.toml` is missing or malformed, the build emits a `cargo:warning=…` and falls back to `FDEMON_VERSION = "unknown"` rather than failing.
4. `website/src/data.rs` exposes `FDEMON_VERSION` (via the generated include).
5. `website/src/pages/docs/installation.rs` contains no literal `0.1.0` strings (`grep -n "0\.1\.0" website/src/pages/docs/installation.rs` returns nothing).
6. The install-script command example and the "Expected output" verification example both render with the current workspace version (`0.5.2` today).
7. `cd website && trunk build` succeeds.

### Testing

```bash
# Sanity: ensure build emits the constant correctly
cd website
trunk build
grep -r "FDEMON_VERSION" target/

# Visual check via local server
trunk serve
# Visit http://localhost:8080/docs/installation
# Confirm both code blocks show "0.5.2" (or the current workspace version),
# not "0.1.0".

# Confirm the literal "0.1.0" is gone from the source
grep -n "0\.1\.0" website/src/pages/docs/installation.rs
# (should return no matches)
```

If you have the workspace nightly toolchain installed:

```bash
cd website
cargo check --target wasm32-unknown-unknown
```

### Notes

- The fallback to `"unknown"` means an external consumer who unpacks the `website/` directory in isolation will still get a valid build — the page just shows `fdemon unknown` as the expected output. Acceptable degradation; the warning makes it visible.
- This task does **not** change `website/Cargo.toml:3` (`version = "0.1.0"` — the website crate's own SemVer). That field is independent of fdemon's release version; conflating them would be wrong.
- `website/src/pages/docs/debugging.rs:248`'s `"version": "0.2.0"` is the **VS Code `launch.json` schema version**, not the fdemon version. Do not change it.
- If `format!()` interpolation inside Leptos `view!` doesn't compose cleanly with the existing `CodeBlock` component, alternative: split into a `let install_cmd = format!(…);` outside the view, then pass `code=install_cmd.clone()` (or `code=&install_cmd`) into the component. Match the existing component's prop signature.
