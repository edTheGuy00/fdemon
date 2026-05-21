## Task: Config-knob for timeout + visibility narrowing + banner URL hint + remaining nitpicks

**Objective**: Land the small post-hardening cleanups in one coherent task: add the `[behavior] version_check_timeout_secs` config field (N10), narrow `pub mod version_check` to `pub(crate)` (N1), document the intentional absence of `spawn_version_check` in headless mode (N2), extract a banner-layout helper to deduplicate `render_regions_impl` vs `Widget::render` (N5), add a URL hint to the banner copy (N9), and update the `behavior_settings_auto_launch_defaults_false` test to cover both new fields (m5).

**Depends on**: 04-version-check-hardening (visibility narrowing applies to the post-refactor module; timeout config replaces the hardcoded duration plumbed by task 04)

**Agent:** implementor

**Estimated Time**: 1.5–2 hours

### Scope

**Files Modified (Write):**

- `crates/fdemon-app/src/config/types.rs`:
  - Add field `version_check_timeout_secs: u8` to `BehaviorSettings` with `#[serde(default = "default_version_check_timeout_secs")]`.
  - Add helper fn `default_version_check_timeout_secs() -> u8 { 3 }` co-located with the existing `default_true` helper.
  - Update `impl Default for BehaviorSettings` to include `version_check_timeout_secs: 3`.
  - Update `behavior_settings_auto_launch_defaults_false` test to assert `s.version_check_timeout_secs == 3` and `s.version_check` (m5).

- `crates/fdemon-app/src/lib.rs`: `pub mod version_check;` → `pub(crate) mod version_check;`.

- `crates/fdemon-app/src/version_check.rs`: `pub async fn check_for_newer_release(...)` → `pub(crate) async fn ...`. (Touches the file changed by task 04 — that's why this task depends on 04.)

- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`:
  - Extract `fn split_notice_area(area: Rect) -> (Rect, Rect)` private helper that calls `Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(area)` and returns `(chunks[0], chunks[1])`. Both `render_regions_impl` and the `Widget::render` impl call it (N5).
  - Update `render_startup_notice` to append a URL hint to the formatted line: `⬆ New version available: v0.6.0 (current v0.5.4) — https://github.com/edTheGuy00/fdemon/releases` (N9). Keep the banner one row tall — the URL is appended to the same line, not a separate row.

- `src/headless/runner.rs`: Add a one-line comment near where `spawn_tool_availability_check` is called explaining the intentional absence of `spawn_version_check` (N2). Format: `// version_check is not spawned in headless mode: no banner surface, and CI runs should not generate stderr chatter.`

- `crates/fdemon-tui/src/runner.rs`: Both call sites of `spawn_version_check` (lines `:78` and `:203` after task 04 lands) — replace the hardcoded `Duration::from_secs(3)` with `Duration::from_secs(engine.settings.behavior.version_check_timeout_secs as u64)`.

- `docs/CONFIGURATION.md`: Add `version_check_timeout_secs` documentation alongside the `version_check` block. Same structural pattern as task 05a from the original feature.

**Files Read (Dependencies):**

- `crates/fdemon-app/src/spawn.rs`: read post-task-04 state of `spawn_version_check(msg_tx, timeout: Duration)` signature.
- `crates/fdemon-app/src/state.rs`: confirm `StartupNotice` enum shape for the banner copy update.

### Details

**Config field**:

```rust
pub struct BehaviorSettings {
    #[serde(default = "default_true")]
    pub confirm_quit: bool,
    #[serde(default)]
    pub auto_launch: bool,
    #[serde(default = "default_true")]
    pub version_check: bool,
    #[serde(default = "default_version_check_timeout_secs")]
    pub version_check_timeout_secs: u8,
}

fn default_version_check_timeout_secs() -> u8 { 3 }
```

**Why `u8`**: 0–255 seconds. Practical max for a sane value is ~30 s; `u8` prevents nonsense values like `100_000` without a `Range` validator. Lower bound: a user setting `0` will get an instant timeout, which is equivalent to disabling — acceptable.

**Banner copy update** (N9):

```rust
fn render_startup_notice(notice: &StartupNotice, area: Rect, buf: &mut Buffer) {
    let text = match notice {
        StartupNotice::NewVersionAvailable { latest } => format!(
            "\u{2B06} New version available: v{} (current v{}) — https://github.com/edTheGuy00/fdemon/releases",
            latest,
            env!("CARGO_PKG_VERSION")
        ),
    };
    let banner = Paragraph::new(text)
        .style(Style::default().fg(palette::STATUS_YELLOW))
        .alignment(Alignment::Center);
    banner.render(area, buf);
}
```

**Width concern**: the banner is center-aligned in a single row. On terminals narrower than ~80 columns the URL may be truncated visually. That's acceptable — Ratatui clips gracefully and the version-number part remains visible (which is the actionable information). No need for an `area.width`-conditional shortening unless someone actually complains.

**Layout helper** (N5):

```rust
fn split_notice_area(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
    ]).split(area);
    (chunks[0], chunks[1])
}
```

Use at both `render_regions_impl` and the `Widget::render` impl call sites — replace the inline `Layout::vertical(...)` blocks.

**Updated defaults test**:

```rust
fn behavior_settings_auto_launch_defaults_false() {
    let s: BehaviorSettings = toml::from_str("").unwrap();
    assert!(!s.auto_launch);
    assert!(s.confirm_quit);
    assert!(s.version_check);
    assert_eq!(s.version_check_timeout_secs, 3);
}
```

**CONFIGURATION.md addition** (alongside the existing `version_check` block):

```markdown
#### `version_check_timeout_secs`

- **Type:** integer (0–255)
- **Default:** `3`

Total HTTP timeout (seconds) for the GitHub release check. Increase this on slow or
flaky connections; decrease to fail-fast.

```toml
[behavior]
version_check_timeout_secs = 10
```

A value of `0` disables the check (equivalent to setting `version_check = false`).
```

### Acceptance Criteria

1. `cargo build --workspace` succeeds.
2. `cargo test -p fdemon-app config` passes; `behavior_settings_auto_launch_defaults_false` covers all four `BehaviorSettings` fields.
3. `grep -n "pub mod version_check" crates/fdemon-app/src/lib.rs` returns no match.
4. `grep -n "pub async fn check_for_newer_release" crates/fdemon-app/src/version_check.rs` returns no match.
5. `grep -n "split_notice_area" crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` returns at least 3 matches (definition + 2 call sites).
6. `grep -n "github.com/edTheGuy00/fdemon/releases" crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` returns one match — the banner copy.
7. `grep -n "version_check is not spawned" src/headless/runner.rs` returns one match.
8. Loading a `config.toml` with `[behavior]\nversion_check_timeout_secs = 10` yields `version_check_timeout_secs: 10`.
9. The version-check tokio task spawned by `runner.rs` actually uses the configured timeout (verifiable via debug logging or manual test).
10. `cargo clippy --workspace --all-targets -- -D warnings` clean.

### Testing

In `crates/fdemon-app/src/config/types.rs` test module:

```rust
#[test]
fn behavior_version_check_timeout_secs_defaults_to_three() {
    let s: BehaviorSettings = toml::from_str("").unwrap();
    assert_eq!(s.version_check_timeout_secs, 3);
}

#[test]
fn behavior_version_check_timeout_secs_can_be_overridden() {
    let s: BehaviorSettings = toml::from_str("version_check_timeout_secs = 10").unwrap();
    assert_eq!(s.version_check_timeout_secs, 10);
}
```

Plus the updated `behavior_settings_auto_launch_defaults_false` per the section above.

### Notes

- This task depends on task 04 — both touch `crates/fdemon-app/src/version_check.rs` (task 04 lands the refactor, task 05 narrows visibility). Run task 05 sequentially after task 04 lands on the working branch.
- `version_check_timeout_secs` is intentionally co-located with `version_check` in `BehaviorSettings`. The naming is verbose — chosen for self-documenting clarity ("the unit and what it gates" both inline). Don't shorten to `timeout_secs` because that name is too generic at the top of the table.
- The banner-copy URL hint deliberately keeps the line under ~90 characters on a default ratatui-friendly width. If terminal-width adaptivity becomes a concern later, the renderer can do `area.width >= 70` gating on whether to append the URL — out of scope here.
- m5 (defaults test fix) is included in this task because the test naturally needs to learn about both new fields at once (`version_check` from the original feature was missed; `version_check_timeout_secs` is new in this task).

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/version-check-banner-followup

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <Rationale>

### Testing Performed

- `cargo build --workspace` — Pending
- `cargo test -p fdemon-app config` — Pending
- `cargo clippy --workspace --all-targets -- -D warnings` — Pending
- Banner copy visual check at ≥80 cols — Pending

### Risks/Limitations

1. **<Risk>**: <Description>
