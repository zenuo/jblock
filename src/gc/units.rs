//! Size, duration and timestamp decoding shared by the GC log parsers.

use std::cmp::Ordering;

use regex::Regex;

use super::model::MemChange;

/// Parse a HotSpot size token (`1132814K`, `24.0M`, `0.0B`, `2G`) into KiB.
pub fn parse_size_kb(token: &str) -> Option<f64> {
    let token = token.trim();
    let (num, unit) = token.split_at(token.len().checked_sub(1)?);
    let value: f64 = num.parse().ok()?;
    let factor = match unit {
        "B" => 1.0 / 1024.0,
        "K" => 1.0,
        "M" => 1024.0,
        "G" => 1024.0 * 1024.0,
        "T" => 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some(value * factor)
}

/// Matches `A->B(C)`, `A(Ca)->B(Cb)` and `A->B` size transitions.
pub fn mem_change_regex() -> Regex {
    Regex::new(
        r"(?P<b>\d+(?:\.\d+)?[BKMGT])(?:\((?P<cb>\d+(?:\.\d+)?[BKMGT])\))?->(?P<a>\d+(?:\.\d+)?[BKMGT])(?:\((?P<ca>\d+(?:\.\d+)?[BKMGT])\))?",
    )
    .unwrap()
}

/// Decode the first size transition in `text` using a regex from [`mem_change_regex`].
pub fn parse_mem_change(re: &Regex, text: &str) -> Option<MemChange> {
    let caps = re.captures(text)?;
    Some(MemChange {
        before_kb: parse_size_kb(caps.name("b")?.as_str())?,
        after_kb: parse_size_kb(caps.name("a")?.as_str())?,
        capacity_before_kb: caps.name("cb").and_then(|m| parse_size_kb(m.as_str())),
        capacity_after_kb: caps.name("ca").and_then(|m| parse_size_kb(m.as_str())),
    })
}

/// Parse a decimal number that may use `,` as the decimal separator (locale-dependent JVM output).
pub fn parse_decimal(text: &str) -> Option<f64> {
    if text.contains(',') && !text.contains('.') {
        text.replace(',', ".").parse().ok()
    } else {
        text.parse().ok()
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Parse `2026-11-13T16:22:15.343+0800` (also `Z`, `+08:00`, no fraction) into epoch milliseconds.
pub fn parse_iso_epoch_ms(text: &str) -> Option<f64> {
    let b = text.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> { text.get(r)?.parse().ok() };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, s) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let mut rest = &text[19..];
    let mut millis = 0.0;
    if let Some(frac) = rest.strip_prefix('.') {
        let digits: String = frac.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        millis = format!("0.{digits}").parse::<f64>().ok()? * 1000.0;
        rest = &frac[digits.len()..];
    }
    let offset_min = match rest {
        "" | "Z" => 0,
        _ => {
            let sign = match rest.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let digits: String = rest[1..].chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() != 4 {
                return None;
            }
            let oh: i64 = digits[0..2].parse().ok()?;
            let om: i64 = digits[2..4].parse().ok()?;
            sign * (oh * 60 + om)
        }
    };
    let days = days_from_civil(y, mo, d);
    let secs = days * 86_400 + h * 3600 + mi * 60 + s - offset_min * 60;
    Some(secs as f64 * 1000.0 + millis)
}

/// Stable insertion sort for the small collections in the GC report (causes, kinds,
/// findings, file groups). Each `sort_by` closure instantiates a full driftsort in the
/// WASM binary; this keeps those call sites to a few hundred bytes each.
pub fn small_sort_by<T>(v: &mut [T], mut cmp: impl FnMut(&T, &T) -> Ordering) {
    for i in 1..v.len() {
        let mut j = i;
        while j > 0 && cmp(&v[j - 1], &v[j]) == Ordering::Greater {
            v.swap(j - 1, j);
            j -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_decode_to_kib() {
        assert_eq!(parse_size_kb("1132814K"), Some(1_132_814.0));
        assert_eq!(parse_size_kb("24.0M"), Some(24.0 * 1024.0));
        assert_eq!(parse_size_kb("0.0B"), Some(0.0));
        assert_eq!(parse_size_kb("2G"), Some(2.0 * 1024.0 * 1024.0));
        assert_eq!(parse_size_kb("12X"), None);
        assert_eq!(parse_size_kb(""), None);
    }

    #[test]
    fn mem_change_variants() {
        let re = mem_change_regex();
        let c = parse_mem_change(&re, "29M->1M(64M) 0.335ms").unwrap();
        assert_eq!((c.before_kb, c.after_kb), (29.0 * 1024.0, 1024.0));
        assert_eq!(c.capacity_before_kb, None);
        assert_eq!(c.capacity_after_kb, Some(64.0 * 1024.0));

        let c = parse_mem_change(&re, "PSYoungGen: 20995K(28672K)->4083K(28672K) Eden: x").unwrap();
        assert_eq!(c.capacity_before_kb, Some(28672.0));
        assert_eq!(c.after_kb, 4083.0);

        let c = parse_mem_change(&re, "Old regions: 2->2").is_none();
        assert!(c, "region counts have no unit and are handled separately");
    }

    #[test]
    fn iso_timestamps_with_offsets() {
        // 2026-11-13T08:22:15.343Z
        let utc = parse_iso_epoch_ms("2026-11-13T08:22:15.343+0000").unwrap();
        let local = parse_iso_epoch_ms("2026-11-13T16:22:15.343+0800").unwrap();
        assert_eq!(utc, local);
        assert_eq!(parse_iso_epoch_ms("1970-01-01T00:00:01Z"), Some(1000.0));
        assert_eq!(parse_iso_epoch_ms("1970-01-01T08:00:00+08:00"), Some(0.0));
        assert_eq!(parse_iso_epoch_ms("not a time"), None);
    }

    #[test]
    fn small_sort_is_stable_and_ordered() {
        let mut v = vec![(3, 'a'), (1, 'b'), (3, 'c'), (2, 'd'), (1, 'e')];
        small_sort_by(&mut v, |a, b| a.0.cmp(&b.0));
        assert_eq!(v, vec![(1, 'b'), (1, 'e'), (2, 'd'), (3, 'a'), (3, 'c')]);
        let mut empty: Vec<u8> = Vec::new();
        small_sort_by(&mut empty, |a, b| a.cmp(b));
    }

    #[test]
    fn decimal_comma_locale() {
        assert_eq!(parse_decimal("0,017"), Some(0.017));
        assert_eq!(parse_decimal("12.5"), Some(12.5));
    }
}
