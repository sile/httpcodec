extern crate bytecodec;
#[macro_use]
extern crate trackable;

pub use bytecodec::{Error, ErrorKind, Result};

pub mod method;
pub mod token;
