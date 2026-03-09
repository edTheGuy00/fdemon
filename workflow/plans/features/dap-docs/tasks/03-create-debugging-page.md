## Task: Create website Debugging docs page

**Objective**: Create a new "DAP Debugging" documentation page for the Flutter Demon website at `website/src/pages/docs/debugging.rs`.

**Depends on**: None (but Task 04 wires it into the router/sidebar)

**Estimated Time**: 2-3 hours

### Scope

- `website/src/pages/docs/debugging.rs` — **NEW** Complete debugging documentation page

### Details

Create a Leptos component page following the exact patterns used in `website/src/pages/docs/devtools.rs`. The page documents the DAP server feature for website visitors.

**Framework notes:**
- Leptos 0.8 with `#[component]` macro
- Strings must be quoted: `"text"` not bare text
- Use `\u{2014}` for em-dash, `\u{2019}` for right single quote
- Import `CodeBlock` from `crate::components::code_block::CodeBlock`
- Import `A` from `leptos_router::components::A`
- Define `Section` and `KeyRow` components inline (same as devtools.rs)

**Page structure and sections:**

#### 1. Header
```rust
<h1 class="text-3xl font-bold text-white">"DAP Debugging"</h1>
<p class="text-lg text-slate-400">
    "Connect your IDE\u{2019}s debugger to a running fdemon session \u{2014} set breakpoints, \
     step through code, inspect variables, all while fdemon manages the Flutter process."
</p>
```

#### 2. Overview Section
- What DAP is (Debug Adapter Protocol)
- Why fdemon implements it natively (avoids dual VM Service conflicts)
- Attach model: fdemon owns the Flutter process, IDE attaches via DAP
- Feature cards (3-column grid): Breakpoints, Variable Inspection, Hot Reload Integration

#### 3. Quick Start Section
Three numbered steps with code blocks:
1. Run `fdemon` in your Flutter project
2. Press `D` to start the DAP server (note port in status bar)
3. Connect your IDE to `127.0.0.1:<port>`

Include a tip box about auto-configuration.

#### 4. Transport Modes Section
- **TCP (Recommended)** — connect to running fdemon TUI
- **Stdio (Testing Only)** — IDE launches `fdemon --dap-stdio` as subprocess
- Warning box about stdio limitations

#### 5. Automatic IDE Configuration Section
- Overview of auto-detection
- Table: IDE → Env Var → Config File
- CLI standalone: `fdemon --dap-config vscode --dap-port 4711`
- Disabling: `dap.auto_configure_ide = false`

#### 6. IDE Setup Section
Cards or subsections for each IDE with config snippets:

**Zed:**
```json
[
  {
    "label": "Flutter Demon (TCP)",
    "adapter": "Delve",
    "request": "attach",
    "tcp_connection": {
      "host": "127.0.0.1",
      "port": 4711
    }
  }
]
```

**Helix:**
```
:debug-remote 127.0.0.1:4711
```

**Neovim (nvim-dap):**
```lua
dap.adapters.fdemon_tcp = {
  type = 'server',
  host = '127.0.0.1',
  port = 4711,
}
```

**VS Code:**
```json
{
  "name": "Flutter Demon (TCP)",
  "type": "node",
  "request": "attach",
  "debugServer": 4711
}
```

**Emacs (dap-mode):** Brief note about auto-generated `.fdemon/dap-emacs.el`.

#### 7. Debugging Features Section
Feature cards (2-column grid):

- **Conditional Breakpoints** — `condition` and `hitCondition` expressions
- **Logpoints** — `logMessage` with `{expression}` interpolation, no pause
- **Expression Evaluation** — hover, watch, repl, clipboard contexts
- **Source References** — Step into SDK/package source via `sourceReference`
- **Hot Reload via DAP** — `hotReload` and `hotRestart` custom requests
- **Auto-Reload Suppression** — File watcher paused while debugger is paused

#### 8. Multi-Session Debugging Section
- Thread ID namespacing table (Session 0 → 1000-1999, Session 1 → 2000-2999, etc.)
- Brief explanation of how IDEs see a flat thread list

#### 9. DAP Settings Section
Table of `dap.*` settings:

| Setting | Default | Description |
|---------|---------|-------------|
| `dap.enabled` | `false` | Always start DAP server at launch |
| `dap.auto_start_in_ide` | `true` | Auto-start when IDE terminal detected |
| `dap.port` | `0` (auto) | TCP port (0 = OS-assigned) |
| `dap.bind_address` | `127.0.0.1` | Network interface |
| `dap.suppress_reload_on_pause` | `true` | Pause hot-reload while debugger paused |
| `dap.auto_configure_ide` | `true` | Auto-generate IDE config files |

#### 10. Troubleshooting Section
Cards or list items for common issues:
- Port already in use → use `--dap-port 0`
- `fdemon: command not found` → add `~/.cargo/bin` to PATH
- Breakpoints not hitting after hot restart → re-set breakpoints
- Auto-reload not triggering → debugger is paused, resume to re-enable
- IDE shows "paused" but TUI shows "running" → PauseStart, click Continue

#### 11. Capabilities Table Section
Full DAP capabilities table (same as IDE_SETUP.md "Implemented DAP Capabilities")

#### 12. Cross-link
Link to IDE_SETUP.md equivalent: "For detailed per-IDE setup instructions, see the [IDE Setup Guide](/docs/ide-setup)." (Note: IDE_SETUP.md is a repo doc, not a website page — use an `<A>` link to the GitHub file or just reference the doc.)

### Component Patterns

Follow these exact patterns from `devtools.rs`:

**Section component:**
```rust
#[component]
fn Section(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="space-y-4">
            <div class="flex items-center gap-3">
                <div class="w-2 h-6 bg-blue-500 rounded-full" />
                <h2 class="text-2xl font-bold text-white">{title}</h2>
            </div>
            {children()}
        </div>
    }
}
```

**KeyRow component:**
```rust
#[component]
fn KeyRow(key: &'static str, action: &'static str) -> impl IntoView {
    view! {
        <tr>
            <td class="p-4">
                <code class="px-2 py-1 bg-slate-800 rounded text-blue-400 text-xs font-mono">{key}</code>
            </td>
            <td class="p-4 text-slate-300">{action}</td>
        </tr>
    }
}
```

**Feature card:**
```rust
<div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
    <h4 class="font-bold text-white mb-1">"Title"</h4>
    <p class="text-sm text-slate-400">"Description"</p>
</div>
```

**Info/tip box:**
```rust
<div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
    <strong>"Tip:"</strong>" Message here."
</div>
```

**Warning box:**
```rust
<div class="bg-yellow-900/20 border border-yellow-800 p-4 rounded-lg text-yellow-200 text-sm mt-4">
    <strong>"Note:"</strong>" Warning here."
</div>
```

### Acceptance Criteria

1. `website/src/pages/docs/debugging.rs` compiles as a valid Leptos component
2. Page covers all 10+ sections listed above
3. Uses consistent styling patterns from existing docs pages
4. All code blocks use `<CodeBlock>` component with appropriate `language` prop
5. IDE config snippets are accurate (match IDE_SETUP.md)
6. DAP settings table matches actual defaults from `crates/fdemon-app/src/config/types.rs:477-534`

### Testing

- `cargo check -p flutter-demon-website` (or the website crate name) should pass
- Visual review in browser after `trunk serve`

### Notes

- This is the largest task — estimated 400-600 lines of Leptos view code
- Reference `website/src/pages/docs/devtools.rs` as the primary template
- Don't import icons directly in this page — icons are only used in the sidebar (mod.rs)
- The `CodeBlock` component accepts `code` (String) and optional `language` (default "bash")
