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
