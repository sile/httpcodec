use std::mem;
use std::ops::Range;
use std::slice;
use std::str;
use bytecodec::{ByteCount, Decode, Eos};
use bytecodec::bytes::CopyableBytesDecoder;
use bytecodec::combinator::Buffered;
use bytecodec::tuple::Tuple2Decoder;

use {ErrorKind, Result};
use util;

#[derive(Debug)]
pub struct HeaderFields<'a> {
    buf: &'a [u8],
    fields: slice::Iter<'a, HeaderField>,
}
impl<'a> HeaderFields<'a> {
    pub(crate) fn new(buf: &'a [u8], fields: &'a [HeaderField]) -> Self {
        HeaderFields {
            buf,
            fields: fields.iter(),
        }
    }
}
impl<'a> Iterator for HeaderFields<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        self.fields.next().map(|f| unsafe {
            let name = str::from_utf8_unchecked(&self.buf[f.name.clone()]);
            let value = str::from_utf8_unchecked(&self.buf[f.value.clone()]);
            (name, value)
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct HeaderDecoder {
    field_start: usize,
    field_end: usize,
    field_decoder: HeaderFieldDecoder,
    fields: Vec<HeaderField>,
}
impl HeaderDecoder {
    pub fn set_start_position(&mut self, n: usize) {
        self.field_start = n;
        self.field_end = n;
    }
}
impl Decode for HeaderDecoder {
    type Item = Vec<HeaderField>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        while offset < buf.len() {
            let (size, item) = track!(self.field_decoder.decode(&buf[offset..], eos))?;
            offset += size;
            self.field_end += size;
            if let Some(field) = item {
                self.fields.push(field.add_offset(self.field_start));
                self.field_start = self.field_end;
            }
            if self.field_decoder.has_terminated() {
                self.field_decoder = HeaderFieldDecoder::default();
                self.field_start = 0;
                self.field_end = 0;
                let fields = mem::replace(&mut self.fields, Vec::new());
                return Ok((offset, Some(fields)));
            }
        }
        Ok((offset, None))
    }

    fn has_terminated(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}

#[derive(Debug)]
pub(crate) struct HeaderField {
    name: Range<usize>,
    value: Range<usize>,
}
impl HeaderField {
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
    peek: Buffered<CopyableBytesDecoder<[u8; 2]>>,
    inner: Tuple2Decoder<HeaderFieldNameDecoder, HeaderFieldValueDecoder>,
}
impl Decode for HeaderFieldDecoder {
    type Item = HeaderField;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        if !self.peek.has_item() {
            offset += track!(self.peek.decode(buf, eos))?.0;
            if let Some(peek) = self.peek.get_item().map(|b| b.as_ref()) {
                if peek == b"\r\n" {
                    return Ok((offset, None));
                }
                track!(self.inner.decode(peek, Eos::new(false)))?;
            } else {
                return Ok((offset, None));
            }
        }

        let (size, item) = track!(self.inner.decode(&buf[offset..], eos))?;
        offset += size;
        let item = item.map(|(name, mut value)| {
            self.peek.take_item();
            value.start += name.end + 1;
            value.end += name.end + 1;
            HeaderField { name, value }
        });
        Ok((offset, item))
    }

    fn has_terminated(&self) -> bool {
        self.peek.get_item() == Some(&[b'\r', b'\n'])
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}

#[derive(Debug, Default)]
struct HeaderFieldNameDecoder {
    end: usize,
}
impl Decode for HeaderFieldNameDecoder {
    type Item = Range<usize>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        if let Some(n) = buf.iter().position(|b| !util::is_tchar(*b)) {
            track_assert_eq!(buf[n] as char, ':', ErrorKind::InvalidInput; n, self.end);
            let end = self.end + n;
            self.end = 0;
            Ok((n + 1, Some(Range { start: 0, end })))
        } else {
            track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
            self.end += buf.len();
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

#[derive(Debug, Default)]
struct HeaderFieldValueDecoder {
    start: usize,
    size: usize,
    trailing_whitespaces: usize,
    before_newline: bool,
}
impl Decode for HeaderFieldValueDecoder {
    type Item = Range<usize>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
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
            } else if self.before_newline {
                track_assert_eq!(b, b'\n', ErrorKind::InvalidInput);
                let range = Range {
                    start: self.start,
                    end: self.start + self.size,
                };
                *self = Self::default();
                return Ok((offset, Some(range)));
            } else {
                track_assert_eq!(b, b'\r', ErrorKind::InvalidInput);
                self.before_newline = true;
            }
        }

        track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
        Ok((offset, None))
    }

    fn has_terminated(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}

#[cfg(test)]
mod test {
    use std::ops::Range;
    use bytecodec::io::IoDecodeExt;

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
    }
}
