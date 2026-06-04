## Task: Test-quality fixes — [/] key tests + misleading test names (m7 + m8 + m9)

**Severity:** MINOR (m7, m8, m9)

**Objective**: Close the test-coverage and test-honesty gaps the review found: add
the missing `[`/`]` key-mapping tests, rename/strengthen a test that asserts a
now-false invariant, and make the package-manager precedence claim testable.

**Depends on**: 04-pure-guided-commands-tea (it re-touches `state.rs` and
`prerequisites.rs`; run these test fixes on top of the final code shape)

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/keys.rs` (m7)
- `crates/fdemon-app/src/install_wizard/state.rs` (m8)
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs` (m9)

**Files Read (Dependencies):**
- Task 04's final signatures for `prerequisites_guided_commands` / `build_steps`.

### Details

**m7 — `[`/`]` key tests (MINOR).** `keys.rs:444-445` maps `Char('[')` →
`Message::InstallWizardPrevCommand` and `Char(']')` → `Message::InstallWizardNextCommand`
in `handle_key_install_wizard`, but neither has a test, unlike every sibling binding
(`Enter`/`Esc`/`Tab`/`r`/`c`, tests ~`3613-3676`). Add two tests beside
`test_c_in_install_wizard_emits_copy_command`:

```rust
#[test]
fn test_bracket_open_in_install_wizard_emits_prev_command() {
    let state = make_install_wizard_state();
    let msg = handle_key(&state, InputKey::Char('['));
    assert!(matches!(msg, Some(Message::InstallWizardPrevCommand)),
        "'[' in InstallWizard should emit InstallWizardPrevCommand, got: {msg:?}");
}

#[test]
fn test_bracket_close_in_install_wizard_emits_next_command() {
    let state = make_install_wizard_state();
    let msg = handle_key(&state, InputKey::Char(']'));
    assert!(matches!(msg, Some(Message::InstallWizardNextCommand)),
        "']' in InstallWizard should emit InstallWizardNextCommand, got: {msg:?}");
}
```

(Use the existing `make_install_wizard_state` / `handle_key` helpers; drop `mut` if
unused.)

**m8 — false invariant (MINOR).** `state.rs:1093-1104` —
`test_non_android_steps_have_no_guided_commands` loops over all non-AndroidTools
steps asserting `guided_commands.is_empty()`. This invariant is **no longer true**:
the Prerequisites step now carries guided commands when prerequisites are missing.
The test passes only because its fixture (`report_with_jdk`) has no Prerequisites/Git
component, so `prerequisites_guided_commands` short-circuits. Rename it to reflect
the actual fixture (e.g. `test_non_android_non_prereq_steps_have_no_guided_commands_when_prereqs_absent`)
and add a dedicated test asserting the steps that should **never** gain guided
commands do not:

```rust
#[test]
fn test_path_config_flutter_sdk_doctor_never_have_guided_commands() {
    // Use a report exercising all component kinds.
    let steps = build_steps(/* report with FlutterSdk/Jdk/Prerequisites all present */);
    for kind in [WizardStepKind::PathConfig, WizardStepKind::FlutterSdk, WizardStepKind::Doctor] {
        let step = steps.iter().find(|s| s.kind == kind).unwrap();
        assert!(step.guided_commands.is_empty(), "{kind:?} must never have guided commands");
    }
}
```

**m9 — precedence test with no assertion (MINOR).** `prerequisites.rs:504-516` —
`test_package_manager_precedence_apt_before_dnf` calls `detect_linux_package_manager()`
and discards the result with a comment that `which` cannot be mocked; it asserts
nothing, and the module comment (~`499-502`) falsely claims a "pure helper" exists.
Make precedence genuinely testable: extract a pure helper, e.g.

```rust
fn detect_from_candidates(present: &[&str]) -> LinuxPackageManager { /* same order */ }
```

that `detect_linux_package_manager` calls with the `which`-resolved set, and unit-test
the ordering against it (apt before dnf, dnf before yum, …, Unknown when empty).
At minimum, if extraction is not done, rename the test to
`test_detect_linux_package_manager_does_not_panic` to remove the false precedence
claim and fix the misleading module comment (also dedupe with the existing
`test_detect_linux_package_manager_never_panics`).

### Acceptance Criteria

1. `[` and `]` in `UiMode::InstallWizard` each have a passing key-mapping test.
2. The renamed non-android test reflects its fixture, and a new test asserts
   `PathConfig`/`FlutterSdk`/`Doctor` never carry guided commands.
3. Package-manager precedence is asserted via a pure helper, **or** the no-assertion
   test is renamed to its true claim and the false module comment is corrected.
4. No duplicate/redundant precedence tests remain; all tests follow
   `test_<fn>_<scenario>_<result>` naming.

### Testing

Run `cargo test --workspace`; the new/renamed tests pass and exercise the real
behavior (not vacuous truth).

### Notes

- Test-only changes — no production logic should change in this task.
- Sequenced after task 04 so the new tests assert against the final
  `prerequisites_guided_commands` / report signatures.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/keys.rs` | Added `test_bracket_open_in_install_wizard_emits_prev_command` and `test_bracket_close_in_install_wizard_emits_next_command` tests (m7) |
| `crates/fdemon-app/src/install_wizard/state.rs` | Renamed `test_non_android_steps_have_no_guided_commands` → `test_non_android_non_prereq_steps_have_no_guided_commands_when_prereqs_absent`; added `test_path_config_flutter_sdk_doctor_never_have_guided_commands` (m8) |
| `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs` | Extracted `detect_from_candidates(present: &[&str])` pure helper; refactored `detect_linux_package_manager` to call it; replaced no-assertion precedence test with 7 real precedence/edge-case tests; updated module comment (m9) |

### Notable Decisions/Tradeoffs

1. **`detect_from_candidates` uses a `const ORDER` slice**: The ordering table `&[(&str, LinuxPackageManager)]` is the single source of truth for precedence, making the logic readable and the tests trivially verifiable against it.
2. **m8 new test uses all-Ok components**: The `test_path_config_flutter_sdk_doctor_never_have_guided_commands` test builds a complete all-Ok report so both `prerequisites_guided_commands` and JDK guidance short-circuit, giving clean isolation of the "never" claim.
3. **Pre-existing fdemon-tui clippy issue**: `step_detail.rs` has two `doc-lazy-continuation` warnings unrelated to this task — confirmed pre-existing before these changes.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app --lib` - Passed (2813 tests)
- `cargo test -p fdemon-daemon --lib` - Passed (1040 tests)
- `cargo clippy -p fdemon-app -p fdemon-daemon --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Pre-existing clippy failure in fdemon-tui**: `doc-lazy-continuation` errors in `step_detail.rs` were present before this task and are not introduced by these changes.
