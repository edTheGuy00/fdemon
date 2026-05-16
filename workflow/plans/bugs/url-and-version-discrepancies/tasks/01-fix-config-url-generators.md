## Task: Fix `.fdemon/config.toml` and `.fdemon/launch.toml` URL Generators

**Objective**: Replace the placeholder `https://github.com/example/flutter-demon` URL embedded in the four config-file generator functions with the canonical `https://fdemon.dev/docs/configuration` URL, and add a regression test so the URL can't silently drift again.

**Depends on**: None

**Estimated Time**: 30 minutes

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/config/settings.rs`: Three string literals (header comments inside three generator functions) + one new regression test.
- `crates/fdemon-app/src/config/launch.rs`: One string literal (header comment in `init_launch_file`) + one new regression test.

**Files Read (Dependencies):**
- `README.md` lines 18–21, 107 — canonical docs URLs (`https://fdemon.dev/docs/configuration`).
- `website/src/pages/docs/configuration.rs` — verifies the docs page covers both `config.toml` and `launch.toml`, so the same URL is correct for both.

### Details

**Current state:** four occurrences of the dead placeholder URL ship as the `See:` line of the auto-generated config files. None of these are reachable — `github.com/example/flutter-demon` is a placeholder from the original project scaffold (see `workflow/plans/features/initial-project-setup/phase_3/tasks/01-config-module.md`).

**Exact replacements:**

`crates/fdemon-app/src/config/settings.rs`:

| Line | Function | Before | After |
|------|----------|--------|-------|
| 450 | `init_project_config()` default-content block | `# See: https://github.com/example/flutter-demon#configuration` | `# See: https://fdemon.dev/docs/configuration` |
| 554 | `generate_config_header()` returned string | `# See: https://github.com/example/flutter-demon#configuration` | `# See: https://fdemon.dev/docs/configuration` |
| 656 | `generate_default_config()` returned string | `# See: https://github.com/example/flutter-demon#configuration` | `# See: https://fdemon.dev/docs/configuration` |

`crates/fdemon-app/src/config/launch.rs`:

| Line | Function | Before | After |
|------|----------|--------|-------|
| 123 | `init_launch_file()` default-content block | `# See: https://github.com/example/flutter-demon#launch-configurations` | `# See: https://fdemon.dev/docs/configuration` |

The `launch.toml` URL drops the `#launch-configurations` anchor: the website's configuration page documents launch configs in a tabbed section without an HTML anchor by that name, so a bare URL is the most accurate target.

**Regression test (settings.rs):** add a `#[test]` next to the existing `test_saved_settings_file_has_header` (around line 1366) that calls `save_settings()` against a tempdir, reads the resulting `.fdemon/config.toml`, and asserts both:

```rust
#[test]
fn test_default_config_references_fdemon_dev_docs() {
    let temp = tempdir().unwrap();
    let settings = Settings::default();
    save_settings(temp.path(), &settings).unwrap();
    let content = std::fs::read_to_string(temp.path().join(".fdemon/config.toml")).unwrap();
    assert!(
        content.contains("https://fdemon.dev/docs/configuration"),
        "config.toml header must point at fdemon.dev docs"
    );
    assert!(
        !content.contains("github.com/example"),
        "config.toml must not carry the placeholder URL"
    );
}
```

Also add an equivalent test for `generate_default_config()` (called by `init_fdemon_directory()`) — call `init_fdemon_directory()` on a tempdir, read the resulting file, assert the same two properties. This is the path that actually fires on first-run, so it's worth covering directly.

**Regression test (launch.rs):** add an analogous test next to whatever existing tests already cover `init_launch_file()`. If none exist, add one:

```rust
#[test]
fn test_default_launch_references_fdemon_dev_docs() {
    let temp = tempdir().unwrap();
    init_launch_file(temp.path()).unwrap();
    let content = std::fs::read_to_string(temp.path().join(".fdemon/launch.toml")).unwrap();
    assert!(content.contains("https://fdemon.dev/docs/configuration"));
    assert!(!content.contains("github.com/example"));
}
```

### Acceptance Criteria

1. All four `See:` URL strings in `settings.rs` and `launch.rs` point at `https://fdemon.dev/docs/configuration`.
2. `grep -rn "github.com/example" crates/fdemon-app/src/config/` returns zero hits.
3. Existing `test_saved_settings_file_has_header` still passes (it only asserts on `"Flutter Demon Configuration"` and `starts_with('#')`, both preserved).
4. New regression tests pass and would fail if either URL drifts.
5. `cargo fmt --all`, `cargo check -p fdemon-app`, `cargo test -p fdemon-app`, `cargo clippy -p fdemon-app -- -D warnings` all pass.

### Testing

Run the focused test suite:

```bash
cargo test -p fdemon-app config::settings
cargo test -p fdemon-app config::launch
```

And the full quality gate:

```bash
cargo fmt --all -- --check && \
  cargo check --workspace --all-targets && \
  cargo test --workspace && \
  cargo clippy --workspace --all-targets -- -D warnings
```

### Notes

- This task only touches the four generator functions plus the new test code. Do **not** edit `tests/fixtures/simple_app/.fdemon/config.toml`, `example/app1/.fdemon/config.toml`, or `crates/fdemon-tui/.fdemon/config.toml` — those are owned by Task 02.
- The launch URL anchor (`#launch-configurations`) doesn't currently exist on the configuration page. Use the bare URL; a follow-up can re-add the fragment if a heading is later added.
- The placeholder URL also appears in three historical workflow plan docs (`workflow/plans/features/...`). Those are immutable historical artifacts and explicitly out of scope.
