use std::io::Write;
use bytecodec::{ByteCount, Decode, DecodeExt, Encode, Eos, Error, ErrorKind, Result};
use bytecodec::combinator::Slice;

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

        let size = track!(self.0.encode(&mut buf[offset..], eos))?;
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
}
impl<E: Encode> BodyEncode for ChunkedBodyEncoder<E> {
    fn update_header(&self, header: &mut HeaderMut) -> Result<()> {
        header.add_field(HeaderField::new("transfer-encoding", "chunked")?);
        Ok(())
    }
}

// FIXME:
// - Support trailer part
// - Support chunk extension
#[derive(Debug, Default)]
pub struct ChunkedBodyDecoder<T: Decode> {
    size: ChunkSizeDecoder,
    inner: Slice<T>,
    item: Option<T::Item>,
}
impl<T: Decode> ChunkedBodyDecoder<T> {
    pub fn new(inner: T) -> Self {
        ChunkedBodyDecoder {
            size: ChunkSizeDecoder::default(),
            inner: inner.slice(),
            item: None,
        }
    }

    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}
impl<T: Decode> Decode for ChunkedBodyDecoder<T> {
    type Item = T::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        while offset < buf.len() {
            if self.inner.is_suspended() {
                let (size, item) = track!(self.size.decode(&buf[offset..], eos))?;
                offset += size;
                if let Some(n) = item {
                    if n == 0 {
                        if self.item.is_none() {
                            self.item = track!(self.inner.decode(&[][..], Eos::new(true)))?.1;
                        }
                        let item = track_assert_some!(self.item.take(), ErrorKind::Other);
                        return Ok((offset, Some(item)));
                    }
                    self.inner.set_consumable_bytes(n);
                }
            }
            if !self.inner.is_suspended() {
                let (size, item) = track!(self.inner.decode(&buf[offset..], eos))?;
                offset += size;
                if let Some(item) = item {
                    track_assert!(
                        self.inner.is_suspended(),
                        ErrorKind::Other,
                        "Too few consumption"
                    );
                    self.item = Some(item);
                }
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

#[derive(Debug, Default)]
struct ChunkSizeDecoder {
    size: u64,
    is_last: bool,
}
impl Decode for ChunkSizeDecoder {
    type Item = u64;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        for (i, b) in buf.iter().cloned().enumerate() {
            if self.is_last {
                track_assert_eq!(b as char, '\n', ErrorKind::InvalidInput);
                let size = self.size;
                self.size = 0;
                self.is_last = false;
                return Ok((i + 1, Some(size)));
            } else if b == b'\r' {
                self.is_last = true;
            } else {
                let n = match b {
                    b'0'...b'9' => b - b'0',
                    b'a'...b'f' => b - b'a' + 10,
                    b'A'...b'F' => b - b'A' + 10,
                    _ => track_panic!(
                        ErrorKind::InvalidInput,
                        "Not hexadecimal character: {}",
                        b as char
                    ),
                };
                self.size = (self.size * 16) + u64::from(n);
            }
        }
        track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
        Ok((buf.len(), None))
    }

    fn has_terminated(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        if self.is_last {
            ByteCount::Finite(1)
        } else {
            ByteCount::Unknown
        }
    }
}

#[cfg(test)]
mod test {
    use bytecodec::{Encode, EncodeExt, Eos};
    use bytecodec::bytes::RemainingBytesDecoder;
    use bytecodec::fixnum::U8Encoder;
    use bytecodec::io::IoDecodeExt;

    use super::*;

    #[test]
    fn chunked_body_encoder_works() {
        let mut body = U8Encoder::new().repeat();
        track_try_unwrap!(body.start_encoding((0..(1024 * 1024)).map(|_| b'a')));

        let eos = Eos::new(false);
        let mut buf = vec![0; 0x10000];
        let mut encoder = ChunkedBodyEncoder(body);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..1], eos));
        assert_eq!(&buf[..1], b"0");
        assert_eq!(size, 1);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..3], eos));
        assert_eq!(&buf[..3], b"000");
        assert_eq!(size, 3);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..4], eos));
        assert_eq!(&buf[..4], b"1\r\na");
        assert_eq!(size, 4);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..0xF + 2], eos));
        assert_eq!(&buf[..4], b"e\r\na");
        assert_eq!(size, 0xF + 2);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..0xF + 3], eos));
        assert_eq!(&buf[..4], b"f\r\na");
        assert_eq!(size, 0xF + 3);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..0xF + 4], eos));
        assert_eq!(&buf[..4], b"0f\r\n");
        assert_eq!(size, 0xF + 4);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..0xF + 5], eos));
        assert_eq!(&buf[..4], b"10\r\n");
        assert_eq!(size, 0xF + 5);

        let size = track_try_unwrap!(encoder.encode(&mut buf, eos));
        assert_eq!(&buf[..6], b"fffa\r\n");
        assert_eq!(size, buf.len());
        assert!(buf.iter().skip(6).all(|&b| b == b'a'));
    }

    #[test]
    fn chunked_body_decoder_works() {
        let mut decoder = ChunkedBodyDecoder::new(RemainingBytesDecoder::new());

        let input = b"1\r\na03\r\nfoo00000\r\n";
        let item = track_try_unwrap!(decoder.decode_exact(input.as_ref()));
        assert_eq!(item, b"afoo");
    }
}