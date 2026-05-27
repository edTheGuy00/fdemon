## Task: Validate `extra_args` at the build chokepoint (optional / defense-in-depth)

**Objective**: Add lax validation of `extra_args` at the single point where they are consumed for process spawning, so malformed entries from `.fdemon/launch.toml` or `.vscode/launch.json` cannot reach `Command` — especially relevant now that one confirm fans out to N processes.

**Depends on**: None

**Estimated Time**: 1–2h

**Addresses review item**: m7 (security advisory — HIGH in isolation, MINOR under local-developer trust model)

**Priority**: Low / Optional. There is no shell-injection risk (args reach `Command::args()` as separate, non-shell-evaluated elements) and the config files are within the developer's own trust zone. This task is defense-in-depth; defer with a note in TASKS.md if scope is tight.

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/config/launch.rs`: filter/validate `extra_args` inside `build_flutter_args`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/types.rs`: `LaunchConfig.extra_args: Vec<String>` (line ~42).

### Details

`extra_args` flow into the spawned command at the single chokepoint `build_flutter_args` (`config/launch.rs:306–337`), which appends them raw:

```rust
// Add extra args
args.extend(self.extra_args.clone());  // ~line 334
```

Both config sources funnel through here regardless of origin:
- `.fdemon/launch.toml` → `load_launch_configs` (serde, no validation).
- `.vscode/launch.json` → `vscode.rs::parse_tool_args` pushes unrecognized args to `extra_args` (no validation).

**Replace the raw `extend` with a validated filter** — a conservative allowlist that drops obviously malformed entries and logs them, without rejecting legitimate Flutter flags:

```rust
/// Maximum accepted length of a single extra CLI argument.
/// Generous cap to catch accidental file-content injection without
/// rejecting realistic flags (e.g. long --split-debug-info paths).
const MAX_EXTRA_ARG_LEN: usize = 256;

// Add extra args (validated): must look like a flag, no NUL bytes, bounded length.
for arg in &self.extra_args {
    let ok = arg.starts_with('-') && !arg.contains('\0') && arg.len() <= MAX_EXTRA_ARG_LEN;
    if ok {
        args.push(arg.clone());
    } else {
        tracing::warn!("Ignoring malformed extra_arg: {:?}", arg);
    }
}
```

> Decide `starts_with('-')` vs `starts_with("--")`. Some valid Flutter args are single-dash (`-d`, `-t`, `--`). Prefer `starts_with('-')` to avoid false rejects; the goal is to block free-text / file-content injection, not to enforce a strict grammar.

### Acceptance Criteria

1. `build_flutter_args` drops `extra_args` entries containing NUL bytes, exceeding `MAX_EXTRA_ARG_LEN`, or not starting with `-`, and logs each drop at `warn`.
2. Well-formed flags (`--obfuscate`, `--split-debug-info=build/symbols`, `-d`, `-t lib/main.dart`) pass through unchanged.
3. `MAX_EXTRA_ARG_LEN` is a named constant with a derivation comment (per CODE_STANDARDS Principle 4).
4. Unit tests cover: a valid flag passes, a NUL-containing arg is dropped, an over-length arg is dropped, a non-dash free-text arg is dropped.
5. `cargo test -p fdemon-app` and `cargo clippy --workspace --all-targets -- -D warnings` pass.

### Notes

- Validate at `build_flutter_args` (not at load time) because it is the tightest single chokepoint covering both config sources for every spawn.
- Do not change the on-disk config schema or reject whole configs — silently dropping individual malformed args with a warn log is the least-surprising behavior for a local tool.
- If deferring: update `TASKS.md` to mark this task `Deferred` with the trust-model rationale rather than deleting it.
