use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI32, Ordering};

use crate::constants::log_level;
use crate::host_functions::host_log;

static LOG_LEVEL: AtomicI32 = AtomicI32::new(log_level::INFO);

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    #[default]
    None,
}

impl LogLevel {
    pub fn as_i32(self) -> i32 {
        match self {
            LogLevel::Debug => log_level::DEBUG,
            LogLevel::Info => log_level::INFO,
            LogLevel::Warn => log_level::WARN,
            LogLevel::Error => log_level::ERROR,
            LogLevel::None => log_level::NONE,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "debug" => LogLevel::Debug,
            "info" => LogLevel::Info,
            "warn" | "warning" => LogLevel::Warn,
            "error" => LogLevel::Error,
            "none" | _ => LogLevel::None,
        }
    }
}

/// Initialize the logger with the configured log level
pub fn init(level: LogLevel) {
    LOG_LEVEL.store(level.as_i32(), Ordering::Relaxed);
    log(LogLevel::Debug, &format!("Log level set to: {:?}", crate::CONFIG.log_level));
}

/// Log a message with the specified level
pub fn log(level: LogLevel, message: &str) {
    if should_log(level) {
        host_log(level.as_i32(), message);
    }
}

/// Check if a message should be logged at the specified level
fn should_log(level: LogLevel) -> bool {
    level.as_i32() >= LOG_LEVEL.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_conversion() {
        assert_eq!(LogLevel::Debug.as_i32(), log_level::DEBUG);
        assert_eq!(LogLevel::Info.as_i32(), log_level::INFO);
        assert_eq!(LogLevel::Warn.as_i32(), log_level::WARN);
        assert_eq!(LogLevel::Error.as_i32(), log_level::ERROR);
        assert_eq!(LogLevel::None.as_i32(), log_level::NONE);
    }

    #[test]
    fn test_log_level_from_str() {
        assert_eq!(LogLevel::from_str("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::from_str("INFO"), LogLevel::Info);
        assert_eq!(LogLevel::from_str("Warning"), LogLevel::Warn);
        assert_eq!(LogLevel::from_str("ERROR"), LogLevel::Error);
        assert_eq!(LogLevel::from_str("none"), LogLevel::None);
        assert_eq!(LogLevel::from_str("invalid"), LogLevel::None);
    }
}
