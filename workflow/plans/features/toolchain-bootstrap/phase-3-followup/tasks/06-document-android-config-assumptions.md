## Task: Document Android config assumptions (n7)

**Objective**: Document the two implicit Android-install assumptions in
`docs/CONFIGURATION.md` so users have an escape hatch when they bite: the
`build-tools;<api>.0.0` patch-version assumption and the hardcoded
`DEFAULT_CMDLINE_TOOLS_BUILD` (which goes stale when Google rotates `_latest.zip`).

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 0.5 hours

### Background (verified)

- `crates/fdemon-daemon/src/toolchain/types.rs` — `sdkmanager_packages(api)` hardcodes
  `build-tools;{api}.0.0`. If a future API ships build-tools as `.0.1` first (or `.0.0` is
  withdrawn), `sdkmanager` returns "package not found" → `WizardStepFailed`. The only
  in-product knob today is `android_api_level` (which also moves `platforms;android-<api>`).
- `DEFAULT_CMDLINE_TOOLS_BUILD` is a hardcoded build number; Google publishes no stable
  build-less URL, so it can 404 when the published build rotates. `[toolchain] cmdline_tools_build`
  overrides it — but a fresh-machine user only discovers this after a cryptic 404.

`docs/CONFIGURATION.md` is an unmanaged doc (implementor may edit directly).

### Scope

**Files Modified (Write):**
- `docs/CONFIGURATION.md` — in the existing `[toolchain]` section:
  - Note that `cmdline_tools_build` overrides the shipped default and **when to set it**
    (the install 404s because the default build rotated; point users to
    `https://developer.android.com/studio#command-tools` for the current build number).
  - Note the `build-tools;<api>.0.0` assumption tied to `android_api_level`, and that a
    "package not found" install error means the configured API's `.0.0` build-tools isn't
    published — adjust `android_api_level` accordingly.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs` (confirm the exact package strings + default
  build constant value so the doc matches the code).

### Acceptance Criteria

1. `docs/CONFIGURATION.md` `[toolchain]` section documents both assumptions with the
   override/remediation guidance above.
2. Values quoted (default API level, package naming) match `ToolchainSettings` /
   `sdkmanager_packages` exactly.
3. Edits are confined to the `[toolchain]` section; no unrelated doc changes.

### Notes

- Documentation-only task — no code changes, no tests.
- Do not duplicate content already added by Phase 3 task 10; extend it.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `docs/CONFIGURATION.md` | Added "Android install assumptions" subsection inside `[toolchain]` section with remediation guidance for both the `build-tools;<api>.0.0` patch-version assumption and the `cmdline_tools_build` 404 scenario |

### Notable Decisions/Tradeoffs

1. **Subsection placement**: Added as an `####` subsection after the property reference table, before `### Editor Settings`. This keeps it inside the `[toolchain]` section without duplicating the one-liner descriptions already in the table.
2. **Values verified against source**: Default build number `11076708` from `DEFAULT_CMDLINE_TOOLS_BUILD` in `types.rs` (line 377); `build-tools;36.0.0` pattern from `sdkmanager_packages()` (line 459); URL structure from `cmdline_tools_url()` (lines 435-437). All match exactly.
3. **No duplication**: The table already has brief mentions of both fields; the new subsection extends them with failure scenarios and remediation steps rather than restating the same content.

### Testing Performed

- Manual review of `crates/fdemon-daemon/src/toolchain/types.rs` to confirm all quoted values match the source constants and functions.
- Documentation-only task — no build or test commands applicable.

### Risks/Limitations

1. **Default build number goes stale**: The `11076708` value in the doc will itself become stale when Google rotates the build. The doc now instructs users where to find the current value, which mitigates this.
