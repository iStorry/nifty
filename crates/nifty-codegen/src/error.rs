use std::fmt;

use cranelift_codegen::CodegenError as CraneliftError;
use cranelift_codegen::settings::SetError;
use cranelift_module::ModuleError;

/// Why an app couldn't be compiled or linked.
#[derive(Debug)]
pub struct CodegenError(String);

impl CodegenError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CodegenError {}

impl From<ModuleError> for CodegenError {
    fn from(error: ModuleError) -> Self {
        Self(error.to_string())
    }
}

impl From<SetError> for CodegenError {
    fn from(error: SetError) -> Self {
        Self(error.to_string())
    }
}

impl From<CraneliftError> for CodegenError {
    fn from(error: CraneliftError) -> Self {
        Self(error.to_string())
    }
}
