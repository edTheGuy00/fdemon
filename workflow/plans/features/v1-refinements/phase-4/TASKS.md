# Phase 4: Website Updates, Changelog & GHCR Publishing - Task Index

## Overview

Update the website with Network Monitor documentation, fix keybinding data accuracy, replace the installation placeholder with real instructions, add a changelog page tracking every release, and create a GitHub Actions workflow to containerize the website and publish to GHCR for deployment to fdemon.dev.

**Total Tasks:** 6

## Task Dependency Graph

```
Wave 1 (independent)
┌─────────────────────────────┐  ┌─────────────────────────────┐  ┌─────────────────────────────┐
│  01-fix-keybindings-data    │  │  03-update-installation     │  │  04-changelog-setup         │
│  Fix data.rs keybindings    │  │  Real install instructions  │  │  git-cliff + release.yml    │
└──────────────┬──────────────┘  └─────────────────────────────┘  └──────────────┬──────────────┘
               │                                                                 │
Wave 2         ▼                                                                 ▼
┌─────────────────────────────┐                                  ┌─────────────────────────────┐
│  02-devtools-network-docs   │                                  │  05-changelog-page          │
│  Network section in devtools│                                  │  /docs/changelog route      │
└─────────────────────────────┘                                  └─────────────────────────────┘

Independent (any wave)
┌─────────────────────────────┐
│  06-ghcr-publish-workflow   │
│  Build & push Docker image  │
└─────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-keybindings-data](tasks/01-fix-keybindings-data.md) | Not Started | - | `website/src/data.rs` |
| 2 | [02-devtools-network-docs](tasks/02-devtools-network-docs.md) | Not Started | 1 | `website/src/pages/docs/devtools.rs` |
| 3 | [03-update-installation-page](tasks/03-update-installation-page.md) | Not Started | - | `website/src/pages/docs/installation.rs` |
| 4 | [04-changelog-setup](tasks/04-changelog-setup.md) | Not Started | - | `cliff.toml`, `.github/workflows/release.yml` |
| 5 | [05-changelog-page](tasks/05-changelog-page.md) | Not Started | 4 | `website/src/data.rs`, `website/src/pages/docs/changelog.rs`, `website/src/pages/docs/mod.rs`, `website/src/lib.rs` |
| 6 | [06-ghcr-publish-workflow](tasks/06-ghcr-publish-workflow.md) | Not Started | - | `.github/workflows/publish-site.yml` |

## Success Criteria

Phase 4 is complete when:

- [ ] DevTools page documents the Network Monitor panel with full keybindings
- [ ] Phantom `l` → "Layout Panel" keybinding removed from `data.rs`
- [ ] `n` → "Network Panel" keybinding added to `data.rs`
- [ ] All Network panel keybindings documented (14+ bindings across 2 sections)
- [ ] Missing Performance panel keybindings added (3 bindings)
- [ ] Installation page updated with curl install command + platform instructions
- [ ] `cliff.toml` configured for conventional commits
- [ ] `release.yml` generates changelog on release
- [ ] Changelog page exists at `/docs/changelog` on the website
- [ ] `publish-site.yml` workflow builds website Docker image and pushes to `ghcr.io`
- [ ] Website builds successfully (`trunk build` in `website/`)

## Notes

- The website is a Leptos 0.8 CSR WASM app built with Trunk, served via nginx in Docker
- The website is hosted on the user's own server at fdemon.dev — GHCR is used as a container registry only (not GitHub Pages)
- The existing `website/Dockerfile` (multi-stage: rust builder + nginx:alpine) is already production-ready
- Keybinding corrections are based on verified codebase analysis: `DevToolsPanel` has 3 variants (Inspector, Performance, Network) — no Layout variant
