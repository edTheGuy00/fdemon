## Task: Update Checked-In `.fdemon/config.toml` Fixtures to Real Docs URL

**Objective**: Replace the placeholder `github.com/example/flutter-demon#configuration` URL in the three checked-in `.fdemon/config.toml` files so they don't keep modelling the stale URL for anyone reading the repo as a template.

**Depends on**: None

**Estimated Time**: 5 minutes

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/.fdemon/config.toml`: Line 2 — replace placeholder URL.
- `example/app1/.fdemon/config.toml`: Line 2 — replace placeholder URL.
- `tests/fixtures/simple_app/.fdemon/config.toml`: Line 2 — replace placeholder URL.

**Files Read (Dependencies):**
- None.

### Details

Each of the three files has the identical second line:

```toml
# See: https://github.com/example/flutter-demon#configuration
```

Replace with:

```toml
# See: https://fdemon.dev/docs/configuration
```

Leave every other line in each file unchanged.

**Why these three files specifically (and not `example/app2`–`app5`):** the latter four examples have already been customized with topic-specific headers (e.g., `# Flutter Demon Configuration — Example App 4`) and dropped the `See:` line altogether. Only these three still carry the original placeholder boilerplate.

### Acceptance Criteria

1. All three target files contain `https://fdemon.dev/docs/configuration` on line 2.
2. None of the three files contain `github.com/example` anywhere.
3. `grep -rn "github.com/example" crates/fdemon-tui/.fdemon/ example/app1/.fdemon/ tests/fixtures/simple_app/.fdemon/` returns no hits.
4. No other lines in those files are modified (preserving the file structure and behaviour exactly).
5. Rust tests still pass: `cargo test --workspace` succeeds — no test in the repo greps these fixtures for URL content (verified by `grep -rn "github.com/example" --include='*.rs'`).

### Testing

```bash
# Confirm the replacements landed
grep -n "fdemon.dev/docs/configuration" \
  crates/fdemon-tui/.fdemon/config.toml \
  example/app1/.fdemon/config.toml \
  tests/fixtures/simple_app/.fdemon/config.toml

# Confirm no stale URL remains in fixtures
grep -rn "github.com/example" crates/ tests/fixtures/ example/

# Confirm tests are unaffected
cargo test --workspace
```

### Notes

- These three files exist as user-facing examples / dev configs / Flutter test fixture configs. They are read at runtime by `fdemon` itself (parsing TOML), not by any test that asserts on the comment text. Verified by `grep -rn "github.com/example" --include='*.rs'` returning zero matches in tests.

---

## Completion Summary

**Status:** Done
**Branch:** fix/url-and-version-discrepancies

### Files Modified

| File | Changes |
|------|---------|
| `example/app1/.fdemon/config.toml` | Line 2: replaced `https://github.com/example/flutter-demon#configuration` with `https://fdemon.dev/docs/configuration` |
| `tests/fixtures/simple_app/.fdemon/config.toml` | Line 2: replaced `https://github.com/example/flutter-demon#configuration` with `https://fdemon.dev/docs/configuration` |

### Notable Decisions/Tradeoffs

1. **`crates/fdemon-tui/.fdemon/config.toml` does not exist**: The task and BUG.md reference this file, but it was intentionally deleted in a previous commit (`4f5bdf6` — "remove leaked dev-time config files and guard .gitignore"). Since the file does not exist, there is no placeholder URL to replace there, and `grep` against the non-existent path produces zero hits (satisfying acceptance criterion #3 for that path). The file was not recreated.

### Testing Performed

- `grep -n "fdemon.dev/docs/configuration" example/app1/.fdemon/config.toml tests/fixtures/simple_app/.fdemon/config.toml` - PASS (both files show the new URL on line 2)
- `grep -rn "github.com/example" crates/fdemon-tui/.fdemon/ example/app1/.fdemon/ tests/fixtures/simple_app/.fdemon/` - PASS (no hits)
- `cargo test --workspace` - PASS (5,573 tests passed across all crates, zero failures)

### Risks/Limitations

1. **Third file absent**: `crates/fdemon-tui/.fdemon/config.toml` was deleted prior to this task. The two remaining fixture files are now updated. If the fdemon-tui dev config is ever re-added to the repo, it should use the correct URL from the start.
- This is independent of Task 01: Task 01 fixes the *generator* so newly-created configs use the right URL; this task fixes *checked-in* configs so the placeholder doesn't keep getting copy-pasted by anyone using the repo as a reference.
