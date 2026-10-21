//! Trying a connection again after it fails in a way that may pass, waiting longer after
//! each failure in a row.

use std::time::Duration;

/// How long to wait before the first retry; each one after waits twice as long.
const FIRST_DELAY: Duration = Duration::from_secs(1);
/// The longest wait, so a download that keeps failing still tries now and then.
const LONGEST_DELAY: Duration = Duration::from_secs(30);

/// How many times in a row a connection is tried again when the call asks for `asked`;
/// any number below zero asks for none.
pub(crate) fn retry_count(asked: i64) -> u32 {
    u32::try_from(asked.max(0)).unwrap_or(u32::MAX)
}

/// How long to wait after `failures` failures in a row, the first being `1`.
pub(crate) fn delay(failures: u32) -> Duration {
    let doublings = failures.saturating_sub(1);
    FIRST_DELAY
        .saturating_mul(2_u32.saturating_pow(doublings))
        .min(LONGEST_DELAY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_twice_as_long_after_each_failure_up_to_a_limit() {
        let waits: Vec<_> = (1..=7).map(|failures| delay(failures).as_secs()).collect();
        assert_eq!(waits, [1, 2, 4, 8, 16, 30, 30]);
        assert_eq!(delay(u32::MAX), LONGEST_DELAY);
    }

    #[test]
    fn retries_at_least_zero_times() {
        assert_eq!(retry_count(-1), 0);
        assert_eq!(retry_count(3), 3);
        assert_eq!(retry_count(i64::MAX), u32::MAX);
    }
}
