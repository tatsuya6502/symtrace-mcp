use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const MAX_LOG_AGE_DAYS: u64 = 7;

/// Initialize the global tracing subscriber for JSONL file logging.
///
/// Returns `Some(WorkerGuard)` if logging was initialized, or `None` if
/// logging is disabled (`SYMTRACE_LOG=off`). The guard MUST be held for
/// the lifetime of the process — dropping it flushes and closes the log file.
pub fn init_logging(
    cwd: &Path,
    config_level: Option<&str>,
) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let level_str = resolve_log_level(config_level);

    if level_str == "off" {
        return None;
    }

    let log_dir = resolve_log_dir(cwd);
    if let Err(e) = fs::create_dir_all(&log_dir) {
        // Can't log yet — fall back to stderr for this critical error
        eprintln!(
            "warning: failed to create log directory {}: {e}",
            log_dir.display()
        );
        return None;
    }

    cleanup_old_logs(&log_dir);

    let log_file = match create_log_file(&log_dir) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("warning: failed to create log file: {e}");
            return None;
        }
    };

    let (non_blocking, guard) = tracing_appender::non_blocking(log_file);

    let filter = parse_filter(&level_str);

    let subscriber = tracing_subscriber::Registry::default().with(filter);

    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(non_blocking)
        .with_target(true)
        .with_file(false)
        .with_line_number(false);

    subscriber.with(json_layer).init();

    Some(guard)
}

/// Determine log level from env var, config, or hardcoded default.
///
/// Priority: `SYMTRACE_LOG` env var > config file `level` > hardcoded `"info"`.
fn resolve_log_level(config_level: Option<&str>) -> String {
    if let Ok(env_val) = std::env::var("SYMTRACE_LOG")
        && !env_val.is_empty()
    {
        return env_val;
    }

    config_level
        .map(|s| s.to_string())
        .unwrap_or_else(|| "info".to_string())
}

/// Resolve log directory: `SYMTRACE_LOG_DIR` env var or `<cwd>/.symtrace/logs/`.
fn resolve_log_dir(cwd: &Path) -> PathBuf {
    if let Ok(dir) = std::env::var("SYMTRACE_LOG_DIR") {
        return PathBuf::from(dir);
    }
    cwd.join(".symtrace").join("logs")
}

/// Create a per-invocation log file named `symtrace-mcp.YYYY-MM-DD_HHmmss.PID.log`.
fn create_log_file(log_dir: &Path) -> std::io::Result<std::fs::File> {
    let timestamp = format_utc_timestamp();
    let pid = std::process::id();
    let filename = format!("symtrace-mcp.{timestamp}.{pid}.log");

    fs::File::create(log_dir.join(filename))
}

/// Delete log files older than `MAX_LOG_AGE_DAYS`.
fn cleanup_old_logs(log_dir: &Path) {
    let cutoff = SystemTime::now() - Duration::from_secs(MAX_LOG_AGE_DAYS * 24 * 60 * 60);

    let Ok(entries) = fs::read_dir(log_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_symtrace_log_file(&path) {
            continue;
        }

        if let Ok(metadata) = fs::metadata(&path)
            && let Ok(modified) = metadata.modified()
            && modified < cutoff
        {
            let _ = fs::remove_file(&path);
        }
    }
}

/// Check if a path matches the log file naming pattern.
fn is_symtrace_log_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.starts_with("symtrace-mcp.") && name.ends_with(".log")
}

/// Parse the level string into a tracing filter.
///
/// Accepts simple level names (`off`, `error`, `warn`, `info`, `debug`, `trace`)
/// or `tracing-subscriber::filter::Targets` syntax
/// (e.g., `symtrace_mcp=debug,turso=warn`).
fn parse_filter(level_str: &str) -> tracing_subscriber::filter::Targets {
    use std::str::FromStr;

    // Try as a simple level name first (e.g., "debug", "info", "warn")
    if let Ok(level) = tracing::level_filters::LevelFilter::from_str(level_str) {
        return tracing_subscriber::filter::Targets::new().with_default(level);
    }

    // Try as Targets filter syntax (e.g., "symtrace_mcp=debug,turso=warn")
    if let Ok(filter) = tracing_subscriber::filter::Targets::from_str(level_str) {
        return filter;
    }

    // Fall back to info if parsing fails
    tracing_subscriber::filter::Targets::new().with_default(tracing::level_filters::LevelFilter::INFO)
}

/// Format current UTC time as `YYYY-MM-DD_HHmmss` without external crates.
fn format_utc_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let days = (secs / 86400) as i32;
    let time_secs = (secs % 86400) as u32;
    let hour = time_secs / 3600;
    let min = (time_secs % 3600) / 60;
    let sec = time_secs % 60;

    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02}_{:02}{:02}{:02}",
        year, month, day, hour, min, sec
    )
}

/// Convert days since Unix epoch to `(year, month, day)`.
///
/// Uses the civil calendar algorithm from
/// <http://howardhinnant.github.io/date_algorithms.html#civil_from_days>.
fn days_to_ymd(mut days: i32) -> (i32, u32, u32) {
    days += 719468; // days from 0000-03-01 to 1970-01-01

    let era = if days >= 0 {
        days / 146097
    } else {
        (days - 146096) / 146097
    };
    let doe = days - era * 146097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month index [0, 11] from March
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };

    (y, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_symtrace_log_file() {
        assert!(is_symtrace_log_file(Path::new(
            "symtrace-mcp.2026-06-07_100500.12345.log"
        )));
        assert!(is_symtrace_log_file(Path::new(
            "symtrace-mcp.2025-01-01_000000.1.log"
        )));
        assert!(!is_symtrace_log_file(Path::new("other.log")));
        assert!(!is_symtrace_log_file(Path::new("symtrace-mcp.txt")));
        assert!(!is_symtrace_log_file(Path::new("stats.db")));
    }

    #[test]
    fn test_days_to_ymd() {
        // 1970-01-01 = day 0
        assert_eq!(days_to_ymd(0), (1970, 1, 1));
        // 2000-01-01 = day 10957
        assert_eq!(days_to_ymd(10957), (2000, 1, 1));
        // 2026-06-07 — verify it's plausible
        let (y, m, d) = days_to_ymd(20573);
        assert_eq!(y, 2026);
        assert!((1..=12).contains(&m));
        assert!((1..=31).contains(&d));
    }

    #[test]
    fn test_resolve_log_level_default() {
        let result = resolve_log_level(None);
        assert_eq!(result, "info");
    }

    #[test]
    fn test_resolve_log_level_config() {
        let result = resolve_log_level(Some("debug"));
        assert_eq!(result, "debug");
    }

    #[test]
    fn test_parse_filter_simple_levels() {
        let filter = parse_filter("info");
        assert!(filter.would_enable("symtrace_mcp", &tracing::Level::INFO));
        assert!(!filter.would_enable("symtrace_mcp", &tracing::Level::DEBUG));

        let filter = parse_filter("debug");
        assert!(filter.would_enable("symtrace_mcp", &tracing::Level::DEBUG));
    }

    #[test]
    fn test_parse_filter_valid_targets_syntax() {
        // Targets syntax: specific module levels
        let filter = parse_filter("symtrace_mcp=debug");
        assert!(filter.would_enable("symtrace_mcp", &tracing::Level::DEBUG));
    }
}
