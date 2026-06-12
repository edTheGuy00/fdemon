//! Log buffer access and subscription
//!
//! This module provides the LogService trait for accessing and filtering
//! application logs. Both the TUI and future MCP handlers use this trait.

use std::sync::Arc;

use chrono::{DateTime, Local};
use tokio::sync::RwLock;

use fdemon_core::{LogEntry, LogLevel};

/// Filter for querying logs
///
/// All criteria are conjunctive: an entry must match every populated field.
/// `source` is compared case-insensitively against [`fdemon_core::LogSource::prefix`]
/// (e.g. `"app"`, `"daemon"`, `"flutter"`, or a native tag name).
#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    pub level: Option<LogLevel>,
    pub source: Option<String>,
    pub pattern: Option<String>,
    /// Only include entries with `timestamp >= since`.
    pub since: Option<DateTime<Local>>,
    pub limit: Option<usize>,
}

impl LogFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn errors() -> Self {
        Self {
            level: Some(LogLevel::Error),
            ..Default::default()
        }
    }

    pub fn warnings() -> Self {
        Self {
            level: Some(LogLevel::Warning),
            ..Default::default()
        }
    }

    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.level = Some(level);
        self
    }

    pub fn with_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_since(mut self, since: DateTime<Local>) -> Self {
        self.since = Some(since);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Check whether `log` matches every populated criterion.
    pub fn matches(&self, log: &LogEntry) -> bool {
        if let Some(level) = &self.level {
            if &log.level != level {
                return false;
            }
        }

        if let Some(source) = &self.source {
            if !log.source.prefix().eq_ignore_ascii_case(source) {
                return false;
            }
        }

        if let Some(pattern) = &self.pattern {
            if !log.message.contains(pattern.as_str()) {
                return false;
            }
        }

        if let Some(since) = &self.since {
            if log.timestamp < *since {
                return false;
            }
        }

        true
    }
}

/// Log buffer access and subscription
#[trait_variant::make(LogService: Send)]
pub trait LocalLogService {
    /// Get logs matching the filter
    async fn get_logs(&self, filter: Option<LogFilter>) -> Vec<LogEntry>;

    /// Get error-level logs
    async fn get_errors(&self) -> Vec<LogEntry>;

    /// Get the total log count
    async fn log_count(&self) -> usize;

    /// Clear all logs
    async fn clear(&self);

    /// Add a log entry
    async fn add_log(&self, entry: LogEntry);
}

/// Implementation using shared log buffer
pub struct SharedLogService {
    logs: Arc<RwLock<Vec<LogEntry>>>,
    max_logs: usize,
}

impl SharedLogService {
    pub fn new(logs: Arc<RwLock<Vec<LogEntry>>>, max_logs: usize) -> Self {
        Self { logs, max_logs }
    }
}

impl LocalLogService for SharedLogService {
    async fn get_logs(&self, filter: Option<LogFilter>) -> Vec<LogEntry> {
        let logs = self.logs.read().await;

        let filter = filter.unwrap_or_default();

        let mut result: Vec<LogEntry> = logs
            .iter()
            .filter(|log| filter.matches(log))
            .cloned()
            .collect();

        // Apply limit (from end, most recent)
        if let Some(limit) = filter.limit {
            let start = result.len().saturating_sub(limit);
            result = result[start..].to_vec();
        }

        result
    }

    async fn get_errors(&self) -> Vec<LogEntry> {
        self.get_logs(Some(LogFilter::errors())).await
    }

    async fn log_count(&self) -> usize {
        self.logs.read().await.len()
    }

    async fn clear(&self) {
        self.logs.write().await.clear();
    }

    async fn add_log(&self, entry: LogEntry) {
        let mut logs = self.logs.write().await;
        logs.push(entry);

        // Trim if over max size
        if logs.len() > self.max_logs {
            let drain_count = logs.len() - self.max_logs;
            logs.drain(0..drain_count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::LogSource;

    fn create_test_service(max_logs: usize) -> SharedLogService {
        SharedLogService::new(Arc::new(RwLock::new(Vec::new())), max_logs)
    }

    #[test]
    fn test_log_filter_new() {
        let filter = LogFilter::new();
        assert!(filter.level.is_none());
        assert!(filter.source.is_none());
        assert!(filter.pattern.is_none());
        assert!(filter.limit.is_none());
    }

    #[test]
    fn test_log_filter_errors() {
        let filter = LogFilter::errors();
        assert_eq!(filter.level, Some(LogLevel::Error));
    }

    #[test]
    fn test_log_filter_warnings() {
        let filter = LogFilter::warnings();
        assert_eq!(filter.level, Some(LogLevel::Warning));
    }

    #[test]
    fn test_log_filter_builder() {
        let filter = LogFilter::new()
            .with_level(LogLevel::Error)
            .with_pattern("test")
            .with_limit(10);

        assert_eq!(filter.level, Some(LogLevel::Error));
        assert_eq!(filter.pattern, Some("test".to_string()));
        assert_eq!(filter.limit, Some(10));
    }

    #[tokio::test]
    async fn test_log_service_add_and_get() {
        let service = create_test_service(100);

        service
            .add_log(LogEntry::info(LogSource::App, "test message"))
            .await;

        let logs = service.get_logs(None).await;
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "test message");
    }

    #[tokio::test]
    async fn test_log_service_filtering() {
        let service = create_test_service(100);

        // Add mixed logs
        service
            .add_log(LogEntry::info(LogSource::App, "info message"))
            .await;
        service
            .add_log(LogEntry::error(LogSource::App, "error message"))
            .await;
        service
            .add_log(LogEntry::warn(LogSource::App, "warning"))
            .await;

        // Get all
        let all = service.get_logs(None).await;
        assert_eq!(all.len(), 3);

        // Get errors only
        let errors = service.get_errors().await;
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("error"));

        // Get with limit
        let limited = service
            .get_logs(Some(LogFilter::default().with_limit(2)))
            .await;
        assert_eq!(limited.len(), 2);
    }

    #[tokio::test]
    async fn test_log_service_pattern_filter() {
        let service = create_test_service(100);

        service
            .add_log(LogEntry::info(LogSource::App, "apple"))
            .await;
        service
            .add_log(LogEntry::info(LogSource::App, "banana"))
            .await;
        service
            .add_log(LogEntry::info(LogSource::App, "apple pie"))
            .await;

        let filtered = service
            .get_logs(Some(LogFilter::new().with_pattern("apple")))
            .await;
        assert_eq!(filtered.len(), 2);
    }

    #[tokio::test]
    async fn test_log_service_max_size() {
        let service = create_test_service(3); // Max 3 logs

        service.add_log(LogEntry::info(LogSource::App, "1")).await;
        service.add_log(LogEntry::info(LogSource::App, "2")).await;
        service.add_log(LogEntry::info(LogSource::App, "3")).await;
        service.add_log(LogEntry::info(LogSource::App, "4")).await;

        let logs = service.get_logs(None).await;
        assert_eq!(logs.len(), 3);
        assert_eq!(logs[0].message, "2"); // First was trimmed
    }

    #[tokio::test]
    async fn test_log_service_clear() {
        let service = create_test_service(100);

        service
            .add_log(LogEntry::info(LogSource::App, "test"))
            .await;
        assert_eq!(service.log_count().await, 1);

        service.clear().await;
        assert_eq!(service.log_count().await, 0);
    }

    #[tokio::test]
    async fn test_log_count() {
        let service = create_test_service(100);

        assert_eq!(service.log_count().await, 0);

        service.add_log(LogEntry::info(LogSource::App, "1")).await;
        service.add_log(LogEntry::info(LogSource::App, "2")).await;

        assert_eq!(service.log_count().await, 2);
    }

    #[tokio::test]
    async fn test_source_filter_matches_prefix_case_insensitive() {
        let service = create_test_service(100);

        service
            .add_log(LogEntry::info(LogSource::App, "from app"))
            .await;
        service
            .add_log(LogEntry::info(LogSource::Daemon, "from daemon"))
            .await;
        service
            .add_log(LogEntry::info(LogSource::Flutter, "from flutter"))
            .await;

        let filtered = service
            .get_logs(Some(LogFilter::new().with_source("DAEMON")))
            .await;
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "from daemon");
    }

    #[tokio::test]
    async fn test_source_filter_matches_native_tag() {
        let service = create_test_service(100);

        service
            .add_log(LogEntry::info(
                LogSource::Native {
                    tag: "ActivityManager".to_string(),
                },
                "native line",
            ))
            .await;
        service
            .add_log(LogEntry::info(LogSource::App, "app line"))
            .await;

        let filtered = service
            .get_logs(Some(LogFilter::new().with_source("ActivityManager")))
            .await;
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "native line");
    }

    #[tokio::test]
    async fn test_since_filter_excludes_older_entries() {
        let service = create_test_service(100);

        let mut old = LogEntry::info(LogSource::App, "old");
        old.timestamp = chrono::Local::now() - chrono::Duration::seconds(60);
        service.add_log(old).await;
        service.add_log(LogEntry::info(LogSource::App, "new")).await;

        let cutoff = chrono::Local::now() - chrono::Duration::seconds(30);
        let filtered = service
            .get_logs(Some(LogFilter::new().with_since(cutoff)))
            .await;
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "new");
    }

    #[tokio::test]
    async fn test_since_filter_is_inclusive_of_boundary() {
        let service = create_test_service(100);

        let entry = LogEntry::info(LogSource::App, "boundary");
        let exact = entry.timestamp;
        service.add_log(entry).await;

        let filtered = service
            .get_logs(Some(LogFilter::new().with_since(exact)))
            .await;
        assert_eq!(filtered.len(), 1, "timestamp == since must match");
    }

    #[tokio::test]
    async fn test_combined_level_pattern_since_filter() {
        let service = create_test_service(100);

        let mut old_error = LogEntry::error(LogSource::App, "apple crash");
        old_error.timestamp = chrono::Local::now() - chrono::Duration::seconds(120);
        service.add_log(old_error).await;
        service
            .add_log(LogEntry::error(LogSource::App, "apple crash again"))
            .await;
        service
            .add_log(LogEntry::error(LogSource::App, "banana crash"))
            .await;
        service
            .add_log(LogEntry::info(LogSource::App, "apple info"))
            .await;

        let cutoff = chrono::Local::now() - chrono::Duration::seconds(30);
        let filtered = service
            .get_logs(Some(
                LogFilter::new()
                    .with_level(LogLevel::Error)
                    .with_pattern("apple")
                    .with_since(cutoff),
            ))
            .await;
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "apple crash again");
    }

    #[tokio::test]
    async fn test_combined_level_and_source_filter() {
        let service = create_test_service(100);

        service
            .add_log(LogEntry::error(LogSource::App, "app error"))
            .await;
        service
            .add_log(LogEntry::error(LogSource::Daemon, "daemon error"))
            .await;
        service
            .add_log(LogEntry::info(LogSource::Daemon, "daemon info"))
            .await;

        let filtered = service
            .get_logs(Some(
                LogFilter::new()
                    .with_level(LogLevel::Error)
                    .with_source("daemon"),
            ))
            .await;
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "daemon error");
    }

    #[tokio::test]
    async fn test_limit_applies_after_other_filters() {
        let service = create_test_service(100);

        for i in 0..5 {
            service
                .add_log(LogEntry::error(LogSource::App, format!("error {}", i)))
                .await;
            service
                .add_log(LogEntry::info(LogSource::App, format!("info {}", i)))
                .await;
        }

        let filtered = service
            .get_logs(Some(LogFilter::errors().with_limit(2)))
            .await;
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].message, "error 3");
        assert_eq!(filtered[1].message, "error 4");
    }

    #[tokio::test]
    async fn test_limit_returns_most_recent() {
        let service = create_test_service(100);

        service
            .add_log(LogEntry::info(LogSource::App, "oldest"))
            .await;
        service
            .add_log(LogEntry::info(LogSource::App, "middle"))
            .await;
        service
            .add_log(LogEntry::info(LogSource::App, "newest"))
            .await;

        let logs = service.get_logs(Some(LogFilter::new().with_limit(2))).await;
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].message, "middle");
        assert_eq!(logs[1].message, "newest");
    }
}
