//! Compiles a checked program: its functions, its states' starting values, and every
//! dynamic part of its windows and views, then emits the `nifty_rt::ProgramDesc` that
//! describes them to the runtime.

mod body;
mod menu;
mod runtime;
mod types;
mod ui;

use std::collections::HashMap;
use std::mem::{offset_of, size_of};

use cranelift_codegen::ir::types::{I8, I64};
use cranelift_codegen::ir::{InstBuilder, MemFlags, StackSlotData, StackSlotKind};
use cranelift_module::{DataDescription, DataId, FuncId, Module};
use nifty_check::hir::{FnDef, Program, StateDef};
use nifty_rt::{ABI_VERSION, FrameDesc, ProgramDesc, SourceLocation, SpanDesc, StateDesc, Str};
use nifty_syntax::Span;

use self::body::{Body, Operand, Returns};
use self::runtime::Runtime;
use self::types::Types;
use crate::CodegenError;
use crate::emit::{Blob, Emitter, Target};

type Result<T> = std::result::Result<T, CodegenError>;

/// What every function being compiled consults besides the module.
struct Shared<'p> {
    program: &'p Program,
    runtime: Runtime,
    types: Types,
    /// The compiled Nifty functions, by function id.
    functions: Vec<FuncId>,
    spans: Spans,
    /// The bytes of each piece of text in the program, emitted once.
    texts: HashMap<String, DataId>,
}

impl Shared<'_> {
    /// The data holding `text`'s bytes.
    fn text<M: Module>(&mut self, module: &mut M, text: &str) -> Result<DataId> {
        if let Some(id) = self.texts.get(text) {
            return Ok(*id);
        }
        let id = module.declare_anonymous_data(false, false)?;
        let mut description = DataDescription::new();
        description.define(text.as_bytes().into());
        module.define_data(id, &description)?;
        self.texts.insert(text.to_owned(), id);
        Ok(id)
    }
}

/// The spans failures point at, numbered in the order code first uses them.
#[derive(Debug, Default)]
struct Spans {
    ids: HashMap<Span, u32>,
    spans: Vec<Span>,
}

impl Spans {
    fn id(&mut self, span: Span) -> Result<u32> {
        if let Some(id) = self.ids.get(&span) {
            return Ok(*id);
        }
        let id = u32::try_from(self.spans.len())
            .map_err(|_| CodegenError::new("the program has too many places that can fail"))?;
        self.ids.insert(span, id);
        self.spans.push(span);
        Ok(id)
    }
}

/// Compiles `program` into `emitter`, and returns its `nifty_rt::ProgramDesc`. `locate`
/// says where each span is, for the errors the app may report.
///
/// # Errors
/// When the program has errors, or Cranelift can't compile it.
pub fn compile<M: Module>(
    emitter: &mut Emitter<M>,
    program: &Program,
    locate: &dyn Fn(Span) -> SourceLocation,
) -> Result<DataId> {
    let runtime = Runtime::import(emitter)?;
    let functions = program
        .functions
        .iter()
        .map(|function| emitter.declare_function(&function_signature(emitter, function)))
        .collect::<Result<_>>()?;
    let mut shared = Shared {
        program,
        runtime,
        types: Types::default(),
        functions,
        spans: Spans::default(),
        texts: HashMap::new(),
    };
    for (index, function) in program.functions.iter().enumerate() {
        let id = shared.functions[index];
        compile_function(emitter, &mut shared, function, id)?;
    }
    let states = program
        .states
        .iter()
        .map(|state| compile_state(emitter, &mut shared, state))
        .collect::<Result<Vec<_>>>()?;
    let windows = ui::windows(emitter, &mut shared)?;
    let views = ui::views(emitter, &mut shared)?;
    let (menus, menu_locals) = menu::menu_bar(emitter, &mut shared)?;
    let state_list = state_list(emitter, &mut shared, &states)?;
    let names = function_names(emitter, &mut shared)?;
    let spans = span_list(emitter, &mut shared, locate)?;
    shared.types.emit(emitter, &shared.runtime, program)?;

    let mut desc = Blob::zeroed(size_of::<ProgramDesc>());
    desc.put_u32(offset_of!(ProgramDesc, abi_version), ABI_VERSION);
    for (field, list) in [
        (offset_of!(ProgramDesc, states), state_list),
        (offset_of!(ProgramDesc, windows), windows),
        (offset_of!(ProgramDesc, views), views),
        (offset_of!(ProgramDesc, functions), names),
        (offset_of!(ProgramDesc, spans), spans),
        (offset_of!(ProgramDesc, menus), menus),
        (
            offset_of!(ProgramDesc, menu_frame) + offset_of!(FrameDesc, locals),
            menu_locals,
        ),
    ] {
        list.put(&mut desc, field);
    }
    emitter.data(desc)
}

/// An array emitted as data, ready to be pointed at by a `nifty_rt::Slice`.
#[derive(Clone, Copy, Debug)]
struct List {
    data: Option<DataId>,
    len: usize,
}

impl List {
    const EMPTY: Self = Self { data: None, len: 0 };

    /// Emits `len` items of `size` bytes each, which `write` lays out in a blob at the
    /// offset it's given.
    fn emit<M: Module>(
        emitter: &mut Emitter<M>,
        len: usize,
        size: usize,
        mut write: impl FnMut(&mut Blob, usize, usize) -> Result<()>,
    ) -> Result<Self> {
        if len == 0 {
            return Ok(Self::EMPTY);
        }
        let mut blob = Blob::zeroed(len * size);
        for index in 0..len {
            write(&mut blob, index, index * size)?;
        }
        Ok(Self {
            data: Some(emitter.data(blob)?),
            len,
        })
    }

    fn put(self, blob: &mut Blob, offset: usize) {
        blob.put_slice(offset, self.data.map(Target::Data), self.len);
    }
}

/// A `Str` field at `offset` holding `text`.
fn put_str<M: Module>(
    emitter: &mut Emitter<M>,
    shared: &mut Shared<'_>,
    blob: &mut Blob,
    offset: usize,
    text: &str,
) -> Result<()> {
    let bytes = (!text.is_empty())
        .then(|| shared.text(emitter.module_mut(), text))
        .transpose()?;
    blob.put_slice(offset, bytes.map(Target::Data), text.len());
    Ok(())
}

/// Nifty functions take the context, then each argument as a slot, then where to write
/// their result, and return whether they succeeded.
fn function_signature<M: Module>(
    emitter: &Emitter<M>,
    function: &FnDef,
) -> cranelift_codegen::ir::Signature {
    let mut params = vec![I64];
    params.extend(std::iter::repeat_n(I64, function.params));
    params.push(I64);
    emitter.signature(&params, &[I8])
}

/// Arguments come in owned, into the first locals; a function releases its locals when
/// it returns.
fn compile_function<M: Module>(
    emitter: &mut Emitter<M>,
    shared: &mut Shared<'_>,
    function: &FnDef,
    id: FuncId,
) -> Result<()> {
    let signature = function_signature(emitter, function);
    emitter.define_function(id, signature, |b, module| {
        let entry = b
            .current_block()
            .ok_or_else(|| CodegenError::new("a function has no entry block"))?;
        let params = b.block_params(entry).to_vec();
        let (context, out) = (params[0], params[params.len() - 1]);
        let frame_size = u32::try_from(function.locals.len() * 8)
            .map_err(|_| CodegenError::new(format!("`{}` has too many locals", function.name)))?;
        let slot = b.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            frame_size.max(8),
            3,
        ));
        let frame = b.ins().stack_addr(I64, slot, 0);
        let zero = b.ins().iconst(I64, 0);
        for local in 0..function.locals.len() {
            let value = params.get(local + 1).filter(|_| local < function.params);
            let at =
                i32::try_from(local * 8).map_err(|_| CodegenError::new("a frame is too large"))?;
            b.ins().store(
                MemFlags::trusted(),
                value.copied().unwrap_or(zero),
                frame,
                at,
            );
        }
        let mut body = Body::new(b, module, shared, context, frame, &function.locals);
        body.returns = Some(Returns { out });
        body.block(&function.body)?;
        body.release_locals()?;
        body.finish();
        Ok(())
    })
}

/// A state's starting value, written to `out`.
fn compile_state<M: Module>(
    emitter: &mut Emitter<M>,
    shared: &mut Shared<'_>,
    state: &StateDef,
) -> Result<FuncId> {
    let signature = emitter.signature(&[I64, I64], &[I8]);
    emitter.function(signature, |b, module| {
        let entry = b
            .current_block()
            .ok_or_else(|| CodegenError::new("a function has no entry block"))?;
        let (context, out) = (b.block_params(entry)[0], b.block_params(entry)[1]);
        let frame = b.ins().iconst(I64, 0);
        let mut body = Body::new(b, module, shared, context, frame, &[]);
        let operand: Operand = body.expr(&state.init)?;
        let value = body.own(operand, &state.ty)?;
        body.b.ins().store(MemFlags::trusted(), value, out, 0);
        body.finish();
        Ok(())
    })
}

fn state_list<M: Module>(
    emitter: &mut Emitter<M>,
    shared: &mut Shared<'_>,
    inits: &[FuncId],
) -> Result<List> {
    let types = shared
        .program
        .states
        .iter()
        .map(|state| {
            shared
                .types
                .desc(emitter.module_mut(), &shared.runtime, &state.ty)
        })
        .collect::<Result<Vec<_>>>()?;
    List::emit(
        emitter,
        inits.len(),
        size_of::<StateDesc>(),
        |blob, index, at| {
            blob.put_pointer(at + offset_of!(StateDesc, ty), Target::Data(types[index]));
            blob.put_pointer(
                at + offset_of!(StateDesc, init),
                Target::Function(inits[index]),
            );
            Ok(())
        },
    )
}

fn function_names<M: Module>(emitter: &mut Emitter<M>, shared: &mut Shared<'_>) -> Result<List> {
    let names = shared
        .program
        .functions
        .iter()
        .map(|function| shared.text(emitter.module_mut(), &function.name))
        .collect::<Result<Vec<_>>>()?;
    let functions = &shared.program.functions;
    List::emit(emitter, names.len(), size_of::<Str>(), |blob, index, at| {
        blob.put_slice(
            at,
            Some(Target::Data(names[index])),
            functions[index].name.len(),
        );
        Ok(())
    })
}

fn span_list<M: Module>(
    emitter: &mut Emitter<M>,
    shared: &mut Shared<'_>,
    locate: &dyn Fn(Span) -> SourceLocation,
) -> Result<List> {
    let locations: Vec<SourceLocation> = shared
        .spans
        .spans
        .iter()
        .map(|span| locate(*span))
        .collect();
    let files = locations
        .iter()
        .map(|location| shared.text(emitter.module_mut(), &location.file))
        .collect::<Result<Vec<_>>>()?;
    List::emit(
        emitter,
        locations.len(),
        size_of::<SpanDesc>(),
        |blob, index, at| {
            let location = &locations[index];
            blob.put_slice(
                at + offset_of!(SpanDesc, file),
                Some(Target::Data(files[index])),
                location.file.len(),
            );
            blob.put_u32(at + offset_of!(SpanDesc, line), location.line);
            blob.put_u32(at + offset_of!(SpanDesc, column), location.column);
            Ok(())
        },
    )
}
