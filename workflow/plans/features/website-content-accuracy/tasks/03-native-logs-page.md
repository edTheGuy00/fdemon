## Task: Fix Native Logs page TOML/defaults + add "process orchestrator" section

**Objective**: Fix the broken native_logs TOML examples and add a new featured section
presenting custom sources as a process orchestrator (boot a backend, health-check it,
then launch Flutter — all in one log view).

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `website/src/pages/docs/native_logs.rs`: TOML/default corrections + a new "Boot your
  whole stack" section.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/types.rs`: `CustomSourceConfig` (~872), `ReadyCheck`
  (~738), `NativeLogsSettings` (~987).
- `crates/fdemon-core/src/types.rs:831`: `OutputFormat`.
- `crates/fdemon-daemon/src/native_logs/custom.rs`: "no shell expansion" execution.

### Details

**Part 1 — Fix the broken TOML/defaults (mirror of T02's native_logs fixes):**
- `min_level` default → `"info"` (not `"debug"`). [D-17] (`native_logs.rs:231/237`)
- `[[native_logs.sources]]` → `[[native_logs.custom_sources]]`. [D-18] (`native_logs.rs:258-268`)
- `[native_logs.tag_levels]` → `[native_logs.tags.<TAG>]` with `min_level`. [D-20]
  (`native_logs.rs:247-250`)
- Remove `buffer_size` — field does not exist. [D-19] (`native_logs.rs:231-239`)

**Part 2 — Add a new featured section: "Boot your whole stack" (orchestrator).**
A "sleeper feature" / second differentiator worth highlighting on the site. Use the
narrative below, **with the verified corrections applied** (the draft copy has three
inaccuracies — accuracy is the whole point of this plan, so fix them).

_Narrative:_ Custom log sources (`[[native_logs.custom_sources]]`) can run **any
command**, not just log tailers — including the backend your app depends on. fdemon starts
it, waits until it's actually ready, *then* launches Flutter, and streams its output into
the same unified, tagged, filterable log view. A mini `docker-compose`/`Procfile`/
`foreman` baked into the Flutter loop — one command boots the whole stack.

_Configurable fields (all verified against `CustomSourceConfig`):_
- `name` — display name; becomes the tag in the log view + tag-filter overlay.
- `command` + `args` — any executable. **Run directly via `Command::new`, never through
  a shell** (no shell expansion / injection). ✅ `custom.rs:8`
- `working_dir` — run from anywhere (e.g. a separate backend repo). Defaults to the
  Flutter project root when omitted. ✅ `types.rs:887-890`
- `env` — inject env vars: `env = { LOG_LEVEL = "debug" }`. ✅ `types.rs:892-896`
- `start_before_app = true` — spawn during the pre-app phase, before Flutter runs.
- `ready_check` — gate the Flutter launch until the dependency is up. **Requires
  `start_before_app = true`** (validation error otherwise — `types.rs:966`). Five kinds:
  - `http` — poll a URL for 2xx. Fields: `url`, `interval_ms` (def 500), `timeout_s` (def 30).
  - `tcp` — connect to `host`:`port`. Fields: `host`, `port`, `interval_ms`, `timeout_s`.
  - `command` — run e.g. `pg_isready` until exit 0. Fields: `command`, `args`,
    `interval_ms`, `timeout_s`.
  - `stdout` — watch for a regex in process output. Fields: `pattern`, **`timeout_s`
    only** (no `interval_ms` — it watches the stream).
  - `delay` — wait N seconds. Field: **`seconds` only** (def 5).
  - **CORRECTION vs draft copy:** do *not* say "each has `interval_ms`/`timeout_s`" —
    only `http`/`tcp`/`command` have both; `stdout` has `timeout_s` only; `delay` has
    `seconds` only.
  - On timeout the check is **non-fatal** — fdemon proceeds with the launch anyway.
- `shared = true` — spawn **once** and broadcast logs to *all* sessions (persists until
  fdemon quits), instead of per-session (default `false`). ✅ `types.rs:905-911`
- `format` — `raw` (default) / `json` / `logcat-threadtime` / `syslog`, so structured
  output is parsed into levels/tags. **`syslog` is macOS-only** (rejected at parse time
  elsewhere — `types.rs:950`). ✅ format map `types.rs:2665-2668`

_Worked example (verified valid — use a placeholder path for `working_dir`):_

```toml
[[native_logs.custom_sources]]
name = "backend"
shared = true
command = "python3"
args = ["server/server.py"]
working_dir = "/path/to/your/backend-repo"
format = "raw"
start_before_app = true
ready_check = { type = "http", url = "http://127.0.0.1:8085/health", interval_ms = 500, timeout_s = 15 }
```

_Framing line:_ fdemon isn't just watching your app — it can bring up the entire
environment your app needs and prove it's healthy before the first frame renders, with
backend, app, and native logs interleaved in one screen. Removes a second terminal and the
manual "is the server up yet?" dance for full-stack Flutter devs.

### Acceptance Criteria

1. All native_logs TOML snippets parse against `NativeLogsSettings` (`custom_sources`,
   `tags.<TAG>.min_level`, no `buffer_size`, `min_level = "info"`).
2. New "Boot your whole stack" section present with the verified field list and worked
   example.
3. The `ready_check` field-per-kind detail is accurate (stdout = `timeout_s` only; delay
   = `seconds` only); `syslog` noted macOS-only; `ready_check` noted to require
   `start_before_app = true`.
4. `cd website && trunk build` compiles.

### Notes

- T02 (Configuration page) and T03 edit different files; both keep their own TOML copy.
- The orchestrator capability is also documented in `docs/CONFIGURATION.md` by T07.
- The SEO landing-copy task (S09) references this as the "second differentiator" — no
  file overlap here.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-add6722273d4ab7b7

### Files Modified

| File | Changes |
|------|---------|
| `website/src/pages/docs/native_logs.rs` | Fixed TOML/defaults (min_level → "info", custom_sources key, tags.<TAG>.min_level syntax, removed buffer_size); added "Boot Your Whole Stack" section with full field table, ready_check types table with per-kind accuracy, worked example, and framing callout |

### Notable Decisions/Tradeoffs

1. **Syslog macOS-only note**: Noted in both the format options table (existing Custom Log Sources section) and in the new Boot Your Whole Stack field table, so both entry points are accurate.
2. **Tag override syntax update**: Also fixed the Troubleshooting tip text that referenced the old `[native_logs.tag_levels]` (changed to `[native_logs.tags.<TAG>]`).
3. **Cargo check in worktree**: The worktree path causes `cargo check` to fail with a workspace resolution error when run from inside `.claude/worktrees/`. Verified compilation by temporarily copying the file to the main repo `website/` and running `cargo check` there — passes cleanly with only the pre-existing `dead_code` warning in `debugging.rs`.

### Testing Performed

- `cd /Users/ed/Dev/zabin/flutter-demon/website && cargo check` — Passed (1 pre-existing warning in debugging.rs, none in native_logs.rs)
- Manual review of all TOML snippets against `NativeLogsSettings`, `CustomSourceConfig`, `ReadyCheck` enum in source — Verified correct

### Risks/Limitations

1. **Trunk build not verified**: `trunk build` requires wasm32 target and build tools not present in this environment. `cargo check` (which validates Leptos macro expansion + type checking) passed, which is the meaningful compilation check for correctness.
2. **Worktree cargo check limitation**: Running `cargo check` directly in the worktree fails due to workspace path resolution. This is a tooling limitation of the worktree setup, not a code issue.
</content>
