//! What a `download(…)` call can bind, shared by the checker, which reads the call, and
//! everything that runs it.

coded_enum! {
    /// A piece of state a download reports into. Compiled apps pass it to the runtime by
    /// its code.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum DownloadField {
        /// The percentage done, `0` to `100`.
        Progress,
        /// Where the download stands, in words.
        Status,
        /// The bytes saved so far.
        Received,
        /// The file's size in bytes, once the server says; `0` until then.
        Total,
        /// The bytes received per second, over the last moments.
        Speed,
        /// The download's id, which code passes to `cancel_download` and the like, and
        /// which its other bindings follow when their list changes.
        Id,
    }
}

impl DownloadField {
    /// The setting that binds it, such as `progress`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Progress => "progress",
            Self::Status => "status",
            Self::Received => "received",
            Self::Total => "total",
            Self::Speed => "speed",
            Self::Id => "id",
        }
    }

    /// Whether every call must bind it.
    #[must_use]
    pub const fn is_required(self) -> bool {
        matches!(self, Self::Progress | Self::Status)
    }

    /// Whether it holds text rather than a whole number.
    #[must_use]
    pub const fn is_text(self) -> bool {
        matches!(self, Self::Status | Self::Id)
    }
}

/// What code asks of a running download, by its id. `repr(u32)` because compiled apps
/// pass it to the runtime, which every platform's C calling convention passes without
/// widening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum DownloadCommand {
    /// Stops it for good, removing what it saved so far.
    Cancel,
    /// Stops it for now, keeping what it saved so far.
    Pause,
    /// Carries on with a paused download from where it stopped, when the server can.
    Resume,
}

impl DownloadCommand {
    /// The built-in function that asks for it, such as `cancel_download`.
    #[must_use]
    pub const fn function(self) -> &'static str {
        match self {
            Self::Cancel => "cancel_download",
            Self::Pause => "pause_download",
            Self::Resume => "resume_download",
        }
    }
}
