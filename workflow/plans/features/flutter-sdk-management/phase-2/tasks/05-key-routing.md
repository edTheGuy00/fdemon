## Task: Key Routing for Flutter Version Panel

**Objective**: Wire the `V` key binding in Normal mode to open the panel, and add full key routing for `UiMode::FlutterVersion` in `keys.rs`.

**Depends on**: 03-messages-and-update

### Scope

- `crates/fdemon-app/src/handler/keys.rs`: Add `V` key in `handle_key_normal`, add `handle_key_flutter_version()` function, add `UiMode::FlutterVersion` dispatch arm

### Details

#### 1. Top-Level Dispatch (`handle_key`)

Add `UiMode::FlutterVersion` to the main `match state.ui_mode` block in `handle_key()`:

```rust
pub fn handle_key(state: &AppState, key: InputKey) -> Option<Message> {
    match state.ui_mode {
        // ... existing arms ...
        UiMode::FlutterVersion => handle_key_flutter_version(key, state),
        // ...
    }
}
```

#### 2. Opening Key in Normal Mode (`handle_key_normal`)

Add `V` (uppercase) to `handle_key_normal`. Place it near other panel-opening keys (e.g., `','` for Settings, `'d'` for DevTools):

```rust
fn handle_key_normal(state: &AppState, key: InputKey) -> Option<Message> {
    // ... existing tag filter intercept ...

    match key {
        // ... existing arms ...
        InputKey::Char('V') => Some(Message::ShowFlutterVersion),
        // ...
    }
}
```

**Why uppercase `V`?** Lowercase `v` might conflict with future vim-style visual mode. Uppercase `V` is consistent with the PLAN.md specification and is easily discoverable alongside other shift-key shortcuts.

#### 3. Flutter Version Key Handler (`handle_key_flutter_version`)

```rust
fn handle_key_flutter_version(key: InputKey, state: &AppState) -> Option<Message> {
    match key {
        // ── Global keys ──
        InputKey::CtrlC => Some(Message::Quit),

        // ── Panel lifecycle ──
        InputKey::Escape => Some(Message::FlutterVersionEscape),

        // ── Pane switching ──
        InputKey::Tab => Some(Message::FlutterVersionSwitchPane),

        // ── Navigation (active in both panes, but handlers gate on focused pane) ──
        InputKey::Char('k') | InputKey::Up => Some(Message::FlutterVersionUp),
        InputKey::Char('j') | InputKey::Down => Some(Message::FlutterVersionDown),

        // ── Actions (only meaningful in VersionList pane, handlers gate) ──
        InputKey::Enter => Some(Message::FlutterVersionSwitch),
        InputKey::Char('d') => Some(Message::FlutterVersionRemove),
        InputKey::Char('i') => Some(Message::FlutterVersionInstall),
        InputKey::Char('u') => Some(Message::FlutterVersionUpdate),

        _ => None,
    }
}
```

#### 4. Key Mapping Summary

| Key | Message | Handler gates on |
|-----|---------|-----------------|
| `Ctrl+C` | `Quit` | — (always) |
| `Esc` | `FlutterVersionEscape` | — (always) |
| `Tab` | `FlutterVersionSwitchPane` | — (always) |
| `k` / `Up` | `FlutterVersionUp` | VersionList pane |
| `j` / `Down` | `FlutterVersionDown` | VersionList pane |
| `Enter` | `FlutterVersionSwitch` | VersionList pane + non-active |
| `d` | `FlutterVersionRemove` | VersionList pane + non-active |
| `i` | `FlutterVersionInstall` | Phase 3 stub |
| `u` | `FlutterVersionUpdate` | Phase 3 stub |

**Note**: Key actions like `Enter`, `d`, `i`, `u` emit messages regardless of focused pane. The **handlers** (Task 04) gate on `focused_pane == VersionList` and return `UpdateResult::none()` if the pane isn't right. This keeps key routing simple and stateless.

### Acceptance Criteria

1. `V` in Normal mode emits `Message::ShowFlutterVersion`
2. `UiMode::FlutterVersion` dispatches to `handle_key_flutter_version()`
3. `Ctrl+C` → `Quit`
4. `Esc` → `FlutterVersionEscape`
5. `Tab` → `FlutterVersionSwitchPane`
6. `j`/`Down` → `FlutterVersionUp` (sic: Down key maps to Down message — verify naming)
7. `k`/`Up` → `FlutterVersionUp`
8. `Enter` → `FlutterVersionSwitch`
9. `d` → `FlutterVersionRemove`
10. `i` → `FlutterVersionInstall`
11. `u` → `FlutterVersionUpdate`
12. Unmapped keys return `None` (no action)
13. `cargo check --workspace` compiles
14. `cargo test --workspace` passes
15. `cargo clippy --workspace -- -D warnings` passes

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn fv_state() -> AppState {
        let mut state = test_app_state();
        state.ui_mode = UiMode::FlutterVersion;
        state
    }

    #[test]
    fn test_v_key_in_normal_opens_panel() {
        let mut state = test_app_state();
        state.ui_mode = UiMode::Normal;
        let msg = handle_key(&state, InputKey::Char('V'));
        assert_eq!(msg, Some(Message::ShowFlutterVersion));
    }

    #[test]
    fn test_escape_closes_panel() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Escape);
        assert_eq!(msg, Some(Message::FlutterVersionEscape));
    }

    #[test]
    fn test_tab_switches_pane() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Tab);
        assert_eq!(msg, Some(Message::FlutterVersionSwitchPane));
    }

    #[test]
    fn test_j_navigates_down() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Char('j'));
        assert_eq!(msg, Some(Message::FlutterVersionDown));
    }

    #[test]
    fn test_k_navigates_up() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Char('k'));
        assert_eq!(msg, Some(Message::FlutterVersionUp));
    }

    #[test]
    fn test_enter_switches_version() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Enter);
        assert_eq!(msg, Some(Message::FlutterVersionSwitch));
    }

    #[test]
    fn test_d_removes_version() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Char('d'));
        assert_eq!(msg, Some(Message::FlutterVersionRemove));
    }

    #[test]
    fn test_unmapped_key_returns_none() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::Char('z'));
        assert_eq!(msg, None);
    }

    #[test]
    fn test_ctrl_c_quits() {
        let state = fv_state();
        let msg = handle_key(&state, InputKey::CtrlC);
        assert_eq!(msg, Some(Message::Quit));
    }
}
```

### Notes

- **`InputKey` variants**: Check the actual `InputKey` enum for exact variant names. The codebase may use `InputKey::Char('k')`, `InputKey::Up`, `InputKey::CtrlC`, etc. — adjust the match arms to match the actual enum. Look at how `handle_key_new_session_dialog` matches keys for the exact syntax.
- **No state reads needed**: The key routing function only needs `key` and `state.ui_mode`. Unlike the New Session Dialog (which checks `fuzzy_modal` and `dart_defines_modal` for priority intercept), the Flutter Version panel has no sub-modals in Phase 2.
- **`V` vs `v`**: The PLAN specifies `V` (uppercase). Use `InputKey::Char('V')`. If the InputKey enum normalizes shift+letter differently, check `handle_key_normal` for how `'D'` (toggle DAP) is matched.
