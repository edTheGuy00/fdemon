## Task: Update website toolchain docs for the new step order

**Objective**: Update the public docs page so the wizard's documented step order matches the reorder
(`Prerequisites → Android Tools → Flutter SDK → PATH Config → Doctor`). Swap the **PATH Config** and
**Flutter SDK** rows in the ASCII-art step list and in the numbered table, and review the
"Step order vs. install order" note. The page still describes **five** steps — do not change the count.

**Depends on**: None (disjoint file from task 01; safe to run in parallel)

**Agent:** implementor

**Estimated Time**: ~30 minutes

### Scope

**Files Modified (Write):**
- `website/src/pages/docs/toolchain.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/state.rs` (read-only, to confirm the final order from `build_steps()`).

### Details

> Locate by the quoted strings below, not by absolute line number.

#### 1. ASCII-art step list (in the `<pre>` block under the "The Five Steps" section)

The left-pane list currently reads (top→bottom): `Prerequisites`, `Android Tools`, `PATH Config`,
`Flutter SDK`, `Doctor`. **Swap the `PATH Config` and `Flutter SDK` rows** (label + status glyph),
preserving the exact box column padding. Both labels are 11 characters, so alignment is preserved.

Current:
```
\u{2502} \u{25cb} PATH Config        \u{2502}                                        \u{2502}
\u{2502} \u{2714} Flutter SDK        \u{2502}   Enter: run \u{00b7} r: re-check             \u{2502}
```
Becomes (Flutter SDK now above PATH Config; keep the right-pane illustration as-is):
```
\u{2502} \u{2714} Flutter SDK        \u{2502}                                        \u{2502}
\u{2502} \u{25cb} PATH Config        \u{2502}   Enter: run \u{00b7} r: re-check             \u{2502}
```
(Only the left-pane label + glyph move. Do not alter the `│` borders, widths, or the box-drawing
header/footer.)

#### 2. Numbered table (`<tbody>` rows)

Swap table rows **3** and **4** — both the step name *and* its "What it does" description, so the row
number stays sequential and the description follows the step:

- Row that currently reads **"3. PATH Config"** (Mode `"Auto"`, description "Writes marker-fenced PATH
  and `ANDROID_HOME` exports …") becomes **"3. Flutter SDK"** with the Flutter SDK description
  ("Installs a managed Flutter SDK via `git clone` (default) with an archive-download fallback, then
  runs `flutter precache`.").
- Row that currently reads **"4. Flutter SDK"** becomes **"4. PATH Config"** with the PATH Config
  description ("Writes marker-fenced PATH and `ANDROID_HOME` exports to your shell rc files
  (idempotent). Runs after a successful install.").

Rows 1 (Prerequisites), 2 (Android Tools), and 5 (Doctor) are unchanged. The "Mode" cell is `"Auto"`
for both swapped rows, so it does not need editing.

#### 3. "Step order vs. install order" info box

The blue note currently says the steps are "numbered in a familiar reading order, but the wizard
installs in dependency order (prerequisites → Flutter SDK → Android tools, which need a JDK → PATH →
doctor)". After the reorder the **UI order now matches the install/dependency order more closely**
(Flutter SDK precedes PATH). Update the note so it is still accurate — either:
- simplify it to note that the UI order now mirrors the dependency order (prerequisites → Android tools
  → Flutter SDK → PATH → doctor) and the wizard skips already-satisfied steps; or
- keep the dependency-chain sentence but ensure it no longer implies the UI shows PATH before Flutter SDK.

Keep it factually correct and brief; do not introduce Platforms-submenu content (that is a later phase).

### Acceptance Criteria

1. The ASCII-art step list shows `Flutter SDK` immediately above `PATH Config`; box alignment is intact.
2. The numbered table lists `3. Flutter SDK` then `4. PATH Config`, each with its matching description.
3. The "Step order vs. install order" note is accurate for the new order and contains no stale claim
   that PATH precedes Flutter SDK.
4. The page still describes five steps; no Platforms/submenu content is added.
5. The website crate builds (`cargo build -p <website-crate>` or the project's website build command);
   no broken markup.

### Testing

- Build the website crate to confirm the Rust/Leptos markup still compiles (see `docs/DEVELOPMENT.md`
  for the exact website build/preview command).
- Visually confirm the swapped rows render in order if a local preview is available.

### Notes

- Text/markup-only change; no behavior. Do not touch `crates/` source here (task 01 owns that).
- `docs/ARCHITECTURE.md` needs no change for this phase (its execution-flow description is unchanged).

---

## Completion Summary

**Status:** _(fill in)_
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
