//! The ids that name downloads to code, such as `download-3`.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// What every id starts with, so an id reads as one and not as a count.
const PREFIX: &str = "download-";

/// Ids are counted for the whole process, so after `nf dev` reloads a program, a new
/// download never gets the id of one from before, which state may still hold.
static NEXT: AtomicU64 = AtomicU64::new(1);

/// A download's id, unique for as long as the process runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DownloadId(u64);

impl DownloadId {
    /// An id no download has had yet.
    #[must_use]
    pub fn next() -> Self {
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    /// The id `text` shows, if it is one.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        text.strip_prefix(PREFIX)?.parse().ok().map(Self)
    }
}

impl fmt::Display for DownloadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{PREFIX}{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_back_the_ids_it_shows() {
        let id = DownloadId::next();
        assert_ne!(DownloadId::next(), id);
        assert_eq!(DownloadId::parse(&id.to_string()), Some(id));
        assert_eq!(DownloadId::parse("download-x"), None);
        assert_eq!(DownloadId::parse("7"), None);
        assert_eq!(DownloadId::parse(""), None);
    }
}
