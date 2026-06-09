# Phase 3 Followup — Web leaf review findings — Task Index

## Overview

Addresses the findings from the Phase 3 code review
(`workflow/reviews/features/toolchain-platforms-submenu-phase-3/{REVIEW.md,ACTION_ITEMS.md}`, verdict
**⚠️ NEEDS WORK**). The review passed architecture, logic, and security cleanly; this followup closes the
**one blocking functional regression** plus the medium/low polish items.

**Headline issue (blocking):** the Phase-3 "Web never blocks" contract is enforced only inside the wizard
(`build_steps` caps `Missing → Partial`). The `fdemon doctor` CLI reads the **raw** preflight report and
gates its exit code with `gates = true` for every non-Android component, so a `Missing` `WebBrowser` makes
`fdemon doctor` exit `1` on browser-less hosts (CI containers, headless servers) — exactly what Phase 3
set out not to block. Task 01 fixes this; the rest harden detection testability, guided-command accuracy,
docs, and forward-compat.

### Source findings → tasks

| Review item | Severity | Task |
|-------------|----------|------|
| A — `fdemon doctor` exits 1 on missing browser | 🔴 MAJOR (blocking) | 01 |
| B — macOS/Windows detection arms untested on Linux CI | 🟡 MEDIUM | 02 |
| E — `probe_version(&PathBuf)` → `&Path` | 🟢 LOW | 02 |
| F — tautological test assertion | 🟢 LOW | 02 |
| G — non-`#[serial]` tests read global `CHROME_EXECUTABLE` | 🟢 LOW | 02 |
| H — `probe_version` 10s timeout over-generous | 🟢 LOW | 02 |
| C — guided-command distro/tool drift (apt/snap, Debian, pacman AUR, winget) | 🟡 MEDIUM | 03 |
| K — duplicated `CHROME_EXECUTABLE` note suffix | 🟢 LOW | 03 |
| D — `web_browser_executable` source doc comment inaccurate | 🟡 MEDIUM | 04 |
| J — `web_browser_executable` unvalidated free-form string (optional) | 🟢 LOW | 04 (optional) |
| I — fixed count/index assertion is a Phase-4 tripwire | 🟢 LOW | 05 |
| (A doc) — `fdemon doctor` gating entry in ARCHITECTURE.md | doc | 06 |
| M — `step_caption(PlatformWeb)` always `Some` (cosmetic) | ⚪ optional | Deferred (see Notes) |
| K (dismissed) — `step_detail.rs:2116` "malformed comment" | — | False positive — no action |

**Total Tasks:** 6
**Estimated Hours:** 6–9 hours

## Task Dependency Graph

```
   ┌───────────────────────────────────────────────────────────────────────┐
   │                       Wave 1 (all parallel worktrees — write-disjoint)  │
   │                                                                         │
   │ ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────────────┐ │
   │ │ 01 doctor gating  │ │ 02 web.rs harden  │ │ 03 guided-cmd robustness │ │
   │ │ (BLOCKING)        │ │ (daemon checks)   │ │ (app state.rs)           │ │
   │ │ src/doctor.rs     │ │ checks/web.rs     │ │ install_wizard/state.rs  │ │
   │ └────────┬─────────┘ └──────────────────┘ └──────────────────────────┘ │
   │          │           ┌──────────────────┐ ┌──────────────────────────┐ │
   │          │           │ 04 config doc     │ │ 05 count-assert fwd-compat│ │
   │          │           │ config/types.rs   │ │ toolchain/mod.rs          │ │
   │          │           └──────────────────┘ └──────────────────────────┘ │
   └──────────┼──────────────────────────────────────────────────────────────┘
              ▼                                                       Wave 2
   ┌────────────────────────────────────────┐
   │ 06 docs (doc_maintainer)                │
   │ docs/ARCHITECTURE.md — doctor gating    │
   └────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-doctor-web-non-gating](tasks/01-doctor-web-non-gating.md) | ✅ Done (validated, merged) | - | 1.5–2h | `src/doctor.rs` |
| 2 | [02-web-detection-hardening](tasks/02-web-detection-hardening.md) | ✅ Done (validated, merged) | - | 2–3h | `crates/fdemon-daemon/src/toolchain/checks/web.rs` |
| 3 | [03-guided-command-robustness](tasks/03-guided-command-robustness.md) | ✅ Done (re-impl after FAIL on AC2, validated, merged) | - | 1.5–2h | `crates/fdemon-app/src/install_wizard/state.rs` |
| 4 | [04-config-doc-accuracy](tasks/04-config-doc-accuracy.md) | ✅ Done (validated CONCERN: dropped stray settings.local.toml, merged) | - | 0.5h | `crates/fdemon-app/src/config/types.rs` |
| 5 | [05-count-assertion-forward-compat](tasks/05-count-assertion-forward-compat.md) | ✅ Done (validated, merged) | - | 0.5–1h | `crates/fdemon-daemon/src/toolchain/mod.rs` |
| 6 | [06-update-docs](tasks/06-update-docs.md) | ✅ Done (validated, committed) | 1 | 0.5h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `src/doctor.rs` | `crates/fdemon-daemon/src/toolchain/types.rs` (`ComponentKind`, `ComponentStatus`) |
| 02 | `crates/fdemon-daemon/src/toolchain/checks/web.rs` | `crates/fdemon-daemon/src/toolchain/checks/mod.rs` (`PROBE_TIMEOUT`, `strip_and_truncate`), `types.rs` |
| 03 | `crates/fdemon-app/src/install_wizard/state.rs` | `crates/fdemon-daemon/src/toolchain/types.rs` (`HostPlatform`, `LinuxPackageManager`, `ToolchainReport.winget_available`) |
| 04 | `crates/fdemon-app/src/config/types.rs` | `docs/CONFIGURATION.md` (for wording parity) |
| 05 | `crates/fdemon-daemon/src/toolchain/mod.rs` | `crates/fdemon-daemon/src/toolchain/types.rs` |
| 06 | `docs/ARCHITECTURE.md` | task 01 result, `~/.claude/skills/doc-standards/schemas.md` |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 + 03 + 04 + 05 | **none** (every task writes a different file) | **All parallel (worktree)** — Wave 1 |
| 02 + 05 | none (both daemon crate, different files: `checks/web.rs` vs `toolchain/mod.rs`) | Parallel (worktree) |
| 03 + 04 | none (both app crate, different files: `install_wizard/state.rs` vs `config/types.rs`) | Parallel (worktree) |
| 06 vs all | none | Sequential (after 01) |

> Wave 1 is maximally parallel: tasks 01–05 each own exactly one source file, in three different crates,
> with zero write overlap. Task 06 (`doc_maintainer`) documents the doctor contract change from Task 01,
> so it runs in Wave 2.

## Success Criteria

- [ ] `fdemon doctor` exits **0** when the only non-Ok component is a `Missing`/`Partial` `WebBrowser`
      (Flutter + Android healthy); `WebBrowser` is printed but never gates the exit code.
- [ ] The doctor gating decision is a pure, unit-tested helper (no reliance on the inline loop).
- [ ] macOS/Windows browser-detection candidate paths are extracted into testable units and covered by
      cross-host unit tests (no longer compiled-but-unexecuted on the Linux CI).
- [ ] `probe_version` takes `&Path`; uses a dedicated short timeout; the tautological override assertion is
      replaced; all `check_web` tests that read `CHROME_EXECUTABLE` are `#[serial]`.
- [ ] Guided commands lead with the cross-distro-robust `CHROME_EXECUTABLE` + download fallback; the
      Windows winget command is gated on `report.winget_available`; the repeated note suffix is a `const`.
- [ ] The `web_browser_executable` source doc comment no longer claims it "sets `CHROME_EXECUTABLE`"; it
      matches the corrected `.md` wording (probe-only; does not affect Flutter's own process).
- [ ] The daemon component-count test uses presence-based assertions (Phase-4 host-gating safe) or carries
      an explicit forward-pointer comment.
- [ ] `docs/ARCHITECTURE.md`'s `fdemon doctor` entry documents WebBrowser as non-gating.
- [ ] `cargo test --workspace --lib` green; `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Notes

- **Deferred (optional, finding M):** gating `step_caption(PlatformWeb)` on guided-command presence (so the
  "Browser required…" caption doesn't show when a browser is already detected). The logic reviewer rated
  this cosmetic and not misleading; defer unless a future polish pass wants caption/JDK symmetry. If picked
  up, it is a `step_detail.rs`-only change and would slot into Wave 1 as a 7th write-disjoint task.
- **Dismissed (false positive):** the code-quality reviewer alleged a malformed comment at
  `step_detail.rs:2116`; it is a valid `//` comment and the build is clean. No task.
- **Daemon raw `Missing` stays correct.** Do **not** change `check_web` to emit `Partial` — the daemon
  reports ground truth; non-blocking is a *consumer* policy. Task 01 makes the doctor consumer treat Web as
  non-gating (matching the wizard's leaf-local cap), keeping the two consumers consistent without moving the
  policy into the daemon.
- **Locate by symbol, not line** — line numbers are a current snapshot and will drift.
