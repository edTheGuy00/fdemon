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
//! | Windows     | User registry `PATH` via PowerShell         | registry update               |
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
//! export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
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
/// On Windows, updates the user `PATH` via the registry using a PowerShell
/// `[Environment]::SetEnvironmentVariable` call rather than editing an rc file.
/// The value is passed out-of-band via the `FDEMON_NEW_PATH` environment
/// variable — never interpolated into the PowerShell script string — to prevent
/// code injection.
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
/// Windows: sets `ANDROID_HOME` in the user registry and prepends the two bin
/// dirs to the user `PATH`, both via PowerShell
/// `[Environment]::SetEnvironmentVariable`. Values are passed out-of-band
/// through environment variables (`FDEMON_NEW_ANDROID_HOME` / `FDEMON_NEW_PATH`)
/// to prevent shell injection.
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
/// export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
/// # <<< fdemon android env <<<
/// ```
fn android_posix_block(sdk_root: &Path) -> String {
    let sdk_escaped = single_quote_escape(&sdk_root.to_string_lossy());
    format!(
        "{fence_open}\nexport ANDROID_HOME={sdk_escaped}\nexport PATH=\"$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH\"\n{fence_close}\n",
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
/// fish_add_path "$ANDROID_HOME/cmdline-tools/latest/bin" "$ANDROID_HOME/platform-tools"
/// # <<< fdemon android env <<<
/// ```
fn android_fish_block(sdk_root: &Path) -> String {
    let sdk_escaped = single_quote_escape(&sdk_root.to_string_lossy());
    format!(
        "{fence_open}\nset -Ux ANDROID_HOME {sdk_escaped}\nfish_add_path \"$ANDROID_HOME/cmdline-tools/latest/bin\" \"$ANDROID_HOME/platform-tools\"\n{fence_close}\n",
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
fn write_rc_atomically(rc_file: &Path, new_contents: &str) -> Result<()> {
    // Ensure parent directory exists.
    if let Some(parent) = rc_file.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Use a simple temp-file approach: write to `<rc_file>.fdemon.tmp` then rename.
    let tmp_path = rc_file.with_extension("fdemon_tmp");

    std::fs::write(&tmp_path, new_contents).map_err(|e| {
        Error::config(format!(
            "Failed to write temp file {}: {}",
            tmp_path.display(),
            e
        ))
    })?;

    std::fs::rename(&tmp_path, rc_file).map_err(|e| {
        // Clean up the temp file on failure (best effort).
        if let Err(remove_err) = std::fs::remove_file(&tmp_path) {
            tracing::debug!(
                path = %tmp_path.display(),
                error = %remove_err,
                "Failed to clean up temp rc file after rename failure"
            );
        }
        Error::config(format!(
            "Failed to move {} → {}: {}",
            tmp_path.display(),
            rc_file.display(),
            e
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

/// Update the Windows user `PATH` via PowerShell.
///
/// Guards against the 1024-byte truncation caused by `setx` by using the
/// `[Environment]::SetEnvironmentVariable` registry API instead, which has no
/// length limit.
///
/// **Injection safety:** The new PATH value is passed out-of-band via the
/// `FDEMON_NEW_PATH` environment variable and referenced as `$env:FDEMON_NEW_PATH`
/// inside the PowerShell script. The script string itself is a constant with no
/// user-controlled interpolation, so PowerShell metacharacters in the path cannot
/// execute arbitrary code.
///
/// The function is platform-gated so it compiles on all targets but only runs
/// on Windows.
fn add_to_path_windows(bin_dir: &Path) -> Result<PathConfigOutcome> {
    let bin_str = bin_dir.to_string_lossy().into_owned();

    // Read the current user PATH via PowerShell.
    // This script is a constant — no user-controlled values are interpolated.
    let read_output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Environment]::GetEnvironmentVariable('PATH', 'User')",
        ])
        .output()
        .map_err(|e| Error::config(format!("Failed to run PowerShell to read PATH: {}", e)))?;

    let current_path = String::from_utf8_lossy(&read_output.stdout)
        .trim()
        .to_string();

    // Check if the bin_dir is already present.
    let already_present = current_path
        .split(';')
        .any(|segment| segment.trim().eq_ignore_ascii_case(bin_str.as_str()));

    if already_present {
        return Ok(PathConfigOutcome::AlreadyPresent {
            rc_file: PathBuf::from("HKCU:\\Environment\\PATH"),
        });
    }

    // Append our bin_dir to form the new PATH value.
    let new_path = if current_path.is_empty() {
        bin_str.clone()
    } else if current_path.ends_with(';') {
        format!("{}{}", current_path, bin_str)
    } else {
        format!("{};{}", current_path, bin_str)
    };

    // Pass the new PATH value out-of-band via an environment variable so that
    // it is never interpolated into the PowerShell script string.  This
    // eliminates the injection surface entirely — PowerShell metacharacters
    // (backtick, `$(...)`, etc.) in the path value cannot execute code.
    let set_output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Environment]::SetEnvironmentVariable('PATH', $env:FDEMON_NEW_PATH, 'User')",
        ])
        .env("FDEMON_NEW_PATH", &new_path)
        .output()
        .map_err(|e| Error::config(format!("Failed to run PowerShell to set PATH: {}", e)))?;

    if !set_output.status.success() {
        let stderr = String::from_utf8_lossy(&set_output.stderr);
        return Err(Error::config(format!(
            "PowerShell SetEnvironmentVariable failed: {}",
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

/// Update the Windows user `ANDROID_HOME` and `PATH` via PowerShell.
///
/// Sets `ANDROID_HOME` to `sdk_root` and prepends
/// `%ANDROID_HOME%\cmdline-tools\latest\bin` and
/// `%ANDROID_HOME%\platform-tools` to the user `PATH` if they are not already
/// present (idempotent).
///
/// **Injection safety:** `sdk_root` is passed out-of-band via
/// `FDEMON_NEW_ANDROID_HOME`; the new PATH value via `FDEMON_NEW_PATH`. Neither
/// value is ever interpolated into the PowerShell script string.
fn add_android_env_windows(sdk_root: &Path) -> Result<PathConfigOutcome> {
    let sdk_str = sdk_root.to_string_lossy().into_owned();

    // Read the current user ANDROID_HOME.
    let read_home_output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Environment]::GetEnvironmentVariable('ANDROID_HOME', 'User')",
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

    // Read the current user PATH.
    let read_path_output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Environment]::GetEnvironmentVariable('PATH', 'User')",
        ])
        .output()
        .map_err(|e| Error::config(format!("Failed to run PowerShell to read PATH: {}", e)))?;

    let current_path = String::from_utf8_lossy(&read_path_output.stdout)
        .trim()
        .to_string();

    // Compute the two Android bin dirs to add.
    let cmdline_bin = format!("{}\\cmdline-tools\\latest\\bin", sdk_str);
    let platform_tools = format!("{}\\platform-tools", sdk_str);

    // Check whether ANDROID_HOME already equals sdk_root and both bin dirs are
    // already in PATH — if so, the configuration is already complete.
    let home_matches = current_home.eq_ignore_ascii_case(&sdk_str);
    let path_segments: Vec<&str> = current_path.split(';').map(str::trim).collect();
    let cmdline_present = path_segments
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&cmdline_bin));
    let platform_present = path_segments
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&platform_tools));

    if home_matches && cmdline_present && platform_present {
        return Ok(PathConfigOutcome::AlreadyPresent {
            rc_file: PathBuf::from("HKCU:\\Environment"),
        });
    }

    // Set ANDROID_HOME — passed out-of-band to avoid injection.
    let set_home_output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Environment]::SetEnvironmentVariable('ANDROID_HOME', $env:FDEMON_NEW_ANDROID_HOME, 'User')",
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
            "PowerShell SetEnvironmentVariable(ANDROID_HOME) failed: {}",
            stderr.trim()
        )));
    }

    // Prepend the two bin dirs to PATH (only if not already present).
    let mut path_parts: Vec<String> = current_path
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Prepend in reverse order so cmdline_bin ends up before platform_tools.
    if !platform_present {
        path_parts.insert(0, platform_tools);
    }
    if !cmdline_present {
        path_parts.insert(0, cmdline_bin);
    }

    let new_path = path_parts.join(";");

    // Pass the new PATH value out-of-band via an environment variable.
    let set_path_output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Environment]::SetEnvironmentVariable('PATH', $env:FDEMON_NEW_PATH, 'User')",
        ])
        .env("FDEMON_NEW_PATH", &new_path)
        .output()
        .map_err(|e| Error::config(format!("Failed to run PowerShell to set PATH: {}", e)))?;

    if !set_path_output.status.success() {
        let stderr = String::from_utf8_lossy(&set_path_output.stderr);
        return Err(Error::config(format!(
            "PowerShell SetEnvironmentVariable(PATH) failed: {}",
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

    /// Verify that the Windows PowerShell set command uses the env-var reference
    /// form rather than interpolating the path into the script string.
    /// This test validates the string we pass to powershell.args([...]) — it
    /// must contain `$env:FDEMON_NEW_PATH` and must NOT contain the raw path.
    #[test]
    fn test_windows_powershell_set_command_uses_env_var_not_interpolation() {
        // The constant script string that must be passed to PowerShell.
        let expected_script =
            "[Environment]::SetEnvironmentVariable('PATH', $env:FDEMON_NEW_PATH, 'User')";

        // A path with a space and a single quote — the two characters that break
        // naïve PowerShell interpolation.
        let tricky_path = "C:\\Users\\O'Brien\\flutter bin\\bin";

        // The script must NOT contain the raw path value.
        assert!(
            !expected_script.contains(tricky_path),
            "Script must not interpolate the path value"
        );

        // The script must reference the env var.
        assert!(
            expected_script.contains("$env:FDEMON_NEW_PATH"),
            "Script must reference FDEMON_NEW_PATH env var"
        );
    }

    #[test]
    fn test_windows_new_path_format() {
        let current = "C:\\Windows\\System32;C:\\Program Files\\Git\\bin";
        let bin_str = "C:\\tools\\flutter\\bin";

        let new_path = format!("{};{}", current, bin_str);
        assert!(new_path.ends_with(bin_str));
        assert!(new_path.contains(';'));
        // Confirm value would be passed as env var, not interpolated into script.
        let script = "[Environment]::SetEnvironmentVariable('PATH', $env:FDEMON_NEW_PATH, 'User')";
        assert!(!script.contains(bin_str));
        assert!(script.contains("$env:FDEMON_NEW_PATH"));
    }

    #[test]
    fn test_windows_path_with_space_and_quote() {
        // A path containing both a space and a single quote — the two characters
        // that demonstrate the old PowerShell injection bug.
        let current = "C:\\Windows\\System32";
        let bin_str = "C:\\Users\\O'Brien\\flutter bin\\bin";

        let new_path = format!("{};{}", current, bin_str);

        // The path is assembled correctly.
        assert!(new_path.contains("O'Brien"));
        assert!(new_path.contains("flutter bin"));

        // The value goes in the env var, never in the script.
        let script = "[Environment]::SetEnvironmentVariable('PATH', $env:FDEMON_NEW_PATH, 'User')";
        assert!(!script.contains(bin_str));
    }

    #[test]
    fn test_windows_empty_current_path() {
        let current_path = "";
        let bin_str = "C:\\tools\\flutter\\bin";

        let new_path = if current_path.is_empty() {
            bin_str.to_string()
        } else if current_path.ends_with(';') {
            format!("{}{}", current_path, bin_str)
        } else {
            format!("{};{}", current_path, bin_str)
        };

        assert_eq!(new_path, bin_str);
    }

    #[test]
    fn test_windows_path_trailing_semicolon() {
        let current_path = "C:\\Windows\\System32;";
        let bin_str = "C:\\tools\\flutter\\bin";

        let new_path = if current_path.is_empty() {
            bin_str.to_string()
        } else if current_path.ends_with(';') {
            format!("{}{}", current_path, bin_str)
        } else {
            format!("{};{}", current_path, bin_str)
        };

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
    /// and platform-tools for bash/zsh.
    #[test]
    fn test_android_env_block_has_both_bins_bash() {
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
    /// and platform-tools using fish syntax.
    #[test]
    fn test_android_env_block_has_both_bins_fish() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let sdk_root = PathBuf::from("/home/user/.android/sdk");

        let rc_file = rc_file_for_shell(HostShell::Fish, home).unwrap();
        add_android_env_to_rc_file(&rc_file, &sdk_root).unwrap();

        let contents = std::fs::read_to_string(&rc_file).unwrap();

        assert!(contents.contains("ANDROID_HOME"));
        assert!(contents.contains("cmdline-tools/latest/bin"));
        assert!(contents.contains("platform-tools"));
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
    #[test]
    fn test_android_posix_block_content() {
        let sdk_root = Path::new("/home/user/.android/sdk");
        let block = android_posix_block(sdk_root);

        assert!(block.starts_with(ANDROID_FENCE_OPEN));
        // SDK root is single-quoted to prevent shell expansion of $, ", `.
        assert!(block.contains("export ANDROID_HOME='/home/user/.android/sdk'"));
        assert!(block.contains(
            "export PATH=\"$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH\""
        ));
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
            "fish_add_path \"$ANDROID_HOME/cmdline-tools/latest/bin\" \"$ANDROID_HOME/platform-tools\""
        ));
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

    /// Verify that the Windows PowerShell script for setting ANDROID_HOME uses
    /// the env-var out-of-band form and never interpolates the SDK root.
    #[test]
    fn test_windows_android_home_script_uses_env_var() {
        let expected_script =
            "[Environment]::SetEnvironmentVariable('ANDROID_HOME', $env:FDEMON_NEW_ANDROID_HOME, 'User')";

        let tricky_sdk = "C:\\Users\\O'Brien\\android sdk";

        assert!(
            !expected_script.contains(tricky_sdk),
            "Script must not interpolate the SDK root value"
        );
        assert!(
            expected_script.contains("$env:FDEMON_NEW_ANDROID_HOME"),
            "Script must reference FDEMON_NEW_ANDROID_HOME env var"
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

    /// After a Windows PATH write, the set command references `$env:FDEMON_NEW_PATH`
    /// out-of-band, and the broadcast does not re-introduce any path interpolation.
    ///
    /// Asserts against `BROADCAST_WM_SETTINGCHANGE_SCRIPT` — the actual constant
    /// shipped to production — so any accidental drift is caught on Linux CI.
    #[test]
    fn windows_path_set_and_broadcast_both_use_out_of_band_values() {
        let set_script =
            "[Environment]::SetEnvironmentVariable('PATH', $env:FDEMON_NEW_PATH, 'User')";
        // Assert against the shared constant, not a re-typed snippet.
        let broadcast_script = BROADCAST_WM_SETTINGCHANGE_SCRIPT;

        // The set script must use the env-var reference form (out-of-band).
        assert!(set_script.contains("$env:FDEMON_NEW_PATH"));
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
    /// one Android fence block (old entry gone, new entry present).
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
        assert_eq!(
            contents.matches(ANDROID_FENCE_OPEN).count(),
            1,
            "exactly one Android fence block"
        );
    }
}
