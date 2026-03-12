## Task: Fix TESTING.md and App4 Config Consistency

**Objective**: Resolve path mismatches between `example/TESTING.md` documentation and the actual `example/app4/.fdemon/config.toml` so developers following the test guide see expected behavior.

**Depends on**: None

**Priority**: Should Fix

### Scope

- `example/TESTING.md`: Fix path references in Test C and directory structure section
- `example/app4/.fdemon/config.toml`: Fix typo in non-existent path entry

### Details

The codebase researcher found 3 discrepancies:

#### Discrepancy 1: `../../shared_lib` vs `../shared_lib`

TESTING.md Test C shows:
```toml
paths = ["lib", "../../shared_lib", "../app1/lib"]
```

But `app4/.fdemon/config.toml` actually contains:
```toml
paths = ["lib", "../shared_lib", "../app1/lib", "../non-existant"]
```

Since `app4/` is one level inside `example/`, `../shared_lib` correctly reaches `example/shared_lib/`. The TESTING.md path `../../shared_lib` would go up two levels to the repo root, which is wrong.

**Fix**: Update TESTING.md to match the actual config: `../shared_lib`.

#### Discrepancy 2: Missing `../non-existant` in TESTING.md

The actual config has a 4th path `"../non-existant"` (note typo: "existant" → "existent") which is not documented in TESTING.md Test C.

**Fix**: Fix the typo to `"../nonexistent"` in the config, and add it to TESTING.md Test C with a note that it exercises the non-existent path warning behavior.

#### Discrepancy 3: Test G overlap

TESTING.md Test G says to "temporarily add a non-existent path" to test the warning behavior, but `../non-existant` already exists permanently in the config. Test G's instruction is misleading.

**Fix**: Update Test G to reference the existing non-existent path entry rather than asking users to add a temporary one. Or remove the permanent bad path from config and keep Test G as a manual edit test.

### Acceptance Criteria

1. TESTING.md Test C path matches actual `app4/.fdemon/config.toml`
2. The typo `non-existant` is corrected to `nonexistent` (or equivalent)
3. TESTING.md directory structure section at the bottom matches actual configs
4. Test G instructions are consistent with the actual config state

### Testing

No code tests needed — this is documentation-only. Manual review of TESTING.md against actual config files.

### Notes

- Decide whether the non-existent path should be permanent in the config (simpler) or only added during Test G (more explicit). Permanent is recommended since it exercises the warning path automatically.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `example/app4/.fdemon/config.toml` | Fixed typo `"../non-existant"` -> `"../nonexistent"`; updated header comment to document the intentional missing path |
| `example/TESTING.md` | Test C: corrected `../../shared_lib` -> `../shared_lib`, added `../nonexistent` 4th path entry with explanatory note, updated steps and expected result; Test G: removed "temporarily add" manual-edit instruction, replaced with reference to permanent entry; Directory Structure: updated app4 config.toml comment to show all 4 paths; fixed shared_lib comment from `../../` to `../` |

### Notable Decisions/Tradeoffs

1. **Permanent non-existent path**: Kept `../nonexistent` as a permanent entry in the config (not a manual-edit test artifact), matching the task recommendation. This means Test C now covers both valid path resolution and the warning-on-missing-path behavior in one run, and Test G requires no config editing.

### Testing Performed

- `cargo fmt --all` - Passed (no output, no changes to Rust code)
- `cargo check --workspace` - Passed (compiled cleanly in 1.30s)

### Risks/Limitations

1. **Documentation-only change**: No Rust code was modified, so there is no risk of regression. The only risk is human error in the doc text, which was verified by reading back both files after editing.
