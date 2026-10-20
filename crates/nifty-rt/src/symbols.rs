//! Everything compiled code links to, by the name it's exported under. Each function's
//! name comes from its own identifier, so the names can't drift from the exports;
//! [`addresses`] pairs every name with what it names, for loading code in-process.

use crate::abi::{
    nf_enter, nf_equal, nf_fail_divide_by_zero, nf_fail_too_large, nf_float_rem, nf_leave,
    nf_list_get, nf_list_item_mut, nf_list_len, nf_list_new, nf_list_push, nf_list_remove,
    nf_release, nf_retain, nf_state, nf_state_mut, nf_store, nf_string_compare, nf_string_concat,
    nf_string_len, nf_string_new, nf_struct_field_mut, nf_struct_get, nf_struct_new,
    nf_text_finish, nf_text_new, nf_text_push, nf_text_push_str,
};
use crate::dialog::{nf_fail_dialog_into_variable, nf_pick};
use crate::download::{
    nf_download, nf_download_command, nf_fail_download_into_variable, nf_format_bytes,
};
use crate::nifty_rt_main;
use crate::show::{nf_show_item, nf_show_same, nf_show_view};
use crate::value::{TypeDesc, TypeKind};
use crate::window::nf_window;

macro_rules! exports {
    (
        functions { $($function:ident = $symbol:ident,)* }
        types { $($type_name:ident = $type_symbol:literal: $type_static:ident = $kind:ident,)* }
    ) => {
        $(pub const $function: &str = stringify!($symbol);)*

        $(
            pub const $type_name: &str = $type_symbol;
            #[unsafe(export_name = $type_symbol)]
            pub static $type_static: TypeDesc = TypeDesc::scalar(TypeKind::$kind);
        )*

        /// Every export with its address.
        #[must_use]
        pub fn addresses() -> Vec<(&'static str, *const u8)> {
            vec![
                $(($function, $symbol as *const u8),)*
                $(($type_name, (&raw const $type_static).cast::<u8>()),)*
            ]
        }
    };
}

exports! {
    functions {
        MAIN = nifty_rt_main,
        RETAIN = nf_retain,
        RELEASE = nf_release,
        EQUAL = nf_equal,
        STORE = nf_store,
        FLOAT_REM = nf_float_rem,
        STRING_NEW = nf_string_new,
        STRING_CONCAT = nf_string_concat,
        STRING_LEN = nf_string_len,
        STRING_COMPARE = nf_string_compare,
        TEXT_NEW = nf_text_new,
        TEXT_PUSH_STR = nf_text_push_str,
        TEXT_PUSH = nf_text_push,
        TEXT_FINISH = nf_text_finish,
        LIST_NEW = nf_list_new,
        LIST_LEN = nf_list_len,
        LIST_GET = nf_list_get,
        LIST_PUSH = nf_list_push,
        LIST_REMOVE = nf_list_remove,
        LIST_ITEM_MUT = nf_list_item_mut,
        STRUCT_NEW = nf_struct_new,
        STRUCT_GET = nf_struct_get,
        STRUCT_FIELD_MUT = nf_struct_field_mut,
        STATE = nf_state,
        STATE_MUT = nf_state_mut,
        ENTER = nf_enter,
        LEAVE = nf_leave,
        FAIL_DIVIDE_BY_ZERO = nf_fail_divide_by_zero,
        FAIL_TOO_LARGE = nf_fail_too_large,
        SHOW_SAME = nf_show_same,
        SHOW_ITEM = nf_show_item,
        SHOW_VIEW = nf_show_view,
        DOWNLOAD = nf_download,
        FAIL_DOWNLOAD_INTO_VARIABLE = nf_fail_download_into_variable,
        FORMAT_BYTES = nf_format_bytes,
        DOWNLOAD_COMMAND = nf_download_command,
        PICK = nf_pick,
        FAIL_DIALOG_INTO_VARIABLE = nf_fail_dialog_into_variable,
        WINDOW = nf_window,
    }
    types {
        TYPE_INT = "nf_type_int": INT = Int,
        TYPE_FLOAT = "nf_type_float": FLOAT = Float,
        TYPE_BOOL = "nf_type_bool": BOOL = Bool,
        TYPE_STRING = "nf_type_string": STRING = String,
    }
}
