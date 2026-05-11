# Task 05: Document OSC 22 caveat and manual mouse-exit verification

**Agent:** doc_maintainer
**Files:** `docs/DEVELOPMENT.md`; and `docs/MOUSE.md` if it exists, otherwise add a short note in the appropriate mouse-docs file (check `docs/KEYBINDINGS.md` and `docs/CONFIGURATION.md` first — the mouse-support phase-6 work added docs there)
**Depends on:** None
**Wave:** 1 (Worktree C, independent)

## Background

Bug 2 ships OSC 22 pointer-shape emission. It is best-effort and depends on
terminal support. Bug 3 introduces a stdin drain step on exit. Both have
manual-verification flows that belong in the developer docs.

## What to do

### 1. Locate the existing mouse docs

Run a quick search to find where the mouse-support feature documented
itself:

```bash
grep -rn "mouse" docs/ --include="*.md"
```

The phase-6 plan summary lists `docs/MOUSE.md` or a section in
`docs/CONFIGURATION.md` / `docs/KEYBINDINGS.md`. Use whichever already
holds the mouse-support narrative; do not create a new top-level mouse
doc unless one is clearly missing.

### 2. Add an "Pointer shape" / "Cursor shape" subsection

Add a short subsection explaining:

- fdemon requests the `default` (arrow) OS-level pointer shape via OSC 22
  while the TUI is active, and resets on exit.
- Practical terminal support: kitty, xterm, Ghostty, Foot work out of the
  box; Alacritty needs `terminal.osc22 = true` in its config; iTerm2,
  macOS Terminal.app, Windows Terminal, and GNOME Terminal silently
  ignore the request and continue to display the text I-beam.
- Link to the kitty pointer-shapes documentation:
  <https://sw.kovidgoyal.net/kitty/pointer-shapes/>
- Link to the compatibility table:
  <https://can-i-use-terminal.github.io/features/osc22.html>

### 3. Add a manual mouse-exit verification line in `docs/DEVELOPMENT.md`

In the "Common Issues" section of `docs/DEVELOPMENT.md` (around line 256),
add a new entry:

```markdown
### Mouse Exit Leak Verification

If you are working on the terminal teardown path (`runner.rs`,
`terminal.rs`), manually verify after exit:

1. Launch fdemon in a terminal with mouse support enabled.
2. Move the mouse rapidly while pressing `Q` to quit.
3. The shell prompt must come back cleanly — no `0;NN;NN m` /
   `NN;NN;NN M` SGR mouse-report bytes should appear.

A regression in the disable-then-shutdown ordering will reintroduce the
leak. See `workflow/plans/bugs/mouse-support-followup/BUG.md` Bug 3 for
the failure mode.
```

## Verification

- `cargo doc` (or whatever docs check the project uses) does not regress.
- Manual: render the changed markdown locally (any md preview), confirm
  links resolve and headings nest correctly.
- The `doc-standards` skill, if applicable, validates the changes against
  the documentation schema.
