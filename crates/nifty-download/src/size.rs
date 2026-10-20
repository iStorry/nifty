//! `format_bytes(n)`: a size in bytes the way people read it.

/// Each unit is this many of the one before, as macOS and web browsers count, so a size
/// reads the same on every system.
const STEP: u64 = 1000;
const UNITS: &[&str] = &["KB", "MB", "GB", "TB", "PB", "EB"];
/// Tenths kept for sizes of a kilobyte or more.
const TENTHS: u64 = 10;

/// `bytes` as text: `"1 byte"` and `"999 bytes"`, then one decimal place in the largest
/// unit that keeps the number at least 1, such as `"1.5 MB"`.
#[must_use]
pub fn format_bytes(bytes: i64) -> String {
    let sign = if bytes < 0 { "-" } else { "" };
    let size = bytes.unsigned_abs();
    if size < STEP {
        let noun = if size == 1 { "byte" } else { "bytes" };
        return format!("{sign}{size} {noun}");
    }
    let mut divisor = STEP;
    let mut unit = 0;
    // Tenths of the unit, rounded to nearest; rounding may reach the next unit, as
    // 999,950 bytes does, so that is checked after rounding.
    let mut tenths = rounded_tenths(size, divisor);
    while tenths >= STEP * TENTHS && unit + 1 < UNITS.len() {
        divisor *= STEP;
        unit += 1;
        tenths = rounded_tenths(size, divisor);
    }
    format!(
        "{sign}{}.{} {}",
        tenths / TENTHS,
        tenths % TENTHS,
        UNITS[unit]
    )
}

fn rounded_tenths(size: u64, divisor: u64) -> u64 {
    let tenths =
        (u128::from(size) * u128::from(TENTHS) + u128::from(divisor) / 2) / u128::from(divisor);
    u64::try_from(tenths).expect("a size in tenths of a kilobyte or more fits a u64")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_bytes_then_larger_units() {
        assert_eq!(format_bytes(0), "0 bytes");
        assert_eq!(format_bytes(1), "1 byte");
        assert_eq!(format_bytes(999), "999 bytes");
        assert_eq!(format_bytes(1000), "1.0 KB");
        assert_eq!(format_bytes(1_540), "1.5 KB");
        assert_eq!(format_bytes(1_550), "1.6 KB");
        assert_eq!(format_bytes(15_200_000), "15.2 MB");
        assert_eq!(format_bytes(2_000_000_000), "2.0 GB");
    }

    #[test]
    fn rounding_up_moves_to_the_next_unit() {
        assert_eq!(format_bytes(999_949), "999.9 KB");
        assert_eq!(format_bytes(999_950), "1.0 MB");
    }

    #[test]
    fn handles_the_ends_of_the_int_range() {
        assert_eq!(format_bytes(-1_500), "-1.5 KB");
        assert_eq!(format_bytes(i64::MAX), "9.2 EB");
        assert_eq!(format_bytes(i64::MIN), "-9.2 EB");
    }
}
