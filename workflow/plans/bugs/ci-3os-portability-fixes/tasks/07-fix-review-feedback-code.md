## Task: Address Copilot reviewer comments — workflow comment + Windows error message

**Objective**: Resolve two of the three Copilot inline review comments on PR #38 — the inaccurate workflow comment about toolchain pinning, and the Windows-specific `InvalidInput` error message that points to the wrong `launch.toml` path. The third comment (DEVELOPMENT.md) is handled by Task 08 (`doc_maintainer`).

**Depends on**: None

**Estimated Time**: 0.5 hours

### Scope

**Files Modified (Write):**
- `.github/workflows/ci.yml`: Fix the comment near line 33 (the `dtolnay/rust-toolchain` SHA pin) so it accurately describes the supply-chain benefit and does not claim toolchain version pinning.
- `crates/fdemon-daemon/src/process.rs`: Fix the Windows `InvalidInput` error message at line 93 to reference `.fdemon/launch.toml` instead of bare `launch.toml`.

**Files Read (Dependencies):**
- None.

### Details

#### Sub-fix A: `.github/workflows/ci.yml` comment

The Copilot reviewer flagged a comment near the `dtolnay/rust-toolchain` SHA-pin that implies pinning the action SHA "freezes the Rust toolchain version." This is inaccurate — the SHA pin only freezes the *action's code*, not the rustc version. The `with: toolchain: stable` line still resolves to the latest stable rustc at workflow runtime.

Read the comment in context (lines around 33; surrounding comments may also need a tweak), then either:
- **Rewrite** the comment to honestly describe the SHA pin's benefit: action-code immutability for supply-chain hardening. Example: `# Pin to commit SHA for supply-chain immutability — the rustc version itself still tracks 'stable'.`
- **Remove** the inaccurate sentence entirely if a shorter "pinned for supply-chain hardening" comment is already present.

Do **not** change the actual `with: toolchain: stable` value or add a `rust-toolchain.toml`. Pinning the toolchain version is a separate policy decision tracked elsewhere (`workflow/plans/bugs/msrv-is-multiple-of-cleanup/BUG.md` "Further Considerations") — out of scope here.

Apply the same correction to any sibling workflow files (`.github/workflows/release.yml`, `.github/workflows/e2e.yml`, `.github/workflows/publish-site.yml`) if they have the same misleading comment pattern. Verify by grepping `.github/workflows/` for the inaccurate phrasing.

#### Sub-fix B: `crates/fdemon-daemon/src/process.rs` line 93

The Copilot reviewer flagged the Windows `InvalidInput` error message — when `Command::spawn` returns `InvalidInput` (post-CVE-2024-24576 cmd.exe escaper rejection), the user-facing error tells them to "Check launch.toml" but the actual path is `.fdemon/launch.toml`.

Replace the bare path reference with the correct relative path. Example:

```rust
// Before:
return Err(Error::process(format!(
    "Spawning flutter failed: arguments contain characters cmd.exe cannot escape. \
     Check launch.toml for unusual --dart-define values."
)));

// After:
return Err(Error::process(format!(
    "Spawning flutter failed: arguments contain characters cmd.exe cannot escape. \
     Check .fdemon/launch.toml for unusual --dart-define values."
)));
```

Read the actual error string before editing — the wording above is illustrative. Match the existing voice and only change the path.

If the same path reference appears in nearby user-facing messages or doc-comments in `process.rs`, fix them too for consistency.

### Acceptance Criteria

1. `.github/workflows/ci.yml` no longer claims SHA-pinning "freezes the Rust toolchain version." The comment accurately describes the supply-chain benefit (or is removed).
2. Sibling workflow files (`release.yml`, `e2e.yml`, `publish-site.yml`) have consistent comments — no other workflow makes the same inaccurate claim.
3. `crates/fdemon-daemon/src/process.rs` line 93 references `.fdemon/launch.toml` (not bare `launch.toml`) in the Windows `InvalidInput` user-facing message.
4. `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` exits 0.
5. `cargo test -p fdemon-daemon` passes (no test asserts the exact error wording, but verify).
6. `cargo fmt --all -- --check` is clean.
7. The Copilot reviewer comments on PR #38 are resolvable by these edits — mark them resolved on GitHub after merge.
8. The third Copilot comment (DEVELOPMENT.md `--all-targets`) is **not** addressed here — it is Task 08's scope.

### Testing

```bash
cargo build --workspace        # quick sanity check
cargo test -p fdemon-daemon    # nothing asserts the error string, but no regression
```

For the workflow change there is no automated test — the next CI run on this branch is the verification.

### Notes

- Resist the temptation to actually pin the toolchain version (e.g., `with: toolchain: 1.77.2`). That is a meaningful policy change with implications for the developer workflow (forcing every contributor to use 1.77.2 locally) and is tracked separately.
- If the workflow comment lives in a multi-line block, a one-line edit is preferred over a full rewrite — match the existing comment style.
- The `process.rs` message is user-facing diagnostic text. It should be terse and actionable. Do not turn it into a multi-paragraph explanation.

---

## Completion Summary

**Status:** Not Started
**Branch:** _to be filled by implementor_

### Files Modified

| File | Changes |
|------|---------|
| _tbd_ | _tbd_ |

### Notable Decisions/Tradeoffs

_tbd_

### Testing Performed

- `cargo clippy --workspace --all-targets -- -D warnings` — _tbd_
- `cargo test --workspace` — _tbd_
- `cargo fmt --all -- --check` — _tbd_

### Risks/Limitations

_tbd_
