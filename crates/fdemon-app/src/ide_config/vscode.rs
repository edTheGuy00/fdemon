//! VS Code DAP configuration generator.
//!
//! Generates or merges a `.vscode/launch.json` entry so that VS Code,
//! VS Code Insiders, and Cursor can attach to the fdemon DAP server via
//! the `debugServer` field (a VS Code-internal mechanism that redirects
//! the debug-adapter transport to an existing TCP server).

use std::path::{Path, PathBuf};

use fdemon_core::Result;
use serde_json::json;

use super::{
    merge::{clean_jsonc, merge_json_array_entry, to_pretty_json, FDEMON_CONFIG_NAME},
    IdeConfigGenerator,
};

/// Detect the workspace root for a Flutter project.
///
/// Walks up from `project_root` looking for a directory that contains either
/// `.vscode/` (existing VS Code workspace) or `.git/` (repository root).
/// Returns the nearest ancestor that qualifies, or `project_root` itself if
/// no workspace marker is found.
///
/// The search stops at the filesystem root. Home-directory boundaries are not
/// treated specially — the walk stops at the first match or the root.
///
/// # Priority
///
/// 1. A parent with `.vscode/` is preferred (VS Code was opened there).
/// 2. A parent with `.git/` is the fallback (likely the repo root).
/// 3. If neither is found, the project root is returned unchanged.
pub fn detect_workspace_root(project_root: &Path) -> PathBuf {
    // Canonicalize to resolve symlinks so ancestor comparisons are reliable.
    let canonical = match project_root.canonicalize() {
        Ok(p) => p,
        Err(_) => return project_root.to_path_buf(),
    };

    let mut git_root: Option<PathBuf> = None;

    // Walk the parent chain (skip the project_root itself — we want ancestors).
    for ancestor in canonical.ancestors().skip(1) {
        if ancestor.join(".vscode").is_dir() {
            return ancestor.to_path_buf();
        }
        // Record the first (nearest) `.git/` we see, but keep walking in case
        // a `.vscode/` ancestor exists further up.
        if git_root.is_none() && ancestor.join(".git").exists() {
            git_root = Some(ancestor.to_path_buf());
        }
    }

    // No `.vscode/` ancestor found — fall back to the nearest `.git/` root.
    git_root.unwrap_or(canonical)
}

/// Compute the `cwd` value for the launch.json entry.
///
/// When `workspace_root` and `project_root` are the same directory, VS Code's
/// `${workspaceFolder}` variable is the correct value.
///
/// When they differ (monorepo case) the `cwd` must be the relative path from
/// the workspace root to the project root so that VS Code resolves sources
/// correctly inside the nested project.
fn compute_cwd(project_root: &Path, workspace_root: &Path) -> String {
    // Canonicalize both sides so we compare real paths.
    let canonical_project = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let canonical_workspace = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());

    if canonical_project == canonical_workspace {
        return "${workspaceFolder}".to_string();
    }

    // Compute the relative path from workspace_root to project_root.
    match canonical_project.strip_prefix(&canonical_workspace) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        // If the prefix strip fails (shouldn't happen, but be safe), fall back.
        Err(_) => "${workspaceFolder}".to_string(),
    }
}

/// Generates `.vscode/launch.json` DAP config for VS Code, VS Code Insiders, and Cursor.
///
/// Uses the `debugServer` field which tells VS Code to connect to an already-running
/// DAP server on the given port instead of spawning a debug adapter process.
/// The Dart extension must be installed (provides `"type": "dart"`).
pub struct VSCodeGenerator;

impl VSCodeGenerator {
    /// Build the fdemon launch configuration entry for a given port and project paths.
    ///
    /// When `project_root` and `workspace_root` are the same directory, `cwd` is
    /// set to `"${workspaceFolder}"`. For monorepo setups where the project lives
    /// in a subdirectory of the workspace, `cwd` is set to the relative path from
    /// the workspace root to the project root (e.g. `"example/app3"`).
    fn fdemon_entry(port: u16, project_root: &Path, workspace_root: &Path) -> serde_json::Value {
        let cwd = compute_cwd(project_root, workspace_root);
        json!({
            "name": FDEMON_CONFIG_NAME,
            "type": "dart",
            "request": "attach",
            "debugServer": port,
            "cwd": cwd
        })
    }
}

impl IdeConfigGenerator for VSCodeGenerator {
    fn config_path(&self, project_root: &Path) -> PathBuf {
        let workspace_root = detect_workspace_root(project_root);
        workspace_root.join(".vscode").join("launch.json")
    }

    fn generate(&self, port: u16, project_root: &Path) -> Result<String> {
        let workspace_root = detect_workspace_root(project_root);
        let config = json!({
            "version": "0.2.0",
            "configurations": [
                Self::fdemon_entry(port, project_root, &workspace_root)
            ]
        });
        Ok(to_pretty_json(&config))
    }

    fn merge_config(&self, existing: &str, port: u16, project_root: &Path) -> Result<String> {
        // Treat an empty/whitespace-only file as a fresh generation.
        if existing.trim().is_empty() {
            return self.generate(port, project_root);
        }

        let workspace_root = detect_workspace_root(project_root);

        // Strip JSONC comments and trailing commas before parsing.
        let clean = clean_jsonc(existing);
        let mut root: serde_json::Value = serde_json::from_str(&clean)?;

        // Ensure `configurations` array exists.
        if root.get("configurations").is_none() {
            root["configurations"] = json!([]);
        }

        let configurations = root["configurations"]
            .as_array_mut()
            .ok_or_else(|| fdemon_core::Error::config("`configurations` is not an array"))?;

        merge_json_array_entry(
            configurations,
            "name",
            FDEMON_CONFIG_NAME,
            Self::fdemon_entry(port, project_root, &workspace_root),
        );

        Ok(to_pretty_json(&root))
    }

    fn ide_name(&self) -> &'static str {
        "VS Code"
    }
}

// ─────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // ── detect_workspace_root ────────────────────────────────────

    #[test]
    fn test_detect_workspace_root_project_has_vscode_returns_project() {
        // project_root itself has .vscode/ — walk should return project_root.
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".vscode")).unwrap();
        // detect_workspace_root walks ANCESTORS, so .vscode in project_root
        // itself is NOT found (we skip the project_root in the walk).
        // => returns canonical(project_root) since no ancestor has .vscode or .git.
        let detected = detect_workspace_root(dir.path());
        assert_eq!(
            detected.canonicalize().unwrap(),
            dir.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn test_detect_workspace_root_parent_has_vscode() {
        // Layout: workspace/.vscode/  workspace/app/  (project_root = workspace/app)
        let root = tempdir().unwrap();
        let workspace = root.path();
        let project = workspace.join("app");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(workspace.join(".vscode")).unwrap();

        let detected = detect_workspace_root(&project);
        assert_eq!(
            detected.canonicalize().unwrap(),
            workspace.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_detect_workspace_root_grandparent_has_vscode() {
        // Layout: repo/.vscode/  repo/packages/myapp/
        let root = tempdir().unwrap();
        let repo = root.path();
        let project = repo.join("packages").join("myapp");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(repo.join(".vscode")).unwrap();

        let detected = detect_workspace_root(&project);
        assert_eq!(
            detected.canonicalize().unwrap(),
            repo.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_detect_workspace_root_parent_has_git() {
        // Layout: repo/.git/  repo/app/  (no .vscode anywhere)
        let root = tempdir().unwrap();
        let repo = root.path();
        let project = repo.join("app");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();

        let detected = detect_workspace_root(&project);
        assert_eq!(
            detected.canonicalize().unwrap(),
            repo.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_detect_workspace_root_grandparent_has_git() {
        // Layout: repo/.git/  repo/packages/app/
        let root = tempdir().unwrap();
        let repo = root.path();
        let project = repo.join("packages").join("app");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();

        let detected = detect_workspace_root(&project);
        assert_eq!(
            detected.canonicalize().unwrap(),
            repo.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_detect_workspace_root_no_git_no_vscode_returns_project() {
        // No markers anywhere → returns canonical(project_root)
        let dir = tempdir().unwrap();
        let project = dir.path().join("app");
        std::fs::create_dir_all(&project).unwrap();

        let detected = detect_workspace_root(&project);
        assert_eq!(
            detected.canonicalize().unwrap(),
            project.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_detect_workspace_root_vscode_preferred_over_git() {
        // Layout: repo/.git/  repo/workspace/.vscode/  repo/workspace/app/
        // .vscode is nearer to project_root than .git → .vscode wins.
        let root = tempdir().unwrap();
        let repo = root.path();
        let workspace = repo.join("workspace");
        let project = workspace.join("app");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(workspace.join(".vscode")).unwrap();

        let detected = detect_workspace_root(&project);
        assert_eq!(
            detected.canonicalize().unwrap(),
            workspace.canonicalize().unwrap()
        );
    }

    // ── compute_cwd ──────────────────────────────────────────────

    #[test]
    fn test_compute_cwd_same_dir_returns_workspace_folder() {
        let dir = tempdir().unwrap();
        let result = compute_cwd(dir.path(), dir.path());
        assert_eq!(result, "${workspaceFolder}");
    }

    #[test]
    fn test_compute_cwd_project_is_child_returns_relative_path() {
        let root = tempdir().unwrap();
        let project = root.path().join("example").join("app3");
        std::fs::create_dir_all(&project).unwrap();

        let result = compute_cwd(&project, root.path());
        assert_eq!(result, "example/app3");
    }

    #[test]
    fn test_compute_cwd_direct_child_returns_relative_path() {
        let root = tempdir().unwrap();
        let project = root.path().join("myapp");
        std::fs::create_dir_all(&project).unwrap();

        let result = compute_cwd(&project, root.path());
        assert_eq!(result, "myapp");
    }

    // ── config_path ──────────────────────────────────────────────

    #[test]
    fn test_vscode_config_path_single_project() {
        // project_root has no parent .vscode or .git → config_path is under project_root
        let dir = tempdir().unwrap();
        let project = dir.path().join("app");
        std::fs::create_dir_all(&project).unwrap();

        let gen = VSCodeGenerator;
        let path = gen.config_path(&project);
        assert!(path.ends_with(".vscode/launch.json"));
        // The base should be project (or its canonical form) since no markers found
        let canonical_project = project.canonicalize().unwrap();
        assert_eq!(path, canonical_project.join(".vscode").join("launch.json"));
    }

    #[test]
    fn test_vscode_config_path_monorepo_uses_workspace_root() {
        // Layout: workspace/.vscode/  workspace/app/
        let root = tempdir().unwrap();
        let workspace = root.path();
        let project = workspace.join("app");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(workspace.join(".vscode")).unwrap();

        let gen = VSCodeGenerator;
        let path = gen.config_path(&project);
        assert_eq!(
            path,
            workspace
                .canonicalize()
                .unwrap()
                .join(".vscode")
                .join("launch.json")
        );
    }

    // ── fresh generation ─────────────────────────────────────────

    #[test]
    fn test_vscode_fresh_generation() {
        let gen = VSCodeGenerator;
        let dir = tempdir().unwrap();
        let content = gen.generate(4711, dir.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["version"], "0.2.0");
        let configs = parsed["configurations"].as_array().unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0]["name"], "Flutter (fdemon)");
        assert_eq!(configs[0]["debugServer"], 4711);
        assert_eq!(configs[0]["type"], "dart");
        assert_eq!(configs[0]["request"], "attach");
        assert!(configs[0].get("fdemon-managed").is_none());
    }

    #[test]
    fn test_vscode_fresh_generation_port_substitution() {
        let gen = VSCodeGenerator;
        let dir = tempdir().unwrap();
        let content = gen.generate(9999, dir.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["configurations"][0]["debugServer"], 9999);
    }

    #[test]
    fn test_vscode_single_project_cwd_is_workspace_folder() {
        // No markers → workspace root == project root → cwd = ${workspaceFolder}
        let dir = tempdir().unwrap();
        let project = dir.path().join("app");
        std::fs::create_dir_all(&project).unwrap();

        let gen = VSCodeGenerator;
        let content = gen.generate(4711, &project).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["configurations"][0]["cwd"], "${workspaceFolder}");
    }

    #[test]
    fn test_vscode_monorepo_cwd_is_relative_path() {
        // Layout: workspace/.vscode/  workspace/example/app3/
        let root = tempdir().unwrap();
        let workspace = root.path();
        let project = workspace.join("example").join("app3");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(workspace.join(".vscode")).unwrap();

        let gen = VSCodeGenerator;
        let content = gen.generate(4711, &project).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["configurations"][0]["cwd"], "example/app3");
    }

    // ── merge_config ─────────────────────────────────────────────

    #[test]
    fn test_vscode_merge_updates_existing_entry() {
        let existing = r#"{
            "version": "0.2.0",
            "configurations": [
                {"name": "Dart", "type": "dart", "request": "launch"},
                {"name": "Flutter (fdemon)", "type": "dart", "debugServer": 1234, "fdemon-managed": true}
            ]
        }"#;
        let gen = VSCodeGenerator;
        let dir = tempdir().unwrap();
        let merged = gen.merge_config(existing, 5678, dir.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        let configs = parsed["configurations"].as_array().unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0]["name"], "Dart"); // preserved
        assert_eq!(configs[1]["debugServer"], 5678); // updated
    }

    #[test]
    fn test_vscode_merge_appends_when_no_fdemon_entry() {
        let existing = r#"{
            "version": "0.2.0",
            "configurations": [
                {"name": "Dart", "type": "dart", "request": "launch"}
            ]
        }"#;
        let gen = VSCodeGenerator;
        let dir = tempdir().unwrap();
        let merged = gen.merge_config(existing, 4711, dir.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        let configs = parsed["configurations"].as_array().unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[1]["name"], "Flutter (fdemon)");
    }

    #[test]
    fn test_vscode_merge_handles_jsonc_comments() {
        let existing = r#"{
            // This is a comment
            "version": "0.2.0",
            "configurations": [
                {"name": "Dart", "type": "dart"}
            ]
        }"#;
        let gen = VSCodeGenerator;
        let dir = tempdir().unwrap();
        let result = gen.merge_config(existing, 4711, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_vscode_merge_malformed_json_returns_error() {
        let gen = VSCodeGenerator;
        let dir = tempdir().unwrap();
        let result = gen.merge_config("not json at all {{{", 4711, dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_vscode_merge_preserves_version() {
        let existing = r#"{"version": "0.2.0", "configurations": []}"#;
        let gen = VSCodeGenerator;
        let dir = tempdir().unwrap();
        let merged = gen.merge_config(existing, 4711, dir.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(parsed["version"], "0.2.0");
    }

    #[test]
    fn test_vscode_merge_no_configurations_key() {
        let existing = r#"{"version": "0.2.0"}"#;
        let gen = VSCodeGenerator;
        let dir = tempdir().unwrap();
        let merged = gen.merge_config(existing, 4711, dir.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        assert!(parsed["configurations"].is_array());
    }

    #[test]
    fn test_vscode_merge_empty_file_acts_as_fresh_generation() {
        let gen = VSCodeGenerator;
        let dir = tempdir().unwrap();
        let result = gen.merge_config("", 4711, dir.path());
        assert!(result.is_ok());
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed["configurations"][0]["debugServer"], 4711);
    }

    #[test]
    fn test_vscode_merge_preserves_other_configurations() {
        let existing = r#"{
            "version": "0.2.0",
            "configurations": [
                {"name": "Config A", "type": "dart"},
                {"name": "Flutter (fdemon)", "debugServer": 1000, "fdemon-managed": true},
                {"name": "Config B", "type": "dart"}
            ]
        }"#;
        let gen = VSCodeGenerator;
        let dir = tempdir().unwrap();
        let merged = gen.merge_config(existing, 4711, dir.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        let configs = parsed["configurations"].as_array().unwrap();
        assert_eq!(configs.len(), 3);
        assert_eq!(configs[0]["name"], "Config A");
        assert_eq!(configs[1]["debugServer"], 4711);
        assert_eq!(configs[2]["name"], "Config B");
    }

    #[test]
    fn test_vscode_ide_name() {
        assert_eq!(VSCodeGenerator.ide_name(), "VS Code");
    }
}
