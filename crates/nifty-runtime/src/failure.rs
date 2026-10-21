//! Why running an app's code can fail. The interpreter and compiled apps both report these,
//! so an app fails the same way however it runs.

use std::fmt;

/// How deeply functions may call each other before the app stops with an error. The
/// interpreter takes roughly 14 KB of native stack per call in debug builds, so this stops
/// runaway recursion before a 2 MB thread stack overflows.
pub const MAX_CALL_DEPTH: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    DivideByZero,
    /// An `int` result outside the 64-bit range.
    TooLarge,
    /// A length that doesn't fit in an `int`.
    TooLong,
    NegativePosition(i64),
    NoItem {
        position: usize,
        len: usize,
    },
    /// `text.slice(start, end)` with positions outside the text's `len` characters, or
    /// `start` after `end`.
    NoSlice {
        start: i64,
        end: i64,
        len: usize,
    },
    /// A call deeper than [`MAX_CALL_DEPTH`], into the function named `function`.
    TooDeep {
        function: String,
    },
    DownloadIntoVariable,
    /// A file dialog's answer bound to a variable, which is gone by the time it closes.
    DialogIntoVariable,
    /// The platform couldn't show a file dialog or read its answer, for this reason.
    Dialog(String),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DivideByZero => f.write_str("can't divide by zero"),
            Self::TooLarge => f.write_str("this number is too large"),
            Self::TooLong => f.write_str("this is too long to count"),
            Self::NegativePosition(index) => {
                write!(f, "list positions start at 0, so {index} isn't one")
            }
            Self::NoItem { position, len } => {
                write!(f, "there is no item {position}; the list has {len}")
            }
            Self::NoSlice { start, end, len } => write!(
                f,
                "can't slice from {start} to {end}; the text has {len} characters, and a slice needs 0 <= start <= end <= len"
            ),
            Self::TooDeep { function } => {
                write!(
                    f,
                    "`{function}` calls itself too deeply; does it ever stop?"
                )
            }
            Self::DownloadIntoVariable => {
                f.write_str("`download` can only report into state, not into a variable")
            }
            Self::DialogIntoVariable => {
                f.write_str("a file dialog can only answer into state, not into a variable")
            }
            Self::Dialog(reason) => write!(f, "the file dialog failed: {reason}"),
        }
    }
}
