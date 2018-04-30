//! Encoders and decoders for HTTP/1.x messages based on [bytecodec] crate.
//!
//! [bytecodec]: https://crates.io/crates/bytecodec
//!
//! # Examples
//!
//! Encodes a HTTP request message:
//!
//! ```
//! # extern crate bytecodec;
//! # extern crate httpcodec;
//! use bytecodec::Encode;
//! use bytecodec::bytes::BytesEncoder;
//! use bytecodec::io::IoEncodeExt;
//! use httpcodec::{BodyEncoder, HttpVersion, Method, Request, RequestEncoder, RequestTarget};
//!
//! # fn main() {
//! let request = Request::new(
//!     Method::new("GET").unwrap(),
//!     RequestTarget::new("/foo").unwrap(),
//!     HttpVersion::V1_1,
//!     b"barbaz",
//! );
//!
//! let mut encoder = RequestEncoder::new(BodyEncoder::new(BytesEncoder::new()));
//! encoder.start_encoding(request).unwrap();
//!
//! let mut buf = Vec::new();
//! encoder.encode_all(&mut buf).unwrap();
//! assert_eq!(buf, "GET /foo HTTP/1.1\r\nContent-Length: 6\r\n\r\nbarbaz".as_bytes());
//! # }
//! ```
//!
//! Decodes a HTTP response message:
//!
//! ```
//! # extern crate bytecodec;
//! # extern crate httpcodec;
//! use bytecodec::bytes::RemainingBytesDecoder;
//! use bytecodec::io::IoDecodeExt;
//! use httpcodec::{BodyDecoder, HttpVersion, ResponseDecoder};
//!
//! # fn main() {
//! let mut decoder =
//!     ResponseDecoder::<BodyDecoder<RemainingBytesDecoder>>::default();
//!
//! let input = b"HTTP/1.0 200 OK\r\nContent-Length: 6\r\n\r\nbarbaz";
//! let response = decoder.decode_exact(input.as_ref()).unwrap();
//!
//! assert_eq!(response.http_version(), HttpVersion::V1_0);
//! assert_eq!(response.status_code().as_u16(), 200);
//! assert_eq!(response.reason_phrase().as_str(), "OK");
//! assert_eq!(
//!     response.header()
//!         .fields()
//!         .map(|f| (f.name().to_owned(), f.value().to_owned()))
//!         .collect::<Vec<_>>(),
//!     vec![("Content-Length".to_owned(), "6".to_owned())]
//! );
//! assert_eq!(response.body(), b"barbaz");
//! # }
//! ```
//!
//! # References
//!
//! - [RFC 7230] Hypertext Transfer Protocol (HTTP/1.1): Message Syntax and Routing
//!
//! [RFC 7230]: https://tools.ietf.org/html/rfc7230
#![warn(missing_docs)]
extern crate bytecodec;
#[macro_use]
extern crate trackable;

pub use body::{BodyDecode, BodyDecoder, BodyEncode, BodyEncoder, HeadBodyEncoder, NoBodyDecoder,
               NoBodyEncoder};
pub use header::{Header, HeaderField, HeaderFields, HeaderMut};
pub use method::Method;
pub use options::DecodeOptions;
pub use request::{Request, RequestDecoder, RequestEncoder};
pub use request_target::RequestTarget;
pub use response::{Response, ResponseDecoder, ResponseEncoder};
pub use status::{ReasonPhrase, StatusCode};
pub use version::HttpVersion;

mod body;
mod chunked_body;
mod header;
mod message;
mod method;
mod options;
mod request;
mod request_target;
mod response;
mod status;
mod util;
mod version;
