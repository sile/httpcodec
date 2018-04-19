use std::io::Write;
use bytecodec::{ByteCount, Decode, Encode, Eos, Error, Result};

use {BodyEncode, HeaderField, HeaderMut};

#[derive(Debug, Default)]
pub struct ChunkedBodyEncoder<E>(pub E);
impl<E: Encode> Encode for ChunkedBodyEncoder<E> {
    type Item = E::Item;

    fn encode(&mut self, mut buf: &mut [u8], eos: Eos) -> Result<usize> {
        if buf.len() < 4 {
            for b in &mut buf[..] {
                *b = b'0';
            }
            return Ok(buf.len());
        }

        // TODO: test
        let offset = if buf.len() <= 3 + 0xF {
            3
        } else if buf.len() <= 4 + 0xFF {
            4
        } else if buf.len() <= 5 + 0xFFF {
            5
        } else if buf.len() <= 6 + 0xFFFF {
            6
        } else if buf.len() <= 7 + 0xFFFF_F {
            7
        } else if buf.len() <= 8 + 0xFFFF_FF {
            8
        } else if buf.len() <= 9 + 0xFFFF_FFF {
            9
        } else {
            10
        };

        let size = track!(self.encode(&mut buf[offset..], eos))?;
        track!(write!(buf, "{:01$x}\r\n", size, offset - 2).map_err(Error::from))?;
        Ok(offset + size)
    }

    fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
        track!(self.0.start_encoding(item))
    }

    fn is_idle(&self) -> bool {
        self.0.is_idle()
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }

    fn cancel(&mut self) -> Result<()> {
        track!(self.0.cancel())
    }
}
impl<E: Encode> BodyEncode for ChunkedBodyEncoder<E> {
    fn update_header(&self, header: &mut HeaderMut) {
        unsafe {
            header.add_field(HeaderField::new_unchecked("transfer-encoding", "chunked"));
        }
    }
}

// FIXME:
// - Support trailer part
// - Support chunk extension
#[derive(Debug, Default)]
pub struct ChunkedBodyDecoder<T>(pub T);
impl<T: Decode> Decode for ChunkedBodyDecoder<T> {
    type Item = T::Item;

    fn decode(&mut self, _buf: &[u8], _eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        unimplemented!()
    }

    fn has_terminated(&self) -> bool {
        unimplemented!()
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}
