//! Windows integration: data folders, process CPU/RAM statistics, recycle-bin deletion.

use std::path::{Path, PathBuf};
use std::time::Instant;

/// `%APPDATA%\WolfCraft` (falls back to `./WolfCraftData`).
pub fn default_data_dir() -> PathBuf {
    match std::env::var_os("APPDATA") {
        Some(p) => PathBuf::from(p).join("WolfCraft"),
        None => PathBuf::from("WolfCraftData"),
    }
}

/// Directory containing the running executable.
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Resident memory (working set) of this process in bytes.
#[cfg(windows)]
pub fn process_memory_bytes() -> u64 {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut c: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) != 0 { c.WorkingSetSize as u64 } else { 0 }
    }
}
#[cfg(not(windows))]
pub fn process_memory_bytes() -> u64 {
    0
}

/// Total CPU time (user + kernel) consumed by this process, in seconds.
#[cfg(windows)]
fn process_cpu_seconds() -> f64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    unsafe {
        let mut c: FILETIME = std::mem::zeroed();
        let mut e = c;
        let mut k = c;
        let mut u = c;
        if GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) == 0 {
            return 0.0;
        }
        let ft = |f: FILETIME| ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) as f64 * 1e-7;
        ft(k) + ft(u)
    }
}
#[cfg(not(windows))]
fn process_cpu_seconds() -> f64 {
    0.0
}

/// Measures process CPU usage (% of all logical cores) between samples.
pub struct CpuMeter {
    last_cpu: f64,
    last_wall: Instant,
    cores: f64,
    value: f32,
}

impl Default for CpuMeter {
    fn default() -> Self {
        Self {
            last_cpu: process_cpu_seconds(),
            last_wall: Instant::now(),
            cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64,
            value: 0.0,
        }
    }
}

impl CpuMeter {
    /// Samples at most twice per second; returns the latest value.
    pub fn sample(&mut self) -> f32 {
        let wall = self.last_wall.elapsed().as_secs_f64();
        if wall >= 0.5 {
            let cpu = process_cpu_seconds();
            self.value = (((cpu - self.last_cpu) / wall / self.cores) * 100.0) as f32;
            self.last_cpu = cpu;
            self.last_wall = Instant::now();
        }
        self.value
    }
    pub fn cores(&self) -> usize {
        self.cores as usize
    }
}

/// Moves a file or folder to the Windows Recycle Bin (so deletion can be undone by the user).
#[cfg(windows)]
pub fn move_to_recycle_bin(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::{
        FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW, SHFileOperationW,
    };
    let abs = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    // canonicalize returns a \\?\ prefixed path, which the shell API does not accept.
    let s = abs.to_string_lossy().trim_start_matches(r"\\?\").to_string();
    let mut wide: Vec<u16> = std::ffi::OsStr::new(&s).encode_wide().collect();
    wide.push(0);
    wide.push(0); // double-null terminated list
    let mut op = SHFILEOPSTRUCTW {
        hwnd: std::ptr::null_mut(),
        wFunc: FO_DELETE,
        pFrom: wide.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI) as u16,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    let r = unsafe { SHFileOperationW(&mut op) };
    if r == 0 && op.fAnyOperationsAborted == 0 { Ok(()) } else { Err(format!("SHFileOperation failed: code {r}")) }
}
#[cfg(not(windows))]
pub fn move_to_recycle_bin(path: &Path) -> Result<(), String> {
    std::fs::remove_dir_all(path).map_err(|e| e.to_string())
}

/// Shows a modal error box (used when the game cannot even open a window).
#[cfg(windows)]
pub fn message_box(title: &str, text: &str) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    let wide = |s: &str| -> Vec<u16> { std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect() };
    let (t, m) = (wide(title), wide(text));
    unsafe {
        MessageBoxW(std::ptr::null_mut(), m.as_ptr(), t.as_ptr(), MB_OK | MB_ICONERROR);
    }
}
#[cfg(not(windows))]
pub fn message_box(title: &str, text: &str) {
    eprintln!("{title}: {text}");
}

/// Seconds since the Unix epoch.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Formats a Unix timestamp as `YYYY-MM-DD HH:MM` (UTC).
pub fn format_timestamp(ts: u64) -> String {
    let days = (ts / 86_400) as i64;
    let rem = ts % 86_400;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rem / 3600, (rem % 3600) / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_format() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00");
        assert_eq!(format_timestamp(1_700_000_000), "2023-11-14 22:13");
    }

    #[test]
    fn memory_is_reported() {
        if cfg!(windows) {
            assert!(process_memory_bytes() > 0);
        }
    }
}
