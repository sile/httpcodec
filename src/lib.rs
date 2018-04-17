extern crate bytecodec;
#[macro_use]
extern crate trackable;

// TODO: delete
use bytecodec::{ErrorKind, Result};

pub use method::Method;
pub use version::HttpVersion;

pub mod body;
pub mod header;

pub mod request;

mod method;
mod util;
mod version;
