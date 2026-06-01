## Task: Update Documentation — docs/CONFIGURATION.md accuracy + orchestrator

**Agent:** doc_maintainer

**Objective**: Verify the canonical config doc against the code and fix any drift
matching the website discrepancies; ensure custom sources are documented as a process
orchestrator.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `docs/CONFIGURATION.md`: correct any drifted config keys/defaults/TOML syntax; document
  the custom-source orchestrator surface.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: content boundary rules.
- `crates/fdemon-app/src/config/types.rs`: source of truth for config keys/defaults.
- `workflow/plans/features/website-content-accuracy/tasks/02-configuration-page.md`,
  `03-native-logs-page.md`: the discrepancy list and orchestrator copy.

### Change Context

Research indicated the markdown docs were largely the correct source the website lagged —
so **verify first, change only what has actually drifted.**

Checklist (confirm each against `config/types.rs`; fix if wrong):
1. `[native_logs] min_level` default is `"info"`. [D-17]
2. Custom sources use `[[native_logs.custom_sources]]` (not `sources`). [D-18]
3. Per-tag config uses `[native_logs.tags.<TAG>] min_level = …`. [D-20]
4. No `buffer_size` key documented for `[native_logs]`. [D-19]
5. `devtools.default_panel` lists `inspector/performance/network/memory` (no `layout`). [D-16]
6. `[behavior]` includes `version_check`, `version_check_timeout_secs`. [D-13]
7. `[ui]` includes `icons`. [D-14]
8. `[devtools]` covers the full key set [D-15]; `[dap]` and `[flutter]` sections present. [D-22]
9. **Orchestrator:** `native_logs.custom_sources` documents `working_dir`, `env`,
   `start_before_app`, `shared`, `format` (`raw`/`json`/`logcat-threadtime`/`syslog`,
   syslog macOS-only), and the `ready_check` kinds (`http`/`tcp`/`command`/`stdout`/
   `delay`) with accurate per-kind fields — `stdout` has `timeout_s` only, `delay` has
   `seconds` only; `ready_check` requires `start_before_app = true`; timeout is non-fatal.
   Cross-check `types.rs:738-918`.

### Acceptance Criteria

1. Every config key/default in the doc matches `config/types.rs`.
2. Custom-source orchestrator capability (`start_before_app`/`ready_check`/`shared`) is
   documented with accurate per-kind ready_check fields.
3. No content boundary violations; `doc-validate` passes for `docs/CONFIGURATION.md`.
4. PR notes list which items were already correct vs. fixed.

### Notes

- Make targeted edits, do not rewrite the whole document.
- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `docs/CONFIGURATION.md` | Expanded `[devtools]` section with 13 missing keys; added `[dap]` section (6 keys); added `[flutter]` section (1 key); updated TOC; updated complete example |

### Checklist Results

| Item | Result |
|------|--------|
| [D-17] `min_level` default `"info"` | Already correct — no change needed |
| [D-18] `[[native_logs.custom_sources]]` syntax | Already correct — no change needed |
| [D-20] `[native_logs.tags.<TAG>] min_level` | Already correct — no change needed |
| [D-19] No `buffer_size` key | Already correct — not present |
| [D-16] `devtools.default_panel` options | Fixed — added `default_panel` to devtools section with values `inspector`/`performance`/`network`/`memory` |
| [D-13] `[behavior]` has `version_check` + `version_check_timeout_secs` | Already correct — no change needed |
| [D-14] `[ui]` has `icons` | Already correct — no change needed |
| [D-15] `[devtools]` full key set | Fixed — added 13 missing keys (default_panel, inspector_fetch_timeout_secs, tree_max_depth, hide_implementation_widgets, performance_refresh_ms, auto_repaint_rainbow, auto_performance_overlay, memory_history_size, allocation_profile_interval_ms, max_network_entries, network_auto_record, network_poll_interval_ms) |
| [D-22] `[dap]` and `[flutter]` sections | Fixed — both sections added |
| Orchestrator fields accuracy | Already correct — ready_check per-kind fields match types.rs |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: N/A
</content>
