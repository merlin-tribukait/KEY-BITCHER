use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_MAX_BYTES: u64 = 5 * 1024 * 1024;

struct LoggerState {
    debug_enabled: bool,
    file: std::fs::File,
}

static LOGGER: Mutex<Option<LoggerState>> = Mutex::new(None);

/// Rotates `<log>` to `<log>.1` (overwriting the old backup) when the current
/// file exceeds `max_bytes`. Runs before the new file is opened.
fn rotate_if_needed(log_path: &str, max_bytes: u64) {
    let Ok(meta) = std::fs::metadata(log_path) else {
        return;
    };
    if meta.len() <= max_bytes {
        return;
    }
    let backup = format!("{}.1", log_path);
    let _ = std::fs::remove_file(&backup);
    if std::fs::rename(log_path, &backup).is_ok() {
        eprintln!("[INFO] log rotated: {} -> {}", log_path, backup);
    }
}

/// Opens the log file in append mode (previous runs are never deleted, but the
/// file is rotated to `<log>.1` once it grows past `max_bytes`) and enables or
/// disables verbose debug output. Safe to call once at startup.
pub fn init(log_path: &str, debug: bool, max_bytes: Option<u64>) -> anyhow::Result<()> {
    rotate_if_needed(log_path, max_bytes.unwrap_or(DEFAULT_MAX_BYTES));
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    {
        let mut guard = LOGGER.lock().unwrap();
        *guard = Some(LoggerState {
            debug_enabled: debug,
            file,
        });
    }
    log_line(
        "INFO",
        &format!("logging initialized (debug={}, file={})", debug, log_path),
    );
    Ok(())
}

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Writes a level-tagged line to the append-only log file and mirrors it to
/// the console. Debug lines only reach the console when --debug is enabled,
/// but are always recorded in the log file.
pub fn log_line(level: &str, msg: &str) {
    let mut guard = LOGGER.lock().unwrap();
    let Some(state) = guard.as_mut() else {
        if level == "ERROR" || level == "WARN" {
            eprintln!("[{level}] {msg}");
        }
        return;
    };

    let line = format!("[{}] [{}] {}", now_ms(), level, msg);
    let _ = writeln!(state.file, "{}", line);

    let echo = match level {
        "DEBUG" => state.debug_enabled,
        _ => true,
    };
    if echo {
        if level == "ERROR" || level == "WARN" {
            eprintln!("{}", line);
        } else {
            println!("{}", line);
        }
    }
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::logging::log_line("DEBUG", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logging::log_line("INFO", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::logging::log_line("WARN", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::logging::log_line("ERROR", &format!($($arg)*))
    };
}
