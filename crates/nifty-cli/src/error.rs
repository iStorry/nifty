use std::fmt;

/// A problem that stops a command before it can do its work, shown as `error: …`.
#[derive(Debug)]
pub(crate) struct CliError(String);

impl CliError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
