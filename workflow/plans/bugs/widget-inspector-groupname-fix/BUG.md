# Bugfix Plan: Widget Inspector `getRootWidgetTree` Null Check Failure

## TL;DR

`getRootWidgetTree` throws "Null check operator used on a null value" because we pass `objectGroup` as the parameter key, but the Flutter framework expects `groupName`. The `!` on `parameters['groupName']!` fires immediately when the key is missing. The browser DevTools works because it sends `groupName`. Fix: use `groupName` for `getRootWidgetTree` and keep `objectGroup` for the legacy `getRootWidgetSummaryTree`.

## Bug Report

### Symptom

Widget inspector times out or shows error on large Flutter projects. The Flutter framework throws:

```
_TypeError: Null check operator used on a null value
  #0  WidgetInspectorService._getRootWidgetTree (widget_inspector.dart:2103:53)
```

The browser DevTools can fetch the same widget tree successfully (showing ZabinApp -> MultiRepositoryProvider -> MultiBlocProvider -> etc.).

### Expected

Widget inspector should successfully fetch and display the widget tree, matching browser DevTools behavior.

### Root Cause Analysis

**The Flutter framework `_getRootWidgetTree` method (line ~2103):**

```dart
Future<Map<String, Object?>> _getRootWidgetTree(Map<String, String> parameters) {
  final String groupName = parameters['groupName']!;  // <-- LINE 2103: throws if key missing
  final isSummaryTree = parameters['isSummaryTree'] == 'true';
  final withPreviews  = parameters['withPreviews']  == 'true';
  // ...
}
```

**What we send:**

```json
{
  "isolateId": "isolates/...",
  "objectGroup": "fdemon-inspector-1",
  "isSummaryTree": "true",
  "withPreviews": "false"
}
```

**What the framework expects:**

```json
{
  "isolateId": "isolates/...",
  "groupName": "fdemon-inspector-1",
  "isSummaryTree": "true",
  "withPreviews": "true"
}
```

**Three issues:**

1. **Wrong parameter key**: We send `objectGroup` but `getRootWidgetTree` expects `groupName`. The `!` null-assertion on `parameters['groupName']!` fires immediately. This is the direct cause of the crash.

2. **Missing `groupName` in fallback**: When `getRootWidgetTree` fails (due to the wrong key), we fall back to `getRootWidgetSummaryTree`. This older API uses a `_registerObjectGroupServiceExtension` helper that wraps the callback and uses `objectGroup` as its key — so the fallback *should* work with `objectGroup`. However, the null check failure from attempt 1 still throws a Flutter-side exception that appears in the user's log.

3. **`withPreviews` is `"false"`**: Browser DevTools hardcodes `"true"`. While not a crash cause, this affects what data is returned.

**Key difference between the two APIs:**

| Extension | Parameter Key | Registration Style |
|-----------|--------------|-------------------|
| `getRootWidgetTree` (new, Flutter 3.22+) | `groupName` | Raw `registerServiceExtension` |
| `getRootWidgetSummaryTree` (old, deprecated) | `objectGroup` | `_registerObjectGroupServiceExtension` helper |

**What the browser DevTools sends:**

```json
{
  "groupName": "inspector_5",
  "isSummaryTree": "true",
  "withPreviews": "true",
  "fullDetails": "true"
}
```

DevTools calls ONLY `getRootWidgetTree` — never `getRootWidgetSummaryTree`.

---

## Affected Modules

- `crates/fdemon-app/src/actions.rs`: `try_fetch_widget_tree` — uses wrong param key + wrong `withPreviews` value
- `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs`: `get_root_widget_tree` — also uses wrong param key (same bug, unused code path)

---

## Fix

### Task 1: Fix parameter key in `try_fetch_widget_tree` (actions.rs)

In `try_fetch_widget_tree`, change the `getRootWidgetTree` call params:

**Before:**
```rust
newer_args.insert("objectGroup".to_string(), object_group.to_string());
newer_args.insert("isSummaryTree".to_string(), "true".to_string());
newer_args.insert("withPreviews".to_string(), "false".to_string());
```

**After:**
```rust
newer_args.insert("groupName".to_string(), object_group.to_string());
newer_args.insert("isSummaryTree".to_string(), "true".to_string());
newer_args.insert("withPreviews".to_string(), "true".to_string());
```

The legacy `getRootWidgetSummaryTree` fallback must keep `objectGroup` — that API's registration helper extracts it with that exact key.

### Task 2: Fix parameter key in `get_root_widget_tree` (inspector.rs)

Same fix in the lower-level function (currently unused but should be kept consistent):

**Before:**
```rust
newer_args.insert("objectGroup".to_string(), object_group.to_string());
newer_args.insert("isSummaryTree".to_string(), "true".to_string());
newer_args.insert("withPreviews".to_string(), "false".to_string());
```

**After:**
```rust
newer_args.insert("groupName".to_string(), object_group.to_string());
newer_args.insert("isSummaryTree".to_string(), "true".to_string());
newer_args.insert("withPreviews".to_string(), "true".to_string());
```

### Task 3: Verify `disposeGroup` still uses `objectGroup`

`ext.flutter.inspector.disposeGroup` is registered via `_registerObjectGroupServiceExtension` — it expects `objectGroup`, not `groupName`. Confirm no change needed here.

---

## Edge Cases & Risks

### Flutter SDK Version Compatibility
- **Risk:** `getRootWidgetTree` only exists in Flutter 3.22+ (June 2024). Older SDKs will return "method not found".
- **Mitigation:** The existing fallback to `getRootWidgetSummaryTree` (which uses `objectGroup`) handles this correctly.

### Parameter Key Mismatch Per Extension
- **Risk:** Different extensions use different keys (`groupName` vs `objectGroup`).
- **Mitigation:** Only `getRootWidgetTree` uses `groupName`. All other inspector extensions (`getDetailsSubtree`, `getLayoutExplorerNode`, `disposeGroup`, `getSelectedWidget`) use `objectGroup`. The fix is scoped to the two call sites that use `GET_ROOT_WIDGET_TREE`.

---

## Success Criteria

- [ ] `getRootWidgetTree` called with `groupName` parameter key
- [ ] Widget tree successfully loads on the same large project that previously failed
- [ ] No Flutter-side "Null check operator" exceptions in the log
- [ ] Fallback to `getRootWidgetSummaryTree` still works (uses `objectGroup`)
- [ ] `cargo build --workspace` compiles cleanly
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` no new warnings

---

## References

- [Flutter `widget_inspector.dart` source](https://github.com/flutter/flutter/blob/master/packages/flutter/lib/src/widgets/widget_inspector.dart) — `_getRootWidgetTree` at line ~2103 does `parameters['groupName']!`
- [PR #150010: Add getRootWidgetTree extension](https://github.com/flutter/flutter/pull/150010) — Added June 2024, uses `groupName`
- [DevTools `inspector_service.dart`](https://github.com/flutter/devtools/blob/master/packages/devtools_app/lib/src/shared/diagnostics/inspector_service.dart) — Browser DevTools sends `groupName`
- [Issue #99460](https://github.com/flutter/flutter/issues/99460) — Related null check errors in inspector
