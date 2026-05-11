## Task: CODE_STANDARDS.md Region Registry Pattern

**Agent:** doc_maintainer

**Objective**: Add a "Region Registry Pattern" subsection to `docs/CODE_STANDARDS.md` under the existing Responsive Layout Guidelines / TEA-exception material, citing the `Cell<usize>` render-hint precedent (lines 434–476) and codifying the call-site comment style for `Cell<MouseRegions>` writes.

**Depends on**: None

**Estimated Time**: 0.75h

### Scope

**Files Modified (Write):**
- `docs/CODE_STANDARDS.md`: Add a new subsection (~30–60 lines) titled "Region Registry Pattern" near the existing Cell-based render-hint exception (around line 476's "TEA exception note"). Cross-link to `docs/ARCHITECTURE.md`'s Mouse Region Registry section.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- `crates/fdemon-app/src/mouse_regions.rs` — `MouseRegions`, `MouseRegionsCell`, `MouseRegionsBuilder`, `MouseRegionGuard`.
- `crates/fdemon-app/src/state.rs` — `mouse_regions: MouseRegionsCell` field on `AppState`.
- `crates/fdemon-tui/src/render/mod.rs` — `MouseCtx` thread-through pattern, `take_guard()` usage at frame start.

### Change Context

`docs/CODE_STANDARDS.md` Principle 3 establishes `Cell<usize>` as a pragmatic TEA exception for render-hint feedback (e.g. `last_known_visible_height` written by the renderer, read by the handler). The mouse region registry uses the same exception class but with a richer payload (a `MouseRegions` struct, taken/restored via RAII guard). Without an explicit pattern entry, future contributors will not know:

- That this is an approved exception (not a code smell).
- That production code must use `take_guard()`, not `take()` + `set()` (panic-safety).
- That `// EXCEPTION:` annotations are required at every cell-write site.
- That the registry is purely a render-hint — no business logic, no state-equality participation.

### Details

Add the subsection in the same style as the existing `Cell<usize>` writeup. Suggested structure:

#### "Region Registry Pattern (Approved TEA Exception)"

Lead paragraph: what the registry is, where it lives (`AppState::mouse_regions: MouseRegionsCell`), why it is a TEA exception (it is mutated during render, read during the next event-handling cycle).

Subsection: **Why the exception is approved**

- Mirrors the `Cell<usize>` precedent (Principle 3, lines 434–476) — single render-hint value, no business logic, no state-equality participation, not part of any `EngineEvent`.
- The `MouseRegions` content is regenerated every frame; persistence across frames is intentional only because the next click event arrives between two `view()` calls.

Subsection: **How to use it correctly**

- Production code uses `MouseRegionsCell::take_guard()` (RAII) to take the registry; the `MouseRegionGuard` puts it back on `Drop`, even on panic. Never use `take()` + `set()` in production — only in unit tests where guard semantics get in the way.
- The renderer constructs `MouseCtx::new(regions.builder())` and threads `Option<&mut MouseCtx<'_>>` through widget render functions. Widgets call `ctx.click(...)`, `ctx.click_at_z(...)`, or `ctx.click_left_middle(...)`.
- When the active `UiMode` is a modal (`Startup`, `NewSessionDialog`, `ConfirmDialog`, `Settings`, `LinkHighlight`, `FlutterVersion`) or `tag_filter_visible` is set, the renderer passes `None` as the `MouseCtx` to base-UI widgets so no base-UI z=0 regions are registered. This is the documented mechanism for modal precedence (see ARCHITECTURE.md "Modal Precedence and Sub-Modal Gates").

Subsection: **Annotation requirement**

Every cell-write site must carry an `// EXCEPTION:` annotation matching the existing `Cell<usize>` style:

```rust
// EXCEPTION (TEA): mouse_regions is a render-hint cell. See docs/CODE_STANDARDS.md
// "Region Registry Pattern" and docs/REVIEW_FOCUS.md approved-exceptions list.
let mut guard = state.mouse_regions.take_guard();
```

Subsection: **Anti-patterns**

| Anti-pattern | Why it's wrong | Correct alternative |
|---|---|---|
| Using `take()` + `set()` directly in production code | Panic between take and set leaves the registry empty for the rest of the session | `take_guard()` (RAII) |
| Calling `regions.hit_test(...)` with manual `z_index >= 1` filter for modal precedence | Renderer-level suppression is the canonical mechanism (Phase 5.5); manual filtering duplicates and risks drift | Trust the renderer; pass `None` MouseCtx in modal modes |
| Storing business state on the registry | The registry is a render hint, not state | Add real state to `AppState` and emit messages |
| Writing without the `// EXCEPTION:` annotation | Reviewers cannot distinguish intentional Cell writes from accidental ones | Add the annotation |

Cross-reference at the end: link to `docs/ARCHITECTURE.md` "Mouse Region Registry" for the type-level details, and `docs/REVIEW_FOCUS.md` "Approved TEA Exception → Current usage" for the exhaustive list.

### Acceptance Criteria

1. A subsection titled "Region Registry Pattern" (or near-equivalent) exists in `docs/CODE_STANDARDS.md`, placed adjacent to the existing `Cell<usize>` render-hint exception material.
2. The subsection covers: what the pattern is, why the TEA exception is approved, the `take_guard()` requirement, the `MouseCtx` thread-through, the modal-precedence renderer-level approach, the annotation requirement, and at least three anti-patterns.
3. Cross-references to `docs/ARCHITECTURE.md` and `docs/REVIEW_FOCUS.md` are present and resolve.
4. No code-example block exceeds the existing project's example length (rough match to the `Cell<usize>` block at lines 442–476).
5. No content boundary violations — architecture details (registry type definitions, hit-test algorithm) stay in ARCHITECTURE.md; this doc only standardizes how to *use* the pattern.

### Testing

```bash
# Cross-references resolve:
grep -n "Region Registry Pattern" docs/CODE_STANDARDS.md
grep -n "Mouse Region Registry" docs/ARCHITECTURE.md
grep -n "Approved TEA Exception" docs/REVIEW_FOCUS.md
```

### Notes

- Mirror the existing `Cell<usize>` writeup (lines 442–476) for tone, length, and code-fence style.
- Do not duplicate ARCHITECTURE.md content. The two docs are siblings: ARCHITECTURE describes structure; CODE_STANDARDS prescribes usage.
- The `// EXCEPTION:` annotation language is enforced — match the existing style verbatim where possible.
- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `docs/CODE_STANDARDS.md` | Added "Region Registry Pattern (Approved TEA Exception)" subsection (~65 lines) after the Principle 3 TEA exception note, before Principle 4. Covers: what the pattern is, why approved, `take_guard()` RAII requirement, `MouseCtx` thread-through, modal-precedence renderer-level approach, annotation requirement, four anti-patterns, and cross-references. |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: N/A

### Notable Decisions/Tradeoffs

1. **Placement adjacent to Principle 3**: The new subsection sits directly after the Principle 3 TEA exception note (line 481), before Principle 4, making the `Cell<usize>` → `Cell<MouseRegions>` progression immediately obvious to readers.
2. **Annotation style**: Used `// EXCEPTION (TEA):` to distinguish this richer annotation from the shorter `// EXCEPTION:` in the `Cell<usize>` block, matching the actual comment seen in `render/mod.rs` line 135.
3. **Architecture detail withheld**: Type definitions (`MouseRect`, hit-test algorithm, RAII guard internals) were deliberately not repeated here — the cross-reference to ARCHITECTURE.md covers those.
