# Bug: Windows Android step fails with "sdkmanager --licenses exited; the system cannot find the path specified"

**Reported:** 2026-06-10 (Windows 11 E2E VM, branch feat/toolchain-platforms-submenu)
**Status:** FIXED 2026-06-11 — fix-01 implemented + validated CONCERN (non-blocking: `java -version`
pre-validation uses the inherited env, not the sdkmanager env pairs — functionally equivalent since
java is invoked by absolute path and doesn't read JAVA_HOME; `output_tail` byte/char guard mismatch
harmless for ASCII sdkmanager output). Awaiting re-test on the Windows VM.
Diagnosis: 7-agent workflow `wf_7d96fe4c-dd8`, adversarially verified.
**Severity:** High on Windows (Android wizard step unusable when JDK resolution misfires); Linux unaffected

## Symptom

On the wizard's Android step, license acceptance fails with
`Flutter process error: sdkmanager --licenses exited with <status>; see log above for details`,
with cmd.exe's "The system cannot find the path specified." streamed into the step log.
Linux runs the same step successfully.

## Diagnosis (verified)

**Not a regression.** All files on this code path (`android_install.rs`, `process_stream.rs`,
`jdk.rs`, `path_config.rs`, `checks/android.rs`) have exactly one commit ever — the Phase 3
toolchain-bootstrap squash `c245f2e9` (2026-06-08) — and the Phase 6 merges are disjoint.

**Mechanism.** The spawn itself works (Rust std auto-wraps `.bat` in `cmd.exe /e:ON /v:OFF /d /c`
since the CVE-2024-24576 hardening — verified against std source; the "exited with {status}" error
string is only reachable AFTER a successful spawn). `sdkmanager.bat` runs and cmd.exe prints
"The system cannot find the path specified." while expanding `%JAVA_HOME%\bin\java.exe` — i.e. the
JDK home fdemon injected via `build_sdkmanager_env` does not resolve to a working JDK in the VM.

**Contributing defects found in the chain:**

1. `jdk.rs:248` (`java_home_from_which`): the JDK marker check is POSIX-only —
   `jdk_home.join("bin").join("javac").exists()` — so on Windows the `which java` fallback can
   only accept a home via the `release` file and silently rejects valid JDKs otherwise
   (`validate_jdk_home` at jdk.rs:96-98 is already `.exe`-aware; this call site was missed).
2. `android_install.rs:385-388`: the licenses failure says "see log above for details" but the
   `WizardStepFailed` reason (what the user actually sees) never carries the bat's real error —
   the collected `log_lines` are discarded.
3. No pre-validation of the resolved JDK: a stale `[toolchain] jdk_path` / bad `JAVA_HOME` is only
   discovered deep inside `sdkmanager.bat` with a cryptic cmd error instead of a clear
   "this JDK path cannot execute java" message before the run.

## Fix

Single task: [tasks/fix-01-jdk-windows-marker-and-error-surfacing.md](tasks/fix-01-jdk-windows-marker-and-error-surfacing.md)

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read |
|------|----------------------|------------|
| fix-01 | `crates/fdemon-daemon/src/toolchain/jdk.rs`, `crates/fdemon-daemon/src/toolchain/android_install.rs` | `toolchain/process_stream.rs`, `toolchain/mod.rs` |

Single task — no overlap; sequential on the current branch.
