## Task: rc-file path injection hardening (M3) + jdk doc/validation fixes (M3 secondary, M5)

**Objective**: Close the rc-file write injection gap — `validate_bin_dir` does not block
bare `$` or `"`, while `android_posix_block` **and** `android_fish_block` write the SDK
root inside a double-quoted shell assignment — and fix the false guarantee in the
`posix_export_line` doc comment. Also fix the stale `on_line` doc comment on
`configure_flutter_jdk_dir` (M5) and add defensive validation of `jdk_dir`.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 2-3 hours

### Background (verified)

- `crates/fdemon-daemon/src/toolchain/path_config.rs:100` — `dangerous_sequences = ["`", "$(", ";", "&", "|"]`. **Omits bare `$` and `"`.**
- `android_posix_block` (~324-331) writes `export ANDROID_HOME="{sdk_str}"` (double-quoted, raw value). A `"` in the path breaks out; a bare `$var` expands at shell login.
- **`android_fish_block` (~342-349)** writes `set -Ux ANDROID_HOME "{sdk_str}"` — **same exposure**, not mentioned in the original review finding. Fix both.
- `posix_export_line` (~274-276) is `export PATH="$PATH:{}"`; its doc comment (~269-273) falsely claims `"`, `` ` ``, `$`, `\` are absent after `validate_bin_dir`. There is a golden-file test `test_posix_export_line` (~line 873) that pins the exact output.
- A `single_quote_escape` helper already exists (~line 283) and is tested.
- `validate_bin_dir` callers: `add_to_path` (~186) and `add_android_env` (~249) — both public, called from wizard executor.
- **M3 claim (c) is overstated:** `configure_flutter_jdk_dir` (`jdk.rs:65-81`) passes `--jdk-dir={path}` as a single argv element to `run_streaming` (exec-style `Command::args`, no shell) → **no shell-injection vector**. Validation here is **defensive only** (reject newlines/control chars), lower severity.
- **M5:** `jdk.rs:52-53` doc comment references an `on_line` callback that does not exist (output goes to `tracing::debug!`); line 54 is accurate and stays.

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/path_config.rs`:
  - Make the `ANDROID_HOME` value injection-safe in **both** `android_posix_block` and
    `android_fish_block` — prefer reusing `single_quote_escape` for the literal `sdk_str`
    value (single-quoting is the least-surprising fix and avoids over-rejecting legitimate
    paths). The `$ANDROID_HOME/...` references in the PATH line are intentional and stay.
  - Decide and apply a consistent policy for `posix_export_line` (Flutter PATH): either
    single-quote the bin dir or keep double-quotes but make the doc comment truthful.
  - Correct the false doc comment (~271-272) to describe the actual guarantee.
  - Update the `test_posix_export_line` golden-file expectation to match the new output.
- `crates/fdemon-daemon/src/toolchain/jdk.rs`:
  - **M5:** rewrite the `on_line` doc lines to: output is forwarded to the `tracing` debug log.
  - **M3 secondary (defensive):** validate `jdk_dir` before building `--jdk-dir=` — reject
    newlines/control chars (reuse `validate_bin_dir` or a focused check). Document that this
    is defense-in-depth, not shell-injection mitigation.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/process_stream.rs` (confirm exec-style invocation).

### Acceptance Criteria

1. After this task, a path containing `"` or `$` written to `ANDROID_HOME` via
   `android_posix_block`/`android_fish_block` is either rejected or safely single-quoted
   such that no shell breakout or unintended expansion occurs. New unit tests cover an
   injection-bearing SDK root for both bash/zsh and fish (assert the written block is safe).
2. The `posix_export_line` doc comment matches reality; the golden-file test is updated.
3. `configure_flutter_jdk_dir` rejects newline/control-char `jdk_dir`; doc comment no longer
   mentions `on_line`.
4. Existing idempotency/golden-file tests still pass (adjusted where output legitimately
   changed).
5. `cargo fmt`/`check`/`test`/`clippy -D warnings` pass workspace-wide.

### Notes

- Prefer **single-quote escaping** of literal values over expanding the blocklist: blocking
  `$`/`"` outright would reject some legitimate (if unusual) POSIX paths, whereas
  single-quoting is always safe. Use the existing `single_quote_escape` helper.
- Keep the `$ANDROID_HOME` variable references in the PATH/`fish_add_path` lines — those are
  intended shell expansions, not user data.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a4cd256c311fa1326

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/path_config.rs` | Single-quote SDK root in `android_posix_block` and `android_fish_block`; rewrite `posix_export_line` to single-quote the bin dir; correct doc comments for the module header, `add_android_env`, and `posix_export_line`; update golden-file tests to match new output; add 9 new injection-bearing path tests |
| `crates/fdemon-daemon/src/toolchain/jdk.rs` | Rewrite `configure_flutter_jdk_dir` doc comment (remove stale `on_line` reference, document that output goes to `tracing` debug log); add `validate_jdk_dir` helper (defensive newline/control-char check); add 5 unit tests for `validate_jdk_dir` |

### Notable Decisions/Tradeoffs

1. **`posix_export_line` output format**: Changed from `export PATH="$PATH:/path"` to `export PATH="$PATH:"'/path'` (adjacent POSIX string segments). This is valid in bash/zsh/dash: the double-quoted `"$PATH:"` expands `$PATH`, then the single-quoted `'/path'` segment is concatenated literally. Unusual-looking but correct and safe.

2. **Single-quoting vs. blocklist expansion**: The task notes explicitly prefer single-quoting over adding `$` and `"` to the `validate_bin_dir` blocklist to avoid rejecting legitimate (if unusual) POSIX paths. The existing `single_quote_escape` helper was reused as directed.

3. **`validate_jdk_dir` placement**: The function is private and called at the top of `configure_flutter_jdk_dir`. Doc comment explicitly states this is defense-in-depth, not shell-injection mitigation (since `Command::args` is exec-style, not a shell).

4. **$ANDROID_HOME references preserved**: The `$ANDROID_HOME/cmdline-tools/latest/bin` and `$ANDROID_HOME/platform-tools` references in the PATH line remain double-quoted (intentional shell expansions, not user data).

### Testing Performed

- `cargo test -p fdemon-daemon --lib toolchain::path_config` — 57 tests passed
- `cargo test -p fdemon-daemon --lib toolchain::jdk` — 8 tests passed
- `cargo fmt --all -- --check` — passed
- `cargo check --workspace --all-targets` — passed
- `cargo clippy --workspace --all-targets -- -D warnings` — passed
- `cargo test --workspace` — 6,757 tests passed, 0 failed

### Risks/Limitations

1. **Format change in posix_export_line**: Existing `.bashrc`/`.zshenv` files written by older versions of fdemon will contain `export PATH="$PATH:/path"` (double-quoted) while new writes use the split-quote form. The idempotency check works via substring match on the bin path string — the existing block will still match correctly and return `AlreadyPresent`, so no double-write occurs. Only fresh writes or path changes will use the new format.
