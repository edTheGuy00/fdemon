## Task: Document multi-launch resource behavior

**Objective**: Tell users that confirming N checked devices launches up to N concurrent Flutter processes (builds, VM Service connections, native-log capture) — so the resource cost of multi-launch is not a surprise.

**Depends on**: None

**Estimated Time**: 0.5h

**Addresses review item**: m5 (risk: unbounded N-way concurrent spawn, no throttling)

### Scope

**Files Modified (Write):**
- `docs/KEYBINDINGS.md`: add a short note in the New Session Dialog section (where `Space` / `a` / multi-launch are already documented).

**Files Read (Dependencies):**
- None.

### Details

`docs/KEYBINDINGS.md` already documents the multi-launch keys in its "New Session Dialog → Target Selector (Left Pane)" section (`Space` toggle, `a` select-all, `Enter` launch). Add a brief behavior note near that table so users understand the cost of launching many devices at once.

Suggested note (adjust wording to match the doc's voice):

```markdown
> **Multi-launch resource note:** Confirming with multiple devices checked starts
> one Flutter session per checked device (up to the 9-session limit), each running
> its own `flutter run` build, VM Service connection, and native-log capture.
> Launching many cold-build targets at once can spike CPU/memory and contend for
> build tools (Gradle, Xcode). Check only the devices you need. Sessions launch
> concurrently — there is currently no staggering. Devices already running a
> session are skipped; a toast reports "Launched X of Y" when some are skipped.
```

### Acceptance Criteria

1. `docs/KEYBINDINGS.md` contains a note in the New Session Dialog section explaining that multi-launch starts up to N concurrent Flutter sessions and the resource implications.
2. The note mentions the 9-session cap, the concurrent (non-staggered) spawn, and the skip-with-toast behavior.
3. Wording matches the surrounding doc style; no broken Markdown / table formatting.

### Notes

- `docs/KEYBINDINGS.md` is an unmanaged doc — the implementor may edit it directly (no `doc_maintainer` needed).
- Do not document a staggered-spawn feature as if it exists — it is a possible future enhancement, not current behavior. Phrase the note in present tense about today's behavior only.
- If a more natural user-facing home exists (e.g. a README "Sessions" section), adding the note there as well is acceptable, but `KEYBINDINGS.md` is the required target since that is where the multi-launch keys live.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a399e4020717597dd

### Files Modified

| File | Changes |
|------|---------|
| `docs/KEYBINDINGS.md` | Added multi-launch resource note as a blockquote after the Target Selector (Left Pane) key table, before the Launch Context (Right Pane) section |

### Notable Decisions/Tradeoffs

1. **Placement**: Added the note immediately after the Target Selector table (where `Space`, `a`, and `Enter` keys are documented) and before the "Launch Context (Right Pane)" heading. This is the natural reading position — the user has just seen the multi-launch keys and can read the resource implications immediately after.
2. **Blockquote style**: Used `>` blockquote formatting consistent with the `> **Note:**` and `> **Migration note:**` patterns already used elsewhere in the document.
3. **Present-tense only**: The note describes only current behavior (concurrent, non-staggered). No mention of possible future staggered spawn.

### Testing Performed

- Visual inspection of the Markdown output - the blockquote renders correctly after the table and before the next section heading.
- All three acceptance criteria verified: 9-session cap mentioned, concurrent/non-staggered spawn mentioned, skip-with-toast behavior mentioned.

### Risks/Limitations

1. **Documentation only**: This task makes no code changes. All behavior described (concurrent spawn, skip logic, toast) must already be implemented by the time users read this.
