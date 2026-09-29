#[cfg(unix)]
#[repr(C)]
struct Tm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
    tm_gmtoff: i64,
    tm_zone: *const u8,
}

#[cfg(unix)]
extern "C" {
    fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
}

pub fn days_to_civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn local_parts() -> [i64; 9] {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs() as i64;
    let ms = now.subsec_millis() as i64;
    #[cfg(unix)]
    unsafe {
        let mut tm: Tm = std::mem::zeroed();
        if !localtime_r(&secs, &mut tm).is_null() {
            return [
                tm.tm_year as i64 + 1900,
                tm.tm_mon as i64 + 1,
                tm.tm_mday as i64,
                tm.tm_hour as i64,
                tm.tm_min as i64,
                tm.tm_sec as i64,
                ms,
                tm.tm_wday as i64,
                tm.tm_gmtoff,
            ];
        }
    }
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, m, d) = days_to_civil(days);
    [y, m, d, rem / 3600, (rem % 3600) / 60, rem % 60, ms, (days + 4).rem_euclid(7), 0]
}
