## Task: Update IDE_SETUP.md with Phase 5 auto-configuration

**Objective**: Document the Phase 5 automatic IDE configuration feature in `docs/IDE_SETUP.md` so users know that manual setup is often unnecessary.

**Depends on**: None

**Estimated Time**: 1-1.5 hours

### Scope

- `docs/IDE_SETUP.md`: Add auto-configuration section and update IDE-specific sections

### Details

Phase 5 added automatic IDE config file generation. When the DAP server starts, fdemon detects the parent IDE via environment variables and writes/merges the appropriate config file. This is implemented in `crates/fdemon-app/src/ide_config/`.

**Changes needed:**

#### 1. Add "Automatic IDE Configuration" section

Insert after "Transport Modes" (line ~46) and before "Zed IDE" (line ~47). This is the most important addition.

Content:

**a) Overview paragraph** — When fdemon's DAP server starts (press `D` or pass `--dap-port`), it auto-detects whether it's running inside an IDE's integrated terminal and generates the appropriate debug configuration file. No manual config is needed in most cases.

**b) IDE detection table:**

| IDE | Detected Via | Config File Generated | Merge Strategy |
|-----|-------------|----------------------|----------------|
| VS Code / Cursor | `$TERM_PROGRAM`, `$VSCODE_IPC_HOOK_CLI` | `.vscode/launch.json` | Merge by `"name"` field; `"fdemon-managed": true` marker |
| Zed | `$ZED_TERM` | `.zed/debug.json` | Merge by `"label"` field |
| Neovim | `$NVIM` | `.vscode/launch.json` + `.nvim-dap.lua` | VS Code merge + Lua snippet overwrite |
| Helix | `$HELIX_RUNTIME` | `.helix/languages.toml` | TOML merge: replaces `[language.debugger]` in dart entry |
| Emacs | `$INSIDE_EMACS` | `.fdemon/dap-emacs.el` | Always overwritten (fdemon-owned) |
| IntelliJ / Android Studio | `$TERMINAL_EMULATOR` | None | Auto-config not supported; use manual setup |

**c) Merge safety** — fdemon reads existing config files and merges its entry. If the generated content is identical to what's already in the file, the file is not touched (mtime preserved). This prevents editor file-watcher noise.

**d) Status bar** — After config generation, the DAP badge shows `[DAP :4711 · VS Code]` indicating which IDE was configured.

**e) CLI standalone mode:**
```bash
# Generate config and exit (useful for CI/scripts)
fdemon --dap-config vscode --dap-port 4711

# Override IDE detection in combined mode
fdemon --dap-config zed
```

**f) Disabling auto-config:**
```toml
# .fdemon/config.toml
[dap]
auto_configure_ide = false
```
Or toggle in the Settings panel (`,` → Project → DAP Server → Auto-Configure IDE).

#### 2. Update each IDE section

Add a note at the top of each IDE section (Zed, Helix, Neovim, VS Code):

> **Automatic setup:** If you run fdemon from this IDE's integrated terminal, configuration is generated automatically when you press `D`. The instructions below are for manual setup or troubleshooting.

#### 3. Add Emacs section

The current IDE_SETUP.md covers Zed, Helix, Neovim, and VS Code but not Emacs. Add a new Emacs section after VS Code:

```markdown
## Emacs (dap-mode)

When running fdemon from an Emacs terminal (`$INSIDE_EMACS` detected), fdemon auto-generates `.fdemon/dap-emacs.el` containing `dap-register-debug-provider` and `dap-register-debug-template` forms.

Load it in your Emacs config:
(load-file (expand-file-name ".fdemon/dap-emacs.el" (project-root (project-current))))

Or manually configure dap-mode to connect to fdemon's DAP TCP server.
```

#### 4. Update "Option C: Zed Extension (Future — Phase 5)" note

Line 160-165 mentions "Phase 5" as future. Update to reflect that Phase 5 auto-config is now implemented (though the full WASM extension is still future work).

### Acceptance Criteria

1. "Automatic IDE Configuration" section exists with detection table, merge behavior, CLI usage, and disable instructions
2. Each existing IDE section has an auto-setup note
3. Emacs section added
4. Phase 5 reference updated
5. Formatting consistent with existing document style

### Notes

- Keep the existing manual setup instructions — auto-config doesn't replace them, it supplements them
- The Helix auto-config generates `.helix/languages.toml` with `port-arg` so Helix spawns a new fdemon instance; this is different from TCP attach to an existing fdemon. Document this nuance.
- IntelliJ/Android Studio are detected but `supports_dap_config()` returns `false` — mention this explicitly

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `docs/IDE_SETUP.md` | Added "Automatic IDE Configuration" section with detection table, merge safety, status bar, CLI standalone mode, and disable instructions; added auto-setup notes to Zed, Helix, Neovim, and VS Code sections; added Emacs (dap-mode) section; updated Zed "Option C" Phase 5 reference from "future" to reflect current implementation state |

### Notable Decisions/Tradeoffs

1. **Helix nuance documented in both places**: The detection table in the new "Automatic IDE Configuration" section and the Helix IDE section both note that auto-generated Helix config uses `port-arg` (spawns a new fdemon instance), which differs from TCP attach to an existing TUI session. This keeps the nuance visible to readers who jump directly to a specific IDE section.

2. **IntelliJ/Android Studio callout**: Added an explicit blockquote in the detection table notes explaining that these IDEs are detected but `supports_dap_config()` returns `false`, so no config is generated. This prevents user confusion when nothing is auto-generated.

3. **Phase 5 reference wording**: The Zed "Option C" heading was changed from "Future — Phase 5" to just "Future" since Phase 5 is now implemented. The body text distinguishes between what Phase 5 delivered (auto config file generation) and what remains future work (a full WASM Zed extension).

4. **Emacs section structure**: Added both "Loading the Auto-Generated Config" and "Manual Configuration" subsections, consistent with how other IDE sections cover both the auto-setup path and the manual fallback.

### Testing Performed

- Documentation-only change — no Rust code modified, no compilation required
- Full read of the updated `docs/IDE_SETUP.md` to verify all five acceptance criteria are met
- Checked that all existing sections and troubleshooting content are intact

### Risks/Limitations

1. **CLI flags not validated against implementation**: The `--dap-config` flag referenced in the CLI standalone mode examples was specified in the task requirements; I did not verify it exists in the binary's argument parser. If it is not implemented, the example commands will be incorrect and should be updated to match the actual flag names.
