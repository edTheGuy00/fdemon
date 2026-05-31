## Task: Update Documentation — docs/KEYBINDINGS.md (multi-launch keys)

**Agent:** doc_maintainer

**Objective**: Verify the canonical keybinding doc is complete and current, and add the
shipped multi-device launch keys if missing.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `docs/KEYBINDINGS.md`: add/verify multi-launch keys and any missing bindings.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: content boundary rules.
- `crates/fdemon-app/src/handler/keys.rs`,
  `crates/fdemon-app/src/handler/new_session/`: source of truth.

### Change Context

`docs/KEYBINDINGS.md` was cited throughout the sweep as the *correct* source the website
lagged. Verify completeness; the multi-device launch keys may also be missing here.

Checklist:
1. Confirm coverage of `Alt+m`, `D` (DAP), `V` (Flutter version), `w`, `m` (Memory
   panel), full Performance bindings, Flutter Version + Loading modes.
2. **Add (if missing) the multi-device launch keys** for the new-session dialog Connected
   tab: `Space` toggle, `a` select-all/clear, `Enter` launch all checked (or cursor),
   `r` refresh — matching `handler/new_session/target_selector.rs` and `keys.rs:1368-1369`.
3. Confirm `[`/`]` are documented as Performance detail-tab keys, not session cycling.

### Acceptance Criteria

1. Multi-launch keys present and correct.
2. No keybinding contradicts `keys.rs`.
3. No content boundary violations; `doc-validate` passes for `docs/KEYBINDINGS.md`.

### Notes

- Make targeted edits, do not rewrite the whole document.
- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
</content>
