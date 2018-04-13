use bytecodec::{ByteCount, Decode, Eos};
use bytecodec::combinator::Buffered;

use {ErrorKind, Result};
use token::{PushByte, Token, TokenDecoder};
use util::{is_whitespace, WithColonDecoder, WithOwsDecoder};

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

        let (size, item) = track!(self.value.decode(buf, eos))?;
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

#[derive(Debug, Default)]
pub struct HeaderFieldValueDecoder<B> {
    inner: B,
    is_not_first: bool,
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
                let item = self.inner.take_bytes();
                return Ok((i, Some(HeaderFieldValue(item))));
            }
            track_assert!(
                self.inner.push_byte(buf[i]),
                ErrorKind::Other,
                "Buffer full"
            );
            self.is_not_first = true;
        }

        let item = if eos.is_reached() {
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
