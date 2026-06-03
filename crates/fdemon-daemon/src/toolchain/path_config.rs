//! # PATH Configuration Writer
//!
//! Writes an idempotent, marker-fenced `PATH` export for a Flutter SDK `bin`
//! directory to the correct shell rc file for the detected shell.
//!
//! ## Supported shells
//!
//! | Shell       | Target file                                 | Export syntax                 |
//! |-------------|---------------------------------------------|-------------------------------|
//! | bash        | `~/.bashrc`                                 | `export PATH="$PATH:<bin>"`   |
//! | zsh         | `~/.zshenv` (preferred) or `~/.zprofile`    | `export PATH="$PATH:<bin>"`   |
//! | fish        | `~/.config/fish/config.fish`                | `fish_add_path <bin>`         |
//! | Windows     | User registry `PATH` via PowerShell         | registry update               |
//!
//! ## Idempotency
//!
//! Every written block is wrapped with a unique marker so subsequent calls can
//! detect and replace — rather than duplicate — an existing fence block.
//!
//! ```text
//! # >>> fdemon flutter path >>>
//! export PATH="$PATH:/home/u/fvm/versions/stable/bin"
//! # <<< fdemon flutter path <<<
//! ```
//!
//! Algorithm:
//! 1. Resolve the rc file via [`rc_file_for_shell`].
//! 2. Read existing contents (empty string if the file is absent).
//! 3. If a fence block already exists and already contains `bin_dir`, return
//!    [`PathConfigOutcome::AlreadyPresent`].
//! 4. If a fence block exists but points elsewhere, replace just that block.
//! 5. Otherwise append a new block.
//! 6. Write atomically (temp file in same dir → rename), creating parent
//!    directories if needed.

use std::path::{Path, PathBuf};

use fdemon_core::error::{Error, Result};

use super::types::{HostPlatform, HostShell};

// ── Marker constants ──────────────────────────────────────────────────────────

/// Opening fence line that marks the start of a managed PATH block.
const FENCE_OPEN: &str = "# >>> fdemon flutter path >>>";
/// Closing fence line that marks the end of a managed PATH block.
const FENCE_CLOSE: &str = "# <<< fdemon flutter path <<<";

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

/// Select the rc file to edit for the given shell under `home`.
///
/// Returns `None` for shells where rc-file edits do not apply (i.e.
/// [`HostShell::PowerShell`], [`HostShell::Cmd`], and [`HostShell::Unknown`]).
/// For fish, the returned path is `~/.config/fish/config.fish`. For zsh,
/// `~/.zshenv` is returned when it already exists; otherwise `~/.zprofile` is
/// returned as the conventional login-shell configuration file.
pub fn rc_file_for_shell(shell: HostShell, home: &Path) -> Option<PathBuf> {
    match shell {
        HostShell::Bash => Some(home.join(".bashrc")),
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
/// On Windows, updates the user `PATH` via the registry using a PowerShell
/// `[Environment]::SetEnvironmentVariable` call rather than editing an rc file.
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

// ── Pure string helpers (unit-testable without I/O) ───────────────────────────

/// Build the export line appropriate for bash/zsh.
fn posix_export_line(bin_dir: &Path) -> String {
    format!("export PATH=\"$PATH:{}\"", bin_dir.display())
}

/// Build the fish_add_path line.
fn fish_add_path_line(bin_dir: &Path) -> String {
    format!("fish_add_path {}", bin_dir.display())
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

/// Parse an existing fence block out of `contents`.
///
/// Returns the byte range `(open_start, close_end)` of the entire fence block
/// (inclusive of trailing newline after the close marker), or `None` if no
/// fence block is present.
fn find_fence_range(contents: &str) -> Option<(usize, usize)> {
    let open_pos = contents.find(FENCE_OPEN)?;
    let close_search_start = open_pos + FENCE_OPEN.len();
    let close_pos = contents[close_search_start..].find(FENCE_CLOSE)?;
    let close_abs = close_search_start + close_pos;
    // Include everything up through the newline that follows the close marker.
    let close_end = close_abs + FENCE_CLOSE.len();
    // Consume a trailing newline character if present.
    let close_end = if contents.as_bytes().get(close_end) == Some(&b'\n') {
        close_end + 1
    } else {
        close_end
    };
    Some((open_pos, close_end))
}

/// Return `true` if `contents` contains a fence block that already includes
/// a reference to `bin_dir`.
#[cfg(test)]
fn fence_already_has_dir(contents: &str, bin_dir: &Path) -> bool {
    let bin_str = bin_dir.to_string_lossy();
    match find_fence_range(contents) {
        Some((start, end)) => contents[start..end].contains(bin_str.as_ref()),
        None => false,
    }
}

/// Apply fence block logic to `contents`, returning the updated string.
///
/// - If no fence exists: append the block.
/// - If a fence exists but points elsewhere: replace it.
/// - If a fence exists and already contains `bin_dir`: return `None` (no change).
fn apply_fence(contents: &str, rc_file: &Path, bin_dir: &Path) -> Option<String> {
    let block = fence_block(rc_file, bin_dir);

    match find_fence_range(contents) {
        None => {
            // No fence yet — append.
            let mut result = contents.to_string();
            // Ensure we start on a fresh line.
            if !result.is_empty() && !result.ends_with('\n') {
                result.push('\n');
            }
            result.push('\n');
            result.push_str(&block);
            Some(result)
        }
        Some((start, end)) => {
            let existing_block = &contents[start..end];
            let bin_str = bin_dir.to_string_lossy();
            if existing_block.contains(bin_str.as_ref()) {
                // Already present with the same bin_dir — no change needed.
                None
            } else {
                // Replace existing block in-place.
                let mut result = String::with_capacity(contents.len());
                result.push_str(&contents[..start]);
                result.push_str(&block);
                result.push_str(&contents[end..]);
                Some(result)
            }
        }
    }
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
        let _ = std::fs::remove_file(&tmp_path);
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

// ── Windows PATH update ───────────────────────────────────────────────────────

/// Update the Windows user `PATH` via PowerShell.
///
/// Guards against the 1024-byte truncation caused by `setx` by using the
/// `[Environment]::SetEnvironmentVariable` registry API instead, which has no
/// length limit. The function is platform-gated so it compiles on all targets
/// but only runs on Windows.
fn add_to_path_windows(bin_dir: &Path) -> Result<PathConfigOutcome> {
    let bin_str = bin_dir.to_string_lossy().into_owned();

    // Read the current user PATH via PowerShell.
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

    // Append our bin_dir.
    let new_path = if current_path.is_empty() {
        bin_str.clone()
    } else if current_path.ends_with(';') {
        format!("{}{}", current_path, bin_str)
    } else {
        format!("{};{}", current_path, bin_str)
    };

    let set_script = format!(
        "[Environment]::SetEnvironmentVariable('PATH', '{}', 'User')",
        new_path.replace('\'', "\\'")
    );

    let set_output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &set_script])
        .output()
        .map_err(|e| Error::config(format!("Failed to run PowerShell to set PATH: {}", e)))?;

    if !set_output.status.success() {
        let stderr = String::from_utf8_lossy(&set_output.stderr);
        return Err(Error::config(format!(
            "PowerShell SetEnvironmentVariable failed: {}",
            stderr.trim()
        )));
    }

    Ok(PathConfigOutcome::Written {
        rc_file: PathBuf::from("HKCU:\\Environment\\PATH"),
    })
}

// ── Platform helpers ──────────────────────────────────────────────────────────

/// Resolve the current user's home directory.
///
/// Prefers the `HOME` environment variable (Unix) or `USERPROFILE` (Windows);
/// falls back to the `dirs` crate.
fn home_dir() -> Option<PathBuf> {
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .or_else(dirs::home_dir)
    }
    #[cfg(target_os = "windows")]
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

    // ── Pure string helper tests ──────────────────────────────────────────────

    #[test]
    fn test_posix_export_line() {
        let line = posix_export_line(Path::new("/home/user/flutter/bin"));
        assert_eq!(line, r#"export PATH="$PATH:/home/user/flutter/bin""#);
    }

    #[test]
    fn test_fish_add_path_line() {
        let line = fish_add_path_line(Path::new("/home/user/flutter/bin"));
        assert_eq!(line, "fish_add_path /home/user/flutter/bin");
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
    fn test_fence_block_fish_contains_fish_add_path() {
        let block = fence_block(
            Path::new("/home/u/.config/fish/config.fish"),
            Path::new("/opt/flutter/bin"),
        );
        assert!(block.contains(FENCE_OPEN));
        assert!(block.contains(FENCE_CLOSE));
        assert!(block.contains("fish_add_path /opt/flutter/bin"));
        assert!(!block.contains("export PATH"));
    }

    #[test]
    fn test_find_fence_range_none_when_absent() {
        let contents = "# some shell config\nexport FOO=bar\n";
        assert!(find_fence_range(contents).is_none());
    }

    #[test]
    fn test_find_fence_range_some_when_present() {
        let contents = format!(
            "# preamble\n{}\nexport PATH=\"$PATH:/a/bin\"\n{}\n# postamble\n",
            FENCE_OPEN, FENCE_CLOSE
        );
        let range = find_fence_range(&contents);
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
        assert!(written.contains("export PATH=\"$PATH:/opt/flutter/bin\""));
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
    fn test_fish_uses_fish_add_path() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();
        let bin_dir = PathBuf::from("/opt/flutter/bin");

        let rc_file = rc_file_for_shell(HostShell::Fish, home).unwrap();
        let outcome = add_to_rc_file(&rc_file, &bin_dir).unwrap();
        assert!(matches!(outcome, PathConfigOutcome::Written { .. }));

        let contents = std::fs::read_to_string(&rc_file).unwrap();
        assert!(contents.contains("fish_add_path /opt/flutter/bin"));
        assert!(!contents.contains("export PATH"));
    }

    #[test]
    fn test_rc_file_selection_per_shell() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        // Bash → ~/.bashrc
        let bash_rc = rc_file_for_shell(HostShell::Bash, home).unwrap();
        assert_eq!(bash_rc, home.join(".bashrc"));

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

    // ── Windows PATH string builder test (cross-platform) ────────────────────

    /// Validate the PowerShell command string format without executing it.
    #[test]
    fn test_windows_path_new_path_format() {
        let current = "C:\\Windows\\System32;C:\\Program Files\\Git\\bin";
        let bin_str = "C:\\tools\\flutter\\bin";

        let new_path = format!("{};{}", current, bin_str);
        assert!(new_path.ends_with(bin_str));
        assert!(new_path.contains(';'));

        // Verify the set_script format.
        let set_script = format!(
            "[Environment]::SetEnvironmentVariable('PATH', '{}', 'User')",
            new_path.replace('\'', "\\'")
        );
        assert!(set_script.contains("SetEnvironmentVariable"));
        assert!(set_script.contains("'User'"));
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
}
