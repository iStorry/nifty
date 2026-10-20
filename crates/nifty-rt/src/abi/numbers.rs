//! Arithmetic Cranelift has no instruction for.

/// `a % b` on floats, which keeps the sign of `a`, as the interpreter's does.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_float_rem(a: f64, b: f64) -> f64 {
    a % b
}
