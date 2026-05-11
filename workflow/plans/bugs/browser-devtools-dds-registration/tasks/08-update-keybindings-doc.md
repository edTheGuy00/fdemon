## Task: Update KEYBINDINGS.md for Browser DevTools Behavior

**Objective**: Document that `B` opens the served DevTools URL (or legacy fallback with toast). No new keybinding — just a behavior clarification.

**Depends on**: 07-fallback-and-recovery-toast

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `docs/KEYBINDINGS.md`: In the DevTools-mode section, augment the `B` key entry with a note about behavior on modern vs. older Flutter SDKs.

**Files Read (Dependencies):**
- `docs/KEYBINDINGS.md`: Existing entries for format reference.

### Details

Add a short note under the `B` keybinding (no schema changes, no new keys):

> `B` — Open browser DevTools.
> On Flutter SDK ≥ 3.16, opens the DevTools web app served by the Flutter daemon. On older SDKs, falls back to the legacy DDS-served URL and shows a recovery toast.

### Acceptance Criteria

1. `docs/KEYBINDINGS.md` accurately describes the new behavior.
2. Format consistent with surrounding entries.
3. No new keybindings introduced.

### Notes

- This is an unmanaged doc; implementor can edit directly.
