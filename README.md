httpcodec
=========

[![httpcodec](http://meritbadge.herokuapp.com/httpcodec)](https://crates.io/crates/httpcodec)
[![Documentation](https://docs.rs/httpcodec/badge.svg)](https://docs.rs/httpcodec)
[![Build Status](https://travis-ci.org/sile/httpcodec.svg?branch=master)](https://travis-ci.org/sile/httpcodec)
[![Code Coverage](https://codecov.io/gh/sile/httpcodec/branch/master/graph/badge.svg)](https://codecov.io/gh/sile/httpcodec/branch/master)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Encoders and decoders for HTTP/1.x messages based on [bytecodec] crate.

[Documentation](https://docs.rs/httpcodec)

[bytecodec]: https://crates.io/crates/bytecodec

Examples
--------

Encodes a HTTP request message:

```rust
use bytecodec::Encode;
use bytecodec::bytes::BytesEncoder;
use bytecodec::io::IoEncodeExt;
use httpcodec::{BodyEncoder, HttpVersion, Method, Request, RequestEncoder, RequestTarget};

let request = Request::new(
    Method::new("GET").unwrap(),
    RequestTarget::new("/foo").unwrap(),
    HttpVersion::V1_1,
    b"barbaz",
);

let mut encoder = RequestEncoder::new(BodyEncoder::new(BytesEncoder::new()));
encoder.start_encoding(request).unwrap();

let mut buf = Vec::new();
encoder.encode_all(&mut buf).unwrap();
assert_eq!(buf, "GET /foo HTTP/1.1\r\ncontent-length: 6\r\n\r\nbarbaz".as_bytes());
```

Decodes a HTTP response message:

```
use bytecodec::bytes::RemainingBytesDecoder;
use bytecodec::io::IoDecodeExt;
use httpcodec::{BodyDecoder, HttpVersion, ResponseDecoder};

let mut decoder =
    ResponseDecoder::<BodyDecoder<RemainingBytesDecoder>>::default();

let input = b"HTTP/1.0 200 OK\r\ncontent-length: 6\r\n\r\nbarbaz";
let response = decoder.decode_exact(input.as_ref()).unwrap();

assert_eq!(response.http_version(), HttpVersion::V1_0);
assert_eq!(response.status_code().as_u16(), 200);
assert_eq!(response.reason_phrase().as_str(), "OK");
assert_eq!(
    response.header()
        .fields()
        .map(|f| (f.name().to_owned(), f.value().to_owned()))
        .collect::<Vec<_>>(),
    vec![("content-length".to_owned(), "6".to_owned())]
);
assert_eq!(response.body(), b"barbaz");
```

References
----------

- [RFC 7230] Hypertext Transfer Protocol (HTTP/1.1): Message Syntax and Routing

[RFC 7230]: https://tools.ietf.org/html/rfc7230
