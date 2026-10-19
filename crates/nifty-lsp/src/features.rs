//! One module per LSP feature. Each turns a [`Document`](crate::document::Document) into
//! protocol types and knows nothing about the connection.

pub(crate) mod diagnostics;
pub(crate) mod navigation;
pub(crate) mod outline;
pub(crate) mod semantic_tokens;
