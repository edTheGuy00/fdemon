//! Project-level Flutter operations (pub get, clean)
//!
//! This module provides the ProjectService trait for one-shot `flutter`
//! commands that operate on the project directory rather than a running
//! session. Both headless tooling and future MCP handlers use this trait.

use std::path::PathBuf;
use std::process::Stdio;

use fdemon_core::prelude::*;
use fdemon_daemon::FlutterExecutable;

/// Captured output of a completed one-shot flutter command.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    /// Whether the command exited with status 0
    pub success: bool,
    /// Exit code, if the process exited normally (None on signal termination)
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Project-level Flutter operations
///
/// Both TUI-side consumers and future MCP handlers use this trait.
#[trait_variant::make(ProjectService: Send)]
pub trait LocalProjectService {
    /// Run `flutter pub get` in the project directory.
    async fn pub_get(&self) -> Result<CommandOutput>;

    /// Run `flutter clean` in the project directory.
    async fn clean(&self) -> Result<CommandOutput>;
}

/// Implementation that invokes the resolved Flutter SDK executable.
///
/// `Send + 'static`, so it can be moved into spawned tokio tasks.
pub struct FlutterProjectService {
    flutter: FlutterExecutable,
    project_path: PathBuf,
}

impl FlutterProjectService {
    pub fn new(flutter: FlutterExecutable, project_path: PathBuf) -> Self {
        Self {
            flutter,
            project_path,
        }
    }

    /// Run the flutter executable with `args` in the project directory and
    /// capture its output. Non-zero exits are reported via
    /// [`CommandOutput::success`], not as errors; spawn failures are errors.
    async fn run_flutter(&self, args: &[&str]) -> Result<CommandOutput> {
        let output = self
            .flutter
            .command()
            .args(args)
            .current_dir(&self.project_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output()
            .await
            .map_err(|e| {
                Error::process(format!("failed to run flutter {}: {}", args.join(" "), e))
            })?;

        Ok(CommandOutput {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

impl ProjectService for FlutterProjectService {
    async fn pub_get(&self) -> Result<CommandOutput> {
        self.run_flutter(&["pub", "get"]).await
    }

    async fn clean(&self) -> Result<CommandOutput> {
        self.run_flutter(&["clean"]).await
    }
}

#[cfg(test)]
mod tests {
    // Import only the Send-variant trait: `FlutterProjectService` implements
    // both variants (Local via the trait_variant blanket impl), so importing
    // both would make plain method-call syntax ambiguous.
    use super::{CommandOutput, FlutterProjectService, ProjectService};
    use fdemon_daemon::FlutterExecutable;
    use std::path::PathBuf;

    fn service_with(executable: &str) -> FlutterProjectService {
        FlutterProjectService::new(
            FlutterExecutable::Direct(PathBuf::from(executable)),
            std::env::temp_dir(),
        )
    }

    #[test]
    fn test_command_output_fields() {
        let output = CommandOutput {
            success: true,
            exit_code: Some(0),
            stdout: "ok".to_string(),
            stderr: String::new(),
        };
        assert!(output.success);
        assert_eq!(output.exit_code, Some(0));
        assert_eq!(output.stdout, "ok");
        assert!(output.stderr.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_pub_get_invokes_executable_with_pub_get_args() {
        // `echo` stands in for the flutter binary: it prints its args, which
        // proves the service passed `pub get` through.
        let service = service_with("/bin/echo");

        let output = ProjectService::pub_get(&service).await.unwrap();
        assert!(output.success);
        assert_eq!(output.exit_code, Some(0));
        assert_eq!(output.stdout.trim(), "pub get");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_clean_invokes_executable_with_clean_arg() {
        let service = service_with("/bin/echo");

        let output = ProjectService::clean(&service).await.unwrap();
        assert!(output.success);
        assert_eq!(output.stdout.trim(), "clean");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_nonzero_exit_reported_as_unsuccessful_not_error() {
        let service = service_with("/bin/false");

        let output = ProjectService::clean(&service).await.unwrap();
        assert!(!output.success);
        assert_eq!(output.exit_code, Some(1));
    }

    #[tokio::test]
    async fn test_missing_executable_returns_error() {
        let service = service_with("/nonexistent/flutter-binary-for-test");

        let result = ProjectService::pub_get(&service).await;
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_project_service_usable_from_spawned_task() {
        let service = service_with("/bin/echo");

        let handle = tokio::spawn(async move { ProjectService::pub_get(&service).await });
        let output = handle.await.unwrap().unwrap();
        assert!(output.success);
    }
}
