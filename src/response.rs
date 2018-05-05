use std::fmt;
use std::str;
use bytecodec::{ByteCount, Decode, Encode, Eos, ExactBytesEncode, Result};
use bytecodec::tuple::Tuple4Decoder;

use {BodyDecode, BodyEncode, DecodeOptions, Header, HeaderMut, HttpVersion, ReasonPhrase,
     StatusCode};
use header::HeaderFieldPosition;
use message::{Message, MessageDecoder, MessageEncoder};
use status::{ReasonPhraseDecoder, StatusCodeDecoder};
use util::SpaceDecoder;
use version::HttpVersionDecoder;

/// HTTP response message.
#[derive(Debug)]
pub struct Response<T> {
    buf: Vec<u8>,
    status_line: StatusLine,
    header: Vec<HeaderFieldPosition>,
    body: T,
}
impl<T> Response<T> {
    /// Makes a new `Response` instance with the given status-line components and body.
    pub fn new(version: HttpVersion, status: StatusCode, reason: ReasonPhrase, body: T) -> Self {
        let mut buf = Vec::new();
        buf.extend_from_slice(version.as_str().as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(&status.as_bytes()[..]);
        buf.push(b' ');
        buf.extend_from_slice(reason.as_str().as_bytes());
        buf.extend_from_slice(b"\r\n");

        let status_line = StatusLine {
            http_version: version,
            status_code: status,
            reason_phrase_size: reason.as_str().len(),
        };

        Response {
            buf,
            status_line,
            header: Vec::new(),
            body,
        }
    }

    /// Returns the HTTP version of the response.
    pub fn http_version(&self) -> HttpVersion {
        self.status_line.http_version
    }

    /// Returns the status code of the response.
    pub fn status_code(&self) -> StatusCode {
        self.status_line.status_code
    }

    /// Returns the reason phrase of the response.
    pub fn reason_phrase(&self) -> ReasonPhrase {
        let start = 8 /* version */ + 1 + 3 /* status */ + 1;
        let end = start + self.status_line.reason_phrase_size;
        unsafe { ReasonPhrase::new_unchecked(str::from_utf8_unchecked(&self.buf[start..end])) }
    }

    /// Returns the header of the response.
    pub fn header(&self) -> Header {
        Header::new(&self.buf, &self.header)
    }

    /// Returns the mutable header of the response.
    pub fn header_mut(&mut self) -> HeaderMut {
        HeaderMut::new(&mut self.buf, &mut self.header)
    }

    /// Returns a reference to the body of the response.
    pub fn body(&self) -> &T {
        &self.body
    }

    /// Returns a mutable reference to the body of the response.
    pub fn body_mut(&mut self) -> &mut T {
        &mut self.body
    }

    /// Converts the body of the response to `U` by using the given function.
    pub fn map_body<U, F>(self, f: F) -> Response<U>
    where
        F: FnOnce(T) -> U,
    {
        let body = f(self.body);
        Response {
            buf: self.buf,
            status_line: self.status_line,
            header: self.header,
            body,
        }
    }

    /// Takes ownership of the response, and returns its body.
    pub fn into_body(self) -> T {
        self.body
    }

    /// Splits the head part and the body part of the response.
    pub fn take_body(self) -> (Response<()>, T) {
        let res = Response {
            buf: self.buf,
            status_line: self.status_line,
            header: self.header,
            body: (),
        };
        (res, self.body)
    }
}
impl<T: fmt::Display> fmt::Display for Response<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "{} {} {}\r",
            self.http_version(),
            self.status_code(),
            self.reason_phrase(),
        )?;
        write!(f, "{}", self.header())?;
        write!(f, "{}", self.body)?;
        Ok(())
    }
}

#[derive(Debug)]
struct StatusLine {
    http_version: HttpVersion,
    status_code: StatusCode,
    reason_phrase_size: usize,
}

#[derive(Debug, Default)]
struct StatusLineDecoder(
    Tuple4Decoder<HttpVersionDecoder, SpaceDecoder, StatusCodeDecoder, ReasonPhraseDecoder>,
);
impl Decode for StatusLineDecoder {
    type Item = StatusLine;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let (size, item) = track!(self.0.decode(buf, eos))?;
        let item = item.map(|t| StatusLine {
            http_version: t.0,
            status_code: t.2,
            reason_phrase_size: t.3,
        });
        Ok((size, item))
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}

/// HTTP response decoder.
#[derive(Debug)]
pub struct ResponseDecoder<D>(MessageDecoder<StatusLineDecoder, D>);
impl<D: BodyDecode> ResponseDecoder<D> {
    /// Make a new `ResponseDecoder` instance.
    pub fn new(body_decoder: D) -> Self {
        Self::with_options(body_decoder, DecodeOptions::default())
    }

    /// Make a new `ResponseDecoder` instance with the given options.
    pub fn with_options(body_decoder: D, options: DecodeOptions) -> Self {
        let inner = MessageDecoder::new(StatusLineDecoder::default(), body_decoder, options);
        ResponseDecoder(inner)
    }
}
impl<D: BodyDecode> Decode for ResponseDecoder<D> {
    type Item = Response<D::Item>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let (size, item) = track!(self.0.decode(buf, eos))?;
        let item = item.map(|m| Response {
            buf: m.buf,
            status_line: m.start_line,
            header: m.header,
            body: m.body,
        });
        Ok((size, item))
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}
impl<D: Default + BodyDecode> Default for ResponseDecoder<D> {
    fn default() -> Self {
        Self::new(D::default())
    }
}

/// HTTP response encoder.
#[derive(Debug, Default)]
pub struct ResponseEncoder<E>(MessageEncoder<E>);
impl<E: BodyEncode> ResponseEncoder<E> {
    /// Makes a new `ResponseEncoder` instance.
    pub fn new(body_encoder: E) -> Self {
        ResponseEncoder(MessageEncoder::new(body_encoder))
    }
}
impl<E: BodyEncode> Encode for ResponseEncoder<E> {
    type Item = Response<E::Item>;

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
impl<E: ExactBytesEncode + BodyEncode> ExactBytesEncode for ResponseEncoder<E> {
    fn exact_requiring_bytes(&self) -> u64 {
        self.0.exact_requiring_bytes()
    }
}

#[cfg(test)]
mod test {
    use bytecodec::EncodeExt;
    use bytecodec::bytes::{BytesEncoder, RemainingBytesDecoder, Utf8Decoder};
    use bytecodec::io::{IoDecodeExt, IoEncodeExt};

    use {BodyDecoder, BodyEncoder, HttpVersion, ReasonPhrase, StatusCode};
    use super::*;

    #[test]
    fn response_encoder_works() {
        let response = Response::new(
            HttpVersion::V1_0,
            StatusCode::new(200).unwrap(),
            ReasonPhrase::new("OK").unwrap(),
            b"barbaz",
        );
        let mut encoder =
            ResponseEncoder::<BodyEncoder<BytesEncoder<_>>>::with_item(response).unwrap();

        let mut buf = Vec::new();
        track_try_unwrap!(encoder.encode_all(&mut buf));
        assert_eq!(
            buf,
            b"HTTP/1.0 200 OK\r\nContent-Length: 6\r\n\r\nbarbaz".as_ref()
        );
    }

    #[test]
    fn response_decoder_works() {
        let mut decoder =
            ResponseDecoder::<BodyDecoder<Utf8Decoder<RemainingBytesDecoder>>>::default();
        let item = track_try_unwrap!(
            decoder.decode_exact(b"HTTP/1.0 200 OK\r\nContent-Length: 6\r\n\r\nbarbaz".as_ref())
        );
        assert_eq!(
            item.to_string(),
            "HTTP/1.0 200 OK\r\nContent-Length: 6\r\n\r\nbarbaz"
        );
        assert_eq!(item.http_version(), HttpVersion::V1_0);
        assert_eq!(item.status_code().as_u16(), 200);
        assert_eq!(item.reason_phrase().as_str(), "OK");
        assert_eq!(
            item.header()
                .fields()
                .map(|f| (f.name().to_owned(), f.value().to_owned()))
                .collect::<Vec<_>>(),
            vec![("Content-Length".to_owned(), "6".to_owned())]
        );
        assert_eq!(item.body(), "barbaz");
    }
}
