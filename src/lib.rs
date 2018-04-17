extern crate bytecodec;
#[macro_use]
extern crate trackable;

// TODO: delete
use bytecodec::{ErrorKind, Result};

pub use method::Method;

pub mod body;
pub mod header;

pub mod request;
pub mod version;

mod method;
mod util;
