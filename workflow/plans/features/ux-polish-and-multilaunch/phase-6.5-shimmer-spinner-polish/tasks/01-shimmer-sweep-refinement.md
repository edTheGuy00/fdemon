## Task: Refine the shimmer sweep so it breathes instead of popping

**Objective**: Make the status-label shimmer (Phase 2) read as a smooth, breathing
sweep — fading in from off-screen, exiting off-screen, with a brief all-dim rest gap
between cycles — instead of the current bright head that pops in at the first
character and snaps back every cycle. This is a **pure-math change to the head
position and falloff in `shimmer_spans`**; the period (frame counter `% 30`,
~1.5 s) and all call sites are unchanged.

**Depends on**: none

**Estimated Time**: 1–1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/shimmer.rs`: change the `head` computation and the
  falloff constant in `shimmer_spans`; update the affected `#[cfg(test)] mod tests`
  cases.

**Files Read (Dependencies):**
- none (pure helper module; no `AppState`, no I/O).

### Background

Current implementation (`shimmer.rs:58,64`):

```rust
const SHIMMER_HEAD_WIDTH: f32 = 4.0;
// ...
let head = phase * chars.len() as f32;          // range [0, n)
// per char:
let dist = (i as f32 - head).abs();
let t = (1.0 - dist / SHIMMER_HEAD_WIDTH).max(0.0);
```

Because `head` ranges over `[0, n)`, at `phase = 0` the head sits exactly on
character 0 (full brightness at the left edge), and as `phase → 1` it reaches the
last character then snaps back — a hard, rest-less, mechanical loop.

The smoother behaviour maps the head off-screen at **both** ends so it travels
`[−LEAD, n + LEAD)`. The head spends part of each cycle fully off-screen, which
produces an all-dim rest gap and a soft fade-in/fade-out at the edges.

### Details

**1. Add a lead constant and re-map `head`:**

```rust
/// Width of the bright "head" of the sweep, in characters.
const SHIMMER_HEAD_WIDTH: f32 = 3.5; // was 4.0 — slightly tighter head

/// How far (in characters) the sweep head travels off-screen past each edge, so it
/// fades in from the left, exits off the right, and leaves a brief all-dim rest gap
/// between cycles instead of popping in / snapping back.
const SHIMMER_LEAD: f32 = 3.0;
```

```rust
// in shimmer_spans, replace:
//   let head = phase * chars.len() as f32;
let n = chars.len() as f32;
let head = phase * (n + SHIMMER_LEAD * 2.0) - SHIMMER_LEAD; // range [-LEAD, n+LEAD)
```

Keep the per-character body identical (`dist = (i - head).abs()`,
`t = (1.0 - dist / SHIMMER_HEAD_WIDTH).max(0.0)`, `lerp_color(base, highlight, t)`,
preserve `modifier`). Only the `head` formula and `SHIMMER_HEAD_WIDTH` change.

**2. Do not change `shimmer_phase` or `SHIMMER_PERIOD_FRAMES`.** The cycle length is
correct (~1.5 s); the frame-counter source keeps every concurrent shimmer in unison.
Switching to a per-widget wall-clock phase is explicitly out of scope (it would
desync separate labels).

**3. Update the unit tests.** Two existing tests encode the old range and must be
re-derived for `[-LEAD, n+LEAD)` with `SHIMMER_HEAD_WIDTH = 3.5`:

- `shimmer_spans_head_is_brightest` — at `phase = 0.0` the head is now at
  `-SHIMMER_LEAD = -3.0`, **not** index 0, so character 0 is no longer the global
  brightest. Re-target this test to a phase where the head sits over a known
  interior index (e.g. choose `phase` so `head ≈ middle index`) and assert that
  index is brightest and a far index is at/near `base`. Its "char at dist >4 == base"
  assertion must become "dist > 3.5 (= `SHIMMER_HEAD_WIDTH`) == base".
- Keep `shimmer_phase_wraps_over_period`, `shimmer_phase_no_panic_near_u64_max`,
  `lerp_*`, `shimmer_spans_one_per_char_and_empty`, `shimmer_spans_preserves_modifier`,
  and `shimmer_spans_unicode_multibyte` — they remain valid (span count, modifier,
  unicode, and phase wrap are independent of the head range).
- Add a test asserting the **rest gap / off-screen lead-in**: at `phase = 0.0`,
  `head = -3.0`, so index 0 is `3.0` away → `t = 1 - 3.0/3.5 ≈ 0.143` (dim, not full
  highlight); assert index 0's fg is closer to `base` than to `highlight` at the
  cycle start (proves no pop-in). Optionally assert there exists a phase at which
  **every** character is at `base` (fully off-screen head) to prove the rest gap.

### Acceptance Criteria

1. `shimmer_spans` produces one span per character (unchanged), preserves the caller
   `modifier`, and degrades gracefully on non-RGB terminals (via `lerp_color`).
2. The bright head enters from off-screen left and exits off-screen right: at
   `phase = 0.0` the leftmost character is **not** at full highlight (no pop-in), and
   there is a phase range where no character reaches the highlight (rest gap).
3. `SHIMMER_HEAD_WIDTH` is `3.5` and `SHIMMER_LEAD` is a named constant with a
   comment (no inline magic numbers).
4. The shimmer period is unchanged (`SHIMMER_PERIOD_FRAMES = 30`, `shimmer_phase`
   untouched); all shimmers remain in unison.
5. No call site of `shimmer_spans` requires modification.
6. `cargo test -p fdemon-tui --lib widgets::shimmer` passes; `cargo fmt`/`clippy`
   clean.

### Testing

```bash
cargo test -p fdemon-tui --lib widgets::shimmer
cargo test -p fdemon-tui --lib
cargo clippy -p fdemon-tui --all-targets -- -D warnings
```

Manual (recommended): run a session and trigger a hot reload / launch to watch the
`Launching`/`Reloading` label — the sweep should fade in, glide across, fade out,
and pause briefly before repeating, rather than blinking back to the start.

### Notes

- **One function, broad effect.** Every shimmer caller (the status label today, any
  future reuse) inherits the smoother sweep from this single change — that is the
  intent; do not fork a second variant.
- **Tuning constants.** `SHIMMER_LEAD = 3.0` and `SHIMMER_HEAD_WIDTH = 3.5` are
  reasonable starting values that match the smoother variant studied during
  planning; nudge during the manual check if the rest gap reads too long/short, but
  keep both as named, commented constants.
- **No managed-doc change.** Pure-math tweak inside an existing helper — no
  architecture/standards/dev-doc update required.
