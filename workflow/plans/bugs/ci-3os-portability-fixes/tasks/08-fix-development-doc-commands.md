## Task: Align DEVELOPMENT.md verification commands with CI

**Agent: doc_maintainer**

**Objective**: Update the verification-command and quality-gate sections of `docs/DEVELOPMENT.md` to include `--all-targets`, matching the actual command invocations used by `.github/workflows/ci.yml`. Eliminates local/CI drift that causes contributors to run a weaker local gate.

**Depends on**: None

**Estimated Time**: 0.25 hours

### Scope

**Files Modified (Write):**
- `docs/DEVELOPMENT.md`: Update lines 33–45 (Verification Commands + Full verification) and any other sections listing the workspace verification commands. Also confirm the per-crate command tables (lines 64–80 currently) accurately reflect what CI runs.

**Files Read (Dependencies):**
- `.github/workflows/ci.yml` — read the CI quality-gate steps as the source of truth for the canonical command list.

### Details

#### Source of truth

`.github/workflows/ci.yml` runs (per the post-`clippy-rust-191-cleanup` Wave 7 state, commit `1dd8b59`):

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets    # if --all-targets is used in CI for test; verify by reading ci.yml
cargo clippy --workspace --all-targets -- -D warnings
```

Read the actual workflow file before editing the docs to confirm the exact commands. The docs must match.

#### Current docs (lines 33–45)

The current "Verification Commands" section likely shows:

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace
```

(without `--all-targets` or `-- -D warnings`).

#### Corrected docs

Update both the "Verification Commands" table and the "Full verification" one-liner to include `--all-targets` everywhere CI uses it, and `-- -D warnings` on the clippy step:

```bash
cargo fmt --all                                              # Format all crates
cargo check --workspace --all-targets                        # Check all crates compile
cargo test --workspace                                       # Test all crates
cargo clippy --workspace --all-targets -- -D warnings        # Lint all crates (warnings = errors)
```

```bash
# Full verification (quality gate — must match CI):
cargo fmt --all && \
  cargo check --workspace --all-targets && \
  cargo test --workspace && \
  cargo clippy --workspace --all-targets -- -D warnings
```

If CI uses `cargo test --workspace --all-targets`, mirror that too. Confirm by reading `ci.yml`.

The "CI / Continuous Integration" section (lines ~155–180 currently) already lists the CI commands; verify it matches the new "local" section. They should be byte-identical to underline that local and CI are the same gate.

#### Per-crate examples (lines 64–80)

The per-crate tables currently show `cargo check -p fdemon-core`, `cargo test -p fdemon-app`, etc. These do not need `--all-targets` per-crate (the workspace-wide command is the canonical gate), but optionally a footnote can clarify that the per-crate commands are for fast iteration during development and the full workspace command is what CI runs. Use judgment — minimal edit is acceptable.

### Acceptance Criteria

1. `docs/DEVELOPMENT.md` "Verification Commands" table contains `--all-targets` on every command CI uses with `--all-targets`.
2. `docs/DEVELOPMENT.md` "Full verification" one-liner matches the CI quality-gate exactly (modulo `&&` vs separate steps).
3. `docs/DEVELOPMENT.md` "CI / Continuous Integration" section is consistent with "Verification Commands" — both list the same commands.
4. The doc edit respects `~/.claude/skills/doc-standards/schemas.md` content boundaries — DEVELOPMENT.md is for build/run/test commands and environment setup, not for narrative or design rationale.
5. No source code, workflow, or other doc files are modified.

### Testing

This is a docs-only change. Verification:

1. `cargo fmt --all -- --check` succeeds (no Rust files touched, but always run after a doc commit).
2. `cargo build --workspace` (sanity check, ensures no accidental code changes).
3. Manually run the documented "Full verification" one-liner from a clean shell to confirm it actually passes:

```bash
cargo fmt --all && \
  cargo check --workspace --all-targets && \
  cargo test --workspace && \
  cargo clippy --workspace --all-targets -- -D warnings
```

If this exits 0, the docs match the gate the project actually wants enforced.

### Notes

- This task uses the `doc_maintainer` agent (set via the `Agent:` frontmatter at the top of this file). `docs/DEVELOPMENT.md` is one of the three core docs that only `doc_maintainer` can edit.
- Read `~/.claude/skills/doc-standards/schemas.md` before making changes to verify the DEVELOPMENT.md content schema. Stay strictly within those boundaries.
- Do not introduce a new section (e.g., "Why we use `--all-targets`"). The doc is reference material, not a tutorial. Keep changes minimal — replace the commands and move on.
- After this task lands, the third Copilot review comment on PR #38 (`docs/DEVELOPMENT.md:178`) becomes resolvable.

---

## Completion Summary

**Status:** Not Started
**Branch:** _to be filled by doc_maintainer_

### Files Modified

| File | Changes |
|------|---------|
| _tbd_ | _tbd_ |

### Notable Decisions/Tradeoffs

_tbd_

### Testing Performed

- `cargo fmt --all -- --check` — _tbd_
- `cargo build --workspace` — _tbd_
- Manual run of the documented "Full verification" one-liner — _tbd_

### Risks/Limitations

_tbd_
