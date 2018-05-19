use bytecodec::bytes::BytesEncoder;
use bytecodec::combinator::Slice;
use bytecodec::{ByteCount, Decode, DecodeExt, Encode, Eos, Error, ErrorKind, Result};
use std::io::Write;

use util::CrlfDecoder;
use {BodyEncode, HeaderField, HeaderMut};

#[derive(Debug, Default)]
pub struct ChunkedBodyEncoder<E> {
    inner: E,
    delim: BytesEncoder<[u8; 2]>,
    last: BytesEncoder<[u8; 7]>,
}
impl<E> ChunkedBodyEncoder<E> {
    pub fn new(inner: E) -> Self {
        ChunkedBodyEncoder {
            inner,
            delim: BytesEncoder::new(),
            last: BytesEncoder::new(),
        }
    }
}
impl<E: Encode> Encode for ChunkedBodyEncoder<E> {
    type Item = E::Item;

    fn encode(&mut self, mut buf: &mut [u8], eos: Eos) -> Result<usize> {
        if !self.last.is_idle() {
            return track!(self.last.encode(buf, eos));
        }
        if !self.delim.is_idle() {
            let mut size = track!(self.delim.encode(buf, eos))?;
            if self.delim.is_idle() && !self.inner.is_idle() {
                size += track!(self.encode(&mut buf[size..], eos))?;
            }
            return Ok(size);
        }
        if self.inner.is_idle() {
            return Ok(0);
        }

        if buf.len() < 4 {
            for b in &mut buf[..] {
                *b = b'0';
            }
            return Ok(buf.len());
        }

        let mut offset = if buf.len() <= 3 + 0xF {
            3
        } else if buf.len() <= 4 + 0xFF {
            4
        } else if buf.len() <= 5 + 0xFFF {
            5
        } else if buf.len() <= 6 + 0xFFFF {
            6
        } else if buf.len() <= 7 + 0xF_FFFF {
            7
        } else if buf.len() <= 8 + 0xFF_FFFF {
            8
        } else if buf.len() <= 9 + 0xFFF_FFFF {
            9
        } else {
            10
        };

        let size = track!(self.inner.encode(&mut buf[offset..], eos))?;
        if size == 0 && !self.inner.is_idle() {
            // The encoder is suspended for some reasons
            return Ok(0);
        }

        track!(write!(buf, "{:01$x}\r\n", size, offset - 2).map_err(Error::from))?;
        if self.inner.is_idle() && size != 0 {
            track!(self.last.start_encoding(*b"\r\n0\r\n\r\n"))?;
        } else {
            track!(self.delim.start_encoding(*b"\r\n"))?;
        }
        offset += track!(self.encode(&mut buf[size..], eos))?;

        Ok(offset + size)
    }

    fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
        track_assert!(self.is_idle(), ErrorKind::EncoderFull);
        track!(self.inner.start_encoding(item))
    }

    fn is_idle(&self) -> bool {
        self.inner.is_idle() && self.delim.is_idle() && self.last.is_idle()
    }

    fn requiring_bytes(&self) -> ByteCount {
        if self.is_idle() {
            ByteCount::Finite(0)
        } else {
            ByteCount::Unknown
        }
    }
}
impl<E: Encode> BodyEncode for ChunkedBodyEncoder<E> {
    fn update_header(&self, header: &mut HeaderMut) -> Result<()> {
        header.add_field(HeaderField::new("Transfer-Encoding", "chunked")?);
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
    crlf: Option<CrlfDecoder>,
    eos: bool,
}
impl<T: Decode> ChunkedBodyDecoder<T> {
    pub fn new(inner: T) -> Self {
        ChunkedBodyDecoder {
            size: ChunkSizeDecoder::default(),
            inner: inner.slice(),
            crlf: None,
            eos: false,
        }
    }

    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}
impl<T: Decode> Decode for ChunkedBodyDecoder<T> {
    type Item = T::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        if self.is_idle() {
            return Ok(0);
        }

        let mut offset = 0;
        while offset < buf.len() {
            if self.inner.is_suspended() {
                if let Some(crlf) = self.crlf.as_mut() {
                    bytecodec_try_decode!(crlf, offset, buf, eos);
                    if self.eos {
                        return Ok(offset);
                    }
                }
                self.crlf = None;

                bytecodec_try_decode!(self.size, offset, buf, eos);
                let n = track!(self.size.finish_decoding())?;
                if n == 0 {
                    self.eos = true;
                }
                self.inner.set_consumable_bytes(n);
                self.crlf = Some(CrlfDecoder::default());
            }
            if !self.inner.is_suspended() {
                offset += track!(self.inner.decode(&buf[offset..], eos))?;
            }
        }
        track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
        Ok(offset)
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        if !self.inner.is_idle() {
            track!(self.inner.decode(&[][..], Eos::new(true)))?;
            track_assert!(self.inner.is_idle(), ErrorKind::UnexpectedEos);
        }
        let item = track!(self.inner.finish_decoding())?;
        track_assert!(
            self.inner.is_suspended(),
            ErrorKind::Other,
            "Too few consumption"
        );
        self.eos = false;
        self.crlf = None;
        Ok(item)
    }

    fn requiring_bytes(&self) -> ByteCount {
        if self.is_idle() {
            ByteCount::Finite(0)
        } else {
            ByteCount::Unknown
        }
    }

    fn is_idle(&self) -> bool {
        self.eos && self.crlf.as_ref().map_or(false, |x| x.is_idle())
    }
}

#[derive(Debug, Default)]
struct ChunkSizeDecoder {
    size: u64,
    remaining: ByteCount,
}
impl Decode for ChunkSizeDecoder {
    type Item = u64;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        if self.is_idle() {
            return Ok(0);
        }

        for (i, b) in buf.iter().cloned().enumerate() {
            if self.remaining == ByteCount::Finite(1) {
                track_assert_eq!(b as char, '\n', ErrorKind::InvalidInput);
                self.remaining = ByteCount::Finite(0);
                return Ok(i + 1);
            } else if b == b'\r' {
                self.remaining = ByteCount::Finite(1);
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
        Ok(buf.len())
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        track_assert_eq!(
            self.remaining,
            ByteCount::Finite(0),
            ErrorKind::IncompleteDecoding
        );
        let size = self.size;
        self.remaining = ByteCount::Unknown;
        self.size = 0;
        Ok(size)
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.remaining
    }

    fn is_idle(&self) -> bool {
        self.remaining == ByteCount::Finite(0)
    }
}

#[cfg(test)]
mod test {
    use bytecodec::bytes::RemainingBytesDecoder;
    use bytecodec::fixnum::U8Encoder;
    use bytecodec::io::IoDecodeExt;
    use bytecodec::{Encode, EncodeExt, Eos};

    use super::*;

    #[test]
    fn chunked_body_encoder_works() {
        let mut body = U8Encoder::new().repeat();
        track_try_unwrap!(body.start_encoding((0..(1024 * 1024)).map(|_| b'a')));

        let eos = Eos::new(false);
        let mut buf = vec![0; 0x10000];
        let mut encoder = ChunkedBodyEncoder::new(body);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..1], eos));
        assert_eq!(&buf[..1], b"0");
        assert_eq!(size, 1);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..3], eos));
        assert_eq!(&buf[..3], b"000");
        assert_eq!(size, 3);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..4], eos));
        assert_eq!(&buf[..4], b"1\r\na");
        assert_eq!(size, 4);
        assert_eq!(track_try_unwrap!(encoder.encode(&mut buf[..2], eos)), 2);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..0xF + 2], eos));
        assert_eq!(&buf[..4], b"e\r\na");
        assert_eq!(size, 0xF + 2);
        assert_eq!(track_try_unwrap!(encoder.encode(&mut buf[..2], eos)), 2);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..0xF + 3], eos));
        assert_eq!(&buf[..4], b"f\r\na");
        assert_eq!(size, 0xF + 3);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..0xF + 4 + 2], eos));
        assert_eq!(&buf[..6], b"\r\n0f\r\n");
        assert_eq!(size, 0xF + 4 + 2);
        assert_eq!(track_try_unwrap!(encoder.encode(&mut buf[..2], eos)), 2);

        let size = track_try_unwrap!(encoder.encode(&mut buf[..0xF + 5], eos));
        assert_eq!(&buf[..4], b"10\r\n");
        assert_eq!(size, 0xF + 5);
        assert_eq!(track_try_unwrap!(encoder.encode(&mut buf[..2], eos)), 2);

        let size = track_try_unwrap!(encoder.encode(&mut buf, eos));
        assert_eq!(&buf[..6], b"fffa\r\n");
        assert_eq!(size, buf.len());
        assert!(buf.iter().skip(6).all(|&b| b == b'a'));
    }

    #[test]
    fn chunked_body_decoder_works() {
        let mut decoder = ChunkedBodyDecoder::new(RemainingBytesDecoder::new());

        let input = b"1\r\na\r\n03\r\nfoo\r\n00000\r\n\r\n";
        let item = track_try_unwrap!(decoder.decode_exact(input.as_ref()));
        assert_eq!(item, b"afoo");

        let input = b"1\r\na\r\n1\r\nb\r\n1\r\nc\r\n0\r\n\r\n";
        let item = track_try_unwrap!(decoder.decode_exact(input.as_ref()));
        assert_eq!(item, b"abc");
    }
}
