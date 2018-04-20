use std::fmt;
use std::str;
use bytecodec::{ByteCount, Decode, Encode, Eos, ExactBytesEncode, Result};
use bytecodec::tuple::Tuple4Decoder;

use body::{BodyDecode, BodyEncode};
use header::{Header, HeaderFieldPosition, HeaderMut};
use message::{Message, MessageDecoder, MessageEncoder};
use method::{Method, MethodDecoder};
use options::DecodeOptions;
use request_target::{RequestTarget, RequestTargetDecoder};
use util::CrlfDecoder;
use version::{HttpVersion, HttpVersionDecoder};

/// HTTP request message.
#[derive(Debug)]
pub struct Request<T> {
    buf: Vec<u8>,
    request_line: RequestLine,
    header: Vec<HeaderFieldPosition>,
    body: T,
}
impl<T> Request<T> {
    /// Makes a new `Request` instance with the given request-line components and body.
    pub fn new(method: Method, target: RequestTarget, version: HttpVersion, body: T) -> Self {
        let mut buf = Vec::new();
        buf.extend_from_slice(method.as_str().as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(target.as_str().as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(version.as_str().as_bytes());
        buf.extend_from_slice(b"\r\n");

        let request_line = RequestLine {
            method_size: method.as_str().len(),
            request_target_size: target.as_str().len(),
            http_version: version,
        };

        Request {
            buf,
            request_line,
            header: Vec::new(),
            body,
        }
    }

    /// Returns the method of the request.
    pub fn method(&self) -> Method {
        unsafe {
            Method::new_unchecked(str::from_utf8_unchecked(
                &self.buf[..self.request_line.method_size],
            ))
        }
    }

    /// Returns the target of the request.
    pub fn request_target(&self) -> RequestTarget {
        let start = self.request_line.method_size + 1;
        let end = start + self.request_line.request_target_size;
        unsafe { RequestTarget::new_unchecked(str::from_utf8_unchecked(&self.buf[start..end])) }
    }

    /// Returns the HTTP version of the request.
    pub fn http_version(&self) -> HttpVersion {
        self.request_line.http_version
    }

    /// Returns the header of the request.
    pub fn header(&self) -> Header {
        Header::new(&self.buf, &self.header)
    }

    /// Returns the mutable header of the request.
    pub fn header_mut(&mut self) -> HeaderMut {
        HeaderMut::new(&mut self.buf, &mut self.header)
    }

    /// Returns a reference to the body of the request.
    pub fn body(&self) -> &T {
        &self.body
    }

    /// Returns a mutable reference to the body of the request.
    pub fn body_mut(&mut self) -> &mut T {
        &mut self.body
    }

    /// Takes ownership of the request, and returns its body.
    pub fn into_body(self) -> T {
        self.body
    }
}
impl<T: fmt::Display> fmt::Display for Request<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "{} {} {}\r",
            self.method(),
            self.request_target(),
            self.http_version()
        )?;
        write!(f, "{}", self.header())?;
        write!(f, "{}", self.body)?;
        Ok(())
    }
}

/// HTTP request decoder.
#[derive(Debug)]
pub struct RequestDecoder<D>(MessageDecoder<RequestLineDecoder, D>);
impl<D: BodyDecode> RequestDecoder<D> {
    /// Make a new `RequestDecoder` instance.
    pub fn new(body_decoder: D) -> Self {
        Self::with_options(body_decoder, DecodeOptions::default())
    }

    /// Make a new `RequestDecoder` instance with the given options.
    pub fn with_options(body_decoder: D, options: DecodeOptions) -> Self {
        let inner = MessageDecoder::new(RequestLineDecoder::default(), body_decoder, options);
        RequestDecoder(inner)
    }
}
impl<D: BodyDecode> Decode for RequestDecoder<D> {
    type Item = Request<D::Item>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let (size, item) = track!(self.0.decode(buf, eos))?;
        let item = item.map(|m| Request {
            buf: m.buf,
            request_line: m.start_line,
            header: m.header,
            body: m.body,
        });
        Ok((size, item))
    }

    fn has_terminated(&self) -> bool {
        self.0.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}
impl<D: Default + BodyDecode> Default for RequestDecoder<D> {
    fn default() -> Self {
        Self::new(D::default())
    }
}

#[derive(Debug)]
struct RequestLine {
    method_size: usize,
    request_target_size: usize,
    http_version: HttpVersion,
}

#[derive(Debug, Default)]
struct RequestLineDecoder(
    Tuple4Decoder<MethodDecoder, RequestTargetDecoder, HttpVersionDecoder, CrlfDecoder>,
);
impl Decode for RequestLineDecoder {
    type Item = RequestLine;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let (size, item) = track!(self.0.decode(buf, eos))?;
        let item = item.map(|t| RequestLine {
            method_size: t.0,
            request_target_size: t.1,
            http_version: t.2,
        });
        Ok((size, item))
    }

    fn has_terminated(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}

/// HTTP request encoder.
#[derive(Debug, Default)]
pub struct RequestEncoder<E>(MessageEncoder<E>);
impl<E: BodyEncode> RequestEncoder<E> {
    /// Makes a new `RequestEncoder` instance.
    pub fn new(body_encoder: E) -> Self {
        RequestEncoder(MessageEncoder::new(body_encoder))
    }
}
impl<E: BodyEncode> Encode for RequestEncoder<E> {
    type Item = Request<E::Item>;

    fn encode(&mut self, buf: &mut [u8], eos: Eos) -> Result<usize> {
        track!(self.0.encode(buf, eos))
    }

    fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
        let item = Message {
            buf: item.buf,
            start_line: (),
            header: item.header,
            body: item.body,
        };
        track!(self.0.start_encoding(item))
    }

    fn is_idle(&self) -> bool {
        self.0.is_idle()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}
impl<E: ExactBytesEncode + BodyEncode> ExactBytesEncode for RequestEncoder<E> {
    fn exact_requiring_bytes(&self) -> u64 {
        self.0.exact_requiring_bytes()
    }
}

#[cfg(test)]
mod test {
    use std::str;
    use bytecodec::EncodeExt;
    use bytecodec::bytes::{BytesEncoder, RemainingBytesDecoder, Utf8Decoder};
    use bytecodec::io::{IoDecodeExt, IoEncodeExt};

    use {BodyDecoder, BodyEncoder, HttpVersion, Method, RequestTarget};
    use super::*;

    #[test]
    fn request_encoder_works() {
        let request = Request::new(
            Method::new("GET").unwrap(),
            RequestTarget::new("/foo").unwrap(),
            HttpVersion::V1_1,
            b"barbaz",
        );
        let mut encoder =
            RequestEncoder::<BodyEncoder<BytesEncoder<_>>>::with_item(request).unwrap();

        let mut buf = Vec::new();
        track_try_unwrap!(encoder.encode_all(&mut buf));
        assert_eq!(
            str::from_utf8(&buf).ok(),
            Some("GET /foo HTTP/1.1\r\ncontent-length: 6\r\n\r\nbarbaz")
        );
    }

    #[test]
    fn request_decoder_works() {
        let mut decoder =
            RequestDecoder::<BodyDecoder<Utf8Decoder<RemainingBytesDecoder>>>::default();
        let item = track_try_unwrap!(
            decoder.decode_exact(b"GET /foo HTTP/1.1\r\ncontent-length: 6\r\n\r\nbarbaz".as_ref())
        );
        assert_eq!(
            item.to_string(),
            "GET /foo HTTP/1.1\r\ncontent-length: 6\r\n\r\nbarbaz"
        );
        assert_eq!(item.method().as_str(), "GET");
        assert_eq!(item.request_target().as_str(), "/foo");
        assert_eq!(item.http_version(), HttpVersion::V1_1);
        assert_eq!(
            item.header()
                .fields()
                .map(|f| (f.name().to_owned(), f.value().to_owned()))
                .collect::<Vec<_>>(),
            vec![("content-length".to_owned(), "6".to_owned())]
        );
        assert_eq!(item.body(), "barbaz");
    }
}
