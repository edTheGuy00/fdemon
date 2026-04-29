# Task Index — Re-gate cache-driven auto-launch behind `[behavior] auto_launch`

Plan: [BUG.md](./BUG.md)

Decisions locked in (BUG.md §"Decisions"):
- Flag name: `[behavior] auto_launch` (default `false`).
- Headless: option (b) — always auto-launches; cache hard-disabled (`cache_allowed = false`).
- Settings Panel: own row in Behavior section.
- Example fixture: commented-out discoverability line in `example/app2/.fdemon/config.toml`.
- Migration: one-time `info!` log when cache present but opt-in absent (TUI + headless paths).

---

## Tasks

| # | Task | File | Agent | Depends on |
|---|------|------|-------|------------|
| 01 | Add `[behavior] auto_launch` field to `BehaviorSettings` + Settings Panel row | [tasks/01-add-auto-launch-field.md](./tasks/01-add-auto-launch-field.md) | implementor | — |
| 02 | Plumb `cache_allowed: bool` through `Message::StartAutoLaunch` → `UpdateAction::DiscoverDevicesAndAutoLaunch` → `spawn_auto_launch` → `find_auto_launch_target`; skip Tier 2 when disallowed | [tasks/02-plumb-cache-allowed-param.md](./tasks/02-plumb-cache-allowed-param.md) | implementor | — |
| 03 | Re-gate TUI `startup_flutter` so `cache_trigger` requires `settings.behavior.auto_launch == true`; pass real value as `cache_allowed`; emit migration `info!` | [tasks/03-tui-startup-gate.md](./tasks/03-tui-startup-gate.md) | implementor | 01, 02 |
| 04 | Headless: reuse `find_auto_launch_target` with hard-wired `cache_allowed = false`; emit migration `info!` (sibling-bug coordination required) | [tasks/04-headless-gate.md](./tasks/04-headless-gate.md) | implementor | 02 + sibling `launch-toml-device-ignored` Task 03 |
| 05 | Docs: rewrite `docs/CONFIGURATION.md` Auto-Start Behavior section; add commented-out `# auto_launch = true` line to `example/app2/.fdemon/config.toml` | [tasks/05-docs-and-example.md](./tasks/05-docs-and-example.md) | implementor | 01, 02, 03, 04 |
| 06 | `docs/ARCHITECTURE.md` startup-sequence line update for the new gate condition | [tasks/06-architecture-doc.md](./tasks/06-architecture-doc.md) | doc_maintainer | 01, 02, 03, 04 |

---

## Wave Plan

- **Wave 1 (parallel):** Tasks 01 and 02. They write to disjoint files and 02's hardcoded `cache_allowed: false` is intentionally a no-behavior-change interim step (it preserves today's "cache fires auto-launch" semantics until Wave 2 wires it up).
- **Wave 2 (parallel):** Tasks 03 and 04. They write to disjoint files (`crates/fdemon-tui/*` vs `src/headless/*`). Both depend on the param plumbed in 02 and the field added in 01.
- **Wave 3 (parallel):** Tasks 05 and 06. Pure documentation; different files; both routed independently (05 → implementor; 06 → doc_maintainer).

> **Sibling coordination:** Task 04 explicitly depends on the sibling bug `launch-toml-device-ignored` Task 03 having merged (it owns the `find_auto_launch_target` headless wiring). If the sibling has not merged when Wave 2 starts, Task 04 must either (a) wait, or (b) re-implement the wiring inline and the sibling task becomes a no-op on merge. Prefer (a).

---

## File Overlap Analysis

### Files Modified (Write)

| Task | Files Modified (Write) | Files Read (dependency) |
|------|------------------------|--------------------------|
| 01 | `crates/fdemon-app/src/config/types.rs` (add `auto_launch: bool` to `BehaviorSettings`) · `crates/fdemon-app/src/config/settings.rs` (round-trip in `save_settings`) · `crates/fdemon-app/src/settings_items.rs` (Behavior tab row) | — |
| 02 | `crates/fdemon-app/src/message.rs` (add `cache_allowed` to `Message::StartAutoLaunch`) · `crates/fdemon-app/src/handler/mod.rs` (add field to `UpdateAction::DiscoverDevicesAndAutoLaunch`) · `crates/fdemon-app/src/handler/update.rs` (propagate field in match arm) · `crates/fdemon-app/src/handler/tests.rs` (update test constructors to pass `cache_allowed: true` to preserve existing assertions) · `crates/fdemon-app/src/actions/mod.rs` (pass to `spawn_auto_launch`) · `crates/fdemon-app/src/spawn.rs` (accept param; thread to `find_auto_launch_target`; skip Tier 2 when `false`; add unit tests) · `crates/fdemon-tui/src/runner.rs` (construct `StartAutoLaunch` with `cache_allowed: false` — placeholder until Task 03) | — |
| 03 | `crates/fdemon-tui/src/startup.rs` (cache_trigger now requires `settings.behavior.auto_launch`; migration `info!`; update G1/G2 tests; add G3/G4/G5 tests; activate `_settings` parameter) · `crates/fdemon-tui/src/runner.rs` (replace Task 02's hardcoded `false` with `engine.settings.behavior.auto_launch`) | `crates/fdemon-app/src/config/types.rs` (Task 01's new field) · `crates/fdemon-app/src/message.rs` (Task 02's new field) |
| 04 | `src/headless/runner.rs` (reuse `find_auto_launch_target` from sibling Task 03; pass `cache_allowed = false`; migration `info!` when applicable; update headless test) | `crates/fdemon-app/src/spawn.rs` (Task 02's signature) · `crates/fdemon-app/src/config/mod.rs` (`load_all_configs`, sibling Task 03 entry point) |
| 05 | `docs/CONFIGURATION.md` (rewrite "Auto-Start Behavior" section §183-216; correct priority cascade table; add `auto_launch` reference under "Behavior Settings" §234-247; document migration note) · `example/app2/.fdemon/config.toml` (add commented `# auto_launch = true` line in `[behavior]` block) | All implementation tasks (to describe shipped behavior accurately) |
| 06 | `docs/ARCHITECTURE.md` (line 1444 startup-sequence line — augment "if auto_start=false" → "unless auto_start or auto_launch fires"; optional: add gate diagram in Data Flow → Startup Sequence) | All implementation tasks |

### Overlap Matrix

|        | 01 | 02 | 03 | 04 | 05 | 06 |
|--------|----|----|----|----|----|----|
| **01** | —  | none | none (read-only on Task 01's field) | none | none | none |
| **02** | none | — | **shared write: `crates/fdemon-tui/src/runner.rs`** → sequential (03 after 02) | none | none | none |
| **03** | read-only | shared write `runner.rs` | — | none | none | none |
| **04** | none | none (read-only on Task 02's signature) | none | — | none | none |
| **05** | none | none | none | none | — | none |
| **06** | none | none | none | none | none | — |

### Strategy Per Pair

- **01 ↔ 02:** zero overlap → **parallel (worktree)**.
- **02 ↔ 03:** both write `crates/fdemon-tui/src/runner.rs` (the `Message::StartAutoLaunch { configs }` construction site at line 181). 02 introduces `cache_allowed: false` placeholder; 03 swaps to real value. **Sequential (same branch)** — 02 first, then 03. (Task 03 depends on 02 in dependency order anyway, so no extra constraint.)
- **02 ↔ 04:** 04 reads 02's signature but writes a different file (`src/headless/runner.rs`). **Parallel (worktree)** — but only after 02 has merged so the call site compiles.
- **03 ↔ 04:** disjoint write sets (`crates/fdemon-tui/*` vs `src/headless/*`). **Parallel (worktree)**.
- **05 ↔ 06:** disjoint write sets (`docs/CONFIGURATION.md` vs `docs/ARCHITECTURE.md`). **Parallel (worktree)**, different agents (implementor vs doc_maintainer).
- **01 ↔ {03, 04}:** read-only relationship → **parallel** but downstream tasks depend on 01 having merged before they compile.

### Recommended Merge Order

```
01 ──┐
     ├── 02 ── 03 ──┐
     │             ├── 05 ──┐
     │   sibling ─ 04 ──────┤
     │   bug-T03            ├── (release)
     └─────────────── 06 ───┘
```

01 and 02 land first (Wave 1, parallel). 03 layers on top of 02; 04 lands in parallel with 03 once both 02 and the sibling Task 03 have merged. 05 and 06 land last to document the shipped behavior accurately.

---

## Documentation Updates

- **`docs/ARCHITECTURE.md`** — single-line update routed to `doc_maintainer` (Task 06). The startup-sequence summary at line 1444 currently reads "Show device selector (if auto_start=false)" — this becomes inaccurate when `auto_launch` is added.
- **`docs/CONFIGURATION.md`** — substantive rewrite of the Auto-Start Behavior section + new entry under Behavior Settings. Implementor-routed (Task 05) since CONFIGURATION.md is not in the doc_maintainer-only list per `docs/DEVELOPMENT.md`.
- **`docs/DEVELOPMENT.md`, `docs/CODE_STANDARDS.md`** — unaffected (no new build steps, no new patterns or layer crossings).
- **`docs/TESTING.md`** — optional follow-up to add a regression test note (Test K?) for the new gate. Not required to ship; flagged in Task 05 as a suggested follow-up.

---

## Verification (run once after all six tasks merge)

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### Manual Smoke Tests

1. **Repro from BUG.md** — in `example/app2`:
   - `.fdemon/launch.toml` has no `auto_start = true` configs.
   - `.fdemon/config.toml` has no `auto_launch` line.
   - `.fdemon/settings.local.toml` has a non-empty `last_device`.
   - Run `fdemon` → **expect:** New Session dialog appears. Cached device pre-selected.
   - Migration `info!` appears in the fdemon log file.

2. **Opt-in cache-based auto-launch** — same `example/app2`:
   - Add `auto_launch = true` under `[behavior]` in `config.toml`.
   - Run `fdemon` → **expect:** auto-launches on cached `last_device`.

3. **Per-config wins over cache opt-in** — same as #2 plus:
   - Add `auto_start = true` to one of the `launch.toml` configs.
   - Run `fdemon` → **expect:** that config's `device` field is honored (Tier 1), cache is ignored.

4. **Headless backwards compat** — in any project:
   - No `auto_launch`, no `auto_start = true` config, no cached device.
   - Run `fdemon --headless` → **expect:** auto-launches with the first available device (option 2b semantic preserved).

5. **Headless honors launch.toml** — coordinated with sibling bug Task 03:
   - `launch.toml` has `auto_start = true` with `device = "macos"` and a macOS device connected.
   - Run `fdemon --headless` → **expect:** session spawns on the macOS device (Tier 1).

6. **Settings Panel toggle** — TUI:
   - Open Settings (`S` key) → Behavior tab → toggle `auto_launch` on, save.
   - Restart fdemon (cache present) → **expect:** auto-launches (Tier 2).

---

## Risks & Mitigations

- **R1 — Sibling bug not merged:** Task 04 explicitly depends on `launch-toml-device-ignored` Task 03's `find_auto_launch_target` headless wiring. If sibling stalls, Task 04 either waits or absorbs the wiring (doubling its scope). *Mitigation:* coordinate merge order; prefer waiting.
- **R2 — Existing users relying on `c5879fa` cache-auto-launch:** their fdemon will stop auto-launching until they add the new flag. *Mitigation:* migration `info!` log explains the new opt-in; `docs/CONFIGURATION.md` rewrite documents it; example fixture shows it.
- **R3 — Test churn in `handler/tests.rs`:** updating all `Message::StartAutoLaunch` constructions to include the new field is mechanical but touches many tests. *Mitigation:* Task 02 owns this churn in one PR; downstream tasks see a stable signature.
- **R4 — Settings serde compatibility:** existing `config.toml` files lacking `auto_launch` must continue to load. *Mitigation:* `#[serde(default)]` on the new field (default `false`) — no `deny_unknown_fields` is set on `BehaviorSettings`, so older/newer files both round-trip cleanly. Task 01 includes a regression test.
