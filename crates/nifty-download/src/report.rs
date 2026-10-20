//! What a download tells the app, and what each report writes into the state it bound.

use std::path::PathBuf;

use nifty_runtime::DownloadField;

use crate::id::DownloadId;

/// What `status` shows from the click until the download finishes.
const DOWNLOADING: &str = "Downloading…";
/// What `status` shows while the download is paused.
const PAUSED: &str = "Paused";
/// What `status` shows once code cancels the download.
const CANCELLED: &str = "Cancelled";
const DONE_PERCENT: i64 = 100;

/// How far a download has got, sent to the UI thread.
#[derive(Debug, PartialEq, Eq)]
pub enum Report {
    /// The call was just made, giving the download this id: everything starts from
    /// nothing.
    Started(DownloadId),
    Progress(Progress),
    /// Code paused it, with this much saved.
    Paused(Progress),
    /// Code resumed it, and it is downloading again.
    Resumed,
    /// The file is saved at `path`, `size` bytes long.
    Saved {
        path: PathBuf,
        size: u64,
    },
    Failed(String),
    /// Code cancelled it, and what it had saved is removed.
    Cancelled,
}

/// The bytes received so far, of `total` when the server said, at `speed` bytes a second.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub received: u64,
    pub total: Option<u64>,
    pub speed: u64,
}

/// A value a report writes: a whole number or text, as its field holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Written {
    Int(i64),
    Text(String),
}

impl Report {
    /// The fields this report changes, with their new values. A field the call didn't
    /// bind is skipped by whoever applies them.
    #[must_use]
    pub fn writes(&self) -> Vec<(DownloadField, Written)> {
        use DownloadField::{Id, Progress as Percent, Received, Speed, Status, Total};
        let int = |field, value: u64| (field, Written::Int(saturating_int(value)));
        match self {
            Self::Started(id) => vec![
                (Id, Written::Text(id.to_string())),
                (Percent, Written::Int(0)),
                (Status, Written::Text(DOWNLOADING.to_owned())),
                int(Received, 0),
                int(Total, 0),
                int(Speed, 0),
            ],
            Self::Paused(progress) => {
                let mut writes = Self::Progress(*progress).writes();
                writes.push((Status, Written::Text(PAUSED.to_owned())));
                writes
            }
            Self::Resumed => vec![(Status, Written::Text(DOWNLOADING.to_owned()))],
            Self::Progress(progress) => {
                let mut writes = vec![int(Received, progress.received), int(Speed, progress.speed)];
                if let Some(total) = progress.total {
                    writes.push((Percent, Written::Int(percent(progress.received, total))));
                    writes.push(int(Total, total));
                }
                writes
            }
            Self::Saved { path, size } => vec![
                (Percent, Written::Int(DONE_PERCENT)),
                (
                    Status,
                    Written::Text(format!("Saved to {}", path.display())),
                ),
                int(Received, *size),
                int(Total, *size),
                int(Speed, 0),
            ],
            Self::Failed(reason) => vec![
                (Status, Written::Text(format!("Failed: {reason}"))),
                int(Speed, 0),
            ],
            Self::Cancelled => vec![(Status, Written::Text(CANCELLED.to_owned())), int(Speed, 0)],
        }
    }

    pub(crate) fn is_final(&self) -> bool {
        matches!(self, Self::Saved { .. } | Self::Failed(_) | Self::Cancelled)
    }
}

/// A byte count as an `int`; no file comes near the largest one.
fn saturating_int(bytes: u64) -> i64 {
    i64::try_from(bytes).unwrap_or(i64::MAX)
}

/// The whole percentage `received` is of `total`.
fn percent(received: u64, total: u64) -> i64 {
    if total == 0 {
        return DONE_PERCENT;
    }
    let percent = u128::from(received.min(total)) * 100 / u128::from(total);
    i64::try_from(percent).expect("a percentage of at most 100 fits an i64")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_whole_percentages() {
        assert_eq!(percent(0, 200), 0);
        assert_eq!(percent(199, 200), 99);
        assert_eq!(percent(200, 200), 100);
        assert_eq!(percent(300, 200), 100);
        assert_eq!(percent(0, 0), 100);
        assert_eq!(percent(u64::MAX - 1, u64::MAX), 99);
    }

    #[test]
    fn progress_only_moves_the_percentage_once_the_size_is_known() {
        let unknown = Report::Progress(Progress {
            received: 10,
            total: None,
            speed: 5,
        });
        assert_eq!(
            unknown.writes(),
            [
                (DownloadField::Received, Written::Int(10)),
                (DownloadField::Speed, Written::Int(5)),
            ]
        );
        let known = Report::Progress(Progress {
            received: 10,
            total: Some(40),
            speed: 5,
        });
        assert!(
            known
                .writes()
                .contains(&(DownloadField::Progress, Written::Int(25)))
        );
        assert!(
            known
                .writes()
                .contains(&(DownloadField::Total, Written::Int(40)))
        );
    }

    #[test]
    fn saving_fills_in_the_size_and_stops_the_speed() {
        let saved = Report::Saved {
            path: PathBuf::from("/x/a.zip"),
            size: 7,
        };
        let writes = saved.writes();
        for expected in [
            (DownloadField::Progress, Written::Int(100)),
            (DownloadField::Received, Written::Int(7)),
            (DownloadField::Total, Written::Int(7)),
            (DownloadField::Speed, Written::Int(0)),
        ] {
            assert!(writes.contains(&expected), "{writes:?}");
        }
    }
}
