//! The runtime calls compiled code makes, each imported once with its C signature.

use cranelift_codegen::ir::types::{F64, I8, I32, I64};
use cranelift_module::{DataId, FuncId, Module};
use nifty_rt::symbols;

use crate::CodegenError;
use crate::emit::Emitter;

/// Pointers and slots are both 64 bits on the systems Nifty runs on.
const P: cranelift_codegen::ir::Type = I64;
const S: cranelift_codegen::ir::Type = I64;
/// `bool` results.
const B: cranelift_codegen::ir::Type = I8;
/// `u32` ids: states, spans, views, lists and functions.
const U: cranelift_codegen::ir::Type = I32;

macro_rules! runtime {
    ($($field:ident = $symbol:ident ($($param:expr),*) $(-> $result:expr)?;)*) => {
        #[derive(Debug)]
        pub(super) struct Runtime {
            $(pub $field: FuncId,)*
            pub int: DataId,
            pub float: DataId,
            pub bool: DataId,
            pub string: DataId,
        }

        impl Runtime {
            pub fn import<M: Module>(emitter: &mut Emitter<M>) -> Result<Self, CodegenError> {
                Ok(Self {
                    $($field: {
                        let signature = emitter.signature(&[$($param),*], &[$($result)?]);
                        emitter.import_function(symbols::$symbol, &signature)?
                    },)*
                    int: emitter.import_data(symbols::TYPE_INT)?,
                    float: emitter.import_data(symbols::TYPE_FLOAT)?,
                    bool: emitter.import_data(symbols::TYPE_BOOL)?,
                    string: emitter.import_data(symbols::TYPE_STRING)?,
                })
            }
        }
    };
}

runtime! {
    retain = RETAIN(S, P);
    release = RELEASE(S, P);
    equal = EQUAL(S, S, P) -> B;
    store = STORE(P, S, P);
    float_rem = FLOAT_REM(F64, F64) -> F64;
    string_new = STRING_NEW(P, I64) -> S;
    string_concat = STRING_CONCAT(S, S) -> S;
    string_len = STRING_LEN(S) -> I64;
    string_compare = STRING_COMPARE(S, S) -> I32;
    text_new = TEXT_NEW() -> P;
    text_push_str = TEXT_PUSH_STR(P, P, I64);
    text_push = TEXT_PUSH(P, S, P);
    text_finish = TEXT_FINISH(P) -> S;
    list_new = LIST_NEW(P, P, I64) -> S;
    list_len = LIST_LEN(S) -> I64;
    list_get = LIST_GET(P, S, I64, U, P) -> B;
    list_push = LIST_PUSH(P, S);
    list_remove = LIST_REMOVE(P, P, I64, U) -> B;
    list_item_mut = LIST_ITEM_MUT(P, P, I64, U, P) -> B;
    struct_new = STRUCT_NEW(P, P) -> S;
    struct_get = STRUCT_GET(S, I64) -> S;
    struct_field_mut = STRUCT_FIELD_MUT(P, I64) -> P;
    state = STATE(P, U) -> S;
    state_mut = STATE_MUT(P, U) -> P;
    enter = ENTER(P, U, U) -> B;
    leave = LEAVE(P);
    fail_divide_by_zero = FAIL_DIVIDE_BY_ZERO(P, U);
    fail_too_large = FAIL_TOO_LARGE(P, U);
    fail_download_into_variable = FAIL_DOWNLOAD_INTO_VARIABLE(P, U);
    show_same = SHOW_SAME(P, U);
    show_item = SHOW_ITEM(P, U) -> P;
    show_view = SHOW_VIEW(P, U) -> P;
    download = DOWNLOAD(P, S, S, P, P, U) -> B;
}
