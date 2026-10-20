//! Object files for `nf build`: a compiled program and the `main` that runs it.

use cranelift_object::{ObjectBuilder, ObjectModule};
use nifty_check::hir::Program;
use nifty_rt::SourceLocation;
use nifty_syntax::Span;

use crate::CodegenError;
use crate::compile::compile;
use crate::emit::{Emitter, host_isa};

/// Compiles `program` to an object file whose `main` runs it, with `locate` saying where
/// each span is, for the errors the app may report.
///
/// # Errors
/// When the program has errors, or Cranelift can't compile it for this system.
pub fn compile_app(
    name: &str,
    program: &Program,
    locate: &dyn Fn(Span) -> SourceLocation,
) -> Result<Vec<u8>, CodegenError> {
    let mut emitter = object_emitter(name)?;
    let descriptor = compile(&mut emitter, program, locate)?;
    emitter.define_main(descriptor)?;
    finish(emitter)
}

/// An empty object file for the system `nf` runs on, named `name` inside.
///
/// # Errors
/// When Cranelift doesn't support this system.
pub fn object_emitter(name: &str) -> Result<Emitter<ObjectModule>, CodegenError> {
    let builder = ObjectBuilder::new(
        host_isa(true)?,
        name,
        cranelift_module::default_libcall_names(),
    )?;
    Ok(Emitter::new(ObjectModule::new(builder)))
}

/// The object file's bytes.
///
/// # Errors
/// When the object can't be written.
pub fn finish(emitter: Emitter<ObjectModule>) -> Result<Vec<u8>, CodegenError> {
    let product = emitter.into_module().finish();
    #[cfg(target_os = "macos")]
    let product = {
        let mut product = product;
        product
            .object
            .set_macho_build_version(macos_build_version());
        product
    };
    product
        .emit()
        .map_err(|error| CodegenError::new(error.to_string()))
}

/// The oldest macOS compiled apps run on: the oldest Apple-silicon Macs run, which Rust,
/// and so the runtime library, also targets.
#[cfg(target_os = "macos")]
const MINIMUM_MACOS: (u32, u32) = (11, 0);

/// Says which macOS the object is for, which the linker otherwise warns it has to assume.
#[cfg(target_os = "macos")]
fn macos_build_version() -> object::write::MachOBuildVersion {
    let (major, minor) = MINIMUM_MACOS;
    let mut version = object::write::MachOBuildVersion::default();
    version.platform = object::macho::PLATFORM_MACOS;
    // Versions are encoded in nibbles as xxxx.yy.zz.
    version.minos = (major << 16) | (minor << 8);
    version
}
