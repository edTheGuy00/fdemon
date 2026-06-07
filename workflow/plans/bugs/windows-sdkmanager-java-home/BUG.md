# Bug: Android sdkmanager fails on Windows — JAVA_HOME not propagated to the child

## Status

🔬 **Investigated — root cause confirmed (pending one live diagnostic in the VM).**
Awaiting approval of the fix before the task breakdown.

Found testing the install wizard on the real Windows 11 VM (`tests/docker/windows/`).
Relates to [`workflow/plans/features/toolchain-bootstrap/PLAN.md`](../../features/toolchain-bootstrap/PLAN.md)
Phase 3 (Android cmdline-tools + sdkmanager license acceptance).

## Symptom

On Windows 11, the Android Tools step **downloads** the cmdline-tools zip fine, then
**immediately fails at license acceptance**:

```
Flutter process error: sdkmanager --licenses exited with … The system cannot find the path specified.
```

## Root Cause (CONFIRMED in code; one live check to confirm the ambient state)

The error is emitted **inside `sdkmanager.bat`**, not by fdemon failing to find it.
fdemon's path construction is correct: `sdkmanager_path()` builds the absolute path
`<sdk_root>\cmdline-tools\latest\bin\sdkmanager.bat` using `sdkmanager_bin_name()`,
which returns `"sdkmanager.bat"` under `cfg!(target_os = "windows")`
(`checks/android.rs:179-185`, `android_install.rs:576-582`). Our cross-compiled
binary targets `x86_64-pc-windows-gnu`, so `cfg!(target_os="windows")` is **true** —
the `.bat` name is used, and Rust ≥1.77.2 spawns a `.bat` by absolute path correctly
(same mechanism as `FlutterExecutable::WindowsBatch`, `flutter_sdk/types.rs:54-95`).

`sdkmanager.bat` (a Gradle-style launcher) resolves Java like this:

```batch
if defined JAVA_HOME goto findJavaFromJavaHome
set JAVA_EXE=java.exe            &  rem  PATH fallback when JAVA_HOME unset
...
:findJavaFromJavaHome
set JAVA_HOME=%JAVA_HOME:"=%
set JAVA_EXE=%JAVA_HOME%/bin/java.exe
if exist "%JAVA_EXE%" goto init
... echo ERROR: JAVA_HOME is set to an invalid directory ...
:execute
"%JAVA_EXE%" ... com.android.sdklib.tool.sdkmanager.SdkManagerCli %CMD_LINE_ARGS%
```

**"The system cannot find the path specified"** is the cmd.exe/CreateProcess error
when `%JAVA_HOME%\bin\java.exe` is structurally invalid — i.e. `JAVA_HOME` points at
a non-existent dir, at the `…\bin` subdir, has a trailing `\` (yielding `…\/bin/…`),
or carries stray quotes. (Sources: sdkmanager.bat launcher source; TheServerSide
JAVA_HOME guide; Flutter #53648.)

**The fdemon defect:** fdemon sets `JAVA_HOME` (and prepends the JDK `bin` to the
child PATH) for the sdkmanager child **only when `target.jdk_path` is `Some`**
(`android_install.rs:337-371`):

```rust
let java_home_str = target.jdk_path.as_ref().map(|p| p.to_string_lossy().into_owned());
let mut env_pairs = vec![("ANDROID_HOME", sdk_root_str)];
if let Some(java_home) = java_home_str {           // <-- only when jdk_path is Some
    env_pairs.push(("JAVA_HOME", java_home));
    // …prepend <java_home>\bin to PATH…
}
```

`target.jdk_path` comes from `settings.toolchain.jdk_path` (`actions.rs:207`), which
is **`None` in the normal flow** — the user installs a JDK via the guided command but
never sets `[toolchain] jdk_path` in config. So fdemon passes **no `JAVA_HOME`** and
**doesn't add the JDK to the child PATH**, leaving `sdkmanager.bat` at the mercy of
the **ambient** Windows `JAVA_HOME` — which is frequently unset, stale (the same
process-env staleness as the git bug — a JDK installed after fdemon launched isn't on
fdemon's PATH), or malformed. fdemon already owns `resolve_jdk_home()` (jdk.rs:30) —
the exact resolver the JDK preflight *check* uses — but the **installer never falls
back to it.** (This mirrors the PathConfig fix where the executor falls back to
`resolve_android_sdk_root_path(None)` when `settings.android_sdk_root` is unset — the
Android installer needs the same fallback for the JDK.)

### Live confirmation (run in the VM)

```powershell
$env:JAVA_HOME ; Test-Path "$env:JAVA_HOME\bin\java.exe"   # empty / False == smoking gun
$sm = gci C:\ -Recurse -Filter sdkmanager.bat -ea SilentlyContinue | select -First1 -Expand FullName
& $sm --version                                            # reproduces the real error
where.exe java                                             # check for a Store java shim
```

## Evidence Map

| Finding | Location |
|---|---|
| sdkmanager path correct (`sdkmanager.bat` on Windows, absolute) | `checks/android.rs:179-185`; `android_install.rs:576-582` |
| `.bat`-by-absolute-path spawn is supported (Flutter precedent) | `flutter_sdk/types.rs:54-95` |
| JAVA_HOME/PATH set on child **only if `target.jdk_path` is Some** | `android_install.rs:337-371` |
| `jdk_path` sourced from `settings.toolchain.jdk_path` (None by default) | `handler/install_wizard/actions.rs:207` |
| `resolve_jdk_home()` exists but installer never calls it as a fallback | `toolchain/jdk.rs:30-47` |
| sdkmanager.bat Java resolution → "path specified" on bad JAVA_HOME | external research (launcher source; Flutter #53648) |

## Proposed Fix

**Make the Android installer guarantee a valid `JAVA_HOME` for the sdkmanager child,
independent of whether `[toolchain] jdk_path` was configured.**

1. In `android_install.rs`, when assembling the sdkmanager child env, resolve the JDK
   home with this precedence:
   - `target.jdk_path` (explicit config), else
   - **`resolve_jdk_home()`** (env `JAVA_HOME`, then walk from `which java`) — the
     fallback that is currently missing.
2. **Validate & normalize** the chosen JDK home before using it (per research):
   - strip surrounding quotes and any trailing `\` / `/`;
   - require the directory to exist **and** contain `bin\java.exe` (Windows) /
     `bin/java` (POSIX) — and ideally `bin\javac[.exe]` to confirm it's a JDK, not a
     JRE;
   - if validation fails, **fail the step with a clear, actionable message** (point
     the user to set `[toolchain] jdk_path` or fix `JAVA_HOME`) instead of letting
     sdkmanager emit the cryptic "path specified".
3. Always set `JAVA_HOME` to the validated home **and** prepend `<home>\bin` to the
   child PATH (already done for the `Some` case — just make it run for the resolved
   case too). Use the OS-correct separator (already via `split_paths`/`join_paths`).
4. **Defense-in-depth:** before spawning, check `sdkmanager_path(&sdk_root).is_file()`
   and, on failure, list `cmdline-tools\latest\bin\` in the error — so a future
   layout/relocation problem yields a precise message rather than "path specified".
5. Consider a `cfg(windows)` note: the same JDK staleness as the git re-check applies;
   the `resolve_jdk_home()` fallback reads the *current* env, and the preflight PATH
   refresh (the just-merged Windows fix) means `which java` now sees a JDK installed
   after launch — so the fallback will actually find it on a re-run.

### Why this is the right fix

- It removes the dependency on a correct *ambient* `JAVA_HOME`, which is exactly what
  breaks on a fresh Windows box.
- It reuses `resolve_jdk_home()` (no new resolution logic) and mirrors the established
  "fall back to the resolver when settings is None" pattern from PathConfig.
- The validation converts the cryptic Windows error into an actionable one.

## Affected Modules

| Crate | File | Change |
|---|---|---|
| `fdemon-daemon` | `toolchain/android_install.rs` | resolve+validate JAVA_HOME (fallback to `resolve_jdk_home()`); always set JAVA_HOME + child PATH; pre-spawn sdkmanager existence check + better errors |
| `fdemon-daemon` | `toolchain/jdk.rs` | (maybe) a small `validate_jdk_home()` / Windows `bin\java.exe` check helper, reused by the installer |
| docs | `docs/ARCHITECTURE.md` | note the installer's JDK-home fallback + validation (→ `doc_maintainer`) |

## Verification

- Unit: JDK-home precedence (explicit > resolver) and `validate_jdk_home` (rejects
  `…\bin`, trailing slash, missing `bin/java`, JRE-only); Windows-gated where needed.
- **E2E (authoritative), `tests/docker/windows/`:** install a JDK, run the Android
  Tools step → license acceptance now succeeds (sdkmanager gets a valid JAVA_HOME);
  and with a deliberately-broken ambient `JAVA_HOME`, fdemon surfaces a clear error
  rather than "path specified".

## Success Criteria

- [ ] On Windows, the Android Tools step accepts licenses successfully when a JDK is
      installed, **without** requiring `[toolchain] jdk_path` to be set manually.
- [ ] The sdkmanager child always receives a validated `JAVA_HOME` + JDK `bin` on PATH
      (resolved via `target.jdk_path` → `resolve_jdk_home()`).
- [ ] A missing/invalid JDK home produces a clear, actionable error (not "The system
      cannot find the path specified").
- [ ] POSIX behaviour unchanged/working; no regression.
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      pass; verified E2E in the Windows VM.

## Open Questions (for approval)

1. **JRE rejection strictness:** require `bin\javac` (true JDK) or accept any dir with
   `bin\java`? (Recommend require `javac` — sdkmanager needs a JDK.)
2. **Failure vs. best-effort:** if no valid JDK home can be resolved, **fail the step
   with guidance** (recommended) vs. attempt sdkmanager with bare PATH java and hope?
