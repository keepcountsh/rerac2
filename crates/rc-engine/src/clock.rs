//! The console's clock (`sceCdReadClock`) for the front end's random seed (crate::rac2): the host's local time as BCD.

/// `sceCdReadClock` (BCD) now, in the host's **local** time: the console's clock chip keeps JST and the game converts
/// it with the system configuration's time zone and summer time (0x12d6d8), so a save shows the player's own date;
/// the host's local time (its zone and daylight saving applied) is that. UTC only when the OS gives no local time
/// (an evening save in the Americas would otherwise show the next day's date).
pub fn clock_now() -> [u8; 8] {
    if let Some([y, mo, d, h, mi, s]) = local_time() { return bcd_clock_civil(y, mo, d, h, mi, s); }
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    bcd_clock(t)
}

/// The host's local date and time now: year, month, day, hour, minute, second.
#[cfg(windows)]
fn local_time() -> Option<[u64; 6]> {
    // `GetLocalTime` (kernel32): SYSTEMTIME = year, month, day of week, day, hour, minute, second, milliseconds.
    #[repr(C)]
    struct SystemTime([u16; 8]);
    #[link(name = "kernel32")]
    unsafe extern "system" { fn GetLocalTime(st: *mut SystemTime); }
    let mut st = SystemTime([0; 8]);
    // SAFETY: GetLocalTime only writes the SYSTEMTIME it is given.
    unsafe { GetLocalTime(&mut st) };
    let [y, mo, _, d, h, mi, s, _] = st.0;
    (y != 0).then(|| [y as u64, mo as u64, d as u64, h as u64, mi as u64, s as u64])
}

/// The host's local date and time now: year, month, day, hour, minute, second.
#[cfg(unix)]
fn local_time() -> Option<[u64; 6]> {
    // SAFETY: `time` and `localtime_r` only read / write the values they are given (the reentrant form).
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() { return None; }
        Some([(tm.tm_year + 1900) as u64, (tm.tm_mon + 1) as u64, tm.tm_mday as u64, tm.tm_hour as u64, tm.tm_min as u64, tm.tm_sec as u64])
    }
}

#[cfg(not(any(windows, unix)))]
fn local_time() -> Option<[u64; 6]> { None }

/// `sceCdCLOCK` (BCD) of a civil date and time: [0] stat 0, second, minute, hour, pad 0, day, month, year % 100.
fn bcd_clock_civil(year: u64, month: u64, day: u64, hour: u64, minute: u64, second: u64) -> [u8; 8] {
    let bcd = |v: u64| (((v / 10) % 10) << 4 | (v % 10)) as u8;
    [0, bcd(second), bcd(minute), bcd(hour), 0, bcd(day), bcd(month), bcd(year % 100)]
}

/// `sceCdCLOCK` (BCD) of a Unix time in seconds, in UTC.
fn bcd_clock(unix: u64) -> [u8; 8] {
    let days = unix / 86400;
    let secs = unix % 86400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    bcd_clock_civil(y as u64, m as u64, d as u64, secs / 3600, secs / 60 % 60, secs % 60)
}
