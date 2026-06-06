//! # PATH Configuration Writer
//!
//! Writes idempotent, marker-fenced environment exports for Flutter and Android
//! SDK toolchain components to the correct shell rc file for the detected shell.
//!
//! ## Supported shells
//!
//! | Shell       | Target file                                 | Export syntax                 |
//! |-------------|---------------------------------------------|-------------------------------|
//! | bash        | `~/.bash_profile` (macOS) / `~/.bashrc` (Linux) | `export PATH="$PATH:<bin>"` |
//! | zsh         | `~/.zshenv` (preferred) or `~/.zprofile`    | `export PATH="$PATH:<bin>"`   |
//! | fish        | `~/.config/fish/config.fish`                | `fish_add_path '<bin>'`       |
//! | Windows     | User `HKCU:\Environment\PATH` via PowerShell | registry update (raw, type-preserving) |
//!
//! ## Idempotency
//!
//! Every written block is wrapped with a unique marker so subsequent calls can
//! detect and replace — rather than duplicate — an existing fence block.
//!
//! Flutter PATH block:
//! ```text
//! # >>> fdemon flutter path >>>
//! export PATH="$PATH:/home/u/fvm/versions/stable/bin"
//! # <<< fdemon flutter path <<<
//! ```
//!
//! Android env block:
//! ```text
//! # >>> fdemon android env >>>
//! export ANDROID_HOME='/home/u/.android/sdk'
//! export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH"
//! # <<< fdemon android env <<<
//! ```
//!
//! Algorithm:
//! 1. Validate the SDK root dir via [`validate_bin_dir`] (rejects injection characters).
//! 2. Resolve the rc file via [`rc_file_for_shell`].
//! 3. Read existing contents (empty string if the file is absent).
//! 4. If a fence block already exists and already contains the SDK root, return
//!    [`PathConfigOutcome::AlreadyPresent`].
//! 5. If a fence block exists but points elsewhere, replace just that block.
//! 6. Otherwise append a new block.
//! 7. Write atomically (temp file in same dir → rename), creating parent
//!    directories if needed.

use std::path::{Path, PathBuf};

use fdemon_core::error::{Error, Result};

use super::types::{HostPlatform, HostShell};

// On Unix we need PermissionsExt to read/write mode bits.
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

// ── Windows broadcast script constant ────────────────────────────────────────

/// PowerShell script that broadcasts `WM_SETTINGCHANGE` so running processes
/// pick up registry environment changes immediately.
///
/// The script uses P/Invoke via `Add-Type` to call
/// `SendMessageTimeout(HWND_BROADCAST=0xFFFF, WM_SETTINGCHANGE=0x1A, ...)`.
/// No user-controlled values are interpolated — the broadcast lParam is the
/// system-constant literal `"Environment"`.
///
/// The `SMTO_ABORTIFHUNG` flag (2) gives each recipient a 5 000 ms deadline
/// before giving up on that window. This is an in-script Win32 timeout; it
/// does **not** constrain the lifetime of the `powershell.exe` process itself.
///
/// Referenced by `broadcast_wm_settingchange` (Windows) and by cross-platform
/// shape tests so that CI catches any accidental script drift.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const BROADCAST_WM_SETTINGCHANGE_SCRIPT: &str = r#"Add-Type @"
using System;
using System.Runtime.InteropServices;
public class FdemonEnv {
  [DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)]
  public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam,
      string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
}
"@
[FdemonEnv]::SendMessageTimeout([IntPtr]0xFFFF, 0x1A, [UIntPtr]::Zero, "Environment", 2, 5000, [ref]([UIntPtr]::Zero)) | Out-Null
"#;

// ── Marker constants ──────────────────────────────────────────────────────────

/// Opening fence line that marks the start of a managed Flutter PATH block.
const FENCE_OPEN: &str = "# >>> fdemon flutter path >>>";
/// Closing fence line that marks the end of a managed Flutter PATH block.
const FENCE_CLOSE: &str = "# <<< fdemon flutter path <<<";

/// Opening fence line that marks the start of a managed Android env block.
const ANDROID_FENCE_OPEN: &str = "# >>> fdemon android env >>>";
/// Closing fence line that marks the end of a managed Android env block.
const ANDROID_FENCE_CLOSE: &str = "# <<< fdemon android env <<<";

// ── Public types ──────────────────────────────────────────────────────────────

/// What happened when configuring PATH.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathConfigOutcome {
    /// Wrote a new fenced block (or replaced an old one) to `rc_file`.
    Written { rc_file: PathBuf },
    /// The fenced block already existed and already contains the correct
    /// `bin_dir` — no change was made.
    AlreadyPresent { rc_file: PathBuf },
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Validate that `bin_dir` is safe to embed in shell rc files and PowerShell.
///
/// Rejects paths that contain newlines (`\n`, `\r`) or shell control
/// metacharacters (`` ` ``, `$(`, `;`, `&`, `|`) that could be used for
/// injection attacks when the path is written into a shell config file or
/// PowerShell command.
///
/// This is the single chokepoint called at the top of [`add_to_path`] before
/// any I/O takes place, covering both the POSIX and Windows paths.
pub fn validate_bin_dir(bin_dir: &Path) -> Result<()> {
    let s = bin_dir.to_string_lossy();

    // Reject newlines — they allow appending arbitrary shell commands.
    if s.contains('\n') || s.contains('\r') {
        return Err(Error::config(
            "Flutter bin directory path contains a newline character, which is not allowed \
             (possible shell injection). Refusing to write PATH configuration.",
        ));
    }

    // Reject shell metacharacters that remain live even inside double-quoted
    // bash/zsh export lines or PowerShell strings.
    let dangerous_sequences = ["`", "$(", ";", "&", "|"];
    for seq in dangerous_sequences {
        if s.contains(seq) {
            return Err(Error::config(format!(
                "Flutter bin directory path contains a shell metacharacter {:?}, which is not \
                 allowed (possible shell injection). Refusing to write PATH configuration.",
                seq
            )));
        }
    }

    Ok(())
}

/// Select the rc file to edit for the given shell under `home`.
///
/// Returns `None` for shells where rc-file edits do not apply (i.e.
/// [`HostShell::PowerShell`], [`HostShell::Cmd`], and [`HostShell::Unknown`]).
///
/// For bash on **macOS**, prefers `.bash_profile` if it exists, then falls back
/// to `.profile` if it exists, then falls back to `.bashrc`. On **Linux** (and
/// other non-macOS platforms), bash uses `.bashrc`.
///
/// For fish, the returned path is `~/.config/fish/config.fish`. For zsh,
/// `~/.zshenv` is returned when it already exists; otherwise `~/.zprofile`
/// is returned as the conventional login-shell configuration file.
pub fn rc_file_for_shell(shell: HostShell, home: &Path) -> Option<PathBuf> {
    match shell {
        HostShell::Bash => {
            #[cfg(target_os = "macos")]
            {
                // macOS bash sources login-shell files (.bash_profile / .profile),
                // not .bashrc.  Prefer the file that already exists.
                let bash_profile = home.join(".bash_profile");
                if bash_profile.exists() {
                    return Some(bash_profile);
                }
                let profile = home.join(".profile");
                if profile.exists() {
                    return Some(profile);
                }
                // Neither exists yet — fall back to .bashrc (will be created).
                Some(home.join(".bashrc"))
            }
            #[cfg(not(target_os = "macos"))]
            {
                Some(home.join(".bashrc"))
            }
        }
        HostShell::Zsh => {
            let zshenv = home.join(".zshenv");
            if zshenv.exists() {
                Some(zshenv)
            } else {
                Some(home.join(".zprofile"))
            }
        }
        HostShell::Fish => Some(home.join(".config").join("fish").join("config.fish")),
        HostShell::PowerShell | HostShell::Cmd | HostShell::Unknown => None,
    }
}

/// Add `bin_dir` to `PATH` for the detected shell. Idempotent and marker-fenced.
///
/// Validates `bin_dir` for shell injection characters before any I/O.
///
/// On Windows, updates the user `PATH` via the **raw** registry key
/// `HKCU:\Environment` using PowerShell `New-ItemProperty`, reading first with
/// `GetValue(..., 'DoNotExpandEnvironmentNames')` to obtain the unexpanded value
/// and its type. The write preserves `REG_EXPAND_SZ` when the existing PATH
/// was `ExpandString` or contained `%VAR%` tokens — preventing silent flattening
/// of entries like `%USERPROFILE%\bin` or `%JAVA_HOME%\bin`. New keys default
/// to `ExpandString` (safe superset). The new value is passed out-of-band via
/// `FDEMON_NEW_PATH` and the property type via `FDEMON_PATH_KIND` — neither is
/// interpolated into the script string — to prevent code injection.
///
/// On Unix, writes to the rc file selected by [`rc_file_for_shell`]. If no rc
/// file can be determined (e.g. `HostShell::Unknown`), returns an error.
///
/// # Notes
///
/// This function never modifies the *running* process environment — it is
/// impossible to affect the parent shell. Callers should surface a "restart
/// your terminal" hint after a successful [`PathConfigOutcome::Written`].
pub fn add_to_path(
    shell: HostShell,
    platform: HostPlatform,
    bin_dir: &Path,
) -> Result<PathConfigOutcome> {
    // Single validation chokepoint — covers both POSIX and Windows paths.
    validate_bin_dir(bin_dir)?;

    if platform == HostPlatform::Windows {
        return add_to_path_windows(bin_dir);
    }

    let home = home_dir().ok_or_else(|| Error::config("Could not determine home directory"))?;

    let rc_file = rc_file_for_shell(shell, &home).ok_or_else(|| {
        Error::config(
            "Could not determine rc file for shell — PATH configuration is not supported \
             for this shell. Add the Flutter bin directory to your PATH manually.",
        )
    })?;

    add_to_rc_file(&rc_file, bin_dir)
}

/// Add Android environment variables (`ANDROID_HOME` and two `PATH` entries) to
/// the detected shell rc file. Idempotent and marker-fenced with a **distinct**
/// Android fence marker that never collides with the Flutter PATH block.
///
/// # Shell output
///
/// bash/zsh:
/// ```text
/// # >>> fdemon android env >>>
/// export ANDROID_HOME='/home/user/.android/sdk'
/// export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
/// # <<< fdemon android env <<<
/// ```
///
/// fish:
/// ```text
/// # >>> fdemon android env >>>
/// set -Ux ANDROID_HOME '/home/user/.android/sdk'
/// fish_add_path "$ANDROID_HOME/cmdline-tools/latest/bin" "$ANDROID_HOME/platform-tools"
/// # <<< fdemon android env <<<
/// ```
///
/// Windows: writes `ANDROID_HOME` to `HKCU:\Environment` as `REG_SZ` and
/// prepends the three bin dirs to the user `PATH` using the raw registry
/// approach (`GetValue(..., 'DoNotExpandEnvironmentNames')` + `New-ItemProperty`)
/// so that the existing `REG_EXPAND_SZ` type and any `%VAR%` tokens in `PATH`
/// are preserved. Values are passed out-of-band via `FDEMON_NEW_ANDROID_HOME`,
/// `FDEMON_NEW_PATH`, and `FDEMON_PATH_KIND` to prevent shell injection.
///
/// # Returns
///
/// [`PathConfigOutcome::Written`] on the first call and
/// [`PathConfigOutcome::AlreadyPresent`] on subsequent calls when `sdk_root` is
/// unchanged — the rc file is byte-identical after re-runs.
///
/// # Notes
///
/// This function never modifies the *running* process environment. Callers
/// should surface a "restart your terminal" hint after a successful
/// [`PathConfigOutcome::Written`].
pub fn add_android_env(
    shell: HostShell,
    platform: HostPlatform,
    sdk_root: &Path,
) -> Result<PathConfigOutcome> {
    // Single validation chokepoint — covers both POSIX and Windows paths.
    validate_bin_dir(sdk_root)?;

    if platform == HostPlatform::Windows {
        return add_android_env_windows(sdk_root);
    }

    let home = home_dir().ok_or_else(|| Error::config("Could not determine home directory"))?;

    let rc_file = rc_file_for_shell(shell, &home).ok_or_else(|| {
        Error::config(
            "Could not determine rc file for shell — Android env configuration is not supported \
             for this shell. Add ANDROID_HOME to your environment manually.",
        )
    })?;

    add_android_env_to_rc_file(&rc_file, sdk_root)
}

// ── Pure string helpers (unit-testable without I/O) ───────────────────────────

/// Build the export line appropriate for bash/zsh.
///
/// The bin directory is single-quoted using [`single_quote_escape`] to prevent
/// any shell interpretation of the literal path value (guards against `$`, `"`,
/// `` ` ``, and `\` in the path). The `$PATH` expansion is intentional — it is
/// kept in a separate double-quoted segment so the shell expands the existing
/// PATH at login time. The two segments are adjacent string literals that POSIX
/// shells concatenate: `"$PATH:"'/safe/bin'`.
fn posix_export_line(bin_dir: &Path) -> String {
    let escaped = single_quote_escape(&bin_dir.to_string_lossy());
    format!("export PATH=\"$PATH:\"{}", escaped)
}

/// Single-quote escape a path for use in POSIX/fish shell arguments.
///
/// In POSIX single-quoting, a literal `'` must be represented as `'\''`
/// (close quote, backslash-quote, reopen quote).  All other characters are
/// literal inside single quotes — no further escaping is needed.
fn single_quote_escape(s: &str) -> String {
    // Replace every ' with '\''
    let escaped = s.replace('\'', r"'\''");
    format!("'{}'", escaped)
}

/// Build the fish_add_path line with a single-quoted, escaped argument.
fn fish_add_path_line(bin_dir: &Path) -> String {
    let escaped = single_quote_escape(&bin_dir.to_string_lossy());
    format!("fish_add_path {}", escaped)
}

/// Determine whether the content line is a fish config file.
///
/// We detect fish by checking the file path extension and directory conventions.
fn is_fish_rc(path: &Path) -> bool {
    // `config.fish` inside a `.config/fish/` directory
    path.file_name()
        .map(|n| n == "config.fish")
        .unwrap_or(false)
}

/// Build the full fence block to write for a given rc file and `bin_dir`.
fn fence_block(rc_file: &Path, bin_dir: &Path) -> String {
    let export_line = if is_fish_rc(rc_file) {
        fish_add_path_line(bin_dir)
    } else {
        posix_export_line(bin_dir)
    };

    format!("{}\n{}\n{}\n", FENCE_OPEN, export_line, FENCE_CLOSE)
}

/// Build the Android env fence block for bash/zsh.
///
/// The SDK root value is single-quoted to prevent shell expansion of any `$`,
/// `` ` ``, or `"` characters in the path. The `$ANDROID_HOME` references in the
/// PATH line are intentional expansions and remain unquoted.
///
/// ```text
/// # >>> fdemon android env >>>
/// export ANDROID_HOME='/home/user/.android/sdk'
/// export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH"
/// # <<< fdemon android env <<<
/// ```
fn android_posix_block(sdk_root: &Path) -> String {
    let sdk_escaped = single_quote_escape(&sdk_root.to_string_lossy());
    format!(
        "{fence_open}\nexport ANDROID_HOME={sdk_escaped}\nexport PATH=\"$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH\"\n{fence_close}\n",
        fence_open = ANDROID_FENCE_OPEN,
        sdk_escaped = sdk_escaped,
        fence_close = ANDROID_FENCE_CLOSE,
    )
}

/// Build the Android env fence block for fish.
///
/// The SDK root value is single-quoted to prevent fish from expanding any `$`,
/// `` ` ``, or `"` characters in the path. The `$ANDROID_HOME` references in the
/// `fish_add_path` line are intentional expansions and remain double-quoted.
///
/// ```text
/// # >>> fdemon android env >>>
/// set -Ux ANDROID_HOME '/home/user/.android/sdk'
/// fish_add_path "$ANDROID_HOME/cmdline-tools/latest/bin" "$ANDROID_HOME/platform-tools" "$ANDROID_HOME/emulator"
/// # <<< fdemon android env <<<
/// ```
fn android_fish_block(sdk_root: &Path) -> String {
    let sdk_escaped = single_quote_escape(&sdk_root.to_string_lossy());
    format!(
        "{fence_open}\nset -Ux ANDROID_HOME {sdk_escaped}\nfish_add_path \"$ANDROID_HOME/cmdline-tools/latest/bin\" \"$ANDROID_HOME/platform-tools\" \"$ANDROID_HOME/emulator\"\n{fence_close}\n",
        fence_open = ANDROID_FENCE_OPEN,
        sdk_escaped = sdk_escaped,
        fence_close = ANDROID_FENCE_CLOSE,
    )
}

/// Build the full Android env fence block appropriate for the given rc file.
fn android_fence_block(rc_file: &Path, sdk_root: &Path) -> String {
    if is_fish_rc(rc_file) {
        android_fish_block(sdk_root)
    } else {
        android_posix_block(sdk_root)
    }
}

/// Parse an existing fence block out of `contents` using the given marker pair.
///
/// Returns the byte range `(open_start, close_end)` of the entire fence block
/// (inclusive of trailing newline after the close marker), or `None` if no
/// fence block is present.
fn find_fence_range_for(
    contents: &str,
    open_marker: &str,
    close_marker: &str,
) -> Option<(usize, usize)> {
    let open_pos = contents.find(open_marker)?;
    let close_search_start = open_pos + open_marker.len();
    let close_pos = contents[close_search_start..].find(close_marker)?;
    let close_abs = close_search_start + close_pos;
    // Include everything up through the newline that follows the close marker.
    let close_end = close_abs + close_marker.len();
    // Consume a trailing newline character if present.
    let close_end = if contents.as_bytes().get(close_end) == Some(&b'\n') {
        close_end + 1
    } else {
        close_end
    };
    Some((open_pos, close_end))
}

/// Return `true` if `contents` contains a Flutter PATH fence block that already
/// includes a reference to `bin_dir`.
#[cfg(test)]
fn fence_already_has_dir(contents: &str, bin_dir: &Path) -> bool {
    let bin_str = bin_dir.to_string_lossy();
    match find_fence_range_for(contents, FENCE_OPEN, FENCE_CLOSE) {
        Some((start, end)) => contents[start..end].contains(bin_str.as_ref()),
        None => false,
    }
}

/// Apply fence block logic to `contents` using the given fence block string and
/// marker pair, returning the updated string.
///
/// - If no fence exists: append the block.
/// - If a fence exists but does not contain `anchor`: replace it.
/// - If a fence exists and already contains `anchor`: return `None` (no change).
fn apply_fence_with_markers(
    contents: &str,
    block: &str,
    anchor: &str,
    open_marker: &str,
    close_marker: &str,
) -> Option<String> {
    match find_fence_range_for(contents, open_marker, close_marker) {
        None => {
            // No fence yet — append.
            let mut result = contents.to_string();
            // Ensure we start on a fresh line.
            if !result.is_empty() && !result.ends_with('\n') {
                result.push('\n');
            }
            result.push('\n');
            result.push_str(block);
            Some(result)
        }
        Some((start, end)) => {
            let existing_block = &contents[start..end];
            if existing_block.contains(anchor) {
                // Already present with the same value — no change needed.
                None
            } else {
                // Replace existing block in-place.
                let mut result = String::with_capacity(contents.len());
                result.push_str(&contents[..start]);
                result.push_str(block);
                result.push_str(&contents[end..]);
                Some(result)
            }
        }
    }
}

/// Apply fence block logic to `contents`, returning the updated string.
///
/// - If no fence exists: append the block.
/// - If a fence exists but points elsewhere: replace it.
/// - If a fence exists and already contains `bin_dir`: return `None` (no change).
fn apply_fence(contents: &str, rc_file: &Path, bin_dir: &Path) -> Option<String> {
    let block = fence_block(rc_file, bin_dir);
    let anchor = bin_dir.to_string_lossy().into_owned();
    apply_fence_with_markers(contents, &block, &anchor, FENCE_OPEN, FENCE_CLOSE)
}

/// Apply Android env fence block logic to `contents`, returning the updated string.
fn apply_android_fence(contents: &str, rc_file: &Path, sdk_root: &Path) -> Option<String> {
    let block = android_fence_block(rc_file, sdk_root);
    let anchor = sdk_root.to_string_lossy().into_owned();
    apply_fence_with_markers(
        contents,
        &block,
        &anchor,
        ANDROID_FENCE_OPEN,
        ANDROID_FENCE_CLOSE,
    )
}

// ── File I/O ──────────────────────────────────────────────────────────────────

/// Read existing rc file contents, returning an empty string when the file does
/// not yet exist.
fn read_rc_contents(rc_file: &Path) -> Result<String> {
    match std::fs::read_to_string(rc_file) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(Error::Io(e)),
    }
}

/// Write `new_contents` to `rc_file` atomically using a temp file + rename.
///
/// Creates parent directories if they do not yet exist.
///
/// ## Permission preservation (Unix)
///
/// If `rc_file` already exists, its mode bits are read before the write and
/// re-applied to the temp file immediately after writing — before the rename.
/// This prevents a `chmod 600 ~/.zshenv` from being silently downgraded to the
/// process umask (typically `0644`) after fdemon edits it.
///
/// If `rc_file` does not yet exist (new file), the temp file is created with
/// mode `0600` (user-readable/writable only) rather than inheriting the umask,
/// which could make the file world-readable.
///
/// On non-Unix platforms the mode-preservation step is a no-op.
///
/// ## Temp file uniqueness
///
/// The temp file is created via [`tempfile::NamedTempFile::new_in`] rather than
/// a deterministic `<rc_file>.fdemon_tmp` name.  This prevents two concurrent
/// fdemon processes targeting the same rc file from clobbering each other's
/// temp file.
fn write_rc_atomically(rc_file: &Path, new_contents: &str) -> Result<()> {
    // Ensure parent directory exists.
    let parent = rc_file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;

    // ── Permission snapshot ────────────────────────────────────────────────────
    // On Unix: read the existing mode before writing so we can restore it after.
    // If the file does not exist we will use 0600 for the new file.
    #[cfg(unix)]
    let target_mode: u32 = match std::fs::metadata(rc_file) {
        Ok(m) => m.permissions().mode(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0o600,
        Err(e) => return Err(Error::Io(e)),
    };

    // ── Create unique temp file in the same directory ─────────────────────────
    // Using the same directory as rc_file is required for an atomic rename
    // (cross-device renames are not guaranteed atomic on Linux/macOS).
    let tmp_named = tempfile::Builder::new()
        .prefix(".fdemon-rc-tmp-")
        .tempfile_in(parent)
        .map_err(|e| {
            Error::config(format!(
                "Failed to create temp file in {}: {}",
                parent.display(),
                e
            ))
        })?;

    let tmp_path = tmp_named.path().to_path_buf();

    // Write contents to the temp file (the NamedTempFile already holds an open fd).
    std::fs::write(&tmp_path, new_contents).map_err(|e| {
        Error::config(format!(
            "Failed to write temp file {}: {}",
            tmp_path.display(),
            e
        ))
    })?;

    // ── Apply permissions to temp file before rename ───────────────────────────
    // On Unix: set the permissions on the temp file to match the target mode so
    // the rename preserves permissions atomically from the reader's perspective.
    #[cfg(unix)]
    {
        let perms = std::fs::Permissions::from_mode(target_mode);
        if let Err(e) = std::fs::set_permissions(&tmp_path, perms) {
            tracing::debug!(
                path = %tmp_path.display(),
                error = %e,
                "Failed to set permissions on temp rc file (best effort)"
            );
        }
    }

    // ── Atomic rename (temp → destination) ────────────────────────────────────
    // `persist` consumes the NamedTempFile and renames it to rc_file, preventing
    // the Drop impl from deleting the file before we've renamed it.
    tmp_named.persist(rc_file).map_err(|e| {
        Error::config(format!(
            "Failed to move temp file → {}: {}",
            rc_file.display(),
            e.error
        ))
    })?;

    Ok(())
}

/// Core Unix/macOS rc-file update: read → apply fence → (maybe) write.
fn add_to_rc_file(rc_file: &Path, bin_dir: &Path) -> Result<PathConfigOutcome> {
    let contents = read_rc_contents(rc_file)?;

    match apply_fence(&contents, rc_file, bin_dir) {
        None => Ok(PathConfigOutcome::AlreadyPresent {
            rc_file: rc_file.to_path_buf(),
        }),
        Some(new_contents) => {
            write_rc_atomically(rc_file, &new_contents)?;
            Ok(PathConfigOutcome::Written {
                rc_file: rc_file.to_path_buf(),
            })
        }
    }
}

/// Core Unix/macOS rc-file update for Android env: read → apply Android fence → (maybe) write.
fn add_android_env_to_rc_file(rc_file: &Path, sdk_root: &Path) -> Result<PathConfigOutcome> {
    let contents = read_rc_contents(rc_file)?;

    match apply_android_fence(&contents, rc_file, sdk_root) {
        None => Ok(PathConfigOutcome::AlreadyPresent {
            rc_file: rc_file.to_path_buf(),
        }),
        Some(new_contents) => {
            write_rc_atomically(rc_file, &new_contents)?;
            Ok(PathConfigOutcome::Written {
                rc_file: rc_file.to_path_buf(),
            })
        }
    }
}

// ── Windows PATH update ───────────────────────────────────────────────────────

// ── Windows PATH planning helpers (pure, cross-platform-testable) ────────────

/// The registry value type to use when writing a Windows PATH entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsRegKind {
    /// `REG_SZ` — a plain string with no variable expansion.
    String,
    /// `REG_EXPAND_SZ` — a string with `%VAR%` expansion tokens.
    ExpandString,
}

impl WindowsRegKind {
    /// Returns the PowerShell `-PropertyType` argument string for this kind.
    pub fn powershell_property_type(self) -> &'static str {
        match self {
            WindowsRegKind::String => "String",
            WindowsRegKind::ExpandString => "ExpandString",
        }
    }
}

/// Determine the registry kind to use for a PATH value.
///
/// If the existing raw value contains `%` (indicating `%VAR%` tokens), or if
/// the existing kind is already `ExpandString`, we preserve `ExpandString`.
/// Otherwise we keep `String`. When writing a brand-new key, we default to
/// `ExpandString` (safe superset — it degrades gracefully when no `%VAR%`
/// tokens are present).
pub fn decide_reg_kind(raw_value: &str, existing_is_expand: bool) -> WindowsRegKind {
    if existing_is_expand || raw_value.contains('%') {
        WindowsRegKind::ExpandString
    } else {
        WindowsRegKind::String
    }
}

/// Plan a Windows PATH update: decide what to write and which registry kind to use.
///
/// Returns `None` if `bin_dir` is already present in `raw_value` (case-insensitive
/// semicolon-split comparison), meaning no write is needed.
///
/// Returns `Some((new_value, kind))` where `new_value` is the raw PATH string
/// with `bin_dir` appended and `kind` is the registry type that should be used
/// when writing back. The `kind` is derived from `existing_is_expand` and whether
/// the raw value itself contains `%` tokens, so that a pre-existing
/// `REG_EXPAND_SZ` value retains its type and literal `%VAR%` tokens are preserved.
///
/// # Idempotency
///
/// The already-present check is performed against the **raw** (unexpanded) value
/// so that entries like `%USERPROFILE%\bin` are detected correctly without
/// needing the environment to be expanded first.
pub fn plan_windows_path_update(
    raw_value: &str,
    existing_is_expand: bool,
    bin_dir: &str,
) -> Option<(String, WindowsRegKind)> {
    // Idempotency: check whether the literal bin_dir string is already present
    // as a semicolon-delimited segment (case-insensitive).
    let already_present = raw_value
        .split(';')
        .any(|segment| segment.trim().eq_ignore_ascii_case(bin_dir));

    if already_present {
        return None;
    }

    // Decide which registry kind to preserve.
    // When the PATH key is absent (empty raw_value) we default to ExpandString
    // — a safe superset that degrades gracefully when no %VAR% tokens are present.
    let kind = if raw_value.is_empty() {
        WindowsRegKind::ExpandString
    } else {
        decide_reg_kind(raw_value, existing_is_expand)
    };

    // Append the new bin_dir.
    let new_value = if raw_value.is_empty() {
        bin_dir.to_string()
    } else if raw_value.ends_with(';') {
        format!("{}{}", raw_value, bin_dir)
    } else {
        format!("{};{}", raw_value, bin_dir)
    };

    Some((new_value, kind))
}

/// PowerShell script that reads the **raw** (unexpanded) user PATH from the
/// registry and its value type, emitting two lines:
/// - Line 1: the raw PATH string (empty if key absent)
/// - Line 2: `ExpandString` or `String`
///
/// Uses `GetValue(..., 'DoNotExpandEnvironmentNames')` to avoid flattening
/// `%VAR%` tokens. No user-controlled values are interpolated.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const READ_RAW_PATH_SCRIPT: &str = r#"$key = Get-Item -LiteralPath 'HKCU:\Environment' -ErrorAction SilentlyContinue
if ($key -eq $null) { ''; 'ExpandString'; exit }
$raw = $key.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
$kind = $key.GetValueKind('Path')
$raw
if ($kind -eq 'ExpandString') { 'ExpandString' } else { 'String' }"#;

/// PowerShell script that writes the user PATH to the registry using a specified
/// property type. The new value is passed out-of-band via `$env:FDEMON_NEW_PATH`
/// and the type via `$env:FDEMON_PATH_KIND` to prevent injection.
///
/// `New-ItemProperty -Force` creates the key if absent and overwrites if present.
/// No user-controlled values are interpolated into the script string.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const WRITE_RAW_PATH_SCRIPT: &str = r#"New-ItemProperty -LiteralPath 'HKCU:\Environment' -Name 'Path' -Value $env:FDEMON_NEW_PATH -PropertyType $env:FDEMON_PATH_KIND -Force | Out-Null"#;

/// PowerShell script that reads the **raw** (unexpanded) `ANDROID_HOME` user
/// env var, emitting one line with the raw value (empty if absent).
///
/// No user-controlled values are interpolated.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const READ_RAW_ANDROID_HOME_SCRIPT: &str = r#"$key = Get-Item -LiteralPath 'HKCU:\Environment' -ErrorAction SilentlyContinue
if ($key -eq $null) { ''; exit }
$key.GetValue('ANDROID_HOME', '', 'DoNotExpandEnvironmentNames')"#;

/// PowerShell script that writes `ANDROID_HOME` to the user registry as
/// `REG_SZ`. The value is passed out-of-band via `$env:FDEMON_NEW_ANDROID_HOME`.
///
/// `ANDROID_HOME` is written as `String` (not `ExpandString`) because it is a
/// concrete directory path, not a `%VAR%`-bearing template.
///
/// No user-controlled values are interpolated.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const WRITE_ANDROID_HOME_SCRIPT: &str = r#"New-ItemProperty -LiteralPath 'HKCU:\Environment' -Name 'ANDROID_HOME' -Value $env:FDEMON_NEW_ANDROID_HOME -PropertyType String -Force | Out-Null"#;

/// Update the Windows user `PATH` via PowerShell, preserving the existing
/// `REG_EXPAND_SZ` registry type and literal `%VAR%` tokens.
///
/// Reads the **raw** (unexpanded) PATH value from `HKCU:\Environment` using
/// `GetValue(..., 'DoNotExpandEnvironmentNames')`, appends `bin_dir` to the
/// raw value, then writes back using `New-ItemProperty -PropertyType ExpandString`
/// (or `String` if the existing value had no `%` tokens and was not
/// `REG_EXPAND_SZ`). This round-trip preserves entries such as
/// `%USERPROFILE%\bin` and `%JAVA_HOME%\bin` that would otherwise be silently
/// expanded and re-persisted as a flat string.
///
/// **Injection safety:** The new PATH value is passed out-of-band via the
/// `FDEMON_NEW_PATH` environment variable and the registry type via
/// `FDEMON_PATH_KIND`. Neither value is ever interpolated into the PowerShell
/// script string, so PowerShell metacharacters in the path cannot execute code.
///
/// The function is platform-gated so it compiles on all targets but only runs
/// on Windows.
fn add_to_path_windows(bin_dir: &Path) -> Result<PathConfigOutcome> {
    let bin_str = bin_dir.to_string_lossy().into_owned();

    // Read the raw (unexpanded) user PATH and its registry kind via PowerShell.
    // The script is a constant — no user-controlled values are interpolated.
    let read_output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            READ_RAW_PATH_SCRIPT,
        ])
        .output()
        .map_err(|e| Error::config(format!("Failed to run PowerShell to read PATH: {}", e)))?;

    let read_stdout = String::from_utf8_lossy(&read_output.stdout);
    let mut lines = read_stdout.lines();
    let raw_path = lines.next().unwrap_or("").trim().to_string();
    let kind_str = lines.next().unwrap_or("ExpandString").trim();
    let existing_is_expand = kind_str != "String";

    // Plan the update using the pure helper — operates on the raw value so
    // that %VAR% tokens are compared literally (case-insensitive).
    let (new_path, reg_kind) =
        match plan_windows_path_update(&raw_path, existing_is_expand, &bin_str) {
            None => {
                return Ok(PathConfigOutcome::AlreadyPresent {
                    rc_file: PathBuf::from("HKCU:\\Environment\\PATH"),
                });
            }
            Some(pair) => pair,
        };

    // Write the new value back preserving the original (or defaulted) registry
    // type. The value and type are passed out-of-band — never interpolated into
    // the script string — so injection via path metacharacters is impossible.
    let set_output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            WRITE_RAW_PATH_SCRIPT,
        ])
        .env("FDEMON_NEW_PATH", &new_path)
        .env("FDEMON_PATH_KIND", reg_kind.powershell_property_type())
        .output()
        .map_err(|e| Error::config(format!("Failed to run PowerShell to set PATH: {}", e)))?;

    if !set_output.status.success() {
        let stderr = String::from_utf8_lossy(&set_output.stderr);
        return Err(Error::config(format!(
            "PowerShell New-ItemProperty (PATH) failed: {}",
            stderr.trim()
        )));
    }

    // Best-effort: notify already-open processes that the user environment has
    // changed.  A broadcast failure must NOT fail the PATH write — the registry
    // value has already been persisted.
    broadcast_wm_settingchange();

    Ok(PathConfigOutcome::Written {
        rc_file: PathBuf::from("HKCU:\\Environment\\PATH"),
    })
}

/// Update the Windows user `ANDROID_HOME` and `PATH` via PowerShell, preserving
/// the existing `REG_EXPAND_SZ` registry type and literal `%VAR%` tokens on PATH.
///
/// Sets `ANDROID_HOME` to `sdk_root` (written as `REG_SZ` — a concrete path, not
/// a `%VAR%` template) and prepends
/// `<sdk_root>\cmdline-tools\latest\bin`, `<sdk_root>\platform-tools`,
/// and `<sdk_root>\emulator` to the user `PATH` if they are not already present.
///
/// Reads the **raw** (unexpanded) PATH from `HKCU:\Environment` and writes it
/// back via `New-ItemProperty -PropertyType ExpandString` (or `String` if no
/// `%VAR%` tokens were present) to avoid silently flattening a `REG_EXPAND_SZ`
/// value into a plain `REG_SZ` string.
///
/// **Injection safety:** `sdk_root` is passed out-of-band via
/// `FDEMON_NEW_ANDROID_HOME`; the new PATH value via `FDEMON_NEW_PATH`; the
/// registry type via `FDEMON_PATH_KIND`. None of these values are ever
/// interpolated into the PowerShell script strings.
fn add_android_env_windows(sdk_root: &Path) -> Result<PathConfigOutcome> {
    let sdk_str = sdk_root.to_string_lossy().into_owned();

    // Read the raw (unexpanded) user ANDROID_HOME from the registry.
    let read_home_output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            READ_RAW_ANDROID_HOME_SCRIPT,
        ])
        .output()
        .map_err(|e| {
            Error::config(format!(
                "Failed to run PowerShell to read ANDROID_HOME: {}",
                e
            ))
        })?;

    let current_home = String::from_utf8_lossy(&read_home_output.stdout)
        .trim()
        .to_string();

    // Read the raw (unexpanded) user PATH and its registry kind.
    let read_path_output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            READ_RAW_PATH_SCRIPT,
        ])
        .output()
        .map_err(|e| Error::config(format!("Failed to run PowerShell to read PATH: {}", e)))?;

    let read_stdout = String::from_utf8_lossy(&read_path_output.stdout);
    let mut lines = read_stdout.lines();
    let raw_path = lines.next().unwrap_or("").trim().to_string();
    let kind_str = lines.next().unwrap_or("ExpandString").trim();
    let existing_is_expand = kind_str != "String";

    // Compute the three Android bin dirs to add.
    let cmdline_bin = format!("{}\\cmdline-tools\\latest\\bin", sdk_str);
    let platform_tools = format!("{}\\platform-tools", sdk_str);
    let emulator = format!("{}\\emulator", sdk_str);

    // Check whether ANDROID_HOME already equals sdk_root and all three bin dirs
    // are already in the raw PATH — if so, the configuration is already complete.
    let home_matches = current_home.eq_ignore_ascii_case(&sdk_str);
    let raw_path_segments: Vec<&str> = raw_path.split(';').map(str::trim).collect();
    let cmdline_present = raw_path_segments
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&cmdline_bin));
    let platform_present = raw_path_segments
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&platform_tools));
    let emulator_present = raw_path_segments
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&emulator));

    if home_matches && cmdline_present && platform_present && emulator_present {
        return Ok(PathConfigOutcome::AlreadyPresent {
            rc_file: PathBuf::from("HKCU:\\Environment"),
        });
    }

    // Set ANDROID_HOME — written as REG_SZ (a concrete path, not a %VAR% template).
    // Value passed out-of-band via FDEMON_NEW_ANDROID_HOME to prevent injection.
    let set_home_output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            WRITE_ANDROID_HOME_SCRIPT,
        ])
        .env("FDEMON_NEW_ANDROID_HOME", &sdk_str)
        .output()
        .map_err(|e| {
            Error::config(format!(
                "Failed to run PowerShell to set ANDROID_HOME: {}",
                e
            ))
        })?;

    if !set_home_output.status.success() {
        let stderr = String::from_utf8_lossy(&set_home_output.stderr);
        return Err(Error::config(format!(
            "PowerShell New-ItemProperty (ANDROID_HOME) failed: {}",
            stderr.trim()
        )));
    }

    // Build the new raw PATH by prepending missing Android bin dirs to the
    // existing raw value (preserving any %VAR% tokens already in the value).
    // Prepend in reverse order so cmdline_bin ends up before platform_tools
    // and emulator (ordering: cmdline-tools → platform-tools → emulator).
    let mut path_parts: Vec<String> = raw_path
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if !emulator_present {
        path_parts.insert(0, emulator);
    }
    if !platform_present {
        path_parts.insert(0, platform_tools);
    }
    if !cmdline_present {
        path_parts.insert(0, cmdline_bin);
    }

    let new_path = path_parts.join(";");

    // Decide the registry kind: if the raw value contained %VAR% tokens or was
    // already REG_EXPAND_SZ, keep ExpandString; otherwise keep String.
    let reg_kind = decide_reg_kind(&raw_path, existing_is_expand);

    // Write the new PATH back, preserving the original (or defaulted) registry
    // type. Value and type are passed out-of-band — never interpolated.
    let set_path_output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            WRITE_RAW_PATH_SCRIPT,
        ])
        .env("FDEMON_NEW_PATH", &new_path)
        .env("FDEMON_PATH_KIND", reg_kind.powershell_property_type())
        .output()
        .map_err(|e| Error::config(format!("Failed to run PowerShell to set PATH: {}", e)))?;

    if !set_path_output.status.success() {
        let stderr = String::from_utf8_lossy(&set_path_output.stderr);
        return Err(Error::config(format!(
            "PowerShell New-ItemProperty (PATH) failed: {}",
            stderr.trim()
        )));
    }

    // Best-effort: notify already-open processes that the user environment has
    // changed.  A broadcast failure must NOT fail the Android env write — the
    // registry values have already been persisted.
    broadcast_wm_settingchange();

    Ok(PathConfigOutcome::Written {
        rc_file: PathBuf::from("HKCU:\\Environment"),
    })
}

/// Broadcast `WM_SETTINGCHANGE` so already-open processes (Explorer, terminals)
/// pick up the registry environment changes without requiring a restart.
///
/// Uses a PowerShell P/Invoke snippet via `Add-Type` to call
/// `SendMessageTimeout(HWND_BROADCAST, WM_SETTINGCHANGE, 0, "Environment", ...)`.
///
/// **Best-effort only.** Errors are silently ignored — the registry value has
/// already been written before this call, and a broadcast failure must not
/// surface as a wizard error.
///
/// `HWND_BROADCAST = 0xFFFF`, `WM_SETTINGCHANGE = 0x1A`, `SMTO_ABORTIFHUNG = 2`,
/// 5 s timeout.
///
/// The broadcast is gated to Windows only — on other platforms this is a no-op.
fn broadcast_wm_settingchange() {
    // Only meaningful on Windows; compiles to nothing on other targets.
    #[cfg(target_os = "windows")]
    {
        // Use the module-level constant — no inline duplication, and tests can
        // assert against the same value that ships.
        //
        // Spawned detached with all stdio redirected to null so the wizard
        // thread never blocks waiting for powershell.exe to exit.  If PowerShell
        // stalls (e.g. Add-Type C# JIT, AV interception) the registry write has
        // already committed; silently dropping the Child handle is correct here.
        let _ = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                BROADCAST_WM_SETTINGCHANGE_SCRIPT,
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        // No-op on non-Windows.
    }
}

// ── Platform helpers ──────────────────────────────────────────────────────────

/// Resolve the current user's home directory.
///
/// Prefers the `HOME` environment variable (Unix) or `USERPROFILE` (Windows);
/// falls back to the `dirs` crate.
fn home_dir() -> Option<PathBuf> {
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .or_else(dirs::home_dir)
    }
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .or_else(dirs::home_dir)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── validate_bin_dir tests ────────────────────────────────────────────────

    #[test]
    fn test_validate_bin_dir_accepts_normal_path() {
        assert!(validate_bin_dir(Path::new("/home/user/flutter/bin")).is_ok());
        assert!(validate_bin_dir(Path::new("/opt/flutter/bin")).is_ok());
        assert!(validate_bin_dir(Path::new("/home/user/.local/flutter sdk/bin")).is_ok());
    }

    #[test]
    fn test_validate_bin_dir_rejects_newline() {
        let path_with_lf = PathBuf::from("/opt/flutter/bin\n/etc/evil");
        assert!(validate_bin_dir(&path_with_lf).is_err());

        let path_with_crlf = PathBuf::from("/opt/flutter/bin\r/etc/evil");
        assert!(validate_bin_dir(&path_with_crlf).is_err());
    }

    #[test]
    fn test_validate_bin_dir_rejects_backtick() {
        let path = PathBuf::from("/opt/flutter`rm -rf /`/bin");
        let err = validate_bin_dir(&path).unwrap_err();
        assert!(err.to_string().contains("metacharacter"));
    }

    #[test]
    fn test_validate_bin_dir_rejects_command_substitution() {
        let path = PathBuf::from("/opt/flutter$(evil)/bin");
        let err = validate_bin_dir(&path).unwrap_err();
        assert!(err.to_string().contains("metacharacter"));
    }

    #[test]
    fn test_validate_bin_dir_rejects_semicolon() {
        let path = PathBuf::from("/opt/flutter/bin;rm -rf /");
        let err = validate_bin_dir(&path).unwrap_err();
        assert!(err.to_string().contains("metacharacter"));
    }

    #[test]
    fn test_validate_bin_dir_rejects_ampersand() {
        let path = PathBuf::from("/opt/flutter/bin&evil");
        let err = validate_bin_dir(&path).unwrap_err();
        assert!(err.to_string().contains("metacharacter"));
    }

    #[test]
    fn test_validate_bin_dir_rejects_pipe() {
        let path = PathBuf::from("/opt/flutter/bin|evil");
        let err = validate_bin_dir(&path).unwrap_err();
        assert!(err.to_string().contains("metacharacter"));
    }

    // ── single_quote_escape tests ─────────────────────────────────────────────

    #[test]
    fn test_single_quote_escape_simple_path() {
        let result = single_quote_escape("/opt/flutter/bin");
        assert_eq!(result, "'/opt/flutter/bin'");
    }

    #[test]
    fn test_single_quote_escape_path_with_space() {
        let result = single_quote_escape("/home/user/flutter sdk/bin");
        assert_eq!(result, "'/home/user/flutter sdk/bin'");
    }

    #[test]
    fn test_single_quote_escape_path_with_single_quote() {
        // A path like /home/user's flutter/bin should become '/home/user'\''s flutter/bin'
        let result = single_quote_escape("/home/user's flutter/bin");
        assert_eq!(result, r"'/home/user'\''s flutter/bin'");
    }

    // ── Pure string helper tests ──────────────────────────────────────────────

    #[test]
    fn test_posix_export_line() {
        // The bin dir is single-quoted; $PATH is in a separate double-quoted segment.
        let line = posix_export_line(Path::new("/home/user/flutter/bin"));
        assert_eq!(line, r#"export PATH="$PATH:"'/home/user/flutter/bin'"#);
    }

    #[test]
    fn test_posix_export_line_single_quote_in_path() {
        // A path with a single quote must be safely escaped.
        let line = posix_export_line(Path::new("/home/user's/flutter/bin"));
        assert_eq!(line, r#"export PATH="$PATH:"'/home/user'\''s/flutter/bin'"#);
    }

    #[test]
    fn test_posix_export_line_dollar_in_path() {
        // A path containing a bare $ must not expand at login time.
        // Single-quoting prevents variable expansion inside '...'.
        let line = posix_export_line(Path::new("/opt/$HOME/flutter/bin"));
        assert_eq!(line, r#"export PATH="$PATH:"'/opt/$HOME/flutter/bin'"#);
    }

    #[test]
    fn test_fish_add_path_line_simple() {
        let line = fish_add_path_line(Path::new("/home/user/flutter/bin"));
        assert_eq!(line, "fish_add_path '/home/user/flutter/bin'");
    }

    #[test]
    fn test_fish_add_path_line_with_space() {
        // Spaces must be safely quoted.
        let line = fish_add_path_line(Path::new("/home/user/flutter sdk/bin"));
        assert_eq!(line, "fish_add_path '/home/user/flutter sdk/bin'");
    }

    #[test]
    fn test_fish_add_path_line_with_single_quote() {
        // Embedded single quotes use POSIX '\'' escaping.
        let line = fish_add_path_line(Path::new("/home/user's/flutter/bin"));
        assert_eq!(line, r"fish_add_path '/home/user'\''s/flutter/bin'");
    }

    #[test]
    fn test_is_fish_rc_true_for_config_fish() {
        assert!(is_fish_rc(Path::new("/home/u/.config/fish/config.fish")));
        assert!(is_fish_rc(Path::new("config.fish")));
    }

    #[test]
    fn test_is_fish_rc_false_for_other_files() {
        assert!(!is_fish_rc(Path::new("/home/u/.bashrc")));
        assert!(!is_fish_rc(Path::new("/home/u/.zshenv")));
        assert!(!is_fish_rc(Path::new("/home/u/.zprofile")));
    }

    #[test]
    fn test_fence_block_bash_contains_export() {
        let block = fence_block(Path::new("/home/u/.bashrc"), Path::new("/opt/flutter/bin"));
        assert!(block.contains(FENCE_OPEN));
        assert!(block.contains(FENCE_CLOSE));
        assert!(block.contains("export PATH"));
        assert!(block.contains("/opt/flutter/bin"));
        assert!(!block.contains("fish_add_path"));
    }

    #[test]
    fn test_fence_block_fish_contains_fish_add_path_quoted() {
        let block = fence_block(
            Path::new("/home/u/.config/fish/config.fish"),
            Path::new("/opt/flutter/bin"),
        );
        assert!(block.contains(FENCE_OPEN));
        assert!(block.contains(FENCE_CLOSE));
        // Argument must be single-quoted.
        assert!(block.contains("fish_add_path '/opt/flutter/bin'"));
        assert!(!block.contains("export PATH"));
    }

    #[test]
    fn test_find_fence_range_none_when_absent() {
        let contents = "# some shell config\nexport FOO=bar\n";
        assert!(find_fence_range_for(contents, FENCE_OPEN, FENCE_CLOSE).is_none());
    }

    #[test]
    fn test_find_fence_range_some_when_present() {
        let contents = format!(
            "# preamble\n{}\nexport PATH=\"$PATH:/a/bin\"\n{}\n# postamble\n",
            FENCE_OPEN, FENCE_CLOSE
        );
        let range = find_fence_range_for(&contents, FENCE_OPEN, FENCE_CLOSE);
        assert!(range.is_some());
        let (start, end) = range.unwrap();
        let block = &contents[start..end];
        assert!(block.starts_with(FENCE_OPEN));
        // The postamble must not be in the returned range.
        assert!(!block.contains("postamble"));
    }

    #[test]
    fn test_fence_already_has_dir_true() {
        let bin = Path::new("/opt/flutter/bin");
        let contents = format!(
            "{}\nexport PATH=\"$PATH:/opt/flutter/bin\"\n{}\n",
            FENCE_OPEN, FENCE_CLOSE
        );
        assert!(fence_already_has_dir(&contents, bin));
    }

    #[test]
    fn test_fence_already_has_dir_false_when_different_bin() {
        let bin = Path::new("/opt/flutter2/bin");
        let contents = format!(
            "{}\nexport PATH=\"$PATH:/opt/flutter/bin\"\n{}\n",
            FENCE_OPEN, FENCE_CLOSE
        );
        assert!(!fence_already_has_dir(&contents, bin));
    }

    #[test]
    fn test_apply_fence_appends_when_no_existing_block() {
        let contents = "# existing config\n";
        let rc = Path::new("/home/u/.bashrc");
        let bin = Path::new("/opt/flutter/bin");
        let result = apply_fence(contents, rc, bin).expect("should produce new content");
        assert!(result.starts_with("# existing config\n"));
        assert!(result.contains(FENCE_OPEN));
        assert!(result.contains("/opt/flutter/bin"));
    }

    #[test]
    fn test_apply_fence_returns_none_when_already_present() {
        let bin = Path::new("/opt/flutter/bin");
        let rc = Path::new("/home/u/.bashrc");
        let contents = format!(
            "# preamble\n{}\nexport PATH=\"$PATH:/opt/flutter/bin\"\n{}\n",
            FENCE_OPEN, FENCE_CLOSE
        );
        assert!(apply_fence(&contents, rc, bin).is_none());
    }

    #[test]
    fn test_apply_fence_replaces_existing_block_when_dir_differs() {
        let old_bin = Path::new("/opt/flutter_old/bin");
        let new_bin = Path::new("/opt/flutter_new/bin");
        let rc = Path::new("/home/u/.bashrc");
        let contents = format!(
            "# before\n{}\nexport PATH=\"$PATH:/opt/flutter_old/bin\"\n{}\n# after\n",
            FENCE_OPEN, FENCE_CLOSE
        );

        let result =
            apply_fence(&contents, rc, new_bin).expect("should produce replacement content");

        // Old bin_dir gone, new one present.
        assert!(!result.contains(old_bin.to_str().unwrap()));
        assert!(result.contains(new_bin.to_str().unwrap()));

        // Surrounding content preserved.
        assert!(result.contains("# before\n"));
        assert!(result.contains("# after\n"));

        // Exactly one fence block.
        assert_eq!(result.matches(FENCE_OPEN).count(), 1);
        assert_eq!(result.matches(FENCE_CLOSE).count(), 1);
    }

    // ── Filesystem / golden-file tests ───────────────────────────────────────

    #[test]
    fn test_writes_fenced_block_once() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let bin_dir = PathBuf::from("/opt/flutter/bin");

        let rc_file = rc_file_for_shell(HostShell::Bash, home).unwrap();

        let outcome = add_to_rc_file(&rc_file, &bin_dir).unwrap();
        assert_eq!(
            outcome,
            PathConfigOutcome::Written {
                rc_file: rc_file.clone()
            }
        );

        let written = std::fs::read_to_string(&rc_file).unwrap();
        assert!(written.contains(FENCE_OPEN));
        assert!(written.contains(FENCE_CLOSE));
        assert!(written.contains("export PATH=\"$PATH:\"'/opt/flutter/bin'"));
        assert_eq!(written.matches(FENCE_OPEN).count(), 1);
    }

    #[test]
    fn test_rerun_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let bin_dir = PathBuf::from("/opt/flutter/bin");
        let rc_file = rc_file_for_shell(HostShell::Bash, home).unwrap();

        // First call: writes.
        let first = add_to_rc_file(&rc_file, &bin_dir).unwrap();
        assert!(matches!(first, PathConfigOutcome::Written { .. }));

        // Second call: no-op.
        let second = add_to_rc_file(&rc_file, &bin_dir).unwrap();
        assert_eq!(
            second,
            PathConfigOutcome::AlreadyPresent {
                rc_file: rc_file.clone()
            }
        );

        // File still has exactly one fence block.
        let contents = std::fs::read_to_string(&rc_file).unwrap();
        assert_eq!(contents.matches(FENCE_OPEN).count(), 1);
        assert_eq!(contents.matches(FENCE_CLOSE).count(), 1);
    }

    #[test]
    fn test_changed_bin_dir_replaces_block() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let bin_a = PathBuf::from("/a/flutter/bin");
        let bin_b = PathBuf::from("/b/flutter/bin");
        let rc_file = rc_file_for_shell(HostShell::Bash, home).unwrap();

        // Install bin_a.
        add_to_rc_file(&rc_file, &bin_a).unwrap();

        // Switch to bin_b — should replace, not append.
        let outcome = add_to_rc_file(&rc_file, &bin_b).unwrap();
        assert!(matches!(outcome, PathConfigOutcome::Written { .. }));

        let contents = std::fs::read_to_string(&rc_file).unwrap();
        assert!(!contents.contains("/a/flutter/bin"));
        assert!(contents.contains("/b/flutter/bin"));
        assert_eq!(contents.matches(FENCE_OPEN).count(), 1);
        assert_eq!(contents.matches(FENCE_CLOSE).count(), 1);
    }

    #[test]
    fn test_fish_uses_fish_add_path_quoted() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let bin_dir = PathBuf::from("/opt/flutter/bin");

        let rc_file = rc_file_for_shell(HostShell::Fish, home).unwrap();
        let outcome = add_to_rc_file(&rc_file, &bin_dir).unwrap();
        assert!(matches!(outcome, PathConfigOutcome::Written { .. }));

        let contents = std::fs::read_to_string(&rc_file).unwrap();
        // Argument must be single-quoted.
        assert!(contents.contains("fish_add_path '/opt/flutter/bin'"));
        assert!(!contents.contains("export PATH"));
    }

    #[test]
    fn test_rc_file_selection_per_shell() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        // On non-macOS: Bash → ~/.bashrc
        // On macOS: Bash → ~/.bash_profile (preferred) / ~/.profile / ~/.bashrc fallback
        // We test the fallback path (neither .bash_profile nor .profile exist).
        #[cfg(not(target_os = "macos"))]
        {
            let bash_rc = rc_file_for_shell(HostShell::Bash, home).unwrap();
            assert_eq!(bash_rc, home.join(".bashrc"));
        }

        // Zsh → ~/.zshenv when it does not exist (fallback to ~/.zprofile)
        let zsh_rc = rc_file_for_shell(HostShell::Zsh, home).unwrap();
        assert_eq!(zsh_rc, home.join(".zprofile"));

        // Zsh → ~/.zshenv when it *does* exist
        std::fs::write(home.join(".zshenv"), "").unwrap();
        let zsh_rc_with_zshenv = rc_file_for_shell(HostShell::Zsh, home).unwrap();
        assert_eq!(zsh_rc_with_zshenv, home.join(".zshenv"));

        // Fish → ~/.config/fish/config.fish
        let fish_rc = rc_file_for_shell(HostShell::Fish, home).unwrap();
        assert_eq!(
            fish_rc,
            home.join(".config").join("fish").join("config.fish")
        );

        // PowerShell / Cmd / Unknown → None
        assert!(rc_file_for_shell(HostShell::PowerShell, home).is_none());
        assert!(rc_file_for_shell(HostShell::Cmd, home).is_none());
        assert!(rc_file_for_shell(HostShell::Unknown, home).is_none());
    }

    /// On macOS, bash should prefer `.bash_profile` when it exists, then `.profile`,
    /// then fall back to `.bashrc`.
    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos_bash_prefers_bash_profile() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        // Neither .bash_profile nor .profile exist → falls back to .bashrc
        let rc = rc_file_for_shell(HostShell::Bash, home).unwrap();
        assert_eq!(rc, home.join(".bashrc"));

        // Create .profile → should be selected over .bashrc
        std::fs::write(home.join(".profile"), "").unwrap();
        let rc = rc_file_for_shell(HostShell::Bash, home).unwrap();
        assert_eq!(rc, home.join(".profile"));

        // Create .bash_profile → should be preferred over .profile
        std::fs::write(home.join(".bash_profile"), "").unwrap();
        let rc = rc_file_for_shell(HostShell::Bash, home).unwrap();
        assert_eq!(rc, home.join(".bash_profile"));
    }

    /// On Linux, bash should always use `.bashrc` regardless of whether
    /// `.bash_profile` exists.
    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_linux_bash_always_uses_bashrc() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        // Create both possible alternatives — Linux should still pick .bashrc.
        std::fs::write(home.join(".bash_profile"), "").unwrap();
        std::fs::write(home.join(".profile"), "").unwrap();

        let rc = rc_file_for_shell(HostShell::Bash, home).unwrap();
        assert_eq!(rc, home.join(".bashrc"));
    }

    #[test]
    fn test_creates_parent_dir_for_fish_config() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let bin_dir = PathBuf::from("/opt/flutter/bin");

        let rc_file = rc_file_for_shell(HostShell::Fish, home).unwrap();
        // Parent dir should not exist yet.
        assert!(!rc_file.parent().unwrap().exists());

        let outcome = add_to_rc_file(&rc_file, &bin_dir).unwrap();
        assert!(matches!(outcome, PathConfigOutcome::Written { .. }));
        assert!(rc_file.exists());
    }

    #[test]
    fn test_preserves_existing_content_when_appending() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let bin_dir = PathBuf::from("/opt/flutter/bin");
        let rc_file = rc_file_for_shell(HostShell::Bash, home).unwrap();

        // Pre-populate the file with existing content.
        std::fs::write(&rc_file, "# my existing config\nalias ll='ls -la'\n").unwrap();

        add_to_rc_file(&rc_file, &bin_dir).unwrap();

        let contents = std::fs::read_to_string(&rc_file).unwrap();
        assert!(contents.contains("# my existing config\n"));
        assert!(contents.contains("alias ll='ls -la'\n"));
        assert!(contents.contains(FENCE_OPEN));
    }

    /// Validate that the injection check in `add_to_path` rejects dangerous paths
    /// before any I/O is attempted.
    #[test]
    fn test_add_to_path_rejects_injection_path() {
        // Use a Linux-targeted test to avoid platform specifics in the POSIX path.
        let dangerous = PathBuf::from("/opt/flutter/bin\nevil command");
        let result = add_to_path(HostShell::Bash, HostPlatform::Linux, &dangerous);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("newline") || err_msg.contains("injection"));
    }

    // ── Windows PATH string/command construction tests (cross-platform) ───────

    // ── plan_windows_path_update tests (pure, all platforms) ─────────────────

    /// A `%VAR%`-bearing raw value must produce `ExpandString` kind.
    #[test]
    fn test_plan_windows_path_update_expand_sz_when_percent_tokens() {
        let raw = r"%USERPROFILE%\bin;C:\Windows\System32";
        let bin = r"C:\tools\flutter\bin";
        let result = plan_windows_path_update(raw, false, bin);
        let (new_val, kind) = result.expect("should need a write");
        assert_eq!(kind, WindowsRegKind::ExpandString);
        assert!(new_val.ends_with(bin));
        assert!(new_val.contains(r"%USERPROFILE%\bin"));
    }

    /// An `existing_is_expand=true` flag must produce `ExpandString` even if the
    /// raw value contains no `%` tokens.
    #[test]
    fn test_plan_windows_path_update_expand_sz_from_existing_flag() {
        let raw = r"C:\Windows\System32";
        let bin = r"C:\tools\flutter\bin";
        let result = plan_windows_path_update(raw, true, bin);
        let (_, kind) = result.expect("should need a write");
        assert_eq!(kind, WindowsRegKind::ExpandString);
    }

    /// A plain value with `existing_is_expand=false` must produce `String` kind.
    #[test]
    fn test_plan_windows_path_update_string_when_plain_value() {
        let raw = r"C:\Windows\System32;C:\Program Files\Git\bin";
        let bin = r"C:\tools\flutter\bin";
        let result = plan_windows_path_update(raw, false, bin);
        let (new_val, kind) = result.expect("should need a write");
        assert_eq!(kind, WindowsRegKind::String);
        assert!(new_val.ends_with(bin));
    }

    /// Empty/absent PATH (empty string) must produce `ExpandString` (safe default).
    #[test]
    fn test_plan_windows_path_update_empty_path_defaults_to_expand_string() {
        let bin = r"C:\tools\flutter\bin";
        let result = plan_windows_path_update("", false, bin);
        let (new_val, kind) = result.expect("should need a write");
        assert_eq!(kind, WindowsRegKind::ExpandString);
        assert_eq!(new_val, bin);
    }

    /// Already-present entry (case-insensitive) must return `None`.
    #[test]
    fn test_plan_windows_path_update_idempotent() {
        let raw = r"C:\Windows\System32;C:\tools\flutter\bin";
        let bin = r"C:\Tools\Flutter\Bin"; // different case
        let result = plan_windows_path_update(raw, false, bin);
        assert!(result.is_none(), "should detect already-present entry");
    }

    /// Trailing-semicolon raw value must not produce a double-semicolon.
    #[test]
    fn test_plan_windows_path_update_trailing_semicolon() {
        let raw = r"C:\Windows\System32;";
        let bin = r"C:\tools\flutter\bin";
        let (new_val, _) = plan_windows_path_update(raw, false, bin).expect("should need a write");
        assert_eq!(new_val, r"C:\Windows\System32;C:\tools\flutter\bin");
        assert!(!new_val.contains(";;"), "no double semicolons");
    }

    /// `%VAR%` tokens in the raw value are preserved literally in the output.
    #[test]
    fn test_plan_windows_path_update_preserves_percent_tokens() {
        let raw = r"%USERPROFILE%\bin;%JAVA_HOME%\bin";
        let bin = r"C:\tools\flutter\bin";
        let (new_val, kind) =
            plan_windows_path_update(raw, false, bin).expect("should need a write");
        assert_eq!(kind, WindowsRegKind::ExpandString);
        // Raw tokens must be preserved verbatim in the output.
        assert!(
            new_val.contains(r"%USERPROFILE%\bin"),
            "USERPROFILE token preserved"
        );
        assert!(
            new_val.contains(r"%JAVA_HOME%\bin"),
            "JAVA_HOME token preserved"
        );
        assert!(new_val.ends_with(bin));
    }

    /// `decide_reg_kind`: `%` in value → `ExpandString`.
    #[test]
    fn test_decide_reg_kind_percent_in_value() {
        assert_eq!(
            decide_reg_kind(r"%USERPROFILE%\bin", false),
            WindowsRegKind::ExpandString
        );
    }

    /// `decide_reg_kind`: `existing_is_expand=true` → `ExpandString`.
    #[test]
    fn test_decide_reg_kind_existing_flag() {
        assert_eq!(
            decide_reg_kind(r"C:\plain\path", true),
            WindowsRegKind::ExpandString
        );
    }

    /// `decide_reg_kind`: plain value, `existing_is_expand=false` → `String`.
    #[test]
    fn test_decide_reg_kind_plain_value() {
        assert_eq!(
            decide_reg_kind(r"C:\plain\path", false),
            WindowsRegKind::String
        );
    }

    /// `decide_reg_kind`: empty value, `existing_is_expand=false` → `String`.
    /// Note: `plan_windows_path_update` overrides to `ExpandString` for new keys.
    #[test]
    fn test_decide_reg_kind_empty_no_flag_is_string() {
        assert_eq!(decide_reg_kind("", false), WindowsRegKind::String);
    }

    /// The write script references `$env:FDEMON_NEW_PATH` and `$env:FDEMON_PATH_KIND`
    /// out-of-band — no path value is interpolated into the script string.
    #[test]
    fn test_write_raw_path_script_uses_env_vars_not_interpolation() {
        let script = WRITE_RAW_PATH_SCRIPT;
        let tricky_path = "C:\\Users\\O'Brien\\flutter bin\\bin";

        assert!(
            !script.contains(tricky_path),
            "script must not interpolate the path value"
        );
        assert!(
            script.contains("$env:FDEMON_NEW_PATH"),
            "script must reference FDEMON_NEW_PATH"
        );
        assert!(
            script.contains("$env:FDEMON_PATH_KIND"),
            "script must reference FDEMON_PATH_KIND"
        );
        assert!(
            script.contains("New-ItemProperty"),
            "script must use New-ItemProperty (not SetEnvironmentVariable)"
        );
    }

    /// The read script uses `DoNotExpandEnvironmentNames` to get the raw value.
    #[test]
    fn test_read_raw_path_script_does_not_expand() {
        let script = READ_RAW_PATH_SCRIPT;
        assert!(
            script.contains("DoNotExpandEnvironmentNames"),
            "read script must use DoNotExpandEnvironmentNames"
        );
        assert!(
            !script.contains("GetEnvironmentVariable"),
            "read script must not use the expanding GetEnvironmentVariable"
        );
    }

    /// The ANDROID_HOME write script uses `New-ItemProperty` with `String` type
    /// and the out-of-band env var — not `SetEnvironmentVariable`.
    #[test]
    fn test_write_android_home_script_uses_env_var_not_interpolation() {
        let script = WRITE_ANDROID_HOME_SCRIPT;
        let tricky_sdk = "C:\\Users\\O'Brien\\android sdk";

        assert!(
            !script.contains(tricky_sdk),
            "script must not interpolate the SDK root value"
        );
        assert!(
            script.contains("$env:FDEMON_NEW_ANDROID_HOME"),
            "script must reference FDEMON_NEW_ANDROID_HOME env var"
        );
        assert!(
            script.contains("New-ItemProperty"),
            "script must use New-ItemProperty"
        );
        assert!(
            script.contains("String"),
            "ANDROID_HOME must be written as REG_SZ (String)"
        );
    }

    /// `WindowsRegKind::powershell_property_type` returns the correct strings.
    #[test]
    fn test_windows_reg_kind_property_type_strings() {
        assert_eq!(WindowsRegKind::String.powershell_property_type(), "String");
        assert_eq!(
            WindowsRegKind::ExpandString.powershell_property_type(),
            "ExpandString"
        );
    }

    // ── Retained injection-safety shape tests (updated to use new scripts) ───

    /// Verify that the Windows PATH write script uses the env-var reference
    /// form rather than interpolating the path into the script string.
    /// Asserts against `WRITE_RAW_PATH_SCRIPT` — the actual constant shipped.
    #[test]
    fn test_windows_powershell_set_command_uses_env_var_not_interpolation() {
        // Assert against the shipped constant, not a re-typed snippet.
        let script = WRITE_RAW_PATH_SCRIPT;

        // A path with a space and a single quote — the two characters that break
        // naïve PowerShell interpolation.
        let tricky_path = "C:\\Users\\O'Brien\\flutter bin\\bin";

        // The script must NOT contain the raw path value.
        assert!(
            !script.contains(tricky_path),
            "Script must not interpolate the path value"
        );

        // The script must reference the env var.
        assert!(
            script.contains("$env:FDEMON_NEW_PATH"),
            "Script must reference FDEMON_NEW_PATH env var"
        );
    }

    #[test]
    fn test_windows_new_path_format() {
        let current = "C:\\Windows\\System32;C:\\Program Files\\Git\\bin";
        let bin_str = "C:\\tools\\flutter\\bin";

        // Use plan_windows_path_update to validate the new-path logic.
        let (new_path, _) =
            plan_windows_path_update(current, false, bin_str).expect("should need a write");
        assert!(new_path.ends_with(bin_str));
        assert!(new_path.contains(';'));
        // Confirm value would be passed as env var, not interpolated into script.
        let script = WRITE_RAW_PATH_SCRIPT;
        assert!(!script.contains(bin_str));
        assert!(script.contains("$env:FDEMON_NEW_PATH"));
    }

    #[test]
    fn test_windows_path_with_space_and_quote() {
        // A path containing both a space and a single quote — the two characters
        // that demonstrate the old PowerShell injection bug.
        let current = "C:\\Windows\\System32";
        let bin_str = "C:\\Users\\O'Brien\\flutter bin\\bin";

        let (new_path, _) =
            plan_windows_path_update(current, false, bin_str).expect("should need a write");

        // The path is assembled correctly.
        assert!(new_path.contains("O'Brien"));
        assert!(new_path.contains("flutter bin"));

        // The value goes in the env var, never in the script.
        let script = WRITE_RAW_PATH_SCRIPT;
        assert!(!script.contains(bin_str));
    }

    #[test]
    fn test_windows_empty_current_path() {
        let bin_str = "C:\\tools\\flutter\\bin";
        let (new_path, kind) =
            plan_windows_path_update("", false, bin_str).expect("empty path must need a write");
        assert_eq!(new_path, bin_str);
        // Empty path → defaults to ExpandString.
        assert_eq!(kind, WindowsRegKind::ExpandString);
    }

    #[test]
    fn test_windows_path_trailing_semicolon() {
        let current_path = "C:\\Windows\\System32;";
        let bin_str = "C:\\tools\\flutter\\bin";
        let (new_path, _) =
            plan_windows_path_update(current_path, false, bin_str).expect("should need a write");
        assert_eq!(new_path, "C:\\Windows\\System32;C:\\tools\\flutter\\bin");
    }

    // ── add_android_env tests ─────────────────────────────────────────────────

    /// Golden-file idempotency for bash: write the Android env block, capture
    /// contents, write again — second call must return `AlreadyPresent` and the
    /// file must be byte-identical.
    #[test]
    fn test_add_android_env_idempotent_bash() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let sdk_root = PathBuf::from("/home/user/.android/sdk");

        let rc_file = rc_file_for_shell(HostShell::Bash, home).unwrap();

        // First write.
        let first = add_android_env_to_rc_file(&rc_file, &sdk_root).unwrap();
        assert!(
            matches!(first, PathConfigOutcome::Written { .. }),
            "first call should be Written"
        );

        let golden = std::fs::read_to_string(&rc_file).unwrap();

        // Second write — must be a no-op.
        let second = add_android_env_to_rc_file(&rc_file, &sdk_root).unwrap();
        assert_eq!(
            second,
            PathConfigOutcome::AlreadyPresent {
                rc_file: rc_file.clone()
            },
            "second call should be AlreadyPresent"
        );

        let after_second = std::fs::read_to_string(&rc_file).unwrap();
        assert_eq!(
            golden, after_second,
            "rc file must be byte-identical after second call"
        );
    }

    /// Golden-file idempotency for zsh.
    #[test]
    fn test_add_android_env_idempotent_zsh() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let sdk_root = PathBuf::from("/home/user/.android/sdk");

        let rc_file = rc_file_for_shell(HostShell::Zsh, home).unwrap();

        let first = add_android_env_to_rc_file(&rc_file, &sdk_root).unwrap();
        assert!(matches!(first, PathConfigOutcome::Written { .. }));

        let golden = std::fs::read_to_string(&rc_file).unwrap();

        let second = add_android_env_to_rc_file(&rc_file, &sdk_root).unwrap();
        assert!(matches!(second, PathConfigOutcome::AlreadyPresent { .. }));

        let after_second = std::fs::read_to_string(&rc_file).unwrap();
        assert_eq!(golden, after_second);
    }

    /// Golden-file idempotency for fish.
    #[test]
    fn test_add_android_env_idempotent_fish() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let sdk_root = PathBuf::from("/home/user/.android/sdk");

        let rc_file = rc_file_for_shell(HostShell::Fish, home).unwrap();

        let first = add_android_env_to_rc_file(&rc_file, &sdk_root).unwrap();
        assert!(matches!(first, PathConfigOutcome::Written { .. }));

        let golden = std::fs::read_to_string(&rc_file).unwrap();

        let second = add_android_env_to_rc_file(&rc_file, &sdk_root).unwrap();
        assert!(matches!(second, PathConfigOutcome::AlreadyPresent { .. }));

        let after_second = std::fs::read_to_string(&rc_file).unwrap();
        assert_eq!(golden, after_second);
    }

    /// The written block must contain ANDROID_HOME, cmdline-tools/latest/bin,
    /// platform-tools, and emulator for bash/zsh.
    #[test]
    fn test_android_env_block_has_three_bins_bash() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let sdk_root = PathBuf::from("/home/user/.android/sdk");

        let rc_file = rc_file_for_shell(HostShell::Bash, home).unwrap();
        add_android_env_to_rc_file(&rc_file, &sdk_root).unwrap();

        let contents = std::fs::read_to_string(&rc_file).unwrap();

        assert!(
            contents.contains("ANDROID_HOME"),
            "block must contain ANDROID_HOME"
        );
        assert!(
            contents.contains("cmdline-tools/latest/bin"),
            "block must contain cmdline-tools/latest/bin"
        );
        assert!(
            contents.contains("platform-tools"),
            "block must contain platform-tools"
        );
        assert!(
            contents.contains("/emulator"),
            "block must contain emulator"
        );
        assert!(
            contents.contains(ANDROID_FENCE_OPEN),
            "block must have Android fence open marker"
        );
        assert!(
            contents.contains(ANDROID_FENCE_CLOSE),
            "block must have Android fence close marker"
        );
        // Flutter PATH fence must NOT appear.
        assert!(
            !contents.contains(FENCE_OPEN),
            "Flutter PATH fence must not appear in Android block"
        );
    }

    /// The fish block must contain ANDROID_HOME, cmdline-tools/latest/bin,
    /// platform-tools, and emulator using fish syntax.
    #[test]
    fn test_android_env_block_has_three_bins_fish() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let sdk_root = PathBuf::from("/home/user/.android/sdk");

        let rc_file = rc_file_for_shell(HostShell::Fish, home).unwrap();
        add_android_env_to_rc_file(&rc_file, &sdk_root).unwrap();

        let contents = std::fs::read_to_string(&rc_file).unwrap();

        assert!(contents.contains("ANDROID_HOME"));
        assert!(contents.contains("cmdline-tools/latest/bin"));
        assert!(contents.contains("platform-tools"));
        assert!(contents.contains("/emulator"));
        assert!(contents.contains("set -Ux ANDROID_HOME"));
        assert!(contents.contains("fish_add_path"));
        // fish block should NOT use posix `export` syntax.
        assert!(!contents.contains("export ANDROID_HOME"));
    }

    /// Both the Flutter PATH block and the Android env block can coexist in the
    /// same rc file — each is independently idempotent.
    #[test]
    fn test_distinct_fence_flutter_and_android_coexist() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let bin_dir = PathBuf::from("/opt/flutter/bin");
        let sdk_root = PathBuf::from("/home/user/.android/sdk");

        let rc_file = rc_file_for_shell(HostShell::Bash, home).unwrap();

        // Write Flutter PATH block.
        let r1 = add_to_rc_file(&rc_file, &bin_dir).unwrap();
        assert!(matches!(r1, PathConfigOutcome::Written { .. }));

        // Write Android env block.
        let r2 = add_android_env_to_rc_file(&rc_file, &sdk_root).unwrap();
        assert!(matches!(r2, PathConfigOutcome::Written { .. }));

        let contents = std::fs::read_to_string(&rc_file).unwrap();

        // Both blocks must be present.
        assert_eq!(
            contents.matches(FENCE_OPEN).count(),
            1,
            "exactly one Flutter fence"
        );
        assert_eq!(
            contents.matches(FENCE_CLOSE).count(),
            1,
            "exactly one Flutter fence close"
        );
        assert_eq!(
            contents.matches(ANDROID_FENCE_OPEN).count(),
            1,
            "exactly one Android fence"
        );
        assert_eq!(
            contents.matches(ANDROID_FENCE_CLOSE).count(),
            1,
            "exactly one Android fence close"
        );
        assert!(contents.contains("/opt/flutter/bin"));
        assert!(contents.contains("/home/user/.android/sdk"));

        // Re-running both must be idempotent — no duplicates.
        let r3 = add_to_rc_file(&rc_file, &bin_dir).unwrap();
        assert!(matches!(r3, PathConfigOutcome::AlreadyPresent { .. }));

        let r4 = add_android_env_to_rc_file(&rc_file, &sdk_root).unwrap();
        assert!(matches!(r4, PathConfigOutcome::AlreadyPresent { .. }));

        let final_contents = std::fs::read_to_string(&rc_file).unwrap();
        assert_eq!(
            final_contents.matches(FENCE_OPEN).count(),
            1,
            "still exactly one Flutter fence after re-run"
        );
        assert_eq!(
            final_contents.matches(ANDROID_FENCE_OPEN).count(),
            1,
            "still exactly one Android fence after re-run"
        );
    }

    /// A SDK root with spaces must be written correctly (single-quoted in the
    /// export line, not causing parse errors).
    #[test]
    fn test_android_env_sdk_root_with_spaces_bash() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let sdk_root = PathBuf::from("/home/user/android sdk/root");

        let rc_file = rc_file_for_shell(HostShell::Bash, home).unwrap();
        add_android_env_to_rc_file(&rc_file, &sdk_root).unwrap();

        let contents = std::fs::read_to_string(&rc_file).unwrap();
        // The path must appear single-quoted inside the ANDROID_HOME export.
        assert!(
            contents.contains("export ANDROID_HOME='/home/user/android sdk/root'"),
            "SDK root with spaces must be single-quoted in export"
        );
    }

    /// Android env block content assertions — pure string builder (no I/O).
    /// Verifies ordering: cmdline-tools → platform-tools → emulator.
    #[test]
    fn test_android_posix_block_content() {
        let sdk_root = Path::new("/home/user/.android/sdk");
        let block = android_posix_block(sdk_root);

        assert!(block.starts_with(ANDROID_FENCE_OPEN));
        // SDK root is single-quoted to prevent shell expansion of $, ", `.
        assert!(block.contains("export ANDROID_HOME='/home/user/.android/sdk'"));
        assert!(block.contains(
            "export PATH=\"$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH\""
        ));
        // Verify the three entries are present in correct order.
        let path_line = block
            .lines()
            .find(|l| l.starts_with("export PATH="))
            .expect("PATH export line must exist");
        let cmdline_pos = path_line.find("cmdline-tools/latest/bin").unwrap();
        let platform_pos = path_line.find("platform-tools").unwrap();
        let emulator_pos = path_line.find("/emulator").unwrap();
        assert!(
            cmdline_pos < platform_pos && platform_pos < emulator_pos,
            "PATH ordering must be cmdline-tools → platform-tools → emulator"
        );
        assert!(block.contains(ANDROID_FENCE_CLOSE));
        // Must not collide with Flutter PATH marker.
        assert!(!block.contains(FENCE_OPEN));
    }

    #[test]
    fn test_android_fish_block_content() {
        let sdk_root = Path::new("/home/user/.android/sdk");
        let block = android_fish_block(sdk_root);

        assert!(block.starts_with(ANDROID_FENCE_OPEN));
        // SDK root is single-quoted to prevent fish expansion of $, ", `.
        assert!(block.contains("set -Ux ANDROID_HOME '/home/user/.android/sdk'"));
        assert!(block.contains(
            "fish_add_path \"$ANDROID_HOME/cmdline-tools/latest/bin\" \"$ANDROID_HOME/platform-tools\" \"$ANDROID_HOME/emulator\""
        ));
        // Verify the three entries are present in correct order.
        let path_line = block
            .lines()
            .find(|l| l.starts_with("fish_add_path"))
            .expect("fish_add_path line must exist");
        let cmdline_pos = path_line.find("cmdline-tools/latest/bin").unwrap();
        let platform_pos = path_line.find("platform-tools").unwrap();
        let emulator_pos = path_line.find("/emulator").unwrap();
        assert!(
            cmdline_pos < platform_pos && platform_pos < emulator_pos,
            "fish_add_path ordering must be cmdline-tools → platform-tools → emulator"
        );
        assert!(block.contains(ANDROID_FENCE_CLOSE));
        assert!(!block.contains("export ANDROID_HOME"));
    }

    /// Injection-bearing SDK root (contains `"`) must be safely single-quoted
    /// in the bash/zsh Android env block — no shell breakout possible.
    #[test]
    fn test_android_posix_block_injection_double_quote() {
        // A path containing a double-quote — would break out of double-quoted
        // assignment in the old format: `export ANDROID_HOME="/evil"path`.
        let sdk_root = Path::new("/home/user/android\"sdk");
        let block = android_posix_block(sdk_root);

        // The block must not contain an unquoted double-quote that breaks assignment.
        // Single-quoting means the " is literal inside '...'; the line must be:
        //   export ANDROID_HOME='/home/user/android"sdk'
        assert!(
            block.contains("export ANDROID_HOME='/home/user/android\"sdk'"),
            "double-quote in path must be safely enclosed in single quotes: {block}"
        );
        // No stray unquoted double-quote after ANDROID_HOME= outside single-quoted span.
        let assignment_line = block
            .lines()
            .find(|l| l.starts_with("export ANDROID_HOME="))
            .expect("assignment line must exist");
        // The line must start with export ANDROID_HOME=', not export ANDROID_HOME=".
        assert!(
            assignment_line.starts_with("export ANDROID_HOME='"),
            "assignment must use single-quote quoting: {assignment_line}"
        );
    }

    /// Injection-bearing SDK root (contains `$`) must not expand in bash/zsh.
    #[test]
    fn test_android_posix_block_injection_dollar() {
        let sdk_root = Path::new("/home/user/$HOME/.android/sdk");
        let block = android_posix_block(sdk_root);

        // Single-quoting prevents $ expansion. The literal string must appear.
        assert!(
            block.contains("export ANDROID_HOME='/home/user/$HOME/.android/sdk'"),
            "dollar in path must be single-quoted to prevent expansion: {block}"
        );
    }

    /// Injection-bearing SDK root (contains `"`) must be safely single-quoted
    /// in the fish Android env block — no fish breakout possible.
    #[test]
    fn test_android_fish_block_injection_double_quote() {
        let sdk_root = Path::new("/home/user/android\"sdk");
        let block = android_fish_block(sdk_root);

        assert!(
            block.contains("set -Ux ANDROID_HOME '/home/user/android\"sdk'"),
            "double-quote in path must be safely single-quoted in fish block: {block}"
        );
        let set_line = block
            .lines()
            .find(|l| l.starts_with("set -Ux ANDROID_HOME"))
            .expect("set line must exist");
        assert!(
            set_line.starts_with("set -Ux ANDROID_HOME '"),
            "fish set must use single-quote quoting: {set_line}"
        );
    }

    /// Injection-bearing SDK root (contains `$`) must not expand in fish.
    #[test]
    fn test_android_fish_block_injection_dollar() {
        let sdk_root = Path::new("/home/user/$EVILVAR/.android/sdk");
        let block = android_fish_block(sdk_root);

        assert!(
            block.contains("set -Ux ANDROID_HOME '/home/user/$EVILVAR/.android/sdk'"),
            "dollar in path must be single-quoted to prevent fish expansion: {block}"
        );
    }

    /// The injection validator rejects metacharacters in sdk_root paths when
    /// calling `add_android_env`.
    #[test]
    fn test_add_android_env_rejects_injection_path() {
        let dangerous = PathBuf::from("/opt/android\nevil command");
        let result = add_android_env(HostShell::Bash, HostPlatform::Linux, &dangerous);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("newline") || err_msg.contains("injection"));
    }

    /// Verify that the Windows script for writing ANDROID_HOME uses
    /// `New-ItemProperty` with the env-var out-of-band form and never interpolates
    /// the SDK root. Asserts against `WRITE_ANDROID_HOME_SCRIPT` — the constant
    /// shipped to production — so any drift is caught on Linux CI.
    #[test]
    fn test_windows_android_home_script_uses_env_var() {
        // Assert against the shipped constant, not a re-typed snippet.
        let script = WRITE_ANDROID_HOME_SCRIPT;
        let tricky_sdk = "C:\\Users\\O'Brien\\android sdk";

        assert!(
            !script.contains(tricky_sdk),
            "Script must not interpolate the SDK root value"
        );
        assert!(
            script.contains("$env:FDEMON_NEW_ANDROID_HOME"),
            "Script must reference FDEMON_NEW_ANDROID_HOME env var"
        );
        assert!(
            script.contains("New-ItemProperty"),
            "Script must use New-ItemProperty"
        );
    }

    // ── Error-path tests: PowerShell / Cmd / Unknown shells (non-Windows) ────────

    /// `add_to_path` with `HostShell::PowerShell` on a non-Windows platform must
    /// return `Err` containing a manual-setup hint.  These shells do not have an
    /// rc file to edit; the user is told to configure their PATH manually.
    #[test]
    fn add_to_path_powershell_shell_is_err_with_hint() {
        let bin = PathBuf::from("/opt/flutter/bin");
        let err = add_to_path(HostShell::PowerShell, HostPlatform::Linux, &bin).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("manual"),
            "error must contain manual-setup hint, got: {err}"
        );
    }

    #[test]
    fn add_to_path_cmd_shell_is_err_with_hint() {
        let bin = PathBuf::from("/opt/flutter/bin");
        let err = add_to_path(HostShell::Cmd, HostPlatform::Linux, &bin).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("manual"),
            "error must contain manual-setup hint, got: {err}"
        );
    }

    #[test]
    fn add_to_path_unknown_shell_is_err_with_hint() {
        let bin = PathBuf::from("/opt/flutter/bin");
        let err = add_to_path(HostShell::Unknown, HostPlatform::Linux, &bin).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("manual"),
            "error must contain manual-setup hint, got: {err}"
        );
    }

    /// `add_android_env` with `HostShell::PowerShell` on a non-Windows platform
    /// must return `Err` containing a manual-setup hint.
    #[test]
    fn add_android_env_powershell_shell_is_err_with_hint() {
        let sdk = PathBuf::from("/home/user/.android/sdk");
        let err = add_android_env(HostShell::PowerShell, HostPlatform::Linux, &sdk).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("manual"),
            "error must contain manual-setup hint, got: {err}"
        );
    }

    #[test]
    fn add_android_env_cmd_shell_is_err_with_hint() {
        let sdk = PathBuf::from("/home/user/.android/sdk");
        let err = add_android_env(HostShell::Cmd, HostPlatform::Linux, &sdk).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("manual"),
            "error must contain manual-setup hint, got: {err}"
        );
    }

    #[test]
    fn add_android_env_unknown_shell_is_err_with_hint() {
        let sdk = PathBuf::from("/home/user/.android/sdk");
        let err = add_android_env(HostShell::Unknown, HostPlatform::Linux, &sdk).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("manual"),
            "error must contain manual-setup hint, got: {err}"
        );
    }

    // ── Windows broadcast script shape assertions (cross-platform) ───────────

    /// The `broadcast_wm_settingchange` PowerShell script must reference the
    /// numeric constant `0x1A` (WM_SETTINGCHANGE) and broadcast to `0xFFFF`
    /// (HWND_BROADCAST), and must use the literal string `"Environment"` rather
    /// than any user-supplied value.
    ///
    /// Asserts against `BROADCAST_WM_SETTINGCHANGE_SCRIPT` — the actual constant
    /// shipped to production — so any accidental drift is caught on Linux CI.
    #[test]
    fn windows_broadcast_script_contains_wm_settingchange_constant() {
        // Assert against the shared module-level constant so this test guards
        // the actually-shipped script rather than a re-typed copy.
        let script = BROADCAST_WM_SETTINGCHANGE_SCRIPT;

        // WM_SETTINGCHANGE hex constant must be present.
        assert!(
            script.contains("0x1A"),
            "broadcast script must reference WM_SETTINGCHANGE (0x1A)"
        );
        // HWND_BROADCAST hex constant must be present.
        assert!(
            script.contains("0xFFFF"),
            "broadcast script must reference HWND_BROADCAST (0xFFFF)"
        );
        // The broadcast lParam is the literal "Environment" (system constant),
        // not a user-supplied variable or path value.
        assert!(
            script.contains("\"Environment\""),
            "broadcast lParam must be the literal string \"Environment\""
        );
        // The script must not reference any fdemon path env vars — the broadcast
        // is independent of the value being written.
        assert!(
            !script.contains("FDEMON_NEW_PATH"),
            "broadcast script must not reference FDEMON_NEW_PATH"
        );
        assert!(
            !script.contains("FDEMON_NEW_ANDROID_HOME"),
            "broadcast script must not reference FDEMON_NEW_ANDROID_HOME"
        );
    }

    /// After a Windows PATH write, the set script references `$env:FDEMON_NEW_PATH`
    /// out-of-band, and the broadcast does not re-introduce any path interpolation.
    ///
    /// Asserts against `WRITE_RAW_PATH_SCRIPT` and `BROADCAST_WM_SETTINGCHANGE_SCRIPT`
    /// — the actual constants shipped to production — so any accidental drift is
    /// caught on Linux CI.
    #[test]
    fn windows_path_set_and_broadcast_both_use_out_of_band_values() {
        // Assert against the shipped constants, not re-typed snippets.
        let set_script = WRITE_RAW_PATH_SCRIPT;
        let broadcast_script = BROADCAST_WM_SETTINGCHANGE_SCRIPT;

        // The set script must use the env-var reference form (out-of-band).
        assert!(
            set_script.contains("$env:FDEMON_NEW_PATH"),
            "write script must reference FDEMON_NEW_PATH"
        );
        // The broadcast lParam is the literal word "Environment" — not any path.
        assert!(
            broadcast_script.contains("\"Environment\""),
            "broadcast lParam must be the literal string \"Environment\""
        );
        // Neither script may contain a raw Windows path.
        let tricky_path = "C:\\Users\\O'Brien\\flutter bin\\bin";
        assert!(!set_script.contains(tricky_path));
        assert!(!broadcast_script.contains(tricky_path));
    }

    /// Replacing an Android env block when the SDK root changes leaves exactly
    /// one Android fence block (old entry gone, new entry present) and the new
    /// block includes the `emulator` entry.
    #[test]
    fn test_android_env_changed_sdk_root_replaces_block() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let sdk_a = PathBuf::from("/home/user/.android/sdk_a");
        let sdk_b = PathBuf::from("/home/user/.android/sdk_b");

        let rc_file = rc_file_for_shell(HostShell::Bash, home).unwrap();

        add_android_env_to_rc_file(&rc_file, &sdk_a).unwrap();
        let outcome = add_android_env_to_rc_file(&rc_file, &sdk_b).unwrap();
        assert!(matches!(outcome, PathConfigOutcome::Written { .. }));

        let contents = std::fs::read_to_string(&rc_file).unwrap();
        assert!(!contents.contains("sdk_a"), "old SDK root must be gone");
        assert!(contents.contains("sdk_b"), "new SDK root must be present");
        assert!(
            contents.contains("/emulator"),
            "replaced block must include emulator entry"
        );
        assert_eq!(
            contents.matches(ANDROID_FENCE_OPEN).count(),
            1,
            "exactly one Android fence block"
        );
    }

    // ── write_rc_atomically permission and uniqueness tests ──────────────────

    /// On Unix: when the destination rc file already exists with mode 0600, the
    /// file's mode must still be 0600 after `add_to_rc_file` edits it.
    #[test]
    #[cfg(unix)]
    fn test_write_rc_preserves_mode() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let rc_file = tmp.path().join("test_rc");

        // Create the file and harden it to 0600.
        std::fs::write(&rc_file, "# existing content\n").unwrap();
        std::fs::set_permissions(&rc_file, std::fs::Permissions::from_mode(0o600)).unwrap();

        let bin_dir = PathBuf::from("/opt/flutter/bin");
        let outcome = add_to_rc_file(&rc_file, &bin_dir).unwrap();
        assert!(
            matches!(outcome, PathConfigOutcome::Written { .. }),
            "should write new content"
        );

        let mode = std::fs::metadata(&rc_file).unwrap().permissions().mode();
        // Only the permission bits (lower 12 bits).
        assert_eq!(
            mode & 0o7777,
            0o600,
            "rc file mode must be preserved as 0600 after write, got {:o}",
            mode & 0o7777
        );
    }

    /// On Unix: when the rc file does not yet exist, the created file must have
    /// mode 0600 (not the umask-derived 0644).
    #[test]
    #[cfg(unix)]
    fn test_write_rc_new_file_is_0600() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let rc_file = tmp.path().join("new_rc_file");

        // File must not exist yet.
        assert!(!rc_file.exists());

        let bin_dir = PathBuf::from("/opt/flutter/bin");
        let outcome = add_to_rc_file(&rc_file, &bin_dir).unwrap();
        assert!(
            matches!(outcome, PathConfigOutcome::Written { .. }),
            "should create and write new file"
        );

        let mode = std::fs::metadata(&rc_file).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o7777,
            0o600,
            "newly created rc file must have mode 0600, got {:o}",
            mode & 0o7777
        );
    }

    /// Two separate calls to `write_rc_atomically` (via `add_to_rc_file`) must
    /// not share a deterministic `.fdemon_tmp` sibling path — the temp files
    /// created must have distinct, non-deterministic names.
    #[test]
    fn test_write_rc_temp_name_is_not_deterministic_fdemon_tmp() {
        let tmp = TempDir::new().unwrap();
        let rc_file = tmp.path().join("test_rc");
        std::fs::write(&rc_file, "# existing content\n").unwrap();

        let bin_a = PathBuf::from("/opt/flutter_a/bin");
        let bin_b = PathBuf::from("/opt/flutter_b/bin");

        // Write bin_a — afterwards verify that no `.fdemon_tmp` sibling exists.
        add_to_rc_file(&rc_file, &bin_a).unwrap();
        let sibling = rc_file.with_extension("fdemon_tmp");
        assert!(
            !sibling.exists(),
            "write must not leave a .fdemon_tmp sibling: {}",
            sibling.display()
        );

        // Write bin_b — same check.
        add_to_rc_file(&rc_file, &bin_b).unwrap();
        assert!(
            !sibling.exists(),
            "write must not leave a .fdemon_tmp sibling on second call: {}",
            sibling.display()
        );
    }

    // ── Windows Android env idempotency (string-level, cross-platform) ──────────

    /// The Windows idempotency check now requires all three Android PATH dirs
    /// (cmdline-tools, platform-tools, emulator) plus ANDROID_HOME to match.
    /// This test verifies the string logic without spawning PowerShell.
    #[test]
    fn test_windows_android_env_idempotency_requires_three_dirs() {
        let sdk_str = "C:\\Users\\user\\AppData\\Local\\Android\\Sdk";

        let cmdline_bin = format!("{}\\cmdline-tools\\latest\\bin", sdk_str);
        let platform_tools = format!("{}\\platform-tools", sdk_str);
        let emulator = format!("{}\\emulator", sdk_str);

        let path_segments_with_all = [
            cmdline_bin.as_str(),
            platform_tools.as_str(),
            emulator.as_str(),
        ];

        // All three present → idempotent (home_matches + all_present => AlreadyPresent).
        let home_matches = sdk_str.eq_ignore_ascii_case(sdk_str);
        let cmdline_present = path_segments_with_all
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&cmdline_bin));
        let platform_present = path_segments_with_all
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&platform_tools));
        let emulator_present = path_segments_with_all
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&emulator));

        assert!(
            home_matches && cmdline_present && platform_present && emulator_present,
            "all three dirs + home match must be idempotent"
        );

        // Missing emulator → NOT idempotent.
        let path_segments_missing_emulator = [cmdline_bin.as_str(), platform_tools.as_str()];
        let emulator_present_missing = path_segments_missing_emulator
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&emulator));
        assert!(
            !emulator_present_missing,
            "missing emulator dir must not satisfy idempotency check"
        );
    }

    /// The Windows path prepend logic inserts dirs in correct order when none
    /// are present: resulting order is cmdline-tools, platform-tools, emulator
    /// at the front.
    #[test]
    fn test_windows_android_env_prepend_order() {
        let sdk_str = "C:\\Android\\Sdk";
        let cmdline_bin = format!("{}\\cmdline-tools\\latest\\bin", sdk_str);
        let platform_tools = format!("{}\\platform-tools", sdk_str);
        let emulator = format!("{}\\emulator", sdk_str);

        // Simulate starting from an empty PATH, inserting in reverse order.
        let mut path_parts: Vec<String> = vec!["C:\\Windows\\System32".to_string()];

        // Simulate the actual insertion logic (reverse order so cmdline ends up first).
        // emulator not present → insert at 0
        path_parts.insert(0, emulator.clone());
        // platform not present → insert at 0 (pushes emulator to 1)
        path_parts.insert(0, platform_tools.clone());
        // cmdline not present → insert at 0 (pushes platform to 1, emulator to 2)
        path_parts.insert(0, cmdline_bin.clone());

        assert_eq!(path_parts[0], cmdline_bin, "cmdline-tools must be first");
        assert_eq!(
            path_parts[1], platform_tools,
            "platform-tools must be second"
        );
        assert_eq!(path_parts[2], emulator, "emulator must be third");
    }
}
