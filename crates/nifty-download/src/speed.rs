//! How fast a download is going, averaged over its last moments so the number doesn't
//! jump with every chunk.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How far back the average reaches.
const WINDOW: Duration = Duration::from_secs(2);
const NANOS_PER_SECOND: u128 = 1_000_000_000;

#[derive(Debug, Default)]
pub(crate) struct SpeedMeter {
    /// When each count was taken, oldest first, with the bytes received by then.
    samples: VecDeque<(Instant, u64)>,
}

impl SpeedMeter {
    /// Notes that `received` bytes have arrived by `now`, and returns the bytes per second
    /// over the window up to it.
    pub(crate) fn record(&mut self, now: Instant, received: u64) -> u64 {
        self.samples.push_back((now, received));
        // Keep the newest sample older than the window, so the average spans all of it.
        while self
            .samples
            .get(1)
            .is_some_and(|&(taken, _)| now.duration_since(taken) >= WINDOW)
        {
            self.samples.pop_front();
        }
        let Some(&(since, from)) = self.samples.front() else {
            return 0;
        };
        let elapsed = now.duration_since(since).as_nanos();
        if elapsed == 0 {
            return 0;
        }
        let bytes = u128::from(received.saturating_sub(from));
        u64::try_from(bytes * NANOS_PER_SECOND / elapsed).unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages_over_the_last_two_seconds() {
        let start = Instant::now();
        let at = |millis| start + Duration::from_millis(millis);
        let mut meter = SpeedMeter::default();
        assert_eq!(meter.record(at(0), 0), 0, "one sample has no speed yet");
        assert_eq!(meter.record(at(500), 1_000), 2_000);
        assert_eq!(meter.record(at(1_000), 2_000), 2_000);
        assert_eq!(
            meter.record(at(3_000), 2_000),
            0,
            "nothing in the last two seconds"
        );
        assert_eq!(
            meter.record(at(3_250), 2_500),
            222,
            "500 bytes over 2.25 seconds"
        );
    }
}
