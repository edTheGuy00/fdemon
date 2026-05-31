# Plan: Website Content Accuracy & Multi-Launch Documentation

## TL;DR

Two tightly-coupled bodies of website work, both confined to `website/src/`:

1. **Document the shipped "UX Polish & Multi-Device Launch" features** — the multi-device
   launch picker, launch-lifecycle phases (`Preparing`/`Launching`), the jump-to-latest
   log indicator, runnable-device filtering, shimmer/spinner animations, the reload
   success flash, and the multi-session header separator. All seven are **confirmed
   implemented in the codebase** (see Research Findings) but the website never mentions
   them.

2. **Fix ~32 documentation-vs-codebase discrepancies** surfaced by a full sweep of the
   website against the actual Rust code and the canonical `docs/*.md`. 20 are HIGH
   severity (wrong facts, fabricated keys/panels, broken TOML examples, wrong defaults).

Both are website-content corrections to the same set of files, so they are planned
together to avoid merge conflicts. SEO work is tracked separately in
`workflow/plans/features/website-seo/PLAN.md`.

---

## Decisions (confirmed)

- **Multi-launch docs:** fold into existing pages (Introduction overview + the
  "Multi-Device" feature card + a Keybindings subsection) — no new route/page.
- **`docs/*.md`:** fix in parallel. Separate `doc_maintainer` tasks audit and correct
  `docs/CONFIGURATION.md`, `docs/KEYBINDINGS.md`, `docs/ARCHITECTURE.md` where they share
  the same drift (research indicated the markdown docs are largely the *correct* source
  the website lagged — these tasks verify-and-fix only what has actually drifted).
- **Sequencing vs SEO plan:** content lands first; the SEO plan's per-route
  `leptos_meta` additions (which edit the same page files) run after — see File Overlap
  Analysis in `TASKS.md`.

---

## Background

The website (`website/`) is a Leptos 0.8 **CSR/WASM** app. Doc content lives in two places:

- **`website/src/data.rs`** — structured data: `features()`, `all_keybinding_sections()`,
  changelog types. Rendered by `keybindings.rs` and `home.rs`.
- **`website/src/pages/docs/*.rs`** — long-form doc pages (`configuration.rs`,
  `devtools.rs`, `native_logs.rs`, `architecture.rs`, `installation.rs`, `mouse.rs`,
  `introduction.rs`, etc.), each a Leptos component emitting hand-written HTML.

The canonical sources of truth are the Rust code under `crates/` and the markdown docs
under `docs/`. The website has drifted from both: it predates the workspace split into
5 crates, predates the Memory DevTools panel, predates the DAP server and Flutter
Version manager, predates several config sections, and has never documented the
multi-launch UX work that has since shipped (git log shows phases 0–7 landed on
`feat/ux-polish-and-multilaunch`).

---

## Research Findings

### Shipped features confirmed (all IMPLEMENTED)

| Feature | Status | User-facing details to document (verbatim from code) |
|---|---|---|
| **Multi-device launch picker** | ✅ | New-session dialog (Connected tab) multi-select. Keys: `Space` toggle current device, `a` select-all/clear-all, `Enter` launch all checked (falls back to cursor device if none checked), `r` refresh. Footer hint: `Space select · a all · Enter launch · r refresh` and `(N selected)` suffix when ≥1 checked. Spawns one session per checked device; respects the 9-session cap. |
| **Launch lifecycle phases** | ✅ | `AppPhase` now has `Preparing` (pre-app `ready_check` polling) and `Launching` (process attached / building, before `app.started`). Labels/icons: `Preparing` → `○` `"Preparing"` (blue), `Launching` → `○` `"Launching"` (blue+bold), `Running` → `●` `"Running"` (green+bold). `Running` is now set only on the real `app.started` daemon event. Live build/readiness progress text shows next to the label. |
| **Jump-to-latest log indicator (#31)** | ✅ | Floating right-aligned pill in the log view: `↓ N new · G to jump` (shows `999+` past 999). Appears when scrolled up with unseen lines; hidden at tail. `G` / `End` jumps to live tail and clears it. |
| **Runnable-device filtering** | ✅ | `Device.is_supported` (serde default `true`). Unsupported connected devices are excluded from the dialog list and cannot be checked. |
| **Shimmer + spinner animations** | ✅ | Status labels for `Initializing`/`Preparing`/`Launching` shimmer (left-to-right color sweep, ~1.5 s). Braille spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) on device discovery (`⠋ Discovering devices…`), background refresh, and the launch-phase status glyph. |
| **Reload success flash** | ✅ | On hot-reload success the header bar briefly tints green (blends up to 35% toward success-green, decays ~quickly), driven by `last_reload_time`; suppressed when not `Running`. |
| **Multi-session header separator** | ✅ | With ≥2 sessions the header renders title → dim `─` rule → device tabs (no empty band). |

Session cap confirmed: `MAX_SESSIONS = 9` (`crates/fdemon-app/src/session_manager.rs:12`).

### Discrepancy inventory (sweep result)

20 HIGH, 7 MEDIUM, 5 LOW. Full per-item detail (IDs D-01…D-38) is preserved in the task
files; grouped here by website file:

**`website/src/data.rs` (keybindings + features)**
- **Missing keys (HIGH):** `Alt+m` toggle mouse capture [D-01]; `D` toggle DAP server
  [D-02]; `V` Flutter version panel [D-03]; `m` Memory DevTools panel [D-06].
- **Wrong mapping (HIGH):** `s` "allocation sort" is documented under Performance but is
  a **Memory** panel binding [D-07]; Performance panel is radically incomplete — missing
  `Tab/Shift+Tab`, `j/k`, `PageUp/Dn`, `Home/End`, `]`/`[`, `f`, `R`, `+/-`, `g`, `/`,
  `n/N` [D-09]; `←`/`→` "prev/next frame" is only one of three context behaviors [D-08].
- **Missing modes (HIGH/LOW):** Flutter Version mode section [D-10]; Loading mode [D-11].
- **Missing/incomplete (MEDIUM/LOW):** `w` toggle wrap [D-04]; `t` lowercase also
  toggles tag overlay [D-05].
- **Feature copy (LOW):** "monitors your `lib/` directory" implies non-configurability;
  watcher paths are configurable [D-12].

**`website/src/pages/docs/configuration.rs` (+ duplicated in `native_logs.rs`)**
- **Wrong defaults (HIGH):** `native_logs.min_level` default is `"info"`, not `"debug"`
  [D-17].
- **Broken TOML keys (HIGH):** `[[native_logs.sources]]` → real key
  `[[native_logs.custom_sources]]` [D-18]; `[native_logs.tag_levels]` → real key
  `[native_logs.tags.<TAG>]` with `min_level` field [D-20]; `buffer_size` does not
  exist in `NativeLogsSettings` [D-19]. Configs written per the current docs silently
  fail.
- **Wrong option values (HIGH):** `devtools.default_panel` lists `"layout"` (invalid);
  missing `"memory"` and `"network"` [D-16].
- **Missing config keys (MEDIUM):** `[behavior]` missing `version_check`,
  `version_check_timeout_secs` [D-13]; `[ui]` missing `icons` (Nerd Fonts vs Unicode)
  [D-14]; `[devtools]` missing ~13 keys incl. `hide_implementation_widgets`,
  `network_auto_record`, `max_network_entries`, etc. [D-15]; entire `[dap]` and
  `[flutter]` sections undocumented [D-22].
- **Verify (LOW):** `auto_start` "deprecation warning" claim [D-21].

**`website/src/pages/docs/devtools.rs`**
- **Fabricated panel/key (HIGH):** "Layout Explorer (l)" — pressing `l` in DevTools does
  nothing; layout is part of the Inspector details view [D-28].
- **Missing Memory panel (HIGH):** absent from panel nav and keybinding tables [D-26].
- **Wrong mapping (HIGH):** `s` "sort frames by duration" under Performance — no such
  binding exists [D-27].
- **Imprecise (MEDIUM):** Inspector `→ / Enter` both labeled "Expand"; `Enter` opens the
  Details view [D-29].

**`website/src/pages/docs/installation.rs`**
- **Wrong min version (HIGH):** "Rust 1.70+" — actual `rust-version = "1.77.2"`
  (`Cargo.toml:11`, CVE-2024-24576) [D-23].

**`website/src/pages/docs/mouse.rs`**
- **Fabricated keys (HIGH):** `[` / `]` described as session-cycling (twice) — sessions
  cycle with `Tab`/`Shift+Tab`; `[`/`]` are Performance detail-tab keys [D-37].
- **Phrasing (LOW):** "Shift+L" vs "L" for link mode [D-38].

**`website/src/pages/docs/architecture.rs`**
- **Wrong structure (HIGH):** shows a monolithic `src/core /app /tui …` layout — the
  project is a 5-crate workspace (`fdemon-core/-daemon/-app/-tui/-dap` + binary)
  [D-30]; phantom "Common"/"Services" layers, missing `fdemon-dap` [D-31]; all Module
  Reference file paths point at non-existent `common/ core/ config/…` folders [D-32].
- **Stale snippets (MEDIUM):** `UpdateResult` shape [D-33]; `AppState.device_selector`
  → `new_session_dialog_state` [D-34].

> Verified-correct (no action): install script URL/`edTheGuy00/fdemon` repo & Windows
> handling [D-24]; build-time version constant via `build.rs` [D-25].

---

## Affected Modules (all under `website/src/`)

- `data.rs` — keybinding sections (add missing keys/modes, fix mappings), `features()`
  copy, and a new "Multi-Device Launch" keybinding subsection.
- `pages/home.rs` + `data.rs::features()` — optionally add/adjust a feature card for
  multi-launch.
- `pages/docs/introduction.rs` — mention multi-launch + launch lifecycle in the overview.
- `pages/docs/installation.rs` — Rust min version `1.77.2`.
- `pages/docs/configuration.rs` — fix native_logs TOML/defaults, add missing keys and
  `[dap]`/`[flutter]` sections, fix `default_panel` options.
- `pages/docs/native_logs.rs` — same native_logs TOML/default fixes **plus a new
  featured "Boot your whole stack" section** documenting custom sources as a process
  orchestrator (`start_before_app` + `ready_check` + `shared`). This is a "sleeper
  feature" / second differentiator worth highlighting. All field claims verified against
  `CustomSourceConfig`/`ReadyCheck`/`OutputFormat`; three draft-copy inaccuracies are
  corrected in the task (`stdout`/`delay` ready-check fields, `ready_check` requires
  `start_before_app`, `syslog` is macOS-only). See T03.
- `pages/docs/devtools.rs` — add Memory panel, remove fabricated Layout Explorer key,
  fix `s` mapping, fix Inspector nav wording, add missing Performance keys.
- `pages/docs/mouse.rs` — fix `[`/`]` session-cycle claim, "L" phrasing.
- `pages/docs/architecture.rs` — rewrite structure/layer/module sections to the real
  5-crate workspace; refresh stale code snippets.
- (New, optional) a short "Multi-Device Launch" section on a relevant docs page
  (Introduction or a dedicated subsection of Keybindings).

> **Canonical-doc note:** The website is *not* one of the `doc_maintainer`-managed core
> docs. However, several discrepancies (esp. architecture, native_logs config syntax,
> Rust min version) also exist or could be cross-checked against `docs/ARCHITECTURE.md`,
> `docs/CONFIGURATION.md`, `docs/KEYBINDINGS.md`. This plan corrects the **website**;
> if the markdown `docs/*.md` share any of the same errors, a follow-up `doc_maintainer`
> task should be filed (see Open Questions).

---

## Development Phases

### Phase A — High-severity factual corrections (do first)
Fix everything that is actively wrong or misleading: broken native_logs TOML keys
[D-18/19/20], wrong defaults [D-17], invalid `default_panel` options [D-16], fabricated
keys/panels [D-28, D-37, D-27], wrong Rust version [D-23], wrong architecture structure
[D-30/31/32], and the missing keys that hide whole features [D-01/02/03/06].

**Milestone:** No website statement contradicts the code; copy-pasting any documented
config/keybinding works.

### Phase B — Document the shipped multi-launch / UX-polish features
Add the multi-device launch picker (keys + footer hint), launch-lifecycle phases,
jump-to-latest indicator, runnable-device filtering, animations, reload flash, and
header separator. Update `features()` / a home card if warranted, plus an Introduction
mention and a Keybindings subsection.

**Milestone:** A new user reading the site learns they can launch N devices at once and
understands the launch/running phase indicators.

### Phase C — Medium/low completeness pass
Add remaining missing config keys [D-13/14/15/22], complete the Performance keybinding
table [D-09], add Flutter Version + Loading mode sections [D-10/11], refine wording
[D-08/29/12/38], refresh stale architecture snippets [D-33/34].

**Milestone:** Keybinding/config references are complete, not just correct.

### Phase D — Verification
`cd website && trunk build` (or the project's build command) compiles; manual spot-check
of changed pages; cross-check the fixed values one more time against code. No Rust app
crates are touched, so `cargo` workspace tests are unaffected.

---

## Edge Cases & Risks

- **Risk:** Re-introducing drift. **Mitigation:** every changed value cites a
  `crate/file:line` in the task file so a reviewer can verify against source.
- **Risk:** Documenting behavior that differs by `icons`/Nerd-Font mode or terminal
  capability (e.g. glyphs `○ ● ↻`). **Mitigation:** document the Unicode-mode glyphs and
  note Nerd Fonts is the default (`UiSettings.icons`).
- **Risk:** The same errors live in `docs/*.md`. **Mitigation:** flag for a separate
  `doc_maintainer` task; this plan's scope is the website.
- **Risk:** Phase B/C touch the same files as the SEO plan (`leptos_meta` `<Title>`
  additions to page components). **Mitigation:** sequence — land content first, then SEO
  meta; or coordinate via the File Overlap Analysis in the TASKS breakdown.

---

## Success Criteria

- [ ] All 20 HIGH-severity discrepancies are corrected and each correction is traceable
      to a `crate/file:line`.
- [ ] The multi-device launch picker is documented with the exact keys
      (`Space`/`a`/`Enter`/`r`) and footer hint.
- [ ] Launch-lifecycle phases (`Preparing`/`Launching`/`Running`) and the
      jump-to-latest indicator are documented.
- [ ] `native_logs` TOML examples on the site parse against the real
      `NativeLogsSettings` (`custom_sources`, `tags.<TAG>.min_level`, no `buffer_size`,
      `min_level = "info"`).
- [ ] DevTools docs include the Memory panel and drop the fabricated Layout Explorer key.
- [ ] Architecture page reflects the real 5-crate workspace.
- [ ] Installation page states Rust `1.77.2`.
- [ ] `website` build succeeds; changed pages render correctly.

---

## Open Questions

1. Should the markdown `docs/*.md` files be corrected in parallel (separate
   `doc_maintainer` task), or is this website-only for now?
2. Do you want a **dedicated** docs page/section for multi-launch, or fold it into
   Introduction + Keybindings?
3. Should the home-page feature grid gain a 5th card (e.g. "One-key multi-device launch")
   or just update the existing "Multi-Device" card copy?

---

## References

- Feature plan: `workflow/plans/features/ux-polish-and-multilaunch/PLAN.md`
- SEO plan: `workflow/plans/features/website-seo/PLAN.md`
- Code: `crates/fdemon-app/src/session_manager.rs`, `handler/keys.rs`,
  `new_session_dialog/`, `config/types.rs`; `crates/fdemon-daemon/src/devices.rs`;
  `crates/fdemon-tui/src/widgets/{log_view,header,shimmer,spinner}.rs`
- Canonical docs: `docs/KEYBINDINGS.md`, `docs/CONFIGURATION.md`, `docs/ARCHITECTURE.md`
</content>
</invoke>
