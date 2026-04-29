# Bugfix Plan: `flutter devices` fails on Windows (`The system cannot find the path specified`)

## TL;DR

Two Windows users have reported the same hard-fail at startup:

> `Flutter process error: flutter devices failed with exit code Some(1): The system cannot find the path specified.`

GitHub issues:
- https://github.com/edTheGuy00/fdemon/issues/32 (`maxkabechani`, fdemon 0.4.0, Windows 10)
- https://github.com/edTheGuy00/fdemon/issues/34 (`Far-Se`, Flutter at `E:\flutter`, project on `E:\Projects\Side Projects\colorpicker`)

The reporter in #32 hypothesised that fdemon was calling `Command::new("flutter")` directly, which would fail on Windows because Rust's `CreateProcessW` does not consult `PATHEXT` to find `flutter.bat`. **That hypothesis is partially right but does not match what fdemon actually does today** — fdemon already has a `FlutterExecutable::WindowsBatch` variant that wraps invocations in `cmd /c <abs-path-to-flutter.bat>` (`crates/fdemon-daemon/src/flutter_sdk/types.rs:74-83`).

The most likely *actual* root cause is one (or a combination) of three Windows-specific problems in the spawn path:

1. **`cmd /c <path>` quote-stripping** — when the resolved `flutter.bat` path contains spaces (e.g. `C:\Program Files\flutter\bin\flutter.bat` or any path under `C:\Users\First Last\…`), `cmd.exe` mis-parses the command line and prints "The system cannot find the path specified." This is a long-standing cmd quirk (the `/c` quote rules require an empty leading `""` to preserve quotes around the program path). Our wrapper does not insert that empty string.
2. **SDK locator silently picks a wrong root for shim-style installs** (Chocolatey, scoop, winget) — those installers place `flutter.bat` at `C:\ProgramData\chocolatey\bin\flutter.bat` (a forwarder shim, not inside an SDK tree). `find_flutter_in_dir` locates the shim, walks two parents, and produces a non-SDK path. `validate_sdk_path` then fails (no `bin/cache/dart-sdk`, no `VERSION`) and the locator returns `Err(FlutterNotFound)` — which would surface as a *different* error than the one reported. So shim layouts are unlikely to be the failure mode for #32/#34, but they remain a latent bug that this fix should address.
3. **The manual `cmd /c` wrapper is now redundant on Rust 1.77.2+** — since CVE-2024-24576, `std::process::Command` correctly handles direct invocation of `.bat` / `.cmd` files when the program path has an explicit extension. Removing the wrapper and invoking the `.bat` directly is the modern, correct pattern and avoids the cmd quote-parsing trap entirely.

**Fix (recommended):** Replace the bespoke PATH walker + `WindowsBatch(cmd /c …)` shim with the [`which`](https://crates.io/crates/which) crate (which respects `PATHEXT` so it finds `.bat`/`.cmd`/`.exe` on Windows) and call `Command::new(absolute_path_to_flutter.bat)` directly. The stdlib then takes care of the cmd-escape correctly. As a bonus, this also handles shim-style installs (the locator stops needing to find a "valid" SDK root when all it needs is a working executable path).

We cannot reproduce on macOS, so validation depends on (a) adding a Windows GitHub Actions runner that builds + smoke-tests the binary, and (b) sending a test build to the reporters of #32 and #34.

---

## Bug Report

### Symptom

Both reporters see the same error at startup, before any UI is drawn:

> `Flutter process error: flutter devices failed with exit code Some(1): The system cannot find the path specified.`

In both cases:
- Running `flutter devices` (or `flutter devices -v`, `flutter run --machine`) directly from the terminal works.
- The reporters were running fdemon **0.4.0**.
- #34 confirms the project loads fine in `dashmonx` (a different Dart-based tool), so the Flutter install itself is healthy.

### Environments

| Reporter | OS | Flutter install | Project path | PATH-relevant detail |
|---|---|---|---|---|
| `maxkabechani` (#32) | Windows 10 (build 26200.8037) | not stated | not stated | hypothesis: `flutter.bat` on PATH |
| `Far-Se` (#34) | Windows 10 (19045.6466) | `E:\flutter\bin\flutter.bat` | `E:\Projects\Side Projects\colorpicker` (note the **space** in `Side Projects`) | flutter and adb on PATH; Android SDK at `E:\AppData\Android\Sdk` |

`Far-Se`'s scenario is especially suggestive — the project path contains a space, the Flutter SDK is on a non-system drive (`E:`), and the user installed `flutter` to a custom location. Any of these can interact badly with `cmd /c` quoting.

### Where the error string comes from

`crates/fdemon-daemon/src/devices.rs:215-219`:

```rust
return Err(Error::process(format!(
    "flutter devices failed with exit code {:?}: {}",
    output.status.code(),
    stderr
)));
```

This branch is reached when `tokio::process::Command::output()` succeeded (the spawn worked, a process ran), the process exited non-zero, **and** stdout did not contain a JSON device array. The stderr text quoted by both users — "The system cannot find the path specified." — is a `cmd.exe` / Windows-shell error, not a Flutter framework error. It originates from a path-resolution failure inside the `cmd /c` invocation.

That rules out:
- `Error::FlutterNotFound` (would happen if spawn returned `ErrorKind::NotFound`).
- `Failed to run flutter devices` (other spawn errors).
- A timeout (would say "Device discovery timed out").

It is consistent with: cmd was invoked, cmd tried to resolve some path it was given, and that resolution failed.

---

## Root Cause Analysis

### Code path on Windows today

```
fdemon main
  └── Engine::new()                                               crates/fdemon-app/src/engine.rs:189
        └── flutter_sdk::find_flutter_sdk()                       crates/fdemon-daemon/src/flutter_sdk/locator.rs:46
              ├── strategy 1-9: explicit / env / version-managers (largely Unix-shaped)
              ├── strategy 10: try_system_path() ──► find_flutter_in_dir()
              │     └── Windows branch: tries `flutter.bat`, then `flutter.exe` in each PATH dir
              │           └── resolve_sdk_root_from_binary(): canonicalize + walk up 2 parents
              └── validate_sdk_path(root) → FlutterExecutable::WindowsBatch(<root>/bin/flutter.bat)
                                            crates/fdemon-daemon/src/flutter_sdk/types.rs:134-178

… later …

DiscoverDevices action
  └── discover_devices(&flutter_executable)                       crates/fdemon-daemon/src/devices.rs:141
        └── run_flutter_devices(flutter)                          crates/fdemon-daemon/src/devices.rs:180
              └── flutter.command()                               crates/fdemon-daemon/src/flutter_sdk/types.rs:74
                    └── tokio::process::Command::new("cmd")
                          .args(["/c", &*path.to_string_lossy()])  ← absolute flutter.bat path
                          .args(["devices", "--machine"])
                          .output().await
```

### Hypothesis 1 (most likely): `cmd /c "<path with spaces>" devices --machine` mis-parses

When the resolved absolute path contains whitespace, `tokio::process::Command` quotes it on the Windows command line:

```
cmd /c "C:\Program Files\flutter\bin\flutter.bat" devices --machine
```

`cmd /c`'s quote-handling rule (see `cmd /?`, "If /S is specified or /C or /K is specified") preserves quotes only when **all** of these hold:
- exactly two quote characters;
- no special characters (`&<>()@^|`) between them;
- whitespace between them;
- the quoted string is the name of an existing executable file.

If any precondition fails, cmd falls back to "strip the leading and trailing quote and run the rest verbatim", which produces:

```
C:\Program Files\flutter\bin\flutter.bat devices --machine
```

cmd then tokenises by whitespace, treating `C:\Program` as the program and `Files\flutter\bin\flutter.bat` as an arg, and emits the exact stderr text our users see. The classic workaround is to insert an empty `""` before the quoted path so cmd's old-behaviour quote-strip path does not eat the meaningful quotes:

```
cmd /c "" "C:\Program Files\flutter\bin\flutter.bat" devices --machine
```

This is *exactly* the pattern that `npm`, `yarn`, and the Rust stdlib `Command` post-1.77.2 use internally. We do not insert it in `FlutterExecutable::command()`.

For `Far-Se`'s setup, the flutter path itself (`E:\flutter\bin\flutter.bat`) has no spaces, but two adjacent cases would still trigger the same failure:
- the `current_dir` may be `E:\Projects\Side Projects\colorpicker` (has a space); some `.bat` invocations interact with CWD;
- if the user's PATH contains entries with spaces ahead of `E:\flutter\bin`, `find_flutter_in_dir` could resolve `flutter.bat` from a wrong dir first.

We cannot fully confirm without seeing the user's `tracing` log output, but the symptom matches.

### Hypothesis 2 (latent, separate bug): shim installers (Chocolatey/scoop/winget)

`crates/fdemon-daemon/src/flutter_sdk/locator.rs:346-350`:

```rust
pub(crate) fn resolve_sdk_root_from_binary(binary_path: &Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(binary_path).ok()?;
    canonical.parent()?.parent().map(|p| p.to_path_buf())
}
```

Walking up two parents assumes the binary lives at `<root>/bin/flutter`. Chocolatey shims live at `C:\ProgramData\chocolatey\bin\flutter.bat`; walking up two parents yields `C:\ProgramData\chocolatey`, which is not a Flutter SDK. `validate_sdk_path` rejects it (no `bin/cache/dart-sdk`, no `VERSION` file). Strategy 10 fails. Strategy 11 (lenient) re-uses the same `try_system_path()` and therefore fails the same way.

End state: `Err(Error::FlutterNotFound)`. This presents as a *different* error message than the one reported (`SDK-dependent features will be unavailable`), so shim installs are probably **not** the immediate cause for #32/#34 — but the locator design will silently fail for these users and is worth fixing in the same change.

### Hypothesis 3: Symlink canonicalisation produces UNC paths

On Windows, `fs::canonicalize` returns `\\?\` UNC-prefixed paths. Walking two parents from `\\?\E:\flutter\bin\flutter.bat` gives `\\?\E:\flutter`. Subsequent `is_file()` / `Path::join` calls work on UNC paths, but downstream consumers that pass the path through `to_string_lossy()` into `cmd /c` may end up with `cmd /c \\?\E:\flutter\bin\flutter.bat …`, and cmd does not handle the `\\?\` prefix. This would surface as the same "system cannot find the path specified" stderr.

This is reported in upstream Rust as a known sharp edge ([rust-lang/rust#42869](https://github.com/rust-lang/rust/issues/42869)). The standard fix is to use [`dunce::canonicalize`](https://crates.io/crates/dunce) instead of `fs::canonicalize` so UNC prefixes are stripped when not required.

### Why the user's "use `cmd.exe`" hypothesis (in #32) doesn't quite fit

Issue #32 says:

> "When Rust spawns child processes on Windows it does not resolve `.bat` files the same way an interactive shell does, so `flutter` cannot be found even when it is correctly on the system PATH."

This describes plain `Command::new("flutter")` — which fdemon does not do in production code. We searched: every production use of `FlutterExecutable` flows through `find_flutter_sdk` → `WindowsBatch(absolute_path)` → `cmd /c <abs path>`. The bare-`"flutter"` form only appears in `#[cfg(test)]` modules (`process.rs:444`, `emulators.rs:586/611`, `devices.rs:640`) and never reaches Windows users.

So #32's reasoning is a generic recollection of the Rust-Windows-`.bat` problem, not a confirmed reading of fdemon source. The symptom is real; their guessed cause is approximate.

---

## Suspect / Affected Code Locations

| File | Lines | Why |
|------|-------|-----|
| `crates/fdemon-daemon/src/flutter_sdk/types.rs` | 74-83 | `FlutterExecutable::command()` builds `cmd /c <path>` without empty `""` shield — root of hypothesis 1 |
| `crates/fdemon-daemon/src/flutter_sdk/types.rs` | 134-207 | `validate_sdk_path` / `validate_sdk_path_lenient` use `#[cfg(target_os = "windows")]` to choose `flutter.bat`. Will need updating if we drop `WindowsBatch` |
| `crates/fdemon-daemon/src/flutter_sdk/locator.rs` | 304-350 | `try_system_path` / `find_flutter_in_dir` / `resolve_sdk_root_from_binary` — relevant to hypothesis 2 (shim installs) and hypothesis 3 (UNC prefixes); also reinvents `which`/`PATHEXT` |
| `crates/fdemon-daemon/src/devices.rs` | 180-220 | Where the user-visible error string is produced; we should also surface stderr to the user-visible startup banner more clearly |
| `crates/fdemon-daemon/src/process.rs` | 60-90 | `FlutterProcess::spawn_internal()` — also routes through `flutter.command()`, so it will benefit from the same fix |
| `crates/fdemon-daemon/src/flutter_sdk/version_probe.rs` | 29-45 | `probe_flutter_version()` — same code path |
| `crates/fdemon-app/src/engine.rs` | 198-213 | Where SDK resolution failure is downgraded from fatal to "warn and disable features". A clearer Windows-specific diagnostic (e.g. "We could not resolve a Flutter SDK from PATH; if you installed via Chocolatey/scoop, please set `[flutter] sdk_path` in `.fdemon/config.toml`") would help diagnose hypothesis 2 |

No file under `crates/fdemon-daemon/src/native_logs/` has a Windows backend — that is a separate gap and **not** in scope for this bug. (Confirmed: only `android.rs`, `macos.rs`, `ios.rs` exist.)

`adb` is invoked as `Command::new("adb")` in `crates/fdemon-daemon/src/native_logs/android.rs:102` without Windows-specific handling, but `adb.exe` is a real PE executable so `CreateProcessW` will find it via standard `.exe` resolution. **Not a bug**, but worth noting.

There is no `.github/workflows/` directory — the project has **no CI**, which is why this never surfaced before shipping.

---

## Proposed Fix

### Strategy

Adopt the [`which`](https://crates.io/crates/which) crate (8.0.x — MSRV 1.70, matches our minimum) and stop hand-rolling PATH walking + `cmd /c`. The crate respects `PATHEXT` on Windows and returns an absolute path with the correct extension.

When `Command::new` is given an absolute path that ends in `.bat` or `.cmd`, the Rust standard library (post-1.77.2, well above our MSRV) automatically and **safely** delegates to `cmd.exe` with correct argument escaping — including the empty-`""` quote-shield. We do not need to write that shim ourselves.

### High-level changes

1. **Add `which = "8"`** to the workspace `Cargo.toml` and to `crates/fdemon-daemon/Cargo.toml`.
2. **Replace `FlutterExecutable` with a single `PathBuf`** (or keep the enum for now but simplify both variants to `Command::new(path)`). The `WindowsBatch` `cmd /c …` wrapper is removed.
3. **Update `validate_sdk_path` / `validate_sdk_path_lenient`** to return a `PathBuf` (or a one-variant enum) that points at the correct absolute path with extension on each platform. Behaviour is unchanged on Unix.
4. **Update `try_system_path`** to delegate to `which::which("flutter")` on every platform. The current hand-rolled PATH walker becomes a fallback only. This fixes shim-style installs because we no longer try to derive an SDK root from the shim location — for strategy 10/11, having a working executable path is sufficient. We still attempt `resolve_sdk_root_from_binary` for *metadata* (version, channel) but tolerate failure (downgrade to `SdkSource::PathInferred`, which strategy 11 already handles).
5. **Use [`dunce::canonicalize`](https://crates.io/crates/dunce)** (or strip the `\\?\` prefix manually) when canonicalising paths that will later be handed to `cmd.exe` or surfaced to the user, to avoid hypothesis-3 UNC-prefix issues. (`dunce` is a tiny, single-purpose, zero-dep crate widely used in the Rust-on-Windows ecosystem.)
6. **Improve the Windows-specific error path:** if `flutter devices` exits non-zero on Windows, log the full resolved binary path and the exit code so we can debug without a Windows machine if a user files another bug.
7. **Add a basic Windows CI smoke test** (GitHub Actions, `windows-latest`) that at minimum runs `cargo check`, `cargo build`, and `cargo test --workspace`. We do not need a real Flutter install in CI — unit tests cover the path-resolution logic, and the build gate alone would have caught any future Windows-specific regression.

### Why `which` over the alternatives

| Approach | Verdict |
|---|---|
| **`which` crate** | Respects `PATHEXT`, handles `.bat`/`.cmd`/`.exe` uniformly, returns absolute path. Modern Rust then handles `.bat` invocation correctly. **Recommended.** |
| **Manually try `flutter.bat`, `flutter.cmd`, `flutter.exe` in order** | Reinvents `PATHEXT`, ignores user customisation, brittle. Not recommended. |
| **Keep `cmd /c` but add empty `""` shield** | Fixes hypothesis 1 but not 2 or 3. Smallest possible patch — defensible if we want a minimal-risk dot-release while planning a larger refactor. |
| **Wrap with `cmd.exe /C ...` everywhere with `raw_arg`** | Same downsides as keeping `cmd /c`. Adds `windows-process-extensions-raw-arg` complexity. |

### Argument-escaping caveats (post-CVE-2024-24576)

Rust ≥ 1.77.2 returns `Err(InvalidInput)` from `Command::spawn()` if it cannot safely escape arguments to a `.bat`/`.cmd` invocation. The arguments fdemon passes to flutter (`devices`, `--machine`, `run`, `--machine`, dart-define key/value pairs from `launch.toml`) are user-controlled in principle. We should:
- audit user-supplied args (especially dart-defines from `launch.toml`) for characters that could trip the escaper (`%`, `"`, `&`, `|`, `<`, `>`, `^`);
- if we hit `Err(InvalidInput)`, surface a clear "this dart-define value contains characters that cannot be safely passed to cmd.exe" diagnostic rather than the generic spawn failure.

This is a small, targeted addition — most users will not hit it.

---

## Validation Strategy (Without a Windows Machine)

Because we cannot run Windows locally, validation runs on three tracks in parallel:

### Track 1 — Unit tests

The path-resolution logic (`validate_sdk_path`, `try_system_path`, `find_flutter_in_dir`, `resolve_sdk_root_from_binary`) is currently exercised on Unix only. We will add tests guarded by `#[cfg(target_os = "windows")]` that:
- create a fake SDK tree under `tempfile::TempDir`;
- write a fake `flutter.bat` (just a `@echo` shim);
- prepend the temp `bin/` to `PATH` for the test;
- assert that `find_flutter_sdk` returns the expected absolute path;
- assert that `which::which("flutter")` returns the expected path with `.bat` extension;
- assert that `Command::new(returned_path).arg("/q").output()` succeeds (the shim echoes successfully).

These tests will only execute when CI runs on `windows-latest`.

### Track 2 — Windows GitHub Actions runner

Add `.github/workflows/ci.yml` (currently absent) with a `windows-latest` job that runs:
- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

This gives us ongoing protection. It will **not** catch end-to-end Flutter invocation failures (CI would need a Flutter SDK installed), but it will catch every category of bug we have evidence for.

### Track 3 — User-driven smoke test

After merging the fix, build a Windows binary via `cargo build --release --target x86_64-pc-windows-msvc` (cross-compiling from macOS using `cargo-zigbuild` or via the new Windows CI runner artifact). Post a comment on issues #32 and #34 inviting both reporters to run the build and report whether the error reproduces. Ask them, in the same comment, to attach the contents of `%TEMP%\fdemon\fdemon-*.log` so we have authoritative evidence either way (fdemon already writes structured `tracing` output to a temp file per `docs/DEVELOPMENT.md`).

If they report success, ship 0.4.2 and close both issues. If they report failure, the temp log will tell us which hypothesis (1 / 2 / 3) is actually firing, and we iterate.

### Track 4 — Diagnostic logging (lands in the same fix)

Before relying on the user, we should improve our own logs so we can diagnose remotely:
- log the resolved absolute path of `flutter.bat` (or whatever `which` returns) at `info!`;
- log the full constructed command line (program + args + cwd) at `debug!`;
- on non-zero exit, log stdout *and* stderr at `error!` (currently `debug!`-only when stderr is non-empty).

This costs almost nothing and turns "user pastes a screenshot" into "user pastes a log file."

---

## Risks / Open Questions

1. **We are not 100% sure which hypothesis is the immediate cause.** The fix addresses 1, 2, and 3 simultaneously, but we should still ask the reporters to share logs so we can confirm post-merge.
2. **`which` adds a dependency.** It's a tiny, well-maintained crate. Acceptable.
3. **Removing `WindowsBatch` is a breaking change to the public-ish `FlutterExecutable` enum** in `fdemon-daemon`. The enum is `pub` and re-exported. We should either:
   - keep the enum but simplify both variants to direct `Command::new(path)` (smallest API impact); or
   - replace it with a transparent newtype wrapper around `PathBuf` and bump `fdemon-daemon`'s minor version.
   Pick the first option unless there's a downstream consumer.
4. **`dunce::canonicalize` is a third-party crate.** Alternatively we can strip `\\?\` ourselves with three lines of code. Either is fine.
5. **CI on Windows will slow down PRs slightly.** `windows-latest` runners are 2× the cost of Linux. Acceptable for a project shipping to Windows users.
6. **There is still no Windows native-log backend** (`fdemon-daemon/src/native_logs/`). Out of scope here, but worth a follow-up issue: Windows app logs would come from ETW or app stdout. Track separately.
7. **Argument-escaping for user-controlled dart-defines** could surface new errors after the fix on Windows. Mitigation: validate dart-defines on load and emit a config-time warning.
8. **The locator's strategy 10/11 fallback chain is now redundant** with `which`. Resist the temptation to refactor everything in this PR — keep the diff focused on the bug, file a follow-up to consolidate.

---

## Tasks (high-level, not yet broken down)

Pending user approval of the plan, the implementation will split into roughly:

1. **Add `which` (and `dunce`) deps** — workspace `Cargo.toml`, `crates/fdemon-daemon/Cargo.toml`.
2. **Refactor `FlutterExecutable`** to drop the `cmd /c` wrapper; both variants invoke the absolute path directly.
3. **Rewrite `try_system_path` to use `which::which`** as the primary discovery path; keep the existing PATH walker as a Windows-specific fallback for environments where `PATHEXT` is misconfigured.
4. **Improve diagnostic logging** in `flutter_sdk/locator.rs`, `devices.rs`, `process.rs`, `version_probe.rs`.
5. **Add Windows-only unit tests** for the locator + executable-resolution paths.
6. **Add `.github/workflows/ci.yml`** with Linux + macOS + Windows jobs running fmt/check/test/clippy.
7. **Update docs** — `docs/ARCHITECTURE.md` (mention `which` + simplified executable handling) and `docs/DEVELOPMENT.md` (CI + Windows-test guidance). Routed to `doc_maintainer`.
8. **Comment on issues #32 and #34** asking reporters to validate a Windows build, and request log files.

A concrete `TASKS.md` will follow once the high-level plan is approved.
