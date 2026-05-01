//! ISO-8601 UTC timestamps without an external crate.
//!
//! GENESIS §10 schema stores `created_at` / `last_used_at` as ISO-8601
//! strings. Rather than pull in `chrono` / `time` / `jiff`, we hand-roll
//! formatting from `SystemTime` using Howard Hinnant's "days from civil"
//! algorithm — about 30 lines of arithmetic and well-tested below.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "Hinnant's epoch-to-civil-date algorithm is well-tested arithmetic with documented bounds; rewriting it with checked_*/from-tries would obscure intent without changing behavior"
)]

use std::time::{SystemTime, UNIX_EPOCH};

/// Format the current UTC time as an RFC-3339 / ISO-8601 string,
/// e.g. `"2026-04-30T16:42:00Z"`.
#[must_use]
pub fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    epoch_to_iso8601(secs)
}

/// Format a Unix epoch second count as an ISO-8601 string.
#[must_use]
pub fn epoch_to_iso8601(epoch_secs: i64) -> String {
    let (y, mo, d, h, mi, s) = epoch_to_ymdhms(epoch_secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Convert epoch seconds to `(year, month, day, hour, minute, second)` in
/// UTC. Uses Howard Hinnant's days-from-civil algorithm; valid for any
/// year representable in `i32`.
const fn epoch_to_ymdhms(epoch_secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = epoch_secs.div_euclid(86_400);
    let secs_in_day = epoch_secs.rem_euclid(86_400);
    let h = (secs_in_day / 3600) as u32;
    let mi = ((secs_in_day % 3600) / 60) as u32;
    let s = (secs_in_day % 60) as u32;

    // 1970-01-01 is day 719_468 in Hinnant's count.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32; // [0, 146_096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y_civil = yoe as i32 + (era as i32) * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y_civil + 1 } else { y_civil };

    (y, m, d, h, mi, s)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::{epoch_to_iso8601, now_iso8601};

    #[test]
    fn epoch_zero_is_unix_epoch() {
        assert_eq!(epoch_to_iso8601(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn one_second_after_epoch() {
        assert_eq!(epoch_to_iso8601(1), "1970-01-01T00:00:01Z");
    }

    #[test]
    fn known_dates() {
        // Verified against `date -u -r <epoch>` on a Unix system.
        assert_eq!(epoch_to_iso8601(86_400), "1970-01-02T00:00:00Z");
        assert_eq!(epoch_to_iso8601(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(epoch_to_iso8601(1_500_000_000), "2017-07-14T02:40:00Z");
        // Leap year boundary
        assert_eq!(epoch_to_iso8601(951_868_799), "2000-02-29T23:59:59Z");
        assert_eq!(epoch_to_iso8601(951_868_800), "2000-03-01T00:00:00Z");
    }

    #[test]
    fn now_is_well_formed() {
        let s = now_iso8601();
        assert_eq!(s.len(), 20);
        assert!(s.ends_with('Z'));
        assert_eq!(s.chars().nth(4), Some('-'));
        assert_eq!(s.chars().nth(7), Some('-'));
        assert_eq!(s.chars().nth(10), Some('T'));
        assert_eq!(s.chars().nth(13), Some(':'));
        assert_eq!(s.chars().nth(16), Some(':'));
    }

    #[test]
    fn pre_epoch_dates_work() {
        // 1969-12-31 23:59:59 UTC = -1 epoch
        assert_eq!(epoch_to_iso8601(-1), "1969-12-31T23:59:59Z");
        // Some date in 1900
        assert_eq!(epoch_to_iso8601(-2_208_988_800), "1900-01-01T00:00:00Z");
    }
}
