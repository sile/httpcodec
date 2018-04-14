use bytecodec::{ByteCount, Decode, Eos};
use bytecodec::bytes::CopyableBytesDecoder;
use bytecodec::combinator::Buffered;

use {ErrorKind, Result};
use token::{PushByte, Token, TokenDecoder};
use util::{is_whitespace, WithColonDecoder, WithCrlfDecoder, WithOwsDecoder};

#[derive(Debug, Default)]
pub struct HeaderDecoder<N, V>
where
    N: Decode,
    V: Decode,
{
    buf: CopyableBytesDecoder<[u8; 2]>,
    field: WithCrlfDecoder<HeaderFieldDecoder<N, V>>,
    decoding_field: bool,
    has_terminated: bool,
}
impl<N, V> Decode for HeaderDecoder<N, V>
where
    N: Decode,
    V: Decode,
{
    type Item = HeaderField<N::Item, V::Item>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        track_assert!(!self.has_terminated, ErrorKind::DecoderTerminated);

        let mut offset = 0;
        if !self.decoding_field {
            let (size, item) = track!(self.buf.decode(buf, eos))?;
            offset += size;
            match item.as_ref().map(|b| &b[..]) {
                Some(b"\r\n") => {
                    self.has_terminated = true;
                    return Ok((offset, None));
                }
                Some(peeked) => {
                    self.decoding_field = true;
                    let _ = track!(self.field.decode(peeked, Eos::new(false)))?;
                }
                None => return Ok((offset, None)),
            }
        }

        let (size, item) = track!(self.field.decode(&buf[offset..], eos))?;
        offset += size;
        if item.is_some() {
            self.decoding_field = false;
        }
        Ok((offset, item))
    }

    fn has_terminated(&self) -> bool {
        self.has_terminated
    }

    fn requiring_bytes(&self) -> ByteCount {
        // TODO:
        if self.has_terminated {
            ByteCount::Finite(0)
        } else {
            ByteCount::Unknown
        }
    }
}

#[derive(Debug)]
pub struct HeaderField<N, V> {
    pub name: N,
    pub value: V,
}

#[derive(Debug, Default)]
pub struct HeaderFieldDecoder<N, V>
where
    N: Decode,
    V: Decode,
{
    name: Buffered<WithColonDecoder<N>>,
    value: WithOwsDecoder<V>,
}
impl<N, V> Decode for HeaderFieldDecoder<N, V>
where
    N: Decode,
    V: Decode,
{
    type Item = HeaderField<N::Item, V::Item>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        if !self.name.has_item() {
            offset += track!(self.name.decode(buf, eos))?.0;
            if !self.name.has_item() {
                return Ok((offset, None));
            }
        }

        let (size, item) = track!(self.value.decode(&buf[offset..], eos))?;
        offset += size;
        if let Some(value) = item {
            let field = HeaderField {
                name: self.name.take_item().expect("Never fails"),
                value,
            };
            Ok((offset, Some(field)))
        } else {
            Ok((offset, None))
        }
    }

    fn has_terminated(&self) -> bool {
        self.name.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        // TODO:
        ByteCount::Unknown
    }
}

#[derive(Debug)]
pub struct HeaderFieldName<B>(Token<B>);
impl<B: AsRef<[u8]>> AsRef<[u8]> for HeaderFieldName<B> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(Debug, Default)]
pub struct HeaderFieldNameDecoder<B>(TokenDecoder<B>);
impl<B: PushByte> Decode for HeaderFieldNameDecoder<B> {
    type Item = HeaderFieldName<B>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let (size, item) = track!(self.0.decode(buf, eos))?;
        Ok((size, item.map(HeaderFieldName)))
    }

    fn has_terminated(&self) -> bool {
        self.0.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}

#[derive(Debug)]
pub struct HeaderFieldValue<B>(B);
impl<B: AsRef<[u8]>> AsRef<[u8]> for HeaderFieldValue<B> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(Debug, Default)]
pub struct HeaderFieldValueDecoder<B> {
    inner: B,
    is_not_first: bool,
    whitespace_count: usize,
}
impl<B: PushByte> Decode for HeaderFieldValueDecoder<B> {
    type Item = HeaderFieldValue<B>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        for i in 0..buf.len() {
            let is_target = if self.is_not_first {
                is_vchar(buf[i]) || is_whitespace(buf[i])
            } else {
                is_vchar(buf[i])
            };

            if !is_target {
                for _ in 0..self.whitespace_count {
                    self.inner.pop_byte();
                }
                self.whitespace_count = 0;
                let item = self.inner.take_bytes();
                return Ok((i, Some(HeaderFieldValue(item))));
            }
            track_assert!(
                self.inner.push_byte(buf[i]),
                ErrorKind::Other,
                "Buffer full"
            );
            self.is_not_first = true;
            if is_whitespace(buf[i]) {
                self.whitespace_count += 1;
            } else {
                self.whitespace_count = 0;
            }
        }

        let item = if eos.is_reached() {
            for _ in 0..self.whitespace_count {
                self.inner.pop_byte();
            }
            self.whitespace_count = 0;
            Some(HeaderFieldValue(self.inner.take_bytes()))
        } else {
            None
        };
        Ok((buf.len(), item))
    }

    fn has_terminated(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}

fn is_vchar(b: u8) -> bool {
    0x21 <= b && b <= 0x7E
}

#[cfg(test)]
mod test {
    use bytecodec::io::IoDecodeExt;

    use super::*;

    #[test]
    fn header_decoder_works() {
        let mut decoder: HeaderDecoder<
            HeaderFieldNameDecoder<Vec<u8>>,
            HeaderFieldValueDecoder<Vec<u8>>,
        > = HeaderDecoder::default();

        let mut input = b"foo: bar\r\n111:222   \r\n\r\n".as_ref();

        let field = track_try_unwrap!(decoder.decode_exact(&mut input));
        assert_eq!(field.name.as_ref(), b"foo");
        assert_eq!(field.value.as_ref(), b"bar");

        let field = track_try_unwrap!(decoder.decode_exact(&mut input));
        assert_eq!(field.name.as_ref(), b"111");
        assert_eq!(field.value.as_ref(), b"222");

        assert_eq!(
            decoder.decode_exact(&mut input).err().map(|e| *e.kind()),
            Some(ErrorKind::DecoderTerminated)
        );
    }
}
