# Task 10 — Update `docs/ARCHITECTURE.md` for new service + side-effect channel

**Agent:** **doc_maintainer**
**Wave:** 4
**Depends on:** Task 02 (new `Clipboard` service), Task 03 (new `UpdateAction` variant), Task 07 (runner side-effect glue)
**Files written:** `docs/ARCHITECTURE.md`

---

## Goal

Capture the two structural additions from this bug fix:

1. **New service** — `crates/fdemon-app/src/services/clipboard.rs` with `Clipboard` trait, `SystemClipboard` impl (arboard-backed), and `MemoryClipboard` test mock. Joins the existing `flutter_controller.rs`, `log_service.rs`, `state_service.rs` family.
2. **New side-effect channel** — `UpdateAction::SetMouseCapture(bool)` and `UpdateAction::WriteClipboard { text }` extend the existing handler → runner action protocol. Both follow the established "round-trip via the message bus" pattern (the runner sends a follow-up `Message::MouseCaptureChanged` after `SetMouseCapture` succeeds; `WriteClipboard` is fire-and-forget with a warning toast on failure).

## What to add

In the existing "Workspace Crates" / "Service layer" section, add `clipboard.rs` to the list of services with a one-line description: *"Cross-platform clipboard writer behind a trait — `SystemClipboard` for production, `MemoryClipboard` for tests."*

In the section describing `UpdateAction` (likely under "Data Flow" or "TEA Message Flow" — search the doc), add the two new variants with a sentence each on what they instruct the runner to do.

In the "Key Patterns" section, augment the existing **Service layer** bullet to mention that the `Clipboard` trait is the first service in the family that is owned by the runner (not held on `AppState`) — this preserves TEA purity for side-effect-only services, in contrast to `LogService` / `StateService` which carry data.

## What NOT to add

Per `doc_maintainer`'s strict content boundaries:

- Do NOT add any keymap, configuration value, or end-user how-to to ARCHITECTURE.md — those live in `KEYBINDINGS.md`, `CONFIGURATION.md`, and `MOUSE.md`. Task 09 covers those.
- Do NOT document the `?1003` DECSET decision in ARCHITECTURE.md — it is a terminal-protocol detail, not an architectural concern. The decision is recorded in `BUG.md`.
- Do NOT add line-by-line implementation notes; keep the descriptions at the same abstraction level as the rest of ARCHITECTURE.md.

## Acceptance Criteria

- [ ] `docs/ARCHITECTURE.md` mentions the new `clipboard.rs` service in the services list.
- [ ] The `UpdateAction` documentation (wherever it lives in this doc) includes the two new variants.
- [ ] The "Service layer" Key Pattern bullet is augmented as described.
- [ ] No content outside the `doc_maintainer` boundary is added (no keymaps, no user how-to, no terminal-protocol detail).
- [ ] No code is touched.

## Notes for Reviewer

This task is the smallest in the wave but the most boundary-sensitive: the `doc_maintainer` agent will reject content that belongs in `MOUSE.md` / `CONFIGURATION.md`. If in doubt about where a sentence belongs, ask whether a developer reading **only** ARCHITECTURE.md needs the sentence to understand the module structure. If no, it belongs elsewhere.
