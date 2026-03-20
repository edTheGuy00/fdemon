## Task: Fix `device = "ios"` Mismatch in app3 Fixture

**Objective**: Align the app3 fixture's launch config with the TESTING.md documentation by changing `device = "ios"` to `device = "auto"` so the repro path works on any platform.

**Depends on**: None

**Estimated Time**: 0.5 hours

**PR Review Comments**: #4 (launch.toml:10), #5 (TESTING.md:262)

### Scope

**Files Modified (Write):**
- `example/app3/.fdemon/launch.toml`: Change `device = "ios"` to `device = "auto"` on the "Profile (Issue #25)" config
- `example/TESTING.md`: Verify Test I snippet matches updated launch.toml (it already says `device = "auto"`, so this may just need confirming — no change if already correct)

**Files Read (Dependencies):**
- None

### Details

#### Current State

**Actual file** (`example/app3/.fdemon/launch.toml`):
```toml
[[configurations]]
name = "Profile (Issue #25)"
device = "ios"        # <-- hardcoded to iOS
mode = "profile"
auto_start = true
```

**TESTING.md Test I snippet** (line 262):
```toml
[[configurations]]
name = "Profile (Issue #25)"
device = "auto"       # <-- says auto
mode = "profile"
auto_start = true
```

The mismatch breaks the documented repro path on non-iOS setups. The `"ios"` value is an artifact of the author's dev machine, not an intentional constraint — the other three configs in the same file all use `device = "auto"`, and the bug being reproduced (VM Service polling pressure in profile mode) is platform-independent.

#### Fix

**Step 1**: In `example/app3/.fdemon/launch.toml`, change:
```toml
device = "ios"
```
to:
```toml
device = "auto"
```

**Step 2**: Verify `example/TESTING.md` Test I snippet already says `device = "auto"` — it does, so no change needed there.

### Acceptance Criteria

1. `example/app3/.fdemon/launch.toml` "Profile (Issue #25)" config uses `device = "auto"`
2. `example/TESTING.md` Test I snippet matches the actual file
3. All four configs in launch.toml use `device = "auto"`

### Notes

- The TESTING.md Test E snippet (lines 131-146) also shows a different config set than the actual file — this is a pre-existing issue from before this PR and is out of scope for this fix. It can be addressed separately if needed.
- This is a fixture/documentation fix only — no Rust code changes.
