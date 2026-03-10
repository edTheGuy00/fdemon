## Task: Documentation Updates

**Objective**: Update project documentation to reflect phase 2 additions — iOS native log capture, per-tag filtering, and the `T` keybinding.

**Depends on**: 05-app-ios-integration, 09-per-tag-filter-ui

### Scope

- `docs/ARCHITECTURE.md`: Update native log module documentation, add iOS to platform table
- `docs/KEYBINDINGS.md` (if exists): Add `T` keybinding for tag filter
- `CLAUDE.md`: Update project overview if needed (e.g., test counts)

### Details

#### 1. Update `docs/ARCHITECTURE.md`

Update the native logs module section to include iOS:

**Platform support table:**

```markdown
| Platform | Mechanism          | Module        |
|----------|--------------------|---------------|
| Android  | `adb logcat`       | `android.rs`  |
| macOS    | `log stream`       | `macos.rs`    |
| iOS (sim)| `simctl log stream`| `ios.rs`      |
| iOS (phy)| `idevicesyslog`    | `ios.rs`      |
| Others   | Not needed (pipe)  | —             |
```

**Tool availability section:**

Add `idevicesyslog` and iOS-specific tool checks to the tool availability documentation.

**Module reference:**

Add `ios.rs` to the `fdemon-daemon/src/native_logs/` module listing with a brief description.

#### 2. Update keybindings documentation

If `docs/KEYBINDINGS.md` exists, add:

```markdown
| `T` | Open/close native tag filter overlay | Log view |
```

If the keybindings are documented elsewhere (e.g., in `CLAUDE.md` or a help screen), update there.

#### 3. Update `CLAUDE.md` if needed

If test counts have changed significantly, update the "Testing" section. Example:

```markdown
- `crates/fdemon-daemon/src/` - XXX unit tests (was 375, now includes iOS capture tests)
```

Also update the "Keyboard Shortcuts Summary" in the PLAN.md if needed.

#### 4. Update the PLAN.md success criteria

Mark phase 2 items as complete:

```markdown
### Phase 2 Complete When:
- [x] iOS native logs captured on physical devices and simulators
- [x] Per-tag filter UI allows toggling individual tags
- [x] Per-tag priority thresholds configurable
- [x] Works across iOS 15+ / Xcode 15+ (graceful degradation for older versions)
```

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` includes iOS native log capture documentation
2. Keybinding `T` is documented
3. Platform support table includes iOS (simulator + physical) rows
4. `idevicesyslog` tool dependency is documented
5. All documentation is accurate and consistent with the implementation
6. No broken links or references

### Testing

- Review all documentation changes for accuracy
- Verify keybinding documentation matches actual implementation
- `cargo check --workspace` still compiles (no code changes in this task)

### Notes

- This is a documentation-only task. No code changes.
- Keep documentation concise — follow existing doc style.
- If `docs/KEYBINDINGS.md` doesn't exist, add the keybinding info to wherever keybindings are currently documented (check `CLAUDE.md` "Keyboard Shortcuts Summary" section in the PLAN.md).
