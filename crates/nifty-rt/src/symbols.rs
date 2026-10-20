//! The names compiled code links to, which must match the runtime's exports. The link test
//! fails when one it uses doesn't.

pub const MAIN: &str = "nifty_rt_main";
pub const TYPE_INT: &str = "nf_type_int";
pub const FAIL_DIVIDE_BY_ZERO: &str = "nf_fail_divide_by_zero";
