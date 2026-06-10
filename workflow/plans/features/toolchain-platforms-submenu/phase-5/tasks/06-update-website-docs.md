## Task: Website docs — rewrite the toolchain page for the Platforms submenu

**Objective**: Perform the website rewrite deferred since Phase 2: update
`website/src/pages/docs/toolchain.rs` so it describes the shipped wizard — the Platforms submenu with
all five leaves (Android, iOS, macOS, Web, Windows) — instead of the legacy flat "Android Tools" step.

**Depends on**: Tasks 01–04 (merged — the Windows leaf is the last to carry content; Phases 2–4
explicitly parked this rewrite until now).

**Agent:** implementor

**Complexity:** medium

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `website/src/pages/docs/toolchain.rs` (549 lines; Leptos view! prose components)

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/state.rs` — authoritative leaf titles, statuses, guided-command
  builders (`web_browser_guided_commands`, `xcode_guided_commands`, `windows_guided_commands`).
- `crates/fdemon-daemon/src/toolchain/checks/{web,ios,windows}.rs` — what each probe actually checks.
- Phase 2–5 TASKS.md — submenu semantics (expand/collapse, host gating, non-blocking rollup).
- Other pages under `website/src/pages/docs/` for component/idiom conventions.

### Details

The page is currently a Phase-1-era snapshot: "five ordered steps", a step table whose row 2 is
`2. Android Tools`, an ASCII mock showing `✖ Android Tools`, and a checks table with no Web/Xcode/VS
rows. Rewrite the affected sections; preserve the page's component structure, tone, and styling
classes.

1. **Meta description + intro** (~lines 11–34): mention platform setup beyond Android — the wizard
   diagnoses Android, iOS/macOS (Xcode/CocoaPods), Web (browser), and Windows (Visual Studio C++)
   and guides what it can't auto-install.
2. **"The Five Steps" section** (~line 70): still five top-level steps —
   `Prerequisites → Platforms → Flutter SDK → PATH → Doctor` — but step 2 is now the expandable
   **Platforms** row. Update:
   - the ASCII mock: `▸/▾ Platforms` parent with indented leaves (host-gated note), statuses;
   - the numbered table row 2: Platforms — expandable submenu; Android = managed auto-install
     (JDK-gated, unchanged); iOS/macOS/Web/Windows = detect + guided copy-paste commands;
   - the "Step order vs. install order" note (~line 147) for the current order/wording.
3. **Checks table** (~line 197): add rows — Web browser (`CHROME_EXECUTABLE` → default paths →
   Edge), Xcode + CocoaPods (macOS hosts; full Xcode, not CLT), Visual Studio C++ workload
   (Windows hosts; `vswhere` two-gate). Note host-gating: rows appear only on the relevant host.
4. **"Android Toolchain" section** (~line 254): reframe as the Android *leaf* under Platforms
   (content otherwise intact), and add a sibling subsection (or compact table) for the guided-only
   leaves summarizing each platform's guided commands: Web (`CHROME_EXECUTABLE` export / browser
   install), iOS/macOS (Xcode App Store, `xcode-select -s`, `-runFirstLaunch`, license,
   `-downloadPlatform iOS`, `brew install cocoapods`), Windows (winget/choco Build Tools with the
   `NativeDesktop` workload, modify-existing-VS hint). Mention the `r` re-check, `c` copy, `[`/`]`
   cycle keys and the non-blocking (Partial, never Missing) semantics.
5. **Keys/footer prose**: ensure `Enter`-on-Platforms expand/collapse is mentioned wherever the page
   lists wizard keys.
6. Mention `web_browser_executable` in the config-related prose if the page covers toolchain config
   (it feeds the Web check), linking to the existing configuration page if one is referenced.

### Acceptance Criteria

1. No stale references remain: "Android Tools" as a step name, the old step order, or a 9-component
   checks list (`grep -in "android tools" website/src/pages/docs/toolchain.rs` is clean apart from
   legitimate prose about the Android leaf's tooling).
2. The five-leaf submenu, host gating, guided-only model, and non-blocking semantics are described and
   match the shipped code (verify commands against the builders in `state.rs` — copy them verbatim).
3. The website builds: `cargo check -p website` (or the project's website build command — check
   `website/README`/`Trunk.toml` and use what exists).
4. `cargo fmt --all` clean.

### Testing

```bash
cargo check -p website
cargo fmt --all
grep -in "android tools\|five ordered" website/src/pages/docs/toolchain.rs
```

### Notes

- This is the wrap-up of the deferral noted in Phase 2/3/4 TASKS.md ("website docs remain deferred to
  the Phase-5 wrap-up docs task").
- Phase 6 (version picker) is **not** shipped — do not document it; the Flutter SDK step prose stays
  latest-stable.
- Runs in parallel with Task 05 (ARCHITECTURE.md) — write-disjoint.
- Escape-sequence conventions: the file uses `\u{2014}`-style escapes in string literals — follow them.
