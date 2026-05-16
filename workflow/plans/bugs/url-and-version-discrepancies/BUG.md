# Bugfix Plan: URL and Version Discrepancies

## TL;DR

Two user-reported issues — (1) the auto-generated `.fdemon/config.toml` and `.fdemon/launch.toml` carry a placeholder GitHub URL (`github.com/example/flutter-demon`) that was never replaced with the real docs URL, and (2) the website home page hardcodes `v0.1.0` in the release badge instead of reflecting the current GitHub release. Research also surfaced two related strays on the website's Installation page that hardcode `0.1.0`. All are simple text fixes scoped to a handful of files.

## Bug Reports

### Bug 1: Generated config files reference a placeholder GitHub URL

**Symptom:** When `fdemon` runs in a project for the first time and creates `.fdemon/config.toml` (and `.fdemon/launch.toml`), the file header points users to a dead URL:

```toml
# Flutter Demon Configuration
# See: https://github.com/example/flutter-demon#configuration
```

The canonical docs live at `https://fdemon.dev/docs/configuration` (per `README.md` lines 18–21 and 107).

**Expected:** Header should read

```toml
# Flutter Demon Configuration
# See: https://fdemon.dev/docs/configuration
```

…and the analogous launch-config header should also point at `https://fdemon.dev/docs/configuration` (the website's configuration page documents both `config.toml` and `launch.toml` — see `website/src/pages/docs/configuration.rs:32`, `:192`, `:420`).

**Root Cause:** The placeholder `github.com/example/flutter-demon` string was scaffolded during initial project setup (`workflow/plans/features/initial-project-setup/phase_3/tasks/01-config-module.md`) and never updated when the project moved to its real GitHub home + dedicated docs site. Four occurrences in generator code:

- `crates/fdemon-app/src/config/settings.rs:450` — `init_project_config()` default-content writer
- `crates/fdemon-app/src/config/settings.rs:554` — `generate_config_header()` (used by `save_settings()`)
- `crates/fdemon-app/src/config/settings.rs:656` — `generate_default_config()` (used by `init_fdemon_directory()`)
- `crates/fdemon-app/src/config/launch.rs:123` — launch-config default-content writer (uses `#launch-configurations` anchor which also doesn't exist)

Plus three checked-in fixture/example configs already carry the stale string and need to be re-aligned so they don't drift back into examples:

- `crates/fdemon-tui/.fdemon/config.toml:2`
- `example/app1/.fdemon/config.toml:2`
- `tests/fixtures/simple_app/.fdemon/config.toml:2`

**Affected Files:**
- `crates/fdemon-app/src/config/settings.rs` — three string literals
- `crates/fdemon-app/src/config/launch.rs` — one string literal
- `crates/fdemon-tui/.fdemon/config.toml` — checked-in dev config
- `example/app1/.fdemon/config.toml` — example app config
- `tests/fixtures/simple_app/.fdemon/config.toml` — integration-test fixture

---

### Bug 2: Website home page hardcodes `v0.1.0` release badge

**Symptom:** The Flutter Demon home page renders a badge that says `release v0.1.0` regardless of the actual released version (currently `0.5.2` per `Cargo.toml:7`).

**Expected:** The badge should always reflect the latest GitHub release. The `README.md` already uses a dynamic shields.io endpoint that pulls live from GitHub:

```
https://img.shields.io/github/v/release/edTheGuy00/fdemon?style=flat&labelColor=1d1d1d&color=54c5f8&logo=GitHub&logoColor=white
```

The website home page should use the same dynamic badge URL so it stays in sync without needing a website rebuild for each release.

**Root Cause:** `website/src/pages/home.rs:54` hardcodes the badge:

```rust
src="https://img.shields.io/badge/release-v0.1.0-blue?style=flat&labelColor=1d1d1d&color=54c5f8"
```

The build script (`website/build.rs`) reads `changelog.json` for the changelog page but doesn't expose a version constant to the home page, and `website/Cargo.toml:3` is still `0.1.0` — so even a `CARGO_PKG_VERSION` approach would be wrong without also bumping the website crate version in lockstep. Swapping to the dynamic shields.io GitHub-release endpoint sidesteps the entire problem.

**Affected Files:**
- `website/src/pages/home.rs` — single `<img src=…>` URL

---

### Bug 3 (related, surfaced during research): Installation page hardcodes `0.1.0`

**Symptom:** Two strings on the Installation page reference `0.1.0`:

- `website/src/pages/docs/installation.rs:31` — example command `… | bash -s -- --version 0.1.0`
- `website/src/pages/docs/installation.rs:149` — `"Expected output: fdemon 0.1.0 (or the installed version)"`

**Expected:** The docs should auto-update with each release rather than drifting silently every time the version bumps. The cleanest fix mirrors what `build.rs` already does for `changelog.json`: read the workspace `Cargo.toml` version at build time and emit a `FDEMON_VERSION` constant the page can interpolate. That way the installation example pins to the current release with zero ongoing maintenance.

**Root Cause:** The page was scaffolded with `0.1.0` as a literal — there's no automated link between the website source and the workspace version. `website/build.rs` already reads `changelog.json` and emits a `changelog_generated.rs` include file, so the pattern is established; we just need a second emitted constant.

**Affected Files:**
- `website/build.rs` — extend to read workspace `Cargo.toml` and emit `FDEMON_VERSION` const
- `website/src/pages/docs/installation.rs` — two string literals replaced with `{FDEMON_VERSION}` interpolations
- `website/src/data.rs` (or similar shared module) — expose the generated constant for consumption by the installation page

---

### Non-issues confirmed during research

- `website/src/pages/docs/debugging.rs:248` — `"version": "0.2.0"` is the **VS Code `launch.json` schema version**, not the fdemon version. Leave as is.
- `website/Cargo.toml:3` `version = "0.1.0"` — the website crate's own SemVer, independent of the fdemon release. Leave as is (the dynamic shields.io badge approach avoids any dependency on this).
- `example/app2/.fdemon/config.toml` through `example/app5/.fdemon/config.toml` do **not** carry the stale `See:` line — no change needed.

---

## Affected Modules

- `crates/fdemon-app/src/config/settings.rs` — three header strings in default-config generators
- `crates/fdemon-app/src/config/launch.rs` — one header string in launch-config generator
- `crates/fdemon-tui/.fdemon/config.toml` — re-align checked-in config
- `example/app1/.fdemon/config.toml` — re-align example config
- `tests/fixtures/simple_app/.fdemon/config.toml` — re-align fixture
- `website/src/pages/home.rs` — swap hardcoded badge for dynamic shields.io endpoint
- `website/build.rs` — extend to emit `FDEMON_VERSION` const from workspace `Cargo.toml`
- `website/src/data.rs` — expose `FDEMON_VERSION` const generated by `build.rs`
- `website/src/pages/docs/installation.rs` — replace two hardcoded `0.1.0` strings with `FDEMON_VERSION`

---

## Phases

Single phase — four independent tasks (see `TASKS.md`).

### Phase 1: URL + Version Discrepancy Fixes

**Goal:** Eliminate placeholder URLs in generated config files, eliminate hardcoded `v0.1.0` from the website's public release surface, and remove the underlying mechanism that lets version strings go stale on the install docs.

**Task summary:**

1. **`01-fix-config-url-generators`** — replace four placeholder URLs in the `fdemon-app` config generators with `https://fdemon.dev/docs/configuration`; add a regression test so the URL can't silently drift again.
2. **`02-update-config-fixtures`** — apply the same URL replacement to three checked-in `.fdemon/config.toml` files so they don't reseed the old placeholder into anyone reading the repo as a template.
3. **`03-website-home-dynamic-release-badge`** — swap the home-page's hardcoded `release-v0.1.0` badge for the dynamic `github/v/release/edTheGuy00/fdemon` shields.io endpoint (matching `README.md:11`), so the badge tracks GitHub releases automatically.
4. **`04-website-installation-version-constant`** — extend `website/build.rs` to read the workspace `Cargo.toml` version and emit a `FDEMON_VERSION` const; consume it from `installation.rs` in place of the two literal `0.1.0` strings.

**Measurable Outcomes:**

- A fresh `fdemon` run in a brand-new project writes `# See: https://fdemon.dev/docs/configuration` to `.fdemon/config.toml` and `.fdemon/launch.toml`.
- `grep -rn "github.com/example" crates/ tests/ example/ website/` returns zero results in code/fixtures (workflow plan docs that quote historical content are out of scope).
- The website home page renders a badge that auto-tracks the latest GitHub release (verified locally with `trunk serve` from `website/`).
- Installation page worked-example version equals the workspace `Cargo.toml` version and updates automatically on the next release.
- `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cd website && trunk build` succeeds.

---

## Edge Cases & Risks

### Existing user projects with stale URLs
- **Risk:** Users who already have a generated `.fdemon/config.toml` won't get the new URL automatically — the generator only writes when the file is absent.
- **Mitigation:** Acceptable. The URL is a code comment, not functional config. New projects are fixed; existing users can update by hand or by deleting and regenerating. No migration code warranted.

### Test fixtures embedded in integration tests
- **Risk:** `tests/fixtures/simple_app/.fdemon/config.toml` may be loaded by tests that assert on header text.
- **Mitigation:** Confirmed by grep — no test asserts on the placeholder URL string. The two unit tests in `settings.rs` (around line 1373) only check for `"Flutter Demon Configuration"` and the leading `#`, both preserved.

### Workspace Cargo.toml path resolution in build.rs
- **Risk:** `website/build.rs` needs to locate the workspace `Cargo.toml` (one directory up from `website/`) to read the version. If the website is ever built standalone (e.g. unpacked outside the monorepo), the read will fail and the build will break.
- **Mitigation:** Use `CARGO_MANIFEST_DIR` + `..` and gate the read with a clear fallback: if the parent `Cargo.toml` is missing, emit `FDEMON_VERSION = "unknown"` rather than panicking. Print a `cargo:warning=…` so the failure is visible. Add `cargo:rerun-if-changed=../Cargo.toml`.

### Anchor `#launch-configurations` doesn't exist on the configuration page
- **Risk:** Pointing both header comments at `https://fdemon.dev/docs/configuration` is more useful than pointing at a non-existent anchor.
- **Mitigation:** Use the bare URL; if/when the configuration page grows a `#launch-configurations` heading, a follow-up can re-add the fragment.

---

## Further Considerations

1. **Out-of-band release-version display:** Once `FDEMON_VERSION` exists, the home page could render the version as a styled span instead of relying on the external shields.io badge image — fewer external dependencies, faster initial paint. Optional polish; out of scope here because the shields.io badge already matches the `README.md` convention.
2. **Stale URL in workflow plan docs:** Three task files under `workflow/plans/features/*` (e.g., `log-config-enhancements/phase_4/tasks/11-settings-persistence.md`, `12-init-gitignore.md`, `initial-project-setup/phase_3/tasks/01-config-module.md`) still quote the original placeholder URL as historical context. These are immutable historical artifacts of how a task was scoped — explicitly out of scope to edit.
3. **Anchor on configuration page:** If/when the configuration page grows a `#launch-configurations` heading, follow-up task can re-add the fragment to the `launch.toml` `See:` URL. Today the page uses tabbed sections without anchors; bare URL is correct.

---

## Task Dependency Graph

```
Phase 1 (all parallel, no inter-task deps)
├── 01-fix-config-url-generators
├── 02-update-config-fixtures
├── 03-website-home-dynamic-release-badge
└── 04-website-installation-version-constant
```

All four tasks write disjoint files and have no behavioural coupling — see `TASKS.md` for the full File Overlap Analysis.

---

## Success Criteria

### Phase 1 Complete When:
- [ ] All four generator-side `See:` URLs in `crates/fdemon-app/src/config/` point at `https://fdemon.dev/docs/configuration`.
- [ ] A regression test in `settings.rs` asserts the new URL appears in generated content and `github.com/example` does not.
- [ ] All three checked-in `.fdemon/config.toml` fixtures (`fdemon-tui`, `example/app1`, `tests/fixtures/simple_app`) point at the same URL.
- [ ] `website/src/pages/home.rs` release badge uses the dynamic shields.io GitHub-release endpoint.
- [ ] `website/build.rs` reads workspace `Cargo.toml` and emits `FDEMON_VERSION`; `installation.rs` consumes it; both literal `0.1.0` references are gone.
- [ ] `grep -rn "github.com/example" crates/ tests/ example/ website/` returns no hits.
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cd website && trunk build` succeeds without errors.

---

## Milestone Deliverable

Auto-generated config files point users at the real docs site, and the website's home page release badge automatically reflects the live GitHub release — eliminating the two most visible stale-text discrepancies between the project and its public surface.

---

## File Overlap Analysis

See `TASKS.md` for the canonical task-level breakdown and overlap matrix. Summary: all four tasks write disjoint files and can run in parallel worktrees.
