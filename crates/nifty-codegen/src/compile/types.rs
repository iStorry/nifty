//! The type descriptors compiled code hands the runtime. The built-in types are the
//! runtime's own; each list and struct type is emitted once, after the code that uses it,
//! since struct types can lead back to themselves.

use std::collections::HashMap;
use std::mem::size_of;

use cranelift_module::{DataId, Module};
use nifty_check::hir::{Program, Type};
use nifty_rt::{TypeDesc, TypeKind};

use super::runtime::Runtime;
use crate::CodegenError;
use crate::emit::{Blob, Emitter, Target};

#[derive(Debug, Default)]
pub(super) struct Types {
    declared: HashMap<Type, DataId>,
    /// Declared but not yet emitted.
    pending: Vec<(Type, DataId)>,
}

impl Types {
    /// The descriptor of `ty`, declaring it if it's new.
    pub fn desc<M: Module>(
        &mut self,
        module: &mut M,
        runtime: &Runtime,
        ty: &Type,
    ) -> Result<DataId, CodegenError> {
        match ty {
            Type::Int => return Ok(runtime.int),
            Type::Float => return Ok(runtime.float),
            Type::Bool => return Ok(runtime.bool),
            Type::String => return Ok(runtime.string),
            Type::List(_) | Type::Struct(_) => {}
            Type::Void | Type::Error => {
                return Err(CodegenError::new(format!("no value has the type {ty:?}")));
            }
        }
        if let Some(id) = self.declared.get(ty) {
            return Ok(*id);
        }
        let id = module.declare_anonymous_data(false, false)?;
        self.declared.insert(ty.clone(), id);
        self.pending.push((ty.clone(), id));
        Ok(id)
    }

    /// Emits every descriptor declared so far, and the ones they lead to.
    pub fn emit<M: Module>(
        &mut self,
        emitter: &mut Emitter<M>,
        runtime: &Runtime,
        program: &Program,
    ) -> Result<(), CodegenError> {
        while let Some((ty, id)) = self.pending.pop() {
            let (kind, children) = match &ty {
                Type::List(element) => (TypeKind::List, vec![Type::clone(element)]),
                Type::Struct(id) => {
                    let fields = &program.structs[id.0].fields;
                    let fields = fields.iter().map(|field| field.ty.clone()).collect();
                    (TypeKind::Struct, fields)
                }
                other => {
                    return Err(CodegenError::new(format!(
                        "{other:?} has no descriptor of its own"
                    )));
                }
            };
            let mut list = Blob::zeroed(children.len() * size_of::<&TypeDesc>());
            for (index, child) in children.iter().enumerate() {
                let child = self.desc(emitter.module_mut(), runtime, child)?;
                list.put_pointer(index * size_of::<&TypeDesc>(), Target::Data(child));
            }
            let list = emitter.data(list)?;
            let mut desc = Blob::zeroed(size_of::<TypeDesc>());
            desc.put_u8(TypeDesc::KIND_OFFSET, kind.code());
            desc.put_slice(
                TypeDesc::CHILDREN_OFFSET,
                Some(Target::Data(list)),
                children.len(),
            );
            emitter.define_data(id, desc)?;
        }
        Ok(())
    }
}

/// Whether values of `ty` are reference-counted objects rather than plain bits.
pub(super) fn is_object(ty: &Type) -> bool {
    matches!(ty, Type::String | Type::List(_) | Type::Struct(_))
}
