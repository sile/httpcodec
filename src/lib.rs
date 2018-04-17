extern crate bytecodec;
#[macro_use]
extern crate trackable;

// TODO: delete
use bytecodec::{ErrorKind, Result};

pub use method::Method;
pub use options::DecodeOptions;
pub use request_target::RequestTarget;
pub use version::HttpVersion;

pub mod body;
pub mod header;

pub mod request;

mod method;
mod options;
mod request_target;
mod util;
mod version;
