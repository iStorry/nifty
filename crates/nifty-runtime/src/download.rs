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
        matches!(self, Self::Status)
    }
}
