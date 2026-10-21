//! What a `fetch(…)` call can bind, shared by the checker, which reads the call, and
//! everything that runs it.

coded_enum! {
    /// A piece of state a fetch writes its response into. Compiled apps pass it to the
    /// runtime by its code.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum FetchField {
        /// Where the request stands, in words.
        Status,
        /// The response's body, as text.
        Text,
        /// The response's HTTP status code, such as `200`; `0` when no response came.
        Code,
        /// The response's headers, each written `name: value`.
        ResponseHeaders,
    }
}

impl FetchField {
    /// The setting that binds it, such as `status`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Text => "text",
            Self::Code => "code",
            Self::ResponseHeaders => "response_headers",
        }
    }

    /// Whether every call must bind it.
    #[must_use]
    pub const fn is_required(self) -> bool {
        matches!(self, Self::Status)
    }
}
