//! Minimal, dependency-free timestamp parsing for pacman logs and RSS feeds.

use crate::Timestamp;

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = i64::from(month);
    let d = i64::from(day);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`]: (year, month, day).
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn num(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn valid(year: i64, month: i64, day: i64, h: i64, mi: i64, s: i64) -> bool {
    (1970..=9999).contains(&year)
        && (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && (0..=23).contains(&h)
        && (0..=59).contains(&mi)
        && (0..=60).contains(&s)
}

/// Parses a numeric UTC offset `+HHMM`, `-HHMM` or `+HH:MM` into seconds.
fn offset_secs(s: &str) -> Option<i64> {
    let (sign, rest) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    let rest = rest.replace(':', "");
    if rest.len() != 4 {
        return None;
    }
    let h = num(&rest[..2])?;
    let m = num(&rest[2..])?;
    Some(sign * (h * 3600 + m * 60))
}

/// Parses pacman's log timestamp `2026-09-30T18:24:13+0200`.
pub fn parse_pacman(ts: &str) -> Option<Timestamp> {
    if ts.len() < 24 || ts.as_bytes().get(10) != Some(&b'T') {
        return None;
    }
    let year = num(ts.get(0..4)?)?;
    let month = num(ts.get(5..7)?)?;
    let day = num(ts.get(8..10)?)?;
    let hour = num(ts.get(11..13)?)?;
    let minute = num(ts.get(14..16)?)?;
    let second = num(ts.get(17..19)?)?;
    if !valid(year, month, day, hour, minute, second) {
        return None;
    }
    let offset = offset_secs(ts.get(19..)?)?;
    let days = days_from_civil(year, month as u32, day as u32);
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

fn month_index(name: &str) -> Option<i64> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let lower = name.to_ascii_lowercase();
    MONTHS
        .iter()
        .position(|m| lower.starts_with(m))
        .map(|i| i as i64 + 1)
}

/// Parses RFC 2822 dates as used by RSS (`Tue, 22 Sep 2026 09:09:27 +0000`,
/// `Sun, 09 Aug 2026 17:46:32 GMT`).
pub fn parse_rfc2822(value: &str) -> Option<Timestamp> {
    let value = value.trim();
    let value = match value.split_once(',') {
        Some((_, rest)) => rest.trim(),
        None => value,
    };
    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }
    let day = num(parts[0])?;
    let month = month_index(parts[1])?;
    let mut year = num(parts[2])?;
    if year < 100 {
        year += if year < 50 { 2000 } else { 1900 };
    }
    let time: Vec<&str> = parts[3].split(':').collect();
    if time.len() < 2 {
        return None;
    }
    let hour = num(time[0])?;
    let minute = num(time[1])?;
    let second = time.get(2).map_or(Some(0), |s| num(s))?;
    if !valid(year, month, day, hour, minute, second) {
        return None;
    }
    let offset = match parts.get(4).copied() {
        None | Some("GMT") | Some("UT") | Some("UTC") | Some("Z") => 0,
        Some("EST") => -5 * 3600,
        Some("EDT") => -4 * 3600,
        Some("CST") => -6 * 3600,
        Some("CDT") => -5 * 3600,
        Some("MST") => -7 * 3600,
        Some("MDT") => -6 * 3600,
        Some("PST") => -8 * 3600,
        Some("PDT") => -7 * 3600,
        Some(other) => offset_secs(other)?,
    };
    let days = days_from_civil(year, month as u32, day as u32);
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// Formats a timestamp as `YYYY-MM-DD HH:MM UTC` (diagnostic reports).
pub fn format_utc(ts: Timestamp) -> String {
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        secs / 3600,
        (secs % 3600) / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_roundtrip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        for d in [-1000, 0, 1, 11_017, 20_000, 30_000] {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
    }

    #[test]
    fn pacman_timestamps() {
        // 2026-09-30T16:24:13Z
        let ts = parse_pacman("2026-09-30T18:24:13+0200").unwrap();
        assert_eq!(format_utc(ts), "2026-09-30 16:24 UTC");
        assert_eq!(parse_pacman("2026-09-30T18:24:13+0000").unwrap() - ts, 7200);
        assert!(parse_pacman("2026-09-30 18:24").is_none());
        assert!(parse_pacman("2026-13-30T18:24:13+0200").is_none());
        assert!(parse_pacman("garbage").is_none());
    }

    #[test]
    fn rfc2822_dates() {
        let a = parse_rfc2822("Tue, 22 Sep 2026 09:09:27 +0000").unwrap();
        assert_eq!(format_utc(a), "2026-09-22 09:09 UTC");
        let b = parse_rfc2822("Sun, 09 Aug 2026 17:46:32 GMT").unwrap();
        assert_eq!(format_utc(b), "2026-08-09 17:46 UTC");
        let c = parse_rfc2822("22 Sep 2026 11:09:27 +0200").unwrap();
        assert_eq!(c, a);
        assert!(parse_rfc2822("yesterday").is_none());
        assert!(parse_rfc2822("Tue, 32 Sep 2026 09:09:27 +0000").is_none());
    }
}
