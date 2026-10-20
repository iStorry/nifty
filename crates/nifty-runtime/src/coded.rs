//! Enums compiled apps store by number.

/// Declares a fieldless enum whose variants have stable numeric codes, numbered in
/// declaration order, which is how compiled apps store styles.
macro_rules! coded_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident,)+
        }
    ) => {
        $(#[$meta])*
        #[repr(u8)]
        pub enum $name {
            $($(#[$variant_meta])* $variant,)+
        }

        impl $name {
            pub const ALL: &[Self] = &[$(Self::$variant,)+];

            #[must_use]
            pub fn code(self) -> u8 {
                self as u8
            }

            #[must_use]
            pub fn from_code(code: u8) -> Option<Self> {
                Self::ALL.get(usize::from(code)).copied()
            }
        }
    };
}
