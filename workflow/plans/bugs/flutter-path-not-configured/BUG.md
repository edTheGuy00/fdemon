# Bug: Managed Flutter install never writes the SDK `bin` dir to PATH

## Status

🔬 **Investigated — root cause confirmed.** Awaiting approval of the fix approach
before the task breakdown (`TASKS.md`) is written.

Relates to [`workflow/plans/features/toolchain-bootstrap/PLAN.md`](../../features/toolchain-bootstrap/PLAN.md)
(Phase 2 — *Managed Flutter SDK install + PATH configuration*).

---

## Symptom (as reported)

After using the install wizard, the user's `~/.zshenv` contains:

```sh
# >>> fdemon flutter path >>>
export PATH="$PATH:"'/tmp/.tmpo9xtqR/bin'
# <<< fdemon flutter path <<<

# >>> fdemon android env >>>
export ANDROID_HOME='/home/ed/Android/Sdk'
export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH"
# <<< fdemon android env <<<
```

Two distinct problems are visible:

1. **The Flutter PATH points at a dead temp dir** (`/tmp/.tmpo9xtqR/bin`) that no
   longer exists — so `flutter` is **not** on PATH in new shells. The Android
   block, by contrast, is correct (`/home/ed/Android/Sdk`).
2. A leftover **empty Android SDK temp dir** exists at `/tmp/.tmpGOfMr6/`.

The user clarified the `/tmp/.tmp*` path is likely **from an older run**, and the
real ask is: **fix "the Flutter path is not added (correctly)."**

---

## Root Cause (CONFIRMED)

### Primary defect — PATH configuration never runs automatically after an install

The wizard treats **PathConfig** (writing `<sdk>/bin` to the shell rc file) as a
**separate, manually-triggered step**. Nothing chains it after a managed install
completes.

`handle_step_completed` for `WizardStepKind::FlutterSdk`
(`crates/fdemon-app/src/handler/install_wizard/actions.rs:446-464`):

```rust
if kind == WizardStepKind::FlutterSdk {
    if let Some(path) = sdk_path {
        state.install_wizard_state.installed_sdk_path = Some(path.clone());   // stash
        state.settings.flutter.sdk_path = Some(path);                         // settings only
        return UpdateResult::message_and_action(
            Message::InstallWizardRerunPreflight,                             // re-check
            UpdateAction::PersistSettings { /* .fdemon/config.toml */ },      // persist
        );
    }
}
```

The chain is **PersistSettings → re-run preflight → ScanInstalledSdks**. There is
**no** `RunWizardStep { kind: PathConfig }` dispatch — the SDK `bin` dir is never
written to the rc file as part of installing Flutter. The user must *separately*
select the PathConfig step and press Enter.

The same gap applies to `WizardStepKind::AndroidTools`
(`actions.rs:467-487`): it persists `android_sdk_root` and re-runs preflight, but
the `ANDROID_HOME` / Android `PATH` block is **only** written by the PathConfig
executor (`crates/fdemon-app/src/actions/mod.rs:1150-1175`, which calls both
`add_to_path` and `add_android_env`). So Android env is *also* only configured
when PathConfig is run manually.

**Effect:** "Install Flutter" leaves `flutter` off the PATH for new shells. The
PLAN intended the opposite — Phase 2 step 3-4 says the install flow should *write
an idempotent PATH export for `<flutter>/bin`* **and** write `[flutter]
sdk_path`. Only the second half shipped.

### Why the user's stale temp path exists (secondary, self-healing)

- The install outcome's `sdk_path` is **always** `final_dir` (e.g.
  `~/fvm/versions/stable`), never a temp dir, in the current code:
  `FlutterInstallOutcome.sdk_path = final_dir` (`flutter_install.rs:784`), set
  after the atomic temp-dir→`final_dir` rename (`flutter_install.rs:740-754`).
- The PathConfig writer is **content-aware idempotent**: `apply_fence`
  (`path_config.rs:453-487`) *replaces* an existing fence block when it does not
  contain the new `bin_dir`. So a stale `/tmp/.tmp…/bin` block **will be replaced**
  with the correct path the next time PathConfig runs with a real SDK.
- Therefore the `/tmp/.tmpo9xtqR/bin` entry is a relic of an **older build** (one
  whose install reported a temp dir, before the rename/`TempDirGuard` hardening)
  that has simply never been overwritten — because PathConfig has not been run
  since. Auto-chaining PathConfig (the primary fix) makes the stale block
  self-correct on the next install.

### The `/tmp/.tmp*` dirs point to a test-isolation hazard

Production toolchain code never uses the `tempfile` crate (`/tmp/.tmpXXXXXX` is a
`tempfile::TempDir`/`tempdir()` name) — every `tempfile::TempDir` usage under
`crates/fdemon-daemon/src/toolchain/` is inside `#[cfg(test)]`. The public rc-file
writers resolve the **real** `$HOME`:

- `add_to_path` → `home_dir()` → `rc_file_for_shell(Zsh, home)` → real
  `~/.zshenv` (`path_config.rs:229-238`, `159-192`).
- `add_android_env` likewise (`path_config.rs:293-302`).

The two known tests that call the real writers
(`test_add_to_path_rejects_injection_path` :1550, `test_add_android_env_rejects_injection_path`
:2202) pass paths containing a newline, which `validate_bin_dir` rejects
**before** any file I/O — so they do **not** pollute `~/.zshenv` today. But the
pattern is fragile: any test (now or future) that calls `add_to_path` /
`add_android_env` with a *clean* path and a supported shell on a matching platform
**will write to the developer's real `~/.zshenv`**. This is the most plausible
origin of the stale `/tmp/.tmp*` blocks from an earlier build, and it must be
fenced off so it can never recur.

---

## Evidence Map

| Finding | Location |
|---|---|
| FlutterSdk completion: no PathConfig dispatch (only persist + preflight) | `crates/fdemon-app/src/handler/install_wizard/actions.rs:446-464` |
| AndroidTools completion: same gap (Android env only via PathConfig) | `actions.rs:467-487` |
| PathConfig executor writes both Flutter PATH + Android env | `crates/fdemon-app/src/actions/mod.rs:1144-1175` |
| `installed_sdk_path` cleared only on PathConfig success | `actions.rs:489-502` |
| Install outcome `sdk_path` = `final_dir` (never temp) | `crates/fdemon-daemon/src/toolchain/flutter_install.rs:784`, rename `740-754` |
| `apply_fence` replaces a stale block on `bin_dir` mismatch | `crates/fdemon-daemon/src/toolchain/path_config.rs:453-487` |
| Public writers resolve the real `$HOME` rc file | `path_config.rs:217-238`, `281-303`, `159-192` |
| HostShell from `$SHELL`; zsh → `~/.zshenv` if present else `~/.zprofile` | `crates/fdemon-daemon/src/toolchain/types.rs:171-200`; `path_config.rs:182-192` |
| Injection tests reject before write (safe today) | `path_config.rs:1550-1558`, `2202-2208` |

---

## Proposed Fix

### Fix 1 (PRIMARY) — Auto-configure PATH after a managed install

After a successful **FlutterSdk** install, automatically run the PathConfig
write (Flutter `<sdk>/bin` → rc file), then re-run preflight. Do the same after a
successful **AndroidTools** install so `ANDROID_HOME` + Android `PATH` are written
without a manual step.

**Recommended design (keeps TEA purity, reuses existing executor):**

1. Add a follow-up message, e.g. `Message::InstallWizardAutoConfigurePath { kind }`,
   emitted by `handle_step_completed` after FlutterSdk/AndroidTools success (in
   addition to / sequenced with `PersistSettings`).
2. Its handler reuses the existing PathConfig run logic (`handle_run_selected_step`
   for `WizardStepKind::PathConfig`) to produce
   `UpdateAction::RunWizardStep { kind: PathConfig, path_bin_dir, android_sdk_root, … }`,
   driven by the freshly-stashed `installed_sdk_path` / `settings.flutter.sdk_path`
   and the resolved Android root.
3. The existing PathConfig completion already re-runs preflight
   (`actions.rs:501`), so the step list refreshes after the write.

**Sequencing note for the implementer:** `UpdateResult` carries one follow-up
message + one action. The persist + auto-config + preflight chain must be threaded
through messages (e.g. FlutterSdk completion → `PersistSettings` action +
`AutoConfigurePath` message; `AutoConfigurePath` → `RunWizardStep{PathConfig}`;
PathConfig completion → `InstallWizardRerunPreflight`). Confirm no `run_seq` /
`install_task` clobber across the auto-chained step (see the Phase-7 task-01
seq-guard work).

**Behavioural guardrails:**

- Auto-config must be **idempotent** (it already is — `apply_fence`) and must not
  loop (PathConfig completion must not re-trigger FlutterSdk).
- If the SDK is *already installed* (short-circuit at `flutter_install.rs:612-625`),
  the outcome still carries `final_dir`, so auto-config still corrects a stale
  block — desired.
- Decide whether the FlutterSdk auto-config should also write the Android block if
  an Android root happens to be discoverable, or only the Flutter PATH. Recommend
  **only the Flutter PATH on FlutterSdk auto-config** (pass `android_sdk_root:
  None`) and let AndroidTools auto-config own the Android block, to keep each
  step's side effects scoped to what it installed.

### Fix 2 (SECONDARY) — Self-heal stale fence blocks whose target is gone

Optional hardening: when PathConfig runs (or during preflight), if an existing
fdemon Flutter fence block points at a directory that **no longer exists**, treat
it as replaceable even if the user has not reinstalled. With Fix 1, the common
case self-corrects; this covers the "user never reinstalls" tail. Low priority.

### Fix 3 (SECONDARY) — Fence the rc-file writers off from the real `$HOME` in tests

Make it **impossible** for the test suite to mutate a developer's real
`~/.zshenv` / `~/.zprofile`:

- Audit every test that calls `add_to_path`, `add_android_env`, or any path that
  reaches `home_dir()` + an rc-file write, across the workspace (not just
  `path_config.rs`).
- Sandbox them: route the writers' home resolution through an injectable seam
  (e.g. a `home_dir()` that honours a test override, or test-only variants that
  take an explicit `home: &Path`), and have all tests pass a `TempDir`. The
  pure-string/`*_to_rc_file(rc_file, …)` helpers already accept an explicit path —
  prefer those in tests; reserve the `home_dir()`-resolving public functions for
  the error-path tests that reject *before* I/O.
- Add a regression guard test asserting the public writers are never exercised
  with a clean path against an unsandboxed home.
- Clean up the leftover empty `/tmp/.tmp*` Android SDK temp dir(s): confirm the
  android-install tests use `TempDir` (auto-removed on drop) and that
  `relocate_cmdline_tools` / the android temp handling never leaves an empty
  `sdk_root` behind on the real filesystem.

### Non-fix (documented) — zsh `~/.zshenv` vs `~/.zprofile`

If `~/.zshenv` is absent at write time, the writer targets `~/.zprofile`
(`path_config.rs:182-192`). This is intended behaviour, not part of this bug; note
it only so reviewers don't conflate "written to a different zsh file" with "not
written."

---

## Affected Modules

| Crate | File | Change |
|---|---|---|
| `fdemon-app` | `handler/install_wizard/actions.rs` | Emit auto-config follow-up on FlutterSdk/AndroidTools completion |
| `fdemon-app` | `message.rs` | New `InstallWizardAutoConfigurePath { kind }` variant |
| `fdemon-app` | `handler/update.rs` | Wire the new message |
| `fdemon-app` | `actions/mod.rs` | (Possibly) scope `android_sdk_root` per auto-config origin |
| `fdemon-daemon` | `toolchain/path_config.rs` | (Fix 2) replace stale-target fence; (Fix 3) injectable home seam + test sandboxing |
| `fdemon-daemon` | `toolchain/android_install.rs` | (Fix 3) verify no empty `sdk_root` temp leak; test `TempDir` hygiene |
| docs | `docs/ARCHITECTURE.md` | Document the auto-PATH-config chain (→ `doc_maintainer`) |

---

## Edge Cases & Risks

- **Auto-chain loops / seq clobber.** The auto-config step runs while the wizard is
  mid-chain; reuse the Phase-7 `run_seq`/`install_task` seq-guard to ensure the
  auto-started PathConfig cannot be clobbered and cannot re-trigger the installer.
- **Unsupported shell.** If `$SHELL` is unset or exotic, `HostShell::Unknown` makes
  `add_to_path` error. Auto-config will surface a `WizardStepFailed` for PathConfig
  — acceptable, and arguably better than today's silent no-write, but ensure the
  Flutter install itself is still reported successful.
- **Headless mode.** Confirm auto-config behaves (or is intentionally skipped) when
  the wizard runs headlessly.
- **Double-write of Android block.** If both FlutterSdk and AndroidTools auto-config
  touch the Android block, ensure idempotency (it is) and prefer scoping (Fix 1
  guardrail).

---

## Success Criteria

- [ ] After the wizard installs Flutter, `~/.zshenv` (or the correct rc file) gains
      a Flutter fence block pointing at the **real** installed `<sdk>/bin`, with **no
      manual PathConfig step required**.
- [ ] A pre-existing stale fdemon Flutter fence block is replaced with the correct
      path on the next install (no duplicate blocks).
- [ ] After the wizard installs Android tools, `ANDROID_HOME` + Android `PATH` are
      written automatically.
- [ ] The test suite cannot write to a developer's real `~/.zshenv` / `~/.zprofile`;
      a regression guard enforces this. No leftover empty `/tmp/.tmp*` SDK dirs.
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      all pass; no regressions.

---

## Open Questions (for approval before task breakdown)

1. **Auto-config scope:** auto-run PathConfig after **both** FlutterSdk *and*
   AndroidTools (recommended), or Flutter only for now?
2. **Android block on Flutter auto-config:** write Flutter PATH only (recommended)
   vs. also write the Android block when a root is discoverable?
3. **Fix 2 (stale-target self-heal):** include now, or defer (Fix 1 covers the
   common case)?
4. **Fix 3 (test sandboxing):** include in this bug's scope (recommended — it is the
   likely source of the reported artifact) or split into its own hygiene task?
