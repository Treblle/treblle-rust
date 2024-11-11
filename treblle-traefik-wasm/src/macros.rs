/// Helper macro for debug logging
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        crate::host_functions::host_log(-1, &format!($($arg)*))
    };
}

/// Helper macro for info logging
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        crate::host_functions::host_log(0, &format!($($arg)*))
    };
}

/// Helper macro for warning logging
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        crate::host_functions::host_log(1, &format!($($arg)*))
    };
}

/// Helper macro for error logging
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        crate::host_functions::host_log(2, &format!($($arg)*))
    };
}
