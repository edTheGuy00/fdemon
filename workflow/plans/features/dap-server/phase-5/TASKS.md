# Phase 5: IDE DAP Auto-Configuration — Task Index

## Overview

Automatically detect which IDE the terminal is running inside and generate the appropriate DAP client configuration so users can start debugging with zero manual setup. Extends the existing `ParentIde` detection with Emacs and Helix variants, introduces an `IdeConfigGenerator` trait with per-IDE implementations (VS Code, Neovim, Helix, Zed, Emacs), and integrates config generation into the DAP server lifecycle via the TEA message/action cycle.

**Total Tasks:** 11
**Waves:** 4 (dependency-ordered)
**Estimated Hours:** 28–38 hours

## Task Dependency Graph

```
Wave 1 — Foundation (parallel)
├── 01-extend-parent-ide
├── 02-ide-config-trait
└── 03-dap-settings-and-messages

Wave 2 — Generators (parallel, depend on Wave 1)
├── 04-vscode-generator       (depends on 1, 2)
├── 05-neovim-generator       (depends on 1, 2)
├── 06-helix-generator        (depends on 1, 2)
├── 07-zed-generator          (depends on 1, 2)
└── 08-emacs-generator        (depends on 1, 2)

Wave 3 — Integration (parallel, depend on Wave 2)
├── 09-auto-generation-trigger (depends on 3, 4–8)
└── 10-dap-config-cli          (depends on 3, 4–8)

Wave 4 — Polish (depends on Wave 3)
└── 11-tui-integration         (depends on 9)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-extend-parent-ide](tasks/01-extend-parent-ide.md) | Done | - | 2–3h | `fdemon-app/config/types.rs`, `fdemon-app/config/settings.rs` |
| 2 | [02-ide-config-trait](tasks/02-ide-config-trait.md) | Done | - | 3–4h | `fdemon-app/ide_config/mod.rs` **NEW**, `fdemon-app/ide_config/merge.rs` **NEW** |
| 3 | [03-dap-settings-and-messages](tasks/03-dap-settings-and-messages.md) | Done | - | 2–3h | `fdemon-app/config/types.rs`, `fdemon-app/message.rs`, `fdemon-app/handler/dap.rs` |
| 4 | [04-vscode-generator](tasks/04-vscode-generator.md) | Done | 1, 2 | 3–4h | `fdemon-app/ide_config/vscode.rs` **NEW** |
| 5 | [05-neovim-generator](tasks/05-neovim-generator.md) | Done | 1, 2 | 3–4h | `fdemon-app/ide_config/neovim.rs` **NEW** |
| 6 | [06-helix-generator](tasks/06-helix-generator.md) | Done | 1, 2 | 3–4h | `fdemon-app/ide_config/helix.rs` **NEW** |
| 7 | [07-zed-generator](tasks/07-zed-generator.md) | Done | 1, 2 | 2–3h | `fdemon-app/ide_config/zed.rs` **NEW** |
| 8 | [08-emacs-generator](tasks/08-emacs-generator.md) | Done | 1, 2 | 2–3h | `fdemon-app/ide_config/emacs.rs` **NEW** |
| 9 | [09-auto-generation-trigger](tasks/09-auto-generation-trigger.md) | Done | 3, 4–8 | 3–4h | `fdemon-app/handler/dap.rs`, `fdemon-app/actions/mod.rs`, `fdemon-app/engine.rs` |
| 10 | [10-dap-config-cli](tasks/10-dap-config-cli.md) | Done | 3, 4–8 | 2–3h | `flutter-demon/src/main.rs`, `fdemon-app/ide_config/mod.rs` |
| 11 | [11-tui-integration](tasks/11-tui-integration.md) | Done | 9 | 2–3h | `fdemon-tui/widgets/log_view/mod.rs`, `fdemon-app/handler/dap.rs` |
| 12 | [12-fix-ide-config-discrepancies](tasks/12-fix-ide-config-discrepancies.md) | Done | 8 | 1–2h | `fdemon-app/ide_config/emacs.rs` |

## Success Criteria

- [ ] `ParentIde` enum extended with `Emacs` and `Helix` variants (with env var detection)
- [ ] `ParentIde::supports_dap_config()` and `dap_config_path()` methods implemented
- [ ] VS Code config generator creates valid `launch.json` with `debugServer` field
- [ ] VS Code config generator merges into existing `launch.json` without clobbering other configs
- [ ] Neovim config generator produces `.vscode/launch.json` + `.nvim-dap.lua` snippet
- [ ] Helix config generator produces valid `.helix/languages.toml` with `transport = "tcp"`
- [ ] Zed config generator produces valid `.zed/debug.json` with `tcp_connection`
- [ ] Emacs config generator produces `.fdemon/dap-emacs.el` with `dap-register-debug-provider`
- [ ] Auto-generation triggers on DAP server bind, skips gracefully when no IDE detected
- [ ] `--dap-config <ide>` CLI flag works for manual generation
- [ ] Config merge logic handles malformed files without data loss (skip + warn)
- [ ] 50+ unit tests covering generation, merging, and edge cases
- [ ] `cargo fmt --all` — Pass
- [ ] `cargo check --workspace` — Pass
- [ ] `cargo test --workspace` — Pass (all existing + new tests green)
- [ ] `cargo clippy --workspace -- -D warnings` — Pass

## Notes

- The `ide_config/` module lives in `fdemon-app` (not `fdemon-dap`) because it needs access to `ParentIde` and integrates with the handler/action system. `fdemon-dap` has no dependency on `fdemon-app`.
- The existing `config/vscode.rs` is a read-only parser for importing `.vscode/launch.json` into fdemon's launch configs. The new `ide_config/vscode.rs` writes DAP-specific entries — these are complementary, not conflicting.
- JSON merge uses the existing `clean_jsonc()` function from `config/vscode.rs` for reading, but writes standard JSON (no comments) since VS Code handles both.
- All generators are pure functions (port + project_path → config file) with no runtime state, making them trivially testable with `tempdir()`.
- Helix has an inherent limitation: its `transport = "tcp"` always spawns the adapter binary. The generated config uses `fdemon` as the command with `--dap-port {}`, which spawns a new fdemon instance. This is documented as a known limitation.
