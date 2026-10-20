//! Writing functions and data into a Cranelift module: an object file for `nf build`, or
//! memory for code loaded in-process.

use std::fmt;

use cranelift_codegen::Context;
use cranelift_codegen::ir::{AbiParam, InstBuilder, Signature, Type, types};
use cranelift_codegen::isa::OwnedTargetIsa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use nifty_rt::{Slice, symbols};

use crate::CodegenError;

/// What a pointer in data points at.
#[derive(Clone, Copy, Debug)]
pub enum Target {
    Data(DataId),
    Function(FuncId),
}

/// The bytes of one piece of data, and the pointers in it, which the linker fills in.
#[derive(Debug)]
pub struct Blob {
    bytes: Vec<u8>,
    pointers: Vec<(usize, Target)>,
}

impl Blob {
    /// `size` zero bytes, which read as empty slices, null pointers and zeros.
    #[must_use]
    pub fn zeroed(size: usize) -> Self {
        Self {
            bytes: vec![0; size],
            pointers: Vec::new(),
        }
    }

    #[must_use]
    pub fn bytes(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.to_vec(),
            pointers: Vec::new(),
        }
    }

    pub fn put_u8(&mut self, offset: usize, value: u8) {
        self.put(offset, &[value]);
    }

    pub fn put_u32(&mut self, offset: usize, value: u32) {
        self.put(offset, &value.to_ne_bytes());
    }

    pub fn put_i64(&mut self, offset: usize, value: i64) {
        self.put(offset, &value.to_ne_bytes());
    }

    pub fn put_f32(&mut self, offset: usize, value: f32) {
        self.put(offset, &value.to_ne_bytes());
    }

    pub fn put_usize(&mut self, offset: usize, value: usize) {
        self.put(offset, &value.to_ne_bytes());
    }

    pub fn put_pointer(&mut self, offset: usize, target: Target) {
        self.pointers.push((offset, target));
    }

    /// A `nifty_rt::Slice` or `Str` at `offset`: `len` items starting at `items`, or an
    /// empty one when there are none.
    pub fn put_slice(&mut self, offset: usize, items: Option<Target>, len: usize) {
        if let Some(items) = items {
            self.put_pointer(offset + Slice::<u8>::PTR_OFFSET, items);
        }
        self.put_usize(offset + Slice::<u8>::LEN_OFFSET, len);
    }

    /// # Panics
    /// When the value doesn't fit inside the blob, which is a bug in the caller's layout.
    fn put(&mut self, offset: usize, value: &[u8]) {
        self.bytes[offset..offset + value.len()].copy_from_slice(value);
    }
}

/// The instruction set of the system `nf` runs on. Code loaded in-process must not be
/// position-independent; executables on macOS and Windows must be, since they load at a
/// random address.
///
/// # Errors
/// When Cranelift doesn't support this system.
pub fn host_isa(position_independent: bool) -> Result<OwnedTargetIsa, CodegenError> {
    let mut flags = settings::builder();
    flags.set(
        "is_pic",
        if position_independent {
            "true"
        } else {
            "false"
        },
    )?;
    flags.set("opt_level", "speed")?;
    Ok(cranelift_native::builder()
        .map_err(CodegenError::new)?
        .finish(settings::Flags::new(flags))?)
}

/// A module being written, with the contexts Cranelift reuses between functions.
pub struct Emitter<M: Module> {
    module: M,
    context: Context,
    builder_context: FunctionBuilderContext,
}

impl<M: Module> Emitter<M> {
    pub fn new(module: M) -> Self {
        Self {
            module,
            context: Context::new(),
            builder_context: FunctionBuilderContext::new(),
        }
    }

    pub fn into_module(self) -> M {
        self.module
    }

    pub(crate) fn module_mut(&mut self) -> &mut M {
        &mut self.module
    }

    #[must_use]
    pub fn pointer_type(&self) -> Type {
        self.module.target_config().pointer_type()
    }

    /// A signature in the system's C calling convention.
    #[must_use]
    pub fn signature(&self, params: &[Type], returns: &[Type]) -> Signature {
        let mut signature = self.module.make_signature();
        signature
            .params
            .extend(params.iter().copied().map(AbiParam::new));
        signature
            .returns
            .extend(returns.iter().copied().map(AbiParam::new));
        signature
    }

    /// A runtime function, called by name.
    ///
    /// # Errors
    /// When `name` was already declared with another signature.
    pub fn import_function(
        &mut self,
        name: &str,
        signature: &Signature,
    ) -> Result<FuncId, CodegenError> {
        Ok(self
            .module
            .declare_function(name, Linkage::Import, signature)?)
    }

    /// Runtime data, such as a built-in type, referred to by name.
    ///
    /// # Errors
    /// When `name` was already declared as something else.
    pub fn import_data(&mut self, name: &str) -> Result<DataId, CodegenError> {
        Ok(self
            .module
            .declare_data(name, Linkage::Import, false, false)?)
    }

    /// Read-only data private to this module, defined later with [`Emitter::define_data`],
    /// so other data can point at it first.
    ///
    /// # Errors
    /// When Cranelift can't declare it.
    pub fn declare_data(&mut self) -> Result<DataId, CodegenError> {
        Ok(self.module.declare_anonymous_data(false, false)?)
    }

    /// # Errors
    /// When Cranelift can't define it.
    pub fn define_data(&mut self, id: DataId, blob: Blob) -> Result<(), CodegenError> {
        let mut description = DataDescription::new();
        description.set_align(8);
        for (offset, target) in &blob.pointers {
            let offset = u32::try_from(*offset).map_err(|_| {
                CodegenError::new(format!("data is too large to point at offset {offset}"))
            })?;
            match *target {
                Target::Data(data) => {
                    let global = self.module.declare_data_in_data(data, &mut description);
                    description.write_data_addr(offset, global, 0);
                }
                Target::Function(function) => {
                    let function = self.module.declare_func_in_data(function, &mut description);
                    description.write_function_addr(offset, function);
                }
            }
        }
        description.define(blob.bytes.into_boxed_slice());
        self.module.define_data(id, &description)?;
        Ok(())
    }

    /// Declares and defines read-only data private to this module.
    ///
    /// # Errors
    /// When Cranelift can't define it.
    pub fn data(&mut self, blob: Blob) -> Result<DataId, CodegenError> {
        let id = self.declare_data()?;
        self.define_data(id, blob)?;
        Ok(id)
    }

    /// A function private to this module, compiled later with
    /// [`Emitter::define_function`], so code can call it first.
    ///
    /// # Errors
    /// When Cranelift can't declare it.
    pub fn declare_function(&mut self, signature: &Signature) -> Result<FuncId, CodegenError> {
        Ok(self.module.declare_anonymous_function(signature)?)
    }

    /// Compiles `id`, whose body `build` emits from its entry block, which has the
    /// signature's parameters and is current when `build` starts. `build` also gets the
    /// module, to refer to other functions and data.
    ///
    /// # Errors
    /// The error `build` returns, or Cranelift's when it can't compile the body.
    pub fn define_function(
        &mut self,
        id: FuncId,
        signature: Signature,
        build: impl FnOnce(&mut FunctionBuilder<'_>, &mut M) -> Result<(), CodegenError>,
    ) -> Result<(), CodegenError> {
        self.context.func.signature = signature;
        let mut builder = FunctionBuilder::new(&mut self.context.func, &mut self.builder_context);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        let body = build(&mut builder, &mut self.module);
        builder.seal_all_blocks();
        builder.finalize();
        let defined = body.and_then(|()| Ok(self.module.define_function(id, &mut self.context)?));
        self.module.clear_context(&mut self.context);
        defined
    }

    /// Declares and compiles a function private to this module.
    ///
    /// # Errors
    /// As [`Emitter::define_function`].
    pub fn function(
        &mut self,
        signature: Signature,
        build: impl FnOnce(&mut FunctionBuilder<'_>, &mut M) -> Result<(), CodegenError>,
    ) -> Result<FuncId, CodegenError> {
        let id = self.declare_function(&signature)?;
        self.define_function(id, signature, build)?;
        Ok(id)
    }

    /// Emits `main`, which hands `program`, a `nifty_rt::ProgramDesc`, to the runtime and
    /// returns the exit status it gives back.
    ///
    /// # Errors
    /// When Cranelift can't compile it.
    pub fn define_main(&mut self, program: DataId) -> Result<(), CodegenError> {
        let pointer = self.pointer_type();
        let run =
            self.import_function(symbols::MAIN, &self.signature(&[pointer], &[types::I32]))?;
        let signature = self.signature(&[types::I32, pointer], &[types::I32]);
        let main = self
            .module
            .declare_function("main", Linkage::Export, &signature)?;
        self.define_function(main, signature, |builder, module| {
            let global = module.declare_data_in_func(program, builder.func);
            let program = builder.ins().global_value(pointer, global);
            let run = module.declare_func_in_func(run, builder.func);
            let call = builder.ins().call(run, &[program]);
            let status = builder.inst_results(call).to_vec();
            builder.ins().return_(&status);
            Ok(())
        })
    }
}

impl<M: Module> fmt::Debug for Emitter<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Emitter").finish_non_exhaustive()
    }
}
