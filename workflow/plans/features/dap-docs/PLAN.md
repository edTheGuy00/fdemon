# Plan: DAP Server Documentation Update

## TL;DR

Update all documentation to reflect the completed DAP server feature (Phases 1–5). This covers three areas: (1) update `docs/IDE_SETUP.md` to document Phase 5 auto-configuration, (2) add the `D` (toggle DAP) keybinding to `docs/KEYBINDINGS.md`, and (3) add a new "DAP Debugging" page to the Leptos website at `website/src/pages/docs/debugging.rs`.

---

## Background

The DAP server feature is complete across all 5 phases. The existing `docs/IDE_SETUP.md` was written during Phases 3–4 and thoroughly documents manual IDE setup and Phase 4 debugging features. However:

1. **Phase 5 auto-configuration is undocumented** — fdemon now auto-generates IDE config files (`.vscode/launch.json`, `.zed/debug.json`, `.helix/languages.toml`, `.nvim-dap.lua`, `.fdemon/dap-emacs.el`) when the DAP server starts. The `--dap-config` CLI flag and `dap.auto_configure_ide` setting are not mentioned in IDE_SETUP.md.

2. **`docs/KEYBINDINGS.md` is missing the DAP toggle** — `Shift+D` (`D`) toggles the DAP server on/off in Normal mode. This keybinding is not listed in the keybindings doc (note: lowercase `d` for DevTools IS listed).

3. **The website has no DAP/debugging page** — The website docs (`/docs/*`) cover Introduction, Installation, Keybindings, DevTools, Configuration, Architecture, and Changelog. There is no debugging page. Users visiting the website have no way to discover the DAP server feature.

---

## Affected Files

- `docs/IDE_SETUP.md` — Add Phase 5 auto-configuration section
- `docs/KEYBINDINGS.md` — Add `D` keybinding to Normal mode > Session Management
- `website/src/pages/docs/debugging.rs` — **NEW** Debugging docs page
- `website/src/pages/docs/mod.rs` — Add `debugging` module + sidebar entry
- `website/src/lib.rs` — Add route for `/docs/debugging`
- `website/src/components/icons.rs` — Add `Bug` icon (Lucide) for sidebar

---

## Tasks

All tasks are independent and can be executed in parallel (Wave 1).

### Task 1: Update `docs/KEYBINDINGS.md` with DAP toggle

**What to change:**

In the "Normal Mode > Session Management" table, add a row for `D`:

| Key | Action | Description |
|-----|--------|-------------|
| `D` | Toggle DAP Server | Start or stop the DAP server (status shown in status bar) |

This should go after the `d` (DevTools Mode) row since they're related (`d` = DevTools, `D` = DAP).

Also add a new "DAP Server" subsection under Normal Mode (after DevTools) with a brief description:

```markdown
### DAP Server

| Key | Action | Description |
|-----|--------|-------------|
| `D` | Toggle DAP Server | Start or stop the DAP debug adapter server. When active, `[DAP :PORT]` appears in the status bar. |
```

**Files:** `docs/KEYBINDINGS.md`

---

### Task 2: Update `docs/IDE_SETUP.md` with Phase 5 auto-configuration

**What to add:**

Insert a new section after "Transport Modes" and before the IDE-specific sections (Zed, Helix, etc.) titled "Automatic IDE Configuration (Phase 5)".

Content should cover:

1. **Overview** — When the DAP server starts, fdemon auto-detects the parent IDE and generates/merges the appropriate config file. No manual setup needed in most cases.

2. **IDE detection table:**

| IDE | Environment Variable | Config File Written |
|-----|---------------------|---------------------|
| VS Code / Cursor | `$TERM_PROGRAM`, `$VSCODE_IPC_HOOK_CLI` | `.vscode/launch.json` |
| Zed | `$ZED_TERM` | `.zed/debug.json` |
| Neovim | `$NVIM` | `.vscode/launch.json` + `.nvim-dap.lua` |
| Helix | `$HELIX_RUNTIME` | `.helix/languages.toml` |
| Emacs | `$INSIDE_EMACS` | `.fdemon/dap-emacs.el` |
| IntelliJ / Android Studio | `$TERMINAL_EMULATOR` | Not supported (use manual setup) |

3. **Merge behavior** — Existing config files are merged safely. fdemon uses `"fdemon-managed": true` markers (VS Code) or label matching (Zed) to identify its own entries. Files are only written when content actually changes (mtime preserved otherwise).

4. **CLI standalone mode** — `fdemon --dap-config <IDE> --dap-port <PORT>` generates the config and exits (useful for CI/scripts). Example: `fdemon --dap-config vscode --dap-port 4711`.

5. **Disabling auto-config** — Set `dap.auto_configure_ide = false` in `.fdemon/config.toml` or toggle in Settings panel.

6. **Status bar indicator** — After config generation, the DAP badge changes to `[DAP :4711 · VS Code]` showing which IDE was configured.

Also update each IDE-specific section to mention: "If you launched fdemon from this IDE's integrated terminal, configuration is automatic — see [Automatic IDE Configuration](#automatic-ide-configuration)."

**Files:** `docs/IDE_SETUP.md`

---

### Task 3: Add "Debugging" page to website

**New file:** `website/src/pages/docs/debugging.rs`

This page documents the DAP server feature for the website. It should follow the exact same patterns as `devtools.rs`:

- Use `Section` component (defined inline, same as other pages)
- Use `CodeBlock` for config snippets
- Use `KeyRow` for keybinding tables
- Use the standard card/table styling patterns

**Page sections:**

1. **Header** — "DAP Debugging" title + tagline about IDE-integrated debugging
2. **Overview** — What the DAP server is, why it exists (attach debugger from any IDE to a running fdemon session)
3. **Quick Start** — 3 steps: run fdemon, press `D`, connect IDE
4. **Transport Modes** — TCP (recommended) vs Stdio (testing only)
5. **Auto-Configuration** — IDE detection, generated files table, merge behavior
6. **IDE Setup** — Cards/tabs for Zed, Helix, Neovim, VS Code, Emacs with code snippets
7. **Debugging Features** — Feature cards for: Breakpoints, Conditional Breakpoints, Logpoints, Expression Evaluation, Source References, Hot Reload/Restart via DAP, Auto-Reload Suppression
8. **Multi-Session Debugging** — Thread ID namespacing table
9. **DAP Settings** — Table of `dap.*` settings with defaults
10. **Troubleshooting** — Common issues (port in use, command not found, breakpoints after hot restart, etc.)

**Estimated length:** ~400-600 lines (comparable to `devtools.rs` at ~520 lines or `configuration.rs` at ~417 lines)

---

### Task 4: Wire up the debugging page in the website

**Changes to existing files:**

1. **`website/src/components/icons.rs`** — Add a `Bug` icon (Lucide bug icon) for the sidebar. SVG paths:
   ```
   <path d="m8 2 1.88 1.88" />
   <path d="M14.12 3.88 16 2" />
   <path d="M9 7.13v-1a3.003 3.003 0 1 1 6 0v1" />
   <path d="M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6" />
   <path d="M12 20v-9" />
   <path d="M6.53 9C4.6 8.8 3 7.1 3 5" />
   <path d="M6 13H2" />
   <path d="M3 21c0-2.1 1.7-3.9 3.8-4" />
   <path d="M20.97 5c0 2.1-1.6 3.8-3.5 4" />
   <path d="M22 13h-4" />
   <path d="M17.2 17c2.1.1 3.8 1.9 3.8 4" />
   ```

2. **`website/src/pages/docs/mod.rs`** — Add `pub mod debugging;` and a new `DocItem` in `doc_items()`:
   ```rust
   DocItem {
       href: "/docs/debugging",
       label: "Debugging",
       icon: || view! { <Bug class="w-4 h-4 mr-3" /> }.into_any(),
   },
   ```
   Place it after DevTools in the sidebar order (Introduction → Installation → Keybindings → DevTools → **Debugging** → Configuration → Architecture → Changelog).

3. **`website/src/lib.rs`** — Add the route inside the `ParentRoute`:
   ```rust
   <Route path=path!("/debugging") view=Debugging />
   ```
   And the import:
   ```rust
   use pages::docs::debugging::Debugging;
   ```

---

## Task Dependency Graph

```
Wave 1 (all parallel — no dependencies)
├── Task 1: Update KEYBINDINGS.md
├── Task 2: Update IDE_SETUP.md
├── Task 3: Create debugging.rs page
└── Task 4: Wire up debugging page (icons, mod.rs, lib.rs)
```

> Note: Task 3 and Task 4 are logically related but can be implemented by the same agent sequentially (Task 4 is trivial wiring).

---

## Success Criteria

- [ ] `docs/KEYBINDINGS.md` includes `D` (Toggle DAP Server) in Normal mode
- [ ] `docs/IDE_SETUP.md` has an "Automatic IDE Configuration" section documenting Phase 5
- [ ] Each IDE section in `IDE_SETUP.md` mentions auto-config availability
- [ ] Website has `/docs/debugging` route with full DAP documentation
- [ ] Website sidebar shows "Debugging" entry with Bug icon between DevTools and Configuration
- [ ] `cargo build --workspace` passes (website is separate, not in workspace)
- [ ] Website builds with `trunk build` (if available) or at minimum compiles with `cargo check`

---

## Notes

- The website uses **Leptos 0.8** (Rust WASM framework) with **Tailwind CSS** — not a JS framework
- All website docs pages define a local `Section` component and `KeyRow` component inline
- The existing `devtools.rs` page is the closest structural analog for the new debugging page
- Icons use a `lucide_icon!` macro — adding new icons is a one-liner macro invocation
- Available icons that already exist: `Shield`, `Eye`, `Zap`, `Cpu`, `Terminal` — none fit "debugging" well, so adding `Bug` is appropriate
