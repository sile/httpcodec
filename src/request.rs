use std::fmt;
use std::mem;
use std::str;
use bytecodec::{ByteCount, Decode, DecodeExt, Encode, Eos, ErrorKind, ExactBytesEncode, Result};
use bytecodec::bytes::BytesEncoder;
use bytecodec::combinator::{Buffered, MaxBytes};
use bytecodec::tuple::Tuple3Decoder;

use body::{BodyDecode, BodyEncode};
use header::{Header, HeaderDecoder, HeaderFieldPosition, HeaderMut};
use method::{Method, MethodDecoder};
use options::DecodeOptions;
use request_target::{RequestTarget, RequestTargetDecoder};
use version::{HttpVersion, HttpVersionDecoder};

/// HTTP request.
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
        write!(
            f,
            "{} {} {}\r\n",
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
pub struct RequestDecoder<D> {
    buf: Vec<u8>,
    request_line: Buffered<MaxBytes<RequestLineDecoder>>,
    header: Buffered<MaxBytes<HeaderDecoder>>,
    body: D,
}
impl<D: BodyDecode> RequestDecoder<D> {
    /// Make a new `RequestDecoder` instance.
    pub fn new(body_decoder: D) -> Self {
        Self::with_options(body_decoder, DecodeOptions::default())
    }

    /// Make a new `RequestDecoder` instance with the given options.
    pub fn with_options(body_decoder: D, options: DecodeOptions) -> Self {
        RequestDecoder {
            buf: Vec::new(),
            request_line: RequestLineDecoder::default()
                .max_bytes(options.max_start_line_size as u64)
                .buffered(),
            header: HeaderDecoder::default()
                .max_bytes(options.max_header_size as u64)
                .buffered(),
            body: body_decoder,
        }
    }
}
impl<D: BodyDecode> Decode for RequestDecoder<D> {
    type Item = Request<D::Item>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        if !self.request_line.has_item() {
            offset = track!(self.request_line.decode(buf, eos))?.0;
            if !self.request_line.has_item() {
                self.buf.extend_from_slice(&buf[..offset]);
                return Ok((offset, None));
            } else {
                self.header
                    .inner_mut()
                    .inner_mut()
                    .set_start_position(self.buf.len());
            }
        }

        if !self.header.has_item() {
            offset += track!(self.header.decode(&buf[offset..], eos))?.0;
            self.buf.extend_from_slice(&buf[..offset]);
            if let Some(header) = self.header.get_item() {
                track!(self.body.initialize(&Header::new(&self.buf, header)))?;
            } else {
                return Ok((offset, None));
            }
        }

        let (size, item) = track!(self.body.decode(&buf[offset..], eos))?;
        offset += size;
        let item = item.map(|body| {
            let buf = mem::replace(&mut self.buf, Vec::new());
            let request_line = self.request_line.take_item().expect("Never fails");
            let header = self.header.take_item().expect("Never fails");
            Request {
                buf,
                request_line,
                header,
                body,
            }
        });
        Ok((offset, item))
    }

    fn has_terminated(&self) -> bool {
        self.body.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        if self.header.has_item() {
            self.body.requiring_bytes()
        } else {
            ByteCount::Unknown
        }
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
struct RequestLineDecoder(Tuple3Decoder<MethodDecoder, RequestTargetDecoder, HttpVersionDecoder>);
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
pub struct RequestEncoder<E> {
    before_body: BytesEncoder<Vec<u8>>,
    body: E,
}
impl<E: BodyEncode> RequestEncoder<E> {
    /// Makes a new `RequestEncoder` instance.
    pub fn new(body_encoder: E) -> Self {
        RequestEncoder {
            before_body: BytesEncoder::new(),
            body: body_encoder,
        }
    }
}
impl<E: BodyEncode> Encode for RequestEncoder<E> {
    type Item = Request<E::Item>;

    fn encode(&mut self, buf: &mut [u8], eos: Eos) -> Result<usize> {
        let mut offset = 0;
        if !self.before_body.is_idle() {
            track!(self.before_body.encode(buf, eos))?;
            if !self.before_body.is_idle() {
                return Ok(offset);
            }
        }
        offset += track!(self.body.encode(&mut buf[offset..], eos))?;
        Ok(offset)
    }

    fn start_encoding(&mut self, mut item: Self::Item) -> Result<()> {
        track_assert!(self.is_idle(), ErrorKind::EncoderFull);
        track!(self.body.start_encoding(item.body))?;
        {
            self.body
                .update_header(&mut HeaderMut::new(&mut item.buf, &mut item.header));
        }
        track!(self.before_body.start_encoding(item.buf))?;
        Ok(())
    }

    fn is_idle(&self) -> bool {
        self.before_body.is_idle() && self.body.is_idle()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.before_body
            .requiring_bytes()
            .add_for_encoding(self.body.requiring_bytes())
    }
}
impl<E: ExactBytesEncode + BodyEncode> ExactBytesEncode for RequestEncoder<E> {
    fn exact_requiring_bytes(&self) -> u64 {
        self.before_body.exact_requiring_bytes() + self.body.exact_requiring_bytes()
    }
}
