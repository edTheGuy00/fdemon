## Task: Shell-aware PATH configuration writer

**Objective**: Implement `path_config.rs`: detect the user's shell and write an
idempotent, marker-fenced `PATH` export for `<flutter>/bin` to the correct rc file
(`.bashrc`/`.zshenv`/`.zprofile`/fish `fish_add_path`/Windows registry via
PowerShell `setx`-equivalent). Re-running never duplicates the entry.

**Depends on**: 03 (for `toolchain/mod.rs` ordering; logically only needs task 01's
`HostShell`)

**Agent:** implementor

**Estimated Time**: 4-5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/path_config.rs` — **NEW**
- `crates/fdemon-daemon/src/toolchain/mod.rs` — add `mod path_config;` and
  re-export `add_to_path`, `PathConfigOutcome`, `rc_file_for_shell`.

**Files Read (Dependencies):**
- `toolchain/types.rs` — `HostShell`, `HostPlatform`.

### Details

```rust
/// What happened when configuring PATH.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathConfigOutcome {
    /// Wrote a new fenced block to `rc_file`.
    Written { rc_file: PathBuf },
    /// The fenced block already existed and was up to date — no change.
    AlreadyPresent { rc_file: PathBuf },
}

/// Select the rc file to edit for the given shell (Unix) under `home`.
pub fn rc_file_for_shell(shell: HostShell, home: &Path) -> Option<PathBuf>;

/// Add `<bin_dir>` to PATH for the detected shell. Idempotent + marker-fenced.
/// On Windows, updates the user PATH via the registry (PowerShell
/// `[Environment]::SetEnvironmentVariable(...,'User')`) rather than rc files.
pub fn add_to_path(shell: HostShell, platform: HostPlatform, bin_dir: &Path) -> Result<PathConfigOutcome>;
```

**Marker fencing** — every written block is wrapped so it can be detected and
replaced idempotently:

```
# >>> fdemon flutter path >>>
export PATH="$PATH:/home/u/fvm/versions/stable/bin"
# <<< fdemon flutter path <<<
```

Algorithm for Unix rc files:
1. Resolve rc file via `rc_file_for_shell` (bash → `~/.bashrc`; zsh → `~/.zshenv`
   preferred, else `~/.zprofile`; fish → handled specially).
2. Read existing contents (empty if absent). If a fence block already exists and
   already contains `bin_dir`, return `AlreadyPresent`. If a fence block exists
   but points elsewhere, replace just that block. Otherwise append a new block.
3. Write atomically (temp file in same dir → rename), creating the file/dir if needed.

Shell-specific export lines:
- bash/zsh: `export PATH="$PATH:<bin>"`
- fish: write `fish_add_path <bin>` into `~/.config/fish/config.fish` (still
  marker-fenced), since fish doesn't use POSIX `export`.
- Windows: no rc file — call PowerShell to read the current user PATH, append
  `<bin>` if missing, and set it back. Return `Written`/`AlreadyPresent` accordingly.
  Guard against the 1024-char `setx` truncation by using the registry/Environment
  API, not `setx` (per PLAN.md risk note).

The function never edits the *running* process env (impossible to affect the
parent shell). The caller surfaces a "restart your terminal" hint (task 09/10).

### Acceptance Criteria

1. `rc_file_for_shell` returns the documented file per shell.
2. `add_to_path` writes a marker-fenced block exactly once; calling it twice with
   the same `bin_dir` yields `AlreadyPresent` on the second call and the file
   contains exactly one fence block (idempotency).
3. Changing `bin_dir` replaces the existing fence block rather than appending a
   second one.
4. Fish writes `fish_add_path`; bash/zsh write `export PATH`.
5. Writes are atomic and create the parent directory if needed.
6. Golden-file unit tests cover write, no-op re-run, and block replacement.
   No clippy warnings.

### Testing

```rust
#[test]
fn test_writes_fenced_block_once() {
    // tempdir HOME, add_to_path(Bash,...) → Written; file has one fence + export
}

#[test]
fn test_rerun_is_idempotent() {
    // call twice → second returns AlreadyPresent, still exactly one block
}

#[test]
fn test_changed_bin_dir_replaces_block() {
    // add_to_path with /a/bin then /b/bin → one block pointing at /b/bin
}

#[test]
fn test_fish_uses_fish_add_path() { /* fish branch writes config.fish with fish_add_path */ }

#[test]
fn test_rc_file_selection_per_shell() { /* bash→.bashrc, zsh→.zshenv, etc. */ }
```

Use `tempfile::TempDir` as a fake HOME; never touch the real user rc files in tests.
Gate the Windows registry path behind `#[cfg(target_os = "windows")]` tests or pure
string-builder helpers that can be unit-tested cross-platform.

### Notes

- Keep the PATH-line builder and the fence parser as small pure functions so they
  are unit-testable without filesystem I/O (the golden tests can target those).
- Confirmation UX: pressing `Enter` on the PATH step is the confirmation (task 09);
  this module just performs the write when asked.
- Do not attempt `sudo` or system-wide profile edits — user-scope files only.

---

## Completion Summary

**Status:** Not Started
</content>
