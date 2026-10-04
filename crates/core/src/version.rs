//! Version comparison of pacman packages, as `vercmp` does it.
//!
//! Follows libalpm's `alpm_pkg_vercmp`: `[epoch:]pkgver[-pkgrel]`, where a
//! missing epoch is `0` and the release only counts when both sides have one.
//! The parts are compared segment by segment with rpm's algorithm.

use std::cmp::Ordering;

/// Compares two package versions; `Greater` means `a` is newer.
pub fn vercmp(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (epoch_a, version_a, release_a) = split(a);
    let (epoch_b, version_b, release_b) = split(b);
    segments(epoch_a, epoch_b)
        .then_with(|| segments(version_a, version_b))
        .then_with(|| match (release_a, release_b) {
            (Some(x), Some(y)) => segments(x, y),
            _ => Ordering::Equal,
        })
}

/// Splits `[epoch:]version[-release]`.
fn split(evr: &str) -> (&str, &str, Option<&str>) {
    let digits = evr.bytes().take_while(u8::is_ascii_digit).count();
    let (epoch, rest) = if evr.as_bytes().get(digits) == Some(&b':') {
        (
            if digits == 0 { "0" } else { &evr[..digits] },
            &evr[digits + 1..],
        )
    } else {
        ("0", evr)
    };
    match rest.rsplit_once('-') {
        Some((version, release)) => (epoch, version, Some(release)),
        None => (epoch, rest, None),
    }
}

/// rpm's comparison: alternating numeric and alphabetic segments, separated by
/// any other characters. Numbers compare by value and beat letters; a trailing
/// letter segment (`1.0rc`) is older than the end of the string (`1.0`).
fn segments(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let at = |s: &[u8], i: usize| s.get(i).copied().unwrap_or(0);
    let separator = |s: &[u8], i: usize| {
        s[i.min(s.len())..]
            .iter()
            .take_while(|c| !c.is_ascii_alphanumeric())
            .count()
    };
    // `one`/`two` are the current positions, `end1`/`end2` the ends of the previous segments.
    let (mut one, mut two, mut end1, mut end2) = (0, 0, 0, 0);
    while at(a, one) != 0 && at(b, two) != 0 {
        one += separator(a, one);
        two += separator(b, two);
        if at(a, one) == 0 || at(b, two) == 0 {
            break;
        }
        // Different separator lengths decide (`2.0a` is older than `2.0.a`).
        if one - end1 != two - end2 {
            return (one - end1).cmp(&(two - end2));
        }
        let numeric = at(a, one).is_ascii_digit();
        let same_class = |c: u8| {
            if numeric {
                c.is_ascii_digit()
            } else {
                c.is_ascii_alphabetic()
            }
        };
        end1 = one + a[one..].iter().take_while(|&&c| same_class(c)).count();
        end2 = two + b[two..].iter().take_while(|&&c| same_class(c)).count();
        // Segments of different classes: numbers are newer than letters.
        if end2 == two {
            return if numeric {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        let (mut x, mut y) = (&a[one..end1], &b[two..end2]);
        if numeric {
            x = trim_zeros(x);
            y = trim_zeros(y);
            if x.len() != y.len() {
                return x.len().cmp(&y.len());
            }
        }
        if x != y {
            return x.cmp(y);
        }
        one = end1;
        two = end2;
    }
    let (c1, c2) = (at(a, one), at(b, two));
    if c1 == 0 && c2 == 0 {
        Ordering::Equal
    } else if (c1 == 0 && !c2.is_ascii_alphabetic()) || c1.is_ascii_alphabetic() {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

fn trim_zeros(digits: &[u8]) -> &[u8] {
    let zeros = digits.iter().take_while(|&&c| c == b'0').count();
    &digits[zeros..]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected results checked against pacman's `vercmp` (pacman 7.1).
    const CASES: &[(&str, &str, i8)] = &[
        ("1.5.0", "1.5.0", 0),
        ("1.5.1", "1.5.0", 1),
        ("1.5.1", "1.5", 1),
        ("1.10", "1.9", 1),
        ("1.002", "1.2", 0),
        ("1.5.0-1", "1.5.0-2", -1),
        ("1.5.0-2", "1.5.1-1", -1),
        ("1.5-1", "1.5", 0),
        ("1.1-1", "1.0", 1),
        ("1.5b-1", "1.5-1", -1),
        ("1.5b", "1.5.1", -1),
        ("1.0a", "1.0alpha", -1),
        ("1.0alpha", "1.0b", -1),
        ("1.0beta", "1.0rc", -1),
        ("1.0rc", "1.0", -1),
        ("1.0a", "1.0", -1),
        ("1.0.a", "1.0", 1),
        ("1.5.b", "1.5.a", 1),
        ("1.5.1", "1.5.b", 1),
        ("1.5.b-1", "1.5.b", 0),
        ("2.0", "2_0", 0),
        ("2.0_a", "2_0.a", 0),
        ("2.0a", "2.0.a", -1),
        ("2___a", "2_a", 1),
        ("1.0", "1.0.", -1),
        ("0:1.0", "1.0", 0),
        ("1:1.0", "2.0", 1),
        ("1:1.0", "2:0.5", -1),
        (":1.0", "1.0", 0),
        ("7.2.9-1", "7.2.8-2", 1),
        ("26.930.31730-1", "26.930.21537-1", 1),
        ("2.12.0-1.1", "2.11.3-1.1", 1),
        ("1.4.6-1.1", "1.4.6-1", 1),
        ("r1234.abcdef-1", "r999.fedcba-1", 1),
    ];

    #[test]
    fn matches_pacman() {
        for &(a, b, expected) in CASES {
            let expected = expected.cmp(&0);
            assert_eq!(vercmp(a, b), expected, "{a} vs {b}");
            assert_eq!(vercmp(b, a), expected.reverse(), "{b} vs {a}");
        }
    }
}
