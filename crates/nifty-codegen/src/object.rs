//! An object file being written: functions compiled with Cranelift, data such as the
//! program descriptor, and the `main` that hands the descriptor to the runtime.

use std::fmt;

use cranelift_codegen::ir::{AbiParam, Block, InstBuilder, Signature, Type, Value, types};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::{CodegenError as CraneliftError, Context};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module, ModuleError};
use cranelift_object::{ObjectBuilder, ObjectModule};
use nifty_rt::{Slice, symbols};

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

impl From<settings::SetError> for CodegenError {
    fn from(error: settings::SetError) -> Self {
        Self(error.to_string())
    }
}

impl From<CraneliftError> for CodegenError {
    fn from(error: CraneliftError) -> Self {
        Self(error.to_string())
    }
}

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

    pub fn put_u32(&mut self, offset: usize, value: u32) {
        self.put(offset, &value.to_ne_bytes());
    }

    pub fn put_usize(&mut self, offset: usize, value: usize) {
        self.put(offset, &value.to_ne_bytes());
    }

    pub fn put_pointer(&mut self, offset: usize, target: Target) {
        self.pointers.push((offset, target));
    }

    /// A `nifty_rt::Slice` or `Str` at `offset`: `len` items starting at `items`.
    pub fn put_slice(&mut self, offset: usize, items: Target, len: usize) {
        self.put_pointer(offset + Slice::<u8>::PTR_OFFSET, items);
        self.put_usize(offset + Slice::<u8>::LEN_OFFSET, len);
    }

    /// # Panics
    /// When the value doesn't fit inside the blob, which is a bug in the caller's layout.
    fn put(&mut self, offset: usize, value: &[u8]) {
        self.bytes[offset..offset + value.len()].copy_from_slice(value);
    }
}

pub struct ObjectFile {
    module: ObjectModule,
    context: Context,
    builder_context: FunctionBuilderContext,
}

/// A function being compiled: its Cranelift builder, and the object it can call into.
pub struct FunctionBody<'a> {
    pub builder: FunctionBuilder<'a>,
    module: &'a mut ObjectModule,
    entry: Block,
}

impl FunctionBody<'_> {
    /// The function's parameters, in order.
    #[must_use]
    pub fn params(&self) -> &[Value] {
        self.builder.block_params(self.entry)
    }

    /// Calls `function` with `args`, returning its results.
    pub fn call(&mut self, function: FuncId, args: &[Value]) -> Vec<Value> {
        let callee = self
            .module
            .declare_func_in_func(function, self.builder.func);
        let call = self.builder.ins().call(callee, args);
        self.builder.inst_results(call).to_vec()
    }

    /// The address of `data`.
    pub fn address(&mut self, data: DataId) -> Value {
        let pointer = self.module.target_config().pointer_type();
        let global = self.module.declare_data_in_func(data, self.builder.func);
        self.builder.ins().global_value(pointer, global)
    }
}

impl ObjectFile {
    /// An empty object file for the system `nf` runs on, named `name` inside.
    ///
    /// # Errors
    /// When Cranelift doesn't support this system.
    pub fn new(name: &str) -> Result<Self, CodegenError> {
        let mut flags = settings::builder();
        // Executables on macOS and Windows load at a random address.
        flags.set("is_pic", "true")?;
        flags.set("opt_level", "speed")?;
        let isa = cranelift_native::builder()
            .map_err(|error| CodegenError(error.to_owned()))?
            .finish(settings::Flags::new(flags))?;
        let builder = ObjectBuilder::new(isa, name, cranelift_module::default_libcall_names())?;
        Ok(Self {
            module: ObjectModule::new(builder),
            context: Context::new(),
            builder_context: FunctionBuilderContext::new(),
        })
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

    /// Defines read-only data private to this object.
    ///
    /// # Errors
    /// When Cranelift can't define it.
    pub fn define_data(&mut self, blob: Blob) -> Result<DataId, CodegenError> {
        let id = self.module.declare_anonymous_data(false, false)?;
        let mut description = DataDescription::new();
        description.set_align(8);
        for (offset, target) in &blob.pointers {
            let offset = u32::try_from(*offset).map_err(|_| {
                CodegenError(format!("data is too large to point at offset {offset}"))
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
        Ok(id)
    }

    /// Compiles a function private to this object, whose body `build` emits from its entry
    /// block, which has the signature's parameters.
    ///
    /// # Errors
    /// When Cranelift can't compile the body.
    pub fn define_function(
        &mut self,
        signature: Signature,
        build: impl FnOnce(&mut FunctionBody<'_>),
    ) -> Result<FuncId, CodegenError> {
        let id = self.module.declare_anonymous_function(&signature)?;
        self.compile(id, signature, build)?;
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
        self.compile(main, signature, |body| {
            let program = body.address(program);
            let status = body.call(run, &[program]);
            body.builder.ins().return_(&status);
        })
    }

    /// # Errors
    /// When the object can't be written.
    pub fn finish(self) -> Result<Vec<u8>, CodegenError> {
        let product = self.module.finish();
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
            .map_err(|error| CodegenError(error.to_string()))
    }

    fn compile(
        &mut self,
        id: FuncId,
        signature: Signature,
        build: impl FnOnce(&mut FunctionBody<'_>),
    ) -> Result<(), CodegenError> {
        self.context.func.signature = signature;
        let mut builder = FunctionBuilder::new(&mut self.context.func, &mut self.builder_context);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let mut body = FunctionBody {
            builder,
            module: &mut self.module,
            entry,
        };
        build(&mut body);
        body.builder.finalize();
        self.module.define_function(id, &mut self.context)?;
        self.module.clear_context(&mut self.context);
        Ok(())
    }
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

impl fmt::Debug for ObjectFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectFile").finish_non_exhaustive()
    }
}

impl fmt::Debug for FunctionBody<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FunctionBody").finish_non_exhaustive()
    }
}
