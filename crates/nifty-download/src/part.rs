//! The parts of a file a download fetches, each over its own connection: the whole file
//! for one connection, or for several, stretches that end where the next begins.

use std::fs::File;
use std::sync::Arc;

use crate::connection::{ConnectionId, Outlet};
use crate::http::ByteRange;

/// The most connections one download opens, so a large `connections:` can't swamp a
/// server.
pub(crate) const MAX_CONNECTIONS: usize = 16;
/// The smallest part worth a connection of its own.
pub(crate) const MIN_PART_SIZE: u64 = 64 * 1024;

/// How many connections a download opens when its call asks for `asked`: at least one,
/// and at most [`MAX_CONNECTIONS`].
pub(crate) fn connection_count(asked: i64) -> usize {
    usize::try_from(asked).map_or(1, |asked| asked.clamp(1, MAX_CONNECTIONS))
}

/// A stretch of the file, and how much of it is saved.
#[derive(Debug)]
pub(crate) struct Part {
    pub start: u64,
    /// Where the next part starts; `None` for a part that runs to the end of the file.
    pub end: Option<u64>,
    /// The bytes saved from `start` on.
    pub saved: u64,
    /// The connection fetching it, unless it is paused or done.
    pub connection: Option<Live>,
}

/// A connection fetching a part.
#[derive(Debug)]
pub(crate) struct Live {
    pub id: ConnectionId,
    pub outlet: Arc<Outlet>,
    /// Where in the file the outlet's first byte goes, once the server has answered.
    pub writes_from: Option<u64>,
}

impl Part {
    /// The whole file, as one connection fetches it.
    pub(crate) fn whole() -> Self {
        Self {
            start: 0,
            end: None,
            saved: 0,
            connection: None,
        }
    }

    /// Where in the file its next byte goes.
    pub(crate) fn position(&self) -> u64 {
        self.start + self.saved
    }

    /// What is left of it to ask for.
    pub(crate) fn rest(&self) -> ByteRange {
        ByteRange {
            from: self.position(),
            to: self.end,
        }
    }

    /// How many more bytes it takes, when it ends before the file does.
    pub(crate) fn room(&self) -> Option<u64> {
        self.end.map(|end| end - self.position())
    }

    /// Whether all of it is saved, given the whole file's length when known.
    pub(crate) fn is_complete(&self, total: Option<u64>) -> bool {
        match self.end {
            Some(end) => self.position() == end,
            None => total == Some(self.position()),
        }
    }

    /// Stops its connection, counting everything it wrote, and gives the file it was
    /// writing, if the server had answered.
    pub(crate) fn hang_up(&mut self) -> Option<File> {
        let live = self.connection.take()?;
        let closed = live.outlet.close();
        if let Some(from) = live.writes_from {
            self.saved = from + closed.written - self.start;
        }
        closed.file
    }
}

/// A file of `total` bytes split into at most `connections` parts of nearly equal size,
/// none smaller than [`MIN_PART_SIZE`] unless the whole file is.
pub(crate) fn split(total: u64, connections: usize) -> Vec<Part> {
    let most = u64::try_from(connections).unwrap_or(u64::MAX);
    let count = (total / MIN_PART_SIZE).clamp(1, most);
    let boundary = |index: u64| {
        let at = u128::from(total) * u128::from(index) / u128::from(count);
        u64::try_from(at).expect("a boundary is at most the total")
    };
    (0..count)
        .map(|index| Part {
            start: boundary(index),
            end: Some(boundary(index + 1)),
            saved: 0,
            connection: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(parts: &[Part]) -> Vec<(u64, Option<u64>)> {
        parts.iter().map(|part| (part.start, part.end)).collect()
    }

    #[test]
    fn splits_into_nearly_equal_parts_that_meet() {
        let total = 4 * MIN_PART_SIZE + 3;
        let parts = split(total, 4);
        assert_eq!(
            bounds(&parts),
            [
                (0, Some(MIN_PART_SIZE)),
                (MIN_PART_SIZE, Some(2 * MIN_PART_SIZE + 1)),
                (2 * MIN_PART_SIZE + 1, Some(3 * MIN_PART_SIZE + 2)),
                (3 * MIN_PART_SIZE + 2, Some(total)),
            ]
        );
    }

    #[test]
    fn keeps_parts_large_enough_to_be_worth_a_connection() {
        assert_eq!(bounds(&split(100, 8)), [(0, Some(100))]);
        assert_eq!(split(3 * MIN_PART_SIZE, 8).len(), 3);
        assert_eq!(bounds(&split(0, 4)), [(0, Some(0))]);
    }

    #[test]
    fn opens_between_one_and_the_most_connections() {
        assert_eq!(connection_count(-3), 1);
        assert_eq!(connection_count(0), 1);
        assert_eq!(connection_count(4), 4);
        assert_eq!(connection_count(1_000), MAX_CONNECTIONS);
    }

    #[test]
    fn a_part_is_complete_at_its_end_or_the_files() {
        let mut part = split(10, 1).remove(0);
        part.saved = 10;
        assert!(part.is_complete(None));
        let mut whole = Part::whole();
        whole.saved = 10;
        assert!(!whole.is_complete(None), "a file of unknown size");
        assert!(whole.is_complete(Some(10)));
        assert_eq!(whole.rest(), ByteRange { from: 10, to: None });
    }
}
