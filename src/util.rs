use bytecodec::bytes::CopyableBytesDecoder;
use bytecodec::{ByteCount, Decode, Eos, ErrorKind, Result};

#[derive(Debug, Default)]
pub struct SpaceDecoder(CopyableBytesDecoder<[u8; 1]>);
impl Decode for SpaceDecoder {
    type Item = ();

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        track!(self.0.decode(buf, eos))
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        let b = track!(self.0.finish_decoding())?;
        track_assert_eq!(b[0], b' ', ErrorKind::InvalidInput);
        Ok(())
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }

    fn is_idle(&self) -> bool {
        self.0.is_idle()
    }
}

#[derive(Debug, Default)]
pub struct CrlfDecoder(CopyableBytesDecoder<[u8; 2]>);
impl Decode for CrlfDecoder {
    type Item = ();

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        track!(self.0.decode(buf, eos))
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        let b = track!(self.0.finish_decoding())?;
        track_assert_eq!(b, [b'\r', b'\n'], ErrorKind::InvalidInput);
        Ok(())
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }

    fn is_idle(&self) -> bool {
        self.0.is_idle()
    }
}

pub fn is_whitespace(b: u8) -> bool {
    match b {
        b' ' | b'\t' => true,
        _ => false,
    }
}

// https://tools.ietf.org/html/rfc7230#section-3.2.6
pub fn is_tchar(b: u8) -> bool {
    match b {
        b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_'
        | b'`' | b'|' | b'~' => true,
        _ => is_digit(b) || is_alpha(b),
    }
}

pub fn is_digit(b: u8) -> bool {
    b'0' <= b && b <= b'9'
}

pub fn is_alpha(b: u8) -> bool {
    (b'a' <= b && b <= b'z') || (b'A' <= b && b <= b'Z')
}

pub fn is_vchar(b: u8) -> bool {
    0x21 <= b && b <= 0x7E
}
