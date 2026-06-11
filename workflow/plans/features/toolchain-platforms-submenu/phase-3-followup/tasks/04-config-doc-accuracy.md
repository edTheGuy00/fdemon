## Task: Config doc accuracy — `web_browser_executable` does not set `CHROME_EXECUTABLE`

**Objective**: Correct the `ToolchainSettings::web_browser_executable` source doc comment, which currently
claims it "Sets `CHROME_EXECUTABLE`." It does not — the value is a probe override consumed by the Install
Wizard's `check_web` (read directly as a path), and it does **not** call `set_var` or affect Flutter's own
`flutter run -d chrome` process environment. The prose docs (`docs/CONFIGURATION.md`,
`docs/ARCHITECTURE.md`) were already corrected in commit `468b3e1`; this aligns the Rust source comment.

**Depends on**: Phase 3 (merged). Review finding D (MEDIUM); finding J (LOW) is an optional extension below.

**Agent:** implementor

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/config/types.rs` — `web_browser_executable` field doc comment (+ optional validation).

**Files Read (Dependencies):**
- `docs/CONFIGURATION.md` — for wording parity (already corrected to "Takes precedence over…").

### Details

> Line numbers are a current snapshot and will drift — locate by the `web_browser_executable` field.

#### D. Fix the doc comment

The field doc comment (`config/types.rs:~219–228`) reads, in part, "…doctor check. Sets `CHROME_EXECUTABLE`.
…". Reword to match the corrected `docs/CONFIGURATION.md` semantics:

- It is the browser the **Install Wizard's web probe** uses (precedence: this override → `CHROME_EXECUTABLE`
  env var → per-OS default browser locations).
- It does **not** set the `CHROME_EXECUTABLE` environment variable, and does **not** change what
  `flutter run -d chrome` uses for Flutter's own process. (To make Flutter itself use a custom browser, the
  user must export `CHROME_EXECUTABLE` in their shell profile.)
- Keep the "any Chromium-based browser (Chrome, Edge, Brave, Chromium)" guidance and the `None` →
  auto-detect note.

Use wording consistent with the `docs/CONFIGURATION.md` entry ("Takes precedence over the
`CHROME_EXECUTABLE` environment variable and the per-OS default browser locations when fdemon probes for a
web browser").

#### J. (Optional, may defer) Parse-time validation

Optionally add a lightweight guard so a corrupted/adversarial config value can't carry a null byte or be
absurdly long. Safe today (`is_file()` + no shell), so this is hardening only. If implemented, prefer a
small shared helper usable by future executable-path fields (`jdk_path`, etc.) rather than a one-off; if the
shared-helper shape is unclear, **defer** and leave a `// TODO(followup): validate executable-path config
fields` note rather than adding a one-off check. Do not block the task on this.

### Acceptance Criteria

1. The `web_browser_executable` doc comment no longer states it "sets `CHROME_EXECUTABLE`"; it accurately
   describes probe-only precedence and clarifies it does not affect Flutter's own process.
2. Wording is consistent with `docs/CONFIGURATION.md`.
3. (If J implemented) invalid values (null byte / over-long) are rejected or normalized at parse time with a
   test; otherwise a TODO note is left and no behaviour changes.
4. `cargo test -p fdemon-app --lib config` green; `cargo fmt --all` + `cargo clippy -p fdemon-app --all-targets -- -D warnings` clean.

### Testing

```bash
cargo test -p fdemon-app --lib config
cargo fmt --all && cargo clippy -p fdemon-app --all-targets -- -D warnings
```

The existing `test_web_browser_executable_*` tests (defaults, round-trip, no-collision) must stay green. Add
a validation test only if finding J is implemented.

### Notes

- This is primarily a doc-comment correction in source (implementor-editable — it is **not** one of the
  managed `docs/*` files). The managed-doc parity was already handled in `468b3e1`.
- Confined to `config/types.rs`, so it parallelizes with Task 03 (`install_wizard/state.rs`) in the app crate.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/config/types.rs` | Rewrote `web_browser_executable` doc comment: removed "Sets `CHROME_EXECUTABLE`" claim; clarified probe-only semantics, probe precedence order, and that it does not affect `flutter run -d chrome`; added TODO for finding J (validation helper) |

### Notable Decisions/Tradeoffs

1. **Finding J deferred with TODO**: The task says to defer parse-time validation if the shared-helper shape is unclear. Since there's no existing helper pattern for executable-path fields, I added a `// TODO(followup): validate executable-path config fields` comment rather than a one-off check, as directed.

2. **Wording consistency with `docs/CONFIGURATION.md`**: The phrase "Takes precedence over the `CHROME_EXECUTABLE` environment variable and the per-OS default browser locations" is taken directly from the corrected prose docs (commit `468b3e1`). The source doc is now consistent with that.

3. **`///` vs `//` for TODO**: The TODO uses a regular `//` comment (not `///`) so it doesn't appear in generated rustdoc output — only the user-facing doc comment uses `///`.

### Testing Performed

- `cargo test -p fdemon-app --lib config` - Passed (586 tests, 0 failed)
- `cargo fmt --all -- --check` - Passed (no formatting changes needed)
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Doc-comment only**: No behavioural change; zero runtime risk. Existing `test_web_browser_executable_*` tests all remain green as the field semantics are unchanged.
