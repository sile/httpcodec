use bytecodec::bytes::CopyableBytesDecoder;
use bytecodec::tuple::TupleDecoder;
use bytecodec::{ByteCount, Decode, Eos, ErrorKind, Result};
use std;
use std::fmt;
use std::iter::{DoubleEndedIterator, ExactSizeIterator};
use std::mem;
use std::ops::Range;
use std::slice;
use std::str::{self, FromStr};

use util;

/// HTTP header.
#[derive(Debug)]
pub struct Header<'a> {
    buf: &'a [u8],
    fields: &'a [HeaderFieldPosition],
}
impl<'a> Header<'a> {
    /// Returns an iterator over the fields in the header.
    pub fn fields(&self) -> HeaderFields {
        HeaderFields::new(self.buf, self.fields)
    }

    /// Returns the value of the first field that has the name `name` in the header.
    ///
    /// Note that header names are compared by using `str::eq_ignore_ascii_case` method.
    pub fn get_field(&self, name: &str) -> Option<&str> {
        self.fields()
            .find(|f| f.name().eq_ignore_ascii_case(name))
            .map(|f| f.value())
    }

    /// Returns the parsed result of the value of the first field that
    /// has the name `name` in the header.
    ///
    /// Note that header names are compared by using `str::eq_ignore_ascii_case` method.
    pub fn parse_field<T>(&self, name: &str) -> std::result::Result<Option<T>, T::Err>
    where
        T: FromStr,
    {
        match self.get_field(name).map(|v| v.parse()) {
            None => Ok(None),
            Some(Err(e)) => Err(e),
            Some(Ok(v)) => Ok(Some(v)),
        }
    }

    pub(crate) fn new(buf: &'a [u8], fields: &'a [HeaderFieldPosition]) -> Self {
        Header { buf, fields }
    }
}
impl<'a> fmt::Display for Header<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for field in self.fields() {
            writeln!(f, "{}\r", field)?;
        }
        writeln!(f, "\r")?;
        Ok(())
    }
}

/// Mutable HTTP header.
#[derive(Debug)]
pub struct HeaderMut<'a> {
    buf: &'a mut Vec<u8>,
    fields: &'a mut Vec<HeaderFieldPosition>,
}
impl<'a> HeaderMut<'a> {
    /// Adds the field to the tail of the header.
    pub fn add_field<'n, 'v, F>(&mut self, field: F) -> &mut Self
    where
        F: Into<HeaderField<'n, 'v>>,
    {
        let field = field.into();

        let start = self.buf.len();
        self.buf.extend_from_slice(field.name().as_bytes());
        let end = self.buf.len();
        let name = Range { start, end };
        self.buf.extend_from_slice(b": ");

        let start = self.buf.len();
        self.buf.extend_from_slice(field.value().as_bytes());
        let end = self.buf.len();
        let value = Range { start, end };
        self.buf.extend_from_slice(b"\r\n");

        self.fields.push(HeaderFieldPosition { name, value });
        self
    }

    /// Returns an iterator over the fields in the header.
    pub fn fields(&self) -> HeaderFields {
        HeaderFields::new(self.buf, self.fields)
    }

    pub(crate) fn new(buf: &'a mut Vec<u8>, fields: &'a mut Vec<HeaderFieldPosition>) -> Self {
        HeaderMut { buf, fields }
    }
}
impl<'a> fmt::Display for HeaderMut<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for field in self.fields() {
            writeln!(f, "{}\r", field)?;
        }
        writeln!(f, "\r")?;
        Ok(())
    }
}

/// HTTP header field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HeaderField<'n, 'v> {
    name: &'n str,
    value: &'v str,
}
impl<'n, 'v> HeaderField<'n, 'v> {
    /// Makes a new `HeaderField` instance.
    ///
    /// # Errors
    ///
    /// `name` must be a "token" defined in [RFC 7230].
    /// Otherwise it will return an `ErrorKind::InvalidInput` error.
    ///
    /// `value` must be composed of "VCHAR" characters that defined in [RFC 7230].
    /// If it contains any other characters,
    /// an `ErrorKind::InvalidInput` error will be returned.
    ///
    /// [RFC 7230]: https://tools.ietf.org/html/rfc7230
    pub fn new(name: &'n str, value: &'v str) -> Result<Self> {
        track_assert!(name.bytes().all(util::is_tchar), ErrorKind::InvalidInput);
        track_assert!(value.bytes().all(util::is_vchar), ErrorKind::InvalidInput);
        Ok(HeaderField { name, value })
    }

    /// Makes a new `HeaderField` instance without any validation.
    pub unsafe fn new_unchecked(name: &'n str, value: &'v str) -> Self {
        HeaderField { name, value }
    }

    /// Returns the name of the header field.
    pub fn name(&self) -> &'n str {
        self.name
    }

    /// Returns the value of the header field.
    pub fn value(&self) -> &'v str {
        self.value
    }
}
impl<'n, 'v> fmt::Display for HeaderField<'n, 'v> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.name(), self.value())
    }
}

/// An iterator over the fields in a HTTP header.
///
/// This is created by calling `Request::header_fields` or `Response::header_fields`.
#[derive(Debug)]
pub struct HeaderFields<'a> {
    buf: &'a [u8],
    fields: slice::Iter<'a, HeaderFieldPosition>,
}
impl<'a> HeaderFields<'a> {
    pub(crate) fn new(buf: &'a [u8], fields: &'a [HeaderFieldPosition]) -> Self {
        HeaderFields {
            buf,
            fields: fields.iter(),
        }
    }

    fn field(buf: &'a [u8], f: &HeaderFieldPosition) -> HeaderField<'a, 'a> {
        unsafe {
            let name = str::from_utf8_unchecked(&buf[f.name.clone()]);
            let value = str::from_utf8_unchecked(&buf[f.value.clone()]);
            HeaderField { name, value }
        }
    }
}
impl<'a> Iterator for HeaderFields<'a> {
    type Item = HeaderField<'a, 'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.fields.next().map(|f| Self::field(&self.buf, f))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.fields.size_hint()
    }

    fn count(self) -> usize {
        self.fields.len()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.fields.nth(n).map(|f| Self::field(&self.buf, f))
    }

    fn last(self) -> Option<Self::Item> {
        let HeaderFields { buf, fields } = self;
        fields.last().map(|f| Self::field(&buf, f))
    }
}
impl<'a> ExactSizeIterator for HeaderFields<'a> {
    fn len(&self) -> usize {
        self.fields.len()
    }
}
impl<'a> DoubleEndedIterator for HeaderFields<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.fields.next_back().map(|f| Self::field(&self.buf, f))
    }
}

#[derive(Debug, Default)]
pub(crate) struct HeaderDecoder {
    field_start: usize,
    field_end: usize,
    field_decoder: HeaderFieldDecoder,
    fields: Vec<HeaderFieldPosition>,
}
impl HeaderDecoder {
    pub fn set_start_position(&mut self, n: usize) {
        self.field_start = n;
        self.field_end = n;
    }
}
impl Decode for HeaderDecoder {
    type Item = Vec<HeaderFieldPosition>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        if self.is_idle() {
            return Ok(0);
        }

        let mut offset = 0;
        while offset < buf.len() {
            let size = track!(self.field_decoder.decode(&buf[offset..], eos))?;
            offset += size;
            self.field_end += size;
            if self.field_decoder.is_idle() {
                let field = track!(self.field_decoder.finish_decoding())?;
                self.fields.push(field.add_offset(self.field_start));
                self.field_start = self.field_end;
            }
            if self.field_decoder.is_crlf_reached() {
                return Ok(offset);
            }
        }
        track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
        Ok(offset)
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        track_assert!(
            self.field_decoder.is_crlf_reached(),
            ErrorKind::IncompleteDecoding
        );
        self.field_decoder = HeaderFieldDecoder::default();
        self.field_start = 0;
        self.field_end = 0;
        let fields = mem::replace(&mut self.fields, Vec::new());
        Ok(fields)
    }

    fn requiring_bytes(&self) -> ByteCount {
        if self.is_idle() {
            ByteCount::Finite(0)
        } else {
            ByteCount::Unknown
        }
    }

    fn is_idle(&self) -> bool {
        self.field_decoder.is_crlf_reached()
    }
}

#[derive(Debug)]
pub struct HeaderFieldPosition {
    pub name: Range<usize>,
    pub value: Range<usize>,
}
impl HeaderFieldPosition {
    fn add_offset(mut self, offset: usize) -> Self {
        self.name.start += offset;
        self.name.end += offset;
        self.value.start += offset;
        self.value.end += offset;
        self
    }
}

#[derive(Debug, Default)]
struct HeaderFieldDecoder {
    peek: CopyableBytesDecoder<[u8; 2]>,
    inner: TupleDecoder<(HeaderFieldNameDecoder, HeaderFieldValueDecoder)>,
}
impl HeaderFieldDecoder {
    fn is_crlf_reached(&self) -> bool {
        self.peek.inner_ref() == b"\r\n"
    }
}
impl Decode for HeaderFieldDecoder {
    type Item = HeaderFieldPosition;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        if self.is_crlf_reached() {
            return Ok(0);
        }

        let mut offset = 0;
        if !self.peek.is_idle() {
            bytecodec_try_decode!(self.peek, offset, buf, eos);
            if self.is_crlf_reached() {
                return Ok(offset);
            }
            track!(self.inner.decode(self.peek.inner_ref(), Eos::new(false)))?;
        }

        bytecodec_try_decode!(self.inner, offset, buf, eos);
        Ok(offset)
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        track!(self.peek.finish_decoding())?;
        let (name, mut value) = track!(self.inner.finish_decoding())?;
        value.start += name.end + 1;
        value.end += name.end + 1;
        Ok(HeaderFieldPosition { name, value })
    }

    fn requiring_bytes(&self) -> ByteCount {
        if self.is_crlf_reached() {
            ByteCount::Finite(0)
        } else if !self.peek.is_idle() {
            self.peek.requiring_bytes()
        } else {
            self.inner.requiring_bytes()
        }
    }

    fn is_idle(&self) -> bool {
        self.inner.is_idle()
    }
}

#[derive(Debug, Default)]
struct HeaderFieldNameDecoder {
    end: usize,
    idle: bool,
}
impl Decode for HeaderFieldNameDecoder {
    type Item = Range<usize>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        if self.idle {
            Ok(0)
        } else if let Some(n) = buf.iter().position(|b| !util::is_tchar(*b)) {
            track_assert_eq!(buf[n] as char, ':', ErrorKind::InvalidInput; n, self.end);
            self.end += n;
            self.idle = true;
            Ok(n + 1)
        } else {
            track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
            self.end += buf.len();
            Ok(buf.len())
        }
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        track_assert!(self.is_idle(), ErrorKind::IncompleteDecoding);
        let item = Range {
            start: 0,
            end: self.end,
        };
        self.idle = false;
        self.end = 0;
        Ok(item)
    }

    fn requiring_bytes(&self) -> ByteCount {
        if self.idle {
            ByteCount::Finite(0)
        } else {
            ByteCount::Unknown
        }
    }

    fn is_idle(&self) -> bool {
        self.idle
    }
}

#[derive(Debug, Default)]
struct HeaderFieldValueDecoder {
    start: usize,
    size: usize,
    trailing_whitespaces: usize,
    remaining: ByteCount,
}
impl Decode for HeaderFieldValueDecoder {
    type Item = Range<usize>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        if self.is_idle() {
            return Ok(0);
        }

        let mut offset = 0;
        if self.size == 0 {
            offset = buf.iter()
                .position(|&b| !util::is_whitespace(b))
                .unwrap_or_else(|| buf.len());
            self.start += offset;
        }

        for &b in &buf[offset..] {
            offset += 1;
            if util::is_whitespace(b) {
                self.trailing_whitespaces += 1;
            } else if util::is_vchar(b) {
                self.size += self.trailing_whitespaces + 1;
                self.trailing_whitespaces = 0;
            } else if self.remaining == ByteCount::Finite(1) {
                track_assert_eq!(b, b'\n', ErrorKind::InvalidInput);
                self.remaining = ByteCount::Finite(0);
                return Ok(offset);
            } else {
                track_assert_eq!(b, b'\r', ErrorKind::InvalidInput);
                self.remaining = ByteCount::Finite(1);
            }
        }

        track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
        Ok(offset)
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        track_assert!(self.is_idle(), ErrorKind::IncompleteDecoding);
        let range = Range {
            start: self.start,
            end: self.start + self.size,
        };
        *self = Self::default();
        Ok(range)
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.remaining
    }
}

#[cfg(test)]
mod test {
    use bytecodec::io::IoDecodeExt;
    use bytecodec::ErrorKind;
    use std::ops::Range;

    use super::*;

    #[test]
    fn header_decoder_works() {
        let mut decoder = HeaderDecoder::default();
        let mut input = b"foo: bar\r\n111:222   \r\n\r\n".as_ref();

        let fields = track_try_unwrap!(decoder.decode_exact(&mut input));
        assert_eq!(fields.len(), 2);

        assert_eq!(fields[0].name, Range { start: 0, end: 3 });
        assert_eq!(fields[0].value, Range { start: 5, end: 8 });

        assert_eq!(fields[1].name, Range { start: 10, end: 13 });
        assert_eq!(fields[1].value, Range { start: 14, end: 17 });

        assert_eq!(
            decoder
                .decode_exact(b"foo: bar".as_ref())
                .err()
                .map(|e| *e.kind()),
            Some(ErrorKind::UnexpectedEos)
        );

        let mut decoder = HeaderDecoder::default();
        assert_eq!(
            decoder
                .decode_exact(b"fo o: bar\r\n".as_ref())
                .err()
                .map(|e| *e.kind()),
            Some(ErrorKind::InvalidInput)
        );
    }
}
