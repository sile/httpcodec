use std::mem;
use std::str;
use bytecodec::{ByteCount, Decode, DecodeExt, Eos};
use bytecodec::combinator::{Buffered, MaxBytes};
use bytecodec::tuple::Tuple3Decoder;

use {ErrorKind, Result};
use body::{BodyDecoder, Unread};
use header::{HeaderDecoder, HeaderField, HeaderFields};
use method::{Method, MethodDecoder};
use version::{HttpVersion, HttpVersionDecoder};
use util;

#[derive(Debug)]
pub struct DecodeOptions {
    pub max_start_line_size: usize,
    pub max_header_size: usize,
}
impl Default for DecodeOptions {
    fn default() -> Self {
        DecodeOptions {
            max_start_line_size: 0xFFFF,
            max_header_size: 0xFFFF,
        }
    }
}

#[derive(Debug)]
pub struct Request<T> {
    buf: Vec<u8>,
    request_line: RequestLine,
    header: Vec<HeaderField>,
    body: T,
}
impl<T> Request<T> {
    pub fn method(&self) -> Method<&str> {
        Method(unsafe { str::from_utf8_unchecked(&self.buf[..self.request_line.method_size]) })
    }

    pub fn request_target(&self) -> RequestTarget<&str> {
        let start = self.request_line.method_size + 1;
        let end = start + self.request_line.request_target_size;
        RequestTarget(unsafe { str::from_utf8_unchecked(&self.buf[start..end]) })
    }

    pub fn http_version(&self) -> HttpVersion {
        self.request_line.http_version
    }

    pub fn header_fields(&self) -> HeaderFields {
        HeaderFields::new(&self.buf, &self.header)
    }

    pub fn body(&self) -> &T {
        &self.body
    }

    pub fn body_mut(&mut self) -> &mut T {
        &mut self.body
    }

    pub fn into_body(self) -> T {
        self.body
    }
}
impl Request<Unread> {
    pub fn start_decoding_body<U: Decode>(&self, _decoder: U) -> Result<BodyDecoder<U>> {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct RequestDecoder<T> {
    buf: Vec<u8>,
    request_line: Buffered<MaxBytes<RequestLineDecoder>>,
    header: Buffered<MaxBytes<HeaderDecoder>>,
    body: T,
}
impl<T: Decode> Decode for RequestDecoder<T> {
    type Item = Request<T::Item>;

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
            if !self.header.has_item() {
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
impl<T: Default> Default for RequestDecoder<T> {
    fn default() -> Self {
        let options = DecodeOptions::default();
        RequestDecoder {
            buf: Vec::new(),
            request_line: RequestLineDecoder::default()
                .max_bytes(options.max_start_line_size as u64)
                .buffered(),
            header: HeaderDecoder::default()
                .max_bytes(options.max_header_size as u64)
                .buffered(),
            body: T::default(),
        }
    }
}

#[derive(Debug)]
struct RequestLine {
    method_size: usize,
    request_target_size: usize,
    http_version: HttpVersion,
}

#[derive(Debug)]
pub struct RequestTarget<B>(B);

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

#[derive(Debug, Default)]
struct RequestTargetDecoder {
    size: usize,
}
impl Decode for RequestTargetDecoder {
    type Item = usize;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        if let Some(n) = buf.iter().position(|b| !util::is_vchar(*b)) {
            track_assert_eq!(buf[n], b' ', ErrorKind::InvalidInput);
            let size = self.size + n;
            self.size = 0;
            Ok((n + 1, Some(size)))
        } else {
            track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
            self.size += buf.len();
            Ok((buf.len(), None))
        }
    }

    fn has_terminated(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}
