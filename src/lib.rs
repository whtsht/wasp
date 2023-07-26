#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(not(feature = "std"))]
pub mod lib {
    #[doc(hidden)]
    #[cfg(not(feature = "alloc"))]
    pub use core::borrow;

    #[cfg(feature = "alloc")]
    #[doc(hidden)]
    pub use alloc::{
        borrow,
        boxed::Box,
        format,
        string::{self, String, ToString},
        vec,
        vec::{IntoIter, Vec},
    };

    #[doc(hidden)]
    pub use core::{
        cmp, convert, fmt, iter, mem, num, ops,
        option::{self, Option},
        result::{self, Result},
        slice, str,
    };
}

pub mod binary;
pub mod exec;
pub mod loader;
