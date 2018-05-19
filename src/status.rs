use bytecodec::bytes::CopyableBytesDecoder;
use bytecodec::{ByteCount, Decode, Eos, Error, ErrorKind, Result};
use std;
use std::fmt;
use std::str;
use trackable::error::ErrorKindExt;

use util;

/// Status code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StatusCode(u16);
impl StatusCode {
    /// Makes a new `StatusCode` instance.
    ///
    /// # Errors
    ///
    /// `code` must be a integer between 200 and 999.
    /// Otherwise it will return an `ErrorKind::InvalidInput` error.
    pub fn new(code: u16) -> Result<Self> {
        track_assert!(100 <= code && code < 1000, ErrorKind::InvalidInput; code);
        Ok(StatusCode(code))
    }

    /// Makes a new `StatusCode` instance without any validation.
    pub unsafe fn new_unchecked(code: u16) -> Self {
        StatusCode(code)
    }

    /// Returns the status code as an `u16` value.
    pub fn as_u16(&self) -> u16 {
        self.0
    }

    pub(crate) fn as_bytes(&self) -> [u8; 3] {
        let a = ((self.0 / 100) % 10) as u8;
        let b = ((self.0 / 10) % 10) as u8;
        let c = (self.0 % 10) as u8;
        [a + b'0', b + b'0', c + b'0']
    }
}
impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Default)]
pub struct StatusCodeDecoder {
    code: CopyableBytesDecoder<[u8; 3]>,
    idle: bool,
}
impl Decode for StatusCodeDecoder {
    type Item = StatusCode;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        let mut offset = 0;
        bytecodec_try_decode!(self.code, offset, buf, eos);
        if offset < buf.len() {
            track_assert_eq!(buf[offset] as char, ' ', ErrorKind::InvalidInput);
            self.idle = true;
            return Ok(offset + 1);
        }
        track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
        Ok(offset)
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        let code = track!(self.code.finish_decoding())?;
        let code = track!(str::from_utf8(&code).map_err(into_invalid_input); code)?;
        let code = track!(code.parse().map_err(into_invalid_input); code)?;
        let code = track!(StatusCode::new(code))?;
        self.idle = false;
        Ok(code)
    }

    fn requiring_bytes(&self) -> ByteCount {
        if self.idle {
            ByteCount::Finite(0)
        } else {
            ByteCount::Finite(1).add_for_decoding(self.code.requiring_bytes())
        }
    }

    fn is_idle(&self) -> bool {
        self.idle
    }
}

/// Reason phrase of a response status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReasonPhrase<'a>(&'a str);
impl<'a> ReasonPhrase<'a> {
    /// Makes a new `ReasonPhrase` instance.
    ///
    /// # Errors
    ///
    /// `phrase` must be composed of whitespaces (i.e., " " or "\t") or
    /// "VCHAR" characters that defined in [RFC 7230].
    /// If it contains any other characters,
    /// an `ErrorKind::InvalidInput` error will be returned.
    ///
    /// [RFC 7230]: https://tools.ietf.org/html/rfc7230
    pub fn new(phrase: &'a str) -> Result<Self> {
        track_assert!(phrase.bytes().all(is_phrase_char), ErrorKind::InvalidInput);
        Ok(ReasonPhrase(phrase))
    }

    /// Makes a new `ReasonPhrase` instance without any validation.
    pub unsafe fn new_unchecked(phrase: &'a str) -> Self {
        ReasonPhrase(phrase)
    }

    /// Returns a reference to the phrase string.
    pub fn as_str(&self) -> &'a str {
        self.0
    }
}
impl<'a> AsRef<str> for ReasonPhrase<'a> {
    fn as_ref(&self) -> &str {
        self.0
    }
}
impl<'a> fmt::Display for ReasonPhrase<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Default)]
pub struct ReasonPhraseDecoder {
    size: usize,
    remaining: ByteCount,
}
impl Decode for ReasonPhraseDecoder {
    type Item = usize;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        if self.is_idle() {
            return Ok(0);
        }

        let mut offset = 0;
        if self.remaining == ByteCount::Unknown {
            if let Some(n) = buf.iter().position(|b| !is_phrase_char(*b)) {
                track_assert_eq!(buf[n] as char, '\r', ErrorKind::InvalidInput);
                self.size += n;
                self.remaining = ByteCount::Finite(1);
                offset = n + 1;
            } else {
                self.size += buf.len();
                offset = buf.len();
            }
        }
        if self.remaining == ByteCount::Finite(1) && offset < buf.len() {
            track_assert_eq!(buf[offset] as char, '\n', ErrorKind::InvalidInput);
            self.remaining = ByteCount::Finite(0);
            Ok(offset + 1)
        } else {
            track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
            Ok(offset)
        }
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        track_assert_eq!(
            self.remaining,
            ByteCount::Finite(0),
            ErrorKind::IncompleteDecoding
        );
        let size = self.size;
        self.size = 0;
        self.remaining = ByteCount::Unknown;
        Ok(size)
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.remaining
    }

    fn is_idle(&self) -> bool {
        self.remaining == ByteCount::Finite(0)
    }
}

fn is_phrase_char(b: u8) -> bool {
    util::is_vchar(b) || util::is_whitespace(b)
}

#[cfg(test)]
mod test {
    use bytecodec::ErrorKind;
    use bytecodec::io::IoDecodeExt;

    use super::*;

    #[test]
    fn status_code_decoder_works() {
        let mut decoder = StatusCodeDecoder::default();
        let item = track_try_unwrap!(decoder.decode_exact(b"200 OK\r\n".as_ref()));
        assert_eq!(item, StatusCode(200));

        assert_eq!(
            decoder
                .decode_exact(b"90 \r\n".as_ref())
                .err()
                .map(|e| *e.kind()),
            Some(ErrorKind::InvalidInput)
        );

        let mut decoder = StatusCodeDecoder::default();
        assert_eq!(
            decoder
                .decode_exact(b"1000 ".as_ref())
                .err()
                .map(|e| *e.kind()),
            Some(ErrorKind::InvalidInput)
        );

        let mut decoder = StatusCodeDecoder::default();
        assert_eq!(
            decoder
                .decode_exact(b"10a ".as_ref())
                .err()
                .map(|e| *e.kind()),
            Some(ErrorKind::InvalidInput)
        );

        let mut decoder = StatusCodeDecoder::default();
        assert_eq!(
            decoder
                .decode_exact(b"200\r\n".as_ref())
                .err()
                .map(|e| *e.kind()),
            Some(ErrorKind::InvalidInput)
        );

        let mut decoder = StatusCodeDecoder::default();
        assert_eq!(
            decoder
                .decode_exact(b"200".as_ref())
                .err()
                .map(|e| *e.kind()),
            Some(ErrorKind::UnexpectedEos)
        );
    }

    #[test]
    fn reason_phrase_decoder_works() {
        let mut decoder = ReasonPhraseDecoder::default();
        let item = track_try_unwrap!(decoder.decode_exact(b"Not Found\r\n".as_ref()));
        assert_eq!(item, 9);

        assert_eq!(
            decoder
                .decode_exact(b"Not\rFound".as_ref())
                .err()
                .map(|e| *e.kind()),
            Some(ErrorKind::InvalidInput)
        )
    }
}

fn into_invalid_input<E: std::error::Error + Send + Sync + 'static>(e: E) -> Error {
    ErrorKind::InvalidInput.cause(e).into()
}
