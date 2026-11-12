//! Minimal logger writing to stderr and to a log file.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

struct Logger {
    file: Mutex<Option<File>>,
    level: log::LevelFilter,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        // Silence chatty graphics crates below warn level.
        let t = metadata.target();
        if (t.starts_with("wgpu") || t.starts_with("naga")) && metadata.level() > log::Level::Warn {
            return false;
        }
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let line = format!("[{} {:>5} {}] {}\n", secs, record.level(), record.target(), record.args());
        eprint!("{line}");
        if let Ok(mut f) = self.file.lock() {
            if let Some(f) = f.as_mut() {
                let _ = f.write_all(line.as_bytes());
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut f) = self.file.lock() {
            if let Some(f) = f.as_mut() {
                let _ = f.flush();
            }
        }
    }
}

/// Initializes global logging. Safe to call once; later calls are ignored.
pub fn init(log_file: &Path, level: log::LevelFilter) {
    if let Some(dir) = log_file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = OpenOptions::new().create(true).write(true).truncate(true).open(log_file).ok();
    let logger = Box::leak(Box::new(Logger { file: Mutex::new(file), level }));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(level);
    }
    let path = log_file.to_path_buf();
    let prev = std::panic::take_hook();
    // crash.txt survives the next start (latest.log is truncated), so the game can show it.
    let crash = log_file.with_file_name("crash.txt");
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        let bt = std::backtrace::Backtrace::force_capture();
        log::error!("PANIC in thread '{thread}': {info}\n{bt}");
        if let Ok(mut f) = OpenOptions::new().append(true).open(&path) {
            let _ = writeln!(f, "PANIC in thread '{thread}': {info}\n{bt}");
        }
        let _ = std::fs::write(&crash, format!("Поток '{thread}': {info}\n\n{bt}"));
        prev(info);
    }));
}
