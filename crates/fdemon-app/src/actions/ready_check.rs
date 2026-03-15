//! Ready check execution for pre-app custom sources.
//!
//! Each check type runs in a loop (or awaits a signal) until the readiness
//! condition is met or the timeout expires. All checks return a `ReadyCheckResult`
//! indicating success (with elapsed duration) or timeout.
//!
//! The public API (`run_ready_check`, `ReadyCheckResult`) is consumed by
//! [`super::native_logs::spawn_pre_app_sources`].

use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::oneshot;

use crate::config::ReadyCheck;

/// Result of a readiness check execution.
#[derive(Debug)]
pub enum ReadyCheckResult {
    /// The check succeeded. Contains the elapsed time.
    Ready(Duration),
    /// The check timed out after the configured duration.
    TimedOut(Duration),
    /// The check failed (e.g., process exited before stdout match).
    Failed(String),
}

impl ReadyCheckResult {
    /// Returns `true` if the check succeeded.
    pub fn is_ready(&self) -> bool {
        matches!(self, ReadyCheckResult::Ready(_))
    }
}

/// Execute a readiness check.
///
/// For `Stdout` checks, `ready_rx` must be provided (the corresponding
/// `oneshot::Sender` is held by the daemon capture loop). For all other
/// check types, `ready_rx` is ignored.
///
/// Returns `ReadyCheckResult::Ready(elapsed)` on success,
/// `ReadyCheckResult::TimedOut(elapsed)` on timeout.
pub async fn run_ready_check(
    check: &ReadyCheck,
    source_name: &str,
    ready_rx: Option<oneshot::Receiver<()>>,
) -> ReadyCheckResult {
    let start = Instant::now();
    match check {
        ReadyCheck::Http {
            url,
            interval_ms,
            timeout_s,
        } => run_http_check(url, *interval_ms, *timeout_s, source_name, start).await,
        ReadyCheck::Tcp {
            host,
            port,
            interval_ms,
            timeout_s,
        } => run_tcp_check(host, *port, *interval_ms, *timeout_s, source_name, start).await,
        ReadyCheck::Command {
            command,
            args,
            interval_ms,
            timeout_s,
        } => run_command_check(command, args, *interval_ms, *timeout_s, source_name, start).await,
        ReadyCheck::Stdout { timeout_s, .. } => {
            run_stdout_check(ready_rx, *timeout_s, source_name, start).await
        }
        ReadyCheck::Delay { seconds } => run_delay_check(*seconds, start).await,
    }
}

/// Poll an HTTP endpoint until it responds with a 2xx status or the timeout expires.
///
/// Uses raw TCP + minimal HTTP/1.1 (no `reqwest` dependency, per PLAN Decision 6).
/// Only `http://` URLs are supported; for HTTPS use the `Tcp` check type instead.
async fn run_http_check(
    url: &str,
    interval_ms: u64,
    timeout_s: u64,
    source_name: &str,
    start: Instant,
) -> ReadyCheckResult {
    let timeout = Duration::from_secs(timeout_s);
    let interval = Duration::from_millis(interval_ms);

    let (host, port, path) = match parse_http_url(url) {
        Ok(parts) => parts,
        Err(e) => return ReadyCheckResult::Failed(format!("invalid URL: {}", e)),
    };

    let addr = format!("{}:{}", host, port);

    loop {
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return ReadyCheckResult::TimedOut(start.elapsed());
        }

        // Cap each attempt at the remaining timeout so slow TCP connects
        // can't block past our deadline.
        match tokio::time::timeout(remaining, try_http_get(&addr, &host, &path)).await {
            Ok(Ok(true)) => return ReadyCheckResult::Ready(start.elapsed()),
            Ok(Ok(false)) => {
                tracing::debug!(
                    "Pre-app source '{}': HTTP check got non-2xx, retrying...",
                    source_name
                );
            }
            Ok(Err(e)) => {
                tracing::debug!(
                    "Pre-app source '{}': HTTP check failed: {}, retrying...",
                    source_name,
                    e
                );
            }
            Err(_) => {
                return ReadyCheckResult::TimedOut(start.elapsed());
            }
        }

        // Sleep for interval, but cap at remaining timeout to avoid oversleeping.
        let remaining = timeout.saturating_sub(start.elapsed());
        tokio::time::sleep(interval.min(remaining)).await;
    }
}

/// Attempt a single HTTP GET and return `true` if the status code is 2xx.
async fn try_http_get(addr: &str, host: &str, path: &str) -> std::io::Result<bool> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = TcpStream::connect(addr).await?;

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host
    );
    stream.write_all(request.as_bytes()).await?;

    // Read just enough to get the status line.
    let mut buf = [0u8; 256];
    let n = stream.read(&mut buf).await?;
    let response = String::from_utf8_lossy(&buf[..n]);

    // Parse "HTTP/1.x 2xx ..."
    if let Some(status_line) = response.lines().next() {
        if let Some(code_str) = status_line.split_whitespace().nth(1) {
            if let Ok(code) = code_str.parse::<u16>() {
                return Ok((200..300).contains(&code));
            }
        }
    }

    Ok(false)
}

/// Parse an HTTP URL into `(host, port, path)`.
///
/// Only `http://` scheme is supported. HTTPS is out of scope per PLAN Decision 6;
/// users can use the `Tcp` check type for HTTPS endpoints.
fn parse_http_url(url: &str) -> Result<(String, u16, String), String> {
    let stripped = url
        .strip_prefix("http://")
        .ok_or_else(|| "URL must start with http://".to_string())?;

    let (host_port, path) = match stripped.find('/') {
        Some(i) => (&stripped[..i], &stripped[i..]),
        None => (stripped, "/"),
    };

    let (host, port) = match host_port.find(':') {
        Some(i) => {
            let port = host_port[i + 1..]
                .parse::<u16>()
                .map_err(|e| format!("invalid port: {}", e))?;
            (&host_port[..i], port)
        }
        None => (host_port, 80u16),
    };

    Ok((host.to_string(), port, path.to_string()))
}

/// Poll a TCP host:port until a connection succeeds or the timeout expires.
async fn run_tcp_check(
    host: &str,
    port: u16,
    interval_ms: u64,
    timeout_s: u64,
    source_name: &str,
    start: Instant,
) -> ReadyCheckResult {
    let timeout = Duration::from_secs(timeout_s);
    let interval = Duration::from_millis(interval_ms);
    let addr = format!("{}:{}", host, port);

    loop {
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return ReadyCheckResult::TimedOut(start.elapsed());
        }

        // Cap each connect attempt at the remaining timeout so a slow OS-level
        // TCP SYN timeout (up to 75s on macOS) can't block past our deadline.
        match tokio::time::timeout(remaining, TcpStream::connect(&addr)).await {
            Ok(Ok(_)) => return ReadyCheckResult::Ready(start.elapsed()),
            Ok(Err(e)) => {
                tracing::debug!(
                    "Pre-app source '{}': TCP check {}:{} failed: {}, retrying...",
                    source_name,
                    host,
                    port,
                    e
                );
            }
            Err(_) => {
                return ReadyCheckResult::TimedOut(start.elapsed());
            }
        }

        // Sleep for interval, but cap at remaining timeout to avoid oversleeping.
        let remaining = timeout.saturating_sub(start.elapsed());
        tokio::time::sleep(interval.min(remaining)).await;
    }
}

/// Run an external command in a loop until it exits with code 0 or the timeout expires.
async fn run_command_check(
    command: &str,
    args: &[String],
    interval_ms: u64,
    timeout_s: u64,
    source_name: &str,
    start: Instant,
) -> ReadyCheckResult {
    let timeout = Duration::from_secs(timeout_s);
    let interval = Duration::from_millis(interval_ms);

    loop {
        if start.elapsed() >= timeout {
            return ReadyCheckResult::TimedOut(start.elapsed());
        }

        match tokio::process::Command::new(command)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
        {
            Ok(status) if status.success() => {
                return ReadyCheckResult::Ready(start.elapsed());
            }
            Ok(status) => {
                tracing::debug!(
                    "Pre-app source '{}': command check exited with {:?}, retrying...",
                    source_name,
                    status.code()
                );
            }
            Err(e) => {
                tracing::debug!(
                    "Pre-app source '{}': command check failed to spawn: {}, retrying...",
                    source_name,
                    e
                );
            }
        }

        // Sleep for interval, but cap at remaining timeout to avoid oversleeping.
        let remaining = timeout.saturating_sub(start.elapsed());
        tokio::time::sleep(interval.min(remaining)).await;
    }
}

/// Await a oneshot signal from the daemon stdout capture loop.
///
/// Returns `Ready` when the sender fires, `Failed` when the sender is dropped
/// (process exited before the pattern matched), or `TimedOut` if `timeout_s`
/// elapses with no signal.
async fn run_stdout_check(
    ready_rx: Option<oneshot::Receiver<()>>,
    timeout_s: u64,
    source_name: &str,
    start: Instant,
) -> ReadyCheckResult {
    let timeout = Duration::from_secs(timeout_s);

    let rx = match ready_rx {
        Some(rx) => rx,
        None => {
            tracing::warn!(
                "Pre-app source '{}': stdout check has no ready_rx — misconfiguration",
                source_name
            );
            return ReadyCheckResult::Failed("no ready_rx for stdout check".to_string());
        }
    };

    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(())) => ReadyCheckResult::Ready(start.elapsed()),
        Ok(Err(_)) => {
            // Sender was dropped — process exited before the pattern was matched.
            ReadyCheckResult::Failed(format!(
                "process exited before stdout pattern matched (after {:.1}s)",
                start.elapsed().as_secs_f64()
            ))
        }
        Err(_) => ReadyCheckResult::TimedOut(start.elapsed()),
    }
}

/// Wait a fixed duration before returning `Ready`.
async fn run_delay_check(seconds: u64, start: Instant) -> ReadyCheckResult {
    tokio::time::sleep(Duration::from_secs(seconds)).await;
    ReadyCheckResult::Ready(start.elapsed())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── URL parsing ───────────────────────────────────────

    #[test]
    fn test_parse_http_url_with_port_and_path() {
        let (host, port, path) = parse_http_url("http://localhost:8080/health").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 8080);
        assert_eq!(path, "/health");
    }

    #[test]
    fn test_parse_http_url_default_port() {
        let (host, port, path) = parse_http_url("http://example.com/status").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 80);
        assert_eq!(path, "/status");
    }

    #[test]
    fn test_parse_http_url_no_path() {
        let (host, port, path) = parse_http_url("http://localhost:3000").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 3000);
        assert_eq!(path, "/");
    }

    #[test]
    fn test_parse_http_url_rejects_https() {
        assert!(parse_http_url("https://localhost/health").is_err());
    }

    #[test]
    fn test_parse_http_url_rejects_no_scheme() {
        assert!(parse_http_url("localhost:8080/health").is_err());
    }

    #[test]
    fn test_parse_http_url_with_nested_path() {
        let (host, port, path) = parse_http_url("http://localhost:8080/api/v1/health").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 8080);
        assert_eq!(path, "/api/v1/health");
    }

    // ── TCP check ─────────────────────────────────────────

    #[tokio::test]
    async fn test_tcp_check_timeout_on_closed_port() {
        let check = ReadyCheck::Tcp {
            host: "127.0.0.1".to_string(),
            port: 1, // Port 1 is almost certainly not listening
            interval_ms: 100,
            timeout_s: 1,
        };
        let result = run_ready_check(&check, "test", None).await;
        assert!(matches!(result, ReadyCheckResult::TimedOut(_)));
    }

    #[tokio::test]
    async fn test_tcp_check_succeeds_on_open_port() {
        // Bind a listener, then check against it
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let check = ReadyCheck::Tcp {
            host: "127.0.0.1".to_string(),
            port,
            interval_ms: 100,
            timeout_s: 5,
        };
        let result = run_ready_check(&check, "test", None).await;
        assert!(result.is_ready());
    }

    // ── Command check ─────────────────────────────────────

    #[tokio::test]
    async fn test_command_check_succeeds_on_true() {
        let check = ReadyCheck::Command {
            command: "true".to_string(),
            args: vec![],
            interval_ms: 100,
            timeout_s: 5,
        };
        let result = run_ready_check(&check, "test", None).await;
        assert!(result.is_ready());
    }

    #[tokio::test]
    async fn test_command_check_timeout_on_false() {
        let check = ReadyCheck::Command {
            command: "false".to_string(),
            args: vec![],
            interval_ms: 100,
            timeout_s: 1,
        };
        let result = run_ready_check(&check, "test", None).await;
        assert!(matches!(result, ReadyCheckResult::TimedOut(_)));
    }

    // ── Stdout check ──────────────────────────────────────

    #[tokio::test]
    async fn test_stdout_check_succeeds_on_signal() {
        let (tx, rx) = oneshot::channel();
        let check = ReadyCheck::Stdout {
            pattern: "ready".to_string(),
            timeout_s: 5,
        };

        // Fire the signal immediately
        tx.send(()).unwrap();

        let result = run_ready_check(&check, "test", Some(rx)).await;
        assert!(result.is_ready());
    }

    #[tokio::test]
    async fn test_stdout_check_fails_on_sender_drop() {
        let (tx, rx) = oneshot::channel::<()>();
        let check = ReadyCheck::Stdout {
            pattern: "ready".to_string(),
            timeout_s: 5,
        };

        // Drop sender (simulates process exit before match)
        drop(tx);

        let result = run_ready_check(&check, "test", Some(rx)).await;
        assert!(matches!(result, ReadyCheckResult::Failed(_)));
    }

    #[tokio::test]
    async fn test_stdout_check_timeout() {
        let (_tx, rx) = oneshot::channel::<()>(); // tx held but never sent
        let check = ReadyCheck::Stdout {
            pattern: "ready".to_string(),
            timeout_s: 1,
        };

        let result = run_ready_check(&check, "test", Some(rx)).await;
        assert!(matches!(result, ReadyCheckResult::TimedOut(_)));
    }

    #[tokio::test]
    async fn test_stdout_check_no_rx() {
        let check = ReadyCheck::Stdout {
            pattern: "ready".to_string(),
            timeout_s: 5,
        };

        let result = run_ready_check(&check, "test", None).await;
        assert!(matches!(result, ReadyCheckResult::Failed(_)));
    }

    // ── Delay check ───────────────────────────────────────

    #[tokio::test]
    async fn test_delay_check() {
        let check = ReadyCheck::Delay { seconds: 1 };
        let start = Instant::now();
        let result = run_ready_check(&check, "test", None).await;
        assert!(result.is_ready());
        assert!(start.elapsed() >= Duration::from_secs(1));
    }
}
