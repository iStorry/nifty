//! Compiles Nifty programs to object files for the host system, and links them against
//! the `nifty-rt` runtime library into executables.

mod link;
mod object;

pub use link::{RUNTIME_LIBRARY, link};
pub use object::{Blob, CodegenError, FunctionBody, ObjectFile, Target};
