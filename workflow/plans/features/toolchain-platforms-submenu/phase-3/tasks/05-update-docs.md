## Task: Document the live Web leaf + `web_browser_executable`

**Objective**: Update the managed core docs to reflect Phase 3: the `PlatformWeb` leaf is now a live
detect + guided-only step backed by `ComponentKind::WebBrowser`, with non-blocking semantics, and the new
`[toolchain] web_browser_executable` config field.

**Depends on**: Tasks 01, 02, 03, 04 (document the shipped behaviour, not the plan).

**Agent:** doc_maintainer

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — install_wizard + toolchain detection sections.
- `docs/CONFIGURATION.md` — `[toolchain] web_browser_executable`.

**Files Read (Dependencies):**
- All Phase-3 task files + the implemented diffs.
- `~/.claude/skills/doc-standards/schemas.md` — content-boundary schema for the managed docs.

### Details

#### `docs/ARCHITECTURE.md`

- In the `fdemon-daemon` toolchain description (the `toolchain/` module bullet and `checks/` entry): add
  `ComponentKind::WebBrowser` and the new `checks/web.rs` probe (`CHROME_EXECUTABLE` → default Chrome
  paths → Edge on Windows; cross-host; `Unknown` off-display). Note `run_preflight` now emits **10**
  components and accepts a `web_browser_executable` override.
- In the `install_wizard` description: graduate the Web leaf from "host-gated placeholder" to a live
  detect + guided-only step. Document the **non-blocking** contract: a missing browser surfaces as
  `Partial` (capped from the daemon's raw `Missing` at the Web leaf in `build_steps`), the Platforms
  parent rolls up to at most `Partial`, and handback (`flutter_now_live` / `close_wizard_and_dispatch_discovery`)
  is unaffected because it reads only `FlutterSdk`.
- Mention the override-plumbing path: `settings.toolchain.web_browser_executable` →
  `RunToolchainPreflight` → `run_preflight` → embedded `WebBrowser` component (the established
  pre-compute-in-preflight pattern; `build_steps` stays pure-on-report).

#### `docs/CONFIGURATION.md`

- Add `web_browser_executable` under the `[toolchain]` section: purpose (custom Chromium-based browser for
  `flutter run -d chrome` / the web doctor check; sets `CHROME_EXECUTABLE`), type (`Option<String>`,
  defaults unset), and an example. Clarify it is distinct from `[devtools] browser` (which opens the
  DevTools UI).

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` accurately describes the `WebBrowser` check, the 10-component preflight, the
   live Web leaf, and the non-blocking rollup — within the doc's content boundaries (no API tutorials, no
   config-key reference dumps that belong in CONFIGURATION.md).
2. `docs/CONFIGURATION.md` documents `[toolchain] web_browser_executable` with type, default, and example,
   and disambiguates it from `[devtools] browser`.
3. `doc-validate` (doc-standards skill) passes for both files.

### Notes

- **Website docs (`website/src/pages/docs/toolchain.rs`) are out of scope** — deferred to the Phase-5
  wrap-up docs task (per the Phase-2 TASKS.md note) so the Platforms prose is rewritten once iOS/macOS/
  Windows leaves carry content.
- **No new keybindings** in Phase 3 — Web reuses the existing `Enter` / `c` (copy) / `r` (re-check) keys,
  so `docs/KEYBINDINGS.md` needs no change. Confirm and skip it.
- Keep ARCHITECTURE.md structural and CONFIGURATION.md reference-style per the doc schemas; do not
  duplicate config-key details into ARCHITECTURE.md.
