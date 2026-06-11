## Task: Daemon — pinned-version install plumbing (`release_date`, `version_tag`, `resolve_version_release`)

**Objective**: Teach the daemon installer to install an exact Flutter version: deserialize
`release_date` into `FlutterRelease`, add `version_tag: Option<String>` to `FlutterInstallTarget`,
add a private `resolve_version_release` (exact-version + arch matching, hard error on miss), thread
the tag through the git (`-b <tag>`) and archive paths, and widen ref validation to accept `+`.
Plus the one-line `version_tag: None` stub in the app executor so the workspace keeps compiling.

**Depends on**: None (Wave 1).

**Agent:** implementor

**Complexity:** high

**Estimated Time**: 4–5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/types.rs`
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs`
- `crates/fdemon-app/src/actions/mod.rs` — **one line only**: add `version_tag: None,` to the
  `FlutterInstallTarget` struct literal (~line 924) so the new field compiles. Task 04 replaces it.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/mod.rs` — re-export list (`FlutterRelease`,
  `FlutterReleaseManifest`, `HostArch` are already exported; verify, don't duplicate).

> Line numbers are a snapshot at `f53a9b54` — locate by symbol.

### Details

#### 1. `FlutterRelease.release_date` (`types.rs:321`, `flutter_install.rs:378`)

Add `pub release_date: Option<String>` to `FlutterRelease` and `#[serde(default)] release_date:
Option<String>` to `RawRelease`; map it through in `fetch_release_manifest_from`'s raw→public
conversion. Keep the raw ISO-8601 string verbatim (display formatting is a TUI concern). Old manifest
entries may omit it → `None` must parse cleanly.

Do **not** deserialize `hash` or `dart_sdk_version` — nothing consumes them (the picker's default
cursor is "first stable entry", which `resolve_stable()` already implements without the hash).

#### 2. `FlutterInstallTarget.version_tag` (`types.rs:380`)

```rust
pub struct FlutterInstallTarget {
    pub method: InstallMethod,
    pub channel: String,              // the release's channel — kept for metadata/fallbacks
    pub install_root: PathBuf,
    pub version_dir_name: String,     // unchanged: callers decide the dir name
    /// Exact manifest version (e.g. "3.24.0") or "master"/"main". When set, the git
    /// path clones `-b <version_tag>` and the archive path resolves by exact version
    /// (hard error on a manifest miss — never the stable fallback).
    pub version_tag: Option<String>,
}
```

Update the doc-comment on the struct describing the precedence. Existing daemon-internal
constructions and tests gain `version_tag: None`.

#### 3. Ref validation — allow `+` (`validate_channel`)

Pinned refs include old tags like `1.12.13+hotfix.5`. Widen the allowed charset to
`[A-Za-z0-9._+-]` for non-first characters; keep rejecting empty refs and a leading `-`
(argument-injection guard) and a leading `+`/`.` for tidiness. Refs are passed to git via
`run_streaming` argv (no shell), so `+` is inert. Validate **both** `target.channel` and
`target.version_tag` (when set) at the top of `install_flutter`. Consider renaming the helper to
`validate_ref` with a `validate_channel` alias if churn is small; otherwise keep the name and update
its doc-comment.

#### 4. Git path (`git_install`)

The `-b` argument becomes `target.version_tag.as_deref().unwrap_or(&target.channel)`. `git clone -b`
accepts tags as well as branches, and `--depth 1` still applies — pinned installs are shallow clones
detached at the tag. No `git reset --hard`. `master`/`main` work unchanged (they are real branches).

#### 5. Archive path (`archive_install`)

- Add beside `resolve_channel_release` (`flutter_install.rs:353`), same privacy:

```rust
/// Resolve a release by exact `version` string. Two-pass arch matching like
/// `resolve_channel_release`: exact `dart_sdk_arch` match first, any-arch fallback.
fn resolve_version_release<'m>(
    manifest: &'m FlutterReleaseManifest,
    version: &str,
    arch: HostArch,
) -> Option<&'m FlutterRelease>
```

- In `archive_install`: when `target.version_tag` is `Some(v)`, resolve via
  `resolve_version_release`; **a miss is a hard error** (`Error` message naming the version and
  suggesting the git method / a re-fetched manifest) — do NOT fall back to stable. The existing
  channel→stable fallback stays for the `version_tag: None` path only.
- `master`/`main` never appear in the manifest, so with `method == Archive` (or git missing from
  PATH, which flips `use_git` off) a pinned `master`/`main` hits this error path — make the message
  explicit: `"<ref> is only installable via git; install git or choose a released version"`.

#### 6. Short-circuit & version readback

`final_dir = install_root / version_dir_name` already short-circuits when `bin/flutter` exists —
verify it now covers `~/fvm/versions/3.24.0`. `read_installed_version(final_dir, channel)` keeps the
channel fallback; no change needed (the `version`/`VERSION` file of a pinned install carries the real
version).

#### 7. App-side stub (`fdemon-app/src/actions/mod.rs` ~924)

Add `version_tag: None,` to the `FlutterInstallTarget` literal, with a `// Task 04 threads the picker
selection here.` comment. Nothing else in fdemon-app changes in this task.

### Acceptance Criteria

1. `MANIFEST_FIXTURE` parsing yields `release_date: Some(..)` for entries that carry it and `None`
   when the key is absent (extend the fixture with one date-less entry).
2. `resolve_version_release`: exact version + exact arch wins; any-arch fallback works; unknown
   version → `None`; duplicate-version dual-arch (macOS) entries resolve to the host arch.
3. `validate_*` accepts `stable`, `3.24.0`, `1.12.13+hotfix.5`; rejects ``""``, `-evil`, `+x`.
4. `FlutterInstallTarget { version_tag: Some("3.24.0"), .. }` with the git path produces a
   `git clone -b 3.24.0 --depth 1 …` invocation (assert via the existing command-construction test
   seam, or extract the arg-builder into a testable helper if none exists).
5. Pinned archive install with a version absent from the manifest returns a clear error (no stable
   download); channel-only archive installs keep the legacy stable fallback (existing tests stay
   green).
6. Workspace compiles (`cargo check --workspace`) with the app-side stub; `cargo test -p
   fdemon-daemon --lib` green; fmt + clippy clean.

### Testing

```bash
cargo test -p fdemon-daemon --lib toolchain
cargo check --workspace
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

New tests (inline, beside the existing `resolve_channel_release` / fixture tests):
- `test_release_date_deserialized` / `test_release_date_absent_is_none`
- `test_resolve_version_release_exact_arch` / `…_any_arch_fallback` / `…_unknown_version_none` /
  `…_dual_arch_prefers_host`
- `test_validate_ref_accepts_plus_tag` / `…_rejects_leading_plus`
- `test_install_flutter_rejects_invalid_version_tag` (mirror
  `test_install_flutter_rejects_invalid_channel`)
- `test_archive_pinned_version_miss_is_error_not_stable_fallback` (fixture-driven; exercise the
  resolution branch directly if `archive_install` is too network-coupled — extracting the
  "pick the release" step into a pure helper is encouraged)

### Notes

- **Do not change `version_dir_name` semantics here** — callers own the dir name; the app executor
  (Task 04) sets it to the version tag. Daemon tests may exercise
  `version_dir_name: "3.24.0"` to prove the path shape.
- The `+` widening intentionally also applies to channels — harmless, and keeps one validator.
- Keep `resolve_version_release` private; the app never needs it (the picker selects from its own
  fetched manifest).
- `serial_test`/wiremock conventions: no live CDN calls in tests.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/types.rs` | Added `pub release_date: Option<String>` to `FlutterRelease`; added `pub version_tag: Option<String>` to `FlutterInstallTarget` with channel/precedence doc-comment; updated test-fixture `FlutterRelease` literals with `release_date: None`. |
| `crates/fdemon-daemon/src/toolchain/flutter_install.rs` | Added `#[serde(default)] release_date` to `RawRelease` and mapped it in the production raw→public conversion; renamed `validate_channel`→`validate_ref` (widened charset to `[A-Za-z0-9._+-]`, rejects leading `-`/`+`/`.`) with a `validate_channel` alias; validate both `channel` and `version_tag` in `install_flutter`; extracted pure `git_clone_args` helper and routed the `-b` ref through `version_tag.as_deref().unwrap_or(&channel)`; added private `resolve_version_release`; pinned archive path (hard error on miss, explicit master/main git-only message); extended `MANIFEST_FIXTURE` with a date-less `1.12.13+hotfix.5` entry; added all required new tests; updated fixture-count assertions 3→4 and test literals with `release_date: None` / `version_tag: None`. |
| `crates/fdemon-app/src/actions/mod.rs` | One-line stub: `version_tag: None,` in the `FlutterInstallTarget` literal with a `// Task 04 threads the picker selection here.` comment. |

### Notable Decisions/Tradeoffs

1. **`validate_channel` kept as an alias**: Rather than renaming all call sites, `validate_ref` is the implementation and `validate_channel` delegates to it. Existing `test_validate_channel_*` tests stay valid (their `contains` assertions still match the new messages). Low churn, single validator.
2. **Pinned-miss test exercises the resolver directly**: `archive_install` is network-coupled (calls `fetch_release_manifest`), so the hard-error/no-stable-fallback acceptance is proven against `resolve_version_release` plus a `resolve_stable` sanity check — confirming the None is a true miss, not an empty manifest.
3. **`master`/`main` handled before the resolver**: an explicit "only installable via git" error is returned before `resolve_version_release` so the message is actionable rather than a generic "version not found".
4. **Date-less fixture entry uses a `+` version** (`1.12.13+hotfix.5`): doubles as evidence the `+`-bearing version string round-trips through serde and the manifest.

### Testing Performed

- `cargo test -p fdemon-daemon --lib toolchain` — Passed (458 tests)
- `cargo test -p fdemon-daemon --lib` — Passed (1236 tests, 3 ignored)
- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (clean)

### Risks/Limitations

1. **No end-to-end pinned archive download test**: per the task, the network-coupled `archive_install` path is not exercised with a real download; the resolution branch (the load-bearing logic) is covered. Mitigation: the git-clone arg shape and the resolver are unit-tested directly.
