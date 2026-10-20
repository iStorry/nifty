//! Compiles checked Nifty programs to machine code with Cranelift, and links them against
//! the `nifty-rt` runtime library into executables.

mod compile;
mod emit;
mod error;
mod link;
mod object;
#[cfg(feature = "testing")]
pub mod testing;

pub use compile::compile;
pub use emit::{Blob, Emitter, Target, host_isa};
pub use error::CodegenError;
pub use link::{RUNTIME_LIBRARY, link};
pub use object::{compile_app, finish, object_emitter};
