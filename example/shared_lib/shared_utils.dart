/// Shared utility library for cross-project watcher testing.
///
/// This directory is intentionally outside any Flutter project — it
/// represents the shared-code-directory pattern common in monorepos.
///
/// When running `cargo run -- example/app4`, fdemon resolves the watcher
/// path "../../shared_lib" (relative to example/app4/) to this directory.
/// Editing this file should trigger hot reload in the running app4 session.

/// Returns a formatted greeting string.
String greet(String name) => 'Hello, $name!';

/// Formats a duration as a human-readable string.
String formatDuration(Duration d) {
  if (d.inHours > 0) return '${d.inHours}h ${d.inMinutes.remainder(60)}m';
  if (d.inMinutes > 0) return '${d.inMinutes}m ${d.inSeconds.remainder(60)}s';
  return '${d.inSeconds}s';
}
