#![warn(missing_docs)]

// Proc-macros and doc-tests currently have some limitations, so we allow referncing self via
// screen_13:
// https://github.com/rust-lang/cargo/issues/9886
// https://github.com/bkchr/proc-macro-crate/issues/10
#[allow(unused_extern_crates)]
extern crate self as screen_13_macros;

pub mod vertex;
pub use screen_13::driver::ash;
