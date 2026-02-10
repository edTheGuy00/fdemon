## Task: Add Settings Icon Methods to IconSet

**Objective**: Add 6 new icon methods to `IconSet` in `theme/icons.rs` — `zap()`, `eye()`, `code()`, `user()`, `keyboard()`, `save()` — with both Unicode and NerdFonts variants, for use in settings panel group headers and footer shortcuts.

**Depends on**: None

### Scope

- `crates/fdemon-tui/src/theme/icons.rs` — Add 6 new icon methods to `IconSet`

### Details

#### Missing Icons

The design reference (`tmp/redesign/settings-page-focus.tsx`) uses these icons that don't exist in `IconSet`:

| Icon | Usage | Unicode Fallback | NerdFonts Glyph |
|------|-------|-----------------|-----------------|
| `zap` | Behavior group header | `⚡` (`\u{26a1}`) | `\u{f0e7}` (nf-fa-bolt) |
| `eye` | Watcher group header | `◉` (`\u{25c9}`) | `\u{f06e}` (nf-fa-eye) |
| `code` | Editor group header | `<>` | `\u{f121}` (nf-fa-code) |
| `user` | Session/User group header | `●` (`\u{25cf}`) | `\u{f007}` (nf-fa-user) |
| `keyboard` | Footer "Tab:" shortcut | `⌨` (`\u{2328}`) | `\u{f11c}` (nf-fa-keyboard_o) |
| `save` | Footer "Ctrl+S:" shortcut | `💾` or `[S]` | `\u{f0c7}` (nf-fa-floppy_o) |

#### Implementation Pattern

Follow the existing pattern in `icons.rs`. Each icon method returns `&'static str` and matches on `self.mode`:

```rust
/// Lightning bolt icon for behavior/performance settings.
pub fn zap(&self) -> &'static str {
    match self.mode {
        IconMode::Unicode => "\u{26a1}",  // ⚡
        IconMode::NerdFonts => "\u{f0e7}", // nf-fa-bolt
    }
}
```

#### Placement

Add the new methods in the "Settings/Info Icons" section (after `info()` method, around line 151). Group them logically:

```rust
// --- Settings Group Icons ---

/// Lightning bolt for behavior/performance settings.
pub fn zap(&self) -> &'static str { ... }

/// Eye icon for watcher/observation settings.
pub fn eye(&self) -> &'static str { ... }

/// Code brackets for editor/IDE settings.
pub fn code(&self) -> &'static str { ... }

/// User icon for user/session settings.
pub fn user(&self) -> &'static str { ... }

// --- Footer Shortcut Icons ---

/// Keyboard icon for key binding hints.
pub fn keyboard(&self) -> &'static str { ... }

/// Floppy disk / save icon.
pub fn save(&self) -> &'static str { ... }
```

#### Unicode Fallback Considerations

Unicode fallbacks should be single-width characters that are commonly available:

- `zap`: `⚡` is widely supported
- `eye`: Use `◉` (`\u{25c9}`) rather than emoji 👁 — single-width and widely available
- `code`: Use `<>` (two chars) — no good single Unicode glyph exists. Alternatively `⟨⟩` (`\u{27e8}\u{27e9}`) but these may not render well. Simplest: just `<>`.
- `user`: Use `●` (`\u{25cf}`) to match the dot pattern, or `♦` (`\u{2666}`). Alternatively the simpler `*`.
- `keyboard`: `⌨` (`\u{2328}`) is standard but may not render on all terminals. Fallback to `[K]`.
- `save`: No good single-width Unicode. Use `[S]` as text fallback.

**Decision**: Prefer simple, safe fallbacks. Better to show `[S]` clearly than a mangled emoji.

### Acceptance Criteria

1. `IconSet` has 6 new public methods: `zap()`, `eye()`, `code()`, `user()`, `keyboard()`, `save()`
2. Each method returns different values for `IconMode::Unicode` and `IconMode::NerdFonts`
3. Unicode fallbacks render as single-width or clearly paired characters
4. NerdFonts glyphs use correct codepoints from the Nerd Fonts cheat sheet
5. All methods have `///` doc comments
6. `cargo check -p fdemon-tui` passes
7. `cargo clippy -p fdemon-tui` passes
8. Existing icon tests still pass

### Testing

Add tests following the existing pattern in `icons.rs`:

```rust
#[test]
fn test_settings_icons_unicode() {
    let icons = IconSet::new(IconMode::Unicode);
    assert!(!icons.zap().is_empty());
    assert!(!icons.eye().is_empty());
    assert!(!icons.code().is_empty());
    assert!(!icons.user().is_empty());
    assert!(!icons.keyboard().is_empty());
    assert!(!icons.save().is_empty());
}

#[test]
fn test_settings_icons_nerdfonts() {
    let icons = IconSet::new(IconMode::NerdFonts);
    // NerdFonts glyphs should differ from Unicode fallbacks
    assert_ne!(icons.zap(), IconSet::new(IconMode::Unicode).zap());
    // ... etc
}
```

### Notes

- **No dead_code warnings**: These icons will be used in tasks 02-06. If clippy warns about unused methods, add `#[allow(dead_code)]` temporarily with a comment `// Used in Phase 4 tasks 02-06`.
- **Existing `circle()` uses nf-fa-eye**: The `circle()` method currently maps to `\u{f06e}` (nf-fa-eye) in NerdFonts mode. The new `eye()` method should use the same NerdFonts glyph (`\u{f06e}`) but a different Unicode fallback (`◉` instead of `○`). This is intentional — they're semantically different icons that happen to share a NerdFonts glyph.
- **Consider `monitor()` for UI group**: The UI Preferences group in the design uses a monitor icon. `IconSet` already has `monitor()` — no need to add it.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/theme/icons.rs` | Added 6 new icon methods (`zap()`, `eye()`, `code()`, `user()`, `keyboard()`, `save()`) with Unicode and NerdFonts variants, added 3 new test functions |

### Notable Decisions/Tradeoffs

1. **Unicode Fallbacks**: Used simple, widely-supported Unicode characters or text fallbacks (`[S]` for save) to ensure compatibility across terminals
2. **Grouping**: Organized new methods into "Settings Group Icons" and "Footer Shortcut Icons" sections for clear categorization
3. **NerdFonts Glyph Reuse**: The `eye()` method intentionally shares NerdFonts glyph `\u{f06e}` with existing `circle()` method, but uses different Unicode fallback (`◉` vs `○`)

### Testing Performed

- `cargo check -p fdemon-tui` - Passed
- `cargo test -p fdemon-tui --lib icons` - Passed (10 tests including 3 new test functions)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Dead Code Warnings**: These methods are not yet used in the codebase. Will be used in Phase 4 tasks 02-06. No dead code warnings were raised by clippy
