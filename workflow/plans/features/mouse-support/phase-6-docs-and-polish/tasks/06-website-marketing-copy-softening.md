## Task: Website Marketing Copy — Soften Mouse-Disparaging Lines

**Objective**: Replace two lines of website marketing copy that actively disparage mouse use with neutral, keyboard-first phrasing that does not contradict the newly-shipped mouse support.

**Depends on**: None

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `website/src/pages/home.rs` (line ~75): replace `"Designed for power users who prefer the keyboard over the mouse."` with neutral keyboard-first copy.
- `website/src/pages/docs/introduction.rs` (line ~25): replace the `FeatureCard` text `"Vim-style navigation, search, and controls. Never reach for the mouse."` with neutral copy.

**Files Read (Dependencies):**
- None — this is pure copy editing.

### Details

#### `website/src/pages/home.rs` line 75

Current:

```rust
<p class="text-slate-400">
    "Designed for power users who prefer the keyboard over the mouse."
</p>
```

Replacement (suggested — author may pick equivalent neutral copy):

```rust
<p class="text-slate-400">
    "Keyboard-first ergonomics, with optional mouse for clickable surfaces."
</p>
```

Or:

```rust
<p class="text-slate-400">
    "Designed for keyboard-first power users — mouse support is opt-in."
</p>
```

The replacement must (a) preserve the keyboard-first identity that the page is selling and (b) not claim mouse is unsupported / unavailable / discouraged.

#### `website/src/pages/docs/introduction.rs` line 25

Current:

```rust
<FeatureCard
    icon=|| view! { <Terminal class="w-5 h-5 text-blue-400" /> }.into_any()
    title="Keyboard-First"
    text="Vim-style navigation, search, and controls. Never reach for the mouse."
/>
```

Replacement:

```rust
<FeatureCard
    icon=|| view! { <Terminal class="w-5 h-5 text-blue-400" /> }.into_any()
    title="Keyboard-First"
    text="Vim-style navigation, search, and controls. Mouse is supported as an opt-in second input."
/>
```

Or shorter:

```rust
text="Vim-style navigation, search, and controls. Mouse support is opt-in."
```

The `Keyboard-First` card title stays — it accurately reflects the project's identity.

### Acceptance Criteria

1. `grep -n "Never reach for the mouse" website/src/` returns zero results.
2. `grep -n "prefer the keyboard over the mouse" website/src/` returns zero results.
3. The replacement copy preserves the keyboard-first messaging and references mouse as supported (opt-in is acceptable framing).
4. No other copy on either page is modified — feature lists, CTAs, screenshots, headings remain unchanged.
5. `cargo check -p website` (or the project's existing build command) succeeds. The website still compiles after the string edits.
6. Visual review: the two updated `<p>` / `<FeatureCard>` blocks render the new copy without layout regressions.

### Testing

```bash
# Verify replacements:
grep -ni "mouse" website/src/pages/home.rs website/src/pages/docs/introduction.rs

# Build verification (run from /Users/ed/Dev/zabin/flutter-demon/website):
cd website && cargo check
```

### Notes

- The literal replacement strings are suggestions — the author may pick equivalents as long as the acceptance criteria are met.
- Do not rewrite the surrounding card grid layout or paragraph structure. The change is text-only.
- Do not add a new feature card titled "Mouse Support" on the introduction page — that surface is sold through the keybindings/mouse docs page (Tasks 07/08), not the marketing intro grid. The `Keyboard-First` card is correctly positioned to mention mouse-as-opt-in in its body text.
- Tailwind classes and the `FeatureCard` component shape stay identical.
