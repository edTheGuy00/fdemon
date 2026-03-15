## Task: Resolve `url` Crate vs `parse_http_url` Inconsistency

**Objective**: Eliminate the dual-parser inconsistency where `url::Url::parse()` is used for validation at config load time and a manual `parse_http_url()` is used at check execution time. Use `parse_http_url` for both and remove the `url` crate dependency.

**Depends on**: None

**Severity**: Minor

### Scope

- `crates/fdemon-app/src/config/types.rs`: Replace `url::Url::parse()` validation with `parse_http_url()` (line 639)
- `crates/fdemon-app/src/actions/ready_check.rs`: Make `parse_http_url` available to the config module
- `crates/fdemon-app/Cargo.toml`: Remove `url` dependency (if no other uses)

### Details

#### The Problem

Two different HTTP URL parsers are used:

1. **Validation time** (`config/types.rs:639`): `url::Url::parse(url)` — full RFC 3986 parser
2. **Execution time** (`ready_check.rs:161-182`): `parse_http_url(url)` — manual parser that only supports `http://host[:port][/path]`

These can disagree on edge cases (e.g., IPv6 addresses `http://[::1]:8080/`, userinfo `http://user:pass@host/`, URL-encoded paths). A URL that passes validation could fail at execution time, or vice versa.

#### Recommended Approach

Use `parse_http_url` for validation too, since it is the parser that matters at runtime:

1. Move `parse_http_url` to a location importable by both `config/types.rs` and `ready_check.rs` — either make it `pub(crate)` in `ready_check.rs`, or move it to a shared location within `fdemon-app`
2. In `ReadyCheck::validate()` for the `Http` variant, replace:
   ```rust
   let parsed = url::Url::parse(url)
       .map_err(|e| format!("invalid ready_check url '{}': {}", url, e))?;
   if parsed.host().is_none() {
       return Err(format!("ready_check url '{}' has no host", url));
   }
   ```
   With:
   ```rust
   crate::actions::ready_check::parse_http_url(url)
       .map_err(|e| format!("invalid ready_check url '{}': {}", url, e))?;
   ```
3. Remove the `url` dependency from `Cargo.toml` (check for other uses first)

### Acceptance Criteria

1. The same parser (`parse_http_url`) is used for both validation and execution
2. `url` crate dependency removed from `fdemon-app/Cargo.toml` (if unused elsewhere)
3. Config validation catches the same URLs that would fail at runtime
4. All existing URL validation tests pass (may need adjustment for different error messages)
5. Existing HTTP check tests pass

### Notes

- Check whether `url` is used anywhere else in `fdemon-app` before removing the dependency
- The `parse_http_url` function needs to become `pub(crate)` (currently private) — it only needs crate-internal visibility
- If `ready_check` visibility has been changed to `pub(super)` (task 03), the import path `crate::actions::ready_check::parse_http_url` will work because `pub(crate)` on the function itself provides crate-wide access regardless of module visibility

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/ready_check.rs` | Changed `parse_http_url` from `fn` (private) to `pub(crate) fn` with added doc comment explaining the reason |
| `crates/fdemon-app/src/config/types.rs` | Replaced `url::Url::parse()` + host check with `crate::actions::ready_check::parse_http_url()` call |
| `crates/fdemon-app/Cargo.toml` | Removed `url.workspace = true` dependency |

### Notable Decisions/Tradeoffs

1. **Kept `pub(crate)` rather than `pub`**: The function only needs to be visible within the crate. `pub(crate)` gives access to `config::types` via the absolute path `crate::actions::ready_check::parse_http_url` while keeping the function internal. The `ready_check` module remains `pub(super)` as set by task 03.

2. **Test `test_ready_check_http_validate_url_no_host`**: With the old parser, `"file:///path/to/file"` would parse successfully but fail the host check. With `parse_http_url`, it fails the `http://` prefix check. Both result in `Err(...)` so the test (which only checks `is_err()`) continues to pass without modification.

3. **Confirmed `url` crate was only used in one place**: Grepped all of `crates/fdemon-app/src` and confirmed `url::` appeared only on line 639 of `config/types.rs`. Safe to remove.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1649 unit tests, 1 doc-test)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Narrower validation**: `parse_http_url` only accepts `http://` URLs. The old `url::Url::parse` would accept `https://`, `ftp://`, etc. and only reject on missing host. The new code rejects non-http schemes earlier with a clearer error ("URL must start with http://"). This is the correct behavior given the HTTP check only supports plain HTTP.
